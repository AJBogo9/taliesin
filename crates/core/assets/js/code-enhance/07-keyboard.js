// Keyboard reader: `?` opens the Settings menu (which lists these shortcuts), `/` opens search,
// left/right move to the previous/next chapter (the book prev/next anchors). The shortcut list
// is mounted as a section of the Settings menu, so it is a visible cheatsheet, not a separate
// dialog. All guarded so they never fire while typing or under a modal. Read-only, deck-skipped,
// idempotent.
function taliInitKeyboard() {
  if (window.__qmdKeyboard) return;
  if (document.querySelector('.tali-deck')) return;
  window.__qmdKeyboard = true;

  // Mount the shortcut list into the Settings menu (built by taliInitReaderMenu, which runs
  // first via the registry order). A static list of literal <kbd>s, no interpolation.
  if (window.taliReaderMenu) {
    var dl = document.createElement('dl');
    dl.className = 'tali-keys-list';
    dl.innerHTML =
      '<div><dt><kbd>?</kbd></dt><dd>Open settings</dd></div>' +
      '<div><dt><kbd>/</kbd></dt><dd>Search</dd></div>' +
      '<div><dt><kbd>f</kbd></dt><dd>Focus mode</dd></div>' +
      '<div><dt><kbd>&larr;</kbd> <kbd>&rarr;</kbd></dt><dd>Previous / next chapter</dd></div>' +
      '<div><dt><kbd>Esc</kbd></dt><dd>Close</dd></div>';

    // WCAG 2.1.4's turn-off mechanism, sitting directly above the list it governs. Same shape as
    // the Focus mode row (03-focus-mode.js): a one-button `.tali-reader-seg` reading On/Off with
    // aria-pressed. It is a sibling of the list, never inside it, so switching shortcuts OFF can
    // never hide the control that switches them back ON.
    var row = document.createElement('div');
    row.className = 'tali-reader-row';
    var label = document.createElement('span');
    label.textContent = 'Shortcuts';
    var seg = document.createElement('div');
    seg.className = 'tali-reader-seg';
    var btn = document.createElement('button');
    btn.type = 'button';
    btn.title = 'Single-key shortcuts (f, ?, /)';
    var syncKeys = function () {
      var on = taliShortcutsOn();
      btn.textContent = on ? 'On' : 'Off';
      btn.setAttribute('aria-pressed', on ? 'true' : 'false');
      dl.hidden = !on; // don't advertise dead keys
    };
    btn.addEventListener('click', function () { taliSetShortcuts(!taliShortcutsOn()); syncKeys(); });
    seg.appendChild(btn);
    row.appendChild(label);
    row.appendChild(seg);

    var box = document.createElement('div');
    box.appendChild(row);
    box.appendChild(dl);
    window.taliReaderMenu.addSection('Keyboard shortcuts', box, syncKeys);
  }

  document.addEventListener('keydown', function (e) {
    var t = e.target;
    var typing =
      t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable);
    var modal = document.querySelector('[aria-modal="true"]');
    // `?` (Shift+/) toggles the Settings menu (which shows this list).
    if (e.key === '?' && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
      if (modal) return; // a modal owns the keys
      if (!taliShortcutsOn()) return;
      if (window.taliReaderMenu) { e.preventDefault(); window.taliReaderMenu.toggle(); }
      return;
    }
    if (typing || e.metaKey || e.ctrlKey || e.altKey) return;
    if (modal) return;
    if (e.key === '/') {
      if (!taliShortcutsOn()) return;
      if (window.taliOpenSearch) { e.preventDefault(); window.taliOpenSearch(); }
      return;
    }
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      // leave arrows to a focused interactive control (slider, tablist, link, button)
      if (t && t.closest && t.closest('a,button,input,select,textarea,[role="tab"]')) return;
      var nav = document.querySelector(e.key === 'ArrowRight' ? '.tali-book-next' : '.tali-book-prev');
      if (nav && nav.href) { e.preventDefault(); window.location.assign(nav.href); }
    }
  });
}
