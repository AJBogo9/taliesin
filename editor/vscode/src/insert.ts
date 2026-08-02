// The paste and drop gestures.
//
// These are the one place the companion is allowed to feel like it knows `.tmd`, and it does not:
// every string inserted below comes from `taliesin/insertEdit`. What lives here is only what VS
// Code owns and an LSP cannot express: which clipboard flavour the author pasted, the bytes of a
// pasted image, and the undo grouping.
//
// Single-editing-surface is untouched. These are edits the AUTHOR initiates in the editing
// surface, which is the same standing as the section-move commands; the preview still never
// writes back.
import * as path from "node:path";
import * as vscode from "vscode";
import { languageClient } from "./client";
import {
  IMAGE_MIMES,
  InsertEditResult,
  InsertKind,
  classifyPaste,
  hasHtmlTable,
  isBibtex,
  isUrl,
} from "./pastekind";

/** `taliesin/insertEdit`: the text a gesture inserts, computed server-side. */
const INSERT_EDIT = "taliesin/insertEdit";

/** Ask the server what to insert. `null` on any refusal, so the caller falls back to plain paste. */
async function ask(
  document: vscode.TextDocument,
  kind: InsertKind,
  payload: string
): Promise<InsertEditResult | null> {
  const client = languageClient();
  if (!client) return null;
  try {
    return await client.sendRequest<InsertEditResult>(INSERT_EDIT, {
      textDocument: { uri: document.uri.toString() },
      kind,
      payload,
    });
  } catch {
    // A refusal ("that does not look like a table", "unsupported image type") is a normal answer
    // on a speculative route. Showing it would mean a popup every time the author pastes prose
    // containing a tab, so the message is surfaced only where the author explicitly chose the
    // gesture (see the drop handler).
    return null;
  }
}

/** Turn a server answer into an edit, writing any side file first. */
async function toPasteEdit(
  document: vscode.TextDocument,
  result: InsertEditResult,
  bytes: Uint8Array | null,
  title: string
): Promise<vscode.DocumentPasteEdit> {
  if (result.writeFile && bytes) {
    const target = vscode.Uri.file(path.join(path.dirname(document.uri.fsPath), result.writeFile));
    await vscode.workspace.fs.writeFile(target, bytes);
  }
  const insert = result.isSnippet
    ? new vscode.SnippetString(result.text)
    : result.text;
  const edit = new vscode.DocumentPasteEdit(
    insert,
    title,
    vscode.DocumentDropOrPasteEditKind.Text.append("taliesin")
  );
  if (result.append) {
    // A WorkspaceEdit rather than a bare write, so appending to the .bib is part of the same
    // undo as the paste instead of a change behind the editor's back.
    const we = new vscode.WorkspaceEdit();
    const bib = vscode.Uri.file(result.append.path);
    const doc = await vscode.workspace.openTextDocument(bib);
    we.insert(bib, doc.lineAt(doc.lineCount - 1).range.end, result.append.text);
    edit.additionalEdit = we;
  }
  return edit;
}

const pasteProvider: vscode.DocumentPasteEditProvider = {
  async provideDocumentPasteEdits(document, ranges, dataTransfer, _context, token) {
    const mimes: string[] = [];
    dataTransfer.forEach((_item, mime) => mimes.push(mime));
    const hasSelection = ranges.some((r) => !r.isEmpty);
    const route = classifyPaste(mimes);
    if (!route || token.isCancellationRequested) return undefined;

    if (route === "image") {
      const mime = IMAGE_MIMES.find((m) => mimes.includes(m))!;
      const file = dataTransfer.get(mime)?.asFile();
      if (!file) return undefined;
      const bytes = await file.data();
      const result = await ask(document, "image", mime);
      if (!result) return undefined;
      return [await toPasteEdit(document, result, bytes, "Insert figure")];
    }

    if (route === "htmlTable") {
      const html = await dataTransfer.get("text/html")?.asString();
      if (!html || !hasHtmlTable(html)) return undefined;
      const result = await ask(document, "htmlTable", html);
      if (!result) return undefined;
      return [await toPasteEdit(document, result, null, "Insert table")];
    }

    // Everything left depends on the pasted text.
    const plain = (await dataTransfer.get("text/plain")?.asString()) ?? "";
    if (!plain) return undefined;

    if (isBibtex(plain)) {
      const result = await ask(document, "bibtex", plain);
      if (!result) return undefined;
      return [await toPasteEdit(document, result, null, "Insert citation")];
    }

    if (hasSelection && isUrl(plain)) {
      const selected = document.getText(ranges.find((r) => !r.isEmpty));
      return [
        new vscode.DocumentPasteEdit(
          `[${selected}](${plain.trim()})`,
          "Insert link",
          vscode.DocumentDropOrPasteEditKind.Text.append("taliesin", "link")
        ),
      ];
    }

    if (plain.includes("\t")) {
      const result = await ask(document, "tsvTable", plain);
      if (!result) return undefined;
      const edit = await toPasteEdit(document, result, null, "Insert table");
      // NOT the default. Plain text containing tabs is not a table, and silently becoming one is
      // worse than one extra keystroke, so this yields to the ordinary text paste and appears in
      // the paste-as menu instead. The HTML route above had a real <table> and does not yield.
      edit.yieldTo = [vscode.DocumentDropOrPasteEditKind.Text];
      return [edit];
    }
    return undefined;
  },
};

/**
 * Exported for the Extension Host suite only.
 *
 * VS Code publishes no command that drives a drop provider (verified by listing every command
 * matching `/drop|paste/` inside a real host: `editor.action.pasteAs` and
 * `clipboardPasteAction` exist for paste, and there is no drop equivalent), so the drop test
 * calls this directly. That covers the provider's logic, the server round-trip and the real
 * `DataTransfer`, but NOT VS Code's own routing of a drop event to it.
 */
export const dropProvider: vscode.DocumentDropEditProvider = {
  async provideDocumentDropEdits(document, _position, dataTransfer, token) {
    const list = await dataTransfer.get("text/uri-list")?.asString();
    if (!list || token.isCancellationRequested) return undefined;
    const first = list.split(/\r?\n/).find((l) => l.trim().length > 0);
    if (!first) return undefined;
    let dropped: vscode.Uri;
    try {
      dropped = vscode.Uri.parse(first.trim());
    } catch {
      return undefined;
    }
    if (dropped.scheme !== "file") return undefined;

    let result = await ask(document, "asset", dropped.fsPath);
    if (!result) return undefined;

    if (result.outside) {
      // A drag is an explicit gesture, so unlike a speculative paste route this one talks to the
      // author rather than silently doing nothing.
      const copy = "Copy into the document folder";
      const anyway = "Insert path anyway";
      const choice = await vscode.window.showWarningMessage(result.outside, copy, anyway);
      if (choice === copy) {
        const target = vscode.Uri.file(
          path.join(path.dirname(document.uri.fsPath), path.basename(dropped.fsPath))
        );
        await vscode.workspace.fs.copy(dropped, target, { overwrite: false });
        // Ask again for the now-inside path, so the inserted reference comes from the server
        // rather than being assembled here.
        result = await ask(document, "asset", target.fsPath);
        if (!result) return undefined;
      } else if (choice !== anyway) {
        return undefined;
      }
    }

    const insert = result.isSnippet ? new vscode.SnippetString(result.text) : result.text;
    return new vscode.DocumentDropEdit(insert);
  },
};

export function registerInsertProviders(context: vscode.ExtensionContext): void {
  const selector: vscode.DocumentSelector = { language: "taliesin" };
  context.subscriptions.push(
    vscode.languages.registerDocumentPasteEditProvider(selector, pasteProvider, {
      providedPasteEditKinds: [vscode.DocumentDropOrPasteEditKind.Text.append("taliesin")],
      pasteMimeTypes: [...IMAGE_MIMES, "text/html", "text/plain"],
    }),
    vscode.languages.registerDocumentDropEditProvider(selector, dropProvider)
  );
}
