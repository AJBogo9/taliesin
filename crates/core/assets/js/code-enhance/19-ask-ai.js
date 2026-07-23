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

// Max encodeURIComponent length of the compact deep-link string (headroom below the
// ~2000-char cross-infrastructure URL ceiling; some browsers/providers truncate above it).
var TALI_ASK_BUDGET = 1900;

/**
 * Tier A (book-linked) only when the page's canonical URL is a real public http(s) host —
 * so the "read the book" link is reachable by a browsing AI. Everything else is Tier B
 * (paste-only, no link). `?taliAskTier=B` forces B for testing the degraded path.
 * @returns {'A' | 'B'}
 */
function taliAskTier() {
  try {
    if (new URLSearchParams(location.search).get('taliAskTier') === 'B') return 'B';
  } catch (e) {}
  var link = document.querySelector('link[rel="canonical"]');
  var href = link ? link.getAttribute('href') : null;
  if (!href) return 'B';
  var u;
  try {
    u = new URL(href, location.href);
  } catch (e) {
    return 'B';
  }
  if (u.protocol !== 'http:' && u.protocol !== 'https:') return 'B';
  var h = u.hostname;
  if (h === 'localhost' || h === '0.0.0.0' || /\.local$/.test(h)) return 'B';
  if (
    /^127\./.test(h) ||
    /^10\./.test(h) ||
    /^192\.168\./.test(h) ||
    /^172\.(1[6-9]|2\d|3[01])\./.test(h)
  )
    return 'B';
  return 'A';
}

/** encodeURIComponent length of a candidate string. @param {string} s @returns {number} */
function taliAskEnc(s) {
  return encodeURIComponent(s).length;
}

/**
 * Trim `text` to at most `n` chars at a word boundary, ellipsised if cut.
 * @param {string} text @param {number} n @returns {string}
 */
function taliAskClip(text, n) {
  if (n >= text.length) return text;
  if (n <= 0) return '';
  var cut = text.slice(0, n);
  var sp = cut.lastIndexOf(' ');
  return (sp > 0 ? cut.slice(0, sp) : cut).replace(/\s+$/, '') + '…';
}

/**
 * Largest n in [0, max] with taliAskEnc(make(n)) <= TALI_ASK_BUDGET (binary search).
 * @param {(n: number) => string} make @param {number} max @returns {number}
 */
function taliAskFit(make, max) {
  var lo = 0;
  var hi = max;
  var best = 0;
  while (lo <= hi) {
    var mid = (lo + hi) >> 1;
    if (taliAskEnc(make(mid)) <= TALI_ASK_BUDGET) {
      best = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return best;
}

/**
 * @typedef {Object} TaliAskPayload
 * @property {string} bookTitle
 * @property {string} sectionHeading
 * @property {string} passage
 * @property {string} sectionText
 * @property {string} question
 * @property {string} pageUrl
 * @property {string} llmsUrl
 */

/**
 * Build the two prompt strings from one payload. `full` (clipboard + composer, unlimited)
 * leads passage-first with a conditional link block in Tier A. `compact` (URL-encoded into a
 * deep-link) keeps a trimmed section floor, budgeted on ENCODED length: section trimmed
 * first, then passage; `deepLinkable=false` when passage+question alone overflow (caller
 * then opens the provider bare and relies on the clipboard).
 * @param {TaliAskPayload} p
 * @param {'A' | 'B'} tier
 * @returns {{ full: string, compact: string, deepLinkable: boolean }}
 */
function taliAskComposePrompt(p, tier) {
  var q = p.question && p.question.trim() ? p.question.trim() : 'Explain this passage in simpler terms.';

  var linkBlock =
    tier === 'A' && p.pageUrl
      ? '\n\nIf you can browse the web, you may also open this page for fuller context and answer using it; otherwise answer from the passage and section above:\n' +
        p.pageUrl +
        (p.llmsUrl ? '\n(Whole-book map, if you need it: ' + p.llmsUrl + ')' : '')
      : '';
  var full =
    'I\'m reading "' + p.bookTitle + '", section "' + p.sectionHeading + '".\n\n' +
    'Passage I highlighted:\n"""\n' + p.passage + '\n"""\n\n' +
    'Surrounding section (for context):\n"""\n' + p.sectionText + '\n"""\n\n' +
    'My question: ' + q + linkBlock;

  var linkTail = tier === 'A' && p.pageUrl ? ' If you can browse, more at ' + p.pageUrl + '.' : '';
  /** @param {number} passageLen @param {number} sectionLen @returns {string} */
  function build(passageLen, sectionLen) {
    return (
      'From "' + p.bookTitle + '" § "' + p.sectionHeading + '". Passage: "' +
      taliAskClip(p.passage, passageLen) + '". Context: "' +
      taliAskClip(p.sectionText, sectionLen) + '". ' + q + '.' + linkTail
    );
  }
  var deepLinkable = true;
  var compact = build(p.passage.length, p.sectionText.length);
  if (taliAskEnc(compact) > TALI_ASK_BUDGET) {
    var secLen = taliAskFit(function (n) {
      return build(p.passage.length, n);
    }, p.sectionText.length);
    compact = build(p.passage.length, secLen);
    if (taliAskEnc(compact) > TALI_ASK_BUDGET) {
      var passLen = taliAskFit(function (n) {
        return build(n, 0);
      }, p.passage.length);
      compact = build(passLen, 0);
      if (taliAskEnc(compact) > TALI_ASK_BUDGET) deepLinkable = false;
    }
  }
  return { full: full, compact: compact, deepLinkable: deepLinkable };
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
