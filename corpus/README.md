# Corpus

Real documents that serve as the specification for `taliesin`. "Done" means
these render correctly (judged by inspection). Each doc was copied from the
author's own projects; provenance is below.

## Documents

| Path | Category | Exercises | Source |
|---|---|---|---|
| `native-tmd.tmd` | Native source extension | authored in `.tmd` (Taliesin's native *and only* source extension); pins that the walker/`check`/link-rewrite recognize `.tmd` and that a stray `.qmd` file is no longer recognized as a source document | (purpose-built) |
| `highlight.tmd` | Syntax-highlighting coverage | server-side `tali-hl-` scope classes per language; pins that `ts`/`toml` highlight (syntect's bundled set has neither) and that `text`/`console`/unlabelled fences stay plain without tripping `validate_code_languages` | (purpose-built) |
| `theme-css/` | Custom `theme:` resolution | `theme: brand.css` (a sibling stylesheet) is read from disk relative to the document and inlined after the base stylesheet, into the page's `<style id="tali-theme">`; the installed `_extensions/<name>/theme.css` bundle branch resolves by bare name. Pinned by `theme_css.rs` (the file-read + extension-bundle positive paths; the missing-file *warning* is pinned in the render unit tests) | (purpose-built) |
| `posts/born-machines.tmd` | Prose blog post | pure prose (no math/code) — the simplest Phase 1 target | `personal/blog` |
| `posts/em-algorithm/` | Math blog post | heavy KaTeX (~100 math spans), 6 code cells, `{js}` cells | `personal/tech-blog` |
| `posts/pca-geometry/` | Live-demo blog post | `{js}` cells + Three.js + math + code | `personal/tech-blog` |
| `posts/fourier-transform/` | Interactive blog post | `define(...)` Python→`{js}` bridge, raw-HTML (`{=html}`) audio players, labelled equations (`@eq-`) | `personal/tech-blog` |
| `posts/cite-coverage/` | Citation/bibliography fixes | `.bib` rendering edge cases: LaTeX accents → Unicode (Müller/Schölkopf/Erdős/Rényi), brace-protected corporate author, `@string` macro substitution, `@incollection` `booktitle`+`pages`, and a manual `# References` heading suppressing the auto one | (purpose-built) |
| `deck.tmd` | Slide deck (`format: deck`) | the deck-engine regression net: headings→slides, `. . .` pause fragments + `.incremental`, code line-stepping (`code-line-numbers`), `.magic-move`, an `auto-animate` pair, a per-slide `background-color`, `.tali-stretch`, speaker notes, a vertical stack (h1 lead + h2 children), and an explicit `{#id}` slide anchor — one kept rich feature per slide, pinned by `corpus_deck_pins_every_kept_rich_feature` | (purpose-built) |
| `embed/` | `{{< embed >}}` shortcode | a page embeds a deck (`{{< embed talk.tmd >}}`); a **site** build must build the deck as a standalone `.html` beside the page and keep it out of the nav, so the iframe resolves. Pinned by `embed_site_build.rs` (site half) + render-level unit tests (iframe markup, `.tmd`→`.html` href, `embed_targets` extraction); the single-doc *warning* half is `embed_warning.rs` | (purpose-built) |
| `bayesian-website/` | Single-page website | one page assembled from `subsections/` includes, cross-refs, bib + CSL, TOC | `personal/bayesian-fatality-analysis` |
| `tech-blog/` | Multi-page website | `_site.yml` project config, many pages + posts, navbar/footer, prev/next, `.tmd`→`.html` cross-page links; `posts/draft-example/` pins **draft-aware preview** (`draft: true` → shown+badged in `preview`, excluded from `build`) | `personal/tech-blog` |
| `demo-book/` | Multi-chapter book | flat native `chapters:` (with a `part:` and `{ file:, text: }` label overrides), left chapter sidebar, chapter + section numbering, prev/next-chapter nav; `appendix.tmd` (last) pins a **draftable chapter** (dropped + renumbered in `build`, marked in `preview`) | (purpose-built for the book format) |
| `theorem-book/` | Book-level `theorems:` | `_site.yml` sets a book-wide theorem-numbering policy (`numbered: false`); `alpha.tmd` (no `theorems:` block) inherits it and renders an **unnumbered** theorem, while `beta.tmd` declares its own `theorems: { numbered: true }` and overrides it wholesale (chapter-scoped "Theorem 2.1"). Pinned by `book_theorems.rs` | (purpose-built) |
| `narrate/walkthrough.tmd` | Narrated code walkthrough | `::: {.code-walkthrough}` sticky code panel + scroll-driven line-range focus (`.step lines=`) | (purpose-built) |
| `layout/panels.tmd` | Tabsets + margin notes | `::: {.panel-tabset}` (headings → ARIA tabs) + `::: {.column-margin}`; `@fig-` cross-ref resolves through a tab | (purpose-built) |
| `callouts/kinds.tmd` | Callout contract | all 5 callout kinds with bundled icons + `appearance=` (simple/minimal) + `icon="false"` | (purpose-built) |
| `media/gallery.tmd` | Image lightbox / gallery | `layout-ncol` figure grid; click-to-zoom + ←/→ gallery navigation in the lightbox | (purpose-built) |
| `reactive/graph.tmd` | `{js}` reactive graph | `//| viewof`/`//| name`/`//| input` chains; a slider re-runs only its transitive-downstream closure | (purpose-built) |
| `reactive/inputs.tmd` | `{{< input >}}` controls | declarative reactive controls (slider/number/checkbox/text/select) feeding `{js}` cells through the graph (incl. a transitive chain) | (purpose-built) |
| `reactive/js-error.tmd` | `{js}` cell error state | a throwing `{js}` cell surfaces the `.tali-js-error` box (themed light + dark); pins the runtime-error state for browser verification | (purpose-built) |
| `refs/theorems.tmd` | Theorem environments | all 8 kinds across the 3 amsthm styles, `title=`, a proof with auto-QED, per-kind continuous numbering, and `@thm-`/`@def-`/`@lem-` cross-refs resolving | (purpose-built) |
| `refs/theorems-shared.tmd` | Shared theorem counters | `theorems: shared: [...]` makes theorem/lemma/corollary/proposition draw one sequence (Theorem 1, Lemma 2, Corollary 3) while `definition` counts separately; cross-refs resolve to the shared numbers | (purpose-built) |
| `refs/theorems-unnumbered.tmd` | Conditional theorem numbering | `theorems: numbered: unless-unique` leaves a lone `definition` unnumbered while numbering the two recurring `theorem`s (1, 2) | (purpose-built) |
| `refs/theorems-interactive.tmd` | Web-native theorem affordances | hover-preview of a `@thm-` ref (link-preview card), a collapsible `::: {.proof collapse="true"}` (native `<details>`), and a deep-link copy-anchor on the theorem box | (purpose-built) |
| `explorable/scrolly.tmd` | Scrollytelling | `::: {.scrolly}` sticky stage + `.step` scenes; the active step drives a reactive value a `{js}` cell reads (`//| input:`) | (purpose-built) |
| `reader/` | Reader experience | read-only reader enhancers: display prefs (theme/sepia/size/width/spacing), reading progress + resume, hover cross-ref cards, anchor copy-links, focus mode, and a read-state right-rail TOC | (purpose-built) |
| `diagnostics/` | Validator coverage | docs that deliberately trip Taliesin's schema validators (`typos.tmd`) + the opt-in prose linter (`prose.tmd`: doubled/weasel/banned, markdown-aware) + the `check`-superset static lints (`check-superset.tmd`: duplicate `{#id}`, broken in-page anchor, missing image, citation with no `bibliography:`) | (purpose-built) |
| `bare-draft.tmd` | Bare build (`--bare`) | prose + inline math + a server-highlighted code block + an image + a `{js}` cell + a Mermaid block; pins the `build --bare` contract (zero `<script>`/zero CDN, CSS-only theme, math kept, `{js}` dropped, Mermaid as source) and the Phase-1 enhancer gating | (purpose-built) |
| `agent/executed-read.tmd` | Headless executed-output (DX17) | a labelled-figure python cell + a printed stream + a deliberately-erroring cell; pins `read --run`'s executed-output projection (`[figure fig-x: produced, alt "…"]` / `[output: …]` / `[cell error: …]`) via `read_run.rs` | (purpose-built) |
| `agent/executed-read-js.tmd` | Headless `{js}` executed-output (DX17b) | an Observable-Plot `{js}` cell that paints an `<svg>` + a deliberately-throwing `{js}` cell; pins `read --run`'s headless-Chrome `{js}` projection (`[js: produced, <svg W×H>]` / `[js error: …]` / `[js: skipped …]`) via the Chrome-gated `read_run_js.rs` | (purpose-built) |
| `course/` | Realistic course (demand-probe pilot) | a lecturer's interactive lecture-notes **book** + an embedded companion **deck**: theorems/proofs numbered + cross-referenced **across chapters** (shared counter × chapter scope), a `{{< embed >}}` deck inside a chapter, a `.code-walkthrough`, a `{python}` cell, and a draft appendix. The first corpus doc that *stacks* these interactions (single-feature pins never combine them). Pinned by `course.rs`; also the first marketing-site **gallery** exhibit (`/gallery/course`). See `notes/2026-07-22-corpus-demand-probe-course-author.md` | (purpose-built, demand-probe pilot) |
| `tarn/` | Realistic library docs (demand-probe #2) | the documentation site for a small illustrative dataframe library: a **book** with Guide + API **Reference** parts, dual `.panel-tabset`s (package-manager + per-OS install, per-language usage), a `.code-walkthrough`, version/deprecation callouts, and **cross-page** guide→reference links; full-text Cmd-K search spans the book **including tabset-hidden panel content**. The first corpus doc to stack tabsets × search × an API reference. Pinned by `tarn.rs`; the second marketing-site **gallery** exhibit (`/gallery/tarn`). See `notes/2026-07-22-corpus-demand-probe-docs-maintainer.md` | (purpose-built, demand-probe #2) |
| `descent/` | Interactive explainer (demand-probe #3) | a single-page **explorable explanation** of gradient descent that stacks the interactive cluster on one page: three `{{< input >}}` sliders driving a **draggable** `{js}` loss-surface graphic (once-cell + `tali.onInput` redraw + pointer-capture drag), a `.scrolly` whose sticky `{js}` figure redraws per scene, a reactive **Observable Plot** loss-curve over the same sliders, display **math**, and two numbered theme-adaptive **SVG figures** with `@fig-` cross-refs. The first corpus doc to stack reactive-input × drag × scrolly × Plot × math on one page (and the first single-page *website* gallery exhibit, vs the book personas). Pinned by `descent.rs`; the third marketing-site **gallery** exhibit (`/gallery/descent`). See `notes/2026-07-22-corpus-demand-probe-interactive-explainer.md` | (purpose-built, demand-probe #3) |

`tech-blog/` is the multi-page spec (the destination in `todo.md` §4). It is the
author's real blog with the deploy caches stripped (`.venv`, `_freeze`, `_site`,
`infra`, heavy demo media); only the renderable sources are vendored. `build
corpus/tech-blog` emits a static `_site/`; `preview corpus/tech-blog` serves it
live with cross-page navigation and per-page hot reload. Its `listing:` blocks
(blog index, projects index, homepage recent-posts) render post cards, and the
homepage's `about:` block renders a profile header (see `todo.md` §4).

`posts/pca-geometry/index.tmd` pulls in `_includes/three-scene.tmd` via
`{{< include ../../_includes/three-scene.tmd >}}`; the `posts/` + `_includes/`
layout is mirrored from the source project so that path resolves verbatim.

`diagnostics/` holds docs that deliberately trip Taliesin's schema validators
(`typos.tmd`: a misspelled key in each surface, front-matter top-level + nested,
callout kind, cell option). It is pinned by `crates/core/tests/nested_validation.rs`,
which asserts the exact click-to-source warnings, and is exempted from the corpus
"clean vocabulary" guards.

## How the corpus is used

`crates/core/tests/corpus.rs` renders every doc here and asserts the
load-bearing invariants (each block has an id + valid sourcepos, ids unique,
blocks in document order, includes resolved, decks split into slides, the book
gets a TOC + numbered figures, and the `tech-blog/` site discovers its pages and
renders them with chrome + `.tmd`→`.html` link rewriting). These are the
project's regression tests, so the corpus must stay.

Structural comparison against **Quarto** (rendering the same doc with both and
diffing) lives in the separate `qmd-fast-testbed` repo, not here.

`crates/core/tests/tech_blog.rs` tracks progress toward using Taliesin as the
edit-preview loop for the author's tech-blog: passing tests lock in the per-post
features (math, callouts, citations, raw-`{=html}` passthrough, numbered/labelled
equations + `@eq-` refs, collapsible callouts, `code-fold`, and live `{js}`
cells). The remaining `#[ignore]`d tests encode the website-scope `listing:`/
`about:` features. Run `cargo test --test tech_blog -- --ignored` for those.

Live `{js}` cells need a real web server (a cell's relative `import()` is blocked over
`file://`), so they are verified by `serve` + browser rather than a unit test; see
`notes/backlog.md` for known follow-ups.
