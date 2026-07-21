# E7 capability 7 (final): `taliesin lsp` rename (design + plan)

**Date:** 2026-07-22 · **Status:** approved, lean process (combined design+plan) ·
**Follows:** the E7 diagnostics / go-to-definition / outline / hover / completion / quick-fix
code-action capabilities on the same `taliesin lsp` server. This closes out E7.

## Goal

Add `textDocument/rename` (+ `prepareRename`) to `taliesin lsp` so renaming a cross-reference
anchor in any LSP editor rewrites its definition **and** every `@`-reference in one atomic
edit. Put the cursor on `{#fig-scree}` (or any `@fig-scree`), invoke rename, type `fig-plot`,
and the `{#…}` attribute (or `#| label:` cell label) plus all `@fig-scree` references become
`fig-plot` together — no reference is left dangling. Read-only w.r.t. the preview; the edit
flows through the editor (the legitimate editing surface), never the preview.

## Scope

**Cross-reference anchors only** — ids where `cite::is_xref_anchor(id)` holds (a known xref
prefix: `fig-`, `tbl-`, `sec-`, `eq-`, `thm-`, …), i.e. exactly the ids `@ref` resolves. These
have a clean, symmetric rename: one definition (`{#id}` attribute or `#| label: id` cell) and N
`@id` references, all in this document. **Out of scope** (a different, broader operation): plain
heading-id renames (`{#intro}` referenced via `[jump](#intro)` Markdown anchor links, not `@`);
cross-file rename; renaming citation keys or front-matter keys. The `prepareRename` gate makes
the boundary visible in the editor: on anything but an xref anchor, the client shows "cannot
rename here".

## Design

Two pure helpers in `lsp_nav.rs` (LSP-free, unit-tested, sharing its existing char scanners),
plus two thin LSP handlers in `lsp.rs`. No new module — rename is navigation-adjacent and reuses
`classify_target` + `definition_site`'s prefix logic.

1. **`is_anchor_site(chars, i)`** (private) — whether the id token starting at char offset `i`
   sits in a rename site: a `@id` *reference* (the `@` a real xref sigil — not preceded by a
   word char, `@`, or `[`, so a `[@key]` citation is excluded), a `#id` *attribute*, or a
   `label: id` *cell label*. This is `definition_site`'s `prefix_ok` test plus the reference
   form, factored out so the rename set and go-to-definition can't disagree on what an anchor is.
2. **`anchor_occurrences(text, id) -> Vec<(u32,u32,u32)>`** (pub(crate)) — every site in the
   buffer where `id` appears as a whole xref-id token in a rename site, each a 0-based
   `(line, start_col, end_col)` covering **exactly the id** (never the `@`/`#` sigil). Includes
   the definition, so renaming keeps refs resolving. This is the set the edit rewrites.
3. **`anchor_at(text, line, character) -> Option<(String, usize, usize)>`** (pub(crate)) — the
   xref anchor under the cursor + its id-only span on `line`, whether the cursor is on a
   reference or a definition. Reuses `classify_target` for the reference form (which also covers
   a cursor on the `@`); scans a maximal xref-id run for the definition form. Gated by
   `is_xref_anchor` — a plain heading id / cite key / prose returns `None`. Underlies
   `prepareRename` (its range) and `rename` (the id to rewrite).

The id-only span is load-bearing: `prepareRename`'s placeholder range and every `rename` edit
must cover the same id text (never the sigil), so the box pre-fills with `fig-scree` and the
edits replace `fig-scree`, keeping `@`/`#`/`label:` intact.

## Architecture

- **`crates/server/src/lsp_nav.rs`** — add `is_anchor_site`, `anchor_occurrences`, `anchor_at`
  (uses `taliesin_core::cite::is_xref_anchor`, already used elsewhere in the server).
- **`crates/server/src/lsp.rs`** — advertise `rename_provider: OneOf::Right(RenameOptions {
  prepare_provider: Some(true), .. })`; route `PrepareRenameRequest::METHOD` →
  `resolve_prepare_rename` and `Rename::METHOD` → `resolve_rename` (one `WorkspaceEdit` whose
  `changes` maps this uri to the `anchor_occurrences` edits). Empty `new_name` → `None`.
- Offsets stay char-based (UTF-16 for ASCII), consistent with the prior capabilities.

## Plan (TDD)

### Commit 1 — spec doc (this file)

### Commit 2 — `lsp_nav.rs` rename helpers

- Failing unit tests: `anchor_at` finds a `@fig-scree` reference and a `{#fig-scree}` definition
  (id-only spans); rejects a `[@cite]` key and a plain `{#intro}` heading id (not xref anchors).
  `anchor_occurrences` returns the definition + both `@fig-scree` refs for a 3-site doc and
  excludes a `[@fig-scree]` citation.
- Implement the three helpers with hand-rolled scanning. RED → GREEN.

### Commit 3 — `textDocument/rename` + `prepareRename`

- Advertise `rename_provider`; route both methods; implement `resolve_prepare_rename` /
  `resolve_rename`.
- Failing integration pin (in-process `Connection::memory()`): `didOpen` a doc with
  `{#fig-scree}` + two `@fig-scree`; `prepareRename` at a reference returns the id range;
  `rename` to `fig-plot` returns a `WorkspaceEdit` with 3 edits, all `new_text: "fig-plot"`,
  over the definition + both references.

### Commit 4 — narrow the backlog E7 item (rename shipped → E7 complete)

## Verify

`cargo test -p taliesin-server` (serial if the exec/kernel flake trips), `cargo fmt --check`,
`cargo clippy -p taliesin-server --all-targets`, and a real-process smoke: spawn `taliesin lsp`,
`didOpen` an anchor doc, `prepareRename` + `rename`, confirm the multi-edit `WorkspaceEdit` on
stdout. Then mark E7 complete in the backlog.

## Non-goals

Plain heading-id / cross-file / cite-key / front-matter-key rename; new-name validation beyond
rejecting empty (the editor is the editing surface, the user owns the input); `codeAction`-style
rename; companion migration to `vscode-languageclient` (a separate later item).
