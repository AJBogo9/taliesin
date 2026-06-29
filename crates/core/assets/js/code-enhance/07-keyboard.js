// Keyboard reader: `?` opens a shortcuts cheatsheet, `/` opens search, left/right move
// to the previous/next chapter (the book prev/next anchors). All guarded so they never
// fire while typing or under another modal. Read-only, deck-skipped, idempotent.
function qmdInitKeyboard() {
  if (window.__qmdKeyboard) return;
  if (document.querySelector('.qmd-deck')) return;
  window.__qmdKeyboard = true;

  var sheet = null;
  var sheetRelease = null;
  function buildSheet() {
    var wrap = document.createElement('div');
    wrap.className = 'qmd-keys';
    wrap.setAttribute('role', 'dialog');
    wrap.setAttribute('aria-modal', 'true');
    wrap.setAttribute('aria-label', 'Keyboard shortcuts');
    wrap.hidden = true;
    var card = document.createElement('div');
    card.className = 'qmd-keys-card';
    card.innerHTML =
      '<h2>Keyboard shortcuts</h2>' +
      '<dl class="qmd-keys-list">' +
      '<div><dt><kbd>?</kbd></dt><dd>Show this help</dd></div>' +
      '<div><dt><kbd>/</kbd></dt><dd>Search</dd></div>' +
      '<div><dt><kbd>f</kbd></dt><dd>Focus mode</dd></div>' +
      '<div><dt><kbd>&larr;</kbd> <kbd>&rarr;</kbd></dt><dd>Previous / next chapter</dd></div>' +
      '<div><dt><kbd>Esc</kbd></dt><dd>Close</dd></div>' +
      '</dl>';
    var close = document.createElement('button');
    close.className = 'qmd-keys-close';
    close.type = 'button';
    close.setAttribute('aria-label', 'Close');
    close.textContent = '×';
    card.appendChild(close);
    wrap.appendChild(card);
    document.body.appendChild(wrap);
    close.addEventListener('click', closeSheet);
    wrap.addEventListener('click', function (e) { if (e.target === wrap) closeSheet(); });
    sheet = wrap;
  }
  function sheetOpen() { return !!sheet && !sheet.hidden; }
  function openSheet() {
    if (!sheet) buildSheet();
    sheet.hidden = false;
    if (window.qmdFocusTrap) {
      sheetRelease = window.qmdFocusTrap(sheet, sheet.querySelector('.qmd-keys-close'));
    }
  }
  function closeSheet() {
    if (!sheetOpen()) return;
    sheet.hidden = true;
    if (sheetRelease) { sheetRelease(); sheetRelease = null; }
  }

  document.addEventListener('keydown', function (e) {
    var t = e.target;
    var typing =
      t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable);
    var modal = document.querySelector('[aria-modal="true"]');
    // `?` (Shift+/) toggles help — allowed even when the cheatsheet itself is open.
    if (e.key === '?' && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
      if (modal && !sheetOpen()) return; // a different modal owns the keys
      e.preventDefault();
      if (sheetOpen()) closeSheet(); else openSheet();
      return;
    }
    if (typing || e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === 'Escape' && sheetOpen()) { e.preventDefault(); closeSheet(); return; }
    if (modal) return;
    if (e.key === '/') {
      if (window.qmdOpenSearch) { e.preventDefault(); window.qmdOpenSearch(); }
      return;
    }
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      // leave arrows to a focused interactive control (slider, tablist, link, button)
      if (t && t.closest && t.closest('a,button,input,select,textarea,[role="tab"]')) return;
      var nav = document.querySelector(e.key === 'ArrowRight' ? '.qmd-book-next' : '.qmd-book-prev');
      if (nav && nav.href) { e.preventDefault(); window.location.assign(nav.href); }
    }
  });
}

