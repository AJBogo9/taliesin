# Reader letter/word-spacing controls — design

> Status: building (2026-06-25, branch `feat/reader-letter-word-spacing`). Completes WCAG 1.4.12
> Text Spacing alongside the shipped line-spacing control: reader-adjustable letter (tracking) and
> word spacing on prose. Continues the reader-experience cluster; mirrors the line-spacing /
> text-size / width controls exactly (pre-paint var + Display-menu segmented row).

## Goal

Two **segmented controls** in the reader menu's Display section, after Line spacing:

- **Letter spacing** — Normal (0) / Wide (0.06em) / **Wider (0.12em)**
- **Word spacing** — Normal (0) / Wide (0.08em) / **Wider (0.16em)**

The **Wider** steps hit the WCAG 1.4.12 minimums exactly (letter ≥ 0.12×font-size, word ≥
0.16×font-size), so the reader can reach WCAG text spacing on demand; `em` keeps each proportional
to the (reader-scaled) font size. Applied before paint (no flash) by the existing reader-pref
pipeline, persisted in the reader's own `localStorage`, cleared by Reset.

## Mechanism (mirror line-spacing)

Add two more reader prefs alongside `scale` / `width` / `leading`: `qmd-reader-letter` →
`--qmd-reader-letter`, `qmd-reader-word` → `--qmd-reader-word`.

- **`theme.rs applyReader()`**: read both keys; set/remove the two vars on
  `document.documentElement` (alongside the existing three). **`qmdResetReader`** also removes them.
- **`base.css`**: prose blocks only — add to the existing prose rule
  `body p, body li, body dd, body blockquote, body figcaption`:
  `letter-spacing: var(--qmd-reader-letter, normal); word-spacing: var(--qmd-reader-word, normal);`.
  Default `normal` = no change until opt-in. Two re-pins guard the leaks letter-spacing can cause
  that line-height could not, because tracking *inherits down into inline descendants*:
  - **Monospace + math integrity:** `code, pre, kbd, samp, .katex { letter-spacing: normal;
    word-spacing: normal; }` — an inline `<code>`/`.katex` inside a tracked `<p>` would otherwise
    inherit the tracking and distort fixed-width glyphs / math metrics. A direct rule on the child
    beats the inherited value regardless of the ancestor's higher specificity.
  - **Chrome leak (same gotcha as line-spacing):** `nav li, [role="listbox"] li, .sidenote p,
    .marginnote p, .column-margin p, .aside p { letter-spacing: normal; word-spacing: normal; }` —
    navigation (TOC / sidebar / postnav / navbar), search results, and margin notes wrap prose in
    `<li>`/`<p>` and must keep default spacing. (`normal`, not `inherit`: chrome sets no custom
    tracking, and a hard reset can't re-propagate a future leak.)
- **`qmdInitReaderPrefs`**: two `seg(...)` rows after Line spacing —
  `LETTER = [['0','Normal'],['0.06em','Wide'],['0.12em','Wider']]`,
  `WORD = [['0','Normal'],['0.08em','Wide'],['0.16em','Wider']]`; `curLetter/curWord` default
  `'0'`; picking Normal stores `null` (clears → falls back to `normal`). Both added to `syncAll`.

## Invariants

Reader-side + read-only; pre-paint (no flash); offline; additive (two CSS vars + two menu rows + a
few lines in the pre-paint script); no block-model / output / Rust-pipeline change; idempotent;
decks unaffected (no reader menu on decks). Does not touch code, math, headings, or tables.

## Verification

- **Rust test** (`render/tests.rs`): the page ships `--qmd-reader-letter` and `--qmd-reader-word`.
- **Browser (chrome-devtools MCP):** open the reader menu → Letter spacing + Word spacing rows;
  pick Wider on each → a prose `<p>` computed `letter-spacing`/`word-spacing` increases while an
  inline `code`, a `pre`, the `.katex`, a heading, and a TOC `<li>` stay `normal`/0; the choices
  persist across reload (pre-paint, no flash) and sync the pressed state; Reset clears them; a deck
  shows no control. Reuses the `corpus/reader/preferences.qmd` pin (prose + code + inline/display
  math + a figure caption).
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt` + `tsc`; a focused adversarial review.

## Files

`crates/core/src/render/theme.rs` (applyReader + reset), `crates/core/assets/css/base.css` (the
prose tracking vars + monospace/math + chrome re-pins), `crates/core/assets/js/code-enhance.js`
(the two Display rows), a test in `crates/core/src/render/tests.rs`. No new corpus pin.
