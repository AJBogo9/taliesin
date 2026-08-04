
// --- Enhancer registry (the public extension hook) ---------------------------
// An *enhancer* is `fn(root)` that decorates freshly-mounted DOM. An extension's
// JS opts in with `window.taliEnhancers.register(fn)`; the registered fn then runs
// after every (re)mount in the live preview, on DOMContentLoaded in the static
// build, and once immediately if it registers after the page is already mounted
// (an extension script loaded in `include-after-body`). Enhancers MUST be
// idempotent — guard with a data-attribute — since they re-run on every change.
// The built-in copy-button / etc. below (and mermaid, in its own
// mermaid.js) register through the exact same API, so a third-party enhancer is
// indistinguishable from core's.
(function () {
  if (window.taliEnhancers) return;
  /** @type {Array<(root: ParentNode) => void>} */
  var list = [];
  var mounted = false;
  /** @param {(root: ParentNode) => void} fn @param {ParentNode | null} [root] */
  function run1(fn, root) {
    try { fn(root || document); } catch (e) { console.error('[taliesin] enhancer failed', e); }
  }
  window.taliEnhancers = {
    register: function (fn) {
      if (typeof fn === 'function') {
        list.push(fn);
        if (mounted) run1(fn, document); // late registration: catch up on existing DOM
      }
      return this;
    },
    run: function (root) {
      mounted = true;
      for (var i = 0; i < list.length; i++) run1(list[i], root);
    },
  };
  var enh = window.taliEnhancers; // captured non-undefined for the entry-point closure
  // The single entry point every caller uses (live client, static build, reveal).
  window.taliEnhanceCode = function (root) { enh.run(root); };
})();

// Shared clipboard helper: navigator.clipboard in a secure context, with a hidden-textarea
// execCommand fallback for insecure contexts (file://, plain-http --host LAN). Never throws;
// calls onOk on success, onFail (optional) on total failure.
/** @param {string} text @param {() => void} onOk @param {() => void} [onFail] */
function taliCopyText(text, onOk, onFail) {
  function legacy() {
    try {
      var ta = document.createElement('textarea');
      ta.value = text; ta.setAttribute('readonly', '');
      ta.style.position = 'fixed'; ta.style.top = '0'; ta.style.opacity = '0';
      document.body.appendChild(ta); ta.select();
      var done = document.execCommand('copy'); document.body.removeChild(ta);
      return done;
    } catch (e) { return false; }
  }
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(onOk, function () { if (legacy()) onOk(); else if (onFail) onFail(); });
  } else if (legacy()) { onOk(); }
  else if (onFail) { onFail(); }
}

