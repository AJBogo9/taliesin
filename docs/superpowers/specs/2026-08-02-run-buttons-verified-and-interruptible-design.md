# Run buttons: verified, and interruptible

Item 175(d), second half. Closes `FEATURE-IDEAS.md` idea 86 in the reduced form ruled below.

The first half of 175(d) (per-cell run via CodeLens) shipped in `1b8b3756` alongside
`taliesin run`. This spec covers what that commit left: the buttons have no test coverage,
and they can start work the author cannot stop.

## Correction to the item's filed text

**Item 175(d) is half stale.** It reads "No per-cell run and no interrupt", and names both as
unbuilt. Per-cell run exists: `editor/vscode/src/runcell.ts` registers a `CodeLensProvider`
giving `▶ Run Cell` and `Run Above` over every executable fence, wired to
`taliesin run --line N`. Only the interrupt half is real. **Rewrite the item's first sentence
when this lands, then delete the item.**

**One earlier hypothesis was wrong and is recorded so it is not re-investigated.** The lens
data comes from `taliesin/cellRegions`, and `didChange` is coalesced in a 120 ms window
(`lsp.rs:146`), which suggests lenses could render against a stale buffer. They cannot: the
handler reads `docs.get(&uri)` (`lsp.rs:615`), the buffer map updated synchronously on
`didChange`. The coalescing gates *diagnostics publishing* only. There is no staleness bug.

## The problem

**Nothing proves VS Code ever accepted the provider.** There is no `runcell.test.ts`, and the
e2e suite's `registers its contributed commands` list names eleven commands, neither of them
`taliesin.runCell` or `taliesin.runAll`. This repo has been bitten twice by exactly this
class: a typo'd `contributes` enum silently disables a feature, and a unit test cannot show
that VS Code asked the server, wired the response into a provider, and rendered it.

**A run cannot be stopped without destroying the thing the design protects.** Ctrl-C in the
terminal kills `taliesin run`, which is only the thin client: `runspec::event_stream` sees the
hangup, breaks its forwarding loop, and the queued run keeps executing in the session. The
author is left with `restart_kernel`, which nukes every warm variable. So a one-click button
can launch a twenty-minute cell whose only brake discards the 3M-row load that made the warm
session worth having.

## The interrupt path: one signal implementation, not two

`Kernel::interrupt()` (`kernel.rs:1266`) already sends `SIGINT` by PID, and is how the silence
cap stops a runaway cell. It is private and is called from *inside* `execute_streaming`'s
polling loop, which holds the kernel mutably for the whole run. An interrupt arriving on
another request can therefore never obtain `&mut Kernel`, and restructuring that ownership to
allow it would be a large change to the execution core for one feature.

It does not need to. `interrupt()` needs only a PID, and `KernelProc::pid()` (`kernel.rs:624`)
supplies one for both owned and forkserver children.

1. Extract `pub(crate) fn interrupt_pid(pid: u32)` in `kernel.rs`, holding the `libc::kill`
   call and its existing safety comment. `Kernel::interrupt()` becomes a call to it. **The
   runaway-cell cap and the explicit interrupt are then literally the same code**, so they
   cannot drift on what "interrupt" means.
2. A run registry: `Mutex<HashMap<String, Running>>` with `Running { lang, pid, cancelled }`,
   keyed by page key (the rel for a site, the document for a single doc). The exec loop
   records an entry when a cell starts and clears it when the run ends, by any exit including
   error and cap.
3. `POST /__taliesin/interrupt`, loopback-only, on both servers. Guarded exactly like
   `RUN_PATH`: a non-loopback peer gets 403, and the `--host` LAN token must not hand a
   visitor a trigger. Body `{ "file": "<abs path>" }`; it resolves to a page the same way
   `run_handler` does. Answers `{ "interrupted": true, "lang": "python" }`, or
   `{ "interrupted": false }` when that page has nothing running.

> **Corrections from implementation (2026-08-02).** Two things in this spec were wrong and are
> left in place with this note rather than edited away, because both were found by running the
> feature rather than by reasoning about it.
>
> 1. **`cancelled` had to become an epoch, not a boolean.** Runs *queue*: `taliesin run` starts
>    a session, that session immediately does its own execution pass, and the client's run waits
>    behind it. A boolean cleared at each run's start stopped the in-flight pass and then let the
>    queued run begin clean and execute the remaining cells anyway. Verified end to end with a
>    marker file. A run now carries the epoch it was **requested** at and stops as soon as the
>    live epoch differs, which covers in-flight and queued runs alike. `begin_run`/`end_run`/
>    `is_cancelled` do not exist; `RunControl::epoch()` and a `requested_at` parameter on
>    `run_through` replace them.
> 2. **"Non-loopback peer: 403, mirroring the run endpoint's own test" could not be done.** The
>    run endpoint has no such test; nothing in the tree references `RUN_PATH`. Manufacturing a
>    non-loopback peer needs real scaffolding, so this is filed in `notes/DETECTION-DEBT.md`
>    (D=7) with the fix that would close it for both endpoints at once, rather than faked.

**Interrupt stops the run, not just the cell.** This is the part that is easy to get wrong.
Signalling the running cell's PID ends *that cell*; a multi-cell run would then carry on into
cell 4 of 10, which is the opposite of what Ctrl-C means. So the endpoint does two things: it
signals the current PID **and** sets `cancelled` on the entry. `exec`'s run loop checks that
flag between cells and abandons the rest of the plan, reporting how many cells were skipped.
Both halves are required and neither alone is correct: the flag without the signal leaves the
current cell running to completion, and the signal without the flag stops one cell out of ten.

Nothing about executor ownership, the exec pool LRU, or freeze keying changes.

## The client

**`--interrupt` is a flag on `run`, not a subcommand.** A new subcommand trips five CLI drift
gates (`COMMANDS`, `COMMANDS_HELP`, `subcommand_help`, `complete::command_desc`,
`flags_for`/`positional_kind`) plus the "no subcommand hand-writes its own usage line" rule. A
flag trips only `flags_for`. The capability does not justify the surface.

- It refuses to combine with `--cell`, `--line` or `--all`, in the same
  refuse-rather-than-resolve style those three already use on each other.
- **It never starts a session.** No live session means nothing is running: say so and exit 0.
  Booting a kernel in order to interrupt it is absurd, and would take `SESSION_READY_TIMEOUT`
  (45 s) to do nothing.

**Ctrl-C is the gesture.** A `tokio::signal::ctrl_c` handler in `run_cmd::run`:

- First Ctrl-C posts the interrupt and **keeps streaming**, so the `KeyboardInterrupt`
  traceback arrives and is printed like any other cell error. A client that exited here would
  hide the very output that confirms the stop worked.
- Second Ctrl-C exits immediately, for a session that is wedged or unreachable.
- Exit code stays `1` on an interrupted run: the document was not made true.

## What is deliberately not built

**No Stop lens and no status-bar item.** Ruled 2026-08-02. Once Ctrl-C works, the run's
terminal is already open and focused whenever a cell is running, so a Stop control is a second
surface for an act the author can already perform with zero discovery. The lens version
additionally needs a state channel from session to editor built purely so the editor can know
a run is in flight, which is real complexity for a redundant button. Minimal config: perfect
the default before adding a control.

**This reduces `FEATURE-IDEAS.md` idea 86**, which reads
`▶ Run cell · ⟲ Run below · ⏹ Interrupt · ⚡ cached (4.2s)`. Three of those four are now
settled: `▶ Run cell` shipped, `⏹ Interrupt` lands here as Ctrl-C rather than a lens, and
**`⟲ Run below` and `⚡ cached (4.2s)` are not built and are not filed as debt.** `Run below`
is a plausible cheap add; the cached-time annotation needs the same state channel the Stop
lens was rejected for. Annotate idea 86 with this outcome rather than deleting it, so the
reduction is visible and not re-proposed as a gap.

**Nothing new is cached on interrupt.** Ruled 2026-08-02. A `KeyboardInterrupt` is an error,
and errors are already never persisted to `_freeze`, so the interrupted cell and everything
downstream stay uncomputed and the next run recomputes them. Flushing the completed prefix was
considered and rejected: a run the author aborted would then write to the cache, and the
capped-run "cached tail" rule already produced one wrong answer in this exact area. **No new
rule is introduced, which is the point.**

## Testing

**The lenses, in a real Extension Host** (`src/e2e/suite/integration.test.ts`, the existing
harness). A unit test cannot make these claims.

- `taliesin.runCell` and `taliesin.runAll` appear in the existing contributed-commands list.
- `vscode.executeCodeLensProvider` over a corpus document with cells returns lenses anchored
  on fence lines, titled `▶ Run Cell` / `Run Above`.
- **No lens over a non-executable fence.** A `{bash}` or plain ```` ```python ```` fence must
  get nothing; a Run button over an unrunnable fence is what drift looks like to an author.
- **`Run Above` is absent on the first runnable cell** and present on later ones.
- **The lens's `--line` argument round-trips.** Feed each lens's line argument to
  `runspec::resolve` and assert it selects the cell the lens sits over. This is the pin that
  binds the TypeScript anchor arithmetic to the Rust resolver; without it the two can drift by
  one line and every button silently runs the wrong cell.

**The interrupt, in Rust.**

- Idle page: the endpoint answers `interrupted: false` and signals nothing.
- Non-loopback peer: 403, mirroring the run endpoint's own test.
- `--interrupt` with `--cell`/`--line`/`--all` is refused by `Opts::parse`.
- `--interrupt` with no live session exits 0 and starts nothing. Assert no session was
  spawned, not merely that the exit code was 0.
- **Live-kernel** (gated on `TALIESIN_REQUIRE_KERNEL`): a cell that sleeps is interrupted, the
  run ends non-zero, **a variable set by an earlier cell is still readable afterwards**, and
  `_freeze` gained no entry for the interrupted cell. The surviving-variable assertion is the
  whole feature; without it this is indistinguishable from a restart.
- **Live-kernel, the cancellation half:** a document whose cell 2 sleeps and whose cell 3
  writes an observable side effect (a file, or a variable the test reads back). Interrupt
  during cell 2 and assert **cell 3's effect never happened**. A test that only asserts cell 2
  raised would pass against the wrong implementation, the one that stops a cell and then runs
  the remaining eight.

`./tools/gates.sh` must be green with all nine gates running, not a bare `cargo test`.

## Out of scope

- The scratch console, `--restart`, `--status`, and `taliesin vars`. Still deferred.
- `⟲ Run below` and `⚡ cached (4.2s)` from idea 86, per the reduction above.
- Any change to freeze keying, the block model, the exec pool LRU, or `MAX_WARM_PAGES`.
- Windows. `runcell.ts` quotes its argv for a POSIX shell, and a quoted command name does not
  invoke in PowerShell. Pre-existing, unrelated to this change, and this is a Linux tool.
- Moving the CodeLens list into Rust via `textDocument/codeLens`. The `Run Above`-on-first-cell
  policy currently lives in TypeScript (`runcell.ts:99`), which is in tension with "add an
  editor feature in Rust, not here". Real, but it is a refactor of working code and belongs in
  its own change; the round-trip test above pins the behaviour either way.
