# Handover: feature-audit backlog, Phases 4-5

Phases 0-3 of `notes/2026-08-20-feature-audit-backlog.md` are **done and on main**
(10 commits, `./tools/gates.sh` PASSED 12/12). What follows is for the session that
picks up Phase 4.

## Start here

> Read `notes/2026-08-20-feature-audit-backlog.md` in full, including its "How to use
> this file" section, and read `notes/2026-08-20-handover-phase4.md` (this file) for
> what the last session learned the hard way. Phases 0-3 are already on main; do not
> redo them. Implement **Phase 4 (C7a-C7e) as ONE coordinated commit**, then STOP and
> report. Do not start Phase 5 (C8) without a go-ahead: it needs its spot-check verdict
> first. Never touch X1 (shared bibliography) - it is ruled frozen pending the author's
> explicit written override. Work on a branch off main; do not commit on main.
> Line numbers in the backlog are reads of the 2026-08-19/20 tree and HAVE drifted -
> re-locate every one by symbol or search string, and verify each claim against the
> current code before acting on it.

## What Phase 4 is

`C7a-C7e`, five VS Code companion features, in **one commit** because
`editor/vscode/src/test/manifest.test.ts` asserts cross-references between commands,
walkthrough steps and contributions - three generic gates fail on *partial* removal.
Either land it as one commit, or in an order that keeps `node --test` green at every step.

- **C7a** first-kernel-failure doctor hint (`doctorhint.ts`, `kernelfail.ts`, its drift
  test, and the registration pair in `extension.ts`)
- **C7b** build/check tasks + Problems-panel matchers (`tasks.ts`, `taskspecs.ts`,
  `test/tasks.test.ts`, manifest `taskDefinitions`/`problemMatchers`, wiring, and the
  docs section in `docs/guide/using/preview.tmd`)
- **C7c** Diagnose Setup command (`commands.ts` is the whole 33-line file)
- **C7d** Get Started walkthrough (manifest contribution + steps, three markdown pages,
  and the `manifest.test.ts` walkthrough block - it asserts `walkthroughs.length > 0`,
  so it MUST go in the same commit)
- **C7e** bundled `_site.yml` JSON schema copy (`editor/vscode/schema/`, the
  `contributes.yamlValidation` block, its node gates). The crate's own schema survives,
  golden-locked by `site_schema_matches_committed`.

**Also required by C7e:** CLAUDE.md's paragraph saying a new front-matter key trips
**FOUR** drift gates must become **three** - the schema-copy gate is the one being cut.
Check `docs/guide/reference/cli.tmd` for rows mentioning tasks or the walkthrough, and
reword `docs/guide/reference/frontmatter.tmd`'s claim that the companion "wires it up
for you". Do NOT add `yaml-language-server` modelines to the four `_site.yml` files.

**KEEP explicitly:** `taliesin.restartServer`, `taliesin.showServerLog`, both default
keybindings, `termlinks.ts`, the preview webview, embedded completion, the grammar,
snippets.

## Read this before you trust the backlog's checklists

The single most useful thing the last session learned:

> **Every task's "Removal surface" list was INCOMPLETE.** Not once, every time. Sweep for
> extra consumers yourself before you start, and expect to find them.

Concretely, per task: C1 had two undocumented docs rows (one a WCAG 3.1.1 conformance
claim) plus a corpus witness interaction nothing mentioned; C2 had a cheatsheet
paragraph, a stale const reference and a now-vacuous assertion needle; C3 had four
`Page`/`FrontInfo` construction sites and two extra docs examples; C4's explicit
"do NOT touch `parse_pandoc_attrs`" rested on a **false premise** (its stated reason
named consumers that use a different, similarly-named helper); C5 and C6 each orphaned
test fixtures that would have failed clippy.

None of these were caught by reading the backlog. All were caught by grepping the tree.

## Traps that cost real time

1. **`cargo build` is NOT a sufficient compile check.** It does not compile
   `#[cfg(test)]` fixtures. C3 removed a struct field, `cargo build` passed, and
   `crates/core/src/site/links.rs` only failed later under the test build. Use
   **`cargo test --workspace --no-run`**.

2. **`cargo clippy --workspace --all-targets -- -D warnings` is a real gate** (pre-push
   [2/5]). Any function left dead by a cut fails it. A `pub` item does NOT fail it - so
   dead *public* API is invisible; grep for callers rather than trusting the compiler.

3. **The portability census only runs in `gates.sh`.** Not in `cargo test`, not in the
   pre-push hook. Any commit that changes lines under `corpus/` stales the published
   figures in `README.md` and `docs/guide/using/choosing.tmd`, and `cargo test` stays
   green while they are wrong. Phase 4 is editor-only so it probably will not trip this,
   but check if you touch corpus.
   Fix: `python3 tools/portability-census.py` and copy its output into both files.

4. **`the_reference_page_documents_every_known_key` walks KNOWN_KEYS -> docs only.** A
   docs row for a *removed* key fails nothing. Docs rows need a manual sweep.

5. **The corpus fixture `corpus/diagnostics/typos.tmd` is position-sensitive.**
   `crates/server/tests/lsp_stdio.rs` hovers at a hard-coded 0-based line. Two cuts in a
   row each removed a front-matter line and each time the position had to follow. There
   is a "fixture moved" assert above it that turns the drift into a clear failure - keep
   that pattern if you touch the file.

## Verification (non-negotiable)

- `export TALIESIN_PYTHON="$PWD/.venv/bin/python"` before `gates.sh`. The pre-push hook
  now **self-arms** this (T2, landed this session), so a bare `git push` no longer
  silently skips the live-kernel tests - but `gates.sh` still needs it in the env.
- **Never run two cargo test suites concurrently** - they deadlock. Kill stale runs first.
- Run **`cargo fmt --all` LAST**, after all `.rs` edits in a task.
- Per commit: `cargo test -p taliesin-core` plus the task's named tests. For Phase 4 the
  companion's own suite is the one that matters: `cd editor/vscode && npm test` (that is
  what `gates.sh` runs; plain `node --test` from the repo root does NOT work).
- Before pushing: `./tools/gates.sh`, and **take the gate count from its own verdict
  line**, never from prose.
- C7's stated manual check: open VS Code once and confirm the companion still activates,
  the preview opens, and diagnostics squiggle.

## Open decisions the author has NOT made

Do not decide these unilaterally; raise them if they get in the way.

1. **The advice tier now has no producer.** C6 cut the uncited-entry lint, which was the
   sole non-test producer of `Severity::Suggestion`. The six remaining mentions are all
   consumers (LSP HINT mapping, `is_advice`, the label, the grey colour, the summary
   count, the `--strict` filter). ~40 LOC, harmless dormant. Deleting the tier is a
   separate scope question. Recorded in commit `2ca2b4db`.
2. **`cite::cited_keys_in_source` has no caller outside its own test** after C6. It is
   `pub`, so nothing flags it. Removing public API is a scope call.
3. **R8 (optional, not taken):** four of the seven bundled social-icon glyphs
   (x/twitter, mastodon, bluesky, email) are used by no project - only `github` and
   `linkedin` are. Verified that no validator or schema enumerates icon names, so the cut
   is clean whenever wanted.
4. **R9 (note only):** `tools/subset-fonts.sh`'s fontTools pin has never been re-verified
   against the on-disk woff2 bytes. Regenerate and diff before the NEXT font bump.

## Standing rules that still bind

- **The ordering rule is absolute**: a feature's code, tests/pins, docs rows and corpus
  fixtures die in the SAME commit. A corpus document deleted ahead of its code leaves
  that code silently unguarded while every gate still passes.
- **The parser-side pin rule**: withdrawing a construct means deleting the READ, not just
  the vocabulary entry. Add a test asserting the read is gone.
- **No retirement registers, no compatibility notes, no "did you mean another tool's
  key"** (standing ruling 2026-08-17).
- **Do not touch**: `MAX_WARM_PAGES` + the ExecPool LRU, the 7-item list in
  `notes/native-rewrite.md`, anything in `notes/DO-NOT-REBUILD.md`.
- **Do not extend any WATCH feature.** The audit froze 51 of them.
