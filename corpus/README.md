# Corpus

Real documents that serve as the specification for `qmd-fast`. "Done" means
these render correctly (judged by inspection). Each doc was copied from the
author's own projects; provenance is below.

## Documents

| Path | Category | Exercises | Source |
|---|---|---|---|
| `posts/born-machines.qmd` | Prose blog post | pure prose (no math/code) — the simplest Phase 1 target | `personal/blog` |
| `posts/em-algorithm/` | Math blog post | heavy KaTeX (~100 math spans), 6 code cells, OJS | `personal/tech-blog` |
| `posts/pca-geometry/` | Live-demo blog post | OJS + Three.js + math + code | `personal/tech-blog` |
| `liquid-glass-slides/example.qmd` | reveal.js deck | slide structure, custom `liquid-glass` format extension | `personal/liquid-glass-revealjs` |
| `bayesian-book/` | Multi-file book | includes (`subsections/`), cross-refs, bib + CSL, TOC | `personal/bayesian-fatality-analysis` |

`posts/pca-geometry/index.qmd` pulls in `_includes/three-scene.qmd` via
`{{< include ../../_includes/three-scene.qmd >}}`; the `posts/` + `_includes/`
layout is mirrored from the source project so that path resolves verbatim.

## How the corpus is used

`crates/core/tests/corpus.rs` renders every doc here and asserts the
load-bearing invariants (each block has an id + valid sourcepos, ids unique,
blocks in document order, includes resolved, decks split into slides, the book
gets a TOC + numbered figures). These are the project's regression tests, so the
corpus must stay.

Structural comparison against **Quarto** (rendering the same doc with both and
diffing) lives in the separate `qmd-fast-testbed` repo, not here.
