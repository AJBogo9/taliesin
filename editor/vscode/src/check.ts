// Pure parsing + range mapping for `taliesin check --format json` output.
// No `vscode` import, so it stays in the fast `node:test` loop (mirrors paths.ts/ports.ts).
// The CLI emits one of three shapes (crates/server/src/check.rs):
//   - the current `{ "diagnostics": [...], "environment": [...] }` object,
//   - a legacy bare `[{file, line, message}, ...]` array (older binaries), or
//   - a `{ "error": "..." }` failure envelope.
// We read `.diagnostics` from the object and ignore the informational `environment` block;
// the bare array is still accepted so an older `taliesin` on PATH keeps surfacing squiggles.
// Non-zero exit is expected when findings exist, so callers parse stdout regardless of exit code.

export interface CheckDiag {
  file: string;
  line: number | null;
  message: string;
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
    const diags = rawDiags
      .filter((d): d is CheckDiag => !!d && typeof (d as any).message === "string")
      .map((d) => ({
        file: typeof d.file === "string" ? d.file : "",
        line: typeof d.line === "number" ? d.line : null,
        message: d.message,
      }));
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
}

export function toDiagnostics(out: CheckOutput, lineCount: number): DiagShape[] {
  const lastLine = Math.max(0, lineCount - 1);
  const clamp = (line0: number) => Math.max(0, Math.min(line0, lastLine));
  if (out.kind === "error") {
    return [{ line0: 0, message: out.error }];
  }
  return out.diags.map((d) => ({
    // comrak lines are 1-based; a null line is a document-level finding -> line 1 -> line0 0.
    line0: clamp((d.line ?? 1) - 1),
    message: d.message,
  }));
}
