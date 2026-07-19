# PL8 — `<meta name="theme-color">` (dynamic) + `<meta name="generator">`

## Problem

The page head had neither a `generator` meta nor a `theme-color` meta:

- **No `theme-color`** → mobile browser chrome (the status/URL bar) stays **white against a
  dark page**, a jarring seam the rest of the theme system doesn't have.
- **No `generator`** → the HTML doesn't advertise the tool (the Atom feed already does, via
  `<generator>Taliesin</generator>`), a small provenance gap.

## Fix

1. **`generator` (static).** Emit `<meta name="generator" content="Taliesin" />` in the head
   (`page.rs` `PAGE_TEMPLATE`), beside the existing charset/viewport/referrer metas.

2. **`theme-color` (dynamic, zero hex duplication).** The pre-paint theme script (`theme.rs`
   `theme_head`) already resolves the active mode and paints the canvas from a `BG` map
   (`{ dark:'#16181d', sepia:'#f4ecd8', light:'#ffffff' }`) at one `apply()` choke point that
   **every** theme change routes through (initial load, the in-page toggle via `taliSetTheme`,
   and the OS-scheme listener). Create-or-update a `<meta name="theme-color">` there and set
   its content to the **same** `BG[mode]`. So the chrome tint:
   - follows the reader's **in-page** toggle, not just the OS scheme, and
   - reuses the one `BG` map — **no** literal hex is re-typed in Rust (avoiding the very
     drift PL4 is about).

   The meta is created by the script rather than emitted statically so its value is never a
   stale build-time literal; for a no-JS reader the page also isn't theme-switched, so a
   default (light) chrome stays consistent.

Frozen `qmd-theme` / `qmd:themechange` names untouched.

## Tests

- `crates/core/tests/head_meta.rs`:
  - `head_advertises_the_generator` — the built page head carries the generator meta.
  - `pre_paint_script_keeps_a_theme_color_meta_in_sync` — the emitted head script creates a
    theme-color meta and sets its content from `BG[mode]` (pins the content-set line, not just
    presence). Mutation-checked: dropping either the meta or the content-set fails a test.
- **Browser-verified** (isolated puppeteer-core, since the chrome-devtools profile was held):
  across the 3-viewport matrix (mobile/laptop/portrait) × {light,dark,sepia} the `theme-color`
  meta equals the matching `BG`, and a runtime `taliSetTheme('light')` from dark flips it
  `#16181d → #ffffff` — proving it follows the in-page toggle.
