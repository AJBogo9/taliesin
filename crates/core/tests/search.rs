//! Full-text search index (`Site::search_index_json`): every page contributes a
//! page-level entry plus one entry per anchored heading, each carrying the
//! plain-text body of its section so Cmd-K matches prose, not just headings.
//! The index builder's text helpers are unit-tested in `site/search.rs`; this
//! covers the end-to-end shape over a discovered site.

use taliesin_core::Site;

mod common;
use common::TempProj;

#[test]
fn cross_page_search_wires_a_script_loadable_index_not_a_raw_fetch() {
    // The cross-page index must load via a `<script>`-loadable URL (a `.js` that
    // assigns window.TALIESIN_SEARCH_INDEX), which works under file:// too. A raw
    // `search.json` fetched with fetch() is CORS-blocked on file://, silently
    // killing Cmd-K when a book is opened from disk (the author's bug report).
    let d = TempProj::new();
    d.file(
        "_site.yml",
        "title: S\nnav:\n  - { text: Home, href: index.tmd }\n  - { text: Two, href: two.tmd }\n",
    );
    // `toc: true` forces the TOC (and the search palette rides with it), so the search
    // wiring is emitted regardless of heading count.
    d.file(
        "index.tmd",
        "---\ntitle: One\ntoc: true\n---\n\nAlpha prose about kangaroos.\n\n## Head A {#sec-a}\n\nBody a.\n",
    );
    d.file(
        "two.tmd",
        "---\ntitle: Two\n---\n\nBeta prose about wombats.\n\n## Head B {#sec-b}\n\nBody b.\n",
    );
    let site = Site::discover(&d.0);
    let html = site.render_page("index.tmd").expect("renders");
    assert!(
        html.contains("search-index.js"),
        "page must wire the script-loadable index (search-index.js)"
    );
    assert!(
        !html.contains("search.json"),
        "page must not reference the fetch-only search.json"
    );
}

/// The index must carry the SAME numbers as the page it links to. Every other site path
/// renders scoped to the page's chapter; `build_sections` rendered unscoped, so a book's
/// index said "Figure 1" while the page said "2.1" — a snippet contradicting its own
/// target.
///
/// The number is asserted as the reader SEES it ("Figure 2.1"), which is the only form
/// that makes the agreement meaningful — Cmd-K matches the indexed text, so a number the
/// index stores as `Theorem&nbsp;2.1` agrees with the page and is still unsearchable.
/// This test used to pin exactly that, `&nbsp;` and all, with a note conceding the reader
/// could not find it; the extraction now decodes the entity, so the pin asserts the fix.
#[test]
fn a_books_index_carries_the_chapter_scoped_numbers_its_pages_show() {
    use common::corpus_dir;
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let idx = &site.search_index_json;
    // methods.tmd is chapter 2, so its first figure is 2.1 — the number `corpus.rs`
    // asserts on the rendered page, which shows it as `Figure&nbsp;2.1`.
    assert!(
        idx.contains("Figure 2.1"),
        "the index should carry the chapter-scoped figure number, as a reader types it: {}",
        &idx[..idx.len().min(400)]
    );
    assert!(
        !idx.contains("Figure 1:"),
        "no flat number should survive in a book's index: {idx}"
    );
    assert!(
        !idx.contains("&nbsp;"),
        "the index is searched as text: a raw entity means the number a reader can SEE \
         matches nothing: {idx}"
    );
}

#[test]
fn index_captures_page_title_heading_and_section_body_prose() {
    let d = TempProj::new();
    d.file(
        "_site.yml",
        "title: S\nnav:\n  - { text: Home, href: index.tmd }\n",
    );
    d.file(
        "index.tmd",
        "---\ntitle: Welcome\n---\n\nIntro paragraph about kangaroos.\n\n\
         ## Photosynthesis {#sec-photo}\n\nLeaves convert sunlight into glucose.\n",
    );
    let site = Site::discover(&d.0);
    let idx = &site.search_index_json;

    // It's a JSON array (the client does JSON.parse on it).
    assert!(
        idx.starts_with('[') && idx.ends_with(']'),
        "not a JSON array: {idx}"
    );
    // A page-level entry carries the page title; its body is the intro prose.
    assert!(
        idx.contains("\"t\":\"Welcome\""),
        "page title not indexed: {idx}"
    );
    assert!(
        idx.contains("kangaroos"),
        "intro body prose not indexed: {idx}"
    );
    // A heading entry carries the anchor id, the heading text, and — the point of
    // full-text search — the prose of the section beneath it.
    assert!(
        idx.contains("\"i\":\"sec-photo\""),
        "heading anchor not indexed: {idx}"
    );
    assert!(
        idx.contains("\"t\":\"Photosynthesis\""),
        "heading text not indexed: {idx}"
    );
    assert!(
        idx.contains("Leaves convert sunlight into glucose"),
        "section body prose not indexed (headings-only regression?): {idx}"
    );
}
