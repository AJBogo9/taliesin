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
    // The title text is used as the callout title (the kind icon precedes it).
    assert!(
        h.contains("class=\"callout-title\"") && h.contains(">My Note</div>"),
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
fn callout_emits_kind_icon_and_respects_icon_false() {
    let tip = render_document("::: {.callout-tip}\nBody.\n:::\n");
    assert!(
        tip.blocks[0].html.contains("<svg class=\"callout-icon\""),
        "tip should carry a bundled icon: {}",
        tip.blocks[0].html
    );
    let none = render_document("::: {.callout-note icon=\"false\"}\nBody.\n:::\n");
    assert!(
        !none.blocks[0].html.contains("callout-icon"),
        "icon=\"false\" suppresses the icon: {}",
        none.blocks[0].html
    );
}

#[test]
fn callout_appearance_adds_modifier_class() {
    let simple = render_document("::: {.callout-warning appearance=\"simple\"}\nBody.\n:::\n");
    assert!(
        simple.blocks[0].html.contains("callout-simple"),
        "simple appearance adds a modifier class: {}",
        simple.blocks[0].html
    );
    let def = render_document("::: {.callout-note}\nBody.\n:::\n");
    assert!(
        !def.blocks[0].html.contains("callout-simple")
            && !def.blocks[0].html.contains("callout-minimal"),
        "default appearance adds no modifier: {}",
        def.blocks[0].html
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
fn code_walkthrough_builds_sticky_panel_and_line_focused_steps() {
    let src = "::: {.code-walkthrough}\n\n\
        ```python\ndef f(x):\n    y = x + 1\n    return y\n```\n\n\
        ::: {.step lines=\"1\"}\n\nDefine the function.\n\n:::\n\n\
        ::: {.step lines=\"2-3\"}\n\nCompute and return.\n\n:::\n\n\
        :::\n";
    let doc = render_document(src);
    assert_eq!(doc.blocks.len(), 1, "one walkthrough container block");
    let h = &doc.blocks[0].html;

    // Wrapper carries the block-model attrs + the layout scaffold.
    assert!(h.contains("class=\"code-walkthrough\""), "got: {h}");
    assert!(h.contains("data-block-id=\"b-"), "wrapper id: {h}");
    assert!(
        h.contains("data-sourcepos=\"1:1-"),
        "wrapper sourcepos: {h}"
    );
    assert!(h.contains("class=\"cw-steps\""), "steps column: {h}");
    assert!(h.contains("class=\"cw-stage\""), "sticky stage: {h}");
    assert!(h.contains("class=\"cw-code\""), "code holder: {h}");

    // The panel code block is line-wrapped so its lines are addressable by ordinal.
    assert!(
        h.contains("class=\"qhl-ln\""),
        "panel lines not wrapped: {h}"
    );

    // Each step carries its focus spec AND keeps its own block id (click-to-source).
    assert!(
        h.contains("data-cw-lines=\"1\""),
        "step 1 spec missing: {h}"
    );
    assert!(
        h.contains("data-cw-lines=\"2-3\""),
        "step 2 spec missing: {h}"
    );
    assert_eq!(h.matches("class=\"step\"").count(), 2, "two step divs: {h}");
    assert!(
        h.matches("data-block-id").count() >= 4,
        "wrapper + panel + steps each keep a block id: {h}"
    );

    // No fence leakage; steps precede the stage in DOM/source order.
    assert!(!doc.body_html().contains(":::"), "fence leaked: {h}");
    assert!(
        h.find("cw-steps").unwrap() < h.find("cw-stage").unwrap(),
        "steps come before the stage in DOM order: {h}"
    );
}

#[test]
fn code_walkthrough_step_without_lines_has_no_focus_spec() {
    // A step with no `lines` clears the focus (full code undimmed): no data-cw-lines.
    let src = "::: {.code-walkthrough}\n\n\
        ```python\nx = 1\n```\n\n\
        ::: {.step}\n\nJust narration.\n\n:::\n\n\
        :::\n";
    let doc = render_document(src);
    let h = &doc.blocks[0].html;
    assert!(h.contains("class=\"step\""), "step present: {h}");
    assert!(!h.contains("data-cw-lines"), "no focus spec expected: {h}");
}

#[test]
fn panel_tabset_builds_aria_tabs_from_headings() {
    let src = "::: {.panel-tabset}\n\n\
        ## Python\n\n\
        ```python\nprint(\"hi\")\n```\n\n\
        ## R\n\n\
        ```r\nprint(\"hi\")\n```\n\n\
        :::\n";
    let doc = render_document(src);
    assert_eq!(doc.blocks.len(), 1, "one tabset container block");
    let h = &doc.blocks[0].html;

    assert!(h.contains("class=\"panel-tabset\""), "got: {h}");
    assert!(h.contains("data-block-id=\"b-"), "wrapper id: {h}");
    assert!(h.contains("role=\"tablist\""), "tablist: {h}");
    assert_eq!(h.matches("role=\"tab\"").count(), 2, "two tabs: {h}");
    assert_eq!(h.matches("role=\"tabpanel\"").count(), 2, "two panels: {h}");

    // Labels come from the headings, which are NOT re-emitted as <hN> (no TOC pollution).
    assert!(h.contains(">Python</button>"), "Python tab label: {h}");
    assert!(h.contains(">R</button>"), "R tab label: {h}");
    assert!(
        !h.contains("<h2"),
        "headings must be absorbed, not emitted: {h}"
    );

    // First tab selected, second not; exactly one panel hidden.
    assert_eq!(
        h.matches("aria-selected=\"true\"").count(),
        1,
        "one selected tab: {h}"
    );
    assert_eq!(h.matches(" hidden").count(), 1, "one hidden panel: {h}");

    // Panel bodies keep their inner code blocks (with their own ids).
    assert!(h.contains("print"), "panel bodies present: {h}");
    assert!(
        h.matches("data-block-id").count() >= 3,
        "wrapper + 2 code blocks each keep an id: {h}"
    );
    assert!(
        h.contains("aria-controls=") && h.contains("aria-labelledby="),
        "aria wiring: {h}"
    );
    assert!(!doc.body_html().contains(":::"), "fence leaked: {h}");
}

#[test]
fn panel_tabset_without_headings_falls_back_and_warns() {
    let doc = render_document("::: {.panel-tabset}\n\nJust prose, no tabs.\n\n:::\n");
    let h = &doc.blocks[0].html;
    assert!(
        h.contains("class=\"panel-tabset\""),
        "still a container: {h}"
    );
    assert!(
        !h.contains("role=\"tablist\""),
        "no tablist without headings: {h}"
    );
    assert!(h.contains("Just prose"), "content still rendered: {h}");
    assert!(
        doc.warnings.iter().any(|w| w.message.contains("tab")),
        "expected a no-tabs warning, got: {:?}",
        doc.warnings
    );
}

#[test]
fn panel_tabset_in_tab_figure_resolves_crossref() {
    let src = "::: {.panel-tabset}\n\n\
        ## Plot\n\n\
        ![A fit.](fit.png){#fig-fit}\n\n\
        :::\n\n\
        See @fig-fit for the result.\n";
    let doc = render_document(src);
    let body = doc.body_html();
    // The figure inside the tab still gets a number + anchor, and the @fig- ref links to
    // it (cross-ref resolution sees through the tabset grouping).
    assert!(
        body.contains("id=\"fig-fit\""),
        "figure anchor missing: {body}"
    );
    assert!(body.contains("Figure"), "figure not numbered: {body}");
    assert!(!body.contains("@fig-fit"), "ref left unresolved: {body}");
    assert!(
        body.contains("#fig-fit"),
        "ref not linked to the figure: {body}"
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
fn spaced_option_directives_are_recognized() {
    // Quarto tolerates whitespace between the comment marker and the pipe (`# |`,
    // `// |`, `%% |`); qmd-fast must too, or the spaced lines leak into the displayed
    // source AND their options (echo/label/...) are silently ignored.
    // Regression: corpus/posts/pca-geometry writes `# | label:` / `# | echo: false`.

    // 1. A spaced option is stripped from echoed source (not left as a comment).
    //    Check the stripped text, since highlighting splits the literal `# |`.
    let warn = render_document("```{python}\n# | warning: false\nprint(1)\n```\n");
    let text = strip_tags(&warn.blocks[0].html);
    assert!(text.contains("print(1)"));
    assert!(
        !text.contains("warning"),
        "spaced option line leaked into source: {text}"
    );

    // 2. A spaced `# | echo: false` is honoured: parsed onto the cell, source hidden.
    let echo = render_document("```{python}\n# | echo: false\nprint(1)\n```\n");
    let b = &echo.blocks[0];
    assert!(
        !b.cell.as_ref().unwrap().echo,
        "spaced echo:false must be parsed onto the cell"
    );
    assert!(
        b.html.contains("qmd-cell-hidden") && !b.html.contains("print(1)"),
        "spaced echo:false must hide the source: {}",
        b.html
    );

    // 3. A spaced `%% |` label registers the figure + resolves its cross-reference
    //    (mirrors the canonical-form mermaid test, exercised in the no-exec path).
    let fig = render_document(
        "See @fig-x.\n\n```{mermaid}\n%% | label: fig-x\n%% | fig-cap: \"Cap\"\nflowchart LR\n  A --> B\n```\n",
    );
    let body = fig.body_html();
    assert!(
        body.contains("id=\"fig-x\""),
        "spaced label did not register the figure anchor: {body}"
    );
    assert!(
        !body.contains("%% |"),
        "spaced mermaid options leaked: {body}"
    );
    assert!(
        body.contains("<a href=\"#fig-x\" class=\"qmd-xref\">Figure&nbsp;1</a>"),
        "spaced-label cross-reference did not resolve: {body}"
    );

    // 4. A spaced `// |` js option is parsed (data-name on the live placeholder).
    let js = render_document("```{js}\n// | name: x\nreturn 1;\n```\n");
    let jh = &js.blocks[0].html;
    assert!(
        jh.contains("data-name=\"x\""),
        "spaced js option not parsed: {jh}"
    );
    assert!(!jh.contains("// |"), "spaced js option line leaked: {jh}");
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

    // `{js}` cells become live placeholders; their `//|` options drive the
    // enhancer (data-* attrs) and are stripped from the emitted source.
    let js = render_document("```{js}\n//| name: x\n//| echo: false\nreturn 1;\n```\n");
    let jh = &js.blocks[0].html;
    assert!(
        jh.contains("class=\"cell qmd-js-cell\""),
        "js cell should be a live placeholder: {jh}"
    );
    assert!(
        jh.contains("type=\"application/qmd-js\"") && jh.contains("data-name=\"x\""),
        "js cell missing the qmd-js script / parsed option: {jh}"
    );
    assert!(
        !jh.contains("//| name"),
        "option lines should be stripped: {jh}"
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
fn execute_flow_mapping_sets_document_cell_defaults() {
    // The inline YAML flow form `execute: {echo: false}` must behave like the
    // block form (`execute:\n  echo: false`).
    let doc = render_document("---\nexecute: {echo: false}\n---\n\n```{python}\nprint(1)\n```\n");
    let cell = doc
        .blocks
        .iter()
        .find(|b| b.cell.is_some())
        .expect("a code cell");
    // echo:false renders the cell as the hidden marker (no source listing).
    assert!(
        cell.html.contains("qmd-cell-hidden"),
        "execute: {{echo: false}} (flow form) should hide the cell source: {}",
        cell.html
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
fn js_cell_emits_native_wire_format_with_options() {
    // A native `{js}` cell is a live placeholder: a target div + an
    // `application/qmd-js` script carrying the source, with `//|` options as data-*.
    let d = render_document("```{js}\n//| name: signalX\nreturn [1, 2, 3];\n```\n");
    let h = &d.blocks[0].html;
    assert!(
        h.contains("class=\"cell qmd-js-cell\""),
        "js placeholder: {h}"
    );
    assert!(
        h.contains("<script type=\"application/qmd-js\"") && h.contains("data-name=\"signalX\""),
        "qmd-js script + name option: {h}"
    );
    assert!(!h.contains("ojs"), "no OJS vocabulary remains: {h}");

    let v =
        render_document("```{js}\n//| viewof: n\nreturn document.createElement(\"input\");\n```\n");
    assert!(
        v.blocks[0].html.contains("data-viewof=\"n\""),
        "viewof option: {}",
        v.blocks[0].html
    );

    let s = render_document("```{js}\n//| input: n, m\nreturn container;\n```\n");
    assert!(
        s.blocks[0].html.contains("data-inputs=\"n,m\""),
        "input option: {}",
        s.blocks[0].html
    );
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
        page.contains("<div class=\"qmd-deck\">"),
        "should render as a deck"
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

// --- deck / slides ---

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
        slides.contains("<section id=\"first\" class=\"qmd-slide\" data-level=\"2\">"),
        "got: {slides}"
    );
    assert!(
        slides.contains("<section id=\"second\" class=\"qmd-slide\" data-level=\"2\">"),
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
fn pause_marker_drops_and_fragments_following_blocks() {
    // Quarto `. . .` is a pause: the marker itself is dropped, and every block
    // after it (until end of slide) becomes a `.fragment` step.
    let doc = render_document(
        "---\nformat: revealjs\n---\n\n## S\n\nVisible now.\n\n. . .\n\nAfter the pause.\n",
    );
    let slides = slides_html(None, None, &doc.blocks);
    assert!(
        !slides.contains(". . ."),
        "the pause marker must not render as text: {slides}"
    );
    // The block before the pause stays plain; the one after gains `class="fragment"`.
    assert!(
        slides.contains(">Visible now.</p>"),
        "pre-pause block should be unmodified: {slides}"
    );
    assert!(
        slides.contains("class=\"fragment\">After the pause."),
        "post-pause block should become a fragment: {slides}"
    );
}

#[test]
fn h1_wraps_following_h2s_in_a_vertical_stack() {
    let doc =
        render_document("---\nformat: revealjs\n---\n\n# Part One\n\nIntro.\n\n## A\n\n## B\n");
    let slides = slides_html(None, None, &doc.blocks);
    // Outer wrapper section, then the h1 lead slide, then the two h2 children.
    assert!(
        slides
            .contains("<section>\n<section id=\"part-one\" class=\"qmd-slide\" data-level=\"1\">"),
        "got: {slides}"
    );
    assert!(
        slides.contains("<section id=\"a\" class=\"qmd-slide\" data-level=\"2\">"),
        "got: {slides}"
    );
    assert!(
        slides.contains("<section id=\"b\" class=\"qmd-slide\" data-level=\"2\">"),
        "got: {slides}"
    );
    // 1 wrapper + lead + 2 children = 4 sections.
    assert_eq!(slides.matches("<section").count(), 4, "got: {slides}");
}

#[test]
fn deck_page_carries_native_scaffolding() {
    let page = render_html_page(
        "---\ntitle: D\nformat: revealjs\n---\n\n## Slide\n",
        "fallback",
    );
    assert!(page.contains("class=\"qmd-deck\""));
    assert!(page.contains("class=\"qmd-slides\""));
    // The deck engine is bundled (no CDN); it exposes the window.QmdDeck API.
    assert!(page.contains("window.QmdDeck"));
    assert!(page.contains("QmdDeck.initialize("));
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
        "deck heading should not carry an id: {}",
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
fn figure_honors_both_width_and_height() {
    // `height=` must land in the inline style alongside `width=` (it was silently
    // dropped before), each escaped like width.
    let doc = render_document("![Plot](p.png){#fig-p width=50% height=200px}\n");
    let h = &doc.blocks[0].html;
    assert!(
        h.contains("style=\"width:50%;height:200px\""),
        "both width and height must be in the style: {h}"
    );
}

#[test]
fn figure_height_only_emits_height_style() {
    let doc = render_document("![Plot](p.png){#fig-p height=200px}\n");
    let h = &doc.blocks[0].html;
    assert!(
        h.contains("style=\"height:200px\""),
        "a height-only figure must emit a height style: {h}"
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
fn assembled_page_ships_reader_preferences() {
    let page = render_html_page("# Title\n\nProse to read.\n", "doc");
    // The reader-preferences enhancer ("Aa" control) ships on every built page via
    // code_scripts(), so a reader can pick size / width / theme.
    assert!(
        page.contains("qmdInitReaderPrefs"),
        "reader-preferences enhancer not shipped in the assembled page"
    );
    // The pre-paint head script applies the reader's saved size/width before paint
    // (no flash), so it must reference the reader-scale preference.
    assert!(
        page.contains("qmd-reader-scale"),
        "pre-paint reader-preference apply missing from the page head"
    );
}

#[test]
fn assembled_page_ships_reading_progress() {
    let page = render_html_page("# Title\n\nProse to read at length.\n", "doc");
    // The reading-progress enhancer (progress bar + min-left + resume) ships on every
    // built page via code_scripts().
    assert!(
        page.contains("qmdInitReadingProgress"),
        "reading-progress enhancer not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_reader_highlights() {
    let page = render_html_page("# Title\n\nProse a reader can highlight.\n", "doc");
    assert!(
        page.contains("qmdInitHighlights"),
        "reader-highlights enhancer not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_highlight_index() {
    let page = render_html_page("# Title\n\nProse to highlight and review.\n", "doc");
    assert!(
        page.contains("qmdInitHighlightIndex"),
        "highlight-index enhancer not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_cite_action() {
    let page = render_html_page("# Title\n\nProse a reader can cite.\n", "doc");
    // The selection toolbar's "Cite" action copies a BibTeX entry deep-linking to the
    // selection; the pure builder qmdBuildBibtex is its unique discriminator token.
    assert!(
        page.contains("qmdBuildBibtex"),
        "copy-as-citation (BibTeX) builder not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_anchor_links() {
    let page = render_html_page("# Title\n\n## A section\n\nProse.\n", "doc");
    // The anchor-copy-link enhancer reveals a `#` on each heading/float and copies its
    // canonical deep link; qmdInitAnchorLinks is its unique discriminator token.
    assert!(
        page.contains("qmdInitAnchorLinks"),
        "anchor copy-link enhancer not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_focus_mode() {
    let page = render_html_page("# Title\n\nProse to read in focus.\n", "doc");
    // Focus/reading mode hides site chrome and centers the prose; qmdInitFocusMode is its
    // discriminator, and body.qmd-focus is the CSS hook.
    assert!(
        page.contains("qmdInitFocusMode") && page.contains("qmd-focus"),
        "focus/reading mode not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_hover_cards() {
    let page = render_html_page("# Title\n\nProse with a [link](#title).\n", "doc");
    // Hover cross-reference cards: hovering any in-page reference (@fig-/@sec-/[@cite]/
    // footnote) previews its target block. qmdInitLinkPreview is the enhancer; this guards
    // it against accidental removal (parity with the other reader enhancers' ships-tests).
    assert!(
        page.contains("qmdInitLinkPreview"),
        "hover cross-reference cards (link-preview) enhancer not shipped"
    );
}

#[test]
fn assembled_page_ships_reader_menu() {
    let page = render_html_page("# Title\n\nProse.\n", "doc");
    // The reader-menu host consolidates the reader controls into one launcher + menu.
    assert!(
        page.contains("qmdInitReaderMenu") && page.contains("qmdReaderMenu"),
        "reader-menu host not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_bookmarks() {
    let page = render_html_page("# Title\n\nProse.\n", "doc");
    // Section bookmarks: hover-heading toggle + a Bookmarks menu section.
    assert!(
        page.contains("qmdInitBookmarks"),
        "bookmarks enhancer not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_selection_toolbar() {
    let page = render_html_page("# Title\n\nProse.\n", "doc");
    // Selection toolbar: the .qmd-seltools bar + the text-fragment share-link builder.
    assert!(
        page.contains("qmd-seltools") && page.contains("qmdBuildTextFragmentUrl"),
        "selection toolbar not shipped in the assembled page"
    );
}

#[test]
fn search_js_ships_tokenizing_matcher() {
    // The Cmd-K matcher is multi-term/prefix/fuzzy with a shared range emitter, not a single
    // whole-query indexOf. emitRanges is the marker symbol of the rework.
    assert!(
        super::SEARCH_JS.contains("emitRanges"),
        "search.js still ships the old single-indexOf matcher"
    );
}

#[test]
fn search_js_localizes_the_kbd_hint_off_mac() {
    // The kbd badge is server-rendered with the Mac glyph (⌘K); the shipped client JS must
    // rewrite it to "Ctrl K" on non-Mac platforms (the badge class is the rewrite target).
    assert!(
        super::SEARCH_JS.contains("Ctrl K"),
        "search.js must localize the ⌘K hint to Ctrl K off Mac"
    );
    assert!(
        super::SEARCH_JS.contains("qmd-search-kbd"),
        "search.js must target the .qmd-search-kbd badge to localize it"
    );
}

#[test]
fn assembled_page_ships_focus_trap() {
    let page = render_html_page("# Title\n\n![alt](x.png)\n", "doc");
    // The shared modal focus-trap utility (lightbox / reader menu / Cmd-K) ships in the page.
    assert!(
        page.contains("qmdFocusTrap"),
        "modal focus-trap utility not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_line_spacing() {
    let page = render_html_page("# Title\n\nProse.\n", "doc");
    // The reader line-spacing control drives prose line-height via this CSS var.
    assert!(
        page.contains("--qmd-reader-leading"),
        "reader line-spacing var not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_letter_word_spacing() {
    let page = render_html_page("# Title\n\nProse.\n", "doc");
    // The reader letter/word-spacing controls (WCAG 1.4.12) drive prose tracking via these vars.
    assert!(
        page.contains("--qmd-reader-letter"),
        "reader letter-spacing var not shipped in the assembled page"
    );
    assert!(
        page.contains("--qmd-reader-word"),
        "reader word-spacing var not shipped in the assembled page"
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
fn toc_page_ships_read_state_marker() {
    // A page with a TOC ships the read-state scrollspy decoration: the script marks the
    // sections a reader has scrolled through (`.qmd-toc-read`) and persists them in the
    // reader's OWN localStorage (`qmd-read:<path>`). Reader-side, read-only.
    let toc_page = render_html_page(
        "---\ntitle: Doc\nformat:\n  html:\n    toc: true\n---\n\n# A\n\ntext\n\n## B\n\nmore\n",
        "fb",
    );
    assert!(
        toc_page.contains("qmd-toc-read"),
        "read-state class/CSS missing from a TOC page"
    );
    assert!(
        toc_page.contains("qmd-read:"),
        "read-state storage key missing from the TOC scrollspy"
    );

    // No TOC -> no scrollspy script -> the read-state persistence logic never ships
    // (guards against the feature being always-on). The CSS class lives in base.css
    // unconditionally, so the storage key is the TOC-only discriminator.
    let plain = render_html_page("---\ntitle: Doc\n---\n\n# A\n", "fb");
    assert!(
        !plain.contains("qmd-read:"),
        "read-state logic should ship only with a TOC"
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
            .any(|w| w.message.contains("bibliography file not found: nope.bib")),
        "got: {:?}",
        doc.warnings
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("theme file not found: gone.css")),
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

#[test]
fn input_slider_shortcode_emits_reactive_control() {
    let doc = render_document_with_includes(
        "{{< input name=\"k\" type=\"slider\" min=\"1\" max=\"10\" value=\"3\" label=\"k\" >}}\n",
        std::path::Path::new("."),
    );
    let h = doc.body_html();
    assert!(h.contains("class=\"qmd-input\""), "wrapper: {h}");
    assert!(h.contains("data-qmd-input=\"k\""), "named input: {h}");
    assert!(h.contains("type=\"range\""), "range control: {h}");
    assert!(h.contains("min=\"1\"") && h.contains("max=\"10\"") && h.contains("value=\"3\""));
    assert!(
        h.contains("<output class=\"qmd-input-out\" data-qmd-out>3</output>"),
        "slider readout: {h}"
    );
    assert!(h.contains(">k</label>"), "label: {h}");
}

#[test]
fn input_shortcode_other_types_emit_their_native_control() {
    let p = std::path::Path::new(".");
    let num =
        render_document_with_includes("{{< input name=\"n\" type=\"number\" step=\"0.1\" >}}\n", p)
            .body_html();
    assert!(num.contains("type=\"number\"") && num.contains("step=\"0.1\""));
    assert!(!num.contains("data-qmd-out"), "no readout on number: {num}");

    let cb = render_document_with_includes(
        "{{< input name=\"on\" type=\"checkbox\" value=\"true\" >}}\n",
        p,
    )
    .body_html();
    assert!(
        cb.contains("type=\"checkbox\"") && cb.contains(" checked"),
        "checked: {cb}"
    );

    let tx =
        render_document_with_includes("{{< input name=\"q\" type=\"text\" value=\"hi\" >}}\n", p)
            .body_html();
    assert!(tx.contains("type=\"text\"") && tx.contains("value=\"hi\""));

    let sel = render_document_with_includes(
        "{{< input name=\"c\" type=\"select\" options=\"a,b,c\" value=\"b\" >}}\n",
        p,
    )
    .body_html();
    assert!(sel.contains("<select"), "select: {sel}");
    assert!(
        sel.contains("<option>a</option>") && sel.contains("<option selected>b</option>"),
        "options: {sel}"
    );
}

#[test]
fn scrolly_arm_emits_stage_steps_and_reactive_input() {
    let doc = render_document(
        "::: {.scrolly name=\"scene\"}\nThe stage paragraph.\n\n::: {.step state=\"a\"}\nStep A.\n:::\n\n::: {.step state=\"b\"}\nStep B.\n:::\n:::\n",
    );
    let h = doc.body_html();
    assert!(h.contains("class=\"qmd-scrolly\""), "wrapper: {h}");
    assert!(
        h.contains("class=\"scrolly-steps\"") && h.contains("class=\"scrolly-stage\""),
        "split: {h}"
    );
    assert!(h.contains("data-scrolly-name=\"scene\""), "name attr: {h}");
    assert!(
        h.contains(
            "<input type=\"hidden\" class=\"qmd-scrolly-input\" data-qmd-input=\"scene\" value=\"a\">"
        ),
        "hidden reactive input with first step's state: {h}"
    );
    assert!(
        h.contains("data-state=\"a\"") && h.contains("data-state=\"b\""),
        "step states: {h}"
    );
    assert!(
        h.contains("The stage paragraph."),
        "stage content present: {h}"
    );
}

#[test]
fn scrolly_without_name_omits_hidden_input() {
    let doc = render_document("::: {.scrolly}\nStage.\n\n::: {.step state=\"a\"}\nA.\n:::\n:::\n");
    let h = doc.body_html();
    assert!(h.contains("class=\"qmd-scrolly\""));
    assert!(
        !h.contains("data-qmd-input"),
        "no hidden input without name=: {h}"
    );
    assert!(!h.contains("data-scrolly-name"), "no name attr: {h}");
}

#[test]
fn prose_lint_emits_located_warnings_when_opted_in() {
    let doc = render_document("---\ntitle: T\nprose-lint: true\n---\n\nThis is very very good.\n");
    // "very very" -> a doubled word AND two weasel-word hits, all on line 6.
    let msgs: Vec<_> = doc.warnings.iter().map(|w| w.message.as_str()).collect();
    assert!(
        msgs.contains(&"repeated word `very`"),
        "expected doubled-word warning, got: {msgs:?}"
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("weasel word `very`") && w.line == Some(6)),
        "weasel warning should be located on line 6, got: {:?}",
        doc.warnings
    );
}

#[test]
fn prose_lint_is_silent_when_not_opted_in() {
    let doc = render_document("# T\n\nThis is very very good.\n");
    assert!(
        !doc.warnings
            .iter()
            .any(|w| w.message.contains("weasel") || w.message.contains("repeated word")),
        "prose-lint must be off without opt-in, got: {:?}",
        doc.warnings
    );
}

#[test]
fn typography_polish_css_ships() {
    let page = render_html_page("# Title\n\nSome prose.\n", "doc");
    assert!(
        page.contains("text-wrap: pretty"),
        "pretty wrap rule must ship"
    );
    assert!(
        page.contains("text-wrap: balance"),
        "balance rule must ship"
    );
}

#[test]
fn build_mode_content_gates_separate_enhancers() {
    // Pure prose: a static build keeps code-enhance.js (the reader menu + a11y layer
    // that every page benefits from) but drops the DOM-specific enhancers it can't use.
    let prose = code_scripts_for("<p>Just prose.</p>", OutputMode::Build);
    assert!(
        prose.contains("qmdInitReaderMenu"),
        "build keeps code-enhance.js"
    );
    assert!(
        !prose.contains("Narrated code walkthrough"),
        "no walkthrough.js on a prose page"
    );
    assert!(
        !prose.contains("Tabbed panels: the interaction layer"),
        "no tabset.js on a prose page"
    );
    assert!(
        !prose.contains("Scrollytelling: scroll-driven sticky-stage"),
        "no scrolly.js on a prose page"
    );
    assert!(
        !prose.contains("self-contained enhancer module"),
        "no mermaid.js on a prose page"
    );
    assert!(
        !prose.contains("a tiny enhancer that replaces the vendored"),
        "no qmd-js.js on a prose page"
    );

    // A page that actually contains a tabset gets tabset.js in a build (but still not
    // the enhancers for constructs it lacks).
    let tabset = code_scripts_for("<div class=\"panel-tabset\"></div>", OutputMode::Build);
    assert!(
        tabset.contains("Tabbed panels: the interaction layer"),
        "a tabset on the page ships tabset.js"
    );
    assert!(
        !tabset.contains("Narrated code walkthrough"),
        "still no walkthrough.js"
    );

    // Preview ships every enhancer regardless of body (a doc can gain any construct on
    // an edit — same reasoning as KaTeX/d3 always-on in preview). Gating is Build-only.
    let preview = code_scripts_for("<p>Just prose.</p>", OutputMode::Preview);
    assert!(
        preview.contains("self-contained enhancer module"),
        "preview ships mermaid.js unconditionally"
    );
    assert!(
        preview.contains("Tabbed panels: the interaction layer"),
        "preview ships tabset.js unconditionally"
    );

    // Bare ships no enhancer scripts at all (the zero-<script> contract).
    assert!(
        code_scripts_for("<p>x</p>", OutputMode::Bare).is_empty(),
        "bare ships no enhancer scripts"
    );
}

#[test]
fn bare_theming_resolves_per_theme_default() {
    // CSS-only theming has three branches: a forced dark theme hard-codes the dark
    // layer onto :root (no media query); a forced light theme adds nothing (base
    // :root is light); an unforced (auto) theme follows the OS via a media query.
    // `#16181d` is the dark `--qmd-bg`, present only in the dark layer.
    let bare = |src: &str| render_doc_to_page(&render_document(src), "t", OutputMode::Bare);

    let dark = bare("---\ntheme: dark\n---\n\nx\n");
    assert!(dark.contains("#16181d"), "forced dark ships the dark layer");
    assert!(
        !dark.contains("@media (prefers-color-scheme: dark)"),
        "forced dark is unconditional, not OS-gated"
    );
    assert!(!dark.contains("<script"), "still script-free");

    let light = bare("---\ntheme: light\n---\n\nx\n");
    assert!(
        !light.contains("#16181d"),
        "forced light ships no dark layer (base :root is light)"
    );

    let auto = bare("---\ntitle: T\n---\n\nx\n");
    assert!(
        auto.contains("@media (prefers-color-scheme: dark)"),
        "an unforced theme follows the OS via a media query"
    );
    assert!(
        auto.contains("#16181d"),
        "the OS-gated layer still carries the dark vars"
    );
}

#[test]
fn site_build_path_content_gates_enhancers() {
    // The in-site page builder hardcodes OutputMode::Build, so a site/book build
    // content-gates the separate enhancers just like a single-doc build (this pins
    // the spec's "site builds get Phase-1 gating too" claim). Markers are each
    // script's distinctive comment (absent from base.css, unlike "walkthrough").
    let doc = render_document("# A chapter\n\nProse only — no tabset, mermaid, or scrolly.\n");
    let page = html_page_from_doc_in_site(&doc, "chapter", &SiteCtx::default());
    assert!(
        page.contains("qmdInitReaderMenu"),
        "a site page still ships code-enhance.js (reader menu + a11y)"
    );
    assert!(
        !page.contains("Tabbed panels: the interaction"),
        "no tabset.js on a prose site page"
    );
    assert!(
        !page.contains("Scrollytelling: scroll-driven"),
        "no scrolly.js on a prose site page"
    );
    assert!(
        !page.contains("self-contained enhancer module"),
        "no mermaid.js on a prose site page"
    );
}
