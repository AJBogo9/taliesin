# Cut progress

Durable state for the scope reduction. **This file is the handoff.** A fresh session
needs only three things: this file, [the ruling](2026-08-08-scope-ruling.md), and
[the playbook](2026-08-08-cut-playbook.md).

## Standing directive (2026-08-08, from the author)

> "When it comes to cutting something, always lean towards cutting. I'd rather have a
> polished lean product, and then add features when I have real users that need them
> than having a bloated product with features that nobody uses."

**This resolves every judgement call in the ruling's §9 toward the cut.** Where the
ruling left a choice, take the smaller option. The only exceptions are sequencing
constraints, not feature retentions:

- Keep a static-lint front door (`taliesin lint` or `build --check-only`, ~40 lines
  replacing 2,800). That is not keeping `check`; it is replacing it with something
  1/70th the size. Deleting the verb with no replacement leaves the project with no
  pre-publish gate at all, and nine waves' verification recipes call it.
- Keep exactly one machine-readable output (`--format json` on the survivor). Going to
  zero breaks the author's own AI-assisted workflow within a week.

Everything else in §9 is decided: **deck cut entirely** (not reduced), **theorems cut**,
**debug stepper cut**, **`{r}` cut**, **`--host` cut**, **panel-tabset cut**,
**`image_opt` cut**, **warm pool cut** (accept the ~1.9 s spinner on cold/evicted pages).

## Rules

1. **One wave per session, one branch per wave, one commit per wave.** Never two.
2. Baseline before starting: `./tools/gates.sh` must be green (or its failures known
   and recorded below).
3. After each wave: `./tools/gates.sh`, then record the measured reclaim here, then
   commit. Do not estimate the reclaim; measure it with `git diff --stat`.
4. Never delete a corpus pin ahead of the feature it guards. That is why wave 12 is last.
5. If a wave turns out bigger than its section says, stop and split it. A half-finished
   wave that lands green is fine; a wave that lands red is not.

## Baseline

| | |
|---|---|
| Start commit | `f6dee87d` |
| Safety tag | `pre-cut` (create with `git tag pre-cut f6dee87d`) |
| `cargo test --workspace` | 123 suites, 2,318 passed, 0 failed, 0 ignored, exit 0 |
| `./tools/gates.sh` | **NOT YET RUN.** Establish before wave 1. |
| Rust LOC | 130,500 |
| Bundled JS+CSS | 16,535 |
| `.tmd` (corpus + docs + site + samples) | 22,562 |
| CLI verbs | 18 |
| Catalogued document features | 115 |
| LSP providers advertised | 16 (+6 custom methods) |
| External runtimes needed by the gate | 4 (Python, R, Node, Chrome) |

Target: ~69,000 lines removed, 9 verbs, ~55 features, 7 providers, 2 runtimes.

## Waves

| # | Wave | Status | Branch | Measured reclaim | Notes |
|---|---|---|---|---|---|
| 0 | Establish `gates.sh` baseline + `pre-cut` tag | not started | | | prerequisite |
| 1 | Anti-drift simplification + doctrine + dead code | not started | | | do FIRST: takes each later retirement from ~39 lines to ~1 |
| 2 | Machine-facing verbs (keep one JSON surface) | not started | | | |
| 3 | Debug mode | not started | | | also fixes unconditional `DEBUG_CSS` |
| 4 | Publishing + web-platform ops | not started | | | `image_opt` lands here, once |
| 5 | The deck engine | not started | | | biggest churn win: 122 commits |
| 6 | Reactive tail, R, Chrome kill | not started | | | drops 2 gate runtimes |
| 7 | Vocabulary contraction | not started | | | needs wave 1 first |
| 8 | CLI ergonomics + scaffolding | not started | | | keep `doctor.rs` |
| 9 | Diagnostics catalogue (keep lint front door) | not started | | | save `codes.rs` prose first |
| 10 | LSP long tail | not started | | | |
| 11 | Serve layer, opened once | not started | | | only wave that opens `exec_pool.rs` |
| 12 | Justification layer (corpus, docs, tests) | not started | | | genuinely last |
| 13 | **`taliesin run`** (unadjudicated, 2,406 lines) | not started | | | needs an adjudication pass first |

## Open items carried forward

- **Wave 13 is not planned yet.** `run_cmd.rs` (513), `run_print.rs` (820),
  `runspec.rs` (328), `run_control.rs` (238), `session.rs` (242), `http1.rs` (265) fell
  through the bundle partition. `runspec.rs` and `run_control.rs` are **not** run-only
  (the preview server's Run buttons use them) so they survive regardless. Adjudicate
  before executing.
- **File-to-bundle collisions to resolve before waves 4/5/7/11/12:** `card.rs` and
  `manifest.rs` (publishing vs site-layer), `image_opt.rs` (publishing vs
  content-shortcodes), `corpus/course/` + `corpus/tarn/` (justification deletes them
  while theorems and narrative rewrite them), `docs/guide/using/from-quarto.tmd`
  (anti-drift deletes it while six bundles add rows to it). Assign each to exactly one
  wave.
- **Re-measure the warm pool on the preview path** before wave 11 if you want the
  number. The directive says cut regardless; measure only if you want to know the cost.

## Hedges to take before wave 1

- [x] `git tag pre-cut f6dee87d` — **done 2026-08-08.** Everything is recoverable from it.
- [x] `notes/retired/diagnostics-explanations.rs` — **done 2026-08-08**, the full 1,222-line
      `codes.rs` preserved verbatim before wave 9 touches it.
- [x] Entry points wired — **done 2026-08-08.** `CLAUDE.md` now opens with the cut banner
      (it previously steered a fresh session toward *growing* the tool), `notes/README.md`
      indexes these three files, and `notes/ROADMAP.md` is marked paused.
- [ ] Write `tools/build-site.sh` before `mounts:` goes, and wire it into
      `.githooks/pre-push`. `build.rs:1651` records that the shell-script alternative is
      what once shipped this project's own call-to-action with a 404.
- [ ] Decide whether to keep one browser smoke test. After wave 6 there is no automated
      browser test net at all, and nothing tests that a `{js}` cell's teardown runs on a
      block diff.

## Log

_Append one entry per wave: date, branch, commit, measured `git diff --stat`, gates
result, and anything that surprised you._
