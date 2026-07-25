//! B2: the book landing-page auto-TOC (the hardcover Contents list).
//!
//! Renders the corpus `demo-book/` and pins: the landing (`index.html`) grows a
//! `tali-book-toc` Contents nav listing the numbered chapters + the "Core" part divider,
//! in order, each linking its `.html`; a chapter with a `description:` shows its blurb and
//! one without shows none; the landing does not link to itself; a chapter page renders no
//! landing TOC; and a plain (non-book) website landing renders none either.

use taliesin_core::Site;

mod common;
use common::{TempProj, corpus_dir};

#[test]
fn the_book_landing_lists_chapters_parts_and_a_blurb() {
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let html = site.render_page("index.tmd").expect("renders");
    assert!(
        html.contains("data-block-id=\"tali-book-toc\""),
        "the book landing must grow a Contents nav: {html}"
    );
    assert!(html.contains(">Contents<"));
    // Numbered chapters, linked, in order, with the part divider between them.
    assert!(html.contains("href=\"intro.html\"") && html.contains(">Introduction<"));
    assert!(html.contains("class=\"tali-btoc-part\">Core<"));
    assert!(html.contains("href=\"methods.html\"") && html.contains(">Methodology<"));
    let i_intro = html.find("intro.html").unwrap();
    let i_core = html.find(">Core<").unwrap();
    let i_methods = html.find("methods.html").unwrap();
    assert!(
        i_intro < i_core && i_core < i_methods,
        "chapters/part out of order on the landing"
    );
    // methods.tmd sets a description:, results.tmd does not.
    assert!(
        html.contains("<p class=\"tali-btoc-desc\">The one-equation core"),
        "the chapter with a description must show its blurb: {html}"
    );
    // The landing must not list itself. Key on the TOC's own link class: `index.html`
    // also appears as the book chrome's home/brand link, which is not the Contents list.
    assert!(
        !html.contains("class=\"tali-btoc-link\" href=\"index.html\""),
        "the landing's Contents list must not link to itself"
    );
}

#[test]
fn a_chapter_page_has_no_landing_toc() {
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let html = site.render_page("methods.tmd").expect("renders");
    assert!(
        !html.contains("tali-book-toc"),
        "only the landing page carries the Contents nav, not a chapter"
    );
}

#[test]
fn a_non_book_website_landing_has_no_book_toc() {
    let d = TempProj::new();
    d.file(
        "_site.yml",
        "title: \"Plain Site\"\nnav:\n  - { text: Home, href: index.tmd }\n",
    );
    d.file(
        "index.tmd",
        "---\ntitle: Home\n---\n\n# Hi\n\nA website, not a book.\n",
    );
    let site = Site::discover(&d.0);
    let html = site.render_page("index.tmd").expect("renders");
    assert!(
        !html.contains("tali-book-toc"),
        "a non-book website landing must not grow a book Contents nav"
    );
}

/// The Continue slot exactly as emitted. Every needle about this feature must be the
/// whole tag, never a class or attribute name on its own: the enhancer bundle and the
/// stylesheet are INLINED into every page's `<head>`, so `contains("data-tali-continue")`
/// and `contains("tali-book-continue")` are both satisfied by a page that renders no slot
/// at all (the JS carries the selector, the CSS carries the rule).
const CONTINUE_SLOT: &str = r#"<p class="tali-book-continue" data-tali-continue hidden></p>"#;

/// The `tali-book-toc` block's own HTML, without the page chrome and (crucially) without
/// the inlined enhancer bundle: `15-reading-progress.js` ships the literal
/// "Continue reading" in every page's `<head>`, so a whole-page `contains` for it is
/// satisfied by the stylesheet-and-script payload of a page that renders nothing.
fn landing_toc_block(html: &str) -> &str {
    let start = html
        .find(r#"<nav class="tali-book-landing-toc""#)
        .expect("the landing Contents nav is present");
    let end = html[start..].find("</nav>").expect("nav closes") + start;
    &html[start..end]
}

#[test]
fn the_landing_emits_an_inert_continue_slot_and_the_book_carries_a_stable_identity() {
    // book-resume. Which chapter a reader left off in is reader-local state that exists
    // only in their browser, so the *built* page must be identical for every reader: an
    // empty, hidden slot the client fills. Pinned on `corpus/tarn`, the multi-part book.
    let site = Site::discover(&corpus_dir().join("tarn"));
    let landing = site.render_page("index.tmd").expect("renders");
    let block = landing_toc_block(&landing);
    assert!(
        block.contains(CONTINUE_SLOT),
        "the landing's Continue slot must ship inert and empty: {block}"
    );
    // No reader state may leak into the artifact — no stored path, no chapter name, nothing
    // that would differ between two readers or two builds.
    assert!(
        !block.contains("Continue reading"),
        "the Continue label is written by the client, never baked into the build: {block}"
    );

    // The book's identity for that state is its landing href. NOT the title: a retitled
    // book must not orphan every reader's position, and an untitled book has no title to
    // key on at all.
    assert!(
        landing.contains(r#"data-tali-book="index.html""#),
        "the landing must carry the book identity: {landing}"
    );
    let chapter = site.render_page("install.tmd").expect("renders");
    assert!(
        chapter.contains(r#"data-tali-book="index.html""#),
        "every page of one book must agree on its identity: {chapter}"
    );
    // A chapter is a destination, not the way back in.
    assert!(
        !chapter.contains(CONTINUE_SLOT),
        "only the landing page offers Continue"
    );
    assert!(
        site.book.as_ref().unwrap().title.is_some(),
        "sanity: tarn IS titled, so a title-keyed identity would have looked fine here — \
         which is exactly why the identity is the href"
    );
}

#[test]
fn the_book_identity_is_depth_relative_so_a_nested_chapter_resolves_to_one_root() {
    // No book anywhere in the corpus keeps a chapter in a SUBDIRECTORY (enumerated, not
    // grepped: all five corpus books are flat), so `{up}` is empty everywhere and the pin
    // above passes with the prefix deleted. Both dogfood books are nested and are not in
    // the test net. Mint the missing shape rather than leave the guard vacuous.
    let proj = TempProj::new();
    proj.file(
        "_site.yml",
        "title: \"Nested\"\nproject:\n  type: book\nchapters:\n  - index.tmd\n  - guide/deep.tmd\n",
    )
    .file("index.tmd", "# Nested\n\nLanding.\n")
    .file(
        "guide/deep.tmd",
        "# Deep\n\nA chapter one directory down.\n",
    );
    let site = Site::discover(&proj.0);
    assert!(
        site.render_page("index.tmd")
            .expect("landing renders")
            .contains(r#"data-tali-book="index.html""#)
    );
    let deep = site.render_page("guide/deep.tmd").expect("chapter renders");
    assert!(
        deep.contains(r#"data-tali-book="../index.html""#),
        "a chapter one level down must point back up to the same landing: {deep}"
    );
}
