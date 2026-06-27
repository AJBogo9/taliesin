# Marketing showcase: live, self-referential capability demos

- **Date:** 2026-06-27
- **Status:** approved design, pending spec review
- **Topic:** add genuinely-flashy, genuinely-real interactive demos to the qmd-fast marketing site

## Goal

The marketing site (`site/`) today shows qmd-fast's most compelling claims (the
live edit loop, code-runs-in-place) as **`.mp4` screencasts**. A visitor cannot
touch them. This work replaces passive video with **live, interactive demos that
run client-side in the static page**, so the page itself is the proof.

Guiding constraints, set by the author:

1. **Realistic, not faked.** Every demo is a capability a real author could write.
   The source shown beside each result is the *actual* source that produced it. No
   smoke and mirrors, no bespoke engine tricks unavailable to users.
2. **Flashy by selection.** We choose the subset of real capabilities that are
   inherently impressive (interactive 3D, reactive graphics, scrollytelling), and
   present them well.
3. **Self-referential.** Each demo pairs the result with its real `.qmd`/`{js}`
   source ("this jaw-dropping thing is ~30 lines of plain text"). The reveal uses
   the framework's own `.panel-tabset` (a `Result` / `Source` tab pair), so the
   reveal mechanism is itself a feature on display.

This is **content work only**: authoring `.qmd` documents with existing features.
It must not require any change to the Rust crates or the bundled JS enhancers. If a
demo seems to need an engine change, it is out of scope and the demo is rethought.

## Where it lives

- **New page `site/showcase.qmd`** ("See it live"), an ordinary qmd-fast website
  page, added to `site/_site.yml` `nav.left` (after "Document types") and linked
  from the index hero's primary actions.
- **A live interactive 3D hero on `site/index.qmd`**: the reactive-3D demo (below)
  is placed near the top of the landing page so the site wows in the first
  screenful, replacing the lead screencast as the headline visual. The two existing
  `{{< video >}}` screencasts may stay lower on the page as a quieter "here it is in
  an editor" beat, or be removed; implementation decides based on flow, but the
  *live* 3D piece leads.
- **No new output formats, no faked in-browser editor** (the author chose
  self-referential spectacle over a simulated edit loop).

## The self-referential reveal

Each demo is wrapped in a `::: {.panel-tabset}` with two tabs:

- **`## Result`** holds the live demo (the executing `{js}` cell, the `{{< input >}}`
  controls, the `.scrolly`, etc.).
- **`## Source`** holds the *same* code in a **non-executing** fenced block
  (` ```js ` / ` ```markdown `), so the visitor reads exactly what produced the
  result. The two must be kept in sync by the author (the Source tab is a copy of
  the Result's cell body); a short note on the page states this is the real source.

Model the tabset structure on `corpus/layout/panels.qmd`.

## Demos

### 1. Reactive 3D hero (flagship; also on the index)

- **Visitor sees:** a polished Three.js scene they can orbit and zoom (pointer +
  touch), gently auto-rotating, transparent renderer so it follows the page's
  light/dark theme, with a fullscreen button. Two or three `{{< input >}}` sliders
  (e.g. *detail*, *twist*, *amplitude*) reshape the geometry live.
- **Real features:** a `{js}` cell that `await import()`s Three.js + `OrbitControls`
  from `esm.sh` (exactly as `corpus/_includes/three-scene.qmd` and the globe in
  `docs/guide/using/code.qmd` already do), reading `qmd.value("detail")` etc. and
  re-running on `//| input: detail, twist, amplitude` (the reactive graph).
- **Subject:** a tasteful parametric surface (a morphing wave-interference mesh or a
  supershape/torus-knot), not a gimmick. Must look intentional and calm, not noisy.
- **Source revealed:** the ~30-line `{js}` cell + the `{{< input >}}` lines.

### 2. Scrollytelling story

- **Visitor sees:** a sticky visual stage (canvas or a light Three.js scene) that
  transforms as they scroll through 3-4 short narrated steps telling the pitch:
  *one source → top-level blocks → a save diffs only what changed → the same blocks
  drive a post, a deck, and a book.*
- **Real feature:** `::: {.scrolly name="stage"}` with a stage block and `.step`
  divs carrying `state="..."`; the stage is a `{js}` cell that reads the active
  step (via the reactive `name` / `qmd.value("stage")`) and animates between states.
  Model on `corpus/explorable/scrolly.qmd`.
- **Source revealed:** the `.scrolly` markdown skeleton + the stage `{js}` cell.

### 3. Reactive explorer (data-viz + offline math)

- **Visitor sees:** sliders driving a live **Observable Plot** chart *and* a
  **KaTeX** equation that updates with the same inputs, e.g. a Fourier square-wave
  synthesis (add odd harmonics, watch the wave sharpen) or a distribution explorer.
- **Real features:** `{{< input >}}` + a `{js}` cell using the vendored `Plot`/`d3`
  globals (no Observable runtime), plus server-side KaTeX `$...$`. Model on
  `corpus/reactive/inputs.qmd` and the Fourier reactive in
  `docs/guide/using/code.qmd`.
- **Source revealed:** the inputs + the `{js}` plotting cell.

### 4. Code that moves (page-level)

- **Visitor sees:** a sticky code panel whose highlighted lines change as they
  scroll through short prose steps, walking through a real qmd-fast snippet.
- **Real feature:** `::: {.code-walkthrough}` with a lead code block and `.step`
  divs carrying `lines="1-5,8"` focus specs (page-level enhancer `walkthrough.js`).
  Model on `corpus/narrate/walkthrough.qmd`.
- **Honesty note:** `.magic-move` (the FLIP code-morph) animates **only inside the
  deck engine** (`deck.js`), not on a standalone page, so it is **not** used here.
  Magic-move and code line-stepping are instead shown in the embedded live deck
  (#5), where they genuinely animate.

### 5. The embedded live deck (keep + frame)

- Keep the existing `{{< embed demo.qmd >}}`. Optionally add one **magic-move**
  slide and one **code-stepping** slide to `site/demo.qmd` so the deck demonstrates
  those deck-only flashy features in their real home. Framed with a line that the
  deck is the running engine, not a screenshot.

## Cross-cutting constraints (baked into every demo)

- **Reduced motion:** honor `prefers-reduced-motion: reduce`. Auto-rotation and
  scroll-driven animation pause/settle to a static, legible state.
- **Mobile:** usable and legible down to 390px wide. The 3D canvas scales to
  container width; sliders are reachable; tabsets stack. Verified at the three
  viewport sizes (390x844, 1440x900, 900x1440).
- **Performance:** cap `devicePixelRatio` (<= 2), dispose WebGL contexts /
  `cancelAnimationFrame` on the cell's `invalidation` promise (as the corpus globe
  does), and **lazy-init heavy scenes** with an `IntersectionObserver` so a scene
  starts only when scrolled near. The page must not jank or ship a runaway loop.
- **Theme-aware:** transparent renderers and CSS-variable colors so demos follow the
  light/dark toggle without re-running (3D scenes read `--qmd-*` / theme on the
  `qmd:themechange` event where needed).
- **Dependency honesty:** Three.js is fetched from `esm.sh` at view time (the same
  CDN dependency the corpus already accepts). Plot/d3 are vendored offline by the
  framework. No new vendored assets unless a small `showcase.css` for layout.
- **Keep the tree green:** `qmd-fast check site` stays clean (valid links/anchors,
  valid `{{< input >}}` types, no dangling reactive inputs); the static
  `qmd-fast build site` succeeds.

## Files touched

- `site/showcase.qmd` (new) — the showcase page.
- `site/index.qmd` — add the live 3D hero near the top.
- `site/_site.yml` — add "See it live" to nav.
- `site/demo.qmd` — optionally add a magic-move + code-stepping slide.
- `site/showcase.css` or an additive block in the site CSS (new) — only the layout
  the demos need (tabset framing, demo card spacing). No engine CSS changes.
- No changes under `crates/` or `web-client/`.

## Verification plan

1. `qmd-fast check site` is clean.
2. `qmd-fast build site` (or preview) succeeds; the showcase page and the index hero
   render; the bundled enhancers (`qmd-js.js`, `scrolly.js`, `walkthrough.js`,
   `tabset.js`) ship because the page contains those constructs.
3. Browser verification (chrome-devtools MCP): each demo at 1440x900, 900x1440, and
   390x844; light and dark; with `prefers-reduced-motion` emulated. Console must be
   error-free. The 3D scene must orbit, the sliders must reshape it, the scrolly
   stage must track scroll, the explorer must update on input, the walkthrough must
   track scroll, and each `Source` tab must show the real code.
4. A performance sanity check: scrolling the full page stays smooth; no scene runs
   before it is scrolled into view; leaving the page disposes the contexts.

## Out of scope

- Any change to the Rust engine or bundled JS enhancers.
- A simulated/faked in-browser edit loop.
- New output formats, RSS, or other site features.
- Replacing the docs books' content (this is the marketing site only).

## Risks

- **esm.sh availability:** if the visitor is offline or the CDN is down, the 3D
  demos degrade. Mitigate with a graceful fallback (a static poster / message) in
  each 3D cell's error path, consistent with how the framework handles a failed
  import.
- **Low-end GPUs:** keep geometry modest; the DPR cap and lazy-init bound the cost.
- **Source/Result drift:** the `Source` tab is a hand-kept copy of the cell body; a
  short author note acknowledges this, and verification confirms they match at build
  time.
