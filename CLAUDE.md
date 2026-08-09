# Taliesin

A single-purpose Rust dev server that renders `.tmd` files to **HTML only** (blog
posts, papers, books, multi-page sites) for one author's workflow, built around
three load-bearing goals: click-to-source, block-level incremental updates, and no
per-edit startup cost (warm server + Jupyter kernel). It is **not** a general document
compiler: HTML is the only output target (no LaTeX/Typst/Word/ePub; a future print/PDF
track would render *from* the built HTML, never as a parallel format).

> ## ⚠ THE PROJECT IS IN SCOPE REDUCTION. READ THIS BEFORE PLANNING ANY WORK.
>
> **Ruled 2026-08-08: the tool is being cut by roughly 40%** (~69,000 lines), from 18
> CLI verbs to 9 (**reached in Wave 8**) and 115 document features to ~55, to reach a surface
> small enough to polish before release. **This supersedes the growth framing below.** Do not add
> features, do not "restore parity", and do not defend a feature on the grounds that a
> corpus document pins it (that pinning is circular and is the very thing the audit
> disproved).
>
> **THE CUT IS COMPLETE. Wave 13 landed 2026-08-09 and there is no wave 14.** It
> adjudicated the one bundle the ruling left open and **cut `taliesin run` entirely**, so the
> CLI is **7 subcommands**: `preview`, `build`, `init`, `new`, `doctor`, `lsp`, `help`. The
> corpus is 83 documents and the Internals book is 7 chapters; a fresh session should read
> `notes/CUT-PROGRESS.md` before assuming a file named in the playbook still exists, because
> eight of wave 12's "must survive" justifications named files that had already gone.
>
> **The campaign is over; its doctrine is not.** The standing directive below still governs,
> and so do the register rules and the ordering rule. What changes is that there is no next
> wave to defer a question to: `notes/ROADMAP.md` can be unpaused by the author.
>
> **Standing directive from the author:** *"always lean towards cutting. I'd rather have
> a polished lean product, and then add features when I have real users that need them
> than having a bloated product with features that nobody uses."* When a call is close,
> cut.
>
> Read in this order:
> 1. `notes/CUT-PROGRESS.md` — the durable state: what has landed, what is next, the rules.
> 2. `notes/2026-08-08-scope-ruling.md` — the verdicts, the evidence, the corrections.
> 3. `notes/2026-08-08-cut-playbook.md` — the 182 file-level removal steps, by wave.
>
> **One wave per session, one branch, one commit, `./tools/gates.sh` green before and
> after.** `notes/ROADMAP.md` is PAUSED for the duration; its open items are not to be
> worked until the cut lands.

**Scope was corpus-plus-roadmap** (paused, see the box above). "Done" still means the
documents under `corpus/` render correctly: the corpus is the regression net and the
arbiter of done. **"Wider" means richer browser behavior in a live HTML view, not new
output formats, and never at the cost of the load-bearing invariants or the
Do-NOT-touch discipline.** The active roadmap is `notes/ROADMAP.md` (successor to the
completed `notes/native-rewrite.md`).

**The corpus records; it does not lead.** The rule used to be "each new capability ships
pinned by a target corpus document added in the same change". That is retired, because
it made the corpus circular as evidence: the `taliesin features` scan (cut in Wave 2)
reported 0 of 115 features unused, which is guaranteed by construction and therefore says
nothing. Pointed at the 79 documents written to be *read*, the same instrument reported 32
features used by nothing anywhere. **The keep rule now:** a corpus document earns its place by being
something a person wanted to read, or by being a golden no unit test can hold. A feature
witness belongs in `crates/core/src/render/tests.rs`.

> **⚠ ORDERING RULE, and it is the one most likely to bite during the cut.** *A pin and
> its docs page are deleted in the SAME commit as their feature, never before.* A corpus
> document deleted ahead of the code it guards leaves that code silently unguarded while
> every gate still passes — the sweeps in `crates/core/tests/corpus.rs` iterate over
> whatever exists, so removing a document removes coverage without removing a test.

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
drag-to-reorder feature once broke this and was removed: a second write path
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
    emit.rs          per-block HTML (server-side highlighting, code line-wrapping)
    divs.rs          `:::` fenced divs (callouts, the `layout-ncol` grid, width escapes)
    figure.rs        numbered figures + captions
    extension/       shortcode expansion: `{{< input >}}` (the only one that expands
                     here; `{{< include >}}` is resolved a pass earlier). NOT `_extensions/`, which is a theme-CSS lookup
                     in `theme.rs` and nothing else: there is no format-extension
                     mechanism, and no `format:` key either — HTML is the only output
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
  src/diagnostics/ the static validators `lint::page_static_diagnostics` runs: headings,
                   anchors, assets, media, links, the `{js}` reactive graph, a11y (alt text
                   + heading skips), bibliography, and `RETIRED_CELL_LANGS`. **The keep test
                   is "a defect the author cannot see in the rendered page"**. That is why
                   wave 9 cut document shape, KaTeX render failure, link-text collision,
                   accessible-name and the generic unknown-fence-language lint, and why the
                   media + reactive rules stayed
  src/cite/        citations ([@key]) + cross-references (@fig-, @sec-): a module dir
                   (parse/render/format/validate/author/clean)
  src/site/        multi-page project (mod.rs): _site.yml config (config/), page
                   discovery, chrome, link rewrite, listings + `hero:` blocks,
                   front-matter parse (frontmatter.rs), books (book.rs),
                   Atom feeds per dated listing (feed.rs, which also owns
                   `nav_ordered`), sitemap.xml + robots.txt (seo.rs),
                   the five-tag OpenGraph head (meta.rs — `og:image` is the page's
                   own front-matter `image:`; the generated social-card rasterizer,
                   the JSON-LD graph, llms.txt and the PWA manifest were all cut in
                   Wave 4), Cmd-K search (search.rs),
                   cross-refs (xref.rs). ONE project per build: composing several
                   into one deploy is tools/build-site.sh, parent first (the
                   parent's sweep deletes output it did not itself write)
  assets/          bundled offline: css/ (base, dark, site),
                   js/ (code-enhance/ fragments, mermaid.js, tali-js.js
                   + vendored plot.umd.min.js/d3.min.js for `{js}` cells), katex/
crates/server    taliesin-server, bin `taliesin`: CLI + websocket dev server
  src/main.rs      the subcommand dispatch + COMMANDS + RETIRED_COMMANDS (a verb that
                   was cut names its replacement instead of a did-you-mean)
  src/cli.rs       CLI arg parsing + subcommand dispatch
  src/serve/       the dev server's SHARED layer, not a server: HTTP/asset plumbing,
                   port binding + the single-instance probe, security.rs's origin/Host/
                   identity guards, the watch predicates, and the CLI error helpers
                   (`guarded`, `unknown_flag_error`, `bad_format_error`) that eight
                   modules import, plus `RETIRED_FLAGS` (the fourth CLI register, read
                   by `unknown_flag_error` ahead of the did-you-mean). Wave 1.1 deleted
                   the single-doc server that used to live here; the path stays so
                   `crate::serve::` imports resolve. **The preview binds 127.0.0.1 and
                   nothing else** (`--host` went in Wave 11), so both guards are about a
                   local peer: `ws_origin_ok` is the only thing stopping an open tab
                   sending `restart_kernel`, and `with_host_guard` is the unconditional
                   DNS-rebinding allowlist. Wave 13 removed the last two POST routes
                   (`/__taliesin/run`, `/__taliesin/interrupt`) with `taliesin run`, so the
                   only write the server accepts from a client is `restart_kernel` over the
                   websocket
  src/serve_site/  THE dev server — one server for a project and for a single document
                   alike (mod.rs: per-page state/executor, cross-page nav, hot reload;
                   exec_pool.rs: the MAX_WARM_PAGES LRU, the one freeze). ONE project
                   per server since Wave 11 cut `mounts:`, so the cap now means what its
                   name says (it used to be per-project, letting site/'s seven mounts
                   keep 8x the cap resident). Composing several projects into one
                   deploy is `tools/build-site.sh`, run by `.githooks/pre-push`.
                   `preview <file.tmd>` resolves to the file's enclosing `_site.yml`
                   project, opened at that page; with no ancestor `_site.yml` it is a
                   project of just that document (`Site::discover_single`). That
                   synthesized one-page project renders no navbar and no footer (the
                   theme toggle still shows), so `preview <file>` and `build <file>`
                   now agree on page chrome; the table of contents does not share that
                   agreement: `preview` auto-detects and renders one, `build` does
                   not. A directory with no `_site.yml` is refused by both verbs, with
                   guidance to build or preview one page inside it, or add a
                   `_site.yml`. Previewing a file alone would be an orphan (no
                   cross-page nav, dead cross-page links), which is why the companion
                   already resolved to the project (item 150)
  src/exec.rs      runs a doc's code cells, splices outputs back as blocks; plans
                   what re-runs via cumulative-hash keys (warm reuse + cold replay)
  src/freeze.rs    persistent execution cache (`_freeze/<page>.json`): rendered cell
                   outputs keyed by a cumulative content hash, so unchanged cells
                   restore instead of re-executing across builds + preview restarts.
                   Also records the `packages:` digest each output was produced under
                   (`packages.rs`) — the one axis the key cannot see — so a replay that
                   crossed a `pip install --upgrade` says so; it does NOT change what hits
  src/packages.rs  what an interpreter actually has installed (`name==version`) + one
                   digest for the set. Read by `doctor --format json` and by `freeze.rs`;
                   one memoized subprocess per interpreter per process
  src/kernel.rs    warm Jupyter kernel (ZMQ), reused across edits
  src/log.rs       colorized dev-server console output (to stderr)
  src/lint.rs      the SHARED static-lint kernel, not a verb: `Diagnostic`, `diag_from`,
                   `blocking` (what fails `--strict`), `page_static_diagnostics` (the
                   check-superset, ONE definition for `build`, `build --check-only`, the
                   preview and the LSP), `buffer_diagnostics_in_site` (the LSP's seam) and
                   `cmd_check_only` (the ~40-line front door). It was `check.rs`, a 5-flag
                   verb with an interpreter probe, until wave 9. **Severity is a field on
                   `render::Warning`**, set by the validator that found the defect: there is
                   no `TAL-*` code catalogue and no `docs/DIAGNOSTICS.md` any more, and a
                   reworded message can no longer silently reclassify a family
  src/lsp*.rs      `taliesin lsp`: the offline, kernel-free LSP server (lsp.rs dispatch +
                   capabilities; lsp_complete/lsp_nav/lsp_outline/lsp_pos/
                   lsp_fold/lsp_cells/lsp_diag/lsp_project).
                   ALL editor intelligence lives here, and since 2026-08-09 it is **SIX
                   read-only providers**: completion, hover, definition, documentSymbol,
                   codeAction, foldingRange, plus pushed `publishDiagnostics`
                   and two namespaced extensions (`taliesin/cellRegions`, `siteMap`).
                   `codeLens` was the seventh until wave R6 cut it: wave 13 had already taken
                   `taliesin run` out from under its ▶ Run Cell button, leaving a bare
                   `⚡ cached` label shipped as a `Command` with an empty name, and the one
                   ground on record for keeping it (that `runcell.ts` proved a TypeScript lens
                   would regrow) was spent — that file had gone with the verb. **Nothing here writes to a buffer except a code action the author
                   explicitly invokes** (`lsp.rs`'s "Change to `X`" quick fix returns a
                   `WorkspaceEdit`, which is the standard LSP contract and is pinned by its
                   own test). The distinction that matters is user-invoked versus
                   server-initiated: nothing here rewrites a `.tmd` on its own. The nine that went
                   on 2026-08-08 are named in `the_initialize_handshake_advertises_…`,
                   which fails if one is advertised again: five of them (formatting,
                   rename, `sectionEdit`, `insertEdit`, `renameFileEdits`) were the only
                   paths besides the author's own keystrokes that rewrote a `.tmd`, and the
                   single-editing-surface rule is one owner, not two. The whole-book answer
                   the `diagnostic` pull model gave is `build <dir> --check-only`.
                   stdout is the JSON-RPC wire, so never print to it (use `crate::log`,
                   stderr). `didChange` is COALESCED (a 120 ms window in lsp.rs) because
                   publishing diagnostics re-walks every page in the project. (`lsp_memo`, a
                   buffer-render cache keyed on `(uri, text)`, went with the code lens in
                   wave R6: the lens arm was its only consumer, which nothing had noticed.)
                   **`$/cancelRequest` is batch-scoped**: the loop
                   drains the channel before dispatching, so a request the client withdrew
                   while it was queued is abandoned rather than run; a cancel is matched
                   only against requests in the same batch, and `read_batch` must not read
                   past `shutdown` (the `exit` that follows belongs to `handle_shutdown`)
editor/vscode/   the VS Code companion. It implements NO language features of its own:
                 `src/client.ts` is a `vscode-languageclient` over `taliesin lsp`. What is
                 left in TS is what LSP has no concept of — the preview webview +
                 bidirectional source sync, editor commands, and `src/embedded.ts`, which
                 forwards completion inside a `{python}`/`{js}` cell to whoever owns
                 that language (LSP cannot express "go ask Pylance"). Even that keeps the
                 knowledge in Rust: cell locations come from the server's
                 `taliesin/cellRegions` (`lsp_cells.rs`), never from a fence scan in TS.
                 Add an editor feature in Rust, not here (a second copy in TS is what this
                 replaced; see `notes/2026-07-28-vscode-companion-audit.md`)
web-client/      browser preview client (vanilla JS, the only client): client.js mounts
                 blocks + applies ops (Ctrl-click opens source in the editor),
                 search.js (Cmd-K), toc-spy.js (scrollspy)
docs/            project's own manual: TWO sibling book projects, authored in .tmd
                 (dogfooding). docs/guide/ = User Guide (using/ + reference/);
                 docs/internals/ = Internals book. docs/ itself is just a container
                 (no _site.yml). tools/build-site.sh writes each under the marketing
                 site at /docs/guide + /docs/internals.
corpus/          the real .tmd docs (the spec); cargo test renders them all
```

## Read before working

- **docs/** is the project's own manual, authored in `.tmd` as TWO sibling book
  projects (dogfooding):
  - **`docs/guide/`** = the User Guide (`using/` feature showcase + `reference/`):
    how to *use* Taliesin. Preview it: `taliesin preview docs/guide`.
  - **`docs/internals/`** = the Internals book: the architecture, the rendering
    pipeline, the block model, the execution model, the dev server,
    and how to extend it. Preview it: `taliesin preview docs/internals`.
  - `docs/` itself is just a container (no `_site.yml`); the books are siblings
    because the page-walker would otherwise swallow a nested book's pages. The
    marketing site's deploy carries them at `/docs/guide` + `/docs/internals`, composed
    by `tools/build-site.sh`. Cross-book links are written as relative `.html` (e.g.
    `../guide/using/formats.html`).
- **corpus/README.md** for what the test documents exercise.

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
./tools/build-site.sh [--check]                                # compose the marketing-site deploy (site + docs books + gallery)
cargo test -p taliesin-core                                    # corpus invariants + unit tests
cd web-client && npx -y -p typescript tsc -p jsconfig.json     # type-check the client JS (client.js + search.js/toc-spy.js; // @ts-check, no build step)
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json  # type-check the bundled assets JS (code-enhance/ fragments + tali-js.js/mermaid/scrolly/tabset/walkthrough, strict; globals.d.ts + web-client's are merged; run it by hand, nothing gates it)
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
Python with `ipykernel` (`TALIESIN_PYTHON`, default `python3`). `{r}` was the second
kernel language and was cut in Wave 6, so `Executor::langs` and `FreezeCache::packages`
are one-key maps that **must stay maps** — see the wave-6 prohibitions. Without a
kernel, cells render as source and the preview shows a
"kernel unavailable" diagnostic. A cell is capped on **silence, not runtime**: one
that produces no output for `TALIESIN_CELL_SILENCE` seconds (default 600; `0`
disables) is interrupted (SIGINT), while a long cell that prints progress resets that
budget on every line and runs to completion. A streaming runaway is caught by the
output caps instead. `TALIESIN_CELL_TIMEOUT` is an optional wall-clock cap, **off by
default**. Either way the warm kernel and prior cells survive.

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

**`.githooks/pre-push` is the only gate that runs automatically today.** It is wired
via `core.hooksPath`, so it is invisible in `.git/hooks`: a push that includes `main`
runs `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace`, `build docs/guide --check-only --no-exec` (new in
wave 9) and `tools/build-site.sh --check` (new in wave 11) first, and a WIP-branch push
skips it. Step 4 is the DOCUMENT gate the `check` verb never had wired anywhere: a broken
cross-reference in this project's own manual used to reach a reader with every gate green.
Step 5 is what replaced `mounts:`: it composes the whole deploy with `--no-exec` into a
temp dir and refuses the push if any cross-project link in `site/` has nothing behind it,
because a composition script nobody runs is how this project's own call-to-action shipped
404ing (item 149).
`git push --no-verify` bypasses. Note `core.hooksPath` is **unset in a fresh clone**,
so this hook does not exist for anyone but the author.

`.github/workflows/ci.yml` was restored on 2026-07-28 (it had been deleted on
2026-07-26 for billing Actions minutes on this private repo) and covers all of the
above plus everything below. **Every one of its jobs is guarded on
`github.event.repository.private != true`,** so while this repo is private it is
inert and certifies nothing; it arms itself the moment the repository is public,
where standard runners are free. Do not credit it for a check until then.

**`./tools/gates.sh` runs every gate in one process and refuses to be green unless
every one of them actually ran.** That is the point of it: the gates below *skip
silently* when their interpreter is absent, so a plain `cargo test` can be green and
mean almost nothing. The script arms **both** `TALIESIN_REQUIRE_*` variables
(`TALIESIN_REQUIRE_KERNEL` and `TALIESIN_REQUIRE_NODE` — it armed four until wave 6 cut
the `{r}` cell language and the headless-Chrome driver, and nothing is left for those two
to gate), asserts by name that each interpreter's canary test printed `... ok`, and treats
a single ignored test as a failure. Reach for it instead of running these by hand: the
live-kernel suite (Python, `TALIESIN_REQUIRE_KERNEL`), the Node-backed reactive
live-region test (`TALIESIN_REQUIRE_NODE`), the two `tsc` type-checks above, the VS Code
companion's offline TextMate grammar test, `cargo audit` / `cargo deny check`
(`deny.toml` is still the policy) on any dependency change, and **the two document gates
the pre-push hook also runs** (`build docs/guide --check-only` and
`tools/build-site.sh --check`). Never call one of these verified without its output.

**It runs TEN gates and prints the count.** It ran eight while this paragraph claimed all
of them, for two waves: the document gate (wave 9) and the composition gate (wave 11) were
wired into `.githooks/pre-push` and never added here, because neither can *skip* and so
neither looked like this script's problem. An absent gate hollows out "runs every gate"
exactly as completely as a skipped one, and more quietly, since there is no `SKIPPED` line
to read. `gate_script.rs`'s `every_pre_push_command_is_also_run_by_the_gate_script` now
compares the script's list against the hook's on every run. The hook keeps running both
too — it is the only gate that runs automatically and this script is manual, so the two
are a pair, not a move.

## Conventions

- Rust edition 2024, workspace resolver 3. Shared deps go in the root
  `[workspace.dependencies]` so versions stay centralized.
- Every emitted block carries `data-block-id` (content-hash) + `data-sourcepos`;
  included blocks also carry `data-source-file`. Source mapping, incremental
  re-render, and live-state preservation all key off this one block model, so
  preserve those invariants (`crates/core/tests/corpus.rs` enforces them).
- **`vocab.rs` is the OFFERED-completions subset, not the implemented set.** After wave 7
  the two agree for div classes (`DIV_CLASS_NAMES` and `render::DIV_FEATURE_CLASSES` are
  both the same **3** width escapes, and a test pins the subset relation), but `vocab`
  still under-reports elsewhere: `xrefPrefixes` offers **5** of the **12** `XREF_LABELS`,
  because the other seven resolve a label for a construct nothing can define any more.
  Answer "what does the tool support" from the validator consts, never from `vocab`.
  (`taliesin features` was the other honest instrument; Wave 2 cut it, so the consts are
  now the only one.)
- **A new front-matter key trips FOUR drift gates; a RETIREMENT costs ONE line.**
  Adding a key still means `KNOWN_KEYS`, `the_reference_page_documents_every_known_key`
  (→ `docs/guide/reference/frontmatter.tmd`), `vocab.rs` + its `descriptions_present`,
  and — for a `_site.yml` key — `editor/vscode/schema/tali-site.schema.json`, a bundled
  COPY of the crate's schema gated only by the companion's own `node --test`. **That last
  one lives outside `taliesin-core`**, so `cargo test --workspace` can be green while it is
  stale: only `./tools/gates.sh` catches it. (The `agents_md` golden was the fifth until
  Wave 2 deleted `AGENTS.md`.)
  **Retiring is the cheap direction, as of 2026-08-08.** It used to cost eight gates and a
  ~39-line hand-written tombstone. Both extras are gone: the migration page that had to
  name every retirement was deleted, and the three vocabulary tombstones collapsed into
  `render::validate`'s
  `every_retired_vocabulary_name_is_gone_unstyled_and_diagnosed_without_a_did_you_mean`,
  which DERIVES the tombstone from the register. **Add the register entry and you are
  done — do not write a test for it.**
  A new *subcommand* has four registration sites in `main.rs`, each drift-gated. It cost
  three more until Wave 8 deleted the shell-completion generator, which carried a
  hand-maintained second copy of every verb's flag set. **There is a FIFTH site and nothing
  gates it: the verb's row in `docs/guide/reference/cli.tmd`'s table.** Wave 13 left the
  retired `run` row standing through several edits; what eventually caught it was
  `documented_cli_flags_exist_in_the_cli` noticing the *flags* inside the row, so a verb
  with no flags would have left a documented command the binary does not answer, with every
  gate green. Same genus as the two `--help`-prose holes below: grep, do not trust.
- **`RETIRED_KEYS` is SCOPED — `(scope, key, note)` — and nothing may flatten it.** The same
  word is retired in one vocabulary and live in another: `toc:`/`theorems:` are gone from
  `_site.yml` but live in front matter, `image:` is gone from `hero:` but live at top level,
  `echo:` is gone from `execute:` but live as `#| echo:`. Both validators consult the register
  through `unknown_key_message` (the `_site.yml` side under the `config key` scope; it had its
  own `did_you_mean` until Wave 2 and answered a retired `toc:` with "did you mean `logo`?").
- **A withdrawn div class needs a `RETIRED_DIV_CLASSES` entry**; div classes are an *open*
  vocabulary, so without one a leftover class gets **silence**, not a did-you-mean, and the
  page quietly loses its layout. (Front-matter keys have `RETIRED_KEYS` for the same job,
  a retired verb `RETIRED_COMMANDS`. A retired **shortcode** needs no register of its own:
  that vocabulary is CLOSED, so a leftover already draws a located warning, and
  `expand_in_line` reads the removal note out of `RETIRED_KEYS` under the `shortcode`
  scope.) **The entry is ONE SENTENCE — the date, then the
  successor or an explicit "nothing" — not a migration paragraph.** An author reads it
  mid-edit and wants the replacement; the deliberation belongs in the commit that retired
  the thing. No entry may be phrased as a did-you-mean: `codes::extract_suggestion` lifts
  that exact phrase into a fix an agent applies mechanically, and none of these are
  mechanical renames.
- Minimal config: perfect the default before adding a knob. Aim for a
  near-perfect default experience so the user does not *need* to configure;
  only explore configuration once the defaults are perfected, and prefer a
  better default over a new option. Reader-local a11y preferences (theme, text
  size, spacing) are exempt, they are personal, not document config. This is the
  deciding lens for any new user-facing control.
