// Scrollytelling: scroll-driven sticky-stage scenes.
//
// The server emits `::: {.scrolly}` as a `.scrolly-steps` column (one `.step[data-state]`
// per scene) beside a sticky `.scrolly-stage`. As the reader scrolls, the step nearest the
// viewport centre becomes active: its `data-state` is mirrored to `data-scrolly-state` on
// the root (for pure-CSS effects) and, when the `.scrolly` was given a `name=`, pushed into
// a hidden `.qmd-scrolly-input[data-qmd-input]` (value + an `input` event) so the shipped
// reactive graph re-runs the sticky `{js}` cell via `//| input:`. Read-only / scroll-only.
//
// Activation is a scroll-driven trigger line (not an IntersectionObserver band, which cannot
// isolate steps shorter than the viewport, so it broke on portrait / mobile). Does NOT depend
// on walkthrough.js. Registered through `qmdEnhancers`; idempotent (`data-scrolly-init`);
// self-cleans its scroll listener when its container is swapped out by a live diff.
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
    // Active step = the LAST one whose top has scrolled above a trigger line at the viewport
    // centre. Unlike an IntersectionObserver activation band, this is robust to short steps
    // and to a stage shorter than the viewport (portrait / mobile, where a band can never
    // isolate a step): the trigger line crosses each step top in document order, so every
    // step is reachable and the active index is monotonic: it never snaps back to the first
    // step mid-scroll, and the last step stays active past the end.
    function currentStep() {
      var vh = window.innerHeight, doc = document.documentElement;
      // Past the very bottom of a scrollable page, force the last step: a story whose final
      // narration sits near the page end has too little runout to bring its top up to the
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
        if (!root.isConnected) { // container swapped out by a live diff: self-clean and stop
          window.removeEventListener('scroll', onScroll);
          window.removeEventListener('resize', onScroll);
          return;
        }
        apply(currentStep(), true);
      });
    }
    window.addEventListener('scroll', onScroll, { passive: true });
    window.addEventListener('resize', onScroll, { passive: true });
    // Initial: sync to the step under the trigger (handles a page that loads already
    // scrolled). The `input.value !== state` guard in apply() skips a redundant dispatch at
    // the top, where the server-rendered value already matches step 0.
    apply(currentStep(), true);
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
