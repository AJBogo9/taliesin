//! The deck's in-product key sheet must not drift from what the keys actually do.
//!
//! It has drifted twice: first the sheet said "Vertical slides" for ↑↓ while `up()`/
//! `down()` called `moveTopic`; then the topic-jump itself was removed (2026-07-24) for
//! being confusing (from a slide in the main run, ↓ "kept the column" and could skip the
//! first sub-slide of a section). ↑↓ now navigate linearly, like every mainstream deck
//! tool: ↓/→ = next, ↑/← = previous. These pins read the *binding* and the *sheet*
//! together, so neither can quietly say something the other doesn't.

use std::path::Path;

fn deck_js() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rel = "crates/core/assets/js/deck.js";
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// The description text of a `key('<k>', '<desc>')` row in `KEYS_HTML`.
fn key_sheet_label(js: &str, k: &str) -> String {
    let marker = format!("key('{k}', '");
    let start = js
        .find(&marker)
        .unwrap_or_else(|| panic!("no `{k}` row in the deck key sheet"))
        + marker.len();
    let rest = &js[start..];
    let end = rest.find('\'').expect("unterminated key-sheet label");
    rest[..end].to_string()
}

/// ↑↓ navigate linearly (↓ = next, ↑ = previous), the same as →/←. Guard the binding
/// first: if it ever changes, the wording pins below are measuring the wrong thing and
/// must be rechecked rather than silently passing.
#[test]
fn key_sheet_describes_up_down_as_the_linear_nav_it_is() {
    let js = deck_js();
    assert!(
        js.contains("function down() { next(); }") && js.contains("function up() { prev(); }"),
        "↓/↑ no longer map to next()/prev(); recheck the key-sheet wording pins in this file"
    );
    assert!(
        js.contains("case 'ArrowDown': down(); break;")
            && js.contains("case 'ArrowUp': up(); break;"),
        "ArrowUp/ArrowDown no longer route to up()/down(); recheck the key-sheet wording pins"
    );
    // The confusing grid-row jump is gone: `moveTopic` must not come back without a
    // deliberate revisit of this whole decision (and this test).
    assert!(
        !js.contains("moveTopic"),
        "moveTopic is back; the ↑↓ topic-jump was removed 2026-07-24 as confusing — \
         if it is being reinstated, rewrite this test to match"
    );

    // The sheet must credit ↓ with going forward and ↑ with going back, and must not
    // still describe either as a topic jump or vertical-slide movement.
    let next = key_sheet_label(&js, "→ ↓ Space");
    let prev = key_sheet_label(&js, "← ↑");
    assert!(
        next.to_lowercase().contains("next"),
        "the key sheet's forward row (→ ↓ Space) is labelled {next:?}, not a 'next' action"
    );
    assert!(
        prev.to_lowercase().contains("previous") || prev.to_lowercase().contains("back"),
        "the key sheet's back row (← ↑) is labelled {prev:?}, not a 'previous' action"
    );
    for label in [&next, &prev] {
        let l = label.to_lowercase();
        assert!(
            !l.contains("topic") && !l.contains("vertical"),
            "the key sheet still credits an arrow with a topic-jump / vertical move ({label:?}); \
             ↑↓ now navigate linearly like ←→"
        );
    }
}
