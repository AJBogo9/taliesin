// @ts-check
// qmd-fast preview client.
//
// Connects to the dev server's websocket and applies a `full_render` followed
// by incremental `update`/`insert`/`remove` block ops. Unchanged blocks are
// never touched, so scroll position and the runtime state of live blocks
// (Three.js canvases, OJS cells) survive edits. Math is rendered server-side,
// so there is nothing to re-run on the client.
//
// ── Websocket protocol ──────────────────────────────────────────────────────
// The server is the producer; these typedefs are the consumer's view of the
// contract. The Rust producers (serve.rs / serve_site.rs `*_json`) are locked to
// these shapes by a contract test (`protocol_contract`); keep the two in sync.
// This file is `// @ts-check`ed (see web-client/jsconfig.json), so the shapes are
// enforced here too.
/**
 * @typedef {{ level: string, message: string }} Diagnostic
 * @typedef {{ type: "full_render", title: ?string, body_html: string, diagnostics: Diagnostic[] }} FullRenderMsg
 * @typedef {{ type: "diagnostics", messages: Diagnostic[] }} DiagnosticsMsg
 * @typedef {{ type: "update", target_id: string, html: string }} UpdateMsg
 * @typedef {{ type: "insert", after_id: ?string, html: string }} InsertMsg
 * @typedef {{ type: "remove", target_id: string }} RemoveMsg
 * @typedef {{ type: "error", message: string }} ErrorMsg
 * @typedef {{ type: "reload" }} ReloadMsg
 * @typedef {FullRenderMsg|DiagnosticsMsg|UpdateMsg|InsertMsg|RemoveMsg|ErrorMsg|ReloadMsg} ServerMessage
 */
(() => {
  const root = document.getElementById("qmd-root");
  if (!root) return; // the client mounts into #qmd-root; nothing to do without it
  let statusEl = /** @type {HTMLElement|null} */ (null);
  let wordCountEl = /** @type {HTMLElement|null} */ (null);
  let ws = /** @type {WebSocket|undefined} */ (undefined);

  const setStatus = (/** @type {string} */ s) => {
    const state =
      s === "live" ? "live" : s === "error" ? "error" : /reconnect/.test(s) ? "warn" : "wait";
    if (statusEl) {
      statusEl.textContent = s;
      statusEl.dataset.state = state;
    }
    // The collapsed dev-menu button shows this state as a colored dot.
    const dot = document.getElementById("qmd-dev-dot");
    if (dot) dot.dataset.state = state;
  };

  // Words + reading time (prose only: code and math are excluded), refreshed on
  // every change. Shown in the control bar; no-op in reveal mode / without it.
  const updateWordCount = () => {
    if (!wordCountEl) return;
    const clone = /** @type {Element} */ (root.cloneNode(true));
    clone.querySelectorAll("pre, .katex, .qmd-eqn-number").forEach((n) => n.remove());
    const words = ((clone.textContent || "").match(/[^\s]+/g) || []).length;
    const mins = Math.max(1, Math.round(words / 200));
    wordCountEl.textContent = `${words.toLocaleString()} words · ${mins} min`;
  };

  // --- diagnostics: render/include/kernel issues the server pushes -----------
  // A small bottom-left stack, shown only when there are issues, so the author
  // sees a broken include or a missing kernel without watching the terminal.
  // The diagnostics list lives inside the dev menu's panel (moved there once the
  // menu is built). Its style is part of the dev-menu CSS (STATUS_CSS).
  // Two issue sources, both shown in the dev panel and counted on the collapsed
  // pill: server diagnostics (include/kernel) in `diagEl`, and per-cell runtime
  // errors (Python `.qmd-error`, OJS `.observablehq--error`) in `cellErrEl`.
  const diagEl = document.createElement("div");
  diagEl.id = "qmd-diagnostics";
  diagEl.style.display = "none";
  const cellErrEl = document.createElement("div");
  cellErrEl.id = "qmd-cell-errors";
  cellErrEl.style.display = "none";
  let cellErrCount = 0;

  // Reflect the total issue count on the collapsed dev button (amber + a badge),
  // so problems are noticeable without expanding the panel.
  const refreshAlert = () => {
    const diagCount = diagEl.style.display === "none" ? 0 : diagEl.children.length;
    const total = diagCount + cellErrCount;
    const toggle = document.getElementById("qmd-dev-toggle");
    if (toggle) toggle.classList.toggle("qmd-dev-alert", total > 0);
    const badge = document.getElementById("qmd-dev-count");
    if (badge) {
      badge.textContent = total ? String(total) : "";
      badge.hidden = total === 0;
    }
  };

  const setDiagnostics = (/** @type {Diagnostic[]=} */ items) => {
    const list = (items || []).filter(Boolean);
    diagEl.textContent = "";
    diagEl.style.display = list.length ? "flex" : "none";
    for (const it of list) {
      const level = it.level === "error" ? "error" : "warning";
      const row = document.createElement("div");
      row.className = "qmd-diag qmd-diag-" + level;
      row.textContent = (level === "error" ? "✗ " : "⚠ ") + (it.message || it);
      diagEl.appendChild(row);
    }
    refreshAlert();
  };

  // Scan the mounted content for per-cell errors, list them in the panel (each a
  // button that scrolls to + flashes the failing cell), and update the pill badge.
  // Re-run after every mount and (via a MutationObserver) when async OJS errors land.
  const scanCellErrors = () => {
    const errs = root ? [...root.querySelectorAll(".qmd-error, .observablehq--error")] : [];
    cellErrCount = errs.length;
    cellErrEl.textContent = "";
    cellErrEl.style.display = errs.length ? "flex" : "none";
    errs.forEach((el, i) => {
      if (!el.id) el.id = "qmd-cellerr-" + i;
      const row = document.createElement("button");
      row.type = "button";
      row.className = "qmd-cellerr";
      row.textContent = "✗ " + (el.textContent || "cell error").trim().slice(0, 90);
      row.addEventListener("click", () => {
        el.scrollIntoView({ block: "center", behavior: "smooth" });
        pulse(/** @type {HTMLElement} */ (el), "qmd-hl-flash");
      });
      cellErrEl.appendChild(row);
    });
    refreshAlert();
  };

  // --- fatal-error overlay ---------------------------------------------------
  // A render/read failure leaves the last good content in place; this overlays it
  // so a broken save is impossible to miss (including on a phone). It clears on
  // the next successful render, or on click-outside / Escape.
  const errorEl = (() => {
    const style = document.createElement("style");
    style.textContent =
      "#qmd-error{position:fixed;inset:0;z-index:2147482500;display:none;flex-direction:column;" +
      "align-items:center;justify-content:center;padding:2rem;box-sizing:border-box;" +
      "background:rgba(10,12,16,.86);-webkit-backdrop-filter:blur(3px);backdrop-filter:blur(3px);}" +
      "#qmd-error.qmd-show{display:flex;}" +
      "#qmd-error .qmd-error-card{max-width:min(680px,92vw);width:100%;max-height:74vh;overflow:auto;" +
      "background:#1b1d23;border:1px solid #5a2a2a;border-left:4px solid #e5534b;border-radius:10px;" +
      "padding:1rem 1.2rem;box-shadow:0 14px 44px rgba(0,0,0,.55);}" +
      "#qmd-error .qmd-error-title{font:600 13px ui-sans-serif,system-ui,sans-serif;color:#ff8c82;margin-bottom:.55rem;}" +
      "#qmd-error pre{margin:0;padding:0;background:transparent;white-space:pre-wrap;word-break:break-word;" +
      "font:13px/1.5 ui-monospace,SFMono-Regular,Menlo,monospace;color:#f2d5d5;}" +
      "#qmd-error .qmd-error-hint{margin-top:.85rem;font:12px ui-sans-serif,system-ui,sans-serif;color:#9aa0aa;}";
    (document.head || document.documentElement).appendChild(style);
    const el = document.createElement("div");
    el.id = "qmd-error";
    el.innerHTML =
      '<div class="qmd-error-card"><div class="qmd-error-title">⚠ Render failed</div><pre></pre>' +
      '<div class="qmd-error-hint">Fix the source and save; this clears on the next successful render. (Esc to dismiss)</div></div>';
    document.body.appendChild(el);
    el.addEventListener("click", (e) => { if (e.target === el) el.classList.remove("qmd-show"); });
    return el;
  })();
  const showError = (/** @type {string=} */ message) => {
    const pre = errorEl.querySelector("pre");
    if (pre) pre.textContent = message || "Unknown error";
    errorEl.classList.add("qmd-show");
  };
  const hideError = () => errorEl.classList.remove("qmd-show");
  // A successful render arrived: drop the overlay and clear the "error" status.
  const renderOk = () => {
    hideError();
    if (statusEl && statusEl.textContent === "error") setStatus("live");
  };
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && errorEl.classList.contains("qmd-show")) hideError();
  });

  // Briefly pulse a block with a self-removing animated class: the change-flash on
  // re-render (`qmd-flash`), and the click-to-source highlight (`qmd-hl-flash`).
  const pulse = (/** @type {Element|null} */ el, /** @type {string} */ cls) => {
    if (!el || !el.classList) return;
    el.classList.remove(cls);
    void (/** @type {HTMLElement} */ (el)).offsetWidth; // restart the animation when re-pulsing the same node
    el.classList.add(cls);
    el.addEventListener("animationend", () => el.classList.remove(cls), { once: true });
  };

  // --- preview control bar: theme toggle + click-to-source toggle ----------
  const inWebview = window.parent !== window;
  // Click-to-source is an explicit "locate mode", default OFF: when on, clicks
  // reveal source (and links stop navigating so cards/nav are locatable too), so
  // it must default off or it would break browsing the preview. The choice persists.
  const CLICK_KEY = "qmd-click-source";
  let clickSource = (() => {
    try { return localStorage.getItem(CLICK_KEY) === "1"; } catch (e) { return false; }
  })();

  // One collapsed dev menu (Next.js-style): a corner button showing the live
  // status dot, expanding to a panel with the preview-only tools — live status,
  // word count, the click-to-source toggle, diagnostics, and (only when the page
  // chrome has no real theme toggle, i.e. single-doc preview) a theme toggle.
  // The site navbar's theme toggle is a real, shipped feature, not a dev tool.
  (function buildDevMenu() {
    const host = document.getElementById("qmd-controls");
    if (!host) return;
    host.classList.add("qmd-dev");

    const devRow = (/** @type {string} */ label, /** @type {HTMLElement} */ valueEl) => {
      const row = document.createElement("div");
      row.className = "qmd-dev-row";
      const l = document.createElement("span");
      l.className = "qmd-dev-label";
      l.textContent = label;
      row.append(l, valueEl);
      return row;
    };

    const toggle = document.createElement("button");
    toggle.id = "qmd-dev-toggle";
    toggle.className = "qmd-dev-toggle";
    toggle.type = "button";
    toggle.setAttribute("aria-label", "Developer tools");
    toggle.setAttribute("aria-expanded", "false");
    toggle.innerHTML =
      '<span class="qmd-dev-dot" id="qmd-dev-dot"></span><span class="qmd-dev-glyph">&lt;/&gt;</span>' +
      '<span class="qmd-dev-count" id="qmd-dev-count" hidden></span>';

    const panel = document.createElement("div");
    panel.id = "qmd-dev-panel";
    panel.className = "qmd-dev-panel";
    panel.hidden = true;

    toggle.addEventListener("click", (e) => {
      e.stopPropagation();
      panel.hidden = !panel.hidden;
      toggle.setAttribute("aria-expanded", panel.hidden ? "false" : "true");
    });
    document.addEventListener("click", (e) => {
      if (!panel.hidden && e.target instanceof Node && !host.contains(e.target)) {
        panel.hidden = true;
        toggle.setAttribute("aria-expanded", "false");
      }
    });

    statusEl = document.createElement("span");
    statusEl.id = "qmd-status";
    wordCountEl = document.createElement("span");
    wordCountEl.id = "qmd-wordcount";

    // Click-to-source on/off. When off, clicks pass through normally (so you can
    // select text / drive OJS widgets without jumping to source).
    const srcBtn = document.createElement("button");
    srcBtn.id = "qmd-src-ctl";
    srcBtn.className = "qmd-dev-ctl";
    srcBtn.type = "button";
    srcBtn.textContent = "Click-to-source";
    srcBtn.title =
      "Double-click any block to reveal its source" + (inWebview ? " in the editor" : " in your editor");
    const syncSrc = () => srcBtn.setAttribute("aria-pressed", clickSource ? "true" : "false");
    srcBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      clickSource = !clickSource;
      try { localStorage.setItem(CLICK_KEY, clickSource ? "1" : "0"); } catch (e) {}
      syncSrc();
    });
    syncSrc();

    // Restart the warm Jupyter kernel: drops the (possibly dead/wedged) kernel and
    // re-runs every cell against a fresh one. Recovers after fixing QMD_FAST_PYTHON.
    const kernelBtn = document.createElement("button");
    kernelBtn.id = "qmd-kernel-ctl";
    kernelBtn.className = "qmd-dev-ctl";
    kernelBtn.type = "button";
    kernelBtn.textContent = "Restart kernel";
    kernelBtn.title = "Drop the Jupyter kernel and re-run all cells against a fresh one";
    kernelBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: "restart_kernel" }));
        const prev = kernelBtn.textContent;
        kernelBtn.textContent = "Restarting…";
        kernelBtn.disabled = true;
        setTimeout(() => {
          kernelBtn.textContent = prev;
          kernelBtn.disabled = false;
        }, 1500);
      }
    });

    panel.append(devRow("Status", statusEl), devRow("Words", wordCountEl), srcBtn, kernelBtn);

    // Single-doc preview has no site navbar, so give the dev menu its own theme
    // toggle (wired by the shared theme_head). Sites use the navbar's instead.
    if (!document.querySelector("[data-qmd-theme-toggle]")) {
      const themeBtn = document.createElement("button");
      themeBtn.className = "qmd-dev-ctl qmd-dev-theme";
      themeBtn.type = "button";
      themeBtn.setAttribute("data-qmd-theme-toggle", "");
      panel.appendChild(themeBtn);
      if (window.qmdWireThemeToggles) window.qmdWireThemeToggles();
    }

    // Diagnostics + per-cell errors both live inside the panel.
    panel.append(diagEl, cellErrEl);
    host.append(toggle, panel);
    setStatus("connecting…");
  })();
  // Reveal mode (and any layout without the control bar) keeps its status pill.
  if (!statusEl) statusEl = document.getElementById("qmd-status");

  // OJS errors are rendered asynchronously by the Observable runtime, after the
  // mount; watch the content for them (debounced) so the dev-menu count stays live.
  if (window.MutationObserver) {
    let t = 0;
    new MutationObserver(() => {
      clearTimeout(t);
      t = setTimeout(scanCellErrors, 200);
    }).observe(root, { childList: true, subtree: true });
  }

  // Deck mode: the body is sectioned slides mounted into `.reveal > .slides`
  // (root). After any DOM change we (re)attach reveal.js — the first change
  // initializes, later ones only `sync()`, so the current slide and the
  // runtime state of live blocks survive edits.
  const isReveal = window.QMD_FORMAT === "reveal";
  let revealReady = false;
  const syncReveal = () => {
    if (!isReveal || !window.Reveal) return;
    if (!revealReady) {
      window.Reveal.initialize({ hash: true, slideNumber: "c/t", center: false });
      revealReady = true;
    } else {
      window.Reveal.sync();
      window.Reveal.layout();
    }
  };

  // TOC mode: rebuild `<nav id="TOC">` from the mounted, anchored headings after
  // every change, so the contents stay live as headings are edited/added/removed.
  const tocEl = window.QMD_TOC === true ? document.getElementById("TOC") : null;
  // Mobile pull-up sheet chrome (present only on the live TOC page).
  const tocHandle = tocEl && document.getElementById("qmd-toc-handle");
  const tocBackdrop = tocEl && document.getElementById("qmd-toc-backdrop");
  const escText = (/** @type {string|null} */ s) =>
    (s || "").replace(/[&<>]/g, (/** @type {string} */ c) =>
      /** @type {Record<string, string>} */ ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]);
  const buildToc = () => {
    if (!tocEl) return;
    const heads = [...root.querySelectorAll("h1[id], h2[id], h3[id]")];
    if (!heads.length) { tocEl.innerHTML = ""; return; }
    const lvl = (/** @type {Element} */ h) => +h.tagName[1];
    const base = Math.min(...heads.map(lvl));
    let html = "<ul>";
    let level = base;
    let openLi = false;
    for (const h of heads) {
      const l = Math.max(lvl(h), base);
      if (l > level) {
        while (level < l) { html += "<ul>"; level++; }
      } else {
        if (openLi) html += "</li>";
        while (level > l) { html += "</ul></li>"; level--; }
      }
      html += `<li><a href="#${h.id}">${escText(h.textContent)}</a>`;
      openLi = true;
    }
    if (openLi) html += "</li>";
    while (level > base) { html += "</ul></li>"; level--; }
    tocEl.innerHTML = html + "</ul>";
  };

  // TOC scrollspy lives in the shared toc-spy.js (window.qmdInitTocSpy) so the
  // live preview and the static build highlight the active section identically.
  // The client only re-inits it after rebuilding the nav (see mountAll) and feeds
  // it the mobile pull-up label flash on scroll.
  if (tocEl) window.qmdTocScrollHook = () => flashTocLabel();

  // Mobile pull-up TOC: drag the handle up (the sheet follows) or tap it to open;
  // tap the backdrop or a TOC entry to close. The current-section chip flashes in
  // while scrolling, then fades, so the resting handle stays quiet.
  let tocLabelTimer = 0;
  function flashTocLabel() {
    if (!tocHandle) return;
    tocHandle.classList.add("qmd-show-label");
    clearTimeout(tocLabelTimer);
    tocLabelTimer = setTimeout(() => tocHandle.classList.remove("qmd-show-label"), 1000);
  }
  if (tocHandle && tocEl && tocBackdrop) {
    const isSheetMode = () => !window.matchMedia || matchMedia("(max-width: 60rem)").matches;
    // #TOC doubles as the desktop sidebar, so only hide it from assistive tech and
    // pull it out of the tab order when it is an off-screen sheet (narrow + closed).
    const syncSheetA11y = () => {
      const open = document.body.classList.contains("qmd-toc-open");
      tocHandle.setAttribute("aria-expanded", open ? "true" : "false");
      if (isSheetMode() && !open) {
        tocEl.setAttribute("inert", ""); tocEl.setAttribute("aria-hidden", "true");
      } else {
        tocEl.removeAttribute("inert"); tocEl.removeAttribute("aria-hidden");
      }
    };
    const setOpen = (/** @type {boolean} */ open) => {
      document.body.classList.toggle("qmd-toc-open", open);
      syncSheetA11y();
      if (open) { const f = tocEl.querySelector("a"); if (f) f.focus(); } // focus into the sheet
    };
    const resetSheet = () => {
      tocEl.style.transition = ""; tocEl.style.transform = "";
      tocBackdrop.style.transition = ""; tocBackdrop.style.opacity = ""; tocBackdrop.style.pointerEvents = "";
    };
    let d = /** @type {{ y: number, t: number, moved: number, h: number }|null} */ (null);
    tocHandle.addEventListener("pointerdown", (e) => {
      d = { y: e.clientY, t: Date.now(), moved: 0, h: tocEl.offsetHeight || Math.round(innerHeight * 0.6) };
      try { tocHandle.setPointerCapture(e.pointerId); } catch (_) {}
    });
    tocHandle.addEventListener("pointermove", (e) => {
      if (!d) return;
      d.moved = d.y - e.clientY;                         // upward drag is positive
      const up = Math.max(0, Math.min(d.moved, d.h));
      tocEl.style.transition = "none";
      tocEl.style.transform = "translateY(calc(100% - " + up + "px))";
      tocBackdrop.style.transition = "none";
      tocBackdrop.style.opacity = (up / d.h * 0.42).toFixed(3);
      tocBackdrop.style.pointerEvents = up > 2 ? "auto" : "none";
    });
    const finish = () => {
      if (!d) return;
      const dt = Date.now() - d.t;
      const tap = d.moved < 6 && dt < 300;
      const open = tap || d.moved > d.h * 0.3 || (d.moved > 36 && d.moved / Math.max(dt, 1) > 0.45);
      resetSheet();
      setOpen(!!open);
      d = null;
    };
    tocHandle.addEventListener("pointerup", finish);
    tocHandle.addEventListener("pointercancel", finish);
    tocBackdrop.addEventListener("click", () => { setOpen(false); tocHandle.focus(); });
    tocEl.addEventListener("click", (e) => { if (e.target instanceof Element && e.target.closest("a")) setOpen(false); });

    // Drag the sheet DOWN to dismiss, but only when its list is scrolled to the
    // top (otherwise a downward swipe just scrolls the list). Touch events, not
    // pointer: native scroll won't deliver pointermove, so we take over the touch
    // stream with preventDefault instead.
    let sd = /** @type {{ y: number, t0: number, atTop: boolean, active: boolean, dy: number, h: number }|null} */ (null);
    tocEl.addEventListener("touchstart", (e) => {
      if (!document.body.classList.contains("qmd-toc-open")) { sd = null; return; }
      sd = { y: e.touches[0].clientY, t0: Date.now(), atTop: tocEl.scrollTop <= 0,
             active: false, dy: 0, h: tocEl.offsetHeight || Math.round(innerHeight * 0.6) };
    }, { passive: true });
    tocEl.addEventListener("touchmove", (e) => {
      if (!sd) return;
      const dy = e.touches[0].clientY - sd.y;            // downward is positive
      if (!sd.active) { if (sd.atTop && dy > 4) sd.active = true; else return; }
      e.preventDefault();                                // take over from native scroll
      sd.dy = Math.max(0, dy);
      tocEl.style.transition = "none";
      tocEl.style.transform = "translateY(" + sd.dy + "px)";
      tocBackdrop.style.transition = "none";
      tocBackdrop.style.opacity = (0.42 * Math.max(0, 1 - sd.dy / sd.h)).toFixed(3);
    }, { passive: false });
    const endSheetDrag = () => {
      if (!sd) return;
      const active = sd.active, dy = sd.dy, h = sd.h, dt = Date.now() - sd.t0;
      sd = null;
      if (!active) return;
      const close = dy > h * 0.28 || dy > 90 || (dy > 40 && dy / Math.max(dt, 1) > 0.45);
      resetSheet();
      setOpen(!close);
    };
    tocEl.addEventListener("touchend", endSheetDrag);
    tocEl.addEventListener("touchcancel", endSheetDrag);

    // keyboard: Enter/Space on the handle opens; Escape closes and returns focus.
    tocHandle.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setOpen(true); }
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && document.body.classList.contains("qmd-toc-open")) {
        setOpen(false); tocHandle.focus();
      }
    });
    window.addEventListener("resize", syncSheetA11y);
    syncSheetA11y();

    // teach the gesture once on a narrow screen
    if (isSheetMode()) {
      tocHandle.classList.add("qmd-hint");
      setTimeout(() => tocHandle.classList.remove("qmd-hint"), 2700);
      flashTocLabel();
    }
  }

  const cssEscape = (/** @type {string} */ s) =>
    window.CSS && CSS.escape ? CSS.escape(s) : s.replace(/["\\]/g, "\\$&");

  const elById = (/** @type {string} */ id) =>
    root.querySelector(`[data-block-id="${cssEscape(id)}"]`);

  const fragment = (/** @type {string} */ html) => {
    const t = document.createElement("template");
    t.innerHTML = html.trim();
    return t.content.firstElementChild;
  };

  // Apply a mutation while keeping the scroll position pinned. `instant` overrides
  // the page's smooth scroll-behavior so live re-renders never animate the restore.
  const keepScroll = (/** @type {() => void} */ fn) => {
    const y = window.scrollY;
    fn();
    window.scrollTo({ top: y, left: 0, behavior: "instant" });
  };

  // React to OJS content arriving/changing in a freshly-mounted op node.
  // - An `ojs-define` script (a {python} cell's output bridging values to OJS):
  //   bind it into the live module so the cells that reference it recompute. This
  //   is what fixes the cold-load race — the values arrive after the OJS cells
  //   first interpret — and also live-updates a figure when its Python inputs change.
  // - An `ojs-module-contents` script (an authored {ojs} cell whose source changed):
  //   the runtime can't redefine a whole cell in place, so reload to re-interpret.
  /** @param {Element|null} node */
  const afterOjsMutation = (node) => {
    if (!node || !window.__qmdOjsRan || !node.querySelector) return;
    if (node.querySelector('script[type="ojs-module-contents"]')) {
      location.reload();
      return;
    }
    if (node.querySelector('script[type="ojs-define"]') && window.qmdBindOjsDefines) {
      window.qmdBindOjsDefines(node);
    }
  };

  // Re-attach reveal, rebuild the TOC, and (re)highlight + add copy buttons to
  // code blocks after any DOM change (each is a no-op when not applicable).
  const afterChange = () => {
    syncReveal();
    buildToc();
    if (window.qmdInitTocSpy) window.qmdInitTocSpy(); // re-collect against the fresh nav
    updateWordCount();
    if (window.qmdEnhanceCode) window.qmdEnhanceCode(root);
    scanCellErrors();
  };

  // The server renders the initial body into the page (so content paints before
  // the websocket connects). The first `full_render` after that is identical, so
  // skip re-mounting it (avoids a flash + needless OJS/reveal re-init); reconnects
  // still re-mount normally.
  let ssrPending = window.QMD_SSR === true;

  /** @param {ServerMessage} msg */
  const handle = (msg) => {
    switch (msg.type) {
      case "full_render":
        renderOk(); // a fresh render arrived: any prior failure is resolved
        document.title = msg.title || "qmd-fast";
        if (ssrPending) {
          ssrPending = false; // content already server-rendered into #qmd-root
        } else {
          keepScroll(() => { root.innerHTML = msg.body_html; });
        }
        afterChange();
        setDiagnostics(msg.diagnostics);
        // Run Observable cells once the cells are in the DOM (no-op without OJS).
        if (window.qmdRunOJS) window.qmdRunOJS();
        break;
      case "diagnostics":
        setDiagnostics(msg.messages);
        break;
      case "update": {
        renderOk();
        const el = elById(msg.target_id);
        const node = fragment(msg.html);
        if (el && node) {
          keepScroll(() => el.replaceWith(node));
          pulse(node, "qmd-flash");
          afterOjsMutation(node);
        }
        afterChange();
        break;
      }
      case "insert": {
        renderOk();
        const node = fragment(msg.html);
        if (node) {
          keepScroll(() => {
            const after = msg.after_id && elById(msg.after_id);
            if (after) after.after(node);
            else root.prepend(node);
          });
          pulse(node, "qmd-flash");
          afterOjsMutation(node);
        }
        afterChange();
        break;
      }
      case "remove": {
        renderOk();
        const el = elById(msg.target_id);
        if (el) keepScroll(() => el.remove());
        afterChange();
        break;
      }
      case "error":
        setStatus("error");
        showError(msg.message);
        break;
      // Multi-page site: the project config (or a structural change) changed,
      // so the whole page is re-fetched rather than block-diffed.
      case "reload":
        location.reload();
        break;
    }
  };

  const connect = () => {
    // In a multi-page site the ws is scoped to the current page (QMD_WS_PATH);
    // a single-doc preview uses the plain "/ws".
    const wsPath = window.QMD_WS_PATH || "/ws";
    ws = new WebSocket(`ws://${location.host}${wsPath}`);
    ws.onopen = () => setStatus("live");
    ws.onmessage = (e) => handle(JSON.parse(e.data));
    ws.onclose = () => { setStatus("reconnecting…"); setTimeout(connect, 1000); };
    ws.onerror = () => ws?.close();
  };

  // Click-to-source: report the clicked block to the server (the editor client
  // will act on this in Phase 3). Also highlight it locally.
  const blockRef = (/** @type {HTMLElement} */ el) => ({
    block_id: el.dataset.blockId,
    source_file: el.dataset.sourceFile || null,
    sourcepos: el.dataset.sourcepos || null,
  });

  // The nearest locatable ancestor: a `data-qmd-src` element (cards, about block,
  // navbar/footer → an explicit source file) or a `data-block-id` block (the
  // page's own prose/headings/code). Whichever is closer wins.
  const locatable = (/** @type {Element} */ t) =>
    /** @type {HTMLElement|null} */ (t.closest("[data-qmd-src], [data-block-id]"));

  // Open the source for an element: an explicit `data-qmd-src` (site-root-relative,
  // `rel` or `rel:line`) wins; else the block's sourcepos on the current page (or an
  // included file). In the webview, relay to the host; in a browser, `vscode://`.
  const openSource = (/** @type {HTMLElement} */ el) => {
    const doc = window.QMD_DOC;
    if (!doc) return;
    const src = el.getAttribute("data-qmd-src");
    let abs, line = "1", col = "1";
    if (src && doc.root) {
      const i = src.indexOf(":");
      abs = doc.root.replace(/\/+$/, "") + "/" + (i >= 0 ? src.slice(0, i) : src);
      if (i >= 0) line = src.slice(i + 1);
    } else {
      const ref = blockRef(el);
      if (inWebview) {
        window.parent.postMessage({ type: "qmd-goto", ...ref }, "*");
        return;
      }
      abs = ref.source_file ? doc.baseDir.replace(/\/+$/, "") + "/" + ref.source_file : doc.path;
      const m = /^(\d+):(\d+)/.exec(ref.sourcepos || "");
      if (m) { line = m[1]; col = m[2]; }
    }
    if (inWebview) {
      window.parent.postMessage({ type: "qmd-goto", source_file: src, sourcepos: line + ":" + col }, "*");
      return;
    }
    window.location.href = "vscode://file" + encodeURI(abs) + ":" + line + ":" + col;
  };

  const inDevMenu = (/** @type {Element} */ t) => !!t.closest("#qmd-controls");

  // Single click in locate mode (click-to-source ON): pulse the target + report it,
  // and suppress link navigation so a card/link can be double-clicked to its source.
  document.addEventListener("click", (e) => {
    const t = e.target instanceof Element ? e.target : null;
    if (!clickSource || !t || inDevMenu(t)) return;
    if (t.closest("a")) e.preventDefault(); // locate mode: links don't navigate
    const el = locatable(t);
    if (!el) return;
    pulse(el, "qmd-hl-flash");
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "click_block", ...blockRef(el) }));
    }
  });

  // Double click in locate mode: jump to source (browser -> editor, webview -> host).
  document.addEventListener("dblclick", (e) => {
    if (!clickSource) return;
    const t = e.target instanceof Element ? e.target : null;
    if (!t || inDevMenu(t)) return;
    const el = locatable(t);
    if (el) { e.preventDefault(); openSource(el); }
  });

  // Reverse sync: highlight (and reveal/scroll to) the block under the editor
  // cursor. The matching block is the smallest one whose sourcepos range covers
  // `line` in the same source file, else the nearest block starting before it.
  const highlightAtLine = (/** @type {string|null} */ file, /** @type {number} */ line) => {
    const want = file || null;
    /** @type {HTMLElement|null} */ let contained = null;
    let containedSpan = Infinity;
    /** @type {HTMLElement|null} */ let preceding = null;
    let precedingStart = -1;
    for (const node of root.querySelectorAll("[data-sourcepos]")) {
      const el = /** @type {HTMLElement} */ (node);
      if ((el.dataset.sourceFile || null) !== want) continue;
      const m = /^(\d+):\d+-(\d+):\d+$/.exec(el.dataset.sourcepos || "");
      if (!m) continue;
      const start = +m[1], end = +m[2];
      if (line >= start && line <= end) {
        if (end - start < containedSpan) { contained = el; containedSpan = end - start; }
      } else if (start <= line && start > precedingStart) {
        preceding = el;
        precedingStart = start;
      }
    }
    const target = contained || preceding;
    if (!target) return;
    document.querySelectorAll(".qmd-hl").forEach((n) => n.classList.remove("qmd-hl"));
    target.classList.add("qmd-hl");
    if (isReveal && window.Reveal) {
      const sections = [...root.querySelectorAll(".slides > section")];
      const sec = target.closest(".slides > section");
      const i = sec ? sections.indexOf(sec) : -1;
      if (i >= 0) window.Reveal.slide(i);
    } else {
      const r = target.getBoundingClientRect();
      if (r.top < 0 || r.bottom > window.innerHeight) {
        target.scrollIntoView({ block: "center", behavior: "smooth" });
      }
    }
  };

  window.addEventListener("message", (e) => {
    const m = e.data;
    if (m && m.type === "qmd-cursor") highlightAtLine(m.file, m.line);
  });

  connect();
})();
