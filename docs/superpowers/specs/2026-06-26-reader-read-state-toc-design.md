# Reader read-state TOC — design

> Status: shipped (2026-06-26, `feat/reader-read-state-toc` → main). Mark the
> sections a reader has already scrolled through, right in the TOC, so a long
> document shows your trail at a glance. Browser-verified (chrome-devtools):
> scroll-through marks + mute + ✓, bottom marks the last section, reload restores
> from localStorage, computed `::after "✓"` in accent + `opacity .62`.

## Problem

On a long page the TOC tells you the document's shape and (via scrollspy) where you
*are*, but not where you've *been*. A reader returning to a half-finished article, or
scanning back over a long reference, has no cue for which sections they've already
covered. The reading-progress bar gives one global percentage; it can't say "you read
§1–§4, skipped §5, you're in §6".

## Goal

Decorate each TOC entry whose section the reader has scrolled through with a small ✓
and a subtle de-emphasis, distinct from the active (current) entry. The marker is:

- **Reader-side, read-only.** State lives in the reader's own `localStorage`, keyed by
  `location.pathname`; it never touches the `.qmd` source (single-editing-surface
  invariant).
- **Passive/ambient.** No menu control, no click target — it just reflects scrolling.
  (Like the scrollspy highlight, not like bookmarks/highlights which have UI.)
- **Monotonic.** Once a section is read it stays read (scrolling back up does not
  un-mark it); a returning reader sees their prior trail restored on load.

"Scrolled through" = the reader's position has advanced past the section: every entry
*before* the current scrollspy section is read, plus the final entry once the page
bottom is reached. Jumping forward (a TOC click / resume) counts the skipped-over
sections as read — read-state is position-anchored, matching resume/progress.

Out of scope: a reset/clear UI (re-reading re-marks; state is per-path and ambient),
per-section percentages, and anything that writes back to source.

## Invariants honored

- Reader-side, read-only, offline; no block-model change; additive.
- **Block-id-anchored:** the read set stores each read heading's `data-block-id` (the
  stable content hash, same anchor bookmarks/highlights/resume use), so it survives
  reflow and an entry is matched back to its TOC link via the heading `id`→element.
- **Decks excluded** implicitly: decks emit no `#TOC`, so `toc-spy.js` is inert there.
- **No flash needed:** the decoration is a subtle marker applied by the same script
  that already runs the scrollspy (not layout/colour that would flash), so it rides the
  existing `toc-spy.js` init — no pre-paint head script.

## Mechanism

All of it lives in the existing scrollspy, `web-client/toc-spy.js` (bundled as
`TOC_SPY_JS`, inlined whenever a page has a TOC), which already holds
`entries = [{link, heading}]`, the rAF scroll loop, the activation line, and
bottom-pinning. We extend it rather than add a second scroll observer (no drift between
"active" and "read").

1. **Load** on `init()`: read `localStorage['qmd-read:' + location.pathname]` (a JSON
   array of heading `data-block-id`s) into a `Set`. For each `entry` whose heading's
   `data-block-id` is in the set, apply the read decoration immediately (returning
   reader's trail restored before they scroll).
2. **Advance** inside `update()`, right after `cur`/`atBottom` are computed: the
   high-water index of scrolled-through sections is `indexOf(cur)` (the current section
   is excluded — you're still in it), or `entries.length` when `atBottom`. For each
   newly-passed entry, add its heading `data-block-id` to the set and decorate its link.
   Persist (`JSON.stringify([...set])`, try/catch) only when the set actually grows.
3. **Decorate** (`markRead(link)`, idempotent): add class `qmd-toc-read`; append a
   visually-hidden `<span class="qmd-sr-only"> (read)</span>` once so a screen reader
   announces it (the ✓ itself is CSS `::after`, decorative).

CSS (`crates/core/assets/css/base.css`, beside the `#TOC a` rules):

```css
#TOC a.qmd-toc-read { opacity: .62; }                 /* recede; default is already muted */
#TOC a.qmd-toc-read::after { content: "✓"; margin-left: .45ch; opacity: .85;
  color: var(--qmd-accent); font-size: .82em; }
#TOC a.qmd-toc-read.qmd-toc-active { opacity: 1; }    /* the current section reads at full strength */
```

The active entry keeps `color: var(--qmd-fg)` + the accent left-border (unchanged). A
read entry that is also the active one (you scrolled back to a finished section) shows
the active treatment at full opacity, with the ✓ still present.

## Verification

- **Corpus pin:** reuse `corpus/reader/long-read.qmd` (a long, many-heading doc with
  `toc: true`) — the natural fixture; no new corpus doc.
- **Rust test** (`render/tests.rs`): `assembled_page_ships_read_state_toc()` — render a
  page with `toc: true` and assert it ships the read-state code (`qmd-toc-read` class +
  the `qmd-read:` storage key from `toc-spy.js`, plus the `#TOC a.qmd-toc-read` CSS).
  A page with no TOC ships neither (toc-spy.js is omitted) — guards against always-on.
- **Browser (chrome-devtools MCP):** open `long-read.qmd` with a TOC; scroll through a
  few sections; assert their TOC entries gain `.qmd-toc-read` + the ✓ while the current
  one stays accent; reload and confirm the read entries are restored from localStorage;
  confirm decks (no TOC) are unaffected.
- **Gates:** `cargo test -p qmd-fast-core`, `cargo clippy -D warnings`, `cargo fmt`,
  `cd web-client && tsc` (toc-spy.js is not in the tsc bundle, but client.js still is).

## Files

- `web-client/toc-spy.js` — load + advance + decorate read-state (the whole behaviour).
- `crates/core/assets/css/base.css` — the `.qmd-toc-read` rules.
- `crates/core/src/render/tests.rs` — `assembled_page_ships_read_state_toc()`.
- `corpus/reader/long-read.qmd` — ensure `toc: true` + enough headings (pin only).
