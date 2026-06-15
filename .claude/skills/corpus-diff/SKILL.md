---
name: corpus-diff
description: Compare qmd-fast's render output against the Quarto baselines in corpus/expected/ and report structural divergence. Use when refining the renderer to match Quarto's HTML format, hunting render regressions, or investigating why a corpus doc renders differently than expected.
---

# Corpus render-and-diff

The corpus is the spec. `corpus/expected/*.html` is **Quarto's** HTML for each
doc, kept as a *structural reference, not a byte-exact oracle* (see
corpus/README.md). Cosmetic diffs (whitespace, attribute order, Quarto's class
names, nav/sidebar/TOC chrome, vendored libs) are expected and are **not**
failures. The job is structural parity of the content: same blocks, same order,
same element types, nothing dropped.

## Run it

```sh
python3 scripts/corpus_diff.py --all            # count summary for every doc
python3 scripts/corpus_diff.py <doc-key>        # summary + structural diff for one
python3 scripts/corpus_diff.py --list           # show the doc keys
```

Doc keys: `born-machines`, `em-algorithm`, `pca-geometry`, `liquid-glass`,
`bayesian-book`. (Mapping is explicit in the script: e.g. `example.qmd` →
`liquid-glass.html`.)

The script renders via `target/debug/qmd-fast` if built, else falls back to
`cargo run`. Build first (`cargo build -p qmd-fast-server`) to keep it fast.

## Read the output

1. **Count table** per doc: element kinds (headings, paragraphs, code blocks,
   lists, tables, figures, callouts, references) for qmd-fast vs Quarto. Rows
   marked `<-- differs` are where to look first. A count gap is the cheapest
   signal: "figures 0 vs 7" means figure handling is missing entirely.
2. **Structural diff** (single-doc mode): a unified diff of the normalized block
   skeletons, `-` Quarto / `+` qmd-fast. Each line is `tag.semantic-class: text`.
   This pinpoints *where* in document order the divergence is.

## The refinement loop

1. `--all` to find the doc/element with the biggest gap.
2. Drill into that doc: `python3 scripts/corpus_diff.py <key>`.
3. Read the actual HTML on both sides for the diverging block:
   `target/debug/qmd-fast render <qmd>` vs `corpus/expected/<key>.html`.
   Use `qmd-fast blocks <qmd>` to map a block back to its source position.
4. Fix the renderer in `crates/core` (render.rs / math.rs / cite.rs / etc.).
5. Re-run the single-doc diff to confirm the gap closed, then `cargo test -p
   qmd-fast-core` to confirm no invariant regressed.

## Caveats baked into the baselines

- `bayesian-book.html` and the OJS/Three.js demos were rendered without warm
  execution, so some computed figures are absent or differ. Treat missing
  *computed* outputs as known, not as renderer bugs (until Phase 4 execution).
- The skeleton intentionally ignores `<script>`, `<style>`, `<nav>`, and
  non-semantic wrapper `<div>`s. If you need to compare those, read the raw HTML.

## Extending

New corpus doc: add it to the `DOCS` map in `scripts/corpus_diff.py` and drop its
Quarto render into `corpus/expected/<key>.html` (see "Regenerating a snapshot" in
corpus/README.md). New semantic class to track: extend `KEEP_CLASS`.
