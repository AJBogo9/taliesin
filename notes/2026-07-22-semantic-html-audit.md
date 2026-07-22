# Audit: semantic-HTML / document-model validity (perspective AP9)

Date: 2026-07-22. Perspective: AP9 from the backlog "Audit perspectives" section
(semantic-HTML / document-model validity of the emitted output). Run as a single-
perspective session alongside two live sessions (a feature session on DX17b and an audit
session on AP2 fuzzing, both now in isolated worktrees), so it touches no source and
builds nothing from the working tree. Evidence: rendered all 84 corpus `.tmd` documents
(and built one full website) with the frozen `taliesin-stable` binary
(`/home/bogo/.local/bin/taliesin-stable`, Jul 7), then parsed the output with two
browser-equivalent parsers (a raw stack tokenizer that does NOT auto-fix, plus `html5lib`)
to find invalid nesting, duplicate ids, malformed figures/tables/lists, unlabelled
sections, and broken heading outlines. No `cargo build`, no port, no browser drive, so it
did not contend with the two building sessions.

## Why this perspective

The tool's job is to emit HTML. Every prior round judged behavior (does the feature work,
does it look right, is it accessible in affordance) but nobody validated the STRUCTURE of
the generated markup against what a browser and assistive tech actually require: correct
element nesting, one document root heading, well-formed figures and tables, labelled
landmarks. The a11y polish round added affordances (h1 injection when absent, `<time>`,
`<th scope>`, ARIA on chrome); this perspective checks the underlying document model those
affordances sit on.

## Executive summary

The render pipeline emits structurally valid HTML. Across 84 corpus documents plus a full
website build, there were ZERO genuine invalid-nesting emissions, ZERO per-page duplicate
ids, well-formed figures (exactly one `<figcaption>` each), labelled deck slide
`<section>`s, and valid list / table / definition-list structure. The single nesting hit
was in `corpus/diagnostics/a11y.tmd`, which deliberately contains raw inline
`<div role="button">` in prose as a diagnostics fixture (the file says so and is exempt
from the clean-render guards), not a pipeline bug. This is a strong positive bill of
health and the most valuable output of this pass: a later reviewer can trust the emitted
structure and does not need to re-audit it.

One real finding remains, and it recurs across nearly every titled multi-section document:
pages emit MULTIPLE `<h1>` elements, breaking the single-root document outline that
assistive-technology heading navigation and the tool's OWN a11y logic assume.

## Findings

### HTML-1 (medium): pages emit multiple `<h1>`, breaking the document outline

A titled document emits a title-block `<h1 class="title">` (inside
`<header class="tali-title-block">`) AND renders every author `#` heading as `<h1>` (the
markdown level-1 mapping, `emit.rs:15` emits `<hN>` at the author's raw level). Because
the corpus's authoring convention uses `#` for top-level sections, the two stack up.

Evidence (built website, `corpus/bayesian-website`, all inside one `<main>`):

```
h1  "Bayesian Analysis of Aviation Accidents"   (header.tali-title-block)
h1  "Introduction"                              (main)
h1  "Data Description"                           (main)
h1  "Data Modeling"                              (main)
... 11 section <h1> siblings in total, 12 <h1> on the page
```

The same shape recurs on real blog posts: `corpus/posts/fourier-transform` emits 5 `<h1>`
(title + `# Sound is a sum of sinusoids`, `# The formula`, ...), `pca-geometry` emits 7.
Across the corpus, twenty documents emit 2 to 12 `<h1>` each. The visual rendering is fine
(CSS sizes headings), so this is invisible to sighted users; the damage is to the SEMANTIC
outline: a screen-reader user navigating by heading gets a flat list of a dozen "level 1"
headings instead of a title with nested sections, and any outline/reading-order tool sees
twelve competing document roots.

This is not merely a style opinion: the tool already declares an intent for exactly one
`<h1>`. The a11y polish round (PA-H2) injects a hidden `<h1>` ONLY when the body has none,
specifically to avoid a second h1. That intent is undermined the moment the body carries
its own `#` headings, which is the common case. So the tool contradicts its own single-h1
goal.

Fix direction (this is the known-but-gated "heading-demotion" idea from the
2026-07-11 website-design audit; AP9 supplies the concrete evidence and scope): when a
document renders a title-block `<h1>`, demote author heading levels by one for the HTML
document view (`#` becomes `<h2>`, `##` becomes `<h3>`, ...), so the outline is single-
rooted. The change is well-scoped and safe on two axes I verified:

- **Anchor ids survive.** Heading ids come from `slugify(visible_text)` (`mod.rs:1496`),
  deduped by text (`mod.rs:520-534`), never from the level, so demotion does not move any
  `@sec-` / `#id` target or break an in-page link.
- **But decks must be exempt.** The deck engine groups blocks into slide `<section>`s BY
  heading level (`deck.rs`), so heading level is structurally load-bearing there. Demotion
  is a prose/site-HTML concern only; it must run after (or not touch) deck slide grouping,
  and per-slide multiple headings on a deck are fine.

Other scoping: a document with NO title block should keep its `#` as `<h1>` (that single
h1 is correct, and PA-H2 already handles the no-heading case); the demotion applies only
when the title block provides the root h1. This edits `crates/core/src/render` and shifts
many corpus render snapshots, so it wants an owner ruling on the model (demote vs. leave)
before building, which is exactly why it was gated. Size: M (plus a wide but mechanical
snapshot update). Owner-gated.

## Verified valid (honest negatives, so later audits can skip them)

Measured across 84 rendered corpus documents plus a full `corpus/bayesian-website` build:

- **No invalid element nesting.** A raw stack tokenizer (no browser auto-fix) found zero
  block-level elements inside a `<p>`, zero `<p>` inside `<p>`, zero `<a>` inside `<a>`,
  zero interactive elements inside an `<a>`. `html5lib` produced zero empty-`<p>` fixup
  artifacts. The only hit was the intentional `corpus/diagnostics/a11y.tmd` fixture.
- **No per-page duplicate ids.** The heading-slug dedup (`mod.rs:520-534`) holds; every
  rendered page had a unique id set.
- **Figures are well-formed.** Every `<figure>` across the corpus (33 in the flagship doc
  alone) carries exactly one `<figcaption>`. No orphan or double captions.
- **Deck slides are labelled.** Every slide `<section>` (`.tali-slide` / with a
  `data-slide-anchor`) has an accessible name via a heading or label. No unnamed sections.
- **Lists and definition lists are valid.** No `<li>` outside a list container, no
  `<dt>`/`<dd>` outside a `<dl>`, no `<dl>` with foreign children.
- **Landmarks present.** Built pages carry `<header>` and `<main>`; content sits inside
  `<main>`.
- **No heading-level skips.** Ignoring the multiple-h1 issue, no document jumps a heading
  level by more than one on the way down.

## Build-ready items to fold into backlog.md "Open work"

- **HTML-1 (M, owner-gated):** heading-demotion. When a title-block `<h1>` is present,
  demote author heading levels by one in the HTML document view so each page has a single
  root `<h1>` and a nested outline. Anchor ids are level-independent (safe); decks must be
  exempt (levels drive slide grouping); no-title docs keep `#` as `<h1>`. Aligns with the
  gated idea from the 2026-07-11 website-design audit. `crates/core/src/render`. Needs an
  owner ruling on the model before building (it reshapes the outline of most corpus docs).

## Method notes for the next AP9-style run

- The fastest sweep is: `taliesin-stable render` every `corpus/**/*.tmd`, then run a raw
  stack tokenizer (stdlib `html.parser`, no auto-fix) for nesting + a `html5lib` parse for
  the empty-`<p>` fixup signal. Agreement between the two is high-confidence.
- Exclude `corpus/diagnostics/*`: those are intentionally malformed and exempt from the
  clean-render guards, so any structural hit there is a fixture, not a bug.
- What this automated pass did NOT cover, and a deeper manual review could add: `<article>`
  for posts (already tracked as PA-M2 residual), `lang` attribute correctness, redundant or
  conflicting ARIA roles, and reading-order vs. visual-order for the two-column and margin
  layouts.
