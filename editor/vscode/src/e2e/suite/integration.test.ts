import * as assert from "node:assert";
import * as path from "node:path";
import * as vscode from "vscode";

const REPO_ROOT = path.resolve(__dirname, "../../../../../"); // out/e2e/suite -> editor/vscode -> editor -> repo
const SAMPLE_POST = path.join(REPO_ROOT, "corpus/posts/born-machines.tmd");
const SAMPLE_TMD = path.join(REPO_ROOT, "corpus/native-tmd.tmd");
const DIAG_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/diag-typo.tmd");
const QMD_FAST_BIN = path.join(REPO_ROOT, "target/debug/taliesin");

suite("qmd-fast companion (integration)", () => {
  suiteSetup(async () => {
    // The extension activates lazily (on its command); activate it explicitly so the
    // command-registration assertion doesn't race activation regardless of test order.
    const ext = vscode.extensions.getExtension("qmd-fast.qmd-fast-companion");
    assert.ok(ext, "extension should be discoverable by id");
    await ext!.activate();
  });

  test("registers the openPreview command", async () => {
    const cmds = await vscode.commands.getCommands(true);
    assert.ok(
      cmds.includes("qmdFast.openPreview"),
      "qmdFast.openPreview should be registered after activation"
    );
  });

  test("contributes the `taliesin` language and assigns it to a .tmd file", async () => {
    const langs = await vscode.languages.getLanguages();
    assert.ok(langs.includes("taliesin"), "the taliesin language is registered");
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(SAMPLE_TMD));
    assert.equal(doc.languageId, "taliesin", ".tmd resolves to the taliesin language");
  });

  test("Open Preview creates a webview panel for the active source document", async () => {
    // Point the extension at the locally-built binary (PATH may not have it in CI).
    await vscode.workspace
      .getConfiguration("qmdFast")
      .update("path", QMD_FAST_BIN, vscode.ConfigurationTarget.Global);

    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(SAMPLE_POST));
    await vscode.window.showTextDocument(doc);

    await vscode.commands.executeCommand("qmdFast.openPreview");

    // Wait for the webview tab to appear (the panel spawns the server first).
    const hasWebviewTab = await waitFor(
      () =>
        vscode.window.tabGroups.all.some((g) =>
          g.tabs.some((t) => t.input instanceof vscode.TabInputWebview)
        ),
      12000
    );
    assert.ok(hasWebviewTab, "Open Preview should open a webview panel");
  });

  test("surfaces `check` findings as diagnostics on the active .tmd", async () => {
    await vscode.workspace
      .getConfiguration("qmdFast")
      .update("path", QMD_FAST_BIN, vscode.ConfigurationTarget.Global);

    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(DIAG_FIXTURE));
    await vscode.window.showTextDocument(doc);

    // Diagnostics refresh asynchronously after open; poll until they land.
    const ok = await waitFor(() => vscode.languages.getDiagnostics(doc.uri).length > 0, 12000);
    assert.ok(ok, "check should produce at least one diagnostic for the typo fixture");

    const diags = vscode.languages.getDiagnostics(doc.uri);
    const typo = diags.find((d) => d.message.includes("titel"));
    assert.ok(typo, `expected a diagnostic mentioning the typo'd key: ${JSON.stringify(diags.map((d) => d.message))}`);
    assert.equal(typo!.range.start.line, 2, "the `titel` typo is on line 3 (0-based line 2)");
    assert.equal(typo!.severity, vscode.DiagnosticSeverity.Warning);
  });
});

function waitFor(pred: () => boolean, timeoutMs: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve) => {
    const tick = () => {
      if (pred()) return resolve(true);
      if (Date.now() > deadline) return resolve(false);
      setTimeout(tick, 200);
    };
    tick();
  });
}
