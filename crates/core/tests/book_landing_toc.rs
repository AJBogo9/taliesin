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
