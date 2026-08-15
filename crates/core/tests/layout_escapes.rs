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
#[test]
fn the_escape_is_a_grid_span_not_an_arithmetic() {
    let base = css("base.css");
    assert!(
        base.contains("> .column-page { grid-column: bleed; }"),
        "`.column-page` must be a named grid span"
    );
    assert!(
        !base.contains("margin-left: calc("),
        "an escape computed from a margin is the formula this grid replaced"
    );
}

/// The band the escape reaches is the cap the retired formula carried: 20rem + the 40rem
/// measure = 60rem. That the number survives is what makes the rewrite lossless rather than a
/// redesign wearing a refactor's clothes.
#[test]
fn the_escape_band_preserves_the_sixty_rem_cap() {
    let bleed: f64 = css("tokens.css")
        .split("--tali-bleed:")
        .nth(1)
        .expect("--tali-bleed is defined")
        .split(';')
        .next()
        .unwrap()
        .trim()
        .trim_end_matches("rem")
        .parse()
        .expect("--tali-bleed is in rem");
    // 32em of a 1.25rem body = 40rem.
    assert_eq!(bleed + 40.0, 60.0, "the escape cap moved off 60rem");
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
