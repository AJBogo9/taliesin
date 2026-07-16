//! The deck's in-product key sheet must not drift from what the keys actually do.
//!
//! It did: the sheet advertised "Vertical slides" for ↑↓ while `up()`/`down()` called
//! `moveTopic`. The 2026-07-12 deck audit's drift sweep corrected the `.tmd` docs and
//! never read the string every presenter opens, so these pins read the *binding* and
//! the *sheet* together rather than pinning either one's wording alone.

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

/// ↑↓ jump between topics (keeping the column); they do not step vertical slides.
/// Guard the binding first: if ↑↓ ever stop being `moveTopic`, the wording pin below
/// is measuring the wrong thing and must be rechecked rather than silently passing.
#[test]
fn key_sheet_describes_up_down_as_the_topic_jump_it_is() {
    let js = deck_js();
    assert!(
        js.contains("function down() { moveTopic(1); }")
            && js.contains("function up() { moveTopic(-1); }"),
        "↑↓ no longer map to moveTopic; recheck the key-sheet wording pin in this file"
    );
    assert!(
        js.contains("case 'ArrowDown': down(); break;")
            && js.contains("case 'ArrowUp': up(); break;"),
        "ArrowUp/ArrowDown no longer route to up()/down(); recheck the key-sheet wording pin"
    );

    let label = key_sheet_label(&js, "↑ ↓");
    assert!(
        label.to_lowercase().contains("topic"),
        "the key sheet calls ↑↓ {label:?}, but up()/down() call moveTopic: they jump topics, \
         keeping the column. Vertical slides are stepped with ←→ (see `next`/`prev`). \
         docs/guide/using/formats.tmd says \"Jump to the topic above / below\"."
    );
    assert!(
        !label.to_lowercase().contains("vertical"),
        "the key sheet still credits ↑↓ with vertical-slide movement ({label:?}); ←→ do that"
    );
}
