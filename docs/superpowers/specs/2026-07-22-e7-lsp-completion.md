# E7 capability 5: `taliesin lsp` completion (design + plan)

**Date:** 2026-07-22 · **Status:** approved, lean process (combined design+plan) ·
**Follows:** the E7 diagnostics / go-to-definition / document-outline / hover capabilities on
the same `taliesin lsp` server.

## Goal

Add `textDocument/completion` to `taliesin lsp` so any LSP editor autocompletes `.tmd` as the
VS Code companion does, across the seven cursor contexts its `completions.ts` handles:

1. **front-matter key** (top level, or a key under a nested parent like `execute:`) → the vocab keys.
2. **front-matter value** (`format:`/`theme:` closed sets) → the vocab values, filtered by what's typed.
3. **cell option** (`#|`/`//|`/`%%|` inside a code cell) → the vocab cell-option keys.
4. **fenced-div class** (`::: {.`) → `callout-*` kinds + theorem kinds + structural div classes.
5. **cross-reference** (`@`) → the `prefix-` stubs + the document's live + rendered xref targets.
6. **citation** (`[@`) → the citation keys harvested from the front-matter `.bib`(s).
7. **shortcode path** (`{{< include `/`{{< embed `) → the `.tmd` files + descendable dirs beside the doc.

Read-only, offline. A port of the companion's pure `complete.ts` (`detectContext` + the
harvest helpers + `shortcodePathCandidates`) into the editor-agnostic server, drawing on the
same Rust-authoritative vocab the companion fetches via `taliesin vocab`.

## Data sources (all Rust-authoritative, all in-process)

- **static vocab** — `taliesin_core::vocab::vocab()` (a `serde_json::Value`): `frontmatter.keys`
  / `frontmatter.nested[parent]`, `frontmatterValues[key]`, `cellOptions`, `calloutKinds`,
  `theoremKinds`, `divClasses`, `xrefPrefixes`. Same source the hover capability already reads.
- **xref targets** — render the **live buffer** (`render_buffer`, the shared parse-only,
  panic-guarded render hover uses) → `RenderedDoc::xref_numbers`, filtered to
  `cite::is_xref_anchor`, giving `{id, label number}` (label from vocab). Merged with a buffer
  `{#id}` harvest so a just-typed anchor is completable even before it numbers. This is the
  in-process, staleness-free equivalent of the companion's `mergeXrefTargets(harvestAnchorIds,
  symbols, labels)`.
- **cite keys** — `lsp_nav::frontmatter_bib_paths` (already built) + a new
  `harvest_bib_keys(bib)` over each `.bib`'s text.
- **shortcode paths** — the doc's directory listing (from the URI) + `shortcode_path_candidates`.

## Architecture

- **New module `crates/server/src/lsp_complete.rs`** — pure, LSP-free, unit-tested (mirrors
  `lsp_nav.rs`): `CompletionContext` enum; `detect_context(line_prefix, doc_prefix)`; the
  `in_frontmatter` / `in_code_cell` / `nested_parent` context helpers; `harvest_anchor_ids`,
  `harvest_bib_keys`; `shortcode_path_candidates` (+ `DirEntry` / `PathCandidate`). Hand-rolled
  scanning, **no `regex` dependency** (each `complete.ts` regex is anchored at the cursor / line
  start, so a small backward/forward scan replaces it), matching the minimal-dep ethos.
- **`crates/server/src/lsp.rs`** — advertise `completion_provider` (trigger chars
  `@ . | - / :`); route `Completion::METHOD` to `resolve_completion(docs, &params) ->
  Option<CompletionResponse>`; build `CompletionItem`s per context. Factor a `render_buffer`
  helper shared with hover's `xref_number`.
- Line/char offsets stay char-based (UTF-16 for ASCII), consistent with the prior capabilities.
- Rendered on demand per xref completion (parse-only; consistent with hover). No caching this cut.

## Plan (TDD)

### Commit 1 — spec doc (this file)

### Commit 2 — `lsp_complete.rs` pure port

- Failing unit tests for `detect_context`: each of the 7 kinds at a representative cursor, plus
  the tricky boundaries the ordering guards — `[@k` is `cite` not `xref`; `{{< include a@b` is
  `shortcode-path` not `xref`; a `#|` line only completes inside an open code fence; a
  front-matter key vs value (`title:` value position vs bare key); a nested key resolves its
  `parent`; outside the `---` block a bare word is `none`. Tests for `harvest_anchor_ids`
  (`{#fig-1}` yes, `{.theorem #x}` no), `harvest_bib_keys` (`@article{smith2020,` → `smith2020`),
  and `shortcode_path_candidates` (dirs suffixed `/`, `.tmd` files, IGNORE_DIRS + dotfiles hidden).
- Implement each with hand-rolled scanning. Run RED → GREEN. `mod lsp_complete;` in `main.rs`.

### Commit 3 — `textDocument/completion`

- Advertise `completion_provider`; `render_buffer` helper (refactor hover to share it).
- `resolve_completion`: slice `line_prefix` (line start → cursor) and `doc_prefix` (doc start →
  cursor) from the buffer; `detect_context`; load vocab; per context emit items:
  - key → keys/nested; value → `frontmatterValues[key]` filtered by typed; cell-option →
    cellOptions; div-class → `callout-{k}` + theorems + divClasses; xref → `{prefix}-` stubs +
    merged targets filtered by typed; cite → harvested `.bib` keys; shortcode-path →
    `shortcode_path_candidates` with a `TextEdit` replacing the typed path segment.
  - `none` → `None`.
- Route `Completion::METHOD` in `handle_request`.
- Failing integration pins (in-process `Connection::memory()`): completion inside `execute:`
  offers `echo`; after `@` in a doc with `{#fig-scree}` offers `fig-scree` labeled `Figure 1`;
  inside `[@` with a temp `.bib` offers the citation key.

## Verify

`cargo test -p taliesin-server` (serial if the exec/kernel flake trips), `cargo fmt --check`,
`cargo clippy -p taliesin-server --all-targets`, and a real-process smoke: spawn `taliesin lsp`,
initialize + didOpen + `textDocument/completion` at a front-matter key, an `@` xref, and a `[@`
cite; confirm the item lists on stdout. Then narrow the backlog E7 item (completion shipped).

## Non-goals

Rename, quick-fix code-actions (later capabilities on this server); completion-item `resolve`
(all detail is computed up front); snippet insert text; per-document render caching; companion
migration to `vscode-languageclient`.
