import * as assert from "node:assert";
import * as path from "node:path";
import * as vscode from "vscode";
import * as http from "node:http";
import { execFileSync } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import { kernelFailure } from "../../kernelfail";

const REPO_ROOT = path.resolve(__dirname, "../../../../../"); // out/e2e/suite -> editor/vscode -> editor -> repo
const SAMPLE_POST = path.join(REPO_ROOT, "corpus/posts/born-machines.tmd");
const SAMPLE_TMD = path.join(REPO_ROOT, "corpus/native-tmd.tmd");
const DIAG_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/diag-typo.tmd");
const COMPLETE_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/complete.tmd");
const MATH_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/math.tmd");
const PATHS_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/paths.tmd");
const INCLUDE_DOC = path.join(REPO_ROOT, "corpus/single-page-report/index.tmd");
const TALIESIN_BIN = path.join(REPO_ROOT, "target/debug/taliesin");
// A real multi-chapter book from the corpus rather than a minted fixture: the corpus is the
// regression net, and `demo-book` already carries the shape this needs (five chapters, one
// `_site.yml`) while starting in a tenth of a second.
const BOOK_ROOT = path.join(REPO_ROOT, "corpus/demo-book");

// These tests exercise the companion in a real Extension Host, which is the only place the
// claims are worth anything: the language features now come from `taliesin lsp` over stdio,
// so a unit test can prove the server answers but never that VS Code asked it, wired the
// response into a provider, and rendered it.
suite("Taliesin companion (integration)", () => {
  suiteSetup(async () => {
    // Point at the locally-built binary BEFORE activation: the language client launches the
    // server on activate, and PATH may not have `taliesin` in CI. Setting it afterwards
    // would work (a config change restarts the server) but would race every first test.
    await vscode.workspace
      .getConfiguration("taliesin")
      .update("path", TALIESIN_BIN, vscode.ConfigurationTarget.Global);

    const ext = vscode.extensions.getExtension("taliesin.taliesin-companion");
    assert.ok(ext, "extension should be discoverable by id");
    await ext!.activate();

    // The server starts asynchronously. Wait until it has actually answered something
    // before any test asserts on a provider, or a slow start reads as a missing feature.
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(DIAG_FIXTURE));
    await vscode.window.showTextDocument(doc);
    const ready = await waitFor(
      () => vscode.languages.getDiagnostics(doc.uri).length > 0,
      30000
    );
    assert.ok(
      ready,
      "the language server never published diagnostics; it probably failed to start"
    );
  });

  test("registers its contributed commands", async () => {
    const cmds = await vscode.commands.getCommands(true);
    for (const id of [
      "taliesin.openPreview",
      "taliesin.restartServer",
      "taliesin.showServerLog",
      "taliesin.doctor",
      "taliesin.insertMathSymbol",
      "taliesin.revealInPreview",
    ]) {
      assert.ok(cmds.includes(id), `${id} should be registered after activation`);
    }
  });

  test("contributes the `taliesin` language and assigns it to a .tmd file", async () => {
    const langs = await vscode.languages.getLanguages();
    assert.ok(langs.includes("taliesin"), "the taliesin language is registered");
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(SAMPLE_TMD));
    assert.equal(doc.languageId, "taliesin", ".tmd resolves to the taliesin language");
  });

  // The manifest paints the `$` math delimiters bold, because no bundled theme defines a rule
  // for that scope and they are otherwise invisible. Whether VS Code *accepts* an
  // extension-contributed default for `editor.tokenColorCustomizations` is a platform question
  // the manifest and grammar tests cannot answer: both would still pass if VS Code silently
  // dropped the contribution, and the delimiters would stay plain. `inspect().defaultValue` is
  // the exact answer — it is the merged default, so our rule appearing there means the
  // contribution was read and honoured, with no user setting involved.
  test("VS Code accepts the contributed math-delimiter token colour default", async () => {
    const info = vscode.workspace.getConfiguration().inspect("editor.tokenColorCustomizations");
    const rules = (info?.defaultValue as { textMateRules?: { scope: string | string[] }[] })
      ?.textMateRules;
    assert.ok(rules, "the contributed default never reached the configuration service");
    const scopes = rules.flatMap((r) => (typeof r.scope === "string" ? [r.scope] : r.scope));
    assert.ok(
      scopes.includes("punctuation.definition.math.begin.tmd"),
      `expected the math-delimiter scope among the defaults, got ${JSON.stringify(scopes)}`
    );
  });

  test("Open Preview creates a webview panel for the active source document", async () => {
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(SAMPLE_POST));
    await vscode.window.showTextDocument(doc);

    await vscode.commands.executeCommand("taliesin.openPreview");

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

  // The reuse half of the same command. Before the preview registry, `openPreview` allocated
  // a port, spawned `taliesin preview` and created a webview on EVERY invocation, so a second
  // press left two panels and two file watchers running against one document. Counting webview
  // tabs is the observable form of that: the registry must reveal the panel it already has.
  test("Open Preview a second time reuses the panel instead of opening another", async () => {
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(SAMPLE_POST));
    await vscode.window.showTextDocument(doc);

    const webviewTabs = () =>
      vscode.window.tabGroups.all.reduce(
        (n, g) => n + g.tabs.filter((t) => t.input instanceof vscode.TabInputWebview).length,
        0
      );

    await vscode.commands.executeCommand("taliesin.openPreview");
    assert.ok(await waitFor(() => webviewTabs() >= 1, 12000), "the first preview should open");
    const after1 = webviewTabs();

    await vscode.commands.executeCommand("taliesin.openPreview");
    // Give a would-be second panel time to appear; the assertion is that it never does.
    await new Promise((r) => setTimeout(r, 2500));
    assert.strictEqual(
      webviewTabs(),
      after1,
      "a second Open Preview on the same document must reveal the existing panel, not add one"
    );
  });

  // Forward search's active half. This asserts the command is wired and survives being
  // invoked with a live preview; whether the preview actually scrolls is a client-side
  // property, pinned by the `cursor-sync` probe in tools/ui-audit.
  test("Reveal Cursor in Preview runs against a live preview without throwing", async () => {
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(SAMPLE_POST));
    const editor = await vscode.window.showTextDocument(doc);
    await vscode.commands.executeCommand("taliesin.openPreview");
    assert.ok(
      await waitFor(
        () =>
          vscode.window.tabGroups.all.some((g) =>
            g.tabs.some((t) => t.input instanceof vscode.TabInputWebview)
          ),
        12000
      ),
      "a preview must be open for the reveal command to have a target"
    );

    // Opening a preview leaves the WEBVIEW focused, so `activeTextEditor` is undefined here.
    // Invoking the command in that state is the palette case, and it must not throw or bail:
    // the command falls back to the visible .tmd editor.
    await vscode.commands.executeCommand("taliesin.revealInPreview");

    // Now the realistic keybinding case: the author is typing, so the editor holds focus.
    const focused = await vscode.window.showTextDocument(doc, vscode.ViewColumn.One);
    const line = Math.min(5, doc.lineCount - 1);
    focused.selection = new vscode.Selection(line, 0, line, 0);
    await vscode.commands.executeCommand("taliesin.revealInPreview");

    // The editor must keep focus: revealing is meant to move the preview, not the author.
    assert.strictEqual(
      vscode.window.activeTextEditor?.document.uri.fsPath,
      doc.uri.fsPath,
      "revealing must not steal focus from the editor"
    );
  });

  test("surfaces `check` findings as diagnostics, from the language server", async () => {
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(DIAG_FIXTURE));
    await vscode.window.showTextDocument(doc);

    const ok = await waitFor(() => vscode.languages.getDiagnostics(doc.uri).length > 0, 20000);
    assert.ok(ok, "check should produce at least one diagnostic for the typo fixture");

    const diags = vscode.languages.getDiagnostics(doc.uri);
    const typo = diags.find((d) => d.message.includes("titel"));
    assert.ok(
      typo,
      `expected a diagnostic mentioning the typo'd key: ${JSON.stringify(diags.map((d) => d.message))}`
    );
    assert.equal(typo!.range.start.line, 2, "the `titel` typo is on line 3 (0-based line 2)");
    assert.equal(typo!.severity, vscode.DiagnosticSeverity.Warning);
    // The server (not the old TS shim) is the source. It carries no `code`: the `TAL-*`
    // catalogue went on 2026-08-08, and a code an editor shows with nothing to look it up in
    // is a token the author cannot act on.
    assert.equal(typo!.source, "taliesin");
    assert.strictEqual(typo!.code, undefined, "no code survives the catalogue");
    // The fix travels inline in the message instead, which is what the hover shows.
    assert.ok(typo!.message.includes("did you mean"), typo!.message);
  });

  test("the doctor hint fires on a diagnostic that made the real platform round trip", async () => {
    // The trap item 219 walked into, in its 2026-08-08 form. It used to be the `code` SHAPE:
    // `code_description` made vscode-languageclient deliver `code` as `{ value, target }`
    // rather than the string the JSON wire carried, so a watcher written from memory as
    // `d.code === "TAL-KERNEL"` passed every hand-made unit fixture and never fired once in a
    // real editor. The code is gone and the hint keys on the MESSAGE, which still has to
    // survive the round trip, since a platform that reformatted it would break the match the
    // same silent way.
    const collection = vscode.languages.createDiagnosticCollection("taliesin-doctor-hint-test");
    const uri = vscode.Uri.file(path.join(os.tmpdir(), "taliesin-kernel-probe.tmd"));
    const kernel = new vscode.Diagnostic(
      new vscode.Range(0, 0, 0, 1),
      "code cell did not run (no kernel was available)",
      vscode.DiagnosticSeverity.Error
    );
    collection.set(uri, [kernel]);
    try {
      assert.strictEqual(
        kernelFailure(vscode.languages.getDiagnostics(uri)),
        "code cell did not run (no kernel was available)",
        "the doctor hint would never fire on a real kernel failure"
      );
    } finally {
      collection.dispose();
    }
  });

  test("offers cell-option and div-class completions", async () => {
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(COMPLETE_FIXTURE));
    await vscode.window.showTextDocument(doc);
    const text = doc.getText().split("\n");
    const cellLine = text.findIndex((l) => l.startsWith("#|"));
    const divLine = text.findIndex((l) => l.startsWith("::: {."));
    assert.ok(cellLine >= 0 && divLine >= 0, "fixture must contain a #| line and a ::: {. line");

    const cellLabels = await completionLabels(doc.uri, new vscode.Position(cellLine, 2));
    assert.ok(cellLabels.includes("echo"), `cell options should include echo: ${cellLabels}`);

    const divLabels = await completionLabels(doc.uri, new vscode.Position(divLine, 6));
    assert.ok(
      divLabels.includes("callout-note"),
      `div classes should include callout-note: ${divLabels}`
    );
    assert.ok(
      divLabels.includes("column-margin"),
      `div classes should include column-margin: ${divLabels}`
    );
  });

  test("offers math commands inside `$…$` and nothing in prose", async () => {
    // The gap this whole migration was measured against: `$…$` was the one place in a .tmd
    // where the editor knew nothing, even though the grammar colorized it.
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(MATH_FIXTURE));
    await vscode.window.showTextDocument(doc);
    const lines = doc.getText().split("\n");
    const mathLine = lines.findIndex((l) => l.includes("$\\al"));
    const proseLine = lines.findIndex((l) => l.startsWith("Prose"));
    assert.ok(mathLine >= 0 && proseLine >= 0, "fixture must contain a math line and a prose line");

    const inMath = await completionLabels(
      doc.uri,
      new vscode.Position(mathLine, lines[mathLine].length)
    );
    assert.ok(inMath.includes("\\alpha"), `math should offer \\alpha: ${inMath.slice(0, 20)}`);

    const inProse = await completionLabels(
      doc.uri,
      new vscode.Position(proseLine, lines[proseLine].length)
    );
    assert.ok(
      !inProse.includes("\\alpha"),
      `prose must not offer math commands: ${inProse.slice(0, 20)}`
    );
  });

  test("completes paths, shortcode names and cell-option values", async () => {
    // Three of the twelve cursor positions the audit measured as answering nothing. Each
    // was a place the author already had to know the answer before the editor would help.
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(PATHS_FIXTURE));
    await vscode.window.showTextDocument(doc);
    const lines = doc.getText().split("\n");
    const at = (needle: string) => {
      const line = lines.findIndex((l) => l.includes(needle));
      assert.ok(line >= 0, `fixture must contain ${needle}`);
      return new vscode.Position(line, lines[line].length);
    };

    const bib = await completionLabels(doc.uri, at("bibliography: re"));
    assert.ok(
      bib.includes("refs.bib"),
      `a path-valued front-matter key should offer the .bib beside it: ${bib}`
    );

    const shortcodes = await completionLabels(doc.uri, at("{{< inc"));
    assert.ok(
      shortcodes.includes("include"),
      `\`{{< \` should offer the shortcode names: ${shortcodes}`
    );

    const values = await completionLabels(doc.uri, at("#| echo: tr"));
    assert.ok(values.includes("true"), `\`echo:\` should offer true/false: ${values}`);
  });

  test("hovering an `{{< include >}}` path says where it goes", async () => {
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(INCLUDE_DOC));
    await vscode.window.showTextDocument(doc);
    const line = doc
      .getText()
      .split("\n")
      .findIndex((l) => l.trim().startsWith("{{< include"));
    const col = doc.lineAt(line).text.indexOf("subsections/");

    const text = await waitForValue(async () => {
      const hovers = (await vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        doc.uri,
        new vscode.Position(line, col + 2)
      )) as vscode.Hover[];
      const joined = (hovers ?? [])
        .flatMap((h) => h.contents)
        .map((c) => (typeof c === "string" ? c : (c as vscode.MarkdownString).value))
        .join("\n");
      return joined.includes("subsections/_introduction.tmd") ? joined : undefined;
    }, 15000);

    assert.ok(text, "hover should name the included file");
  });

  test("completes inside a `{js}` cell from the real JS language service", async () => {
    // `{js}` is the one cell language whose provider ships WITH VS Code, so this proves the
    // whole embedded path in a bare Extension Host: no Python extension required. `charAt`
    // is the load-bearing assertion — it can only come from the TS server actually typing
    // `greeting` as a string. Word-based fallback completion would offer `greeting` and
    // `const` and never `charAt`, which is exactly the failure this must not silently pass.
    const doc = await vscode.workspace.openTextDocument({
      language: "taliesin",
      content: "---\ntitle: T\n---\n\n```{js}\nconst greeting = 'hi';\ngreeting.\n```\n",
    });
    await vscode.window.showTextDocument(doc);

    const labels = await waitForValue(async () => {
      const list = (await vscode.commands.executeCommand(
        "vscode.executeCompletionItemProvider",
        doc.uri,
        new vscode.Position(6, 9) // after `greeting.`
      )) as vscode.CompletionList | undefined;
      const got = (list?.items ?? []).map((i) =>
        typeof i.label === "string" ? i.label : i.label.label
      );
      return got.includes("charAt") ? got : undefined;
    }, 20000);

    assert.ok(labels, "expected string members from the JS language service inside the cell");
  });

  test("a later cell sees an earlier cell's definitions", async () => {
    // Two things at once, both of which fail silently rather than loudly if broken:
    //   - every cell of the language is projected, not just the one under the cursor, so
    //     `greeting` declared in the first cell is in scope in the second. That matches how
    //     Taliesin runs them: one warm kernel, shared state.
    //   - the leading `#|` is stripped. It is a Taliesin directive but a SYNTAX ERROR in
    //     JavaScript, so leaving it in poisons the whole shadow buffer and `charAt` vanishes.
    const doc = await vscode.workspace.openTextDocument({
      language: "taliesin",
      content:
        "---\ntitle: T\n---\n\n```{js}\n#| echo: false\nconst greeting = 'hi';\n```\n\n" +
        "Prose between.\n\n```{js}\ngreeting.\n```\n",
    });
    await vscode.window.showTextDocument(doc);

    const labels = await waitForValue(async () => {
      const list = (await vscode.commands.executeCommand(
        "vscode.executeCompletionItemProvider",
        doc.uri,
        new vscode.Position(12, 9) // after `greeting.` in the SECOND cell
      )) as vscode.CompletionList | undefined;
      const got = (list?.items ?? []).map((i) =>
        typeof i.label === "string" ? i.label : i.label.label
      );
      return got.includes("charAt") ? got : undefined;
    }, 20000);

    assert.ok(labels, "the second cell should see `greeting` from the first");
  });

  test("hovering math previews what it renders as", async () => {
    // The server can only answer if VS Code asks, and it only asks inside a math span it
    // routed to the Taliesin server. An untitled buffer also re-proves the documentSelector
    // covers `scheme: untitled`, which the first migration got wrong.
    const doc = await vscode.workspace.openTextDocument({
      language: "taliesin",
      content: "---\ntitle: T\n---\n\nLet $\\alpha + \\beta$ stand.\n",
    });
    await vscode.window.showTextDocument(doc);

    const text = await waitForValue(async () => {
      const hovers = (await vscode.commands.executeCommand(
        "vscode.executeHoverProvider",
        doc.uri,
        new vscode.Position(4, 8) // inside `\alpha`
      )) as vscode.Hover[];
      const joined = (hovers ?? [])
        .flatMap((h) => h.contents)
        .map((c) => (typeof c === "string" ? c : (c as vscode.MarkdownString).value))
        .join("\n");
      // Two legitimate shapes, and which one arrives depends on the binary under test: a
      // a build on a host with Chrome once rasterized the real KaTeX render,
      // everything else falls back to the Unicode approximation. Waiting only for the
      // glyphs would time out precisely when the better path is working.
      const rendered = joined.includes("α+β");
      const rasterized = joined.includes("](data:image/png;base64,iVBORw0KGgo");
      return rendered || rasterized ? joined : undefined;
    }, 15000);

    assert.ok(text, "hover should preview the math, rasterized or approximated");
    if (text.includes("data:image/png")) {
      // The alt text is what a screen reader announces and what shows if the image is ever
      // dropped, so an image hover that lost it is a regression even though it looks fine.
      assert.ok(
        text.includes("![\\alpha + \\beta]"),
        `a rasterized hover must keep the source as alt text: ${text.slice(0, 80)}`
      );
    }
  });

  // Enter's default keybinding IS `type` with a `\n`, so this presses Enter the way a person
  // does and lets the editor apply `language-configuration.json`. A unit test over those
  // regexes would only prove the regexes match; it could not prove VS Code loaded the file,
  // scoped the rules to `taliesin`, or preferred the right rule when two match.
  async function pressEnterAfter(content: string): Promise<string> {
    const doc = await vscode.workspace.openTextDocument({
      language: "taliesin",
      content,
    });
    const editor = await vscode.window.showTextDocument(doc);
    // `showTextDocument` resolves before focus has landed, so wait for the document to be the
    // active editor.
    const active = await waitFor(
      () => vscode.window.activeTextEditor?.document.uri.toString() === doc.uri.toString(),
      5000
    );
    assert.ok(active, "the test document should be the active editor before typing");

    // Being *active* is not the same as having keyboard focus, and `type` is delivered to the
    // focused editor. The preview test opens a webview panel, which takes focus; when it has
    // not been given back, `type` is a silent no-op and the document comes back unchanged —
    // which reads exactly like a broken onEnterRule. This was an intermittent failure, so the
    // keystroke is confirmed rather than assumed: focus the editor group, type, and only retry
    // if the document version did not move (so a delivered keystroke is never sent twice).
    const end = doc.lineAt(doc.lineCount - 1).range.end;
    for (let attempt = 0; attempt < 5; attempt++) {
      await vscode.commands.executeCommand("workbench.action.focusActiveEditorGroup");
      const target = vscode.window.activeTextEditor ?? editor;
      target.selection = new vscode.Selection(end, end);
      const before = doc.version;
      await vscode.commands.executeCommand("type", { text: "\n" });
      if (doc.version !== before) return doc.getText();
      await new Promise((r) => setTimeout(r, 200));
    }
    assert.fail(
      `the Enter keystroke was never delivered to the editor (content: ${JSON.stringify(
        doc.getText()
      )})`
    );
  }

  test("continues a list and a blockquote on Enter, keeping the author's marker", async () => {
    assert.equal(await pressEnterAfter("- one"), "- one\n- ");
    // One rule per marker, because `appendText` takes no capture groups: a single generic
    // rule would rewrite every list to `-`.
    assert.equal(await pressEnterAfter("* one"), "* one\n* ");
    assert.equal(await pressEnterAfter("+ one"), "+ one\n+ ");
    assert.equal(await pressEnterAfter("> quoted"), "> quoted\n> ");
    // The task rule must win over the plain `-` rule it also matches.
    assert.equal(await pressEnterAfter("- [x] done"), "- [x] done\n- [ ] ");
    // Nesting: `indent: "none"` keeps the current line's indentation, then appends.
    assert.equal(await pressEnterAfter("  - deep"), "  - deep\n  - ");
  });

  test("an empty list item is how you leave the list", async () => {
    // Every continuation rule demands a `\S`, so an empty marker matches none of them and
    // Enter behaves normally. Without this there is no way out of a list but backspace.
    assert.equal(await pressEnterAfter("- one\n- "), "- one\n- \n");
    // Ordered lists are deliberately not continued: `appendText` is a literal and cannot
    // count. If this ever starts appending, it was done by binding Enter to a command —
    // read the note in language-configuration.json before accepting that trade.
    assert.equal(await pressEnterAfter("1. one"), "1. one\n");
  });

  // Item 150. A book chapter previewed on its own is an orphan page: no nav, no breadcrumb,
  // and every cross-page link dead. Opening one now serves the PROJECT at that chapter's URL,
  // so one server serves the whole book — which is also why a second chapter must reveal that
  // preview instead of spawning a second server on a second port.
  //
  // **Deliberately last in the suite, as a precaution.** This suite is load-sensitive: under a
  // loaded machine the list-continuation tests fail with "the Enter keystroke was never
  // delivered", on this branch and on `HEAD` alike (measured, four alternating pairs at
  // comparable load: zero failures either side; the earlier failures were all at load ~6-7).
  // This is the only test that leaves a SECOND preview panel in the window, and a webview
  // holding keyboard focus is exactly what makes `type` a silent no-op — so it runs where it
  // cannot contribute to that, and hands its resource to the command rather than relying on
  // which editor is focused.
  test("Open Preview on a book chapter serves the whole project, once", async () => {
    const chapter = path.join(BOOK_ROOT, "intro.tmd");
    const sibling = path.join(BOOK_ROOT, "methods.tmd");
    // Named panels, not a count: previews opened by earlier tests come and go as VS Code
    // reshuffles columns, so a count delta reads as "no preview opened" when one did.
    const previewTabs = () =>
      vscode.window.tabGroups.all.flatMap((g) =>
        g.tabs.filter((t) => t.input instanceof vscode.TabInputWebview).map((t) => t.label)
      );

    // Naming the resource is the title-bar button's path, and it is the one that does not
    // depend on which editor is focused — `activeTextEditor` is undefined whenever a webview
    // holds focus, which is the state every earlier preview test leaves behind.
    await vscode.commands.executeCommand("taliesin.openPreview", vscode.Uri.file(chapter));
    assert.ok(
      await waitFor(() => previewTabs().includes("Preview: demo-book"), 20000),
      `the book preview should open, saw ${JSON.stringify(previewTabs())}`
    );
    const after1 = previewTabs().length;

    // The claim is not "a server started" but "the server serves the BOOK". Its argv is where
    // that is observable from the host, and its port is how the page itself can be read.
    const port = await waitForValue(() => bookServerPort(), 20000);
    assert.ok(port, `a \`taliesin preview ${BOOK_ROOT}\` server should be running`);

    // The payoff, fetched from the real preview: the chapter renders with the book's sidebar,
    // which is exactly what a single-file preview of the same chapter cannot have.
    const html = await get(`http://127.0.0.1:${port}/intro.html`);
    assert.match(html, /tali-book-sidebar/, "the previewed chapter should carry the book nav");

    await vscode.commands.executeCommand("taliesin.openPreview", vscode.Uri.file(sibling));
    // Give a would-be second panel time to appear; the assertion is that it never does.
    await new Promise((r) => setTimeout(r, 2500));
    assert.strictEqual(
      previewTabs().length,
      after1,
      `a sibling chapter must reveal the book's preview, not add a second one; saw ${JSON.stringify(previewTabs())}`
    );

    // Close the panel this test opened. Nothing downstream depends on it (this test is last),
    // but the harness fails the run on a `taliesin preview` server that outlives it, and
    // disposing the panel is what reaps the child.
    const mine = vscode.window.tabGroups.all
      .flatMap((g) => g.tabs)
      .filter((t) => t.input instanceof vscode.TabInputWebview && t.label === "Preview: demo-book");
    await vscode.window.tabGroups.close(mine);
  });

  // The keybindings are a promise about keys the platform also uses. VS Code resolves a
  // conflict in the extension's favour when the `when` clause matches, so a clash does not
  // fail loudly — it silently takes a key the author has used for years everywhere else and
  // breaks it in `.tmd` files only. Nothing but the running editor knows the real default
  // table, so this reads it: `Preferences: Open Default Keyboard Shortcuts (JSON)` is that
  // table, generated from what is actually registered in this build.
  test("no new keybinding shadows a VS Code default", async () => {
    await vscode.commands.executeCommand("workbench.action.openDefaultKeybindingsFile");
    const defaults = await waitForValue(async () => {
      const doc = vscode.workspace.textDocuments.find(
        (d) => d.uri.path.endsWith("keybindings.json") && d.getText().includes('"key"')
      );
      return doc?.getText();
    }, 15000);
    assert.ok(defaults, "could not read the default keybindings");

    // The default table spells a punctuation key by its scan code (`ctrl+alt+[BracketLeft]`)
    // while a contribution may spell it by character (`ctrl+alt+[`). Comparing the raw
    // strings would call every punctuation binding free however taken it was.
    const SCAN_CODES: Record<string, string> = {
      "[BracketLeft]": "[",
      "[BracketRight]": "]",
      "[Period]": ".",
      "[Comma]": ",",
      "[Semicolon]": ";",
      "[Quote]": "'",
      "[Slash]": "/",
      "[Backslash]": "\\",
      "[Backquote]": "`",
      "[Minus]": "-",
      "[Equal]": "=",
    };
    const normalize = (key: string) =>
      Object.entries(SCAN_CODES).reduce((k, [code, ch]) => k.split(code).join(ch), key);

    const entries = [
      ...defaults!.matchAll(
        /\{\s*"key":\s*"([^"]+)",\s*"command":\s*"([^"]+)"(?:,\s*"when":\s*"([^"]*)")?/g
      ),
    ].map((m) => ({ key: normalize(m[1]), command: m[2] }));
    // A regex that stopped matching the file's shape would make this test pass while
    // asserting nothing at all.
    assert.ok(
      entries.length > 200,
      `parsed only ${entries.length} default keybindings; the scan no longer matches the file`
    );

    // Overrides that predate item 165 and are deliberate: Open Preview claims the key the
    // Markdown preview trained everyone to reach for, at the cost of delete-line inside a
    // `.tmd` buffer. Listed rather than exempted wholesale, so a NEW clash still fails.
    const deliberate = new Set(["ctrl+shift+k"]);
    const contributed: { command: string; key: string }[] = JSON.parse(
      fs.readFileSync(path.join(REPO_ROOT, "editor/vscode/package.json"), "utf8")
    ).contributes.keybindings;

    // Our own contributions are IN this table — it is the merged default table, not just
    // VS Code's built-ins — so without this filter every binding clashes with itself and the
    // test reports a conflict for a key nothing else uses.
    const theirs = entries.filter((e) => !e.command.startsWith("taliesin."));
    const clashes = contributed
      .filter((c) => !deliberate.has(c.key))
      .flatMap((c) =>
        theirs
          .filter((e) => e.key === normalize(c.key))
          .map((e) => `${c.command} binds ${c.key}, which VS Code already uses for ${e.command}`)
      );
    // What the excluded keys actually cost, so "deliberate" stays a decision and not a
    // forgotten exemption.
    for (const key of deliberate) {
      const shadowed = theirs.filter((e) => e.key === key).map((e) => e.command);
      console.log(`deliberate override ${key} shadows: ${shadowed.join(", ") || "nothing"}`);
    }
    assert.deepStrictEqual(clashes, [], "keybinding clash");
  });

  // The code lens had no coverage at all until these (item 175(d), first half, shipped in
  // 1b8b3756, when the lens still carried ▶ Run Cell buttons). A unit test over the provider
  // could only prove it builds a lens array; it could never show that VS Code accepted the
  // provider, asked the language server, and rendered anything. This repo has been bitten by
  // exactly that gap twice, so these drive `vscode.executeCodeLensProvider` in a real
  // Extension Host.
  //
  // Wave 13 cut `taliesin run`, so what a lens carries now is a LABEL with no command. The
  // anchor arithmetic is the same and is still what these pin; what is gone is the argument
  // round trip, which had nothing left to be an argument to.
  //
  // The fixture is a dedicated file rather than a corpus document because these assert exact
  // line numbers: a corpus doc is edited for unrelated reasons and would drift the arithmetic
  // silently. The corpus keeps its place in the net via the last test here, which asserts
  // against a real document without hardcoding a line.
  const CODELENS_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/codelens.tmd");

  /** The lenses VS Code renders for `uri`, once the server has answered. */
  async function lensesFor(uri: vscode.Uri): Promise<vscode.CodeLens[]> {
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc);
    const lenses = await waitForValue(async () => {
      const got = (await vscode.commands.executeCommand(
        "vscode.executeCodeLensProvider",
        uri
      )) as vscode.CodeLens[] | undefined;
      // The provider answers `[]` while the language client is still starting, which is not
      // the same as "this document has no cells" — poll rather than assert on the empty.
      return got && got.length > 0 ? got : undefined;
    }, 15000);
    return lenses ?? [];
  }

  test("labels every executable fence, anchored on the fence line", async () => {
    const lenses = await lensesFor(vscode.Uri.file(CODELENS_FIXTURE));

    // The fixture has three runnable cells, all `{python}`, all `#| cache: false`.
    assert.strictEqual(lenses.length, 3, "expected one label per executable cell");

    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(CODELENS_FIXTURE));
    for (const lens of lenses) {
      // The label must sit ON the fence, not over the cell's first statement: that is the
      // whole point of the `startLine - 1` in the provider, and the failure it prevents is a
      // lens that covers the author's code.
      const anchored = doc.lineAt(lens.range.start.line).text;
      assert.match(
        anchored,
        /^```\{python\}/,
        `a label is anchored on "${anchored}", which is not an executable fence`
      );
      // A label, not a button: an empty command name is what tells VS Code there is nothing
      // to click. A name here would be a name nothing implements.
      assert.strictEqual(lens.command?.command, "", "a lens must not offer a command");
    }
    assert.deepStrictEqual(
      lenses.map((l) => l.range.start.line + 1).sort((a, b) => a - b),
      [13, 28, 38],
      "the three labels should sit on the fixture's three executable fence lines"
    );
  });

  test("labels no fence a kernel does not run", async () => {
    // Three negatives in one fixture, and each is a different way to not be runnable:
    // a plain ```bash fence, a `{bash}` cell block (a cell, but no kernel runs it), and a
    // plain ```python fence. A label over any of them is what drift looks like to an author.
    const lenses = await lensesFor(vscode.Uri.file(CODELENS_FIXTURE));
    // Without this, the whole test passes vacuously: every `!includes` below is trivially
    // true of an empty list, so a provider that broke completely would look like a provider
    // that correctly withheld three labels.
    assert.ok(lenses.length > 0, "no lenses at all, so the negatives below prove nothing");
    const lines = lenses.map((l) => l.range.start.line + 1);
    for (const [line, what] of [
      [20, "a plain ```bash fence"],
      [24, "a `{bash}` cell block, which no kernel runs"],
      [33, "a plain ```python fence"],
    ] as [number, string][]) {
      assert.ok(!lines.includes(line), `a label was placed over ${what} (line ${line})`);
    }
  });

  test("labels a real corpus document too", async () => {
    // The fixture pins the arithmetic; this keeps the corpus in the regression net. No line
    // numbers, so editing the post for unrelated reasons cannot break it. The corpus post is
    // ordinarily cacheable, so what it must produce is a `⚡ cached` label after a build has
    // filled `_freeze/` — or nothing at all before one has. Asserting on the SHAPE rather
    // than on presence is the only honest test here: a lens that appears must be a label.
    const lenses = await lensesFor(
      vscode.Uri.file(path.join(REPO_ROOT, "corpus/tech-blog/posts/pca-geometry/index.tmd"))
    );
    for (const lens of lenses) {
      assert.strictEqual(lens.command?.command, "", "a lens on a corpus doc must be a label");
      assert.match(lens.command!.title, /cached|re-runs/, "and must say what the cell will do");
    }
  });
});

async function completionLabels(
  uri: vscode.Uri,
  position: vscode.Position
): Promise<string[]> {
  // The server may not have answered yet on the first call; retry until it does.
  const labels = await waitForValue(async () => {
    const list = (await vscode.commands.executeCommand(
      "vscode.executeCompletionItemProvider",
      uri,
      position
    )) as vscode.CompletionList;
    const items = list?.items ?? [];
    return items.length > 0 ? items.map((i) => labelText(i.label)) : undefined;
  }, 15000);
  return labels ?? [];
}

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

/** Poll an async producer until it yields a defined value, or the timeout expires. */
async function waitForValue<T>(
  produce: () => Promise<T | undefined>,
  timeoutMs: number
): Promise<T | undefined> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = await produce();
    if (value !== undefined) return value;
    if (Date.now() > deadline) return undefined;
    await new Promise((r) => setTimeout(r, 200));
  }
}

function labelText(label: string | vscode.CompletionItemLabel): string {
  return typeof label === "string" ? label : label.label;
}

/**
 * The port of a live `taliesin preview <BOOK_ROOT>` started by THIS build, or `undefined`.
 *
 * Reading the argv is what makes the claim specific. "A preview server is running" is nearly
 * always true on a developer's machine; "a server is serving the book root" is the thing item
 * 150 changed, and the port it yields is how the test can then read the page itself.
 */
async function bookServerPort(): Promise<string | undefined> {
  if (process.platform === "win32") return undefined;
  let listing: string;
  try {
    listing = execFileSync("ps", ["-eo", "args="], { encoding: "utf8" });
  } catch {
    return undefined;
  }
  for (const line of listing.split("\n")) {
    const m = new RegExp(`^${escapeRe(TALIESIN_BIN)} preview ${escapeRe(BOOK_ROOT)} (\\d+)\\s*$`).exec(
      line.trim()
    );
    if (m) return m[1];
  }
  return undefined;
}

function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function get(url: string): Promise<string> {
  return new Promise((resolve, reject) => {
    http
      .get(url, (res) => {
        let body = "";
        res.setEncoding("utf8");
        res.on("data", (c) => (body += c));
        res.on("end", () => resolve(body));
      })
      .on("error", reject);
  });
}


// The project-level surfaces, in a real Extension Host.
//
// Every one of these is registered rather than computed, and a registration is exactly the
// thing a unit test cannot check: an unaccepted contribution fails silently, with the feature
// simply absent. That is how the companion sat inert for weeks once before. The manifest tests
// prove the JSON is right and the Rust tests prove the answers are right; only a running host
// can say VS Code took them.
suite("Taliesin project surfaces", () => {
  /** A two-page project on disk, so the server has cross-page anchors to find. */
  function project(tag: string): string {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), `tali-${tag}-`));
    fs.writeFileSync(path.join(dir, "_site.yml"), "title: Scratch\n");
    fs.writeFileSync(
      path.join(dir, "index.tmd"),
      "---\ntitle: Index\n---\n\n# Index\n\nSee @sec-elsewhere.\n"
    );
    fs.writeFileSync(
      path.join(dir, "other.tmd"),
      "---\ntitle: Other\n---\n\n# Elsewhere {#sec-elsewhere}\n\n## Deeper\n"
    );
    return dir;
  }

  test("go to definition crosses into the page that defines the anchor", async () => {
    const dir = project("xdef");
    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(path.join(dir, "index.tmd"))
    );
    await vscode.window.showTextDocument(doc);
    const line = doc.getText().split("\n").findIndex((l) => l.includes("@sec-elsewhere"));
    assert.ok(line >= 0, "fixture must reference the sibling anchor");

    const hit = await waitForValue(async () => {
      const locs = (await vscode.commands.executeCommand(
        "vscode.executeDefinitionProvider",
        doc.uri,
        new vscode.Position(line, doc.lineAt(line).text.indexOf("@sec-") + 2)
      )) as vscode.Location[] | undefined;
      return locs?.find((l) => l.uri.fsPath.endsWith("other.tmd"));
    }, 15000);

    assert.ok(hit, "F12 on a cross-page reference resolved to nothing");
  });

  // NOT a test that the task provider was accepted, because that cannot be observed here.
  // Measured in this host: with no folder open, `vscode.tasks.fetchTasks()` returns zero tasks
  // of ANY type, so VS Code's task system is inert without a workspace and a provider's
  // acceptance is unobservable. The suite deliberately runs folderless (30+ other tests depend
  // on that), and adding the first folder to an empty workspace restarts the extension host.
  //
  // What IS observable is that the task system is inert rather than that our provider is
  // broken, which is the distinction that matters if this ever starts failing. See
  // DETECTION-DEBT.md.
  test("the task system is inert without a workspace folder, which is VS Code's own rule", async () => {
    assert.strictEqual(
      vscode.workspace.workspaceFolders,
      undefined,
      "this suite runs folderless on purpose; if that changed, assert on fetchTasks instead"
    );
    const all = await vscode.tasks.fetchTasks();
    assert.strictEqual(
      all.length,
      0,
      "a folderless host offers no tasks at all; a non-zero count here means the platform " +
        "changed and the task provider can now be asserted directly"
    );
  });

  test("no always-on language model tool is offered", async () => {
    // Wave 4.3 folded the five `taliesin_*` tools into the MCP provider, which a user points
    // an agent at deliberately. `vscode.lm.tools` is the merged list the platform actually
    // offers a model, so this is the real check that none of them came back.
    const offered = [...vscode.lm.tools.map((t) => t.name)].filter((n) =>
      n.startsWith("taliesin")
    );
    assert.deepStrictEqual(offered, [], "the companion registers no always-on model tools");
  });
});
