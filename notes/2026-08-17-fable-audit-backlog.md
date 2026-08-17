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
| `cargo test` / `./tools/gates.sh` | **not run by this audit** (read-only session; reproductions used the release binary and targeted greps). The first grinding session should run `./tools/gates.sh` first and record the verdict line here. |
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

# BATCH F1: the destructive sweep bypass (data loss)

## FA1 [A] RELEASE-BLOCKING: `build --out` can delete a stranger's directory and exit 0

`is_taliesin_output`'s ownership fallback claims any directory containing
`_assets/app.*.css`: the check is `name.starts_with("app.") && name.ends_with(".css")`
(`build.rs`, grep `is_taliesin_output`), which matches `app.min.css`, a top-conventional
stylesheet name, and any webpack/parcel `app.<hash>.css`. Once claimed,
`unowned_output_entries` returns `None`, the refusal never fires, and `sweep_stale`
deletes the user's files. Reproduced live by the refuter: a `victim/` containing only
`_assets/app.min.css` plus the user's own files came out swept and overwritten, exit 0.
This is a **bypass of the guard the landed Batch 1 (2026-08-13 queue) installed**; the
guard's own comment names this disaster as its motivation. Real emitted names are
`app.<1-16 lowercase hex>.css` (`write_asset_bundle`), so the fallback is strictly looser
than what it recognizes.

- Repro: `mkdir -p victim/_assets && touch victim/_assets/app.min.css && echo hi > victim/precious.txt`,
  then `taliesin build <any site dir> --out victim --no-exec`; observe `swept`, exit 0,
  `precious.txt` gone.
- Fix: tighten the fallback to the actual emitted shape (lowercase-hex hash segment), or
  require the claim marker outright; a foreign `_assets/app.min.css` must hit the refusal
  path.
- Done when: a failing test first (fixture above, in `stale_sweep.rs`'s lane) asserting
  the build **refuses** and touches nothing; then green.
- Effort: S.

# BATCH F2: trust surfaces (exit codes and gates that lie)

## FA2 [V] RELEASE-BLOCKING: site build exits 0 under `--strict` on a page it could not read or write, and keeps the stale page

Reproduced: `chmod 444 _site/p2.html`, edit `p2.tmd`, `build . --strict --no-exec` prints
`error cannot write ...: Permission denied`, then `built ... 1 page`, **exit 0**, and the
output still holds the old body (the sweep keeps the failed page's URL). Same for an
unreadable source. The unreadable-source early return hard-codes `problems: 0` and the
write-failure arm increments neither `problems` nor `unparseable` (grep `cannot write` /
`cannot read` in `build.rs`); the verdict is `unparseable == 0 && !strict_fail &&
!kernel_fail`. The single-doc path already returns `ExitCode::FAILURE` on a write failure,
so the two verbs disagree.

- Fix: count both I/O arms into the exit verdict (plain build too, not just `--strict`: a
  page that could not be written is not a built site).
- Done when: failing integration test first (read-only output file; assert non-zero exit),
  then green; `--check-only` behaviour unchanged (it already fails).
- Effort: S.

## FA3 [V] the vacuous-needle class: full-page assertions whose needles ship inside the inlined assets

`a11y_chrome_emits_landmarks_and_a_skip_link` (`crates/core/tests/corpus.rs`, grep the
name) pins `<main id="tali-main" tabindex="-1">` against a full rendered page; that exact
string also sits in a comment in
`crates/core/assets/js/code-enhance/06-skip-link.js:3`, which ships un-minified into the
page. Reproduced on the test's own input: the needle occurs **twice**; delete the real
`<main>` emission and the test stays green. Same genus [A]: `token_contract.rs`'s
source-blob censuses ingest whole `.rs` files on `contains("<script")`, prose included
(commit `5b9684ae`'s message records three checks passing on prose while dead JS shipped).

- Fix: assert chrome needles against the page with `<script>`/`<style>` contents blanked
  (one small test helper), or against `PageParts` before asset inlining. For
  `token_contract.rs`, exclude comment lines from the census blob or anchor needles to
  call-site shapes.
- Done when: mutation-checked per the "gate the gate" rule: comment out the `<main>`
  emission in `page.rs`, confirm the test goes red, restore. Sweep the other full-page
  needles in `corpus.rs` (`tali-site-nav`, `tali-site-footer`, the `with_subresources`
  counter) the same way.
- Effort: S-M.

## FA4 [V] RELEASE-BLOCKING: CI has never executed and the committed story about why is wrong

`gh api repos/AJBogo9/taliesin/actions/permissions` returns `{"enabled":false}`: Actions
is disabled at the **repository-settings level**. ci.yml's header says the per-job
`private != true` guard makes skipped runs visible ("an inert CI never looks like a
passing one"); in reality **no run is ever created**: zero executions since 2026-07-26,
and release.yml has never fired. ~300 lines of YAML across 10 jobs and a 3-OS matrix
execute for the first time on launch day. (Ties into S16/S17 in the 2026-08-13 queue:
whatever repo the public flip creates, the workflows are unrehearsed.)

- Fix: correct ci.yml's header to name the real off-switch; add "dry-run both workflows"
  to the pre-flip sequence (a throwaway fork or the fresh repo pre-announcement; a
  `workflow_dispatch` trigger makes this cheap).
- Done when: the comment matches reality, and a rehearsal run of ci.yml and release.yml
  (on a scratch tag) has completed once with logs read.
- Effort: S for the comment, M for the rehearsal.

## FA5 [V] the corpus-deletion hole, and the accommodation that outlived its feature

Nothing pins which corpus documents must exist: the sweeps iterate whatever is on disk
with floors like `files.len() >= 5` against 82 docs, so most of the corpus can vanish
before anything notices (CLAUDE.md's ordering rule warns about exactly this). The scar
proving it bites: `corpus.rs` (grep `transclude`) still cites `corpus/transclude.tmd`
("does exactly that") to justify re-arming the per-file sourcepos floor on every
source-file alternation, but that document was deleted in wave 7 (`git log
--diff-filter=D -- corpus/transclude.tmd`), so the weakening now guards a feature that
does not exist while blinding the check to out-of-order sourcepos.

- Fix: (1) a manifest test: a checked-in sorted list of corpus doc paths compared against
  the tree, so a deletion is a deliberate one-line diff in the same commit (the
  `token_contract.rs` philosophy applied to documents). (2) Remove the re-arm
  accommodation and the stale comment; restore the strict monotonic per-file check (run
  the suite first to confirm no surviving document legitimately violates it).
- Done when: manifest test mutation-checked (delete a doc locally, confirm red); ordering
  check strict; comment gone.
- Effort: S.

## FA6 [V] the getting-started deploy workflow ships error-severity diagnostics with a green exit

`grep -n "check-only\|--strict" docs/guide/using/getting-started.tmd` returns nothing;
its GitHub Actions example and the Netlify/Vercel/Cloudflare bullets use bare
`taliesin build .` (line ~208), which by design exits 0 on broken xrefs and missing
images. The CLI reference's own "thorough two-stage CI gate"
(`build . --check-only && build . --strict`) never reaches the chapter users copy from.

- Fix: put `--check-only` into the copy-paste workflow and one sentence naming why.
- Done when: the docs build green (`build docs/guide --check-only`) and the example
  includes the gate. Optionally extend `stale_docs.rs` to assert deploy examples carry
  it, mutation-checked per rule 5, or skip the gate and accept the docs fix alone.
- Effort: S.

# BATCH F3: execution and freeze integrity

## FA7 [A] outputs computed downstream of a failed or interrupted cell are persisted to `_freeze/`

The persist loop (`exec.rs`, grep `self.freeze.put`) skips only the `#| cache: false`
position (`first_uncacheable`) and cells whose OWN output is an error (`is_uncacheable`).
A cell that ran AFTER an upstream cell errored or was interrupted executed against
half-mutated kernel state, and its output is persisted under a cumulative key asserting
it follows from a complete upstream run. Durable stale hit inside the code-only axes the
key claims to close; the refuter tried to kill this and reported every load-bearing claim
checks out.

- Fix: bound the persist range at the first errored/interrupted index in the run, the
  same shape `first_uncacheable` already has.
- Done when: failing test first (run A-errors, B-ok; assert B's key absent from the
  written `_freeze/<page>.json`), in the `freeze_cold_replay.rs` lane; then green.
- Effort: S.

## FA8 [A] the silence cap is reset by iopub traffic from other cells, and an ignored SIGINT leaves a runaway running

Two halves (`exec.rs`/`kernel.rs`, grep `TALIESIN_CELL_SILENCE`): the silence window is
re-armed by any iopub output rather than output attributed to the executing cell's parent
header, so one chatty cell disarms the cap for a silent runaway later in the run; and the
SIGINT path has no escalation if the kernel ignores it.

- Fix: scope the silence window to the executing cell's `parent_header.msg_id`; on an
  ignored SIGINT (cap fires twice), surface a diagnostic naming the pid instead of
  waiting forever.
- Done when: kernel-gated test (the `TALIESIN_REQUIRE_KERNEL` lane) with a two-cell doc:
  chatty cell then silent `sleep`; assert interruption fires.
- Effort: M.

## FA9 [V] four doc pages still make the "stale hit impossible" overclaim that `freeze.rs` itself was corrected for

`freeze.rs`'s module doc honestly bounds the claim ("impossible for the axes the key can
see") and enumerates the out-of-band class. The shipped docs do not:
`docs/guide/using/choosing.tmd:114`, `docs/guide/reference/cli.tmd:243`,
`docs/guide/using/code.tmd:108`, `docs/internals/execution.tmd:141` (grep `stale hit`).
FA7's fix narrows the true statement further.

- Fix: align all four with freeze.rs's boundary sentence and the `#| cache: false`
  escape hatch. **Hazard**: choosing.tmd carries census-gated numbers; touch only the
  prose.
- Done when: `build docs/guide --check-only` and `docs/internals` green; grep for the
  absolute phrasing returns nothing.
- Effort: S.

# BATCH F4: the line model (the CR class)

## FA10 [V] a lone `\r` silently collapses every later block's id, slug and source slice

comrak counts a bare CR as a line ending (CommonMark); core's line model is
`str::lines()`, which does not. Reproduced:
`printf 'line one\rline two\n\n## A heading\n\npara.\n'` built with the release binary
emits `<h2 id="section" data-block-id="b-cbf29ce48422">`: `section` is the empty-slug
fallback and `b-cbf29ce48422` is fnv1a("") , i.e. `slice_lines` returned the wrong
(empty) line for every block after the CR; the next block deduped to `-1` of the same
empty hash. Zero diagnostics. **The LSP fixed exactly this class on 2026-08-13**
(`lsp_pos::lines`, whose doc says std's split "disagrees on every buffer"); the fix
stopped at the LSP boundary. Core sites (grep `.lines()` and `split('\n')` under
`crates/core/src`): the `lines` vec in `render/mod.rs` (`render_internal_impl`),
`includes.rs`'s LineOrigin map, `divs.rs`'s `preprocess`/`scan_div_spans`,
`overlong_nesting`, `cell_extract::slice_lines`.

- Fix: one shared comrak-compatible line splitter (relocate `lsp_pos::lines` into core,
  re-export for the server), used at every site above. Alternatively normalize lone CRs
  at ingest with a located warning; pick one, not both.
- Done when: the repro doc renders `id="a-heading"` with a real content hash; unit test
  in `render/tests.rs` pinning a lone-CR document's ids/slugs; grep shows core render
  paths use only the shared splitter.
- Effort: S-M.

# BATCH F5: string surgery over finished HTML (one class, four fixes)

## FA11 [A] `dedup_element_ids` rewrites `id="..."` inside escaped code text

`rename_repeated_ids` scans flat HTML for ` id="` with no tag-vs-text state, and
`escape_html` never escapes `"`, so a plain fence or inline code span SHOWING
`<div id="example">` twice gets its visible text rewritten to `example-1`, steals the
real element's anchor, and fires two bogus error-severity diagnostics (refuter reproduced
end-to-end; plain/unknown-lang fences and inline code all reach the escaper verbatim).
Severity medium only because the bogus errors at least block `--check-only`.

- Fix: make the scan tag-aware (only match inside an open tag: same discipline
  `open_tag_end`/`tag_end` already practice elsewhere in the module), or dedup on the
  block model before emission.
- Done when: failing test first: fence showing a duplicate id, assert text unchanged and
  no diagnostic; the real duplicate-div case still renames and warns.
- Effort: M.

## FA12 [A] `rewrite_tmd_links` rewrites hrefs inside inline code samples the reader sees

Raw `find("href=\"")` over finished page HTML (`site/links.rs`, grep
`rewrite_tmd_links`). An inline code span containing `<a href="other.tmd">` displays as
`other.html` in the built page (refuter reproduced). Fenced blocks survive only because
syntect escapes quotes; prose survives only because smart punctuation curls them. Two
accidents are the current defense.

- Fix: same tag-context awareness as FA11 (share the helper).
- Done when: failing test: inline code with a `.tmd` href survives verbatim; real links
  still rewrite.
- Effort: S-M.

## FA13 [A] the build scrapers: escaped prose publishes files, single-quoted attributes evade copying and warnings

Two confirmed defects plus a structural one in `build.rs`'s HTML/JS substring scanners
(`local_refs`, `external_refs`, `copy_js_imports`, `dynamic_import_specifiers`, etc):
escaped text content is harvested, so prose documenting HTML published a never-linked
`.md` into the deploy; single-quoted raw-HTML attributes are invisible, so a portable
`--out` folder ships broken with no offline warning; and the attribute-scan loop plus
its load-bearing guard exist in multiple hand-copies, so any fix must land N times.

- Fix: one shared, quote-aware (both quote kinds), text-vs-tag-aware attribute scanner
  used by every scraper; no new dependency (rule 6).
- Done when: failing tests for the two reproduced defeats (escaped-prose ref not copied;
  single-quoted `src` copied AND warned when remote); the copies collapsed to one helper.
- Effort: M-L.

## FA14 [A] offline-ref warnings inside an include name the parent file with the included file's line number

The warning's location pairs the wrong file with the line (`build.rs` offline warnings;
the include source map has the right answer). An author opens the named file at that
line and finds nothing.

- Fix: map through the per-file source map the way render warnings already do
  (`map_origin` discipline).
- Done when: failing test: offline ref inside an included partial warns with the
  partial's path and line.
- Effort: S.

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

## FA16 [V mechanism] the preview's page shell is a hand-aligned twin of the build's, and it already hardcodes `lang: "en"`

`serve_site/mod.rs` (grep `Kept structurally identical` and `byte-aligned`): the site
shell exists twice, once in core `page.rs` for the build, once in `site_page_html` for
the live preview, kept equal by comments; and the preview passes `lang: "en"` (grep
`lang: "en"`) where the build honors front-matter `lang:`. The 2026-08-13 single-file
TOC incident (CLAUDE.md records it) is this same genus: parity by duplicated
orchestration.

- Fix: extract one shared shell function in core that both paths call (the preview adds
  its dev-menu on top); thread the real `lang` through.
- Done when: the "byte-aligned" comments are deleted because there is nothing to align;
  a non-`en` `lang:` page previews with the right `<html lang>`.
- Effort: M.

## FA17 [U] the finishing sequence (`page_toc` -> warnings -> `finish_blocks` -> title) is written out ~4 times, and the copies allegedly disagree on ordering

Finder claims `render_page_doc_warned` (core `site/mod.rs`) and `render_markdown_only`
(serve_site) compute `page_toc` BEFORE `finish_blocks` while `build_page` (serve_site)
computes it AFTER, benign today only by accident of the toc gates. **Verify first**:
read the four sites, confirm the ordering difference and whether any input observes it
(a page whose block count changes in `finish_blocks`/`expand_page`).

- If confirmed: one finishing function in core, four callers; pin the order with a test
  that would have caught the divergence.
- If refuted: delete this item and record it in DO-NOT-REBUILD.md.
- Effort: verify S, fix M.

# BATCH F7: derive, don't police

## FA18 [V] the LSP completion vocabulary rotted through the cut, outside every drift gate

`NESTED_PARENTS` is duplicated verbatim in `lsp_complete.rs:15` and `lsp_nav.rs:16` and
still lists `about` (retired 2026-07-17) and `prose-lint` (retired 2026-08-02).
`PATH_KEYS` (`lsp_complete.rs`) offers path completion for `css` and all three
`include-*` keys: none is in `KNOWN_KEYS`, all three `include-*` are in `RETIRED_KEYS`,
so completion inserts what the same server's lint then squiggles. The comment above
PATH_KEYS claims it is "sourced from what the renderer actually resolves": false,
nothing derives it, nothing pins it.

- Fix: derive both from core (`vocab`'s nested-vocabulary keys for NESTED_PARENTS;
  KNOWN_KEYS-filtered path-typed keys for PATH_KEYS), single copy in one crate; or, at
  minimum, one drift test pinning both against `KNOWN_KEYS`/`RETIRED_KEYS`,
  mutation-checked per rule 5.
- Done when: completing in a front-matter block offers no retired key; the duplicate
  const is gone.
- Effort: S-M.

## FA19 [V facts] CLAUDE.md and gates.sh corrections, then the diet decision

Three verified-stale facts and one wrong warning, all in the file every session loads:

1. CLAUDE.md:22 says the Internals book is "six chapters plus an index"; the tree has
   five plus an index (server.tmd folded into architecture.tmd on 2026-08-14).
2. CLAUDE.md:489 references "the two `--help`-prose holes below"; nothing below matches
   (grep `--help` in CLAUDE.md: one hit, line 489 itself).
3. `tools/gates.sh:16` header says "ELEVEN gates" while the script runs twelve and
   CLAUDE.md says twelve (the script's verdict line is the authority either way).
4. CLAUDE.md's standing-freeze paragraph says the LRU eviction order is "not
   test-guarded"; `exec_pool.rs`'s test module pins eviction order and cap
   (`evicts_least_recently_built_beyond_cap`, `touching_a_page_keeps_it_warm`). Verify
   whether "the build relies on it" means something beyond the pool; if not, correct the
   claim (the freeze itself can stand, the fear should be accurate).

Fix the four in one small commit. The larger diet (roughly 250 of 518 lines are incident
narrative whose invariants named tests now enforce) is FD4, a decision, not this item.

- Effort: S.

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

# BATCH F10: release hygiene

## FA24 [V] RELEASE-BLOCKING-adjacent: comrak's default features compile oniguruma (C) plus a CLI stack core never uses

Confirmed in `cargo tree`: `onig`/`onig_sys`, `clap`, `bon`, `emojis` are all present via
`comrak = "0.52.0"` with default features, and onig wins the syntect backend over the
workspace's deliberate pure-Rust `default-fancy` selection (the root Cargo.toml comment
already suspects this). The manifest elsewhere bans a C build dependency (`ravif`/nasm)
for breaking `cargo install`; this is the same class kept by accident.

- Fix: `comrak = { version = "0.52.0", default-features = false }`; core sets only
  runtime `Options`, none of which is feature-gated (verify: no use of comrak's syntect
  plugin or CLI). Update the root Cargo.toml comment that documents the old state, and
  deny.toml if it names onig.
- Done when: `cargo tree | grep -i onig` empty; corpus + highlight tests green (syntect
  now actually runs fancy-regex: watch for regex-dialect differences in the two-face
  syntaxes; `highlight_langs.rs` is the canary); `cargo build` from a clean checkout on
  a machine with no C toolchain assumption.
- Effort: S (plus watching the canary).

## FA25 [U] no `[profile.release]`: the shipped binary carries strippable symbols

The release binary is ~32 MB; finder claims ~5 MB is strippable symbols and there is no
`[profile.release]` in the workspace. Verify (`grep -rn 'profile.release' Cargo.toml`;
`strip` a copy and diff sizes), then add `strip = true` (+ `lto = "thin"` if link time
stays acceptable). Binary size is a dated, uninstrumented number by convention: measure,
date it, do not gate it.

- Effort: S.

## FA26 [U] deny.toml and the root manifest document a dependency configuration that no longer exists

Finder claims stale prose in both. Verify each named claim against `cargo tree` (FA24
changes the answer for the syntect/onig paragraphs), fix what is actually stale, delete
the item if nothing is.

- Effort: S.

## FA27 [A] kernel startup preamble failures are silent

~270 lines of version-sensitive Python are embedded in `kernel.rs`; if the preamble
fails on a future Python, the error is swallowed and cells misbehave downstream with no
pointer to the cause.

- Fix: surface a preamble stderr/exception as a located "kernel preamble failed"
  diagnostic naming the interpreter.
- Done when: kernel-gated test with a poisoned preamble asserts the diagnostic.
- Effort: S-M.

## FA28 [A] `social_head` hand-rolls absolute URLs, drifting from `abs_page_url`'s percent-encoding

Low: two spellings of URL assembly; a space-containing page path produces an invalid
`og:url` in one of them. Unify on the shared helper (`site/meta.rs`, grep
`social_head`).

- Effort: S.

## FA29 [U] doctor: exit 0 on a nonexistent project dir; `--format json` buries the two verdicts

Two low UX claims, unverified: `doctor <nonexistent>` reports on the wrong environment
and exits 0; the JSON output buries check verdicts under a ~150-entry package inventory.
Verify both with the binary; fix is small if real (refuse the missing dir; hoist the
verdicts to the top of the JSON object).

- Effort: S.

# BATCH F11: diagnostic precision

## FA30 [V] broken-xref and broken-citation squiggles cover the whole line instead of the `@token`, which also disables their quick fix

Author-observed on `corpus/diagnostics/refs.tmd:18` (`@fig-reslts`): the squiggle spans
the line, not the reference. The plumbing for precise ranges exists end to end and is
already used by the front-matter linter: `render::Warning` carries `col`/`end_col`
("None = whole-line", `model.rs`), and `to_lsp` maps a columned diagnostic to an exact
UTF-16 range (`lint.rs`, `to_lsp_uses_a_precise_span_when_columned`). The xref validator
is the exception for a structural reason: it recovers anchors from the RENDERED HTML
(`data-tali-xref="..."` markers) after the source is gone, so it only has the block's
start line and files `w.at(file, line)` with no columns (`cite/validate.rs`, grep
`broken cross-reference`); same for broken citations (`cite/render.rs`, grep
`broken citation`). The compounding cost: `to_lsp` attaches the one-click-fix payload
ONLY when a suggestion has a precise span (`lint.rs`, "ONLY when a suggestion has a
precise column span"), so the did-you-mean the message already computed can never become
a "Change to `@fig-results`" code action today.

- Fix: after resolving the anchor name, locate the literal `@<anchor>` in the source
  line the warning points at and set `col`/`end_col` (1-based Unicode-scalar columns per
  the `Warning` field docs); if the token is not on that line (a multi-line block whose
  ref sits later), scan the block's sourcepos span before falling back to whole-line.
  Apply to both the xref and the citation site.
- Done when: failing lint-level test first: a `refs.tmd`-shaped input yields a
  broken-xref diagnostic whose `[col, end_col)` spans exactly `@fig-reslts`,
  mutation-checked per rule 5 (widen the range in the fix, confirm red); the whole-line
  fallback is pinned for a token the line-scan cannot find; and, verified while
  implementing, the "Change to `@fig-results`" quick fix now appears for a columned
  xref did-you-mean (the `suggestion` field must survive into `lint::Diagnostic` for
  that; check `diag_from`).
- Effort: S-M.

---

# DECIDE: the author's calls, not code

Recommendations recorded because leaving none is how these get answered by accident.
Each cites the prior ruling it touches; none should be worked without the author's
explicit go.

## FD1: `taliesin new` (revisits the 2026-08-13 "do not cut another feature" verdict)

Anatomy (verified in outline): `cmd_new` produces one 12-line scaffold, carried by slug
validation, a hand-rolled civil-date algorithm kept to avoid a date dependency, the
`RETIRED_NEW_KINDS` register, and a 337-line integration suite. The standing directive
says lean toward cutting; the 2026-08-13 audit verdict says the seven-verb journey has
no hole and "do not cut another feature". Both are the author's own words, so only the
author resolves them. If cut: fold one example post into `init`'s scaffold, add the
`RETIRED_COMMANDS` entry, delete `new_cli.rs`. If kept: delete this item.

## FD2: retirement-register audience split (RULED 2026-08-17: prune the private half, keep and reword the external half)

The author raised this on 2026-08-17 ("no previous users, so no backwards
compatibility"), which supersedes S15's "keep everything through 1.0". The premise is
half right: the registers serve two audiences, and only one is the (empty) set of
previous Taliesin users. The other is strangers typing vocabulary another tool taught
them, who exist from day one of the public flip. The full census at `2827c3bb` (92
entries), classified by "could a stranger ever type this name":

**DELETE (~40, private history, can never fire for a public user):**
- `RETIRED_COMMANDS`: `blocks`, `symbols`, `skim`, `read`, `map`, `features`, `vocab`,
  `schema`, `mcp`
- `RETIRED_KEYS`: `mounts`, `body-start`, `body-end`, `output` (config), `datasets`,
  `prose-lint`, `fig-export`, hero `image`, hero `image-alt`, `venue`, `award`,
  `links`, `theorems` (both scopes), config `r`, config `publish`, `acknowledgments`/
  `acknowledgements` (judgment: plausible scholarly guess, lean delete)
- `RETIRED_DIV_CLASSES`: `code-walkthrough`, `scrolly`, `step`, `fade-out`,
  `highlight`, `debug`, `magic-move`, `sidenote`, `marginnote` (the last two are
  tufte-css vocabulary, judgment, lean delete). Deleting an open-vocabulary class
  entry is a true no-op for strangers: the class becomes CSS passthrough, correct for
  a name that never did anything here.
- `RETIRED_FLAGS`: `--bare`. `RETIRED_CELL_LANGS`: `glsl`, `pyodide` (judgment: a
  quarto-live extension uses the name, lean delete).
- `RETIRED_NEW_KINDS`: all three, resolved together with FD1 either way.

**KEEP (~45, another tool's vocabulary, will fire for real newcomers):**
- Quarto front-matter/config/cell vocabulary: `format`, `css` (both scopes),
  `include-in-header`, `include-before-body`, `include-after-body`, execute `echo`,
  execute `include`, config `toc`, `about`, `doi`, listing `sort`, listing
  `categories`, callout `important`, `caution`, `appearance`, `icon`, input `range`,
  cell option `code-line-numbers`, `footer`, `logo`, shortcode `video`, cell lang `r`
- Quarto/reveal div classes: `theorem`, `lemma`, `corollary`, `definition`, `proof`,
  `example`, `proposition`, `remark`, `panel-tabset`, `columns`, `column`,
  `column-screen`, `aside`, `fragment`, `incremental`, `notes`
- Guessable verbs: `render`, `serve`, `dev`, `publish`, `check`, `pdf` (the HTML-only
  positioning answer, delivered at the moment of the ask), `run`, `completions`
  (judgment, lean keep)
- `--host` (every dev server has one; the note carries the loopback-only stance)

**Plus a reword pass on the keep-set:** the notes read "it was removed on <date>",
history a stranger does not share. Reword to the stranger's answer ("Taliesin renders
HTML only; there is no `format:` key: ..."), same one-line register mechanism. Registers
stay; `every_retired_vocabulary_name_is_gone_unstyled_and_diagnosed_without_a_did_you_mean`
derives the checks either way. Parser-side behavior pins whose key survives
(`a_retired_listing_sort_cannot_reverse_the_cards_or_the_feed`) stay; a behavior pin for
a DELETED entry's key is re-checked: the read must already be gone, then the pin can go
with the entry.

- Done when: the delete-list entries are gone (each is one register line, per the
  retirement rule); the keep-list notes read as answers, not tombstones; record the
  ruling in DO-NOT-REBUILD.md so S15's superseded recommendation is not re-applied.
- Effort: S-M (mechanical deletes; the reword is the judgment half).

## FD3: the first-run execution notice (revisits a shipped 2026-07-29 decision)

The one runtime acknowledgment that preview executes a document's code fires once per
machine ever (TTY- and marker-gated), i.e. on the user's own first document, not when
previewing someone else's. Options: reclassify it as onboarding UX and accept the
documented Jupyter-style trust model (legitimate, matches the docs), or retarget the
line at a threat-correlated signal (first execution of a document outside any previously
previewed project). The notice shipped as designed, so this is a re-scope; default is
keep as-is.

## FD4: the CLAUDE.md diet (overlaps S12's judgment call on notes/)

After FA19's fact fixes: roughly 250 of CLAUDE.md's 518 lines narrate incidents whose
invariants named tests now enforce (the tests were verified to exist). The prose is
advisory where a gate is load-bearing. Candidate rule: an incident paragraph survives
only if it changes what a session DOES (a hazard, an order of operations), not what it
knows happened. This is the author's voice and the author's file; recommendation is to
cut to ~250 lines, but it is not a defect.

---

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
7. **"~121 KB of comments ship to every reader."** Half wrong: CSS is minified on both
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
