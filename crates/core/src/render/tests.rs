//! Unit + corpus-invariant tests for the render module (split out of mod.rs
//! to keep mod.rs focused; `use super::*` reaches the render internals).

use super::*;

#[test]
fn heading_and_paragraph_become_blocks() {
    let doc = render_document("# Title\n\nHello *world*.\n");
    assert_eq!(doc.blocks.len(), 2);
    assert!(doc.blocks[0].html.starts_with("<h1 "));
    assert!(doc.blocks[0].html.contains("data-sourcepos=\"1:1-"));
    assert!(doc.blocks[0].html.contains("data-block-id=\"b-"));
    assert!(doc.blocks[1].html.contains("<em>world</em>"));
}

#[test]
fn ids_are_stable_across_runs_and_unique_for_duplicates() {
    let doc = render_document("Para.\n\nPara.\n");
    assert_eq!(doc.blocks.len(), 2);
    assert_ne!(
        doc.blocks[0].id, doc.blocks[1].id,
        "duplicate content must get a tiebreak"
    );
    let again = render_document("Para.\n\nPara.\n");
    assert_eq!(
        doc.blocks[0].id, again.blocks[0].id,
        "ids must be stable across runs"
    );
}

#[test]
fn front_matter_title_extracted_and_rendered_as_title_block() {
    let doc = render_document("---\ntitle: \"My Post\"\nfoo: bar\n---\n\nBody.\n");
    assert_eq!(doc.title.as_deref(), Some("My Post"));
    // A generated title block is prepended, then the body paragraph.
    assert_eq!(doc.blocks.len(), 2);
    assert_eq!(doc.blocks[0].id, "qmd-title-block");
    assert!(
        doc.blocks[0]
            .html
            .contains("<h1 class=\"title\">My Post</h1>"),
        "got: {}",
        doc.blocks[0].html
    );
    assert!(doc.blocks[1].html.contains("Body."));
}

#[test]
fn title_block_style_none_keeps_title_metadata_but_drops_visible_block() {
    let doc = render_document("---\ntitle: \"Blog\"\ntitle-block-style: none\n---\n\nIntro.\n");
    // Metadata title is preserved (drives `<title>`, OpenGraph, nav)...
    assert_eq!(doc.title.as_deref(), Some("Blog"));
    // ...but no visible title-block header is emitted, only the body.
    assert!(
        !doc.blocks.iter().any(|b| b.id == "qmd-title-block"),
        "expected no title block, got ids: {:?}",
        doc.blocks.iter().map(|b| &b.id).collect::<Vec<_>>()
    );
    assert_eq!(doc.blocks.len(), 1);
    assert!(doc.blocks[0].html.contains("Intro."));
}

#[test]
fn title_block_includes_subtitle_date_and_description() {
    let doc = render_document(
        "---\ntitle: T\nsubtitle: S\ndate: 2026-05-15\nauthor: A\ndescription: D\n---\n\nx\n",
    );
    let h = &doc.blocks[0].html;
    assert!(h.contains("class=\"qmd-title-block\""));
    assert!(h.contains("<p class=\"subtitle\">S</p>"), "got: {h}");
    assert!(h.contains("<p class=\"description\">D</p>"), "got: {h}");
    assert!(
        h.contains("<span>A</span>") && h.contains("<span>2026-05-15</span>"),
        "got: {h}"
    );
}

#[test]
fn reveal_deck_has_no_html_title_block() {
    // The deck builds its own title slide; no `qmd-title-block` block.
    let doc = render_document("---\ntitle: T\nformat: revealjs\n---\n\n## Slide\n");
    assert!(!doc.blocks.iter().any(|b| b.id == "qmd-title-block"));
}

#[test]
fn html_is_escaped_in_text() {
    let doc = render_document("a < b & c\n");
    assert!(doc.blocks[0].html.contains("a &lt; b &amp; c"));
}

#[test]
fn qmd_code_cell_language_detected() {
    let doc = render_document("```{python}\nprint(1)\n```\n");
    assert!(doc.blocks[0].html.contains("<pre "));
    assert!(doc.blocks[0].html.contains("class=\"language-python\""));
}

#[test]
fn table_uses_thead_th_and_tbody_td() {
    let doc = render_document("| A | B |\n|---|--:|\n| 1 | 2 |\n");
    let h = &doc.blocks[0].html;
    assert!(h.starts_with("<table "), "got: {h}");
    assert!(h.contains("<thead><tr><th>A</th><th"), "got: {h}");
    assert!(h.contains("<tbody><tr><td>1</td>"), "got: {h}");
    assert!(
        h.contains("text-align: right"),
        "alignment from |--:| missing: {h}"
    );
}

#[test]
fn callout_wraps_content_using_leading_heading_as_title() {
    let doc = render_document("::: {.callout-note}\n## My Note\n\nBody text.\n:::\n");
    assert_eq!(doc.blocks.len(), 1, "the callout is one container block");
    let h = &doc.blocks[0].html;
    assert!(h.contains("class=\"callout callout-note\""), "got: {h}");
    assert!(
        h.contains("<div class=\"callout-title\">My Note</div>"),
        "got: {h}"
    );
    assert!(!doc.body_html().contains(":::"));
    // inner content keeps its own sourcepos so click-to-source still works.
    assert!(
        h.contains("<p data-block-id"),
        "inner block lost its id: {h}"
    );
    assert!(h.contains("Body text."));
}

#[test]
fn callout_uses_explicit_title_and_default_title() {
    let titled = render_document("::: {.callout-tip title=\"Pro tip\"}\nDo this.\n:::\n");
    assert!(titled.blocks[0].html.contains("callout-tip"));
    assert!(
        titled.blocks[0].html.contains(">Pro tip</div>"),
        "got: {}",
        titled.blocks[0].html
    );

    let bare = render_document("::: {.callout-warning}\nBe careful.\n:::\n");
    assert!(
        bare.blocks[0].html.contains(">Warning</div>"),
        "got: {}",
        bare.blocks[0].html
    );
}

#[test]
fn layout_ncol_div_becomes_grid() {
    let doc = render_document("::: {layout-ncol=2}\n![](a.png)\n\n![](b.png)\n:::\n");
    assert_eq!(doc.blocks.len(), 1);
    let h = &doc.blocks[0].html;
    assert!(h.contains("qmd-layout"), "got: {h}");
    assert!(h.contains("repeat(2,"), "got: {h}");
}

#[test]
fn unterminated_div_renders_content_without_a_container() {
    // A `:::` open with no matching close forms no span: the fence line is
    // blanked and the content renders as ordinary blocks (no crash, no
    // stray `:::`, no wrapper div).
    let doc = render_document("::: {.callout-note}\n\nOrphan body.\n");
    let body = doc.body_html();
    assert!(body.contains("Orphan body."), "got: {body}");
    assert!(
        !body.contains(":::"),
        "fence marker must be stripped: {body}"
    );
    assert!(
        !body.contains("callout"),
        "no container without a close: {body}"
    );
}

#[test]
fn stray_closing_fence_is_ignored() {
    // A `:::` close with nothing open is dropped, not turned into an empty div.
    let doc = render_document("A paragraph.\n\n:::\n");
    let body = doc.body_html();
    assert!(body.contains("A paragraph."), "got: {body}");
    assert!(!body.contains(":::"), "got: {body}");
    assert!(!body.contains("qmd-div"), "got: {body}");
}

#[test]
fn empty_div_emits_no_block() {
    // An open immediately followed by a close contains no blocks, so it
    // produces no container at all (the documented "empty fenced div emits
    // no block" behaviour the listing injector relies on).
    let doc = render_document("::: {.callout-note}\n:::\n");
    assert!(doc.blocks.is_empty(), "got {} blocks", doc.blocks.len());
}

#[test]
fn nested_divs_group_inside_out() {
    let doc = render_document("::: {.outer}\n\n::: {.inner}\n\nDeep text.\n\n:::\n\n:::\n");
    assert_eq!(doc.blocks.len(), 1, "one outer container");
    let h = &doc.blocks[0].html;
    let outer = h.find("outer").expect("outer class");
    let inner = h.find("inner").expect("inner class");
    let text = h.find("Deep text.").expect("inner text");
    assert!(
        outer < inner && inner < text,
        "outer wraps inner wraps text: {h}"
    );
}

#[test]
fn mermaid_block_emits_pre_mermaid_without_code() {
    // Both the executable cell form and a plain fence become a mermaid pre.
    for src in [
        "```{mermaid}\nflowchart LR\n  A --> B\n```\n",
        "```mermaid\nflowchart LR\n  A --> B\n```\n",
    ] {
        let doc = render_document(src);
        let h = &doc.blocks[0].html;
        assert!(h.contains("<pre data-block-id"), "got: {h}");
        assert!(h.contains("class=\"mermaid\""), "got: {h}");
        assert!(
            !h.contains("<code"),
            "mermaid must not wrap a <code> element: {h}"
        );
        assert!(h.contains("flowchart LR"), "got: {h}");
        assert!(
            h.contains("A --&gt; B"),
            "diagram source should be escaped: {h}"
        );
    }
}

#[test]
fn labelled_mermaid_becomes_numbered_referenceable_figure() {
    let doc = render_document(
        "See @fig-flow.\n\n```{mermaid}\n%%| label: fig-flow\n%%| fig-cap: \"The pipeline\"\nflowchart LR\n  A --> B\n```\n",
    );
    let body = doc.body_html();
    // the diagram is wrapped in a numbered figure with the #fig- anchor
    assert!(
        body.contains("id=\"fig-flow\""),
        "figure anchor missing: {body}"
    );
    assert!(
        body.contains("class=\"qmd-figure"),
        "mermaid not wrapped in a figure: {body}"
    );
    assert!(
        body.contains("<pre class=\"mermaid\">"),
        "diagram pre missing: {body}"
    );
    assert!(
        body.contains("<figcaption>Figure&nbsp;1: The pipeline</figcaption>"),
        "got: {body}"
    );
    // the `%%|` option lines are stripped from the diagram source
    assert!(!body.contains("%%|"), "mermaid cell options leaked: {body}");
    // and `@fig-flow` resolves to the numbered link
    assert!(
        body.contains("<a href=\"#fig-flow\" class=\"qmd-xref\">Figure&nbsp;1</a>"),
        "cross-reference did not resolve: {body}"
    );
}

#[test]
fn unlabelled_mermaid_stays_a_bare_diagram() {
    // No label/fig-cap -> not a figure, not numbered (stays a plain pre).
    let doc = render_document("```{mermaid}\nflowchart LR\n  A --> B\n```\n");
    let h = &doc.blocks[0].html;
    assert!(h.contains("<pre data-block-id"), "got: {h}");
    assert!(
        !h.contains("qmd-figure"),
        "unlabelled mermaid should not be a figure: {h}"
    );
    assert!(
        !h.contains("figcaption"),
        "unlabelled mermaid should have no caption: {h}"
    );
}

#[test]
fn cell_option_lines_are_dropped() {
    let doc = render_document("```{python}\n#| warning: false\nprint(1)\n```\n");
    let h = &doc.blocks[0].html;
    // (code is highlighted, so its text is split across scope spans)
    assert!(strip_tags(h).contains("print(1)"));
    assert!(!h.contains("#|"), "option lines should be stripped: {h}");

    // OJS cells become live placeholders; their `//|` options are stripped
    // before the source is base64-encoded into an ojs-module-contents script.
    let ojs = render_document("```{ojs}\n//| echo: false\nx = 1\n```\n");
    let oh = &ojs.blocks[0].html;
    assert!(
        oh.contains("class=\"cell ojs-cell\""),
        "ojs cell should be a live placeholder: {oh}"
    );
    assert!(
        oh.contains("ojs-module-contents"),
        "ojs cell missing module-contents: {oh}"
    );
    assert!(
        !oh.contains("//| echo"),
        "option lines should be stripped: {oh}"
    );
}

#[test]
fn echo_and_include_false_hide_source_but_keep_the_cell() {
    // echo:false hides the source; the cell stays (so the executor still runs
    // it) and its output (added by the executor) is unaffected.
    let echo = render_document("```{python}\n#| echo: false\nprint(1)\n```\n");
    let b = &echo.blocks[0];
    assert!(
        b.cell.is_some(),
        "cell metadata must survive so the executor runs it"
    );
    assert!(
        b.cell.as_ref().unwrap().include,
        "echo:false keeps include true"
    );
    assert!(
        !b.html.contains("print(1)"),
        "echo:false must hide the source: {}",
        b.html
    );
    assert!(
        b.html.contains("qmd-cell-hidden"),
        "expected a hidden marker: {}",
        b.html
    );

    // include:false hides the source too and flags the cell so the executor
    // suppresses its output.
    let inc = render_document("```{python}\n#| include: false\nprint(1)\n```\n");
    let b = &inc.blocks[0];
    assert!(b.cell.is_some());
    assert!(
        !b.cell.as_ref().unwrap().include,
        "include:false must be recorded on the cell"
    );
    assert!(
        !b.html.contains("print(1)"),
        "include:false must hide the source: {}",
        b.html
    );

    // A plain cell still shows its source.
    let plain = render_document("```{python}\nprint(1)\n```\n");
    assert!(
        strip_tags(&plain.blocks[0].html).contains("print(1)"),
        "default cell shows source"
    );
}

#[test]
fn execute_block_sets_document_cell_defaults() {
    // `execute: echo: false` hides every cell's source by default.
    let doc = render_document("---\nexecute:\n  echo: false\n---\n\n```{python}\nprint(1)\n```\n");
    let cell = doc
        .blocks
        .iter()
        .find(|b| b.cell.is_some())
        .expect("a code cell");
    assert!(
        !cell.html.contains("print(1)"),
        "execute.echo:false should hide source by default: {}",
        cell.html
    );

    // A per-cell `#| echo: true` overrides the document default.
    let doc2 = render_document(
        "---\nexecute:\n  echo: false\n---\n\n```{python}\n#| echo: true\nprint(1)\n```\n",
    );
    let cell2 = doc2
        .blocks
        .iter()
        .find(|b| b.cell.is_some())
        .expect("a code cell");
    assert!(
        strip_tags(&cell2.html).contains("print(1)"),
        "per-cell echo:true must override the execute default: {}",
        cell2.html
    );
}

#[test]
fn explicit_heading_id_is_applied_and_stripped() {
    let doc = render_document("## Methods {#sec-methods}\n\nText.\n");
    let h = &doc.blocks[0].html;
    assert!(
        h.contains("id=\"sec-methods\""),
        "explicit id not applied: {h}"
    );
    assert!(
        !h.contains('{'),
        "the {{#id}} attribute leaked into the heading: {h}"
    );
    assert!(
        h.contains(">Methods</h2>"),
        "heading text wrong after strip: {h}"
    );

    // A heading without an attribute still gets a slug id.
    let plain = render_document("## My Heading\n");
    assert!(
        plain.blocks[0].html.contains("id=\"my-heading\""),
        "slug id missing: {}",
        plain.blocks[0].html
    );
}

#[test]
fn sec_label_makes_at_sec_resolve_to_a_number() {
    let doc = render_document("## Methods {#sec-methods}\n\nSee @sec-methods.\n");
    let body = doc.body_html();
    assert!(
        body.contains("id=\"sec-methods\""),
        "heading id missing: {body}"
    );
    assert!(
        body.contains("class=\"qmd-xref\">Section&nbsp;1</a>"),
        "@sec-methods did not resolve to a numbered Section link: {body}"
    );
}

#[test]
fn table_caption_is_numbered_folded_and_referenceable() {
    let doc = render_document(
        "| a | b |\n|---|---|\n| 1 | 2 |\n\n: My caption {#tbl-data}\n\nSee @tbl-data.\n",
    );
    let body = doc.body_html();
    assert!(
        body.contains("<table id=\"tbl-data\""),
        "table did not get the explicit id: {body}"
    );
    assert!(
        body.contains("<caption>Table&nbsp;1: My caption</caption>"),
        "caption not folded/numbered into the table: {body}"
    );
    assert!(
        !body.contains("{#tbl-data}") && !body.contains(">: My caption"),
        "the caption paragraph leaked instead of folding into the table: {body}"
    );
    assert!(
        body.contains("class=\"qmd-xref\">Table&nbsp;1</a>"),
        "@tbl-data did not resolve to a number: {body}"
    );
}

#[test]
fn ojs_cell_emits_live_placeholder_and_classifies_declarations() {
    // A named declaration is hidden (nodetype="declaration"); a viewof and a
    // bare expression stay visible.
    let decl = render_document("```{ojs}\nsignalX = [1, 2, 3]\n```\n");
    assert!(decl.blocks[0].html.contains("class=\"cell ojs-cell\""));
    assert!(
        decl.blocks[0].html.contains("nodetype=\"declaration\""),
        "named decl should be hidden"
    );
    assert!(
        decl.blocks[0]
            .html
            .contains("<script type=\"ojs-module-contents\">")
    );

    let view = render_document("```{ojs}\nviewof n = Inputs.range([0, 9])\n```\n");
    assert!(
        !view.blocks[0].html.contains("nodetype=\"declaration\""),
        "viewof must stay visible"
    );

    let expr = render_document("```{ojs}\nPlot.lineY([1, 2, 3]).plot()\n```\n");
    assert!(
        !expr.blocks[0].html.contains("nodetype=\"declaration\""),
        "expression must stay visible"
    );
}

#[test]
fn ojs_declaration_classifier() {
    assert!(ojs_is_declaration("foo = 1 + 2"));
    assert!(ojs_is_declaration("// a comment\nbar = {\n  return 3;\n}"));
    assert!(ojs_is_declaration("function makeScene(a, b) { return a; }"));
    assert!(ojs_is_declaration(
        "async function makeScene3D(build, invalidation) { return 0; }"
    ));
    assert!(ojs_is_declaration("class Particle { constructor() {} }"));
    assert!(!ojs_is_declaration("viewof x = Inputs.button()"));
    assert!(!ojs_is_declaration("import {a} from \"./x.js\""));
    assert!(!ojs_is_declaration("md`hello ${name}`"));
    assert!(!ojs_is_declaration("a == b"));
    assert!(!ojs_is_declaration("x => x + 1"));
    assert!(!ojs_is_declaration("{ const y = 1; return y; }"));
}

#[test]
fn dollar_math_is_rendered_by_katex() {
    let doc = render_document("The value $x^2$ is positive.\n");
    let h = &doc.blocks[0].html;
    assert!(h.contains("katex"), "expected katex markup, got: {h}");
    assert!(
        !h.contains("$x^2$"),
        "raw dollar math should be consumed: {h}"
    );
}

#[test]
fn display_math_block_renders() {
    let doc = render_document("$$\n\\sum_{i=1}^n x_i\n$$\n");
    assert!(
        doc.body_html().contains("katex-display"),
        "got: {}",
        doc.body_html()
    );
}

#[test]
fn bare_latex_environment_renders_as_display_math() {
    let doc = render_document("\\begin{align*}\na &= b \\\\\nc &= d\n\\end{align*}\n");
    assert_eq!(doc.blocks.len(), 1);
    let h = &doc.blocks[0].html;
    // rendered as a display-math block (the raw TeX only survives inside
    // KaTeX's <annotation>, which is expected).
    assert!(h.contains("qmd-math-block"), "got: {h}");
    assert!(
        h.contains("katex-display"),
        "expected display math, got: {h}"
    );
}

#[test]
fn display_equation_label_becomes_numbered_id() {
    // A display equation labelled `$$ ... $$ {#eq-foo}` consumes the attribute,
    // emits a matching `id`, and gets a number.
    let src = "$$\nX = 1\n$$ {#eq-foo}\n";
    let html = render_document(src).body_html();
    assert!(
        html.contains("id=\"eq-foo\""),
        "equation did not get its #eq-foo id"
    );
    assert!(
        !html.contains("{#eq-foo}"),
        "the {{#eq-foo}} attribute leaked as text"
    );
    assert!(html.contains("qmd-eqn-number"), "equation was not numbered");
}

#[test]
fn html_block_attrs_injected_into_leading_tag() {
    let doc = render_document("<div class=\"demo\">\nhi\n</div>\n");
    let h = &doc.blocks[0].html;
    assert!(h.contains("<div class=\"demo\" data-block-id="), "got: {h}");
    // the wrapper-div double-emit bug must not reappear
    assert!(
        !h.contains("<div data-block-id"),
        "should inject, not wrap: {h}"
    );
}

#[test]
fn raw_html_block_is_passed_through() {
    // ```{=html}``` is Pandoc/Quarto raw-passthrough: its body is emitted
    // verbatim, not escaped as a code listing.
    let src =
        "```{=html}\n<audio controls><source src=\"x.wav\" type=\"audio/wav\"></audio>\n```\n";
    let html = render_document(src).body_html();
    assert!(
        html.contains("<audio controls"),
        "raw <audio> HTML was not passed through"
    );
    assert!(!html.contains("&lt;audio"), "raw HTML was escaped");
    assert!(
        !html.contains("language-=html"),
        "raw block was treated as a code cell"
    );
}

// --- edge cases / robustness ---

#[test]
fn empty_and_whitespace_inputs_do_not_panic() {
    assert!(render_document("").blocks.is_empty());
    assert!(render_document("   \n\n\t\n").blocks.is_empty());
}

#[test]
fn front_matter_only_yields_just_the_title_block() {
    let doc = render_document("---\ntitle: Only Meta\n---\n");
    assert_eq!(doc.title.as_deref(), Some("Only Meta"));
    // Only the generated title block (no body content).
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(doc.blocks[0].id, "qmd-title-block");
}

#[test]
fn front_matter_without_title_yields_no_blocks() {
    // No title -> no title block, and no body -> empty.
    assert!(render_document("---\nfoo: bar\n---\n").blocks.is_empty());
}

#[test]
fn reveal_deck_injects_includes_and_theme() {
    let src = "---\n\
            format: revealjs\n\
            include-in-header:\n  text: |\n    <meta name=\"deck\" content=\"1\">\n\
            include-after-body:\n  text: |\n    <script>window.__deck=1</script>\n\
            ---\n\n## Slide\n";
    let page = render_html_page(src, "deck");
    assert!(
        page.contains("<div class=\"reveal\">"),
        "should render as a reveal deck"
    );
    let head = &page[..page.find("</head>").expect("has </head>")];
    assert!(
        head.contains("<meta name=\"deck\" content=\"1\">"),
        "include-in-header not injected into the deck <head>"
    );
    assert!(
        page.contains("<script>window.__deck=1</script>"),
        "include-after-body not injected into the deck"
    );
}

#[test]
fn front_matter_lang_sets_html_lang_attr() {
    // `lang:` drives `<html lang>` (for screen readers + SEO); absent falls back to en.
    assert!(
        render_html_page("---\ntitle: Bonjour\nlang: fr\n---\n\nSalut.\n", "f")
            .contains("<html lang=\"fr\">"),
        "front-matter lang not applied to <html>"
    );
    assert!(
        render_html_page("---\ntitle: Hi\n---\n\nHi.\n", "f").contains("<html lang=\"en\">"),
        "missing lang should default to en"
    );
}

#[test]
fn front_matter_include_text_injected_at_head_and_body() {
    let src = "---\n\
            title: T\n\
            include-in-header:\n  text: |\n    <meta name=\"x\" content=\"y\">\n\
            include-before-body:\n  text: |\n    <div id=\"top-banner\"></div>\n\
            include-after-body:\n  text: |\n    <script>window.__after=1</script>\n\
            ---\n\nBody.\n";
    let page = render_html_page(src, "fallback");
    let head = &page[..page.find("</head>").expect("has </head>")];
    assert!(
        head.contains("<meta name=\"x\" content=\"y\">"),
        "include-in-header not injected into <head>"
    );
    // before-body lands ahead of the rendered body paragraph.
    let banner = page.find("top-banner").expect("before-body injected");
    let body_para = page.find("Body.").expect("body present");
    assert!(
        banner < body_para,
        "include-before-body must precede the body"
    );
    assert!(
        page.contains("<script>window.__after=1</script>"),
        "include-after-body not injected"
    );
}

#[test]
fn nested_lists_render_with_nesting() {
    let doc = render_document("- a\n    - b\n    - c\n- d\n");
    let h = &doc.blocks[0].html;
    assert!(h.starts_with("<ul "), "got: {h}");
    assert!(
        h.contains("<li>a<ul><li>b</li><li>c</li></ul></li>"),
        "got: {h}"
    );
}

#[test]
fn ordered_list_start_attribute_preserved() {
    let doc = render_document("3. third\n4. fourth\n");
    assert!(doc.blocks[0].html.starts_with("<ol "));
    assert!(
        doc.blocks[0].html.contains("start=\"3\""),
        "got: {}",
        doc.blocks[0].html
    );
}

#[test]
fn links_images_and_blockquotes_render() {
    let link = render_document("[text](https://example.com \"t\")\n");
    assert!(
        link.blocks[0]
            .html
            .contains("<a href=\"https://example.com\" title=\"t\">text</a>")
    );

    let img = render_document("![alt text](/img.png)\n");
    assert!(
        img.blocks[0]
            .html
            .contains("<img src=\"/img.png\" alt=\"alt text\" />")
    );

    let quote = render_document("> quoted line\n");
    assert!(quote.blocks[0].html.starts_with("<blockquote "));
    assert!(quote.blocks[0].html.contains("quoted line"));
}

#[test]
fn attribute_values_are_escaped() {
    let doc = render_document("[x](https://e.com?a=1&b=\"2\")\n");
    let h = &doc.blocks[0].html;
    assert!(
        h.contains("&amp;"),
        "ampersand should be escaped in href: {h}"
    );
    assert!(h.contains("&quot;"), "quote should be escaped in href: {h}");
}

#[test]
fn unicode_text_is_preserved() {
    let doc = render_document("naïve café — ψ ∈ ℂ, Σ over 𝒩\n");
    assert!(doc.blocks[0].html.contains("naïve café — ψ ∈ ℂ, Σ over 𝒩"));
}

#[test]
fn special_chars_in_inline_code_are_escaped_not_interpreted() {
    let doc = render_document("use `a < b && c` here\n");
    let h = &doc.blocks[0].html;
    assert!(h.contains("<code>a &lt; b &amp;&amp; c</code>"), "got: {h}");
}

// --- reveal.js / slides ---

#[test]
fn reveal_format_detected_from_front_matter() {
    // Nested block form (the corpus shape): `format:` with a *-revealjs subkey.
    let deck = render_document(
        "---\nformat:\n  liquid-glass-revealjs:\n    slide-number: true\n---\n\n## A\n",
    );
    assert_eq!(deck.format, DocFormat::Reveal);
    // Inline form.
    let inline = render_document("---\nformat: revealjs\n---\n\n## A\n");
    assert_eq!(inline.format, DocFormat::Reveal);
    // A normal post is Html, even if a nested non-format key mentions revealjs.
    let post = render_document("---\ntitle: Post\nformat: html\n---\n\nHi.\n");
    assert_eq!(post.format, DocFormat::Html);
}

#[test]
fn deck_splits_into_title_slide_and_one_section_per_heading() {
    let doc = render_document(
        "---\ntitle: Deck\nsubtitle: A subtitle\nformat: revealjs\n---\n\n## First\n\nHello.\n\n## Second\n\nWorld.\n",
    );
    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    // Title slide from front matter.
    assert!(slides.contains("id=\"title-slide\""), "got: {slides}");
    assert!(
        slides.contains("<h1 class=\"title\">Deck</h1>"),
        "got: {slides}"
    );
    assert!(
        slides.contains("<p class=\"subtitle\">A subtitle</p>"),
        "got: {slides}"
    );
    // One <section> per h2, id slugged from the heading text.
    assert!(
        slides.contains("<section id=\"first\" class=\"slide level2\">"),
        "got: {slides}"
    );
    assert!(
        slides.contains("<section id=\"second\" class=\"slide level2\">"),
        "got: {slides}"
    );
    // Heading keeps its block id inside the section (block-swap/click-to-source).
    assert!(
        slides.contains("<h2 data-block-id="),
        "heading lost its block id: {slides}"
    );
    // title + two content slides, no nesting.
    assert_eq!(slides.matches("<section").count(), 3, "got: {slides}");
}

#[test]
fn thematic_break_starts_a_new_slide_and_is_not_emitted() {
    let doc = render_document("---\nformat: revealjs\n---\n\nOne.\n\n---\n\nTwo.\n");
    let slides = slides_html(None, None, &doc.blocks);
    assert!(
        !slides.contains("<hr"),
        "the --- delimiter must not render: {slides}"
    );
    assert_eq!(slides.matches("<section").count(), 2, "got: {slides}");
}

#[test]
fn h1_wraps_following_h2s_in_a_vertical_stack() {
    let doc =
        render_document("---\nformat: revealjs\n---\n\n# Part One\n\nIntro.\n\n## A\n\n## B\n");
    let slides = slides_html(None, None, &doc.blocks);
    // Outer wrapper section, then the h1 lead slide, then the two h2 children.
    assert!(
        slides.contains("<section>\n<section id=\"part-one\" class=\"slide level1\">"),
        "got: {slides}"
    );
    assert!(
        slides.contains("<section id=\"a\" class=\"slide level2\">"),
        "got: {slides}"
    );
    assert!(
        slides.contains("<section id=\"b\" class=\"slide level2\">"),
        "got: {slides}"
    );
    // 1 wrapper + lead + 2 children = 4 sections.
    assert_eq!(slides.matches("<section").count(), 4, "got: {slides}");
}

#[test]
fn reveal_page_carries_revealjs_scaffolding() {
    let page = render_html_page(
        "---\ntitle: D\nformat: revealjs\n---\n\n## Slide\n",
        "fallback",
    );
    assert!(page.contains("class=\"reveal\""));
    assert!(page.contains("class=\"slides\""));
    // The deck engine is bundled (no CDN); it exposes a window.Reveal facade.
    assert!(page.contains("window.QmdDeck"));
    assert!(page.contains("Reveal.initialize("));
    assert!(
        !page.contains("jsdelivr") || !page.contains("reveal.js@"),
        "the deck must not load reveal.js from a CDN"
    );
}

#[test]
fn code_line_numbers_wraps_lines_for_stepping() {
    let page = render_html_page(
        "---\nformat: revealjs\n---\n\n## S\n\n```{.python code-line-numbers=\"1|2\"}\na = 1\nb = 2\n```\n",
        "fallback",
    );
    assert!(
        page.contains("data-code-lines=\"1|2\""),
        "missing line spec"
    );
    // two source lines -> two line spans (the trailing-newline line is dropped).
    assert_eq!(
        page.matches("class=\"qhl-ln\"").count(),
        2,
        "expected one line span per source line"
    );
    // a code block without the attribute is left unwrapped.
    let plain = render_html_page(
        "---\nformat: revealjs\n---\n\n## S\n\n```python\na = 1\n```\n",
        "fb",
    );
    // (check the attribute, not bare "qhl-ln" — the inlined CSS mentions `.qhl-ln`.)
    assert!(
        !plain.contains("class=\"qhl-ln\""),
        "plain code should not be line-wrapped"
    );
}

#[test]
fn heading_background_attr_moves_to_section() {
    let page = render_html_page(
        "---\nformat: revealjs\n---\n\n## Title {background-color=\"#123456\"}\n\nbody\n",
        "fb",
    );
    // the background hoists onto the <section> and the `{...}` is stripped.
    assert!(
        page.contains("data-background-color=\"#123456\""),
        "bg attr missing"
    );
    assert!(
        !page.contains("{background-color"),
        "the {{...}} must be stripped"
    );
    assert!(
        !page.contains("<h2 data-background"),
        "bg must move off the heading"
    );
}

#[test]
fn heading_auto_animate_marks_the_section() {
    let page = render_html_page(
        "---\nformat: revealjs\n---\n\n## Title {auto-animate=true}\n\nbody\n",
        "fb",
    );
    assert!(
        page.contains("data-auto-animate"),
        "auto-animate marker missing"
    );
    // it hoists onto the <section>, not the heading, and the `{...}` is stripped.
    assert!(
        !page.contains("<h2 data-auto-animate"),
        "must move off the heading"
    );
    assert!(
        !page.contains("{auto-animate"),
        "the {{...}} must be stripped"
    );
}

#[test]
fn magic_move_div_wraps_code_lines() {
    let page = render_html_page(
        "---\nformat: revealjs\n---\n\n## S\n\n::: {.magic-move}\n```js\na = 1\n```\n\n```js\na = 2\nb = 3\n```\n:::\n",
        "fb",
    );
    assert!(
        page.contains("class=\"magic-move\""),
        "magic-move div missing"
    );
    // both blocks' lines are wrapped so the engine can match/glide them (1 + 2 = 3).
    assert_eq!(
        page.matches("class=\"qhl-ln\"").count(),
        3,
        "magic-move code blocks should be line-wrapped"
    );
}

#[test]
fn deck_dedups_repeated_slide_ids() {
    // Repeated headings (common with auto-animate's shared titles) must get
    // distinct section ids, else `#/hash` + getElementById only find the first.
    let page = render_html_page(
        "---\nformat: revealjs\n---\n\n## Step\n\na\n\n## Step\n\nb\n",
        "fb",
    );
    assert!(page.contains("id=\"step\""), "first slide id missing");
    assert!(
        page.contains("id=\"step-1\""),
        "duplicate slide id not deduped"
    );
}

// --- books: heading anchors, figures, toc ---

#[test]
fn headings_get_deduped_anchor_ids() {
    let doc = render_document("# Intro\n\nbody\n\n# Intro\n");
    assert!(
        doc.blocks[0].html.starts_with("<h1 id=\"intro\""),
        "got: {}",
        doc.blocks[0].html
    );
    // a repeated heading slug is deduped with a -N suffix.
    let last = doc.blocks.last().unwrap();
    assert!(last.html.contains("id=\"intro-1\""), "got: {}", last.html);
}

#[test]
fn reveal_headings_have_no_id_to_avoid_duplicating_section_ids() {
    // In a deck the slug lives on the wrapping <section>, so the heading must
    // not also carry it (that would be a duplicate id in the DOM).
    let doc = render_document("---\nformat: revealjs\n---\n\n## A Slide\n");
    let h = doc
        .blocks
        .iter()
        .find(|b| b.html.starts_with("<h2"))
        .unwrap();
    assert!(
        !h.html.contains(" id=\""),
        "reveal heading should not carry an id: {}",
        h.html
    );
}

#[test]
fn standalone_image_becomes_a_numbered_figure() {
    let doc =
        render_document("![Scree plot](scree.png){#fig-scree width=50% fig-align=\"center\"}\n");
    let h = &doc.blocks[0].html;
    assert!(h.starts_with("<figure"), "got: {h}");
    assert!(h.contains("id=\"fig-scree\""), "got: {h}");
    assert!(
        h.contains("class=\"qmd-figure qmd-figure-center\""),
        "got: {h}"
    );
    assert!(h.contains("<img src=\"scree.png\""), "got: {h}");
    assert!(h.contains("style=\"width:50%\""), "got: {h}");
    assert!(
        h.contains("<figcaption>Figure&nbsp;1: Scree plot</figcaption>"),
        "got: {h}"
    );
    assert!(!h.contains("{#fig-"), "the attribute block leaked: {h}");
    // the figure still carries the block model attributes.
    assert!(
        h.contains("data-block-id=") && h.contains("data-sourcepos="),
        "got: {h}"
    );
}

#[test]
fn inline_image_in_a_sentence_stays_inline() {
    let doc = render_document("See ![logo](l.png) for the mark.\n");
    let h = &doc.blocks[0].html;
    assert!(h.starts_with("<p "), "got: {h}");
    assert!(h.contains("<img src=\"l.png\""), "got: {h}");
    assert!(
        !h.contains("<figure"),
        "a non-standalone image must not become a figure: {h}"
    );
}

#[test]
fn toc_page_lists_headings_with_anchor_links() {
    let page = render_html_page(
        "---\ntitle: Doc\nformat:\n  html:\n    toc: true\n---\n\n# A\n\ntext\n\n## B\n",
        "fb",
    );
    assert!(page.contains("id=\"TOC\""), "missing TOC nav");
    assert!(
        page.contains("<body class=\"has-toc\">"),
        "missing toc layout class"
    );
    assert!(
        page.contains("<a href=\"#a\">A</a>"),
        "missing TOC entry for A: {page}"
    );
    assert!(
        page.contains("<a href=\"#b\">B</a>"),
        "missing nested TOC entry for B"
    );
}

#[test]
fn no_toc_when_not_requested() {
    let page = render_html_page("---\ntitle: Doc\n---\n\n# A\n", "fb");
    // (the `#TOC`/`has-toc` CSS rules are always present; assert on markup.)
    assert!(
        !page.contains("<nav id=\"TOC\""),
        "TOC nav should be absent without toc: true"
    );
    assert!(
        !page.contains("<body class=\"has-toc\">"),
        "toc layout should be off"
    );
}

#[test]
fn missing_bibliography_and_theme_files_warn() {
    // A named `.bib`/`.css` that can't be read is reported on the doc's
    // `warnings` (the core's non-fatal error channel), not silently dropped.
    let doc = render_document_with_includes(
        "---\ntitle: X\nbibliography: nope.bib\ntheme: gone.css\n---\n\nSee [@k].\n",
        std::path::Path::new("/qmd-fast-nonexistent-dir"),
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("bibliography file not found: nope.bib")),
        "got: {:?}",
        doc.warnings
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("theme file not found: gone.css")),
        "got: {:?}",
        doc.warnings
    );
    // A bare theme name (a possible Quarto built-in) must NOT warn.
    let ok = render_document_with_includes(
        "---\ntitle: X\ntheme: darkly\n---\n\ntext\n",
        std::path::Path::new("/qmd-fast-nonexistent-dir"),
    );
    assert!(
        ok.warnings.is_empty(),
        "bare theme warned: {:?}",
        ok.warnings
    );
}

#[test]
fn detect_toc_is_tristate_so_explicit_false_can_override_a_site_default() {
    // Unset, on, and off must be distinguishable: a plain bool can't tell an
    // explicit `toc: false` (which should beat the site default) from "unset".
    assert_eq!(detect_toc("title: X\n"), None);
    assert_eq!(detect_toc("title: X\ntoc: true\n"), Some(true));
    assert_eq!(detect_toc("title: X\ntoc: false\n"), Some(false));
    // `toc-depth:`/`toc-title:` are not the `toc:` key and must not match.
    assert_eq!(detect_toc("toc-depth: 2\ntoc-title: Contents\n"), None);
}

#[test]
fn theme_dark_default_drives_data_theme_resolver() {
    // Built-in dark no longer inlines a per-page override; it sets the default
    // mode, and the always-shipped dark CSS is selected at runtime by data-theme.
    let dark = render_document("---\ntheme: dark\n---\n\nx\n");
    assert!(
        dark.theme_css.is_empty(),
        "built-in dark should not inline override CSS"
    );
    assert_eq!(dark.theme_default, "dark");

    let page = render_html_page("---\ntheme: dark\n---\n\nx\n", "fb");
    assert!(
        page.contains("html[data-theme=\"dark\"]"),
        "scoped dark CSS not shipped"
    );
    assert!(page.contains("--qmd-bg: #16181d"), "dark vars missing");
    assert!(
        page.contains("var DEFAULT = \"dark\""),
        "resolver default should be dark"
    );

    // No theme -> auto (follow OS); light -> light. No inlined override either way.
    let plain = render_document("---\ntitle: x\n---\n\nx\n");
    assert!(plain.theme_css.is_empty());
    assert_eq!(plain.theme_default, "auto");
    assert_eq!(
        render_document("---\ntheme: light\n---\n\nx\n").theme_default,
        "light"
    );
}

#[test]
fn deck_theme_is_custom_and_head_gating() {
    // A plain deck (built-in theme) is managed: the deck theme head is emitted
    // and the deck follows OS/host/front-matter.
    let plain = render_document("---\nformat: revealjs\n---\n\n# A\n");
    assert!(!plain.theme_is_custom, "a plain deck has no custom theme");
    assert!(
        deck_theme_head(&plain.theme_default, plain.theme_is_custom).contains("qmdDeckApplyTheme"),
        "a built-in-theme deck should get the theme head"
    );
    // A user `include-in-header` is not a theme extension, so it must not flip
    // the deck out of built-in light/dark management.
    let with_header = render_document(
        "---\nformat: revealjs\ninclude-in-header:\n  text: \"<meta name=x>\"\n---\n\n# A\n",
    );
    assert!(!with_header.theme_is_custom);
    // An explicit `theme: dark` forces dark and is still managed.
    assert_eq!(
        render_document("---\nformat: revealjs\ntheme: dark\n---\n\n# A\n").theme_default,
        "dark"
    );
    // A custom theme owns the colours -> no theme-management script.
    assert!(deck_theme_head("auto", true).is_empty());
}

#[test]
fn theme_list_takes_first_entry() {
    // `theme: [dark, custom.scss]` (Quarto list form) selects the base.
    let d = render_document("---\ntheme: [dark, custom.scss]\n---\n\nx\n");
    assert_eq!(
        d.theme_default, "dark",
        "first list entry (dark) should win"
    );
}

#[test]
fn footnotes_emit_ref_and_gathered_section() {
    let page = render_html_page(
        "---\ntitle: T\n---\n\nA claim.[^1] More text.\n\n[^1]: The supporting note.\n",
        "fb",
    );
    // The reference is a superscript link to the definition.
    assert!(page.contains("class=\"qmd-fnref\""), "footnote ref: {page}");
    assert!(page.contains("href=\"#fn-1\""), "ref links to def");
    // Definitions are gathered into one footnotes section (not rendered in place).
    assert!(page.contains("class=\"footnotes\""), "footnotes section");
    assert!(page.contains("id=\"fn-1\""), "footnote def id");
    assert!(page.contains("The supporting note"), "footnote body");
    assert!(page.contains("qmd-fn-back"), "backlink to the reference");
}

#[test]
fn sidenote_div_renders_with_class() {
    // `::: {.sidenote}` is a margin note (styled via base.css float); it just needs
    // to emit a `.sidenote` block carrying the usual data-block-id (click-to-source).
    let page = render_html_page(
        "---\ntitle: T\n---\n\n::: {.sidenote}\nA margin note.\n:::\n",
        "fb",
    );
    assert!(page.contains("class=\"sidenote\""), "sidenote div: {page}");
    assert!(page.contains("A margin note"), "sidenote content");
}
