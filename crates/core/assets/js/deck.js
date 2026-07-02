// taliesin deck engine: the navigation + scaling for slides, owned by the project
// so block-level incremental updates and click-to-source work in decks the same
// way they do on a page. It drives taliesin's own DOM contract
// (.tali-deck > .tali-slides > section, nested <section> stacks) and exposes a
// window.TaliesinDeck API (initialize/sync/layout/slide + on/getSlides/getCurrentSlide/
// registerPlugin) that the preview client and theme extensions bind to.
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

  function slidesEl() { return document.querySelector('.tali-deck .tali-slides'); }
  function deckEl() { return document.querySelector('.tali-deck'); }

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

  // Flat list of leaf slides (what getSlides returns), for plugins
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
  // `.tali-slides` is the "camera": focused on the current cell at full scale (normal),
  // or zoomed out to frame the whole map (overview). Panning the camera between
  // cells IS the slide transition; zooming it out IS the overview. There is no
  // second view, so the two animate into each other with no cut.
  // Group the deck into visual ROWS: each `#`-section stack is one row (its slides
  // laid out ACROSS), and a run of consecutive top-level slides is one row. So a
  // topic reads left-to-right and the next topic is the row beneath it: the main
  // storyline is the top row, with any branch/appendix as a row hanging below.
  function gridRows() {
    var T = tops(), rows = [], run = null;
    for (var h = 0; h < T.length; h++) {
      if (isStack(T[h])) {
        if (run) { rows.push(run); run = null; }
        rows.push(vertsOf(T[h]).map(function (sec, v) { return { h: h, v: v }; }));
      } else {
        if (!run) run = [];
        run.push({ h: h, v: 0 });
      }
    }
    if (run) rows.push(run);
    return rows.length ? rows : [[{ h: 0, v: 0 }]];
  }
  // The visual (row, col) of a leaf, plus the row grid it came from.
  function posOf(h, v) {
    var rows = gridRows();
    for (var r = 0; r < rows.length; r++)
      for (var c = 0; c < rows[r].length; c++)
        if (rows[r][c].h === h && rows[r][c].v === v) return { row: r, col: c, rows: rows };
    return { row: 0, col: 0, rows: rows };
  }
  function gridDims() {
    var rows = gridRows();
    var cols = rows.reduce(function (m, r) { return Math.max(m, r.length); }, 1);
    return { cols: Math.max(1, cols), rows: Math.max(1, rows.length) };
  }
  // Place each section at its grid cell via an inline transform. A stack wrapper is
  // translated to its column; its children drop down by row, relative to it. In
  // overview each leaf tile shrinks slightly to open a gutter between flush cells.
  function positionGrid() {
    var W = deck.config.width, H = deck.config.height;
    var gut = deck.overview ? ' scale(.9)' : ''; // shrink tiles in overview to open gutters
    var rows = gridRows(), T = tops(), s = slidesEl();
    var loc = {}, maxCols = 1; // per top index: its row + the column of its first leaf
    rows.forEach(function (rowArr, r) {
      maxCols = Math.max(maxCols, rowArr.length);
      rowArr.forEach(function (cell, c) { if (!(cell.h in loc)) loc[cell.h] = { row: r, col0: c }; });
    });
    if (s) {
      s.style.setProperty('--tali-cols', maxCols);
      s.style.setProperty('--tali-rows', rows.length);
    }
    T.forEach(function (top, h) {
      var L = loc[h] || { row: 0, col0: 0 };
      if (isStack(top)) {
        top.classList.add('tali-stack');
        top.style.transform = 'translate(' + (L.col0 * W) + 'px,' + (L.row * H) + 'px)';
        vertsOf(top).forEach(function (sec, v) {
          sec.style.transform = 'translate(' + (v * W) + 'px,0px)' + gut; // sub-slides flow ACROSS the row
        });
      } else {
        top.style.transform = 'translate(' + (L.col0 * W) + 'px,' + (L.row * H) + 'px)' + gut;
      }
    });
    drawThreads(rows, W, H, s);
  }
  // A horizontal connector thread per multi-slide row (topic), drawn behind the tiles
  // in `.tali-slides` world coords so it pans/zooms with the camera and reads as a line
  // joining the cards through the gutters. Rebuilt each layout; shown only in overview.
  function drawThreads(rows, W, H, s) {
    if (!s) return;
    var tl = s.querySelector(':scope > .tali-threads');
    if (!tl) { tl = document.createElement('div'); tl.className = 'tali-threads'; tl.setAttribute('aria-hidden', 'true'); s.insertBefore(tl, s.firstChild); }
    tl.innerHTML = '';
    rows.forEach(function (rowArr, r) {
      if (rowArr.length < 2) return;
      var d = document.createElement('div');
      d.className = 'tali-thread-line';
      d.style.transform = 'translate(' + (W / 2) + 'px,' + (r * H + H / 2) + 'px)';
      d.style.width = ((rowArr.length - 1) * W) + 'px';
      tl.appendChild(d);
    });
  }
  // The camera target for the current state: the cell that fills the 16:9 stage
  // (normal), or the free map camera (overview).
  function cameraTarget() {
    var rev = deckEl();
    var W = deck.config.width, H = deck.config.height;
    var sw = rev.clientWidth || window.innerWidth, sh = rev.clientHeight || window.innerHeight;
    if (deck.overview) {
      if (!deck.ov) fitOverview();
      return { cx: deck.ov.cx, cy: deck.ov.cy, scale: deck.ov.scale > 0 ? deck.ov.scale : 1 };
    }
    var scale = Math.min(sw / W, sh / H);
    var p = posOf(deck.h, deck.v);
    return { cx: p.col * W + W / 2, cy: p.row * H + H / 2, scale: scale > 0 ? scale : 1 };
  }
  // Apply a camera (one translate+scale on `.tali-slides`, mapping world -> screen so the
  // target lands centred). mode: 'css' = CSS transition, anything else = instant.
  function applyCam(cx, cy, scale, mode) {
    var s = slidesEl(), rev = deckEl(); if (!s || !rev) return;
    var W = deck.config.width;
    var sw = rev.clientWidth || window.innerWidth, sh = rev.clientHeight || window.innerHeight;
    s.style.setProperty('--tali-thread', (3.5 / scale).toFixed(1) + 'px'); // constant ~3.5px on-screen thread
    rev.classList.toggle('tali-lod-far', !!deck.overview && scale * W < 200); // semantic zoom threshold
    var tx = sw / 2 - scale * cx, ty = sh / 2 - scale * cy;
    s.classList.toggle('tali-cam-anim', mode === 'css');
    s.style.transform = 'translate(' + tx + 'px,' + ty + 'px) scale(' + scale + ')';
    document.documentElement.style.setProperty('--tali-deck-scale', String(scale));
    deck.cam = { cx: cx, cy: cy, scale: scale };
    updateMinimapView();
  }
  // van Wijk & Nuij (2003) optimal smooth zoom-and-pan: a path in [cx, cy, w] (w =
  // world width on screen) that minimises perceived velocity. Ported from
  // d3.interpolateZoom (dependency-free). Used for big moves (overview, long jumps).
  function interpolateZoom(p0, p1) {
    var rho = Math.SQRT2, rho2 = 2, rho4 = 4, eps = 1e-12;
    var ux0 = p0[0], uy0 = p0[1], w0 = p0[2], ux1 = p1[0], uy1 = p1[1], w1 = p1[2];
    var dx = ux1 - ux0, dy = uy1 - uy0, d2 = dx * dx + dy * dy, i, S;
    if (d2 < eps) {
      S = Math.log(w1 / w0) / rho;
      i = function (t) { return [ux0 + t * dx, uy0 + t * dy, w0 * Math.exp(rho * t * S)]; };
    } else {
      var d1 = Math.sqrt(d2);
      var b0 = (w1 * w1 - w0 * w0 + rho4 * d2) / (2 * w0 * rho2 * d1);
      var b1 = (w1 * w1 - w0 * w0 - rho4 * d2) / (2 * w1 * rho2 * d1);
      var r0 = Math.log(Math.sqrt(b0 * b0 + 1) - b0);
      var r1 = Math.log(Math.sqrt(b1 * b1 + 1) - b1);
      S = (r1 - r0) / rho;
      i = function (t) {
        var s = t * S, coshr0 = Math.cosh(r0);
        var u = w0 / (rho2 * d1) * (coshr0 * Math.tanh(rho * s + r0) - Math.sinh(r0));
        return [ux0 + u * dx, uy0 + u * dy, w0 * coshr0 / Math.cosh(rho * s + r0)];
      };
    }
    i.duration = S * 1000;
    return i;
  }
  function reducedMotion() {
    return window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }
  // A "big" move (worth the smooth fly) is a real zoom change or a long pan.
  function bigChange(a, b) {
    var zr = Math.max(a.scale / b.scale, b.scale / a.scale);
    if (zr > 1.4) return true;
    var rev = deckEl(), sw = rev.clientWidth || window.innerWidth;
    return Math.hypot((b.cx - a.cx) * b.scale, (b.cy - a.cy) * b.scale) > 2.2 * sw;
  }
  function flyTo(t) {
    if (deck.flyRAF) { cancelAnimationFrame(deck.flyRAF); deck.flyRAF = null; }
    var rev = deckEl(), sw = rev.clientWidth || window.innerWidth, from = deck.cam;
    var iz = interpolateZoom([from.cx, from.cy, sw / from.scale], [t.cx, t.cy, sw / t.scale]);
    var dur = Math.max(320, Math.min(iz.duration * 0.85, 820)), start = performance.now();
    (function frame(now) {
      var k = dur > 0 ? Math.min(1, (now - start) / dur) : 1;
      var p = iz(k);
      applyCam(p[0], p[1], sw / p[2], 'instant');
      deck.flyRAF = k < 1 ? requestAnimationFrame(frame) : null;
    })(start);
  }
  // setCamera: snap (animate falsy), CSS-tween a small move, or van Wijk-Nuij-fly a
  // big one (overview enter/exit, long jumps), respecting reduced-motion.
  function setCamera(animate) {
    var t = cameraTarget();
    if (animate && deck.cam && !reducedMotion() && bigChange(deck.cam, t)) flyTo(t);
    else {
      if (deck.flyRAF) { cancelAnimationFrame(deck.flyRAF); deck.flyRAF = null; }
      applyCam(t.cx, t.cy, t.scale, animate ? 'css' : 'instant');
    }
  }
  function layout() {
    if (!slidesEl()) return;
    positionGrid();
    applyBackgrounds();
    buildLodCards(); // semantic-zoom title cards (shown when zoomed far out)
    buildMinimap(); // overview+detail minimap (shown when zoomed beyond fit)
    buildOverviewSearch(); // the overview filter box
    allSlides().forEach(fitSlide); // all slides are laid out now, not just the current one
    if (deck.overview) fitOverview(); // viewport changed: re-fit the map
    setCamera(false);
  }

  // Off-camera slides stay in the DOM (the camera just frames the current cell), but for
  // assistive tech + the tab order that means every non-visible slide is still reachable.
  // `inert` removes a leaf from the AT tree AND tab order (and blocks its clicks) in one
  // attribute, so a screen-reader/keyboard user only meets the current slide in step mode.
  // The single source of truth: in overview / scroll(reader) / print every slide is meant
  // to be readable, so inert is cleared from all of them; otherwise only the current leaf
  // is non-inert. Called from applyClasses (commit + init), the mode enter/exit hooks, and
  // setOverview, so any path that changes "what's visible" re-derives inert consistently.
  function syncInert() {
    var showAll = deck.overview || deck.scroll ||
      document.documentElement.classList.contains('tali-print');
    var cur = showAll ? null : currentSlide();
    allSlides().forEach(function (s) {
      if (showAll || s === cur) s.removeAttribute('inert');
      else s.setAttribute('inert', '');
    });
  }

  // --- the non-camera part of a slide change -----------------------------
  // Fragment visibility, chrome, and the annotation redraw. Split out so
  // auto-animate can update these without moving the camera. Per-slide visibility
  // is the camera transform itself: every slide is laid out into its grid cell and
  // the camera frames the current one (no per-slide show/hide class needed).
  function applyClasses() {
    applyFragments();
    if (deck.draw) redrawAnnotations(); // restore the new slide's annotations
    updateChrome(); // progress bar / menu state follow the current slide
    syncInert(); // keep off-camera slides out of the AT tree + tab order (step mode)
    deck.lastSlide = currentSlide(); // remember for the next auto-animate transition
  }
  function apply() {
    applyClasses();
    setCamera(true); // pan/zoom the camera to the current cell (the transition)
  }
  // --- per-slide backgrounds ---------------------------------------------
  // Each slide carries its own `data-background-*` as a layer behind its content,
  // so the background travels with the slide as the camera pans, and shows per-tile
  // in overview. `.tali-dark-bg` on the section flips its own text light over a dark
  // / image / gradient background. Set once per layout (the attributes are static).
  function ensureSlideBg(sec) {
    var bg = sec.querySelector(':scope > .tali-slide-bg');
    if (!bg) {
      bg = document.createElement('div');
      bg.className = 'tali-slide-bg';
      sec.insertBefore(bg, sec.firstChild);
    }
    return bg;
  }
  function applyBackgrounds() {
    allSlides().forEach(function (sec) {
      var color = sec.getAttribute('data-background-color');
      var gradient = sec.getAttribute('data-background-gradient');
      var image = sec.getAttribute('data-background-image');
      sec.classList.remove('tali-dark-bg');
      var existing = sec.querySelector(':scope > .tali-slide-bg');
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
      if (image || gradient || (color && isDarkColor(color))) sec.classList.add('tali-dark-bg');
    });
  }
  // --- semantic zoom (level-of-detail) -----------------------------------
  // When the overview is zoomed out far enough that full slide content is an
  // illegible smudge, each tile collapses to a clean title card (shown by the
  // `.tali-lod-far` class that setCamera toggles on tile on-screen size). The real
  // content stays in the DOM, just faded, so live state is never lost. Cards are
  // built once and their title/number refreshed on each layout.
  function buildLodCards() {
    allSlides().forEach(function (sec, i) {
      var card = sec.querySelector(':scope > .tali-lod');
      if (!card) {
        card = document.createElement('div');
        card.className = 'tali-lod';
        card.setAttribute('aria-hidden', 'true'); // decorative overview title card; the real heading stays in the AT tree
        card.innerHTML = '<div class="tali-lod-title"></div><div class="tali-lod-num"></div>';
        sec.appendChild(card);
      }
      var h = sec.querySelector('h1, h2, h3, h4, h5, h6');
      var title = h ? h.textContent.trim() : (sec.textContent || '').trim().split('\n')[0].slice(0, 80);
      card.firstChild.textContent = title;
      card.lastChild.textContent = String(i + 1);
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
    var scale = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--tali-deck-scale')) || 1;
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
    setTimeout(function () { to.classList.remove('tali-aa'); }, 520);
  }
  // Auto-animate in the camera model: instead of panning between the two cells, hold
  // the camera and overlay `to` on `from`'s cell so the matched elements morph in
  // place; then snap `to` and the camera to `to`'s real cell together — a net-zero
  // screen move, so the reposition is invisible.
  function autoAnimateTo(from, to) {
    var toTransform = to.style.transform;       // to's real grid cell
    to.style.transform = from.style.transform;  // overlap `to` onto `from`'s cell
    to.classList.add('tali-aa');
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
  // --- fragments (incremental steps) -----------------------------------
  // A fragment is any `.fragment` element or a list item inside `.incremental`,
  // in document order. They start hidden (via visibility, so layout + shrink-to-
  // fit are unaffected) and show one per forward step before the slide advances.
  // A slide's ordered "steps": each `.fragment`/`.incremental` item is a step
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
        // A `<pre>` INSIDE a `.magic-move` is one of its morph blocks, already counted
        // by the `.magic-move` branch above — don't double-count it as its own step.
        if (node.closest('.magic-move')) return;
        // A code-step pre that also follows a `. . .` pause carries `.fragment`;
        // give it a fragment step first (else it stays visibility:hidden for the
        // whole talk), then its per-segment line-highlight steps.
        if (node.classList.contains('fragment')) steps.push({ frag: node });
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
    slide.querySelectorAll(FRAG_SEL).forEach(function (el) { el.classList.remove('tali-frag-visible'); });
    var mmCount = new Map();
    slide.querySelectorAll('.magic-move').forEach(function (d) { mmCount.set(d, 0); });
    // then apply each taken step in order (later code steps overwrite earlier)
    for (var i = 0; i < deck.frag; i++) {
      var s = steps[i];
      if (s.frag) s.frag.classList.add('tali-frag-visible');
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
    else pres.forEach(function (p, i) { p.classList.toggle('tali-mm-active', i === target); });
    div.__mm = target;
  }
  function morphMM(div, pres, from, to) {
    var blockFrom = pres[from], blockTo = pres[to];
    var scale = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--tali-deck-scale')) || 1;
    var byText = {};
    Array.prototype.forEach.call(blockFrom.querySelectorAll('.tali-hl-ln'), function (l) {
      (byText[lineText(l)] || (byText[lineText(l)] = [])).push(l);
    });
    blockTo.classList.add('tali-mm-active');
    blockFrom.classList.remove('tali-mm-active'); // fades out (CSS opacity transition)
    Array.prototype.forEach.call(blockTo.querySelectorAll('.tali-hl-ln'), function (lt) {
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
    var lines = pre.querySelectorAll('.tali-hl-ln');
    spec = (spec || '').trim();
    if (!spec || spec === 'all') {
      pre.classList.remove('tali-hl-lines-active');
      lines.forEach(function (l) { l.classList.remove('tali-hl-ln-hl'); });
      return;
    }
    var on = parseLineSpec(spec);
    pre.classList.add('tali-hl-lines-active');
    lines.forEach(function (l, i) { l.classList.toggle('tali-hl-ln-hl', on.has(i + 1)); });
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
  // preview) follows the fragment step, not just slide changes.
  function fragChanged() {
    deck.animSteps = true; // an in-slide step: let magic-move morph (vs. set on slide entry)
    if (deck.mode === 'speaker') updateSpeakerUI();
    else applyFragments();
    deck.animSteps = false;
    writeHash(); // reflect the in-slide step in the URL so a fragment is deep-linkable
    broadcastState();
  }
  function showNextFrag() {
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
    // Iterate to convergence: a single pass under-shrinks when a slide holds a
    // fixed-size element (a chart image doesn't scale with font-size), leaving the
    // content tight against the bottom. The 0.95 leaves a small bottom margin.
    var size = BASE;
    for (var i = 0; i < 4; i++) {
      var fh = sec.scrollHeight > sec.clientHeight ? sec.clientHeight / sec.scrollHeight : 1;
      var fw = sec.scrollWidth > sec.clientWidth ? sec.clientWidth / sec.scrollWidth : 1;
      var f = Math.min(fh, fw);
      if (f >= 1) break; // fits (with margin)
      size = Math.max(12, size * f * 0.95);
      sec.style.fontSize = size.toFixed(2) + 'px';
    }
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
  // Move to a slide. `showAll` shows all its fragments (a backward step or a
  // jump lands on a complete slide); otherwise they start hidden (forward entry).
  function moveTo(h, v, showAll) {
    deck.h = h; deck.v = v;
    clampIndices();
    deck.frag = showAll ? fragCount() : 0;
    commit();
  }
  // Forward steps show the next fragment first, then advance; backward steps
  // hide the last fragment first, then retreat (landing fully shown).
  // Arrow keys map to the visual grid: left/right step through the current topic
  // (flowing on to the next topic at its ends, i.e. linear order); up/down jump
  // straight to the topic above/below, keeping the column.
  function right() { next(); }
  function left() { prev(); }
  function down() { moveTopic(1); }
  function up() { moveTopic(-1); }
  function moveTopic(d) {
    var p = posOf(deck.h, deck.v), r = p.row + d;
    if (r < 0 || r >= p.rows.length) return;
    var rowArr = p.rows[r], cell = rowArr[Math.min(p.col, rowArr.length - 1)];
    moveTo(cell.h, cell.v, true);
  }
  // Linear next/prev: fragments first, then flow down a stack, then across.
  function next() {
    if (showNextFrag()) return;
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
    allSlides().forEach(function (s) { s.classList.toggle('tali-overview-current', s === cur); });
    markMinimapCurrent();
  }
  function fitOverview() {
    var rev = deckEl(); if (!rev) return;
    var W = deck.config.width, H = deck.config.height;
    var sw = rev.clientWidth || window.innerWidth, sh = rev.clientHeight || window.innerHeight;
    var g = gridDims(), gw = g.cols * W, gh = g.rows * H;
    var fit = Math.min(sw / gw, sh / gh) * (1 - 2 * 0.06); // whole map, with margin
    if (!(fit > 0)) fit = 1;
    deck.ov = { scale: fit, cx: gw / 2, cy: gh / 2, fit: fit };
  }
  function ovStage() {
    var rev = deckEl();
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
    // Disambiguate input: a trackpad pinch arrives as ctrlKey+wheel (zoom toward the
    // cursor); a mouse wheel is a large vertical-only notch (zoom); a trackpad
    // two-finger scroll (horizontal component, or small pixel deltas) pans the map.
    var pinch = e.ctrlKey;
    var mouseWheel = !pinch && (e.deltaMode !== 0 || (Math.abs(e.deltaY) >= 100 && e.deltaX === 0));
    if (!pinch && !mouseWheel) { // trackpad pan
      deck.ov.cx += e.deltaX / deck.ov.scale;
      deck.ov.cy += e.deltaY / deck.ov.scale;
      clampOv();
      setCamera(false);
      return;
    }
    var st = ovStage(), rev = deckEl(), r = rev.getBoundingClientRect();
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
  // Pressing anywhere on the overview pans the map.
  function onOverviewPointerDown(e) {
    if (!deck.overview || !deck.ov || e.button !== 0) return;
    ovDrag = { x: e.clientX, y: e.clientY, cx: deck.ov.cx, cy: deck.ov.cy, moved: false };
  }
  function onOverviewPointerMove(e) {
    if (!ovDrag) return;
    var dx = e.clientX - ovDrag.x, dy = e.clientY - ovDrag.y;
    if (!ovDrag.moved && dx * dx + dy * dy < 25) return;    // 5px before it counts as a drag
    ovDrag.moved = true;
    deckEl().classList.add('tali-ov-panning');
    deck.ov.cx = ovDrag.cx - dx / deck.ov.scale;
    deck.ov.cy = ovDrag.cy - dy / deck.ov.scale;
    clampOv();
    setCamera(false);
  }
  function onOverviewPointerUp() {
    var rev = deckEl();
    if (ovDrag && ovDrag.moved) deck.ovDragged = true;      // a pan: swallow the click that follows
    ovDrag = null;
    if (rev) rev.classList.remove('tali-ov-panning');
  }
  // Pan (if needed) so the highlighted tile stays comfortably on-screen; keep zoom.
  function ensureCurrentTileVisible(animate) {
    if (!deck.overview || !deck.ov) return;
    var st = ovStage(), W = deck.config.width, H = deck.config.height;
    var p = posOf(deck.h, deck.v);
    var wx = p.col * W + W / 2, wy = p.row * H + H / 2;
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
    var rev = deckEl();
    if (!rev) return;
    deck.overview = on;
    rev.classList.toggle('overview', on);
    if (on && deck.blackout) toggleBlackout(false); // can't navigate a map you can't see
    if (on && deck.draw && deck.draw.on) { deck.draw.on = false; rev.classList.remove('tali-drawing'); }
    if (on) { fitOverview(); markCurrentTile(); }
    else { deck.ov = null; clearFilter(); allSlides().forEach(function (s) { s.classList.remove('tali-overview-current'); }); }
    syncInert(); // overview: every tile is browsable, so clear inert; exiting re-inerts off-camera
    positionGrid(); // add (or remove) the per-tile gutter shrink
    setCamera(true); // zoom out to the map, or back into the current cell
  }
  // Move the overview highlight one leaf forward/back in deck order, keeping it
  // on-screen as the map pans.
  function moveHighlight(dCol, dRow) {
    var p = posOf(deck.h, deck.v), rows = p.rows, r = p.row, c = p.col;
    if (dRow) { r = Math.max(0, Math.min(r + dRow, rows.length - 1)); c = Math.min(c, rows[r].length - 1); }
    if (dCol) { c = Math.max(0, Math.min(c + dCol, rows[r].length - 1)); }
    var cell = rows[r][c];
    deck.h = cell.h; deck.v = cell.v;
    markCurrentTile();
    ensureCurrentTileVisible(true);
  }
  function onSlidesClick(e) {
    if (!deck.overview) return;
    if (deck.ovDragged) { deck.ovDragged = false; return; } // that was a pan, not a pick
    var sec = e.target.closest && e.target.closest('.tali-deck .tali-slides section');
    if (!sec) return;
    e.preventDefault();
    var T = tops();
    for (var h = 0; h < T.length; h++) {
      var v = vertsOf(T[h]).indexOf(sec);
      if (sec === T[h] || v >= 0) { setOverview(false); moveTo(h, v < 0 ? 0 : v, true); return; }
    }
    setOverview(false);
  }

  // --- minimap (overview+detail) -----------------------------------------
  // When the map is zoomed beyond fit, a corner minimap shows the whole deck as a
  // schematic of tiles plus a rectangle for the current view; click/drag it to fly
  // the camera. (Cockburn/Karlson/Bederson: pan+zoom WITH an overview is the most
  // efficient navigation technique.) Rebuilt on layout; the view rect tracks setCamera.
  function buildMinimap() {
    var rev = deckEl(); if (!rev) return;
    var mm = rev.querySelector(':scope > .tali-minimap'), inner;
    if (!mm) {
      mm = document.createElement('div');
      mm.className = 'tali-minimap';
      mm.setAttribute('aria-hidden', 'true'); // decorative overview map; navigation is keyboard-driven
      mm.innerHTML = '<div class="tali-minimap-inner"><div class="tali-mini-view"></div></div>';
      rev.appendChild(mm);
      inner = mm.firstChild;
      var dragging = false;
      var fly = function (e) {
        if (!deck.ov || !deck.mini) return;
        var r = inner.getBoundingClientRect();
        deck.ov.cx = (e.clientX - r.left) / deck.mini.scale;
        deck.ov.cy = (e.clientY - r.top) / deck.mini.scale;
        clampOv(); setCamera(false);
      };
      inner.addEventListener('pointerdown', function (e) { dragging = true; fly(e); e.preventDefault(); e.stopPropagation(); });
      window.addEventListener('pointermove', function (e) { if (dragging) fly(e); });
      window.addEventListener('pointerup', function () { dragging = false; });
    }
    inner = mm.firstChild;
    var W = deck.config.width, H = deck.config.height;
    var rows = gridRows(), gd = gridDims(), gw = gd.cols * W, gh = gd.rows * H;
    var ms = Math.min(232 / gw, 150 / gh);
    deck.mini = { scale: ms };
    inner.style.width = (gw * ms) + 'px';
    inner.style.height = (gh * ms) + 'px';
    var view = inner.querySelector('.tali-mini-view');
    Array.prototype.slice.call(inner.querySelectorAll('.tali-mini-tile')).forEach(function (t) { t.remove(); });
    rows.forEach(function (rowArr, r) {
      rowArr.forEach(function (cell, c) {
        var t = document.createElement('div');
        t.className = 'tali-mini-tile';
        t.style.cssText = 'left:' + (c * W * ms) + 'px;top:' + (r * H * ms) + 'px;width:' + (W * ms - 2) + 'px;height:' + (H * ms - 2) + 'px';
        t.dataset.h = cell.h; t.dataset.v = cell.v;
        inner.insertBefore(t, view);
      });
    });
    markMinimapCurrent();
  }
  function markMinimapCurrent() {
    var rev = deckEl(); if (!rev) return;
    var mm = rev.querySelector(':scope > .tali-minimap'); if (!mm) return;
    Array.prototype.forEach.call(mm.querySelectorAll('.tali-mini-tile'), function (t) {
      t.classList.toggle('tali-mini-cur', +t.dataset.h === deck.h && +t.dataset.v === deck.v);
    });
  }
  // Position the "current view" rectangle; show the minimap only when zoomed beyond fit
  // (otherwise the whole deck is already on screen and it would be redundant).
  function updateMinimapView() {
    var rev = deckEl(); if (!rev) return;
    var mm = rev.querySelector(':scope > .tali-minimap'); if (!mm || !deck.mini) return;
    var show = deck.overview && deck.ov && deck.ov.scale > deck.ov.fit * 1.12;
    mm.classList.toggle('tali-minimap-on', show);
    if (!show) return;
    var st = ovStage(), ms = deck.mini.scale;
    var vw = st.sw / deck.ov.scale, vh = st.sh / deck.ov.scale;
    var view = mm.querySelector('.tali-mini-view');
    view.style.cssText = 'left:' + ((deck.ov.cx - vw / 2) * ms) + 'px;top:' + ((deck.ov.cy - vh / 2) * ms) + 'px;width:' + (vw * ms) + 'px;height:' + (vh * ms) + 'px';
  }

  // --- overview filter (Shneiderman's "filter" leg) ----------------------
  // Press `/` in the overview to filter slides by title: non-matches dim, matches get
  // an accent ring, Enter jumps to the first match. Type -> locate -> dive in.
  function buildOverviewSearch() {
    var rev = deckEl(); if (!rev || rev.querySelector(':scope > .tali-ov-search')) return;
    var box = document.createElement('input');
    box.className = 'tali-ov-search';
    box.type = 'text';
    box.setAttribute('placeholder', 'Filter slides…  ( / focus · ↵ jump )');
    box.addEventListener('input', function () { filterTiles(box.value); });
    box.addEventListener('keydown', function (e) {
      e.stopPropagation(); // typing must not drive the deck
      if (e.key === 'Enter') jumpToFirstMatch();
      else if (e.key === 'Escape') { box.value = ''; filterTiles(''); box.blur(); }
    });
    rev.appendChild(box);
  }
  function leafAt(h, v) {
    var top = tops()[h];
    return top ? (isStack(top) ? vertsOf(top)[v] : top) : null;
  }
  function filterTiles(q) {
    q = (q || '').trim().toLowerCase();
    deck.ovQuery = q;
    var rev = deckEl();
    allSlides().forEach(function (sec) {
      var h = sec.querySelector('h1, h2, h3, h4, h5, h6');
      var title = (h ? h.textContent : sec.textContent || '').toLowerCase();
      var hit = !!q && title.indexOf(q) >= 0;
      sec.classList.toggle('tali-ov-dim', !!q && !hit);
      sec.classList.toggle('tali-ov-hit', hit);
    });
    if (rev) rev.classList.toggle('tali-ov-filtering', !!q); // dims the threads via CSS
    var mm = rev && rev.querySelector(':scope > .tali-minimap'); // mirror onto the minimap
    if (mm) Array.prototype.forEach.call(mm.querySelectorAll('.tali-mini-tile'), function (t) {
      var leaf = leafAt(+t.dataset.h, +t.dataset.v);
      t.classList.toggle('tali-mini-dim', !!leaf && leaf.classList.contains('tali-ov-dim'));
      t.classList.toggle('tali-mini-hit', !!leaf && leaf.classList.contains('tali-ov-hit'));
    });
  }
  function clearFilter() {
    var rev = deckEl(); if (!rev) return;
    var box = rev.querySelector(':scope > .tali-ov-search');
    if (box) box.value = '';
    filterTiles('');
  }
  function jumpToFirstMatch() {
    var T = tops();
    for (var h = 0; h < T.length; h++) {
      var leaves = vertsOf(T[h]);
      for (var v = 0; v < leaves.length; v++) {
        if (leaves[v].classList.contains('tali-ov-hit')) {
          deck.h = h; deck.v = v; markCurrentTile(); ensureCurrentTileVisible(true); return;
        }
      }
    }
  }

  // --- presenter mode + cross-window sync --------------------------------
  // `s` opens a speaker window (a popup at ?qmd=speaker). It shows the current +
  // next slide as live previews (same-origin iframes at ?qmd=embed), the slide's
  // speaker notes (`::: {.notes}`), and a timer + clock. Audience and speaker stay
  // in sync via opener<->popup postMessage (works on file://); either can drive.
  function withQmd(url, val) { return url + (url.indexOf('?') >= 0 ? '&' : '?') + 'qmd=' + val; }
  function deckBaseUrl() { return location.href.split('#')[0].split('?')[0]; }
  // Only accept/sync with windows of our own origin, so a third-party page that
  // embeds the deck can't drive it (or read its slide position). file:// has no
  // real origin ("" / "null"), so allow it there. When posting, target our origin
  // on http(s) and fall back to '*' on file:// (a "null" targetOrigin would throw).
  function sameOrigin(e) { return e.origin === location.origin || e.origin === '' || e.origin === 'null'; }
  function targetOrigin() { return (location.origin && location.origin !== 'null') ? location.origin : '*'; }

  // Apply a position received from the other window (or, in an embed iframe, from
  // the speaker). Never re-broadcasts, so there is no echo loop.
  function applyRemote(h, v, frag) {
    if (deck.blackout) toggleBlackout(false); // an external slide change lifts the curtain
    deck.h = h; deck.v = v;
    clampIndices();
    deck.frag = (frag == null) ? fragCount() : frag;
    if (deck.mode === 'speaker') updateSpeakerUI();
    else { apply(); updateNumber(); writeHash(); }
    fire('slidechanged');
  }
  function broadcastState() {
    var msg = { qmd: 'deck', type: 'state', h: deck.h, v: deck.v, frag: deck.frag };
    var t = targetOrigin();
    if (deck.speakerWin && !deck.speakerWin.closed) { try { deck.speakerWin.postMessage(msg, t); } catch (e) {} }
    if (window.opener && !window.opener.closed) { try { window.opener.postMessage(msg, t); } catch (e) {} }
  }
  function onMessage(e) {
    if (!sameOrigin(e)) return; // ignore cross-origin drivers
    var d = e.data;
    if (!d || d.qmd !== 'deck') return;
    if (d.type === 'goto' || d.type === 'state') applyRemote(d.h, d.v, d.frag);
    else if (d.type === 'hello') broadcastState(); // a freshly-opened speaker asks for our position
  }
  function openSpeaker() {
    if (deck.mode !== 'normal') return;
    if (deck.speakerWin && !deck.speakerWin.closed) { deck.speakerWin.focus(); return; }
    deck.speakerWin = window.open(withQmd(deckBaseUrl(), 'speaker'), 'tali-speaker', 'width=1180,height=760');
  }
  function nextIndex(h, v) {
    var T = tops(), top = T[h];
    if (top && isStack(top) && v < vertsOf(top).length - 1) return { h: h, v: v + 1 };
    if (h < T.length - 1) return { h: h + 1, v: 0 };
    return null;
  }
  function postFrame(frame, h, v, frag) {
    if (frame && frame.contentWindow) {
      try { frame.contentWindow.postMessage({ qmd: 'deck', type: 'goto', h: h, v: v, frag: frag }, targetOrigin()); } catch (e) {}
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
    var t = document.querySelector('.tali-speaker .sp-timer');
    var c = document.querySelector('.tali-speaker .sp-clock');
    if (t) { var s = Math.max(0, Math.floor((Date.now() - deck.spStart) / 1000)); t.textContent = Math.floor(s / 60) + ':' + ('0' + (s % 60)).slice(-2); }
    if (c) c.textContent = new Date().toLocaleTimeString();
  }
  function initSpeaker() {
    document.title = 'Speaker · ' + document.title;
    var rev = deckEl(); if (rev) rev.style.display = 'none'; // keep as data source for notes/counts
    var root = document.createElement('div');
    root.className = 'tali-speaker';
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
    if (deck.spClock) clearInterval(deck.spClock); // don't stack intervals on re-init
    deck.spClock = setInterval(updateSpeakerClock, 500);
    // Stop the clock when the speaker window is closed / navigated away (a bfcache
    // restore re-runs initSpeaker, which re-arms it) so the interval can't leak.
    window.addEventListener('pagehide', function () {
      if (deck.spClock) { clearInterval(deck.spClock); deck.spClock = null; }
    });
    updateSpeakerClock();
    clampIndices();
    if (window.opener) { try { window.opener.postMessage({ qmd: 'deck', type: 'hello' }, targetOrigin()); } catch (e) {} }
  }

  // --- PDF export (print) ------------------------------------------------
  // On `beforeprint` (Cmd/Ctrl+P) the deck flattens to one slide per page: every
  // slide shown, transforms dropped, all fragments shown, code un-dimmed.
  // `@page` makes each page the deck's aspect, so the browser's "Save as PDF"
  // yields a clean handout. `?qmd=print` enters the same layout on screen.
  function enterPrint() {
    var rev = deckEl(); if (!rev) return;
    document.documentElement.classList.add('tali-print');
    rev.classList.remove('tali-dark-bg'); // bg layer is hidden in print; keep text readable on the page
    tops().forEach(function (top) {
      top.style.removeProperty('display'); top.removeAttribute('aria-hidden');
      if (isStack(top)) {
        top.classList.add('tali-print-stack');
        vertsOf(top).forEach(function (s) { s.style.removeProperty('display'); s.removeAttribute('aria-hidden'); });
      }
    });
    rev.querySelectorAll(FRAG_SEL).forEach(function (e) { e.classList.add('tali-frag-visible'); });
    rev.querySelectorAll('pre[data-code-lines]').forEach(function (p) { highlightLines(p, 'all'); });
    rev.querySelectorAll('.magic-move').forEach(function (div) { // show the final block
      var pres = mmBlocks(div);
      pres.forEach(function (p, i) { p.classList.toggle('tali-mm-active', i === pres.length - 1); });
    });
    allSlides().forEach(fitSlide); // size every slide to its page (not just visited ones)
    syncInert(); // print shows every slide: clear inert so all pages are readable
  }
  function exitPrint() {
    document.documentElement.classList.remove('tali-print');
    tops().forEach(function (t) { t.classList.remove('tali-print-stack'); });
    apply(); // -> applyClasses -> syncInert re-inerts the off-camera slides
  }

  // --- scroll / reader mode ----------------------------------------------
  // On a narrow/portrait screen (or ?qmd=scroll) the fixed-aspect deck would
  // letterbox badly, so it flattens to a vertically-scrollable, readable document:
  // every slide stacked full-width at a responsive size, all fragments shown.
  function enterScroll() {
    var rev = deckEl();
    if (!rev || deck.scroll) return;
    deck.scroll = true;
    document.documentElement.classList.add('tali-scroll');
    rev.classList.remove('tali-dark-bg'); // backgrounds are hidden in reader; keep text readable
    tops().forEach(function (top) {
      top.style.removeProperty('display');
      top.style.removeProperty('font-size');
      top.removeAttribute('aria-hidden');
      if (isStack(top)) {
        top.classList.add('tali-scroll-stack');
        vertsOf(top).forEach(function (s) { s.style.removeProperty('display'); s.style.removeProperty('font-size'); });
      }
    });
    rev.querySelectorAll(FRAG_SEL).forEach(function (e) { e.classList.add('tali-frag-visible'); });
    rev.querySelectorAll('pre[data-code-lines]').forEach(function (p) { highlightLines(p, 'all'); });
    rev.querySelectorAll('.magic-move').forEach(function (div) {
      var pres = mmBlocks(div);
      pres.forEach(function (p, i) { p.classList.toggle('tali-mm-active', i === pres.length - 1); });
    });
    syncInert(); // reader mode stacks every slide for reading: clear inert from all of them
  }
  function exitScroll() {
    if (!deck.scroll) return;
    deck.scroll = false;
    document.documentElement.classList.remove('tali-scroll');
    tops().forEach(function (t) { t.classList.remove('tali-scroll-stack'); });
    apply(); // -> applyClasses -> syncInert re-inerts the off-camera slides
    layout();
  }

  // --- drawing / annotations ---------------------------------------------
  // `d` toggles a pen: a canvas inside `.tali-slides` (so it scales with the deck) that
  // captures pointer strokes over the current slide. Strokes are kept per slide and
  // redrawn on navigation. A small toolbar offers colours, an eraser and clear.
  function ensureDraw() {
    if (deck.draw) return deck.draw;
    var canvas = document.createElement('canvas');
    canvas.className = 'tali-draw';
    canvas.width = deck.config.width;
    canvas.height = deck.config.height;
    slidesEl().appendChild(canvas);
    var bar = document.createElement('div');
    bar.className = 'tali-draw-bar';
    bar.innerHTML =
      '<button class="tali-draw-color" data-c="#ef4444" style="background:#ef4444"></button>' +
      '<button class="tali-draw-color" data-c="#3b82f6" style="background:#3b82f6"></button>' +
      '<button class="tali-draw-color" data-c="#22c55e" style="background:#22c55e"></button>' +
      '<button class="tali-draw-erase" title="Erase">erase</button>' +
      '<button class="tali-draw-clear" title="Clear slide">clear</button>' +
      '<button class="tali-draw-done" title="Done (d)">done</button>';
    deckEl().appendChild(bar);
    var d = deck.draw = {
      canvas: canvas, ctx: canvas.getContext('2d'), bar: bar,
      color: '#ef4444', erase: false, on: false, strokes: {}, drawing: false, stroke: null,
    };
    bar.querySelectorAll('.tali-draw-color').forEach(function (b) {
      b.addEventListener('click', function () { d.color = b.getAttribute('data-c'); d.erase = false; updateDrawBar(); });
    });
    bar.querySelector('.tali-draw-erase').addEventListener('click', function () { d.erase = !d.erase; updateDrawBar(); });
    bar.querySelector('.tali-draw-clear').addEventListener('click', clearSlideDrawing);
    bar.querySelector('.tali-draw-done').addEventListener('click', function () { toggleDraw(false); });
    canvas.addEventListener('pointerdown', drawStart);
    canvas.addEventListener('pointermove', drawMove);
    window.addEventListener('pointerup', function () { if (deck.draw) deck.draw.drawing = false; });
    return d;
  }
  function updateDrawBar() {
    var d = deck.draw; if (!d) return;
    d.bar.querySelectorAll('.tali-draw-color').forEach(function (b) {
      b.classList.toggle('sel', !d.erase && b.getAttribute('data-c') === d.color);
    });
    d.bar.querySelector('.tali-draw-erase').classList.toggle('sel', d.erase);
  }
  function toggleDraw(force) {
    if (deck.mode !== 'normal' || deck.scroll || deck.overview) return;
    var d = ensureDraw();
    d.on = (force == null) ? !d.on : force;
    deckEl().classList.toggle('tali-drawing', d.on);
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
    // #/<slide>[/<v>]/<frag>: append the in-slide step index when past step 0 so a deep
    // link restores the exact fragment. A numeric slide includes its `v` when a frag
    // follows (keeping the frag slot unambiguous); a named slide takes `/<frag>`.
    var parts = c && c.id ? [c.id] : [deck.h];
    if (!(c && c.id) && (deck.v || deck.frag > 0)) parts.push(deck.v);
    if (deck.frag > 0) parts.push(deck.frag);
    var frag = parts.join('/');
    var url = '#/' + frag;
    if (url === location.hash) return;
    if (deck.config.history) location.hash = '/' + frag;
    else history.replaceState(null, '', url);
  }
  function readHash() {
    var raw = location.hash.replace(/^#\/?/, '');
    if (!raw) return false;
    var parts = raw.split('/');
    var fragPart;
    if (parts[0] && isNaN(parseInt(parts[0], 10))) {
      var el = document.getElementById(parts[0]);
      if (!el) return false;
      var ix = indexOf(el);
      deck.h = ix.h; deck.v = ix.v;
      fragPart = parts[1]; // named slide: the fragment index follows the id
    } else {
      deck.h = parseInt(parts[0], 10) || 0;
      deck.v = parseInt(parts[1], 10) || 0;
      fragPart = parts[2];
    }
    var f = parseInt(fragPart, 10);
    deck.pendingFrag = isNaN(f) ? null : f; // consumed by onHashChange / init
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
    var ph = deck.h, pv = deck.v, pf = deck.frag;
    if (!readHash()) return;
    clampIndices();
    var target = deck.pendingFrag;
    // Our own writeHash echo (history mode fires hashchange): nothing actually moved.
    if (deck.h === ph && deck.v === pv && (target == null || target === pf)) return;
    if (deck.blackout) toggleBlackout(false); // an external slide change lifts the curtain
    var fc = fragCount();
    // Restore the linked fragment step; without one (a plain slide link) show them all.
    deck.frag = target != null ? Math.max(0, Math.min(target, fc)) : fc;
    apply(); updateNumber(); fire('slidechanged'); // apply pans the camera
    broadcastState(); // keep a speaker/embed window in sync on hash (back/forward) nav
  }

  // --- slide number -------------------------------------------------------
  function updateNumber() {
    updateSlideLabels(); // keep the per-slide aria-labels + live announcement current
    if (!deck.config.slideNumber) return;
    var rev = deckEl();
    if (!rev) return;
    var el = rev.querySelector('.tali-slide-number');
    if (!el) { el = document.createElement('div'); el.className = 'tali-slide-number'; rev.appendChild(el); }
    var all = allSlides();
    el.textContent = (all.indexOf(currentSlide()) + 1) + ' / ' + all.length;
  }

  // a11y: name each leaf slide "Slide N of M" (the server-side <section> already carries
  // role="group" + aria-roledescription="slide", but only JS knows the flat order across
  // vertical stacks), and announce the current slide through a polite live region so a
  // screen-reader user hears the position change on every navigation. Re-run on every
  // slide change + after a live edit re-splits the deck, so the count stays right.
  function updateSlideLabels() {
    var rev = deckEl();
    if (!rev) return;
    var all = allSlides(), cur = currentSlide(), idx = all.indexOf(cur);
    for (var i = 0; i < all.length; i++) {
      all[i].setAttribute('aria-label', 'Slide ' + (i + 1) + ' of ' + all.length);
    }
    var live = rev.querySelector('.tali-deck-live');
    if (!live) {
      live = document.createElement('div');
      live.className = 'tali-deck-live';
      live.setAttribute('aria-live', 'polite');
      live.setAttribute('aria-atomic', 'true');
      rev.appendChild(live);
    }
    if (idx >= 0) {
      var hd = cur && cur.querySelector('h1,h2,h3');
      var title = hd ? hd.textContent.trim() : '';
      live.textContent = 'Slide ' + (idx + 1) + ' of ' + all.length + (title ? ': ' + title : '');
    }
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
        case 'ArrowRight': moveHighlight(1, 0); break;
        case 'ArrowLeft': moveHighlight(-1, 0); break;
        case 'ArrowDown': moveHighlight(0, 1); break;
        case 'ArrowUp': moveHighlight(0, -1); break;
        case '0': fitOverview(); setCamera(true); break; // re-fit the whole map
        case '/': { var b = deckEl().querySelector(':scope > .tali-ov-search'); if (b) b.focus(); break; } // filter
        default: handled = false;
      }
      if (handled) e.preventDefault();
      return;
    }
    // Black-screen / pause: while blacked out, any navigation key (plus b / . / Esc)
    // RESUMES — it lifts the curtain without also advancing, so a presenter tapping a
    // clicker to continue is met by the slide, not a jump. All other keys are swallowed.
    if (deck.blackout) {
      var RESUME = [' ', 'PageDown', 'PageUp', 'ArrowRight', 'ArrowLeft', 'ArrowDown',
        'ArrowUp', 'Home', 'End', 'Enter', 'b', '.', 'Escape'];
      if (RESUME.indexOf(e.key) !== -1) { toggleBlackout(false); e.preventDefault(); }
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
      case 'End': {
        // The very last slide: the last vertical of the last stack, not v=0.
        var lh = tops().length - 1, lt = tops()[lh];
        moveTo(lh, isStack(lt) ? vertsOf(lt).length - 1 : 0, true);
        break;
      }
      case 'Escape': case 'o': if (deck.mode === 'normal') setOverview(true); break;
      case 's': openSpeaker(); break;
      case 'd': toggleDraw(); break;
      case 'f': toggleFullscreen(); break;
      case 'b': case '.': toggleBlackout(true); break;
      case 'm': case '?': toggleMenu(); break;
      default: handled = false;
    }
    if (handled) e.preventDefault();
  }
  // Black the whole viewport (pull attention back to the speaker). The overlay is a
  // body-level element (not a `.tali-deck` child) so it escapes `.tali-deck`'s stacking
  // context and covers ALL chrome — including the preview dev menu at z-9999. Keys
  // are gated in onKey; a tap dismisses it where there's no Esc/B (touch).
  function toggleBlackout(on) {
    var rev = deckEl();
    deck.blackout = !!on;
    if (rev) rev.classList.toggle('tali-blackout', deck.blackout);
    if (deck.blackout) {
      // Blackout means eyes on the speaker — drop drawing too (mirrors setOverview).
      if (deck.draw && deck.draw.on) { deck.draw.on = false; if (rev) rev.classList.remove('tali-drawing'); }
      if (!deck.blackoutEl) {
        deck.blackoutEl = document.createElement('div');
        deck.blackoutEl.className = 'tali-blackout-overlay';
        deck.blackoutEl.addEventListener('pointerdown', function () { toggleBlackout(false); });
        document.body.appendChild(deck.blackoutEl);
      }
      deck.blackoutEl.style.display = 'block';
    } else if (deck.blackoutEl) {
      deck.blackoutEl.style.display = 'none';
    }
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

  // --- events + plugins (deck API) -----------------------------------
  function on(evt, cb) { (deck.listeners[evt] = deck.listeners[evt] || []).push(cb); }
  function fire(evt) {
    var detail = { h: deck.h, v: deck.v, currentSlide: currentSlide() };
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
  // menu plus a progress bar + prev/next arrows. Built once in
  // normal mode; auto-hides on idle. Fixed to the viewport (not the scaled
  // .tali-slides), so it doesn't ride the deck transform.
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
    return '<button class="tali-menu-item" data-action="' + action + '"><span class="tali-menu-ico">' + ico +
      '</span><span class="tali-menu-label">' + label + '</span>' + (hint ? '<span class="tali-menu-hint">' + hint + '</span>' : '') + '</button>';
  }
  function key(k, d) { return '<div class="tali-key"><kbd>' + k + '</kbd><span>' + d + '</span></div>'; }
  var KEYS_HTML =
    key('← →', 'Navigate') + key('↑ ↓', 'Vertical slides') + key('Space', 'Next') +
    key('O', 'Overview') + key('F', 'Fullscreen') + key('S', 'Speaker view') +
    key('D', 'Annotate') + key('B', 'Black screen') + key('⌘/Ctrl P', 'Export PDF') +
    key('?', 'This menu') + key('Esc', 'Close');

  function buildChrome() {
    var rev = deckEl();
    if (!rev || deck.chrome) return;
    var prog = document.createElement('div');
    prog.className = 'tali-progress';
    prog.innerHTML = '<div class="tali-progress-fill"></div>';
    rev.appendChild(prog);
    var ctl = document.createElement('div');
    ctl.className = 'tali-controls';
    ctl.innerHTML =
      '<button class="tali-ctl tali-ctl-prev" aria-label="Previous slide" title="Previous (←)">‹</button>' +
      '<button class="tali-ctl tali-ctl-next" aria-label="Next slide" title="Next (→)">›</button>' +
      '<button class="tali-ctl tali-ctl-menu" aria-label="Menu" title="Menu (m)" aria-haspopup="menu" aria-expanded="false">' + IC.menu + '</button>';
    rev.appendChild(ctl);
    ctl.querySelector('.tali-ctl-prev').addEventListener('click', function () { prev(); });
    ctl.querySelector('.tali-ctl-next').addEventListener('click', function () { next(); });
    deck.menuBtn = ctl.querySelector('.tali-ctl-menu');
    deck.menuBtn.addEventListener('click', function () { toggleMenu(); });
    deck.chrome = { fill: prog.querySelector('.tali-progress-fill'), ctl: ctl };
    buildMenu();
    document.addEventListener('mousemove', showChrome);
    document.addEventListener('touchstart', showChrome, { passive: true });
    showChrome();
    updateChrome();
  }
  function buildMenu() {
    var menu = document.createElement('div');
    menu.className = 'tali-menu';
    menu.setAttribute('hidden', '');
    var themeRow = (window.taliDeckThemeManaged && !window.taliDeckEmbedded)
      ? '<div class="tali-menu-head">Theme</div><div class="tali-menu-tools">' +
        tool('theme', IC.moon, 'Dark mode', '<span class="tali-theme-state"></span>') + '</div>'
      : '';
    menu.innerHTML =
      '<div class="tali-menu-head">Slides</div><div class="tali-menu-slides"></div>' +
      '<div class="tali-menu-head">Tools</div><div class="tali-menu-tools">' +
        tool('overview', IC.grid, 'Overview', 'O') +
        tool('reader', IC.reader, 'Reader mode', '') +
        tool('draw', IC.pen, 'Annotate', 'D') +
        tool('speaker', IC.speak, 'Speaker view', 'S') +
        tool('fullscreen', IC.fs, 'Fullscreen', 'F') +
        tool('print', IC.pdf, 'Export PDF', '⌘P') +
      '</div>' + themeRow +
      '<div class="tali-menu-head">Keyboard</div><div class="tali-menu-keys">' + KEYS_HTML + '</div>';
    document.body.appendChild(menu);
    var backdrop = document.createElement('div');
    backdrop.className = 'tali-menu-backdrop';
    backdrop.setAttribute('hidden', '');
    backdrop.addEventListener('click', function () { toggleMenu(false); });
    document.body.appendChild(backdrop);
    menu.addEventListener('click', onMenuClick);
    deck.menu = menu;
    deck.menuBackdrop = backdrop;
  }
  function refreshSlideList() {
    var box = deck.menu && deck.menu.querySelector('.tali-menu-slides');
    if (!box) return;
    var all = allSlides(), cur = currentSlide(), html = '';
    for (var i = 0; i < all.length; i++) {
      var hd = all[i].querySelector('h1,h2,h3');
      var label = hd ? hd.textContent.trim() : ('Slide ' + (i + 1));
      html += '<button class="tali-menu-slide' + (all[i] === cur ? ' tali-on' : '') + '" data-i="' + i + '">' +
        '<span class="tali-menu-slide-n">' + (i + 1) + '</span><span class="tali-menu-slide-t">' + esc(label) + '</span></button>';
    }
    box.innerHTML = html;
    var on = box.querySelector('.tali-on');
    if (on && on.scrollIntoView) on.scrollIntoView({ block: 'nearest' });
  }
  function markActiveTools() {
    if (!deck.menu) return;
    var set = function (action, on) {
      var b = deck.menu.querySelector('[data-action="' + action + '"]');
      if (b) b.classList.toggle('tali-on', !!on);
    };
    set('reader', deck.scroll);
    set('draw', deck.draw && deck.draw.on);
    set('overview', deck.overview);
    var st = deck.menu.querySelector('.tali-theme-state');
    if (st) st.textContent = document.documentElement.classList.contains('tali-deck-dark') ? 'On' : 'Off';
  }
  function onMenuClick(e) {
    var slide = e.target.closest && e.target.closest('.tali-menu-slide');
    if (slide) { jumpToIndex(parseInt(slide.getAttribute('data-i'), 10)); return; }
    var item = e.target.closest && e.target.closest('.tali-menu-item');
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
    // The control menu is a light-dismiss POPOVER (transparent backdrop, Esc + click-away),
    // not a content-covering modal — so, like the reader menu, it is deliberately NOT
    // focus-trapped (aria-modal would misrepresent it). aria-expanded on the launcher is
    // the correct popover signal.
    if (deck.menuBtn) deck.menuBtn.setAttribute('aria-expanded', open ? 'true' : 'false');
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
    if (!window.taliDeckSetTheme) return;
    var dark = document.documentElement.classList.contains('tali-deck-dark');
    window.taliDeckSetTheme(dark ? 'light' : 'dark');
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
    document.documentElement.classList.remove('tali-idle');
    clearTimeout(idleTimer);
    idleTimer = setTimeout(function () { if (!deck.menuOpen) document.documentElement.classList.add('tali-idle'); }, 3000);
  }

  // --- lifecycle ----------------------------------------------------------
  function initialize(opts) {
    if (opts) for (var k in opts) deck.config[k] = opts[k];
    if (deck.ready) { sync(); return facade; } // idempotent: client.js may call again
    var rev = deckEl();
    if (!rev || !slidesEl()) return facade;
    var qmd = new URLSearchParams(location.search).get('qmd');
    deck.mode = qmd === 'speaker' ? 'speaker' : qmd === 'embed' ? 'embed'
      : qmd === 'print' ? 'print' : 'normal';
    var d = document.documentElement.style;
    d.setProperty('--tali-deck-w', deck.config.width + 'px');
    d.setProperty('--tali-deck-h', deck.config.height + 'px');

    // The speaker window doesn't render the deck itself; it builds the control UI.
    if (deck.mode === 'speaker') { initSpeaker(); deck.ready = true; return facade; }

    // Print preview: lay every slide out as a page on screen (same layout Cmd/Ctrl+P uses).
    if (deck.mode === 'print') {
      clampIndices();
      rev.classList.add('tali-ready');
      enterPrint();
      deck.ready = true;
      return facade;
    }

    if (!readHash()) { deck.h = 0; deck.v = 0; }
    clampIndices();
    // Restore a deep-linked fragment step (#/h/v/frag) once the slide is known.
    if (deck.pendingFrag != null) deck.frag = Math.max(0, Math.min(deck.pendingFrag, fragCount()));
    apply(); layout(); updateNumber();
    rev.classList.add('tali-ready'); // show the deck now the first slide is placed
    // Coalesce a burst of resize events (a drag-resize / rotate fires many) into ONE
    // layout per animation frame — layout re-fits every slide (fitSlide measures each),
    // so running it per-event thrashed the main thread.
    var resizeRAF = null;
    window.addEventListener('resize', function () {
      if (resizeRAF) return;
      resizeRAF = requestAnimationFrame(function () { resizeRAF = null; layout(); });
    });
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
      // If the presentation window goes away, close its speaker popup + drop the ref so
      // a stale `deck.speakerWin` isn't messaged after this window is gone.
      window.addEventListener('pagehide', function () {
        if (deck.speakerWin && !deck.speakerWin.closed) { try { deck.speakerWin.close(); } catch (e) {} }
        deck.speakerWin = null;
      });
      buildChrome(); // the control menu + progress bar + nav arrows
      // Embedded in a same-origin page: follow the host's light/dark toggle live.
      if (window.taliDeckEmbedded && window.taliDeckApplyTheme) {
        try {
          new MutationObserver(window.taliDeckApplyTheme)
            .observe(window.top.document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
        } catch (e) {}
      }
      window.addEventListener('beforeprint', enterPrint); // Cmd/Ctrl+P -> one slide per page
      window.addEventListener('afterprint', exitPrint);
      // Scroll/reader mode vs. step (presentation) mode.
      //   - A deck opened directly as its own page (standalone, NOT embedded in a
      //     host page) defaults to scroll/reader view: that's how a reader meets it
      //     on their own. The stepped deck is the *presenting* surface; reach it with
      //     the ?qmd=present (or ?qmd=slides) opt-in.
      //   - An embedded deck ({{< embed deck.qmd >}}, detected via the iframe flag
      //     window.taliDeckEmbedded) is NOT "opened as a link": it keeps the old
      //     contract — step mode unless ?qmd=scroll or a narrow viewport.
      //   - ?qmd=scroll always forces scroll; a narrow/portrait screen always uses
      //     scroll (the fixed-aspect deck letterboxes badly there).
      //   Re-evaluated on resize/rotate via the media-query change listener.
      var narrow = window.matchMedia('(max-width: 600px)');
      var present = (qmd === 'present' || qmd === 'slides'); // step-mode opt-in
      var standalone = !window.taliDeckEmbedded;              // a directly-opened deck page
      var syncScroll = function () {
        var scroll = qmd === 'scroll' || narrow.matches || (standalone && !present);
        scroll ? enterScroll() : exitScroll();
      };
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

  window.TaliesinDeck = facade;
  // Back-compat: the pre-rename public global. Same live object, so every method
  // (and any spec-added method) is reachable through either name.
  window.QmdDeck = window.TaliesinDeck;
})();
