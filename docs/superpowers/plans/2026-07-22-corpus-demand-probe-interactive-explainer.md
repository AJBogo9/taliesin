# Corpus demand-probe — interactive-explainer persona Implementation Plan

## Overview

**Goal:** Build a realistic in-scope "interactive-explainer author" project (`corpus/descent/`,
a single long explorable-explanation page on **gradient descent**) that stacks the feature
interactions the current corpus never combines in one narrative — a reactive `{js}` graphic
driven by `{{< input >}}` sliders **and a draggable start point** × a `.scrolly` section with a
`{js}` sticky graphic that redraws per scene × display math interleaved with reactive cells ×
numbered figures with `@fig-` cross-refs — while mining and logging every point of resistance,
then pinning what works and exhibiting it as the **third** marketing-site gallery card.

**Architecture:** `corpus/descent/` is a **website** project (`_site.yml`, **no `chapters:`** — a
single-page site, which is itself a new structural probe: personas 1-2 were both multi-chapter
*books*). One page, `index.tmd`. Authoring is the probe: each resistance point becomes a
categorized finding in a dated notes doc. The parts that render cleanly are locked by a new
`crates/core/tests/descent.rs` pin test (modeled on `course.rs`/`tarn.rs`) and mounted into
`site/` under `/gallery/descent` as a third `site/gallery.tmd` card. **No engine/crate source
changes** — this pass authors a document, writes a test, edits site config, and records findings.

**Tech Stack:** Taliesin `.tmd` (comrak Markdown + `:::` divs incl. `.scrolly`/`.step` +
`{{< input >}}` shortcode + `{js}` cells + `$$` KaTeX math + numbered figures + `@fig-` xrefs),
the `taliesin` CLI (`build`/`preview`/`read`), Rust integration tests (`cargo test -p
taliesin-core`), chrome-devtools MCP for live-interaction verification. The `{js}` cells run
**client-side**, so they execute correctly both standalone and in the `/gallery/descent` mount
(verified this session; F-04 only affects kernel cells → persona 4). No `{python}`/`{r}`.

## Global Constraints

- **No engine source changes.** If a finding needs an engine fix, it is logged + folded to the
  backlog, not fixed here (matches personas 1-2).
- **Single editing surface / load-bearing invariants** untouched. Every block keeps
  `data-block-id` + `data-sourcepos`; the pin test asserts these where relevant.
- **Branch:** `feat/corpus-demand-probe-explainer`; commits stay on the branch; do **not** move
  `main` (author pushes / rebases at request, per the landing mechanics for personas 1-2).
- **Browser-verify** across the viewport matrix (mobile ~390×844, laptop-landscape ~1440×900,
  laptop-portrait ~900×1440), light + dark, **zero console errors**, before claiming done. Use the
  scroll-feature testing discipline (force `scroll-behavior:auto`, settle rAF) for the scrolly.

## The math (fixed, so content is credible)

Anisotropic quadratic bowl (the classic "narrow valley" that motivates both η and momentum):

- Loss: `L(x, y) = ½ (x² + b·y²)`, with `b = 6`. Global minimum at the origin.
- Gradient: `∇L = (x, b·y)`.
- Plain GD: `θ ← θ − η ∇L(θ)`.
- Momentum: `v ← β v − η ∇L(θ)`;  `θ ← θ + v`.
- Level sets `x² + b·y² = 2c` are ellipses (drawn as nested SVG ellipses).
- Drawing domain `x ∈ [−4, 4]`, `y ∈ [−2.5, 2.5]`; default start `θ₀ = (−3.4, 1.4)`.
- Pedagogy: small η → slow but stable; large η → zig-zag/overshoot across the steep y axis;
  momentum → accelerates along the shallow x axis. The draggable start shows basin behavior.

## Runtime contract (authoring against the real API — verified in source)

- `{{< input name="lr" type="slider" min=.. max=.. step=.. value=.. label=".." >}}` emits a
  reactive control; its value rides the URL fragment (shareable state).
- A `{js}` cell body gets `(tali, qmd, Plot, d3, container, invalidation)`; `qmd`≡`tali`.
- `qmd.value(n)` reads a control's live value even if the cell did **not** declare `//| input:`.
- A cell with **no** `//| input:` is a "once" cell: built once, never torn down by the DAG.
- `qmd.onInput(names, cb)` subscribes cb to input changes — redraw **in place**, no teardown.
- `invalidation.then(cleanup)` runs on re-run/unmount — remove any window/document drag listeners
  there (SVG-local listeners GC with the node).
- `.scrolly name="scene"` + inner `{js}` `//| input: scene` reading `qmd.value("scene")`;
  `::: {.step state="..."}` blocks drive the scene value as they scroll into view.

## File structure

```
corpus/descent/
  _site.yml     # website, title "Gradient descent, by hand", description, url off (offline-clean)
  index.tmd     # the one long explainer page
crates/core/tests/descent.rs          # pin test (new)
notes/2026-07-22-corpus-demand-probe-interactive-explainer.md   # findings doc (new)
site/_site.yml           # + mounts: gallery/descent: ../corpus/descent
site/gallery.tmd         # + third exhibit card
corpus/README.md         # + a row for corpus/descent
notes/backlog.md         # + item 18 (persona-3 findings)
```

## Tasks

### Task 1 — Scaffold the single-page site + findings-doc skeleton (green baseline)
**Files:** create `corpus/descent/_site.yml`, `corpus/descent/index.tmd` (title + one intro
paragraph), findings-doc skeleton. Verify `taliesin build corpus/descent` produces one page and
`cargo test -p taliesin-core` stays green.

### Task 2 — Intro + the headline interactive (sliders + draggable start)
**Files:** modify `corpus/descent/index.tmd`.
- Three `{{< input >}}` sliders: `lr` (η), `beta` (momentum), `steps`.
- One **"once"** `{js}` cell: builds an SVG (nested iso-loss ellipses + origin marker +
  descent-path polyline + a draggable start circle), reads sliders via `qmd.value`, redraws the
  path on drag and on `qmd.onInput(["lr","beta","steps"], redraw)`, cleans up on `invalidation`.
- A short caption line reflecting the live end-loss (a second small `//| input:` cell or in-cell
  text node) so the reactive chain is visible.

### Task 3 — The math + Figure 1 + a cross-ref
**Files:** modify `index.tmd`. Prose deriving `∇L` and the update rule in `$$` display math +
inline `$…$`; a numbered figure (the annotated landscape, authored SVG or a small static `{js}`)
as **Figure 1** with an `@fig-landscape` reference in the prose.

### Task 4 — The `.scrolly` walk-through (5 scenes, `{js}` sticky graphic)
**Files:** modify `index.tmd`. `::: {.scrolly name="scene"}` with a sticky `{js}` `//| input:
scene` cell that redraws for scenes `landscape → gradient → step → iterate → diverge`; five
`::: {.step state="..."}` blocks with the narration. This is the highest-risk/highest-yield stack.

### Task 5 — Learning-rate story (Plot figure) + momentum + callouts + takeaways
**Files:** modify `index.tmd`. A **Plot** loss-vs-iteration figure (**Figure 2**, exercises the
vendored Plot/d3 heavy-lib path) comparing a couple of η; a `:::` note callout on choosing η; a
short momentum paragraph; a closing takeaways callout. Keep momentum tight (one paragraph + the
slider already wired in Task 2).

### Task 6 — `read` probe
Run `taliesin read corpus/descent/index.tmd`; log how the text projection handles `.scrolly`,
`{{< input >}}`, reactive `{js}`, and math (expect F-03-like projection friction — a finding, not
a fix here).

### Task 7 — Pin test `crates/core/tests/descent.rs`
Model on `course.rs`/`tarn.rs`. Assert (static-render, no browser): the three `{{< input >}}`
controls emit; the headline + scrolly + Plot `{js}` cells carry their source + input wiring
(`data-inputs`/`application/qmd-js`); the `.scrolly` has `name="scene"` + five `.step`s with the
expected `state`s; Figure numbering + `@fig-landscape` resolves; every block keeps
`data-block-id` + `data-sourcepos`; the page builds standalone. Keep clippy `-D warnings` clean.

### Task 8 — Gallery exhibit
Add `gallery/descent: ../corpus/descent` to `site/_site.yml` mounts; add a third card to
`site/gallery.tmd`; wire the static build (its own `build corpus/descent --out …/gallery/descent`
step, mirroring course/tarn). Add a `corpus/README.md` row.

### Task 9 — Browser-verify (standalone + mounted)
`preview corpus/descent` and `preview site` → `/gallery/descent/`. Verify across the viewport
matrix, light + dark, **0 console errors**: sliders drive the path; the start point drags; the
scrolly graphic changes per scene; Plot figure renders; math renders; `@fig-` link works; URL
state (`#lr=…`) restores. Screenshot the exhibit.

### Task 10 — Findings doc + backlog fold + retro
Categorized findings (repro + disposition), actionable ones folded to `notes/backlog.md` (item
18), a roll-up + retro (go/no-go for persona 4), and a memory update.

## Testing strategy

- **Unit/pin:** `descent.rs` (static-render invariants above) + the full `cargo test -p
  taliesin-core` corpus net stays green.
- **Live:** chrome-devtools MCP is the only way to verify the reactive/drag/scrolly behavior;
  static tests cannot. Both are required before "done".

## Findings doc

`notes/2026-07-22-corpus-demand-probe-interactive-explainer.md`: brief, categorized
(bug/gap/friction/WAI), each with repro + disposition; a "which surfaces produced findings"
progress log; a roll-up; a retro answering "did the recipe hold" + persona-4 note.
