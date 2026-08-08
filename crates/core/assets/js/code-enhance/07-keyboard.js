// Keyboard reader: left/right move to the previous/next chapter (the book prev/next
// anchors). Guarded so it never fires while typing or under a modal. Read-only,
// idempotent.
//
// The `?` (open Settings) and `/` (open search) character-key shortcuts, and the
// WCAG 2.1.4 off-switch they forced into the Settings menu (the shared reader-preference
// accessors this used to call, defined in 01-registry.js, plus the cheatsheet list this
// file used to mount there), were deleted 2026-08-04: Esc and the arrow keys are not
// character keys, so they carry no such obligation and stay live with no control.
function taliInitKeyboard() {
  if (window.__taliKeyboard) return;
  window.__taliKeyboard = true;

  document.addEventListener('keydown', function (e) {
    var t = /** @type {HTMLElement | null} */ (e.target);
    var typing =
      t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable);
    var modal = document.querySelector('[aria-modal="true"]');
    if (typing || e.metaKey || e.ctrlKey || e.altKey) return;
    if (modal) return;
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      // leave arrows to a focused interactive control (slider, tablist, link, button)
      if (t && t.closest && t.closest('a,button,input,select,textarea,[role="tab"]')) return;
      var nav = /** @type {HTMLAnchorElement | null} */ (document.querySelector(e.key === 'ArrowRight' ? '.tali-book-next' : '.tali-book-prev'));
      if (nav && nav.href) { e.preventDefault(); window.location.assign(nav.href); }
    }
  });
}
