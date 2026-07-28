import * as vscode from "vscode";
import { PreviewServer } from "./server";
import { relayHtml } from "./webview";
import {
  parseSourcepos,
  resolveSourceFile,
  relativeKey,
  isSourceFile,
  previewTarget,
} from "./paths";
import { registerLanguageClient } from "./client";
import { registerCommands } from "./commands";
import { PreviewRegistry } from "./previews";

/** Module-level, not per-activation: `openPreview` is a free function and both it and the
 *  reveal command must see the same set of live previews. */
const previews = new PreviewRegistry();

// The companion is two halves that do not overlap:
//
//   1. Language intelligence — completion, hover, go-to-definition, document links,
//      symbols, diagnostics, quick fixes, rename. All of it lives in `taliesin lsp`
//      (Rust), and `client.ts` is the whole client. Adding a feature means adding it in
//      the engine, where the vocabulary already is, and every other editor gets it too.
//
//   2. The live preview + bidirectional source sync, below. This cannot be an LSP concept:
//      it owns a webview, spawns `taliesin preview`, and bridges click-to-source. It stays
//      read-only — the preview navigates the editor, it never writes the source.
export function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(
    // The title-bar button passes the resource it belongs to; the keybinding and the palette
    // pass nothing. `previewTarget` is what reconciles them — see the note there.
    vscode.commands.registerCommand("taliesin.openPreview", (resource?: vscode.Uri) =>
      openPreview(context, resource)
    )
  );
  registerLanguageClient(context);
  registerCommands(context);
}

async function openPreview(context: vscode.ExtensionContext, resource?: vscode.Uri) {
  const active = vscode.window.activeTextEditor?.document;
  const docPath = previewTarget(
    resource?.scheme === "file" ? resource.fsPath : null,
    active?.uri.scheme === "file" ? active.fileName : null
  );
  if (!docPath) {
    vscode.window.showWarningMessage("Taliesin: open a .tmd file first.");
    return;
  }
  // Reuse before spawn. A second invocation reveals the panel it already has.
  const existing = previews.get(docPath);
  if (existing) {
    existing.panel.reveal(vscode.ViewColumn.Beside);
    return;
  }
  if (!previews.beginStart(docPath)) return; // a start is already in flight

  const binary = vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");

  let server: PreviewServer;
  try {
    server = await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title: "Starting Taliesin preview…" },
      () => PreviewServer.start(binary, docPath)
    );
  } catch (e) {
    vscode.window.showErrorMessage(String((e as Error).message || e));
    previews.endStart(docPath); // a failed start must not wedge the document forever
    return;
  }

  // Closing the panel is not the only way a preview ends. Closing the WINDOW tears down the
  // extension host without disposing panels, so a server registered only on
  // `panel.onDidDispose` outlives VS Code — still holding its port, its file watcher and its
  // inotify instances. They accumulated until VS Code itself could not start.
  // `dispose()` is idempotent, so both paths may fire.
  context.subscriptions.push(server);

  const panel = vscode.window.createWebviewPanel(
    "taliesinPreview",
    `Preview: ${docPath.split("/").pop()}`,
    vscode.ViewColumn.Beside,
    { enableScripts: true, retainContextWhenHidden: true }
  );

  const local = await vscode.env.asExternalUri(
    vscode.Uri.parse(`http://127.0.0.1:${server.port}/`)
  );
  panel.webview.html = relayHtml(local.toString(), panel.webview.cspSource);

  previews.set({ panel, server, docPath });
  previews.endStart(docPath);

  // forward: preview -> editor (reveal source on tali-goto)
  panel.webview.onDidReceiveMessage(
    async (m) => {
      if (!m || m.type !== "tali-goto") return;
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

  // Forward search, passive half: the editor cursor MARKS its block in the preview
  // (debounced), and never moves the page. `reveal: false` is the whole difference —
  // the active half is `taliesin.revealInPreview`, which sends `reveal: true`.
  let timer: NodeJS.Timeout | undefined;
  const sel = vscode.window.onDidChangeTextEditorSelection((e) => {
    const f = e.textEditor.document.fileName;
    if (!isSourceFile(f)) return;
    const key = relativeKey(docPath, f);
    const line = e.selections[0].active.line + 1;
    if (timer) clearTimeout(timer);
    timer = setTimeout(
      () => panel.webview.postMessage({ type: "tali-cursor", file: key, line, reveal: false }),
      80
    );
  });

  panel.onDidDispose(
    () => {
      previews.delete(docPath);
      sel.dispose();
      if (timer) clearTimeout(timer);
      server.dispose();
    },
    undefined,
    context.subscriptions
  );
}

export function deactivate() {}
