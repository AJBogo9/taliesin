# Corpus

Real documents that serve as the specification for `taliesin`. "Done" means these render
correctly. `crates/core/tests/corpus.rs` renders every one of them and asserts the
load-bearing invariants (every block carries a unique `data-block-id` and a valid
`data-sourcepos`, blocks stay in document order, includes resolve when a document is
built alone, and the reverse sourcepos scan that click-to-source runs is total). Those two
sweeps get **stronger with every document added and weaker with every one removed**, so
they are never thinned ahead of a feature.

## The keep rule

**A corpus document earns its place by being something a person wanted to read, or by
being a golden no unit test can hold.** A witness for one feature belongs in
`crates/core/src/render/tests.rs`, not here.

This replaces the old rule ("each new capability ships pinned by a target corpus
document added in the same change"), which made the corpus circular as evidence: an
instrument pointed at documents written to exercise features reported that every feature
was used, which is true by construction and therefore says nothing.

**Ordering, and it is the one that bites:** a pin and its docs page are deleted in the
**same commit** as their feature, never before. A document deleted ahead of the code it
guards leaves that code unguarded while every gate still passes, because the sweeps
iterate over whatever exists.

## The documents

| Path | What it is |
|---|---|
| `tech-blog/` | The author's real deployed blog (19 docs): `_site.yml`, navbar/footer, listings, the `hero:` homepage header, `.tmd`→`.html` link rewriting, a `logo:` brand image, a draft post, and the three heavy posts (`em-algorithm` math, `pca-geometry` `{js}`+Three.js, `fourier-transform` Python→`{js}` bridge + raw `{=html}` audio). The only realistic multi-page workload in the suite. |
| `demo-book/` | The book format, purpose-built and small: `chapters:` with a `part:` and `{ file:, text: }` label overrides, chapter and section numbering, prev/next, a draft appendix, a brand logo in both book slots, and chapter-scoped numbering for **two** float kinds (figures and display equations) with cross-page refs to each. |
| `tarn/` | A larger book: Guide + API **Reference** parts including a nested one, deep install subsections, cross-page guide→reference links, and a full-text Cmd-K index that spans the whole book and carries each record's chapter number and heading path. Pinned by `tarn.rs`; the marketing site's `/gallery/tarn` exhibit. |
| `analyst/` | A two-page computational report that **executes**: `{python}` cleans and charts one committed CSV, and one table counter is shared by the authored `: caption {#tbl-}` path and the executed `#\| label: tbl-` path in document order, with cross-page refs to cell-produced floats. `/gallery/analyst`. |
| `descent/` | A single-page explorable explanation: `{{< input >}}` sliders driving a draggable `{js}` graphic, a select-driven scene walk, a reactive Observable Plot chart, math, and two numbered theme-adaptive SVG figures. `/gallery/descent`. |
| `single-page-report/` | One page assembled from seven `subsections/` includes, with cross-refs, a bibliography, `toc: true`, and document-order figure numbering across both cell figures and labelled image figures. |
| `diagnostics/` | Six documents that deliberately trip the validators: typo'd / retired / inert keys (`typos.tmd`), the static check-superset (`check-superset.tmd`), widget shapes, links, refs and a11y. Exempt from the clean-vocabulary sweeps. Strictly more valuable as the retirement registers grow. |
| `agent/executed-read.tmd` | The lone-document fixture: `standalone_document_chrome.rs` (preview and build must agree on a single document's page chrome) and the project-refusal message both name it. |
| `posts/born-machines.tmd` | Pure prose, no math and no code: the simplest render target in the tree. |
| `posts/cite-coverage/` | `.bib` edge cases: LaTeX accents → Unicode, a brace-protected corporate author, `@string` substitution, `@incollection` `booktitle`+`pages`, and a manual `# References` heading suppressing the generated one. |
| `shared-bib/` | A project-wide `bibliography:` in `_site.yml`, merged **under** a page's own `.bib` (same key → the page wins). |
| `structured-authors/` | Structured `author:` front matter with superscript numbers derived from first appearance, plus the generated Author Contributions appendix. |
| `layout/` | `structure.tmd` (every heading shape `data-section-end` must survive, including an empty section and a final one followed by generated furniture), `escapes.tmd` (the three width escapes), `dense-output.tmd` (the three overflow shapes, and the raw-HTML root that opens in one block and closes in a later one). |
| `media/` | A `layout-ncol` figure grid, intrinsic `width`/`height` read from the file with the LCP exception, and a theme-adaptive figure. |
| `callouts/kinds.tmd` | All 3 callout kinds with bundled icons, `appearance=` and `icon="false"`. |
| `nested-cells.tmd` | One executable cell per container kind (callout, grid column, width escape, two deep), pinning the output slots' order and depth. |
| `reactive/` | The `{js}` graph: `//\| viewof`/`name`/`input` chains, the `{{< input >}}` control set, and the error box a throwing cell shows. |
| `reader/` | Read-only reader enhancers: pre-paint theme, display preferences, the TOC scrollspy. |
| `highlight.tmd` | Per-language `tali-hl-` scope classes, including the highlighted-but-never-executed languages. Why `ts` and `toml` are the load-bearing cases is in `highlight_langs.rs`. |
| `native-tmd.tmd` | `.tmd` is the native **and only** source extension: the walker, the lint and the link rewrite recognize it, and a stray `.qmd` is not a source document. |
| `theme-css/` | `theme: brand.css` read relative to the document, and the `_extensions/<name>/theme.css` bundle branch resolved by bare name. |
| `recipes/`, `render-fixes/` | A data-to-figure recipe, and the regression page for individually-fixed render defects. |

## Two things worth knowing before you edit

**The Three.js page needs the network, deliberately.**
`tech-blog/_includes/three-scene.tmd` `import()`s three.js, `OrbitControls` and
`GLTFLoader` from `https://esm.sh` at *view* time, so `pca-geometry` does not draw its 3D
figures offline, and the build says so per page. This is the corpus's own authored `{js}`,
not something the tool injects, and it is the one place the corpus depends on a third
party at view time. Taliesin vendors d3, Observable Plot and mermaid for exactly this
reason, so vendoring three.js is the obvious symmetry. It is left undone on purpose, because it
is ~600 kB plus an ESM bundling step, a `THIRD_PARTY.md` entry and a `third_party.rs`
version pin, which is a product decision rather than a bug fix.

**Every document must resolve its includes when built *alone*.**
A single invoked document is confined to its own directory unless an `_site.yml` above it
declares a wider project boundary (PT-2, see `crates/core/tests/include_root_parity.rs`).
`every_corpus_doc_resolves_its_includes_when_built_alone` sweeps every document through
the single-file entry point, because a named-page test cannot see the document nobody
named, which is exactly how a `../../` include rotted here once.
