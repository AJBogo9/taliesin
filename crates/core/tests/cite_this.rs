//! B1: the reader-facing "Cite this" box (BibTeX / CSL-JSON / RIS).
//!
//! Renders the corpus `cite-this/` mini-site and pins the render gate end to end:
//! a page with its own `author:` gets a box with that byline (scholarly typing when it
//! declares a `bibliography:`); an authorless dated page falls back to the site author;
//! a dateless page renders no box; and a site with NO author anywhere degrades an
//! authorless dated page to nothing (never an empty shell).

use taliesin_core::Site;

mod common;
use common::{TempProj, corpus_dir};

#[test]
fn a_page_with_its_own_author_renders_a_scholarly_cite_box() {
    let site = Site::discover(&corpus_dir().join("cite-this"));
    let html = site.render_page("paper.tmd").expect("renders");
    assert!(
        html.contains("class=\"tali-cite-this\""),
        "cite box missing on an authored, dated page: {html}"
    );
    assert!(html.contains("data-format=\"bibtex\""));
    assert!(html.contains("data-format=\"csl\""));
    assert!(html.contains("data-format=\"ris\""));
    // The page's own author is the byline; the bibliography makes it scholarly.
    assert!(
        html.contains("AU  - Hopper, Grace"),
        "the page's own author must be the byline"
    );
    assert!(
        html.contains("TY  - JOUR"),
        "a page with a bibliography is a scholarly article"
    );
}

#[test]
fn an_authorless_dated_page_falls_back_to_the_site_author() {
    let site = Site::discover(&corpus_dir().join("cite-this"));
    let html = site.render_page("note.tmd").expect("renders");
    assert!(
        html.contains("class=\"tali-cite-this\""),
        "cite box missing on the site-author-fallback page"
    );
    assert!(
        html.contains("AU  - Lovelace, Ada"),
        "the site-level author must be the fallback byline"
    );
    assert!(
        html.contains("TY  - BLOG"),
        "no bibliography -> a blog post, not scholarly"
    );
}

#[test]
fn a_dateless_page_renders_no_cite_box() {
    let site = Site::discover(&corpus_dir().join("cite-this"));
    let html = site.render_page("index.tmd").expect("renders");
    // Key on the block id: the `.tali-cite-this` CLASS string also appears in the
    // always-inlined enhancer JS, so only the generated block's id proves absence.
    assert!(
        !html.contains("qmd-cite-this"),
        "a page without a date must render no citation box"
    );
}

#[test]
fn a_site_without_any_author_degrades_to_no_box() {
    let d = TempProj::new();
    d.file(
        "_site.yml",
        "title: \"No Author Here\"\nurl: https://ex.org\nnav:\n  - { text: Home, href: index.tmd }\n",
    );
    d.file("index.tmd", "---\ntitle: Home\n---\n\n# Hi\n");
    d.file(
        "post.tmd",
        "---\ntitle: Dated but authorless\ndate: 2026-02-02\n---\n\nBody.\n",
    );
    let site = Site::discover(&d.0);
    let html = site.render_page("post.tmd").expect("renders");
    assert!(
        !html.contains("qmd-cite-this"),
        "no author anywhere -> the box degrades to nothing"
    );
}
