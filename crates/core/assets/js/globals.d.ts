// Dev-only ambient types so `tsc --checkJs` can type-check the bundled browser
// scripts under `assets/js/` (the code-enhance/ fragments, deck.js, qmd-js.js,
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
  /** Back-compat alias for `taliEnhanceCode` kept during the qmd->tali rename. */
  qmdEnhanceCode?: (root: ParentNode | null) => void;
  /** Back-compat alias for `taliEnhancers` kept during the qmd->tali rename. */
  qmdEnhancers?: Window['taliEnhancers'];
  /** Reader menu controller (code-enhance/13-reader-menu.js): opens/closes the
   *  reading-tools sheet and lets other fragments dock a section into it. */
  taliReaderMenu?: {
    /** Dock a section (title, its content node, an onOpen sync hook) into the sheet. */
    addSection: (title: string, node: Element, onOpen?: () => void) => void;
    open: () => void;
    close: () => void;
    toggle: () => void;
  };

  // --- {js} reactive runtime (qmd-js.js) -------------------------------------
  /** Internal per-page state bag for the `{js}` runtime (cell registry, teardown
   *  handles, observers). Private to qmd-js.js; typed loosely. */
  __talijs?: any;
  /** Back-compat alias for `taliJs` kept during the qmd->tali rename. */
  qmdJs?: Window['taliJs'];
  /** Vendored Observable Plot global (plot.umd.min.js), available to `{js}` cells. */
  Plot?: any;
  /** Vendored d3 global (d3.min.js), available to `{js}` cells. */
  d3?: any;

  // --- deck engine (deck.js) -------------------------------------------------
  /** Back-compat alias for `TaliesinDeck` kept during the qmd->tali rename. */
  QmdDeck?: any;
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
  __qmdMermaidLoading?: boolean;

  // --- link preview / hover cards (code-enhance/12-link-preview.js) ----------
  /** URL of the lazy-loaded cross-page hover-card index. */
  TALIESIN_HOVER_URL?: string;
  /** Inlined hover-card index: a rendered-HTML snippet string keyed by target anchor id. */
  TALIESIN_HOVER_INDEX?: Record<string, string>;

  // --- install-once guards (each feature fragment sets its own) --------------
  /** anchor-links (02) shared aria-live region element (announces "Link copied"). */
  __qmdAnchorLive?: HTMLElement;
  /** Keyboard entry point for the lightbox (11): open the viewer for a decorated
   *  image / mermaid element. Set once taliInitLightbox has run. */
  __qmdLightboxOpen?: (el: Element) => void;
  /** lightbox (11) document-level machinery has been installed (install-once guard). */
  __qmdLightbox?: boolean;
  __qmdLinkPreview?: boolean;
  __qmdKeyboard?: boolean;
  __qmdFocus?: boolean;
  __qmdSkipLink?: boolean;
  __qmdReaderPrefs?: boolean;
  __qmdProgress?: boolean;
}
