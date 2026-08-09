# AP3: concurrency and race conditions (2026-07-25)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Perspective:** AP3 from `backlog.md`'s "Audit perspectives", picked by the owner after AP7 ran
the same day. **Run solo** against the tip of `backlog/backlink-context-and-resume` (`cca5395`),
release binary, real `preview` server with a real warm Python kernel. **Nothing was changed**:
findings only, and the M6a freeze on `exec_pool.rs` was respected (observe, do not retune).

## Headline

**The concurrency model is safe by construction, and what it buys with that safety is
head-of-line blocking.**

AP3's entry predicted logic races: "save-while-executing, file-change-mid-build, two clients on one
preview, concurrent freeze writes, eviction interleaving". Measured and read, **none of those is a
race**, because the server does not actually run builds concurrently. There is exactly **one**
builder task consuming a single channel for the whole server (the root site *and* every mounted
project), the `ExecPool` is owned by that task rather than shared behind a lock, and freeze writes
already use a per-writer temp plus an atomic rename.

The cost is the thing nobody measured: **a page with no code cells at all takes 110x longer to
hot-reload when an unrelated page is executing** (0.11s to 12.15s, measured). That is not a race,
it is a queueing property, and it is the finding worth acting on.

## The entry's own premises, re-measured first

| Premise | Verdict |
|---|---|
| "save-while-executing" / "file-change-mid-build" races | **Refuted as a race.** Builds are strictly serialized: `serve_site/mod.rs:1028` is `while let Some(msg) = build_rx.recv().await { … build_page_guarded(…).await }`, a single-consumer loop. A save during a build is queued, not interleaved. |
| "two clients on one preview" races | **Refuted as a race.** Both clients' saves land in the same queue and are processed one at a time. |
| "concurrent freeze writes" | **Refuted, already solved.** `freeze.rs:228-246` writes to a temp path *unique per writer* (not a fixed `<page>.json.tmp`) then `rename`s it into place, and the source comments give exactly this reasoning. Atomic, no corruption, no stale-hit path. |
| "eviction interleaving" on the `MAX_WARM_PAGES` LRU | **Refuted as a race.** `pools` is a plain `HashMap<ProjectKey, ExecPool>` owned by the builder task (`mod.rs:1016`); `ExecPool::get` takes `&mut self` and there is no lock, so no two evictions can interleave. The deterministic order the build relies on is preserved by single-threaded ownership, not by a lock. |
| "two `exec::tests` fail ~2 runs in 3 in a full `--bins` run" | **Refuted by measurement: 0 failures in 13 full runs.** See below. |

**The one premise that survives is the one the entry did not state:** with a single builder, latency
is shared. That is AP3-1.

## Verified findings, ranked

### AP3-1 (medium-high): one slow cell anywhere stalls hot reload everywhere

**Measured** on a two-page preview project: `slow.tmd` (one `{python}` cell that sleeps 12s) and
`fast.tmd` (prose only, **no code cells at all**), both open in real browser tabs with live
websockets, against a warm pool.

```
BASELINE  fast.tmd edited alone, nothing else building   ->  0.11s
CONTENDED fast.tmd edited 1.2s into slow.tmd's 12s cell  -> 12.15s
```

**110x.** The cell-free page needs no kernel, no execution and no cache: its rebuild is pure render.
It waits anyway, because `spawn_builder` (`mod.rs:1006-1053`) is one task serving one
`mpsc::UnboundedReceiver<BuildMsg>` for **every project the server hosts**, root and mounts alike,
and it `.await`s each `build_page_guarded` to completion before taking the next message.

**Why this is not a theoretical concern here.** The marketing site `mounts:` both dogfood books, and
the corpus carries genuinely slow cells (3D scenes, matplotlib, Bayesian fits). Editing prose in
`docs/guide` while any other open page is mid-execution stalls the *entire* preview, including
pages in a different project with a different `_freeze` and a different interpreter. The serialized
design is defensible for pages that need a kernel (they share one warm pool, and kernels are
80-150 MB each), but it currently serializes on the wrong predicate: **a page with no cells is
queued behind kernel work it will never use.**

*Scope honestly stated:* this is the preview dev loop, not a built page, and it degrades latency
rather than correctness. The final state is always right.

### AP3-2 (low, observation rather than defect): the build queue has no dedupe, but the block diff absorbs it

`build_tx` is a bare `mpsc::UnboundedSender<BuildMsg>` (`mod.rs:52`, `:282`) with no in-flight or
already-queued tracking, so each 80ms watcher debounce window that touches an open page enqueues
another `Build` for it. Five distinct edits to `fast.tmd` during one 12s build therefore queue five
rebuilds.

**But the user-visible cost is nil, which I measured before reporting it as a defect.** By the time
build 1 runs, the file on disk already holds the *last* edit, so builds 2..5 render byte-identical
HTML and the block-level diff emits no ops at all:

```
5 distinct edits queued during the slow build
-> server log: 1 `update` line for fast.tmd
-> final page shows MARKER_B_5 (the last edit): true
```

So the redundant rebuilds are silent and the final state is correct. What they still cost is CPU,
and per AP1 each one carries two full-site render passes. **I did not measure that wasted CPU**, so
this is filed as an observation with a known upper bound on its harm, not as a bug. It matters only
if AP3-1 is fixed by parallelising, at which point dedupe stops being free.

### AP3-3 (low): a test declared fixed and deterministic still flakes, 1 run in 13

`kernel::tests::kernel_executes_state_errors_and_interrupts_runaway_cell` failed **1 of 13** full
`--bins` runs (all three gates set, full parallelism). The backlog records this test as **fixed on
2026-07-25** with its cause identified as `OnceLock` memoization of `cell_timeout()`, replaced by a
per-kernel `cell_cap` "so it is deterministic regardless of test order".

That fix is real and the rate is clearly much lower than before, but it is **not zero**, so either
the cap has a second order-dependence or there is an unrelated timing edge in the interrupt path.
**I did not capture the assertion text**: the run that failed was under a harness that recorded only
the summary line, and the six runs under the detail-capturing harness all passed. The harness is in
the scratchpad and the next session can just loop it.

## Verified sound, do not re-audit

- **The freeze write path.** Unique-per-writer temp plus atomic `rename` (`freeze.rs:228-246`), with
  the reasoning already in the source comments. Two processes building the same page cannot corrupt
  each other.
- **`ExecPool` and the `MAX_WARM_PAGES` LRU.** Task-owned, `&mut self`, no lock anywhere. The
  eviction order is deterministic because nothing else can touch it. (Observed only; the M6a freeze
  was respected and nothing was retuned.)
- **The watcher debounce.** An 80ms window collecting paths into a `HashSet<PathBuf>`
  (`mod.rs:1367-1379`), so a burst of writes to one file is one batch. It also deliberately ignores
  the executor's own `_freeze/` writes, which would otherwise self-trigger a rebuild loop.
- **Builder panic isolation.** `build_page_guarded` wraps each build in `catch_unwind`, so one bad
  page cannot kill the shared builder task and silently stop hot reload for every page. Given the
  single-task design this is load-bearing, and it is already there.
- **Malformed-config safety.** A mid-edit save that leaves `_site.yml` transiently broken keeps the
  last-good `Site` rather than collapsing the preview (`mod.rs:1400-1395` region).

## Not chased

- **The wasted CPU of AP3-2's redundant builds** (needs instrumenting the builder, not a probe).
- **`ETXTBSY` in `write_exe`**, the backlog's leading hypothesis for the two `exec::tests`: it never
  reproduced, so there was nothing to diagnose. The recorded "cheap first move" (make `probe_version`
  log why it returned `None`) was therefore not spent; note that `probe_interp_id` only memoizes an
  *answer*, so a failed ask is genuinely retried and the 5s `interp_id_settled` loop already absorbs
  a transient exec refusal.
- **Multi-client stress at scale** (N > 2 tabs), and **kernel restart racing an in-flight build**.
- **The `--host` LAN path**, where two real machines could drive the same preview.

## Method

Release binary at `cca5395`. Flake rate: 13 full `cargo test --bins` runs with all three gates
(`TALIESIN_REQUIRE_NODE`, `TALIESIN_R`/`TALIESIN_REQUIRE_R`, `TALIESIN_PYTHON`/`TALIESIN_REQUIRE_KERNEL`)
at full parallelism. Latency: a purpose-built two-page project under a real `preview` with a warm
pool, driven by two live browser tabs via the project's own `puppeteer-core` (`tools/ui-audit`), with
the uncontended baseline measured separately so the contended number means something. Code read over
`serve_site/mod.rs`, `serve_site/exec_pool.rs`, `freeze.rs`, `exec.rs`. Probe scripts are in the
session scratchpad. No repo file was modified; the tree was verified clean after the run.

**One probe bug worth recording:** the first head-of-line run showed no blocking at all (3.25s for
both pages) because it edited the slow page's *heading*. The cell's cumulative hash was unchanged, so
the executor logged `restored 1 cached cell · 0 re-ran` and no slow work ever ran. **To force real
execution you must edit the cell body**, and the server log is what catches the mistake.
