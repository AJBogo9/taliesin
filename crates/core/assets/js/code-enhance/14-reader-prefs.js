// Reader theme picker (auto / light / dark), mounted as a row in the Settings menu. The
// choice lives in the reader's own localStorage and is applied before paint by the pre-paint
// head script (taliSetTheme / taliGetThemeChoice in theme.rs), so this enhancer is only the UI.
// Read-only. Skipped on decks.
function taliInitReaderPrefs() {
  if (window.__taliReaderPrefs) return;
  if (!window.taliSetTheme || !window.taliReaderMenu) return; // need the pre-paint API + the menu host
  if (document.querySelector('.tali-deck')) return; // a slide deck has its own chrome
  window.__taliReaderPrefs = true;

  var THEMES = [
    ['auto', 'Auto', 'Follow your system light/dark setting'],
    ['light', 'Light'], ['dark', 'Dark'],
  ];
  // Compare against the stored CHOICE, not the resolved mode: the mode is never "auto",
  // so syncing on it would leave the Auto button permanently unpressed.
  function curTheme() { return (window.taliGetThemeChoice && window.taliGetThemeChoice()) || 'auto'; }

  // One segmented control row: `title` labels it, each option is [value, label, hint?].
  /** @param {string} title @param {string[][]} options @param {() => string} getCur @param {(v: string) => void} onPick */
  function seg(title, options, getCur, onPick) {
    var row = document.createElement('div');
    row.className = 'tali-reader-row';
    var label = document.createElement('span');
    label.textContent = title;
    var group = document.createElement('div');
    group.className = 'tali-reader-seg';
    group.setAttribute('role', 'group');
    group.setAttribute('aria-label', title);
    /** @type {HTMLButtonElement[]} */
    var buttons = [];
    options.forEach(function (opt) {
      var b = document.createElement('button');
      b.type = 'button';
      b.textContent = opt[1];
      if (opt[2]) b.title = opt[2];
      b.addEventListener('click', function () { onPick(opt[0]); });
      group.appendChild(b);
      buttons.push(b);
    });
    function sync() {
      var cur = getCur();
      buttons.forEach(function (b, i) {
        b.setAttribute('aria-pressed', options[i][0] === cur ? 'true' : 'false');
      });
    }
    row.appendChild(label);
    row.appendChild(group);
    return { row: row, sync: sync };
  }

  var setTheme = window.taliSetTheme; // guarded present above; captured for the pick closure
  var themeSeg = seg('Theme', THEMES, curTheme, function (v) {
    setTheme(/** @type {'auto' | 'light' | 'dark'} */ (v));
  });
  window.addEventListener('tali:themechange', themeSeg.sync);
  window.taliReaderMenu.addSection(themeSeg.row, themeSeg.sync);
}
