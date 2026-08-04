//! `::: {.debug}` emission. Asserts the DOM contract `debug.js` depends on, so a change
//! to either side that breaks the pair fails here rather than silently in a browser.

use std::path::Path;

fn render(src: &str) -> String {
    taliesin_core::render_document_with_includes(src, Path::new("."))
        .blocks
        .iter()
        .map(|b| b.html.clone())
        .collect::<Vec<_>>()
        .join("")
}

#[test]
fn debug_div_emits_a_line_wrapped_code_panel_and_a_hidden_reactive_input() {
    let out = render(
        "---\ntitle: T\n---\n\n::: {.debug name=\"sort\"}\n\
         ```{python}\n#| trace: true\na = [3, 1]\n```\n:::\n",
    );
    assert!(
        out.contains(
            r#"<div class="tali-debug column-page" role="group" aria-label="Algorithm debugger""#
        ),
        "the container must be labelled and escape the prose measure:\n{out}"
    );
    assert!(
        out.contains(
            r#"<input type="hidden" class="tali-debug-input" data-tali-input="sort" value="0">"#
        ),
        "stepping publishes through the SAME hidden-input bridge scrolly uses:\n{out}"
    );
    assert!(
        out.contains(r#"class="tali-hl-ln""#),
        "the code panel must be line-wrapped so the cursor has lines to address:\n{out}"
    );
}

#[test]
fn debug_div_without_a_name_still_renders_but_emits_no_bridge() {
    let out = render(
        "---\ntitle: T\n---\n\n::: {.debug}\n\
         ```{python}\n#| trace: true\na = 1\n```\n:::\n",
    );
    assert!(
        out.contains(r#"class="tali-debug"#),
        "still renders:\n{out}"
    );
    assert!(
        !out.contains("tali-debug-input"),
        "no name means nothing to address, so no bridge element:\n{out}"
    );
}

/// The line numbers in a trace index the DISPLAYED source. Both the executed code and
/// the rendered panel come from `strip_cell_options(&cb.literal)` (mod.rs:750 and
/// emit.rs:48), so `#| trace: true` must not shift the panel's line ordinals. If this
/// ever diverges the cursor silently points one line off on every traced cell.
#[test]
fn cell_option_lines_are_stripped_from_the_panel_so_ordinals_match_the_executed_source() {
    let out = render(
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\nfirst = 1\nsecond = 2\n```\n:::\n",
    );
    let lines = out.matches(r#"class="tali-hl-ln""#).count();
    assert_eq!(
        lines, 2,
        "the `#|` directive must not occupy a panel line; expected `first`/`second` only:\n{out}"
    );
}

/// Fix round 1, finding 1: `code_idx` (first code block) and `traced` (count of traced
/// cells) used to be computed independently, so a `.debug` with two code cells where the
/// SECOND is the traced one showed the wrong, untraced cell in the panel while `traced ==
/// 1` reported healthy: a silent misrender with no warning. The panel must follow the
/// trace, not document order.
#[test]
fn debug_panel_follows_the_traced_cell_even_when_it_is_not_the_first_code_block() {
    let out = render(
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\nfirst_cell = 1\n```\n\n\
         ```{python}\n#| trace: true\nsecond_cell = 2\n```\n:::\n",
    );
    let code_start = out.find(r#"class="dbg-code""#).expect("panel present");
    let views_start = out.find(r#"class="dbg-views""#).expect("views present");
    let panel = &out[code_start..views_start];
    assert!(
        panel.contains("second_cell"),
        "the panel must show the TRACED cell even though it's second in the div:\n{out}"
    );
    assert!(
        !panel.contains("first_cell"),
        "the untraced first cell must not end up in the panel:\n{out}"
    );
    let views = &out[views_start..];
    assert!(
        views.contains("first_cell"),
        "the untraced cell still rides along in the views slot:\n{out}"
    );
}

/// Fix round 1, finding 2: `trace_attr` used to be interpolated only into the
/// non-folded `<pre>` branches, so `#| code-fold: true` + `#| trace: true` together
/// never got `data-tali-trace="1"` and `.debug` wrongly warned "no traced cell" against
/// correct authoring. A folded traced cell must still carry the marker.
#[test]
fn a_folded_traced_cell_still_carries_the_trace_marker() {
    let out = render(
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\n#| code-fold: true\na = 1\n```\n:::\n",
    );
    assert!(
        out.contains(r#"data-tali-trace="1""#),
        "code-fold must not swallow the trace marker:\n{out}"
    );
}
