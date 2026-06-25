# Reading progress & resume — design

> Status: building (2026-06-25, branch `feat/reading-progress`). The second
> reader-experience feature from `FEATURE-IDEAS.md` (#1 resume + #2 progress), building on
> the reader-prefs substrate shipped in `2026-06-25-reader-preferences-design.md`.

## Problem

A reader of a long qmd-fast post or book chapter has no orientation and no memory: the page
gives no sense of how far they are or how much is left (the "bottomless chapter" feeling),
and closing the tab loses their place (the browser dumps them at the top next time). Word
count + reading time already exist, but only in the dev preview's control bar, never in the
built page the reader actually sees.

## Goal

Two reader-side, offline, no-write-back affordances on any reading page:

1. **Reading progress** — a thin top progress bar tied to scroll, plus a small "**N min
   left**" estimate (from prose word count, code/math excluded). Universal orientation cue.
2. **Resume where you left off** — the reader's scroll position is remembered, anchored to a
   block's content-hash `data-block-id` (so it is *exact* and survives reflow / re-render,
   unlike a raw scroll offset). On return, a dismissable "**Resume reading · N% →**" pill
   scrolls them back.

Out of scope (follow-ups): bookmarks, per-chapter (vs per-page) progress in a book,
read/unread TOC checkmarks.

## Invariants honored

Reader-side only: progress is derived from the live DOM; the resume position lives in the
reader's own `localStorage` keyed by `location.pathname`. Nothing writes the author's `.qmd`
(single editing surface), no output format (HTML-only), offline (an additive `qmdEnhancers`
enhancer + CSS), no Do-NOT-touch machinery. Block ids are read, never mutated. Decks skip it
(own chrome).

## Mechanism (one enhancer, `qmdInitReadingProgress` in `code-enhance.js`)

- **Idempotent + deck-skip**, like the other document-level enhancers (`window.__qmdProgress`
  guard; early return on `.qmd-deck`).
- **Word count once on init:** sum the text of the content blocks (`[data-block-id]`
  elements, scoped to top-level so nested blocks aren't double-counted), excluding `pre`,
  `code`, `.katex` descendants (mirrors the existing client.js prose count). `totalMin =
  max(1, round(words / 200))`. Recomputed cheaply only if the block set changes (preview
  re-mount) — never per scroll/op (avoids the known per-op deep-clone cost).
- **Progress bar:** a fixed top `.qmd-readbar` with a `.qmd-readbar-fill`; on scroll
  (rAF-throttled) `frac = scrollTop / (scrollHeight - innerHeight)`, set fill width = `frac`,
  and a small `.qmd-readbar-time` shows `ceil(totalMin * (1 - frac))` min left (hidden at
  the very end).
- **Resume:** on scroll (debounced ~500ms) save `frac|topBlockId` to
  `localStorage["qmd-pos:" + location.pathname]`, where `topBlockId` is the first
  `[data-block-id]` whose `getBoundingClientRect().top >= -4`. On init, if a saved entry has
  `frac > 0.04` and its block still exists and isn't already in view, show a fixed
  `.qmd-resume` pill ("Resume reading · N% →"); clicking it `scrollIntoView`s the saved block;
  the pill auto-dismisses on the next user scroll or after ~8s, and a × closes it.

## Verification

- **Corpus pin:** `corpus/reader/long-read.qmd` (several sections of prose, long enough to
  scroll), covered by the corpus block-invariant test for free.
- **Rust test** (`render/tests.rs`): the assembled page ships the enhancer
  (`qmdInitReadingProgress`). Guards the wiring.
- **Browser (chrome-devtools MCP):** the bar fills as you scroll; "min left" decreases;
  after scrolling + reload the resume pill appears and returns you to the saved block;
  a deck shows no bar/pill; console clean.
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt` + `tsc`.

## Files

`crates/core/assets/js/code-enhance.js` (the enhancer + registration),
`crates/core/assets/css/base.css` (bar + pills styling), `corpus/reader/long-read.qmd`
(pin), a test in `crates/core/src/render/tests.rs`.
