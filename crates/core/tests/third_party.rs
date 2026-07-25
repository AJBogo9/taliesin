use std::path::Path;

/// taliesin's OWN (AGPL-3.0) bundled scripts. Everything else in `assets/js/` is a
/// vendored third party that MUST be attributed by filename in THIRD_PARTY.md.
/// Adding a new vendored lib without documenting it fails `vendored_js_is_attributed`.
// (code-enhance.js is now authored as per-feature fragments under the
// `code-enhance/` subdirectory, which the non-recursive read_dir below skips.)
const OWN_JS: &[&str] = &[
    "deck.js",
    "mermaid.js",
    "tali-js.js",
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

/// The Mermaid version is claimed in three places that can drift apart silently: the
/// attribution line, the CDN fallback URL, and the doc comment on the vendored blob. A
/// stale attribution is the failure mode that matters — it is what a downstream consumer
/// (or an audit) reads to decide whether a known CVE applies to this build.
#[test]
fn the_mermaid_version_claim_matches_the_vendored_library() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = std::fs::read_to_string(core.join("assets/js/mermaid.min.js"))
        .expect("the vendored mermaid library should exist");
    // Every esbuild mermaid bundle carries its own version string as `<ident>="11.x.y"`
    // right before the `getVersion` accessor. Find it by anchoring on that accessor.
    let anchor = lib
        .find("\"getVersion\"")
        .expect("the mermaid bundle should define getVersion");
    let version = lib[..anchor]
        .rmatch_indices('"')
        .filter_map(|(i, _)| {
            let rest = &lib[i + 1..];
            let end = rest.find('"')?;
            let candidate = &rest[..end];
            let mut parts = candidate.split('.');
            let ok = candidate.len() >= 5
                && parts.clone().count() == 3
                && parts.all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
            ok.then(|| candidate.to_string())
        })
        .next()
        .expect("the mermaid bundle should carry an x.y.z version string");

    let doc = third_party_md();
    assert!(
        doc.contains(&format!("v{version}")),
        "THIRD_PARTY.md claims a different Mermaid version than the vendored \
         `mermaid.min.js`, which reports {version}. Update the attribution when you \
         re-vendor the library."
    );
    let render = std::fs::read_to_string(core.join("src/render/mod.rs")).unwrap();
    assert!(
        render.contains(&format!("mermaid@{version}/dist")),
        "the CDN fallback URL in render/mod.rs pins a different Mermaid version than the \
         vendored library ({version}); a reader who hits the fallback would silently get \
         a different build than the one this binary ships"
    );
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
