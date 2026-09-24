// Mermaid diagrams as a self-contained enhancer module that registers through
// the public `window.taliEnhancers` API — exactly how a third-party extension
// would add a renderer. Shipped by core but fully decoupled from the mount
// logic; the mermaid library itself is still fetched lazily (only when a
// `pre.mermaid` is actually present). Loaded right after code-enhance.js, so
// the registry already exists.
(function () {
  if (!window.taliEnhancers) return; // registry (code-enhance.js) must load first

// mermaid bakes colours into the SVG at run() time, so a diagram can't be
// recoloured by CSS when the theme flips — it has to be re-rendered. Diagrams use
// Mermaid's own `neutral` and `dark` themes: grey for flowcharts, sequence, class and
// state diagrams (Gantt `crit`/`active` tasks keep Mermaid's red and blue). Each diagram's source is stashed (dataset.src) so a later
// `tali:themechange` can restore and re-run it.
function taliMermaidConfig() {
  // Dark is a page's `data-theme="dark"`
  var el = document.documentElement;
  var dark = el.getAttribute('data-theme') === 'dark';
  /** @type {Record<string, any>} */
  var cfg = {
    startOnLoad: false,
    theme: dark ? 'dark' : 'neutral',
    // Set EXPLICITLY, not left to the library's default. Diagram source is author text
    // that reaches mermaid's parser and comes back as SVG injected into the page, so the
    // sanitiser setting is ours to own: inheriting it means a mermaid upgrade could
    // silently loosen it. 'strict' sanitises HTML in labels and disables click handlers,
    // which no Taliesin diagram uses.
    securityLevel: 'strict',
  };
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
  // Edge routing is mermaid's own default (bezier, `curve: 'basis'`) — deliberately NOT
  // set here. It was 'step' (right angles) from 2026-07-31 until 2026-08-14; the layout
  // underneath is dagre either way, since that measured CLEANER than ELK on this corpus
  // (1 edge-crossing pair vs 4, and 11.5k vs 20.9k total edge length across all 28
  // diagrams in corpus/ + docs/), so the orthogonal look never bought a better layout.
  return cfg;
}
/** @param {NodeListOf<Element>} nodes */
function taliRunMermaid(nodes) {
  try {
    window.mermaid.initialize(taliMermaidConfig());
    // `run` is async, so a diagram with a syntax error rejects rather than throws, and the
    // `catch` below never saw it: every save logged an unhandled rejection. Mermaid has
    // already drawn its error graphic into the diagram's place by then, which is where
    // the author looks.
    Promise.resolve(window.mermaid.run({ nodes: nodes })).catch(function () {});
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
    'border:1px solid var(--tali-callout-important,#8B3A2E);border-radius:var(--tali-radius,2px);' +
    'padding:.5em .75em;margin:.5em 0;color:var(--tali-callout-important,#8B3A2E);' +
    'background:color-mix(in srgb,var(--tali-callout-important,#8B3A2E) 8%,transparent);font-size:.9em';
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
  if (window.__taliMermaidLoading) return; // its onload will sweep the whole doc
  window.__taliMermaidLoading = true;
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
    window.__taliMermaidLoading = false;
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
