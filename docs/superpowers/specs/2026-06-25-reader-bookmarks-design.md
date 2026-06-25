# Reader bookmarks (section markers) — design

> Status: building (2026-06-25, branch `feat/reader-bookmarks`). The save-and-return
> complement to highlights: highlights mark a *passage*; bookmarks mark a *section* to come
> back to. Slots into the reader menu (`2026-06-25-reader-menu-design.md`) as a new section,
> exercising the `qmdReaderMenu.addSection` host shipped in that change.

## Problem

A reader of a long document (a book chapter, the Internals book) wants to flag "the part
about X" and jump back to it later. Resume restores one position (where you stopped);
highlights mark fine-grained passages. Neither is the lightweight "mark these few sections"
gesture that a physical bookmark gives. Headings are the natural unit: you return to a
section, not a paragraph.

## Goal

Hovering a heading reveals a small **☆** toggle in its left margin; clicking bookmarks that
section (the toggle fills to **★** and the heading keeps a persistent margin star). A
**Bookmarks** section in the reader menu lists every bookmarked heading (its text), jumps to
one on click (scroll + flash), and removes one (×). Bookmarks persist in the reader's own
`localStorage`, anchored to the heading's `data-block-id`, so they are exact and survive a
re-render; an edit that removes the heading orphans the bookmark gracefully (it stops
resolving and is skipped). Shown only when the page has bookmarks.

## Invariants honored

Reader-side + read-only, exactly like highlights: bookmarks live in
`localStorage["qmd-bm:" + pathname]` as an array of block ids; the toggle + margin star are
DOM decorations applied after mount; never writes the author's `.qmd`; no output format;
offline; an additive `qmdEnhancers` enhancer + CSS; no block-model change (the id/sourcepos
are on the heading element, untouched). Decks skipped.

## Mechanism (one enhancer, `qmdInitBookmarks` in `code-enhance.js`)

- **Storage:** `qmd-bm:pathname` = `[blockId, …]`. `dispatch()` fires `qmd:bmchange`.
- **Re-apply every pass** (`applyMarkers`, not guarded): for each heading (`h1`–`h6` with a
  `data-block-id`), toggle the `.qmd-bookmarked` class from storage, so a freshly
  mounted/built DOM shows its stars. Idempotent.
- **Setup once** (guard `window.__qmdBookmarks`):
  - One floating **`.qmd-bm-toggle`** button. Heading hover is handled by **delegation**
    (one `mouseover` listener; `closest('h1,…,h6')` with a `data-block-id`), so it survives
    live-preview block swaps. On hover, position the toggle at the heading's left margin and
    set ★/☆ + `aria-pressed` from storage. Moving onto the toggle keeps it; moving elsewhere
    hides it (a short grace timeout). Clicking toggles that heading's id in storage,
    updates the marker, and dispatches `qmd:bmchange`.
  - A **Bookmarks** menu section via `qmdReaderMenu.addSection('Bookmarks', node, render)`,
    hidden (via `setVisible`) when there are no resolvable bookmarks. `render` lists each
    bookmarked heading's text with **jump** (`scrollIntoView` + `.qmd-flash`, closes the
    menu) and **remove** (×). Refreshed on `qmd:bmchange` and on menu open.

If the menu host is absent the margin stars + hover toggle still work; only the menu list is
skipped. The heading text for the list reuses the heading's `textContent` (headings have no
math/code carve-out concern that highlights need).

## Verification

- **Corpus pin:** `corpus/reader/bookmarks.qmd` (several headed sections), covered by the
  corpus block-invariant test.
- **Rust test** (`render/tests.rs`): the assembled page ships `qmdInitBookmarks`.
- **Browser (chrome-devtools MCP):** hover a heading → the ☆ toggle appears; click → the
  heading gains a persistent ★ and `localStorage` has its block id; open the menu → a
  **Bookmarks** section lists the section, jump scrolls to it, remove clears the star +
  storage + list; reload → the star re-applies (block-id anchored); a deck shows no bookmark
  UI; `data-sourcepos` on the heading is unchanged; console clean.
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt` + `tsc`.

## Files

`crates/core/assets/js/code-enhance.js` (the `qmdInitBookmarks` enhancer + its registration),
`crates/core/assets/css/base.css` (`.qmd-bm-toggle` + `.qmd-bookmarked` margin star),
`corpus/reader/bookmarks.qmd` (pin), a test in `crates/core/src/render/tests.rs`.
