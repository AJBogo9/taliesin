// Scrollytelling: scroll-driven sticky-stage scenes.
//
// The server emits `::: {.scrolly}` as a `.scrolly-steps` column (one `.step[data-state]`
// per scene) beside a sticky `.scrolly-stage`. As the reader scrolls, the step nearest the
// viewport centre becomes active: its `data-state` is mirrored to `data-scrolly-state` on
// the root (for pure-CSS effects) and, when the `.scrolly` was given a `name=`, pushed into
// a hidden `.qmd-scrolly-input[data-qmd-input]` (value + an `input` event) so the shipped
// reactive graph re-runs the sticky `{js}` cell via `//| input:`. Read-only / scroll-only.
//
// Reuses the deck/walkthrough IntersectionObserver activation band, but does NOT depend on
// walkthrough.js. Registered through `qmdEnhancers`; idempotent (`data-scrolly-init`).
(function () {
  function initScrolly(root) {
    var steps = Array.prototype.slice.call(root.querySelectorAll('.scrolly-steps .step'));
    if (!steps.length) return;
    var input = root.querySelector('.qmd-scrolly-input');
    var active = -1;
    function apply(i, dispatch) {
      if (i === active) return;
      active = i;
      steps.forEach(function (s, j) { s.classList.toggle('scrolly-step-active', j === i); });
      var state = steps[i] ? (steps[i].getAttribute('data-state') || '') : '';
      root.setAttribute('data-scrolly-state', state);
      if (input && dispatch && input.value !== state) {
        input.value = state;
        input.dispatchEvent(new Event('input', { bubbles: true }));
      }
    }
    // Track which steps straddle the activation band; the LAST one wins. Before any step
    // crosses, the first is active so the stage never starts blank.
    var visible = new Set();
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) visible.add(e.target); else visible.delete(e.target);
      });
      var last = -1;
      steps.forEach(function (s, j) { if (visible.has(s)) last = j; });
      apply(last === -1 ? 0 : last, true);
    }, { rootMargin: '-45% 0px -45% 0px', threshold: 0 });
    steps.forEach(function (s) { io.observe(s); });
    // Initial: set the state attribute but do NOT dispatch — the hidden input's
    // server-rendered value already matches step 0, and the cell ran once on mount.
    apply(0, false);
  }

  function enhance(root) {
    (root || document)
      .querySelectorAll('.qmd-scrolly:not([data-scrolly-init])')
      .forEach(function (el) {
        el.setAttribute('data-scrolly-init', '1');
        initScrolly(el);
      });
  }

  if (window.qmdEnhancers && window.qmdEnhancers.register) {
    window.qmdEnhancers.register(enhance);
  } else {
    document.addEventListener('DOMContentLoaded', function () { enhance(document); });
  }
})();
