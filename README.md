# Taliesin

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)

Taliesin turns `.tmd` files into HTML: blog posts, papers, books and multi-page sites. A
`.tmd` file is Markdown with executable code cells, in the syntax Pandoc and Quarto use
(fenced divs, attributes, citations, cross-references, `{python}` cells), plus `{js}`
cells that run in the browser. It is for people who write in their own editor and publish
static HTML, the same niche as Quarto, MyST and Jupyter Book. Where it differs:

1. **Click-to-source.** Ctrl-click (Cmd-click on Mac) an element in the preview to jump to
   its `.tmd` source.
2. **Block-level live updates.** Saving swaps only the changed block(s) in the open
   `taliesin preview`, preserving scroll position and the runtime state of live components
   (Three.js, `{js}` cells).
3. **A warm server and kernel.** The server and its Jupyter kernel stay running between
   edits, so a save re-runs code from the first changed cell on, not the whole document.
4. **HTML only.** There is no PDF, LaTeX or Word output.

The manual is two books written in `.tmd` and built with Taliesin: the
[User Guide](https://guide.taliesin.sh/) (how to use it, starting with
[Getting started](https://guide.taliesin.sh/using/getting-started.html)) and
[Internals](https://internals.taliesin.sh/) (the architecture, websocket protocol and
block model).

## Before you adopt it

[Choosing Taliesin](https://guide.taliesin.sh/using/choosing.html) covers each point below
at length, with its sources and method.

- **Portability.** Across the 81 documents / 7,202 lines of the project's own corpus,
  6.6% of lines carry any construct beyond plain CommonMark, and all six construct
  families involved are existing Pandoc/Quarto vocabulary. Check it yourself with
  `python3 tools/portability-census.py`. Your writing is Markdown in your repository, and
  built pages are static HTML that needs no runtime.
- **Speed** (figures re-measured 2026-08-27, the single-document ready time 2026-09-01,
  on a 16-core machine). A 6-page book (`docs/internals`) builds in 0.15 s (25 ms/page);
  `preview` is serving in ≈50 ms for a single document (spawn to first HTTP 200) and
  ≈90 ms for a 16-page book; a warm keystroke-sized edit diffs in 0.21 ms and ships a
  32 KB patch instead of a 287 KB page reload, and 53 of its 55 ops are metadata-only
  patches that never touch a DOM node (those 53 plus the one `insert` for the newly typed
  paragraph total ~3.2 KB), which is why live state survives the edit. These figures
  measure Taliesin's work only and are not comparable with a cold Pandoc pass by a batch
  compiler, which does different work.
- **One maintainer, and the scope is closed.** There is no support contract or release
  cadence, and the bus factor is one. 1.0 means the feature set is final for this tool's
  one use case, not that a team stands behind it. The risk is limited because the source
  is Markdown you already hold, built HTML has no dependency on this tool, and the
  AGPL-3.0 licence makes a fork always available.

## Project status

Taliesin 1.0 is feature-complete for its one use case: rendering `.tmd` to HTML for one
author's writing workflow. The scope is closed.

- **Bug reports are welcome.** Something rendering wrongly, a crash, a diagnostic that
  fires on valid source: please open an issue.
- **Feature requests are closed by design**, not deferred. The tool is designed by
  subtraction, and a 2026-08 campaign cut roughly 40% of the tree to reach this scope.
  Adding an output format (PDF, LaTeX, Word, ePub) is out of scope permanently; HTML is
  the only target.
- **Security reports go through `SECURITY.md`**, privately, not as a public issue.

`CONTRIBUTING.md` has the scope rules in full. If you want something the tool will not do,
the AGPL licence means forking is always available and is often the right choice.

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
mkdir -p ~/.local/bin
install -m755 "taliesin-$VERSION-$TARGET/taliesin" ~/.local/bin/    # or anywhere on PATH
taliesin --help
```

`~/.local/bin` must be on your `PATH` for the last line to work: macOS does not add it,
and Ubuntu's `~/.profile` adds it at the next login once the directory exists.

The Linux build is statically linked against musl, so it has no glibc floor and runs on
any distribution. The macOS builds are unsigned and unnotarized: fetched with `curl` as
above they carry no quarantine attribute and run straight away, but downloaded through a
browser they do, and the first launch is refused until you clear it with
`xattr -d com.apple.quarantine ./taliesin`.

**Windows is not supported.** It has never been built or tested, no gate covers it, and
the process and kernel layer is Unix-only.

**Or build from source**, which is always supported. Taliesin is a Rust workspace
(edition 2024), so a recent stable toolchain (via [rustup](https://rustup.rs)) is all you
need:

```sh
git clone https://github.com/AJBogo9/taliesin && cd taliesin
cargo build --release            # binary at target/release/taliesin
cargo run -p taliesin-server -- --help   # or run it straight from the workspace
```

**Build cost** (measured 2026-08-26 on a cold build). `cargo clean` followed by
`cargo build --release -p taliesin-server` compiles 220 crates in about 48s (16-core
machine, cargo's default parallelism) and produces a single ~30 MB self-contained binary
(30,296,240 bytes, re-measured 2026-08-27; it embeds KaTeX with its fonts, the
syntax-highlighting definitions, and every bundled stylesheet and script, which is why
rendered pages need no network). `Cargo.lock` lists 289 packages across the whole
workspace, higher than the 220 actually compiled because it also covers the separate
benchmark tool and dev-only dependencies the shipped binary never links. Nothing is
fetched at runtime and there is no `node_modules`. Put `target/release/taliesin` on your
`PATH` to call `taliesin` from anywhere.

**Jupyter-kernel prerequisites (only for executing code cells).** Prose, math,
highlighting, and sites render with no kernel; a kernel is needed only
to *run* `{python}` code cells (without one they render as source), which use one
warm kernel reused across edits:

- **`{python}` cells** need a Python with [`ipykernel`](https://pypi.org/project/ipykernel/).
  In your project directory, run `python3 -m venv .venv && .venv/bin/pip install ipykernel`
  (on Debian and Ubuntu, `python3 -m venv` needs the `python3-venv` package). Taliesin finds
  a project `.venv` with no configuration; `taliesin doctor` shows which Python it picked. A
  system `pip install` is refused on current Debian, Ubuntu and Homebrew Pythons (PEP 668).

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
includes/bibliography), and on each save re-renders, executes changed code cells
against a warm Jupyter kernel (re-running only the earliest changed cell and
everything downstream), diffs against the previous block list, and pushes only the
changed blocks over a websocket. Unchanged blocks are never touched, so scroll
position and the runtime state of live blocks (Three.js, `{js}` cells) survive edits. Open
the preview in a browser; Ctrl-clicking a block jumps to its `.tmd` source.

Point it at a single file or a directory (a multi-page site project):

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

The core parses `.tmd` with comrak (sourcepos), splits the document into top-level
blocks with content-hash ids, and emits HTML with `data-block-id` + `data-sourcepos` on
every block.

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
See the [User Guide](https://guide.taliesin.sh/) and [Internals](https://internals.taliesin.sh/)
books, authored in `.tmd` and built with Taliesin itself.

## Documents you did not write

**Previewing a `.tmd` runs it.** `{python}` cells execute against a Jupyter
kernel with your permissions, a `{js}` cell runs in your browser, and raw HTML (plus
anything the project injects through `_site.yml`'s `head:`) passes through
verbatim. Opening a document someone sent you is the same kind of decision as running a
script they sent you. `--no-exec` stops the code cells, both kinds, but it is not a
sanitizer, and Taliesin has no sandbox. The full account is in
[the CLI reference](https://guide.taliesin.sh/reference/cli.html) and the trust model is in
[`SECURITY.md`](SECURITY.md).

## Accessibility

The HTML Taliesin generates has a published WCAG 2.1 AA conformance report
([Accessibility](https://guide.taliesin.sh/reference/accessibility.html)), the
ACR half of a VPAT, in the form an institutional evaluator expects. It states what
conforms, what only partially conforms, and (at equal length) what has not been
evaluated: there has been no screen-reader pass and no full keyboard walkthrough, and the
report says the automated results do not cover them.

## Contributing

[`CONTRIBUTING.md`](CONTRIBUTING.md) covers the setup step git will not do for you
(`git config core.hooksPath .githooks`), the command that runs every gate
(`./tools/gates.sh`), and the licence terms a contribution is submitted under.

## License

Taliesin is licensed under the **GNU Affero General Public License v3.0**
([`LICENSE`](LICENSE)), © 2026 Andreas Bogossian. The AGPL closes the "SaaS
loophole": anyone who runs a modified version as a network service must offer
their complete corresponding source to that service's users.

**What you build with it is yours.** A built page contains copies of Taliesin's own
CSS and JavaScript (which is what makes it work offline with no CDN), so the
[**Taliesin Output Exception**](LICENSE-OUTPUT-EXCEPTION.md) grants you the right to
publish that output under any terms you like, with nothing to attribute and no offer
of source. The AGPL governs Taliesin; it makes no claim on the documents you write
with it. Serving a page you built does not engage section 13.

As the sole copyright holder, the author is not bound by the AGPL grant and
reserves the right to offer Taliesin under other terms, including a proprietary
hosted service or a commercial license.

The VS Code editor companion under [`editor/vscode`](editor/vscode) is a separate
work licensed under the **MIT License** ([`editor/vscode/LICENSE`](editor/vscode/LICENSE)).
Vendored third-party assets keep their own licenses; see [`THIRD_PARTY.md`](THIRD_PARTY.md).
