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
| `posts/fourier-transform/` | Interactive blog post | `ojs_define` Python→OJS bridge, raw-HTML (`{=html}`) audio players, labelled equations (`@eq-`) | `personal/tech-blog` |
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

`crates/core/tests/tech_blog.rs` tracks progress toward using qmd-fast as the
edit-preview loop for the author's tech-blog: passing tests lock in the per-post
features (math, callouts, citations, raw-`{=html}` passthrough, numbered/labelled
equations + `@eq-` refs, collapsible callouts, `code-fold`, and live Observable
cells). The remaining `#[ignore]`d tests encode the website-scope `listing:`/
`about:` features. Run `cargo test --test tech_blog -- --ignored` for those.

Live OJS needs a real web server (the Observable runtime rejects `file://`), so it
is verified by `serve` + browser rather than a unit test; see `todo.md` for the
OJS design + known follow-ups.
