# Taliesin

A single-purpose Rust dev server that renders `.tmd` files to **HTML only** (blog
posts, papers, books, multi-page sites) for one author's workflow, built around
three load-bearing goals: click-to-source, block-level incremental updates, and no
per-edit startup cost (warm server + Jupyter kernel). It is **not** a general document
compiler: HTML is the only output target (no LaTeX/Typst/Word/ePub; a future print/PDF
track would render *from* the built HTML, never as a parallel format).

> **Standing directive from the author:** *"always lean towards cutting. I'd rather have
> a polished lean product, and then add features when I have real users that need them
> than having a bloated product with features that nobody uses."* When a call is close,
> cut. Do not add features, do not "restore parity", and do not defend a feature on the
> grounds that a corpus document pins it: that pinning is circular.
>
> `notes/DO-NOT-REBUILD.md` is the anti-rot register: read it before re-filing anything
> that looks obvious. `notes/ROADMAP.md` is the author's to prioritize.

**"Done" means the documents under `corpus/` render correctly**: the corpus is the
regression net. But **the corpus records; it does not lead**: a document earns its place
by being something a person wanted to read, or a golden no unit test can hold. A feature
witness belongs in `crates/core/src/render/tests.rs`. **"Wider" means richer browser
behavior in a live HTML view**, not new output formats, and never at the cost of the
invariants below.

> **⚠ ORDERING RULE.** *A pin and its docs page are deleted in the SAME commit as their
> feature, never before.* A corpus document deleted ahead of the code it guards leaves
> that code unguarded: the sweeps in `crates/core/tests/corpus.rs` iterate over whatever
> exists. `crates/core/tests/corpus_manifest.rs` lists every corpus document, so a
> deletion costs one deliberate line, but nothing checks that it lands with its feature.

**The one standing freeze is warm-page eviction**: `MAX_WARM_PAGES` plus the
deterministic LRU order in `serve_site/exec_pool.rs`, which the **preview** relies on
(`ExecPool` exists nowhere else; `build.rs` gives each page a fresh executor). Its own
tests pin the cap and the order. The "Do NOT touch" list in `notes/native-rewrite.md` is a
completed decision, **not** a freeze.

**The `.tmd` file is the single editing surface; the browser is a read-only view.**
Click-to-source is the only bridge back, and it *navigates* (preview → editor cursor), it
never *writes*: a second write path (a drag-to-reorder, removed) fights click-to-source
over who owns the file and invites WYSIWYG scope creep. Make a source edit ergonomic with
an editor command, not a preview gesture.

## Where things are

```
crates/core      taliesin-core lib: parser (comrak + sourcepos) → block model → render
  src/render/      block model + emission (a module dir; its own CLAUDE.md maps it):
    mod.rs           the render pipeline (parse → block model → HTML) + head/asset helpers
    model.rs         the block-model data types (Cell, Block, RenderedDoc, SiteDefaults)
    tests.rs         render unit + corpus-invariant tests
    emit.rs          per-block HTML (server-side highlighting, the `code-fold` <details>)
    divs.rs          `:::` fenced divs (callouts, the `layout-ncol` grid, width escapes)
    figure.rs        numbered figures + captions
    extension/       shortcode expansion: `{{< input >}}` only (`{{< include >}}` is
                     resolved a pass earlier). No format-extension mechanism, no `format:`
    theme.rs         `theme_head`, the pre-paint script, and NOTHING ELSE. **Which
                     palette paints is the reader's DEVICE**: `theme_head()` takes NO
                     argument, both palettes always ship, and there is **no author theme
                     control**. The `tali-theme` storage key is only the preview dev menu's
                     reader toggle; `tali:themechange` fires when the OS scheme flips
    page.rs          full HTML-page assembly (PAGE_TEMPLATE shell, site-chrome wiring,
                     favicon): `render_doc_to_page`, the one RenderedDoc → page every
                     build writes (the preview calls `assemble_html_page` itself)
  src/diff.rs      block-level diff (BlockOp) for incremental updates
  src/lines.rs     what each source line is, as comrak parses it (markdown, code, raw HTML,
                   front matter; quote/list depth): the ONE answer for every pass that reads
                   source line by line (includes, shortcodes, the `:::` scan, the prose
                   count, the anchor scan). Never hand-roll a fence tracker
  src/includes.rs  {{< include >}} resolution + per-file source map; `read_source` (the one
                   normalizing reader) and `publishable`, the one publication and
                   containment rule the copiers, the asset lint and the preview share
  src/reads.rs     the files a render reads or looks for, recorded as it runs: what the
                   preview rebuilds a page on
  src/frontmatter.rs the one front-matter splitter (`front_matter_block`,
                   `blank_front_matter`) + YAML parse + lint (typo warnings); the renderer
                   reads the block once as YAML (`render::DocFront`)
  src/math.rs      KaTeX server-side render (bundled, offline), memoized on
                   `(latex, display)`, on ONE long-lived worker thread reached by channel:
                   the `katex` crate keeps its JS context thread-local, and a context per
                   render thread paid its boot per page. Pinned by
                   `katex_runs_on_exactly_one_thread_…`
  src/highlight.rs server-side syntax highlighting (syntect → `tali-hl-` scope classes),
                   memoized on `(code, alias-resolved lang)` under a BYTE budget. The memo
                   is consulted BEFORE `resolve`, which is load-bearing: resolving a token
                   the bundled set lacks deserializes the slow `two-face` extras
  src/diagnostics/ the static validators `lint::page_static_diagnostics` runs: anchors,
                   assets (images only), links, the `{js}` reactive graph, a11y (alt text +
                   heading skips) and bibliography. **The keep test is "a defect the author
                   cannot see in the rendered page"**
  src/cite/        citations ([@key]) + cross-references (@fig-, @sec-): a module dir
  src/site/        multi-page projects and books (its own CLAUDE.md maps it).
                   `Site::discover` is the project, `discover_document` a named document's
                   project scoped to it, `discover_registry` (the LSP's) renders nothing.
                   The two whole-project render passes (`harvest_xref_numbers`, numbers and
                   heading text only, and `search::build_sections`) fan out via
                   `fanout::map_ordered`: results in PAGE order (duplicate labels are
                   "first definition wins", so completion order would make a build depend
                   on scheduling), and a panic propagates (`refresh_xrefs`'s all-or-nothing
                   `catch_unwind` needs it). ONE project per build and per deploy: the four
                   sites publish separately and link by absolute URL (tools/publish.sh)
  assets/          bundled offline: css/, js/ (code-enhance/ fragments, mermaid.js,
                   tali-js.js + vendored plot/d3 for `{js}` cells), katex/
crates/server    taliesin-server, bin `taliesin`: CLI + websocket dev server
  src/main.rs      the COMMANDS table (one row per verb) + the dispatch and both help
                   surfaces derived from it
  src/cli.rs       `init` (the scaffold) + `preview` arg parsing
  src/serve/       the dev server's SHARED layer, not a server: HTTP/asset plumbing, port
                   binding + the single-instance probe, security.rs's three guards, the
                   watch predicates, the CLI error helpers (`guarded`,
                   `unknown_flag_error`, `bad_format_error`). **The preview binds
                   127.0.0.1 and nothing else**, so the guards are about a local peer:
                   `ws_origin_ok` is the only thing stopping an open tab sending
                   `restart_kernel`; `with_host_guard` is the unconditional DNS-rebinding
                   allowlist and, from Fetch Metadata, refuses a cross-site load that is
                   not a navigation. `restart_kernel` is the ONLY write the server accepts
                   from a client
  src/serve_site/  THE dev server, one per project (exec_pool.rs: the MAX_WARM_PAGES LRU,
                   the one freeze). `preview <file.tmd>` opens the enclosing `_site.yml`
                   project at that page; with none, a project of just that document, no
                   navbar or footer, so it agrees with `build <file>` on chrome and TOC.
                   **Inside a project the verbs part company by design**: `build p3.tmd`
                   writes one self-contained file, where a navbar to siblings it never
                   wrote would be broken chrome. Both refuse a directory with no `_site.yml`
  src/exec.rs      runs a doc's code cells, splices outputs back as blocks; plans what
                   re-runs via cumulative-hash keys (warm reuse + cold replay). A cell's
                   failure travels as data (`exec::Failure`), never read back out of HTML
  src/freeze.rs    persistent execution cache (`_freeze/<page>.json`): rendered outputs
                   keyed by a cumulative content hash. Also records the `packages:` digest
                   (`packages.rs`), the one axis the key cannot see, so a replay that
                   crossed a `pip install --upgrade` says so; it does NOT change what hits
  src/packages.rs  an interpreter's installed `name==version` set + one digest for it, one
                   memoized probe per interpreter; read by `freeze.rs` and `doctor`
  src/kernel.rs    warm Jupyter kernel (ZMQ), reused across edits
  src/log.rs       colorized dev-server console output (to stderr)
  src/lint.rs      the SHARED static-lint kernel, not a verb: `PagePass`, THE one page
                   pass every verb wraps (`build`, `--check-only`, the preview, the LSP:
                   render, front matter, static checks, cells when the verb runs them,
                   the project's finish); `Diagnostic`, the one diagnostic type (the
                   preview's wire carries it too); `blocking` (what fails `--strict`);
                   `page_static_diagnostics`; `buffer_diagnostics_in_site` (the LSP's
                   seam); `cmd_check_only`. **Severity is a field on `render::Warning`**,
                   set by the validator that found the defect; there is no code catalogue
  src/lsp*.rs      `taliesin lsp`, the offline, kernel-free LSP server: ALL editor
                   intelligence, as **SIX read-only providers** (completion, hover,
                   definition, documentSymbol, codeAction, foldingRange), pushed
                   `publishDiagnostics` and two extensions (`taliesin/cellRegions`,
                   `siteMap`). **Nothing here writes to a buffer except a code action the
                   author invokes**. `the_initialize_handshake_advertises_…` fails if a cut
                   provider is advertised again. Four hazards: stdout is the JSON-RPC wire
                   (log to stderr via `crate::log`); **split a buffer with
                   `lsp_pos::lines`, never `split('\n')` or `str::lines`** (a lone `\r`
                   ends a line too); `didChange` is COALESCED (120 ms) because a publish
                   re-walks every page, and it runs in `main_loop`'s timeout arm, so **that
                   arm needs its own `guarded`**; `$/cancelRequest` is batch-scoped, so
                   `read_batch` must not read past `shutdown` (`handle_shutdown` owns `exit`)
editor/vscode/   the VS Code companion: NO language features (`src/client.ts` is a
                 language client over `taliesin lsp`), only what LSP has no concept of:
                 the preview webview + source sync, editor commands, and `src/embedded.ts`
                 (completion, hover, signature help and go-to-definition inside a cell,
                 routed to that language's own provider; cells located by
                 `taliesin/cellRegions`, never a fence scan in TS; the vscode-free
                 `src/projection.ts` builds the hidden shadow, each `{js}` cell wrapped in
                 the function the server's `wrap` names, and `src/shadowlinks.ts` moves a
                 definition out of it back onto the `.tmd`).
                 **Add an editor feature in Rust, not here.**
web-client/      browser scripts: client.js is the preview client (never ships in a
                 build); search.js (Cmd-K) and toc-spy.js ship in built pages too
docs/            the manual, TWO sibling book projects in .tmd: docs/guide/ (User Guide)
                 and docs/internals/ (no _site.yml in docs/ itself)
gallery/         one-page demos: its own flat, self-contained project + domain
corpus/          the real .tmd docs (the spec); cargo test renders them all
```

The two books are siblings because the page walker would otherwise swallow a nested
book's pages, and **each is its own deploy on its own domain**, so cross-book links are
ABSOLUTE URLs (`https://guide.taliesin.sh/using/choosing.html`);
`crates/core/tests/cross_site_links.rs` resolves each against the source tree.
`corpus/README.md` says what each test document exercises.

## Commands

```sh
cargo run -p taliesin-server -- preview <file.tmd> [port]      # live preview
cargo run -p taliesin-server -- preview <dir>                  # live multi-page SITE preview (nav + per-page hot reload)
cargo run -p taliesin-server -- build  <file.tmd> [out.html]   # self-contained HTML file (default <name>.html)
cargo run -p taliesin-server -- build  <file.tmd> --out <dir>  # portable folder: <dir>/index.html + copied local assets
cargo run -p taliesin-server -- build  <dir> [--out <dir>]     # multi-page SITE -> _site/ (one .html per page + assets)
cargo run -p taliesin-server -- build  <file.tmd> --stdout     # the page to stdout (+ --no-exec for a static dump)
cargo run -p taliesin-server -- build  <dir> --check-only      # THE PRE-PUBLISH GATE: lint, write nothing, exit non-zero
                                                               #   (+ --strict to fail on advice, + --format json for one machine surface)
./tools/publish.sh [--check] [site|guide|internals|gallery]    # build + deploy the four sites (--check = build all, deploy none)
cargo test -p taliesin-core                                    # corpus invariants + unit tests
cd web-client && npx -y -p typescript tsc -p jsconfig.json     # type-check the client JS (// @ts-check, no build step)
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json  # type-check the bundled assets JS (strict)
```

The CLI is **six subcommands**: `preview`, `build`, `init`, `doctor`, `lsp`, `help`.
A `taliesin` launcher on `PATH` (`~/.local/bin/taliesin`) rebuilds the release binary
when the tool's sources change, then runs it. For UI work, `/preview <file.tmd>` builds,
serves on port 4388, and verifies it in the browser via the chrome-devtools MCP. A
`PostToolUse` hook runs `rustfmt` on every edited `.rs` file.

**Editing `assets/css/*`, `assets/js/*` or `web-client/*` needs a `cargo build` before the
change shows up.** They are `include_str!`-compiled, so rebuilding only the site re-emits
the *old* bundle and you measure a stale page. A rebuilt server has a new boot id, which
`client.js` answers with `location.reload()` (a re-mount would keep the old client
running), so the reload fetches the fresh bundle.

## Executing cells

`{python}` cells need a Python with `ipykernel`. The interpreter is the first of: the
`_site.yml` `python:` field, the project's own `.venv`, `TALIESIN_PYTHON`, a `.venv` found
walking up from the project (the walk stops at a `.git` or `pyproject.toml` directory,
after probing it), then `python3` (`interpreter.rs`; `doctor` names the one it picked).
Python is the one kernel language, so the executor holds one kernel state.
`FreezeCache::packages` **stays a map**: it is keyed by interpreter identity and is
on-disk format (`_freeze/*.json`), so a scalar would fail to parse every existing cache.
Without a kernel, cells render as source and the preview shows a "kernel unavailable"
diagnostic.

A cell is capped on **silence, not runtime**: one that produces no output for
`TALIESIN_CELL_SILENCE` seconds (default 600; `0` disables) is interrupted (SIGINT), while
a cell that prints progress resets that budget on every line. `TALIESIN_CELL_TIMEOUT` is
an optional wall-clock cap, **off by default**. Either way the warm kernel and prior cells
survive.

Cell outputs persist in `_freeze/` (gitignored), keyed by a cumulative content hash (this
cell's code + all upstream code + interpreter id), so a change busts that cell and
everything downstream, with nothing to clear by hand. Never persisted: errors, `#| cache:
false` cells **and everything downstream of one** (the key would claim an output follows
from upstream code when it does not; `first_uncacheable` in `exec.rs` is the one
definition that rule and `plan`'s re-run range share), and anything a WARM kernel re-ran:
a run persists only when its kernel executed exactly the reused prefix
(`LangState::executed == shared`), because a kernel that ran an earlier version of a cell
still holds the names it defined, so the preview persists cold runs only.
`TALIESIN_NO_CACHE` skips the cache; "Restart kernel" forces a fresh re-run. Kernel
*variable* state is never cached, so a cold start skips work only when the whole document
is unchanged.

## Gates

**`./tools/gates.sh` runs every gate in one process and refuses to be green unless every
one of them actually ran**: several gates *skip silently* when their interpreter is
absent, so a plain `cargo test` can be green and mean almost nothing. It arms
`TALIESIN_REQUIRE_KERNEL` and `TALIESIN_REQUIRE_NODE`, asserts by name that each
interpreter's canary printed `... ok`, treats one ignored test as a failure, and needs
`TALIESIN_PYTHON="$PWD/.venv/bin/python"` or exits 2 at preflight. It covers the
live-kernel suite, the Node-backed reactive test, both `tsc` type-checks, the companion's
tests, `cargo audit`/`cargo deny check`, the two `build docs/<book> --check-only` document
gates, `tools/publish.sh --check` and the portability census, so reach for it instead of
running the pieces by hand. **Take the gate count from its own verdict line, never from
prose, and never call a gate verified without its output.**

**CI and the pre-push hook run on their own.** `.github/workflows/ci.yml` runs on every
push to `main`, every pull request and weekly, visibly to strangers; `stale_docs.rs` fails
if a job is made conditional on visibility.
`.githooks/pre-push` runs fmt, clippy, the workspace tests, both document gates and
`tools/publish.sh --check` before a push that includes `main`. It is wired via
`core.hooksPath`: invisible in `.git/hooks`, **unset in a fresh clone**, skipped by a
WIP-branch push and bypassed by `--no-verify`. `gate_script.rs` cross-checks the hook,
`gates.sh` and `ci.yml` (`every_pre_push_command_is_also_run_by_the_gate_script`,
`every_docs_book_is_linted_by_every_gate_file`), so a third book cannot inherit a hole.

## Conventions

- Rust edition 2024, workspace resolver 3. Shared deps go in the root
  `[workspace.dependencies]`.
- **Never publish a number about this tool that has no committed instrument.** A number
  without one carries its measured-on date and is re-measured before a release tag. The
  portability census is gated (`tools/portability-census.py --verify`) because the page
  hands the reader the command, so a mismatch refutes itself. Wall clocks, binary size and
  crate count are NOT gated: they measure the machine, so they carry a date.
- Every emitted block carries `data-block-id` (content-hash) + `data-sourcepos`; included
  blocks also carry `data-source-file`. Source mapping, incremental re-render and
  live-state preservation all key off this one block model (`crates/core/tests/corpus.rs`
  enforces it).
- **Two line coordinate systems, kept apart by the compiler.** A post-include BUFFER line
  is a `BufLine` (`render/model.rs`): no `Display`, no conversion, so it cannot reach a
  `data-sourcepos` or `Warning::at`. `map_origin`/`map_span` are the way out, to the
  author's own file and line. **A `source_file` may only ever be paired with a mapped
  line** (any include shifts every later buffer line, putting a diagnostic N lines off in a
  real file), and a block's `data-sourcepos` range must stay inside ONE file, which
  `map_span` guarantees for both ends (`client.js` skips a range like `39:1-6:25`).
  `render/CLAUDE.md` has the detail.
- **Read finished HTML through `render::tags` / `render::attrs`, never a substring scan
  like `find("src=\"")`.** Page TEXT can show a real-looking attribute (a code sample of
  `<a href=x.md>`), and the inlined mermaid/Plot bundles build HTML from string fragments
  (`<img src="${e}"`); a substring scan reads all of it as markup, and has been the same
  bug four times. The walker knows tag-versus-text, skips `<script>`/`<style>` bodies and
  comments, reads all three value forms, and hands back `Attr::value` decoded
  (`R&amp;D.png` reads `R&D.png`).
- **A duplicate element id is RENAMED, never refused** (`dedup_element_ids`, the last
  id-assigning pass). The first definition keeps the author's spelling, so every link and
  `@ref` still resolves; the duplicate draws an error-severity located diagnostic.
  Refusing would invent a hard-fail path no other error has, while the preview rendered
  the page anyway.
- **`vocab.rs` is the OFFERED-completions subset, not the implemented set**: answer "what
  does the tool support" from the validator consts, never from `vocab`. It agrees with
  the validators for div classes (`DIV_CLASS_NAMES` and `render::DIV_FEATURE_CLASSES` are
  the same **2** width escapes; a test pins the subset) and for cross-references
  (`vocab::xref_prefixes()` is all **5** `XREF_LABELS`). **An unknown xref prefix is
  silent, always, and deliberately**: `parse_xref` cannot tell `@rust-lang` in prose from
  a misspelled reference, so a diagnostic would false-fire on writing (and did-you-mean
  misfires: `thm` is edit distance 2 from `tbl`).
- **Taliesin answers for its own vocabulary and nothing else.** An unknown key, cell
  option, callout kind, shortcode, div class, flag or verb gets a did-you-mean inside edit
  distance 2, a bare "unknown", or (for div classes, an open vocabulary) silence and the
  author's own CSS. **Do not reintroduce a retirement register, a compatibility note, or a
  "did you mean <another tool's key>" answer.** Withdrawing a construct means deleting the
  *read* as well as the vocabulary entry: a key dropped from `KNOWN_KEYS`/`HERO_KEYS`/… is
  only *diagnosed*, and one the parser still honours goes on working. A parser-side pin is
  the only thing that says the read is gone.
- **A new front-matter key trips THREE drift gates**, all inside `taliesin-core`:
  `KNOWN_KEYS`, `the_reference_page_documents_every_known_key` (→
  `docs/guide/reference/frontmatter.tmd`), and `vocab.rs` + its `descriptions_present`.
  A `_site.yml` key also reaches `crates/core/assets/schema/tali-site.schema.json`, which
  is GENERATED from `site::NATIVE_KEYS` by the bless path in `schema.rs`
  (`TALIESIN_BLESS=1 cargo test -p taliesin-core --lib schema`) and golden-locked by
  `site_schema_matches_committed`.
- **A new subcommand is ONE row in `main.rs`'s `COMMANDS` table** (dispatch, the
  did-you-mean and both help surfaces derive from it), plus a row in the table in
  `docs/guide/reference/cli.tmd`, which `every_subcommand_has_a_row_in_the_cli_reference`
  checks both ways. Every other prose claim here is ungated: grep, do not trust.
- Minimal config: perfect the default before adding a knob, so the user does not *need* to
  configure; this is the deciding lens for any new user-facing control. **A reader-local
  preference is an argument for honouring the answer they already gave their OS** (the
  browser's own zoom outranks a comfort panel), not for asking again per site. Ship no
  reader-facing control without new evidence that the device answer is wrong for someone.
- **Copy states facts; it does not perform.** On the four sites and in scaffolded text: no
  "No X, no Y, no Z" closers, no "one idea" thesis reveals, no count announced for its own
  sake in a heading ("Three things it gets right"; "Two line coordinate systems" names a
  fact and is fine), no "→" appended to link text. A heading or a link names its subject or
  its destination. Ungated by ruling (prose linting was ruled out), so it holds by review.
