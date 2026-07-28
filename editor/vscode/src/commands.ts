import * as vscode from "vscode";
import { spawn } from "node:child_process";
import * as path from "node:path";
import { isSourceFile } from "./paths";

// Editor commands that are not language intelligence: running a CLI subcommand where its
// output belongs (a terminal), and the math symbol picker.
//
// Everything an author can type is completable from the language server, but a symbol you
// cannot spell is not reachable by completion at all — you have to already know that ⊗ is
// `\otimes` to type `\ot`. A searchable picker is the answer state-of-the-art math editors
// converge on (Tinymist's symbol view), and it costs one QuickPick over a vocabulary the
// binary already publishes.

interface MathCommand {
  name: string;
  description: string;
  category: string;
  snippet: string;
}

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

/** `taliesin vocab`'s `mathCommands`, fetched once per session. */
function fetchMathCommands(): Promise<MathCommand[]> {
  return new Promise((resolve, reject) => {
    let stdout = "";
    // Inline literal args (not a computed array) so the manifest gate can statically check
    // every spawned subcommand against main.rs's COMMANDS.
    const child = spawn(binaryPath(), ["vocab"]);
    child.on("error", (e) => reject(e));
    child.stdout?.on("data", (b) => (stdout += b.toString()));
    child.on("close", () => {
      try {
        const parsed = JSON.parse(stdout) as { mathCommands?: MathCommand[] };
        resolve(parsed.mathCommands ?? []);
      } catch (e) {
        reject(e);
      }
    });
  });
}

export function registerCommands(context: vscode.ExtensionContext): void {
  let mathCache: Promise<MathCommand[]> | undefined;
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("taliesin.path")) mathCache = undefined;
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("taliesin.check", () => {
      const doc = vscode.window.activeTextEditor?.document;
      if (!doc || !isSourceFile(doc.fileName)) {
        vscode.window.showWarningMessage("Taliesin: open a .tmd file first.");
        return;
      }
      runInTerminal("Taliesin check", ["check", doc.fileName], path.dirname(doc.fileName));
    }),

    vscode.commands.registerCommand("taliesin.doctor", () => {
      const cwd =
        vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? path.dirname(process.cwd());
      runInTerminal("Taliesin doctor", ["doctor"], cwd);
    }),

    vscode.commands.registerCommand("taliesin.insertMathSymbol", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) return;
      let commands: MathCommand[];
      try {
        commands = await (mathCache ??= fetchMathCommands());
      } catch (e) {
        mathCache = undefined; // a transient failure must not poison the session
        vscode.window.showErrorMessage(
          `Taliesin: could not read the math vocabulary (${(e as Error).message}). ` +
            `Set "taliesin.path" to the taliesin binary.`
        );
        return;
      }
      // `description` is the rendered glyph for a plain symbol, which is what makes the
      // list searchable by what you SEE rather than by what it is called: typing "sum"
      // finds `\sum`, and typing "⊗" finds `\otimes`.
      const picked = await vscode.window.showQuickPick(
        commands.map((c) => ({
          label: c.name,
          description: c.description,
          detail: c.category,
          command: c,
        })),
        {
          title: "Insert math symbol",
          placeHolder: "Search by name, symbol, or category",
          matchOnDescription: true,
          matchOnDetail: true,
        }
      );
      if (!picked) return;
      const body = picked.command.snippet || picked.command.name;
      await editor.insertSnippet(new vscode.SnippetString(body));
    })
  );
}
