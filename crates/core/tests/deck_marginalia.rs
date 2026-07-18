//! B4: the deck "Marginalia" visual identity (Direction A, serif titles).
//!
//! A visual pass verified in the browser (the project's UI loop), but pinned here against
//! silent regression on two coupled facts: (1) the shipped deck CSS carries the identity —
//! the Newsreader serif head font, the section-divider treatment, and the title accent
//! rule; and (2) the engine still emits the structural hook that CSS targets — an h1-led
//! slide is a `data-level="1"` section. If either half rots, the identity silently
//! disappears while every other test stays green.

use taliesin_core::render_html_page_with_includes;

mod common;
use common::corpus_dir;

#[test]
fn the_marginalia_deck_ships_its_serif_identity_and_section_divider_hook() {
    let dir = corpus_dir();
    let src = std::fs::read_to_string(dir.join("deck-marginalia.tmd")).expect("exemplar deck");
    let page = render_html_page_with_includes(&src, &dir, "The Marginalia Deck");

    // It really is a deck page (routes through the deck engine, inlines deck.css).
    assert!(page.contains("tali-deck"), "not rendered as a deck");

    // (1) The identity is in the shipped CSS: the serif head font is defined AND applied,
    // the section-divider slide is styled, and the title carries the accent rule.
    assert!(
        page.contains("--deck-font-head") && page.contains("Newsreader"),
        "the serif head-font token/family must ship"
    );
    assert!(
        page.contains("font-family: var(--deck-font-head)"),
        "headings must render in the serif head font"
    );
    assert!(
        page.contains("section.tali-slide[data-level=\"1\"]"),
        "the section-divider treatment must ship"
    );
    assert!(
        page.contains("section.tali-title-slide h1.title::after"),
        "the title-slide accent rule must ship"
    );

    // (2) The engine still emits the hook the section-divider CSS targets: the two h1s
    // ("First movement", "Second movement") each open a data-level="1" slide.
    assert!(
        page.matches("data-level=\"1\"").count() >= 2,
        "each h1 section must emit a data-level=\"1\" divider slide the CSS can target"
    );
}
