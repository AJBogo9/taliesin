// Repairing the references around a renamed or moved file.
//
// The whole computation is `taliesin/renameFileEdits`: which references exist, which of the two
// link spellings a page used, and where a `_site.yml` scalar sits are all `.tmd` knowledge. A scan
// written here would be a second copy of it, free to disagree with the renderer about what a
// reference is.
//
// `waitUntil` rather than a follow-up edit, so the repair lands inside VS Code's own rename
// transaction and one Ctrl+Z reverses both.
//
// There is deliberately no confirmation prompt. TypeScript's `updateImportsOnFileMove.enabled`
// offers one because its repair spans a whole workspace and can be wrong; this is scoped to a
// declared project (item 70: no `_site.yml`, no inbound walk) with a single undo behind it, and
// minimal-config says perfect the default rather than add a knob.
import * as vscode from "vscode";
import { languageClient } from "./client";

const RENAME_FILE_EDITS = "taliesin/renameFileEdits";

interface LspPosition {
  line: number;
  character: number;
}

/** Mirrors `FileEdits` in `lsp_rename_file.rs`. */
interface FileEdits {
  uri: string;
  edits: { range: { start: LspPosition; end: LspPosition }; newText: string }[];
}

export function registerRenameRepair(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.workspace.onWillRenameFiles((event) => {
      const files = event.files.filter((f) => f.oldUri.scheme === "file");
      if (files.length === 0) return;
      event.waitUntil(repair(files));
    })
  );
}

async function repair(
  files: readonly { readonly oldUri: vscode.Uri; readonly newUri: vscode.Uri }[]
): Promise<vscode.WorkspaceEdit> {
  const edit = new vscode.WorkspaceEdit();
  const client = languageClient();
  if (!client) return edit;

  let answer: FileEdits[];
  try {
    answer = await client.sendRequest<FileEdits[]>(RENAME_FILE_EDITS, {
      files: files.map((f) => ({
        oldUri: f.oldUri.toString(),
        newUri: f.newUri.toString(),
      })),
    });
  } catch (e) {
    // A rename must never FAIL because the repair did. Report it and let the rename proceed:
    // the author asked to rename a file, not to run a reference check.
    vscode.window.showWarningMessage(
      `Taliesin: could not update references (${String((e as Error).message || e)})`
    );
    return edit;
  }

  for (const file of answer) {
    const uri = vscode.Uri.parse(file.uri);
    for (const e of file.edits) {
      edit.replace(
        uri,
        new vscode.Range(
          new vscode.Position(e.range.start.line, e.range.start.character),
          new vscode.Position(e.range.end.line, e.range.end.character)
        ),
        e.newText
      );
    }
  }
  return edit;
}
