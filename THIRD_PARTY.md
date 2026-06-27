# Third-party components

qmd-fast itself is MIT licensed (see [LICENSE](LICENSE)). It builds on the
following third-party work.

## Vendored (redistributed with this project)

Bundled so the tool works fully offline.

- **KaTeX** (MIT, Copyright (c) 2013-2020 Khan Academy and other contributors).
  The stylesheet and WOFF2 fonts under `crates/core/assets/katex/` render math
  offline. License: <https://github.com/KaTeX/KaTeX/blob/main/LICENSE>.
- **D3** (`crates/core/assets/js/d3.min.js`, ISC, v7.9.0, Copyright 2010-2023
  Mike Bostock). The plotting primitive used by `{js}` cells. License:
  <https://github.com/d3/d3/blob/main/LICENSE>.
- **Observable Plot** (`crates/core/assets/js/plot.umd.min.js`, ISC, v0.6.16,
  Copyright 2020-2023 Observable, Inc.). The high-level chart library for `{js}`
  cells; depends on the vendored D3 above. License:
  <https://github.com/observablehq/plot/blob/main/LICENSE>.
- **GitHub Octicons** (MIT, Copyright (c) GitHub, Inc.). A handful of inline SVG
  glyph paths are embedded directly in source — the copy/check button in
  `code-enhance.js` and the callout-kind icons in `crates/core/src/render/divs.rs`.
  No Octicons package is bundled; only individual path data. License:
  <https://github.com/primer/octicons/blob/main/LICENSE>.

The other scripts under `crates/core/assets/js/` (`code-enhance.js`, `deck.js`,
`mermaid.js`, `qmd-js.js`, `walkthrough.js`, `tabset.js`) are qmd-fast's own (MIT).

## Loaded at runtime from a CDN (not redistributed here)

The only library qmd-fast *itself* fetches over the network is the Mermaid diagram
engine, and only on a page that actually has a diagram. Everything else above is
bundled offline.

- **Mermaid** (MIT, diagrams). The bundled `mermaid.js` loader lazy-loads the ~2.8 MB
  Mermaid library the first time a `mermaid` block renders — a client-side
  presentation layer that never touches the block model. The source URL defaults to a
  pinned jsDelivr build but is **configurable**: set the `QMD_FAST_MERMAID_URL`
  environment variable to self-host the library (a relative or absolute URL) for a
  fully offline build. We deliberately do not `include_str!` the 2.8 MB library into
  the binary / every built page to cover this minority case. When the library cannot
  load (offline or blocked), the diagram is replaced by a **visible**
  `[data-mermaid-error]` banner with the source kept below, so the failure is loud and
  diagnosable rather than silent.

Note that **author content can introduce its own CDN dependencies** outside
qmd-fast's control: a `{js}` cell may `import` from a CDN (e.g. the corpus
`three-scene` demo imports `three` from esm.sh), and a project's `_site.yml` may add
its own `<link rel="preconnect">` or `<script>`. Those are the author's choices, not
shipped by qmd-fast; only the Mermaid loader above is emitted by the tool.

## Build dependencies

The Rust crates in `Cargo.lock` (comrak, axum, tokio, syntect, etc.) are fetched
by Cargo at build time under their own licenses (predominantly MIT, Apache-2.0,
and ISC). They are not redistributed in this repository. `deny.toml` pins the
allowed license set; run `cargo deny check` to verify them (CI wiring is a
deferred follow-up).

## Note on Quarto

qmd-fast is an independent reimplementation of a subset of Quarto's `.qmd` ->
HTML behavior, not a copy of Quarto's source. "Quarto" is a trademark of its
owner; this project is not affiliated with or endorsed by it.
