// Tabset enhancer: a `::: {.panel-tabset}` div whose direct children are headings
// (each starting a tab) + that tab's content becomes a clickable tab strip.
// Registers through the public window.qmdEnhancers API — the same hook core's
// built-ins use. Idempotent (guarded by data-tabset), so it survives re-mounts.
(function () {
  if (!window.qmdEnhancers) return; // registry (code-enhance.js) loads first
  window.qmdEnhancers.register(function (root) {
    (root || document).querySelectorAll('.panel-tabset:not([data-tabset])').forEach(function (set) {
      set.dataset.tabset = '1';
      // group children into (heading -> following nodes) tabs
      var tabs = [], cur = null;
      Array.prototype.forEach.call(set.children, function (node) {
        if (/^H[1-6]$/.test(node.tagName)) { cur = { title: node.textContent, nodes: [] }; tabs.push(cur); }
        else if (cur) { cur.nodes.push(node); }
      });
      if (!tabs.length) return;
      var bar = document.createElement('div'); bar.className = 'qmd-tab-bar'; bar.setAttribute('role', 'tablist');
      var panels = document.createElement('div');
      set.innerHTML = '';
      tabs.forEach(function (t, i) {
        var sel = i === 0;
        var btn = document.createElement('button');
        btn.className = 'qmd-tab-btn'; btn.type = 'button'; btn.textContent = t.title;
        btn.setAttribute('role', 'tab'); btn.setAttribute('aria-selected', sel ? 'true' : 'false');
        var panel = document.createElement('div'); panel.className = 'qmd-tab-panel';
        panel.setAttribute('role', 'tabpanel'); panel.hidden = !sel;
        t.nodes.forEach(function (n) { panel.appendChild(n); });
        btn.addEventListener('click', function () {
          bar.querySelectorAll('.qmd-tab-btn').forEach(function (b) { b.setAttribute('aria-selected', 'false'); });
          panels.querySelectorAll('.qmd-tab-panel').forEach(function (p) { p.hidden = true; });
          btn.setAttribute('aria-selected', 'true'); panel.hidden = false;
        });
        bar.appendChild(btn); panels.appendChild(panel);
      });
      set.appendChild(bar); set.appendChild(panels);
    });
  });
})();
