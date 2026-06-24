# Design: `panel-tabset-margin` (Wave 3 feature)

Status: approved 2026-06-24. Branch `feat/panel-tabset-margin`. Roadmap:
`BEYOND-QUARTO.md` Pillar IV (breadth). Two web-native layout capabilities, each
read-only-additive, riding the supported `:::` + enhancer seams. Closes a concrete
Quarto breadth gap; the moat applies (an open tab + a running `{js}` widget inside it
survive an edit elsewhere — Quarto's reload can't).

## Shape note (vs the roadmap)

The margin-rail half collapses to a **one-line CSS alias**: base.css already has a
`.sidenote`/`.marginnote` mechanism (float-right into the margin, `<73rem` inline
fallback), and the generic div arm already emits `<div class="column-margin">`. So
`.column-margin`/`.aside` need **no Rust** — just extend the existing selector. The
real new work is the **tabset**.

## Decisions (from brainstorming, approved)

1. Tabs split on headings at the **shallowest heading level present** (a `##` delimits
   tabs; a `###` inside a tab stays in that panel).
2. Tab label = heading text; the heading is **not** re-emitted as `<hN>` (no TOC
   pollution; ARIA `<button role="tab">` is the correct element anyway).
3. Blocks before the first heading render as a normal intro **above** the tab strip.
4. Tab/panel ids derive from the tabset's own block id (`{id}-t{i}`/`{id}-p{i}`) — unique
   and stable, no dependence on heading slugs.
5. `.column-margin`/`.aside` alias the existing `.sidenote` mechanism (don't reinvent).

## Authoring contract

```markdown
::: {.panel-tabset}
## Python
```python
print("hi")
```
## R
```r
print("hi")
```
:::

::: {.column-margin}
A note in the right margin on wide screens; inline on narrow ones.
:::
```

## Server-side (`crates/core/src/render/divs.rs`)

One new `.panel-tabset` arm before the generic `else`:
- Find the shallowest heading level L among `inner` (via `block_heading_level`). If no
  heading: `validate_tabset(false, ..)` warns; fall back to `<div class="panel-tabset">`
  + concat(inner).
- Partition `inner`: blocks before the first level-L heading → intro; each level-L
  heading starts a new tab whose body is the following blocks until the next level-L
  heading (deeper headings stay in the body). Label = `strip_tags(heading.html)`.
- Emit the ARIA structure (ids from the tabset block id):
  ```html
  <div class="panel-tabset"{data}>
    {intro}
    <div class="tabset-tablist" role="tablist">
      <button class="tabset-tab" role="tab" id="{id}-t0" aria-controls="{id}-p0"
              aria-selected="true" tabindex="0">Python</button>
      <button class="tabset-tab" role="tab" id="{id}-t1" aria-controls="{id}-p1"
              aria-selected="false" tabindex="-1">R</button>
    </div>
    <div class="tabset-panel" role="tabpanel" id="{id}-p0" aria-labelledby="{id}-t0">…</div>
    <div class="tabset-panel" role="tabpanel" id="{id}-p1" aria-labelledby="{id}-t1" hidden>…</div>
  </div>
  ```
  Labels are `html_escape`d. Inner blocks keep their own `data-block-id`/`data-sourcepos`
  via `concat`. Helpers (`is_heading`, `block_heading_level`, `strip_tags`) are reachable
  from divs.rs via `use super::*` (child sees parent privates; the callout arm already
  uses them).

`.column-margin`/`.aside`: no Rust change.

## Validation (`crates/core/src/render/validate.rs`)

`validate_tabset(has_tabs: bool, line, file) -> Option<Warning>` — located,
click-to-source warning when a `.panel-tabset` has no headings (it'd render with no tabs).

## Client-side enhancer (`crates/core/assets/js/tabset.js`, new)

~55 lines, registered via `qmdEnhancers`, idempotent (`data-tabset-init` guard), bundled
in `code_scripts()`. Standard ARIA tabs pattern: click selects a tab (toggle
`aria-selected`, panel `hidden`, roving `tabindex`); Left/Right/Home/End move focus +
select. Read-only; re-inits on `afterChange` after an incremental swap.

## CSS (`base.css`, near `.callout`)

- Tabset: flex `.tabset-tablist` with a bottom border; `.tabset-tab[role=tab]` underline-
  on-active using `--qmd-*` tokens; `[role=tabpanel][hidden]` hidden; `:focus-visible`
  ring; transition guarded by `prefers-reduced-motion`.
- Margin rail: add `.column-margin, .aside` to the existing `.sidenote, .marginnote`
  selector (float-right + `<73rem` inline fallback). One-line change.

## Corpus pin + tests

- Pin: `corpus/layout/panels.qmd` — a 2-3-tab tabset, one tab holding a labeled `@fig-`
  figure referenced from prose (proves cross-ref resolves through the tabset), plus a
  `.column-margin` note. README row added.
- Unit tests (`render/tests.rs`): tablist + N tabs + N panels; first panel visible, rest
  `hidden`; labels correct; no `<h` leak; inner blocks keep `data-block-id`; no-heading
  tabset falls back + warns. `validate.rs` warning test. Cross-ref test: the in-tab figure
  numbers + the `@fig-` ref links to it.
- Corpus invariants (`corpus.rs`) stay green automatically.

## Invariants preserved

Read-only-additive. No change to the `:::` scanner, diff, sourcepos, exec/freeze/kernel,
citations, includes, numbering, click-to-source. Inner blocks keep ids/sourcepos via
`group_divs`; tab switch toggles only `aria-*`/`hidden`. `deck.js`/`deck.css` untouched.
HTML-only intact.

## Out of scope (YAGNI)

URL-hash tab routing; nested tabsets (work incidentally, untested/unstyled); `.column-page`
/`.column-screen` full-bleed columns; a dedicated `<aside>` element.
