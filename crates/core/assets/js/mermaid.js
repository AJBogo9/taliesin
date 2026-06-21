// Mermaid diagrams as a self-contained enhancer module that registers through
// the public `window.qmdEnhancers` API — exactly how a third-party extension
// would add a renderer. Shipped by core but fully decoupled from the mount
// logic; the mermaid library itself is still fetched lazily (only when a
// `pre.mermaid` is actually present). Loaded right after code-enhance.js, so
// the registry already exists.
(function () {
  if (!window.qmdEnhancers) return; // registry (code-enhance.js) must load first

// mermaid bakes colours into the SVG at run() time, so a diagram can't be
// recoloured by CSS when the theme flips — it has to be re-rendered. The config is
// CSS-driven so a theme extension can style diagrams with no JS: set
// `--qmd-mermaid-theme` (a mermaid theme name; defaults to dark/default by mode),
// and optionally `--qmd-mermaid-{bg,node,node-border,text,line}` to tune colours
// (most effective with `--qmd-mermaid-theme: base`). Each diagram's source is
// stashed (dataset.src) so a later `qmd:themechange` can restore and re-run it.
function qmdMermaidConfig() {
  var cs = getComputedStyle(document.documentElement);
  var get = function (n) { return cs.getPropertyValue(n).trim(); };
  var dark = document.documentElement.getAttribute('data-theme') === 'dark';
  var cfg = { startOnLoad: false, theme: get('--qmd-mermaid-theme') || (dark ? 'dark' : 'default') };
  var map = {
    background: '--qmd-mermaid-bg',
    primaryColor: '--qmd-mermaid-node',
    primaryBorderColor: '--qmd-mermaid-node-border',
    primaryTextColor: '--qmd-mermaid-text',
    lineColor: '--qmd-mermaid-line',
  };
  var vars = {};
  for (var key in map) { var v = get(map[key]); if (v) vars[key] = v; }
  if (Object.keys(vars).length) cfg.themeVariables = vars;
  return cfg;
}
function qmdRunMermaid(nodes) {
  try {
    window.mermaid.initialize(qmdMermaidConfig());
    window.mermaid.run({ nodes: nodes });
  } catch (e) {}
}

function qmdRenderMermaid(root) {
  var pending = root.querySelectorAll('pre.mermaid:not([data-processed])');
  if (!pending.length) return;
  // Keep the source text so the diagram survives a theme-driven re-render.
  pending.forEach(function (p) { if (p.dataset.src == null) p.dataset.src = p.textContent; });
  if (window.mermaid) { qmdRunMermaid(pending); return; }
  if (window.__qmdMermaidLoading) return; // its onload will sweep the whole doc
  window.__qmdMermaidLoading = true;
  var s = document.createElement('script');
  s.src = '{{MERMAID}}';
  s.onload = function () {
    qmdRunMermaid(document.querySelectorAll('pre.mermaid:not([data-processed])'));
  };
  s.onerror = function () {
    // The library couldn't load (offline / blocked). Don't wedge: clear the flag so
    // a later mutation can retry, and leave each diagram's source text visible as a
    // readable fallback (it's already the <pre>'s content).
    window.__qmdMermaidLoading = false;
    document
      .querySelectorAll('pre.mermaid:not([data-processed])')
      .forEach(function (p) { p.setAttribute('data-mermaid-error', '1'); });
  };
  document.head.appendChild(s);
}
// Re-render every diagram from its stashed source under the new theme.
function qmdReRenderMermaid() {
  if (!window.mermaid) return; // not loaded yet => first render will use the theme
  var all = document.querySelectorAll('pre.mermaid');
  if (!all.length) return;
  all.forEach(function (p) {
    if (p.dataset.src == null) return;
    p.textContent = p.dataset.src;
    p.removeAttribute('data-processed');
  });
  qmdRunMermaid(document.querySelectorAll('pre.mermaid:not([data-processed])'));
}
window.addEventListener('qmd:themechange', qmdReRenderMermaid);

  window.qmdEnhancers.register(qmdRenderMermaid);
})();
