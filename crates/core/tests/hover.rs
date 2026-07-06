//! Cross-page hover-preview snippet index (`Site::hover_index_json`): the
//! render → harvest → URL-rebase pipeline over a discovered site. The string
//! helpers are unit-tested in `site/hover.rs`; this pins the integration seam
//! that the corpus book (demo-book, which has no image figures) can't reach —
//! a real rendered figure block whose relative image is rebased root-relative,
//! and a nested viewing page whose hover pointer carries the right depth prefix.

use taliesin_core::Site;

mod common;
use common::TempProj;

#[test]
fn hover_index_rebases_a_nested_page_figure_image_to_root_relative() {
    let d = TempProj::new();
    d.file("_site.yml", "title: Gallery\n");
    // The figure is DEFINED on a NESTED page (charts/), with an image path relative to
    // that page's own directory — so the rebaser must prepend `charts/` to make it
    // root-relative (the interesting join_rel branch a flat corpus never hits).
    d.file(
        "charts/index.tmd",
        "---\ntitle: Charts\n---\n\n![A chart](chart.png){#fig-chart}\n",
    );
    d.file("charts/chart.png", "not-a-real-png");
    // A different, also-nested page references it cross-page.
    d.file(
        "deep/reader.tmd",
        "---\ntitle: Reader\n---\n\nSee @fig-chart on the charts page.\n",
    );
    let site = Site::discover(&d.0);

    // The served index carries the figure snippet with its image rebased root-relative
    // (charts/chart.png), NOT the raw `chart.png` that would 404 from another directory.
    let idx = &site.hover_index_json;
    assert!(
        idx.contains("\"fig-chart\""),
        "index missing fig-chart: {idx}"
    );
    assert!(
        idx.contains("src=\\\"charts/chart.png\\\""),
        "figure image not rebased root-relative: {idx}"
    );
    assert!(
        !idx.contains("src=\\\"chart.png\\\""),
        "raw (un-rebased) image path leaked into the index: {idx}"
    );

    // A nested viewing page points at the lazy index with the right up-prefix, so the
    // client can resolve the rebased root-relative URL from depth (SITE_ROOT="../").
    let reader = site.render_page("deep/reader.tmd").expect("reader renders");
    assert!(
        reader.contains("window.TALIESIN_SITE_ROOT=\"../\""),
        "nested page needs a depth-1 site root: {reader}"
    );
    assert!(
        reader.contains("window.TALIESIN_HOVER_URL=\"../hover-index.js\""),
        "nested page needs a depth-relative hover-index pointer"
    );
}
