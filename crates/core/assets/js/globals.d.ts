// Dev-only ambient types so `tsc --checkJs` can type-check the bundled browser
// scripts under `assets/js/` (the code-enhance/ fragments, deck.js, tali-js.js,
// mermaid.js, scrolly/tabset/walkthrough). NOT shipped or embedded — the server
// `include_str!`s the .js sources verbatim; this file only teaches the checker
// about the globals these scripts share with each other, with the web-client
// (client.js) and with the server-injected inline head scripts.
//
// The jsconfig also pulls in `web-client/globals.d.ts`, whose `interface Window`
// declarations MERGE with the ones below (TypeScript declaration merging), so the
// members those scripts already share (taliEnhancers, taliFocusTrap, TaliesinDeck,
// taliJs, the theme API, the search index, …) are not repeated here — only the
// globals authored under `assets/js/` that web-client does not itself reference.

interface Window {
  // --- code-enhance registry aliases + reader surfaces -----------------------
  /** Reader menu controller (code-enhance/13-reader-menu.js): opens/closes the
   *  reading-tools sheet and lets other fragments dock a section into it. */
  taliReaderMenu?: {
    /** Dock a section (its content node, an onOpen sync hook) into the sheet. The menu
     *  holds exactly one section (Theme) today, so there is no title and no
     *  show/hide handle — see 13-reader-menu.js. */
    addSection: (node: Element, onOpen?: () => void) => void;
    open: () => void;
    close: () => void;
    toggle: () => void;
  };

  // --- {js} reactive runtime (tali-js.js) -------------------------------------
  /** Internal per-page state bag for the `{js}` runtime (cell registry, teardown
   *  handles, observers). Private to tali-js.js; typed loosely. */
  __talijs?: any;
  /** Vendored Observable Plot global (plot.umd.min.js), available to `{js}` cells. */
  Plot?: any;
  /** Vendored d3 global (d3.min.js), available to `{js}` cells. */
  d3?: any;

  // --- deck engine (deck.js) -------------------------------------------------
  /** The deck is rendered inside an `{{< embed >}}` iframe, not standalone. */
  taliDeckEmbedded?: boolean;
  /** Deck-local theme controls (deck.js), distinct from the page theme API. */
  taliDeckThemeChoice?: () => string;
  taliDeckSetTheme?: (choice: string) => void;
  taliDeckApplyTheme?: (choice: string) => void;
  /** The host page manages the deck's theme (embedded decks follow the parent). */
  taliDeckThemeManaged?: boolean;

  // --- mermaid loader (mermaid.js) -------------------------------------------
  /** Vendored mermaid API, lazy-attached by the loader shim. */
  mermaid?: any;
  /** In-flight guard so the mermaid bundle is fetched/initialised once. */
  __taliMermaidLoading?: boolean;

  // --- install-once guards (each feature fragment sets its own) --------------
  __taliKeyboard?: boolean;
  __taliSkipLink?: boolean;
  __taliReaderPrefs?: boolean;

  // --- algorithm debug mode (debug.js) ----------------------------------------
  /** Public, READ-ONLY accessors over a `::: {.debug name="…"}` block's recorded trace.
   *  `tali.frame(n)` in a `{js}` cell (tali-js.js's `makeApi`) is a thin wrapper over
   *  `current`; there is deliberately no setter here (see the comment beside `frame` in
   *  tali-js.js: a writable frame index reachable from author source would let a cell
   *  drive the very stepper that re-runs it). */
  taliDebug?: {
    /** Every recorded frame for a named block (`[]` before it mounts). */
    frames: (n: string) => any[];
    /** The frame the stepper currently sits on: an empty, frame-shaped stand-in
     *  (never `null`) before it mounts. */
    current: (n: string) => any;
  };
}
