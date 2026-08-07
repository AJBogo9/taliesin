// Explorer badges: every `.tmd` in the project carries its worst Taliesin severity.
//
// This is the one surface that shows project health with no interaction at all.
//
// **It reads the language client's diagnostics, and spawns nothing.** It used to run
// `taliesin check <root> --format json --strict` as a subprocess on every save — 369 ms for the
// 25-page guide — because the language server could only ever speak about buffers it had been
// sent, so a page nobody had opened was invisible to it. `taliesin lsp` now implements the LSP
// 3.17 `workspace/diagnostic` pull model, which reports **every page of the project** whether or
// not it is open, and `vscode-languageclient` polls it and files the results in an ordinary
// diagnostic collection. So the badge is now: read what is already there.
//
// Three things got better and none of them are the point on their own — the point is that the
// knowledge lives in Rust, on the protocol, once. Live instead of on-save; no process per save;
// and no second severity vocabulary, because the LSP diagnostics carry the same `TAL-*` codes at
// the same severities `check` prints (the buffer lint applies no severity floor, so the
// `--strict` flag this used to pass has nothing left to add).
//
// **Scoped to the check dot, deliberately.** The original idea also wanted `⚡ fully cached` and
// a dot for never-executed cells. Both need freeze-key machinery that lives in the execution
// layer, and nothing in this extension is allowed to start a kernel. Those two are recorded in
// DETECTION-DEBT.md rather than half-built here — and the *cached* half now has a home that is
// not this file at all: `textDocument/codeLens` puts it above the cell it describes.

import * as vscode from "vscode";
import { badgeFor, worstByFile, type FileSeverity, type Severity } from "./checkstatus";
import { TALIESIN_SOURCE } from "./client";

/** VS Code's severity enum → the three names the badge ranks. */
function severityName(severity: vscode.DiagnosticSeverity): string {
  switch (severity) {
    case vscode.DiagnosticSeverity.Error:
      return "error";
    case vscode.DiagnosticSeverity.Warning:
      return "warning";
    // `Information` and `Hint` are where `suggestion` lands (`Diagnostic::to_lsp` in Rust maps
    // the advice severities to them), and both mean the same thing to a badge.
    default:
      return "suggestion";
  }
}

/** Every Taliesin diagnostic VS Code currently holds, flattened to (file, severity). */
function taliesinRows(): FileSeverity[] {
  const rows: FileSeverity[] = [];
  for (const [uri, diagnostics] of vscode.languages.getDiagnostics()) {
    if (uri.scheme !== "file") continue;
    for (const d of diagnostics) {
      // Ours only. An editor attaches several providers to the same file, and badging a
      // `.tmd` for someone else's finding would be a claim this extension cannot stand behind.
      if (d.source !== TALIESIN_SOURCE) continue;
      rows.push({ file: uri.fsPath, severity: severityName(d.severity) });
    }
  }
  return rows;
}

class CheckDecorations implements vscode.FileDecorationProvider {
  private worst = new Map<string, Severity>();
  private readonly changed = new vscode.EventEmitter<vscode.Uri[]>();
  readonly onDidChangeFileDecorations = this.changed.event;

  provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    if (uri.scheme !== "file") return undefined;
    const severity = this.worst.get(uri.fsPath);
    if (!severity) return undefined;
    const { badge, tooltip } = badgeFor(severity);
    return new vscode.FileDecoration(badge, tooltip);
  }

  /** Re-read the diagnostics and repaint both what changed and what just went clean. */
  refresh(): void {
    const next = worstByFile(taliesinRows());
    // The union, not just the new keys: a file that dropped its last problem has to lose its
    // badge, and firing only for current keys would leave it wearing one forever.
    const touched = new Set([...this.worst.keys(), ...next.keys()]);
    this.worst = next;
    if (touched.size > 0) this.changed.fire([...touched].map((p) => vscode.Uri.file(p)));
  }

  dispose(): void {
    this.changed.dispose();
  }
}

/** Badge `.tmd` files with their check status, following the language server's diagnostics. */
export function registerDecorations(context: vscode.ExtensionContext): void {
  const provider = new CheckDecorations();
  context.subscriptions.push(provider, vscode.window.registerFileDecorationProvider(provider));

  const enabled = () =>
    vscode.workspace.getConfiguration("taliesin").get<boolean>("explorerBadges", true);
  const refresh = () => {
    if (enabled()) provider.refresh();
  };

  context.subscriptions.push(
    // Fires for pushed and pulled diagnostics alike, so this needs to know nothing about which
    // transport the server chose.
    vscode.languages.onDidChangeDiagnostics(refresh)
  );
  refresh();
}
