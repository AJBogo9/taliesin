# Third-party components

Taliesin itself is MIT licensed (see [LICENSE](LICENSE)). It builds on the
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
- **Mermaid** (`crates/core/assets/js/mermaid.min.js`, MIT, v11.4.1, Copyright (c)
  2014-2024 Knut Sveidqvist). The diagram engine. Inlined into a static build page
  that has a diagram so it renders fully offline (content-gated); the live preview
  lazy-loads it instead (see the runtime note below). License:
  <https://github.com/mermaid-js/mermaid/blob/develop/LICENSE>.
- **GitHub Octicons** (MIT, Copyright (c) GitHub, Inc.). A handful of inline SVG
  glyph paths are embedded directly in source — the copy/check button in
  `code-enhance.js` and the callout-kind icons in `crates/core/src/render/divs.rs`.
  No Octicons package is bundled; only individual path data. License:
  <https://github.com/primer/octicons/blob/main/LICENSE>.

The other scripts under `crates/core/assets/js/` (`code-enhance.js`, `deck.js`,
`mermaid.js`, `qmd-js.js`, `walkthrough.js`, `tabset.js`) are Taliesin's own (MIT).

## Loaded at runtime from a CDN (live preview only)

Taliesin itself fetches nothing over the network in a **static build** — Mermaid is now
vendored (above) and inlined into build pages that have a diagram, so a `--out` doc/book
is fully offline. The one runtime fetch remaining is in the **live preview**:

- **Mermaid loader.** In preview, `mermaid.js` lazy-loads the ~2.5 MB Mermaid library the
  first time a diagram renders, from the pinned jsDelivr build (`TALIESIN_MERMAID_URL`
  overrides the URL — e.g. to the vendored copy for offline dev). Inlining 2.5 MB on every
  save would bloat the preview payload, so preview keeps the lazy loader; the *build* path
  inlines it. When the library cannot load (offline or blocked), the diagram is replaced by
  a **visible** `[data-mermaid-error]` banner with the source kept below, never a silent blank.

Note that **author content can introduce its own CDN dependencies** outside
Taliesin's control: a `{js}` cell may `import` from a CDN (e.g. the corpus
`three-scene` demo imports `three` from esm.sh), and a project's `_site.yml` may add
its own `<link rel="preconnect">` or `<script>`. Those are the author's choices, not
shipped by Taliesin; only the Mermaid loader above is emitted by the tool.

## Build dependencies

The Rust crates in `Cargo.lock` (comrak, axum, tokio, syntect, etc.) are fetched
by Cargo at build time under their own licenses (predominantly MIT, Apache-2.0,
and ISC). They are not redistributed in this repository. `deny.toml` pins the
allowed license set; run `cargo deny check` to verify them (CI wiring is a
deferred follow-up).
