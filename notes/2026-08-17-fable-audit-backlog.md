# Fable audit backlog, 2026-08-17

Durable state for the 2026-08-16 fresh-model audit (Fable 5). **This file is the handoff**:
every item carries its own anchor, reproduction (where one exists) and done-condition, so no
item needs the conversation that produced it.

Produced by a 16-lens audit at `5b9684ae` (one adversarial refuter per finding). The refuter
fleet was killed mid-run by the account's monthly spend limit, so confidence is graded
per-item (see the key below): 26 findings completed the finder/refuter cycle (6 refuted, 20
confirmed), and the orchestrator hand-reproduced the highest-impact unverified claims
against the release binary before filing them. Findings that got neither check are marked
**[U]** and start with a verification step. Full structured findings: the audit session's
scratchpad `audit-full.json` (not committed; this file supersedes it).

**Landed 2026-08-17**, each with `./tools/gates.sh` green before and
after: FA1, FA2, FA3, FA5, FA6, FA7, FA8, FA9, FA10, FA11, FA12, FA13, FA14, FA17, FA18,
FA19, FA24, FA25, FA26, FA27, FA28, FA29, FA30, plus the correctable halves of FA4 and FA16, plus
all four DECIDE calls (FD1, FD2, FD3, FD4). Every fix that
had a done-test was mutation-checked in both directions (revert the fix, watch the test go
red). Deleted from this file per rule 3; what remains below is what remains.

**Relationship to the other queues.** [2026-08-13-mvp-audit-backlog.md](2026-08-13-mvp-audit-backlog.md)
is the older defect queue (its batches 11-12 remain open; nothing here duplicates them,
overlaps are cross-referenced). [backlog.md](backlog.md) is the release critical path and
was deliberately not edited: its admission rule is the author's. Items here that arguably
meet its bar ("blocks the release") are flagged **RELEASE-BLOCKING** for the author to
promote or not.

## Baseline at `5b9684ae`

| | |
|---|---|
| `target/release/taliesin` | reports `0.3.0 (5b9684ae)`, i.e. current with HEAD at audit time |
| `cargo test` / `./tools/gates.sh` | Run at the start of the first grinding session (2026-08-17): `PASSED — every gate ran and passed (12 gates).` Green again after each batch below landed. |
| Working tree | clean, untouched by the audit |

## Rules for working this file

1. **One batch per session, one branch, one commit.** Batches are drawn so each is a
   coherent unit touching related code.
2. `./tools/gates.sh` green before and after. It needs
   `TALIESIN_PYTHON="$PWD/.venv/bin/python"` or it exits 2 at preflight. Take the gate
   count from the script's own verdict line. A bare `git push` hangs in the pre-push hook
   without `TALIESIN_PYTHON`; do not run two workspace test suites concurrently (they
   deadlock).
3. **Delete an item from this file when it lands.** Never a `[x]`, never a strikethrough.
   Delete this file when it is empty.
4. **Trust an item's symptom, never its cause, line number or cost.** Anchors were correct
   at `5b9684ae`; grep the named symbol before pricing work.
5. Failing test first wherever the item names a done-test. **Any new drift gate must be
   mutation-checked against exactly the shape it guards** (the standing "gate the gate"
   rule in [DO-NOT-REBUILD.md](DO-NOT-REBUILD.md)); that rule is itself the subject of
   FA3, so it applies doubly here.
6. Library outsourcing was decided against for this tree (DO-NOT-REBUILD.md names the
   list: no clap, no html-escape, no morphdom, no lightningcss, etc). No item below needs
   a new dependency; if a fix seems to, re-read the item.

## Confidence key

- **[V]** reproduced by the auditing orchestrator directly against the release binary or
  the tree, command and output in hand.
- **[A]** confirmed by the finder/refuter pair (adversarial verification completed).
  Where the refuter narrowed a finding, the narrowed statement is what is written here.
- **[U]** finder quoted evidence but no independent check ran (spend limit). **The item's
  first step is to verify the claim**; if it does not reproduce, delete the item and
  record the refutation in DO-NOT-REBUILD.md.

---

# BATCH F2: trust surfaces (gates that lie)

## FA4 [V] RELEASE-BLOCKING: CI has never executed (the comment half landed; the rehearsal has not)

`gh api repos/AJBogo9/taliesin/actions/permissions` returns `{"enabled":false}`: Actions
is disabled at the **repository-settings level**, so **no run is ever created**. Re-measured
2026-08-17: 382 runs exist, the newest from 2026-07-26, none since ci.yml was restored on
2026-07-28; release.yml has never fired. ~300 lines of YAML across 10 jobs and a 3-OS
matrix execute for the first time on launch day. (Ties into S16/S17 in the 2026-08-13
queue: whatever repo the public flip creates, the workflows are unrehearsed.)

**ci.yml's header was corrected on 2026-08-17** and now names the real off-switch and the
unrehearsed consequence; the false "a skipped job is visible in the Actions UI" claim is
gone. What is left is the rehearsal, which needs Actions turned back on and is therefore
the author's call (billing on a private repo is why it is off).

- Done when: a rehearsal run of ci.yml and release.yml (a throwaway fork, or a scratch tag
  once the repo is public; a `workflow_dispatch` trigger makes this cheap) has completed
  once with its logs read.
- Effort: M.

# BATCH F5: string surgery over finished HTML (three of four fixed)

## FA15 [A] the two line coordinate systems are still both bare `usize`

CLAUDE.md calls pairing them "the bug that keeps happening"; the current defense is
comments, and the 2026-08-13 incident touched ten sites. A `BufLine(usize)` /
`SrcLine(usize)` newtype pair (construction at the two boundaries: `group_divs` output
and `map_origin`/`map_span` output) makes a wrong pairing a compile error and retires
the class.

- Done when: `Warning::at` and `data-sourcepos` assembly accept only the source-side
  type; the buffer-side type never reaches a user-visible surface; no behavior change
  (pure refactor, existing tests green).
- Effort: M.

# BATCH F6: parity by construction

## FA16 [V mechanism] the preview's page shell is a hand-aligned twin of the build's

`serve_site/mod.rs` (grep `Kept structurally identical` and `byte-aligned`): the site
shell exists twice, once in core `page.rs` for the build, once in `site_page_html` for
the live preview, kept equal by comments. The 2026-08-13 single-file TOC incident
(CLAUDE.md records it) is this same genus: parity by duplicated orchestration.

**The `lang` symptom landed 2026-08-17** (`PageDoc` carries the resolved lang and the
shell reads it; `a_page_previews_with_the_lang_it_builds_with` pins it, mutation-checked).
That was one invented value; the shell can still invent the next one, which is what this
item is actually about.

- Fix: extract one shared shell function in core that both paths call (the preview adds
  its dev-menu on top).
- Done when: the "byte-aligned" comments are deleted because there is nothing to align.
- Effort: M.

# BATCH F7: derive, don't police

## FA20 [U anatomy verified] main.rs: one verb table instead of four hand-synced copies plus 678 lines of self-scanning police

The 7-verb table exists as the dispatch match, the `COMMANDS` const, the help text block,
and the `subcommand_help` match, plus an ungated fifth copy in
`docs/guide/reference/cli.tmd`; main.rs then spends more than half its lines on tests
that scan its own source via `include_str!("main.rs")` to keep the copies aligned.
**Verify the four-copy count first**, then: one `const COMMANDS: &[Command]` (name,
blurb, help text, handler fn pointer) from which dispatch, the const, and both help
surfaces derive. No clap (rule 6); the hand-written microcopy survives as struct fields.
The self-scanning gates whose subject disappears get deleted with it.

- Done when: adding a hypothetical verb requires exactly one table row (plus its cli.tmd
  row); `main.rs` shrinks by several hundred lines; behavior identical (`help_cli.rs`
  and `new_cli.rs` suites green unchanged).
- Effort: M.

# BATCH F8: the browser client

## FA21 [U] `keepScroll`'s Y-restore defeats native scroll anchoring: edits above the viewport yank the page

`web-client/client.js` (grep `keepScroll`): records `scrollY`, applies the op,
force-restores. Chrome/Firefox scroll anchoring already keeps viewed content pixel-stable
on above-viewport mutations; the restore reverts that adjustment, so the content shifts
by the height delta: the exact yank the function's comment claims to prevent. Finder
measured 245px shift through keepScroll vs 0px without, in a Chrome harness; the code
path and the browser mechanism are confirmed, the measurement is not.

- Verify first with the chrome-devtools MCP against a live preview (edit a block above
  the viewport, watch the viewed block's rect). If confirmed: keep the X-restore
  (that half fixed a real bug, the comment records it), drop or condition the Y-restore
  (only restore when the mutation is below the viewport, or trust anchoring and pin Y
  only for same-block updates).
- Done when: the MCP loop shows a stable viewport across an above-viewport edit AND
  across the original bug's reproduction (type one character while scrolled: no yank).
- Effort: S-M.

## FA22 [V gap] the client half of two load-bearing goals has zero executed coverage

`web-client/` has no test files; the only check is `tsc`. The Rust half of the op
protocol is contract-pinned; `applyOps`' Remove-before-Insert ordering, SetMeta, and
state preservation are exercised by nothing executable. **Note the constraint before
filing code**: wave 6 cut the headless-Chrome driver deliberately, so "add browser
tests" re-adds a cut. Options, author's pick: (a) extract the pure op-application core
into a function testable under `node --test` with a ~50-line DOM stub (the vscode
companion already runs `node --test`, so the lane exists); (b) accept the gap
deliberately and record it in DO-NOT-REBUILD.md so it stops being re-found by every
audit.

- Done when: either the node-level test exists and covers apply-ops ordering + SetMeta,
  or the acceptance is recorded.
- Effort: M for (a), S for (b).

# BATCH F9: performance truth

## FA23 [U] every save in a site preview renders the whole project, and the published warm-edit number measures one page

Finder: `rebuild_project` calls `refresh_xrefs` on every non-structural save, whose
harvest renders EVERY page to full HTML and keeps only the xref numbers
(`site/mod.rs`, grep `harvest_xref_numbers`); the edited page renders up to three times
along the path; the LSP process re-walks the project per didChange batch on top. The
committed live-edit-bench measures the single-page render, so the headline number does
not describe a book-sized save. Architecture lens converged independently on the same
mechanism.

- Verify first by measurement (the WS-batching precedent: measure, then decide): a
  synthetic 100-200 page project, instrument save-to-reload. If the ceiling is fine,
  record the number and close; if not: gate the harvest on the cheap
  `scan_xref_targets` registry diff (skip the O(pages) render when the anchor set and
  the edited page's own float/cell labels are unchanged), and extend live-edit-bench +
  `regression.rs` with the project-scale number so it has a tripwire.
- Done when: a committed instrument carries the project-scale save number either way
  (the numbers rule: no uninstrumented claims).
- Effort: verify S-M, fix M.

# BATCH F11: the committed-design residue

Opened 2026-08-17 by the author's no-backwards-compatibility ruling (recorded in
[DO-NOT-REBUILD.md](DO-NOT-REBUILD.md)). Both items were the two open author calls the
Fable audit left; the ruling answers the first and the second is a design question it
raises rather than settles.

## FA31 [V] the seven theorem xref prefixes: RULED CUT, one design question first

`XREF_LABELS` (`crates/core/src/cite/render.rs`) holds 12 prefixes; **7** — `thm`, `lem`,
`cor`, `def`, `prp`, `exm`, `rem` — name theorem environments retired 2026-08-03/08-08 that
nothing can define a target for. `RETIRED_XREF_PREFIXES` subtracts them from the completion
menu. They stay in the label table for one reason only: so a leftover `@thm-a` resolves far
enough to draw a broken-cross-reference error instead of passing through as literal text.
**That reason is a backwards-compatibility argument and the author has ruled it void.**

**Measured against the release binary 2026-08-17**, on a real build, so the consequence is
not in doubt:

| written | published page | `--check-only --strict` |
|---|---|---|
| `@sec-one` (live) | linked "Section 1" | — |
| `@thm-pythagoras` | linked "Theorem" | error: broken cross-reference |
| `@figg-scree` (typo) | literal `@figg-scree` | **silent** |
| `@Fig-scree` (wrong case) | literal `@Fig-scree` | **silent** |

So deleting the seven moves row 2 to row 3 — and that also hits a **new** author who assumes
theorem refs exist, which is not a migration case. The seven entries were papering over a
general gap: **an unknown xref prefix is silent, always.** They fixed it for seven names.

- **Open question, the author's** (asked 2026-08-17, not answered): should
  `@<unknown>-<ident>` be diagnosed at all? Constraint that probably explains why no general
  rule exists: `parse_xref` would treat `@rust-lang` in prose as an xref candidate, so a
  blanket diagnostic false-fires on ordinary writing. Note the tool's usual answer
  (did-you-mean inside edit distance 2) misfires here — `thm` is exactly 2 from `tbl`.
- Do when answered: delete the 7 tuples + `RETIRED_XREF_PREFIXES`, invert `vocab.rs` to read
  a positive live list, collapse `a_retired_xref_prefix_is_diagnosable_but_not_offered`.
  **Check `corpus/theorem-book/` first** — it exists and may carry `@thm-`/`@exm-` refs that
  the ruling says to rewrite rather than preserve.
- Effort: S once the question is answered.

## FA32 [V] `_site.yml`'s `head:` is the last documented forcing hatch, and is unused

`head:` injects arbitrary markup into every page's `<head>`. It is used by **zero**
documents in the tree (it is also one of the two entries on the recorded "unused offered
vocabulary" tail). Deliberately left OUT of the 2026-08-17 `theme:` cut: it is a general
head-injection key, not a theming key, so folding its removal into a theming commit would
have decided a separate question silently. The theming recipe it used to teach was deleted
from the guide with that commit.

- Decide: does `head:` earn its keep for the things it is actually for (an analytics
  snippet, a search-console `<meta>`), or does it go as unused surface? Weigh against the
  standing cut directive; note it cannot be justified by adoption, since there is none.
- Effort: S either way.

---

# DECIDE: answered 2026-08-17

All four calls were made by the author on 2026-08-17 and are recorded in
[DO-NOT-REBUILD.md](DO-NOT-REBUILD.md). Nothing here is open.

- **FD1 — cut `taliesin new`.** Landed 2026-08-17: `init` writes the example post the
  verb used to scaffold, and the CLI is six subcommands.
- **FD2 — delete the retirement registers, BOTH halves.** The ruling went past the
  recommendation: the author declined to keep the ~45 entries that answer another tool's
  vocabulary ("I want taliesin to be a lean completely separate tool from everything
  else"). Landed 2026-08-17.
- **FD3 — keep the first-run execution notice as it shipped**, on the documented
  Jupyter-style trust model. No code change; do not re-scope it.
- **FD4 — cut CLAUDE.md.** Landed 2026-08-17: 524 → 346 lines.

# Refuted or corrected: do not re-file

Killed by the refuter pass or by the orchestrator's own reproduction. Migrate to
[DO-NOT-REBUILD.md](DO-NOT-REBUILD.md) when this file is worked.

1. **"feed.rs is unused: no deployed site emits a feed."** Wrong denominator: the finder
   grepped the marketing/docs sites. `corpus/tech-blog` is the author's real deployed
   blog, sets `url:`, carries two uncapped dated listings, and `tech_blog.rs` pins the
   emitted `blog.xml`/`projects.xml`. Feeds also visibly survived the Wave-4 cut of
   their sibling features. Feeds stay.
2. **"A draft page's anchors win the xref registry in preview" as a user-facing
   divergence.** Mechanism real (first-definition-wins, preview includes drafts, build
   excludes), consequences overstated; refuted at filed severity. If it ever bites, the
   symptom is an @ref resolving to a draft in preview only.
3. **"The data-* censuses are pure tax with zero catches."** They caught real drift; the
   recommendation would have lost coverage the second pin uniquely provides.
4. **"The numeric anti-vacuity floors only ever fire as false alarms."** Two real fires
   on record; the floors stay.
5. **"notes/ is uninstrumented by neglect."** The exclusion is deliberate and recorded
   (dated record, not shipped surface).
6. **"Process prose regrows faster than campaigns cut it."** The current
   docs/superpowers plans are an active campaign, not residue.
7. **"`doctor --format json` buries the two verdicts under a ~150-entry package
   inventory."** (FA29's second half.) Checked against the binary 2026-08-17: the object's
   top-level keys are `checks`, `ok`, `packages`, and serde_json writes them in that order,
   so both verdicts precede the inventory. The first half of FA29 (exit 0 on a nonexistent
   project directory) did reproduce and is fixed.
8. **"~121 KB of comments ship to every reader."** Half wrong: CSS is minified on both
   paths since 2026-08-09 (`minify.rs`, called by `page.rs` and `build.rs`). JS comments
   DO ship, but by recorded decision: the JS minifier was cut 2026-08-08 with reasoning
   in `minify.rs`'s module doc (mis-tokenization risk vs the CSS half carrying ~3/4 of
   the win). Do not re-file a JS minifier without new evidence; measured single-file
   weights for context: 207 KB no-math page, 578 KB with one formula (KaTeX CSS+fonts).

# Affirmations worth not re-litigating

Measured or read directly during this audit; each closes a question a future session
would otherwise reopen.

- **`diff.rs` is sound.** The LCS-via-LIS reduction documents and debug-asserts its
  uniqueness precondition; moves, SetMeta-vs-Update, and chained inserts are each
  pinned. Do not "improve" it without a failing case.
- **The LSP transport is bought, not hand-rolled** (lsp-server/lsp-types), every
  dispatch path is wrapped in catch_unwind with correct JSON-RPC error replies, and
  shipped LSP code contains zero bare unwrap/expect (all 233 hits are in test modules).
- **kernel.rs delegates the Jupyter wire protocol** (jupyter-protocol /
  jupyter-zmq-client), including HMAC; the alarming line count is ~half embedded Python
  preambles and tests.
- **`exec_pool`'s eviction order IS test-guarded** (see FA19.4; the standing-freeze
  paragraph's fear is stale, not the freeze).
- **The security guards are correct and self-testing** (`ws_origin_ok` +
  `with_host_guard`, including the bracketed-IPv6 case their tests record as a past
  bug).
- **Cmd-K search earns its cost** at current scale (61 KB gzipped, lazy-loaded);
  seo.rs/meta.rs are lean and url-gated.
- **The freeze-cache module doc is honest about its boundary**; FA9 is about the shipped
  docs lagging it, not the design.
