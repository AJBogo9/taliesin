# Design: `typography-craft-pass` (Wave 3 feature, = backlog #6)

Status: approved 2026-06-24. Branch `feat/typography-craft-pass`. Roadmap:
`BEYOND-QUARTO.md` Pillar V. CSS-only craft pass, zero web fonts (offline/FOUC-free
stays a past-Quarto default). No HTML/block-model change.

## Grounding finding

`base.css` body is a flat `17px/1.7` serif; **headings (h1-h4) set only the family +
`line-height: 1.25`, with NO explicit sizes** → they fall back to browser defaults. The
biggest lever is an intentional type scale.

## Decisions (approved defaults)

- Scale ratio `--qmd-scale: 1.2` (minor third, editorial).
- Body line-height stays `1.7`.
- Font smoothing ON (judged in-browser; easy to drop if it thins the serif body).

## Changes (all `base.css`, a couple `dark.css` touches if needed)

1. **Modular heading scale.** Add `--qmd-scale: 1.2`. Explicit, intentional sizes:
   h1 `2.0rem`, h2 `1.62rem`, h3 `1.35rem`, h4 `1.15rem`, h5 `1rem` (uppercase, tracked,
   muted), h6 `.9rem` (muted). `line-height` tightens with size (≈1.15 → 1.3).
   `letter-spacing: -0.011em` on h1/h2. Vertical rhythm: deliberate heading margins
   (more space-before than -after, binding a heading to its section).
2. **Font features.** `body { font-feature-settings: "liga" 1, "calt" 1, "kern" 1; }`.
   `font-variant-numeric: tabular-nums` scoped to `pre, code, table, .katex` (aligned
   figures in code/columns; prose keeps proportional numerals).
3. **KaTeX alignment.** `.katex { font-size: 1.05em; }` so inline math matches the 17px
   serif x-height (KaTeX defaults to 1.21em, reads oversized).
4. **Font smoothing.** `-webkit-font-smoothing: antialiased; -moz-osx-font-smoothing: grayscale;`
   on `body` (most visible in dark mode).
5. **Measure** unchanged (`--qmd-maxw: 46rem` ≈ 72-75 chars, already ideal).

## Verification

No unit tests (pure CSS, no behavior/HTML change — TDD's stated config/aesthetic
exception). Verify by before/after screenshots of `corpus/posts/em-algorithm/index.qmd`
(headings + math + code + table), light + dark, via chrome-devtools. Existing suite stays
green (no HTML change).

## Invariants

CSS-only; block model, sourcepos, diff untouched; offline (no web fonts); `deck.css`
untouched (deck has its own type system). Callout color/spacing already handled by
`callout-kind-contract`, not restyled here.

## Out of scope (YAGNI)

Web-font loading; a full spacing-token system; deck typography; per-doc scale overrides.
