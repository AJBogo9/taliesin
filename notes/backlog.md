# Taliesin backlog

Single-author Rust `.tmd` to **HTML-only** dev server. **Scope: corpus-plus-roadmap** ("done" = the
docs under `corpus/` render correctly; each new capability ships pinned by a target corpus doc).
Roadmap: [ROADMAP.md](ROADMAP.md).

> **Only open tasks live here.** Completed work lives in git + [AUDITS.md](AUDITS.md) +
> [ROADMAP.md](ROADMAP.md); delete an item when it lands, don't leave a `[x]`. The "already shipped"
> list near the bottom is the compact anti-rot guard (do not re-add / re-scope), not a changelog.

## State (2026-07-25, latest: the AP7 accessibility audit, findings only, no code)

**Branch `backlog/backlink-context-and-resume`, notes-only commit on top of the 13 unpushed
commits below.** The owner ruling that sent the previous session to an audit was discharged: **AP7
(deep accessibility) ran**, produced
[2026-07-25-ap7-accessibility-audit.md](2026-07-25-ap7-accessibility-audit.md), a row in
[AUDITS.md](AUDITS.md)'s round index, and **item 34** in band C. **No source file was touched** (the
ruling forbade picking a feature); the tree was verified clean after the run.

**What the round found:** the rendered *document* is sound, the rendered *application* is not. Every
static one-shot surface came back healthy (several better than AP7's own entry claimed), and every
defect is one shape: **content that changes without the reader operating a control is never
announced.** The one exception, and the only reader-facing defect, is **AP7-1: 37 of 51 book pages
emit a skipped heading level while `check` prints "no problems found"**, from two independent causes
(absolute `+1` demotion; and `heading_level` needing the block html to *start with* `<hN`, which the
`<header>`-wrapped title block does not, so the page's only `<h1>` never sets `prev`).

**Four things this round got that the entry did not say:**
- **A fourth premise was refuted, this one mine.** Off-screen deck slides are **not** leaked to AT:
  they carry `inert`, so the full a11y tree holds 3 slide nodes, all "Slide 1 of 19". Checked
  because the first probe *looked* like a carousel bug (`aria-hidden` absent on 18 of 18 slides):
  `aria-hidden` was simply the wrong thing to count.
- **The audit's most valuable output was nearly a wrong headline.** "34 invisible focus stops on a
  chapter" was an artifact of reading `getComputedStyle().opacity` immediately after `Tab`:
  `.tali-copy`/`.tali-anchor` are `opacity:0` + `:focus-visible{opacity:1}` **plus a
  `transition: opacity var(--tali-dur)`**, and `--tali-dur` is `.12s`, so a synchronous read returns
  the interpolated value. Settle past `--tali-dur` before judging any visibility/opacity/transform.
  The real count is **0**. Two more of the round's own false leads (a regex matching inside
  `data-block-id`; `interestingOnly:true` pruning 17 headings to 1) are in the doc.
- **The chrome-devtools MCP was unusable**: a parallel session held
  `~/.cache/chrome-devtools-mcp/chrome-profile`. The fallback that works is the project's own
  `puppeteer-core` (`tools/ui-audit/node_modules`) with a private `userDataDir`. Assume AP6 needs
  the same.
- **`AUDITS.md`'s "six rounds have no ledger line" gap is already closed**: that file now opens
  with a complete round-index table. Two places in this file still claimed otherwise; both fixed.

### Prior state (2026-07-25, earlier: the backlink-context + resume batch)

**Branch `backlog/backlink-context-and-resume`, 6 commits, NOT PUSHED** (off
`backlog/book-outline-drawer`, which is itself 5 commits off `origin/main` at `994bcba` and
also unpushed — check `git log --oneline origin/main..HEAD` before believing either). Cleared
**all three remaining buildable bullets of item 24's independent-medium set** (the
citing-sentence backlink line, the link-text collision lint, book-scoped resume) and **item
16's F-03**, which empties item 16. Gates at landing, re-run not trusted from this file:
**1481 tests / 0 fail across 88 binaries** with all three gates and `--test-threads=1`;
`cargo fmt --check`, `clippy --workspace --all-targets -D warnings` (0 warnings) and both JS
`tsc` gates clean; `check` reports no problems on `corpus/tarn`, `docs/guide`,
`docs/internals` and `site`. Every fix is mutation-verified (10 mutations, all caught); the
client work is browser-verified against real release builds of `corpus/tarn` **and** the
dogfood `docs/guide` at 1440x900 / 390x844 / 900x1440 in light and dark, 0 console errors.

**What this batch shipped**
- **The citing-sentence backlink line.** "Referenced by Results" now carries the sentence
  the reference is made in, quoted beside the link (never inside it: the sentence contains
  the reference's own `<a>`). Dropped wholesale past two referrers.
- **`TAL-LINK-TEXT`**, the lint half: two links on a page with the same accessible name and
  different destinations, compared **modulo fragment**. `suggestion` severity.
- **Book-scoped resume.** A "Continue reading → 3 Executable content" pill on a book's
  landing page, keyed to `data-tali-book` (the landing href), inert in the build.
- **Item 16 F-03**, both halves of the lossy `read` projection.

**Six things this batch got that the entries did not say:**
- **The needle trap is systemic, and it bit three times.** Any new class, attribute or
  label name is ALSO shipped inside the *inlined* CSS/JS payload of every page, so a
  whole-page `contains("data-tali-continue")` / `contains("tali-backref-")` /
  `contains("Continue reading")` is satisfied by a page that renders none of it. Two
  existing tests broke on this and one new one passed for the wrong reason. **Needle the
  full emitted tag**, or scope the read to the block. The pre-existing comment in
  `xref_backlinks.rs` warned about exactly this for `tali-backrefs` and it still recurred.
- **F-03's recorded cause was wrong, and fixing it exposed a wider bug.** The entry said a
  walkthrough's "steps + code concatenate"; measured, the code was dropped **entirely**.
  Restoring it revealed that `wrap_code_lines` drops the `\n` (each line is a
  block-displayed span, so the element *is* the break), so **any** line-wrapped block — a
  walkthrough or a magic-move deck slide — welded into one line under `decode_code`. Fixed
  once, for both callers. And the substitution has to happen before **one** strip pass:
  `strip_tags` trims, so stripping per line ate the indentation.
- **The audit's `data-footnote-ref`/`-backref` exemption would have been dead code.** This
  project emits `role="doc-noteref"` and `class="tali-fn-back"`, not comrak's attributes.
  Back-references are already silent for a better reason (they all point at *bare*
  fragments, so the modulo-fragment rule covers them). Cross-references ARE exempted, for a
  reason the audit did not give: an *unnumbered* theorem renders every reference to it as a
  bare "Theorem", which collides on text the author cannot reword.
- **`TAL-LINK-TEXT` fires ZERO times on the whole corpus** — in this form *and* in the naive
  whole-href form, so the audit's "4 findings on the dogfood books" does not reproduce at
  block scope (it presumably measured built pages, i.e. chrome). It is a latent guard, which
  is why `corpus/diagnostics/link-text.tmd` carries the near-misses **in the same document**:
  lose the fragment trim, the same-destination check or the footnote silence and it reports
  2+ instead of 1.
- **Position had to be measured, not chosen.** The Continue pill first shipped inside the
  landing Contents nav — which put it **3,120 px down a 4,109 px `docs/guide` page**, three
  viewports below the fold, for an affordance whose entire job is "get me back in". Moved to
  its own block after the title block: 164 px. The pin asserts *order*, not just presence.
- **`skim::plain` was asymmetric**, found by reading real built output. It stripped the space
  `indexable_text` leaves *before* closing punctuation but not *after* an opening bracket, so
  `(@sec-install)` projected as `( Chapter 1)` — 1 occurrence in `corpus/tarn` and **7 in
  `docs/guide`'s own `skim` output**, now 0. The backlink line reuses `plain` rather than
  minting a second extractor, which is how it surfaced.

**One process failure worth not repeating:** `git checkout -- <file>` was used twice to undo
a mutation on an **uncommitted** file, which restores from HEAD and destroyed the working
implementation both times. Commit before mutation-testing, then `git stash`; or patch and
restore from a copy.

### Prior state (2026-07-25, earlier: the book-wayfinding batch)

**Branch `backlog/book-outline-drawer`, 3 commits, NOT PUSHED** (off `origin/main` at `994bcba`).
Cleared **item 23 entirely** (Ship B was its last piece, so the item is gone) and **two of item
24's independent-medium bullets** (the preview/build TOC divergence, per-chapter prose length).
Gates at landing, re-run not trusted from this file: **1460 tests / 0 fail across 88 binaries**
with all three gates and `--test-threads=1`; `cargo fmt --check`,
`clippy --workspace --all-targets -D warnings` (0 warnings) and both JS `tsc` gates clean;
`check` reports no problems on `corpus/tarn`, `docs/guide`, `docs/internals` and `site`. Every
fix is mutation-verified; the client work is browser-verified against a real build, a real
mounted book in a live preview and a real live re-render, at 1440x900 / 390x844 / 900x1440 in
light and dark, 0 console errors.

**What this batch shipped**
- **23 Ship B, re-scoped on measurement.** The Chapters drawer expands each chapter into its
  own numbered section outline (active chapter open, indented by depth, parts get no expander).
  The audit specced a **second per-page build artifact**; that was declined after measuring —
  see the correction below.
- **The preview/build TOC divergence** (24's independent set). The preview rebuilt `#TOC` from
  `h1,h2,h3` by absolute tag while the build takes two levels below the *shallowest heading
  present*, so any page with a title block lost its third level in the preview only.
- **Per-chapter prose length** in the drawer and the landing Contents, from one shared
  `words_label`. Words, never a time; absolute units, never a bar.

**Five things this batch got that the entries did not say:**
- **Ship B's second artifact does not pay for itself.** Measured: the search index is 172 KB
  raw / 60 KB gzipped on `docs/internals` (146 KB / 50 KB on `docs/guide`), it is already
  lazy-loaded on every page via `TALIESIN_SEARCH_URL`, and Cmd-K pulls it anyway. An
  outline-only sidecar saves ~55 KB gzipped in exchange for a second copy of
  `search::page_fragment`'s render-then-number-then-resolve recipe, a second whole-project
  assembly, a second `refresh_*_for_page`, a second serve route and a second build write. The
  drawer reads the same index through the same loader (`search.js` now exports
  `window.taliLoadSearchIndex`). **Do not re-cost the sidecar; it is decided against, not
  deferred.**
- **`drawer-typeahead` is decided against with it**, not deferred: Cmd-K plus a collapsible
  outline covers it, and the audit itself flagged a second search-like box beside a Search
  button as a discoverability smell.
- **The `tsc` gate reads an EXPLICIT include list, and `18-media.js` was outside it** —
  shipping type-unchecked since it landed while the gate reported success. Fixed, plus
  `every_code_enhance_fragment_is_in_the_type_check_gate`, which reads the fragment directory
  mechanically. Same lesson as the CLI-help gate: compare the lists, don't assert one per file.
- **`skim`/`map`'s `words` violated `word_count`'s documented contract.** It counts the RAW
  source (`render_finished` expands internally but returns the raw text), so an include-built
  chapter reported **1 word where a reader reads 9**. Fixed alongside, because otherwise the
  new nav count would have disagreed with what an agent reads.
- **Two of the new pins were vacuous and mutation caught both.** No book in the repo has an
  include-built chapter at all, so the cross-surface skim pin passed with the fix deleted (that
  shape is now minted in a temp-dir test). And the drawer-vs-Contents pin compared the drawer
  **with itself**: the landing page carries both surfaces, so a whole-page search found the
  drawer's span on each side. Scope every read to the surface it names.

### Prior state (2026-07-25, earlier: the P3 residual batch)

**Branch `backlog/p3-residual-batch`, 6 commits, PUSHED** (fast-forwarded onto `origin/main`,
`4c97071..8aad995`; re-verified immediately before the push, and the pre-push hook passed).
Cleared **item 32** (gone), **item 17's
F-04**, and **three of item 11's four remaining bullets** (tokens incl. PA-C5, a11y interaction
B3/B5/B14/B15, CLI docs CLI1/2/3) plus **PA-B9** as an adjacent find. Item 11 is down to its
Semantics bullet. Gates at landing, re-run not trusted from this file: **1450 tests / 0 fail across
88 binaries** with all three gates and `--test-threads=1`; `cargo fmt --check`,
`clippy --workspace --all-targets -D warnings` (0 warnings) and both JS `tsc` gates clean; `check`
reports no problems on `corpus/tarn`, `docs/guide`, `docs/internals` and `site`. Every fix is
mutation-verified; the client work is browser-verified against a real build, a real single-doc
preview and a real deck, 0 console errors.

**Four things this batch got that the entries did not say** (the "entries rot" law, again):
- **Three of the four token bullets had already shipped** before the entry listing them was
  written. Grep the token before scheduling the work.
- **The generic CLI gate found 9 undocumented flags where the audit filed 2**, and the drift ran
  in BOTH directions: `preview`'s hand-written usage line advertised a `--port` its help omitted,
  while `check`'s dropped five flags its help documented. One assertion per flag is what let that
  sit; read the parser's own `*_FLAGS` const and compare the lists mechanically.
- **Item 32's recorded cause was wrong.** It blamed `roomy` for computing false when the map
  "measurably fits". `roomy` reads the deck STAGE, which is letterboxed to 16:9 — at a 1100x1000
  window the stage is 1100x619, so a 626 px map genuinely does not fit. The audit measured the
  window. The real defect was the fallback: "follow the current tile" centres row 0 and wastes half
  the stage above it, and `clampOv` permitted it because it never looked at the stage or the scale.
- **Three of six a11y pins were vacuous and mutation caught all three.** A bare
  `contains("focusout")` on the concatenated enhancer bundle passes with the fix deleted (three
  other fragments listen for it); `contains("taliFocusTrap")` passes with the call deleted (the
  feature-detect guard names it). Needle the element-scoped registration AND the handler body.

**Two gaps found and NOT closed, recorded so they are not rediscovered as bugs:**
- **No corpus page renders a "Cite this" box at all.** It needs title + date + author and nothing
  in the corpus sets an author, so that whole surface is unit-tested only. PA-B5 was verified
  against a throwaway site built with an `author:`.
- **A site preview emits no mobile-TOC sheet chrome**, so `client.js`'s copy of the sheet is
  reachable only in a single-doc preview. Worth knowing before testing anything about that sheet.

### Prior state (2026-07-25, earlier still: SKIM-2 sessions 2-3 + SKIM-3a + deck-motion (5))

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
2026-07-22, so a future session could have re-run a done round. **That notes-hygiene gap is since
CLOSED** (confirmed 2026-07-25 while adding AP7's row): `AUDITS.md` now opens with a complete
round-index table covering every dated findings doc, including the six rounds that had none (AP2,
AP4, the 2026-07-17 security audit, the 2026-07-24 deck-motion audit, the CAD research, the
companion version-skew bug).

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

> ### That audit ruling is DISCHARGED: AP7 ran 2026-07-25.
>
> The owner ruled the previous session to an audit and picked **AP7 (deep accessibility)**. It
> ran: findings in
> [2026-07-25-ap7-accessibility-audit.md](2026-07-25-ap7-accessibility-audit.md), ledger line
> added, and the five findings are filed below as **item 34**. Nothing was implemented (the
> ruling forbade picking a feature), so **item 34 is now the band-C work that band-C did not
> have**. AP7-1 in particular is a real defect on 37 of 51 book pages.
>
> The ruling covered that one session only and is **not** a standing preference for audits over
> features. AP3 (concurrency), AP6 (cross-browser) and AP11 (chaos) remain the unrun
> perspectives, all stateful/solo and all unranked; there is no ruling that the next session
> must take one.
>
> **Read AP7-1 before picking it up:** its two causes pull in opposite directions, and the
> cheap half (making `check` *see* the defect) turns 37 currently-green pages red, which is a
> visible change, not a silent fix.

**State: TWO batches are stacked and NEITHER is pushed.** `backlog/book-outline-drawer`
(5 commits off `origin/main` at `994bcba`) and `backlog/backlink-context-and-resume`
(6 more, branched off it). The P3 residual batch before them IS pushed. Before anything
else, check what is actually where: `git log --oneline origin/main..HEAD` and
`git branch -v`. **Do not trust a SHA written here** — the author pushes mid-session with no
signal in this file. The SKIM batch and the naming purge were both pushed earlier the same day.
**An audit session should branch off `backlog/backlink-context-and-resume`** (the newest tip),
not `origin/main`, or it will audit a tree two batches stale — and note that a findings doc is
the deliverable, so it will rarely need to branch at all.

**Owner ruling 2026-07-25: the Cmd-K empty state stays the whole-book outline.** It was the one change
in the batch that altered a daily-driver surface, and the one-line revert to the flat chapter
jump-menu was offered and declined. Do not re-litigate it; a collapsed-by-default variant was also
considered and is not wanted (it is not a one-liner: it needs collapse state + keyboard handling).

**Items 22 (SKIM-1), 23 (all three ships) and 33 (the naming purge) are gone; all shipped 2026-07-25.**
The purge is finished end to end and is now enforced by `crates/core/tests/retired_names.rs`, so
the retired brand cannot come back silently. **It is also PUSHED** (2026-07-25, 7 commits
fast-forwarded onto `origin/main`, `6bef1d7..7721432`), re-verified immediately before the push:
1426 tests / 0 fail across 88 binaries with the three gates + `--test-threads=1`;
`cargo fmt --check`, `clippy --workspace --all-targets -D warnings` (0 warnings) and both JS `tsc`
gates clean; `check` reports no problems on `corpus/tarn`, `docs/guide` and `docs/internals`.
One manual step is still owed by the author: the
in-editor click-to-source check (Task 8 Step 5 of its plan) — the companion was repackaged and
reinstalled, and the relay harness passes both directions, but nothing automated covers the real
editor round-trip.

**Four owner rulings were also taken** (see the prior-state block), which un-gated a large amount of
previously-blocked work. What is left now sorts into:

- **Build-ready TODAY: item 34 (AP7), filed 2026-07-25.** That band was empty, which is why the
  previous session was ruled to an audit; the audit refilled it. **AP7-1 is the pick** (a measured
  defect on 37 of 51 reader-facing book pages, unblocked, with both causes already re-derived from
  source). AP7-2/3/4/5 are smaller and each stands alone. 24's independent-medium set is still down
  to the **"Part, Chapter" ribbon**, an owner call rather than a task (see item 24). The other
  non-audit picks remain **item 30** (`corpus/analyst/`, writing not code) and the P3 residuals
  below, each of which carries its own blocker.
  - **23 is GONE, fully shipped 2026-07-25** (Ship B closed it). Two decisions in it are settled,
    not deferred: the **outline sidecar artifact is declined** (measured — the search index it
    would duplicate is 60 KB gzipped, already lazy-loaded on every page and already fetched by
    Cmd-K), and **`drawer-typeahead` is declined with it** (Cmd-K plus the collapsible drawer
    outline covers it). Do not re-cost either.
  - **24c shipped 2026-07-25** (see item 24). The standing lesson repeats one level further down:
    calibrating against real `skim`/`check` output killed **four** of that entry's own prescriptions,
    including its most valuable rule (`contentless` off the `skim` projection fired on 11.8% of the
    corpus, essentially all false positives) and one whose stated justification — a TOC-DROP residual
    on `cli.html` — does not exist in the tree at all. **Measure the rule against the corpus before
    writing it, not after.**
- **Writing, not code** — 30 (`corpus/analyst/`), the last un-probed persona. Diminishing returns
  are real: personas 1-3 found **0** interaction-bugs between them.
- **Needs a device or a demand signal** — 4 (deck mobile, needs a phone), 2 (deferred, revive on a
  real speaker ask), band D (the standing freeze), Tier 3 (waits on real users).
- **P3 residuals on secondary surfaces** — what is LEFT after the 2026-07-25 batches: **11**'s
  Semantics bullet only (the `<ul>`/`role=list` restructure, the image-alt lint nudge, the deck
  `theme-color`/OG residual), **12**, **17**'s F-01 (needs a vendoring decision, not a one-liner
  — see the correction below) and F-02 (WAI), **18**'s F-02/F-03, **29**'s T2. Items **32 and
  16 are gone** (16's last finding, F-03, shipped in the backlink-context batch).

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

**A second missing shape, measured 2026-07-25: NO book in the repo has an include-built chapter.**
Enumerated, not grepped — 9 files carry a real block `{{< include >}}` directive and exactly one of
them (`docs/guide/reference/shortcodes.tmd`) is a book chapter, where both directives sit inside
fenced code documenting the syntax. So any rule that reads a chapter's *source* (word counts,
`skim`, prose lints) passes vacuously over the whole corpus whether or not it expands includes,
and a bug there is invisible. `crates/core/src/site/skim.rs`'s tests now mint that shape in a temp
dir; anything else source-reading needs the same treatment (or a corpus fixture, which was NOT
added here — `corpus/tarn` is a documentation book and gaining a partial would muddy it).

**A third missing shape, measured 2026-07-25: no book in the corpus keeps a chapter in a
SUBDIRECTORY.** Enumerated, not grepped — all five corpus books (`demo-book`, `tarn`, `course`,
`theorem-book`, `scaffold-book`) are flat, while both dogfood books are nested and neither is in
the test net. So any depth-relative emission (`{up}`-prefixed hrefs, `../index.html`) is the empty
string everywhere the suite can see, and a pin over the corpus passes with the prefix deleted.
`crates/core/tests/book_landing_toc.rs` now mints that shape in a `TempProj`.

**The inlined-asset needle trap, which bit three times in one batch and is worth internalising:**
every page inlines the whole CSS + enhancer-JS payload into its `<head>`, so **any new class name,
`data-` attribute or user-facing string you add is present in the HTML of every page whether or not
that page renders the feature.** `html.contains("data-tali-continue")`,
`contains("tali-backref-")` and `contains("Continue reading")` were each satisfied by a page that
rendered none of it — two existing tests broke on this and one new test passed for the wrong
reason. Needle the **full emitted tag**, or slice the block out first. (`xref_backlinks.rs` already
carried a comment warning about exactly this, and it still recurred.)

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

- **The *audit perspectives* track** ("Audit perspectives" section below): proactive,
  findings-generating angles the prior rounds structurally could not see. **Done so far: AP1, AP2,
  AP4, AP5, AP7, AP8, AP9, AP10, AP12** (perf, fuzzing, cache-correctness, i18n/sourcepos, **a11y**,
  codebase health, determinism, semantic HTML, offline-proof). **Remaining: AP3 (concurrency), AP6
  (cross-browser), AP11 (chaos)**. All three are *stateful/solo* (server/kernel/browser), so run one
  when no parallel session owns that surface, and all three are unranked. Each is a fresh session
  that writes a dated findings doc and feeds build-ready items back here; the author has credits
  queued for exactly this.

Working method for an audit is in "Audit perspectives" (findings doc + `AUDITS.md` ledger line +
items filed back here), not the feature method. For features it is in "Standing constraints":
branch per feature, verify by mutation, browser-verify, ff-merge locally, delete the item here on
landing.

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

**That changed on 2026-07-25 when AP7 ran: band C now has buildable work again, and it is item 34.**
Before AP7 this list was genuinely empty of buildable, unruled tasks: A's single item is an owner
decision ruled deferred; both of B's need a device or a demand signal; and C was down to **24**'s
"Part, Chapter" ribbon (an owner call), item **30** (writing, not code), and P3 residuals that each
carry their own blocker (**17**'s F-01 needs a vendoring decision, **17**'s F-02 and **18**'s F-03
are WAI, **12** is demand-driven, **29**'s T2 is explicitly "only if you are already in there", and
**11**'s Semantics bullet needs a CSS-grid + filter-JS restructure). **Item 34's AP7-1 is the one
piece of unblocked, reader-facing, measured-defect work in the whole list**. Read its sequencing
note before starting, because its two causes pull in opposite directions.

### A. High impact (build first)

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

4. **Deck engine mobile polish** (P2): mobile pinch/pan + touch gestures (they matter for the phone-feed
   deck mode; hard to verify without a device); drop `fitSlide` from the resize path (needs a lazy
   fit-on-show refactor first). *(The desktop trackpad half shipped 2026-07-24 — pinch / ctrl+wheel-down
   opens the overview map, with a 250 ms hysteresis. What that left behind is all shipped or ruled —
   see "Deck-motion: the whole item is closed" under "Decided against", formerly item 28.)*

2. **Deck presenter tools** *(owner deferred 2026-07-22 — NOT selected this round)*: one-command deck
   publish (Share QR still encodes `localhost:PORT`), a presenter laser/spotlight, auto-advance. The
   `footer:`/`logo:` threading from this item **shipped** (see "Already shipped"); the presenter pieces
   were considered and left for later. Revive only on a real speaker ask.

### C. Low / hardening (P3)

34. **AP7 accessibility findings** (detail:
    [2026-07-25-ap7-accessibility-audit.md](2026-07-25-ap7-accessibility-audit.md)). Five findings; the
    doc also records what came back **sound** (deck `inert`, KaTeX MathML, tabsets, focus rings) and
    three false leads, so re-derive from it before doubting any of this. **AP7-1 is the only one that
    is a defect on shipped reader-facing pages.**
    - **AP7-1 (medium-high, S+M): 37 of 51 book pages emit a skipped heading level while
      `check` prints "no problems found".** Measured across `docs/guide` + `docs/internals` +
      `corpus/tarn`: 35 pages `h1→h3`, 2 pages `h1→h4`; `h2` is empty on essentially every chapter of
      both dogfood books. **Two independent causes, both re-derived from source:**
      (1) `render/mod.rs:2490 demote_heading_html` is an absolute `+1`, right for a `#`-rooted chapter
      and wrong for the `##`-rooted house style, while the build's TOC already windows relative to
      the *shallowest heading present*, so the two disagree; (2) `diagnostics/a11y.rs:211` starts
      `prev = 0` and `helpers.rs:47 heading_level` needs the block html to **start with** `<hN`, but
      the title block is `blocks[0]` as `<header class="tali-title-block">…<h1>` (`render/mod.rs:1133`),
      so the page's only `<h1>` is skipped and the largest jump on the page is never compared.
      **Sequencing matters:** fixing (2) alone is cheap but turns 37 green pages red rather than
      fixing them; fixing (1) changes emitted levels, which `site/chapter.rs` numbers *post*-demotion,
      so the relative-demotion fix and `ChapterNumbering`'s per-site base must move together or
      `@sec-` refs drift. Needs a minted pin: there is no `crates/core/tests/a11y*.rs`.
    - **AP7-2 (medium, S): the reactive `{js}` graph rewrites the document silently.** Keyboard-driving
      a `{{< input >}}` slider on built `corpus/reactive/inputs.tmd` changed six output regions
      (`k=3 n=20` → `k=8 n=20`) with **every** live region empty; no `.tali-js-out` carries `aria-live`
      or `role` (7 of 7), and `tali-js.js` has no `aria-live` at all. The control itself is correct
      (real `<label for>`, keyboard-operable); only the consequence is unannounced.
    - **AP7-3 (medium, M): `.scrolly` and `.code-walkthrough` carry no a11y semantics at all.**
      Measured: 0 focusable steps, 0 steps with `aria`/`role`, 0 live regions, `null` root role, for
      both. `scrolly.js`/`walkthrough.js` contain no `keydown`/`tabindex`/`role`/`aria`. The step
      prose reads fine linearly; what is never conveyed is the **stage** each step drives (no
      `aria-controls`/`aria-describedby`), and its state advances only as a consequence of visual
      scrolling. *The audit did not manage to drive a state transition headlessly (the known
      scroll-testing gotcha), so it reports the semantics, not the flip timing.*
    - **AP7-4 (low-medium, S): a preview block swap strands keyboard focus.** Measured against a
      live preview: focus **inside** the edited block → `<body>` (next Tab restarts at the top of the
      document); focus in an **unrelated** block survives, so the block-level diff is already doing
      its job. Nothing announced either way. `client.js:1276` `replaceWith` / `:1312` `remove` have no
      focus handling. **Preview-only** (a built page has no swap), so this costs an author who works
      keyboard-first or with AT, not a reader.
    - **AP7-5 (low, S): the in-page TOC is tab stop 56 of 62** on a chapter, after all 48 content
      stops, though it is a sticky sidebar visible the whole time. Screen-reader users are unaffected
      (`role="doc-toc"` is exposed as a landmark, verified in the full a11y tree); this lands on
      keyboard-only users not running AT. The skip link goes to `#tali-main` only.

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
    - ~~**`skim-shape-lints`, heavily trimmed (M).**~~ **SHIPPED 2026-07-25.**
      `crates/core/src/diagnostics/shape.rs`, five `TAL-SHAPE-*` codes, all `SUGGESTION` so advice
      never gates `build --strict` / `publish`: `-EMPTY` (unnamed heading), `-DUP` (two headings on a
      page reading the same), `-ECHO` (a *body* heading restating `title:`), `-HOLLOW` (a heading with
      neither text nor subsections), `-CAPTION` (a numbered float whose caption is empty or only
      `Figure 2:`). `heading_level`/`strip_tags` moved to `diagnostics/helpers.rs` rather than becoming
      a third copy. **Calibrated against real `taliesin skim` + `check` output over all 14 site
      projects (91 pages, 468 sections), which killed four of this entry's own prescriptions:**
      (1) **`contentless` off the `skim` projection is a false-positive factory** — it fired on
      **55 sections (11.8%)**, essentially all wrong, because `skim` reads the first `<p>` and a `<ul>`
      / fenced block / table / figure is not a `<p>` (`## Similar Projects` in
      `corpus/bayesian-website` is a bullet list). Rebuilt on the block model, where any non-heading
      block counts. (2) **`near-duplicate first-two-words` is a strict subset of exact-duplicate** on
      real data — all 5 of its hits were already `-DUP`, so it adds zero signal and was cut.
      (3) **TOC-DROP has nothing to catch and its justification is not reproducible**: the repo's
      deepest heading anywhere is `h4` (4 occurrences in 3 files), max section depth is 2, and
      `cli.tmd` is `h2`/`h3` only — there is no "`cli.html` residual". Cut. (4) **`title-echo` as
      specced fires on house style**: all 4 hits were landing pages whose leading heading restates the
      title, including both dogfood books, so the leading heading is exempt and only a *body* echo
      counts. NO-DESC stays cut (it needs a derived gist, else it degenerates). (5) **Decks invert
      every rule and are exempt wholesale** — found by running the lints over every `.tmd` in the tree
      rather than only the site projects: `corpus/deck.tmd:92`/`:96` deliberately repeat a slide title
      under `{auto-animate=true}` (the magic-move idiom), and a titleless slide is image-only while a
      title-only slide is a section divider. A deck has no TOC, so "this heading does not earn its TOC
      row" is not a question that applies to it. **`-HOLLOW` was
      narrowed after measuring**: the broad "next thing is another heading" form fired 13 times and
      every one was an ordinary grouping parent, so a heading followed by *deeper* headings is exempt —
      demanding an intro paragraph there is a style opinion, not a defect. **Net: 5 fires across the
      whole corpus, all true positives, all `-DUP` in `corpus/bayesian-website`**; the other four rules
      are latent guards, which is exactly why the fixture pin below is load-bearing. `classify` needed
      the shape rows placed **first** in `TABLE` (ahead of the prose rows): every shape message quotes
      the author's own heading back, so a section titled "Math" or "Bibliography" would otherwise
      classify as TAL-MATH / TAL-CITE-BIB — pinned by
      `a_shape_diagnostic_outranks_a_needle_inside_the_authors_own_heading`.
      **The "extend `check_cli.rs` with the three-state cases" pin was already satisfied** by 24a
      (`strict_gates_on_advice_and_errors_only_hides_it` over `corpus/diagnostics/prose.tmd`); what was
      genuinely missing, and is now added, is a suggestion-only doc that does not have to opt into
      `prose-lint:`. **Cut RUN, DENSITY, EMPHASIS, FANOUT, SKELETON, FORWARD:** measured against the
      corpus none has a defensible threshold (the flagship RUN rule fires on exactly **one** of 36
      dogfood pages and that one is a false positive; the headline "1,832-word run" is 1,021 words of
      table cells). Nothing resembling a readability grade, and never a rule about heading *form*
      (Sanchez/Lorch: no differential effect).
    - **Independent medium items.** Four of the five SHIPPED 2026-07-25: ~~per-chapter prose
      length~~, ~~the preview/build TOC selector divergence~~, ~~the citing-sentence backlink
      line~~ and ~~book-scoped resume~~ (the last two in the backlink-context batch — see the
      State block; the link-text collision lint shipped with them as `TAL-LINK-TEXT`).
      **Only one is left, and it is not a task:** a static "Part, Chapter" ribbon
      (`book-breadcrumb`), which is an **owner call** — it adds a fourth persistent top
      element, and the dwell-time evidence says the first viewport is the screening surface.
      The audit itself downgraded it to "cheap and mildly orienting" and notes it must be
      argued as a reversal of D114's "no breadcrumbs", not as an unexamined gap.
      *Facts from shipping the other four, so they are not re-learned:* the TOC entry's
      recorded cause was **right** for once (absolute tag vs. relative window, and `base` was
      indeed already correct) — but there is no equivalence test between the two
      implementations and one cannot be written cheaply, because `buildToc` closes over
      `root`/`tocEl` inside client.js's single IIFE; it is pinned by a needle pair instead.
      The prose-length entry did not mention that `skim`/`map`'s `words` **violated
      `word_count`'s include-expanded contract** (1 word reported where a reader reads 9).
      The backlink sentence had to be harvested in a **second pass over blocks
      `harvest_xref_numbers` already rendered**, because the xref registry is not final until
      that loop ends and an unresolved marker reads "in Section (in particular…)" — retaining
      a handful of block strings per page is what buys the ordering without a second
      whole-site render. And **`book-resume`'s record deliberately does not copy the block id
      or the scroll fraction**: `tali-pos:<path>` already holds those, the pill's job is only
      to reach the right chapter, and two records for one position is how they drift.
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
    - ~~**F-04 (friction, P3).**~~ **SHIPPED 2026-07-25.** A single-file check now walks up for an
      enclosing `_site.yml` and accepts a link that resolves under its `mounts:` — asked only on a link
      already about to be reported broken, so the common path costs nothing. The mount test is
      `under_mount`, **lifted out of `resolve_link_warnings` rather than reimplemented**, so the
      standalone and site-aware checkers cannot drift on what a mount covers; same for the upward walk,
      which core now owns and `query.rs` uses instead of keeping a second copy. The exemption is exactly
      as wide as the mount, verified not assumed: a typo'd prefix, a sibling that is not a mount and an
      ordinary missing file all still error, and with no enclosing site nothing is exempt. Watch the
      mutant choice here — the obvious "treat `doc_dir` as the root" mutant is *equivalent* (no
      `_site.yml` there means no mounts either) and passes without proving anything.
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
    "Already shipped"). **The tokens, a11y-interaction and CLI-docs bullets all shipped 2026-07-25**
    (see below). One bullet is left:
    - **Semantics (M3/M13, H1):** `<ul>`/`role=list` (needs a CSS-grid + category-filter-JS restructure +
      browser verify), hero/card image-alt lint nudge, deck `theme-color`/OG (PA-H1 residual).
      Owner design-Qs (deck copy-button, card whole-`<a>`) are parked in the doc, not build-ready.
    - ~~**Tokens (F2/C5/F4/S3).**~~ **CLOSED 2026-07-25, and three of the four had already shipped**
      before this entry was written: `--tali-scrim` exists (PA-F2), the `.15s` transitions are gone
      (PA-S3, the one `1.15s` left is an animation duration) and the breakpoints are `40rem` (PA-F4).
      Only **PA-C5** was real; it is now drift-locked, in two places rather than the one filed. Both
      sets of hexes are literals *by necessity* — a per-slide background must force the OPPOSITE
      theme's ink (so `var(--tali-link)` is exactly wrong), and the pre-paint canvas map runs before
      any stylesheet parses — so the fix is a lock, not a token substitution. The token reader takes a
      selector as well as a name, because `tokens.css` defines `--tali-bg` twice (light root + sepia)
      and a first-match scan silently measures the wrong palette.
    - ~~**a11y interaction (B3/B5/B14/B15).**~~ **SHIPPED 2026-07-25**, browser-verified against a real
      build + a real deck, 0 console errors. **B14's gate is the design, not a shortcut:** Cmd-K is an
      aria-activedescendant combobox, so focus never leaves the input and with a query typed Home/End
      are the CARET's keys — the list binding is allowed only when the input is empty, where it takes
      nothing away and where the list is longest (the outline). **B15 must not dismiss on a null
      `relatedTarget`**: that is what a window blur delivers, so closing there drops the menu when the
      reader switches apps. **B3 needed both copies of the sheet** (`toc-sheet.js` and the preview's own
      in `client.js`) and a release on leaving sheet mode, or a widen leaves Tab trapped in the desktop
      sidebar. Two facts for whoever tests this next: the pull-up handle opens on a tap or Enter/Space
      and **ignores a synthetic `click()`**, and client.js's copy is reachable **only in a single-doc
      preview** — a site preview emits no sheet chrome at all. **PA-B9 went with it** (a deletion: the
      handle read "Conclusion (read)" because the sheet wrote the label from the TOC *link*, which
      carries toc-spy's sr-only suffix, while toc-spy already writes it from the heading).
      **Three of the six pins were vacuous and mutation caught all three** —
      `CODE_ENHANCE_JS.contains("focusout")` passes with the fix deleted (three other fragments listen
      for it) and `contains("taliFocusTrap")` passes with the call deleted (the feature-detect guard
      mentions it). Needle the element-scoped registration AND the body.
    - ~~**CLI docs (CLI1/2/3).**~~ **SHIPPED 2026-07-25.** Ten subcommands hand-wrote their
      missing-positional `usage:` line; all now derive it from their `--help` synopsis via
      `usage_line(cmd)`. `command_synopsis` no longer takes `lines().next()`: a wrapped synopsis
      continues on an indented line, and `check`'s `--require-kernel`/`--stdin`/`--explain` lived
      there, so `taliesin check` with no path advertised **two of its seven flags**. The generic gate
      (`every_parsed_flag_is_documented_in_its_subcommand_help`, which reads each parser's own
      `*_FLAGS` const out of the sources) found **nine** undocumented flags where the audit filed two:
      also `init --json`/`--format`/`--yes`, `new --format`, `publish --strict`. Each verified accepted
      before being documented — and `publish --strict` is **last-wins** with `--no-strict`, which is
      what its test pins, not "`--strict` wins" as the first draft of that help line said.

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

**That "six rounds have no ledger line" gap is CLOSED** (verified 2026-07-25 while adding AP7's
line): `AUDITS.md` now opens with a complete round-index table covering every dated findings doc in
`notes/`, including the six that were missing (AP2, AP4, the 2026-07-17 security audit, the
2026-07-24 deck-motion audit, the CAD research, the companion version-skew bug). Just add your own
row to that table the day you run a round.

**Every audit's first job is to falsify its own entry.** Both of the last two rounds to be picked up
found that the entry overstated or misnamed what it was pointing at (AP2's "the codebase is
unguarded" premise was already false; AP9's headline finding measured a stale build artifact), and
AP7's brief below now records three premises measured false before the round even started. Budget
the first hour for that, not for probes.

**Run one perspective per session** (context isolation + token budget: the author's stated preference).
The *stateful* ones (AP1, AP2, AP3, AP4, AP5, AP6, AP11) each build, fuzz, run the server, bind ports,
drive a browser, or spawn kernels, so they corrupt each other if run at once: keep them solo. Only the
pure code-read ones (AP9, AP10, AP12, and the read half of AP8) are safe to fan out together in one
Workflow. The recommended-first set (**AP1, AP2, AP4, AP5**) plus the one safe code-read pick (**AP10**) are
now all RUN (see their entries below), and **AP7 was run 2026-07-25** under the owner ruling that
picked it. **Everything remaining (AP3 concurrency, AP6 cross-browser, AP11 chaos) is
stateful/solo** (server/kernel/browser/ports), so each needs a session where no parallel work owns
that surface. All three are unranked. Note from AP7's run: the **chrome-devtools MCP was unusable
because a parallel session held its Chrome profile**, and the fallback that worked is the project's
own `puppeteer-core` under `tools/ui-audit/node_modules` with a private `userDataDir`. Assume any
browser-driving perspective (AP6 especially) may need the same.

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

- **AP7: Deep accessibility of the output. RUN 2026-07-25** (findings:
  [2026-07-25-ap7-accessibility-audit.md](2026-07-25-ap7-accessibility-audit.md); the five findings
  are Open-work item **34**). Result: **the rendered *document* is sound; the rendered *application*
  is not.** Every static one-shot surface came back healthy, and better than this entry claimed,
  while every defect is one shape: **content that changes without the reader operating a control is
  never announced**. AP7-1 (37 of 51 book pages emit a skipped heading level while `check` says "no
  problems found", from two independent causes), AP7-2 (the reactive `{js}` graph rewrites six
  regions silently), AP7-3 (`.scrolly` / `.code-walkthrough` carry zero aria/focusable steps/live
  regions), AP7-4 (a preview block swap strands focus to `<body>`), AP7-5 (the TOC is tab stop 56 of
  62). **Verified sound, do not re-audit:** non-current deck slides are `inert` (out of the a11y tree
  *and* the focus order, only 3 slide nodes in the full tree); KaTeX ships `<math>` with the visual
  twin `aria-hidden="true"`, so the "read twice" question is answered *no*; tabsets are the full APG
  pattern; the closed drawer is `display:none` so its 19 links are not phantom tab stops; the
  settings menu toggles `aria-expanded` and returns focus on Escape; and there are **0** invisible,
  zero-size or unnamed focus stops across all 62 stops on a chapter. **The round's own three false
  leads are recorded in the doc**: the most useful is that reading `getComputedStyle().opacity`
  right after `Tab` returns the value mid-`--tali-dur` transition, which manufactured a headline
  "34 invisible focus stops" that is really 0. **Not chased:** a real screen reader (this was
  Chrome's a11y tree + computed style + keyboard driving), contrast (a documented `check` non-goal),
  reduced-motion, and callouts/theorem boxes as driven widgets. *Was stateful/solo; run with the
  project's own `puppeteer-core` because a parallel session held the chrome-devtools MCP profile.*
  **Corpus note, still true:** `corpus/diagnostics/a11y.tmd` is the *lint* fixture, not a reader
  fixture, and there is no `crates/core/tests/a11y*.rs`, so any AP7 fix that deserves a pin needs one
  minted.
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

- **The 2026-07-25 backlink-context + resume batch** (branch `backlog/backlink-context-and-resume`;
  was three of 24's independent-medium bullets + item 16's F-03). Grep before doubting any of it:
  - **The citing sentence in the backlink line**: `Site::backlinks` is now
    `HashMap<String, Vec<Backref>>` (`Backref { url, context }`), harvested in a second pass
    inside `harvest_xref_numbers` over the marker-bearing blocks it already rendered, resolved
    through `xref::rewrite_cross_refs` so it quotes the number the referring page shows.
    `backlinks::citing_sentence` plants a NUL in the link's open tag and walks
    `skim::sentence_at` (which walks `skim::first_sentence`, so the backlink line and the skim
    projection cannot disagree on where a sentence ends). Rendered as a sibling
    `.tali-backref-cite` span, dropped past `MAX_CITED_REFERRERS` (2).
  - **`TAL-LINK-TEXT`** (`diagnostics::validate_link_text_collisions`), `SUGGESTION`, compared
    modulo `#fragment`, reusing a11y's nesting-aware `interactives` scan. Pinned by
    `corpus/diagnostics/link-text.tmd` (one fire, three near-misses in the same file).
  - **Book-scoped resume**: `data-tali-book` on the book sidebar nav (the landing href,
    relative per page), `tali-book:<resolved root>` in `localStorage`, and a
    `tali-book-continue` block emitted **after the title block** on the landing page,
    hydrated by `15-reading-progress.js` from the Contents list already on the page.
  - **`read` projection of `{{< embed >}}`** (`[embed <src>: <title>]`) and of a
    `.code-walkthrough` (its `.cw-code` fenced first, then `[lines N] …` per step);
    `decode_code` turns `tali-hl-ln` wrappers back into the newlines they replaced, which
    also un-welds a magic-move deck slide.
  - `skim::plain` now strips the tag-boundary space on **both** sides of punctuation.
- **The 2026-07-25 book-wayfinding batch** (branch `backlog/book-outline-drawer`; was 23's Ship B
  and two of 24's independent-medium bullets). Grep before doubting any of it:
  - The **chapter drawer's per-chapter section outline**: `code-enhance/19-book-outline.js` hydrates
    `#tali-book-chapters` from the shared `search-index.js` on drawer open (expander per chapter with
    sections, active chapter open, indent relative to each page's own shallowest heading, parts and
    section-less chapters get no expander). Nothing server-side, so JS-off is the flat list it was.
    `search.js` exports its loader as `window.taliLoadSearchIndex` — **one loader, two readers**.
  - The **preview's live `#TOC` uses the build's relative window** (`lvl(h) - base <= 2` over every
    anchored heading), not `h1,h2,h3` by tag. Pinned by
    `the_previews_toc_uses_the_same_relative_window_as_the_build`.
  - **Per-chapter prose length** in the drawer (`.tali-chap-words`) and the landing Contents
    (`.tali-btoc-words`), both from `site::book::words_label`; `BookEntry.words` is
    `prose::word_count` over the **include-expanded** source, and `skim`/`map` now expand too.
  - `every_code_enhance_fragment_is_in_the_type_check_gate` reads `assets/js/code-enhance/` and
    asserts each file is in `jsconfig.json`'s explicit include list (`18-media.js` was not).
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
  (arms live in `project_block`, which search doesn't call). Item 16 F-03 (the embed iframe-chrome
  leak + the walkthrough) was a SEPARATE finding, not folded here; it **shipped 2026-07-25** in the
  backlink-context batch.
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

- **Deck-motion: the whole item is closed** (was Open-work item 28, lifted out 2026-07-25 because it
  had no code left in it and "only open tasks live here"; detail:
  [2026-07-24-deck-motion-audit.md](2026-07-24-deck-motion-audit.md)). Option A shipped 2026-07-24;
  its two residuals (instant overview content flips via a `.tali-nofx` frame, magic-move resynced
  onto `CAM.morph`/`morphFade`/`morphFadeDelay`) and **(5)** (one viewport-driven wrap count for the
  whole overview map) shipped 2026-07-25. The owner delegated the remaining calls and they were
  **ruled, not deferred**: **(3) no-change** — an out-of-order arrival stays visually identical to a
  step, and distinguishing them buys a cue the reader has no vocabulary for; **(4) Option C (the
  shared-element FLIP rewrite) is declined** — the overview is a glance at a ~20-slide talk, not a
  navigator for 100+, and the readability floor closed most of the gap it existed for. **Do not
  re-cost Option C a third time.** A coverage-weighted refinement of (5) was tried and measured
  *worse* (15 of 25 slides against 23 of 25); the comment in the source says so — do not re-refine
  without measuring. Two LOW tradeoffs were flagged to the author and left as-is, not defects:
  ctrl+wheel-*down* claims browser page-zoom-out over the deck (that is the approved gesture), and
  it also fires inside an embedded deck on a scrollable page. *(Option B, the mode-invariant
  serpentine grid, is costed in the audit and was not chosen; the overview work is identical under
  A and B, so nothing shipped is wasted if B is ever revisited.)*
- **A separate per-page outline artifact for the book drawer** (`book-outline-artifact` Ship B's
  own spec, declined 2026-07-25 while building it). Measured rather than argued: the search index
  the sidecar would duplicate is **172 KB raw / 60 KB gzipped** on `docs/internals` (146 KB / 50 KB
  on `docs/guide`), it is already lazy-loaded on every page via `TALIESIN_SEARCH_URL`, and Cmd-K
  fetches it anyway — so a ~13x-smaller sidecar buys ~55 KB gzipped on one cached subresource in
  exchange for a second copy of `search::page_fragment`'s render-then-number-then-resolve recipe, a
  second whole-project assembly, a second `refresh_*_for_page` invalidation, a second serve route
  and a second build write. The drawer reads the same index through the same loader
  (`window.taliLoadSearchIndex`). Revisit only if the index ever grows past the point where loading
  it on a drawer open is felt — and measure again before believing it has.
- **`drawer-typeahead`, a filter box in the chapter drawer** (declined 2026-07-25 with the above).
  The audit named the cheaper alternative itself: Cmd-K plus the drawer's collapsible outline
  covers the need, and a second search-like box beside a Search button is a discoverability smell.
- **A "~N min read" label on a book chapter** (decided 2026-07-25 while shipping the cost signal).
  `prose::word_count` excludes fenced code and math, so a code-heavy chapter is understated — and
  reading code is *slower* than reading prose, so a minutes label carries that error into a promise
  about the reader's time, in the wrong direction, on exactly the chapters this tool exists for. The
  drawer and Contents print words. (The dated-post reading-time estimate in `render/mod.rs` is a
  different surface and is unchanged; `is_article` is test-pinned, do not touch it.)
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
