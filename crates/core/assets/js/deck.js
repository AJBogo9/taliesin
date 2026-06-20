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

  // --- grid layout + camera ----------------------------------------------
  // Every slide is laid out once in a 2-D grid: top-level slides across (column =
  // h), a stack's sub-slides down under their column (row = v). One transform on
  // `.slides` is the "camera": focused on the current cell at full scale (normal),
  // or zoomed out to frame the whole map (overview). Panning the camera between
  // cells IS the slide transition; zooming it out IS the overview. There is no
  // second view, so the two animate into each other with no cut.
  function gridDims() {
    var T = tops(), rows = 1;
    T.forEach(function (top) { rows = Math.max(rows, vertsOf(top).length); });
    return { cols: Math.max(1, T.length), rows: rows };
  }
  // Place each section at its grid cell via an inline transform. A stack wrapper is
  // translated to its column; its children drop down by row, relative to it. In
  // overview each leaf tile shrinks slightly to open a gutter between flush cells.
  function positionGrid() {
    var W = deck.config.width, H = deck.config.height;
    var gut = deck.overview ? ' scale(.9)' : ''; // shrink tiles in overview to open gutters
    var T = tops(), s = slidesEl();
    if (s) s.style.setProperty('--qmd-cols', T.length); // for the overview spine width
    T.forEach(function (top, h) {
      if (isStack(top)) {
        top.classList.add('qmd-stack');
        top.style.setProperty('--branch-rows', vertsOf(top).length); // for the branch connector height
        top.style.transform = 'translate(' + (h * W) + 'px,0px)';
        vertsOf(top).forEach(function (sec, v) {
          sec.style.transform = 'translate(0px,' + (v * H) + 'px)' + gut;
        });
      } else {
        top.style.transform = 'translate(' + (h * W) + 'px,0px)' + gut;
      }
    });
  }
  // The camera: one translate+scale on `.slides`, mapping world coords to screen so
  // the target rect lands centred in the viewport.
  function setCamera(animate) {
    var s = slidesEl(), rev = revealEl(); if (!s || !rev) return;
    var W = deck.config.width, H = deck.config.height;
    // The "stage" is `.reveal`, a fixed 16:9 box centred in the viewport (CSS). The
    // cell fills it exactly, so adjacent cells fall outside and are clipped — no
    // peek — and the area around the stage is the letterbox.
    var sw = rev.clientWidth || window.innerWidth, sh = rev.clientHeight || window.innerHeight;
    var scale, cx, cy;
    if (deck.overview) {
      if (!deck.ov) fitOverview();                          // free "map" camera: fit-all, then wheel/drag
      scale = deck.ov.scale; cx = deck.ov.cx; cy = deck.ov.cy;
    } else {
      scale = Math.min(sw / W, sh / H);                    // one cell fills the stage exactly
      var top = tops()[deck.h], row = top && isStack(top) ? deck.v : 0;
      cx = deck.h * W + W / 2; cy = row * H + H / 2;        // centre the current cell
    }
    if (!(scale > 0)) scale = 1;
    s.style.setProperty('--qmd-thread', (3.5 / scale).toFixed(1) + 'px'); // constant ~3.5px on-screen storyline thread
    var tx = sw / 2 - scale * cx, ty = sh / 2 - scale * cy;
    s.classList.toggle('qmd-cam-anim', !!animate);
    s.style.transform = 'translate(' + tx + 'px,' + ty + 'px) scale(' + scale + ')';
    document.documentElement.style.setProperty('--qmd-deck-scale', String(scale));
  }
  function layout() {
    if (!slidesEl()) return;
    positionGrid();
    applyBackgrounds();
    allSlides().forEach(fitSlide); // all slides are laid out now, not just the current one
    if (deck.overview) fitOverview(); // viewport changed: re-fit the map
    setCamera(false);
  }

  // --- show the current slide --------------------------------------------
  // Visibility is driven by an inline `display: none !important` on hidden
  // slides (not just a CSS class), so it beats theme rules that force a display
  // on every section — e.g. liquid-glass's `section { display: flex !important }`.
  // The present slide has its inline display removed so the theme's own layout
  // (or deck.css's `.present` rule) decides how it renders. The past/present/
  // future classes are kept for CSS transitions.
  // The non-camera part of a slide change: present/past/future classes, fragments,
  // chrome. Split out so auto-animate can update state without moving the camera.
  function applyClasses() {
    var T = tops();
    var curTop = T[deck.h];
    T.forEach(function (top, i) {
      setClass(top, i < deck.h ? 'past' : (i > deck.h ? 'future' : 'present'));
      if (isStack(top)) {
        vertsOf(top).forEach(function (sec, j) {
          setClass(sec, top === curTop ? (j < deck.v ? 'past' : (j > deck.v ? 'future' : 'present')) : 'future');
        });
      }
    });
    applyFragments();
    if (deck.draw) redrawAnnotations(); // restore the new slide's annotations
    updateChrome(); // progress bar / menu state follow the current slide
    deck.lastSlide = currentSlide(); // remember for the next auto-animate transition
  }
  function apply() {
    applyClasses();
    setCamera(true); // pan/zoom the camera to the current cell (the transition)
  }
  // --- per-slide backgrounds ---------------------------------------------
  // Each slide carries its own `data-background-*` as a layer behind its content,
  // so the background travels with the slide as the camera pans, and shows per-tile
  // in overview. `.qmd-dark-bg` on the section flips its own text light over a dark
  // / image / gradient background. Set once per layout (the attributes are static).
  function ensureSlideBg(sec) {
    var bg = sec.querySelector(':scope > .qmd-slide-bg');
    if (!bg) {
      bg = document.createElement('div');
      bg.className = 'qmd-slide-bg';
      sec.insertBefore(bg, sec.firstChild);
    }
    return bg;
  }
  function applyBackgrounds() {
    allSlides().forEach(function (sec) {
      var color = sec.getAttribute('data-background-color');
      var gradient = sec.getAttribute('data-background-gradient');
      var image = sec.getAttribute('data-background-image');
      sec.classList.remove('qmd-dark-bg');
      var existing = sec.querySelector(':scope > .qmd-slide-bg');
      if (!color && !gradient && !image) { if (existing) existing.remove(); return; }
      var bg = ensureSlideBg(sec);
      bg.style.cssText = '';
      if (color) bg.style.backgroundColor = color;
      if (gradient) bg.style.backgroundImage = gradient;
      if (image) {
        bg.style.backgroundImage = 'url("' + image + '")';
        bg.style.backgroundSize = sec.getAttribute('data-background-size') || 'cover';
        bg.style.backgroundPosition = sec.getAttribute('data-background-position') || 'center';
        bg.style.backgroundRepeat = sec.getAttribute('data-background-repeat') || 'no-repeat';
      }
      if (image || gradient || (color && isDarkColor(color))) sec.classList.add('qmd-dark-bg');
    });
  }
  function isDarkColor(c) {
    var m = c.replace(/\s/g, '').match(/^#?([0-9a-f]{3}|[0-9a-f]{6})$/i);
    if (!m) return true; // named/unknown colour -> assume dark (decorative)
    var h = m[1];
    if (h.length === 3) h = h[0] + h[0] + h[1] + h[1] + h[2] + h[2];
    var r = parseInt(h.slice(0, 2), 16), g = parseInt(h.slice(2, 4), 16), b = parseInt(h.slice(4, 6), 16);
    return 0.299 * r + 0.587 * g + 0.114 * b < 140;
  }

  // --- auto-animate -------------------------------------------------------
  // When moving between two consecutive `data-auto-animate` slides, matched
  // elements (same tag + text) tween from their position/size on the old slide to
  // the new one (FLIP: measure both, translate the element to its old spot, then
  // animate to identity). Unmatched elements just appear.
  var AA_SEL = 'h1,h2,h3,h4,p,li,pre,blockquote,img,figure';
  function isAutoAnimate(s) { return !!(s && s.hasAttribute && s.hasAttribute('data-auto-animate')); }
  function aaKey(el) { return el.tagName + '|' + (el.textContent || '').replace(/\s+/g, ' ').trim(); }
  // Measure matched element rects in both slides (both must be laid out, so the
  // incoming slide is briefly force-shown — no paint happens mid-call).
  function snapshotMatched(from, to) {
    to.style.setProperty('display', 'block', 'important');
    var byKey = {};
    Array.prototype.forEach.call(from.querySelectorAll(AA_SEL), function (el) {
      (byKey[aaKey(el)] || (byKey[aaKey(el)] = [])).push(el);
    });
    var snap = [];
    Array.prototype.forEach.call(to.querySelectorAll(AA_SEL), function (el) {
      var list = byKey[aaKey(el)];
      if (list && list.length) {
        var a = list.shift();
        snap.push({
          to: el,
          fr: a.getBoundingClientRect(), tr: el.getBoundingClientRect(),
          ff: getComputedStyle(a).fontSize, tf: getComputedStyle(el).fontSize,
        });
      }
    });
    to.style.removeProperty('display');
    return snap;
  }
  function flipTo(snap, to) {
    var scale = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--qmd-deck-scale')) || 1;
    snap.forEach(function (s) {
      var el = s.to, st = el.style;
      var dx = (s.fr.left - s.tr.left) / scale, dy = (s.fr.top - s.tr.top) / scale;
      var animFont = s.ff !== s.tf;
      if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5 && !animFont) return;
      st.transition = 'none';
      st.transformOrigin = 'top left';
      st.transform = 'translate(' + dx + 'px,' + dy + 'px)';
      if (animFont) st.fontSize = s.ff;
      void el.offsetWidth; // reflow so the start state sticks before we animate
      st.transition = 'transform .5s cubic-bezier(.2,.8,.2,1), font-size .5s cubic-bezier(.2,.8,.2,1)';
      st.transform = 'translate(0,0)';
      if (animFont) st.fontSize = s.tf;
      setTimeout(function () {
        st.transition = ''; st.transform = ''; st.transformOrigin = '';
        if (animFont) st.fontSize = '';
      }, 520);
    });
    setTimeout(function () { to.classList.remove('qmd-aa'); }, 520);
  }
  // Auto-animate in the camera model: instead of panning between the two cells, hold
  // the camera and overlay `to` on `from`'s cell so the matched elements morph in
  // place; then snap `to` and the camera to `to`'s real cell together — a net-zero
  // screen move, so the reposition is invisible.
  function autoAnimateTo(from, to) {
    var toTransform = to.style.transform;       // to's real grid cell
    to.style.transform = from.style.transform;  // overlap `to` onto `from`'s cell
    to.classList.add('qmd-aa');
    var snap = snapshotMatched(from, to);        // measure both at the same screen spot
    from.style.opacity = '0';                    // hide the old slide; the morph carries the motion
    applyClasses();                              // update state, but DON'T move the camera
    flipTo(snap, to);
    setTimeout(function () {
      from.style.opacity = '';
      to.style.transform = toTransform;          // restore `to`'s real cell ...
      setCamera(false);                          // ... and move the camera to it (net screen move = 0)
    }, 520);
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
    slide.querySelectorAll(FRAG_SEL + ', pre[data-code-lines], .magic-move').forEach(function (node) {
      if (node.classList.contains('magic-move')) {
        var n = node.querySelectorAll(':scope > pre').length;
        for (var k = 1; k < n; k++) steps.push({ mm: node }); // one step per block-to-block morph
      } else if (node.tagName === 'PRE') {
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
    var mmCount = new Map();
    slide.querySelectorAll('.magic-move').forEach(function (d) { mmCount.set(d, 0); });
    // then apply each taken step in order (later code steps overwrite earlier)
    for (var i = 0; i < deck.frag; i++) {
      var s = steps[i];
      if (s.frag) s.frag.classList.add('qmd-frag-visible');
      else if (s.code) highlightLines(s.code, s.seg);
      else if (s.mm) mmCount.set(s.mm, (mmCount.get(s.mm) || 0) + 1);
    }
    mmCount.forEach(function (idx, div) { setOrMorphMM(div, idx); });
  }
  // Magic-move: show block `target` of a `.magic-move` div. On an in-slide step
  // (deck.animSteps) it morphs from the previous block: matched lines (same text)
  // glide to their new positions, new lines fade in, the old block fades out.
  function mmBlocks(div) { return Array.prototype.slice.call(div.querySelectorAll(':scope > pre')); }
  function lineText(l) { return (l.textContent || '').replace(/\s+/g, ' ').trim(); }
  function setOrMorphMM(div, target) {
    var pres = mmBlocks(div);
    if (!pres.length) return;
    target = Math.max(0, Math.min(target, pres.length - 1));
    var prev = div.__mm;
    if (deck.animSteps && prev != null && prev !== target) morphMM(div, pres, prev, target);
    else pres.forEach(function (p, i) { p.classList.toggle('qmd-mm-active', i === target); });
    div.__mm = target;
  }
  function morphMM(div, pres, from, to) {
    var blockFrom = pres[from], blockTo = pres[to];
    var scale = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--qmd-deck-scale')) || 1;
    var byText = {};
    Array.prototype.forEach.call(blockFrom.querySelectorAll('.qhl-ln'), function (l) {
      (byText[lineText(l)] || (byText[lineText(l)] = [])).push(l);
    });
    blockTo.classList.add('qmd-mm-active');
    blockFrom.classList.remove('qmd-mm-active'); // fades out (CSS opacity transition)
    Array.prototype.forEach.call(blockTo.querySelectorAll('.qhl-ln'), function (lt) {
      var list = byText[lineText(lt)], st = lt.style;
      if (list && list.length) { // matched line: glide from its old position
        var lf = list.shift(), rf = lf.getBoundingClientRect(), rt = lt.getBoundingClientRect();
        st.transition = 'none';
        st.transform = 'translate(' + (rf.left - rt.left) / scale + 'px,' + (rf.top - rt.top) / scale + 'px)';
        void lt.offsetWidth;
        st.transition = 'transform .45s cubic-bezier(.2,.8,.2,1)';
        st.transform = 'translate(0,0)';
        setTimeout(function () { st.transition = ''; st.transform = ''; }, 480);
      } else { // new line: fade in
        st.opacity = '0'; st.transition = 'none'; void lt.offsetWidth;
        st.transition = 'opacity .4s ease .12s'; st.opacity = '1';
        setTimeout(function () { st.transition = ''; st.opacity = ''; }, 560);
      }
    });
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
    deck.animSteps = true; // an in-slide step: let magic-move morph (vs. set on slide entry)
    if (deck.mode === 'speaker') updateSpeakerUI();
    else applyFragments();
    deck.animSteps = false;
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
    if (deck.mode === 'speaker') { updateSpeakerUI(); fire('slidechanged'); broadcastState(); return; }
    // Auto-animate between two consecutive opted-in slides morphs the matched
    // elements in place (autoAnimateTo) rather than panning between their cells.
    var to = currentSlide(), from = deck.lastSlide;
    if (from && to && from !== to && isAutoAnimate(from) && isAutoAnimate(to)) {
      autoAnimateTo(from, to);
    } else {
      apply(); // pan/zoom the camera to the current cell
    }
    updateNumber(); writeHash(); focusCurrent();
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

  // --- overview (a free "map" camera over the whole grid) ----------------
  // The overview is a pannable, zoomable map (so it scales to 100+ slides, not just
  // a fit-all that makes every tile a speck). deck.ov = {scale, cx, cy} is the
  // camera: the world point (cx,cy) sits at screen centre, at zoom `scale`. `fit` is
  // the zoomed-out bound (whole map visible); maxScale (one tile fills the stage) is
  // the zoomed-in bound. Wheel zooms toward the cursor, drag pans, `0` re-fits.
  function markCurrentTile() {
    var cur = currentSlide();
    allSlides().forEach(function (s) { s.classList.toggle('qmd-overview-current', s === cur); });
  }
  function fitOverview() {
    var rev = revealEl(); if (!rev) return;
    var W = deck.config.width, H = deck.config.height;
    var sw = rev.clientWidth || window.innerWidth, sh = rev.clientHeight || window.innerHeight;
    var g = gridDims(), gw = g.cols * W, gh = g.rows * H;
    var fit = Math.min(sw / gw, sh / gh) * (1 - 2 * 0.06); // whole map, with margin
    if (!(fit > 0)) fit = 1;
    deck.ov = { scale: fit, cx: gw / 2, cy: gh / 2, fit: fit };
  }
  function ovStage() {
    var rev = revealEl();
    var W = deck.config.width, H = deck.config.height;
    var sw = rev.clientWidth || window.innerWidth, sh = rev.clientHeight || window.innerHeight;
    return { sw: sw, sh: sh, maxScale: Math.min(sw / W, sh / H) };
  }
  // Keep the map from drifting into the void: clamp the centred point to the grid
  // plus a one-cell margin on every side.
  function clampOv() {
    if (!deck.ov) return;
    var W = deck.config.width, H = deck.config.height;
    var g = gridDims(), gw = g.cols * W, gh = g.rows * H;
    deck.ov.cx = Math.max(-W, Math.min(deck.ov.cx, gw + W));
    deck.ov.cy = Math.max(-H, Math.min(deck.ov.cy, gh + H));
  }
  function onOverviewWheel(e) {
    if (!deck.overview) return;
    if (!deck.ov) fitOverview();
    e.preventDefault();
    var st = ovStage(), rev = revealEl(), r = rev.getBoundingClientRect();
    var px = e.clientX - r.left, py = e.clientY - r.top;   // cursor in stage coords
    var scale = deck.ov.scale;
    var tx = st.sw / 2 - scale * deck.ov.cx, ty = st.sh / 2 - scale * deck.ov.cy;
    var wx = (px - tx) / scale, wy = (py - ty) / scale;    // world point under the cursor
    var ns = scale * Math.exp(-e.deltaY * 0.0015);          // smooth, proportional zoom
    ns = Math.max(deck.ov.fit, Math.min(ns, st.maxScale));
    deck.ov.scale = ns;
    deck.ov.cx = (st.sw / 2 - (px - ns * wx)) / ns;         // keep that point under the cursor
    deck.ov.cy = (st.sh / 2 - (py - ns * wy)) / ns;
    clampOv();
    setCamera(false);
  }
  var ovDrag = null;
  function onOverviewPointerDown(e) {
    if (!deck.overview || !deck.ov || e.button !== 0) return;
    ovDrag = { x: e.clientX, y: e.clientY, cx: deck.ov.cx, cy: deck.ov.cy, moved: false };
  }
  function onOverviewPointerMove(e) {
    if (!ovDrag) return;
    var dx = e.clientX - ovDrag.x, dy = e.clientY - ovDrag.y;
    if (!ovDrag.moved && dx * dx + dy * dy < 25) return;    // 5px before it counts as a drag
    ovDrag.moved = true;
    revealEl().classList.add('qmd-ov-panning');
    deck.ov.cx = ovDrag.cx - dx / deck.ov.scale;
    deck.ov.cy = ovDrag.cy - dy / deck.ov.scale;
    clampOv();
    setCamera(false);
  }
  function onOverviewPointerUp() {
    if (ovDrag && ovDrag.moved) deck.ovDragged = true;      // a pan: swallow the click that follows
    ovDrag = null;
    var rev = revealEl(); if (rev) rev.classList.remove('qmd-ov-panning');
  }
  // Pan (if needed) so the highlighted tile stays comfortably on-screen; keep zoom.
  function ensureCurrentTileVisible(animate) {
    if (!deck.overview || !deck.ov) return;
    var st = ovStage(), W = deck.config.width, H = deck.config.height;
    var top = tops()[deck.h], row = top && isStack(top) ? deck.v : 0;
    var wx = deck.h * W + W / 2, wy = row * H + H / 2;
    var scale = deck.ov.scale;
    var sx = st.sw / 2 + scale * (wx - deck.ov.cx);
    var sy = st.sh / 2 + scale * (wy - deck.ov.cy);
    var mx = st.sw * 0.18, my = st.sh * 0.18;
    if (sx < mx || sx > st.sw - mx || sy < my || sy > st.sh - my) {
      deck.ov.cx = wx; deck.ov.cy = wy; clampOv();
      setCamera(animate);
    }
  }
  function setOverview(on) {
    if (on === deck.overview) return;
    var rev = revealEl();
    if (!rev) return;
    deck.overview = on;
    rev.classList.toggle('overview', on);
    if (on && deck.draw && deck.draw.on) { deck.draw.on = false; rev.classList.remove('qmd-drawing'); }
    if (on) { fitOverview(); markCurrentTile(); }
    else { deck.ov = null; allSlides().forEach(function (s) { s.classList.remove('qmd-overview-current'); }); }
    positionGrid(); // add (or remove) the per-tile gutter shrink
    setCamera(true); // zoom out to the map, or back into the current cell
  }
  // Move the overview highlight one leaf forward/back in deck order, keeping it
  // on-screen as the map pans.
  function moveHighlight(delta) {
    var leaves = allSlides();
    var i = leaves.indexOf(currentSlide());
    i = Math.max(0, Math.min((i < 0 ? 0 : i) + delta, leaves.length - 1));
    var target = leaves[i], T = tops();
    for (var h = 0; h < T.length; h++) {
      var v = vertsOf(T[h]).indexOf(target);
      if (v >= 0) { deck.h = h; deck.v = v; break; }
    }
    markCurrentTile();
    ensureCurrentTileVisible(true);
  }
  function onSlidesClick(e) {
    if (!deck.overview) return;
    if (deck.ovDragged) { deck.ovDragged = false; return; } // that was a pan, not a pick
    var sec = e.target.closest && e.target.closest('.reveal .slides section');
    if (!sec) return;
    e.preventDefault();
    var T = tops();
    for (var h = 0; h < T.length; h++) {
      var v = vertsOf(T[h]).indexOf(sec);
      if (sec === T[h] || v >= 0) { setOverview(false); moveTo(h, v < 0 ? 0 : v, true); return; }
    }
    setOverview(false);
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
    else { apply(); updateNumber(); writeHash(); }
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

  // --- PDF export (print) ------------------------------------------------
  // On `beforeprint` (Cmd/Ctrl+P) the deck flattens to one slide per page: every
  // slide shown, transforms dropped, all fragments revealed, code un-dimmed.
  // `@page` makes each page the deck's aspect, so the browser's "Save as PDF"
  // yields a clean handout. `?qmd=print` enters the same layout on screen.
  function enterPrint() {
    var rev = revealEl(); if (!rev) return;
    document.documentElement.classList.add('qmd-print');
    rev.classList.remove('qmd-dark-bg'); // bg layer is hidden in print; keep text readable on the page
    tops().forEach(function (top) {
      top.style.removeProperty('display'); top.removeAttribute('aria-hidden');
      if (isStack(top)) {
        top.classList.add('qmd-print-stack');
        vertsOf(top).forEach(function (s) { s.style.removeProperty('display'); s.removeAttribute('aria-hidden'); });
      }
    });
    rev.querySelectorAll(FRAG_SEL).forEach(function (e) { e.classList.add('qmd-frag-visible'); });
    rev.querySelectorAll('pre[data-code-lines]').forEach(function (p) { highlightLines(p, 'all'); });
    rev.querySelectorAll('.magic-move').forEach(function (div) { // show the final block
      var pres = mmBlocks(div);
      pres.forEach(function (p, i) { p.classList.toggle('qmd-mm-active', i === pres.length - 1); });
    });
    allSlides().forEach(fitSlide); // size every slide to its page (not just visited ones)
  }
  function exitPrint() {
    document.documentElement.classList.remove('qmd-print');
    tops().forEach(function (t) { t.classList.remove('qmd-print-stack'); });
    apply();
  }

  // --- scroll / reader mode ----------------------------------------------
  // On a narrow/portrait screen (or ?qmd=scroll) the fixed-aspect deck would
  // letterbox badly, so it flattens to a vertically-scrollable, readable document:
  // every slide stacked full-width at a responsive size, all fragments revealed.
  function enterScroll() {
    var rev = revealEl();
    if (!rev || deck.scroll) return;
    deck.scroll = true;
    document.documentElement.classList.add('qmd-scroll');
    rev.classList.remove('qmd-dark-bg'); // backgrounds are hidden in reader; keep text readable
    tops().forEach(function (top) {
      top.style.removeProperty('display');
      top.style.removeProperty('font-size');
      top.removeAttribute('aria-hidden');
      if (isStack(top)) {
        top.classList.add('qmd-scroll-stack');
        vertsOf(top).forEach(function (s) { s.style.removeProperty('display'); s.style.removeProperty('font-size'); });
      }
    });
    rev.querySelectorAll(FRAG_SEL).forEach(function (e) { e.classList.add('qmd-frag-visible'); });
    rev.querySelectorAll('pre[data-code-lines]').forEach(function (p) { highlightLines(p, 'all'); });
    rev.querySelectorAll('.magic-move').forEach(function (div) {
      var pres = mmBlocks(div);
      pres.forEach(function (p, i) { p.classList.toggle('qmd-mm-active', i === pres.length - 1); });
    });
  }
  function exitScroll() {
    if (!deck.scroll) return;
    deck.scroll = false;
    document.documentElement.classList.remove('qmd-scroll');
    tops().forEach(function (t) { t.classList.remove('qmd-scroll-stack'); });
    apply();
    layout();
  }

  // --- drawing / annotations ---------------------------------------------
  // `d` toggles a pen: a canvas inside `.slides` (so it scales with the deck) that
  // captures pointer strokes over the current slide. Strokes are kept per slide and
  // redrawn on navigation. A small toolbar offers colours, an eraser and clear.
  function ensureDraw() {
    if (deck.draw) return deck.draw;
    var canvas = document.createElement('canvas');
    canvas.className = 'qmd-draw';
    canvas.width = deck.config.width;
    canvas.height = deck.config.height;
    slidesEl().appendChild(canvas);
    var bar = document.createElement('div');
    bar.className = 'qmd-draw-bar';
    bar.innerHTML =
      '<button class="qmd-draw-color" data-c="#ef4444" style="background:#ef4444"></button>' +
      '<button class="qmd-draw-color" data-c="#3b82f6" style="background:#3b82f6"></button>' +
      '<button class="qmd-draw-color" data-c="#22c55e" style="background:#22c55e"></button>' +
      '<button class="qmd-draw-erase" title="Erase">erase</button>' +
      '<button class="qmd-draw-clear" title="Clear slide">clear</button>' +
      '<button class="qmd-draw-done" title="Done (d)">done</button>';
    revealEl().appendChild(bar);
    var d = deck.draw = {
      canvas: canvas, ctx: canvas.getContext('2d'), bar: bar,
      color: '#ef4444', erase: false, on: false, strokes: {}, drawing: false, stroke: null,
    };
    bar.querySelectorAll('.qmd-draw-color').forEach(function (b) {
      b.addEventListener('click', function () { d.color = b.getAttribute('data-c'); d.erase = false; updateDrawBar(); });
    });
    bar.querySelector('.qmd-draw-erase').addEventListener('click', function () { d.erase = !d.erase; updateDrawBar(); });
    bar.querySelector('.qmd-draw-clear').addEventListener('click', clearSlideDrawing);
    bar.querySelector('.qmd-draw-done').addEventListener('click', function () { toggleDraw(false); });
    canvas.addEventListener('pointerdown', drawStart);
    canvas.addEventListener('pointermove', drawMove);
    window.addEventListener('pointerup', function () { if (deck.draw) deck.draw.drawing = false; });
    return d;
  }
  function updateDrawBar() {
    var d = deck.draw; if (!d) return;
    d.bar.querySelectorAll('.qmd-draw-color').forEach(function (b) {
      b.classList.toggle('sel', !d.erase && b.getAttribute('data-c') === d.color);
    });
    d.bar.querySelector('.qmd-draw-erase').classList.toggle('sel', d.erase);
  }
  function toggleDraw(force) {
    if (deck.mode !== 'normal' || deck.scroll || deck.overview) return;
    var d = ensureDraw();
    d.on = (force == null) ? !d.on : force;
    revealEl().classList.toggle('qmd-drawing', d.on);
    if (d.on) { redrawAnnotations(); updateDrawBar(); }
  }
  function drawKey() { var c = currentSlide(); return c ? (c.id || 'i' + deck.h + '-' + deck.v) : ''; }
  function drawPoint(e) {
    var d = deck.draw, r = d.canvas.getBoundingClientRect();
    return { x: (e.clientX - r.left) / r.width * d.canvas.width, y: (e.clientY - r.top) / r.height * d.canvas.height };
  }
  function drawStroke(ctx, s) {
    ctx.save();
    ctx.globalCompositeOperation = s.erase ? 'destination-out' : 'source-over';
    ctx.strokeStyle = s.color; ctx.lineWidth = s.w; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.beginPath();
    s.pts.forEach(function (p, i) { i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y); });
    ctx.stroke();
    ctx.restore();
  }
  function drawStart(e) {
    var d = deck.draw; if (!d.on) return;
    e.preventDefault();
    d.drawing = true;
    d.stroke = { color: d.color, erase: d.erase, w: d.erase ? 30 : 4, pts: [drawPoint(e)] };
    (d.strokes[drawKey()] || (d.strokes[drawKey()] = [])).push(d.stroke);
  }
  function drawMove(e) {
    var d = deck.draw; if (!d.on || !d.drawing) return;
    var p = drawPoint(e), prev = d.stroke.pts[d.stroke.pts.length - 1];
    d.stroke.pts.push(p);
    var ctx = d.ctx;
    ctx.save();
    ctx.globalCompositeOperation = d.stroke.erase ? 'destination-out' : 'source-over';
    ctx.strokeStyle = d.stroke.color; ctx.lineWidth = d.stroke.w; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.beginPath(); ctx.moveTo(prev.x, prev.y); ctx.lineTo(p.x, p.y); ctx.stroke();
    ctx.restore();
  }
  function redrawAnnotations() {
    var d = deck.draw; if (!d) return;
    d.ctx.clearRect(0, 0, d.canvas.width, d.canvas.height);
    (d.strokes[drawKey()] || []).forEach(function (s) { drawStroke(d.ctx, s); });
  }
  function clearSlideDrawing() {
    var d = deck.draw; if (!d) return;
    d.strokes[drawKey()] = [];
    redrawAnnotations();
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
    var ph = deck.h, pv = deck.v;
    if (!readHash()) return;
    clampIndices();
    if (deck.h === ph && deck.v === pv) return; // our own writeHash, or no real change
    deck.frag = fragCount(); apply(); updateNumber(); fire('slidechanged'); // apply pans the camera
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
    if (deck.scroll) return; // reader mode: let the browser scroll normally
    var t = /** @type {any} */ (e.target);
    if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
    var handled = true;
    if (deck.menuOpen) { // swallow keys while the control menu is open
      if (e.key === 'Escape' || e.key === 'm' || e.key === '?') { toggleMenu(false); e.preventDefault(); }
      return;
    }
    if (deck.overview) {
      switch (e.key) {
        // `o` toggles overview closed (mirrors opening it with `o`), alongside
        // Escape/Enter/Space; all land on the highlighted slide.
        case 'Escape': case 'Enter': case ' ': case 'o': setOverview(false); moveTo(deck.h, deck.v, true); break;
        case 'ArrowRight': case 'ArrowDown': moveHighlight(1); break;
        case 'ArrowLeft': case 'ArrowUp': moveHighlight(-1); break;
        case '0': fitOverview(); setCamera(true); break; // re-fit the whole map
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
      case 'd': toggleDraw(); break;
      case 'f': toggleFullscreen(); break;
      case 'm': case '?': toggleMenu(); break;
      default: handled = false;
    }
    if (handled) e.preventDefault();
  }
  var touch = { x: null, y: null, t: 0 };
  function onTouchStart(e) {
    if (deck.scroll || e.touches.length !== 1) { touch.x = null; return; } // reader mode scrolls
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

  // --- on-screen chrome: control menu, progress bar, nav arrows -----------
  // The deck's features (overview, annotate, speaker, PDF, reader, dark mode)
  // were keyboard-only and so undiscoverable; this surfaces them in a corner
  // menu (reveal-style) plus a progress bar + prev/next arrows. Built once in
  // normal mode; auto-hides on idle. Fixed to the viewport (not the scaled
  // .slides), so it doesn't ride the deck transform.
  function svg(p) { return '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' + p + '</svg>'; }
  var IC = {
    menu: svg('<path d="M4 7h16M4 12h16M4 17h16"/>'),
    grid: svg('<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/>'),
    pen: svg('<path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/>'),
    speak: svg('<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>'),
    fs: svg('<path d="M8 3H5a2 2 0 0 0-2 2v3M16 3h3a2 2 0 0 1 2 2v3M8 21H5a2 2 0 0 1-2-2v-3M16 21h3a2 2 0 0 0 2-2v-3"/>'),
    pdf: svg('<path d="M14 3v5h5"/><path d="M5 3h9l5 5v13H5z"/>'),
    reader: svg('<path d="M4 5h16M4 10h16M4 15h10"/>'),
    moon: svg('<path d="M21 12.8A8 8 0 1 1 11.2 3a6 6 0 0 0 9.8 9.8z"/>'),
  };
  function esc(s) { return String(s).replace(/[&<>"]/g, function (c) { return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]; }); }
  function tool(action, ico, label, hint) {
    return '<button class="qmd-menu-item" data-action="' + action + '"><span class="qmd-menu-ico">' + ico +
      '</span><span class="qmd-menu-label">' + label + '</span>' + (hint ? '<span class="qmd-menu-hint">' + hint + '</span>' : '') + '</button>';
  }
  function key(k, d) { return '<div class="qmd-key"><kbd>' + k + '</kbd><span>' + d + '</span></div>'; }
  var KEYS_HTML =
    key('← →', 'Navigate') + key('↑ ↓', 'Vertical slides') + key('Space', 'Next') +
    key('O', 'Overview') + key('F', 'Fullscreen') + key('S', 'Speaker view') +
    key('D', 'Annotate') + key('⌘/Ctrl P', 'Export PDF') + key('?', 'This menu') + key('Esc', 'Close');

  function buildChrome() {
    var rev = revealEl();
    if (!rev || deck.chrome) return;
    var prog = document.createElement('div');
    prog.className = 'qmd-progress';
    prog.innerHTML = '<div class="qmd-progress-fill"></div>';
    rev.appendChild(prog);
    var ctl = document.createElement('div');
    ctl.className = 'qmd-controls';
    ctl.innerHTML =
      '<button class="qmd-ctl qmd-ctl-prev" aria-label="Previous slide" title="Previous (←)">‹</button>' +
      '<button class="qmd-ctl qmd-ctl-next" aria-label="Next slide" title="Next (→)">›</button>' +
      '<button class="qmd-ctl qmd-ctl-menu" aria-label="Menu" title="Menu (m)">' + IC.menu + '</button>';
    rev.appendChild(ctl);
    ctl.querySelector('.qmd-ctl-prev').addEventListener('click', function () { prev(); });
    ctl.querySelector('.qmd-ctl-next').addEventListener('click', function () { next(); });
    ctl.querySelector('.qmd-ctl-menu').addEventListener('click', function () { toggleMenu(); });
    deck.chrome = { fill: prog.querySelector('.qmd-progress-fill'), ctl: ctl };
    buildMenu();
    document.addEventListener('mousemove', showChrome);
    document.addEventListener('touchstart', showChrome, { passive: true });
    showChrome();
    updateChrome();
  }
  function buildMenu() {
    var menu = document.createElement('div');
    menu.className = 'qmd-menu';
    menu.setAttribute('hidden', '');
    var themeRow = (window.qmdDeckThemeManaged && !window.qmdDeckEmbedded)
      ? '<div class="qmd-menu-head">Theme</div><div class="qmd-menu-tools">' +
        tool('theme', IC.moon, 'Dark mode', '<span class="qmd-theme-state"></span>') + '</div>'
      : '';
    menu.innerHTML =
      '<div class="qmd-menu-head">Slides</div><div class="qmd-menu-slides"></div>' +
      '<div class="qmd-menu-head">Tools</div><div class="qmd-menu-tools">' +
        tool('overview', IC.grid, 'Overview', 'O') +
        tool('reader', IC.reader, 'Reader mode', '') +
        tool('draw', IC.pen, 'Annotate', 'D') +
        tool('speaker', IC.speak, 'Speaker view', 'S') +
        tool('fullscreen', IC.fs, 'Fullscreen', 'F') +
        tool('print', IC.pdf, 'Export PDF', '⌘P') +
      '</div>' + themeRow +
      '<div class="qmd-menu-head">Keyboard</div><div class="qmd-menu-keys">' + KEYS_HTML + '</div>';
    document.body.appendChild(menu);
    var backdrop = document.createElement('div');
    backdrop.className = 'qmd-menu-backdrop';
    backdrop.setAttribute('hidden', '');
    backdrop.addEventListener('click', function () { toggleMenu(false); });
    document.body.appendChild(backdrop);
    menu.addEventListener('click', onMenuClick);
    deck.menu = menu;
    deck.menuBackdrop = backdrop;
  }
  function refreshSlideList() {
    var box = deck.menu && deck.menu.querySelector('.qmd-menu-slides');
    if (!box) return;
    var all = allSlides(), cur = currentSlide(), html = '';
    for (var i = 0; i < all.length; i++) {
      var hd = all[i].querySelector('h1,h2,h3');
      var label = hd ? hd.textContent.trim() : ('Slide ' + (i + 1));
      html += '<button class="qmd-menu-slide' + (all[i] === cur ? ' qmd-on' : '') + '" data-i="' + i + '">' +
        '<span class="qmd-menu-slide-n">' + (i + 1) + '</span><span class="qmd-menu-slide-t">' + esc(label) + '</span></button>';
    }
    box.innerHTML = html;
    var on = box.querySelector('.qmd-on');
    if (on && on.scrollIntoView) on.scrollIntoView({ block: 'nearest' });
  }
  function markActiveTools() {
    if (!deck.menu) return;
    var set = function (action, on) {
      var b = deck.menu.querySelector('[data-action="' + action + '"]');
      if (b) b.classList.toggle('qmd-on', !!on);
    };
    set('reader', deck.scroll);
    set('draw', deck.draw && deck.draw.on);
    set('overview', deck.overview);
    var st = deck.menu.querySelector('.qmd-theme-state');
    if (st) st.textContent = document.documentElement.classList.contains('qmd-deck-dark') ? 'On' : 'Off';
  }
  function onMenuClick(e) {
    var slide = e.target.closest && e.target.closest('.qmd-menu-slide');
    if (slide) { jumpToIndex(parseInt(slide.getAttribute('data-i'), 10)); return; }
    var item = e.target.closest && e.target.closest('.qmd-menu-item');
    if (!item) return;
    var a = item.getAttribute('data-action');
    if (a === 'theme') { toggleThemeMode(); return; } // stay open; reflects state
    toggleMenu(false);
    if (a === 'overview') setOverview(true);
    else if (a === 'reader') toggleScroll();
    else if (a === 'draw') toggleDraw(true);
    else if (a === 'speaker') openSpeaker();
    else if (a === 'fullscreen') toggleFullscreen();
    else if (a === 'print') window.print();
  }
  function toggleMenu(force) {
    if (!deck.menu) return;
    var open = (force == null) ? deck.menu.hasAttribute('hidden') : force;
    deck.menuOpen = open;
    if (open) {
      refreshSlideList(); markActiveTools();
      deck.menu.removeAttribute('hidden'); deck.menuBackdrop.removeAttribute('hidden');
      showChrome();
    } else {
      deck.menu.setAttribute('hidden', ''); deck.menuBackdrop.setAttribute('hidden', '');
    }
  }
  function jumpToIndex(i) {
    var all = allSlides(), el = all[i];
    if (!el) return;
    var ix = indexOf(el);
    toggleMenu(false);
    if (deck.overview) setOverview(false);
    moveTo(ix.h, ix.v, true);
  }
  function toggleScroll() { deck.scroll ? exitScroll() : enterScroll(); updateChrome(); }
  function toggleFullscreen() {
    try {
      if (document.fullscreenElement) document.exitFullscreen();
      else if (document.documentElement.requestFullscreen) document.documentElement.requestFullscreen();
    } catch (e) {}
  }
  function toggleThemeMode() {
    if (!window.qmdDeckSetTheme) return;
    var dark = document.documentElement.classList.contains('qmd-deck-dark');
    window.qmdDeckSetTheme(dark ? 'light' : 'dark');
    markActiveTools();
  }
  function updateChrome() {
    if (!deck.chrome) return;
    var all = allSlides(), idx = all.indexOf(currentSlide());
    var pct = all.length ? (idx + 1) / all.length * 100 : 0;
    deck.chrome.fill.style.width = pct + '%';
  }
  var idleTimer;
  function showChrome() {
    document.documentElement.classList.remove('qmd-idle');
    clearTimeout(idleTimer);
    idleTimer = setTimeout(function () { if (!deck.menuOpen) document.documentElement.classList.add('qmd-idle'); }, 3000);
  }

  // --- lifecycle ----------------------------------------------------------
  function initialize(opts) {
    if (opts) for (var k in opts) deck.config[k] = opts[k];
    if (deck.ready) { sync(); return facade; } // idempotent: client.js may call again
    var rev = revealEl();
    if (!rev || !slidesEl()) return facade;
    var qmd = new URLSearchParams(location.search).get('qmd');
    deck.mode = qmd === 'speaker' ? 'speaker' : qmd === 'embed' ? 'embed'
      : qmd === 'print' ? 'print' : 'normal';
    // reveal-styled extensions expect a .reveal-viewport host (e.g. liquid-glass
    // inserts background layers behind .reveal); reveal puts it on .reveal's parent.
    if (rev.parentNode && rev.parentNode.classList) rev.parentNode.classList.add('reveal-viewport');
    var d = document.documentElement.style;
    d.setProperty('--qmd-deck-w', deck.config.width + 'px');
    d.setProperty('--qmd-deck-h', deck.config.height + 'px');

    // The speaker window doesn't render the deck itself; it builds the control UI.
    if (deck.mode === 'speaker') { initSpeaker(); deck.ready = true; return facade; }

    // Print preview: lay every slide out as a page on screen (same layout Cmd/Ctrl+P uses).
    if (deck.mode === 'print') {
      clampIndices();
      rev.classList.add('qmd-ready');
      enterPrint();
      deck.ready = true;
      return facade;
    }

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
      rev.addEventListener('wheel', onOverviewWheel, { passive: false }); // overview: zoom the map
      rev.addEventListener('pointerdown', onOverviewPointerDown);         // overview: drag to pan
      window.addEventListener('pointermove', onOverviewPointerMove);
      window.addEventListener('pointerup', onOverviewPointerUp);
      window.addEventListener('hashchange', onHashChange);
      buildChrome(); // the control menu + progress bar + nav arrows
      // Embedded in a same-origin page: follow the host's light/dark toggle live.
      if (window.qmdDeckEmbedded && window.qmdDeckApplyTheme) {
        try {
          new MutationObserver(window.qmdDeckApplyTheme)
            .observe(window.top.document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
        } catch (e) {}
      }
      window.addEventListener('beforeprint', enterPrint); // Cmd/Ctrl+P -> one slide per page
      window.addEventListener('afterprint', exitPrint);
      // Scroll/reader mode: explicit ?qmd=scroll, or auto on a narrow/portrait screen
      // (the fixed-aspect deck letterboxes badly there); re-evaluated on resize/rotate.
      var narrow = window.matchMedia('(max-width: 600px)');
      var syncScroll = function () { (qmd === 'scroll' || narrow.matches) ? enterScroll() : exitScroll(); };
      if (narrow.addEventListener) narrow.addEventListener('change', syncScroll);
      syncScroll();
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
    print: function () { window.print(); },
    isReady: function () { return deck.ready; },
  };

  window.QmdDeck = facade;
  window.Reveal = facade; // compatibility facade for reveal extensions
})();
