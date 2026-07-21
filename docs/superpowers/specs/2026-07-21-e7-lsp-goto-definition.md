# E7 capability 2: `taliesin lsp` go-to-definition (design + plan)

**Date:** 2026-07-21 · **Status:** approved, lean process (combined design+plan) ·
**Follows:** the E7 diagnostics slice ([design](2026-07-21-e7-lsp-diagnostics-slice-design.md)).

## Goal

Add `textDocument/definition` to `taliesin lsp` so any LSP editor can jump from a `.tmd`
token to its source: `{{< include/embed PATH >}}` → the file; `@fig-x`/`@sec-x`/… → its
`{#fig-x}` or `#| label: fig-x` definition **in this document**; `[@key]` → the BibTeX
entry in the front-matter `.bib`. Read-only, offline, navigates-never-writes. A port of
the companion's `definition-provider.ts` + the shared `classifyHover` (`hover.ts`) into
Rust, so it is editor-agnostic.

## Two foundations this builds (reused by later capabilities)

1. **Document store.** Diagnostics were notification-driven (text in hand), so the server
   kept a `HashSet<Url>`. A *request* arrives between edits, so the server must hold buffer
   text: promote to `HashMap<Url, String>`. Hover/completion will reuse it.
2. **`classify_target`** (Rust port of `classifyHover`): the token under the cursor
   (cite > xref > include > front-matter key, each with a span). Hover reuses it next.

## Architecture

- **New module `crates/server/src/lsp_nav.rs`** — pure, LSP-free string ports, unit-tested
  without the server loop (mirrors `check.rs`/`hover.ts` style). No `regex` dependency:
  hand-rolled char scanning (the tokens are simple), matching the minimal-dep ethos.
- **`crates/server/src/lsp.rs`** — document store + `definitionProvider: true` + a
  `textDocument/definition` request handler that calls `lsp_nav`.
- Position offsets are char-based (UTF-16 for ASCII, which covers all realistic xref
  ids / cite keys / paths / front-matter keys); documented, consistent with the
  diagnostics slice's `to_lsp`.

### `lsp_nav.rs` surface

```rust
pub(crate) enum Target {
    None,
    Xref { id: String, start: usize, end: usize },
    Cite { key: String, start: usize, end: usize },
    Include { path: String, start: usize, end: usize },
    FrontmatterKey { key: String, parent: Option<String>, start: usize, end: usize },
}
// Classify the token at 0-based (line, character). Cite `[@k]` wins over xref `@k`;
// front-matter key only inside the `---` body, on the key token.
pub(crate) fn classify_target(text: &str, line: usize, character: usize) -> Target;
// 0-based (line, col) where xref `id` is DEFINED here: first `#id`/`label: id`, never `@id`.
pub(crate) fn definition_site(text: &str, id: &str) -> Option<(u32, u32)>;
// 0-based (line, col) of `@type{key,` in a .bib, or None.
pub(crate) fn bib_entry_site(bib: &str, key: &str) -> Option<(u32, u32)>;
// `bibliography:` front-matter paths (scalar or list).
pub(crate) fn frontmatter_bib_paths(text: &str) -> Vec<String>;
```

## Plan (TDD, three commits)

### Commit 1 — `lsp_nav.rs` pure ports

- Write failing unit tests: `classify_target` on each kind + the tricky boundaries
  (`[@k]` classifies as cite not xref; an email `a@b` is not an xref; cursor at the last
  char still hovers; a key line inside `---` only). `definition_site` finds `{#fig-1}` and
  `#| label: fig-1`, rejects `@fig-1`, returns None for an absent id. `bib_entry_site`
  finds `@article{smith2020,`. `frontmatter_bib_paths` reads scalar + list forms.
- Implement each function with hand-rolled scanning. Run RED → GREEN.
- `mod lsp_nav;` in `main.rs`.

### Commit 2 — document store refactor (behavior-preserving)

- Change `main_loop`'s `tracked: HashSet<Url>` → `docs: HashMap<Url, String>`;
  `handle_notification` upserts text on didOpen (languageId `taliesin`) / didChange, removes
  on didClose. Diagnostics publish from the same text (unchanged findings).
- The existing three lsp integration tests must still pass unchanged (the refactor is
  invisible to them). No new test needed here beyond keeping them green.

### Commit 3 — `textDocument/definition`

- Advertise `definition_provider: Some(OneOf::Left(true))` in `server_capabilities`.
- In `main_loop`, after the shutdown check, route non-shutdown requests to
  `handle_request(connection, &docs, req)`: if method == `GotoDefinition::METHOD`, parse
  `GotoDefinitionParams`, look up the buffer text, `classify_target`, resolve:
  - `Include` → resolve path against the URI's dir; if the file exists →
    `Location(file, 0:0)`, else `null`.
  - `Xref` → `definition_site` → `Location(same uri, [line:col, line:col+id.len))`.
  - `Cite` → first `frontmatter_bib_paths` entry that exists + `bib_entry_site` finds →
    `Location(bib uri, line:col)`; else `null`.
  - else `null`.
  Respond with `GotoDefinitionResponse::Scalar(loc)` or `null`. Unhandled non-shutdown
  requests get a `MethodNotFound` error response (so the client never hangs).
- Failing integration pin first: `didOpen` a doc with `@fig-1` and `# Title {#fig-1}`, send
  `textDocument/definition` at the `@fig-1` char → assert the `Location` points at the
  `{#fig-1}` line/col. A `[@key]` → `.bib` case using a temp fixture.

## Verify

`cargo test -p taliesin-server` (serial if the exec/kernel flake trips), `cargo fmt
--check`, `cargo clippy -p taliesin-server --all-targets`, and a real-process smoke: spawn
`taliesin lsp`, initialize + didOpen + `textDocument/definition`, confirm a `Location` on
stdout. Then narrow the backlog E7 item (definition shipped).

## Non-goals

Hover, completion, outline, rename (later capabilities); cross-file xref resolution
(same-doc only, mirrors the companion); companion migration to `vscode-languageclient`.
