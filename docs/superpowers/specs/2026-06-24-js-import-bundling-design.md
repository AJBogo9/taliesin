# Design: P3c — bundle `{js}` cell local imports in single-doc `build --out`

Status: approved 2026-06-24. Branch `feat/js-import-bundling`. From `backlog.md` Open/next
(Format & structure audit round 3). Build-time only; zero new dependencies.

## Problem

`copy_local_assets` (`crates/server/src/main.rs`) bundles a built doc's assets by scanning
the rendered HTML's `src=`/`href=` attributes. A `{js}` cell's `await import("./helper.js")`
lives in the cell's *script body*, not an attribute, so it's invisible — a standalone
interactive post built with `build doc.qmd --out <dir>` 404s on its helper. The corpus hits
this: `corpus/posts/em-algorithm` does `await import("./em-helpers.js")`. (pca-geometry's
`import("https://esm.sh/three…")` is remote and must NOT be copied.)

## Approach (approved: any relative quoted specifier)

Add a `{js}`-import bundling pass to `copy_local_assets`, recursive, zero-dep:
- Extract the bodies of `<script type="application/qmd-js">…</script>` cells from the HTML.
- In each, collect every quoted string literal starting with `./` or `../` (bundles
  `import()`/`from`/`fetch()` alike). Remote (`https://…`) + bare specifiers are ignored.
- Resolve each against the doc's base dir, copy to the same relative path under `dest`.
- **Recurse:** read each copied `.js`/`.mjs` and scan it for relative specifiers (resolved
  against *its own* dir), following the chain. Dedup via a visited set; reject + warn on
  specifiers that escape the tree (`..` above base / absolute), reusing the existing
  escape-guard ethos. In-place builds skip the self-copy but still recurse.

Slightly over-inclusive (a relative string that isn't a real reference) but benign:
non-existent paths are skipped, tree-escaping paths warn.

## New helpers (`main.rs`)

- `qmd_js_cell_sources(html: &str) -> Vec<&str>` — bodies of `application/qmd-js` scripts.
- `relative_specifiers(src: &str) -> Vec<String>` — quoted literals starting `./`/`../`.
- `normalize_rel(dir: &str, spec: &str) -> Option<String>` — join importer's dir + spec,
  collapse `.`/`..`; `None` if it escapes base.
- `copy_js_imports(html, base, dest) -> usize` — the recursive worklist; called from
  `copy_local_assets`, its count added to the existing `src=`/`href=` count.

## Test (TDD, `#[cfg(test)]` in `main.rs`)

Construct HTML with a `{js}` cell doing `await import("./helper.js")` and
`await import("https://esm.sh/three")`, plus `<img src="pic.png">`; on disk under a temp
base: `helper.js` (which `import`s `./util.js`), `util.js`, `pic.png`, and an unreferenced
`secret.js`. Call `copy_local_assets(html, base, out)` and assert:
- COPIED: `helper.js` (direct), `util.js` (recursion), `pic.png` (`src=` scan);
- NOT copied: `secret.js` (unreferenced), and no file created for the remote `https://…`
  specifier.

Plus a real `build corpus/posts/em-algorithm/index.qmd --out <tmp>` confirms `em-helpers.js`
lands next to `index.html`.

## Invariants

Build-time only; no change to render/exec or the emitted HTML (paths stay as-authored, the
folder stays portable); zero new deps; tree-escape guard preserved.

## Out of scope

Rewriting import paths; bundling remote/bare specifiers; the multi-page `build <site>` path
(its `mirror_assets` already copies sibling files — P3a-adjacent, separate). P3b (`mounts:`).
