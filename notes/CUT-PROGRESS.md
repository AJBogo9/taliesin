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
| 6 | Reactive tail, R, Chrome kill | **done** 2026-08-08 | `cut/wave-6-reactive-r-chrome` | **−4,146** (+960 / −5,106, 100 files) | gate runtimes 4 → **2**, canaries 4 → **2**, gates 8 → 8; `chromiumoxide` and the `headless-js` feature are gone |
| 7 | Vocabulary contraction | **done** 2026-08-08 | `cut/wave-7-vocabulary` | **−5,703** (+788 / −6,491, 127 files) | 14 registered retirements in one commit; `DIV_FEATURE_CLASSES` 7 → **3**, `RETIRED_XREF_PREFIXES` 3 → **7**, shortcodes 3 → **2**; **−5,866 B of CSS off every page** |
| 8 | CLI ergonomics + scaffolding | **done** 2026-08-08 | `cut/wave-8-cli-scaffolding` | **−2,349** (+397 / −2,746, 40 files, 16 deleted) | 10 verbs → **9, the ruling's target**; `doctor.rs` + `packages.rs` untouched as ruled; `dialoguer` gone; `init` templates 3 → **1**, `new` kinds 3 → **1** |
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
- **File-to-bundle collisions to resolve before waves 11/12:** `corpus/tarn/` is
  **RESOLVED** — wave 7 REWROTE it (tabsets → `###` subsections, the walkthrough → prose,
  the two `.definition` blocks → titled callouts) and left the project standing, so wave 12
  inherits it whole and may still delete it.
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
- **Chrome is GONE from the tree, and there is no automated browser test net at all.**
  Wave 6 deleted `reactive_browser.rs`, `headless_js_feature.rs`, the `headless-js` cargo
  feature and the `chromiumoxide` dependency together. `gates.sh` and `ci.yml` pass no
  feature flag any more — the default-feature build IS the whole workspace. **Nothing tests
  that a `{js}` cell's teardown runs on a block diff**; see the open hedge below.
- **`gate_script.rs`'s REQUIRE scan reads raw source text, including its own file.** Naming
  a retired `TALIESIN_REQUIRE_*` variable in full, even inside a comment explaining that it
  is retired, puts it straight back into the scanned set and fails the arming loop on a gate
  that no longer exists. Say "the R gate", not the variable. Recorded in the file too.
- **Sweep `docs/guide` for indented retired keys by hand before retiring nested
  vocabulary.** Still open as a standing rule; no gate sees an indented key. Wave 7 ran the
  sweep and it came back **clean** — the only indented match in the whole shipped tree is
  `corpus/diagnostics/typos.tmd`'s `shared:`, which is deliberate (see wave 7's finding 2:
  a retired PARENT key takes its children with it, so nothing under it is validated at
  all). That is the cheap direction and it is worth knowing: retiring a parent costs one
  register entry and silences the whole block, where retiring N children costs N.
- **Decide whether `Site::nav_ordered` still belongs where it is.** It lived in `llms.rs`
  and moved to `feed.rs` in wave 4 because `feed_hosts` was its other caller. If wave 11's
  site-layer reduction touches feeds, that is the moment to look at it again.
- **A RETIREMENT NOTE CAN GO STALE, AND NO GATE SEES IT. New in wave 8, applies to every
  wave after it.** `RETIRED_COMMANDS`' `schema` entry named `.taliesin/tali-site.schema.json`
  as its replacement; wave 8 deleted that file's writer, so the note would have shipped a
  pointer to something the tool no longer does, with all eight gates green.
  `a_retired_command_names_its_replacement_instead_of_guessing` checks that a note is
  non-empty and that the name is not live, never that its *claim* is true, and the same
  holds for `RETIRED_KEYS` and `RETIRED_DIV_CLASSES`. **Before cutting anything, grep the
  three registers for it.**
- **`--help` PROSE is ungated in the same way.** `init --help` promised "Every template also
  writes AGENTS.md (the agent onramp)" for six days after wave 2 deleted `agents.rs`.
  `every_parsed_flag_is_documented_in_its_subcommand_help` ties a FLAGS const to the help
  text and would pass on any claim at all; `commands_help_lists_every_subcommand` ties names
  to `COMMANDS`. Nothing ties a sentence to a behavior. Two known holes now (this and the
  one above), both cheap to walk into and neither worth new machinery. Grep instead.
- **Removing a SHORT flag from a parser that takes bare positionals is a reclassification.**
  Wave 8's `-y`/`--yes`: with the flag merely deleted, `taliesin init -y` would have created
  a directory named `-y`. Both scaffolders now reject any leading-dash token. A later wave
  dropping a short flag from `build`, `check` or `run` owes the same check.

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

### Wave 6 — 2026-08-08, `cut/wave-6-reactive-r-chrome`

**Measured reclaim: −4,146 lines** (`+960 / −5,106` over 100 files, 16 deleted outright)
against the ~4,700 estimate, plus a **5,422,412-byte** `ToyCar.glb` that no line count
sees. By area: `crates/server/tests` −1,155, `corpus` −1,043, `crates/core/assets` −892,
root md/toml −342, `crates/server/src` −267, `crates/core/tests` −254, `docs/guide` −123,
`tools` −35, `web-client` −19, `editor/vscode` −18, `site` −17, `crates/core/src` **+19**
(the register entries and the block-model retirement scan cost more than the glsl/numerics
wiring they replaced).

**`./tools/gates.sh` is GREEN on the committed tree:** **8/8** gates, **2/2** canaries,
**94 suites / 1,909 passed / 0 failed / 0 ignored**, exit 0. Measure wave 7 against
**1,909, two canaries and eight gates**. There is no longer any difference between this
figure and a bare `cargo test --workspace` — the feature-gated browser binaries that made
the two disagree since wave 0 are gone, so the number a stranger sees is now the number the
gate certifies.

**THE HEADLINE CHECK, which is the point of the wave.** `./tools/gates.sh` needs **two**
external runtimes now, not four: Python and Node. R + IRkernel and a system Chrome are
prerequisites of nothing, and the script says so in its own header rather than skipping
silently. Canaries are **2** (`CANARY_KERNEL`, `CANARY_NODE`); `CANARY_R` and
`CANARY_REACTIVE` were **dropped, not repointed**, per the rule in `gate_script.rs`'s count
assertion — after this wave the tree has no browser-driving test to repoint the Chrome
canary at, which is exactly the condition that rule describes. The workspace test line lost
`--features taliesin-server/headless-js` because there is no such feature: `chromiumoxide`,
the `headless-js` flag, `reactive_browser.rs` and `headless_js_feature.rs` all went
together, and the default-feature build is now the whole workspace.

**THE FOUR PROHIBITIONS HELD, and each was checked rather than asserted.**

1. `interp_identity`'s `{lang}::{path}::{version}` is byte-identical; the only diff near it
   is a new comment.
2. `Executor::langs` and `FreezeCache::packages` are still `HashMap`s with one key, with the
   reason written on the field.
3. `git diff crates/server/src/serve_site/exec_pool.rs` is **8 lines, all parameter drops**.
   `MAX_WARM_PAGES` and the eviction loop are untouched.
4. `FORMAT_VERSION` is still 4 and the digest still `71f1fe21dc878fcd`; neither needed
   editing, which is the signal the ruling asked for.

**FREEZE-KEY PROOF, run on real pages rather than reasoned about.** Keys for
`corpus/posts/{em-algorithm,fourier-transform,pca-geometry}` were captured before the cut
and diffed after: **every pre-cut key survives, in order, with no substitutions** (the diff
is pure additions from cells that had no entry yet). The decisive half is the rebuild log —
all three pages report `restored N cached cells · 0 re-ran`, so every key the post-cut
binary computed matched one the pre-cut binary wrote. A changed `interp_identity` shape
would have missed all of them.

**Six things that were not true, or that the playbook did not know.** Same genus as waves
1–5:

1. **A `RETIRED_CELL_LANGS` entry for `{r}` alone would have warned on every R LISTING in
   the manual, and the obvious fix opens a second hole.** ` ```r ` and ` ```{r} ` both emit
   `class="language-r"`, so the existing HTML scan cannot tell a display fence from a cell —
   and this repo *teaches* R code (`corpus/single-page-report` shows four brms blocks;
   `highlight.rs` keeps `r` in the syntect set on purpose). Keying the retirement on
   `data-tali-cell` separates them, but a cell with `#| echo: false` / `#| include: false`
   emits no listing at all: **measured, 3 of the 4 `{r}` cells in
   `corpus/single-page-report` warned and the hidden one did not.** The check now reads
   `Block::cells()`, which is echo-blind and descends into `:::` containers (`b.cell` alone
   would miss a cell inside a callout). Proven both directions on a scratch doc: the two
   cells warn once each with `TAL-CELL-RETIRED`, the two display fences are silent.
2. **The playbook's "port TWO assertions from `r_kernel.rs`" was half spent.** The
   `is_traced` guard it names does not exist — it went with debug mode in wave 3. The
   alt-text assertion did port, and it is the one thing in that file that was never about R:
   the bug lived in `render_media`'s **generic** PNG fallback (`kernel.rs:1511`), which every
   inline image except matplotlib's reaches. The replacement uses PIL, because matplotlib's
   twin-render path bypasses the fallback and would pass against the very code the bug lived
   in. It is `crates/server/tests/executed_figure_alt.rs`, and it is **proven able to fail**:
   putting `alt="output"` back turns it red.
3. **`three_scene_theme.rs`'s undercount is fixed by DERIVING the list, not by extending
   it.** A hand-kept four-path list is what produced the undercount in the first place;
   `helper_copies()` now walks `corpus/`, `site/` and `docs/`. Proven, by printing what it
   finds: it picks up `corpus/posts/pca-geometry/_includes/three-scene.tmd`, the fifth copy
   the list omitted, which had been drifting unpinned under a gate whose own doc comment
   claimed it covered every copy. The byte-identical pin is now grouped by CONTENT (extended
   vs base variant) rather than by a named pair, because deleting `corpus/graphics3d/` left
   the extended variant with a single copy — and it still fails loudly if a future edit
   merges the two variants, which was the other half of what the paired pin bought.
4. **`gate_script.rs`'s REQUIRE scan reads raw source text, including its own file.** Writing
   the retired variable names in full inside a comment *explaining that they are retired* put
   both straight back into `found` and failed the arming loop on gates that no longer exist.
   The comment now says "the R gate" and carries a warning for the next person.
5. **`OWN_JS` in `third_party.rs` still listed `deck.js`,** deleted in wave 5. Harmless (the
   list is an exemption set, so a stale entry asserts nothing) but it would have masked a
   future vendored file of that name. Removed with `glsl.js`/`numerics.js`.
6. **`repro.rs`'s `"r"` arms were KEPT, against the playbook's step 3.** `collect()` gathers
   every non-client-language cell whether or not it executes — `{julia}` and `{sql}` reach it
   today — so deleting the arms would hand a leftover `{r}` cell a `.txt` download claiming
   nothing, which is the opposite of what that fallback exists for. The test that drove them
   moved from `{r}` to `{julia}` and its doc comment now says why.

**The cargo-deny log was checked, per wave 4's lesson, and dropping `chromiumoxide`
orphaned nothing.** All three ignored advisories (`RUSTSEC-2024-0320`, `RUSTSEC-2025-0141`,
`RUSTSEC-2026-0205`) ride in through `syntect` and `zeromq`, not the browser driver, so all
three are still *encountered* and the gate is silent about them. `Cargo.lock` lost **268
lines** with the driver's whole tree. One `license-exception-not-encountered` warning does
fire, for `libfuzzer-sys` — but that is **pre-existing and unrelated**: the crate is not in
`Cargo.lock` at all and this repository has no fuzz target, so the exception was already
stale before this wave. It is a warning, not a failure; a later wave touching `deny.toml`
should drop the row.

**Three judgement calls, and how they went.**

- **The site's Three.js hero stays; `corpus/graphics3d/` goes.** Wave 5 already established
  that `site/index.tmd`'s live scene is the landing page's above-the-fold artefact and that
  the closing paragraph names it, so deleting the hero would have re-opened the question wave
  5 closed. `site/_includes/three-scene.tmd` survives, the `gallery/graphics3d` mount, the
  gallery section and the showcase paragraph pointing at it do not, and `THIRD_PARTY.md`
  loses its ToyCar attribution with the asset.
- **NO new register for the `range` alias.** `unknown_key_message` is already **scoped**, so
  a `("input type", "range", …)` row in `RETIRED_KEYS` is the entire cost and the diagnostic
  comes out as `TAL-INPUT-TYPE` with the removal note instead of a did-you-mean. This is the
  same reasoning wave 5 used to decline a shortcode register, reached from the other
  direction: there the vocabulary was closed and already spoke, here the register already
  covered the scope.
- **`window.taliJs.registerLanguage` is deleted, not kept as a seam.** `glsl.js` was its only
  caller; the `{js}` language registers into the internal `languages` map directly. The
  server-side `CLIENT_LANGS` registry **is** kept as a one-entry registry, per the ruling —
  `client_lang.rs`'s own history records that the pre-registry `lang == "js"` spelling was
  silently wrong once.

**Measured, not asserted, and the shape of the saving is not where the plan implied.**
`corpus/tech-blog` rebuilt with the release binary: `_assets/app.<hash>.css`
**54,951 → 53,478 bytes**, **−1,473 off every page** (the `.tali-math`, `.tali-mini-table`
and `.tali-glsl-canvas` blocks leaving `base.css`, which drops 79,534 → 77,105 at source).
The shared **`app.<hash>.js` is byte-identical at 93,172**, which is the part worth knowing:
neither `numerics.js` nor `tali-js.js` was ever in it. `numerics.js` (16,229 B) rode in the
conditional `jslibs.<hash>.js`, so its saving lands only on `{js}` pages, and `tali-js.js`
(39,133 → 28,505 B, **−10,628**) is emitted **inline per page**, so that one is per-page on
`{js}` pages too. `glsl.js` (8,795 B) shipped only on shader pages, of which none remain.
`corpus/analyst` and `corpus/single-page-report` were rebuilt through a live kernel after
conversion: zero warnings, zero error boxes, figure and table numbering unchanged
(Table 1/2/3, Figure 1/2 on the readout).

**The corpus conversion was verified against a live R render, not eyeballed.** `corpus/analyst`'s
three `{r}` cells became Python and the fit reproduces R's `lm()` **to every printed digit**:
intercept 4.675811, us-east −0.113122, ap-south 0.365546, canary 0.092340, per-week
−0.020258, and both half-quarter canary factors (1.161646 and 1.041428) with identical
confidence intervals. The model is written out as a design matrix rather than handed to a
formula library, because `statsmodels` is not in this repo's venv and a five-column OLS is
clearer written down than described.

**`corpus/single-page-report`'s seed changed, deliberately.** R's `set.seed(20260803)` has no
numpy equivalent, and the document's prose quotes the *result* — "a band roughly 0.9 wide",
"σ ≈ 0.22", the second repeated in `_data-modeling.tmd`. `default_rng(20260826)` was chosen
by scanning seeds for an observed region-mean band of 0.877 (σ ≈ 0.219), so the document's
own arithmetic stays true across two files rather than needing three prose rewrites.

**What was given up, stated plainly.**

**The second implementation, and with it the only thing that kept `kernel.rs`'s
"ZMQ is language-agnostic" claim honest.** The dissent's strongest point stands: R had no UI,
no CSS, no client code and 20 of `kernel.rs`'s 2,487 lines, so its polish *was* Python's
polish — and it had already paid for itself twice, catching the `#| trace: true` harness bug
and the generic `alt="output"` a11y bug that affected every non-matplotlib image. What the cut
actually buys is the `gates.sh` prerequisite and the second-arm audit tax. The alt-text half is
preserved and proven; the language-agnosticism claim is not.

**The `--no-exec` glsl twin, which was the only row that could catch that flag being
re-spelled `lang == "js"` instead of driven off `CLIENT_LANGS`.** With one registered language
the two spellings are indistinguishable by test. `no_exec_js_cells.rs` now carries a comment
saying a second language added to the registry **owes this file a row**, which is the moment
the distinction becomes observable again.

**Every automated browser test.** Nothing now tests that a `{js}` cell's teardown runs on a
block diff — the load-bearing path at four call sites in `client.js`'s diff-apply. This is the
open hedge below, and it is the largest single loss of coverage in the whole cut.

**`num`, `tali.tex`, `tali.table`, `tali.state`, `{glsl}` and `corpus/graphics3d/`.** The
numerics namespace held a seeded PRNG whose whole point was that a published explorable
resamples reproducibly; that argument survives the code. `tali.tex` closed the gap against
Jupyter's rich-display protocol for the two shapes a scientific cell returns, over the bundled
KaTeX fonts and with no parser. The 3-D gallery was four documents and a 5.4 MB sample model.

**A pre-existing inaccuracy this wave surfaced but did not fix.** `corpus/analyst/index.tmd`
says the second-half canary interval "covers 1.0". It does not, and did not before the
conversion either: measured identically in R and in Python it is [1.003, 1.081], p = 0.036.
Changing an analytical conclusion in a document written to be read is the author's call, not
a cut wave's.

### Wave 7 — 2026-08-08, `cut/wave-7-vocabulary`

**Measured reclaim: −5,703 lines** (`+788 / −6,491` over 127 files, 27 deleted outright)
against the ~5,140 estimate, plus **1.77 MB** of video the line count does not see (two
`site/assets/live-*-light.mp4` clips at 1,768,784 B, and `corpus/media`'s four `tour.*`
fixtures at 13,226 B). By area: `crates/core/src` −3,243, `crates/core/tests` −696,
`corpus` −541, `crates/core/assets` −412, `docs/guide` −396, `crates/server/src` −189,
`site` −141, `web-client` −49, docs (other) −24, `editor/vscode` −23, `docs/internals` −6,
`crates/server/tests` **+12** (the grid re-fixture is longer than the tabset it replaced),
root/other **+5**. `notes/` is excluded from every figure above and below.

**`./tools/gates.sh` is GREEN on the committed tree:** **8/8** gates, **2/2** canaries,
**90 suites / 1,814 passed / 0 failed / 0 ignored**, exit 0. Measure wave 8 against
**1,814, two canaries and eight gates**; a bare `cargo test --workspace` gives the same
figure, as it has since wave 6.

**THE GATE EARNED ITS KEEP, in the one way a bare `cargo test` cannot.** The first full run
came back RED on `every_tab_of_a_tabset_runs_its_own_cell_into_its_own_panel`
(`nested_cell_executes.rs`), a **kernel-gated** test that had passed all afternoon by
skipping. Everything else was green, twice over, including the render-side nested-cell pin
that looks like it covers the same ground. Re-fixtured onto a two-column `layout-ncol`
grid, which is the surviving multi-slot container, and the ordering claim is now a real one
rather than a co-location one: the second cell's `<pre>` must fall BETWEEN the two outputs,
which is exactly what a sibling splice would not produce.

**FOURTEEN RETIREMENTS, PROVEN ON A SCRATCH DOCUMENT RATHER THAN GREPPED FOR.** One `.tmd`
carrying every retired name at once, run through `taliesin check`, produced **15 located
diagnostics and no silence**: `theorems:` → `TAL-FM-KEY` with its note; all five theorem
kinds and all four widget classes → `TAL-DIV-CLASS`, each with its own note and never a
did-you-mean; `{{< video >}}` → `TAL-SHORTCODE` with its note; and `@thm-x` →
`TAL-XREF-UNDEF`, which is the whole point of the next paragraph.

**KEEPING ALL 12 `XREF_LABELS` TUPLES WAS THE LOAD-BEARING HALF, and the proof is that
`@thm-x` reports as a BROKEN REFERENCE rather than rendering as literal text.**
`RETIRED_XREF_PREFIXES` went 3 → **7** (`thm`/`lem`/`cor`/`def` joining `prp`/`exm`/`rem`)
while the label table stayed at 12. Deleting the four tuples instead would have made
`parse_xref` stop recognising `@thm-x` as a reference at all — no link, no diagnostic,
nothing — which is the silent-fallthrough this whole register family exists against.

**Six things that were not true, or that the playbook did not know.** Same genus as waves
1–6:

1. **A retired PARENT key takes its whole nested block with it, and that is the cheap
   direction.** `theorems:` had a `shared:` sub-key whose VALUES were themselves validated
   (`TAL-THM-KIND`). Retiring the parent silences the entire block — one warning, one
   register entry — rather than one warning per child, which is also what an author wants:
   they delete the block, not each line of it. `TAL-THM-KIND` went with it, and the
   long-carried "sweep `docs/guide` for indented retired keys" hazard came back **clean**
   for the first time, because there is no longer anything indented under a live key.
2. **`.code-walkthrough` was the only caller of the whole line-wrap machinery**, which the
   plan treats as shared infrastructure. `emit::wrap_pre_lines`, `wrap_code_lines`,
   `line_has_text` and `text.rs`'s `.tali-hl-ln` newline restoration all died with it —
   the playbook's step 3 explicitly says to LEAVE that restoration "(magic-move and
   `.debug` still wrap lines)", and both of those went in waves 5 and 3. ~110 lines the
   estimate did not carry.
3. **`web-client/search.js` carried 44 lines of tabset-reveal logic that nothing else
   touches.** `selectOwningTab` + `revealFor` existed because a Cmd-K hit could land in a
   collapsed `hidden="until-found"` panel; with no tabsets, nothing on any page carries
   that attribute, so both were dead. Neither `cargo test` nor `tsc` would ever have said
   so — the census in `token_contract.rs` is what surfaced it, by reporting `data-src` and
   three `*-init` attributes as browser-selected but no longer emitted.
4. **`theme.rs`'s `syncThemeVideos` survived `{{< video >}}`'s deletion by one indirection**
   and would have shipped inert JS on every page forever. It promotes `data-src`→`src` on
   the theme-visible clip of a light/dark pair; with the shortcode gone nothing emits
   `data-src` at all. Found the same way as (3), which is worth recording: **the data-*
   census is the instrument that catches a client-side orphan, and it caught two.**
5. **`check-superset.tmd` must stay CELL-FREE, and re-fixturing it broke that silently.**
   The theorem-id-unreferenceable case was replaced with a hidden-cell `label:`, which
   looked like the obvious substitute — but `validate_internal_anchors` returns early on
   any document with an executable cell (a cell can emit the target id at runtime), so
   adding one switched OFF the broken-anchor assertion two paragraphs above it. The test
   failed, the fixture is cell-free again with a comment saying why, and the
   unreferenceable-label family lives in `hidden_cell_xref_targets.rs`, which is where it
   was already covered.
6. **`manifest.test.ts`'s Rust-const parser insisted on `= &[` with a literal space**, so
   the moment rustfmt wrapped `RETIRED_XREF_PREFIXES` onto its own line the gate reported
   the const as **ABSENT** rather than as a parse failure. It reads `=\s*&[` now. Only
   `./tools/gates.sh` runs this suite, exactly as CLAUDE.md warns.

**Three judgement calls, and how they went.**

- **NO new `RETIRED_SHORTCODE` register, per the playbook's CUT C4 — but the removal note
  is delivered anyway, for six lines.** Waves 5 and 6 both declined new registers; this
  takes the cheaper third path wave 6 found for the `range` alias. `expand_in_line` reads
  `frontmatter::retired_note("shortcode", name)` and appends it, so `{{< video >}}` answers
  with "write the `<video …>` tag yourself" instead of a bare "unknown", the `{{<` opener
  stays in the message (`codes::classify` keys `TAL-SHORTCODE` off it), and the entire cost
  is one `RETIRED_KEYS` row under a new scope. **Precedent: prefer a scope on the existing
  register to a fourth register, every time.**
- **The site keeps its screencasts, as hand-written `<video>` in a `<figure
  class="tali-figure">`.** `.tali-video`'s nine CSS lines went and one rule (`figure
  .tali-figure video`) replaced them, so the frame + caption styling now applies to what
  the docs actually tell an author to write. The cost is the playbook's: **one clip per
  slot instead of a theme-matched pair**, so the dark screencast now also plays on a light
  page. That is the accepted downgrade on the shop window, and it deletes 1.77 MB.
- **`corpus/descent`'s scrolly became a `{{< input type=select >}}`, not five figures.**
  The playbook says to lower the five scenes to five ordinary numbered figures with prose
  between them; that is 5× the `{js}` code for a page whose entire point is that the reader
  drives it. One select control named `scene` keeps the one cell, the five states and the
  five narrations, using only live vocabulary. The reader picks the scene instead of
  scrolling to it — which is the honest description of what was lost.

**What was given up, stated plainly.**

**Numbered, cross-referenceable prose — the one capability in this wave with no substitute
at all.** The ruling's dissent stands unaltered: callouts get a `title=` and an `#sec-` id
and nothing more, so "as we showed in Theorem 3" is now hand-numbered or written as
`## Theorem: X {#sec-x}` and referenced as "Section 2.3". `float_number` keeps 7 of its 8
callers and `register_xref` 8 of its 9; every other numbered thing Taliesin offers (figure,
table, equation, listing) is non-prose. `corpus/course/` survives as a book, with its
theorems rewritten as display equations (`@eq-expectation`, `@eq-score`, `@eq-elbo`) and
sections (`@sec-consistency`) — which reads fine, and is not the same document.

**The knowledge in `render/mod.rs:2877`'s unreferenceable-theorem warning**, which encoded a
measured bug: the div's own `id=` path, unlike figures and tables, never gated on the
cross-reference prefix, so `::: {.theorem #pythagoras}` was numbered, silently
unreferenceable, and `check` said nothing. Rebuilding the feature is a day; rediscovering
that class of finding is a measurement pass.

**`panel-tabset`, which is the member of the widget bundle whose replacement is visibly
worse for the reader.** `corpus/tarn/install.tmd`'s two tabsets became six `###`
subsections, so a reader after the macOS command now scrolls past Linux and Windows. Its
churn record was one post-landing fix in its whole life, and its solved subtleties —
roving tabindex, full ARIA keyboard nav, `hidden="until-found"` so Ctrl-F still reaches an
inactive panel, tab labels as buttons so they stay out of the TOC — die with the code and
will be rediscovered at the same defects by any re-implementation. Cut on the judgement
that this tool is for prose.

**The `#anchor` include slice, and with it go-to-definition landing on a named section.**
`line_offset` is deleted rather than threaded as a hard-coded zero (the playbook's one real
warning about this cut), so the source map is genuinely simpler. It degrades **loudly**:
verified on a scratch project where `parts.tmd` really exists, `{{< include parts.tmd#sec-x
>}}` stays literal on the page AND draws a located "include not resolved" warning, while
the plain include beside it still splices.

**`scrolly`'s a11y work and `explorable_scrolly.html`'s byte snapshot.** `label_steps`
carried a measured finding — a scrolly was 0 steps with `aria`/`role` and a `null` root
role, so a screen-reader user got the words and never the stage — and its fix
(`role="group"` + an ordinal `aria-label` + `aria-controls`, kept out of `indexable_text`)
is gone with the container.

**Measured, not asserted.** `corpus/tech-blog` rebuilt with the release binary on both
sides of the cut: `_assets/app.<hash>.css` **53,478 → 47,612 bytes, −5,866 off every page
(−11.0%)**, the largest CSS win since wave 3's stepper removal; the shared
`app.<hash>.js` **93,172 → 77,301, −15,871** (scrolly.js + tabset.js + walkthrough.js
leaving `core_enhance_js`, plus search.js's reveal path). The output tree is **54 files
before and 54 after**, differing only in the two content-hashed asset names, and an
**in-place** rebuild over the existing tree produced a byte-identical file list — so no
surviving keep-contributor lost its writer.

### Wave 8, 2026-08-08, `cut/wave-8-cli-scaffolding`

**Measured reclaim: −2,349 lines** (`+397 / −2,746` over 40 files, **16 deleted outright**)
against the ~2,540 estimate. By area: `crates/server/src` −1,882 (complete.rs 1,344 +
interactive.rs 78 + the cli.rs collapse), `crates/server/tests` −272, `corpus` −114,
`docs/guide` −45, `Cargo.lock` −35, `Cargo.toml` −8, `crates/server/Cargo.toml` −1,
`docs/internals` **+6**, `crates/core/src` **+1**, `CLAUDE.md` **+1**, `README.md` 0.
`notes/` is excluded, as in every figure above.

**`./tools/gates.sh` is GREEN on the committed tree:** **8/8** gates, **2/2** canaries
(`kernel_executes_state_errors_and_interrupts_runaway_cell` and
`only_a_textual_sink_becomes_a_live_region` both printed `... ok`), **88 suites / 1,771
passed / 0 failed / 0 ignored**, exit 0. Measure wave 9 against **1,771, two canaries and
eight gates**; a bare `cargo test --workspace` gives the same figure.

**TEN CLI VERBS ARE NOW NINE, WHICH IS THE RULING'S TARGET NUMBER** (§4: `preview`, `build`,
`new`, `init`, `lsp`, `doctor`, `run`, `check`, `help`). `completions` went with one
`RETIRED_COMMANDS` line naming no replacement, as ruled. The hidden `__complete` went with
it and gets an ordinary "unknown command", correctly: it was underscore-prefixed and never
user-facing, so there is no author to answer.

**`doctor.rs`, `doctor_cli.rs`, `packages.rs`, `interpreter.rs`, the whole VS Code doctor
chain, `exec_pool.rs`, `freeze.rs`, `exec.rs`, `kernel.rs`, `diff.rs`, `render/`,
`diagnostics/`, `docs/DIAGNOSTICS.md`, `crates/core/assets/` and `web-client/` are all
BYTE-IDENTICAL to main**, checked as an explicit `git diff --stat` over that path list,
not asserted. `MAX_WARM_PAGES` is still 6. This wave ships **no asset change at all**, so
unlike waves 3/5/7 there is no per-page byte win to report; the shipped-bytes win is in the
binary instead (see the measurement below).

**Seven things that were not true, or that the playbook did not know.** Same genus as waves
1 to 7:

1. **The playbook's sharpest fact was half spent, and the surviving half was one file.**
   "`init` currently writes a 5 KB AGENTS.md and a `.taliesin/` dot-directory". The
   AGENTS.md half went with `crates/core/src/agents.rs` in wave 2, taking `agents_md_cli.rs`
   with it, so `onramp_files()` was already down to `.taliesin/tali-site.schema.json` alone.
   **The measured claim held exactly, and is worth restating because it is the whole case:**
   `find . -name .taliesin` returns nothing, and the only two `# yaml-language-server:`
   modelines anywhere in the tree were in `corpus/scaffold-site/_site.yml` and
   `corpus/scaffold-book/_site.yml`, the byte pins for the very templates this wave
   deletes. The feature's entire footprint in the author's own work was the fixtures that
   existed to pin it. `mcp.rs` is gone too (wave 2), so TIER 3's "fix mcp.rs:123" and TIER
   5's "shrink mcp.rs's PROMPTS so `every_scaffold_kind_has_a_prompt` stays green" are both
   spent steps.
2. **`new` was at THREE kinds, not one.** The playbook's "Why here" says wave 5 left `new`
   "down to one kind"; `NEW_KINDS` was `["post", "page", "paper"]` and wave 5 removed only
   `deck`. So TIER 5's `NEW_KINDS = &["post", "deck"]` was stale in both directions, and the
   wave removes two kinds rather than reducing to two.
3. **A retirement note can name a live feature, and cutting that feature makes the note lie
   with every gate green.** `RETIRED_COMMANDS`' entry for the wave-2 `schema` verb read
   "`init` writes `.taliesin/tali-site.schema.json` for you": true when written, false the
   moment TIER 3 landed. Nothing checks a note's *claim*:
   `a_retired_command_names_its_replacement_instead_of_guessing` asserts only that the note
   is non-empty and that the name is not in `COMMANDS`. It now points at the companion.
   **Rule for waves 9 to 13: when you cut a feature, grep the three registers for it.** A note
   is prose about the surviving surface, and it rots exactly like a doc page.
4. **Deleting a SHORT flag from a parser that accepts bare positionals is a
   reclassification, not a subtraction.** Both scaffolders treat any non-`--` token as a
   positional, so with `-y`/`--yes` merely removed, `taliesin init -y` would have scaffolded
   a project into a directory literally named `-y`, and `taliesin new -y post x` would have
   read `-y` as the kind. The fix is one character per parser (`starts_with("--")` →
   `starts_with('-')`) and both directions are pinned in
   `the_retired_yes_flag_is_not_read_as_a_directory`. The playbook does not mention it.
5. **`init --help` still promised a file the tool stopped writing six days earlier.** "Every
   template also writes AGENTS.md (the agent onramp) and the `.taliesin/` config schemas"
   survived wave 2 untouched, because no gate ties `--help` *prose* to behavior:
   `every_parsed_flag_is_documented_in_its_subcommand_help` compares a FLAGS const to the
   help text and would pass on any claim at all. Deleted with the block; the gap is real and
   is now the second known one (finding 3 is the first).
6. **`complete.rs` was the only consumer of the scaffolder's `pub(crate)` surface.**
   `NewKind`, `NEW_KINDS`, `new_files`, `init_files` and `NewOpts` were crate-visible for the
   completion generator's kind and template tables. With it gone, nothing outside `cli.rs`
   names any of them, so all five are private, the same demotion wave 2 made to
   `text.rs::project_block`. Only a grep finds this; `-D warnings` does not, because
   `pub(crate)` items used *anywhere* in the crate are live.
7. **`every_parsed_flag_is_documented_in_its_subcommand_help` did NOT inherit a bigger
   burden, contrary to the playbook's verification note.** `complete.rs` declared no
   `<PREFIX>_FLAGS` const of its own (it read the parsers'), so the scan's population is
   unchanged at six (`BUILD`, `CHECK`, `DOCTOR`, `INIT`, `NEW`, `SERVE`) and its `>= 6` floor
   is now exact rather than slack. What the deletion really retires is the *other* copy: the
   `flags_for()` / `describe()` / positional tables that every earlier wave's verb removal
   had to edit by hand.

**Four judgement calls, and how they went.**

- **The onramp was cut; the schema was NOT.** `crates/core/src/schema.rs` and
  `assets/schema/tali-site.schema.json` are untouched, and so is the bless test that
  regenerates the file from `site::NATIVE_KEYS`. What went is only the *delivery mechanism
  into a stranger's project*. A VS Code writer loses nothing measurable: the companion
  bundles its own copy and wires it through `yamlValidation` at `package.json:104`, which is
  the surface this tool is built alongside. Everyone else now saves the file and writes one
  comment line, documented in `docs/guide/reference/configuration.tmd`,
  `docs/internals/validation.tmd` and `docs/guide/using/preview.tmd`.
- **`page`, `paper` and `deck` get hand arms in a LOCAL table, not a scope on
  `RETIRED_KEYS`.** Wave 7's precedent is "prefer a scope on the existing register to a
  fourth register, every time", and this is the one place in the cut where it does not
  apply: `RETIRED_KEYS` lives in `taliesin-core` and is the *document* vocabulary, consulted
  by `unknown_key_message` inside the renderer. A CLI positional is not a document key, and
  putting one there would make the renderer's register answer for the argv parser.
  `RETIRED_NEW_KINDS` is eleven lines in `cli.rs`, beside the thing it retires and beside
  `main.rs`'s `RETIRED_COMMANDS`, which is the register it is actually the sibling of.
  Proven the wave-7 way, by running the commands rather than grepping for absence: all three
  answer with a dated note and never a did-you-mean, while `new pots x` still resolves to
  `post`.
- **`NewKind` was collapsed rather than kept at one variant**, on wave 5's `DocFormat`
  precedent, and it forced one decision the playbook does not raise: the `--json` receipt's
  `"kind"` field. It **stays**, reading `NEW_KINDS[0]`, because an agent already parsing that
  receipt should not see its shape change for a reason invisible to it. `new_files` is now
  `(slug, today, opts)` with no match at all.
- **The three corpus byte pins were deleted and NOT replaced with an in-crate assertion.**
  The playbook asks for one; comparing `init_files()` to the consts it is built from asserts
  nothing, and a vacuous gate is worse than an absent one (wave 1's finding 1, in the other
  direction). The behavioral pins are stronger and both survive: `init_cli.rs` and
  `new_cli.rs` run the **real binary** and then the **real `check`** over what it wrote.
  What is genuinely given up is drift detection on the scaffold *prose*, which is not a bug
  class.

**Proven by running the commands, not by grepping.** From a clean temp dir with the release
binary: `init smoke` writes exactly `_site.yml` + `index.tmd` (no `.taliesin/`, no
AGENTS.md, no modeline; `find` and `grep` both empty), `new post hello` writes
`posts/hello/index.tmd`, and `build smoke --strict --no-exec` exits 0 on 2 pages. Then every
retirement, each answering rather than falling silent:

```
taliesin completions zsh   → `completions` was removed: nothing; type the subcommand out…  (exit 1)
taliesin schema site       → `schema` was removed: nothing on the CLI; the VS Code companion…
taliesin new page x        → `new page` was removed on 2026-08-08: a page is a `.tmd` file…
taliesin new paper x       → `new paper` was removed on 2026-08-08: scaffold a `post` and add…
taliesin new deck x        → `new deck` was removed on 2026-08-08 with the slide-deck engine…
taliesin init --template site → unknown flag `--template`   (and t1/ was never created)
taliesin init -y           → unknown flag `-y`              (and ./-y was never created)
taliesin new pots x        → unknown kind `pots` (did you mean `post`?)   ← still works
taliesin doc               → unknown command: `doc` (did you mean `doctor`?)
```

That last line is the re-pointed prefix rule: `("com", "completions")` died with the verb, so
`a_name_that_extends_or_abbreviates_a_command_suggests_it` now carries `("doc", "doctor")`,
with the premise re-verified (`closest("doc", COMMANDS)` is `None`, so the prefix rule fills
silence and never overrides a did-you-mean).

**Measured, not asserted.** No asset changed, so there is no per-page byte delta this time.
The shipped **binary** is what shrank: two cold `cargo build --release` runs into separate
target dirs, same toolchain, `main` (in a throwaway worktree) against this branch:
**31,075,656 → 30,897,064 bytes, −178,592 (−0.57%)**. That is `complete.rs`'s four
shell-script templates and its
per-subcommand description/flag tables leaving the executable. `cargo tree -p taliesin-server
-i dialoguer` now errors, and `Cargo.lock` lost `dialoguer`, `console`, `encode_unicode` and
`unicode-width`.

**The cargo-deny log was checked, per wave 4's lesson, and dropping `dialoguer` orphaned
nothing.** `advisories ok, bans ok, licenses ok, sources ok`. The one
`license-exception-not-encountered` warning is still `libfuzzer-sys`, still **pre-existing
and unrelated** (not in `Cargo.lock`, no fuzz target in the tree). `deny.toml` was not
edited this wave, so the stale row is carried forward exactly as wave 6 left it.

**Swept and clean:** `tools/ui-audit/` names nothing this wave removed (wave 7's sweep found
it empty too); `editor/vscode/` is byte-identical, correctly, because the companion never
shelled out to `completions` and its schema copy is unaffected; and the `data-*` census in
`token_contract.rs` is unchanged, as it must be, because this wave removed no emitter.

**What was given up, stated plainly.**

**Tab completion, and nothing replaces it.** 1,344 lines, of which the interesting half was
the `__complete` runtime rather than the four shell shims: it offered only `.tmd` files plus
directories that contain one, with site and book roots sorted first; per-command flags with
one-line descriptions; enumerated positionals; and it registered both `taliesin` and the
`tali` alias, so no per-shell list could drift from the binary. `--install` detected the
shell from `$SHELL` and wrote the script into the right directory. That is a genuine
ergonomic loss for a daily driver, and the register note says there is no replacement rather
than pretending otherwise. What it cost, and what every earlier wave paid by hand, was a
second copy of every verb's flag set inside it.

**`new paper`, which is the real loss in this wave.** It was the one scaffold that was more
than a front-matter block: a citation-wired document (`bibliography: [references.bib]`, a
real `[@knuth1984literate]`, and the `.bib` shipped beside it so `check` was clean on the
first save), a labelled matplotlib cell with `#| label: fig-demo` and `#| fig-cap:`, an
`@fig-demo` cross-reference resolving off that label, a `{#sec-methods}` anchor and display
math. It taught five features at once, by example, in a file the author could immediately
run, which is a different thing from documenting them. A reader now meets those features
one reference page at a time.

**`init --template site|book`.** The `site` starter was a nav plus an About stub; the `book`
starter was three chapters and the `chapters:` key. The first is five minutes of typing, but
the second is the one that changes what the project *is*: `chapters:` turns a website into
a book with a sidebar, numbering and prev/next, and discovering it now means reading
`docs/guide/reference/configuration.tmd` rather than typing `--template book`. Both
`getting-started.tmd` and `cli.tmd` were rewritten to name that path explicitly rather than
leaving the reader to find it.

**The wizard.** 78 lines of `dialoguer` behind a stdin-is-a-TTY gate, so a human who typed
`taliesin new` with nothing after it got an arrow-key picker and a re-prompting slug
validator instead of a usage line. It was purely additive at a terminal and invisible to CI,
a pipe or an agent by construction, which is also why nothing is lost outside a terminal.
`wizard_gate.rs`'s three tests went with it; the behavior they guarded (never prompt when
stdin is not a TTY) is now unconditional, and `a_bare_init_scaffolds_without_prompting` in
`init_cli.rs` keeps driving `/dev/null` at it so a reintroduced prompt would fail rather than
hang.

**Nine corpus documents (104 lines) across `corpus/scaffold/`, `corpus/scaffold-site/` and
`corpus/scaffold-book/`**, and `docs/guide/reference/shell-completion.tmd`. The guide is down
one chapter and the reference nav on the other seven pages lost the link.
