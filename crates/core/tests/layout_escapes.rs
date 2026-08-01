//! Layout escapes: `::: {.column-page}` / `::: {.column-screen}` (backlog item 181).
//!
//! **Why these pins are on the stylesheet source rather than a rendered page.** The escape
//! has no render path at all — it is a plain class on a plain block, which is the whole point
//! of the design — so every load-bearing decision lives in CSS. Asserting on a rendered page
//! would also walk straight into the inlined-asset trap: every Taliesin page inlines the whole
//! stylesheet, so `page.contains(".column-page")` is true on a page that renders no escape.
//!
//! The *geometry* these rules produce was verified in a browser across all five container
//! modes (single-document `body`, `body.has-toc`, `.tali-site-main`,
//! `.tali-site-main.has-toc`, `.tali-book-main`) at a wide and a narrow viewport. What is
//! pinned here is each decision that measurement forced, so a later edit that looks harmless
//! cannot silently undo one.

use std::path::Path;

fn css(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/css")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// The text between `selector {` and the next `}`.
///
/// Anchored on the newline before the selector, so a lookup for `.column-screen` cannot
/// silently return the body of the shared `.column-page, .column-screen` rule — which is
/// exactly what an unanchored `split` did here, and it made the gutter assertion below fail
/// against CSS that was correct.
fn rule(sheet: &str, selector: &str) -> String {
    sheet
        .split(&format!("\n  {selector} {{"))
        .nth(1)
        .and_then(|r| r.split('}').next())
        .unwrap_or_else(|| panic!("no `{selector}` rule in the stylesheet"))
        .to_string()
}

fn render(src: &str) -> taliesin_core::RenderedDoc {
    taliesin_core::render_document_with_includes(src, Path::new("."))
}

/// Both classes must reach the page as ordinary blocks, carrying the block attributes every
/// emitted block owes (`corpus.rs` enforces the general rule; this is the specific one), and
/// neither may trip the closed-vocabulary did-you-mean that fires on a misspelled `:::` class.
#[test]
fn the_escape_classes_render_as_plain_blocks_and_are_known_vocabulary() {
    let doc = render(
        "---\ntitle: T\n---\n\n::: {.column-page}\nWide.\n:::\n\n::: {.column-screen}\nWider.\n:::\n",
    );
    let body = doc.body_html();

    for class in ["column-page", "column-screen"] {
        assert!(
            body.contains(&format!("class=\"{class}\"")),
            "`::: {{.{class}}}` must emit a block carrying that class: {body}"
        );
    }
    assert!(
        body.matches("data-block-id").count() >= 2 && body.contains("data-sourcepos"),
        "an escape is a normal block, so it keeps click-to-source and the incremental \
         swap: {body}"
    );
    assert!(
        doc.warnings.is_empty(),
        "both classes are in the closed vocabulary, so neither may warn: {:?}",
        doc.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );

    // The control: the gate is only a gate if a near-miss still warns.
    let typo = render("---\ntitle: T\n---\n\n::: {.column-pag}\nOops.\n:::\n");
    assert!(
        typo.warnings
            .iter()
            .any(|w| w.message.contains("column-pag")),
        "a misspelled escape class must still get a did-you-mean: {:?}",
        typo.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}

/// The shared formula. `margin: 50% - w/2` centres the box on its CONTAINER whatever that
/// container's width is, which is what lets one rule serve five container modes without any
/// of them naming the others. Replacing it with a viewport-relative centring (`50vw`) would
/// look identical in the three plain modes and be wrong in both grid modes.
#[test]
fn an_escape_is_centred_on_its_container_not_on_the_viewport() {
    let base = css("base.css");
    let r = rule(&base, ".column-page, .column-screen");

    assert!(
        r.contains("margin-left: calc(50% - var(--tali-escape-w) / 2)")
            && r.contains("margin-right: calc(50% - var(--tali-escape-w) / 2)"),
        "the container-centred formula is the whole design; found:\n{r}"
    );
    assert!(
        r.contains("box-sizing: border-box"),
        "the global reset is content-box, so without this the padding on `.column-screen` \
         would add to the width and overflow the viewport:\n{r}"
    );
    // The floor. Below ~26rem `100vw - 2rem` is NARROWER than the reading column, so without
    // `max(100%, …)` a `.column-page` renders indented on a phone — an escape that shrinks.
    assert!(
        rule(&base, ".column-page").contains("max(100%,"),
        "`.column-page` needs the `max(100%, …)` floor or it inverts on a narrow screen"
    );
}

/// The two table-of-contents grids are the modes that broke the shared assumption: their
/// container is the prose track, and a sticky rail of ordinary text sits to its right.
/// Measured at 1702px before this override: a `.column-page` reached x=1331 while the rail
/// began at x=1111, i.e. 220px of text over text.
#[test]
fn a_toc_grid_grows_an_escape_leftward_so_it_never_runs_under_the_rail() {
    for (sheet, selector) in [
        (
            "base.css",
            "body.has-toc > main :is(.column-page, .column-screen)",
        ),
        (
            "site.css",
            ".tali-site-main.has-toc > main :is(.column-page, .column-screen)",
        ),
    ] {
        let r = rule(&css(sheet), selector);
        assert!(
            r.contains("margin-right: 0"),
            "{sheet}: the right edge must stay flush with the prose column, or the escape \
             crosses the rail:\n{r}"
        );
        assert!(
            r.contains("margin-left: calc(100% - var(--tali-escape-w))"),
            "{sheet}: the escape grows leftward into the unused page margin:\n{r}"
        );
        // 8.25rem is half of (2.5rem gap + 14rem rail) — the distance from the prose track's
        // centre to the page's. It is derivable, not tuned, which is why it may be written
        // once: only the prose track flexes, so it holds at every width above the collapse.
        assert!(
            r.contains("calc(50vw + 50% - 8.25rem)"),
            "{sheet}: the room an escape has is the span from the viewport's left edge to \
             the prose column's right edge:\n{r}"
        );
    }
}

/// Where each grid collapses to a single column there is no rail left to avoid, so the
/// override above must hand the escapes back to the shared page-centred formula. Without
/// this a phone would render every escape right-aligned against a rail that is not there.
#[test]
fn the_leftward_override_is_released_where_the_grid_collapses() {
    for (sheet, selector) in [
        (
            "base.css",
            "body.has-toc > main :is(.column-page, .column-screen)",
        ),
        (
            "site.css",
            ".tali-site-main.has-toc > main :is(.column-page, .column-screen)",
        ),
    ] {
        let sheet_src = css(sheet);
        // The second occurrence is the one inside the collapse media query.
        let reset = sheet_src
            .split(&format!("{selector} {{"))
            .nth(2)
            .and_then(|r| r.split('}').next())
            .unwrap_or_else(|| panic!("{sheet}: no collapse-breakpoint reset for `{selector}`"));
        assert!(
            reset.contains("--tali-escape-room: 100vw")
                && reset.contains("margin-right: calc(50% - var(--tali-escape-w) / 2)"),
            "{sheet}: the collapsed grid must restore the page-centred formula:\n{reset}"
        );
    }
}

/// `100vw` includes the classic scrollbar, so an edge-to-edge box is ~15px wider than the
/// space it has. Measured before the guard: scrollWidth 1710 against clientWidth 1702, i.e. a
/// horizontal scrollbar on a page that never had one.
///
/// **`clip`, never `hidden`.** `hidden` would make `<html>` a scroll container and break
/// every `position: sticky` element on the page — the TOC rail and the book topbar. Verified
/// in a browser after this rule: vertical scrolling still works, the rail still sticks at its
/// 2rem offset, and the page cannot be scrolled horizontally.
///
/// **`:has()`, never a blanket rule.** A page with no full-bleed block may have some other
/// reason to overflow horizontally, and clipping that would hide content with no way to reach
/// it.
#[test]
fn a_full_bleed_escape_cannot_give_the_page_a_horizontal_scrollbar() {
    let base = css("base.css");
    let guard = rule(&base, "html:has(.column-screen)");
    assert!(
        guard.contains("overflow-x: clip"),
        "the full-bleed overshoot must be clipped:\n{guard}"
    );
    assert!(
        !guard.contains("hidden"),
        "`overflow-x: hidden` here would make <html> a scroll container and kill every \
         sticky element on the page; `clip` does not:\n{guard}"
    );
    // The other half: the overshoot is ~7px per side, and with no padding what gets clipped
    // is the first 7px of the author's own text.
    // Asserted as a NON-ZERO length, not merely as a declaration that exists: a
    // `padding-inline: 0` satisfies "the property is there" while reintroducing exactly the
    // clipping it was added to prevent. (Measured: that mutant survived the weaker pin.)
    let screen = rule(&base, ".column-screen");
    let gutter = screen
        .split("padding-inline:")
        .nth(1)
        .and_then(|v| v.split(';').next())
        .map(str::trim)
        .unwrap_or_else(|| {
            panic!(
                "`.column-screen` needs an inline gutter, or edge-to-edge text is clipped \
                    by the very rule that stops the scrollbar:\n{screen}"
            )
        });
    assert!(
        gutter
            .trim_end_matches(|c: char| c.is_alphabetic())
            .parse::<f32>()
            .unwrap_or(0.0)
            > 0.0,
        "the `.column-screen` gutter must be a non-zero length — it is what absorbs the ~7px \
         full-bleed overshoot before the clip reaches the author's text; found `{gutter}`"
    );
}

/// Paper has no viewport to bleed to, and `taliesin pdf` lays out to a fixed page box.
#[test]
fn print_collapses_both_escapes_back_to_the_text_column() {
    let base = css("base.css");
    let printed = base
        .match_indices("@media print {")
        .map(|(i, _)| &base[i..(i + 300).min(base.len())])
        .find(|blk| blk.contains(".column-page"))
        .expect("no `@media print` block resets the escapes");
    // `100%` makes each margin `50% - 50%` = 0, so this is a true reset rather than an
    // override fight, and it neutralises the leftward TOC-grid override too
    // (`100% - 100%` = 0). The gutter goes with it: paper has no scrollbar to dodge.
    assert!(
        printed.contains("--tali-escape-w: 100%")
            && printed.contains("--tali-escape-room: 100%")
            && printed.contains("padding-inline: 0"),
        "print must reset the escape to the text column:\n{printed}"
    );
}
