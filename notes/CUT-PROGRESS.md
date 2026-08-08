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
| 4 | Publishing + web-platform ops | **done** 2026-08-08 | `cut/wave-4-publishing` | **−9,576** (+808 / −10,384, 91 files) | 12 verbs → **10**; `headless_js.rs` went with `pdf.rs`; gates 9 → **8**, canaries 5 → **4** |
| 5 | The deck engine | **done** 2026-08-08 | `cut/wave-5-deck` | **−11,553** (+657 / −12,210, 161 files) | the biggest wave so far; `DocFormat` deleted outright, not collapsed; `code-line-numbers` went with it |
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
- **File-to-bundle collisions to resolve before waves 7/11/12:** `corpus/tarn/`
  (justification deletes it while narrative rewrites it). Assign it to exactly one wave.
  **`corpus/course/` is RESOLVED:** wave 5 took only `lecture.tmd` (its deck) and left the
  book standing, so waves 7 and 12 inherit it whole. **`card.rs`, `manifest.rs` and `image_opt.rs` are
  RESOLVED — all three were deleted in wave 4**, so any later step naming them is spent.
  - **`docs/guide/using/from-quarto.tmd` — RESOLVED, assigned to wave 1, deleted there.**
    Six later bundles have a removal step saying "add the migration row to
    `from-quarto.tmd`". **Skip every one of them; the page does not exist.** The register
    entry IS the migration note now, and it reaches the author with a file and a line at
    the moment they hit the retired key, which no page can beat.
- **Re-measure the warm pool on the preview path** before wave 11 if you want the
  number. The directive says cut regardless; measure only if you want to know the cost.
- **Wave 6's Chrome kill is down to one test binary.** Wave 5 deleted `deck_browser.rs`, so
  `reactive_browser.rs` is the SOLE consumer of `headless-js` + `chromiumoxide`. Both were
  deliberately kept: cutting them here would have meant cutting wave 6's own subject.
  `headless_js_feature.rs`'s binary list is now a one-element `BROWSER_TEST_BINARIES` const,
  so adding or removing one is a row.
- **Wave 6 inherits exactly ONE chrome canary now.** `gates.sh` is down to **4** canaries
  (python, R, node, reactive); wave 4 dropped `CANARY_PRINT` with the print track, and
  `CANARY_REACTIVE` is the only thing left that proves Chrome ran. Waves 2, 3 and 4 each
  dropped a canary without repointing it, per the rule written into `gate_script.rs`'s
  count assertion: *a canary is dropped only when the sole thing it proved goes away, never
  by repointing it at a surviving test.* A later wave that deletes a capability should read
  that string before touching the count.
- **Wave 6's Chrome kill is now nearly done, and what remains is smaller than its plan
  says.** Wave 2 removed the `{js}` observation path from `headless_js.rs` (wave 6's
  STAGE 5) and **wave 4 deleted the whole file** with `pdf.rs`, its only consumer. What is
  left for wave 6: the `deck_browser` / `reactive_browser` test binaries, the
  `headless-js` cargo feature, the `chromiumoxide` dependency, `TALIESIN_REQUIRE_CHROME`,
  and `CANARY_REACTIVE`. Note `release.yml` **no longer passes `--features
  taliesin-server/headless-js`**: nothing a released binary can run touches the driver
  since `pdf` went, so paying 24% of every cross-build for it was pure waste.
  `headless_js_feature.rs` now asserts the *absence* of that flag on the release build.
- **Before wave 7 retires more nested vocabulary, sweep `docs/guide` for indented retired
  keys by hand.** Still open — carried from wave 1; no gate sees an indented key. Wave 4
  hit this exact hole: retiring the `orcid:`/`email:` author sub-keys needed a hand edit of
  `docs/guide/reference/frontmatter.tmd` and `corpus/structured-authors/paper.tmd`, and no
  gate would have caught either.
- **Decide whether `Site::nav_ordered` still belongs where it is.** It lived in `llms.rs`
  and moved to `feed.rs` in wave 4 because `feed_hosts` was its other caller. If wave 11's
  site-layer reduction touches feeds, that is the moment to look at it again.

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
- [ ] **A fence attribute has no validator at all, and `code-line-numbers=` just became the
      first retirement that spelling cannot report.** The `#|` form answers with its
      `RETIRED_KEYS` note; `{.python code-line-numbers="1|2"}` is silent. Every other fence
      attribute was always silent, so this is a pre-existing hole a retirement walked into
      rather than a new one — but a later wave retiring a fence-attribute vocabulary should
      know it is retiring into silence.
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

### Wave 4 — 2026-08-08, `cut/wave-4-publishing`

**Measured reclaim: −9,576 lines** (`+808 / −10,384` over 91 files, `git show --numstat`
on the commit itself) against the ~8,190 estimate, plus a **451,664-byte** Newsreader TTF
that no line count sees. **Excluding `notes/` — which grew by 254 lines on purpose, for the
preserved paged.js traps and this entry — the code reclaim is −9,831** (`+514 / −10,345`).
By area: `crates/server/src` −3,667, `crates/core/src` −3,286, `crates/server/tests`
−1,149, `crates/core/tests` −465, `crates/core/assets` −386, `docs` −100, `corpus` −73,
root md/toml −50, `web-client` −31, `editor` −18, `tools` −15.

**`./tools/gates.sh` is GREEN on the committed tree:** **8/8** gates, **4/4** canaries,
**105 suites / 2,037 passed / 0 failed / 0 ignored**, exit 0. Measure wave 5 against
**2,037, four canaries and eight gates**. (A bare default-feature `cargo test --workspace`
is **103 suites / 2,027 passed**.)

**Twelve CLI verbs are now ten.** `publish` and `pdf` are gone, one `RETIRED_COMMANDS`
line each. The `publish:` config key got one scoped `RETIRED_KEYS` entry under the
`config key` scope, and the bundled `editor/vscode/schema/tali-site.schema.json` copy was
re-synced by hand — `cargo test --workspace` stayed green while it was stale, exactly as
CLAUDE.md warns, and only the companion's `node --test` catches it.

**THE SWEEP, VERIFIED IN BOTH DIRECTIONS, because it was the whole risk of this wave.**
`find <out> -type f | sort` over `docs/internals/_book` + `corpus/tech-blog/_site` before
and after: **117 files → 77**. Every one of the 40 removals is on the allowed list —
16 `og/*.png`, 2 `manifest.webmanifest`, 3 `icon-*.png`, 2 `llms*.txt`,
5 `*.citations.json`, 12 `*.avif` — plus the expected `app.<hash>.js` rename. **No
`.html`, no `search-index.js`, no `sitemap.xml`, no `robots.txt`, no Atom feed and no
mirrored source asset disappeared.** Then the second direction, which a fresh-directory
build cannot show: rebuilding **in place** over the existing tree produced a byte-identical
file list, so no surviving keep-contributor lost its writer.

**Six things that were not true, or that the playbook did not know.** Same genus as waves
1–3:

1. **`headless_js.rs` was `pdf.rs`'s alone, and all 559 lines went with it.** The
   playbook's "must survive" list named five non-pdf consumers of the launch policy; three
   no longer existed (`read --run-js` and `query.rs` went in wave 2, the LSP's rasterized
   KaTeX hover in wave 4.1) and the other two, `deck_browser.rs` and `reactive_browser.rs`,
   use `chromiumoxide` **directly** — they only mirror `chrome_path` in a comment.
   `every_browser_await_is_bounded` went with the file, correctly: it scanned only those
   two files. The `headless-js` feature and the `chromiumoxide` dep stay, because both test
   binaries declare `required-features`.
2. **So `release.yml` should no longer ask for the driver, and now does not.** With no
   runtime consumer left, `--features taliesin-server/headless-js` cost 24% of every
   cross-build for a dependency the shipped binary cannot reach.
   `headless_js_feature.rs`'s clause (c) is inverted rather than deleted: it now fails if
   the flag creeps *back* onto the release build. Its old rationale ("otherwise every
   published binary silently lacks `read --run-js`") had been false since wave 2.
3. **`Site::nav_ordered` lived in `llms.rs` and `feed.rs` calls it.** Deleting `llms.rs`
   broke the Atom feeds' page ordering at compile time — loudly, so no harm, but nothing in
   the plan mentioned it. It now lives in `feed.rs`, which was always its other caller.
4. **`docs/DIAGNOSTICS.md` is a GENERATED golden and I edited it by hand first.** The three
   "or `publish`" removals belong in `codes.rs`'s `Explanation` prose; editing the output
   made `diagnostics_md_matches_committed` fail, which is the gate working. Fixed at the
   source, then re-blessed with `TALIESIN_BLESS=1`. **This is wave 3's own recorded
   precedent** ("retire a diagnostic code by deleting its rows, its `Explanation` and its
   pin, and re-bless"), and I walked past it once.
5. **Two `deny.toml` advisory ignores died with their dependencies**, and cargo-deny says
   so out loud rather than silently: `RUSTSEC-2026-0192` (ttf-parser, via `ab_glyph`) went
   with the card rasterizer, and `RUSTSEC-2024-0436` (paste, via `image`'s `avif` feature →
   `ravif` → `rav1e`) went with `image_opt`. An un-encountered ignore is a warning, not a
   failure, so a wave that removes a dependency should check that log.
6. **`Page.authors`, `Page.has_bibliography` and the `orcid:`/`email:` author sub-keys all
   became dead in the same edit.** Their only reader was `jsonld_head`. The first two were
   deleted outright (clippy caught `authors`; `has_bibliography` is `pub`, so nothing would
   have). The two sub-keys got `RETIRED_KEYS` entries under the `author key` scope — a key
   left parsed, honored-looking and inert is precisely what the site-level `image:`
   retirement was written against. `author:` itself, `affiliation:`, `url:`, `equal:` and
   `contribution:` are untouched and still render.

**The two judgement calls, and how they went.**

- **`og:image` feeds from the page's own front-matter `image:`**, as ruled. The retired
  site-level `image:` in `config/mod.rs` stays retired. The emitter is **6 tags, ~45
  lines**: `og:title`, `og:description`, `og:url`, `og:image`, `twitter:card`, and
  `<meta name="description">`. That last one is the one addition to the ruling's list of
  five, at one line: it is what a search result actually reads, which is the same job the
  surviving `seo.rs` sitemap exists to do. An absolute `image:` is used verbatim; a page
  with none degrades to `twitter:card: summary`; the 404 page advertises nothing.
- **The paged.js traps were preserved before deletion, as a step and not a nicety.**
  `notes/retired/paged-js-traps.md` carries the `PAGED_CONFIG`, `PAGED_START`, `eager_media`
  and `max_float_height` comments verbatim. `notes/ROADMAP.md`'s `print-pdf-track` entry
  now points at it and is closed as CUT; `build-seo-completeness` is closed as CUT-DOWN
  naming exactly what survived.

**What was given up, stated plainly.**

**AVIF (`image_opt.rs`), which the auditor's own dissent said was the one line of the
verdict they would concede.** What goes with it, measured rather than asserted: **513 kB
saved on one 17-page site**, 12 derivative files, and about **7% of the original PNG**
on this repository's images (a 294 KB screenshot shipped as 22 KB). It had no vocabulary,
no verb, no config key and no doc page, so the demand-signal test this whole audit runs on
could never have been applied to it — a writer never met it. Its module header recorded
real evidence for the encoder choice (image-webp is lossless-only; libwebp needs a C
toolchain on the macOS cross-builds; ravif hard-fails without nasm; q72/s4 came from three
measured encodes on a real corpus image), and that reasoning cost something to acquire.
The case against it rested on `_freeze/` co-tenancy and the sweep interaction — arguments
about where the code lived, not about whether a reader benefited — and an hour moving
`CACHE_SUBDIR` out of `_freeze/` would have neutralised both. Ruled cut by the author on
2026-08-08 and executed. `_freeze/` is `freeze.rs`'s alone again. The docs now tell the
truth: `taliesin build` copies your bytes across unchanged, and shrinking an image is
yours to do before you reference it.

**`taliesin publish`'s passcode-gated draft workflow — the one genuinely writer-shaped
thing in this bundle.** "Send my editor a private link to an unfinished post" is a real
need that a generic static host does not hand you free, and `cli.tmd`'s publish section
was the only place in the docs describing a complete writer workflow end to end. Zero
adoption and an external wrangler dependency made it the wrong place to spend a finite
perfection budget, but the need does not disappear with the code. The host-agnostic
publishing prose was kept and **expanded** so it reads as the recommendation rather than
the fallback: it now opens by saying a folder of files *is* the whole publishing story,
carries a copy-paste deploy script, and answers the private-draft case directly (every
host above it has password protection; or send the folder, which is often what "let me
read your draft" means). It also answers printing: open the built page and print it.

**Dropping `minify_js` costs real bytes, and more than the playbook implies.** Measured on
`corpus/tech-blog`, the shared `app.js` goes **46,329 → 93,684 bytes raw, and 13,176 →
29,669 gzipped — +16,493 gzipped bytes per site.** It is one shared asset, not per page,
and the CSS bundle is byte-identical at 56,429 (so `minify_css` is untouched and still
pays). But the playbook's "the CSS half carries roughly three quarters of the measured
gzipped saving on its own" was not re-verified here, and the JS quarter was worth 16.5 kB
gzipped on a real site. What is bought for it: 235 lines of stateful JS tokenizer (ASI-safe
newline preservation, regex-vs-division disambiguation, nested template interpolation), its
acorn token-stream oracle over Node, its mutation canary, and the only place in this
repository where a silent mis-tokenization could ship a broken script no page visibly
failed on. If the byte cost ever matters, the honest fix is `esbuild --minify` at build
time, not re-deriving the tokenizer.

**Three smaller losses.** The PWA install path is gone (`manifest.webmanifest`, the three
bundled icons, and the "bring your own icon" documentation), so a reader can no longer
install a built site as an app — no service worker ever shipped with it, so this only
changed how a reader *returns*, never whether the site worked. `llms.txt`/`llms-full.txt`
and the per-page `<page>.citations.json` sidecar are gone, so a crawler reading a built
site now reads its HTML like everyone else. And JSON-LD went entirely, which is the one
structured-data surface a search engine reads directly — `sitemap.xml`, `robots.txt`, the
Atom feeds and the OpenGraph block all survive, so discoverability is reduced rather than
removed.

### Wave 5 — 2026-08-08, `cut/wave-5-deck`

**Measured reclaim: −11,553 lines** (`+657 / −12,210` over 161 files) against the ~8,500
estimate — the largest wave so far, ahead of wave 4's −9,576. By area:
`crates/core/assets` −3,902 (deck.js 2,720 + deck.css 1,129 + the `.tali-embed` block and
the client trims), `crates/core/src` −3,036, `crates/server/tests` −1,366,
`crates/server/src` −717, `crates/core/tests` −710, `docs/guide` −564, `corpus` −316,
`samples` −281, `docs/internals` −265, `web-client` −145, `site` −140, `tools` −75,
`editor` −16, root/docs −20. **25 files deleted outright.**

**`./tools/gates.sh` is GREEN on the committed tree:** **8/8** gates, **4/4** canaries,
**97 suites / 1,932 passed / 0 failed / 0 ignored**, exit 0. Measure wave 6 against
**1,932, four canaries and eight gates**. (A bare default-feature `cargo test --workspace`
is **95 suites / 1,927 passed**.)

**Ten CLI verbs are still ten.** This wave cut no verb — it cut a `new` KIND. `taliesin new
deck` answers with a one-sentence removal note from a hand-written arm in `NewKind::parse`,
not a did-you-mean, because `deck` is edit-distance 3 from `page` and a did-you-mean would
have sent the author to a scaffold that writes something else.

**THE VOCABULARY, WHICH THE PLAYBOOK CORRECTLY CALLED THE SILENT FAILURE MODE — PROVEN, NOT
ASSERTED.** Eleven register entries went in, and each was verified by authoring a scratch
`.tmd` and running `taliesin check` on it rather than by grepping for absence:

- `RETIRED_DIV_CLASSES` ×6 (`fragment`, `incremental`, `notes`, `fade-out`, `highlight`,
  `magic-move`) — a scratch doc with all three of `.fragment`/`.magic-move`/`.notes`
  produced three located `TAL-DIV-CLASS` warnings naming the date and the successor.
- `RETIRED_KEYS` ×3 under `front-matter key` (`format`, `footer`, `logo`) — a scratch doc
  with `format: deck` + `footer:` + `logo:` produced three located `TAL-FM-KEY` warnings.
- `RETIRED_KEYS` ×1 under a NEW `cell option` scope (`code-line-numbers`, see finding 1).
- One hand arm for `new deck`, verified by running the command.

`corpus/diagnostics/typos.tmd` now carries a `::: {.fragment}` as a permanent witness, so
the retirement path is exercised by the corpus and not only by a scratch file.

**Six things that were not true, or that the playbook did not know.** Same genus as waves
1–4:

1. **`code-line-numbers` was deck-only, and nothing in the plan said so.** Its
   `data-code-lines` attribute was read by `deck.js` alone, and `.code-walkthrough` — the
   feature the guide claims it serves — calls `wrap_pre_lines` on its panel unconditionally
   and never reads the option. Left in place it would have been exactly wave 4's
   `orcid:`/`email:` failure: parsed, honored-looking and inert. Cut, and retired under a
   scope `RETIRED_KEYS` did not have before. **Its fence-attribute spelling stays silent**
   (`{.python code-line-numbers="1|2"}`) because this tree has no fence-attribute validator
   at all; the register note says so in its own sentence.
2. **`DocFormat` was deleted, not collapsed to one variant.** A one-variant enum threaded
   through `RenderedDoc.format`, `validate_a11y(blocks, format)`,
   `validate_document_shape(blocks, format)`, `page_static_diagnostics` and `preview_diag`
   is ceremony that says nothing. Deleting it took the parameter off both diagnostics
   entry points and took `BuildResult::Refused` with it — that variant's only case in the
   whole tree was `--bare` on a deck.
3. **`format:` is retired as a KEY, not as a value, and that was the cheap direction.** The
   ruling asked for a retired-VALUE note for `format: deck`; there is no value register, so
   that would have meant new machinery for one entry. Retiring the key costs one line and
   answers `format: deck`, `format: pdf` AND `format: revealjs` with one sentence — so
   `NON_HTML_FORMATS` (12 names), `validate_format_value`, `validate_format_subkeys`, the
   `revealjs`→`deck` did-you-mean and the whole `TAL-FM-FORMAT` diagnostic went too.
   `docs/DIAGNOSTICS.md` was re-blessed from `codes.rs` with `TALIESIN_BLESS=1`, per wave
   3's recorded precedent — this time without editing the golden by hand first.
4. **`token_contract.rs`'s browser census scans Rust files containing `<script>`, which
   includes `render/tests.rs`.** `data-code-lines` stayed in the ACTUAL census after every
   `.js`/`.css` reference was gone, because two *test string literals* were keeping it
   alive. Worth knowing before the next census diff: the census is not asset-only.
5. **NO `RETIRED_SHORTCODE` register was added, deliberately.** The shortcode vocabulary is
   CLOSED, so a leftover `{{< embed x.tmd >}}` already gets a located, named "unknown
   shortcode … (left as literal text)" warning — and the corpus proved it, by failing
   `every_corpus_doc_emits_no_unknown_key_warnings` on the one `{{< embed lecture.tmd >}}`
   left in `corpus/course/em.tmd`. There is no silence to prevent, so a fourth register
   serving one entry is the machinery wave 3 declined for `TAL-DEBUG-TRACE`. The playbook's
   CUT C4 proposes one for `{{< video >}}` in wave 7; that proposal should be re-examined
   against this, since the same closed-vocabulary warning covers it.
6. **`tools/ui-audit/` had a whole deck probe and a deck readiness gate, and no gate reads
   it.** `probeDeck` waited on `window.TaliesinDeck.isReady()`, `browser.mjs`'s settle
   predicate ANDed in a `deckOk` term, and `units.mjs` carried a `detectFormat` whose only
   job was to answer "deck". All of it could only ever hang or no-op after this cut, and
   `cargo test` would never have said so.

**The judgement call the playbook flagged, and the fact that resolved it.**

The dissent and the ruling both call the embedded deck "the marketing site's single live
interactive artefact" and make deleting `site/demo.tmd` conditional on replacing it.
**That premise is false, and reading the page is what showed it.** `site/index.tmd:19`
already opens with a live Three.js `{js}` scene *above the fold* — the reader spins a
surface before they reach the deck section at :118 — and `site/showcase.tmd` carries five
more live exhibits, including the equation-plus-plot and the Lorenz attractor that wave 3
recorded as surviving. So the landing page needed no replacement artefact. The
"A real slide deck, embedded" section is deleted, the closing "nothing here a screenshot
could fake" paragraph now names the surface the reader already spun, and the feature grid
drops to three cards. `site/formats.tmd` goes from **"One source, four outputs"** to three,
losing its `## Slide decks` section and its hero copy with it. `docs/guide/index.tmd`'s
"60-second tour" is now a 60-second *description* of the edit loop, which is what the tour
deck was demonstrating anyway.

**What was given up, stated plainly.**

**The layout engine and the bug class it was built against.** `deck_browser.rs`'s own
header recorded that its first real-browser run found two shipped defects that 300-plus
server-side emission assertions had all passed over: code blocks clipped off the right edge
of 5 of 21 slides, and a focus ring painted around every slide in a vertical stack. That is
direct evidence that scale-to-fit slide layout has a failure mode invisible to the kind of
test this repo is good at, and `deck.js`'s 372 lines of fit logic were hard-won against it.
Rebuilding later means rediscovering the bug class and rebuilding the CDP harness that
catches it.

**`deck_qr_golden.rs` and the encoder under it.** ~180 lines of table-driven Reed-Solomon
over GF(256), format/version bit tables and ISO/IEC 18004 mask-penalty scoring, verified
bit-for-bit against a reference encoder off-repo and pinned here by fingerprint. It was the
one deck capability with real algorithmic surface, and the golden was its only net.

**A use case, not a decoration** — the point the dissent made and the only one in the audit
that applies. A writer who gives talks got slides from the same source, the same warm
kernel and the same offline bundle, with no second tool. That is gone; a talk is now a page.

**Three documents a person wrote to be read:** `samples/deck.tmd` ("Decisions in the Room",
the business-value sample that exercised every slide feature), `docs/guide/tour.tmd` and
`docs/guide/demo.tmd`. `corpus/course/` survives whole apart from its `lecture.tmd`.

**Two gates got strictly stronger, and nothing had to be fixed for it.** Both diagnostics
exemptions are gone (`a11y.rs`'s heading-skip and `shape.rs`'s whole document-shape family),
so every page now reaches every rule. `a11y_outline.rs`'s book walk lost its deck branch —
its own doc comment said the exemption let two files pass "by construction" — and the corpus
and both books were clean under the newly-armed rules on the first run.

**Measured, not asserted.** `corpus/tech-blog` rebuilt with the release binary:
`_assets/app.<hash>.css` **56,429 → 54,951 bytes (−1,478 off every page)**, which is the
`.tali-embed` iframe block leaving `base.css`; `app.<hash>.js` 93,684 → 93,172 (−512). An
**in-place** rebuild over the existing output produced a byte-identical 53-file list, so no
surviving keep-contributor lost its writer, and no `deck`/`embed` artefact remains in the
output tree.
