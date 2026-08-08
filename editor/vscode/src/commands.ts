import * as vscode from "vscode";
import * as path from "node:path";
import { languageClient } from "./client";

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

/** `taliesin/mathCommands`: the symbol picker's table, from the vocabulary Rust owns. */
const MATH_COMMANDS = "taliesin/mathCommands";

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

/**
 * The picker's table, fetched once per session.
 *
 * This spawned `taliesin vocab` and read one key out of a whole-vocabulary JSON dump until
 * Wave 2 cut that verb. The table it was generated from did not move: the server answers
 * `taliesin/mathCommands` from the same `math_vocab` rows, over the connection the companion
 * already holds. Which is the doctrine anyway — editor intelligence lives in the LSP, not in
 * a TypeScript subprocess call.
 */
async function fetchMathCommands(): Promise<MathCommand[]> {
  const client = languageClient();
  if (!client) {
    throw new Error(
      "the language server is not running — run “Taliesin: Restart Language Server”"
    );
  }
  return (await client.sendRequest<MathCommand[] | null>(MATH_COMMANDS)) ?? [];
}

export function registerCommands(context: vscode.ExtensionContext): void {
  let mathCache: Promise<MathCommand[]> | undefined;
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("taliesin.path")) mathCache = undefined;
    })
  );

  context.subscriptions.push(
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
