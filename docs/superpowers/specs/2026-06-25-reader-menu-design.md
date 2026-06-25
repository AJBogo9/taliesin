# Reader menu (consolidation) — design

> Status: building (2026-06-25, branch `feat/reader-menu`). A consolidation, not a new
> capability: the four reader features shipped this session each grew their own floating
> control, and the corners got crowded. This folds them into one menu.

## Problem

The reader cluster now has: an **Aa** prefs button + panel (bottom-right), a **N min left**
pill (bottom-right), a **N highlights** button + panel (bottom-left), plus the top progress
bar and the transient resume pill. Three persistent floating controls is clutter.

## Goal

One **Reader** launcher (the familiar **Aa** button, bottom-right) opens a single menu with
three sections:

- **Reading** — a small "~N min left · X% read" readout (folds in the min-left pill).
- **Display** — theme (light/dark/sepia), text size, width, reset (the old prefs panel).
- **Highlights** — the list + jump/remove + Markdown export (the old index panel), shown
  only when the page has highlights.

The **top progress bar** stays (it is ambient, a 3px line, not a control). The **resume
pill** stays (it is a contextual prompt that comes to the reader, not a menu item). The
**highlight selection action** stays (transient, appears on selection).

## Architecture

A small **menu host** so each existing enhancer keeps its logic and only changes where its
UI mounts (minimal disruption; the enhancer function names are unchanged, so the existing
"ships qmdInit…" tests still hold):

- **`qmdInitReaderMenu`** (registered first) builds the **Aa** launcher + the menu panel and
  exposes `window.qmdReaderMenu.addSection(title, node, onOpen) -> { setVisible }`. Opening
  the menu calls every section's `onOpen` (so each refreshes its live state). Open/close on
  click, click-away, and Esc. Skipped on decks.
- **`qmdInitReaderPrefs`** builds its segmented controls into a node and
  `addSection('Display', node, syncAll)`; it no longer builds a standalone button/panel.
- **`qmdInitHighlightIndex`** builds the list + export into a node and
  `addSection('Highlights', node, render)`, hiding the section (via `setVisible`) when there
  are no highlights; no standalone button/panel.
- **`qmdInitReadingProgress`** keeps the top bar + resume, drops the floating min-left pill,
  and `addSection('Reading', node, update)` with the "~N min left · X% read" readout.

All sections degrade gracefully if the host is absent (each guards on
`window.qmdReaderMenu`). Order in the menu = registration order: Reading, Display,
Highlights.

## Invariants

Unchanged: reader-side, read-only, offline, additive enhancers, no block-model change. This
change only moves existing UI under one launcher; the apply/storage logic (theme.rs
pre-paint, the highlight storage, the scroll math) is untouched.

## Verification

- **Rust test** (`render/tests.rs`): the page ships `qmdInitReaderMenu` (new). The existing
  `qmdInitReaderPrefs` / `qmdInitReadingProgress` / `qmdInitHighlights` /
  `qmdInitHighlightIndex` tests still pass (names unchanged).
- **Browser (chrome-devtools MCP):** one Aa launcher (no separate min-left pill or highlights
  button); opening it shows Reading + Display sections, and Highlights appears after a
  highlight is made; theme/size/width still apply + persist; the highlights list/jump/remove/
  export still work from inside the menu; the top bar + resume still work; deck shows no menu;
  console clean.
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt` + `tsc`.

## Files

`crates/core/assets/js/code-enhance.js` (host + the three conversions),
`crates/core/assets/css/base.css` (unified `.qmd-rmenu-*`; drop the standalone toggle/pill
styles), `crates/core/src/render/tests.rs` (the new test). No new corpus pin (a UI
refactor of already-pinned features; verified against the existing reader corpus docs).
