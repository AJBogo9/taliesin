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
- **Mermaid** (`crates/core/assets/js/mermaid.min.js`, MIT, v11.16.0, Copyright (c)
  2014-2024 Knut Sveidqvist). The diagram engine. Inlined into a static build page
  that has a diagram so it renders fully offline (content-gated); the live preview
  lazy-loads this same vendored copy from a same-origin route. License:
  <https://github.com/mermaid-js/mermaid/blob/develop/LICENSE>.
- **GitHub Octicons** (MIT, Copyright (c) GitHub, Inc.). A handful of inline SVG
  glyph paths are embedded directly in source — the copy/check button in
  `code-enhance.js` and the callout-kind icons in `crates/core/src/render/divs.rs`.
  No Octicons package is bundled; only individual path data. License:
  <https://github.com/primer/octicons/blob/main/LICENSE>.
- **Newsreader** (SIL Open Font License 1.1, Copyright 2020 The Newsreader
  Project Authors). Used two ways. (1) The variable serif TTF at
  `crates/core/assets/fonts/Newsreader[opsz,wght].ttf` is rasterized at build time
  for the headline/byline/footer text on the auto-generated social-card image
  (`crates/core/src/site/card.rs`). (2) The **body typeface** for rendered pages:
  the two variable woff2 faces
  `crates/core/assets/fonts/newsreader-latin-wght-{normal,italic}.woff2` (roman +
  italic, Latin subset, from `@fontsource-variable/newsreader@5.2.10`) are inlined
  as `data:` URIs into every page's CSS at build time (`build.rs`), so pages need no
  network for text. Full license text ships alongside them in
  `crates/core/assets/fonts/OFL.txt` (and `newsreader-OFL-fontsource.txt`). License:
  <https://github.com/productiontype/Newsreader>.

The other scripts under `crates/core/assets/js/` (`code-enhance.js`, `deck.js`,
`mermaid.js`, `tali-js.js`, `walkthrough.js`, `tabset.js`) are Taliesin's own (MIT).

### Sample 3-D models (corpus content)

- **ToyCar** (`corpus/graphics3d/assets/ToyCar.glb`, CC0 1.0 Universal / public
  domain dedication). A glTF sample asset displayed by the "Live 3-D graphics"
  gallery exhibit's CAD page (`corpus/graphics3d/cad.tmd`). Source: Khronos
  glTF-Sample-Assets
  (<https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models/ToyCar>).
  License: <https://creativecommons.org/publicdomain/zero/1.0/>.

## Loaded at runtime

**Taliesin fetches nothing over the network, in any mode.** A static build inlines the
vendored Mermaid into pages that have a diagram, and the live preview lazy-loads that same
vendored copy from a **same-origin** route (`/_taliesin/mermaid.min.js`, served straight out
of the binary). Preview uses a route rather than inlining because the page shell is
re-served on every navigation, so a route is fetched once and then cached, and it keeps
working when a document gains its first diagram mid-session.

- `TALIESIN_MERMAID_URL` overrides that URL if you want the library from somewhere else.
  Setting it to a CDN is the only way to make Taliesin reach the network for its own assets.
- When the library cannot load at all, the diagram is replaced by a **visible**
  `[data-mermaid-error]` banner with the source kept below, never a silent blank.
- Mermaid is initialised with an explicit `securityLevel: 'strict'` rather than whatever the
  library's default happens to be, so an upgrade cannot silently loosen the sanitiser.

Note that **author content can introduce its own CDN dependencies** outside
Taliesin's control: a `{js}` cell may `import` from a CDN (e.g. the corpus
`three-scene` demo imports `three` from esm.sh), and a project's `_site.yml` may add
its own `<link rel="preconnect">` or `<script>`. Those are the author's choices, not
shipped by Taliesin; only the Mermaid loader above is emitted by the tool.

## Build dependencies

The Rust crates in `Cargo.lock` (comrak, axum, tokio, syntect, etc.) are fetched
by Cargo at build time under their own licenses (predominantly MIT, Apache-2.0,
and ISC). They are not redistributed in this repository. `deny.toml` pins the
allowed license set, and CI enforces it (the `deny` job runs `cargo deny check`).

One build dependency embeds third-party *data* into the compiled binary rather
than only linking code:

- **two-face** (MIT OR Apache-2.0). Supplies the syntax definitions syntect's
  bundled set omits (TypeScript and TOML, both of which the docs use) as a
  ~900 KB compiled dump linked into the binary. The definitions are curated by
  the [`bat`](https://github.com/sharkdp/bat) project and each retains its own
  upstream license; `cargo deny` checks the crate's license, not theirs. The full
  per-syntax attribution listing ships with the crate and is reachable at runtime
  via `two_face::acknowledgement::listing()`.

Two further build dependencies power the auto-generated social-card raster
pipeline (`crates/core/src/site/card.rs`), called out individually because they
replace what would otherwise be an external image tool; both are pure Rust with
no C toolchain or system library requirement, which is why they were picked:

- **ab_glyph** (Apache-2.0). Rasterizes glyphs from the bundled Newsreader font
  above into the card's headline/byline/footer text.
- **png** (MIT OR Apache-2.0). Encodes the rendered 1200×630 RGBA card buffer
  to the PNG served at `/og/<hash>.png`.
