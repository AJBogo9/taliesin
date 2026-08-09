# Post-cut remediation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development`
> (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.
>
> **TRUST THE GIT LOG OVER THE CHECKBOXES.** R1 (`7f53bf8a`), R2 (`819b7a7e`), R3 (`57e06a6a`)
> and R4 (`169b1ca7`) all landed on 2026-08-09 with their boxes still unticked; R5 landed as
> `17c4bf47` and says so in its own section. Ticking them retroactively would be guesswork
> about which sub-steps shipped as written, and R5's own header records two that did not.

**Goal:** Close the 17 defects the 2026-08-09 post-cut audit confirmed, close the finish
gaps the cut left behind, and then remove the residue the campaign did not reach — without
re-opening the scope ruling.

**Architecture:** Six waves. The first repairs the *verification instrument* itself, because
`./tools/gates.sh` currently claims completeness it does not have and every later wave's
"green" depends on it. Then two waves of behaviour fixes (test-first), one pure-prose
sweep, one finish wave, and finally the deletions, which are the only part that inherits the
cut campaign's wave discipline unchanged.

**Tech stack:** Rust 2024 (workspace resolver 3), tokio + axum, comrak, ZMQ/Jupyter,
vanilla JS clients, `.tmd` corpus as the regression net.

**Plan location note.** The `writing-plans` skill defaults to `docs/superpowers/plans/`. This
plan lives in `notes/` instead, because deleting `docs/superpowers/` **is item R6-1 of this
plan** and a plan that deletes its own directory is a trap. `notes/` is also where this
project's durable state already lives (`CUT-PROGRESS.md`, the ruling, the playbook).

---

## Global constraints

Every task's requirements implicitly include this section.

- **Scope is closed.** The audit found the MVP *shape* right. This plan fixes and deletes;
  it adds no capability. The only two additions are four lines of content in a string
  constant (R5-1) and a CSS rule *or* its removal (R5-3).
- **The standing directive still governs:** *"always lean towards cutting."* When a call
  inside a task is close, cut.
- **The ordering rule still governs:** a corpus pin and its docs page are deleted in the
  **same commit** as their feature, never before. `crates/core/tests/corpus.rs` sweeps
  whatever exists, so deleting a document removes coverage without failing a test.
- **A retirement costs ONE register entry.** Do not write a tombstone test
  (`RETIRED_KEYS` / `RETIRED_DIV_CLASSES` / `RETIRED_COMMANDS` / `RETIRED_FLAGS` /
  `RETIRED_NEW_KINDS` are all derived).
- **One branch and one commit per wave.** Named `fix/r<N>-<slug>`.
- **The gate is `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh`.** Bare
  `gates.sh` exits 2 at preflight on this machine. Run it on the tree you are about to
  commit, not on the tree you started from.
- **After wave R1 only:** gates.sh is the whole gate. **Before R1 lands**, also run
  `taliesin build docs/guide --check-only --no-exec` and `tools/build-site.sh --check`
  by hand, because gates.sh does not.
- **`crates/core/assets/*` are `include_str!`-compiled.** A CSS/JS edit needs
  `cargo build` before a rebuilt site shows it. A live `preview` hot-swaps CSS.
- **Do not touch the one standing freeze:** `MAX_WARM_PAGES` and the deterministic LRU
  order in `crates/server/src/serve_site/exec_pool.rs`. R2-1 adds a field *around* the
  pool; it must not reorder eviction.
- **Before honouring any "must survive" note in this plan or elsewhere, check the file
  exists.** Wave 12 of the cut campaign found eight spent justifications in one wave.
- **`stdout` is the LSP's JSON-RPC wire.** Never `println!` in `crates/server/src/lsp*.rs`;
  use `crate::log` (stderr).

---

## Wave order, and why it is this order

| Wave | Name | Kind | Why here |
|---|---|---|---|
| **R1** | Make the gate honest | tooling | Everything after it is verified by this instrument. Doing it second means re-verifying wave R2. |
| **R2** | The execution-loop defects | behaviour, test-first | The only capability hole in the tool, plus the worst first-run experience. Highest user impact. |
| **R3** | The cache path and the `{js}` asset gate | behaviour, test-first | Two correctness bugs. Independent of R2, so it may run in parallel on a second branch. |
| **R4** | The truth sweep | prose only | Zero behaviour change, so it must not be mixed with R2/R3 — a prose diff hiding inside a behaviour diff is how the campaign lost track of claims. |
| **R5** | Close the finish gaps | small features | Changes what a first user sees. R5-2 also changes the arithmetic on the mermaid decision, so it precedes R6. |
| **R6+** | Deletions | removal | Inherits the cut campaign's discipline unchanged. Nothing here blocks release. |

**R2 and R3 are independent** and may be worked concurrently on separate branches. Every
other pair has a real dependency.

---

## Wave R1 — Make the gate honest

**Branch:** `fix/r1-gate-completeness`

**The defect.** `CLAUDE.md` says *"`./tools/gates.sh` runs every gate in one process and
refuses to be green unless every one of them actually ran."* It runs 8 of the repo's 10
enforced gates: `build docs/guide --check-only` (added to pre-push in wave 9) and
`tools/build-site.sh --check` (added in wave 11) exist only in `.githooks/pre-push`.
`crates/core/tests/gate_script.rs` never compares the two lists, so nothing notices.

**Files:**
- Modify: `tools/gates.sh` (add two gate stanzas + two `PASSED` entries)
- Modify: `crates/core/tests/gate_script.rs` (add the cross-check)
- Modify: `.githooks/pre-push` (delegate steps 4–5 rather than duplicate them)

**Interfaces:**
- Produces: a `gates.sh` whose `PASSED` list is a superset of pre-push's steps, and a test
  named `every_pre_push_step_is_also_a_gate_script_gate` that fails if they diverge.

- [ ] **Step 1: Write the failing test**

In `crates/core/tests/gate_script.rs`, add:

```rust
/// `gates.sh` advertises itself as the ONE script that runs every gate, and CLAUDE.md
/// instructs every session to trust it. Two gates the cut campaign ADDED (wave 9's document
/// gate, wave 11's composition gate) landed only in `.githooks/pre-push`, so the script was
/// green while covering 8 of 10. Nothing compared the two lists. This does.
#[test]
fn every_pre_push_command_is_also_run_by_the_gate_script() {
    let hook = std::fs::read_to_string(repo_root().join(".githooks/pre-push"))
        .expect("the pre-push hook is committed");
    let gates = std::fs::read_to_string(repo_root().join("tools/gates.sh"))
        .expect("the gate script is committed");

    // The load-bearing invocations, spelled as they appear in the hook.
    const REQUIRED: &[&str] = &["--check-only", "build-site.sh"];

    let mut checked = 0usize;
    for needle in REQUIRED {
        assert!(
            hook.contains(needle),
            "{needle} is no longer in the pre-push hook; if it was deliberately \
             removed, remove it from REQUIRED here too"
        );
        assert!(
            gates.contains(needle),
            "`.githooks/pre-push` runs `{needle}` and `tools/gates.sh` does not, \
             so `gates.sh` reports PASSED while covering less than the hook"
        );
        checked += 1;
    }
    // Anti-vacuity: measured at 2 on 2026-08-09. A rewrite that empties REQUIRED
    // must fail here rather than pass silently.
    assert!(checked >= 2, "the cross-check collected {checked} commands, expected >= 2");
}
```

- [ ] **Step 2: Run it and watch it fail**

```sh
cargo test -p taliesin-core --test gate_script every_pre_push_command -- --nocapture
```

Expected: FAIL on `--check-only`, with *"`.githooks/pre-push` runs `--check-only` and
`tools/gates.sh` does not"*.

- [ ] **Step 3: Add the two gates to `tools/gates.sh`**

Append two stanzas after the existing `cargo deny check` gate, following the file's own
`PASSED+=` / `FAILED+=` convention exactly (copy the shape of the neighbouring gate, do not
invent a new one). Gate 9 runs
`cargo run --release -p taliesin-server -- build docs/guide --check-only --no-exec`;
gate 10 runs `./tools/build-site.sh --check`. Both must add their name to `PASSED` on
success and to `FAILED` on any non-zero exit. Update the script's own header comment and
the `════ gates ════` summary so the count it prints matches what it runs.

- [ ] **Step 4: Make `.githooks/pre-push` delegate instead of duplicate**

Replace hook steps 4 and 5 with a comment pointing at the gate script, so the two lists
cannot drift again by construction. Keep the hook's early steps as they are.

- [ ] **Step 5: Run the full gate on the tree you are about to commit**

```sh
TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh
```

Expected: `PASSED — every gate ran and passed`, now naming **10** gates. Record the suite
count; it should still be 81 suites / 1,377 passed plus the one test added here.

- [ ] **Step 6: Fix the CLAUDE.md paragraph that was wrong (finding 14, gate half)**

`CLAUDE.md:335` says the script *"arms all four `TALIESIN_REQUIRE_*` variables"* and names
the R and headless-Chrome gates, both deleted in wave 6. It arms two. Correct the sentence
to the measured truth and name the NODE gate it omits. **Do not write the retired variable
names in full anywhere in the repo** — `gate_script.rs`'s REQUIRE scan reads raw source
text, including comments, and a full spelling puts a dead gate back into the armed set. Say
"the R gate", not the variable.

- [ ] **Step 7: Commit**

```sh
git add tools/gates.sh .githooks/pre-push crates/core/tests/gate_script.rs CLAUDE.md
git commit -m "fix: gates.sh runs every gate it claims to, and a test says so"
```

---

## Wave R2 — The execution-loop defects

**Branch:** `fix/r2-exec-loop`

Two defects that share one subsystem, so they share one wave.

### Task R2-1: A runaway cell can be interrupted again

**The defect (audit finding 01, reproduced).** `web-client/client.js` sends
`{"type":"restart_kernel"}`; `serve_site/mod.rs:892–895` turns it into `BuildMsg::Restart`
on `app.build_tx`; the sole consumer is the **serial** loop at `serve_site/mod.rs:966`,
which awaits `build_page_guarded` before receiving the next message. So the restart queues
behind the very build it exists to abort. Verified live: the client sent the message and
the server logged nothing for 45 s+. With `TALIESIN_CELL_SILENCE` at its 600 s default, the
only recovery is killing the server.

**The mechanism to use, verified in source, not invented.** `kernel.rs:1071`
`pub(crate) fn interrupt_pid(pid: u32)` already exists and already does exactly this job —
SIGINT, which raises `KeyboardInterrupt` in the running cell while the warm kernel and every
prior cell's variables survive. Its one caller today is the silence cap, *inside* the polling
loop. Wave 13 deleted `Kernel::pid()`, the accessor that let an interrupt arrive from
outside that loop, on the correct observation that it then had no caller. This task gives it
one again, through a side channel rather than a public accessor.

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs` (add `SiteApp.interrupt`, wire the ws arm)
- Modify: `crates/server/src/serve_site/exec_pool.rs` (thread the handle to the executor)
- Modify: `crates/server/src/exec.rs` (publish/clear the pid around each cell)
- Modify: `crates/server/src/kernel.rs` (expose the running pid to the executor)
- Test: `crates/server/src/exec.rs` (in-file `#[cfg(test)]`, beside the existing
  interrupt canary)

**Interfaces:**
- Produces: `SiteApp.interrupt: Arc<AtomicU32>` — the pid of the cell currently executing,
  `0` when none. Written by the executor around each cell, read by the websocket handler.
- Consumes: `kernel::interrupt_pid(pid)` as it already exists.

- [ ] **Step 1: Write the failing test**

Add beside the existing `an_interrupt_stops_the_whole_run_and_keeps_the_warm_state`. Learn
from that test's own recorded flake: **do not** race on a timer. Have the long cell announce
itself by writing a file from inside its own body, and interrupt on that file appearing.

```rust
/// The dev menu's "Restart kernel" must be able to reach a cell that is ALREADY RUNNING.
/// Before this, the request was queued on the build channel behind the build it meant to
/// abort, so it was a no-op in exactly the situation it exists for (audit finding 01).
#[tokio::test]
async fn an_interrupt_from_outside_the_run_loop_stops_the_running_cell() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("cell-2-started");
    let interrupt = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

    let mut ex = Executor::new(dir.path().join("_freeze"), python_for_test());
    ex.set_interrupt_handle(interrupt.clone());

    let blocks = cells(&[
        "warm = 41",
        &format!("open(r'{}', 'w').close()\nimport time\ntime.sleep(120)", marker.display()),
    ]);

    let handle = tokio::spawn(async move { ex.run(blocks).await });

    // Cell 2 announces itself from inside its own body: no timing window at all.
    while !marker.exists() {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let pid = interrupt.load(std::sync::atomic::Ordering::SeqCst);
    assert_ne!(pid, 0, "the executor must publish the running kernel's pid");
    crate::kernel::interrupt_pid(pid);

    let out = tokio::time::timeout(std::time::Duration::from_secs(20), handle)
        .await
        .expect("the interrupt must end the run well inside the silence cap")
        .unwrap();

    assert!(
        rendered(&out).contains("KeyboardInterrupt"),
        "the interrupted cell reports the interrupt"
    );
}
```

- [ ] **Step 2: Run it and watch it fail**

```sh
TALIESIN_PYTHON="$PWD/.venv/bin/python" \
  cargo test -p taliesin-server exec::tests::an_interrupt_from_outside -- --nocapture
```

Expected: FAIL to compile — `set_interrupt_handle` does not exist.

- [ ] **Step 3: Publish the running pid from the executor**

Add to `Executor`: a `interrupt: Option<Arc<AtomicU32>>` field and

```rust
/// Publish the currently-executing kernel's pid so an interrupt can arrive from OUTSIDE
/// the run loop. The silence cap interrupts from inside the polling loop and needs no
/// handle; the dev menu's "Restart kernel" arrives on the websocket task while this task
/// is blocked in `run`, which is the whole reason this channel exists (audit finding 01).
pub fn set_interrupt_handle(&mut self, handle: Arc<AtomicU32>) {
    self.interrupt = Some(handle);
}
```

In `run`, store the kernel's pid before each cell executes and store `0` after it returns.
Use `Ordering::SeqCst`. The pid comes from the same place `Kernel::interrupt` already reads
it (`self.proc.id()`); expose it to the executor as a `pub(crate) fn running_pid(&self) ->
Option<u32>` on `Kernel` rather than re-deriving it.

- [ ] **Step 4: Run the test again**

Expected: PASS. Also re-run the existing canary
`kernel_executes_state_errors_and_interrupts_runaway_cell` and confirm it still passes —
the silence cap must be untouched.

- [ ] **Step 5: Wire the websocket handler**

Add `interrupt: Arc<AtomicU32>` as a fourth field on `SiteApp` (`serve_site/mod.rs:41`),
created in the same place `build_tx` is, and handed to the `ExecPool` so each `Executor` it
makes gets it via `set_interrupt_handle` (`exec_pool.rs`'s `make`, beside the existing
`set_progress` precedent — **do not touch the eviction loop or `MAX_WARM_PAGES`**).

Then change the `is_restart_kernel` arm to interrupt *first*, then queue:

```rust
if is_restart_kernel(t.as_str()) {
    // SIGINT the running cell before queueing, or the Restart waits behind the very
    // build it is meant to abort: the builder is serial and awaits each page to
    // completion (audit finding 01). A pid of 0 means nothing is executing, and the
    // queued Restart alone is then the whole action.
    let pid = app.interrupt.load(std::sync::atomic::Ordering::SeqCst);
    if pid != 0 {
        crate::kernel::interrupt_pid(pid);
    }
    let _ = app.build_tx.send(BuildMsg::Restart(rel.clone()));
}
```

- [ ] **Step 6: Correct `interrupt_pid`'s doc comment**

It currently says *"Its one caller is the silence/wall-clock cap inside the polling loop."*
That becomes false in this commit. State both callers and what distinguishes them. This is
the class of doc-drift the audit found seventeen instances of; do not add an eighteenth.

- [ ] **Step 7: Verify by hand in a browser, because no automated net covers this**

```sh
# a scratch project with `while True: time.sleep(0.5)` in a cell
TALIESIN_PYTHON="$PWD/.venv/bin/python" cargo run --release -p taliesin-server -- preview <scratch> 4390
```

Open it, click **Restart kernel**, and confirm the server log shows the run ending and a
rebuild starting within a second or two. Record the observed log lines in the commit body.

- [ ] **Step 8: Commit** — hold until R2-2 lands, one commit per wave.

### Task R2-2: A slow first build shows the page instead of a blank screen

**The defect (audit finding 02, reproduced).** On a page that has never been built,
`build_page` renders the markdown, then awaits `exec.run(...)` for **all** cells, and only
publishes to `ps.doc.blocks` and broadcasts at the very end. The websocket's opening
snapshot is `full_render_json(&ps.doc)` over an empty doc. Measured: a page with one
25-second cell showed a bare navbar for 20 seconds, withholding static prose that needed no
kernel, with no spinner or status. Wave 11 recorded its accepted cost as *"a
`warming-kernel` state on the first cell"*; what ships is no state at all.

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs` (`build_page`, publish pre-exec)
- Test: `crates/server/tests/` — new file `preview_publishes_before_exec.rs`

**Interfaces:**
- Consumes: the existing `ps.doc.generation` re-mount machinery, which already anticipates
  this case — `serve_site/mod.rs:1188` says *"so a client that server-rendered this page
  pre-exec re-mounts to pick up the outputs."*
- Produces: no new public API. One extra broadcast per first build.

- [ ] **Step 1: Write the failing test**

Drive a real preview over its websocket. Assert that a snapshot containing the page's static
prose arrives **before** the cell's output does, on a cell slow enough that the two cannot
be confused (a 5-second sleep, with a 3-second assertion deadline).

- [ ] **Step 2: Run it and watch it fail**

Expected: FAIL — the first body-bearing message arrives only after the cell completes.

- [ ] **Step 3: Publish the pre-exec blocks**

In `build_page`, immediately before `doc.blocks = exec.run(...).await`, and **only when the
page has no blocks yet** (a first build — a warm edit already has a body on screen and a
second publish there would flash), write the pre-exec blocks into `ps.doc.blocks` and
broadcast them. Cells render as source, exactly as `--no-exec` already does, so the shape is
already supported end to end. Leave the post-exec publish exactly as it is; the diff between
the two is what turns source into outputs.

- [ ] **Step 4: Run the test** — expected PASS.

- [ ] **Step 5: Verify by hand**

Re-run the 25-second-cell scratch project. Expected: prose and cells-as-source appear
immediately; the output appears when the cell finishes.

- [ ] **Step 6: Update the wave-11 note in `notes/CUT-PROGRESS.md`**

Its accepted-cost entry describes a `warming-kernel` state that was not what shipped. State
what shipped and what this commit changed. The campaign's log is the handoff; leave it true.

- [ ] **Step 7: Commit the wave**

```sh
git add -A
git commit -m "fix: an interrupt reaches a running cell, and a slow first build shows the page"
```

---

## Wave R3 — The cache path and the `{js}` asset gate

**Branch:** `fix/r3-freeze-and-js-assets` (independent of R2; may run concurrently)

### Task R3-1: `build <file.tmd>` replays the project's cache

**The defect (audit finding 03, reproduced).** `build.rs:321–323` sets `base` to the file's
own parent and `build.rs:683–684` roots the freeze cache at `base.join("_freeze")`, with no
project resolution — `cmd_build` branches only on `is_dir()`. Meanwhile `preview` resolves a
single file to its enclosing `_site.yml` project (wave 1.1) and roots the cache there. So:

```
build <project>              → fz/_freeze/posts/p.json
build <project>/posts/p.tmd  → fz/posts/_freeze/  (a SECOND cache)   exec cell 1/1
```

It re-executes, and drops a stray `_freeze/` in a project subdirectory that nothing sweeps.
This makes the wave-13 `run` retirement note (`main.rs:168`) false where it promises *"a
later `build` still replays without one"* — the note the campaign shipped as its
justification for cutting the verb.

**Files:**
- Modify: `crates/server/src/build.rs` (resolve the project before choosing the freeze root)
- Test: `crates/server/tests/freeze_cold_replay.rs` (extend, do not create — it already
  drives builds and owns this subject)

- [ ] **Step 1: Write the failing test** in `freeze_cold_replay.rs`: build a two-page
      project, then build one of its pages by path, and assert stdout reports restored
      cached cells and that no second `_freeze/` directory was created.
- [ ] **Step 2: Run it and watch it fail** — expected: `exec cell 1/1` and a stray dir.
- [ ] **Step 3: Resolve the enclosing project on the single-file path**, using the same
      resolution `preview` already uses (`Site::discover` / `discover_single`), and root the
      freeze cache at the project when one is found. A loose file with no ancestor
      `_site.yml` keeps today's behaviour.
- [ ] **Step 4: Run the test** — expected PASS.
- [ ] **Step 5: Re-read the `run` retirement note at `main.rs:168`** and confirm its promise
      is now true. If any clause is still false, correct the note — one sentence, the date,
      then the successor, never phrased as a did-you-mean.
- [ ] **Step 6:** hold the commit for R3-2.

### Task R3-2: A `{js}` cell added mid-session gets its libraries

**The defect (audit finding 04, reproduced).** `page.rs:228` (and its twin at `:259`) gate
`js_cell_head()` — the ~490 KB of vendored d3 + Observable Plot — on `has_js_cells(p.body)`
against the server-rendered body. In a live preview that body is whatever the page had when
the tab loaded. Measured: `typeof d3` is `"undefined"` on the edit that adds the first `{js}`
cell and `"object"` after a manual reload. A cell calling `Plot.plot(...)` — the entire
reason those bytes are vendored — throws on the edit that creates it.
`render/mod.rs:1905` asserts the opposite ("always-on in preview").

- [ ] **Step 1: Write the failing test.** Drive a preview: load a page with no `{js}` cell,
      add one, and assert the client can reach `Plot` without a reload. If a headless
      browser is genuinely unavailable (wave 6 removed the net), assert the narrower
      server-side property instead — that the preview shell emits `js_cell_head()`
      unconditionally — and say so in the test's doc comment.
- [ ] **Step 2: Run it and watch it fail.**
- [ ] **Step 3: Emit the `{js}` head unconditionally on the preview path.** Preview is a
      loopback dev server; the bytes are already on disk and the correctness of a live edit
      outranks a first-paint saving that only ever applied to a page the author is actively
      adding a cell to. **Leave the build path content-gated** — that is the path a reader
      pays for, and it is correct there because the body is final.
- [ ] **Step 4: Run the test** — expected PASS.
- [ ] **Step 5: Correct `render/mod.rs:1905`,** which claimed this was already true.
- [ ] **Step 6: Verify by hand in a browser** with a real `Plot.plot(...)` cell added
      mid-session.
- [ ] **Step 7: Commit the wave.**

---

## Wave R4 — The truth sweep

**Branch:** `fix/r4-truth-sweep` · **No behaviour change. Nothing but prose and two registers.**

This wave exists separately because a prose diff hiding inside a behaviour diff is how the
campaign repeatedly lost track of what it had claimed. **No gate catches any item below.
Grep, do not trust.**

**Files (one commit):**

- [ ] **`crates/server/src/main.rs:267` and `:397–399`** — `--help` still promises the R
      interpreter, `IRkernel`, `_site.yml r:` and `TALIESIN_R`. R went in wave 6; `doctor`
      builds exactly one check, named `python`. Also `doctor.rs:293` and
      `interpreter.rs:354`. **Verified live against the shipped binary.**
- [ ] **`docs/guide/using/interactive.tmd:3` and `:7–8`** — the `description:` (which ships
      as both `<meta name="description">` and `og:description`) sells scrollytelling,
      narrated code walkthroughs and tabbed panels; wave 7 deleted all three sections from
      this same file's body. Surviving headings are reactive `{js}` cells,
      `{{< input >}}`, 3-D and teardown.
- [ ] **`taliesin build site --check-only` exits 1 — decide which way, then make it
      consistent.** All 11 errors are cross-project links (`docs/guide/`, `gallery/*/`) that
      only resolve in the deploy `tools/build-site.sh` composes, and that script *does*
      verify them (6 distinct targets, all resolve). So it is a false alarm — but the
      command CLAUDE.md and the guide both call **"THE PRE-PUBLISH GATE"** is permanently
      red on the one project the author ships, which trains you to ignore it. Cheapest
      honest fix: have the site's link validator skip a prefix that `build-site.sh` declares
      as a sub-project mount point, so the two tools agree on what a cross-project link is.
      Whatever you choose, `build site --check-only` must exit 0 by the end of this wave or
      the gate stops meaning anything.
- [ ] **`site/index.tmd:107` and `site/formats.tmd:43–44`** — *"Navbar, footer, listings,
      an Atom feed, OpenGraph, and Cmd-K search. This whole site is a Taliesin website."*
      Measured: **zero `listing:` blocks in `site/`** and **no `.xml` feed in the composed
      output**. Either correct the claim or make it true — and note R5-1 makes the
      `listing:` half true for `init`, not for `site/`.
- [ ] **`crates/core/src/frontmatter.rs:275–276`** — the `datasets:` note tells authors to
      write `{{< dataset … >}}`; `SHORTCODE_NAMES` is `["input", "include"]`, so following
      the note yields an unknown-shortcode warning and raw text in the page. Rewrite to one
      sentence: the date, then the successor or an explicit "nothing".
- [ ] **`crates/core/Cargo.toml`** — drop `sha2`, a direct dependency justified in the root
      manifest solely for that deleted shortcode. Re-run `cargo deny check` and clear any
      advisory ignore that goes with it (wave 4 recorded that an un-encountered ignore is a
      *warning*, not a failure). **Also drop the `libfuzzer-sys` licence exception in
      `deny.toml:77`** — cargo-deny reported it unmatched in this audit's run.
- [ ] **`CLAUDE.md:133`** — the fenced "Where things are" map lists `scrolly.js`,
      `tabset.js` and `walkthrough.js`, deleted in wave 7. The fenced map is **ungated by
      construction** (the path extractor discards fenced tokens); wave 1 measured this and
      declined to widen the extractor. Fix by hand.
- [ ] **`CLAUDE.md`, LSP paragraph** — *"Nothing here writes to a buffer."* Overstated:
      `lsp.rs:1137` builds a `WorkspaceEdit` for a "Change to `X`" quick fix, pinned by its
      own test. It is user-invoked and is the standard LSP contract, so the *behaviour* is
      right and the *sentence* is wrong. Add the qualifying clause.
- [ ] **`crates/core/tests/token_contract.rs:88, :96, :119`** — three
      `NO_RUNTIME_CONSUMER` exemptions cite `deck.rs:425` (deleted wave 5), theorem
      numbering and scrolly (both wave 7). Delete the exemptions with their subjects.
- [ ] **`crates/server/src/build.rs:2018–2021, :2668–2670`** — the `data-src` media harvest
      is dead (its promoter went with `{{< video >}}` in wave 7) and cites
      `corpus/media/screencast.tmd`, which does not exist. Delete the branch and the
      comment.
- [ ] **`crates/server/tests/help_cli.rs:43–47`** — the "no command was dropped" loop still
      lists `run`, and passes only on the unanchored substring `"run "` inside `ENV_HELP`'s
      *"never run code cells"*. Remove `run` and anchor the remaining needles so the test
      cannot pass on incidental prose.
- [ ] **`crates/core/src/render/mod.rs:2059, :2068`** — `callout_kinds()` and
      `retired_div_note()` have zero callers workspace-wide and their doc comments claim an
      out-of-crate test consumes them. Delete both.
- [ ] **`crates/server/src/protocol.rs:1–6, :175`** — *"shared by BOTH DEV SERVERS"*; wave
      1.1 deleted the single-document server and CLAUDE.md says so. Also
      `serve_site/mod.rs:946`. Sweep for the retired `check` and `read` verbs named as live
      (`preview_diag.rs:1–3`, `lint.rs`'s `Scope` docs, `interpreter.rs:15`,
      `site/bibliography.rs:62`, `exec.rs:1169`, `site/search.rs:158`, `cite/render.rs:21`,
      `lsp_cells.rs:16–17`).
- [ ] **The code-lens prose, four surfaces.** `docs/guide/reference/cli.tmd:387–388` says
      the server offers "the Run-Cell code lens" while `:446–452` of the *same page* says
      "There is no command to bind: the lens is a label". Also
      `editor/vscode/README.md`, `editor/vscode/walkthroughs/setup.md`, and
      `crates/server/src/lsp.rs:54–56`. **If you intend to take R6-2 (delete the lens),
      skip this item** — do not write prose you are about to delete.
- [ ] **`rmdir corpus/print/`** (empty since wave 4) and remove
      `corpus/_freeze/deck.json` (a cached record for a document deleted in wave 5). Both
      are gitignored, so this is hygiene, not a shipped change.

- [ ] **Verification:** `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh`, then
      re-read `taliesin help` and `taliesin help doctor` and confirm every sentence
      describes something the binary does.
- [ ] **Commit:** `docs: stop describing a tool that no longer exists`

---

## Wave R5 — Close the finish gaps

**Branch:** `fix/r5-finish`

> **LANDED 2026-08-09 as `17c4bf47`.** Gate green either side (10 gates; 81 suites, 1,384
> passed, 0 failed, 0 ignored). Two tasks shipped differently from the text below, both
> because the text was measured wrong; the commit body carries the numbers.
>
> - **R5-2 needed a crate move, not a call.** `minify_css` lived in `taliesin-server` and the
>   `AssetMode::Inline` arm lives in `taliesin-core`, which cannot depend on the server crate.
>   It now lives at `crates/core/src/minify.rs`, `build.rs`'s three call sites go through
>   `taliesin_core::minify_css`, and the inline path minifies once per process behind two
>   `LazyLock`s. **KaTeX is deliberately excluded**: it arrives already minified, so running it
>   through saved 1 byte of 369,347 and would have cost a second 369 KB resident. Measured, a
>   prose page 274,966 → 230,775 B (−16.1%); a math page 648,761 → 604,570 B (−6.8%).
>   **This moves the mermaid arithmetic far less than R6 assumes** — ~164 KB of what remains is
>   base64 font payload no minifier can touch.
> - **R5-3 cut `fig-align=` whole**, not just its `left` value. `left` was not a rendering
>   no-op: `figure.tali-figure` sets no `text-align`, so the undefined class inherited left and
>   looked correct. Deleting only that arm drops it to the `_` fallthrough, which is *centre*,
>   so a figure the author asked to be left would have silently centred with no validator to
>   say otherwise. Zero uses of `left` or `right` in `corpus/`, `site/` or either docs book,
>   no page documents the attribute, and `notes/2026-08-02-final-scope-audit.md:226` had
>   already ruled it a cut (author 0 / manual 0 / pin 1). Ruled by the author before landing.
>
> Also worth carrying into R6: the plan's R5-3 text claimed a "docs row" for `fig-align`.
> There was none anywhere outside `docs/superpowers/`, which R6-1 deletes.

### Task R5-1: The scaffolders compose

**The gap.** `init myblog` → `new post my-first-post` → `build .` produces a post that is
**unreachable from the homepage**. Its only appearance in `index.html` is the literal
instruction string `INIT_INDEX_TMD` wrote. The listing machinery already exists (~250 impl
lines, all three types exercised); it is simply not wired into the thing that teaches it.

- [ ] **Step 1: Write the failing test** in `crates/server/tests/init_cli.rs`: run `init`
      then `new post`, build, and assert the built `index.html` contains an `<a>` whose
      `href` is the post's path.
- [ ] **Step 2: Run it and watch it fail.**
- [ ] **Step 3: Add a `listing:` block** with `contents: posts` and `type: list` to the
      `INIT_INDEX_TMD` string constant. **Four lines of content, no code.**
- [ ] **Step 4: Run the test** — expected PASS.
- [ ] **Step 5:** Confirm the scaffolded project still passes `build . --check-only` with a
      posts directory that is *empty* (a fresh `init` before any `new post`). An empty
      listing must not be an error.
- [ ] **Step 6:** hold the commit.

### Task R5-2: `build <file.tmd>` minifies its inline assets

**The gap.** `build <dir>` runs `minify_css` and emits a ~47 KB stylesheet. The single-file
path uses `AssetMode::Inline`, which concatenates the raw constants with **no**
minification: measured 276 KB for a prose page, of which ~42 KB is developer comments. The
minifier already exists and was never pointed at this path. **This is the verb a first user
reaches for, and it produces the worse artifact.**

Do this before R6 decides mermaid: the 3.9 MB figure quoted against mermaid is largely this
defect, and fixing it changes that arithmetic.

- [ ] **Step 1: Write the failing test** asserting the inline `<style>` block of a
      standalone build contains no `/*` comment and is materially smaller than the raw
      constants.
- [ ] **Step 2: Run it and watch it fail.**
- [ ] **Step 3: Call the existing `minify_css` on the `AssetMode::Inline` path** in
      `render/page.rs`. Do not write a new minifier. Note wave 4 deleted `minify_js`
      deliberately; **do not restore it** — this task is CSS only.
- [ ] **Step 4: Run the test, and measure** the before/after byte counts on a real page.
      Record both in the commit body; the campaign's convention is measured, not asserted.

### Task R5-3: `fig-align="left"` stops being a silent no-op

`render/figure.rs:73` emits `class="tali-figure-left"` and **no stylesheet defines it**.
`base.css` has only `.tali-figure-center` and `.tali-figure-right`. It looks correct only
because left is the LTR default, and fence attributes have no validator, so the third
documented value gets silence.

- [ ] **Decide, then do one of two things.** Per the standing directive, prefer the cut:
      remove the `left` value and its documentation row, retiring it properly. If you keep
      it instead, add the `.tali-figure-left` rule to `base.css`. **Shipping one of three
      documented values as a no-op is not an option either way.**
- [ ] **Commit the wave:** `fix: the scaffolders compose, and a single-page build minifies`

---

## Wave R6+ — Deletions

**One wave per item or per coherent bundle, one branch, one commit, gates green either
side.** This is the only part of the plan that inherits the cut campaign's discipline
unchanged, including the ordering rule.

> **The corpus book floor was DISPROVEN 2026-08-09 and must not be built.** The paragraph
> below claimed deleting `corpus/demo-book` would *silently* vacate three named tests. It
> does not. Measured by moving the directory aside and running the suite: **four tests in
> `corpus.rs` alone fail loudly** (`book_discovers_chapters_with_parts_numbering_and_chrome`,
> `book_chapter_scopes_float_numbers_across_chapters`,
> `demo_book_logo_brands_both_the_topbar_and_the_chapter_drawer`,
> `a_site_page_prefers_its_authored_title_then_its_leading_h1`), plus dedicated files in
> `xref_cell_label_targets.rs`, `search.rs`, `lsp_stdio.rs` and
> `parallel_build_determinism.rs`. Every other book is hardcoded the same way:
> `analyst.rs`, `tarn.rs` and `descent.rs` each join their directory by name. The sweeps do
> iterate, but they are not the only coverage, which is what the premise assumed. **Wave 12's
> declination stands, for a better reason than it gave:** the floor is not machinery whose
> only job is to forbid a future cut, it is machinery duplicating a guard that already fires.

**~~Before starting R6, add the floor the campaign deferred.~~** `crates/core/tests/corpus.rs`
has **no assertion that any book exists**; its sweeps iterate over whatever is there, so
deleting `corpus/demo-book` would silently vacate three named tests. Wave 12 declined the
floor because *"a floor whose only job is to forbid a future cut is machinery this campaign
is removing"* — correct then, expired now that the campaign is over. One assertion.

| # | What | ~Lines | Note |
|---|---|---|---|
| ~~**R6-1**~~ | ~~`docs/superpowers/`~~ — **LANDED 2026-08-09**, see below | **35,585** | Design archive for features mostly cut, exempt from every drift gate. Larger than any wave of the campaign except wave 5, and none of it is product. Measured, not estimated. |
| ~~**R6-2**~~ | ~~The codeLens provider, whole~~ — **LANDED 2026-08-09**, see below | 520 | Half-removed: ships an empty command string on the wire. Deleting it retires an advertised provider, so `the_initialize_handshake_advertises_…` and the `extending.tmd` capability table both move in the same commit. |
| ~~**R6-3**~~ | ~~The VS Code e2e suite~~ — **LANDED 2026-08-09**, see below | 1,054 | **Verified: nothing runs it.** `gates.sh` runs `npm test`; the e2e sits behind `test:e2e`, invoked by no script, hook or workflow. `notes/2026-08-02` already recorded this. |
| **R6-4** | 15 hand-written tombstones in `retired_names.rs` | 429 | Pre-rule stock; wave 1 made retirements derived and these were never converted. |
| **R6-5** | The `_site.yml` JSON Schema, all three copies | 378 | Generator, committed golden, and the VS Code byte-copy CLAUDE.md already flags as drift-prone. |
| **R6-6** | Project-wide shared bibliography | 543 | Per-page citations survive. |
| **R6-7** | The structured `author:` form | 360 | Plain `author:` stays. **`AUTHOR_KEYS`' guide rows are ungated** — grep, do not trust. |
| **R6-8** | `doctor --format json` / `--json` + the package dump | 345 | The ruling sanctioned exactly one machine surface, and it is `build --format json`. |
| ~~**R6-9**~~ | ~~`math_preview.rs`, the hover math arm, `taliesin/mathCommands`~~ — **LANDED 2026-08-09**, see below | 560 | Editor-only surface living inside `taliesin-core`. |
| **R6-10** | `new`'s `<kind>` positional and its two registers | 176 | Wave 8 left one kind; a vocabulary of one needs no register. |
| ~~**R6-11**~~ | ~~`RenderedDoc::body_text()`~~ — **LANDED 2026-08-09**, see below | ~~~500~~ **959** | Audit finding 10. **Zero production callers** — the only two callers of `text::project` are `body_text` itself and one inside a `#[cfg(test)]` module. Both consumers its doc comment names are gone: `site/llms.rs` (wave 4) and the `read` verb (wave 2); the Cmd-K index uses the independent `render::indexable_text`. Invisible to clippy because `RenderedDoc` is re-exported from `lib.rs`. **Ordering rule applies:** `crates/core/tests/text_projection.rs`, `tests/snapshots/text-projection.txt` and `corpus/reader/text-projection.tmd` die in the same commit. Keep `decode`/`decode_numeric`/`indexable_text`; `render/mod.rs:2872 strip_tags_block_separated` goes too (its only caller is inside the dead subtree). |
| **R6-12** | Nine smaller items | ~1,100 | Dead validators, the cgroup-v2 container-memory walk, `csl:`, one of `preview`'s two port spellings, two corpus projects, the Cmd-K palette *actions*, `TALIESIN_CELL_TIMEOUT` (**sequence after R2**, which changes the interrupt story). |

### R6-1 — LANDED 2026-08-09, `cut/r6-1-plan-archive`

97 tracked files (22 plans, 75 specs), **35,585 lines, 2,813,883 bytes**. The 1,129,527-byte
`2026-07-03-quarto-design-decisions-catalog.md` was 40% of it on its own. Gate green either
side (10 gates).

- **No shipped document referenced it.** `docs/guide`, `docs/internals`, `site/`, `corpus/`,
  `README.md` and `CLAUDE.md` are all clean, which is why this is the largest item in the plan
  and also the lowest-risk.
- **Six live references were fixed in the same commit**, four of them not named in the plan:
  `retired_names.rs` (the `SKIP_PATHS` entry, its two doc comments, and the assertion —
  **retargeted at `notes/`, not deleted**, so the path-prefix exemption keeps a live guard
  instead of becoming vacuous), `stale_docs.rs`'s exclusion comment, and three source comments
  citing specs that were about to vanish: `preview_diag.rs`, `serve_site/mod.rs` and
  **`runtime_dirs.rs`**. That last one was missed on the first sweep because the grep output was
  truncated at 30 lines — a reminder that `| head` on a fallout survey is how a reference
  survives its subject.
- **`notes/`'s 34 remaining references were deliberately NOT rewritten.** They are dated
  records, and this repo's own rule (`stale_docs.rs`, `retired_names.rs`) is that rewriting a
  dated document to match today's tree destroys the record. They now point into git history,
  which is where the archive lives. **Two live lists are affected and are the exception worth
  knowing about:** `ROADMAP.md` (paused) and `FEATURE-IDEAS.md` carry plan pointers that now
  resolve only in history — a session unpausing either should expect that.
- **Backlog item 100 was corrected, not overridden.** Its 2026-07-28 "kept, not purged" ruling
  listed `docs/superpowers/` as part of the public-flip exhibit. That ruling is about *history
  rewriting* ("the history IS published"), and a `HEAD` deletion leaves every byte in the
  record, so it does not conflict. Its own line 172 left pruning open, and its line 175 argued
  for it. Three stale figures in that item were re-measured: 69 files → **97**,
  `git grep -Il "/home/bogo"` 21 → **14** (all now in `notes/`), and the catalog's byte count.

### R6-11 — LANDED 2026-08-09, `cut/r6-11-dead-text-projection`

**959 deletions / 49 insertions**, roughly double the ~500 the row estimated: the row counted
`text.rs` and missed the cascade behind it. Gate green either side (10 gates).

- **Method worth reusing for the remaining items.** Delete the roots, then let
  `clippy -D warnings` enumerate the dead cascade instead of hand-tracing 24 functions. It
  named **23** items, including two the plan did not: the `Separate::NonPhrasing` variant and
  the `PHRASING` table, which existed only to serve `strip_tags_block_separated`. `text.rs` is
  now the three survivors the search index needs.
- **The trap the plan flagged is real and had to be worked around.** A function reached only
  from a `#[cfg(test)]` module is not dead to `--all-targets`, so the in-file projection tests
  had to go before clippy could see the truth.
- **One test re-pointed, not deleted** — same call as R6-1's exemption assertion.
  `repro.rs`'s `the_code_download_box_stays_out_of_every_text_projection` guarded that the
  "Run this yourself" box never leaks into text across three consumers that each carried
  their own `REPRO_BLOCK_ID` literal. **One consumer is left** (the Cmd-K index), so it is now
  `the_code_download_box_stays_out_of_the_search_index` and runs the exact filter/extract pair
  `site/search.rs` runs. The constant's doc comment claimed four consumers in three modules
  and named the old test; both were corrected.
- **`indexable_text` gained a test it never had.** Its block-boundary and whitespace-collapse
  behaviour was covered only through the projection's `visible` tests, which went with it —
  a coverage hole the deletion would otherwise have opened silently.

### R6-2 + R6-3 + R6-9 — LANDED 2026-08-09, `cut/r6-editor-surface`

Taken as one wave because all three move `the_initialize_handshake_advertises_…`, and moving
one test three times is how a register drifts. **−3,180 lines** (`+346 / −3,526`, 35 files, 12
deleted) against a 2,134 estimate; **−2,489 in code**, excluding `package-lock.json` (−861) and
`notes/` (+169, this entry and the tier-2 rulings). Gate green either side (10 gates; 80 suites,
1,347 passed, 0 failed, 0 ignored). Full log in `notes/CUT-PROGRESS.md`.

- **R6-2's real justification was not the one the plan gave.** The plan called the lens
  "half-removed: ships an empty command string on the wire", which is a rendering convention
  rather than a defect. What decided it: the **one ground on record for keeping it** — that
  `editor/vscode/src/runcell.ts:9-14` proved a TypeScript `CodeLensProvider` would regrow in
  its place — names a file **wave 13 had already deleted**. The campaign's most-recurring rule
  landing on the campaign's own justification.
- **The cascade was larger than the three roots, and clippy found all of it** (R6-11's method,
  reused): `lsp_memo` entire (85 lines — the lens arm was its **only** consumer, in a module
  CLAUDE.md described as part of the `didChange` story), `exec::cell_cache_keys` +
  `CellCacheKey` (52), `lsp_nav`'s `enclosing_math`, the `Target::Math` variant and two offset
  helpers, and three of `MathSpan`'s five fields.
- **The plan's R4 item that said "if you intend to take R6-2, skip this" was right to say so.**
  The four code-lens prose surfaces it listed are deleted here rather than corrected, which is
  a wave of R4 never paid.
- **No coverage hole from the math trim, checked rather than assumed.** The four
  `classify_target` math tests deleted here pinned fence-awareness and the line-break rule;
  `lsp_complete` pins both against `scan_math` directly, which is the code that survives.
- **`@vscode/test-electron` is NOT e2e-only, and only `gates.sh` could tell me.** Deleting it
  with the suite turned the companion gate red: `scripts/ensure-vscode.cjs` uses it to download
  the VS Code build whose bundled markdown/python/yaml grammars the **surviving** offline
  `grammar.test.ts` reads. Restored, with the reason written into the script's header. The local
  `npm test` was green throughout, because the download was already on disk — exactly the class
  of silently-inert gate `tools/gates.sh` exists for. `editor/vscode/.vscode-test/` (3.1 GB) is
  that gate's fixture, not residue.

**Tier 2: CLOSED AND FULLY EXECUTED 2026-08-09.** All four were ruled by the author, and both
rulings that needed a wave have landed (`cut/r6-t2-powershell`, `cut/r6-t2-notes-banner`).
**Nothing in tier 2 is owed: no decision, no wave.** The two KEEPs each left one residual defect
recorded in their bullets below: mermaid's standalone-inline blow-up (a real, separate wave), and
its stale byte figures in `render/mod.rs:1842`/`:1849`/`:1871` (one correcting commit).

**Both KEEP residuals are now PAID, 2026-08-09, `fix/r6-t2-mermaid-standalone`**, taken as one
wave because they are one subject: `build <file.tmd> --out <dir>` links a sibling
`mermaid.min.js` instead of inlining the library (**3,803,736 B → 238,566 B, −93.7%**), and the
byte figures the block carried were replaced with measured ones (3,565,102 B on disk, 971,040 B
gzipped). Full log in `notes/CUT-PROGRESS.md`.

| item | ruling | what it costs the next session |
|---|---|---|
| vendored mermaid (~3.5 MB) | **KEEP** | nothing. Both residuals paid; see the bullet below and the wave log. |
| Atom feeds (602 lines) | **KEEP** | nothing. Reasoning below. |
| vendored PowerShell grammar (1,557 + LICENSE) | **CUT, LANDED 2026-08-09**, `cut/r6-t2-powershell`, **−1,738 in code** | nothing. See the bullet below for what the wave found. |
| `notes/`'s 64 July-dated audits (18,454 lines) | **BANNER, LANDED 2026-08-09**, `cut/r6-t2-notes-banner`, **+256** | nothing. **Tier 2 is now fully executed.** See the bullet below. |

- **The vendored PowerShell grammar. RULED CUT 2026-08-09, LANDED the same day** as
  `cut/r6-t2-powershell`, **−1,738 lines of code** (`+14 / −1,752`, 7 files, 2 deleted) against
  the ~1,650 estimate. Gate green either side (10 gates; 80 suites, 1,342 passed, 0 failed, 0
  ignored — exactly the 5 tests this wave deletes below R6's 1,347). Full log in
  `notes/CUT-PROGRESS.md`. **Three things the ruling below did not know:** the release binary
  shrank **−479,104 B** for a **50,804 B** asset, because `vendored()` was the only reachable
  caller of syntect's `.sublime-syntax` *source parser* (everything else loads a precompiled
  dump), so the reclaim was the format reader rather than the file; `clippy -D warnings` found
  **zero** cascade, unlike every other R6 item; and the silence claim was verified rather than
  assumed (both fences, 0 scope spans, `--check-only` and `--strict` both clean, exit 0). Two
  adjacent findings were surfaced and left for a later wave: **`highlight::known_language` is
  dead workspace-wide** (invisible to clippy because it is `pub`; an R6-12 candidate), and the
  stale `scrolly.js`/`tabset.js`/`walkthrough.js` list has **three copies of which R4 fixed
  one**. The ruling as it stood:
  `crates/core/assets/syntaxes/
  PowerShell.sublime-syntax` (1,557 lines) plus its LICENSE, `include_str!`-compiled into every
  shipped binary. **Its only witness anywhere is `corpus/highlight.tmd`** — zero uses in
  `docs/guide`, `docs/internals`, `site/` or `samples/` — and a corpus-only pin is exactly the
  circular evidence the scope ruling disproved. It was added on the 2026-07-22 demand probe,
  whose "user" was a **docs-maintainer persona**, which is the same evidence class. Cutting it
  costs a `powershell`/`ps1` fence its colours and nothing else: wave 9 removed the generic
  unknown-fence-language lint, so there is no diagnostic to go stale either.
  **The wave, and the ordering rule applies to all of it in one commit:** delete the two asset
  files and the `include_str!` + match arm in `highlight.rs`, the `powershell_highlights_under_
  both_of_its_tokens` unit test, `highlight_langs.rs`'s two `powershell_*` tests, and the
  `powershell`/`ps1` sections of `corpus/highlight.tmd`. Check `THIRD_PARTY.md` for its licence
  row. **No register entry is owed** — a fence language is not a Taliesin vocabulary, so there
  is no retired name for the tool to answer.

- **Atom feeds. RULED KEEP 2026-08-09.** `feed.rs` (602 lines, not the 545 this plan said) plus
  the `<link rel="alternate">` autodiscovery in `meta.rs`. It fires only for a project that sets
  `url:` **and** carries an uncapped listing with at least one dated page, so a fresh `init`
  (whose `_site.yml` is `title: My site` and nothing else) gets none, and neither does the
  marketing deploy. The one witness, `corpus/tech-blog`, mirrors a blog the author actually
  publishes. *"When close, cut"* adjudicates features nobody uses; this one has a user, adds no
  vocabulary, needs no config, and costs a project that does not want it exactly zero.
  `Site::nav_ordered` stays in `feed.rs` with it.

- **`notes/`'s 64 July-dated audits. RULED BANNER-DO-NOT-DELETE 2026-08-09, LANDED the same
  day** as `cut/r6-t2-notes-banner`: **+256 lines, 0 deletions, 64 files**, four lines each, and
  the diff touches nothing outside `notes/2026-07-*.md`. **The ruling's premise is now measured
  rather than asserted, and it was understated: 37 of the 64 audits cite at least one repo path
  that no longer exists, across 112 distinct dead paths**, a floor, since the scan is
  backticked-only, extension-gated and resolves a token by suffix the way `stale_docs.rs` does.
  `corpus/deck.tmd` is named by **14** of them. So more than half of the directory points at
  something gone and nothing in the tree said so. Prose only, exactly as ruled: no gate, no
  register, no index entry. Full log in `notes/CUT-PROGRESS.md`. The ruling as it stood:
  They carry
  measurements that cost sessions to acquire, and every byte would survive in git — but the
  reason to act is that their **live cost is demonstrated, not hypothetical**. Wave 12 found
  eight spent justifications in one wave, and wave R6 found a ninth: the playbook's ground for
  keeping the code lens named `editor/vscode/src/runcell.ts`, a file wave 13 had already
  deleted, and it was two greps from being honoured again.
  **The wave:** one STATUS line at the top of each dated audit, to the effect of *"dated record.
  Superseded by the 2026-08-08 scope ruling; before acting on anything here, check that the file
  it names still exists."* Prose, not machinery — no gate, no register, no index. This is the
  same call R6-1 made when it left `notes/`'s 34 references pointing into git history rather
  than rewriting them: **do not rewrite a dated document to match today's tree**; say that it is
  dated, and let the reader do the check.

The mermaid split, stated so the decision is a decision:

- **Mermaid. RULED KEEP 2026-08-09 by the author, on the re-measurement this bullet asked
  for.** The re-measure, post-R5-2: the library is **3,565,102 B on disk, not the 3.9 MB the
  dispute was argued on**, and **971,040 B gzipped**, which is the number that was never taken
  and the one a reader actually pays. On the site path it is one deferred, content-hashed,
  shared asset loaded by the pages that have a diagram (5 of 8 in `docs/internals`, 2 of 20 in
  `docs/guide`); a docs page is 15 KB gzipped beside it. R5-2 saved 44 KB against mermaid's
  3,572 KB, so **minification moved the arithmetic by 1.2%** and settled nothing, exactly as
  predicted. 18 diagrams confirmed, 16 in the two books.
  Three arguments carried the ruling: the cost was overstated 3.7×; the diagrams are not 18
  fences but **18 figures** carrying `%%| label: fig-*` and referenced through the xref system,
  so cutting them unpicks cross-references across both books; and every alternative adds more
  machinery than mermaid removes — `{js}`/d3 has no graph layout (hand-placed coordinates,
  re-placed on every edit), Python+graphviz puts a system `dot` binary on the critical path of
  `tools/build-site.sh` with `_freeze` gitignored, and server-side SVG means adopting a layout
  crate *and* rewriting all 18 diagrams into another syntax.
  **Both residuals below were PAID 2026-08-09** in `fix/r6-t2-mermaid-standalone`; the text is
  kept as it stood so the wave log can be read against what it was asked to do.
  **The prose is stale where the numbers are:** `render/mod.rs:1842` says "~2.8 MB" and
  `:1849`/`:1871` say "~2.5 MB". Worth one correcting commit.
  **The residual defect, which is a real and separate wave:** the standalone path inlines the
  whole library. Measured, one 2-node diagram takes a page from 230,642 B to **3,803,188 B
  (16.5×)**, and `build <file.tmd> --out <dir>` inlines it too even though that mode's contract
  already permits sibling assets. Fixing the `--out <dir>` spelling alone would be contained and
  would leave true single-file `build page.tmd` self-contained.
- **Atom feeds.** One witness in 110 documents and zero feeds in the composed deploy, but
  that witness is `corpus/tech-blog`, a mirror of a blog the author actually publishes.
  "When close, cut" adjudicates features nobody uses; this one has a user. If it goes, make
  it a recorded ruling that Taliesin does not do syndication, not a byte trade.

**Tier 3 — watch, do not act.** Cmd-K search, LSP completion, and the single-instance
takeover probe. Each is large; each was defended by all three judges.

---

## Structural gaps this plan deliberately does NOT close

Recorded so the next session knows they are choices, not oversights. Each would *add*
machinery, which is the direction the ruling wanted reduced.

- **No CLI→docs gate direction.** `stale_docs.rs:552` gates docs→CLI only, so a flag can be
  parsed, unit-tested, named in `--help` and appear in zero pages a reader opens — the
  measured state of `--port`, `--json`, `--draft` and `--dir`. R6-10 and R6-11 remove two of
  the four; the hole stays.
- **41 of 97 offered vocabulary names have no drift gate.** Only `KNOWN_KEYS` is tied to the
  reference page.
- **`RETIRED_FLAGS`' derived test covers 2 of the 5 parsers that consult it.** `init`, `new`
  and `doctor` all route through the same `unknown_flag_error`.
- **A fence attribute has no validator at all** (carried forward from the campaign; R5-3
  walks into it).
- **No documented install path for the VS Code companion,** which the User Guide names seven
  times. Basic click-to-source survives without it via the `vscode://` deep link; forward
  search and cursor routing do not.

If the author wants these closed, they are one wave of their own — and that wave should be
an explicit decision about the anti-drift ratio (`main.rs` is already 43% implementation,
57% gate), not accretion.
