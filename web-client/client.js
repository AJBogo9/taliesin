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

  const handle = (msg) => {
    switch (msg.type) {
      case "full_render":
        document.title = msg.title || "qmd-fast";
        keepScroll(() => { root.innerHTML = msg.body_html; });
        syncReveal();
        break;
      case "update": {
        const el = elById(msg.target_id);
        if (el) keepScroll(() => el.replaceWith(fragment(msg.html)));
        syncReveal();
        break;
      }
      case "insert":
        keepScroll(() => {
          const node = fragment(msg.html);
          const after = msg.after_id && elById(msg.after_id);
          if (after) after.after(node);
          else root.prepend(node);
        });
        syncReveal();
        break;
      case "remove": {
        const el = elById(msg.target_id);
        if (el) keepScroll(() => el.remove());
        syncReveal();
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

  connect();
})();
