# Taliesin

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)

> The native (and only) source extension is `.tmd`; the CLI is `taliesin`
> (with `tali` as a shorter alias).

A single-purpose, performance-oriented tool for authoring HTML from `.tmd`
files: blog posts, slide decks, books, and **multi-page websites**. Built for
one author's workflow around three goals:

1. **Click-to-source.** Ctrl-click (Cmd-click on Mac) a rendered element, jump to its `.tmd` source.
2. **Block-level incremental updates.** Saving a change swaps only the affected
   block(s) in place, preserving scroll position and the runtime state of live
   components (Three.js, `{js}` cells).
3. **No per-edit startup cost.** A long-running Rust server with a warm Jupyter kernel.

Output is **HTML only**. The project's own manual is two sibling books authored in
`.tmd`: the [User Guide](docs/guide/index.tmd) (how to use it) and the
[Internals](docs/internals/index.tmd) book (the architecture, websocket protocol,
and block model).

## Before you adopt it

Three things a stranger should know, each measured rather than asserted. The long version,
with the sources and the method, is [Choosing Taliesin](docs/guide/using/choosing.tmd).

- **Your source stays yours.** Across the 133 documents / 11,534 lines of the project's own
  corpus, **7.1% of lines carry any construct beyond plain CommonMark** — and all six of
  those families are existing Pandoc/Quarto vocabulary, not invented here. Check it
  yourself with `python3 tools/portability-census.py`. Your writing is
  Markdown in your repository; built pages are static HTML that needs no runtime, and
  `taliesin read --format json` projects a document to structured text.
- **Speed, in absolutes and with no multiplier.** A 15-page book builds in **0.25 s**
  (16.7 ms/page); `preview` is serving in **1–5 ms** for a single document and **≈200 ms**
  for a 23-page book; a warm keystroke-sized edit diffs in **0.7 ms** and ships a **3.2 KB**
  patch instead of a 270 KB page reload. These measure Taliesin's work only — a batch
  compiler doing a cold Pandoc pass is doing different work, so no ratio is quoted.
- **One maintainer, pre-1.0.** No support contract, no release cadence, no bus factor above
  one. What that risk is bounded by: Markdown source you already hold, built HTML with no
  dependency on this tool, and an AGPL-3.0 licence that makes a fork always available.

## Architecture (at a glance)

An editor-agnostic Rust dev server owns all logic behind a versioned websocket
protocol. A plain browser preview is the client; Ctrl-clicking a block opens
its source in your editor (a `vscode://` deep link by default). The protocol is
open, so a third-party editor client (a VS Code extension, etc.) can speak it too.

```
crates/core     parser (comrak + sourcepos) + block model + render
crates/server   dev server, websocket, file watcher, kernel pool
web-client/     browser preview client (vanilla JS), the only client
```

## Install & prerequisites

**Download a binary.** Every tagged release attaches a `.tar.gz` (with a `.sha256`
next to it) for each supported platform — one download, no toolchain:

| Platform | Target | Status |
| --- | --- | --- |
| Linux x86-64 | `x86_64-unknown-linux-gnu` | built and released |
| macOS Apple silicon | `aarch64-apple-darwin` | built and released |
| macOS Intel | `x86_64-apple-darwin` | built and released |
| Windows | — | **not supported**: never built, never tested, no gate covers it. The process and kernel layer is Unix-only. |

The tarball holds the `taliesin` binary plus `LICENSE` and `THIRD_PARTY.md`. Put the
binary anywhere on your `PATH`.

**Or build from source.** Taliesin is a Rust workspace (edition 2024); a recent stable
toolchain (via [rustup](https://rustup.rs)) is all you need:

```sh
git clone https://github.com/AJBogo9/taliesin && cd taliesin
cargo build --release            # binary at target/release/taliesin
cargo run -p taliesin-server -- --help   # or run it straight from the workspace
```

**What that costs, measured, so it is not a surprise:** a cold release build compiles
**252 crates in about 1m 40s** at `-j3`, and produces a single ~40 MB self-contained
binary (it embeds KaTeX with its fonts, the syntax-highlighting definitions, and every
bundled stylesheet and script, which is why rendered pages need no network). Nothing is
fetched at runtime and there is no `node_modules`. Put `target/release/taliesin` on your
`PATH` to call `taliesin` from anywhere.

**One optional feature, off by default, because it is a third of the build.**
`read --run-js` can drive a local headless Chrome to check whether a `{js}` cell actually
painted a chart. Its browser driver is the single most expensive thing in the dependency
graph: turning it on takes the same cold build from 1m 40s to **2m 30s** and 252 crates
to 268 (measured on one machine at `-j3`, so treat the ratio as the durable part). Since
it *also* needs a system Chrome at runtime and most people never invoke it, you only pay
for it if you ask:

```sh
cargo build --release --features taliesin-server/headless-js
```

The [release binaries](#install--prerequisites) are built with it, so a download is the complete tool.
Without it every other command is unchanged and `read --run-js` still answers — each
`{js}` cell just reports `skipped`, exactly as it does when no Chrome is installed.

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
| `TALIESIN_CELL_SILENCE` | `600` | Seconds a cell may produce **no output** before it is interrupted (SIGINT). This is the default liveness cap: a cell that keeps printing is never interrupted, however long it runs. `0` disables it. |
| `TALIESIN_CELL_TIMEOUT` | unset | Optional per-cell wall-clock cap in seconds, off by default. Set it to bound total runtime regardless of output; `0` disables it. |
| `TALIESIN_NO_CACHE` | unset | Ignore and skip writing the `_freeze/` execution cache (always re-run cells). |

(See `taliesin --help` for the rest: `TALIESIN_NO_EXEC`, `TALIESIN_NO_CLEAR`.)

**Quick start.** Scaffold a starter site and preview it:

```sh
taliesin init my-site        # _site.yml, index.tmd, AGENTS.md + .taliesin/ schemas
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
the preview in a browser; Ctrl-clicking a block jumps to its `.tmd` source.

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
  `listing:` post-card indexes, and `hero:` landing banners. Live preview navigates
  between pages and hot-reloads the edited one.
- **Live diagnostics** in the preview's dev panel: broken includes, missing kernels,
  config typos (with did-you-mean), and an advisory client-side accessibility audit
  (missing alt text, heading skips, low contrast), each click-to-source.

The native deck engine, mermaid, and the `{js}` cell enhancer are the only
client-side pieces; everything else (parse, render, highlight, math) happens in Rust.
See the [User Guide](docs/guide/index.tmd) and [Internals](docs/internals/index.tmd)
books, authored in `.tmd` and built with Taliesin itself.

## Documents you did not write

**Previewing a `.tmd` runs it.** `{python}` / `{r}` cells execute against a Jupyter
kernel with your permissions, a `{js}` cell runs in your browser, and raw HTML (plus
anything a document injects through `include-in-header` / `css:`) passes through
verbatim — so opening a document someone sent you is the same kind of decision as
running a script they sent you. `--no-exec` stops the code cells, both kinds, but it is
**not** a sanitizer. Taliesin says this plainly rather than implying a sandbox it does
not have: the full account is in
[the CLI reference](docs/guide/reference/cli.tmd) and the trust model is in
[`SECURITY.md`](SECURITY.md).

## Accessibility

The HTML Taliesin generates has a published **WCAG 2.1 AA conformance report**
([docs/guide/reference/accessibility.tmd](docs/guide/reference/accessibility.tmd)) — the
ACR half of a VPAT, in the form an institutional evaluator expects. It states what
conforms, what only partially conforms, and (at equal length) what has **not** been
evaluated: there has been no screen-reader pass and no full keyboard walkthrough, and the
report says so rather than claiming the automated results cover them.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) — it covers the one setup step git will not do
for you (`git config core.hooksPath .githooks`), the single command that runs every gate
(`./tools/gates.sh`), and the licence terms a contribution is submitted under.

## License

Taliesin is licensed under the **GNU Affero General Public License v3.0**
([`LICENSE`](LICENSE)), © 2026 Andreas Bogossian. The AGPL closes the "SaaS
loophole": anyone who runs a modified version as a network service must offer
their complete corresponding source to that service's users.

**What you build with it is yours.** A built page contains copies of Taliesin's own
CSS and JavaScript — that is what makes it work offline with no CDN — so the
[**Taliesin Output Exception**](LICENSE-OUTPUT-EXCEPTION.md) grants you the right to
publish that output under any terms you like, with nothing to attribute and no offer
of source. The AGPL governs *Taliesin*; it makes no claim on the documents you write
with it. Serving a page you built does not engage section 13.

As the sole copyright holder, the author is not bound by the AGPL grant and
reserves the right to offer Taliesin under other terms, including a proprietary
hosted service or a commercial license.

The VS Code editor companion under [`editor/vscode`](editor/vscode) is a separate
work licensed under the **MIT License** ([`editor/vscode/LICENSE`](editor/vscode/LICENSE)).
Vendored third-party assets keep their own licenses; see [`THIRD_PARTY.md`](THIRD_PARTY.md).
