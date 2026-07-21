import * as vscode from "vscode";
import { outline, OutlineNode } from "./outline";
import { isSourceFile } from "./paths";

// A thin `DocumentSymbolProvider` shell over the pure `outline` scanner: it turns the heading
// tree into `vscode.DocumentSymbol`s, which power the Outline view, breadcrumbs, sticky scroll,
// and `Ctrl+Shift+O`. Read-only; correctness lives in `outline.ts` (tested vscode-free).
function toSymbol(doc: vscode.TextDocument, n: OutlineNode): vscode.DocumentSymbol {
  const lastLine = Math.max(0, doc.lineCount - 1);
  const start = Math.min(n.startLine, lastLine);
  const end = Math.min(Math.max(n.endLine, n.startLine), lastLine);
  const full = new vscode.Range(start, 0, end, doc.lineAt(end).text.length);
  const selection = new vscode.Range(start, 0, start, doc.lineAt(start).text.length);
  const sym = new vscode.DocumentSymbol(
    n.title || "(untitled)",
    "",
    vscode.SymbolKind.String,
    full,
    selection
  );
  sym.children = n.children.map((c) => toSymbol(doc, c));
  return sym;
}

export function registerDocumentSymbols(context: vscode.ExtensionContext): void {
  const provider: vscode.DocumentSymbolProvider = {
    provideDocumentSymbols(document) {
      if (!isSourceFile(document.fileName)) return [];
      return outline(document.getText()).map((n) => toSymbol(document, n));
    },
  };
  context.subscriptions.push(
    vscode.languages.registerDocumentSymbolProvider({ language: "taliesin" }, provider)
  );
}
