// @ts-check
// qmd-fast preview client.
//
// Connects to the dev server's websocket and applies a `full_render` followed
// by incremental `update`/`insert`/`remove` block ops. Unchanged blocks are
// never touched, so scroll position and the runtime state of live blocks
// (Three.js canvases, {js} cells) survive edits. Math is rendered server-side,
// so there is nothing to re-run on the client.
//
// ── Websocket protocol ──────────────────────────────────────────────────────
// The server is the producer; these typedefs are the consumer's view of the
// contract. The Rust producers (serve.rs / serve_site.rs `*_json`) are locked to
// these shapes by a contract test (`protocol_contract`); keep the two in sync.
// This file is `// @ts-check`ed (see web-client/jsconfig.json), so the shapes are
// enforced here too.
/**
 * @typedef {{ level: string, message: string, file?: ?string, line?: number, frame?: string }} Diagnostic
 * @typedef {{ type: "full_render", title: ?string, body_html: string, diagnostics: Diagnostic[] }} FullRenderMsg
 * @typedef {{ type: "diagnostics", messages: Diagnostic[] }} DiagnosticsMsg
 * @typedef {{ type: "update", target_id: string, html: string }} UpdateMsg
 * @typedef {{ type: "insert", after_id: ?string, html: string }} InsertMsg
 * @typedef {{ type: "remove", target_id: string }} RemoveMsg
 * @typedef {{ type: "set_meta", target_id: string, sourcepos: string, source_file: ?string }} SetMetaMsg
 * @typedef {{ type: "error", message: string }} ErrorMsg
 * @typedef {{ type: "reload" }} ReloadMsg
 * @typedef {{ type: "style", css: string }} StyleMsg
 * @typedef {{ type: "build-state", page: ?string, phase: "warming-kernel"|"executing"|"idle"|"error", ran: number, total: number, lang: string }} BuildStateMsg
 * @typedef {{ type: "cell-state", page: ?string, cell_id: string, state: "queued"|"running"|"done"|"error", started_ms: ?number, duration_ms: ?number }} CellStateMsg
 * @typedef {FullRenderMsg|DiagnosticsMsg|UpdateMsg|InsertMsg|RemoveMsg|SetMetaMsg|ErrorMsg|ReloadMsg|StyleMsg|BuildStateMsg|CellStateMsg} ServerMessage
 */
(() => {
  const root = document.getElementById("tali-root");
  if (!root) return; // the client mounts into #tali-root; nothing to do without it
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
    const dot = document.getElementById("tali-dev-dot");
    if (dot) dot.dataset.state = state;
  };

  // Words + reading time (prose only: code and math are excluded), refreshed on
  // every change. Shown in the control bar; no-op in deck mode / without it.
  const updateWordCount = () => {
    if (!wordCountEl) return;
    const clone = /** @type {Element} */ (root.cloneNode(true));
    clone.querySelectorAll("pre, .katex, .tali-eqn-number").forEach((n) => n.remove());
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
  // errors (Python `.tali-error`, `{js}` cell `.tali-js-error`) in `cellErrEl`.
  const diagEl = document.createElement("div");
  diagEl.id = "tali-diagnostics";
  diagEl.style.display = "none";
  const cellErrEl = document.createElement("div");
  cellErrEl.id = "tali-cell-errors";
  cellErrEl.style.display = "none";
  let cellErrCount = 0;
  // A third source: accessibility issues found by scanning the rendered output
  // (missing alt text, heading skips, …). Each row jumps to the offending source.
  const a11yEl = document.createElement("div");
  a11yEl.id = "tali-a11y";
  a11yEl.style.display = "none";
  let a11yCount = 0;

  // Reflect the total issue count on the collapsed dev button (amber + a badge),
  // so problems are noticeable without expanding the panel.
  const refreshAlert = () => {
    const diagCount = diagEl.style.display === "none" ? 0 : diagEl.children.length;
    const total = diagCount + cellErrCount + a11yCount;
    const toggle = document.getElementById("tali-dev-toggle");
    if (toggle) toggle.classList.toggle("tali-dev-alert", total > 0);
    const badge = document.getElementById("tali-dev-count");
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
      const located = typeof it.line === "number"; // clickable jump-to-source
      const row = document.createElement(located ? "button" : "div");
      row.className = "tali-diag tali-diag-" + level + (located ? " tali-diag-loc" : "");
      const msg = document.createElement("div");
      msg.textContent = (level === "error" ? "✗ " : "⚠ ") + (it.message || it);
      row.appendChild(msg);
      if (it.frame) {
        const pre = document.createElement("pre");
        pre.className = "tali-diag-frame";
        pre.textContent = it.frame;
        row.appendChild(pre);
      }
      if (located) {
        /** @type {HTMLButtonElement} */ (row).type = "button";
        row.title = "Open this line in your editor";
        row.addEventListener("click", () => gotoSource(it.file || null, /** @type {number} */ (it.line)));
      }
      diagEl.appendChild(row);
    }
    refreshAlert();
  };

  // Scan the mounted content for per-cell errors, list them in the panel (each a
  // button that scrolls to + flashes the failing cell), and update the pill badge.
  // Re-run after every mount and (via a MutationObserver) when async `{js}` errors land.
  const scanCellErrors = () => {
    const errs = root ? [...root.querySelectorAll(".tali-error, .tali-js-error")] : [];
    cellErrCount = errs.length;
    cellErrEl.textContent = "";
    cellErrEl.style.display = errs.length ? "flex" : "none";
    errs.forEach((el, i) => {
      if (!el.id) el.id = "tali-cellerr-" + i;
      const row = document.createElement("button");
      row.type = "button";
      row.className = "tali-cellerr";
      row.textContent = "✗ " + (el.textContent || "cell error").trim().slice(0, 90);
      row.addEventListener("click", () => {
        el.scrollIntoView({ block: "center", behavior: "smooth" });
        pulse(/** @type {HTMLElement} */ (el), "tali-hl-flash");
      });
      cellErrEl.appendChild(row);
    });
    refreshAlert();
  };

  // --- accessibility audit of the rendered output ----------------------------
  // A handful of high-confidence, recurring-and-invisible a11y checks run over the
  // mounted DOM after every render. Each issue becomes a panel row; located ones
  // (tied to a block) jump to the offending source line on click, like a server
  // diagnostic. Cheap, advisory, and never blocks rendering.

  // The source file:line of the nearest locatable ancestor of `el` (a `data-block-id`
  // block or `data-source-file` include), or null when nothing carries a sourcepos.
  const a11yLoc = (/** @type {Element} */ el) => {
    const block = el.closest("[data-sourcepos], [data-block-id]");
    if (!(block instanceof HTMLElement)) return null;
    const m = /^(\d+):/.exec(block.dataset.sourcepos || "");
    if (!m) return null;
    const fileEl = el.closest("[data-source-file]");
    const file = fileEl instanceof HTMLElement ? fileEl.dataset.sourceFile || null : null;
    return { file, line: Number(m[1]) };
  };

  // WCAG relative-luminance contrast ratio between two `[r,g,b]` colors.
  const contrastRatio = (/** @type {number[]} */ a, /** @type {number[]} */ b) => {
    const lum = (/** @type {number[]} */ c) => {
      const f = c.map((v) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
      });
      return 0.2126 * f[0] + 0.7152 * f[1] + 0.0722 * f[2];
    };
    const l1 = lum(a), l2 = lum(b);
    return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
  };
  const parseRgb = (/** @type {string} */ s) => {
    const m = /rgba?\(([^)]+)\)/.exec(s);
    if (!m) return null;
    const p = m[1].split(",").map((x) => parseFloat(x));
    // A fully transparent color carries no contrast signal.
    if (p.length >= 4 && p[3] === 0) return null;
    return [p[0], p[1], p[2]];
  };
  // The first opaque background walking up from `el` (defaults to white).
  const bgColor = (/** @type {Element|null} */ el) => {
    for (let e = el; e; e = e.parentElement) {
      const rgb = parseRgb(getComputedStyle(e).backgroundColor);
      if (rgb) return rgb;
    }
    return [255, 255, 255];
  };

  const scanA11y = () => {
    /** @type {{message:string, file:?string, line?:number}[]} */
    const issues = [];
    /** @param {string} message @param {Element} [near] */
    const add = (message, near) => {
      const loc = near ? a11yLoc(near) : null;
      issues.push({ message, file: loc?.file ?? null, line: loc?.line });
    };
    if (root) {
      // 1. Images need an `alt` attribute (decorative images use `alt=""`).
      let nAlt = 0;
      for (const img of root.querySelectorAll("img:not([alt])")) {
        if (nAlt++ < 8) add("Image is missing alt text (use alt=\"\" if decorative)", img);
      }
      if (nAlt > 8) add(`…and ${nAlt - 8} more images missing alt text`);

      // 2. Heading levels shouldn't skip a level going deeper (h2 → h4).
      let prev = 0;
      for (const h of root.querySelectorAll("h1,h2,h3,h4,h5,h6")) {
        const lvl = Number(h.tagName[1]);
        if (prev && lvl > prev + 1) {
          add(`Heading level skips from h${prev} to h${lvl}`, h);
        }
        prev = lvl;
      }

      // 3. Links/buttons need an accessible name (text, aria-label, title, or an
      //    alt-bearing image), or a screen reader announces nothing.
      let nName = 0;
      for (const el of root.querySelectorAll("a[href], button")) {
        const named =
          (el.textContent || "").trim() ||
          el.getAttribute("aria-label") ||
          el.getAttribute("title") ||
          el.querySelector("img[alt]:not([alt=''])") ||
          el.querySelector("svg [role='img'], svg title");
        if (!named && nName++ < 5) {
          add(`${el.tagName === "A" ? "Link" : "Button"} has no accessible name`, el);
        }
      }
    }

    // 4. The document needs a language (set on <html lang>).
    const lang = document.documentElement.getAttribute("lang");
    if (!lang || !lang.trim()) add("Document is missing a language (<html lang>)");

    // 5. Body-text contrast should meet WCAG AA (4.5:1).
    try {
      const probe = root && root.querySelector("p") ? root.querySelector("p") : document.body;
      if (probe) {
        const fg = parseRgb(getComputedStyle(probe).color);
        if (fg) {
          const ratio = contrastRatio(fg, bgColor(probe));
          if (ratio < 4.5) {
            add(`Body text contrast ${ratio.toFixed(1)}:1 is below WCAG AA (4.5:1)`);
          }
        }
      }
    } catch (_e) {
      /* getComputedStyle can throw in odd layouts; a contrast miss is non-fatal */
    }

    a11yCount = issues.length;
    a11yEl.textContent = "";
    a11yEl.style.display = issues.length ? "flex" : "none";
    for (const it of issues) {
      const located = typeof it.line === "number";
      const row = document.createElement(located ? "button" : "div");
      row.className = "tali-diag tali-diag-warning" + (located ? " tali-diag-loc" : "");
      const msg = document.createElement("div");
      msg.textContent = "♿ " + it.message;
      row.appendChild(msg);
      if (located) {
        /** @type {HTMLButtonElement} */ (row).type = "button";
        row.title = "Open this line in your editor";
        row.addEventListener("click", () => gotoSource(it.file, /** @type {number} */ (it.line)));
      }
      a11yEl.appendChild(row);
    }
    refreshAlert();
  };

  // --- fatal-error overlay ---------------------------------------------------
  // A render/read failure leaves the last good content in place; this overlays it
  // so a broken save is impossible to miss (including on a phone). It clears on
  // the next successful render, or on click-outside / Escape.
  const errorEl = (() => {
    const style = document.createElement("style");
    style.textContent =
      "#tali-error{position:fixed;inset:0;z-index:2147482500;display:none;flex-direction:column;" +
      "align-items:center;justify-content:center;padding:2rem;box-sizing:border-box;" +
      "background:rgba(10,12,16,.86);-webkit-backdrop-filter:blur(3px);backdrop-filter:blur(3px);}" +
      "#tali-error.tali-show{display:flex;}" +
      "#tali-error .tali-error-card{max-width:min(680px,92vw);width:100%;max-height:74vh;overflow:auto;" +
      "background:#1b1d23;border:1px solid #5a2a2a;border-left:4px solid #e5534b;border-radius:10px;" +
      "padding:1rem 1.2rem;box-shadow:0 14px 44px rgba(0,0,0,.55);}" +
      "#tali-error .tali-error-title{font:600 13px ui-sans-serif,system-ui,sans-serif;color:#ff8c82;margin-bottom:.55rem;}" +
      "#tali-error pre{margin:0;padding:0;background:transparent;white-space:pre-wrap;word-break:break-word;" +
      "font:13px/1.5 ui-monospace,SFMono-Regular,Menlo,monospace;color:#f2d5d5;}" +
      "#tali-error .tali-error-hint{margin-top:.85rem;font:12px ui-sans-serif,system-ui,sans-serif;color:#9aa0aa;}";
    (document.head || document.documentElement).appendChild(style);
    const el = document.createElement("div");
    el.id = "tali-error";
    el.innerHTML =
      '<div class="tali-error-card"><div class="tali-error-title">⚠ Render failed</div><pre></pre>' +
      '<div class="tali-error-hint">Fix the source and save; this clears on the next successful render. (Esc to dismiss)</div></div>';
    document.body.appendChild(el);
    el.addEventListener("click", (e) => { if (e.target === el) el.classList.remove("tali-show"); });
    return el;
  })();
  const showError = (/** @type {string=} */ message) => {
    const pre = errorEl.querySelector("pre");
    if (pre) pre.textContent = message || "Unknown error";
    errorEl.classList.add("tali-show");
  };
  const hideError = () => errorEl.classList.remove("tali-show");
  // A successful render arrived: drop the overlay and clear the "error" status.
  const renderOk = () => {
    hideError();
    if (statusEl && statusEl.textContent === "error") setStatus("live");
  };
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && errorEl.classList.contains("tali-show")) hideError();
  });

  // Briefly pulse a block with a self-removing animated class: the change-flash on
  // re-render (`tali-flash`), and the click-to-source highlight (`tali-hl-flash`).
  const pulse = (/** @type {Element|null} */ el, /** @type {string} */ cls) => {
    if (!el || !el.classList) return;
    el.classList.remove(cls);
    void (/** @type {HTMLElement} */ (el)).offsetWidth; // restart the animation when re-pulsing the same node
    el.classList.add(cls);
    el.addEventListener("animationend", () => el.classList.remove(cls), { once: true });
  };

  // --- preview control bar: theme toggle + click-to-source hint ------------
  const inWebview = window.parent !== window;
  // Click-to-source is a modifier gesture (Alt/Option-click), not a mode: a plain
  // click always browses normally, so there's no state to toggle or remember and no
  // way to accidentally jump to the editor. The dev menu carries a hint.

  // One collapsed dev menu (Next.js-style): a corner button showing the live
  // status dot, expanding to a panel with the preview-only tools — live status,
  // word count, the click-to-source toggle, diagnostics, and (only when the page
  // chrome has no real theme toggle, i.e. single-doc preview) a theme toggle.
  // The site navbar's theme toggle is a real, shipped feature, not a dev tool.
  (function buildDevMenu() {
    const host = document.getElementById("tali-controls");
    if (!host) return;
    host.classList.add("tali-dev");

    const devRow = (/** @type {string} */ label, /** @type {HTMLElement} */ valueEl) => {
      const row = document.createElement("div");
      row.className = "tali-dev-row";
      const l = document.createElement("span");
      l.className = "tali-dev-label";
      l.textContent = label;
      row.append(l, valueEl);
      return row;
    };

    const toggle = document.createElement("button");
    toggle.id = "tali-dev-toggle";
    toggle.className = "tali-dev-toggle";
    toggle.type = "button";
    toggle.setAttribute("aria-label", "Developer tools");
    toggle.setAttribute("aria-expanded", "false");
    toggle.innerHTML =
      '<span class="tali-dev-dot" id="tali-dev-dot"></span><span class="tali-dev-glyph">&lt;/&gt;</span>' +
      '<span class="tali-dev-count" id="tali-dev-count" hidden></span>';

    const panel = document.createElement("div");
    panel.id = "tali-dev-panel";
    panel.className = "tali-dev-panel";
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
    statusEl.id = "tali-status";
    wordCountEl = document.createElement("span");
    wordCountEl.id = "tali-wordcount";

    // Click-to-source hint: Alt/Option-click any block to open its source. No toggle
    // — a plain click browses normally; the modifier is the whole gesture.
    const srcHint = document.createElement("span");
    srcHint.id = "tali-src-hint";
    srcHint.textContent = "Alt-click a block";
    srcHint.title =
      "Hold Alt (Option on Mac) and click any block to open its source" +
      (inWebview ? " in the editor" : " in your editor");

    // Restart the warm Jupyter kernel: drops the (possibly dead/wedged) kernel and
    // re-runs every cell against a fresh one. Recovers after fixing QMD_FAST_PYTHON.
    const kernelBtn = document.createElement("button");
    kernelBtn.id = "tali-kernel-ctl";
    kernelBtn.className = "tali-dev-ctl";
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

    panel.append(devRow("Status", statusEl), devRow("Words", wordCountEl), devRow("Source", srcHint), kernelBtn);

    // Single-doc preview has no site navbar, so give the dev menu its own theme
    // toggle (wired by the shared theme_head). Sites use the navbar's instead.
    if (!document.querySelector("[data-qmd-theme-toggle]")) {
      const themeBtn = document.createElement("button");
      themeBtn.className = "tali-dev-ctl tali-dev-theme";
      themeBtn.type = "button";
      themeBtn.setAttribute("data-qmd-theme-toggle", "");
      panel.appendChild(themeBtn);
      if (window.taliWireThemeToggles) window.taliWireThemeToggles();
    }

    // Diagnostics, per-cell errors, and a11y findings all live inside the panel.
    panel.append(diagEl, cellErrEl, a11yEl);
    host.append(toggle, panel);
    setStatus("connecting…");
  })();
  // Deck mode (and any layout without the control bar) keeps its status pill.
  if (!statusEl) statusEl = document.getElementById("tali-status");

  // --- in-browser execution progress chip ------------------------------------
  // A small fixed chip (bottom-right) that shows k/N while code cells are
  // executing, then "Up to date" when idle. Preview-only — never in build output.
  // --- per-cell execution state decoration ------------------------------------
  // Decorates each output block ({cell_id}-out) with a colored left-border and
  // a small badge showing queued/running (with live elapsed)/done/error state.
  // Driven entirely by server `cell-state` messages; we never infer state.
  var runningTimers = /** @type {Record<string, number>} */ ({}); // cell_id -> started_ms
  var activeCell = /** @type {string|null} */ (null); // last cell_id seen in "running" state
  function fmtElapsed(/** @type {number} */ ms) { return (ms / 1000).toFixed(1) + "s"; }
  function applyCellState(/** @type {CellStateMsg} */ msg) {
    var out = elById(msg.cell_id + "-out") || elById(msg.cell_id);
    if (!out) return;
    out.setAttribute("data-qmd-cell-state", msg.state);
    var badge = out.querySelector(":scope > .tali-cell-badge") || (function () {
      var b = document.createElement("span"); b.className = "tali-cell-badge";
      out.insertBefore(b, out.firstChild); return b;
    })();
    if (msg.state === "running") {
      activeCell = msg.cell_id; // track the active cell for click-to-scroll
      runningTimers[msg.cell_id] = msg.started_ms || Date.now();
      badge.textContent = "⏳ 0.0s";
    } else {
      delete runningTimers[msg.cell_id];
      if (msg.state === "error") activeCell = msg.cell_id; // keep erroring cell as scroll target
      if (msg.state === "done") badge.textContent = "✓ " + (msg.duration_ms != null ? fmtElapsed(msg.duration_ms) : "");
      else if (msg.state === "error") badge.textContent = "✕";
      else badge.textContent = "⏳"; // queued
    }
  }
  setInterval(function () {
    var now = Date.now();
    Object.keys(runningTimers).forEach(function (id) {
      var out = elById(id + "-out") || elById(id);
      if (!out) return;
      var b = out.querySelector(":scope > .tali-cell-badge");
      if (b) b.textContent = "⏳ " + fmtElapsed(now - runningTimers[id]);
    });
  }, 200);

  // --- progress chip: idle/busy dot, k/N bar, click-to-scroll, tab-title/favicon ---
  var progressEl = /** @type {HTMLElement|null} */ (null);
  var buildStartMs = /** @type {number|null} */ (null); // set on first non-idle build-state
  var warmStartMs = /** @type {number|null} */ (null); // set at first warming-kernel of a build
  var warmTimer = 0; // interval id ticking the warm-up elapsed label
  var buildErrored = false; // latched on `error`; cleared only when a fresh build starts
  var baseTitle = document.title || "qmd-fast"; // save original title for restore

  // Canvas-drawn favicon: a coloured dot superimposed on the base favicon SVG.
  // Swapped in while busy/error; the link[rel=icon] href is restored on idle.
  var origFavicon = /** @type {string|null} */ (null); // original href, captured once
  function setFaviconDot(/** @type {string|null} */ color) {
    var link = /** @type {HTMLLinkElement|null} */ (document.querySelector("link[rel~='icon']"));
    if (!link) {
      link = document.createElement("link");
      link.rel = "icon";
      document.head.appendChild(link);
    }
    if (origFavicon === null) origFavicon = link.href; // capture once
    if (!color) { link.href = origFavicon || ""; return; } // restore on idle
    try {
      var c = document.createElement("canvas");
      c.width = 32; c.height = 32;
      var ctx = c.getContext("2d");
      if (!ctx) return;
      // Draw the dot (bottom-right quadrant, radius 7, with a 1.5px white ring)
      ctx.clearRect(0, 0, 32, 32);
      ctx.beginPath(); ctx.arc(24, 24, 8.5, 0, 2 * Math.PI);
      ctx.fillStyle = "#fff"; ctx.fill();
      ctx.beginPath(); ctx.arc(24, 24, 7, 0, 2 * Math.PI);
      ctx.fillStyle = color; ctx.fill();
      link.href = c.toDataURL("image/png");
    } catch (_e) { /* canvas blocked (CSP / non-browser env) */ }
  }

  function ensureProgress() {
    if (progressEl) return progressEl;
    progressEl = document.createElement("div");
    progressEl.id = "tali-progress";
    progressEl.setAttribute("aria-live", "polite");
    progressEl.setAttribute("role", "status");
    // Click-to-scroll: jump to the currently running or last-errored cell output.
    progressEl.addEventListener("click", function () {
      if (!activeCell) return;
      var target = elById(activeCell + "-out") || elById(activeCell);
      if (target) target.scrollIntoView({ block: "center", behavior: "smooth" });
    });
    document.body.appendChild(progressEl);
    return progressEl;
  }

  // Stop the warm-up elapsed ticker (called whenever we leave the warming phase).
  function stopWarmTimer() {
    if (warmTimer) { clearInterval(warmTimer); warmTimer = 0; }
    warmStartMs = null;
  }

  function updateProgress(/** @type {BuildStateMsg} */ msg) {
    var el = ensureProgress();
    if (msg.phase === "idle") {
      // Honest failures: a build that settled on `error` must NOT be flipped to
      // "Up to date" by a stray/later `idle` for that same failed build. The server
      // no longer emits a trailing `idle` after a boot failure, but we latch here
      // too so the error chip survives until a genuinely new build begins.
      if (buildErrored) return;
      stopWarmTimer();
      var elapsed = buildStartMs !== null ? Math.round((Date.now() - buildStartMs) / 1000) : null;
      buildStartMs = null;
      var elapsedTxt = elapsed !== null ? ", built in " + elapsed + "s" : "";
      // Inner HTML: dot + text label
      el.innerHTML =
        "<span class=\"tali-prog-dot\"></span>" +
        "<span class=\"tali-prog-label\">Up to date" + elapsedTxt + "</span>";
      el.setAttribute("data-state", "idle");
      el.removeAttribute("title");
      // Restore tab title and favicon
      document.title = baseTitle;
      setFaviconDot(null);
      return;
    }
    // A fresh build starting: clear any latched error so the chip can recover, and
    // start (or restart) the build timer. We trigger on warming-kernel/executing rather
    // than on buildStartMs===null because after a failure buildStartMs is still set
    // (the idle branch returned early without clearing it), so the null-check alone
    // would never fire and the error latch would never clear.
    var isNewBuild = msg.phase === "warming-kernel" || msg.phase === "executing";
    if (isNewBuild) { buildErrored = false; if (buildStartMs === null) buildStartMs = Date.now(); }
    if (msg.phase === "error") {
      buildErrored = true; // latch: subsequent `idle` won't overwrite the error chip
      stopWarmTimer();
      el.innerHTML =
        "<span class=\"tali-prog-dot\"></span>" +
        "<span class=\"tali-prog-label\">Error</span>";
      el.setAttribute("data-state", "error");
      el.title = "Click to scroll to erroring cell";
      document.title = "⚠ error — " + baseTitle;
      setFaviconDot("#e5534b");
      return;
    }
    // warming-kernel: a distinct, timed phase — "Starting <lang> kernel… (Ns)". No
    // k/N bar (nothing has run yet), and queued cells stay `queued` (the server emits
    // no `running` cell-state during warm-up), so nothing falsely shows as running.
    var isWarming = msg.phase === "warming-kernel";
    if (isWarming) {
      if (warmStartMs === null) {
        warmStartMs = Date.now();
        warmTimer = setInterval(function () { renderWarming(el, msg.lang); }, 200);
      }
      renderWarming(el, msg.lang);
      el.setAttribute("data-state", "warming");
      el.title = "Starting kernel…";
      document.title = "● starting kernel… — " + baseTitle;
      setFaviconDot("#d9a23a");
      return;
    }
    // executing: show dot + k/N text + mini bar.
    stopWarmTimer();
    var barPct = msg.total > 0 ? (msg.ran / msg.total) : 0;
    el.innerHTML =
      "<span class=\"tali-prog-dot\"></span>" +
      "<span class=\"tali-prog-label\"></span>" +
      "<span class=\"tali-prog-bar\" aria-hidden=\"true\">" +
        "<span class=\"tali-prog-fill\" style=\"width:" + Math.round(barPct * 100) + "%\"></span>" +
      "</span>";
    // Set label via textContent so server-controlled values can't inject HTML.
    var busyLabel = el.querySelector(".tali-prog-label");
    if (busyLabel) busyLabel.textContent = msg.ran + "/" + msg.total;
    el.setAttribute("data-state", "busy");
    el.title = "Click to scroll to active cell";
    document.title = "● building… — " + baseTitle;
    setFaviconDot("#4c8dff");
  }

  // Render the warm-up chip: a dot + "Starting <lang> kernel… (Ns)". The lang and
  // elapsed are set via textContent so a server-controlled `lang` can't inject HTML.
  function renderWarming(/** @type {HTMLElement} */ el, /** @type {string} */ lang) {
    if (!el.querySelector(".tali-prog-label")) {
      el.innerHTML =
        "<span class=\"tali-prog-dot\"></span>" +
        "<span class=\"tali-prog-label\"></span>";
    }
    var secs = warmStartMs !== null ? ((Date.now() - warmStartMs) / 1000).toFixed(1) : "0.0";
    var warmLabel = el.querySelector(".tali-prog-label");
    if (warmLabel)
      warmLabel.textContent = "Starting " + lang + " kernel… (" + secs + "s)";
  }

  // A `{js}` cell can error asynchronously (its async body runs after the mount);
  // watch the content for them (debounced) so the dev-menu count stays live.
  if (window.MutationObserver) {
    let t = 0;
    new MutationObserver(() => {
      clearTimeout(t);
      t = setTimeout(scanCellErrors, 200);
    }).observe(root, { childList: true, subtree: true });
  }

  // Deck mode: the body is sectioned slides mounted into `.tali-deck > .tali-slides`
  // (root). After any DOM change we (re)attach the deck engine: the first change
  // initializes, later ones only `sync()`, so the current slide and the runtime
  // state of live blocks survive edits.
  const isDeck = window.TALIESIN_FORMAT === "deck";
  let deckReady = false;
  const syncDeck = () => {
    if (!isDeck || !window.TaliesinDeck) return;
    if (!deckReady) {
      window.TaliesinDeck.initialize({ hash: true, slideNumber: "c/t", center: false });
      deckReady = true;
    } else {
      window.TaliesinDeck.sync();
      window.TaliesinDeck.layout();
    }
  };

  // TOC mode: rebuild `<nav id="TOC">` from the mounted, anchored headings after
  // every change, so the contents stay live as headings are edited/added/removed.
  const tocEl = window.TALIESIN_TOC === true ? document.getElementById("TOC") : null;
  // Mobile pull-up sheet chrome (present only on the live TOC page).
  const tocHandle = tocEl && document.getElementById("tali-toc-handle");
  const tocBackdrop = tocEl && document.getElementById("tali-toc-backdrop");
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

  // TOC scrollspy lives in the shared toc-spy.js (window.taliInitTocSpy) so the
  // live preview and the static build highlight the active section identically.
  // The client only re-inits it after rebuilding the nav (see mountAll) and feeds
  // it the mobile pull-up label flash on scroll.
  if (tocEl) window.taliTocScrollHook = () => flashTocLabel();

  // Mobile pull-up TOC: drag the handle up (the sheet follows) or tap it to open;
  // tap the backdrop or a TOC entry to close. The current-section chip flashes in
  // while scrolling, then fades, so the resting handle stays quiet.
  let tocLabelTimer = 0;
  function flashTocLabel() {
    if (!tocHandle) return;
    tocHandle.classList.add("tali-show-label");
    clearTimeout(tocLabelTimer);
    tocLabelTimer = setTimeout(() => tocHandle.classList.remove("tali-show-label"), 1000);
  }
  if (tocHandle && tocEl && tocBackdrop) {
    const isSheetMode = () => !window.matchMedia || matchMedia("(max-width: 60rem)").matches;
    // #TOC doubles as the desktop sidebar, so only hide it from assistive tech and
    // pull it out of the tab order when it is an off-screen sheet (narrow + closed).
    const syncSheetA11y = () => {
      const open = document.body.classList.contains("tali-toc-open");
      tocHandle.setAttribute("aria-expanded", open ? "true" : "false");
      if (isSheetMode() && !open) {
        tocEl.setAttribute("inert", ""); tocEl.setAttribute("aria-hidden", "true");
      } else {
        tocEl.removeAttribute("inert"); tocEl.removeAttribute("aria-hidden");
      }
    };
    const setOpen = (/** @type {boolean} */ open) => {
      document.body.classList.toggle("tali-toc-open", open);
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
      if (!document.body.classList.contains("tali-toc-open")) { sd = null; return; }
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
      if (e.key === "Escape" && document.body.classList.contains("tali-toc-open")) {
        setOpen(false); tocHandle.focus();
      }
    });
    window.addEventListener("resize", syncSheetA11y);
    syncSheetA11y();

    // teach the gesture once on a narrow screen
    if (isSheetMode()) {
      tocHandle.classList.add("tali-hint");
      setTimeout(() => tocHandle.classList.remove("tali-hint"), 2700);
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

  // Tear down any `{js}` cells inside `el` (a block about to be detached on
  // update/remove): qmd-js resolves the cell's `invalidation`, so the author's
  // `invalidation.then(() => renderer.dispose() / cancelAnimationFrame(...))` cleanup
  // runs, and splices the cell out of its push-only registry. Without this, editing a
  // `{js}`/Three.js cell (which changes its content-hash block id, so we replaceWith a
  // fresh node) would leak a WebGL context + RAF loop on every edit. No-op when qmd-js
  // isn't loaded (decks/pages with no `{js}` cells). `window.taliJs` is set by qmd-js.js
  // and declared on the shared `Window` type in globals.d.ts.
  const teardownJs = (/** @type {Element|null} */ el) => {
    const q = window.taliJs;
    if (q && q.teardown && el) q.teardown(el);
  };
  const resetJs = () => {
    const q = window.taliJs;
    if (q && q.reset) q.reset();
  };

  // Re-attach the deck, rebuild the TOC, and (re)highlight + add copy buttons to
  // code blocks after any DOM change (each is a no-op when not applicable).
  const afterChange = () => {
    syncDeck();
    buildToc();
    if (window.taliInitTocSpy) window.taliInitTocSpy(); // re-collect against the fresh nav
    updateWordCount();
    if (window.taliEnhanceCode) window.taliEnhanceCode(root);
    scanCellErrors();
    scanA11y();
  };

  // A single save emits a BURST of block ops (each its own websocket message).
  // afterChange() is entirely O(document) derived-UI recompute (TOC + scrollspy +
  // word count deep-clones #tali-root + a11y/code scans), so running it per op was an
  // O(ops × doc) cliff on the save hot path. Coalesce the burst into ONE afterChange on
  // the next animation frame — every op in the frame has applied by then.
  let afterChangeRAF = 0;
  const scheduleAfterChange = () => {
    if (afterChangeRAF) return;
    afterChangeRAF = requestAnimationFrame(() => {
      afterChangeRAF = 0;
      afterChange();
    });
  };

  // The server renders the initial body into the page (so content paints before
  // the websocket connects). The first `full_render` after that is identical, so
  // skip re-mounting it (avoids a flash + needless {js}/deck re-init); reconnects
  // still re-mount normally.
  let ssrPending = window.TALIESIN_SSR === true;

  /** @param {ServerMessage} msg */
  const handle = (msg) => {
    switch (msg.type) {
      case "full_render":
        renderOk(); // a fresh render arrived: any prior failure is resolved
        document.title = msg.title || "qmd-fast";
        if (ssrPending) {
          ssrPending = false; // content already server-rendered into #tali-root
        } else {
          // Wholesale re-mount (reconnect / structural change): tear down ALL prior
          // `{js}` cells first (resolving every outstanding `invalidation`) so their
          // WebGL contexts + RAF loops are released and the qmd-js runtime is rebuilt
          // fresh, rather than re-pushing duplicate cells onto a never-reset registry.
          resetJs();
          keepScroll(() => { root.innerHTML = msg.body_html; });
        }
        scheduleAfterChange();
        setDiagnostics(msg.diagnostics);
        break;
      case "diagnostics":
        setDiagnostics(msg.messages);
        break;
      case "build-state":
        updateProgress(/** @type {BuildStateMsg} */ (msg));
        break;
      case "cell-state":
        applyCellState(/** @type {CellStateMsg} */ (msg));
        break;
      case "update": {
        renderOk();
        const el = elById(msg.target_id);
        const node = fragment(msg.html);
        if (el && node) {
          teardownJs(el); // resolve invalidation + drop {js} cells in the outgoing block
          keepScroll(() => el.replaceWith(node));
          pulse(node, "tali-flash");
        }
        scheduleAfterChange();
        break;
      }
      case "insert": {
        renderOk();
        const node = fragment(msg.html);
        if (node) {
          // Block ids are unique per document, so drop any element already
          // carrying this id before inserting. The server emits Removes before
          // Inserts, so this is normally a no-op; it defends against a stale
          // duplicate if ops ever arrive out of order (a reorder splits a moved
          // block into Remove+Insert of the same id).
          const newId = node.getAttribute && node.getAttribute("data-block-id");
          const stale = newId && elById(newId);
          if (stale) teardownJs(stale); // tear down {js} cells in a stale duplicate before dropping it
          keepScroll(() => {
            if (stale) stale.remove();
            const after = msg.after_id && elById(msg.after_id);
            if (after) after.after(node);
            else root.prepend(node);
          });
          pulse(node, "tali-flash");
        }
        scheduleAfterChange();
        break;
      }
      case "remove": {
        renderOk();
        const el = elById(msg.target_id);
        if (el) {
          teardownJs(el); // resolve invalidation + drop {js} cells in the removed block
          keepScroll(() => el.remove());
        }
        scheduleAfterChange();
        break;
      }
      case "set_meta": {
        // A structural edit elsewhere shifted this block's lines but not its
        // content. Patch only its position attributes so click-to-source stays
        // exact — without re-rendering, so its live DOM state (video, {js} widget,
        // open <details>) survives. No afterChange(): content is unchanged.
        renderOk();
        const el = elById(msg.target_id);
        if (el) {
          el.setAttribute("data-sourcepos", msg.sourcepos);
          if (msg.source_file) el.setAttribute("data-source-file", msg.source_file);
          else el.removeAttribute("data-source-file");
        }
        break;
      }
      case "error":
        setStatus("error");
        showError(msg.message);
        break;
      case "style": {
        // Hot-swap theme CSS in place (no reload): scroll + deck slide survive.
        let s = document.getElementById("qmd-theme");
        if (!s) {
          s = document.createElement("style");
          s.id = "qmd-theme";
          (document.head || document.documentElement).appendChild(s);
        }
        s.textContent = msg.css;
        break;
      }
      // Multi-page site: the project config (or a structural change) changed,
      // so the whole page is re-fetched rather than block-diffed.
      case "reload":
        location.reload();
        break;
    }
  };

  const connect = () => {
    // In a multi-page site the ws is scoped to the current page (TALIESIN_WS_PATH);
    // a single-doc preview uses the plain "/ws".
    const wsPath = window.TALIESIN_WS_PATH || "/ws";
    ws = new WebSocket(`ws://${location.host}${wsPath}`);
    ws.onopen = () => setStatus("live");
    ws.onmessage = (e) => {
      // Never let a malformed message or a handler bug throw uncaught (which would kill
      // the socket silently): surface it in the diagnostics overlay + console and keep
      // the connection live so the next good message still applies.
      let msg;
      try {
        msg = JSON.parse(e.data);
      } catch (err) {
        const m = err instanceof Error ? err.message : String(err);
        console.error("qmd: could not parse server message", err, e.data);
        setStatus("error");
        showError("malformed server message: " + m);
        return;
      }
      try {
        handle(msg);
      } catch (err) {
        const m = err instanceof Error ? err.message : String(err);
        console.error("qmd: error handling server message", err, msg);
        setStatus("error");
        showError("client error applying a server update: " + m);
      }
    };
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

  // Jump the editor to a source file:line directly (file relative to the doc's base
  // dir, or null = the previewed doc itself). Used by located diagnostics; webview
  // relays to the host, a browser opens vscode://. openSource() handles the el case.
  const gotoSource = (/** @type {?string} */ file, /** @type {number} */ line) => {
    const doc = window.TALIESIN_DOC;
    if (!doc) return;
    if (inWebview) {
      window.parent.postMessage({ type: "qmd-goto", source_file: file, sourcepos: line + ":1" }, "*");
      return;
    }
    const abs = file ? doc.baseDir.replace(/\/+$/, "") + "/" + file : doc.path;
    window.location.href = "vscode://file" + encodeURI(abs) + ":" + line + ":1";
  };

  // Open the source for an element: an explicit `data-qmd-src` (site-root-relative,
  // `rel` or `rel:line`) wins; else the block's sourcepos on the current page (or an
  // included file). In the webview, relay to the host; in a browser, `vscode://`.
  const openSource = (/** @type {HTMLElement} */ el) => {
    const doc = window.TALIESIN_DOC;
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

  const inDevMenu = (/** @type {Element} */ t) => !!t.closest("#tali-controls");

  // Click-to-source: Alt/Option-click any block to jump to its source line (browser
  // -> vscode://, webview -> host). A plain click browses normally, so there's no
  // mode and no way to land in the editor by accident.
  document.addEventListener("click", (e) => {
    if (!e.altKey) return;
    const t = e.target instanceof Element ? e.target : null;
    if (!t || inDevMenu(t)) return;
    const el = locatable(t);
    if (!el) return;
    e.preventDefault(); // suppress text selection / link navigation on the Alt-click
    pulse(el, "tali-hl-flash");
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "click_block", ...blockRef(el) }));
    }
    openSource(el);
  });

  // Click-to-source affordance: while Alt is held, make the otherwise-invisible
  // gesture visible. `html.tali-alt` flips every source-mapped block to a pointer
  // cursor, and the single block a click would actually resolve to (via the same
  // `locatable()` used by the click handler, so highlight and jump can never drift)
  // wears a dashed outline that tracks the mouse. Pure feedback — no write path,
  // nothing shown until Alt is down, and no animation: hover is continuous tracking
  // and must read as instantaneous, while the jump itself already pulses on commit.
  (() => {
    let altOn = false;
    /** @type {HTMLElement|null} */ let hovered = null;
    let lastX = 0, lastY = 0;

    // Move the dashed outline to the locatable block at `target`, or clear it.
    // Idempotent: touches the DOM only when the resolved block actually changes.
    const markEl = (/** @type {Element|null} */ target) => {
      const el = target && !inDevMenu(target) ? locatable(target) : null;
      if (el === hovered) return;
      if (hovered) hovered.classList.remove("tali-src-hover");
      hovered = el;
      if (el) el.classList.add("tali-src-hover");
    };

    const enterAlt = () => {
      if (altOn) return;
      altOn = true;
      document.documentElement.classList.add("tali-alt");
      markEl(document.elementFromPoint(lastX, lastY)); // highlight what's already under the cursor
    };
    const exitAlt = () => {
      if (!altOn) return;
      altOn = false;
      document.documentElement.classList.remove("tali-alt");
      markEl(null);
    };

    window.addEventListener("keydown", (e) => { if (e.key === "Alt") enterAlt(); });
    window.addEventListener("keyup", (e) => { if (e.key === "Alt") exitAlt(); });
    document.addEventListener("mousemove", (e) => {
      lastX = e.clientX;
      lastY = e.clientY;
      if (altOn) markEl(e.target instanceof Element ? e.target : null);
    }, { passive: true });
    // Never leave the affordance stuck "armed" if focus leaves mid-press (alt-tab,
    // an Alt-click that navigates to vscode://, switching tabs): the keyup may never
    // arrive, so reset on blur / hide.
    window.addEventListener("blur", exitAlt);
    document.addEventListener("visibilitychange", () => { if (document.hidden) exitAlt(); });
  })();

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
    document.querySelectorAll(".tali-hl").forEach((n) => n.classList.remove("tali-hl"));
    target.classList.add("tali-hl");
    if (isDeck && window.TaliesinDeck) {
      const sections = [...root.querySelectorAll(".tali-slides > section")];
      const sec = target.closest(".tali-slides > section");
      const i = sec ? sections.indexOf(sec) : -1;
      if (i >= 0) window.TaliesinDeck.slide(i);
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

  // `--host` puts the session token in the first URL (`?t=…`); the server has
  // already set the auth cookie on this very response, so the token in the address
  // bar is now just leakage (browser history, a bookmark, a copied link, an outbound
  // Referer). Strip only `t`, preserving the path, any other query params, and the
  // hash. A no-op when there is no token (the common, non-`--host` case).
  try {
    const u = new URL(window.location.href);
    if (u.searchParams.has("t")) {
      u.searchParams.delete("t");
      const qs = u.searchParams.toString();
      history.replaceState(history.state, "", u.pathname + (qs ? "?" + qs : "") + u.hash);
    }
  } catch (e) {}

  connect();
})();
