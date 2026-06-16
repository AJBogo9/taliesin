// qmd-fast preview client.
//
// Connects to the dev server's websocket and applies a `full_render` followed
// by incremental `update`/`insert`/`remove` block ops. Unchanged blocks are
// never touched, so scroll position and the runtime state of live blocks
// (Three.js canvases, OJS cells) survive edits. Math is rendered server-side,
// so there is nothing to re-run on the client.
(() => {
  const root = document.getElementById("qmd-root");
  let statusEl = null;
  let ws;

  const setStatus = (s) => { if (statusEl) statusEl.textContent = s; };

  // --- diagnostics: render/include/kernel issues the server pushes -----------
  // A small bottom-left stack, shown only when there are issues, so the author
  // sees a broken include or a missing kernel without watching the terminal.
  const diagEl = (() => {
    const style = document.createElement("style");
    style.textContent =
      "#qmd-diagnostics{position:fixed;bottom:2.3rem;left:.5rem;z-index:9998;max-width:min(560px,92vw);" +
      "display:none;flex-direction:column;gap:.3rem;font:12px ui-sans-serif,system-ui,sans-serif;}" +
      "#qmd-diagnostics .qmd-diag{padding:.3rem .55rem;border-radius:6px;background:var(--qmd-bg,#fff);" +
      "color:var(--qmd-fg,#111);border:1px solid var(--qmd-border,#e0e0e0);box-shadow:0 2px 12px rgba(0,0,0,.18);}" +
      "#qmd-diagnostics .qmd-diag-error{border-left:3px solid #e5534b;}" +
      "#qmd-diagnostics .qmd-diag-warning{border-left:3px solid #d9a23a;}";
    (document.head || document.documentElement).appendChild(style);
    const el = document.createElement("div");
    el.id = "qmd-diagnostics";
    document.body.appendChild(el);
    return el;
  })();
  const setDiagnostics = (items) => {
    const list = (items || []).filter(Boolean);
    diagEl.textContent = "";
    if (!list.length) { diagEl.style.display = "none"; return; }
    for (const it of list) {
      const level = it.level === "error" ? "error" : "warning";
      const row = document.createElement("div");
      row.className = "qmd-diag qmd-diag-" + level;
      row.textContent = (level === "error" ? "✗ " : "⚠ ") + (it.message || it);
      diagEl.appendChild(row);
    }
    diagEl.style.display = "flex";
  };

  // --- preview control bar: theme toggle + click-to-source toggle ----------
  const inWebview = window.parent !== window;
  const CLICK_KEY = "qmd-click-source";
  let clickSource = (() => {
    try { return localStorage.getItem(CLICK_KEY) !== "0"; } catch (e) { return true; }
  })();

  (function buildControls() {
    const bar = document.getElementById("qmd-controls");
    if (!bar) return;

    // Theme: cycle auto -> light -> dark, driven by the head-script API
    // (window.qmdSetTheme/qmdGetThemePref), which also honours the OS default.
    const themeBtn = document.createElement("button");
    themeBtn.className = "qmd-ctl";
    themeBtn.type = "button";
    themeBtn.title = "Theme: light / dark / auto (follows your OS)";
    const ICON = { auto: "🖥 auto", light: "☀ light", dark: "🌙 dark" };
    const ORDER = ["auto", "light", "dark"];
    const syncTheme = () => {
      const p = (window.qmdGetThemePref && window.qmdGetThemePref()) || "auto";
      themeBtn.textContent = ICON[p] || ICON.auto;
    };
    themeBtn.addEventListener("click", () => {
      const cur = (window.qmdGetThemePref && window.qmdGetThemePref()) || "auto";
      if (window.qmdSetTheme) window.qmdSetTheme(ORDER[(ORDER.indexOf(cur) + 1) % ORDER.length]);
      syncTheme();
    });
    syncTheme();

    // Click-to-source on/off. When off, clicks pass through normally (so you can
    // select text / drive OJS widgets without jumping to source).
    const srcBtn = document.createElement("button");
    srcBtn.className = "qmd-ctl";
    srcBtn.type = "button";
    srcBtn.textContent = "⌖ source";
    srcBtn.title =
      "Double-click a block to reveal its source" + (inWebview ? " in the editor" : " in VS Code");
    const syncSrc = () => srcBtn.setAttribute("aria-pressed", clickSource ? "true" : "false");
    srcBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      clickSource = !clickSource;
      try { localStorage.setItem(CLICK_KEY, clickSource ? "1" : "0"); } catch (e) {}
      syncSrc();
    });
    syncSrc();

    statusEl = document.createElement("span");
    statusEl.id = "qmd-status";
    statusEl.textContent = "connecting…";
    bar.append(themeBtn, srcBtn, statusEl);
  })();
  // Reveal mode (and any layout without the control bar) keeps its status pill.
  if (!statusEl) statusEl = document.getElementById("qmd-status");

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
  const tocCur = tocEl && document.getElementById("qmd-toc-cur");
  const escText = (s) =>
    s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));
  const buildToc = () => {
    if (!tocEl) return;
    const heads = [...root.querySelectorAll("h1[id], h2[id], h3[id]")];
    if (!heads.length) { tocEl.innerHTML = ""; return; }
    const lvl = (h) => +h.tagName[1];
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

  // TOC scrollspy: highlight the entry whose section currently sits at the top of
  // the viewport, and keep it in view if the TOC is its own scroll area. The link
  // nodes are recreated on every TOC rebuild, so the set is re-collected then; the
  // per-scroll update is a cheap rect read, throttled to one rAF.
  const TOC_ACTIVE = "qmd-toc-active";
  let tocSpy = []; // [{ link, heading }] in document order
  let tocSpyActive = null;
  let tocSpyRaf = 0;
  const updateTocActive = () => {
    if (!tocEl || !tocSpy.length) return;
    // A heading above this y marks the current section. Kept small (just past the
    // 1rem scroll-margin a clicked anchor lands at) so clicking a heading lights up
    // that heading, not the sub-heading right beneath it.
    const line = 44;
    let active = tocSpy[0];
    for (const item of tocSpy) {
      if (item.heading.getBoundingClientRect().top - line <= 0) active = item;
      else break;
    }
    if (active === tocSpyActive) return;
    tocSpyActive = active;
    for (const item of tocSpy) item.link.classList.toggle(TOC_ACTIVE, item === active);
    if (tocCur) tocCur.textContent = active.heading.textContent; // mobile handle chip
    // keep the active entry within view when the TOC scrolls independently
    const lr = active.link.getBoundingClientRect();
    const tr = tocEl.getBoundingClientRect();
    if (lr.top < tr.top) tocEl.scrollTop -= tr.top - lr.top + 8;
    else if (lr.bottom > tr.bottom) tocEl.scrollTop += lr.bottom - tr.bottom + 8;
  };
  const refreshTocSpy = () => {
    if (!tocEl) return;
    tocSpy = [];
    for (const link of tocEl.querySelectorAll("a[href^='#']")) {
      const heading = document.getElementById(
        decodeURIComponent(link.getAttribute("href").slice(1)),
      );
      if (heading) tocSpy.push({ link, heading });
    }
    tocSpyActive = null; // re-apply against the fresh link nodes
    updateTocActive();
  };
  if (tocEl) {
    const onSpy = () => {
      flashTocLabel();
      if (tocSpyRaf) return;
      tocSpyRaf = requestAnimationFrame(() => { tocSpyRaf = 0; updateTocActive(); });
    };
    window.addEventListener("scroll", onSpy, { passive: true });
    window.addEventListener("resize", onSpy);
  }

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
  if (tocHandle && tocEl) {
    const setOpen = (open) => document.body.classList.toggle("qmd-toc-open", open);
    const resetSheet = () => {
      tocEl.style.transition = ""; tocEl.style.transform = "";
      tocBackdrop.style.transition = ""; tocBackdrop.style.opacity = ""; tocBackdrop.style.pointerEvents = "";
    };
    let d = null;
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
    tocBackdrop.addEventListener("click", () => setOpen(false));
    tocEl.addEventListener("click", (e) => { if (e.target.closest("a")) setOpen(false); });

    // Drag the sheet DOWN to dismiss, but only when its list is scrolled to the
    // top (otherwise a downward swipe just scrolls the list). Touch events, not
    // pointer: native scroll won't deliver pointermove, so we take over the touch
    // stream with preventDefault instead.
    let sd = null;
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
    // teach the gesture once on a narrow screen
    if (window.matchMedia && matchMedia("(max-width: 60rem)").matches) {
      tocHandle.classList.add("qmd-hint");
      setTimeout(() => tocHandle.classList.remove("qmd-hint"), 2700);
      flashTocLabel();
    }
  }

  const cssEscape = (s) =>
    window.CSS && CSS.escape ? CSS.escape(s) : s.replace(/["\\]/g, "\\$&");

  const elById = (id) => root.querySelector(`[data-block-id="${cssEscape(id)}"]`);

  const fragment = (html) => {
    const t = document.createElement("template");
    t.innerHTML = html.trim();
    return t.content.firstElementChild;
  };

  // Apply a mutation while keeping the scroll position pinned.
  const keepScroll = (fn) => {
    const y = window.scrollY;
    fn();
    window.scrollTo(0, y);
  };

  // Re-attach reveal, rebuild the TOC, and (re)highlight + add copy buttons to
  // code blocks after any DOM change (each is a no-op when not applicable).
  const afterChange = () => {
    syncReveal();
    buildToc();
    refreshTocSpy();
    if (window.qmdEnhanceCode) window.qmdEnhanceCode(root);
  };

  // The server renders the initial body into the page (so content paints before
  // the websocket connects). The first `full_render` after that is identical, so
  // skip re-mounting it (avoids a flash + needless OJS/reveal re-init); reconnects
  // still re-mount normally.
  let ssrPending = window.QMD_SSR === true;

  const handle = (msg) => {
    switch (msg.type) {
      case "full_render":
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
        const el = elById(msg.target_id);
        if (el) keepScroll(() => el.replaceWith(fragment(msg.html)));
        afterChange();
        break;
      }
      case "insert":
        keepScroll(() => {
          const node = fragment(msg.html);
          const after = msg.after_id && elById(msg.after_id);
          if (after) after.after(node);
          else root.prepend(node);
        });
        afterChange();
        break;
      case "remove": {
        const el = elById(msg.target_id);
        if (el) keepScroll(() => el.remove());
        afterChange();
        break;
      }
      case "error":
        setStatus("error");
        setDiagnostics([{ level: "error", message: msg.message }]);
        break;
    }
  };

  const connect = () => {
    ws = new WebSocket(`ws://${location.host}/ws`);
    ws.onopen = () => setStatus("live");
    ws.onmessage = (e) => handle(JSON.parse(e.data));
    ws.onclose = () => { setStatus("reconnecting…"); setTimeout(connect, 1000); };
    ws.onerror = () => ws.close();
  };

  // Click-to-source: report the clicked block to the server (the editor client
  // will act on this in Phase 3). Also highlight it locally.
  const blockRef = (el) => ({
    block_id: el.dataset.blockId,
    source_file: el.dataset.sourceFile || null,
    sourcepos: el.dataset.sourcepos || null,
  });

  // Open a block's source: in the VS Code webview, relay to the host (which
  // calls revealRange); in a plain browser, open a `vscode://file/…:line:col`
  // link (the server injected the absolute doc + base-dir paths as QMD_DOC).
  const openSource = (el) => {
    const ref = blockRef(el);
    if (inWebview) {
      window.parent.postMessage({ type: "qmd-goto", ...ref }, "*");
      return;
    }
    const doc = window.QMD_DOC;
    if (!doc) return;
    const abs = ref.source_file
      ? doc.baseDir.replace(/\/+$/, "") + "/" + ref.source_file
      : doc.path;
    const m = /^(\d+):(\d+)/.exec(ref.sourcepos || "");
    const line = m ? m[1] : "1";
    const col = m ? m[2] : "1";
    window.location.href = "vscode://file" + encodeURI(abs) + ":" + line + ":" + col;
  };

  // Single click: highlight + tell the server. Skipped when click-to-source is
  // off, or when the click lands on the control bar.
  document.addEventListener("click", (e) => {
    if (!clickSource || (e.target.closest && e.target.closest(".qmd-ctl"))) return;
    const el = e.target.closest("[data-block-id]");
    document.querySelectorAll(".qmd-hl").forEach((n) => n.classList.remove("qmd-hl"));
    if (!el) return;
    el.classList.add("qmd-hl");
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "click_block", ...blockRef(el) }));
    }
  });

  // Double click: jump to source (browser -> VS Code, webview -> host).
  document.addEventListener("dblclick", (e) => {
    if (!clickSource) return;
    const el = e.target.closest("[data-block-id]");
    if (el) openSource(el);
  });

  // Reverse sync: highlight (and reveal/scroll to) the block under the editor
  // cursor. The matching block is the smallest one whose sourcepos range covers
  // `line` in the same source file, else the nearest block starting before it.
  const highlightAtLine = (file, line) => {
    const want = file || null;
    let contained = null, containedSpan = Infinity, preceding = null, precedingStart = -1;
    for (const el of root.querySelectorAll("[data-sourcepos]")) {
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
      const i = sections.indexOf(target.closest(".slides > section"));
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
