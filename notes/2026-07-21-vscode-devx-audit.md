# VS Code companion DevX audit (2026-07-21)

**Prompt:** compare Taliesin's editor integration to what Quarto 2 (the Rust rewrite,
`quarto-dev/q2`) is promising, and find where the Taliesin VS Code companion can improve
the authoring developer experience (real-time errors, autocompletion, YAML validation).

**Context:** Quarto 2's headline pitch is *"a new Markdown parser for real-time error
messages, autocompletion, and YAML validation across the entire project"* plus a
collaborative visual editor (automerge). Taliesin already has the parser (comrak + block
model) and a deep validator suite in Rust; the gap is not validation *depth* but how
*live* and how *rich* the editor surface is. The **single-editing-surface** invariant
(browser read-only; the `.tmd` file is the only authoring surface) is the argument *for*
this work: the editor is the only place authoring happens, so editor DevX is where
authoring quality lives. The collaborative/visual-editor half of Quarto 2 is explicitly
*not* a goal (it requires multiple write paths into one document, which the invariant
forbids).

## What already exists (do not rebuild)

- **Autocompletion is already good.** `editor/vscode/src/completions.ts` +
  `complete.ts`: front-matter keys (incl. nested under `execute:`/`about:`/`hero:`/…),
  cell-option keys, `:::{.` div classes, `@`-cross-reference targets (merged live-buffer
  scan + `taliesin symbols`), `[@` citation keys read from the front-matter
  `bibliography:` `.bib`, and `{{< embed/include >}}` path completion. Vocabulary is
  Rust-authoritative via `taliesin vocab`. Triggers on `@ . | - /`.
- **Validation is deep — arguably deeper than Quarto 2's pitch.** `crates/server/src/
  check.rs` + `taliesin_core::diagnostics`: malformed YAML, unknown/typo front-matter
  keys, broken xrefs, duplicate heading ids, dangling anchors, missing local
  assets/media, broken links (site-aware cross-page + `mounts:`-aware), the `{js}`
  reactive dependency graph (unknown inputs, cycles), a11y (alt text, heading-level
  skips, unnamed links/buttons), math, citations-without-bibliography, typo'd
  categories. Every finding is agent-grade: stable `TAL-*` `code`, `severity`,
  `docs_url`, and a structured `suggestion { replacement }` for "did you mean".
- **The live preview already shows diagnostics** (`crates/server/src/preview_diag.rs`,
  DX1): the same check-superset is pushed into the browser dev menu.

## Findings

### The core problem: on-save, and lossy

1. **Editor diagnostics refresh only on open/save, never on-type.**
   `editor/vscode/src/diagnostics.ts` listens for `onDidOpenTextDocument` +
   `onDidSaveTextDocument` only — no `onDidChangeTextDocument`. This is the "I don't have
   real-time error messages" feeling.
2. **`taliesin check` reads from disk, not the buffer** (`check.rs`,
   `read_to_string(path)`). Even with on-type refresh, it would lint the last-*saved*
   file, not unsaved edits. Real-time linting needs a buffer/stdin input mode (or the LSP
   path below).
3. **The bridge is lossy.** `check.ts` `CheckDiag` parses only `{file, line, message}`,
   dropping `code`, `severity`, `docs_url`, and `suggestion`. Then `diagnostics.ts`
   hard-codes `DiagnosticSeverity.Warning` and a whole-line range. The Rust side emits
   rich structured diagnostics; the editor renders them all as identical yellow
   whole-line squiggles. **The Problems panel is behind the tool's own preview.**

### Missing LSP-grade features

4. **No column-accurate ranges** — whole-line squiggles because the check JSON carries no
   column.
5. **No hover** — no `HoverProvider` to resolve `@fig-2` → "Figure 2", show a
   front-matter key's doc, or preview a `[@key]` reference. `vocab`/`symbols` already
   carry the data.
6. **No document outline / go-to-definition** — `taliesin symbols` exists but is not
   wired to a `DocumentSymbolProvider` (outline/breadcrumbs) or `DefinitionProvider`
   (`@fig-x` → figure, `{{< include x.tmd >}}` → file, `[@key]` → `.bib` entry).
7. **No front-matter *value* completion** — `format:`/`theme:`/etc. complete the key but
   not the value (`detectContext` in `complete.ts` has no `frontmatter-value` case).

### The strategic bet

8. **No `taliesin lsp` server.** A `taliesin lsp` subcommand speaking LSP over stdio
   would hold the parsed doc warm, receive `didChange` with full buffer text (solving
   on-type + unsaved-buffer in one move), and unify diagnostics + hover + definition +
   symbols + completion + rename behind one protocol that works in any LSP editor. This
   is Quarto 2's exact headline, and for Taliesin it is *wiring existing engine parts*
   (parser + validators already in Rust), not a rebuild. It subsumes findings 1, 2, 4,
   5, 6.

## Recommended sequencing

- **Tier 1 (this session):** stop discarding `severity`/`code`/`docs_url`/`suggestion` in
  the bridge — real Error/Warning severity, `Diagnostic.code = {value, target: docs_url}`
  (Ctrl-click to catalog), and a `CodeActionProvider` quick-fix from `suggestion`. Small,
  no architecture change, backend already emits it.
- **Tier 2:** on-type diagnostics (`check` buffer/stdin mode + debounced
  `onDidChangeTextDocument`), column-accurate ranges, hover, outline + go-to-definition,
  front-matter value completion.
- **Tier 3:** `taliesin lsp` — its own spec/brainstorm; subsumes most of Tier 2.

Each increment pins via the extension's `node:test` harness (`editor/vscode/src/test/`)
+ the Rust `check` corpus (`corpus/diagnostics/`), the editor-side analogue of the
corpus-plus-roadmap discipline.
