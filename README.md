# Taliesin

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)

> The native (and only) source extension is `.tmd`; the CLI is `taliesin`.

A single-purpose, performance-oriented tool for authoring HTML from `.tmd`
files: blog posts, papers, books, and **multi-page websites**. Built for
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

- **Your source stays yours.** Across the 81 documents / 7,095 lines of the project's own
  corpus, **6.7% of lines carry any construct beyond plain CommonMark** — and all six of
  those families are existing Pandoc/Quarto vocabulary, not invented here. Check it
  yourself with `python3 tools/portability-census.py`. Your writing is
  Markdown in your repository, and built pages are static HTML that needs no runtime.
- **Speed, in absolutes and with no multiplier** (build and preview figures measured
  2026-08-10, warm-edit figures re-measured 2026-08-18). A 6-page book (`docs/internals`)
  builds in **0.13 s** (21.7 ms/page); `preview` is serving in **3 to 8 ms** for a single
  document and **≈130 ms** for a 16-page book; a warm keystroke-sized edit diffs in
  **0.35 ms** and ships a **32 KB** patch instead of a 287 KB page reload, and
  **53** of its 55 ops are metadata-only patches that never touch a DOM node — those 53
  plus the one `insert` for the newly typed paragraph total ~3.2 KB — which is why live
  state survives the edit. These measure Taliesin's work only — a batch compiler doing a
  cold Pandoc pass is doing different work, so no ratio is quoted.
- **One maintainer, and the scope is closed.** No support contract, no release cadence, no
  bus factor above one. 1.0 means the feature set is final for this tool's one use case,
  not that a team stands behind it. What that risk is bounded by: Markdown source you
  already hold, built HTML with no dependency on this tool, and an AGPL-3.0 licence that
  makes a fork always available.

## Project status

**Taliesin 1.0 is feature-complete for its one use case:** rendering `.tmd` to HTML for one
author's writing workflow. The scope is deliberately closed.

- **Bug reports are welcome.** Something rendering wrongly, a crash, a diagnostic that
  fires on valid source: please open an issue.
- **Feature requests are closed by design**, not by backlog order. The tool is built around
  subtraction, and a 2026-08 campaign cut roughly 40% of the tree to get here. Adding an
  output format (PDF, LaTeX, Word, ePub) is out of scope permanently; HTML is the only
  target.
- **Security reports go through `SECURITY.md`**, privately, not as a public issue.

`CONTRIBUTING.md` has the scope rules in full. If you want something the tool will not do,
the AGPL licence means forking is always available and is often the honest answer.

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

**Download a prebuilt binary.** Every `v*` tag attaches one `.tar.gz` per platform with a
`.sha256` beside it, holding the binary plus `LICENSE`, `THIRD_PARTY.md` and the bundled
dependencies' licence notices. There is nothing else to install and nothing is fetched at
runtime: KaTeX with its fonts, the syntax definitions and every bundled stylesheet and
script live inside the binary.

| Platform | `TARGET` |
| --- | --- |
| Linux x86-64 (static, any distro) | `x86_64-unknown-linux-musl` |
| macOS, Apple silicon | `aarch64-apple-darwin` |
| macOS, Intel | `x86_64-apple-darwin` |

```sh
VERSION=v1.0.1
TARGET=x86_64-unknown-linux-musl        # your row from the table above
BASE=https://github.com/AJBogo9/taliesin/releases/download

curl -LO "$BASE/$VERSION/taliesin-$VERSION-$TARGET.tar.gz"
curl -LO "$BASE/$VERSION/taliesin-$VERSION-$TARGET.tar.gz.sha256"
shasum -a 256 -c "taliesin-$VERSION-$TARGET.tar.gz.sha256"          # must print: OK
tar xzf "taliesin-$VERSION-$TARGET.tar.gz"
install -m755 "taliesin-$VERSION-$TARGET/taliesin" ~/.local/bin/    # or anywhere on PATH
taliesin --help
```

The Linux build is statically linked against musl, so it has no glibc floor and runs on
any distribution. The macOS builds are unsigned and unnotarized: fetched with `curl` as
above they carry no quarantine attribute and run straight away, but downloaded through a
browser they do, and the first launch is refused until you clear it with
`xattr -d com.apple.quarantine ./taliesin`.

**Windows is not supported**: never built, never tested, no gate covers it, and the
process and kernel layer is Unix-only.

**Or build from source**, which is always supported. Taliesin is a Rust workspace
(edition 2024), so a recent stable toolchain (via [rustup](https://rustup.rs)) is all you
need:

```sh
git clone https://github.com/AJBogo9/taliesin && cd taliesin
cargo build --release            # binary at target/release/taliesin
cargo run -p taliesin-server -- --help   # or run it straight from the workspace
```

**What that costs, measured 2026-08-20 on a genuine cold build, so it is not a surprise:**
`cargo clean` followed by `cargo build --release -p taliesin-server` compiles **229 crates
in about 1m 34s** (16-core machine, cargo's default parallelism) and produces a single
~30 MB self-contained binary (30,138,576 bytes; it embeds KaTeX with its fonts, the
syntax-highlighting definitions, and every bundled stylesheet and script, which is why
rendered pages need no network). `Cargo.lock` lists 299 packages across the whole
workspace, higher than the 229 actually compiled because it also covers the separate
benchmark tool and dev-only dependencies the shipped binary never links. Nothing is
fetched at runtime and there is no `node_modules`. Put `target/release/taliesin` on your
`PATH` to call `taliesin` from anywhere.

**Jupyter-kernel prerequisites (only for executing code cells).** Prose, math,
highlighting, and sites render with no kernel at all; a kernel is needed only
to *run* `{python}` code cells (without one they render as source), which use one
warm kernel reused across edits:

- **`{python}` cells** need a Python with [`ipykernel`](https://pypi.org/project/ipykernel/)
  (`python3 -m pip install ipykernel`).

`{js}` cells run in the browser and need no kernel.

**Environment variables.**

| Variable | Default | Effect |
| --- | --- | --- |
| `TALIESIN_PYTHON` | `python3` | Interpreter used for `{python}` cells (point it at a venv). |
| `TALIESIN_CELL_SILENCE` | `600` | Seconds a cell may produce **no output** before it is interrupted (SIGINT). This is the default liveness cap: a cell that keeps printing is never interrupted, however long it runs. `0` disables it. |
| `TALIESIN_CELL_TIMEOUT` | unset | Optional per-cell wall-clock cap in seconds, off by default. Set it to bound total runtime regardless of output; `0` disables it. |
| `TALIESIN_NO_CACHE` | unset | Ignore and skip writing the `_freeze/` execution cache (always re-run cells). |

(See `taliesin --help` for the rest: `TALIESIN_NO_EXEC`, `TALIESIN_NO_CLEAR`.)

**Quick start.** Scaffold a starter site and preview it:

```sh
taliesin init my-site        # _site.yml, index.tmd, and one dated example post
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
cargo run -p taliesin-server -- build   corpus/tech-blog                # static _site/
cargo run -p taliesin-server -- build   corpus/posts/born-machines.tmd --stdout > out.html
```

The preview binds to loopback only.

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
- Callouts, `layout-ncol` grids, raw `{=html}` passthrough.
- Citations (`[@key]`) + an auto-generated References section, and cross-references
  (`@fig-`/`@eq-`/`@lst-`/`@tbl-`/`@sec-`) into numbered, labelled anchor links.
- **Print-clean matplotlib.** Inline figures are web-themed without tainting global
  `rcParams`, so a `savefig` inside the same cell stays black-on-white.
- Live **`{js}`** cells (a tiny native enhancer with vendored d3 + Observable Plot,
  no Observable runtime), **mermaid** diagrams, and a responsive reading layout
  (print stylesheet). Light and dark palettes both ship and the reader's device
  selects one, with no per-site theme control to configure.
- **Multi-page sites** (`preview`/`build` a directory): a `_site.yml` project with
  a redesigned navbar/footer + book chapter prev/next, `.tmd`→`.html` link rewriting,
  `listing:` post-card indexes, and `hero:` landing banners. Live preview navigates
  between pages and hot-reloads the edited one.
- **Live diagnostics** in the preview's dev panel: broken includes, missing kernels,
  config typos (with did-you-mean), and an advisory server-side accessibility audit
  (missing alt text, heading skips), each click-to-source.

Mermaid and the `{js}` cell enhancer are the only
client-side pieces; everything else (parse, render, highlight, math) happens in Rust.
See the [User Guide](docs/guide/index.tmd) and [Internals](docs/internals/index.tmd)
books, authored in `.tmd` and built with Taliesin itself.

## Documents you did not write

**Previewing a `.tmd` runs it.** `{python}` cells execute against a Jupyter
kernel with your permissions, a `{js}` cell runs in your browser, and raw HTML (plus
anything the project injects through `_site.yml`'s `head:`) passes through
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
