// Settings menu: a gear launcher opening a single popover that the reader features mount their
// sections into (Theme, Focus, Keyboard shortcuts) via
// window.taliReaderMenu.addSection(title, node, onOpen). On sites the gear is docked in the
// navbar / book topbar (server-rendered, [data-tali-settings]); on a chrome-less single doc we
// create a floating one. Click handling is delegated on document so a hot-reload that re-injects
// the navbar keeps working without re-running this (guarded) initializer. Reader-side, read-only.
// Skipped on decks. Built once. (Internal names keep the `taliReaderMenu` / `.tali-rmenu-*` spelling.)
function taliInitReaderMenu() {
  if (window.taliReaderMenu) return;
  if (document.querySelector('.tali-deck')) return; // a slide deck has its own chrome

  var panelId = 'tali-rmenu-panel';
  // Same gear as chrome.rs SETTINGS_ICON (kept in sync); only used for the floating fallback.
  var GEAR =
    '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" ' +
    'stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
    '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1' +
    '-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 ' +
    '1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 ' +
    '.33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33' +
    '-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 ' +
    '2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l' +
    '-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 ' +
    '0-1.51 1z"/></svg>';

  // No docked gear (single doc, no chrome) → create a floating one, tagged the same way so the
  // delegated click handler treats both identically.
  if (!document.querySelector('[data-tali-settings]')) {
    var floatBtn = document.createElement('button');
    floatBtn.type = 'button';
    floatBtn.className = 'tali-rmenu-toggle';
    floatBtn.setAttribute('data-tali-settings', '');
    floatBtn.setAttribute('aria-label', 'Settings');
    floatBtn.innerHTML = GEAR;
    document.body.appendChild(floatBtn);
  }

  // A DISCLOSURE, not a dialog: the launcher's aria-expanded + aria-controls point at a labelled
  // group, the correct ARIA shape for a light-dismiss popover. It does not TRAP focus (see below)
  // but it does MOVE focus in on open: the disclosure "leave focus where it is" rule assumes the
  // panel follows its trigger in DOM order so you can Tab straight into it. This panel is appended
  // to <body> while the gear lives in the navbar, so without the move, opening the menu from the
  // keyboard strands you a whole page away from what you just opened. Esc restores focus (below).
  var panel = document.createElement('div');
  panel.className = 'tali-rmenu-panel';
  panel.id = panelId;
  panel.setAttribute('role', 'group');
  panel.setAttribute('aria-label', 'Settings');
  panel.hidden = true;
  document.body.appendChild(panel);

  function launchers() { return document.querySelectorAll('[data-tali-settings]'); }
  function setExpanded(v) {
    [].forEach.call(launchers(), function (b) {
      b.setAttribute('aria-controls', panelId);
      b.setAttribute('aria-expanded', v ? 'true' : 'false');
    });
  }

  // Light-dismiss POPOVER, not a modal (it doesn't cover/inert the page): no taliFocusTrap, so
  // trapping/focus-restore can't fight the outside-click dismissal, and aria-modal would suppress
  // the reader shortcuts, which treat [aria-modal="true"] as "a modal owns the keys". Moving focus
  // once on open is not trapping and does not fight dismissal. aria-expanded + Esc-to-close
  // (returning focus to the launcher) + click-away is the right shape.
  var sections = [];
  function openMenu() {
    panel.hidden = false; setExpanded(true);
    sections.forEach(function (s) { if (s.onOpen) s.onOpen(); });
    // Focus AFTER unhiding and AFTER the onOpen hooks: taliFocusables filters on
    // offsetWidth/Height, so a still-hidden panel yields nothing, and a hook may have just
    // shown or hidden its own controls (07-keyboard's shortcut list does exactly that).
    var first = taliFocusables(panel)[0];
    if (first) { try { first.focus(); } catch (e) {} }
  }
  function closeMenu() { panel.hidden = true; setExpanded(false); }
  function toggleMenu() { if (panel.hidden) openMenu(); else closeMenu(); }
  setExpanded(false);

  // One delegated click handler: a click on any launcher toggles; an outside click dismisses.
  // Delegation (vs. a direct listener) survives a navbar re-injection on hot reload.
  document.addEventListener('click', function (e) {
    var launch = e.target.closest && e.target.closest('[data-tali-settings]');
    if (launch) { toggleMenu(); return; }
    if (!panel.hidden && !panel.contains(e.target)) closeMenu();
  });
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape' && !panel.hidden) {
      closeMenu();
      var l = document.querySelector('[data-tali-settings]');
      if (l) l.focus();
    }
  });

  // Public API: each reader feature adds its own section and an optional refresh hook
  // (called when the menu opens). `open`/`toggle` let the `?` shortcut summon it.
  // Returns a handle to show/hide the section.
  window.taliReaderMenu = {
    open: openMenu,
    close: closeMenu,
    toggle: toggleMenu,
    addSection: function (title, node, onOpen) {
      var wrap = document.createElement('section');
      wrap.className = 'tali-rmenu-section';
      if (title) { var h = document.createElement('h2'); h.textContent = title; wrap.appendChild(h); }
      wrap.appendChild(node);
      panel.appendChild(wrap);
      sections.push({ wrap: wrap, onOpen: onOpen });
      if (onOpen) onOpen();
      return { setVisible: function (v) { wrap.hidden = !v; } };
    }
  };

}
