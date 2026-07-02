// Tabbed panels: the interaction layer for `::: {.panel-tabset}`.
//
// The server emits a `.panel-tabset` with a `[role=tablist]` of `[role=tab]` buttons
// (`aria-controls` -> a panel id) and matching `[role=tabpanel]` panels (the inactive
// ones `hidden`). This wires the standard ARIA tabs keyboard + click behaviour:
// clicking or arrow-keying a tab selects it (toggling `aria-selected`, the panel's
// `hidden`, and a roving `tabindex`). Left/Right move between tabs, Home/End jump to
// the ends. Read-only: it toggles only `aria-*`/`hidden`, never source.
//
// Registered through the shared `taliEnhancers` API, so it re-runs after every
// incremental block swap and is idempotent (guarded with `data-tabset-init`). A
// replaced subtree's listeners are GC'd with it; the fresh tabset re-initialises.
(function () {
  function initTabset(set) {
    var tablist = set.querySelector('[role="tablist"]');
    if (!tablist) return;
    var tabs = Array.prototype.slice.call(tablist.querySelectorAll('[role="tab"]'));
    if (!tabs.length) return;

    var select = function (i, focus) {
      tabs.forEach(function (tab, j) {
        var on = j === i;
        tab.setAttribute('aria-selected', on ? 'true' : 'false');
        tab.tabIndex = on ? 0 : -1;
        var panel = document.getElementById(tab.getAttribute('aria-controls'));
        if (panel) panel.hidden = !on;
      });
      if (focus) tabs[i].focus();
    };

    tablist.addEventListener('click', function (e) {
      var tab = e.target.closest('[role="tab"]');
      if (tab) select(tabs.indexOf(tab), false);
    });

    tablist.addEventListener('keydown', function (e) {
      var cur = tabs.indexOf(document.activeElement);
      if (cur < 0) return;
      var next = null;
      if (e.key === 'ArrowRight' || e.key === 'ArrowDown') next = (cur + 1) % tabs.length;
      else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') next = (cur - 1 + tabs.length) % tabs.length;
      else if (e.key === 'Home') next = 0;
      else if (e.key === 'End') next = tabs.length - 1;
      if (next !== null) { e.preventDefault(); select(next, true); }
    });
  }

  function enhance(root) {
    (root || document)
      .querySelectorAll('.panel-tabset:not([data-tabset-init])')
      .forEach(function (set) {
        set.setAttribute('data-tabset-init', '1');
        initTabset(set);
      });
  }

  if (window.taliEnhancers && window.taliEnhancers.register) {
    window.taliEnhancers.register(enhance);
  } else {
    document.addEventListener('DOMContentLoaded', function () { enhance(document); });
  }
})();
