// qmd-fast deck engine — the navigation + scaling reveal.js used to provide,
// owned by the project so block-level incremental updates and click-to-source
// work in decks the same way they do on a page. It drives reveal's own DOM
// contract (.reveal > .slides > section, .present/.past/.future, nested
// <section> stacks) and exposes a window.Reveal-shaped facade so existing
// reveal extensions (liquid-glass) and the preview client keep working
// unmodified. Internally it is window.QmdDeck.
(function () {
  var deck = {
    config: {
      width: 960, height: 540, margin: 0.04, // 16:9 default
      center: false, hash: true, history: false, slideNumber: false,
    },
    h: 0, v: 0, frag: 0,
    ready: false,
    overview: false,
    plugins: [],
    listeners: {},
  };

  function slidesEl() { return document.querySelector('.reveal .slides'); }
  function revealEl() { return document.querySelector('.reveal'); }

  // Top-level horizontal sections (a stack wrapper counts as one).
  function tops() {
    var s = slidesEl();
    return s ? Array.prototype.filter.call(s.children, isSection) : [];
  }
  function isSection(n) { return n.tagName === 'SECTION'; }
  // The vertical slides of a top: a stack's children, else the top itself.
  function vertsOf(top) {
    var kids = Array.prototype.filter.call(top.children, isSection);
    return kids.length ? kids : [top];
  }
  function isStack(top) { return vertsOf(top)[0] !== top; }

  function currentSlide() {
    var T = tops();
    if (!T.length) return null;
    var top = T[deck.h];
    if (!top) return null;
    return isStack(top) ? vertsOf(top)[deck.v] : top;
  }

  // Flat list of leaf slides (what reveal's getSlides returns), for plugins
  // and the slide-number total.
  function allSlides() {
    var out = [];
    tops().forEach(function (top) {
      if (isStack(top)) vertsOf(top).forEach(function (s) { out.push(s); });
      else out.push(top);
    });
    return out;
  }

  function clampIndices() {
    var T = tops();
    if (!T.length) { deck.h = 0; deck.v = 0; return; }
    deck.h = Math.max(0, Math.min(deck.h, T.length - 1));
    var V = vertsOf(T[deck.h]);
    deck.v = Math.max(0, Math.min(deck.v, V.length - 1));
  }

  // --- scale-to-fit -------------------------------------------------------
  function layout() {
    var s = slidesEl();
    if (!s) return;
    var w = deck.config.width, h = deck.config.height, m = deck.config.margin;
    var scale = Math.min(window.innerWidth / w, window.innerHeight / h) * (1 - 2 * m);
    if (!(scale > 0)) scale = 1;
    document.documentElement.style.setProperty('--qmd-deck-scale', String(scale));
  }

  // --- show the current slide --------------------------------------------
  // Visibility is driven by an inline `display: none !important` on hidden
  // slides (not just a CSS class), so it beats theme rules that force a display
  // on every section — e.g. liquid-glass's `section { display: flex !important }`.
  // The present slide has its inline display removed so the theme's own layout
  // (or deck.css's `.present` rule) decides how it renders. The past/present/
  // future classes are kept for CSS transitions.
  function apply() {
    var T = tops();
    var curTop = T[deck.h];
    T.forEach(function (top, i) {
      setClass(top, i < deck.h ? 'past' : (i > deck.h ? 'future' : 'present'));
      setVisible(top, top === curTop);
      if (isStack(top)) {
        vertsOf(top).forEach(function (sec, j) {
          setClass(sec, j < deck.v ? 'past' : (j > deck.v ? 'future' : 'present'));
          setVisible(sec, top === curTop && j === deck.v);
        });
      }
    });
    fitSlide(currentSlide());
    applyFragments();
  }
  // --- fragments (incremental reveals) -----------------------------------
  // A fragment is any `.fragment` element or a list item inside `.incremental`,
  // in document order. They start hidden (via visibility, so layout + shrink-to-
  // fit are unaffected) and reveal one per forward step before the slide advances.
  // A slide's ordered "steps": each `.fragment`/`.incremental` item is a reveal
  // step; a `pre[data-code-lines]` with K `|`-separated segments contributes K-1
  // steps (segment 0 is the slide's base highlight, applied before any step).
  var FRAG_SEL = '.fragment, .incremental > ul > li, .incremental > ol > li';
  function fragsOf(slide) {
    if (!slide) return [];
    var steps = [];
    slide.querySelectorAll(FRAG_SEL + ', pre[data-code-lines]').forEach(function (node) {
      if (node.tagName === 'PRE') {
        var segs = node.getAttribute('data-code-lines').split('|');
        for (var i = 1; i < segs.length; i++) steps.push({ code: node, seg: segs[i] });
      } else {
        steps.push({ frag: node });
      }
    });
    return steps;
  }
  function fragCount() { return fragsOf(currentSlide()).length; }
  function applyFragments() {
    var slide = currentSlide();
    if (!slide) return;
    var steps = fragsOf(slide);
    if (deck.frag > steps.length) deck.frag = steps.length;
    // base state: every code block to its segment 0, every fragment hidden
    slide.querySelectorAll('pre[data-code-lines]').forEach(function (pre) {
      highlightLines(pre, pre.getAttribute('data-code-lines').split('|')[0]);
    });
    slide.querySelectorAll(FRAG_SEL).forEach(function (el) { el.classList.remove('qmd-frag-visible'); });
    // then apply each taken step in order (later code steps overwrite earlier)
    for (var i = 0; i < deck.frag; i++) {
      var s = steps[i];
      if (s.frag) s.frag.classList.add('qmd-frag-visible');
      else highlightLines(s.code, s.seg);
    }
  }
  // Highlight the lines named by `spec` ("3-5", "1,4", "all", "") in a code block,
  // dimming the rest. "all"/empty clears the dim.
  function highlightLines(pre, spec) {
    var lines = pre.querySelectorAll('.qhl-ln');
    spec = (spec || '').trim();
    if (!spec || spec === 'all') {
      pre.classList.remove('qhl-lines-active');
      lines.forEach(function (l) { l.classList.remove('qhl-ln-hl'); });
      return;
    }
    var on = parseLineSpec(spec);
    pre.classList.add('qhl-lines-active');
    lines.forEach(function (l, i) { l.classList.toggle('qhl-ln-hl', on.has(i + 1)); });
  }
  function parseLineSpec(spec) {
    var on = new Set();
    spec.split(',').forEach(function (part) {
      var m = part.trim().match(/^(\d+)\s*-\s*(\d+)$/);
      if (m) { for (var n = +m[1]; n <= +m[2]; n++) on.add(n); }
      else if (/^\d+$/.test(part.trim())) on.add(+part.trim());
    });
    return on;
  }
  // A fragment step doesn't go through commit(), so it must render the right view
  // for the current mode AND broadcast, so the other window (audience or speaker
  // preview) follows the reveal, not just slide changes.
  function fragChanged() {
    if (deck.mode === 'speaker') updateSpeakerUI();
    else applyFragments();
    broadcastState();
  }
  function revealNextFrag() {
    if (deck.frag < fragCount()) { deck.frag++; fragChanged(); fire('fragmentshown'); return true; }
    return false;
  }
  function hidePrevFrag() {
    if (deck.frag > 0) { deck.frag--; fragChanged(); fire('fragmenthidden'); return true; }
    return false;
  }
  // Shrink-to-fit: if the current slide's content overflows the design box, scale
  // its font-size down so it fits (everything is em-based, incl. padding, so it
  // scales uniformly). Depends only on content vs box, not viewport, so it needn't
  // re-run on resize. Cleared in overview. The base BASE px is the deck font-size.
  var BASE = 40;
  function fitSlide(sec) {
    if (!sec || deck.overview) return;
    sec.style.removeProperty('font-size'); // measure at natural size
    var fh = sec.scrollHeight > sec.clientHeight ? sec.clientHeight / sec.scrollHeight : 1;
    var fw = sec.scrollWidth > sec.clientWidth ? sec.clientWidth / sec.scrollWidth : 1;
    var f = Math.min(fh, fw);
    if (f < 1) sec.style.fontSize = (BASE * f * 0.97).toFixed(2) + 'px';
  }
  function setClass(el, state) {
    el.classList.remove('past', 'present', 'future');
    el.classList.add(state);
  }
  function setVisible(el, visible) {
    if (visible) { el.style.removeProperty('display'); el.removeAttribute('aria-hidden'); }
    else { el.style.setProperty('display', 'none', 'important'); el.setAttribute('aria-hidden', 'true'); }
  }

  // --- navigation ---------------------------------------------------------
  function commit() {
    clampIndices();
    if (deck.mode === 'speaker') updateSpeakerUI();
    else { apply(); layout(); updateNumber(); writeHash(); focusCurrent(); }
    fire('slidechanged');
    broadcastState();
  }
  // Move focus to the slide that just became current so keyboard + screen-reader
  // users follow the navigation. Not called from sync(), which must never yank
  // focus during a live edit. The input guard in onKey means this can't fire
  // while the user is typing in a field.
  function focusCurrent() {
    if (deck.overview) return;
    var c = currentSlide();
    if (!c) return;
    if (!c.hasAttribute('tabindex')) c.setAttribute('tabindex', '-1');
    try { c.focus({ preventScroll: true }); } catch (e) {}
  }
  // Move to a slide. `revealAll` shows all its fragments (a backward step or a
  // jump lands on a complete slide); otherwise they start hidden (forward entry).
  function moveTo(h, v, revealAll) {
    deck.h = h; deck.v = v;
    clampIndices();
    deck.frag = revealAll ? fragCount() : 0;
    commit();
  }
  // Forward steps reveal the next fragment first, then advance; backward steps
  // hide the last fragment first, then retreat (landing fully revealed).
  function right() {
    if (revealNextFrag()) return;
    if (deck.h < tops().length - 1) moveTo(deck.h + 1, 0, false);
  }
  function left() {
    if (hidePrevFrag()) return;
    if (deck.h > 0) moveTo(deck.h - 1, 0, true);
  }
  function down() {
    if (revealNextFrag()) return;
    var top = tops()[deck.h];
    if (top && isStack(top) && deck.v < vertsOf(top).length - 1) moveTo(deck.h, deck.v + 1, false);
  }
  function up() {
    if (hidePrevFrag()) return;
    var top = tops()[deck.h];
    if (top && isStack(top) && deck.v > 0) moveTo(deck.h, deck.v - 1, true);
  }
  // Linear next/prev: fragments first, then flow down a stack, then across.
  function next() {
    if (revealNextFrag()) return;
    var T = tops(), top = T[deck.h];
    if (top && isStack(top) && deck.v < vertsOf(top).length - 1) moveTo(deck.h, deck.v + 1, false);
    else if (deck.h < T.length - 1) moveTo(deck.h + 1, 0, false);
  }
  function prev() {
    if (hidePrevFrag()) return;
    var T = tops(), top = T[deck.h];
    if (top && isStack(top) && deck.v > 0) moveTo(deck.h, deck.v - 1, true);
    else if (deck.h > 0) {
      var nh = deck.h - 1, pt = T[nh];
      moveTo(nh, isStack(pt) ? vertsOf(pt).length - 1 : 0, true);
    }
  }

  // --- overview (grid of all slides) -------------------------------------
  function setOverview(on) {
    if (on === deck.overview) return;
    var rev = revealEl();
    if (!rev) return;
    deck.overview = on;
    var T = tops();
    if (on) {
      rev.classList.add('overview');
      var cur = T[deck.h];
      T.forEach(function (top) {
        top.style.removeProperty('display');
        top.style.removeProperty('font-size'); // tiles render at natural size, not shrunk
        top.removeAttribute('aria-hidden');
        top.classList.toggle('qmd-overview-current', top === cur);
        // a stack tile shows only its lead sub-slide
        if (isStack(top)) vertsOf(top).forEach(function (sec, j) {
          sec.style.removeProperty('font-size');
          if (j === 0) sec.style.removeProperty('display');
          else sec.style.setProperty('display', 'none', 'important');
        });
      });
      var c = T[deck.h];
      if (c && c.scrollIntoView) c.scrollIntoView({ block: 'nearest' });
    } else {
      rev.classList.remove('overview');
      T.forEach(function (t) { t.classList.remove('qmd-overview-current'); });
      apply();
      layout();
    }
  }
  function moveHighlight(delta) {
    var T = tops();
    deck.h = Math.max(0, Math.min(deck.h + delta, T.length - 1));
    deck.v = 0;
    T.forEach(function (t, i) { t.classList.toggle('qmd-overview-current', i === deck.h); });
    var c = T[deck.h];
    if (c && c.scrollIntoView) c.scrollIntoView({ block: 'nearest' });
  }
  function onSlidesClick(e) {
    if (!deck.overview) return;
    var sec = e.target.closest && e.target.closest('.reveal .slides > section');
    if (!sec) return;
    e.preventDefault();
    var idx = tops().indexOf(sec);
    setOverview(false);
    if (idx >= 0) moveTo(idx, 0, true);
    else commit();
  }

  // --- presenter mode + cross-window sync --------------------------------
  // `s` opens a speaker window (a popup at ?qmd=speaker). It shows the current +
  // next slide as live previews (same-origin iframes at ?qmd=embed), the slide's
  // speaker notes (`::: {.notes}`), and a timer + clock. Audience and speaker stay
  // in sync via opener<->popup postMessage (works on file://); either can drive.
  function withQmd(url, val) { return url + (url.indexOf('?') >= 0 ? '&' : '?') + 'qmd=' + val; }
  function deckBaseUrl() { return location.href.split('#')[0].split('?')[0]; }

  // Apply a position received from the other window (or, in an embed iframe, from
  // the speaker). Never re-broadcasts, so there is no echo loop.
  function applyRemote(h, v, frag) {
    deck.h = h; deck.v = v;
    clampIndices();
    deck.frag = (frag == null) ? fragCount() : frag;
    if (deck.mode === 'speaker') updateSpeakerUI();
    else { apply(); layout(); updateNumber(); writeHash(); }
    fire('slidechanged');
  }
  function broadcastState() {
    var msg = { qmd: 'deck', type: 'state', h: deck.h, v: deck.v, frag: deck.frag };
    if (deck.speakerWin && !deck.speakerWin.closed) { try { deck.speakerWin.postMessage(msg, '*'); } catch (e) {} }
    if (window.opener && !window.opener.closed) { try { window.opener.postMessage(msg, '*'); } catch (e) {} }
  }
  function onMessage(e) {
    var d = e.data;
    if (!d || d.qmd !== 'deck') return;
    if (d.type === 'goto' || d.type === 'state') applyRemote(d.h, d.v, d.frag);
    else if (d.type === 'hello') broadcastState(); // a freshly-opened speaker asks for our position
  }
  function openSpeaker() {
    if (deck.mode !== 'normal') return;
    if (deck.speakerWin && !deck.speakerWin.closed) { deck.speakerWin.focus(); return; }
    deck.speakerWin = window.open(withQmd(deckBaseUrl(), 'speaker'), 'qmd-speaker', 'width=1180,height=760');
  }
  function nextIndex(h, v) {
    var T = tops(), top = T[h];
    if (top && isStack(top) && v < vertsOf(top).length - 1) return { h: h, v: v + 1 };
    if (h < T.length - 1) return { h: h + 1, v: 0 };
    return null;
  }
  function postFrame(frame, h, v, frag) {
    if (frame && frame.contentWindow) {
      try { frame.contentWindow.postMessage({ qmd: 'deck', type: 'goto', h: h, v: v, frag: frag }, '*'); } catch (e) {}
    }
  }
  function updateSpeakerUI() {
    postFrame(deck.spCur, deck.h, deck.v, deck.frag);
    var nx = nextIndex(deck.h, deck.v);
    if (nx) { postFrame(deck.spNext, nx.h, nx.v, null); if (deck.spNextPane) deck.spNextPane.style.visibility = ''; }
    else if (deck.spNextPane) deck.spNextPane.style.visibility = 'hidden';
    var c = currentSlide();
    var notes = c && c.querySelector('.notes');
    if (deck.spNotesBody) deck.spNotesBody.innerHTML = notes ? notes.innerHTML : '<span class="sp-empty">No notes for this slide.</span>';
  }
  function updateSpeakerClock() {
    var t = document.querySelector('.qmd-speaker .sp-timer');
    var c = document.querySelector('.qmd-speaker .sp-clock');
    if (t) { var s = Math.max(0, Math.floor((Date.now() - deck.spStart) / 1000)); t.textContent = Math.floor(s / 60) + ':' + ('0' + (s % 60)).slice(-2); }
    if (c) c.textContent = new Date().toLocaleTimeString();
  }
  function initSpeaker() {
    document.title = 'Speaker · ' + document.title;
    var rev = revealEl(); if (rev) rev.style.display = 'none'; // keep as data source for notes/counts
    var root = document.createElement('div');
    root.className = 'qmd-speaker';
    root.innerHTML =
      '<div class="sp-top"><div class="sp-timer">0:00</div><button class="sp-reset">Reset</button><div class="sp-clock"></div></div>' +
      '<div class="sp-stage">' +
        '<div class="sp-pane"><div class="sp-label">Current</div><iframe class="sp-frame-cur"></iframe></div>' +
        '<div class="sp-pane sp-pane-next"><div class="sp-label">Next</div><iframe class="sp-frame-next"></iframe></div>' +
      '</div>' +
      '<div class="sp-notes"><div class="sp-label">Notes</div><div class="sp-notes-body"></div></div>';
    document.body.appendChild(root);
    deck.spCur = root.querySelector('.sp-frame-cur');
    deck.spNext = root.querySelector('.sp-frame-next');
    deck.spNextPane = root.querySelector('.sp-pane-next');
    deck.spNotesBody = root.querySelector('.sp-notes-body');
    var loaded = 0, ready = function () { if (++loaded >= 2) updateSpeakerUI(); };
    deck.spCur.onload = ready; deck.spNext.onload = ready;
    var embed = withQmd(deckBaseUrl(), 'embed');
    deck.spCur.src = embed; deck.spNext.src = embed;
    document.addEventListener('keydown', onKey);
    window.addEventListener('message', onMessage);
    deck.spStart = Date.now();
    root.querySelector('.sp-reset').addEventListener('click', function () { deck.spStart = Date.now(); updateSpeakerClock(); });
    setInterval(updateSpeakerClock, 500);
    updateSpeakerClock();
    clampIndices();
    if (window.opener) { try { window.opener.postMessage({ qmd: 'deck', type: 'hello' }, '*'); } catch (e) {} }
  }

  // --- URL hash (replaceState by default: no history pollution) ----------
  function writeHash() {
    if (!deck.config.hash) return;
    var c = currentSlide();
    var frag = c && c.id ? c.id : deck.h + (deck.v ? '/' + deck.v : '');
    var url = '#/' + frag;
    if (url === location.hash) return;
    if (deck.config.history) location.hash = '/' + frag;
    else history.replaceState(null, '', url);
  }
  function readHash() {
    var raw = location.hash.replace(/^#\/?/, '');
    if (!raw) return false;
    var parts = raw.split('/');
    if (parts[0] && isNaN(parseInt(parts[0], 10))) {
      var el = document.getElementById(parts[0]);
      if (!el) return false;
      var ix = indexOf(el);
      deck.h = ix.h; deck.v = ix.v;
      return true;
    }
    deck.h = parseInt(parts[0], 10) || 0;
    deck.v = parseInt(parts[1], 10) || 0;
    return true;
  }
  function indexOf(el) {
    var T = tops();
    for (var i = 0; i < T.length; i++) {
      if (T[i] === el) return { h: i, v: 0 };
      if (isStack(T[i])) {
        var V = vertsOf(T[i]);
        for (var j = 0; j < V.length; j++) if (V[j] === el) return { h: i, v: j };
      }
    }
    return { h: 0, v: 0 };
  }
  function onHashChange() {
    if (readHash()) { clampIndices(); deck.frag = fragCount(); apply(); layout(); updateNumber(); fire('slidechanged'); }
  }

  // --- slide number -------------------------------------------------------
  function updateNumber() {
    if (!deck.config.slideNumber) return;
    var rev = revealEl();
    if (!rev) return;
    var el = rev.querySelector('.qmd-slide-number');
    if (!el) { el = document.createElement('div'); el.className = 'qmd-slide-number'; rev.appendChild(el); }
    var all = allSlides();
    el.textContent = (all.indexOf(currentSlide()) + 1) + ' / ' + all.length;
  }

  // --- keyboard + touch ---------------------------------------------------
  function onKey(e) {
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return;
    var t = /** @type {any} */ (e.target);
    if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
    var handled = true;
    if (deck.overview) {
      switch (e.key) {
        case 'Escape': case 'Enter': case ' ': setOverview(false); moveTo(deck.h, deck.v, true); break;
        case 'ArrowRight': case 'ArrowDown': moveHighlight(1); break;
        case 'ArrowLeft': case 'ArrowUp': moveHighlight(-1); break;
        default: handled = false;
      }
      if (handled) e.preventDefault();
      return;
    }
    switch (e.key) {
      case ' ': case 'PageDown': e.shiftKey ? prev() : next(); break;
      case 'PageUp': prev(); break;
      case 'ArrowRight': right(); break;
      case 'ArrowLeft': left(); break;
      case 'ArrowDown': down(); break;
      case 'ArrowUp': up(); break;
      case 'Home': moveTo(0, 0, false); break;
      case 'End': moveTo(tops().length - 1, 0, true); break;
      case 'Escape': case 'o': if (deck.mode === 'normal') setOverview(true); break;
      case 's': openSpeaker(); break;
      default: handled = false;
    }
    if (handled) e.preventDefault();
  }
  var touch = { x: null, y: null, t: 0 };
  function onTouchStart(e) {
    if (e.touches.length !== 1) { touch.x = null; return; }
    touch.x = e.touches[0].clientX; touch.y = e.touches[0].clientY; touch.t = Date.now();
  }
  function onTouchEnd(e) {
    if (touch.x == null) return;
    var c = e.changedTouches[0];
    var dx = c.clientX - touch.x, dy = c.clientY - touch.y, dt = Date.now() - touch.t;
    touch.x = null;
    if (dt > 600 || Math.max(Math.abs(dx), Math.abs(dy)) < 50) return;
    if (Math.abs(dx) > Math.abs(dy)) { dx < 0 ? right() : left(); }
    else { dy < 0 ? down() : up(); }
  }

  // --- events + plugins (reveal facade) -----------------------------------
  function on(evt, cb) { (deck.listeners[evt] = deck.listeners[evt] || []).push(cb); }
  function fire(evt) {
    var detail = { indexh: deck.h, indexv: deck.v, currentSlide: currentSlide() };
    (deck.listeners[evt] || []).forEach(function (cb) { try { cb(detail); } catch (e) {} });
  }
  function initPlugin(p) {
    if (!p || p.__qmdInited || typeof p.init !== 'function') return;
    p.__qmdInited = true;
    try { p.init(facade); } catch (e) {}
  }
  function registerPlugin(p) { if (p) { deck.plugins.push(p); if (deck.ready) initPlugin(p); } }

  // --- lifecycle ----------------------------------------------------------
  function initialize(opts) {
    if (opts) for (var k in opts) deck.config[k] = opts[k];
    if (deck.ready) { sync(); return facade; } // idempotent: client.js may call again
    var rev = revealEl();
    if (!rev || !slidesEl()) return facade;
    var qmd = new URLSearchParams(location.search).get('qmd');
    deck.mode = qmd === 'speaker' ? 'speaker' : (qmd === 'embed' ? 'embed' : 'normal');
    // reveal-styled extensions expect a .reveal-viewport host (e.g. liquid-glass
    // inserts background layers behind .reveal); reveal puts it on .reveal's parent.
    if (rev.parentNode && rev.parentNode.classList) rev.parentNode.classList.add('reveal-viewport');
    var d = document.documentElement.style;
    d.setProperty('--qmd-deck-w', deck.config.width + 'px');
    d.setProperty('--qmd-deck-h', deck.config.height + 'px');

    // The speaker window doesn't render the deck itself; it builds the control UI.
    if (deck.mode === 'speaker') { initSpeaker(); deck.ready = true; return facade; }

    if (!readHash()) { deck.h = 0; deck.v = 0; }
    clampIndices(); apply(); layout(); updateNumber();
    rev.classList.add('qmd-ready'); // reveal the deck now the first slide is placed
    window.addEventListener('resize', layout);
    window.addEventListener('message', onMessage); // sync (audience) / goto (embed)
    // An embed preview (in the speaker window's iframes) is passive: no input, no
    // broadcasting; it only follows postMessage 'goto'.
    if (deck.mode === 'normal') {
      document.addEventListener('keydown', onKey);
      rev.addEventListener('touchstart', onTouchStart, { passive: true });
      rev.addEventListener('touchend', onTouchEnd, { passive: true });
      slidesEl().addEventListener('click', onSlidesClick);
      window.addEventListener('hashchange', onHashChange);
    }
    deck.ready = true;
    deck.plugins.forEach(initPlugin);
    fire('ready');
    fire('slidechanged');
    return facade;
  }
  // Re-scan after an incremental block patch, PRESERVING the current index so an
  // edit never resets the deck to slide 0 or destroys live block state.
  function sync() {
    if (!deck.ready) return;
    clampIndices(); apply(); layout(); updateNumber();
  }

  var facade = {
    initialize: initialize,
    configure: function (o) { if (o) for (var k in o) deck.config[k] = o[k]; },
    sync: sync,
    layout: layout,
    slide: function (h, v) { moveTo(h || 0, v || 0, true); },
    next: next, prev: prev, left: left, right: right, up: up, down: down,
    getIndices: function () { return { h: deck.h, v: deck.v, f: deck.frag }; },
    getCurrentSlide: currentSlide,
    getSlides: allSlides,
    on: on, addEventListener: on,
    registerPlugin: registerPlugin,
    openSpeaker: openSpeaker,
    isReady: function () { return deck.ready; },
  };

  window.QmdDeck = facade;
  window.Reveal = facade; // compatibility facade for reveal extensions
})();
