# Client-side accessibility audit of the rendered output

## Problem

Accessibility regressions (missing alt text, heading-level skips, nameless links,
low contrast, a missing `lang`) are recurring and invisible: nothing in the normal
author loop surfaces them. The preview already has a diagnostics panel with
click-to-source; a11y findings should ride the same channel.

## Design

A `scanA11y()` pass runs over the mounted DOM after every render (in `afterChange`,
next to `scanCellErrors`). Findings populate a third panel sub-list (`#qmd-a11y`,
parallel to server diagnostics and per-cell errors) and feed the dev-button badge.
Located findings (tied to a block) are buttons that jump to the offending source line
via the existing `gotoSource(file, line)`; page-level findings are plain rows.

Entirely client-side and advisory — no server work, never blocks rendering. Kept to
high-confidence, low-noise checks so the panel stays trustworthy:

1. **Missing `alt`** — `img:not([alt])` (decorative images should use `alt=""`).
   Capped at 8 rows + an "…and N more" summary.
2. **Heading-level skip** — a heading that goes deeper by more than one level
   (`h2 → h4`); going back up is fine.
3. **Nameless link/button** — `a[href]`/`button` with no text, `aria-label`, `title`,
   or alt-bearing image/SVG title. Capped at 5.
4. **Missing `lang`** — empty/absent `<html lang>` (page-level). qmd-fast always emits
   `lang="en"`, so this only fires on a regression — no false positives.
5. **Low contrast** — WCAG relative-luminance ratio of body text vs the first opaque
   ancestor background; warns below AA (4.5:1). Page-level, wrapped in try/catch.

Source mapping reuses the block model: `a11yLoc(el)` finds the nearest
`[data-sourcepos]`/`[data-block-id]` ancestor, parses `line` from `data-sourcepos`,
and `file` from the nearest `[data-source-file]` (so an issue inside an `{{< include >}}`
points at the included file).

## Corpus pin / verification

`corpus/diagnostics/a11y.qmd` carries a raw `<img>` with no alt, an `h2→h4` skip, and
an empty `[](#)` link. Browser-verified (chrome-devtools): the panel shows exactly
those 3 located findings, the badge reads 3, each row jumps to its source line (the
img row → sourcepos `11:1`), and the contrast + lang checks stay silent (no false
positives) on the default theme. The checks are client-side, so they aren't covered
by `cargo test`; `corpus/diagnostics/` is already exempt from the front-matter lint
tests, so the deliberately-broken doc doesn't trip them.
