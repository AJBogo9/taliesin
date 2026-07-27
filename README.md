# Taliesin

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)

> The native (and only) source extension is `.tmd`; the CLI is `taliesin`
> (with `tali` as a shorter alias).

A single-purpose, performance-oriented tool for authoring HTML from `.tmd`
files: blog posts, slide decks, books, and **multi-page websites**. Built for
one author's workflow around three goals:

1. **Click-to-source.** Alt-click (Option-click on Mac) a rendered element, jump to its `.tmd` source.
2. **Block-level incremental updates.** Saving a change swaps only the affected
   block(s) in place, preserving scroll position and the runtime state of live
   components (Three.js, `{js}` cells).
3. **No per-edit startup cost.** A long-running Rust server with a warm Jupyter kernel.

Output is **HTML only**. The project's own manual is two sibling books authored in
`.tmd`: the [User Guide](docs/guide/index.tmd) (how to use it) and the
[Internals](docs/internals/index.tmd) book (the architecture, websocket protocol,
and block model).

## Architecture (at a glance)

An editor-agnostic Rust dev server owns all logic behind a versioned websocket
protocol. A plain browser preview is the client; Alt-clicking a block opens
its source in your editor (a `vscode://` deep link by default). The protocol is
open, so a third-party editor client (a VS Code extension, etc.) can speak it too.

```
crates/core     parser (comrak + sourcepos) + block model + render
crates/server   dev server, websocket, file watcher, kernel pool
web-client/     browser preview client (vanilla JS), the only client
```

## Install & prerequisites

**Build from source.** Taliesin is a Rust workspace (edition 2024); a recent stable
toolchain (via [rustup](https://rustup.rs)) is all you need to build it:

```sh
git clone https://github.com/AJBogo9/taliesin && cd taliesin
cargo build --release            # binary at target/release/taliesin
cargo run -p taliesin-server -- --help   # or run it straight from the workspace
```

Put `target/release/taliesin` on your `PATH` to call `taliesin` from anywhere.

**Jupyter-kernel prerequisites (only for executing code cells).** Prose, math,
highlighting, decks, and sites render with no kernel at all; a kernel is needed only
to *run* `{python}`/`{r}` code cells (without one they render as source). Each
language runs against its own warm kernel:

- **`{python}` cells** need a Python with [`ipykernel`](https://pypi.org/project/ipykernel/)
  (`python3 -m pip install ipykernel`).
- **`{r}` cells** need an R with [IRkernel](https://irkernel.github.io)
  (`install.packages("IRkernel")`).

`{js}` cells run in the browser and need no kernel.

**Environment variables.**

| Variable | Default | Effect |
| --- | --- | --- |
| `TALIESIN_PYTHON` | `python3` | Interpreter used for `{python}` cells (point it at a venv). |
| `TALIESIN_R` | `R` | Interpreter used for `{r}` cells. |
| `TALIESIN_CELL_TIMEOUT` | `120` | Per-cell wall-clock seconds before a runaway cell is interrupted (SIGINT); `0` disables the limit. |
| `TALIESIN_NO_CACHE` | unset | Ignore and skip writing the `_freeze/` execution cache (always re-run cells). |

(See `taliesin --help` for the rest: `TALIESIN_OPEN`, `TALIESIN_HOST`,
`TALIESIN_NO_EXEC`, `TALIESIN_NO_CLEAR`.)

**Quick start.** Scaffold a starter site and preview it:

```sh
taliesin init my-site        # writes my-site/_site.yml + my-site/index.tmd
taliesin preview my-site     # live preview at http://localhost:4321
```

`init` refuses to overwrite existing files, so it is safe to run in a populated dir.

## Usage

`taliesin preview` runs a long-lived dev server: it watches the `.tmd` (and its
includes/bibliography), and on each save re-renders, **executes changed code cells
against a warm Jupyter kernel** (re-running only the earliest changed cell and
everything downstream), diffs against the previous block list, and pushes only the
changed blocks over a websocket. Unchanged blocks are never touched, so scroll
position and the runtime state of live blocks (Three.js, `{js}` cells) survive edits. Open
the preview in a browser; Alt-clicking a block jumps to its `.tmd` source.

Point it at a **single file** or a **directory** (a multi-page site project):

```sh
cargo run -p taliesin-server -- preview corpus/posts/born-machines.tmd  # one doc
cargo run -p taliesin-server -- preview corpus/tech-blog                # a whole site
cargo run -p taliesin-server -- preview corpus/tech-blog --host         # + LAN URL & QR
cargo run -p taliesin-server -- build   corpus/tech-blog                # static _site/
cargo run -p taliesin-server -- render  corpus/posts/born-machines.tmd > out.html
cargo run -p taliesin-server -- blocks  corpus/posts/born-machines.tmd
```

`--host` exposes the preview on your LAN with a phone-scannable QR code, gated by a
per-session access token baked into the printed URL (loopback access needs none).

Code execution needs a Python with `ipykernel`; point the server at it with the
`TALIESIN_PYTHON` env var (defaults to `python3`). Cells render as source if no
kernel is available. Outputs (stdout/stderr, results, images, HTML, errors)
become their own blocks keyed to the cell, so they swap in place.

The render pipeline underneath: the core parses `.tmd` with comrak (sourcepos),
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
  a redesigned navbar/footer + book chapter prev/next, `.tmd`→`.html` link rewriting,
  `listing:` post-card indexes, and `about:` profile pages. Live preview navigates
  between pages and hot-reloads the edited one.
- **Live diagnostics** in the preview's dev panel: broken includes, missing kernels,
  config typos (with did-you-mean), and an advisory client-side accessibility audit
  (missing alt text, heading skips, low contrast), each click-to-source.

The native deck engine, mermaid, and the `{js}` cell enhancer are the only
client-side pieces; everything else (parse, render, highlight, math) happens in Rust.
See the [User Guide](docs/guide/index.tmd) and [Internals](docs/internals/index.tmd)
books, authored in `.tmd` and built with Taliesin itself.

## License

Taliesin is licensed under the **GNU Affero General Public License v3.0**
([`LICENSE`](LICENSE)), © 2026 Andreas Bogossian. The AGPL closes the "SaaS
loophole": anyone who runs a modified version as a network service must offer
their complete corresponding source to that service's users.

As the sole copyright holder, the author is not bound by the AGPL grant and
reserves the right to offer Taliesin under other terms, including a proprietary
hosted service or a commercial license.

The VS Code editor companion under [`editor/vscode`](editor/vscode) is a separate
work licensed under the **MIT License** ([`editor/vscode/LICENSE`](editor/vscode/LICENSE)).
Vendored third-party assets keep their own licenses; see [`THIRD_PARTY.md`](THIRD_PARTY.md).
