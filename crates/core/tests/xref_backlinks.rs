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
            methods.contains(&format!("data-block-id=\"tali-backref-{anchor}\"")),
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
        .split(r#"data-block-id="tali-backref-thm-kl""#)
        .nth(1)
        .expect("thm-kl backref block present")
        .split("</div>")
        .next()
        .unwrap();
    // `class="tali-backref"` exactly, not a bare `tali-backref` substring: the citing
    // sentence beside each referrer is a `tali-backref-cite` span, so the loose needle
    // counts two per referrer and this assertion would pass for the wrong reason.
    assert_eq!(
        block.matches(r#"class="tali-backref""#).count(),
        1,
        "thm-kl should have exactly one (cross-page) referrer, not the same page too"
    );
}

#[test]
fn a_page_that_defines_no_referenced_targets_has_no_backrefs() {
    let site = Site::discover(&corpus_dir().join("demo-book"));
    // `summary.tmd`, not `results.tmd`: results now DEFINES `fig-stages` (a cell-labelled
    // figure that summary refers to), so it legitimately carries a backref block. Summary
    // only *refers* to targets, defining none — which is the property this pins, and the
    // property results.tmd used to have. Referring is not defining: a referrer gets no
    // backref block of its own.
    let summary = site.render_page("summary.tmd").expect("summary renders");
    // Check for the backref BLOCK by its full `data-block-id="tali-backref-` opener, not
    // by any class-name substring — `.tali-backrefs` AND `.tali-backref-cite` are both
    // inlined in every page's <head>, so `tali-backrefs` and even `tali-backref-` match
    // the stylesheet on a page with no backlink line at all.
    assert!(
        !summary.contains(r#"data-block-id="tali-backref-"#),
        "summary.tmd defines no referenced-to anchors, so it shows no backref block"
    );
}

#[test]
fn a_backlink_quotes_the_sentence_the_reference_is_made_in() {
    // The default-output half of `link-text-self-describing`: a bare page title is a weak
    // proximal cue, the citing sentence is the strongest one available. Pinned end to end
    // because the sentence is harvested during discovery, from the *resolved* block —
    // "Theorem 2.1", the text results.html actually shows, not cite's bare "Theorem".
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let methods = site.render_page("methods.tmd").expect("methods renders");
    let line = methods
        .split(r#"data-block-id="tali-backref-thm-kl""#)
        .nth(1)
        .expect("thm-kl backref block present")
        .split("</div>")
        .next()
        .unwrap();
    assert!(
        line.contains(
            r#"<span class="tali-backref-cite">“It also leans on Theorem 2.1 from the methods chapter.”</span>"#
        ),
        "thm-kl's backlink should quote results.tmd's citing sentence with the resolved \
         number, got: {line}"
    );
    // Adjacent to the link, never inside it: the sentence carries the reference's own
    // `<a>` on the referring page, and an anchor inside an anchor is invalid HTML.
    assert!(line.contains(r#"</a> <span class="tali-backref-cite">"#));
}

#[test]
fn a_backlink_quotes_only_the_sentence_containing_the_reference() {
    // results.tmd's first paragraph is two sentences; `@thm-kl` is in the second. A
    // whole-paragraph excerpt would pass every "contains the sentence" assertion above,
    // so pin what must be ABSENT: the neighbouring sentence.
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let methods = site.render_page("methods.tmd").expect("methods renders");
    let line = methods
        .split(r#"data-block-id="tali-backref-thm-kl""#)
        .nth(1)
        .expect("thm-kl backref block present")
        .split("</div>")
        .next()
        .unwrap();
    assert!(
        !line.contains("What we found"),
        "the preceding sentence of the same paragraph must not be quoted: {line}"
    );
}

#[test]
fn a_cell_labelled_figure_is_backlinked_like_a_brace_id_one() {
    // The reverse index keys off `xref_targets`, so a cell-labelled float only earns a
    // "Referenced by" line once the render-harvest inserts it. Pins that the two anchor
    // forms are equivalent all the way through to backlinks, not just forward refs.
    let site = Site::discover(&corpus_dir().join("demo-book"));
    let results = site.render_page("results.tmd").expect("results renders");
    assert!(
        results.contains(r#"data-block-id="tali-backref-fig-stages""#),
        "the cell-labelled fig-stages should show a backref block for summary.tmd"
    );
    // Labelled "Wrap-up", not "Summary": `_site.yml` gives summary.tmd a `text:` override,
    // and the backref honours the book's chapter label.
    assert!(
        results.contains(r#"<a href="summary.html" class="tali-backref">Wrap-up</a>"#),
        "fig-stages' backref should link to summary.html labelled by its chapter override"
    );
}
