# Audit: internal codebase health (perspective AP10)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

Date: 2026-07-23. Perspective: AP10 from the backlog "Audit perspectives" section
(internal codebase health) — the pure code-read, fan-out-safe lens. Run **alongside a live
parallel session** (the `ask-ai-handoff` feature session, actively editing
`crates/core/assets/js/code-enhance/19-ask-ai.js` in the shared tree), so it was chosen
precisely because it binds no ports, spawns no kernel, drives no browser, and edits no
source — it only reads. Write-up done in an isolated worktree off `origin/main`
(`audit/ap10-codebase-health`) so it touches neither the shared tree nor the feature
branch. Distinct from the 2026-07-17 reduction audit ("is the codebase lean?", answered
yes) and the 2026-07-18 vacuous-test audit ("do the tests constrain behavior?"): this pass
asks **which of the ~700 panic sites are reachable from user input, and which are behind a
recovery boundary.**

## Why this perspective

Non-test code carries ~708 `unwrap()`/`expect()`/`panic!`/`unreachable!` sites. AP2
(fuzzing, 2026-07-22) proved the **parse→render pipeline** is hardened: every *render* entry
on the `serve`/`serve_site`/`build` paths is wrapped in `catch_unwind` and runs on a 256 MB
worker stack, so 7,500 generative inputs produced 0 unexpected panics. But AP2's census of
"guarded render entries" was scoped to the dev-server + CLI-build paths. The unanswered
question: are there **other** entries that render/handle user input **without** that
boundary — and is any of the panic surface reachable there?

## Executive summary

**The codebase is healthy: dead code is essentially nil, and the panic surface is dominated
by guarded or structurally-unreachable sites.** The one real finding is a **coverage hole in
the panic-safety discipline, not in the tests**: the two *persistent stdio servers* — `lsp`
(the editor language server) and `mcp` (the agent tool server) — render and project user
documents inside their request loop with **no per-request panic boundary**, unlike the
`serve`/`build` paths AP2 verified, and unlike the LSP's *own* `render_buffer` helper, which
already wraps its render in `serve::guarded`. A catchable panic anywhere in the
diagnostics/resolve/projection path unwinds out of the loop and **kills the server for the
whole session** — for the LSP that silently takes down all editor intelligence (diagnostics,
completion, hover, go-to-definition, rename); for MCP it fails every subsequent agent tool
call. The LSP is the sharper instance: it renders the buffer on *every keystroke* and is the
most-recently-shipped surface (the E1-E7 editor-DevX initiative), fed the most continuous
adversarial input (cursor positions past EOF / mid-astral-char, rename params).

## Finding

### HEALTH-1 (medium): the persistent stdio servers (`lsp`, `mcp`) have no per-request panic boundary

**Root cause.**
- `lsp::main_loop` (`lsp.rs:93-104`) dispatches each message with
  `handle_request(...)?` / `handle_notification(...)?`. The `?` propagates a `Result` error,
  but a **panic** is not a `Result` — it unwinds straight through `main_loop` → `run` →
  `cmd_lsp` → process exit. There is no `catch_unwind` in the loop.
- `handle_notification` calls `publish()` on every `didOpen`/`didChange`, i.e. every
  keystroke. `publish()` (`lsp.rs`) calls `crate::check::buffer_diagnostics(&path, text)` —
  which **renders the buffer** and runs the full check/lint/xref-validate pipeline — with **no
  guard**, then `d.to_lsp(&lines)` (AP5-flagged Unicode-scalar column math). A panic in any of
  that crashes the language server.
- `mcp::cmd_mcp` (`mcp.rs:91-114`) is the same shape: `let outcome = handle(method, &req);`
  (`mcp.rs:105`) dispatches to tools that render/project user docs (`read`/`map`/`query`),
  **unguarded**; a panic exits the loop and the MCP server dies.

**Why this is the finding (inconsistency, not just absence).** The authors already know the
request loop must survive a malformed-buffer render panic — `render_buffer` (`lsp.rs:722-732`,
used by hover + completion) wraps its render in `crate::serve::guarded(...)` with the exact
comment *"Panic-guarded so a malformed buffer yields `None` rather than crashing the request
loop."* The guard is applied to the xref-registry render (hover/completion) but **not** to the
every-keystroke diagnostics render in `publish()`, nor to the nav paths
(`resolve_definition`/`resolve_prepare_rename`/`resolve_rename`), nor to `document_symbols`,
nor anywhere in `mcp`. The `serve`/`serve_site`/`build` entries are all guarded
(`serve/mod.rs:171,1480`, `serve_site/mod.rs:1019`, `build.rs:238`); the two stdio servers are
the outliers AP2's census structurally missed (AP2 fuzzed via build/serve, not the LSP/MCP
entry).

**Severity nuance (honest exploitability).** AP2 showed the parse→render *core* resists
fuzzing (0 catchable panics in 7,500 inputs), so a plain panic is not trivially triggered.
Two things keep this at *medium*, not low:
1. The layer **above** render is not fuzz-covered. `check::buffer_diagnostics` runs the
   diagnostics/lint/xref pipeline (`check.rs` has 57 panic sites) and `to_lsp` does
   scalar-based column math AP5 flagged as diverging on astral characters — neither was in
   AP2's parse→render scope. This is residual catchable-panic surface on the every-keystroke
   path.
2. AP2's two *real* defects crash/hang these servers and `catch_unwind` **cannot** fix either:
   **AP2-1** (deep `>` → ~900 KB stack-overflow **abort**, uncatchable) and **AP2-2**
   (balanced nested brackets → comrak O(n²) inline **hang**, never returns). On `serve` these
   degrade to a recovered 500 / a slow rebuild; on `lsp`/`mcp` they **kill a persistent server
   the editor/agent depends on**. So AP10 *raises the priority of AP2-1/AP2-2*: they are not
   just "dev-server 500s", they are "editor/agent-server death", and the LSP is the strongest
   argument for shipping the pre-parse depth guard + render watchdog.

**Build-ready fix (HEALTH-1):**
- Wrap the per-message dispatch in `lsp::main_loop` in `serve::guarded` (already the LSP's own
  pattern): a panicking **request** returns an error response / empty result (so the client
  doesn't hang), a panicking **notification** (e.g. `publish`) logs + skips (so the loop
  survives and the next edit re-tries). Same for `mcp`'s `handle` call → a JSON-RPC
  `internal error` response instead of process death.
- Pin it: a test that feeds a buffer engineered to panic the diagnostics/projection path and
  asserts the server **still answers the next request** (the resilience test that cannot exist
  today because the server actually dies). This is the coverage hole below, closed by the fix.
- Explicitly **out of scope for this fix** (route to AP2-1/AP2-2): the deep-`>` abort and the
  O(n²) hang — a catch_unwind boundary does not stop an `abort()` or a non-returning call.
  Note this in the fix so it isn't mistaken for full hardening.

Surface: `crates/server/src/lsp.rs` (the loop + `publish`) and `crates/server/src/mcp.rs`
(the loop); reuses the existing `crate::serve::guarded`. Touches no core render code, no
block model, no diff, and nothing in the Do-NOT-touch `exec_pool` LRU.

## Verified healthy (do not re-audit)

- **Dead code is essentially nil.** The whole non-test tree carries **2** `#[allow(dead_code)]`
  (`warm_pool.rs:677,685`) and no `#[allow(unused_*)]` sprawl — corroborating the 2026-07-17
  reduction audit's "the codebase is already lean". No dead modules, no unused-`pub` rot found.
- **The panic census is dominated by guarded/structural sites.** Of ~708 non-test
  `unwrap`/`expect`/`panic!`/`unreachable!`, the render/site core (`crates/core`) executes
  behind the `serve`/`build` `catch_unwind` guards + the 256 MB worker stack (AP2), and the
  bulk of the `unwrap`s are on just-checked invariants, lock acquisitions, or const indices.
  The user-reachable **unguarded** concentration is the two stdio servers (HEALTH-1).
- **LSP position math is defensively written.** `lsp_pos.rs` clamps an out-of-range line
  (`text.split('\n').nth(line).unwrap_or("")`) and rounds a UTF-16 offset that lands inside a
  surrogate pair *up* to the next char boundary, with tests
  (`a_utf16_offset_inside_a_surrogate_pair_rounds_to_a_char_boundary`). A cursor position past
  EOF or mid-astral-char yields a wrong-but-safe offset (the AP5 correctness divergence), not a
  panic.

## Coverage holes (behaviors with zero test — AP10's distinct deliverable)

- **No panic-resilience test for either stdio server.** It cannot exist while the loop is
  unguarded (the server would actually die under the test), so the HEALTH-1 fix and its pin are
  the same work item.
- **One-shot CLI subcommands are unguarded too** (`check`, `read`, `map`, `blocks`, `doctor`,
  `render`, `schema`, `symbols`, `vocab` — dispatched bare in `main.rs:56-69`), but a panic
  there is a single non-zero process exit (a crash, not a session-killer), so it is *lower*
  severity than the persistent servers. Noted, not a headline; the guarded one is `build`
  (`build.rs:238`).
- **The check/`to_lsp`/projection layer is not fuzz-covered.** AP2 fuzzed parse→render; the
  diagnostics + LSP-column + agent-projection layer above it has no hostile-input coverage — a
  natural AP2-followup that HEALTH-1's pin only partially touches.

## False leads (refuted — "trust the symptom, re-derive the cause")

- **"LSP position conversion panics on a bad cursor position"** — REFUTED. `lsp_pos.rs` is
  defensive and tested (clamp + surrogate-pair rounding). The encoding divergence AP5 found is
  a wrong-offset *correctness* issue, not a panic.
- **"Dead code / module sprawl is a health problem"** — REFUTED. 2 allows, no sprawl; the
  codebase is lean.

## Residuals not chased (for a future AP10-style pass)

- A **per-site reachability classification** of all ~708 panic sites (this pass sampled the
  high-risk unguarded surfaces and confirmed the concentration is the two stdio servers; a full
  mechanical census remains).
- A **module-coupling metric** (afferent/efferent, cycle detection): only dead-code was
  measured, not the coupling graph.
- The **kernel/exec panic paths** (`kernel.rs` 27, `warm_pool.rs` 31, `exec.rs` 35) — most are
  behind the exec/kernel guards from the M-audit + `warm_pool.rs:1431`'s `catch_unwind`, but a
  reachability pass on the ZMQ/spawn error handling was not done here (overlaps AP11 chaos).

## Method notes for the next AP10-style run

- The fast path to "which unwraps are reachable": don't read all 708. Map the **recovery
  boundary** first (`grep catch_unwind`), then census panic sites **per entry point** and ask
  "is this entry inside a guard?" The finding is always at an *unguarded* entry that touches
  user input — here, the two stdio server loops.
- Cross-reference AP2: its "guarded render entries" list is the guard map; an entry it did
  *not* list (the LSP/MCP `publish`/`handle` render) is exactly where a fuzzing-proven-safe
  core becomes unsafe again for lack of the boundary.
- Run it as a pure code-read when other sessions are live: it needs no port/kernel/browser, so
  it is the correct audit to pick under contention (which is why it was picked this session).
