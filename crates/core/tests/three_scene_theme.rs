//! The shared `makeScene3D` helper must not paint a theme-blind slab.
//!
//! The helper used to default to `bgColor = 0x111827, alpha = false` and then
//! `setClearColor(bgColor, alpha ? 0 : 1)`, so every caller that did not opt into
//! `alpha: true` cleared to an opaque dark rectangle. On a light page that is a black
//! slab: measured on the showcase Lorenz canvas at `body` background `rgb(255,255,255)`,
//! `readPixels` returned `[11,15,26,255]`. The same held for the three `pca-geometry`
//! scenes (`[17,24,39,255]`, the `0x111827` default).
//!
//! The fix is that transparency is not a knob: the canvas is always transparent and the
//! page background (which already flips with the theme via `--tali-*`) shows through. So
//! this pins the *construct*, not a caller's opt-in, in every copy of the helper the walk
//! below finds, plus the two properties that made the knob's absence safe:
//!
//! * no caller anywhere passes a clear colour (the knob is gone, not merely defaulted);
//! * the helper's DOM chrome is token-driven, so it flips instead of shipping a fixed
//!   dark chip on white.
//!
//! `three-scene.tmd` lives in more than one copy, in two variants: an EXTENDED one
//! (`controls`/`autoRotate`/`rebuild`/`loadGLTF`, used by the marketing site) and a base
//! one. The copies are **discovered by walking the tree**, not listed. That is the fix for
//! a real defect: the list used to name four paths while five files existed, and a copy
//! under a duplicated corpus post drifted unpinned under a gate whose own assertion
//! claimed it covered every copy. A walk cannot undercount, which is why the duplicate
//! cull on 2026-08-09 took three of the five copies without leaving a hole here.

use std::fs;
use std::path::{Path, PathBuf};

/// The repository root (the workspace dir that holds `corpus/` and `site/`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every copy of the `makeScene3D` helper, as (repo-relative path, source), found by
/// walking rather than by a hand-kept list — see this file's header for why.
fn helper_copies() -> Vec<(String, String)> {
    let root = repo_root();
    let mut copies: Vec<(String, String)> = all_docs()
        .into_iter()
        .filter(|(_, src)| src.contains("makeScene3D"))
        .filter(|(p, _)| p.ends_with("three-scene.tmd"))
        .map(|(p, src)| {
            let rel = Path::new(&p)
                .strip_prefix(&root)
                .map(|r| r.display().to_string())
                .unwrap_or(p);
            (rel, src)
        })
        .collect();
    copies.sort();
    // A rename must not silently make this file vacuous. Two is the floor because two is
    // what survives the 2026-08-09 duplicate cull: one copy per variant, which is also the
    // minimum `same_variant_three_scene_copies_stay_byte_identical` needs to stay honest.
    assert!(
        copies.len() >= 2,
        "expected at least one copy of the helper per variant, found {}: {copies:#?}",
        copies.len()
    );
    copies
}

/// Every `.tmd` under `corpus/`, `site/`, and `docs/`: the documents that can call the
/// helper. Skips build output (`_freeze/`, `_site/`, `_book/`) and anything unreadable.
fn all_docs() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<(String, String)>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
        entries.sort_by_key(std::fs::DirEntry::path);
        for e in entries {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                // `_book` belongs here beside the other two. It is build output exactly as
                // they are, and every other walker in the tree already lists all three
                // (`serve/mod.rs`'s SKIP_DIRS, `stale_docs.rs`, `retired_names.rs`,
                // `svg_assets_render.rs`, `parallel_build_determinism.rs`). This one was
                // short, harmless only because `_book` holds no `.tmd` today.
                if name != "_freeze" && name != "_site" && name != "_book" {
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

/// Copies of the SAME variant are hand-kept identical and must stay so, or a theme fix
/// lands in one and rots the others. Grouped by content rather than by a listed pair,
/// because the variant a given copy belongs to is a property of the file, not of a name:
/// the base variant has one copy today (`corpus/tech-blog/_includes/`) and so does the
/// extended one (`site/_includes/`, carrying `controls`/`autoRotate`/`rebuild`/`loadGLTF`).
///
/// The extended variant HAD a second copy under `corpus/graphics3d/` (cut 2026-08-08) and
/// the base variant had three (`corpus/_includes/` and the loose `corpus/posts/`
/// pca-geometry copy went with the duplicate cull on 2026-08-09). One copy cannot drift, so
/// the assertion below simply has nothing to compare, but it still fails loudly if a
/// future edit merges the two variants, which is the other half of what the old paired pin
/// was buying, and it fails again the moment a second copy of either variant reappears.
#[test]
fn same_variant_three_scene_copies_stay_byte_identical() {
    let copies = helper_copies();

    let mut extended: Vec<&(String, String)> = Vec::new();
    let mut base: Vec<&(String, String)> = Vec::new();
    for c in &copies {
        if ["autoRotate", "loadGLTF", "frameObject", "function rebuild("]
            .iter()
            .all(|m| c.1.contains(m))
        {
            extended.push(c);
        } else {
            base.push(c);
        }
    }

    // Both variants must still exist, or a drive-by "sync" merged them and this test
    // would pass by having nothing left to compare.
    assert!(
        !extended.is_empty() && !base.is_empty(),
        "the two three-scene variants must not be merged; found extended={:?} base={:?}",
        extended.iter().map(|c| &c.0).collect::<Vec<_>>(),
        base.iter().map(|c| &c.0).collect::<Vec<_>>()
    );

    for group in [&extended, &base] {
        let (first_path, first_src) = group[0];
        for (path, src) in group.iter().skip(1) {
            assert_eq!(
                src, first_src,
                "{path} has drifted from {first_path}; a fix landed in one copy only"
            );
        }
    }
}
