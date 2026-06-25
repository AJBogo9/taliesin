# Reader line-spacing control — design

> Status: building (2026-06-25, branch `feat/reader-line-spacing`, ultracode). A reading-comfort
> / accessibility control (WCAG 1.4.12 Text Spacing): adjustable prose line-height. Continues the
> a11y thread (after the focus trap) and the reader-experience cluster; mirrors the existing
> text-size / width controls exactly.

## Goal

A **Line spacing** segmented control in the reader menu's Display section — **Tight (1.5)** /
**Normal (1.7, default)** / **Relaxed (2.0)** — adjusting the line-height of *prose* only. Applied
before paint (no flash) by the existing reader-pref pipeline, persisted in the reader's own
`localStorage`, cleared by Reset.

## Mechanism (mirror size / width)

The pre-paint head script (`render/theme.rs`) and the Display UI (`qmdInitReaderPrefs` in
`code-enhance.js`) already drive `qmd-reader-scale` → `--qmd-reader-scale` and `qmd-reader-width`
→ `--qmd-maxw`. Add a third, `qmd-reader-leading` → `--qmd-reader-leading`:

- **`theme.rs applyReader()`**: read `qmd-reader-leading`; set/remove `--qmd-reader-leading` on
  `document.documentElement` (alongside scale/width). **`qmdResetReader`** also removes it.
- **`base.css`**: prose blocks only —
  `body p, body li, body dd, body blockquote, body figcaption { line-height: var(--qmd-reader-leading, 1.7); }`.
  Scoped this way, headings (own optical line-heights), `pre`/`code` (monospace), `.katex`, and
  tables are untouched by construction, so code/math integrity is preserved. The default `1.7`
  equals the current body line-height, so nothing changes until the reader opts in. **`pre` is
  given an explicit `line-height: 1.5`** so block code stays fixed even nested in a list item.
  An adversarial review caught that `body p`/`body li` also leaks into *chrome that wraps prose*
  — the TOC / book sidebar / postnav / navbar (`<nav><ul><li>`), the search results
  (`role="listbox"`), and margin notes (`.column-margin`/`.sidenote`/`.aside` set 1.5 on their
  container). A follow-up rule **re-pins those to `inherit`** (`nav li, [role="listbox"] li,
  .sidenote p, .marginnote p, .column-margin p, .aside p`), restoring each container's own
  leading. Using the semantic `nav` element (not per-class) covers all navigation chrome.
- **`qmdInitReaderPrefs`**: a `seg('Line spacing', LEADINGS, curLeading, …)` row after Width,
  `LEADINGS = [['1.5','Tight'], ['1.7','Normal'], ['2','Relaxed']]`,
  `curLeading = qmdGetReaderPref('leading') || '1.7'`, picking Normal stores `null` (clears the
  pref, falling back to 1.7) — exactly the width control's pattern. Added to `syncAll`.

## Invariants

Reader-side + read-only; pre-paint (no flash); offline; additive (one CSS var + one menu row + 3
lines in the pre-paint script); no block-model / output / Rust-pipeline change; idempotent;
decks unaffected (no reader menu on decks). Does not touch code, math, headings, or tables.

## Verification

- **Rust test** (`render/tests.rs`): the page ships `--qmd-reader-leading`.
- **Browser (chrome-devtools MCP):** open the reader menu → a Line spacing row with Tight/Normal/
  Relaxed; pick Relaxed → a prose `<p>` computed `line-height` increases while a `pre`/`code` and
  a heading line-height are UNCHANGED; the choice persists across reload (pre-paint, no flash) and
  syncs the pressed state; Reset clears it back to 1.7; a deck shows no control. Reuses the
  `corpus/reader/preferences.qmd` pin (the prefs/display demo).
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt` + `tsc`; a focused adversarial review.

## Files

`crates/core/src/render/theme.rs` (applyReader + reset), `crates/core/assets/css/base.css` (the
prose line-height var), `crates/core/assets/js/code-enhance.js` (the Display row), a test in
`crates/core/src/render/tests.rs`. No new corpus pin.
