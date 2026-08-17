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

## What you verify by eye

**Three of these nineteen projects are the ones a person looks at**, and they are not chosen
by taste: they are the three exhibits `tools/publish.sh` deploys to gallery.taliesin.sh, which
is where a defect is public. `tarn/` at `/tarn`, `descent/` at `/descent`, `analyst/` at
`/analyst`. Plus one post out of `tech-blog/`, sampled rather than swept, because 19
near-identical posts do not each earn an eyeball.

Everything else is checked by machine, and looking at it is wasted effort rather than diligence:
the diagnostics documents produce a warnings list and not a page, `layout/structure.tmd` pins
`data-section-end`, `native-tmd.tmd` exercises the walker. The `Pass` column below answers this
per document.

**The column is derived, not maintained.** `the_readme_marks_the_same_visual_set_the_deploy_ships`
reads the `GALLERY_EXHIBITS` block out of `tools/publish.sh`, so publishing a new exhibit moves
that project into the visual pass whether or not anyone remembers to, and a new corpus project
with no row here fails the test rather than sitting unclassified. `tech-blog/` is the one hand-named member:
it is human-facing and deliberately not deployed, so no script can derive it.

This thins nothing. All 82 documents still render under the sweeps above, and the two that get
stronger with every document added are untouched. The column governs a person's attention, not
the regression net.

## The documents

| Path | Pass | What it is |
|---|---|---|
| `tech-blog/` | eye | The author's real deployed blog (19 docs): `_site.yml`, navbar/footer, listings, the `hero:` homepage header, `.tmd`→`.html` link rewriting, a `logo:` brand image, a draft post, and the three heavy posts (`em-algorithm` math, `pca-geometry` `{js}`+Three.js, `fourier-transform` Python→`{js}` bridge + raw `{=html}` audio). The only realistic multi-page workload in the suite. |
| `demo-book/` | machine | The book format, purpose-built and small: `chapters:` with a `part:` and `{ file:, text: }` label overrides, chapter and section numbering, prev/next, a draft appendix, a brand logo in both book slots, and chapter-scoped numbering for **two** float kinds (figures and display equations) with cross-page refs to each. |
| `tarn/` | eye | A larger book: Guide + API **Reference** parts including a nested one, deep install subsections, cross-page guide→reference links, and a full-text Cmd-K index that spans the whole book and carries each record's chapter number and heading path. Pinned by `tarn.rs`; the marketing site's `/gallery/tarn` exhibit. |
| `analyst/` | eye | A two-page computational report that **executes**: `{python}` cleans and charts one committed CSV, and one table counter is shared by the authored `: caption {#tbl-}` path and the executed `#\| label: tbl-` path in document order, with cross-page refs to cell-produced floats. `/gallery/analyst`. |
| `descent/` | eye | A single-page explorable explanation: `{{< input >}}` sliders driving a draggable `{js}` graphic, a select-driven scene walk, a reactive Observable Plot chart, math, and two numbered theme-adaptive SVG figures. `/gallery/descent`. |
| `single-page-report/` | machine | One page assembled from seven `subsections/` includes, with cross-refs, a bibliography, `toc: true`, and document-order figure numbering across both cell figures and labelled image figures. |
| `diagnostics/` | machine | Six documents that deliberately trip the validators: typo'd / unguessable / inert keys (`typos.tmd`), the static check-superset (`check-superset.tmd`), widget shapes, links, refs and a11y. Exempt from the clean-vocabulary sweeps. |
| `agent/executed-read.tmd` | machine | The lone-document fixture: `standalone_document_chrome.rs` (preview and build must agree on a single document's page chrome) and the project-refusal message both name it. |
| `posts/born-machines.tmd` | machine | Pure prose, no math and no code: the simplest render target in the tree. |
| `posts/cite-coverage/` | machine | `.bib` edge cases: LaTeX accents → Unicode, a brace-protected corporate author, `@string` substitution, `@incollection` `booktitle`+`pages`, and a manual `# References` heading suppressing the generated one. |
| `shared-bib/` | machine | A project-wide `bibliography:` in `_site.yml`, merged **under** a page's own `.bib` (same key → the page wins). |
| `structured-authors/` | machine | Structured `author:` front matter with superscript numbers derived from first appearance, plus the generated Author Contributions appendix. |
| `layout/` | machine | `structure.tmd` (every heading shape `data-section-end` must survive, including an empty section and a final one followed by generated furniture), `escapes.tmd` (the three width escapes), `dense-output.tmd` (the three overflow shapes, and the raw-HTML root that opens in one block and closes in a later one). |
| `media/` | machine | A `layout-ncol` figure grid, intrinsic `width`/`height` read from the file with the LCP exception, and a theme-adaptive figure. |
| `callouts/kinds.tmd` | machine | All 3 callout kinds: the 2px left rule and the kind word, an authored `title=` staying in the author's voice, and a `collapse="true"` fold. |
| `nested-cells.tmd` | machine | One executable cell per container kind (callout, grid column, width escape, two deep), pinning the output slots' order and depth. |
| `reactive/` | machine | The `{js}` graph: `//\| viewof`/`name`/`input` chains, the `{{< input >}}` control set, and the error box a throwing cell shows. |
| `reader/` | machine | Read-only reader enhancers: pre-paint theme, display preferences, the TOC scrollspy. |
| `highlight.tmd` | machine | Per-language `tali-hl-` scope classes, including the highlighted-but-never-executed languages. Why `ts` and `toml` are the load-bearing cases is in `highlight_langs.rs`. |
| `native-tmd.tmd` | machine | `.tmd` is the native **and only** source extension: the walker, the lint and the link rewrite recognize it, and a stray `.qmd` is not a source document. |
| `recipes/`, `render-fixes/` | machine | A data-to-figure recipe, and the regression page for individually-fixed render defects. |

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
