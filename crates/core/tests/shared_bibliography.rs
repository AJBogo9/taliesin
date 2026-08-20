//! Site-level shared `bibliography:` (item 163): a `.bib` declared once in `_site.yml` is
//! inherited by every page, and a page's own `bibliography:` is a layer laid OVER it rather
//! than a replacement. Pins `corpus/shared-bib/` (`index.tmd` cites a shared key with no
//! `bibliography:` of its own; `notes.tmd` overrides one shared key and adds another).
//!
//! The read-only hygiene half is the duplicate-key check, tested against temp sites in
//! `crates/core/src/site/bibliography.rs` since it needs a deliberately broken `.bib` the
//! corpus must not carry. It also reported "declared but never cited" at both scopes until
//! that lint was cut on 2026-08-20.

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn site() -> Site {
    Site::discover(&corpus_dir().join("shared-bib"))
}

#[test]
fn a_page_with_no_bibliography_of_its_own_cites_the_project_wide_one() {
    let html = site().render_page("index.tmd").expect("index renders");
    // Resolved, not left as a raw key: a resolved citation is a numbered link into the
    // reference list, and the list carries the formatted entry.
    assert!(
        html.contains("href=\"#ref-shannon1948\""),
        "the shared key resolves to a reference link:\n{html}"
    );
    assert!(
        html.contains("Mathematical Theory of Communication"),
        "the shared entry is formatted into this page's reference list:\n{html}"
    );
    assert!(
        !html.contains("<code>shannon1948</code>"),
        "an unresolved citation renders as a bare <code> key; this one must not"
    );
}

#[test]
fn a_pages_own_entry_wins_over_the_shared_one_with_the_same_key() {
    let html = site().render_page("notes.tmd").expect("notes renders");
    // `turing1950` is defined in BOTH references.bib and local.bib. The page's own copy
    // is the corrected reprint; the shared one is not. Layer order is the feature.
    assert!(
        html.contains("Computing Machinery and Intelligence (corrected reprint)"),
        "the page's own entry overrides the shared one:\n{html}"
    );
    // And the shared record must not ALSO be rendered. The two titles share a prefix, so a
    // plain `!contains` of the shared title would match the page's own row and pass
    // vacuously; assert instead that EVERY occurrence of the shared title is the page's
    // longer one. Derived from the emitted row, not guessed: the reference list renders
    // `…, “<title>,” <em>journal</em>, <year>.`
    const SHARED_TITLE: &str = "Computing Machinery and Intelligence";
    for (i, _) in html.match_indices(SHARED_TITLE) {
        let tail = &html[i + SHARED_TITLE.len()..];
        assert!(
            tail.starts_with(" (corrected reprint)"),
            "the shared entry's own title leaked into the page — one key, one row:\n\
             …{}",
            &tail[..tail.len().min(80)]
        );
    }
    assert_eq!(
        html.matches("id=\"ref-turing1950\"").count(),
        1,
        "the overridden key gets exactly one reference row"
    );
}

#[test]
fn a_key_defined_only_by_the_page_still_resolves_alongside_the_shared_layer() {
    let html = site().render_page("notes.tmd").expect("notes renders");
    assert!(
        html.contains("href=\"#ref-hamming1950\"") && html.contains("Error Detecting"),
        "a page-only key resolves when a shared layer is also present:\n{html}"
    );
}

#[test]
fn a_page_opened_on_its_own_still_inherits_the_project_bibliography() {
    // `preview post.tmd` / `check post.tmd` / the LSP go through `render_single_doc`, which
    // has no `Site`. Without the nearest-`_site.yml` read, one source rendered as two
    // different documents: resolved under `preview <dir>`, raw keys under
    // `preview <page.tmd>` — and previewing one post of a series is the workflow the shared
    // key exists for.
    let page = corpus_dir().join("shared-bib").join("index.tmd");
    let src = std::fs::read_to_string(&page).expect("page reads");
    let doc = taliesin_core::render_single_doc(&src, page.parent().unwrap());
    let html = doc.body_html();
    assert!(
        html.contains("href=\"#ref-shannon1948\""),
        "a directly-invoked page resolves the project bibliography:\n{html}"
    );
    assert!(
        !html.contains("<code>shannon1948</code>"),
        "and does not fall back to a raw key:\n{html}"
    );
}

#[test]
fn the_corpus_site_is_clean_under_the_shared_bibliography_lint() {
    // The pin is only a pin if it is green: nothing in the shared `.bib` is duplicated, so
    // a regression that made the lint fire on correct input shows up here rather than as
    // noise in a real project. (It checked the "declared but never cited" half too until
    // that lint was cut on 2026-08-20.)
    let site = site();
    let site_level = site.validate_shared_bibliography();
    assert!(
        site_level.is_empty(),
        "corpus/shared-bib declares nothing duplicated: {:?}",
        site_level.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}
