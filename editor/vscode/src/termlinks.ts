// Making `page.tmd:12:` in the dev-server log clickable.
//
// This is the ONE module in the companion that holds knowledge of its own: a pattern describing
// output that Rust formats. Everywhere else the server is asked. That is why it gets a drift gate
// (`src/test/termlinks.test.ts`), which pins the Rust format strings this pattern was written
// against, in both directions.
import * as vscode from "vscode";
import { DIAGNOSTIC_LINE, resolveUnique } from "./diaglink";

/** A link we produced, carrying the resolved file so `handleTerminalLink` need not re-resolve. */
interface TaliesinLink extends vscode.TerminalLink {
  file: string;
  line: number;
}

/**
 * Where a relative diagnostic path might live.
 *
 * The terminal's own cwd first, because the companion sets it when it runs `taliesin` itself, then
 * each workspace folder. Order matters only for reporting; ambiguity is resolved by refusing.
 */
function candidateRoots(terminal: vscode.Terminal): string[] {
  const roots: string[] = [];
  const shellCwd = terminal.shellIntegration?.cwd;
  if (shellCwd?.scheme === "file") roots.push(shellCwd.fsPath);
  const opts = terminal.creationOptions as vscode.TerminalOptions;
  if (typeof opts.cwd === "string") roots.push(opts.cwd);
  else if (opts.cwd?.scheme === "file") roots.push(opts.cwd.fsPath);
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    if (folder.uri.scheme === "file") roots.push(folder.uri.fsPath);
  }
  return roots;
}

const provider: vscode.TerminalLinkProvider<TaliesinLink> = {
  provideTerminalLinks(context) {
    const m = DIAGNOSTIC_LINE.exec(context.line);
    if (!m) return [];
    const file = resolveUnique(m[1], candidateRoots(context.terminal));
    if (!file) return [];
    return [
      {
        startIndex: 0,
        length: m[1].length + (m[2] ? m[2].length + 1 : 0),
        tooltip: "Open in editor",
        file,
        // The tools print 1-based lines; VS Code positions are 0-based.
        line: m[2] ? Math.max(0, Number(m[2]) - 1) : 0,
      },
    ];
  },
  async handleTerminalLink(link) {
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(link.file));
    const editor = await vscode.window.showTextDocument(doc);
    const at = new vscode.Position(link.line, 0);
    editor.selection = new vscode.Selection(at, at);
    editor.revealRange(new vscode.Range(at, at), vscode.TextEditorRevealType.InCenterIfOutsideViewport);
  },
};

export function registerTerminalLinks(context: vscode.ExtensionContext): void {
  context.subscriptions.push(vscode.window.registerTerminalLinkProvider(provider));
}
