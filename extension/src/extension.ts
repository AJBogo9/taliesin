// qmd-fast VS Code extension.
//
// Spawns `qmd-fast serve` for the active .qmd file and hosts the live preview
// in a webview (an iframe pointed at the server). Save-triggered block updates
// flow through the server's file watcher automatically; double-clicking a block
// in the preview jumps the editor to that block's source (click-to-source).

import * as vscode from "vscode";
import * as cp from "child_process";
import * as http from "http";
import * as net from "net";
import * as path from "path";
import * as fs from "fs";

interface Preview {
  panel: vscode.WebviewPanel;
  child: cp.ChildProcess;
}

interface GotoMessage {
  type: "qmd-goto";
  source_file: string | null;
  sourcepos: string | null;
}

const previews = new Map<string, Preview>();

export function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(
    vscode.commands.registerCommand("qmd-fast.openPreview", () => openPreview()),
  );
}

export function deactivate() {
  for (const { child } of previews.values()) {
    child.kill();
  }
  previews.clear();
}

async function openPreview() {
  const editor = vscode.window.activeTextEditor;
  if (!editor || path.extname(editor.document.fileName) !== ".qmd") {
    vscode.window.showWarningMessage("qmd-fast: open a .qmd file first.");
    return;
  }
  const file = editor.document.uri.fsPath;

  const existing = previews.get(file);
  if (existing) {
    existing.panel.reveal(vscode.ViewColumn.Beside);
    return;
  }

  const baseDir = path.dirname(file);
  const bin = resolveBinary(baseDir);

  let port: number;
  try {
    port = await freePort();
  } catch {
    vscode.window.showErrorMessage("qmd-fast: could not find a free port.");
    return;
  }

  const child = cp.spawn(bin, ["serve", file, String(port)], { cwd: baseDir });
  let stderr = "";
  child.stderr?.on("data", (d) => (stderr += d.toString()));
  child.on("error", (err) =>
    vscode.window.showErrorMessage(`qmd-fast: failed to launch '${bin}': ${err.message}`),
  );

  const panel = vscode.window.createWebviewPanel(
    "qmdFastPreview",
    `Preview: ${path.basename(file)}`,
    vscode.ViewColumn.Beside,
    { enableScripts: true, retainContextWhenHidden: true },
  );

  try {
    await waitForServer(port);
  } catch {
    panel.dispose();
    child.kill();
    vscode.window.showErrorMessage(
      `qmd-fast: the server did not start.${stderr ? " " + stderr.trim() : ""}`,
    );
    return;
  }

  panel.webview.html = webviewHtml(port);

  const messageSub = panel.webview.onDidReceiveMessage((msg: GotoMessage) => {
    if (msg && msg.type === "qmd-goto") {
      void gotoSource(file, baseDir, msg);
    }
  });

  previews.set(file, { panel, child });
  panel.onDidDispose(() => {
    messageSub.dispose();
    child.kill();
    previews.delete(file);
  });
}

/// Open the (possibly included) source file and reveal the block's range.
async function gotoSource(mainFile: string, baseDir: string, msg: GotoMessage) {
  const target = msg.source_file
    ? path.isAbsolute(msg.source_file)
      ? msg.source_file
      : path.join(baseDir, msg.source_file)
    : mainFile;

  try {
    const doc = await vscode.workspace.openTextDocument(target);
    const editor = await vscode.window.showTextDocument(doc, {
      viewColumn: vscode.ViewColumn.One,
      preserveFocus: false,
    });
    const range = parseSourcepos(msg.sourcepos);
    if (range) {
      editor.selection = new vscode.Selection(range.start, range.start);
      editor.revealRange(range, vscode.TextEditorRevealType.InCenter);
    }
  } catch (e) {
    vscode.window.showErrorMessage(`qmd-fast: cannot open ${target}: ${e}`);
  }
}

/// Parse a "startLine:startCol-endLine:endCol" sourcepos (1-based) into a Range.
function parseSourcepos(sp: string | null): vscode.Range | undefined {
  if (!sp) return undefined;
  const m = /^(\d+):(\d+)-(\d+):(\d+)$/.exec(sp);
  if (!m) return undefined;
  const start = new vscode.Position(Math.max(0, +m[1] - 1), Math.max(0, +m[2] - 1));
  const end = new vscode.Position(Math.max(0, +m[3] - 1), Math.max(0, +m[4] - 1));
  return new vscode.Range(start, end);
}

/// Locate the qmd-fast binary: explicit setting, then the workspace/doc Cargo
/// target dirs, then PATH.
function resolveBinary(docDir: string): string {
  const configured = vscode.workspace.getConfiguration("qmd-fast").get<string>("serverPath");
  if (configured && configured.trim()) return configured.trim();

  const roots = [vscode.workspace.workspaceFolders?.[0]?.uri.fsPath, docDir].filter(
    (r): r is string => !!r,
  );
  for (const root of roots) {
    for (const profile of ["release", "debug"]) {
      const candidate = path.join(root, "target", profile, "qmd-fast");
      if (fs.existsSync(candidate)) return candidate;
    }
  }
  return "qmd-fast";
}

function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.unref();
    srv.on("error", reject);
    srv.listen(0, "127.0.0.1", () => {
      const addr = srv.address();
      if (addr && typeof addr === "object") {
        const port = addr.port;
        srv.close(() => resolve(port));
      } else {
        srv.close(() => reject(new Error("no port")));
      }
    });
  });
}

function pingServer(port: number): Promise<void> {
  return new Promise((resolve, reject) => {
    const req = http.get({ host: "127.0.0.1", port, path: "/", timeout: 800 }, (res) => {
      res.resume();
      resolve();
    });
    req.on("error", reject);
    req.on("timeout", () => {
      req.destroy();
      reject(new Error("timeout"));
    });
  });
}

async function waitForServer(port: number, timeoutMs = 8000): Promise<void> {
  const start = Date.now();
  for (;;) {
    try {
      await pingServer(port);
      return;
    } catch {
      if (Date.now() - start > timeoutMs) throw new Error("server did not start");
      await new Promise((r) => setTimeout(r, 150));
    }
  }
}

/// The webview shell: an iframe to the dev server, plus a relay that forwards
/// the iframe's `qmd-goto` messages to the extension host.
function webviewHtml(port: number): string {
  const src = `http://127.0.0.1:${port}/`;
  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8" />
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; frame-src http://127.0.0.1:* http://localhost:*; script-src 'unsafe-inline'; style-src 'unsafe-inline';" />
<style>html, body { margin: 0; padding: 0; height: 100%; } iframe { display: block; border: 0; width: 100%; height: 100vh; }</style>
</head>
<body>
<iframe id="preview" src="${src}"></iframe>
<script>
  const vscode = acquireVsCodeApi();
  window.addEventListener("message", (e) => {
    const m = e.data;
    if (m && m.type === "qmd-goto") vscode.postMessage(m);
  });
</script>
</body>
</html>`;
}
