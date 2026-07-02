// Cross-reference graph: an interactive force-directed map of the project's pages and
// their cross-page connections (a `@sec-`/`@fig-` cross-reference or a prose `.qmd` link).
// Data is `window.QMD_REF_GRAPH` (built at discovery); this draws it in a focus-trapped
// modal, click-a-node-to-navigate. Read-only — it navigates, never writes to source.
// Self-contained (no d3): a small hand-rolled force simulation, so it works on any page.
(function () {
  function data() {
    var g = window.QMD_REF_GRAPH;
    return g && g.nodes ? g : null;
  }

  var overlay = null, sim = null, raf = 0, releaseTrap = null;

  function close() {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    sim = null;
    if (releaseTrap) { releaseTrap(); releaseTrap = null; }
    if (overlay) { overlay.remove(); overlay = null; }
    document.removeEventListener('keydown', onKey, true);
  }
  function onKey(e) {
    if (e.key === 'Escape') { e.preventDefault(); close(); }
  }

  function open() {
    var g = data();
    if (!g || !g.nodes.length || overlay) return;
    var W = Math.min(window.innerWidth * 0.92, 1000);
    var H = Math.min(window.innerHeight * 0.86, 720);

    overlay = document.createElement('div');
    overlay.className = 'qmd-graph-overlay';
    overlay.setAttribute('role', 'dialog');
    overlay.setAttribute('aria-modal', 'true');
    overlay.setAttribute('aria-label', 'Reference graph');
    var panel = document.createElement('div');
    panel.className = 'qmd-graph-panel';
    panel.style.width = W + 'px';
    panel.style.height = H + 'px';
    panel.innerHTML =
      '<div class="qmd-graph-head"><span class="qmd-graph-title">Reference graph</span>' +
      '<button class="qmd-graph-close" type="button" aria-label="Close">✕</button></div>' +
      '<svg class="qmd-graph-svg" width="' + W + '" height="' + (H - 44) + '"></svg>';
    overlay.appendChild(panel);
    document.body.appendChild(overlay);

    overlay.addEventListener('pointerdown', function (e) { if (e.target === overlay) close(); });
    panel.querySelector('.qmd-graph-close').addEventListener('click', close);
    document.addEventListener('keydown', onKey, true);
    if (window.qmdFocusTrap) releaseTrap = window.qmdFocusTrap(panel);

    layout(panel.querySelector('.qmd-graph-svg'), g, W, H - 44);
  }

  // Build the sim + SVG, then run the force loop.
  function layout(svg, g, w, h) {
    var SVGNS = 'http://www.w3.org/2000/svg';
    var here = window.QMD_PAGE_URL;
    // Nodes laid out on a circle to start (deterministic, no Math.random needed).
    var n = g.nodes.length;
    var nodes = g.nodes.map(function (nd, i) {
      var a = (i / n) * 2 * Math.PI;
      return {
        u: nd.u, t: nd.t,
        x: w / 2 + Math.cos(a) * Math.min(w, h) * 0.32,
        y: h / 2 + Math.sin(a) * Math.min(w, h) * 0.32,
        vx: 0, vy: 0, deg: 0,
      };
    });
    var index = {};
    nodes.forEach(function (nd, i) { index[nd.u] = i; });
    var links = [];
    g.edges.forEach(function (e) {
      var a = index[e.s], b = index[e.t];
      if (a != null && b != null) { links.push([a, b]); nodes[a].deg++; nodes[b].deg++; }
    });

    // --- SVG elements ---
    var edgeEls = links.map(function () {
      var l = document.createElementNS(SVGNS, 'line');
      l.setAttribute('class', 'qmd-graph-edge');
      svg.appendChild(l);
      return l;
    });
    var nodeEls = nodes.map(function (nd) {
      var gEl = document.createElementNS(SVGNS, 'g');
      gEl.setAttribute('class', 'qmd-graph-node' + (nd.u === here ? ' qmd-graph-current' : ''));
      gEl.setAttribute('tabindex', '0');
      gEl.setAttribute('role', 'link');
      var r = 6 + Math.min(nd.deg, 6);
      var c = document.createElementNS(SVGNS, 'circle');
      c.setAttribute('r', r);
      var label = document.createElementNS(SVGNS, 'text');
      label.setAttribute('class', 'qmd-graph-label');
      label.setAttribute('x', r + 4);
      label.setAttribute('y', 4);
      label.textContent = nd.t;
      gEl.appendChild(c);
      gEl.appendChild(label);
      var nav = function () { location.href = (window.QMD_SITE_ROOT || '') + nd.u; };
      gEl.addEventListener('click', nav);
      gEl.addEventListener('keydown', function (e) {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); nav(); }
      });
      svg.appendChild(gEl);
      return gEl;
    });

    // --- pointer drag (pins a node under the cursor) ---
    var drag = null;
    svg.addEventListener('pointerdown', function (e) {
      var t = e.target.closest && e.target.closest('.qmd-graph-node');
      if (!t) return;
      var i = nodeEls.indexOf(t);
      if (i < 0) return;
      drag = i; nodes[i].fixed = true;
      t.setPointerCapture(e.pointerId);
    });
    svg.addEventListener('pointermove', function (e) {
      if (drag == null) return;
      var rect = svg.getBoundingClientRect();
      nodes[drag].x = e.clientX - rect.left;
      nodes[drag].y = e.clientY - rect.top;
      nodes[drag].vx = nodes[drag].vy = 0;
    });
    svg.addEventListener('pointerup', function () {
      if (drag != null) { nodes[drag].fixed = false; drag = null; }
    });

    // --- force simulation (repulsion + spring + centering, damped) ---
    var alpha = 1;
    function step() {
      alpha *= 0.985;
      // Repulsion (O(n^2); n is small — a project's pages).
      for (var i = 0; i < n; i++) {
        for (var j = i + 1; j < n; j++) {
          var dx = nodes[i].x - nodes[j].x, dy = nodes[i].y - nodes[j].y;
          var d2 = dx * dx + dy * dy || 0.01;
          var f = 1400 / d2;
          var d = Math.sqrt(d2);
          var fx = (dx / d) * f, fy = (dy / d) * f;
          nodes[i].vx += fx; nodes[i].vy += fy;
          nodes[j].vx -= fx; nodes[j].vy -= fy;
        }
      }
      // Springs along edges (target length ~90).
      links.forEach(function (lk) {
        var a = nodes[lk[0]], b = nodes[lk[1]];
        var dx = b.x - a.x, dy = b.y - a.y;
        var d = Math.sqrt(dx * dx + dy * dy) || 0.01;
        var f = (d - 90) * 0.02;
        var fx = (dx / d) * f, fy = (dy / d) * f;
        a.vx += fx; a.vy += fy; b.vx -= fx; b.vy -= fy;
      });
      // Centering + integrate + damping.
      nodes.forEach(function (nd) {
        nd.vx += (w / 2 - nd.x) * 0.01;
        nd.vy += (h / 2 - nd.y) * 0.01;
        if (nd.fixed) { nd.vx = nd.vy = 0; return; }
        nd.vx *= 0.85; nd.vy *= 0.85;
        nd.x += nd.vx * alpha * 2; nd.y += nd.vy * alpha * 2;
        nd.x = Math.max(20, Math.min(w - 20, nd.x));
        nd.y = Math.max(16, Math.min(h - 16, nd.y));
      });
      // Paint.
      links.forEach(function (lk, i) {
        var a = nodes[lk[0]], b = nodes[lk[1]];
        edgeEls[i].setAttribute('x1', a.x); edgeEls[i].setAttribute('y1', a.y);
        edgeEls[i].setAttribute('x2', b.x); edgeEls[i].setAttribute('y2', b.y);
      });
      nodeEls.forEach(function (el, i) {
        el.setAttribute('transform', 'translate(' + nodes[i].x + ',' + nodes[i].y + ')');
      });
      if (alpha > 0.02 || drag != null) raf = requestAnimationFrame(step);
    }
    raf = requestAnimationFrame(step);
  }

  // A `[data-qmd-graph]` control (chrome button / reader-menu entry) opens the map.
  document.addEventListener('click', function (e) {
    if (e.target && e.target.closest && e.target.closest('[data-qmd-graph]')) {
      e.preventDefault();
      overlay ? close() : open();
    }
  });
  window.qmdOpenGraph = open;
})();
