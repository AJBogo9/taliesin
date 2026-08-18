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
after: FA1, FA2, FA3, FA5, FA6, FA7, FA8, FA9, FA10, FA11, FA12, FA13, FA14, FA15, FA16, FA17, FA18,
FA19, FA20, FA24, FA25, FA26, FA27, FA28, FA29, FA30, plus the correctable half of FA4, plus
all four DECIDE calls (FD1, FD2, FD3, FD4). Every fix that
had a done-test was mutation-checked in both directions (revert the fix, watch the test go
red). Deleted from this file per rule 3; what remains below is what remains.

**Landed 2026-08-18**: FA21, FA22, FA23, FA31, FA32 — the whole of batches F8, F9 and F11.
FA21 was confirmed by measurement and fixed; FA22, FA23 and FA31's open question were
author-delegated calls, decided toward the standing cut directive and recorded in
[DO-NOT-REBUILD.md](DO-NOT-REBUILD.md) so they are not re-found. **Only FA4 remains**, and
only its rehearsal half, which needs Actions switched back on.

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

## FA4 [V] RELEASE-BLOCKING, **DEFERRED BY THE AUTHOR to immediately before the release**: CI has never executed (the comment half landed; the rehearsal has not)

> **Scheduled 2026-08-18, the author's call: do this just before the release, not now.** It is
> not blocked on anything technical any more — only on Actions being switched back on, which
> costs money on a private repo and therefore buys nothing until the tree is the one that
> actually ships. Rehearsing early would also rehearse the *wrong tree*: the workflows run
> against whatever exists at flip time, so a green run today expires with the next merge.
>
> **This is the last item in this file.** When it lands, rule 3 says delete the file.
> `notes/backlog.md` item 100 carries a pointer to it so the release path does not depend on
> anyone rereading this one.

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

**Made cheap on 2026-08-18**: `ci.yml` now carries a `workflow_dispatch:` trigger
(`release.yml` already had one), so the rehearsal is a Run-workflow click on a chosen branch
rather than a scratch tag or a throwaway fork. Re-enabling Actions is still a billing
decision and therefore still the author's — nothing else here is blocked on anything but that.

- Done when: a rehearsal run of ci.yml and release.yml has completed once with its logs read,
  **on the tree that is about to ship**.
- Effort: S once Actions is on.
- Order: after the pre-flight `./tools/gates.sh` re-run, before item 100's Phase 2 (the
  irreversible half of the public flip). Both workflows have `workflow_dispatch`, so the
  sequence is: enable Actions → Run workflow on the release branch → read both logs → flip.

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
