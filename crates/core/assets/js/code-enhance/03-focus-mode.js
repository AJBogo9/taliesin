// Focus / reading mode: hide site chrome and centre the prose into one calm column for
// distraction-free reading. Reader-side, ephemeral (no localStorage) — toggled by the `f`
// key (ignored while typing or while a modal is open), Esc, or a Reader-menu toggle. All
// the hiding/centring is CSS on body.tali-focus; this just flips the class + wires triggers.
//
// Focus mode and OS fullscreen are two SEPARATE affordances (deliberately decoupled): a
// reader can get the calm column without going fullscreen, and can go fullscreen without
// the calm column. Each has its own Reader-menu toggle; `f` drives focus only.
function taliInitFocusMode() {
  if (document.querySelector('.tali-deck')) return;
  if (window.__taliFocus) return;
  window.__taliFocus = true;

  var live = document.createElement('span');
  live.className = 'tali-sr-only';
  live.setAttribute('aria-live', 'polite');
  document.body.appendChild(live);

  var btn = /** @type {HTMLButtonElement | null} */ (null);
  function on() { return document.body.classList.contains('tali-focus'); }
  function sync() {
    if (!btn) return;
    btn.setAttribute('aria-pressed', on() ? 'true' : 'false');
    btn.textContent = on() ? 'On' : 'Off';
  }
  /** @param {boolean} v */
  function setFocus(v) {
    document.body.classList.toggle('tali-focus', v);
    sync();
    live.textContent = '';
    live.textContent = v ? 'Focus mode on' : 'Focus mode off';
  }

  // Fullscreen is its own toggle (was welded to focus mode; split so each stands alone).
  // Best-effort: `requestFullscreen` needs a user gesture — the menu button and the `F`
  // key both are — and it degrades silently where the API is blocked.
  var fsBtn = /** @type {HTMLButtonElement | null} */ (null);
  function fullOn() { return !!document.fullscreenElement; }
  function syncFs() {
    if (!fsBtn) return;
    fsBtn.setAttribute('aria-pressed', fullOn() ? 'true' : 'false');
    fsBtn.textContent = fullOn() ? 'On' : 'Off';
  }
  /** @param {boolean} v */
  function setFullscreen(v) {
    try {
      var el = document.documentElement;
      if (v && !document.fullscreenElement && el.requestFullscreen) {
        var p = el.requestFullscreen();
        if (p && p.catch) p.catch(function () {});
      } else if (!v && document.fullscreenElement && document.exitFullscreen) {
        var q = document.exitFullscreen();
        if (q && q.catch) q.catch(function () {});
      }
    } catch (e) {}
  }
  // Keep the fullscreen toggle in sync when the reader enters/leaves fullscreen through the
  // browser (F11 / Esc). Focus mode is untouched — the two no longer desync because they no
  // longer track each other.
  document.addEventListener('fullscreenchange', syncFs);

  // Settings-menu toggles (discoverable). The launcher stays visible in focus mode, so this
  // remains the mouse exit + the theme control.
  if (window.taliReaderMenu) {
    var rm = window.taliReaderMenu; // captured non-undefined for the click closures
    /**
     * @param {string} labelText
     * @param {string} titleText
     * @returns {HTMLButtonElement}
     */
    function menuToggle(labelText, titleText) {
      var row = document.createElement('div');
      row.className = 'tali-reader-row';
      var label = document.createElement('span');
      label.textContent = labelText;
      var seg = document.createElement('div');
      seg.className = 'tali-reader-seg';
      var b = document.createElement('button');
      b.type = 'button';
      b.textContent = 'Off';
      b.setAttribute('aria-pressed', 'false');
      b.title = titleText;
      seg.appendChild(b);
      row.appendChild(label);
      row.appendChild(seg);
      return b;
    }

    btn = menuToggle('Focus mode', 'Hide chrome for distraction-free reading (press f)');
    btn.addEventListener('click', function () { setFocus(!on()); rm.close(); });
    rm.addSection('', /** @type {HTMLElement} */ (btn.closest('.tali-reader-row')), sync);

    fsBtn = menuToggle('Fullscreen', 'Fill the screen (press F)');
    fsBtn.addEventListener('click', function () { setFullscreen(!fullOn()); rm.close(); });
    rm.addSection('', /** @type {HTMLElement} */ (fsBtn.closest('.tali-reader-row')), syncFs);
  }

  // `f` toggles focus; `F` (Shift-f) toggles fullscreen; Esc exits focus. All are off while
  // typing in a field or while a modal ([aria-modal="true"] — the Cmd-K palette / lightbox)
  // is open, so they never steal keys.
  document.addEventListener('keydown', function (e) {
    var t = /** @type {HTMLElement | null} */ (e.target);
    if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable)) return;
    var modal = document.querySelector('[aria-modal="true"]');
    if (e.key === 'Escape') {
      if (on() && !modal) setFocus(false);
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey || modal) return;
    if (e.key === 'f') {
      if (!taliShortcutsOn()) return;
      e.preventDefault();
      setFocus(!on());
    } else if (e.key === 'F') {
      if (!taliShortcutsOn()) return;
      e.preventDefault();
      setFullscreen(!fullOn());
    }
  });
}
