//! A document that belongs to no project gets no project chrome. `preview <file>` used to
//! wrap a lone .tmd in a site header carrying a brand link to itself labelled "Home", a
//! burger over an empty nav, a search button and a site footer, none of which
//! `build <file>` has ever emitted.

use std::path::Path;
use taliesin_core::Site; // re-exported at the crate root (`pub use site::{DraftMode, Page, Site}`)

fn corpus(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus")
        .join(rel)
}

#[test]
fn a_lone_document_is_marked_standalone() {
    let site = Site::discover_single(&corpus("agent/executed-read.tmd"));
    assert!(
        site.standalone,
        "corpus/agent has no _site.yml, so this document belongs to no project"
    );
}

#[test]
fn a_document_inside_a_project_is_not_standalone() {
    let site = Site::discover_single(&corpus("shared-bib/index.tmd"));
    assert!(
        !site.standalone,
        "corpus/shared-bib HAS an _site.yml, so its pages keep project chrome"
    );
}

#[test]
fn a_standalone_document_renders_no_site_header_or_footer() {
    let site = Site::discover_single(&corpus("agent/executed-read.tmd"));
    let page = site.pages.first().expect("the one scoped page");
    let ctx = site.page_chrome(page);
    assert_eq!(ctx.navbar_html, "", "no site navbar: {:?}", ctx.navbar_html);
    assert_eq!(ctx.footer_html, "", "no site footer: {:?}", ctx.footer_html);
}

#[test]
fn a_project_page_still_renders_its_header() {
    let site = Site::discover(&corpus("shared-bib"));
    let page = site.pages.first().expect("a page");
    let ctx = site.page_chrome(page);
    assert!(
        ctx.navbar_html.contains("tali-site-nav"),
        "a real project keeps its navbar: {:?}",
        ctx.navbar_html
    );
}
