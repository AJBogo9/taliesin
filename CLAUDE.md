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
> grounds that a corpus document pins it — that pinning is circular.
>
> The 2026-08 scope reduction (18 CLI verbs to 6, ~40% of the tree) is **complete**;
> `notes/CUT-PROGRESS.md` is its durable record and `notes/DO-NOT-REBUILD.md` is the
> anti-rot register — read the latter before re-filing anything that looks obvious.
> `notes/ROADMAP.md` is unpaused and is the author's to prioritize.

**"Done" means the documents under `corpus/` render correctly**: the corpus is the
regression net. But **the corpus records; it does not lead** — a document earns its place
by being something a person wanted to read, or by being a golden no unit test can hold. A
feature witness belongs in `crates/core/src/render/tests.rs`. **"Wider" means richer
browser behavior in a live HTML view**, not new output formats, and never at the cost of
the invariants below.

> **⚠ ORDERING RULE.** *A pin and its docs page are deleted in the SAME commit as their
> feature, never before.* A corpus document deleted ahead of the code it guards leaves
> that code silently unguarded while every gate still passes — the sweeps in
> `crates/core/tests/corpus.rs` iterate over whatever exists, so removing a document
> removes coverage without removing a test.

**The one standing freeze is warm-page eviction**: `MAX_WARM_PAGES` plus the
deterministic LRU order in `serve_site/exec_pool.rs`, which the **preview** relies on
(`ExecPool` exists nowhere else; `build.rs` gives each page a fresh executor). It is a
scoping decision, not an untested invariant — its own tests pin the cap and the order.
The 7-item "Do NOT touch" list in `notes/native-rewrite.md` is a *completed
rewrite-scoping* decision, **not** a freeze.

**The `.tmd` file is the single editing surface; the browser is a read-only view.** You
change the source in your editor, the preview re-renders. Click-to-source is the only
bridge back, and it *navigates* (preview → editor cursor), it never *writes*. A
drag-to-reorder feature broke this and was removed: a second write path fights
click-to-source over who owns the file, and the line "you may reorder but not
edit/delete" invites WYSIWYG scope creep. Make a source edit ergonomic with an editor
command, not a preview gesture.

## Where things are

```
crates/core      taliesin-core lib: parser (comrak + sourcepos) → block model → render
  src/render/      block model + emission (a module dir):
    mod.rs           the render pipeline (parse → block model → HTML) + head/asset helpers
    model.rs         the block-model data types (Cell, Block, RenderedDoc, PageIncludes)
    tests.rs         render unit + corpus-invariant tests
    emit.rs          per-block HTML (server-side highlighting, code line-wrapping)
    divs.rs          `:::` fenced divs (callouts, the `layout-ncol` grid, width escapes)
    figure.rs        numbered figures + captions
    extension/       shortcode expansion: `{{< input >}}` (the only one that expands
                     here; `{{< include >}}` is resolved a pass earlier). NOT
                     `_extensions/`, which is a theme-CSS lookup in `theme.rs` and
                     nothing else: there is no format-extension mechanism and no
                     `format:` key — HTML is the only output
    theme.rs         `--tali-*` CSS-variable themes (light/dark, extension themes) +
                     `theme_head`, the pre-paint script. **Which palette paints is the
                     reader's DEVICE and nothing else**: `theme_head()` takes NO
                     argument, which makes that structural rather than a promise each
                     call site keeps. Both palettes always ship; `html[data-theme]`
                     selects one at paint, and `tali-theme`/`tali:themechange` survive
                     only for the preview dev menu's toggle, never in a build
    page.rs          full HTML-page assembly (PAGE_TEMPLATE shell, site-chrome wiring,
                     favicon): RenderedDoc → standalone page for build + in-process render
  src/diff.rs      block-level diff (BlockOp) for incremental updates
  src/includes.rs  {{< include >}} resolution + per-file source map
  src/frontmatter.rs YAML front-matter parse + lint (typo warnings)
  src/math.rs      KaTeX server-side render (bundled CSS/fonts, offline)
  src/highlight.rs server-side syntax highlighting (syntect → `tali-hl-` scope classes)
  src/diagnostics/ the static validators `lint::page_static_diagnostics` runs: headings,
                   anchors, assets, media, links, the `{js}` reactive graph, a11y (alt
                   text + heading skips) and bibliography. **The keep test is "a defect
                   the author cannot see in the rendered page"**
  src/cite/        citations ([@key]) + cross-references (@fig-, @sec-): a module dir
                   (parse/render/format/validate/author/clean)
  src/site/        multi-page project (mod.rs): _site.yml config (config/), page
                   discovery, chrome, link rewrite, listings + `hero:` blocks,
                   front-matter parse (frontmatter.rs), books (book.rs), Atom feeds per
                   dated listing (feed.rs, which also owns `nav_ordered`), sitemap.xml +
                   robots.txt (seo.rs), the five-tag OpenGraph head (meta.rs), Cmd-K
                   search (search.rs), cross-refs (xref.rs). ONE project per build and
                   ONE PROJECT PER DEPLOY: the four sites publish separately and link by
                   absolute URL (tools/publish.sh). Only gallery/ nests others under its
                   output, parent first (the parent's sweep deletes output it did not
                   itself write)
  assets/          bundled offline: css/ (base, dark, site), js/ (code-enhance/
                   fragments, mermaid.js, tali-js.js + vendored
                   plot.umd.min.js/d3.min.js for `{js}` cells), katex/
crates/server    taliesin-server, bin `taliesin`: CLI + websocket dev server
  src/main.rs      subcommand dispatch + COMMANDS + the help surfaces
  src/cli.rs       `init` (the scaffold) + `preview` arg parsing
  src/serve/       the dev server's SHARED layer, not a server: HTTP/asset plumbing,
                   port binding + the single-instance probe, security.rs's origin/Host/
                   identity guards, the watch predicates, and the CLI error helpers
                   (`guarded`, `unknown_flag_error`, `bad_format_error`). **The preview
                   binds 127.0.0.1 and nothing else**, so both guards are about a local
                   peer: `ws_origin_ok` is the only thing stopping an open tab sending
                   `restart_kernel`, and `with_host_guard` is the unconditional
                   DNS-rebinding allowlist. `restart_kernel` over the websocket is the
                   ONLY write the server accepts from a client
  src/serve_site/  THE dev server — one server for a project and for a single document
                   alike (mod.rs: per-page state/executor, cross-page nav, hot reload;
                   exec_pool.rs: the MAX_WARM_PAGES LRU, the one freeze). ONE project per
                   server. Building and deploying the four sites is `tools/publish.sh`,
                   whose `--check` runs in `.githooks/pre-push`.
                   `preview <file.tmd>` resolves to the file's enclosing `_site.yml`
                   project, opened at that page; with no ancestor it is a project of just
                   that document (`Site::discover_single`), rendering no navbar and no
                   footer, so `preview <file>` and `build <file>` agree on page chrome —
                   **including the TOC**, which `build.rs` asks `Site::page_toc` for when
                   (and only when) the front matter left `toc:` out. **Inside a project
                   the two verbs part company by design**: `preview p3.tmd` opens the
                   whole project at that page, `build p3.tmd` writes one self-contained
                   file, and a navbar linking to `.html` siblings the build never wrote
                   would be broken chrome. A directory with no `_site.yml` is refused by
                   both verbs
  src/exec.rs      runs a doc's code cells, splices outputs back as blocks; plans what
                   re-runs via cumulative-hash keys (warm reuse + cold replay)
  src/freeze.rs    persistent execution cache (`_freeze/<page>.json`): rendered cell
                   outputs keyed by a cumulative content hash, so unchanged cells restore
                   instead of re-executing across builds + preview restarts. Also records
                   the `packages:` digest each output was produced under (`packages.rs`)
                   — the one axis the key cannot see — so a replay that crossed a
                   `pip install --upgrade` says so; it does NOT change what hits
  src/packages.rs  what an interpreter actually has installed (`name==version`) + one
                   digest for the set. Read by `doctor --format json` and by `freeze.rs`;
                   one memoized subprocess per interpreter per process
  src/kernel.rs    warm Jupyter kernel (ZMQ), reused across edits
  src/log.rs       colorized dev-server console output (to stderr)
  src/lint.rs      the SHARED static-lint kernel, not a verb: `Diagnostic`, `diag_from`,
                   `blocking` (what fails `--strict`), `page_static_diagnostics` (the
                   check-superset, ONE definition for `build`, `build --check-only`, the
                   preview and the LSP), `buffer_diagnostics_in_site` (the LSP's seam)
                   and `cmd_check_only` (the ~40-line front door). **Severity is a field
                   on `render::Warning`**, set by the validator that found the defect:
                   there is no `TAL-*` code catalogue, so a reworded message can no longer
                   silently reclassify a family
  src/lsp*.rs      `taliesin lsp`: the offline, kernel-free LSP server (lsp.rs dispatch +
                   capabilities; lsp_complete/lsp_nav/lsp_outline/lsp_pos/lsp_fold/
                   lsp_cells/lsp_diag/lsp_project). ALL editor intelligence lives here,
                   as **SIX read-only providers**: completion, hover, definition,
                   documentSymbol, codeAction, foldingRange, plus pushed
                   `publishDiagnostics` and two namespaced extensions
                   (`taliesin/cellRegions`, `siteMap`).
                   **Nothing here writes to a buffer except a code action the author
                   explicitly invokes** ("Change to `X`" returns a `WorkspaceEdit`, the
                   standard LSP contract, pinned by its own test); the distinction is
                   user-invoked versus server-initiated. The nine providers cut on
                   2026-08-08 are named in `the_initialize_handshake_advertises_…`, which
                   fails if one is advertised again — five of them were the only paths
                   besides the author's keystrokes that rewrote a `.tmd`.
                   Four hazards: stdout is the JSON-RPC wire, so never print to it (use
                   `crate::log`, stderr); **split a buffer with `lsp_pos::lines`, never
                   `split('\n')` or `str::lines`**, since CommonMark ends a line at a lone
                   `\r` too and an `\n`-only split desyncs every index after the first
                   stray CR; `didChange` is COALESCED (120 ms) because publishing
                   re-walks every page, and that publish runs in `main_loop`'s timeout
                   arm, so **that arm needs its own `guarded`**; and `$/cancelRequest` is
                   batch-scoped, so `read_batch` must not read past `shutdown` (the `exit`
                   that follows belongs to `handle_shutdown`)
editor/vscode/   the VS Code companion. It implements NO language features of its own:
                 `src/client.ts` is a `vscode-languageclient` over `taliesin lsp`. What
                 is left in TS is what LSP has no concept of — the preview webview +
                 bidirectional source sync, editor commands, and `src/embedded.ts`, which
                 forwards completion inside a `{python}`/`{js}` cell to whoever owns that
                 language. Even that keeps the knowledge in Rust: cell locations come from
                 `taliesin/cellRegions` (`lsp_cells.rs`), never from a fence scan in TS.
                 **Add an editor feature in Rust, not here.**
web-client/      browser preview client (vanilla JS, the only client): client.js mounts
                 blocks + applies ops (Ctrl-click opens source in the editor),
                 search.js (Cmd-K), toc-spy.js (scrollspy)
docs/            project's own manual: TWO sibling book projects, authored in .tmd
                 (dogfooding). docs/guide/ = User Guide (using/ + reference/);
                 docs/internals/ = Internals book. docs/ itself is just a container
                 (no _site.yml). Each publishes to its OWN domain
                 (guide/internals.taliesin.sh), not under the marketing site.
gallery/         the exhibit index: its own project + domain, the ONE project that
                 builds others (corpus/{tarn,descent,analyst}) under its output
corpus/          the real .tmd docs (the spec); cargo test renders them all
```

## Read before working

- **docs/** is the manual, in two sibling book projects: `docs/guide/` (how to *use*
  Taliesin) and `docs/internals/` (architecture, pipeline, block model, execution model,
  dev server, extending). Preview either with `taliesin preview docs/<book>`. They are
  siblings because the page-walker would otherwise swallow a nested book's pages, and
  **each is its own Cloudflare Pages deploy on its own domain**, so cross-book links are
  ABSOLUTE URLs (`https://guide.taliesin.sh/using/choosing.html`).
  `crates/core/tests/cross_site_links.rs` resolves every such URL against the source
  tree, so a renamed page fails `cargo test` instead of a reader's click.
- **corpus/README.md** for what the test documents exercise.

## Commands

```sh
cargo run -p taliesin-server -- preview <file.tmd> [port]      # live preview
cargo run -p taliesin-server -- preview <dir>                  # live multi-page SITE preview (nav + per-page hot reload)
cargo run -p taliesin-server -- build  <file.tmd> [out.html]   # self-contained HTML file (default <name>.html)
cargo run -p taliesin-server -- build  <file.tmd> --out <dir>  # portable folder: <dir>/index.html + copied local assets (+ mermaid.min.js if it has a diagram; the single-file spelling above inlines that instead)
cargo run -p taliesin-server -- build  <dir> [--out <dir>]     # multi-page SITE -> _site/ (one .html per page + assets)
cargo run -p taliesin-server -- build  <file.tmd> --stdout     # the page to stdout (+ --no-exec for a static dump)
cargo run -p taliesin-server -- build  <dir> --check-only      # THE PRE-PUBLISH GATE: lint, write nothing, exit non-zero
                                                               #   (+ --strict to fail on advice, + --format json for one machine surface)
./tools/publish.sh [--check] [site|guide|internals|gallery]    # build + deploy the four sites (--check = build all, deploy none)
cargo test -p taliesin-core                                    # corpus invariants + unit tests
cd web-client && npx -y -p typescript tsc -p jsconfig.json     # type-check the client JS (client.js + search.js/toc-spy.js; // @ts-check, no build step)
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json  # type-check the bundled assets JS (code-enhance/ fragments + tali-js.js + mermaid.js, strict; globals.d.ts + web-client's are merged)
```

The CLI is **six subcommands**: `preview`, `build`, `init`, `doctor`, `lsp`, `help`.
A `taliesin` launcher on `PATH` (`~/.local/bin/taliesin`) rebuilds the release binary
when the tool's sources change, then runs it, so `taliesin preview <file>` works from
anywhere. For UI work, `/preview <file.tmd>` builds, serves on port 4388, and verifies it
in the browser via the chrome-devtools MCP. A `PostToolUse` hook runs `rustfmt` on every
edited `.rs` file, so the tree stays `cargo fmt`-clean.

**Editing `assets/css/*` or `assets/js/*` needs a `cargo build` before the change shows
up.** They are `include_str!`-compiled into the binary, so rebuilding only the site
re-emits the *old* bundled CSS/JS and you will measure a stale page. (A live `preview`
hot-swaps CSS, so this bites the build-and-inspect loop, not the dev loop.)

## Executing cells

`{python}` cells need a Python with `ipykernel` (`TALIESIN_PYTHON`, default `python3`).
`{r}` was the second kernel language and was cut in Wave 6, so `Executor::langs` and
`FreezeCache::packages` are one-key maps that **must stay maps**. Without a kernel, cells
render as source and the preview shows a "kernel unavailable" diagnostic.

A cell is capped on **silence, not runtime**: one that produces no output for
`TALIESIN_CELL_SILENCE` seconds (default 600; `0` disables) is interrupted (SIGINT), while
a long cell that prints progress resets that budget on every line and runs to completion.
`TALIESIN_CELL_TIMEOUT` is an optional wall-clock cap, **off by default**. Either way the
warm kernel and prior cells survive.

Cell outputs persist in `_freeze/` (gitignored), keyed by a cumulative content hash (this
cell's code + all upstream same-language code + interpreter id), so a change busts that
cell and everything downstream with no stale hits and nothing to clear by hand. Errors,
`#| cache: false` cells **and everything downstream of one** are never persisted: a
downstream entry would assert "this output follows from this upstream code" about the one
cell whose output does not, and deleting the directive later publishes that contradiction.
`exec.rs`'s `first_uncacheable` is the single definition both that rule and `plan`'s re-run
range turn on. `TALIESIN_NO_CACHE` skips the cache; "Restart kernel" forces a fresh re-run.
Kernel *variable* state is never cached, so a cold start can only skip work when the whole
document is unchanged.

## Gates

**`./tools/gates.sh` runs every gate in one process and refuses to be green unless every
one of them actually ran.** That is the point of it: several gates *skip silently* when
their interpreter is absent, so a plain `cargo test` can be green and mean almost nothing.
The script arms `TALIESIN_REQUIRE_KERNEL` and `TALIESIN_REQUIRE_NODE`, asserts by name that
each interpreter's canary test printed `... ok`, and treats a single ignored test as a
failure. It needs `TALIESIN_PYTHON="$PWD/.venv/bin/python"` or it exits 2 at preflight.
**Take the gate count from the script's own verdict line; never trust a count written in
prose.** Reach for it instead of running the pieces by hand — it covers the live-kernel
suite, the Node-backed reactive test, both `tsc` type-checks, the companion's tests,
`cargo audit`/`cargo deny check`, the two `build docs/<book> --check-only` document gates,
`tools/publish.sh --check` and the portability census. Never call one of these verified
without its output.

**`.githooks/pre-push` is the only gate that runs automatically today.** It is wired via
`core.hooksPath`, so it is invisible in `.git/hooks`, and **unset in a fresh clone** — it
exists for nobody but the author. A push that includes `main` runs fmt, clippy, the
workspace tests, both document gates and `tools/publish.sh --check`; a WIP-branch push
skips it, and `git push --no-verify` bypasses. `gate_script.rs` cross-checks the two lists
(`every_pre_push_command_is_also_run_by_the_gate_script`,
`every_docs_book_is_linted_by_every_gate_file`), so a third book cannot inherit a hole.

`.github/workflows/ci.yml` exists but **every job is guarded on
`github.event.repository.private != true`**, so while this repo is private it is inert and
certifies nothing. Do not credit it for a check until the repo is public.

## Conventions

- Rust edition 2024, workspace resolver 3. Shared deps go in the root
  `[workspace.dependencies]` so versions stay centralized.
- **Never publish a number about this tool that has no committed instrument.** A number
  without one carries its measured-on date and is re-measured before a release tag. The
  portability census is gated (`tools/portability-census.py --verify`) because the page
  hands the reader the command, which makes a mismatch self-refuting rather than merely
  stale; it had already rotted twice. Wall clocks, binary size and crate count are NOT
  gated — they measure the machine, so they carry a date instead.
- Every emitted block carries `data-block-id` (content-hash) + `data-sourcepos`; included
  blocks also carry `data-source-file`. Source mapping, incremental re-render and
  live-state preservation all key off this one block model, so preserve those invariants
  (`crates/core/tests/corpus.rs` enforces them).
- **There are TWO line coordinate systems in `render/mod.rs` and pairing them is the bug
  that keeps happening.** `buf_start` is the line in the post-include BUFFER (what
  `group_divs` matches `:::` spans in); the mapped line from `map_origin`/`map_span` is the
  line in the file the author actually wrote. **A `source_file` may only ever be paired
  with a mapped line** — any include shifts every later buffer line, so the mismatch puts a
  diagnostic N lines off in a real openable file with nothing signalling it. For the same
  reason a block's `data-sourcepos` range must stay inside ONE file's numbering: mapping
  the two ends independently emits spans like `39:1-6:25` on a paragraph comrak merged
  across an include boundary, which `client.js`'s `highlightAtLine` skips outright.
  `map_span` is the single answer to both ends.
- **A duplicate element id is RENAMED, never refused** (`dedup_element_ids`, the last
  id-assigning pass). The first definition keeps the author's own spelling, so every link
  and `@ref` they wrote still resolves; the duplicate draws an error-severity located
  diagnostic. Refusing the build would invent a hard-fail path no other error-severity
  diagnostic has and would leave the preview rendering the invalid page anyway.
- **`vocab.rs` is the OFFERED-completions subset, not the implemented set.** It agrees
  with the validators for div classes (`DIV_CLASS_NAMES` and `render::DIV_FEATURE_CLASSES`
  are the same **2** width escapes, and a test pins the subset relation), but
  `xrefPrefixes` offers **5** of the **12** `XREF_LABELS`, because the other seven resolve
  a label for a construct nothing can define any more. Answer "what does the tool support"
  from the validator consts, never from `vocab`.
- **Taliesin answers for its own vocabulary and nothing else** (ruled 2026-08-17). The
  five retirement registers that named cut features and other tools' spellings are gone:
  an unknown key, cell option, callout kind, shortcode, div class, flag or verb gets a
  did-you-mean inside edit distance 2, a bare "unknown", or — for div classes, an open
  vocabulary — silence and the author's own CSS. **Do not reintroduce a register, a
  compatibility note, or a "did you mean <another tool's key>" answer.** Withdrawing a
  construct means deleting the *read* as well as the vocabulary entry: dropping a key
  from `KNOWN_KEYS`/`HERO_KEYS`/… only makes it *diagnosed*, and a key the parser still
  honours goes on working (`listing: sort:` really did reverse the cards for eleven days
  after it was "retired"). A parser-side pin is the only thing that says the read is gone.
- **A new front-matter key trips FOUR drift gates.** `KNOWN_KEYS`,
  `the_reference_page_documents_every_known_key` (→
  `docs/guide/reference/frontmatter.tmd`), `vocab.rs` + its `descriptions_present`, and —
  for a `_site.yml` key — `editor/vscode/schema/tali-site.schema.json`, a bundled COPY of
  the crate's schema gated only by the companion's own `node --test`. **That last one
  lives outside `taliesin-core`**, so `cargo test --workspace` can be green while it is
  stale; only `./tools/gates.sh` catches it.
- **A new subcommand has four registration sites in `main.rs`**, each drift-gated, and a
  fifth in `docs/guide/reference/cli.tmd`'s table.
  `every_subcommand_has_a_row_in_the_cli_reference` walks that table in both directions,
  so a documented verb the binary does not answer fails too. Same genus as every ungated
  prose claim in this file: grep, do not trust.
- Minimal config: perfect the default before adding a knob. Aim for a near-perfect default
  experience so the user does not *need* to configure; prefer a better default over a new
  option. This is the deciding lens for any new user-facing control. **A reader-local
  preference is an argument for honouring the answer they already gave their OS**, not for
  asking them again per site — which is why the theme picker, the text-size control and the
  spacing control are all gone (the browser's own zoom outranks a comfort panel). Ship no
  reader-facing control without new evidence that the device answer is wrong for someone.
