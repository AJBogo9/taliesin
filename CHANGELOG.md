# Changelog

All notable changes to Taliesin are recorded here. From 1.0 this project follows
[semantic versioning](https://semver.org/): a breaking change to the load-bearing
invariants (content-hash block model, click-to-source, single editing surface, HTML-only
output) or to the CLI's six verbs needs a major version. Before 1.0 the policy was looser
and minor versions carried breaking changes; the 0.x entries below were written under it.

## [Unreleased]

## [1.0.1] - 2026-08-21

No code changed. This release exists because v1.0.0's Linux binary could not run on most
Linux machines, and because nothing in the project pointed a reader at the binaries at all.

### Fixed

- **The Linux release binary is now statically linked against musl**
  (`x86_64-unknown-linux-musl`, replacing `x86_64-unknown-linux-gnu`). A gnu build inherits
  the runner image's glibc as a floor; `ubuntu-latest` had become 24.04, so the v1.0.0 asset
  required `GLIBC_2.39` and failed to start on Ubuntu 22.04, Debian 12, RHEL 9 and Amazon
  Linux 2023. It went unseen because the only machine that had ever run a released binary
  was new enough to load it. A static binary has no floor left to drift.

### Changed

- **The install instructions lead with the download.** README, the User Guide's getting
  started page and the marketing site all opened by telling a reader to clone the repository
  and compile 229 crates, while three prebuilt tarballs with checksums sat on the releases
  page unmentioned. `readme_install_command_names_the_current_version` now fails the suite if
  the version in that command drifts from the workspace version.

## [1.0.0] - 2026-08-20

The scope is closed. This is the first public release: 1.0 states that the feature set is
final for this tool's one use case, not that nothing changed since 0.3.0. Alongside the
scope-closing cuts below, this release also ships a full visual redesign and a new public
gallery site.

### Added

- **A full visual redesign** ("Instrument"): the tool now owns its typography (Literata for
  prose, JetBrains Mono for code and labels, replacing borrowed system fonts) and its
  colour (two palettes designed rather than inverted from each other, an owned
  syntax-highlighting palette replacing a borrowed one, every text colour shipped with a
  computed WCAG contrast ratio). Reading measure, heading scale and spacing scale are all
  derived from one system instead of copied per component, and the favicon now follows the
  reader's device like every other painted surface.
- **A public gallery site**, its own project and domain: five self-contained one-page demos
  (a gradient-descent explainer, a parametric meshed-gears model, an executed data report,
  an API-documentation craft piece, and a molecules demo), plus an index that dogfoods
  `listing:` with theme-aware card thumbnails.

### Changed

- **The project is public**, and the version says the scope is closed. Feature requests are
  closed by design from here; bug reports are welcome. See "Project status" in `README.md`.
- The CLI is **six subcommands**: `preview`, `build`, `init`, `doctor`, `lsp`, `help`.
- **The four public sites deploy separately** instead of composing into one domain's
  subpaths: the marketing site, both docs books, and the gallery each build, preview and
  publish as their own project on their own domain, linking to each other by absolute URL.

### Removed

Continuing the 2026-08 reduction campaign:

- The `lang:` and `csl:` front-matter keys, `page-layout: full`, and link attribute blocks
  (`[text](url){.class}`).
- The `theme:` key and all author theme control. Both palettes always ship and the reader's
  device selects one at paint.
- The `taliesin new` verb: `init` now scaffolds the same dated starter post directly, which
  is why the CLI above is six subcommands rather than the seven `0.3.0` shipped.
- The seven retired theorem cross-reference prefixes (already unable to resolve to
  anything), and the `head:` and `external-prefixes:` site config keys.
- The retirement registers that echoed another tool's spelling for a withdrawn key, verb,
  or class. An unrecognized one now gets a plain did-you-mean or "unknown", the same
  treatment a typo already got.
- The reader-facing code-download aside on pages with code cells, which offered every
  cell's source as a script; the source was already on the page and in view-source.
- The missing-local-video lint and the uncited-entry lint.
- Five VS Code companion features: the first-kernel-failure doctor hint, the build/check
  tasks and their Problems-panel matchers, the Diagnose Setup command, the Get Started
  walkthrough, and the bundled `_site.yml` schema copy. The terminal path replaces the task
  provider: every located diagnostic line in the integrated terminal is clickable.

### Fixed

- `README.md` no longer advertises constructs the tool deleted, and a test now keeps it
  that way.

## [0.3.0] - 2026-08-10

The scope-reduction release: eighteen CLI verbs become seven, and the document
feature set is cut roughly in half. Breaking, and deliberately so — the goal was a
surface small enough to polish before release. Every retired verb still answers
with the one line that says what replaced it.

### Changed

- **Relicensed from MIT to AGPL-3.0-only.** The Affero copyleft closes the
  "SaaS loophole" so a modified network deployment must share its source. As the
  sole copyright holder, the author reserves the right to offer Taliesin under
  other terms (a proprietary hosted service or a commercial license); the
  dependency license policy in `deny.toml` stays permissive-only to keep that
  option open. The VS Code editor companion under `editor/vscode` remains MIT.

- **One preview per project.** Re-running `taliesin preview` on a project that
  is already being previewed now replaces that server rather than binding the
  next free port. Previews answer a `/__taliesin` identity endpoint (canonical
  root, pid, version), so a launch can tell its own project's preview from an
  unrelated server holding the port; an unrelated holder still falls back to the
  next port, so two projects can be previewed side by side. Stacking was not
  harmless: every surplus preview kept its own file watcher and kernel subtree
  re-executing the same sources, on a port nobody was looking at. Any local user
  can bind a loopback port, so the pid a holder reports is treated as untrusted:
  non-positive values (`kill(-1, ...)` reaches every process the user owns) are
  rejected outright, and on Linux the pid is checked against `/proc/<pid>/exe`
  before it is signalled, so answering the probe cannot get an unrelated process
  terminated.

### Removed

- **Eighteen CLI subcommands became seven**, across a thirteen-wave scope-reduction
  campaign (2026-08-08 → 2026-08-09) that also cut the document feature set roughly in
  half. What ships is `preview`, `build`, `init`, `new`, `doctor`, `lsp` and `help`. The
  goal was a surface small enough to polish before release; features come back when real
  users need them.

  Every retired verb is still *recognized* when typed exactly — `taliesin <verb>` answers
  with the line below rather than a did-you-mean over the survivors, because a wrong
  suggestion is worse than none when the person typing it is following an older page:

  | Removed | What replaces it |
  |---|---|
  | `check` | `build <file\|dir> --check-only` lints without writing, and takes `--strict` and `--format json` the same way |
  | `render` | `build <file.tmd> --stdout --no-exec` writes the same page to stdout |
  | `run` | `preview <file.tmd>` executes the same cells against the same warm kernel and writes the same `_freeze/`, so a later `build` still replays without one |
  | `publish` | `build <dir> --out <dir>` writes a plain folder any static host serves (Netlify, GitHub Pages, Cloudflare Pages, rsync) |
  | `serve` | use `preview` |
  | `dev` | use `preview` |
  | `blocks` | `taliesin lsp` publishes the block model now |
  | `symbols` | `taliesin lsp` completes cross-reference targets after `@` |
  | `vocab` | `taliesin lsp` serves the same vocabulary as completions |
  | `map` | nothing on the CLI; `taliesin lsp` answers `taliesin/siteMap` for your editor |
  | `schema` | nothing on the CLI; the VS Code companion bundles the `_site.yml` schema |
  | `features` | `build <dir> --check-only --format json` is the machine surface |
  | `mcp` | `build <dir> --check-only --format json`, run from your agent |
  | `skim` | nothing; read the `.tmd` source |
  | `read` | nothing; read the `.tmd` source |
  | `pdf` | nothing; print the built HTML to PDF from your browser |
  | `completions` | nothing; type the subcommand out, or bind your own shell alias |

  The table is the same data the binary answers from (`RETIRED_COMMANDS` in
  `crates/server/src/main.rs`), so the reader who wonders where `check` went gets the
  same sentence here and at the prompt.

- **The `{r}` cell language, and the R kernel behind it.** `{python}` is the only
  executable kernel language. `TALIESIN_R` and `TALIESIN_REQUIRE_R` are gone with it.

### Fixed

- **The symlink containment check no longer fails open on a bare filename.**
  `safe_join`'s canonical-path check was skipped whenever the containment root
  could not be canonicalized, and `taliesin build index.tmd` (no directory
  component) produced exactly that: the doc's base dir is the empty path,
  `std::path::absolute("")` is an error, so the base stayed relative and the root
  came out empty. An in-tree symlink pointing outside the project was then read
  and inlined verbatim into the page. The same document was correctly refused when
  invoked as `./index.tmd` or as a site, so a `cd` decided whether an out-of-tree
  file leaked. The empty path now resolves against the cwd, and a boundary that
  cannot be canonicalized refuses instead of falling through.

- **A symlinked resource may point anywhere inside the repository.** The symlink
  check reused the *lexical* containment root, so a book whose `_site.yml` bounds
  it to `book/` could not symlink `references.bib` to the `paper/references.bib`
  beside it in the same checkout: the canonical target left `book/` and the file
  was refused. Symlinks are now bounded by the enclosing repository (nearest
  `.git`), falling back to the lexical root outside a checkout. The lexical rule is
  unchanged, so `../../etc/passwd` in the document text is still refused, as is a
  symlink whose target actually leaves the checkout. The distinction: document text
  is what an untrusted `.tmd` controls, whereas a symlink is a filesystem fact
  placed by whoever owns the checkout.

- **The build's asset passes are bounded by the repository too.** `mirror_assets`
  walks the source tree directly rather than resolving paths through `safe_join`, so it
  applied no boundary: a directory or file symlinked out of the checkout was mirrored
  straight into `_site/`. The two ref-driven passes (the portable `--out` bundle, and the
  deploy of linked `.md`/`.scss` sources) checked only that the *ref* was lexically
  in-tree, which says nothing about what an in-tree path resolves to: `<img src="fig.png">`
  where `fig.png` is a symlink passed that test and shipped the target. All three now hold
  a symlink to the same repository boundary the document paths use, and the single-doc
  bundle warns on what it drops.

- **A symlink under the output no longer breaks the walk.** The build emits no symlinks,
  so one under `_site/`/`_book/` is the author's own mount (the stale sweep leaves them in
  place for that reason) and its contents belong in the deploy — but the archive and
  linked-source passes followed a mount pointing back up the tree without a cycle guard.
  The book archive failed outright (`FilesystemLoop` from the walk, taking the whole
  offline download with it); the linked-source pass re-walked the deploy once per level,
  re-copying what it had already shipped (41 deploys of one file in the regression
  fixture). Both descend into each directory once.

- **Site page discovery is bounded by the repository too.** The page walker reads
  directories directly rather than resolving paths through `safe_join`, so it applied
  no containment at all: a `.tmd` symlinked out of the tree was walked, rendered, and
  published as its own page in `_site/`. It now follows a link only while the target
  stays inside the repository. Directory links are additionally deduplicated by
  canonical path: a link back up the tree used to recurse until the path outgrew
  `PATH_MAX`, emitting one duplicate copy of every page per level (41 copies of a
  single page in the regression fixture).

- **A refused resource no longer reports itself as missing.** A `bibliography:`
  that resolved outside the project root warned "bibliography file not found" for a
  file plainly sitting on disk, and the page then rendered every reference as a
  bare BibTeX key. Containment refusals now say so and name which boundary was hit.

- **SIGHUP no longer leaks the kernel subtree.** `shutdown_signal` raced only
  SIGINT and SIGTERM, so closing a terminal tab took the preview down via
  SIGHUP's default disposition, skipping the teardown that reaps the Jupyter
  kernel and its forkserver children. SIGHUP now takes the same graceful path.

## [0.2.0] - 2026-06-27

The release-hardening release. Two waves of correctness, accessibility, and
authoring-trust work, each landed behind a full test + adversarial-review gate.
The throughline: **a green `taliesin check` (and build) should mean
publishable.**

### Added

- **`taliesin check` is now a real pre-publish gate.** Static, kernel-free, and
  deterministic, it flags (each click-to-source): broken internal/relative
  links and cross-page anchors, missing local video files, dangling `//| input`
  names and reactive-graph cycles, and a built-in **accessibility audit**
  (`validate_a11y`): heading-level skips, `<img>` missing `alt`, and
  `<a>`/`<button>` with no accessible name. `--format json` stays valid JSON
  even on its own errors (an unreadable path emits `{"error": ...}` on stdout),
  so it pipes cleanly into `jq` and CI.
- **Onboarding.** `taliesin init [dir]` scaffolds a minimal previewable site; a
  README install/prerequisites section; per-subcommand `--help` (focused
  synopsis + flags + example for `preview`/`build`/`check`/`render`/`schema`/
  `blocks`/`init`); unknown-command did-you-mean; the top-level usage now
  advertises the `<dir>` site mode.
- **Accessibility (rendered output).** Distinguishing `aria-label`s on every nav
  landmark (primary nav, book chapters, pager, table of contents), one
  consistent `:focus-visible` ring, deck slide roles + "Slide N of M" with a
  polite live region, `forced-colors`/`prefers-contrast` styles, and a
  server-side skip-to-content link + focusable `<main>` + real image `alt`
  (previously only present after JavaScript ran).
- **Citations.** LaTeX accents render as Unicode (Müller, Erdős), brace-
  protected corporate authors stay whole (e.g. `{World Health Organization}`),
  `@string` macros resolve, and `@inbook`/`@incollection` render their
  `booktitle` + pages. A manual `# References` section suppresses the
  auto-generated one.
- **`{{< video >}}`** accepts query strings in the source (`clip.mp4?token=…`),
  and figures honor `height=`.
- A `_quarto.yml`-only directory now gets a migration breadcrumb instead of a
  confusing "no `_site.yml`".

### Changed

- **Mermaid offline behavior.** A diagram whose library can't load now shows a
  visible `[data-mermaid-error]` banner (with the source below) instead of
  failing silently; the library URL is configurable via `TALIESIN_MERMAID_URL`
  for a fully self-hosted, offline build.
- Site `build` now **honors an author's `404.qmd`** (it is no longer overwritten
  by the built-in template, and is kept out of the search index).
- The build "kernel unavailable" hint names the right interpreter
  (`TALIESIN_R` for an R cell, not always `TALIESIN_PYTHON`).
- `THIRD_PARTY.md` now gives an accurate offline/CDN inventory.

### Fixed

- The cross-page link checker no longer false-flags intra-site links that carry
  a `?query` string.
- `taliesin init` (current directory) prints a runnable preview hint
  (`taliesin preview .`).

### Internal

- `cargo clippy` is warning-free; `cargo fmt` and the full `cargo test` suite
  (core + server, including the corpus invariant + no-false-positive guards)
  are green. Each batch was landed via isolated worktree lanes and verified by
  an adversarial pre-merge review.

### Known limitations

> **All four are resolved since this release** and are kept only as the record of
> what 0.2.0 shipped with. Current state: the `_extensions/` showcase no longer
> exists in the tree; `.github/workflows/` holds `ci.yml` and `release.yml`, restored
> 2026-07-28, but every `ci.yml` job is guarded on `repository.private != true`, so
> while this repository is private CI is inert and `.githooks/pre-push` is the only
> gate that runs automatically (`./tools/gates.sh` runs everything, by hand);
> Mermaid is vendored and inlined, so nothing fetches it; warm
> pages are evicted by a `MAX_WARM_PAGES` LRU, and a book has no sidebar to stack
> — navigation is a sticky topbar plus an off-canvas drawer.

- The bundled `_extensions/` showcase (liquid-glass) is currently non-functional
  against the native deck engine; an extension-ecosystem audit is the next
  dedicated pass.
- CI runs `fmt`/`test`/`clippy` plus a weekly `cargo-audit` advisory scan
  (`.github/workflows/ci.yml`); `cargo-deny` is configured (`deny.toml`) but its
  CI step is not wired yet.
- Mermaid still loads from a CDN by default (now configurable and non-silent,
  but not yet vendored).
- Long-running previews can grow memory unboundedly (visited pages are not
  evicted); the book sidebar stacks rather than drawers at ~900px (laptop
  portrait).

## [0.1.0]

Baseline: a fully native `.qmd` → HTML dev server (no legacy compat shims, no
reveal.js, no Observable runtime). All four output shapes render and deploy:
blog post, slide deck (Taliesin's own engine), book, and multi-page site, with a
warm Jupyter kernel, block-level incremental updates with DOM-state
preservation, a `_freeze` execution cache, click-to-source, reverse cursor sync,
located diagnostics, and Cmd-K search.

[0.2.0]: https://github.com/AJBogo9/taliesin/releases/tag/v0.2.0
[0.1.0]: https://github.com/AJBogo9/taliesin/releases/tag/v0.1.0
