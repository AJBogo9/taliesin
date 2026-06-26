# Reader focus / reading mode — design

> Status: shipped (2026-06-26, `feat/reader-focus-mode` → main). A toggle that hides
> site chrome and centers the prose into one calm column for distraction-free reading.
> Browser-verified (chrome-devtools) on all three layouts — single-doc, book, site:
> `f`/Esc/menu toggle, chrome hidden, prose centered at the measure, launcher + progress
> bar kept, `f` ignored while typing, no console errors. (Verification caught that the
> book sidebar is `.qmd-book-sidebar`, not `.qmd-book` — the latter is the whole wrapper.)

## Problem

The reading surface carries chrome that helps you navigate but competes with the words
once you've settled in: a navbar, a TOC rail, a footer, a book sidebar, dev controls. A
reader who wants to *just read* a long passage has no way to clear it. Every e-reader and
read-later app has a one-key "focus / immersive" mode; qmd-fast lacks one.

## Goal

A toggle that hides site chrome and centers the prose into a single calm column; toggle
again or **Esc** to restore. Reader-side, CSS-driven via a `body.qmd-focus` class — the
enhancer flips the class and wires the triggers; all the hiding/centering is CSS.

- **Trigger:** the **`f` key** (guarded — ignored while typing in an input and while a modal
  is open) **and** a **"Focus" toggle in the Reader menu** (discoverable, `aria-pressed`);
  **Esc** exits (only when no modal is open, so it doesn't steal Esc from Cmd-K / lightbox).
- **Hidden** (`display:none` under `body.qmd-focus`): navbar (`.qmd-site-nav`), footer
  (`.qmd-site-footer`), TOC (`#TOC`, `#qmd-toc-handle`, `#qmd-toc-backdrop`), book sidebar
  (`.qmd-book-sidebar` — *not* `.qmd-book`, which is the whole book wrapper), dev controls
  (`#qmd-controls`). Each layout's content|TOC grid (single-doc body, `.qmd-book-inner`,
  `.qmd-site-main`) collapses to a block centered at `--qmd-maxw`.
- **Kept:** the prose, the reading-progress bar (`.qmd-readbar`), and the reader-menu
  launcher (`.qmd-rmenu-toggle`) — so mouse users keep an exit path and their size/theme
  controls.
- **Layout:** drop the `has-toc` grid → a single column centered at the *same* comfortable
  `--qmd-maxw`. Deliberately **not** wider lines (a longer measure hurts readability);
  "one column" = no sidebar, not longer lines.
- **Persistence:** **ephemeral** — focus is a momentary action, not a stored preference
  like theme/size, so it does not persist; reload/navigation returns to normal. (A returning
  reader should not find all chrome mysteriously gone.)

Reader-side, read-only (no source write, no `localStorage`). Deck-skipped (own chrome).

## Invariants honored

- Reader-side, read-only, offline, additive; no block-model change.
- Decks excluded: the enhancer returns early on `.qmd-deck`.
- Idempotent: one-time setup guarded by `window.__qmdFocus`.
- No flash concern: focus is reader-initiated after load (never the initial state), so no
  pre-paint script is needed.

## Mechanism

All in `crates/core/assets/js/code-enhance.js`, enhancer `qmdInitFocusMode()` (one-time
setup, registered with the others):

- A `setFocus(on)` toggles `document.body.classList` `qmd-focus`, sets the menu button's
  `aria-pressed`, and announces "Focus mode on/off" via an owned `aria-live` span.
- **Reader-menu toggle:** if `window.qmdReaderMenu` exists, `addSection('Focus', node, sync)`
  with a button labelled "Focus mode" (`aria-pressed`) plus a muted "press f" hint; clicking
  toggles and closes the menu. `sync` (the section's `onOpen`) refreshes `aria-pressed`.
- **Keyboard:** a `keydown` listener — `f` (no modifier, not while typing, not while a
  `[aria-modal="true"]` is open) toggles; `Escape` exits when focused and no modal is open.

CSS (`crates/core/assets/css/base.css`):

- `body.qmd-focus :is(.qmd-site-nav, .qmd-site-footer, #TOC, #qmd-toc-handle,
  #qmd-toc-backdrop, .qmd-book, #qmd-controls) { display: none !important; }`
- Re-centre the reading column for each layout that uses the TOC grid / site / book
  wrappers: `body.qmd-focus.has-toc`, `body.qmd-focus .qmd-site-main`,
  `body.qmd-focus .qmd-book-inner` collapse to a single block centred at `--qmd-maxw`
  (tuned empirically in the browser across single-doc / site / book).
- A subtle transition is optional; keep it instant to avoid reflow jank.

## Verification

- **Corpus pin:** reuse a TOC-bearing doc (`corpus/reader/long-read.qmd`) — no new doc.
- **Rust test** (`render/tests.rs`): `assembled_page_ships_focus_mode` — `render_html_page`
  contains `qmdInitFocusMode`.
- **Browser (chrome-devtools MCP):** on a doc with a TOC, press `f` → chrome hides, prose
  centers, the menu launcher + progress bar remain; `f`/`Esc` restore; the Reader-menu
  "Focus" toggle works + reflects `aria-pressed`; `f` is ignored while a text field is
  focused; no console errors. Spot-check a site/book layout if available.
- **Gates:** `cargo test -p qmd-fast-core`, `clippy -D warnings`, `fmt`, `tsc`.

## Files

- `crates/core/assets/js/code-enhance.js` — `qmdInitFocusMode` + registration.
- `crates/core/assets/css/base.css` — `body.qmd-focus` rules.
- `crates/core/src/render/tests.rs` — `assembled_page_ships_focus_mode`.
