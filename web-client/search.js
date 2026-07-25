// @ts-check
// Client-side command palette (Cmd/Ctrl-K): full-text search to jump around a
// long document — the book, a paper, any page with a table of contents. Matches
// both headings and the body text of each section, and shows a snippet around the
// hit. A single doc builds its index from the live DOM on open; a site/book lazy-
// loads the cross-page index (search-index.js) on first open via window.TALIESIN_SEARCH_URL,
// so the full-text index never bloats every page. Self-contained: injects its own
// themed overlay CSS and rides along as one <script> beside the TOC scrollspy. Not
// concatenated into the client.js bundle; type-checked separately (web-client/jsconfig.json).
(function () {
  if (window.taliSearchInstalled) return;
  window.taliSearchInstalled = true;

  // Honour prefers-reduced-motion for JS-initiated scrolls (the CSS
  // scroll-behavior gate doesn't cover programmatic scrollIntoView/scrollTo).
  function scrollBehavior() {
    return window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches
      ? "auto"
      : "smooth";
  }

  var CSS =
    "#tali-search{position:fixed;inset:0;z-index:10050;display:flex;justify-content:center;" +
    "align-items:flex-start;padding-top:12vh}" +
    "#tali-search[hidden]{display:none}" +
    "#tali-search .tali-s-backdrop{position:absolute;inset:0;background:rgba(0,0,0,.45);" +
    "backdrop-filter:blur(2px)}" +
    "#tali-search .tali-s-box{position:relative;width:min(38rem,92vw);max-height:70vh;display:flex;" +
    "flex-direction:column;background:var(--tali-bg,#fff);color:var(--tali-fg,#111);" +
    "border:1px solid var(--tali-border,#e0e0e0);border-radius:12px;" +
    "box-shadow:0 18px 60px rgba(0,0,0,.4);overflow:hidden}" +
    "#tali-search .tali-s-input{width:100%;box-sizing:border-box;border:0;outline:0;" +
    "padding:.95rem 1.1rem;font-size:1.05rem;background:transparent;color:inherit;" +
    "border-bottom:1px solid var(--tali-border,#e0e0e0)}" +
    "#tali-search .tali-s-results{list-style:none;margin:0;padding:.3rem;overflow:auto;flex:1}" +
    "#tali-search .tali-s-item{display:flex;flex-direction:column;gap:.15rem;padding:.5rem .7rem;" +
    "border-radius:7px;cursor:pointer;scroll-margin:.4rem}" +
    "#tali-search .tali-s-head{display:flex;align-items:baseline;gap:.6rem}" +
    "#tali-search .tali-s-item[aria-selected=true]{background:var(--tali-accent-fill,#1f6feb);color:var(--tali-on-accent,#fff)}" +
    "#tali-search .tali-s-item[aria-selected=true] .tali-s-sec{color:rgba(255,255,255,.8)}" +
    "#tali-search .tali-s-snip{font-size:.78rem;color:var(--tali-muted,#888);overflow:hidden;" +
    "text-overflow:ellipsis;white-space:nowrap}" +
    "#tali-search .tali-s-snip mark{background:transparent;color:var(--tali-link,#2563eb);font-weight:700;padding:0}" +
    "#tali-search .tali-s-item[aria-selected=true] .tali-s-snip{color:rgba(255,255,255,.85)}" +
    "#tali-search .tali-s-item[aria-selected=true] .tali-s-snip mark{color:#fff}" +
    "#tali-search .tali-s-title{font-weight:600}" +
    "#tali-search .tali-s-title mark{background:transparent;color:var(--tali-link,#2563eb);" +
    "font-weight:800;padding:0}" +
    "#tali-search .tali-s-item[aria-selected=true] .tali-s-title mark{color:#fff;" +
    "text-decoration:underline}" +
    // max-width so a long ancestor path ellipsizes instead of squeezing the title it is
    // meant to contextualise (the title is what the reader is scanning for).
    "#tali-search .tali-s-sec{font-size:.8rem;color:var(--tali-muted,#888);white-space:nowrap;margin-left:auto;" +
    "overflow:hidden;text-overflow:ellipsis;max-width:40%}" +
    "#tali-search .tali-s-action .tali-s-sec{color:var(--tali-link,#2563eb);text-transform:uppercase;" +
    "font-size:.66rem;letter-spacing:.05em;font-weight:700}" +
    // Outline rows: a chapter leads with its number, its sections are indented one step per
    // level of nesting WITHIN that chapter, and a chapter that only labels a group of
    // results is not interactive.
    "#tali-search .tali-s-label{display:flex;flex-direction:column;gap:.15rem;padding:.5rem .7rem;" +
    "border-radius:7px}" +
    "#tali-search .tali-s-chapter{margin-top:.35rem}" +
    "#tali-search .tali-s-chapter:first-child{margin-top:0}" +
    "#tali-search .tali-s-chapter .tali-s-title{font-weight:700}" +
    "#tali-search .tali-s-label .tali-s-title{color:var(--tali-muted,#888);font-size:.78rem;" +
    "text-transform:uppercase;letter-spacing:.05em}" +
    "#tali-search .tali-s-num{font-variant-numeric:tabular-nums;font-weight:700;font-size:.8rem;" +
    "color:var(--tali-muted,#888);min-width:1.4em}" +
    "#tali-search .tali-s-item[aria-selected=true] .tali-s-num{color:inherit}" +
    "#tali-search .tali-s-d1{padding-left:1.7rem}" +
    "#tali-search .tali-s-d2{padding-left:2.9rem}" +
    "#tali-search .tali-s-d3{padding-left:4.1rem}" +
    "#tali-search .tali-s-d4{padding-left:5.3rem}" +
    "#tali-search .tali-s-item[class*=tali-s-d] .tali-s-title{font-weight:400}" +
    "#tali-search .tali-s-more .tali-s-title{font-size:.8rem;color:var(--tali-link,#2563eb);font-weight:600}" +
    "#tali-search .tali-s-more[aria-selected=true] .tali-s-title{color:inherit}" +
    "#tali-search .tali-s-miss{font-size:.75rem;color:var(--tali-muted,#888)}" +
    "#tali-search .tali-s-item[aria-selected=true] .tali-s-miss{color:rgba(255,255,255,.85)}" +
    "#tali-search .tali-s-empty{padding:1rem 1.1rem;color:var(--tali-muted,#888)}" +
    "#tali-search .tali-s-hint{display:flex;gap:1rem;padding:.45rem .9rem;font-size:.72rem;" +
    "color:var(--tali-muted,#888);border-top:1px solid var(--tali-border,#e0e0e0)}" +
    "#tali-search .tali-s-hint kbd{font:inherit;border:1px solid var(--tali-border,#e0e0e0);" +
    "border-radius:4px;padding:0 .3rem}";

  function injectCss() {
    if (document.getElementById("tali-search-css")) return;
    var s = document.createElement("style");
    s.id = "tali-search-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }

  /** A built, match-ready index entry (memoized lowercase fields for the matcher). A
   * command-palette action is the same shape with `action:true` + a `run` callback and no
   * `url`/`page` (it executes instead of navigating). */
  /** @typedef {{ id: string, title: string, level: number, body: string, url?: string, page?: string, chapter?: number, path?: string, depth?: number, tLow: string, bLow: string, action?: boolean, run?: () => void }} SearchItem */

  // Lazily created in ensureUi() and always assigned before any use below, so they're
  // typed non-null; ensureUi() self-guards re-entry via `if (overlay) return`.
  /** @type {HTMLElement} */ var overlay;
  /** @type {HTMLInputElement} */ var input;
  /** @type {HTMLElement} */ var list;
  /** @type {(() => void) | null} active focus-trap release while the palette is open */
  var searchRelease = null;
  /** @type {SearchItem[]} */
  var index = [];
  /** @type {Row[]} the listbox's options, parallel with the rendered `.tali-s-item` rows */
  var matches = [];
  var sel = 0;
  /** @type {string[]} the current query's terms, for the search-hit flash */
  var lastTerms = [];

  // --- command-palette actions -----------------------------------------------------
  // Cmd-K runs commands too, not just search. Each action self-gates on a capability
  // global: `taliToggleTheme` ships on every page (theme_head), so the theme action is
  // always offered; `taliRestartKernel` / `taliOpenPageSource` ship only in the preview
  // client (client.js), so those appear only in a live preview, never a static build.
  // Matched by the same score() as content, over the title + keyword synonyms.
  /** @typedef {{ id: string, title: string, keywords: string, run: () => void, available: () => boolean }} PaletteAction */
  /** @type {PaletteAction[]} */
  var ACTIONS = [
    {
      id: "theme",
      title: "Toggle light / dark theme",
      keywords: "theme dark light mode appearance colour color",
      available: function () { return typeof window.taliToggleTheme === "function"; },
      run: function () { if (window.taliToggleTheme) window.taliToggleTheme(); },
    },
    {
      id: "kernel",
      title: "Restart kernel",
      keywords: "restart kernel jupyter python r rerun re-run execute cells",
      available: function () { return typeof window.taliRestartKernel === "function"; },
      run: function () { if (window.taliRestartKernel) window.taliRestartKernel(); },
    },
    {
      id: "source",
      title: "Open source in editor",
      keywords: "open source editor vscode file code edit",
      available: function () { return typeof window.taliOpenPageSource === "function"; },
      run: function () { if (window.taliOpenPageSource) window.taliOpenPageSource(); },
    },
  ];

  // The currently-available actions as match-ready SearchItems: level 0 so they sort with
  // top-level entries, body = keyword synonyms so score() can match them, and action+run
  // carrying the behavior. Rebuilt per render() so capability gates stay live.
  /** @returns {SearchItem[]} */
  function availableActions() {
    /** @type {SearchItem[]} */
    var out = [];
    for (var i = 0; i < ACTIONS.length; i++) {
      var a = ACTIONS[i];
      if (!a.available()) continue;
      out.push({
        id: "action:" + a.id, title: a.title, level: 0, body: a.keywords,
        tLow: a.title.toLowerCase(), bLow: a.keywords.toLowerCase(), action: true, run: a.run,
      });
    }
    return out;
  }

  // Build the index: every anchored heading, plus the lowercased text of the
  // blocks that follow it until the next heading (so body keywords match too).
  /** @returns {SearchItem[]} */
  function buildIndex() {
    // Site/book: search the whole project from the inlined cross-page index
    // (every page's title + anchored headings). A result carries its page url so
    // selecting it can navigate across chapters.
    if (window.TALIESIN_SEARCH_INDEX) {
      var built = window.TALIESIN_SEARCH_INDEX.map(function (e) {
        var body = e.b || "";
        // tLow/bLow are memoized once so the per-keystroke matcher is just indexOf scans.
        return { id: e.i, title: e.t, level: e.l, body: body, url: e.u, page: e.p,
          chapter: e.c, path: e.h, depth: 0,
          tLow: (e.t || "").toLowerCase(), bLow: body.toLowerCase() };
      });
      // Outline depth is relative to each PAGE's own shallowest heading, never the absolute
      // level: whether a chapter's sections land on h2, h3 or h4 depends on whether it emits
      // a title block and what level it happens to root at, so indenting by `level` made a
      // `###`-rooted chapter's top-level sections sit three steps in beside a `##`-rooted
      // chapter's. Computed once here, not per keystroke.
      /** @type {Record<string, number>} */
      var shallowest = {};
      built.forEach(function (it) {
        var key = it.url || "";
        if (it.level && (shallowest[key] == null || it.level < shallowest[key])) {
          shallowest[key] = it.level;
        }
      });
      built.forEach(function (it) {
        if (it.level) it.depth = it.level - shallowest[it.url || ""] + 1;
      });
      return built;
    }
    // Single doc: build from the current DOM (so it reflects live edits).
    var main = document.querySelector("main") || document.body;
    var heads = main.querySelectorAll("h1[id],h2[id],h3[id],h4[id]");
    /** @type {SearchItem[]} */
    var out = [];
    for (var i = 0; i < heads.length; i++) {
      var h = heads[i];
      var title = headingText(h);
      if (!title) continue;
      var sbody = sectionText(h, heads[i + 1]);
      out.push({
        id: h.id,
        title: title,
        level: parseInt(h.tagName.charAt(1), 10) || 1,
        body: sbody,
        tLow: title.toLowerCase(),
        bLow: sbody.toLowerCase(),
      });
    }
    return out;
  }

  // A heading's own words, without the hover `#` permalink the anchor-links enhancer
  // appends: reading `textContent` straight off the element put a trailing "#" in every
  // single-doc palette row (the same strip `toc-spy.js` does for its mobile chip).
  /** @param {Element} h */
  function headingText(h) {
    var clone = /** @type {HTMLElement} */ (h.cloneNode(true));
    clone.querySelectorAll(".tali-anchor").forEach(function (a) { a.remove(); });
    return (clone.textContent || "").trim();
  }

  /** @param {Element} h @param {Element | undefined} next */
  function sectionText(h, next) {
    var txt = "";
    var node = h.nextElementSibling;
    while (node && node !== next && txt.length < 1500) {
      txt += " " + (node.textContent || "");
      node = node.nextElementSibling;
    }
    return txt.replace(/\s+/g, " ").trim();
  }

  // Load the cross-page index from `search-index.js` (a site/book links to it via
  // TALIESIN_SEARCH_URL instead of inlining it into every page), then run `cb`. A
  // single doc (no URL) just runs `cb` against the live DOM index.
  //
  // In a LIVE PREVIEW (a websocket is present) the server's index is refreshed as pages
  // are edited, so re-fetch it on every open with a cache-busting query — otherwise the
  // once-loaded `window.TALIESIN_SEARCH_INDEX` would freeze search at page-load state. A
  // STATIC BUILD (file://, no ws) has immutable content, so load the index once.
  var fetchSeq = 0;
  var loading = false;
  /** @param {() => void} cb */
  function loadIndexThen(cb) {
    var livePreview = typeof window.TALIESIN_WS_PATH === "string" && !!window.TALIESIN_WS_PATH;
    // Single doc: no cross-page index, search the DOM.
    if (!window.TALIESIN_SEARCH_URL) {
      cb();
      return;
    }
    // Static build: the index can't change, so keep the first load.
    if (window.TALIESIN_SEARCH_INDEX && !livePreview) {
      cb();
      return;
    }
    // A fetch is already in flight (rapid re-open): use whatever we have for now.
    if (loading) {
      cb();
      return;
    }
    loading = true;
    // Load the index with a <script> element (it assigns window.TALIESIN_SEARCH_INDEX)
    // rather than fetch(): a script subresource loads under file:// too, so Cmd-K works
    // when the book is opened from disk with no dev server (fetch() of a local file is
    // CORS-blocked). The `?t=` cache-buster forces the preview re-fetch past any HTTP
    // caching; a static build omits it (the URL must stay a plain file path).
    var s = document.createElement("script");
    s.src = window.TALIESIN_SEARCH_URL + (livePreview ? "?t=" + ++fetchSeq : "");
    s.onload = function () {
      loading = false;
      s.remove();
      cb();
    };
    s.onerror = function () {
      loading = false;
      s.remove();
      // Surface the failure instead of a silently-empty palette.
      window.TALIESIN_SEARCH_LOAD_FAILED = true;
      cb();
    };
    document.head.appendChild(s);
  }

  function ensureUi() {
    injectCss();
    if (overlay) return;
    overlay = document.createElement("div");
    overlay.id = "tali-search";
    overlay.hidden = true;
    overlay.innerHTML =
      '<div class="tali-s-backdrop"></div>' +
      '<div class="tali-s-box">' +
      // ARIA 1.2 combobox: the input IS the combobox, owning aria-expanded /
      // aria-controls / aria-activedescendant; the listbox is separately named.
      '<input class="tali-s-input" type="text" autocomplete="off" spellcheck="false" ' +
      'role="combobox" aria-expanded="true" aria-haspopup="listbox" aria-autocomplete="list" ' +
      'placeholder="Search or run a command…" aria-label="Search or run a command" ' +
      'aria-controls="tali-s-results" />' +
      '<ul class="tali-s-results" id="tali-s-results" role="listbox" aria-label="Search results"></ul>' +
      '<div class="tali-s-hint"><span><kbd>↑</kbd><kbd>↓</kbd><kbd>tab</kbd> navigate</span>' +
      "<span><kbd>↵</kbd> go to</span><span><kbd>esc</kbd> close</span></div>";
    document.body.appendChild(overlay);
    // These three nodes are in the innerHTML just set, so the casts are sound.
    input = /** @type {HTMLInputElement} */ (overlay.querySelector(".tali-s-input"));
    list = /** @type {HTMLElement} */ (overlay.querySelector(".tali-s-results"));
    overlay.querySelector(".tali-s-backdrop")?.addEventListener("click", close);
    input.addEventListener("input", function () {
      render(input.value);
    });
    input.addEventListener("keydown", onKey);
  }

  function open() {
    ensureUi();
    var isSite = !!(window.TALIESIN_SEARCH_URL || window.TALIESIN_SEARCH_INDEX);
    // Single doc with no headings AND no available actions: nothing to do. With the command
    // palette the always-available theme action normally keeps Cmd-K openable even on a
    // heading-less doc, so it can still run commands.
    if (!isSite && !buildIndex().length && !availableActions().length) return;
    overlay.hidden = false;
    input.value = "";
    input.placeholder = "Search or run a command…";
    // While the cross-page index is fetching on first open, show a loading row.
    list.innerHTML = "";
    if (isSite && !window.TALIESIN_SEARCH_INDEX) {
      var li = document.createElement("li");
      li.className = "tali-s-empty";
      li.textContent = "Loading…";
      list.appendChild(li);
    }
    loadIndexThen(function () {
      index = buildIndex();
      render(input.value);
    });
    // Trap focus in the palette (focus the input); fall back to a bare focus if absent.
    if (window.taliFocusTrap) searchRelease = window.taliFocusTrap(overlay, input);
    else input.focus();
  }

  function close() {
    if (overlay) overlay.hidden = true;
    if (searchRelease) { searchRelease(); searchRelease = null; }
  }

  function isOpen() {
    return overlay && !overlay.hidden;
  }

  // Bounded edit-distance-1: true iff `a` is within one substitution / insertion / deletion /
  // ADJACENT TRANSPOSITION of `b` (Damerau-Levenshtein <= 1). O(len), no matrix.
  /** @param {string} a @param {string} b */
  function within1(a, b) {
    var la = a.length, lb = b.length;
    if (Math.abs(la - lb) > 1) return false;
    var i = 0, j = 0, diff = 0;
    while (i < la && j < lb) {
      if (a.charCodeAt(i) === b.charCodeAt(j)) { i++; j++; continue; }
      if (++diff > 1) return false;
      // Damerau: a swapped adjacent pair ("teh" for "the") is the single most common typo
      // class, and plain Levenshtein charges it two edits — so it was the one real typo the
      // fuzzy tier could never forgive. Only equal-length strings can differ by a pure swap;
      // past the end charCodeAt is NaN, which compares false, so the bounds are safe.
      if (la === lb &&
          a.charCodeAt(i) === b.charCodeAt(j + 1) &&
          a.charCodeAt(i + 1) === b.charCodeAt(j)) {
        i += 2; j += 2; continue;
      }
      if (la > lb) i++; // deletion from a
      else if (lb > la) j++; // insertion into a
      else { i++; j++; } // substitution
    }
    if (i < la || j < lb) diff++; // a trailing unmatched char is one more edit
    return diff <= 1;
  }

  // Does any whitespace-delimited word of `fieldLow` typo-match `term` (edit distance <= 1)?
  /** @param {string} term @param {string} fieldLow */
  function fuzzyWord(term, fieldLow) {
    var words = fieldLow.split(/\s+/);
    for (var k = 0; k < words.length; k++) {
      if (words[k] && within1(term, words[k])) return true;
    }
    return false;
  }

  // Multi-term matcher. Every query term hits some field (title or body) by exact substring
  // or, for terms >= 4 chars, a Damerau-distance-1 typo against a word. Returns the
  // field-boosted score plus the terms that hit NOTHING (`s` of 0 rejects). Title outranks
  // body; bonuses reward all-title hits, a title-leading match, and an exact contiguous
  // phrase. Single-term degenerates to the old prefix > contains > body ordering.
  //
  // `strict` keeps the original hard AND, and the command-palette actions are the reason it
  // still exists: they are scored by this same function and pinned above content, so under
  // relaxed AND a query about prose would surface "Toggle light / dark theme" (one stray
  // keyword hit) above the section the reader asked for. Content relaxes; actions do not.
  /** @typedef {{ s: number, missing: string[] }} Scored */
  /** @param {SearchItem} item @param {string[]} terms @param {boolean} strict @returns {Scored} */
  function score(item, terms, strict) {
    var t = item.tLow, b = item.bLow, total = 0, allTitle = true, leadPrefix = false;
    /** @type {string[]} */
    var missing = [];
    for (var k = 0; k < terms.length; k++) {
      var term = terms[k], pos = t.indexOf(term);
      if (pos >= 0) { total += 6; if (pos === 0) leadPrefix = true; }
      else if (b.indexOf(term) >= 0) { total += 3; allTitle = false; }
      else if (term.length >= 4 && fuzzyWord(term, t)) { total += 2; }
      else if (term.length >= 4 && fuzzyWord(term, b)) { total += 1; allTitle = false; }
      else if (strict) return { s: 0, missing: [] }; // hard AND: one miss rejects the item
      else { missing.push(term); allTitle = false; }
    }
    // Relaxed still rejects an item that matched *nothing*, so a nonsense query stays an
    // honest "No matches" rather than scoring the whole book at its floor.
    if (terms.length && missing.length === terms.length) return { s: 0, missing: [] };
    if (allTitle) total += 3;
    if (leadPrefix) total += 2;
    if (terms.length > 1) {
      var phrase = terms.join(" "); // the normalized query, contiguous
      if (t.indexOf(phrase) >= 0) total += 2;
      else if (b.indexOf(phrase) >= 0) total += 1;
    }
    return { s: total, missing: missing };
  }

  /** One rendered row. `pick` rows are the listbox's real options (keyboard-selectable, in
   * `matches`); a non-`pick` row is a chapter LABEL. `head` renders the row as a chapter
   * rather than a section; `expand` makes it the "+N more" disclosure for that page url. */
  /** @typedef {{ it: SearchItem, missing: string[], pick: boolean, s?: number, head?: boolean, expand?: string }} Row */

  var PER_GROUP = 3; // matching sections shown per chapter before the "+N more" row
  var MAX_RESULTS = 60; // relaxed AND has no natural bound, so cap the scored set
  /** @type {Record<string, boolean>} pages the reader expanded; reset on every new query */
  var expandedPages = {};
  /** @type {string | null} */
  var lastQuery = null;
  // Whether results group by page: a site/book index carries `url` on every record, the
  // single-doc DOM index does not, so there is nothing to group a single doc by.
  var grouped = false;

  /** @param {string} query @param {number} [keepSel] preserve the cursor across a re-render */
  function render(query, keepSel) {
    var q = query.trim().toLowerCase();
    var terms = q ? q.split(/\s+/).filter(Boolean) : [];
    lastTerms = terms; // so go() can flash the matched term after navigating
    if (q !== lastQuery) { expandedPages = {}; lastQuery = q; }
    grouped = !!window.TALIESIN_SEARCH_INDEX;
    // Command-palette actions come first: all available ones when the query is empty (a
    // discoverable menu — the point of a palette), else those whose title/keywords match.
    var acts = availableActions();
    if (terms.length) {
      acts = acts.filter(function (a) { return score(a, terms, true).s > 0; });
    }
    /** @type {Row[]} in DOM order, options and labels interleaved */
    var view = acts.map(function (a) { return { it: a, missing: [], pick: true }; });

    if (!terms.length) {
      // No query: the whole-book OUTLINE — every page AND every section under it, in reading
      // order (`page_fragment` emits them that way already), indented by heading level. This
      // used to filter to `level === 0`, i.e. the same flat chapter list the drawer shows,
      // leaving every section record in the index reachable only by typing a query that
      // happened to match it. A single doc shows its heading list, as before.
      index.forEach(function (it) {
        view.push({ it: it, missing: [], pick: true, head: grouped && !it.level });
      });
    } else {
      /** @type {{ it: SearchItem, s: number, missing: string[] }[]} */
      var scored = [];
      for (var i = 0; i < index.length; i++) {
        var m = score(index[i], terms, false);
        if (m.s > 0) scored.push({ it: index[i], s: m.s, missing: m.missing });
      }
      // Full matches first, then partials by how much they miss, then by score. A partial is
      // shown rather than dropped because hard AND meant one mistyped word annihilated the
      // whole result set; it is ranked below every full match and says what it missed.
      scored.sort(function (a, b) {
        return a.missing.length - b.missing.length || b.s - a.s ||
          (a.it.level || 0) - (b.it.level || 0);
      });
      if (scored.length > MAX_RESULTS) scored = scored.slice(0, MAX_RESULTS);
      if (!grouped) {
        scored.forEach(function (h) {
          view.push({ it: h.it, missing: h.missing, pick: true, s: h.s });
        });
      } else {
        // Group by page, pages in best-hit order, so one dense chapter can't monopolise the
        // visible rows: each page shows its top few sections and offers the rest.
        /** @type {string[]} */
        var order = [];
        /** @type {Record<string, { it: SearchItem, s: number, missing: string[] }[]>} */
        var byPage = {};
        scored.forEach(function (h) {
          var key = h.it.url || "";
          if (!byPage[key]) { byPage[key] = []; order.push(key); }
          byPage[key].push(h);
        });
        order.forEach(function (key) {
          var hits = byPage[key];
          // If the page's OWN entry matched, that entry is the chapter row (one row, not a
          // label plus a duplicate of it) and it is selectable; otherwise the row is a plain
          // label, so Enter on the top result still lands on a section, never on a chapter
          // the query never matched.
          /** @type {{ it: SearchItem, s: number, missing: string[] } | null} */
          var pageHit = null;
          for (var k = 0; k < hits.length; k++) {
            if (!hits[k].it.level) { pageHit = hits.splice(k, 1)[0]; break; }
          }
          var ref = pageHit ? pageHit.it : hits[0].it;
          view.push({
            it: pageHit ? pageHit.it : {
              id: "", title: ref.page || ref.url || "", level: 0, body: "",
              url: ref.url, page: ref.page, chapter: ref.chapter, tLow: "", bLow: "",
            },
            missing: pageHit ? pageHit.missing : [],
            pick: !!pageHit,
            s: pageHit ? pageHit.s : undefined,
            head: true,
          });
          // The per-page cap exists to stop ONE dense chapter monopolising the visible rows.
          // When every hit is on the same page there is nothing to balance against, so
          // capping would only hide results the reader asked for.
          var cap = order.length > 1 ? PER_GROUP : hits.length;
          var show = expandedPages[key] ? hits.length : Math.min(hits.length, cap);
          for (var s = 0; s < show; s++) {
            view.push({ it: hits[s].it, missing: hits[s].missing, pick: true, s: hits[s].s });
          }
          if (show < hits.length) {
            var n = hits.length - show;
            view.push({
              it: { id: "", title: "+" + n + " more in this chapter", level: 0, body: "",
                    tLow: "", bLow: "" },
              missing: [], pick: true, expand: key,
            });
          }
        });
      }
    }

    matches = view.filter(function (r) { return r.pick; });
    sel = keepSel == null ? startRow(terms, acts.length) : Math.max(0, Math.min(keepSel, matches.length - 1));
    list.innerHTML = "";
    if (!matches.length) {
      var empty = document.createElement("li");
      empty.className = "tali-s-empty";
      empty.textContent = window.TALIESIN_SEARCH_LOAD_FAILED
        ? "Search index failed to load"
        : "No matches";
      list.appendChild(empty);
      // No option is active: drop any stale reference so AT doesn't announce a
      // removed row (the listbox is now empty).
      input.removeAttribute("aria-activedescendant");
      return;
    }
    // Option ids number the PICK rows only, so they stay parallel with `matches` (which is
    // what markSel() indexes) no matter how many labels sit between them.
    var opt = 0;
    view.forEach(function (r) { list.appendChild(itemEl(r, terms, r.pick ? opt++ : -1)); });
    markSel();
  }

  // Where the cursor starts. A chapter row sits at the top of its group for STRUCTURE, not
  // because it scored best, so "first row" and "best match" came apart the moment results
  // grouped: Enter must still land on the strongest match. Actions keep their pinned
  // position (a matching command is what the reader asked for by typing its name).
  /** @param {string[]} terms @param {number} actionCount */
  function startRow(terms, actionCount) {
    if (!terms.length || actionCount) return 0;
    var best = 0, bestS = -1;
    matches.forEach(function (r, i) {
      if (r.s != null && r.s > bestS) { bestS = r.s; best = i; }
    });
    return best;
  }

  /** @param {Row} r @param {string[]} terms @param {number} i */
  function itemEl(r, terms, i) {
    var item = r.it;
    var li = document.createElement("li");
    var cls = r.pick ? "tali-s-item" : "tali-s-label";
    if (item.action) cls += " tali-s-action";
    if (r.head) cls += " tali-s-chapter";
    if (r.expand) cls += " tali-s-more";
    // Indent a section under its chapter by its depth within that chapter (see buildIndex),
    // so the outline reads as one.
    if (!r.head && !r.expand && !item.action && grouped && item.depth) {
      cls += " tali-s-d" + Math.min(item.depth, 4);
    }
    li.className = cls;
    if (r.pick) {
      li.setAttribute("role", "option");
      li.id = "tali-s-opt-" + i;
    } else {
      li.setAttribute("role", "presentation");
    }
    var head = document.createElement("div");
    head.className = "tali-s-head";
    // A book chapter leads with its number: the page-title record carries the bare title
    // (the rendered numbers live on section headings), so without `c` the outline's chapter
    // rows would be the only unnumbered thing in a numbered book.
    if (r.head && item.chapter != null) {
      var num = document.createElement("span");
      num.className = "tali-s-num";
      num.textContent = String(item.chapter);
      head.appendChild(num);
    }
    var title = document.createElement("span");
    title.className = "tali-s-title";
    highlight(title, item.title, terms);
    var sec = document.createElement("span");
    sec.className = "tali-s-sec";
    // Label an action "action" so it reads as a command, not a destination; a grouped result
    // gets its ancestor heading path (the chapter is already the row above it); an ungrouped
    // single-doc result keeps its heading level.
    if (item.action) sec.textContent = "action";
    else if (!grouped) sec.textContent = item.page || "H" + item.level;
    else if (terms.length && item.path) sec.textContent = item.path;
    else sec.textContent = "";
    head.append(title, sec);
    li.appendChild(head);
    // Say which terms did NOT match rather than silently returning a weaker result than the
    // reader asked for: struck through, because that is the part of the query not honoured.
    if (r.missing.length) {
      var miss = document.createElement("div");
      miss.className = "tali-s-miss";
      miss.appendChild(document.createTextNode("Missing: "));
      r.missing.forEach(function (term, k) {
        if (k) miss.appendChild(document.createTextNode(", "));
        var st = document.createElement("s");
        st.textContent = term;
        miss.appendChild(st);
      });
      li.appendChild(miss);
    }
    // A body snippet when the body carries something the title doesn't already show. "In the
    // title" matches score()'s notion (exact OR a >=4-char fuzzy hit) over the terms that
    // actually matched, so neither a fuzzy-title match nor a missing term triggers an
    // unmarkable body snippet. Actions never snippet (their "body" is just keyword synonyms
    // for matching, not prose to show).
    var hit = terms.filter(function (term) { return r.missing.indexOf(term) < 0; });
    var everyInTitle = hit.every(function (term) {
      return item.tLow.indexOf(term) >= 0 || (term.length >= 4 && fuzzyWord(term, item.tLow));
    });
    if (!item.action && hit.length && !everyInTitle && item.body) {
      var snip = document.createElement("div");
      snip.className = "tali-s-snip";
      snippet(snip, item.body, hit);
      li.appendChild(snip);
    }
    if (!r.pick) return li; // a label is not hoverable, selectable or clickable
    li.addEventListener("mousemove", function () {
      if (sel !== i) {
        sel = i;
        markSel();
      }
    });
    li.addEventListener("click", function () {
      go(r);
    });
    return li;
  }

  // Every [start,end) span where a term occurs in `text` (case-insensitive substring, all
  // occurrences). Fuzzy-only terms (no substring) yield no span — honest, never the wrong run.
  /** @param {string} text @param {string[]} terms @returns {number[][]} */
  function termRanges(text, terms) {
    var low = text.toLowerCase();
    /** @type {number[][]} */
    var ranges = [];
    // If lowercasing changed the length (rare Unicode), low-derived offsets no longer align
    // with the original-case slice; skip marking rather than mis-place a <mark>.
    if (low.length !== text.length) return ranges;
    for (var k = 0; k < terms.length; k++) {
      var term = terms[k], from = 0, pos;
      if (!term) continue;
      while ((pos = low.indexOf(term, from)) >= 0) {
        ranges.push([pos, pos + term.length]);
        from = pos + term.length;
      }
    }
    return ranges;
  }

  // Emit `sourceText` into `el` as alternating text nodes / <mark>s over the given ranges
  // (sorted + merged so overlapping/adjacent terms become one continuous mark). DOM-built,
  // never innerHTML; the original-case source is sliced for display.
  /** @param {HTMLElement} el @param {string} sourceText @param {number[][]} ranges */
  function emitRanges(el, sourceText, ranges) {
    if (!ranges.length) { el.appendChild(document.createTextNode(sourceText)); return; }
    ranges.sort(function (a, b) { return a[0] - b[0] || a[1] - b[1]; });
    var merged = [ranges[0].slice()];
    for (var k = 1; k < ranges.length; k++) {
      var last = merged[merged.length - 1], r = ranges[k];
      if (r[0] <= last[1]) { if (r[1] > last[1]) last[1] = r[1]; }
      else merged.push(r.slice());
    }
    var cur = 0;
    for (var m = 0; m < merged.length; m++) {
      var s = merged[m][0], e = merged[m][1];
      if (s > cur) el.appendChild(document.createTextNode(sourceText.slice(cur, s)));
      var mk = document.createElement("mark");
      mk.textContent = sourceText.slice(s, e);
      el.appendChild(mk);
      cur = e;
    }
    if (cur < sourceText.length) el.appendChild(document.createTextNode(sourceText.slice(cur)));
  }

  // A one-line body excerpt: the ~140-char window covering the most distinct terms, each term
  // occurrence inside it marked. Falls back to the head of the body when no term is present.
  /** @param {HTMLElement} el @param {string} body @param {string[]} terms */
  function snippet(el, body, terms) {
    var low = body.toLowerCase(), WINDOW = 140;
    /** @type {number[]} */
    var offs = [];
    // Length-preserving lowercase only (see termRanges); otherwise fall back to an unmarked head.
    if (low.length === body.length) {
      for (var k = 0; k < terms.length; k++) {
        var pos = low.indexOf(terms[k]);
        if (pos >= 0) offs.push(pos);
      }
    }
    if (!offs.length) {
      el.textContent = body.slice(0, 120) + (body.length > 120 ? " …" : "");
      return;
    }
    offs.sort(function (a, b) { return a - b; });
    // Pick the window (already expanded left for context) covering the most distinct terms,
    // counting over the SAME window that will be rendered.
    var bestStart = Math.max(0, offs[0] - 30), bestCount = -1;
    for (var a = 0; a < offs.length; a++) {
      var winStart = Math.max(0, offs[a] - 30), count = 0;
      for (var b2 = 0; b2 < offs.length; b2++) {
        if (offs[b2] >= winStart && offs[b2] < winStart + WINDOW) count++;
      }
      if (count > bestCount) { bestCount = count; bestStart = winStart; }
    }
    var start = bestStart, end = Math.min(body.length, start + WINDOW);
    var slice = body.slice(start, end);
    if (start > 0) el.appendChild(document.createTextNode("… "));
    emitRanges(el, slice, termRanges(slice, terms));
    if (end < body.length) el.appendChild(document.createTextNode(" …"));
  }

  // Render `title` with every term occurrence wrapped in <mark>.
  /** @param {HTMLElement} el @param {string} title @param {string[]} terms */
  function highlight(el, title, terms) {
    emitRanges(el, title, termRanges(title, terms));
  }

  function markSel() {
    list.querySelectorAll(".tali-s-item").forEach(function (opt, i) {
      var on = i === sel;
      opt.setAttribute("aria-selected", on ? "true" : "false");
      if (on) {
        opt.scrollIntoView({ block: "nearest" });
        input.setAttribute("aria-activedescendant", opt.id);
      }
    });
  }

  /**
   * Move the selection by `step`, wrapping. No-op when there is nothing to move through.
   * @param {number} step
   */
  function move(step) {
    if (!matches.length) return;
    sel = (sel + step + matches.length) % matches.length;
    markSel();
  }

  /** @param {KeyboardEvent} e */
  function onKey(e) {
    // Tab / Shift-Tab move the selection exactly as the arrows do, because that is what a
    // reader's hands expect in a palette.
    //
    // Tab was not *escaping* the overlay before this (the shared modal trap in
    // `04-focus-trap.js` already confines it on a capture listener) — it was simply inert:
    // the input is the palette's only focusable element, so the trap kept cycling focus
    // back to it and the key did nothing at all. Browser-measured, not assumed.
    if (e.key === "ArrowDown" || (e.key === "Tab" && !e.shiftKey)) {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowUp" || (e.key === "Tab" && e.shiftKey)) {
      e.preventDefault();
      move(-1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (matches[sel]) go(matches[sel]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  // --- search-hit flash (land on the heading, then flash the matched term) --------
  // CSS Custom Highlight API (zero DOM mutation → honours read-only preview), with a
  // transient <mark> fallback for engines without it. Registered once, lazily.
  var FLASH_KEY = "tali-search-flash";
  /** @type {Highlight | null} */
  var flashHl = null;
  // Create + register the highlight LAZILY on first use (not at module load — the Custom
  // Highlight API guard can read falsy during early script evaluation on some engines),
  // and RETRY on each call until it succeeds (no one-shot latch that would strand a
  // transient early failure on the <mark> fallback for the rest of the session).
  // NOTE: this module shadows the global `CSS` with a local `var CSS` (the overlay
  // stylesheet string), so the Custom Highlight API must be reached via `window.CSS`.
  function flashHighlight() {
    if (!flashHl && window.CSS && window.CSS.highlights && window.Highlight) {
      try {
        flashHl = new Highlight();
        window.CSS.highlights.set(FLASH_KEY, flashHl);
      } catch (e) {
        flashHl = null;
      }
    }
    return flashHl;
  }
  var flashTimer = 0;
  /** @type {HTMLElement | null} */
  var flashMark = null;
  function clearFlash() {
    clearTimeout(flashTimer);
    document.documentElement.classList.remove("tali-search-flashing");
    if (flashHl) flashHl.clear();
    if (flashMark) {
      var p = flashMark.parentNode;
      if (p) {
        while (flashMark.firstChild) p.insertBefore(flashMark.firstChild, flashMark);
        p.removeChild(flashMark);
        p.normalize();
      }
      flashMark = null;
    }
  }
  // The first substring occurrence of any `terms` entry within `[start, next heading)`,
  // as a Range — or null (fuzzy-/title-only matches have no substring occurrence here).
  /** @param {Element | null} startEl @param {string[]} terms @returns {Range | null} */
  function firstTermRange(startEl, terms) {
    var low = terms.filter(Boolean).map(function (t) { return t.toLowerCase(); });
    if (!low.length || !startEl) return null;
    /** @type {Element | null} */
    var el = startEl;
    while (el) {
      var walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, null);
      var tn;
      while ((tn = walker.nextNode())) {
        var text = tn.nodeValue || "", tl = text.toLowerCase();
        if (tl.length !== text.length) continue; // offset-safety, as in termRanges
        var best = -1, bestLen = 0;
        for (var k = 0; k < low.length; k++) {
          var pos = tl.indexOf(low[k]);
          if (pos >= 0 && (best < 0 || pos < best)) { best = pos; bestLen = low[k].length; }
        }
        if (best >= 0) {
          var r = document.createRange();
          r.setStart(tn, best);
          r.setEnd(tn, best + bestLen);
          return r;
        }
      }
      // Advance to the next sibling block; stop at the next heading (section boundary).
      el = el.nextElementSibling;
      if (el && /^H[1-6]$/.test(el.tagName)) break;
    }
    return null;
  }
  // Flash the first occurrence of `terms` in the section headed by `headingEl`. Scrolls
  // to it only if off-screen (the heading is already in view). No-op on decks / no match.
  /** @param {Element | null} headingEl @param {string[]} terms */
  function flashTermsIn(headingEl, terms) {
    if (document.querySelector(".tali-deck")) return; // decks have their own chrome
    if (!headingEl || !terms || !terms.length) return;
    var range = firstTermRange(headingEl, terms);
    if (!range) return;
    var rect = range.getBoundingClientRect();
    var vh = window.innerHeight || document.documentElement.clientHeight;
    if (rect.bottom < 0 || rect.top > vh) {
      var host = range.startContainer.parentElement;
      if (host) host.scrollIntoView({ behavior: scrollBehavior(), block: "center" });
    }
    clearFlash();
    var hl = flashHighlight();
    if (hl) hl.add(range);
    else {
      try {
        var m = document.createElement("mark");
        m.className = "tali-search-mark";
        range.surroundContents(m);
        flashMark = m;
      } catch (e) { return; }
    }
    // Restart the fade animation (remove → reflow → add) even on a repeat search.
    var de = document.documentElement;
    de.classList.remove("tali-search-flashing");
    void de.offsetWidth;
    de.classList.add("tali-search-flashing");
    flashTimer = setTimeout(clearFlash, 1600);
  }

  // Select the tab owning a collapsed panel, through the same click path a reader would use
  // (so aria-selected + the roving tabindex stay correct).
  /** @param {Element} panel */
  function selectOwningTab(panel) {
    if (!panel.id) return;
    var tabs = document.querySelectorAll('[role="tab"][aria-controls]');
    for (var i = 0; i < tabs.length; i++) {
      if (tabs[i].getAttribute("aria-controls") === panel.id) {
        /** @type {HTMLElement} */ (tabs[i]).click();
        return;
      }
    }
  }

  // The matched text can sit in a COLLAPSED tab panel. `hidden="until-found"` lets the
  // browser reveal one for its own find-in-page, but a programmatic jump gets no such
  // courtesy — and the index anchors a section to its HEADING, which is a sibling of the
  // tabset rather than inside it, so walking ancestors alone finds nothing. Do both: reveal
  // any collapsed panel the target is inside, then, within the landed section, reveal the
  // one that actually contains a query term. Without this the reader lands on the right
  // section with their phrase still hidden behind a tab, which is worse than not matching.
  /** @param {Element | null} target @param {string[]} terms */
  function revealFor(target, terms) {
    if (!target) return;
    /** @type {Element | null} */
    var node = target;
    for (; node; node = node.parentElement) {
      if (node.getAttribute("hidden") === "until-found") selectOwningTab(node);
    }
    var low = terms.filter(Boolean).map(function (t) { return t.toLowerCase(); });
    if (!low.length) return;
    // Blocks are flat siblings, so the section runs from the heading to the next heading.
    for (var el = target.nextElementSibling; el; el = el.nextElementSibling) {
      if (/^H[1-6]$/.test(el.tagName)) break;
      var panels = el.querySelectorAll('[hidden="until-found"]');
      for (var i = 0; i < panels.length; i++) {
        var text = (panels[i].textContent || "").toLowerCase();
        for (var k = 0; k < low.length; k++) {
          if (text.indexOf(low[k]) >= 0) { selectOwningTab(panels[i]); break; }
        }
      }
    }
  }

  /** @param {Row} r */
  function go(r) {
    // The "+N more in this chapter" row is a disclosure, not a destination: expand its page
    // and re-render in place, keeping the cursor where the reader left it.
    if (r.expand) {
      expandedPages[r.expand] = true;
      render(input.value, sel);
      return;
    }
    var item = r.it;
    // A command-palette action runs its command and closes; a content result navigates.
    if (item.action && typeof item.run === "function") {
      close();
      try { item.run(); } catch (e) {}
      return;
    }
    close();
    var terms = lastTerms.slice();
    // A result on another page navigates there (a real page load, anchored to the
    // heading); on this page — or in a single doc — it scrolls in place. The flash is
    // handed to the destination via sessionStorage so both paths share one code path.
    if (item.url != null && item.url !== window.TALIESIN_PAGE_URL) {
      try {
        if (terms.length && item.id) {
          sessionStorage.setItem(FLASH_KEY, JSON.stringify(terms));
        }
      } catch (e) {}
      window.location.href =
        (window.TALIESIN_SITE_ROOT || "") + item.url + (item.id ? "#" + item.id : "");
      return;
    }
    if (!item.id) {
      window.scrollTo({ top: 0, behavior: scrollBehavior() });
      return;
    }
    var target = document.getElementById(item.id);
    if (!target) return;
    revealFor(target, terms); // a hit behind a collapsed tab, before measuring anything
    if (history.replaceState) history.replaceState(null, "", "#" + item.id);
    target.scrollIntoView({ behavior: scrollBehavior(), block: "start" });
    flashTermsIn(target, terms);
  }

  // Cross-page arrival: if the previous page stashed search terms, flash the term at the
  // URL's #anchor once, then clear the stash (so a manual reload doesn't re-flash).
  function flashFromSession() {
    var raw;
    try { raw = sessionStorage.getItem(FLASH_KEY); } catch (e) { return; }
    if (!raw) return;
    try { sessionStorage.removeItem(FLASH_KEY); } catch (e) {}
    /** @type {string[]} */
    var terms;
    try { terms = JSON.parse(raw); } catch (e) { return; }
    if (!Array.isArray(terms) || !terms.length) return;
    var id = decodeURIComponent((location.hash || "").replace(/^#/, ""));
    // `const` so the `if (!target) return` null-narrowing survives into the setTimeout closure.
    const target = id && document.getElementById(id);
    if (!target) return;
    // Both the reveal and the flash run in a macrotask, not inline. `revealFor` clicks the
    // owning tab so the reveal goes through tabset.js's own handler, and tabset.js registers
    // that handler on DOMContentLoaded too — inline here, whichever listener happened to be
    // added first would decide whether the click did anything. A timeout is after every
    // DOMContentLoaded handler by construction, not by luck.
    setTimeout(function () {
      revealFor(target, terms); // the cross-page half of the same problem
      flashTermsIn(target, terms);
    }, 60);
  }

  document.addEventListener(
    "keydown",
    function (e) {
      if ((e.metaKey || e.ctrlKey) && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        isOpen() ? close() : open();
      }
    },
    true,
  );

  // A visible search control (the navbar / book-sidebar button) carries
  // `data-qmd-search`; clicking it opens the same palette as Cmd-K. Delegated, so
  // it works no matter when the control entered the DOM.
  document.addEventListener("click", function (e) {
    if (e.target instanceof Element && e.target.closest("[data-qmd-search]")) {
      e.preventDefault();
      isOpen() ? close() : open();
    }
  });

  // Programmatic opener so the keyboard reader's `/` shortcut (and any UI) can open the
  // palette without synthesizing a Cmd-K event.
  window.taliOpenSearch = open;

  // The `.tali-search-kbd` badge is server-rendered with the Mac glyph (⌘K) since the same
  // HTML ships to every OS. On non-Mac platforms, rewrite it to "Ctrl K". (The button's
  // aria-keyshortcuts already lists both Control+K and Meta+K, so only the visible hint
  // needs localizing.)
  var IS_MAC = /Mac|iPhone|iPad|iPod/i.test(
    navigator.platform || navigator.userAgent || "",
  );
  function localizeSearchKbd() {
    if (IS_MAC) return;
    document.querySelectorAll(".tali-search-kbd").forEach(function (kbd) {
      kbd.textContent = "Ctrl K";
    });
  }
  function onReady() {
    localizeSearchKbd();
    flashFromSession(); // flash a cross-page search hit on arrival
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", onReady);
  } else {
    onReady();
  }
})();
