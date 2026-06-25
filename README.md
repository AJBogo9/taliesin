# qmd-fast

A single-purpose, performance-oriented tool for authoring HTML from `.qmd`
files: blog posts, slide decks, books, and **multi-page websites**. A
focused replacement for Quarto for one author's workflow, built around three
goals Quarto's architecture can't deliver:

1. **Click-to-source.** Double-click a rendered element, jump to its `.qmd` source.
2. **Block-level incremental updates.** Saving a change swaps only the affected
   block(s) in place, preserving scroll position and the runtime state of live
   components (Three.js, `{js}` cells).
3. **No per-edit startup cost.** A long-running Rust server with a warm Jupyter kernel.

Output is **HTML only**. The project's own manual is two sibling books authored in
`.qmd`: the [User Guide](docs/guide/index.qmd) (how to use it) and the
[Internals](docs/internals/index.qmd) book (the architecture, websocket protocol,
and block model).

## Architecture (at a glance)

An editor-agnostic Rust dev server owns all logic behind a versioned websocket
protocol. A plain browser preview is the client; double-clicking a block opens
its source in your editor (a `vscode://` deep link by default). The protocol is
open, so a third-party editor client (a VS Code extension, etc.) can speak it too.

```
crates/core     parser (comrak + sourcepos) + block model + render
crates/server   dev server, websocket, file watcher, kernel pool
web-client/     browser preview client (vanilla JS), the only client
```

## Usage

`qmd-fast preview` runs a long-lived dev server: it watches the `.qmd` (and its
includes/bibliography), and on each save re-renders, **executes changed code cells
against a warm Jupyter kernel** (re-running only the earliest changed cell and
everything downstream), diffs against the previous block list, and pushes only the
changed blocks over a websocket. Unchanged blocks are never touched, so scroll
position and the runtime state of live blocks (Three.js, `{js}` cells) survive edits. Open
the preview in a browser; double-clicking a block jumps to its `.qmd` source.

Point it at a **single file** or a **directory** (a multi-page site project):

```sh
cargo run -p qmd-fast-server -- preview corpus/posts/born-machines.qmd  # one doc
cargo run -p qmd-fast-server -- preview corpus/tech-blog                # a whole site
cargo run -p qmd-fast-server -- preview corpus/tech-blog --host         # + LAN URL & QR
cargo run -p qmd-fast-server -- build   corpus/tech-blog                # static _site/
cargo run -p qmd-fast-server -- render  corpus/posts/born-machines.qmd > out.html
cargo run -p qmd-fast-server -- blocks  corpus/posts/born-machines.qmd
```

`--host` exposes the preview on your LAN with a phone-scannable QR code, gated by a
per-session access token baked into the printed URL (loopback access needs none).

Code execution needs a Python with `ipykernel`; point the server at it with the
`QMD_FAST_PYTHON` env var (defaults to `python3`). Cells render as source if no
kernel is available. Outputs (stdout/stderr, results, images, HTML, errors)
become their own blocks keyed to the cell, so they swap in place.

The render pipeline underneath: the core parses `.qmd` with comrak (sourcepos),
splits the document into top-level blocks with content-hash ids, and emits HTML
with `data-block-id` + `data-sourcepos` on every block.

## What it renders

- Prose, tables (with alignment), nested/tight lists, code cells; smart typography.
- **Syntax highlighting server-side** (syntect), emitted as theme-styled scope
  classes, so it ships offline (no CDN), paints highlighted on first load, and
  recolors instantly on the light/dark toggle. Copy button on every block.
- **Math server-side** via KaTeX: inline `$…$`, display `$$…$$`, `\begin{…}`
  environments; CSS + fonts bundled inline, fully offline.
- `{{< include >}}` resolution with a per-file source map (`data-source-file`), so
  click-to-source jumps into the included file.
- Callouts, `layout-ncol` grids, attributed `.btn` links, raw `{=html}` passthrough.
- Citations (`[@key]`) + an auto-generated References section, and cross-references
  (e.g. `@fig-`/`@eq-`/`@lst-`/`@tbl-`/`@sec-`/`@thm-`) into numbered, labelled
  anchor links.
- **Print/LaTeX figure export.** Inline matplotlib figures are web-themed without
  tainting global `rcParams`, so `savefig` stays print-clean; `#| fig-export: x.pdf`
  writes the figure to a vector/raster file (black-on-white) for `\includegraphics`.
- Live **`{js}`** cells (a tiny native enhancer with vendored d3 + Observable Plot,
  no Observable runtime), **mermaid** diagrams, a figure lightbox, themes
  (light/dark + custom), and a responsive reading layout (mobile TOC pull-up sheet,
  print stylesheet).
- **Multi-page sites** (`preview`/`build` a directory): a `_site.yml` project with
  a redesigned navbar/footer + book chapter prev/next, `.qmd`→`.html` link rewriting,
  `listing:` post-card indexes, and `about:` profile pages. Live preview navigates
  between pages and hot-reloads the edited one.
- **Live diagnostics** in the preview's dev panel: broken includes, missing kernels,
  config typos (with did-you-mean), and an advisory client-side accessibility audit
  (missing alt text, heading skips, low contrast), each click-to-source.

The native deck engine, mermaid, and the `{js}` cell enhancer are the only
client-side pieces; everything else (parse, render, highlight, math) happens in Rust.
See the [User Guide](docs/guide/index.qmd) and [Internals](docs/internals/index.qmd)
books, authored in `.qmd` and built with qmd-fast itself.
