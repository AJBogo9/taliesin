// qmd-fast preview client.
//
// Connects to the dev server's websocket and applies a `full_render` followed
// by incremental `update`/`insert`/`remove` block ops. Unchanged blocks are
// never touched, so scroll position and the runtime state of live blocks
// (Three.js canvases, OJS cells) survive edits. Math is rendered server-side,
// so there is nothing to re-run on the client.
(() => {
  const root = document.getElementById("qmd-root");
  const statusEl = document.getElementById("qmd-status");
  let ws;

  const setStatus = (s) => { if (statusEl) statusEl.textContent = s; };

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
    if (window.qmdEnhanceCode) window.qmdEnhanceCode(root);
  };

  const handle = (msg) => {
    switch (msg.type) {
      case "full_render":
        document.title = msg.title || "qmd-fast";
        keepScroll(() => { root.innerHTML = msg.body_html; });
        afterChange();
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
        setStatus("error: " + msg.message);
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

  // Single click: highlight + tell the server (it logs / will drive sync).
  document.addEventListener("click", (e) => {
    const el = e.target.closest("[data-block-id]");
    document.querySelectorAll(".qmd-hl").forEach((n) => n.classList.remove("qmd-hl"));
    if (!el) return;
    el.classList.add("qmd-hl");
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "click_block", ...blockRef(el) }));
    }
  });

  // Double click: jump to source. When embedded in the VS Code webview, relay
  // to the extension host (which calls revealRange); standalone, this is a noop.
  document.addEventListener("dblclick", (e) => {
    const el = e.target.closest("[data-block-id]");
    if (!el || window.parent === window) return;
    window.parent.postMessage({ type: "qmd-goto", ...blockRef(el) }, "*");
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
