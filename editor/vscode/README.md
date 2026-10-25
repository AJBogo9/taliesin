# Taliesin Companion

Hosts the Taliesin live preview in a VS Code webview with source sync in both directions:

- **Preview to editor:** Ctrl-click a block in the preview and the editor jumps to its source
  line.
- **Editor to preview:** moving the cursor marks the matching block without scrolling;
  **`Ctrl+Alt+J`** (*Taliesin: Reveal Cursor in Preview*) scrolls it into view.

The preview stays read-only: the extension navigates and highlights, and never writes back to
the source. It also ships the `.tmd` grammar and snippets. Every language feature (completion,
hover, definition, outline, folding, diagnostics, quick fixes) comes from `taliesin lsp`;
`src/embedded.ts` routes requests inside a code cell to that language's own provider.

## Build and install

```sh
cd editor/vscode
npm install
npm run build
npx @vscode/vsce package         # → taliesin-companion-<version>.vsix
code --install-extension taliesin-companion-<version>.vsix
```

The extension runs the `taliesin` binary on `PATH`, or the one the `taliesin.path` setting
names.

## Develop

Run `npm run build`, then
`code --new-window --extensionDevelopmentPath=$PWD <dir> <file.tmd>` to load `out/extension.js`
in an Extension Development Host without touching the installed extension. `npm test` runs the
unit tests and the offline grammar test (`node scripts/ensure-vscode.cjs` fetches the base
grammars once).
