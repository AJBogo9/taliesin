# Final-scope audit — the feature-portfolio catalogue

**Run 2026-08-02.** The evidence base for
[the final-scope spec](../docs/superpowers/specs/2026-08-02-final-scope-design.md).
Successor to the 2026-08-01 adoption round (FV family); this one is the **FV-2 ablation
lens crossed with cost-to-carry**, run across the whole surface rather than the three
features already filed for cutting.

## Method

501 user-facing features catalogued across nine disjoint surfaces by nine parallel
inventory agents, each record carrying code locations, adoption, dependence, carry cost,
origin and byte cost. Four cross-cutting lenses then reasoned over the whole catalogue:
moat, inherited-vocabulary, a maximal-cut skeptic, and a keep-defender tasked with
refuting the cuts. 13 agents, 2.96M tokens, 963 tool calls, 0 errors.

Adoption came from the tool's own `taliesin features --format json` over all 195 `.tmd`
documents, split by authorship: **author writing** (`corpus/tech-blog` 19, `corpus/tarn`
14, `site/` 7), **the dogfooded manual** (`docs/` 40), **pin fixtures** (`corpus/` 105).

Of 152 tracked authoring features: **63 used in real writing, 19 manual-only, 45
pin-only, 25 used by no document anywhere.**

## Lens headlines

**moat.** The architecture that no static site generator can copy is ~13% of the codebase and receives a fraction of its maintenance attention — the deck engine alone has 178 commits to the block diff and freeze cache's 28 combined — while the moat's published headline number (83x) is measurably 10x wrong, ungated, and about to ship.

**inherited.** The lens's own hypothesis is refuted by measurement: inherited and chosen features are cut-or-folded at an identical rate (17.4% vs 17.3% of 509 catalogued features), because the shed-Quarto ruling was actually applied — three times, in writing, with dates — while the 71% of the surface that was never inherited has never had an equivalent ruling and carries 3x the "freeze" rate.

**skeptic.** Taliesin is one good product (warm-kernel incremental HTML preview with click-to-source) carrying four others it has never used — a slide engine with zero author documents and zero invocations ever, a 16.5 MB browser-Python runtime for one fixture, an academic-publishing cluster whose every key lives in exactly two files, and a Cloudflare deploy tool — and cutting them plus the duplicated dev server removes ~22,000 LOC (17%) and 17.4 MB of the 75.6 MB binary without touching a single load-bearing invariant.

**defender.** The two largest, cleanest-looking deletions on the board — the 551-LOC social-card generator and the 16 MB Pyodide payload — both rest on replacement plans the codebase itself already refutes (one by a named regression test, one by an unlanded-but-designed cargo feature), so the biggest apparent LOC wins are the two most likely regrets.

## Corrections to the record

- The 2026-08-01 round's "theorem environments have 2 real uses including a genuine blog
  post" is **false**: `em-algorithm` contains the phrase "Bayes' theorem" in prose. The
  true figure is one document (`corpus/tarn/grouping.tmd`) using one of eight kinds.
- The **101 MB binary** behind item 205 is rot: 72.1 MiB on disk, **32 MiB as downloaded**
  (releases ship `tar czf`), pyodide 27.6% of that.
- `logo:`/`footer:` are not dead front-matter keys; both are supplied site-wide in
  `_site.yml`. Two mechanisms for one job, not an unused feature.
- `serve/mod.rs` is not a pure duplicate of `serve_site`: the latter imports 15 items from
  it (~424 lines), so the fold is an extract-then-delete, not a 2,753-line deletion.
- `skim` has three consumers (`backlinks.rs`, `query.rs`/MCP), not one, so its cut is ~600
  lines rather than the 1,400 the skeptic booked.

## The catalogue

Verdicts are the inventory agents' own, before the cross-cutting lenses' overrides and
before the owner rulings recorded in the spec. Where a lens overrode a verdict or the
owner ruled differently, **the spec is authoritative, not this table** (decks are frozen,
not cut; pyodide is folded, not cut; `publish` and social cards are kept).

| verdict | features |
|---|---|
| load-bearing | 52 |
| core | 57 |
| keep | 155 |
| narrow-keep | 102 |
| fold | 31 |
| freeze | 50 |
| cut | 57 |

### `frontmatter-config` — 97 features, ~4,726 LOC

The declared configuration surface is 73 validated keys (34 front-matter top-level in `crates/core/src/frontmatter.rs:21-78`, 18 front-matter sub-keys across EXECUTE/LISTING/HERO/PROSE_LINT/THEOREM_KEYS at `frontmatter.rs:134-158`, and 21 `_site.yml` keys in `crates/core/src/site/config/mod.rs:242-271`), plus 3 nested item vocabularies that ARE validated (nav/footer sections, nav items, mounts, pu

| verdict | feature | LOC | adoption |
|---|---|---|---|
| load-bearing | _site.yml: chapters: | 140 | author 1 (tarn) / manual 2 (guide, internals) / pin 4 (course, demo-bo |
| load-bearing | unknown-key diagnostic with did-you-mean (levenshtein) | 80 | Fires on every document; pinned by corpus/diagnostics/typos.tmd (an in |
| load-bearing | front-matter block splitter + YAML error locator | 40 | Every document with front matter |
| load-bearing | _site.yml: url: | 30 | author 2 (tech-blog, site) / manual 0 / pin 2 (cite-this, embed) |
| load-bearing | front matter: title: | 12 | author 30 (tech-blog 18 / tarn 7 / site 5) / manual 40 (guide 25 + int |
| load-bearing | _site.yml: title: | 8 | author 3 (tech-blog, tarn, site) / manual 2 (guide, internals) / pin 1 |
| core | front matter: listing: | 120 | author 4 (tech-blog blog/projects/cv/index) / manual 0 / pin 1 (diagno |
| core | front matter: format: | 90 | author 1 (site 1) / manual 2 (guide demo.tmd, tour.tmd) / pin 6 (all d |
| core | _site.yml: nav: | 90 | author 2 (tech-blog 4 items, site 6+1 items) / manual 0 / pin 4 (analy |
| core | front matter: date: | 70 | author 12 (tech-blog 12) / manual 0 / pin 10 |
| core | front matter: bibliography: | 40 | author 7 (tech-blog 7) / manual 1 (guide using/writing.tmd) / pin 10 |
| core | _site.yml nested: nav/footer item keys (text / href / icon) | 40 | author 2 (tech-blog: 4 nav + 5 footer items; site: 7 nav + 4 footer it |
| core | front matter: image-alt: | 35 | author 11 (tech-blog 11) / manual 0 / pin 3 |
| core | front matter: image: | 25 | author 11 (tech-blog 11) / manual 0 / pin 3 |
| core | front matter: categories: | 8 | author 12 (tech-blog 12) / manual 0 / pin 7 |
| core | front matter: description: | 6 | author 23 (tech-blog 12 / tarn 6 / site 5) / manual 40 (guide 25 + int |
| core | listing.contents: | 6 | author 4 (tech-blog blog/projects/cv/index) / manual 0 / pin 1 |
| core | _site.yml: description: | 6 | author 2 (tech-blog, site) / manual 0 / pin 3 (analyst, descent, graph |
| core | hero.headline: | 4 | author 4 (site 3 / tech-blog 1) / manual 0 / pin 0 |
| keep | Generated JSON Schemas for front matter and _site.yml | 556 | Not measurable as document adoption; consumed by `taliesin schema`, th |
| keep | front matter: links: | 330 | author 0 / manual 0 / pin 1 (corpus/cite-this/paper.tmd) |
| keep | front matter: author: | 320 | author 0 / manual 0 / pin 3 (analyst/index.tmd, bayesian-website/index |
| keep | front matter: hero: | 110 | author 4 (site 3 / tech-blog 1) / manual 0 / pin 0 |
| keep | _site.yml: chapters: entry vocabulary (part / chapters / file /  | 90 | author 3 uses of `part`+`chapters` (tarn ×3 groups) / manual 2 (guide  |
| keep | _site.yml: mounts: | 80 | author 1 (site/_site.yml, 7 mount entries: 2 docs books + 5 gallery ex |
| keep | _site.yml: bibliography: | 70 | author 0 / manual 0 / pin 1 (corpus/shared-bib/_site.yml) |
| keep | date: value validator (calendar_date) | 60 | Zero documents trip it; 2 tests covering 4 bad and 3 good forms |
| keep | _site.yml: footer: | 50 | author 2 (tech-blog, site) / manual 0 / pin 0 |
| keep | RETIRED_KEYS registry | 50 | 3 entries: `datasets` (2026-08-02), `about` (2026-07-17), `theorems.nu |
| keep | format: value validator (revealjs + non-HTML targets) | 50 | Zero documents trip it (that is success); 4 dedicated tests cover stri |
| keep | front matter: draft: | 45 | author 1 (tech-blog 1) / manual 0 / pin 2 (course/problems.tmd, demo-b |
| keep | front matter: execute: | 40 | author 0 / manual 0 / pin 2 (bayesian-website/index.tmd, diagnostics/t |
| keep | format: sub-key lint | 35 | Zero documents trip it; 3 tests pin it |
| keep | front-matter image alt-text lint (PA-M13) | 35 | Fires on 4 real pages (frontmatter.rs:316 records the calibration), al |
| keep | front matter: page-layout: | 30 | author 5 (site 4 / tech-blog 1) / manual 0 / pin 0 |
| keep | front matter: doi: | 20 | author 0 / manual 0 / pin 1 (corpus/cite-this/paper.tmd) |
| keep | hero.actions: | 20 | author 3 (site only |
| keep | _site.yml nested: nav section keys (left / right) | 20 | author 2 (`nav.left` in tech-blog + site; `nav.right` in site only) /  |
| keep | page-layout: value validator | 20 | Zero documents trip it (all 5 real uses write `full`); 1 test with 3 a |
| keep | front matter: toc: | 15 | author 5 (tech-blog 4 / site 1) / manual 0 / pin 10 |
| keep | _site.yml: author: | 15 | author 0 / manual 2 (guide, internals) / pin 4 (cite-this, course, dem |
| keep | _site.yml: logo: | 15 | author 1 (tech-blog logo.svg) / manual 0 / pin 1 (demo-book logo.svg) |
| keep | _site.yml url: scheme validator | 15 | Zero configs trip it (all 4 real uses carry a scheme) |
| keep | _site.yml: favicon: | 12 | author 2 (tech-blog bell-curve.svg, site favicon.svg) / manual 0 / pin |
| keep | front matter: title-block-style: | 10 | author 3 (tech-blog 3) / manual 0 / pin 0 |
| keep | listing.type: | 6 | author 4 (tech-blog: grid ×2, list ×1, `default` ×1) / manual 0 / pin  |
| keep | listing.categories: | 6 | author 2 (tech-blog blog.tmd, projects.tmd) / manual 0 / pin 0 |
| keep | front matter: subtitle: | 4 | author 1 (tech-blog 1) / manual 2 (guide demo.tmd, tour.tmd) / pin 9 |
| keep | hero.eyebrow: | 4 | author 4 (site 3 / tech-blog 1) / manual 0 / pin 0 |
| keep | hero.lead: | 4 | author 4 (site 3 / tech-blog 1) / manual 0 / pin 0 |
| keep | _site.yml key documentation has NO completeness gate |  | Measured by grepping configuration.tmd for each of the 21 NATIVE_KEYS: |
| narrow-keep | front matter: theme: | 60 | author 0 / manual 0 (5 guide prose examples) / pin 1 (corpus/theme-css |
| narrow-keep | front matter: csl: (recognized, not honored) | 45 | author 0 / manual 0 / pin 0 |
| narrow-keep | UNSUPPORTED_KEYS (recognized-but-inert register) | 30 | 1 entry (`csl`). Zero documents set it, by design |
| narrow-keep | theorems.numbered: value validator | 25 | Zero documents trip it; 4 tests. Shared by the `_site.yml` book-level  |
| narrow-keep | front matter: venue: | 15 | author 0 / manual 0 / pin 1 (corpus/cite-this/paper.tmd) |
| narrow-keep | YAML-1.1 boolean words (yes / no / on / off) | 15 | Shared by four readers: site::frontmatter::bool_field, render::cell_ex |
| narrow-keep | front matter: acknowledgments: (US spelling) | 10 | author 0 / manual 0 / pin 1 (corpus/cite-this/paper.tmd) |
| narrow-keep | _site.yml: head: | 10 | author 0 / manual 0 / pin 0 |
| narrow-keep | _quarto.yml rename advisory | 10 | Zero directories trip it in this repo (the rename happened at the .tmd |
| narrow-keep | front matter: footer: (deck chrome) | 8 | author 0 / manual 0 (2 guide prose examples only) / pin 2 (corpus/deck |
| narrow-keep | _site.yml nested: footer section keys (left / center / right) | 8 | author 2 (`footer.left` + `footer.right` in tech-blog and site) / manu |
| narrow-keep | listing.max-items: | 5 | author 1 (tech-blog index.tmd, `max-items: 2`) / manual 0 / pin 0 |
| narrow-keep | front matter: acknowledgements: (British spelling) | 4 | author 0 / manual 0 / pin 0 |
| narrow-keep | execute.cache: | 4 | author 0 / manual 0 / pin 1 (corpus/bayesian-website/index.tmd) |
| narrow-keep | listing.id: | 4 | author 2 (tech-blog index.tmd `recent-posts`, cv.tmd `cv-projects`) /  |
| narrow-keep | front matter: lang: | 3 | author 0 / manual 0 / pin 1 (corpus/print/paged.tmd) |
| fold | The 6-gate / 8-gate documentation drift chain | 220 | Every key change since 2026-06-18. Verified against commit e0cd23a6, w |
| fold | _site.yml: toc: | 30 | author 2 (tech-blog `true`, site `false` |
| fold | _site.yml toc-in-a-book scope validator | 20 | Zero configs trip it now |
| fold | front matter: css: | 15 | author 0 / manual 0 / pin 0 |
| fold | front matter: hero.actions[] item vocabulary (text / href / prim | 15 | author 3 (site's three hero pages) / manual 0 / pin 0 |
| fold | front matter: include-in-header: | 12 | author 0 / manual 0 / pin 0 |
| fold | _site.yml: css: | 12 | author 0 / manual 0 / pin 0 |
| freeze | front matter: theorems: | 120 | author 0 / manual 0 / pin 4 (course/mle.tmd, refs/theorems-shared.tmd, |
| freeze | _site.yml: publish: | 60 | author 0 / manual 0 / pin 0 |
| freeze | _site.yml: python: | 45 | author 0 / manual 0 / pin 0 |
| freeze | theorems.numbered: | 40 | author 0 / manual 0 / pin 2 (refs/theorems-unnumbered.tmd, theorem-boo |
| freeze | _site.yml nested: publish keys (provider / project / gate) | 35 | author 0 / manual 0 / pin 0 |
| freeze | _site.yml: theorems: | 30 | author 0 / manual 0 / pin 1 (corpus/theorem-book/_site.yml, with `theo |
| freeze | _site.yml: r: | 25 | author 0 / manual 0 / pin 0 |
| freeze | theorems.shared: | 15 | author 0 / manual 0 / pin 2 (course/mle.tmd, refs/theorems-shared.tmd) |
| freeze | front matter: award: | 10 | author 0 / manual 0 / pin 1 (corpus/cite-this/paper.tmd) |
| freeze | front matter: logo: (deck corner logo) | 8 | author 0 / manual 0 / pin 0 |
| cut | front matter: prose-lint: | 190 | author 0 / manual 0 / pin 1 (corpus/diagnostics/prose.tmd, the fixture |
| cut | hero.image: | 30 | author 0 / manual 0 / pin 0 |
| cut | _site.yml nested: mount item keys (at / path) | 30 | author 0 / manual 0 / pin 0 |
| cut | prose-lint.banned: | 10 | author 0 / manual 0 / pin 1 (corpus/diagnostics/prose.tmd) |
| cut | front matter: include-before-body: | 8 | author 0 / manual 0 / pin 0 |
| cut | front matter: include-after-body: | 8 | author 0 / manual 0 / pin 0 |
| cut | _site.yml: output: | 8 | author 1 (tech-blog `output: _site`) / manual 0 / pin 1 (bayesian-webs |
| cut | _site.yml: body-start: | 8 | author 0 / manual 0 / pin 0 |
| cut | _site.yml: body-end: | 8 | author 0 / manual 0 / pin 0 |
| cut | hero.image-alt: | 6 | author 0 / manual 0 / pin 0 |
| cut | listing.sort: | 5 | author 4 (tech-blog blog/projects/cv/index) / manual 0 / pin 0 |
| cut | execute.echo: | 4 | author 0 / manual 0 / pin 0 |
| cut | execute.include: | 4 | author 0 / manual 0 / pin 0 |

### `div-classes-blocks` — 53 features, ~3,900 LOC

The `:::` fenced-block vocabulary: 16 classes in `render::validate::DIV_FEATURE_CLASSES` (NOT ~23 — measured at crates/core/src/render/validate.rs:59-81), 2 in `RETIRED_DIV_CLASSES`, 5 callout kinds, 8 theorem kinds, 8 div attributes, the figure/caption machinery, and the `layout-ncol` grid. Roughly 3,900 LOC: divs.rs 932, figure.rs 145, validate.rs ~480 (my share), number_theorems+theorem_divs 10

| verdict | feature | LOC | adoption |
|---|---|---|---|
| load-bearing | The `:::` fenced-div parser and grouper | 300 | author 40+ / manual 40+ / pin 105 |
| load-bearing | `RETIRED_DIV_CLASSES` register | 20 | Fires on migrating documents; pinned by corpus/diagnostics/typos.tmd ( |
| load-bearing | Generic div fall-through (open vocabulary) | 18 | author 3 documents use genuinely custom classes |
| core | Standalone-image `<figure>` detection + numbering | 175 | author 1 (corpus/tarn/concepts.tmd) / manual 2 (docs/guide/using/recip |
| core | Callout block (`::: {.callout-<kind>}`) | 120 | author 8 docs / manual 21 docs / pin 12 docs |
| core | Callout/proof `collapse=` attribute | 22 | author 4 / manual 0 / pin 3 |
| core | Callout kind `note` | 4 | author 6 / manual 18 / pin 9 |
| core | Callout kind `tip` | 4 | author 1 (corpus/tarn/concepts.tmd) / manual 9 (docs/guide 7, docs/int |
| keep | `::: {.code-walkthrough}` narrated code | 240 | author 2 (corpus/tarn/quickstart.tmd, site/showcase.tmd) / manual 0 /  |
| keep | `::: {.panel-tabset}` tabbed panels | 155 | author 3 (corpus/tarn/install.tmd, corpus/tarn/quickstart.tmd, site/sh |
| keep | `number_theorems` post-pass | 106 | Runs on every render; effective on the 12 documents that contain a the |
| keep | Theorem environment `::: {.theorem}` | 60 | author 0 / manual 1 (docs/guide/using/theorems.tmd) / pin 10 |
| keep | `validate_walkthrough` / `validate_tabset` / `validate_scrolly`  | 60 | Pinned by corpus/diagnostics/typos.tmd and corpus/diagnostics/widgets. |
| keep | `label_steps` + `spoken_lines` step accessibility | 60 | Applies automatically to all 9 documents containing a walkthrough or s |
| keep | `::: {.notes}` speaker notes + teleprompter timing | 45 | author 1 (site/demo.tmd) / manual 1 (docs/guide/demo.tmd) / pin 2 (cor |
| keep | `::: {.step}` scroll step | 40 | author 2 (corpus/tarn/quickstart.tmd, site/showcase.tmd) / manual 0 /  |
| keep | `validate_div_class` near-miss diagnostic | 31 | Pinned by corpus/diagnostics/typos.tmd (`.fragmnet`) and by six render |
| keep | Leading-heading title hoist (callouts, and theorems since PL17) | 30 | unmeasured directly |
| keep | `::: {.column-margin}` margin note | 25 | author 0 / manual 1 (docs/guide/using/writing.tmd) / pin 3 (corpus/lay |
| keep | Theorem environment `::: {.proof}` | 25 | author 0 / manual 1 (docs/guide/using/theorems.tmd) / pin 4 (corpus/co |
| keep | `validate_empty_feature_div` | 24 | Pinned by corpus/diagnostics/typos.tmd (which contains an empty `.inpu |
| keep | `::: {.magic-move}` animated code diff | 20 | author 1 (site/demo.tmd, the marketing demo deck) / manual 1 (docs/gui |
| keep | Theorem-id-without-xref-prefix diagnostic | 19 | Fires on authoring mistakes, not on documents; corpus/diagnostics/* pi |
| keep | Mermaid diagram as a numbered figure | 17 | unmeasured as a distinct count |
| keep | Callout/theorem `title=` attribute | 15 | author 2 (corpus/tarn/api-query.tmd, corpus/tarn/concepts.tmd) / manua |
| keep | Unterminated `:::` fence diagnostic | 10 | Fires on authoring mistakes; pinned by the diagnostics corpus and lsp_ |
| keep | `::: {.incremental}` list reveal | 6 | author 1 (site/demo.tmd) / manual 1 (docs/guide/demo.tmd) / pin 3 (cor |
| keep | Theorem environment `::: {.definition}` | 6 | author 1 (corpus/tarn/grouping.tmd) / manual 1 (docs/guide/using/theor |
| keep | `::: {.fragment}` deck reveal | 5 | author 0 / manual 0 / pin 2 (corpus/deck.tmd, corpus/scaffold/deck-tou |
| keep | Callout kind `warning` | 4 | author 1 (corpus/tarn/api-query.tmd) / manual 5 / pin 3 |
| keep | Callout kind `important` | 4 | author 0 / manual 6 (docs/guide 4, docs/internals 2) / pin 1 (corpus/c |
| narrow-keep | `::: {.column-page}` width escape | 40 | author 0 / manual 1 (docs/guide/using/writing.tmd) / pin 1 (corpus/lay |
| narrow-keep | `.fade-out` fragment effect | 22 | author 0 / manual 0 / pin 1 |
| narrow-keep | `.highlight` fragment effect | 18 | author 0 / manual 0 / pin 1 |
| narrow-keep | Theorem cross-reference prefixes (`thm` `lem` `cor` `prp` `def`  | 15 | author 0 / manual 0 / pin: `def` 6 docs, `lem` 4, `thm` 8, `cor` 1, an |
| narrow-keep | Figure `width=` / `height=` attributes | 15 | author 0 / manual 0 / pin 2 |
| narrow-keep | `{layout-ncol=N}` column grid | 6 | author 0 / manual 0 (only prose mentions in docs/guide/using/from-quar |
| narrow-keep | Theorem environment `::: {.remark}` | 6 | author 0 / manual 1 (docs/guide/using/theorems.tmd) / pin 1 (corpus/re |
| narrow-keep | Callout kind `caution` | 4 | author 0 / manual 0 / pin 1 |
| narrow-keep | Theorem environment `::: {.lemma}` | 4 | author 0 / manual 0 / pin 3 (corpus/course/mle.tmd, corpus/refs/theore |
| narrow-keep | Theorem environment `::: {.corollary}` | 4 | author 0 / manual 0 / pin 2 (corpus/refs/theorems-shared.tmd, corpus/r |
| narrow-keep | Theorem environment `::: {.proposition}` | 4 | author 0 / manual 1 (docs/guide/using/theorems.tmd) / pin 1 (corpus/re |
| narrow-keep | Theorem environment `::: {.example}` | 4 | author 0 / manual 0 / pin 1 (corpus/refs/theorems.tmd only). Its xref  |
| freeze | `::: {.scrolly}` scrollytelling | 180 | author 1 (site/showcase.tmd |
| freeze | `::: {.column-screen}` full-viewport escape | 30 | author 0 / manual 0 / pin 1 |
| freeze | Figure `dark=` themed image pair | 23 | author 0 / manual 0 / pin 1 |
| cut | `theorems:` front-matter / `_site.yml` config (`shared:`, `numbe | 140 | author 0 / manual 0 / pin 4 |
| cut | Callout `appearance=` attribute (`simple` / `minimal`) | 12 | author 0 / manual 0 / pin 1 |
| cut | `::: {.aside}` (alias of `.column-margin`) | 8 | author 0 / manual 0 / pin 0 |
| cut | `::: {.sidenote}` (alias of `.column-margin`) | 8 | author 0 / manual 0 / pin 0 |
| cut | `::: {.marginnote}` (alias of `.column-margin`) | 8 | author 0 / manual 0 / pin 0 |
| cut | Figure `fig-align=` attribute | 8 | author 0 / manual 0 / pin 1 |
| cut | Callout `icon="false"` attribute | 5 | author 0 / manual 0 / pin 1 |

### `shortcodes-cells` — 40 features, ~6,070 LOC

Two joined surfaces: the `{{< … >}}` shortcode expander (5 built-ins) and the executable/client cell languages plus the `{js}` reactive graph. ~6,070 implementation LOC (extension/mod.rs 1,282 + dataset.rs 500 + includes.rs 1,201 + pyodide.rs 211 + client_lang.rs 77 + diagnostics/reactive.rs 179 + tali-js.js 1,028 + numerics.js 420 + pyodide.js 319 + glsl.js 207 + mermaid.js 144 + ~500 of language

| verdict | feature | LOC | adoption |
|---|---|---|---|
| load-bearing | {{< include file.tmd >}} transclusion | 1201 | author 5 / manual 1 / pin 6 (corpus/tech-blog/cv.tmd, publications.tmd |
| load-bearing | {js} cells (browser-native reactive cells) | 350 | author 10 / manual 1 / pin 22 |
| load-bearing | {python} cells (warm Jupyter kernel) | 250 | author 7 / manual 4 / pin 11 (braced ```{python} fences only). Command |
| load-bearing | The {js} reactive dependency graph | 110 | Every reactive document rides it: author 7 documents carry `/// input: |
| load-bearing | Client-cell degradation to highlighted source | 40 | Applies to every client cell in every non-preview mode, so all 33 {js} |
| core | /// viewof: NAME | 15 | author 6 / manual 2 / pin 4. Command: `grep -rc '^/// viewof:' corpus/ |
| core | /// name: NAME (and #/ name: for kernel/pyodide cells) | 15 | author 2-3 / manual 1 / pin 13-14 (the tool reports 2/1/14 for the cel |
| core | /// input: a, b | 15 | author 7 / manual 2 / pin 14 |
| keep | The {{< … >}} shortcode expander | 340 | Substrate for all 5 built-ins; the 4 it actually expands (embed/video/ |
| keep | {mermaid} diagram cells | 200 | author 0 / manual 11 / pin 4. Command: scratchpad/fences.py. The dogfo |
| keep | Static reactive-graph validation (dangling inputs + cycles) | 179 | Runs on every document with client cells, so it covers all 10 author { |
| keep | {{< video clip.mp4 >}} framed screencast | 109 | author 2 (site/features.tmd, site/index.tmd) / manual 0 / pin 3. `capt |
| keep | {{< embed deck.tmd >}} embedded deck iframe | 105 | author 2 (site/formats.tmd, site/index.tmd) / manual 2 / pin 2. Comman |
| keep | Shortcode argument validation + did-you-mean | 93 | Runs on every shortcode invocation. Exercised by deliberate typo fixtu |
| keep | The client-side cell-language registry | 77 | Three entries: js (author 10), glsl (pin 1), pyodide (pin 1). Gates th |
| keep | Shareable control state in the URL fragment | 64 | Rides every `{{< input >}}` control automatically, so effectively auth |
| keep | Reactive output live region (a11y announcements) | 35 | Automatic on every {js} output region, so all 10 author {js} documents |
| keep | input type="slider" | 25 | author 1 (site/showcase.tmd) / manual 0 / pin 6. The default type when |
| keep | video dark= (light/dark clip pair with lazy promotion) | 20 | author 2 (site/features.tmd, site/index.tmd) / manual 0 / pin 0. Comma |
| keep | The d3 + Observable Plot payload for {js} cells |  | Gated by `has_client_cells_of(body, "js")`, so it rides all 10 author  |
| narrow-keep | {{< input >}} declarative reactive control | 218 | author 1 / manual 0 / pin 9. The one author document is site/showcase. |
| narrow-keep | input type="select" options="a,b,c" | 18 | author 0 / manual 0 / pin 2. The `options=` argument: author 0 / manua |
| narrow-keep | video controls / audio flags (three playback modes) | 14 | controls: author 0 / manual 0 / pin 1. audio: author 0 / manual 0 / pi |
| narrow-keep | input type="checkbox" | 12 | author 0 / manual 0 / pin 1. Command: scratchpad/sc.py. |
| narrow-keep | video captions=clip.vtt (WCAG caption track) | 10 | author 0 / manual 0 / pin 1 (a single fixture). Command: scratchpad/sc |
| narrow-keep | input type="text" | 8 | author 0 / manual 0 / pin 1. Command: scratchpad/sc.py. |
| narrow-keep | input type="number" | 5 | author 0 / manual 0 / pin 2. Command: scratchpad/sc.py. |
| fold | {pyodide} cells (client-side Python via a vendored WASM runtime) | 530 | author 0 / manual 0 / pin 1 (corpus/reactive/pyodide.tmd). Command: sc |
| fold | scan_shortcodes (the features-report scanner) | 45 | Off the render path; used only by `taliesin features`. Its sibling sca |
| freeze | The num global (curated numerics/stats namespace) | 420 | author 0 / manual 1 / pin 3. Command: scratchpad/probe.sh (`num\.`). T |
| freeze | {glsl} cells (fragment shaders to a live canvas) | 207 | author 0 / manual 0 / pin 1 (corpus/reactive/glsl.tmd). Command: scrat |
| freeze | {r} cells (warm IRkernel) | 90 | author 0 / manual 0 / pin 7. Command: scratchpad/fences.py. Five of th |
| freeze | input type="animate" (play/pause/step/reset tick) | 60 | author 0 / manual 0 / pin 1 (corpus/reactive/animate.tmd). Command: sc |
| freeze | tali.tex and tali.table (rich return values) | 60 | tali.tex author 0 / manual 1 / pin 2; tali.table author 0 / manual 1 / |
| freeze | input type="point" (draggable 2-D coordinate) | 55 | author 0 / manual 0 / pin 1 (corpus/reactive/point.tmd, the fixture bu |
| freeze | tali.state (per-cell state across scheduled re-runs) | 20 | author 0 / manual 1 / pin 1. Command: scratchpad/probe.sh. |
| cut | {{< dataset data/x.csv >}} provenance card | 500 | author 0 / manual 0 / pin 1. The single real use is corpus/datasets.tm |
| cut | The Python ojs_define → {js} bridge | 65 | **author 0 / manual 0 / pin 0 |
| cut | video poster= | 4 | author 0 / manual 0 / pin 0 |
| cut | input type="range" (alias of slider) | 2 | author 0 / manual 0 / pin 0 |

### `cli-surface` — 77 features, ~14,372 LOC

The CLI surface is 21 user-facing subcommands (+1 hidden `__complete`, + help/version), ~45 flags and 13 runtime `TALIESIN_*` env vars, implemented in ~11,180 lines under `crates/server/src` (main.rs 743, cli.rs 1032, complete.rs 795, query.rs 1700, check.rs 1265, build.rs 2864, publish.rs 504, pdf.rs 405, doctor.rs 367, mcp.rs 218, the five `run_*`/session files 1228, interactive.rs 58 — impl onl

| verdict | feature | LOC | adoption |
|---|---|---|---|
| load-bearing | build | 2864 | author 7 / manual 13 guide + 2 internals pages / pin 14 test files ref |
| load-bearing | check | 1265 | author 5 / manual 12 guide + 1 internals pages (the most-documented su |
| load-bearing | help system: --help/-h, help <cmd>, focused per-subcommand pages | 470 | author: unmeasured (a bare `tali`/`taliesin` with no args appears twic |
| load-bearing | the CLI drift gates | 300 | n/a (maintenance machinery) / manual 0 / pin: they ARE the pins. Each  |
| load-bearing | preview (live server) | 132 | author 55 / manual 7 guide + 2 internals pages / pin 4+ test files (pr |
| load-bearing | lsp | 93 | author 0 (structurally invisible: spawned by the editor, never typed)  |
| load-bearing | the test-only env gates: TALIESIN_REQUIRE_KERNEL / _R / _CHROME  | 40 | author 0 hand-set (they are set by tools/gates.sh:262 and .github/work |
| core | the --format human/json / --json convention | 120 | author 0 uses of --format or --json in 82 invocations (`grep -ac -- 't |
| core | did-you-mean suggestions (commands, flags, values) | 60 | author: unmeasurable directly, but the history contains `tali view` tw |
| core | TALIESIN_PYTHON | 30 | author 7 |
| core | build --out <dir> | 10 | author 0 / manual: cli.tmd table row + build's --help / pin: 14 test f |
| core | build --strict / publish --strict | 5 | author 1 |
| keep | features | 867 | author 0 (shipped today) / manual 1 guide page (cli.tmd:544-582) / pin |
| keep | completions | 795 | author 0 (one-shot setup, structurally under-counted) / manual 2 guide |
| keep | read | 597 | author 3, ALL on 2026-07-18 (the same exploration day as vocab/schema/ |
| keep | publish | 504 | author 2, both real and recent: `taliesin publish docs/book --init` an |
| keep | doctor | 367 | author 0 / manual 1 guide page / pin doctor_cli.rs (162 lines, 5 tests |
| keep | init | 311 | author 0 (structurally invisible: one-shot setup, and the author's pro |
| keep | map | 227 | author 0 (structurally invisible: agent-facing) / manual 2 guide + 2 i |
| keep | mcp | 218 | author 1 (2026-07-18, the exploration day) / manual 2 guide pages incl |
| keep | read --run | 180 | author 0 (3 bare `read` invocations, none with --run) / manual: read - |
| keep | __complete (hidden) | 115 | author: uncountable by design (fires on keystrokes, never recorded in  |
| keep | completions --install | 95 | author 0 / manual: completions --help + docs/guide/reference/shell-com |
| keep | the headless-js cargo feature (a build-time CLI capability switc | 80 | author: implicitly always on (the launcher passes --features headless- |
| keep | check --explain <CODE> | 66 | author 0 / manual: check's --help + cli.tmd / pin 1 test file referenc |
| keep | preview --host | 30 | author 0 (`grep -ac 'taliesin.*--host\/tali .*--host' ~/.zsh_history`  |
| keep | publish --init | 30 | author 1 |
| keep | build's [out.html] second positional + the non-HTML-extension gu | 25 | author 1 (`taliesin build 17-the-proposal.tmd s1-proposal-for-tom.html |
| keep | TALIESIN_NO_EXEC | 20 | author 0 as an env var / manual: cli.tmd Environment table + both --he |
| keep | preview/build --no-exec | 15 | author 0 / manual: both --help pages, cli.tmd, and a dedicated 'Docume |
| keep | TALIESIN_CELL_SILENCE | 15 | author 0 / manual: cli.tmd Environment table + CLAUDE.md / pin: the lo |
| keep | TALIESIN_RENDER_TIMEOUT | 15 | author 0 / manual: cli.tmd Environment table (added by DOCS-1, main.rs |
| keep | check --require-kernel | 10 | author 0 / manual: check --help + 3 paragraphs at cli.tmd:117-131 / pi |
| keep | TALIESIN_NO_CACHE | 10 | author 0 / manual: cli.tmd Environment table + CLAUDE.md's freeze para |
| keep | build --jobs <N> / -j | 8 | author 0 / manual: cli.tmd + build --help (documents `--jobs` only, ne |
| keep | publish --public | 5 | author 0 (both invocations took the gated default) / manual: publish - |
| keep | publish --dry-run | 5 | author 0 / manual: publish --help + cli.tmd:661,664 (the guide's recom |
| narrow-keep | new | 450 | author 0 / manual 2 guide pages / pin new_cli.rs (424 lines, 14 tests) |
| narrow-keep | symbols | 181 | author 0 / manual 2 guide pages / pin symbols_cli.rs (168 lines, 7 tes |
| narrow-keep | init --template basic/site/book | 155 | author 0 / manual: init --help + cli.tmd:23 / pin 2 test files; cli.rs |
| narrow-keep | new's four kinds: post / page / deck / paper | 140 | author 0 for all four / manual: cli.tmd + new --help / pin corpus/scaf |
| narrow-keep | completions <bash/zsh/fish/powershell> | 85 | author 0 / manual: completions --help + shell-completion.tmd / pin com |
| narrow-keep | schema | 53 | author 1 (2026-07-18, the same exploration day as vocab/read/mcp) / ma |
| narrow-keep | preview --port / [port] positional | 20 | author 0 uses of `--port` or a port positional in 55 preview invocatio |
| narrow-keep | schema --out <dir> | 15 | author 0 (the one `tali schema` invocation was bare) / manual: schema  |
| narrow-keep | TALIESIN_JS_TIMEOUT | 15 | author 0 / manual: cli.tmd Environment table (also added by DOCS-1's g |
| narrow-keep | TALIESIN_R | 10 | author 0 / manual: cli.tmd Environment table / pin crates/server/tests |
| narrow-keep | TALIESIN_CELL_TIMEOUT | 10 | author 0 / manual: cli.tmd Environment table + CLAUDE.md / pin: kernel |
| narrow-keep | publish --no-strict / --strict | 8 | author 0 / manual: publish --help + cli.tmd / pin 1 test file each |
| narrow-keep | pdf -o / --out <path> | 8 | author 0 / manual: pdf --help + cli.tmd:334 / pin 0 test files |
| narrow-keep | vocab | 7 | author 1 (2026-07-18) / manual 1 guide + 1 internals page / pin a TALI |
| narrow-keep | new --dir <root> | 6 | author 0 / manual: new --help + cli.tmd / pin new_cli.rs |
| narrow-keep | TALIESIN_MERMAID_URL | 6 | author 0 / manual: cli.tmd Environment table / pin: none. It is the va |
| narrow-keep | preview --open | 5 | author 0 (`grep -ac -- '--open' ~/.zsh_history` on taliesin lines → 0) |
| narrow-keep | build --bare | 5 | author 0 / manual: cli.tmd:15 table row plus a 4-paragraph section (cl |
| narrow-keep | check --errors-only | 5 | author 0 / manual: check --help + cli.tmd:100 / pin 1 test file |
| narrow-keep | check --strict | 5 | author 0 on check (1 on build) / manual: check --help + cli.tmd:99 / p |
| narrow-keep | publish --project-name <name> | 5 | author 0 (both real invocations used the default) / manual: publish -- |
| narrow-keep | new --draft | 4 | author 0 / manual: new --help + cli.tmd:24 / pin new_cli.rs references |
| narrow-keep | TALIESIN_NO_CLEAR | 4 | author 0 / manual: cli.tmd Environment table / pin: none |
| fold | render | 74 | author 0 / manual 3 guide + 1 internals pages / pin render_blocks_cli. |
| fold | blocks | 51 | author 0 / manual 0 pages say `taliesin blocks` (it appears only as a  |
| fold | preview --headless | 10 | author 0 / manual: preview's focused --help only |
| freeze | run | 1228 | author 0 / manual 1 guide page (cli.tmd:238-315) / pin 0 integration t |
| freeze | pdf | 697 | author 0 / manual 1 guide page (cli.tmd:334-362) / pin print_pdf.rs (3 |
| freeze | run --all, --quiet/-q, --interrupt | 150 | author 0 for all three / manual: run --help + cli.tmd / pin 0 test fil |
| freeze | -y / --yes and the interactive wizard | 58 | author 0 (neither `new` nor `init` was ever invoked) / manual: both -- |
| freeze | run --cell N | 10 | author 0 / manual: run --help + cli.tmd:269 / pin 0 test files referen |
| freeze | run --line L | 10 | author 0 / manual: run --help + cli.tmd:308 ('In the editor') / pin 0  |
| freeze | pdf --paper a4/letter/a5 | 8 | author 0 / manual: pdf --help + cli.tmd / pin 0 test files reference " |
| cut | skim | 1136 | author 0 / manual 2 guide + 1 internals pages / pin skim_cli.rs (263 l |
| cut | new deck --tour | 80 | author 0 / manual: new --help + cli.tmd / pin new_cli.rs has a drift g |
| cut | check --stdin | 24 | author 0 / manual 0 guide pages |
| cut | preview aliases: dev, serve | 15 | author: dev 2, serve 0 hand-invocations (2 more via `cargo run -p tali |
| cut | TALIESIN_MATH_IMAGE_TIMEOUT | 10 | author 0 / manual: cli.tmd Environment table / pin math_image.rs:325-3 |
| cut | TALIESIN_OPEN and TALIESIN_HOST | 6 | author 0 for both / manual: cli.tmd Environment table + preview --help |
| cut | pdf --keep-html | 4 | author 0 / manual: pdf --help + cli.tmd / pin 0 test files |

### `lsp-editor` — 50 features, ~23,285 LOC

The LSP server + VS Code companion is 23,285 lines: 14,596 in `crates/server/src/lsp*.rs` (7,099 impl + 7,497 unit tests, 291 `#[test]`s), 892 in `crates/server/tests/lsp_stdio.rs`, 6,712 TypeScript in `editor/vscode/src/` (2,834 impl + 2,382 unit + 1,496 e2e), and 1,085 lines of manifest/grammar/snippet/schema JSON. Measured with `wc -l`, `grep -c '#\[test\]'`, and `grep -n 'mod tests'` per file 

| verdict | feature | LOC | adoption |
|---|---|---|---|
| load-bearing | Live preview webview + bidirectional source sync | 757 | Applies to every .tmd. Six e2e tests pin it in a real Extension Host ( |
| load-bearing | Server lifecycle, message loop and panic isolation | 250 | Every session. Two live processes confirmed at audit time via `pgrep - |
| load-bearing | ProjectCache (stat-validated project walk) | 227 | Fires on a gesture, never per keystroke. Four consumers. The per-keyst |
| load-bearing | The vscode-languageclient over `taliesin lsp` | 177 | Every .tmd buffer. Confirmed live: `pgrep -f 'taliesin lsp'` returns a |
| load-bearing | taliesin/cellRegions (custom method) | 164 | Executable cells: author 11/40, manual 6/40, pin 36/105 (`grep -lE '^` |
| load-bearing | Live diagnostics (textDocument/publishDiagnostics) | 147 | author 40/40, manual 40/40, pin 105/105 |
| load-bearing | didChange coalescing (120 ms publish window) | 75 | Every keystroke in every buffer. Measured before shipping: 'one publis |
| load-bearing | UTF-16 position encoding at the wire boundary | 55 | Every position the server emits or receives. Its correctness matters o |
| load-bearing | RenderMemo (one-entry text-keyed render cache) | 40 | One entry, not an LRU, by design: 'the access pattern is many reads of |
| core | Completion (16 cursor contexts) | 1513 | The vocabulary it offers is 152 features (12 groups, `taliesin feature |
| core | Go-to-definition (F12), including cross-file | 250 | Constructs, author/manual/pin: includes 6/16/7, xrefs 10/18/31, citati |
| core | TextMate grammars (.tmd + markdown injection) | 237 | Every .tmd buffer. Gated: `tools/gates.sh:337-352` runs an OFFLINE Tex |
| core | Hover (5 target kinds) | 204 | Constructs it answers on, author/manual/pin: xrefs 10/18/31 docs, cita |
| core | Embedded-language completion forwarding (Pylance inside a cell) | 159 | Executable cells: author 11/40, manual 6/40, pin 36/105 documents. Two |
| core | Language configuration (list continuation, auto-closing pairs, w | 62 | Every keystroke in every .tmd. Two e2e tests pin it: 'continues a list |
| keep | Format Document (pipe tables + no-op whitespace) | 497 | Pipe tables: author 7/40, manual 27/40, pin 5/105 (`grep -lE '^\/.*\/' |
| keep | taliesin/renameFileEdits (rename repair) | 488 | Fires on a project with inbound references: includes author 6/40, embe |
| keep | taliesin/sectionEdit + the four structural commands | 397 | Applies to any document with headings: author 34/40, manual 39/40, pin |
| keep | Inlay hints (xref number, citation author-year, include line cou | 283 | Three kinds with three different supplies, author/manual/pin: xrefs 10 |
| keep | documentSymbol (heading outline + word counts) | 279 | author 34/40, manual 39/40, pin 81/105 documents contain at least one  |
| keep | rename + prepareRename (cross-reference anchors) | 178 | Anchors: author 17/40, manual 9/40, pin 30/105 documents (`grep -lE '\ |
| keep | Task provider + 2 problem matchers | 167 | One e2e test ('the task system is inert without a workspace folder, wh |
| keep | workspace/symbol (Ctrl+T across the project) | 140 | Answers over headings (author 34/40) and anchors (`{#id}`: author 17/4 |
| keep | foldingRange (structural folding) | 132 | Headings author 34/40, fenced divs author 13/40, code cells author 11/ |
| keep | onWillRenameFiles repair (the client half) | 79 | One e2e test ('renaming a chapter repairs the references pointing at i |
| keep | Check / Doctor / Restart Server / Show Server Log commands | 60 | `doctor` verified working from the CLI at audit time (it correctly rep |
| keep | Quick fixes (did-you-mean code actions) | 50 | Fires only where a diagnostic carried a `data.replacement`, i.e. a nea |
| keep | contributes.configurationDefaults | 20 | Every .tmd buffer. Pinned in a real Extension Host ('VS Code accepts t |
| narrow-keep | taliesin/insertEdit — paste and drop (image, table, BibTeX, asse | 732 | Constructs it produces, author/manual/pin: figures `![…](…)` 1/3/22, p |
| narrow-keep | documentLink on include/embed paths | 304 | Fires only on includes and embeds: author 6/40 + 2/40, manual 16/40 +  |
| narrow-keep | Paste and drop gesture providers (the client half) | 279 | Same constructs as taliesin-insert-edit (figures author 1/40, tables 7 |
| narrow-keep | Explorer decorations (worst check severity per .tmd) | 190 | Default ON. Runs `taliesin check --format json` over the whole project |
| narrow-keep | Run / Run Above CodeLens over executable cells | 151 | Executable cells: author 11/40, manual 6/40, pin 36/105. Five e2e test |
| narrow-keep | _site.yml JSON schema (yamlValidation) | 137 | Applies to every project root |
| narrow-keep | 12 .tmd snippets | 100 | Constructs they produce, author/manual/pin: callouts 7/22/10, code cel |
| narrow-keep | Get-started walkthrough (4 steps) | 60 | Unmeasured |
| narrow-keep | taliesin/projectOutline (custom method) | 57 | Meaningful only in an `_site.yml` project |
| narrow-keep | documentHighlight on cross-reference anchors | 52 | Fires only on anchor ids: author 17/40 documents contain a `{#prefix-` |
| narrow-keep | Insert Math Symbol (Ctrl+Alt+M QuickPick) | 45 | Math documents: author 9/40, manual 11/40, pin 24/105. Vocabulary sour |
| narrow-keep | MCP server definition provider | 20 | Five `mcpServer.taliesin.taliesin-companionTaliesin.log` files exist a |
| fold | The Taliesin sidebar (Outline / References / Figures & Tables) | 424 | State evidence (`~/.config/Code/User/globalStorage/state.vscdb`): the  |
| fold | Rasterized math hover (headless Chrome screenshot) | 407 | Math appears in author 9/40, manual 11/40, pin 24/105 documents (`grep |
| freeze | Extension Host e2e suite (46 tests, ungated) | 1496 | **Run by nothing automatically.** `grep -n 'test:e2e\/e2e' tools/gates |
| cut | taliesin/insertEdit — the Dataset drop kind | 264 | **author 0/40**, manual 5/40, pin 1/105 documents contain a `{{< datas |
| cut | 5 VS Code language-model tools | 255 | No e2e coverage. The five MCP server log files on this machine (`mcpSe |
| cut | Clickable file:line in terminal output | 211 | Fires only on terminal output the author is watching. No e2e test in t |
| cut | selectionRange (structural expand-selection) | 156 | Unmeasurable as a construct (it fires anywhere the cursor is). No test |
| cut | Status bar item (preview state + problem count) | 104 | Shown for `.tmd` work. 47 unit-test lines (statusbar.test.ts). No e2e  |
| cut | taliesin/projectRefs (custom method) + the References sidebar vi | 80 | Anchors: author 17/40, manual 9/40, pin 30/105. The view is one of thr |
| cut | taliesin/colorScheme (custom notification) | 30 | Its sole read site is `crates/server/src/lsp.rs:1281` |

### `site-model` — 46 features, ~15,214 LOC

crates/core/src/site/ is 15,214 lines across 23 files + config/mod.rs (measured: `cat crates/core/src/site/*.rs crates/core/src/site/config/mod.rs | wc -l`), split roughly 9,700 impl / 5,500 test. It is the largest single surface in the crate and it carries the whole "multi-page site" and "book" formats. The single most important scope finding: **the discoverability/SEO stack (feeds, sitemap, robo

| verdict | feature | LOC | adoption |
|---|---|---|---|
| load-bearing | Site discovery (_site.yml + page walk) | 900 | author 3 (site/, corpus/tech-blog, corpus/tarn) / manual 2 (docs/guide |
| load-bearing | Project-wide cross-references (@fig-, @sec-, @thm-) | 462 | author: tarn uses 6 (fig 1, sec 5), tech-blog uses 4 kinds (eq 2, fig  |
| load-bearing | Book projects (chapters:) | 460 | author 1 (corpus/tarn) / manual 2 (docs/guide, docs/internals) / pin 4 |
| load-bearing | .tmd → .html link rewriting | 292 | author 3/3 projects (every cross-page link in site/, tech-blog, tarn)  |
| load-bearing | Embedded-deck discovery + nav/search exclusion | 90 | author 2 (site/showcase.tmd → demo deck, corpus/course) / manual 1 (do |
| core | Listings (listing: contents/type/sort/max-items/id) | 270 | author 4 documents (corpus/tech-blog: blog.tmd, cv.tmd, index.tmd, pro |
| core | Cross-page full-text search index (Cmd-K) | 265 | Emitted for all 5 real projects (build logs report `search-index.js` f |
| core | Book topbar + off-canvas chapter drawer + prev/next | 230 | author 1 (corpus/tarn) / manual 2 (docs/guide, docs/internals) / pin 4 |
| core | Website navbar (nav: left/right, brand, search + settings button | 150 | author 2 (site/, corpus/tech-blog) / manual 0 (both manuals are books; |
| core | Book chapter/section numbering (N, N.1, N.1.1) | 145 | author 1 (corpus/tarn) / manual 2 / pin 4 |
| core | hero: front-matter landing header | 135 | author 4 documents (site/index.tmd, site/features.tmd, site/formats.tm |
| core | draft: front matter + DraftMode (preview shows, build excludes) | 120 | author 1 (corpus/tech-blog/posts/draft-example/index.tmd) / manual 0 / |
| keep | Cross-page link validation | 290 | Runs on every `check`/`build`/`preview` of every project |
| keep | Atom 1.0 feeds, one per uncapped dated listing | 252 | author 1 project (corpus/tech-blog → blog.xml 3,994 B + projects.xml 2 |
| keep | mounts: — serve another project under a URL prefix | 180 | author 1 (site/_site.yml, 7 entries: 2 docs books + 5 gallery exhibits |
| keep | Book landing-page auto Contents | 164 | author 1 (corpus/tarn index) / manual 2 (docs/guide, docs/internals) / |
| keep | OpenGraph + Twitter-card + canonical meta | 140 | twitter:card present on 17/17 tech-blog, 7/7 site, 26/26 guide, 15/15  |
| keep | 404 page (generated, or the author's own 404.tmd) | 80 | Generated for all 5 real projects (build logs: '404.html' for site/gui |
| keep | Site footer (footer: left/center/right + icon links) | 75 | author 2 (site/, corpus/tech-blog) / manual 0 / pin 0. Evidence: grep  |
| keep | logo: brand image | 60 | author 1 (corpus/tech-blog) / manual 0 / pin 1 (demo-book). Evidence:  |
| keep | Category badges + client-side filter chips | 55 | author: `categories:` on 12 tech-blog documents; `listing.categories:  |
| keep | 'Back to listing' post nav | 45 | author: fires on tech-blog's post/project pages (12 dated documents un |
| keep | Right-rail TOC gate (toc: + auto-gate + book scope warning) | 25 | `toc:` in _site.yml: author 2 (site: false, tech-blog: true) / manual  |
| keep | favicon: site icon | 20 | author 2 (site/, corpus/tech-blog) / manual 0 / pin 0. Evidence: grep  |
| narrow-keep | Category vocabulary lint (typo/case forks) | 116 | author 0 findings (verified: `taliesin check corpus/tech-blog` reports |
| narrow-keep | sitemap.xml + robots.txt | 56 | author 2 projects (tech-blog: sitemap 1,796 B + robots 73 B; site/: bo |
| narrow-keep | Bundled social-icon glyphs (icon: shorthand) | 40 | author: 3 of 8 names used |
| narrow-keep | Loose-deck warning | 20 | 0 firings across all 5 real projects (all decks in the repo are embedd |
| narrow-keep | Site-level raw includes (css:, head:, body-start:, body-end:) | 15 | author 0 / manual 0 / pin 0 |
| narrow-keep | output: build directory | 8 | author 1 (corpus/tech-blog: `output: _site`) / manual 0 / pin 1 (bayes |
| narrow-keep | page-layout: full | 6 | author 5 documents (site/index, site/showcase, site/features, site/for |
| narrow-keep | python: / r: project interpreter pins | 6 | author 0 / manual 0 / pin 0 |
| fold | The skim layer-cake projection (taliesin skim) | 472 | Two consumers: `taliesin skim <dir>` (531 lines of output for docs/gui |
| fold | 'Referenced by' backlinks | 232 | author 6 pages (corpus/tarn) / manual 1 page (docs/guide) / blog 0. Ev |
| fold | Book download archive naming + drawer link | 33 | Fires for every book: `taliesin-user-guide.zip` (2,050,572 B) in docs/ |
| fold | publish: deploy block (provider/project/gate) | 15 | author 0 / manual 0 / pin 0 |
| fold | theorems: book-wide numbering policy in _site.yml | 10 | author 0 / manual 0 / pin 1 (corpus/theorem-book). Sub-keys in front m |
| freeze | manifest.webmanifest + app icons | 303 | Emitted unconditionally: rel="manifest" on 17/17 tech-blog, 5/7 site,  |
| freeze | llms.txt + llms-full.txt | 258 | author 2 projects (tech-blog: llms.txt 2,793 B + llms-full.txt 45,393  |
| freeze | Project-wide bibliography: in _site.yml | 171 | author 0 / manual 0 / pin 1 (corpus/shared-bib). Evidence: grep `^bibl |
| freeze | schema.org JSON-LD (BlogPosting / ScholarlyArticle / WebSite+Per | 160 | author: 12/17 tech-blog pages, 1/7 site pages / manual 0 (no `url:`) / |
| freeze | Cross-page hover previews (hover-index.js) | 101 | MEASURED COVERAGE: corpus/tarn ships a 1-entry index (489 B) against 8 |
| cut | Generated OpenGraph social cards (1200x630 PNG per page) | 551 | author 2 projects: tech-blog writes 2,045,649 B of PNGs across 16 og:i |
| cut | 'Cite this' export box (BibTeX / CSL-JSON / RIS) | 382 | author 0 / manual 0 / pin 2. MEASURED: needle grep `<aside class="tali |
| cut | Google Scholar / Highwire citation_* meta | 72 | author 0 / manual 0 / pin 2 (corpus/cite-this/paper.tmd + one sibling) |
| cut | hero: image / image-alt (two-column hero) | 20 | author 0 / manual 0 / pin 0. Evidence: features JSON |

### `reader-runtime` — 67 features, ~15,829 LOC

Everything a browser runs: 15,829 first-party lines (web-client/ 3,627 JS + assets/js first-party 5,048 + assets/css 3,433 + render/{deck,theme,print,pyodide,client_lang}.rs 1,940 + code-enhance/ 1,781), plus ~4.6 MB of vendored payload (mermaid 3.57 MB, pyodide 16 MB on disk, d3 280 KB, Plot 207 KB, paged.js 503 KB). MEASURED, not grepped: four real builds to scratchpad (`TALIESIN_NO_CACHE=1 tali

| verdict | feature | LOC | adoption |
|---|---|---|---|
| load-bearing | The page stylesheet (base.css + site.css + dark.css) | 1981 | author 40 / manual 40 / pin 105 |
| load-bearing | Preview block mount + incremental op application | 547 | author 40 / manual 40 / pin 105 |
| load-bearing | {js} cell runtime: language registry, cell wrapper, reactive gra | 400 | author 8 (6 corpus tech-blog + 2 site) / manual 5 (3 internals + 17 do |
| load-bearing | Ctrl/Cmd-click to source + reverse editor sync | 302 | author 40 / manual 40 / pin 105 |
| load-bearing | Enhancer registry (window.taliEnhancers) — the public extension  | 116 | author 40 / manual 40 / pin 105 |
| load-bearing | Pre-paint theme + code-visibility bootstrap | 110 | author 40 / manual 40 / pin 105 |
| load-bearing | The --tali-* design-token system | 104 | author 40 / manual 40 / pin 105 |
| core | Cmd/Ctrl-K command palette + full-text search | 1101 | author 40 / manual 40 / pin 105 |
| core | Figure / diagram / video lightbox | 292 | author: 5/19 corpus tech-blog docs and 5/17 external tech-blog docs ha |
| core | Hover preview cards for citations and cross-references | 244 | author: measured 4/16 built tech-blog pages and 6/14 tarn pages carry  |
| core | TOC scrollspy | 199 | author 40 / manual 40 / pin 105 |
| core | Settings menu (the shared popover host) | 128 | author 40 / manual 40 / pin 105 |
| core | Diagnostics panel (render / include / kernel issues) | 98 | author 40 / manual 40 / pin 105 |
| core | Keyboard-scrollable overflow regions (WCAG 2.1.1) | 59 | author 40 / manual 40 / pin 105 |
| core | Reader theme picker (auto / light / dark) | 58 | author 40 / manual 40 / pin 105 |
| core | Heading + float anchor copy-links | 56 | author 40 / manual 40 / pin 105 |
| core | Code copy button | 40 | author 40 / manual 40 / pin 105 |
| core | Skip-to-content link | 33 | author 40 / manual 40 / pin 105 |
| core | Vendored d3 (drawing global for {js} cells) |  | author 8 / manual 5 / pin 29. **External real: 13/15 fl-weather explai |
| keep | Deck engine core: grid camera, navigation, layout, lifecycle | 1100 | author 1 (site/demo.tmd, the marketing demo of the deck itself) / manu |
| keep | Deck fragments / incremental steps | 261 | author 0 / manual 2 (docs/guide, {.incremental} in 2 files) / pin: {.i |
| keep | Book drawer per-chapter section outline | 249 | author 14 (all tarn pages carry #tali-book-drawer-btn) / manual 15 (al |
| keep | Preview TOC rebuild + mobile sheet driving | 218 | author 40 / manual 40 / pin 105 (any doc with enough headings). In the |
| keep | Preview control bar (theme toggle + click-to-source hint + dev m | 186 | author 40 / manual 40 / pin 105 |
| keep | Mobile pull-up contents sheet | 184 | author 40 / manual 40 / pin 105 on TOC pages (8/16 built tech-blog pag |
| keep | Execution progress chip (idle/busy dot, k/N bar, tab title + fav | 168 | author: only documents with executable cells |
| keep | Live accessibility audit of the rendered output | 129 | author 40 / manual 40 / pin 105 |
| keep | User-initiated video playback + single active player | 118 | author 2 (site/features.tmd, site/index.tmd |
| keep | Listing category filter chips | 110 | author 2 (corpus/tech-blog blog.tmd + index.tmd |
| keep | Shareable control state in the URL fragment | 110 | author: rides every page with an input |
| keep | Preview deck mount + structural re-mount | 100 | author 1 (site/demo.tmd) / manual 2 (docs/guide/demo.tmd, tour.tmd) /  |
| keep | Deck URL hash position (replaceState) | 99 | author 0 / manual 0 / pin 0 opt in |
| keep | Extension / file themes (theme: <file>.css or _extensions/<name> | 90 | author 1 (features corpus.json theme=1) / manual 0 / pin 0 in-repo |
| keep | Reader keyboard shortcuts (? / and arrow chapter nav) | 82 | author 40 / manual 40 / pin 105 for ?; arrow nav needs .tali-book-prev |
| keep | Tabbed panels (::: {.panel-tabset}) | 72 | author 3 (corpus/tarn install.tmd + quickstart.tmd, site/showcase.tmd |
| keep | Fatal-error overlay | 70 | author 40 / manual 40 / pin 105 |
| keep | Per-cell execution state decoration | 52 | author: the 15/38 external real docs with {python}; pin 40 corpus docs |
| keep | Live streaming cell output | 51 | author: any {python}/{r} doc with a slow cell |
| keep | Vendored Observable Plot (drawing global for {js} cells) |  | author: 3/17 external tech-blog documents call Plot. **ZERO of the 15  |
| narrow-keep | Deck on-screen chrome: control menu, progress bar, nav arrows | 407 | author 0 / manual 0 / pin 0 opt in |
| narrow-keep | Reading position resume + book 'Continue reading' pill | 141 | author 40 / manual 40 / pin 105 |
| narrow-keep | Section annotations | 140 | author 40 / manual 40 / pin 105 |
| narrow-keep | Scroll-driven code walkthrough (::: {.code-walkthrough}) | 118 | author 2 (corpus/tarn/quickstart.tmd, site/showcase.tmd) / manual 2 (1 |
| narrow-keep | Scroll-driven sticky-stage scenes (::: {.scrolly}) | 89 | author 1 (site/showcase.tmd) / manual 1 (one docs/guide file) / pin 4  |
| narrow-keep | Per-slide backgrounds | 71 | author 0 / manual UNMEASURED / pin: background-color 3/9 repo decks, b |
| narrow-keep | Reader show/hide code switch | 68 | author: pages with executed cells |
| narrow-keep | Deck slide number chip | 55 | author 0 / manual 0 / pin 0 |
| narrow-keep | Deck black-screen / pause (B or .) | 45 | author 0 / manual 0 / pin 0 |
| narrow-keep | Persistent deck chrome (footer: / logo:) | 40 | author 0 / manual 0 / pin: `footer:` 2 corpus docs (features corpus.js |
| fold | Shared modal focus trap + focusables helper | 40 | author 40 / manual 40 / pin 105 |
| fold | Built-in registration block | 18 | author 40 / manual 40 / pin 105 |
| fold | window.TaliesinDeck events + registerPlugin | 17 | author 0 / manual 0 / pin 0 third-party plugins. In-tree consumer: web |
| freeze | The num numerics namespace | 420 | author 0 / manual 1 (one docs/guide file) / pin 3 corpus docs. **ZERO  |
| freeze | Deck overview: a free map camera over the whole grid | 309 | author 0 / manual 0 / pin 0 |
| freeze | Presenter (speaker) window + cross-window sync + teleprompter | 255 | author 0 / manual 2 (docs/guide {.notes} in 2 files) / pin 2 (corpus/d |
| freeze | Mobile slide-feed | 241 | author 0 / manual 0 / pin 0 documents opt in |
| freeze | Print/PDF browser track (paged.polyfill + print.css) | 241 | author 0 / manual 0 / pin: crates/core/tests/print_page.rs + crates/se |
| freeze | {glsl} cell language | 207 | author 0 / manual 1 (docs/guide has one {glsl}) / pin 1 (corpus/reacti |
| freeze | Mermaid diagram runtime (lazy loader + vendored library) | 144 | author 0 (0/16 built tech-blog, 0/14 tarn, 0/6 site pages carry class= |
| freeze | Deck auto-animate (tween matched elements between slides) + magi | 97 | author 0 / manual 2 (docs/guide, {.magic-move} in 2 files) / pin 2 (co |
| freeze | 'Cite this' box format switcher | 83 | author 0 / manual 0 / pin 0 measured in the four scratchpad builds (ze |
| freeze | Cross-re-run cell state (tali.state / tali.setState) | 28 | author 0 / manual 1 (one docs/guide file) / pin 1 corpus doc. ZERO of  |
| cut | Rich output helpers (tali.tex, tali.table) | 533 | author 0 / manual 1 (one docs/guide file uses tali.tex and tali.table) |
| cut | {pyodide} cell language + the vendored Pyodide runtime | 530 | author 0 / manual 1 (one docs/guide file) / pin 1 (corpus/reactive/pyo |
| cut | Offline QR encoder + 'Share this view' | 323 | author 0 / manual 0 / pin 0 |
| cut | The two non-form controls (type="point", type="animate") | 250 | author 0 / manual 1 (one docs/guide file has each) / pin 2 corpus docs |
| cut | Session revision digest | 162 | author 0 measured / manual 0 / pin 0 |

### `build-exec` — 50 features, ~32,555 LOC

The build / execution / caching / publishing surface is 32,555 LOC (32,090 Rust across 30 modules in `crates/server/src` — measured by `wc -l` over build*.rs, exec.rs, kernel.rs, warm_pool.rs, freeze.rs, minify.rs, image_opt.rs, pdf.rs, publish.rs, headless_js.rs, query.rs, check.rs, zip.rs, math_image.rs, interpreter.rs, runtime_dirs.rs, http1.rs, protocol.rs, session.rs, preview_diag.rs, run_*.r

| verdict | feature | LOC | adoption |
|---|---|---|---|
| load-bearing | Multi-page site dev server (preview <dir>) | 2724 | author 64/64 preview invocations (17 `tali preview study/`, 13 `tali p |
| load-bearing | Warm Jupyter kernel (ZMQ) | 2522 | author 17 docs use {python} + 0 use {r} / manual 1 / pin 23 python + 8 |
| load-bearing | build <dir> → _site/ multi-page | 818 | author 6 of 7 `build` invocations were `tali build .` / `tali build do |
| load-bearing | Block-diff → BlockOp → websocket broadcast | 600 | author: every one of the 64 preview invocations / manual n/a / pin too |
| load-bearing | Cumulative-hash freeze cache (_freeze/) | 498 | author: fires on every build/preview of the 17 author docs with {pytho |
| load-bearing | LAN access guards (origin, Host allowlist, session token) | 416 | author: active on every preview (loopback path) / manual notes/2026-07 |
| load-bearing | Cell caps: silence timeout, wall-clock timeout, output caps | 200 | author: implicit on every executed cell / manual documented in guide/r |
| load-bearing | MAX_WARM_PAGES executor LRU (the one standing freeze) | 197 | author: fires on any site preview past 6 pages (docs/guide is 25, corp |
| load-bearing | Execution planner (warm reuse vs cold replay) | 120 | author: every code-cell edit in 64 preview sessions / manual n/a / pin |
| load-bearing | Byte-reproducible builds across processes |  | author: implicit on every build / manual notes/2026-07-22-ap8-determin |
| core | check <file/dir> — static kernel-free lint | 2680 | author 5 invocations (grep -acE '(taliesin/tali)(-stable)? +check' ~/. |
| core | Interpreter resolution + kernel-package probe | 814 | author: implicit on all 17 python docs; explicit `python:`/`r:` in _si |
| core | Live cell-state / output streaming to the preview | 250 | author: fires on every executed preview / manual 0 dedicated pages / p |
| core | build <file.tmd> → self-contained HTML | 250 | author 1 real invocation on an EXTERNAL document (`taliesin build 17-t |
| core | Shared hashed asset bundle + conditional bundling | 220 | author: every site build / manual: measured on a 26-page docs/guide bu |
| core | build --strict / publish strict-by-default | 80 | author 1 real invocation (`taliesin build 17-the-proposal.tmd … --stri |
| keep | run <file.tmd> — execute cells in the terminal | 1929 | author **0 invocations** (shipped 2026-08-01/02, one day old |
| keep | Eager warm kernel pool (forkserver preload) | 1621 | author: fires on every site preview and every site build with Python / |
| keep | Build-time AVIF derivatives (<picture> srcset) | 552 | author 10 rasters in corpus/tech-blog, 1 in site/, 1 in docs/ (find -n |
| keep | build --out <dir> (portable folder) | 470 | author 0 direct `--out` invocations in history (`grep -ac -- '--out' ~ |
| keep | run --interrupt / Ctrl-C (stop a run mid-sequence) | 238 | author 0 (one day old) / manual 1 (guide/reference/cli) / pin 7 inline |
| keep | External-reference detection at build (the offline guarantee) | 219 | author: the one real single-file build in history reports it (`… 4.6 M |
| keep | Parallel page rendering with byte-identical output (--jobs N) | 210 | author 0 explicit `--jobs` invocations (grep -ac -- '--jobs' ~/.zsh_hi |
| keep | Live check-superset validation in the preview dev menu | 206 | author: fires in all 64 preview sessions / manual: notes/2026-07-18 DX |
| keep | Single-instance preview takeover (identity handshake) | 200 | author: fires implicitly on many of the 64 preview invocations (13 are |
| keep | build --format json (structured build diagnostics) | 60 | author 0 hand-typed / manual guide/reference/cli / pin crates/server/t |
| keep | --no-exec / TALIESIN_NO_EXEC | 40 | author 0 invocations / manual 6 pages (troubleshooting, getting-starte |
| keep | sweep_stale (delete output the build no longer produces) | 36 | author: fires on every site build / manual 0 pages / pin 1 dedicated i |
| narrow-keep | doctor [dir] — environment self-audit | 624 | author **0 invocations** / manual 1 page (guide/reference/cli) / pin 1 |
| narrow-keep | Memory-aware build concurrency planner | 599 | author: fires on every auto site build / manual internals/execution /  |
| narrow-keep | Stale /tmp runtime-dir sweep | 277 | author: fires on every server start / manual 0 pages / pin 4 inline te |
| narrow-keep | Offline book download (hand-rolled ZIP writer) | 263 | author 0 books among the author documents (corpus/tarn is a book but i |
| narrow-keep | check --explain <CODE> (offline rustc-style diagnostic explanati | 140 | author 0 invocations / manual: the generated docs/DIAGNOSTICS.md / pin |
| narrow-keep | #/ cache: false — opt a cell out of the freeze cache | 60 | author **0** / manual 2 lines in docs/guide/reference/cell-options.tmd |
| narrow-keep | preview --host (LAN exposure + terminal QR code) | 60 | author **1 invocation ever, under the old name** |
| fold | Single-document dev server (preview <file.tmd>) | 2753 | author 0 / manual 0 / pin ~4 tests. `grep -acE '(taliesin/tali)(-stabl |
| fold | mounts: is preview-only; the static build needs a shell script | 120 | author 1 project sets mounts: (site/_site.yml, the marketing site) / m |
| fold | Shared-passcode HTTP Basic gate (_middleware.js) | 103 | author 1 gated deploy (2026-07-31, sports-competition) / manual 3 page |
| fold | symbols <file.tmd> — list cross-reference targets | 90 | author **0 invocations** / manual 1 page / pin 1 test file. Machine co |
| freeze | Headless-Chrome {js} cell observation for read --run | 1099 | author 0 (`read` 3 invocations total, none with --run-js) / manual 4 p |
| freeze | Dependency-free CSS/JS minifier for the asset bundle | 1016 | author: every site build / manual none / pin 28 inline tests + a Node  |
| freeze | pdf <file.tmd> — paginated PDF rendered FROM the built HTML | 697 | author **0 invocations** (grep -acE '(taliesin/tali)(-stable)? +pdf' ~ |
| freeze | build --bare (zero-JS, CSS-only single document) | 60 | author **0** (`grep -a -- '--bare' ~/.zsh_history` → 2 hits, both `git |
| freeze | Build-time Pyodide payload write | 12 | author 0 / manual 0 (docs-internals shows 0 pyodide) / pin 1 (corpus/r |
| cut | publish <dir> → Cloudflare Pages (wrangler direct upload) | 761 | author **2 invocations, both on 2026-07-31, both against an external p |
| cut | Rasterized math for the editor hover | 377 | author unmeasured (LSP hover leaves no trace; the no-telemetry stance  |
| cut | publish --init (one-time Cloudflare setup) | 130 | author 1 invocation ever (2026-07-31) / manual 2 pages / pin 5 tests i |
| cut | render <file.tmd> — full HTML page to stdout (static, no executi | 70 | author **0 invocations** / manual 1 page (guide/reference/cli) / pin 1 |
| cut | blocks <file.tmd> — list block ids + sourcepos | 50 | author **0 invocations** / manual **0 documentation pages** (the only  |
| cut | #/ fig-export: — write a cell's figure to PDF/PNG files | 40 | author **0** / manual **1** (docs/guide/using/code.tmd:173, the page t |

### `machine-facing` — 24 features, ~8,990 LOC

The agent/tooling surface is ~8,990 LOC of Rust+TS+assets (~7% of the tree): 8 CLI subcommands (read, map, skim, features, symbols, blocks, schema, vocab), a stdio MCP server, two llms.* sidecars, and five separate documents teaching the same agent loop. Measured adoption is the finding: over 6,019 shell-history entries since 2026-01-30 the author invoked `preview` 62x, `build` 7x, `check` 5x, `re

| verdict | feature | LOC | adoption |
|---|---|---|---|
| keep | `taliesin features <file/dir>` — the adoption report | 1228 | author 0 hand invocations (it is one day old) / manual 2 pages / pin 1 |
| keep | `taliesin read <file>` — plain-text projection | 1124 | author 3 (the only machine command with real hand use: `tali read`, `t |
| keep | The 224-entry math command vocabulary | 591 | author unmeasured (an editor completion leaves no shell trace) / manua |
| keep | `taliesin schema` — JSON Schema emission | 567 | author 1 (`tali schema`, one exploratory run) / manual 12 pages mentio |
| keep | `taliesin map <dir>` — project outline + xref graph | 404 | author 0 hand invocations / manual 3 pages / pin 1 (map_cli.rs, 121 LO |
| keep | `.taliesin/` + schema modeline written by `init` | 35 | author 0 (`init` has 0 invocations in ~/.zsh_history) / manual 2 / pin |
| narrow-keep | `taliesin vocab` — editor vocabulary JSON | 802 | author 1 (`tali vocab`) / manual 5 pages / pin: golden-locked in vocab |
| narrow-keep | Generated `AGENTS.md` onramp + `init` scaffolding | 477 | author 0 (`init` never invoked) / manual 7 pages mention AGENTS / pin  |
| narrow-keep | `taliesin read --run` — headless-Chrome `{js}` observation | 247 | author 0 / manual 2 (cli.tmd's `read --run` paragraphs, AGENTS.md) / p |
| narrow-keep | `taliesin read <dir>` — whole-book projection | 215 | author 0 / manual 1 (cli.tmd) / pin 1 (read_book.rs, 85 LOC). Evidence |
| narrow-keep | `taliesin read --format json` — the machine form | 140 | author 0 / manual 3 (cli.tmd, agents.tmd, AGENTS.md) / pin 2 (read_run |
| narrow-keep | `llms.txt` — the curated site map | 90 | author 2 / manual 0 / pin 2. Measured by url-gating: only 4 of 17 `_si |
| narrow-keep | VS Code `mcpServerDefinitionProvider` registration | 23 | author unmeasured (the companion IS installed: `~/.vscode/extensions/t |
| fold | `taliesin symbols <file>` — cross-reference targets | 306 | author 0 / manual 3 pages / pin 1 (symbols_cli.rs, 168 LOC). Evidence: |
| fold | VS Code language-model tools (5 duplicate tools) | 236 | author unmeasured / manual 0 docs pages / pin: drift-gated against mcp |
| fold | `llms-full.txt` — the full-prose dump | 170 | author 2 / manual 0 / pin 2 (same url-gating as llms.txt: site/, corpu |
| fold | `docs/guide/using/agents.tmd` — the agent workflow page | 120 | author n/a (it is prose) / manual 1 page of 25 in docs/guide / pin: co |
| fold | `taliesin blocks <file>` — block-model debug dump | 111 | author 0 / manual 1 (cli.tmd:493-509, plus a row in the command table  |
| freeze | `taliesin mcp` — stdio MCP server | 369 | author 1 (`tali mcp`, run bare once |
| freeze | `taliesin read --run` — executed-cell reporting (python/r) | 355 | author 0 / manual 3 (cli.tmd, using/agents.tmd, AGENTS.md) / pin 1 (re |
| cut | `taliesin skim <dir>` — the layer-cake stream | 1400 | author 0 / manual 3 pages / pin 2 (skim_cli.rs 263 LOC + tarn.rs:408). |
| cut | `editor/claude-code/skills/taliesin/SKILL.md` | 136 | author 0 (nothing installs it; it is not in ~/.claude/skills and the a |
| cut | Repo-root `AGENTS.md` (byte-identical duplicate) | 68 | author: it is the repo's own onramp, so every agent session in this tr |
| cut | `map`'s `words` + `headings` per page | 30 | author 0 / manual 1 (cli.tmd's "Each page also carries `words` and `he |

