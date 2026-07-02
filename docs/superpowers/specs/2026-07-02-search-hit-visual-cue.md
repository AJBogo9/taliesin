# Search-hit visual cue (Cmd-K → flash the matched term)

**Status:** implemented 2026-07-02. Design settled 2026-06-30 (backlog "Reader experience").

## Problem

Clicking a Cmd-K result lands the reader on the target *heading*, but the actual
matched term may be several lines into the section. The reader has to re-find what they
searched for. A brief visual cue on the term closes that loop.

## Design (settled)

On a result click / Enter:

1. Navigate to the heading exactly as today (in-page `scrollIntoView`, or a real page
   load anchored to `#id` for a cross-page result).
2. **Locate** the first substring occurrence of any query term within the target
   heading's *section* (the heading element + following siblings up to the next
   heading), reusing search.js's existing `indexOf`-based matching (the same
   length-preserving lowercase-offset safety as `termRanges`).
3. **Flash** that occurrence via the **CSS Custom Highlight API** — zero DOM mutation, so
   it honours the read-only-preview invariant (no write-back to source, no block-id
   churn). Registered as `qmd-search-flash`; a `<mark class="qmd-ra-mark">` fallback (via
   `Range.surroundContents`) covers browsers without the API, mirroring read-aloud
   (`05-read-aloud.js`).
4. **Auto-scroll to the occurrence only if it is off-screen** (the heading is already in
   view; a short section keeps the term visible, so don't yank the viewport).
5. The flash **fades out** (~1.5s): a theme-aware `@property`-interpolated
   `--qmd-search-flash` colour animated to transparent while the highlight is set, then
   the highlight is cleared.

**Cross-page handoff via `sessionStorage`:** before `location.href`, the query terms are
written to `qmd-search-flash`; on the next page's load they are read + cleared, and the
same locate-and-flash runs against the URL `#anchor` target — so in-page and cross-page
share ONE code path.

**Deck-skip.** Fuzzy-/title-only matches (no substring occurrence in the section) just
land on the heading with **no cue** (never flash the wrong run — `termRanges`' honesty).

## Rejected / simplified

- Native `#:~:text=` text fragments as the primary mechanism: not theme-styleable, no
  fade control, patchy in Firefox. Kept the Custom Highlight API.
- A smooth per-element opacity transition on `::highlight()`: the pseudo can't be CSS-
  transitioned per instance, so the fade is done by animating the shared
  `@property --qmd-search-flash` colour to transparent (registered so it interpolates).

## Touched

- `crates/core/assets/css/base.css`: `@property --qmd-search-flash` + per-theme
  `::highlight(qmd-search-flash)` colour + the fade keyframes.
- `web-client/search.js` (`SEARCH_JS`, ships to preview + built pages): the shared
  `flashTermsIn` locate-and-flash, `go()`'s cross-page `sessionStorage` write, and the
  on-load read that flashes the anchored term.

## Verification

Browser (chrome-devtools): in-page result → heading scroll + term flash; a term below the
fold auto-scrolls; a short section does not; cross-page result flashes the term on the
destination page; fuzzy-only match lands with no flash; 0 console errors; deck pages skip.
