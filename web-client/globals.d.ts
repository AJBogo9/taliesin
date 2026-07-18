// Dev-only ambient types so `tsc --checkJs` can type-check client.js.
// NOT shipped or embedded — the server only `include_str!`s client.js itself.
// These declare the globals client.js shares with the server-injected inline
// scripts (theme_head, deck client, qmd-js cells) and page flags.

interface Window {
  /** Absolute doc + base-dir (+ site root) paths for click-to-source `vscode://` links. */
  TALIESIN_DOC?: { path: string; baseDir: string; root?: string };
  /** Live page has a table of contents (client rebuilds `#TOC`). */
  TALIESIN_TOC?: boolean;
  /** The body was server-rendered, so the first re-mount can normally be skipped. */
  TALIESIN_SSR?: boolean;
  /** The render generation the SSR body was built at; compared to the first
   *  `full_render`'s `gen` to detect an SSR body a rebuild made stale. */
  TALIESIN_SSR_GEN?: number;
  /** The server's per-process boot id; a mismatch on a reconnect (restarted server,
   *  whose `gen` counter reset) forces a re-mount instead of a stale-body skip. */
  TALIESIN_BOOT?: number;
  /** Document format flag (`"deck"` switches the client into deck mode). */
  TALIESIN_FORMAT?: string;
  /** Per-page websocket path for the multi-page site server. */
  TALIESIN_WS_PATH?: string;
  /** Draft pages in the previewed project (preview only; the build ships neither this
   *  nor the dev menu). Powers the dev-menu "Drafts" count + click-to-open list.
   *  Root-absolute urls so a link resolves from any page depth. */
  TALIESIN_DRAFTS?: Array<{ url: string; title: string }>;

  /** Deck engine API (deck mode only), defined by deck.js; typed loosely. */
  TaliesinDeck?: any;
  /** `{js}` reactive runtime API (defined by qmd-js.js): teardown a removed cell
   *  subtree and reset the whole runtime on a full re-mount, to avoid leaking
   *  WebGL contexts / RAF loops across edits + reconnects. */
  taliJs?: { teardown?: (n: Element) => void; reset?: () => void };
  /** Theme API from the head script (`theme_head`). The reader's *choice* may be
   *  `"auto"`; the resolved *mode* that paints never is. Passing `"auto"` (or any
   *  unrecognized value) to `taliSetTheme` clears the saved choice. */
  taliSetTheme?: (choice: 'auto' | 'light' | 'dark' | 'sepia') => void;
  taliGetThemePref?: () => 'light' | 'dark' | 'sepia';
  taliGetThemeChoice?: () => 'auto' | 'light' | 'dark' | 'sepia';
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

  // --- Cmd-K search palette (search.js) ---
  /** Install-once guard so the palette wires its global listeners a single time. */
  taliSearchInstalled?: boolean;
  /** Inlined / lazy-loaded cross-page search index (site + book): raw serialized
   *  entries (i=anchor id, t=title, l=level [0 = a whole-page entry], b=body text,
   *  u=page url, p=page label). A single doc builds its index from the live DOM instead. */
  TALIESIN_SEARCH_INDEX?: Array<{
    i: string;
    t: string;
    l: number;
    b?: string;
    u?: string;
    p?: string;
  }>;
  /** URL of the lazy-loaded cross-page index script (a site/book links to it rather
   *  than inlining the full-text index into every page). */
  TALIESIN_SEARCH_URL?: string;
  /** Set by the index loader's onerror so the palette shows a load-failure row
   *  instead of a silently-empty result list. */
  TALIESIN_SEARCH_LOAD_FAILED?: boolean;
  /** This page's own url, so a search hit on the current page scrolls in place
   *  rather than triggering a same-page navigation. */
  TALIESIN_PAGE_URL?: string;
  /** Site root prefix for cross-page search navigation when served under a mount. */
  TALIESIN_SITE_ROOT?: string;
  /** Focus-trap helper (code-enhance/04-focus-trap.js): traps focus inside
   *  `container`, focuses `initial`, and returns a release function. */
  taliFocusTrap?: (container: Element, initial?: Element | null) => () => void;
  /** Programmatic Cmd-K opener (search.js), for the keyboard reader's `/` shortcut. */
  taliOpenSearch?: () => void;
  /** Reader preference: are single-key shortcuts (`f`, `?`, `/`) live?
   *  (code-enhance/01-registry.js). The first-run nudge omits the `?` line when this
   *  is present and returns false, matching the "don't advertise dead keys" rule. */
  taliShortcutsOn?: () => boolean;
}
