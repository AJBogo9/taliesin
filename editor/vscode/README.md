# qmd-fast Companion (Phase 1)

Hosts the qmd-fast live preview in a VS Code webview with **bidirectional source sync**.
It is the missing *producer* for the source-sync protocol the preview client
(`web-client/client.js`) already consumes:

- **Forward (preview → editor):** Alt-click a block in the preview → the editor jumps to
  that block's source line (`qmd-goto`).
- **Reverse (editor → preview):** move the cursor in the `.qmd` → the matching block
  highlights and scrolls into view in the preview, and the deck jumps to the right slide
  (`qmd-cursor` → `highlightAtLine`).

The preview stays **read-only**: the extension only navigates and highlights; it never
writes back to the source.

## Develop / run

1. `cd editor/vscode && npm install && npm run build`
2. Open the **`editor/vscode`** folder in VS Code and press **F5** ("Run qmd-fast
   Companion"). A second *Extension Development Host* window opens with the extension
   loaded. (Run `npm run build` again after any source change, then reload the host.)
3. Ensure `qmd-fast` is on `PATH` (the launcher), or set the `qmdFast.path` setting to the
   binary (e.g. `target/release/qmd-fast`).

## Manual verification (the loop the headless tests can't close)

A VS Code extension runs in the Extension Development Host, which can't be driven
headlessly — so this checklist is run by hand. In the Extension Development Host window,
opened on the qmd-fast repo:

1. Open `corpus/posts/em-algorithm/index.qmd`. Run **qmd-fast: Open Preview** (command
   palette, or the editor-title button). The preview opens beside the editor and renders.
2. **Reverse sync:** move the cursor onto a heading / paragraph — the matching block in the
   preview gains the `.qmd-hl` outline and scrolls into view.
3. **Forward sync:** Alt-click a block in the preview — the editor cursor jumps to that
   block's source line.
4. **Deck:** open `corpus/liquid-glass-slides/example.qmd`, Open Preview, move the cursor
   into a later slide's content — the deck jumps to that slide.
5. Close the preview panel — the spawned `qmd-fast preview` process exits (no orphan).

## Known risk

`vscode.env.asExternalUri` localhost-iframing inside a webview CSP can behave differently
across VS Code versions / remote setups. If the iframe is blocked, fall back to a webview
`portMapping`, or open the preview in an external browser tab (`vscode.env.openExternal`) —
forward sync still works there via the `vscode://file…` deep links the client already emits.

## Scope

Phase 1 = host + cursor loop, localhost only (no `--host`, so backlog #1d's LAN token isn't
needed yet). Phase 2 (editor commands like insert-block / reorder-slide, strictly as
`.qmd`-buffer text transforms) is deferred. See
`docs/superpowers/specs/2026-06-24-vscode-editor-companion-design.md`.

## Automated verification (three layers)

1. **Unit (`npm test`)** — `node:test` for `ports.ts` (free-port pick, HTTP wait) and
   `paths.ts` (sourcepos parse, source-file mapping). No VS Code needed.
2. **Relay bridge (`node scripts/relay-harness.cjs`)** — serves the real `relayHtml` with a
   same-origin stub iframe so a browser can drive both message directions against the actual
   code (see the script header). Verified: `qmd-goto` from the iframe reaches the host;
   `qmd-cursor` from the host reaches the iframe.
3. **Extension Host (`npm run test:e2e`)** — `@vscode/test-electron` downloads a throwaway
   VS Code and runs `src/e2e/` inside the real Extension Host: asserts the command is
   registered and that *Open Preview* on a `.qmd` actually opens a webview panel (which
   spawns the server). Needs the locally-built `target/debug/qmd-fast`. The runner clears
   `ELECTRON_RUN_AS_NODE` (set in some sandboxes) and passes `--no-sandbox` so VS Code
   launches headless.

What those three **don't** cover — and what the F5 checklist above is for — is the final
visual round-trip *through the live preview iframe*: cursor-move → the block actually
highlights/scrolls, and Alt-click → the editor cursor lands. Layers 2+3 verify each side of
that bridge independently; F5 confirms them end-to-end in a real editor.
