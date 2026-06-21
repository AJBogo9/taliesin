// Client-side command palette (Cmd/Ctrl-K): full-text search to jump around a
// long document — the book, a paper, any page with a table of contents. Matches
// both headings and the body text of each section, and shows a snippet around the
// hit. A single doc builds its index from the live DOM on open; a site/book lazy-
// loads the cross-page index (search.json) on first open via window.QMD_SEARCH_URL,
// so the full-text index never bloats every page. Self-contained: injects its own
// themed overlay CSS and rides along as one <script> beside the TOC scrollspy. Not
// part of the type-checked client.js bundle.
(function () {
  if (window.qmdSearchInstalled) return;
  window.qmdSearchInstalled = true;

  var CSS =
    "#qmd-search{position:fixed;inset:0;z-index:10050;display:flex;justify-content:center;" +
    "align-items:flex-start;padding-top:12vh}" +
    "#qmd-search[hidden]{display:none}" +
    "#qmd-search .qmd-s-backdrop{position:absolute;inset:0;background:rgba(0,0,0,.45);" +
    "backdrop-filter:blur(2px)}" +
    "#qmd-search .qmd-s-box{position:relative;width:min(38rem,92vw);max-height:70vh;display:flex;" +
    "flex-direction:column;background:var(--qmd-bg,#fff);color:var(--qmd-fg,#111);" +
    "border:1px solid var(--qmd-border,#e0e0e0);border-radius:12px;" +
    "box-shadow:0 18px 60px rgba(0,0,0,.4);overflow:hidden}" +
    "#qmd-search .qmd-s-input{width:100%;box-sizing:border-box;border:0;outline:0;" +
    "padding:.95rem 1.1rem;font-size:1.05rem;background:transparent;color:inherit;" +
    "border-bottom:1px solid var(--qmd-border,#e0e0e0)}" +
    "#qmd-search .qmd-s-results{list-style:none;margin:0;padding:.3rem;overflow:auto;flex:1}" +
    "#qmd-search .qmd-s-item{display:flex;flex-direction:column;gap:.15rem;padding:.5rem .7rem;" +
    "border-radius:7px;cursor:pointer;scroll-margin:.4rem}" +
    "#qmd-search .qmd-s-head{display:flex;align-items:baseline;gap:.6rem}" +
    "#qmd-search .qmd-s-item[aria-selected=true]{background:var(--qmd-accent,#4c8dff);color:#fff}" +
    "#qmd-search .qmd-s-item[aria-selected=true] .qmd-s-sec{color:rgba(255,255,255,.8)}" +
    "#qmd-search .qmd-s-snip{font-size:.78rem;color:var(--qmd-muted,#888);overflow:hidden;" +
    "text-overflow:ellipsis;white-space:nowrap}" +
    "#qmd-search .qmd-s-snip mark{background:transparent;color:var(--qmd-accent,#4c8dff);font-weight:700;padding:0}" +
    "#qmd-search .qmd-s-item[aria-selected=true] .qmd-s-snip{color:rgba(255,255,255,.85)}" +
    "#qmd-search .qmd-s-item[aria-selected=true] .qmd-s-snip mark{color:#fff}" +
    "#qmd-search .qmd-s-title{font-weight:600}" +
    "#qmd-search .qmd-s-title mark{background:transparent;color:var(--qmd-accent,#4c8dff);" +
    "font-weight:800;padding:0}" +
    "#qmd-search .qmd-s-item[aria-selected=true] .qmd-s-title mark{color:#fff;" +
    "text-decoration:underline}" +
    "#qmd-search .qmd-s-sec{font-size:.8rem;color:var(--qmd-muted,#888);white-space:nowrap;margin-left:auto}" +
    "#qmd-search .qmd-s-empty{padding:1rem 1.1rem;color:var(--qmd-muted,#888)}" +
    "#qmd-search .qmd-s-hint{display:flex;gap:1rem;padding:.45rem .9rem;font-size:.72rem;" +
    "color:var(--qmd-muted,#888);border-top:1px solid var(--qmd-border,#e0e0e0)}" +
    "#qmd-search .qmd-s-hint kbd{font:inherit;border:1px solid var(--qmd-border,#e0e0e0);" +
    "border-radius:4px;padding:0 .3rem}";

  function injectCss() {
    if (document.getElementById("qmd-search-css")) return;
    var s = document.createElement("style");
    s.id = "qmd-search-css";
    s.textContent = CSS;
    document.head.appendChild(s);
  }

  var overlay, input, list;
  var index = [];
  var matches = [];
  var sel = 0;

  // Build the index: every anchored heading, plus the lowercased text of the
  // blocks that follow it until the next heading (so body keywords match too).
  function buildIndex() {
    // Site/book: search the whole project from the inlined cross-page index
    // (every page's title + anchored headings). A result carries its page url so
    // selecting it can navigate across chapters.
    if (window.QMD_SEARCH_INDEX) {
      return window.QMD_SEARCH_INDEX.map(function (e) {
        return { id: e.i, title: e.t, level: e.l, body: e.b || "", url: e.u, page: e.p };
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
      out.push({
        id: h.id,
        title: title,
        level: parseInt(h.tagName.charAt(1), 10) || 1,
        body: sectionText(h, heads[i + 1]),
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

  // Lazy-load the cross-page index from `search.json` on first open (a site/book
  // links to it via QMD_SEARCH_URL instead of inlining it into every page), then
  // run `cb`. A single doc (no URL) just runs `cb` against the DOM index.
  var indexFetched = false;
  function loadIndexThen(cb) {
    if (window.QMD_SEARCH_INDEX || !window.QMD_SEARCH_URL || indexFetched) {
      cb();
      return;
    }
    indexFetched = true;
    fetch(window.QMD_SEARCH_URL)
      .then(function (r) {
        return r.json();
      })
      .then(function (data) {
        window.QMD_SEARCH_INDEX = data;
        cb();
      })
      .catch(function () {
        cb();
      });
  }

  function ensureUi() {
    injectCss();
    if (overlay) return;
    overlay = document.createElement("div");
    overlay.id = "qmd-search";
    overlay.hidden = true;
    overlay.innerHTML =
      '<div class="qmd-s-backdrop"></div>' +
      '<div class="qmd-s-box" role="combobox" aria-expanded="true" aria-haspopup="listbox">' +
      '<input class="qmd-s-input" type="text" autocomplete="off" spellcheck="false" ' +
      'placeholder="Search this document…" aria-label="Search this document" ' +
      'aria-controls="qmd-s-results" />' +
      '<ul class="qmd-s-results" id="qmd-s-results" role="listbox"></ul>' +
      '<div class="qmd-s-hint"><span><kbd>↑</kbd><kbd>↓</kbd> navigate</span>' +
      "<span><kbd>↵</kbd> go to</span><span><kbd>esc</kbd> close</span></div>";
    document.body.appendChild(overlay);
    input = overlay.querySelector(".qmd-s-input");
    list = overlay.querySelector(".qmd-s-results");
    overlay.querySelector(".qmd-s-backdrop").addEventListener("click", close);
    input.addEventListener("input", function () {
      render(input.value);
    });
    input.addEventListener("keydown", onKey);
  }

  function open() {
    ensureUi();
    var isSite = !!(window.QMD_SEARCH_URL || window.QMD_SEARCH_INDEX);
    // Single doc with no headings: nothing to search.
    if (!isSite && !buildIndex().length) return;
    overlay.hidden = false;
    input.value = "";
    input.placeholder = isSite ? "Search…" : "Search this document…";
    // While the cross-page index is fetching on first open, show a loading row.
    list.innerHTML = "";
    if (isSite && !window.QMD_SEARCH_INDEX) {
      var li = document.createElement("li");
      li.className = "qmd-s-empty";
      li.textContent = "Loading…";
      list.appendChild(li);
    }
    loadIndexThen(function () {
      index = buildIndex();
      render(input.value);
    });
    input.focus();
  }

  function close() {
    if (overlay) overlay.hidden = true;
  }

  function isOpen() {
    return overlay && !overlay.hidden;
  }

  function score(item, q) {
    var t = item.title.toLowerCase();
    var pos = t.indexOf(q);
    if (pos === 0) return 3; // title prefix
    if (pos > 0) return 2; // title contains
    if (item.body.toLowerCase().indexOf(q) >= 0) return 1; // body contains
    return 0;
  }

  function render(query) {
    var q = query.trim().toLowerCase();
    if (!q) {
      // No query: a book shows its chapter list (the level-0 page entries) as a
      // jump menu; a single doc shows its full heading outline.
      matches = window.QMD_SEARCH_INDEX
        ? index.filter(function (it) { return it.level === 0; })
        : index.slice();
    } else {
      matches = index
        .map(function (it) { return { it: it, s: score(it, q) }; })
        .filter(function (m) { return m.s > 0; })
        .sort(function (a, b) { return b.s - a.s; })
        .map(function (m) { return m.it; });
    }
    sel = 0;
    list.innerHTML = "";
    if (!matches.length) {
      var empty = document.createElement("li");
      empty.className = "qmd-s-empty";
      empty.textContent = "No matches";
      list.appendChild(empty);
      return;
    }
    matches.forEach(function (m, i) { list.appendChild(itemEl(m, q, i)); });
    markSel();
  }

  function itemEl(item, q, i) {
    var li = document.createElement("li");
    li.className = "qmd-s-item";
    li.setAttribute("role", "option");
    li.id = "qmd-s-opt-" + i;
    var head = document.createElement("div");
    head.className = "qmd-s-head";
    var title = document.createElement("span");
    title.className = "qmd-s-title";
    highlight(title, item.title, q);
    var sec = document.createElement("span");
    sec.className = "qmd-s-sec";
    // In a book, label the result with its chapter; otherwise its heading level.
    sec.textContent = item.page || "H" + item.level;
    head.append(title, sec);
    li.appendChild(head);
    // A body (full-text) match that isn't in the title gets a snippet around the hit.
    if (q && item.title.toLowerCase().indexOf(q) < 0 && item.body) {
      var snip = document.createElement("div");
      snip.className = "qmd-s-snip";
      snippet(snip, item.body, q);
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

  // A one-line excerpt of body text around the first match of q, q highlighted.
  function snippet(el, body, q) {
    var pos = body.toLowerCase().indexOf(q);
    if (pos < 0) {
      el.textContent = body.slice(0, 120);
      return;
    }
    var start = Math.max(0, pos - 30);
    var tailEnd = pos + q.length + 90;
    el.appendChild(
      document.createTextNode((start > 0 ? "… " : "") + body.slice(start, pos)),
    );
    var m = document.createElement("mark");
    m.textContent = body.slice(pos, pos + q.length);
    el.appendChild(m);
    el.appendChild(
      document.createTextNode(
        body.slice(pos + q.length, tailEnd) + (body.length > tailEnd ? " …" : ""),
      ),
    );
  }

  // Render the title with the matched substring wrapped in <mark>.
  function highlight(el, title, q) {
    var pos = q ? title.toLowerCase().indexOf(q) : -1;
    if (pos < 0) {
      el.textContent = title;
      return;
    }
    el.appendChild(document.createTextNode(title.slice(0, pos)));
    var m = document.createElement("mark");
    m.textContent = title.slice(pos, pos + q.length);
    el.appendChild(m);
    el.appendChild(document.createTextNode(title.slice(pos + q.length)));
  }

  function markSel() {
    list.querySelectorAll(".qmd-s-item").forEach(function (opt, i) {
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

  function go(item) {
    close();
    // A result on another page navigates there (a real page load, anchored to the
    // heading); on this page — or in a single doc — it scrolls in place.
    if (item.url != null && item.url !== window.QMD_PAGE_URL) {
      window.location.href =
        (window.QMD_SITE_ROOT || "") + item.url + (item.id ? "#" + item.id : "");
      return;
    }
    if (!item.id) {
      window.scrollTo({ top: 0, behavior: "smooth" });
      return;
    }
    var target = document.getElementById(item.id);
    if (!target) return;
    if (history.replaceState) history.replaceState(null, "", "#" + item.id);
    target.scrollIntoView({ behavior: "smooth", block: "start" });
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
})();
