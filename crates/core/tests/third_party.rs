use std::path::Path;

/// taliesin's OWN (MIT) bundled scripts. Everything else in `assets/js/` is a
/// vendored third party that MUST be attributed by filename in THIRD_PARTY.md.
/// Adding a new vendored lib without documenting it fails `vendored_js_is_attributed`.
// (code-enhance.js is now authored as per-feature fragments under the
// `code-enhance/` subdirectory, which the non-recursive read_dir below skips.)
const OWN_JS: &[&str] = &[
    "deck.js",
    "mermaid.js",
    "qmd-js.js",
    "walkthrough.js",
    "tabset.js",
    "scrolly.js",
];

fn third_party_md() -> String {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(core.join("../../THIRD_PARTY.md"))
        .expect("THIRD_PARTY.md should exist at the repo root")
}

#[test]
fn vendored_js_is_attributed() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc = third_party_md();
    let js_dir = core.join("assets/js");
    for entry in std::fs::read_dir(&js_dir).expect("assets/js should exist") {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        if !name.ends_with(".js") || OWN_JS.contains(&name.as_str()) {
            continue;
        }
        assert!(
            doc.contains(&name),
            "vendored asset `{name}` is not attributed in THIRD_PARTY.md \
             (document it, or add it to OWN_JS if it is taliesin's own)"
        );
    }
}

#[test]
fn removed_deps_are_not_listed() {
    let doc = third_party_md();
    for gone in ["reveal.js", "highlight.js"] {
        assert!(
            !doc.contains(gone),
            "THIRD_PARTY.md still lists removed dependency `{gone}`"
        );
    }
}
