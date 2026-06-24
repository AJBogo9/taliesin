// Dev-only ambient types so `tsc --checkJs` can type-check client.js.
// NOT shipped or embedded — the server only `include_str!`s client.js itself.
// These declare the globals client.js shares with the server-injected inline
// scripts (theme_head, deck client, qmd-js cells) and page flags.

interface Window {
  /** Absolute doc + base-dir (+ site root) paths for click-to-source `vscode://` links. */
  QMD_DOC?: { path: string; baseDir: string; root?: string };
  /** Live page has a table of contents (client rebuilds `#TOC`). */
  QMD_TOC?: boolean;
  /** The body was server-rendered, so skip the first re-mount. */
  QMD_SSR?: boolean;
  /** Document format flag (`"deck"` switches the client into deck mode). */
  QMD_FORMAT?: string;
  /** Per-page websocket path for the multi-page site server. */
  QMD_WS_PATH?: string;

  /** Deck engine API (deck mode only), defined by deck.js; typed loosely. */
  QmdDeck?: any;
  /** Theme API from the head script (`theme_head`). */
  qmdSetTheme?: (mode: string) => void;
  qmdGetThemePref?: () => string;
  /** Wires every `[data-qmd-theme-toggle]` button (defined in theme_head). */
  qmdWireThemeToggles?: () => void;
  /** Runs all registered enhancers over `root` (the registry runner, code-enhance.js). */
  qmdEnhanceCode?: (root: ParentNode | null) => void;
  /** Public extension hook: register `fn(root)` to enhance freshly-mounted DOM. */
  qmdEnhancers?: {
    register: (fn: (root: ParentNode) => void) => unknown;
    run: (root: ParentNode | null) => void;
  };
  /** (Re)collects `#TOC` links and runs the scrollspy (shared toc-spy.js). */
  qmdInitTocSpy?: () => void;
  /** Per-scroll hook the shared scrollspy calls (preview flashes the mobile label). */
  qmdTocScrollHook?: () => void;
}
