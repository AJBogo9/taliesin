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
6. **A retirement costs ONE register entry. Do not write a tombstone test for it**
   (`RETIRED_KEYS` / `RETIRED_DIV_CLASSES` / `RETIRED_COMMANDS`; wave 1 made all three
   derived). Do not add a migration-page row either — that page is gone.
7. **Run `./tools/gates.sh` on the tree you are about to commit.** A run started before
   the last edit certifies the tree it saw, not the one you push. `.tmd` files under
   `docs/` are read by `taliesin-core`'s tests, so a "docs-only" edit is a code-gate
   edit.

## Baseline

| | |
|---|---|
| Start commit | `f6dee87d` |
| Safety tag | `pre-cut` (create with `git tag pre-cut f6dee87d`) |
| `cargo test --workspace` | 123 suites, 2,318 passed, 0 failed, 0 ignored, exit 0 |
| `./tools/gates.sh` | **GREEN, measured 2026-08-08 at `3ccfa595`.** All 9 gates ran, all 7 canaries `ok`, **127 suites / 2,339 passed / 0 failed / 0 ignored**, exit 0. |

**How to run the gate here (wave 0's one real finding).** `./tools/gates.sh` on its own
**exits 2 at preflight and certifies nothing**: it defaults `PY` to `python3`, and this
machine's system `python3` has no `ipykernel` (the repo's `.venv` is what does). The
honest invocation is

```sh
TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh
```

Everything else was already present: R+IRkernel, node/npx/npm, google-chrome,
cargo-audit, cargo-deny. Wall clock ≈ 25 min (`--test-threads=1`, three Chrome-backed
canaries, a full `npm ci` for the companion). The 2,339 figure is 21 higher than the
`cargo test --workspace` baseline above because `--features taliesin-server/headless-js`
builds four browser test binaries that a default-feature run silently skips — which is
the whole reason this script exists.
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
| 0 | Establish `gates.sh` baseline + `pre-cut` tag | **done** 2026-08-08 | `main` | n/a | green at `3ccfa595`; needs `TALIESIN_PYTHON` (see Baseline) |
| 1 | Anti-drift simplification + doctrine + dead code | **done** 2026-08-08 | `cut/wave-1-antidrift` | **−1,118** (+400 / −1,518, 30 files) | a retirement now costs ONE register line; **no tombstone test is owed** |
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
  while theorems and narrative rewrite them). Assign each to exactly one wave.
  - **`docs/guide/using/from-quarto.tmd` — RESOLVED, assigned to wave 1, deleted there.**
    Six later bundles have a removal step saying "add the migration row to
    `from-quarto.tmd`". **Skip every one of them; the page does not exist.** The register
    entry IS the migration note now, and it reaches the author with a file and a line at
    the moment they hit the retired key, which no page can beat.
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

### Wave 0 — 2026-08-08, `main`, commit `3ccfa595` (docs) + this entry

`./tools/gates.sh` had never been run in this repo. It is **green**: 9/9 gates ran,
7/7 canaries `ok`, 127 suites, 2,339 passed, 0 failed, 0 ignored, exit 0.

Surprises, all recorded above:

1. **The script refuses to start without `TALIESIN_PYTHON`.** System `python3` has no
   `ipykernel`; preflight is a hard failure by design, so the "bare `cargo test` is
   green and means little" trap the script was written against had a sibling nobody had
   met: `gates.sh` itself is inert here unless you point it at `.venv/bin/python`.
2. **The real suite is 21 tests larger than the ruling's baseline** (2,339 vs 2,318).
   `cargo test --workspace` without `--features taliesin-server/headless-js` never
   builds `read_run_js`, `print_pdf`, `deck_browser` or `reactive_browser`. Every
   later wave must be measured against 2,339, not 2,318.
3. Nothing failed, so wave 1 is unblocked.

### Wave 1 — 2026-08-08, `cut/wave-1-antidrift`

**Measured reclaim: −1,118 lines** (`+400 / −1,518` over 30 files) against the ~1,200
estimate. By area: `crates/core/tests` −437, `crates/server` −433, `docs` −162,
`crates/core/src` −96, `CLAUDE.md` +23 (doctrine, deliberately).

**What actually changed for later waves, which is the point of the wave:** a retirement
used to cost a scoped register entry *plus* a ~39-line hand-written tombstone *plus* a
row on the migration page. It now costs **one register entry and nothing else.**
`render::validate`'s
`every_retired_vocabulary_name_is_gone_unstyled_and_diagnosed_without_a_did_you_mean`
iterates `RETIRED_DIV_CLASSES` and `RETIRED_KEYS` and derives all three properties (gone
from the live vocabulary, warns with the register's own note and never a "did you mean",
CSS rule gone). Waves 5 and 7 add roughly 30 entries between them; that is ~1,100 lines
of tombstone never written.

**Five things that were not true, found by doing it.** These are the reason to keep
reading rather than trusting the playbook verbatim:

1. **The playbook's own STEP 0 verification claim is false, and I could not fix it
   cheaply.** It says adding `CLAUDE.md` to `stale_docs.rs`'s walk "would have caught its
   dead reference to `sentences.rs`/`backlinks.rs`". It would not. The path gate reads
   *backticked* tokens; that reference lives in the fenced "Where things are" map, which
   the extractor sees as one token full of spaces and discards. Measured: adding CLAUDE.md
   subjects **11** backticked path claims to the gate, and zero of them was the stale one.
   The stale reference is fixed and the walk is widened (both worth doing), but the
   fenced map — the densest set of path claims in the repo — **is still ungated**.
   Widening the extractor was declined: this wave removes machinery.
2. **Collapsing the register prose is not cosmetic; four gates caught real losses.** The
   one-sentence rewrite silently dropped "site-wide markup goes in `_site.yml head:`" from
   both `include-*-body` notes and the `numbered:` mention from the config `theorems:`
   note — instructions an author needs. All four were restored. Only one gate was itself
   wrong: `every_retired_config_key_explains_itself_instead_of_guessing` pinned `output:`
   on the phrase "wrote the default", which is *justification*, not instruction; its
   needle is now `--out`. **If a later wave collapses prose, expect the gates to be right
   and the collapse to be wrong.**
3. **A naive CSS-selector derivation fails today, not hypothetically.** The obvious
   `css.contains(".{name}")` reports the retired `.column` as still styled, because it
   matches `.column-margin`, its own successor. The table-driven test uses a
   boundary-aware check and pins both directions, and both halves were proven able to
   fail before being trusted.
4. **Two more comments asserting something that was not so**, the same genus as the
   deck's click-to-source comment in §8 of the ruling. `deck_slide_blocks`' doc comment
   said "the live deck block diff runs on this projection" — it had zero non-test callers,
   so the deck's live diff never used it. `Site::archive_name`'s said "the topbar links to
   it" — the topbar button was deleted on 2026-08-04, so every book build since has
   written a ZIP nothing pointed at. Both are now deleted rather than corrected.
5. **The manual was teaching two retired keys.** `docs/guide/using/recipes.tmd` showed
   `hero.image:`/`image-alt:` (retired 2026-08-02) and three pages showed `listing.sort:`
   (retired the same day), with a whole callout explaining `"date asc"`. Nothing caught
   this: `shipped_docs_do_not_use_a_retired_front_matter_key` skips indented keys by
   design, and that gap is documented in its own doc comment. Fixed here, in scope,
   because deleting the migration page while the manual still taught the retired spelling
   would have been strictly worse. **Before wave 7 retires more nested vocabulary, sweep
   `docs/guide` for indented retired keys by hand — no gate does it.**

**What was given up, stated plainly.** `release_targets.rs` went per the playbook. Its
first test (README platform matrix vs the release workflow) is genuine anti-drift and no
loss. Its second asserted that the release tarball packages `LICENSE` + `THIRD_PARTY.md`
beside the binary — an AGPL distribution claim, now unguarded. It is cheap to restore as
~10 lines if that matters at release time.
