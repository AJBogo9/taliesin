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
// container is replaced by the live diff, its scroll listener self-cleans on the next
// scroll and the fresh container re-initialises on the next enhancer run.
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

    // Active step = the LAST one whose top has scrolled above a trigger line at the viewport
    // centre. Unlike an IntersectionObserver activation band, this is robust to short steps
    // and to a stage shorter than the viewport (portrait / mobile): the trigger line crosses
    // each step top in document order, so every step is reachable and the active index is
    // monotonic: it never snaps back to the first step mid-scroll, and the last step stays
    // focused past the end.
    function currentStep() {
      var vh = window.innerHeight, doc = document.documentElement;
      // Past the very bottom of a scrollable page, force the last step: a walkthrough whose
      // final step sits near the page end has too little runout to bring its top up to the
      // trigger, so without this it would stall one step short of the end.
      if (doc.scrollHeight > vh + 4 && window.scrollY + vh >= doc.scrollHeight - 2) return steps.length - 1;
      var triggerY = vh * 0.5;
      var idx = 0;
      for (var j = 0; j < steps.length; j++) {
        if (steps[j].getBoundingClientRect().top <= triggerY) idx = j;
      }
      return idx;
    }
    var ticking = false;
    function onScroll() {
      if (ticking) return;
      ticking = true;
      requestAnimationFrame(function () {
        ticking = false;
        if (!cw.isConnected) { // container swapped out by a live diff: self-clean and stop
          window.removeEventListener('scroll', onScroll);
          window.removeEventListener('resize', onScroll);
          return;
        }
        apply(currentStep());
      });
    }
    window.addEventListener('scroll', onScroll, { passive: true });
    window.addEventListener('resize', onScroll, { passive: true });
    apply(currentStep()); // initial focus
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
