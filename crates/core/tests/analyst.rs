//! Interaction pin for the "computational-report analyst" demand-probe persona
//! (corpus/analyst/, a two-page latency readout). Locks the combination no other
//! corpus document has: **two languages executing in one document**, with the
//! numbered floats they produce interleaved against an authored Markdown table and
//! against each other, plus cross-PAGE references to a cell-produced float.
//!
//! These are the *render-time* halves — the numbering registry and the cross-page
//! rewrite — which is exactly what the core crate can see: rendering never executes a
//! cell, so no kernel is needed here and none is gated on. The executed halves (the
//! dual-theme matplotlib pair, the anchor a labelled table cell keeps even when its
//! output is not a table) live with the executor, in `crates/server/src/exec.rs`.
//!
//! See notes/2026-07-26-corpus-demand-probe-analyst.md for the findings this produced.

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn analyst() -> Site {
    Site::discover(&corpus_dir().join("analyst"))
}

fn readout() -> String {
    analyst().render_page("index.tmd").expect("readout renders")
}

/// One table counter spans the authored `: caption {#tbl-x}` path and the executed
/// `#| label: tbl-x` path, numbering them in document order. The two paths live in
/// different functions (`apply_table_captions`'s Markdown arm vs its cell arm) and no
/// other corpus doc puts both in one page, so nothing pinned that they share a counter.
#[test]
fn authored_and_executed_tables_share_one_counter_in_document_order() {
    let h = readout();
    for (anchor, number) in [
        ("tbl-slo", "1"),      // an authored Markdown table + `: caption {#tbl-slo}`
        ("tbl-coverage", "2"), // a {python} cell (`#| label: tbl-coverage`)
        ("tbl-coefs", "3"),    // an {r} cell (`#| label: tbl-coefs`)
    ] {
        let link = format!("href=\"#{anchor}\" class=\"tali-xref\">Table&nbsp;{number}<");
        assert!(
            h.contains(&link),
            "@{anchor} must resolve to Table {number} (document order across the \
             authored and executed paths); looked for {link}: {h}"
        );
    }
}

/// Figures from separate cells are numbered in document order across the whole page: the
/// weekly-p95 figure is Figure 1 and the split-halves figure that follows it is Figure 2.
/// A per-CELL counter would number both of them 1.
///
/// This pinned "across two LANGUAGES" until `{r}` was withdrawn on 2026-08-08 and the
/// readout's model cells were rewritten in Python. The counter is language-blind, so what
/// it really asserts is unchanged; only the sentence describing it was ever about R.
#[test]
fn figures_from_separate_cells_share_one_counter() {
    let h = readout();
    assert!(
        h.contains("href=\"#fig-p95\" class=\"tali-xref\">Figure&nbsp;1<"),
        "the first cell's figure is Figure 1: {h}"
    );
    assert!(
        h.contains("href=\"#fig-effects\" class=\"tali-xref\">Figure&nbsp;2<"),
        "the figure that follows it is Figure 2, not a second Figure 1: {h}"
    );
}

/// A cross-PAGE reference to a *cell-produced* float resolves to the right page AND the
/// right number. The number cannot come from the source scan (a `#| label:` lives inside
/// a code cell, which the scan skips); it is harvested from the defining page's render by
/// `Site::harvest_xref_numbers`. That harvest is what this asserts, on the only corpus
/// project where the target float is produced by a cell rather than authored.
#[test]
fn a_cross_page_ref_to_a_cell_produced_float_keeps_its_number() {
    let methods = analyst()
        .render_page("methods.tmd")
        .expect("methods renders");
    assert!(
        methods.contains("href=\"index.html#tbl-coefs\" class=\"tali-xref\">Table&nbsp;3<"),
        "a cross-page @tbl- to an {{r}} cell's table must carry page AND number: {methods}"
    );
    assert!(
        methods.contains("href=\"index.html#fig-p95\" class=\"tali-xref\">Figure&nbsp;1<"),
        "a cross-page @fig- to a {{python}} cell's figure must carry page AND number: \
         {methods}"
    );
}

// A cross-reference written inside an executed cell's `#| fig-cap:` (the readout has one:
// `@tbl-slo` in the `fig-p95` caption) is NOT asserted here. This file renders without a
// kernel, so that caption does not exist in `readout()` at all — asserting on it would
// pass vacuously either way. The two halves are pinned where each can actually run:
// `exec::tests::executed_caption_emits_cross_reference_markers` (the server emits the
// marker) and `site::xref::tests::a_same_page_marker_resolves_to_a_bare_fragment_with_its_number`
// (the site pass resolves it).

/// A cross-PAGE `@sec-` on a **website** has no number to carry — section numbering is
/// a book's, and `harvest_xref_numbers` deliberately refuses to invent one here (a flat
/// per-page counter would be mislabelled "Chapter 1"). It must still say which section
/// it points at: the readout is a two-page website, so this is the corpus's only
/// unnumbered cross-page section reference, and without the heading title the sentence
/// renders as "…by least squares in Section." (AN-5).
#[test]
fn a_cross_page_sec_ref_on_a_website_names_the_heading_it_points_at() {
    let methods = analyst()
        .render_page("methods.tmd")
        .expect("methods renders");
    assert!(
        methods.contains(
            "<a href=\"index.html#sec-model\" class=\"tali-xref\">Section&nbsp;\u{201c}Is the \
             canary still slower?\u{201d}</a>"
        ),
        "a cross-page @sec- must name its target heading, not render a bare \
         \u{201c}Section\u{201d}: {methods}"
    );
}

/// The authored table is the one float whose anchor + caption exist without execution,
/// so it pins the Markdown-caption path end to end (id on the `<table>`, `<caption>` as
/// its first child) rather than only its xref registration.
#[test]
fn the_authored_table_carries_its_id_and_folded_caption() {
    let h = readout();
    // The id leads the tag; the block-model attributes (`data-block-id`,
    // `data-sourcepos`) sit between it and the folded caption, so needle the two ends
    // rather than one literal run.
    assert!(
        h.contains("<table id=\"tbl-slo\" data-block-id="),
        "the `{{#tbl-slo}}` label lands on the <table> itself: {h}"
    );
    assert!(
        h.contains(
            "<caption><span class=\"tali-caption-label\">Table&nbsp;1</span>: The two \
             service objectives this readout is written against.</caption>"
        ),
        "the `: caption` paragraph folds into the table as a numbered <caption>: {h}"
    );
}

/// The Methods page shows the generating script in a **non-executed** ```` ```python ````
/// block, and that is load-bearing rather than stylistic: as an executable `{python}`
/// cell it would rewrite the project's own committed input on every build, which is
/// exactly what the page's prose says it must not do ("a report that regenerates its own
/// input on every build would quietly hide a change to the input"). The seed is pinned
/// with it, because the script and `data/latency.csv` only agree while it is unchanged.
#[test]
fn the_generating_script_is_shown_but_never_executed() {
    let methods = std::fs::read_to_string(corpus_dir().join("analyst/methods.tmd"))
        .expect("methods source readable");
    // Scoped to the section that carries the script, not to the whole page. The page also
    // holds the diagnostics cells, which DO execute and are meant to — asserting over the
    // file would have made this pass only while `methods.tmd` had no live cells at all,
    // which stopped being true when the R diagnostics were rewritten in Python.
    let section = methods
        .split_once("## How the file was generated")
        .expect("methods.tmd has the generating-script section")
        .1;
    let section = section
        .split_once("\n## ")
        .map_or(section, |(before, _)| before);
    assert!(
        section.contains("```python\n"),
        "the generating script is shown as a plain fenced block: {section}"
    );
    assert!(
        !section.contains("```{python}"),
        "the generating script must NOT become an executable cell — it would rewrite \
         data/latency.csv on every build, and both pages' numbers are quoted in prose"
    );
    assert!(
        methods.contains("default_rng(20260726)"),
        "the shown seed is what makes the script and the committed CSV agree"
    );
}

/// Both pages read one committed CSV, which is what makes the report reproducible and
/// runnable by a reader who clones the repo. If the data file goes missing every code
/// cell in the project fails at run time, and nothing else in the test net would say so
/// (rendering never executes).
#[test]
fn the_committed_dataset_both_languages_read_is_present_and_complete() {
    let csv = std::fs::read_to_string(corpus_dir().join("analyst/data/latency.csv"))
        .expect("the committed dataset both languages read must exist");
    let mut lines = csv.lines();
    assert_eq!(
        lines.next(),
        Some("week,region,channel,requests_k,p50_ms,p95_ms,errors"),
        "the column set is the data dictionary in methods.tmd; changing it silently \
         invalidates that page's table"
    );
    // 13 ISO weeks x 3 regions x 2 channels, no gaps.
    assert_eq!(
        lines.count(),
        78,
        "the readout's prose states 78 region-week-channel rows with no gaps"
    );
}
