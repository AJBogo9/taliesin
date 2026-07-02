// Reading progress + resume: a thin top progress bar tied to scroll, a "N min left"
// estimate (prose only, code/math excluded), and a block-id-anchored resume position
// (reader-local, exact, survives reflow). Reader-side + read-only: derives from the live
// DOM and the reader's own localStorage; never writes the author's source. Skipped on
// decks. Idempotent (document-level, builds once).
function taliInitReadingProgress() {
  if (window.__qmdProgress) return;
  if (document.querySelector('.tali-deck')) return; // a slide deck has its own chrome
  window.__qmdProgress = true;

  // Top-level content blocks (a [data-block-id] not nested inside another block).
  function contentBlocks() {
    return [].slice.call(document.querySelectorAll('[data-block-id]')).filter(function (el) {
      return !el.parentElement || !el.parentElement.closest('[data-block-id]');
    });
  }

  // Prose word count (code + math excluded), computed once / on block-set change.
  var totalMin = 1, counted = -1;
  function countWords() {
    var blocks = contentBlocks();
    if (blocks.length === counted) return;
    counted = blocks.length;
    var words = 0;
    blocks.forEach(function (el) {
      var clone = el.cloneNode(true);
      [].slice.call(clone.querySelectorAll('pre, code, .katex')).forEach(function (n) { n.remove(); });
      var m = (clone.textContent || '').match(/[^\s]+/g);
      if (m) words += m.length;
    });
    totalMin = Math.max(1, Math.round(words / 200));
  }

  var bar = document.createElement('div');
  bar.className = 'tali-readbar';
  bar.setAttribute('aria-hidden', 'true');
  var fill = document.createElement('div');
  fill.className = 'tali-readbar-fill';
  bar.appendChild(fill);
  document.body.appendChild(bar);

  // The "N min left" readout lives in the reader menu's "Reading" section (registered at
  // the end, once the word count is known); the bar itself stays ambient at the top.
  var readout = document.createElement('div');
  readout.className = 'tali-rmenu-readout';
  function updateReadout() {
    var f = frac(), left = Math.ceil(totalMin * (1 - f));
    readout.textContent = (left > 0 ? '~' + left + ' min left' : 'Finished') + ' · ' + Math.round(f * 100) + '% read';
  }

  function frac() {
    var h = document.documentElement;
    var max = (h.scrollHeight || document.body.scrollHeight) - window.innerHeight;
    if (max <= 0) return 0;
    // window.scrollY is 0 at the top; `|| h.scrollTop` would wrongly treat 0 as falsy.
    var y = window.pageYOffset != null ? window.pageYOffset : h.scrollTop;
    return Math.min(1, Math.max(0, y / max));
  }
  var ticking = false;
  function render() {
    ticking = false;
    var f = frac();
    fill.style.width = (f * 100).toFixed(2) + '%';
  }
  function schedule() { if (!ticking) { ticking = true; requestAnimationFrame(render); } }

  // Resume position (block-id anchored), reader-local, keyed by page path.
  var KEY = 'tali-pos:' + location.pathname;
  function topBlockId() {
    var blocks = contentBlocks();
    for (var i = 0; i < blocks.length; i++) {
      if (blocks[i].getBoundingClientRect().top >= -4) return blocks[i].getAttribute('data-block-id');
    }
    return blocks.length ? blocks[blocks.length - 1].getAttribute('data-block-id') : null;
  }
  var saveTimer = null;
  function saveSoon() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(function () {
      var f = frac(), id = topBlockId();
      try {
        if (f <= 0.02 || !id) localStorage.removeItem(KEY);
        else localStorage.setItem(KEY, f.toFixed(3) + '|' + id);
      } catch (e) {}
    }, 500);
  }

  var resumeEl = null, resumeArmed = false;
  function dismissResume() { if (resumeEl) { resumeEl.remove(); resumeEl = null; } }
  function maybeShowResume() {
    var raw = null;
    try { raw = localStorage.getItem(KEY); } catch (e) {}
    if (!raw) return;
    var parts = raw.split('|'), f = parseFloat(parts[0]), id = parts[1];
    if (!(f > 0.04) || !id) return;
    var sel = '[data-block-id="' + (window.CSS && CSS.escape ? CSS.escape(id) : id) + '"]';
    var target = document.querySelector(sel);
    if (!target || Math.abs(frac() - f) < 0.03) return; // missing or already roughly there
    resumeEl = document.createElement('div');
    resumeEl.className = 'tali-resume';
    var go = document.createElement('button');
    go.type = 'button'; go.className = 'tali-resume-go';
    go.textContent = 'Resume reading · ' + Math.round(f * 100) + '% →';
    go.addEventListener('click', function () {
      target.scrollIntoView({ block: 'start', behavior: 'smooth' }); dismissResume();
    });
    var x = document.createElement('button');
    x.type = 'button'; x.className = 'tali-resume-x';
    x.setAttribute('aria-label', 'Dismiss'); x.textContent = '×';
    x.addEventListener('click', dismissResume);
    resumeEl.appendChild(go); resumeEl.appendChild(x);
    document.body.appendChild(resumeEl);
    resumeArmed = false;
    setTimeout(function () { dismissResume(); }, 8000);
  }

  function onScroll() {
    schedule();
    saveSoon();
    // Dismiss the resume pill on the reader's own scroll (not the first programmatic tick).
    if (resumeEl) { if (resumeArmed) dismissResume(); else resumeArmed = true; }
  }

  countWords();
  render();
  if (window.taliReaderMenu) window.taliReaderMenu.addSection('Reading', readout, updateReadout);
  window.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('resize', schedule, { passive: true });
  window.addEventListener('qmd:readerchange', schedule);
  maybeShowResume();
}

