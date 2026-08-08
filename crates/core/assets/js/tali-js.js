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
// invalidation }. Registered through the same taliEnhancers registry as mermaid;
// idempotent (a `data-tali-ran` guard) so it is safe to re-run after every mount.
//
// `publish` is deliberately absent from that list: it is a language-only hook, passed to a
// language's `setup` as a fourth argument and never placed on the author-facing scope.
//
// This file is also the CLIENT HALF of the cell-language registry (`render/client_lang.rs`
// is the server half). `{js}` is its one entry in `languages` below. Everything outside a
// language's own `setup` — mounting the returned node, publishing `//| name`, registering
// `//| viewof`, the dependency graph, the live region, the error box, teardown — is written
// once against the shared wrapper contract, so a second language is a registration and not
// surgery.
(function () {
  "use strict";
  var AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;

  /**
   * A client-side cell language: given the cell's source, its scope api, its parsed
   * `//|` options and the language-only hooks, return the thing that runs it. `run` is
   * called once on mount and again on every scheduled re-run; `dispose` (optional) is
   * called when the cell is torn down. A language never mounts the returned value itself —
   * the shared wrapper does.
   *
   * `hooks` carries what a LANGUAGE may do and author source may not. It is a separate
   * argument precisely because `api` is passed through to author cell source verbatim.
   * @typedef {(src: string, api: any, opts: {name: string|null, viewof: string|null,
   *   inputs: string[], kind: string}, hooks: {publish: (n: string, v: any) => Promise<void>})
   *   => {run: () => any, dispose?: () => void}} TaliLangSetup
   */
  /** @type {Record<string, TaliLangSetup>} */
  var languages = {};

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
   * @property {TaliJsGraph} [graph]
   */

  // Per-page singleton. Reset implicitly on navigation/reload; on a full re-mount
  // the fresh DOM has no `data-tali-ran` guards, so every cell re-runs and rebuilds.
  /** @returns {TaliJsRuntime} */
  function rt() {
    if (!window.__talijs) {
      window.__talijs = {
        scope: {}, inputs: {}, defines: {}, listeners: {}, cells: [],
      };
    }
    return /** @type {TaliJsRuntime} */ (window.__talijs);
  }

  /** @param {any} el @returns {any} */
  function readValue(el) {
    if (!el) return undefined;
    if (el.type === "checkbox") return el.checked;
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
  function parseInputFragment() {
    /** @type {Record<string, string>} */
    var out = {};
    var h = (location.hash || "").replace(/^#/, "");
    var q = h;
    if (q.indexOf("=") < 0) return out; // a plain anchor, not control state
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
      parts.push(encodeURIComponent(n) + "=" + encodeURIComponent(String(readValue(el))));
    });
    var url = "#" + parts.join("&");
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
      scheduleFrom(r, name);
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

  /**
   * @param {TaliJsRuntime} r @param {HTMLElement} container
   * @param {() => (Promise<void> | null)} getInv
   */
  function makeApi(r, container, getInv) {
    return {
      /** @param {string} n */
      get: function (n) { return r.scope[n]; },
      /** @param {string} n @param {any} v */
      set: function (n, v) { r.scope[n] = v; },
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
    var api = makeApi(r, container, function () { return currentInv; });
    // Hand the source to the registered language. `setup` is where a language does its
    // one-time work — compiling the author's JS into an AsyncFunction, linking a shader
    // program — so it is the first thing that can fail, and a failure here used to escape
    // `enhance`'s forEach and take the rest of the page's cells with it (a `{js}` cell with
    // a SyntaxError never reached the run-time catch below, because the throw was at
    // construction). Now a bad cell shows its own error box and its siblings still mount.
    /** @type {{run: () => any, dispose?: () => void}} */
    var impl;
    try {
      impl = setup(src, api, { name: name, viewof: viewof, inputs: inputs, kind: kind }, {
        // Publish a value that arrived AFTER this cell's `run()` resolved, and re-run the
        // cells that consume it. The shared wrapper publishes a cell's RETURN value and the
        // scheduler orders everything, which is correct for a synchronous language; a
        // language whose value is genuinely asynchronous (one that must boot a runtime
        // before it can execute) has no such moment.
        //
        // A FOURTH ARGUMENT rather than a method on `api`, and that is the whole safety
        // design: `api` is handed verbatim to author cell source as `tali` (see the
        // `application/tali-js` registration below), so anything on it — or on its
        // prototype — is author-reachable. A cell that could publish to a name it also
        // declares as an `//| input:` would create a feedback edge `buildGraph` never saw
        // and never cycle-checked, recursing without a guard: the reactive VM this project
        // has refused three times. Passing the capability to the LANGUAGE instead means
        // author source cannot reach it by any route, including a deliberate prototype
        // walk. A masking shield on `api` was tried first and leaked through
        // `Object.getPrototypeOf(tali).publish`.
        publish: function (n, v) { r.scope[n] = v; return scheduleFrom(r, n); },
      });
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
      // context held by an `import()`ed three.js renderer). Idempotent: nulls the resolver
      // so a later dispose is a no-op.
      dispose: function () {
        if (resolveInv) { resolveInv(); resolveInv = null; }
        if (impl.dispose) impl.dispose();
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
  //
  // Author source receives `api` verbatim as `tali`. That is safe because the one
  // capability author source must not have — `publish`, which SCHEDULES a downstream pass —
  // is not on `api` at all; it is a language-only hook passed as `setup`'s fourth argument.
  languages["application/tali-js"] = function (src, api) {
    var fn = new AsyncFunction("tali", "Plot", "d3", "container", "invalidation", src);
    return {
      run: function () {
        return fn(api, window.Plot, window.d3, api.container, api.invalidation);
      },
    };
  };

  // Public API for the live-preview client (web-client/client.js): two teardown hooks, one
  // for a block about to be replaced/removed and one before a full re-mount.
  window.taliJs = window.taliJs || {};
  window.taliJs.teardown = teardownIn;
  window.taliJs.reset = resetRuntime;

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
  // Returns the pass's promise. Every caller ignores it, so an input change is
  // fire-and-forget; it was awaited by the `animate` tick's frame pump, which was retired
  // on 2026-08-03 along with the `pending` map that parked it.
  /** @param {TaliJsRuntime} r @param {string} name @returns {Promise<void>} */
  function scheduleFrom(r, name) {
    var cells = downstreamInOrder(r, name);
    return cells.length ? runSequentially(cells) : Promise.resolve();
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
        // const so the null-guard survives into the `upd` input closure.
        const out = el.parentNode && el.parentNode.querySelector("[data-tali-out]");
        if (out) {
          var upd = function () { out.textContent = String(readValue(el)); };
          el.addEventListener("input", upd);
          upd();
        }
      });
    /** @type {TaliJsCell[]} */
    var fresh = [];
    // Every registered client-side language in ONE document-order pass, so a page mixing
    // two languages still mounts its cells in the order they were authored (which is the
    // producer-before-consumer convention the initial run relies on). Selecting per
    // language would group by language instead, and a cell reading a `//| name:` published
    // by a later-selected cell of another language would read undefined on first paint.
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
  }

  if (window.taliEnhancers && window.taliEnhancers.register) {
    window.taliEnhancers.register(enhance);
  } else {
    document.addEventListener("DOMContentLoaded", function () { enhance(document); });
  }
})();
