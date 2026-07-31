// Native interactive `{js}` cells — a tiny enhancer that replaces the vendored
// 440 KB Observable runtime. Each `{js}` cell is emitted (render/mod.rs) as a
// `<script type="application/tali-js">` carrying the author's JS plus a sibling
// target `<div>`; this enhancer runs the source with a small scope and mounts a
// returned DOM node into the target. Plot + d3 are vendored globals (window.Plot,
// window.d3); Three.js is dynamically `import()`ed by the cell itself.
//
// The corpus's whole reactive surface is single-input fan-out (one input -> a few
// sink cells; intermediate helpers are pure), so there is no dataflow engine here:
// inputs are plain DOM elements and a sink re-runs when a named input fires. Cell
// kinds (from `//|` options):
//   //| viewof: NAME  -> returns a DOM input; registered as input NAME, mounted.
//   //| name: NAME    -> return value stored in the shared scope as NAME.
//   //| input: A, B   -> sink: re-runs when input/define A or B changes.
//   (none)            -> one-shot: runs once; re-runs when a Python define lands.
//
// Cell scope: { get(n), set(n,v), value(n), defines, onInput(names,cb), container,
// invalidation, state(k,init), setState(k,v), tex(v,opts), table(rows,opts) }. Registered
// through the same taliEnhancers registry as mermaid; idempotent (a `data-tali-ran` guard)
// so it is safe to re-run after every mount.
//
// This file is also the CLIENT HALF of the cell-language registry (`render/client_lang.rs`
// is the server half). `{js}` is one entry in `languages` below; `{glsl}` registers itself
// from `glsl.js` via `window.taliJs.registerLanguage`. Everything outside a language's own
// `setup` — mounting the returned node, publishing `//| name`, registering `//| viewof`,
// the dependency graph, the live region, the error box, teardown — is written once against
// the shared wrapper contract, so a new language is a registration and not surgery.
(function () {
  "use strict";
  var AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;

  /**
   * A client-side cell language: given the cell's source, its scope api and its parsed
   * `//|` options, return the thing that runs it. `run` is called once on mount and again
   * on every scheduled re-run; `dispose` (optional) is called when the cell is torn down.
   * A language never mounts the returned value itself — the shared wrapper does.
   * @typedef {(src: string, api: any, opts: {name: string|null, viewof: string|null,
   *   inputs: string[], kind: string}) => {run: () => any, dispose?: () => void}} TaliLangSetup
   */
  /** @type {Record<string, TaliLangSetup>} */
  var languages = {};
  // Set once `enhance` has completed a pass, so a language registered afterwards knows it
  // has cells to catch up on. See `registerLanguage`.
  var enhancedOnce = false;

  /**
   * @typedef {Object} TaliJsCell
   * @property {string} kind
   * @property {() => Promise<void>} run
   * @property {string | null} defines
   * @property {string[]} inputs
   * @property {HTMLElement} container
   * @property {() => void} dispose
   */
  /**
   * @typedef {Object} TaliJsGraph
   * @property {Record<string, TaliJsCell[]>} consumers
   * @property {TaliJsCell[]} order
   * @property {TaliJsCell[]} cyclic
   */
  /**
   * @typedef {Object} TaliJsRuntime
   * @property {Record<string, any>} scope
   * @property {Record<string, HTMLElement>} inputs
   * @property {Record<string, any>} defines
   * @property {Record<string, Set<() => void>>} listeners
   * @property {TaliJsCell[]} cells
   * @property {Record<string, Record<string, any>>} state
   * @property {Record<string, Promise<void>>} pending
   * @property {TaliJsGraph} [graph]
   */

  // Per-page singleton. Reset implicitly on navigation/reload; on a full re-mount
  // the fresh DOM has no `data-tali-ran` guards, so every cell re-runs and rebuilds.
  /** @returns {TaliJsRuntime} */
  function rt() {
    if (!window.__talijs) {
      window.__talijs = {
        scope: {}, inputs: {}, defines: {}, listeners: {}, cells: [], state: {}, pending: {},
      };
    }
    return /** @type {TaliJsRuntime} */ (window.__talijs);
  }

  /** @param {any} el @returns {any} */
  function readValue(el) {
    if (!el) return undefined;
    if (el.type === "checkbox") return el.checked;
    // A structured control (the draggable `point`) keeps its value as JSON in a hidden
    // field, so it rides the existing string-valued paths — DOM value, URL fragment,
    // `applyInputValue` — and only widens at the READ end. A malformed blob degrades to
    // the raw string rather than throwing inside an input listener.
    if (el.hasAttribute && el.hasAttribute("data-tali-json")) {
      try { return JSON.parse(el.value); } catch (e) { return el.value; }
    }
    if (el.type === "range" || el.type === "number") return el.valueAsNumber;
    if (el.value !== undefined) return el.value;
    return undefined;
  }

  // --- shareable control state in the URL fragment --------------------------
  // `{{< input >}}` controls persist their values to the URL fragment as
  // `name=value&...`, so a link captures the current view and reopening it (or a
  // live-swap) restores every control + its downstream cells. A plain anchor
  // fragment (`#heading`, no `=`) is left untouched, so heading / footnote / xref
  // links still work; the first control change then replaces it with state.
  // Inside a slide deck the fragment is shared: deck.js owns a `/slide/v/frag` position
  // PREFIX and this input state is a `?k=v&...` SUFFIX after it (C-ADD-3). On a normal
  // page the whole fragment is the bare `k=v&...` query. This reconciliation stops the
  // two hash writers (deck nav + control change) from clobbering each other's segment.
  function inDeck() {
    return !!document.querySelector(".tali-deck .tali-slides");
  }
  function parseInputFragment() {
    /** @type {Record<string, string>} */
    var out = {};
    var h = (location.hash || "").replace(/^#/, "");
    var q = inDeck() ? h.split("?")[1] || "" : h;
    if (q.indexOf("=") < 0) return out; // a plain anchor / bare deck position, not control state
    q.split("&").forEach(function (kv) {
      var i = kv.indexOf("=");
      if (i < 0) return;
      try {
        out[decodeURIComponent(kv.slice(0, i))] = decodeURIComponent(kv.slice(i + 1));
      } catch (e) {}
    });
    return out;
  }
  /** @param {any} el @param {string} val */
  function applyInputValue(el, val) {
    if (!el) return;
    if (el.type === "checkbox") el.checked = val === "true" || val === "1" || val === "on";
    else el.value = val;
  }
  function syncInputFragment() {
    /** @type {string[]} */
    var parts = [];
    document.querySelectorAll("[data-tali-input]").forEach(function (el) {
      var n = el.getAttribute("data-tali-input");
      if (!n) return;
      // A structured control round-trips its RAW field text: `readValue` parsed it into an
      // object, and `String({x: 1})` is "[object Object]" — which `applyInputValue` would
      // then faithfully restore on the next load.
      var raw = el.hasAttribute("data-tali-json")
        ? String(/** @type {HTMLInputElement} */ (el).value)
        : String(readValue(el));
      parts.push(encodeURIComponent(n) + "=" + encodeURIComponent(raw));
    });
    var q = parts.join("&");
    var url;
    if (inDeck()) {
      // Preserve deck.js's position prefix; write control state as the `?`-suffix (C-ADD-3).
      var pos = (location.hash || "").replace(/^#/, "").split("?")[0];
      url = "#" + pos + (q ? "?" + q : "");
    } else {
      url = "#" + q;
    }
    // replaceState: change the URL without scrolling or spamming history on every tick.
    try {
      history.replaceState(null, "", url);
    } catch (e) {
      location.hash = url.replace(/^#/, "");
    }
  }

  /** @param {TaliJsRuntime} r @param {string} name @param {HTMLElement | null} el */
  function registerInput(r, name, el) {
    if (!el) return;
    r.inputs[name] = el;
    el.addEventListener("input", function () {
      // Re-run the transitive-downstream closure of this input, in dependency order
      // (the cells that consume `name`, then whatever consumes their derived names).
      // Parked so the `animate` pump can await this exact pass; see `scheduleFrom`.
      r.pending[name] = scheduleFrom(r, name);
      // Still fire any callbacks registered manually via the public `tali.onInput` API.
      var set = r.listeners[name];
      if (set) set.forEach(function (fn) { fn(); });
    });
  }

  // Ingest `<script type="tali-define">` blobs (the Python ojs_define bridge): set
  // the named values, then re-run every non-input cell — a define can land after
  // the cells first ran (live preview executes Python after the page mounts).
  function bindDefines() {
    var r = rt();
    var changed = false;
    document.querySelectorAll('script[type="tali-define"]:not([data-tali-bound])').forEach(function (s) {
      s.setAttribute("data-tali-bound", "1");
      try {
        var obj = JSON.parse(s.textContent || "{}");
        // The Python bridge emits Observable's `{contents:[{name,value}]}` shape;
        // also accept a flat `{name: value}` map.
        var pairs = /** @type {Array<{ name: string, value: any }>} */ (
          Array.isArray(obj.contents)
            ? obj.contents
            : Object.keys(obj).map(function (k) { return { name: k, value: obj[k] }; })
        );
        pairs.forEach(function (p) { r.defines[p.name] = p.value; });
        changed = true;
      } catch (e) {
        console.error("tali-js: malformed define blob", e);
      }
    });
    // Defines usually land once (cold load) or on kernel restart; re-run every
    // cell (sequentially, document order) so inputs whose range depends on a define
    // (e.g. a slider sized by history.length) and name-helpers reading defines
    // rebuild in dependency order.
    if (changed) runSequentially(r.cells);
  }

  // --- rich output helpers (item 157) ---------------------------------------
  // `tali.tex(v)` typesets a NUMBER, VECTOR or MATRIX; `tali.table(rows)` renders an array
  // of records/rows. Between them they close the gap against Jupyter's rich-display MIME
  // protocol for the two shapes a scientific cell actually returns, over the existing
  // DOM-return contract and with no new machinery.
  //
  // **Why this is not KaTeX running in the browser.** Only KaTeX's CSS + fonts are bundled
  // (23 KB + faces), never its ~280 KB parser, and a `{js}` page already carries ~490 KB
  // of d3 + Plot. But the grammar here is CLOSED — a number, a 1-D array, a 2-D array,
  // with an optional `label` left-hand side — so it needs no parser: the GLYPHS come from
  // KaTeX's own fonts (the same faces `$...$` uses, so on a page with prose math the two
  // match exactly) and the bracket + grid LAYOUT is ours in `base.css`. `.tali-math`'s
  // stack falls back to a serif, so a page with no prose math — hence no KaTeX sheet —
  // degrades to Times rather than to a broken box, and there is nothing on such a page for
  // it to look inconsistent WITH. That is why no asset gate had to learn about this.
  //
  // It is not a TeX renderer and must not grow into one: real math is `$...$` in prose.

  /** @param {number} x @param {number} [digits] */
  function fmtNum(x, digits) {
    if (!isFinite(x)) return x !== x ? "NaN" : (x > 0 ? "∞" : "−∞");
    var d = digits === undefined ? 3 : digits;
    if (Number.isInteger(x) && Math.abs(x) < 1e6) return String(x).replace("-", "−");
    var mag = Math.abs(x);
    var s = mag !== 0 && (mag < 1e-4 || mag >= 1e6) ? x.toExponential(d) : x.toFixed(d);
    return s.replace("-", "−"); // U+2212 MINUS SIGN: a hyphen is not a minus
  }

  /** Write a scalar into `el`, lifting any `e±NN` into a real superscript.
   * @param {HTMLElement} el @param {any} x @param {number} [digits] */
  function appendScalar(el, x, digits) {
    if (typeof x !== "number") { el.textContent = String(x); return; }
    var s = fmtNum(x, digits);
    var m = /^(.*)e([+−-]?\d+)$/.exec(s);
    if (!m) { el.textContent = s; return; }
    el.textContent = m[1] + " × 10";
    var sup = document.createElement("sup");
    sup.textContent = m[2].replace("+", "").replace("-", "−");
    el.appendChild(sup);
  }

  /** A value's matrix rows, or null when it is a scalar. @param {any} v */
  function asRows(v) {
    if (!Array.isArray(v)) return null;
    if (v.length && Array.isArray(v[0])) return v;
    return [v]; // a 1-D array reads as a row vector
  }

  /** @param {any} v @param {{label?: string, digits?: number}} [opts] */
  function texValue(v, opts) {
    var o = opts || {};
    var root = document.createElement("span");
    root.className = "tali-math";
    if (o.label) {
      var lhs = document.createElement("span");
      lhs.className = "tali-math-var";
      lhs.textContent = o.label;
      var rel = document.createElement("span");
      rel.className = "tali-math-rel";
      rel.textContent = "=";
      root.appendChild(lhs);
      root.appendChild(rel);
    }
    var rows = asRows(v);
    if (!rows) {
      var scalar = document.createElement("span");
      scalar.className = "tali-math-num";
      appendScalar(scalar, v, o.digits);
      root.appendChild(scalar);
      return root;
    }
    var wrap = document.createElement("span");
    wrap.className = "tali-math-matrix";
    var grid = document.createElement("span");
    grid.className = "tali-math-grid";
    var width = rows.reduce(function (w, r2) { return Math.max(w, r2.length); }, 0);
    grid.style.gridTemplateColumns = "repeat(" + width + ", auto)";
    rows.forEach(function (row) {
      for (var i = 0; i < width; i++) {
        var c = document.createElement("span");
        c.className = "tali-math-num";
        appendScalar(c, row[i] === undefined ? "" : row[i], o.digits);
        grid.appendChild(c);
      }
    });
    var l = document.createElement("span");
    l.className = "tali-math-delim";
    var r2 = document.createElement("span");
    r2.className = "tali-math-delim";
    wrap.appendChild(l);
    wrap.appendChild(grid);
    wrap.appendChild(r2);
    root.appendChild(wrap);
    return root;
  }

  /** The column keys for a row list, in first-seen order. @param {any[]} data */
  function inferColumns(data) {
    var first = data[0];
    if (Array.isArray(first)) {
      var width = data.reduce(function (w, r) { return Math.max(w, (r || []).length); }, 0);
      return Array.from({ length: width }, function (_, i) { return i; });
    }
    if (first && typeof first === "object") {
      /** @type {string[]} */
      var keys = [];
      data.forEach(function (row) {
        if (row && typeof row === "object") {
          Object.keys(row).forEach(function (k) { if (keys.indexOf(k) < 0) keys.push(k); });
        }
      });
      return keys;
    }
    return ["value"];
  }

  /** @param {any} rows @param {{columns?: any[], limit?: number, digits?: number}} [opts] */
  function miniTable(rows, opts) {
    var o = opts || {};
    var data = Array.isArray(rows) ? rows : [rows];
    var limit = o.limit === undefined ? 20 : o.limit;
    var cols = o.columns || inferColumns(data);
    var structured = data.length > 0 && data[0] !== null && typeof data[0] === "object";
    var wrap = document.createElement("div");
    wrap.className = "tali-mini-table";
    var table = document.createElement("table");
    var thead = document.createElement("thead");
    var hrow = document.createElement("tr");
    cols.forEach(function (c) {
      var th = document.createElement("th");
      th.scope = "col";
      th.textContent = typeof c === "number" ? String(c + 1) : String(c);
      hrow.appendChild(th);
    });
    thead.appendChild(hrow);
    table.appendChild(thead);
    var tbody = document.createElement("tbody");
    data.slice(0, limit).forEach(function (row) {
      var tr = document.createElement("tr");
      cols.forEach(function (c) {
        var td = document.createElement("td");
        var v = structured ? (row || {})[c] : row;
        if (typeof v === "number") {
          td.className = "tali-mini-num";
          appendScalar(td, v, o.digits);
        } else {
          td.textContent = v === undefined || v === null ? "" : String(v);
        }
        tr.appendChild(td);
      });
      tbody.appendChild(tr);
    });
    table.appendChild(tbody);
    // Never silently truncate: a table that shows 20 of 5,000 rows and says so is honest;
    // one that just stops is a lie the reader cannot see.
    if (data.length > limit) {
      var cap = document.createElement("caption");
      cap.textContent =
        "showing " + limit + " of " + data.length + " rows";
      table.appendChild(cap);
    }
    wrap.appendChild(table);
    return wrap;
  }

  /**
   * @param {TaliJsRuntime} r @param {HTMLElement} container
   * @param {() => (Promise<void> | null)} getInv @param {string} cellKey
   */
  function makeApi(r, container, getInv, cellKey) {
    return {
      /** @param {string} n */
      get: function (n) { return r.scope[n]; },
      /** @param {string} n @param {any} v */
      set: function (n, v) { r.scope[n] = v; },
      /**
       * Publish a value that arrived AFTER this cell's `run()` resolved, and re-run the
       * cells that consume it. `set` alone writes the scope and schedules nothing, which is
       * correct for a synchronous cell — the wrapper publishes its return value and the
       * scheduler orders everything. A language whose value is genuinely asynchronous
       * (`{pyodide}`: boot the runtime, then execute) has no such moment.
       *
       * Deliberately NOT general mutable dataflow, and the limit is the design: this is
       * reachable only from a LANGUAGE's `setup`, never from author cell source, so no
       * `{js}` cell can start a cascade with it. The reactive-VM trap this project has
       * refused three times stays refused.
       * @param {string} n @param {any} v @returns {Promise<void>}
       */
      publish: function (n, v) { r.scope[n] = v; return scheduleFrom(r, n); },
      /** @param {string} n */
      value: function (n) {
        return r.inputs[n] ? readValue(r.inputs[n]) : r.defines[n];
      },
      defines: r.defines,
      /** @param {string | string[]} names @param {() => void} cb */
      onInput: function (names, cb) {
        (Array.isArray(names) ? names : [names]).forEach(function (n) {
          (r.listeners[n] = r.listeners[n] || new Set()).add(cb);
        });
      },
      container: container,
      get invalidation() { return getInv(); },
      // --- cross-re-run state (item 156) -------------------------------------
      // A cell body re-runs from scratch on every scheduled pass, which is what keeps
      // this a scheduler and not a reactive VM — but it also means an ITERATIVE demo (an
      // EM sweep, a gradient-descent trace) had nowhere to keep the thing it is
      // iterating. `invalidation` could only tear down. These two formalize the gap.
      //
      // Deliberately NOT general mutable dataflow, and the three limits are the design:
      // (1) the store is **per cell** — two cells using "theta" do not collide, and
      //     sharing a value across cells stays `//| name:` + `tali.get`, which the graph
      //     can see and order; a store both cells wrote would be an invisible edge.
      // (2) writing it **schedules nothing** — no downstream cell re-runs because state
      //     changed, so no write can start a cascade.
      // (3) its lifetime is the cell's DOM lifetime. The key is the container id, which
      //     is derived from the block's content hash: editing the cell mints a new id AND
      //     disposes the old cell, so state clears on edit with nothing to remember to
      //     clear. A re-run keeps the id, so the accumulation survives exactly one frame
      //     to the next.
      /** @param {string} k @param {any} [initial] */
      state: function (k, initial) {
        var bucket = r.state[cellKey] || (r.state[cellKey] = {});
        if (!Object.prototype.hasOwnProperty.call(bucket, k)) bucket[k] = initial;
        return bucket[k];
      },
      /** @param {string} k @param {any} v */
      setState: function (k, v) {
        (r.state[cellKey] || (r.state[cellKey] = {}))[k] = v;
        return v;
      },
      // --- rich output helpers (item 157) ------------------------------------
      tex: texValue,
      table: miniTable,
    };
  }

  // AP7-2. A `//| input:` sink is a region of the document that rewrites itself as a
  // consequence of the reader operating a control somewhere else on the page. Driving a
  // slider from the keyboard changed six output regions on `corpus/reactive/inputs.tmd`
  // with every live region on the page empty and no `.tali-js-out` carrying `aria-live`
  // (7 of 7): a screen-reader user heard the slider value and was told nothing about the
  // document that changed under it. This is the explorable-explanation feature, the most
  // web-native thing the tool does, and it was silent.
  //
  // Marked from the FIRST run's output, so the region is already live before the reader
  // can change anything, and only when that output is TEXT. A chart is the common sink and
  // it has nothing useful to speak: re-reading an SVG's tick labels on every arrow-key tick
  // is worse than silence, and a region that cries wolf gets turned off. (Measured on built
  // `corpus/descent`: one of its three sink regions carries Plot's injected stylesheet as
  // its text, so marking every sink live would announce raw CSS.) Text sinks are exactly
  // the ones whose new content IS the answer the reader is looking for, so they are
  // `atomic` — "k doubled (transitively) = 16" reads as one sentence, not as a diff.
  //
  // Idempotent, and never downgrades: a cell that painted text once keeps its live region
  // even if a later run is empty (an empty region announces nothing anyway).
  /** @param {Element | null} container */
  function markLiveIfTextual(container) {
    if (!container || container.getAttribute("aria-live")) return;
    if (container.querySelector("svg, canvas, img, video, iframe")) return;
    if (!(container.textContent || "").trim()) return;
    container.setAttribute("aria-live", "polite");
    container.setAttribute("aria-atomic", "true");
  }

  // Render a failure into the cell's output container. In the live preview (client.js
  // defines taliOpenPageSource) show the full stack so the author can debug; in
  // built/published output degrade to a terse themed message so a reader never sees a raw
  // `TypeError ... at <anonymous>` leak. The full error always reaches console.error.
  /** @param {HTMLElement} container @param {any} e */
  function showCellError(container, e) {
    var pre = document.createElement("pre");
    pre.className = "tali-js-error";
    pre.textContent = (typeof window.taliOpenPageSource === "function")
      ? String((e && e.stack) || e)
      : "This interactive element couldn't load.";
    container.replaceChildren(pre);
  }

  /** @param {HTMLElement} script */
  function setupCell(script) {
    var r = rt();
    // const so the null-guard below survives into the async run()/dispose closures.
    const container = document.getElementById(script.getAttribute("data-target") || "");
    if (!container) return;
    var setup = languages[script.getAttribute("type") || ""];
    if (!setup) return;
    var src = script.textContent || "";
    var name = script.getAttribute("data-name") || null;
    var viewof = script.getAttribute("data-viewof") || null;
    var inputs = (script.getAttribute("data-inputs") || "")
      .split(",").map(function (s) { return s.trim(); }).filter(Boolean);
    var kind = viewof ? "input" : (inputs.length ? "sink" : "once");

    // Per-run invalidation: resolve the prior run's promise before re-running, so
    // cells can tear down Three.js renderers / RAF loops / listeners on re-run.
    // The same `resolveInv` is also fired by the cell's `dispose()` (below) when its
    // block is edited away / unmounted, so DOM removal triggers author teardown too
    // — not only a re-run does. Without that, editing a `{js}`/Three.js cell (which
    // changes its content-hash block id, so the client replaces the node) would
    // detach the old renderer with its `invalidation.then(...)` cleanup never run,
    // leaking a WebGL context + RAF loop on every edit.
    var resolveInv = /** @type {((value?: unknown) => void) | null} */ (null);
    var currentInv = /** @type {Promise<any> | null} */ (null);
    function freshInv() {
      if (resolveInv) { resolveInv(); }
      currentInv = new Promise(function (res) { resolveInv = res; });
      return currentInv;
    }
    var api = makeApi(r, container, function () { return currentInv; }, container.id);
    // Hand the source to the registered language. `setup` is where a language does its
    // one-time work — compiling the author's JS into an AsyncFunction, linking a shader
    // program — so it is the first thing that can fail, and a failure here used to escape
    // `enhance`'s forEach and take the rest of the page's cells with it (a `{js}` cell with
    // a SyntaxError never reached the run-time catch below, because the throw was at
    // construction). Now a bad cell shows its own error box and its siblings still mount.
    /** @type {{run: () => any, dispose?: () => void}} */
    var impl;
    try {
      impl = setup(src, api, { name: name, viewof: viewof, inputs: inputs, kind: kind });
    } catch (e) {
      console.error("tali-js cell error:", e);
      showCellError(container, e);
      script.setAttribute("data-tali-done", "1");
      return;
    }

    // `run` is async and AWAITED by the processing loops, so a `//| name:` helper's
    // value is in the shared scope before a later cell reads it via tali.get() — the
    // cross-cell contract OJS got from its module graph, without a reactive engine.
    async function run() {
      freshInv();
      try {
        var node = await impl.run();
        // The returned value is arbitrary author output; `na` reads its duck-typed
        // `.value` / `.querySelector` (an input control, a wrapper, or neither).
        var na = /** @type {any} */ (node);
        if (node instanceof Node) {
          /** @type {HTMLElement} */ (container).replaceChildren(node);
          if (viewof) {
            // the cell may return the control itself or a labeled wrapper around it
            var ctrl = na.value !== undefined ? node
              : (na.querySelector ? na.querySelector("input, select, textarea") : null);
            registerInput(r, viewof, ctrl);
          }
        }
        if (name) {
          r.scope[name] = (node instanceof Node && na.value !== undefined) ? na.value : node;
        }
        if (kind === "sink") { markLiveIfTextual(container); }
      } catch (e) {
        console.error("tali-js cell error:", e);
        showCellError(/** @type {HTMLElement} */ (container), e);
      } finally {
        // A finished run, whether or not it painted. `data-tali-ran` is stamped at
        // registration (before the cell body runs), and a cell may legitimately emit
        // no DOM (a `//| name:` value publisher, an `//| input:` effect), so neither
        // that attribute nor "the output div has children" says the cell is done.
        // This one does, which is what a screenshot harness has to wait on.
        script.setAttribute("data-tali-done", "1");
      }
    }

    // `defines` is the name this cell publishes (a `//| name` value or a `//| viewof`
    // input); `inputs` are the names it consumes. The dependency graph (buildGraph) is
    // built from these, so a `//| input:` sink re-runs through the graph, not a per-input
    // listener — which is what makes transitive chains (n -> squared -> here) work.
    var cell = {
      kind: kind,
      run: run,
      defines: name || viewof,
      inputs: inputs,
      container: container,
      // Resolve the outstanding `invalidation` (running the author's
      // `invalidation.then(() => renderer.dispose())` teardown) so the cell can be
      // dropped, then let the LANGUAGE tear down whatever the author cannot see (a WebGL
      // context, a shader program), and drop this cell's `tali.state` bucket — the
      // "cleared on cell edit" half of item 156's lifecycle. Idempotent: nulls the
      // resolver so a later dispose is a no-op.
      dispose: function () {
        if (resolveInv) { resolveInv(); resolveInv = null; }
        if (impl.dispose) impl.dispose();
        delete r.state[container.id];
      },
    };
    r.cells.push(cell);
    return cell;
  }

  // Tear down + drop every cell whose container is inside (or is) `node`: resolve its
  // `invalidation` so author cleanup runs (renderer.dispose / cancelAnimationFrame /
  // removeEventListener), then splice it out of `r.cells` so the push-only list can't
  // accumulate stale cells across edits. Also unregister any input the cell published,
  // so a re-mount re-registers a live element rather than firing a detached one. Called
  // by the client BEFORE it detaches an outgoing block (Update/Remove).
  /** @param {Node | null} node */
  function teardownIn(node) {
    if (!node || !window.__talijs) return;
    var r = /** @type {TaliJsRuntime} */ (window.__talijs);
    /** @type {TaliJsCell[]} */
    var kept = [];
    r.cells.forEach(function (c) {
      var inside = c.container && (c.container === node || (node.contains && node.contains(c.container)));
      if (inside) {
        try { if (c.dispose) c.dispose(); } catch (e) { console.error("tali-js: cell teardown failed", e); }
        if (c.defines && r.inputs[c.defines] && c.container.contains(r.inputs[c.defines])) {
          delete r.inputs[c.defines];
        }
      } else {
        kept.push(c);
      }
    });
    if (kept.length !== r.cells.length) {
      r.cells = kept;
      delete r.graph; // the dependency graph is stale once cells are removed
    }
  }

  // Resolve EVERY outstanding invalidation and drop the whole runtime, so a
  // `full_render` (which blows away `#tali-root` wholesale) doesn't leak the prior
  // page's WebGL contexts / RAF loops and doesn't re-push duplicate cells onto a
  // never-reset `r.cells`. The next `enhance()` lazily rebuilds a fresh `window.__talijs`.
  function resetRuntime() {
    var r = /** @type {TaliJsRuntime | null} */ (window.__talijs);
    if (!r) return;
    (r.cells || []).forEach(function (c) {
      try { if (c.dispose) c.dispose(); } catch (e) { console.error("tali-js: cell teardown failed", e); }
    });
    window.__talijs = null;
  }

  // `{js}` itself: the author's source becomes an async function over the drawing globals.
  // `num` (the bundled numerics namespace, item 154) sits beside Plot/d3 as one more
  // drawing global — nothing about the graph or the scheduler knows it exists.
  languages["application/tali-js"] = function (src, api) {
    var fn = new AsyncFunction(
      "tali", "Plot", "d3", "num", "container", "invalidation",
      src
    );
    return {
      run: function () {
        return fn(api, window.Plot, window.d3, window.taliNum, api.container, api.invalidation);
      },
    };
  };

  // Public API for the live-preview client (web-client/client.js) and for the other
  // client-side cell languages: two teardown hooks (one for a block about to be
  // replaced/removed, one before a full re-mount) plus the language registry itself.
  window.taliJs = window.taliJs || {};
  window.taliJs.teardown = teardownIn;
  window.taliJs.reset = resetRuntime;
  /**
   * Register a client-side cell language. `mime` must match the `<script type>` its
   * server-side registry entry emits (`render/client_lang.rs`), which is the one place the
   * two halves have to agree. Idempotent by key; a language file that loads twice (preview
   * live-swap) re-registers the same setup rather than doubling anything.
   * @param {string} mime @param {TaliLangSetup} setup
   */
  window.taliJs.registerLanguage = function (mime, setup) {
    languages[mime] = setup;
    // A language file that loads AFTER the first enhance pass — an `include-after-body`
    // extension script, a live-preview swap — would otherwise leave every cell of its
    // language unmounted forever, since nothing re-scans on its own. The
    // `:not([data-tali-ran])` guard makes this catch-up pass idempotent for the languages
    // that already ran. Before the first pass there is nothing to catch up on, and running
    // here would only move the mount earlier than `taliEnhanceCode` intends.
    if (enhancedOnce) enhance(document);
  };

  // Run a list of cells in document order, awaiting each — so `//| name:` outputs
  // are stored before dependent cells run.
  /** @param {TaliJsCell[]} cells */
  async function runSequentially(cells) {
    for (var i = 0; i < cells.length; i++) await cells[i].run();
  }

  // Build (and cache on `r`) the cell dependency graph: a name -> consumers map plus a
  // global topological order via Kahn's algorithm over `producer.defines -> consumer`
  // edges. Cells left over after Kahn's are in a dependency cycle -> diagnosed (and then
  // excluded from scheduling). Rebuilt whenever fresh cells mount.
  /** @param {TaliJsRuntime} r @returns {TaliJsGraph} */
  function buildGraph(r) {
    var cells = r.cells;
    /** @type {Record<string, TaliJsCell[]>} */
    var consumers = {}; // define-name -> [cells listing it in `inputs`]
    cells.forEach(function (c) {
      c.inputs.forEach(function (n) { (consumers[n] = consumers[n] || []).push(c); });
    });
    /** @type {Map<TaliJsCell, number>} */
    var indeg = new Map();
    cells.forEach(function (c) { indeg.set(c, 0); });
    cells.forEach(function (c) {
      if (c.defines) (consumers[c.defines] || []).forEach(function (cc) {
        indeg.set(cc, (indeg.get(cc) || 0) + 1);
      });
    });
    var queue = cells.filter(function (c) { return indeg.get(c) === 0; }); // doc order
    /** @type {TaliJsCell[]} */
    var order = [];
    while (queue.length) {
      var c = /** @type {TaliJsCell} */ (queue.shift());
      order.push(c);
      if (c.defines) (consumers[c.defines] || []).forEach(function (cc) {
        indeg.set(cc, (indeg.get(cc) || 0) - 1);
        if (indeg.get(cc) === 0) queue.push(cc);
      });
    }
    var cyclic = cells.filter(function (c) { return order.indexOf(c) < 0; });
    cyclic.forEach(function (c) {
      console.error("tali-js: dependency cycle involving", c.defines || "(unnamed cell)");
      if (c.container) {
        var pre = document.createElement("pre");
        pre.className = "tali-js-error";
        pre.textContent = "tali-js: dependency cycle involving `" + (c.defines || "this cell") + "`";
        c.container.replaceChildren(pre);
      }
    });
    r.graph = { consumers: consumers, order: order, cyclic: cyclic };
    return r.graph;
  }

  // The cells transitively downstream of a changed name, in topological order. BFS over
  // the consumers map, following each hit cell's own `defines` (so n -> squared -> ...
  // chains are followed). Cyclic cells are excluded (they show their diagnostic instead).
  /** @param {TaliJsRuntime} r @param {string} seed @returns {TaliJsCell[]} */
  function downstreamInOrder(r, seed) {
    var g = r.graph || buildGraph(r);
    /** @type {Set<TaliJsCell>} */
    var hit = new Set();
    var q = [seed];
    while (q.length) {
      var n = /** @type {string} */ (q.shift());
      (g.consumers[n] || []).forEach(function (c) {
        if (!hit.has(c)) { hit.add(c); if (c.defines) q.push(c.defines); }
      });
    }
    return g.order.filter(function (c) { return hit.has(c); });
  }

  // Re-run exactly the closure downstream of `name`, once each, in dependency order — a
  // single controlled pass (NOT cascading listener fires, which would be a reactive VM).
  //
  // Returns the pass's promise, which `registerInput` parks on `r.pending[name]`. Only the
  // `animate` tick reads it, and it is the mechanism that keeps a frame pump honest: the
  // next frame is not scheduled until the previous pass has RESOLVED, so a slow sink slows
  // the animation rather than queueing passes behind it. Every other caller ignores it, so
  // an input change is still fire-and-forget.
  /** @param {TaliJsRuntime} r @param {string} name @returns {Promise<void>} */
  function scheduleFrom(r, name) {
    var cells = downstreamInOrder(r, name);
    return cells.length ? runSequentially(cells) : Promise.resolve();
  }

  // --- the two non-form controls (item 155) ---------------------------------
  // `{{< input type="animate" >}}` and `type="point"` publish through exactly the same
  // `[data-tali-input]` element every other control uses — a hidden `<input>` — and drive
  // it with a plain `input` event. That is the whole design: the fragment sync, the
  // readout, `registerInput`'s single scheduled downstream pass and the URL round-trip are
  // all reused verbatim, so neither control adds an edge the dependency graph cannot see.

  /** A readout string for any control value, including the structured `point`. @param {any} v */
  function formatOut(v) {
    if (v && typeof v === "object" && typeof v.x === "number") {
      return fmtNum(v.x, 2) + ", " + fmtNum(v.y, 2);
    }
    return String(v);
  }

  /** @param {HTMLInputElement} el @param {any} value */
  function publish(el, value) {
    el.value = String(value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  }

  /**
   * The `animate` control: play / pause / step / reset over a monotonic tick.
   *
   * **The named trap, and the two mechanisms that hold it off.** A tick that free-runs is a
   * continuous dataflow loop, which is the reactive VM this project has refused three
   * times. So: (1) frames are paced to `fps`, not to the display's refresh rate, and
   * (2) the next frame is not even *considered* until the previous downstream pass has
   * RESOLVED (`r.pending[name]`, parked by `registerInput`). A sink slower than the frame
   * budget therefore slows the animation; it can never queue passes behind itself. The
   * pump also stops itself when the element leaves the document, so a live-preview swap
   * mid-play cannot leave an orphan rAF running against a detached input.
   * @param {TaliJsRuntime} r @param {HTMLInputElement} el @param {string} name
   */
  function bindAnimate(r, el, name) {
    var wrap = el.closest(".tali-input");
    if (!wrap) return;
    /** @param {string} k @param {number} d */
    var num = function (k, d) {
      var v = parseFloat(el.getAttribute(k) || "");
      return isFinite(v) ? v : d;
    };
    var from = num("data-min", 0);
    var to = el.hasAttribute("data-max") ? num("data-max", NaN) : NaN;
    var step = num("data-step", 1) || 1;
    var fps = Math.max(1, num("data-fps", 8));
    var playing = false;
    var last = 0;
    var raf = 0;
    var play = wrap.querySelector('[data-tali-animate="play"]');

    function advance() {
      var next = (el.valueAsNumber || 0) + step;
      // A bounded tick LOOPS rather than stopping: these demos are sweeps, and a sweep
      // that halts on its last frame reads as a hang. Unbounded (`max` unset) counts on.
      if (isFinite(to) && next > to) next = from;
      publish(el, next);
    }

    /** @param {boolean} on */
    function setPlaying(on) {
      playing = on;
      if (play) {
        play.setAttribute("aria-pressed", on ? "true" : "false");
        play.setAttribute("aria-label", on ? "Pause" : "Play");
        play.textContent = on ? "⏸" : "▶";
      }
      if (on) { last = 0; raf = requestAnimationFrame(frame); }
      else if (raf) { cancelAnimationFrame(raf); raf = 0; }
    }

    /** @param {number} now */
    function frame(now) {
      if (!playing) return;
      if (!el.isConnected) { setPlaying(false); return; } // swapped out mid-play
      if (!last || now - last >= 1000 / fps) {
        last = now;
        advance();
        // Await THIS tick's downstream pass before asking for another frame.
        Promise.resolve(r.pending[name]).then(function () {
          if (playing) raf = requestAnimationFrame(frame);
        });
        return;
      }
      raf = requestAnimationFrame(frame);
    }

    wrap.querySelectorAll("[data-tali-animate]").forEach(function (b) {
      b.addEventListener("click", function () {
        var act = b.getAttribute("data-tali-animate");
        if (act === "play") setPlaying(!playing);
        else if (act === "step") { setPlaying(false); advance(); }
        else if (act === "reset") { setPlaying(false); publish(el, from); }
      });
    });
  }

  /**
   * The draggable `point` control: pointer drag/click on a square pad, plus arrow keys.
   * Publishes `{x, y}` as JSON on the hidden field (`readValue` parses it back), with **y
   * increasing upward** — these pads place data points on a plot, and a pad whose y ran
   * downward would silently mirror every demo built on it.
   *
   * Accessibility: the pad is the operable element (`tabindex="0"`), not the hidden field,
   * and arrow keys move by `step`. There is no ARIA pattern for a 2-D value — `slider` is
   * one-dimensional — so the pad takes `role="application"` (which routes arrow keys to it
   * rather than to the reader's browse mode) with the label, and the sibling readout is the
   * live region that speaks the new coordinates.
   * @param {HTMLInputElement} el
   */
  function bindPointPad(el) {
    var wrap = el.closest(".tali-input");
    // `const` so the null-guard below survives into the pointer/key closures.
    const pad = wrap && wrap.querySelector(".tali-point-pad");
    if (!pad) return;
    var dot = pad.querySelector(".tali-point-dot");
    /** @param {string} k @param {number} d */
    var attr = function (k, d) {
      var v = parseFloat(pad.getAttribute(k) || "");
      return isFinite(v) ? v : d;
    };
    var lo = attr("data-min", 0);
    var hi = attr("data-max", 1);
    var step = attr("data-step", (hi - lo) / 50);
    var span = hi - lo || 1;
    /** @param {number} v */
    var clamp = function (v) { return Math.min(hi, Math.max(lo, v)); };

    function current() {
      var v = readValue(el);
      return (v && typeof v === "object") ? v : { x: (lo + hi) / 2, y: (lo + hi) / 2 };
    }
    function paint() {
      var p = current();
      if (dot instanceof HTMLElement) {
        dot.style.left = ((p.x - lo) / span) * 100 + "%";
        // Screen y grows downward; the published value grows upward.
        dot.style.top = (1 - (p.y - lo) / span) * 100 + "%";
      }
    }
    /** @param {number} x @param {number} y */
    function emit(x, y) {
      publish(el, JSON.stringify({ x: clamp(x), y: clamp(y) }));
      paint();
    }
    // A function EXPRESSION, not a declaration: a declaration is hoisted above the
    // `if (!pad) return` guard, so the narrowing that guard performs would not apply here.
    /** @param {PointerEvent} e */
    const fromPointer = function (e) {
      var b = pad.getBoundingClientRect();
      if (!b.width || !b.height) return;
      emit(
        lo + ((e.clientX - b.left) / b.width) * span,
        lo + (1 - (e.clientY - b.top) / b.height) * span
      );
    };
    pad.addEventListener("pointerdown", function (e) {
      var pe = /** @type {PointerEvent} */ (e);
      // Pointer capture, like the `descent` corpus doc's drag: the drag keeps tracking
      // after the pointer leaves the pad, and never fights the page's scroll.
      /** @type {any} */ (pad).setPointerCapture(pe.pointerId);
      fromPointer(pe);
      e.preventDefault();
    });
    pad.addEventListener("pointermove", function (e) {
      var pe = /** @type {PointerEvent} */ (e);
      if (/** @type {any} */ (pad).hasPointerCapture(pe.pointerId)) fromPointer(pe);
    });
    pad.addEventListener("keydown", function (e) {
      var ke = /** @type {KeyboardEvent} */ (e);
      /** @type {Record<string, number[]>} */
      var steps = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, 1], ArrowDown: [0, -1] };
      var d = steps[ke.key];
      if (!d) return;
      var p = current();
      emit(p.x + d[0] * step, p.y + d[1] * step);
      e.preventDefault();
    });
    paint();
  }

  /** @param {ParentNode | null} [root] */
  function enhance(root) {
    bindDefines(); // ingest any define blobs already present before running cells
    var r = rt();
    // Register declarative `{{< input >}}` controls (static HTML tagged data-tali-input) as
    // named reactive inputs, BEFORE cells run so their value is available on first run.
    // Reuses the same registerInput path as `//| viewof` cells; the change event fires the
    // existing scheduleFrom (transitive-downstream re-run). Live-swap re-registers via the
    // :not(...) guard. A sibling [data-tali-out] (the slider readout) tracks the value.
    var frag = parseInputFragment();
    /** @type {NodeListOf<HTMLElement>} */ (
      (root || document).querySelectorAll("[data-tali-input]:not([data-tali-input-bound])")
    ).forEach(function (el) {
        el.setAttribute("data-tali-input-bound", "1");
        var name = el.getAttribute("data-tali-input");
        if (!name) return;
        // Hydrate from the URL fragment BEFORE cells run, so a shared link restores the
        // control (and its downstream cells) on first paint.
        if (Object.prototype.hasOwnProperty.call(frag, name)) applyInputValue(el, frag[name]);
        registerInput(r, name, el);
        // Persist to the fragment on every change (shareable/deep-linkable state).
        el.addEventListener("input", syncInputFragment);
        // Behaviour for the two controls that are not a plain form field: the `animate`
        // frame pump and the draggable `point` pad (item 155). Both drive this SAME
        // element through the same `input` event, so everything below — the fragment
        // sync, the readout, `registerInput`'s scheduled pass — is untouched by them.
        if (el.hasAttribute("data-tali-tick")) bindAnimate(r, /** @type {any} */ (el), name);
        if (el.hasAttribute("data-tali-json")) bindPointPad(/** @type {any} */ (el));
        // const so the null-guard survives into the `upd` input closure.
        const out = el.parentNode && el.parentNode.querySelector("[data-tali-out]");
        if (out) {
          var upd = function () { out.textContent = formatOut(readValue(el)); };
          el.addEventListener("input", upd);
          upd();
        }
      });
    /** @type {TaliJsCell[]} */
    var fresh = [];
    // Every registered client-side language in ONE document-order pass, so a page mixing
    // `{js}` and `{glsl}` still mounts its cells in the order they were authored (which is
    // the producer-before-consumer convention the initial run relies on). Selecting per
    // language would group by language instead, and a `{glsl}` cell reading a `//| name:`
    // published by a later-selected `{js}` cell would read undefined on first paint.
    var selector = Object.keys(languages)
      .map(function (m) { return 'script[type="' + m + '"]:not([data-tali-ran])'; })
      .join(",");
    /** @type {NodeListOf<HTMLElement>} */ (
      (root || document).querySelectorAll(selector)
    ).forEach(function (s) {
      s.setAttribute("data-tali-ran", "1");
      var c = setupCell(s);
      if (c) fresh.push(c);
    });
    if (fresh.length) buildGraph(r); // (re)derive the graph + diagnose cycles
    // Initial run in document order (the authoring convention is producer-before-
    // consumer); cyclic cells are left showing their diagnostic rather than run.
    runSequentially(fresh.filter(function (c) {
      return !r.graph || r.graph.cyclic.indexOf(c) < 0;
    }));
    enhancedOnce = true;
  }

  if (window.taliEnhancers && window.taliEnhancers.register) {
    window.taliEnhancers.register(enhance);
  } else {
    document.addEventListener("DOMContentLoaded", function () { enhance(document); });
  }
})();
