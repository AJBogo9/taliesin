# qmd-fast

A single-purpose, performance-oriented tool for authoring HTML from `.qmd`
files: blog posts, reveal.js slide decks, and books. A focused replacement for
Quarto for one author's workflow, built around three goals Quarto's architecture
can't deliver:

1. **Click-to-source** — double-click a rendered element, jump to its `.qmd` source.
2. **Block-level incremental updates** — saving a change swaps only the affected
   block(s) in place, preserving scroll position and the runtime state of live
   components (Three.js, OJS).
3. **No per-edit startup cost** — a long-running Rust server with a warm Jupyter kernel.

Output is **HTML only**. See [PROBLEM.md](PROBLEM.md) for the motivation and
[PLAN.md](PLAN.md) for the architecture and phased build plan.

## Architecture (at a glance)

An editor-agnostic Rust dev server owns all logic behind a versioned websocket
protocol. Thin clients consume it: a VS Code extension (primary) and a plain
browser preview (secondary).

```
crates/core     parser (comrak + sourcepos) + block model + render
crates/server   dev server, websocket, file watcher, kernel pool
extension/      VS Code extension (TypeScript)
web-client/     browser preview client (vanilla JS)
```

## Status

Phase 1 — parse → HTML with source positions. The core parses `.qmd` with
comrak (sourcepos), splits the document into top-level blocks with content-hash
ids, and emits HTML with `data-block-id` + `data-sourcepos` on every block.

```sh
cargo run -p qmd-fast-server -- render corpus/posts/born-machines.qmd > out.html
cargo run -p qmd-fast-server -- blocks corpus/posts/born-machines.qmd
```
