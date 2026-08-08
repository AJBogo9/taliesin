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
| 2 | Machine-facing verbs (keep one JSON surface) | **done** 2026-08-08 | `cut/wave-2-machine-verbs` | **−7,796** (+685 / −8,481, 79 files) | 18 verbs → **12**; `map` + `vocab` re-homed into the LSP |
| 3 | Debug mode | **done** 2026-08-08 | `cut/wave-3-debug` | **−5,374** (+118 / −5,492, 45 files) | `DEBUG_CSS` was ungated in **three** places, not two; proven **−7,245 B** of shipped CSS per page |
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
- **Wave 6 inherits TWO chrome canaries, and wave 3 confirmed both still fire.** `gates.sh`
  is down to **5** canaries (python, R, node, reactive, print). Wave 3 dropped
  `CANARY_DEBUG_TRACE` without repointing it, per the precedent wave 2 set — see the
  reasoning now written into `gate_script.rs`'s count assertion, which states the rule
  outright: *a canary is dropped only when the sole thing it proved goes away, never by
  repointing it at a surviving test.* A later wave that deletes a capability should read
  that string before touching the count.
- **Wave 6's Chrome kill now has one fewer thing to delete and one more to check.** Wave 2
  removed the `{js}` observation path from `headless_js.rs` (that was wave 6's STAGE 5), so
  what is left there is only the launch policy `pdf.rs` uses. Wave 6 also inherits the two
  surviving chrome canaries in `gates.sh` (reactive + print), not three.
- **Before wave 7 retires more nested vocabulary, sweep `docs/guide` for indented retired
  keys by hand.** Still open — carried from wave 1; no gate sees an indented key.

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

### Wave 2 — 2026-08-08, `cut/wave-2-machine-verbs`

**Measured reclaim: −7,796 lines** (`+685 / −8,481` over 79 files) against the ~6,900
estimate. By area (disjoint, summing to −7,796): `crates/server/src` −2,491,
`crates/core/assets` −2,002 (the two golden dumps), `crates/core/src` −1,388,
`crates/server/tests` −1,311, `docs` −305, `editor/vscode` −133, `crates/core/tests` −102,
`corpus` −56, `tools`/`CLAUDE.md`/`README.md` −4.

**`./tools/gates.sh` is GREEN on the committed tree:** 9/9 gates, **6/6** canaries,
**114 suites / 2,238 passed / 0 failed / 0 ignored**, exit 0. Measure the next wave against
2,238 and six canaries, not 2,339 and seven — this wave deleted ~101 tests along with the
verbs and dropped `CANARY_CHROME` (see finding 3).

**Eighteen CLI verbs are now twelve.** `read`, `map`, `features`, `vocab`, `schema` and
`mcp` are gone, each with a one-line `RETIRED_COMMANDS` entry and nothing else — the wave 1
machinery held. `check --format json` is the surviving machine surface, as ruled.

**Five things that were not true, or that the playbook got wrong.** Same genus as wave 1's
list, and the reason to keep reading rather than executing the playbook verbatim:

1. **The playbook's `text.rs` instructions contradict themselves, and following both would
   have changed `llms-full.txt`.** STEP 4(d) says to delete `classify_exec_output`/
   `ExecOutput` *and* to "change nothing between lines 63 and 599" — but `project_block`,
   which lives at 63, **calls** `classify_exec_output`. It is not `read`-only: it is what
   turns an executed cell's output block into `[figure fig-x: produced]` in the projection
   that `site/llms.rs` ships as `llms-full.txt`. Both are kept, demoted from `pub` to
   module-private. Only `project_with_js` (the headless `{js}` interleave) really was
   verb-only and went.
2. **The dissent's mitigation was already half paid, and the other half was missing.**
   `read_run.rs` contains **no freeze-cache assertions at all** — the two the dissent asked
   to "port" do not exist there. What does exist is `freeze_cold_replay.rs`, already
   build-driven and already covering "a cold replay hits the cache" and "an edited cell
   busts". The genuinely unguarded property was the **downstream** half: cell 2's source is
   byte-identical across both builds, so a key over that code alone would replay yesterday's
   number. `editing_an_upstream_cell_re_executes_the_cells_below_it` now drives that through
   a real `build` (proven able to fail: expecting the stale `3458` panics).
3. **The chrome canary was dropped, not repointed.** `read_run_js` was the *only* test of
   the capability it stood for, so pointing `CANARY_CHROME` at a surviving browser test
   would have made two canaries prove the same thing. `gates.sh` is down to **6 canaries**
   and the chrome gate is unchanged: `CANARY_REACTIVE` and `CANARY_PRINT` both still fail
   when Chrome is missing. `gate_script.rs`'s count and prose follow the precedent already
   written into that assertion.
4. **Deleting the `map` verb broke a companion unit test that could not be re-pointed.**
   `map.ts` now imports `vscode` (for `Uri.file`) and `./client`, both of which use the
   `vscode` module as a *value* — so `out/test/map.test.js` can no longer load under plain
   `node --test`. The file is deleted; its validation half (`sitePages`) still has its own
   tests in `paths.test.ts`, now taking a parsed value instead of a JSON string, and the
   wire half is covered in Rust by a new `lsp_stdio.rs` test over `corpus/demo-book`
   (chapter order + the `draft: true` appendix excluded — what `map_cli.rs`'s 242 lines were
   really for).
5. **`manifest.test.ts` read the deleted vocabulary dump.** Its snippet gates checked every
   callout kind, div class, cell option and xref prefix a shipped snippet inserts against
   `tali-vocab.json`. They now parse the Rust consts directly (`CALLOUT_KINDS`,
   `THEOREM_KINDS`, `CELL_OPTION_KEYS`, `DIV_CLASS_NAMES`, `XREF_LABELS` minus
   `RETIRED_XREF_PREFIXES`) with the same regex technique `cargoCommands()` already used —
   one indirection fewer, since the JSON was generated from those lists. Verified the parsed
   sets by hand: 3 callouts, 5 theorem kinds, 14 cell options, 9 div classes, 9 live xref
   prefixes.

**Dead code the cut exposed, removed in the same commit:** `build::build_json` and
`check::check_json` (the MCP `build`/`check` tool wrappers — note `check --format json`
goes through `cmd_check`, not through these), `cite::xref_prefixes`,
`vocab::{cell_language_names, div_attribute_names}`, `extension::{shortcode_argument_names,
scan_shortcodes}`, `divs::{scan_div_attrs, scan_code_fences}`, `headless_js::chrome_available`.

**Corrections landed for later waves.** `CLAUDE.md`'s `vocab.rs` note said 11-of-16;
re-measured and it is **9-of-14**. The five-drift-gate paragraph is now four (the
`agents_md` golden went with `AGENTS.md`). The banner records 12 verbs.

**What was given up, stated plainly.** Three things.
`missing_input_suggests.rs`'s floor drops from four front doors to **two** (`build`,
`check`): `read` and `map` were half that gate's population, and `run`/`pdf` still do not
suggest a near-miss — a pre-existing gap, now a larger fraction of the surface.
`corpus/reader/xref-targets.tmd` went with `map`; the cell-label→xref-target path it pinned
is still covered by `corpus/analyst` (executed `#| label: tbl-` floats, cross-page) and by
`lsp_nav.rs`'s definition-site tests, so this is a fixture loss rather than a coverage loss.
And the author's own agent loop drops from a turnkey MCP server to shelling out to
`check --format json`, which is the trade the ruling made deliberately.

### Wave 3 — 2026-08-08, `cut/wave-3-debug`

**Measured reclaim: −5,374 lines** (`+118 / −5,492` over 45 files) against the ~5,057
estimate. By area: `crates/core/assets` −1,762 (debug.js 1,298 + debug.css 383 + the
tali-js/globals trim), `crates/server` −1,146 (trace_py 350, debug_trace.rs 742, exec.rs
wiring), `crates/core/src` −890, `crates/core/tests` −479, `docs` −443, `corpus` −417,
`site` −61, `tools`/`web-client` −22.

**`./tools/gates.sh` is GREEN on the committed tree:** 9/9 gates, **5/5** canaries,
**112 suites / 2,193 passed / 0 failed / 0 ignored**, exit 0. Measure wave 4 against
**2,193 and five canaries**. (A bare default-feature `cargo test --workspace` is
**109 suites / 2,175 passed**.)

**THE CSS WIN, MEASURED, NOT ASSERTED.** `cargo build` first (assets are `include_str!`-
compiled), then the same site rebuilt both sides. `corpus/tech-blog` (17 prose pages):
`_assets/app.9215aeb6a5c26dc4.css` **63,674 B** → `app.a9002fdedc5ac85e.css` **56,429 B**.
**−7,245 bytes, −11.4%, off every page the tool ships**, and `.dbg-transport` greps 1 → 0.
The 383 source lines of `debug.css` minify to those 7,245 shipped bytes.

**Five things that were not true, or that the playbook did not know.** Same genus as
waves 1 and 2:

1. **`DEBUG_CSS` was ungated in THREE places, not two.** The playbook and the ruling both
   name `shared_site_css` and `shared_site_css_linked_fonts`. There is a third:
   `page.rs:274`'s `style_block`, the inline `<style>` every **standalone** page uses —
   so `build <file.tmd>` was paying the same 7,245 bytes, and a fix that took only the two
   named sites would have left the single-page path shipping stepper CSS forever with
   every gate green. Found by grepping `BASE_CSS`'s assemblies rather than `DEBUG_CSS`'s
   own line numbers.
2. **`token_contract.rs` had FOUR debug names to remove beyond the two the playbook
   names.** Both lists lost `data-debug-inputs`/`data-debug-name`/`data-tali-js-src` as
   expected, but the browser-selected census also pinned **`data-c` and `data-r`** —
   debug.js's grid-view row/column stamps for `dp.tmd`'s edit-distance table. Nothing in
   the playbook mentions them; the census failure is what found them. Also
   `data-dbg-init`.
3. **The `{js}` cell API surface was larger than the two exports the playbook lists.**
   `runDebugSource` and `onInputChange` were named; `__at` (the `yield`-stamp runtime, the
   other half of `yield_scan.rs`'s contract), `tali.frame(n)` and its `EMPTY_DEBUG_FRAME`
   stand-in, and the whole `window.taliDebug` type declaration were not. All were dead the
   moment `debug.js` went: `taliDebug` is set by nothing, so `tali.frame` could only ever
   return the empty stand-in. Cut per the standing directive. Verified live: on
   `corpus/descent`, `typeof window.taliDebug === "undefined"`, and all three `{js}` cells
   still mount and run.
4. **`text.rs`'s branch is NOT left "unwitnessed", and saying so would have been wrong.**
   The playbook and the ruling both describe `project_block`'s `if let Some(cell)` as
   surviving unwitnessed once `.debug` goes. It is witnessed — `projects_code_cell_fenced`
   drives it with an ordinary top-level `{python}` cell. What actually became unreachable
   is the narrower case of a **container** block carrying a `Cell`, since
   `build_container` now sets `cell: None` unconditionally. The comment left on the branch
   says that, not the weaker claim.
5. **`build_stdout` in `asset_bundle.rs` had exactly one caller** (the deleted debug-gating
   test), so `-D warnings` failed on dead code until it went too. `cargo test` alone was
   green; only clippy caught it.

**The two judgement calls, and how they went.**

- **`RETIRED_DIV_CLASSES` got its `debug` entry**, mandatory as ruled: div classes are an
  open vocabulary, so a leftover `::: {.debug}` would otherwise get silence. One sentence,
  the date, then "nothing replaces it". No tombstone test written — wave 1's derived
  gate covers it.
- **`TAL-DEBUG-TRACE` gets NO register, deliberately.** The three registers exist because
  the AUTHOR writes the retired name in their own source and the tool must answer it. A
  diagnostic code is written by the tool and read out of a message that no longer exists;
  there is no `.tmd` file anywhere containing the string `TAL-DEBUG-TRACE`, so there is no
  silence to prevent. The only stale artefact is a bookmarked
  `DIAGNOSTICS.md#tal-debug-trace` anchor, which is a dead link on a doc page, not a
  silent authoring failure. Adding a fourth register for it would be machinery serving one
  entry. **Precedent for later waves: retire a diagnostic code by deleting its rows, its
  `Explanation` and its pin, and re-bless `docs/DIAGNOSTICS.md` with `TALIESIN_BLESS=1`.**

**What was given up, stated plainly, per the dissent.** `site/showcase.tmd:342-401` is
gone: sixty-one lines in which the author staked the tool's claim on this feature on the
shop window ("this one comes from a real interpreter running the exact ten lines below…
so the diagram and the code can never quietly drift apart"). The demand measurement could
not see it, because `site/` was excluded from the read set by construction. **This is the
one cut in the whole audit that deletes a differentiator rather than shrinking one** — no
competitor has an algorithm stepper. Five showcase exhibits survive, including the two
carrying the same web-native claim (the live equation-plus-plot and the browser-integrated
Lorenz attractor).

**What is expensive to re-derive, recorded because reading the deleted code will not give
it back.** Four field-diagnosed bugs were encoded in `trace_py.rs`, none discoverable by
reading the code:

1. **Nested-container snapshot aliasing** — a snapshot of a mutable container captured by
   reference shows every earlier frame the container's FINAL contents, so the whole trace
   silently reads as if the algorithm already finished.
2. **`Subscript(ctx=Store)` filtering** — `a[i] = x` must not be collected as a *read* of
   `a[i]`; without the store-context filter the reads set names the very cell being
   written.
3. **The `AugAssign` counter-exception, collected by node identity** — `x += 1` is both a
   read and a write of the same name at the same source position, so it needs an explicit
   exception keyed on the AST node's identity rather than on its name or line.
4. **Non-finite floats destroy the WHOLE trace** — bare `json.dumps` emits `Infinity` /
   `NaN`, which is not JSON, so one non-finite value anywhere makes the entire blob
   unparseable client-side and the stepper renders nothing at all, with no error naming
   the cause.

Also lost: `debug.js`'s untyped view inference, the thing the showcase copy called magic
("no annotation told Taliesin those three names were pointers"). Recoverable in full from
the `pre-cut` tag; expensive to re-derive from scratch.

**Verified in a real browser, not asserted.** `corpus/descent` built and served, opened
via chrome-devtools MCP: zero console messages, 3 `{js}` cell scripts, 3 mounted outputs,
3 `data-tali-ran` markers, 0 error boxes, 0 debug widgets, `window.taliDebug` undefined.
The `has_client_cells(body)`-as-sole-gate change was the risk here and it holds in both
directions: a prose page in `corpus/tech-blog` ships 0 copies of the runtime, a `{js}` page
ships it.
