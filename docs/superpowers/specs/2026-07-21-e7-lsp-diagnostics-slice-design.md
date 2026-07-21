# E7: `taliesin lsp` — diagnostics-first vertical slice

**Date:** 2026-07-21
**Status:** design approved, ready to plan
**Backlog item:** E7 (`taliesin lsp` server), the strategic capstone of the editor
DevX / E-series initiative (E1–E6 shipped).

## Why

Quarto 2's headline pitch is "a new Markdown parser for real-time errors,
autocompletion, project-wide YAML validation." Taliesin already has the parser
(comrak + block model) and a deep validator suite (`check.rs` +
`taliesin_core::diagnostics`); the gap is that the VS Code companion surfaces it
**on-save-ish and per-request** (spawning `taliesin check --stdin` on a debounce),
and *only* in VS Code. An LSP-over-stdio subcommand holds the parsed doc warm, takes
`didChange` with the full unsaved buffer, and exposes diagnostics behind a protocol
that works in **any** LSP editor (Neovim, Helix, Zed, VS Code). This aligns with the
**single-editing-surface** invariant: the editor is the only authoring surface, so
authoring quality lives there.

E7 is large because a full LSP must answer capabilities whose logic is split today:
diagnostics / symbols / vocab are in Rust, but outline (`outline.ts`), hover
token-classification (`classifyHover`), and definition resolution (`definitionSite`,
`bibEntryOffset`) live purely in TypeScript in the companion. A truly editor-agnostic
LSP needs those in Rust. **This spec covers only the first cut**: the stdio/JSON-RPC
scaffold plus one capability — live buffer diagnostics — which needs near-zero
porting because the buffer-linting seam already exists in Rust. Hover, definition,
completion, outline, and rename are deliberately deferred to additive follow-up
capabilities on the harness this cut builds.

## Scope decisions (locked)

- **v1 = diagnostics-first vertical slice.** Full lifecycle + `publishDiagnostics`
  only. (Chosen over "diagnostics + completion + hover" and over "full-parity +
  companion rewire".)
- **The VS Code companion is untouched.** It keeps its current spawn-per-request
  providers. Migrating it to a `vscode-languageclient` is a separate later item, so
  the first change stays bounded and the companion stays stable.
- **Dependency:** add `lsp-server` + `lsp-types` (rust-analyzer's own crates), both
  pure-Rust, no C, vendored offline via Cargo — consistent with the offline
  invariant and the workspace's dependency ethos. Rejected: `tower-lsp` (drags sync,
  parse-only work into async/tokio for no gain) and hand-rolling JSON-RPC (reinvents
  framing + the initialize/shutdown/exit lifecycle; error-prone).

## Architecture

New synchronous, long-lived server over stdio, separate from the tokio `serve` path
(LSP work is parse-only and kernel-free, so it stays sync — matching `check.rs`).

- **New module** `crates/server/src/lsp.rs`, entry `pub(crate) fn cmd_lsp(args:
  &[String]) -> ExitCode`.
- **Dispatch** in `main.rs`: `Some("lsp") => lsp::cmd_lsp(&args)`. Add `"lsp"` to the
  `COMMANDS`/dispatch list, `subcommand_help`, and the `complete`/`completions`
  surfaces so it is discoverable and shell-completable (the manifest gate that
  statically checks spawned subcommands stays satisfied — the companion does not
  spawn `lsp` in this cut, but the command must exist in `COMMANDS`).
- The loop uses `lsp_server::Connection::stdio()` for the real server and
  `Connection::memory()` for tests. `lsp-server` owns JSON-RPC framing
  (`Content-Length` headers), the `initialize` request/response handshake, and the
  `shutdown` → `exit` teardown.

### Protocol flow (the one capability)

`initialize` advertises **only**:

```
textDocumentSync = TextDocumentSyncKind::FULL, with openClose = true
```

FULL sync (the whole buffer arrives on every change) maps 1:1 onto the `--stdin`
whole-buffer model, so there is no incremental-sync bookkeeping. No provider
capabilities (hover/completion/definition/symbols) are advertised, so a conforming
client will not request them yet.

Server state: an in-memory `HashMap<Url, String>` of open buffer texts.

| Notification                     | Action                                                             |
|----------------------------------|--------------------------------------------------------------------|
| `textDocument/didOpen`           | store text → lint → `publishDiagnostics`                           |
| `textDocument/didChange` (FULL)  | replace text → lint → `publishDiagnostics`                         |
| `textDocument/didClose`          | drop text → `publishDiagnostics` with `[]` (clears squiggles)      |

- Only `languageId == "taliesin"` documents are tracked; others are ignored.
- `file://` URI → filesystem path via `Url::to_file_path`; the path supplies the
  base dir (relative includes/assets/links) and the reported location. An untitled /
  non-`file:` buffer falls back to the current dir as base (relative includes may not
  resolve — an accepted edge; the diagnostic still reports on the buffer).
- **Per-document linting only** (matches the companion's on-type behavior). No
  site-wide / cross-page pass; no kernel; no code execution.
- **Linting is synchronous on each notification** for this cut. `check` is
  parse-only and fast, so no debounce/coalescing is built yet; a burst of
  `didChange`s runs a burst of (cheap) lints. Debouncing is a noted, non-blocking
  follow-up if a large-doc burst ever measures slow.

## Rust seam reuse — no logic reimplementation

The buffer-linting seam already exists: `collect_file_diagnostics_from_src(path,
src)` (`check.rs`) renders in memory and returns `Vec<Diagnostic>` (the render
warning channel + xref + static validators + YAML front-matter parse error). It never
reads the file from disk — it uses `path` only for the base dir + reported location —
so it lints an unsaved buffer directly. This is exactly what `didChange` needs.

Changes in `check.rs`:

1. Expose a thin `pub(crate) fn buffer_diagnostics(path: &Path, src: &str) ->
   Result<Vec<Diagnostic>, String>` wrapping `collect_file_diagnostics_from_src`
   (which is currently module-private). Keeps one source of truth shared with
   `check --stdin`.
2. Add `pub(crate) fn Diagnostic::to_lsp(&self) -> lsp_types::Diagnostic` beside the
   struct (its fields are module-private, so the mapping lives in `check.rs`).

### `Diagnostic` → `lsp_types::Diagnostic` mapping

- **range:** `line` is 1-based → LSP 0-based. When `col`/`end_col` are present
  (1-based char span), convert to a 0-based `[start, end)` range on that line. When
  absent, span the whole line (start col 0 to line length) — mirrors the companion's
  `check.ts` range logic so squiggles match what VS Code shows today.
- **severity:** `"error"|"warning"|"info"|"hint"` → the matching
  `DiagnosticSeverity`.
- **code:** the stable code string → `NumberOrString::String`.
- **codeDescription:** `docs_url` → `CodeDescription { href }`.
- **source:** `"taliesin"`.
- The structured `suggestion` (quick-fix) is **out of this slice** — a quick-fix is a
  `textDocument/codeAction`, a later capability. `code`/`href` still ride along so a
  fix can attach later without a wire change.

## Testing & pin

An LSP server renders no document, so the corpus-pin discipline adapts: the pin is a
Rust integration test that drives the protocol, linting real `corpus/diagnostics/`
fixtures.

- **Primary pin** — an inline `#[cfg(test)]` module in `lsp.rs` (matches the
  codebase's inline-test pattern, e.g. `check.rs`; no separate `tests/` binary
  needed). Fixtures are read from `corpus/diagnostics/` via
  `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics")` (the
  established convention, e.g. `cli.rs`, `check_superset.rs`). Build an in-process
  server on
  `lsp_server::Connection::memory()`, then drive `initialize` → `initialized` →
  `didOpen` with the text of `corpus/diagnostics/typos.tmd` (a known front-matter
  typo). Assert a
  `textDocument/publishDiagnostics` notification arrives for that URI with the located
  diagnostic (expected line, `severity`, and `code`). Then send `didChange` with a
  clean buffer and assert the next `publishDiagnostics` for that URI is empty
  (squiggles clear). Runs headlessly under `cargo test`; no editor required.
- **Second case:** `corpus/diagnostics/refs.tmd` (or `links.tmd`) for a
  multi-diagnostic document, asserting count + that ranges are within the buffer.
- **Lifecycle case:** `shutdown` → `exit` returns cleanly (exit code 0).
- **Manual smoke (optional, best-effort, not gated):** point Neovim or Helix at
  `taliesin lsp` for one screenshot of a live squiggle. Skipped if the environment
  lacks an LSP editor; the Rust test is the authoritative pin.

## Docs & backlog

- A short internals-book note on the `taliesin lsp` subcommand (what it is, the one
  capability, how to wire an editor).
- Narrow the backlog E7 item to "cut 1 (diagnostics) shipped; hover / definition /
  completion / outline / rename capabilities remain, each additive on the LSP
  harness."

## Non-goals (this cut)

Hover, go-to-definition, completion, document symbols / outline, rename,
code-actions / quick-fix, semantic tokens, formatting, workspace/project-wide
diagnostics, and the VS Code companion migration to `vscode-languageclient`. Each is a
clean additive follow-up.

## Invariant check

- **Single editing surface:** the LSP is read + diagnose only; it never writes to the
  buffer or source. (Formatting/rename, which *would* edit, are explicit non-goals.)
- **Offline:** `lsp-server` + `lsp-types` are pure-Rust, vendored via Cargo; no
  network, no CDN.
- **HTML-only output:** unaffected — the LSP renders in memory to *diagnose*, never to
  emit a new artifact.
- **Block-model invariants:** untouched — reuses the existing render + validator path.
- **Do-NOT-touch (warm-page eviction / `exec_pool.rs`):** untouched — the LSP path is
  kernel-free and separate from the exec pool.
