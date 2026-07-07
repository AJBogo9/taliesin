# samples/ — the standalone format test set

One comprehensive document per Taliesin HTML format, each **standalone-previewable**,
for honing every format's UX in isolation. Integration between them (mounts, embeds)
is deliberately deferred: polish each on its own first, then wire them together.

This dir holds the two documents that did not already exist as standalone projects
(`deck.qmd`, `paper.qmd`); the website and the two books live in their canonical
locations and are listed here so the whole test set is in one place.

## The five entities

| # | Format | Path | Preview |
|---|--------|------|---------|
| 1 | Marketing website (multi-page) | `site/` | `taliesin preview site` |
| 2 | User reference (book) | `docs/guide/` | `taliesin preview docs/guide` |
| 3 | Developer reference (book) | `docs/internals/` | `taliesin preview docs/internals` |
| 4 | Slide deck (all slide features) | `samples/deck.qmd` | `taliesin preview samples/deck.qmd` |
| 5 | Research paper (single page) | `samples/paper.qmd` | `taliesin preview samples/paper.qmd` |

(Each command also works as `cargo run -p taliesin-server -- preview <path>`. The deck
and the paper execute `{python}` cells, so they need a Python with `ipykernel` +
`numpy` + `matplotlib` on `TALIESIN_PYTHON`; without it those cells render as source.)

## What each one exercises

**1. Marketing website (`site/`)** — `hero:` block, top navbar + footer, `.feature-grid`
cards, `{{< video >}}` screencasts, light/dark toggle, OpenGraph, Cmd-K search. The
website chrome and the editorial default theme.

**2 & 3. Books (`docs/guide/`, `docs/internals/`)** — left chapter sidebar with
`part:` groups, chapter + section numbering, sticky right-rail sub-TOC, prev/next
nav, Cmd-K search, callouts, server-side code highlighting, math, and (internals)
numbered mermaid figures with cross-references.

**4. Slide deck (`samples/deck.qmd`)** — "Decisions in the Room", a business-value
story (the deck-assembly tax, and how answering live closes the deal) that exercises
**every** slide feature on Taliesin's own engine: auto-animate, incremental fragments
and a `.fragment`, code line-stepping (`code-line-numbers`), magic-move, **live
`{python}` cells that compute the business charts on the slide**, an **interactive `{js}`
slider** the audience can drag, math, a two-column layout (`layout-ncol`), callouts, a
table, a mermaid diagram, per-slide backgrounds (colour, gradient, and a local offline
image in `assets/`), vertical sub-slide stacks, speaker notes (`S`), PDF export
(`Ctrl/Cmd-P`), reader mode, drawing (`D`), and the menu (`M`).

**5. Research paper (`samples/paper.qmd`)** — a single page with title/subtitle/author/
date, a right-rail TOC, an abstract, numbered display equations (`@eq-`) + aligned
environments + inline math, a captioned numbered table (`@tbl-`), a **live matplotlib
figure** (`@fig-`), callouts (incl. a collapsible one), citations + an auto-generated
References section (`references.bib`), a footnote, and a margin sidenote. Section
cross-references (`@sec-`) throughout.

## Notes for honing

- Use these as the fixtures while improving the **renderer** (CSS + engine): a fix to
  the deck engine is judged against `deck.qmd`, a fix to single-page layout against
  `paper.qmd`, and so on.
- Drive click-to-source + live reload from these standalone previews (they are fully
  live), not from a mounted view.
