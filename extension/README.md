# qmd-fast VS Code extension

The primary client. Hosts the live preview in a webview and provides
click-to-source: double-click a block in the preview to jump the editor to its
`.qmd` source (including into `{{< include >}}`d files).

## How it works

The extension is a thin client over the Rust dev server — all rendering lives
there:

1. On **qmd-fast: Open Preview** (command palette, the editor-title button, or
   `Ctrl/Cmd+Shift+V`), it spawns `qmd-fast serve <file> <free-port>` for the
   active `.qmd`.
2. A webview hosts an `<iframe>` pointed at `http://127.0.0.1:<port>/`, which
   loads the same preview client used in the browser. Saving the file makes the
   server push only the changed blocks over the websocket — unchanged blocks
   (and their live Three.js/OJS state) are never touched.
3. Double-clicking a block posts `{ source_file, sourcepos }` up to the
   extension host, which opens that file and `revealRange`s the block
   (click-to-source). Single-click highlights the block.

## Build & run (development)

```sh
cd extension
npm install
npm run compile        # tsc -> out/extension.js   (npm run watch to rebuild on change)
```

Then open the repo in VS Code and press **F5** ("Run Extension") to launch an
Extension Development Host. Open a `.qmd` (e.g. `corpus/posts/born-machines.qmd`)
and run **qmd-fast: Open Preview**.

The extension finds the `qmd-fast` binary automatically (workspace
`target/release` then `target/debug`, then `PATH`); override with the
`qmd-fast.serverPath` setting. Build it first with `cargo build` (or
`cargo build --release`).

## Status / limitations

- Save-triggered block-swap and double-click-to-source are implemented.
- Reverse scroll-sync (editor cursor → highlight block in preview) is deferred.
- Requires a local server reachable at `127.0.0.1`; remote/Codespaces port
  forwarding is untested.
