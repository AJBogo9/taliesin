//! The shared `makeScene3D` helper must not paint a theme-blind slab.
//!
//! The helper used to default to `bgColor = 0x111827, alpha = false` and then
//! `setClearColor(bgColor, alpha ? 0 : 1)`, so every caller that did not opt into
//! `alpha: true` cleared to an opaque dark rectangle. On a light page that is a black
//! slab: measured on the showcase Lorenz canvas at `body` background `rgb(255,255,255)`,
//! `readPixels` returned `[11,15,26,255]`. The same held for the three `pca-geometry`
//! scenes (`[17,24,39,255]`, the `0x111827` default) and the `graphics3d/` gallery.
//!
//! The fix is that transparency is not a knob: the canvas is always transparent and the
//! page background (which already flips with the theme via `--tali-*`) shows through. So
//! this pins the *construct*, not a caller's opt-in, in all four copies of the helper,
//! plus the two properties that made the knob's absence safe:
//!
//! * no caller anywhere passes a clear colour (the knob is gone, not merely defaulted);
//! * the helper's DOM chrome is token-driven, so it flips instead of shipping a fixed
//!   dark chip on white.
//!
//! `three-scene.tmd` lives in four copies in two variants. `corpus/_includes` <->
//! `corpus/tech-blog/_includes` is already pinned byte-identical by
//! `corpus.rs::twinned_corpus_sources_stay_byte_identical`; `site/_includes` <->
//! `corpus/graphics3d/_includes` was pinned by nothing, so it is pinned here.

use std::fs;
use std::path::{Path, PathBuf};

/// The repository root (the workspace dir that holds `corpus/` and `site/`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every copy of the `makeScene3D` helper, as (label, source).
fn helper_copies() -> Vec<(&'static str, String)> {
    const PATHS: &[&str] = &[
        "site/_includes/three-scene.tmd",
        "corpus/graphics3d/_includes/three-scene.tmd",
        "corpus/_includes/three-scene.tmd",
        "corpus/tech-blog/_includes/three-scene.tmd",
    ];
    let root = repo_root();
    let copies: Vec<(&'static str, String)> = PATHS
        .iter()
        .map(|rel| {
            let src = fs::read_to_string(root.join(rel))
                .unwrap_or_else(|e| panic!("read {rel}: {e}; a rename must update this pin"));
            (*rel, src)
        })
        .collect();
    // A rename must not silently make this file vacuous.
    assert_eq!(copies.len(), 4, "expected four copies of the helper");
    copies
}

/// Every `.tmd` under `corpus/`, `site/`, and `docs/`: the documents that can call the
/// helper. Skips `_freeze/` (generated) and anything unreadable.
fn all_docs() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<(String, String)>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
        entries.sort_by_key(std::fs::DirEntry::path);
        for e in entries {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if name != "_freeze" && name != "_site" {
                    walk(&p, out);
                }
            } else if p.extension().is_some_and(|x| x == "tmd")
                && let Ok(src) = fs::read_to_string(&p)
            {
                out.push((p.display().to_string(), src));
            }
        }
    }
    let root = repo_root();
    let mut out = Vec::new();
    for sub in ["corpus", "site", "docs"] {
        walk(&root.join(sub), &mut out);
    }
    assert!(
        out.len() > 40,
        "expected the corpus/site/docs walk to find the .tmd documents, found {}",
        out.len()
    );
    out
}

/// The renderer is constructed transparent, unconditionally. This is the exact line that
/// regressed: `alpha` was an option defaulting to `false`, so forgetting it painted a slab.
#[test]
fn every_three_scene_helper_builds_a_transparent_canvas() {
    for (rel, src) in helper_copies() {
        assert!(
            src.contains("new THREE.WebGLRenderer({ antialias: true, alpha: true })"),
            "{rel} must construct the renderer with `alpha: true` unconditionally; \
             a caller-supplied `alpha` is how the dark slab shipped"
        );
        assert!(
            src.contains("renderer.setClearAlpha(0);"),
            "{rel} must clear to a fully transparent buffer"
        );
        // The opaque path and its knob are gone, not merely defaulted differently.
        assert!(
            !src.contains("setClearColor"),
            "{rel} still calls setClearColor; the canvas must never clear to a colour, \
             because no fixed colour is right in both light and dark"
        );
        assert!(
            !src.contains("bgColor"),
            "{rel} still offers a `bgColor` option; a clear-colour knob is a theme-blind \
             slab waiting to be re-armed by the next scene"
        );
    }
}

/// The knob being gone from the helper is only half of it: no document may still pass one.
/// A caller that passes `bgColor:` is either dead config or (before the fix) the bug.
#[test]
fn no_document_asks_a_three_scene_for_a_background_colour() {
    let offenders: Vec<String> = all_docs()
        .into_iter()
        .filter(|(_, src)| src.contains("bgColor"))
        .map(|(p, _)| p)
        .collect();
    assert!(
        offenders.is_empty(),
        "these documents still pass a clear colour to makeScene3D:\n  {}",
        offenders.join("\n  ")
    );
}

/// The helper's own DOM chrome (the Fullscreen button) was a hard-coded
/// `rgba(30,30,30,.75)` / `#ddd` / `#555` chip: unreadable-by-design on a light page and
/// unable to follow a theme toggle. It must be built from `--tali-*` tokens instead.
#[test]
fn the_three_scene_fullscreen_button_is_token_driven() {
    for (rel, src) in helper_copies() {
        let at = src
            .find("const btnStyle = [")
            .unwrap_or_else(|| panic!("{rel}: no btnStyle block; this pin must be re-aimed"));
        let end = src[at..]
            .find("].join(\";\");")
            .unwrap_or_else(|| panic!("{rel}: unterminated btnStyle block"));
        let style = &src[at..at + end];

        for needle in [
            "background:color-mix(in srgb, var(--tali-bg) 78%, transparent)",
            "color:var(--tali-fg)",
            "border:1px solid var(--tali-border-strong)",
            "border-radius:var(--tali-radius-sm)",
        ] {
            assert!(
                style.contains(needle),
                "{rel}: the Fullscreen button must carry `{needle}`, so it flips with the theme"
            );
        }
        assert!(
            !style.contains('#') && !style.contains("rgba(") && !style.contains("rgb("),
            "{rel}: the Fullscreen button still carries a raw colour literal:\n{style}"
        );
    }
}

/// The two *extended* copies (the ones with `controls`/`autoRotate`/`rebuild`/`loadGLTF`)
/// are hand-kept identical and nothing pinned that, so a theme fix could land in one and
/// rot the other exactly the way the older pair's pin exists to prevent.
#[test]
fn the_extended_three_scene_copies_stay_byte_identical() {
    let root = repo_root();
    let a = root.join("site/_includes/three-scene.tmd");
    let b = root.join("corpus/graphics3d/_includes/three-scene.tmd");
    let (sa, sb) = (fs::read(&a).unwrap(), fs::read(&b).unwrap());
    assert_eq!(
        sa, sb,
        "site/_includes/three-scene.tmd and corpus/graphics3d/_includes/three-scene.tmd \
         have drifted; a fix landed in one copy only"
    );
    // Guard against the pin passing because both files are the *older* variant: that
    // would mean a drive-by "sync" silently dropped the extended features.
    let src = String::from_utf8(sa).unwrap();
    for marker in ["autoRotate", "loadGLTF", "frameObject", "function rebuild("] {
        assert!(
            src.contains(marker),
            "the extended helper lost `{marker}`; the two variants must not be merged"
        );
    }
}
