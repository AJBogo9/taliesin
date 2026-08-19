//! Layout escapes after the reading grid (2026-08-15): `::: {.column-page}` is a NAMED GRID
//! COLUMN, not a centring formula, so what this file pins is that there is one grid, that the
//! escape spans the wider band, and that the band preserves the cap the formula carried. The
//! three near-identical copies this file used to compare against each other are gone, and
//! comparing them was the whole reason it was long.
//!
//! **Why these pins are on the stylesheet source rather than a rendered page.** The escape
//! has no render path at all — it is a plain class on a plain block, which is the whole point
//! of the design — so every load-bearing decision lives in CSS. Asserting on a rendered page
//! would also walk straight into the inlined-asset trap: every Taliesin page inlines the whole
//! stylesheet, so `page.contains(".column-page")` is true on a page that renders no escape.

use std::path::Path;

fn css(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/css")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// The sheet with every `/* … */` stripped, for the checks that assert an ABSENCE. A
/// stylesheet that explains why a spelling was abandoned necessarily contains that spelling,
/// so a bare `!contains` fires on the explanation and the gate can only be satisfied by
/// deleting the reason. Same failure as reading the occupant list out of the comment above
/// it, in the other direction.
fn decls(name: &str) -> String {
    let src = css(name);
    let mut out = String::with_capacity(src.len());
    let mut rest = src.as_str();
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        rest = match rest[open..].find("*/") {
            Some(close) => &rest[open + close + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

fn render(src: &str) -> taliesin_core::RenderedDoc {
    taliesin_core::render_document_with_includes(src, Path::new("."))
}

/// The escape must reach the page as an ordinary block, carrying the block attributes every
/// emitted block owes (`corpus.rs` enforces the general rule; this is the specific one), and
/// it may not trip the closed-vocabulary did-you-mean that fires on a misspelled `:::` class.
#[test]
fn the_escape_class_renders_as_a_plain_block_and_is_known_vocabulary() {
    let doc = render("---\ntitle: T\n---\n\n::: {.column-page}\nWide.\n:::\n");
    let body = doc.body_html();

    assert!(
        body.contains("class=\"column-page\""),
        "`::: {{.column-page}}` must emit a block carrying that class: {body}"
    );
    assert!(
        body.contains("data-block-id") && body.contains("data-sourcepos"),
        "an escape is a normal block, so it keeps click-to-source and the incremental \
         swap: {body}"
    );
    assert!(
        doc.warnings.is_empty(),
        "the class is in the live vocabulary, so it may not warn: {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );

    // The control: the gate is only a gate if a near-miss still warns.
    let typo = render("---\ntitle: T\n---\n\n::: {.column-pag}\nOops.\n:::\n");
    assert!(
        typo.warnings
            .iter()
            .any(|w| w.message.contains("column-pag")),
        "a near-miss must still be caught: {:?}",
        typo.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}

/// The escape is a grid span. If it ever goes back to computing a margin, this fails.
///
/// `pre` shares the span with the author's opt-in class, and that is the point rather than a
/// convenience: at the measure a code block fits 55 columns inside its own padding against
/// PEP 8's 79, so code that did not opt into anything still has to leave the prose column.
#[test]
fn the_escape_is_a_grid_span_not_an_arithmetic() {
    let base = css("base.css");
    assert!(
        base.contains("grid-column: var(--tali-band-span); }"),
        "the band\'s occupants must take one named span through the one token"
    );
    assert!(
        !decls("base.css").contains("margin-left: calc("),
        "an escape computed from a margin is the formula this grid replaced"
    );
}

/// EVERY WRAPPING OF A CODE BLOCK REACHES THE BAND, not just a top-level fence. This is the
/// regression that motivated the 2026-08-19 pass: the occupant list named `pre` alone, and
/// `> :is(…)` is a CHILD combinator, so a `#| code-fold: true` cell (inside `<details>`), a
/// `#| lst-label:` listing (inside `<figure>`) and every cell output (inside `.tali-output`)
/// silently rendered at the bare measure — 55 columns. Measured at 1600px before the fix:
/// `corpus/tech-blog/posts/em-algorithm` rendered its folded cell 640px wide with 546px of
/// content behind a horizontal scrollbar, on a page whose other code blocks were 960px.
///
/// A unit test cannot see a scrollbar, so what is pinned is the list. Each name here is a
/// wrapper the renderer really emits (`render::emit` for the fold, `render::figure` for the
/// listing, `render::divs::output_slot` for the output slot).
#[test]
fn every_wrapping_of_a_code_block_reaches_the_band() {
    let base = css("base.css");
    // Read the SELECTOR only, never the comment above it: `pre` is a substring of "prevent"
    // and a prose gate that its own explanation satisfies is not a gate.
    let at = base
        .find("grid-column: var(--tali-band-span); }")
        .expect("the occupant rule exists");
    let head = &base[..at];
    let start = head
        .rfind("> :is(")
        .expect("the occupant list is an `:is()` argument list");
    let rule = &head[start..];
    for (name, why) in [
        ("pre", "a top-level fence"),
        (".column-page", "the author\'s opt-in escape"),
        ("details.tali-code-fold", "a `#| code-fold: true` cell"),
        ("figure.tali-listing", "a `#| lst-label:` listing"),
        (".tali-output", "a cell\'s output"),
        ("table", "a data table"),
        ("pre.mermaid", "a generated diagram"),
    ] {
        assert!(
            rule.contains(name),
            "`{name}` ({why}) must be in the band\'s occupant list, or it renders at the \
             55-column measure:\n{rule}"
        );
    }
}

/// A CODE BOX IS AS WIDE AS ITS CODE AND NO WIDER. The band is a CEILING, not a size: a box
/// takes its longest line, floored at the measure so a two-line snippet still lines up with
/// the prose, and capped at the band. Filling was the rule until 2026-08-19 and made a
/// `author: "Ada Lovelace"` snippet a 1136px slab.
///
/// Two things are pinned because both were bugs in the fitting attempt this reinstates.
/// The floor is `40rem` and NOT `var(--tali-measure)`: the token is `32em` OF THE BODY FACE
/// and an `em` inside a `pre` is the MONO size (18.4px), so the token resolved to 589px
/// against the 640px it names. And it is `min()`ed against `100%`, because a bare 40rem
/// min-width outranks `max-width` (CSS 2.2 section 10.4) and would overhang every viewport
/// narrower than the measure.
#[test]
fn a_code_box_is_as_wide_as_its_code_and_no_wider() {
    let base = css("base.css");
    assert!(
        base.contains("width: max-content; min-width: min(40rem, 100%); max-width: 100%; }"),
        "a code box must be fitted to its content, floored at the converted measure and \
         capped at the band"
    );
    assert!(
        !decls("base.css").contains("min-width: var(--tali-measure)"),
        "the floor must be the CONVERTED measure: `em` inside a `pre` is the mono size, so \
         the token resolves to 589px and silently means something else here"
    );
}

/// ...but the author's own escape and a diagram's frame stay FILLED, and the two reasons are
/// different. `.column-page` is the author writing "make this wide", which fitting would
/// refuse. A diagram figure is the box that BOUNDS an oversized diagram, so fitting it to its
/// contents would be circular and would restore the clipping the band was widened to fix.
///
/// Both rely on `justify-self`'s own default rather than a declaration, so what this checks
/// is an ABSENCE: neither may appear in the fitted rule's selector.
#[test]
fn the_author_escape_and_a_diagram_frame_still_fill_the_band() {
    let base = css("base.css");
    let at = base
        .find("width: max-content; min-width: min(40rem, 100%); max-width: 100%; }")
        .expect("the fitted rule exists");
    let head = &base[..at];
    let start = head
        .rfind("> :is(")
        .expect("the fitted rule selects an `:is()` list");
    let selector = &head[start..];
    for name in [".column-page", "figure.tali-figure"] {
        assert!(
            !selector.contains(name),
            "`{name}` must NOT be fitted; it fills the band:\n{selector}"
        );
    }
    for name in [
        "pre",
        "details.tali-code-fold",
        "figure.tali-listing",
        ".tali-output",
    ] {
        assert!(
            selector.contains(name),
            "`{name}` must be fitted to its content:\n{selector}"
        );
    }
}

/// THE BAND IS ONE WIDTH IN TWO POSITIONS. The side tracks are re-split rather than resized,
/// so a code block is the same size on a page with a margin note as on one without and only
/// its anchor moves. Pinned on the spellings because both are `calc()` over the same token:
/// a number parsed out of either one could agree with this test while disagreeing with the
/// band it is supposed to be half of.
#[test]
fn both_positions_of_the_band_are_the_same_width() {
    let tokens = css("tokens.css");
    let base = css("base.css");
    // Free page: the band straddles the measure, so each side is half of what is left over.
    // 40rem is `--tali-measure` (32em of the 1.25rem body) converted once, as tokens.css
    // already does for the note tracks — an `em` here would resolve against the root 16px.
    for side in ["--tali-side-l", "--tali-side-r"] {
        assert!(
            tokens.contains(&format!("{side}: calc((var(--tali-band) - 40rem) / 2);")),
            "{side} must be derived from --tali-band, not stated"
        );
    }
    // Beside a margin note the right side is the note's, so the whole remainder goes left.
    assert!(
        base.contains("--tali-side-l: calc(var(--tali-band) - 40rem); --tali-side-r: 0;"),
        "the note branch must move the SAME remainder to one side, not shrink the band"
    );
    assert!(
        base.contains("--tali-band-span: bleed-start / text-end;")
            && tokens.contains("--tali-band-span: bleed-start / bleed-end;"),
        "the two spans are the two positions; one of them is missing"
    );
    assert!(
        base.contains("--tali-band-anchor: end;") && tokens.contains("--tali-band-anchor: center;"),
        "a fitted box (a `table`) anchors to the prose, and which edge depends on the branch"
    );
}

/// The margin column is reserved ON DEMAND, and that is what frees the right side band.
/// It used to be reserved on every non-rail page whether or not the page had a note: 2 of
/// the 46 pages the four sites publish carry one (measured 2026-08-19), so 44 pages held
/// 20rem + 3.75rem for nobody and no code block could have it.
///
/// The polarity is load-bearing and is why this asserts `:has()` rather than `:not(:has())`.
/// The DEFAULT must be the free band and the reservation the addition, so that an engine
/// without `:has()` drops this rule and renders a note over a wide block — visible — rather
/// than dropping the opposite rule and silently collapsing the band on every page.
#[test]
fn the_margin_column_is_reserved_only_where_a_note_exists() {
    let base = css("base.css");
    assert!(
        base.contains(":has(.column-margin, .tali-sidenote) {\n      --tali-note-w: 20rem;"),
        "the margin column\'s tracks must engage behind `:has()`, on the page\'s own content"
    );
    assert!(
        !decls("base.css").contains(":not(:has(.column-margin"),
        "the inverted spelling makes a missing `:has()` collapse the band instead of the \
         margin column; the default must be the free band"
    );
}

/// One grid, every container mode. The five modes this file used to enumerate — single
/// document, `body.has-toc`, `.tali-site-main`, `.tali-site-main.has-toc`, `.tali-book-main` —
/// no longer each need an answer: the grid lives inside `<main>`, so the container above it
/// only decides how much room it gets, never where a block sits inside it. What is left to
/// pin is that no container reintroduces a width of its own.
#[test]
fn no_container_owns_the_measure_any_more() {
    let base = css("base.css");
    let site = css("site.css");
    assert!(
        base.contains("grid-template-columns: var(--tali-prose-cols)"),
        "the reading grid must be declared in base.css"
    );
    for (name, sheet, selector) in [
        ("site.css", &site, ".tali-site-main {"),
        ("site.css", &site, ".tali-book-main {"),
    ] {
        let at = sheet
            .find(selector)
            .unwrap_or_else(|| panic!("no `{selector}` rule in {name}"));
        let block = &sheet[at..at + 220];
        assert!(
            block.contains("max-width: none"),
            "{name}'s `{selector}` must not own a width: the grid does. Owning one is what \
             made a site page render 63.7 characters against a single document's 67.0, \
             because the container's border-box padding was drawn inside the measure.\n{block}"
        );
    }
}
