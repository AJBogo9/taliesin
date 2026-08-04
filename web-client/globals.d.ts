// Dev-only ambient types so `tsc --checkJs` can type-check client.js.
// NOT shipped or embedded — the server only `include_str!`s client.js itself.
// These declare the globals client.js shares with the server-injected inline
// scripts (theme_head, deck client, tali-js cells) and page flags.

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
  /** Client-side cell runtime API (defined by tali-js.js): teardown a removed cell
   *  subtree, reset the whole runtime on a full re-mount (to avoid leaking WebGL
   *  contexts / RAF loops across edits + reconnects), and register a client-side cell
   *  LANGUAGE. `registerLanguage` is the seam `glsl.js`/`pyodide.js` mount on: the
   *  `mime` must match the `<script type>` that language's `render/client_lang.rs`
   *  entry emits. `hooks` is the fourth argument tali-js.js's own `TaliLangSetup`
   *  typedef declares (language-only; never placed on `api`, which author cell
   *  source receives verbatim) — a language that does not need it, like `{glsl}`,
   *  simply declares a shorter `setup` and ignores it, which a JS function assigned
   *  to a longer parameter list is free to do. */
  taliJs?: {
    teardown?: (n: Element) => void;
    reset?: () => void;
    registerLanguage?: (
      mime: string,
      setup: (
        src: string,
        api: any,
        opts: { name: string | null; viewof: string | null; inputs: string[]; kind: string },
        hooks: { publish: (n: string, v: any) => Promise<void> },
      ) => { run: () => any; dispose?: () => void },
    ) => void;
    /** Run `{js}`-cell-shaped source with the same `tali`/`Plot`/`d3`/`num`/
     *  `container`/`invalidation` scope a live cell gets, plus `__at`, without the
     *  shared wrapper's mount/publish/scheduling machinery. The algorithm debugger's
     *  JS capture adapter (debug.js) is the one caller: a `//| trace: true` `{js}`
     *  cell never becomes a live `<script type="application/tali-js">`, so it has no
     *  other way to reach this scope. Returns whatever the source itself returns
     *  (typically a generator), typed loosely for the same reason `api` is. */
    runDebugSource?: (src: string, container: HTMLElement) => any;
  };
  /** The curated numerics namespace `{js}` cells draw with (numerics.js), handed to a
   *  cell body as `num` beside `Plot` and `d3`. Typed loosely on purpose: it is a plain
   *  data-and-functions object, and pinning its shape here would fork the definition. */
  taliNum?: any;
  /** Theme API from the head script (`theme_head`). The reader's *choice* may be
   *  `"auto"`; the resolved *mode* that paints never is. Passing `"auto"` (or any
   *  unrecognized value) to `taliSetTheme` clears the saved choice. */
  taliSetTheme?: (choice: 'auto' | 'light' | 'dark') => void;
  taliGetThemePref?: () => 'light' | 'dark';
  taliGetThemeChoice?: () => 'auto' | 'light' | 'dark';
  /** Wires every `[data-tali-theme-toggle]` button (defined in theme_head). */
  taliWireThemeToggles?: () => void;
  /** Flip light <-> dark (defined in theme_head; ships on every page). The Cmd-K palette's
   *  "Toggle theme" action and the dev-menu button share it. */
  taliToggleTheme?: () => void;
  /** Restart the warm Jupyter kernel (preview client only; the palette's "Restart kernel"
   *  action gates on its presence, so it's hidden in a static build). */
  taliRestartKernel?: () => void;
  /** Open the previewed document's source in the editor (preview client only; the palette's
   *  "Open source in editor" action gates on its presence). */
  taliOpenPageSource?: () => void;
  /** Runs all registered enhancers over `root` (the registry runner, code-enhance.js). */
  taliEnhanceCode?: (root: ParentNode | null) => void;
  /** Public extension hook: register `fn(root)` to enhance freshly-mounted DOM. */
  taliEnhancers?: {
    register: (fn: (root: ParentNode) => void) => unknown;
    run: (root: ParentNode | null) => void;
  };
  /** (Re)collects `#TOC` links and runs the scrollspy (shared toc-spy.js). */
  taliInitTocSpy?: () => void;

  // --- Cmd-K search palette (search.js) ---
  /** Install-once guard so the palette wires its global listeners a single time. */
  taliSearchInstalled?: boolean;
  /** Inlined / lazy-loaded cross-page search index (site + book): raw serialized
   *  entries (i=anchor id, t=title, l=level [0 = a whole-page entry], b=body text,
   *  u=page url, p=page label, c=book chapter number, h=ancestor heading path).
   *  `c` and `h` are absent outside a book / on a top-level heading, and are what let
   *  the palette render the index as the book's outline rather than a flat row list.
   *  A single doc builds its index from the live DOM instead. */
  TALIESIN_SEARCH_INDEX?: Array<{
    i: string;
    t: string;
    l: number;
    b?: string;
    u?: string;
    p?: string;
    c?: number;
    h?: string;
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
  /** Programmatic Cmd-K opener (search.js). */
  taliOpenSearch?: () => void;
}
