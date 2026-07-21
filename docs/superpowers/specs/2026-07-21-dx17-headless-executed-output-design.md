# DX17: headless executed-output visibility

Date: 2026-07-21
Status: design approved, ready for plan
Backlog item: A.1 (DX17), the top-ranked open item after the AI-native packaging work shipped.
Detail source: `notes/2026-07-18-dx-audit.md` (recommendation #17, "the single biggest thing
that makes an agent fly blind").

## Problem

A headless agent authoring `.tmd` cannot tell whether its computed output actually
executed. The four inspection surfaces each stop short of the executed result:

- `read` projects the **source** block model, parse-only. A `{python}`/`{r}` cell shows its
  code; the figure it would produce is absent. `cmd_read` even warns "kernel cells projected
  as source; outputs will be absent" (`crates/server/src/query.rs`, `cmd_read`).
- `check` is **static** (links, xrefs, a11y, front-matter); it never runs a cell.
- `build` **does** run python/r, but emits only HTML, not an agent-legible summary.
- `{js}` cells are **never** server-run (client-only by design), so an Observable-Plot chart
  (the corpus's own idiom) is unobservable headlessly by any command.

Net: a chart's correctness is not headlessly observable. The agent can write a plotting cell,
run nothing that reports back, and ship a doc whose figure silently errored.

## Goal / non-goals

**Goal.** Give a headless agent a single command that reports what each computed cell actually
produced: a figure (with its label/alt), a table, text output, or an error — for python/r now
(Phase 1) and for `{js}` via a headless browser later (Phase 2).

**Non-goals.**
- No change to `read`'s default behavior. Bare `read` stays parse-only and backward-compatible;
  execution is strictly opt-in behind `--run`.
- No new execution/freeze machinery. Phase 1 reuses `exec::Executor` exactly as `build` does;
  no freeze-key change, no kernel-lifecycle change (the do-not-touch zone is not entered).
- No preview write-back, no block-model/sourcepos change (executed output blocks already carry
  both, pinned by `output_block_keys_id_to_cell_and_carries_clickto_source`).
- No server-side reactive runtime. Phase 2 only *observes* the existing client run headlessly;
  it does not feed inputs back or re-run cells reactively (the CUT `js-kernel-rerun`
  reactive-VM trap stays out).
- Pixel-perfect rendering is not the target. The agent's blind spot is "did it run, did it
  throw, did it produce a plot node" — not a faithful raster.

## Phasing

Two independently-shippable phases, each corpus-pinned, merged and verified on its own branch.
Phase 2's larger risk (a new Chrome dependency) cannot hold up the ★★★★ Phase 1 win.

- **Phase 1 (a):** `read --run` surfaces executed **python/r** output. This spec, in full.
- **Phase 2 (b):** headless **`{js}`** evaluation via a local headless Chrome. Sketched here;
  gets its own spec + plan when Phase 1 lands.

---

## Phase 1 (a): `read --run` — executed python/r visibility

### CLI surface

```
taliesin read <file.tmd> [--run] [--format human|json | --json]
```

- `read <file>` — unchanged. Parse-only text projection + the existing "kernel cells projected
  as source" warning.
- `read --run <file>` — execute python/r cells, then project the *executed* block model.
- `--format json` / `--json` — structured output; composes with `--run` (with `--run` the
  per-cell results carry produced/kind/error; without it, cells report `produced: false,
  kind: "not-run"`).
- `--format` accepts `human` (default) | `json`; an unknown value uses the shared
  `bad_format_error`, matching `map`/`build`.

Flag name: `--run` (reads cleanly; matches the existing warning's "Use `build` or `preview` to
run them"). Chosen over `--exec`/`-x`/`--execute`.

**Wiring.** `cmd_read` grows an arg parser mirroring `cmd_map` (path + `--run` + `--format`/
`--json` + unknown-flag rejection via `unknown_flag_error`). `main.rs` changes
`query::cmd_read(args.get(2))` to `query::cmd_read(&args)`. A `READ_FLAGS` const feeds the
unknown-flag "did you mean".

### Execution path (`--run`)

A subset of `build`'s single-doc exec path (`build.rs` ~639), minus HTML assembly and static
diagnostics (projection, not linting, is `read`'s job):

1. `render_document_with_includes_rooted(&src, base, Some(base))` → `doc` (as today).
2. `exec::Executor::with_freeze(freeze::page_path(&base.join("_freeze"), stem)).in_dir(base)`
   then `.set_interpreters(interpreter::resolve_python(None, base),
   interpreter::resolve_r(None, base))`.
3. Spin a tokio runtime and `block_on(ex.run(std::mem::take(&mut doc.blocks)))` — the same
   `Runtime::new()?.block_on(...)` shape `cmd_build` uses (`build.rs:551`). No warm pool (a
   one-shot CLI read, exactly like single-doc build).
4. Surface `ex.diagnostic()` as a warning (kernel unavailable → same located hint build gives).
5. Project the executed blocks (below).

Behavior inherited unchanged from `Executor`: freeze-replay for unchanged cells (no kernel
boot when the doc is unchanged), kernel only for changed cells, `TALIESIN_NO_EXEC` (returns
source), `TALIESIN_CELL_TIMEOUT`, error tracebacks baked into an output block.

### Text projection

Extend `render/text.rs` with one arm that recognizes the executed output block (the
`<div class="tali-output">` `output_block` emits). The inner leading element decides the kind
(an unambiguous, exhaustive mapping):

- **`<figure>`** (labelled figure cell) → `[figure fig-x: produced, alt "caption"]`.
- **`<img>` / `<svg>`** as an unlabelled rich output (a matplotlib PNG, a plot) → also a
  produced figure to the agent: `[figure: produced (image)]`, `kind: "figure"`.
- **`<table>`** → `[table tbl-x: produced]` / `[table: produced]`.
- **`pre.tali-error`** → `[cell error: <summary>]`, where `<summary>` is the last non-empty
  line of the (ANSI-stripped) `tali-error` text — for a no-traceback error that line is exactly
  `EName: evalue`, and for a traceback the last line is the same summary. `kind: "error"`.
- **`pre.tali-stream`** / other plain text → `[output: <first non-empty line, truncated ~120
  chars>]`, `kind: "stream"`.
- **any other rich HTML** (e.g. an unlabelled DataFrame table `<div>`) → `[output: produced]`,
  `kind: "rich"`.

The cell's source still projects first (as today), then its output block follows, so the agent
reads *code → result* in order. A cell that produced nothing visible projects no extra line
(matches build, which splices no output block for empty/`include:false` cells); in JSON such a
cell is `produced: false, kind: "empty"`.

The detection keys off the `tali-output` class + that inner leading element, reusing the
existing `class_tag_span`/`leading_tag` helpers in `text.rs`. It reads the already-rendered
output HTML — it does **not** reach back into exec for the structured `Output` (keeping the
"no exec machinery change" non-goal), which is why the error is surfaced as a summary line
rather than a clean `{ename, evalue}` split. No dependency on execution to *test* it: an output block can be
constructed directly (as the `output_block` unit tests already do).

### JSON projection

`read --format json` emits:

```json
{
  "path": "posts/x/index.tmd",
  "executed": true,
  "cells": [
    { "id": "b-abc", "lang": "python", "label": "fig-hist", "produced": true,
      "kind": "figure", "fig_id": "fig-hist", "alt": "A histogram of …" },
    { "id": "b-def", "lang": "python", "produced": true, "kind": "stream" },
    { "id": "b-ghi", "lang": "r", "produced": false, "kind": "error",
      "error": "object 'x' not found" }
  ],
  "text": "<the full human text projection>"
}
```

- `executed` reflects whether `--run` was given (`false` → every cell `kind: "not-run"`).
- `kind` ∈ `figure | table | stream | rich | error | empty | not-run`.
- `cells` lists executable cells (python/r) in document order; `label`/`fig_id`/`alt`/`error`
  are present only when applicable. `error` is the same summary string the text projection
  shows (see the error-kind note above), not a structured `{ename, evalue}` split.
- `text` carries the human projection too, so one JSON call gives both shapes.

Serialized with `serde_json::to_string_pretty`, mirroring `cmd_map`. New `#[derive(Serialize)]`
structs live beside `cmd_read` (or a small `read.rs`), not in core.

### Tests / corpus pin

- **Kernel-free unit tests** (always run, carry the real coverage): construct executed
  `output_block`s directly and assert the new `text.rs` arm renders each kind
  (`[figure fig-x: produced, alt "…"]`, `[table …]`, `[output: …]`, `[cell error: …]`), plus a
  JSON-shape test. This is the pin that can't rot, exactly as `render_outputs`' unit tests
  carry the kernel-dependent output-format coverage.
- **Corpus doc** `corpus/agent/executed-read.tmd`: a labelled-figure python cell, a table cell,
  and a deliberately-erroring cell. The corpus suite renders it parse-only (proves it
  parses/renders and stays in the regression net); `corpus/README.md` gets a one-line entry.
- **Kernel-gated integration test** `crates/server/tests/read_run.rs`: run `read --run`
  (and `--run --format json`) over the corpus doc against a real kernel, asserting the executed
  figure/table/error projection. Skips without `TALIESIN_PYTHON`; hard-fails under
  `TALIESIN_REQUIRE_KERNEL` (the CI canary pattern, so an env regression can't silently green
  it).

### Invariants held

Read-only (never writes source). Reuses `exec`/`freeze` unchanged — no new exec machinery, no
freeze-key change, no kernel-lifecycle touch. Block model / sourcepos untouched (`output_block`
already carries `data-block-id`/`data-sourcepos`). Projection is purely additive to `text.rs`.
Offline unaffected (no network). `--run` is opt-in, so the documented parse-only `read`
contract is preserved.

### Docs

`docs/guide/reference/` (the `read`/agent surface) and the scaffolded `AGENTS.md` on-ramp gain
a line on `read --run` as the "did my cell produce a figure?" check. Deferred to the
implementation change so the docs describe shipped behavior.

---

## Phase 2 (b): headless `{js}` via local Chrome — sketch (own spec later)

`read --run` additionally drives a **local headless Chrome** (`chromiumoxide` crate) to run
`{js}` cells the way they actually run — in a browser — and report back:

- Build the page HTML (reuse the existing build artifact / an in-memory render), load it over
  `file://`, wait on the existing settle signal (`data-qmd-done`; do **not** gate on
  `data-qmd-ran`, per the ui-audit harness lesson), then read back each js-cell node.
- Produced `<svg>`/canvas → `[js: produced, <svg 640×400>]`; a captured `qmd-js-error` →
  `[js error: <message>]`.
- **Gated + optional.** No Chrome on the system → `[js: skipped (chrome unavailable)]`, never a
  hard failure; python/r-only users never need Chrome. Offline holds (local page + local
  browser, no network; the built page already inlines all assets).
- Adds `chromiumoxide` (+ `futures`) for this path only. New corpus pin: a `{js}`
  Observable-Plot doc reporting "produced" headlessly.
- **Reactive-VM trap avoided:** observation only. No input feedback, no server-side re-run, no
  `_freeze` write for `{js}`. If ever it grew toward driving inputs, that reverts to the CUT
  `js-kernel-rerun` gate (feature-flagged, freeze-byte-identical merge gate).

Open questions deferred to the Phase 2 spec: exact flag (`--run` doing js too vs a distinct
`--run-js`); whether to reuse `build`'s on-disk HTML or an in-memory render; the Chrome
discovery/launch + timeout policy; SVG size/summary format.

---

## Sequencing

1. **Phase 1** on `feat/dx17-read-run`: flag parsing → exec wiring → `text.rs` arm → JSON
   structs → unit tests → corpus doc → kernel-gated integration test → docs. TDD: the
   kernel-free `text.rs`/JSON unit tests are written first (RED), then the projection arm.
2. Merge Phase 1, strike DX17(a) from the backlog (leave DX17(b) as the remaining fork).
3. **Phase 2** later, its own spec + plan + branch, when Phase 1 has landed and the Chrome
   dependency is worth taking on.
