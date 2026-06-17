// Dev-only ambient types so `tsc --checkJs` can type-check client.js.
// NOT shipped or embedded — the server only `include_str!`s client.js itself.
// These declare the globals client.js shares with the server-injected inline
// scripts (theme_head, OJS init, reveal client) and page flags.

interface Window {
  /** Absolute doc + base-dir (+ site root) paths for click-to-source `vscode://` links. */
  QMD_DOC?: { path: string; baseDir: string; root?: string };
  /** Live page has a table of contents (client rebuilds `#TOC`). */
  QMD_TOC?: boolean;
  /** The body was server-rendered, so skip the first re-mount. */
  QMD_SSR?: boolean;
  /** Document format flag (`"reveal"` switches the client into deck mode). */
  QMD_FORMAT?: string;
  /** Per-page websocket path for the multi-page site server. */
  QMD_WS_PATH?: string;

  /** reveal.js global (deck mode only); external lib, typed loosely. */
  Reveal?: any;
  /** Theme API from the head script (`theme_head`). */
  qmdSetTheme?: (mode: string) => void;
  qmdGetThemePref?: () => string;
  /** Wires every `[data-qmd-theme-toggle]` button (defined in theme_head). */
  qmdWireThemeToggles?: () => void;
  /** Observable runtime driver (OJS init script). */
  qmdRunOJS?: () => void;
  /** Binds Python `ojs_define` values into the live OJS module (OJS init script). */
  qmdBindOjsDefines?: (scope?: Element | null) => void;
  /** True once the OJS cells have been interpreted (OJS init script). */
  __qmdOjsRan?: boolean;
  /** Tracks bound `ojs_define` names -> JSON value (OJS init script). */
  __qmdOjsDefined?: Map<string, string>;
  /** Code enhancer (copy buttons + mermaid), defined in client.js itself. */
  qmdEnhanceCode?: (root: ParentNode | null) => void;
}
