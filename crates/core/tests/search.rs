//! Full-text search index (`Site::search_index_json`): every page contributes a
//! page-level entry plus one entry per anchored heading, each carrying the
//! plain-text body of its section so Cmd-K matches prose, not just headings.
//! The index builder's text helpers are unit-tested in `site/search.rs`; this
//! covers the end-to-end shape over a discovered site.

use qmd_fast_core::Site;

mod common;
use common::TempProj;

#[test]
fn cross_page_search_wires_a_script_loadable_index_not_a_raw_fetch() {
    // The cross-page index must load via a `<script>`-loadable URL (a `.js` that
    // assigns window.QMD_SEARCH_INDEX), which works under file:// too. A raw
    // `search.json` fetched with fetch() is CORS-blocked on file://, silently
    // killing Cmd-K when a book is opened from disk (the author's bug report).
    let d = TempProj::new();
    d.file(
        "_site.yml",
        "title: S\nnav:\n  - { text: Home, href: index.qmd }\n  - { text: Two, href: two.qmd }\n",
    );
    // `toc: true` forces the TOC (and the search palette rides with it), so the search
    // wiring is emitted regardless of heading count.
    d.file(
        "index.qmd",
        "---\ntitle: One\ntoc: true\n---\n\nAlpha prose about kangaroos.\n\n## Head A {#sec-a}\n\nBody a.\n",
    );
    d.file(
        "two.qmd",
        "---\ntitle: Two\n---\n\nBeta prose about wombats.\n\n## Head B {#sec-b}\n\nBody b.\n",
    );
    let site = Site::discover(&d.0);
    let html = site.render_page("index.qmd").expect("renders");
    assert!(
        html.contains("search-index.js"),
        "page must wire the script-loadable index (search-index.js)"
    );
    assert!(
        !html.contains("search.json"),
        "page must not reference the fetch-only search.json"
    );
}

#[test]
fn index_captures_page_title_heading_and_section_body_prose() {
    let d = TempProj::new();
    d.file(
        "_site.yml",
        "title: S\nnav:\n  - { text: Home, href: index.qmd }\n",
    );
    d.file(
        "index.qmd",
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
