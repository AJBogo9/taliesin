// Mermaid diagrams as a self-contained enhancer module that registers through
// the public `window.taliEnhancers` API — exactly how a third-party extension
// would add a renderer. Shipped by core but fully decoupled from the mount
// logic; the mermaid library itself is still fetched lazily (only when a
// `pre.mermaid` is actually present). Loaded right after code-enhance.js, so
// the registry already exists.
(function () {
  if (!window.taliEnhancers) return; // registry (code-enhance.js) must load first

// mermaid bakes colours into the SVG at run() time, so a diagram can't be
// recoloured by CSS when the theme flips — it has to be re-rendered. The config is
// CSS-driven so a theme extension can style diagrams with no JS: set
// `--tali-mermaid-theme` (a mermaid theme name; defaults to dark/default by mode),
// and optionally `--tali-mermaid-{bg,node,node-border,text,line}` to tune colours
// (most effective with `--tali-mermaid-theme: base`). Each diagram's source is
// stashed (dataset.src) so a later `tali:themechange` can restore and re-run it.
function taliMermaidConfig() {
  var cs = getComputedStyle(document.documentElement);
  /** @param {string} n */
  var get = function (n) { return cs.getPropertyValue(n).trim(); };
  // Dark is a page's `data-theme="dark"` OR a deck's own `html.tali-deck-dark` class
  // (the deck flags dark that way via deck_theme_head, not with data-theme).
  var el = document.documentElement;
  var dark = el.getAttribute('data-theme') === 'dark' || el.classList.contains('tali-deck-dark');
  /** @type {Record<string, any>} */
  var cfg = {
    startOnLoad: false,
    theme: get('--tali-mermaid-theme') || (dark ? 'dark' : 'default'),
    // Set EXPLICITLY, not left to the library's default. Diagram source is author text
    // that reaches mermaid's parser and comes back as SVG injected into the page, so the
    // sanitiser setting is ours to own: inheriting it means a mermaid upgrade could
    // silently loosen it. 'strict' sanitises HTML in labels and disables click handlers,
    // which no Taliesin diagram uses.
    securityLevel: 'strict',
  };
  /** @type {Record<string, string>} */
  var map = {
    background: '--tali-mermaid-bg',
    primaryColor: '--tali-mermaid-node',
    primaryBorderColor: '--tali-mermaid-node-border',
    primaryTextColor: '--tali-mermaid-text',
    lineColor: '--tali-mermaid-line',
  };
  /** @type {Record<string, string>} */
  var vars = {};
  for (var key in map) { var v = get(map[key]); if (v) vars[key] = v; }
  if (Object.keys(vars).length) cfg.themeVariables = vars;
  // Render at natural width, not shrunk to the reading column: mermaid's `useMaxWidth`
  // default emits `width="100%"` (an inline attribute a stylesheet can't beat), so a wide
  // diagram scales its labels down to a few px on a narrow screen. Turning it off per
  // diagram type makes each SVG its intrinsic size, so a wide one scrolls inside its <pre>
  // (base.css `pre.mermaid { overflow-x: auto }`) — the "treat as text" behavior — while a
  // small one keeps its size, centred. Every current mermaid diagram type is listed.
  var TYPES = ['flowchart', 'sequence', 'class', 'state', 'er', 'journey', 'gantt', 'pie',
    'requirement', 'gitGraph', 'c4', 'mindmap', 'timeline', 'sankey', 'quadrantChart',
    'xyChart', 'block', 'packet', 'architecture', 'kanban'];
  TYPES.forEach(function (t) { cfg[t] = { useMaxWidth: false }; });
  return cfg;
}
/** @param {NodeListOf<Element>} nodes */
function taliRunMermaid(nodes) {
  try {
    window.mermaid.initialize(taliMermaidConfig());
    window.mermaid.run({ nodes: nodes });
  } catch (e) {}
}

// Make a failed diagram load visible: flag the <pre> and insert a styled banner right
// before it (once). The diagram's source stays in the <pre> below, so the content is
// never lost and a later successful retry can still render it. The inline styles keep the
// banner legible even on a page with no stylesheet (offline / bare).
/** @param {Element} p */
function taliMermaidShowError(p) {
  p.setAttribute('data-mermaid-error', '1');
  var prev = /** @type {Element | null} */ (p.previousSibling);
  if (prev && prev.classList &&
      prev.classList.contains('mermaid-error')) {
    return; // banner already present (idempotent on retry)
  }
  var banner = document.createElement('div');
  banner.className = 'mermaid-error';
  banner.setAttribute('role', 'alert');
  banner.setAttribute('data-mermaid-error', '1');
  banner.style.cssText =
    'border:1px solid #c0392b;border-radius:4px;padding:.5em .75em;margin:.5em 0;' +
    'color:#c0392b;background:rgba(192,57,43,.08);font-size:.9em';
  banner.textContent =
    'Diagram could not be loaded (offline or blocked). Showing the source below.';
  /** @type {Node} */ (p.parentNode).insertBefore(banner, p);
}

/** @param {ParentNode} root */
function taliRenderMermaid(root) {
  var pending = root.querySelectorAll('pre.mermaid:not([data-processed])');
  if (!pending.length) return;
  // Keep the source text so the diagram survives a theme-driven re-render.
  pending.forEach(function (p) {
    var pe = /** @type {HTMLElement} */ (p);
    if (pe.dataset.src == null) pe.dataset.src = pe.textContent || '';
  });
  if (window.mermaid) { taliRunMermaid(pending); return; }
  if (window.__qmdMermaidLoading) return; // its onload will sweep the whole doc
  window.__qmdMermaidLoading = true;
  var s = document.createElement('script');
  s.src = '{{MERMAID}}';
  s.onload = function () {
    taliRunMermaid(document.querySelectorAll('pre.mermaid:not([data-processed])'));
  };
  s.onerror = function () {
    // The library couldn't load (offline / blocked). Don't wedge: clear the flag so a
    // later mutation can retry, and make the failure VISIBLE — render a banner in each
    // diagram's place instead of leaving a silent unstyled blob of source. The original
    // source is kept below the banner so nothing is lost (and a retry can restore it).
    window.__qmdMermaidLoading = false;
    document
      .querySelectorAll('pre.mermaid:not([data-processed])')
      .forEach(taliMermaidShowError);
  };
  document.head.appendChild(s);
}
// Re-render every diagram from its stashed source under the new theme.
function taliReRenderMermaid() {
  if (!window.mermaid) return; // not loaded yet => first render will use the theme
  var all = document.querySelectorAll('pre.mermaid');
  if (!all.length) return;
  all.forEach(function (p) {
    var pe = /** @type {HTMLElement} */ (p);
    if (pe.dataset.src == null) return;
    pe.textContent = pe.dataset.src;
    pe.removeAttribute('data-processed');
  });
  taliRunMermaid(document.querySelectorAll('pre.mermaid:not([data-processed])'));
}
window.addEventListener('tali:themechange', taliReRenderMermaid);

  window.taliEnhancers.register(taliRenderMermaid);
})();
