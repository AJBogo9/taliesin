# Corpus

Real documents that serve as the specification for `taliesin`. "Done" means
these render correctly (judged by inspection). Each doc was copied from the
author's own projects; provenance is below.

## Documents

| Path | Category | Exercises | Source |
|---|---|---|---|
| `native-tmd.tmd` | Native source extension | authored in `.tmd` (Taliesin's native *and only* source extension); pins that the walker/`check`/link-rewrite recognize `.tmd` and that a stray `.qmd` file is no longer recognized as a source document | (purpose-built) |
| `highlight.tmd` | Syntax-highlighting coverage | server-side `tali-hl-` scope classes per language; pins that `ts`/`toml` highlight (syntect's bundled set has neither) and that `text`/`console`/unlabelled fences stay plain without tripping `validate_code_languages`; also the registry's **highlighted-but-never-executed** languages (`julia`, `sql` beside `bash`/`rust`), which are offered by completion and accepted by `check` yet never reach a kernel | (purpose-built) |
| `theme-css/` | Custom `theme:` resolution | `theme: brand.css` (a sibling stylesheet) is read from disk relative to the document and inlined after the base stylesheet, into the page's `<style id="tali-theme">`; the installed `_extensions/<name>/theme.css` bundle branch resolves by bare name. Pinned by `theme_css.rs` (the file-read + extension-bundle positive paths; the missing-file *warning* is pinned in the render unit tests) | (purpose-built) |
| `posts/born-machines.tmd` | Prose blog post | pure prose (no math/code) — the simplest Phase 1 target | `personal/blog` |
| `posts/em-algorithm/` | Math blog post | heavy KaTeX (~100 math spans), 6 code cells, `{js}` cells | `personal/tech-blog` |
| `posts/pca-geometry/` | Live-demo blog post | `{js}` cells + Three.js + math + code | `personal/tech-blog` |
| `posts/fourier-transform/` | Interactive blog post | `define(...)` Python→`{js}` bridge, raw-HTML (`{=html}`) audio players, labelled equations (`@eq-`) | `personal/tech-blog` |
| `posts/cite-coverage/` | Citation/bibliography fixes | `.bib` rendering edge cases: LaTeX accents → Unicode (Müller/Schölkopf/Erdős/Rényi), brace-protected corporate author, `@string` macro substitution, `@incollection` `booktitle`+`pages`, and a manual `# References` heading suppressing the auto one | (purpose-built) |
| `single-page-report/` | Single-page website | one page assembled from seven `subsections/` includes, cross-refs (`@fig-`, `@sec-`), bib + citations, `toc: true`, a `layout-ncol=2` figure pair, and document-order figure numbering across *both* `#\| fig-cap:` cells (1-3) and labelled image figures (4-6). Pinned by `corpus.rs`'s `website_renders_with_toc_anchored_headings_and_numbered_figures` | (purpose-built) |
| `tech-blog/` | Multi-page website | `_site.yml` project config, many pages + posts, navbar/footer, prev/next, `.tmd`→`.html` cross-page links; `posts/draft-example/` pins **draft-aware preview** (`draft: true` → shown+badged in `preview`, excluded from `build`); `logo: logo.svg` pins the site **brand image** in the navbar (depth-relative on a nested post, `title:` as its alt) | `personal/tech-blog` |
| `demo-book/` | Multi-chapter book | flat native `chapters:` (with a `part:` and `{ file:, text: }` label overrides), left chapter sidebar, chapter + section numbering, prev/next-chapter nav; `appendix.tmd` (last) pins a **draftable chapter** (dropped + renumbered in `build`, marked in `preview`); `logo: logo.svg` pins the **brand image** reaching BOTH book brand slots (sticky topbar + chapter-drawer head) | (purpose-built for the book format) |
| `structured-authors/` | Structured `author:` front matter | the paper page (`paper.tmd`) that pins items 184 and 187 end to end: structured `author:` entries (affiliation, url, `equal`, `contribution`) with the superscript numbers **derived** from first appearance, and the generated appendix (Author Contributions). `note.tmd` pins the site-author byline fallback and `index.tmd` the dateless case. The `orcid:`/`email:` sub-keys and the JSON-LD block that read them were cut on 2026-08-08. Repurposed on 2026-08-03 from `cite-this/`, whose cite-this box, `citation_*` block and `doi`/`venue`/`award`/`links`/`acknowledgments` keys were retired; the structured-author half it also pinned is what survives | (purpose-built) |
| `shared-bib/` | Site-level `bibliography:` | `_site.yml` declares a project-wide `references.bib`; `index.tmd` cites a shared key with **no `bibliography:` of its own**, and `notes.tmd` declares `local.bib` which is merged **over** the shared layer (same key -> the page's entry wins, plus a page-only key). Both pages together cite every shared entry, so the site is clean under both read-only lints. Pinned by `shared_bibliography.rs` | (purpose-built) |
| `layout/escapes.tmd` | Width escapes (item 181) | `::: {.column-page}` + `::: {.column-screen}` on a table, a listing and a bare div, beside a `.column-margin` on the same page (the two ends of one axis). Pinned by `layout_escapes.rs`, which browser-measures every container mode | (purpose-built) |
| `callouts/kinds.tmd` | Callout contract | all 3 callout kinds with bundled icons + `appearance=` (simple/minimal) + `icon="false"` | (purpose-built) |
| `nested-cells.tmd` | Code cells inside `:::` containers (item 210) | one executable cell per container kind — callout, `layout-ncol` grid (two, so each output has to stay in its own column), `.column-page`, a titled callout, and one two containers deep — plus a `{js}` cell that must earn **no** output slot. Pins the render half (`Block::nested`, the `data-tali-out-for` slots, their order and depth) in `crates/core/tests/nested_cells.rs`; that the cells actually *run* is `crates/server/tests/nested_cell_executes.rs`, which needs a kernel and so cannot live here | (purpose-built) |
| `media/gallery.tmd` | Image gallery grid | `layout-ncol` figure grid: several figures side by side, each still numbered, captioned and cross-referenceable on its own | (purpose-built) |
| `media/optimized-images.tmd` | Image annotation | intrinsic `width`/`height` on every local raster image, read from the file rather than guessed, and the LCP exception (first image eager + `fetchpriority`, the rest `loading="lazy"`). Pinned by `image_optimization.rs`. It also pinned the build's AVIF derivation until that was cut on 2026-08-08; the build now copies your bytes across unchanged | (purpose-built) |
| `reactive/graph.tmd` | `{js}` reactive graph | `//| viewof`/`//| name`/`//| input` chains; a slider re-runs only its transitive-downstream closure | (purpose-built) |
| `reactive/inputs.tmd` | `{{< input >}}` controls | declarative reactive controls (slider/number/checkbox/text/select) feeding `{js}` cells through the graph (incl. a transitive chain) | (purpose-built) |
| `reactive/js-error.tmd` | `{js}` cell error state | a throwing `{js}` cell surfaces the `.tali-js-error` box (themed light + dark); pins the runtime-error state for browser verification | (purpose-built) |
| `reader/` | Reader experience | read-only reader enhancers: display prefs (light/dark theme, applied before paint) and a live table-of-contents scrollspy | (purpose-built) |
| `diagnostics/` | Validator coverage | docs that deliberately trip Taliesin's schema validators (`typos.tmd`: unknown top-level, `execute:` and `listing:` keys, which cover the three mistakes that need three different messages — a **typo** gets a did-you-mean, a **retired** name gets a removal note, and a **recognized but inert** key (`csl:`) gets neither) + the `check`-superset static lints (`check-superset.tmd`: duplicate `{#id}`, broken in-page anchor, missing image, citation with no `bibliography:` — and it must stay CELL-FREE, or the anchor check switches itself off) + widget-shape mistakes (`widgets.tmd`: a nameless or unknown-typed `{{< input >}}`) | (purpose-built) |
| `bare-draft.tmd` | Bare build (`--bare`) | prose + inline math + a server-highlighted code block + an image + a `{js}` cell + a Mermaid block; pins the `build --bare` contract (zero `<script>`/zero CDN, CSS-only theme, math kept, `{js}` dropped, Mermaid as source) and the Phase-1 enhancer gating | (purpose-built) |
| `agent/executed-read.tmd` | Standalone-document chrome + the project refusal | a labelled-figure python cell + a printed stream + a deliberately-erroring cell. Pinned `read --run`'s executed-output projection until Wave 2 cut that verb; it survives as the fixture three other suites name — `standalone_document_chrome.rs` (preview and build must agree on a lone document's page chrome), `project_required.rs`, and the `serve/mod.rs` unit tests that assert the refusal message names `taliesin preview corpus/agent/` | (purpose-built) |
| `course/` | Realistic course (demand-probe pilot) | a lecturer's interactive lecture-notes **book**: figures and display equations numbered + cross-referenced **across chapters** (chapter scope over three float kinds), display math, a `{python}` cell, and a draft appendix. The first corpus doc that *stacks* these interactions (single-feature pins never combine them). Pinned by `course.rs`; also the first marketing-site **gallery** exhibit (`/gallery/course`). See `notes/2026-07-22-corpus-demand-probe-course-author.md` | (purpose-built, demand-probe pilot) |
| `tarn/` | Realistic library docs (demand-probe #2) | the documentation site for a small illustrative dataframe library: a **book** with Guide + API **Reference** parts, per-package-manager and per-OS install subsections, version/deprecation callouts, and **cross-page** guide→reference links; full-text Cmd-K search spans the book and indexes each section's body **whole**, so a command two heading levels down is findable from the palette. The first corpus doc to stack deep subsections × search × an API reference. Pinned by `tarn.rs`; the second marketing-site **gallery** exhibit (`/gallery/tarn`). See `notes/2026-07-22-corpus-demand-probe-docs-maintainer.md` | (purpose-built, demand-probe #2) |
| `descent/` | Interactive explainer (demand-probe #3) | a single-page **explorable explanation** of gradient descent that stacks the interactive cluster on one page: three `{{< input >}}` sliders driving a **draggable** `{js}` loss-surface graphic (once-cell + `tali.onInput` redraw + pointer-capture drag), a `{{< input type=select >}}` stepping one `{js}` figure through five named scenes, a reactive **Observable Plot** loss-curve over the same sliders, display **math**, and two numbered theme-adaptive **SVG figures** with `@fig-` cross-refs. The first corpus doc to stack reactive-input × drag × a select-driven scene graph × Plot × math on one page (and the first single-page *website* gallery exhibit, vs the book personas). Pinned by `descent.rs`; the third marketing-site **gallery** exhibit (`/gallery/descent`). See `notes/2026-07-22-corpus-demand-probe-interactive-explainer.md` | (purpose-built, demand-probe #3) |
| `analyst/` | Computational report (demand-probe #4) | a two-page quarterly latency **readout** and the only corpus project that runs **two languages in one document**: `{python}` (pandas/matplotlib) cleans and charts, `{r}` (broom/ggplot2/patchwork) fits and diagnoses, both reading one committed `data/latency.csv`. Stacks what no other doc combines: a table counter shared by the **authored** `: caption {#tbl-}` path and the **executed** `#\| label: tbl-` path in document order, a figure counter spanning both languages, and **cross-page** `@tbl-`/`@fig-` refs to *cell-produced* floats (harvested from the defining page's render, not the source scan). Pinned by `analyst.rs` (render-time only — no kernel gated); the fourth **gallery** exhibit (`/gallery/analyst`), and the only one that executes. See `notes/2026-07-26-corpus-demand-probe-analyst.md` | (purpose-built, demand-probe #4) |

`tech-blog/` is the multi-page spec (the destination in `todo.md` §4). It is the
author's real blog with the deploy caches stripped (`.venv`, `_freeze`, `_site`,
`infra`, heavy demo media); only the renderable sources are vendored. `build
corpus/tech-blog` emits a static `_site/`; `preview corpus/tech-blog` serves it
live with cross-page navigation and per-page hot reload. Its `listing:` blocks
(blog index, projects index, homepage recent-posts) render post cards, and the
homepage's `hero:` block renders the Marginalia header (see `todo.md` §4).

`posts/pca-geometry/index.tmd` pulls in the Three.js scene helper via
`{{< include _includes/three-scene.tmd >}}`, from its **own**
`posts/pca-geometry/_includes/`.

It used to reach up to `corpus/_includes/` with `../../`, mirroring the source
project's layout. That silently stopped working: a single invoked document is
confined to its own directory unless an `_site.yml` widens the boundary (PT-2,
see `crates/core/tests/include_root_parity.rs`), and nothing above `corpus/posts/`
declares one — so `build corpus/posts/pca-geometry/index.tmd` shipped the literal
`{{< include … >}}` as text plus three "couldn't load" boxes where the 3D figures
belong. The `corpus/tech-blog/` copy of this post keeps the `../../` form, which
is correct *there* because `corpus/tech-blog/_site.yml` declares that boundary.
The two copies are otherwise byte-identical, and that one line is the difference.
`every_corpus_doc_resolves_its_includes_when_built_alone` (`tests/corpus.rs`)
sweeps every doc through the single-file entry point so this cannot rot again.

**The Three.js pages need the network.** Every copy of `three-scene.tmd`
(`posts/pca-geometry/`, `tech-blog/`) `import()`s three.js, `OrbitControls` and
`GLTFLoader` from `https://esm.sh/three@0.163.0` at *view* time, so both
`pca-geometry` copies do not render their 3D figures offline. The build says so per page ("external
reference not bundled … offline viewing fails"); this is the corpus's own
authored `{js}`, not something the tool injects, and it is the one place the
corpus depends on a third party at view time. Taliesin vendors d3, Observable
Plot and mermaid for exactly this reason, so vendoring three.js too is the
obvious symmetry — it is left undone deliberately, because it is ~600 kB plus an
ESM bundling step, a `THIRD_PARTY.md` entry and a `third_party.rs` version pin,
which is a product decision rather than a bug fix.

`diagnostics/` holds docs that deliberately trip Taliesin's schema validators
(`typos.tmd`: a misspelled key in each surface, front-matter top-level + nested,
callout kind, cell option). It is pinned by `crates/core/tests/nested_validation.rs`,
which asserts the exact click-to-source warnings, and is exempted from the corpus
"clean vocabulary" guards.

## How the corpus is used

`crates/core/tests/corpus.rs` renders every doc here and asserts the
load-bearing invariants (each block has an id + valid sourcepos, ids unique,
blocks in document order, includes resolved, the book
gets a TOC + numbered figures, and the `tech-blog/` site discovers its pages and
renders them with chrome + `.tmd`→`.html` link rewriting). These are the
project's regression tests, so the corpus must stay.

Structural comparison against **Quarto** (rendering the same doc with both and
diffing) lives in the separate `qmd-fast-testbed` repo, not here.

`crates/core/tests/tech_blog.rs` tracks progress toward using Taliesin as the
edit-preview loop for the author's tech-blog: passing tests lock in the per-post
features (math, callouts, citations, raw-`{=html}` passthrough, numbered/labelled
equations + `@eq-` refs, collapsible callouts, `code-fold`, and live `{js}`
cells) plus the site-level surface (`listing:`, the `hero:` homepage header, cross-page
refs, favicon, 404). One `#[ignore]`d test remains; run
`cargo test --test tech_blog -- --ignored` for it.

Live `{js}` cells need a real web server (a cell's relative `import()` is blocked over
`file://`), so they are verified by `serve` + browser rather than a unit test; see
`notes/backlog.md` for known follow-ups.
