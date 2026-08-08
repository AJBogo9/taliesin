//! AP7-1: the emitted heading outline of a titled page, and the rule that guards it.
//!
//! Two independent defects met here, and both have to be pinned or the pair regresses:
//! heading demotion was an absolute `+1` (right only for a `#`-rooted page, so the
//! `##`-rooted house style landed at `h3` under the title's `h1`), and the a11y heading
//! rule could not see the title block's `<h1>` at all, so the largest jump on every page
//! was never compared to anything. 37 of 51 book pages skipped a level while
//! `taliesin check` printed "no problems found".

use std::path::Path;
use taliesin_core::diagnostics;

/// The emitted heading levels of a rendered document, in document order. The title block
/// counts as level 1: it is `blocks[0]` and its `<h1 class="title">` is the page's only
/// `<h1>`, so an outline that ignores it is not the outline a reader navigates.
fn outline(src: &str) -> Vec<u8> {
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    doc.blocks
        .iter()
        .filter_map(|b| {
            if b.html.starts_with("<header class=\"tali-title-block\"") {
                return Some(1);
            }
            let d = *b.html.strip_prefix("<h")?.as_bytes().first()?;
            d.is_ascii_digit().then_some(d - b'0').filter(|l| *l <= 6)
        })
        .collect()
}

fn skips(src: &str) -> Vec<String> {
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    diagnostics::validate_a11y(&doc.blocks)
        .into_iter()
        .filter(|w| w.message.contains("heading level skips"))
        .map(|w| w.message)
        .collect()
}

const FM: &str = "---\ntitle: A page\n---\n\n";

#[test]
fn a_titled_page_emits_a_contiguous_outline_whatever_level_it_is_rooted_at() {
    // `#`-rooted: the one shape absolute `+1` was right for. Unchanged.
    assert_eq!(
        outline(&format!("{FM}# One\n\nText.\n\n## Under\n\nText.\n")),
        vec![1, 2, 3]
    );
    // `##`-rooted — the house style of both dogfood books, since a `#` would restate the
    // front-matter `title:`. This is the 35-page case: it used to emit 1, 3, 4.
    assert_eq!(
        outline(&format!("{FM}## One\n\nText.\n\n### Under\n\nText.\n")),
        vec![1, 2, 3]
    );
    // `###`-rooted (the 2-page case, `h1 -> h4`): the shift is negative here, so a fix
    // that only ever demotes leaves this one skipping.
    assert_eq!(
        outline(&format!("{FM}### One\n\nText.\n\n#### Under\n\nText.\n")),
        vec![1, 2, 3]
    );
    // A page whose shallowest heading is a body `# H1` (corpus/tarn carries one on
    // purpose) still demotes by one, so the title keeps its sole `<h1>`.
    assert_eq!(
        outline(&format!("{FM}## Early\n\nText.\n\n# Body H1\n\nText.\n")),
        vec![1, 3, 2]
    );
    // Every one of the above is clean by the project's own rule.
    for src in [
        format!("{FM}# One\n\nT.\n\n## Under\n\nT.\n"),
        format!("{FM}## One\n\nT.\n\n### Under\n\nT.\n"),
        format!("{FM}### One\n\nT.\n\n#### Under\n\nT.\n"),
    ] {
        assert!(skips(&src).is_empty(), "{src:?} -> {:?}", skips(&src));
    }
}

#[test]
fn demotion_is_relative_to_the_page_not_absolute() {
    // The invariant behind all of the above, stated once: a titled page's shallowest body
    // heading always emits as `h2` — directly under the title block's `h1` and never two
    // levels below it — and the *relative* depth of everything else is preserved. An
    // absolute `+1` satisfies this only when the page is `#`-rooted.
    for root in 1..=4u8 {
        let src = format!(
            "{FM}{} One\n\nT.\n\n{} Under\n\nT.\n\n{} Two\n\nT.\n",
            "#".repeat(root as usize),
            "#".repeat(root as usize + 1),
            "#".repeat(root as usize),
        );
        assert_eq!(
            outline(&src),
            vec![1, 2, 3, 2],
            "rooted at h{root}: shallowest body heading must emit as h2"
        );
    }
}

#[test]
fn an_untitled_page_is_not_demoted() {
    // No title block, so the page's own `#` is its `<h1>` and nothing shifts.
    assert_eq!(outline("# One\n\nText.\n\n## Under\n\nText.\n"), vec![1, 2]);
    // `title-block-style: none` suppresses the VISIBLE title, so `emits_title_block` is
    // false and nothing shifts — but a `tali-sr-only` `<h1>` is still injected (PA-H2), so
    // the outline comes out contiguous without a shift.
    let hidden = "---\ntitle: T\ntitle-block-style: none\n---\n\n## One\n\nT.\n\n### Under\n\nT.\n";
    assert_eq!(outline(hidden), vec![1, 2, 3]);
    assert!(skips(hidden).is_empty(), "{:?}", skips(hidden));
}

#[test]
fn the_heading_rule_counts_the_title_block_as_the_pages_h1() {
    // Cause (2), and the reason this is worth fixing even though cause (1) now prevents the
    // shape from being emitted: `helpers::heading_level` needs a block's html to START with
    // `<hN`, and the title block is `<header class="tali-title-block">…<h1>`. So `prev`
    // stayed 0 through the one comparison that mattered, and the rule reported nothing on
    // 37 pages that skipped. With the title block counted, a regression in demotion is a
    // `check` failure instead of a silent one.
    let doc = taliesin_core::render_document_with_includes(
        &format!("{FM}## One\n\nText.\n"),
        Path::new("."),
    );
    let title = doc.blocks[0].clone();
    assert!(
        title.html.starts_with("<header class=\"tali-title-block\""),
        "the title block is blocks[0]: {}",
        title.html
    );
    // Hand-build the outline demotion used to produce (title h1, then h3) and assert the
    // rule now reports it. Restoring the absolute `+1` reproduces exactly these blocks.
    let mut blocks = vec![title];
    blocks.push(taliesin_core::Block {
        id: "b1".into(),
        sourcepos: "5:1-5:7".into(),
        source_file: None,
        html: "<h3 id=\"one\">One</h3>".into(),
        cell: None,
        nested: Vec::new(),
    });
    let ws = diagnostics::validate_a11y(&blocks);
    assert!(
        ws.iter().any(|w| w.message.contains("from h1 to h3")),
        "a body heading two levels under the title must be reported: {:?}",
        ws.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}

#[test]
fn a_genuine_mid_document_skip_is_still_reported() {
    // The rule must not go quiet: relative demotion shifts the whole outline, it does not
    // flatten it, so an author who jumps h2 -> h4 still hears about it.
    let ws = skips(&format!("{FM}## One\n\nT.\n\n#### Deep\n\nT.\n"));
    assert_eq!(ws.len(), 1, "{ws:?}");
    assert!(ws[0].contains("from h2 to h4"), "{ws:?}");
}

#[test]
fn every_book_in_the_repo_emits_a_contiguous_outline() {
    // The measurement AP7-1 actually made, turned into a gate. The dogfood books are NOT in
    // the regression net (the standing "what the test net structurally cannot see" note), so
    // this walks them explicitly alongside the corpus book that carries the awkward shapes.
    //
    // The rule used to exempt decks wholesale (item 111), which meant two of these files
    // could not fail the walk however they were written. The slide-deck engine was cut on
    // 2026-08-08, so the exemption and its two files are both gone and every page walked
    // here is a page the rule can fire on.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut pages = 0usize;
    let mut bad: Vec<String> = Vec::new();
    for book in ["docs/guide", "docs/internals", "corpus/tarn"] {
        let dir = root.join(book);
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    if !p.ends_with("_freeze") && !p.ends_with("_site") {
                        stack.push(p);
                    }
                } else if p.extension().is_some_and(|x| x == "tmd") {
                    let src = std::fs::read_to_string(&p).unwrap();
                    let doc = taliesin_core::render_document_with_includes(
                        &src,
                        p.parent().unwrap_or(&dir),
                    );
                    pages += 1;
                    let ws: Vec<String> = diagnostics::validate_a11y(&doc.blocks)
                        .into_iter()
                        .filter(|w| w.message.contains("heading level skips"))
                        .map(|w| w.message)
                        .collect();
                    if !ws.is_empty() {
                        bad.push(format!("{}: {}", p.display(), ws.join("; ")));
                    }
                }
            }
        }
    }
    assert!(
        pages >= 40,
        "the book walk found only {pages} pages the heading rule applies to"
    );
    assert!(
        bad.is_empty(),
        "{} of {pages} book pages skip a heading level:\n  {}",
        bad.len(),
        bad.join("\n  ")
    );
}

/// AP7-3: `.scrolly` and `.code-walkthrough` carried NO accessibility semantics — measured
/// at 0 steps with `aria`/`role`, `null` root role, and nothing associating a step with the
/// thing it drives. The prose reads fine linearly, so a screen-reader user gets the words;
/// what they never got is the **stage** whose state moves as a consequence of *visual
/// scrolling*.
#[test]
fn a_scroll_driven_step_names_itself_and_the_stage_it_drives() {
    let render = |src: &str| {
        taliesin_core::render_document_with_includes(src, Path::new("."))
            .blocks
            .iter()
            .map(|b| b.html.clone())
            .collect::<Vec<_>>()
            .join("")
    };

    // A walkthrough knows something extra the reader can use: WHICH lines each step
    // highlights. That is a fact the renderer already had and threw away.
    let cw = render(
        "---\ntitle: T\n---\n\n::: {.code-walkthrough}\n```python\na = 1\nb = 2\nc = 3\n```\n\n\
         ::: {.step lines=\"1\"}\nFirst.\n:::\n\n::: {.step lines=\"2-3\"}\nThen these.\n:::\n:::\n",
    );
    assert!(
        cw.contains(r#"<div class="code-walkthrough" role="group" aria-label="Code walkthrough""#),
        "the container must name itself: {cw}"
    );
    // Singular vs plural, and a range spelled for speech: a screen reader reads the raw
    // `2-3` spec as "two dash three".
    assert!(
        cw.contains(r#"aria-label="Step 1 of 2, highlighting line 1""#),
        "a one-line step is singular: {cw}"
    );
    assert!(
        cw.contains(r#"aria-label="Step 2 of 2, highlighting lines 2 to 3""#),
        "a range is spoken, not spelled with a hyphen: {cw}"
    );
    // Every step points at the stage, and that stage exists with exactly that id.
    let stage_id = {
        let i = cw
            .find(r#"aria-controls=""#)
            .expect("steps carry aria-controls")
            + 15;
        let rest = &cw[i..];
        rest[..rest.find('"').unwrap()].to_string()
    };
    assert!(stage_id.ends_with("-stage"), "{stage_id}");
    assert_eq!(
        cw.matches(&format!(r#"aria-controls="{stage_id}""#))
            .count(),
        2,
        "both steps drive the same stage: {cw}"
    );
    assert!(
        cw.contains(&format!(
            r#"<div class="cw-stage" id="{stage_id}" role="group""#
        )),
        "the aria-controls target must be the sticky panel itself: {cw}"
    );

    // A scrolly gets the same treatment; its `state=` is an author token for scrolly.js,
    // not reader prose, so the label is the ordinal alone.
    let sc = render(
        "---\ntitle: T\n---\n\n::: {.scrolly}\n![A chart](c.png)\n\n\
         ::: {.step state=\"a\"}\nFirst.\n:::\n\n::: {.step state=\"b\"}\nSecond.\n:::\n:::\n",
    );
    assert!(
        sc.contains(
            r#"<div class="tali-scrolly" role="group" aria-label="Scroll-driven walkthrough""#
        ),
        "{sc}"
    );
    assert!(sc.contains(r#"aria-label="Step 2 of 2""#), "{sc}");
    assert!(
        !sc.contains("highlighting"),
        "a scrolly step has no line range to name: {sc}"
    );
    assert!(sc.contains(r#"<div class="scrolly-stage" id="#), "{sc}");

    // Steps stay OUT of the tab order: they are prose, and a keyboard user reads them by
    // scrolling like everyone else. `tabindex` on paragraphs adds stops, not capability.
    for html in [&cw, &sc] {
        assert!(
            !html.contains("tabindex"),
            "steps must not be tab stops: {html}"
        );
    }

    // The labels are attributes, not injected text, so the search index and `llms.txt` are
    // untouched — an `aria-label` on a bare `<div>` would be ignored by AT, which is why
    // the `role="group"` is load-bearing rather than decorative.
    let doc = taliesin_core::render_document_with_includes(
        "---\ntitle: T\n---\n\n::: {.scrolly}\n![A chart](c.png)\n\n::: {.step}\nFirst.\n:::\n:::\n",
        Path::new("."),
    );
    let text = doc.body_text();
    assert!(
        !text.contains("Step 1 of 1"),
        "the label must not leak into the text projection: {text}"
    );
}
