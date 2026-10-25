# Taliesin

Taliesin turns `.tmd` files into HTML: blog posts, papers, books and multi-page sites. A
`.tmd` file is Markdown with executable code cells, in the syntax Pandoc and Quarto use
(fenced divs, attributes, citations, cross-references, `{python}` cells), plus `{js}`
cells that run in the browser. It is a personal tool, built for one author's writing
workflow. Where it differs from Quarto, MyST and Jupyter Book:

1. **Click-to-source.** Ctrl-click (Cmd-click on Mac) an element in the preview to jump to
   its `.tmd` source.
2. **Block-level live updates.** Saving swaps only the changed block(s) in the open
   `taliesin preview`, preserving scroll position and the runtime state of live components
   (Three.js, `{js}` cells).
3. **A warm server and kernel.** The server and its Jupyter kernel stay running between
   edits, so a save re-runs code from the first changed cell on, not the whole document.
4. **HTML only.** There is no PDF, LaTeX or Word output.

In the recording below, text typed into a `.tmd` file in VS Code shows up in the preview on
save, and a Ctrl-click on a paragraph in the preview moves the editor cursor to its source
line.

https://github.com/user-attachments/assets/5450118f-9de3-4214-8653-f2af0807d8ea

## Build and run

```sh
cargo build --release                        # binary at target/release/taliesin
taliesin preview post.tmd                    # live preview of one document
taliesin preview my-site                     # a _site.yml project
taliesin build my-site                       # static my-site/_site/
taliesin build my-site --check-only          # lint, write nothing
```

`{python}` cells need a Python with `ipykernel`. A project `.venv` is found on its own, and
`taliesin doctor` names the interpreter it picked. Previewing a `.tmd` runs its code;
`--no-exec` renders the cells as source. `corpus/tech-blog` is a complete site to try it on.

```
crates/core     parser (comrak + sourcepos), block model, render
crates/server   CLI, dev server, websocket, kernel, LSP
web-client/     browser scripts: the preview client, search, scrollspy
editor/vscode/  the VS Code companion (preview webview + source sync)
corpus/         the real documents the test suite renders
```

## Licence

AGPL-3.0 ([`LICENSE`](LICENSE)), © 2026 Andreas Bogossian. A built page contains copies of
Taliesin's own CSS and JavaScript, and the
[Taliesin Output Exception](LICENSE-OUTPUT-EXCEPTION.md) lets you publish that output under
any terms, with nothing to attribute. The VS Code companion under
[`editor/vscode`](editor/vscode) is MIT. Vendored third-party assets keep their own
licences; see [`THIRD_PARTY.md`](THIRD_PARTY.md).
