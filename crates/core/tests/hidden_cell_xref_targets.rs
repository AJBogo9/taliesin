//! A labelled python/R cell with `#| include: false` must not advertise a
//! cross-reference target that can never exist.
//!
//! A python/R figure or table IS the executor's output block, and `include: false`
//! drops that block outright (`exec.rs`: `!cell.include -> continue`). The render
//! pass, though, registered the `#fig-`/`#tbl-` anchor and consumed a figure/table
//! number from the cell's *declared* label, before knowing whether anything would
//! ever carry it. Two reader-visible consequences:
//!
//! 1. `@fig-x` rendered a confident `<a href="#fig-x">Figure 1</a>` to an id emitted
//!    nowhere on the page — and because the anchor WAS in the registry, the
//!    "broken cross-reference" warning that would have caught it never fired.
//! 2. The burned number shifted every later figure down by one, so a document whose
//!    only visible figure is captioned "Figure 2" — with no Figure 1 anywhere — is
//!    produced with no cross-reference in the document at all.
//!
//! `CellRole::Listing` never had this bug: it gates on `include` *before* counting
//! and registering, because it emits its own artifact rather than deferring to the
//! executor. These tests hold the figure and table arms to the listing's rule:
//! register from what will exist, not from what was declared.
//!
//! mermaid/`{js}` figures are emitted at RENDER time, so their anchor is real
//! whatever `include` says; they must keep registering (pinned below, so the fix
//! cannot over-reach into them).

use std::path::Path;
use taliesin_core::render_document_with_includes;

fn doc(src: &str) -> taliesin_core::RenderedDoc {
    render_document_with_includes(src, Path::new("."))
}

fn warnings(src: &str) -> Vec<String> {
    doc(src).warnings.into_iter().map(|w| w.message).collect()
}

/// A python figure cell that is hidden, then one that is shown, then a reference to
/// each. The hidden cell's output never reaches the page.
fn hidden_then_shown_figures() -> &'static str {
    "---\ntitle: T\n---\n\n\
     ```{python}\n#| label: fig-hidden\n#| fig-cap: Never materializes\n#| include: false\n\
     plot(1)\n```\n\n\
     ```{python}\n#| label: fig-shown\n#| fig-cap: Does materialize\n\
     plot(2)\n```\n\n\
     See @fig-hidden and @fig-shown.\n"
}

fn hidden_then_shown_tables() -> &'static str {
    "---\ntitle: T\n---\n\n\
     ```{python}\n#| label: tbl-hidden\n#| tbl-cap: Never materializes\n#| include: false\n\
     frame(1)\n```\n\n\
     ```{python}\n#| label: tbl-shown\n#| tbl-cap: Does materialize\n\
     frame(2)\n```\n\n\
     See @tbl-hidden and @tbl-shown.\n"
}

/// The rendered text of the `<a>` for `anchor`, e.g. "Figure&nbsp;1" — or "Figure" when
/// the ref stayed unresolved. Asserting on this rather than on a whole-tag string keeps
/// the test from passing on attribute order alone.
fn xref_link_text(body: &str, anchor: &str) -> Option<String> {
    let at = body.find(&format!(r##"href="#{anchor}""##))?;
    let open_end = at + body[at..].find('>')?;
    let close = open_end + body[open_end..].find("</a>")?;
    Some(body[open_end + 1..close].to_string())
}

#[test]
fn an_include_false_figure_cell_registers_no_phantom_anchor() {
    // The bug's signature was a CONFIDENT ref: `@fig-hidden` resolved to "Figure 1"
    // from the registry while no element carried `id="fig-hidden"`. Unregistered, the
    // ref must instead stay unresolved — no number, and `cite`'s `data-qmd-xref` marker
    // still on it (the site layer consumes that for genuine cross-page refs, and
    // `validate_xrefs` reports it when nothing does).
    let body = doc(hidden_then_shown_figures()).body_html();
    assert_eq!(
        xref_link_text(&body, "fig-hidden").as_deref(),
        Some("Figure"),
        "`@fig-hidden` must not resolve to a number: `include: false` means no element \
         ever carries `id=\"fig-hidden\"`; got:\n{body}"
    );
    assert!(
        body.contains(r##"data-qmd-xref="fig-hidden""##),
        "an unresolvable ref must keep its unresolved marker, or nothing reports it:\n{body}"
    );
}

#[test]
fn an_unresolvable_include_false_reference_is_reported_as_broken() {
    // Registering the phantom silenced this warning — the one diagnostic that flagged
    // the dead link. Declining to register must bring it back, or the fix trades a
    // wrong answer for no answer.
    //
    // `validate_xrefs` is not part of a render's own warnings; the servers call it
    // themselves (`build.rs`, `check.rs`, `serve/mod.rs`), so the test calls it the
    // same way — as `located_warnings.rs` and `xref_didyoumean.rs` do.
    let d = doc(hidden_then_shown_figures());
    let ws: Vec<String> = taliesin_core::cite::validate_xrefs(&d.blocks)
        .into_iter()
        .map(|w| w.message)
        .collect();
    assert!(
        ws.iter()
            .any(|m| m.contains("broken cross-reference") && m.contains("@fig-hidden")),
        "a reference to a never-materializing figure must be reported broken: {ws:?}"
    );
    assert!(
        !ws.iter().any(|m| m.contains("@fig-shown")),
        "the figure that DOES materialize must not be reported broken: {ws:?}"
    );
}

#[test]
fn an_include_false_figure_cell_burns_no_figure_number() {
    // The reader-visible half that needs no cross-reference to bite: the hidden cell
    // used to consume Figure 1, leaving the page's only visible figure as "Figure 2".
    let body = doc(hidden_then_shown_figures()).body_html();
    assert_eq!(
        xref_link_text(&body, "fig-shown").as_deref(),
        Some("Figure&nbsp;1"),
        "the first figure that actually renders must be Figure 1, not Figure 2 — a \
         hidden figure must not consume a number; got:\n{body}"
    );
}

#[test]
fn an_include_false_figure_cell_warns_that_its_label_is_unreferenceable() {
    // Mirrors the theorem-prefix warning: a label that can never be reached is an
    // author mistake, and saying so at the DEFINITION is what makes the broken
    // reference diagnosable ("no such figure" about a label they plainly wrote).
    let ws = warnings(hidden_then_shown_figures());
    assert!(
        ws.iter()
            .any(|m| m.contains("fig-hidden") && m.contains("cross-referenc")),
        "a labelled `include: false` figure cell must warn it is unreferenceable: {ws:?}"
    );
}

#[test]
fn an_include_false_table_cell_registers_no_phantom_anchor() {
    let body = doc(hidden_then_shown_tables()).body_html();
    assert_eq!(
        xref_link_text(&body, "tbl-hidden").as_deref(),
        Some("Table"),
        "`@tbl-hidden` must not resolve to a number: the executor drops the output that \
         would carry `id=\"tbl-hidden\"`; got:\n{body}"
    );
    assert!(
        body.contains(r##"data-qmd-xref="tbl-hidden""##),
        "an unresolvable ref must keep its unresolved marker:\n{body}"
    );
}

#[test]
fn an_include_false_table_cell_burns_no_table_number() {
    let body = doc(hidden_then_shown_tables()).body_html();
    assert_eq!(
        xref_link_text(&body, "tbl-shown").as_deref(),
        Some("Table&nbsp;1"),
        "the first table that actually renders must be Table 1, not Table 2; got:\n{body}"
    );
}

#[test]
fn an_include_false_table_cell_warns_that_its_label_is_unreferenceable() {
    let ws = warnings(hidden_then_shown_tables());
    assert!(
        ws.iter()
            .any(|m| m.contains("tbl-hidden") && m.contains("cross-referenc")),
        "a labelled `include: false` table cell must warn it is unreferenceable: {ws:?}"
    );
}

#[test]
fn the_unreferenceable_warning_is_located_at_the_cell() {
    // An unlocated warning is the exact Quarto flaw the project critiques, and the
    // render loop has the cell's line in hand, so there is no excuse for dropping it.
    let ws: Vec<_> = doc(hidden_then_shown_figures())
        .warnings
        .into_iter()
        .filter(|w| w.message.contains("fig-hidden") && w.message.contains("cross-referenc"))
        .collect();
    let w = ws.first().expect("the unreferenceable warning fires");
    assert_eq!(
        w.line,
        Some(5),
        "the warning must point at the cell that declares the label, got {:?}",
        w.line
    );
}

// --- No false positives: the fix must not over-reach ------------------------------

#[test]
fn an_echo_false_figure_cell_still_registers_its_anchor() {
    // `echo: false` hides only the SOURCE; the output — and with it the anchor — still
    // renders. Keying the gate on `echo` instead of `include` would break this.
    let src = "---\ntitle: T\n---\n\n\
               ```{python}\n#| label: fig-quiet\n#| fig-cap: Source hidden, figure shown\n\
               #| echo: false\nplot(1)\n```\n\n\
               See @fig-quiet.\n";
    let body = doc(src).body_html();
    assert_eq!(
        xref_link_text(&body, "fig-quiet").as_deref(),
        Some("Figure&nbsp;1"),
        "an `echo: false` figure still materializes and must stay referenceable; got:\n{body}"
    );
    let ws = warnings(src);
    assert!(
        !ws.iter().any(|m| m.contains("cross-referenc")),
        "an `echo: false` figure is referenceable and must not warn: {ws:?}"
    );
}

// Both names in the `matches!(lang, "mermaid" | "js")` exemption need their own pin.
// With only the mermaid one, deleting `| "js"` passed the ENTIRE core suite (measured),
// which would hide a real `{js}` figure's anchor and burn its number.
//
// NOTE these two pin CURRENT behavior that contradicts the documented contract: `include:
// false` is specified as "hides source AND output" with no lang carve-out, yet mermaid and
// `{js}` figures render fully visible under it. This fix does not change that (it is the
// reason their anchors are real), but if the contract is ever enforced for these langs,
// these two tests must be DELETED, not repaired — the exemption disappears with the bug.
#[test]
fn an_include_false_mermaid_figure_stays_referenceable() {
    let src = "---\ntitle: T\n---\n\n\
               ```{mermaid}\n%%| label: fig-graph\n%%| fig-cap: Rendered at render time\n\
               %%| include: false\ngraph TD\n  A --> B\n```\n\n\
               See @fig-graph.\n";
    let body = doc(src).body_html();
    assert!(
        body.contains(r##"id="fig-graph""##),
        "a mermaid figure is emitted at render time; the anchor must exist:\n{body}"
    );
    assert_eq!(
        xref_link_text(&body, "fig-graph").as_deref(),
        Some("Figure&nbsp;1"),
        "a mermaid figure's anchor is real, so the ref must resolve; got:\n{body}"
    );
}

#[test]
fn an_include_false_js_figure_stays_referenceable() {
    let src = "---\ntitle: T\n---\n\n\
               ```{js}\n//| label: fig-plot\n//| fig-cap: Rendered at render time\n\
               //| include: false\n1 + 1\n```\n\n\
               See @fig-plot.\n";
    let body = doc(src).body_html();
    assert!(
        body.contains(r##"id="fig-plot""##),
        "a `{{js}}` figure is emitted at render time; the anchor must exist:\n{body}"
    );
    assert_eq!(
        xref_link_text(&body, "fig-plot").as_deref(),
        Some("Figure&nbsp;1"),
        "a `{{js}}` figure's anchor is real, so the ref must resolve; got:\n{body}"
    );
}

#[test]
fn an_include_false_cell_without_a_label_does_not_warn() {
    // No label, no anchor, nothing to advertise — hiding output is then just hiding
    // output, which is what `include: false` is for.
    let src = "---\ntitle: T\n---\n\n\
               ```{python}\n#| include: false\nsetup()\n```\n\n\
               Text.\n";
    let ws = warnings(src);
    assert!(
        !ws.iter().any(|m| m.contains("cross-referenc")),
        "an unlabelled `include: false` cell is ordinary usage and must not warn: {ws:?}"
    );
}

#[test]
fn a_plainly_labelled_include_false_setup_cell_does_not_warn() {
    // The commonest real usage, and the only shape the corpus actually contains
    // (`#| label: setup` + `#| include: false`). A plain label takes no CellRole and
    // registers nothing, so it must stay silent.
    let src = "---\ntitle: T\n---\n\n\
               ```{r}\n#| label: setup\n#| include: false\nlibrary(tidyverse)\n```\n\n\
               Text.\n";
    let ws = warnings(src);
    assert!(
        !ws.iter().any(|m| m.contains("cross-referenc")),
        "`label: setup` is not a cross-reference anchor and must not warn: {ws:?}"
    );
}

#[test]
fn an_ordinary_labelled_figure_cell_does_not_warn() {
    let src = "---\ntitle: T\n---\n\n\
               ```{python}\n#| label: fig-ok\n#| fig-cap: A normal figure\nplot(1)\n```\n\n\
               See @fig-ok.\n";
    let ws = warnings(src);
    assert!(
        !ws.iter().any(|m| m.contains("cross-referenc")),
        "an included labelled figure is referenceable and must not warn: {ws:?}"
    );
}
