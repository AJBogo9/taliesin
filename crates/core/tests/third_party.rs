use std::path::Path;

/// taliesin's OWN (AGPL-3.0) bundled scripts. Everything else in `assets/js/` is a
/// vendored third party that MUST be attributed by filename in THIRD_PARTY.md.
/// Adding a new vendored lib without documenting it fails `vendored_js_is_attributed`.
// (code-enhance.js is now authored as per-feature fragments under the
// `code-enhance/` subdirectory, which the non-recursive read_dir below skips.)
const OWN_JS: &[&str] = &["mermaid.js", "tali-js.js"];

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
    // Every name in OWN_JS must still be a file. An entry is an EXEMPTION from the
    // attribution assertion below, so once the script it named is deleted the entry becomes
    // a filename a future vendored bundle could arrive under and be waved through with no
    // THIRD_PARTY.md row. That is not hypothetical: a deleted script's name was left here
    // after wave 5, and three more after wave 7 — inert each time, and inert is one `git mv`
    // away from a licence hole. Stat each name directly rather than intersecting with the
    // walk below, so a future non-`.js` entry cannot pass silently.
    for own in OWN_JS {
        assert!(
            js_dir.join(own).is_file(),
            "OWN_JS names `{own}`, which no longer exists in assets/js. An entry for a \
             deleted script is a filename a future vendored bundle could be silently \
             exempted from attribution under -- delete it with the script."
        );
    }
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

/// The release tarball must carry every verbatim notice the binary's own contents oblige
/// it to, and nothing else in the tree looks at the workflow that builds it.
///
/// Wave 1 deleted `crates/core/tests/release_targets.rs`, which held this assertion, and
/// recorded the loss in as many words as "an AGPL distribution claim, now unguarded …
/// cheap to restore as ~10 lines if that matters at release time". It is now release time.
/// The binary embeds d3, Observable Plot and Mermaid via `include_str!` and KaTeX's CSS via
/// `build.rs`; MIT and ISC both require the **permission notice**, not merely the copyright
/// line, to accompany a copy, and `THIRD_PARTY.md` gives copyright lines and URLs — which
/// that file's own standard says is not a notice.
///
/// A `v*` tag is one-shot and no `v*` tag has ever run, so a missing `cp` here would first
/// be observed by whoever downloads the launch tarball.
#[test]
fn the_release_tarball_carries_every_notice_it_must() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let wf = std::fs::read_to_string(root.join(".github/workflows/release.yml"))
        .expect("the release workflow should exist at .github/workflows/release.yml");
    // Anti-vacuity. Matching filenames anywhere in the YAML would keep passing against a
    // workflow that had stopped packaging anything, so require the step itself to be there.
    assert!(
        wf.contains("name: Package"),
        "`.github/workflows/release.yml` no longer has a `Package` step -- this gate is \
         asserting file names against a workflow that may not build a tarball at all"
    );
    // Exact shell tokens, not substrings: `LICENSE` is a prefix of
    // `LICENSE-OUTPUT-EXCEPTION.md`, so a `contains` check would go on reporting the root
    // AGPL text as packaged after someone deleted it from the `cp`.
    let packaged: std::collections::BTreeSet<&str> = wf
        .split_whitespace()
        .map(|t| t.trim_matches(|c| c == ';' || c == '\\' || c == '"'))
        .collect();
    for rel in [
        "LICENSE",
        "LICENSE-OUTPUT-EXCEPTION.md",
        "THIRD_PARTY.md",
        "crates/core/assets/js/LICENSES.md",
        "crates/core/assets/katex/LICENSE",
        "crates/core/assets/fonts/newsreader-OFL-fontsource.txt",
    ] {
        assert!(
            root.join(rel).is_file(),
            "the release Package step names `{rel}`, which does not exist in the tree -- \
             the tag would fail on `cp`, having already created the release"
        );
        assert!(
            packaged.contains(rel),
            "the release tarball would ship without `{rel}`. A downloaded binary is a \
             distribution, and these are the terms and notices it is distributed under."
        );
    }
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
    // `PowerShell` is the vendored `.sublime-syntax` grammar, deleted 2026-08-09 with the
    // whole `assets/syntaxes/` directory. It is the one entry here that was never a
    // *dependency*: it was redistributed source, so a stale row would be a licence claim
    // about bytes this binary no longer carries.
    for gone in [
        "reveal.js",
        "highlight.js",
        "Pyodide",
        "NumPy",
        "CPython",
        "paged.js",
        "PowerShell",
    ] {
        assert!(
            !doc.contains(gone),
            "THIRD_PARTY.md still lists removed dependency `{gone}`"
        );
    }
}
