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

var TALI_ASK_KEY = 'tali-askai';

/**
 * Provider registry — the one place a broken deep-link is a one-line fix. `deepLink(q)` takes
 * the already-encoded compact string; `paste:true` => open the bare new-chat URL and rely on
 * the clipboard (Claude's web ?q= prefill was removed ~Oct-2025). Verified 2026 mechanics
 * (spec §4/§12). Gemini app / Copilot / Duck.ai are copy-only, so not listed as first-class.
 * @type {Record<string, { label: string, deepLink: ((q: string) => string) | null, paste: boolean }>}
 */
var TALI_ASK_PROVIDERS = {
  chatgpt: {
    label: 'ChatGPT',
    deepLink: function (q) {
      return 'https://chatgpt.com/?q=' + q + '&hints=search';
    },
    paste: false,
  },
  perplexity: {
    label: 'Perplexity',
    deepLink: function (q) {
      return 'https://www.perplexity.ai/search/?q=' + q;
    },
    paste: false,
  },
  google: {
    label: 'Google AI Mode',
    deepLink: function (q) {
      return 'https://www.google.com/search?udm=50&q=' + q;
    },
    paste: false,
  },
  claude: {
    label: 'Claude',
    deepLink: function () {
      return 'https://claude.ai/new';
    },
    paste: true,
  },
  copy: { label: 'Copy prompt', deepLink: null, paste: true },
};

/**
 * @typedef {Object} TaliAskStore
 * @property {number} v
 * @property {string} [provider]
 * @property {boolean} [ack]
 * @property {number} [picked_at]
 */

/** Defensive read: parse failure, wrong version, or unknown provider => null (first-run).
 * @returns {TaliAskStore | null} */
function taliAskRead() {
  try {
    var raw = localStorage.getItem(TALI_ASK_KEY);
    if (!raw) return null;
    var o = JSON.parse(raw);
    if (!o || o.v !== 1) return null;
    if (o.provider && !TALI_ASK_PROVIDERS[o.provider]) return null;
    return o;
  } catch (e) {
    return null;
  }
}

/** @param {TaliAskStore} o */
function taliAskWrite(o) {
  try {
    localStorage.setItem(TALI_ASK_KEY, JSON.stringify(o));
  } catch (e) {}
}

/** @returns {string | null} the remembered provider id, or null on first run */
function taliAskProvider() {
  var o = taliAskRead();
  return o && o.provider ? o.provider : null;
}

/** Set + promote a provider to the default. @param {string} id */
function taliAskSetProvider(id) {
  if (!TALI_ASK_PROVIDERS[id]) return;
  var o = taliAskRead() || { v: 1 };
  o.v = 1;
  o.provider = id;
  o.picked_at = Date.now();
  taliAskWrite(o);
}

/** @returns {boolean} whether the first-run consent was acknowledged */
function taliAskGetAck() {
  var o = taliAskRead();
  return !!(o && o.ack);
}

function taliAskSetAck() {
  var o = taliAskRead() || { v: 1 };
  o.v = 1;
  o.ack = true;
  taliAskWrite(o);
}

/** Reversible: drop the stored choice + ack => back to the first-run picker. */
function taliAskForget() {
  try {
    localStorage.removeItem(TALI_ASK_KEY);
  } catch (e) {}
}

// --- Composer dialog (the single home for the question input + every provider path) -------

/** @type {HTMLElement | null} */
var taliAskDialogEl = null;
/** @type {HTMLElement | null} */
var taliAskBackdropEl = null;
/** @type {(() => void) | null} */
var taliAskRelease = null;
/** @type {(TaliAskPayload & { trigger?: HTMLElement }) | null} */
var taliAskPayload = null;

var TALI_ASK_DISCLOSURE =
  'This opens your chosen AI in a new tab and sends the passage you selected, your question, ' +
  'and (if this book is online) a link to it. It goes to your OWN AI account under their privacy ' +
  'policy, where it may be used to train their AI. This book has no server and stores nothing but ' +
  'your provider choice.';

/** querySelector that asserts non-null (we build the markup ourselves).
 * @param {ParentNode} root @param {string} sel @returns {HTMLElement} */
function taliAskQ(root, sel) {
  return /** @type {HTMLElement} */ (root.querySelector(sel));
}

/** Build the singleton dialog + backdrop once; wire the handlers that persist across opens. */
function taliAskBuildDialog() {
  if (taliAskDialogEl) return taliAskDialogEl;
  var backdrop = document.createElement('div');
  backdrop.className = 'tali-askai-backdrop';
  backdrop.hidden = true;
  backdrop.addEventListener('click', taliAskCloseComposer);

  var dlg = document.createElement('div');
  dlg.className = 'tali-askai-dialog';
  dlg.setAttribute('role', 'dialog');
  dlg.setAttribute('aria-labelledby', 'tali-askai-title');
  dlg.hidden = true;
  dlg.innerHTML =
    '<button type="button" class="tali-askai-close" aria-label="Close">×</button>' +
    '<h2 id="tali-askai-title" class="tali-askai-title">Ask AI</h2>' +
    '<div class="tali-askai-consent" hidden>' +
    '<p class="tali-askai-consent-text"></p>' +
    '<div class="tali-askai-btnrow">' +
    '<button type="button" class="tali-askai-btn tali-askai-consent-continue">Continue</button>' +
    '<button type="button" class="tali-askai-btn-ghost tali-askai-consent-cancel">Cancel</button>' +
    '</div></div>' +
    '<div class="tali-askai-main" hidden>' +
    '<div class="tali-askai-pick"><p class="tali-askai-picklabel">Ask which AI? (remembered next time)</p>' +
    '<div class="tali-askai-providers"></div></div>' +
    '<div class="tali-askai-ready" hidden>' +
    '<div class="tali-askai-actionrow">' +
    '<button type="button" class="tali-askai-btn tali-askai-go"></button>' +
    '<button type="button" class="tali-askai-caret" aria-haspopup="menu" aria-expanded="false" aria-label="Change AI provider">▾</button>' +
    '</div>' +
    '<div class="tali-askai-menu" role="menu" hidden></div>' +
    '<p class="tali-askai-using"></p></div>' +
    '<label class="tali-askai-qlabel">Your question' +
    '<textarea class="tali-askai-q" rows="2" placeholder="Ask about this… (e.g. explain this passage in simpler terms)"></textarea>' +
    '</label>' +
    '<div class="tali-askai-preview" aria-label="Selected passage"></div>' +
    '<details class="tali-askai-full"><summary>Full prompt</summary>' +
    '<textarea class="tali-askai-fulltext" readonly rows="6"></textarea>' +
    '<button type="button" class="tali-askai-btn-ghost tali-askai-copy">Copy prompt</button>' +
    '</details>' +
    '<p class="tali-askai-note"></p></div>';

  document.body.appendChild(backdrop);
  document.body.appendChild(dlg);
  taliAskDialogEl = dlg;
  taliAskBackdropEl = backdrop;

  taliAskQ(dlg, '.tali-askai-close').addEventListener('click', taliAskCloseComposer);
  taliAskQ(dlg, '.tali-askai-consent-cancel').addEventListener('click', taliAskCloseComposer);
  taliAskQ(dlg, '.tali-askai-consent-continue').addEventListener('click', function () {
    taliAskSetAck();
    taliAskRenderState();
  });
  taliAskQ(dlg, '.tali-askai-q').addEventListener('input', taliAskRecompute);
  taliAskQ(dlg, '.tali-askai-copy').addEventListener('click', function () {
    if (!taliAskPayload) return;
    var composed = taliAskComposePrompt(taliAskPayload, taliAskTier());
    taliCopyText(composed.full, function () {}, function () {});
  });
  taliAskQ(dlg, '.tali-askai-caret').addEventListener('click', function () {
    taliAskToggleMenu();
  });
  dlg.addEventListener('keydown', taliAskOnKey);
  return dlg;
}

/** @param {KeyboardEvent} e */
function taliAskOnKey(e) {
  if (e.key === 'Escape') taliAskCloseComposer();
}

/** Render consent -> picker -> ready based on stored ack + provider. */
function taliAskRenderState() {
  var dlg = taliAskDialogEl;
  if (!dlg) return;
  var consent = taliAskQ(dlg, '.tali-askai-consent');
  var main = taliAskQ(dlg, '.tali-askai-main');
  if (!taliAskGetAck()) {
    taliAskQ(dlg, '.tali-askai-consent-text').textContent = TALI_ASK_DISCLOSURE;
    consent.hidden = false;
    main.hidden = true;
    return;
  }
  consent.hidden = true;
  main.hidden = false;
  var pick = taliAskQ(dlg, '.tali-askai-pick');
  var ready = taliAskQ(dlg, '.tali-askai-ready');
  var current = taliAskProvider();
  if (!current) {
    taliAskRenderProviders();
    pick.hidden = false;
    ready.hidden = true;
  } else {
    taliAskRenderReady(current);
    pick.hidden = true;
    ready.hidden = false;
  }
  taliAskRecompute();
}

/** Render the first-run provider tiles. */
function taliAskRenderProviders() {
  var dlg = taliAskDialogEl;
  if (!dlg) return;
  var box = taliAskQ(dlg, '.tali-askai-providers');
  box.innerHTML = '';
  Object.keys(TALI_ASK_PROVIDERS).forEach(function (id) {
    var btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'tali-askai-provider';
    btn.textContent = TALI_ASK_PROVIDERS[id].label;
    btn.addEventListener('click', function () {
      taliAskSetProvider(id);
      taliAskRenderState();
    });
    box.appendChild(btn);
  });
}

/** Render the remembered-provider action row + the change/forget menu. @param {string} id */
function taliAskRenderReady(id) {
  var dlg = taliAskDialogEl;
  if (!dlg) return;
  var prov = TALI_ASK_PROVIDERS[id];
  var go = taliAskQ(dlg, '.tali-askai-go');
  go.textContent = (prov.paste ? 'Open ' : 'Ask ') + prov.label;
  go.onclick = function () {
    taliAskGo(id);
  };
  taliAskQ(dlg, '.tali-askai-using').textContent = 'Using ' + prov.label;
  var menu = taliAskQ(dlg, '.tali-askai-menu');
  menu.innerHTML = '';
  Object.keys(TALI_ASK_PROVIDERS).forEach(function (other) {
    if (other === id) return;
    menu.appendChild(
      taliAskMenuItem('Switch to ' + TALI_ASK_PROVIDERS[other].label, function () {
        taliAskSetProvider(other);
        taliAskToggleMenu(false);
        taliAskRenderState();
      })
    );
  });
  menu.appendChild(
    taliAskMenuItem('Forget my choice', function () {
      taliAskForget();
      taliAskToggleMenu(false);
      taliAskRenderState();
    })
  );
}

/** @param {string} label @param {() => void} onClick @returns {HTMLElement} */
function taliAskMenuItem(label, onClick) {
  var mi = document.createElement('button');
  mi.type = 'button';
  mi.className = 'tali-askai-menuitem';
  mi.setAttribute('role', 'menuitem');
  mi.textContent = label;
  mi.addEventListener('click', onClick);
  return mi;
}

/** @param {boolean} [force] */
function taliAskToggleMenu(force) {
  var dlg = taliAskDialogEl;
  if (!dlg) return;
  var menu = taliAskQ(dlg, '.tali-askai-menu');
  var caret = taliAskQ(dlg, '.tali-askai-caret');
  var open = typeof force === 'boolean' ? force : menu.hidden;
  menu.hidden = !open;
  caret.setAttribute('aria-expanded', open ? 'true' : 'false');
}

/** Recompute the prompt from the payload + current question, refresh preview/full/note. */
function taliAskRecompute() {
  var dlg = taliAskDialogEl;
  if (!dlg || !taliAskPayload) return;
  var qbox = /** @type {HTMLTextAreaElement} */ (dlg.querySelector('.tali-askai-q'));
  taliAskPayload.question = qbox.value;
  var tier = taliAskTier();
  var composed = taliAskComposePrompt(taliAskPayload, tier);
  taliAskQ(dlg, '.tali-askai-preview').textContent = taliAskPayload.passage;
  /** @type {HTMLTextAreaElement} */ (dlg.querySelector('.tali-askai-fulltext')).value = composed.full;
  taliAskQ(dlg, '.tali-askai-note').textContent =
    tier === 'B'
      ? 'Sends the selected passage to your AI. (This book isn’t public, so no link is included.)'
      : 'Opens your AI in a new tab. The full prompt is also copied to your clipboard — paste with Cmd/Ctrl+V if the box is empty.';
}

/** The "Ask {provider}" action. @param {string} id */
function taliAskGo(id) {
  if (!taliAskPayload) return;
  var composed = taliAskComposePrompt(taliAskPayload, taliAskTier());
  taliAskHandOff(id, composed);
  taliAskCloseComposer();
}

/**
 * Hand off to the provider. NOTE: the popup-safe synchronous open sequence lands in Task 8;
 * this stub only copies the prompt so the composer is testable in isolation.
 * @param {string} id @param {{ full: string, compact: string, deepLinkable: boolean }} composed
 */
function taliAskHandOff(id, composed) {
  void id;
  taliCopyText(composed.full, function () {}, function () {});
}

/** @param {TaliAskPayload & { trigger?: HTMLElement }} payload */
function taliAskOpenComposer(payload) {
  taliAskBuildDialog();
  var dlg = taliAskDialogEl;
  if (!dlg) return;
  taliAskPayload = payload;
  var qbox = /** @type {HTMLTextAreaElement} */ (dlg.querySelector('.tali-askai-q'));
  qbox.value = payload.question || '';
  taliAskRenderState();
  if (taliAskBackdropEl) taliAskBackdropEl.hidden = false;
  dlg.hidden = false;
  var initial = taliAskGetAck() && taliAskProvider() ? qbox : null;
  if (window.taliFocusTrap) taliAskRelease = window.taliFocusTrap(dlg, initial);
}

function taliAskCloseComposer() {
  var dlg = taliAskDialogEl;
  if (!dlg) return;
  taliAskToggleMenu(false);
  if (taliAskRelease) {
    taliAskRelease();
    taliAskRelease = null;
  }
  dlg.hidden = true;
  if (taliAskBackdropEl) taliAskBackdropEl.hidden = true;
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
