/*!tali-askai v1*/
// Ask AI — client-side hand-off to the student's own logged-in AI.
// Spec: notes/2026-07-23-ask-ai-handoff-design.md. Read-only; no backend.
// One fragment in the concatenated code-enhance <script> (shares global scope), so
// every symbol is `taliAsk`-prefixed. Top-level functions are both tsc-visible globals
// and runtime window properties (used by the browser test harness).

/**
 * Visible text of a node's contents, math-aware. KaTeX renders its source twice
 * (a MathML <annotation> + visual glyph spans), so raw textContent/getSelection doubles
 * it. We emit the LaTeX from the annotation ($…$, or $$…$$ inside .katex-display) and
 * never descend into the glyphs. Code (<pre>/<code>) is kept verbatim. Everything else is
 * visible text with a space at each element boundary so adjacent blocks stay word-separated
 * (mirrors the server-side `text_content` rule in site/llms.rs).
 * @param {Node} node
 * @returns {string}
 */
function taliAskExtractText(node) {
  /** @type {string[]} */
  var out = [];
  taliAskWalk(node, out);
  return out.join('').replace(/[ \t]+/g, ' ').replace(/ *\n */g, '\n').trim();
}

/**
 * @param {Node} node
 * @param {string[]} out
 */
function taliAskWalk(node, out) {
  if (node.nodeType === 3 /* text */) {
    out.push(node.nodeValue || '');
    return;
  }
  if (node.nodeType !== 1 /* element */) return;
  var el = /** @type {Element} */ (node);
  if (el.classList.contains('katex')) {
    var ann = el.querySelector('annotation[encoding="application/x-tex"]');
    var tex = ann ? (ann.textContent || '').trim() : '';
    if (tex) {
      var display = !!el.closest('.katex-display');
      out.push(display ? '\n$$' + tex + '$$\n' : '$' + tex + '$');
    }
    return; // never descend into the doubled render
  }
  var tag = el.tagName;
  if (tag === 'PRE' || tag === 'CODE') {
    out.push(el.textContent || '');
    return;
  }
  if (tag === 'SCRIPT' || tag === 'STYLE') return;
  out.push(' ');
  for (var c = el.firstChild; c; c = c.nextSibling) taliAskWalk(c, out);
  out.push(' ');
}

/**
 * Entry point; registered in 09-register.js. Idempotent; skips decks.
 * @param {Document | Element} [root]
 */
function taliInitAskAi(root) {
  if (typeof document === 'undefined') return;
  if (document.querySelector('.tali-deck')) return; // decks are not reading views
  var host = document.body;
  if (!host || host.getAttribute('data-tali-askai') === 'on') return;
  host.setAttribute('data-tali-askai', 'on');
  void root; // reserved for future scoped re-init
  // Wiring added in later tasks.
}
