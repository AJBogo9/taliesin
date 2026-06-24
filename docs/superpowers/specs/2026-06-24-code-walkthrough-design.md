# Design: `narrated-code-walkthrough` (Wave 3 feature)

Status: approved 2026-06-24. Branch `feat/code-walkthrough`. Roadmap:
`BEYOND-QUARTO.md` Pillar III. A genuinely-past-Quarto capability: a sticky code
panel + scroll-driven line-range highlighting, unifying what Quarto splits across
code-annotation and magic-move. Read-only-additive; rides the supported `:::` +
enhancer seams; touches none of the Do-NOT-touch machinery.

## Decisions (from brainstorming)

1. **Core interaction:** fixed code, moving highlight (scrollytelling). One code block
   stays pinned; each prose step focuses a different line range (dim the rest, accent
   the focused lines). Morphing/multiple code states is OUT (deferred).
2. **Authoring syntax:** nested `.step` divs each with a `lines=` attribute. The
   container's first code block is the sticky panel; each `.step` is a scroll trigger.
   `lines` uses the existing `parseLineSpec` syntax (`"3-5,7"`). `lines` is optional: a
   step with no `lines` (or `lines="all"`) clears the focus (full code undimmed).
3. **Layout:** prose left (scrolls), code sticky right on wide screens; collapses to a
   single column on narrow screens (code sticks to the top, prose flows beneath).

```markdown
::: {.code-walkthrough}
``​`python
def em_step(x, theta):
    e = expectation(x, theta)
    m = maximize(e)
    return m
``​`

::: {.step lines="1"}
The EM step takes the data and current parameters.
:::

::: {.step lines="2"}
**E-step:** compute responsibilities under the current model.
:::

::: {.step lines="3-4"}
**M-step:** re-estimate parameters and return them.
:::
:::
```

## Server-side rendering (`crates/core/src/render/divs.rs`)

Two new arms in `build_container`, before the generic `else` fallback. Because nested
`:::` divs group **inner-first**, each `.step` is already an emitted HTML string by the
time the `.code-walkthrough` arm runs, and the generic arm would DROP its `lines=`
attribute. So:

- **`.step` arm:** emits `<div class="step"{id_attr}{data} data-cw-lines="<spec>">{body}</div>`
  reading `lines` off `attrs.get("lines")` (omitted/empty → no `data-cw-lines`). Keeps
  the step's own `data-block-id`/`data-sourcepos` so inner prose blocks stay locatable.
- **`.code-walkthrough` arm:** splits `inner` into the first code block (the panel) vs
  everything else (the steps column, in order). Runs `emit::wrap_pre_lines()` on the
  panel so its lines become `.qhl-ln` spans (idempotent; the same call magic-move uses;
  author does NOT need `code-line-numbers`). Emits:
  ```html
  <div class="code-walkthrough"{data}>
    <div class="cw-steps">{steps_html}</div>
    <div class="cw-stage"><div class="cw-code">{panel_html}</div></div>
  </div>
  ```
  Panel detection: first inner block whose html contains `<pre` and `<code`. If none,
  emit a located warning (see Validation) and fall back to wrapping all inner blocks in
  `.cw-steps` (renders, just without a stage).

DOM/source order: steps before stage, so the source reads prose-then-code and
click-to-source order is natural; CSS grid places the stage on the right.

## Validation (`crates/core/src/render/validate.rs`)

`validate_walkthrough(has_code, line, file) -> Option<Warning>`: warns (click-to-source,
via the located `Warning` channel from Wave 1) when a `.code-walkthrough` contains no
code block. Pinned by a case in `corpus/diagnostics/` is out of scope for v1 (the corpus
`every_corpus_doc_emits_no_unknown_key_warnings` test would otherwise need the doc clean);
the warning is unit-tested in `validate.rs` instead. (Class-name typo validation, e.g.
`.stop` for `.step`, is NOT a current validation surface and is left for a later
div-class-vocabulary item.)

## Client-side enhancer (`crates/core/assets/js/walkthrough.js`, new)

~70 lines, registered via `window.qmdEnhancers.register(...)`, bundled by an
`include_str!` const in `mod.rs` appended to `code_scripts()` (ships unconditionally,
no-ops without a `.code-walkthrough`, like mermaid/qmd-js).

- Idempotent: guard each container with `data-cw-init`.
- One `IntersectionObserver` per container over its `.step` elements, `rootMargin`
  defining an activation band near viewport center (`-45% 0px -45% 0px`, threshold 0).
  The last step intersecting the band is active; before the first step crosses, the
  first step is active (panel never starts blank).
- On active change: read the active step's `data-cw-lines`, toggle `.qhl-ln-hl` on the
  matching 1-indexed `.qhl-ln` spans + `.qhl-lines-active` on the panel `<pre>`. The
  ~8-line `parseLineSpec` is reimplemented locally (deck.js is NOT loaded on pages); the
  shared contract is the CSS class names, so `deck.js`/`deck.css` stay untouched.
- After an incremental block swap, `afterChange()` re-runs enhancers; the replaced
  subtree (and its observer) is GC'd and the fresh container re-initializes. Read-only.

## CSS (`crates/core/assets/css/base.css`, always loaded, near `.callout`)

- Grid `grid-template-columns: 1fr minmax(18rem, .85fr)`, gap; `.cw-stage` is
  `position: sticky; top: …`, vertically centered.
- Highlight, scoped under `.code-walkthrough`: `pre.qhl-lines-active .qhl-ln { opacity:.35 }`,
  `.qhl-ln-hl { opacity:1; background: color-mix(in srgb, var(--qmd-accent) 14%, transparent);
  box-shadow: inset .18em 0 0 var(--qmd-accent) }`. Theme-aware via `--qmd-*` tokens.
- Transitions guarded by `@media (prefers-reduced-motion: no-preference)`.
- `@media (max-width: 60rem)`: single column, `.cw-stage` sticks to the top.

## Corpus pin + tests

- **Pin doc:** `corpus/narrate/walkthrough.qmd` (a genuine short walkthrough; auto-found
  by the corpus walker). Add a `corpus/README.md` row.
- **Unit test** (`render/tests.rs`): assert the emitted contract — `.code-walkthrough`
  wrapper carries `data-block-id`+`data-sourcepos`; panel `<pre>` has `.qhl-ln` spans;
  each `.step` carries `data-cw-lines` and keeps its own `data-block-id`/`data-sourcepos`.
- **Validation unit test** (`validate.rs`): the no-code-block warning fires/located,
  and is silent when a code block is present.
- Corpus invariants (`corpus.rs`) stay green automatically (every block has id+sourcepos).

## Invariants preserved

Read-only-additive. No change to the `:::` scanner, the block diff, sourcepos, exec/
freeze/kernel, citations, includes, numbering, or click-to-source. Inner blocks keep
ids/sourcepos via `group_divs`. The enhancer never writes back (single editing surface).
`deck.js`/`deck.css` untouched. HTML-only intact.

## Out of scope (YAGNI)

Morphing/multiple code states; per-step code edits; auto step numbering beyond a CSS
counter; any `data-line` attribute scheme (ordinal `.qhl-ln` indexing suffices).
