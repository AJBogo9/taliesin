
// --- Enhancer registry (the public extension hook) ---------------------------
// An *enhancer* is `fn(root)` that decorates freshly-mounted DOM. An extension's
// JS opts in with `window.taliEnhancers.register(fn)`; the registered fn then runs
// after every (re)mount in the live preview, on DOMContentLoaded in the static
// build, and once immediately if it registers after the page is already mounted
// (an extension script loaded in `include-after-body`). Enhancers MUST be
// idempotent — guard with a data-attribute — since they re-run on every change.
// The built-in copy-button / lightbox / etc. below (and mermaid, in its own
// mermaid.js) register through the exact same API, so a third-party enhancer is
// indistinguishable from core's.
(function () {
  if (window.taliEnhancers) return;
  var list = [];
  var mounted = false;
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
  // Back-compat: the pre-rename public globals (same live objects).
  window.qmdEnhancers = window.taliEnhancers;
  // The single entry point every caller uses (live client, static build, reveal).
  window.taliEnhanceCode = function (root) { window.taliEnhancers.run(root); };
  window.qmdEnhanceCode = window.taliEnhanceCode;
})();

// Shared clipboard helper: navigator.clipboard in a secure context, with a hidden-textarea
// execCommand fallback for insecure contexts (file://, plain-http --host LAN). Never throws;
// calls onOk on success, onFail (optional) on total failure.
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

// Build the canonical absolute deep link to the in-page anchor `id`: this page's URL with
// any existing #id / :~:text= dropped, then this id. Pure.
function taliAnchorUrl(id) {
  var u = new URL(location.href);
  u.hash = '';
  return u.href + '#' + encodeURIComponent(id);
}

// Read a caption's visible text without the interactive chrome that taliInitAnchorLinks splices
// in: the `#` permalink (a `.tali-anchor`, transiently `✓` mid-copy) lives inside the figcaption,
// so a verbatim `.textContent` reads "Figure 1: No pooling.#". Clone-strip-read (the same trick
// the link-preview card's cleanClone uses) keeps the read-only original intact. Returns '' for
// a missing node.
// Clone a node, stripping interactive chrome that has no place in a read-only clone:
// the heading/caption `#` permalink (taliInitAnchorLinks) and code copy buttons. Shared by
// the lightbox caption reader and the link-preview card builder. Returns the clone.
function taliCloneStripped(node) {
  var c = node.cloneNode(true);
  if (c.querySelectorAll) {
    [].forEach.call(c.querySelectorAll('.tali-anchor, .tali-copy'), function (x) { x.remove(); });
  }
  return c;
}
function taliCleanCaptionText(node) {
  if (!node) return '';
  if (!node.cloneNode) return (node.textContent || '').trim();
  return (taliCloneStripped(node).textContent || '').trim();
}

