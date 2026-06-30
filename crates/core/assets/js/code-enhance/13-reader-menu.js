// Reader menu: one launcher ("Aa", bottom-right) opening a single menu that the reader
// features mount their sections into (Reading, Display, Highlights) via
// window.qmdReaderMenu.addSection(title, node, onOpen). Consolidates what used to be three
// separate floating controls. Reader-side, read-only. Skipped on decks. Built once.
function qmdInitReaderMenu() {
  if (window.qmdReaderMenu) return;
  if (document.querySelector('.qmd-deck')) return; // a slide deck has its own chrome

  // A DISCLOSURE, not a dialog: the launcher's aria-expanded + aria-controls point at a labelled
  // group, which is the correct ARIA shape for a light-dismiss popover that does NOT trap or move
  // focus. (role="dialog"/aria-haspopup="dialog" would promise a modal with managed focus we
  // deliberately don't provide — see the openMenu/closeMenu note below.)
  var panelId = 'qmd-rmenu-panel';
  var launcher = document.createElement('button');
  launcher.type = 'button';
  launcher.className = 'qmd-rmenu-toggle';
  launcher.textContent = 'Aa';
  launcher.setAttribute('aria-label', 'Reader menu');
  launcher.setAttribute('aria-controls', panelId);
  launcher.setAttribute('aria-expanded', 'false');

  var panel = document.createElement('div');
  panel.className = 'qmd-rmenu-panel';
  panel.id = panelId;
  panel.setAttribute('role', 'group');
  panel.setAttribute('aria-label', 'Reader settings');
  panel.hidden = true;

  document.body.appendChild(launcher);
  document.body.appendChild(panel);

  // The reader menu is a light-dismiss POPOVER, not a modal (it doesn't cover/inert the page),
  // so it deliberately does NOT use qmdFocusTrap and is exposed as a disclosure (above): trapping
  // /focus-restore would fight the jump buttons + outside-click dismissal. aria-expanded on the
  // launcher + Esc-to-close (returning focus to the launcher) + click-away is the right shape.
  var sections = [];
  function openMenu() {
    panel.hidden = false; launcher.setAttribute('aria-expanded', 'true');
    sections.forEach(function (s) { if (s.onOpen) s.onOpen(); });
  }
  function closeMenu() { panel.hidden = true; launcher.setAttribute('aria-expanded', 'false'); }
  launcher.addEventListener('click', function (e) { e.stopPropagation(); if (panel.hidden) openMenu(); else closeMenu(); });
  document.addEventListener('click', function (e) {
    if (!panel.hidden && !panel.contains(e.target) && e.target !== launcher) closeMenu();
  });
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape' && !panel.hidden) { closeMenu(); launcher.focus(); } });

  // Public API: each reader feature adds its own section and an optional refresh hook
  // (called when the menu opens). Returns a handle to show/hide the section.
  window.qmdReaderMenu = {
    close: closeMenu,
    addSection: function (title, node, onOpen) {
      var wrap = document.createElement('section');
      wrap.className = 'qmd-rmenu-section';
      if (title) { var h = document.createElement('h2'); h.textContent = title; wrap.appendChild(h); }
      wrap.appendChild(node);
      panel.appendChild(wrap);
      sections.push({ wrap: wrap, onOpen: onOpen });
      if (onOpen) onOpen();
      return { setVisible: function (v) { wrap.hidden = !v; } };
    }
  };

  // WCAG 2.1.4 opt-out, surfaced in the menu so it's discoverable: a toggle that turns the
  // single-key shortcuts (f / ? / / / arrows) on or off via the shared `qmd-keyshortcuts`
  // localStorage flag that 03-focus-mode.js + 07-keyboard.js read (__qmdShortcutsOn).
  (function () {
    var row = document.createElement('div');
    row.className = 'qmd-reader-row';
    var label = document.createElement('span');
    label.textContent = 'Keyboard shortcuts';
    var seg = document.createElement('div');
    seg.className = 'qmd-reader-seg';
    var ksBtn = document.createElement('button');
    ksBtn.type = 'button';
    ksBtn.title = 'Single-key shortcuts: f focus, ? help, / search, ←/→ chapters';
    function ksOn() { return typeof __qmdShortcutsOn === 'function' ? __qmdShortcutsOn() : true; }
    function ksSync() {
      var v = ksOn();
      ksBtn.setAttribute('aria-pressed', v ? 'true' : 'false');
      ksBtn.textContent = v ? 'On' : 'Off';
    }
    ksBtn.addEventListener('click', function () {
      try { localStorage.setItem('qmd-keyshortcuts', ksOn() ? 'off' : 'on'); } catch (e) {}
      ksSync();
    });
    ksSync();
    seg.appendChild(ksBtn);
    row.appendChild(label);
    row.appendChild(seg);
    window.qmdReaderMenu.addSection('Keyboard', row, ksSync);
  })();
}

