//! Unit + corpus-invariant tests for the render module (split out of mod.rs
//! to keep mod.rs focused; `use super::*` reaches the render internals).

use super::*;

#[test]
fn humanize_date_formats_iso_and_passes_through_everything_else() {
    assert_eq!(humanize_date("2026-04-14"), "14 April 2026"); // day un-padded
    assert_eq!(humanize_date("2026-05-01"), "1 May 2026");
    assert_eq!(humanize_date("2026-12-31"), "31 December 2026");
    // Non-ISO values are shown verbatim, never mangled.
    assert_eq!(humanize_date("Spring 2026"), "Spring 2026");
    assert_eq!(humanize_date("2026-13-01"), "2026-13-01"); // bad month
    assert_eq!(humanize_date("2026-04-14T09:00"), "2026-04-14T09:00"); // carries a time
    assert_eq!(humanize_date("26-4-14"), "26-4-14"); // not a 4-digit year
}

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
        h.contains("<span>A</span>") && h.contains("<span>15 May 2026</span>"),
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
fn tabset_label_does_not_double_escape_entities() {
    // A tab label sourced via `strip_tags` is ALREADY HTML-safe text (`&` -> `&amp;`
    // in the rendered heading); layering `html_escape` on top produced `&amp;amp;`.
    let src = "::: {.panel-tabset}\n\n\
        ## Q&A\n\nBody one.\n\n\
        ## R\n\nBody two.\n\n\
        :::\n";
    let doc = render_document(src);
    let h = &doc.blocks[0].html;
    assert!(h.contains(">Q&amp;A</button>"), "label under-escaped: {h}");
    assert!(!h.contains("&amp;amp;"), "label double-escaped: {h}");
}

#[test]
fn toc_does_not_double_escape_entities() {
    // TOC entry text also comes from `strip_tags` (already-escaped); it must not be
    // html_escape'd again. A heading `Tips & tricks: x < y` renders its `&`/`<` as
    // entities, which must survive as `&amp;`/`&lt;` (not `&amp;amp;`/`&amp;lt;`).
    let doc = render_document("## Tips & tricks: x < y\n\nBody.\n");
    let toc = toc_html(&doc.blocks);
    assert!(
        toc.contains("Tips &amp; tricks: x &lt; y"),
        "TOC entry wrong: {toc}"
    );
    assert!(!toc.contains("&amp;amp;"), "TOC double-escaped `&`: {toc}");
    assert!(!toc.contains("&amp;lt;"), "TOC double-escaped `<`: {toc}");
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
    // The parser tolerates whitespace between the comment marker and the pipe (`# |`,
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

/// A LEADING DOT means display-only: `{.python}` is "the deck's display form for a
/// non-executing block" (docs/guide/using/formats.tmd). Only bare `{python}` executes.
/// The cell gate used to test `starts_with('{')` alone, and `code_lang` strips the dot,
/// so `{.python code-line-numbers="1|2-3"}` became an executable cell — it warmed a
/// kernel and spliced an output block under an illustrative snippet. `corpus/deck.tmd`
/// authors exactly that shape over an undefined `values`, so a live kernel baked a real
/// NameError traceback into a slide. Invisible to the kernel-free corpus tests, which
/// only assert the static highlight markup.
#[test]
fn a_leading_dot_fence_is_display_only_and_never_executes() {
    let doc = render_document("```{.python code-line-numbers=\"1|2-3\"}\ntotal = 0\n```\n");
    let b = &doc.blocks[0];
    assert!(
        b.cell.is_none(),
        "a `{{.python}}` fence is display-only and must not become an executable cell"
    );
    // It must still render AS python: the dot only suppresses execution, not highlighting.
    assert!(
        b.html.contains("qhl-") || b.html.contains("language-python"),
        "the dot form must still be syntax-highlighted as python: {}",
        b.html
    );
    assert!(
        b.html.contains("data-code-lines"),
        "and must keep its code-line-numbers stepping: {}",
        b.html
    );
    // The bare form is unaffected: it is the executable one.
    let exec = render_document("```{python}\ntotal = 0\n```\n");
    assert!(
        exec.blocks[0].cell.is_some(),
        "a bare `{{python}}` fence is still an executable cell"
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
fn inline_math_heading_gets_a_clean_slug() {
    // `$…$` must not leak LaTeX into the anchor id (was `…-h-0`, `the-12-n-n-1-…`).
    // The math span contributes nothing to the slug; the surrounding text stands alone.
    let cases = [
        (
            "## Expected rank mean under $H_0$\n",
            "expected-rank-mean-under",
        ),
        ("## The $12/N(N+1)$ scaling factor\n", "the-scaling-factor"),
        ("# Deriving the $H$ statistic\n", "deriving-the-statistic"),
    ];
    for (src, want) in cases {
        let doc = render_document(src);
        let h = &doc.blocks[0].html;
        assert!(
            h.contains(&format!("id=\"{want}\"")),
            "want id={want:?}, got heading: {h}"
        );
    }
    // A lone/currency `$` is NOT a math delimiter (comrak leaves it as text), so it must
    // survive in the slug rather than being stripped as a math span.
    let money = render_document("## Save $5 on every $10 spent\n");
    assert!(
        money.blocks[0]
            .html
            .contains("id=\"save-5-on-every-10-spent\""),
        "currency wrongly stripped from slug: {}",
        money.blocks[0].html
    );
}

#[test]
fn heading_slug_respects_comrak_math_boundaries() {
    // The slug stripper must match comrak's `math_dollars` span detection exactly, or it
    // silently deletes literal words from a (load-bearing) anchor id. comrak abandons an
    // opening `$` when the next unescaped `$` is preceded by whitespace or followed by a
    // digit — it does NOT reach for a later `$`. Each case is the id comrak's own parse
    // yields (verified: `$n$` is the only math span in the first, none in the next two).
    let cases = [
        // Only `$n$` is math; `$5 or more` is literal → keep "5 or more".
        ("## Costs $5 or more $n$ items\n", "costs-5-or-more-items"),
        // Every close is digit-followed or space-preceded → whole line literal.
        ("## Total: $5+$10 = $15\n", "total-5-10-15"),
        // `$O(n)$` close is followed by `2` (digit) → not math → whole line literal.
        ("## The $O(n)$2x speedup\n", "the-o-n-2x-speedup"),
        // First `$` abandoned (space before the next `$`); only `$y$` is math.
        ("## $x $y$\n", "x"),
        // Display `$$…$$` may have whitespace after the opening `$$`.
        ("## Cost $$ x $$ table\n", "cost-table"),
    ];
    for (src, want) in cases {
        let doc = render_document(src);
        let h = &doc.blocks[0].html;
        assert!(
            h.contains(&format!("id=\"{want}\"")),
            "want id={want:?}, got heading: {h}"
        );
    }
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
    // ```{=html}``` is Pandoc raw-passthrough: its body is emitted
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
fn dangerous_url_schemes_are_neutralized() {
    // Taliesin renders comrak's AST with raw-HTML passthrough, which also bypasses
    // comrak's safe-mode URL sanitizer, so the markdown link/image path must reject
    // script-bearing schemes itself. The trusted author can still use raw HTML; this
    // is the safe default for any not-fully-authored markdown (a third-party README,
    // an `{{< include >}}`d fragment, a future multi-author surface).
    let link = render_document("[click](javascript:alert)\n");
    let lh = &link.blocks[0].html;
    assert!(
        !lh.contains("javascript:"),
        "javascript: URL leaked into a link href: {lh}"
    );

    let img = render_document("![x](vbscript:evil)\n");
    let ih = &img.blocks[0].html;
    assert!(
        !ih.contains("vbscript:"),
        "vbscript: URL leaked into an img src: {ih}"
    );

    // `data:text/html` is an XSS vector in a link; an inline `data:image` is a
    // legitimate image, so it is allowed in the image context only.
    let data_link = render_document("[x](data:text/html;base64,PHNjcmlwdD4=)\n");
    assert!(
        !data_link.blocks[0].html.contains("data:text/html"),
        "data:text/html leaked into a link href: {}",
        data_link.blocks[0].html
    );
    let data_img = render_document("![x](data:image/png;base64,iVBORw0KGgo=)\n");
    assert!(
        data_img.blocks[0]
            .html
            .contains("data:image/png;base64,iVBORw0KGgo="),
        "a legitimate inline data:image was dropped: {}",
        data_img.blocks[0].html
    );

    // Ordinary schemes, relative paths, and fragments are untouched.
    let ok = render_document("[a](https://ex.com) [b](/rel) [c](#frag) [d](mailto:x@y.z)\n");
    let oh = &ok.blocks[0].html;
    assert!(
        oh.contains("href=\"https://ex.com\""),
        "https dropped: {oh}"
    );
    assert!(oh.contains("href=\"/rel\""), "relative path dropped: {oh}");
    assert!(oh.contains("href=\"#frag\""), "fragment dropped: {oh}");
    assert!(oh.contains("href=\"mailto:x@y.z\""), "mailto dropped: {oh}");
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
fn built_deck_with_mermaid_inlines_the_library_offline() {
    // A built deck (OutputMode::Build) must not breach the offline-build contract:
    // a Mermaid diagram should ship the vendored library inlined (globalThis.mermaid
    // set), exactly like the HTML page path, so the browser never actually reaches
    // the CDN fallback baked into the loader. deck_page_from_doc used to hardcode
    // OutputMode::Preview regardless of the caller's mode, so a built deck never
    // inlined the library and the CDN fetch was live.
    let src = "---\nformat: deck\n---\n\n## A\n\n```mermaid\nflowchart LR\n  A --> B\n```\n";
    let doc = render_document(src);
    assert_eq!(doc.format, DocFormat::Reveal);
    let build = render_doc_to_page(&doc, "t", OutputMode::Build);
    assert!(
        build.contains("__esbuild_esm_mermaid") && build.contains("globalThis.mermaid"),
        "built deck must inline the vendored mermaid library"
    );
    // Preview keeps the lean lazy loader (dev-time network is fine).
    let preview = render_doc_to_page(&doc, "t", OutputMode::Preview);
    assert!(
        !preview.contains("__esbuild_esm_mermaid"),
        "preview deck should not inline the 2.5 MB mermaid library"
    );
}

#[test]
fn revealjs_format_is_no_longer_a_deck() {
    // `format: revealjs` was the deprecated legacy spelling; after shedding it, a
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
    // A `. . .` line is a pause: the marker itself is dropped, and every block
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
fn deck_viewport_allows_pinch_zoom() {
    // B5-1 (WCAG 1.4.4/1.4.10, pairs with the A3 mobile feed): a deck is a reading
    // surface — especially the phone feed — so its viewport must not lock the scale.
    // Keep width=device-width + initial-scale; drop maximum-scale / user-scalable so
    // pinch-zoom works.
    let page = render_html_page("---\nformat: deck\n---\n\n## Slide\n", "fallback");
    let vp = page
        .split("name=\"viewport\"")
        .nth(1)
        .and_then(|s| s.split("/>").next())
        .expect("deck page must carry a viewport meta");
    assert!(vp.contains("width=device-width"), "viewport: {vp}");
    assert!(
        !vp.contains("user-scalable=no"),
        "deck must allow pinch-zoom (no user-scalable=no): {vp}"
    );
    assert!(
        !vp.contains("maximum-scale"),
        "deck must not cap zoom (no maximum-scale): {vp}"
    );
}

#[test]
fn deck_opens_as_a_deck_without_reader_or_pdf_export() {
    // The deck redesign (A1/A2) removed reader/scroll mode and PDF-export mode: a
    // deck opens AS a deck (stepped), and a stray Cmd/Ctrl+P is handled by a minimal
    // `@media print` fallback rather than a bespoke flatten-to-PDF `tali-print` mode.
    // Pin at the bundle level so the machinery can't creep back in; the runtime
    // front-door behavior is covered by the ui-audit deck smoke.
    let page = render_html_page("---\nformat: deck\n---\n\n## A\n\n## B\n", "fallback");
    for gone in [
        "enterScroll",
        "exitScroll",
        "enterPrint",
        "exitPrint",
        "tali-scroll-stack",
        "tali-print-stack",
        "Reader mode",
        "Export PDF",
    ] {
        assert!(
            !page.contains(gone),
            "deck still bundles removed reader/PDF machinery: {gone}"
        );
    }
    // A minimal print fallback survives so a stray Cmd/Ctrl+P stays legible.
    assert!(
        page.contains("@media print"),
        "deck must keep a minimal @media print fallback"
    );
}

#[test]
fn deck_bundles_the_mobile_feed() {
    // A3: on a phone / portrait screen a deck opens as a vertical scroll-feed of full-
    // viewport slides (routed by aspect, with `?qmd=feed` / `?qmd=present` escape hatches).
    // Pin the machinery at the bundle level so it can't silently regress; the runtime
    // front-door behavior is covered by the ui-audit deck browser smoke.
    let page = render_html_page("---\nformat: deck\n---\n\n## A\n\n## B\n", "fallback");
    // CSS: the feed layout (a scroll-snap container gated on html.tali-feed).
    assert!(
        page.contains(".tali-feed"),
        "deck must bundle the mobile-feed CSS"
    );
    assert!(
        page.contains("scroll-snap-type: y mandatory"),
        "feed must use CSS scroll-snap"
    );
    // JS: the feed entry path + aspect routing.
    assert!(
        page.contains("function enterFeed"),
        "deck.js must bundle the feed entry path"
    );
    assert!(
        page.contains("function isPortrait"),
        "deck.js must route the front door by aspect"
    );
    assert!(
        page.contains("qmd === 'feed'"),
        "deck.js must honour the ?qmd=feed escape hatch"
    );
}

#[test]
fn paused_plain_code_block_is_a_fragment_without_line_steps() {
    // B0-1: a `. . .` pause before a PLAIN code block (no code-line-numbers) stamps the
    // `<pre>` with class="fragment" but leaves it WITHOUT data-code-lines — the shape the
    // deck engine must tolerate (fragsOf used to run `null.split('|')` here and wedge nav).
    let doc = render_document(
        "---\nformat: deck\n---\n\n## S\n\nIntro.\n\n. . .\n\n```python\ndef f():\n    return 1\n```\n",
    );
    let slides = slides_html(None, None, &doc.blocks);
    // Find the paused <pre>'s opening tag.
    let pre = slides
        .split("<pre")
        .find(|seg| seg.starts_with(' ') && seg[..seg.find('>').unwrap_or(0)].contains("fragment"))
        .expect("the paused plain code block should render a <pre class=\"fragment\">");
    let open = &pre[..pre.find('>').unwrap()];
    assert!(open.contains("class=\"fragment\""), "got: {open}");
    assert!(
        !open.contains("data-code-lines"),
        "a plain code block must carry no line-step spec: {open}"
    );
}

#[test]
fn deck_title_slide_id_does_not_collide_with_a_slide_titled_title_slide() {
    // B2-10: the front-matter title slide hardcodes id="title-slide"; a content slide
    // literally titled "Title Slide" slugs to the same id -> two #title-slide in the DOM
    // (getElementById/#hash target the wrong section). The injected id must be reserved
    // in the dedup map so the colliding heading becomes title-slide-1.
    let doc = render_document("---\ntitle: Deck\nformat: deck\n---\n\n## Title Slide\n\nHi.\n");
    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    assert_eq!(
        slides.matches("id=\"title-slide\"").count(),
        1,
        "duplicate #title-slide: {slides}"
    );
    assert!(
        slides.contains("id=\"title-slide-1\""),
        "colliding heading should dedup to title-slide-1: {slides}"
    );
}

#[test]
fn deck_explicit_slide_id_is_kept_verbatim_not_slugified() {
    // B2-11: an author `{#id}` on a slide heading becomes the <section> anchor VERBATIM
    // so `@sec-…`/`#hash` resolve; only the heading-text fallback is slugged. Today the
    // deck path slugs both, so `## Two {#sec-My_Two}` emits id="sec-my-two" while the
    // xref href stays "#sec-My_Two" -> dead link.
    let doc = render_document(
        "---\nformat: deck\n---\n\n## Intro\n\nSee @sec-My_Two.\n\n## Two {#sec-My_Two}\n\nBody.\n",
    );
    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    assert!(
        slides.contains("<section id=\"sec-My_Two\""),
        "explicit slide id was slugified instead of kept verbatim: {slides}"
    );
    assert!(
        slides.contains("href=\"#sec-My_Two\""),
        "xref href drifted from the section id: {slides}"
    );
}

#[test]
fn deck_explicit_slide_id_with_special_chars_is_escaped_once() {
    // The explicit {#id} rides in as an HTML-attr-escaped data-slide-anchor; split_slides
    // must unescape before storing so render_section escapes exactly once — otherwise an
    // id with & < > double-escapes (id="a&amp;amp;b") and its @ref/#hash goes dead.
    let doc = render_document("---\ntitle: D\nformat: deck\n---\n\n## Two {#a&b}\n\nBody.\n");
    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    assert!(
        slides.contains("id=\"a&amp;b\""),
        "explicit id should be escaped exactly once: {slides}"
    );
    assert!(
        !slides.contains("a&amp;amp;b"),
        "explicit id double-escaped: {slides}"
    );
}

#[test]
fn editing_a_post_pause_block_keeps_its_fragment_in_the_update() {
    use crate::{BlockOp, diff_blocks};
    // B3-14: a within-slide edit of a post-`. . .` block must ship slide-transformed html,
    // or the `.fragment` class is stripped and the block becomes permanently visible. The
    // live deck diff runs on `deck_slide_blocks` (the projection), not the raw block list.
    let before =
        render_document("---\nformat: deck\n---\n\n## S\n\nIntro.\n\n. . .\n\nAfter the pause.\n");
    let after = render_document(
        "---\nformat: deck\n---\n\n## S\n\nIntro.\n\n. . .\n\nAfter the pause, edited.\n",
    );

    // The RAW diff strips the fragment — this is the bug the projection fixes.
    let raw = diff_blocks(&before.blocks, &after.blocks);
    let raw_update = raw
        .iter()
        .find_map(|op| match op {
            BlockOp::Update { html, .. } => Some(html.clone()),
            _ => None,
        })
        .expect("editing the post-pause block yields an Update");
    assert!(
        !raw_update.contains("class=\"fragment\""),
        "raw update already had .fragment, test premise stale: {raw_update}"
    );

    // The slide-transformed projection keeps it.
    let ops = diff_blocks(
        &deck_slide_blocks(&before.blocks),
        &deck_slide_blocks(&after.blocks),
    );
    let update = ops
        .iter()
        .find_map(|op| match op {
            BlockOp::Update { html, .. } => Some(html.clone()),
            _ => None,
        })
        .expect("projection diff yields an Update for the edited post-pause block");
    assert!(
        update.contains("class=\"fragment\""),
        "projection Update must carry .fragment so the block stays a step: {update}"
    );
    assert!(update.contains("After the pause, edited."), "got: {update}");
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
fn figure_with_dark_attr_emits_a_theme_swapped_image_pair() {
    // A `dark=` source ships a light + dark <img> pair (like `{{< video dark= >}}`); CSS
    // shows the one matching html[data-theme]. Both variants carry the shared alt + dims.
    let doc =
        render_document("![A model fit.](fit-b.png){#fig-fit dark=\"fit-c.png\" width=60%}\n");
    let h = &doc.blocks[0].html;
    assert!(
        h.contains("<img class=\"tali-img-light\" src=\"fit-b.png\""),
        "light variant: {h}"
    );
    assert!(
        h.contains("<img class=\"tali-img-dark\" src=\"fit-c.png\""),
        "dark variant: {h}"
    );
    assert_eq!(
        h.matches("alt=\"A model fit.\"").count(),
        2,
        "alt on both: {h}"
    );
    assert_eq!(
        h.matches("style=\"width:60%\"").count(),
        2,
        "width on both: {h}"
    );
    // No `dark=` → a single <img>, unchanged (no variant classes).
    let plain = render_document("![Plain.](p.png){#fig-p}\n");
    assert!(
        !plain.blocks[0].html.contains("tali-img-"),
        "no variant class without dark=: {}",
        plain.blocks[0].html
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
fn theme_head_separates_the_reader_choice_from_the_resolved_mode() {
    // A reader who once toggled could never return to following the OS: the only saved
    // values were light/dark/sepia, and nothing cleared the key. The head script must
    // therefore expose the RAW choice (which can be "auto") alongside the resolved mode
    // (which never is), and selecting "auto" must CLEAR the key so `hasSaved()` goes false
    // and the `prefers-color-scheme` listener resumes driving the page.
    let head = theme_head("auto");
    assert!(
        head.contains("taliGetThemeChoice"),
        "head script must expose the raw reader choice, not just the resolved mode"
    );
    assert!(
        head.contains("removeItem"),
        "choosing `auto` must clear qmd-theme, not store an unrecognized value"
    );
    // The change event has to carry the choice too, or the picker cannot re-sync
    // its pressed state after an OS flip.
    assert!(
        head.contains("choice: choice()"),
        "qmd:themechange must report the choice alongside the mode"
    );
}

#[test]
fn reader_theme_picker_offers_auto_and_syncs_on_the_choice() {
    // The picker's segmented control marks the pressed option by comparing each option's
    // value against the current one. Comparing against the RESOLVED mode means an "Auto"
    // button can never read as pressed (the mode is always light/dark/sepia), so it must
    // compare against the stored choice.
    assert!(
        CODE_ENHANCE_JS.contains("['auto', 'Auto'"),
        "the Theme row must offer an Auto (follow the OS) option"
    );
    assert!(
        CODE_ENHANCE_JS.contains("taliGetThemeChoice"),
        "the Theme row must sync its pressed state against the stored choice, not the resolved mode"
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
    // Top-level `toc:` — the form the guide teaches. (This fixture used the Quarto
    // `format: html: toc:` shape, which only worked because `detect_toc` trimmed before
    // matching; that scan is top-level-only now, so the nested form no longer applies.)
    let page = render_html_page(
        "---\ntitle: Doc\ntoc: true\n---\n\n# A\n\ntext\n\n## B\n",
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
        "---\ntitle: Doc\ntoc: true\n---\n\n# A\n\ntext\n\n## B\n\nmore\n",
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
        std::path::Path::new("/taliesin-nonexistent-dir"),
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
    // A bare theme name (a possible built-in theme) must NOT warn.
    let ok = render_document_with_includes(
        "---\ntitle: X\ntheme: darkly\n---\n\ntext\n",
        std::path::Path::new("/taliesin-nonexistent-dir"),
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
    // The YAML-1.1 boolean words (which serde reads as strings) must coerce too, so
    // `toc: yes` doesn't silently no-op into the inherited site default.
    assert_eq!(detect_toc("toc: yes\n"), Some(true));
    assert_eq!(detect_toc("toc: on\n"), Some(true));
    assert_eq!(detect_toc("toc: no\n"), Some(false));
    assert_eq!(detect_toc("toc: OFF\n"), Some(false));
    // `toc-depth:`/`toc-title:` are not the `toc:` key and must not match.
    assert_eq!(detect_toc("toc-depth: 2\ntoc-title: Contents\n"), None);
}

/// `toc:` is a TOP-LEVEL key. An indented `toc:` is some other block's sub-key and must
/// not reach through — this scan used to trim every line before matching, so a `toc:`
/// nested under ANY block set the document's TOC: `hero:`/`listing:`/`execute:` alike,
/// none of which own a `toc`. `extract_field`/`detect_format` already skip indented
/// lines ("top-level keys only"); this brings the tristate scan in line with them.
#[test]
fn detect_toc_reads_only_a_top_level_key() {
    assert_eq!(
        detect_toc("title: X\nhero:\n  headline: Hi\n  toc: true\n"),
        None,
        "a `toc:` nested under `hero:` is not the document's toc"
    );
    assert_eq!(
        detect_toc("title: X\nformat:\n  html:\n    toc: true\n"),
        None,
        "a `toc:` nested under `format:` is not the document's toc (`format:` sub-keys are inert)"
    );
    // The top-level key still wins from anywhere in the block, including after a nested one.
    assert_eq!(
        detect_toc("format:\n  html:\n    toc: false\ntoc: true\n"),
        Some(true),
        "the top-level key is the only one read"
    );
}

/// Same rule for `title-block-style:`: the other scan that trimmed before matching, so a
/// nested `title-block-style: none` silently suppressed the title block.
#[test]
fn detect_title_block_hidden_reads_only_a_top_level_key() {
    assert!(detect_title_block_hidden("title-block-style: none\n"));
    assert!(
        !detect_title_block_hidden("format:\n  html:\n    title-block-style: none\n"),
        "a nested title-block-style is a sub-key, not the document's"
    );
}

#[test]
fn yaml_11_boolean_words_coerce_on_cell_and_execute_flags() {
    // `#| echo: no` / `execute: {echo: off}` are STRINGS in YAML 1.2; without
    // coercion they read as truthy and the cell echoes anyway (a silent no-op).
    assert!(!cell_flag_or("#| echo: no\n1", "echo", true));
    assert!(!cell_flag_or("#| echo: off\n1", "echo", true));
    assert!(!cell_flag_or("#| echo: false\n1", "echo", true));
    assert!(cell_flag_or("#| echo: yes\n1", "echo", false));
    // A non-boolean value (`echo: fenced`) still counts as "shown".
    assert!(cell_flag_or("#| echo: fenced\n1", "echo", false));
    // Unset falls back to the document default.
    assert!(cell_flag_or("1 + 1", "echo", true));
    assert!(!cell_flag_or("1 + 1", "echo", false));

    // Document-level `execute:` defaults, both flow and block form.
    assert_eq!(
        detect_execute_defaults("execute: {echo: no, cache: off}\n"),
        (false, true, false)
    );
    assert_eq!(
        detect_execute_defaults("execute:\n  echo: off\n  include: no\n"),
        (false, false, true)
    );
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
    // B4-22: an embedded deck follows the host page's data-theme. A `sepia` host is a
    // light reading surface, so hostTheme() must map it to 'light' (not fall through to
    // the OS, which could turn the deck dark inside a cream page).
    let head = deck_theme_head("auto", false);
    assert!(
        head.contains("t==='sepia' ? 'light'"),
        "hostTheme() should map a sepia host to a light deck"
    );
}

#[test]
fn theme_list_takes_first_entry() {
    // `theme: [dark, custom.scss]` (list form) selects the base.
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
fn footnote_li_is_the_locatable_unit_for_click_to_source() {
    // A gathered footnotes section collects notes from MANY, non-contiguous source
    // lines, so no single block-level sourcepos can point at "the" note. The locatable
    // unit is therefore each `<li>`: it carries its own definition's `data-sourcepos`
    // plus a `data-block-id`, because client.js `locatable()` resolves an Alt-click via
    // `closest("[data-qmd-src], [data-block-id]")` — a `data-sourcepos` alone would be
    // walked past, landing on the section (and, with no sourcepos there, on line 1).
    // Definitions sit on lines 7 and 11, scattered between prose rather than bunched
    // at the end, so a first-note-wins block sourcepos could not serve both.
    let src = "---\ntitle: T\n---\n\nFirst claim.[^a]\n\n[^a]: Note A.\n\nSecond claim.[^b]\n\n[^b]: Note B.\n";
    let doc = render_document(src);
    let fns = doc
        .blocks
        .iter()
        .find(|b| b.id == "qmd-footnotes")
        .expect("gathered footnotes block");

    // Each note resolves to the line its OWN definition sits on (7 and 11), not to
    // the first note's line and not to line 1.
    assert!(
        fns.html
            .contains("<li id=\"fn-a\" data-block-id=\"fn-a\" data-sourcepos=\"7:1-7:13\""),
        "note A must carry its own block-id + sourcepos: {}",
        fns.html
    );
    assert!(
        fns.html
            .contains("<li id=\"fn-b\" data-block-id=\"fn-b\" data-sourcepos=\"11:1-11:13\""),
        "note B must carry its own block-id + sourcepos: {}",
        fns.html
    );
}

#[test]
fn gathered_footnotes_block_keeps_an_empty_block_level_sourcepos() {
    // The section is a GATHERED container: comrak moves every definition to the
    // document end, so the block sits last while its content comes from wherever the
    // author wrote it. A block-level sourcepos would be doubly wrong:
    //   1. it would break the monotonic source-order invariant that tests/corpus.rs
    //      asserts ("blocks out of order"), since a note defined mid-document would
    //      give the last block an earlier line than the block before it; and
    //   2. a span from the first to the last note would swallow blank lines across
    //      the whole document, so reverse cursor-sync (`highlightAtLine` picks the
    //      SMALLEST covering range) would yank the cursor to the endnotes.
    // The real positions live on the `<li>`s instead; see the test above.
    let doc = render_document("Claim.[^a]\n\n[^a]: Note A.\n");
    let fns = doc
        .blocks
        .iter()
        .find(|b| b.id == "qmd-footnotes")
        .expect("gathered footnotes block");
    assert!(
        fns.sourcepos.is_empty(),
        "gathered block must not claim one source range, got {:?}",
        fns.sourcepos
    );
    // It still carries data-block-id: the diff addresses it by id (client.js `blockEl`).
    assert!(fns.html.contains("data-block-id=\"qmd-footnotes\""));
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
fn embed_shortcode_renders_isolating_deck_iframe() {
    // `{{< embed slides.tmd >}}` embeds another document's deck view in an isolating
    // iframe. The source path is mapped to its BUILT output (`.tmd` -> `.html`, because
    // the deck is built beside the embedding page), a default accessible name is
    // supplied, and the frame carries the fullscreen + open-in-new-tab affordances.
    let doc =
        render_document_with_includes("{{< embed slides.tmd >}}\n", std::path::Path::new("."));
    let h = doc.body_html();
    assert!(h.contains("class=\"tali-embed\""), "wrapper: {h}");
    assert!(
        h.contains("<iframe class=\"tali-embed-frame\" src=\"slides.html\""),
        "the `.tmd` source maps to its built `.html` output: {h}"
    );
    assert!(
        h.contains("title=\"Embedded slide deck\""),
        "default accessible name: {h}"
    );
    assert!(h.contains("allowfullscreen"), "fullscreen affordance: {h}");
    assert!(
        h.contains("href=\"slides.html\"") && h.contains("target=\"_blank\""),
        "open-in-new-tab link points at the same built deck: {h}"
    );
}

#[test]
fn embed_title_overrides_default_and_is_attribute_escaped() {
    // A `title="…"` names the iframe; the bare token is still the deck path (a `key=value`
    // named arg is never mistaken for it). The title is attribute-escaped so a `&` (or a
    // `"`) can't break out of the double-quoted attribute.
    let doc = render_document_with_includes(
        "{{< embed slides.tmd title=\"Q & A session\" >}}\n",
        std::path::Path::new("."),
    );
    let h = doc.body_html();
    assert!(
        h.contains("src=\"slides.html\""),
        "the named `title=` arg is not mistaken for the deck path: {h}"
    );
    assert!(
        h.contains("title=\"Q &amp; A session\""),
        "custom title, attribute-escaped: {h}"
    );
    assert!(
        !h.contains("Embedded slide deck"),
        "default title is replaced: {h}"
    );
}

#[test]
fn embed_targets_collects_in_order_dedups_and_skips_code_examples() {
    // The build/preview uses `embed_targets` to also build each referenced deck. It must
    // return paths in document order, deduped, and must skip a `{{< embed >}}` shown as an
    // *example* inside inline or fenced code (which stays literal, never a real dependency).
    let src = "\
{{< embed a.tmd >}}\n\
{{< embed b.tmd >}}\n\
{{< embed a.tmd >}}\n\
An inline example `{{< embed inline.tmd >}}` stays literal.\n\
```\n\
{{< embed fenced.tmd >}}\n\
```\n";
    assert_eq!(
        embed_targets(src),
        vec!["a.tmd".to_string(), "b.tmd".to_string()],
        "order preserved, a.tmd deduped, code examples skipped"
    );
}

#[test]
fn video_shortcode_emits_a_framed_autoplaying_screencast() {
    // `{{< video clip.mp4 >}}` — a silent, autoplaying, looping screencast authored in
    // Markdown so a page needs no raw `<video>` HTML. A single source has no light/dark split.
    let doc = render_document_with_includes("{{< video clip.mp4 >}}\n", std::path::Path::new("."));
    let h = doc.body_html();
    assert!(h.contains("<figure class=\"tali-video\""), "frame: {h}");
    assert!(h.contains("src=\"clip.mp4\""), "source: {h}");
    assert!(
        h.contains("autoplay")
            && h.contains("muted")
            && h.contains("loop")
            && h.contains("playsinline"),
        "silent autoplay loop: {h}"
    );
    assert!(
        !h.contains("tali-video-light"),
        "a single source has no theme split: {h}"
    );
}

#[test]
fn video_dark_and_poster_and_caption_args_are_exercised() {
    // C2: `dark=` ships a light + dark clip (CSS shows the one matching `html[data-theme]`);
    // `poster=` lands on the frame; `caption=` becomes the figcaption. None had corpus or
    // unit coverage.
    let doc = render_document_with_includes(
        "{{< video clip.mp4 dark=clip-dark.mp4 poster=poster.png caption=\"A demo\" >}}\n",
        std::path::Path::new("."),
    );
    let h = doc.body_html();
    assert!(
        h.contains("<video class=\"tali-video-light\" src=\"clip.mp4\""),
        "light clip: {h}"
    );
    assert!(
        h.contains("<video class=\"tali-video-dark\" src=\"clip-dark.mp4\""),
        "dark clip: {h}"
    );
    assert!(h.contains("poster=\"poster.png\""), "poster attr: {h}");
    assert!(
        h.contains("<figcaption>A demo</figcaption>"),
        "caption: {h}"
    );
}

#[test]
fn a_duplicate_cross_reference_label_warning_is_located() {
    // A repeated `{#sec-x}`/`{#fig-x}`/`{#tbl-x}` is a duplicate cross-reference label.
    // The warning must carry the DUPLICATE's source line for click-to-source — like the
    // "duplicate heading id" warning right beside it already does — not the unlocated
    // string that half-reproduces the Quarto flaw D53 critiques (§2 #1).
    let doc = render_document("# First {#sec-dup}\n\n## Second {#sec-dup}\n");
    let w = doc
        .warnings
        .iter()
        .find(|w| {
            w.message.contains("duplicate cross-reference label") && w.message.contains("sec-dup")
        })
        .expect("a duplicate cross-reference label warning");
    // The duplicate (second) heading is on line 3.
    assert_eq!(
        w.line,
        Some(3),
        "located at the duplicate's line, got: {w:?}"
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
    // used for `lst-`). The `.tali-listing` margin already zeroes the UA figure
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
fn nested_theorem_is_numbered_and_referenceable() {
    // A `::: {.theorem}` nested inside another fenced div collapses into the parent
    // block, so the number/xref post-pass must look inside blocks, not just at each
    // block's opening tag. Document order here is A = 1, nested = 2, B = 3.
    let doc = render_document(
        "::: {.theorem #thm-a}\nA.\n:::\n\n::: {.column-margin}\n::: {.theorem #thm-nested}\nN.\n:::\n:::\n\n::: {.theorem #thm-b}\nB.\n:::\n\nSee @thm-a, @thm-nested, and @thm-b.\n",
    );
    let body = doc.body_html();
    assert!(
        body.contains("<a href=\"#thm-a\" class=\"tali-xref\">Theorem&nbsp;1</a>"),
        "thm-a is 1: {body}"
    );
    assert!(
        body.contains("<a href=\"#thm-nested\" class=\"tali-xref\">Theorem&nbsp;2</a>"),
        "nested theorem is numbered 2 and resolves: {body}"
    );
    assert!(
        body.contains("<a href=\"#thm-b\" class=\"tali-xref\">Theorem&nbsp;3</a>"),
        "thm-b is 3: {body}"
    );
    // The nested theorem also gets its visible number filled in, not an empty slot.
    assert!(
        !body.contains("<span class=\"tali-theorem-number\"></span>"),
        "no theorem left with an empty number slot: {body}"
    );
    assert!(
        !body.contains("data-qmd-xref"),
        "no ref left dangling: {body}"
    );
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

/// Floats (figures/tables/equations/listings) scope their numbers to a numbered book
/// chapter automatically: "Figure 2.1" is chapter 2's first figure, so a cross-chapter
/// `@fig-` ref is unambiguous. There is no knob to set, and theorems follow the same
/// rule via the same helper.
#[test]
fn book_chapter_scopes_figure_numbers() {
    let doc = render_document_with_includes_scoped(
        "![A fit.](fit.png){#fig-fit}\n\n![A second.](b.png){#fig-two}\n\nSee @fig-fit.\n",
        std::path::Path::new("."),
        Some(2),
    );
    let body = doc.body_html();
    assert!(
        body.contains("<figcaption>Figure&nbsp;2.1: A fit.</figcaption>"),
        "the first figure in chapter 2 numbers as 2.1: {body}"
    );
    assert!(
        body.contains("<figcaption>Figure&nbsp;2.2: A second.</figcaption>"),
        "the second as 2.2: {body}"
    );
    assert!(
        body.contains("<a href=\"#fig-fit\" class=\"tali-xref\">Figure&nbsp;2.1</a>"),
        "the in-page ref agrees with the chapter-scoped number: {body}"
    );
}

/// The other three float kinds scope to the chapter too, each off its own counter: a
/// chapter's first equation/listing/table reads "2.1", never a flat "1".
#[test]
fn book_chapter_scopes_equation_listing_and_table_numbers() {
    let doc = render_document_with_includes_scoped(
        "$$ x = 1 $$ {#eq-one}\n\n\
         ```{python}\n#| label: lst-demo\n#| lst-cap: My listing\nx = 1\n```\n\n\
         | a | b |\n|---|---|\n| 1 | 2 |\n\n: My caption {#tbl-data}\n\n\
         See @eq-one, @lst-demo and @tbl-data.\n",
        std::path::Path::new("."),
        Some(2),
    );
    let body = doc.body_html();
    assert!(
        body.contains("<span class=\"tali-eqn-number\">(2.1)</span>"),
        "the chapter's first equation numbers as (2.1): {body}"
    );
    assert!(
        body.contains("Listing&nbsp;2.1: My listing"),
        "the chapter's first listing numbers as 2.1: {body}"
    );
    assert!(
        body.contains("<caption>Table&nbsp;2.1: My caption</caption>"),
        "the chapter's first table numbers as 2.1: {body}"
    );
    for (anchor, label) in [
        ("eq-one", "Equation&nbsp;2.1"),
        ("lst-demo", "Listing&nbsp;2.1"),
        ("tbl-data", "Table&nbsp;2.1"),
    ] {
        assert!(
            body.contains(&format!(
                "<a href=\"#{anchor}\" class=\"tali-xref\">{label}</a>"
            )),
            "the @{anchor} ref agrees with the chapter-scoped number: {body}"
        );
    }
}

/// Outside a numbered book chapter (a blog post, a standalone page) there is no chapter
/// to scope to, so floats keep flat numbering: "Figure 1" never becomes "Figure .1".
#[test]
fn floats_stay_flat_outside_a_book_chapter() {
    let doc = render_document_with_includes_scoped(
        "![A fit.](fit.png){#fig-fit}\n\n$$ x = 1 $$ {#eq-one}\n\nSee @fig-fit and @eq-one.\n",
        std::path::Path::new("."),
        None,
    );
    let body = doc.body_html();
    assert!(
        body.contains("<figcaption>Figure&nbsp;1: A fit.</figcaption>"),
        "no chapter context keeps flat figure numbering: {body}"
    );
    assert!(
        body.contains("<span class=\"tali-eqn-number\">(1)</span>"),
        "no chapter context keeps flat equation numbering: {body}"
    );
    assert!(
        body.contains("<a href=\"#fig-fit\" class=\"tali-xref\">Figure&nbsp;1</a>"),
        "the flat ref agrees: {body}"
    );
}

/// A theorem in a numbered book chapter scopes to that chapter with NO configuration,
/// the same rule floats follow (`float_number`). Before, this needed an opt-in
/// `theorems: number-within: chapter`, so a book that didn't set it showed "Theorem 5"
/// beside "Figure 2.3" — measured, and the reason the opt-in was dropped.
#[test]
fn theorems_scope_to_the_book_chapter_without_any_config() {
    let doc = render_document_with_includes_scoped(
        "::: {.theorem #thm-a}\nA.\n:::\n\nSee @thm-a.\n\n::: {.theorem #thm-b}\nB.\n:::\n",
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

/// A `shared:` group inside a numbered chapter: the two features compose, so the group
/// draws ONE chapter-scoped sequence (Theorem 2.1, Lemma 2.2) rather than restarting per
/// kind or dropping the chapter. `numbered: unless-unique` still suppresses a lone kind's
/// number, since that decision is made before the number is built.
#[test]
fn a_shared_group_draws_one_chapter_scoped_sequence() {
    let doc = render_document_with_includes_scoped(
        "---\ntheorems:\n  shared: [theorem, lemma]\n  numbered: unless-unique\n---\n\n::: {.theorem}\nA.\n:::\n\n::: {.lemma}\nB.\n:::\n\n::: {.definition}\nOnly one.\n:::\n",
        std::path::Path::new("."),
        Some(2),
    );
    let body = doc.body_html();
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;2.1</span></span>"
        ),
        "the shared group opens at 2.1 in chapter 2: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Lemma<span class=\"tali-theorem-number\">&nbsp;2.2</span></span>"
        ),
        "the lemma continues the SAME chapter-scoped sequence: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Definition<span class=\"tali-theorem-number\"></span></span>"
        ),
        "a lone kind stays unnumbered under unless-unique, chapter or not: {body}"
    );
}

/// Outside a numbered chapter (a standalone doc, a non-book page) there is no chapter to
/// scope to, so numbering stays flat — the `None` half of the same rule floats follow.
#[test]
fn theorems_stay_flat_without_a_chapter() {
    let doc = render_document("::: {.theorem}\nA.\n:::\n\n::: {.theorem}\nB.\n:::\n");
    let body = doc.body_html();
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;1</span></span>"
        ),
        "no chapter context numbers continuously: {body}"
    );
    assert!(
        body.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;2</span></span>"
        ),
        "and keeps counting: {body}"
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
fn strip_tags_drops_katex_mathml_subtree() {
    // Inline math in a heading must not leak KaTeX's `<math>` subtree — the semantic
    // MathML text plus the raw-TeX `<annotation>` — into the visible text used for TOC
    // labels, callout/tabset titles, figure alt-text and deck slugs. Only the visible
    // `katex-html` glyphs should survive (so `$H_0$` reads once as `H0`, never the
    // tripled `H0H_0H0` with leaked LaTeX).
    let doc = render_document("## Expected rank mean under $H_0$\n");
    let h = &doc.blocks[0].html;
    assert!(
        h.contains("<math"),
        "sanity: KaTeX should emit a <math> subtree: {h}"
    );
    let text = strip_tags(h);
    assert!(
        !text.contains("H_0"),
        "raw TeX leaked from the <annotation> into visible text: {text:?}"
    );
    assert!(
        text.matches("H0").count() <= 1,
        "inline math was duplicated (MathML semantic text + katex-html glyphs): {text:?}"
    );
    assert!(
        text.starts_with("Expected rank mean under"),
        "visible heading text was corrupted: {text:?}"
    );
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

    // The REAL caller passes the comrak FrontMatter node, which includes the `---`
    // fences; without stripping them the serde parse fails and a block-sequence
    // bibliography is silently dropped by the fence-less fallback.
    assert_eq!(
        bibliography_paths("---\ntitle: X\nbibliography:\n  - a.bib\n  - b.bib\n---"),
        s(&["a.bib", "b.bib"])
    );
    // A block sequence in front matter that won't parse as YAML at all (unterminated
    // quote below) still resolves via the block-sequence fallback.
    assert_eq!(
        bibliography_paths("bibliography:\n  - a.bib\n  - b.bib\nauthor: \"oops"),
        s(&["a.bib", "b.bib"])
    );
}

// --- accessibility regressions (Batch 3) ---

/// WCAG relative luminance of an sRGB `#rrggbb` color.
#[cfg(test)]
fn wcag_luminance(hex: &str) -> f64 {
    let h = hex.trim_start_matches('#');
    let ch = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f64 / 255.0;
    let lin = |c: f64| {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(ch(0)) + 0.7152 * lin(ch(2)) + 0.0722 * lin(ch(4))
}

#[cfg(test)]
fn wcag_contrast(fg: &str, bg: &str) -> f64 {
    let (a, b) = (wcag_luminance(fg), wcag_luminance(bg));
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

/// Pull the first `#rrggbb` after `needle` in `css` (the color a rule sets).
#[cfg(test)]
fn color_after<'a>(css: &'a str, needle: &str) -> &'a str {
    let i = css
        .find(needle)
        .unwrap_or_else(|| panic!("no `{needle}` in css"));
    let rest = &css[i + needle.len()..];
    let h = rest.find('#').expect("a hex color after the rule");
    &rest[h..h + 7]
}

/// `.sr-only` is aliased to `.tali-sr-only` in base.css so a site that hand-writes the conventional
/// screen-reader class (e.g. a footer icon label) hides it with no per-site custom.css. Guards the
/// tech-blog landmine: without the alias, deleting custom.css un-hides the footer social labels.
#[test]
fn base_css_aliases_the_conventional_sr_only_class() {
    assert!(
        BASE_CSS.contains(".sr-only"),
        "base.css must alias `.sr-only`, not only define `.tali-sr-only`"
    );
}

/// The UI-boundary token must clear the WCAG 1.4.11 3:1 floor against BOTH surfaces a control
/// can sit on: the page background and the code background (a `kbd` and the copy button sit on
/// code-bg). The hairline `--tali-border` stays decorative and is deliberately not checked.
#[test]
fn border_strong_clears_the_ui_boundary_floor_on_both_surfaces() {
    // `:root` defines it first in base.css; the sepia block redefines it further down, so slice
    // from the sepia selector before reading that one.
    let sepia_block = &BASE_CSS[BASE_CSS
        .find("html[data-theme=\"sepia\"] {")
        .expect("sepia block")..];
    for (theme, css, bg, code_bg) in [
        ("light", BASE_CSS, "#ffffff", "#f5f5f5"),
        ("dark", DARK_CSS, "#16181d", "#21242b"),
        ("sepia", sepia_block, "#f4ecd8", "#ece2c8"),
    ] {
        let c = color_after(css, "--tali-border-strong:");
        let (a, b) = (wcag_contrast(c, bg), wcag_contrast(c, code_bg));
        assert!(
            a >= 3.0 && b >= 3.0,
            "{theme} --tali-border-strong {c}: {a:.2} on {bg}, {b:.2} on {code_bg} (need 3.0 on both)"
        );
    }
}

/// Citations and cross-references must not signal "link" with colour alone: against body text the
/// link tone is 1.93:1 in dark (the default theme) and 1.51:1 in sepia. WCAG 1.4.1.
#[test]
fn xref_links_carry_a_non_colour_affordance() {
    let i = BASE_CSS.find(".tali-xref {").expect("the .tali-xref rule");
    let rule = &BASE_CSS[i..i + 160];
    assert!(
        rule.contains("text-decoration: underline"),
        "xref/citation links must be underlined, not colour-only: {rule}"
    );
}

/// `opacity` on text composites it toward the page and silently defeats every contrast assertion
/// written against the authored colour. These four rendered below 4.5:1; none may reintroduce it.
#[test]
fn de_emphasised_text_never_uses_an_opacity_multiplier() {
    for (label, selector) in [
        ("scrolly step", ".scrolly-steps .step {"),
        ("walkthrough step", ".cw-steps .step {"),
        ("read TOC entry", "#TOC a.tali-toc-read {"),
    ] {
        let i = BASE_CSS
            .find(selector)
            .unwrap_or_else(|| panic!("{selector}"));
        let rule = &BASE_CSS[i..i + BASE_CSS[i..].find('}').expect("closing brace")];
        assert!(
            !rule.contains("opacity:"),
            "{label} dims with opacity; recede with --tali-muted instead: {rule}"
        );
    }
    // Code lines carry syntax colours, so no alpha exists that dims visibly AND keeps the comment
    // token at 4.5:1 (it needs >= .94). Both the page walkthrough and the deck must mark the
    // FOCUSED range instead of dimming the rest.
    assert!(
        !BASE_CSS.contains("pre.tali-hl-lines-active .tali-hl-ln { opacity"),
        "the walkthrough must not dim non-focused code lines"
    );
    assert!(
        !super::deck::DECK_CSS.contains("pre.tali-hl-lines-active .tali-hl-ln { opacity"),
        "the deck must not dim non-focused code lines"
    );
}

/// The search-hit `<mark>` shipped one 50%-alpha yellow for every theme; on the dark page it
/// composited to #887219, leaving body text at 3.77:1 on top of the highlight.
#[test]
fn dark_search_mark_keeps_body_text_readable() {
    let c = color_after(
        BASE_CSS,
        "html[data-theme=\"dark\"] mark.tali-search-mark { background-color: ",
    );
    let r = wcag_contrast("#e6e6e6", c);
    assert!(r >= 4.5, "dark search mark {c}: body text at {r:.2}");
}

/// Sepia never overrode `--tali-flash`, so the live-edit pulse painted the `:root` blue onto warm
/// paper. Pin that every theme defines its own.
#[test]
fn every_theme_defines_its_own_flash_tint() {
    let sepia = BASE_CSS
        .find("html[data-theme=\"sepia\"] {")
        .expect("sepia block");
    let block = &BASE_CSS[sepia..sepia + BASE_CSS[sepia..].find('}').expect("closing brace")];
    assert!(
        block.contains("--tali-flash:"),
        "sepia must define --tali-flash, not inherit the :root blue"
    );
    assert!(
        DARK_CSS.contains("--tali-flash:"),
        "dark must define it too"
    );
}

/// Deck link text sat at 4.32:1 on the deck's white background, below AA. The deck now shares
/// the page's accent, which is dark enough to serve as link text unaided.
#[test]
fn deck_link_text_meets_wcag_aa() {
    let c = color_after(super::deck::DECK_CSS, "--deck-accent:");
    let r = wcag_contrast(c, "#ffffff");
    assert!(r >= 4.5, "light deck accent {c} as link on white = {r:.2}");
}

/// The brand rests on ONE owned accent hue. These are the vendor defaults it replaced: three
/// blues (a stock light blue, GitHub Primer's, Tailwind's blue-600), the deck's fourth blue,
/// Material's error red, and the old maximally-saturated callout set. Shipping any of them again
/// is the single loudest "this was assembled from framework defaults" tell, so ban the literals.
#[test]
fn no_vendor_default_colours_remain_in_any_bundled_stylesheet() {
    const BANNED: &[(&str, &str)] = &[
        ("#4c8dff", "the old stock light blue"),
        ("#1f6feb", "GitHub Primer's blue"),
        ("#2563eb", "Tailwind blue-600"),
        ("#4c6ef5", "the deck's fourth blue"),
        ("#b00020", "Material Design's error red"),
        ("#2bb673", "the old callout tip green"),
        ("#e0a800", "the old callout warning amber"),
        ("#e0566b", "the old callout important red"),
        ("#e8730c", "the old callout caution orange"),
    ];
    for (sheet, css) in [
        ("base.css", BASE_CSS),
        ("dark.css", DARK_CSS),
        ("deck.css", super::deck::DECK_CSS),
        ("site.css", SITE_CSS),
    ] {
        let lower = css.to_ascii_lowercase();
        for (hex, what) in BANNED {
            assert!(
                !lower.contains(hex),
                "{sheet} still ships {hex} ({what}); route it through the accent/callout tokens"
            );
        }
    }
}

/// Composite `color-mix(in srgb, C pct%, transparent)` over `bg`: what a callout title bar
/// actually renders as.
#[cfg(test)]
fn mix_over(fg: &str, pct: f64, bg: &str) -> String {
    let ch = |h: &str, i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f64;
    let a = pct / 100.0;
    let (f, b) = (fg.trim_start_matches('#'), bg.trim_start_matches('#'));
    let c = |i: usize| (ch(f, i) * a + ch(b, i) * (1.0 - a)).round() as u8;
    format!("#{:02x}{:02x}{:02x}", c(0), c(2), c(4))
}

/// The callout family is the reader's semantic vocabulary. Every kind's border must clear the 3:1
/// graphical floor against the page, and body text must stay AA on its title tint: in all three
/// themes. (The ICON's own contrast is not a WCAG requirement here: each callout also renders a
/// distinct icon shape and a text title, which puts the icon outside 1.4.11's "required to
/// understand" scope. It is held to 3:1 anyway as a quality bar, but only the two floors below
/// are compliance.)
#[test]
fn callout_family_meets_its_contrast_floors_in_every_theme() {
    let sepia = &BASE_CSS[BASE_CSS
        .find("html[data-theme=\"sepia\"] {")
        .expect("sepia block")..];
    for (theme, css, bg, fg) in [
        ("light", BASE_CSS, "#ffffff", "#1a1a1a"),
        ("dark", DARK_CSS, "#16181d", "#e6e6e6"),
        ("sepia", sepia, "#f4ecd8", "#5b4636"),
    ] {
        for kind in ["note", "tip", "warning", "important", "caution"] {
            let c = color_after(css, &format!("--tali-callout-{kind}:"));
            let border = wcag_contrast(c, bg);
            assert!(
                border >= 3.0,
                "{theme} callout-{kind} border {c}: {border:.2} on {bg} (need 3.0)"
            );
            let pct = if kind == "warning" { 13.0 } else { 12.0 };
            let tint = mix_over(c, pct, bg);
            let body = wcag_contrast(fg, &tint);
            assert!(
                body >= 4.5,
                "{theme} callout-{kind}: body text {body:.2} on its title tint {tint} (need 4.5)"
            );
        }
    }
}

/// Theorem boxes recur densely inside a proof, so they must stay quieter than a callout while
/// still clearing the 3:1 graphical floor for their left border.
#[test]
fn theorem_accents_clear_the_graphical_floor_in_every_theme() {
    let sepia = &BASE_CSS[BASE_CSS
        .find("html[data-theme=\"sepia\"] {")
        .expect("sepia block")..];
    for (theme, css, bg) in [
        ("light", BASE_CSS, "#ffffff"),
        ("dark", DARK_CSS, "#16181d"),
        ("sepia", sepia, "#f4ecd8"),
    ] {
        for kind in ["plain", "definition", "remark"] {
            let c = color_after(css, &format!("--tali-thm-{kind}:"));
            let r = wcag_contrast(c, bg);
            assert!(
                r >= 3.0,
                "{theme} thm-{kind} {c}: {r:.2} on {bg} (need 3.0)"
            );
        }
    }
}

#[test]
fn print_and_high_contrast_blocks_outrank_every_theme_block() {
    // `dark.css` is inlined AFTER `base.css` (see page.rs), so a print/contrast override
    // written as `html[data-theme="dark"]` ties that theme block on specificity (0,1,1) and
    // LOSES on source order: printing from dark mode put #e6e6e6 ink on white paper.
    // `html[data-theme="sepia"]` (0,1,1) likewise outranks a bare `:root` (0,1,0), so sepia
    // printed brown and never got the `prefers-contrast: more` boost. A doubled `:root:root`
    // is (0,2,0), which outranks every `html[data-theme=…]` block regardless of order.
    let print_block = &BASE_CSS[BASE_CSS
        .rfind("@media print")
        .expect("the palette-forcing print block")..];
    assert!(
        print_block.contains(":root:root"),
        "the print block must outrank the theme blocks, not tie with them"
    );
    assert!(
        !print_block.contains("html[data-theme=\"dark\"], :root"),
        "the old tie-on-specificity selector is still present in the print block"
    );
    let contrast_block = &BASE_CSS[BASE_CSS
        .find("prefers-contrast: more")
        .expect("the prefers-contrast block")..];
    assert!(
        contrast_block.contains(":root:root"),
        "prefers-contrast: more must reach dark + sepia, not only light"
    );
}

#[test]
fn printing_forces_the_light_theme_even_from_dark() {
    // The CSS override above only resets the *tokens*. `dark.css` also recolours the syntax
    // scopes (`.tali-hl-string` -> #a5d6ff, 1.6:1 on white paper) and the diagnostic boxes,
    // none of which are tokenised. Swapping `data-theme` to light for the duration of the
    // print job neutralises all of them at once: the same trick deck.js already uses.
    let head = theme_head("auto");
    assert!(
        head.contains("beforeprint"),
        "the page must drop to the light theme while printing"
    );
    assert!(
        head.contains("afterprint"),
        "the page must restore the reader's theme after printing"
    );
}

#[test]
fn syntax_comment_token_meets_wcag_aa() {
    // Batch 3b: the comment token was sub-AA (light 4.17, sepia 3.17) on its code
    // background. Pin ≥ 4.5:1 against the actual code-block backgrounds so a future
    // palette edit can't silently regress it.
    let light = color_after(BASE_CSS, ".tali-hl-comment { color: ");
    assert!(
        wcag_contrast(light, "#f5f5f5") >= 4.5,
        "light comment {light} vs #f5f5f5 = {:.2}",
        wcag_contrast(light, "#f5f5f5")
    );
    let sepia = color_after(
        BASE_CSS,
        "html[data-theme=\"sepia\"] .tali-hl-comment { color: ",
    );
    assert!(
        wcag_contrast(sepia, "#ece2c8") >= 4.5,
        "sepia comment {sepia} vs #ece2c8 = {:.2}",
        wcag_contrast(sepia, "#ece2c8")
    );
}

#[test]
fn cmd_k_palette_uses_aa_accent_tokens_not_raw_accent() {
    // Batch 3a: the selected row + match marks used raw `--tali-accent`, failing AA
    // in every theme. They must use the AA-tuned fill/on-accent (filled row) and
    // link (marks) tokens instead.
    assert!(
        SEARCH_JS.contains(".tali-s-item[aria-selected=true]{background:var(--tali-accent-fill")
            && SEARCH_JS.contains("color:var(--tali-on-accent"),
        "selected row must use accent-fill bg + on-accent text"
    );
    assert!(
        SEARCH_JS.contains(".tali-s-snip mark{background:transparent;color:var(--tali-link")
            && SEARCH_JS
                .contains(".tali-s-title mark{background:transparent;color:var(--tali-link"),
        "match marks must use the AA-tuned --tali-link, not raw --tali-accent"
    );
    // The combobox role moved onto the input; the listbox is named (Batch 3h).
    assert!(
        SEARCH_JS.contains("role=\"combobox\" aria-expanded=\"true\"")
            && SEARCH_JS.contains("role=\"listbox\" aria-label=\"Search results\""),
        "combobox role on the input + a named listbox"
    );
}

#[test]
fn same_page_sec_ref_uses_hierarchical_number_in_a_chapter() {
    // Batch 4: inside a book chapter, a SAME-PAGE `@sec-` must show the same
    // hierarchical number its target heading visibly shows (e.g. "2.2"), not the
    // flat sequential `sec_count` ("1") that contradicts it.
    use std::path::Path;
    let src =
        "See @sec-y.\n\n## First\n\n## Second {#sec-y}\n\n### Deep {#sec-z}\n\nAlso @sec-z.\n";
    let doc = render_document_with_includes_scoped(src, Path::new("."), Some(2));
    let body = doc.body_html();
    // `## Second` is the 2nd h2 of chapter 2 -> 2.2; `### Deep` is its first h3 -> 2.2.1.
    assert!(
        body.contains("class=\"tali-xref\">Section&nbsp;2.2</a>"),
        "same-page @sec-y should be 2.2, got: {body}"
    );
    assert!(
        body.contains("class=\"tali-xref\">Section&nbsp;2.2.1</a>"),
        "same-page @sec-z should be 2.2.1, got: {body}"
    );
}

#[test]
fn unterminated_fence_warns_located() {
    // Batch 5: a `:::` open with no matching close silently dropped its wrapper. It
    // must raise a click-to-source warning at the open line, and still render the
    // content (unwrapped) rather than swallow it.
    let doc = render_document("::: {.callout-note}\nBody with no close.\n");
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("unterminated") && w.line == Some(1)),
        "unterminated fence should warn at line 1, got: {:?}",
        doc.warnings
    );
    assert!(
        doc.body_html().contains("Body with no close."),
        "the content must still render, not be swallowed"
    );
    // A properly closed fence raises no such warning.
    let ok = render_document("::: {.callout-note}\nBody.\n:::\n");
    assert!(
        !ok.warnings
            .iter()
            .any(|w| w.message.contains("unterminated")),
        "a closed fence must not warn"
    );
}

#[test]
fn quoted_figure_width_survives_smart_punctuation() {
    // Batch 5: comrak's smart-punctuation rewrites the straight quotes in a figure's
    // `{width="60%"}` to curly quotes in the rendered text, and the attr parser only
    // stripped straight quotes — leaving `style="width:“60%”"` (invalid CSS, silent
    // no-op). The value must come out clean.
    let doc = render_document("![Cap](x.png){#fig-q width=\"60%\" height=\"40%\"}\n");
    let body = doc.body_html();
    assert!(
        body.contains("style=\"width:60%;height:40%\""),
        "quoted width/height must render clean CSS, got: {body}"
    );
    assert!(
        !body.contains('\u{201c}') && !body.contains('\u{201d}'),
        "no curly quotes should leak into the style, got: {body}"
    );
}

#[test]
fn explicit_slide_heading_id_becomes_the_section_anchor() {
    // Batch 4: an explicit `{#sec-x}` on a slide heading was dropped — the slide got a
    // text-slug id instead, so `@sec-x` linked to a missing anchor. The explicit id
    // must become the `<section>` id so the cross-reference resolves.
    let doc =
        render_document("---\nformat: deck\n---\n\nSee @sec-two.\n\n## One\n\n## Two {#sec-two}\n");
    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    assert!(
        slides.contains("<section id=\"sec-two\""),
        "the explicit {{#sec-two}} must be the slide's section id, got: {slides}"
    );
    assert!(
        !slides.contains("<section id=\"two\""),
        "the text-slug id must not win over the explicit anchor"
    );
}

#[test]
fn heading_consumed_as_callout_title_keeps_its_anchor_id() {
    // Batch 4: a `{#sec-x}` heading used as a callout title had its id stripped with
    // its tags, so `@sec-x` resolved to a number but linked to a missing anchor. The
    // id must survive on the callout title element.
    let doc = render_document(
        "See @sec-note.\n\n::: {.callout-note}\n## Important {#sec-note}\n\nBody.\n:::\n",
    );
    let body = doc.body_html();
    assert!(
        body.contains("class=\"callout-title\" id=\"sec-note\"")
            || body.contains("id=\"sec-note\" class=\"callout-title\""),
        "callout title must keep the consumed heading's #id, got: {body}"
    );
    // And the ref resolves (it was registered) — not a dangling data-qmd-xref marker.
    assert!(
        body.contains("class=\"tali-xref\">Section&nbsp;1</a>"),
        "the @sec-note ref should resolve to Section 1, got: {body}"
    );
}

#[test]
fn same_page_sec_ref_stays_flat_without_a_chapter() {
    // A chapterless doc (single doc / website page) keeps the flat sequential
    // numbering — it has no hierarchical chapter to scope to.
    use std::path::Path;
    let src = "See @sec-a and @sec-b.\n\n## One {#sec-a}\n\n## Two {#sec-b}\n";
    let doc = render_document_with_includes_scoped(src, Path::new("."), None);
    let body = doc.body_html();
    assert!(
        body.contains("class=\"tali-xref\">Section&nbsp;1</a>")
            && body.contains("class=\"tali-xref\">Section&nbsp;2</a>"),
        "chapterless @sec- refs stay flat 1,2, got: {body}"
    );
}

#[test]
fn deck_overview_reveals_magic_move_final_block() {
    // B1-5: a magic-move on a slide never made current stays opacity:0 (a blank tile) in
    // overview unless overview forces its final block visible AND hides the current slide's
    // active non-last block (which would otherwise overlap it in the shared grid cell).
    let deck_css = include_str!("../../assets/css/deck.css");
    assert!(
        deck_css.contains(".tali-deck.overview .magic-move > pre:last-of-type"),
        "overview magic-move final-block override missing (unvisited slides render blank)"
    );
    assert!(
        deck_css.contains(".tali-deck.overview .magic-move > pre {"),
        "overview must also hide non-last magic-move blocks or the current slide overlaps two pres"
    );
}

#[test]
fn deck_fragment_effect_class_rides_alongside_the_fragment_class() {
    // D107: an effect is authored as a second class on the SAME fenced div
    // (`::: {.fragment .fade-out}`), matching the existing `::: {.fragment}` pattern. The
    // generic class-div path joins the classes, so the block still matches deck.js's
    // FRAG_SEL (`.fragment`) and still earns a step: the effect is CSS on top, not a new
    // authoring form and not a new step kind.
    let doc = render_document(
        "---\nformat: deck\n---\n\n## S\n\n::: {.fragment .fade-out}\nGone on the next press.\n:::\n\n::: {.fragment .highlight}\nMarked on the next press.\n:::\n",
    );
    let slides = slides_html(None, None, &doc.blocks);
    assert!(
        slides.contains("<div class=\"fragment fade-out\""),
        "`.fade-out` must survive alongside `fragment`: {slides}"
    );
    assert!(
        slides.contains("<div class=\"fragment highlight\""),
        "`.highlight` must survive alongside `fragment`: {slides}"
    );
}

#[test]
fn deck_fragment_effects_start_visible_and_step_into_their_effect() {
    // D107: a plain fragment is hidden until its step. An EFFECT fragment inverts that: it
    // starts VISIBLE and its step changes it, so it must override the base hidden rule or
    // it reads as a plain fragment (invisible until stepped) and the effect is lost.
    let deck_css = include_str!("../../assets/css/deck.css");
    assert!(
        deck_css.contains(".tali-deck .fragment.fade-out,")
            && deck_css.contains(".tali-deck .fragment.highlight {"),
        "effect fragments must override the base hidden state or they never show before their step"
    );
    assert!(
        deck_css.contains(".tali-deck .fragment.fade-out.tali-frag-visible {"),
        "`.fade-out` must leave on its step"
    );
    assert!(
        deck_css.contains(".tali-deck .fragment.highlight.tali-frag-visible {"),
        "`.highlight` must mark the block on its step"
    );
}

#[test]
fn deck_faded_out_fragment_returns_in_overview_and_feed() {
    // A stepped `.fade-out` still carries `.tali-frag-visible`, and its hide rule outranks
    // overview's and the feed's "show every fragment" overrides on specificity, so without
    // an explicit exception the block VANISHES from the overview grid and the mobile feed,
    // neither of which steps, and the content is unreachable there. Pin both exceptions.
    let deck_css = include_str!("../../assets/css/deck.css");
    assert!(
        deck_css.contains(".tali-deck.overview .fragment.fade-out.tali-frag-visible"),
        "overview must show a faded-out fragment (it shows every slide complete)"
    );
    assert!(
        deck_css.contains("html.tali-feed .fragment.fade-out.tali-frag-visible"),
        "the mobile feed must show a faded-out fragment (it never steps)"
    );
}

#[test]
fn deck_defines_light_bg_text_override() {
    // Batch 3d: a light per-slide background needs a `.tali-light-bg` rule forcing
    // DARK text, or the deck's default (light) text is invisible on it. Pin both the
    // dark-bg and light-bg overrides so the mirror can't be dropped.
    let deck_css = include_str!("../../assets/css/deck.css");
    assert!(
        deck_css.contains(".tali-slides section.tali-dark-bg"),
        "dark-bg text override missing"
    );
    assert!(
        deck_css.contains(".tali-slides section.tali-light-bg"),
        "light-bg text override missing (light named/hex slide backgrounds render invisible text)"
    );
    // The light-bg override forces near-black text.
    let dark_text = color_after(
        deck_css,
        ".tali-slides section.tali-light-bg strong { color: ",
    );
    assert!(
        wcag_contrast(dark_text, "#ffffff") >= 7.0,
        "light-bg text {dark_text} must be dark enough to read on a light slide"
    );
    // B4-20: a contrast-flipped slide forces its own text light/dark, but a code panel
    // keeps its themed `--deck-code-bg`, so `pre`/`code` ink must be re-pinned to
    // `--deck-ink` or untokenized code goes invisible on the panel. Pin the rule so a
    // future refactor can't silently drop it.
    assert!(
        deck_css.contains("section.tali-dark-bg pre")
            && deck_css.contains("section.tali-light-bg code")
            && deck_css.contains("{ color: var(--deck-ink); }"),
        "the contrast-flip code-ink override (B4-20) is missing"
    );
}

/// A front-matter-less document starting with an `# H1` used to render `<title>` as the
/// file stem (standalone) or as nothing at all (site path, where `og:title` then quietly
/// borrowed the site's own name). The leading H1 is what the author called the page:
/// promote it. A better default, not a new knob.
#[test]
fn a_leading_h1_titles_a_standalone_page_instead_of_the_file_stem() {
    let doc = super::render_document("# My Great Post\n\nBody.\n");
    // `RenderedDoc::title` still means "the front-matter title", so a site can prefer its
    // own authored page title over the heading.
    assert_eq!(doc.title, None);
    let html = super::render_doc_to_page(&doc, "the-file-stem", crate::OutputMode::Build);
    assert!(
        html.contains("<title>My Great Post</title>"),
        "the leading h1 must beat the file stem"
    );
}

#[test]
fn front_matter_title_always_wins_over_a_leading_h1() {
    let doc = super::render_document("---\ntitle: Real Title\n---\n\n# Something Else\n\nBody.\n");
    let html = super::render_doc_to_page(&doc, "stem", crate::OutputMode::Build);
    assert!(
        html.contains("<title>Real Title</title>"),
        "front matter wins"
    );
}

#[test]
fn only_a_leading_h1_is_promoted_never_a_later_or_deeper_heading() {
    let page = |src: &str| {
        super::render_doc_to_page(
            &super::render_document(src),
            "stem",
            crate::OutputMode::Build,
        )
    };
    // An h2 first: a section, not the document's name.
    assert!(page("## A section\n\n# A late h1\n").contains("<title>stem</title>"));
    // Prose before the first h1: the h1 is not the document's name either.
    assert!(page("An intro paragraph.\n\n# Not the title\n").contains("<title>stem</title>"));
    // No headings at all.
    assert!(page("Just prose.\n").contains("<title>stem</title>"));
}

#[test]
fn a_promoted_h1_title_is_plain_text_not_html() {
    // The block's html carries an anchor id, inline markup and entities. `<title>` escapes
    // its input, so the promoted value must be decoded plain text or `&` ships as `&amp;amp;`.
    let doc = super::render_document("# `code` & *emphasis* <br> end\n");
    let html = super::render_doc_to_page(&doc, "stem", crate::OutputMode::Build);
    assert!(
        html.contains("<title>code &amp; emphasis  end</title>"),
        "expected decoded-then-escaped title, got: {:?}",
        html.split("<title>")
            .nth(1)
            .and_then(|s| s.split("</title>").next())
    );
}

#[test]
fn leading_h1_text_reads_only_a_first_level_one_heading() {
    let h1 = |src: &str| super::leading_h1_text(&super::render_document(src).blocks);
    assert_eq!(h1("# Hello\n").as_deref(), Some("Hello"));
    assert_eq!(h1("## Hello\n"), None);
    assert_eq!(h1("Prose\n\n# Hello\n"), None);
    assert_eq!(h1(""), None);
}

#[test]
fn toc_filter_is_relative_to_the_shallowest_heading() {
    // A titleless document whose sections start at <h2> (shallowest level = 2): the TOC
    // shows three levels (h2/h3/h4) and drops h5. This is the relative window, not the
    // old absolute `level <= 3` (which would have stopped at h3 and shown only two).
    let doc = render_document("## A\n\n### B\n\n#### C\n\n##### D\n");
    assert_eq!(
        toc_entry_count(&doc.blocks),
        3,
        "h2/h3/h4 shown, h5 dropped"
    );
    // A conventional document (shallowest level = 1) is unchanged: h1/h2/h3 shown, h4 dropped.
    let doc = render_document("# A\n\n## B\n\n### C\n\n#### D\n");
    assert_eq!(
        toc_entry_count(&doc.blocks),
        3,
        "h1/h2/h3 shown, h4 dropped"
    );
}

#[test]
fn title_block_demotes_body_headings_to_a_single_h1() {
    let doc = render_document("---\ntitle: \"Post\"\n---\n\n# Theory\n\n## Model\n\n### Detail\n");
    // The only <h1> is the title block; body sections each shift down one level.
    let body = doc
        .blocks
        .iter()
        .map(|b| b.html.as_str())
        .collect::<String>();
    assert_eq!(
        body.matches("<h1").count(),
        1,
        "exactly one h1 (the title):\n{body}"
    );
    assert!(doc.blocks[0].html.contains("<h1 class=\"title\">Post</h1>"));
    assert!(
        doc.blocks[1].html.starts_with("<h2 "),
        "# Theory -> h2, got: {}",
        doc.blocks[1].html
    );
    assert!(
        doc.blocks[2].html.starts_with("<h3 "),
        "## Model -> h3, got: {}",
        doc.blocks[2].html
    );
    assert!(
        doc.blocks[3].html.starts_with("<h4 "),
        "### Detail -> h4, got: {}",
        doc.blocks[3].html
    );
}

#[test]
fn demotion_preserves_anchor_id_and_source_keyed_block_id() {
    let titled = render_document("---\ntitle: T\n---\n\n# Methods\n");
    let demoted = &titled.blocks[1]; // the body heading, demoted h1 -> h2
    assert!(demoted.html.starts_with("<h2 "), "got: {}", demoted.html);
    // The anchor slug is text-derived, so it survives demotion (#anchors + @sec- refs hold).
    assert!(
        demoted.html.contains("id=\"methods\""),
        "anchor id unchanged: {}",
        demoted.html
    );
    // block-id hashes the SOURCE line, not the emitted tag: same source `# Methods` -> same id.
    let undemoted = render_document("# Methods\n"); // no title block, so <h1> stays
    assert_eq!(
        demoted.id, undemoted.blocks[0].id,
        "block-id keys off source, not the tag"
    );
}

#[test]
fn heading_demotion_clamps_at_h6() {
    let doc = render_document("---\ntitle: T\n---\n\n###### Deep\n");
    // A body <h6> has nowhere lower to go; it stays <h6> (never <h7>).
    assert!(
        doc.blocks[1].html.starts_with("<h6 "),
        "got: {}",
        doc.blocks[1].html
    );
}

#[test]
fn hidden_title_block_leaves_body_headings_alone() {
    // `title-block-style: none` emits no title block, so the trigger is absent: a body
    // `# Section` stays <h1> (the author's own heading hierarchy is untouched).
    let doc = render_document("---\ntitle: T\ntitle-block-style: none\n---\n\n# Section\n");
    assert!(
        doc.blocks[0].html.starts_with("<h1 "),
        "got: {}",
        doc.blocks[0].html
    );
}

#[test]
fn deck_headings_are_not_demoted() {
    // A deck (Reveal) builds its own title slide and uses h1/h2 as slide breaks; demotion
    // must never touch it. `## Slide` stays <h2> (the slide-open level), `### Point` <h3>.
    let doc = render_document("---\ntitle: T\nformat: deck\n---\n\n## Slide\n\n### Point\n");
    let joined = doc
        .blocks
        .iter()
        .map(|b| b.html.as_str())
        .collect::<String>();
    assert!(joined.contains("<h2 "), "slide heading stays h2:\n{joined}");
    assert!(joined.contains("<h3 "), "sub-heading stays h3:\n{joined}");
}

#[test]
fn a_demoted_post_still_lists_all_its_sections_in_the_toc() {
    // After demotion the sections are h2/h3/h4; the relative TOC filter surfaces all three
    // (the title block starts with <header>, not <hN>, so it is not counted as a heading).
    let doc = render_document("---\ntitle: T\n---\n\n# A\n\n## B\n\n### C\n");
    assert_eq!(
        toc_entry_count(&doc.blocks),
        3,
        "all three demoted sections listed"
    );
}

#[test]
fn body_uses_the_inlined_newsreader_face() {
    let doc = render_document("Body prose.\n");
    let page = super::render_doc_to_page(&doc, "stem", crate::OutputMode::Build);
    // Two real @font-face rules for the owned body face, family "Newsreader".
    assert!(page.contains("@font-face"), "no @font-face in page head");
    assert!(
        page.contains("\"Newsreader\""),
        "Newsreader @font-face family missing"
    );
    // A true italic face (not synthesized) alongside the normal one.
    assert!(
        page.contains("font-style: italic"),
        "italic Newsreader face missing"
    );
    // Inlined as a data URI (offline, self-contained), never a bare url(fonts/…) that
    // would 404 since there is no served font path.
    assert!(
        page.contains("url(data:font/woff2;base64,"),
        "font not inlined as a data URI"
    );
    assert!(
        !page.contains("url(fonts/newsreader"),
        "a bare font url leaked into the page (would 404)"
    );
    // The body typeface variable actually names the face (system serif kept as fallback).
    assert!(
        page.contains("\"Newsreader\", ui-serif"),
        "--tali-font-body not pointed at Newsreader"
    );
}

// Marker literals below are each confirmed present via grep before use (see the Task 1
// report): base.css -> ".tali-reader-seg", dark.css -> the dark-theme mermaid override
// selector, site.css -> ".tali-book-topbar" (site-only chrome), the code-enhance bundle
// -> "function taliCopyText" (defined in 01-registry.js), search.js -> "function
// buildIndex", mermaid.min.js -> the esbuild wrapper var, d3.min.js -> its source-map
// comment header, plot.umd.min.js -> its own header comment.

#[test]
fn shared_site_css_bundles_the_framework_sheets() {
    let css = shared_site_css();
    assert!(css.contains(".tali-reader-seg"), "base.css missing");
    assert!(
        css.contains("html[data-theme=\"dark\"] pre.mermaid"),
        "dark.css missing"
    );
    assert!(css.contains(".tali-book-topbar"), "site.css missing");
}

#[test]
fn core_enhance_js_has_our_scripts_not_the_big_libs() {
    let js = core_enhance_js();
    assert!(js.contains("function taliCopyText"), "code-enhance missing");
    assert!(js.contains("function buildIndex"), "search.js missing");
    // The big vendored libs must NOT be in the always-on core bundle.
    assert!(
        !js.contains("__esbuild_esm_mermaid"),
        "mermaid lib leaked into core"
    );
    assert!(!js.contains("d3js.org"), "d3 leaked into core");
    // The `{js}`-cell runtime must NOT be bundled into app.js either: it runs cells via
    // `new AsyncFunction`, whose `import()` resolves against the calling script, so folding
    // it into the shared `/_assets/app.js` would break a cell's page-relative
    // `import("./helper.js")`. It stays inline on the page instead (page.rs `qmd_js_inline`).
    // `"qmd-js cell error:"` is a literal unique to qmd-js.js.
    assert!(
        !js.contains("qmd-js cell error:"),
        "the {{js}}-cell runtime must stay inline, not in the shared app.js"
    );
}

#[test]
fn mermaid_and_jslibs_bundles_carry_their_libs() {
    assert!(
        mermaid_bundle_js().contains("__esbuild_esm_mermaid"),
        "mermaid lib missing"
    );
    // The loader's CDN placeholder must be resolved, never left raw.
    assert!(
        !mermaid_bundle_js().contains("{{MERMAID}}"),
        "loader placeholder unresolved"
    );
    let libs = js_cell_libs_js();
    assert!(
        libs.contains("d3js.org") && libs.contains("@observablehq/plot"),
        "d3/plot missing"
    );
}

#[test]
fn has_mermaid_detects_the_diagram_marker() {
    assert!(has_mermaid("<pre class=\"mermaid\">graph TD</pre>"));
    assert!(!has_mermaid("<p>no diagrams here</p>"));
}
