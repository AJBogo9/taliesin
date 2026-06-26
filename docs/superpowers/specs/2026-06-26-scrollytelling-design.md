# Scrollytelling `:::{.scrolly}` (design)

Date: 2026-06-26
Status: approved (brainstorm), pre-implementation
Feature branch: `feat/scrollytelling`
Pillar: BEYOND-QUARTO.md Pillar III (new web-native capability) + FEATURE-IDEAS.md #46.

## Summary

A `:::{.scrolly}` fenced div that pins a **sticky visual stage** beside a scrolling column
of `.step` narration divs. As the reader scrolls, the active step drives the stage —
generalizing the already-shipped `.code-walkthrough` machine (sticky panel +
IntersectionObserver + active-step contract) from "focus code lines" to "drive any visual".

The active step feeds the **already-shipped reactive graph**: with `name="scene"`, the
`.scrolly` drives a hidden `data-qmd-input` whose value is the active step's `state=`, so a
sticky `{js}` cell reacts via `//| input: scene` (`qmd.value("scene")`) with zero boilerplate
and **no new runtime code**. It also sets `data-scrolly-state` on the root for pure-CSS layer
swaps. This makes scrollytelling, conceptually, *a reactive input driven by scroll position
instead of a slider* — coherent with the `{input}` controls shipped the same day.

## Goals

- `:::{.scrolly}` lays out a sticky stage (first non-`.step` block) + a scrolling `.step`
  column, responsive (single column on mobile, stage pinned on top).
- The active step is the last `.step` crossing the viewport activation band (reusing the
  walkthrough IO logic); the first step is active before any scroll.
- With `name=`, the active step's `state=` becomes a reactive value: a sticky `{js}` cell
  with `//| input: <name>` re-runs (transitively) as the reader scrolls.
- Always set `data-scrolly-state` on the root so pure-CSS scrollytelling (swap stacked
  layers/figures) needs no JS.
- Located diagnostics when a `.scrolly` has no stage block or no `.step` divs.
- Respect `prefers-reduced-motion`.

## Non-goals (v1, YAGNI)

- **No new reactive runtime API / no `qmd-js.js` change.** Reuse the shipped `{input}`
  registration (`[data-qmd-input]` scan → `registerInput` → `scheduleFrom`) by emitting a
  hidden input the enhancer drives.
- **No bespoke `qmd-scrolly` event.** The reactive value + the CSS state attribute cover
  the two real modes; a third event vocabulary is redundant.
- **No explicit `:::{.sticky}` requirement.** The stage is the first non-`.step` block.
- **No per-step transitions/tweening engine, parallax, or horizontal scroll.** Discrete
  step states only (the chart rebuilds per step, like an `{input}` change).
- **No refactor of the shipped `walkthrough.js`** into a shared core (some IO logic is
  duplicated in `scrolly.js`; acceptable to avoid regressing walkthrough).

## Invariants honoured

- **HTML-only**, **offline** (no new dep; reuses bundled d3/Plot when the stage is a `{js}`
  cell), **deck-skipped** (decks have their own chrome; the enhancer no-ops without a
  `.scrolly`).
- **Single editing surface:** scrolling is reader interaction with the read-only rendered
  view; the preview never writes the `.qmd`.
- **Block model untouched:** the `.scrolly` div is one container block with the usual
  `data-block-id` + `data-sourcepos`; inner blocks (stage, steps) keep their own ids via
  grouping. No diff/numbering/sourcepos change.
- **Rides supported seams:** a `build_container` arm + a `qmdEnhancers`-registered enhancer,
  reusing the shipped `{input}` graph. Do-NOT-touch machinery (`cite.rs`, `includes.rs`,
  numbering, exec/freeze/kernel, the `:::` three-pass scanner itself) untouched.

## Authoring syntax

```
::: {.scrolly name="scene"}
```{js}
//| input: scene
return drawChart(qmd.value("scene"));    // sticky stage = first non-.step block
```
::: {.step state="trend"}
First, the overall upward trend.
:::
::: {.step state="spike"}
Now zoom into the 2020 spike.
:::
:::
```

- `.scrolly` attrs: `name` (optional — the reactive node; omit for CSS-only).
- The **stage** is every inner block that is not a `.step` (a `{js}` cell, a figure, an
  image, raw HTML…), concatenated in document order. The **narration** is the `.step` divs;
  each carries `state=` (the value when active). Put intro prose outside the `.scrolly` div.

## Server (`divs.rs`)

A new arm, after the `.code-walkthrough`/`.step`/`.panel-tabset` arms, before the final `else`:

- Partition `inner` into `steps` (blocks whose html starts with `<div class="step"`) and
  `stage` = the concatenation, in document order, of **all** non-`.step` inner blocks (one
  `{js}` cell is the common case; multiple non-step blocks simply stack in the sticky stage).
  Authors put any intro prose *outside* the `.scrolly` div.
- `validate_scrolly(has_stage, has_steps, open_line, file)` pushes located warnings.
- Emit:

```html
<div class="qmd-scrolly"{data} data-scrolly-name="scene">
  <input type="hidden" class="qmd-scrolly-input" data-qmd-input="scene" value="trend">
  <div class="scrolly-steps">{steps}</div>
  <div class="scrolly-stage">{stage}</div>
</div>
```

The hidden input + `data-scrolly-name` are emitted only when `name=` is present; its initial
`value` is the first step's `state` (so consumer cells read a sane value before any scroll).
`name`/`state`/data attributes are escaped (`escape_attr`).

The **`.step` arm is extended**: in addition to `lines=` → `data-cw-lines`, read `state=` →
`data-state` (both may be present; walkthrough uses lines, scrolly uses state).

## Client (`scrolly.js`)

A new enhancer bundled in `code_scripts()` alongside `walkthrough.js`/`tabset.js`; registers
through `qmdEnhancers`, no-ops without a `.scrolly`, idempotent (`data-scrolly-init`).

- For each `.qmd-scrolly`: collect `.scrolly-steps .step`; an IntersectionObserver with
  `rootMargin: '-45% 0px -45% 0px'` tracks which steps straddle the band; the **last** wins
  (before any cross, the first step is active) — identical to `walkthrough.js`.
- `apply(i)`: set `root.dataset.scrollyState = steps[i].dataset.state || ''`; if a
  `.qmd-scrolly-input` exists and its value differs, set `input.value = state` and
  `input.dispatchEvent(new Event('input', { bubbles: true }))`.
- Initial `apply(0)` sets the state attribute but does **not** dispatch (the hidden input's
  server-rendered `value` already matches step 0, and the reactive runtime ran the cell once
  on mount) — avoids a redundant re-run.

The reactive re-run path is entirely the shipped one: `qmd-js.js`'s `enhance` already
registers `[data-qmd-input]` (the hidden input) before cells run; the dispatched `input`
event fires `registerInput`'s listener → `scheduleFrom` → the transitive-downstream closure.

## CSS (`base.css`)

Mirror the `.code-walkthrough` grid:

- `.qmd-scrolly { display: grid; grid-template-columns: 1fr minmax(18rem, .9fr); gap: … }`
  (steps left, stage right).
- `.scrolly-steps .step { min-height: ~60vh; centered; dimmed }`,
  `.scrolly-steps .step.scrolly-step-active { opacity: 1; accent border }`.
- `.scrolly-stage { position: sticky; top: 0; align-self: start; height: 100vh; display:flex;
  align-items:center }`.
- Mobile (`<73rem`): single column, `.scrolly-stage` `order: -1`, pinned top, reduced height.
- `@media (prefers-reduced-motion: no-preference)` for the step opacity transition only.
- The active-step class is `scrolly-step-active` (NOT the walkthrough's `cw-step-active`).

## Validation (`validate.rs`)

```rust
pub(crate) fn validate_scrolly(
    has_stage: bool, has_steps: bool, line: usize, file: Option<String>,
) -> Vec<Warning>
```
Warns: no stage block ("`.scrolly` has no sticky stage (add a figure or `{js}` cell)"); no
`.step` divs ("`.scrolly` has no `.step` divs to scroll through"). Located; the div still
renders. Mirrors `validate_walkthrough`.

## Corpus pin

`corpus/explorable/scrolly.qmd`: a `:::{.scrolly name="scene"}` whose stage is a `{js}` cell
drawing an Observable Plot that changes appearance by `qmd.value("scene")`, with 3 `.step`s
(`state="a"|"b"|"c"`). A short intro paragraph above documents the feature. (`corpus/explorable/`
is a new corpus subdir; the corpus walk picks it up automatically.)

## Testing strategy

1. **Rust render test** (`render/tests.rs`): the `.scrolly` arm emits `.qmd-scrolly`, the
   `scrolly-steps`/`scrolly-stage` split, the hidden `data-qmd-input` with the first step's
   value (when `name=`), and the `.step` `data-state`. `validate_scrolly` unit tests
   (no-stage, no-steps).
2. **Corpus invariants** (auto): the pin doc renders; unique block ids; valid sourcepos.
3. **Browser (chrome-devtools MCP)** against the live preview: `scrollIntoView` the 2nd/3rd
   `.step` → `data-scrolly-state` flips and the sticky `{js}` cell re-runs reading the new
   `qmd.value` (assert the rendered stage changed); first step active on load; 0 console
   errors. Fallback if headless scroll is unreliable: directly set the hidden input value +
   dispatch `input` to prove the reactive wiring, and assert the IO observes the steps.

## Risks & mitigations

- **Headless scroll flakiness** (seen earlier: `window.scrollTo` no-op in the harness):
  verify via `scrollIntoView` (worked for read-aloud autoscroll) + the direct-dispatch
  fallback.
- **Double initial run**: `apply(0)` doesn't dispatch (state matches the server-rendered
  hidden value), so the cell runs once on mount, not twice.
- **Stage detection fragility** (string match on inner html): mirror walkthrough's existing
  `b.html.contains("<pre")` approach; match `<div class="step"` for steps, first remainder is
  the stage. Stable because the `.step` arm emits a known prefix.
- **Live-swap**: a re-emitted `.scrolly` re-inits (`:not([data-scrolly-init])`); its IO is
  GC'd with the old subtree (same as walkthrough). The hidden input re-registers via the
  shipped `{input}` `:not([data-qmd-input-bound])` guard.
- **`name` collision** with another input/viewof of the same name → last writer wins (author
  error, acceptable).

## Out of scope follow-ups (recorded, not built)

- Smooth per-step tweening / parallax / horizontal "scrollygraph".
- Factoring `walkthrough.js` + `scrolly.js` onto a shared active-step core.
- A `progress` value (0–1 within a step) in addition to the discrete `state`.
- Pure-CSS layer-swap helper classes (authors write their own `[data-scrolly-state]` CSS).
