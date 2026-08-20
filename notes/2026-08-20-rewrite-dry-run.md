# History rewrite: dry run, 2026-08-20

Both rewrites executed end to end on throwaway mirror clones under `/tmp`. **Nothing was
pushed, and `git filter-repo` never touched the working repository.**

The exact `--path` and `--replace-text` arguments are NOT reproduced here. They live in the
gitignored execution workspace, deliberately, because the argument list concentrates a third
party's name, a university's copyrighted text and the author's commercial figures into one
file. This document records paths, counts and verdicts only.

## Prerequisite: the tooling was broken, and the backlog said it was fine

`notes/backlog.md` item 100 recorded "Phase 2's tooling prerequisite is discharged:
`git-filter-repo` 2.47.0 is installed at `~/.local/bin/git-filter-repo` (verified 2026-08-05)".

It was not usable. The symlink pointed into `~/snap/code/254/`, while pipx had moved to
`~/snap/code/257/` after a snap revision bump, and the venv's own shebang pointed at the
vanished 254 interpreter, so the package was installed and completely non-functional. `pipx
list` reported it as "symlink missing or pointing to unexpected location". Repaired with
`pipx reinstall git-filter-repo`.

**This is the second recorded prerequisite in item 100 that turned out to be rot**, after the
`corpus/bayesian-website/` gap in the purge table. Verify prerequisites by running them, not
by reading the note that says they were verified.

## Rewrite A: the private archive

Purges the third-party-rights material only. The money and strategy documents stay, because
they are wholly the author's and destroying the last copy buys nothing.

| Purged path | Result |
|---|---|
| `corpus/bayesian-website/` | 0 commits |
| `corpus/bayesian-book/` | 0 commits |
| `corpus/expected/bayesian-book.html` | 0 commits |
| `notes/2026-08-20-purge-enumeration.md` | 0 commits |

Plus the Class 1 text replacement. **The co-author's surname returns 0 hits across every blob
of every commit** in the rewritten history.

## Rewrite B: the new public repository

Rewrite A's set plus the seven money and strategy documents at both their `notes/` and
pre-move root spellings, `todo.md`, and `notes/2026-07-28-public-flip-audit.md`.

**All 16 purged paths return 0 commits.**

Sensitive-content sweep across the whole rewritten object graph, by `git log -S`:

| String class | Hits |
|---|---|
| the co-author's surname | 0 |
| the university IP-policy phrase | 0 |
| competitor revenue figures | 0 |
| the estimate source's name | 0 |
| the assignment-brief platform name | 0 |
| copyright-transfer framing | 0 |
| hosted-platform framing | 0 |
| the moat framing | 0 |

### Structural integrity

| Check | Result |
|---|---|
| Commits, all refs | 2,107 (from 2,155; 48 pruned as empty) |
| Authors | one, unchanged |
| Earliest commit | `321b658d`, 2026-06-15, "Initial commit", intact |
| `cargo test -p taliesin-core --no-fail-fast` | **50 suites, 827 passed, 0 failed, 0 ignored** |
| `build docs/guide --check-only` | no static problems found, exit 0 |
| `build docs/internals --check-only` | no static problems found, exit 0 |
| `docs/guide/using/figures/loss.png` | present (the asset the CI rehearsal rescued survived) |

## Three findings that change Phase 2

### 1. A long-literal replacement missed, and only a broad sweep caught it

The enumeration's Class 2 list matched on long distinctive sentences. One of them was
transcribed with a one-word difference from the real text ("carry" where the file says
"contain"), so it matched nothing, and the phrase survived in 503 blobs of the first Rewrite B
attempt. A second occurrence used a different construction again.

**Lesson for Phase 2: verify by sweeping for a SHORT distinctive key, never by trusting that
the long literal matched.** The fix was to redact on the short two-word key, which is
unambiguous here (the author's own CV names the university alone, which is legitimately
public and must not be redacted).

### 2. The redaction list handled the surname but not the given name

Caught by the whole-branch review, after the first dry run reported clean. The Class 1 list
held the co-author's SURNAME only, so a rewrite that removed every trace of the surname still
published "[REDACTED-NAME] ***REMOVED***" in `notes/AUDITS.md` and in the historical
`crates/core/src/site/cite_this.rs` fixture, both of which are kept rather than path-purged.

Worse, a test fixture stored the given name on its own line as `given: Some("[REDACTED-NAME]".into())`,
which neither full-name form (`[REDACTED-NAME]`, `[REDACTED-NAME]`) matches, so even adding those
left it behind. **The bare given name had to be listed too.**

Two lessons, both now recorded in the enumeration document:
- **`--replace-text` order is load-bearing.** filter-repo applies entries in file order, so
  full-name forms must precede the bare surname or the longer forms never match.
- **Redacting a person requires every form their name takes**, including the ones split
  across structured fields where the parts never appear adjacent.

Re-verified after the correction: `[REDACTED-NAME]`, `[REDACTED-NAME]`, `***REMOVED***`, `FeedbackFruits` and the
competitor figures all return 0 across every commit of the rewritten history (2,116 commits).

### 3. Stale branches would publish

The mirror carries roughly 40 local branches, most of them finished feature branches
(`cut/wave-1-antidrift`, `batch-9-cli-correctness`, and so on). A mirror push publishes all of
them. **Phase 2 must decide explicitly** whether the public repository gets every branch or
only `main`. Publishing 40 stale branches is untidy rather than dangerous, but it is a
decision, not a default.

Also: the throwaway `rehearse-2` branch used for the workflow rehearsal must be deleted before
the flip, or it publishes too.
