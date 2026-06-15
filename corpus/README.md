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

## `expected/` — reference snapshots, NOT a byte-exact oracle

`expected/*.html` is Quarto's current HTML output for each doc, kept as a
**structural reference**. Cosmetic diffs (whitespace, attribute order, class
names) are expected and must not be treated as failures.

**Local-only (gitignored).** These snapshots are large and regenerable, so they
are not committed (`corpus/expected/*.html` is in `.gitignore`). They live here
only as a local baseline for the `corpus-diff` skill. Regenerate them with the
command below after a fresh clone.

Caveats:

- **HTML only, no vendored libs.** The accompanying `*_files/` lib dirs (Bootstrap,
  MathJax, etc.) are intentionally not committed — the snapshots are for
  structural diffing, not standalone viewing, so their external lib references
  do not resolve.
- `em-algorithm.html` and `pca-geometry.html` were reused from the author's
  existing `tech-blog/_site` render (kernels already warm).
- `bayesian-book.html` was rendered with `--no-execute`: structure + code are
  present, but computed cell outputs (figures) are not. Upgrade once Phase 4
  execution lands. The book's `data/` dir was omitted for the same reason; a
  future full render will need it.

## Regenerating a snapshot

```sh
quarto render corpus/posts/born-machines.qmd --to html
# then move the .html into corpus/expected/ and discard the *_files/ dir
```
