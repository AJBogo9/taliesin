// @ts-check
// Canonical TOC scrollspy: highlights the entry whose section currently sits
// just below the sticky navbar. Shared by two callers so the behaviour matches:
//   - the static build inlines this and lets it auto-init on load;
//   - the live preview rebuilds `#TOC` on every edit, then calls
//     window.taliInitTocSpy() to re-collect against the fresh links.
// Inlined as one <script>; not concatenated into the client.js bundle, but
// type-checked separately (web-client/jsconfig.json).
(function () {
  /** @typedef {{ link: Element, heading: HTMLElement }} TocEntry */
  var raf = 0;
  /** @type {TocEntry[]} in document order */
  var entries = [];
  /** @type {TocEntry | null} */
  var active = null;
  var installed = false;

  // The activation line sits exactly where a clicked TOC link lands, which the browser
  // decides from the heading's own `scroll-margin-top`. Read that, rather than measuring
  // a navbar: `.tali-site-nav` is the WEBSITE chrome and a book emits `.tali-book-topbar`
  // instead, so querying the navbar returned 0 on every book page and the highlight
  // lagged a whole section behind the reader. `scroll-margin-top` is already
  // `--tali-nav-h + 1rem` under both chromes, and 1rem on a standalone page.
  var lineOffset = 16;
  // Sampled here, once per (re)init, not in `update()` — that runs every rAF while
  // scrolling and `getComputedStyle` forces a style flush.
  function sampleLine() {
    var first = entries.length ? entries[0].heading : null;
    var px = first ? parseFloat(getComputedStyle(first).scrollMarginTop) : NaN;
    lineOffset = isNaN(px) ? 16 : px;
  }
  function line() {
    return lineOffset;
  }

  function collect() {
    var toc = document.getElementById("TOC");
    entries = [];
    if (!toc) return;
    toc.querySelectorAll("a[href^='#']").forEach(function (link) {
      var id = decodeURIComponent((link.getAttribute("href") || "").slice(1));
      var h = id && document.getElementById(id);
      if (h) entries.push({ link: link, heading: h });
    });
    sampleLine();
  }

  function update() {
    var toc = document.getElementById("TOC");
    if (!toc || !entries.length) return;
    var ln = line();
    var doc = document.documentElement;
    // Within one viewport of the bottom the last heading can never reach the
    // line, so pin the final entry — otherwise the last section never lights up.
    var atBottom = window.innerHeight + window.scrollY >= doc.scrollHeight - 2;
    /** @type {TocEntry | null} */
    var cur = null;
    if (atBottom) {
      cur = entries[entries.length - 1];
    } else {
      for (var i = 0; i < entries.length; i++) {
        // 1px tolerance: scroll offsets are quantized to device pixels, so a heading
        // the reader has just landed on (via a TOC click, a deep link, or resume) can
        // measure a hair BELOW the line and leave the previous entry highlighted —
        // the same visible off-by-one the activation line itself was causing.
        if (entries[i].heading.getBoundingClientRect().top - ln > 1) break;
        cur = entries[i];
      }
    }
    // cur stays null while above the first heading, so nothing is highlighted in
    // the intro (rather than prematurely lighting the first entry).
    if (cur === active) return;
    active = cur;
    entries.forEach(function (e) {
      e.link.classList.toggle("tali-toc-active", e === cur);
    });
    // Collapse: expand only the active entry's branch (its <li> and ancestors), so
    // a long TOC shows top-level entries plus the current section's subsections.
    /** @type {Element[]} */
    var open = [];
    // Walk element ancestors (the TOC links only ever nest inside element <li>/<ul>,
    // so `parentElement` is equivalent to `parentNode` here and types cleanly).
    for (var node = cur && cur.link.parentElement; node && node.id !== "TOC"; node = node.parentElement) {
      if (node.tagName === "LI") open.push(node);
    }
    Array.prototype.forEach.call(toc.getElementsByTagName("li"), function (li) {
      li.classList.toggle("tali-toc-expanded", open.indexOf(li) !== -1);
    });
    var chip = document.getElementById("tali-toc-cur"); // mobile pull-up handle label
    if (chip && cur) {
      // Strip the hover `#` permalink the anchor-links enhancer appends to a heading,
      // so the chip reads "Section title", not "Section title#".
      var h = /** @type {HTMLElement} */ (cur.heading.cloneNode(true));
      var anchors = h.querySelectorAll(".tali-anchor");
      for (var ai = 0; ai < anchors.length; ai++) anchors[ai].remove();
      chip.textContent = (h.textContent || "").trim();
    }
    if (cur) {
      // keep the active link in view when the TOC is its own scroll area
      var lr = cur.link.getBoundingClientRect();
      var tr = toc.getBoundingClientRect();
      if (lr.top < tr.top) toc.scrollTop -= tr.top - lr.top + 8;
      else if (lr.bottom > tr.bottom) toc.scrollTop += lr.bottom - tr.bottom + 8;
    }
  }

  function onScroll() {
    if (window.taliTocScrollHook) window.taliTocScrollHook(); // preview's mobile-label flash
    if (raf) return;
    raf = requestAnimationFrame(function () {
      raf = 0;
      update();
    });
  }

  function init() {
    if (!document.getElementById("TOC")) return; // no TOC on this page
    collect();
    active = null;
    if (!installed) {
      window.addEventListener("scroll", onScroll, { passive: true });
      window.addEventListener("resize", onScroll);
      installed = true;
    }
    update();
  }

  window.taliInitTocSpy = init;
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
