# Modal focus trap (a11y) — design

> Status: building (2026-06-25, branch `feat/modal-focus-trap`, ultracode). The audit's
> small a11y item. The modals shipped this session handle Esc but don't trap Tab focus or set
> `aria-modal`, so keyboard / screen-reader users can Tab into the page behind an open modal,
> and focus isn't restored on close. Hand-rolled (the audit rejected the `focus-trap` npm dep:
> it pulls in `tabbable` to save ~20 lines, against the vendor-single-audited-files discipline).

## Scope

Two genuine, viewport-covering modal surfaces, both on pages that load `code-enhance.js`:

- **Lightbox** `#qmd-lightbox` (`code-enhance.js`) — image/diagram/video zoom; backdrop +
  scroll-locked; focusable = the close button.
- **Cmd-K palette** `#qmd-search` (`web-client/search.js`) — full-screen backdrop; focusable =
  the input (results are `role=option` driven by `aria-activedescendant`, not tab stops).

**The reader menu (`.qmd-rmenu-panel`) is deliberately NOT trapped.** An adversarial review
showed it is a light-dismiss *popover*, not a modal — it doesn't cover or inert the page, so
`aria-modal` would misrepresent the rest of the document as inert to a screen reader; the
modal trap also fought the menu's jump buttons (which navigate, not return to the launcher) and
its outside-click dismissal, and relying on the trap's focus-restore regressed Esc-return on
engines that don't focus a `<button>` on click. It keeps its prior popover shape:
`aria-expanded` on the launcher + Esc-to-close (with an explicit `launcher.focus()`) +
click-away. Deck control menus (`deck.js`, a separate context) are out of scope.

## Mechanism — one shared utility

`window.qmdFocusTrap(container, initial)` defined once in `code-enhance.js` (top-level,
`if (!window.qmdFocusTrap)`), reused by the lightbox + reader menu in-file and by `search.js`
via the global (defined long before any Cmd-K open):

- On call: remember `document.activeElement`; set `container` `aria-modal="true"`; add a
  **document-level capture** `keydown` listener that, on **Tab**, computes the visible focusable
  set within `container` (`a[href]`, enabled `button`/`input`/`select`/`textarea`,
  `[tabindex]:not([tabindex="-1"])`, filtered to rendered) and wraps: Shift+Tab on the first (or
  focus outside) → last; Tab on the last (or focus outside) → first; if focus has escaped the
  container, force it back to the first. Focus `initial` (or the first focusable, or the
  container).
- Returns a `release()` that removes the listener, removes `aria-modal`, and restores focus to
  the remembered element (guarded in try/catch).

Document-capture (not a container listener) so a Tab is trapped even if focus has drifted out
(e.g. a stray mouse click); only one modal is open at a time in practice.

## Wiring

- **Lightbox:** a `markOpen()` helper (replacing the three duplicated `add('open') +
  overflow:hidden` blocks) activates the trap (focus the close button) once; `close()` releases.
- **Cmd-K:** `open()` activates focusing the input (replacing the bare `input.focus()`);
  `close()` releases. Guarded on `window.qmdFocusTrap` so search degrades gracefully if absent.
- **Reader menu:** unchanged (popover; see Scope).

## Invariants

Additive JS only; no block-model / output / Rust change; offline; idempotent (the utility is
defined once; each modal stores its single `release`). No `innerHTML`. Decks unaffected
(lightbox/menu/search aren't on decks; the utility is just an unused definition there).

## Verification

- **Rust test** (`render/tests.rs`): the page ships `qmdFocusTrap`.
- **Browser (chrome-devtools MCP):** for the lightbox + Cmd-K — open it, assert `aria-modal="true"`
  and focus moved inside; dispatch Tab/Shift+Tab at the last/first focusable and assert focus
  wraps within the modal (never to a page element behind it); Esc/close and assert focus restored
  to the opener. Also assert the smart restore (focus moved to the page → close leaves it there).
  Reader menu: confirm it is NOT trapped (no `aria-modal`) and Esc still returns focus to the
  launcher. Verified over the served `docs/internals` book + a doc page.
- **Gates:** `cargo test` + `clippy -D warnings` + `fmt` + `tsc`. Adversarial-review workflow on
  the focus edge cases before merge.

## Files

`crates/core/assets/js/code-enhance.js` (the utility + lightbox + reader-menu wiring),
`web-client/search.js` (the palette wiring), a test in `crates/core/src/render/tests.rs`.
