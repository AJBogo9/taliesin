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

## S11 [A] MAJOR (PART LANDED, TWO SUBJECTS REFUTED): the W7 dead-code sweep

**Landed 2026-08-13:** subject 1 (the tombstone sweep) and subject 5 (the dead `vocab`
index). `retired_names.rs` went 21 tests / 859 lines -> 8 tests / 547 lines, and
`lsp.rs`'s two `vocab["theoremKinds"]` indexes are gone (that key has not existed since
wave 8; serde_json's Index returns `Null` for a missing key, so both were silent no-ops).

**Two of the five subjects are REFUTED and must not be acted on as filed.**

2. ~~`crates/core/src/schema.rs`, 146 lines~~ **REFUTED.** The *const* has no external
   reader, which is what the finder measured, but the module is not dead: its
   `#[cfg(test)] mod generate` is the generator **and drift gate** keeping
   `assets/schema/tali-site.schema.json` in sync with `site::NATIVE_KEYS`, and that file is
   byte-identical to `editor/vscode/schema/tali-site.schema.json`, which the companion
   ships (`editor/vscode/package.json:93` wires it through `yamlValidation`). Deleting it
   removes one of the four gates `CLAUDE.md` says a new `_site.yml` key trips.
4. `render/model.rs` `after_body` -- **narrower than filed.** Never *populated*
   (`doc_includes.rs:22` says so), but it IS read: `page.rs:716` passes it to
   `include_after_body`. A refactor of a live slot, not a dead-field delete.

~~**Related:** `base_css()`/`site_css()` have every caller a test, so they go with the
tombstones.~~ **Also refuted.** Every caller is a test, but the surviving ones are
*integration* tests in `crates/core/tests/` (`tech_blog.rs:368` calls `site_css`,
`retired_names.rs` calls `base_css`), which are separate crates and genuinely need `pub`.
Neither can be demoted.

**Still open:** subject 3, `$/cancelRequest` batching, which its own doc comment retires.

## S12 [A] MAJOR (PART LANDED): `notes/` is still the largest thing nobody gates

**Landed 2026-08-13:** the three deletable subjects are gone (1,683 lines):
`notes/retired/diagnostics-explanations.rs` (1,222, byte-identical to
`git show pre-cut:crates/core/src/diagnostics/codes.rs` and carrying no header saying it was
dead), plus `notes/ap2-fuzz-harness` and `notes/ap8-determinism-harness` (461). Both
misleading banners are rewritten: `ROADMAP.md`'s expired pause no longer reads as permission
to grow, and `FEATURE-IDEAS.md` no longer states the corpus-pin graduation rule `CLAUDE.md`
retired as circular.

**What is left is the bulk and it is a judgement call, not a sweep.** `notes/` is still
~34,600 lines across ~99 files, gated by nothing (`retired_names.rs`'s `SKIP_PATHS` names
`"notes"`, `stale_docs.rs`'s `ROOTS` excludes it, `gate_script.rs` walks only `crates/`), and
it is still the first thing `CLAUDE.md` tells a fresh session to read. Nothing else in it is
deletable *outright* -- every remaining file is either durable state (`CUT-PROGRESS.md`,
`DO-NOT-REBUILD.md`, `LESSONS.md`) or an audit whose findings may not all have landed.
Deciding which audits are spent needs the author, and S17's purge set overlaps it.

**This file is part of the problem it describes. Delete it when it is empty.**

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

**Re-verified 2026-08-13 at `442603ad`. Every fact below was re-measured, not inherited.**
S18 landed. S16 and S17 are decisions and are left for the author; a recommendation is
recorded for each because leaving none is how they get answered by accident.

## S16 [V] BLOCKING: there is no publish path for the manual

Re-measured: `gh repo view` -> `isPrivate: true`; `gh release list` -> empty; `git tag`
holds no `v*`; `.github/workflows/ci.yml` has **no deploy step**; `taliesin.sh` resolves to
**no DNS record**, and `site/_site.yml:8` bakes it into `sitemap.xml`, `robots.txt` and
`og:url` via `seo.rs`/`meta.rs`. So every README link (clone URL, releases page, the `Docs:`
URL `taliesin help` prints) 404s today, and `tools/build-site.sh` composes a deploy that is
run only by hand and by `--check`.

**Recommendation: decide hosting BEFORE the first tag, not after.** A tag is what makes
`release.yml` attach the tarballs the README now correctly caveats, and a tag with no
reachable docs URL publishes a binary whose `--help` points nowhere. The cheapest coherent
order is: point a DNS record at a static host (or change `url:` to whatever the real host
is), wire `tools/build-site.sh` into a deploy job, confirm the built site loads, *then* tag.
Nothing here is code the audit can write.

## S17 [V] BLOCKING: the purge set versus "make the repository public"

Re-measured: **all seven files are tracked at HEAD, 3,061 lines** -- `notes/STARTUP-PLAN.md`,
`notes/FUNDING-RESEARCH.md`, `2026-07-18-pmf-audit.md`, `2026-07-27-due-diligence-audit.md`,
`2026-07-28-demand-positioning-audit.md`, `2026-07-28-launch-critique.md`,
`2026-07-27-adoption-friction-audit.md`. `.githooks/pre-push:36-41` matches only
`--diff-filter=A` and its own comment says why: "the purge set is still tracked today, so a
check against the whole tree would refuse every push". So the guard stops a *new* one
arriving and cannot remove the seven already there. `git grep -Il "/home/bogo"` matches
**15** files.

**Two live documents still disagree, and this is the irreversible one.**
`notes/2026-08-10-mvp-publish-session.md:77-80` lists "make the repository public" as step 2;
`notes/STARTUP-PLAN.md:111-127` records a contrary already-made decision (a fresh
no-history repo). **Flipping visibility publishes the whole history, not HEAD** -- every one
of the seven, in every commit that ever carried it, plus the absolute paths.

**Recommendation: the fresh no-history repo, and treat the flip as unavailable.** The
disagreement resolves on cost asymmetry rather than taste: a wrong flip cannot be undone
(the history is cloned, cached and indexed within minutes), while a fresh repo costs only
the loss of commit history on a project with zero external contributors and no issues or PRs
to preserve. `git-filter-repo` is installed and the rewrite was rehearsed, so the third
option (rewrite then flip) exists -- but it leaves the same one-way door open on a tree
whose guard is explicitly incapable of checking it. **Whichever is chosen, do it before
S16's tag**, and delete `notes/2026-08-10-mvp-publish-session.md`'s step 2 or
`STARTUP-PLAN.md:111-127` in the same commit so the next session finds one answer.

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
