
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
    try { fn(root || document); } catch (e) { console.error('[qmd] enhancer failed', e); }
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

// Build a W3C Text Fragment URL (#:~:text=) that deep-links to `rawText` on this page. Pure;
// returns null for empty input. A long selection uses the textStart,textEnd range form to keep
// the URL short. encTF escapes the three chars structurally significant in a text directive
// ('-' marks prefix/suffix, ',' separates parts, '&' separates directives) on top of
// encodeURIComponent, so the directive can never break out of itself.
function taliBuildTextFragmentUrl(rawText) {
  var text = (rawText || '').replace(/\s+/g, ' ').trim();
  if (!text) return null;
  function encTF(s) { return encodeURIComponent(s).replace(/-/g, '%2D').replace(/,/g, '%2C').replace(/&/g, '%26'); }
  var start = text, end = null;
  if (text.length > 300) {
    var words = text.split(' ');
    if (words.length >= 12) { start = words.slice(0, 6).join(' '); end = words.slice(-6).join(' '); }
    else { var cut = text.slice(0, 300), sp = cut.lastIndexOf(' '); start = sp > 0 ? cut.slice(0, sp) : cut; }
  }
  var directive = 'text=' + encTF(start) + (end ? ',' + encTF(end) : '');
  // Preserve any element-id hash, drop any prior text fragment, emit exactly one ':~:'.
  // Concatenate the href by string (assigning u.hash would re-encode '%' to '%25').
  var u = new URL(location.href);
  var id = u.hash.replace(/^#/, '').split(':~:')[0];
  u.hash = '';
  return u.href + '#' + id + ':~:' + directive;
}

// Build a BibTeX @misc entry citing `title` at `url`, accessed on `date`. Pure. The URL
// rides verbatim inside \url{} (so the deep link's '# : ~ % &' survive LaTeX); the title is
// LaTeX-escaped and double-braced to preserve its casing; the cite key is a slug of the
// title plus the access year. BibTeX is the most portable cite format — reference managers
// import it and re-export to any style — so the toolbar's four actions stay distinct (Copy
// raw / Quote markdown / Share url / Cite bibtex).
function taliBuildBibtex(title, url, date) {
  var MONTHS = ['January', 'February', 'March', 'April', 'May', 'June',
    'July', 'August', 'September', 'October', 'November', 'December'];
  var ESC = {
    '\\': '\\textbackslash{}', '{': '\\{', '}': '\\}', '&': '\\&', '%': '\\%',
    '$': '\\$', '#': '\\#', '_': '\\_', '~': '\\textasciitilde{}', '^': '\\textasciicircum{}'
  };
  // Single pass over the originals, so the braces introduced by a replacement are never
  // themselves re-escaped (a two-pass escape would corrupt \textbackslash{}).
  function latexEsc(s) { return String(s).replace(/[\\{}&%$#_~^]/g, function (c) { return ESC[c]; }); }
  var name = (title || 'Untitled').trim() || 'Untitled';
  var year = date.getFullYear();
  var slug = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
  var key = (slug || 'tali-citation') + '-' + year;
  var accessed = MONTHS[date.getMonth()] + ' ' + date.getDate() + ', ' + year;
  return '@misc{' + key + ',\n' +
    '  title        = {{' + latexEsc(name) + '}},\n' +
    '  howpublished = {\\url{' + url + '}},\n' +
    '  note         = {Accessed ' + accessed + '}\n' +
    '}\n';
}

// Build the canonical absolute deep link to the in-page anchor `id`: this page's URL with
// any existing #id / :~:text= dropped, then this id. Pure; mirrors taliBuildTextFragmentUrl.
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

