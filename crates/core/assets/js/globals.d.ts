// Dev-only ambient types so `tsc --checkJs` can type-check the bundled browser
// scripts under `assets/js/` (the code-enhance/ fragments, tali-js.js,
// mermaid.js). NOT shipped or embedded — the server
// `include_str!`s the .js sources verbatim; this file only teaches the checker
// about the globals these scripts share with each other, with the web-client
// (client.js) and with the server-injected inline head scripts.
//
// The jsconfig also pulls in `web-client/globals.d.ts`, whose `interface Window`
// declarations MERGE with the ones below (TypeScript declaration merging), so the
// members those scripts already share (taliEnhancers, taliFocusTrap,
// taliJs, the theme API, the search index, …) are not repeated here — only the
// globals authored under `assets/js/` that web-client does not itself reference.

interface Window {
  // --- {js} reactive runtime (tali-js.js) -------------------------------------
  /** Internal per-page state bag for the `{js}` runtime (cell registry, teardown
   *  handles, observers). Private to tali-js.js; typed loosely. */
  __talijs?: any;
  /** Vendored Observable Plot global (plot.umd.min.js), available to `{js}` cells. */
  Plot?: any;
  /** Vendored d3 global (d3.min.js), available to `{js}` cells. */
  d3?: any;


  // --- mermaid loader (mermaid.js) -------------------------------------------
  /** Vendored mermaid API, lazy-attached by the loader shim. */
  mermaid?: any;
  /** In-flight guard so the mermaid bundle is fetched/initialised once. */
  __taliMermaidLoading?: boolean;

  // --- install-once guards (each feature fragment sets its own) --------------
  __taliKeyboard?: boolean;
  __taliSkipLink?: boolean;
  __taliReaderPrefs?: boolean;
}
