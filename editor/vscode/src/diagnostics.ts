import * as vscode from "vscode";
import { spawn } from "node:child_process";
import * as path from "node:path";
import { parseCheckJson, toDiagnostics } from "./check";
import { isSourceFile } from "./paths";

// Run `taliesin check --format json <file>` and collect stdout. Never rejects: a spawn
// failure resolves to { spawnError }, and a non-zero exit (expected when findings exist)
// is ignored, we parse stdout regardless of exit code.
function spawnCheck(binary: string, file: string): Promise<{ stdout: string; spawnError?: string }> {
  return new Promise((resolve) => {
    let stdout = "";
    const child = spawn(binary, ["check", file, "--format", "json"], {
      cwd: path.dirname(file),
    });
    child.on("error", (e) => resolve({ stdout: "", spawnError: e.message }));
    child.stdout?.on("data", (b) => (stdout += b.toString()));
    child.on("close", () => resolve({ stdout }));
  });
}

// Own a single DiagnosticCollection, refresh it on open/save/config-change for the active
// Taliesin document, and supersede in-flight checks when a newer run for the same URI starts.
export function registerDiagnostics(context: vscode.ExtensionContext): void {
  const collection = vscode.languages.createDiagnosticCollection("taliesin");
  context.subscriptions.push(collection);

  const runToken = new Map<string, number>(); // per-URI monotonic run id (stale-result guard)
  let warnedMissingBinary = false;

  const binaryPath = () =>
    vscode.workspace.getConfiguration("qmdFast").get<string>("path", "qmd-fast");

  async function refresh(doc: vscode.TextDocument): Promise<void> {
    if (doc.languageId !== "taliesin" || !isSourceFile(doc.fileName)) return;
    const key = doc.uri.toString();
    const token = (runToken.get(key) ?? 0) + 1;
    runToken.set(key, token);

    const result = await spawnCheck(binaryPath(), doc.fileName);
    if (runToken.get(key) !== token) return; // a newer save superseded this run

    if (result.spawnError) {
      collection.delete(doc.uri);
      if (!warnedMissingBinary) {
        warnedMissingBinary = true; // one toast, never per-keystroke
        vscode.window.showWarningMessage(
          `qmd-fast: could not run \`${binaryPath()}\` for diagnostics (${result.spawnError}). ` +
            `Set "qmdFast.path" to the taliesin/qmd-fast binary.`
        );
      }
      return;
    }

    const shapes = toDiagnostics(parseCheckJson(result.stdout), doc.lineCount);
    const diags = shapes.map((s) => {
      const line0 = Math.max(0, Math.min(s.line0, doc.lineCount - 1));
      const range = doc.lineAt(line0).range; // whole-line squiggle (JSON carries no column)
      const d = new vscode.Diagnostic(range, s.message, vscode.DiagnosticSeverity.Warning);
      d.source = "taliesin check";
      return d;
    });
    collection.set(doc.uri, diags);
  }

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => refresh(doc)),
    vscode.workspace.onDidSaveTextDocument((doc) => refresh(doc)),
    vscode.workspace.onDidCloseTextDocument((doc) => collection.delete(doc.uri)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (!e.affectsConfiguration("qmdFast.path")) return;
      warnedMissingBinary = false;
      for (const doc of vscode.workspace.textDocuments) refresh(doc);
    })
  );

  // Seed diagnostics for whatever is already open at activation.
  for (const doc of vscode.workspace.textDocuments) refresh(doc);
}
