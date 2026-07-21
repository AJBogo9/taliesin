import * as vscode from "vscode";
import { spawn } from "node:child_process";
import * as path from "node:path";
import { parseCheckJson, toDiagnostics, fixSpan } from "./check";
import { isSourceFile } from "./paths";
import { createDebouncer } from "./debounce";

// How long to wait after the last keystroke before re-linting the buffer (E2 on-type). Long
// enough to coalesce a burst of typing into one `check --stdin`, short enough to feel live.
const ONTYPE_DEBOUNCE_MS = 300;

// A diagnostic's "did you mean `X`" fix, keyed by the diagnostic object VS Code hands back in
// the CodeAction context (same instance we set on the collection), so the quick-fix provider
// can recover it without mutating the diagnostic. `span` is the exact [start,end) to overwrite
// when the diagnostic carried a column (E3); absent -> the provider locates the token itself.
const suggestionOf = new WeakMap<
  vscode.Diagnostic,
  { replacement: string; span?: { start: number; end: number } }
>();

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
//
// When `source` is given, lint that buffer via `--stdin` instead of the file on disk (E2
// on-type: the buffer holds unsaved edits). `file` is still passed so `check` resolves the
// base dir + reports the location from it; the on-disk file is never read in that mode.
function spawnCheck(
  binary: string,
  file: string,
  source?: string
): Promise<{ stdout: string; spawnError?: string }> {
  return new Promise((resolve) => {
    let stdout = "";
    // Inline `["check", …]` literals (not a computed args array) so the manifest gate can
    // statically verify every spawned subcommand against main.rs's COMMANDS.
    const cwd = { cwd: path.dirname(file) };
    const child =
      source === undefined
        ? spawn(binary, ["check", file, "--format", "json"], cwd)
        : spawn(binary, ["check", file, "--stdin", "--format", "json"], cwd);
    child.on("error", (e) => resolve({ stdout: "", spawnError: e.message }));
    child.stdout?.on("data", (b) => (stdout += b.toString()));
    child.on("close", () => resolve({ stdout }));
    if (source !== undefined) {
      // Feed the unsaved buffer on stdin, then close it so `check` sees EOF and runs.
      child.stdin?.on("error", () => {}); // swallow EPIPE if the child died before we wrote
      child.stdin?.end(source);
    }
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
            const entry = suggestionOf.get(diag);
            if (!entry) continue;
            const line = diag.range.start.line;
            const span = fixSpan(entry, document.lineAt(line).text);
            if (!span) continue; // couldn't locate the token unambiguously: offer nothing
            const action = new vscode.CodeAction(
              `Change to \`${entry.replacement}\``,
              vscode.CodeActionKind.QuickFix
            );
            action.edit = new vscode.WorkspaceEdit();
            action.edit.replace(
              document.uri,
              new vscode.Range(line, span.start, line, span.end),
              entry.replacement
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

  // `source` (E2 on-type) lints the live buffer via `check --stdin`; when omitted (open/save)
  // `check` reads the saved file from disk. Both feed the same collection through the shared
  // per-URI stale-result guard, so a fast on-type run can't clobber a newer save (or vice versa).
  async function refresh(doc: vscode.TextDocument, source?: string): Promise<void> {
    if (doc.languageId !== "taliesin" || !isSourceFile(doc.fileName)) return;
    const key = doc.uri.toString();
    const token = (runToken.get(key) ?? 0) + 1;
    runToken.set(key, token);

    const result = await spawnCheck(binaryPath(), doc.fileName, source);
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
      const lineLen = doc.lineAt(line0).text.length;
      // A precise `[col, endCol)` span (E3) squiggles just the token; else the whole line.
      const range =
        s.col !== undefined && s.endCol !== undefined
          ? new vscode.Range(
              line0,
              Math.min(s.col - 1, lineLen),
              line0,
              Math.min(s.endCol - 1, lineLen)
            )
          : doc.lineAt(line0).range;
      const d = new vscode.Diagnostic(range, s.message, severityOf(s.severity));
      d.source = "taliesin check";
      // Surface the stable TAL-* code, made a clickable link to the catalog when the
      // diagnostic carries a docs_url (VS Code renders `code.target` as a hyperlink).
      if (s.code) {
        d.code = s.docsUrl ? { value: s.code, target: vscode.Uri.parse(s.docsUrl) } : s.code;
      }
      // Remember an applicable "did you mean" fix so the code-action provider can offer it.
      // A column span makes the fix guess-free; without one it falls back to suggestionSpan.
      if (s.suggestion) {
        const span =
          s.col !== undefined && s.endCol !== undefined
            ? { start: s.col - 1, end: s.endCol - 1 }
            : undefined;
        suggestionOf.set(d, { replacement: s.suggestion.replacement, span });
      }
      return d;
    });
    collection.set(doc.uri, diags);
  }

  // On-type refresh, debounced per URI so a burst of keystrokes runs one `check --stdin`.
  const onType = createDebouncer(ONTYPE_DEBOUNCE_MS);
  context.subscriptions.push({ dispose: () => onType.cancelAll() });

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => refresh(doc)),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      // A save reflects the buffer on disk, so re-lint the file (with the environment probe)
      // and drop any pending on-type run to avoid a redundant back-to-back check.
      onType.cancel(doc.uri.toString());
      refresh(doc);
    }),
    vscode.workspace.onDidChangeTextDocument((e) => {
      const doc = e.document;
      if (doc.languageId !== "taliesin" || !isSourceFile(doc.fileName)) return;
      // Capture the buffer text now; lint it after the typing pause.
      onType.schedule(doc.uri.toString(), () => refresh(doc, doc.getText()));
    }),
    vscode.workspace.onDidCloseTextDocument((doc) => {
      onType.cancel(doc.uri.toString());
      collection.delete(doc.uri);
    }),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (!e.affectsConfiguration("taliesin.path")) return;
      warnedMissingBinary = false;
      for (const doc of vscode.workspace.textDocuments) refresh(doc);
    })
  );

  // Seed diagnostics for whatever is already open at activation.
  for (const doc of vscode.workspace.textDocuments) refresh(doc);
}
