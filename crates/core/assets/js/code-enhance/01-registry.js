
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

// Reader preference: are the single-key shortcuts (`f`, `?`, `/`) live? WCAG 2.1.4 (Character
// Key Shortcuts) requires a way to turn character-key shortcuts off; this is that mechanism, and
// it is why `f` can keep entering fullscreen directly. Default ON, so a reader who never opens
// Settings sees no change. Esc + the arrow keys are not character keys and are never gated.
// A blocked or throwing localStorage must not silently cost a reader their shortcuts, so every
// failure path returns true.
//
// Key: `tali-shortcuts`. This deliberately does NOT match its only two siblings, `qmd-theme`
// (render/theme.rs) and `qmd-deck-theme`, which still carry the retired `qmd-` prefix. Those are
// frozen: a storage key has no aliasing mechanism, so renaming one would silently reset every
// existing reader's saved choice. A brand-new key carries no such burden and uses the owned
// prefix. The mismatch is intentional; do not "fix" it.
function taliShortcutsOn() {
  try { return localStorage.getItem('tali-shortcuts') !== 'off'; } catch (e) { return true; }
}
// Absent === on (the default), mirroring how theme.rs stores its non-default choices only.
function taliSetShortcuts(on) {
  try {
    if (on) localStorage.removeItem('tali-shortcuts');
    else localStorage.setItem('tali-shortcuts', 'off');
  } catch (e) {}
}

