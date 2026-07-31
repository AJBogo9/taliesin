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
    // First-party and deliberately so: `{glsl}` needs no vendored library because WebGL is
    // a browser API, and `numerics.js` is written here rather than pulling in jStat so that
    // it stays the curated set a document actually needs.
    "glsl.js",
    "numerics.js",
    // `pyodide.js` is taliesin's own enhancer — a registration against tali-js.js. The
    // VENDORED Pyodide runtime it loads lives in `assets/pyodide/` and is attributed by
    // `the_vendored_pyodide_payload_is_complete_and_carries_its_licence`.
    "pyodide.js",
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

/// paged.js is vendored for the print track (backlog 159): it supplies the CSS Paged Media
/// Level 3 features Chrome does not implement natively — `string-set` running heads and
/// `target-counter()` page references (measured against Chrome 150, 2026-07-31).
///
/// Unlike every other bundle in `assets/js/`, this one is **not** copied into every built
/// page: it is inlined only into the transient page `render/print.rs` assembles. It is still
/// redistributed in this repository, so it owes the same attribution + verbatim notice.
///
/// The version is read from the bundle's own `@license` banner rather than asserted as a
/// literal, so re-vendoring without updating the docs goes red instead of silently lying.
#[test]
fn the_pagedjs_version_claim_matches_the_vendored_library() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = std::fs::read_to_string(core.join("assets/js/paged.polyfill.min.js"))
        .expect("the vendored paged.js polyfill should exist");
    // `/** @license Paged.js v0.4.3 | MIT | https://… */`
    let anchor = lib
        .find("Paged.js v")
        .expect("the paged.js bundle should carry its @license banner");
    let rest = &lib[anchor + "Paged.js v".len()..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .expect("the banner version should be delimited");
    let version = &rest[..end];
    assert!(
        version.split('.').count() == 3,
        "expected an x.y.z paged.js version, got `{version}`"
    );

    let doc = third_party_md();
    assert!(
        doc.contains(&format!("v{version}")),
        "THIRD_PARTY.md claims a different paged.js version than the vendored bundle, \
         which reports {version}. Update the attribution when you re-vendor it."
    );
    assert!(
        lib.contains("MIT"),
        "the paged.js banner should name its MIT licence"
    );
    // MIT requires the permission notice to travel, not just the copyright line, and the
    // minified bundle carries only the one-line banner — so the verbatim text must ship
    // beside it like d3's and Plot's do.
    let notices = std::fs::read_to_string(core.join("assets/js/LICENSES.md"))
        .expect("assets/js/LICENSES.md should exist");
    assert!(
        notices.contains("paged.js") || notices.contains("Paged.js"),
        "assets/js/LICENSES.md must carry paged.js's verbatim MIT permission notice"
    );
    assert!(
        notices.contains("Adam Hyde"),
        "paged.js's MIT notice names Adam Hyde as the copyright holder"
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

/// Pyodide is vendored for `{pyodide}` cells (backlog 158): a CPython + NumPy stack compiled
/// to WebAssembly, so client-side Python runs with no kernel and no network.
///
/// **The version and the licence are both READ from upstream's own `package.json`**, never
/// asserted as literals, so re-vendoring a new Pyodide without updating THIRD_PARTY.md goes
/// red. The minified `pyodide.mjs` is deliberately not the source: its only version
/// occurrence is `var Y="…"`, and `Y` is a build artifact that changes between releases, so
/// a test anchored there would break on a re-vendor that was otherwise correct.
///
/// `package.json` is vendored for exactly this reason and is NOT part of the browser
/// payload — `pyodide_payload()` does not serve it.
#[test]
fn the_pyodide_version_and_licence_claims_match_the_vendored_runtime() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let meta = std::fs::read_to_string(core.join("assets/pyodide/package.json"))
        .expect("the vendored pyodide package.json should exist");

    /// Pull a string value out of upstream's package.json by key.
    fn field<'a>(meta: &'a str, key: &str) -> &'a str {
        let anchor = format!("\"{key}\"");
        let at = meta
            .find(&anchor)
            .unwrap_or_else(|| panic!("pyodide package.json should carry a `{key}` field"));
        let rest = &meta[at + anchor.len()..];
        let open = rest
            .find('"')
            .expect("a quoted value should follow the key");
        let rest = &rest[open + 1..];
        let close = rest.find('"').expect("the value should be terminated");
        &rest[..close]
    }

    let version = field(&meta, "version");
    let licence = field(&meta, "license");
    assert!(
        version.split('.').count() == 3 && version.starts_with(char::is_numeric),
        "expected an x.y.z pyodide version from package.json, got `{version}`"
    );

    let doc = third_party_md();
    assert!(
        doc.contains(version),
        "THIRD_PARTY.md claims a different Pyodide version than the vendored runtime \
         (package.json says `{version}`)"
    );
    assert!(
        doc.contains(licence),
        "THIRD_PARTY.md claims a different Pyodide licence than upstream declares \
         (package.json says `{licence}`)"
    );
}

/// Every file under `assets/pyodide/` is vendored third-party code, and the directory MUST
/// carry the upstream licence text beside it — MPL-2.0 §3.4 forbids removing notices, and the
/// `pyodide-core` tarball ships no LICENSE of its own, so this is the only copy there is.
///
/// The completeness half matters as much as the attribution half: `pyodide.mjs` resolves its
/// siblings by fixed name at runtime, so a payload missing one file fails in the reader's
/// browser with a 404 and no server-side symptom at all.
#[test]
fn the_vendored_pyodide_payload_is_complete_and_carries_its_licence() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = core.join("assets/pyodide");
    for required in [
        "pyodide.mjs",
        "pyodide.asm.mjs",
        "pyodide.asm.wasm",
        "python_stdlib.zip",
        "pyodide-lock.json",
        "numpy-2.4.3-cp314-cp314-pyemscripten_2026_0_wasm32.whl",
        "LICENSE",
        // Provenance only: upstream's package.json, not part of the served browser payload.
        "package.json",
    ] {
        assert!(
            dir.join(required).is_file(),
            "the vendored Pyodide payload is missing `{required}` — the runtime resolves its \
             siblings by fixed name, so this fails only in the reader's browser"
        );
    }
    let licence = std::fs::read_to_string(dir.join("LICENSE")).expect("LICENSE readable");
    assert!(
        licence.contains("Mozilla Public License Version 2.0"),
        "assets/pyodide/LICENSE should be the MPL-2.0 text"
    );
    let doc = third_party_md();
    for claim in ["Pyodide", "NumPy", "CPython"] {
        assert!(
            doc.contains(claim),
            "THIRD_PARTY.md must attribute `{claim}` — it is redistributed inside \
             assets/pyodide/"
        );
    }
}
