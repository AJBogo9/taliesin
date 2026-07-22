// taliesin deck engine: the navigation + scaling for slides, owned by the project
// so block-level incremental updates and click-to-source work in decks the same
// way they do on a page. It drives taliesin's own DOM contract
// (.tali-deck > .tali-slides > section, nested <section> stacks) and exposes a
// window.TaliesinDeck API (initialize/sync/layout/slide + on/getSlides/getCurrentSlide/
// registerPlugin) that the preview client and theme extensions bind to.
(function () {
  // The deck's single mutable state bag. The navigation/config fields are typed;
  // the many DOM-ref / handle / mode fields added across init (menu, share, feed,
  // speaker window, overview, wake-lock, …) are a dynamic bag, so an index
  // signature keeps them `any` rather than enumerating 40+ optional properties.
  /**
   * @type {{
   *   config: { width: number, height: number, margin: number, center: boolean, hash: boolean, slideNumber: boolean },
   *   h: number, v: number, frag: number,
   *   ready: boolean, overview: boolean,
   *   plugins: any[],
   *   listeners: Record<string, Array<(...a: any[]) => void>>,
   *   [k: string]: any
   * }}
   */
  var deck = {
    config: {
      width: 960, height: 540, margin: 0.04, // 16:9 default
      center: false, hash: true, slideNumber: false,
    },
    h: 0, v: 0, frag: 0,
    ready: false,
    overview: false,
    plugins: [],
    listeners: {},
  };

  function slidesEl() { return /** @type {HTMLElement | null} */ (document.querySelector('.tali-deck .tali-slides')); }
  function deckEl() { return /** @type {HTMLElement | null} */ (document.querySelector('.tali-deck')); }

  // Top-level horizontal sections (a stack wrapper counts as one).
  /** @returns {HTMLElement[]} */
  function tops() {
    var s = slidesEl();
    return s ? /** @type {HTMLElement[]} */ (Array.prototype.filter.call(s.children, isSection)) : [];
  }
  /** @param {Element} n */
  function isSection(n) { return n.tagName === 'SECTION'; }
  // The vertical slides of a top: a stack's children, else the top itself.
  /** @param {HTMLElement} top @returns {HTMLElement[]} */
  function vertsOf(top) {
    var kids = /** @type {HTMLElement[]} */ (Array.prototype.filter.call(top.children, isSection));
    return kids.length ? kids : [top];
  }
  /** @param {HTMLElement} top */
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
    /** @type {HTMLElement[]} */
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
  // A run of >6 top-level slides is one over-wide strip of specks in the overview
  // (a flat all-`##` deck is the worst case). In OVERVIEW ONLY, reflow such a run into
  // a near-square block of `ceil(sqrt(n))` columns so tiles are big and up/down work.
  // Present mode keeps the run as one row so the storyline pans straight left-to-right.
  // Only top-level runs wrap: positionGrid lays a stack's sub-slides straight across
  // from their wrapper regardless, so wrapping a stack row would desync the grid.
  var OVERVIEW_ROW_MAX = 6; // a run longer than this reflows; up to 6 stays a readable line
  /** @typedef {{ h: number, v: number }} GridCell */
  /** @param {GridCell[][]} rows @param {GridCell[]} run */
  function pushRun(rows, run) {
    if (deck.overview && run.length > OVERVIEW_ROW_MAX) {
      var cols = Math.ceil(Math.sqrt(run.length));
      for (var i = 0; i < run.length; i += cols) rows.push(run.slice(i, i + cols));
    } else {
      rows.push(run);
    }
  }
  /** @returns {GridCell[][]} */
  function gridRows() {
    var T = tops();
    /** @type {GridCell[][]} */
    var rows = [];
    /** @type {GridCell[] | null} */
    var run = null;
    for (var h = 0; h < T.length; h++) {
      if (isStack(T[h])) {
        if (run) { pushRun(rows, run); run = null; }
        rows.push(vertsOf(T[h]).map(function (sec, v) { return { h: h, v: v }; }));
      } else {
        if (!run) run = [];
        run.push({ h: h, v: 0 });
      }
    }
    if (run) pushRun(rows, run);
    return rows.length ? rows : [[{ h: 0, v: 0 }]];
  }
  // The visual (row, col) of a leaf, plus the row grid it came from.
  /** @param {number} h @param {number} v */
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
    /** @type {Record<number, { row: number, col0: number }>} */
    var loc = {};
    var maxCols = 1; // per top index: its row + the column of its first leaf
    rows.forEach(function (rowArr, r) {
      maxCols = Math.max(maxCols, rowArr.length);
      rowArr.forEach(function (cell, c) { if (!(cell.h in loc)) loc[cell.h] = { row: r, col0: c }; });
    });
    if (s) {
      s.style.setProperty('--tali-cols', String(maxCols));
      s.style.setProperty('--tali-rows', String(rows.length));
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
  }
  // The camera target for the current state: the cell that fills the 16:9 stage
  // (normal), or the free map camera (overview).
  function cameraTarget() {
    var rev = /** @type {HTMLElement} */ (deckEl());
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
  /** @param {number} cx @param {number} cy @param {number} scale @param {string} mode */
  function applyCam(cx, cy, scale, mode) {
    var s = slidesEl(), rev = deckEl(); if (!s || !rev) return;
    var sw = rev.clientWidth || window.innerWidth, sh = rev.clientHeight || window.innerHeight;
    var tx = sw / 2 - scale * cx, ty = sh / 2 - scale * cy;
    s.classList.toggle('tali-cam-anim', mode === 'css');
    s.style.transform = 'translate(' + tx + 'px,' + ty + 'px) scale(' + scale + ')';
    document.documentElement.style.setProperty('--tali-deck-scale', String(scale));
    deck.cam = { cx: cx, cy: cy, scale: scale };
  }
  function reducedMotion() {
    return window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }
  // setCamera: snap (animate falsy) or CSS-tween (animate truthy) to the camera target.
  // A pan IS the transition, so every move — a step, a long jump, overview enter/exit —
  // rides the single CSS transform transition on `.tali-cam-anim`; reduced-motion snaps.
  /** @param {boolean} animate */
  function setCamera(animate) {
    var t = cameraTarget();
    applyCam(t.cx, t.cy, t.scale, (animate && !reducedMotion()) ? 'css' : 'instant');
  }
  // Snap the camera to a SPECIFIC cell (h,v) instead of the live deck.h/v. Used by an
  // auto-animate settle so a flushed morph frames its own captured target, not wherever
  // navigation has since advanced to (fixes the 3+-chained-morph camera misframe).
  /** @param {number} h @param {number} v */
  function setCameraToCell(h, v) {
    var rev = /** @type {HTMLElement} */ (deckEl());
    var W = deck.config.width, H = deck.config.height;
    var sw = rev.clientWidth || window.innerWidth, sh = rev.clientHeight || window.innerHeight;
    var scale = Math.min(sw / W, sh / H);
    var p = posOf(h, v);
    applyCam(p.col * W + W / 2, p.row * H + H / 2, scale > 0 ? scale : 1, 'instant');
  }
  function layout() {
    if (!slidesEl()) return;
    positionGrid();
    applyBackgrounds();
    allSlides().forEach(fitSlide); // all slides are laid out now, not just the current one
    if (deck.overview) fitOverview(); // viewport changed: re-fit the map
    setCamera(false);
  }
  // A pure viewport change (resize / rotate). Grid positions and per-slide font-fit are
  // in FIXED design units (sections are a 960x540 cell scaled by the camera), so they
  // don't change with the viewport — only the overview map fit and the camera scale do.
  // Re-fitting every slide here (the old layout() call) forced O(N) reflows per resize
  // frame and janked a 100-200 slide deck; late in-slide content re-fits via fitRO instead.
  function relayoutViewport() {
    if (!slidesEl()) return;
    if (deck.overview) fitOverview();
    setCamera(false);
  }

  // Off-camera slides stay in the DOM (the camera just frames the current cell), but for
  // assistive tech + the tab order that means every non-visible slide is still reachable.
  // `inert` removes a leaf from the AT tree AND tab order (and blocks its clicks) in one
  // attribute, so a screen-reader/keyboard user only meets the current slide in step mode.
  // The single source of truth: in overview and the feed every slide is meant to be
  // readable, so inert is cleared from all of them; otherwise only the current leaf
  // is non-inert. Called from applyClasses (commit + init), the mode enter/exit hooks, and
  // setOverview, so any path that changes "what's visible" re-derives inert consistently.
  function syncInert() {
    var showAll = deck.overview || deck.feed; // the feed shows every slide, like overview
    var cur = showAll ? null : currentSlide();
    allSlides().forEach(function (s) {
      if (showAll || s === cur) s.removeAttribute('inert');
      else s.setAttribute('inert', '');
    });
  }

  // --- the non-camera part of a slide change -----------------------------
  // Fragment visibility and chrome. Split out so
  // auto-animate can update these without moving the camera. Per-slide visibility
  // is the camera transform itself: every slide is laid out into its grid cell and
  // the camera frames the current one (no per-slide show/hide class needed).
  function applyClasses() {
    applyFragments();
    updateChrome(); // progress bar / menu state follow the current slide
    syncInert(); // keep off-camera slides out of the AT tree + tab order (step mode)
    observeCurrentMedia(); // re-target the late-content re-fit observer onto the new current slide
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
  /** @param {HTMLElement} sec @returns {HTMLElement} */
  function ensureSlideBg(sec) {
    var bg = /** @type {HTMLElement | null} */ (sec.querySelector(':scope > .tali-slide-bg'));
    if (!bg) {
      bg = document.createElement('div');
      bg.className = 'tali-slide-bg';
      sec.insertBefore(bg, sec.firstChild);
    }
    return bg;
  }
  /** @param {HTMLElement} sec */
  function paintSlideBg(sec) {
    var color = sec.getAttribute('data-background-color');
    var gradient = sec.getAttribute('data-background-gradient');
    var image = sec.getAttribute('data-background-image');
    sec.classList.remove('tali-dark-bg');
    sec.classList.remove('tali-light-bg');
    var existing = sec.querySelector(':scope > .tali-slide-bg');
    if (!color && !gradient && !image) { if (existing) existing.remove(); return; }
    var bg = ensureSlideBg(sec);
    bg.style.cssText = '';
    if (color) bg.style.backgroundColor = color;
    if (gradient) bg.style.backgroundImage = gradient;
    if (image) {
      // An image slide assumes a dark background (text is flipped white below). Paint a
      // neutral dark scrim UNDER the image so a failed/missing/typo'd image URL leaves a
      // dark backdrop for that white text instead of the bare (possibly light) deck canvas.
      // Honour an explicit background-color as the fallback if the author set one.
      if (!color) bg.style.backgroundColor = '#1a1a1a';
      bg.style.backgroundImage = 'url("' + image + '")';
      bg.style.backgroundSize = sec.getAttribute('data-background-size') || 'cover';
      bg.style.backgroundPosition = sec.getAttribute('data-background-position') || 'center';
      bg.style.backgroundRepeat = sec.getAttribute('data-background-repeat') || 'no-repeat';
    }
    // Dark bg (or image/gradient, assumed dark) -> light text; a light solid colour
    // -> dark text, so a light named/hex slide background stays readable whatever the
    // deck's own theme is (its default text may be light).
    if (image || gradient || (color && isDarkColor(color))) sec.classList.add('tali-dark-bg');
    else if (color) sec.classList.add('tali-light-bg');
  }
  function applyBackgrounds() { allSlides().forEach(paintSlideBg); }
  /** @type {Record<string, boolean>} */
  var darkColorCache = Object.create(null);
  /** @param {string} c */
  function isDarkColor(c) {
    // Resolve ANY CSS colour (named like "white"/"lightblue", hex, rgb(), hsl()) to
    // rgb via the browser, so a light named background is no longer mis-assumed dark
    // (which flipped heading/body text to invisible white on a light slide). A
    // sentinel detects a truly-unparseable value and preserves the old "assume dark"
    // fallback for it. Memoised by colour string: applyBackgrounds() runs every layout()
    // and each probe forces a sync style/layout flush, so cache the (deterministic) verdict.
    if (c in darkColorCache) return darkColorCache[c];
    var probe = document.createElement('span');
    probe.style.color = 'rgb(1, 2, 3)'; // sentinel: survives an invalid assignment
    probe.style.color = c;
    probe.style.display = 'none';
    document.body.appendChild(probe);
    var resolved = getComputedStyle(probe).color;
    probe.remove();
    var dark;
    var m = resolved === 'rgb(1, 2, 3)' ? null : resolved.match(/rgba?\((\d+)[,\s]+(\d+)[,\s]+(\d+)/i);
    if (!m) dark = true; // unparseable colour -> assume dark
    else dark = 0.299 * +m[1] + 0.587 * +m[2] + 0.114 * +m[3] < 140;
    return (darkColorCache[c] = dark);
  }

  // --- auto-animate -------------------------------------------------------
  // When moving between two consecutive `data-auto-animate` slides, matched
  // elements (same tag + text) tween from their position/size on the old slide to
  // the new one (FLIP: measure both, translate the element to its old spot, then
  // animate to identity). Unmatched elements just appear.
  var AA_SEL = 'h1,h2,h3,h4,p,li,pre,blockquote,img,figure';
  /** @param {Element | null} s */
  function isAutoAnimate(s) { return !!(s && s.hasAttribute && s.hasAttribute('data-auto-animate')); }
  /** @param {Element} el */
  function aaKey(el) { return el.tagName + '|' + (el.textContent || '').replace(/\s+/g, ' ').trim(); }
  // Measure matched element rects in both slides (both must be laid out, so the
  // incoming slide is briefly force-shown — no paint happens mid-call).
  /** @param {HTMLElement} from @param {HTMLElement} to */
  function snapshotMatched(from, to) {
    to.style.setProperty('display', 'block', 'important');
    /** @type {Record<string, HTMLElement[]>} */
    var byKey = {};
    /** @type {NodeListOf<HTMLElement>} */ (from.querySelectorAll(AA_SEL)).forEach(function (el) {
      (byKey[aaKey(el)] || (byKey[aaKey(el)] = [])).push(el);
    });
    /** @type {Array<{ to: HTMLElement, fr: DOMRect, tr: DOMRect, ff: string, tf: string }>} */
    var snap = [];
    /** @type {NodeListOf<HTMLElement>} */ (to.querySelectorAll(AA_SEL)).forEach(function (el) {
      var list = byKey[aaKey(el)];
      if (list && list.length) {
        var a = /** @type {HTMLElement} */ (list.shift());
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
  /** @param {Array<{ to: HTMLElement, fr: DOMRect, tr: DOMRect, ff: string, tf: string }>} snap @param {HTMLElement} to */
  function flipTo(snap, to) {
    // reduced-motion: skip the FLIP tween — the target slide is already in its final
    // layout, so returning early lands the same end state with no inline transition.
    if (reducedMotion()) return;
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
    });
    // Cleanup (clearing inline styles + `.tali-aa`) is owned by autoAnimateTo's single
    // cancellable settle, so a rapid re-nav can flush it instead of racing naked timers.
  }
  // Auto-animate in the camera model: instead of panning between the two cells, hold
  // the camera and overlay `to` on `from`'s cell so the matched elements morph in
  // place; then snap `to` and the camera to `to`'s real cell together — a net-zero
  // screen move, so the reposition is invisible.
  /** @param {HTMLElement} from @param {HTMLElement} to */
  function autoAnimateTo(from, to) {
    // `to`'s own cell — captured now (deck.h/v were set to `to` by moveTo before
    // renderMove), so the settle frames THIS morph's target even after nav advances past it.
    var th = deck.h, tv = deck.v;
    // Flush any in-flight morph to its committed end state FIRST, so `from` (= the
    // previous `to`) is back at its real cell before we read it below, and its matched
    // elements' inline styles are cleared before a re-animation. Fixes the rapid-nav race
    // where a naked 520ms timer fired mid-next-transition (elements snapped / camera
    // jittered / a 3rd slide overlapped onto the 1st's cell).
    if (deck.aaSettle) deck.aaSettle();
    var toTransform = to.style.transform;       // to's real grid cell
    to.style.transform = from.style.transform;  // overlap `to` onto `from`'s cell
    to.classList.add('tali-aa');
    var snap = snapshotMatched(from, to);        // measure both at the same screen spot
    from.style.opacity = '0';                    // hide the old slide; the morph carries the motion
    applyClasses();                              // update state, but DON'T move the camera
    flipTo(snap, to);
    // One cancellable settle does all cleanup: a new autoAnimateTo flushes it via the
    // generation guard above so a rapid re-nav can't strip transforms mid-transition.
    var settle = deck.aaSettle = function () {
      if (deck.aaTimer) { clearTimeout(deck.aaTimer); deck.aaTimer = null; }
      deck.aaSettle = null;
      snap.forEach(function (s) {
        var st = s.to.style;
        st.transition = ''; st.transform = ''; st.transformOrigin = ''; st.fontSize = '';
      });
      to.classList.remove('tali-aa');
      from.style.opacity = '';
      to.style.transform = toTransform;          // restore `to`'s real cell ...
      setCameraToCell(th, tv);                   // ... and frame IT (its captured cell, not live deck.h/v)
    };
    deck.aaTimer = setTimeout(settle, 520);
  }
  // --- fragments (incremental steps) -----------------------------------
  // A fragment is any `.fragment` element or a list item inside `.incremental`,
  // in document order. They start hidden (via visibility, so layout + shrink-to-
  // fit are unaffected) and show one per forward step before the slide advances.
  // A slide's ordered "steps": each `.fragment`/`.incremental` item is a step
  // step; a `pre[data-code-lines]` with K `|`-separated segments contributes K-1
  // steps (segment 0 is the slide's base highlight, applied before any step).
  var FRAG_SEL = '.fragment, .incremental > ul > li, .incremental > ol > li';
  /** @param {Element | null} slide @returns {Array<{ frag?: Element, mm?: Element, code?: Element, seg?: string }>} */
  function fragsOf(slide) {
    if (!slide) return [];
    /** @type {Array<{ frag?: Element, mm?: Element, code?: Element, seg?: string }>} */
    var steps = [];
    slide.querySelectorAll(FRAG_SEL + ', pre[data-code-lines], .magic-move').forEach(function (node) {
      if (node.classList.contains('magic-move')) {
        // A magic-move that also follows a `. . .` pause carries `.fragment`; give it a
        // reveal step first (else it stays visibility:hidden for the whole talk), then
        // its per-block morph steps.
        if (node.classList.contains('fragment')) steps.push({ frag: node });
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
        // A plain code block paused into `.fragment` matched FRAG_SEL, not
        // `pre[data-code-lines]`, so it has no line-step spec — its `.fragment` reveal
        // step above is the whole story. Guard the split or `null.split` wedges nav.
        var codeLines = node.getAttribute('data-code-lines');
        if (codeLines == null) return;
        var segs = codeLines.split('|');
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
      highlightLines(pre, (pre.getAttribute('data-code-lines') || '').split('|')[0]);
    });
    slide.querySelectorAll(FRAG_SEL).forEach(function (el) { el.classList.remove('tali-frag-visible'); });
    /** @type {Map<Element, number>} */
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
  /** @param {Element} div @returns {HTMLElement[]} */
  function mmBlocks(div) { return /** @type {HTMLElement[]} */ (Array.prototype.slice.call(div.querySelectorAll(':scope > pre'))); }
  /** @param {Element} l */
  function lineText(l) { return (l.textContent || '').replace(/\s+/g, ' ').trim(); }
  /** @param {Element} div @param {number} target */
  function setOrMorphMM(div, target) {
    var pres = mmBlocks(div);
    if (!pres.length) return;
    target = Math.max(0, Math.min(target, pres.length - 1));
    var prev = /** @type {any} */ (div).__mm;
    // reduced-motion: fall through to the instant show/hide (no line-glide/fade morph).
    if (deck.animSteps && prev != null && prev !== target && !reducedMotion())
      morphMM(div, pres, prev, target);
    else pres.forEach(function (p, i) { p.classList.toggle('tali-mm-active', i === target); });
    /** @type {any} */ (div).__mm = target;
  }
  /** @param {Element} div @param {HTMLElement[]} pres @param {number} from @param {number} to */
  function morphMM(div, pres, from, to) {
    var blockFrom = pres[from], blockTo = pres[to];
    var scale = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--tali-deck-scale')) || 1;
    /** @type {Record<string, HTMLElement[]>} */
    var byText = {};
    /** @type {NodeListOf<HTMLElement>} */ (blockFrom.querySelectorAll('.tali-hl-ln')).forEach(function (l) {
      (byText[lineText(l)] || (byText[lineText(l)] = [])).push(l);
    });
    blockTo.classList.add('tali-mm-active');
    blockFrom.classList.remove('tali-mm-active'); // fades out (CSS opacity transition)
    /** @type {NodeListOf<HTMLElement>} */ (blockTo.querySelectorAll('.tali-hl-ln')).forEach(function (lt) {
      var list = byText[lineText(lt)], st = lt.style;
      if (list && list.length) { // matched line: glide from its old position
        var lf = /** @type {HTMLElement} */ (list.shift()), rf = lf.getBoundingClientRect(), rt = lt.getBoundingClientRect();
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
  // washing them in the accent (the rest keep their contrast). "all"/empty clears the focus.
  /** @param {Element} pre @param {string | null | undefined} spec */
  function highlightLines(pre, spec) {
    var lines = pre.querySelectorAll('.tali-hl-ln');
    spec = (spec || '').trim();
    if (!spec || spec === 'all') {
      pre.classList.remove('tali-hl-lines-active');
      lines.forEach(function (l) { l.classList.remove('tali-hl-ln-hl'); });
      return;
    }
    var on = parseLineSpec(spec, lines.length);
    pre.classList.add('tali-hl-lines-active');
    lines.forEach(function (l, i) { l.classList.toggle('tali-hl-ln-hl', on.has(i + 1)); });
  }
  // Clamp a range's upper bound to the rendered line count: highlightLines only ever
  // queries on.has(i+1) for i in [0, lines-1], so a line beyond the code could never
  // match — but an unbounded typo range (`10-100000000`) would OOM-freeze the tab.
  /** @param {string} spec @param {number} max @returns {Set<number>} */
  function parseLineSpec(spec, max) {
    var on = /** @type {Set<number>} */ (new Set());
    spec.split(',').forEach(function (part) {
      var m = part.trim().match(/^(\d+)\s*-\s*(\d+)$/);
      if (m) { for (var n = +m[1], hi = Math.min(+m[2], max); n <= hi; n++) on.add(n); }
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
    // Announce the step through the live region so a screen-reader user hears that a
    // reveal happened + where they are within the slide (WCAG 4.1.3).
    else { applyFragments(); announce('Step ' + deck.frag + ' of ' + fragCount()); }
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
  /** @param {HTMLElement | null} sec */
  function fitSlide(sec) {
    if (!sec || deck.overview || deck.feed) return; // the feed sizes by CSS font-size, not fit
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
  // Late-content re-fit. fitSlide measures once at layout, but a slide's {js} chart,
  // <img>, KaTeX, or async widget can render taller AFTER that and overflow the fitted
  // box. A ResizeObserver on the CURRENT slide's embedded media re-fits it when their
  // size lands (re-targeted per slide change from applyClasses). `fitting` guards the
  // loop: fitSlide's own font-size change can nudge an em-sized embed, whose resize
  // would otherwise re-enter — we ignore fires until the frame after our fit settles.
  var fitRO = /** @type {ResizeObserver | null} */ (null), fitRORAF = /** @type {number | null} */ (null), fitting = false, pendingRefit = false;
  function refitCurrent() {
    // A real content resize arriving while our own fit is settling is swallowed by the
    // loop guard (the observer marks it "seen" during the suppressed broadcast), so
    // remember it and honour it once fitting clears — else a two-stage embed (axes, then
    // data) strands the slide fit to the first stage. Converges: fitSlide is idempotent on
    // stable content, so the deferred pass makes no net change and fires nothing further.
    if (fitting) { pendingRefit = true; return; }
    if (fitRORAF) return;
    fitRORAF = requestAnimationFrame(function () {
      fitRORAF = null;
      var cur = currentSlide();
      if (!cur || deck.overview || deck.feed) return; // fitSlide no-ops in those modes anyway
      fitting = true;
      pendingRefit = false;
      fitSlide(cur);
      requestAnimationFrame(function () { // clear after this frame's RO dispatch
        fitting = false;
        if (pendingRefit) { pendingRefit = false; refitCurrent(); } // a growth landed mid-fit
      });
    });
  }
  function observeCurrentMedia() {
    if (deck.mode !== 'normal' || typeof ResizeObserver === 'undefined') return;
    if (!fitRO) fitRO = new ResizeObserver(refitCurrent);
    var ro = fitRO; // captured non-null for the obs() closure
    ro.disconnect();
    var cur = currentSlide();
    if (!cur) return;
    // Direct children catch a content container growing (a late {js} output appended into
    // an auto-height box); descendant media catch a nested <img>/chart loading inside a
    // fixed box. The `fitting` guard absorbs the text reflow our own font-fit provokes.
    /** @type {Set<Element>} */
    var seen = new Set();
    /** @param {Element} el */
    function obs(el) { if (el && !seen.has(el)) { seen.add(el); ro.observe(el); } }
    for (var i = 0; i < cur.children.length; i++) obs(cur.children[i]);
    cur.querySelectorAll('img, canvas, svg, video, iframe').forEach(obs);
  }
  // --- navigation ---------------------------------------------------------
  // Render the move to the current cell: morph matched elements between two consecutive
  // opted-in auto-animate slides (autoAnimateTo), else pan the camera. Shared by commit()
  // and the remote/hash paths so a speaker-driven or deep-link move also morphs, not only
  // a forward key press.
  function renderMove() {
    var to = currentSlide(), from = deck.lastSlide;
    // Never morph while the overview is open: onHashChange/applyRemote can now reach here
    // in overview (commit() never could), and autoAnimateTo would blink/displace a visible
    // map tile. In overview just pan (apply() is a no-op on the camera while overview owns it).
    if (!deck.overview && from && to && from !== to && isAutoAnimate(from) && isAutoAnimate(to)) {
      autoAnimateTo(from, to);
    } else {
      apply(); // pan/zoom the camera to the current cell
    }
  }
  function commit() {
    clampIndices();
    if (deck.mode === 'speaker') { updateSpeakerUI(); fire('slidechanged'); broadcastState(); return; }
    renderMove();
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
  /** @param {number} h @param {number} v @param {boolean} showAll */
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
  /** @param {number} d */
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
    var rev = /** @type {HTMLElement} */ (deckEl());
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
  // Zoom the overview to scale `ns`, keeping the stage-point (px,py) fixed under the
  // anchor (px/py are relative to the deck element's top-left). Shared by wheel zoom
  // and touch pinch so both anchor the zoom identically.
  /** @param {number} px @param {number} py @param {number} ns */
  function zoomOverviewTo(px, py, ns) {
    var st = ovStage(), scale = deck.ov.scale;
    var tx = st.sw / 2 - scale * deck.ov.cx, ty = st.sh / 2 - scale * deck.ov.cy;
    var wx = (px - tx) / scale, wy = (py - ty) / scale;    // world point under the anchor
    ns = Math.max(deck.ov.fit, Math.min(ns, st.maxScale));
    deck.ov.scale = ns;
    deck.ov.cx = (st.sw / 2 - (px - ns * wx)) / ns;         // keep that point fixed
    deck.ov.cy = (st.sh / 2 - (py - ns * wy)) / ns;
    clampOv();
    setCamera(false);
  }
  /** @param {WheelEvent} e */
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
    var rev = /** @type {HTMLElement} */ (deckEl()), r = rev.getBoundingClientRect();
    var px = e.clientX - r.left, py = e.clientY - r.top;   // cursor in stage coords
    zoomOverviewTo(px, py, deck.ov.scale * Math.exp(-e.deltaY * 0.0015)); // smooth, proportional
  }
  var ovDrag = /** @type {{ x: number, y: number, cx: number, cy: number, moved: boolean } | null} */ (null);
  // Mouse / pen drag pans the overview map. Touch is owned entirely by the touch
  // handlers (pan + pinch) so a swipe can never both pan here AND fire nav (B6-31).
  /** @param {PointerEvent} e */
  function onOverviewPointerDown(e) {
    if (!deck.overview || !deck.ov || e.button !== 0 || e.pointerType === 'touch') return;
    ovDrag = { x: e.clientX, y: e.clientY, cx: deck.ov.cx, cy: deck.ov.cy, moved: false };
  }
  /** @param {PointerEvent} e */
  function onOverviewPointerMove(e) {
    if (!ovDrag) return;
    var dx = e.clientX - ovDrag.x, dy = e.clientY - ovDrag.y;
    if (!ovDrag.moved && dx * dx + dy * dy < 25) return;    // 5px before it counts as a drag
    ovDrag.moved = true;
    /** @type {HTMLElement} */ (deckEl()).classList.add('tali-ov-panning');
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
  /** @param {boolean} animate */
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
  /** @param {boolean} on */
  function setOverview(on) {
    if (on === deck.overview) return;
    var rev = deckEl();
    if (!rev) return;
    deck.overview = on;
    rev.classList.toggle('overview', on);
    if (on && deck.blackout) toggleBlackout(false); // can't navigate a map you can't see
    if (on) { fitOverview(); markCurrentTile(); }
    else { deck.ov = null; allSlides().forEach(function (s) { s.classList.remove('tali-overview-current'); }); }
    syncInert(); // overview: every tile is browsable, so clear inert; exiting re-inerts off-camera
    // Arm the transition BEFORE re-placing tiles so the reflow (wrapped-grid <-> strip,
    // plus the gutter shrink) rides the same tween as the camera zoom instead of
    // teleporting when .tali-cam-anim happens to be off (initial frame / post-resize).
    var sl = slidesEl();
    if (sl && !reducedMotion()) sl.classList.add('tali-cam-anim');
    positionGrid(); // add (or remove) the per-tile gutter shrink
    setCamera(true); // zoom out to the map, or back into the current cell
  }
  // Move the overview highlight one leaf forward/back in deck order, keeping it
  // on-screen as the map pans.
  /** @param {number} dCol @param {number} dRow */
  function moveHighlight(dCol, dRow) {
    var p = posOf(deck.h, deck.v), rows = p.rows, r = p.row, c = p.col;
    if (dRow) { r = Math.max(0, Math.min(r + dRow, rows.length - 1)); c = Math.min(c, rows[r].length - 1); }
    if (dCol) { c = Math.max(0, Math.min(c + dCol, rows[r].length - 1)); }
    var cell = rows[r][c];
    deck.h = cell.h; deck.v = cell.v;
    markCurrentTile();
    ensureCurrentTileVisible(true);
    announce(slideDesc(currentSlide())); // overview highlight moves are keyboard-only; voice them
  }
  /** @param {MouseEvent} e */
  function onSlidesClick(e) {
    if (!deck.overview) return;
    if (deck.ovDragged) { deck.ovDragged = false; return; } // that was a pan, not a pick
    var t = /** @type {Element | null} */ (e.target);
    var sec = /** @type {HTMLElement | null} */ (t && t.closest('.tali-deck .tali-slides section'));
    if (!sec) return;
    e.preventDefault();
    var T = tops();
    for (var h = 0; h < T.length; h++) {
      var v = vertsOf(T[h]).indexOf(sec);
      if (sec === T[h] || v >= 0) { setOverview(false); moveTo(h, v < 0 ? 0 : v, true); return; }
    }
    setOverview(false);
  }

  // Map a grid cell (h,v) back to its leaf <section>. Used by the speaker view's
  // next-slide preview.
  /** @param {number} h @param {number} v */
  function leafAt(h, v) {
    var top = tops()[h];
    return top ? (isStack(top) ? vertsOf(top)[v] : top) : null;
  }

  // --- presenter mode + cross-window sync --------------------------------
  // `s` opens a speaker window (a popup at ?qmd=speaker). It shows the current +
  // next slide as static snapshot previews (cloned `<section>`s, see snapshotInto), the
  // slide's speaker notes (`::: {.notes}`), and a timer + clock. Audience and speaker stay
  // in sync via opener<->popup postMessage (works on file://); either can drive.
  /** @param {string} url @param {string} val */
  function withQmd(url, val) { return url + (url.indexOf('?') >= 0 ? '&' : '?') + 'qmd=' + val; }
  function deckBaseUrl() { return location.href.split('#')[0].split('?')[0]; }
  // Only accept/sync with windows of our own origin, so a third-party page that
  // embeds the deck can't drive it (or read its slide position). file:// has no
  // real origin ("" / "null"), so allow those ONLY when we are ourselves on file://
  // — on http(s) a "" / "null" origin is an opaque/sandboxed context (a sandboxed
  // iframe embedding the deck) and must not drive it. When posting, target our
  // origin on http(s) and fall back to '*' on file:// (a "null" targetOrigin throws).
  var onFile = location.protocol === 'file:';
  /** @param {MessageEvent} e */
  function sameOrigin(e) { return e.origin === location.origin || ((e.origin === '' || e.origin === 'null') && onFile); }
  function targetOrigin() { return (location.origin && location.origin !== 'null') ? location.origin : '*'; }

  // Apply a position received from the other window (the speaker or audience).
  // Never re-broadcasts, so there is no echo loop.
  /** @param {number} h @param {number} v @param {number | null | undefined} frag */
  function applyRemote(h, v, frag) {
    if (deck.blackout) toggleBlackout(false); // an external slide change lifts the curtain
    deck.h = h; deck.v = v;
    clampIndices();
    deck.frag = (frag == null) ? fragCount() : frag;
    if (deck.mode === 'speaker') updateSpeakerUI();
    else if (deck.feed) { deck.frag = fragCount(); scrollToCurrent(true); updateNumber(); } // native scroll, not the camera
    else { renderMove(); updateNumber(); writeHash(); }
    fire('slidechanged');
  }
  function broadcastState() {
    var msg = { qmd: 'deck', type: 'state', h: deck.h, v: deck.v, frag: deck.frag };
    var t = targetOrigin();
    if (deck.speakerWin && !deck.speakerWin.closed) { try { deck.speakerWin.postMessage(msg, t); } catch (e) {} }
    if (window.opener && !window.opener.closed) { try { window.opener.postMessage(msg, t); } catch (e) {} }
  }
  /** @param {MessageEvent} e */
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
  /** @param {number} h @param {number} v */
  function nextIndex(h, v) {
    var T = tops(), top = T[h];
    if (top && isStack(top) && v < vertsOf(top).length - 1) return { h: h, v: v + 1 };
    if (h < T.length - 1) return { h: h + 1, v: 0 };
    return null;
  }
  // Reveal the first `n` fragment/code/magic-move steps STATICALLY on a detached snapshot
  // clone (no morph), so a speaker preview shows the slide as it looks at that step. In
  // speaker mode the live sections never run applyFragments (the deck is display:none), so
  // a raw clone sits at base state (nothing revealed) — this replays the steps onto it.
  /** @param {Element} clone @param {number} n */
  function revealStepsInClone(clone, n) {
    var steps = fragsOf(clone);
    if (n > steps.length) n = steps.length;
    clone.querySelectorAll('pre[data-code-lines]').forEach(function (pre) {
      highlightLines(pre, (pre.getAttribute('data-code-lines') || '').split('|')[0]); // base: segment 0
    });
    clone.querySelectorAll(FRAG_SEL).forEach(function (el) { el.classList.remove('tali-frag-visible'); });
    /** @type {Map<Element, number>} */
    var mmCount = new Map();
    clone.querySelectorAll('.magic-move').forEach(function (d) { mmCount.set(d, 0); });
    for (var i = 0; i < n; i++) {
      var s = steps[i];
      if (s.frag) s.frag.classList.add('tali-frag-visible');
      else if (s.code) highlightLines(s.code, s.seg);
      else if (s.mm) mmCount.set(s.mm, (mmCount.get(s.mm) || 0) + 1);
    }
    var prevAnim = deck.animSteps; deck.animSteps = false; // snapshot is static: no line-glide morph
    mmCount.forEach(function (idx, div) { setOrMorphMM(div, idx); });
    deck.animSteps = prevAnim;
  }
  // cloneNode(true) copies a <canvas> element but NOT its drawn bitmap (that lives in the
  // drawing buffer, not the DOM), so a {js}/canvas viz previews blank. Blit each source
  // canvas's pixels onto its clone. (A WebGL canvas without preserveDrawingBuffer copies
  // blank — same as before, no regression; a tainted canvas throws and is left blank.)
  /** @param {Element} sourceSec @param {Element} clone */
  function copyCanvases(sourceSec, clone) {
    var src = sourceSec.querySelectorAll('canvas'), dst = clone.querySelectorAll('canvas');
    for (var i = 0; i < src.length && i < dst.length; i++) {
      try {
        var s = src[i], d = dst[i];
        if (!s.width || !s.height) continue;
        d.width = s.width; d.height = s.height;
        var ctx = d.getContext('2d');
        if (ctx) ctx.drawImage(s, 0, 0);
      } catch (e) {}
    }
  }
  // Render a static snapshot of one slide into a speaker preview pane: a self-contained
  // mini-deck (a cloned <section> in its own .tali-slides box) that reuses the deck CSS,
  // is font-fit to the design box, then scaled to fill the pane. Replaces an earlier pair
  // of live preview iframes: no second/third full document is loaded and re-run (each was
  // executing every {js} cell in the whole deck once), and the clone carries THIS
  // window's already-rendered {js}/KaTeX/SVG output, so the preview matches the audience
  // view without re-executing anything. `fragUpto` reveals steps to that count (the
  // current step for the Current pane, one further for the Next pane).
  /** @param {HTMLElement | null} pane @param {Element | null} sourceSec @param {number} fragUpto */
  function snapshotInto(pane, sourceSec, fragUpto) {
    if (!pane) return;
    pane.textContent = '';
    if (!sourceSec) return;
    var W = deck.config.width, H = deck.config.height;
    var wrap = document.createElement('div');
    wrap.className = 'tali-deck tali-ready';
    wrap.style.cssText = 'position:relative;inset:auto;margin:0;width:100%;height:100%;overflow:hidden';
    // Carry the deck's own page colours so a dark-themed deck's light text keeps its
    // contrast (the pane's own background would otherwise show through transparent slides).
    var bodyCs = getComputedStyle(document.body);
    var bg = bodyCs.backgroundColor;
    if (!bg || bg === 'rgba(0, 0, 0, 0)' || bg === 'transparent') bg = getComputedStyle(document.documentElement).backgroundColor;
    if (bg) wrap.style.background = bg;
    wrap.style.color = bodyCs.color;
    var slides = document.createElement('div');
    slides.className = 'tali-slides';
    var clone = /** @type {HTMLElement} */ (sourceSec.cloneNode(true));
    clone.classList.remove('tali-stack', 'tali-overview-current');
    clone.style.removeProperty('transform'); // drop the grid-cell placement; sit at inset:0
    clone.removeAttribute('inert');
    slides.appendChild(clone);
    wrap.appendChild(slides);
    pane.appendChild(wrap);
    paintSlideBg(clone); // rev is never laid out in speaker mode, so paint the bg...
    revealStepsInClone(clone, fragUpto || 0); // ...reveal fragments/code-steps to the step...
    copyCanvases(sourceSec, clone);           // ...blit any canvas bitmaps cloneNode dropped...
    fitSlide(clone);     // ...and shrink the clone's font-size so its content fits the box
    var pr = pane.getBoundingClientRect();
    var scale = Math.min(pr.width / W, pr.height / H) || 1;
    var tx = (pr.width - W * scale) / 2, ty = (pr.height - H * scale) / 2;
    slides.style.transform = 'translate(' + tx.toFixed(1) + 'px,' + ty.toFixed(1) + 'px) scale(' + scale.toFixed(4) + ')';
  }
  function updateSpeakerUI() {
    var c = currentSlide();
    snapshotInto(deck.spCur, c, deck.frag); // Current: revealed to the current step
    // Next previews the next STEP, not just the next slide: if this slide has more
    // fragments to reveal, that's this slide one step further; otherwise the next slide
    // at its base state (what you land on when you advance off this one).
    var fc = fragsOf(c).length, nextSrc = null, nextFrag = 0;
    if (deck.frag < fc) { nextSrc = c; nextFrag = deck.frag + 1; }
    else { var nx = nextIndex(deck.h, deck.v); if (nx) nextSrc = leafAt(nx.h, nx.v); }
    if (nextSrc) { snapshotInto(deck.spNext, nextSrc, nextFrag); if (deck.spNextPane) deck.spNextPane.style.visibility = ''; }
    else { if (deck.spNext) deck.spNext.textContent = ''; if (deck.spNextPane) deck.spNextPane.style.visibility = 'hidden'; }
    var notes = c && c.querySelector('.notes');
    if (deck.spNotesBody) deck.spNotesBody.innerHTML = notes ? notes.innerHTML : '<span class="sp-empty">No notes for this slide.</span>';
    // Per-slide readout: position in the deck + this slide's script estimate (or none).
    if (deck.spSlideMeta) {
      var all = allSlides(), idx = c ? all.indexOf(c) + 1 : 0;
      var secs = c && c.getAttribute('data-script-secs');
      deck.spSlideMeta.textContent = 'slide ' + idx + ' / ' + all.length + ' · ' +
        (secs ? '~' + fmtClock(parseInt(secs, 10)) : 'no script');
    }
  }
  // `M:SS` for a duration in seconds (elapsed timer + script estimates).
  /** @param {number} secs */
  function fmtClock(secs) { var s = Math.max(0, Math.floor(secs)); return Math.floor(s / 60) + ':' + ('0' + (s % 60)).slice(-2); }
  // The deck's planned narration length: the sum of every slide's `data-script-secs`
  // (word-count / wpm) estimate, emitted server-side. 0 when no slide carries notes.
  function plannedSecs() {
    return allSlides().reduce(function (t, s) {
      var v = parseInt(s.getAttribute('data-script-secs') || '', 10);
      return t + (isNaN(v) ? 0 : v);
    }, 0);
  }
  function updateSpeakerClock() {
    var t = document.querySelector('.tali-speaker .sp-timer');
    var c = document.querySelector('.tali-speaker .sp-clock');
    if (t) t.textContent = fmtClock((Date.now() - deck.spStart) / 1000);
    if (c) c.textContent = new Date().toLocaleTimeString();
  }
  function initSpeaker() {
    document.title = 'Speaker · ' + document.title;
    var rev = deckEl(); if (rev) rev.style.display = 'none'; // keep as data source for notes/counts
    var root = document.createElement('div');
    root.className = 'tali-speaker';
    root.innerHTML =
      '<div class="sp-top">' +
        '<div class="sp-timer">0:00</div><div class="sp-plan"></div><div class="sp-slidemeta"></div>' +
        '<button class="sp-read" type="button" aria-pressed="false" title="Read view for recording (r)">Read</button>' +
        '<span class="sp-size"><button class="sp-size-dn" type="button" title="Smaller script">A−</button>' +
        '<button class="sp-size-up" type="button" title="Larger script">A+</button></span>' +
        '<button class="sp-reset" type="button">Reset</button><div class="sp-clock"></div>' +
      '</div>' +
      '<div class="sp-stage">' +
        '<div class="sp-pane"><div class="sp-label">Current</div><div class="sp-frame-cur"></div></div>' +
        '<div class="sp-pane sp-pane-next"><div class="sp-label">Next</div><div class="sp-frame-next"></div></div>' +
      '</div>' +
      '<div class="sp-notes"><div class="sp-label">Notes</div><div class="sp-notes-body"></div></div>';
    document.body.appendChild(root);
    deck.spCur = root.querySelector('.sp-frame-cur');
    deck.spNext = root.querySelector('.sp-frame-next');
    deck.spNextPane = root.querySelector('.sp-pane-next');
    deck.spNotesBody = root.querySelector('.sp-notes-body');
    deck.spPlan = root.querySelector('.sp-plan');
    deck.spSlideMeta = root.querySelector('.sp-slidemeta');
    // The planned total is fixed for the deck (elapsed ticks against it in the timer).
    var planned = plannedSecs();
    if (deck.spPlan) deck.spPlan.textContent = planned > 0 ? '/ ~' + fmtClock(planned) : '';
    // Read view: the script becomes the large primary surface (previews shrink to a
    // thumbnail) so the author reads comfortably while recording; `A- / A+` size it.
    var readBtn = /** @type {HTMLElement} */ (root.querySelector('.sp-read'));
    function toggleRead() {
      var on = root.classList.toggle('read');
      readBtn.setAttribute('aria-pressed', on ? 'true' : 'false');
      requestAnimationFrame(updateSpeakerUI); // re-fit the resized thumbnail once laid out
    }
    readBtn.addEventListener('click', toggleRead);
    /** @param {number} d */
    function bumpSize(d) {
      var cur = parseInt(getComputedStyle(root).getPropertyValue('--sp-read-size'), 10) || 34;
      root.style.setProperty('--sp-read-size', Math.max(20, Math.min(72, cur + d)) + 'px');
    }
    var dn = root.querySelector('.sp-size-dn'), up = root.querySelector('.sp-size-up');
    if (dn) dn.addEventListener('click', function () { bumpSize(-4); });
    if (up) up.addEventListener('click', function () { bumpSize(4); });
    // `r` toggles the read view (this window has no text inputs; plain `r` is unbound
    // in onKey, which never preventDefaults it).
    document.addEventListener('keydown', function (e) {
      if ((e.key === 'r' || e.key === 'R') && !e.ctrlKey && !e.metaKey && !e.altKey) toggleRead();
    });
    // Panes are snapshot-rendered from this window's own deck copy, so paint them now and
    // again once late {js}/KaTeX output settles (window load) and whenever the pane resizes.
    updateSpeakerUI();
    window.addEventListener('load', updateSpeakerUI);
    var spResizeRAF = /** @type {number | null} */ (null);
    window.addEventListener('resize', function () {
      if (spResizeRAF) return;
      spResizeRAF = requestAnimationFrame(function () { spResizeRAF = null; updateSpeakerUI(); });
    });
    document.addEventListener('keydown', onKey);
    window.addEventListener('message', onMessage);
    deck.spStart = Date.now();
    /** @type {HTMLElement} */ (root.querySelector('.sp-reset')).addEventListener('click', function () { deck.spStart = Date.now(); updateSpeakerClock(); });
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

  // --- URL hash (replaceState: no history pollution) ---------------------
  function writeHash() {
    if (!deck.config.hash) return;
    var c = currentSlide();
    // #/<slide>[/<v>]/<frag>: append the in-slide step index when past step 0 so a deep
    // link restores the exact fragment. A numeric slide includes its `v` when a frag
    // follows (keeping the frag slot unambiguous); a named slide takes `/<frag>`.
    /** @type {Array<string | number>} */
    var parts = c && c.id ? [c.id] : [deck.h];
    if (deck.feed) {
      // In the feed every slide is fully shown, so the position is just the slide (no
      // fragment step): keeps the deep-link clean (`#/code-on-a-slide`) and round-trips.
      if (!(c && c.id) && deck.v) parts.push(deck.v);
    } else {
      if (!(c && c.id) && (deck.v || deck.frag > 0)) parts.push(deck.v);
      if (deck.frag > 0) parts.push(deck.frag);
    }
    var frag = parts.join('/');
    var url = '#/' + frag;
    // Preserve any `{{< input >}}` state suffix (C-ADD-3): inside a deck qmd-js.js writes
    // control state as a `?k=v&...` suffix after this position prefix. The deck owns the
    // prefix, qmd-js owns the suffix, and each rewrite carries the other's segment through
    // untouched — so navigating a slide never drops a shared control value and vice-versa.
    var q = location.hash.split('?')[1];
    if (q) url += '?' + q;
    if (url === location.hash) return;
    history.replaceState(null, '', url); // deep-link without polluting back/forward history
  }
  function readHash() {
    // Drop the `{{< input >}}` state suffix (C-ADD-3) before parsing the deck position;
    // qmd-js.js reads that suffix. `?` can't appear in a position (ids are slugs, indices
    // digits) or an encoded control value, so the split is unambiguous.
    var raw = location.hash.replace(/^#\/?/, '').split('?')[0];
    if (!raw) return false;
    var parts = raw.split('/');
    var fragPart;
    // An id wins over an index regardless of shape: a digit-leading slug (`3-ways`) or a
    // purely-numeric heading (`## 2024` -> id `2024`) is a real element id, NOT a slide
    // number. Climb from the target to its containing leaf slide; an anchor with no slide
    // (a footnote / `@fig-`/`@sec-` xref / off-deck target) is left alone (return false)
    // so the browser does its normal in-page jump instead of snapping the deck to slide 0.
    var el = document.getElementById(parts[0]);
    if (el) {
      var slide = el.closest && el.closest('.tali-slides section');
      if (!slide) return false;
      var ix = indexOf(slide);
      deck.h = ix.h; deck.v = ix.v;
      fragPart = parts[1]; // named slide: the fragment index follows the id
    } else if (/^\d+$/.test(parts[0])) {
      deck.h = parseInt(parts[0], 10) || 0;
      deck.v = parseInt(parts[1], 10) || 0;
      fragPart = parts[2];
    } else {
      return false; // an unknown non-numeric target: not ours to handle
    }
    var f = parseInt(fragPart || '', 10);
    deck.pendingFrag = isNaN(f) ? null : f; // consumed by onHashChange / init
    return true;
  }
  /** @param {Element | null} el */
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
    if (deck.feed) {
      // In the feed a hash change (back/forward, a shared link) scrolls to the slide;
      // native scroll then settles the observer. Our own writeHash uses replaceState (no
      // hashchange), so this only runs on a genuine external navigation.
      if (deck.h === ph && deck.v === pv) return;
      deck.frag = fragCount();
      scrollToCurrent(true); updateNumber();
      broadcastState();
      return;
    }
    var target = deck.pendingFrag;
    // A same-position hashchange (back/forward landing where we already are): nothing moved.
    // `target == null` is only that when fragments were at 0 (writeHash omits the frag
    // segment only then); a genuine re-nav to the current slide with fragments partly
    // shown (pf > 0) must fall through to re-apply (deck.frag = fc below).
    if (deck.h === ph && deck.v === pv && ((target == null && pf === 0) || target === pf)) return;
    if (deck.blackout) toggleBlackout(false); // an external slide change lifts the curtain
    var fc = fragCount();
    // Restore the linked fragment step; without one (a plain slide link) show them all.
    deck.frag = target != null ? Math.max(0, Math.min(target, fc)) : fc;
    renderMove(); updateNumber(); fire('slidechanged'); // morph or pan to the linked slide
    broadcastState(); // keep the speaker window in sync on hash (back/forward) nav
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
    el.textContent = (all.indexOf(/** @type {HTMLElement} */ (currentSlide())) + 1) + ' / ' + all.length;
  }

  // a11y: name each leaf slide "Slide N of M" (the server-side <section> already carries
  // role="group" + aria-roledescription="slide", but only JS knows the flat order across
  // vertical stacks), and announce the current slide through a polite live region so a
  // screen-reader user hears the position change on every navigation. Re-run on every
  // slide change + after a live edit re-splits the deck, so the count stays right.
  // The deck's polite live region (created lazily on the deck root) + a helper to speak a
  // short message through it, so slide changes, fragment steps, overview jumps, and
  // blackout are all announced to a screen reader through the one channel.
  function liveRegion() {
    var rev = deckEl();
    if (!rev) return null;
    var live = rev.querySelector('.tali-deck-live');
    if (!live) {
      live = document.createElement('div');
      live.className = 'tali-deck-live';
      live.setAttribute('aria-live', 'polite');
      live.setAttribute('aria-atomic', 'true');
      rev.appendChild(live);
    }
    return live;
  }
  /** @param {string} msg */
  function announce(msg) { var live = liveRegion(); if (live) live.textContent = msg; }
  // "Slide N of M: title" for a leaf section (empty string if it isn't a known leaf).
  /** @param {Element | null} sec */
  function slideDesc(sec) {
    var all = allSlides(), idx = all.indexOf(/** @type {HTMLElement} */ (sec));
    if (idx < 0) return '';
    var hd = sec && sec.querySelector('h1,h2,h3');
    var title = hd ? (hd.textContent || '').trim() : '';
    return 'Slide ' + (idx + 1) + ' of ' + all.length + (title ? ': ' + title : '');
  }
  function updateSlideLabels() {
    var rev = deckEl();
    if (!rev) return;
    var all = allSlides(), cur = currentSlide();
    for (var i = 0; i < all.length; i++) {
      all[i].setAttribute('aria-label', 'Slide ' + (i + 1) + ' of ' + all.length);
    }
    var desc = slideDesc(cur);
    if (desc) announce(desc);
  }

  // --- keyboard + touch ---------------------------------------------------
  /** @param {KeyboardEvent} e */
  function onKey(e) {
    // The share panel is a light-dismiss dialog over any mode (incl. the feed): Escape
    // closes it, every other key is swallowed so the deck behind doesn't act on it.
    if (deck.share && !deck.share.hasAttribute('hidden')) {
      if (e.key === 'Escape') { closeShare(); e.preventDefault(); }
      return;
    }
    if (deck.feed) return; // native scroll owns the axis in the feed (no deck key-nav)
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return;
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
        // The very last slide: the last vertical of the last stack, not v=0. Guard the
        // empty deck (lh === -1, lt undefined) so isStack(undefined).children can't throw.
        var lh = tops().length - 1, lt = tops()[lh];
        if (lt) moveTo(lh, isStack(lt) ? vertsOf(lt).length - 1 : 0, true);
        break;
      }
      case 'Escape': case 'o': if (deck.mode === 'normal') setOverview(true); break;
      case 's': openSpeaker(); break;
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
  /** @param {boolean} on */
  function toggleBlackout(on) {
    var rev = deckEl();
    var was = deck.blackout;
    deck.blackout = !!on;
    if (rev) rev.classList.toggle('tali-blackout', deck.blackout);
    if (deck.blackout) {
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
    // Announce only a real state change (many nav paths call toggleBlackout(false)
    // defensively to lift a curtain that may already be down) (WCAG 4.1.3).
    if (deck.blackout !== was) announce(deck.blackout ? 'Screen blanked' : 'Resumed');
  }
  var touch = /** @type {{ x: number | null, y: number | null, t: number }} */ ({ x: null, y: null, t: 0 });
  var ovTouch = /** @type {any} */ (null); // overview touch-gesture state: 1-finger pan or 2-finger pinch (mode-dependent shape)
  /** @param {Touch} a @param {Touch} b */
  function touchDist(a, b) { return Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY); }
  // Seat (or re-seat, when a finger is added/lifted mid-gesture) an overview touch
  // gesture: two fingers pinch-zoom, one finger pans. Carries `moved` across finger
  // changes so a pinch that decays to a one-finger drag still swallows the tap.
  /** @param {TouchEvent} e */
  function startOverviewTouch(e) {
    if (!deck.ov) fitOverview();
    var rect = /** @type {HTMLElement} */ (deckEl()).getBoundingClientRect(), moved = ovTouch ? ovTouch.moved : false;
    if (e.touches.length >= 2) {
      ovTouch = { mode: 'pinch', dist: touchDist(e.touches[0], e.touches[1]) || 1, scale: deck.ov.scale, rect: rect, moved: moved };
    } else {
      var t = e.touches[0];
      ovTouch = { mode: 'pan', x: t.clientX, y: t.clientY, cx: deck.ov.cx, cy: deck.ov.cy, rect: rect, moved: moved };
    }
  }
  /** @param {TouchEvent} e */
  function onTouchStart(e) {
    if (deck.feed) return; // native scroll owns the axis in the feed
    if (deck.overview) {
      // A moved touch fires no click to consume `ovDragged` (unlike a mouse drag), so
      // drop any stale flag as a fresh gesture begins, before a possible tile-pick tap.
      if (!ovTouch) deck.ovDragged = false;
      startOverviewTouch(e); return; // overview owns touch: pan/pinch, never nav
    }
    if (e.touches.length !== 1) { touch.x = null; return; }
    touch.x = e.touches[0].clientX; touch.y = e.touches[0].clientY; touch.t = Date.now();
  }
  // In overview, one finger pans the map and two fingers pinch-zoom toward the
  // centroid (reusing the wheel's zoom math). preventDefault keeps the browser off
  // the gesture (deck.overview sets touch-action:none, so this is cancelable).
  /** @param {TouchEvent} e */
  function onTouchMove(e) {
    if (!deck.overview || !ovTouch) return;
    e.preventDefault();
    if (ovTouch.mode === 'pinch' && e.touches.length >= 2) {
      var a = e.touches[0], b = e.touches[1], d = touchDist(a, b);
      var px = (a.clientX + b.clientX) / 2 - ovTouch.rect.left; // pinch centroid, stage coords
      var py = (a.clientY + b.clientY) / 2 - ovTouch.rect.top;
      ovTouch.moved = true;
      zoomOverviewTo(px, py, ovTouch.scale * (d / ovTouch.dist));
    } else if (ovTouch.mode === 'pan' && e.touches.length === 1) {
      var t = e.touches[0], dx = t.clientX - ovTouch.x, dy = t.clientY - ovTouch.y;
      if (!ovTouch.moved && dx * dx + dy * dy < 25) return; // 5px before it counts as a drag
      ovTouch.moved = true;
      deck.ov.cx = ovTouch.cx - dx / deck.ov.scale;
      deck.ov.cy = ovTouch.cy - dy / deck.ov.scale;
      clampOv();
      setCamera(false);
    }
  }
  /** @param {TouchEvent} e */
  function onTouchEnd(e) {
    if (deck.feed) return; // native scroll owns the axis in the feed
    if (deck.overview) {
      if (ovTouch && ovTouch.moved) deck.ovDragged = true; // a pan/pinch: swallow the click that follows
      if (e.touches && e.touches.length >= 1) startOverviewTouch(e); // re-seat from the remaining finger(s)
      else ovTouch = null;
      return; // a still tap falls through to onSlidesClick, which picks the tile
    }
    if (touch.x == null || touch.y == null) return;
    var c = e.changedTouches[0];
    var dx = c.clientX - touch.x, dy = c.clientY - touch.y, dt = Date.now() - touch.t;
    touch.x = null;
    if (dt > 600 || Math.max(Math.abs(dx), Math.abs(dy)) < 50) return;
    if (Math.abs(dx) > Math.abs(dy)) { dx < 0 ? right() : left(); }
    else { dy < 0 ? down() : up(); }
  }
  // An OS interruption (system-UI edge swipe, incoming call, palm rejection) fires
  // touchcancel, not touchend — hard-reset the gesture so a stranded `moved` flag
  // can't swallow the next tile-pick tap. No click follows a cancel, so drop ovDragged too.
  function onTouchCancel() { ovTouch = null; deck.ovDragged = false; }

  // --- mobile slide-feed (A3) --------------------------------------------
  // On a phone / portrait screen a deck opens as a vertical scroll-feed of full-
  // viewport slides. It reuses the identical slide DOM (no wrapper, no clone), so
  // block-ids, click-to-source and live {js} state survive; `html.tali-feed` swaps the
  // camera model for CSS font-size scaling + native scroll-snap. Native scroll owns the
  // vertical axis, so the deck's key/touch nav is disabled here; the counter + a
  // deep-link hash are driven from an IntersectionObserver on the centred slide.
  function isPortrait() {
    try { return window.matchMedia('(orientation: portrait)').matches; }
    catch (e) { return window.innerHeight >= window.innerWidth; }
  }
  // Reveal every fragment / incremental item, clear code line-focus, and rest each
  // magic-move on its final block — so a scrolled slide reads complete. (The CSS
  // `!important` overrides cover visibility; this also clears any JS-set focus state.)
  function revealAllForFeed() {
    var s = slidesEl(); if (!s) return;
    s.querySelectorAll(FRAG_SEL).forEach(function (el) { el.classList.add('tali-frag-visible'); });
    s.querySelectorAll('pre[data-code-lines]').forEach(function (pre) { highlightLines(pre, 'all'); });
    s.querySelectorAll('.magic-move').forEach(function (div) {
      var pres = mmBlocks(div);
      if (pres.length) { deck.animSteps = false; setOrMorphMM(div, pres.length - 1); }
    });
  }
  /** @param {Element | null} sec */
  function feedLeaf(sec) {
    // The observed target maps to a leaf index via indexOf (handles stack children).
    return indexOf(sec);
  }
  /** @param {IntersectionObserverEntry[]} entries */
  function onFeedIntersect(entries) {
    if (!deck.feed || !deck.feedRatios) return;
    entries.forEach(function (e) { deck.feedRatios.set(e.target, e.isIntersecting ? e.intersectionRatio : 0); });
    var best = null, bestR = -1;
    deck.feedRatios.forEach(function (/** @type {number} */ r, /** @type {Element} */ sec) { if (r > bestR) { bestR = r; best = sec; } });
    if (!best || bestR <= 0) return;
    var ix = feedLeaf(best);
    if (ix.h === deck.h && ix.v === deck.v) return;
    deck.h = ix.h; deck.v = ix.v; deck.frag = fragCount();
    updateNumber(); updateChrome();
    scheduleFeedHash();
    broadcastState(); // keep an open speaker window in sync as the feed scrolls
    fire('slidechanged');
  }
  function scheduleFeedHash() {
    if (deck.feedHashRAF) return;
    deck.feedHashRAF = requestAnimationFrame(function () { deck.feedHashRAF = null; writeHash(); });
  }
  function setupFeedObserver() {
    if (deck.feedIO) deck.feedIO.disconnect();
    var scroller = slidesEl(); if (!scroller || typeof IntersectionObserver === 'undefined') return;
    deck.feedRatios = new Map();
    deck.feedIO = new IntersectionObserver(onFeedIntersect, {
      root: scroller, threshold: [0, 0.25, 0.5, 0.6, 0.75, 1],
    });
    allSlides().forEach(function (s) { deck.feedIO.observe(s); });
  }
  /** @param {Element | null} sec @param {boolean} smooth */
  function scrollToSlide(sec, smooth) {
    if (!sec) return;
    try { sec.scrollIntoView({ block: 'start', behavior: smooth ? 'smooth' : 'auto' }); }
    catch (e) { sec.scrollIntoView(); }
  }
  /** @param {boolean} smooth */
  function scrollToCurrent(smooth) { scrollToSlide(currentSlide(), smooth); }
  // Enter the feed: swap to the CSS font-size layout, drop the camera's inline
  // transforms + fit sizes, reveal everything, and start the observer. Idempotent, so a
  // portrait rotation can call it. Native scroll + the always-attached key/touch guards
  // (which early-return on deck.feed) mean no listener juggling is needed.
  function enterFeed() {
    if (deck.feed) return;
    deck.feed = true;
    var rev = deckEl(), s = slidesEl();
    document.documentElement.classList.add('tali-feed');
    if (deck.aaSettle) deck.aaSettle();          // flush any in-flight auto-animate morph
    if (s) s.style.transform = '';               // CSS also forces none; clear the inline value
    tops().forEach(function (top) { top.style.transform = ''; });
    allSlides().forEach(function (sec) { sec.style.transform = ''; sec.style.removeProperty('font-size'); });
    if (deck.overview) setOverview(false);       // the feed is its own browse surface
    // A prior stepped session's blackout doesn't belong over the feed (a rotation can
    // enter the feed with it active) and can't be dismissed since onKey early-returns in
    // the feed. Mirror setOverview and lift it.
    if (deck.blackout) toggleBlackout(false);
    applyBackgrounds();
    revealAllForFeed();
    syncInert();                                 // deck.feed shows all → clear inert
    setupFeedObserver();
    updateNumber(); updateChrome();
    if (rev) rev.classList.add('tali-ready');
  }
  // Leave the feed for the stepped stage (the Present escape hatch, or a rotation to
  // landscape). Re-derives the resting stepped view on the current slide.
  function exitFeed() {
    if (!deck.feed) return;
    deck.feed = false;
    document.documentElement.classList.remove('tali-feed');
    if (deck.feedIO) { deck.feedIO.disconnect(); deck.feedIO = null; }
    if (deck.feedRatios) deck.feedRatios.clear();
    deck.frag = fragCount();                      // land fully-shown on the current slide
    // Frame instantly (applyClasses + layout's setCamera(false)), not apply()'s animated
    // setCamera(true): the feed cleared the camera transform, so an animated re-frame would
    // zoom in from the unframed identity state (the same first-paint flash as init).
    applyClasses(); layout(); updateNumber(); focusCurrent();
  }
  // A rotation may cross the portrait/landscape line: re-route only in auto mode (an
  // explicit ?qmd=feed / ?qmd=present, or an embed, is a fixed choice).
  function maybeReroute() {
    if (!deck.autoRoute) return;
    var wantFeed = isPortrait();
    if (wantFeed && !deck.feed) { enterFeed(); scrollToCurrent(false); }
    else if (!wantFeed && deck.feed) exitFeed();
  }

  // --- events + plugins (deck API) -----------------------------------
  /** @param {string} evt @param {(...a: any[]) => void} cb */
  function on(evt, cb) { (deck.listeners[evt] = deck.listeners[evt] || []).push(cb); }
  /** @param {string} evt */
  function fire(evt) {
    var detail = { h: deck.h, v: deck.v, currentSlide: currentSlide() };
    (deck.listeners[evt] || []).forEach(function (cb) { try { cb(detail); } catch (e) {} });
  }
  /** @param {any} p */
  function initPlugin(p) {
    if (!p || p.__qmdInited || typeof p.init !== 'function') return;
    p.__qmdInited = true;
    try { p.init(facade); } catch (e) {}
  }
  /** @param {any} p */
  function registerPlugin(p) { if (p) { deck.plugins.push(p); if (deck.ready) initPlugin(p); } }

  // --- offline QR encoder (C-ADD-2) ---------------------------------------
  // A self-contained, dependency-free QR encoder (byte mode, ECC level L, versions
  // 1..10) so "point a phone at the screen" works with no CDN, on file://. Verified
  // bit-for-bit against a reference encoder and by decoding the output; longer than a
  // v10-L URL throws and the caller falls back to the copy-link. Spec tables are
  // ISO/IEC 18004; the mask penalty (7.8.3) is scored before format/version placement.
  var qrEncode = (function () {
    var EC_L = [7, 10, 15, 20, 26, 18, 20, 24, 30, 18];
    var GROUPS_L = [[[1, 19]], [[1, 34]], [[1, 55]], [[1, 80]], [[1, 108]], [[2, 68]], [[2, 78]], [[2, 97]], [[2, 116]], [[2, 68], [2, 69]]];
    var DATACAP_L = [19, 34, 55, 80, 108, 136, 156, 194, 232, 274];
    var ALIGN = [[], [6, 18], [6, 22], [6, 26], [6, 30], [6, 34], [6, 22, 38], [6, 24, 42], [6, 26, 46], [6, 28, 50]];
    var FORMAT_L = [30660, 29427, 32170, 30877, 26159, 25368, 27713, 26998]; // level L, masks 0..7
    /** @type {Record<number, number>} */
    var VERSION_INFO = { 7: 31892, 8: 34236, 9: 39577, 10: 42195 };
    var REMAINDER = [0, 7, 7, 7, 7, 7, 0, 0, 0, 0]; // remainder bits, v1..10
    var EXP = new Array(512), LOG = new Array(256);
    (function () { var x = 1; for (var i = 0; i < 255; i++) { EXP[i] = x; LOG[x] = i; x <<= 1; if (x & 0x100) x ^= 0x11d; } for (var j = 255; j < 512; j++) EXP[j] = EXP[j - 255]; })();
    /** @param {number} a @param {number} b */
    function gmul(a, b) { return (a === 0 || b === 0) ? 0 : EXP[LOG[a] + LOG[b]]; }
    /** @param {number} n @returns {number[]} */
    function rsGen(n) { // generator poly, high-degree-first, excluding the leading 1
      var g = /** @type {number[]} */ ([1]);
      for (var i = 0; i < n; i++) { var ng = /** @type {number[]} */ (new Array(g.length + 1).fill(0)); for (var j = 0; j < g.length; j++) { ng[j + 1] ^= g[j]; ng[j] ^= gmul(g[j], EXP[i]); } g = ng; }
      var out = /** @type {number[]} */ ([]); for (var k = g.length - 2; k >= 0; k--) out.push(g[k]); return out;
    }
    /** @param {number[]} data @param {number} n @returns {number[]} */
    function rsEnc(data, n) {
      var gen = rsGen(n), res = data.slice(); for (var z = 0; z < n; z++) res.push(0);
      for (var k = 0; k < data.length; k++) { var coef = res[k]; if (coef !== 0) for (var m = 0; m < n; m++) res[k + m + 1] ^= gmul(coef, gen[m]); }
      return res.slice(data.length);
    }
    /** @param {string} str @returns {number[]} */
    function utf8(str) {
      var b = /** @type {number[]} */ ([]);
      for (var i = 0; i < str.length; i++) {
        var c = str.charCodeAt(i);
        if (c < 0x80) b.push(c);
        else if (c < 0x800) b.push(0xc0 | (c >> 6), 0x80 | (c & 0x3f));
        else if (c >= 0xd800 && c <= 0xdbff) { var c2 = str.charCodeAt(++i), cp = 0x10000 + ((c & 0x3ff) << 10) + (c2 & 0x3ff); b.push(0xf0 | (cp >> 18), 0x80 | ((cp >> 12) & 0x3f), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f)); }
        else b.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f));
      }
      return b;
    }
    /** @param {string} text */
    function encode(text) {
      var bytes = utf8(text), ver = 0, ccBits = 8;
      for (var v = 1; v <= 10; v++) { ccBits = v < 10 ? 8 : 16; if (4 + ccBits + 8 * bytes.length <= DATACAP_L[v - 1] * 8) { ver = v; break; } }
      if (!ver) throw new Error('QR: data too long');
      var bits = /** @type {number[]} */ ([]);
      /** @param {number} val @param {number} len */
      function put(val, len) { for (var i = len - 1; i >= 0; i--) bits.push((val >> i) & 1); }
      put(0x4, 4); put(bytes.length, ccBits);
      for (var i = 0; i < bytes.length; i++) put(bytes[i], 8);
      var cap = DATACAP_L[ver - 1] * 8;
      for (var t = 0; t < 4 && bits.length < cap; t++) bits.push(0);
      var padBits = 8 - (bits.length % 8); for (var pb = 0; pb < padBits; pb++) bits.push(0);
      var data = /** @type {number[]} */ ([]);
      for (var k = 0; k < bits.length; k += 8) { var by = 0; for (var mm = 0; mm < 8; mm++) by = (by << 1) | bits[k + mm]; data.push(by); }
      var padcw = [0xec, 0x11], pi = 0; while (data.length < DATACAP_L[ver - 1]) data.push(padcw[pi++ % 2]);
      var ec = EC_L[ver - 1], blocks = /** @type {number[][]} */ ([]), eccs = /** @type {number[][]} */ ([]), off = 0;
      GROUPS_L[ver - 1].forEach(function (grp) { for (var b = 0; b < grp[0]; b++) { var blk = data.slice(off, off + grp[1]); off += grp[1]; blocks.push(blk); eccs.push(rsEnc(blk, ec)); } });
      var maxData = 0; blocks.forEach(function (b) { if (b.length > maxData) maxData = b.length; });
      var out = /** @type {number[]} */ ([]);
      for (var c = 0; c < maxData; c++) blocks.forEach(function (b) { if (c < b.length) out.push(b[c]); });
      for (var e = 0; e < ec; e++) eccs.forEach(function (b) { out.push(b[e]); });
      var seq = /** @type {number[]} */ ([]);
      out.forEach(function (by) { for (var i2 = 7; i2 >= 0; i2--) seq.push((by >> i2) & 1); });
      for (var r = 0; r < REMAINDER[ver - 1]; r++) seq.push(0);
      return buildMatrix(ver, seq);
    }
    /** @param {number} ver @param {number[]} seq */
    function buildMatrix(ver, seq) {
      var size = ver * 4 + 17, mods = /** @type {boolean[][]} */ ([]), fn = /** @type {boolean[][]} */ ([]);
      for (var i = 0; i < size; i++) { mods.push(new Array(size).fill(false)); fn.push(new Array(size).fill(false)); }
      /** @param {number} r @param {number} c @param {boolean} v */
      function set(r, c, v) { mods[r][c] = v; fn[r][c] = true; }
      /** @param {number} r @param {number} c */
      function finder(r, c) {
        for (var dr = -1; dr <= 7; dr++) for (var dc = -1; dc <= 7; dc++) {
          var rr = r + dr, cc = c + dc; if (rr < 0 || rr >= size || cc < 0 || cc >= size) continue;
          var inring = (dr >= 0 && dr <= 6 && (dc === 0 || dc === 6)) || (dc >= 0 && dc <= 6 && (dr === 0 || dr === 6));
          set(rr, cc, inring || (dr >= 2 && dr <= 4 && dc >= 2 && dc <= 4));
        }
      }
      finder(0, 0); finder(0, size - 7); finder(size - 7, 0);
      for (var t = 8; t < size - 8; t++) { set(6, t, t % 2 === 0); set(t, 6, t % 2 === 0); }
      var ap = ALIGN[ver - 1];
      if (ap.length) {
        var mn = ap[0], mx = ap[ap.length - 1];
        for (var ai = 0; ai < ap.length; ai++) for (var aj = 0; aj < ap.length; aj++) {
          var r = ap[ai], c = ap[aj];
          if ((r === mn && c === mn) || (r === mn && c === mx) || (r === mx && c === mn)) continue;
          for (var dr2 = -2; dr2 <= 2; dr2++) for (var dc2 = -2; dc2 <= 2; dc2++) set(r + dr2, c + dc2, Math.max(Math.abs(dr2), Math.abs(dc2)) !== 1);
        }
      }
      reserveFormat(fn, size);
      if (ver >= 7) reserveVersion(fn, size);
      var idx = 0, upward = true;
      for (var col = size - 1; col > 0; col -= 2) {
        if (col === 6) col--;
        for (var row2 = 0; row2 < size; row2++) {
          var rr2 = upward ? size - 1 - row2 : row2;
          for (var dc3 = 0; dc3 < 2; dc3++) { var cc2 = col - dc3; if (fn[rr2][cc2]) continue; mods[rr2][cc2] = idx < seq.length ? seq[idx] === 1 : false; idx++; }
        }
        upward = !upward;
      }
      var bestMask = 0, bestPen = Infinity, bestMods = /** @type {boolean[][] | null} */ (null);
      for (var mask = 0; mask < 8; mask++) { var m = applyMask(mods, fn, size, mask), pen = penalty(m, size); if (pen < bestPen) { bestPen = pen; bestMask = mask; bestMods = m; } }
      placeFormat(/** @type {boolean[][]} */ (bestMods), size, FORMAT_L[bestMask]);
      if (ver >= 7) placeVersion(/** @type {boolean[][]} */ (bestMods), size, VERSION_INFO[ver]);
      return { size: size, mods: bestMods };
    }
    /** @param {boolean[][]} fn @param {number} size */
    function reserveFormat(fn, size) {
      for (var i = 0; i <= 8; i++) if (i !== 6) { fn[8][i] = true; fn[i][8] = true; }
      for (var j = 0; j < 8; j++) { fn[8][size - 1 - j] = true; fn[size - 1 - j][8] = true; }
    }
    /** @param {boolean[][]} fn @param {number} size */
    function reserveVersion(fn, size) { for (var i = 0; i < 6; i++) for (var j = 0; j < 3; j++) { fn[i][size - 11 + j] = true; fn[size - 11 + j][i] = true; } }
    /** @param {boolean[][]} src @param {boolean[][]} fn @param {number} size @param {number} mask @returns {boolean[][]} */
    function applyMask(src, fn, size, mask) {
      var m = /** @type {boolean[][]} */ ([]);
      for (var r = 0; r < size; r++) {
        m.push(src[r].slice());
        for (var c = 0; c < size; c++) {
          if (fn[r][c]) continue;
          var f = false;
          switch (mask) {
            case 0: f = (r + c) % 2 === 0; break; case 1: f = r % 2 === 0; break;
            case 2: f = c % 3 === 0; break; case 3: f = (r + c) % 3 === 0; break;
            case 4: f = (Math.floor(r / 2) + Math.floor(c / 3)) % 2 === 0; break;
            case 5: f = ((r * c) % 2 + (r * c) % 3) === 0; break;
            case 6: f = (((r * c) % 2 + (r * c) % 3) % 2) === 0; break;
            case 7: f = (((r + c) % 2 + (r * c) % 3) % 2) === 0; break;
          }
          if (f) m[r][c] = !m[r][c];
        }
      }
      return m;
    }
    /** @param {boolean[][]} m @param {number} size @param {number} bits */
    function placeFormat(m, size, bits) {
      /** @param {number} i */
      function bit(i) { return ((bits >> i) & 1) === 1; }
      for (var c = 0; c <= 5; c++) m[8][c] = bit(14 - c);
      m[8][7] = bit(8); m[8][8] = bit(7); m[7][8] = bit(6);
      for (var r = 0; r <= 5; r++) m[r][8] = bit(r);
      for (var i = 0; i <= 7; i++) m[8][size - 1 - i] = bit(i);
      for (var j = 0; j <= 6; j++) m[size - 1 - j][8] = bit(14 - j);
      m[size - 8][8] = true;
    }
    /** @param {boolean[][]} m @param {number} size @param {number} bits */
    function placeVersion(m, size, bits) {
      for (var i = 0; i < 18; i++) { var b = ((bits >> i) & 1) === 1, r = Math.floor(i / 3), c = i % 3; m[r][size - 11 + c] = b; m[size - 11 + c][r] = b; }
    }
    /** @param {Array<number|boolean>} seq @param {boolean[]} pat @param {number} from */
    function findPat(seq, pat, from) {
      for (var i = from; i <= seq.length - pat.length; i++) { var ok = true; for (var j = 0; j < pat.length; j++) if (!!seq[i + j] !== pat[j]) { ok = false; break; } if (ok) return i; }
      return -1;
    }
    /** @param {Array<number|boolean>} seq @param {number} size */
    function n3Line(seq, size) {
      var pat = [true, false, true, true, true, false, true], count = 0, idx = findPat(seq, pat, 0);
      while (idx !== -1) {
        var offset = idx + 7, beforeDark = false, afterDark = false, k;
        for (k = Math.max(idx - 4, 0); k < idx; k++) if (seq[k]) { beforeDark = true; break; }
        for (k = offset; k < Math.min(offset + 4, size); k++) if (seq[k]) { afterDark = true; break; }
        if (idx === 0 || idx === size - 7 || !beforeDark || !afterDark) count += 40; else offset = idx + 4;
        idx = findPat(seq, pat, offset);
      }
      return count;
    }
    /** @param {boolean[][]} m @param {number} size */
    function penalty(m, size) {
      var n1 = 0, n2 = 0, n3 = 0, dark = 0, r, c;
      for (r = 0; r < size; r++) {
        var rowRun = 1, colRun = 1, col = /** @type {boolean[]} */ (new Array(size));
        for (c = 0; c < size; c++) {
          col[c] = m[c][r]; dark += m[r][c] ? 1 : 0;
          if (c > 0) {
            if (m[r][c] === m[r][c - 1]) rowRun++; else { if (rowRun >= 5) n1 += rowRun - 2; rowRun = 1; }
            if (m[c][r] === m[c - 1][r]) colRun++; else { if (colRun >= 5) n1 += colRun - 2; colRun = 1; }
          }
          if (r > 0 && c > 0 && m[r][c] === m[r][c - 1] && m[r][c] === m[r - 1][c] && m[r][c] === m[r - 1][c - 1]) n2 += 3;
        }
        if (rowRun >= 5) n1 += rowRun - 2;
        if (colRun >= 5) n1 += colRun - 2;
        n3 += n3Line(m[r], size); n3 += n3Line(col, size);
      }
      var pct = dark * 100 / (size * size);
      return n1 + n2 + n3 + 10 * Math.floor(Math.abs(pct - 50) / 5);
    }
    return encode;
  })();
  // Render an encoded QR to a self-contained, theme-independent SVG string (always
  // black-on-white with a 4-module quiet zone, whatever the deck theme — scanners need
  // that contrast). Returns null if the text won't fit a v10-L symbol.
  /** @param {string} text */
  function qrSvg(text) {
    var q; try { q = qrEncode(text); } catch (e) { return null; }
    var mods = /** @type {boolean[][]} */ (q.mods); // non-null once encode() returned without throwing
    var n = q.size, qz = 4, dim = n + qz * 2, d = '';
    for (var r = 0; r < n; r++) for (var c = 0; c < n; c++) if (mods[r][c]) d += 'M' + (c + qz) + ' ' + (r + qz) + 'h1v1h-1z';
    return '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ' + dim + ' ' + dim +
      '" shape-rendering="crispEdges" role="img" aria-label="QR code linking to this view">' +
      '<rect width="' + dim + '" height="' + dim + '" fill="#fff"/><path d="' + d + '" fill="#000"/></svg>';
  }

  // --- on-screen chrome: control menu, progress bar, nav arrows -----------
  // The deck's actions (overview, speaker, fullscreen, dark mode) were keyboard-only
  // and so undiscoverable; this surfaces them in a corner menu plus a progress bar +
  // prev/next arrows. Built once in normal mode; auto-hides on idle. Fixed to the
  // viewport (not the scaled .tali-slides), so it doesn't ride the deck transform.
  /** @param {string} p */
  function svg(p) { return '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' + p + '</svg>'; }
  var IC = {
    menu: svg('<path d="M4 7h16M4 12h16M4 17h16"/>'),
    grid: svg('<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/>'),
    speak: svg('<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>'),
    fs: svg('<path d="M8 3H5a2 2 0 0 0-2 2v3M16 3h3a2 2 0 0 1 2 2v3M8 21H5a2 2 0 0 1-2-2v-3M16 21h3a2 2 0 0 0 2-2v-3"/>'),
    moon: svg('<path d="M21 12.8A8 8 0 1 1 11.2 3a6 6 0 0 0 9.8 9.8z"/>'),
    present: svg('<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M10 8l5 3-5 3z"/><path d="M8 21h8"/>'),
    share: svg('<rect x="4" y="4" width="6" height="6" rx="1"/><rect x="14" y="4" width="6" height="6" rx="1"/><rect x="4" y="14" width="6" height="6" rx="1"/><path d="M14 14h3v3M20 14v6M14 20h3"/>'),
  };
  /** @param {any} s */
  function esc(s) { return String(s).replace(/[&<>"]/g, function (c) { return /** @type {Record<string, string>} */ ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]; }); }
  /** @param {string} action @param {string} ico @param {string} label @param {string} [hint] */
  function tool(action, ico, label, hint) {
    return '<button class="tali-menu-item" data-action="' + action + '"><span class="tali-menu-ico">' + ico +
      '</span><span class="tali-menu-label">' + label + '</span>' + (hint ? '<span class="tali-menu-hint">' + hint + '</span>' : '') + '</button>';
  }
  /** @param {string} k @param {string} d */
  function key(k, d) { return '<div class="tali-key"><kbd>' + k + '</kbd><span>' + d + '</span></div>'; }
  var KEYS_HTML =
    key('← →', 'Navigate') + key('↑ ↓', 'Jump topic') + key('Space', 'Next') +
    key('Home End', 'First / last slide') +
    key('O', 'Overview') + key('0', 'Fit map') + key('F', 'Fullscreen') + key('S', 'Speaker view') +
    key('B', 'Black screen') +
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
      '<button class="tali-ctl tali-ctl-menu" aria-label="Menu" title="Menu (m)" aria-haspopup="dialog" aria-expanded="false">' + IC.menu + '</button>';
    rev.appendChild(ctl);
    /** @type {HTMLElement} */ (ctl.querySelector('.tali-ctl-prev')).addEventListener('click', function () { prev(); });
    /** @type {HTMLElement} */ (ctl.querySelector('.tali-ctl-next')).addEventListener('click', function () { next(); });
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
    // A light-dismiss popover (Esc + click-away, non-modal): a non-modal dialog, not an
    // ARIA menu — the items are plain buttons, not menuitems. role="dialog" matches the
    // launcher's aria-haspopup="dialog" so the popup's type isn't misannounced.
    menu.setAttribute('role', 'dialog');
    menu.setAttribute('aria-label', 'Slide navigation and view options');
    menu.setAttribute('hidden', '');
    // A 3-state Auto/Light/Dark segment (mirrors the page's reader theme control); "Auto" clears
    // the stored key so the deck resumes following the OS. Standalone decks only — an embedded
    // deck follows its host, so `taliDeckEmbedded` suppresses the row.
    /** @param {string} v @param {string} l */
    var themeOpt = function (v, l) {
      return '<button class="tali-theme-opt" data-theme-choice="' + v + '" aria-pressed="false">' + l + '</button>';
    };
    var themeRow = (window.taliDeckThemeManaged && !window.taliDeckEmbedded)
      ? '<div class="tali-menu-head">Theme</div>' +
        '<div class="tali-theme-seg" role="group" aria-label="Theme">' +
        themeOpt('auto', 'Auto') + themeOpt('light', 'Light') + themeOpt('dark', 'Dark') +
        '</div>'
      : '';
    menu.innerHTML =
      '<div class="tali-menu-head">Slides</div><div class="tali-menu-slides"></div>' +
      '<div class="tali-menu-head">Tools</div><div class="tali-menu-tools">' +
        tool('present', IC.present, 'Present') + // feed-only (CSS-hidden in stepped mode)
        tool('overview', IC.grid, 'Overview', 'O') +
        tool('share', IC.share, 'Share this view') +
        tool('speaker', IC.speak, 'Speaker view', 'S') +
        tool('fullscreen', IC.fs, 'Fullscreen', 'F') +
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
      var label = hd ? (hd.textContent || '').trim() : ('Slide ' + (i + 1));
      html += '<button class="tali-menu-slide' + (all[i] === cur ? ' tali-on' : '') + '" data-i="' + i + '">' +
        '<span class="tali-menu-slide-n">' + (i + 1) + '</span><span class="tali-menu-slide-t">' + esc(label) + '</span></button>';
    }
    box.innerHTML = html;
    var on = box.querySelector('.tali-on');
    if (on && on.scrollIntoView) on.scrollIntoView({ block: 'nearest' });
  }
  function markActiveTools() {
    if (!deck.menu) return;
    /** @param {string} action @param {any} on */
    var set = function (action, on) {
      var b = deck.menu.querySelector('[data-action="' + action + '"]');
      if (b) b.classList.toggle('tali-on', !!on);
    };
    set('overview', deck.overview);
    updateThemeSeg();
  }
  // Reflect the current theme CHOICE (auto/light/dark) on the segment, pressing the active one.
  function updateThemeSeg() {
    if (!deck.menu || !window.taliDeckThemeChoice) return;
    var cur = window.taliDeckThemeChoice();
    var btns = deck.menu.querySelectorAll('.tali-theme-opt');
    for (var i = 0; i < btns.length; i++) {
      var on = btns[i].getAttribute('data-theme-choice') === cur;
      btns[i].setAttribute('aria-pressed', on ? 'true' : 'false');
      btns[i].classList.toggle('tali-on', on);
    }
  }
  /** @param {MouseEvent} e */
  function onMenuClick(e) {
    var t = /** @type {Element | null} */ (e.target);
    var slide = t && t.closest('.tali-menu-slide');
    if (slide) { jumpToIndex(parseInt(slide.getAttribute('data-i') || '', 10)); return; }
    var opt = t && t.closest('.tali-theme-opt');
    if (opt) { setThemeChoice(opt.getAttribute('data-theme-choice')); return; } // stay open; reflects state
    var item = t && t.closest('.tali-menu-item');
    if (!item) return;
    var a = item.getAttribute('data-action');
    toggleMenu(false);
    // Present / Overview from the feed are a MANUAL mode choice: pin it (like ?qmd=present)
    // so a later resize's maybeReroute() can't auto-snap the user back into the feed.
    if (a === 'present') { deck.autoRoute = false; exitFeed(); }
    else if (a === 'overview') { if (deck.feed) { deck.autoRoute = false; exitFeed(); } setOverview(true); }
    else if (a === 'share') openShare();
    else if (a === 'speaker') openSpeaker();
    else if (a === 'fullscreen') toggleFullscreen();
  }
  /** @param {boolean} [force] */
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
      // If focus was inside the popover when it closed, return it to the launcher so a
      // keyboard user isn't dropped to <body> (WCAG 2.4.3). A navigation triggered from
      // the menu (jumpToIndex) re-focuses the target slide afterward, which wins.
      var focusInMenu = deck.menu.contains(document.activeElement);
      deck.menu.setAttribute('hidden', ''); deck.menuBackdrop.setAttribute('hidden', '');
      if (focusInMenu && deck.menuBtn) deck.menuBtn.focus();
    }
  }
  /** @param {number} i */
  function jumpToIndex(i) {
    var all = allSlides(), el = all[i];
    if (!el) return;
    var ix = indexOf(el);
    toggleMenu(false);
    if (deck.feed) {
      deck.h = ix.h; deck.v = ix.v; clampIndices(); deck.frag = fragCount();
      scrollToCurrent(true); updateNumber(); return;
    }
    if (deck.overview) setOverview(false);
    moveTo(ix.h, ix.v, true);
  }
  function toggleFullscreen() {
    try {
      if (document.fullscreenElement) document.exitFullscreen();
      else if (document.documentElement.requestFullscreen) document.documentElement.requestFullscreen();
    } catch (e) {}
  }
  // Screen Wake Lock (C-ADD-5): hold the display awake while PRESENTING so it doesn't
  // dim mid-sentence. Fullscreen is the "presenting now" signal — a deck read casually
  // in a tab (or scrolled on a phone feed) should NOT block the screensaver; going
  // fullscreen to project is the intent. No config knob; auto-follows fullscreen.
  function acquireWakeLock() {
    // Guard on `wakeLockPending` too, not just the resolved sentinel: request() is async,
    // so two syncWakeLock() calls landing inside the request window (fullscreenchange +
    // visibilitychange near-simultaneously) would each request one and orphan a sentinel
    // that then keeps the screen awake after exit. The flag closes that window.
    if (!navigator.wakeLock || deck.wakeLock || deck.wakeLockPending) return;
    deck.wakeLockPending = true;
    navigator.wakeLock.request('screen').then(function (s) {
      deck.wakeLockPending = false;
      // If we exited fullscreen while the request was in flight, don't keep the lock.
      if (!document.fullscreenElement || document.visibilityState !== 'visible') { try { s.release(); } catch (e) {} return; }
      deck.wakeLock = s;
      // The OS auto-releases the sentinel when the tab is hidden; drop our ref so
      // syncWakeLock() re-requests on the next visibility/fullscreen change.
      s.addEventListener('release', function () { if (deck.wakeLock === s) deck.wakeLock = null; });
    }).catch(function () { deck.wakeLockPending = false; }); // denied / unsupported
  }
  function releaseWakeLock() {
    if (!deck.wakeLock) return;
    try { deck.wakeLock.release(); } catch (e) {}
    deck.wakeLock = null;
  }
  // Hold the lock while fullscreen + visible; release otherwise. Re-runs on
  // fullscreenchange (project/exit) and visibilitychange (a hidden tab drops it).
  function syncWakeLock() {
    if (document.fullscreenElement && document.visibilityState === 'visible') acquireWakeLock();
    else releaseWakeLock();
  }
  // --- Share this view (C-ADD-2): a copy-link + an offline QR of the CURRENT url ---
  // location.href already deep-links the exact slide + fragment + live control state
  // (C-ADD-3), so a QR/copy of it reopens the whole view. Built lazily; refreshed on open.
  function buildShare() {
    if (deck.share) return;
    var wrap = document.createElement('div');
    wrap.className = 'tali-share';
    wrap.setAttribute('role', 'dialog');
    // A backdrop-dimmed, focus-trapped modal — declare it so AT confines to it (PA-B4); it
    // was the one dialog in the app missing aria-modal.
    wrap.setAttribute('aria-modal', 'true');
    wrap.setAttribute('aria-label', 'Share this view');
    wrap.setAttribute('hidden', '');
    wrap.innerHTML =
      '<div class="tali-share-card">' +
        '<button class="tali-share-close" aria-label="Close">' + svg('<path d="M6 6l12 12M18 6L6 18"/>') + '</button>' +
        '<div class="tali-share-head">Point a phone here</div>' +
        '<div class="tali-share-qr"></div>' +
        '<div class="tali-share-note" hidden>This link is too long for a QR code — copy it instead.</div>' +
        '<div class="tali-share-row"><input class="tali-share-url" name="tali-share-url" type="text" readonly aria-label="Link to this view">' +
        '<button class="tali-share-copy">Copy</button></div>' +
      '</div>';
    document.body.appendChild(wrap);
    var backdrop = document.createElement('div');
    backdrop.className = 'tali-share-backdrop';
    backdrop.setAttribute('hidden', '');
    document.body.appendChild(backdrop);
    backdrop.addEventListener('click', function () { closeShare(); });
    /** @type {HTMLElement} */ (wrap.querySelector('.tali-share-close')).addEventListener('click', function () { closeShare(); });
    /** @type {HTMLElement} */ (wrap.querySelector('.tali-share-copy')).addEventListener('click', copyShare);
    /** @type {HTMLElement} */ (wrap.querySelector('.tali-share-url')).addEventListener('focus', /** @this {HTMLInputElement} */ function () { this.select(); });
    deck.share = wrap; deck.shareBackdrop = backdrop;
  }
  function openShare() {
    buildShare();
    var url = location.href;
    var input = deck.share.querySelector('.tali-share-url');
    input.value = url;
    var svgStr = qrSvg(url);
    deck.share.querySelector('.tali-share-qr').innerHTML = svgStr || '';
    deck.share.querySelector('.tali-share-qr').hidden = !svgStr;      // too-long URL: no QR
    deck.share.querySelector('.tali-share-note').hidden = !!svgStr;
    var copyBtn = deck.share.querySelector('.tali-share-copy');
    copyBtn.textContent = 'Copy';
    deck.share.removeAttribute('hidden'); deck.shareBackdrop.removeAttribute('hidden');
    // The opaque backdrop makes this a real modal: mark the deck behind inert so Tab and a
    // screen reader can't reach the slides underneath while it's open (the panel lives
    // outside the deck, on <body>, so it stays reachable).
    var rev = deckEl(); if (rev) rev.inert = true;
    showChrome();
    copyBtn.focus();
  }
  function closeShare() {
    if (!deck.share || deck.share.hasAttribute('hidden')) return;
    var focusInside = deck.share.contains(document.activeElement);
    var rev = deckEl(); if (rev) rev.inert = false;
    deck.share.setAttribute('hidden', ''); deck.shareBackdrop.setAttribute('hidden', '');
    if (focusInside && deck.menuBtn) deck.menuBtn.focus(); // WCAG 2.4.3: don't drop to <body>
  }
  function copyShare() {
    var input = deck.share.querySelector('.tali-share-url');
    var btn = deck.share.querySelector('.tali-share-copy');
    var done = function () { btn.textContent = 'Copied'; };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(input.value).then(done, function () { legacyCopy(input); done(); });
    } else { legacyCopy(input); done(); }
  }
  /** @param {HTMLInputElement} input */
  function legacyCopy(input) { input.focus(); input.select(); try { document.execCommand('copy'); } catch (e) {} }
  // Apply a theme CHOICE from the segment. 'auto' clears the stored key so the deck resumes
  // following the OS (taliDeckSetTheme handles the persistence); light/dark pin it.
  /** @param {string | null} choice */
  function setThemeChoice(choice) {
    if (!window.taliDeckSetTheme) return;
    window.taliDeckSetTheme(/** @type {string} */ (choice));
    updateThemeSeg();
  }
  function updateChrome() {
    if (!deck.chrome) return;
    var all = allSlides(), idx = all.indexOf(/** @type {HTMLElement} */ (currentSlide()));
    var pct = all.length ? (idx + 1) / all.length * 100 : 0;
    deck.chrome.fill.style.width = pct + '%';
  }
  var idleTimer = /** @type {number | undefined} */ (undefined), coldOpen = true;
  function showChrome() {
    document.documentElement.classList.remove('tali-idle');
    clearTimeout(idleTimer);
    // Cold open: a reader who just landed on a static deck hasn't touched anything yet, so
    // hold the nav visible longer the FIRST time (there's no first-run hint otherwise) — long
    // enough to register that controls exist before they fade. After any interaction, revert
    // to the snappy 3s idle-hide.
    var delay = coldOpen ? 6000 : 3000;
    coldOpen = false;
    idleTimer = setTimeout(function () { if (!deck.menuOpen) document.documentElement.classList.add('tali-idle'); }, delay);
  }

  // --- lifecycle ----------------------------------------------------------
  /** @param {any} [opts] */
  function initialize(opts) {
    if (opts) for (var k in opts) (/** @type {any} */ (deck.config))[k] = opts[k];
    if (deck.ready) { sync(); return facade; } // idempotent: client.js may call again
    var rev = deckEl();
    if (!rev || !slidesEl()) return facade;
    var qmd = new URLSearchParams(location.search).get('qmd');
    deck.mode = qmd === 'speaker' ? 'speaker' : 'normal';
    var d = document.documentElement.style;
    d.setProperty('--tali-deck-w', deck.config.width + 'px');
    d.setProperty('--tali-deck-h', deck.config.height + 'px');

    // The speaker window doesn't render the deck itself; it builds the control UI.
    if (deck.mode === 'speaker') { initSpeaker(); deck.ready = true; return facade; }

    // A3: route the front door by aspect. A phone / portrait screen opens the vertical
    // slide-feed; landscape opens stepped slides. `?qmd=feed` / `?qmd=present` are transient
    // escape hatches that force one mode (no config knob); an embedded deck never feeds.
    // `taliDeckEmbedded` is unset for a custom-themed deck (its head script is skipped), so
    // fall back to the frame check.
    var embedded = (typeof window.taliDeckEmbedded !== 'undefined')
      ? window.taliDeckEmbedded : (window.self !== window.top);
    deck.autoRoute = !embedded && qmd !== 'feed' && qmd !== 'present';
    // Whether to OPEN in the feed. Don't set deck.feed here: enterFeed() owns that flag
    // and early-returns when it's already set, so pre-setting it would no-op the enter.
    var wantFeed = !embedded && (qmd === 'feed' || (deck.autoRoute && isPortrait()));

    if (!readHash()) { deck.h = 0; deck.v = 0; }
    clampIndices();
    // Restore a deep-linked fragment step (#/h/v/frag) once the slide is known.
    if (deck.pendingFrag != null) deck.frag = Math.max(0, Math.min(deck.pendingFrag, fragCount()));
    if (wantFeed) {
      enterFeed();            // sets deck.feed + the CSS layout/observer (replaces apply/layout)
      scrollToCurrent(false); // honour a deep-linked slide
    } else {
      // Frame the first slide INSTANTLY: applyClasses() does the fragment/chrome/inert
      // setup (apply() minus the camera), and layout()'s own setCamera(false) places the
      // camera with no transition. Calling apply() here instead would arm an animated
      // setCamera(true) whose transition layout()'s forced reflows then commit — so the
      // deck would visibly zoom in from the unframed identity transform on every open.
      applyClasses(); layout(); updateNumber();
    }
    rev.classList.add('tali-ready'); // show the deck now the first slide is placed
    // Coalesce a burst of resize events (a drag-resize / rotate fires many) into ONE
    // layout per animation frame — layout re-fits every slide (fitSlide measures each),
    // so running it per-event thrashed the main thread.
    var resizeRAF = /** @type {number | null} */ (null);
    window.addEventListener('resize', function () {
      if (resizeRAF) return;
      resizeRAF = requestAnimationFrame(function () {
        resizeRAF = null;
        maybeReroute();                     // a rotation may cross portrait/landscape (auto mode)
        if (deck.feed) setupFeedObserver(); // feed sizes by CSS; just refresh the IO targets
        else relayoutViewport();            // viewport-only: no per-slide re-fit (fixed design units)
      });
    });
    window.addEventListener('message', onMessage); // speaker <-> audience position sync
    if (deck.mode === 'normal') {
      document.addEventListener('keydown', onKey);
      rev.addEventListener('touchstart', onTouchStart, { passive: true });
      rev.addEventListener('touchmove', onTouchMove, { passive: false }); // overview: own pan/pinch
      rev.addEventListener('touchend', onTouchEnd, { passive: true });
      rev.addEventListener('touchcancel', onTouchCancel, { passive: true }); // OS interruption: reset gesture
      /** @type {HTMLElement} */ (slidesEl()).addEventListener('click', onSlidesClick);
      rev.addEventListener('wheel', onOverviewWheel, { passive: false }); // overview: zoom the map
      rev.addEventListener('pointerdown', onOverviewPointerDown);         // overview: drag to pan
      window.addEventListener('pointermove', onOverviewPointerMove);
      window.addEventListener('pointerup', onOverviewPointerUp);
      window.addEventListener('hashchange', onHashChange);
      // C-ADD-5: keep the screen awake while presenting (fullscreen). A hidden tab
      // auto-drops the OS lock, so re-sync on visibility too.
      document.addEventListener('fullscreenchange', syncWakeLock);
      document.addEventListener('visibilitychange', syncWakeLock);
      // If the presentation window goes away, close its speaker popup + drop the ref so
      // a stale `deck.speakerWin` isn't messaged after this window is gone.
      window.addEventListener('pagehide', function () {
        if (deck.speakerWin && !deck.speakerWin.closed) { try { deck.speakerWin.close(); } catch (e) {} }
        deck.speakerWin = null;
        releaseWakeLock();
      });
      buildChrome(); // the control menu + progress bar + nav arrows
      // Embedded in a same-origin page: follow the host's light/dark toggle live.
      if (window.taliDeckEmbedded && window.taliDeckApplyTheme) {
        try {
          new MutationObserver(/** @type {any} */ (window.taliDeckApplyTheme))
            .observe(/** @type {Window} */ (window.top).document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
        } catch (e) {}
      }
      // A deck opens AS a deck: stepped slides on landscape/desktop, the mobile
      // slide-feed on a phone/portrait screen (routed by aspect above; ?qmd=feed /
      // ?qmd=present force one). Reader/scroll mode and PDF-export mode were removed.
      // enterFeed()/apply()+layout() above already placed the first view.
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
    clampIndices();
    // A speaker window is its own ws-connected deck instance: a live block patch runs
    // sync() here too. Its stage is display:none (the panes are DOM-clone snapshots), so
    // repaint the panes from the re-split DOM — like commit()/applyRemote() already do —
    // instead of moving a camera that isn't shown.
    if (deck.mode === 'speaker') { updateSpeakerUI(); return; }
    if (deck.feed) { syncFeedLayout(); updateNumber(); return; }
    apply(); layout(); updateNumber();
  }
  // A live edit in the feed: a re-split may have added / removed / retitled slides, so
  // re-reveal fragments, repaint backgrounds, and re-observe the new leaf set — WITHOUT
  // touching the camera (there is none) or re-running fitSlide, so live {js} widgets keep
  // their identity (the same DOM nodes are reused by the block-swap).
  function syncFeedLayout() {
    applyBackgrounds();
    revealAllForFeed();
    syncInert();
    setupFeedObserver();
  }

  var facade = {
    initialize: initialize,
    configure: /** @param {any} o */ function (o) { if (o) for (var k in o) (/** @type {any} */ (deck.config))[k] = o[k]; },
    sync: sync,
    layout: layout,
    slide: /** @param {number} h @param {number} v */ function (h, v) {
      // In the feed, "go to slide" scrolls (keeps click-to-source-from-editor working);
      // native snap settles the observer, which then updates the counter + hash.
      if (deck.feed) {
        deck.h = h || 0; deck.v = v || 0; clampIndices(); deck.frag = fragCount();
        scrollToCurrent(true); updateNumber(); return;
      }
      moveTo(h || 0, v || 0, true);
    },
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
