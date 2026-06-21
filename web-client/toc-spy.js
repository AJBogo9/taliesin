// Canonical TOC scrollspy: highlights the entry whose section currently sits
// just below the sticky navbar. Shared by two callers so the behaviour matches:
//   - the static build inlines this and lets it auto-init on load;
//   - the live preview rebuilds `#TOC` on every edit, then calls
//     window.qmdInitTocSpy() to re-collect against the fresh links.
// Inlined as one <script>; not part of the type-checked client.js bundle.
(function () {
  var raf = 0;
  var entries = []; // [{ link, heading }] in document order
  var active = null;
  var installed = false;

  // The activation line sits just under the sticky navbar so the highlighted
  // section matches what a clicked TOC link lands at (headings carry a
  // scroll-margin that clears the same navbar). Standalone pages have no navbar,
  // so the line falls back to a small top margin.
  function line() {
    var nav = document.querySelector(".qmd-site-nav");
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
    var cur = null;
    if (atBottom) {
      cur = entries[entries.length - 1];
    } else {
      for (var i = 0; i < entries.length; i++) {
        if (entries[i].heading.getBoundingClientRect().top - ln > 0) break;
        cur = entries[i];
      }
    }
    // cur stays null while above the first heading, so nothing is highlighted in
    // the intro (rather than prematurely lighting the first entry).
    if (cur === active) return;
    active = cur;
    entries.forEach(function (e) {
      e.link.classList.toggle("qmd-toc-active", e === cur);
    });
    // Collapse: expand only the active entry's branch (its <li> and ancestors), so
    // a long TOC shows top-level entries plus the current section's subsections.
    var open = [];
    for (var node = cur && cur.link.parentNode; node && node.id !== "TOC"; node = node.parentNode) {
      if (node.tagName === "LI") open.push(node);
    }
    Array.prototype.forEach.call(toc.getElementsByTagName("li"), function (li) {
      li.classList.toggle("qmd-toc-expanded", open.indexOf(li) !== -1);
    });
    var chip = document.getElementById("qmd-toc-cur"); // mobile pull-up handle label
    if (chip && cur) chip.textContent = cur.heading.textContent;
    if (cur) {
      // keep the active link in view when the TOC is its own scroll area
      var lr = cur.link.getBoundingClientRect();
      var tr = toc.getBoundingClientRect();
      if (lr.top < tr.top) toc.scrollTop -= tr.top - lr.top + 8;
      else if (lr.bottom > tr.bottom) toc.scrollTop += lr.bottom - tr.bottom + 8;
    }
  }

  function onScroll() {
    if (window.qmdTocScrollHook) window.qmdTocScrollHook(); // preview's mobile-label flash
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

  window.qmdInitTocSpy = init;
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
