// Reader menu: one launcher ("Aa", bottom-right) opening a single menu that the reader
// features mount their sections into (Reading, Display, Highlights) via
// window.qmdReaderMenu.addSection(title, node, onOpen). Consolidates what used to be three
// separate floating controls. Reader-side, read-only. Skipped on decks. Built once.
function qmdInitReaderMenu() {
  if (window.qmdReaderMenu) return;
  if (document.querySelector('.qmd-deck')) return; // a slide deck has its own chrome

  var launcher = document.createElement('button');
  launcher.type = 'button';
  launcher.className = 'qmd-rmenu-toggle';
  launcher.textContent = 'Aa';
  launcher.setAttribute('aria-label', 'Reader menu');
  launcher.setAttribute('aria-haspopup', 'dialog');
  launcher.setAttribute('aria-expanded', 'false');

  var panel = document.createElement('div');
  panel.className = 'qmd-rmenu-panel';
  panel.setAttribute('role', 'dialog');
  panel.setAttribute('aria-label', 'Reader');
  panel.hidden = true;

  document.body.appendChild(launcher);
  document.body.appendChild(panel);

  // The reader menu is a light-dismiss POPOVER, not a modal (it doesn't cover/inert the page),
  // so it deliberately does NOT use qmdFocusTrap: aria-modal would mislead a screen reader, and
  // trapping/focus-restore fights the jump buttons + outside-click dismissal. aria-expanded on
  // the launcher + Esc-to-close (returning focus to the launcher) + click-away is the right shape.
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
}

