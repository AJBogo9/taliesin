use std::path::Path;

/// taliesin's OWN (AGPL-3.0) bundled scripts. Everything else in `assets/js/` is a
/// vendored third party that MUST be attributed by filename in THIRD_PARTY.md.
/// Adding a new vendored lib without documenting it fails `vendored_js_is_attributed`.
// (code-enhance.js is now authored as per-feature fragments under the
// `code-enhance/` subdirectory, which the non-recursive read_dir below skips.)
const OWN_JS: &[&str] = &[
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

/// Every file under `assets/syntaxes/` is a vendored grammar: taliesin authors none of
/// them, so each must be attributed by filename, and each must ship the upstream licence
/// text beside it. The licence file is the part that is easy to forget and expensive to
/// get wrong — an MIT grammar's one obligation is that its notice travels with it.
#[test]
fn vendored_syntaxes_are_attributed_and_carry_their_licence() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc = third_party_md();
    let dir = core.join("assets/syntaxes");
    let mut grammars = 0;
    for entry in std::fs::read_dir(&dir).expect("assets/syntaxes should exist") {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        if !name.ends_with(".sublime-syntax") {
            continue;
        }
        grammars += 1;
        assert!(
            doc.contains(&name),
            "vendored grammar `{name}` is not attributed in THIRD_PARTY.md"
        );
        let licence = name.replace(".sublime-syntax", ".LICENSE.txt");
        assert!(
            dir.join(&licence).is_file(),
            "vendored grammar `{name}` ships no `{licence}` beside it; the upstream \
             licence text has to travel with the grammar"
        );
    }
    assert!(
        grammars > 0,
        "fixture precondition: at least one grammar should be vendored here, else this \
         gate passes vacuously"
    );
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
    // `Pyodide` covers the whole withdrawn stack: the vendored CPython/WASM runtime went
    // with it, and so did NumPy's wheel and the CPython build inside it. Listing all three
    // keeps a half-reverted re-vendor (bytes back, attribution stale, or the reverse) red.
    // `paged.js` went with the print track on 2026-08-08; its bundle, its version gate and
    // its verbatim MIT notice in `assets/js/LICENSES.md` went in the same commit, so a
    // stale row here would attribute a library this repository no longer redistributes.
    for gone in [
        "reveal.js",
        "highlight.js",
        "Pyodide",
        "NumPy",
        "CPython",
        "paged.js",
    ] {
        assert!(
            !doc.contains(gone),
            "THIRD_PARTY.md still lists removed dependency `{gone}`"
        );
    }
}
