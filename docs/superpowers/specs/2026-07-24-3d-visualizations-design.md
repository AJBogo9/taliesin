# 3D & interactive visualizations: molecules, Lorenz, CAD, sorting

- **Date:** 2026-07-24
- **Status:** approved design, pending spec review
- **Topic:** add a set of genuinely-flashy, genuinely-real interactive visual demos
  (spin-and-fullscreen molecules, an animated Lorenz attractor, a CAD viewer + a
  live parametric part, and a sorting-algorithm visualizer) to the marketing site
  and docs, pinned by a new corpus exhibit.

## Goal

Add high-"wow" interactive graphics that market Taliesin by *being* the proof: each
demo is a real capability an author could write, its shown source is the actual
source that produced it, and it runs client-side in the static built page. This
extends the existing self-referential marketing showcase (`site/showcase.tmd`) and
the existing `makeScene3D` 3D pattern rather than inventing a parallel mechanism.

Guiding constraints (inherited from the 2026-06-27 marketing-showcase work):

1. **Realistic, not faked.** Every demo is a capability a real author could write;
   the source shown beside each result is the *actual* source. No bespoke engine
   tricks unavailable to users.
2. **Flashy by selection.** We pick the inherently impressive subset (interactive
   3D, chaos, CAD, sorting) and present it well.
3. **Self-referential.** Each headline demo pairs the result with its real `{js}`
   source via the framework's own `.panel-tabset`, so the reveal mechanism is
   itself a feature on display.
4. **Content-only.** No Rust/engine changes. If a demo seems to need one, the demo
   is rethought.

## Scope / non-goals

**In scope (content + include-JS only):**

- A new corpus exhibit `corpus/graphics3d/` (five pages) — the pinned regression
  artifact and the full "Gallery" exhibit.
- Additive, backward-compatible extensions to the `makeScene3D` include
  (`three-scene.tmd`): a controls-toolbar slot, a `loadGLTF(url)` helper with
  auto-framing, and a `rebuild(group)` convention. Applied to the exhibit's copy
  and mirrored into `site/_includes/three-scene.tmd`.
- Surfacing on the marketing site: mount the exhibit at `/gallery/graphics3d`, a
  Gallery card, and three headline demos on `site/showcase.tmd`.
- A compact "build an interactive 3D scene" how-to in
  `docs/guide/using/interactive.tmd`.
- One vendored engineering `.glb` (clean-license) under `corpus/graphics3d/assets/`
  and copied to the site.

**Explicitly out of scope:**

- The full OpenSCAD/CadQuery code-block CAD feature (a separate render subsystem,
  previously evaluated as low-demand). The parametric gear delivers the
  "computed geometry" story without it.
- A general molecule-format parser. The molecule set is curated.
- Any Rust/crate change, any new output format.
- The author's personal tech-blog (`corpus/tech-blog/`). Its `three-scene.tmd`
  copy is left untouched. (A Lorenz blog post there is a possible later follow-up,
  not part of this work.)
- Reconciling all three `three-scene.tmd` copies. The duplication is pre-existing;
  this work only touches the two copies it uses.

## Architecture

`crates/core/tests/corpus.rs` renders every doc under `corpus/` and enforces the
block-model invariants (each emitted block carries `data-block-id` +
`data-sourcepos`). So the three-plus-one visualizations live as **one corpus
exhibit**, which auto-pins them, and that same exhibit is the marketing Gallery
target.

```
corpus/graphics3d/
  _site.yml                 flat native website config; title "Live 3D graphics"
  index.tmd                 exhibit landing: links the four demo pages
  molecules.tmd             curated molecule picker
  lorenz.tmd                animated Lorenz attractor
  cad.tmd                   engineering-model viewer + live parametric gear
  sorting.tmd               2D canvas sorting visualizer
  _includes/
    three-scene.tmd         extended makeScene3D (molecules/lorenz/cad only)
  assets/
    <engineering-model>.glb vendored, clean-license (see "Licensing")
```

Surfacing (reusing existing mechanisms):

- **Marketing site** (`site/_site.yml`): add
  `mounts: gallery/graphics3d: ../corpus/graphics3d` so `/gallery/graphics3d`
  resolves in `preview`; the static `build` wires it with its own
  `build ... --out` step like the other gallery exhibits (course/tarn/descent).
- **Gallery** (`site/gallery.tmd`): a card linking to the exhibit.
- **Showcase** (`site/showcase.tmd`): three headline demos — molecules, sorting,
  Lorenz — each in a `::: {.panel-tabset}` with `### Result` (the live `{js}` cell)
  and `### Source` (the same code shown). CAD is not lifted to the showcase (its
  `.glb` load is heavier); it lives in the full exhibit.
- **Docs** (`docs/guide/using/interactive.tmd`): a short instructional example of
  the 3D-scene pattern (not a copy of the showcase), noting the `{python}`-cell
  variant.

**Kernel independence:** every interactive piece is real instrumented
**JavaScript** (`{js}` cells) — molecule coordinates, Lorenz integration, gear
geometry, and sort traces. The exhibit and site therefore build with **zero
Python/R kernel dependency**, and the "Source" tab always shows genuine code. The
docs how-to notes that a `{python}` cell computing the data + a `{js}` cell
rendering it (the existing PCA-post pattern) works identically for authors who
prefer it.

## `makeScene3D` helper extensions (additive, backward-compatible)

The current helper (`_includes/three-scene.tmd`) already provides: OrbitControls
(spin/zoom with damping), a fullscreen button, a `spriteLabel` helper, an
`onFrame` hook, and cleanup via `invalidation`. Existing callers (PCA post,
showcase) pass a `buildScene` callback and options. All additions are new optional
inputs, so those callers are unaffected.

1. **Controls toolbar slot.** A new `controls` option: an array describing
   `<select>` and range-slider widgets rendered into a toolbar in the returned
   container (used by the molecule picker and the gear sliders). Each control
   reports changes to a callback.
2. **`loadGLTF(url)`.** Exposed to `buildScene` (alongside `O`, `spriteLabel`,
   `ctrl`). Imports `GLTFLoader` from `https://esm.sh/three@0.163.0/examples/jsm/loaders/GLTFLoader`
   (same pinned three version as the helper), loads the model, and **auto-frames**
   the camera to the model's bounding box (fit distance from bounds + fov) so any
   model shows up correctly sized and centered.
3. **`rebuild(group)` convention.** A helper for picker/slider demos to clear a
   dedicated `THREE.Group`'s children and repopulate it without tearing down the
   renderer/controls (so camera state is preserved across a molecule switch or a
   gear-parameter change).

The two copies kept in sync: `corpus/graphics3d/_includes/three-scene.tmd` and
`site/_includes/three-scene.tmd`. (`corpus/_includes/` and
`corpus/tech-blog/_includes/` are left as-is.)

## Visualization specs

### 1. Molecules (`molecules.tmd`)

- **Content:** a picker over five curated molecules — water (3 atoms), benzene
  (12), caffeine (24), an idealized **B-form DNA double helix** (procedurally
  generated backbone + base-pair rungs, clearly labeled *schematic* so we're not
  claiming a real PDB structure), and a **C60 buckminsterfullerene** (60 atoms,
  truncated-icosahedron coordinates) as the symmetric showpiece.
- **Rendering:** CPK-colored spheres for atoms + cylinder bonds, an ambient +
  directional light, a small element-color legend, spin + zoom + fullscreen (all
  from the helper). Switching molecules uses `rebuild(group)`.
- **Data:** small hardcoded coordinate/bond tables in the `{js}` cell for the
  first four; C60 and DNA generated procedurally. No external fetch.

### 2. Lorenz attractor (`lorenz.tmd`)

- **Content:** the classic butterfly, integrated live (RK4 or fine Euler) in `{js}`
  and drawn as a `THREE.Line`/tube that **grows over time** via the existing
  `onFrame` hook. σ/ρ/β exposed as sliders (via the controls slot); moving them
  re-integrates and redraws, so the reader sees how the parameters reshape the
  attractor.
- **Why 3D:** the folded manifold and parameter sensitivity only read in 3D +
  motion — this is the "2D can't tell the story" piece.
- Spin + fullscreen from the helper.

### 3. CAD (`cad.tmd`)

Two paired panels telling the "compute → render, one live doc" story:

- **Viewer:** a real engineering-assembly `.glb`, vendored under
  `assets/`, loaded via `loadGLTF` and auto-framed; spin + fullscreen. Narrative:
  "a real CAD model, rendered live in the page."
- **Computed part:** the document computes an **involute spur gear** from
  `module`, `teeth`, and `pressure angle` (involute tooth profile → extruded 3D
  gear) and **regenerates the mesh live** as the reader drags sliders (controls
  slot + `rebuild`). Narrative: "that one was *loaded*; this one the page just
  *calculated* — change the numbers, watch it regenerate." Thematically pairs with
  the gearbox/engine.

### 4. Sorting (`sorting.tmd`)

- **Style:** 2D **Canvas 2D** rainbow bars (HSL by value) — the readable, iconic,
  "satisfying" format. Not three.js.
- **Data source — real instrumented algorithms:** each sort is a real JS function
  that pushes operations to a trace as it runs — a uniform op stream of
  `compare(i,j)`, `swap(i,j)`, and `overwrite(i,value)` (the last covers merge /
  radix, which write rather than swap in place). The animator replays that true
  trace; the "Source" tab shows the actual instrumented algorithm.
- **Algorithms (7):** bubble, insertion, selection, quicksort, merge, heap, radix
  (LSD).
- **Controls:** algorithm picker, array size, speed (ops/frame), shuffle/restart,
  and an **optional off-by-default "sound of sorting"** toggle (Web Audio maps the
  touched value to a tone on each compare/write; no autoplay — click to enable).
- **Highlighting:** compared indices and the current write are colored distinctly
  each frame; a comparison/array-access counter is shown.

## Assets, offline, and licensing

- The engineering `.glb` is **vendored** into `corpus/graphics3d/assets/` and
  copied into the site build, so it is served from our own origin (no runtime CDN
  fetch for the model) and the portable-folder build copies it (dogfooding that
  feature). three.js + GLTFLoader still import from esm.sh, consistent with every
  existing 3D demo.
- **Test safety:** `{js}` cells run client-side only, so `cargo test` never fetches
  the model or three.js — no CI network dependency. `body_html_snapshots.rs` will
  need regenerating for the `showcase.tmd` / `interactive.tmd` edits (expected).
- **Licensing (resolved at implementation, before shipping):** the iconic Khronos
  engineering models (`2CylinderEngine`, `GearboxAssy`) look ideal but have murky
  provenance (legacy JT→COLLADA conversions with no clean per-model license line),
  so they are **not** shipped without confirmation. Rule: pin an engineering model
  whose license is unambiguous and record attribution in the page +
  `THIRD_PARTY.md`. Priority order: (a) confirm a Khronos engineering model is
  permissively licensed; else (b) a **CC0** engineering/mechanical model
  (NASA/Smithsonian open-access, or a CC0 glTF), converted to `.glb` if needed.
  The confirmed pick is brought back at implementation time.

## Testing / verification

- **`cargo test -p taliesin-core`** — `corpus.rs` renders the new exhibit and
  enforces the block-id/sourcepos invariants on it.
- **Regenerate `body_html_snapshots.rs`** for the `showcase.tmd` /
  `interactive.tmd` edits; review the diff.
- **`cargo test`** (server + core) green before any commit; the tree stays
  `cargo fmt`-clean (PostToolUse hook).
- **Type-check the include JS** where practical (the `{js}` lives in corpus/site
  includes, not the bundled `crates/core/assets/js`, so the strict assets `tsc`
  gate does not apply; keep the include JS clean regardless).
- **Browser verification via chrome-devtools MCP** on the built pages: molecule
  spin + picker + fullscreen; Lorenz animation + sliders; CAD model load +
  auto-frame + gear regeneration; sorting playback across all seven algorithms +
  sound toggle. Verified at **three viewports** (mobile ~390×844, laptop landscape
  ~1440×900, laptop portrait ~900×1440) per the project's UI-testing matrix.

## Open items (decided at implementation)

- Final engineering-model pick + its exact license/attribution (per rule above).
- Whether the DNA helix or C60 leads the molecule picker's default selection.
- Exact showcase copy for each headline demo's "this is ~N lines of plain text"
  reveal.

## Build sequence (for the implementation plan)

1. Extend `makeScene3D` in a new `corpus/graphics3d/_includes/three-scene.tmd`
   (controls slot, `loadGLTF`, `rebuild`); mirror into `site/_includes/`.
2. Author the exhibit: `_site.yml`, `index.tmd`, then `molecules.tmd`,
   `lorenz.tmd`, `sorting.tmd`, `cad.tmd` (CAD last — it needs the vendored asset).
3. Vendor + license the `.glb`; add attribution to the page + `THIRD_PARTY.md`.
4. Mount + surface on the site: `_site.yml` mount, `gallery.tmd` card,
   `showcase.tmd` three headliners.
5. Docs how-to in `docs/guide/using/interactive.tmd`.
6. Regenerate snapshots; run full test suite; browser-verify at three viewports.
