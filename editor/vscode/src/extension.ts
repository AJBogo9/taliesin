import * as vscode from "vscode";
import { PreviewServer } from "./server";
import { relayHtml } from "./webview";
import { parseSourcepos, resolveSourceFile, relativeKey, isSourceFile } from "./paths";

export function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(
    vscode.commands.registerCommand("qmdFast.openPreview", () => openPreview(context))
  );
}

async function openPreview(context: vscode.ExtensionContext) {
  const editor = vscode.window.activeTextEditor;
  if (!editor || !isSourceFile(editor.document.fileName)) {
    vscode.window.showWarningMessage("qmd-fast: open a .tmd (or .qmd) file first.");
    return;
  }
  const docPath = editor.document.fileName;
  const binary = vscode.workspace.getConfiguration("qmdFast").get<string>("path", "qmd-fast");

  let server: PreviewServer;
  try {
    server = await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title: "Starting qmd-fast preview…" },
      () => PreviewServer.start(binary, docPath)
    );
  } catch (e) {
    vscode.window.showErrorMessage(String((e as Error).message || e));
    return;
  }

  const panel = vscode.window.createWebviewPanel(
    "qmdFastPreview",
    `Preview: ${docPath.split("/").pop()}`,
    vscode.ViewColumn.Beside,
    { enableScripts: true, retainContextWhenHidden: true }
  );

  const local = await vscode.env.asExternalUri(
    vscode.Uri.parse(`http://127.0.0.1:${server.port}/`)
  );
  panel.webview.html = relayHtml(local.toString(), panel.webview.cspSource);

  // forward: preview -> editor (reveal source on qmd-goto)
  panel.webview.onDidReceiveMessage(
    async (m) => {
      if (!m || m.type !== "qmd-goto") return;
      const abs = resolveSourceFile(docPath, m.source_file ?? null);
      const pos = parseSourcepos(m.sourcepos || "") || { line: 1, col: 1 };
      const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(abs));
      const ed = await vscode.window.showTextDocument(doc, vscode.ViewColumn.One);
      const p = new vscode.Position(Math.max(0, pos.line - 1), Math.max(0, pos.col - 1));
      ed.selection = new vscode.Selection(p, p);
      ed.revealRange(new vscode.Range(p, p), vscode.TextEditorRevealType.InCenterIfOutsideViewport);
    },
    undefined,
    context.subscriptions
  );

  // reverse: editor cursor -> preview (debounced qmd-cursor)
  let timer: NodeJS.Timeout | undefined;
  const sel = vscode.window.onDidChangeTextEditorSelection((e) => {
    const f = e.textEditor.document.fileName;
    if (!isSourceFile(f)) return;
    const key = relativeKey(docPath, f);
    const line = e.selections[0].active.line + 1;
    if (timer) clearTimeout(timer);
    timer = setTimeout(
      () => panel.webview.postMessage({ type: "qmd-cursor", file: key, line }),
      80
    );
  });

  panel.onDidDispose(
    () => {
      sel.dispose();
      if (timer) clearTimeout(timer);
      server.dispose();
    },
    undefined,
    context.subscriptions
  );
}

export function deactivate() {}
