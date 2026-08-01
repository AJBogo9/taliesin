//! The Google Scholar / Highwire-Press `citation_*` block, pinned against the corpus
//! paper (`cite-this/paper.tmd`) rather than a synthetic fixture.
//!
//! Unit tests in `site/meta.rs` cover each tag's shape; this is the end-to-end pin that a
//! real corpus document — structured authors with shared and multiple affiliations, a
//! `date:`, a `doi:` and a `bibliography:` — actually reaches the emitted head. Scholar's
//! indexing is entirely invisible from the page, so nothing about a regression here is
//! noticeable by eye or by rendering the document.

use taliesin_core::Site;

mod common;
use common::corpus_dir;

/// Every `<meta name="citation_…">` tag in `html`, in source order, as `(key, content)`.
fn citation_tags(html: &str) -> Vec<(String, String)> {
    let head = &html[..html.find("</head>").expect("has </head>")];
    let mut out = Vec::new();
    for (i, _) in head.match_indices("<meta name=\"citation_") {
        let rest = &head[i + "<meta name=\"".len()..];
        let key = &rest[..rest.find('"').expect("key closes")];
        let after = &rest[rest.find("content=\"").expect("has content") + "content=\"".len()..];
        let val = &after[..after.find('"').expect("content closes")];
        out.push((key.to_string(), val.to_string()));
    }
    out
}

#[test]
fn the_corpus_paper_emits_the_full_scholar_block_in_scholar_order() {
    let site = Site::discover(&corpus_dir().join("cite-this"));
    let html = site.render_page("paper.tmd").expect("renders");
    let tags = citation_tags(&html);
    let keys: Vec<&str> = tags.iter().map(|(k, _)| k.as_str()).collect();

    // The whole block, in order. An institution binds to the author ABOVE it, so this is
    // an ordered assertion on purpose: a set-membership check would pass while every
    // affiliation was silently reattributed to the last author.
    assert_eq!(
        tags,
        vec![
            ("citation_title".into(), "On the Analytical Engine".into()),
            ("citation_author".into(), "Grace Hopper".into()),
            (
                "citation_author_institution".into(),
                "Harvard Computation Laboratory".into()
            ),
            ("citation_author".into(), "Charles Babbage".into()),
            (
                "citation_author_institution".into(),
                "Harvard Computation Laboratory".into()
            ),
            (
                "citation_author_institution".into(),
                "Analytical Society".into()
            ),
            ("citation_publication_date".into(), "2026-03-09".into()),
            (
                "citation_journal_title".into(),
                "Journal of Examples".into()
            ),
            ("citation_doi".into(), "10.5281/zenodo.1825009".into()),
            (
                "citation_public_url".into(),
                "https://example.org/paper.html".into()
            ),
            (
                "citation_abstract_html_url".into(),
                "https://example.org/paper.html".into()
            ),
        ],
        "the emitted scholar block drifted; keys were {keys:?}"
    );
}

#[test]
fn the_doi_reaches_the_page_normalised_not_as_the_author_wrote_it() {
    // The corpus writes the `https://doi.org/…` spelling, which is what a publisher page
    // hands you. Scholar wants the bare identifier, so the URL form must not survive into
    // the tag — and asserting the absence of the URL is what catches a pass-through.
    let site = Site::discover(&corpus_dir().join("cite-this"));
    let html = site.render_page("paper.tmd").expect("renders");
    assert!(
        html.contains(r#"<meta name="citation_doi" content="10.5281/zenodo.1825009">"#),
        "the bare DOI must reach the head: {html}"
    );
    assert!(
        !html.contains(r#"content="https://doi.org/10.5281/zenodo.1825009""#),
        "the doi.org URL form must not be emitted as the citation_doi content"
    );
}

/// The `citation_*` block reads `page.authors` **with no site-level fallback**, unlike the
/// "Cite this" box (`cite_this::resolve`) and the JSON-LD `author` on the very same page,
/// which both fall back to `_site.yml`'s `author:`. So `note.tmd` — dated, and authored as
/// far as every other consumer is concerned — emits no scholar block at all.
///
/// This pins the asymmetry as *measured*, not as *intended*: it is filed as its own backlog
/// item, and it is deliberately outside item 186 (which completes the block's tag list, not
/// its gate). The value of pinning it is that whoever changes the gate is told, by a failing
/// test naming the other two chains, that three metadata blocks on one page disagree about
/// whether it has an author.
#[test]
fn the_scholar_block_does_not_follow_the_site_author_fallback() {
    let site = Site::discover(&corpus_dir().join("cite-this"));
    let html = site.render_page("note.tmd").expect("renders");
    assert!(
        html.contains("data-block-id=\"tali-cite-this\""),
        "note.tmd's Cite-this box DOES resolve an author via the site fallback"
    );
    assert!(
        html.contains(r#""@type":"Person","name":"Ada Lovelace""#),
        "...and so does its JSON-LD author: {html}"
    );
    assert!(
        !html.contains("citation_"),
        "...while the scholar block emits nothing. If this now fails, the gate was fixed \
         (good) — update this test to assert the block instead of its absence."
    );
}
