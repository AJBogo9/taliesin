# The public flip audit

**Date:** 2026-07-28

The repository goes public. The commit history goes with it, on purpose: 1608 commits, all
authored and committed by one person, are the strongest available evidence of how that person
works. What does not go with it is a small set of private planning documents, and they have to
leave both the working tree and every commit that ever held them.

This spec defines a two-phase operation. Phase 1 is a read-only audit that produces a findings
document. Phase 2 rewrites history and creates the public repository, and runs only after the
findings are signed off.

> **Execution status: not started, and not to be started without a separate instruction.**
> The deliverable right now is the plan itself. Neither phase runs on the strength of this
> document. Phase 1 is read-only and safe whenever it is wanted; Phase 2 is irreversible and
> additionally gated on the Phase 1 findings being signed off.

## Starting state (measured 2026-07-28)

| Fact | Value |
|---|---|
| Remote | `git@github.com:AJBogo9/taliesin.git`, **private** |
| Forks / stars / issues / PRs | 0 / 0 / 0 / 0 |
| Created | 2026-06-15 |
| Commits | 1608, all `Andreas Bogossian <andreas.bogossian9@gmail.com>` |
| AI trailers (`Co-Authored-By: Claude`, `🤖`) | 0 |
| `.git` size | 68 MB |
| License | AGPL-3.0, README badge already present |
| Credentials in HEAD | none (the only password-shaped strings are `hunter2` fixtures in `crates/server/src/assets/_middleware.test.mjs`) |
| `git-filter-repo` installed | **no** (nor BFG) |
| `gh` token scopes | `repo`, `gist`, `read:org`, `admin:public_key`. **No `delete_repo`.** |
| Live worktrees | `.claude/worktrees/critique` (`critique-pass-2026-07-27`), `.claude/worktrees/item-77` (`item-77-residuals`) |

Zero forks and never having been public is what makes this cheap: no third party holds an object
from this repository, so there is no leaked-SHA problem to chase.

## Decisions taken

These were settled before this spec was written. They are inputs, not open questions.

1. **Purge policy: money and strategy documents only.** Security audits stay. AI-collaboration
   artifacts (`.claude/`, `docs/superpowers/`, `AGENTS.md`, `notes/LESSONS.md`) stay, because for
   the stated goal they are the exhibit rather than a liability. Corpus documents with personal
   provenance stay, subject to the D7 provenance check below.
2. **Purged documents are relocated, not destroyed.** They move to
   `~/Documents/personal/taliesin-private/` before anything is rewritten.
3. **Commit messages that name purged documents are rewritten** (roughly 11 subjects), so the
   public log has no visible seam.
4. **Remote strategy: archive plus fresh public.** `AJBogo9/taliesin` is renamed to
   `AJBogo9/taliesin-private-archive` and stays private as the complete backup. A new **public**
   `AJBogo9/taliesin` is created and receives the rewritten history. No destructive remote
   operation, no force-push, and the private blobs are never uploaded to the public repository at
   all. The cost is that the public repository's creation date resets to today; commit author
   dates are preserved, so the log and the contribution graph still show the real
   2026-06-15 onward timeline.
5. **Only `main` is published.** Merged feature branches are already reachable from `main` through
   their merge commits. The unmerged ones survive in the private archive.
6. **The security-audit class is reported, not purged.** The audit flags any still-open finding
   that reads as an exploit recipe so it can be judged individually. The default is keep.

## Phase 1: the audit

Read-only. Runs inline in one session, no subagents. Nothing in this phase touches git state.

**Output:** `notes/2026-07-28-public-flip-audit.md`, in the repository's existing audit format,
indexed in `notes/AUDITS.md`. Every finding carries path and line, class, quoted evidence, and one
recommended action from **purge / redact / fix / keep**. The point of that shape is that Phase 2
becomes mechanical: the judgement all happens here, under review, before anything is irreversible.

### D1. The purge set, across all history

Enumerate every path matching the money-and-strategy policy in the full object graph, not just in
`HEAD`. Known members:

- `notes/STARTUP-PLAN.md` (3 commits) and the earlier root-level `STARTUP-PLAN.md` (2 commits)
- `notes/FUNDING-RESEARCH.md` (2 commits)
- `notes/2026-07-18-pmf-audit.md`
- `notes/2026-07-27-due-diligence-audit.md`
- `notes/2026-07-28-demand-positioning-audit.md`
- `notes/2026-07-28-launch-critique.md`
- `notes/2026-07-27-adoption-friction-audit.md`

The first two self-declare their status in their own opening paragraphs ("keep it out of any public
release", "keep this file out of git") and were committed anyway, which is the whole reason this
operation exists.

Deleted-and-gone predecessors must be **read, not assumed innocent**: `todo.md`, `PLAN.md`,
`PROBLEM.md`, `AUDIT-2.md`, `AUDIT-BACKLOG.md`, `BEYOND-QUARTO.md`, `visual-ux-audit.md`. They
predate the `notes/` convention and may carry the same material under a different name.

### D2. Leaked content inside kept files

The dimension a path-level purge misses, and the reason this is an audit rather than a
`filter-repo` one-liner. Ten tracked files reference the purge set by name:

`docs/internals/repository.tmd`, `docs/superpowers/plans/2026-07-27-audit-wave-1.md`,
`docs/superpowers/specs/2026-07-03-quarto-design-decisions-catalog.md`,
`docs/superpowers/specs/2026-07-27-audit-slate-design.md`,
`notes/2026-07-28-conformance-acr-audit.md`, `notes/2026-07-28-deck-exemption-audit.md`,
`notes/2026-07-28-first-contact-audit.md`, `notes/AUDITS.md`, `notes/README.md`,
`notes/backlog.md`.

`notes/backlog.md` additionally carries 4 lines of its own pricing/monetize content.

Read every hit and classify it:

- **A link or an index entry** pointing at a purged file. Cosmetic. Fixed by one ordinary repair
  commit on top of history.
- **Restated private content**, for example a backlog item quoting a pricing conclusion or an
  index summarising a PMF finding. Not cosmetic. Must be removed **across all history** with
  `--replace-text`, because a repair commit leaves the original text readable in the parent commit.

That distinction drives the Phase 2 mechanism and is the single most load-bearing output of the
audit.

### D3. Commit messages

Roughly 11 subjects name purged documents (`commit startup plan`, `docs: add funding research notes
for qmd-fast`, `docs: add effort vs reward framing to funding notes`, the PMF batch entries, and
`docs(notes): close #2 + #4, and record that pricing lies in BOTH directions`). Produce the exact
`--replace-message` substitution list, one entry per affected subject, with replacement text that
is truthful about the commit's remaining content rather than blank.

### D4. Secrets and credentials, full history

`HEAD` is clean, but a key could have lived in a file that was later deleted. Pattern-scan every
blob in the object graph, not the checkout. Vendored and minified assets (`*.min.js`,
`plot.umd.min.js`, `d3.min.js`, KaTeX, `Cargo.lock`) are excluded from the signal, since they
produce nothing but false positives.

### D5. Personal data

- `/home/bogo` absolute paths appear in 14 tracked files. The question is not the username, which
  is harmless, but whether any path names an unrelated project (school coursework, client work)
  and thereby discloses something about work outside this repository.
- Home-server, Tailscale, LAN hostnames and IP addresses, which the `--host` feature and the
  relocated STARTUP-PLAN both touch.
- Any real person other than the author.
- **No git author-email rewrite is needed.** `SECURITY.md` already publishes
  `andreas.bogossian9@gmail.com` deliberately as the vulnerability-report fallback, so the address
  is already an intended public contact.

### D6. Third-party components and licensing

`THIRD_PARTY.md` and `crates/core/assets/js/LICENSES.md` against what is actually vendored:
Mermaid, d3, Observable Plot, KaTeX, the Newsreader font, the PowerShell syntax definition,
`corpus/graphics3d/assets/ToyCar.glb` (5.2 MB), and `corpus/liquid-glass-slides`. Verify AGPL-3.0
compatibility and attribution completeness. An incomplete notice on a public AGPL repository is a
real problem rather than a cosmetic one. `ToyCar.glb` (a Khronos sample asset) and the liquid-glass
extension get the hardest look, since both originate outside this project.

### D7. Corpus provenance

`corpus/README.md` records that documents were copied from `personal/blog`, `personal/tech-blog`,
and `personal/bayesian-fatality-analysis`. Confirm three things: no co-author's work is being
republished under this project's AGPL without permission; no course-submitted material is exposed
in a way that carries academic-integrity implications; and the fatality dataset contains no
personal data about real individuals.

### D8. Tone toward named third parties

The notes criticise Quarto, Posit, Curvenote, and Stencila across many rounds. Technical criticism
of a tool is fine and stays. Flag anything that reads as a swipe at a named person.

### D9. Public-repository readiness

- The README's first-run path, executed as a stranger with none of the local setup.
- The `SECURITY.md` advisory URL, which only resolves once the repository is public.
- `.claude/settings.json` allows `python3 scripts/corpus_diff.py`, and `scripts/` no longer exists.
- The GitHub Actions workflow was deleted on 2026-07-26, so a public repository shows no green
  check. Better to state the local gate in the README than to look untested.

### D10. History hygiene (optional, recommended)

`.git` is 68 MB. A rewrite is already touching every commit, so this is the one free moment to drop
superseded large blobs: the pre-rename `site/assets/live-edit.mp4` and `live-code.mp4`, the
superseded 2.5 MB copy of `mermaid.min.js`, and the duplicated 1.1 MB
`2026-07-03-quarto-design-decisions-catalog.md`. Zero extra risk once `filter-repo` is running, and
it is skippable without affecting any other dimension.

## Phase 2: execute

Irreversible. Runs only after the Phase 1 findings are signed off.

### Preconditions

- No other Claude session is operating in this tree. This is not hypothetical: during the writing
  of this spec, `HEAD` moved from `integration-2026-07-28` to `main`, `main` fast-forwarded to
  `e3b369e`, and `origin/main` was pushed, all from another session.
- Both `.claude/worktrees/` worktrees are removed. They hold `critique-pass-2026-07-27` and
  `item-77-residuals`, which must be merged or explicitly abandoned first.
- `main == origin/main`.
- The `.githooks/pre-push` gate is green: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- `git-filter-repo` is installed.

### Steps

1. **Relocate.** Copy the D1 purge set to `~/Documents/personal/taliesin-private/`. Verify each
   file is readable at the destination before anything is removed.
2. **Back up, twice.** `git clone --mirror` into scratch, and rename the GitHub repository to
   `taliesin-private-archive`, which is itself the authoritative backup and stays private.
3. **Repair.** Apply the D2 link fixes and the D9 readiness fixes as ordinary commits on `main`.
   Push them to the archive. These are honest history and are not hidden.
4. **Rewrite, on the mirror only.** `git-filter-repo` with `--invert-paths --path` for D1,
   `--replace-message` for D3, `--replace-text` for the D2 redactions, and the optional D10 blob
   removals. The working clone is never rewritten in place.
5. **Verify before any push.** This step is the test suite, and it gates everything after it:
   - every purged path returns zero hits across all refs and all history
   - every redaction string returns zero hits across all history
   - commit count and author-date range are as expected
   - a **fresh clone of the rewritten mirror** passes `cargo fmt --all -- --check`,
     `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`
   - the corpus renders from that fresh clone
6. **Create the public repository** and push `main` to it. It is created public, so it is never
   private-then-flipped and never holds a private object.
7. **Verify on github.com.** No purged path resolves. The README renders. The `SECURITY.md`
   advisory link works. A fresh `git clone` of the public URL contains only what was intended.
8. **Re-point local.** Move the working clone to the new remote and recreate any worktrees.

### Recurrence guard

Deliberately minimal:

- One line in `CLAUDE.md`: money and strategy notes live in `~/Documents/personal/taliesin-private/`
  and never in the repository.
- A short filename check in the existing `.githooks/pre-push` that refuses a push containing a
  file matching the purge policy.

## Risks and accepted consequences

| Risk | Handling |
|---|---|
| A parallel session pushes mid-rewrite, making the rewrite stale | Freeze first, re-verify `main == origin/main` immediately before step 4 |
| Live worktrees are invalidated by the rewrite | Removed in the preconditions, with their branches merged or abandoned |
| A purged document is needed later | It exists in `~/Documents/personal/taliesin-private/` and in the private archive repository |
| The verification step passes vacuously (greps that match nothing because the pattern is wrong) | Each assertion in step 5 is first run against the **un**rewritten mirror and must produce a non-zero count there, proving the pattern matches before zero means anything |

**Accepted:** every SHA changes. Notes that cite commits (`@ 6ed95a9`, `@ a121df2`, and the rest)
and any external record of a commit hash will point at commits that no longer exist. This is not
repaired. The archive repository preserves the original hashes if one is ever needed.
