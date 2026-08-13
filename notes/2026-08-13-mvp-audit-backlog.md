# MVP audit backlog, 2026-08-13

Durable state for the post-cut defect sweep. **This file is the handoff.** A fresh session
needs only this file: every item carries its own anchor, its own reproduction command and
its own done-condition, so none of them requires the conversation that produced it.

Batch 10 (S1-S9) and S13 landed 2026-08-13. Produced by a 48-agent audit at `aceb566b` (5 scope lenses + 8 subsystem bug finders, one
adversarial refuter per finding). 34 candidate defects, **2 refuted** (recorded in
[DO-NOT-REBUILD.md](DO-NOT-REBUILD.md), do not re-file), 32 confirmed, **29 distinct**
after removing three cross-area duplicates. Plus 36 scope findings, of which 16 survived
synthesis as real work.

## The verdict this came with

**The scope is right. The shipping surface has not caught up.** The seven-verb CLI covers
the whole advertised journey with no hole, all three load-bearing goals were exercised live
and are intact, and an independent scan found the surviving document vocabulary **fully
witnessed** (every offered name in real use; the residual unused tail is two names, not the
ten on record). There is no wave 14 hiding in the feature list. **Do not cut another
feature.** What is left to cut is the layer that was never gated: the README's phantom
bullets, three unread directories, and `notes/` itself.

The single sentence that carries it: **`README.md:157-165` advertises four features the
tool deleted, and `crates/core/tests/retired_names.rs:294` is named
`the_lightbox_is_gone_from_the_client_bundle`.** The test suite asserts the absence of
features the README advertises, and every gate is green.

## Baseline at `aceb566b`

| | |
|---|---|
| `cargo test --workspace` | **81 suites, 1,352 passed, 0 failed, 0 ignored, exit 0** (measured 2026-08-13, with `TALIESIN_PYTHON="$PWD/.venv/bin/python"`) |
| `./tools/gates.sh` | **12 gates, all pass** (measured 2026-08-13 on `d0b4bcdf`, with `TALIESIN_PYTHON="$PWD/.venv/bin/python"`). The audit had held it back so the gates would not take the cargo lock while agents worked. |
| Working tree | clean, untouched by the audit |

**Every defect below exists in a tree where those 1,352 tests pass.** That is the finding
behind the findings: the cut removed features and their tests together, correctly, but did
not add coverage where the *remaining* surface became load-bearing.

## Rules for working this file

1. **One batch per session, one branch, one commit.** The batches below are drawn so each
   is a coherent unit that touches related code.
2. `./tools/gates.sh` green before and after. It needs
   `TALIESIN_PYTHON="$PWD/.venv/bin/python"` or it exits 2 at preflight and certifies
   nothing. **Take the gate count from the script's own verdict line**, never from prose.
3. **Delete an item from this file when it lands.** Never a `[x]`, never a strikethrough.
   That is this project's standing rule and it is why its notes rot.
4. **Trust an item's symptom, never its cause, line number or cost.** Every anchor here was
   correct on 2026-08-13 and line numbers move. Grep the named symbol before pricing work.
5. A retirement costs **one register entry and nothing else**. Do not write a tombstone
   test for it.
6. A pin and its docs page are deleted in the **same commit** as their feature, never
   before.

### Hazards that apply to several batches

- **Any change that adds or removes a `corpus/**/*.tmd` re-arms the census gate.** Gate 11
  (`tools/portability-census.py --verify`) asserts that `README.md` and
  `docs/guide/using/choosing.tmd` still publish the document count, line count,
  beyond-CommonMark count, percentage, complement and all six per-family pairs. Re-run
  `python3 tools/portability-census.py`, copy the figures into both pages, then `--verify`,
  in the same commit. This is mechanical but not optional.
- **Editing `crates/core/assets/css/*` or `assets/js/*` needs a `cargo build` before the
  change shows up.** They are `include_str!`-compiled, so rebuilding only the site re-emits
  the old bundled CSS/JS and you will measure a stale page.
- **`target/release/taliesin` is shared across sessions.** Check `taliesin help`'s version
  line against your own HEAD before trusting any CLI measurement. At `aceb566b` it reports
  `0.2.0 (0178e403)`; `git diff 0178e403..HEAD -- crates/ web-client/` is empty, so it is
  behaviourally current and only the version string lags.
- **`notes/backlog.md`'s "Standing constraints" section is itself stale** and will mislead a
  session that reads it: it names `taliesin features` (cut wave 2), "four gates" (there are
  eleven), "FIVE drift gates / EIGHT for a retired key" (now four and one), and owes a
  four-projection sweep to `taliesin read`, `skim.rs` and `llms-full.txt`, all three cut.
  Filed as **S18**.

## Confidence key

- **[V]** reproduced by the orchestrator directly, command and output in hand.
- **[A]** reproduced by an agent that quoted its command and output, then survived an
  adversarial refuter briefed to kill it.

Where a refuter narrowed or widened a finding, the corrected statement is what is written
here, not the finder's original.

---

# BATCH 11: finish the subtraction

The cut stopped at the code's edge. These are the author's own untaken waves, re-verified at
`aceb566b`: not a line has moved.

## S10 [V] MAJOR: `tools/ui-audit` + `tools/record-demo` + `samples`, 3,755 tracked lines read by nothing

`grep -n 'ui-audit\|record-demo\|samples' tools/gates.sh .githooks/pre-push tools/build-site.sh .github/workflows/*.yml`
exits 1. Measured at HEAD: `tools/ui-audit` 16 files / 3,026 lines, `tools/record-demo` 11
files / 471 lines, `samples` 4 files / 258 lines.

**Correction to the source wave (W6):** it prices this as "382 MB". **Only 212 KB is
tracked**; the 382 MB is gitignored capture output. Do not publish the 382 MB figure.

**`samples/` is not a free delete:** `crates/core/tests/stale_docs.rs:44` and `:479` both
read it, so removing it is a gate edit in the same commit.

Three other tracked references, all trivial: `crates/core/tests/retired_names.rs:40` (a
comment above a `.work` skip entry), `site/README.md:74`, and one more.
**Interacts with S7**: if the screencasts are re-recorded rather than cut, `record-demo`
stays.

## S11 [A] MAJOR: the W7 dead-code sweep is 100% unspent, about 1,050 lines

Re-verified at HEAD, five subjects:

1. `crates/core/tests/retired_names.rs` is 855 lines / 21 tests, of which **18 tests / 562
   lines** are hand-written UI tombstones with no register behind them, which
   `CLAUDE.md`'s own register rule says not to write. The file's charter is lines 1-293.
2. `crates/core/src/schema.rs`, 146 lines, `SITE_SCHEMA` read only inside its own
   `#[cfg(test)]` module (`grep -rn 'schema::' --include='*.rs' crates | grep -v src/schema.rs`
   exits 1), and `crates/core/assets/schema/tali-site.schema.json` is **byte-identical** to
   `editor/vscode/schema/tali-site.schema.json` (`cmp` exits 0).
3. `$/cancelRequest` batching, which its own doc comment retires.
4. `render/model.rs:372` `after_body` (`doc_includes.rs:22` confirms only `in_header` is
   populated).
5. `lsp.rs:1153` and `:1172` index a `vocab` key that does not exist.

**Related:** `crates/core/src/render/mod.rs:2078` `pub fn base_css()` and `:2084`
`pub fn site_css()` are `pub` in `taliesin-core` with **every caller a test**, most of them
in the tombstones above. They go with them.

## S12 [A] MAJOR: `notes/` is 36,315 lines across 104 files, larger than `crates/core/src`

Nothing gates it (`retired_names.rs:61` `SKIP_PATHS` names `"notes"`; `stale_docs.rs`'s
`ROOTS` excludes it; `gate_script.rs:51` walks only `crates/`), and it is the first thing
`CLAUDE.md` tells a fresh session to read. It grew 7 files / 2,274 lines since 2026-08-10.

Deletable outright, about 1,683 lines:
- `notes/retired/diagnostics-explanations.rs`, 1,222 lines, **byte-identical** to
  `git show pre-cut:crates/core/src/diagnostics/codes.rs`, and carrying **no header saying
  it is dead**. Its module doc opens by describing a verb cut in wave 9. Every drift guard is
  blanket-exempt from `notes/`, so this unmarked copy of the pre-cut vocabulary is invisible
  to all of them (`grep -c` inside it: scrolly 8, panel-tabset 4, theorem 20, publish 11,
  prose-lint 5).
- `notes/ap2-fuzz-harness` + `notes/ap8-determinism-harness`, 461 lines.

Two banner edits, both worse than inert:
- `notes/ROADMAP.md:3-7`'s pause banner is expired and **now reads as permission to grow**.
- `notes/FEATURE-IDEAS.md:3-6` (1,181 lines) still tells its reader an idea "graduates to
  the roadmap only when it earns a corpus pin doc", a rule `CLAUDE.md` explicitly retired as
  circular.

**This file is part of the problem it describes.** Delete it when it is empty.

## S14 [A] MINOR: cut-wave residue in production source comments

`stale_docs.rs` walks only `.md`/`.tmd`, so a source comment can name a cut feature forever
with every gate green. Live examples: `TALIESIN_R` presented as a current fix in
`build.rs:528` and `exec.rs:41,555` (and in two READMEs and
`.claude/agents/corpus-verifier.md:28`); `render/mod.rs:99` says the text projection is
"reached via `RenderedDoc::body_text()`" with **no such function anywhere in the tree**, and
labels the module "Text projection (`taliesin read`)" after that verb was cut.

## S15 [A] DECIDE: the retired vocabulary is now the same size as the live vocabulary

Live closed-set vocabulary across every validator const: about **90 names in about 61 lines**.
Retirement registers: `RETIRED_KEYS` 40 + `RETIRED_DIV_CLASSES` 25 + `RETIRED_COMMANDS` 18 +
`RETIRED_XREF_PREFIXES` 7 + `RETIRED_CELL_LANGS` 3 + `RETIRED_NEW_KINDS` 3 + `RETIRED_FLAGS`
2 = **94 entries in 481 lines**, on top of 2,768 lines of dedicated drift/tombstone test
files.

For a tool with zero published users, every one of those 94 entries answers an author who
wrote a spelling that only ever existed in this repository. **The counter-argument is real
and is why this is DECIDE, not CUT:** the registers are the single strongest piece of
evidence that the surviving surface is designed rather than residual, because every retired
name answers with its successor instead of a did-you-mean. Cutting them before publishing
trades the tool's best first-contact property for lines nobody is paying for. **Recommend
keeping through 1.0 and revisiting when real users exist.**

---

# BATCH 12: publish-path decisions (the author's, not code)

## S16 [A] BLOCKING: there is no publish path for the manual

`gh repo view` says private; `gh release list` is empty; `git tag` holds no `v*`. So every
README link (clone URL, releases page, the `Docs:` URL `taliesin help` prints) 404s.
`.github/workflows/ci.yml` has six jobs and **no deploy step**; `tools/build-site.sh`
composes the deploy but is run only by hand and by `--check`; `site/_site.yml:8` declares
`url: "https://taliesin.sh"`, which **has no DNS record** and which `seo.rs`/`meta.rs` bake
into `sitemap.xml`, `robots.txt` and `og:url`.

`notes/2026-08-10-mvp-publish-session.md:74-88` lists four steps to a tag and does not
mention hosting at all. This is a decision, not lines.

## S17 [A] BLOCKING: the purge set versus "make the repository public"

`.githooks/pre-push:36-41` defines a register of files that must never be published, and its
own comment explains it matches only `--diff-filter=A` "because the purge set is still
tracked today, so a check against the whole tree would refuse every push". **All seven are
tracked at HEAD**, 3,061 lines: `notes/STARTUP-PLAN.md`, `notes/FUNDING-RESEARCH.md`,
`2026-07-18-pmf-audit.md`, `2026-07-27-due-diligence-audit.md`,
`2026-07-28-demand-positioning-audit.md`, `2026-07-28-launch-critique.md`,
`2026-07-27-adoption-friction-audit.md`. Two instruct their own removal in their own text.

Meanwhile `notes/2026-08-10-mvp-publish-session.md:77-80` lists "make the repository public"
as step 2, and **flipping visibility publishes the whole history, not HEAD**.
`notes/STARTUP-PLAN.md:111-127` records a contrary already-made decision (a fresh
no-history repo). **Two live documents disagree on the publish step, and getting it wrong is
irreversible.** Also: `git grep -Il "/home/bogo"` matches 16 files.

## S18 [V] MINOR: `notes/backlog.md`'s standing constraints are stale

It names `taliesin features` ("exists, so do not re-derive an adoption table by grep") which
wave 2 cut; "four gates" and `TALIESIN_REQUIRE_..._R`/`_CHROME` when there are eleven gates
and two runtimes; "FIVE drift gates; a RETIRED one trips EIGHT" when `CLAUDE.md` now says
four and one; and owes "the four-projection sweep" to `taliesin read`, `skim.rs` and
`llms-full.txt`, all three cut. A session that reads it for orientation is misdirected on
every count.

---

# Refuted: do not re-file

Both survived a finder and were killed by a refuter. Recorded in
[DO-NOT-REBUILD.md](DO-NOT-REBUILD.md) as well.

1. **"A block containing a `{{< input >}}` reports an end column past the end of its own
   source line"** (`crates/core/src/render/extension/mod.rs`). Refuted.
2. **"`codeAction` quick fix builds a buffer-rewriting `WorkspaceEdit` from client-supplied
   data with no cross-check"** (`crates/server/src/lsp.rs`). Refuted: the "Change to `X`"
   quick fix returning a `WorkspaceEdit` is the standard LSP contract, is user-invoked rather
   than server-initiated, and is pinned by its own test. It does not breach the
   single-editing-surface rule.

# Affirmations worth not re-litigating

Measured during this audit. Each cost real work and each closes a question a future session
would otherwise reopen.

- **Click-to-source is fully intact**, verified live through an include, a fenced div, an
  executed-cell image, and correctly silent on a gathered reference block.
- **Block-level incremental updates and live-state preservation are intact.** Editing one
  paragraph inside an included file produced `update 1 block` with the page's `{js}` runtime
  state fully preserved (slider 9, mount counter 2, teardown counter 1, `window` sentinel).
- **"No per-edit startup cost" is still literally true.** Wave 11 cut the warm *pool*, not
  the per-page warm kernel; `exec.rs:1084` returns before any boot when the page's kernel is
  alive. The cost lands on a cold or evicted page: measured 1.55 / 1.62 / 1.63 s.
- **The surviving document vocabulary is fully witnessed.** Every offered name in
  `CELL_OPTION_KEYS`, `CALLOUT_KINDS`, `INPUT_TYPES`, `DIV_FEATURE_CLASSES`, `LISTING_KEYS`,
  `HERO_KEYS` has real use in the shipped read set (excluding `docs/guide/reference/` so a
  reference table cannot vouch for itself). **The recorded "10 unused offered names" tail is
  stale by eight**: `vocab.rs:351-355` filters `UNSUPPORTED_KEYS` (removing `csl`) and
  `:316-324` filters `RETIRED_XREF_PREFIXES` (the seven theorem prefixes). The real tail is
  **two**: `_site.yml`'s `head:` and `python:`.
- **`corpus/tarn` is NOT cuttable** despite looking like a synthetic persona fixture.
  `tools/build-site.sh:41-43` composes it into the marketing deploy, so deleting it 404s a
  published page and trips gate 5, and its 16 tests are the only golden for cross-page
  `@sec-` numbering by chapter.
- **The code/surface ratio is defensible.** The alarming file sizes are mostly test modules:
  `lsp.rs` is 1,742 production of 4,138; `lint.rs` 670 of 1,906; `lsp_complete.rs` 987 of
  1,911. Whole-workspace production Rust is 35,386 lines against 42,707 test lines, with
  **zero traits and two one-variant enums**. The outlier is `notes/`, not the compiler.
- **The error paths are the best part of the tool.** Every retired verb names its successor
  rather than guessing; typos get edit-distance suggestions; the missing-kernel path prints a
  five-step numbered resolution order showing which rule won; a directory with no `_site.yml`
  is refused with two concrete next commands.

# Method note

The finder/refuter split earned its cost. Refuters killed 2 findings outright and **corrected
the severity, trigger, root cause or anchor of at least 14 more**: A5's blamed file was wrong,
A6's root cause was the persist loop rather than the plan mask, A17's obvious fix would have
reintroduced a wedge, A28's severity went *up* (draft leak), A22's blast radius widened to the
Atom feed, and A8's trigger widened from included files to any document containing an include.
**Every anchor in this file is the corrected one.** Where a finder and a refuter disagreed on
severity, the refuter's is recorded.
