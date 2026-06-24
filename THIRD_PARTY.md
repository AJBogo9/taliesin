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

The other scripts under `crates/core/assets/js/` (`code-enhance.js`, `deck.js`,
`mermaid.js`, `qmd-js.js`) are qmd-fast's own (MIT).

## Loaded at runtime from a CDN (not redistributed here)

- **Mermaid** (MIT, diagrams). Pulled lazily from jsDelivr by the bundled
  `mermaid.js` loader only on pages that contain a `mermaid` block. This is the
  sole CDN dependency.

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
