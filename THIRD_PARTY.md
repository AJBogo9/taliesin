# Third-party components

Taliesin itself is licensed under the GNU Affero General Public License v3.0
(AGPL-3.0-only, see [LICENSE](LICENSE)); the VS Code editor companion under
[`editor/vscode`](editor/vscode) is a separate work licensed under the MIT
License. It builds on the following third-party work.

## Vendored (redistributed with this project)

Bundled so the tool works fully offline.

MIT and ISC both require the **permission notice** — not just the copyright line — to
be included in every copy, and the minified bundles carry at most a one-line header.
The verbatim texts therefore ship beside the files they cover:
[`crates/core/assets/js/LICENSES.md`](crates/core/assets/js/LICENSES.md) (d3, Observable
Plot, Mermaid + the dependencies Mermaid inlines) and
[`crates/core/assets/katex/LICENSE`](crates/core/assets/katex/LICENSE) (KaTeX).
The fonts carry theirs too: `assets/fonts/literata-OFL-fontsource.txt` and
`assets/fonts/jetbrains-mono-OFL-fontsource.txt`, each the exact license text bundled
with the upstream `@fontsource-variable` package the woff2 bytes were subset from
(OFL 1.1 §2 requires the license to accompany every copy; the vendored woff2 binaries
carry no embedded `name[13]`/`name[14]` notice, so this file is the only copy that
travels with them).

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
  glyph paths are embedded directly in source — the copy/check button in the
  `code-enhance/` fragments and the callout-kind icons in
  `crates/core/src/render/divs.rs`.
  No Octicons package is bundled; only individual path data. License:
  <https://github.com/primer/octicons/blob/main/LICENSE>.
- **Literata** — SIL OFL 1.1, no Reserved Font Name, `@fontsource-variable/literata@5.2.8`.
  `Copyright 2017 The Literata Project Authors (https://github.com/googlefonts/literata)`
  Subset by `tools/subset-fonts.sh`. License text:
  `crates/core/assets/fonts/literata-OFL-fontsource.txt`.
- **JetBrains Mono** — SIL OFL 1.1, no Reserved Font Name,
  `@fontsource-variable/jetbrains-mono@5.2.8`.
  `Copyright 2020 The JetBrains Mono Project Authors (https://github.com/JetBrains/JetBrainsMono)`
  Subset by `tools/subset-fonts.sh`, `calt` removed. License text:
  `crates/core/assets/fonts/jetbrains-mono-OFL-fontsource.txt`.

The other scripts under `crates/core/assets/js/` (`mermaid.js`, `tali-js.js`, and the
`code-enhance/` fragments) are Taliesin's own, under the project's AGPL-3.0-only license.

## Loaded at runtime

**Taliesin fetches nothing over the network, in any mode.** Every mode uses the same
vendored copy of Mermaid and differs only in where it puts it: `build doc.tmd` inlines it
into the single file it writes; `build doc.tmd --out <dir>` writes it beside the page as
`mermaid.min.js`; `build <dir>` writes one shared `_assets/mermaid.<hash>.js`; and the live
preview lazy-loads it from a **same-origin** route (`/_taliesin/mermaid.min.js`, served
straight out of the binary). In every case a page without a diagram gets none of it.
Preview uses a route rather than inlining because the page shell is re-served on every
navigation, so a route is fetched once and then cached, and it keeps working when a
document gains its first diagram mid-session.

- `TALIESIN_MERMAID_URL` overrides that URL if you want the library from somewhere else.
  Setting it to a CDN is the only way to make Taliesin reach the network for its own assets.
- When the library cannot load at all, the diagram is replaced by a **visible**
  `[data-mermaid-error]` banner with the source kept below, never a silent blank.
- Mermaid is initialised with an explicit `securityLevel: 'strict'` rather than whatever the
  library's default happens to be, so an upgrade cannot silently loosen the sanitiser.

Note that **author content can introduce its own CDN dependencies** outside
Taliesin's control: a `{js}` cell may `import` from a CDN, and a project's `_site.yml`
may add its own `<link rel="preconnect">` or `<script>`. The `three-scene` include does
the first of those, importing `three` from esm.sh, and it is not confined to the corpus
— this repository's own marketing site and guide use it too
(`site/_includes/three-scene.tmd`, `corpus/tech-blog/_includes/three-scene.tmd`,
`docs/guide/using/code.tmd`, `docs/guide/using/interactive.tmd`). Those are the author's
choices, not shipped by Taliesin; only the Mermaid loader above is emitted by the tool.
No three.js bytes are redistributed under `crates/core/assets/`, which is why there is
no attribution row for it above: nothing here is a copy, so no notice obligation
attaches.

## Build dependencies

The Rust crates in `Cargo.lock` (comrak, axum, tokio, syntect, etc.) are fetched
by Cargo at build time under their own licenses (predominantly MIT, Apache-2.0,
and ISC). They are not redistributed in this repository. `deny.toml` pins the
allowed license set. Run `cargo deny check` on any dependency change: the
`.githooks/pre-push` gate covers fmt/clippy/test only, and the restored workflow is
inert while this repo is private. `./tools/gates.sh` runs it with every other gate.

One build dependency embeds third-party *data* into the compiled binary rather
than only linking code:

- **two-face** (MIT OR Apache-2.0). Supplies the syntax definitions syntect's
  bundled set omits (TypeScript and TOML, both of which the docs use) as a
  ~900 KB compiled dump linked into the binary. The definitions are curated by
  the [`bat`](https://github.com/sharkdp/bat) project and each retains its own
  upstream license; `cargo deny` checks the crate's license, not theirs. The full
  per-syntax attribution listing ships with the crate and is reachable at runtime
  via `two_face::acknowledgement::listing()`.

Those two sets are the whole of the highlighting coverage: **no syntax definition is
vendored as source**, and a fence in a language neither set carries renders as plain
escaped text rather than reaching for a third grammar.

