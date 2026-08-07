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

/// Fix round 1, finding 1: a traced `{js}` cell's `//| input:` names must survive
/// into the DOM so `debug.js` knows which reactive inputs should trigger a
/// re-capture. The server strips `//|` option lines from the displayed source, so
/// `data-debug-inputs` on the container is the only place they land.
#[test]
fn a_traced_js_cell_carries_its_input_names_onto_the_container() {
    let out = render(
        "---\ntitle: T\n---\n\n::: {.debug name=\"sort\"}\n\
         ```{js}\n//| trace: true\n//| input: n, seed\n\
         function* g() { yield 1; }\nreturn g();\n```\n:::\n",
    );
    assert!(
        out.contains(r#"data-debug-inputs="n,seed""#),
        "the container must carry both input names, comma-joined:\n{out}"
    );
}

/// A cell with no `//| input:` (or a Python cell, which never parses one at all)
/// must not emit an empty/dangling attribute: nothing to subscribe to means nothing
/// to declare.
#[test]
fn a_traced_cell_with_no_inputs_emits_no_inputs_attribute() {
    let out = render(
        "---\ntitle: T\n---\n\n::: {.debug name=\"d\"}\n\
         ```{python}\n#| trace: true\na = 1\n```\n:::\n",
    );
    assert!(
        !out.contains("data-debug-inputs"),
        "no //| input: means nothing to declare:\n{out}"
    );
}

/// The whole document, so a test can read `warnings` (and a block's `cell`) rather than
/// only the emitted HTML the `render` helper above keeps.
fn doc(src: &str) -> taliesin_core::RenderedDoc {
    taliesin_core::render_document_with_includes(src, Path::new("."))
}

/// `#| trace: true` on a language taliesin cannot step must SAY so, at the option's own
/// line.
///
/// Everything between the fence and the executor is language-blind: `emit.rs` stamps
/// `data-tali-trace="1"` on any cell carrying the option, and the `.debug` branch counts
/// any such cell as its one traced cell. Reproduced against a live IRkernel before fixing
/// (see `crates/server/tests/r_kernel.rs`): an `{r}` cell in a `.debug` div was handed the
/// PYTHON harness's source to parse and rendered `Error in parse(text = input):
/// <text>:2:5: unexpected input`, while `taliesin check` reported **zero** diagnostics,
/// because as far as the div was concerned it had found exactly the one traced cell it
/// wanted. A raw parse error from another language's kernel is worse than silence, and
/// this project does not ship silence either.
#[test]
fn a_trace_option_on_a_language_that_cannot_be_stepped_is_diagnosed_at_its_own_line() {
    let d = doc("---\ntitle: T\n---\n\n::: {.debug name=\"s\"}\n\
         ```{r}\n#| trace: true\nx <- c(3, 1)\n```\n:::\n");
    let w = d
        .warnings
        .iter()
        .find(|w| w.message.contains("cannot step a"))
        .unwrap_or_else(|| panic!("no `cannot step` warning in {:?}", d.warnings));
    assert!(
        w.message.contains("`{r}`")
            && w.message.contains("`{python}`")
            && w.message.contains("`{js}`"),
        "the message must name the offending language AND the supported set: {}",
        w.message
    );
    // Located on the `#| trace: true` line itself (line 6 is the fence, 7 the option), so
    // click-to-source lands on the option the author has to delete.
    assert_eq!(w.line, Some(7), "{w:?}");

    // A `{python}` or `{js}` cell must stay silent, or the warning is worthless.
    for lang in ["python", "js"] {
        let ok = doc(&format!(
            "---\ntitle: T\n---\n\n::: {{.debug name=\"s\"}}\n\
             ```{{{lang}}}\n#| trace: true\nx = 1\n```\n:::\n"
        ));
        assert!(
            !ok.warnings
                .iter()
                .any(|w| w.message.contains("cannot step a")),
            "`{{{lang}}}` is a supported trace language: {:?}",
            ok.warnings
        );
    }

    // `trace: false` on an unsupported language is not a mistake, it is the default said
    // out loud. Only the value that actually turns tracing on warns.
    let off = doc("---\ntitle: T\n---\n\n::: {.debug name=\"s\"}\n\
         ```{r}\n#| trace: false\nx <- 1\n```\n:::\n");
    assert!(
        !off.warnings
            .iter()
            .any(|w| w.message.contains("cannot step a")),
        "{:?}",
        off.warnings
    );
}

/// A `.debug` nested inside ANY other fenced div still reaches the executor.
///
/// This used to warn (`validate_nested_debug`) and give up. A fenced div's composite block
/// folds its children's HTML into one string and carries at most one `Cell`; `.debug` works
/// because it hoists its traced cell onto its own container, and a wrapping div folded THAT
/// container away in turn, carrying no cell — the trace never ran, and the reader got a dead
/// code panel. The warning was the honest answer only for as long as the fix could rescue
/// just one `.debug` per wrapper; item 210 replaced it with `Block::nested`, which carries
/// every folded cell, so two algorithms in a `.panel-tabset` both work and the warning is
/// gone rather than half-true.
///
/// `.debug` is still the one container that hoists onto `cell` rather than `nested` (its
/// trace lands on the SIBLING output block `debug.js` reads), so what the wrapper collects
/// here is that hoisted cell — and collecting it exactly once is the property under test:
/// twice would re-run the traced cell on every rebuild.
#[test]
fn a_debug_div_nested_inside_another_div_still_reaches_the_executor() {
    for wrapper in [".callout-note", ".panel-tabset", ".column-page"] {
        let src = format!(
            "---\ntitle: T\n---\n\n::: {{{wrapper}}}\n\n\
             ::: {{.debug name=\"s\"}}\n\
             ```{{python}}\n#| trace: true\na = 1\n```\n:::\n\n:::\n"
        );
        let d = doc(&src);
        assert!(
            !d.warnings
                .iter()
                .any(|w| w.message.contains("nested inside another div")),
            "{wrapper}: the nesting limitation is gone, so its warning must be too: {:?}",
            d.warnings
        );
        let top = d
            .blocks
            .iter()
            .find(|b| b.html.contains("tali-debug"))
            .expect("the wrapper block");
        assert_eq!(
            top.nested.len(),
            1,
            "{wrapper}: the wrapper must carry the folded traced cell exactly once \
             (twice re-runs it every rebuild): {top:?}"
        );
        // …and the output slot the executor fills is keyed to that same cell.
        let id = &top.nested[0].id;
        assert!(
            top.html
                .contains(&format!("data-tali-out-for=\"{id}\"></div>")),
            "{wrapper}: no output slot for the folded cell {id}: {}",
            top.html
        );
    }

    // A top-level `.debug` is unchanged: it hands the executor its traced cell directly,
    // and its trace still arrives as the sibling output block `debug.js` looks for.
    let ok = doc("---\ntitle: T\n---\n\n::: {.debug name=\"s\"}\n\
         ```{python}\n#| trace: true\na = 1\n```\n:::\n");
    let top = ok
        .blocks
        .iter()
        .find(|b| b.html.contains("tali-debug"))
        .expect("the debug block");
    assert!(
        top.cell.is_some(),
        "a top-level `.debug` still hands the executor its traced cell: {top:?}"
    );
    assert!(
        top.nested.is_empty(),
        "the hoisted cell must not ALSO be collected as a nested one: {top:?}"
    );
}

/// Two `.debug` blocks sharing one `name=` collide silently in `debug.js`: the second
/// block's `registry[name] = ...` overwrites the first's (`mount`/`recapture`), so
/// `tali.frame(name)` in a downstream view cell reads whichever block ran last, and both
/// blocks' `[data-tali-input]` bridges fight over the same reactive-graph edge and the
/// same URL fragment. This project treats a silent collision as an authoring mistake, so
/// it must warn rather than stay quiet.
#[test]
fn a_duplicate_debug_name_on_one_page_warns_and_locates_the_second_block() {
    let src = "---\ntitle: T\n---\n\n\
        ::: {.debug name=\"sort\"}\n\
        ```{python}\n#| trace: true\na = [2, 1]\n```\n:::\n\n\
        ::: {.debug name=\"sort\"}\n\
        ```{python}\n#| trace: true\nb = [3, 1]\n```\n:::\n";
    let d = doc(src);
    let w = d
        .warnings
        .iter()
        .find(|w| w.message.contains("duplicate") && w.message.contains("sort"))
        .unwrap_or_else(|| panic!("no duplicate-name warning in {:?}", d.warnings));
    // Located on the SECOND block's own opening fence (line 12), not the first's (line
    // 5): the same "locate the duplicate, keep the first" convention the duplicate
    // cross-reference-label warning already uses.
    assert_eq!(w.line, Some(12), "{w:?}");

    // Only ONE such warning: the first definition is not itself flagged.
    let count = d
        .warnings
        .iter()
        .filter(|w| w.message.contains("duplicate") && w.message.contains("sort"))
        .count();
    assert_eq!(count, 1, "{:?}", d.warnings);

    // Two DIFFERENTLY-named `.debug` blocks must stay silent.
    let ok = doc("---\ntitle: T\n---\n\n\
         ::: {.debug name=\"a\"}\n\
         ```{python}\n#| trace: true\nx = 1\n```\n:::\n\n\
         ::: {.debug name=\"b\"}\n\
         ```{python}\n#| trace: true\ny = 1\n```\n:::\n");
    assert!(
        !ok.warnings.iter().any(|w| w.message.contains("duplicate")),
        "{:?}",
        ok.warnings
    );

    // Two UNNAMED `.debug` blocks are unaddressable but not a name COLLISION, so they
    // must not warn about a duplicate name either.
    let unnamed = doc("---\ntitle: T\n---\n\n\
         ::: {.debug}\n```{python}\n#| trace: true\nx = 1\n```\n:::\n\n\
         ::: {.debug}\n```{python}\n#| trace: true\ny = 1\n```\n:::\n");
    assert!(
        !unnamed
            .warnings
            .iter()
            .any(|w| w.message.contains("duplicate `.debug`")),
        "{:?}",
        unnamed.warnings
    );
}
