// `{pyodide}` cells — Python in the reader's browser, no kernel.
//
// Filled in by Task 5. This skeleton exists so the server-side registration and its client
// counterpart land in the same commit: `every_registered_mime_is_looked_up_by_the_client_runtime`
// asserts that some client file registers each registered mime.
(function () {
  "use strict";

  /**
   * @param {string} src @param {any} api
   * @param {{name: string|null, viewof: string|null, inputs: string[], kind: string}} opts
   */
  function setupPyodide(src, api, opts) {
    var out = document.createElement("div");
    out.className = "tali-pyodide-out";
    // The shared wrapper publishes `node.value` when the node has one (tali-js.js:543).
    // `null` until a real value arrives, so a downstream `{js}` cell always has a defined
    // thing to guard on rather than an undefined name. `HTMLDivElement` carries no
    // `.value` of its own, so this is the same `/** @type {any} */` cast deck.js and
    // tali-js.js already use to hang an ad hoc field off a DOM node.
    /** @type {any} */ (out).value = null;
    return {
      run: function () {
        return out;
      },
    };
  }

  if (window.taliJs && window.taliJs.registerLanguage) {
    window.taliJs.registerLanguage("application/tali-pyodide", setupPyodide);
  }
})();
