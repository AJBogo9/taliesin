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
    assert!(h.contains("class=\"tali-title-block\""));
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
    let doc = render_document("---\ntitle: T\nformat: deck\n---\n\n## Slide\n");
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
    assert!(h.contains("tali-layout"), "got: {h}");
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
    assert!(!body.contains("tali-div"), "got: {body}");
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
        h.contains("class=\"tali-hl-ln\""),
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
fn mermaid_library_inlined_into_build_pages_only() {
    // A static Build page WITH a diagram inlines the vendored mermaid library (it sets
    // globalThis.mermaid, which the loader short-circuits on) so the diagram renders
    // fully offline — no CDN fetch.
    let body = "<pre class=\"mermaid\">flowchart LR\n A --&gt; B</pre>";
    let build = code_scripts_for(body, OutputMode::Build);
    assert!(
        build.contains("__esbuild_esm_mermaid") && build.contains("globalThis.mermaid"),
        "Build must inline the vendored mermaid library for a diagram page"
    );
    assert!(
        build.contains("__qmdMermaidLoading"),
        "Build still ships the loader (uses the inlined global)"
    );
    // Content-gated: a Build page with NO diagram inlines nothing.
    let build_plain = code_scripts_for("<p>no diagram</p>", OutputMode::Build);
    assert!(
        !build_plain.contains("__esbuild_esm_mermaid"),
        "Build must NOT inline mermaid on a diagram-less page"
    );
    // Preview keeps the lean lazy loader (inlining 2.5 MB on every save would bloat it).
    let preview = code_scripts_for(body, OutputMode::Preview);
    assert!(
        !preview.contains("__esbuild_esm_mermaid") && preview.contains("__qmdMermaidLoading"),
        "Preview keeps only the lazy loader, not the inlined library"
    );
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
        body.contains("class=\"tali-figure"),
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
        body.contains("<a href=\"#fig-flow\" class=\"tali-xref\">Figure&nbsp;1</a>"),
        "cross-reference did not resolve: {body}"
    );
}

#[test]
fn static_toc_page_ships_the_mobile_pull_up_sheet() {
    // A static TOC page opts into the mobile pull-up sheet (markup + body class + the
    // toc-sheet.js enhancer) so its "on this page" TOC is a bottom sheet on narrow
    // screens instead of stranding at the page bottom.
    let html = render_html_page(
        "---\ntitle: T\ntoc: true\n---\n\n# A\n\ntext\n\n## B\n\nmore\n",
        "f",
    );
    assert!(
        html.contains("id=\"tali-toc-handle\""),
        "sheet handle markup missing"
    );
    assert!(
        html.contains("id=\"tali-toc-backdrop\""),
        "sheet backdrop markup missing"
    );
    assert!(
        !TOC_SHEET_JS.is_empty() && toc_scripts().contains(TOC_SHEET_JS),
        "toc-sheet.js enhancer not bundled on TOC pages"
    );
    // Progressive enhancement: the enhancer (not the server) opts the body into the sheet,
    // so a no-JS page keeps its in-flow TOC layout.
    assert!(
        TOC_SHEET_JS.contains("classList.add(\"tali-toc-sheet\")"),
        "toc-sheet.js should add the tali-toc-sheet class at runtime"
    );
}

#[test]
fn static_page_has_no_dead_click_to_source_outline() {
    // A built/rendered static page is a read-only view with no editor bridge, so it must
    // NOT ship the click-to-source click handler (it drew a `.tali-hl` outline on every
    // click + console.logged with nothing listening). Click-to-source lives only in the
    // live preview (client.js wires it to the editor).
    let html = render_html_page("# Title\n\nBody.\n", "fallback");
    assert!(
        !html.contains("Click any block"),
        "static page still ships the click-to-source handler"
    );
    assert!(
        !html.contains("console.log('block'"),
        "static page still logs click-to-source to the console"
    );
}

#[test]
fn math_in_option_string_caption_renders_katex() {
    // `$...$` in a `fig-cap:`/`lst-cap:` option string must render as KaTeX, exactly
    // like an image-alt caption does — not survive as literal `$E=mc^2$` text.
    let doc = render_document(
        "```{mermaid}\n%%| label: fig-e\n%%| fig-cap: \"Energy is $E=mc^2$ ok\"\nflowchart LR\n  A --> B\n```\n",
    );
    let body = doc.body_html();
    let cap_start = body.find("<figcaption>").expect("no figcaption");
    let cap_end = body[cap_start..].find("</figcaption>").unwrap() + cap_start;
    let caption = &body[cap_start..cap_end];
    assert!(
        caption.contains("katex"),
        "caption math not rendered to KaTeX: {caption}"
    );
    assert!(
        !caption.contains("$E=mc^2$"),
        "literal math delimiters leaked into the caption: {caption}"
    );
    // The surrounding prose still renders.
    assert!(
        caption.contains("Energy is"),
        "caption prose lost: {caption}"
    );
}

#[test]
fn spaced_option_directives_are_recognized() {
    // Quarto tolerates whitespace between the comment marker and the pipe (`# |`,
    // `// |`, `%% |`); taliesin must too, or the spaced lines leak into the displayed
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
        b.html.contains("tali-cell-hidden") && !b.html.contains("print(1)"),
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
        body.contains("<a href=\"#fig-x\" class=\"tali-xref\">Figure&nbsp;1</a>"),
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
        !h.contains("tali-figure"),
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
        jh.contains("class=\"cell tali-js-cell\""),
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
        b.html.contains("tali-cell-hidden"),
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
        cell.html.contains("tali-cell-hidden"),
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
fn duplicate_explicit_heading_ids_are_deduped() {
    // Two headings with the SAME explicit `{#dup}` must NOT emit duplicate element
    // ids (which would silently break in-page anchors + `@sec-` refs): the second
    // gets a `-N` suffix, and a located warning is raised.
    let doc = render_document("# Title {#dup}\n\nText.\n\n# Title {#dup}\n\nMore.\n");
    let headings: Vec<&str> = doc
        .blocks
        .iter()
        .filter(|b| b.html.starts_with("<h"))
        .map(|b| b.html.as_str())
        .collect();
    assert_eq!(
        headings.len(),
        2,
        "expected two heading blocks: {headings:?}"
    );
    assert!(
        headings[0].contains("id=\"dup\""),
        "first id missing: {}",
        headings[0]
    );
    assert!(
        headings[1].contains("id=\"dup-1\"") && !headings[1].contains("id=\"dup\""),
        "second duplicate explicit id not deduped: {}",
        headings[1]
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("duplicate heading id")),
        "no duplicate-heading-id warning: {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );

    // An explicit id colliding with an autoslug is also deduped (autoslug first).
    let mixed = render_document("# Intro\n\nText.\n\n# Other {#intro}\n");
    let mixed_h: Vec<&str> = mixed
        .blocks
        .iter()
        .filter(|b| b.html.starts_with("<h"))
        .map(|b| b.html.as_str())
        .collect();
    assert!(mixed_h[0].contains("id=\"intro\""), "{}", mixed_h[0]);
    assert!(
        mixed_h[1].contains("id=\"intro-1\""),
        "explicit id colliding with an autoslug not deduped: {}",
        mixed_h[1]
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
        body.contains("class=\"tali-xref\">Section&nbsp;1</a>"),
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
        body.contains("class=\"tali-xref\">Table&nbsp;1</a>"),
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
        h.contains("class=\"cell tali-js-cell\""),
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
    assert!(h.contains("tali-math-block"), "got: {h}");
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
    assert!(
        html.contains("tali-eqn-number"),
        "equation was not numbered"
    );
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
            format: deck\n\
            include-in-header:\n  text: |\n    <meta name=\"deck\" content=\"1\">\n\
            include-after-body:\n  text: |\n    <script>window.__deck=1</script>\n\
            ---\n\n## Slide\n";
    let page = render_html_page(src, "deck");
    assert!(
        page.contains("<div class=\"tali-deck\">"),
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
    // Inline form.
    let inline = render_document("---\nformat: deck\n---\n\n## A\n");
    assert_eq!(inline.format, DocFormat::Reveal);
    // Bare block form: `format:` with just `deck:` subkey.
    let bare_block =
        render_document("---\nformat:\n  deck:\n    slide-number: true\n---\n\n## A\n");
    assert_eq!(bare_block.format, DocFormat::Reveal);
    // Nested block form: `format:` with a `<name>-deck` subkey.
    let ext =
        render_document("---\nformat:\n  custom-deck:\n    slide-number: true\n---\n\n## A\n");
    assert_eq!(ext.format, DocFormat::Reveal);
    // A normal post is Html, even if a nested non-format key mentions deck.
    let post = render_document("---\ntitle: Post\nformat: html\n---\n\nHi.\n");
    assert_eq!(post.format, DocFormat::Html);
    // A theme filename that merely contains "deck" must not flip an HTML doc.
    let not_a_deck = render_document("---\nformat: html\ntheme: my-deck.css\n---\n\nHi.\n");
    assert_eq!(not_a_deck.format, DocFormat::Html);
}

#[test]
fn revealjs_format_is_no_longer_a_deck() {
    // `format: revealjs` was the deprecated Quarto spelling; after shedding it, a
    // doc with that format is a normal HTML page, not a deck.
    let doc = render_document("---\nformat: revealjs\n---\n\n## A Slide\n");
    assert_eq!(doc.format, DocFormat::Html);
}

#[test]
fn deck_splits_into_title_slide_and_one_section_per_heading() {
    let doc = render_document(
        "---\ntitle: Deck\nsubtitle: A subtitle\nformat: deck\n---\n\n## First\n\nHello.\n\n## Second\n\nWorld.\n",
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
        slides.contains(
            "<section id=\"first\" class=\"tali-slide\" role=\"group\" aria-roledescription=\"slide\" data-level=\"2\">"
        ),
        "got: {slides}"
    );
    assert!(
        slides.contains(
            "<section id=\"second\" class=\"tali-slide\" role=\"group\" aria-roledescription=\"slide\" data-level=\"2\">"
        ),
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
    let doc = render_document("---\nformat: deck\n---\n\nOne.\n\n---\n\nTwo.\n");
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
        "---\nformat: deck\n---\n\n## S\n\nVisible now.\n\n. . .\n\nAfter the pause.\n",
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
    let doc = render_document("---\nformat: deck\n---\n\n# Part One\n\nIntro.\n\n## A\n\n## B\n");
    let slides = slides_html(None, None, &doc.blocks);
    // Outer wrapper section, then the h1 lead slide, then the two h2 children.
    assert!(
        slides
            .contains("<section>\n<section id=\"part-one\" class=\"tali-slide\" role=\"group\" aria-roledescription=\"slide\" data-level=\"1\">"),
        "got: {slides}"
    );
    assert!(
        slides.contains(
            "<section id=\"a\" class=\"tali-slide\" role=\"group\" aria-roledescription=\"slide\" data-level=\"2\">"
        ),
        "got: {slides}"
    );
    assert!(
        slides.contains(
            "<section id=\"b\" class=\"tali-slide\" role=\"group\" aria-roledescription=\"slide\" data-level=\"2\">"
        ),
        "got: {slides}"
    );
    // 1 wrapper + lead + 2 children = 4 sections.
    assert_eq!(slides.matches("<section").count(), 4, "got: {slides}");
}

#[test]
fn deck_page_carries_native_scaffolding() {
    let page = render_html_page("---\ntitle: D\nformat: deck\n---\n\n## Slide\n", "fallback");
    assert!(page.contains("class=\"tali-deck\""));
    assert!(page.contains("class=\"tali-slides\""));
    // The deck engine is bundled (no CDN); it exposes the window.TaliesinDeck API.
    assert!(page.contains("window.TaliesinDeck"));
    assert!(page.contains("TaliesinDeck.initialize("));
    assert!(
        !page.contains("jsdelivr") || !page.contains("reveal.js@"),
        "the deck must not load reveal.js from a CDN"
    );
}

#[test]
fn code_line_numbers_wraps_lines_for_stepping() {
    let page = render_html_page(
        "---\nformat: deck\n---\n\n## S\n\n```{.python code-line-numbers=\"1|2\"}\na = 1\nb = 2\n```\n",
        "fallback",
    );
    assert!(
        page.contains("data-code-lines=\"1|2\""),
        "missing line spec"
    );
    // two source lines -> two line spans (the trailing-newline line is dropped).
    assert_eq!(
        page.matches("class=\"tali-hl-ln\"").count(),
        2,
        "expected one line span per source line"
    );
    // a code block without the attribute is left unwrapped.
    let plain = render_html_page(
        "---\nformat: deck\n---\n\n## S\n\n```python\na = 1\n```\n",
        "fb",
    );
    // (check the attribute, not bare "tali-hl-ln" — the inlined CSS mentions `.tali-hl-ln`.)
    assert!(
        !plain.contains("class=\"tali-hl-ln\""),
        "plain code should not be line-wrapped"
    );
}

#[test]
fn heading_background_attr_moves_to_section() {
    let page = render_html_page(
        "---\nformat: deck\n---\n\n## Title {background-color=\"#123456\"}\n\nbody\n",
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
        "---\nformat: deck\n---\n\n## Title {auto-animate=true}\n\nbody\n",
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
        "---\nformat: deck\n---\n\n## S\n\n::: {.magic-move}\n```js\na = 1\n```\n\n```js\na = 2\nb = 3\n```\n:::\n",
        "fb",
    );
    assert!(
        page.contains("class=\"magic-move\""),
        "magic-move div missing"
    );
    // both blocks' lines are wrapped so the engine can match/glide them (1 + 2 = 3).
    assert_eq!(
        page.matches("class=\"tali-hl-ln\"").count(),
        3,
        "magic-move code blocks should be line-wrapped"
    );
}

#[test]
fn deck_dedups_repeated_slide_ids() {
    // Repeated headings (common with auto-animate's shared titles) must get
    // distinct section ids, else `#/hash` + getElementById only find the first.
    let page = render_html_page(
        "---\nformat: deck\n---\n\n## Step\n\na\n\n## Step\n\nb\n",
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
    let doc = render_document("---\nformat: deck\n---\n\n## A Slide\n");
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
        h.contains("class=\"tali-figure tali-figure-center\""),
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
fn assembled_page_ships_theme_picker() {
    let page = render_html_page("# Title\n\nProse to read.\n", "doc");
    // The theme-picker enhancer (the Settings gear's Theme row) ships on every built page via
    // code_scripts(), so a reader can choose light / dark / sepia.
    assert!(
        page.contains("taliInitReaderPrefs"),
        "theme-picker enhancer not shipped in the assembled page"
    );
    // The pre-paint head script applies the reader's saved theme before paint (no flash),
    // so it must reference the theme preference key.
    assert!(
        page.contains("qmd-theme"),
        "pre-paint theme apply missing from the page head"
    );
}

#[test]
fn assembled_page_ships_reading_progress() {
    let page = render_html_page("# Title\n\nProse to read at length.\n", "doc");
    // The reading-progress enhancer (progress bar + min-left + resume) ships on every
    // built page via code_scripts().
    assert!(
        page.contains("taliInitReadingProgress"),
        "reading-progress enhancer not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_anchor_links() {
    let page = render_html_page("# Title\n\n## A section\n\nProse.\n", "doc");
    // The anchor-copy-link enhancer reveals a `#` on each heading/float and copies its
    // canonical deep link; taliInitAnchorLinks is its unique discriminator token.
    assert!(
        page.contains("taliInitAnchorLinks"),
        "anchor copy-link enhancer not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_focus_mode() {
    let page = render_html_page("# Title\n\nProse to read in focus.\n", "doc");
    // Focus/reading mode hides site chrome and centers the prose; taliInitFocusMode is its
    // discriminator, and body.tali-focus is the CSS hook.
    assert!(
        page.contains("taliInitFocusMode") && page.contains("tali-focus"),
        "focus/reading mode not shipped in the assembled page"
    );
}

#[test]
fn assembled_page_ships_hover_cards() {
    let page = render_html_page("# Title\n\nProse with a [link](#title).\n", "doc");
    // Hover cross-reference cards: hovering any in-page reference (@fig-/@sec-/[@cite]/
    // footnote) previews its target block. taliInitLinkPreview is the enhancer; this guards
    // it against accidental removal (parity with the other reader enhancers' ships-tests).
    assert!(
        page.contains("taliInitLinkPreview"),
        "hover cross-reference cards (link-preview) enhancer not shipped"
    );
}

#[test]
fn assembled_page_ships_reader_menu() {
    let page = render_html_page("# Title\n\nProse.\n", "doc");
    // The reader-menu host consolidates the reader controls into one launcher + menu.
    assert!(
        page.contains("taliInitReaderMenu") && page.contains("taliReaderMenu"),
        "reader-menu host not shipped in the assembled page"
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
        super::SEARCH_JS.contains("tali-search-kbd"),
        "search.js must target the .tali-search-kbd badge to localize it"
    );
}

#[test]
fn assembled_page_ships_focus_trap() {
    let page = render_html_page("# Title\n\n![alt](x.png)\n", "doc");
    // The shared modal focus-trap utility (lightbox / reader menu / Cmd-K) ships in the page.
    assert!(
        page.contains("taliFocusTrap"),
        "modal focus-trap utility not shipped in the assembled page"
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
    // sections a reader has scrolled through (`.tali-toc-read`) and persists them in the
    // reader's OWN localStorage (`qmd-read:<path>`). Reader-side, read-only.
    let toc_page = render_html_page(
        "---\ntitle: Doc\nformat:\n  html:\n    toc: true\n---\n\n# A\n\ntext\n\n## B\n\nmore\n",
        "fb",
    );
    assert!(
        toc_page.contains("tali-toc-read"),
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
    assert!(page.contains("--tali-bg: #16181d"), "dark vars missing");
    // The resolver threads the forced mode in as `var MODE = "dark"`; with no saved choice
    // its DEFAULT() returns that mode (an unspecified `MODE` would instead follow the OS).
    assert!(
        page.contains("var MODE = \"dark\""),
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
    let plain = render_document("---\nformat: deck\n---\n\n# A\n");
    assert!(!plain.theme_is_custom, "a plain deck has no custom theme");
    assert!(
        deck_theme_head(&plain.theme_default, plain.theme_is_custom).contains("taliDeckApplyTheme"),
        "a built-in-theme deck should get the theme head"
    );
    // A user `include-in-header` is not a theme extension, so it must not flip
    // the deck out of built-in light/dark management.
    let with_header = render_document(
        "---\nformat: deck\ninclude-in-header:\n  text: \"<meta name=x>\"\n---\n\n# A\n",
    );
    assert!(!with_header.theme_is_custom);
    // An explicit `theme: dark` forces dark and is still managed.
    assert_eq!(
        render_document("---\nformat: deck\ntheme: dark\n---\n\n# A\n").theme_default,
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
    assert!(
        page.contains("class=\"tali-fnref\""),
        "footnote ref: {page}"
    );
    assert!(page.contains("href=\"#fn-1\""), "ref links to def");
    // Definitions are gathered into one footnotes section (not rendered in place).
    assert!(page.contains("class=\"footnotes\""), "footnotes section");
    assert!(page.contains("id=\"fn-1\""), "footnote def id");
    assert!(page.contains("The supporting note"), "footnote body");
    assert!(page.contains("tali-fn-back"), "backlink to the reference");
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
    assert!(h.contains("class=\"tali-input\""), "wrapper: {h}");
    assert!(h.contains("data-qmd-input=\"k\""), "named input: {h}");
    assert!(h.contains("type=\"range\""), "range control: {h}");
    assert!(h.contains("min=\"1\"") && h.contains("max=\"10\"") && h.contains("value=\"3\""));
    assert!(
        h.contains("<output class=\"tali-input-out\" data-qmd-out>3</output>"),
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
    assert!(h.contains("class=\"tali-scrolly\""), "wrapper: {h}");
    assert!(
        h.contains("class=\"scrolly-steps\"") && h.contains("class=\"scrolly-stage\""),
        "split: {h}"
    );
    assert!(h.contains("data-scrolly-name=\"scene\""), "name attr: {h}");
    assert!(
        h.contains(
            "<input type=\"hidden\" class=\"tali-scrolly-input\" data-qmd-input=\"scene\" value=\"a\">"
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
    assert!(h.contains("class=\"tali-scrolly\""));
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
fn code_enhance_bundle_matches_fragments_in_order() {
    // CODE_ENHANCE_JS is concat!'d from the ordered per-feature fragments under
    // assets/js/code-enhance/ (no separators). Re-read them here, sorted by name
    // (the numeric prefix == load order), and assert the concatenation matches the
    // emitted const — so a new fragment that is not wired into the concat!, a
    // reordering, or a dropped include fails loudly instead of silently shipping a
    // broken/incomplete reader layer.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/js/code-enhance");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .expect("assets/js/code-enhance should exist")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "js"))
        .collect();
    paths.sort();
    let joined: String = paths
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect();
    assert_eq!(
        joined, CODE_ENHANCE_JS,
        "the code-enhance/ fragments (in filename order) must tile exactly into \
         CODE_ENHANCE_JS — update the concat! in mod.rs when adding/reordering fragments"
    );
}

#[test]
fn captioned_code_listing_is_a_figure_not_a_bare_div() {
    // A `<figcaption>` is only valid inside a `<figure>`; the numbered code listing must
    // wrap as `<figure class="tali-listing">` (valid HTML, and the same float semantics
    // Quarto uses for `lst-`). The `.tali-listing` margin already zeroes the UA figure
    // indent, so the element swap is style-neutral.
    let doc =
        render_document("```{python}\n#| label: lst-demo\n#| lst-cap: My listing\nx = 1\n```\n");
    let html: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
    assert!(
        html.contains("<figure") && html.contains("class=\"tali-listing\""),
        "the listing must wrap in a <figure class=\"tali-listing\">: {html}"
    );
    assert!(
        html.contains("class=\"tali-listing-caption\""),
        "the numbered caption must survive: {html}"
    );
    assert!(
        !html.contains("<div class=\"tali-listing\""),
        "the listing must no longer be a bare <div> (invalid figcaption): {html}"
    );
}

#[test]
fn build_mode_content_gates_separate_enhancers() {
    // Pure prose: a static build keeps code-enhance.js (the reader menu + a11y layer
    // that every page benefits from) but drops the DOM-specific enhancers it can't use.
    let prose = code_scripts_for("<p>Just prose.</p>", OutputMode::Build);
    assert!(
        prose.contains("taliInitReaderMenu"),
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
    // `#16181d` is the dark `--tali-bg`, present only in the dark layer.
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
        page.contains("taliInitReaderMenu"),
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

#[test]
fn theorem_div_emits_styled_block_with_number_slot() {
    let doc = render_document(
        "::: {.theorem #thm-pyth title=\"Pythagorean theorem\"}\n$a^2+b^2=c^2$.\n:::\n",
    );
    assert_eq!(doc.blocks.len(), 1, "the theorem is one container block");
    let h = &doc.blocks[0].html;
    assert!(
        h.contains("class=\"tali-theorem tali-theorem-theorem tali-thm-style-plain\""),
        "got: {h}"
    );
    assert!(h.contains("data-qmd-theorem-kind=\"theorem\""), "got: {h}");
    assert!(
        h.contains(" id=\"thm-pyth\""),
        "author anchor on container: {h}"
    );
    assert!(
        h.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;1</span></span>"
        ),
        "head carries the kind name + the number filled by the post-pass: {h}"
    );
    assert!(
        h.contains("<span class=\"tali-theorem-title\">(Pythagorean theorem)</span>"),
        "got: {h}"
    );
    // inner content keeps its own block id (click-to-source) and math is KaTeX-rendered
    assert!(h.contains("data-block-id"), "inner block lost id: {h}");
    assert!(
        h.contains("katex"),
        "math should render inside the theorem: {h}"
    );
}

#[test]
fn theorem_styles_map_kinds() {
    let d = render_document("::: {.definition}\nA set.\n:::\n");
    assert!(
        d.blocks[0]
            .html
            .contains("tali-theorem-definition tali-thm-style-definition"),
        "got: {}",
        d.blocks[0].html
    );
    assert!(
        d.blocks[0]
            .html
            .contains("<span class=\"tali-theorem-label\">Definition"),
        "got: {}",
        d.blocks[0].html
    );
    let r = render_document("::: {.remark}\nAside.\n:::\n");
    assert!(
        r.blocks[0].html.contains("tali-thm-style-remark"),
        "got: {}",
        r.blocks[0].html
    );
}

#[test]
fn proof_emits_qed_and_no_number_slot() {
    let p = render_document("::: {.proof}\nBy the diagram.\n:::\n");
    let h = &p.blocks[0].html;
    assert!(h.contains("class=\"tali-proof\""), "got: {h}");
    assert!(
        h.contains("<p class=\"tali-proof-head\">Proof.</p>"),
        "got: {h}"
    );
    assert!(
        h.contains("<span class=\"tali-qed\" aria-hidden=\"true\">\u{220e}</span>"),
        "got: {h}"
    );
    assert!(
        !h.contains("tali-theorem-number"),
        "proof is unnumbered: {h}"
    );

    let renamed = render_document("::: {.proof title=\"Proof of the main theorem\"}\nx.\n:::\n");
    assert!(
        renamed.blocks[0]
            .html
            .contains("<p class=\"tali-proof-head\">Proof of the main theorem.</p>"),
        "got: {}",
        renamed.blocks[0].html
    );
}

#[test]
fn theorems_number_continuously_per_kind() {
    let doc = render_document(
        "::: {.theorem}\nA.\n:::\n\n::: {.lemma}\nB.\n:::\n\n::: {.theorem}\nC.\n:::\n",
    );
    let body = doc.body_html();
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;1</span></span>"
        ),
        "first theorem is 1: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Lemma<span class=\"tali-theorem-number\">&nbsp;1</span></span>"
        ),
        "lemma counts independently: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;2</span></span>"
        ),
        "second theorem is 2: {body}"
    );
}

#[test]
fn theorem_crossref_resolves_with_label_and_number() {
    let doc = render_document(
        "See @thm-pyth and @lem-bound.\n\n::: {.theorem #thm-pyth}\nA.\n:::\n\n::: {.lemma #lem-bound}\nB.\n:::\n",
    );
    let body = doc.body_html();
    assert!(
        body.contains("<a href=\"#thm-pyth\" class=\"tali-xref\">Theorem&nbsp;1</a>"),
        "got: {body}"
    );
    assert!(
        body.contains("<a href=\"#lem-bound\" class=\"tali-xref\">Lemma&nbsp;1</a>"),
        "got: {body}"
    );
    assert!(!body.contains("@thm-pyth"), "ref left unresolved: {body}");
}

#[test]
fn proof_is_not_numbered() {
    let doc = render_document("::: {.proof}\nx.\n:::\n");
    assert!(
        !doc.body_html().contains("tali-theorem-number"),
        "proof has no number slot: {}",
        doc.body_html()
    );
}

#[test]
fn theorem_config_shared_group_shares_counter_key() {
    let cfg = parse_theorem_config("theorems:\n  shared: [theorem, lemma]\n");
    assert_eq!(
        cfg.counter_key("theorem"),
        cfg.counter_key("lemma"),
        "shared kinds collapse to one counter key"
    );
    assert_ne!(
        cfg.counter_key("theorem"),
        cfg.counter_key("definition"),
        "an unlisted kind keeps its own key"
    );
    let none = parse_theorem_config("title: x\n");
    assert_ne!(
        none.counter_key("theorem"),
        none.counter_key("lemma"),
        "no config means per-kind counters"
    );
}

#[test]
fn shared_counter_numbers_across_kinds() {
    let doc = render_document(
        "---\ntheorems:\n  shared: [theorem, lemma]\n---\n\n::: {.theorem}\nA.\n:::\n\n::: {.lemma}\nB.\n:::\n\n::: {.theorem}\nC.\n:::\n\n::: {.definition}\nD.\n:::\n",
    );
    let body = doc.body_html();
    // theorem + lemma draw one sequence: Theorem 1, Lemma 2, Theorem 3
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;1</span></span>"
        ),
        "got: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Lemma<span class=\"tali-theorem-number\">&nbsp;2</span></span>"
        ),
        "lemma takes the shared sequence's 2: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;3</span></span>"
        ),
        "got: {body}"
    );
    // definition is NOT shared: its own counter starts at 1
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Definition<span class=\"tali-theorem-number\">&nbsp;1</span></span>"
        ),
        "unlisted kind keeps its own counter: {body}"
    );
}

#[test]
fn theorem_config_parses_number_within_chapter() {
    let cfg = parse_theorem_config("theorems:\n  number-within: chapter\n");
    assert!(
        cfg.chapter_scoped(),
        "number-within: chapter sets chapter scoping"
    );
    let none = parse_theorem_config("theorems:\n  shared: [theorem]\n");
    assert!(
        !none.chapter_scoped(),
        "absent number-within is not chapter-scoped"
    );
}

#[test]
fn number_within_chapter_scopes_to_book_chapter() {
    let doc = render_document_with_includes_scoped(
        "---\ntheorems:\n  number-within: chapter\n---\n\n::: {.theorem #thm-a}\nA.\n:::\n\nSee @thm-a.\n\n::: {.theorem #thm-b}\nB.\n:::\n",
        std::path::Path::new("."),
        Some(2),
    );
    let body = doc.body_html();
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;2.1</span></span>"
        ),
        "first theorem in chapter 2 is 2.1: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;2.2</span></span>"
        ),
        "second is 2.2: {body}"
    );
    assert!(
        body.contains("<a href=\"#thm-a\" class=\"tali-xref\">Theorem&nbsp;2.1</a>"),
        "the in-page ref agrees with the chapter-scoped number: {body}"
    );
}

#[test]
fn number_within_chapter_falls_back_and_warns_without_a_chapter() {
    let doc = render_document(
        "---\ntheorems:\n  number-within: chapter\n---\n\n::: {.theorem}\nA.\n:::\n",
    );
    assert!(
        doc.body_html().contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;1</span></span>"
        ),
        "no chapter context falls back to continuous numbering: {}",
        doc.body_html()
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("number-within")),
        "a warning explains the no-op outside a book: {:?}",
        doc.warnings
    );
}

#[test]
fn numbered_false_suppresses_the_number() {
    let doc = render_document(
        "---\ntheorems:\n  numbered: false\n---\n\n::: {.theorem}\nA.\n:::\n\n::: {.theorem}\nB.\n:::\n",
    );
    let body = doc.body_html();
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\"></span></span>"
        ),
        "numbered: false leaves the number slot empty: {body}"
    );
    assert!(
        !body.contains("tali-theorem-number\">&nbsp;"),
        "no number is emitted anywhere: {body}"
    );
}

#[test]
fn numbered_unless_unique_numbers_only_repeated_kinds() {
    let doc = render_document(
        "---\ntheorems:\n  numbered: unless-unique\n---\n\n::: {.definition}\nLone.\n:::\n\n::: {.theorem}\nT1.\n:::\n\n::: {.theorem}\nT2.\n:::\n",
    );
    let body = doc.body_html();
    // definition appears once -> unnumbered
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Definition<span class=\"tali-theorem-number\"></span></span>"
        ),
        "a lone kind is unnumbered: {body}"
    );
    // theorem appears twice -> numbered 1, 2
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;1</span></span>"
        ) && body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;2</span></span>"
        ),
        "a repeated kind is numbered: {body}"
    );
}

#[test]
fn unnumbered_theorem_ref_resolves_to_bare_label_not_broken() {
    // An id'd but unnumbered theorem (numbered: false) is still a valid same-page ref
    // target: the ref resolves to a bare label and is NOT left as a broken marker.
    let doc = render_document(
        "---\ntheorems:\n  numbered: false\n---\n\n::: {.theorem #thm-x}\nA.\n:::\n\nSee @thm-x.\n",
    );
    let body = doc.body_html();
    assert!(
        body.contains("<a href=\"#thm-x\" class=\"tali-xref\">Theorem</a>"),
        "unnumbered theorem ref resolves to a bare label: {body}"
    );
    assert!(
        !body.contains("data-qmd-xref=\"thm-x\""),
        "the ref must not be left as a broken-ref marker: {body}"
    );
}

#[test]
fn proof_collapse_folds_into_details() {
    let closed = render_document("::: {.proof collapse=\"true\"}\nBody.\n:::\n");
    let h = &closed.blocks[0].html;
    assert!(
        h.contains("<div class=\"tali-proof tali-proof-collapse\"")
            && h.contains("<details><summary class=\"tali-proof-head\">Proof.</summary>"),
        "collapse=true folds the proof behind a closed <details>: {h}"
    );
    assert!(
        h.contains("<span class=\"tali-qed\" aria-hidden=\"true\">\u{220e}</span></details>"),
        "QED sits inside <details> (shown only when expanded): {h}"
    );
    let open = render_document("::: {.proof collapse=\"false\"}\nBody.\n:::\n");
    assert!(
        open.blocks[0].html.contains("<details open>"),
        "collapse=false starts open: {}",
        open.blocks[0].html
    );
    let plain = render_document("::: {.proof}\nBody.\n:::\n");
    assert!(
        !plain.blocks[0].html.contains("<details"),
        "a plain proof is not a <details>: {}",
        plain.blocks[0].html
    );
}

#[test]
fn strip_tags_is_quote_aware() {
    // A `>` inside a quoted attribute value must not end the tag early, else the
    // visible text truncates mid-attribute (KaTeX emits `title="…>…"` on inline math).
    assert_eq!(strip_tags(r#"<span title="a > b">hi</span>"#), "hi");
    assert_eq!(
        strip_tags(r#"<a href="x" title="p>q">text</a> tail"#),
        "text tail"
    );
    assert_eq!(strip_tags("<p>plain <em>text</em></p>"), "plain text");
}

#[test]
fn bibliography_paths_accepts_scalar_seq_and_spaced_path() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(
        bibliography_paths("bibliography: refs.bib"),
        s(&["refs.bib"])
    );
    // Quoted scalar with a space: the old space-split broke this into two tokens.
    assert_eq!(
        bibliography_paths("bibliography: \"my refs.bib\""),
        s(&["my refs.bib"])
    );
    assert_eq!(
        bibliography_paths("bibliography: [a.bib, b.bib]"),
        s(&["a.bib", "b.bib"])
    );
    assert_eq!(
        bibliography_paths("bibliography:\n  - a.bib\n  - b.bib"),
        s(&["a.bib", "b.bib"])
    );
    assert!(bibliography_paths("title: X").is_empty());
}
