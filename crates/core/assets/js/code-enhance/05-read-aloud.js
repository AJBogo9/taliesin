// ===== Read-aloud study mode =====================================================
// Reader-side, read-only "Listen": speaks the built page (Web Speech API) block by
// block from the block in view. Prose -> one utterance per sentence, the sentence
// highlighted (CSS Custom Highlight API) + auto-scrolled; code -> announced then its
// lines highlighted one by one (no code text spoken); figure/equation/table ->
// announced. A floating mini-player controls playback. No source write, no block-model
// change, offline (Web Speech is a browser API), deck-skipped, idempotent.
//
// The speak primitive is injectable so headless tests (no TTS voices) drive the
// playlist deterministically: override window.__qmdSpeakImpl to invoke u.onend().
window.__qmdSpeakImpl = window.__qmdSpeakImpl || function (u) { window.speechSynthesis.speak(u); };

function taliRaGet(k, d) { try { return localStorage.getItem(k) || d; } catch (e) { return d; } }
function taliRaSet(k, v) { try { if (v == null) localStorage.removeItem(k); else localStorage.setItem(k, v); } catch (e) {} }

// Top-level content blocks: a [data-block-id] not nested inside another block.
function taliRaContentBlocks() {
  return [].slice.call(document.querySelectorAll('[data-block-id]')).filter(function (el) {
    return !el.parentElement || !el.parentElement.closest('[data-block-id]');
  });
}

// A text node is non-spoken if it sits inside math/code, or inside the reader's own
// injected chrome (the `#` copy-link anchor), within the block.
function taliRaSkip(node, block) {
  var p = node.parentNode;
  while (p && p !== block) {
    if (p.nodeType === 1 && (p.tagName === 'PRE' || p.tagName === 'CODE' ||
        (p.classList && (p.classList.contains('katex') || p.classList.contains('tali-anchor'))))) return true;
    p = p.parentNode;
  }
  return false;
}

// Collect a root's text nodes (optionally skipping math/code) as one string + a map
// back to text nodes, so a global offset can be turned into a DOM position.
function taliRaTextMap(root, skipCodeMath) {
  var walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
  var full = '', spans = [], n;
  while ((n = walker.nextNode())) {
    if (skipCodeMath && taliRaSkip(n, root)) continue;
    spans.push([full.length, n]);
    full += n.nodeValue;
  }
  return { full: full, spans: spans };
}

// Map a global offset in `full` back to [textNode, localOffset].
function taliRaLocate(spans, off) {
  for (var i = spans.length - 1; i >= 0; i--) {
    if (off >= spans[i][0]) {
      var nd = spans[i][1];
      return [nd, Math.min(off - spans[i][0], nd.nodeValue.length)];
    }
  }
  return spans.length ? [spans[0][1], 0] : null;
}

// A DOM Range over [s,e) of `map.full`, or null if it can't be formed.
function taliRaRange(map, s, e) {
  var a = taliRaLocate(map.spans, s), b = taliRaLocate(map.spans, e);
  if (!a || !b) return null;
  var r = document.createRange();
  try { r.setStart(a[0], a[1]); r.setEnd(b[0], b[1]); return r; } catch (err) { return null; }
}

// Trim whitespace off a [s,e) offset window.
function taliRaTrim(full, s, e) {
  while (s < e && /\s/.test(full.charAt(s))) s++;
  while (e > s && /\s/.test(full.charAt(e - 1))) e--;
  return [s, e];
}

// Sentence boundaries in `full` as [start,end) offsets (Intl.Segmenter, regex fallback).
function taliRaSentences(full, lang) {
  var ranges = [];
  if (window.Intl && Intl.Segmenter) {
    try {
      var seg = new Intl.Segmenter(lang || undefined, { granularity: 'sentence' });
      Array.from(seg.segment(full)).forEach(function (s) {
        if (/\S/.test(s.segment)) ranges.push([s.index, s.index + s.segment.length]);
      });
      if (ranges.length) return ranges;
    } catch (e) {}
  }
  var re = /[^.!?]*[.!?]+["')\]]*\s*|[^.!?]+$/g, m;
  while ((m = re.exec(full))) {
    if (!m[0]) { re.lastIndex++; continue; }
    if (/\S/.test(m[0])) ranges.push([m.index, m.index + m[0].length]);
  }
  return ranges.length ? ranges : (full.trim() ? [[0, full.length]] : []);
}

// One Range per non-blank source line of a <code> element (preserves highlight spans).
function taliRaCodeLineRanges(code) {
  var map = taliRaTextMap(code, false), ranges = [], start = 0, full = map.full;
  for (var i = 0; i <= full.length; i++) {
    if (i === full.length || full.charAt(i) === '\n') {
      if (i > start && /\S/.test(full.slice(start, i))) {
        var r = taliRaRange(map, start, i);
        if (r) ranges.push(r);
      }
      start = i + 1;
    }
  }
  return ranges;
}

// Compile a prose element into per-sentence `say` steps (a DOM Range each).
function taliRaCompileProse(el, steps) {
  var map = taliRaTextMap(el, true);
  if (!map.full.trim()) return;
  var lang = document.documentElement.lang || undefined;
  taliRaSentences(map.full, lang).forEach(function (r) {
    var t = taliRaTrim(map.full, r[0], r[1]);
    var text = map.full.slice(t[0], t[1]);
    if (!text.trim()) return;
    var range = taliRaRange(map, t[0], t[1]);
    if (range) steps.push({ kind: 'say', text: text, range: range, el: el });
  });
}

// Compile one top-level block into ordered steps (code/figure/equation/table/prose).
function taliRaCompileBlock(block, steps) {
  var pre = block.matches('pre') ? block : block.querySelector('pre');
  var code = pre && !pre.closest('.tali-output') ? pre.querySelector('code') : null;
  if (code) {
    var ranges = taliRaCodeLineRanges(code);
    var lang = '', cls = (code.className || '').match(/language-([\w+-]+)/);
    if (cls) lang = cls[1];
    var n = ranges.length;
    var label = 'Code block. ' + n + (n === 1 ? ' line.' : ' lines.') + (lang ? ' ' + lang + '.' : '');
    steps.push({ kind: 'say', text: label, el: pre });
    if (n) steps.push({ kind: 'code', ranges: ranges, el: pre });
    return;
  }
  var fig = block.matches('figure') ? block : block.querySelector('figure');
  if (fig) {
    var cap = fig.querySelector('figcaption');
    var ftext = (cap ? taliRaTextMap(cap, true).full.replace(/ /g, ' ').trim() : '') || 'Figure';
    steps.push({ kind: 'say', text: ftext, el: fig });
    return;
  }
  if (block.querySelector('.katex-display') && !taliRaTextMap(block, true).full.trim()) {
    steps.push({ kind: 'say', text: 'Equation.', el: block });
    return;
  }
  var table = block.matches('table') ? block : block.querySelector('table');
  if (table) {
    var tcap = table.querySelector('caption');
    var ttext = (tcap ? taliRaTextMap(tcap, true).full.replace(/ /g, ' ').trim() : '') || 'Table';
    steps.push({ kind: 'say', text: ttext.replace(/\.?$/, '.'), el: table });
    return;
  }
  if (block.matches('ul, ol, dl')) {
    [].slice.call(block.children).forEach(function (li) {
      if (li.matches && li.matches('li, dd, dt')) taliRaCompileProse(li, steps);
    });
    return;
  }
  taliRaCompileProse(block, steps);
}

// The first content block at/below the viewport top (where Listen starts).
function taliRaStartBlock() {
  var blocks = taliRaContentBlocks();
  for (var i = 0; i < blocks.length; i++) {
    if (blocks[i].getBoundingClientRect().top >= -4) return blocks[i];
  }
  return blocks[0] || null;
}

// Compile the whole playlist from `startEl` to the end; tag each step with its block index.
function taliRaCompile(startEl) {
  var blocks = taliRaContentBlocks(), startIdx = 0;
  if (startEl) { var i = blocks.indexOf(startEl); if (i >= 0) startIdx = i; }
  var steps = [];
  for (var k = startIdx; k < blocks.length; k++) {
    var before = steps.length;
    taliRaCompileBlock(blocks[k], steps);
    for (var j = before; j < steps.length; j++) steps[j].block = k;
  }
  return { steps: steps, blocks: blocks };
}

// A segmented control reusing the prefs CSS (.tali-reader-row/.tali-reader-seg).
function taliRaSeg(title, options, getCur, onPick) {
  var row = document.createElement('div'); row.className = 'tali-reader-row';
  var label = document.createElement('span'); label.textContent = title;
  var group = document.createElement('div'); group.className = 'tali-reader-seg';
  group.setAttribute('role', 'group'); group.setAttribute('aria-label', title);
  var buttons = [];
  function sync() { var cur = getCur(); buttons.forEach(function (b, i) { b.setAttribute('aria-pressed', options[i][0] === cur ? 'true' : 'false'); }); }
  options.forEach(function (opt) {
    var b = document.createElement('button'); b.type = 'button'; b.textContent = opt[1];
    b.addEventListener('click', function () { onPick(opt[0]); sync(); });
    group.appendChild(b); buttons.push(b);
  });
  row.appendChild(label); row.appendChild(group); sync();
  return row;
}

// The voice picker row (OS voices); refresh() re-reads getVoices() (async on some browsers).
function taliRaVoiceRow(onPick) {
  var row = document.createElement('div'); row.className = 'tali-reader-row';
  var label = document.createElement('span'); label.textContent = 'Voice';
  var sel = document.createElement('select'); sel.className = 'tali-ra-voice-sel';
  sel.setAttribute('aria-label', 'Reading voice');
  sel.addEventListener('change', function () { onPick(sel.value); });
  function refresh() {
    var cur = taliRaGet('tali-ra-voice', '');
    var vs = (window.speechSynthesis && window.speechSynthesis.getVoices()) || [];
    sel.innerHTML = '';
    var def = document.createElement('option'); def.value = ''; def.textContent = 'Default'; sel.appendChild(def);
    vs.forEach(function (v) { var o = document.createElement('option'); o.value = v.name; o.textContent = v.name + (v.lang ? ' (' + v.lang + ')' : ''); sel.appendChild(o); });
    sel.value = cur;
  }
  row.appendChild(label); row.appendChild(sel);
  return { row: row, refresh: refresh };
}

function taliInitReadAloud() {
  if (window.__qmdReadAloud && window.__qmdReadAloud.__inited) return;
  if (document.querySelector('.tali-deck')) return;        // decks have their own chrome
  if (!window.speechSynthesis || typeof SpeechSynthesisUtterance === 'undefined') return; // no API -> no UI
  if (!window.taliReaderMenu) return;                       // need the menu host

  // --- highlight (CSS Custom Highlight API, with a <mark> fallback) ---------------
  var hl = (window.CSS && CSS.highlights && window.Highlight) ? new Highlight() : null;
  if (hl) CSS.highlights.set('tali-readaloud', hl);
  var marks = [];
  function clearMark() {
    marks.forEach(function (m) {
      var parent = m.parentNode; if (!parent) return;
      while (m.firstChild) parent.insertBefore(m.firstChild, m);
      parent.removeChild(m); parent.normalize();
    });
    marks = [];
  }
  function setHighlight(range) {
    if (hl) { hl.clear(); if (range) hl.add(range); return; }
    clearMark();
    if (range) { try { var m = document.createElement('mark'); m.className = 'tali-ra-mark'; range.surroundContents(m); marks.push(m); } catch (e) {} }
  }
  function clearHighlight() { if (hl) hl.clear(); else clearMark(); }

  function reducedMotion() { return window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches; }
  function rate() { var r = parseFloat(taliRaGet('tali-ra-rate', '1')); return r > 0 ? r : 1; }
  function currentVoice() {
    var name = taliRaGet('tali-ra-voice', '');
    if (!name) return null;
    var vs = (window.speechSynthesis.getVoices && window.speechSynthesis.getVoices()) || [];
    for (var i = 0; i < vs.length; i++) if (vs[i].name === name) return vs[i];
    return null;
  }
  function scrollTo(el) { if (el) el.scrollIntoView({ block: 'center', behavior: reducedMotion() ? 'auto' : 'smooth' }); }

  // --- driver (state machine) ------------------------------------------------------
  var state = { steps: [], idx: 0, playing: false, codeTimer: null, token: 0 };
  function stopTimers() { if (state.codeTimer) { clearTimeout(state.codeTimer); state.codeTimer = null; } }

  function focusStep(step) {
    if (step.kind === 'say') setHighlight(step.range || null); else clearHighlight();
    scrollTo(step.el);
  }
  function speakSay(step, done) {
    if (!step.text || !step.text.trim()) { done(); return; }
    var u = new SpeechSynthesisUtterance(step.text);
    u.rate = rate();
    var v = currentVoice(); if (v) u.voice = v;
    u.onend = done; u.onerror = done;
    window.__qmdSpeakImpl(u);
  }
  function runCode(step, done) {
    var ranges = step.ranges, i = 0;
    if (!ranges.length) { done(); return; }
    function tick() {
      var r = ranges[i];
      setHighlight(r || null);
      if (r && r.startContainer.parentElement) r.startContainer.parentElement.scrollIntoView({ block: 'nearest', behavior: reducedMotion() ? 'auto' : 'smooth' });
      i++;
      state.codeTimer = setTimeout(i >= ranges.length ? function () { clearHighlight(); done(); } : tick, 650 / rate());
    }
    tick();
  }
  function play() {
    state.playing = true; ui.setPlaying(true);
    var step = state.steps[state.idx];
    if (!step) { stop(); return; }
    if (step.el && !document.body.contains(step.el)) { stop(); return; } // live-swap safety
    var myToken = ++state.token;
    function done() { if (myToken !== state.token) return; advance(); }
    focusStep(step);
    if (step.kind === 'code') runCode(step, done); else speakSay(step, done);
  }
  function advance() {
    state.idx++;
    if (state.idx >= state.steps.length) { stop(); ui.announce('Finished'); return; }
    play();
  }
  function start(steps) {
    state.token++; stopTimers(); window.speechSynthesis.cancel();
    state.steps = steps; state.idx = 0;
    if (!steps.length) return;
    ui.show(); ui.announce('Playing'); play();
  }
  function pause() { state.token++; state.playing = false; ui.setPlaying(false); stopTimers(); window.speechSynthesis.cancel(); }
  function resume() { if (state.steps.length) play(); }
  function stop() { state.token++; state.playing = false; stopTimers(); window.speechSynthesis.cancel(); clearHighlight(); ui.setPlaying(false); ui.hide(); }
  function jumpBlock(dir) {
    if (!state.steps.length) return;
    var cur = state.steps[state.idx].block, ni = -1, j;
    if (dir > 0) { for (j = 0; j < state.steps.length; j++) if (state.steps[j].block > cur) { ni = j; break; } }
    else { for (j = state.steps.length - 1; j >= 0; j--) if (state.steps[j].block < cur) { ni = j; break; } }
    if (ni < 0) return;
    state.token++; stopTimers(); window.speechSynthesis.cancel();
    state.idx = ni;
    if (state.playing) play(); else focusStep(state.steps[ni]);
  }
  function applyRate() { if (state.playing) { stopTimers(); window.speechSynthesis.cancel(); play(); } }

  var driver = { start: start, pause: pause, resume: resume, stop: stop, jumpBlock: jumpBlock, applyRate: applyRate, isPlaying: function () { return state.playing; } };

  // --- mini-player UI --------------------------------------------------------------
  function btn(cls, label, txt) { var b = document.createElement('button'); b.type = 'button'; b.className = 'tali-ra-btn ' + cls; b.setAttribute('aria-label', label); b.textContent = txt; return b; }
  var bar = document.createElement('div');
  bar.className = 'tali-ra-bar'; bar.setAttribute('role', 'group'); bar.setAttribute('aria-label', 'Read aloud'); bar.hidden = true;
  var prev = btn('tali-ra-prev', 'Previous block', '⏮');
  var toggle = btn('tali-ra-toggle', 'Pause', '⏸');
  var next = btn('tali-ra-next', 'Next block', '⏭');
  var speed = document.createElement('span'); speed.className = 'tali-ra-speed';
  var stopb = btn('tali-ra-stop', 'Stop', '✕');
  var live = document.createElement('span'); live.className = 'tali-sr-only'; live.setAttribute('aria-live', 'polite');
  bar.appendChild(prev); bar.appendChild(toggle); bar.appendChild(next); bar.appendChild(speed); bar.appendChild(stopb); bar.appendChild(live);
  document.body.appendChild(bar);

  var ui = {
    show: function () { bar.hidden = false; },
    hide: function () { bar.hidden = true; },
    setPlaying: function (p) { toggle.textContent = p ? '⏸' : '▶'; toggle.setAttribute('aria-label', p ? 'Pause' : 'Play'); },
    announce: function (m) { live.textContent = m; },
    setSpeed: function (r) { speed.textContent = r + '×'; }
  };
  ui.setSpeed(taliRaGet('tali-ra-rate', '1'));

  toggle.addEventListener('click', function () {
    if (driver.isPlaying()) { driver.pause(); ui.announce('Paused'); }
    else { driver.resume(); ui.setPlaying(true); ui.announce('Playing'); }
  });
  prev.addEventListener('click', function () { driver.jumpBlock(-1); });
  next.addEventListener('click', function () { driver.jumpBlock(1); });
  stopb.addEventListener('click', function () { driver.stop(); });

  // --- reader-menu "Listen" section ------------------------------------------------
  var bodyEl = document.createElement('div');
  var listen = document.createElement('button');
  listen.type = 'button'; listen.className = 'tali-reader-reset'; listen.textContent = 'Listen';
  listen.addEventListener('click', function () {
    driver.start(taliRaCompile(taliRaStartBlock()).steps);
    window.taliReaderMenu.close();
  });
  bodyEl.appendChild(listen);

  var SPEEDS = [['0.8', '0.8×'], ['1', '1×'], ['1.25', '1.25×'], ['1.5', '1.5×'], ['2', '2×']];
  bodyEl.appendChild(taliRaSeg('Speed', SPEEDS, function () { return taliRaGet('tali-ra-rate', '1'); }, function (v) {
    taliRaSet('tali-ra-rate', v === '1' ? null : v); ui.setSpeed(v); driver.applyRate();
  }));

  var voiceRow = taliRaVoiceRow(function (name) { taliRaSet('tali-ra-voice', name || null); });
  bodyEl.appendChild(voiceRow.row);
  voiceRow.refresh();
  if (typeof window.speechSynthesis.onvoiceschanged !== 'undefined') {
    window.speechSynthesis.addEventListener('voiceschanged', voiceRow.refresh);
  }

  window.taliReaderMenu.addSection('Listen', bodyEl);

  // --- test hook -------------------------------------------------------------------
  window.__qmdReadAloud = {
    __inited: true,
    driver: driver,
    compile: function () {
      return taliRaCompile(taliRaStartBlock()).steps.map(function (s) {
        return { kind: s.kind, text: s.text || null, block: s.block, lines: s.ranges ? s.ranges.length : null };
      });
    }
  };
}

