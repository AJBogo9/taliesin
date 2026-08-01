import * as vscode from "vscode";
import { isSourceFile } from "./paths";
import { languageClient } from "./client";

// The Run / Run Above buttons over every executable code cell.
//
// The buttons are the only new thing here. Both the cell positions and the decision about
// which fences are runnable come from the server (`taliesin/cellRegions`), because a fence
// scan and an executable-language list in TypeScript are exactly the second copies the LSP
// rewrite existed to delete — and a Run button over a `{bash}` fence, or a missing one over
// `{python}`, is what that drift looks like to an author.
//
// Execution itself is `taliesin run` in a terminal. Nothing about the kernel, the cache, or
// the session lives on this side: the CLI attaches to the project's warm session, and this
// file only decides which line to point it at.

interface CellRegion {
  language: string;
  startLine: number;
  endLine: number;
  executable: boolean;
}

/** The custom request the server answers. Must match `lsp::CELL_REGIONS_METHOD`. */
const CELL_REGIONS = "taliesin/cellRegions";

/** The terminal `taliesin run` reuses, so runs do not open a terminal each time. */
const TERMINAL_NAME = "taliesin run";

function binaryPath(): string {
  return vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");
}

async function cellRegions(doc: vscode.TextDocument): Promise<CellRegion[]> {
  const client = languageClient();
  if (!client) return [];
  try {
    const regions = await client.sendRequest<CellRegion[]>(CELL_REGIONS, {
      textDocument: { uri: doc.uri.toString() },
    });
    return regions ?? [];
  } catch {
    // The server is starting, or this buffer is not one it tracks. No lenses is the right
    // answer; a thrown provider would show an error banner on every keystroke.
    return [];
  }
}

/**
 * `taliesin run <file> --line <L>` in a reused terminal.
 *
 * A terminal rather than a spawned child on purpose: the run's own formatting (colours,
 * the in-place progress redraw, the clickable figure paths) is written for a terminal, and
 * piping it through an output channel would strip exactly the parts that make it readable.
 * `--line` rather than `--cell N` because the editor knows the cursor, and an ordinal
 * computed here would be a third copy of "which fences count".
 */
function runAtLine(doc: vscode.TextDocument, line: number, cwd: string): void {
  const existing = vscode.window.terminals.find((t) => t.name === TERMINAL_NAME);
  const terminal = existing ?? vscode.window.createTerminal({ name: TERMINAL_NAME, cwd });
  terminal.show(true);
  // Quote every argument: this string is interpreted by the user's shell and a document
  // path can contain spaces or quotes.
  const q = (a: string) => `'${a.replace(/'/g, "'\\''")}'`;
  terminal.sendText(
    [binaryPath(), "run", doc.uri.fsPath, "--line", String(line)].map(q).join(" ")
  );
}

/** The document's own directory, which is where the run should be rooted from. */
function cwdFor(doc: vscode.TextDocument): string {
  const folder = vscode.workspace.getWorkspaceFolder(doc.uri);
  return folder ? folder.uri.fsPath : vscode.Uri.joinPath(doc.uri, "..").fsPath;
}

const provider: vscode.CodeLensProvider = {
  async provideCodeLenses(doc) {
    if (!isSourceFile(doc.fileName)) return [];
    const regions = await cellRegions(doc);
    const lenses: vscode.CodeLens[] = [];
    const runnable = regions.filter((r) => r.executable);
    runnable.forEach((r, i) => {
      // Anchor on the fence line (the body's first line minus one), so the buttons sit
      // above the cell rather than over its first statement.
      const anchor = new vscode.Range(Math.max(0, r.startLine - 1), 0, Math.max(0, r.startLine - 1), 0);
      // 1-based for the CLI, and the body's first line is inside the cell, which is what
      // `--line` resolves against.
      const line = r.startLine + 1;
      lenses.push(
        new vscode.CodeLens(anchor, {
          title: "▶ Run Cell",
          command: "taliesin.runCell",
          arguments: [doc.uri, line],
          tooltip: `Run this ${r.language} cell (and any earlier cell the kernel is missing)`,
        })
      );
      // "Run Above" is only meaningful when there IS something above: on the first cell it
      // would run nothing and read as a broken button.
      if (i > 0) {
        const prev = runnable[i - 1];
        lenses.push(
          new vscode.CodeLens(anchor, {
            title: "Run Above",
            command: "taliesin.runCell",
            arguments: [doc.uri, prev.startLine + 1],
            tooltip: "Run every cell before this one",
          })
        );
      }
    });
    return lenses;
  },
};

export function registerRunCell(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider({ language: "taliesin" }, provider),
    // Invoked by the lens (with args) and from the command palette (without), where the
    // cursor names the cell instead.
    vscode.commands.registerCommand(
      "taliesin.runCell",
      async (uri?: vscode.Uri, line?: number) => {
        const editor = vscode.window.activeTextEditor;
        const doc = uri
          ? await vscode.workspace.openTextDocument(uri)
          : editor?.document;
        if (!doc || !isSourceFile(doc.fileName)) {
          vscode.window.showWarningMessage("Open a .tmd file to run a cell.");
          return;
        }
        const target = line ?? (editor ? editor.selection.active.line + 1 : 1);
        await doc.save();
        runAtLine(doc, target, cwdFor(doc));
      }
    ),
    vscode.commands.registerCommand("taliesin.runAll", async () => {
      const doc = vscode.window.activeTextEditor?.document;
      if (!doc || !isSourceFile(doc.fileName)) {
        vscode.window.showWarningMessage("Open a .tmd file to run it.");
        return;
      }
      await doc.save();
      const existing = vscode.window.terminals.find((t) => t.name === TERMINAL_NAME);
      const cwd = cwdFor(doc);
      const terminal = existing ?? vscode.window.createTerminal({ name: TERMINAL_NAME, cwd });
      terminal.show(true);
      const q = (a: string) => `'${a.replace(/'/g, "'\\''")}'`;
      terminal.sendText([binaryPath(), "run", doc.uri.fsPath, "--all"].map(q).join(" "));
    })
  );
}
