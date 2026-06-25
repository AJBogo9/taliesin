# My highlights — index & Markdown export — design

> Status: building (2026-06-25, branch `feat/highlight-index`). Completes the reader
> highlights feature (`2026-06-25-reader-highlights-design.md`) and delivers the portable
> half of the "My Copy" idea from `FEATURE-IDEAS.md`.

## Problem

Reader highlights can be created and removed, but not *reviewed* or *kept*. A reader who
marks ten passages in a chapter has no way to see them together, jump between them, or take
them out into their own notes. Highlighting without review or export is half a feature.

## Goal

When the current page has any highlights, a small **"N highlights"** control appears; it
opens a panel that:

- **lists** every highlight on the page (its text, truncated), newest-anchored;
- **jumps** to a highlight (scroll + a brief flash) on click;
- **removes** a highlight (×), updating the marks live;
- **exports** all highlights as Markdown into a selectable read-only `<textarea>` (and best-
  effort to the clipboard), so the reader can paste them into Obsidian / Zotero / notes.

This is the Readwise "take my notes with me" value, with **zero backend** — the highlights
are the reader's own `localStorage`, exported as a file/paste, never a server.

## Invariants honored

Reader-side + read-only, exactly like the highlights it indexes: reads the same
`localStorage["qmd-hl:" + pathname]`; never writes the author's `.qmd`; no output format;
offline; an additive `qmdEnhancers` enhancer + CSS; no block-model change. Decks skipped.

## Mechanism

A new enhancer **`qmdInitHighlightIndex`** (separate from `qmdInitHighlights`, coordinating
through one event so each stays modular):

- The highlights enhancer (`qmdInitHighlights`) **dispatches `qmd:hlchange`** whenever it
  adds or removes a highlight, and **re-applies on `qmd:hlchange`** (so a removal from the
  index updates the marks). This is the only change to the shipped enhancer.
- `qmdInitHighlightIndex` renders a fixed **"N highlights"** button (bottom-left), shown only
  when the page has ≥1 highlight; refreshed on `qmd:hlchange`. Clicking it toggles a panel.
- The panel reads `qmd-hl:pathname`, and for each entry resolves the highlight's text by
  re-walking the block's highlightable text (the same skip-`.katex`/`pre`/`code` rule) and
  slicing `[s, e)`; shows a truncated snippet. **Jump** = `scrollIntoView` the block (+ a
  `.qmd-flash` pulse). **Remove** = drop that `id:s:e` from storage and dispatch
  `qmd:hlchange` (the marks re-apply; the list refreshes).
- **Export** builds Markdown: a `# <Page Title>` heading, the page URL, then each highlight
  as a `> blockquote`. It fills a read-only `<textarea>` (selectable) and also attempts
  `navigator.clipboard.writeText` (with an `execCommand` fallback) — the textarea is the
  reliable, offline path.

## Verification

- **Corpus pin:** `corpus/reader/notes.qmd` (short readable prose), covered by the corpus
  block-invariant test.
- **Rust test** (`render/tests.rs`): the assembled page ships `qmdInitHighlightIndex`.
- **Browser (chrome-devtools MCP):** create two highlights → the "2 highlights" button
  appears; open the panel → both are listed with their text; click an entry → scrolls to it;
  Remove one → marks + list + count update to 1; Export → the textarea holds correct Markdown
  (page title + the highlight as a blockquote); a deck shows no index; console clean.
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt` + `tsc`.

## Files

`crates/core/assets/js/code-enhance.js` (the `qmd:hlchange` wiring on `qmdInitHighlights` +
the new `qmdInitHighlightIndex` enhancer), `crates/core/assets/css/base.css` (the index
button + panel), `corpus/reader/notes.qmd` (pin), a test in `crates/core/src/render/tests.rs`.
