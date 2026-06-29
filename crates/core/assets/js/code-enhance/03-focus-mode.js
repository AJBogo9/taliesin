// Focus / reading mode: hide site chrome and centre the prose into one calm column for
// distraction-free reading. Reader-side, ephemeral (no localStorage) — toggled by the `f`
// key (ignored while typing or while a modal is open), Esc, or a Reader-menu toggle. All
// the hiding/centring is CSS on body.qmd-focus; this just flips the class + wires triggers.
function qmdInitFocusMode() {
  if (document.querySelector('.qmd-deck')) return;
  if (window.__qmdFocus) return;
  window.__qmdFocus = true;

  var live = document.createElement('span');
  live.className = 'qmd-sr-only';
  live.setAttribute('aria-live', 'polite');
  document.body.appendChild(live);

  var btn = null;
  function on() { return document.body.classList.contains('qmd-focus'); }
  function sync() {
    if (!btn) return;
    btn.setAttribute('aria-pressed', on() ? 'true' : 'false');
    btn.textContent = on() ? 'On' : 'Off';
  }
  function setFocus(v) {
    document.body.classList.toggle('qmd-focus', v);
    sync();
    live.textContent = '';
    live.textContent = v ? 'Focus mode on' : 'Focus mode off';
  }

  // Reader-menu toggle (discoverable). The launcher stays visible in focus mode, so this
  // remains the mouse exit + the size/theme controls.
  if (window.qmdReaderMenu) {
    var row = document.createElement('div');
    row.className = 'qmd-reader-row';
    var label = document.createElement('span');
    label.textContent = 'Focus mode';
    var seg = document.createElement('div');
    seg.className = 'qmd-reader-seg';
    btn = document.createElement('button');
    btn.type = 'button';
    btn.textContent = 'Off';
    btn.setAttribute('aria-pressed', 'false');
    btn.title = 'Hide chrome for distraction-free reading (press f)';
    btn.addEventListener('click', function () { setFocus(!on()); window.qmdReaderMenu.close(); });
    seg.appendChild(btn);
    row.appendChild(label);
    row.appendChild(seg);
    window.qmdReaderMenu.addSection('Focus', row, sync);
  }

  // `f` toggles; Esc exits. Both are off while typing in a field or while a modal
  // ([aria-modal="true"] — the Cmd-K palette / lightbox) is open, so they never steal keys.
  document.addEventListener('keydown', function (e) {
    var t = e.target;
    if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
    var modal = document.querySelector('[aria-modal="true"]');
    if (e.key === 'f' && !e.metaKey && !e.ctrlKey && !e.altKey && !modal) {
      e.preventDefault();
      setFocus(!on());
    } else if (e.key === 'Escape' && on() && !modal) {
      setFocus(false);
    }
  });
}

