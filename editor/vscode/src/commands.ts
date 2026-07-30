import * as vscode from "vscode";
import { spawn } from "node:child_process";
import * as path from "node:path";
import { isSourceFile } from "./paths";
import { languageClient } from "./client";

// Editor commands that are not language intelligence: running a CLI subcommand where its
// output belongs (a terminal), the math symbol picker, and the four structural transforms.
//
// Everything an author can type is completable from the language server, but a symbol you
// cannot spell is not reachable by completion at all — you have to already know that ⊗ is
// `\otimes` to type `\ot`. A searchable picker is the answer state-of-the-art math editors
// converge on (Tinymist's symbol view), and it costs one QuickPick over a vocabulary the
// binary already publishes.
//
// The structural commands (move a section up or down, promote or demote a heading) are the
// legal replacement for the drag-to-reorder-slides gesture removed for breaking the
// single-editing-surface rule: they transform the `.tmd` **buffer**, in the editor, and the
// preview stays a read-only view of it. What is here is only the editor plumbing — the
// cursor, the edit, the message. Which lines make up a section, which neighbour is a sibling
// and when a move is refused all come from `taliesin/sectionEdit`, because a heading scan in
// TypeScript is the second copy the LSP rewrite deleted.

interface MathCommand {
  name: string;
  description: string;
  category: string;
  snippet: string;
}

/** `taliesin/sectionEdit`: the structural transforms, computed server-side. */
const SECTION_EDIT = "taliesin/sectionEdit";

interface LspPosition {
  line: number;
  character: number;
}

interface SectionEditResult {
  edits: { range: { start: LspPosition; end: LspPosition }; newText: string }[];
  /** Where the caret belongs afterwards; absent when the editor's own tracking is right. */
  cursor?: LspPosition;
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

/**
 * Ask the server for a structural transform at the cursor and apply it.
 *
 * A refusal ("this is the last section under its parent") arrives as a request error and is
 * shown in the status bar rather than as a modal: hitting the end of a list of siblings is a
 * normal outcome of holding a key down, not an error the author has to dismiss.
 */
async function applySectionEdit(op: "moveUp" | "moveDown" | "promote" | "demote"): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  // The language id OR a `.tmd` path, which is the same admission rule the server applies to
  // a buffer (`lsp.rs`'s didOpen). An unsaved scratch buffer is named `Untitled-1` and every
  // part of this transform is pathless, so requiring the extension would refuse to reorder
  // the sections of a document you had not saved yet.
  const claimed =
    editor && (editor.document.languageId === "taliesin" || isSourceFile(editor.document.fileName));
  if (!editor || !claimed) {
    vscode.window.showWarningMessage("Taliesin: open a .tmd file first.");
    return;
  }
  const client = languageClient();
  if (!client) {
    vscode.window.showWarningMessage(
      "Taliesin: the language server is not running — run “Taliesin: Restart Language Server”."
    );
    return;
  }

  const cursor = editor.selection.active;
  let result: SectionEditResult;
  try {
    result = await client.sendRequest<SectionEditResult>(SECTION_EDIT, {
      textDocument: { uri: editor.document.uri.toString() },
      position: { line: cursor.line, character: cursor.character },
      op,
    });
  } catch (e) {
    vscode.window.setStatusBarMessage(`Taliesin: ${(e as Error).message}`, 5000);
    return;
  }

  // One `WorkspaceEdit` for the whole transform: it is a single undo step (Ctrl+Z puts the
  // section back rather than unpicking a promotion heading by heading) and it is the path the
  // language client itself uses for server-computed edits, so a multi-edit answer needs no
  // ordering care here.
  const workspaceEdit = new vscode.WorkspaceEdit();
  workspaceEdit.set(
    editor.document.uri,
    result.edits.map(
      (e) => new vscode.TextEdit(client.protocol2CodeConverter.asRange(e.range), e.newText)
    )
  );
  const applied = await vscode.workspace.applyEdit(workspaceEdit);
  if (!applied || !result.cursor) return;
  // The caret has to be told where the section went: see `SectionEdit::cursor` in
  // `lsp_edits.rs` for why the editor's own edit tracking cannot work it out.
  const position = editor.document.validatePosition(
    client.protocol2CodeConverter.asPosition(result.cursor)
  );
  editor.selection = new vscode.Selection(position, position);
  editor.revealRange(new vscode.Range(position, position));
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
    }),

    // The four structural transforms. Literal command strings, because `manifest.test.ts`
    // scans this file for registration calls and matches each name it finds against
    // `contributes.commands` — a computed name would read as an unregistered contribution.
    vscode.commands.registerCommand("taliesin.moveSectionUp", () => applySectionEdit("moveUp")),
    vscode.commands.registerCommand("taliesin.moveSectionDown", () => applySectionEdit("moveDown")),
    vscode.commands.registerCommand("taliesin.promoteHeading", () => applySectionEdit("promote")),
    vscode.commands.registerCommand("taliesin.demoteHeading", () => applySectionEdit("demote"))
  );
}
