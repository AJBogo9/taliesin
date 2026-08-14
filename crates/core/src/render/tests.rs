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
    // Ids must be DERIVED from content, not merely positionally unique: two different
    // single-block docs must get different ids. A `make_id` that ignored the block bytes
    // and leaned only on positional dedup would still satisfy the uniqueness + stability
    // checks above while collapsing every doc's first block to one id (the invariant the
    // diff, click-to-source and live-state preservation all key off). That regression was
    // otherwise caught only incidentally by four `{js}` snapshot docs — pin it directly.
    let alpha = render_document("Alpha.\n");
    let bravo = render_document("Bravo.\n");
    assert_ne!(
        alpha.blocks[0].id, bravo.blocks[0].id,
        "block ids must be content-derived, not positional"
    );
}

#[test]
fn front_matter_title_extracted_and_rendered_as_title_block() {
    let doc = render_document("---\ntitle: \"My Post\"\nfoo: bar\n---\n\nBody.\n");
    assert_eq!(doc.title.as_deref(), Some("My Post"));
    // A generated title block is prepended, then the body paragraph.
    assert_eq!(doc.blocks.len(), 2);
    assert_eq!(doc.blocks[0].id, "tali-title-block");
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
fn reading_time_scales_with_word_count() {
    // The estimate is prose-words / 200 wpm, min 1. Pin the NUMBER, not merely that some
    // "N min read" string is present: a constant `mins = 1` regression sails through a
    // `contains(" min read")` substring check (the site tests only did that), yet mislabels
    // every long post as a one-minute read.
    let body = "lorem ".repeat(400);
    let doc = render_document(&format!("---\ntitle: T\ndate: 2020-01-01\n---\n\n{body}\n"));
    let h = doc.body_html();
    assert!(
        h.contains("class=\"tali-read-time\""),
        "reading time shown: {h}"
    );
    // (400 + 100) / 200 = 2, so it must read "2 min read", not a collapsed constant.
    assert!(
        h.contains("2 min read"),
        "reading time must scale with word count, got: {h}"
    );
    assert!(
        !h.contains("1 min read"),
        "reading time must not collapse to a constant: {h}"
    );
}

#[test]
fn title_block_style_none_injects_a_hidden_h1_but_no_visible_block() {
    let doc = render_document("---\ntitle: \"Blog\"\ntitle-block-style: none\n---\n\nIntro.\n");
    // Metadata title is preserved (drives `<title>`, OpenGraph, nav)...
    assert_eq!(doc.title.as_deref(), Some("Blog"));
    // ...and no VISIBLE title-block header is emitted...
    assert!(
        !doc.blocks.iter().any(|b| b.id == "tali-title-block"),
        "expected no visible title block, got ids: {:?}",
        doc.blocks.iter().map(|b| &b.id).collect::<Vec<_>>()
    );
    // ...but a visually-hidden <h1> now carries the page title so a listing/section page has
    // one `<h1>` for SEO + heading-nav (PA-H2) instead of opening at an H2/H3 card.
    let sr = doc
        .blocks
        .iter()
        .find(|b| b.id == "tali-sr-title")
        .expect("a hidden <h1> should be injected");
    assert!(
        sr.html.contains("<h1 class=\"tali-sr-only\"") && sr.html.contains(">Blog</h1>"),
        "got: {}",
        sr.html
    );
    assert!(doc.blocks.iter().any(|b| b.html.contains("Intro.")));
}

#[test]
fn title_block_style_none_does_not_duplicate_an_existing_h1() {
    // A `hero:` landing (or any page with its own `# Heading`) already has an `<h1>`, so the
    // hidden-title injection must stay out — one `<h1>` per page.
    let doc =
        render_document("---\ntitle: Home\ntitle-block-style: none\n---\n\n# Welcome\n\nHi.\n");
    assert!(
        !doc.blocks.iter().any(|b| b.id == "tali-sr-title"),
        "must not inject a second h1 when the body already has one"
    );
    assert_eq!(
        doc.blocks.iter().filter(|b| b.html.contains("<h1")).count(),
        1,
        "exactly one <h1> per page"
    );
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
        h.contains("<span class=\"tali-author\">A</span>")
            && h.contains("<time datetime=\"2026-05-15\">15 May 2026</time>"),
        "got: {h}"
    );
    // A plain scalar `author:` gains no affiliation furniture: the title block a page
    // without affiliations emits is the one it always emitted, plus a class name.
    assert!(!h.contains("tali-affiliations"), "got: {h}");
}

#[test]
fn a_structured_author_list_renders_a_byline_with_numbered_affiliations() {
    // The whole point of item 184: `author:` carries more than a name, and the byline is
    // the first of the three consumers to show it. The numbers are DERIVED from the
    // affiliation strings (first appearance wins), so nothing in the source names an
    // index and nothing can drift out of sync.
    let doc = render_document(concat!(
        "---\n",
        "title: T\n",
        "author:\n",
        "  - name: Ada Lovelace\n",
        "    affiliation: Analytical Engine Institute\n",
        "    url: https://example.org/ada\n",
        "    equal: true\n",
        "  - name: Charles Babbage\n",
        "    affiliation: [Analytical Engine Institute, Somewhere Else]\n",
        "---\n\nx\n",
    ));
    let h = &doc.blocks[0].html;
    assert!(
        h.contains("<a href=\"https://example.org/ada\">Ada Lovelace</a>"),
        "a `url:` links the name: {h}"
    );
    assert!(
        h.contains("<sup class=\"tali-author-mark\">1,*</sup>"),
        "Ada is at institution 1 and claims equal contribution: {h}"
    );
    assert!(
        h.contains("<sup class=\"tali-author-mark\">1,2</sup>"),
        "Charles shares institution 1 and adds 2: {h}"
    );
    assert!(
        h.contains(
            "<li><sup class=\"tali-affiliation-num\">1</sup>Analytical Engine Institute</li>\
             <li><sup class=\"tali-affiliation-num\">2</sup>Somewhere Else</li>"
        ),
        "the numbers are emitted as content, in first-appearance order: {h}"
    );
    assert!(h.contains("* Equal contribution"), "equal note: {h}");
    // One entry for an institution two authors share — the reason the strings are the
    // key rather than an author-written index.
    assert_eq!(
        h.matches("Analytical Engine Institute").count(),
        1,
        "a shared affiliation is listed once: {h}"
    );
}

#[test]
fn the_appendix_renders_each_authors_contribution() {
    // Item 187. Contributions are declared BESIDE the name (an `author:` sub-key), not in
    // a separate map keyed by name: a map has to match a name string back to an author and
    // silently drops the entry when the two spellings differ.
    //
    // Contributions are the whole appendix since 2026-08-03; its other two parts
    // (`acknowledgments:` and `doi:`) went with the academic-publishing cluster.
    let doc = render_document(concat!(
        "---\n",
        "title: T\n",
        "author:\n",
        "  - name: Ada Lovelace\n",
        "    contribution: Designed the study.\n",
        "  - name: Charles Babbage\n",
        "    contribution: Built the engine.\n",
        "  - name: Anon Nobody\n",
        "---\n\nx\n",
    ));
    let h = &doc
        .blocks
        .iter()
        .find(|b| b.id == crate::render::APPENDIX_BLOCK_ID)
        .expect("appendix block emitted")
        .html;
    assert!(
        h.contains("<dt>Ada Lovelace</dt><dd>Designed the study.</dd>")
            && h.contains("<dt>Charles Babbage</dt><dd>Built the engine.</dd>"),
        "each contributor pairs with what they did: {h}"
    );
    assert!(
        !h.contains("Anon Nobody"),
        "an author who declared no contribution contributes no row: {h}"
    );
    // It lands after the content, not before it.
    let idx = doc
        .blocks
        .iter()
        .position(|b| b.id == crate::render::APPENDIX_BLOCK_ID)
        .unwrap();
    assert!(idx > 0, "the appendix is appended, not prepended");
}

/// The appendix is opt-in furniture: a page whose authors declare no `contribution:` must
/// emit no appendix at all. Load-bearing since 2026-08-03, when contributions became its
/// only part — before that an `acknowledgments:` or a `doi:` could still carry the block,
/// so "contributors empty" did not mean "no appendix".
#[test]
fn no_contributions_emits_no_appendix() {
    let doc = render_document("---\ntitle: T\nauthor: Ada Lovelace\ndate: 2026-05-15\n---\n\nx\n");
    assert!(
        !doc.blocks
            .iter()
            .any(|b| b.id == crate::render::APPENDIX_BLOCK_ID),
        "no contribution -> no appendix"
    );
}

#[test]
fn a_page_declaring_none_of_the_appendix_keys_emits_no_appendix() {
    // Opt-in furniture: an ordinary post gains no trailing block. Keyed on the block ID
    // rather than a class substring, since the bundled CSS names the class on every page.
    let doc = render_document("---\ntitle: T\nauthor: A\ndate: 2026-05-15\n---\n\nx\n");
    assert!(
        !doc.blocks
            .iter()
            .any(|b| b.id == crate::render::APPENDIX_BLOCK_ID),
        "no appendix keys -> no appendix block"
    );
}

#[test]
fn the_appendix_is_deterministic_across_renders() {
    // The rule `cite_this` documents and this block inherits: a generated block that read
    // a clock would change on every build, breaking the byte-identical build AND
    // invalidating the freeze cache on every run. Nothing here reads one, and this is the
    // test that keeps it that way.
    let src = concat!(
        "---\n",
        "title: T\n",
        "doi: 10.5281/zenodo.1234\n",
        "acknowledgments: Thanks.\n",
        "author:\n",
        "  - name: Ada\n",
        "    contribution: Everything.\n",
        "---\n\nx\n",
    );
    let a = render_document(src);
    let b = render_document(src);
    let pick = |d: &crate::render::RenderedDoc| {
        d.blocks
            .iter()
            .find(|b| b.id == crate::render::APPENDIX_BLOCK_ID)
            .map(|b| b.html.clone())
            .expect("appendix emitted")
    };
    assert_eq!(pick(&a), pick(&b), "two renders must be byte-identical");
}

#[test]
fn a_structured_author_still_reaches_the_byline_at_all() {
    // The regression this design is most exposed to. The byline used to be read by
    // `extract_field`, a LINE SCAN that skips indented lines — so a structured
    // `author:` (whose name sits on a sub-line) returned None and the byline vanished
    // with no error. Nothing else would catch it: a title block is generated, so no
    // document's source mentions the author's name.
    let doc = render_document("---\ntitle: T\nauthor:\n  - name: Ada Lovelace\n---\n\nx\n");
    assert!(
        doc.blocks[0].html.contains("Ada Lovelace"),
        "structured author must still produce a byline: {}",
        doc.blocks[0].html
    );
}

#[test]
fn html_is_escaped_in_text() {
    let doc = render_document("a < b & c\n");
    assert!(doc.blocks[0].html.contains("a &lt; b &amp; c"));
}

#[test]
fn tmd_code_cell_language_detected() {
    let doc = render_document("```{python}\nprint(1)\n```\n");
    assert!(doc.blocks[0].html.contains("<pre "));
    assert!(doc.blocks[0].html.contains("class=\"language-python\""));
}

#[test]
fn table_uses_thead_th_and_tbody_td() {
    let doc = render_document("| A | B |\n|---|--:|\n| 1 | 2 |\n");
    let h = &doc.blocks[0].html;
    assert!(h.starts_with("<table "), "got: {h}");
    // Header cells carry `scope="col"` (PA-M6) so AT pairs each data cell with its column.
    assert!(
        h.contains("<thead><tr><th scope=\"col\">A</th><th scope=\"col\""),
        "got: {h}"
    );
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
fn a_retired_columns_div_says_removed_and_layout_ncol_still_works() {
    // `.columns`/`.column` were withdrawn on 2026-08-02: two mechanisms for one job, and
    // six weeks of daily writing adopted the other one. The diagnostic is the whole point
    // of the removal — div classes are an OPEN vocabulary, so without a retired register a
    // leftover `.columns` reads as a custom class and the page silently loses its layout.
    let doc = render_document(
        "::: {.columns}\n::: {.column}\nLeft\n:::\n\n::: {.column}\nRight\n:::\n:::\n",
    );
    let h: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
    assert!(
        !h.contains("tali-layout"),
        "`.columns` no longer builds a grid: {h}"
    );
    let msgs: Vec<&str> = doc.warnings.iter().map(|w| w.message.as_str()).collect();
    let columns = msgs
        .iter()
        .find(|m| m.contains("`columns`"))
        .expect("a leftover `.columns` must warn, not be silent");
    assert!(
        columns.contains("removed") && columns.contains("layout-ncol"),
        "it reads as a removal and names what replaced it: {columns}"
    );
    // NOT a did-you-mean: `codes::extract_suggestion` lifts that phrase into a fix an
    // agent applies mechanically, and this rewrite deletes the child fences rather than
    // renaming anything, so a mechanical rename would break the document differently.
    assert!(
        !msgs.iter().any(|m| m.contains("did you mean")),
        "a removal must never be phrased as a rename: {msgs:?}"
    );
    assert!(
        msgs.iter().any(|m| m.contains("`column`")),
        "the child fences the author must delete are named too: {msgs:?}"
    );

    // The control: the surviving spelling still lays out side by side, so this test
    // cannot pass by the grid having broken everywhere.
    let kept = render_document("::: {layout-ncol=2}\nLeft\n\nRight\n:::\n");
    let kh: String = kept.blocks.iter().map(|b| b.html.as_str()).collect();
    assert!(
        kh.contains("tali-layout") && kh.contains("repeat(2,"),
        "`{{layout-ncol=2}}` is still the two-column grid: {kh}"
    );
    assert!(
        kept.warnings.is_empty(),
        "and it is silent: {:?}",
        kept.warnings
    );
}

#[test]
fn a_block_after_an_empty_div_stays_inside_its_own_container() {
    // Regression (group_divs): the "skip degenerate/empty spans" step ran AFTER the "open
    // containers" step, so a block following an empty div had its own container span skipped
    // over and escaped the div. Here the callout after an empty `.foo` must still wrap its body.
    let doc = render_document("::: {.foo}\n:::\n\n::: {.callout-note}\nInside.\n:::\n");
    let h: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
    assert!(
        h.contains("callout callout-note"),
        "the div after an empty div is still built as a callout: {h}"
    );
    // The body must be INSIDE the callout container, not escaped as a sibling.
    let body_at = h.find("Inside.").expect("body present");
    let callout_at = h
        .find("callout-body")
        .expect("callout body wrapper present");
    assert!(
        callout_at < body_at,
        "the body stays inside the callout container: {h}"
    );
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
fn toc_href_matches_an_explicit_heading_id_containing_an_entity() {
    // The `id` reaches `toc_html` via `extract_attr` over ALREADY-escaped heading HTML,
    // exactly like the entry text above — so escaping it a second time produced a TOC
    // href of `#r&amp;amp;d-notes` against an anchor of `r&amp;d-notes`: a dead link in
    // the *published build*, not just the preview. Auto-slugs strip `&`, which is why
    // only an explicit `{#id}` ever exposed it.
    let doc = render_document("## R&D notes {#r&d-notes}\n\nBody.\n");
    let heading = &doc.blocks[0].html;
    let toc = toc_html(&doc.blocks);
    // The anchor the browser resolves against, read back out of the emitted heading.
    let anchor = extract_attr(heading, "id").expect("heading carries an explicit id");
    assert_eq!(anchor, "r&amp;d-notes", "heading anchor changed: {heading}");
    assert!(
        toc.contains(&format!("href=\"#{anchor}\"")),
        "TOC href must equal the heading's own id, got: {toc}"
    );
    assert!(
        !toc.contains("&amp;amp;"),
        "TOC double-escaped the id: {toc}"
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
        build.contains("__taliMermaidLoading"),
        "Build still ships the loader (uses the inlined global)"
    );
    // Content-gated: a Build page with NO diagram inlines nothing.
    let build_plain = code_scripts_for("<p>no diagram</p>", OutputMode::Build);
    assert!(
        !build_plain.contains("__esbuild_esm_mermaid"),
        "Build must NOT inline mermaid on a diagram-less page"
    );
    // Preview keeps the lean lazy loader (inlining 3,565,102 B on every save would bloat it).
    let preview = code_scripts_for(body, OutputMode::Preview);
    assert!(
        !preview.contains("__esbuild_esm_mermaid") && preview.contains("__taliMermaidLoading"),
        "Preview keeps only the lazy loader, not the inlined library"
    );
}

/// The third delivery, between the two above: a Build page whose caller has undertaken to
/// write the library BESIDE it. `build <file.tmd> --out <dir>` produces a folder whose
/// contract already permits sibling assets, so inlining there bought nothing and cost 16.5x
/// the page. The caller owns the href form, exactly as `ExternalAssets` already does for the
/// multi-page build.
#[test]
fn a_named_mermaid_file_is_linked_instead_of_inlined() {
    let doc = render_document("```mermaid\nflowchart LR\n  A --> B\n```\n");
    let inlined = super::render_doc_to_page(&doc, "stem", crate::OutputMode::Build);
    assert!(
        inlined.contains("__esbuild_esm_mermaid"),
        "the single-file build still inlines"
    );

    let linked = super::render_doc_to_page_mermaid_file(&doc, "stem", "mermaid.min.js");
    assert!(
        !linked.contains("__esbuild_esm_mermaid"),
        "a named sibling file must replace the inlined library, not accompany it"
    );
    assert!(
        linked.contains("s.src = 'mermaid.min.js'"),
        "the lazy loader must fetch the sibling the caller named"
    );
    assert!(
        linked.contains("__taliMermaidLoading"),
        "the loader itself still ships (it is what fetches the sibling)"
    );
    assert!(
        linked.len() * 10 < inlined.len(),
        "linked {} B vs inlined {} B: the library is still in the page",
        linked.len(),
        inlined.len()
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
    // Regression: corpus/tech-blog/posts/pca-geometry writes `# | label:` / `# | echo: false`.

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

/// A `{bash}`/`{sql}`/`{julia}`/… cell is neither render-emitted (mermaid/`{js}`) nor
/// kernel-executed (python/r), so a `label: fig-*` on one can never materialize a
/// figure. It must NOT burn a figure number or register a phantom anchor that shifts
/// every later figure down by one — the classic `@fig-x` resolving to a "Figure 1" no
/// element carries. The cell's source still shows (the author wants to display it), but
/// as a plain code block, and a located warning names the unreferenceable label.
#[test]
fn a_labelled_non_executable_lang_never_phantoms_a_figure_number() {
    let doc = render_document(
        "```{bash}\n#| label: fig-shell\n#| fig-cap: \"Shell\"\necho hi\n```\n\n\
         ![A real one.](r.png){#fig-real}\n\nSee @fig-real.\n",
    );
    let body = doc.body_html();
    // The bash source still shows (visible, not hidden) ...
    assert!(
        strip_tags(&body).contains("echo hi"),
        "the bash source must stay visible: {body}"
    );
    // ... but as a plain code block, NOT a numbered figure carrying the phantom anchor.
    assert!(
        !body.contains("id=\"fig-shell\""),
        "no element must carry the phantom `fig-shell` anchor: {body}"
    );
    assert!(
        !body.contains("Figure&nbsp;1: Shell"),
        "the bash cell must not become a numbered figure: {body}"
    );
    // The real figure keeps number 1: the phantom must not have burned it to Figure 2.
    assert!(
        body.contains("<a href=\"#fig-real\" class=\"tali-xref\">Figure&nbsp;1</a>"),
        "the real figure must stay Figure 1, not be shifted by a phantom: {body}"
    );
    // A located warning names the unreferenceable label AND the reason (the language).
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("fig-shell") && w.message.contains("bash")),
        "expected an unreferenceable-label warning naming the language: {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}

/// The same phantom-anchor defect on the TABLE axis: a table only ever materializes
/// from executed output (a python/r DataFrame), so a `label: tbl-*` on a `{bash}` cell
/// must not burn a table number or register a phantom `@tbl-` anchor. Here a real
/// python table (`c.table` is set at render time, so it numbers even without a kernel)
/// must stay Table 1, not be shifted to Table 2 by the bash cell.
#[test]
fn a_labelled_non_executable_lang_never_phantoms_a_table_number() {
    let doc = render_document(
        "```{bash}\n#| label: tbl-shell\n#| tbl-cap: \"Shell\"\necho hi\n```\n\n\
         ```{python}\n#| label: tbl-real\n#| tbl-cap: \"Real\"\ndf\n```\n\nSee @tbl-real.\n",
    );
    let body = doc.body_html();
    assert!(
        strip_tags(&body).contains("echo hi"),
        "the bash source must stay visible: {body}"
    );
    assert!(
        !body.contains("id=\"tbl-shell\""),
        "no element must carry the phantom `tbl-shell` anchor: {body}"
    );
    assert!(
        body.contains("<a href=\"#tbl-real\" class=\"tali-xref\">Table&nbsp;1</a>"),
        "the real table must stay Table 1, not be shifted by a phantom: {body}"
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("tbl-shell") && w.message.contains("bash")),
        "expected an unreferenceable-label warning naming the language: {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
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
        jh.contains("type=\"application/tali-js\"") && jh.contains("data-name=\"x\""),
        "js cell missing the tali-js script / parsed option: {jh}"
    );
    assert!(
        !jh.contains("//| name"),
        "option lines should be stripped: {jh}"
    );
}

/// A LEADING DOT means display-only: `{.python}` is "the display form for a
/// non-executing block" (docs/guide/using/formats.tmd). Only bare `{python}` executes.
/// The cell gate used to test `starts_with('{')` alone, and `code_lang` strips the dot,
/// so `{.python some-attr="x"}` became an executable cell — it warmed a kernel and spliced
/// an output block under an illustrative snippet. The corpus deck authored exactly that
/// shape over an undefined `values`, so a live kernel baked a real NameError traceback into
/// a slide. Invisible to the kernel-free corpus tests, which only assert the static
/// highlight markup.
#[test]
fn a_leading_dot_fence_is_display_only_and_never_executes() {
    let doc = render_document("```{.python code-line-numbers=\"1|2-3\"}\ntotal = 0\n```\n");
    let b = &doc.blocks[0];
    assert!(
        b.cell.is_none(),
        "a `{{.python}}` fence is display-only and must not become an executable cell"
    );
    // It must still render AS python: the dot only suppresses execution, not highlighting.
    // Assert the highlighter's own class prefix, not `language-python`: that class is
    // emitted whether or not highlighting ran, so it cannot witness this claim. (This
    // assertion previously read `contains("qhl-") || contains("language-python")`; the
    // prefix was renamed to `tali-hl-` long ago, so the first disjunct was dead and the
    // whole check rode on the second, which is furniture.)
    assert!(
        b.html.contains("tali-hl-"),
        "the dot form must still be syntax-highlighted as python: {}",
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
fn execute_flow_mapping_sets_the_document_cache_default() {
    // The inline YAML flow form `execute: {cache: false}` must behave like the block form
    // (`execute:\n  cache: false`). `cache:` is the only sub-key left.
    for src in [
        "---\nexecute: {cache: false}\n---\n\n```{python}\nprint(1)\n```\n",
        "---\nexecute:\n  cache: false\n---\n\n```{python}\nprint(1)\n```\n",
    ] {
        let doc = render_document(src);
        let cell = doc
            .blocks
            .iter()
            .find_map(|b| b.cell.as_ref())
            .expect("a code cell");
        assert!(!cell.cache, "cache: false must reach the cell, for:\n{src}");
    }
    // Absent -> cached.
    let doc = render_document("---\ntitle: T\n---\n\n```{python}\nprint(1)\n```\n");
    assert!(
        doc.blocks
            .iter()
            .find_map(|b| b.cell.as_ref())
            .expect("a code cell")
            .cache
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
fn a_bracketed_fig_reference_resolves_as_a_crossref_not_a_citation() {
    // `[@fig-x]` (bracketed) must resolve as a CROSS-REFERENCE, not fall through the citation
    // renderer. No corpus doc or test used the bracketed cross-ref form (all use the bare
    // `@fig-x`), so the `xref_link` call inside `render_citation_group` was entirely uncovered:
    // dropping it would silently render `[@fig-x]` as a bogus numeric citation + a phantom
    // References entry.
    let doc = render_document("![A fit.](fit.png){#fig-fit}\n\nAs in [@fig-fit].\n");
    let body = doc.body_html();
    assert!(
        body.contains("<a href=\"#fig-fit\" class=\"tali-xref\">Figure&nbsp;1</a>"),
        "bracketed [@fig-fit] must resolve to the numbered figure link: {body}"
    );
    assert!(
        !body.contains("ref-fig-fit"),
        "it must NOT fall through to a citation with a References entry: {body}"
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
    // `application/tali-js` script carrying the source, with `//|` options as data-*.
    let d = render_document("```{js}\n//| name: signalX\nreturn [1, 2, 3];\n```\n");
    let h = &d.blocks[0].html;
    assert!(
        h.contains("class=\"cell tali-js-cell\""),
        "js placeholder: {h}"
    );
    assert!(
        h.contains("<script type=\"application/tali-js\"") && h.contains("data-name=\"signalX\""),
        "tali-js script + name option: {h}"
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

/// A display-math block whose continuation lines begin with `+`, `-` or `*` must
/// still render as one equation. `math_dollars` is an INLINE extension, so a
/// multi-line `$$…$$` lives inside a paragraph — and CommonMark lets a list marker
/// interrupt a paragraph. Before the fix, comrak split the block into a paragraph
/// plus a `<ul>`: the `$$` shipped as literal text, the math never typeset, and the
/// leading operator was swallowed as the bullet marker, so `a = b - c - d` rendered
/// as "a = b" followed by bullets "c" and "d". Silent corruption of the equation's
/// meaning, which is why this is pinned per marker rather than once.
#[test]
fn display_math_survives_list_marker_continuation_lines() {
    for (marker, src) in [
        (
            "+",
            "$$\n\\mathbb{E}\\left[(y - \\hat f(x))^2\\right]\n  = \\operatorname{Bias}^2\n  + \\operatorname{Var}\n  + \\sigma^2\n$$\n",
        ),
        ("-", "$$\na = b\n  - c\n  - d\n$$\n"),
        ("*", "$$\na = b\n  * c\n$$\n"),
        (
            "aligned+",
            "$$\n\\begin{aligned}\nL &= \\sum_i \\log p(x_i)\\\\\n  + \\lambda \\lVert \\theta \\rVert^2\n\\end{aligned}\n$$\n",
        ),
        ("0-indent-", "$$\na = b\n- c\n$$\n"),
    ] {
        let doc = render_document(src);
        let h = doc.body_html();
        assert!(
            h.contains("katex-display"),
            "[{marker}] expected one display-math block, got: {h}"
        );
        assert!(
            !h.contains("<ul"),
            "[{marker}] continuation line was parsed as a list: {h}"
        );
        assert!(
            !h.contains("$$"),
            "[{marker}] `$$` delimiters leaked as literal text: {h}"
        );
    }
}

/// The masking that fixes the case above must not reach into fenced code, where a
/// `$$` line is literal content (documentation that *shows* display math) and a
/// `-` line is a real list in an example.
#[test]
fn display_math_masking_leaves_fenced_code_alone() {
    let doc = render_document("```\n$$\na = b\n- c\n$$\n```\n");
    let h = doc.body_html();
    assert!(
        h.contains("$$"),
        "code block must keep its literal `$$`: {h}"
    );
    assert!(
        h.contains("- c"),
        "code block must keep its literal `- c`: {h}"
    );
    assert!(
        !h.contains("katex"),
        "nothing inside a code fence may be typeset: {h}"
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
    assert_eq!(doc.blocks[0].id, "tali-title-block");
}

#[test]
fn front_matter_without_title_yields_no_blocks() {
    // No title -> no title block, and no body -> empty.
    assert!(render_document("---\nfoo: bar\n---\n").blocks.is_empty());
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
/// The per-document raw-injection family was retired on 2026-08-02. A leftover key must be
/// INERT (nothing reaches the page) and DIAGNOSED — the dangerous outcome for an injection
/// key is the quiet one, where an author believes their analytics snippet is still shipping.
fn retired_front_matter_include_keys_inject_nothing_and_say_so() {
    let src = "---\n\
            title: T\n\
            css:\n  text: |\n    body { color: red }\n\
            include-in-header:\n  text: |\n    <meta name=\"x\" content=\"y\">\n\
            include-before-body:\n  text: |\n    <div id=\"top-banner\"></div>\n\
            include-after-body:\n  text: |\n    <script>window.__after=1</script>\n\
            ---\n\nBody.\n";
    let page = render_html_page(src, "fallback");
    for needle in [
        "<meta name=\"x\" content=\"y\">",
        "top-banner",
        "window.__after=1",
        "body { color: red }",
    ] {
        assert!(
            !page.contains(needle),
            "a retired include key must inject nothing, found {needle:?}"
        );
    }
    let doc = render_document(src);
    for key in [
        "css",
        "include-in-header",
        "include-before-body",
        "include-after-body",
    ] {
        let msg = doc
            .warnings
            .iter()
            .map(|w| &w.message)
            .find(|m| m.contains(&format!("`{key}`")))
            .unwrap_or_else(|| {
                panic!(
                    "silently inert `{key}:` — no diagnostic in {:?}",
                    doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
                )
            });
        assert!(
            msg.contains("removed on 2026-08-02"),
            "`{key}` must say when it went: {msg}"
        );
        assert!(
            msg.contains("head:"),
            "`{key}` must point at the surviving hatch: {msg}"
        );
    }
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

    // `data:image/svg+xml` is NOT a safe inline raster: SVG can embed `<script>`/`onload`,
    // so it is an XSS vector even in an image context and must be neutralized like
    // `data:text/html`, not passed through like png/gif/jpeg/webp/avif. `is_safe_data_image`
    // deliberately omits it; pin that, so re-adding svg to the allow-list can't slip by.
    let svg_img = render_document(
        "![x](data:image/svg+xml;base64,PHN2Zz48c2NyaXB0PmFsZXJ0KDEpPC9zY3JpcHQ+PC9zdmc+)\n",
    );
    assert!(
        !svg_img.blocks[0].html.contains("data:image/svg+xml"),
        "script-bearing data:image/svg+xml leaked into an img src: {}",
        svg_img.blocks[0].html
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
fn standalone_image_becomes_a_numbered_figure() {
    let doc = render_document("![Scree plot](scree.png){#fig-scree width=50%}\n");
    let h = &doc.blocks[0].html;
    assert!(h.starts_with("<figure"), "got: {h}");
    assert!(h.contains("id=\"fig-scree\""), "got: {h}");
    // Centred unconditionally: `fig-align=` was cut on 2026-08-09 (see `emit_figure`).
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
fn a_built_page_ships_no_reader_facing_theme_control() {
    // The device decides the theme and nothing a reader can reach may override it. THREE
    // controls used to, each through a different door, which is why this pins all three at
    // once rather than trusting any one deletion: the Settings gear (server-rendered by
    // `site::chrome`), its floating fallback plus the Theme row (two code-enhance
    // fragments), and the Cmd-K palette's "Toggle light / dark theme" action — that last
    // one self-gated on a global `theme_head` ships in every page, so it was offered on
    // every static build, and removing only the gear would have left it standing.
    let page = render_html_page("# Title\n\nProse to read.\n", "doc");
    // The gear's attribute is JOINED from parts that are not themselves attribute names.
    // `token_contract.rs`'s browser census scans every Rust source containing `<script`,
    // which includes this file, so any `data-…` literal here lands back in the
    // browser-selected set — on a test whose whole point is that the attribute is gone.
    // Interpolating the last segment into a prefix is NOT enough, and that mistake is why
    // this comment spells nothing out: the scanner keeps a trailing `-` on purpose, to
    // catch exactly that kind of concatenation-built name, so the prefix alone registers.
    // Every segment here is therefore inert on its own.
    let gear = ["data", "tali", "settings"].join("-");
    for gone in [
        gear.as_str(),
        "tali-rmenu",
        "taliInitReaderMenu",
        "taliInitReaderPrefs",
        "taliGetThemeChoice",
    ] {
        assert!(
            !page.contains(gone),
            "a built page must carry no reader theme control, found `{gone}`"
        );
    }
    // The CALL form, not the bare identifier: the comment in search.js explaining why the
    // action was removed names the global, and a bare-name needle matched that prose.
    assert!(
        !super::SEARCH_JS.contains("window.taliToggleTheme"),
        "the Cmd-K palette's theme action is offered on every static build; it must be gone"
    );
    assert!(
        !super::SEARCH_JS.contains(r#"id: "theme""#),
        "no palette action may toggle the theme"
    );
}

#[test]
fn the_pre_paint_script_follows_the_device_and_nothing_can_force_it() {
    // The device is the only input to the resolved mode. The OS-change listener is
    // registered UNCONDITIONALLY: it used to be skipped whenever front matter forced a
    // mode, and that state is exactly what no longer exists, so a surviving guard would
    // be a branch that can never be taken.
    let head = theme_head();
    assert!(
        head.contains("prefers-color-scheme"),
        "the pre-paint script must resolve the mode from the device"
    );
    // The picker's read-backs. `taliGetThemePref` had NO consumer anywhere in the tree even
    // before this change; `taliGetThemeChoice` had exactly one, the Theme row.
    for dead in ["taliGetThemePref", "taliGetThemeChoice"] {
        assert!(
            !head.contains(dead),
            "`{dead}` existed only for the picker's read-back and must not survive it"
        );
    }
    assert!(
        !head.contains("choice: choice()"),
        "the tali:themechange detail carried `choice` only so the picker could re-sync"
    );
}

/// Focus/reading mode **and page-level fullscreen are removed** (owner ruling 2026-07-28), and
/// this pins that they stay removed. Focus mode hid `.tali-site-nav` / `.tali-site-footer` /
/// `#TOC` and re-centred the column, but measured against a built book chapter it changed
/// *nothing*: item 76 took the rail, a book's chrome is `.tali-book-topbar` (never
/// `.tali-site-nav`), and its `.tali-book-sidebar` lives inside the `hidden` drawer. The ruling
/// extends that to every page kind, and to fullscreen with it: static chrome is not a
/// distraction, so neither toggle earned its place on a page that is read rather than presented.
///
/// The slide deck kept its own fullscreen until the engine was cut on 2026-08-08; the
/// removed page-level implementation always early-returned on `.tali-deck`, so the two never
/// shared code and this needle was always about the page.
#[test]
fn assembled_page_ships_neither_focus_mode_nor_fullscreen() {
    let page = render_html_page("# Title\n\nProse to read.\n", "doc");
    // `body.tali-focus`, not a bare `tali-focus`: `--tali-focus` is the live focus-RING token
    // in tokens.css, so the bare needle would match on a page that has no focus mode at all.
    for needle in ["taliInitFocusMode", "body.tali-focus", "__taliFocus"] {
        assert!(
            !page.contains(needle),
            "focus mode was removed but `{needle}` is still shipped"
        );
    }
    // `requestFullscreen` gets its own needle rather than joining the loop above, so it is
    // asserted where the content gating is real: a
    // Build-mode page, not `render_html_page`'s Preview bundle, which ships every core
    // enhancer unconditionally so a live-diff edit can gain any construct without a
    // reload.
    //
    // It is asserted against the WHOLE ASSEMBLED PAGE, not against
    // `code_scripts_for` alone: that helper covers only the content-gated enhancer bundle,
    // so page-level fullscreen reintroduced in `web-client/client.js`, in `page.rs`'s own
    // inline bootstrap, or in `tali-js.js` would sail straight past it. A Build-mode
    // assembled prose page contains zero `requestFullscreen`, measured, so the strong form
    // is available and is what runs here.
    let built_prose = page_from_doc(
        &render_document("# Title\n\nProse to read.\n"),
        "doc",
        OutputMode::Build,
    );
    assert!(
        !built_prose.contains("requestFullscreen"),
        "a built page that is not a deck must not ship requestFullscreen: {built_prose}"
    );
}

/// `build <file.tmd>` inlines the framework stylesheet, and until 2026-08-09 it inlined it
/// RAW. Measured on a prose page: 274,966 bytes shipped, 244,662 of them stylesheet, and
/// 41,823 of THAT developer comments addressed to whoever next edits `base.css`. The
/// multi-page build has run the same constants through [`crate::minify_css`] since it grew a
/// shared `_assets/` bundle; the single-file path, the verb a first user reaches for, was
/// simply never pointed at it, so it produced the worse artifact.
#[test]
fn a_standalone_page_minifies_the_css_it_inlines() {
    let page = page_from_doc(
        &render_document("---\ntitle: Prose\n---\n\nJust prose: no math, no cells.\n"),
        "doc",
        OutputMode::Build,
    );
    // The largest `<style>` block, not the first: a page with a `theme:` emits its own
    // small one, and which lands first is a template-ordering detail this test does not own.
    let css = page
        .split("<style>")
        .skip(1)
        .filter_map(|rest| rest.split_once("</style>").map(|(css, _)| css))
        .max_by_key(|css| css.len())
        .expect("a standalone page inlines its framework CSS");

    assert!(
        !css.contains("/*"),
        "the inlined stylesheet still carries developer comments"
    );
    let raw = format!(
        "{}{}{}{}{}",
        super::FONTS_CSS,
        super::TOKENS_CSS,
        super::TOKENS_DARK_CSS,
        super::BASE_CSS,
        super::DARK_CSS
    );
    // Anti-vacuity, and the reason the comment check alone is not enough: most of `raw` is
    // base64 font payload that no minifier can touch, so the saving has to be asserted in
    // bytes or a regression that stripped comments and nothing else would read as a pass.
    // Measured 2026-08-09: 239,499 B raw -> 195,352 B inlined, a 44,147 B saving, almost
    // all of it the comments (41,823 B). Collapsing whitespace buys only ~2 KB, because the
    // minifier collapses runs to a single space rather than deleting space around `{:;`.
    assert!(
        raw.len() > css.len() + 40_000,
        "the inlined CSS ({} B) must be materially smaller than the {} B of constants it is \
         built from",
        css.len(),
        raw.len()
    );
}

#[test]
fn a_retired_theme_mode_cannot_still_force_the_page() {
    // `theme:` stays live for `.css` files and `_extensions/` bundles, so what retired here
    // is two of its VALUES — which no register models, since all three key on the key and
    // not on the value. That leaves the half a register could never derive: the parser
    // still honouring the value. `listing: sort:` answered "deleted" for eleven days while
    // `parse_listing_spec` went on reversing the cards, so this pins the BEHAVIOUR (the
    // page still resolves from the device) and not merely the message.
    for mode in ["dark", "light"] {
        let doc = render_document(&format!("---\ntheme: {mode}\n---\n\nProse.\n"));
        assert!(
            doc.warnings
                .iter()
                .any(|w| w.message.contains("follows the reader's device")),
            "`theme: {mode}` must say it no longer forces a mode"
        );
        assert!(
            doc.theme_css.trim().is_empty(),
            "`theme: {mode}` must not inject override CSS"
        );
    }
    // And the page it produces embeds the SAME pre-paint script as a page with no `theme:`
    // at all. `theme_head()` takes no argument now, so this is what "nothing can force it"
    // reduces to: there is no per-document input left for a mode to enter through.
    let script = theme_head();
    for src in ["---\ntheme: dark\n---\n\nProse.\n", "Prose.\n"] {
        assert!(
            render_html_page(src, "doc").contains(&script),
            "every page must embed the one unforced pre-paint script"
        );
    }
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
fn search_js_ships_the_command_palette_actions() {
    // DX8: Cmd-K runs commands, not only search. The palette must ship the action registry
    // (each action gating on its capability global) + the action branch in go() + the
    // "run a command" placeholder. JS is include_str!'d, so this is the drift guard.
    let js = super::SEARCH_JS;
    for needle in [
        "taliToggleTheme",         // the always-available theme action
        "taliRestartKernel",       // preview-only kernel action
        "taliOpenPageSource",      // preview-only open-in-editor action
        "availableActions",        // the capability-gated action list
        "item.action",             // go()/itemEl() branch on actions vs content
        "Search or run a command", // the palette-not-just-search placeholder
    ] {
        assert!(
            js.contains(needle),
            "search.js missing command-palette wiring: {needle}"
        );
    }
}

#[test]
fn search_js_locks_the_background_scroller_while_the_palette_is_modal() {
    // The palette declares `aria-modal="true"` (taliFocusTrap sets it), which tells a
    // reader nothing behind it is reachable — but a real PageDown scrolled the page 787px
    // underneath it. Whatever else changes, the root `overflow` must be locked on open and
    // put back on close, and it must be put back to the SAVED value: the book drawer locks
    // the same property and Cmd-K opens over it, so restoring `''` would unlock the page
    // under a drawer that is still up.
    let js = super::SEARCH_JS;
    assert!(
        js.contains("document.documentElement.style.overflow = \"hidden\""),
        "search.js opens a modal palette without locking the page scroller"
    );
    assert!(
        js.contains("prevRootOverflow"),
        "search.js must restore the SAVED root overflow, not a hardcoded ''"
    );
    assert!(
        !js.contains("document.documentElement.style.overflow = \"\""),
        "restoring '' unlocks a book drawer that was open before the palette"
    );
}

#[test]
fn theme_head_ships_a_toggle_theme_global() {
    // The preview dev menu's quick toggle calls window.taliToggleTheme. It is defined here
    // rather than in the preview client so the button has one implementation and no
    // duplicated flip logic. The Cmd-K palette shared it until 2026-08-13, which is what
    // made a reader-facing override outlive the Settings gear on every static build.
    assert!(
        theme_head().contains("window.taliToggleTheme"),
        "theme_head must define window.taliToggleTheme for the dev menu's toggle"
    );
}

/// Tab / Shift-Tab move the palette's selection, as the arrow keys do.
///
/// Tab did not previously *escape* the overlay — the shared modal trap in `04-focus-trap.js`
/// already confines it — it was inert, because the input is the palette's only focusable
/// element, so the trap simply cycled focus back to it. Browser-verified on a built book:
/// Tab advances, Shift-Tab reverses, both wrap, and Enter still follows the selection.
#[test]
fn search_js_navigates_results_with_tab_and_shift_tab() {
    // Scan CODE lines only. The comment on the handler explains the Tab behaviour at
    // length, so a whole-file search for `"Tab"` matches the prose rather than the
    // implementation — the exact trap three pins fell into last session.
    let code: String = super::SEARCH_JS
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains("e.key === \"Tab\" && !e.shiftKey"),
        "Tab must advance the palette selection"
    );
    assert!(
        code.contains("e.key === \"Tab\" && e.shiftKey"),
        "Shift-Tab must move the palette selection backwards"
    );
    // The hint bar is the only discovery surface for this, so it must advertise it.
    assert!(
        super::SEARCH_JS.contains("<kbd>tab</kbd> navigate"),
        "the palette hint must advertise tab alongside the arrows"
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
fn a_theme_extension_bundle_cannot_escape_the_project_root() {
    // Item 85: the `_extensions/<name>/theme.css` arm read `base.join(ext)` with no
    // containment at all, while the sibling `.css` arm went through `safe_join_in`. Two
    // shapes escaped: a `../` climb, and an ABSOLUTE name — `Path::join` *replaces* the
    // base on an absolute argument, so `theme: /etc` read `/etc/theme.css` outright. That
    // is item 80's `mounts:` footgun in a second place.
    //
    //   <tmp>/proj/_extensions/ok/theme.css        the legitimate in-project bundle
    //   <tmp>/outside/_extensions/evil/theme.css   a real file OUTSIDE the root
    let uniq = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let tmp = std::env::temp_dir().join(format!("tali-theme-85-{uniq}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let base = tmp.join("proj");
    let outside = tmp.join("outside/_extensions/evil");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::create_dir_all(base.join("_extensions/ok")).unwrap();
    std::fs::write(outside.join("theme.css"), b".escaped{color:red}").unwrap();
    std::fs::write(base.join("_extensions/ok/theme.css"), b".ok{color:green}").unwrap();

    // The positive control, which is what stops this test from passing by refusing
    // everything: a legitimate in-project bundle still loads, and still says nothing.
    let mut w = Vec::new();
    let css = resolve_theme(Some("ok"), Some(&base), Some(&base), &mut w);
    assert!(
        css.contains(".ok"),
        "an in-project extension theme must still load: {css:?}"
    );
    assert!(w.is_empty(), "and must not warn: {w:?}");

    // The climb. `_extensions/` consumes one `..`, so `../../` is what actually leaves
    // `base` — worth spelling out, because `../` alone resolves back inside and would
    // make this assertion pass for the wrong reason.
    let mut w = Vec::new();
    let css = resolve_theme(
        Some("../../outside/_extensions/evil"),
        Some(&base),
        Some(&base),
        &mut w,
    );
    assert!(
        css.is_empty(),
        "a climbing theme bundle must not be read: {css:?}"
    );
    assert!(
        w.iter()
            .any(|x| x.message.contains("outside the project root")),
        "and the refusal is reported, not swallowed: {w:?}"
    );

    // The absolute form. Named as a bare directory (no `.css` suffix) so it lands on the
    // extension arm rather than the file arm.
    let mut w = Vec::new();
    let css = resolve_theme(
        Some(&outside.to_string_lossy()),
        Some(&base),
        Some(&base),
        &mut w,
    );
    assert!(
        css.is_empty(),
        "an absolute theme bundle must not be read: {css:?}"
    );
    assert!(
        w.iter()
            .any(|x| x.message.contains("outside the project root")),
        "and the refusal is reported: {w:?}"
    );

    // A bare unknown name is still silent: it may be a legacy built-in theme taliesin
    // does not ship (`darkly`), which harmlessly falls back to the default. The refusal
    // above must not have turned every miss into a warning.
    let mut w = Vec::new();
    let css = resolve_theme(Some("darkly"), Some(&base), Some(&base), &mut w);
    assert!(css.is_empty() && w.is_empty(), "bare name: {css:?} {w:?}");

    let _ = std::fs::remove_dir_all(&tmp);
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

    // The document-level `execute: cache:` default, both flow and block form.
    assert!(!detect_execute_cache("execute: {cache: off}\n"));
    assert!(!detect_execute_cache("execute:\n  cache: no\n"));
    assert!(detect_execute_cache("execute:\n  cache: true\n"));
    // Absent -> on.
    assert!(detect_execute_cache("title: X\n"));
    // A leftover `echo:`/`include:` (retired 2026-08-02) must not reach `cache`. They warn
    // as unknown `execute` sub-keys; here the point is that they change nothing.
    assert!(detect_execute_cache(
        "execute:\n  echo: off\n  include: no\n"
    ));
    assert!(!detect_execute_cache(
        "execute:\n  echo: off\n  cache: off\n"
    ));
}

#[test]
fn both_palettes_ship_on_every_page_and_are_selected_by_data_theme() {
    // Neither built-in palette is ever a per-page override: both ship in every page and the
    // pre-paint script picks one by setting `data-theme`. That is what makes following the
    // device a runtime decision with no rebuild, and it is why removing the picker could
    // not have changed which colours a page carries.
    let page = render_html_page("# T\n\nx\n", "fb");
    assert!(
        page.contains("html[data-theme=\"dark\"]"),
        "scoped dark CSS not shipped"
    );
    assert!(page.contains("--tali-bg: #16181d"), "dark vars missing");
    assert!(
        render_document("---\ntitle: x\n---\n\nx\n")
            .theme_css
            .is_empty(),
        "a page with no custom theme must inline no override CSS"
    );
}

#[test]
fn theme_list_takes_first_entry() {
    // `theme: [brand.css, extra.scss]` (list form) selects the base; the rest are layers.
    assert_eq!(
        detect_theme("theme: [brand.css, extra.scss]\n").as_deref(),
        Some("brand.css"),
        "first list entry should win"
    );
}

#[test]
fn footnotes_emit_ref_and_a_margin_sidenote_at_the_reference() {
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
    // The ref link is a `doc-noteref` (PA-M8), not a bare number, for AT.
    assert!(
        page.contains("role=\"doc-noteref\" href=\"#fn-1\""),
        "footnote ref carries doc-noteref role: {page}"
    );
    // Owner ruling 2026-08-01: margin placement is the DEFAULT on a wide screen, not a
    // `footnotes:` knob. The note's content therefore sits inline *immediately after its
    // own reference*, which is the only place CSS can float it into the margin from —
    // there is no selector that relocates an end-of-document `<li>` next to an arbitrary
    // earlier reference, so the move has to happen at render time.
    assert!(
        page.contains("</sup><span class=\"tali-sidenote\" id=\"fn-1\""),
        "the note must follow its own reference immediately: {page}"
    );
    assert!(
        page.contains("role=\"doc-footnote\""),
        "the note is a doc-footnote for AT: {page}"
    );
    assert!(page.contains("The supporting note"), "footnote body");
    // The number the reader sees in the margin, so a note is identifiable when several
    // stack up beside one paragraph.
    assert!(
        page.contains("<span class=\"tali-sidenote-num\">1</span>"),
        "the sidenote shows its number: {page}"
    );
    // One copy, in one place. A gathered endnote list in addition to the margin note
    // would put every note's text in the DOM twice, which Ctrl-F and all four text
    // projections (`taliesin read`, skim, the search index, llms-full.txt) would each
    // report twice.
    //
    // These two run against the BODY, not the page: every page inlines the whole CSS/JS
    // payload, so a `.footnotes` or `.tali-fn-back` rule left behind in `base.css` is
    // shipped bytes and would satisfy a whole-page `contains()` — the inlined-asset
    // needle trap, which bites a NEGATIVE assertion just as hard as a positive one.
    let body =
        render_document("A claim.[^1] More text.\n\n[^1]: The supporting note.\n").body_html();
    assert!(
        !body.contains("class=\"footnotes\""),
        "no gathered endnote section survives the margin placement: {body}"
    );
    assert!(
        !body.contains("tali-fn-back"),
        "a note beside its reference needs no backlink: {body}"
    );
}

#[test]
fn a_repeat_reference_to_one_note_renders_the_note_once() {
    // comrak numbers repeat references `fnref-a-1`, `fnref-a-2`, … while the visible
    // index stays the note's own. Only the FIRST reference carries the content, or the
    // margin would show the same note twice and the `id="fn-a"` would be duplicated in
    // the DOM (silently breaking the `href="#fn-a"` anchor the other references use).
    let page = render_html_page(
        "---\ntitle: T\n---\n\nOne[^a] and again[^a].\n\n[^a]: Note A.\n",
        "fb",
    );
    assert_eq!(
        page.matches("class=\"tali-sidenote\"").count(),
        1,
        "exactly one sidenote for two references: {page}"
    );
    assert!(page.contains("id=\"fnref-a-2\""), "second ref still emits");
}

#[test]
fn a_sidenote_is_the_locatable_unit_for_click_to_source() {
    // Same contract the gathered `<li>` used to carry, moved to the element that now
    // holds the note. client.js `locatable()` matches
    // `closest("[data-tali-src], [data-block-id]")`, so without `data-block-id` a
    // Ctrl-click on a note walks up to the enclosing paragraph and lands on the
    // paragraph's line — silently the wrong line, not a no-op. The sourcepos is the
    // DEFINITION's line (where the author wrote `[^a]: …`), not the reference's.
    let doc = render_document("Claim.[^a]\n\nFiller.\n\n[^a]: Note A.\n");
    let html = doc.body_html();
    assert!(
        html.contains(
            "<span class=\"tali-sidenote\" id=\"fn-a\" role=\"doc-footnote\" \
             data-block-id=\"fn-a\" data-sourcepos=\"5:1-5:13\""
        ),
        "sidenote must carry its definition's block-id + sourcepos: {html}"
    );
}

#[test]
fn editing_a_footnote_definition_changes_its_referencing_block_id() {
    // The load-bearing one, and the reason this is not a pure-CSS change. A block id is
    // hashed from the block's SOURCE LINES (`make_id`), and the note now renders inside
    // the referencing block rather than as a block of its own. So unless the definition's
    // source feeds that hash, editing a note leaves every block id identical, the diff
    // emits no op, and the preview silently keeps showing the old note.
    let before = render_document("Claim.[^a]\n\n[^a]: Note A.\n");
    let after = render_document("Claim.[^a]\n\n[^a]: Note A, revised.\n");
    let id_before = &before.blocks[0].id;
    let id_after = &after.blocks[0].id;
    assert_ne!(
        id_before, id_after,
        "editing the note must re-key the block that displays it, or the live \
         preview never updates"
    );
    // And an edit to an UNRELATED note must not churn this block's id (which would
    // replace, and so reset the live state of, a paragraph nothing changed in).
    let unrelated = render_document("Claim.[^a]\n\nOther.[^b]\n\n[^a]: Note A.\n\n[^b]: B2.\n");
    let unrelated2 = render_document("Claim.[^a]\n\nOther.[^b]\n\n[^a]: Note A.\n\n[^b]: B3.\n");
    assert_eq!(
        unrelated.blocks[0].id, unrelated2.blocks[0].id,
        "an edit to note b must not re-key the block that only references note a"
    );
}

#[test]
fn each_sidenote_resolves_to_its_own_definition_line() {
    // Definitions sit on lines 7 and 11, scattered between prose rather than bunched at
    // the end, so this is the case a single first-note-wins sourcepos could not serve.
    // Each note must land on the line the author wrote IT on.
    let src = "---\ntitle: T\n---\n\nFirst claim.[^a]\n\n[^a]: Note A.\n\nSecond claim.[^b]\n\n[^b]: Note B.\n";
    let html = render_document(src).body_html();
    assert!(
        html.contains(
            "id=\"fn-a\" role=\"doc-footnote\" data-block-id=\"fn-a\" data-sourcepos=\"7:1-7:13\""
        ),
        "note A must carry its own block-id + sourcepos: {html}"
    );
    assert!(
        html.contains(
            "id=\"fn-b\" role=\"doc-footnote\" data-block-id=\"fn-b\" data-sourcepos=\"11:1-11:13\""
        ),
        "note B must carry its own block-id + sourcepos: {html}"
    );
}

#[test]
fn a_sidenote_lives_inside_the_block_that_references_it() {
    // The consequence that makes the whole design work, and the one a refactor is most
    // likely to break: the note is not a block of its own any more. It must sit INSIDE
    // the referencing block's html — a sibling block placed after the paragraph would
    // float into the margin beside whatever follows, one paragraph off from the
    // reference it belongs to.
    let doc = render_document("Claim.[^a]\n\nA second paragraph.\n\n[^a]: Note A.\n");
    assert!(
        doc.blocks[0].html.contains("Note A."),
        "the note belongs to the referencing block: {}",
        doc.blocks[0].html
    );
    assert!(
        !doc.blocks.iter().any(|b| b.id == "tali-footnotes"),
        "no gathered footnotes block survives"
    );
    // Monotonic source order still holds with no gathered trailing block in the way.
    assert_eq!(doc.blocks.len(), 2, "one paragraph each, no note block");
}

#[test]
fn a_footnote_definition_with_block_content_is_flattened_and_warns() {
    // The note now renders as phrasing content inside its referencing block, so a
    // definition carrying a list / code block / quote cannot keep its structure: an
    // `<ul>` inside a `<span>` inside a `<p>` makes the HTML parser close the `<p>`
    // early, which would split the block into two root elements and break the
    // one-root-element invariant every block carries. Flatten to the paragraphs' inline
    // content and SAY SO, rather than emitting a DOM the block model cannot address.
    let doc = render_document("Claim.[^a]\n\n[^a]: Lead in.\n\n    - a list item\n");
    let html = doc.body_html();
    assert!(
        !html.contains("<ul>") && !html.contains("<li>"),
        "block content inside a sidenote must not survive as block markup: {html}"
    );
    assert!(
        doc.warnings.iter().any(|w| w.message.contains("footnote")),
        "a flattened note must warn: {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}

#[test]
fn column_margin_div_renders_with_class() {
    // `::: {.column-margin}` is a margin note (styled via base.css float); it just needs
    // to emit a `.column-margin` block carrying the usual data-block-id (click-to-source).
    // `.sidenote`/`.marginnote`/`.aside` were retired aliases of this same block (visual
    // minimalism pass, task 13, 2026-08-03); `retired_names.rs` pins that a leftover one
    // still renders (unstyled) and warns with a removal note, through the full pipeline.
    let page = render_html_page(
        "---\ntitle: T\n---\n\n::: {.column-margin}\nA margin note.\n:::\n",
        "fb",
    );
    assert!(
        page.contains("class=\"column-margin\""),
        "column-margin div: {page}"
    );
    assert!(page.contains("A margin note"), "margin note content");
}

#[test]
fn a_dated_post_wraps_its_reading_content_in_an_article_landmark() {
    // PA-M2: a dated post is a self-contained syndicatable unit — an <article> landmark inside
    // <main>. An undated page (listing / section / generic) stays plain <main> content.
    let post = render_html_page(
        "---\ntitle: A Post\ndate: 2026-05-15\n---\n\nBody text here.\n",
        "fb",
    );
    assert!(
        post.contains("<main id=\"tali-main\" tabindex=\"-1\">\n<article>"),
        "the article opens right inside <main>: {post}"
    );
    assert!(
        post.contains("</article>\n</main>"),
        "the article closes just inside </main>: {post}"
    );

    let page = render_html_page("---\ntitle: A Page\n---\n\nBody text here.\n", "fb");
    assert!(
        !page.contains("<article>"),
        "an undated page is not an article landmark: {page}"
    );
}

#[test]
fn input_slider_shortcode_emits_reactive_control() {
    let doc = render_document_with_includes(
        "{{< input name=\"k\" type=\"slider\" min=\"1\" max=\"10\" value=\"3\" label=\"k\" >}}\n",
        std::path::Path::new("."),
    );
    let h = doc.body_html();
    assert!(h.contains("class=\"tali-input\""), "wrapper: {h}");
    assert!(h.contains("data-tali-input=\"k\""), "named input: {h}");
    assert!(h.contains("type=\"range\""), "range control: {h}");
    assert!(h.contains("min=\"1\"") && h.contains("max=\"10\"") && h.contains("value=\"3\""));
    assert!(
        h.contains("<output class=\"tali-input-out\" for=\"tali-in-k\" data-tali-out>3</output>"),
        "slider readout, tied to its control via for= (PA-M9): {h}"
    );
    assert!(h.contains(">k</label>"), "label: {h}");
}

/// The `data-block-id` of a `tali-input` block, extracted from rendered `body_html`.
fn tali_input_block_id(h: &str) -> String {
    let key = "class=\"tali-input\" data-block-id=\"";
    let i = h.find(key).expect("a tali-input block") + key.len();
    h[i..].split('"').next().unwrap().to_string()
}

#[test]
fn input_control_id_is_position_independent() {
    let p = std::path::Path::new(".");
    let input = "{{< input name=\"rate\" type=\"slider\" min=\"0\" max=\"20\" value=\"8\" >}}\n";
    let top = render_document_with_includes(input, p).body_html();
    let shifted =
        render_document_with_includes(&format!("A leading paragraph.\n\n{input}"), p).body_html();
    // The control id is derived from the reactive name, not the source line, so it is the
    // same whether the input sits at the top or is shifted down by an edit above.
    assert!(
        top.contains("id=\"tali-in-rate\""),
        "name-based control id at top: {top}"
    );
    assert!(
        shifted.contains("id=\"tali-in-rate\""),
        "name-based control id when shifted: {shifted}"
    );
    // Therefore the input block's content-hash `data-block-id` is stable across the shift —
    // the invariant a live deck re-mount / incremental diff relies on to keep control state.
    assert_eq!(
        tali_input_block_id(&top),
        tali_input_block_id(&shifted),
        "the input block's data-block-id must be position-independent"
    );
}

#[test]
fn duplicate_input_names_get_deduped_control_ids() {
    let p = std::path::Path::new(".");
    let h = render_document_with_includes(
        "{{< input name=\"rate\" type=\"slider\" >}}\n\n{{< input name=\"rate\" type=\"slider\" >}}\n",
        p,
    )
    .body_html();
    // Two controls can bind the same reactive name (e.g. the same control on two slides);
    // their DOM ids must still be unique, so the second dedups with a `-N` suffix.
    assert!(h.contains("id=\"tali-in-rate\""), "first control id: {h}");
    assert!(
        h.contains("id=\"tali-in-rate-1\""),
        "second deduped control id: {h}"
    );
}

#[test]
fn input_shortcode_other_types_emit_their_native_control() {
    let p = std::path::Path::new(".");
    let num =
        render_document_with_includes("{{< input name=\"n\" type=\"number\" step=\"0.1\" >}}\n", p)
            .body_html();
    assert!(num.contains("type=\"number\"") && num.contains("step=\"0.1\""));
    assert!(
        !num.contains("data-tali-out"),
        "no readout on number: {num}"
    );

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
fn a_duplicate_cross_reference_label_warning_is_located() {
    // A repeated `{#sec-x}`/`{#fig-x}`/`{#tbl-x}` is a duplicate cross-reference label.
    // The warning must carry the DUPLICATE's source line for click-to-source — like the
    // "duplicate heading id" warning right beside it already does — not the unlocated
    // string that half-reproduces the Quarto flaw D53 critiques (§2 #1).
    let doc = render_document("## First {#sec-dup}\n\n## Second {#sec-dup}\n\nSee @sec-dup.\n");
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
    // "First definition wins" is the whole point, not just the warning: the registry keeps the
    // FIRST section's number (1), and the bare `sec-dup` id stays on that first heading. A silent
    // last-wins regression stores the second number (2) while the id still points at the first —
    // the exact "link and number disagree, no diagnostic" flaw D53 critiques (§2 #1). Resolve the
    // reference so a regression here fails, instead of the warning firing over a wrong number.
    assert_eq!(
        doc.xref_numbers.get("sec-dup").map(String::as_str),
        Some("1"),
        "duplicate label must keep the first definition's number, got: {:?}",
        doc.xref_numbers
    );
    assert!(
        doc.body_html()
            .contains("<a href=\"#sec-dup\" class=\"tali-xref\">Section&nbsp;1</a>"),
        "@sec-dup must resolve to the first Section 1: {}",
        doc.body_html()
    );
}

#[test]
/// The opt-in prose linter was retired on 2026-08-02. A leftover `prose-lint:` must produce
/// the retired-key diagnostic and NO lint warnings — the failure to avoid is a document
/// that still looks linted because the key parsed.
fn prose_lint_is_retired_and_lints_nothing() {
    let doc = render_document("---\ntitle: T\nprose-lint: true\n---\n\nThis is very very good.\n");
    assert!(
        !doc.warnings.iter().any(|w| {
            w.message.starts_with("weasel word ")
                || w.message.starts_with("repeated word ")
                || w.message.starts_with("banned term ")
        }),
        "no lint rule may still fire: {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("`prose-lint`")
                && w.message.contains("removed on 2026-08-02")),
        "the retired key must say so: {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}

/// Reading time still works: `prose::word_count` is the surviving half of `prose.rs` and it
/// has many consumers (the title block here, the book chapter-cost signal, the LSP outline,
/// `map`). Removing the linter must not have taken the selection walk with it.
#[test]
fn reading_time_still_counts_prose() {
    let doc = render_document("---\ntitle: T\ndate: 2026-01-01\n---\n\nOne two three four five.\n");
    assert!(
        doc.body_html().contains("min read"),
        "a dated post still gets its reading-time estimate:\n{}",
        doc.body_html()
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

/// Does a tsc `include`/`exclude` pattern cover `path` (project-relative)? Supports the
/// one wildcard form these configs use: `*` matching within a single path segment.
///
/// Deliberately minimal. It exists so the gate below can assert coverage **per file**
/// against a *globbed* config, rather than degrading to "the config mentions a `*`
/// somewhere" — which would be a drift test that cannot fail, the thing this file's own
/// rule forbids.
fn tsc_pattern_covers(pattern: &str, path: &str) -> bool {
    let (pat_segs, path_segs): (Vec<&str>, Vec<&str>) =
        (pattern.split('/').collect(), path.split('/').collect());
    if pat_segs.len() != path_segs.len() {
        return false;
    }
    pat_segs
        .iter()
        .zip(&path_segs)
        .all(|(pat, seg)| match pat.split_once('*') {
            Some((head, tail)) => {
                seg.starts_with(head) && seg.ends_with(tail) && seg.len() >= head.len() + tail.len()
            }
            None => pat == seg,
        })
}

#[test]
fn every_code_enhance_fragment_is_in_the_type_check_gate() {
    // A fragment added to the concat! but not reached by `jsconfig.json` ships unchecked
    // while the `tsc` gate still reports success. Found by adding one: `18-media.js` had
    // been outside the gate since it landed. Read the config mechanically rather than
    // trusting one assertion per file — the same lesson the CLI help gate learned (nine
    // undocumented flags where the audit had filed two).
    //
    // The config is GLOBBED now (item 98), which is what makes a new fragment covered on
    // arrival instead of on remembering — but this test still checks every fragment
    // individually, against the include patterns *and* the excludes. Asserting merely that
    // a `*` appears would pass for a glob pointing at the wrong directory.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/js");
    let cfg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("jsconfig.json")).unwrap())
            .expect("jsconfig.json is valid JSON");
    let patterns = |key: &str| -> Vec<String> {
        cfg[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let (includes, excludes) = (patterns("include"), patterns("exclude"));
    assert!(
        !includes.is_empty(),
        "jsconfig.json declares an include list"
    );

    let mut names: Vec<String> = std::fs::read_dir(dir.join("code-enhance"))
        .expect("assets/js/code-enhance should exist")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".js"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "the fragment directory cannot be empty");
    for name in &names {
        let rel = format!("code-enhance/{name}");
        assert!(
            includes.iter().any(|p| tsc_pattern_covers(p, &rel)),
            "assets/js/jsconfig.json must cover {rel}, or it ships type-unchecked"
        );
        assert!(
            !excludes.iter().any(|p| tsc_pattern_covers(p, &rel)),
            "{rel} is covered by an include but cancelled by an exclude, which is worse \
             than not listing it: it looks checked and is not"
        );
    }

    // The matcher must be able to say NO, or every assertion above is free.
    assert!(tsc_pattern_covers(
        "code-enhance/*.js",
        "code-enhance/19-book-outline.js"
    ));
    assert!(!tsc_pattern_covers("code-enhance/*.js", "deck.js"));
    assert!(!tsc_pattern_covers(
        "*.js",
        "code-enhance/19-book-outline.js"
    ));
    assert!(tsc_pattern_covers("*.min.js", "d3.min.js"));
    assert!(!tsc_pattern_covers("*.min.js", "deck.js"));
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
        prose.contains("function taliCopyText"),
        "build keeps code-enhance.js"
    );
    assert!(
        !prose.contains("self-contained enhancer module"),
        "no mermaid.js on a prose page"
    );
    assert!(
        !prose.contains("a tiny enhancer that replaces the vendored"),
        "no tali-js.js on a prose page"
    );
    // A page that actually carries the DOM gets that enhancer in a build (but still not
    // the enhancers for constructs it lacks).
    let diagram = code_scripts_for("<pre class=\"mermaid\">graph TD;</pre>", OutputMode::Build);
    assert!(
        diagram.contains("self-contained enhancer module"),
        "a diagram on the page ships mermaid.js"
    );
    assert!(
        !diagram.contains("a tiny enhancer that replaces the vendored"),
        "still no tali-js.js"
    );

    // Preview ships every enhancer regardless of body (a doc can gain any construct on
    // an edit — same reasoning as KaTeX/d3 always-on in preview). Gating is Build-only.
    let preview = code_scripts_for("<p>Just prose.</p>", OutputMode::Preview);
    assert!(
        preview.contains("self-contained enhancer module"),
        "preview ships mermaid.js unconditionally"
    );
    assert!(
        preview.contains("a tiny enhancer that replaces the vendored"),
        "preview ships tali-js.js unconditionally"
    );
}

#[test]
fn site_build_path_content_gates_enhancers() {
    // The in-site page builder hardcodes OutputMode::Build, so a site/book build
    // content-gates the separate enhancers just like a single-doc build (this pins
    // the spec's "site builds get Phase-1 gating too" claim). Each marker is that
    // script's own distinctive comment rather than its filename, so a string that also
    // occurs in base.css cannot make the negative assertion pass for the wrong reason.
    let doc = render_document("# A chapter\n\nProse only — no mermaid diagram.\n");
    let page = html_page_from_doc_in_site(&doc, "chapter", &SiteCtx::default());
    assert!(
        page.contains("function taliCopyText"),
        "a site page still ships code-enhance.js (the copy buttons + a11y layer)"
    );
    assert!(
        !page.contains("self-contained enhancer module"),
        "no mermaid.js on a prose site page"
    );
}

/// Cell output is bounded in CSS, so one runaway cell cannot take the page's scrollbar
/// hostage. Three shapes, three deliberate decisions: a `<pre>` and a table SCROLL, an image
/// is SCALED, and print lifts every bound (paper has no scrollbar, and a clipped traceback
/// on paper is unrecoverable).
#[test]
fn cell_output_is_bounded_and_the_bound_lifts_in_print() {
    let css = BASE_CSS;
    // Per RULE, not "the token appears somewhere": three rules use it, so a whole-file
    // search still passes with the `<pre>` bound deleted — which is the one that matters
    // most (a runaway traceback is the case this exists for).
    let pre_rule = css
        .split(".tali-output > pre {")
        .nth(1)
        .and_then(|s| s.split("\n  .tali-output > table").next())
        .expect("the output pre rule exists");
    assert!(
        pre_rule.contains("max-height: var(--tali-output-max);"),
        "a runaway <pre> is bounded by the token, not a literal: {pre_rule}"
    );
    assert!(
        css.contains(".tali-output > table { display: block; max-height: var(--tali-output-max); overflow: auto; }"),
        "a long result table is bounded too, not just a <pre>"
    );
    assert!(
        css.contains("max-height: var(--tali-output-max); object-fit: contain;"),
        "an image is scaled to its bound, not scrolled"
    );
    // The bound must NOT be `hidden="until-found"`: its reveal is Chrome-only, and the
    // fallback elsewhere is `display: none`, which makes a traceback uncopyable and drops it
    // from print. Strip comments first — the rule's own comment explains that choice by
    // naming the thing it rejects, and a raw substring search matches the explanation.
    let declarations: String = css
        .split("/*")
        .map(|s| s.split_once("*/").map(|(_, rest)| rest).unwrap_or(s))
        .collect();
    let out_rules = declarations
        .split(".tali-output")
        .skip(1)
        .collect::<String>();
    assert!(
        !out_rules.contains("until-found"),
        "cell output must not be hidden with until-found"
    );
    assert!(
        css.contains(".tali-output > pre, .tali-output > table, .tali-output img {\n      max-height: none; overflow: visible; }"),
        "print lifts every --tali-output-max-bounded output pane"
    );
}

/// The output `<pre>`'s fade is a NEW vertical one. The generic `<pre>`'s horizontal
/// scroll-shadow must be untouched by it — they are different axes on different elements,
/// and the rule this replaced used the `background` shorthand, which had already reset the
/// generic layers on an output `<pre>` anyway.
#[test]
fn the_output_fade_is_vertical_and_the_code_fade_stays_horizontal() {
    let css = BASE_CSS;
    let output = css
        .split(".tali-output > pre {")
        .nth(1)
        .and_then(|s| s.split("\n  .tali-output > table").next())
        .expect("the output pre rule exists");
    assert!(
        output.contains("linear-gradient(to bottom,") && output.contains("at top,"),
        "the output fade runs top-to-bottom: {output}"
    );
    assert!(
        output.contains("background-size: 100% 38px"),
        "a vertical fade sizes its bands by height: {output}"
    );
    // The generic `pre` (code INPUT) keeps its horizontal shadow, unchanged.
    let generic = css
        .split("\n  pre { position: relative;")
        .nth(1)
        .and_then(|s| s.split("\n  code {").next())
        .expect("the generic pre rule exists");
    assert!(
        generic.contains("linear-gradient(to right,") && generic.contains("at left,"),
        "code input keeps its left/right scroll shadow: {generic}"
    );
    assert!(
        generic.contains("background-size: 38px 100%"),
        "…sized by width, i.e. still horizontal: {generic}"
    );
}

/// The hex value of `token` inside the first block after `selector` in `css`. The block
/// matters: a stylesheet can define `--tali-bg` more than once (a `:root` and a themed
/// override), so a first-match scan would silently compare against the wrong palette.
#[cfg(test)]
fn token_hex_in(css: &str, selector: &str, token: &str) -> String {
    let block = css
        .find(selector)
        .unwrap_or_else(|| panic!("no `{selector}` block in the token CSS"));
    let rest = &css[block..];
    let at = rest
        .find(&format!("{token}:"))
        .unwrap_or_else(|| panic!("no `{token}:` after `{selector}`"));
    let after = &rest[at + token.len() + 1..];
    let h = after
        .find('#')
        .unwrap_or_else(|| panic!("no hex value for `{token}` after `{selector}`"));
    after[h..h + 7].to_ascii_lowercase()
}

/// PA-C5, half two: the pre-paint head script sets the canvas (and the mobile `theme-color`)
/// from its own `BG` map, because it runs BEFORE any stylesheet parses — that is the whole
/// point of it, and it is why the values cannot be `var(--tali-bg)`. Unlocked, a token change
/// showed up as a one-frame flash of the OLD background on every navigation, which is exactly
/// the bug the script exists to prevent.
#[test]
fn the_pre_paint_canvas_map_tracks_the_theme_tokens() {
    let head = super::theme::theme_head();
    // Each row names the selector its OWN file keys the palette on. `tokens-dark.css` has
    // no `:root` block: it is `html[data-theme="dark"]` throughout, and naming `:root` here
    // matched a mention inside a comment that happened to sit directly above the real
    // block. Editing that comment (wave 11) is what surfaced it.
    for (mode, css, selector) in [
        ("dark", TOKENS_DARK_CSS, "html[data-theme=\"dark\"]"),
        ("light", TOKENS_CSS, ":root"),
    ] {
        let want = token_hex_in(css, selector, "--tali-bg");
        // Matched as the map entry (`dark: '#16181d'`), so a hex that merely appears somewhere
        // else in the script cannot satisfy it.
        let entry = format!("{mode}: '{want}'");
        assert!(
            head.to_ascii_lowercase().contains(&entry),
            "the pre-paint BG map must read `{entry}` (from `--tali-bg`); it does not"
        );
    }
}

/// Item 200: sepia was dropped, leaving light + dark. This is the whole-surface pin, because
/// the theme is assembled from five separate places that each had their own sepia branch and
/// a leftover in any one of them is either a dead rule or a picker offering a mode nothing
/// paints. Light and dark are read alongside as the control: a test that only asserts an
/// absence passes just as well on an empty stylesheet.
#[test]
fn sepia_is_gone_from_every_theme_surface() {
    for (what, css) in [
        ("tokens.css", TOKENS_CSS),
        ("tokens-dark.css", TOKENS_DARK_CSS),
        ("base.css", BASE_CSS),
        ("dark.css", DARK_CSS),
    ] {
        assert!(
            !css.to_ascii_lowercase().contains("sepia"),
            "{what} still mentions sepia; it was removed (item 200)"
        );
    }
    // The two that remain, read from the same files, so "no sepia" cannot be satisfied by
    // an empty or renamed stylesheet.
    assert!(TOKENS_CSS.contains("--tali-bg"), "the light root survives");
    assert!(
        TOKENS_DARK_CSS.contains("--tali-bg"),
        "the dark palette survives"
    );

    // The persisted-choice validator: a stored `sepia` must fall back to following the
    // device rather than name a mode nothing paints.
    let head = super::theme::theme_head();
    assert!(
        !head.to_ascii_lowercase().contains("sepia"),
        "the head theme script still knows sepia: {head}"
    );
    assert!(
        head.contains("light") && head.contains("dark"),
        "control: the head script still knows the two modes that remain"
    );
    assert!(
        !CODE_ENHANCE_JS.contains("sepia"),
        "the reader menu still offers a Sepia row"
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

/// `data-tali-cell` marks an EXECUTED cell's source listing, and only that: it is what the
/// reader's show/hide-code control targets, so what it does *not* match is as load-bearing
/// as what it does.
///
/// The distinction had no representation in the built HTML before this — `is_cell` was
/// computed in `emit.rs` and discarded, and the preview's `data-tali-cell-state` is added at
/// runtime by `client.js`, so a built page carried nothing. Byte-level snapshots do not
/// cover it either: `body_html_snapshots` is deliberately `{js}`-only, and a `{js}` cell
/// emits a script shell rather than a `<pre>`, so this path has no snapshot to drift.
#[test]
fn only_a_cells_source_listing_is_marked_for_the_reader_code_toggle() {
    let doc = render_document(
        "```{python}\n#| label: probe\nprint(1)\n```\n\n```python\nnot_a_cell = 1\n```\n",
    );
    let cell = &doc.blocks[0].html;
    let fence = &doc.blocks[1].html;
    assert!(
        cell.contains("data-tali-cell=\"python\""),
        "a cell's listing is marked, and carries its language: {cell}"
    );
    assert!(
        !fence.contains("data-tali-cell"),
        "a plain fence is prose the author wrote to be read, not a cell: {fence}"
    );
}

/// A folded cell keeps the marker, and keeps it on the `<details>` — the element that
/// actually wraps the listing. On the `<pre>` inside it, hiding would collapse the code and
/// leave a bare disclosure triangle behind.
#[test]
fn a_folded_cell_carries_the_marker_on_the_element_that_wraps_the_listing() {
    let doc = render_document("```{python}\n#| code-fold: true\nprint(1)\n```\n");
    let html = &doc.blocks[0].html;
    let at = html
        .find("data-tali-cell=")
        .expect("a folded cell is still a cell");
    let open = html[..at].rfind('<').expect("an enclosing tag");
    assert!(
        html[open..].starts_with("<details"),
        "the marker belongs on the <details> that wraps the listing, not the inner <pre>: {}",
        &html[open..(open + 60).min(html.len())]
    );
}

/// Cross-document view transitions ship in the BUNDLE, so every multi-page project gets a
/// crossfade between pages without authoring a stylesheet (C-NAV-1; promoted out of the
/// blog's `custom.css`).
///
/// **The assertion is the NESTING, not the presence.** `@view-transition` opts navigation
/// in, so the only way to honor `prefers-reduced-motion` is to not opt in — there is
/// nothing a `reduce` override could override. A future edit that unnests the at-rule
/// would still "contain @view-transition" while silently animating for a reader who asked
/// for no motion, so this walks back from the at-rule to the query that must enclose it.
#[test]
fn cross_document_view_transitions_ship_bundled_and_respect_reduced_motion() {
    // The RULE, not the token: the comment above it names `@view-transition` in prose, and
    // a bare-token search finds that first and then walks back to the wrong media query.
    let at = BASE_CSS
        .find("@view-transition {")
        .expect("base.css declares @view-transition");
    let enclosing = BASE_CSS[..at]
        .rfind("@media")
        .expect("@view-transition sits inside a media query");
    assert!(
        BASE_CSS[enclosing..at].starts_with("@media (prefers-reduced-motion: no-preference)"),
        "the nearest enclosing query must be the reduced-motion opt-in, got: {}",
        &BASE_CSS[enclosing..at]
    );
    assert!(
        BASE_CSS[at..].starts_with("@view-transition { navigation: auto; }"),
        "same-origin navigation is what opts in: {}",
        &BASE_CSS[at..(at + 60).min(BASE_CSS.len())]
    );
}

/// The UI-boundary token must clear the WCAG 1.4.11 3:1 floor against BOTH surfaces a control
/// can sit on: the page background and the code background (a `kbd` and the copy button sit on
/// code-bg). The hairline `--tali-border` stays decorative and is deliberately not checked.
#[test]
fn border_strong_clears_the_ui_boundary_floor_on_both_surfaces() {
    for (theme, css, bg, code_bg) in [
        ("light", TOKENS_CSS, "#ffffff", "#f5f5f5"),
        ("dark", TOKENS_DARK_CSS, "#16181d", "#21242b"),
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
/// link tone is 1.93:1 in dark, the default theme. WCAG 1.4.1.
#[test]
fn xref_links_carry_a_non_colour_affordance() {
    let i = BASE_CSS.find(".tali-xref {").expect("the .tali-xref rule");
    let rule = &BASE_CSS[i..i + 160];
    assert!(
        rule.contains("text-decoration: underline"),
        "xref/citation links must be underlined, not colour-only: {rule}"
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

/// A theme that does not override `--tali-flash` inherits the `:root` value, which is how the
/// live-edit pulse once painted an indigo wash onto a warm page. Pin that every theme that is
/// not the root defines its own.
#[test]
fn every_theme_defines_its_own_flash_tint() {
    assert!(
        TOKENS_CSS.contains("--tali-flash:"),
        "the light root defines it"
    );
    assert!(
        TOKENS_DARK_CSS.contains("--tali-flash:"),
        "dark must define it too, rather than inheriting the light value"
    );
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
        ("#4c6ef5", "the retired deck's fourth blue"),
        ("#b00020", "Material Design's error red"),
        ("#2bb673", "the old callout tip green"),
        ("#e0a800", "the old callout warning amber"),
        ("#e0566b", "the old callout important red"),
        ("#e8730c", "the old callout caution orange"),
    ];
    for (sheet, css) in [
        ("tokens.css", TOKENS_CSS),
        ("tokens-dark.css", TOKENS_DARK_CSS),
        ("base.css", BASE_CSS),
        ("dark.css", DARK_CSS),
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

/// The theme owns TWO faces and no more. `--tali-font-head` was `ui-sans-serif, system-ui`
/// with 20 reads across four files: headings and chrome rendered in whatever the reader's OS
/// shipped, so the page had a different voice on every platform and two of its three voices
/// were not the tool's. Headings now take the body serif; labels take the mono.
#[test]
fn the_theme_owns_exactly_two_faces_and_no_system_ui() {
    for (name, css) in [
        ("tokens.css", TOKENS_CSS),
        ("tokens-dark.css", TOKENS_DARK_CSS),
        ("base.css", BASE_CSS),
        ("dark.css", DARK_CSS),
        ("site.css", SITE_CSS),
    ] {
        assert!(
            !css.contains("--tali-font-head"),
            "{name} still reads --tali-font-head; headings take the body serif now"
        );
        assert!(
            !css.contains("system-ui"),
            "{name} still names system-ui; the theme owns its faces"
        );
    }
    assert!(TOKENS_CSS.contains(r#"--tali-font-body: 1.25rem/1.55 "Literata""#));
    assert!(TOKENS_CSS.contains(r#"--tali-font-mono: "JetBrains Mono""#));
}

/// The geometry and motion scales must describe what the sheets actually contain. The old
/// token file advertised "three roundness tiers, three elevation shadows, two motion
/// durations" while the sheets held three radii, ONE shadow and ONE duration.
#[test]
fn the_geometry_scale_is_one_radius_no_shadows_one_duration() {
    assert_eq!(TOKENS_CSS.matches("--tali-radius").count(), 1);
    assert!(!TOKENS_CSS.contains("--tali-shadow"));
    assert!(!TOKENS_CSS.contains("--tali-dur-slow"));
    for (name, css) in [("base.css", BASE_CSS), ("site.css", SITE_CSS)] {
        assert!(
            !css.contains("box-shadow"),
            "{name} still draws a box-shadow"
        );
        assert!(
            !css.contains("backdrop-filter"),
            "{name} still blurs a sticky bar"
        );
    }
}

/// One `--tali-scrim` token single-sources the "dim behind an overlay" backdrop, which used to
/// carry drifted black alphas per sheet. Folded to one token; no raw scrim literal survives
/// (each literal string was unique to its own backdrop rule). PA-F2.
///
/// **base.css dropped out of the per-sheet loop 2026-08-03**, visual minimalism pass: its only
/// two dimmed-overlay surfaces, the lightbox and the mobile TOC sheet, were both deleted (the
/// second in the same pass that added this note), and nothing else in base.css is a full-screen
/// modal backdrop. The deck's share modal left with the deck engine on 2026-08-08, so
/// site.css's book drawer is the last backdrop the token single-sources.
#[test]
fn overlay_backdrops_share_the_scrim_token() {
    assert_eq!(
        TOKENS_CSS.matches("--tali-scrim:").count(),
        1,
        "--tali-scrim must be defined exactly once, in tokens.css :root"
    );
    assert!(
        SITE_CSS.contains("var(--tali-scrim)"),
        "site.css's overlay backdrop must reference var(--tali-scrim)"
    );
    assert!(
        !SITE_CSS.contains("rgba(0, 0, 0, .38)"),
        "site.css still ships the raw .38 book-drawer scrim; route it through --tali-scrim"
    );
}

/// The motion scale is exactly ONE duration (`--tali-dur`; `--tali-dur-slow` is gone, zero
/// consumers). A bare `.15s` once crept in as an undocumented second value; the guard stays
/// as a shape check, comparing `.15s` against `1.15s` so a distinct literal duration (an
/// intentional special, not part of the scale) is never mistaken for the stray. Neither
/// base.css nor site.css carries either today. PA-S3.
#[test]
fn no_stray_15s_duration_outside_the_motion_scale() {
    assert_eq!(
        BASE_CSS.matches(".15s").count(),
        BASE_CSS.matches("1.15s").count(),
        "base.css has a bare .15s transition; fold it to var(--tali-dur)"
    );
    assert!(
        !SITE_CSS.contains(".15s"),
        "site.css carries a stray .15s duration; fold it to var(--tali-dur)"
    );
}

/// Layout breakpoints are rem, so they scale with the reader's text-zoom; a `640px` width query
/// (the hero in base, one query in site) diverged under zoom. Every `@media` width in base/site
/// is expressed in rem. PA-F4.
#[test]
fn layout_breakpoints_are_rem_not_px() {
    for (sheet, css) in [("base.css", BASE_CSS), ("site.css", SITE_CSS)] {
        for line in css.lines().filter(|l| l.trim_start().starts_with("@media")) {
            assert!(
                !(line.contains("width:") && line.contains("px")),
                "{sheet} has a px width breakpoint (use rem so it scales with text-zoom): {}",
                line.trim()
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
    for (theme, css, bg, fg) in [
        ("light", TOKENS_CSS, "#ffffff", "#1a1a1a"),
        ("dark", TOKENS_DARK_CSS, "#16181d", "#e6e6e6"),
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

/// The exec output + diagnostic boxes (`.tali-stderr`/`.tali-error`/`.tali-js-error`) derive
/// their surface from the callout family (a color-mix of the callout token over the page bg,
/// identical to the matching callout's title tint) instead of per-theme literals. Body text on
/// that tint clears AA in every theme by the callout-contrast test, so no per-theme override is
/// needed and the print token-reset reaches them. Pin both halves so a future edit can't silently
/// re-introduce the ~6 per-theme literals PL12 removed.
#[test]
fn diagnostic_boxes_derive_from_callout_tokens_not_per_theme_literals() {
    assert!(
        BASE_CSS.contains("color-mix(in srgb, var(--tali-callout-warning) 13%, var(--tali-bg))"),
        ".tali-stderr must derive its surface from --tali-callout-warning over the bg"
    );
    assert!(
        BASE_CSS.contains("color-mix(in srgb, var(--tali-callout-important) 12%, var(--tali-bg))"),
        ".tali-error/.tali-js-error must derive their surface from --tali-callout-important"
    );
    // No per-theme (dark) `.tali-stderr`/`.tali-error` override survives.
    for (name, css) in [("dark.css", DARK_CSS), ("base.css", BASE_CSS)] {
        assert!(
            !css.contains("] .tali-stderr {") && !css.contains("] .tali-error {"),
            "{name} must not re-add a per-theme .tali-stderr/.tali-error override"
        );
    }
}

#[test]
fn print_and_high_contrast_blocks_outrank_every_theme_block() {
    // `dark.css` is inlined AFTER `base.css` (see page.rs), so a print/contrast override
    // written as `html[data-theme="dark"]` ties that theme block on specificity (0,1,1) and
    // LOSES on source order: printing from dark mode put #e6e6e6 ink on white paper.
    // Any `html[data-theme=…]` block (0,1,1) likewise outranks a bare `:root` (0,1,0), so a
    // themed page never got the `prefers-contrast: more` boost. A doubled `:root:root`
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
        "prefers-contrast: more must reach dark too, not only light"
    );
}

#[test]
fn printing_forces_the_light_theme_even_from_dark() {
    // The CSS override above only resets the *tokens*. `dark.css` also recolours the syntax
    // scopes (`.tali-hl-string` -> #a5d6ff, 1.6:1 on white paper), which are NOT tokenised.
    // Swapping `data-theme` to light for the duration of the print job neutralises them: the
    // same trick deck.js already uses. (The diagnostic boxes are now token-derived, so the
    // token reset already reaches those.)
    let head = theme_head();
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
    // Batch 3b: the comment token was sub-AA (light 4.17) on its code background. Pin
    // >= 4.5:1 against the actual code-block backgrounds so a future palette edit can't
    // silently regress it.
    let light = color_after(BASE_CSS, ".tali-hl-comment { color: ");
    assert!(
        wcag_contrast(light, "#f5f5f5") >= 4.5,
        "light comment {light} vs #f5f5f5 = {:.2}",
        wcag_contrast(light, "#f5f5f5")
    );
    let dark = color_after(DARK_CSS, ".tali-hl-comment { color: ");
    assert!(
        wcag_contrast(dark, "#21242b") >= 4.5,
        "dark comment {dark} vs #21242b = {:.2}",
        wcag_contrast(dark, "#21242b")
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

/// PA-F3: keyboard focus on a listing card missed the pointer-hover lift/border/title-tint,
/// so a keyboard reader tabbing the blog got no card affordance. The `:focus-visible` state
/// must mirror the `:hover` one.
#[test]
fn listing_card_gets_a_focus_visible_affordance() {
    assert!(
        SITE_CSS.contains(".tali-card:focus-visible")
            || SITE_CSS.contains(".tali-card:hover, .tali-card:focus-visible"),
        "a listing card must show its hover affordance on keyboard focus too"
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
    // And the ref resolves (it was registered) — not a dangling data-tali-xref marker.
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
fn h5_and_h6_are_not_dimmer_than_body_text() {
    // SKIM-1: h5/h6 were both `--tali-muted`, making the two DEEPEST headings the
    // lowest-contrast text on the page — lighter than the prose they introduce. It
    // compounds with title-block demotion, which pushes an author's `####` into `<h5>`.
    let css = BASE_CSS;
    let h5 = css
        .lines()
        .find(|l| l.trim_start().starts_with("h5 {"))
        .expect("base.css must style h5");
    let h6 = css
        .lines()
        .find(|l| l.trim_start().starts_with("h6 {"))
        .expect("base.css must style h6");
    // The h5 rule wraps to a second line; check the whole declaration block.
    let h5_block = &css[css.find(h5).unwrap()..css.find(h6).unwrap()];
    assert!(
        !h5_block.contains("--tali-muted"),
        "h5 must not render dimmer than body text: {h5_block}"
    );
    assert!(
        !h6.contains("--tali-muted"),
        "h6 must not render dimmer than body text: {h6}"
    );
}

#[test]
fn print_un_collapses_the_nested_toc_lists() {
    // SKIM-1: on screen `#TOC ul ul` is hidden and the scrollspy expands only the active
    // branch. On paper there is no scrollspy and no active section, so a printed chapter
    // showed 2 of 8 entries and silently dropped the rest.
    // base.css has several `@media print` blocks; the rule may live in any of them.
    assert!(
        BASE_CSS
            .split("@media print")
            .skip(1)
            .any(|blk| blk.contains("#TOC ul ul") && blk.contains("display: block")),
        "a print block must un-collapse #TOC ul ul"
    );
}

#[test]
fn the_scrollspy_derives_its_line_from_scroll_margin_not_the_website_navbar() {
    // SKIM-1: `line()` measured `.tali-site-nav`, which is the WEBSITE chrome — a book
    // emits `.tali-book-topbar` instead, so the query returned 0 on every book page and
    // the highlight lagged a whole section. `scroll-margin-top` is already correct under
    // both chromes (and on a standalone page).
    // Match the CODE, not the word: the explanatory comment names the old selector.
    assert!(
        !TOC_SPY_JS.contains("querySelector(\".tali-site-nav\")"),
        "the spy must not measure the website-only navbar"
    );
    assert!(
        TOC_SPY_JS.contains("scrollMarginTop"),
        "the spy must derive its activation line from scroll-margin-top"
    );
}

#[test]
fn the_palettes_empty_state_is_the_whole_book_outline_not_a_chapter_list() {
    // SKIM-2 Ship A: with no query a book used to show only its level-0 page entries — the
    // same flat chapter list the drawer already shows — leaving every section record in the
    // index reachable only by typing a query that happened to match it.
    // Match the CODE, not the word: the explanatory comment names the old filter.
    assert!(
        !SEARCH_JS.contains("it.level === 0"),
        "the empty state must not filter the index down to page entries"
    );
    // The outline is only readable if a section indents under its chapter, and the indent
    // must be relative to the PAGE's own shallowest heading: whether a chapter's sections
    // land on h2, h3 or h4 depends on whether it emits a title block and where it roots.
    assert!(
        SEARCH_JS.contains("shallowest[it.url"),
        "outline depth must be measured against the page's own shallowest heading"
    );
}

/// PA-B14: the palette is the longest list in the app and had no jump-to-the-ends, which the
/// cite tabs and the deck menu both have. Gated on an EMPTY input on purpose: focus never
/// leaves the input (aria-activedescendant), so with a query typed Home/End are the caret's
/// keys — the binding may only add where it takes nothing away, which is also exactly where
/// the list is longest (the empty state is the whole book's outline).
#[test]
fn the_palette_jumps_to_the_list_ends_only_when_the_caret_has_no_work() {
    assert!(
        SEARCH_JS.contains("e.key === \"Home\"") && SEARCH_JS.contains("e.key === \"End\""),
        "the palette must bind Home/End to the ends of its list"
    );
    assert!(
        SEARCH_JS.contains("!input.value"),
        "Home/End must yield to caret motion whenever a query is typed"
    );
}

#[test]
fn the_palette_relaxes_and_for_content_but_never_for_actions() {
    // One mistyped word used to annihilate the result set (`else return 0`). Content now
    // keeps >=1-term matches and says what it missed; actions must NOT, because they are
    // scored by the same function and pinned above content, so a relaxed action would put
    // "Toggle light / dark theme" above the prose the reader asked for.
    assert!(
        SEARCH_JS.contains("score(a, terms, true)"),
        "command-palette actions must be scored with hard AND"
    );
    assert!(
        SEARCH_JS.contains("missing.push(term)"),
        "content must record the terms it could not match instead of rejecting"
    );
}

#[test]
fn the_palettes_fuzzy_tier_forgives_a_transposition() {
    // Plain Levenshtein charges an adjacent swap ("teh" for "the") two edits, so the single
    // most common typo class was the one the `within1` tier could never forgive.
    assert!(
        SEARCH_JS.contains("charCodeAt(j + 1)") && SEARCH_JS.contains("charCodeAt(i + 1)"),
        "within1 must compare the swapped pair, not just substitute/insert/delete"
    );
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
    // The shift is relative to the page's shallowest heading (AP7-1), so a `#`-rooted page
    // is the one that can push a body `######` past `<h6>`. It has nowhere lower to go, so
    // it stays `<h6>` (never `<h7>`) and the deepest two levels collapse together.
    let doc = render_document("---\ntitle: T\n---\n\n# Top\n\n###### Deep\n");
    assert!(
        doc.blocks[1].html.starts_with("<h2 "),
        "got: {}",
        doc.blocks[1].html
    );
    assert!(
        doc.blocks[2].html.starts_with("<h6 "),
        "got: {}",
        doc.blocks[2].html
    );
    // A LONE `######` is that page's whole outline, so it is its top level and emits as
    // `<h2>` under the title — the shift is negative here, which an absolute `+1` could
    // never do (it left the page reading `h1` then `h6`).
    let lone = render_document("---\ntitle: T\n---\n\n###### Deep\n");
    assert!(
        lone.blocks[1].html.starts_with("<h2 "),
        "got: {}",
        lone.blocks[1].html
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
fn body_uses_the_inlined_literata_face() {
    let doc = render_document("Body prose.\n");
    let page = super::render_doc_to_page(&doc, "stem", crate::OutputMode::Build);
    // Two real @font-face rules for the owned body face, family "Literata".
    assert!(page.contains("@font-face"), "no @font-face in page head");
    assert!(
        page.contains("\"Literata\""),
        "Literata @font-face family missing"
    );
    // A true italic face (not synthesized) alongside the normal one.
    assert!(
        page.contains("font-style: italic"),
        "italic Literata face missing"
    );
    // Inlined as a data URI (offline, self-contained), never a bare url(fonts/…) that
    // would 404 since there is no served font path.
    assert!(
        page.contains("url(data:font/woff2;base64,"),
        "font not inlined as a data URI"
    );
    assert!(
        !page.contains("url(fonts/literata"),
        "a bare font url leaked into the page (would 404)"
    );
}

/// The bundled faces are the two the theme owns, and nothing else. `fonts.css` must name
/// each one exactly as it sits on disk, because `build.rs`'s inliner matches the literal
/// `url(fonts/<name>.woff2)` and SILENTLY leaves an unmatched reference uninlined — which
/// ships a page that fetches a font that is not there.
#[test]
fn the_bundled_faces_are_literata_and_jetbrains_mono() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("assets/fonts")
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "woff2").then(|| p.file_name()?.to_str().map(String::from))?
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "jetbrains-mono-latin-wght-normal.woff2".to_string(),
            "literata-latin-wght-italic.woff2".to_string(),
            "literata-latin-wght-normal.woff2".to_string(),
        ],
        "the bundled woff2 set changed"
    );
    for n in &names {
        assert!(
            FONTS_CSS_LINKED.contains(&format!("url(fonts/{n})")),
            "fonts.css does not reference {n} in the exact form build.rs inlines"
        );
    }
    // The other direction: disk -> CSS above only proves every file on disk is referenced.
    // A stray `url(fonts/typo.woff2)` added to fonts.css (a name that never landed on disk,
    // or a leftover from a rename) would pass that loop while shipping a page that fetches
    // a font build.rs never inlines. One `@font-face` src per vendored file, no more.
    // (Matches on `"src: url(fonts/"`, not the bare `"url(fonts/"` substring, because the
    // file's own doc comment names that pattern in prose.)
    assert_eq!(
        FONTS_CSS_LINKED.matches("src: url(fonts/").count(),
        names.len(),
        "fonts.css references a different number of fonts than assets/fonts/ holds"
    );
    assert!(
        !FONTS_CSS_LINKED.to_ascii_lowercase().contains("newsreader"),
        "Newsreader is retired; it must not be referenced"
    );
}

// Marker literals below are each confirmed present via grep before use (see the Task 1
// report): base.css -> ".tali-title-block", dark.css -> the dark-theme mermaid override
// selector, site.css -> ".tali-book-topbar" (site-only chrome), the code-enhance bundle
// -> "function taliCopyText" (defined in 01-registry.js), search.js -> "function
// buildIndex", mermaid.min.js -> the esbuild wrapper var, d3.min.js -> its source-map
// comment header, plot.umd.min.js -> its own header comment.

#[test]
fn shared_site_css_bundles_the_framework_sheets() {
    let css = shared_site_css();
    assert!(css.contains(".tali-title-block"), "base.css missing");
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
    // `import("./helper.js")`. It stays inline on the page instead (page.rs `tali_js_inline`).
    // `"tali-js cell error:"` is a literal unique to tali-js.js.
    assert!(
        !js.contains("tali-js cell error:"),
        "the {{js}}-cell runtime must stay inline, not in the shared app.js"
    );
}

/// PL10: a `{js}` runtime throw must not leak a raw stack trace to readers of BUILT output.
/// The error box shows the full stack only in the live preview (client.js defines
/// `taliOpenPageSource`); built/published pages get a terse themed message. The full error is
/// always kept in `console.error` for the author.
#[test]
fn js_cell_error_hides_the_stack_trace_in_built_output() {
    let js = TALIESIN_JS;
    assert!(
        js.contains("typeof window.taliOpenPageSource === \"function\"")
            && js.contains("This interactive element couldn't load."),
        "the {{js}}-cell error box must degrade to a terse message when not in the live preview"
    );
    assert!(
        js.contains("console.error(\"tali-js cell error:\", e)")
            && js.contains("String((e && e.stack) || e)"),
        "the full stack must remain in console.error + the preview branch"
    );
}

/// A `{js}` cell added mid-session gets its libraries (audit finding 04).
///
/// `assemble_html_page` gated the vendored d3 + Observable Plot on `has_js_cells(p.body)`
/// in BOTH asset modes, with no reference to `OutputMode`. In a live preview that body is
/// whatever the page had when the tab loaded, so the edit that adds a page's FIRST `{js}`
/// cell hot-swaps a cell that calls `Plot.plot(...)` into a document whose head has no
/// Plot: measured `typeof d3 === "undefined"` on that edit and `"object"` after a manual
/// reload. `code_scripts_for`'s own doc comment asserted the opposite ("the always-on
/// KaTeX/d3 in preview"), so the claim was in the tree and only the behaviour was missing.
///
/// Preview is a loopback dev server and the bytes are already on disk; the correctness of a
/// live edit outranks a first-paint saving that only ever applied to a page the author is
/// in the act of adding a cell to. **Build stays content-gated** — that is the path a
/// reader pays for, and it is correct there because the body is final.
///
/// Asserted on the emitted shell rather than in a browser: wave 6 removed the headless
/// browser net, so this pins the server-side property the client depends on.
#[test]
fn preview_ships_the_js_libs_before_the_page_has_a_js_cell() {
    const NO_CELLS: &str = "<p data-block-id=\"b-1\">Prose only, no cells at all.</p>";
    assert!(
        !has_js_cells(NO_CELLS),
        "the fixture must have no {{js}} cells, or this test proves nothing"
    );

    // Preview: the libs ship anyway, so the NEXT edit can add a cell that uses them.
    let preview = assemble_html_page(&PageParts {
        mode: OutputMode::Preview,
        body: NO_CELLS,
        ..PageParts::defaults()
    });
    assert!(
        preview.contains("d3.min.js") || preview.contains("__d3"),
        "preview must ship the vendored d3/Plot bundle before a cell needs it"
    );
    assert!(
        preview.contains(&js_cell_head()),
        "preview must emit js_cell_head() unconditionally"
    );

    // Build: still content-gated. A reader's page must not carry ~490 KB it cannot use.
    let build = assemble_html_page(&PageParts {
        mode: OutputMode::Build,
        body: NO_CELLS,
        ..PageParts::defaults()
    });
    assert!(
        !build.contains(&js_cell_head()),
        "a static build with no {{js}} cell must NOT inline d3 + Plot"
    );

    // …and a Build page that DOES have a cell still gets them, so the gate still works.
    //
    // The body comes from a REAL render, not a hand-written marker. `token_contract.rs`'s
    // browser-selected census scans every Rust source containing `<script` — this file
    // among them — and it reads raw text, so writing one of those attribute names in a
    // fixture (or even naming the family in a comment) enters it into the vocabulary the
    // browser is pinned against and fails a census that has nothing to do with `{js}`
    // assets. Same trap as `gate_script.rs`'s scan of the interpreter-gate names.
    // Rendering the cell also tests the marker the emitter really produces rather than
    // this test's guess at it.
    let cell_body = render_document("```{js}\nreturn 1;\n```\n").body_html();
    assert!(
        has_js_cells(&cell_body),
        "the rendered fixture must contain a {{js}} cell: {cell_body}"
    );
    let with_cell = assemble_html_page(&PageParts {
        mode: OutputMode::Build,
        body: &cell_body,
        ..PageParts::defaults()
    });
    assert!(
        with_cell.contains(&js_cell_head()),
        "the Build content-gate must still fire on a page that has a {{js}} cell"
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

// --- input-capability gating (the 2026-07-26 mobile audit's root cause) --------------
//
// Every decision about whether to show a keyboard hint, a hover-revealed control or a
// presenter tool used to be made from viewport WIDTH or from deck layout MODE. Both are
// proxies for "is this a touch device" and both fail the same way: a phone in landscape,
// or a phone in stepped mode, is treated as a desktop. The fix is the ordinary
// `hover`/`pointer` media features, so these tests assert the rules live INSIDE a
// capability query rather than merely existing somewhere in the sheet.
//
// Needling the bare selector would pass vacuously — every one of these selectors already
// appears in its sheet (that is the bug: it appears UNGATED). So each assertion slices the
// capability block out by brace matching first and needles only inside it. Restoring the
// bug (moving a rule back out of the block) fails the named test.

/// The body of the first `@media <query> {` block in `css`, by brace matching.
///
/// Returns `None` when the query is absent, which is the failure these tests are for.
fn media_block<'a>(css: &'a str, query: &str) -> Option<&'a str> {
    let at = css.find(&format!("@media {query}"))?;
    let open = at + css[at..].find('{')?;
    let mut depth = 0usize;
    for (i, c) in css[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&css[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn media_block_slices_a_balanced_block_and_reports_a_missing_query() {
    // The helper is the load-bearing half of the tests below; a broken slicer would make
    // every one of them pass vacuously (it would report the whole sheet, or nothing).
    let css = "a { x: 1 }\n@media (hover: none) { b { y: 2 } @supports (z: 1) { c { w: 3 } } }\nd { v: 4 }";
    let body = media_block(css, "(hover: none)").expect("query present");
    assert!(body.contains("b { y: 2 }"), "inner rule sliced: {body}");
    assert!(body.contains("c { w: 3 }"), "nested block kept: {body}");
    assert!(!body.contains("d { v: 4 }"), "stopped at the close: {body}");
    assert!(!body.contains("a { x: 1 }"), "did not start early: {body}");
    assert!(
        media_block(css, "(pointer: coarse)").is_none(),
        "absent query"
    );
}

#[test]
fn search_kbd_badge_is_hidden_on_touch_at_any_width() {
    // MOB-3: `.tali-search-kbd` was hidden only under `max-width: 40rem`, and the rule's own
    // comment states the intent as CAPABILITY ("meaningless on a touch phone"). 40rem misses
    // every phone in landscape and every tablet, which then render a literal "Ctrl K".
    let block = media_block(SITE_CSS, "(hover: none) and (pointer: coarse)")
        .expect("site.css has no input-capability query");
    assert!(
        block.contains(".tali-search-kbd"),
        "the Cmd-K badge is still gated on width alone:\n{block}"
    );
}

/// Regression pin for a Task 3 fix-round bug (visual minimalism pass, 2026-08-04): deleting
/// the mobile pull-up TOC sheet fixed the single-document layout (base.css's
/// `body.has-toc > #TOC { order: -1; position: static; ... }`, in its own `has-toc`
/// narrow-width block) but missed the SITE layout, which uses a different container
/// (`.tali-site-main.has-toc`, not `body.has-toc`). The sheet used to make `#TOC`
/// `position: fixed`, so its place in the DOM (after `<main>`) never mattered; once the
/// sheet was gone, a site page's on-page TOC rendered below the entire article at narrow
/// widths with no lift to correct it.
///
/// **This pins the CSS RULE'S PRESENCE, not its rendered effect**: it cannot see that the
/// TOC actually paints above the article (that needs a browser; verified by hand at
/// 390x844 and 1440x900 against a built `corpus/tech-blog` page and a `docs/guide` book
/// chapter for this fix). It only proves the declarations a regression would delete are
/// still in the sheet.
#[test]
fn site_toc_gets_the_same_narrow_width_order_lift_as_the_single_document_layout() {
    let block = media_block(SITE_CSS, "(max-width: 60rem)")
        .expect("site.css has no has-toc narrow-width query");
    assert!(
        block.contains(".tali-site-main.has-toc > #TOC"),
        "the site layout's narrow-width block no longer targets its own #TOC:\n{block}"
    );
    assert!(
        block.contains("order: -1"),
        "the lift above <main> is gone, so #TOC (which follows <main> in the DOM) would \
         strand at the bottom of the article again:\n{block}"
    );
    assert!(
        block.contains("position: static"),
        "#TOC would stay sticky/positioned instead of dropping into the in-flow stack:\n{block}"
    );
}

#[test]
fn hover_revealed_copy_controls_stay_reachable_without_a_hover() {
    // MOB-4: the control sat at `opacity: 0`, revealed only by `:hover`/`:focus-visible`, with
    // no `hover: none` fallback — so on a phone it was invisible (and copy-code is arguably
    // MORE valuable there, with no easy selection across a scrolling `<pre>`).
    let block =
        media_block(BASE_CSS, "(hover: none)").expect("base.css has no input-capability query");
    assert!(
        block.contains(".tali-copy"),
        "copy-code is still hover-only, so it is invisible on touch:\n{block}"
    );
    // Presence is NOT enough, and this half of the test exists because presence passed while
    // the fix did nothing. Both the override and `.tali-copy`'s own `opacity: 0` declaration
    // sit at (0,1,0), so the cascade is decided purely by source order. Assert the block
    // comes LAST.
    let zero_copy = BASE_CSS
        .find(".tali-copy { position: absolute")
        .expect("no .tali-copy base declaration");
    let gate = BASE_CSS
        .find("@media (hover: none) {")
        .expect("no capability block");
    assert!(
        gate > zero_copy,
        "the capability block is above an `opacity: 0` of equal specificity, so the cascade \
         silently discards it (gate at {gate}, .tali-copy at {zero_copy})"
    );
}

#[test]
fn book_topbar_title_truncates_instead_of_wrapping_the_sticky_bar_taller() {
    // MOB-8: `.tali-book-brand` is `display: block` with no `min-width: 0`, so as a flex item
    // its default `min-width: auto` refuses to shrink below content and the title WRAPS —
    // measured 3 lines / 77px (13% of the viewport) at 240px, on a bar that is sticky, so the
    // cost is subtracted from every screen of reading.
    //
    // Scoped to the topbar on purpose: `.tali-book-brand` is emitted TWICE (chrome.rs:245 in
    // the topbar, :291 in the drawer's sidebar head), and the drawer heading has room to wrap.
    let rule = SITE_CSS
        .split(".tali-book-topbar .tali-book-brand")
        .nth(1)
        .and_then(|r| r.split('}').next())
        .expect("no topbar-scoped .tali-book-brand rule");
    for prop in [
        "min-width: 0",
        "white-space: nowrap",
        "text-overflow: ellipsis",
    ] {
        assert!(
            rule.contains(prop),
            "topbar title is missing `{prop}`, so it wraps instead of truncating:\n{rule}"
        );
    }
}

#[test]
fn book_drawer_close_button_clears_the_wcag_tap_target_floor() {
    // MOB-5(c): measured 26x22px — under the 24px WCAG 2.5.8 AA floor on the height axis.
    // Severity is bounded (backdrop tap and Escape both dismiss, verified in the audit), but
    // it is the only dismiss control a reader can see and aim at.
    let rule = SITE_CSS
        .split(".tali-book-drawer-close {")
        .nth(1)
        .and_then(|r| r.split('}').next())
        .expect("no .tali-book-drawer-close rule");
    assert!(
        rule.contains("min-width: 24px") && rule.contains("min-height: 24px"),
        "close control is under the 24px AA floor:\n{rule}"
    );
    // MOB-5(a), the other half of the scroll lock: `overscroll-behavior: auto` let a scroll
    // INSIDE the panel chain to the page once the list hit either end.
    let panel = SITE_CSS
        .split(".tali-book-drawer-panel {")
        .nth(1)
        .and_then(|r| r.split('}').next())
        .expect("no .tali-book-drawer-panel rule");
    assert!(
        panel.contains("overscroll-behavior: contain"),
        "panel scroll still chains to the page:\n{panel}"
    );
}

#[test]
fn touch_nav_tap_target_grows_without_growing_the_sticky_bar() {
    // MOB-7. The obvious fix is `min-height: 44px` on `.tali-nav-link`, and it is wrong:
    // browser-measured at 844x390 touch, it took the STICKY navbar from 52px to 75px —
    // 13.3% to 19.2% of a landscape phone's viewport. That trades an already-AA-passing
    // 26px target for permanent reading height on the device with the least of it, which
    // is exactly MOB-8's defect in different clothes. The target is expanded by a centred
    // overlay instead, which reaches into the row's existing padding and moves nothing.
    let block = media_block(SITE_CSS, "(hover: none) and (pointer: coarse)")
        .expect("site.css has no input-capability query");
    assert!(
        block.contains(".tali-nav-link::before"),
        "no tap-target overlay on touch nav links:\n{block}"
    );
    let link_rule = block
        .split(".tali-nav-link {")
        .nth(1)
        .and_then(|r| r.split('}').next())
        .unwrap_or("");
    assert!(
        !link_rule.contains("min-height"),
        "`min-height` on the nav link grows the sticky bar (measured 52px -> 75px); use the \
         overlay:\n{link_rule}"
    );
}

/// A `collapse="true"` callout must show a disclosure caret in BOTH states.
///
/// `.callout-title` is `display: flex`, and a `<summary>` renders the browser's own
/// disclosure marker only at `display: list-item` — so before this rule a collapsed
/// callout was a title bar with no indicator whatsoever, and an OPEN collapsible one
/// was indistinguishable from a plain non-collapsible callout.
///
/// Asserted against BASE_CSS, never a rendered page: every page inlines the whole
/// stylesheet, so a page-level `contains` would pass on a page with no callouts.
#[test]
fn collapsible_callouts_carry_a_disclosure_caret() {
    let sel = ".callout-collapse > details > summary.callout-title";
    assert!(
        BASE_CSS.contains(&format!("{sel}::after")),
        "collapsible callouts need a trailing ::after caret"
    );
    assert!(
        BASE_CSS.contains(&format!("{sel}::-webkit-details-marker")),
        "Safari's native marker must be suppressed too"
    );
    // The caret trails, which is what distinguishes it from the leading `::before`
    // carets on folded code and proofs. Without `margin-left: auto` it would sit
    // flush against the title text instead of at the right edge of the tinted bar.
    let i = BASE_CSS
        .find(&format!("{sel}::after"))
        .expect("the callout caret rule");
    let rule = &BASE_CSS[i..i + BASE_CSS[i..].find('}').expect("closing brace")];
    assert!(
        rule.contains("margin-left: auto"),
        "the caret must trail: {rule}"
    );
    assert!(
        rule.contains("rotate(45deg)"),
        "closed state points right: {rule}"
    );
    // Open rotates to point down, exactly like the proof caret.
    assert!(
        BASE_CSS.contains(
            ".callout-collapse > details[open] > summary.callout-title::after { transform: rotate(135deg); }"
        ),
        "open state must rotate the caret down"
    );
}

/// A scratch directory for a test that needs a real partial on disk to include.
fn source_map_tmpdir(tag: &str) -> std::path::PathBuf {
    let uniq = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let d = std::env::temp_dir().join(format!("tali-srcmap-{tag}-{uniq}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn a_warning_after_an_include_carries_the_authors_own_line_not_the_buffer_line() {
    // Item A8. The per-block tuple bound the POST-INCLUDE buffer line while binding
    // `source_file` to the MAPPED origin file, and ten warning sites then emitted the pair.
    // Both halves are wrong, and the parent half is the worse one: `source_file` is None
    // there, so the diagnostic carries a real, openable path with a line that is off by
    // however much the include expanded above it, and nothing signals it.

    // Half one: the PARENT's own warning, after an include. `part.tmd` expands 5 lines in
    // place of 1, so the buffer line runs 4 ahead of the line the author is looking at.
    let d = source_map_tmpdir("a8-parent");
    std::fs::write(
        d.join("part.tmd"),
        "Partial one.\n\nPartial two.\n\nPartial three.\n",
    )
    .unwrap();
    //                     1   2         3    4  5                          6   7
    let src =
        "---\ntitle: X\n---\n\n{{< include part.tmd >}}\n\n## Dup {#same}\n\n## Dup {#same}\n";
    //                                                              8   9
    assert_eq!(
        src.lines().nth(8),
        Some("## Dup {#same}"),
        "line 9 is the duplicate"
    );
    let doc = render_document_with_includes(src, &d);
    let dup = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("duplicate heading id"))
        .unwrap_or_else(|| panic!("no duplicate-heading warning: {:?}", doc.warnings));
    assert_eq!(dup.file, None, "the duplicate is in the parent document");
    assert_eq!(
        dup.line,
        Some(9),
        "must be the parent's own line 9, not the buffer's 13: {dup:?}"
    );

    // Half two: a warning INSIDE the partial. `part.tmd` is 5 lines long, so the buffer
    // line (17) does not exist in the file the diagnostic names at all.
    let d2 = source_map_tmpdir("a8-partial");
    let part = "Partial one.\n\n## Dup {#same}\n\n## Dup {#same}\n";
    std::fs::write(d2.join("part.tmd"), part).unwrap();
    let src2 = "---\ntitle: X\n---\n\nlead\n\nmore\n\nmore\n\nmore\n\n{{< include part.tmd >}}\n";
    let doc2 = render_document_with_includes(src2, &d2);
    let dup2 = doc2
        .warnings
        .iter()
        .find(|w| w.message.contains("duplicate heading id"))
        .unwrap_or_else(|| panic!("no duplicate-heading warning: {:?}", doc2.warnings));
    assert_eq!(dup2.file.as_deref(), Some("part.tmd"));
    assert_eq!(
        dup2.line,
        Some(5),
        "must be part.tmd's own line 5, not the buffer's 17: {dup2:?}"
    );
    assert!(
        dup2.line.unwrap() as usize <= part.lines().count(),
        "a diagnostic may never point past the end of the file it names"
    );
}

#[test]
fn a_block_straddling_an_include_boundary_stays_inside_one_files_line_numbering() {
    // Item A9. `map_origin` was applied independently to the start and end lines while only
    // the START's file was kept for `data-source-file`, so a paragraph comrak merged across
    // the boundary (partial's last line non-blank, parent's next line non-blank) emitted a
    // range mixing two files' numbering. With the partial longer than the parent's prefix
    // the range comes out INVERTED, which violates `tests/corpus.rs`'s own `sl <= el` and
    // makes `client.js`'s `highlightAtLine` skip the block outright.
    let d = source_map_tmpdir("a9");
    let mut part = String::new();
    for i in 1..=19 {
        part.push_str(&format!("filler {i}\n\n"));
    }
    part.push_str("tail-a\ntail-b\n"); // partial lines 39 and 40
    let part_lines = part.lines().count();
    assert_eq!(part_lines, 40);
    std::fs::write(d.join("part.tmd"), &part).unwrap();
    // The include sits at parent line 5 and the parent's own tail at line 6, with no blank
    // between: comrak merges partial:39-40 with parent:6 into one paragraph.
    let doc = render_document_with_includes(
        "---\ntitle: X\n---\n\n{{< include part.tmd >}}\nparent tail line\n",
        &d,
    );
    let straddler = doc
        .blocks
        .iter()
        .find(|b| b.html.contains("tail-a"))
        .expect("the merged paragraph");
    assert_eq!(
        straddler.source_file.as_deref(),
        Some("part.tmd"),
        "the block is attributed to the file it starts in"
    );
    assert_eq!(
        straddler.sourcepos, "39:1-40:16",
        "the range must stay inside part.tmd, not end on the parent's line 6"
    );
    let (sl, el) = straddler
        .sourcepos
        .split_once('-')
        .map(|(a, b)| {
            (
                a.split(':').next().unwrap().parse::<usize>().unwrap(),
                b.split(':').next().unwrap().parse::<usize>().unwrap(),
            )
        })
        .unwrap();
    assert!(sl <= el, "inverted sourcepos: {}", straddler.sourcepos);
    assert!(
        el <= part_lines,
        "the range runs past the end of {:?}: {}",
        straddler.source_file,
        straddler.sourcepos
    );

    // The positive control: every block that does NOT straddle keeps the exact range it had
    // before the clamp, so this did not buy `sl <= el` by flattening every block.
    let first = doc
        .blocks
        .iter()
        .find(|b| b.html.contains("filler 1<"))
        .expect("the first filler paragraph");
    assert_eq!(first.sourcepos, "1:1-1:8");
}

#[test]
fn a_repeated_explicit_id_is_deduped_and_reported_wherever_it_is_written() {
    // Item A10. Only headings deduped an explicit `{#id}`; every other construct wrote the
    // author's id straight into the element, so one shared partial included twice emitted
    // `<h2 id="sec-shared">` + `<h2 id="sec-shared-1">` correctly beside two identical
    // `<figure id="fig-shared">`. The ruling is RENAME, not refuse: the first definition
    // keeps the author's spelling so their links still resolve, and the duplicate is an
    // error-severity, located diagnostic rather than silently invalid HTML.
    let doc = render_document_with_includes(
        "---\ntitle: X\n---\n\n\
         ![Cap](a.png){#fig-shared}\n\n\
         ![Cap](a.png){#fig-shared}\n\n\
         ::: {#dup-div}\nbody\n:::\n\n\
         ::: {#dup-div}\nbody two\n:::\n",
        std::path::Path::new("."),
    );
    let ids: Vec<&str> = doc
        .blocks
        .iter()
        .filter_map(|b| {
            let i = b.html.find(" id=\"")? + 5;
            let rest = &b.html[i..];
            Some(&rest[..rest.find('"')?])
        })
        .collect();
    assert_eq!(
        ids,
        vec!["fig-shared", "fig-shared-1", "dup-div", "dup-div-1"],
        "the FIRST of each pair keeps the author's own spelling"
    );

    let dups: Vec<&Warning> = doc
        .warnings
        .iter()
        .filter(|w| w.message.contains("duplicate element id"))
        .collect();
    assert_eq!(dups.len(), 2, "one per duplicate: {:?}", doc.warnings);
    assert!(
        dups.iter().all(|w| w.severity == Severity::Error),
        "a duplicate id is invalid HTML, not advice: {dups:?}"
    );
    // Located at the SECOND definition, like the duplicate-heading warning beside it: that
    // is the one the author has to change.
    assert_eq!(dups[0].line, Some(7), "the second figure: {:?}", dups[0]);
    assert_eq!(dups[1].line, Some(13), "the second div: {:?}", dups[1]);
    // The div pair is the half that had NO diagnostic at all before: a figure at least drew
    // `register_xref`'s duplicate-label error, a plain `{#id}` drew silence.
    assert!(dups[1].message.contains("dup-div-1"), "{:?}", dups[1]);

    // A hand-written `-1` already on the page must not be handed out a second time.
    let clash = render_document_with_includes(
        "---\ntitle: X\n---\n\n\
         ![A](a.png){#fig-plot-1}\n\n![B](a.png){#fig-plot}\n\n![C](a.png){#fig-plot}\n",
        std::path::Path::new("."),
    );
    let clash_ids: Vec<&str> = clash
        .blocks
        .iter()
        .filter_map(|b| {
            let i = b.html.find(" id=\"")? + 5;
            let rest = &b.html[i..];
            Some(&rest[..rest.find('"')?])
        })
        .collect();
    assert_eq!(clash_ids, vec!["fig-plot-1", "fig-plot", "fig-plot-2"]);

    // The positive control: a page with no collision is untouched and silent, so the pass
    // cannot be passing the assertions above by renaming everything it sees.
    let clean = render_document_with_includes(
        "---\ntitle: X\n---\n\n## One {#sec-one}\n\n![Cap](a.png){#fig-one}\n\n::: {#note}\nbody\n:::\n",
        std::path::Path::new("."),
    );
    assert!(
        clean
            .blocks
            .iter()
            .any(|b| b.html.contains("id=\"fig-one\"")),
        "the lone figure keeps its id"
    );
    assert!(
        !clean
            .warnings
            .iter()
            .any(|w| w.message.contains("duplicate element id")),
        "no collision, no warning: {:?}",
        clean.warnings
    );
}

/// Mean advance of English lowercase text in Literata, in `em`.
///
/// MEASURED 2026-08-15 with the chrome-devtools MCP, on `corpus/analyst/index.tmd` built
/// with `--no-exec` and served statically at an 810px viewport (no mobile breakpoint
/// engages below that; `body { max-width: var(--tali-measure) }` renders its full 640px
/// column, confirmed via `getComputedStyle(document.body).maxWidth === "640px"`), 20px
/// body font, `document.fonts.check('20px Literata')` true (the vendored face, not a
/// fallback). A `white-space: pre` span holding four repeats of a 76-character ordinary
/// English sentence (including its spaces — an alphabet run overstates density, because
/// English is rich in narrow letters) was appended to a live paragraph and measured by
/// `getBoundingClientRect().width`: 9.5500 px/char at 20px = 0.4775 em/char. Stable
/// (identical to four decimal places) across five different paragraphs on the page,
/// re-affirming the brief's own pre-subsetting estimate of 0.4776 to within 0.0001.
///
/// This is a measurement, so it carries its date. The font hash below is what makes it
/// re-measurable rather than merely asserted: change the face and this test fails, which
/// is the point.
const LITERATA_MEAN_ADVANCE_EM: f64 = 0.4775;

/// The measure is pinned in CHARACTERS, because that is what a reader experiences and it is
/// what WCAG 1.4.8 bounds. Before this theme the column was 46rem: measured 96 characters of
/// capacity with filled paragraphs at 80-92, past the 80-character AAA ceiling and far past
/// the comprehension-optimal band.
#[test]
fn the_measure_is_sixty_to_seventy_characters() {
    let em: f64 = {
        let d = TOKENS_CSS
            .split("--tali-measure:")
            .nth(1)
            .expect("--tali-measure is defined in tokens.css");
        let v = d.split(';').next().unwrap().trim();
        v.trim_end_matches("em")
            .parse()
            .unwrap_or_else(|_| panic!("--tali-measure must be in `em`, got `{v}`"))
    };
    let chars = em / LITERATA_MEAN_ADVANCE_EM;
    assert!(
        (62.0..=72.0).contains(&chars),
        "the measure renders {chars:.1} characters; keep it in 62..=72 \
         (WCAG 1.4.8 caps at 80). Either --tali-measure or the body face moved."
    );
}

/// The advance constant above describes ONE font binary. If the binary changes, the constant
/// is stale and the measure test is measuring nothing. Hash the file so a swap cannot be
/// silent.
#[test]
fn the_body_face_is_the_one_the_measure_was_measured_on() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/fonts/literata-latin-wght-normal.woff2");
    let bytes = std::fs::read(&p).expect("the vendored body face");
    // FNV-1a: no dependency, and collision resistance is irrelevant here — this only has
    // to notice that somebody replaced the file.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in &bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    assert_eq!(
        (bytes.len(), h),
        (48_072, 0x7176_a838_9de9_bbdb), // <- printed by the first run, 2026-08-15
        "the body face changed. Re-measure LITERATA_MEAN_ADVANCE_EM in a browser \
         (render a paragraph, divide column width by realized characters per line, \
         divide by font-size), update it and this hash together, and re-date both."
    );
}
