# Reader Preferences ("Reading settings") — design

> Status: building (2026-06-25, branch `feat/reader-preferences`). The first
> reader-experience feature from `FEATURE-IDEAS.md` (#3). Picked as highest-impact after
> verifying that the audit's #1 pick (hover cross-reference cards) already ships as
> `qmdInitLinkPreview`.

## Problem

qmd-fast's output gives the reader exactly one comfort control: a light/dark toggle. Text
size and reading width are fixed at the author's choice. This punishes readers on phones
(17px is small), on wide monitors (a 46rem measure is fine but some want narrower/wider),
at night/outside (no warm sepia), and anyone who needs larger text (WCAG 1.4.12 says reader
text settings must be overridable). Every e-reader and read-later app has solved this; a
qmd-fast book/post has not.

## Goal

A reader-side "Reading settings" control (an **Aa** floating button) offering, persisted
locally and applied with **no flash**:

- **Theme**: Light / Dark / **Sepia** (extends the existing `data-theme` set).
- **Text size**: 5 steps via a `--qmd-reader-scale` root multiplier (≈0.85–1.4).
- **Reading width**: Narrow / Normal / Wide via a `--qmd-maxw` override.
- **Reset**.

Out of scope this pass (follow-ups noted): high-contrast theme, comfortable line-spacing
toggle, serif/sans/dyslexia font switch, hyphenation toggle. (Each is an additive control on
the same substrate this builds.)

## Invariants honored

Reader-side only: every preference is stored in the **reader's own** `localStorage` and
applied to the rendered DOM via CSS variables / `data-theme`. It never writes the author's
`.qmd` (single editing surface intact), adds no output format (HTML-only intact), bundles
offline (no network, an additive enhancer over the `qmdEnhancers` seam + the existing
pre-paint theme pipeline), and touches no Do-NOT-touch machinery (no block model / diff /
sourcepos / cite / includes / exec change). Decks are excluded (own chrome).

## Mechanism

1. **Scaling substrate (CSS, `base.css`).**
   - `html { font-size: calc(100% * var(--qmd-reader-scale, 1)); }` — one multiplier scales
     every `rem`/`em` content (headings already `rem`).
   - `--qmd-font-body` changes `17px` → `1.0625rem` (identical at scale 1, but now scales
     with the root). Line-height unchanged.
   - Reading width = the panel overriding `--qmd-maxw` on `html` (narrow 38rem / normal 46rem
     / wide 58rem).
   - `html[data-theme="sepia"] { ... }` — a warm palette (`--qmd-bg #f4ecd8`, `--qmd-fg
     #5b4636`, muted/border/code-bg/link tuned warm); `color-scheme: light`.

2. **No-flash apply (pre-paint script, `theme.rs::theme_head`).** The existing pre-paint head
   script already sets `data-theme` + inline bg before paint. Extend it to:
   - accept `sepia` as a valid stored `qmd-theme` (+ its inline bg `#f4ecd8`, color-scheme
     light);
   - read `qmd-reader-scale` + `qmd-reader-width` from `localStorage` and set them as inline
     `--qmd-reader-scale` / `--qmd-maxw` on `<html>` *before paint* (so a returning reader
     never sees a flash of the default size/width);
   - expose `window.qmdSetReaderPref(key, val)` (setItem + apply) and `qmdGetReaderPref(key)`,
     mirroring `qmdSetTheme`/`qmdGetThemePref`. The apply lives in the head so it is the
     single source of truth for both first paint and runtime changes.

3. **The UI (enhancer, `code-enhance.js`).** A new `qmdInitReaderPrefs()` registered on the
   `qmdEnhancers` registry (so it ships in `code_scripts()` on every built page + the
   preview). It: returns early on a deck; injects one fixed **Aa** button (bottom-right) +
   a popover with the theme / size / width / reset controls; reflects the current values;
   on change calls `qmdSetTheme` / `qmdSetReaderPref` (no bespoke persistence). ARIA:
   the button is a labeled `<button aria-haspopup>`, the popover is keyboard-dismissable
   (Esc), focus-trapped lightly. Styled via additive `base.css` rules (`.qmd-reader-*`).

The Aa button cycles independently of the existing light/dark navbar toggle; both call the
same `qmdSetTheme`, and a `qmd:themechange` event keeps them in sync.

## Verification

- **Corpus pin:** `corpus/reader/preferences.qmd` (a reading-oriented doc: headings, prose,
  a figure) is the target; the existing corpus block-invariant test covers it for free.
- **Rust test** (`render/tests.rs` or `page.rs`): the assembled page for a normal doc ships
  the reader-prefs enhancer (`qmdInitReaderPrefs`) and the pre-paint reader apply
  (`qmd-reader-scale`). Guards the wiring so it can't silently drop out.
- **Browser (chrome-devtools MCP):** Aa opens; changing size/width/theme applies live;
  reload preserves them with no flash; a deck shows no Aa button; light/dark navbar toggle
  stays in sync.
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt`; `tsc` type-check of the client.

## Files

`crates/core/src/render/theme.rs` (pre-paint extension), `crates/core/assets/css/base.css`
(scaling vars + sepia + Aa/panel styling), `crates/core/assets/js/code-enhance.js`
(the `qmdInitReaderPrefs` enhancer), `corpus/reader/preferences.qmd` (pin), a test in
`crates/core/src/render/tests.rs`.
