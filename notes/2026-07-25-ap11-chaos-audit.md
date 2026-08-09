# AP11: chaos / failure-injection UX (2026-07-25)

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Perspective:** AP11 from `backlog.md`'s "Audit perspectives", run at the author's request as the
third audit of the day. **Run solo** against `e7d6bb9`, release binary, real builds with a real
warm Python kernel. **Nothing was changed**: findings only.

## Headline

**The degradation paths are genuinely well-built, and this round's yield is correspondingly low.**

Every failure injected either self-healed silently (when the lost thing was an optimization) or
failed loudly with exit 1 (when the lost thing was the output). The single defect is a *wording*
bug, not a behaviour one: a missing interpreter is reported to the console as **the author's code
raising an uncaught exception with a traceback baked into the output**, which is false on both
counts and sends them hunting in the wrong place.

That is worth saying plainly rather than padding the round: AP11 was ranked "best yield-per-hour of
the two remaining" and it returned one low-to-medium finding. **AP6 is now the only unrun
perspective**, and the non-AP lenses are likely a better use of a session than re-running this one.

## The entry's own premise, re-measured first

AP11's entry carried exactly one concrete seed, and it is already closed:

| Premise | Verdict |
|---|---|
| "PA-B1: the kernel-unavailable message tells headless callers to click a Restart button that is not there" | **Refuted, already fixed.** `exec.rs:338-340` documents removing that clause *precisely* because the message is shared with headless `build`/`read`/CI. The current wording names the interpreter and the missing Jupyter package and routes to `taliesin doctor`; the live preview still offers Restart in its dev menu. |

## The one finding

### AP11-1 (low-medium): a missing interpreter is reported to the console as an author code exception

**Measured.** With `TALIESIN_PYTHON=/nonexistent/python`, a build prints three console lines. The
first and third are correct. The middle one is not:

```
warn  kernel unavailable (cannot launch `/nonexistent/python`: No such file or directory …)   <- correct
warn  cell error in index.tmd (@ 7:1-10:3): code cell raised an uncaught exception;
      its traceback is baked into the output                                                  <- FALSE
warn  kernel unavailable; code cells were emitted as source (set TALIESIN_PYTHON …)           <- correct
```

The cell raised nothing (the kernel was never launched) and no traceback exists anywhere. **The
rendered page is correct** and carries a precise located diagnostic:

```html
<div class="tali-output" …><pre class="tali-error">python kernel unavailable; this cell did not
execute (cannot launch `/nonexistent/python`: No such file or directory (os error 2) …)</pre>
```

**Root cause, re-derived from source.** `build.rs:380 is_cell_error_output` classifies a block as a
crashed cell purely by shape:

```rust
html.trim_start().starts_with("<div class=\"tali-output\"")
    && html.contains("class=\"tali-error\"")
```

The kernel-unavailable diagnostic is emitted with **exactly** that shape (verified in the built
HTML above), so it matches, and `cell_error_message` (`build.rs:478`) then asserts an uncaught
exception and a baked-in traceback unconditionally.

**Why it matters.** A wrong interpreter path is plausibly the single most common setup failure, and
this is the one place the tool tells the author what went wrong. Misattributing an infrastructure
failure to their code, and pointing at a traceback that does not exist, is the opposite of the
graceful degradation the rest of this surface achieves. It also reaches `--format json`, since
`cell_error_diagnostics` (`build.rs:493`) filters on the same predicate.

**Fix shape:** distinguish the two at the source of truth rather than by HTML shape. Either give the
unavailable diagnostic its own class (and exclude it from `is_cell_error_output`), or have the
executor mark the block so the console can say "did not execute" instead of "raised". Reproduced in
both the missing-interpreter and the not-actually-a-python (`/bin/true`) cases.

## Verified sound, do not re-audit

Every one of these was injected and measured, not read:

| Injected failure | Behaviour | Exit |
|---|---|---|
| `_freeze/*.json` truncated mid-write | discarded, cell re-executed, correct output | 0 |
| `_freeze/*.json` valid JSON, wrong schema | same | 0 |
| `_freeze/*.json` replaced with binary garbage | same | 0 |
| `_freeze/` directory unwritable | located `warn cannot write cache …`, build completes | 0 |
| **Output directory unwritable** | `error cannot write …/_assets: Permission denied`, **nothing half-written** | **1** |
| Interpreter path does not exist | precise located page diagnostic, cells render as source | 0 (by design) |
| Interpreter exists but is not a python (`/bin/true`) | same | 0 (by design) |
| Either of the above **under `--strict`** | `error --strict: 1 problem … failing the build` | **1** |

The cache-corruption trio is the notable one: the freeze cache is an *optimization*, and all three
corruption shapes degrade to "just re-run the cell" with a byte-correct result. That is the right
call, and it means a crash mid-write cannot poison a later build.

**Websocket / server death** (read, not injected): `client.js:1392` sets the status to
`reconnecting…` on `onclose` and retries every 1s, `onerror` closes to trigger the same path, and a
boot-id + generation check (`:1187-1212`) forces a **full re-mount** when the reconnect lands on a
*restarted* server rather than trusting a generation number that reset. So a SIGKILLed server
produces a visible reconnecting state and a correct remount, not a silently stale page.

## Not chased

- **Disk-full during a build.** Needs a real small filesystem to mount; the unwritable-directory
  case exercises the same `write` error path but not a *partial* write.
- **SIGKILL mid-build**, i.e. whether a half-written `_site/` is left behind (the atomic-rename
  discipline is only proven for `_freeze/`).
- **Kernel death mid-cell in the browser.** There is a passing unit test
  (`exec::tests::mid_run_kernel_death_self_heals_on_the_next_rebuild`), so the recovery is covered;
  what is untested is what the author *sees* while it happens.
- **Chaos with multiple concurrent clients**, and the `--host` LAN path.

## False lead, mine

**`cmd | tail -4; echo $?` reports `tail`'s exit status, not the binary's.** This made the
unwritable-output-dir case read as `exit=0`, i.e. a build that could not write anything and
reported success, which would have been the round's headline finding and is completely wrong.
Re-measured without the pipe: **exit 1**, and nothing written. **Never read `$?` after a pipeline**
(use a temp file, or `${PIPESTATUS[0]}` / zsh `$pipestatus[1]`).

## Method

Release binary at `e7d6bb9`. A throwaway one-page site project with a single `{python}` cell, built
repeatedly under injected failures (`chmod` for permissions, hand-written corrupt JSON for the
cache, bogus `TALIESIN_PYTHON` values for the interpreter), with exit codes captured to a file
rather than through a pipe. Console output, the built HTML and the on-disk output directory were all
inspected per case. Code read over `build.rs`, `exec.rs` and `web-client/client.js`. No repo file was
modified.
