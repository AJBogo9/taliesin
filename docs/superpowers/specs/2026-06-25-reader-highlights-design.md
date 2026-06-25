# Reader highlights — design

> Status: building (2026-06-25, branch `feat/reader-highlights`). The flagship reader
> feature from `FEATURE-IDEAS.md` (#4) and the foundation of the "My Copy" moonshot, on the
> reader-prefs / reading-position substrate.

## Problem

A reader of a dense technical doc wants to mark the one line that matters and find it again.
Web docs are read-only glass; the reader's own emphasis has nowhere to live. Hypothesis and
Readwise approximate this with brittle fuzzy-text anchoring that breaks on re-render. qmd-fast
can do it *exactly*, because every block already carries a stable content-hash `data-block-id`.

## Goal

Select prose in the built output, click **Highlight**, and it persists: the passage is
wrapped in a `<mark>`, stored in the reader's own `localStorage` anchored to the block's
`data-block-id` plus character offsets within that block, and re-applied on every load.
Clicking a highlight offers **Remove**. One color, theme-aware (light/dark/sepia).

Because the anchor is the content hash, a highlight is exact and survives a re-render or a
layout change; only an edit to *that block's text* can orphan it (gracefully: the offsets no
longer resolve, so it is dropped, not mis-placed). This is the moat cashed for the reader.

Out of scope (follow-ups): multiple colors, margin notes on a highlight, cross-block
selections, export/import, a "my highlights" index. v1 is single-block prose highlights.

## Invariants honored

Reader-side + read-only: highlights live in the reader's own `localStorage` (keyed by
`location.pathname`); the `<mark>` is a DOM decoration applied *after* mount. It never writes
the author's `.qmd`, adds no output format, ships offline (an additive `qmdEnhancers`
enhancer + CSS), and changes no block id / sourcepos (those are on the block element, not the
wrapped text), so click-to-source and the diff are untouched. In the live preview a block
swap drops the mark; the enhancer re-applies on the next mount (same pattern as every other
enhancer). Decks are skipped.

## Mechanism (one enhancer, `qmdInitHighlights` in `code-enhance.js`)

- **Setup once** (guard `window.__qmdHL`): the floating action button + the
  selection/click listeners. **Re-apply on every enhance pass** (not guarded), so a freshly
  mounted/built DOM gets its highlights.
- **Offset model:** a highlight is `{id, s, e}` where `id` is the block's `data-block-id` and
  `s`/`e` are character offsets into the block's **highlightable text** — the concatenation
  of its text nodes, *skipping `.katex` and `pre`/`code` subtrees* (KaTeX duplicates text via
  MathML, which would corrupt offsets; code is excluded for v1). The create path and the
  apply path use the *same* filtered text walk, so offsets always line up.
- **Create:** on a non-collapsed selection whose start and end are in the **same** block and
  not inside `.katex`/`pre`/`code`, show a **Highlight** button near the selection. Click →
  compute `{id, s, e}`, push to storage, wrap, clear the selection.
- **Wrap (`applyOne`):** walk the filtered text nodes; for each node overlapping `[s, e)`,
  `splitText` to isolate the overlapping substring and wrap it in
  `<mark class="qmd-userhl" data-hl="id:s:e">`. A multi-text-node range (a phrase crossing a
  `<strong>`/link) yields several contiguous marks that read as one highlight. Each original
  text node is processed independently, so splitting one never invalidates another.
- **Re-apply (`applyAll`):** unwrap every existing `.qmd-userhl` (replace with its text +
  `normalize()`), then `applyOne` each stored highlight. Idempotent; safe to run on every
  mount.
- **Remove:** clicking a `.qmd-userhl` shows a **Remove** button; it drops that `id:s:e` from
  storage and re-applies.

## Verification

- **Corpus pin:** `corpus/reader/highlights.qmd` (prose with inline emphasis + a link, so a
  highlight can span multiple text nodes), covered by the corpus block-invariant test.
- **Rust test** (`render/tests.rs`): the assembled page ships `qmdInitHighlights`.
- **Browser (chrome-devtools MCP):** programmatically select a phrase (incl. one spanning a
  `<strong>`), trigger Highlight → a `<mark class="qmd-userhl">` wraps exactly that text and
  `localStorage` has the entry; reload → the mark re-applies at the same text (block-id
  anchored); Remove → mark gone + storage cleared; a deck shows no highlight UI; console
  clean; click-to-source (`data-sourcepos`) on the block is unchanged after highlighting.
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt` + `tsc`.

## Files

`crates/core/assets/js/code-enhance.js` (the enhancer), `crates/core/assets/css/base.css`
(`.qmd-userhl` + the action button + `--qmd-userhl-bg`), `corpus/reader/highlights.qmd`
(pin), a test in `crates/core/src/render/tests.rs`.
