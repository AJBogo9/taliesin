//! Cross-reference backlinks: a cross-referenced target shows a quiet "Referenced
//! by" line linking each page that references it (the reverse of forward xref).
//! Exercised through the real `demo-book`, where `results.tmd` references
//! `@sec-methods`, `@sec-setup`, and `@thm-kl` — all defined on `methods.tmd`. The
//! pure index/formatting helpers are unit-tested in `site/backlinks.rs`; this pins
//! the discover → render integration seam.

use taliesin_core::Site;

mod common;
use common::corpus_dir;

#[test]
fn methods_targets_show_referenced_by_results() {
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let methods = site.render_page("methods.tmd").expect("methods renders");

    // Each cross-referenced target on this page gets its own backref block.
    for anchor in ["sec-methods", "sec-setup", "thm-kl"] {
        assert!(
            methods.contains(&format!("data-block-id=\"qmd-backref-{anchor}\"")),
            "methods.html is missing the backref block for {anchor}"
        );
    }
    assert!(
        methods.contains("Referenced by"),
        "no 'Referenced by' label"
    );
    // The referrer link is page-level to results.html, labelled by its page title.
    assert!(
        methods.contains(r#"<a href="results.html" class="tali-backref">Results</a>"#),
        "backref should link to results.html labelled 'Results'"
    );
}

#[test]
fn same_page_reference_is_not_a_backlink() {
    // methods.tmd references @thm-kl itself (same page). A same-page reference must
    // not list the defining page as its own referrer — thm-kl is referred to only by
    // results.html, so exactly one referrer link, not two.
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let methods = site.render_page("methods.tmd").expect("methods renders");
    let block = methods
        .split(r#"data-block-id="qmd-backref-thm-kl""#)
        .nth(1)
        .expect("thm-kl backref block present")
        .split("</div>")
        .next()
        .unwrap();
    assert_eq!(
        block.matches("tali-backref").count(),
        1,
        "thm-kl should have exactly one (cross-page) referrer, not the same page too"
    );
}

#[test]
fn a_page_that_defines_no_referenced_targets_has_no_backrefs() {
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let results = site.render_page("results.tmd").expect("results renders");
    // Check for the backref BLOCK (its `qmd-backref-` block id), not the class name
    // `tali-backrefs` — the latter also appears in the inlined `.tali-backrefs` CSS
    // rule in every page's <head>, so it is not a reliable "no backref line" signal.
    assert!(
        !results.contains("qmd-backref-"),
        "results.tmd defines no referenced-to anchors, so it shows no backref block"
    );
}
