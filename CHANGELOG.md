# Changelog

All notable changes to Taliesin are recorded here. This project follows a
loose [semantic versioning](https://semver.org/) while pre-1.0: minor versions
may carry new features and small behavior changes; the load-bearing invariants
(content-hash block model, click-to-source, single editing surface, HTML-only
output) are kept stable.

## [Unreleased]

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
