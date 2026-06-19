//! Full-text search index (`Site::search_index_json`): every page contributes a
//! page-level entry plus one entry per anchored heading, each carrying the
//! plain-text body of its section so Cmd-K matches prose, not just headings.
//! The index builder's text helpers are unit-tested in `site/search.rs`; this
//! covers the end-to-end shape over a discovered site.

use qmd_fast_core::Site;

mod common;
use common::TempProj;

#[test]
fn index_captures_page_title_heading_and_section_body_prose() {
    let d = TempProj::new();
    d.file(
        "_quarto.yml",
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
