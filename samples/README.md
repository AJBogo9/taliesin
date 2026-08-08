# samples/ — the standalone format test set

One comprehensive document per Taliesin HTML format, each **standalone-previewable**,
for honing every format's UX in isolation. Integration between them (mounts, embeds)
is deliberately deferred: polish each on its own first, then wire them together.

This dir holds the one document that did not already exist as a standalone project
(`paper.tmd`); the website and the two books live in their canonical locations and are
listed here so the whole test set is in one place.

## The four entities

| # | Format | Path | Preview |
|---|--------|------|---------|
| 1 | Marketing website (multi-page) | `site/` | `taliesin preview site` |
| 2 | User reference (book) | `docs/guide/` | `taliesin preview docs/guide` |
| 3 | Developer reference (book) | `docs/internals/` | `taliesin preview docs/internals` |
| 4 | Research paper (single page) | `samples/paper.tmd` | `taliesin preview samples/paper.tmd` |

(Each command also works as `cargo run -p taliesin-server -- preview <path>`. The paper
executes `{python}` cells, so it needs a Python with `ipykernel` + `numpy` +
`matplotlib` on `TALIESIN_PYTHON`; without it those cells render as source.)

## What each one exercises

**1. Marketing website (`site/`)** — `hero:` block, top navbar + footer, `.feature-grid`
cards, light/dark toggle, OpenGraph, Cmd-K search. The
website chrome and the editorial default theme.

**2 & 3. Books (`docs/guide/`, `docs/internals/`)** — left chapter sidebar with
`part:` groups, chapter + section numbering, sticky right-rail sub-TOC, prev/next
nav, Cmd-K search, callouts, server-side code highlighting, math, and (internals)
numbered mermaid figures with cross-references.

**4. Research paper (`samples/paper.tmd`)** — a single page with title/subtitle/author/
date, a right-rail TOC, an abstract, numbered display equations (`@eq-`) + aligned
environments + inline math, a captioned numbered table (`@tbl-`), a **live matplotlib
figure** (`@fig-`), callouts (incl. a collapsible one), citations + an auto-generated
References section (`references.bib`), a footnote, and a margin sidenote. Section
cross-references (`@sec-`) throughout.

## Notes for honing

- Use these as the fixtures while improving the **renderer** (CSS + engine): a fix to
  single-page layout is judged against `paper.tmd`, a fix to book chrome against
  `docs/guide/`, and so on.
- Drive click-to-source + live reload from these standalone previews (they are fully
  live), not from a mounted view.
