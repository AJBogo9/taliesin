// Visible focusable descendants of `container`, in DOM order, for the modal trap below. The
// `el === document.activeElement` clause keeps a zero-size element that currently holds focus.
var TALI_FOCUS_SEL = 'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';
/** @param {Element} container @returns {HTMLElement[]} */
function taliFocusables(container) {
  return /** @type {HTMLElement[]} */ ([].slice.call(container.querySelectorAll(TALI_FOCUS_SEL))).filter(function (el) {
    return el.offsetWidth > 0 || el.offsetHeight > 0 || el === document.activeElement;
  });
}

// Shared modal focus trap: while a modal is open, confine Tab/Shift+Tab to `container`, mark it
// aria-modal, and (on release) restore focus to the opener IF focus is still inside (a keyboard
// or programmatic close) — not when the user clicked elsewhere. Used, via this global, by the
// Cmd-K palette in search.js and the book's chapter drawer. Returns release().
window.taliFocusTrap = window.taliFocusTrap || function (container, initial) {
  var prev = /** @type {HTMLElement | null} */ (document.activeElement);
  container.setAttribute('aria-modal', 'true');
  /** @param {KeyboardEvent} e */
  function onKey(e) {
    if (e.key !== 'Tab') return;
    var f = taliFocusables(container);
    if (!f.length) { e.preventDefault(); return; }
    var first = f[0], last = f[f.length - 1], a = document.activeElement;
    if (!container.contains(a)) { e.preventDefault(); first.focus(); return; }
    if (e.shiftKey) { if (a === first) { e.preventDefault(); last.focus(); } }
    else if (a === last) { e.preventDefault(); first.focus(); }
  }
  document.addEventListener('keydown', onKey, true);
  try { /** @type {HTMLElement} */ (initial || taliFocusables(container)[0] || container).focus(); } catch (e) {}
  return function () {
    document.removeEventListener('keydown', onKey, true);
    container.removeAttribute('aria-modal');
    if (container.contains(document.activeElement) && prev && prev.focus) {
      try { prev.focus(); } catch (e) {}
    }
  };
};

