# Third-party components

qmd-fast itself is MIT licensed (see [LICENSE](LICENSE)). It builds on the
following third-party work.

## Vendored (redistributed with this project)

- **KaTeX** — MIT License, Copyright (c) 2013-2020 Khan Academy and other
  contributors. The stylesheet and WOFF2 fonts under `crates/core/assets/katex/`
  are bundled so math renders offline. Full license:
  <https://github.com/KaTeX/KaTeX/blob/main/LICENSE>.

## Loaded at runtime from a CDN (not redistributed here)

Used only by the generated preview pages / decks, fetched from jsDelivr in the
browser:

- **reveal.js** — MIT License (slide decks).
- **highlight.js** — BSD 3-Clause License (code syntax highlighting).
- **mermaid** — MIT License (diagrams).

## Build dependencies

The Rust crates in `Cargo.lock` (comrak, axum, tokio, etc.) are fetched by Cargo
at build time under their own licenses (predominantly MIT and Apache-2.0). They
are not redistributed in this repository.

## Note on Quarto

qmd-fast is an independent reimplementation of a subset of Quarto's `.qmd` ->
HTML behavior, not a copy of Quarto's source. "Quarto" is a trademark of its
owner; this project is not affiliated with or endorsed by it.
