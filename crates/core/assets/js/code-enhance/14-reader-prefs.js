// Reader theme picker (light / dark / sepia), mounted as a row in the Settings menu. The
// choice lives in the reader's own localStorage and is applied before paint by the pre-paint
// head script (taliSetTheme / taliGetThemePref in theme.rs), so this enhancer is only the UI.
// Read-only. Skipped on decks.
function taliInitReaderPrefs() {
  if (window.__qmdReaderPrefs) return;
  if (!window.taliSetTheme || !window.taliReaderMenu) return; // need the pre-paint API + the menu host
  if (document.querySelector('.tali-deck')) return; // a slide deck has its own chrome
  window.__qmdReaderPrefs = true;

  var THEMES = [['light', 'Light'], ['dark', 'Dark'], ['sepia', 'Sepia']];
  function curTheme() { return (window.taliGetThemePref && window.taliGetThemePref()) || 'light'; }

  // One segmented control row: `title` labels it, each option is [value, label].
  function seg(title, options, getCur, onPick) {
    var row = document.createElement('div');
    row.className = 'tali-reader-row';
    var label = document.createElement('span');
    label.textContent = title;
    var group = document.createElement('div');
    group.className = 'tali-reader-seg';
    group.setAttribute('role', 'group');
    group.setAttribute('aria-label', title);
    var buttons = [];
    options.forEach(function (opt) {
      var b = document.createElement('button');
      b.type = 'button';
      b.textContent = opt[1];
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

  var themeSeg = seg('Theme', THEMES, curTheme, function (v) { window.taliSetTheme(v); });
  window.addEventListener('qmd:themechange', themeSeg.sync);
  window.taliReaderMenu.addSection('', themeSeg.row, themeSeg.sync);
}
