# `{input}` reactive controls (design)

Date: 2026-06-26
Status: implemented
Feature branch: `feat/reactive-input-controls`
Pillar: BEYOND-QUARTO.md Pillar III (new web-native capability) + FEATURE-IDEAS.md #47.

> **Amendment (during implementation): the authoring surface is a shortcode, not a `:::`
> div.** The design below specifies `::: {.input …}`, but a bodyless fenced div cannot work:
> `group_divs` drops empty divs (they have no block to anchor them), and emitting empty
> containers would require rewriting the Do-NOT-touch `:::` three-pass div machine. So the
> control ships as a **built-in shortcode `{{< input name="k" type="slider" … >}}`** (the
> same supported seam as `{{< video >}}`/`{{< embed >}}`, in `render/extension/mod.rs`), which
> needs no body and expands to the control HTML as a block. Everything else in this spec holds
> verbatim — the emitted `.qmd-input` HTML, `data-qmd-input`, `validate_input`, the `qmd-js.js`
> registration, the five types, the slider readout, and all invariants. Mentally read
> "`::: {.input …}`" as "`{{< input … >}}`" throughout.

## Summary

A declarative `::: {.input name="k" type="slider" …}` fenced div that emits a **static,
keyboard-accessible labeled control** feeding the already-shipped `{js}` reactive graph.
The control's value becomes a named reactive node, so a consuming `{js}` cell (and its
transitive downstream) re-runs when the reader changes it — "drag the slider, the chart
updates" as a one-liner, with **no `//| viewof` boilerplate**, fully client-side and
offline.

It is the authoring sugar that turns the Wave-3 reactive scheduler into an everyday
explorable-document primitive. v1 ships five control types: **slider** (range), **number**,
**checkbox**, **text**, **select**.

## Goals

- `::: {.input name="k" type="slider" min=1 max=10 value=3}` renders a labeled range
  control whose value is the reactive node `k`.
- A `{js}` cell with `//| input: k` reading `qmd.value("k")` re-runs when `k` changes,
  including transitively (`k → derived → consumer`), via the existing scheduler.
- Five control types, each emitting the matching native control: slider, number,
  checkbox, text, select.
- The control is server-rendered static HTML (renders + is keyboard-operable before/without
  JS); the runtime only *registers* it.
- A slider shows a live numeric readout; other controls show their own value natively.
- Invalid usage (no `name`, unknown `type`, `select` without `options`) emits a located,
  click-to-source warning but still renders.

## Non-goals (v1, YAGNI)

- **No new reactive machinery.** Reuse `registerInput` + `scheduleFrom` + the graph as-is.
- **No prose interpolation** of an input's value (controls feed `{js}` cells only).
- **No radio/date/color/file/multi-select** types (add later via the same arm if a corpus
  doc needs one).
- **No two-way bind to the source** — the control is reader interaction with the rendered
  output, never a write back to the `.qmd` (see Invariants).
- **No `<output>` readout** for non-slider controls (they display their own value).

## Invariants honoured

- **HTML-only**, **offline** (native form controls + the bundled `qmd-js.js`; no new dep).
- **Single editing surface preserved.** The control drives client-side JS exactly like the
  existing `//| viewof` sliders; it is reader interaction with the read-only rendered view,
  not a source edit. The preview never writes the `.qmd`.
- **Block model untouched.** The `.input` div is one container block carrying the usual
  `data-block-id` + `data-sourcepos` (like every other `build_container` arm); no diff /
  numbering / sourcepos change.
- **Rides supported seams:** a `build_container` arm (`divs.rs`) + an additive scan in the
  `qmdEnhancers`-registered `enhance` (`qmd-js.js`). Do-NOT-touch machinery (`cite.rs`,
  `includes.rs`, numbering, exec/freeze/kernel) untouched.

## Authoring syntax

```
::: {.input name="k" type="slider" min="1" max="10" step="1" value="3" label="k"}
:::
```

Attributes (parsed by the existing `parse_attrs` → `DivAttrs.kv`):

| Attr      | Applies to            | Notes                                                        |
|-----------|-----------------------|-------------------------------------------------------------|
| `name`    | all (**required**)    | the reactive node name; missing → located warning           |
| `type`    | all (default `slider`)| `slider`\|`range`, `number`, `checkbox`, `text`, `select`    |
| `label`   | all                   | display label; default = `name`                             |
| `min`/`max`/`step` | slider, number | numeric bounds                                              |
| `value`   | all                   | initial value; checkbox `value="true"` → checked            |
| `options` | select (**required**) | comma-separated, e.g. `options="a,b,c"`; missing → warning   |

The div body is ignored (empty by convention).

## Emitted HTML

Slider (the only type with a live `<output>` readout):

```html
<div class="qmd-input" data-block-id="…" data-sourcepos="…">
  <label class="qmd-input-label" for="qin-<id>">k</label>
  <input id="qin-<id>" class="qmd-input-control" data-qmd-input="k"
         type="range" min="1" max="10" step="1" value="3">
  <output class="qmd-input-out" data-qmd-out>3</output>
</div>
```

Other types emit the matching native control inside the same `.qmd-input` + `<label>`
wrapper, with `data-qmd-input="name"` on the control and NO `<output>`:

- `number`  → `<input type="number" min max step value>`
- `checkbox`→ `<input type="checkbox" checked?>` (`value="true"` → `checked`)
- `text`    → `<input type="text" value>`
- `select`  → `<select><option [selected]>opt</option>…</select>` from `options=`

All attribute values are HTML-escaped (`escape_attr` / `html_escape`). The `for`/`id`
pair derives from the container's block id (unique + stable). `qmd.value(name)` returns,
via the existing `readValue`: number (range/number → `valueAsNumber`), boolean (checkbox →
`checked`), or string (text/select → `value`).

## Runtime (`qmd-js.js`, additive)

In `enhance`, **before** the cell scan + run, register static inputs so their value is
available when consumer cells first run:

```js
(root || document).querySelectorAll('[data-qmd-input]:not([data-qmd-input-bound])')
  .forEach(function (el) {
    el.setAttribute('data-qmd-input-bound', '1');
    var name = el.getAttribute('data-qmd-input');
    if (!name) return;
    registerInput(r, name, el);                       // existing: stores r.inputs[name] + input->scheduleFrom
    var out = el.parentNode && el.parentNode.querySelector('[data-qmd-out]');
    if (out) { var upd = function () { out.textContent = readValue(el); };
               el.addEventListener('input', upd); upd(); }
  });
```

This reuses `registerInput` (the same path `//| viewof` cells use) and the graph: a
consumer cell lists `name` in `//| input:`, so `g.consumers[name]` is populated and the
control's `input` event → `scheduleFrom(r, name)` re-runs the transitive-downstream
closure in topological order — the existing, tested behavior. Live block-swap re-registers
via the `:not([data-qmd-input-bound])` guard (overwrites `r.inputs[name]`, same as viewof).

## Validation (`validate.rs`)

A `validate_input(attrs, line, file)` called from the `.input` arm, emitting located
(click-to-source) warnings; the div still renders:

- missing `name` → "`.input` needs a `name=` to feed the reactive graph".
- unknown `type` → did-you-mean against `["slider","range","number","checkbox","text","select"]`.
- `select` with no `options` → "`.input type=select` needs `options=\"a,b,c\"`".

Mirrors the existing `validate_callout_kind` / `validate_walkthrough` pattern.

## Styling (`base.css`)

A `.qmd-input` flex row: `display:flex; gap:.6rem; align-items:center; margin:…`. The
`.qmd-input-label` is the control's clickable label; `.qmd-input-out` is the slider's
value readout (tabular-nums). Native controls inherit `color-scheme` (already set per
theme), so dark/sepia work without per-control overrides. Wraps gracefully on narrow
viewports.

## Corpus pin

`corpus/reactive/inputs.qmd`, exercising every type + the graph:
- a `slider` `k` and a `{js}` cell `//| input: k` that shows `k` (and a derived `//| name`
  consumed transitively, proving the scheduler);
- a `number`, a `checkbox`, a `text`, and a `select`, each feeding a small `{js}` cell;
- one `{js}` cell reading several inputs at once.

Auto-covered by the corpus invariants (`corpus.rs`: renders, unique block ids, valid
sourcepos, document order). Front-matter stays clean.

## Testing strategy

1. **Rust render test** (`crates/core/tests/` or `render/tests.rs`): the `.input` arm emits,
   for each of the 5 types, a `.qmd-input` block with `data-qmd-input="<name>"`, the correct
   native control (`type=range`/`number`/`checkbox`/`text`, or `<select>` with `<option>`s),
   the label, slider `<output data-qmd-out>`, and escaped attributes. Plus `validate_input`
   unit tests: missing-name, unknown-type (did-you-mean), select-without-options.
2. **Corpus invariants** (auto): the pin doc renders and satisfies the block-model
   guarantees.
3. **Browser (chrome-devtools MCP)** against the live preview of the pin doc (live `{js}`
   needs a real server): moving the slider re-runs its consumer cell AND the transitive
   downstream; checkbox/select/text/number each drive their cell; the slider `<output>`
   tracks the value; keyboard operation works; 0 console errors.

## Risks & mitigations

- **Input registered after consumer cell first runs** → stale initial value. Mitigated by
  ordering the static-input scan *before* the cell scan/run in `enhance`.
- **Name collision** between a `.input` and a `//| viewof:` of the same name → last
  registration wins (author error; acceptable, same as two viewofs).
- **`select` value not matching any option** → no option selected (native behavior);
  the warning path covers the missing-options case, not value mismatch (low stakes).
- **Live-swap** of an `.input` block → `:not([data-qmd-input-bound])` re-registers the new
  element and overwrites `r.inputs[name]`; consumer cells re-run on next change.

## Out of scope follow-ups (recorded, not built)

- Additional types (radio, date, color, range-pair, multi-select).
- `value:Label` pairs in `select options`.
- Using an input's value directly in prose (would need a text-interpolation surface).
- Two-device / shared-state sync of control values.
