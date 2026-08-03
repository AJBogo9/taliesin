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
    /** Dock a section (title, its content node, an onOpen sync hook) into the sheet.
     *  Returns a handle for showing/hiding that section, so a feature can offer its
     *  row only on documents it governs. */
    addSection: (
      title: string,
      node: Element,
      onOpen?: () => void,
    ) => { setVisible: (v: boolean) => void };
    open: () => void;
    close: () => void;
    toggle: () => void;
  };

  // --- reader show/hide code (code-enhance/20-code-visibility.js) --------------
  /** Guard: the code-visibility UI is built once per document. */
  __taliCodeVis?: boolean;
  /** Set by the pre-paint bootstrap (render/theme.rs), NOT by the fragment: the
   *  class has to land before the first frame or every listing renders and vanishes. */
  taliSetCodeHidden?: (hidden: boolean) => void;
  taliGetCodeHidden?: () => boolean;

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
  /** Keyboard entry point for the lightbox (11): open the viewer for a decorated
   *  image / mermaid element. Set once taliInitLightbox has run. */
  __taliLightboxOpen?: (el: Element) => void;
  /** lightbox (11) document-level machinery has been installed (install-once guard). */
  __taliLightbox?: boolean;
  __taliKeyboard?: boolean;
  __taliSkipLink?: boolean;
  __taliReaderPrefs?: boolean;
}
