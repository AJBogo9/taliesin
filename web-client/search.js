// Client-side command palette (Cmd/Ctrl-K): full-text search to jump around a
// long document — the book, a paper, any page with a table of contents. Matches
// both headings and the body text of each section, and shows a snippet around the
// hit. A single doc builds its index from the live DOM on open; a site/book lazy-
// loads the cross-page index (search-index.js) on first open via window.TALIESIN_SEARCH_URL,
// so the full-text index never bloats every page. Self-contained: injects its own
// themed overlay CSS and rides along as one <script> beside the TOC scrollspy. Not
// part of the type-checked client.js bundle.
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
    "#tali-search .tali-s-sec{font-size:.8rem;color:var(--tali-muted,#888);white-space:nowrap;margin-left:auto}" +
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

  var overlay, input, list;
  var searchRelease = null; // active focus-trap release while the palette is open
  var index = [];
  var matches = [];
  var sel = 0;
  var lastTerms = []; // the current query's terms, for the search-hit flash

  // Build the index: every anchored heading, plus the lowercased text of the
  // blocks that follow it until the next heading (so body keywords match too).
  function buildIndex() {
    // Site/book: search the whole project from the inlined cross-page index
    // (every page's title + anchored headings). A result carries its page url so
    // selecting it can navigate across chapters.
    if (window.TALIESIN_SEARCH_INDEX) {
      return window.TALIESIN_SEARCH_INDEX.map(function (e) {
        var body = e.b || "";
        // tLow/bLow are memoized once so the per-keystroke matcher is just indexOf scans.
        return { id: e.i, title: e.t, level: e.l, body: body, url: e.u, page: e.p,
          tLow: (e.t || "").toLowerCase(), bLow: body.toLowerCase() };
      });
    }
    // Single doc: build from the current DOM (so it reflects live edits).
    var main = document.querySelector("main") || document.body;
    var heads = main.querySelectorAll("h1[id],h2[id],h3[id],h4[id]");
    var out = [];
    for (var i = 0; i < heads.length; i++) {
      var h = heads[i];
      var title = (h.textContent || "").trim();
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

  function sectionText(h, next) {
    var txt = "";
    var node = h.nextElementSibling;
    while (node && node !== next && txt.length < 1500) {
      txt += " " + (node.textContent || "");
      node = node.nextElementSibling;
    }
    return txt.replace(/\s+/g, " ").trim();
  }

  // Lazy-load the cross-page index from `search-index.js` on first open (a site/book
  // links to it via TALIESIN_SEARCH_URL instead of inlining it into every page), then
  // run `cb`. A single doc (no URL) just runs `cb` against the DOM index.
  var indexFetched = false;
  function loadIndexThen(cb) {
    if (window.TALIESIN_SEARCH_INDEX || !window.TALIESIN_SEARCH_URL || indexFetched) {
      cb();
      return;
    }
    indexFetched = true;
    // Load the index with a <script> element (it assigns window.TALIESIN_SEARCH_INDEX)
    // rather than fetch(): a script subresource loads under file:// too, so Cmd-K
    // works when the book is opened from disk with no dev server (fetch() of a local
    // file is CORS-blocked). Still lazy: only injected on the first palette open.
    var s = document.createElement("script");
    s.src = window.TALIESIN_SEARCH_URL;
    s.onload = function () {
      cb();
    };
    s.onerror = function () {
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
      'placeholder="Search this document…" aria-label="Search this document" ' +
      'aria-controls="tali-s-results" />' +
      '<ul class="tali-s-results" id="tali-s-results" role="listbox" aria-label="Search results"></ul>' +
      '<div class="tali-s-hint"><span><kbd>↑</kbd><kbd>↓</kbd> navigate</span>' +
      "<span><kbd>↵</kbd> go to</span><span><kbd>esc</kbd> close</span></div>";
    document.body.appendChild(overlay);
    input = overlay.querySelector(".tali-s-input");
    list = overlay.querySelector(".tali-s-results");
    overlay.querySelector(".tali-s-backdrop").addEventListener("click", close);
    input.addEventListener("input", function () {
      render(input.value);
    });
    input.addEventListener("keydown", onKey);
  }

  function open() {
    ensureUi();
    var isSite = !!(window.TALIESIN_SEARCH_URL || window.TALIESIN_SEARCH_INDEX);
    // Single doc with no headings: nothing to search.
    if (!isSite && !buildIndex().length) return;
    overlay.hidden = false;
    input.value = "";
    input.placeholder = isSite ? "Search…" : "Search this document…";
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

  // Bounded edit-distance-1: true iff `a` is within one substitution / insertion / deletion of
  // `b` (Levenshtein <= 1). O(len), no matrix. Transpositions count as 2 (out of scope for v1).
  function within1(a, b) {
    var la = a.length, lb = b.length;
    if (Math.abs(la - lb) > 1) return false;
    var i = 0, j = 0, diff = 0;
    while (i < la && j < lb) {
      if (a.charCodeAt(i) === b.charCodeAt(j)) { i++; j++; continue; }
      if (++diff > 1) return false;
      if (la > lb) i++; // deletion from a
      else if (lb > la) j++; // insertion into a
      else { i++; j++; } // substitution
    }
    if (i < la || j < lb) diff++; // a trailing unmatched char is one more edit
    return diff <= 1;
  }

  // Does any whitespace-delimited word of `fieldLow` typo-match `term` (edit distance <= 1)?
  function fuzzyWord(term, fieldLow) {
    var words = fieldLow.split(/\s+/);
    for (var k = 0; k < words.length; k++) {
      if (words[k] && within1(term, words[k])) return true;
    }
    return false;
  }

  // Multi-term AND matcher. Every query term must hit some field (title or body) by exact
  // substring or, for terms >= 4 chars, an edit-distance-1 typo against a word. Returns a
  // field-boosted score (0 rejects). Title outranks body; bonuses reward all-title hits, a
  // title-leading match, and an exact contiguous phrase. Single-term degenerates to the old
  // prefix > contains > body ordering.
  function score(item, terms) {
    var t = item.tLow, b = item.bLow, total = 0, allTitle = true, leadPrefix = false;
    for (var k = 0; k < terms.length; k++) {
      var term = terms[k], pos = t.indexOf(term);
      if (pos >= 0) { total += 6; if (pos === 0) leadPrefix = true; }
      else if (b.indexOf(term) >= 0) { total += 3; allTitle = false; }
      else if (term.length >= 4 && fuzzyWord(term, t)) { total += 2; }
      else if (term.length >= 4 && fuzzyWord(term, b)) { total += 1; allTitle = false; }
      else return 0; // AND: this term matched nothing -> reject the item
    }
    if (allTitle) total += 3;
    if (leadPrefix) total += 2;
    if (terms.length > 1) {
      var phrase = terms.join(" "); // the normalized query, contiguous
      if (t.indexOf(phrase) >= 0) total += 2;
      else if (b.indexOf(phrase) >= 0) total += 1;
    }
    return total;
  }

  function render(query) {
    var q = query.trim().toLowerCase();
    var terms = q ? q.split(/\s+/).filter(Boolean) : [];
    lastTerms = terms; // so go() can flash the matched term after navigating
    if (!terms.length) {
      // No query: a book shows its chapter list (the level-0 page entries) as a
      // jump menu; a single doc shows its full heading outline.
      matches = window.TALIESIN_SEARCH_INDEX
        ? index.filter(function (it) { return it.level === 0; })
        : index.slice();
    } else {
      matches = index
        .map(function (it) { return { it: it, s: score(it, terms) }; })
        .filter(function (m) { return m.s > 0; })
        .sort(function (a, b) { return b.s - a.s || (a.it.level || 0) - (b.it.level || 0); })
        .map(function (m) { return m.it; });
    }
    sel = 0;
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
    matches.forEach(function (m, i) { list.appendChild(itemEl(m, terms, i)); });
    markSel();
  }

  function itemEl(item, terms, i) {
    var li = document.createElement("li");
    li.className = "tali-s-item";
    li.setAttribute("role", "option");
    li.id = "tali-s-opt-" + i;
    var head = document.createElement("div");
    head.className = "tali-s-head";
    var title = document.createElement("span");
    title.className = "tali-s-title";
    highlight(title, item.title, terms);
    var sec = document.createElement("span");
    sec.className = "tali-s-sec";
    // In a book, label the result with its chapter; otherwise its heading level.
    sec.textContent = item.page || "H" + item.level;
    head.append(title, sec);
    li.appendChild(head);
    // A body snippet when the body carries something the title doesn't already show. "In the
    // title" matches score()'s notion (exact OR a >=4-char fuzzy hit), so a fuzzy-title match
    // doesn't trigger an unmarkable body snippet.
    var everyInTitle = terms.every(function (term) {
      return item.tLow.indexOf(term) >= 0 || (term.length >= 4 && fuzzyWord(term, item.tLow));
    });
    if (terms.length && !everyInTitle && item.body) {
      var snip = document.createElement("div");
      snip.className = "tali-s-snip";
      snippet(snip, item.body, terms);
      li.appendChild(snip);
    }
    li.addEventListener("mousemove", function () {
      if (sel !== i) {
        sel = i;
        markSel();
      }
    });
    li.addEventListener("click", function () {
      go(item);
    });
    return li;
  }

  // Every [start,end) span where a term occurs in `text` (case-insensitive substring, all
  // occurrences). Fuzzy-only terms (no substring) yield no span — honest, never the wrong run.
  function termRanges(text, terms) {
    var low = text.toLowerCase(), ranges = [];
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
  function snippet(el, body, terms) {
    var low = body.toLowerCase(), WINDOW = 140, offs = [];
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

  function onKey(e) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (matches.length) {
        sel = (sel + 1) % matches.length;
        markSel();
      }
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (matches.length) {
        sel = (sel - 1 + matches.length) % matches.length;
        markSel();
      }
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
  var flashTimer = 0, flashMark = null;
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
  function firstTermRange(startEl, terms) {
    var low = terms.filter(Boolean).map(function (t) { return t.toLowerCase(); });
    if (!low.length || !startEl) return null;
    var el = startEl;
    while (el) {
      var walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, null);
      var tn;
      while ((tn = walker.nextNode())) {
        var text = tn.nodeValue, tl = text.toLowerCase();
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

  function go(item) {
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
    var terms;
    try { terms = JSON.parse(raw); } catch (e) { return; }
    if (!Array.isArray(terms) || !terms.length) return;
    var id = decodeURIComponent((location.hash || "").replace(/^#/, ""));
    var target = id && document.getElementById(id);
    if (!target) return;
    // Let the browser settle on the anchor first, then flash.
    setTimeout(function () { flashTermsIn(target, terms); }, 60);
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
    if (e.target && e.target.closest && e.target.closest("[data-qmd-search]")) {
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
