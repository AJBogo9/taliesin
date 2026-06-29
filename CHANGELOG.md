# Changelog

All notable changes to qmd-fast are recorded here. This project follows a
loose [semantic versioning](https://semver.org/) while pre-1.0: minor versions
may carry new features and small behavior changes; the load-bearing invariants
(content-hash block model, click-to-source, single editing surface, HTML-only
output) are kept stable.

## [0.2.0] - 2026-06-27

The release-hardening release. Two waves of correctness, accessibility, and
authoring-trust work, each landed behind a full test + adversarial-review gate.
The throughline: **a green `qmd-fast check` (and build) should mean
publishable.**

### Added

- **`qmd-fast check` is now a real pre-publish gate.** Static, kernel-free, and
  deterministic, it flags (each click-to-source): broken internal/relative
  links and cross-page anchors, missing local video files, dangling `//| input`
  names and reactive-graph cycles, and a built-in **accessibility audit**
  (`validate_a11y`): heading-level skips, `<img>` missing `alt`, and
  `<a>`/`<button>` with no accessible name. `--format json` stays valid JSON
  even on its own errors (an unreadable path emits `{"error": ...}` on stdout),
  so it pipes cleanly into `jq` and CI.
- **Onboarding.** `qmd-fast init [dir]` scaffolds a minimal previewable site; a
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
  failing silently; the library URL is configurable via `QMD_FAST_MERMAID_URL`
  for a fully self-hosted, offline build.
- Site `build` now **honors an author's `404.qmd`** (it is no longer overwritten
  by the built-in template, and is kept out of the search index).
- The build "kernel unavailable" hint names the right interpreter
  (`QMD_FAST_R` for an R cell, not always `QMD_FAST_PYTHON`).
- `THIRD_PARTY.md` now gives an accurate offline/CDN inventory.

### Fixed

- The cross-page link checker no longer false-flags intra-site links that carry
  a `?query` string.
- `qmd-fast init` (current directory) prints a runnable preview hint
  (`qmd-fast preview .`).

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

Baseline: a fully native `.qmd` → HTML dev server (no Quarto compat shims, no
reveal.js, no Observable runtime). All four output shapes render and deploy:
blog post, slide deck (qmd-fast's own engine), book, and multi-page site, with a
warm Jupyter kernel, block-level incremental updates with DOM-state
preservation, a `_freeze` execution cache, click-to-source, reverse cursor sync,
located diagnostics, and Cmd-K search.

[0.2.0]: https://github.com/AJBogo9/qmd-fast/releases/tag/v0.2.0
[0.1.0]: https://github.com/AJBogo9/qmd-fast/releases/tag/v0.1.0
