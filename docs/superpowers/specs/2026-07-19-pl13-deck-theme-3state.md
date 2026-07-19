# PL13 — Deck theme: 3-state Auto/Light/Dark (mirror the page)

A standalone deck's stored toggle won in `resolve()` **forever** with no clear-path
(`deck.rs`; `taliDeckSetTheme` persisted to `localStorage['qmd-deck-theme']`), a binary
"Dark mode" On/Off item. Tap "Dark" once on a shared deck at night and it's opted out of
daylight light mode with no visible undo — while a *page* reader gets Auto/Light/Dark where
"Auto" clears the key and resumes OS-follow. Scope: standalone decks only (an embedded deck
correctly follows its host, unchanged).

## Fix

- **`deck.rs` head script:** `taliDeckSetTheme(m)` now CLEARS the stored key for any value that
  isn't `light`/`dark` (so `'auto'` resumes OS-follow), mirroring the page's `taliSetTheme`. Add
  `taliDeckThemeChoice()` → the current choice (`'auto'` when no key). Add a
  `matchMedia('(prefers-color-scheme: dark)')` listener so a standalone deck in Auto follows a
  **live** OS flip (the page already did this). `resolve()` already fell through to OS when the
  key was absent, so no logic change there.
- **`deck.js` menu:** replace the binary "Dark mode" tool with a 3-state
  `Auto | Light | Dark` segment (`.tali-theme-seg` of `.tali-theme-opt` buttons, `aria-pressed`
  on the active choice). Clicking an option calls `taliDeckSetTheme(choice)`; the segment
  reflects `taliDeckThemeChoice()` on open + after each change (`updateThemeSeg`, from
  `markActiveTools`). The old `toggleThemeMode`/`.tali-theme-state` are gone (no orphans).
- **`deck.css`:** `.tali-theme-seg` / `.tali-theme-opt` styled to match the menu (active option
  washed in `--deck-accent`, like `.tali-menu-item.tali-on`).

## Tests / verification

- `render::tests::deck_theme_is_custom_and_head_gating` extended: the head exposes
  `taliDeckThemeChoice`, clears the key (`removeItem('qmd-deck-theme')`) for Auto, and wires the
  live OS listener.
- **Browser-verified** (isolated puppeteer): the segment is Auto/Light/Dark; initial = no key →
  OS-follow, Auto pressed; click Dark → `dark` + key stored + Dark pressed; click Auto → key
  CLEARED + resumes OS light + Auto pressed (the exact regression the binary toggle couldn't
  undo); reload + OS→dark → no key, the deck follows the OS. Screenshot confirms the segment.
