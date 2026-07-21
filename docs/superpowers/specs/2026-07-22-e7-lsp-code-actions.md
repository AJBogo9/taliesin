# E7 capability 6: `taliesin lsp` quick-fix code actions (design + plan)

**Date:** 2026-07-22 · **Status:** approved, lean process (combined design+plan) ·
**Follows:** the E7 diagnostics / go-to-definition / outline / hover / completion capabilities
on the same `taliesin lsp` server.

## Goal

Add `textDocument/codeAction` to `taliesin lsp` so a diagnostic that already carries a
"did you mean `X`?" fix becomes a one-click quick-fix in any LSP editor, exactly as the VS
Code companion's `diagnostics.ts` code-action provider does today: a front-matter key typo
(`tittle:` → `title:`) offers **Change to `title`**, replacing the mis-typed token. This
closes the diagnostics loop the E7 slice opened: the squiggle you can already see becomes the
squiggle you can fix. Read-only w.r.t. the preview; the edit flows through the editor (the
legitimate editing surface), never the preview.

## Where the fix comes from (already computed)

`check::Diagnostic` already lifts an inline "did you mean" hint into a structured
`suggestion { replacement }` (`Diagnostic::new` via `codes::extract_suggestion`), and E3 gave
the front-matter-key-typo warning a precise 1-based `[col, end_col)` span. So the fix and the
exact token to overwrite are both in hand; this capability only carries them to the editor.

## Design

The fix rides on the LSP diagnostic's `data` field — the round-trip channel LSP added for
exactly this (the client echoes each diagnostic, `data` included, back in the code-action
request's `context.diagnostics`). Two pieces:

1. **`check::Diagnostic::to_lsp`** attaches `data: { "replacement": <text> }` **only when the
   diagnostic has both a `suggestion` and a precise `col`/`end_col`**. That guard is
   load-bearing: `data` present ⟺ the diagnostic's `range` is exactly the token span, so a
   consumer can replace `range` with `replacement` without guessing. A suggestion without a
   column (should not occur for key typos, but be safe) attaches nothing, so no imprecise fix
   is ever offered — mirroring the companion's "couldn't locate the token unambiguously: offer
   nothing".
2. **`lsp.rs` `resolve_code_actions`** reads `params.context.diagnostics`; for each whose
   `data.replacement` is a string, it emits a `CodeAction { title: "Change to \`{replacement}\`",
   kind: QuickFix, edit: replace diag.range with replacement, diagnostics: [diag], is_preferred:
   true }`. No re-render, no buffer scan: the echoed diagnostic already carries the range + fix.

This is leaner and more robust than recomputing diagnostics per request, and works for every
spec-compliant client (VS Code, Neovim, Helix, Zed all round-trip `data`).

## Architecture

- **`crates/server/src/check.rs`** — extend `to_lsp` to set `data` under the guard above
  (reads its own private `suggestion.replacement`, so nothing new is exposed cross-module).
- **`crates/server/src/lsp.rs`** — advertise `code_action_provider: Simple(true)`; route
  `CodeActionRequest::METHOD` in `handle_request` to `resolve_code_actions(&params) ->
  Option<CodeActionResponse>`.
- No new module: the whole capability is a `to_lsp` guard + one request handler.

## Plan (TDD)

### Commit 1 — spec doc (this file)

### Commit 2 — `to_lsp` carries the fix on `data`

- Failing unit test in `check.rs`: a columned, suggestion-bearing `Diagnostic` → `to_lsp().data`
  is `Some({ "replacement": "title" })`; an uncolumned suggestion-bearing one → `data` is
  `None` (no imprecise fix); a no-suggestion one → `data` is `None`.
- Implement the guarded `data` assignment in `to_lsp`. Run RED → GREEN.

### Commit 3 — `textDocument/codeAction`

- Advertise `code_action_provider`; route `CodeActionRequest::METHOD`.
- `resolve_code_actions`: build a QuickFix per `context.diagnostics` entry carrying
  `data.replacement`; `WorkspaceEdit { changes: { uri: [TextEdit { diag.range, replacement }] } }`.
- Failing integration pin (in-process `Connection::memory()`): `didOpen` a doc with a
  front-matter typo (`tittle: Hi`), take the published diagnostic (which now carries `data`),
  send `textDocument/codeAction` with that diagnostic in the context → assert one QuickFix
  titled `Change to \`title\`` whose edit replaces the token range with `title`.

## Verify

`cargo test -p taliesin-server` (serial if the exec/kernel flake trips), `cargo fmt --check`,
`cargo clippy -p taliesin-server --all-targets`, and a real-process smoke: spawn `taliesin lsp`,
`didOpen` a typo doc, read the published diagnostic, send `textDocument/codeAction`, confirm the
quick-fix + its `WorkspaceEdit` on stdout. Then narrow the backlog E7 item (code actions shipped).

## Non-goals

Rename (the last E7 capability); fixes for diagnostics that carry no structured suggestion; a
token-locating fallback for uncolumned suggestions (offer nothing instead); `codeAction/resolve`
(the edit is computed up front); companion migration to `vscode-languageclient`.
