import * as vscode from "vscode";
import * as path from "node:path";

// Editor commands that are not language intelligence: running a CLI subcommand where its
// output belongs, which is a terminal.
//
// This also held the math symbol picker, a QuickPick over `taliesin/mathCommands`, until
// 2026-08-09. Math commands stay completable from the language server inside `$…$`; what
// went with the picker is searching that table by rendered glyph.

function binaryPath(): string {
  return vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");
}

/** Run a `taliesin` subcommand in a reused terminal, where its own formatting survives. */
function runInTerminal(name: string, args: string[], cwd: string): void {
  const existing = vscode.window.terminals.find((t) => t.name === name);
  const terminal = existing ?? vscode.window.createTerminal({ name, cwd });
  terminal.show(true);
  // Quote every argument: a document path can contain spaces, and this string is
  // interpreted by the user's shell.
  terminal.sendText([binaryPath(), ...args].map((a) => `'${a.replace(/'/g, "'\\''")}'`).join(" "));
}

export function registerCommands(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("taliesin.doctor", () => {
      const cwd =
        vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? path.dirname(process.cwd());
      runInTerminal("Taliesin doctor", ["doctor"], cwd);
    })
  );
}
