import * as vscode from "vscode";
import { spawn } from "node:child_process";
import * as path from "node:path";
import { parseCheckJson, toDiagnostics, suggestionSpan } from "./check";
import { isSourceFile } from "./paths";

// The replacement string of a diagnostic's "did you mean `X`" fix, keyed by the diagnostic
// object VS Code hands back in the CodeAction context (same instance we set on the
// collection), so the quick-fix provider can recover it without mutating the diagnostic.
const suggestionOf = new WeakMap<vscode.Diagnostic, string>();

// Map the CLI's severity word to a VS Code severity. An older `taliesin` (or an
// unclassified finding) has no severity; default to Warning, the prior behavior.
function severityOf(severity: string | undefined): vscode.DiagnosticSeverity {
  return severity === "error"
    ? vscode.DiagnosticSeverity.Error
    : vscode.DiagnosticSeverity.Warning;
}

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

  // Offer a one-click fix for any diagnostic on the cursor's line that carried a
  // "did you mean `X`" suggestion, replacing the located bad token with the correction.
  context.subscriptions.push(
    vscode.languages.registerCodeActionsProvider(
      { language: "taliesin" },
      {
        provideCodeActions(document, _range, ctx) {
          const actions: vscode.CodeAction[] = [];
          for (const diag of ctx.diagnostics) {
            const replacement = suggestionOf.get(diag);
            if (!replacement) continue;
            const line = diag.range.start.line;
            const span = suggestionSpan(document.lineAt(line).text, replacement);
            if (!span) continue; // couldn't locate the token unambiguously: offer nothing
            const action = new vscode.CodeAction(
              `Change to \`${replacement}\``,
              vscode.CodeActionKind.QuickFix
            );
            action.edit = new vscode.WorkspaceEdit();
            action.edit.replace(
              document.uri,
              new vscode.Range(line, span.start, line, span.end),
              replacement
            );
            action.diagnostics = [diag];
            action.isPreferred = true;
            actions.push(action);
          }
          return actions;
        },
      },
      { providedCodeActionKinds: [vscode.CodeActionKind.QuickFix] }
    )
  );

  const runToken = new Map<string, number>(); // per-URI monotonic run id (stale-result guard)
  let warnedMissingBinary = false;

  const binaryPath = () =>
    vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");

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
          `Taliesin: could not run \`${binaryPath()}\` for diagnostics (${result.spawnError}). ` +
            `Set "taliesin.path" to the taliesin binary.`
        );
      }
      return;
    }

    const shapes = toDiagnostics(parseCheckJson(result.stdout), doc.lineCount);
    const diags = shapes.map((s) => {
      const line0 = Math.max(0, Math.min(s.line0, doc.lineCount - 1));
      const range = doc.lineAt(line0).range; // whole-line squiggle (JSON carries no column)
      const d = new vscode.Diagnostic(range, s.message, severityOf(s.severity));
      d.source = "taliesin check";
      // Surface the stable TAL-* code, made a clickable link to the catalog when the
      // diagnostic carries a docs_url (VS Code renders `code.target` as a hyperlink).
      if (s.code) {
        d.code = s.docsUrl ? { value: s.code, target: vscode.Uri.parse(s.docsUrl) } : s.code;
      }
      // Remember an applicable "did you mean" fix so the code-action provider can offer it.
      if (s.suggestion) suggestionOf.set(d, s.suggestion.replacement);
      return d;
    });
    collection.set(doc.uri, diags);
  }

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => refresh(doc)),
    vscode.workspace.onDidSaveTextDocument((doc) => refresh(doc)),
    vscode.workspace.onDidCloseTextDocument((doc) => collection.delete(doc.uri)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (!e.affectsConfiguration("taliesin.path")) return;
      warnedMissingBinary = false;
      for (const doc of vscode.workspace.textDocuments) refresh(doc);
    })
  );

  // Seed diagnostics for whatever is already open at activation.
  for (const doc of vscode.workspace.textDocuments) refresh(doc);
}
