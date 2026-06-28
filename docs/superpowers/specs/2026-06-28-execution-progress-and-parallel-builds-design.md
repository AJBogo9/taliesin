# In-browser execution progress + parallel page builds — Design

- **Date:** 2026-06-28
- **Status:** Approved (design); implementation deferred (a concurrent session is editing the tool)
- **Author:** Andreas Bogossian (with Claude)
- **Scope:** Surface code-cell execution progress in the *browser*, and build independent
  site pages in *parallel*, to remove the "is it frozen?" wait and use multi-core hardware.

## 1. Why

Across every comparable tool (Jupyter, JupyterLab, VS Code notebooks, Quarto, Observable,
Marimo, Pluto.jl, Streamlit, Hex, Deepnote, Databricks) the dominant user complaint about
execution is **"is it frozen or is it working?"** qmd-fast is structurally *worse* than
vanilla Jupyter here: the only execution signal today is `crate::log::exec(k, n)` printed to
the terminal (`crates/server/src/exec.rs:372-374`); the browser sees **nothing** until the
entire rebuild of every cell finishes and the diff ops are broadcast once under a lock. A
reader watching the rendered page has zero liveness signal.

Because the reader watches the **HTML page**, not the terminal, the browser is exactly where
qmd-fast can beat Quarto (whose render feedback is terminal-only and whose browser is a dumb
auto-reload target). This is the highest-value, lowest-controversy win.

Two hard lessons from the research shape the whole design:

1. **A lying indicator is worse than none.** Every tool that collapsed *queued* into *running*
   got bug reports (Jupyter `[*]`, Pluto #1585, VS Code #13426); every stuck/desynced
   indicator (JupyterLab #6267/#16001) eroded trust. States must be honest and distinct.
2. **qmd-fast cannot honestly compute an ETA or percentage.** There is no per-cell dependency
   graph (`docs/internals/...execution.qmd:113-116` says so explicitly) and no historical
   per-cell timings. A fabricated ETA would violate lesson 1. What qmd-fast *can* show
   honestly is exactly what it already computes: which cells are done/queued/running (from
   `plan()`'s zones), a deterministic **cell k of N** (from the `exec(k, n)` counter), and a
   **live elapsed timer** (ticked client-side from a start timestamp).

## 2. Locked decisions

| Dimension | Choice | Rationale |
|---|---|---|
| Parallelism level | **Page-level only** | Pages already have isolated kernels + cwd; near-linear speedup, zero shared-namespace risk. Intra-page is GIL-bound and breaks top-to-bottom semantics. |
| Parallel rollout | **Default on, memory-capped** (off switch) | Matches "use the hardware"; RAM, not cores, is the binding limit. |
| Progress honesty | **Per-cell states + live timer + k/N count; no ETA** | Everything qmd-fast can compute truthfully today; no fabricated ETA. |
| Kernel warm-up | **Eager forkserver warm-pool** (fallback: eager pre-boot) | Pre-import heavy libs once; near-instant first edit; uses spare RAM for speed. |
| Chip during parallel builds | **Viewed-page progress + a small "N pages building" line** | Matches the 1-viewer-1-page model without hiding that the machine is busy. |
| Warm-pool vs `MAX_WARM_PAGES` | **Decouple**: transient build kernels scheduled by RAM budget, separate from the warm-MRU set | Avoids eviction churn while allowing peak build concurrency. |

**Non-goals (explicitly out of scope):** running one page's cells across cores in a single
kernel; multi-kernel reconciliation of a single page's namespace (SoS `%put/%get`,
fork-and-merge); any ETA/percentage; a static def/ref dependency DAG (revisit only if a single
page ever dominates wall-clock).

## 3. Architecture grounding (current behavior)

- **Execution is per-language sequential, no DAG.** `Executor::run` (`exec.rs:227`) groups
  cells by language preserving document order, then `compute_outputs(lang, cells)`
  (`exec.rs:263`) runs them. `plan(ran, hashes, known, cacheable)` (`exec.rs:515`) returns
  three zones (`exec.rs:340-388`): warm-prefix `[0, shared)` restored from memory, run-range
  `[shared, run_end)` executed sequentially via `exec_cell -> kernel.execute().await`
  (`exec.rs:383, 477-491`), cached-tail `[run_end, len)` restored from `_freeze/`. Incremental
  correctness is the cumulative content-hash chain (`freeze.rs:64-72`): editing cell *i* moves
  key *i* and all downstream keys. The kernel boots lazily, only when `to_run > 0`
  (`exec.rs:329`, `ensure_kernel` `exec.rs:430`).
- **One warm kernel per language per page.** `ExecPool` (`serve_site.rs:679`) holds one
  `Executor` per page, bounded to `MAX_WARM_PAGES = 6` by MRU (`serve_site.rs:675`); eviction
  drops the executor and kills its kernels. Each executor runs with `cwd =` the page's own
  directory (`exec.rs:148`). A single `spawn_builder` worker drains `build_tx` and owns the
  pool, so **all page builds are serialized through one worker** (`serve_site.rs:784-805`); the
  single-doc server has one executor for the session (`serve.rs:1015`).
- **Output streaming is end-of-build only.** On WS connect the server sends a `snapshot`
  (`serve_site.rs:583-588`), then streams broadcast text frames; today those frames are the
  final diff ops produced after the whole rebuild. Each cell's output is its own block with id
  `{cell.id}-out` (`exec.rs:622-646`), and the client `replaceWith`s that block on update
  (`web-client/client.js:696-707`).
- **`{js}` cells already have a client-only reactive graph** (`qmd-js.js`); Python/R cells do
  not. This design does not change either graph.

## 4. The one enabling change

Relax **"the whole rebuild completes before any op is sent"** to **stream lightweight progress
ops mid-build**. Everything else is additive (decoration + earlier/finer op emission), not a
new execution engine.

Mechanism: thread a `ProgressSink` (a cheap message sender) into `Executor::run` /
`compute_outputs`. The executor emits transitions as it walks `plan()`'s zones; the builder
forwards them onto the page's existing `broadcast` channel (`serve_site.rs:581`) and the
single-doc `tx`. The sink is a no-op when absent (keeps `build`/headless paths unchanged).

## 5. Progress protocol (server → browser)

Two new WS message kinds on the existing channel (JSON, same envelope as current ops):

- `cell-state`: `{ type, page, cell_id, state, started_at?, duration_ms? }`,
  `state ∈ queued | running | done | error`.
  - `queued` emitted up front for every cell in the run-range `[shared, run_end)`.
  - `running` emitted immediately before `kernel.execute().await`, with `started_at`.
  - `done` emitted after, with `duration_ms`. Warm-prefix and cached-tail cells emit `done`
    immediately (cache hits feel instant).
  - `error` on an uncacheable/`qmd-error` output (`is_uncacheable` `exec.rs:579`) or kernel
    death (`KERNEL_DIED_HTML`).
- `build-state`: `{ type, page, phase, ran, total, lang }`,
  `phase ∈ warming-kernel | executing | idle | error`. `ran/total` come straight from the
  existing `exec(ran_count, to_run)` call site (`exec.rs:372`). `warming-kernel` is emitted at
  the lazy `ensure_kernel` boundary before the first cell; `idle` when the build settles (with
  total build duration); `error` for `KERNEL_RETRY_AFTER` backoff / mid-run death.

Rules: states are **monotonic per cell** (`queued → running → done|error`) and **never
collapse queued into running**. The server never sends sub-second timer updates; the client
ticks locally from `started_at`. A cold start where every freeze key is on disk boots no
kernel and runs zero cells (`exec.rs:329`): emit `idle` ("up to date, cached") immediately, no
spinner.

## 6. Client UX

**Per-cell decoration.** On `cell-state`, set `data-qmd-cell-state` on the existing
`{cell.id}-out` block (no output HTML resent) and render a thin colored left-border + a small
badge: clock (queued), spinner + **live ticking timer** (running), green check + final
duration (done), red + jump affordance (error). While `running`, keep the previous output in
place but dimmed (skeleton/continuity), since the op model swaps the `-out` block atomically.

**Global status chip.** A fixed-corner chip injected into the previewed page (extends the
existing dev panel surface):
- idle/busy dot that never lies;
- one honest line: `Starting kernel (python)…` → `Executing 3/8: model_fit 4.2s` →
  `Up to date, built in 1.2s`;
- a deterministic **k-of-N** mini-bar (never a fake percentage);
- **click-to-scroll** to the active (or erroring) cell (long pages scroll it out of view);
- a **tab-title / favicon** flip while building, on error, or for builds > ~5s (out-of-band
  signal for a backgrounded tab).

During parallel builds the chip shows the **viewed page's** progress plus a small
`N pages building` line; it does not blend other pages' cell counts into the viewed page.

JS style matches the bundled `qmd-js.js` / `client.js` (ES5-ish `var`/`function`, no new deps,
offline). No change to the `{js}` reactive graph or the click-to-source block model.

## 7. Parallel page builds (default on, memory-capped)

Replace the single serialized `spawn_builder` worker (`serve_site.rs:784`) with a
**memory-budgeted scheduler** that builds independent **dirty** pages concurrently:

- Concurrency cap = `min(physical_cores, floor(free_RAM / per_kernel_estimate))`, with a
  conservative `per_kernel_estimate` (≥ ~150 MB; higher when torch/numpy are imported) and a
  floor of 1. Configurable via `--jobs N` (0/auto = computed cap, 1 = sequential).
- Each page already gets its own `Executor` (kernel + cwd), so concurrent builds share no
  in-memory namespace. Per-page execution stays **exactly sequential**.
- **Invariant (must be tested):** a parallel build's output is byte-identical to a sequential
  build's output. Parallelism changes *scheduling*, never *results*.
- **File isolation:** verify `_freeze/` writes are per-page-keyed (no cross-page race) and that
  no two pages write the same intermediate filename in a shared cwd; give each build job an
  isolated working/temp dir where needed.
- **Ordering edges:** pages that depend on others (e.g. a `listing:`/index page that consumes
  sibling pages) build **after** their sources; only mutually-independent pages run
  concurrently.
- **Relationship to `MAX_WARM_PAGES`:** decouple transient build concurrency (RAM-budgeted)
  from the warm-MRU set used for fast revisits, so building more pages than are kept warm does
  not cause eviction churn of the warm set.

## 8. Eager forkserver warm-pool

At server start, pre-warm a small pool of kernels via Python's **forkserver** start method
with `set_forkserver_preload([...])` of the heavy libs (configurable; default e.g.
`numpy, matplotlib`, and `torch` when present), so forked ipykernel children inherit
already-imported modules via copy-on-write and the first cell runs near-instantly. qmd-fast
currently spawns `python -m ipykernel_launcher` directly (`kernel.rs:265-298`); the warm-pool
adds a small spawner path that hands out pre-warmed kernels and falls back to plain eager
pre-boot when forkserver is unavailable (e.g. R, or preload import failure). Overlap next-page
kernel boot with the current page's output writing. Pool size is part of the same RAM budget
as §7.

## 9. Components & interfaces (where the work lands)

- `crates/server/src/exec.rs` — add a `ProgressSink`; emit `cell-state`/`build-state` from
  `compute_outputs` zone-walk and `ensure_kernel`; re-route the `exec()` counter.
- `crates/server/src/serve_site.rs` — forward progress onto the per-page broadcast; replace the
  serial builder with the memory-budgeted concurrent scheduler; wire `--jobs`/RAM cap; warm-pool
  ownership.
- `crates/server/src/serve.rs` — single-doc path: forward progress on the session `tx`.
- `crates/server/src/protocol.rs` — the new message kinds.
- `crates/server/src/kernel.rs` — forkserver warm-pool spawner path + fallback.
- `web-client/client.js` (+ a small CSS surface) — handle `cell-state`/`build-state`, per-cell
  decoration, the status chip, click-to-scroll, tab-title/favicon, local timer.
- CLI/config — `--jobs`, warm-pool/preload config keys; document defaults.

## 10. Global constraints (must hold)

- **HTML-only, offline:** no new browser dependency; chip + decorations are vanilla JS/CSS.
- **Single editing surface:** progress UI is read-only reader feedback; it never writes the
  `.qmd`.
- **Click-to-source block model untouched:** decoration rides the existing `-out` block's
  `data-block-id`/`data-sourcepos`; no new blocks, no numbering/sourcepos change.
- **Freeze determinism preserved:** progress emission is side-effect-free w.r.t. cached
  outputs; parallel scheduling never changes a page's computed result.
- **Do-NOT-touch machinery** (`cite.rs`, `includes.rs`, numbering, the `:::` div machine, the
  `{js}` reactive graph) is untouched.
- **Honest-state rules** from §1 are normative, not optional.

## 11. Testing strategy

- **Determinism:** parallel build (`--jobs N`) output == sequential output, byte-for-byte, on
  the multi-page corpus.
- **Progress ordering:** per-cell states are monotonic and never collapse queued→running;
  `build-state` `ran/total` matches the executed count; cache-hit pages emit `idle` with zero
  running cells.
- **Scheduler:** RAM-budget cap respected; only dirty pages run; ordering edges honored
  (dependent pages after sources); off switch (`--jobs 1`) restores serial behavior.
- **Warm-pool:** forkserver pool boots; first-cell latency drops vs cold; graceful fallback
  when forkserver/preload fails.
- **Client:** state decoration + chip render correct states; timer ticks; click-to-scroll
  targets the active/erroring cell. (chrome-devtools MCP for the visual checks, matching the
  repo's existing corpus/visual test approach.)

## 12. Phasing (smallest-valuable-first; each independently shippable)

- **P0 — progress plumbing + dumb chip:** route `exec(k, n)` to a WS `build-state` op; render a
  minimal global chip showing `cell k/N`. Proves terminal→browser progress end to end; kills
  "is it frozen?".
- **P1 — per-cell honest states:** stream `cell-state` ops; paint queued/running(+live
  timer)/done(+duration)/error on each `-out` block. Delivers the spinner-on-not-yet-run ask.
- **P2 — chip polish + warm-up legibility:** idle/busy dot, k/N bar, click-to-scroll,
  `Up to date in Xs`, tab-title/favicon, distinct `Starting kernel…` state.
- **P3 — parallel page builds:** memory-budgeted concurrent scheduler (default on, `--jobs`),
  file-isolation + ordering-edge guards, determinism test.
- **P4 — eager forkserver warm-pool + boot/import overlap:** decouple from `MAX_WARM_PAGES`;
  tune RAM cap.

## 13. Risks

- **RAM is the binding constraint**, not cores (~150 MB+/kernel, far more with torch): the
  scheduler must cap on memory and degrade gracefully, not OOM.
- **File collisions / cross-page deps:** the two mechanical ways parallelism can corrupt
  output; mitigated by per-page cwd, freeze-key isolation checks, and ordering edges.
- **Mid-build progress from multiple pages** complicates the chip; mitigated by per-viewed-page
  attribution + a small site-wide line.
- **forkserver caveats:** helps only when pages share heavy imports; needs a clean fallback.
- **Concurrent tool edits:** another session edits qmd-fast; implementation must rebase onto
  that and re-verify the cited line numbers before coding.
