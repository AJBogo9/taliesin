# qmd-fast

A single-purpose Rust dev server that renders `.qmd` files to **HTML only** (blog
posts, slide decks, books, multi-page sites) for one author's workflow, built around
three load-bearing goals: click-to-source, block-level incremental updates, and no
per-edit startup cost (warm server + Jupyter kernel). It is **not** a general document
compiler: HTML is the only output target (no LaTeX/Typst/Word/ePub; a future print/PDF
track would render *from* the built HTML, never as a parallel format).

**Scope is corpus-plus-roadmap.** "Done" still means the documents under `corpus/`
render correctly: the corpus is the regression net and the arbiter of done. But the
corpus now *leads* as well as records: each new capability ships pinned by a target
corpus document added in the same change, so scope can grow deliberately toward "wider
than Quarto in web-native capability" without ever outrunning the test net. **"Wider"
means richer browser behavior in a live HTML view, not new output formats, and never at
the cost of the load-bearing invariants or the Do-NOT-touch discipline.** The active
roadmap is `notes/BEYOND-QUARTO.md` (successor to the completed `notes/DROP-QUARTO.md`); the prior
"the corpus is the spec / not a general Quarto replacement" framing is superseded by it.

**The `.qmd` file is the single editing surface; the browser is a read-only view.**
Edits flow one way: you change the source in your editor, the preview re-renders.
Click-to-source is the only bridge back, and it *navigates* (preview → editor
cursor), it never *writes*. The preview must not mutate the source. A
drag-to-reorder-slides feature once broke this and was removed: a second write path
fights click-to-source over who owns the file (editor-buffer vs. on-disk conflicts),
and "you may reorder but not edit/delete" is an arbitrary line that invites WYSIWYG
scope creep. The in-scope way to make a source edit ergonomic is an editor command,
not a preview gesture.

## Where things are

```
crates/core      qmd-fast-core lib: parser (comrak + sourcepos) → block model → render
  src/render/      block model + emission (a module dir):
    mod.rs           the render pipeline (parse → block model → HTML) + head/asset helpers
    model.rs         the block-model data types (Cell, Block, RenderedDoc, PageIncludes)
    tests.rs         render unit + corpus-invariant tests
    deck.rs          slide decks on qmd-fast's OWN engine (reveal.js removed): bundles
                     deck.css/deck.js, emits the native `.qmd-deck`/`.qmd-slides`
                     contract + a `window.QmdDeck` API (no reveal vocabulary)
    emit.rs          per-block HTML (server-side highlighting, code line-wrapping)
    divs.rs          `:::` fenced divs (callouts, columns, magic-move)
    figure.rs        numbered figures + captions
    extension/       format extensions (`_extensions/`) + shortcode expansion, incl. the
                     built-in `{{< embed deck.qmd >}}` + `{{< video clip.mp4 dark= >}}`
    theme.rs         `--qmd-*` CSS-variable themes (light/dark, extension themes)
    page.rs          full HTML-page assembly (PAGE_TEMPLATE shell, site-chrome wiring,
                     favicon): RenderedDoc → standalone page for build + in-process render
  src/diff.rs      block-level diff (BlockOp) for incremental updates
  src/includes.rs  {{< include >}} resolution + per-file source map
  src/frontmatter.rs YAML front-matter parse + lint (typo warnings)
  src/math.rs      KaTeX server-side render (bundled CSS/fonts, offline)
  src/highlight.rs server-side syntax highlighting (syntect → `qhl-` scope classes)
  src/cite.rs      citations ([@key]) + cross-references (@fig-, @sec-)
  src/site/        multi-page project (mod.rs): _site.yml config (config/), page
                   discovery, chrome, link rewrite, listings + about/`hero:` blocks,
                   front-matter parse (frontmatter.rs), books (book.rs),
                   Cmd-K search (search.rs), cross-refs (xref.rs); an {{< embed >}}-
                   referenced deck is built/served but kept out of nav. `mounts:`
                   serves another project (e.g. the docs book) under a URL prefix in preview
  assets/          bundled offline: css/ (base, dark, deck, site),
                   js/ (deck.js, code-enhance.js, mermaid.js, qmd-js.js + vendored
                   plot.umd.min.js/d3.min.js for `{js}` cells), katex/
crates/server    qmd-fast-server, bin `qmd-fast`: CLI + websocket dev server
  src/main.rs      render / blocks / build / serve subcommands (a dir = a site project)
  src/serve.rs     single-doc axum websocket + notify file watcher
  src/serve_site.rs multi-page site server (per-page state/executor, cross-page nav, hot reload)
  src/exec.rs      runs a doc's code cells, splices outputs back as blocks; plans
                   what re-runs via cumulative-hash keys (warm reuse + cold replay)
  src/freeze.rs    persistent execution cache (`_freeze/<page>.json`): rendered cell
                   outputs keyed by a cumulative content hash, so unchanged cells
                   restore instead of re-executing across builds + preview restarts
  src/kernel.rs    warm Jupyter kernel (ZMQ), reused across edits
  src/log.rs       colorized dev-server console output (to stderr)
web-client/      browser preview client (vanilla JS, the only client): client.js mounts
                 blocks + applies ops (Alt-click opens source in the editor),
                 search.js (Cmd-K), toc-spy.js (scrollspy)
docs/            project's own manual: TWO sibling book projects, authored in .qmd
                 (dogfooding). docs/guide/ = User Guide (using/ + reference/ + demo/tour
                 decks); docs/internals/ = Internals book. docs/ itself is just a container
                 (no _site.yml). The site mounts each at /docs/guide + /docs/internals.
corpus/          the real .qmd docs (the spec); cargo test renders them all
```

## Read before working

- **docs/** is the project's own manual, authored in `.qmd` as TWO sibling book
  projects (dogfooding):
  - **`docs/guide/`** = the User Guide (`using/` feature showcase + `reference/`):
    how to *use* qmd-fast. Preview it: `qmd-fast preview docs/guide`.
  - **`docs/internals/`** = the Internals book: the architecture, the rendering
    pipeline, the deck engine, the block model, the execution model, the dev server,
    and how to extend it. Preview it: `qmd-fast preview docs/internals`.
  - `docs/` itself is just a container (no `_site.yml`); the books are siblings
    because the page-walker would otherwise swallow a nested book's pages. The
    marketing site mounts them at `/docs/guide` + `/docs/internals`. Cross-book links
    are written as relative `.html` (e.g. `../guide/using/formats.html`).
- **corpus/README.md** for what the test documents exercise.

## Commands

```sh
cargo run -p taliesin-server -- preview <file.qmd> [port]      # live preview (aliases: dev, serve)
cargo run -p taliesin-server -- preview <file.qmd> --host      # + expose on LAN with a phone QR code
cargo run -p taliesin-server -- preview <dir>                  # live multi-page SITE preview (nav + per-page hot reload)
cargo run -p taliesin-server -- build  <file.qmd> [out.html]   # self-contained HTML file (default <name>.html)
cargo run -p taliesin-server -- build  <file.qmd> --out <dir>  # portable folder: <dir>/index.html + copied local assets
cargo run -p taliesin-server -- build  <dir> [--out <dir>]     # multi-page SITE -> _site/ (one .html per page + assets)
cargo run -p taliesin-server -- render <file.qmd> > out.html   # one-shot full page to stdout
cargo run -p taliesin-server -- blocks <file.qmd>              # block ids + sourcepos (debug)
cargo test -p taliesin-core                                    # corpus invariants + unit tests
cd web-client && npx -y -p typescript tsc -p jsconfig.json     # type-check client.js (// @ts-check, no build step)
```

A `qmd-fast` launcher on `PATH` (`~/.local/bin/qmd-fast`) rebuilds the release
binary when the tool's sources change, then runs it, so `qmd-fast preview <file>`
works from anywhere.

Executing code cells needs a matching Jupyter kernel: `{python}` cells need a
Python with `ipykernel` (`QMD_FAST_PYTHON`, default `python3`); `{r}` cells need an
R with `IRkernel` (`QMD_FAST_R`, default `R`). Each language runs against its own
warm kernel. Without a kernel, cells render as source and the preview shows a
"kernel unavailable" diagnostic. A cell that runs longer than `QMD_FAST_CELL_TIMEOUT`
seconds (default 120; `0` disables) is interrupted (SIGINT) so a runaway cell can't
wedge the kernel; the warm kernel and prior cells survive.

Cell outputs persist in `_freeze/` (gitignored), keyed by a cumulative content hash
(this cell's code + all upstream same-language code + interpreter id) — so a change
to a cell or anything upstream busts it and everything downstream, with no stale
hits and nothing to clear by hand. An unchanged doc replays from disk on the next
`build`/preview without booting the kernel; a warm preview still re-runs only the
edited cell + downstream. Errors and `#| cache: false` cells are never persisted.
`QMD_FAST_NO_CACHE` ignores + skips writing the cache; "Restart kernel" forces a
fresh re-run. (Kernel *variable* state is never cached — that's what makes Quarto's
per-cell `cache` fragile — so a cold start can only skip work when the whole
document is unchanged.) See `crates/server/src/freeze.rs`.

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
- Minimal config: perfect the default before adding a knob. Aim for a
  near-perfect default experience so the user does not *need* to configure;
  only explore configuration once the defaults are perfected, and prefer a
  better default over a new option. Reader-local a11y preferences (theme, text
  size, spacing) are exempt, they are personal, not document config. This is the
  deciding lens for any new user-facing control.
