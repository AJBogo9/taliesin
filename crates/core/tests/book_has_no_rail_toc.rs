//! Item 76 (owner ruling 2026-07-27): **a book has no right-rail "on this page" TOC.**
//!
//! This reverses the 2026-07-06 "keep both nav surfaces" decision. The measurement behind
//! the ruling: on a 1440 px book chapter the Chapters drawer auto-expands the current
//! chapter and lists it to h3, while the right rail showed h2 only — so the drawer is not
//! a substitute for the rail, it is *strictly more detailed*. What removal costs is
//! scrollspy ("you are here" while reading); the ruling accepts that.
//!
//! **The removal is book-scoped, and that scope is the thing this file pins.** A website
//! page and a single document keep the rail and keep `toc-spy.js`. (The mobile floating
//! "Contents" pill that used to ride along with the rail — `TOC_SHEET_MARKUP`, one copy
//! all four assemblers emitted — was itself deleted 2026-08-03, visual minimalism pass:
//! it duplicated the topbar. That is a separate, non-book-scoped removal; this file no
//! longer has anything to pin about it.) So a change that deletes the rail everywhere
//! passes half of this file and fails the other half, and a change that forgets books
//! entirely fails the first half.

mod common;
use common::corpus_dir;
use taliesin_core::{Site, render};

fn tarn() -> Site {
    Site::discover(&corpus_dir().join("tarn"))
}

/// `corpus/tarn` sets a site-wide `toc: true` and `install.tmd` carries 8 `##` headings,
/// far above `MIN_TOC_HEADINGS` — before item 76 this was the two-column chapter. It must
/// now render as one reading column.
#[test]
fn a_long_book_chapter_renders_no_right_rail_toc() {
    let install = tarn().render_page("install.tmd").expect("install renders");
    assert!(
        !install.contains("id=\"TOC\""),
        "a book chapter must not emit the right-rail TOC nav: {install}"
    );
    // The layout half: without a rail there is nothing to reserve a track for, so the
    // book's grid classes must not carry `has-toc` either. Needle the full class attribute
    // — the bundled CSS ships `.has-toc` selectors on every page, so a bare `contains`
    // would pass vacuously.
    for cls in [
        "class=\"tali-book-main has-toc\"",
        "class=\"tali-book-inner has-toc\"",
    ] {
        assert!(
            !install.contains(cls),
            "the book layout must not reserve a TOC track ({cls}): {install}"
        );
    }
}

/// The rail's runtime goes with it: the scrollspy that lit it up, and the skip-to-TOC
/// link that pointed at it. Neither has anything to drive. (The mobile floating
/// "Contents" pill that used to ride along with the rail on any page, book or not, was
/// deleted separately 2026-08-03 — see the module doc — so it is not this test's to pin.)
#[test]
fn a_book_chapter_ships_no_scrollspy_or_toc_skip_link() {
    let install = tarn().render_page("install.tmd").expect("install renders");
    // A full emitted tag, never a bare class or id: the page inlines the whole CSS+JS
    // payload, so `tali-skip-toc` alone matches `base.css`'s `.tali-skip-toc:focus` rule
    // on a page that renders no skip link at all.
    assert!(
        !install.contains("<a class=\"tali-skip tali-skip-toc\""),
        "book chapter still ships the skip-to-TOC link: {install}"
    );
    // `toc-spy.js` is inlined verbatim, so pin a distinctive line of its own source. The
    // guard above it fails loudly if that line is ever edited away, rather than letting
    // this quietly become an assertion about nothing.
    let spy_needle = "window.taliInitTocSpy";
    assert!(
        render::TOC_SPY_JS.contains(spy_needle),
        "toc-spy.js must still contain `{spy_needle}`, or the check below is vacuous"
    );
    assert!(
        !install.contains(spy_needle),
        "book chapter still inlines the scrollspy: {install}"
    );
}

/// A per-page `toc: true` cannot bring the rail back. `page_toc` short-circuits on the
/// book before it consults the page's own front matter, so the key is inert in a book
/// rather than a hidden way to reinstate a removed surface.
#[test]
fn an_explicit_page_level_toc_true_does_not_reinstate_the_rail_in_a_book() {
    let site = tarn();
    let page = site.page("install.tmd").expect("install is a page");
    let src = std::fs::read_to_string(&page.input).expect("install.tmd reads");
    let doc = render::render_document(&src);
    // Guard the guard: `install.tmd` must actually clear the heading gate, or both
    // assertions below would hold for the boring reason. (`MIN_TOC_HEADINGS` is 3 and
    // private; count the `##` sections the gate counts.)
    assert!(
        src.lines().filter(|l| l.starts_with("## ")).count() >= 3,
        "install.tmd is supposed to be a long chapter, above MIN_TOC_HEADINGS"
    );
    assert!(
        !site.page_toc(page, Some(true), &doc.blocks),
        "an explicit `toc: true` must stay inert in a book"
    );
    assert!(
        !site.page_toc(page, None, &doc.blocks),
        "and the site-wide `toc: true` must not reach a book chapter either"
    );
}

/// The negative control, and the reason this file exists rather than a one-line deletion:
/// the removal is **book-scoped**. `corpus/tech-blog` is a website (no `chapters:`), so a
/// long post keeps its rail and its scrollspy. (Its mobile "Contents" pill is gone too,
/// but that removal is global, not book-scoped — see the module doc — so it is not part
/// of this negative control.)
#[test]
fn a_website_page_keeps_its_rail_toc_and_scrollspy() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let post = site
        .render_page("posts/KL-divergence/index.tmd")
        .expect("KL-divergence post renders");
    assert!(
        post.contains("id=\"TOC\""),
        "a website page must keep the right-rail TOC: {post}"
    );
    assert!(
        post.contains("class=\"tali-site-main has-toc\""),
        "…and its two-column layout: {post}"
    );
    assert!(
        post.contains("tali-toc-active"),
        "…and the scrollspy that drives it: {post}"
    );
}
