import * as assert from "node:assert";
import * as path from "node:path";
import * as vscode from "vscode";
import * as http from "node:http";
import { execFileSync } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import { dropProvider } from "../../insert";

const REPO_ROOT = path.resolve(__dirname, "../../../../../"); // out/e2e/suite -> editor/vscode -> editor -> repo
const SAMPLE_POST = path.join(REPO_ROOT, "corpus/posts/born-machines.tmd");
const SAMPLE_TMD = path.join(REPO_ROOT, "corpus/native-tmd.tmd");
const DIAG_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/diag-typo.tmd");
const COMPLETE_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/complete.tmd");
const MATH_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/math.tmd");
const PATHS_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/paths.tmd");
const INCLUDE_DOC = path.join(REPO_ROOT, "corpus/bayesian-website/index.tmd");
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
      "taliesin.check",
      "taliesin.restartServer",
      "taliesin.showServerLog",
      "taliesin.doctor",
      "taliesin.insertMathSymbol",
      "taliesin.revealInPreview",
      "taliesin.moveSectionUp",
      "taliesin.moveSectionDown",
      "taliesin.promoteHeading",
      "taliesin.demoteHeading",
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
    // The server (not the old TS shim) is the source, and it carries the stable TAL code.
    assert.equal(typo!.source, "taliesin");
    assert.ok(typo!.code, "the diagnostic should carry its TAL-* code");
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
    assert.ok(divLabels.includes("theorem"), `div classes should include theorem: ${divLabels}`);
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

  test("paints a document link on an `{{< include >}}` path", async () => {
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(INCLUDE_DOC));
    await vscode.window.showTextDocument(doc);
    const line = doc
      .getText()
      .split("\n")
      .findIndex((l) => l.trim().startsWith("{{< include"));
    assert.ok(line >= 0, "fixture must contain an include directive");

    // Links arrive from the server, so poll rather than assume the first call is answered.
    const hit = await waitForValue(async () => {
      const links = (await vscode.commands.executeCommand(
        "vscode.executeLinkProvider",
        doc.uri
      )) as vscode.DocumentLink[];
      return links?.find((l) => l.range.start.line === line);
    }, 15000);

    assert.ok(hit, `expected a document link on line ${line}`);
    assert.ok(hit!.target, "the link must carry a target uri");
    assert.ok(
      hit!.target!.fsPath.endsWith("subsections/_introduction.tmd"),
      `link should point at the included file, got ${hit!.target!.fsPath}`
    );
    // The link spans the path token only, not the whole `{{< … >}}` directive.
    const text = doc.lineAt(line).text;
    assert.equal(
      text.slice(hit!.range.start.character, hit!.range.end.character),
      "subsections/_introduction.tmd"
    );
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
      // build with `headless-js` on a host with Chrome rasterizes the real KaTeX render,
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

  test("renames a cross-reference anchor and every reference to it", async () => {
    // Rename existed in the server the whole time and the companion never exposed it; it is
    // the clearest proof that the editor is now driven by the server rather than by a
    // parallel TypeScript copy.
    const doc = await vscode.workspace.openTextDocument({
      language: "taliesin",
      content: "---\ntitle: T\n---\n\n# Intro {#sec-intro}\n\nSee @sec-intro and @sec-intro.\n",
    });
    await vscode.window.showTextDocument(doc);
    const edit = await waitForValue(
      async () =>
        (await vscode.commands.executeCommand(
          "vscode.executeDocumentRenameProvider",
          doc.uri,
          new vscode.Position(4, 10), // inside `sec-intro` in the heading anchor
          "sec-overview"
        )) as vscode.WorkspaceEdit | undefined,
      15000
    );
    assert.ok(edit, "rename should return a workspace edit");
    assert.equal(
      edit!.get(doc.uri).length,
      3,
      "the definition and both references should be rewritten"
    );
  });

  test("Format Document tidies a table and leaves the prose alone", async () => {
    const doc = await vscode.workspace.openTextDocument({
      language: "taliesin",
      content: "---\ntitle: T\n---\n\nA | pipe in prose.\n\n|a|long|\n|-|-:|\n|1|2|\n",
    });
    await vscode.window.showTextDocument(doc);
    const edits = await waitForValue(
      async () =>
        (await vscode.commands.executeCommand(
          "vscode.executeFormatDocumentProvider",
          doc.uri
        )) as vscode.TextEdit[] | undefined,
      15000
    );
    assert.ok(edits && edits.length > 0, "formatting should return edits");
    // Assert on the APPLIED result, not on the edit list: VS Code minimizes a formatter's
    // edits before handing them back, so the one edit the server sent over the wire (pinned
    // in `crates/server/tests/lsp_stdio.rs`) can arrive here split into several.
    const wsEdit = new vscode.WorkspaceEdit();
    wsEdit.set(doc.uri, edits!);
    assert.ok(await vscode.workspace.applyEdit(wsEdit), "edits should apply");
    assert.equal(
      doc.getText(),
      "---\ntitle: T\n---\n\nA | pipe in prose.\n\n" +
        "| a   | long |\n| --- | ---: |\n| 1   |    2 |\n"
    );
    // The paragraph on line 4 also contains a pipe. A formatter that reached it would be
    // rewriting prose into a table, which is the one failure this feature must not have.
    for (const e of edits!) {
      assert.ok(
        e.range.start.line > 5,
        `no edit may reach the prose above the table: ${e.range.start.line}`
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

  // --- Structural commands (backlog item 165) -----------------------------------------
  //
  // `lsp_edits.rs` owns the transforms and proves them on text. What only a real Extension
  // Host can prove is the half that lives here: that VS Code applied the server's edits to
  // the buffer and then put the caret where the section went. A wrong caret is not a
  // cosmetic bug for these commands — the next keypress acts on whatever section the caret
  // is in, so an off-by-one turns "move this down twice" into moving two different sections.

  test("moves a section, with its subtree, and takes the caret with it", async () => {
    const doc = await vscode.workspace.openTextDocument({
      language: "taliesin",
      content: "## Alpha\n\nalpha body\n\n### Child\n\nchild body\n\n## Beta\n\nbeta body\n",
    });
    const editor = await vscode.window.showTextDocument(doc);
    // Caret on "alpha body", inside the section but not on its heading.
    editor.selection = new vscode.Selection(2, 3, 2, 3);

    await vscode.commands.executeCommand("taliesin.moveSectionDown");

    assert.strictEqual(
      doc.getText(),
      "## Beta\n\nbeta body\n\n## Alpha\n\nalpha body\n\n### Child\n\nchild body\n",
      "Alpha and its `### Child` should have moved below Beta as one block"
    );
    // Alpha's heading is now line 4, so the caret's line-2 offset into the section lands on
    // line 6 — still on "alpha body", the line the author was editing.
    assert.strictEqual(editor.selection.active.line, 6);
    assert.strictEqual(
      doc.lineAt(editor.selection.active.line).text,
      "alpha body",
      "the caret should still be on the line it started on"
    );
    assert.strictEqual(editor.selection.active.character, 3, "the column should survive");

    // Deliberately NOT asserted here: that Ctrl+Z restores the order in one step. The
    // property is real and is why the command applies ONE `WorkspaceEdit` rather than an
    // edit per heading (see `applySectionEdit`), but `executeCommand("undo")` acts on
    // whatever the workbench considers focused and resolves when the command is invoked
    // rather than when the buffer settles — polling for the restored text still failed at
    // load ~3. Asserting it here would have shipped a flake that reads as a product bug.
  });

  test("promotes a heading with its descendants and leaves every other line alone", async () => {
    // No section after Alpha's subtree, so promote/demote is a true round trip here. With a
    // `## Beta` following, promoting Alpha to `#` would adopt Beta as a child and demoting
    // would take it down too — see `promoting_adopts_a_following_sibling…` in lsp_edits.rs.
    const content = "## Alpha\n\n### Child\n\nbody\n";
    const doc = await vscode.workspace.openTextDocument({ language: "taliesin", content });
    const editor = await vscode.window.showTextDocument(doc);
    editor.selection = new vscode.Selection(0, 4, 0, 4);

    await vscode.commands.executeCommand("taliesin.promoteHeading");
    assert.strictEqual(doc.getText(), "# Alpha\n\n## Child\n\nbody\n");
    // The caret stays on the heading it acted on, so the keys can be pressed repeatedly.
    assert.strictEqual(editor.selection.active.line, 0);

    await vscode.commands.executeCommand("taliesin.demoteHeading");
    assert.strictEqual(doc.getText(), content, "demote should undo the promote");
  });

  test("a refusal is reported, not applied", async () => {
    // "## Beta" is the last section at its level: moving it down would have to invent a
    // sibling. The buffer must come back untouched rather than half-transformed.
    const content = "## Alpha\n\na\n\n## Beta\n\nb\n";
    const doc = await vscode.workspace.openTextDocument({ language: "taliesin", content });
    const editor = await vscode.window.showTextDocument(doc);
    editor.selection = new vscode.Selection(4, 0, 4, 0);

    await vscode.commands.executeCommand("taliesin.moveSectionDown");
    assert.strictEqual(doc.getText(), content, "a refused move must not edit the buffer");
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


// The paste and drop gestures, in a real Extension Host.
//
// A unit test proves the routing is right and `lsp_insert.rs` proves the emitted text is right.
// Neither can prove VS Code ACCEPTED the provider, which is exactly how a provider registered
// against a stale `engines.vscode` fails: silently, with the feature simply absent.
//
// What is drivable here was measured, not assumed. Listing every command matching /drop|paste/
// inside a real host shows `editor.action.clipboardPasteAction` and `editor.action.pasteAs` for
// paste and NOTHING for drop, and `vscode.env.clipboard` is text-only. So: the text/plain routes
// go end to end, and the drop route calls the provider directly. See DETECTION-DEBT.md for the
// two gestures that cannot be reached at all.
suite("Taliesin authoring gestures", () => {
  /** A scratch project with a `_site.yml`, so the server sees a declared boundary. */
  function scratch(tag: string): string {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), `tali-${tag}-`));
    fs.writeFileSync(path.join(dir, "_site.yml"), "title: Scratch\n");
    return dir;
  }

  /** Wait until `check` passes, polling rather than sleeping once. */
  async function until(check: () => boolean, what: string): Promise<void> {
    for (let i = 0; i < 80; i++) {
      if (check()) return;
      await new Promise((r) => setTimeout(r, 50));
    }
    assert.fail(`timed out waiting for ${what}`);
  }

  test("pasting a URL over a selection makes a link", async () => {
    const dir = scratch("url");
    const doc = path.join(dir, "notes.tmd");
    fs.writeFileSync(doc, "See the manual here.\n");

    const opened = await vscode.workspace.openTextDocument(vscode.Uri.file(doc));
    const editor = await vscode.window.showTextDocument(opened);
    // Select "manual".
    editor.selection = new vscode.Selection(new vscode.Position(0, 8), new vscode.Position(0, 14));

    await vscode.env.clipboard.writeText("https://taliesin.dev/guide");
    await vscode.commands.executeCommand("editor.action.clipboardPasteAction");

    await until(() => opened.getText().includes("]("), "the link paste to apply");
    assert.match(opened.getText(), /\[manual\]\(https:\/\/taliesin\.dev\/guide\)/);
  });

  test("pasting a BibTeX entry cites it and appends it to the .bib", async () => {
    const dir = scratch("bib");
    const doc = path.join(dir, "notes.tmd");
    const bib = path.join(dir, "refs.bib");
    fs.writeFileSync(doc, "---\nbibliography: refs.bib\n---\n\nAs shown in \n");
    fs.writeFileSync(bib, "@book{knuth1984, title = {TeX}}\n");

    const opened = await vscode.workspace.openTextDocument(vscode.Uri.file(doc));
    const editor = await vscode.window.showTextDocument(opened);
    const end = new vscode.Position(4, 13);
    editor.selection = new vscode.Selection(end, end);

    await vscode.env.clipboard.writeText("@article{bishop2006,\n  title = {Pattern Recognition},\n}");
    await vscode.commands.executeCommand("editor.action.clipboardPasteAction");

    await until(() => opened.getText().includes("[@bishop2006]"), "the citation to be inserted");
    // The append is a WorkspaceEdit on another document, so read it through the editor's model
    // rather than off disk: it is applied but not necessarily saved.
    const bibDoc = await vscode.workspace.openTextDocument(vscode.Uri.file(bib));
    await until(() => bibDoc.getText().includes("bishop2006"), "the .bib append to apply");
    assert.match(bibDoc.getText(), /@book\{knuth1984/, "the existing entry survives");
  });

  test("pasting a tab-separated grid is offered as a table, but is not the default", async () => {
    const dir = scratch("tsv");
    const doc = path.join(dir, "notes.tmd");
    fs.writeFileSync(doc, "\n");

    const opened = await vscode.workspace.openTextDocument(vscode.Uri.file(doc));
    await vscode.window.showTextDocument(opened);
    await vscode.env.clipboard.writeText("site\tdepth\nnorth\t3\n");

    // The plain paste first: the TSV edit yields to text, so tab-separated prose must NOT
    // silently become a table.
    await vscode.commands.executeCommand("editor.action.clipboardPasteAction");
    await until(() => opened.getText().includes("site"), "the plain paste to apply");
    assert.ok(
      !opened.getText().includes("|"),
      `the default paste stayed plain text: ${JSON.stringify(opened.getText())}`
    );

    // Now ask for the table explicitly, the way the paste-as menu does.
    await vscode.commands.executeCommand("undo");
    await vscode.commands.executeCommand("editor.action.pasteAs", { kind: "text.taliesin" });
    await until(() => opened.getText().includes("|"), "the table paste to apply");
    assert.match(opened.getText(), /\| site\s+\| depth \|/, "aligned by the server");
  });

  test("dropping a CSV inserts a dataset card and a loader cell", async () => {
    const dir = scratch("drop");
    const doc = path.join(dir, "notes.tmd");
    fs.writeFileSync(doc, "# Notes\n");
    const csv = path.join(dir, "m.csv");
    fs.writeFileSync(csv, "a,b\n1,2\n");

    const opened = await vscode.workspace.openTextDocument(vscode.Uri.file(doc));
    await vscode.window.showTextDocument(opened);

    const dt = new vscode.DataTransfer();
    dt.set("text/uri-list", new vscode.DataTransferItem(vscode.Uri.file(csv).toString()));

    // Called directly: no built-in command drives a drop provider (measured, see the suite note).
    const edit = await dropProvider.provideDocumentDropEdits!(
      opened,
      new vscode.Position(1, 0),
      dt,
      new vscode.CancellationTokenSource().token
    );
    assert.ok(edit, "the provider answered the drop");
    const text = String((edit as vscode.DocumentDropEdit).insertText);
    assert.match(text, /\{\{< dataset m\.csv >\}\}/, `card: ${text}`);
    assert.match(text, /```\{python\}/, `loader: ${text}`);
    assert.match(text, /pd\.read_csv\("m\.csv"\)/, `loader body: ${text}`);
  });
});

// The rename repair, in a real Extension Host.
//
// This is the test the unit layer cannot write: `lsp_rename_file.rs` proves the edits are right,
// but only a real host proves the `onWillRenameFiles` hook fires, that `waitUntil` is honoured,
// and that the returned WorkspaceEdit is applied to files that are not open.
suite("Taliesin rename repair", () => {
  test("renaming a chapter repairs the references pointing at it", async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tali-rn-"));
    fs.writeFileSync(path.join(dir, "_site.yml"), "title: P\nchapters:\n  - intro.tmd\n");
    fs.writeFileSync(path.join(dir, "intro.tmd"), "# Intro\n");
    // Deliberately NOT opened in an editor: the repair has to reach a file on disk.
    fs.writeFileSync(path.join(dir, "two.tmd"), "See [intro](intro.html) and intro.tmd.\n");

    const oldUri = vscode.Uri.file(path.join(dir, "intro.tmd"));
    const newUri = vscode.Uri.file(path.join(dir, "overview.tmd"));

    // The real editor rename, so this covers the hook, the request and the edit application
    // rather than any one of them alone.
    const we = new vscode.WorkspaceEdit();
    we.renameFile(oldUri, newUri);
    assert.ok(await vscode.workspace.applyEdit(we), "the rename applied");

    const two = path.join(dir, "two.tmd");
    for (let i = 0; i < 100; i++) {
      const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(two));
      if (doc.getText().includes("overview.html")) break;
      await new Promise((r) => setTimeout(r, 50));
    }
    const text = (await vscode.workspace.openTextDocument(vscode.Uri.file(two))).getText();
    assert.match(text, /\(overview\.html\)/, `the .html spelling: ${text}`);
    assert.match(text, /overview\.tmd/, `the .tmd spelling: ${text}`);
    assert.ok(!text.includes("intro.html"), `no stale reference: ${text}`);

    // The book spine too, or the chapter drops out of the book entirely.
    const yml = (
      await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(dir, "_site.yml")))
    ).getText();
    assert.match(yml, /- overview\.tmd/, `the spine: ${yml}`);
  });
});
