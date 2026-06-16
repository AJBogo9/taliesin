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
  src/render.rs    document + block model; HTML / reveal.js / book page emission
  src/diff.rs      block-level diff (BlockOp) for incremental updates
  src/includes.rs  {{< include >}} resolution + per-file source map
  src/math.rs      KaTeX server-side render (bundled CSS/fonts, offline)
  src/cite.rs      citations ([@key]) + cross-references (@fig-, @sec-)
crates/server    qmd-fast-server, bin `qmd-fast`: CLI + websocket dev server
  src/main.rs      render / blocks / serve subcommands
  src/serve.rs     axum websocket + notify file watcher
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
cargo run -p qmd-fast-server -- render <file.qmd> > out.html   # one-shot full page
cargo run -p qmd-fast-server -- blocks <file.qmd>              # block ids + sourcepos (debug)
cargo run -p qmd-fast-server -- serve  <file.qmd> [port]       # live preview (default 4321)
cargo test -p qmd-fast-core                                    # corpus invariants + unit tests
```

## Conventions

- Rust edition 2024, workspace resolver 3. Shared deps go in the root
  `[workspace.dependencies]` so versions stay centralized.
- Every emitted block carries `data-block-id` (content-hash) + `data-sourcepos`;
  included blocks also carry `data-source-file`. Source mapping, incremental
  re-render, and live-state preservation all key off this one block model, so
  preserve those invariants (`crates/core/tests/corpus.rs` enforces them).
