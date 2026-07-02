// Reader preferences: a reader-local text size / reading width / theme picker, mounted as
// the "Display" section of the reader menu. State lives in the reader's own localStorage and
// is applied before paint by the pre-paint head script (qmdSetTheme / qmdSetReaderPref /
// qmdResetReader in theme.rs), so this enhancer is only the UI. Read-only. Skipped on decks.
function qmdInitReaderPrefs() {
  if (window.__qmdReaderPrefs) return;
  if (!window.qmdSetReaderPref || !window.qmdReaderMenu) return; // need the pre-paint API + the menu host
  if (document.querySelector('.tali-deck')) return; // a slide deck has its own chrome
  window.__qmdReaderPrefs = true;

  var THEMES = [['light', 'Light'], ['dark', 'Dark'], ['sepia', 'Sepia']];
  var SIZES = [['0.9', 'small'], ['1', 'normal'], ['1.15', 'large'], ['1.3', 'x-large']];
  var WIDTHS = [['38rem', 'Narrow'], ['', 'Normal'], ['58rem', 'Wide']];
  var LEADINGS = [['1.5', 'Tight'], ['1.7', 'Normal'], ['2', 'Relaxed']];
  // Letter/word spacing (WCAG 1.4.12): the "Wider" step hits the WCAG minimum (letter 0.12em,
  // word 0.16em); em keeps it proportional to the reader-scaled font size.
  var LETTERS = [['0', 'Normal'], ['0.06em', 'Wide'], ['0.12em', 'Wider']];
  var WORDS = [['0', 'Normal'], ['0.08em', 'Wide'], ['0.16em', 'Wider']];
  var SIZE_FS = { '0.9': '.78rem', '1': '.95rem', '1.15': '1.15rem', '1.3': '1.4rem' };

  function curTheme() { return (window.qmdGetThemePref && window.qmdGetThemePref()) || 'light'; }
  function curSize() { return window.qmdGetReaderPref('scale') || '1'; }
  function curWidth() { return window.qmdGetReaderPref('width') || ''; }
  function curLeading() { return window.qmdGetReaderPref('leading') || '1.7'; }
  function curLetter() { return window.qmdGetReaderPref('letter') || '0'; }
  function curWord() { return window.qmdGetReaderPref('word') || '0'; }

  // One segmented control row. `labelFn(btn, opt)` customizes a button (else opt[1] text).
  function seg(title, options, getCur, onPick, labelFn) {
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
      if (labelFn) labelFn(b, opt); else b.textContent = opt[1];
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

  var body = document.createElement('div');
  var themeSeg = seg('Theme', THEMES, curTheme, function (v) { window.qmdSetTheme(v); });
  var sizeSeg = seg('Text size', SIZES, curSize,
    function (v) { window.qmdSetReaderPref('scale', v === '1' ? null : v); },
    function (b, opt) { b.textContent = 'A'; b.style.fontSize = SIZE_FS[opt[0]] || '.95rem';
      b.setAttribute('aria-label', opt[1] + ' text'); });
  var widthSeg = seg('Width', WIDTHS, curWidth,
    function (v) { window.qmdSetReaderPref('width', v || null); });
  var leadingSeg = seg('Line spacing', LEADINGS, curLeading,
    function (v) { window.qmdSetReaderPref('leading', v === '1.7' ? null : v); });
  var letterSeg = seg('Letter spacing', LETTERS, curLetter,
    function (v) { window.qmdSetReaderPref('letter', v === '0' ? null : v); });
  var wordSeg = seg('Word spacing', WORDS, curWord,
    function (v) { window.qmdSetReaderPref('word', v === '0' ? null : v); });
  body.appendChild(themeSeg.row);
  body.appendChild(sizeSeg.row);
  body.appendChild(widthSeg.row);
  body.appendChild(leadingSeg.row);
  body.appendChild(letterSeg.row);
  body.appendChild(wordSeg.row);

  var reset = document.createElement('button');
  reset.className = 'tali-reader-reset';
  reset.type = 'button';
  reset.textContent = 'Reset to defaults';
  reset.addEventListener('click', function () { if (window.qmdResetReader) window.qmdResetReader(); });
  body.appendChild(reset);

  function syncAll() { themeSeg.sync(); sizeSeg.sync(); widthSeg.sync(); leadingSeg.sync(); letterSeg.sync(); wordSeg.sync(); }
  window.addEventListener('qmd:themechange', syncAll);
  window.addEventListener('qmd:readerchange', syncAll);
  window.qmdReaderMenu.addSection('Display', body, syncAll);
}

