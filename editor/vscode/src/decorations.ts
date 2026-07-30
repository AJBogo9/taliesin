// Explorer badges: every `.tmd` in the project carries its worst `taliesin check` severity.
//
// This is the one surface that shows project health with no interaction at all. The language
// server only diagnoses buffers it has been sent, so a page you have never opened is invisible
// to it; `check` sees the whole project, and at 369 ms for the 25-page guide it is cheap
// enough to run on save.
//
// **Scoped to the check dot, deliberately.** The original idea also wanted `⚡ fully cached`
// and a dot for never-executed cells. Both need freeze-key machinery that lives in the
// execution layer, and nothing in this extension or in `taliesin lsp` is allowed to start a
// kernel. Those two are recorded in DETECTION-DEBT.md rather than half-built here.

import { execFile } from "node:child_process";
import * as vscode from "vscode";
import { badgeFor, worstByFile, type CheckJson, type Severity } from "./checkstatus";
import { isSourceFile, projectRootFor } from "./paths";

class CheckDecorations implements vscode.FileDecorationProvider {
  private worst = new Map<string, Severity>();
  private readonly changed = new vscode.EventEmitter<vscode.Uri[]>();
  readonly onDidChangeFileDecorations = this.changed.event;
  /** Guards against piling up `check` runs when a burst of saves arrives. */
  private running = false;

  /** Told the total problem count after every run, so the status bar need not re-run check. */
  constructor(private readonly onCount: (count: number | null) => void) {}

  provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    if (uri.scheme !== "file") return undefined;
    const severity = this.worst.get(uri.fsPath);
    if (!severity) return undefined;
    const { badge, tooltip } = badgeFor(severity);
    return new vscode.FileDecoration(badge, tooltip);
  }

  /** Replace the map and repaint both what changed and what just went clean. */
  private apply(next: Map<string, Severity>): void {
    // The union, not just the new keys: a file that dropped its last problem has to lose its
    // badge, and firing only for current keys would leave it wearing one forever.
    const touched = new Set([...this.worst.keys(), ...next.keys()]);
    this.worst = next;
    this.changed.fire([...touched].map((p) => vscode.Uri.file(p)));
  }

  async refresh(root: string, binary: string): Promise<void> {
    if (this.running) return;
    this.running = true;
    try {
      const json = await runCheck(binary, root);
      // A run that could not produce JSON clears the badges rather than leaving stale ones:
      // a badge that no longer reflects the project is worse than no badge, and reports an
      // unknown count rather than claiming zero.
      this.apply(json ? worstByFile(json, root) : new Map());
      this.onCount(json ? (json.diagnostics ?? []).length : null);
    } finally {
      this.running = false;
    }
  }

  dispose(): void {
    this.changed.dispose();
  }
}

/** `taliesin check <root> --format json --strict`, parsed, or `null` if anything went wrong. */
function runCheck(binary: string, root: string): Promise<CheckJson | null> {
  return new Promise((resolve) => {
    execFile(
      binary,
      // `--strict` so suggestions are reported too: this surface is informational, and its
      // whole value is showing what a default gate would let through silently.
      ["check", root, "--format", "json", "--strict"],
      { cwd: root, maxBuffer: 16 * 1024 * 1024 },
      (_err, stdout) => {
        // A non-zero exit is the NORMAL case here: `check` exits non-zero whenever it finds
        // anything, which is exactly when there is something to badge. Only unparseable
        // output counts as a failure.
        try {
          resolve(JSON.parse(stdout) as CheckJson);
        } catch {
          resolve(null);
        }
      }
    );
  });
}

/** Badge `.tmd` files with their check status, refreshed on save. */
export function registerDecorations(
  context: vscode.ExtensionContext,
  onCount: (count: number | null) => void = () => {}
): void {
  const provider = new CheckDecorations(onCount);
  context.subscriptions.push(provider, vscode.window.registerFileDecorationProvider(provider));

  const enabled = () =>
    vscode.workspace.getConfiguration("taliesin").get<boolean>("explorerBadges", true);
  const binary = () =>
    vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");

  const refresh = (from: string | undefined): void => {
    if (!enabled() || !from) return;
    const root = projectRootFor(from) ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    if (root) void provider.refresh(root, binary());
  };

  context.subscriptions.push(
    // On save, not on change: `check` reads from disk, so refreshing per keystroke would
    // measure a file the buffer is already ahead of, and pay 369 ms to do it.
    vscode.workspace.onDidSaveTextDocument((d) => {
      if (isSourceFile(d.fileName)) refresh(d.fileName);
    }),
    vscode.window.onDidChangeActiveTextEditor((e) => {
      if (e && isSourceFile(e.document.fileName)) refresh(e.document.fileName);
    })
  );
  refresh(vscode.window.activeTextEditor?.document.fileName);
}
