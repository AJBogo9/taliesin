# Taliesin

A single-purpose Rust dev server that renders `.tmd` files to **HTML only** (blog
posts, slide decks, books, multi-page sites) for one author's workflow, built around
three load-bearing goals: click-to-source, block-level incremental updates, and no
per-edit startup cost (warm server + Jupyter kernel). It is **not** a general document
compiler: HTML is the only output target (no LaTeX/Typst/Word/ePub; a future print/PDF
track would render *from* the built HTML, never as a parallel format).

**Scope is corpus-plus-roadmap.** "Done" still means the documents under `corpus/`
render correctly: the corpus is the regression net and the arbiter of done. But the
corpus now *leads* as well as records: each new capability ships pinned by a target
corpus document added in the same change, so scope can grow deliberately toward "wider
than existing computational-document tools in web-native capability" without ever
outrunning the test net. **"Wider" means richer browser behavior in a live HTML view,
not new output formats, and never at the cost of the load-bearing invariants or the
Do-NOT-touch discipline.** The active roadmap is `notes/ROADMAP.md` (successor to the
completed `notes/native-rewrite.md`); the prior "the corpus is the spec / not a general
document compiler" framing is superseded by it.

**"Do-NOT-touch" is one freeze, not two.** The only *standing* freeze is warm-page
eviction: `MAX_WARM_PAGES` plus the deterministic LRU order in
`serve_site/exec_pool.rs`, which the build relies on (an accidental reorder is not
test-guarded, so it breaks silently). The 7-item "Do NOT touch" list in
`notes/native-rewrite.md` is a *completed rewrite-scoping* decision (don't rewrite
those subsystems for parity, since a rewrite comes out identical-or-worse), **not** a
standing freeze; their behavior may still change when a change makes the tool better.

**The `.tmd` file is the single editing surface; the browser is a read-only view.**
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
crates/core      taliesin-core lib: parser (comrak + sourcepos) → block model → render
  src/render/      block model + emission (a module dir):
    mod.rs           the render pipeline (parse → block model → HTML) + head/asset helpers
    model.rs         the block-model data types (Cell, Block, RenderedDoc, PageIncludes)
    tests.rs         render unit + corpus-invariant tests
    deck.rs          slide decks on Taliesin's OWN engine (reveal.js removed): bundles
                     deck.css/deck.js, emits the native `.tali-deck`/`.tali-slides`
                     contract + a `window.TaliesinDeck` API (no reveal vocabulary).
                     The old back-compat alias is deleted; `window.TaliesinDeck` is
                     the only name
    emit.rs          per-block HTML (server-side highlighting, code line-wrapping)
    divs.rs          `:::` fenced divs (callouts, columns, magic-move)
    figure.rs        numbered figures + captions
    extension/       shortcode expansion, incl. the built-in `{{< embed deck.tmd >}}`
                     + `{{< video clip.mp4 dark= >}}`. NOT `_extensions/`, which is a
                     theme-CSS lookup in `theme.rs` and nothing else: there is no
                     format-extension mechanism (`frontmatter.rs` says so at the
                     `format:` sub-key validator)
    theme.rs         `--tali-*` CSS-variable themes (light/dark, extension themes).
                     The storage key is `tali-theme` and the event is
                     `tali:themechange`; `crates/core/tests/retired_names.rs` keeps the
                     retired `q`-prefix spelling out of the tree
    page.rs          full HTML-page assembly (PAGE_TEMPLATE shell, site-chrome wiring,
                     favicon): RenderedDoc → standalone page for build + in-process render
  src/diff.rs      block-level diff (BlockOp) for incremental updates
  src/includes.rs  {{< include >}} resolution + per-file source map
  src/frontmatter.rs YAML front-matter parse + lint (typo warnings)
  src/math.rs      KaTeX server-side render (bundled CSS/fonts, offline)
  src/highlight.rs server-side syntax highlighting (syntect → `tali-hl-` scope classes)
  src/cite/        citations ([@key]) + cross-references (@fig-, @sec-): a module dir
                   (parse/render/format/validate/author/clean)
  src/site/        multi-page project (mod.rs): _site.yml config (config/), page
                   discovery, chrome, link rewrite, listings + `hero:` blocks,
                   front-matter parse (frontmatter.rs), books (book.rs),
                   Atom feeds per dated listing (feed.rs),
                   Cmd-K search (search.rs), the skim layer-cake projection (skim.rs,
                   shares search's render recipe, not its text extraction),
                   cross-refs (xref.rs); an {{< embed >}}-
                   referenced deck is built/served but kept out of nav. `mounts:`
                   serves another project (e.g. the docs book) under a URL prefix in preview
  assets/          bundled offline: css/ (base, dark, deck, site),
                   js/ (deck.js, code-enhance/ fragments, mermaid.js, tali-js.js,
                   scrolly.js, tabset.js, walkthrough.js + vendored
                   plot.umd.min.js/d3.min.js for `{js}` cells), katex/
crates/server    taliesin-server, bin `taliesin`: CLI + websocket dev server
  src/main.rs      render / blocks / build / serve subcommands (a dir = a site project)
  src/cli.rs       CLI arg parsing + subcommand dispatch
  src/serve/       single-doc axum websocket + notify file watcher (mod.rs, security.rs)
  src/serve_site/  multi-page site server (mod.rs: per-page state/executor, cross-page
                   nav, hot reload; exec_pool.rs: the MAX_WARM_PAGES LRU, the one freeze)
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
docs/            project's own manual: TWO sibling book projects, authored in .tmd
                 (dogfooding). docs/guide/ = User Guide (using/ + reference/ + demo/tour
                 decks); docs/internals/ = Internals book. docs/ itself is just a container
                 (no _site.yml). The site mounts each at /docs/guide + /docs/internals.
corpus/          the real .tmd docs (the spec); cargo test renders them all
```

## Read before working

- **docs/** is the project's own manual, authored in `.tmd` as TWO sibling book
  projects (dogfooding):
  - **`docs/guide/`** = the User Guide (`using/` feature showcase + `reference/`):
    how to *use* Taliesin. Preview it: `taliesin preview docs/guide`.
  - **`docs/internals/`** = the Internals book: the architecture, the rendering
    pipeline, the deck engine, the block model, the execution model, the dev server,
    and how to extend it. Preview it: `taliesin preview docs/internals`.
  - `docs/` itself is just a container (no `_site.yml`); the books are siblings
    because the page-walker would otherwise swallow a nested book's pages. The
    marketing site mounts them at `/docs/guide` + `/docs/internals`. Cross-book links
    are written as relative `.html` (e.g. `../guide/using/formats.html`).
- **corpus/README.md** for what the test documents exercise.

## Commands

```sh
cargo run -p taliesin-server -- preview <file.tmd> [port]      # live preview (aliases: dev, serve)
cargo run -p taliesin-server -- preview <file.tmd> --host      # + expose on LAN with a phone QR code
cargo run -p taliesin-server -- preview <dir>                  # live multi-page SITE preview (nav + per-page hot reload)
cargo run -p taliesin-server -- build  <file.tmd> [out.html]   # self-contained HTML file (default <name>.html)
cargo run -p taliesin-server -- build  <file.tmd> --out <dir>  # portable folder: <dir>/index.html + copied local assets
cargo run -p taliesin-server -- build  <dir> [--out <dir>]     # multi-page SITE -> _site/ (one .html per page + assets)
cargo run -p taliesin-server -- render <file.tmd> > out.html   # one-shot full page to stdout
cargo run -p taliesin-server -- blocks <file.tmd>              # block ids + sourcepos (debug)
cargo run -p taliesin-server -- skim   <dir>                   # the book's skimmable layers as one linear stream
cargo test -p taliesin-core                                    # corpus invariants + unit tests
cd web-client && npx -y -p typescript tsc -p jsconfig.json     # type-check the client JS (client.js + search/toc-spy/toc-sheet; // @ts-check, no build step)
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json  # type-check the bundled assets JS (code-enhance/ fragments + deck.js/tali-js.js/mermaid/scrolly/tabset/walkthrough, strict; globals.d.ts + web-client's are merged; run it by hand, nothing gates it)
```

A `taliesin` launcher on `PATH` (`~/.local/bin/taliesin`) rebuilds the release
binary when the tool's sources change, then runs it, so `taliesin preview <file>`
works from anywhere.

**Editing `assets/css/*` or `assets/js/*` needs a `cargo build` before the change
shows up.** They are `include_str!`-compiled into the binary, so rebuilding only the
site (`taliesin build <dir>`) re-emits the *old* bundled CSS/JS and you will measure a
stale page. Rebuild the binary first, then the site. (A live `preview` hot-swaps CSS,
so this bites the build-and-inspect loop, not the dev loop.)

Executing code cells needs a matching Jupyter kernel: `{python}` cells need a
Python with `ipykernel` (`TALIESIN_PYTHON`, default `python3`); `{r}` cells need an
R with `IRkernel` (`TALIESIN_R`, default `R`). Each language runs against its own
warm kernel. Without a kernel, cells render as source and the preview shows a
"kernel unavailable" diagnostic. A cell that runs longer than `TALIESIN_CELL_TIMEOUT`
seconds (default 120; `0` disables) is interrupted (SIGINT) so a runaway cell can't
wedge the kernel; the warm kernel and prior cells survive.

Cell outputs persist in `_freeze/` (gitignored), keyed by a cumulative content hash
(this cell's code + all upstream same-language code + interpreter id) — so a change
to a cell or anything upstream busts it and everything downstream, with no stale
hits and nothing to clear by hand. An unchanged doc replays from disk on the next
`build`/preview without booting the kernel; a warm preview still re-runs only the
edited cell + downstream. Errors and `#| cache: false` cells are never persisted.
`TALIESIN_NO_CACHE` ignores + skips writing the cache; "Restart kernel" forces a
fresh re-run. (Kernel *variable* state is never cached, which is what makes a naive
per-cell `cache` fragile, so a cold start can only skip work when the whole
document is unchanged.) See `crates/server/src/freeze.rs`.

For UI work, `/preview <file.tmd>` builds, serves on port 4388, and verifies it in
the browser via the chrome-devtools MCP (screenshot + console). A `PostToolUse`
hook runs `rustfmt` on every edited `.rs` file, so the tree stays `cargo fmt`-clean.

**There is no CI. The pre-push hook is the only automatic gate.** The GitHub Actions
workflow was deleted on 2026-07-26 (it was billing Actions minutes on this private
repo). What survives is `.githooks/pre-push` (wired via `core.hooksPath`, so it is
invisible in `.git/hooks`): a push that includes `main` runs `cargo fmt --all --
--check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test
--workspace` first, and a WIP-branch push skips it. `git push --no-verify` bypasses.

The workflow's *other* jobs now run nowhere, so they are yours to run by hand and
they are the ones most likely to rot unnoticed: the live-kernel suites (Python and R,
via `TALIESIN_REQUIRE_KERNEL` / `TALIESIN_REQUIRE_R`, which silently *skip* when the
interpreter is absent, which is exactly how they reach zero coverage), the two `tsc`
type-checks above, `node --test crates/server/src/assets/_middleware.test.mjs` (the
publish passcode gate), the VS Code companion's offline TextMate grammar test, and
`cargo audit` / `cargo deny check` (`deny.toml` is still the policy) on any
dependency change. Never call one of these verified without its output.

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
