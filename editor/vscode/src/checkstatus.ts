// Turning `taliesin check --format json` into "how bad is each file", as a pure function.
//
// Split from `decorations.ts` (no `vscode` import here) so `node --test` can check the
// severity ranking without an Extension Host, which is where the ordering bug would be.

import * as path from "node:path";

/** The three severities `crates/core/src/diagnostics/codes.rs` defines, all lowercase. */
export type Severity = "error" | "warning" | "suggestion";

export interface CheckDiagnostic {
  severity: string;
  file: string;
  line?: number | null;
  code: string;
  message: string;
}

export interface CheckJson {
  diagnostics: CheckDiagnostic[];
  environment: unknown[];
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
export function worstByFile(json: CheckJson, root: string): Map<string, Severity> {
  const worst = new Map<string, Severity>();
  for (const d of json.diagnostics ?? []) {
    if (!d.file) continue;
    // `path.resolve` leaves an already-absolute path alone. Joining a root onto one would
    // produce a key nothing in the Explorer can ever match.
    const key = path.resolve(root, d.file);
    const seen = worst.get(key);
    if (seen === undefined || rank(d.severity) > rank(seen)) {
      worst.set(key, normalize(d.severity));
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
