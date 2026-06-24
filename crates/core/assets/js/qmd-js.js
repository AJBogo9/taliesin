// Native interactive `{js}` cells — a tiny enhancer that replaces the vendored
// 440 KB Observable runtime. Each `{js}` cell is emitted (render/mod.rs) as a
// `<script type="application/qmd-js">` carrying the author's JS plus a sibling
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
// invalidation }. Registered through the same qmdEnhancers registry as mermaid;
// idempotent (a `data-qmd-ran` guard) so it is safe to re-run after every mount.
(function () {
  "use strict";
  var AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;

  // Per-page singleton. Reset implicitly on navigation/reload; on a full re-mount
  // the fresh DOM has no `data-qmd-ran` guards, so every cell re-runs and rebuilds.
  function rt() {
    if (!window.__qmdjs) {
      window.__qmdjs = { scope: {}, inputs: {}, defines: {}, listeners: {}, cells: [] };
    }
    return window.__qmdjs;
  }

  function readValue(el) {
    if (!el) return undefined;
    if (el.type === "checkbox") return el.checked;
    if (el.type === "range" || el.type === "number") return el.valueAsNumber;
    if (el.value !== undefined) return el.value;
    return undefined;
  }

  function registerInput(r, name, el) {
    if (!el) return;
    r.inputs[name] = el;
    el.addEventListener("input", function () {
      // Re-run the transitive-downstream closure of this input, in dependency order
      // (the cells that consume `name`, then whatever consumes their derived names).
      scheduleFrom(r, name);
      // Still fire any callbacks registered manually via the public `qmd.onInput` API.
      var set = r.listeners[name];
      if (set) set.forEach(function (fn) { fn(); });
    });
  }

  // Ingest `<script type="qmd-define">` blobs (the Python ojs_define bridge): set
  // the named values, then re-run every non-input cell — a define can land after
  // the cells first ran (live preview executes Python after the page mounts).
  function bindDefines() {
    var r = rt();
    var changed = false;
    document.querySelectorAll('script[type="qmd-define"]:not([data-qmd-bound])').forEach(function (s) {
      s.setAttribute("data-qmd-bound", "1");
      try {
        var obj = JSON.parse(s.textContent || "{}");
        // The Python bridge emits Observable's `{contents:[{name,value}]}` shape;
        // also accept a flat `{name: value}` map.
        var pairs = Array.isArray(obj.contents)
          ? obj.contents
          : Object.keys(obj).map(function (k) { return { name: k, value: obj[k] }; });
        pairs.forEach(function (p) { r.defines[p.name] = p.value; });
        changed = true;
      } catch (e) {
        console.error("qmd-js: malformed define blob", e);
      }
    });
    // Defines usually land once (cold load) or on kernel restart; re-run every
    // cell (sequentially, document order) so inputs whose range depends on a define
    // (e.g. a slider sized by history.length) and name-helpers reading defines
    // rebuild in dependency order.
    if (changed) runSequentially(r.cells);
  }

  function makeApi(r, container, getInv) {
    return {
      get: function (n) { return r.scope[n]; },
      set: function (n, v) { r.scope[n] = v; },
      value: function (n) {
        return r.inputs[n] ? readValue(r.inputs[n]) : r.defines[n];
      },
      defines: r.defines,
      onInput: function (names, cb) {
        (Array.isArray(names) ? names : [names]).forEach(function (n) {
          (r.listeners[n] = r.listeners[n] || new Set()).add(cb);
        });
      },
      container: container,
      get invalidation() { return getInv(); },
    };
  }

  function setupCell(script) {
    var r = rt();
    var container = document.getElementById(script.getAttribute("data-target"));
    if (!container) return;
    var src = script.textContent || "";
    var name = script.getAttribute("data-name") || null;
    var viewof = script.getAttribute("data-viewof") || null;
    var inputs = (script.getAttribute("data-inputs") || "")
      .split(",").map(function (s) { return s.trim(); }).filter(Boolean);
    var kind = viewof ? "input" : (inputs.length ? "sink" : "once");

    // Per-run invalidation: resolve the prior run's promise before re-running, so
    // cells can tear down Three.js renderers / RAF loops / listeners on re-run.
    var resolveInv = null;
    var currentInv = null;
    function freshInv() {
      if (resolveInv) { resolveInv(); }
      currentInv = new Promise(function (res) { resolveInv = res; });
      return currentInv;
    }
    var api = makeApi(r, container, function () { return currentInv; });
    var fn = new AsyncFunction(
      "qmd", "Plot", "d3", "container", "invalidation",
      src
    );

    // `run` is async and AWAITED by the processing loops, so a `//| name:` helper's
    // value is in the shared scope before a later cell reads it via qmd.get() — the
    // cross-cell contract OJS got from its module graph, without a reactive engine.
    async function run() {
      freshInv();
      try {
        var node = await fn(api, window.Plot, window.d3, container, currentInv);
        if (node instanceof Node) {
          container.replaceChildren(node);
          if (viewof) {
            // the cell may return the control itself or a labeled wrapper around it
            var ctrl = node.value !== undefined ? node
              : (node.querySelector ? node.querySelector("input, select, textarea") : null);
            registerInput(r, viewof, ctrl);
          }
        }
        if (name) {
          r.scope[name] = (node instanceof Node && node.value !== undefined) ? node.value : node;
        }
      } catch (e) {
        console.error("qmd-js cell error:", e);
        var pre = document.createElement("pre");
        pre.className = "qmd-js-error";
        pre.textContent = String((e && e.stack) || e);
        container.replaceChildren(pre);
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
    };
    r.cells.push(cell);
    return cell;
  }

  // Run a list of cells in document order, awaiting each — so `//| name:` outputs
  // are stored before dependent cells run.
  async function runSequentially(cells) {
    for (var i = 0; i < cells.length; i++) await cells[i].run();
  }

  // Build (and cache on `r`) the cell dependency graph: a name -> consumers map plus a
  // global topological order via Kahn's algorithm over `producer.defines -> consumer`
  // edges. Cells left over after Kahn's are in a dependency cycle -> diagnosed (and then
  // excluded from scheduling). Rebuilt whenever fresh cells mount.
  function buildGraph(r) {
    var cells = r.cells;
    var consumers = {}; // define-name -> [cells listing it in `inputs`]
    cells.forEach(function (c) {
      c.inputs.forEach(function (n) { (consumers[n] = consumers[n] || []).push(c); });
    });
    var indeg = new Map();
    cells.forEach(function (c) { indeg.set(c, 0); });
    cells.forEach(function (c) {
      if (c.defines) (consumers[c.defines] || []).forEach(function (cc) {
        indeg.set(cc, indeg.get(cc) + 1);
      });
    });
    var queue = cells.filter(function (c) { return indeg.get(c) === 0; }); // doc order
    var order = [];
    while (queue.length) {
      var c = queue.shift();
      order.push(c);
      if (c.defines) (consumers[c.defines] || []).forEach(function (cc) {
        indeg.set(cc, indeg.get(cc) - 1);
        if (indeg.get(cc) === 0) queue.push(cc);
      });
    }
    var cyclic = cells.filter(function (c) { return order.indexOf(c) < 0; });
    cyclic.forEach(function (c) {
      console.error("qmd-js: dependency cycle involving", c.defines || "(unnamed cell)");
      if (c.container) {
        var pre = document.createElement("pre");
        pre.className = "qmd-js-error";
        pre.textContent = "qmd-js: dependency cycle involving `" + (c.defines || "this cell") + "`";
        c.container.replaceChildren(pre);
      }
    });
    r.graph = { consumers: consumers, order: order, cyclic: cyclic };
    return r.graph;
  }

  // The cells transitively downstream of a changed name, in topological order. BFS over
  // the consumers map, following each hit cell's own `defines` (so n -> squared -> ...
  // chains are followed). Cyclic cells are excluded (they show their diagnostic instead).
  function downstreamInOrder(r, seed) {
    var g = r.graph || buildGraph(r);
    var hit = new Set();
    var q = [seed];
    while (q.length) {
      var n = q.shift();
      (g.consumers[n] || []).forEach(function (c) {
        if (!hit.has(c)) { hit.add(c); if (c.defines) q.push(c.defines); }
      });
    }
    return g.order.filter(function (c) { return hit.has(c); });
  }

  // Re-run exactly the closure downstream of `name`, once each, in dependency order — a
  // single controlled pass (NOT cascading listener fires, which would be a reactive VM).
  function scheduleFrom(r, name) {
    var cells = downstreamInOrder(r, name);
    if (cells.length) runSequentially(cells);
  }

  function enhance(root) {
    bindDefines(); // ingest any define blobs already present before running cells
    var r = rt();
    var fresh = [];
    (root || document).querySelectorAll(
      'script[type="application/qmd-js"]:not([data-qmd-ran])'
    ).forEach(function (s) {
      s.setAttribute("data-qmd-ran", "1");
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

  if (window.qmdEnhancers && window.qmdEnhancers.register) {
    window.qmdEnhancers.register(enhance);
  } else {
    document.addEventListener("DOMContentLoaded", function () { enhance(document); });
  }
})();
