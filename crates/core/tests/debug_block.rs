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
