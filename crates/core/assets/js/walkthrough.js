// Narrated code walkthrough: scroll-driven line-range highlighting.
//
// The server emits `::: {.code-walkthrough}` as a steps column (`.cw-steps`, one
// `.step[data-cw-lines]` per narration step) beside a sticky code panel
// (`.cw-stage > .cw-code > pre`). As the reader scrolls, the step nearest the
// viewport centre becomes active and its `data-cw-lines` spec focuses the matching
// lines in the panel — dimming the rest. Read-only / scroll-only: it never writes
// source. Reuses the deck's `.qhl-ln` / `.qhl-ln-hl` / `.qhl-lines-active` class
// contract (styled in base.css), but does NOT depend on deck.js (not loaded on
// pages), so the tiny line-spec parse lives here.
//
// Registered through the shared `qmdEnhancers` API, so it re-runs after every
// incremental block swap and is idempotent (guarded with `data-cw-init`). When a
// container is replaced by the live diff, its IntersectionObserver is GC'd with the
// old subtree and the fresh container re-initialises on the next run.
(function () {
  // Parse a line spec ("3-5", "1,4", "all", "") into a Set of 1-based line numbers.
  function parseLineSpec(spec) {
    var on = new Set();
    (spec || '').split(',').forEach(function (part) {
      var m = part.trim().match(/^(\d+)\s*-\s*(\d+)$/);
      if (m) { for (var n = +m[1]; n <= +m[2]; n++) on.add(n); }
      else if (/^\d+$/.test(part.trim())) on.add(+part.trim());
    });
    return on;
  }

  // Focus the lines named by `spec` in `pre`, dimming the rest. ""/"all" clears it.
  function focusLines(pre, spec) {
    if (!pre) return;
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

  function initWalkthrough(cw) {
    var pre = cw.querySelector('.cw-stage pre');
    var steps = Array.prototype.slice.call(cw.querySelectorAll('.cw-steps .step'));
    if (!pre || !steps.length) return;

    var active = -1;
    var apply = function (i) {
      if (i === active) return;
      active = i;
      steps.forEach(function (s, j) { s.classList.toggle('cw-step-active', j === i); });
      focusLines(pre, steps[i] ? steps[i].getAttribute('data-cw-lines') : '');
    };

    // Track which steps straddle the activation band; the LAST one wins (the step the
    // reader has scrolled down into). Before any step crosses, the first step is
    // active so the panel never starts blank.
    var visible = new Set();
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) visible.add(e.target); else visible.delete(e.target);
      });
      var last = -1;
      steps.forEach(function (s, j) { if (visible.has(s)) last = j; });
      apply(last === -1 ? 0 : last);
    }, { rootMargin: '-45% 0px -45% 0px', threshold: 0 });

    steps.forEach(function (s) { io.observe(s); });
    apply(0); // initial focus before the first scroll event
  }

  function enhance(root) {
    (root || document)
      .querySelectorAll('.code-walkthrough:not([data-cw-init])')
      .forEach(function (cw) {
        cw.setAttribute('data-cw-init', '1');
        initWalkthrough(cw);
      });
  }

  if (window.qmdEnhancers && window.qmdEnhancers.register) {
    window.qmdEnhancers.register(enhance);
  } else {
    document.addEventListener('DOMContentLoaded', function () { enhance(document); });
  }
})();
