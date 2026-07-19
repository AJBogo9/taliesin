// Focus / reading mode: hide site chrome and centre the prose into one calm column for
// distraction-free reading. Reader-side, ephemeral (no localStorage) — toggled by the `f`
// key (ignored while typing or while a modal is open), Esc, or a Reader-menu toggle. All
// the hiding/centring is CSS on body.tali-focus; this just flips the class + wires triggers.
function taliInitFocusMode() {
  if (document.querySelector('.tali-deck')) return;
  if (window.__qmdFocus) return;
  window.__qmdFocus = true;

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
  // Focus mode also enters native fullscreen so nothing but the text remains (the
  // author's ask). Best-effort: `requestFullscreen` needs a user gesture — the `f` key
  // and the menu button both are — and it degrades silently where the API is blocked.
  // Exiting focus mode leaves fullscreen; leaving fullscreen via the browser (F11/Esc)
  // exits focus mode (the fullscreenchange sync below), so the two stay coupled.
  /** @param {boolean} v */
  function goFullscreen(v) {
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
  /** @param {boolean} v */
  function setFocus(v) {
    document.body.classList.toggle('tali-focus', v);
    goFullscreen(v);
    sync();
    live.textContent = '';
    live.textContent = v ? 'Focus mode on' : 'Focus mode off';
  }
  // If the reader leaves fullscreen through the browser (F11 / Esc) while focus mode is
  // on, drop focus mode too so the two never desync.
  document.addEventListener('fullscreenchange', function () {
    if (!document.fullscreenElement && on()) setFocus(false);
  });

  // Settings-menu toggle (discoverable). The launcher stays visible in focus mode, so this
  // remains the mouse exit + the theme control.
  if (window.taliReaderMenu) {
    var rm = window.taliReaderMenu; // captured non-undefined for the click closure
    var row = document.createElement('div');
    row.className = 'tali-reader-row';
    var label = document.createElement('span');
    label.textContent = 'Focus mode';
    var seg = document.createElement('div');
    seg.className = 'tali-reader-seg';
    btn = document.createElement('button');
    btn.type = 'button';
    btn.textContent = 'Off';
    btn.setAttribute('aria-pressed', 'false');
    btn.title = 'Hide chrome for distraction-free reading (press f)';
    btn.addEventListener('click', function () { setFocus(!on()); rm.close(); });
    seg.appendChild(btn);
    row.appendChild(label);
    row.appendChild(seg);
    rm.addSection('', row, sync);
  }

  // `f` toggles; Esc exits. Both are off while typing in a field or while a modal
  // ([aria-modal="true"] — the Cmd-K palette / lightbox) is open, so they never steal keys.
  document.addEventListener('keydown', function (e) {
    var t = /** @type {HTMLElement | null} */ (e.target);
    if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable)) return;
    var modal = document.querySelector('[aria-modal="true"]');
    // `f` toggles focus mode; Esc exits (a universal dismiss).
    if (e.key === 'f' && !e.metaKey && !e.ctrlKey && !e.altKey && !modal) {
      if (!taliShortcutsOn()) return;
      e.preventDefault();
      setFocus(!on());
    } else if (e.key === 'Escape' && on() && !modal) {
      setFocus(false);
    }
  });
}

