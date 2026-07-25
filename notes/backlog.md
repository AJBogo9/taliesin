# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git + [AUDITS.md](AUDITS.md) +
> [ROADMAP.md](ROADMAP.md); delete an item when it lands, don't leave a `[x]`. The "already shipped"
> list near the bottom is the compact anti-rot guard (do not re-add / re-scope), not a changelog.

## State (2026-07-25, latest: SKIM-2 fully shipped + SKIM-3a + deck-motion (5))

**Branch `backlog/skim-batch`.** Three items landed after SKIM-1: **23's Ship A**, **24a** (the
three-state `check` severity floor) and **28's (5)** (the overview column count), which empties item
28 completely. Ship A is done (the Cmd-K empty state is now the whole-book
outline; results group by chapter; partial matches survive with `Missing:`; `within1` is Damerau-aware;
actions keep hard AND). Gates at landing: **1379 tests / 0 fail** across 85 binaries with all three gates
and `--test-threads=1`; `cargo fmt --check`, `clippy --all-targets` (0 warnings) and both JS `tsc` gates
clean; `check corpus/tarn`, `check docs/guide` and `check docs/internals` all report no problems. The three
producer fixes were verified **by mutation**; the client was browser-verified at 1440x900 / 390x844 /
900x1440 on a book, a plain website and a single doc, with 0 console errors.

**Four causes the audit recorded wrong, re-derived from source** (the "entries rot" law, again):
- **Ship A's stated dependency on 22b was false as written.** The audit said the index's heading text
  "carries the rendered numbers". It carried **none**: `page_fragment` scoped the render (which numbers
  floats and theorems) but never called `number_chapter_headings`, a *separate* step that only
  `Site::finish_blocks` made. The dependency had to be created, not consumed.
- **Indenting the outline by the record's `l` is wrong.** Absolute heading level depends on whether a
  chapter emits a title block and where it roots, so a `###`-rooted chapter's top-level sections indented
  three steps beside a `##`-rooted chapter's. Depth is measured per page against its own shallowest heading.
- **Every untitled chapter's `# H1` was indexed twice** — the page record and a heading record, same
  destination, same words, adjacent rows. Folded into the page record.
- **The single-doc palette had been showing `Section title#`** since the anchor-links enhancer shipped: it
  read `textContent` straight off the heading, including the hover `#` permalink. `toc-spy.js` already
  strips it for its mobile chip; `search.js` now does the same.

**`corpus/tarn` had zero nested headings**, so neither the `h` path nor the outline indent was exercised by
anything. `grouping.tmd` gained two `###` subsections. Same standing lesson as SKIM-1, one level down: the
fixture only covers the shapes you actually put in it.

### Prior state (2026-07-25, earlier: SKIM-1 shipped + four owner rulings recorded)

**Branch `backlog/skim-batch`, commit `29bc976`.** All of **item 22 (SKIM-1)** shipped: the
corpus fixture (22a), all six heading-layer defects (22b), the docs one-liner (22c) and the
notes hygiene (22d). Gates at landing: **1373 tests / 0 fail** across 85 binaries with all
three gates set and `--test-threads=1`; `cargo fmt --check`, `clippy --all-targets` and both
JS `tsc` gates clean; `check corpus/tarn` reports no problems. Defects 1 and 2 were verified
by mutation; defect 4 was browser-verified at 1440x900 / 390x844 / 900x1440 with 0 console
errors.

**Four owner rulings were taken this session** and are recorded in place on their items:
- **25 / oss-4 — deferred with the public flip.** Not going public until end of summer; the
  tool gets honed first. Nothing gates on it.
- **24 — YES to the three-state `check` severity floor** (reversing the 2026-07-10 decline).
  The "only four binary rules, red gate" fallback is dead; build the floor first.
- **23 — ship A, then B.** Order load-bearing; A is build-ready now.
- **28 — delegated to the implementer, ruled:** (3) and (4) are no-change, **Option C is
  declined**, and only (5), the viewport-driven overview column count, remains as code.

**Two causes were recorded wrong and re-derived from source** (the "entries rot" law again):
- **The numbering bug was not "three sites disagree with each other."** It was ONE shared
  assumption: `section_number`'s hardcoded `level - 2`. `site/chapter.rs` numbers
  POST-demotion emitted HTML; the other two number PRE-demotion source levels. Threading a
  per-site base through a shared `ChapterNumbering` is what makes the slot come out equal on
  both sides of that shift.
- **The scrollspy needed a second fix the audit did not name.** Deriving the activation line
  from `scroll-margin-top` was necessary but not sufficient: scroll offsets quantize to
  device pixels, so a just-landed heading measured a hair below the line and left the
  PREVIOUS entry highlighted. Browser-measured; a 1px tolerance closed it.

**Why the corpus never caught any of this:** every corpus book chapter opens with `# Title`
and no front-matter `title:`, so **nothing in the regression net exercised heading demotion
at all** — while 32 of 32 dogfood chapters do. `corpus/tarn` now carries the titled,
`###`-rooted, body-`# H1`, below-TOC-gate, nested-part and unnumbered-appendix shapes. Treat
that as the standing lesson: the dogfood books are not in the net, so a shape only they have
is a shape the suite cannot see.

### Prior state (2026-07-25, earlier: the hardening batch landed)

The whole build-ready hardening set shipped in one pass, each piece verified by mutation:
**21** (lsp/mcp panic boundary), **26** (AP2: depth guard + render watchdog + the fuzz-regression
harness), **27** (AP4 follow-ups), **13** (OFF-2), **20** (PERF-1), **25** except its owner decision,
**28**'s two code residuals, and two bullets of **10**. Gates at landing: 1351 tests pass / 0 fail.

**Three recorded causes were wrong there too:**
- **The `kernel_executes_..._runaway_cell` flake was never load-sensitive.** `cell_timeout()`
  memoizes in a `OnceLock`, so the test's `set_var("TALIESIN_CELL_TIMEOUT","3")` only took effect
  when that test happened to be the first in the binary to touch the lock. Fixed properly (a
  per-kernel `cell_cap`); the full `--bin` suite went 155 s -> 49 s as a side effect.
- **OFF-2's premise ("inlining 2.5 MB on every save would bloat the payload") was false.** The page
  shell is re-served per *navigation*, not per save. The fix was a same-origin route serving the
  vendored copy, which also keeps working when a doc gains its first diagram mid-session.
- **F-01's fix does not exist as written.** `two-face` ships 199 syntaxes and **none** is PowerShell
  (enumerated, not grepped). See item 17.

**PERF-1 was solved by (b), which subsumes (a):** whole-site -> scoped is **20.1x** on `tech-blog`,
3.5x on `docs/guide`, and **51.7x on a synthetic 200-page book** — and the scoped cost is bounded by
*one page's link count*, so it no longer grows with the book at all.

### The 2026-07-25 audit sweep (earlier the same day)

**2026-07-25 audit-sweep pass.** Every dated audit in `notes/` was re-read and its findings checked against
source at `225a08a`; six items were filed that had been written up but never reached this file, and the
whole open-work list was re-sorted by product impact. New: **25** (the security audit's deferred pre-public
set — the one item with an external date, the repo goes public ~2026-08), **26** (AP2's two input-bound
gaps + the fuzz harness), **27** (AP4's three cache follow-ups), **28** (deck-motion residuals), **29**
(the reduction pass's deferred R1/T2), **30** (demand-probe persona 4). Re-banded: **13, 20, 21 moved from
C to B** — 20 and 21 were tagged P2 while sitting in a band headed "Low / hardening (P3)". Also corrected:
the AP2 and AP4 entries in "Audit perspectives" still read as *unrun* though both produced findings on
2026-07-22, so a future session could have re-run a done round. **Known notes-hygiene gap, not fixed here:**
`AUDITS.md` has no ledger line for six rounds (AP2, AP4, the 2026-07-17 security audit, the 2026-07-24
deck-motion audit, the CAD research, the companion version-skew bug) — the AP2 file carries a
ready-to-paste one.

### Prior state (2026-07-22)

v0.2.0. All four formats render + deploy; the dev loop is strong (block-level incremental updates with
DOM-state preservation, warm server + Jupyter kernel, `_freeze` cache, Alt-click + reverse cursor sync,
located diagnostics, CSS hot-swap, Cmd-K search). The editor language intelligence (diagnostics,
go-to-definition, outline, hover, completion, quick-fix code actions, rename) now ships editor-agnostically
as the `taliesin lsp` stdio server: the **E1-E7 editor-DevX initiative is complete** (see "Already
shipped"). **Most of the backlog has already shipped.** Through item 19 everything is pushed (`origin/main`
at `cc45af4`); the live-executor-mounts F-04 fix landed after that. A large **2026-07-22 (late) backlog-clearing pass**
shipped: focus-mode/fullscreen split (was item 3); a Vite-user
hint banner (item 9); deck `footer:`/`logo:` (item 2); a per-book offline `<book>.zip` (item 6); the
cross-page duplicate-label warning is now located (item 5); DX16 update-nudge ruled **skip**; item 8 i18n
labels **assessed → defer**; and all six item-11 polish passes (a)-(f). **DX17b headless `{js}` also shipped
2026-07-22** (the last high-impact feature); the AP8 determinism guards (was item 15) are complete and
that item is now removed. **The machine-facing `read` projection (was item 19) shipped + pushed 2026-07-22**
(structure-preserving lists/steps/inputs + book-aware chapter/cross-page scoping + whole-book `read <dir>`;
see "Already shipped"). **The live-executor-mounts F-04 fix also landed.** What
remains open is smaller and mostly P3. Ranked below by product impact.

## Next session: start here

**State: the whole SKIM batch is PUSHED** (2026-07-25, 8 commits fast-forwarded onto `origin/main`,
`c964436..10e4a4b`). Re-verified immediately before the push, not trusted from this file: 1394 tests /
0 fail across 85 binaries with the three gates + `--test-threads=1`; `cargo fmt --check`,
`clippy --all-targets` (0 warnings) and both JS `tsc` gates clean; `check` reports no problems on
`corpus/tarn`, `docs/guide` and `docs/internals`. **Do not trust a SHA written here** — re-check with
`git log --oneline origin/main..main`; the author pushes mid-session with no signal in this file.

**Owner ruling 2026-07-25: the Cmd-K empty state stays the whole-book outline.** It was the one change
in the batch that altered a daily-driver surface, and the one-line revert to the flat chapter
jump-menu was offered and declined. Do not re-litigate it; a collapsed-by-default variant was also
considered and is not wanted (it is not a one-liner: it needs collapse state + keyboard handling).

**Items 22 (SKIM-1) and 23's Ship A are gone; both shipped 2026-07-25.** **Four owner rulings were
also taken** (see the prior-state block), which un-gated a large amount of previously-blocked work.
What is left now sorts into:

- **Build-ready TODAY, in this order:**
  1. **33 — finish the `qmd` purge, resume at Task 3** (M). **Take this first.** It is half landed:
     Tasks 1 and 2 of nine are on `origin/main`, and the tree is green and self-consistent in that
     state, but *nothing enforces finishing it* until Task 9 adds the repo-wide guard. Every task has
     an exact file list, the commands and the expected output already written, so this is execution,
     not design. Full brief: [below](#the-qmd-purge-resume-at-task-3). **Wants a quiet tree** — the
     remaining tasks rewrite tokens across ~40 files, so do not run it beside a session editing
     `render/tests.rs`, `base.css` or `web-client/search.js`.
     **Re-scan before starting:** 24b landed after this brief was written and it touched two of those
     three files (`render/tests.rs`, `web-client/search.js`) plus a new `crates/core/src/site/skim.rs`,
     so the task file lists are one merge out of date. `skim.rs` reads the rendered class contract
     (`callout-title`, `tali-theorem-head`, `tali-proof-head`, `panel-tabset`) and is pinned by
     `crates/server/tests/skim_cli.rs` — a rename that misses it fails there, not silently.
  2. **24c** (M) — the trimmed `skim-shape-lints`. Both prerequisites are now down: 24a (the
     three-state severity floor) and 24b (`skim`, the instrument the thresholds are calibrated
     against), both shipped 2026-07-25. **Calibrate against `taliesin skim` output, not intuition** —
     that is what it was built for, and running it already killed three of its own assumptions.
  3. **23 Ship B** (L) — the drawer outline sidecar, now RULED in, after A. **Re-scope first:** Ship A
     put a browsable whole-book outline one keystroke away, so B's remaining value is the *drawer*
     specifically, not the outline as such.
- **Writing, not code** — 30 (`corpus/analyst/`), the last un-probed persona. Diminishing returns
  are real: personas 1-3 found **0** interaction-bugs between them.
- **Needs a device or a demand signal** — 4 (deck mobile, needs a phone), 2 (deferred, revive on a
  real speaker ask), band D (the standing freeze), Tier 3 (waits on real users).
- **P3 residuals on secondary surfaces** — 11, 12, 16, 17, 18, 29's T2. Real, small, low reward.

**`grow-tarn` is done and is now the fixture the scale-sensitive items were waiting on.**
`corpus/tarn` is 12 numbered chapters across 3 parts + a nested part, and it deliberately carries the
shapes the rest of the corpus lacks: a titled chapter (so heading demotion is exercised at all), a
`###`-rooted one, one with a body `# H1`, one below `MIN_TOC_HEADINGS`, an over-`BODY_CAP` section
whose distinctive term sits in its last paragraph, two `{.definition}` blocks, and an unnumbered
appendix. **Use it instead of minting a fixture.** Note it is a *documentation* book, not a scale
fixture: do NOT grow it toward 200 pages, and do NOT mint `corpus/longbook` (the walker renders every
corpus doc on every `cargo test`).

**The standing lesson from SKIM-1, worth more than the six fixes:** every corpus book chapter opened
with `# Title` and no front-matter `title:`, so **nothing in the regression net exercised heading
demotion** — while 32 of 32 dogfood chapters do. A bug lived on every dogfood page under a green
suite. The dogfood books (`docs/guide`, `docs/internals`) are NOT in the test net, so any shape only
they have is a shape the suite structurally cannot see. When a defect is reported on a dogfood page,
first ask whether the corpus has that shape at all.

**Two live corrections a fresh session should not re-learn the hard way:**
- **Item 17's F-01 cannot be fixed as written** — `two-face` has no PowerShell syntax at all (199
  syntaxes, enumerated). Don't spend a session on the "one-liner".
- **The `kernel_executes_..._runaway_cell` flake is fixed and its cause was never load** (it was
  `OnceLock` memoization of `cell_timeout()`). `--no-verify` is no longer the move for a pre-push
  failure there; a failure now means something real.

**Item 14 (heading-demotion) was found already shipped** (2026-07-12, `7e60f6c`) when picked up
2026-07-22: AP9's "12 sibling `<h1>`" was a stale-artifact false lead (it measured a gitignored pre-fix
`corpus/bayesian-website/_site/index.html`; a fresh render/build emits exactly one `<h1>`). See "Refuted by
measurement". (Item 22 was NOT a re-open of it: demotion worked; the section-number counter had never been
taught about it. Both shipped 2026-07-25.)

- **Or run one of the four remaining *audit perspectives* ("Audit perspectives" section below):**
  proactive, findings-generating angles the prior rounds structurally could not see. **Done so far:
  AP1, AP2, AP4, AP5, AP8, AP9, AP10, AP12** (perf, fuzzing, cache-correctness, i18n/sourcepos, codebase
  health, determinism, semantic HTML, offline-proof). **Remaining: AP3 (concurrency), AP6 (cross-browser),
  AP7 (a11y), AP11 (chaos)** — all four are *stateful/solo* (server/kernel/browser), so run one when no
  parallel session owns that surface. Each is a fresh session that writes a dated findings doc and feeds
  build-ready items back here; the author has credits queued for exactly this. Recommended next:
  **AP7 (deep a11y)** and **AP3 (concurrency)** are the highest-yield of the remaining stateful set.

Working method is in "Standing constraints": branch per feature, verify by mutation, browser-verify,
ff-merge locally, delete the item here on landing.

## The `qmd` purge: resume at Task 3

> **This is item 33, the top of the build-ready queue.** Half landed, plan written to the step,
> and the only open item whose cost grows while it waits.

**Goal: zero live `qmd` tokens.** Owner ruling 2026-07-25: purge everything **except**
`docs/superpowers/` and `notes/`, which stay frozen as the pre-rename record (rewriting them would
make a 2026-06 plan claim it used names that did not exist yet). Breaking changes are fine, no
back-compat aliases survive.

**Plan:** [docs/superpowers/plans/2026-07-25-purge-qmd-naming.md](../docs/superpowers/plans/2026-07-25-purge-qmd-naming.md).
Nine tasks, each with an exact file list, the commands, and the expected output. **Tasks 1 and 2 are
done and pushed.** Resume at **Task 3**.

**Why this is not a `sed`.** Measured, not assumed: a blanket `qmd` -> `tali` substitution over the
whole tree **builds clean** (one `include_str!` path aside) and changes the state of only **5 of 1387
tests**, three of which are block-id hash drift. The suite is structurally blind to renames because
every assertion is a string literal living in the same file as its emitter, so both sides move
together. Do not "just try it".

**Task 1 (done) built the net the suite lacked** — `crates/core/tests/token_contract.rs` +
a `FORMAT_VERSION` pin in `freeze.rs`. Each was verified to *fire* on the defect it targets, not
merely to pass. Four traps found while building it, all of which will bite again:
- A block-level corpus render reaches only **4 of the 15** `data-qmd-*` names. Site chrome, the page
  shell, and JS-stamped attributes never appear in it, hence the second source-side census.
- The two censuses need **different scanners**. Rendered HTML needs `data-` whitespace-preceded *and*
  followed by `=`/`>`/`/`, or it matches the `id="data-modeling"` heading slug and prose like "the
  canonical data-figure loop". Source needs only a token boundary (attributes appear as
  `querySelectorAll("[data-x]")`); the whitespace rule finds **1 of 13**.
- The orphan check must read the **live census, not the pin**, or it keeps checking the old name and
  passes. It must also **exclude Rust from "consumers"** — Rust is the emitter, so counting it made
  the check vacuous.
- `data-qmd-drawer-close` exists only inside a `<script>` string literal in `site/chrome.rs`, so the
  Rust half of the scan is found **by content**, never by a hand-written file list.

**Task 2 (done) renamed the contracts no Rust test can see** — storage keys, the theme CustomEvent,
`?tali=`, the `tali_token` cookie, and the editor postMessage protocol. Browser-verified. Two things
the file inventory missed, found only by grepping for residue afterwards:
- `<style id="qmd-theme">` is also read by `client.js:1328` (preview theme hot-swap) and named in
  `protocol.rs` and `theme_css.rs`. Renaming just the `theme.rs` emitter breaks the hot-swap silently.
- `deck.js` carries a **second** postMessage protocol for speaker-window sync, discriminated by a
  `{ qmd: 'deck' }` field. Both ends are in `deck.js`, so it stayed self-consistent by luck.

**Remaining, in order.** 3: `data-qmd-*` -> `data-tali-*` (15 attrs, 39 files). 4: script types
`application/tali-js` + `tali-define`, rename `qmd-js.js`, drop the `qmd` cell-API parameter, **bump
`FORMAT_VERSION` to 4**. 5: the 13 name collisions + delete the seven `window.qmd*` aliases. 6:
`_qmd_*` -> `_tali_*` in the injected Python prelude. 7: corpus/docs/site/tools/CI + re-bless
snapshots. 8: repackage the VS Code companion. 9: the repo-wide `no_qmd.rs` guard + CLAUDE.md.

**Three traps waiting in the remaining tasks:**
- **Task 4: do NOT rename the `qmd` cell-API parameter to `tali`** — it sits beside an existing
  `tali` parameter, so you get a duplicate parameter name: legal in sloppy mode, a **`SyntaxError`
  under `"use strict"`**. Delete it instead.
- **Task 4: the `FORMAT_VERSION` bump is not optional.** `d0b1ffa` is this exact bug already shipped
  once. The freeze key hashes a cell's *source*, never its output, so renaming a token that appears
  in cached output leaves every existing `_freeze/` replaying markup the runtime cannot read. The pin
  added in Task 1 now forces the bump.
- **Task 5: renaming the aliases instead of deleting them yields `window.taliJs = window.taliJs;`** —
  a self-assignment that looks right and silently removes the compatibility it existed for.

**Two corrections to earlier notes, re-derived from source:** `qmdFast.path` / `qmdFast.open` are
**not** live VS Code settings keys (the real one is `taliesin.path`); those hits are a stale-branding
guard regex plus stale prose in `troubleshooting.tmd`, and should be deleted, not renamed. And
`site/showcase.tmd:183-232` references six `var(--qmd-*)` custom properties that **are not defined
anywhere** — the real tokens are `--tali-*` — so those two `{js}` exhibits have been rendering
unstyled. Task 7 fixes that as a side effect.

**Found in passing, not fixed:** `data-scrolly-name` (emitted at `divs.rs:660`) is read by **nothing**
— not `scrolly.js`, not Rust. `scrolly.js` takes the name off the hidden input's `data-qmd-input`.
Recorded in `NO_RUNTIME_CONSUMER`; deleting an emitted attribute is a behaviour change, not a rename.

## Standing constraints (read before working)

- **Do-NOT-touch (one freeze):** `MAX_WARM_PAGES` + the deterministic LRU eviction in
  `serve_site/exec_pool.rs` (M6a, sign-off refused 2026-07-17) and the **single-editing-surface**
  invariant (the preview is read-only; it must never write back to source). The rest of the
  exec/kernel zone is not frozen (its audit finished, M2-M5 sign-offs granted + spent).
- **Website / brand** (2026-07-11 audit, detail:
  [2026-07-11-website-design-audit.md](2026-07-11-website-design-audit.md)): the personal blog
  (`corpus/tech-blog/`) is the forward-facing brand, direction **"Marginalia"**; its 14 explicit KEEPs
  live in that file. Every change stays invariant-safe: no CDN, no preview write-back, no new output
  format, offline bundling, `--tali-*` tokens only.
- **Author policy:** feature-first (finish framework features before marketing-site work).
- **Working method:** branch per feature; brainstorm if there's a fork; spec under
  `docs/superpowers/specs/`; implement TDD; verify (cargo + browser via chrome-devtools, or the
  extension harnesses); fast-forward merge locally; delete the item here. Push to `origin/main` only
  when the author asks. **Review subagents get a git worktree or you commit first** (a "read-only"
  reviewer with `Bash` still writes scratch files to your CWD; one ran `cat > Cargo.toml` in the repo
  root and destroyed the workspace manifest).
- **Tests: three gates, or the suite silently under-tests itself** (CI sets all three):
  `TALIESIN_REQUIRE_NODE=1` (JS-equivalence guard), `TALIESIN_R=R TALIESIN_REQUIRE_R=1` (R kernel),
  `TALIESIN_PYTHON=… TALIESIN_REQUIRE_KERNEL=1` (pool-booted `--jobs` path; a missing interpreter is a
  hard fail, not a skip). `cargo test` aborts the remaining binaries at the first failure, so re-run
  before trusting a total. **If an `exec` probe test fails, `--test-threads=1` before blaming your
  change** (there are two flake families: two load-sensitive *timing* tests, and two `exec::tests`
  *concurrency-race* tests, both under P3 below). **The runaway-cell one is fixed as of 2026-07-25 and
  its recorded cause was wrong** — it was `OnceLock` memoization of `cell_timeout()`, not load; see item 10.
- **Git:** do not trust a SHA written in notes. Check `git log --oneline origin/main..main` for what is
  unpushed and `git reflog show origin/main` before believing any "not pushed" claim; the author pushes
  mid-session with no signal here.
- **How this file lies to you:** entries rot (the author pushes mid-session; a scoped prune leaves the
  rest looking freshly reviewed). Before picking an item, **grep its named symbol/flag in source** and
  prefer measuring the running product over reading this file. Trust an item's *symptom*, never its
  cause, line number, or stated cost (all three have rotted). Verify a fix by **mutation** (restore the
  bug, watch the named test fail), not by a green suite. Grep traps live here: a bare word matches
  prose, `grep | head` reports head's exit code, quote `--include='*.tmd'` in zsh.

## Open work (priority order: product impact)

Ranked highest user/product value first. Impact is not the same as buildability, so each item carries a
gating tag: a high-impact item can still be frozen or need a ruling.

### A. High impact (build first)

33. **Finish the `qmd` purge (Tasks 3-9 of nine).** ⬅ **top of the queue.** Plan:
    [2026-07-25-purge-qmd-naming.md](../docs/superpowers/plans/2026-07-25-purge-qmd-naming.md);
    full brief and the traps in [The `qmd` purge](#the-qmd-purge-resume-at-task-3) above.
    Tasks 1 (the regression net) and 2 (the contracts no Rust test can observe) are on
    `origin/main` at `375ad81`. Remaining, in order: **3** `data-qmd-*` -> `data-tali-*`;
    **4** script types + rename `qmd-js.js` + drop the `qmd` cell-API parameter + **bump
    `FORMAT_VERSION` to 4**; **5** the 13 name collisions + delete the seven `window.qmd*`
    aliases; **6** `_qmd_*` -> `_tali_*` in the injected Python prelude; **7**
    corpus/docs/site/tools/CI + re-bless snapshots; **8** repackage the VS Code companion;
    **9** the repo-wide `no_qmd.rs` guard + CLAUDE.md.
    **Why it is high impact despite being a rename:** it is the only item whose *cost grows*
    while it sits. Every new `data-qmd-*` attribute, storage key or script type added
    meanwhile is another site to rewrite, and the half-done state has no guard holding it in
    place. It is also cheap and low-risk now — the net exists, each task ends green, and the
    plan is written to the step.
    **Do not** attempt it as a blanket `sed`; see the brief for why that builds clean and
    still breaks things.

25. **Pre-public release checklist: one owner decision left** (detail:
    [2026-07-17-security-release-audit.md](2026-07-17-security-release-audit.md)). The five code
    items shipped 2026-07-25 (`dos-pages`: a ws `?page=` the site cannot resolve no longer allocates
    a never-evicted `PageState`; **DEP-03**: mermaid vendored at 11.16.0 with an explicit
    `securityLevel: 'strict'`, `THIRD_PARTY.md` updated and now drift-locked by a test that reads the
    version out of the bundle itself; `dos-rich`: an 8 MB cap on rich-output bytes, the axis the
    stream-byte and output-count caps both missed; `dos-ws-size`: `max_message_size` on both ws
    upgrades; **CMD-01**: the warm pool logs its resolved interpreter like the cold path already did).
    **What remains is not a task:**
    - **oss-4 — RULED 2026-07-25: deferred, and the public flip with it.** The owner is not
      going public yet ("I'll do it at the end of summer; before that I want to hone the tool
      to its final form"). So this is not a task and not a blocker: nothing here gates any
      other work. Re-ask when a flip date is actually set. The question when it is: whether to
      prune `notes/` + `docs/superpowers/`. No secret is exposed (the `--host` token design doc
      discloses only a per-session UUID mechanism), but it is a curated bug roadmap.
    **Verified NOT open, do not re-scope:** `SECURITY.md` exists, the tracked `/home/bogo` paths are
    scrubbed, and PT-1 / PT-2 / NET-1 / OUT-1 / DEP-01 / DEP-02 all shipped 2026-07-17. Refuted by the
    audit and not worth revisiting: `dos-yaml` (libyaml rejects the alias bomb in ~30 ms — the guard is
    in the C library, so grepping our source for it correctly finds nothing) and NET-3
    (non-constant-time token compare).

### B. Medium impact

23. **SKIM-2: honest search, honest output, and the whole-book outline** (P2, M; **22 has shipped, so this is unblocked**; detail:
    [2026-07-24-skimmability-audit.md](2026-07-24-skimmability-audit.md)). Two sessions, in order:
    - ~~**Session 2, honest output.**~~ **SHIPPED 2026-07-25.** `BODY_CAP` is gone (re-measured
      first: it truncated **18.7%** of Guide and **25.9%** of Internals section records;
      uncapping costs **1.20x** indexed chars / **1.17x** index bytes, and 3.2-10.5 ms per
      keystroke INCLUDING the full result re-render). **Not split into per-block records**,
      which the audit also proposed: the record count is exactly what Ship A's outline
      renders, so splitting would put duplicate rows in it, and the measured growth gives
      splitting nothing to fix. `section_text` is now exactly `render::indexable_text`, which
      also closes **29's R1** on the Cmd-K side. Tab panels ship `hidden="until-found"` with
      all four edits, **plus a fifth the audit named but understated**: walking ancestors from
      the target finds nothing, because the index anchors a section to its HEADING and the
      tabset is a SIBLING of it — `revealFor` also reveals the panel in the landed section
      that contains a query term. And it must run in a macrotask: inline it raced tabset.js's
      own DOMContentLoaded registration (measured — the click did nothing). Cell output is
      bounded in CSS via `--tali-output-max` with a NEW vertical fade, a `<pre>`/table scroll
      + image `object-fit: contain` split, and a print reset; new fixture
      `corpus/layout/dense-output.tmd` (kernel-free) drops that page from ~19,000 px to
      2,275 px.
    - ~~**Session 3, Ship A.**~~ **SHIPPED 2026-07-25.** The palette's empty state is now the whole-book
      outline (every page + its sections, indented), results group by chapter with a "+N more" disclosure,
      partial matches survive with a struck-through `Missing: X`, `within1` is Damerau-aware, and actions
      keep hard AND. Producer gained `c` + `h` (omitted when empty, so a website's index is unchanged).
      **Three things the audit got wrong, re-derived from source:**
      (1) **Ship A's stated dependency on 22b was false as written.** The index's heading text carried
      **no** section numbers at all: `page_fragment` scoped the render (which numbers floats + theorems)
      but never called `number_chapter_headings`, which is a *separate* step `Site::finish_blocks` makes.
      So the dependency had to be *created*, not consumed. (2) **Indenting by `l` is wrong** — absolute
      level depends on whether a chapter emits a title block and where it roots, so a `###`-rooted chapter
      indented three steps beside a `##`-rooted one. Depth is now measured against each page's own
      shallowest heading, client-side. (3) **Every untitled chapter's `# H1` was indexed twice** (page
      record + heading record, same destination, same words, adjacent rows); it is now folded into the
      page record. None of the three is visible without rendering the outline, which is why the audit
      missed them.
    - **Ship B** (the drawer sidecar, L, genuinely new: mdBook, Docusaurus, GitBook, Starlight and Material
      all list author-declared pages, never harvested headings). **RULED 2026-07-25: ship A, then B**, and A
      has now shipped. It needs a per-page outline fragment (measured: the body field is 87% of raw and 92%
      of gzipped index bytes, so an outline-only sidecar is ~13x smaller gzipped) plus a
      `refresh_search_for_page`-shaped invalidation. Ship B also decides whether a drawer type-ahead is
      wanted at all. **Re-scope it before building:** Ship A already put a browsable whole-book outline one
      keystroke away, so B's remaining value is the *drawer* specifically, not the outline as such.
    **Pins:** `corpus/tarn` throughout — grown 2026-07-25, use it, do not mint a fixture (its
    over-`BODY_CAP` section is `filtering.tmd`'s "How nulls behave", whose distinctive term
    `null-dropping-filter` sits in its LAST paragraph, i.e. past the old cap; its 12 chapters for
    grouping); extend
    `corpus/tarn/install.tmd` for the tab attribute + the zero-intrinsic-size rule + the visible panel's
    first-child margin; new `corpus/layout/dense-output.tmd` (kernel-free: long `<pre>`, 200-row table, tall
    image) asserting the bound, the print reset, **and that the horizontal `pre` shadow is unchanged**.
    *(Ship A's pins landed: `tarn.rs` asserts every record carries `u`/`i`/`l`, that a numbered chapter's
    records carry `c` + a numbered `h` path, and that a website's carry no `c`; `render/tests.rs` pins the
    three client behaviours against the bundled JS. `grouping.tmd` gained two `###` subsections because
    tarn had **zero** nested headings, so neither `h` nor the outline indent was exercised at all. The
    grouping render itself is unpinned, by design.)*
    **Invariants:** offline (no CDN, the index is a same-origin lazily-loaded subresource), read-only overlay,
    zero new config keys (no `search.boost`, no per-page `search.exclude`). Nothing here touches the frozen
    `exec_pool` LRU; at 60+ chapters a preview reader evicts constantly, and the consequence is **a slower
    cold chapter, not a correctness bug**.

4. **Deck engine mobile polish** (P2): mobile pinch/pan + touch gestures (they matter for the phone-feed
   deck mode; hard to verify without a device); drop `fitSlide` from the resize path (needs a lazy
   fit-on-show refactor first). *(The desktop trackpad half shipped 2026-07-24 — pinch / ctrl+wheel-down
   opens the overview map, with a 250 ms hysteresis; see item 28 for what that left behind.)*

2. **Deck presenter tools** *(owner deferred 2026-07-22 — NOT selected this round)*: one-command deck
   publish (Share QR still encodes `localhost:PORT`), a presenter laser/spotlight, auto-advance. The
   `footer:`/`logo:` threading from this item **shipped** (see "Already shipped"); the presenter pieces
   were considered and left for later. Revive only on a real speaker ask.

### C. Low / hardening (P3)

24. **SKIM-3: author-side structure tooling** (P3, M-L, **its severity-floor prerequisite shipped 2026-07-25**; detail:
    [2026-07-24-skimmability-audit.md](2026-07-24-skimmability-audit.md)). `taliesin check` has 27 diagnostic
    families and **none** concerns document structure: it prints "no problems found" on a 32,600-word book
    with a 4,077-word chapter behind 9 headings and a broken number scheme on every page. Genuine market gap
    (measured from source: Vale/Google 2 of 31 rules structural, Microsoft 4 of 39, proselint 0 of 26,
    markdownlint's are syntactic). Dependency order is strict:
    - ~~**`skim-suggestion-severity` first (S).**~~ **SHIPPED 2026-07-25.** `Floor` is three-state
      (`--errors-only` / default / `--strict`); `codes::SUGGESTION` + `severity_rank` + `gates_at` own the
      ordering so no two commands can disagree; `check --strict` is new; `build --strict` and `publish`
      count only `check::blocking(...)`, so advice never blocks a release. **The audit understated it:
      this was not just plumbing for a future lint.** The opt-in `prose-lint:` rules were classified
      `TAL-CHECK`/**ERROR** by the `classify` fallback, so `weasel word \`simply\` (consider cutting)`
      already failed `check`, `build --strict` and `publish` — a green gate cost you the rule. They are
      now `TAL-PROSE-WEASEL`/`-REPEAT`/`-BANNED` at `suggestion`, placed **first** in `TABLE` because the
      needles below include `("math", …)` and "weasel word \`mathematically\`" would have classified as a
      math diagnostic. The summary no longer calls advice a "problem" beside an exit 0.
    - ~~**`taliesin skim` + `machine-shape-projections`**~~ **SHIPPED 2026-07-25.** `taliesin skim <dir>
      [--format human|json]` prints the layer cake (numbered headings, each section's opening sentence,
      captions / callout titles / theorem statements) as one linear stream; `Site::skim()` is the typed
      form in `crates/core/src/site/skim.rs`. `map --format json` gained `words` + `headings` per page, and
      the LSP outline's `detail` is now the section's prose length. `prose::word_count` is `pub`.
      **Three of this entry's own facts were wrong, re-derived from source:**
      (1) **`BODY_CAP` does not exist** — SKIM-2 deleted it, so "not `search::section_text`, which is
      `BODY_CAP`-truncated" named a dead reason. Counting from markdown extents is still right, but because
      prose selection (excluding fenced code + `:::` fences) is a *markdown* notion. (2) **`lsp.rs:806`/`:809`
      is `frontmatter_key_doc`**, unrelated; the `detail` site is `to_document_symbol`. (3) `prose.rs:69` was
      correct. **What the instrument found by being run** (none of it visible from source): reading a section
      with `indexable_text` welded a tabset's labels and shell commands onto the opening sentence, because the
      flattened stream has no terminators — `skim` reads the first **paragraph** instead, skipping `<pre>`,
      figures, tables and the callout/theorem/proof boxes. A separate "skip past the title heading" fix was
      written, then **deleted as dead**: the first-`<p>` rule already excludes headings, and removing it left
      the projection byte-identical on both `corpus/tarn` and `docs/guide`. Cost measured, not assumed:
      `map --json` on `docs/internals` went 0.33 s → 0.42 s (debug) because it now renders every page.
      Pins: `crates/server/tests/skim_cli.rs` (13), `first_sentence` unit tests (11), one LSP `detail` test.
    - **`skim-shape-lints`, heavily trimmed (M).** Ship only the threshold-free binary rules: HEADING
      (duplicate, empty, contentless, title-echo, near-duplicate first-two-words), CAPTION (empty,
      label-only, uncaptioned float that an xref points at), TOC-DROP (a heading the shared `toc_items` filter
      discards; natural home for 22's `cli.html` residual), and NO-DESC (**depends on a derived first-sentence
      gist**, else it degenerates into "you did not set `description:`" and fires on 100% of the corpus).
      **Cut RUN, DENSITY, EMPHASIS, FANOUT, SKELETON, FORWARD:** measured against the corpus none has a
      defensible threshold (the flagship RUN rule fires on exactly **one** of 36 dogfood pages and that one is
      a false positive; the headline "1,832-word run" is 1,021 words of table cells). Nothing resembling a
      readability grade, and never a rule about heading *form* (Sanchez/Lorch: no differential effect). Watch
      `codes.rs::classify`, which matches by ordered substring with needles as generic as `("math", …)`.
    - **Independent medium items, no ordering constraint:** per-chapter prose length in the drawer and landing
      Contents (absolute units, never a normalized bar; do not touch `is_article`, it is test-pinned); the
      preview/build TOC selector divergence (`client.js:847` selects by absolute tag, the build filters
      relative to the shallowest, so the author tunes navigation against a TOC readers never see; `base` is
      already correct, do not "fix" it); a citing-sentence backlink line; book-scoped resume; a static "Part,
      Chapter" ribbon (**owner call**: it adds a fourth persistent top element, and the dwell-time evidence
      says the first viewport is the screening surface).
    - **`section-extents` is an owner ruling, not a task.** The DOM has no section boundaries (zero
      `<section>` wrapping content headings on 17 of 19 built guide pages; `using/code.html` is 47 flat
      siblings; repo-wide `<section>` is emitted only by `render/deck.rs` and the footnotes block at
      `render/mod.rs:905`), which blocks four proposals. **Recommendation: option (b), a `data-section-end`
      marker computed from the walk `lsp_outline.rs` already does** (purely additive, invisible to the diff
      and to the corpus invariants). Option (a), a real wrapper, is the one that would also unlock
      `content-visibility: auto` and sticky section headings, but it changes the parent/child shape the
      incremental diff mounts, which is a design question, not an implementation detail. Pin:
      `corpus/layout/structure.tmd` (already named by `FEATURE-IDEAS` #26, still does not exist).
    **Pins:** `corpus/diagnostics/skim-shape.tmd` tripping each surviving code exactly once **plus** a
    well-shaped `skim-shape-clean.tmd` asserted to produce zero, so the rules cannot pass vacuously; extend
    `check_cli.rs`'s DX18 exit-code tests with the three-state cases; `corpus/demo-book` + `corpus/tarn` (grown 2026-07-25) for the
    projections.
    **Invariants:** the finding lands in the CLI or the editor and the **author** edits the `.tmd`: no preview
    gesture, no auto-fix, no write-back. The preview "skim view" is a *display* of a read-only projection, not
    a transformation of the source. No LLM anywhere: byte-identical build output is actively pinned
    (`build_reproducibility.rs`, `parallel_build_determinism.rs`) and `include_str!`-bundling cannot carry
    model weights, so generated summaries are dead at both read time and build time. Zero new YAML keys.
    **Deferred / do not schedule** (record in "Decided against" so they are not rediscovered): a
    reading-density fold (three unbuilt prerequisites, and its premise is measurably overstated);
    `content-visibility: auto` (behind a measured trigger and option (a)); the `:~:text=` half of deep links
    (`strip_tags_separated` inserts a space at every tag boundary, and 669 of 876 dogfood paragraphs contain
    inline code, so fragments miss on exactly the identifier queries they exist for; ship the `?h=` half
    alone); `changed-since`; read-aloud (verdict recorded: out on cost, not on principle).
    **Killed by verification, do not re-scope:** section hover previews (built and deleted at `318f22f` 13
    days before the audit, pinned by three tests), a TOC entry budget (the depth window is already relative,
    two tests pin it), margin footnotes (two real footnotes exist in the whole repo), and `taliesin split` (it
    would repair 0 references on the chapter it was designed for, and `_site.yml` round-trips destroy
    load-bearing comments).
    **Note for the author, no code in it:** roughly half the measured problem is *content*. Zero of 37 dogfood
    pages set `description:`, 8 xref links exist across 19 chapters, 0 backlink lines render, and
    `docs/internals` is 60,208 words with zero `{.definition}` blocks. A glossary, a term index and a float
    digest all produce near-empty output until an authoring pass happens; defer those three rather than
    building them into an empty registry.

28. **Deck-motion: decided, no code left** (detail:
    [2026-07-24-deck-motion-audit.md](2026-07-24-deck-motion-audit.md)). Option A shipped
    2026-07-24 and its **two residuals shipped 2026-07-25**: overview content flips are now instant
    (a `.tali-nofx` frame suppresses the fragment and magic-move transitions while `.overview`
    toggles, so the zoom is the one thing moving — browser-measured 4 concurrent content transitions
    → **0**), and magic-move is resynced (`CAM.morph`/`morphFade`/`morphFadeDelay` replace the
    hand-copied `.45s`/`.4s`/`480`/`560` literals, with one cancellable per-div settle mirroring
    `deck.aaSettle`; hammering forward/back mid-morph now leaves **0** stranded inline styles where
    it used to race naked timers). **RULED 2026-07-25** (owner delegated the call: "use your own best judgement"):
    - **(3) no-change.** An out-of-order arrival stays visually identical to a step. Distinguishing
      them buys a cue the reader has no vocabulary for, and every arrival path already cuts when far.
    - **(4) the overview is a glance at a ~20-slide talk, NOT a navigator for 100+.** So **Option C
      (the shared-element FLIP rewrite) is declined** — the readability floor closed most of the gap
      it existed for, and a 100+-slide deck is not a shape this tool is being built for. Record it as
      decided, not deferred, so it is not re-costed a third time.
    - ~~**(5)**~~ **SHIPPED 2026-07-25.** One wrap count for the whole map, chosen by measuring
      the map at each candidate count and taking the one whose whole-map fit is largest — rows
      counted from the REAL run lengths, because `n/cols` under-counts (a run boundary rounds up)
      and picks a count that then does not fit. **Measured, because the premise needed checking:**
      on `corpus/deck.tmd` the change is a **no-op at every viewport tested** (that deck's runs
      already wrap optimally), so the item's shape had to be built to see it. On a 21-slide
      three-topic deck at 1100x1000 the overview now shows **23 of 25 slides against 13**, at 145 px
      tiles instead of 218 px — which is the right trade for a glance surface. A
      coverage-weighted refinement that also modelled the readability-floor fallback was tried
      and measured **worse** (15 of 25); it is out, and the comment says so. Do not re-refine
      without measuring.
    **Two LOW tradeoffs flagged to the author and left as-is, not defects:** ctrl+wheel-*down* claims
    browser page-zoom-out over the deck (that *is* the approved gesture), and it also fires inside an
    embedded deck on a scrollable page.
    *Option B (mode-invariant serpentine grid) and Option C are costed in the audit and were not
    chosen; the overview work is identical under A and B, so nothing shipped is wasted if B is ever
    revisited.*

17. **Demand-probe (OSS docs-maintainer, persona #2) findings** (P3, in-scope; detail:
    [2026-07-22-corpus-demand-probe-docs-maintainer.md](2026-07-22-corpus-demand-probe-docs-maintainer.md)).
    A realistic library documentation site (`corpus/tarn/`, corpus-pinned by `tarn.rs` + a `/gallery/tarn`
    marketing-site exhibit) probed the tabsets × full-text-search × API-reference cluster. The *stacked*
    interactions (book × Guide/Reference parts × two `.panel-tabset`s per page × `.code-walkthrough` ×
    guide→reference `.tmd#anchor` cross-page links × chapter-scoped `@sec-` refs × Cmd-K search spanning the
    book incl. tabset-hidden content × version/deprecation callouts × mount) ALL work — 0 interaction-bugs.
    Four P3 findings, all on secondary surfaces. **Highest-placed of the P3 demand-probe set because F-01 is
    the only one a reader sees on the page:**
    - **F-01 (friction, P3) — SYMPTOM REAL, RECORDED FIX WRONG (re-derived 2026-07-25).** The symptom
      stands: `powershell` and `ps1` both render as unstyled plain text with a `TAL-CODE-LANG` warning
      (`bash` highlights fine). But the filed one-liner cannot work: **`two-face` has no PowerShell
      syntax at all.** Enumerated, not grepped — its set is 199 syntaxes and PowerShell is not among
      them, and no feature flag adds one. (The "ordering trap" the old entry warned about is moot too:
      `resolve()` already consults the bundled set first and falls back to the extras, so a syntax in
      either set would already resolve.)
      **A real fix means vendoring a grammar**, which is a decision, not a drive-by: the upstream
      PowerShell/EditorSyntax grammar is a 43 KB `.tmLanguage` plist (needs syntect's `plist-load`
      feature, which is not enabled) and its `LICENSE.txt` 404s, so its terms need establishing before
      anything is vendored — particularly with the repo about to go public (item 25). Left to the
      author with that groundwork done. A cheap alias to another language is NOT an option: it would
      mean confidently wrong highlighting instead of honestly absent highlighting.
    - **F-04 (friction, P3):** single-file `check` (the editor companion) false-positives a `site/gallery.tmd`
      card's `mounts:` link as broken, because single-file mode lacks site/mount context; whole-site
      `taliesin check site` is clean and the build is unaffected. Candidate: treat an unknown-prefix link
      matching an enclosing site's `mounts:` entry as valid in single-file mode. Related to item 10's
      "`mounts:` live serve untested" + item 16's F-04.
    - **F-02 (WAI, no action):** the a11y heading-skip lint fires on a `#` title + flat `###` API entries;
      the linter is correct (demote entries to `##`). Recorded as an authoring-DX nuance, not a defect.

10. **Reliability / test-infra long tail** (P3, dev-facing):
    - **R cold-kernel orphan residual:** IRkernel has no `ParentPollerUnix` equivalent, so R cold
      kernels still orphan on ungraceful parent death; there is no clean fix (PDEATHSIG is the only
      lever and is hazardous), and R is rarely the cold single-doc path. `kernel.rs`. (The
      warm-pool, cold-Python and `/tmp`-sweep halves all landed.)
    - **`mounts:` live serve/discovery: only an automated live-HTTP test is missing** (the live-executor-mounts
      branch LANDED): the F-04 work reworked `serve_site` mount discovery/serving and unit-pins
      the pure `match_mount`/`resolve_project`/`classify_change` helpers, and live mount serving is
      browser-verified. What remains is only the bin-crate gap of an end-to-end live-HTTP serve test (no
      `reqwest`/`TcpListener` harness). Low-value (mounts are preview-only), demand-driven.
    - **The "two load-sensitive timing tests" were one bug and one false alarm (settled 2026-07-25).**
      `kernel::tests::kernel_executes_state_errors_and_interrupts_runaway_cell` was **not** load-
      sensitive: `cell_timeout()` memoizes in a `OnceLock`, so its `set_var("TALIESIN_CELL_TIMEOUT","3")`
      only bit when it happened to be the first test in the binary to reach that lock; otherwise the cap
      stayed at 120 s and the 20 s assertion failed. **Fixed** — the cap is now a per-kernel `cell_cap`
      the test sets directly, so it is deterministic regardless of test order (the full `--bin` suite
      also dropped 155 s → 49 s). `exec::tests::pooled_kernel_serves_cells_without_a_long_warming_state`
      does **not** assert on elapsed time at all: it polls `pool.ready_len()` — already the "wait on a
      state signal" shape this bullet asked for — bounded at 10 s. Nothing to fix unless that bound is
      ever seen to trip.
    - **Two `exec::tests` concurrency-race tests** (NOT timing):
      `a_successful_probe_pins_the_freeze_key_format` +
      `a_failed_interp_probe_is_not_memoized_for_the_process_lifetime`. On pristine `main` they fail
      ~2 runs in 3 in a full `--bins` run, never when filtered, and pass 3/3 under `--test-threads=1`
      (which is slower, so it refutes timing). The assertion: the freeze key's interpreter-id segment
      comes back **empty**; `probe_version` returned `None`, and since the 10s `bound` can't have fired,
      the spawn failed. Leading (unproven) hypothesis: **`ETXTBSY`** from `write_exe`'s (`exec.rs:1228`)
      write-then-exec race across tokio threads. **Do not fix from this note** (exec/kernel zone,
      unproven): the cheap first move is to make `probe_version` log *why* it returned `None`, then
      re-run the full suite until it trips.
    - **Mermaid `<script>` SRI + `crossorigin`: now moot by construction.** Nothing fetches mermaid
      from a CDN any more — build inlines the vendored copy and preview serves it from a same-origin
      route (OFF-2) — so there is no cross-origin subresource left to pin. It would only come back if
      someone points `TALIESIN_MERMAID_URL` at a CDN, which is an explicit opt-out.
    - **Perf (low):** protocol-level op-message batching (one WS message per save, not per-op). Worst
      case: an edit near the top of a long doc where every downstream block emits a `SetMeta`
      (`diff.rs` `anchor_op`). Client + server ship together, no wire-compat constraint.
    - **Audit long-tail:** a tens-of-MB cell output blocks ZMQ receive before the cap fires
      (`kernel.rs`, do-not-touch).

11. **2026-07-22 polish-audit residuals** (P3 hardening + a11y + "feels finished"; detail:
    [2026-07-22-polish-audit.md](2026-07-22-polish-audit.md); [AUDITS.md](AUDITS.md) records the round).
    **Passes (a)-(f) all shipped** (design-system single-source, scaffold `<h1>`/`<time>`, `<article>`
    landmark, announce/focus holes, CLI/diagnostics, reduced-motion+print, emitted-markup a11y; see
    "Already shipped"). Only low/opportunistic leftovers remain:
    - **Tokens (F2/C5/F4/S3):** a `--tali-scrim` token folding 3 divergent overlay alphas (PA-F2),
      per-slide-bg hex drift-lock (PA-C5), px/rem breakpoints (PA-F4), base.css's own `.15s` uses (PA-S3).
    - **a11y interaction (B3/B5/B14/B15):** mobile-TOC focus-trap, cite-tabs roving-tabindex, Cmd-K
      Home/End, menu tab-out-close.
    - **Semantics (M3/M13, H1):** `<ul>`/`role=list` (needs a CSS-grid + category-filter-JS restructure +
      browser verify), hero/card image-alt lint nudge, deck `theme-color`/OG (PA-H1 residual).
    - **CLI docs (CLI1/2/3):** `--help` drift (undocumented `preview --port` / `read --run` / hand-written
      usage). Owner design-Qs (deck copy-button, card whole-`<a>`) are parked in the doc, not build-ready.

16. **Demand-probe (course pilot) findings** (P2/P3, in-scope; detail:
    [2026-07-22-corpus-demand-probe-course-author.md](2026-07-22-corpus-demand-probe-course-author.md)).
    A realistic lecturer's course (`corpus/course/`, corpus-pinned by `course.rs` + a `/gallery/course`
    marketing-site exhibit) was authored to probe where a book-length computational project meets friction.
    The *stacked* HTML interactions (book × shared-theorem-counter × chapter-scope × cross-page refs ×
    deck-embed-in-chapter × code-walkthrough × `{python}` cell × draft-appendix) ALL work — 0 interaction-bugs.
    The remaining findings sit on secondary surfaces (F-01 book-level `theorems:` and F-02 book-scoped `read`
    both shipped 2026-07-22, see "Already shipped"):
    - **F-03 (friction, P3):** the `read` text projection of `{{< embed >}}` (leaks iframe UI chrome) and
      `.code-walkthrough` (steps + code concatenate) is lossy.

18. **Demand-probe (interactive-explainer, persona #3) findings** (P3, in-scope; detail:
    [2026-07-22-corpus-demand-probe-interactive-explainer.md](2026-07-22-corpus-demand-probe-interactive-explainer.md)).
    A single-page explorable explanation (`corpus/descent/`, gradient descent, pinned by `descent.rs` +
    a `/gallery/descent` exhibit) stacked the interactive cluster the corpus never combined on one page —
    `{{< input >}}` sliders × a **draggable** `{js}` graphic × a `.scrolly` sticky `{js}` graphic × a
    reactive Plot cell × math × two numbered SVG figures — and it ALL works, standalone and mounted, 0
    console errors. Two remaining P3 findings (F-01 read-projection fusion shipped 2026-07-22, see "Already
    shipped"):
    - **F-02 (gap, P3):** an authored numbered figure is emitted as `<img src="fig.svg">`, and an
      `<img>`-embedded SVG is style-isolated: it can't see `--tali-*` or the `qmd-theme` toggle, only the
      **OS** `prefers-color-scheme`. So a reader who forces the page theme opposite their OS gets the
      figure in the wrong palette (light-palette labels, weak contrast, on a dark page). Inline `{js}`/SVG
      graphics on the same page track the toggle fine (they use `--tali-*`). Candidates: an inline-SVG
      figure path so `![](x.svg)` inherits page vars, or document a neutral-palette convention. Edits
      would touch `crates/core/src/render/figure.rs` (figure emission).
    - **F-03 (WAI, authoring nuance):** a `{js}` "once" cell's returned node is mounted *after* the cell
      body runs, so an attachment-gated init (`if (!node.isConnected) return`) silently no-ops the first
      paint. Gate teardown on `invalidation`, not DOM attachment. WAI but a sharp edge — candidate: a doc
      line in the `{js}`-cell reference, or an optional post-mount hook.

29. **Reduction-audit residuals** (P3, dev-facing; detail:
    [2026-07-17-reduction-audit-map.md](2026-07-17-reduction-audit-map.md)). Phase 2 + T1 + R2 shipped and
    the codebase is lean; two items were explicitly deferred and never filed here. Both re-verified open:
    - **R1 — two divergent text extractors.** *Half closed 2026-07-25:* the Cmd-K side no longer has
      an extractor of its own — `search::section_text` was `indexable_text` **plus a 1500-char cap**,
      and with the cap gone it is exactly `render::indexable_text`. What remains is the original
      divergence: `text_content` (which feeds `llms.txt`) decodes `&#8217;`/`&nbsp;`,
      `render::indexable_text` does not, so naively reusing one would leak raw entities into
      `llms.txt`. That fork is pinned by a passing test, so it is conscious, not a bug. Its stated
      sequencing hook is spent (item 23 has shipped); revisit only if a consumer needs them equal.
    - **T2 — three site modules each run their own raw-source pre-scan** (`site/xref.rs`, `site/book.rs`,
      `site/discovery.rs` each `read_to_string` the page and re-implement a slice of the include/parse
      pipeline). A recurring pattern rather than a single bug; unify on one shared pre-scan **if you are
      already in there**, not as a standalone refactor. Overlaps item 20, which wants exactly one shared
      whole-site pass.

32. **The overview map is framed low, with dead space above it** (P3, S; measured 2026-07-25 while
    verifying 28's (5), and **pre-existing** — it reproduces identically on a build from before that
    change, so it is not a regression from it). On a 21-slide three-topic deck at 1100x1000 the map
    is 626 px tall in a 1000 px viewport (it would fit centred) yet opens at `mapTop` **459 px**, so
    ~460 px of empty stage sits above it and the last row is clipped 85 px below. The old column
    count showed the same offset (`mapTop` 439). Suspect `fitOverview`'s `roomy` test
    (`deck.js`: `all >= floor` → centre on `gw/2, gh/2`, else follow the current tile): the map
    measurably fits, so `roomy` should be true and it is not — either `gridDims` reports more rows
    than are placed, or `presentScale()` is read at a moment when it is not yet meaningful. **Trust
    the symptom, re-derive the cause**; instrument `deck.ov` rather than reasoning from the formula
    (that is how the 28(5) refinement went wrong). Pin: none exists — the overview is client JS with
    no server-rendered output, so this needs a browser measurement, not a corpus doc.

30. **Demand-probe persona 4 (analyst) artifact** (P3, M, mostly writing; spec
    `docs/superpowers/specs/2026-07-22-corpus-demand-probe-design.md` §4). The four-persona demand-probe
    program ships each persona as one artifact in three roles — a green corpus pin, a findings doc, and a
    `/gallery/<name>` exhibit. Personas 1-3 landed (`corpus/course/`, `corpus/tarn/`, `corpus/descent/`, all
    pushed) and their findings are items 16-18. **Persona 4's `corpus/analyst/` was never authored**
    (confirmed absent), so the program is 3 of 4 and its slate is the only remaining un-probed shape.
    Diminishing returns are real and should set the priority: personas 1-3 each stacked the interactions the
    corpus had never combined and found **0 interaction-bugs** between them, only P3 friction on secondary
    surfaces. Worth finishing for corpus coverage, not because a fourth probe is likely to find a defect.

12. **i18n / Unicode multibyte correctness: DONE bar a demand-driven residual.** The LSP UTF-16 encoding
    fix shipped 2026-07-22 (folded from AP5; detail:
    [2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md)): the stdio
    LSP advertises `positionEncoding: utf-16` and converts at every boundary (I18N-2/3/4/5); I18N-1 was
    resolved as documentation (block start columns are always ASCII-prefixed, so the client conversion was
    unreachable). *Residual (not build-ready, demand-driven, do not spin up without a real ask): RTL
    layout, CJK line-breaking, non-ASCII heading-slug collisions.*

### D. Gated, not actionable now (kept visible, do not spin up)

- **M6a `MAX_WARM_PAGES` / `exec_pool.rs` eviction:** the standing freeze; sign-off refused 2026-07-17.
  Eviction drops the executor and kills its kernel child processes, so this is kernel lifecycle, not a
  constant. Do not tune without a new ruling.
- **M2's hanging-interpreter sibling** *(needs its own exec/kernel ruling)*: a *hanging* (not missing)
  interpreter costs ~161s recovery, downstream of the (bounded) `interp_id` probe in the warm-pool
  forkserver READY wait + kernel-start retries. `kernel::tests::transient_start_errors_retry_but_missing_interpreter_does_not`
  shows the *missing* case is handled and the *hanging* one is not. `kernel.rs`/`warm_pool.rs`.
  *(Aside, pre-existing + load-bearing: `crates/server/Cargo.toml` doesn't list tokio's `process`
  feature though `kernel.rs`/`warm_pool.rs`/`exec.rs` use it; it compiles only via feature unification.)*
- **M4 test stand-in flake:** the M4 test's `sleep 300` stand-in kernel survives ~2 of 8 full-suite runs,
  only when the build is cold. Measured, unexplained, argued test-only (a real kernel has three reclaim
  nets where the stand-in has one). Worth an hour only if a real kernel is ever seen outliving its pool.
- **D72 bare `@key`:** declined for now (the diagnostic already ships, so nothing renders wrong
  silently, which makes it a feature question not a defect). Edits `crates/core/src/cite/`, needs
  sign-off if revived.

## Audit perspectives (unexplored angles: pick ONE per session)

Brainstormed 2026-07-22 against the [AUDITS.md](AUDITS.md) ledger, which already covers UI, feature polish
(x3), website/marketing design, machine-facing/AI-native, the deck subsystem, DX, PMF, the VS Code
companion, simplification/reduction, and a pre-open-source security + supply-chain pass. The twelve items
below are the dimensions those rounds could **not** see: they need the tool *run hard*, fed *hostile
input*, or reasoned about as a *concurrent system*, rather than "look at rendered output" or "read code for
feature quality." They are the moat's crown jewels (source-mapped, warm, incremental, offline) which no
round has yet stress-tested.

**These are perspectives, not tasks.** Point a fresh session at one item. It produces a dated findings doc
in `notes/` (same shape as the existing audit files: headline, verified findings ranked by corrected
severity, and false leads recorded honestly per the project's "trust the symptom, re-derive the cause"
rule), records the round in [AUDITS.md](AUDITS.md), and files the build-ready findings back into "Open
work" above with their own prefix. Every session inherits the **Standing constraints** at the top of this
file (Do-NOT-touch freeze, verify-by-mutation, entries rot so re-derive from source). Two facts that set
the priority: non-test code carries ~700 `unwrap()`/`expect()`/`panic!`/`unreachable!` sites, and
`data-sourcepos` (the load-bearing invariant) is byte-offset based.

**Run one perspective per session** (context isolation + token budget: the author's stated preference).
The *stateful* ones (AP1, AP2, AP3, AP4, AP5, AP6, AP11) each build, fuzz, run the server, bind ports,
drive a browser, or spawn kernels, so they corrupt each other if run at once: keep them solo. Only the
pure code-read ones (AP9, AP10, AP12, and the read half of AP8) are safe to fan out together in one
Workflow. The recommended-first set (**AP1, AP2, AP4, AP5**) plus the one safe code-read pick (**AP10**) are
now all RUN (see their entries below). **Everything remaining — AP3 (concurrency), AP6 (cross-browser), AP7
(a11y), AP11 (chaos) — is stateful/solo** (server/kernel/browser/ports), so each needs a session where no
parallel work owns that surface. Highest-yield of the four: **AP7 (deep a11y)** and **AP3 (concurrency)**.

### Tier 1: genuinely untouched, highest expected yield

- **AP1: Performance & scale. RUN 2026-07-23** (findings:
  [2026-07-23-ap1-performance-scale-audit.md](2026-07-23-ap1-performance-scale-audit.md); build-ready PERF-1
  folded into Open-work item 20). Result: **no quadratic anywhere** — single-doc render is sublinear per
  block (8000 blocks in 647 ms), site cold build linear + parallel (400 pages in 874 ms), the block diff is
  O(n log n) by construction. The one degradation is the warm-preview moat: every `.tmd` save in a site/book
  preview runs **two** independent full-site sequential render passes (`refresh_xrefs` + `validate_cross_page_links`),
  a linear-in-(pages × blocks-per-page) tax the source annotates as a fixed "~27 ms" — already ~60 ms/keystroke
  on the real 17-page `tech-blog`, extrapolating to ~360 ms at 100 pages. Not a bug (DX1 rightly OK'd it at
  corpus size); the fix is to stop paying it twice. Refuted: `site.clone()` O(pages²) (it is `Arc`), hover-index
  quadratic, render quadratic. Residuals not chased: kernel RSS drift, multi-hour warm RSS. *Was stateful/solo;
  done as a release-binary measurement + a `taliesin-core` path-dep harness.*
- **AP2: Robustness / adversarial input (fuzzing). RUN 2026-07-22** (findings:
  [2026-07-22-ap2-robustness-fuzzing-audit.md](2026-07-22-ap2-robustness-fuzzing-audit.md); build-ready
  AP2-1/2/3 folded into Open-work item 26). Run in an isolated worktree with a subprocess-isolated harness
  that makes a panic, a stack-overflow **abort** and a **hang** all separately observable (an in-process
  fuzzer cannot — an abort kills it too): 133 hand-crafted hostile `.tmd` + 7,500 generative mutations over
  both the doc and full-page paths + targeted include/KaTeX/site-config/deck/`check` probes through the real
  binary. Result: **the premise was overstated and is corrected** — every server/CLI render entry already
  wraps rendering in `catch_unwind`, the core render already runs on a 256 MB worker stack, and the round
  produced **zero unexpected panics**. The two real gaps both bypass that armor via one root cause (no
  size/depth/time bound): AP2-1 deep nesting → an uncatchable `abort()` that defeats even the per-page site
  isolation, AP2-2 balanced nested brackets → a comrak-0.52 inline O(n²) render hang. Plus AP2-3, the
  still-true "zero fuzz coverage" half. Refuted: the four grep-flagged reachable `unwrap` sites (all correct
  code). *Was stateful/solo.*
- **AP3: Concurrency / race conditions.** The server multiplexes a `notify` file watcher, websocket
  handlers, a warm ZMQ kernel, the exec pool, the `MAX_WARM_PAGES` LRU, and `_freeze/` writes across N
  browser clients. Rust stops data races, not logic races: save-while-executing, file-change-mid-build,
  two clients on one preview, concurrent freeze writes, eviction interleaving. Start: a stress driver plus
  a code read of shared-state ordering in `serve_site/exec_pool.rs` (respect the M6a freeze: observe, do
  not retune). *Stateful, solo.*
- **AP4: Cache-correctness (adversarial freeze). RUN 2026-07-22** (findings:
  [2026-07-22-cache-correctness-audit.md](2026-07-22-cache-correctness-audit.md); AP4-1 shipped, AP4-2/3/4
  folded into Open-work item 27). Covered BOTH halves — the read hunt (enumerate every input the cumulative
  key folds in, then find a change it cannot see) and the empirical half (construct that change and prove a
  stale hit against a real `ipykernel` 7.3.0 in a throwaway dir). Result: the design is sound on every axis
  the key is *supposed* to see, but the promise is worded as near-absolute ("the **lone** by-design stale-hit
  path = packages") and that overclaims. **AP4-1 (medium, reproduced and FIXED the same day):** a cacheable
  cell downstream of a `#| cache: false` cell restored a stale output **on a cold build** — one rendered doc
  printing `A: 890903` and `B: 859248` for the same variable — because `plan()` capped only the warm prefix,
  not the disk-tail restore. Shipped the correctness fix (option 2: force the whole downstream tail to
  re-run) over the audit's own doc-only lean, because the stale hit was observed in practice and option 1
  would have left "nothing to clear by hand" false. Refuted: options-stripped-code staleness, FNV chain
  ambiguity, interpreter-swap invalidation, atomic-write crash safety. *Was stateful/solo.*
- **AP5: i18n / Unicode / multibyte sourcepos. RUN 2026-07-22** (findings:
  [2026-07-22-i18n-unicode-sourcepos-audit.md](2026-07-22-i18n-unicode-sourcepos-audit.md); folded into Open-work
  item 12). The starting hypothesis (byte-based sourcepos breaks Alt-click on any CJK/accent doc) was mostly
  *refuted*: the primary Alt-click locator uses the line only, block start columns are ~1, and all BMP text is
  correct. The real find was the editor LSP: it speaks Unicode scalars, comrak emits byte columns, and the TS
  companion is UTF-16 (three conventions, none of them UTF-16), diverging on astral characters (rename is a write path).
  A follow-up, done as a code read rather than the browser sweep first planned. Residual not yet chased: RTL
  layout, CJK line-breaking, non-ASCII heading-slug collisions.
- **AP6: Cross-browser / cross-platform.** CLAUDE.md mandates chrome-devtools MCP and development is
  Linux-only, so Safari, Firefox, and mobile browsers are effectively untested, as are macOS/Windows path
  handling, file-watch semantics, and kernel spawning. The vanilla-JS client and the deck engine are where
  these bugs hide. Start: drive the client through Firefox + WebKit (Playwright headless); grep the Rust for
  `\`-vs-`/` path assumptions and Linux-only syscalls. *Stateful, solo.*

### Tier 2: partially touched; a dedicated deep pass still pays

- **AP7: Deep accessibility of the output.** The polish rounds found chrome-level a11y holes; nobody has
  done a real screen-reader + keyboard pass over rendered docs, and especially the **deck as an interactive
  application** (focus management, `aria`, KaTeX a11y, live-region announcement on slide change). Overlaps
  item 11 pass (c) but goes deeper than one-hole-per-surface. *Stateful, solo.*
- **AP8: Determinism / reproducibility. RUN 2026-07-22, findings shipped + closed** (findings:
  [2026-07-22-determinism-audit.md](2026-07-22-determinism-audit.md); was Open-work item 15, now complete +
  removed). Covered
  BOTH halves (the read hunt AND the stateful rebuild-twice check, via the frozen binary). Result: a positive
  bill of health. Single-doc renders and a full multi-page site build are byte-identical across separate
  processes with fresh HashMap seeds, and determinism holds by construction (sorted discovery/listings/hover
  index, index-placed parallel builds, no time/random in output, cross-machine reproducible). One low finding,
  DET-1: no explicit end-to-end regression guard, so the manually-maintained property could silently regress.
- **AP9: Semantic-HTML / document-model correctness. RUN 2026-07-22** (findings:
  [2026-07-22-semantic-html-audit.md](2026-07-22-semantic-html-audit.md)).
  Result: a strong positive bill of health. Across 84 corpus renders + a site build the emitted HTML is
  structurally valid (no invalid nesting, no per-page duplicate ids, well-formed figures/tables/lists,
  labelled deck sections). Its one finding, HTML-1 (titled docs emit many sibling `<h1>`), was **REFUTED on
  2026-07-22** when picked up: heading-demotion had already shipped 2026-07-12 (`7e60f6c`), and AP9's "12
  `<h1>`" measurement came from a stale gitignored `corpus/bayesian-website/_site/index.html` (a pre-fix
  build); a fresh render/build of that page emits exactly one `<h1>`. See "Refuted by measurement". Done as a
  render-probe + offline HTML-parse audit, no browser drive needed.
- **AP10: Internal codebase health. RUN 2026-07-23** (findings:
  [2026-07-23-ap10-codebase-health-audit.md](2026-07-23-ap10-codebase-health-audit.md); build-ready HEALTH-1
  folded into Open-work item 21). Run as the pure code-read pick alongside a live parallel session
  (`ask-ai-handoff`), written up in an isolated worktree. Result: healthy — **dead code is essentially nil**
  (2 `#[allow(dead_code)]`, corroborating the reduction audit), and the ~708-panic surface is dominated by
  guarded/structural sites. One finding, **HEALTH-1 (medium):** the two *persistent stdio servers* (`lsp`,
  `mcp`) render/project user docs in their request loop with **no per-request `catch_unwind`**, unlike the
  guarded `serve`/`build` paths and unlike the LSP's own `render_buffer` (which uses `serve::guarded`); a
  catchable panic in the every-keystroke diagnostics render (`publish`→`buffer_diagnostics`) or the MCP
  `handle` kills the server for the session. Also **raises AP2-1/AP2-2 priority** (the abort + hang kill a
  persistent server, not a recoverable 500). Refuted: LSP position-math panics (`lsp_pos.rs` defensive +
  tested), dead-code sprawl. *Was code-read, fan-out-safe — the correct pick under contention.*
- **AP11: Chaos / failure-injection UX.** Kill the kernel mid-cell, fill the disk during a build, drop the
  websocket, SIGKILL the server: how graceful is each degradation and what does the author actually see? DX
  touched error loops; nobody has injected real failures. (Note PA-B1 in item 11: the kernel-unavailable
  message already tells headless callers to click a Restart button that is not there.) *Stateful, solo.*
- **AP12: Offline-guarantee verification. RUN 2026-07-22** (findings:
  [2026-07-22-offline-guarantee-audit.md](2026-07-22-offline-guarantee-audit.md); folded into Open-work item
  13). The tool's own assets proved genuinely offline; the gap is author-introduced external references, which a
  `--out` build keeps with no diagnostic (proven by a build probe), plus preview lazy-loading mermaid from a CDN
  despite the vendored copy. Done as a code read + a frozen-binary build probe (no network capture needed). Not
  chased: whether built HTML leaks absolute local paths or author identity (the second sub-question here).

## Tier 3: demand-driven (band E; build only when a real user asks)

Per the PMF audit ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md)) the highest-value next move is
**real users, not more features**, so this whole band waits on demand.

- **Companion (Phase 2):** editor commands (`.tmd`-buffer text transforms only, never preview gestures);
  `editor.wordWrap` default for `[taliesin]`; grammar polish (YAML-type `#|`/`//|`/`%%|` values;
  recommend cell-language extensions via `.vscode/extensions.json`); **marketplace packaging hygiene**
  (`.vscodeignore` misses `.vscode-test/` (1.8 GB), `test-fixtures/`, `scripts/`, `out/test/`,
  `out/e2e/`; no top-level `icon`/`repository`/`license`/`keywords`; `"private": true` blocks publish);
  `symbolCache` only invalidates on save (`completions.ts`, low). **Two release-hygiene residuals from the
  2026-07-13 version-skew bug** ([2026-07-13-companion-check-unexpected-output-bug.md](2026-07-13-companion-check-unexpected-output-bug.md)):
  the extension version is still `0.1.0`, so a stale install silently shadows a fixed build instead of being
  visible at a glance — bump it on every repackage; and `editor/vscode/` carries **two untracked `.vsix`
  build artifacts** (`taliesin-companion.vsix` from Jul 13, `taliesin-companion-0.1.0.vsix` from Jul 21),
  neither in `git ls-files`, so they are a stale trap unless release regenerates them. *The reported bug
  itself is closed: the CLI moved from a bare array to `{diagnostics, environment}` and the packaged parser
  lagged, producing a false "check produced unexpected output" on line 1 of every file; the parser fix
  (`b40ec0e`) is now present in the installed bundle (verified). Also worth a design call if the CLI's JSON
  shape ever moves again: a `"schema"` field the parser can branch on, or a pinned/bundled CLI.*
- **LaTeX hover-preview in the VS Code editor** (Companion Phase 2, a sub-case of the LSP item below):
  hover `$…$`/`$$…$$` to see a rendered preview. Math is already grammar-recognized
  (`tmd.injection.tmLanguage.json:15-37`), but the extension has **no HoverProvider** yet
  (`editor/vscode/src/`). Rendering-reuse is cheap: `math::render(latex, display)` is a pure, memoized
  function (`math.rs:57`), wrappable in a thin `taliesin math <expr>` subcommand. The **hard part is
  fidelity**: KaTeX's HTML+CSS will not survive VS Code's Hover sanitizer (no external stylesheet or
  `@font-face`), and the `katex` crate emits no image/SVG, so a legible offline hover likely needs a
  rasterization step (new dependency surface), not a reuse of the offline KaTeX path. **Spike first**
  (does the Hover sanitizer keep enough inline styling to be legible? VS Code's own Markdown extension
  does math hover, so there is precedent). Build it as a sub-case of the **LSP** item below (write-once
  for Neovim/Helix/Zed/VS Code), not a bespoke VS Code-only hack. *Gating: M, demand-driven, fidelity risk.*
- **`.tmd` format-on-save** (open question): a source pretty-printer must preserve `data-sourcepos` line
  stability for click-to-source; brainstorm reflow-vs-risk first.
- **Dogfood: migrate the FL-weather book to Taliesin** — a real Quarto to Taliesin migration +
  portability stress test; pin a reduced version under `corpus/` if it renders clean.
- **`check` online-link mode** (opt-in `--online`; default stays offline/deterministic).
- **`taliesin publish` follow-ups:** optional `--init` wrapper for the one-time `wrangler` setup;
  email-allowlist (Cloudflare Access) mode. (Also the Zenodo DOI on-ramp, `CITATION.cff`/`.zenodo.json`
  to a GitHub-release DOI, belongs with Wave 5's repro/print-pdf track.)
- **Interactive/explorable numerics** (`FEATURE-IDEAS.md` #62-66; none pinned; promote with a corpus pin
  when one graduates; must NOT reintroduce a reactive VM). Highest-leverage: **#62** a bundled
  numerics/stats global for `{js}` + **#63** `animate`/play-tick + draggable-`point` `{{< input >}}`.
- **Wave 5** (`ROADMAP.md`): print-pdf track (paged render *of* the built HTML), docs-as-spec, `{glsl}`
  cell language, SEO completeness. **Fold `llms.txt`/`llms-full.txt`** in (the block model separates
  prose from code/math at `client.js:50`). *Pin: a `tech_blog.rs` assertion that `llms.txt` lists
  discovered pages + `llms-full.txt` excludes drafts.*
- **Site-level shared bibliography + hygiene** (M). `bibliography:` is per-document only
  (`cite/mod.rs:42`). Allow it in `_site.yml`, merged under each page's; add two read-only diagnostics
  ("entry never cited", "duplicate key") over the parsed registry (does NOT touch the BibTeX/CSL
  do-not-touch core). *Pin: a small site, one entry cited from two pages, one uncited.*
- **Author structure panel** (M/L). A read-only preview sidebar: the heading tree with per-section word
  count (`client.js:50-58` already counts) + a badge per node for unresolved xref / TODO / over-goal
  length. Click to scroll; move the editor cursor via cursor sync. An annotation layer on the dev panel,
  not a new component. *Pin: `corpus/layout/structure.tmd`.*
- **Session revision digest** (M). Surface the `BlockOp` stream the client already receives: a session
  word delta + a feed of the last N ops, each click-to-source. *Behavioral pin (a `tools/live-edit-bench`
  assertion), not a corpus doc.*
- **Block-level transclusion** `{{< include file.tmd#sec-id >}}` (M). Reuse a section across a series.
  Must ride **on top of** the `includes.rs` source-map pass (resolve fragment to block range, hand a
  sub-slice), never rewrite it. Hard gate: the source map must not perturb. Defer until a series needs it.
- **LSP for the language intelligence** (L). Everything an LSP needs is already in Rust (`check`,
  `vocab`, `register_xref`, bib parser, `closest()`); write-once for Neovim/Helix/Zed/VS Code, removes
  the `#| label:` completion drift. The preview stays the view (two `postMessage` shapes in
  `docs/internals/protocol.tmd:325-350`). Do NOT rebuild the preview as an LSP.
- **Image optimization** (WebP/AVIF + `srcset` + lazy-load behind a content-hashed cache) — until posts
  get image-heavy.
- **Marketing site** (deferred, feature-first; rolls into a demo-machine rebuild): `live-edit-hero-demo`
  clip; swap `site/_site.yml` placeholders; demo-led hero rebuild (3-viewport spot-check of the
  already-fixed 390px hero overflow + theme/video desync); **#12 demo video needs a pause affordance
  (WCAG 2.2.2) + reduced-motion respect** and its baked-in desktop text downscales ~3x on mobile
  (re-record or ship a mobile source); mobile embed refine; deploy.
- **`serde_yaml` fallback watch-item:** if 0.9 breaks against a future serde/edition, swap to
  `serde_yaml_ng` (v0.10), gated on a test that `Error::location().line()` still works. Fix the stale
  `Cargo.toml` comment (names the unsound `serde_yml`) when touched.
- **PMF demand-driven tail** ([2026-07-18-pmf-audit.md](2026-07-18-pmf-audit.md), Tier C; each waits on a
  real user asking): hover-preview extended to inline `[@key]`/footnotes (reuse `site/hover.rs`);
  reader-owned document-level show/hide-code toggle (a reader-local pref, a11y-exempt); on-page code+data
  download plus a "reproducible" affordance; scroll-synced TOC greying of passed sections;
  versioned/permanent-URL scheme for link-rot distrust; deck autoplay/kiosk loop; a docs "deck powers"
  page (the `?`/`m` shortcut menu exists; first-timers don't know it does).

## Quarto catalog (policy, not a task)

**Owner ruling 2026-07-16: no sweep. Triage an area on demand, when you next work that area.** Before
consulting it read the triage doc's "three layers" section
([2026-07-16-quarto-catalog-triage.md](2026-07-16-quarto-catalog-triage.md)): the entries are the asset
and were well-grounded on 2026-07-03, but the heading status is degenerate and the executive summary is
misleading. A skeptic verdict is evidence, never a ruling (its "drop Atom feeds" verdict was overruled;
Atom shipped with autodiscovery).

## Already shipped: do not re-add / re-scope

The bulk of this file used to be blow-by-blow `LANDED` records; that detail lives in git +
[AUDITS.md](AUDITS.md). Kept here only as the anti-rot guard (grep the named symbol before trusting any
claim that one of these is "missing"):

- **The 2026-07-25 hardening batch** (branch `backlog/hardening-batch`; was items 13, 20, 21, 25, 26, 27,
  28's code half, two bullets of 10). Grep before doubting any of it:
  - `serve::guarded` now wraps the per-message dispatch in `lsp::main_loop` **and** `mcp::dispatch`, so a
    panicking request answers with JSON-RPC `-32603` and a panicking notification is logged and skipped
    instead of killing the session. Pinned by `a_panicking_message_does_not_kill_the_session` +
    `a_panicking_method_becomes_an_error_and_the_next_call_still_answers` (both use a `#[cfg(test)]`
    `PANIC_PROBE_METHOD`, since real input does not panic — AP2 proved that).
  - `MAX_NESTING_DEPTH` (1000) + `overlong_nesting()` bound blockquote/list nesting **before** the parse,
    turning AP2-1's uncatchable SIGABRT into a located diagnostic (verified: exit 134 → exit 1).
  - `TALIESIN_RENDER_TIMEOUT` (default 30 s, `0` disables) is a watchdog on a now-**detached** big-stack
    render worker, so AP2-2's comrak O(n²) bracket hang returns a diagnostic instead of freezing. The
    worker takes owned inputs for `'static`; the include path hands over its existing `String`/`Vec`.
  - `crates/server/tests/hostile_input.rs` is the AP2-3 regression net: a trimmed hostile battery through
    the real binary, classifying panic / abort / hang as three distinct outcomes.
  - `_freeze/` temp files are `<page>.json.<pid>_<uuid>.tmp`; `is_uncacheable` matches
    `kernel::TRUNCATION_MARKER` (the bracketed emitted form, single-sourced beside the emitters).
  - Mermaid is vendored at **11.16.0**, initialised `securityLevel: 'strict'`, and served in preview from
    the same-origin `PREVIEW_MERMAID_PATH` (`/_taliesin/mermaid.min.js`) — **nothing fetches it from a CDN
    in any mode now**. `the_mermaid_version_claim_matches_the_vendored_library` drift-locks the version.
  - `Site::validate_cross_page_links_for(page_rel)` renders one page plus its link targets; the preview
    uses it instead of running the whole-site check and discarding the rest.
  - `.tali-nofx` (deck.css) + `CAM.morph`/`morphFade`/`morphFadeDelay` + a per-div `__mmSettle`.
  - `Kernel.cell_cap` replaces the `OnceLock`-memoized `cell_timeout()` read per execution.
- **Book-level `theorems:`** (was item 16 F-01; shipped 2026-07-23): a
  book-wide theorem-numbering policy in `_site.yml` (`theorems:`), inherited by any chapter with no
  `theorems:` block of its own and overridden wholesale by one that declares its own. `theorems` is now a
  recognized `_site.yml` key (`NATIVE_KEYS`), parsed into `SiteConfig.theorems: Option<TheoremConfig>` and
  value-validated via the shared `validate_theorem_values`. Render carries it through a new public
  `render_document_scoped_with_theorems(src, base, chapter, book_theorems)` (the merge:
  `theorem_config_with_fallback` in `fm_extract.rs` + a book-defaulted init in `render_internal_impl`, so a
  chapter that starts straight into `#` with no front-matter still inherits). Threaded through EVERY site
  render path: core `Site::render_page`/discovery/`llms`/`search::page_fragment` AND the server's site build
  (`build.rs`) + live preview + per-page search refresh (`serve_site`, the paths that actually bypass
  `Site::render_page`). `TheoremConfig` is now a public opaque type; the `_site.yml` schema gained a shared
  `theorems_schema()`. Pinned by `corpus/theorem-book/` + `crates/core/tests/book_theorems.rs` (alpha
  inherits `numbered:false` -> empty number span; beta overrides -> "Theorem 2.1") + render/config unit
  tests; existing books (no `_site.yml theorems:`) render byte-identically (the `None` path is inert, no
  snapshot churn). Whole-config override, not per-field (YAGNI). Spec/plan:
  `docs/superpowers/{specs,plans}/2026-07-22-book-level-theorems*`.
- **Live-executor mounts (F-04 full fix)** (was item 16 F-04; shipped 2026-07-22): a mounted sub-project now serves through the **same live per-page path** as the root, so its
  `{python}`/`{r}` cells execute live in the host `preview` (not only in the static `build`). Engine is all
  in `serve_site/mod.rs`: `Project`/`MountPoint`/`ProjectKey` + pure `match_mount`/`resolve_project`/
  `classify_change` (unit-pinned) + **one `ExecPool` per project** (the frozen `exec_pool.rs` byte-unchanged,
  used once per project); a mount shares the warm pool only when its interpreter matches root, else cold-start.
  Each project owns its `_freeze` + websocket + hot-reload. Browser-verified on `/gallery/course/em.html`.
  Spec/plan: `docs/superpowers/{specs,plans}/2026-07-22-live-executor-mounts*`. Remaining (item 10, low): an
  automated live-HTTP serve test (the bin crate has no `reqwest`/`TcpListener` harness).
- **Structure-preserving, book-aware `read`** (was item 19; shipped + pushed 2026-07-22): the recurring
  cross-persona `read`-projection seam
  (folded items 16 F-02 + 17 F-03 + 18 F-01). Three pure arms in `render/text.rs::project_block`
  (`project_list` one line per `<li>` incl. ordered/nested; `project_steps` each `.scrolly`/`.step`
  narration its own paragraph; `project_inputs` `[input] label = value`), pinned by unit tests +
  `corpus/reader/text-projection.tmd` snapshot. Book-aware `read` in `query.rs`: `scoped_site_doc`
  auto-detects an enclosing `_site.yml` (walk-up, `.git`-bounded) and renders a page as the site does
  (`render_document_with_includes_scoped` + `Site::number_chapter` + `resolve_cross_refs`), so
  `@thm-elbo`→"Theorem 3.1", cross-page `@thm-consistency`→"Theorem 2.1"/"Chapter 2"; `read <dir>`
  projects a whole book (`===== rel (Chapter N) =====` headers, human + `--json`), `--run` on a dir
  rejected. Pinned by `crates/server/tests/read_book.rs`; `indexable_text` (Cmd-K) unchanged
  (arms live in `project_block`, which search doesn't call). Still open: item 16 F-03 (embed iframe-chrome
  leak in `read`) is a SEPARATE finding, NOT folded here.
- **2026-07-22 (late) backlog-clearing pass** (shipped 2026-07-22, on origin/main): **focus mode split from OS fullscreen**
  (`f` = calm column, `F`/menu = fullscreen; `03-focus-mode.js`); **Vite-user hint banner** (`log::keys_hint`,
  TTY-gated, points at the `◇` dev menu); **deck `footer:`/`logo:`** (`render::deck_overlay_html` +
  `DeckParts.deck_overlay`, corpus-pinned in `deck.tmd`); **per-book offline `<book>.zip`** (`server::zip`
  hand-rolled DEFLATE over the already-present flate2/crc32fast, topbar `<a download>` gated to the build via
  `page_chrome(downloads)`, `Site::archive_name`); **cross-page dup-label warning located** (`file:line:` at
  the redefining anchor via `content_lines_numbered`); **item 11 passes (b)-(e)** (see item 11). Owner
  rulings: DX16 skip, i18n defer, item-9 design-Qs documented (see "Decided against").
- **AP8-1 executed-output path scrub** (item 15, AP8) **shipped 2026-07-22** (branch
  `worktree-ap8-1-ipykernel-path-scrub`, now on origin/main): a cell's stream (matplotlib's Agg `UserWarning`, any
  `warnings.warn`, a `print(__file__)`) cited the kernel's per-process temp file
  `<tmpdir>/ipykernel_<PID>/<HASH>.py`, making builds non-reproducible + leaking a local absolute path into
  published HTML. Fix: a hand-rolled `scrub_kernel_paths` (no new dep, mirrors `strip_ansi`) normalizes that
  path — and the legacy `<ipython-input-…>` form — to a stable `<cell>` marker in the `Output::Stream` arm of
  `render_outputs` (`crates/server/src/kernel.rs`), before escaping; the `:<line>:` suffix is deterministic and
  kept. Language-agnostic (R warnings carry no such path — verified). Pinned by pure unit tests
  (`scrub_kernel_paths_normalizes_cell_source_paths`, `render_outputs_scrubs_nondeterministic_kernel_paths`) +
  a kernel-gated end-to-end `crates/server/tests/executed_output_reproducible.rs` (build the same warning doc
  twice under `TALIESIN_NO_CACHE=1` → byte-identical, no `ipykernel_` path); mutation-checked both ways.
  Completes item 15 alongside DET-1.
- **DET-1 reproducibility guard** (item 15, AP8) **shipped 2026-07-22** (branch
  `worktree-det1-determinism-guard`, now on origin/main): `crates/server/tests/build_reproducibility.rs` builds a
  feature-rich **kernel-free** site (listing + categories + Atom feed, cross-page `@thm-`/`@def-` xrefs,
  8 hover targets, site `url:` → sitemap + OG cards) twice in **separate processes at separate paths**
  (⇒ different HashMap seeds *and* `read_dir` order) and asserts **every** emitted file is byte-identical
  (not just `.html`, unlike `parallel_build_determinism.rs`), plus a non-vacuity test that the guarded
  aggregates (`search-index.js`/`hover-index.js`/`index.xml`/`sitemap.xml`/`llms.txt`/`og/*.png`) are
  populated. Mutation-checked: deleting the `entries.sort_by` in `Site::build_hover_index` diverges
  `hover-index.js` and fails it. Lands alongside AP8-1, completing item 15.
- **DX audit batch** DX1-DX15, DX18, DX19 shipped; **DX17(a)** shipped 2026-07-21 (below); **DX16 ruled
  skip** (Decided against); **DX17(b)** (headless `{js}`) **shipped 2026-07-22** — `read --run` drives a
  local headless Chrome (`chromiumoxide` 0.9, `default-features = false` so no fetcher/openssl; tokio
  1.52/edition-2024 clean) over the built page and projects each `{js}` cell's outcome. Pure
  `classify_js_node`/`JsOutcome` (`headless_js.rs`), core interleave `body_text_with_js`
  (`render/text.rs::project_with_js`), a `detail` field on `read --format json`'s cells (skip-if-none, so
  python/r JSON stays byte-identical), gated + optional (no Chrome → `[js: skipped (chrome unavailable)]`,
  exit 0), observation-only (no reactive re-run, no `{js}` freeze write). Pinned by
  `corpus/agent/executed-read-js.tmd` + the Chrome-gated `read_run_js.rs` (`TALIESIN_REQUIRE_CHROME`
  canary) + pure unit tests; `TALIESIN_JS_TIMEOUT` (default 10s) settle budget. **The whole DX audit is now
  complete.**
- **Editor DevX (VS Code companion) E1-E6 shipped 2026-07-21; E7 (`taliesin lsp`) shipped 2026-07-22 —
  the whole initiative is complete** (audit
  [2026-07-21-vscode-devx-audit.md](2026-07-21-vscode-devx-audit.md);
  spec/plan `docs/superpowers/specs|plans/2026-07-21-editor-devx-e3-e5.*`):
  E1 rich diagnostics + did-you-mean quick-fix; **E2** on-type diagnostics (`taliesin check --stdin`
  lints the piped buffer, not the saved file, skipping the interpreter probe; debounced
  `onDidChangeTextDocument`; pin `stdin_buffer_is_linted_instead_of_the_on_disk_file` + `debounce.ts`
  node:tests); **E3** column-accurate diagnostics (a `[col,end_col)` span on `Warning`/`Diagnostic`,
  serialized `skip_if_none` so un-columned JSON stays byte-identical; front-matter key typos get the span
  via `block_key_span`/`nested_key_span`, xref stays whole-line — it is HTML-derived, block-line only; the
  squiggle covers the token and the quick-fix uses `fixSpan` (exact span, no edit-distance guess); pins
  `frontmatter::tests::unknown_*_column_span`, `check_json_front_matter_typo_carries_a_column_span`,
  `check.test.ts` `fixSpan`); **E4** `HoverProvider` resolving `@xref`→label / front-matter key→doc /
  `[@key]`→BibTeX entry (pure `hover.ts` + shared `backend.ts`); **E5** document outline
  (`DocumentSymbolProvider` over a pure `outline.ts` heading scan) + go-to-definition
  (`DefinitionProvider`: `{{< include >}}`→file, `@xref`→same-doc def via `definitionSite`, `[@key]`→`.bib`
  via `bibEntryOffset`; buffer+filesystem, no backend; `outline.test.ts`/`definition.test.ts`); **E6**
  front-matter value completion (`vocab` `frontmatterValues` for `format`/`theme` + a `frontmatter-value`
  `detectContext` case).
- **E7 `taliesin lsp` (editor-agnostic language server over stdio) shipped 2026-07-22, all capabilities**
  (`crates/server/src/lsp.rs` + `lsp_nav.rs` + `lsp_outline.rs` + `lsp_complete.rs`, `lsp-server`/`lsp-types`;
  specs `docs/superpowers/specs/2026-07-2{1,2}-e7-lsp-*.md`). `textDocumentSync: FULL` + a `HashMap<Url,String>`
  store; **live diagnostics** (`check::buffer_diagnostics` → `to_lsp`); **definition** (`@xref`/`[@cite]`/`{{<
  include >}}` via `lsp_nav`); **documentSymbol** (heading outline via `lsp_outline::outline`); **hover** (xref
  label+number from a live-buffer render's `xref_numbers`, key docs + `.bib` entry via `bib_entry_text`);
  **completion** (7 cursor contexts via `lsp_complete::detect_context` + `vocab` + `render_buffer`);
  **codeAction** (one-click quick-fix from a diagnostic's precise `data.replacement`); **rename** + prepare
  (`lsp_nav::{anchor_at, anchor_occurrences}` rewrite an xref anchor's definition + all `@`-refs in one
  `WorkspaceEdit`, gated to `is_xref_anchor` ids). Porting the companion itself to `vscode-languageclient` is
  a separate, still-open, later item (not scoped here).
- **DX17(a) headless executed-output (python/r) shipped 2026-07-21:** `taliesin read --run` executes
  python/r via build's exec path and projects `[figure fig-x: produced, alt "…"]` / `[output: …]` /
  `[cell error: …]` (+ `--format json` per-cell). Core `classify_exec_output`; pinned by
  `corpus/agent/executed-read.tmd` + `read_run.rs`; AGENTS.md onramp documents it. Phase 2 (headless
  `{js}`) remains as item 1.
- **Click-to-source into `{{< include >}}`d files already works** (do not re-scope as "build it"): an
  Alt-click on included content already opens the *included* file at the correct line on both paths
  (plain-browser `vscode://`, and the VS Code webview via `qmd-goto`), because included blocks carry
  `data-source-file` from the `includes.rs` per-line source map and labels are kept primary-doc-relative.
  Pinned by `corpus.rs:161-219` (plus the "every `source_file` must be relative" invariant,
  `corpus.rs:124-137`) and the companion's `paths.test.ts:45-58`. **Only real gap:** `web-client/` has no
  JS tests at all, so `openSource()`'s include handling is proven by corpus attributes and inspection, not
  a JS assertion on the emitted `vscode://` URL or `qmd-goto` payload (a small P3 hardening add if wanted).
- **Deck audit** fully shipped; **B3-18** (the last item) landed 2026-07-21: a structural deck edit now
  re-mounts only the edited `<section>`s (client-side signature-keyed reconcile in `client.js`), so
  untouched slides keep their live `{js}`/WebGL/input state. Prerequisite fix: `{{< input >}}` control
  ids are name-based (`qin-<name>`), not line-based, so an input block's `data-block-id` is
  position-independent (`render/extension/mod.rs`).
- **Polish audit batch** PL1-PL20 all shipped (`git log --oneline origin/main | grep PL`).
- **PMF builds** B1 (reader "Cite this" box), B2 (book landing-page auto-TOC), B4 (deck Marginalia
  identity) shipped. B5 Zenodo DOI is demand-driven (above).
- **Corpus-coverage** C1-C7 pinned; only C5's `serve_site` mount serve-path remains (in P3 above). C3/C4
  done, C6 was never a gap.
- **Machine-facing audit** M1, M2, M3-M5, M6b shipped; **M6a is frozen**, M2's hanging-interpreter
  sibling + the M4 stand-in flake remain (gated, above).
- **AI-native packaging + guardrails** (the former Medium #2) fully shipped: `taliesin map --format json`
  (`map_cli.rs`), the citation-wired `paper` scaffold + `--json` on `new`/`init` (`corpus/scaffold/`),
  `build`/`publish` `--format json` (`structured_build_errors.rs`), the default-on placeholder-alt nudge
  (`diagnostics/a11y.rs::placeholder_alt_message`), and the distributable Claude Code skill
  (`editor/claude-code/skills/taliesin`, drift-locked by `skill_freshness.rs`).
- **R/Python stream ANSI leak fixed 2026-07-21** (the former #6): `render_outputs`' `Output::Stream` arm
  now `strip_ansi`s before escaping, matching the error arm, so R `message()`/`warning()` (and Python
  coloured stderr) no longer leak `[31m…[0m` into the page (`kernel.rs`; pinned by
  `render_outputs_strips_ansi_from_streams`, verified end-to-end against a real R kernel).
- **Live defects** §2 #1 Part A, #2, #4-#10 shipped; only Part B (P3) + #3 i18n (low) remain (above).
- **Reduction/modularity** Phase 2 + T1 + R2 (scanner unification) shipped; the codebase is already lean.
- **Ungraceful-death reaping** warm-pool forkserver + cold-Python kernel + stale-`/tmp` sweep shipped;
  only the R cold-kernel residual remains (in P3 above).
- **`assets/js/*` `tsc`/`@ts-check`** at strict-zero, CI-gated. **Interpreter selection signal +
  project-local `python:`/`r:` `_site.yml` fields** shipped. **OG-card coverage** (book chapters + decks)
  shipped.

## Decided against / do-not-re-litigate

- **Flipping a book chapter's label to prefer `title:` over its `# H1`** (raised + resolved
  2026-07-25 while building 23's Ship A). The symptom is real — a chapter's drawer / Contents /
  Cmd-K label can differ from its `<title>` — but **measured across every book in the repo only 3
  of 48 chapters differ, and in 2 of them the `# H1` is the BETTER nav label** (`docs/guide`'s
  preface is `title: Taliesin` opening `# Why Taliesin`; `docs/internals`' is `title: Taliesin
  Internals` opening `# How Taliesin works`). Flipping the precedence would relabel those to the
  duller name to fix a divergence the author created on purpose. **The evidence I first filed was
  overstated**: the "all these surfaces agree" claim I cited is a comment on a *website*-page test
  and is true in the case it describes; a book chapter simply has a nav label distinct from its
  page title. Resolved as documentation (a note in `docs/internals/sites.tmd` + a corrected comment
  at `site/mod.rs`'s website-title test), not code. Nothing is searchable-only-by-one-name: the
  page record's body carries the rendered title block, so both names find the page in Cmd-K.

- **CAD-as-code (`{openscad}` / CadQuery cell → live 3-D preview): researched 2026-07-23, NOT built**
  (detail: [2026-07-23-cad-as-code-research.md](2026-07-23-cad-as-code-research.md); two background research
  passes, feasibility + market). Technically **feasible and a clean fit** (user-installed `openscad`
  subprocess → STL → bundled MIT three.js, the same shape as the shipped `graphics3d` viewer) and
  commercially **legally green** (an arm's-length CLI call is FSF "mere aggregation"; the models are the
  user's own). Killed on **demand**: wrong audience, tiny niche, and the peer group (Quarto, Jupyter Book,
  mdBook) ships nothing like it with zero requests for it. **Do not bundle openscad-wasm (GPL).** Five
  named revisit triggers, any one of which reopens it: (1) *author-pull* — you actually want to write a
  `.tmd` that is better with a live parametric model (a 3-D-printing build log, a mechanism tutorial); under
  corpus-plus-roadmap that alone is sufficient, just name the pin doc; (2) the peer group ships embedded
  CAD; (3) notebook-CAD usage multiplies materially; (4) text-to-CAD becomes reliable *and* moves
  in-document; (5) a concrete external ask (course, client, grant scope). The implementation path is
  pre-decided in the doc so a revival needs no re-research.
- **2026-07-22 rulings** (owner, this session): **DX16 update-nudge = SKIP** — a version check is network
  egress that undercuts the offline-first identity; drop it (was item 7). **Cross-ref labels i18n = DEFER** —
  no corpus doc demands it and full i18n is a real scope question; minimal-config says don't add speculative
  config (was item 8; revive with a corpus pin + a real ask). **Item 9 design-Qs documented as-is** (owner
  chose only the Vite banner, which shipped): the deck serif/sans inversion (`deck.css`), no `//| uses:`
  alias (vocab sprawl), and the callout-namespaced/theorem-bare asymmetry all stay as intentional, not
  bugs. **Deck presenter tools** (one-command publish / laser-spotlight / auto-advance) considered and NOT
  selected — revive on a real speaker ask.
- **2026-07-12 wishlist cut to `FEATURE-IDEAS.md`** (revive only when a corpus doc needs one):
  cross-revision diff, repro manifest, List-of-Figures/Tables/Theorems, interactive tables, line-level
  code xrefs, image `dark=`. Reader text-size/line-spacing controls declined (a11y-exempt substrate in
  `14-reader-prefs.js`). Stale `new-post`/`new-project` scaffolder skills retired (the `deploy` skill
  stays).
- **TODO / FIXME surfacing skipped** (owner ruled 2026-07-10): no `level` concept exists, so a TODO
  warning would fail `check` on every draft. If revived, design A (preview-only `Diagnostic::info` at
  `serve/mod.rs::compute_diagnostics`) beats re-plumbing a real `level`; the scan must NOT reuse
  `prose::strip_inline` (it blanks code, where TODOs live).
- **AI-native leftovers declined 2026-07-16:** `check --online` citation resolution (the only proposed
  network egress; buys a link-rot check at the cost of the offline invariant; if ever revived, check-only,
  off by default, never reachable from `build`/`publish`); numeric/quoted-claim-without-citation hint
  (its own spec rates it FP-prone); per-page text/JSON sidecar (redundant, `taliesin read` +
  `llms.txt`/`llms-full.txt` ship).
- **Refuted by measurement (do NOT re-scope):** **heading-demotion (AP9's HTML-1 / former item 14) already
  ships** (`7e60f6c`, 2026-07-12): a titled HTML doc demotes every body heading one level under its
  `<h1 class="title">` (`demote_heading_html`, gated Html+titled+`!hide_title_block`, decks/books excluded by
  construction), so a fresh render/build of a titled page emits exactly one `<h1>`. AP9's "12 `<h1>`" measured
  a stale gitignored `corpus/bayesian-website/_site/index.html` (a pre-fix build artifact). The only corpus
  docs with multiple `<h1>` are decks (`deck.tmd`/`deck-marginalia.tmd`/`embed/talk.tmd`), which are exempt by
  design. `build` does not leak forkserver subtrees (the graceful
  path is reaped; the *ungraceful* R residual is the only gap, above); the warm pool booting Python on
  prose-only builds is hygiene, not latency; dev attributes are 0.29% of page bytes (don't strip); a
  `--version -dirty` marker is stale-by-construction (refused); the `assets/css` stale-embed claim did
  not reproduce (re-verify for `assets/js` before any touch-render workaround); the 390px `hero:`
  overflow + theme/video desync are already fixed; include symlink-loop SIGABRT does not exist (Linux
  caps at `MAXSYMLINKS=40`).
- **`_redirects`/`_headers` preserved, never generated** (`build.rs:1881` treats them as author-placed
  deploy metadata; `stale_sweep.rs` pins it). Auto-generating them is a "perfect the default vs add a
  knob" call; leave as-is unless a real deploy proves it needs one.
- **Gate the gate:** a drift test that cannot fail is worse than none. Any new drift gate must be
  mutation-checked against exactly the shape it guards.
- **Library outsourcing decided against** (each verified vs the invariants): hayagriva/biblatex,
  schemars, jsonschema, morphdom/idiomorph, similar/dissimilar, clap, owo-colors, slug, html-escape,
  lightningcss/palette, IntersectionObserver/scrollspy libs, deck micro-helpers. Keep `two_face` extras
  filling gaps only (the bundled syntect set is consulted first and must win).
- **Reading-first defaults, research-validated keeps** (do NOT "fix"): serif body for long-form screen
  reading; ~70ch measure `--tali-maxw: 46rem`; right-rail scrollspy + width-gated sidenotes; scroll (not
  pagination) book reading; ship REAL bold/italic faces, never synthesized.
- **2026-07-06 decisions:** book pager stays bottom-only; book page-TOC fix-in-place, keep both nav
  surfaces; xref graph tool removed; focus mode stays ephemeral; deck overview keeps per-slide
  backgrounds; dev-menu + `#tali-progress` + reading-progress bar stay three separate signals.
- **2026-07-18 PMF re-derivations:** the reader "Cite this" box (D70) was REVIVED and shipped as B1; the
  deck desktop "async handout" reading view stays CUT (do not re-open without a fresh ruling).

## Product / distribution

Resolved (2026-06-20): ship as **open source + personal tool**, no company for now (optionality kept:
sole copyright + trademarkable name; `STARTUP-PLAN.md`). Per the PMF audit (2026-07-18) the tool is
feature-complete for ~one real user, so the highest-leverage next move is **real users**, not more
features; the owner is publishing soon to gather feedback. When publishing, lead the copy with the
**speed moat** (warm server, block-level incremental, no per-edit rebuild), the single most-repeated
Quarto grievance and the most under-marketed asset.
