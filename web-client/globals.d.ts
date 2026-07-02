// Dev-only ambient types so `tsc --checkJs` can type-check client.js.
// NOT shipped or embedded — the server only `include_str!`s client.js itself.
// These declare the globals client.js shares with the server-injected inline
// scripts (theme_head, deck client, qmd-js cells) and page flags.

interface Window {
  /** Absolute doc + base-dir (+ site root) paths for click-to-source `vscode://` links. */
  TALIESIN_DOC?: { path: string; baseDir: string; root?: string };
  /** Live page has a table of contents (client rebuilds `#TOC`). */
  TALIESIN_TOC?: boolean;
  /** The body was server-rendered, so skip the first re-mount. */
  TALIESIN_SSR?: boolean;
  /** Document format flag (`"deck"` switches the client into deck mode). */
  TALIESIN_FORMAT?: string;
  /** Per-page websocket path for the multi-page site server. */
  TALIESIN_WS_PATH?: string;

  /** Deck engine API (deck mode only), defined by deck.js; typed loosely. */
  TaliesinDeck?: any;
  /** `{js}` reactive runtime API (defined by qmd-js.js): teardown a removed cell
   *  subtree and reset the whole runtime on a full re-mount, to avoid leaking
   *  WebGL contexts / RAF loops across edits + reconnects. */
  taliJs?: { teardown?: (n: Element) => void; reset?: () => void };
  /** Theme API from the head script (`theme_head`). */
  taliSetTheme?: (mode: string) => void;
  taliGetThemePref?: () => string;
  /** Wires every `[data-qmd-theme-toggle]` button (defined in theme_head). */
  taliWireThemeToggles?: () => void;
  /** Runs all registered enhancers over `root` (the registry runner, code-enhance.js). */
  taliEnhanceCode?: (root: ParentNode | null) => void;
  /** Public extension hook: register `fn(root)` to enhance freshly-mounted DOM. */
  taliEnhancers?: {
    register: (fn: (root: ParentNode) => void) => unknown;
    run: (root: ParentNode | null) => void;
  };
  /** (Re)collects `#TOC` links and runs the scrollspy (shared toc-spy.js). */
  taliInitTocSpy?: () => void;
  /** Per-scroll hook the shared scrollspy calls (preview flashes the mobile label). */
  taliTocScrollHook?: () => void;
}
