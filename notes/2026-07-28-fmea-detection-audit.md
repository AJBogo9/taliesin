# R7 — FMEA with a detection axis

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

**Date:** 2026-07-28
**Round:** Wave 2 / R7 of the [audit slate](../docs/superpowers/specs/2026-07-27-audit-slate-design.md).
**Question.** Which failures would nobody find out about?

**Why this round is different from AP11.** AP11 injected failures and watched degradation. It did not
score **detection**. Detection is the novel column, and with no CI, no telemetry by design, and the
corpus as the regression net, Taliesin's detection scores are systematically weak by construction.

**This round consumes [R14's exemption register](2026-07-28-deck-exemption-audit.md), as the slate
requires, and it re-prices Wave 1's three HIGH findings rather than re-finding them.** Its real
product is the **high-detection cluster** at the end: the failures that would ship silently regardless
of severity. New items are numbered from **117**.

---

## The scoring rubric, written before scoring

Required, or the numbers are not comparable across modes.

**Severity (S) — impact when it happens**

| Score | Meaning |
|---|---|
| 9-10 | A user's machine or data is compromised, or a licence/legal claim is wrong |
| 7-8 | Published output is wrong or broken and the author does not know |
| 5-6 | The author's own loop is degraded, or a claim the project makes is false |
| 3-4 | Cosmetic, or recoverable with one obvious action |
| 1-2 | Noise |

**Occurrence (O) — likelihood in normal use**

| Score | Meaning |
|---|---|
| 9-10 | Happens on ordinary use, today, in this repo |
| 7-8 | Happens whenever a common shape is used |
| 5-6 | Needs an uncommon but reasonable shape |
| 3-4 | Needs a deliberate or unusual configuration |
| 1-2 | Needs a hostile actor or a rare coincidence |

**Detection (D) — likelihood of catching it *before a user does*. Higher is worse.**
Scored to the mutation question — *would the test actually fail?* — never to "a test exists."

| Score | Meaning |
|---|---|
| 10 | Nothing anywhere would catch it. Measured zero coverage |
| 8-9 | Covered only by a gate that skips silently, or by a test that asserts an adjacent property |
| 6-7 | Caught only by a human looking at the right surface |
| 4-5 | Caught by a test that would probably fail |
| 1-3 | Caught by a test that certainly fails, and that runs by default |

---

## Detection substrate, measured

Facts every score below rests on. Counted by test **body**, not by filename — the filename count is a
trap this project has been bitten by before, and it was wrong here too (`mounts` shows 0 files by
name and 3 by body, of which 2 are false positives).

- **1,658 `#[test]` functions**, 436 in integration test dirs, ~1,222 unit tests in `src`.
- **51 core + 41 server integration test files.** 125 corpus documents, 37 docs, 7 site.
- **`MAX_WARM_PAGES` / the LRU eviction order: 0 test references.** `CLAUDE.md` says so itself, and it
  is the project's one standing freeze.
- **`mounts:` containment: 0 tests.** Of the three files matching "mounts", two are unrelated ("the
  diff mounts") and the third (`map_cli.rs:75-109`) asserts only that a mount is *surfaced in the map
  JSON*. Nothing anywhere tests path containment.
- **`TALIESIN_NO_EXEC`: 1 test**, `read_run.rs:42`, covering `read --run` only.
- **An embedded deck in a site: 0 of 13 static validators** (R14, item 109).
- **A `draft:` page: 0 of 13 static validators** (R14, item 110).
- **`deck.js` DOM behaviour: 0 browser tests.** The QR encoder is the exception and has a genuine
  golden net (R14).
- **Prose-versus-behaviour: no gate exists.** AUDITS.md names it as a structurally invisible class,
  and the concurrent critique branch found ~20 false claims across the guide and Internals book.
- **CI: none. `core.hooksPath` is unset in a fresh clone**, so a contributor's PR runs nothing (Wave 1
  item 84's substrate).

---

## The RPN table

Ranked by `RPN = S × O × D`. **Severity is shown separately and must be read separately** — RPN is a
ranking aid, not a truth, and a severity-9 with a low RPN can still deserve action first.

| # | Failure mode | S | O | D | RPN | Detection basis (measured) |
|---|---|---|---|---|---|---|
| F1 | A prose claim in the docs or site contradicts shipped behaviour | 5 | 9 | 9 | **405** | No gate compares prose to behaviour. ~20 live instances found by the critique branch |
| F2 | An embedded deck ships a broken asset, link, or unrenderable math | 7 | 6 | 10 | **420** | 0 of 13 validators reach it; `check` and `--strict` both exit 0 (R14 item 109) |
| F3 | A `deck.js` edit regresses navigation, fragments, overview or presenter sync | 6 | 6 | 9 | **324** | 0 browser tests; existing deck tests assert `deck.rs` emission or read `deck.js` as text |
| F4 | A shape the corpus does not contain is broken | 5 | 6 | 10 | **300** | By construction. LESSONS.md records three real bugs already hidden this way; R14 measured 13 such deck shapes |
| F5 | `--no-exec` fails to stop a document's browser-side code | 8 | 4 | 9 | **288** | 1 test, and it covers `read --run`, not `{js}` emission / `<script>` passthrough / header injection (Wave 1 item 79) |
| F6 | A dogfood-only shape breaks (`docs/`, `site/` are outside the corpus walker) | 5 | 6 | 8 | **240** | `corpus.rs` walks `corpus_dir()` only; `docs/` is reached by four targeted tests. R14 measured that deck math and deck kernel cells exist **only** in dogfood files |
| F7 | A contributor's change breaks a gate nobody ran | 6 | 5 | 8 | **240** | No CI; `core.hooksPath` unset in a fresh clone; the four hand-run gates skip silently when the interpreter is absent |
| F8 | `mounts:` resolves outside the project root | 9 | 3 | 9 | **243** | 0 containment tests (Wave 1 item 80). Preview-only, and needs a hostile or careless `_site.yml`, which is why O is 3 and not higher |
| F9 | `check` spawns an interpreter named by an untrusted project | 9 | 3 | 8 | **216** | No `TALIESIN_NO_EXEC` gate on that path (Wave 1 item 81). Detection is 8 not 10 because the interpreter line is printed in the Environment section |
| F10 | The `MAX_WARM_PAGES` LRU order is silently reordered | 6 | 2 | 10 | **120** | 0 test references, stated in `CLAUDE.md` as breaking silently. O is low only because a standing freeze deters the edit |
| F11 | A `draft:` page accumulates defects invisibly | 3 | 7 | 10 | **210** | 0 validators (R14 item 110). S is low: the preview still lints it and nothing ships |
| F12 | A published tag ships the wrong `LICENSE` | 9 | 2 | 10 | **180** | Nothing checks a tag's contents. `v0.2.0` genuinely ships MIT while HEAD is AGPL (Wave 1 item 83) |
| F13 | A shipped SVG asset renders as a broken image | 5 | 3 | 3 | 45 | **Was 10.** `svg_assets_render.rs` now pins both properties an `img`-loaded SVG needs |
| F14 | A cell output is served stale from `_freeze/` | 8 | 2 | 3 | 48 | 7 test files, 14 references; AP4 hardened the cumulative key. Genuinely well covered |
| F15 | The block-id / sourcepos invariant regresses | 8 | 2 | 2 | 32 | `corpus.rs` enforces it over all 125 corpus documents |
| F16 | Click-to-source stops landing the editor cursor | 6 | 3 | 9 | **162** | The harness stops at the relay; the end-to-end path is permanently manual (`[[qmd-purge-completed]]`) |
| F17 | A kernel-dependent test silently skips, so a live-kernel regression ships | 6 | 4 | 8 | 192 | The three gates exist precisely because a missing interpreter would otherwise be a skip; nothing runs them automatically |

---

## The high-detection cluster

**The round's real product.** Every mode scoring **D ≥ 8** — the failures that would ship silently
regardless of severity — sorted by severity so the reading is honest rather than RPN-flattened.

| D | S | Mode | Why it is undetectable |
|---|---|---|---|
| 9 | 9 | F8 `mounts:` escapes the root | zero containment tests |
| 10 | 9 | F12 a tag ships the wrong licence | nothing inspects a tag |
| 8 | 9 | F9 `check` spawns a chosen interpreter | no gate on that path |
| 9 | 8 | F5 `--no-exec` doesn't stop browser-side code | the one test covers a different command |
| 10 | 7 | F2 a deck ships broken | zero validators reach it |
| 8 | 6 | F7 a contributor breaks a gate | no CI, hooks unset in a fresh clone |
| 9 | 6 | F3 `deck.js` regresses | zero browser tests |
| 10 | 6 | F10 the LRU order is reordered | zero test references |
| 9 | 6 | F16 click-to-source stops landing | harness stops at the relay |
| 8 | 6 | F17 a kernel test skips silently | the gate is the author's memory |
| 9 | 5 | F1 prose contradicts behaviour | no gate of this kind exists |
| 10 | 5 | F4 an absent corpus shape breaks | by construction |
| 8 | 5 | F6 a dogfood-only shape breaks | outside the corpus walker |
| 10 | 3 | F11 a draft accumulates defects | zero validators reach it |

**Fourteen of seventeen enumerated modes score D ≥ 8.** That is the round's headline and it is not a
criticism of the test suite: 1,658 tests is a serious net. It is a statement about *where* the net is.
The suite is dense on **pure functions over the block model** and thin-to-absent on **everything that
only exists at runtime, in a browser, in a published artefact, or in prose**.

**Three structural causes, and every mode above is an instance of one of them:**

1. **The regression net is the corpus, so it can only catch what the corpus contains.** F2, F4, F6,
   F11 — and R14 measured 13 deck constructs the corpus has nowhere.
2. **Nothing runs on anyone's machine but the author's.** F7, F17, and the whole hand-run gate set.
   Wave 1 measured that the gates *pass*; this round's point is that passing is not the same as
   running.
3. **The project's checks assert on emitted strings, and its riskiest surfaces are behaviour.** F3,
   F16, and — importantly — F5, F8, F9, which are exactly Wave 1's three HIGH security findings. This
   is the direct answer to the slate's founding question: those three survived ~30 correctness rounds
   because **correctness rounds read code and this class only fails when something runs.**

---

## Items

### 117. `mounts:` containment has zero tests, and item 80's fix must not ship without one. (HIGH, pairs with item 80)

**Measured.** Grepping test bodies for `mounts` returns three files: `token_contract.rs:147` and
`section_extents.rs:5` are unrelated (the phrase "the diff mounts"), and `map_cli.rs:75-109` asserts
only that a configured mount appears in `taliesin map`'s JSON with the right `at` and `path`. **No
test anywhere asserts that a mount path stays inside the project.**

This is filed separately from item 80 because it is a different deliverable and could otherwise be
lost: item 80 is the fix, this is the pin, and the project's own rule is that a fix is verified by
mutation — restore the bug, watch the named test fail. With no test to name, that verification is
impossible.

**What the pin must cover**, from item 80's own measurement: an absolute `path:` (Rust's `Path::join`
*replaces* the base), and a `../`-climbing relative path. Two cases, one test.

**Refuted if** any test asserts containment on a mount path.

### 118. The `--no-exec` test covers a different command from the one the documentation promises. (MEDIUM, pairs with item 79)

**Measured.** `TALIESIN_NO_EXEC` appears in exactly one test, `read_run.rs:42`
(`read_run_under_no_exec_projects_source_without_a_kernel`), which asserts that `taliesin read --run`
does not touch a kernel and does not crash. That is a correct test of `read --run`.

The documented promise is different: `docs/guide/reference/cli.tmd:148` says `--no-exec` lets you
"preview untrusted docs safely", and item 79 measured that `crates/core` contains **zero**
`TALIESIN_NO_EXEC` references, so `{js}` cells, raw `<script>` and header injection all still run.

**The detection failure is the interesting part.** A reader of the test list would conclude
`--no-exec` is covered. It is, for one command, on the axis that does not matter. This is the
"asserts an adjacent property" shape the D=8-9 band exists for.

**Deliverable is the same either way item 79 resolves** (fix the wording or fix the flag): whichever
is chosen needs a test that names the *preview* path and the *browser-side* channels.

### 119. Detection debt has no home, so each round re-derives it. (MEDIUM, structural)

The high-detection cluster above is the third time this project has assembled a list of
structurally-invisible failure classes. LESSONS.md names four of them in prose; AUDITS.md names the
hand-run gates repeatedly; R14 produced the deck exemption register. None of these is queryable, and
each round pays to rebuild it.

**Proposal, and it is deliberately not a tool.** One table in `notes/` — the D ≥ 8 cluster above, kept
as a live file rather than a dated findings doc — with one row per class and a column for "what would
change this score." F13 is the proof that the column is real: the broken-SVG mode went from D=10 to
D=3 the day `svg_assets_render.rs` landed, and that improvement is currently recorded nowhere a future
round would find it.

**Explicitly not proposed:** a coverage tool, a dashboard, or CI. The cost of this is one file.

**Refuted if** such a register already exists and this round missed it.

---

## What this round deliberately did not file

Per contract rule 4 ("an audit's stated fix can be a revert") and the slate's own trap list:

- **CI is not proposed.** It was deliberately deleted 2026-07-26 for billing reasons and a public repo
  changes that calculation. F7 names the cost; the ruling is Wave 1 item 84's and the author's, and
  R7 has no new evidence to add to it.
- **Telemetry is not proposed** for any detection score. It violates the offline invariant, which is
  one of the attributes producing the good results in R6.
- **No new items for F1, F2, F5, F8, F9, F11, F12** — every one is already an open item (79, 80, 81,
  83, 109, 110, and the critique branch's prose-drift item). This round's contribution to them is the
  detection score, which changes their *ordering*, not their existence.

**The reordering is the actionable output.** By severity-with-detection rather than by severity alone,
the launch-blocking set is: **F8/item 80** (S9 D9), **F12/item 83** (S9 D10), **F9/item 81** (S9 D8),
**F5/item 79** (S8 D9), then **F2/item 109** (S7 D10). Items 80 and 83 rank above 79 on this axis,
which is a change from Wave 1's ordering — 79 led there because it is the most-documented promise, but
83 ships a wrong licence at a tag today and nothing anywhere would catch it.

---

## Not measured

- **No failure was actually injected.** This is a paper FMEA against measured coverage; AP11 is the
  round that injected, and this one deliberately did not repeat it.
- **Occurrence scores are the weakest column.** Severity and Detection rest on measurements;
  Occurrence rests on judgement about usage patterns that have no users yet. Treat O as a tiebreak,
  not evidence.
- **The 1,658 tests were counted, not assessed.** The 2026-07-18 mutation round assessed a subset and
  is the authority on which of them would actually fail; this round did not re-run it.

## Round bookkeeping

This round wrote only this file. Items 117-119 follow R6's 114-116. See
[R14](2026-07-28-deck-exemption-audit.md) on the 79-90 numbering collision between the two live
branches.

**Wave 2 is now complete** (R14, R6, R7). Remaining: Wave 3 (R2, R8, R9, R11), plus R10, R13, and R12
which needs the author's phone.
