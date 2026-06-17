# qmd-fast

A single-purpose Rust dev server that renders `.qmd` files to **HTML only** (blog
posts, reveal.js slides, books) for one author's workflow. Not a general Quarto
replacement. Three load-bearing goals: click-to-source, block-level incremental
updates, and no per-edit startup cost (warm server + Jupyter kernel).

**The corpus is the spec.** "Done" means the real documents under `corpus/` render
correctly, not that some feature checklist is complete. Scope is those ~5 documents
(13 `.qmd` files counting book subsections), not Quarto's feature set.

## Where things are

```
crates/core      qmd-fast-core lib: parser (comrak + sourcepos) → block model → render
  src/render.rs    document + block model; HTML / reveal.js / book / site-chrome emission
  src/diff.rs      block-level diff (BlockOp) for incremental updates
  src/includes.rs  {{< include >}} resolution + per-file source map
  src/math.rs      KaTeX server-side render (bundled CSS/fonts, offline)
  src/highlight.rs server-side syntax highlighting (syntect → `qhl-` scope classes)
  src/cite.rs      citations ([@key]) + cross-references (@fig-, @sec-)
  src/site.rs      multi-page project: _quarto.yml config, page discovery, chrome, link rewrite
crates/server    qmd-fast-server, bin `qmd-fast`: CLI + websocket dev server
  src/main.rs      render / blocks / build / serve subcommands (a dir = a site project)
  src/serve.rs     single-doc axum websocket + notify file watcher
  src/serve_site.rs multi-page site server (per-page state/executor, cross-page nav, hot reload)
  src/exec.rs      runs a doc's code cells, splices outputs back as blocks
  src/kernel.rs    warm Jupyter kernel (ZMQ), reused across edits
  src/log.rs       colorized dev-server console output (to stderr)
web-client/      browser preview client (vanilla JS, the only client): mounts
                 blocks, applies ops; double-click opens source in the editor
docs/            project's own manual + tour deck, authored in .qmd (dogfooding)
corpus/          the real .qmd docs (the spec); cargo test renders them all
```

## Read before working

- **docs/index.qmd** for the architecture, the websocket protocol, and the block
  model (the project's own manual, authored in `.qmd`).
- **corpus/README.md** for what the test documents exercise.

## Commands

```sh
cargo run -p qmd-fast-server -- preview <file.qmd> [port]      # live preview (aliases: dev, serve)
cargo run -p qmd-fast-server -- preview <file.qmd> --host      # + expose on LAN with a phone QR code
cargo run -p qmd-fast-server -- preview <dir>                  # live multi-page SITE preview (nav + per-page hot reload)
cargo run -p qmd-fast-server -- build  <file.qmd> [out.html]   # self-contained HTML file (default <name>.html)
cargo run -p qmd-fast-server -- build  <file.qmd> --out <dir>  # portable folder: <dir>/index.html + copied local assets
cargo run -p qmd-fast-server -- build  <dir> [--out <dir>]     # multi-page SITE -> _site/ (one .html per page + assets)
cargo run -p qmd-fast-server -- render <file.qmd> > out.html   # one-shot full page to stdout
cargo run -p qmd-fast-server -- blocks <file.qmd>              # block ids + sourcepos (debug)
cargo test -p qmd-fast-core                                    # corpus invariants + unit tests
```

A `qmd-fast` launcher on `PATH` (`~/.local/bin/qmd-fast`) rebuilds the release
binary when the tool's sources change, then runs it, so `qmd-fast preview <file>`
works from anywhere.

Executing code cells (`{python}`) needs a Python with `ipykernel`; point the
server at it with `QMD_FAST_PYTHON` (defaults to `python3`). Without a kernel,
cells render as source and the preview shows a "kernel unavailable" diagnostic.

For UI work, `/preview <file.qmd>` builds, serves on port 4388, and verifies it in
the browser via the chrome-devtools MCP (screenshot + console). A `PostToolUse`
hook runs `rustfmt` on every edited `.rs` file, so the tree stays `cargo fmt`-clean
(CI enforces it).

## Conventions

- Rust edition 2024, workspace resolver 3. Shared deps go in the root
  `[workspace.dependencies]` so versions stay centralized.
- Every emitted block carries `data-block-id` (content-hash) + `data-sourcepos`;
  included blocks also carry `data-source-file`. Source mapping, incremental
  re-render, and live-state preservation all key off this one block model, so
  preserve those invariants (`crates/core/tests/corpus.rs` enforces them).
