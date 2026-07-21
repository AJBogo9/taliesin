// Pure parsing + range mapping for `taliesin check --format json` output.
// No `vscode` import, so it stays in the fast `node:test` loop (mirrors paths.ts/ports.ts).
// The CLI emits one of three shapes (crates/server/src/check.rs):
//   - the current `{ "diagnostics": [...], "environment": [...] }` object,
//   - a legacy bare `[{file, line, message}, ...]` array (older binaries), or
//   - a `{ "error": "..." }` failure envelope.
// We read `.diagnostics` from the object and ignore the informational `environment` block;
// the bare array is still accepted so an older `taliesin` on PATH keeps surfacing squiggles.
// Non-zero exit is expected when findings exist, so callers parse stdout regardless of exit code.

// A structured "did you mean `X`" fix the CLI lifted from the message (`suggestion` in the
// JSON). `replacement` is the corrected value; the bad token is located on the line by
// `suggestionSpan`, since a whole-line diagnostic carries no column.
export interface Suggestion {
  replacement: string;
}

export interface CheckDiag {
  file: string;
  line: number | null;
  message: string;
  // Agent-grade fields the CLI emits under `--format json` (crates/server/src/check.rs).
  // All optional: an older `taliesin` emits only {file,line,message}, and we must not
  // invent them, so the wiring falls back to a whole-line warning.
  severity?: string; // "error" | "warning"
  code?: string; // stable TAL-* code
  docsUrl?: string; // catalog anchor; the wire field is snake_case `docs_url`
  suggestion?: Suggestion;
}

export type CheckOutput =
  | { kind: "diags"; diags: CheckDiag[] }
  | { kind: "error"; error: string };

// Parse the CLI's stdout. Never throws: malformed output becomes a {error} so the
// caller can surface it instead of dropping squiggles silently.
export function parseCheckJson(stdout: string): CheckOutput {
  const text = stdout.trim();
  if (text === "") return { kind: "diags", diags: [] };
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return { kind: "error", error: `check produced unparseable output: ${text.slice(0, 200)}` };
  }
  // The `{ error }` failure envelope wins first: it is an object, but never carries
  // diagnostics, so check it before we look for a `.diagnostics` array.
  if (value && typeof value === "object" && !Array.isArray(value) && typeof (value as any).error === "string") {
    return { kind: "error", error: (value as any).error };
  }
  // Accept both the current `{ diagnostics: [...] }` object and the legacy bare array.
  const rawDiags: any[] | null = Array.isArray(value)
    ? value
    : value && typeof value === "object" && Array.isArray((value as any).diagnostics)
      ? (value as any).diagnostics
      : null;
  if (rawDiags) {
    const diags: CheckDiag[] = rawDiags
      .filter((d) => !!d && typeof d.message === "string")
      .map((d) => {
        const diag: CheckDiag = {
          file: typeof d.file === "string" ? d.file : "",
          line: typeof d.line === "number" ? d.line : null,
          message: d.message,
        };
        // Carry the agent-grade fields when present; each is validated so a malformed
        // one (e.g. a non-string replacement) is dropped, not surfaced as junk.
        if (typeof d.severity === "string") diag.severity = d.severity;
        if (typeof d.code === "string") diag.code = d.code;
        if (typeof d.docs_url === "string") diag.docsUrl = d.docs_url;
        if (d.suggestion && typeof d.suggestion.replacement === "string") {
          diag.suggestion = { replacement: d.suggestion.replacement };
        }
        return diag;
      });
    return { kind: "diags", diags };
  }
  return { kind: "error", error: `check produced unexpected output: ${text.slice(0, 200)}` };
}

// A `vscode`-free description of where a diagnostic lands. The wiring turns each into a
// whole-line `vscode.Diagnostic` via `document.lineAt(line0).range`, so the horizontal
// (EOL) extent is VS Code's job and this stays testable.
export interface DiagShape {
  line0: number; // 0-based, clamped to [0, lineCount - 1]
  message: string;
  // Passed through from CheckDiag when present; absent keys are omitted (never set to
  // `undefined`) so a legacy diagnostic round-trips to the bare {line0,message} shape.
  severity?: string;
  code?: string;
  docsUrl?: string;
  suggestion?: Suggestion;
}

export function toDiagnostics(out: CheckOutput, lineCount: number): DiagShape[] {
  const lastLine = Math.max(0, lineCount - 1);
  const clamp = (line0: number) => Math.max(0, Math.min(line0, lastLine));
  if (out.kind === "error") {
    // A check failure (unreadable file, render panic) is a real error, not a warning.
    return [{ line0: 0, message: out.error, severity: "error" }];
  }
  return out.diags.map((d) => {
    // comrak lines are 1-based; a null line is a document-level finding -> line 1 -> line0 0.
    const shape: DiagShape = { line0: clamp((d.line ?? 1) - 1), message: d.message };
    if (d.severity !== undefined) shape.severity = d.severity;
    if (d.code !== undefined) shape.code = d.code;
    if (d.docsUrl !== undefined) shape.docsUrl = d.docsUrl;
    if (d.suggestion !== undefined) shape.suggestion = d.suggestion;
    return shape;
  });
}

// Levenshtein edit distance between two short strings (a line token vs. the suggested
// replacement). Iterative two-row DP; inputs are single words, so this stays cheap.
function editDistance(a: string, b: string): number {
  const n = b.length;
  let prev = Array.from({ length: n + 1 }, (_, j) => j);
  let cur = new Array<number>(n + 1);
  for (let i = 1; i <= a.length; i++) {
    cur[0] = i;
    for (let j = 1; j <= n; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + cost);
    }
    [prev, cur] = [cur, prev];
  }
  return prev[n];
}

// A "did you mean" hint is generated at edit distance <= 2, so that is the window in which
// a token on the line is a plausible match for the suggested replacement.
const MAX_SUGGESTION_DISTANCE = 2;

// The [start, end) span on `lineText` of the token a "did you mean `replacement`" fix
// should overwrite: the single token nearest `replacement` by edit distance (1..=2), or
// null when there is no unique close token (nothing close, or a tie — in which case we
// offer no fix rather than guess wrong). Pure + vscode-free, so it stays in the fast
// node:test loop; the caller turns the span into a WorkspaceEdit. This locates the bad
// token because a whole-line diagnostic carries no column (see DiagShape).
export function suggestionSpan(
  lineText: string,
  replacement: string
): { start: number; end: number } | null {
  // A token: `@`/word char, then word chars or hyphens — matches `treme`, `@fig-reslts`.
  const re = /[@\w][\w-]*/g;
  let best: { start: number; end: number; dist: number } | null = null;
  let tie = false;
  let m: RegExpExecArray | null;
  while ((m = re.exec(lineText)) !== null) {
    const token = m[0];
    if (token === replacement) continue; // already correct: nothing to replace
    const dist = editDistance(token, replacement);
    if (dist < 1 || dist > MAX_SUGGESTION_DISTANCE) continue;
    if (!best || dist < best.dist) {
      best = { start: m.index, end: m.index + token.length, dist };
      tie = false;
    } else if (dist === best.dist) {
      tie = true;
    }
  }
  return best && !tie ? { start: best.start, end: best.end } : null;
}
