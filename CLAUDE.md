# qmd-fast

A single-purpose Rust dev server that renders `.qmd` files to **HTML only** (blog
posts, reveal.js slides, books) for one author's workflow. Not a general Quarto
replacement. Three load-bearing goals: click-to-source, block-level incremental
updates, and no per-edit startup cost (warm server + Jupyter kernel).

**The corpus is the spec.** "Done" means the real documents under `corpus/` render
correctly, not that some feature checklist is complete. Scope is those ~30 docs,
not Quarto's feature set.

## Where things are

```
crates/core      qmd-fast-core lib: parser (comrak + sourcepos) → block model → render
  src/render.rs    document + block model, HTML page emission
  src/diff.rs      block-level diff (BlockOp) for incremental updates
  src/includes.rs  {{< include >}} resolution + per-file source map
  src/math.rs      KaTeX server-side render (bundled CSS/fonts, offline)
  src/cite.rs      citations ([@key]) + cross-references (@fig-, @sec-)
crates/server    qmd-fast-server, bin `qmd-fast`: CLI + websocket dev server
  src/main.rs      render / blocks / serve subcommands
  src/serve.rs     axum websocket + notify file watcher
extension/       VS Code client (primary). Placeholder until Phase 3.
web-client/      browser preview client (vanilla JS). Placeholder until Phase 2.
corpus/          the real .qmd docs (the spec); corpus/expected/ holds baselines
```

## Read before working

- **PLAN.md** for any structural or cross-cutting work: architecture, the websocket
  protocol, and the phased build plan. Read this before adding a feature.
- **PROBLEM.md** for motivation and the hard constraints (why HTML-only, why
  block-granular, what's explicitly out of scope).
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
