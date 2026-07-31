// `{pyodide}` cells — Python in the reader's browser, no kernel.
//
// This file is the whole of the `{pyodide}` language: a REGISTRATION against the seam
// `tali-js.js` exposes (`window.taliJs.registerLanguage`), not a second runtime. Mounting,
// `#| name:` publication, the dependency graph, the error box, teardown and click-to-source
// are the shared wrapper's job and appear nowhere below.
//
// TWO THINGS HERE ARE LOAD-BEARING AND NON-OBVIOUS.
//
// 1. `run()` MUST NOT await the boot. `tali-js.js` runs every freshly-mounted cell in ONE
//    sequential `await` loop, so a cell that blocked on a scroll-triggered download would
//    stall every cell below it on the page — a `{js}` chart further down would stay blank
//    until the reader scrolled here. `run()` therefore returns a placeholder at once and the
//    real value is published later through `hooks.publish`, which re-runs the consumers.
//    `hooks` is `setup`'s FOURTH argument and is language-only: it is deliberately NOT on
//    `api`, because `api` is handed verbatim to `{js}` author source as `tali`. Do not copy
//    it onto `api`, `out`, or anything else author source can reach.
//
// 2. The output node carries the value on `.value`. `tali-js.js` publishes
//    `node.value` when a returned Node has one, so ONE returned node both mounts the display
//    and publishes the value — no wrapper change needed. It is `null` until the first real
//    result, so a downstream `{js}` cell always has a defined thing to guard on.
(function () {
  "use strict";

  /** The build/serve-time index URL, stamped into the head by render/pyodide.rs. */
  function indexUrl() {
    var m = document.querySelector('meta[name="tali-pyodide-index"]');
    return (m && m.getAttribute("content")) || "";
  }

  /** One boot per page, shared by every cell. @type {Promise<any> | null} */
  var booting = null;

  function boot() {
    if (booting) return booting;
    var base = indexUrl();
    if (!base) {
      booting = Promise.reject(
        new Error(
          "pyodide: this page was built as a single self-contained file, which cannot " +
            "carry the 12.9 MB runtime. Rebuild with `--out <dir>` for a working page."
        )
      );
      return booting;
    }
    // A dynamic `import()` in an INLINE script resolves relative to the page, which is what
    // makes a page-relative `_assets/...` index work in a nested book chapter. See the note
    // in render/page.rs about why this runtime stays inline even in External asset mode.
    booting = import(base + "pyodide.mjs")
      .then(function (mod) {
        return mod.loadPyodide({ indexURL: base });
      })
      .then(function (py) {
        return py.loadPackage("numpy").then(function () {
          return py;
        });
      });
    return booting;
  }

  /** Run `cb` once `el` is near the viewport. Returns a disposer.
   * @param {Element} el @param {() => void} cb @returns {() => void} */
  function whenNear(el, cb) {
    if (typeof IntersectionObserver !== "function") {
      cb();
      return function () {};
    }
    // 600px of lead time: the runtime starts fetching while the reader is still reading the
    // paragraph above, so it is usually running by the time the cell is actually on screen.
    var io = new IntersectionObserver(
      function (entries) {
        for (var i = 0; i < entries.length; i++) {
          if (entries[i].isIntersecting) {
            io.disconnect();
            cb();
            return;
          }
        }
      },
      { rootMargin: "600px" }
    );
    io.observe(el);
    return function () { io.disconnect(); };
  }

  /** @param {string} text @param {string} cls */
  function note(text, cls) {
    var p = document.createElement("p");
    p.className = cls;
    p.textContent = text;
    return p;
  }

  /**
   * Turn a failure into the message the reader can act on. A bare ModuleNotFoundError is the
   * ONE predictable failure of vendoring numpy and nothing else, so it does not get to
   * surface as a raw traceback.
   * @param {any} e
   */
  function explain(e) {
    var msg = (e && e.message) || String(e);
    if (msg.indexOf("ModuleNotFoundError") >= 0) {
      return (
        msg +
        "\n\nOnly the Python standard library and numpy are vendored with this page. " +
        "Installing another package would need a network fetch, which Taliesin does not do."
      );
    }
    return msg;
  }

  /**
   * @param {string} src @param {any} api
   * @param {{name: string|null, viewof: string|null, inputs: string[], kind: string}} opts
   * @param {{publish: (n: string, v: any) => Promise<void>}} hooks
   */
  function setupPyodide(src, api, opts, hooks) {
    var out = document.createElement("div");
    out.className = "tali-pyodide-out";
    // `.value` is an own-property assignment on a plain <div>, not a native property; the
    // codebase's idiom for that under `tsc --strict` is an inline `/** @type {any} */` cast
    // (precedent: tali-js.js and deck.js). See note 2 in the header.
    /** @type {any} */ (out).value = null;
    var stop = /** @type {(() => void) | null} */ (null);
    var dead = false;
    var started = false;

    /** @param {Node} node */
    function show(node) {
      if (dead) return;
      out.replaceChildren(node);
    }

    async function execute() {
      if (dead) return;
      show(note("Starting Python…", "tali-pyodide-status"));
      var chunks = /** @type {string[]} */ ([]);
      var result = null;
      try {
        var py = await boot();
        if (dead) return;
        py.setStdout({ batched: function (/** @type {string} */ s) { chunks.push(s); } });
        py.setStderr({ batched: function (/** @type {string} */ s) { chunks.push(s); } });
        result = await py.runPythonAsync(src);
        if (dead) return;

        var frag = document.createDocumentFragment();
        if (chunks.length) {
          var pre = document.createElement("pre");
          pre.className = "tali-pyodide-stdout";
          pre.textContent = chunks.join("");
          frag.appendChild(pre);
        }
        // Rich display first, `repr` second — Jupyter's order, so a `{pyodide}` cell looks
        // like the `{python}` cell beside it.
        if (result && typeof result._repr_html_ === "function") {
          var host = document.createElement("div");
          host.innerHTML = result._repr_html_();
          frag.appendChild(host);
        } else if (result !== undefined && result !== null) {
          var v = document.createElement("pre");
          v.className = "tali-pyodide-value";
          v.textContent = String(result.toString ? result.toString() : result);
          frag.appendChild(v);
        }
        show(frag);

        if (opts.name) {
          var js = result && typeof result.toJs === "function"
            ? result.toJs({ dict_converter: Object.fromEntries })
            : result;
          /** @type {any} */ (out).value = js;
          await hooks.publish(opts.name, js);
        }
      } catch (e) {
        if (dead) return;
        var box = document.createElement("pre");
        box.className = "tali-js-error";
        box.textContent = explain(e);
        show(box);
      } finally {
        // PyProxies are not garbage collected: a chapter re-run twenty times would otherwise
        // leak the WASM heap until the tab dies.
        if (result && typeof result.destroy === "function") {
          try { result.destroy(); } catch (_) { /* already destroyed */ }
        }
      }
    }

    return {
      run: function () {
        if (out.parentNode !== api.container) {
          show(note("Python runs when this scrolls into view.", "tali-pyodide-status"));
        }
        if (!started) {
          started = true;
          stop = whenNear(api.container, function () { execute(); });
        } else {
          // A re-run: an input this cell consumes changed. Fire and FORGET — awaiting here
          // would reintroduce the stall note 1 exists to prevent. Downstream consumers see
          // the previous value on this tick and the fresh one when `hooks.publish` lands.
          execute();
        }
        return out;
      },
      dispose: function () {
        dead = true;
        if (stop) stop();
      },
    };
  }

  if (window.taliJs && window.taliJs.registerLanguage) {
    window.taliJs.registerLanguage("application/tali-pyodide", setupPyodide);
  }
})();
