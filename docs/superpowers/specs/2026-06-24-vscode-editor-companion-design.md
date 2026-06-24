# Design: `vscode-editor-companion` Phase 1 (Wave 4)

Status: DESIGN ONLY (author chose "design + plan, don't build yet", 2026-06-24). Branch:
none yet. Roadmap: `BEYOND-QUARTO.md` Pillar II. Phase 1 = host + bidirectional cursor
loop. Phase 2 (editor commands) is capped + deferred.

## Why this closes the loop

The preview's source-sync protocol is **half-built**: the consumer side exists in
`web-client/client.js`, with no producer.
- **Forward (preview → editor):** an Alt-click posts `qmd-goto {source_file, sourcepos}`
  to `window.parent` when `inWebview` (client.js:646-680), else opens a `vscode://file…`
  deep link (already works in a plain browser).
- **Reverse (editor → preview):** the client listens for `qmd-cursor {file, line}` and runs
  `highlightAtLine` (client.js:790-793) — verified working across block types + deck-slide
  jump by `reverse-sync-coverage-audit`.

Phase 1 builds the missing producer: a VS Code extension that hosts the preview and wires
both directions.

## Architecture (3 processes, 2 message hops)

```
VS Code Extension Host (Node/TS)
  │  spawns: `qmd-fast preview <file> <port>`  ── child process (the Rust server)
  │  creates: WebviewPanel
  ▼
Webview document (relay HTML the extension supplies)
  │   <iframe src="{asExternalUri(http://127.0.0.1:port)}">  ── the live preview
  ▼
Preview client (client.js, inWebview === true)
```

- **Hop A — preview iframe ↔ webview doc:** the iframe posts `qmd-goto` to `window.parent`
  (the webview doc); the webview doc posts `qmd-cursor` into `iframe.contentWindow`.
- **Hop B — webview doc ↔ extension host:** the relay script bridges Hop A to VS Code's
  webview messaging (`acquireVsCodeApi().postMessage` ⇄ `panel.webview.postMessage` /
  `onDidReceiveMessage`).

The webview doc is a ~30-line relay: forward any `qmd-goto` from the iframe up to the host;
forward any `qmd-cursor` from the host down into the iframe.

## Extension behavior (Phase 1)

- **Command `qmd-fast: Open Preview`** (and an editor-title icon for `.qmd`): pick a free
  port, spawn `qmd-fast preview <activeFile> <port>` (localhost, NO `--host` → no LAN token
  needed in Phase 1), wait for HTTP 200, open a `WebviewPanel` beside the editor whose
  webview loads the relay doc with the `asExternalUri`-mapped iframe.
- **Forward (`qmd-goto` in):** resolve the target file (`source_file` is null = the
  previewed doc, else relative to its base dir), `openTextDocument` + `showTextDocument`,
  parse `sourcepos` (`L:C`), move the selection/cursor and `revealRange`.
- **Reverse (cursor out):** on `onDidChangeTextEditorSelection` for the previewed `.qmd`
  (or an included file), debounce (~80 ms) and `panel.webview.postMessage({type:"qmd-cursor",
  file, line})`. `file` = the source-file key the preview uses (null for the main doc; the
  relative path for an included file).
- **Lifecycle:** kill the spawned server on panel dispose / extension deactivate; one
  preview per doc; re-focus an existing panel instead of spawning twice.

## Decisions to confirm (flagged)

1. **Location:** `editor/vscode/` (a new top-level subproject) vs `tools/vscode-companion/`
   (matches `tools/record-demo`). *Recommend `editor/vscode/`* — it's a distinct concern.
2. **Binary discovery:** how the extension finds `qmd-fast` — a `qmdFast.path` setting
   defaulting to `qmd-fast` on PATH (the launcher), with a fallback to the workspace's
   `target/release/qmd-fast`. *Recommend the setting + PATH default.*
3. **Packaging:** Phase 1 ships as an F5-dev extension (run from source in the Extension
   Dev Host); `vsce package` to a `.vsix` is a later polish. *Recommend dev-only for now.*
4. **Source-file mapping scope:** Phase 1 wires the **main previewed doc** fully; included
   files (`source_file` relative) are best-effort (resolve against base dir) and refined
   later. *Recommend main-doc-first.*

## #1d (LAN token) coordination

The companion launches on `127.0.0.1` (no `--host`), so the LAN token #1d adds is not
needed for Phase 1. The seam: if/when the URL carries a `?token=…`, the webview iframe src
must include it. Documented so #1d and the companion stay compatible; no work in Phase 1.

## Verification (author-side — NOT headless)

A VS Code extension runs in the Extension Development Host; the chrome-devtools loop can't
drive it. The plan ends with explicit steps for the author: F5 → open a corpus `.qmd` (e.g.
`corpus/posts/em-algorithm/index.qmd`) → run *Open Preview* → (a) move the cursor, see the
matching block highlight + scroll; (b) Alt-click a block, see the editor cursor jump; (c)
open the deck doc and confirm a cursor move jumps slides. The extension's own logic
(port-pick, path-resolve, sourcepos-parse, debounce) gets unit tests where it's pure Node;
the VS Code API surface is thin and exercised manually.

## Invariants (load-bearing)

The preview stays **read-only**: cursor sync only highlights/scrolls, `qmd-goto` only
navigates. No preview write-back, no buffer mutation (Phase 2's editor commands, when built,
are strictly `.qmd`-buffer text transforms in the editor — never preview gestures). The
extension never edits the rendered DOM or the source from the preview. Single-editing-surface
intact.

## Out of scope (YAGNI / Phase 2)

Editor commands (insert block / reorder slide) — deferred, capped, buffer-transform-only.
`.vsix` packaging + marketplace. Multi-root workspaces. Hot-swapping the previewed file when
the active editor changes (Phase 1 = one panel pinned to the doc it was opened for).
