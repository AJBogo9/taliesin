# VS Code Editor Companion (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A VS Code extension that hosts the `qmd-fast` live preview in a webview and wires bidirectional source sync — Alt-click in the preview reveals the source line; moving the editor cursor highlights the matching block (and jumps deck slides).

**Architecture:** Three processes, two message hops. The extension host (Node/TS) spawns `qmd-fast preview <file> <port>` and opens a `WebviewPanel`; the webview loads a small relay document that embeds the preview in an `<iframe>` (via `asExternalUri`). The relay bridges the iframe's `window.postMessage` protocol (`qmd-goto` up, `qmd-cursor` down) to VS Code webview messaging. The extension resolves `qmd-goto` to a `revealRange`, and posts `qmd-cursor` on selection change.

**Tech Stack:** TypeScript, `@types/vscode`, esbuild (bundler), Node child_process, the VS Code Webview + Window APIs. No runtime npm deps beyond the VS Code API.

## Global Constraints

- **Preview is read-only forever.** The extension only navigates (reveal source) and highlights (cursor sync). NO buffer mutation from the preview, NO preview write-back. (Phase 2's editor commands are out of scope here.)
- **Localhost only in Phase 1.** Spawn `qmd-fast preview` WITHOUT `--host`; bind `127.0.0.1`. No LAN token needed yet (the `?token=` seam for backlog #1d is documented, not implemented).
- **Protocol is fixed by the existing client** (`web-client/client.js`), do not change it:
  - preview → host: `{ type: "qmd-goto", source_file: string|null, sourcepos: "L:C" }`
  - host → preview: `{ type: "qmd-cursor", file: string|null, line: number }`
- **Location:** `editor/vscode/` (new top-level subproject).
- **Binary discovery:** a `qmdFast.path` setting, default `"qmd-fast"` (the PATH launcher).
- **Phase 1 is F5-dev** (run from source in the Extension Development Host); no `.vsix` packaging.
- **Node ≥ 18, VS Code engine `^1.85.0`.**
- The extension is NOT part of the Rust workspace; it has its own `package.json` + build. The Rust `cargo test` suite must stay untouched and green.

---

## File Structure

- `editor/vscode/package.json` — extension manifest (engine, command, activation, setting, scripts).
- `editor/vscode/tsconfig.json` — TS config (CommonJS, ES2021, strict).
- `editor/vscode/.vscodeignore` + `.gitignore` — exclude `node_modules`, `out`.
- `editor/vscode/src/ports.ts` — pure: pick a free TCP port; wait for HTTP 200.
- `editor/vscode/src/paths.ts` — pure: parse `sourcepos` → `{line,col}`; resolve a `source_file` (relative|null) against the previewed doc's path → absolute fs path.
- `editor/vscode/src/server.ts` — spawn/track/kill the `qmd-fast preview` child process.
- `editor/vscode/src/webview.ts` — build the relay HTML (iframe + bridge script + CSP).
- `editor/vscode/src/extension.ts` — activate(): command, panel, wiring, selection listener.
- `editor/vscode/src/test/ports.test.ts`, `paths.test.ts` — unit tests for the pure modules (Node test runner).
- `editor/vscode/README.md` — what it is + the F5 manual-verification steps.

Pure logic (`ports.ts`, `paths.ts`) is unit-tested with the built-in `node:test` runner (no VS Code host needed). `server.ts`/`webview.ts`/`extension.ts` touch the VS Code/OS surface and are verified manually in the Extension Development Host (steps in Task 7).

---

### Task 1: Scaffold the extension subproject

**Files:**
- Create: `editor/vscode/package.json`
- Create: `editor/vscode/tsconfig.json`
- Create: `editor/vscode/.gitignore`
- Create: `editor/vscode/.vscodeignore`

**Interfaces:**
- Consumes: nothing.
- Produces: an installable/buildable extension skeleton; npm scripts `build` (esbuild bundle → `out/extension.js`) and `test` (compile + `node --test out/test`).

- [ ] **Step 1: Write `package.json`**

```json
{
  "name": "qmd-fast-companion",
  "displayName": "qmd-fast Companion",
  "description": "Live preview + bidirectional source sync for qmd-fast .qmd documents.",
  "version": "0.1.0",
  "publisher": "qmd-fast",
  "private": true,
  "engines": { "vscode": "^1.85.0", "node": ">=18" },
  "categories": ["Other"],
  "activationEvents": [],
  "main": "./out/extension.js",
  "contributes": {
    "commands": [
      { "command": "qmdFast.openPreview", "title": "qmd-fast: Open Preview", "category": "qmd-fast" }
    ],
    "menus": {
      "editor/title": [
        { "command": "qmdFast.openPreview", "when": "resourceExtname == .qmd", "group": "navigation" }
      ]
    },
    "configuration": {
      "title": "qmd-fast",
      "properties": {
        "qmdFast.path": {
          "type": "string",
          "default": "qmd-fast",
          "description": "Path to the qmd-fast executable (default: the qmd-fast launcher on PATH)."
        }
      }
    }
  },
  "scripts": {
    "build": "esbuild src/extension.ts --bundle --outfile=out/extension.js --external:vscode --format=cjs --platform=node",
    "compile-tests": "tsc -p . --outDir out",
    "test": "npm run compile-tests && node --test out/test"
  },
  "devDependencies": {
    "@types/node": "^20.0.0",
    "@types/vscode": "^1.85.0",
    "esbuild": "^0.21.0",
    "typescript": "^5.4.0"
  }
}
```

- [ ] **Step 2: Write `tsconfig.json`**

```json
{
  "compilerOptions": {
    "module": "commonjs",
    "target": "ES2021",
    "lib": ["ES2021"],
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "rootDir": "src",
    "outDir": "out"
  },
  "include": ["src"]
}
```

- [ ] **Step 3: Write `.gitignore`**

```
node_modules/
out/
*.vsix
```

- [ ] **Step 4: Write `.vscodeignore`**

```
src/
node_modules/
tsconfig.json
**/*.map
```

- [ ] **Step 5: Install deps + verify the skeleton builds a no-op**

Create a one-line `src/extension.ts` stub: `export function activate() {}` and `export function deactivate() {}`.
Run: `cd editor/vscode && npm install && npm run build`
Expected: `out/extension.js` is produced, no errors. (Remove the stub's body in Task 6.)

- [ ] **Step 6: Commit**

```bash
git add editor/vscode/package.json editor/vscode/tsconfig.json editor/vscode/.gitignore editor/vscode/.vscodeignore editor/vscode/src/extension.ts
git commit -m "feat(vscode): scaffold qmd-fast companion extension"
```

---

### Task 2: Port selection + readiness wait (`ports.ts`)

**Files:**
- Create: `editor/vscode/src/ports.ts`
- Test: `editor/vscode/src/test/ports.test.ts`

**Interfaces:**
- Produces:
  - `freePort(): Promise<number>` — an OS-assigned free TCP port (bind to `127.0.0.1:0`, read `.port`, close).
  - `waitForHttp(port: number, timeoutMs: number): Promise<boolean>` — polls `http://127.0.0.1:{port}/` until a response or timeout; resolves `true` on any HTTP status, `false` on timeout.

- [ ] **Step 1: Write the failing test**

```ts
import { test } from "node:test";
import assert from "node:assert";
import * as http from "node:http";
import { freePort, waitForHttp } from "../ports";

test("freePort returns a usable port", async () => {
  const p = await freePort();
  assert.ok(p > 0 && p < 65536);
});

test("waitForHttp resolves true once a server answers", async () => {
  const p = await freePort();
  const srv = http.createServer((_req, res) => res.end("ok"));
  await new Promise<void>((r) => srv.listen(p, "127.0.0.1", r));
  try {
    assert.equal(await waitForHttp(p, 2000), true);
  } finally {
    srv.close();
  }
});

test("waitForHttp resolves false when nothing answers", async () => {
  const p = await freePort(); // free, nothing listening
  assert.equal(await waitForHttp(p, 600), false);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd editor/vscode && npm run test`
Expected: FAIL — `Cannot find module '../ports'`.

- [ ] **Step 3: Write `ports.ts`**

```ts
import * as net from "node:net";
import * as http from "node:http";

export function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.on("error", reject);
    srv.listen(0, "127.0.0.1", () => {
      const addr = srv.address();
      const port = typeof addr === "object" && addr ? addr.port : 0;
      srv.close(() => (port ? resolve(port) : reject(new Error("no port"))));
    });
  });
}

export function waitForHttp(port: number, timeoutMs: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve) => {
    const tryOnce = () => {
      const req = http.get({ host: "127.0.0.1", port, path: "/", timeout: 500 }, (res) => {
        res.resume();
        resolve(true);
      });
      req.on("error", () => (Date.now() < deadline ? setTimeout(tryOnce, 120) : resolve(false)));
      req.on("timeout", () => { req.destroy(); });
    };
    tryOnce();
  });
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd editor/vscode && npm run test`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add editor/vscode/src/ports.ts editor/vscode/src/test/ports.test.ts
git commit -m "feat(vscode): free-port pick + HTTP readiness wait"
```

---

### Task 3: sourcepos + source-file path resolution (`paths.ts`)

**Files:**
- Create: `editor/vscode/src/paths.ts`
- Test: `editor/vscode/src/test/paths.test.ts`

**Interfaces:**
- Produces:
  - `parseSourcepos(sp: string): { line: number; col: number } | null` — parse the leading `L:C` of a `qmd-goto` sourcepos; `null` if it doesn't start with digits.
  - `resolveSourceFile(docPath: string, sourceFile: string | null): string` — the absolute fs path for a `qmd-goto` target: `docPath` when `sourceFile` is null, else `sourceFile` resolved against `dirname(docPath)`.
  - `relativeKey(docPath: string, editorPath: string): string | null` — the inverse for cursor-sync: `null` when `editorPath === docPath` (the main doc), else the path of `editorPath` relative to `dirname(docPath)` (POSIX separators), or `undefined`-style `""`-guarded when unrelated → return the relative path regardless (the preview only highlights if it matches a `source_file`).

- [ ] **Step 1: Write the failing test**

```ts
import { test } from "node:test";
import assert from "node:assert";
import { parseSourcepos, resolveSourceFile, relativeKey } from "../paths";

test("parseSourcepos reads leading L:C", () => {
  assert.deepEqual(parseSourcepos("12:3-14:7"), { line: 12, col: 3 });
  assert.deepEqual(parseSourcepos("5:1"), { line: 5, col: 1 });
  assert.equal(parseSourcepos("garbage"), null);
});

test("resolveSourceFile: null = the doc itself", () => {
  assert.equal(resolveSourceFile("/p/post/index.qmd", null), "/p/post/index.qmd");
});

test("resolveSourceFile: relative is joined to the doc's dir", () => {
  assert.equal(
    resolveSourceFile("/p/post/index.qmd", "../_includes/x.qmd"),
    "/p/_includes/x.qmd"
  );
});

test("relativeKey: the main doc maps to null", () => {
  assert.equal(relativeKey("/p/post/index.qmd", "/p/post/index.qmd"), null);
});

test("relativeKey: an included file maps to its relative path", () => {
  assert.equal(relativeKey("/p/post/index.qmd", "/p/_includes/x.qmd"), "../_includes/x.qmd");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd editor/vscode && npm run test`
Expected: FAIL — `Cannot find module '../paths'`.

- [ ] **Step 3: Write `paths.ts`**

```ts
import * as path from "node:path";

export function parseSourcepos(sp: string): { line: number; col: number } | null {
  const m = /^(\d+):(\d+)/.exec(sp || "");
  return m ? { line: +m[1], col: +m[2] } : null;
}

export function resolveSourceFile(docPath: string, sourceFile: string | null): string {
  if (!sourceFile) return docPath;
  return path.resolve(path.dirname(docPath), sourceFile);
}

export function relativeKey(docPath: string, editorPath: string): string | null {
  if (path.resolve(editorPath) === path.resolve(docPath)) return null;
  const rel = path.relative(path.dirname(docPath), editorPath);
  return rel.split(path.sep).join("/"); // POSIX separators for the protocol
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd editor/vscode && npm run test`
Expected: PASS (all paths + ports tests).

- [ ] **Step 5: Commit**

```bash
git add editor/vscode/src/paths.ts editor/vscode/src/test/paths.test.ts
git commit -m "feat(vscode): sourcepos parse + source-file path mapping"
```

---

### Task 4: Server process lifecycle (`server.ts`)

**Files:**
- Create: `editor/vscode/src/server.ts`

**Interfaces:**
- Consumes: `freePort`, `waitForHttp` (Task 2).
- Produces:
  - `class PreviewServer { readonly port: number; constructor(...); static async start(binary: string, file: string): Promise<PreviewServer>; dispose(): void; }`
  - `start` picks a free port, spawns `binary preview <file> <port>` (cwd = `dirname(file)`), waits up to ~8 s for HTTP 200, resolves the instance (or throws on timeout / spawn error). `dispose` SIGTERM-kills the child.

- [ ] **Step 1: Write `server.ts`** (manually verified in Task 7 — no unit test, it spawns a real binary)

```ts
import { spawn, ChildProcess } from "node:child_process";
import * as path from "node:path";
import { freePort, waitForHttp } from "./ports";

export class PreviewServer {
  private constructor(readonly port: number, private readonly child: ChildProcess) {}

  static async start(binary: string, file: string): Promise<PreviewServer> {
    const port = await freePort();
    const child = spawn(binary, ["preview", file, String(port)], {
      cwd: path.dirname(file),
      stdio: "ignore",
    });
    const spawnError = new Promise<never>((_, reject) =>
      child.on("error", (e) => reject(new Error(`failed to launch \`${binary}\`: ${e.message}`)))
    );
    const ready = waitForHttp(port, 8000).then((ok) => {
      if (!ok) throw new Error(`qmd-fast preview did not answer on ${port} within 8s`);
      return new PreviewServer(port, child);
    });
    return Promise.race([ready, spawnError]);
  }

  dispose(): void {
    try { this.child.kill("SIGTERM"); } catch { /* already gone */ }
  }
}
```

- [ ] **Step 2: Build to type-check**

Run: `cd editor/vscode && npm run build`
Expected: bundles without TS errors.

- [ ] **Step 3: Commit**

```bash
git add editor/vscode/src/server.ts
git commit -m "feat(vscode): spawn/kill the qmd-fast preview server"
```

---

### Task 5: Webview relay HTML (`webview.ts`)

**Files:**
- Create: `editor/vscode/src/webview.ts`

**Interfaces:**
- Produces: `relayHtml(iframeSrc: string, cspSource: string): string` — the webview document: a full-bleed `<iframe src=iframeSrc>` + a bridge `<script>` that (a) forwards any `message` whose `data.type === "qmd-goto"` from the iframe up to the host via `acquireVsCodeApi().postMessage`, and (b) forwards any `message` whose `data.type === "qmd-cursor"` from the host down into `iframe.contentWindow.postMessage(data, "*")`. CSP must allow `frame-src {iframeSrc origin}` and the inline bridge script (via a nonce).

- [ ] **Step 1: Write `webview.ts`**

```ts
export function relayHtml(iframeSrc: string, cspSource: string): string {
  const nonce = Math.random().toString(36).slice(2); // host-side only; not a security boundary
  const origin = new URL(iframeSrc).origin;
  return `<!DOCTYPE html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none';
  frame-src ${origin} ${cspSource}; script-src 'nonce-${nonce}'; style-src 'unsafe-inline';">
<style>html,body,iframe{margin:0;padding:0;border:0;width:100%;height:100vh;display:block}</style>
</head><body>
<iframe id="qmd" src="${iframeSrc}" allow="clipboard-read; clipboard-write"></iframe>
<script nonce="${nonce}">
  const vscode = acquireVsCodeApi();
  const iframe = document.getElementById("qmd");
  // iframe (preview) -> host
  window.addEventListener("message", (e) => {
    const m = e.data;
    if (!m || typeof m !== "object") return;
    if (m.type === "qmd-goto") { vscode.postMessage(m); return; }
    // host -> iframe (the extension posts qmd-cursor to THIS window)
    if (m.type === "qmd-cursor" && iframe.contentWindow) {
      iframe.contentWindow.postMessage(m, "*");
    }
  });
</script>
</body></html>`;
}
```

> Note: the extension posts `qmd-cursor` via `panel.webview.postMessage`, which arrives as a `message` event on the webview `window` — the same handler routes it into the iframe. `qmd-goto` arrives from the iframe (a different source) and is routed up. The `type` discriminates direction.

- [ ] **Step 2: Build to type-check**

Run: `cd editor/vscode && npm run build`
Expected: bundles without errors.

- [ ] **Step 3: Commit**

```bash
git add editor/vscode/src/webview.ts
git commit -m "feat(vscode): webview relay HTML bridging iframe <-> host"
```

---

### Task 6: Wire it together (`extension.ts`)

**Files:**
- Modify: `editor/vscode/src/extension.ts` (replace the Task 1 stub)

**Interfaces:**
- Consumes: `PreviewServer.start` (Task 4), `relayHtml` (Task 5), `parseSourcepos`/`resolveSourceFile`/`relativeKey` (Task 3).
- Produces: `activate(context)` registering `qmdFast.openPreview`; `deactivate()`.

- [ ] **Step 1: Write `extension.ts`**

```ts
import * as vscode from "vscode";
import { PreviewServer } from "./server";
import { relayHtml } from "./webview";
import { parseSourcepos, resolveSourceFile, relativeKey } from "./paths";

export function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(
    vscode.commands.registerCommand("qmdFast.openPreview", () => openPreview(context))
  );
}

async function openPreview(context: vscode.ExtensionContext) {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId === undefined || !editor.document.fileName.endsWith(".qmd")) {
    vscode.window.showWarningMessage("qmd-fast: open a .qmd file first.");
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

  const local = await vscode.env.asExternalUri(vscode.Uri.parse(`http://127.0.0.1:${server.port}/`));
  panel.webview.html = relayHtml(local.toString(), panel.webview.cspSource);

  // forward: preview -> editor (reveal source on qmd-goto)
  panel.webview.onDidReceiveMessage(async (m) => {
    if (!m || m.type !== "qmd-goto") return;
    const abs = resolveSourceFile(docPath, m.source_file ?? null);
    const pos = parseSourcepos(m.sourcepos || "") || { line: 1, col: 1 };
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(abs));
    const ed = await vscode.window.showTextDocument(doc, vscode.ViewColumn.One);
    const p = new vscode.Position(Math.max(0, pos.line - 1), Math.max(0, pos.col - 1));
    ed.selection = new vscode.Selection(p, p);
    ed.revealRange(new vscode.Range(p, p), vscode.TextEditorRevealType.InCenterIfOutsideViewport);
  }, undefined, context.subscriptions);

  // reverse: editor cursor -> preview (debounced qmd-cursor)
  let timer: NodeJS.Timeout | undefined;
  const sel = vscode.window.onDidChangeTextEditorSelection((e) => {
    const f = e.textEditor.document.fileName;
    if (!f.endsWith(".qmd")) return;
    const key = relativeKey(docPath, f);
    const line = e.selections[0].active.line + 1;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => panel.webview.postMessage({ type: "qmd-cursor", file: key, line }), 80);
  });

  panel.onDidDispose(() => { sel.dispose(); if (timer) clearTimeout(timer); server.dispose(); }, undefined, context.subscriptions);
}

export function deactivate() {}
```

- [ ] **Step 2: Build to type-check**

Run: `cd editor/vscode && npm run build`
Expected: bundles `out/extension.js` without TS errors.

- [ ] **Step 3: Run the unit suite (pure modules still green)**

Run: `cd editor/vscode && npm run test`
Expected: PASS (ports + paths tests).

- [ ] **Step 4: Commit**

```bash
git add editor/vscode/src/extension.ts
git commit -m "feat(vscode): wire preview panel + bidirectional source sync"
```

---

### Task 7: Manual verification harness + README

**Files:**
- Create: `editor/vscode/README.md`
- Create: `editor/vscode/.vscode/launch.json` (F5 → Extension Development Host)

**Interfaces:**
- Consumes: the full extension.
- Produces: a launch config + documented author-side verification steps (this is where the loop is closed, since it can't be headless).

- [ ] **Step 1: Write `.vscode/launch.json`**

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Run qmd-fast Companion",
      "type": "extensionHost",
      "request": "launch",
      "args": ["--extensionDevelopmentPath=${workspaceFolder}/editor/vscode"],
      "outFiles": ["${workspaceFolder}/editor/vscode/out/**/*.js"],
      "preLaunchTask": "${defaultBuildTask}"
    }
  ]
}
```

- [ ] **Step 2: Write `README.md`** with the manual verification checklist

```markdown
# qmd-fast Companion (Phase 1)

Hosts the qmd-fast live preview in a VS Code webview with bidirectional source sync.

## Develop / run
1. `cd editor/vscode && npm install && npm run build`
2. Open `editor/vscode` in VS Code, press **F5** (Run qmd-fast Companion). A second
   "Extension Development Host" window opens.
3. Ensure `qmd-fast` is on PATH (or set `qmdFast.path`).

## Manual verification (the loop the headless tests can't close)
In the Extension Development Host:
1. Open `corpus/posts/em-algorithm/index.qmd`. Run **qmd-fast: Open Preview** (cmd palette
   or the editor-title button). The preview opens beside the editor and renders.
2. **Reverse sync:** move the cursor onto a heading / paragraph — the matching block in the
   preview gains the `.qmd-hl` outline and scrolls into view.
3. **Forward sync:** Alt-click a block in the preview — the editor cursor jumps to that
   block's source line.
4. **Deck:** open `corpus/liquid-glass-slides/example.qmd`, Open Preview, move the cursor
   into a later slide's content — the deck jumps to that slide.
5. Close the preview panel — the spawned `qmd-fast preview` process exits.
```

- [ ] **Step 3: Author runs the manual checklist**

Run: F5, then steps 1-5 above.
Expected: all five behaviors hold; no orphaned `qmd-fast` process after closing the panel.

- [ ] **Step 4: Commit**

```bash
git add editor/vscode/README.md editor/vscode/.vscode/launch.json
git commit -m "docs(vscode): F5 launch config + manual verification steps"
```

---

## Self-Review

**Spec coverage:** host/spawn (Tasks 1,4) · webview + asExternalUri + CSP (Tasks 5,6) ·
relay both hops (Task 5) · forward `qmd-goto`→reveal (Task 6) · reverse cursor→`qmd-cursor`
debounced (Task 6) · localhost-only/no-token (Global Constraints; server.ts spawns without
`--host`) · binary discovery setting (Task 1 + 6) · source-file mapping (Task 3) · lifecycle
kill (Tasks 4,6) · verification (Task 7). The four spec "decisions to confirm" are resolved
in Global Constraints (location `editor/vscode/`, `qmdFast.path` default `qmd-fast`, F5-dev,
main-doc-first via `relativeKey`).

**Placeholder scan:** none — every file has complete content.

**Type consistency:** `freePort`/`waitForHttp` (Task 2) ↔ used in `PreviewServer.start`
(Task 4); `parseSourcepos`/`resolveSourceFile`/`relativeKey` (Task 3) ↔ used in
`extension.ts` (Task 6); `relayHtml(iframeSrc, cspSource)` (Task 5) ↔ called with
`local.toString(), panel.webview.cspSource` (Task 6); the protocol message shapes match
`web-client/client.js` verbatim.

**Open risk (flag for the implementer):** `asExternalUri` localhost iframing inside a
webview CSP is the one thing that can behave differently across VS Code versions / remote
setups; if the iframe is blocked, the fallback is a webview `portMapping` or opening the
preview in an external browser tab (`vscode.env.openExternal`) with the `vscode://` deep-link
forward-sync path that already works. Note in README if hit.
```
