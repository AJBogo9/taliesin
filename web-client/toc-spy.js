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
  // Read-state: sections the reader has scrolled through, decorated in the TOC.
  // Reader-side + read-only: the set lives in the reader's OWN localStorage, keyed by
  // path and anchored to each heading's stable `data-block-id` (the same anchor that
  // reading-progress + resume use), so it survives reflow and never touches source.
  /** @type {Record<string, number>} read[headingBlockId] = 1 once scrolled through */
  var read = {};
  var readHigh = 0; // forward-only high-water index of scrolled-through entries
  var READ_KEY = "qmd-read:" + location.pathname;

  function loadRead() {
    read = {};
    try {
      var raw = localStorage.getItem(READ_KEY);
      var arr = raw && JSON.parse(raw);
      if (arr && arr.length) for (var i = 0; i < arr.length; i++) read[arr[i]] = 1;
    } catch (e) {}
  }
  function saveRead() {
    try {
      localStorage.setItem(READ_KEY, JSON.stringify(Object.keys(read)));
    } catch (e) {}
  }
  // Decorate a TOC link as read (idempotent): the class drives the ✓ + fade, and a
  // visually-hidden label announces "read" to a screen reader.
  /** @param {Element} link */
  function markRead(link) {
    if (!link || link.classList.contains("tali-toc-read")) return;
    link.classList.add("tali-toc-read");
    var vh = document.createElement("span");
    vh.className = "tali-sr-only";
    vh.textContent = " (read)";
    link.appendChild(vh);
  }
  // Restore a returning reader's trail: mark every entry whose heading is already read,
  // regardless of current scroll position (the high-water loop only reaches the current
  // section). Called on (re)init against the fresh links.
  function applySeededRead() {
    entries.forEach(function (e) {
      var bid = e.heading.getAttribute("data-block-id");
      if (bid && read[bid]) markRead(e.link);
    });
  }

  // The activation line sits just under the sticky navbar so the highlighted
  // section matches what a clicked TOC link lands at (headings carry a
  // scroll-margin that clears the same navbar). Standalone pages have no navbar,
  // so the line falls back to a small top margin.
  function line() {
    var nav = document.querySelector(".tali-site-nav");
    return (nav ? nav.getBoundingClientRect().height : 0) + 16;
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
        if (entries[i].heading.getBoundingClientRect().top - ln > 0) break;
        cur = entries[i];
      }
    }
    // Read-state advance: every entry strictly before the current section has been
    // scrolled through; the final entry counts once the page bottom is reached. The
    // high-water index only moves forward, so scrolling back up never un-marks a
    // section. (A forward jump — TOC click / resume — counts the skipped sections as
    // read, matching the position-anchored resume/progress model.)
    var reached = atBottom ? entries.length : cur ? entries.indexOf(cur) : 0;
    if (reached > readHigh) {
      var changed = false;
      for (; readHigh < reached; readHigh++) {
        var e = entries[readHigh];
        var bid = e.heading.getAttribute("data-block-id");
        if (bid && !read[bid]) {
          read[bid] = 1;
          changed = true;
        }
        markRead(e.link);
      }
      if (changed) saveRead();
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
    readHigh = 0; // fresh links on a live-preview rebuild: re-seed from storage
    loadRead();
    applySeededRead();
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
