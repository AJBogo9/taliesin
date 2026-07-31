// Reader "show/hide code" (C-READ-1): a document-level switch over EXECUTED cell source,
// mounted as a row in the Settings menu beside Theme. The reader-owned counterpart of the
// author's per-cell `echo:` — an author decides what a document shows by default, a reader
// decides what they want to look at, and neither should have to be the other.
//
// The state and its class live in the pre-paint bootstrap (render/theme.rs:
// window.taliSetCodeHidden / taliGetCodeHidden), for the same reason the theme does: applied
// after paint, every listing would render and then vanish. This file is ONLY the UI.
// Read-only. Skipped on decks.
function taliInitCodeVisibility() {
  if (window.__taliCodeVis) return;
  // Both halves of the pre-paint API, plus the menu host. Checking only the setter would
  // leave the sync path calling an undefined getter on a page that somehow has one and not
  // the other — they ship together, so require them together.
  if (!window.taliSetCodeHidden || !window.taliGetCodeHidden || !window.taliReaderMenu) return;
  if (document.querySelector('.tali-deck')) return; // a slide deck has its own chrome
  window.__taliCodeVis = true;

  var setHidden = window.taliSetCodeHidden; // both guarded present above
  var getHidden = window.taliGetCodeHidden;

  var row = document.createElement('div');
  row.className = 'tali-reader-row';
  var label = document.createElement('span');
  label.textContent = 'Code';
  var group = document.createElement('div');
  group.className = 'tali-reader-seg';
  group.setAttribute('role', 'group');
  group.setAttribute('aria-label', 'Code');

  var OPTIONS = [
    ['show', 'Show', 'Show the source behind each computed result'],
    ['hide', 'Hide', 'Read the results only; the code stays in the document'],
  ];
  /** @type {HTMLButtonElement[]} */
  var buttons = [];
  OPTIONS.forEach(function (opt) {
    var b = document.createElement('button');
    b.type = 'button';
    b.textContent = opt[1];
    b.title = opt[2];
    b.addEventListener('click', function () { setHidden(opt[0] === 'hide'); });
    group.appendChild(b);
    buttons.push(b);
  });
  row.appendChild(label);
  row.appendChild(group);

  /** @type {{ setVisible: (v: boolean) => void } | undefined} */
  var handle;
  // Runs on every menu open (addSection's onOpen contract) as well as on every change, so
  // both halves are re-derived rather than captured once. The presence check matters in the
  // preview, where blocks are swapped in and out as you edit: a document gains its first
  // cell with no reload, and a row decided at load would keep telling that reader there is
  // no code to hide. On a static page it is simply always the same answer.
  function sync() {
    var cur = getHidden() ? 'hide' : 'show';
    buttons.forEach(function (b, i) {
      b.setAttribute('aria-pressed', OPTIONS[i][0] === cur ? 'true' : 'false');
    });
    // Offer the control only where it governs something.
    if (handle) handle.setVisible(!!document.querySelector('[data-tali-cell]'));
  }

  handle = window.taliReaderMenu.addSection('', row, sync);
  sync(); // addSection's own onOpen call ran before `handle` was assigned
  window.addEventListener('tali:codevisibility', sync);
}
