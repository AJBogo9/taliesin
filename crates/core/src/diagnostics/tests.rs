use super::*;
use crate::render::{Warning, render_document, render_document_scoped_with_site};
use std::path::Path;

/// A throwaway directory under the system temp dir, removed on drop.
struct Tmp(std::path::PathBuf);
impl Tmp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "tali-diag-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn msgs(ws: &[Warning]) -> Vec<String> {
    ws.iter().map(|w| w.message.clone()).collect()
}

#[test]
fn local_links_flag_missing_relative_target_only() {
    let dir = Tmp::new("links");
    std::fs::write(dir.0.join("exists.tmd"), "x").unwrap();
    let doc = render_document(
        "[gone](missing.tmd) [here](exists.tmd) [ext](https://example.com) \
         [page](sub/page.html#frag) [anchor](#top) [abs](/root.html)\n",
    );
    let ws = validate_local_links(&doc.blocks, &dir.0);
    let m = msgs(&ws);
    assert_eq!(m.len(), 2, "only the two missing local files: {m:?}");
    assert!(m.iter().any(|s| s.contains("`missing.tmd`")), "{m:?}");
    assert!(m.iter().any(|s| s.contains("`sub/page.html`")), "{m:?}");
    // The existing sibling, external, in-page anchor, and absolute links are clean.
    assert!(!m.iter().any(|s| s.contains("exists.tmd")), "{m:?}");
    assert!(!m.iter().any(|s| s.contains("example.com")), "{m:?}");
    assert!(!m.iter().any(|s| s.contains("/root.html")), "{m:?}");
    // Located to a line.
    assert!(ws.iter().all(|w| w.line.is_some()), "located: {ws:?}");
}

#[test]
fn local_links_skip_xref_links() {
    // A `@sec-`/`@fig-` cross-reference renders an `<a … data-tali-xref>`; it is
    // validated by `validate_xrefs`, so the link checker must not double-flag it.
    let doc = render_document("## Sec {#sec-a}\n\nSee @sec-a.\n");
    let ws = validate_local_links(&doc.blocks, Path::new("."));
    assert!(msgs(&ws).is_empty(), "xref link must be skipped: {ws:?}");
}

#[test]
fn local_links_flag_html_link_whose_only_source_is_the_retired_ext() {
    // After the .tmd-only flip, a `.qmd` file on disk is no longer a recognized source:
    // an `.html` link (or directory link) whose only on-disk source is `.qmd` is now
    // flagged broken, same as any other missing target.
    let dir = Tmp::new("links-html-retired-ext-gone");
    std::fs::write(dir.0.join("page.qmd"), "x").unwrap();
    std::fs::create_dir_all(dir.0.join("guide")).unwrap();
    std::fs::write(dir.0.join("guide/index.qmd"), "x").unwrap();
    let doc =
        render_document("[built page](page.html) [dir link](guide/) [really gone](ghost.html)\n");
    let m = msgs(&validate_local_links(&doc.blocks, &dir.0));
    assert_eq!(
        m.len(),
        3,
        "page.html, guide/, and ghost.html are all now broken (.qmd is not a source): {m:?}"
    );
    assert!(m.iter().any(|s| s.contains("`page.html`")), "{m:?}");
    assert!(m.iter().any(|s| s.contains("`guide/`")), "{m:?}");
    assert!(m.iter().any(|s| s.contains("`ghost.html`")), "{m:?}");
}

#[test]
fn local_links_accept_html_link_with_tmd_source() {
    // Unlike `local_links_flag_html_link_whose_only_source_is_the_retired_ext`, the on-disk source
    // here is spelled `.tmd` (Taliesin's native and only source extension), so the
    // probe finds it and the `.html` link resolves clean.
    let dir = Tmp::new("links-html-tmd");
    std::fs::write(dir.0.join("page.tmd"), "x").unwrap();
    std::fs::create_dir_all(dir.0.join("guide")).unwrap();
    std::fs::write(dir.0.join("guide/index.tmd"), "x").unwrap();
    let doc =
        render_document("[built page](page.html) [dir link](guide/) [really gone](ghost.html)\n");
    let m = msgs(&validate_local_links(&doc.blocks, &dir.0));
    assert_eq!(m.len(), 1, "only the truly missing target: {m:?}");
    assert!(m[0].contains("`ghost.html`"), "{m:?}");
}

#[test]
fn js_reactive_graph_flags_dangling_input() {
    let doc = render_document(
        "```{js}\n//| viewof: n\nreturn html`<input type=range>`;\n```\n\n\
         ```{js}\n//| input: n, missing\nreturn n;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert_eq!(
        m,
        [
            "unknown reactive input `missing`: no `{js}` cell, `{{< input >}}` or Python \
          `define(...)` defines it"
        ],
        "only `missing` is dangling (`n` is defined)"
    );
}

/// A `{js}` cell inside a `:::` container is invisible to the block model: `Block::nested`
/// records only the cells the KERNEL runs (they are the ones needing an output slot), so a
/// folded client cell has no `Block` and no `Cell` left anywhere. The validator read `b.cell`
/// and therefore reported error-severity "unknown reactive input `n`" for a page that runs
/// perfectly in both the preview and the build, which made `build --check-only` exit 1 on a
/// WORKING document. The inverse held too: a genuinely dangling input among folded cells was
/// never reported.
#[test]
fn js_reactive_graph_sees_cells_folded_into_a_container() {
    // The producer is folded into a grid, the consumer is top level. Nothing is wrong here.
    let doc = render_document(
        "::: {layout-ncol=2}\n```{js}\n//| viewof: n\nreturn html`<input type=range>`;\n```\n\n\
         Side.\n:::\n\n```{js}\n//| input: n\nreturn n * 2;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(
        m.is_empty(),
        "a working page must draw no diagnostic: {m:?}"
    );

    // And the other direction: a dangling input INSIDE a container is still reported, at the
    // folded cell's own line rather than the container's.
    let bad =
        render_document("::: {.callout-note}\n```{js}\n//| input: nope\nreturn nope;\n```\n:::\n");
    let w = validate_js_reactive_graph(&bad.blocks);
    let m = msgs(&w);
    assert_eq!(m.len(), 1, "the folded dangling input is reported: {m:?}");
    assert!(m[0].contains("unknown reactive input `nope`"), "{m:?}");
    assert_eq!(w[0].line, Some(2), "located at the cell, not the container");

    // A cycle between two folded cells is a cycle, and pooling a container's cells into one
    // node would also invent one where the two are merely neighbours — so assert both.
    let cyc = render_document(
        "::: {.callout-note}\n```{js}\n//| name: a\n//| input: b\nreturn b;\n```\n\n\
         ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n:::\n",
    );
    let m = msgs(&validate_js_reactive_graph(&cyc.blocks));
    assert_eq!(m.len(), 2, "both folded cells are in the cycle: {m:?}");
    assert!(m.iter().all(|x| x.contains("dependency cycle")), "{m:?}");

    let pair = render_document(
        "::: {.callout-note}\n```{js}\n//| name: a\nreturn 1;\n```\n\n\
         ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n:::\n",
    );
    let m = msgs(&validate_js_reactive_graph(&pair.blocks));
    assert!(m.is_empty(), "two folded cells are not a cycle: {m:?}");
}

/// The `runtime_defines` escape hatch has the same blind spot. A `{python}` bridge cell whose
/// `define(...)` passes a positional argument publishes names the static pass cannot
/// enumerate, so that call suppresses the dangling-input half, but asked as `b.cell` it
/// missed a bridge cell inside a container, leaving the check armed and the page drawing a
/// false error.
#[test]
fn a_kernel_define_bridge_inside_a_container_still_suppresses_dangling_inputs() {
    let doc = render_document(
        "::: {.callout-note}\n```{python}\ndefine(\"n\", 3)\n```\n:::\n\n\
         ```{js}\n//| input: n\nreturn n;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(
        m.is_empty(),
        "a folded bridge cell suppresses the check exactly like a top-level one: {m:?}"
    );
}

#[test]
fn js_reactive_graph_did_you_mean_over_defines() {
    let doc = render_document(
        "```{js}\n//| name: count\nreturn 1;\n```\n\n\
         ```{js}\n//| input: cont\nreturn cont;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("did you mean `count`?"), "{m:?}");
}

#[test]
fn js_reactive_graph_input_shortcode_define_clears_dangling() {
    // A declarative `{{< input name="k" >}}` defines `k`, so a cell consuming it is clean.
    let dir = Tmp::new("js-input");
    let doc = render_document_scoped_with_site(
        "{{< input name=\"k\" type=\"slider\" min=\"0\" max=\"10\" >}}\n\n\
         ```{js}\n//| input: k\nreturn k;\n```\n",
        &dir.0,
        None,
        None,
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(m.is_empty(), "shortcode-defined input must resolve: {m:?}");
}

#[test]
fn js_reactive_graph_detects_cycle() {
    let doc = render_document(
        "```{js}\n//| name: a\n//| input: b\nreturn b;\n```\n\n\
         ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    // Both cells are undrained -> one cycle warning each; `a` and `b` are mutually defined.
    assert_eq!(m.len(), 2, "both cycle members flagged: {m:?}");
    assert!(
        m.iter().all(|s| s.contains("reactive dependency cycle")),
        "{m:?}"
    );
}

/// `define` is `def define(**kwargs)` (the kernel preamble), so every name it can publish is
/// a keyword spelled in the call, and the static pass reads it there. Suppressing the check
/// page-wide whenever a cell called `define(` switched it off on most of the real blog
/// posts, exactly the pages where a typo'd `//| input:` is likeliest.
#[test]
fn a_keyword_define_is_read_statically_so_a_typo_still_draws_did_you_mean() {
    let doc = render_document(
        "```{python}\ndefine(runtime_name=5)\n```\n\n\
         ```{js}\n//| input: runtim_name\nreturn runtim_name;\n```\n",
    );
    let w = validate_js_reactive_graph(&doc.blocks);
    assert_eq!(
        msgs(&w),
        [
            "unknown reactive input `runtim_name`: no `{js}` cell, `{{< input >}}` or Python \
          `define(...)` defines it (did you mean `runtime_name`?)"
        ]
    );
    assert_eq!(w[0].severity, crate::render::Severity::Error);
    assert_eq!(w[0].line, Some(5), "located at the consuming cell");
}

/// The call shapes the real posts write: multi-line, a trailing comma, spaces around `=`,
/// nested brackets, an f-string with braces, and comments that carry commas and brackets
/// of their own. Every keyword is read, and nothing inside a string or a comment is taken
/// for an argument or a closing parenthesis.
#[test]
fn a_multi_line_keyword_define_publishes_every_keyword() {
    let doc = render_document(
        "```{python}\nx = 1  # not a define(call)\n\
         define(\n    pts = X.tolist(),  # pcs[0] = PC1 vector, etc.\n\
         \x20   # a comment line, with (a paren\n\
         \x20   ticks=[f\"{m + 2:.0f} cm)\" for m in mu],\n\
         \x20   pair={\"k\": (1, 2)},\n)\nredefine(ghost=1)\n```\n\n\
         ```{js}\n//| input: pts, ticks, pair\nreturn pts;\n```\n\n\
         ```{js}\n//| input: ghost\nreturn ghost;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert_eq!(
        m.len(),
        1,
        "every keyword resolves, and `redefine(` is not the bridge: {m:?}"
    );
    assert!(m[0].contains("unknown reactive input `ghost`"), "{m:?}");
}

/// The fallback: a `**` splat publishes names no static pass can enumerate, so the
/// dangling-input half stays suppressed page-wide, exactly as before.
#[test]
fn a_splat_define_still_suppresses_dangling_inputs() {
    let doc = render_document(
        "```{python}\nd = {\"n\": 1}\ndefine(**d)\n```\n\n\
         ```{js}\n//| input: anything\nreturn anything;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(m.is_empty(), "a splat must suppress: {m:?}");
}

/// A call the static read cannot finish (no balancing `)`) is treated like a splat.
#[test]
fn an_unparseable_define_still_suppresses_dangling_inputs() {
    let doc = render_document(
        "```{python}\ndefine(a=1\n```\n\n\
         ```{js}\n//| input: anything\nreturn anything;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(m.is_empty(), "an unparseable call must suppress: {m:?}");
}

/// The dangling-input messages for a page holding `fence` and then one `{js}` cell that
/// reads `x`.
fn dangling_x_after(fence: &str) -> Vec<String> {
    let doc = render_document(&format!(
        "{fence}\n\n```{{js}}\n//| input: x\nreturn x;\n```\n"
    ));
    msgs(&validate_js_reactive_graph(&doc.blocks))
        .into_iter()
        .filter(|m| m.contains("unknown reactive input"))
        .collect()
}

/// Only a cell that runs on a kernel executes its `define`. A display fence's text is never
/// run, so it neither defines `x` nor switches the check off: a Scheme `(define (f y) …)`
/// used to read as a positional call and silence the whole page.
#[test]
fn a_define_in_a_display_fence_neither_defines_nor_suppresses() {
    for fence in [
        "```{scheme}\n(define (f y) (* y y))\n```",
        "```{bash}\ndefine(x=1)\n```",
        "```{c}\nint define(int q);\n```",
    ] {
        assert_eq!(dangling_x_after(fence).len(), 1, "for {fence:?}");
    }
}

/// The define bridge is a side channel, not visible output: the `<script type="tali-define">`
/// blob survives `include: false`, so a define in an `include: false` cell still publishes
/// its names. A keyword call defines them, so a typo beside one is still reported, and a
/// splat still suppresses the check.
#[test]
fn an_include_false_define_still_publishes_its_names() {
    for body in ["define(x=1)", "d = {\"x\": 1}\ndefine(**d)"] {
        let fence = format!("```{{python}}\n#| include: false\n{body}\n```");
        assert!(dangling_x_after(&fence).is_empty(), "for {body:?}");
    }
    let doc = render_document(
        "```{python}\n#| include: false\ndefine(x=1)\n```\n\n\
         ```{js}\n//| input: x, nope\nreturn x;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(
        m.len() == 1 && m[0].contains("`nope`"),
        "`x` is defined and `nope` is reported: {m:?}"
    );
}

/// `obj.define(...)` is some object's method, not the kernel bridge.
#[test]
fn a_method_named_define_is_not_the_bridge() {
    for call in ["obj.define(x=1)", "obj.define(**d)", "obj . define(1)"] {
        let fence = format!("```{{python}}\n{call}\n```");
        assert_eq!(dangling_x_after(&fence).len(), 1, "for {call:?}");
    }
}

/// A `#` comment is never code, so a `define` in one is not a call.
#[test]
fn a_define_in_a_comment_is_ignored() {
    assert_eq!(
        dangling_x_after("```{python}\n# define(x=1) later\ny = 1\n```").len(),
        1
    );
}

/// An f-string field is code the static read skips as a string, so the call inside it
/// could run unseen.
#[test]
fn a_define_inside_an_f_string_field_suppresses() {
    assert!(dangling_x_after("```{python}\nprint(f\"{define(x=1)}\")\n```").is_empty());
}

/// A string handed to `exec` runs too.
#[test]
fn a_define_inside_an_exec_string_suppresses() {
    assert!(dangling_x_after("```{python}\nexec(\"define(x=1)\")\n```").is_empty());
}

/// An alias calls the bridge under a name the static read does not follow.
#[test]
fn an_aliased_define_suppresses() {
    assert!(dangling_x_after("```{python}\nd = define\nd(x=1)\n```").is_empty());
}

/// A backslash continuation puts the call's `(` on the next line.
#[test]
fn a_define_continued_by_a_backslash_suppresses() {
    assert!(dangling_x_after("```{python}\ndefine \\\n    (x=1)\n```").is_empty());
}

/// Inside brackets a call's `(` may sit on the next line with no backslash.
#[test]
fn a_define_with_its_paren_on_the_next_line_suppresses() {
    assert!(dangling_x_after("```{python}\nr = (define\n     (x=1))\n```").is_empty());
}

/// A call nested in another call's arguments publishes too, and the outer read skips it.
#[test]
fn a_define_nested_in_another_define_suppresses() {
    assert!(dangling_x_after("```{python}\ndefine(y=define(x=1))\n```").is_empty());
}

/// The other half, and the one the narrowing bought. A Python cell that does NOT call
/// `define(` publishes nothing into the reactive graph, so the broken reference below is
/// reported — where until 2026-08-03 merely *having* a `{python}` cell switched the check
/// off, which is every real blog post in the corpus.
#[test]
fn js_dangling_input_is_reported_when_the_python_cell_defines_nothing() {
    let doc = render_document(
        "```{python}\nx = 5\n```\n\n\
         ```{js}\n//| input: runtime_name\nreturn runtime_name;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(
        m.iter().any(|s| s.contains("`runtime_name`")),
        "a kernel cell that defines nothing must not suppress the check: {m:?}"
    );
}

#[test]
fn js_cycle_still_flagged_with_python_cell_present() {
    // The cycle half is a structural fact among js cells; it survives a python cell.
    let doc = render_document(
        "```{python}\nx = 1\n```\n\n\
         ```{js}\n//| name: a\n//| input: b\nreturn b;\n```\n\n\
         ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(
        m.iter().filter(|s| s.contains("cycle")).count() == 2,
        "cycle still flagged: {m:?}"
    );
}

#[test]
fn js_reactive_graph_clean_chain_is_silent() {
    // n -> squared -> consumer, no cycle, every input defined.
    let doc = render_document(
        "```{js}\n//| viewof: n\nreturn html`<input type=range>`;\n```\n\n\
         ```{js}\n//| name: squared\n//| input: n\nreturn n*n;\n```\n\n\
         ```{js}\n//| input: squared\nreturn squared;\n```\n",
    );
    assert!(
        validate_js_reactive_graph(&doc.blocks).is_empty(),
        "a clean reactive chain must be silent"
    );
}

#[test]
fn a11y_flags_heading_level_skip_mid_document() {
    // h2 -> h4 skips h3; flagged, located. The leading h2 (no prior heading) is fine,
    // and "doesn't start at h1" is never flagged.
    let doc = render_document("## Top\n\nbody\n\n#### Deep\n\nmore\n");
    let ws = validate_a11y(&doc.blocks);
    let m = msgs(&ws);
    assert_eq!(m.len(), 1, "only the h2->h4 skip: {m:?}");
    assert!(m[0].contains("heading level skips from h2 to h4"), "{m:?}");
    assert!(ws[0].line.is_some(), "located: {ws:?}");
}

/// The outline is every heading ELEMENT on the page, including those a `:::` container
/// holds: a container is one block whose html carries its children, and the rule asked
/// only about each block's root element. So an h2 -> h4 skip inside `::: {.foo}` was never
/// reported, and an h3 in `.column-margin` was invisible, which made the h4 after it a
/// false "skips from h2 to h4". Each skip is located at its own heading.
#[test]
fn a11y_walks_every_heading_a_container_holds() {
    let doc = render_document(
        "## Two\n\n::: {.foo}\n#### Four inside\n:::\n\n## Two again\n\n\
         ::: {.column-margin}\n### Three in margin\n:::\n\n#### Four after\n",
    );
    let ws = validate_a11y(&doc.blocks);
    let got: Vec<(String, Option<u32>)> = ws.iter().map(|w| (w.message.clone(), w.line)).collect();
    assert_eq!(
        got.len(),
        1,
        "exactly the skip inside the container: {got:?}"
    );
    assert!(got[0].0.contains("from h2 to h4"), "{got:?}");
    assert_eq!(got[0].1, Some(4), "located at the heading itself: {got:?}");
}

#[test]
fn a11y_one_level_deeper_is_fine() {
    // h2 -> h3 is a single step, not a skip; never flagged.
    let doc = render_document("## A\n\n### B\n\n#### C\n");
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(m.is_empty(), "single-level steps must be silent: {m:?}");
}

#[test]
fn a11y_does_not_flag_first_heading_below_h1() {
    // A doc whose first heading is an h2 (a common pattern; the title is the h1) must
    // NOT be flagged — only a mid-document skip counts.
    let doc = render_document("## Section\n\nbody\n\n## Another\n");
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(m.is_empty(), "first-heading-below-h1 is not a skip: {m:?}");
}

#[test]
fn a11y_flags_raw_img_without_alt() {
    // A hand-written `<img>` with no `alt` is flagged; an `<img alt="">` (decorative)
    // and a markdown image (which always emits an alt) are clean.
    let doc = render_document(
        "<img src=\"logo.png\">\n\n<img src=\"ok.png\" alt=\"described\">\n\n\
         <img src=\"deco.png\" alt=\"\">\n\n![real alt](pic.png)\n",
    );
    let ws = validate_a11y(&doc.blocks);
    let m = msgs(&ws);
    assert_eq!(m.len(), 1, "only the alt-less raw img: {m:?}");
    assert!(m[0].contains("image is missing alt text"), "{m:?}");
    assert!(ws[0].line.is_some(), "located: {ws:?}");
}

#[test]
fn a11y_flags_placeholder_alt_but_not_descriptive() {
    // A non-empty but useless alt (a bare medium word, or an echo of the filename) is
    // flagged; a descriptive alt and alt="" (decorative) are clean. The common LLM tell.
    let doc = render_document(
        "![image](photo.png)\n\n\
         <img src=\"scree.png\" alt=\"scree.png\">\n\n\
         ![A scree plot of the eigenvalues](scree.png)\n\n\
         <img src=\"deco.png\" alt=\"\">\n",
    );
    let ws = validate_a11y(&doc.blocks);
    let m = msgs(&ws);
    assert_eq!(
        m.iter()
            .filter(|s| s.contains("looks like a placeholder"))
            .count(),
        2,
        "medium-word + filename-echo alts flagged, descriptive + decorative clean: {m:?}"
    );
    assert!(
        !m.iter().any(|s| s.contains("scree plot")),
        "a descriptive alt is never accused: {m:?}"
    );
}

/// The filename echo is judged against the FILE's name, so `my%20pic.png` (the spelling VS
/// Code inserts for `my pic.png`) is echoed by `my pic` exactly as `<my pic.png>` is. The
/// check compared against the undecoded `my%20pic`, and let that one spelling through.
#[test]
fn a11y_hears_a_filename_echo_through_percent_encoding() {
    let doc = render_document("![my pic](my%20pic.png)\n\n![my pic](<my pic.png>)\n");
    let m = msgs(&validate_a11y(&doc.blocks));
    assert_eq!(
        m.iter()
            .filter(|s| s.contains("looks like a placeholder"))
            .count(),
        2,
        "both spellings echo the file name: {m:?}"
    );
}

#[test]
fn a11y_clean_document_is_silent() {
    // Markdown headings stepping by one, a markdown image (auto-alt), and a text link:
    // no a11y warnings at all.
    let doc = render_document(
        "# Title\n\n## Section\n\n### Subsection\n\n\
         ![a described picture](pic.png)\n\nA [normal link](page.html).\n",
    );
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(m.is_empty(), "a clean doc must be silent: {m:?}");
}

/// A page that inherits its project's `_site.yml` `bibliography:` is never told that no
/// `bibliography:` is declared, even when every one of its citations fails to resolve (one
/// typo in a one-citation post): the broken-citation warning is the true one, and the
/// advice sent the author off to declare a file the project already declares (audit
/// 2026-09-24, bibtex #8).
#[test]
fn a_page_inheriting_the_project_bibliography_is_not_told_none_is_declared() {
    let dir = Tmp::new("bib-inherit");
    std::fs::write(
        dir.0.join("_site.yml"),
        "title: S\nbibliography: shared.bib\n",
    )
    .unwrap();
    std::fs::write(dir.0.join("shared.bib"), "@misc{shared1, title={S}}\n").unwrap();
    let src = "---\ntitle: P\n---\n\nSee [@shared2].\n";
    let doc = crate::render::render_single_doc(src, &dir.0);
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("broken citation")),
        "the true diagnostic: {:?}",
        doc.warnings
    );
    let w = citations_without_bibliography(src, &doc.blocks, &dir.0);
    assert!(w.is_empty(), "{:?}", msgs(&w));

    // A project bibliography that yields nothing (here: not UTF-8) is no inheritance at all.
    // Checked on its own, the page must not read clean: nothing else on this surface says
    // why every reference is a raw key.
    let unread = Tmp::new("bib-unread");
    std::fs::write(
        unread.0.join("_site.yml"),
        "title: S\nbibliography: shared.bib\n",
    )
    .unwrap();
    std::fs::write(
        unread.0.join("shared.bib"),
        b"@misc{shared2, title={M\xfcller}}\n",
    )
    .unwrap();
    let doc = crate::render::render_single_doc(src, &unread.0);
    let w = citations_without_bibliography(src, &doc.blocks, &unread.0);
    assert_eq!(w.len(), 1, "{:?}", msgs(&w));
    assert!(
        w[0].message
            .starts_with("citations are present but no `bibliography:`")
            && w[0].message.contains("_site.yml"),
        "{}",
        w[0].message
    );

    // The control: a page with no bibliography anywhere still gets the advice.
    let bare = Tmp::new("bib-none");
    let doc = crate::render::render_single_doc(src, &bare.0);
    let w = citations_without_bibliography(src, &doc.blocks, &bare.0);
    assert_eq!(w.len(), 1, "{:?}", msgs(&w));
}

// The bare-`@key` tests moved to `cite::tests` with the check itself, which now runs in
// the citation walk (`cite::render`) instead of scanning the finished HTML here.

// The `csl:` recognized-but-unsupported tests moved to `frontmatter::tests` with the rule
// itself, which now runs on the render path so the preview is not silent. This module is
// check-only, so testing it here would have pinned the wrong surface.

// ---- document-shape lints (item 24c) -------------------------------------------------
//
// Every threshold-bearing candidate was cut after measuring it against the corpus with
// `taliesin skim`; what survives is binary and threshold-free. The rules were calibrated
// on the real 14-project corpus, and the cases below pin both what they catch and, just
// as importantly, what they must not.

// ---- the validators read finished HTML through the one walker -------------------------

/// The check-superset must see the same references the build's asset copier does, and no
/// others. Every clause here is a defect reproduced against the release binary at
/// b69377fa, and the two directions cost different things: a MISSED reference is a broken
/// image the gate promises does not exist, and an INVENTED one fails `build --check-only`
/// on a page whose only sin is carrying a hand-written `<script>`.
#[test]
fn the_asset_check_reads_tags_not_a_substring_scan() {
    let dir = Tmp::new("assets-walker");
    let doc = render_document_scoped_with_site(
        concat!(
            "---\ntitle: T\n---\n\n",
            "![control](control-missing.png)\n\n",
            // A `>` inside `alt` is not the end of the tag. Ending the tag there hid the
            // `src` that follows it, so this image was never checked at all.
            "<img alt=\"width > height\" src=\"case-a-missing.png\">\n\n",
            // Raw HTML is in the trust model: the author's quoting is theirs to choose.
            "<img src='case-b-missing.png'>\n\n",
            // Script TEXT is not markup. The mermaid and Plot bundles every page inlines
            // build `<img src="${e}">` out of string fragments exactly like this.
            "<script>\nfunction card(e) { return '<img src=\"' + e + '\">'; }\n</script>\n",
        ),
        &dir.0,
        None,
        None,
    );
    let m = msgs(&validate_local_assets(&doc.blocks, &dir.0));
    assert_eq!(m.len(), 3, "exactly the three real broken images: {m:?}");
    for missing in [
        "control-missing.png",
        "case-a-missing.png",
        "case-b-missing.png",
    ] {
        assert!(
            m.iter().any(|s| s.contains(missing)),
            "{missing} missed: {m:?}"
        );
    }
    assert!(
        !m.iter().any(|s| s.contains("' + e + '")),
        "a script's string fragment is not an asset reference: {m:?}"
    );
}

/// A percent-encoded ref names the same file the dev server serves for it. The preview
/// decodes `%XX` in a request path, so `![x](my%20image.png)` beside a real
/// `my image.png` (the spelling VS Code inserts when a file whose name has spaces is
/// dragged in) must not be flagged; the angle-bracket spelling `![y](<my image.png>)`
/// stays the working control, an encoded ref whose decoded file is absent is still a
/// defect, and an invalid escape stays literal.
#[test]
fn the_asset_check_percent_decodes_a_ref_before_resolving_it() {
    let dir = Tmp::new("assets-pct");
    std::fs::write(dir.0.join("my image.png"), "x").unwrap();
    std::fs::write(dir.0.join("50%.png"), "x").unwrap();
    let doc = render_document_scoped_with_site(
        concat!(
            "---\ntitle: T\n---\n\n",
            "![spaced](my%20image.png)\n\n",
            "![control](<my image.png>)\n\n",
            "![literal](50%.png)\n\n",
            "![gone](still%20missing.png)\n",
        ),
        &dir.0,
        None,
        None,
    );
    let m = msgs(&validate_local_assets(&doc.blocks, &dir.0));
    assert_eq!(m.len(), 1, "only the truly missing file: {m:?}");
    assert!(
        m[0].contains("still missing.png"),
        "flagged under its decoded, on-disk name: {m:?}"
    );
}

/// The gate applies the build's publication rule (`includes::publishable`). It accepted any
/// file that existed, so `![x](../outside.png)`, an image symlinked out of the checkout and
/// one in a dot-folder all passed `--check-only --strict` while the site build never
/// shipped them: a broken image in the deploy behind a clean gate. Each is a located error
/// that says why; an image a page references in an `_images/` folder ships, so it passes.
#[test]
#[cfg(unix)]
fn the_asset_check_refuses_what_the_build_cannot_publish() {
    //   <dir>/.git                     the checkout
    //   <dir>/outside.png              in the checkout, above the project
    //   <dir>/proj/_site.yml           the project root
    //   <dir>/proj/.hidden/a.png       private
    //   <dir>/proj/_images/hero.png    referenced from an underscore folder
    //   <dir>/proj/posts/p/leak.png -> <elsewhere>/secret.png   out of the checkout
    let dir = Tmp::new("assets-publish");
    let elsewhere = Tmp::new("assets-publish-elsewhere");
    let root = dir.0.join("proj");
    let page = root.join("posts/p");
    for d in [".hidden", "_images", "posts/p"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(dir.0.join(".git"), "").unwrap();
    std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
    for f in [
        dir.0.join("outside.png"),
        root.join(".hidden/a.png"),
        root.join("_images/hero.png"),
        elsewhere.0.join("secret.png"),
    ] {
        std::fs::write(f, "x").unwrap();
    }
    std::os::unix::fs::symlink(elsewhere.0.join("secret.png"), page.join("leak.png")).unwrap();
    let doc = render_document_scoped_with_site(
        concat!(
            "![Above the project.](../../../outside.png)\n\n",
            "![In a dot folder.](../../.hidden/a.png)\n\n",
            "![Out of the checkout.](leak.png)\n\n",
            "![Referenced, so it ships.](../../_images/hero.png)\n",
        ),
        &page,
        None,
        None,
    );
    let ws = validate_local_assets(&doc.blocks, &page);
    let m = msgs(&ws);
    assert_eq!(ws.len(), 3, "exactly the three unpublishable images: {m:?}");
    for (w, (file, line)) in
        ws.iter()
            .zip([("outside.png", 1), (".hidden/a.png", 3), ("leak.png", 5)])
    {
        assert!(w.message.contains(file), "{file} missed: {m:?}");
        assert_eq!(w.severity, crate::render::Severity::Error, "{w:?}");
        assert_eq!(w.line, Some(line), "located at its own line: {w:?}");
    }
    assert!(
        !m.iter().any(|s| s.contains("hero.png")),
        "a referenced `_images/` file is published: {m:?}"
    );
}

/// `srcset` and `<picture><source srcset>` name images as surely as `src` does: the 2x
/// candidate a high-density screen fetches, the dark-mode source. The gate read `src` only,
/// so a `srcset` naming a file that does not exist passed `--strict`.
#[test]
fn the_asset_check_reads_every_srcset_candidate() {
    let dir = Tmp::new("assets-srcset");
    std::fs::write(dir.0.join("fig.png"), "x").unwrap();
    let doc = render_document_scoped_with_site(
        concat!(
            "<img src=\"fig.png\" srcset=\"fig.png 1x, missing-2x.png 2x\" alt=\"A.\">\n\n",
            "<picture><source srcset=\"missing-dark.png\" media=\"(prefers-color-scheme: dark)\">",
            "<img src=\"fig.png\" alt=\"B.\"></picture>\n",
        ),
        &dir.0,
        None,
        None,
    );
    let m = msgs(&validate_local_assets(&doc.blocks, &dir.0));
    assert_eq!(m.len(), 2, "exactly the two missing candidates: {m:?}");
    for missing in ["missing-2x.png", "missing-dark.png"] {
        assert!(
            m.iter().any(|s| s.contains(missing)),
            "{missing} missed: {m:?}"
        );
    }
}

/// A front-matter `image:` is the `og:image` a shared link unfurls with and the listing
/// card's thumbnail, and the page itself never shows it, so a typo is a defect the author
/// cannot see: it published an `og:image` that 404s under a clean `--strict`. It is held to
/// the body-image rule, located at the `image:` line; a `%20` spelling, a root-absolute
/// path (from the project root) and an external URL are all fine.
#[test]
fn a_front_matter_image_must_name_a_file_the_build_publishes() {
    let dir = Tmp::new("fm-image");
    let root = &dir.0;
    std::fs::create_dir_all(root.join("posts")).unwrap();
    std::fs::write(root.join("_site.yml"), "title: S\n").unwrap();
    std::fs::write(root.join("posts/my cover.png"), "x").unwrap();
    std::fs::write(root.join("brand.png"), "x").unwrap();
    let check = |image: &str| {
        let src = format!("---\ntitle: P\nimage: {image}\nimage-alt: A.\n---\n\nBody.\n");
        msgs_and_lines(&validate_front_matter_image(&src, &root.join("posts")))
    };

    let missing = check("typo-cover.png");
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert!(missing[0].0.contains("typo-cover.png"), "{missing:?}");
    assert_eq!(
        missing[0].1,
        Some(3),
        "located at the `image:` line: {missing:?}"
    );
    for fine in [
        "my%20cover.png",
        "\"my cover.png\"",
        "/brand.png",
        "https://cdn.example.com/card.png",
    ] {
        assert!(check(fine).is_empty(), "`image: {fine}` names a real file");
    }
    assert!(
        validate_front_matter_image("---\ntitle: P\n---\n\nx\n", root).is_empty(),
        "no `image:`, nothing to check"
    );
}

fn msgs_and_lines(ws: &[Warning]) -> Vec<(String, Option<u32>)> {
    ws.iter().map(|w| (w.message.clone(), w.line)).collect()
}

/// An `&` in a file name is an ordinary file name. The walker used to hand the checks the
/// value still entity-encoded, so a present `img/R&D.png` was reported missing as
/// `img/R&amp;D.png` and a working link to `Q&A data.csv` failed the gate. A really missing
/// file is still reported, under the name the author wrote.
#[test]
fn the_asset_and_link_checks_resolve_an_ampersand_in_a_file_name() {
    let dir = Tmp::new("amp");
    std::fs::create_dir_all(dir.0.join("img")).unwrap();
    std::fs::write(dir.0.join("img/R&D.png"), "x").unwrap();
    std::fs::write(dir.0.join("Q&A data.csv"), "x").unwrap();
    let doc = render_document_scoped_with_site(
        concat!(
            "---\ntitle: T\n---\n\n",
            "![Chart of spend](img/R&D.png)\n\n",
            "Download [the data](<Q&A data.csv>).\n\n",
            "![gone](img/X&Y.png)\n",
        ),
        &dir.0,
        None,
        None,
    );
    let assets = msgs(&validate_local_assets(&doc.blocks, &dir.0));
    assert_eq!(assets.len(), 1, "only the truly missing image: {assets:?}");
    assert!(assets[0].contains("`img/X&Y.png`"), "{assets:?}");
    let links = msgs(&validate_local_links(&doc.blocks, &dir.0));
    assert!(links.is_empty(), "the linked file exists: {links:?}");
}

/// A link resolves the way the browser resolves it: `%XX` in the path names the decoded
/// file, and a fragment matches an id either as written or percent-decoded (the HTML
/// spec's two tries). Both checks compared the encoded text, so a working
/// `[f](my%20file.txt)` and a working `[u](#%C3%BCber)` each failed the publish gate.
#[test]
fn the_link_and_anchor_checks_percent_decode_like_a_browser() {
    let dir = Tmp::new("pct-links");
    std::fs::write(dir.0.join("my file.txt"), "x").unwrap();
    let doc = render_document_scoped_with_site(
        concat!(
            "---\ntitle: T\n---\n\n",
            "## Über {#über}\n\n",
            "[f](my%20file.txt) [g](gone%20file.txt) [u](#%C3%BCber) [raw](#über) \
             [x](#%C3%BCbex)\n",
        ),
        &dir.0,
        None,
        None,
    );
    let links = msgs(&validate_local_links(&doc.blocks, &dir.0));
    assert_eq!(links.len(), 1, "only the missing file: {links:?}");
    assert!(links[0].contains("`gone file.txt`"), "{links:?}");
    let anchors = msgs(&validate_internal_anchors(&doc.blocks));
    assert_eq!(anchors.len(), 1, "only the missing anchor: {anchors:?}");
    assert!(anchors[0].contains("#%C3%BCbex"), "{anchors:?}");
}

/// The same rule on the link and alt-text checks, which shared the scan.
#[test]
fn the_link_and_alt_checks_read_tags_not_a_substring_scan() {
    let dir = Tmp::new("links-walker");
    let doc = render_document_scoped_with_site(
        concat!(
            "---\ntitle: T\n---\n\n",
            "<script>\nvar h = '<a href=\"' + e + '.md\">x</a>';\n</script>\n\n",
            "<a href='gone.tmd'>single-quoted</a>\n",
        ),
        &dir.0,
        None,
        None,
    );
    let m = msgs(&validate_local_links(&doc.blocks, &dir.0));
    assert_eq!(m.len(), 1, "the real broken link only: {m:?}");
    assert!(m[0].contains("gone.tmd"), "{m:?}");

    let a11y = msgs(&validate_a11y(&doc.blocks));
    assert!(
        !a11y.iter().any(|s| s.contains("alt")),
        "there is no image on this page — a script's text is not one: {a11y:?}"
    );
}

/// The anchor set is every `id` ATTRIBUTE on the page, in any quoting form — and nothing
/// that merely looks like one in the page's text.
///
/// Both directions were wrong, and both are visible to an author. A link to a real
/// `<div id='target'>` was reported as broken, because the scan knew only double quotes;
/// and a link to an id that exists only inside a code sample resolved, because the scan had
/// no notion of tag-versus-text.
#[test]
fn the_anchor_set_is_id_attributes_and_not_text_that_resembles_one() {
    let doc = render_document(
        "<div id='target'>real, single-quoted</div>\n\n\
         A sample: `<div id=\"in-text-only\">`\n\n\
         [good](#target) [bad](#in-text-only)\n",
    );
    let m = msgs(&validate_internal_anchors(&doc.blocks));
    assert_eq!(m.len(), 1, "exactly the one broken jump: {m:?}");
    assert!(m[0].contains("#in-text-only"), "{m:?}");
    assert!(
        !m.iter().any(|s| s.contains("#target")),
        "an author's single-quoted id is a real anchor: {m:?}"
    );
}
