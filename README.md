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

Phases 0–4 done. `qmd-fast serve` runs a long-lived dev server: it watches the
`.qmd` (and its includes/bibliography), and on each save re-renders, **executes
changed code cells against a warm Jupyter kernel** (re-running only the earliest
changed cell and everything downstream), diffs against the previous block list,
and pushes only the changed blocks over a websocket. Unchanged blocks are never
touched, so scroll position and the runtime state of live blocks (Three.js, OJS)
survive edits. The VS Code extension hosts the same preview with
double-click-to-source.

```sh
cargo run -p qmd-fast-server -- serve corpus/posts/born-machines.qmd  # http://127.0.0.1:4321
```

Code execution needs a Python with `ipykernel`; point the server at it with the
`QMD_FAST_PYTHON` env var (defaults to `python3`). Cells render as source if no
kernel is available. Outputs (stdout/stderr, results, images, HTML, errors)
become their own blocks keyed to the cell, so they swap in place.

The render pipeline underneath: the core parses `.qmd` with comrak (sourcepos),
splits the document into top-level blocks with content-hash ids, and emits HTML
with `data-block-id` + `data-sourcepos` on every block.

Supported so far:

- Prose, tables (with alignment), nested/tight lists, code cells.
- Math server-side via KaTeX — inline `$…$`, display `$$…$$`, and bare
  `\begin{…}` environments. CSS + fonts are bundled and inlined, so pages are
  self-contained and work offline (no CDN).
- `{{< include >}}` resolution with a per-file source map: included blocks carry
  their origin file + that file's line numbers (`data-source-file`), so
  click-to-source jumps into the included file.
- Callouts (`.callout-note`/`tip`/`warning`/`important`/`caution`) and
  `layout-ncol` grids.
- Pragmatic citations (`[@key]`) → numbered links + an auto-generated References
  section parsed from the `.bib`; cross-references (`@fig-`, `@sec-`, …) → labelled
  anchor links.

```sh
cargo run -p qmd-fast-server -- render corpus/posts/born-machines.qmd > out.html
cargo run -p qmd-fast-server -- blocks corpus/posts/born-machines.qmd
```
