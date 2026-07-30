import * as path from "node:path";
import * as vscode from "vscode";
import { PreviewServer } from "./server";
import { relayHtml } from "./webview";
import {
  parseSourcepos,
  resolveSourceFile,
  cursorTarget,
  isSourceFile,
  previewTarget,
  projectRootFor,
  pageUrlFor,
} from "./paths";
import { readSiteMap } from "./map";
import { registerLanguageClient } from "./client";
import { registerCommands } from "./commands";
import { LivePreview, PreviewRegistry, previewKey } from "./previews";

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
    ),
    // Forward search, active half: put the preview where the cursor is, on request.
    // The passive half (marking, never scrolling) rides the selection listener in
    // `openPreview` and sends `reveal: false`.
    vscode.commands.registerCommand("taliesin.revealInPreview", () => {
      // `activeTextEditor` is `undefined` whenever a webview holds focus, which is exactly
      // the state opening a preview leaves you in — the same trap `previewTarget` documents
      // for the title-bar button. Keybound invocations are gated on `editorTextFocus` and so
      // always have an active editor, but a palette invocation right after opening a preview
      // would silently do nothing. Fall back to the visible `.tmd` editor.
      const editor =
        vscode.window.activeTextEditor ??
        vscode.window.visibleTextEditors.find((e) => isSourceFile(e.document.fileName));
      if (!editor || !isSourceFile(editor.document.fileName)) {
        vscode.window.showWarningMessage("Taliesin: open a .tmd file first.");
        return;
      }
      const target = previews.previewFor(editor.document.fileName);
      if (!target) {
        vscode.window.showWarningMessage(
          previews.size > 1
            ? "Taliesin: several previews are open. Open this document's preview to reveal in it."
            : "Taliesin: open a preview first (Ctrl+Shift+K)."
        );
        return;
      }
      // preserveFocus: revealing must not steal the cursor from the editor the author is
      // typing in — the whole point is to look at the preview without leaving the text.
      target.panel.reveal(vscode.ViewColumn.Beside, true);
      postCursor(target, editor.document.fileName, editor.selection.active.line + 1, true);
    })
  );
  registerLanguageClient(context);
  registerCommands(context);
}

/**
 * Point the preview at the editor cursor: select the page holding it if that is not the page
 * on screen, then mark the block.
 *
 * The anchor is the page the webview is **showing**, which after a cross-page link is not the
 * document the preview was opened for — keying against the latter sends a key nothing on
 * screen can match and the mark silently lands nowhere (item 150 §4). Selecting a page is a
 * message because a webview panel and its iframe are different origins; the cursor rides
 * along in `pendingCursor` and is sent once the new page reports itself.
 */
function postCursor(p: LivePreview, editorPath: string, line: number, reveal: boolean): void {
  const pageDoc = p.currentPage?.docPath ?? p.docPath;
  const target = cursorTarget(pageDoc, p.pages, p.root, editorPath);
  if (target.navigateTo) {
    p.pendingCursor = { editorPath, line, reveal };
    p.panel.webview.postMessage({ type: "tali-navigate", url: target.navigateTo });
    return;
  }
  p.panel.webview.postMessage({ type: "tali-cursor", file: target.file, line, reveal });
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
  // A document inside a project is previewed as its PROJECT, opened at that document's page.
  // Previewing the file alone gives an orphan: no nav, no breadcrumb, and every cross-page
  // link dead (item 150). The project is a fact about the tree — the nearest `_site.yml` —
  // so there is nothing here for the author to configure.
  const root = projectRootFor(docPath);
  const key = previewKey(docPath, root);

  // Reuse before spawn. A second invocation reveals the panel it already has — and for a
  // project preview that panel is very likely showing a DIFFERENT chapter, so it is also told
  // to select this one.
  const existing = previews.get(key) ?? previews.get(docPath);
  if (existing) {
    existing.panel.reveal(vscode.ViewColumn.Beside);
    const url =
      existing.root && existing.pages
        ? pageUrlFor(existing.pages, existing.root, docPath)
        : null;
    if (url) existing.panel.webview.postMessage({ type: "tali-navigate", url });
    return;
  }
  if (!previews.beginStart(key)) return; // a start is already in flight

  const binary = vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");

  // Where the document is served inside its project. `taliesin map` owns this: `.tmd`→`.html`,
  // book chapter numbering and `index` handling all live in Rust, and a second implementation
  // here is what the LSP rewrite existed to delete. Anything that leaves us without a URL —
  // `map` unusable, or a document the project does not publish (a draft, an `{{< embed >}}`ed
  // deck) — falls back to the single-file preview rather than losing the preview.
  const pages = root ? await readSiteMap(binary, root) : null;
  const pageUrl = root && pages ? pageUrlFor(pages, root, docPath) : null;
  const site = root && pages && pageUrl ? { root, pages, pageUrl } : null;

  let server: PreviewServer;
  try {
    server = await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title: "Starting Taliesin preview…" },
      () =>
        PreviewServer.start(
          binary,
          site ? site.root : docPath,
          site ? site.root : path.dirname(docPath)
        )
    );
  } catch (e) {
    vscode.window.showErrorMessage(String((e as Error).message || e));
    previews.endStart(key); // a failed start must not wedge the document forever
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
    // A project preview is named for the PROJECT, because it does not stay on the chapter it
    // was opened at: it follows cross-page links and the editor cursor, so a title naming one
    // chapter is wrong as soon as the author moves.
    `Preview: ${path.basename(site ? site.root : docPath)}`,
    vscode.ViewColumn.Beside,
    { enableScripts: true, retainContextWhenHidden: true }
  );

  const local = await vscode.env.asExternalUri(
    vscode.Uri.parse(`http://127.0.0.1:${server.port}/${site ? site.pageUrl : ""}`)
  );
  panel.webview.html = relayHtml(local.toString(), panel.webview.cspSource);

  const entry: LivePreview = {
    panel,
    server,
    docPath,
    root: site ? site.root : null,
    pages: site ? site.pages : null,
  };
  previews.set(entry);
  previews.endStart(key);

  // forward: preview -> editor (reveal source on tali-goto), plus the page reports that keep
  // the host's idea of "which page is showing" honest.
  panel.webview.onDidReceiveMessage(
    async (m) => {
      if (!m) return;
      if (m.type === "tali-page") {
        // The webview followed a link. Record where it landed, and release a cursor that was
        // waiting on exactly this page to load.
        entry.currentPage = { docPath: m.doc_path, baseDir: m.base_dir };
        const waiting = entry.pendingCursor;
        entry.pendingCursor = null;
        if (waiting) {
          const t = cursorTarget(m.doc_path, entry.pages, entry.root, waiting.editorPath);
          // Only ever settle here. If the cursor has moved on to a third page meanwhile, the
          // selection listener will ask again — re-navigating from inside a page report is
          // how this turns into a loop.
          if (!t.navigateTo) {
            panel.webview.postMessage({
              type: "tali-cursor",
              file: t.file,
              line: waiting.line,
              reveal: waiting.reveal,
            });
          }
        }
        return;
      }
      if (m.type !== "tali-goto") return;
      // The page that sent the message supplies the directory its `source_file` is relative
      // to, so a preview that has navigated to another page resolves against THAT page
      // rather than the one this preview was opened for (item 150).
      const abs = resolveSourceFile(docPath, m.source_file ?? null, {
        baseDir: m.base_dir ?? null,
        docPath: m.doc_path ?? null,
      });
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
  //
  // In a project preview it may also SELECT a page, and that is not a hole in the split:
  // the yank this split exists to prevent ("scroll to compare two figures, type one
  // character, get pulled back") is a cursor in the page already on screen, which
  // `cursorTarget` answers with a pure mark. A cursor in another chapter is one the preview
  // is not showing at all, where staying put means showing the author a stale page.
  let timer: NodeJS.Timeout | undefined;
  const sel = vscode.window.onDidChangeTextEditorSelection((e) => {
    const f = e.textEditor.document.fileName;
    if (!isSourceFile(f)) return;
    const line = e.selections[0].active.line + 1;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => postCursor(entry, f, line, false), 80);
  });

  panel.onDidDispose(
    () => {
      previews.delete(entry);
      sel.dispose();
      if (timer) clearTimeout(timer);
      server.dispose();
    },
    undefined,
    context.subscriptions
  );
}

export function deactivate() {}
