// Turning "what does the language server say about this project" into "how bad is each file",
// as a pure function.
//
// No `vscode` import here, so `node --test` can check the severity ranking without an Extension
// Host — which is where the ordering bug would be. The caller converts VS Code's severity enum
// to the three names below; this file only ranks them.
//
// **This used to parse `taliesin check --format json` from a subprocess per save.** It did
// because the language server had no way to speak about a file nobody had opened, so the whole
// project could only be learned by running `check` over it. `taliesin lsp` now implements the
// LSP 3.17 `workspace/diagnostic` pull model, so those findings arrive over the protocol the
// client is already connected to, for every page, live rather than on save.

/** The three severities `crates/core/src/diagnostics/codes.rs` defines, all lowercase. */
export type Severity = "error" | "warning" | "suggestion";

/** One diagnostic reduced to what a badge needs: which file, and how bad. */
export interface FileSeverity {
  /** Absolute path. VS Code's `Uri.fsPath` already is one. */
  file: string;
  severity: string;
}

/**
 * Higher is worse. The same ordering `codes::severity_rank` uses in Rust, so the badge cannot
 * disagree with the gate about what outranks what. An unrecognised severity ranks with
 * `error`, for the reason Rust gives: a diagnostic nobody classified is not something to
 * silently stop caring about.
 */
function rank(severity: string): number {
  switch (severity) {
    case "suggestion":
      return 0;
    case "warning":
      return 1;
    default:
      return 2;
  }
}

/** The worst severity per file, keyed by absolute path. */
export function worstByFile(rows: Iterable<FileSeverity>): Map<string, Severity> {
  const worst = new Map<string, Severity>();
  for (const d of rows) {
    if (!d.file) continue;
    const seen = worst.get(d.file);
    if (seen === undefined || rank(d.severity) > rank(seen)) {
      worst.set(d.file, normalize(d.severity));
    }
  }
  return worst;
}

/** Map an arbitrary severity string onto the three we badge. */
function normalize(severity: string): Severity {
  return severity === "warning" || severity === "suggestion" ? severity : "error";
}

/** The badge glyph and hover text for a severity. */
export function badgeFor(severity: Severity): { badge: string; tooltip: string } {
  switch (severity) {
    case "error":
      return { badge: "!", tooltip: "Taliesin: this page has errors" };
    case "warning":
      return { badge: "▲", tooltip: "Taliesin: this page has warnings" };
    case "suggestion":
      return { badge: "·", tooltip: "Taliesin: this page has suggestions" };
  }
}
