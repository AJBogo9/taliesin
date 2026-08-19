# The gallery as a self-contained demo site

Date: 2026-08-19. Status: awaiting author review.

## Goal

Rebuild the gallery as one flat, self-contained Taliesin project of short one-page
demos. Its job changes from "whole projects you can open" to "boast: show the most
impressive things Taliesin can do on a single page each". Everything the site
publishes lives under `gallery/`; the publish-time composition of corpus projects is
deleted. Still one deploy on gallery.taliesin.sh; still four deploys total.

## Shape

`gallery/` becomes a single project with one `_site.yml`:

```
gallery/
  _site.yml          one config: title, url, nav, footer (external-prefixes removed)
  index.tmd          the pitch page, rewritten: short intro + one section per demo
  descent.tmd        demo 1 (plus landscape.svg, momentum.svg alongside)
  report.tmd         demo 2 (plus data/queue_latency.csv, trimmed)
  api-craft.tmd      demo 3
  molecules.tmd      demo 4
  gears.tmd          demo 5
  _includes/
    three-scene.tmd  the shared three.js scene helper, recovered from git history
```

Exact demo file names may be adjusted during implementation; the roster is fixed.
`_includes/` is skipped by page discovery (the standing `_`-prefix rule), so the
helper is not a page. `preview gallery` and `build gallery` handle the whole site;
Cmd-K search spans all demos automatically because search is site-level.

## The demo roster

Each demo is one page, distilled to impress, not to document.

1. **Gradient descent, by hand** (from `corpus/descent`, near-verbatim). Reactive
   sliders, a draggable `{js}` graphic, a select-driven scene walk, a live
   Observable Plot chart, real math. Its two theme-adaptive SVGs come along.
2. **A computational report, in one page** (distilled from `corpus/analyst`).
   `{python}` cleans and charts a small committed CSV at build time; generated
   figures and tables share one numbered counter with authored ones, and the prose
   cross-references them.
3. **The craft of an API page** (distilled from `corpus/tarn`). Line-by-line code
   reading, version and deprecation callouts, a definition-list glossary,
   cross-references, server-side highlighting.
4. **Molecules** (resurrected from `corpus/graphics3d/molecules.tmd` at
   `834bb89a^`, trimmed). A ball-and-stick viewer built entirely in a `{js}` cell:
   inline public-domain XYZ coordinates, CPK colors, drag to spin, scroll to zoom.
5. **A mechanical part, parametric** (new). A gear or small assembly built from
   three.js primitives, with `{{< input >}}` sliders driving tooth count and speed.
   Procedural geometry only: no committed model binaries, ever (the 5.4 MB glTF
   sample was part of the wave 6 cut and stays cut).

### 3D is content, not a feature rebuild

`corpus/graphics3d/` went in cut wave 6 (commit `834bb89a`, 2026-08-08) together
with `{glsl}` and the headless-browser test driver. The reactive core the demos
need (`{js}` cells, the graph, `{{< input >}}`, `tali.get`) survived whole, and the
old scene helper was plain cell code importing a pinned three.js from
`https://esm.sh/three@0.163.0` at view time. The demos reuse exactly that pattern:

- The scene helper returns to `gallery/_includes/three-scene.tmd`, adapted from
  git history (transparent canvas, theme-following labels, fullscreen).
- three.js stays a pinned esm.sh import, matching the helper's original design.
  Nothing is vendored into core assets, `{glsl}` is not rebuilt, and no browser
  test driver returns. Trade-off, accepted: the two 3D demos depend on esm.sh at
  view time; every other page stays self-hosted. Revisit only if that dependency
  bites.

## What gets deleted

**The composition.** `tools/publish.sh` loses `GALLERY_EXHIBITS`, the composed
build in `build_target`, the parent-first ordering (and its comment block), and the
exhibit-link resolution. The gallery builds and deploys exactly like the other
three sites. The corpus.rs tests that read `GALLERY_EXHIBITS` back and cross-check
`corpus/README.md`'s deploy column die in the same commit as the composition they
gate (the ordering rule).

**The `external-prefixes` key**, cut from core entirely; the gallery was its only
user and nothing in the tree nests projects any more. Withdrawal per the standing
convention (delete the read, not just the vocabulary):

- `crates/core/src/site/config/mod.rs`: the field, the parse at ~319, the
  `KNOWN` entry at ~150, and the doc comments.
- `crates/core/src/site/chrome.rs` ~277 and `crates/core/src/site/mod.rs` ~791:
  the link-checker branches that consulted it, plus the chrome.rs test at ~1123.
- Both schema copies: `crates/core/assets/schema/tali-site.schema.json` and
  `editor/vscode/schema/tali-site.schema.json`.
- Docs rows: `docs/guide/reference/frontmatter.tmd` ~390 and the nested-build
  paragraph in `docs/guide/reference/cli.tmd` ~296.
- A parser-side pin: a test asserting the key now draws the unknown-key
  diagnostic and that links into a formerly-external prefix are reported broken.

**Prose that becomes false.** `corpus/README.md` (the three exhibit rows and the
line calling them "the three exhibits publish.sh deploys"), `docs/guide/index.tmd`
line 31 ("whole projects"), `site/README.md`'s gallery row is checked for stale
wording during implementation, and CLAUDE.md's gallery paragraphs (the "ONE project
that builds others" sentences in the tree map and the deploy notes).

## What stays untouched

`corpus/tarn`, `corpus/descent`, and `corpus/analyst` remain exactly where they
are, now as unpublished corpus goldens. Every test that reads them keeps its
fixture unchanged: `tarn.rs` (412 lines of book pins), `book_has_no_rail_toc.rs`,
`descent.rs`, `analyst.rs`, `exec_pool.rs`, and the generic corpus sweeps. The
gallery demos will drift from their corpus cousins over time, and that is
deliberate: the corpus records, the gallery boasts. Verified: the three exhibits'
anchor and xref labels are disjoint, and no external site deep-links into
`gallery.taliesin.sh/<exhibit>`; only root links exist.

## Gates and verification

- `build gallery --check-only` must be clean; `tools/publish.sh --check` (already
  in pre-push and gates.sh) builds the simplified gallery with `--no-exec`.
- The report demo executes Python at deploy exactly as the composed analyst did,
  so no new interpreter requirement appears anywhere.
- The `{js}` and 3D demos cannot be gate-executed (the browser driver was cut);
  each demo page is verified in the live preview via the chrome-devtools loop
  (console clean, both themes, interactions exercised) before it ships.
- Full `./tools/gates.sh` green before any commit claims completion.

## Non-goals

No book chrome in the gallery, no new core features or `_site.yml` keys, no
rebuilt `{glsl}`, no vendoring of three.js into core assets, no committed model
binaries, no new deploys, no hero/listing machinery on the index.
