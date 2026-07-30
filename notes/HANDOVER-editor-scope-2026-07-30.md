# Handover: the editor scope, 2026-07-30

Paste the block below into a fresh session.

---

Continue the Taliesin editor-scope work. Branch **`editor-scope-completion-2026-07-30`**,
16 commits ahead of `origin/main`, working tree clean, **not pushed**.

## What is done

The VS Code / LSP surface opened by `notes/FEATURE-IDEAS.md` Session 3 is **closed**. Nine ideas
shipped, four were cut on measured evidence, one was dropped by owner ruling, one stays filed
elsewhere. Spec and plan are in the repo and are the record of why:

- `docs/superpowers/specs/2026-07-30-editor-scope-completion-design.md`
- `docs/superpowers/plans/2026-07-30-editor-scope-completion.md`

**Shipped:** 75 (cross-file go-to-definition + hover), 76 (workspace symbols), 77 (sidebar, three
read-only views), 78 (Explorer check badges), 79 (status bar), 80 (task provider + two problem
matchers), 85a (language-model tools), 85b (MCP server definition provider). All on one new
substrate, `crates/server/src/lsp_project.rs`.

**Cut, with the reason written into `FEATURE-IDEAS.md` so none is re-proposed:**

- **67** (semantic tokens): idea 75 removed its last justification. Its only surviving case was
  distinguishing states "only one of which is reachable by go-to-definition"; both are reachable now.
- **74** (project index, L, needs-care): never needed. Every surface it was to enable fires on a
  *gesture*, so all of them want a walk. `lsp_project.rs` is one walk behind a memo validated by
  `stat`ing each page for `(mtime, len)`: no watcher, no invalidation protocol.
- **83** (URI handler): premise was rot. `web-client/client.js` has navigated the standalone
  browser to `vscode://file<abs>:<line>:<col>` all along.
- **72** (colour provider): premise was false. **No `--tali-*` token is authored in any `_site.yml`
  or front matter anywhere in the repo**; they live in a `theme:` CSS file, where VS Code's bundled
  `css-language-features` already provides `findDocumentColors`/`getColorPresentations`, oklch included.

**81** (Testing API) was dropped by owner ruling in favour of 80. **86** (cell CodeLens) stays filed
as backlog item **175(d)**, blocked on 175(b) output streaming. It is the only editor-surface idea
still open anywhere.

## Verification actually run (do not re-credit, do re-run after any change)

- **`./tools/gates.sh` PASSED, exit 0, every gate ran**, invoked as
  `TALIESIN_PYTHON=$HOME/.local/share/qmd-venv/bin/python TALIESIN_R=R CARGO_BUILD_JOBS=4 ./tools/gates.sh`.
  Without `TALIESIN_PYTHON` it refuses to start (no ipykernel on `python3`), which is correct.
- `cargo test -p taliesin-server --bin taliesin`: **678 passed, 0 failed**. Companion unit suite:
  **145 passed, 0 failed**. All three `tsc` checks clean.
- **Companion e2e: 33 passing, 2 failing.** The two failures are `pressEnterAfter` list-continuation
  tests and are **PRE-EXISTING**: a worktree at `origin/main` fails the identical two (27 passing /
  2 failing). They failed at load ~2 to ~3.4, **not** the "load ~6-7" the backlog previously
  recorded, so that threshold is understated.
- Every behaviour change was mutation-verified (restore the bug, watch the named test fail).

## What is left

1. **Hand-check the packaged extension.** The e2e proves VS Code accepted the sidebar views, the
   language-model tools, workspace symbols, cross-file definition and the two custom requests. It
   **cannot** observe the task provider (measured: a folderless host returns zero tasks of any type),
   the Explorer decorations, or the status bar item. Package, install, reload, and look at:
   the sidebar's three views on `docs/guide`, a badge in the Explorer, the status bar item, and
   `Tasks: Run Task` offering check / build / build --out. **Uninstall any older companion first**
   and remember a stale `.vsix` has silently shipped a fixed bug here before.
2. **Push, only if the author asks.** `git push origin editor-scope-completion-2026-07-30:main`.

## Traps this batch hit, in priority order

- **`cargo test` does NOT rebuild `target/debug/taliesin`**, which is the binary the e2e drives. A
  real fix looked broken for two runs because of it. `cargo build -p taliesin-server --bin taliesin`
  before believing any e2e result.
- **`git checkout <file>` to undo a mutation destroyed uncommitted work** (it restored `package.json`
  from HEAD, wiping the manifest edits). Use a file backup in the scratchpad instead. This is already
  in `LESSONS.md`; it still cost time.
- **A surviving mutant is a finding about the test.** Two here. Deleting the `is_xref_anchor` gate
  stayed green because the fixture's only non-anchor `@` was an email address already rejected by
  the *site* predicate; it needed `@handle`. Deleting `needle.is_empty() ||` in `workspace_symbols`
  also stayed green, and that one was right: `contains("")` is true for every string, so the guard
  was dead code and was removed.
- **The e2e found a real bug three green suites missed.** `workspace/symbol` names no file, and the
  handler took the first key of a **hash map** to stand for "the project", so Ctrl+T searched an
  arbitrary one of two open projects. Fixed and pinned.
- **`taliesin check`'s human format was mis-transcribed in `FEATURE-IDEAS.md`.** Severities are
  lowercase (`error`/`warning`/`suggestion`) and codes are `TAL-XREF-UNDEF`-shaped. There is also an
  **unlocated** `file: severity[CODE]: message` form with no line, needing its own matcher.
- **`enclosing_site_root` existed twice and the two disagreed** at a `.git` boundary. Now one walk
  with the boundary as a named parameter, both behaviours preserved and pinned.
- **The engine floor is `^1.101.0` with `@types/vscode` pinned exactly `1.101.0`.** Measured by
  packing both candidates. Never put a caret on the types: it resolves to latest and re-opens the gap.

Three new rows are in `notes/DETECTION-DEBT.md` for what cannot be observed: cell cache/execution
state in a badge, live kernel state in the status bar, and the three registrations no API exposes.
