import * as assert from "assert";
import * as fs from "fs";
import * as http from "http";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { cursorMessageFor, getPreview, gotoSource, parseSourcepos } from "../../extension";

// out/test/suite -> out/test -> out -> extension -> repo root
const REPO = path.resolve(__dirname, "../../../..");
const QMD_FAST_BIN = path.join(REPO, "target", "debug", "qmd-fast");

function ping(port: number): Promise<void> {
  return new Promise((resolve, reject) => {
    const req = http.get({ host: "127.0.0.1", port, path: "/", timeout: 1000 }, (res) => {
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

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

suite("qmd-fast extension", () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "qmd-ext-"));
  const main = path.join(tmp, "doc.qmd");
  const inc = path.join(tmp, "inc.qmd");

  suiteSetup(async () => {
    fs.writeFileSync(
      main,
      "---\ntitle: Test\n---\n\n# Heading\n\nA paragraph on line 7.\n\n{{< include inc.qmd >}}\n",
    );
    fs.writeFileSync(inc, "Included line one.\n\nIncluded line two.\n");
    // Point the extension at the freshly-built binary so it doesn't rely on PATH.
    await vscode.workspace
      .getConfiguration("qmd-fast")
      .update("serverPath", QMD_FAST_BIN, vscode.ConfigurationTarget.Global);
    // The extension activates lazily on its command; force activation so
    // registration assertions don't race it.
    await vscode.extensions.getExtension("ajbogo9.qmd-fast")?.activate();
  });

  suiteTeardown(() => {
    const pv = getPreview(main);
    pv?.panel.dispose();
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  test("the built qmd-fast binary exists", () => {
    assert.ok(fs.existsSync(QMD_FAST_BIN), `expected binary at ${QMD_FAST_BIN} (run cargo build)`);
  });

  test("openPreview command is registered", async () => {
    const cmds = await vscode.commands.getCommands(true);
    assert.ok(cmds.includes("qmd-fast.openPreview"));
  });

  test("parseSourcepos maps 1-based sourcepos to a 0-based range", () => {
    const r = parseSourcepos("7:1-7:23");
    assert.ok(r);
    assert.strictEqual(r!.start.line, 6);
    assert.strictEqual(r!.start.character, 0);
    assert.strictEqual(r!.end.line, 6);
    assert.strictEqual(r!.end.character, 22);
    assert.strictEqual(parseSourcepos(""), undefined);
    assert.strictEqual(parseSourcepos(null), undefined);
  });

  test("cursorMessageFor maps the primary file and included files", () => {
    // Cursor in the primary doc -> no source_file, 1-based line preserved.
    assert.deepStrictEqual(cursorMessageFor(main, main, 7), {
      type: "qmd-cursor",
      file: null,
      line: 7,
    });
    // Cursor in an included file under the doc's dir -> its relative path.
    assert.deepStrictEqual(cursorMessageFor(main, inc, 3), {
      type: "qmd-cursor",
      file: "inc.qmd",
      line: 3,
    });
    // A file outside the doc's directory is not one of its blocks.
    assert.strictEqual(cursorMessageFor(main, path.join(os.tmpdir(), "elsewhere.qmd"), 1), null);
  });

  test("gotoSource jumps to the block in the primary file", async () => {
    await gotoSource(main, tmp, { type: "qmd-goto", source_file: null, sourcepos: "7:1-7:23" });
    const ed = vscode.window.activeTextEditor;
    assert.ok(ed);
    assert.strictEqual(ed!.document.uri.fsPath, main);
    assert.strictEqual(ed!.selection.active.line, 6); // 1-based line 7 -> 0-based 6
  });

  test("gotoSource jumps into an included file", async () => {
    await gotoSource(main, tmp, { type: "qmd-goto", source_file: "inc.qmd", sourcepos: "3:1-3:18" });
    const ed = vscode.window.activeTextEditor;
    assert.ok(ed);
    assert.strictEqual(ed!.document.uri.fsPath, inc);
    assert.strictEqual(ed!.selection.active.line, 2);
  });

  test("openPreview spawns the server and creates a live webview", async () => {
    const doc = await vscode.workspace.openTextDocument(main);
    await vscode.window.showTextDocument(doc);

    await vscode.commands.executeCommand("qmd-fast.openPreview");

    const pv = getPreview(main);
    assert.ok(pv, "a preview was registered for the document");
    assert.ok(pv!.port > 0, "a port was assigned");
    assert.ok(
      pv!.panel.webview.html.includes(`http://127.0.0.1:${pv!.port}/`),
      "webview iframe points at the spawned server",
    );

    // The spawned server actually responds.
    let ok = false;
    for (let i = 0; i < 20 && !ok; i++) {
      try {
        await ping(pv!.port);
        ok = true;
      } catch {
        await delay(200);
      }
    }
    assert.ok(ok, "the spawned qmd-fast server responds on its port");

    pv!.panel.dispose(); // kills the child server
  });
});
