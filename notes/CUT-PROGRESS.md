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
  **PAID in wave 9: `build <file|dir> --check-only`**, a ~40-line `lint::cmd_check_only`
  plus one `parse_build_args` arm. A FLAG, not a tenth verb, so wave 8's target of nine
  survives.
- Keep exactly one machine-readable output (`--format json` on the survivor). Going to
  zero breaks the author's own AI-assisted workflow within a week.
  **PAID in wave 9: `build … --format json`**, carrying `severity`, `file`, `line`,
  `col`/`end_col` and the structured `suggestion`. It lost `code` and `docs_url` with the
  catalogue, so an agent matching a family now matches a message prefix.

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
| 9 | Diagnostics catalogue (keep lint front door) | **done** 2026-08-08 | `cut/wave-9-diagnostics` | **−4,957** (+1,288 / −6,245, 93 files, 13 deleted) | verbs stay **9**: `check` retired into `build --check-only`; `check.rs` → `lint.rs`, 1,189 → **620** impl lines; severity is a FIELD, the `TAL-*` catalogue is gone |
| 10 | LSP long tail | **done** 2026-08-09 | `cut/wave-10-lsp` | **−9,838** (+180 / −10,018, 38 files, 17 deleted) | 16 advertised providers → **7**, custom methods 8 → **3**; every LSP **write path** is gone; binary **−415,712 B**; verbs stay **9** |
| 11 | Serve layer, opened once | **done** 2026-08-09 | `cut/wave-11-serve` | **−5,015** (+768 / −5,783, 80 files, 5 deleted) | the one wave that opened `exec_pool.rs`, and the LRU is absent from its diff; warm pool, `mounts:`, `--bare` and `--host` all gone; binary **−491,072 B**; a FOURTH CLI register (`RETIRED_FLAGS`); `tools/build-site.sh` is pre-push step 5 |
| 12 | Justification layer (corpus, docs, tests) | **done** 2026-08-09 | `cut/wave-12-justification` | **−4,627** (+266 / −4,893, 61 files, 30 deleted) | the last planned wave; corpus 93 → **83** docs, Internals 14 → **7** chapters; `corpus/tarn` KEPT and `corpus/course` cut, reasoned below; the wave's stated deliverable was re-specified and then **refuted by its own measurement** |
| 13 | **`taliesin run`** (adjudicated in-session: CUT) | **done** 2026-08-09 | `cut/wave-13-run` | **−4,081** (+276 / −4,357, 35 files, 8 deleted) | the campaign's last wave; verbs **8 → 7**; the ruling's "survives regardless" list was **wrong**, so the real cut is 2.5x its estimate; the code lens survives as a LABEL |

## Open items carried forward

- **THE UNUSED-VOCABULARY TAIL, MEASURED 2026-08-09, IS THE INSTRUMENT'S ANSWER AND THE
  NEXT ROUND'S INPUT. New in wave 12.** `taliesin features` is gone, so the surviving
  instrument is a scan of the validator consts against the shipped `.tmd` read set (110
  documents: `corpus/` minus `corpus/diagnostics/`, plus `docs/guide`, `docs/internals`,
  `site`, `samples`). **10 of 63 offered names are witnessed by nothing:** `csl:` (front
  matter), `head:` and `python:` (`_site.yml`), and the seven theorem-family xref prefixes
  `cor`/`def`/`exm`/`lem`/`prp`/`rem`/`thm`. Zero unused cell options, callout kinds, div
  classes or shortcodes. **The same scan on the pre-wave tree gives the same 10**, which is
  the finding: the corpus's coverage was never an artefact of the documents wave 12 cut.
- **THERE IS STILL NO FLOOR ASSERTING THAT ANY BOOK EXISTS.** `corpus.rs` sweeps whatever
  is there, so a later wave could delete `corpus/tarn`, `corpus/demo-book` and both
  `docs/` books one at a time and never fail a test for it. Wave 12 kept `tarn` partly on
  that ground and did not add the floor, because a floor whose only job is to forbid a
  future cut is machinery this campaign is removing. Know it before cutting a book.
- **BEFORE HONOURING A "MUST SURVIVE", CHECK THE FILE EXISTS. Wave 12 found EIGHT spent
  justifications in one wave** (four the handoff already named, four more it did not: the
  PT-2 synthetic pin `include_root_parity.rs` was asked to add already exists,
  `section_extents.rs` reads `structure.tmd` not `dense-output.tmd`, and the survive list
  names `corpus/transclude.tmd`, `corpus/_includes/shared-derivation.tmd` and
  `corpus/scaffold*/`, none of which are in the tree). This is now the rule with the most
  recurrences in the campaign.
- **AN ANTI-VACUITY FLOOR IS NOT A CONTENT FLOOR, AND THE DIFFERENCE MATTERS WHEN YOU CUT.**
  Wave 12 moved three: `stale_docs.rs`'s walk floor (40 → 25), its path-claim floor
  (120 → 60, measured at 122 with two of headroom, which would have failed the next docs
  edit for the wrong reason), and `three_scene_theme.rs`'s copy floor (4 → 2, which would
  have hard-failed on the duplicate cull). Each now carries the count it was measured
  against and the reason it exists, so the next wave can tell "the walk broke" from "the
  content shrank" without re-deriving it.
- **`tools/ui-audit/` IS NOT CLEAN BY DEFAULT AND NO GATE READS IT.** Five waves running it
  has found something. Wave 12's was the sharpest: `make-sweep-index.mjs` parses
  `corpus/README.md`'s table with a four-column shape, so the two-column rewrite would have
  skipped every row and produced a sweep index with no annotations at all, looking exactly
  like a corpus with nothing worth saying about it. Run the parser by hand after touching
  any file it reads.
- **A retired FLAG now has a register: `serve::RETIRED_FLAGS`** (`crates/server/src/serve/mod.rs`),
  the fourth CLI one beside `RETIRED_COMMANDS` (verbs) and `RETIRED_NEW_KINDS` (`new` kinds).
  Keyed on the flag alone, not on the verb, and consulted by `unknown_flag_error` **before**
  the did-you-mean. It carries `--bare` and `--host`. One derived test covers the register, so
  a later retirement is one entry and nothing else, the same contract the other three have.
- **Wave 13 is DONE and the campaign is over.** `taliesin run` was adjudicated and cut
  whole. **The ruling's §6.1 reason for deferring `runspec.rs` and `run_control.rs` was
  false**: it says "the preview server's Run buttons use them", and the preview has no Run
  buttons. `web-client/client.js` sends exactly three client-to-server messages
  (`restart_kernel` twice, `click_block`), and `POST /__taliesin/run` and
  `/__taliesin/interrupt` had one client each, `run_cmd.rs`. So all six files went, plus a
  chain of things they alone kept alive. See the wave 13 log entry.
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
- **The warm pool is GONE (wave 11) and the accepted cost is recorded in that wave's log**:
  a `warming-kernel` state on the first cell of every fresh code-cell page in preview, and
  again on any page evicted past `MAX_WARM_PAGES = 6`.
  **CORRECTED 2026-08-09 (wave R2): that is not what shipped.** A `warming-kernel` state is
  a *state on a page*, and on a first build there was no page to put it on — `build_page`
  published nothing at all until every cell had finished, so a page the websocket reached
  before its first build showed a bare navbar for the length of its slowest cell (measured:
  20 s on a 25 s cell, no spinner, no status). The accepted cost was therefore understated:
  it was not a label on a visible page, it was a blank one. Wave R2 publishes the pre-exec
  body on a first build, so the cost is now the one this entry describes.
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
- **`Site::nav_ordered` stays in `feed.rs`.** Wave 11's site-layer reduction did not touch
  feeds (`feed.rs` is byte-identical), so the question the wave-4 note deferred never came
  up; `feed_hosts` is still its only caller and the file is still the right home.
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
- **AN INTEGRATION TEST THAT SHELLS OUT TO A VERB CAN PASS VACUOUSLY ONCE THAT VERB IS
  RETIRED. New in wave 9, applies to every wave after it.** An unknown command exits 1, so
  any test whose success condition is "exit 0 or 1" (`hostile_input.rs`: fifteen hostile
  documents, "the pipeline survived") or "this string is absent from stderr"
  (`mount_static_build.rs`: a retired code) keeps passing against a binary that never read
  the input. **Before retiring a verb, grep the test tree for it and read each call site's
  success condition, not just its name.** Four of the six wave 9 found failed loudly; two
  did not.
- **A DERIVED CLASSIFICATION CAN BE WRONG IN THE CORPUS WITH EVERY GATE GREEN.** `codes.rs`'s
  ordered substring table filed `corpus/diagnostics/links.tmd:28`'s dangling reactive input
  as `warning[TAL-SHORTCODE]`, because the message quotes `{{< input >}}` and the shortcode
  row sat above the reactive one. The cycle half of the same family, two lines below,
  reported `error[TAL-REACTIVE]` correctly. Nothing could see it: both the code and the
  severity looked plausible. **When a property is derived from prose, the derivation is a
  place bugs live silently**, which is the argument that made wave 9 worth doing, found by
  doing it.
- **A TEST CAN BE ASSERTING ON A DOC COMMENT, and only deleting the comment shows it. New in
  wave 11.** `the_pre_paint_canvas_map_tracks_the_theme_tokens` located `--tali-bg` by finding
  `:root` in `TOKENS_DARK_CSS`, a file with **no `:root` block at all**: the only match was
  inside a comment that happened to sit directly above the real
  `html[data-theme="dark"]` block. It read the right value for months by accident. **When a
  test locates its subject by a string that also occurs in prose, editing the prose is a
  code change**, and no gate says so. Same genus as wave 9's derived-classification finding.
- **A FLAKY TEST CAN ACCUSE THE WRONG SUBSYSTEM, AND THIS ONE ACCUSED THE MOAT. Found on the
  wave 11 push, 2026-08-09; the defect is PRE-EXISTING and predates the wave.**
  `exec::tests::an_interrupt_stops_the_whole_run_and_keeps_the_warm_state` cancelled the run
  300 ms after `control.running_lang()` first went `Some`, on the comment "cell 1 is instant,
  so this is cell 2". `begin_cell` fires for **every** cell, so the clock starts on cell 1;
  when the box is loaded enough that cell 1's ZMQ round trip outlasts 300 ms, the cancel lands
  on `warm = 41` and the run never sets `warm`. The test then fails on its last assertion with
  *"cell 1's variable did not survive the interrupt, so this was a restart"*, an accusation
  aimed squarely at the SIGINT path in `kernel.rs`, which was innocent, and which wave 11 had
  just edited. Diagnosed by making the race deterministic (a 1 s cell 1): the old waiter fails
  every time, the new one passes every time. **Cell 2 now announces itself by writing a file
  from inside its own body**, and the waiter cancels on that file plus a live `running_lang`,
  which has no window at all. Note the shape for later waves: `run_control.rs` was byte-identical
  across wave 11 and the interrupt canary passed in every gate run, so the diff was the fastest
  way to establish innocence. **Check what a failing test actually proves before believing the
  string it prints.**
- **A VISIBILITY CHANGE CAN DROP A CONST OUT OF A SOURCE-SCANNING GATE. New in wave 11.**
  `every_parsed_flag_is_documented_in_its_subcommand_help` finds `<PREFIX>_FLAGS` by requiring
  the literal `const ` at column 0, so making `BUILD_FLAGS`/`SERVE_FLAGS` `pub(crate)` removed
  `build` and `preview` from the comparison entirely. Its own **floor assertion** caught it
  (`lists.len() >= 5`, collected 3), which is the whole reason that floor exists. Any later
  wave that changes a scanned declaration's shape owes a look at the scanners in `main.rs`.
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
- [x] Write `tools/build-site.sh` before `mounts:` goes, and wire it into
      `.githooks/pre-push`. **Done 2026-08-09 in wave 11, and it verifies rather than
      merely building**: it resolves every cross-project link written in `site/` against the
      composed output and exits non-zero naming any that has nothing behind it. Pre-push
      step 5, `--check` (`--no-exec`, temp dir, 1.1 s).
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

### Wave 9, 2026-08-08, `cut/wave-9-diagnostics`

**Measured reclaim: −4,957 lines** (`+1,288 / −6,245` over 93 files, **13 deleted outright**)
against the ~7,850 estimate. By area: `crates/core/src` −2,268, `crates/server/src` −982,
`crates/server/tests` −553, `crates/core/tests` −442, root md/hooks −307,
`editor/vscode` −301, `corpus` −122, `docs/guide` −6, `docs/internals` **+24** (the
validation chapter was rewritten, not deleted, see the judgement calls). `notes/` is
excluded, as in every figure above.

**The estimate missed by ~2,900 lines and every line of the gap is deliberate.** Five things
were kept that the playbook deletes, and each is named with its reason under the judgement
calls: `media.rs` + `reactive.rs` whole (263, the bundle's own coordination overrides),
`code_lang.rs`'s retirement register **and its scan** (87), `docs/internals/validation.tmd`
(254), `diaglink.ts` + `termlinks.ts` (114), and the collectors + printer that make
`--check-only` a front door rather than a flag on a writing build (~250).

**`./tools/gates.sh` is GREEN on the committed tree:** **8/8** gates, **2/2** canaries
(`kernel_executes_state_errors_and_interrupts_runaway_cell` and
`only_a_textual_sink_becomes_a_live_region` both printed `... ok`), **86 suites / 1,666
passed / 0 failed / 0 ignored**, exit 0. Measure wave 10 against **1,666, two canaries and
eight gates**; a bare `cargo test --workspace` gives the same figure. The 105-test drop from
wave 8's 1,771 is `check_superset.rs` (432) and `check_cli.rs` (600) going whole, plus the
trims to `diagnostics/tests.rs` and `lint.rs`'s own module.

**NINE CLI VERBS ARE STILL NINE.** `check` is retired into a **flag**:
`build <file|dir> --check-only` lints, writes nothing, and takes `--strict` and
`--format json` the same way. That is the standing directive's first exception paid at its
stated price: the front door is a ~40-line `cmd_check_only` plus one `parse_build_args` arm,
where the verb was 2,661 lines. `crates/server/src/check.rs` → `crates/server/src/lint.rs`,
**620 impl lines** (from 1,189) and the module doc rewritten from "the `check` subcommand" to
what it actually is: the shared static-lint kernel with four consumers.

**THE MOAT IS BYTE-IDENTICAL TO MAIN,** checked as an explicit `git diff --stat` over the
path list and not asserted: `crates/core/assets/`, `web-client/`, `serve_site/exec_pool.rs`,
`freeze.rs`, `exec.rs`, `kernel.rs`, `diff.rs`, `packages.rs`, `doctor.rs`.
`MAX_WARM_PAGES` is still 6 and `FORMAT_VERSION` still 4. So this wave ships **no asset
change at all** and there is no per-page byte win to report, the same shape as wave 8.

**Measured, not asserted.** The shipped **binary** is where it lands: two cold
`cargo build --release` runs into separate target dirs, same toolchain, `main` (in a throwaway
worktree) against this branch: **30,897,448 → 30,792,128 bytes, −105,320 (−0.34%)**. That is
the 518-line `EXPLANATIONS` table, the 51-row `TABLE`, the `--explain` reader and the
interpreter probe leaving the executable. (A first attempt built main twice, into two
different target dirs, and reported a delta of exactly 0. Worth keeping: it is also a clean
demonstration that this build is byte-reproducible, so the −105,320 is signal.)

**THE PRE/POST DIAGNOSTIC DIFF, WHICH IS THE WHOLE VERIFICATION OF THE WAVE.** Captured from
the surviving path (`build --strict --no-exec`) over all twelve projects plus every loose
`corpus/` document, 168 lines pre. The diff contains **only**: the cut families
(`empty heading` ×1, `duplicate heading text` ×1, `repeats the page title` ×1, `has no content
under it` ×2, `caption is only its label` ×1, `ambiguous link text` ×1, `math failed to
render` ×1, `unknown code language` ×1, `has no accessible name` ×4), the three deleted
fixtures' whole blocks, the `--strict: N problems` counts that follow from those removals,
and a one-line shift in `a11y.tmd` / `check-superset.tmd` because both fixtures were
re-prosed. **Every surviving diagnostic is byte-identical.** The `[TAL-…]` bracket the plan
expected to see disappear never appeared in this capture at all: `build`'s human log prints
`log::warn`'s level, never the diagnostic's own severity or code. Only `check`'s formatter
did, and it is the one this wave rewrote.

**Then the severity census, which is where the wave found a real bug.** Same corpus through
`--format json` on both sides, compared as `(severity, file:line, message)`: 60 unique
diagnostics pre, 46 post, and after the allowed removals **exactly one row changed
severity**.

**FINDING: the derived-severity table was misclassifying a live corpus diagnostic, and the
field-based severity corrects it.** `corpus/diagnostics/links.tmd:28`'s dangling reactive
input reported as `warning[TAL-SHORTCODE]` and now reports as `error`. The cause is the
failure mode `codes.rs`'s own comments were written against: `classify` is an *ordered*
first-hit-wins substring scan, the message is

```
unknown reactive input `undefined_name`: no `{js}` cell or `{{< input >}}` defines it
```

and the row `("{{<", "TAL-SHORTCODE", WARNING)` sits **above** `("unknown reactive input",
"TAL-REACTIVE", ERROR)`. So the diagnostic quoted the tool's own shortcode syntax and was
filed as a shortcode typo. Two halves of one family came out at two severities under two
codes, in the corpus, today: the *cycle* half reported `error[TAL-REACTIVE]` correctly two
lines below. Nothing could see it, because both the code and the severity looked plausible on
their own. Neither spelling changes a gate (error and warning both gate by default), so what
it cost was the printed word and the LSP squiggle colour. **This is the argument for the
refactor, found by doing it rather than by reasoning about it.**

**Eight things that were not true, or that the playbook did not know.** Same genus as waves
1 to 8:

1. **The two coordination overrides were larger than "~130 lines back": both files survive
   WHOLE.** `media.rs` (82) *is* the raw-`<video>`/`poster=` scan and `reactive.rs` (181) *is*
   the dangling-input + cycle rules, so there was no sub-part to trim. `page_static_diagnostics`
   therefore lands at **10 validators, not 8** (the plan's figure), and the two overrides are
   3 of the 10.
2. **Deleting `validate_code_languages` would have retired `RETIRED_CELL_LANGS` into
   silence, one wave after wave 6 built it.** The register's only emitter lived inside the
   function the plan deletes, so `{r}`, `{glsl}` and `{pyodide}` would have gone back to
   "your kernel is broken" with the register still sitting in the tree. Split instead: the
   generic unknown-fence-language lint is cut (the defect is visible, the block renders
   unhighlighted), the register's scan survives as `validate_retired_cell_langs`. That is
   CUT-PROGRESS rule 6 and the whole register doctrine, not a feature retention.
3. **`Diagnostic::new`'s severity had to be ERROR, and that is a fact about its call sites,
   not a default.** All ~20 surviving callers are hard failures the tool found *outside* a
   validator: cannot read, cannot write, cannot create, malformed front-matter YAML, a cell
   that raised, a kernel that would not start, no publishable pages, a page task that
   panicked, a refusal to build in place. Under the old table every one of those fell through
   to `(GENERIC, ERROR)`, so ERROR is the faithful translation *and* the honest one. Pinned by
   `a_hard_failure_gates_without_being_classified`, because a constructor that defaulted to
   `Warning` would silently stop failing a `--check-only` run on a page it could not read.
4. **`validate_shared_bibliography` was losing its severity before this wave, through
   `Diagnostic::new(…, w.message)`.** The site-wide "declared but never cited" is the one
   SUGGESTION in the whole surviving set, and taking `w.message` out of the `Warning` dropped
   the classification the message would otherwise have carried. It goes through `diag_from`
   now. (Under the old table it re-derived correctly from the message, so this was latent
   rather than broken; with severity as a field it would have become a real regression.)
5. **The path gate caught my own stale reference, which is the one gate in this repo that
   has now paid for itself twice.** `stale_docs.rs`'s backticked-path scan failed on
   `docs/internals/validation.tmd: docs/DIAGNOSTICS.md`, a file this wave deletes, named in
   a sentence this wave *wrote* to explain the deletion. Wave 1's finding 1 recorded that the
   fenced "Where things are" map is still ungated; a backticked path in prose is not, and it
   works.
6. **Six integration tests shelled out to `taliesin check` and two of them would have passed
   VACUOUSLY.** `hostile_input.rs` accepts exit 0 or 1 as "the pipeline survived", and an
   unknown command exits 1: every one of its fifteen hostile documents would have been
   "handled" by a binary that never read them. `mount_static_build.rs`'s
   `!stderr.contains("TAL-MOUNT-PREVIEW")` is the same shape in the other direction (a
   retired code is absent from an error message too). The other four fail loudly, which is
   why they were found. All six are repointed at `build … --check-only`.
7. **`missing_input_suggests.rs`'s floor is met by two *invocations of one verb*, and that is
   not padding.** `build x.tdm` reaches `cannot_read` from `cmd_build`'s own
   `read_to_string`; `build x.tdm --check-only` reaches it from `lint::collect_diagnostics`,
   the call site the retired verb owned. Two code paths, two front doors an author types. The
   list is `&[&["build"], &["build", "--check-only"]]` now.
8. **`every_parsed_flag_is_documented_in_its_subcommand_help`'s floor drops 6 → 5**, and the
   *wrapped-synopsis* test lost its only subject: `command_synopsis` joins a synopsis that
   wraps onto an indented continuation, and `check`'s was the one that did. `build`'s now
   does, because `--check-only` pushed it over, so the test is repointed there and asserts
   `--check-only`/`--strict`/`--jobs`/`--no-exec`/`--format json` are all reachable only by
   joining.

**Five judgement calls, and how they went.**

- **`build --check-only`, not a `taliesin lint` verb.** Wave 8 reached the ruling's target of
  nine verbs; a tenth would give it back for a command that shares `build`'s arg parsing, its
  validator set and its `--format json` shape. The flag also lets the gate be *stated* as
  what it is: the same build, stopped before it writes. It **refuses** `--out` / `--stdout` /
  `--bare` / `--jobs` rather than ignoring them, because `build x --check-only --out dist`
  that quietly produces no `dist/` is the trap `--stdout`'s existing conflict check was
  written against.
- **`--errors-only`, `--require-kernel`, `--explain` and the whole interpreter probe are
  gone, and `--strict` is the only knob left.** The probe is `doctor`'s job and always was;
  `--errors-only` existed so a suggestion could not fail CI, which is what `Severity` does
  structurally now; `--explain` had nothing left to read. Three flags to one, and `Floor`'s
  three-state enum collapses to a `bool`.
- **`docs/internals/validation.tmd` was REWRITTEN, not deleted.** The playbook deletes all
  254 lines, but only the last 76 were about the verb: the rest documents the closed
  vocabularies, the did-you-mean rules, the retirement registers, the prose linter and the
  `_site.yml` schema, all of which are live. Deleting the chapter to remove two sections
  would have taken the only prose explanation of the machinery every later wave depends on.
  The two sections are rewritten and a new one, **"Severity is a field, not a
  classification"**, records what the table cost.
- **`diaglink.ts` + `termlinks.ts` KEPT; `checkstatus.ts` + `decorations.ts` cut.** The
  playbook deletes all four as "the check chain". Only two of them were: Explorer badges
  re-lint the whole project on every save to decorate a tree the author is not reading, and
  the Problems panel already holds the same findings on demand. The terminal-link half is not
  about the verb at all: its pattern is `^(\S+?\.tmd)(?::(\d+))?:\s`, severity-agnostic and
  code-free, and the pre-push wiring below makes it *more* useful, not less. `taliesin.check`
  the command, the `taliesin.explorerBadges` setting and `walkthroughs/check.md` go with them.
- **The `[CODE]` bracket comes out of `taliesin run` too, and the problem matchers with it.**
  `run_print.rs` hand-wrote `error[TAL-CELL-ERROR]` / `error[TAL-KERNEL]` to be matchable by
  `$taliesin`, so the matcher regexes lose their code group in the same commit. The
  distinction those codes carried survives where it is read: the message says "raised an
  uncaught exception" or "did not run", and `kernelfail.ts` keys its doctor hint on those two
  strings instead of on a wrapped `code` object. That removes a fork the companion had to
  understand (the language client delivered `code` as `{value, target}`, a problem matcher as
  a bare string) and `kernelfail.test.ts` is the drift gate on the Rust format strings.

**Wired: the document gate the `check` verb never had.** `.githooks/pre-push` gains a fourth
step, `build docs/guide --check-only --no-exec`, so a broken cross-reference or a dead link in
this project's own manual cannot reach a reader with every gate green. `--strict` is
deliberately absent: advice must not block a push.

**Proven by running the commands, not by grepping.**

```
taliesin check .          → `check` was removed: `build <file|dir> --check-only` lints
                            without writing, and takes `--strict` and `--format json`
                            the same way                                        (exit 1)
taliesin check --explain TAL-FM-KEY → the same note (a retired verb answers by name)
taliesin build docs/guide --check-only            → no problems found            (exit 0)
taliesin build docs/guide --check-only --out /tmp/zzz
                          → error: --check-only writes nothing, but --out /tmp/zzz
                            describes output. Drop one.   (and /tmp/zzz never existed)
taliesin build docs/guide --check-only --stdout    → the same refusal
```

And the `new --help` claim, checked end to end from a clean temp dir rather than restated:
`init smoke` + `new post hello` writes three files, `build . --check-only` reports
`no problems found` and exits 0, `--format json` emits `{"diagnostics": []}`, and `find`
shows the same three files afterwards.

**Swept and clean:** all five retirement registers (`RETIRED_KEYS`, `RETIRED_DIV_CLASSES`,
`RETIRED_COMMANDS`, `RETIRED_NEW_KINDS`, `RETIRED_CELL_LANGS`) name nothing this wave
removed, checked by reading each note body, per wave 8's lesson; every surviving `--help`
block, run through the real binary and grepped for `check`/`TAL-`/`explain`/`badge`;
`tools/ui-audit/`, which names nothing this wave removed (waves 7 and 8 found it empty too);
`tools/gates.sh` and `.github/workflows/ci.yml`, neither of which ever invoked the verb; and
the `data-*` census in `token_contract.rs`, unchanged, as it must be, because this wave
removed no emitter. `docs/superpowers/` is byte-identical.

**The cargo-deny log was checked, per wave 4's lesson, and this wave removes no dependency.**
`advisories ok, bans ok, licenses ok, sources ok`. The one
`license-exception-not-encountered` warning is still `libfuzzer-sys`, still **pre-existing and
unrelated** (not in `Cargo.lock`, no fuzz target in the tree). `deny.toml` was not edited, so
the stale row is carried forward exactly as wave 6 left it.

**What was given up, stated plainly.**

**518 lines of hand-written cause-and-fix prose, one per diagnostic code, and it is the one
loss in this wave that is genuinely irreversible in a way the validators are not.** The
ruling's dissent called it a week of writing and that is the right order of magnitude. It was
reachable three ways (`check --explain <CODE>`, a `codeDescription` link from every squiggle,
and the generated `docs/DIAGNOSTICS.md`), and `lsp_diag.rs`'s header recorded the research the
hover placement rested on: Barik et al. (ICSE 2017, eye-tracking) measured that reading an
error message costs about as much as reading source, which makes the moment you send someone
to a browser the moment they stop. Preserved verbatim in
`notes/retired/diagnostics-explanations.rs` before the wave started, so it is recoverable as
text; what is not recoverable cheaply is the judgement of which cause and which canonical fix
each family deserved.

**The stable code itself, which was the tool's agent-facing contract.** An agent matching on
`.diagnostics[].code` now matches on message prefixes instead, which is exactly the fragility
the codes existed to remove. The trade is that the codes were *derived from those same
prefixes* by an ordered substring table, so the stability was one indirection deep and the
finding above shows it was already wrong in the corpus. `--format json` survives as the one
machine surface, with `severity`, `file`, `line`, `col`/`end_col` and the structured
`suggestion` an editor applies as a quick fix.

**Four a11y rules and two structural families, one of which had a measured provenance.**
The accessible-name rules (icon-only `<a>`/`<button>`, plus the `[role=button|link|tab]`
variants) and WCAG 2.5.3's label-in-name mismatch are gone; `visible_label`'s
`aria-hidden`-subtree handling encoded a real shipped defect (the search button's `⌘K` hint
was *painted* but is not part of the accessible name, so counting it accused the correct fix
of being the defect). `docs/guide/reference/accessibility.tmd`'s 2.4.4 row now rests on the
audit rather than on a linter. `math_render.rs` went on the "you can see it" test, which is
true (KaTeX paints a red error span) and is the weakest application of that test in the wave:
a reader sees it, an author skimming their own long page may not.

**`--errors-only` as an escape hatch.** A project with a warning it has decided to live with
can no longer gate on errors alone; it fixes the warning or drops `--check-only` from CI.
Measured on this repository: zero of the twelve projects need it.

**A note for a later wave.** Five `Warning` construction sites fall through to `Error` today
without any validator having chosen it: the footnote-flattened warning (`render/mod.rs`, the
note *renders*, just inline, so `Warning` is arguably right), `theme file not found` ×2 plus
the refused-theme message, `duplicate cross-reference label`, and the listing's
`has no title:`. None fires anywhere in the corpus, so none was observable in the diff above,
and all five are left at `Error` deliberately: preserving the old fall-through is a
translation, and re-tuning severities is not a cut wave's job.

### Wave 10, 2026-08-09, `cut/wave-10-lsp`

**Measured reclaim: −9,838 lines** (`+180 / −10,018` over 38 files, **17 deleted outright**)
against the ~9,105 estimate. By area: `crates/server/src` −7,606, `editor/vscode` −1,701,
`crates/server/tests` −387, `docs/guide` −127, `docs/internals` −11, `crates/core/src` −9,
root +3. `notes/` is excluded, as in every figure above.

**`./tools/gates.sh` is GREEN on the committed tree:** **8/8** gates, **2/2** canaries
(`kernel_executes_state_errors_and_interrupts_runaway_cell` and
`only_a_textual_sink_becomes_a_live_region` both printed `... ok`), **86 suites / 1,507
passed / 0 failed / 0 ignored**, exit 0. Measure wave 11 against **1,507, two canaries and
eight gates**; a bare `cargo test --workspace` gives the same figure. The 159-test drop from
wave 9's 1,666 is the nine deleted modules' own suites plus the lsp.rs and lsp_stdio.rs
blocks for the cut surfaces.

**NINE CLI VERBS ARE STILL NINE, and no register entry was owed.** This wave retires
protocol methods, not author vocabulary: a client that asks for `textDocument/rename` gets
JSON-RPC `MethodNotFound`, which is the protocol's own answer and reaches the client with
the method named in it. There is no `.tmd` file anywhere containing the string
`textDocument/rename`, so there is no silence to prevent. That is the same reasoning wave 3
recorded for `TAL-DEBUG-TRACE`.

**SIXTEEN ADVERTISED PROVIDERS ARE NOW SEVEN, and every one of them answers a question.**
Surviving: `completion`, `hover`, `definition`, `documentSymbol`, `codeAction`,
`foldingRange`, `codeLens`, plus pushed `publishDiagnostics` and three namespaced
extensions (`taliesin/cellRegions`, `siteMap`, `mathCommands`). Gone: `documentLink`,
`rename` (with `prepareRename`), `formatting`, `inlayHint`, `documentHighlight`,
`references`, `selectionRange`, `workspaceSymbol`, the 3.17 `diagnostic` pull model, and the
five custom methods `sectionEdit` / `insertEdit` / `renameFileEdits` / `projectOutline` /
`projectRefs`. **Five of those were write paths** (formatting, rename and the three
edit-producing extensions), which is the real shape of this wave: the `.tmd` file is the
single editing surface, the preview is forbidden from writing to it, and a language server
that sometimes rewrote it was the same rule with a second owner.

**BEHAVIOURAL VERIFICATION, over real stdio with the release binary, because a compile
proves nothing here.** Driven against a scratch project with a broken `@fig-nope`, a
`{python}` fence and a `:::` div, from a client that **declares pull support**:

```
providers: 7 ['codeActionProvider', 'codeLensProvider', 'completionProvider',
              'definitionProvider', 'documentSymbolProvider', 'foldingRangeProvider',
              'hoverProvider']
diagnosticProvider present: False
publishDiagnostics count: 1
   diag: line 6 col 0  'broken cross-reference: @fig-nope (no such figure/section/…)'
codeLens: err=None n=1        foldingRange: err=None n=4      cellRegions: err=None n=1
rename / formatting / textDocument/diagnostic: error -32601 'unhandled request: …'
```

So the client that used to be answered by pull is answered by push instead, rather than
falling silent, which was the risk of deleting `Transport` rather than keeping its Push
arm. `cargo test -p taliesin-core` is unchanged pass-for-pass (576), as required.

**Measured, not asserted.** Two `cargo build --release -p taliesin-server` runs into
separate target dirs, same toolchain, `main` in a throwaway worktree against this branch:
**30,791,664 → 30,375,952 bytes, −415,712 (−1.35%)**. Four times wave 9's binary win, from
roughly twice the lines. **THE MOAT IS BYTE-IDENTICAL TO MAIN**, checked as an explicit
`git diff --stat` over the path list: `crates/core/assets/`, `web-client/`,
`serve_site/exec_pool.rs`, `freeze.rs`, `exec.rs`, `kernel.rs`, `diff.rs`. So this wave ships
**no asset change at all** and there is no per-page byte win to report, the same shape as
waves 8 and 9. The `data-*` census in `token_contract.rs` is likewise untouched, as expected
for a wave that removes no emitter.

**Seven things that were not true, or that the playbook did not know.** Same genus as waves
1 to 9:

1. **The compiler found four dead helpers in `lsp_nav.rs` and a whole struct pair in
   `lsp_project.rs` that the plan never mentions.** `anchor_at`, `anchor_occurrences`,
   `xref_occurrences`, `anchor_highlights` and `is_anchor_site` were rename's and document
   highlight's alone; `ProjectHeading` and `ProjectUse` existed only for `workspace/symbol`
   and `projectRefs`, so `ProjectScan` collapses from five fields to **one** (`anchors`) and
   `walk` stops calling `lsp_outline::sections` and `xref_occurrences` per page. That is
   ~400 lines the estimate did not carry, and none of it would have been flagged if these
   had been `pub` in a lib rather than `pub(crate)` in a bin.
2. **`Bibliography::contains` was `pub` in `taliesin-core` for `lsp_insert` alone, and
   `pub` in a lib crate is invisible to dead-code analysis.** Its doc comment said so in
   as many words ("Public because `taliesin lsp` needs it when the author pastes a BibTeX
   entry"), which is the only reason it was findable. Deleted. `build.rs`'s `inside_repo`
   was the same shape one crate over, and `pub(crate)` in a **bin** *is* checked, so that
   one would have been caught anyway; it is now private. **The lesson for wave 11 and 12:
   grep `crates/core/src` for doc comments justifying a `pub` by naming the consumer, before
   trusting clippy to find the orphan.**
3. **THE PLAYBOOK CONTRADICTS ITSELF ABOUT `lsp_trace.rs`, and one of the two is a wave-3
   note.** Its line 344 says "NOT part of this bundle … Do not delete it: it is the only
   method that can measure the LSP bundle", while wave 10's step 1 deletes it. Both are
   right in their own place: line 344 sits under **wave 3's** "Must survive" list and exists
   because phase-1 matched the file on the word *trace* and nearly swept it into the debug
   cut. Once wave 10 executes, the instrument has nothing left to decide. It also never
   measured anything: the 2026-08-07 audit records that `TALIESIN_LSP_TRACE` was **never
   armed** and no trace file exists on this machine, so deleting it costs a capability that
   produced zero observations in the four days it existed. Deleted, per the standing
   directive.
4. **A WIRE TEST WHOSE SUCCESS CONDITION IS "no error came back" DOES NOT PASS VACUOUSLY
   HERE. IT FAILS, WHICH IS BETTER, AND THE FIX IS STILL NOT TO WEAKEN IT.** Wave 9's
   lesson predicted the analogue and it landed on `a_cancelled_request_is_answered_rather_
   than_run`, which drove `workspace/symbol` twice and asserted the *live* one came back
   with `error: None`. After the cut that is `-32601`, so it failed loudly. Both cancellation
   tests are re-pointed at `documentSymbol` (a request the server still handles) rather than
   relaxed, and `read_batch`'s doc comment (which justified the whole feature on
   `workspace/symbol`'s measured 167 ms walk) now says the measurement belonged to a method
   that is gone and states the loop property on its own terms.
5. **`server_capabilities()` has a mutation history and the obvious edit would have re-opened
   it.** Its own gate records that **all twelve mutants survived the 2026-07-27 run, including
   replacing the whole body with `Default::default()`**: a server advertising *nothing*
   passed the entire suite. Deleting nine `assert_eq!`s from
   `the_initialize_handshake_advertises_every_feature_the_editor_needs` would have shrunk the
   only thing standing between that and a silent regression. The nine are **inverted instead**:
   the test now asserts each retired provider key `is_null()`, so re-advertising one is a
   decision that fails a test rather than a detail that arrives unnoticed.
6. **`the_internals_capability_table` carried a wire-name-to-prose-name special case that
   died with its capability.** `documentFormatting` → `formatting` was the one place the
   advertised key and the book's row differed, and its comment explained that renaming the
   row to satisfy the test would be "the test writing the documentation". With formatting
   gone the mapping is the identity, so the special case is deleted rather than left as a
   branch nothing takes. `advertised.len() >= 12` → `>= 7` per the plan; the sibling gate
   `the_internals_book_documents_every_taliesin_namespaced_method` self-corrected to a floor
   of 3, which is exactly the truth.
7. **`RETIRED_COMMANDS`' `map` note had gone stale, and this is the second wave to catch that
   class.** It read "nothing on the CLI; `taliesin lsp` answers the project outline in your
   editor", and `taliesin/projectOutline` is one of the five methods this wave deletes.
   `map`'s actual successor is `taliesin/siteMap`, which survives, so the note now names it.
   Wave 8 recorded that a retirement note can go stale with every gate green; that is now a
   measured recurrence, not a hypothesis. **Grep the registers for every name you remove,
   including protocol names that no register mentions directly.**

**The judgement call, and how it went.** `taliesin/mathCommands` and `taliesin/siteMap`
survive alongside `cellRegions`, which is what the playbook's "five retired method consts,
keeping only `CELL_REGIONS_METHOD`" arithmetic actually specifies (five of the eight, and the
two survivors sit after the range). Both earn it on the doctrine rather than on affection:
`siteMap` is what makes "Preview" open at the chapter you are editing instead of the book's
cover, and deriving the URL in TypeScript is the second implementation the whole LSP rewrite
existed to delete; `mathCommands` is the one shape completion cannot serve, because a symbol
you cannot spell is unreachable by typing a prefix of it. `lsp_lens` survives on the recorded
ground the plan names: `runcell.ts:9-14` records that a TypeScript `CodeLensProvider` already
existed here and was deleted for the Rust one, so cutting the server lens regrows demonstrated
pressure for the duplication CLAUDE.md forbids.

**What was given up, stated plainly.**

**Anchor maintenance in a long book is now entirely manual.** `references` and
`renameFileEdits` were the only two capabilities in the tool that answered a question at the
*book* boundary rather than the file boundary, which is the boundary a 25-chapter project is
actually authored at, and `rename` went with them, so renaming a cross-reference anchor is
now find-and-replace across the project, by hand, with nothing checking that the fragment on
an external URL was left alone. That last rule was a real bug fixed once: accepting every `#`
meant renaming a section also rewrote `[x](https://example.com/p.html#id)`, silently
retargeting a fragment on someone else's page.

**`lsp_format`'s whitespace-equivalence proof and `lsp_rename_file`'s two-link-spelling rules
were investigations, not code.** The defence graded the irreversibility correctly:
`inlayHint`, `documentLink`, `selectionRange` and `documentHighlight` are thin projections of
surviving machinery and cost a weekend each to rebuild, but re-deriving the other two means
re-running the investigation. Everything is recoverable in full from the `pre-cut` tag.

**The Problems panel now shows the chapters you have open, and nothing else.**
`workspace/diagnostic` was the only way to list the whole book from inside the editor. The
replacement is `build <dir> --check-only` from a terminal, which is the same findings and is
where the author already runs them before publishing. But it is a command you remember to
type, not a panel that is already correct. `docs/guide/reference/cli.tmd` now says that
plainly rather than promising the panel.

**The companion loses five author-facing gestures**, all of which were genuinely useful and
none of which was language intelligence: paste an image (figure block + caption cursor), paste
a spreadsheet (aligned pipe table), paste a BibTeX entry (`[@key]` + a `.bib` append as one
undo), drop a file (a reference the build can ship, with the containment verdict `build`
itself applies), and rename a `.tmd` (every inbound reference repaired inside VS Code's rename
transaction). The four structural commands (move section up/down, promote/demote heading) go
with them: they were the sanctioned replacement for the drag-to-reorder gesture removed for
breaking the single-editing-surface rule, and the honest reading is that the rule takes them
too. The **Project** sidebar (outline in reading order, cross-references with their uses and
the dangling ones grouped, the numbered-float index) is gone as a read-only view that cost two
custom methods, a `TreeView` and 694 lines of TypeScript to project answers the server was
already computing.

**Two gates got weaker in a way worth naming.** `crates/server/tests/lsp_stdio.rs` drops from
13 wire tests to 8, and the e2e suite loses the paste/drop and rename-repair suites entirely.
Those were the tests only a real Extension Host could write, and the capabilities they covered
are gone, so nothing is left unguarded. But `missing_input_suggests.rs`-style arithmetic
applies here too: the *surviving* wire tests are now a larger fraction of a smaller surface,
and each survivor's success condition was re-read rather than trusted by name.

### Wave 11, 2026-08-09, `cut/wave-11-serve`

**Measured reclaim: −5,015 lines** (`+768 / −5,783` over 80 files, **5 deleted outright**)
against the ~5,107 estimate. By area: `crates/server/src` −3,225, `crates/server/tests`
−854, `crates/core/src` −717, `docs` −155, `crates/core/tests` −108, `corpus` −36,
`site` −14, `web-client` −14, `crates/core/assets` −2, `editor` −1, and `tools` **+98**
(`build-site.sh`, deliberately) and root +13. `notes/` is excluded, as in every figure
above.

**`./tools/gates.sh` is GREEN on the committed tree:** **8/8** gates, **2/2** canaries
(`kernel_executes_state_errors_and_interrupts_runaway_cell` and
`only_a_textual_sink_becomes_a_live_region` both printed `... ok`), **83 suites / 1,443
passed / 0 failed / 0 ignored**, exit 0. Measure wave 12 against **1,443, two canaries and
eight gates**; a bare `cargo test --workspace` gives the same figure. The three suites gone
from wave 10's 86 are `mount_serving_live`, `mount_static_build` and `build_jobs`.

**NINE CLI VERBS ARE STILL NINE.** This wave retires two FLAGS, `--bare` and `--host`, and
one config key, `mounts:`.

**THE FREEZE, AUDITED BY EYE AND NOT BY A GREEN TEST, which is the whole reason this wave
exists.** `git diff main -- crates/server/src/serve_site/exec_pool.rs` is 21 lines and every
one of them is above `get()`: the `warm_pool` field, the second `ExecPool::new` parameter,
the `set_warm_pool` call in `make()`, and three doc comments. Grepping the diff for `mru`,
`MAX_WARM_PAGES`, `execs.remove`, `pop()`, `retain` and `insert(0` returns **nothing**.
`MAX_WARM_PAGES` also stops lying: it was per-project, so `site/`'s six mounts permitted up
to 7x the cap resident, and after the mounts cut the number means what its name says.

**THE HARD BLOCKER WAS REAL AND IT WENT FIRST.** `tools/build-site.sh` did not exist;
`build.rs` recorded that the shell-script alternative had already shipped this project's own
call-to-action with a 404 (item 149), and removing `mounts:` without a replacement would have
repeated it. The script is written, and it **verifies rather than merely builds**: it greps
every `docs/`- or `gallery/`-prefixed link out of `site/_site.yml` and `site/*.tmd`, resolves
each against the composed output, and exits non-zero naming any that has nothing behind it.
`.githooks/pre-push` gained a fifth step, `tools/build-site.sh --check` (`--no-exec`, into a
temp dir, 1.1 s). Verified against the pre-cut build: the composed tree is **file-set
identical to what `build site` with `mounts:` produced, 126 files**, and all 7 cross-project
links resolve.

**BEHAVIOURAL VERIFICATION, because two of these are security assertions nothing automated
covers.** Against a live `preview corpus/tech-blog` on 4388, with real curl:

```
Host: 127.0.0.1:4388            -> 200      Origin: http://evil.example   -> 403
Host: localhost:4388            -> 200      Origin: https://evil.example  -> 403
Host: evil.example              -> 403      Origin: null                  -> 403
Host: 127.0.0.1.evil.example    -> 403      Origin: http://127.0.0.1:4388 -> 101
/search-index.js, Host: evil.example -> 403 Origin: http://localhost:9999 -> 101
```

So the DNS-rebinding guard survived the `lan_ip` removal and is still layered over every
route (not just the page route), and `ws_origin_ok` did **not** become a no-op: a hostile
origin cannot open the socket, which is the only thing stopping an open tab from sending
`restart_kernel` and destroying the warm kernel. The console prints `ready` and `watch` and
no `network` line.

**THE BYTE-DIFF OF `corpus/tech-blog` IS NOT CLEAN, AND THE ONE DIFFERENCE IS WORTH THE
PARAGRAPH.** Deduplicated across all 54 files, the entire diff is **two lines**: one JS
comment in `code-enhance/01-registry.js` that named `--host`, and the `app.<hash>.js` src
that changed because of it. `minify_js` went in wave 4, so JS comments now ship verbatim and
a comment edit is a shipped-byte edit. Everything else is byte-identical, checked by name:
**402 `data-block-id`, 373 `data-sourcepos`, 9 `data-source-file`** (hashes of the sorted
occurrence lists compared), `blog.xml` (3,994 B), `search-index.js` (121,312 B),
`sitemap.xml`, `robots.txt`, `projects.xml`, the hero markup and all four listings. Filtering
those two lines out leaves **0** differing lines.

**Measured, not asserted.** Two `cargo build --release -p taliesin-server` runs into separate
target dirs, same toolchain, `main` in a throwaway worktree: **30,376,272 → 29,885,200 bytes,
−491,072 (−1.62%)**, the largest binary win of the campaign so far and ahead of wave 10's
−415,712 from half the lines. The CSS bundle is unchanged apart from three comment edits, so
there is no per-page CSS win to report.

**Eight things that were not true, or that the playbook did not know.** Same genus as waves
1 to 10:

1. **A TEST WAS ASSERTING ON A DOC COMMENT, and only deleting the comment could show it.**
   `the_pre_paint_canvas_map_tracks_the_theme_tokens` read `--tali-bg` out of the first block
   after `:root` in `TOKENS_DARK_CSS`. That file **has no `:root` block**: it is
   `html[data-theme="dark"]` throughout, and the only `:root` in it was inside a comment
   explaining how `bare_theme_css` flattened the prefix. The comment happened to sit directly
   above the real block, so the scan landed on the right value for four months by accident.
   Removing the `--bare` mention made it panic `no :root block in the token CSS`. Re-pointed
   at the selector the file actually uses. **The genus is wave 9's derived-classification
   finding: when a test locates its subject by a string that also appears in prose, the prose
   is load-bearing and nothing says so.**
2. **MAKING A FLAG CONST `pub(crate)` SILENTLY DROPPED IT FROM THE DRIFT GATE.**
   `every_parsed_flag_is_documented_in_its_subcommand_help` finds each `<PREFIX>_FLAGS` by
   requiring the literal `const ` at column 0. `RETIRED_FLAGS`' own gate needs to read
   `BUILD_FLAGS` and `SERVE_FLAGS`, so both became `pub(crate) const`, and the scan stopped
   seeing them, taking `build`'s and `preview`'s whole flag sets out of the comparison. Only
   the scan's own **floor assertion** (`lists.len() >= 5`, collected 3) caught it, which is
   exactly the "a scan that finds nothing would pass every assertion below it" guard its
   author wrote it for. The scan now accepts an optional visibility prefix.
3. **`stale_sweep.rs` MUST NOT BE DELETED, and the playbook's STAGE 1e says to.** The file
   contains no mount reference at all; its subject is the stale-output sweep, which survives
   and is now *more* load-bearing than before, because the sweep is precisely why
   `build-site.sh` must build the parent first. Kept.
4. **THREE OF THE SIX SITE-LAYER STAGES WERE ALREADY PAID.** STAGE 3 (social cards) and
   STAGE 4 (the PWA manifest) both went in wave 4. STAGE 5 (the dead hero-image branch) went
   on 2026-08-02: `HeroSpec` has no `image` field and `frontmatter.rs` already carries a test
   asserting a retired `hero.image:` never reaches it. Read the code before executing a stage.
5. **THE TWO `mounts entry key` RETIRED_KEYS ROWS WENT STALE THE MOMENT THE KEY DID.** Both
   said "write `mounts:` as a mapping of URL prefix to project directory instead": an
   instruction to write a key that no longer exists, in a register whose whole job is to name
   a live successor. Deleted; the new `config key`/`mounts` row answers instead. This is the
   **third** recurrence of wave 8's lesson (wave 10 found `map`'s), and the first where the
   stale rows were in a *scope* the cut removed rather than in the key itself.
6. **A FOURTH REGISTER IS RIGHT HERE, AND IT IS RIGHT BECAUSE IT SERVES TWO ENTRIES.** Wave 3
   declined a register for `TAL-DEBUG-TRACE` on the ground that machinery serving one entry is
   not worth it; wave 8's `RETIRED_NEW_KINDS` is the counter-precedent. `RETIRED_FLAGS` lands
   in `serve/mod.rs` beside `unknown_flag_error`, carries **`--bare` and `--host`**, is keyed
   on the flag alone (so `build --host` is answered too) and is consulted **before** the
   did-you-mean, which matters: with the flag merely deleted, `--bare` fell through to
   `BUILD_FLAGS` and `--host` to `SERVE_FLAGS`, and CLAUDE.md forbids answering a retirement
   with a did-you-mean because `codes::extract_suggestion` lifts that phrase into a mechanical
   fix. One derived test over the register, mirroring `a_retired_kind_names_what_to_do_instead`;
   no per-entry tombstone.
7. **`adopt_forked` TOOK MORE WITH IT THAN THE PLAN LISTS.** `wait_until_reachable` (the
   TCP-probe loop that existed because a forked kernel binds its ZMQ ports a beat after the
   daemon reports its PID) had no other caller, and its error string was a row in
   `start_error_is_transient`'s test. `KernelSpec::kernel_name` was `warm_pool::warm_one`'s
   alone. `runtime_dirs.rs` lost `warmpool_dir` **and** `WARMPOOL_PREFIX`, which forced a real
   decision: the sweep's live-non-own-pid row used a `tali-warmpool-` dir as its fixture, so it
   is re-pointed at `tali-kernel-` rather than deleted. `prepare_connection` stays, as
   required, and `KernelProc` collapsed to a bare `Child` rather than to a one-variant enum
   (wave 5's precedent).
8. **THE PLAYBOOK'S OWN DISSENT WAS RIGHT ABOUT THE COST, AND THE COST IS VISIBLE.** With
   `mounts:` gone, `build site` reports **12 broken cross-project links** (the nav, the two
   `docs/guide/` CTAs, the four gallery cards). Those links are correct in the composed deploy
   and unresolvable from inside `site/`, so the diagnostic is true. Three ways out were
   considered and rejected: making the site checker skip root-absolute links (a real weakening,
   since `corpus/tech-blog/404.tmd` links `/` and `/blog.tmd` and those ARE validated today),
   writing the links as absolute URLs against `url:` (breaks the local composed preview), and
   simply accepting silence. What ships instead is the honest pair: the tool reports what it
   can see, and `build-site.sh` resolves the links against the composed output and says so out
   loud before the site build runs.

**The judgement call, and how it went.** The dissent's strongest point was that a shell script
nobody runs is how item 149 happened. That is answered structurally, not by promising to run
it: the script is a **pre-push step**, and it **asserts** rather than builds. What it checks is
also strictly stronger than what `mounts:` gave: `under_mount` only tested that a link's
prefix was *declared* in the config, never that anything was behind it, so a mount pointing at
an empty directory passed. The script tests the file.

**What was given up, stated plainly.**

**The warm pool, and the ~1.9 s spinner it was buying.** Recorded knowingly, as the plan asked:
every fresh code-cell page in preview regains a `warming-kernel` state on its first cell, and
past `MAX_WARM_PAGES = 6` an evicted page pays it again. `exec.rs`'s
`pooled_kernel_serves_cells_without_a_long_warming_state` was the user-facing contract and it is
deliberately deleted. The dissent's case is narrow but real and parallelism cannot substitute
for it in preview, because `spawn_builder` is a single task draining an mpsc: preview builds
pages strictly one at a time. What goes with it is 1,621 lines of `warm_pool.rs` (a forkserver
daemon written as an embedded, unlinted Python program, `set_forkserver_preload`ing numpy /
matplotlib / torch, plus the `ready + in_flight <= cap` accounting whose off-by-one would
transiently overshoot the RAM budget), the `Forked` kernel process model, and `build_budget.rs`'s
whole split half. The memory-aware `concurrency_cap` survives untouched, as ruled: it is a
safety mechanism, not a perf knob.

**`--host` plus the QR code: the only in-tool way to put a live, hot-reloading preview on a
phone in under five seconds.** The ruling cut it and the dissent is worth restating, because it
is the one question only the author can answer: the bundled CSS carries 53 `@media` blocks and
the honest way to check a phone layout is now `build` plus a static file server, or a deploy.
What is bought is a smaller trusted surface: the session token, its cookie, the LAN guard
middleware, the `?t=` strip in the client, `local_ip`, `print_qr` and the `qrcode` dependency
are gone, and `origin_allowed`/`host_allowed` each lost a parameter and a whole mode. Two modes
collapse to one on the path that runs every day.

**`mounts:`, and with it the ability to preview the composed site.** `preview site` now shows
the marketing site alone; the Guide and Internals links go nowhere until the deploy is composed.
That is the sharpest daily loss in this wave, and it is why the callout in `docs/internals/sites.tmd`
now says to preview each project directly.

**The warm-path Cmd-K refresh.** A heading renamed in a live preview no longer surfaces under
its new text until the page set changes or a cross-reference anchor moves; the index is rebuilt
whole on an anchor move (the correctness guard) and at discovery, and nowhere else. This
re-opens a defect the author had closed (`serve_site/mod.rs`'s own comment explained that a
stale index makes a search snippet contradict the page it links to), and the trade is 40 lines
of per-page re-render under a panic guard, plus `page_search_fragment` /
`install_search_fragment` / `refresh_search_for_page` in `taliesin-core`, all of which were
`pub` and so invisible to dead-code analysis (wave 10's lesson, applied on purpose rather than
discovered).

**`--bare`.** Zero-`<script>`, zero-CDN, CSS-only-theme single-doc output, its `bare_theme_css`
prefix-rewriting trick, `strip_tali_js_scripts` (driven off the `CLIENT_LANGS` registry so a
second client language could not silently break the zero-script contract), and
`corpus/bare-draft.tmd` with it.

### Wave 12, 2026-08-09, `cut/wave-12-justification`

**Measured reclaim: −4,627 lines** (`+266 / −4,893` over 61 files, **30 deleted outright**)
against the ~5,976 estimate; the gap is almost exactly the steps that were already spent
(`corpus/graphics3d`, 920 lines, and two Internals chapters cut with waves 3 and 5). By
area: `corpus` −2,498, `docs` −2,004, `crates` −126, `site` −9, `CLAUDE.md` **+6**,
`tools` **+4**, `editor` and `web-client` 0. `notes/` is excluded, as in every figure
above.

**`./tools/gates.sh` is GREEN on the committed tree:** **8/8** gates, **2/2** canaries
(`kernel_executes_state_errors_and_interrupts_runaway_cell` and
`only_a_textual_sink_becomes_a_live_region` both printed `... ok`), **82 suites / 1,439
passed / 0 failed / 0 ignored**, exit 0. `tools/build-site.sh --check` composes 5
sub-projects and resolves **6** cross-project links (was 7; the course card's link went
with the exhibit). Measure any later wave against **1,439, two canaries and eight gates**.

**NO CODE SHIPPED.** The diff touches exactly two lines of non-test Rust
(`render/tests.rs`, which is `#[cfg(test)]`, and a comment inside `site/mod.rs`'s
`mod tests`) and **zero** bytes of `crates/core/assets/` or `web-client/`. So there is no
binary win and no per-page byte win to report, and none was measured: the release binary
stays at wave 11's **29,885,200 B** by construction, not by claim.

**THE STATED DELIVERABLE WAS IMPOSSIBLE, AND ITS REPLACEMENT REFUTED THE PREMISE.** The
playbook ends: *"`taliesin features corpus` must now report a NON-EMPTY unused tail. That
is the point of the whole wave."* That verb was cut in wave 2. The instrument that
survives is the validator consts, so the deliverable was re-specified as: **scan the whole
shipped `.tmd` read set for every name the live vocabulary offers, and report the names
nothing witnesses.** Six registers, 63 names (`KNOWN_KEYS` 19, `NATIVE_KEYS` 12,
`CELL_OPTION_KEYS` 12, `XREF_LABELS` 12, `CALLOUT_KINDS` 3, `DIV_FEATURE_CLASSES` 3,
`SHORTCODE_NAMES` 2), read set = every `.tmd` under `corpus/` (minus `corpus/diagnostics/`,
which trips validators on purpose), `docs/guide`, `docs/internals`, `site`, `samples`.

Run on this branch and, for comparison, on `main` in a throwaway worktree:

| | read set | unused tail |
|---|---|---|
| `main` (before) | 127 documents | **10 of 63** |
| this branch (after) | 110 documents | **10 of 63** |

**Identical.** The tail is `csl:` (front matter), `head:` and `python:` (`_site.yml`), and
the seven theorem-family xref prefixes `cor`/`def`/`exm`/`lem`/`prp`/`rem`/`thm`. Zero
unused cell options, callout kinds, div classes or shortcodes, before and after.

That is a real finding and it goes the other way from the playbook. **The premise was that
the corpus's full-coverage answer was an artefact of the documents this wave deletes.
Measured, it is not**: the 17 documents cut were duplicates and fiction that used only
vocabulary other documents also use, so removing them removed no *last* witness of
anything. The tail was already non-empty and is unchanged. What the wave actually
delivered is the other half, 4,627 lines of justification layer gone with **zero** loss of
vocabulary coverage and zero loss of sweep power, and the tail it hands the next round is
the same tail `main` already had. Note also that the seven xref prefixes are exactly what
CLAUDE.md's `vocab.rs` paragraph already asserts from the consts ("the other seven resolve
a label for a construct nothing can define any more"); this is an independent
confirmation, not a discovery.

**THE JUDGEMENT CALL THE HANDOFF ASKED FOR, MADE EXPLICITLY: `corpus/tarn` is KEPT,
`corpus/course` is CUT.** Both are fiction and the standing directive says cut, so this
needs its reasoning on the record.

- **`course/` (5 pages, 145 lines; `course.rs`, 56 lines, 2 tests) is cut** because its
  unique coverage is one property and that property re-fixtures for six lines.
  `corpus.rs::book_chapter_scopes_float_numbers_across_chapters` already pinned
  chapter-scoped **figure** numbering and cross-page `@fig-` refs on `corpus/demo-book`;
  `tarn.rs::cross_page_section_refs_number_by_chapter` already pinned the `@sec-` half.
  What only `course` had was the **equation** float kind. `demo-book/intro.tmd` gained one
  labelled display equation and `methods.tmd` labelled the one it already carried and now
  references the intro's, and the existing test grew six assertions. Those assertions are
  **stronger** than the ones deleted: `course.rs` asserted `mle.contains("1.1")`, the
  substring `1.1` anywhere on the page, where the new ones pin the exact
  `<span class="tali-eqn-number">(2.1)</span>` badge and the exact
  `<a href="intro.html#eq-euler" …>Equation&nbsp;1.1</a>` link.
- **`tarn/` (14 pages, 634 lines; `tarn.rs`, 446 lines, 15 tests) is kept** on the keep
  rule as written, not on affection. "A golden no unit test can hold" is exactly what it
  is: a nested part, chapter numbering with no spurious zero, agreement between the
  rendered number / the TOC row / the resolved ref, an appendix that is unnumbered, a
  below-`toc:`-gate chapter whose sections still reach the drawer, and a **search index
  spanning a whole book** with each record carrying its chapter number and heading path.
  None of that is expressible in `render/tests.rs`, which renders one document. Deleting it
  leaves `docs/guide` and `docs/internals` as the only book fixtures, which is the dissent's
  point (the manual becomes the only test of the feature the manual documents), and this
  wave **just shrank the Internals book by half**, which is the erosion the dissent
  predicted, happening in the same commit. `corpus.rs` has no floor asserting any book
  exists. The dissent's own compromise (trim tarn to four chapters, cut `tarn.rs` to ~150
  lines) was rejected as the worst option available: it costs the rewrite and still destroys
  the nested part and the spans-the-whole-book search index, which are precisely what the
  extra chapters buy.

**THE INTERNALS BOOK GOES TO SEVEN CHAPTERS, NOT FIVE, AND THE PLAYBOOK'S REASON FOR
`extending.tmd` IS FALSE ON THIS TREE.** The plan's rm list names eleven pages, nine of
which existed. Seven went (`repository`, `sites`, `data-types`, `protocol`, `client`,
`validation`, `offline-theming`). Two were kept:

- **`extending.tmd` is load-bearing for three gates.** `lsp.rs`'s
  `the_internals_capability_table_names_every_capability_the_server_advertises` and
  `the_internals_book_documents_every_taliesin_namespaced_method` read it **by name** and
  assert every advertised `*Provider` and every `taliesin/…` method has a row in it; wave 10
  deliberately re-pointed the first at a floor of 7 rather than weakening it.
  `asset_bundle.rs` locates a fixture by its `TALI-PIN-HOOK` marker. Deleting the page means
  moving two tables and re-pointing three gates, for a page whose subject survives. And the
  playbook's stated ground, *"one of whose chapters documents an extension mechanism
  CLAUDE.md states does not exist"*, **does not describe this page.** What CLAUDE.md says
  does not exist is the **format**-extension mechanism (`_extensions/` is a theme-CSS lookup);
  `extending.tmd`'s "two extension points" are declarative shortcodes and
  `window.taliEnhancers`, both of which exist and both of which ship.
- **`server.tmd` (142 lines) is kept because CLAUDE.md already says the book covers it.**
  Its own description of `docs/internals` is "the architecture, the rendering pipeline, the
  block model, the execution model, **the dev server, and how to extend it**": six subjects
  plus the index, which is exactly the keep set now. Cutting to five would have made
  CLAUDE.md wrong in the same commit that cut for anti-drift reasons.

**Seven things that were not true, or that the playbook did not know.** Same genus as
waves 1 to 11:

1. **FOUR MORE SPENT JUSTIFICATIONS, on top of the four the handoff already named.**
   `crates/core/tests/include_root_parity.rs` **already contains** the synthetic temp-dir
   PT-2 pin STEP 1 asks to add (`a_loose_document_is_confined_to_its_own_directory_despite_
   an_ancestor_checkout`). `section_extents.rs` reads `corpus/layout/**structure**.tmd`, not
   `dense-output.tmd`, so STEP 5's "verify `section_extents.rs` still passes; adjust its
   expectations if the anchor moves" had no subject. The "Must survive" list names
   `corpus/transclude.tmd + corpus/_includes/shared-derivation.tmd` (neither exists) and
   `corpus/scaffold*/` "guarded by a hard panic at `crates/server/src/cli.rs:1218`", and wave 8
   took the templates and the goldens both. **Eight spent justifications in one wave is the
   pattern, not the exception: check the file exists before honouring a "must survive".**
2. **`three_scene_theme.rs`'s VACUITY FLOOR WOULD HAVE HARD-FAILED, and nothing in the plan
   mentions it.** `helper_copies()` asserts `copies.len() >= 4`; deleting the duplicated
   posts took `corpus/_includes/three-scene.tmd` and
   `corpus/posts/pca-geometry/_includes/three-scene.tmd`, leaving two. Lowered to **2**,
   which is the honest floor: one copy per variant is the minimum
   `same_variant_three_scene_copies_stay_byte_identical` needs to stay non-vacuous.
3. **THE `checked >= 120` PATH-CLAIM FLOOR SURVIVED WITH TWO OF HEADROOM, which is a trap
   for the next wave.** `shipped_docs_do_not_name_a_file_that_does_not_exist` examined 122
   backticked path claims after the Internals cut, because the seven deleted chapters were the
   densest path-claim prose in the tree. It is an anti-vacuity guard (a broken extractor
   yields ~0, not 60), so it is lowered to **60** with the reason written on it, alongside
   the `out.len() > 40` → **25** the plan already asked for. A floor two below the live count
   fails the next docs edit for the wrong reason.
4. **A NEEDLE TEST WHOSE SUBJECT FILE IS DELETED IS NOT RE-POINTED, IT IS SUBSUMED.**
   `internals_do_not_describe_the_deleted_shim` read `docs/internals/sites.tmd` by name to
   assert it no longer mentioned `site/config/quarto.rs`. The page is gone, and the test is
   deleted rather than re-aimed, because the derived gate three functions below already
   catches it: `site/config/quarto.rs` is a backticked `.rs` path that resolves to no file,
   so **any** shipped doc naming it fails. A comment where the test was says so, because
   "deleted, and here is what covers it" is the thing a later reader needs.
5. **`tools/ui-audit/` WOULD HAVE SILENTLY LOST ITS ANNOTATIONS, AND NO GATE READS IT.**
   `make-sweep-index.mjs` parses `corpus/README.md`'s table for what each unit is FOR, with
   `if (cells.length < 5) continue;`, a four-column shape. The rewritten README is two
   columns, so **every row would have been skipped** and the sweep index would have rendered
   with no notes at all, looking exactly like a corpus with nothing worth saying about it.
   Parser and renderer both adapted (22 notes parse today, verified by running it). Its
   `DRAFT_ONLY` list also still named `corpus/course/problems.tmd`. This is the fifth wave
   in a row where the ui-audit sweep found something; it is not clean by default.
6. **`tools/live-edit-bench` IS A WORKSPACE MEMBER and its regression test reads a deleted
   corpus doc.** `src/main.rs`, `tests/regression.rs`, `RESULTS.json` and `RESULTS.md` all
   named `corpus/posts/em-algorithm/index.tmd`. Re-pointed at the surviving tech-blog copy,
   which is byte-identical (that is what the deleted `twinned_corpus_sources_stay_byte_
   identical` guaranteed, so the measurement is unchanged by construction).
   `editor/vscode`'s e2e suite and README named two of the deleted copies too.
7. **ONE SHIPPED DOC NAMED A FILE INSIDE THE CUT AND THE DERIVED GATE CAUGHT IT, WHICH IS
   THE GATE EARNING ITS LINES.** `docs/guide/using/theming.tmd` cited
   `corpus/course/likelihood.svg` as the example of a figure drawn in a both-themes palette.
   Re-pointed at `corpus/demo-book/structure.svg`, chosen by measurement rather than by name
   (`#888` strokes, zero `prefers-color-scheme` blocks: the only surviving SVGs with that
   shape are demo-book's two and single-page-report's three).

**What was given up, stated plainly.**

**The prose record of the protocol, the data types, the client and the validators.** Seven
Internals chapters, 1,998 lines, and the dissent's objection stands: the author will
re-derive some of this from code in six months. What is *not* given up is accuracy: every
deleted chapter's pointer in a surviving chapter was replaced by the file that states the
same thing precisely (`render/model.rs` for the field shapes, `protocol.rs` for the wire
messages, `site/xref.rs` for cross-page refs, `web-client/client.js` for applying an op),
and `index.tmd` now says out loud that the trade was one description that cannot go stale
over two that can disagree. The Internals book lints clean (`build docs/internals
--check-only`: *no problems found*), so no chapter links into the hole.

**The course exhibit on the marketing site.** The gallery drops from four projects to
three. `corpus/analyst` is still the one that executes, `corpus/descent` the interactive
one and `corpus/tarn` the book, so the "entire projects, not screenshots" claim keeps a
witness of each shape, but a lecturer looking for lecture notes no longer sees themselves
on that page.

**`corpus/layout/dense-output.tmd` is 634 lines → 108.** 400 stream lines became 40 and 200
table rows became 25. All three shapes survive (the scroll-bounded `<pre>`, the bounded
table, and the scaled-to-fit image), and so does the one that is load-bearing beyond the
CSS: the raw-HTML root that **opens in one block and closes in a later one**, which
`render/emit.rs`'s `is_single_root` names this file as its witness for. The document now
says that in its own prose, so the next person to shorten it knows what not to flatten.

**Two more anti-vacuity floors are lower**, and both are recorded on the assertion itself
with the count they were measured against. Neither is a content floor; both exist against a
`read_dir` that silently returns nothing.

### Wave 13, 2026-08-09, `cut/wave-13-run` — the last wave

**Measured reclaim: −4,081 lines** (`+276 / −4,357` over 35 files, **8 deleted outright**)
against the ruling's ~2,406 estimate and against the handoff's ~1,597 "run-only tail". Both
were low for one reason, given below. By area: `crates/server/src` −3,311,
`editor/vscode` −486, `crates/server/tests` −273, `docs/guide` −48, `web-client` −4,
`crates/server/Cargo.toml` −3, `docs/internals` 0, `crates/core/tests` **+30** (a rewritten
extractor, deliberately) and root +14. `notes/` is excluded, as in every figure above.

**`./tools/gates.sh` is GREEN on the committed tree:** **8/8** gates, **2/2** canaries
(`kernel_executes_state_errors_and_interrupts_runaway_cell` and
`only_a_textual_sink_becomes_a_live_region` both printed `... ok`), **81 suites / 1,377
passed / 0 failed / 0 ignored**, exit 0. A bare `cargo test --workspace` gives the same
figure. The one suite gone from wave 12's 82 is `run_session_discovery`; the 62-test drop
from 1,439 is that suite plus the run-only blocks in `exec.rs`, `lsp_lens.rs`,
`protocol.rs`, `lsp_stdio.rs` and the companion's own tests.

**Measured, not asserted.** `cargo build --release -p taliesin-server`, same toolchain:
**29,885,200 → 29,294,216 bytes, −590,984 (−1.98%)** — the largest binary win of the
campaign, ahead of wave 11's −491,072.

## THE ADJUDICATION, WHICH WAS THIS WAVE'S ACTUAL DELIVERABLE

The ruling left `taliesin run` unadjudicated: six files that "fell through the bundle
partition", surviving in `COMMANDS` "by omission, not by decision", with zero corpus
documents. **Ruled: cut, whole.** The reasoning, on the record the way wave 12 recorded the
tarn/course call, because a later reader will want to know it was a decision:

1. **It serves none of the three load-bearing goals.** CLAUDE.md's opening sentence names
   them: click-to-source, block-level incremental updates, no per-edit startup cost.
   `taliesin run` is a *second front end* onto an execution engine whose first front end
   already delivers all three. The workflow its module header staked its claim on ("edit a
   cell, read the result, and by the time you want the HTML there is nothing left to
   compute, which Quarto structurally cannot offer") is delivered in full by `preview`: the
   same warm kernel, the same `_freeze/` writer, the same block-level re-run of the edited
   cell and everything downstream. What `run` added is that the output lands in a terminal
   instead of a browser. That is a *view* preference, and the doctrine is explicit that the
   browser is the view.
2. **Verified rather than asserted, because the whole retirement note rests on it.** A
   scratch two-cell project, `preview` on a real kernel, one page request; the console
   printed `exec index.tmd cell 1/2`, `cell 2/2` and `_freeze/index.json` appeared. Then
   `build` on the same project: `exec index.tmd restored 2 cached cells · 0 re-ran`, and
   both outputs are in the HTML. (First attempt pointed `TALIESIN_PYTHON` at a nonexistent
   interpreter to prove no kernel booted; that busts the key **by design** — the interpreter
   id seeds the cumulative hash — so the honest test is the same interpreter and the
   `0 re-ran` line.)
3. **The standing directive**, and the campaign's own precedent. Wave 3 cut the algorithm
   stepper, which this log calls "the one cut in the whole audit that deletes a
   differentiator rather than shrinking one". `run` is a smaller call than that one.
4. **A weak but real usage signal, stated as weak.** There is no `_freeze/figs/` directory
   anywhere in the tree, though ten `_freeze/` directories exist from builds and previews.
   Decoding a figure to a file and printing its path is one of `run_print.rs`'s two headline
   behaviours, and it has produced zero artefacts in this repository. The directory is
   disposable, so this is suggestive, not conclusive.
5. **What it cost:** a hint-file discovery protocol proven by an identity handshake against
   `/proc/<pid>/exe`, a hand-rolled HTTP/1.1 client with three body-framing modes, a
   detached session spawn with a 45 s readiness timeout, an NDJSON event stream, a per-page
   cancel-epoch registry, and a terminal renderer of the browser's own protocol. A
   distributed system inside a single-purpose dev server.

**Verbs: 8 names in `COMMANDS` → 7** (`preview`, `build`, `init`, `new`, `doctor`, `lsp`,
`help`). Note for the record that this campaign's bookkeeping has said "9 verbs" since wave
8 while `COMMANDS` has held 8 entries; the ruling's list of nine counted `lint`, which
shipped as `build --check-only`, a flag. The count that is checkable is `COMMANDS`, and it
is 7.

**One `RETIRED_COMMANDS` entry, and nothing else**, per the wave 1 contract. Proven by
running the binary rather than by grepping:

```
taliesin run x.tmd  →  `run` was removed: `preview <file.tmd>` executes the same cells
                       against the same warm kernel and writes the same `_freeze/`, so a
                       later `build` still replays without one            (exit 1)
```

## SIX THINGS THAT WERE NOT TRUE, OR THAT THE PLAN DID NOT KNOW

Same genus as waves 1 to 12.

1. **THE RULING'S "SURVIVES REGARDLESS" LIST IS WRONG, AND IT IS WHY THIS WAVE IS 2.5x ITS
   ESTIMATE.** §6.1 says *"`runspec.rs` and `run_control.rs` are **not** run-only, the
   preview server's Run buttons use them, so they survive regardless."* **The preview has no
   Run buttons.** `web-client/client.js` sends exactly three client-to-server messages:
   `restart_kernel` (two call sites) and `click_block`. The Run buttons were always a *code
   lens* in the editor, which invoked the CLI; the browser never had one. So:
   - `runspec.rs` (328) is entirely run-only. `RunReq` and `event_stream` are the POST's
     request and response bodies; `RunScope`/`resolve`/`Resolved` exist to cap a run, and
     every surviving `RunRequest` was `RunRequest::none()` → `RunScope::All` → never caps.
     `lsp_lens.rs` names `runspec::resolve` in a **comment**, not a call.
   - `run_control.rs` (238) is entirely run-only. `cancel()` had two callers: the interrupt
     handler and one test. The `epoch`/`begin_cell`/`end_cell` wiring in `exec.rs` existed
     to serve `cancel`.
   - `session.rs` (242) is entirely run-only. `hinted_port` and `project_root_for` were
     `run_cmd.rs`'s; `write_hint`/`clear_hint` wrote a file only `run` read. The
     single-instance probe uses `identify` + `is_sibling_preview` **directly**
     (`serve/mod.rs`), never the hint.
   This is the same failure class as wave 12's `extending.tmd` finding — *a plan's stated
   reason can be false even when the file exists* — and it is now the campaign's second
   recurrence. **Read the code, not the sentence about it.**
2. **THE CODE LENS HAS TWO JOBS AND ONLY ONE OF THEM DIED, so `lsp_lens.rs` is kept and
   trimmed rather than cut.** The handoff flagged the real risk: cutting the verb makes
   ▶ Run Cell name a command nothing can run, and wave 10 kept this provider on recorded
   ground (`runcell.ts` records that a TypeScript `CodeLensProvider` existed here and was
   deleted for the Rust one, so cutting the server lens regrows demonstrated pressure for
   the duplication CLAUDE.md forbids). The answer is that the two buttons go and the
   **`⚡ cached` / `↻ always re-runs` label stays**: it is the 2026-07-18 DX audit's
   still-open "make caching legible" item, computed from `exec::cell_cache_keys` (the
   executor's own key function, so it cannot claim a hit the executor would miss), and it
   answers a question nothing else in the editor answers — *will editing this cell cost a
   re-run?* It survives the verb because the document still runs, via `preview` and `build`.
   A lens with an empty command name is what tells a client there is nothing to click.
   `codeLensProvider` therefore stays advertised and **wave 10's seven providers are still
   seven**, so `the_initialize_handshake_advertises_…` needed no edit.
3. **A GATE WAS READING A PARITY-DEPENDENT SLICE OF EVERY DOCUMENT, AND EDITING PROSE
   ELSEWHERE MOVED THE SLICE.** `stale_docs.rs`'s `backticked()` paired backticks across a
   whole file with no notion of fenced blocks, so which spans it produced downstream of a
   fence depended on the document's *running backtick parity*. Rewriting one section of the
   CLI reference took it from **55 flag mentions to 69** and made it report
   `rsync -a --delete` — a third-party flag inside a ```sh example — as "a flag the CLI does
   not accept". Its own `checked >= 50` floor passed on both sides, so nothing announced it,
   and the gate had been reading roughly four fifths of the file by luck for months.
   `backticked()` is now fence-aware, which is also the right rule on the merits: an inline
   span is a *claim* about this tool, a fenced block is an example that may legitimately
   invoke `rsync` or name a path the reader is about to create. Measured: the path-claim
   population across the walked docs is **unchanged at 112**, and both halves were proven
   able to fail (a sentinel `--nonesuch` and a sentinel missing path each fail their own
   assertion). Same genus as wave 11's `:root`-inside-a-comment and wave 9's derived
   classification: **when a gate locates its subject by a fragile derivation, the derivation
   is where bugs live silently.**
4. **THE FLAG-COUNT FLOOR WAS AT 50 WITH ONE OF HEADROOM.** With the extractor made
   deterministic, the reference carried 55 flag mentions; deleting the `run` row took it to
   **49** and the floor hard-failed. It is an anti-vacuity guard, not a content floor (a
   broken extractor yields ~0), so it is lowered to **25** with the count it was measured
   against written on it — the same treatment wave 12 gave its three. This is the second
   wave running to trip a floor whose headroom was smaller than the cut.
5. **THERE IS A FIFTH SUBCOMMAND REGISTRATION SITE AND NOTHING GATES IT.** CLAUDE.md says a
   new verb has four drift-gated sites in `main.rs`. It also needs a row in
   `docs/guide/reference/cli.tmd`'s verb table, and no test ties that table to `COMMANDS`.
   The retired `run` row survived several passes of this wave; what eventually caught it was
   the flag gate noticing the *flags inside the row*, so **a verb with no flags would have
   left a documented command the binary does not answer, with every gate green**. Recorded
   in CLAUDE.md beside the two `--help`-prose holes it belongs with. Not fixed: a fifth gate
   is machinery this campaign removes, and the honest instruction is to grep.
6. **THE COMPILER AND ONE DEPENDENCY COMMENT FOUND FOUR MORE ORPHANS.** `Kernel::pid()` was
   published solely so an interrupt arriving *outside* the run loop could signal the process
   — the silence/wall-clock cap calls `interrupt_pid` directly from inside the polling loop,
   so the canary is untouched and `interrupt_pid`'s doc no longer claims two callers.
   `protocol::CellSite` (the `file`/`line` on every `cell-state`) was `run_print.rs`'s alone;
   its own doc said so (*"`taliesin run` is the reason it exists"*) and no client reads those
   fields. The `skipped` cell state existed **only** for a capped run and is gone from the
   protocol, the client typedef and the badge. And `sha2` was in the server crate for
   `session.rs`'s digest alone; removing it also removed the comment two lines below it,
   which described `qrcode`'s feature flags and had outlived that dependency since wave 11
   by sitting above an unrelated one.

## THE MOAT, CHECKED BY NAME

`git diff main` over the path list: **`crates/core/assets/` and `serve_site/exec_pool.rs`
are byte-identical**, and so are `freeze.rs` and `diff.rs`. The one standing freeze
(`MAX_WARM_PAGES` + the deterministic LRU) is absent from this wave's diff entirely. `plan()`
in `exec.rs` lost **only** the cap clamp: the longest-common-prefix walk, `first_uncacheable`
and the `cache: false` extension are unchanged, which is the whole of the cache planner.
`web-client/client.js` is the one shipped-asset change, six lines, and `minify_js` went in
wave 4 so those bytes ship verbatim.

## WHAT WAS GIVEN UP, STATED PLAINLY

**The browser-free terminal loop.** Someone iterating on a headless box over SSH now needs a
browser, or a forwarded port, to see what a cell produced. That is the honest cost and it is
the one an author on a remote GPU box would feel; the answer if it ever bites is a terminal
client against the preview's existing websocket, which is a smaller thing than the session
protocol just deleted.

**Interrupting a run in flight, which is a real loss and larger than it looks.** Ctrl-C on a
`taliesin run` stopped the *run* — the signal ended the executing cell and an epoch flag
stopped the queued ones — while the warm kernel and every earlier cell's variables survived.
`taliesin run <file> --interrupt` did the same thing from a second terminal, and because
`build_page` handed every rebuild the same `RunControl`, it could stop a **preview's**
rebuild too. Nothing replaces that: a runaway cell in a preview now waits out
`TALIESIN_CELL_SILENCE` (600 s by default, and the budget resets on every line it prints) or
is ended by "Restart kernel", which drops all state. The two-part design that went with it
was hard-won and is recorded in `run_control.rs`'s header for whoever needs it back: a
boolean is the obvious flag and it is wrong, because runs *queue*, so a cancel has to
invalidate a run that has been asked for but has not started.

**The companion's Run Cell / Run All commands and their task plumbing**: the progress
indicator, and the completion notification for a long run that CHI 2020 asked for by name.
With them goes the e2e test that pinned VS Code's `onDidEndTaskProcess` round trip — the
measured platform fact that the *first* task executed in a window reports
`exitCode: undefined` whatever it is. The lens e2e tests are re-pointed at the surviving
label rather than deleted, and the fixture moved to `test-fixtures/codelens.tmd` with
`#| cache: false` on each cell, because a fresh cacheable cell is deliberately silent and
counting lenses on the old fixture would have passed while proving nothing.

**One suite and one wire test's subject.** `crates/server/tests/run_session_discovery.rs`
(271 lines) pinned both halves of the hint-file discovery bug and is gone with its subject.
`lsp_stdio.rs`'s `code_lens_offers_the_run_command_over_the_wire` is re-pointed, not deleted:
it now pins that a lens arrives over real stdio carrying a label and **no** command name.

### Wave R6 (editor surface) — 2026-08-09, `cut/r6-editor-surface`

The remediation plan's R6-2, R6-3 and R6-9 in one wave, because all three move the same LSP
handshake test and taking them separately would have moved it three times.

**Measured reclaim: −3,180 lines** (`+346 / −3,526` over 35 files, **12 deleted**) against the
plan's 520 + 1,054 + 560 = **2,134 estimate**. `package-lock.json` is −861 of it (two e2e-only
dev dependencies); the rest is a cascade the plan did not name, which `clippy -D warnings`
enumerated, exactly as wave R6-11 recorded the method. **Excluding `notes/` — which grew 169
lines on purpose, for this entry and the tier-2 rulings — and the lockfile, the code reclaim is
−2,489** (`+129 / −2,618`). By area: `editor/vscode` −2,105 (−1,244 without the lockfile),
`crates/server` −855, `crates/core` −380, `docs` −14, `CLAUDE.md`/CI +4.

**Gate: 10/10, both canaries `ok`, 80 suites / 1,347 passed / 0 failed / 0 ignored.** Measure
the next wave against **1,347 and 80 suites**, not R6-11's figures. Companion `npm test`: 142.

**Six providers, two namespaced methods.** `codeLensProvider` is now in
`the_initialize_handshake_advertises_…`'s `gone` list beside the nine from wave 10, and the
floor drops 7 → 6 with the count it was measured against written on it. `taliesin/mathCommands`
went, so the namespaced-method floor drops 3 → 2 the same way.

**Four things that were not true, or that the plan did not know.**

1. **The one recorded ground for keeping the code lens was already spent, and no one had
   checked.** Both the playbook (`:942`) and wave 10's log kept `lsp_lens.rs` on the argument
   that `editor/vscode/src/runcell.ts:9-14` records a TypeScript `CodeLensProvider` having
   existed and been deleted for the Rust one, so cutting the server lens would regrow pressure
   for the TS duplication CLAUDE.md forbids. **`runcell.ts` was deleted by wave 13 and
   `decorations.ts` by wave 9.** The file carrying the record is gone, so the pressure it
   evidenced is gone with it. This is the campaign's most-recurring rule (*"before honouring a
   'must survive', check the file exists"*) landing on a justification the campaign itself
   wrote, twice.
2. **`lsp_memo` had exactly one consumer, and it was the code lens.** 85 lines, a module
   CLAUDE.md described as part of the `didChange` coalescing story ("keyed on `(uri, text)`,
   which is why it needs no invalidation logic"). `RenderMemo` was constructed in `run` and
   threaded through `handle_request` to one arm. Nothing else in the crate touched it; the
   `memo` parameter went dead the moment the lens arm did, and clippy said so on the first
   run. The prose is corrected rather than deleted, because the *reason* the memo existed is
   still worth knowing.
3. **`exec::cell_cache_keys` + `CellCacheKey` (52 lines) were the lens's alone.** Extracted so
   "the lens cannot claim a hit the executor would miss on"; the executor computes its keys on
   its own path. `cells_of` survives and is still the one walk, and its doc comment now says so
   for a reason that does not name a second caller that no longer exists. Same for
   `version_line`'s.
4. **The math hover took `Target::Math` down with it, and `MathSpan` shrank to two fields.**
   `lsp_nav::scan_math` survives because completion asks "am I inside math?" — but
   `enclosing_math` ("*which* expression am I inside?"), the `Target::Math` variant, and
   `line_char_to_offset`/`offset_to_line_char` were hover's alone. `MathSpan` kept `start` and
   `closed`; `latex`, `display` and `end` went. **No coverage hole**, and that was checked
   rather than assumed: the four `classify_target` math tests deleted here pinned fence-
   awareness and the line-break rule, and `lsp_complete`'s own
   `a_backslash_in_a_code_cell_offers_nothing` and
   `display_math_survives_a_line_break_but_inline_math_does_not` pin both against `scan_math`
   directly.

**What was given up, stated plainly.**

**The cache-status lens was the only place `_freeze/` was legible outside the browser**, and
the 2026-07-18 DX audit's "make caching legible" item is open again. What it actually shipped
was one ⚡ label, silent on every ordinary cell, carried by a `Command` with an empty name —
a client-side rendering convention, not a protocol contract. `DO-NOT-REBUILD.md`'s item 217 now
says cut-whole and points the reopened item at the preview rather than the editor.

**The Insert Math Symbol picker was the one shape completion cannot serve.** Completion still
offers every KaTeX command inside `$…$` on `\`, verified by `math_vocab.rs`'s
`every_command_renders`, so what is lost is finding a command *by its rendered glyph* — you
have to know that ⊗ is `\otimes` to type a prefix of it. That was the recorded argument for
`taliesin/mathCommands` in wave 10 and it is still true; it lost to the standing directive.

**The Extension Host suite (1,054 lines, 46 tests) was the only thing proving VS Code asked
the server and rendered the reply**, as opposed to the server answering. A unit test cannot
see that gap, and `notes/2026-08-02` had already recorded the suite as run by nothing. It is
gone with `test-fixtures/`, `mocha` and `@types/mocha`. `glob` stays — `grammar.test.ts`
uses it.

**And a fifth thing that was not true, found by the gate rather than by reading.**
`@vscode/test-electron` is **not** e2e-only, and deleting it turned the companion gate red.
`scripts/ensure-vscode.cjs` calls its `downloadAndUnzipVSCode` to populate
`editor/vscode/.vscode-test/` with a VS Code build, because the **surviving** offline grammar
gate (`grammar.test.ts`) loads the bundled MIT markdown/python/yaml base grammars out of a real
install. The dependency is restored and the script's header now says it is the only consumer,
so the next cut does not walk into this again. The local `npm test` passed either way — the
download was already on disk — and only `gates.sh`'s `npm ci` could see it. **Which is the
whole argument for that script**, landing on this wave. The 3.1 GB in `.vscode-test/` is a test
fixture, not residue; do not delete it.

**The three open tier-2 questions were ruled in this session** and are recorded in full in
`notes/2026-08-09-remediation-plan.md`'s tier-2 table. In short: the **vendored PowerShell
grammar is CUT** (one wave, ~1,650 lines, ordering rule applies — its only witness is
`corpus/highlight.tmd`, and it was added on a persona-sourced demand probe); **Atom feeds are
KEPT** (the one witness mirrors a blog the author publishes, and *"when close, cut"* adjudicates
features nobody uses); **`notes/`'s 64 dated audits are BANNERED, not deleted** (one STATUS
line each: dated record, check the file exists before acting). With mermaid already ruled keep,
**tier 2 is closed** — two waves of work remain in it, no decisions do.

### Wave R6 (tier 2, PowerShell) — 2026-08-09, `cut/r6-t2-powershell`

The first of tier 2's two waves of work: the vendored PowerShell grammar, ruled CUT the session
before. **Measured reclaim in code: −1,738 lines** (`+14 / −1,752` over 7 files, **2 deleted**),
against the ~1,650 estimate. `notes/` grew on purpose for this entry and the plan's tier-2 row;
the commit body carries the whole-diff total. **5.4% over, where this campaign's waves have run
49% to 150% over**
(R6-2/3/9 was 3,180 against 2,134; R6-11 was 959 against ~500; wave 13 was 2.5×), because the
subject was a genuine leaf — see the zero-cascade note below. Only R6-1 was closer, and that row
had been measured in advance rather than estimated.

**Gate: 10/10, both canaries `ok`, 80 suites / 1,342 passed / 0 failed / 0 ignored.** Measure the
next wave against **1,342 and 80 suites**. The drop of exactly 5 from R6's 1,347 is the five
tests this wave deletes and nothing else, which is the cheapest possible check that no test went
vacuous instead of going away: two unit tests in `highlight.rs`
(`powershell_highlights_under_both_of_its_tokens`,
`the_vendored_set_only_fills_holes_the_others_leave`), two in `highlight_langs.rs`, and
`third_party.rs`'s `vendored_syntaxes_are_attributed_and_carry_their_licence`. No suite
disappeared, so the count is 80 either side.

**THE BINARY SHRANK BY 9.4× THE ASSET'S OWN WEIGHT, AND THAT IS THE FINDING.** Measured both
ways with a release build on each tree (the baseline built from a stashed tree at `32cd69ff`, not
from a stale artefact): **29,222,672 B → 28,743,568 B, −479,104 B**, for an `include_str!` asset
of **50,804 B**. The other ~428 KB is **syntect's `.sublime-syntax` *source* parser**:
`SyntaxDefinition::load_from_str` plus `SyntaxSetBuilder`, whose only reachable caller in this
workspace was `vendored()`. Every other syntax set in the tool loads a **precompiled dump**
(`SyntaxSet::load_defaults_newlines`, `two_face::syntax::extra_newlines`), so with the one
grammar gone the whole source-parsing half of the dependency became unreachable and the linker
dropped it. **Verified rather than inferred from the byte delta:** `load_from_str`,
`SyntaxDefinition`, `SyntaxSetBuilder`, `into_builder` and `load_from_folder` now match **nothing**
under `crates/` or `tools/`, and `highlight.rs` is the only syntect consumer left in the tree.
**No `Cargo.toml` or feature change was needed or made** — this fell out of deleting the caller.
Worth knowing for any later wave that removes the last caller of a *format reader* rather than of
data: the reclaim is the reader, not the file.

One follow-up deliberately declined: syntect is still pulled in as `default-features = false,
features = ["default-fancy"]`, which carries the now-unused YAML loader. Trimming it would buy
**zero** binary bytes — the linker has already dropped the code, which is what the −479 KB *is* —
so it would only shrink the build graph. That is dependency micro-surgery with a real risk of
changing what `load_defaults_newlines` returns, and the standing directive is about features
nobody uses, not about feature flags.

**Zero cascade.** `clippy -D warnings` was clean on the first run, which is the opposite of
R6-11 (23 dead items) and R6-2/3/9 (`lsp_memo` entire). R6-11's method was still the right thing
to run; a leaf simply has nothing behind it.

**The degradation was verified, not assumed**, because the ruling rested on a claim about
silence. A scratch document with both fences, built by the release binary: **0 `tali-hl-` spans
in either fence**, the text HTML-escaped and intact, and `build --check-only --no-exec` prints
*"no problems found"* and exits 0 — **`--strict` too**. So the ruling's ground holds: wave 9's
removal of the generic unknown-fence-language lint means there is no diagnostic to go stale, and
a `powershell` fence is now exactly as quiet as any other language the tool has no grammar for.

**`third_party.rs` lost a gate and gained a needle, and the gate's own floor is why the loss was
loud.** `vendored_syntaxes_are_attributed_and_carry_their_licence` is deleted with its subject
directory; its anti-vacuity assertion (`grammars > 0`) meant it would have **hard-failed** rather
than passed over an empty directory, which is the ordering rule doing its job on a licence gate
rather than on a corpus pin. In its place, `PowerShell` joins `removed_deps_are_not_listed`'s
list — the wave-4 `paged.js` precedent — and it is the one entry there that was never a
*dependency*: it was redistributed **source**, so a stale row would be a licence claim about
bytes this binary no longer carries.

**What was given up, stated plainly.** A `powershell`/`ps1` fence renders as plain escaped text
instead of coloured, and **the third-tier mechanism itself is gone**, not just its one occupant:
`resolve` is two sets now, so the next language neither syntect nor `bat` carries costs a
re-vendor *and* a re-derivation of the format constraint, which is the expensive part and is
therefore recorded **here** rather than in the code: **syntect loads `.sublime-syntax` only, and
cannot consume a `.tmLanguage` plist as a syntax at any feature level** (`plist-load` covers
themes and metadata), which is why Microsoft's own `PowerShell/EditorSyntax` was unusable and a
Sublime-format grammar had to be found instead. `resolve`'s surviving comment says only that
there is no third tier and when it went — deliberately, because a comment in `resolve` explaining
how to vendor a grammar would describe something the code no longer does, which is the exact
doc-drift genus the post-cut audit found seventeen of. No register entry is owed and none was
written: a fence language is not a
Taliesin vocabulary, so there is no retired name for the tool to answer.

**Two adjacent findings, surfaced and deliberately NOT fixed here** (both are R4's genus, and
folding a second prose subject into this diff is what R4 exists to prevent):

1. **`highlight::known_language` is dead workspace-wide, and R4 did not catch it.** Zero callers
   outside its own file and its own two unit tests, verified by grepping every `.rs`/`.ts`/`.js`/
   `.tmd`/`.md` in the tree. It is `pub` in `taliesin-core`, so **clippy cannot see it** — the
   same blind spot that hid `RenderedDoc::body_text` from R6-11. Its whole purpose was to feed
   the generic unknown-fence-language lint **wave 9 deleted**, and `INTENTIONALLY_PLAIN` exists
   only to serve it. ~30 lines with its two tests. **An R6-12 candidate**, and note it is a
   `pub` API removal, not an internal one.
2. **The stale bundled-JS list has THREE copies and R4 fixed one.** `scrolly.js`, `tabset.js`
   and `walkthrough.js` went in wave 7. R4 corrected `CLAUDE.md`'s fenced map (0 hits now) and
   left **`THIRD_PARTY.md:50`** (one line naming all three as "Taliesin's own") and
   **`third_party.rs`'s `OWN_JS`** (3 dead exemptions in the list `vendored_js_is_attributed`
   skips). Both files are ones this wave edits, which is how they were found. Harmless in
   behaviour — the loop skips names with no file — but the first is a factual error in the
   licence document, which is the one document where being wrong costs something.
