// @ts-check
// Mobile pull-up "on this page" TOC sheet for STATIC builds (sites + books).
//
// The live preview drives its own copy of this from client.js (it also rebuilds the
// TOC live). A static export has no websocket client, so this self-contained enhancer
// wires the same bottom-sheet interaction against the server-rendered `#TOC`:
//   drag the handle up (or tap it) to open; tap the backdrop / a TOC entry / Esc, or
//   drag the open sheet down, to close.
// It only does anything at the sheet breakpoint (<= 60rem) where the CSS reveals the
// handle; on desktop the `#TOC` stays the sticky sidebar and this stays inert.
// Registered on TOC pages by `render::toc_scripts()` (build-only).
(function () {
  "use strict";
  /** @param {string} id */
  var byId = function (id) { return document.getElementById(id); };

  function init() {
    var toc = byId("TOC");
    var handle = byId("qmd-toc-handle");
    var backdrop = byId("qmd-toc-backdrop");
    var cur = byId("qmd-toc-cur");
    // Nothing to wire without the sheet chrome (e.g. a page with no TOC).
    if (!toc || !handle || !backdrop) return;
    // Opt the body into the sheet ONLY now that JS is running (progressive enhancement):
    // the server ships the chrome hidden + leaves the body in its in-flow layout, so with
    // JS off the TOC never ends up off-screen and unreachable.
    document.body.classList.add("qmd-toc-sheet");

    var isSheetMode = function () {
      return !window.matchMedia || matchMedia("(max-width: 60rem)").matches;
    };
    // `#TOC` doubles as the desktop sidebar, so only hide it from assistive tech and
    // pull it from the tab order when it is an off-screen sheet (narrow + closed).
    var syncA11y = function () {
      var open = document.body.classList.contains("qmd-toc-open");
      handle.setAttribute("aria-expanded", open ? "true" : "false");
      if (isSheetMode() && !open) {
        toc.setAttribute("inert", ""); toc.setAttribute("aria-hidden", "true");
      } else {
        toc.removeAttribute("inert"); toc.removeAttribute("aria-hidden");
      }
    };
    /** @param {boolean} open */
    var setOpen = function (open) {
      document.body.classList.toggle("qmd-toc-open", open);
      syncA11y();
      if (open) { var f = toc.querySelector("a"); if (f) f.focus(); }
    };
    var resetSheet = function () {
      toc.style.transition = ""; toc.style.transform = "";
      backdrop.style.transition = ""; backdrop.style.opacity = ""; backdrop.style.pointerEvents = "";
    };

    // Drag the handle up (the sheet follows) or tap it to open.
    var d = null;
    handle.addEventListener("pointerdown", function (e) {
      d = { y: e.clientY, t: Date.now(), moved: 0, h: toc.offsetHeight || Math.round(innerHeight * 0.6) };
      try { handle.setPointerCapture(e.pointerId); } catch (_) {}
    });
    handle.addEventListener("pointermove", function (e) {
      if (!d) return;
      d.moved = d.y - e.clientY;                 // upward drag is positive
      var up = Math.max(0, Math.min(d.moved, d.h));
      toc.style.transition = "none";
      toc.style.transform = "translateY(calc(100% - " + up + "px))";
      backdrop.style.transition = "none";
      backdrop.style.opacity = (up / d.h * 0.42).toFixed(3);
      backdrop.style.pointerEvents = up > 2 ? "auto" : "none";
    });
    var finish = function () {
      if (!d) return;
      var dt = Date.now() - d.t;
      var tap = d.moved < 6 && dt < 300;
      var open = tap || d.moved > d.h * 0.3 || (d.moved > 36 && d.moved / Math.max(dt, 1) > 0.45);
      resetSheet();
      setOpen(!!open);
      d = null;
    };
    handle.addEventListener("pointerup", finish);
    handle.addEventListener("pointercancel", finish);
    backdrop.addEventListener("click", function () { setOpen(false); handle.focus(); });
    toc.addEventListener("click", function (e) {
      if (e.target instanceof Element && e.target.closest("a")) setOpen(false);
    });

    // Drag the open sheet DOWN to dismiss, but only when its list is scrolled to the
    // top (otherwise a downward swipe just scrolls the list). Touch events, not
    // pointer: native scroll won't deliver pointermove, so we take over with preventDefault.
    var sd = null;
    toc.addEventListener("touchstart", function (e) {
      if (!document.body.classList.contains("qmd-toc-open")) { sd = null; return; }
      sd = { y: e.touches[0].clientY, t0: Date.now(), atTop: toc.scrollTop <= 0,
             active: false, dy: 0, h: toc.offsetHeight || Math.round(innerHeight * 0.6) };
    }, { passive: true });
    toc.addEventListener("touchmove", function (e) {
      if (!sd) return;
      var dy = e.touches[0].clientY - sd.y;      // downward is positive
      if (!sd.active) { if (sd.atTop && dy > 4) sd.active = true; else return; }
      e.preventDefault();
      sd.dy = Math.max(0, dy);
      toc.style.transition = "none";
      toc.style.transform = "translateY(" + sd.dy + "px)";
      backdrop.style.transition = "none";
      backdrop.style.opacity = (0.42 * Math.max(0, 1 - sd.dy / sd.h)).toFixed(3);
    }, { passive: false });
    var endDrag = function () {
      if (!sd) return;
      var active = sd.active, dy = sd.dy, h = sd.h, dt = Date.now() - sd.t0;
      sd = null;
      if (!active) return;
      var close = dy > h * 0.28 || dy > 90 || (dy > 40 && dy / Math.max(dt, 1) > 0.45);
      resetSheet();
      setOpen(!close);
    };
    toc.addEventListener("touchend", endDrag);
    toc.addEventListener("touchcancel", endDrag);

    // Keyboard: Enter/Space opens the handle; Escape closes and returns focus.
    handle.addEventListener("keydown", function (e) {
      if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setOpen(true); }
    });
    document.addEventListener("keydown", function (e) {
      if (e.key === "Escape" && document.body.classList.contains("qmd-toc-open")) {
        setOpen(false); handle.focus();
      }
    });
    window.addEventListener("resize", syncA11y);

    // The current-section chip flashes in while scrolling, then fades, so the resting
    // handle stays quiet. Source the label from whatever toc-spy.js marks active.
    var labelTimer = 0;
    var flash = function () {
      if (!isSheetMode() || document.body.classList.contains("qmd-toc-open")) return;
      var active = toc.querySelector("a.qmd-toc-active");
      if (cur && active) cur.textContent = active.textContent;
      handle.classList.add("qmd-show-label");
      clearTimeout(labelTimer);
      labelTimer = setTimeout(function () { handle.classList.remove("qmd-show-label"); }, 1000);
    };
    window.addEventListener("scroll", flash, { passive: true });

    syncA11y();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
