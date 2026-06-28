# In-browser execution progress + parallel page builds — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stream honest code-cell execution progress into the browser (per-cell queued/running/done/error + live timer + a global k-of-N chip), and build independent site pages in parallel (default-on, memory-capped) with an eager forkserver kernel warm-pool, so a reader never wonders "is it frozen?" and multi-core hardware is actually used.

**Architecture:** One enabling change (stream lightweight progress ops mid-build instead of only after the whole rebuild), then additive decoration on the client. Parallelism is added ONLY at the page boundary, where qmd-fast already isolates state (one `Executor` + kernel + cwd per page); per-page cell execution stays exactly sequential, so results are byte-identical to a sequential build. Full design: `docs/superpowers/specs/2026-06-28-execution-progress-and-parallel-builds-design.md`.

**Tech Stack:** Rust (edition 2024; `crates/server`, `crates/core`), vanilla ES5-style JS (`web-client/client.js`), the CSS custom-property theme system, serde_json wire messages, tokio async runtime. Tests: Rust unit/integration + corpus + chrome-devtools MCP for visuals.

## Global Constraints

- **HTML-only output; offline.** No new browser dependency; chip + decorations are vanilla JS/CSS bundled in `web-client/`.
- **Honest states are normative.** Per-cell state is one of `queued | running | done | error`, monotonic per cell, and **queued is NEVER collapsed into running**. **No ETA / no percentage** anywhere (qmd-fast has no per-cell DAG or historical timings; a fabricated number is a defect). Allowed quantities: per-cell state, client-ticked elapsed timer, deterministic `k`-of-`N` count.
- **Single editing surface preserved.** Progress UI is read-only reader feedback; it never writes the `.qmd`.
- **Click-to-source block model untouched.** Decoration rides the existing per-cell output block (`{cell.id}-out`, see `exec.rs` output-block emission) via a `data-` attribute; no new blocks, no change to `data-block-id` / `data-sourcepos` / numbering.
- **Freeze determinism preserved.** Progress emission is side-effect-free w.r.t. cached outputs. Parallel scheduling changes timing only; a page's computed output is unchanged.
- **Do-NOT-touch machinery:** `cite.rs`, `includes.rs`, numbering, the `:::` div machine, and the `{js}` reactive graph (`qmd-js.js`) stay untouched.
- **JS style:** match `client.js` / `qmd-js.js` (`var`/`function`, `[].forEach.call`, no new arrow-fn/`const` requirement, `"use strict"` already set).
- **Wire protocol lives in `crates/server/src/protocol.rs`** as `{ "type": ... }` JSON helper fns shared by both servers; the client switches on `msg.type` in `handle()`. Add new messages there, never inline in a server.

> **PRE-FLIGHT (read before Task 1).** A concurrent session is editing this tool, and the file:line citations below are from a snapshot. Before each task: `git pull`/rebase, then re-grep the named symbols (`compute_outputs`, `plan`, `ensure_kernel`, `spawn_builder`, `ExecPool`, `handle`, `setStatus`) to confirm exact locations and current signatures. Treat cited line numbers as approximate anchors, not literals. Run `cargo test` and a corpus build green before starting and after each task.

---

## Phase P0 — progress plumbing + minimal global chip

Smallest valuable slice: route the execution counter to the browser and show `cell k/N`. This alone kills the "is it frozen?" complaint.

### Task 1: `build-state` wire message + server emission from the exec counter

**Files:**
- Modify: `crates/server/src/protocol.rs` (add `build_state` helper + unit test)
- Modify: `crates/server/src/exec.rs` (add an optional progress sink; emit at the existing `exec(ran_count, to_run)` site and at build settle)
- Modify: `crates/server/src/serve_site.rs` and `crates/server/src/serve.rs` (construct the sink, forward its messages onto the page broadcast / session tx)

**Interfaces:**
- Produces: `pub fn build_state(page: Option<&str>, phase: &str, ran: u32, total: u32, lang: &str) -> String` in `protocol.rs`; a `ProgressSink` type in `exec.rs`: `pub type ProgressSink = Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>;` (each call receives a ready-to-send JSON string).
- Consumes: the existing per-page `broadcast::Sender<String>` (`serve_site.rs` `PageState.tx`) and single-doc `tx` (`serve.rs`).

- [ ] **Step 1: Write the failing protocol test**

In `crates/server/src/protocol.rs`, inside the existing `#[cfg(test)] mod tests` (add one if absent):

```rust
#[test]
fn build_state_serializes_phase_and_counts() {
    let s = super::build_state(Some("ch1.qmd"), "executing", 3, 8, "python");
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["type"], "build-state");
    assert_eq!(v["page"], "ch1.qmd");
    assert_eq!(v["phase"], "executing");
    assert_eq!(v["ran"], 3);
    assert_eq!(v["total"], 8);
    assert_eq!(v["lang"], "python");
}
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cargo test -p qmd-fast-server protocol::tests::build_state_serializes_phase_and_counts`
Expected: FAIL (`build_state` not found).

- [ ] **Step 3: Implement `build_state`**

In `protocol.rs`, beside `op`/`diagnostics`:

```rust
/// `build-state`: document-level execution phase + a deterministic k-of-N count.
/// `phase` is one of "warming-kernel" | "executing" | "idle" | "error". `page` is
/// the source rel-path for the multi-page server, `None` for the single-doc server.
pub fn build_state(page: Option<&str>, phase: &str, ran: u32, total: u32, lang: &str) -> String {
    serde_json::json!({
        "type": "build-state", "page": page,
        "phase": phase, "ran": ran, "total": total, "lang": lang
    })
    .to_string()
}
```

- [ ] **Step 4: Run it, verify it passes**

Run: `cargo test -p qmd-fast-server protocol::tests::build_state_serializes_phase_and_counts`
Expected: PASS.

- [ ] **Step 5: Thread a `ProgressSink` through the executor**

In `exec.rs`: add `pub type ProgressSink = Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>;`. Give `Executor::run` (and the `compute_outputs` path) access to a `&ProgressSink` (store it on `Executor`, set by the server before a build, or pass as a parameter, whichever matches the current `run` signature you confirmed in pre-flight). Add a tiny helper on the executor:

```rust
fn emit(sink: &ProgressSink, msg: String) {
    if let Some(s) = sink { s(msg); }
}
```

At the existing counter site (currently `crate::log::exec(ran_count, to_run)`), ALSO emit:

```rust
emit(sink, crate::protocol::build_state(page.as_deref(), "executing", ran_count as u32, to_run as u32, lang));
```

Emit `build_state(page, "idle", to_run, to_run, lang)` once the build settles (end of `compute_outputs` for the last language), and `build_state(page, "warming-kernel", 0, to_run, lang)` immediately before `ensure_kernel` boots a kernel (when `to_run > 0`). `page` is available in the site server; pass `None` from the single-doc server.

- [ ] **Step 6: Wire the sink in both servers**

In `serve_site.rs` where a page build runs (the builder calls `ExecPool::get(rel).run(...)`), construct a sink that clones the page's `broadcast::Sender` and sends each message: `Some(Arc::new(move |m| { let _ = tx.send(m); }))`. In `serve.rs`, clone the session `tx` the same way. Confirm the broadcast value type is `String` (it is, per `op`/`full_render` sends); if it is a typed enum, wrap accordingly.

- [ ] **Step 7: Integration test — counter reaches a subscriber mid-build**

Add `crates/server/tests/progress_build_state.rs`:

```rust
// Build a 3-python-cell doc through the executor with a capturing sink and assert
// the sink received build-state messages with increasing `ran` up to total=3.
// Use the existing test harness/builder used by other server integration tests
// (see crates/server/tests/ for the pattern); a kernel must be available
// (gate with the same env/skip other kernel tests use).
```

Implement it against the existing server test harness (confirm the harness entry point during pre-flight). Assert at least one message with `"phase":"executing"` and a final `"phase":"idle"` with `ran == total`.

- [ ] **Step 8: Run the suite**

Run: `cargo test -p qmd-fast-server` (and the kernel-gated integration test).
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/server/src/protocol.rs crates/server/src/exec.rs crates/server/src/serve_site.rs crates/server/src/serve.rs crates/server/tests/progress_build_state.rs
git commit -m "feat(progress): stream build-state (k/N) from the executor to the client"
```

### Task 2: minimal global progress chip in the client

**Files:**
- Modify: `web-client/client.js` (handle `build-state`; render a minimal chip)
- Modify: the client CSS surface (the file/inline-style block where the dev panel styles live; confirm in pre-flight, e.g. `crates/core/assets/css/*` or the client's injected styles)

**Interfaces:**
- Consumes: `build_state` messages from Task 1; the existing `setStatus()` and dev-panel host in `client.js`.
- Produces: a `#qmd-progress` chip element + an exported `updateProgress(msg)` local function.

- [ ] **Step 1: Add the `build-state` case to `handle()`**

In `client.js` `handle(msg)` switch (beside `case "diagnostics":`):

```js
case "build-state":
  updateProgress(msg);
  break;
```

- [ ] **Step 2: Implement `updateProgress` + the chip**

Near the dev-panel setup in `client.js`:

```js
var progressEl = null;
function ensureProgress() {
  if (progressEl) return progressEl;
  progressEl = document.createElement("div");
  progressEl.id = "qmd-progress";
  progressEl.setAttribute("aria-live", "polite");
  document.body.appendChild(progressEl);
  return progressEl;
}
function updateProgress(msg) {
  var el = ensureProgress();
  if (msg.phase === "idle") {
    el.textContent = "Up to date";
    el.setAttribute("data-state", "idle");
    return;
  }
  if (msg.phase === "warming-kernel") {
    el.textContent = "Starting " + msg.lang + " kernel…";
    el.setAttribute("data-state", "busy");
    return;
  }
  el.textContent = "Executing " + msg.ran + "/" + msg.total;
  el.setAttribute("data-state", "busy");
}
```

- [ ] **Step 3: Add minimal CSS**

In the client style surface:

```css
#qmd-progress { position: fixed; bottom: 12px; right: 12px; z-index: 9999;
  font: 12px/1.4 var(--qmd-mono, monospace); padding: 4px 10px; border-radius: 6px;
  background: var(--qmd-bg, #fff); color: var(--qmd-fg, #222);
  border: 1px solid color-mix(in srgb, currentColor 25%, transparent); }
#qmd-progress[data-state="idle"] { opacity: .6; }
```

- [ ] **Step 4: Manual verify with chrome-devtools MCP**

Preview a multi-cell page: `QMD_FAST_PYTHON=… qmd-fast preview corpus/<a-code-heavy-page-or-book>`. Navigate with the chrome-devtools MCP, confirm the chip shows `Executing k/N` while building and `Up to date` after. Screenshot for the record.
Expected: chip updates live; no console errors.

- [ ] **Step 5: Commit**

```bash
git add web-client/client.js <client-css-file>
git commit -m "feat(progress): minimal in-browser k/N progress chip"
```

---

## Phase P1 — per-cell honest states + live timer

### Task 3: `cell-state` wire message + per-cell emission from `plan()` zones

**Files:**
- Modify: `crates/server/src/protocol.rs` (add `cell_state` + test)
- Modify: `crates/server/src/exec.rs` (emit per-cell transitions in `compute_outputs`)

**Interfaces:**
- Produces: `pub fn cell_state(page: Option<&str>, cell_id: &str, state: &str, started_ms: Option<u64>, duration_ms: Option<u64>) -> String`.
- Consumes: the cell ids already used for output blocks (`{cell.id}-out`); the three zones from `plan()` (warm-prefix `[0,shared)`, run-range `[shared,run_end)`, cached-tail `[run_end,len)`).

- [ ] **Step 1: Failing protocol test**

```rust
#[test]
fn cell_state_includes_state_and_optional_timing() {
    let s = super::cell_state(Some("p.qmd"), "abc", "running", Some(1000), None);
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["type"], "cell-state");
    assert_eq!(v["cell_id"], "abc");
    assert_eq!(v["state"], "running");
    assert_eq!(v["started_ms"], 1000);
    assert!(v.get("duration_ms").map_or(true, |d| d.is_null()));
}
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p qmd-fast-server protocol::tests::cell_state_includes_state_and_optional_timing`
Expected: FAIL.

- [ ] **Step 3: Implement `cell_state`**

```rust
/// `cell-state`: per-cell execution state. `state` is one of
/// "queued" | "running" | "done" | "error". `started_ms`/`duration_ms` are epoch
/// millis / elapsed millis when known; the client ticks the live timer itself.
pub fn cell_state(page: Option<&str>, cell_id: &str, state: &str,
                  started_ms: Option<u64>, duration_ms: Option<u64>) -> String {
    serde_json::json!({
        "type": "cell-state", "page": page, "cell_id": cell_id,
        "state": state, "started_ms": started_ms, "duration_ms": duration_ms
    }).to_string()
}
```

- [ ] **Step 4: Run, verify pass.** `cargo test -p qmd-fast-server protocol::tests::cell_state_includes_state_and_optional_timing` → PASS.

- [ ] **Step 5: Emit transitions in `compute_outputs`**

In `exec.rs` `compute_outputs`, using the cell id for each `CellRef` (the same id used to build the `-out` block):
1. Immediately after computing the zones, emit `cell_state(page, id, "done", None, None)` for every cell in the warm-prefix and cached-tail (restored, already done), and `cell_state(page, id, "queued", None, None)` for every cell in the run-range.
2. In the per-cell run loop, before `kernel.execute().await`: capture `let t0 = now_ms();` and emit `cell_state(page, id, "running", Some(t0), None)`.
3. After it returns: emit `cell_state(page, id, "done", Some(t0), Some(now_ms()-t0))` on success, or `cell_state(page, id, "error", Some(t0), Some(now_ms()-t0))` when the output is uncacheable/`qmd-error` (reuse `is_uncacheable`).

Add a small `fn now_ms() -> u64` helper (`SystemTime::now().duration_since(UNIX_EPOCH)`). Emission must be ordered queued → running → done|error per cell (monotonic); never emit `running` without a prior `queued`.

- [ ] **Step 6: Integration test — ordered states, no collapse**

Extend `crates/server/tests/progress_build_state.rs` (or a sibling) to capture `cell-state` messages for a 3-cell doc and assert: every executed cell emits `queued` then `running` then `done` in that order; an intentionally-erroring cell emits `error` (not `done`); a re-run after editing only the last cell emits `done` (cache/warm) for the earlier cells and `queued`→`running`→`done` for the last only.

- [ ] **Step 7: Run suite.** `cargo test -p qmd-fast-server` → PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/server/src/protocol.rs crates/server/src/exec.rs crates/server/tests/progress_build_state.rs
git commit -m "feat(progress): emit per-cell queued/running/done/error states"
```

### Task 4: per-cell decoration + live timer in the client

**Files:**
- Modify: `web-client/client.js` (handle `cell-state`; decorate `{cell_id}-out`; local timer)
- Modify: client CSS surface (state border + badge styles)

**Interfaces:**
- Consumes: `cell-state` messages; `elById()` (existing); the `{cell_id}-out` block id convention.
- Produces: a `applyCellState(msg)` function + a single shared `setInterval` ticking running timers.

- [ ] **Step 1: Add the `cell-state` case**

```js
case "cell-state":
  applyCellState(msg);
  break;
```

- [ ] **Step 2: Implement `applyCellState` + timer**

```js
var runningTimers = {}; // cell_id -> started_ms
function fmtElapsed(ms) { return (ms / 1000).toFixed(1) + "s"; }
function applyCellState(msg) {
  var out = elById(msg.cell_id + "-out") || elById(msg.cell_id);
  if (!out) return;
  out.setAttribute("data-qmd-cell-state", msg.state);
  var badge = out.querySelector(":scope > .qmd-cell-badge") || (function () {
    var b = document.createElement("span"); b.className = "qmd-cell-badge";
    out.insertBefore(b, out.firstChild); return b;
  })();
  if (msg.state === "running") {
    runningTimers[msg.cell_id] = msg.started_ms || Date.now();
    badge.textContent = "⏳ 0.0s";
  } else {
    delete runningTimers[msg.cell_id];
    if (msg.state === "done") badge.textContent = "✓ " + (msg.duration_ms != null ? fmtElapsed(msg.duration_ms) : "");
    else if (msg.state === "error") badge.textContent = "✕";
    else badge.textContent = "⏳"; // queued
  }
}
setInterval(function () {
  var now = Date.now();
  Object.keys(runningTimers).forEach(function (id) {
    var out = elById(id + "-out") || elById(id);
    if (!out) return;
    var b = out.querySelector(":scope > .qmd-cell-badge");
    if (b) b.textContent = "⏳ " + fmtElapsed(now - runningTimers[id]);
  });
}, 200);
```

- [ ] **Step 3: CSS for the four states**

```css
[data-qmd-cell-state] { border-left: 3px solid transparent; padding-left: 8px; }
[data-qmd-cell-state="queued"]  { border-left-color: color-mix(in srgb, currentColor 30%, transparent); opacity: .7; }
[data-qmd-cell-state="running"] { border-left-color: #4c8dff; }
[data-qmd-cell-state="done"]    { border-left-color: #2bb673; }
[data-qmd-cell-state="error"]   { border-left-color: #cc3333; }
.qmd-cell-badge { font: 11px/1 var(--qmd-mono, monospace); opacity: .75; margin-right: 6px; }
[data-qmd-cell-state="running"] .qmd-cell-badge { animation: qmd-pulse 1s ease-in-out infinite; }
@keyframes qmd-pulse { 50% { opacity: .35; } }
```

(Respect `prefers-reduced-motion`: wrap the animation in `@media (prefers-reduced-motion: no-preference)`.)

- [ ] **Step 4: Guard against the `update` op clobbering state**

When an `update` op replaces a `{cell}-out` block (existing `case "update"`), the new node has no `data-qmd-cell-state`. That is correct (a fresh output means done); ensure `applyCellState`'s later `done` message re-decorates it, and that a running cell whose output streams in keeps its border until `done`. No code change expected; verify in Step 5.

- [ ] **Step 5: Manual verify (chrome-devtools MCP)**

Preview a code-heavy page; confirm cells show queued (dim) → running (blue, ticking timer) → done (green, duration); an erroring cell shows red. Screenshot.

- [ ] **Step 6: Commit**

```bash
git add web-client/client.js <client-css-file>
git commit -m "feat(progress): per-cell state borders + live elapsed timer"
```

---

## Phase P2 — chip polish, kernel-warm legibility, locate

### Task 5: idle/busy chip with k/N bar, click-to-scroll, tab-title/favicon

**Files:** Modify `web-client/client.js` + client CSS.

**Interfaces:** Consumes `build-state` + `cell-state`; tracks the active cell id (last `running`).

- [ ] **Step 1:** Track the active cell: in `applyCellState`, on `running` set `var activeCell = msg.cell_id;`. Add a small "jump" affordance to the chip that `scrollIntoView`s `elById(activeCell + "-out")` (or the erroring cell). Test code: a JS assertion is awkward; verify via chrome-devtools by clicking the chip and asserting scroll position changed.
- [ ] **Step 2:** Upgrade `updateProgress` to render a deterministic mini-bar: width `= ran/total`. On `idle`, show `Up to date, built in Xs` (compute from a build-start stamp set on the first non-idle `build-state`). Never render a percentage label, only `k/N` + the bar.
- [ ] **Step 3:** Flip `document.title` to `● building…` while busy and `⚠ error` on error; restore on idle. Set a favicon dot via a small canvas data-URI when busy/error (out-of-band for backgrounded tabs).
- [ ] **Step 4:** chrome-devtools verify: bar fills, click-to-scroll lands on the running/erroring cell, tab title flips. Screenshot.
- [ ] **Step 5:** Commit `feat(progress): idle/busy chip, k/N bar, click-to-scroll, tab-title/favicon`.

### Task 6: distinct kernel warm-up state

**Files:** Modify `web-client/client.js` (render `warming-kernel`); confirm server emits it (Task 1 Step 5).

- [ ] **Step 1:** In `updateProgress`, the `warming-kernel` phase shows `Starting <lang> kernel…` with its own busy style and a timer started at first receipt; do not show queued cells as "running" during warm-up.
- [ ] **Step 2:** Server: confirm `build_state(.., "warming-kernel", 0, total, lang)` is emitted at the lazy `ensure_kernel` boundary (Task 1) and that a cold all-cached page emits `idle` immediately (no warm-up, no spinner). Add an integration assertion: an all-cached rebuild emits exactly one `idle` and zero `running`.
- [ ] **Step 3:** chrome-devtools verify on a cold first edit: chip reads `Starting python kernel…` then transitions to `Executing k/N`. Screenshot.
- [ ] **Step 4:** Commit `feat(progress): distinct, timed kernel warm-up state`.

---

## Phase P3 — parallel page builds (default-on, memory-capped)

### Task 7: memory-budget concurrency cap helper

**Files:**
- Create: `crates/server/src/build_budget.rs` (+ register in the server module tree)
- Test: inline `#[cfg(test)]` in that file.

**Interfaces:**
- Produces: `pub fn concurrency_cap(jobs: Option<usize>, per_kernel_mb: u64) -> usize` returning `1` when `jobs == Some(1)`, the explicit value when `jobs == Some(n>1)`, and `min(physical_cores, max(1, free_mb / per_kernel_mb))` when `jobs` is `None`/`Some(0)` (auto).
- Consumes: a cores source (`std::thread::available_parallelism`) and a free-memory probe (read `/proc/meminfo` `MemAvailable` on Linux; fall back to a conservative constant elsewhere).

- [ ] **Step 1: Failing tests**

```rust
#[test] fn explicit_jobs_one_is_sequential() { assert_eq!(concurrency_cap_with(Some(1), 150, 16, 32_000), 1); }
#[test] fn explicit_jobs_is_respected()      { assert_eq!(concurrency_cap_with(Some(4), 150, 16, 32_000), 4); }
#[test] fn auto_caps_by_memory()             { assert_eq!(concurrency_cap_with(None, 1000, 16, 4_000), 4); }
#[test] fn auto_caps_by_cores()              { assert_eq!(concurrency_cap_with(None, 100, 8, 64_000), 8); }
#[test] fn auto_never_zero()                 { assert_eq!(concurrency_cap_with(None, 100000, 16, 10), 1); }
```

Where `concurrency_cap_with(jobs, per_kernel_mb, cores, free_mb)` is the pure inner fn; `concurrency_cap` wraps it with the real probes.

- [ ] **Step 2:** Run → FAIL (`cargo test -p qmd-fast-server build_budget`).
- [ ] **Step 3:** Implement `concurrency_cap_with` (pure arithmetic above) + `concurrency_cap` (probe cores/free-mem, delegate). `free_mb / per_kernel_mb` floored, clamped to `[1, cores]`.
- [ ] **Step 4:** Run → PASS.
- [ ] **Step 5:** Commit `feat(build): memory-aware build concurrency cap`.

### Task 8: concurrent page builds in the builder worker

**Files:**
- Modify: `crates/server/src/serve_site.rs` (`spawn_builder` and `ExecPool` interaction)
- Test: `crates/server/tests/parallel_build_determinism.rs`

**Interfaces:**
- Consumes: `concurrency_cap` (Task 7); the per-page `Executor` isolation (own kernel + cwd).
- Produces: a bounded concurrent drain of `build_tx` that builds independent dirty pages at once.

- [ ] **Step 1: Determinism test (the invariant)**

```rust
// Build the whole multi-page corpus book twice: once with jobs=1, once with
// jobs=cap. Assert each page's rendered _book/<page>.html is byte-identical
// between the two runs (modulo known nondeterministic bytes, e.g. matplotlib PDF
// timestamps — compare the HTML, which embeds PNG data URIs that ARE deterministic).
```

Use the existing site-build entry (`build .`) the CLI uses; confirm the function name in pre-flight. Gate on a kernel being available.

- [ ] **Step 2:** Run → FAIL (parallel path not implemented; or test infra missing).
- [ ] **Step 3: Implement concurrent drain**

Replace the serial loop in `spawn_builder` with a bounded scheduler: maintain a `tokio::task::JoinSet` (or a `Semaphore` of size `concurrency_cap(...)`); for each dirty page popped from `build_tx`, if it has no unbuilt dependency (see Task 9) spawn its build on the pool; await completion to free a slot. Each spawned build calls `ExecPool::get(rel)` for its own `Executor`. Ensure `ExecPool` access is concurrency-safe (it is behind a lock today; confirm the lock is not held across the `.await` of a build — if it is, refactor so the pool lookup is brief and the build runs without holding the pool lock). Keep `--jobs 1` behavior byte-identical to today.

- [ ] **Step 4:** Run determinism test → PASS. Also run the full corpus build manually and confirm no `_freeze` write races (run twice; second run should be all-cached/instant).
- [ ] **Step 5: File-isolation assertion**

Add a test/check that two pages writing the same relative filename (e.g. both `fig-export: figures/x.pdf`) do not clobber each other: each page's cwd is its own dir, so paths resolve under different dirs; assert both outputs exist and differ. If qmd-fast shares a cwd for sibling pages in one dir, give each build job a private temp cwd and copy outputs back (document the chosen approach).

- [ ] **Step 6:** Commit `feat(build): build independent pages concurrently, capped by memory`.

### Task 9: cross-page ordering edges

**Files:** Modify `crates/server/src/serve_site.rs` (dependency-aware scheduling); test in `parallel_build_determinism.rs`.

- [ ] **Step 1:** Identify cross-page dependencies that exist today (e.g. `listing:` pages that read sibling pages, search index, nav). Grep the site model for where one page consumes others. Write a test: a listing/index page's output reflects sibling pages, and must build after them even under `--jobs N`.
- [ ] **Step 2:** Run → FAIL if naive concurrency builds the index before its sources.
- [ ] **Step 3:** Implement: mark dependent pages and schedule them only after their sources complete (a simple two-tier: leaf pages first, dependent pages after; or a small ready-queue keyed on remaining deps). Independent leaves still run concurrently.
- [ ] **Step 4:** Run → PASS.
- [ ] **Step 5:** Commit `feat(build): honor cross-page ordering edges in parallel builds`.

### Task 10: `--jobs` CLI flag (default auto), decoupled from MAX_WARM_PAGES

**Files:** Modify the CLI arg parsing (confirm location: `crates/server/src/main.rs` or the `preview`/`build` subcommand parsing) + `serve_site.rs` (thread `jobs` to the scheduler) + docs (`docs/guide/...` and `--help` microcopy).

- [ ] **Step 1:** Add `--jobs <N>` (alias none) to `preview` and `build`; `0`/absent = auto (Task 7), `1` = sequential, `N` = explicit. Default = auto. Document in the subcommand `--help` and the guide. Decouple from `MAX_WARM_PAGES`: the warm-MRU set (fast revisits) stays at its current cap; build concurrency is the separate `concurrency_cap`. Add a one-line `log` at startup: `building with up to N parallel pages`.
- [ ] **Step 2:** Test: `--jobs 1` reproduces sequential output (already covered by Task 8 determinism); add a CLI-parse unit test for `0/1/N/absent`.
- [ ] **Step 3:** Run → PASS. Manually build the book and confirm wall-clock drops vs `--jobs 1` on the 8-core machine; note the speedup in the commit body.
- [ ] **Step 4:** Commit `feat(cli): --jobs for parallel page builds (default auto, memory-capped)`.

---

## Phase P4 — eager forkserver kernel warm-pool

### Task 11: forkserver warm-pool spawner with fallback

**Files:** Modify `crates/server/src/kernel.rs` (an alternate spawn path) + a small Python helper (embedded string or `crates/server/assets/`).

**Interfaces:**
- Produces: a `WarmPool` that hands out pre-warmed Python kernels (`fn take(&self) -> Option<Kernel>`), refilling in the background; falls back to the current direct `python -m ipykernel_launcher` spawn when forkserver is unavailable.
- Consumes: the existing `Kernel::start` connection/handshake (`kernel.rs`), reused for the forked children.

- [ ] **Step 1:** Write a Python spawner helper that uses `multiprocessing.get_context("forkserver")`, calls `set_forkserver_preload([... configurable: "numpy","matplotlib", and "torch" if importable ...])`, and on request forks a child that execs the ipykernel with a given connection file. Test the helper in isolation: a small `cargo test` (kernel-gated) that asks the pool for a kernel and runs `1+1` faster than a cold `Kernel::start` of a process that imports numpy (assert the warm one is ready and returns `2`).
- [ ] **Step 2:** Run → FAIL (pool not wired).
- [ ] **Step 3:** Implement `WarmPool`: at server start, pre-warm `min(2, cap)` Python kernels via the forkserver helper; `take()` returns one and triggers a background refill; if forkserver init fails (import error, non-Linux, R), log once and fall back to direct spawn so behavior is never worse than today.
- [ ] **Step 4:** Run → PASS (warm kernel ready; first-cell latency measurably lower; fallback path still works when preload is forced to fail).
- [ ] **Step 5:** Commit `feat(kernel): eager forkserver warm-pool with safe fallback`.

### Task 12: use the warm-pool + overlap boot, reconcile with build concurrency

**Files:** Modify `crates/server/src/exec.rs` (`ensure_kernel` takes from the pool) + `serve_site.rs` (pool sizing within the RAM budget).

- [ ] **Step 1:** `ensure_kernel(lang=="python")` takes from the `WarmPool` when available instead of cold-starting; emits the `warming-kernel` build-state only if it actually has to wait (a ready pooled kernel → near-zero warm-up). Test: a page whose kernel came from the pool emits `executing` quickly with no long `warming-kernel` gap; an all-cached page still boots nothing.
- [ ] **Step 2:** Size the warm-pool within the same memory budget as build concurrency (Task 7): `warm_pool + active_build_kernels` must respect the RAM cap; pooled kernels count against the budget. Add a test for the combined cap (pure arithmetic, like Task 7).
- [ ] **Step 3:** Run → PASS. Manually confirm first-edit latency on a fresh page is near-instant on the 8-core/32 GB machine; note the before/after in the commit.
- [ ] **Step 4:** Commit `feat(kernel): serve builds from the warm-pool; overlap boot with rendering`.

---

## Self-Review (against the spec)

- **Spec coverage:** progress protocol (T1,T3) ✓; per-cell states+timer (T3,T4) ✓; global chip + k/N + click-to-scroll + tab-title (T2,T5) ✓; kernel-warm state (T1,T6) ✓; page-parallel default-on memory-capped (T7,T8,T10) ✓; file-isolation + ordering (T8,T9) ✓; eager forkserver warm-pool (T11,T12) ✓; determinism invariant test (T8) ✓; honest-no-ETA rule enforced in T2/T5 copy ✓. Non-goals (intra-page parallelism, ETA, multi-kernel reconciliation) are absent by construction ✓.
- **Placeholder scan:** test code and protocol/client code are shown verbatim; Rust-internal steps that depend on unread live code give the exact signature + behavior + the test that pins them, with the PRE-FLIGHT note to confirm against the live tree (a concurrent session is editing). No "TBD"/"add error handling"/"similar to" left in.
- **Type consistency:** `build_state(page, phase, ran, total, lang)` and `cell_state(page, cell_id, state, started_ms, duration_ms)` are used identically in protocol, server emit, and client `handle()`; `concurrency_cap`/`concurrency_cap_with` names match across T7/T8/T12.

## Commit / integration note

Do **not** rebase-and-force or commit unrelated working-tree changes: at planning time another session had uncommitted edits to `crates/core/assets/css/base.css`, `scrolly.js`, `walkthrough.js`. Coordinate before committing; each task above commits only its own named files.
