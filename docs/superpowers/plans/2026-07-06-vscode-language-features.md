# VS Code companion language features (diagnostics + completions) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Taliesin VS Code companion two net-new language features, error squiggles from `taliesin check` and autocomplete from a drift-proof `taliesin vocab` dump, as in-process providers that shell out to the CLI.

**Architecture:** A new "language features" layer in `editor/vscode/src/`, registered in `activate()` beside the existing preview command. Pure logic (JSON parsing, range mapping, completion-context detection, buffer/`.bib` scanning) lives in no-`vscode`-import modules covered by `node:test`, mirroring `paths.ts`/`ports.ts`; the thin `vscode` wiring (a `DiagnosticCollection`, a `CompletionItemProvider`) is proven by one `@vscode/test-electron` e2e per feature. Phase 1 (diagnostics) needs **zero** Rust changes. Phase 2 adds a small `taliesin vocab` command whose JSON is generated from the validator's own consts and golden-file-locked, exactly like `schema.rs`.

**Tech Stack:** Rust (edition 2024, `serde_json`), TypeScript (`@types/vscode`, `node:test`, `@vscode/test-electron`, esbuild bundler).

## Global Constraints

- **No em dashes or en dashes** in any prose, comment, or doc string. Use commas, colons, parentheses, or restructured sentences.
- **Do-NOT-touch exec/kernel zone:** `check` and `vocab` execute no code and boot no kernel. Do not touch `crates/server/src/{exec,kernel,freeze}.rs`.
- **Single-editing-surface invariant:** diagnostics are read-only; completions insert only on explicit user acceptance (ordinary editor behavior). Nothing here writes back to source from the preview.
- **Not the rebrand:** keep the existing manifest ids verbatim: package name `qmd-fast-companion`, config key `qmdFast.path` (default `qmd-fast`), command id `qmdFast.openPreview`, extension id `qmd-fast.qmd-fast-companion`. New code follows those ids so it folds cleanly into the later rebrand.
- **Language id:** register all features for the existing `taliesin` language id (`.tmd`).
- **Drift-proof vocabulary:** completion vocabulary is generated from the Rust validator consts and golden-file-locked. Never hand-list vocabulary in TypeScript.
- **`rustfmt` clean:** a `PostToolUse` hook runs `rustfmt` on edited `.rs` files; CI enforces `cargo fmt --check`.
- **Naming convention for the new TS modules:** a singular-named module (`check.ts`, `complete.ts`) is the pure, `vscode`-free, `node:test`-covered logic; a plural-named sibling (`diagnostics.ts`, `completions.ts`) is the impure `vscode`+spawn wiring proven only by e2e.

---

## Phase 1: Diagnostics (error squiggles). Zero Rust changes.

### Task 1: Pure `check` output parsing + range mapping (`src/check.ts`)

**Files:**
- Create: `editor/vscode/src/check.ts`
- Test: `editor/vscode/src/test/check.test.ts`

**Interfaces:**
- Produces:
  - `interface CheckDiag { file: string; line: number | null; message: string }`
  - `type CheckOutput = { kind: "diags"; diags: CheckDiag[] } | { kind: "error"; error: string }`
  - `function parseCheckJson(stdout: string): CheckOutput`
  - `interface DiagShape { line0: number; message: string }`
  - `function toDiagnostics(out: CheckOutput, lineCount: number): DiagShape[]`

- [ ] **Step 1: Write the failing test**

Create `editor/vscode/src/test/check.test.ts`:

```ts
import { test } from "node:test";
import assert from "node:assert";
import { parseCheckJson, toDiagnostics } from "../check";

test("parseCheckJson reads the diagnostics array", () => {
  const out = parseCheckJson('[{"file":"a.tmd","line":3,"message":"unknown key `titel`"}]');
  assert.deepEqual(out, {
    kind: "diags",
    diags: [{ file: "a.tmd", line: 3, message: "unknown key `titel`" }],
  });
});

test("parseCheckJson tolerates a null line", () => {
  const out = parseCheckJson('[{"file":"_site.yml","line":null,"message":"needs a name"}]');
  assert.equal(out.kind, "diags");
  assert.equal((out as any).diags[0].line, null);
});

test("parseCheckJson surfaces the {error} envelope", () => {
  const out = parseCheckJson('{"error":"cannot read missing.tmd"}');
  assert.deepEqual(out, { kind: "error", error: "cannot read missing.tmd" });
});

test("parseCheckJson treats malformed output as an error, not a throw", () => {
  const out = parseCheckJson("not json at all");
  assert.equal(out.kind, "error");
});

test("parseCheckJson treats an empty string as no diagnostics", () => {
  assert.deepEqual(parseCheckJson(""), { kind: "diags", diags: [] });
});

test("toDiagnostics maps a 1-based line to a 0-based line", () => {
  const shapes = toDiagnostics(
    { kind: "diags", diags: [{ file: "a.tmd", line: 3, message: "m" }] },
    10
  );
  assert.deepEqual(shapes, [{ line0: 2, message: "m" }]);
});

test("toDiagnostics clamps a null line and an over-long line to the document", () => {
  const shapes = toDiagnostics(
    {
      kind: "diags",
      diags: [
        { file: "a.tmd", line: null, message: "doc-level" },
        { file: "a.tmd", line: 999, message: "past EOF" },
      ],
    },
    5
  );
  assert.deepEqual(shapes, [
    { line0: 0, message: "doc-level" },
    { line0: 4, message: "past EOF" }, // clamped to last line (lineCount - 1)
  ]);
});

test("toDiagnostics renders the {error} envelope as one document-level diagnostic", () => {
  const shapes = toDiagnostics({ kind: "error", error: "cannot read x" }, 3);
  assert.deepEqual(shapes, [{ line0: 0, message: "cannot read x" }]);
});

test("toDiagnostics on empty diags is empty", () => {
  assert.deepEqual(toDiagnostics({ kind: "diags", diags: [] }, 3), []);
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd editor/vscode && npm run compile-tests && node --test out/test/check.test.js`
Expected: FAIL (cannot find module `../check`).

- [ ] **Step 3: Write the minimal implementation**

Create `editor/vscode/src/check.ts`:

```ts
// Pure parsing + range mapping for `taliesin check --format json` output.
// No `vscode` import, so it stays in the fast `node:test` loop (mirrors paths.ts/ports.ts).
// The CLI emits either an array of {file, line, message} or a {"error": "..."} envelope
// (crates/server/src/check.rs). Non-zero exit is expected when findings exist, so callers
// parse stdout regardless of exit code.

export interface CheckDiag {
  file: string;
  line: number | null;
  message: string;
}

export type CheckOutput =
  | { kind: "diags"; diags: CheckDiag[] }
  | { kind: "error"; error: string };

// Parse the CLI's stdout. Never throws: malformed output becomes a {error} so the
// caller can surface it instead of dropping squiggles silently.
export function parseCheckJson(stdout: string): CheckOutput {
  const text = stdout.trim();
  if (text === "") return { kind: "diags", diags: [] };
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return { kind: "error", error: `check produced unparseable output: ${text.slice(0, 200)}` };
  }
  if (Array.isArray(value)) {
    const diags = value
      .filter((d): d is CheckDiag => !!d && typeof (d as any).message === "string")
      .map((d) => ({
        file: typeof d.file === "string" ? d.file : "",
        line: typeof d.line === "number" ? d.line : null,
        message: d.message,
      }));
    return { kind: "diags", diags };
  }
  if (value && typeof value === "object" && typeof (value as any).error === "string") {
    return { kind: "error", error: (value as any).error };
  }
  return { kind: "error", error: `check produced unexpected output: ${text.slice(0, 200)}` };
}

// A `vscode`-free description of where a diagnostic lands. The wiring turns each into a
// whole-line `vscode.Diagnostic` via `document.lineAt(line0).range`, so the horizontal
// (EOL) extent is VS Code's job and this stays testable.
export interface DiagShape {
  line0: number; // 0-based, clamped to [0, lineCount - 1]
  message: string;
}

export function toDiagnostics(out: CheckOutput, lineCount: number): DiagShape[] {
  const lastLine = Math.max(0, lineCount - 1);
  const clamp = (line0: number) => Math.max(0, Math.min(line0, lastLine));
  if (out.kind === "error") {
    return [{ line0: 0, message: out.error }];
  }
  return out.diags.map((d) => ({
    // comrak lines are 1-based; a null line is a document-level finding -> line 1 -> line0 0.
    line0: clamp((d.line ?? 1) - 1),
    message: d.message,
  }));
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd editor/vscode && npm run compile-tests && node --test out/test/check.test.js`
Expected: PASS (8 tests).

- [ ] **Step 5: Type-check the whole extension**

Run: `cd editor/vscode && npx tsc -p . --noEmit`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add editor/vscode/src/check.ts editor/vscode/src/test/check.test.ts
git commit -m "feat(companion): pure check-json parsing + diagnostic range mapping"
```

---

### Task 2: Diagnostics wiring + registration + e2e (`src/diagnostics.ts`)

**Files:**
- Create: `editor/vscode/src/diagnostics.ts`
- Modify: `editor/vscode/src/extension.ts` (call `registerDiagnostics` from `activate`)
- Modify: `editor/vscode/package.json` (add `onLanguage:taliesin` activation event)
- Create: `editor/vscode/test-fixtures/diag-typo.tmd`
- Modify: `editor/vscode/src/e2e/suite/integration.test.ts` (add a diagnostics test)
- Modify: `editor/vscode/README.md` (F5-checklist addendum)

**Interfaces:**
- Consumes: `parseCheckJson`, `toDiagnostics` (Task 1); `isSourceFile` (`./paths`).
- Produces: `function registerDiagnostics(context: vscode.ExtensionContext): void`

- [ ] **Step 1: Add the activation event so `.tmd` opens run `activate()`**

In `editor/vscode/package.json`, change:

```json
  "activationEvents": [],
```

to:

```json
  "activationEvents": [
    "onLanguage:taliesin"
  ],
```

(Diagnostics-on-open and the completion provider both need `activate()` to run when a `.tmd` file opens; the contributed command already triggers activation for preview, but a language open does not without this.)

- [ ] **Step 2: Create the e2e fixture**

Create `editor/vscode/test-fixtures/diag-typo.tmd` with exactly this content (the `titel` typo on line 3 must trip the front-matter validator's did-you-mean):

```
---
title: Diagnostics Fixture
titel: oops
---

Just clean prose after a typo'd key.
```

- [ ] **Step 3: Write the failing e2e test**

In `editor/vscode/src/e2e/suite/integration.test.ts`, add this constant near the existing `SAMPLE_*` consts (after line 8):

```ts
const DIAG_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/diag-typo.tmd");
```

and add this test inside the `suite(...)` block (after the existing "Open Preview" test):

```ts
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
```

(`waitFor` and `QMD_FAST_BIN` already exist in this file.)

- [ ] **Step 4: Build the binary and run the e2e test to verify it fails first**

Run: `cargo build -p taliesin-server`
Then: `cd editor/vscode && npm run test:e2e`
Expected: the new test FAILS (`getDiagnostics` stays empty, the extension has no diagnostics code yet). The existing preview/language tests still pass.

Note: `npm run test:e2e` downloads a throwaway VS Code into `.vscode-test/` (gitignored) on first run; subsequent runs reuse it.

- [ ] **Step 5: Write the diagnostics wiring**

Create `editor/vscode/src/diagnostics.ts`:

```ts
import * as vscode from "vscode";
import { spawn } from "node:child_process";
import * as path from "node:path";
import { parseCheckJson, toDiagnostics } from "./check";
import { isSourceFile } from "./paths";

// Run `taliesin check --format json <file>` and collect stdout. Never rejects: a spawn
// failure resolves to { spawnError }, and a non-zero exit (expected when findings exist)
// is ignored, we parse stdout regardless of exit code.
function spawnCheck(binary: string, file: string): Promise<{ stdout: string; spawnError?: string }> {
  return new Promise((resolve) => {
    let stdout = "";
    const child = spawn(binary, ["check", file, "--format", "json"], {
      cwd: path.dirname(file),
    });
    child.on("error", (e) => resolve({ stdout: "", spawnError: e.message }));
    child.stdout?.on("data", (b) => (stdout += b.toString()));
    child.on("close", () => resolve({ stdout }));
  });
}

// Own a single DiagnosticCollection, refresh it on open/save/config-change for the active
// Taliesin document, and supersede in-flight checks when a newer run for the same URI starts.
export function registerDiagnostics(context: vscode.ExtensionContext): void {
  const collection = vscode.languages.createDiagnosticCollection("taliesin");
  context.subscriptions.push(collection);

  const runToken = new Map<string, number>(); // per-URI monotonic run id (stale-result guard)
  let warnedMissingBinary = false;

  const binaryPath = () =>
    vscode.workspace.getConfiguration("qmdFast").get<string>("path", "qmd-fast");

  async function refresh(doc: vscode.TextDocument): Promise<void> {
    if (doc.languageId !== "taliesin" || !isSourceFile(doc.fileName)) return;
    const key = doc.uri.toString();
    const token = (runToken.get(key) ?? 0) + 1;
    runToken.set(key, token);

    const result = await spawnCheck(binaryPath(), doc.fileName);
    if (runToken.get(key) !== token) return; // a newer save superseded this run

    if (result.spawnError) {
      collection.delete(doc.uri);
      if (!warnedMissingBinary) {
        warnedMissingBinary = true; // one toast, never per-keystroke
        vscode.window.showWarningMessage(
          `qmd-fast: could not run \`${binaryPath()}\` for diagnostics (${result.spawnError}). ` +
            `Set "qmdFast.path" to the taliesin/qmd-fast binary.`
        );
      }
      return;
    }

    const shapes = toDiagnostics(parseCheckJson(result.stdout), doc.lineCount);
    const diags = shapes.map((s) => {
      const line0 = Math.max(0, Math.min(s.line0, doc.lineCount - 1));
      const range = doc.lineAt(line0).range; // whole-line squiggle (JSON carries no column)
      const d = new vscode.Diagnostic(range, s.message, vscode.DiagnosticSeverity.Warning);
      d.source = "taliesin check";
      return d;
    });
    collection.set(doc.uri, diags);
  }

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => refresh(doc)),
    vscode.workspace.onDidSaveTextDocument((doc) => refresh(doc)),
    vscode.workspace.onDidCloseTextDocument((doc) => collection.delete(doc.uri)),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (!e.affectsConfiguration("qmdFast.path")) return;
      warnedMissingBinary = false;
      for (const doc of vscode.workspace.textDocuments) refresh(doc);
    })
  );

  // Seed diagnostics for whatever is already open at activation.
  for (const doc of vscode.workspace.textDocuments) refresh(doc);
}
```

- [ ] **Step 6: Register it in `activate()`**

In `editor/vscode/src/extension.ts`, add the import after the existing imports:

```ts
import { registerDiagnostics } from "./diagnostics";
```

and call it inside `activate()`, after the `registerCommand` push:

```ts
export function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(
    vscode.commands.registerCommand("qmdFast.openPreview", () => openPreview(context))
  );
  registerDiagnostics(context);
}
```

- [ ] **Step 7: Run the e2e test to verify it now passes**

Run: `cd editor/vscode && npm run test:e2e`
Expected: all e2e tests PASS, including the new diagnostics test (a Warning on line 3 mentioning `titel`).

- [ ] **Step 8: Run the pure unit tests + type-check to confirm nothing regressed**

Run: `cd editor/vscode && npm test && npx tsc -p . --noEmit`
Expected: all `node:test` tests PASS; no type errors.

- [ ] **Step 9: Update the README F5 checklist**

In `editor/vscode/README.md`, append this bullet under the existing F5 acceptance checklist (add it verbatim; adjust only the leading marker to match the list's `-` or `*` style):

```markdown
- Open a `.tmd` with a front-matter typo (or any `taliesin check` finding): a yellow squiggle appears on the offending line, refreshing on save.
```

- [ ] **Step 10: Commit**

```bash
git add editor/vscode/src/diagnostics.ts editor/vscode/src/extension.ts \
  editor/vscode/package.json editor/vscode/test-fixtures/diag-typo.tmd \
  editor/vscode/src/e2e/suite/integration.test.ts editor/vscode/README.md
git commit -m "feat(companion): surface taliesin check findings as editor diagnostics"
```

---

## Phase 2: `taliesin vocab` + completions.

### Task 3: `taliesin_core::vocab` module + golden-file lock

**Files:**
- Create: `crates/core/src/vocab.rs`
- Create: `crates/core/assets/vocab/tali-vocab.json` (empty placeholder first, then blessed)
- Modify: `crates/core/src/lib.rs` (add `pub mod vocab;`)
- Modify: `crates/core/src/render/mod.rs` (re-export the validate consts `pub(crate)`)
- Modify: `crates/core/src/cite/render.rs` (add `XREF_LABELS` const; back `xref_label` with it)
- Modify: `crates/core/src/cite/mod.rs` (re-export `XREF_LABELS` `pub(crate)`)
- Modify: `crates/core/src/site/xref.rs` (agreement unit test, bonus)
- Modify: `Cargo.toml` (add `serde_json` to `[workspace.dependencies]`)
- Modify: `crates/core/Cargo.toml` (make `serde_json` a regular dependency)

**Interfaces:**
- Consumes (existing consts): `crate::frontmatter::{KNOWN_KEYS, EXECUTE_KEYS, LISTING_KEYS, ABOUT_KEYS, HERO_KEYS, PROSE_LINT_KEYS, THEOREM_KEYS}`; `crate::render::validate::{CELL_OPTION_KEYS, CALLOUT_KINDS, THEOREM_KINDS, INPUT_TYPES}`.
- Produces:
  - `crate::cite::XREF_LABELS: &[(&str, &str)]`
  - `crate::render::{CELL_OPTION_KEYS, CALLOUT_KINDS, THEOREM_KINDS, INPUT_TYPES}` (re-exports)
  - `taliesin_core::vocab::VOCAB_JSON: &str`
  - `taliesin_core::vocab::vocab() -> serde_json::Value`
  - `taliesin_core::vocab::to_pretty_json() -> String`

- [ ] **Step 1: Make `serde_json` a regular dependency of core**

In root `Cargo.toml`, under `[workspace.dependencies]`, add (after the `serde_yaml` line):

```toml
serde_json = "1"
```

In `crates/core/Cargo.toml`, add to `[dependencies]` (after the `serde` line):

```toml
serde_json = { workspace = true }
```

and remove the now-redundant line from `[dev-dependencies]`:

```toml
serde_json = "1"
```

(Leave the server crate's own `serde_json` pin untouched; that is out of scope here.)

- [ ] **Step 2: Re-export the validate consts from `render`**

In `crates/core/src/render/mod.rs`, directly below the existing `mod validate;` line (line 69), add:

```rust
// Re-exported for the editor vocabulary dump (crate::vocab), which sources completion
// vocabulary from the SAME consts the validator enforces so the two cannot drift.
pub(crate) use validate::{CALLOUT_KINDS, CELL_OPTION_KEYS, INPUT_TYPES, THEOREM_KINDS};
```

- [ ] **Step 3: Back `xref_label` with a `pub(crate)` const and re-export it**

In `crates/core/src/cite/render.rs`, replace the `xref_label` function (lines 10-27) with:

```rust
/// Cross-reference kind prefixes -> display label, in canonical order. The single source
/// of truth for both `xref_label` (the lookup) and the editor `vocab` dump, so the two
/// cannot drift. The parallel bare-prefix list in `site::xref::is_ref_anchor` is guarded
/// against this one by a unit test there.
pub(crate) const XREF_LABELS: &[(&str, &str)] = &[
    ("fig", "Figure"),
    ("tbl", "Table"),
    ("sec", "Section"),
    ("eq", "Equation"),
    ("lst", "Listing"),
    ("thm", "Theorem"),
    ("lem", "Lemma"),
    ("cor", "Corollary"),
    ("prp", "Proposition"),
    ("def", "Definition"),
    ("exm", "Example"),
    ("rem", "Remark"),
];

/// Cross-reference kind prefixes -> display label.
fn xref_label(prefix: &str) -> Option<&'static str> {
    XREF_LABELS.iter().find(|(p, _)| *p == prefix).map(|(_, l)| *l)
}
```

In `crates/core/src/cite/mod.rs`, add below the existing `pub use render::process;` (line 35):

```rust
pub(crate) use render::XREF_LABELS;
```

- [ ] **Step 4: Create the placeholder golden file**

Create `crates/core/assets/vocab/tali-vocab.json` with exactly:

```json
{}
```

(This is a placeholder so `include_str!` compiles; Step 6 blesses it to the real content.)

- [ ] **Step 5: Write the vocab module**

Create `crates/core/src/vocab.rs`:

```rust
//! Editor vocabulary dump for the VS Code companion's autocomplete.
//!
//! Emits, as one JSON blob, every closed-set body construct taliesin recognizes:
//! front-matter keys (top-level + nested), cell options, callout/theorem kinds,
//! structural div classes, input types, and cross-reference prefixes. The lists are
//! sourced from the SAME consts the validator and `check` use, so completions can never
//! drift from what `check` enforces. Human descriptions are additive doc text authored
//! here (the consts carry none). Golden-file-locked like `schema.rs`: regenerate ONLY via
//! `QMD_FAST_BLESS=1 cargo test -p taliesin-core --lib vocab`, never hand-edit.

use serde_json::{Value, json};

/// The committed vocabulary JSON, bundled so the `taliesin vocab` CLI can print it verbatim
/// (no runtime generation), exactly as `schema.rs` bundles the schemas.
pub const VOCAB_JSON: &str = include_str!("../assets/vocab/tali-vocab.json");

/// `[{ "name", "description" }]` for each key in `names`, looking each description up in
/// `desc` (missing -> empty string, which the `descriptions_present` test forbids).
fn named(names: &[&str], desc: &[(&str, &str)]) -> Value {
    Value::Array(
        names
            .iter()
            .map(|n| {
                let d = desc.iter().find(|(k, _)| k == n).map(|(_, d)| *d).unwrap_or("");
                json!({ "name": n, "description": d })
            })
            .collect(),
    )
}

fn frontmatter_key_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        ("title", "The document or page title."),
        ("subtitle", "A secondary title shown under the title."),
        ("author", "Author name(s)."),
        ("date", "Publication date."),
        ("description", "Short summary used for listings and social cards."),
        ("lang", "Content language (BCP-47), for example `en`."),
        ("categories", "Tags used to group the page in listings."),
        ("image", "Social-card and listing thumbnail image path."),
        ("image-alt", "Alt text for `image`."),
        ("format", "Output format (for example `deck`); an extension owns its sub-keys."),
        ("theme", "Named theme or theme overrides."),
        ("css", "Extra CSS file(s) to include."),
        ("page-layout", "Page width and layout mode."),
        ("draft", "`true` excludes the page from a site build, nav, and listings."),
        ("title-block-style", "`none` suppresses the visible title header."),
        ("include-in-header", "Raw HTML injected into `<head>`."),
        ("include-before-body", "Raw HTML injected at the top of the body."),
        ("include-after-body", "Raw HTML injected at the end of the body."),
        ("toc", "Show a table of contents."),
        ("bibliography", "Path(s) to `.bib` file(s) for citations."),
        ("csl", "Citation Style Language file."),
        ("execute", "Document-level code-cell execution defaults."),
        ("listing", "Auto-generated listing of child pages."),
        ("about", "About-page block configuration."),
        ("hero", "Landing-page hero block configuration."),
        ("prose-lint", "Enable prose linting (`true` or `{ banned: [...] }`)."),
        ("theorems", "Theorem-environment numbering configuration."),
    ]
}

fn nested_key_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        // execute:
        ("echo", "Show the cell's source code."),
        ("include", "Include the cell's output."),
        ("cache", "Persist the cell's output in `_freeze/`."),
        // listing:
        ("contents", "Glob(s) of pages to include."),
        ("id", "Listing element id."),
        ("sort", "Sort field and order."),
        ("type", "Listing layout (`default`, `grid`, `table`)."),
        ("max-items", "Maximum entries shown."),
        ("categories", "Show a category filter."),
        // about:
        ("template", "About-page template."),
        ("image", "About-page image path."),
        ("links", "Social and contact links."),
        // hero:
        ("eyebrow", "Small label above the headline."),
        ("headline", "Hero headline."),
        ("lead", "Hero lead paragraph."),
        ("actions", "Call-to-action buttons."),
        // prose-lint:
        ("banned", "Words and phrases to flag."),
        // theorems:
        ("shared", "Kinds that share one counter."),
        ("number-within", "Reset numbering within `chapter`."),
        ("numbered", "Whether or when to number (`true`, `false`, `unless-unique`)."),
        // shared across blocks (about/listing reuse these):
        ("image-alt", "Alt text for the image."),
    ]
}

fn cell_option_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        ("echo", "Show the cell's source code."),
        ("include", "Include the cell's output."),
        ("cache", "Persist the cell's output in `_freeze/`."),
        ("label", "Cross-reference id (for example `fig-scree`)."),
        ("fig-cap", "Figure caption."),
        ("lst-cap", "Listing (code) caption."),
        ("tbl-cap", "Table caption."),
        ("fig-export", "Export the figure as a file."),
        ("code-fold", "Collapse the code block (`true` or `show`)."),
        ("code-summary", "Summary label for a folded code block."),
        ("code-line-numbers", "Show or highlight code line numbers."),
        ("name", "Reactive `{js}` cell name that other cells can depend on."),
        ("viewof", "Bind a `{js}` input control to this name."),
        ("input", "Reactive `{js}` inputs this cell depends on."),
    ]
}

fn callout_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        ("note", "Informational callout."),
        ("tip", "Helpful tip callout."),
        ("warning", "Warning callout."),
        ("important", "Important callout."),
        ("caution", "Caution callout."),
    ]
}

fn theorem_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        ("theorem", "Numbered theorem."),
        ("lemma", "Numbered lemma."),
        ("corollary", "Numbered corollary."),
        ("proposition", "Numbered proposition."),
        ("definition", "Numbered definition."),
        ("example", "Numbered example."),
        ("remark", "Numbered remark."),
        ("proof", "Proof block (unnumbered)."),
    ]
}

/// Structural fenced-div classes. There is no single Rust const for these (each is
/// dispatched by name in `render::divs` with styles in `assets/css/base.css`), so they are
/// enumerated here as their named home. Keep in sync with the `.class` dispatch in
/// `render/divs.rs` and the aliases wired in `base.css`.
fn div_classes() -> Value {
    named(
        &[
            "panel-tabset",
            "code-walkthrough",
            "scrolly",
            "magic-move",
            "column-margin",
            "aside",
            "sidenote",
        ],
        &[
            ("panel-tabset", "Tabbed panel; each `##` heading becomes a tab."),
            ("code-walkthrough", "Step-through narrated code."),
            ("scrolly", "Scroll-driven storytelling section."),
            ("magic-move", "Animated code diff between steps."),
            ("column-margin", "Place content in the margin."),
            ("aside", "Margin aside (alias of `column-margin`)."),
            ("sidenote", "Margin sidenote (alias of `column-margin`)."),
        ],
    )
}

fn xref_prefixes() -> Value {
    Value::Array(
        crate::cite::XREF_LABELS
            .iter()
            .map(|(prefix, label)| json!({ "prefix": prefix, "label": label }))
            .collect(),
    )
}

/// Build the vocabulary JSON from the validator's consts.
pub fn vocab() -> Value {
    use crate::frontmatter::{
        ABOUT_KEYS, EXECUTE_KEYS, HERO_KEYS, KNOWN_KEYS, LISTING_KEYS, PROSE_LINT_KEYS,
        THEOREM_KEYS,
    };
    use crate::render::{CALLOUT_KINDS, CELL_OPTION_KEYS, INPUT_TYPES, THEOREM_KINDS};

    let nested_desc = nested_key_descriptions();
    json!({
        "frontmatter": {
            "keys": named(KNOWN_KEYS, frontmatter_key_descriptions()),
            "nested": {
                "execute": named(EXECUTE_KEYS, nested_desc),
                "listing": named(LISTING_KEYS, nested_desc),
                "about": named(ABOUT_KEYS, nested_desc),
                "hero": named(HERO_KEYS, nested_desc),
                "prose-lint": named(PROSE_LINT_KEYS, nested_desc),
                "theorems": named(THEOREM_KEYS, nested_desc),
            }
        },
        "cellOptions": named(CELL_OPTION_KEYS, cell_option_descriptions()),
        "calloutKinds": named(CALLOUT_KINDS, callout_descriptions()),
        "theoremKinds": named(THEOREM_KINDS, theorem_descriptions()),
        "divClasses": div_classes(),
        "inputTypes": Value::Array(INPUT_TYPES.iter().map(|t| json!(t)).collect()),
        "xrefPrefixes": xref_prefixes(),
    })
}

/// Deterministic pretty JSON with a trailing newline (so the committed file ends cleanly),
/// matching `schema::generate::to_pretty_json`.
pub fn to_pretty_json() -> String {
    let mut s = serde_json::to_string_pretty(&vocab()).expect("vocab serializes");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert the generated JSON equals the committed file, OR (under `QMD_FAST_BLESS=1`)
    /// rewrite the committed file from the generator. Mirrors `schema.rs`.
    #[test]
    fn vocab_matches_committed() {
        let generated = to_pretty_json();
        if std::env::var("QMD_FAST_BLESS").is_ok() {
            let path = format!("{}/assets/vocab/tali-vocab.json", env!("CARGO_MANIFEST_DIR"));
            std::fs::write(&path, &generated).unwrap_or_else(|e| panic!("write {path}: {e}"));
            eprintln!("blessed assets/vocab/tali-vocab.json");
        } else {
            assert_eq!(
                generated, VOCAB_JSON,
                "vocab drift; regenerate with `QMD_FAST_BLESS=1 cargo test -p taliesin-core --lib vocab`"
            );
        }
    }

    /// Every name carries a non-empty description, so a new validator const forces the
    /// author to add doc text here instead of silently shipping a blank tooltip.
    #[test]
    fn descriptions_present() {
        fn check_named(v: &Value, where_: &str) {
            for item in v.as_array().unwrap() {
                let name = item["name"].as_str().unwrap();
                let desc = item["description"].as_str().unwrap();
                assert!(!desc.is_empty(), "empty description for `{name}` in {where_}");
            }
        }
        let v = vocab();
        check_named(&v["frontmatter"]["keys"], "frontmatter.keys");
        for parent in ["execute", "listing", "about", "hero", "prose-lint", "theorems"] {
            check_named(&v["frontmatter"]["nested"][parent], parent);
        }
        check_named(&v["cellOptions"], "cellOptions");
        check_named(&v["calloutKinds"], "calloutKinds");
        check_named(&v["theoremKinds"], "theoremKinds");
        check_named(&v["divClasses"], "divClasses");
    }

    /// The bundled string parses as JSON (catches an empty or corrupt committed file).
    #[test]
    fn bundled_vocab_is_valid_json() {
        serde_json::from_str::<Value>(VOCAB_JSON).expect("bundled vocab is valid JSON");
    }
}
```

- [ ] **Step 6: Register the module**

In `crates/core/src/lib.rs`, add alongside the other `pub mod` lines (after `pub mod site;` / near line 41):

```rust
pub mod vocab;
```

- [ ] **Step 7: Bless the golden file, then verify the lock holds**

Run: `QMD_FAST_BLESS=1 cargo test -p taliesin-core --lib vocab`
Expected: PASS; `crates/core/assets/vocab/tali-vocab.json` is rewritten with the real content.

Run: `cargo test -p taliesin-core --lib vocab`
Expected: PASS (3 tests: `vocab_matches_committed`, `descriptions_present`, `bundled_vocab_is_valid_json`) with no bless.

- [ ] **Step 8: Add the xref-prefix agreement test (bonus)**

In `crates/core/src/site/xref.rs`, inside its existing `#[cfg(test)] mod tests` block (or add one if none exists), add:

```rust
    /// The bare-prefix list in `is_ref_anchor` must recognize every cross-reference prefix
    /// that `cite::XREF_LABELS` defines, so the two parallel lists cannot drift apart.
    #[test]
    fn xref_anchor_recognizes_every_cite_prefix() {
        for (prefix, _) in crate::cite::XREF_LABELS {
            assert!(
                super::is_ref_anchor(&format!("{prefix}-x")),
                "cite prefix `{prefix}` is not a recognized ref anchor"
            );
        }
    }
```

(If `xref.rs` has no `tests` module, add `#[cfg(test)] mod tests { use super::*; ... }` at the end of the file. Confirm the `is_ref_anchor` path: it is `pub(super)`, so `super::is_ref_anchor` resolves from an inner `tests` module.)

- [ ] **Step 9: Run the full core test suite + clippy**

Run: `cargo test -p taliesin-core && cargo clippy -p taliesin-core --all-targets`
Expected: all PASS; no clippy warnings from the new code.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml crates/core/Cargo.toml crates/core/src/vocab.rs \
  crates/core/assets/vocab/tali-vocab.json crates/core/src/lib.rs \
  crates/core/src/render/mod.rs crates/core/src/cite/render.rs \
  crates/core/src/cite/mod.rs crates/core/src/site/xref.rs
git commit -m "feat(core): drift-proof vocab dump generated from validator consts"
```

---

### Task 4: `taliesin vocab` CLI command

**Files:**
- Modify: `crates/server/src/query.rs` (add `cmd_vocab`)
- Modify: `crates/server/src/main.rs` (dispatch, `COMMANDS`, `usage()`, `subcommand_help`, microcopy test)

**Interfaces:**
- Consumes: `taliesin_core::vocab::VOCAB_JSON` (Task 3).
- Produces: `pub(crate) fn cmd_vocab() -> std::process::ExitCode`

- [ ] **Step 1: Write the CLI command**

In `crates/server/src/query.rs`, add after `cmd_schema` (before the `preview` helper `fn preview`):

```rust
/// Emit the bundled editor vocabulary JSON (front-matter keys, cell options, callout and
/// theorem kinds, div classes, input types, cross-reference prefixes) so the VS Code
/// companion's autocomplete can never drift from what the validator enforces. Prints the
/// committed, bundled string (no runtime generation), like `cmd_schema`.
pub(crate) fn cmd_vocab() -> ExitCode {
    print!("{}", taliesin_core::vocab::VOCAB_JSON);
    ExitCode::SUCCESS
}
```

- [ ] **Step 2: Dispatch it from `main()`**

In `crates/server/src/main.rs`, add to the `match` (after the `schema` arm, line 44):

```rust
        Some("vocab") => query::cmd_vocab(),
```

- [ ] **Step 3: Add `vocab` to the command list and help**

In `crates/server/src/main.rs`, add `"vocab"` to `COMMANDS` (line 108-110):

```rust
const COMMANDS: &[&str] = &[
    "render", "build", "blocks", "schema", "vocab", "check", "init", "serve", "preview", "dev",
    "help",
];
```

In `usage()`, add a line after the `schema` line (after line 149):

```rust
    println!(
        "  vocab                      emit editor autocomplete vocabulary as JSON (companion)"
    );
```

In `subcommand_help`, add a `vocab` arm after the `schema` arm (after line 233):

```rust
        "vocab" => {
            "taliesin vocab\n\
             \n\
             Emit taliesin's editor vocabulary (front-matter keys, cell options, callout\n\
             and theorem kinds, div classes, cross-reference prefixes) as one JSON blob,\n\
             for the VS Code companion's autocomplete. Generated from the validator's own\n\
             lists, so it never drifts from what `check` enforces.\n\
             \n\
             Example:\n\
             \x20 taliesin vocab | jq .cellOptions\n"
        }
```

- [ ] **Step 4: Extend the microcopy test to cover `vocab`**

In `crates/server/src/main.rs`, in `cli_microcopy_tests::subcommand_help_covers_documented_commands`, add `"vocab"` to the array (line 280-282):

```rust
        for cmd in [
            "preview", "build", "check", "render", "schema", "vocab", "blocks", "init",
        ] {
```

- [ ] **Step 5: Run the server tests + a live smoke check**

Run: `cargo test -p taliesin-server`
Expected: PASS (including `subcommand_help_covers_documented_commands`).

Run: `cargo run -p taliesin-server -- vocab | head -c 200`
Expected: prints the start of the vocab JSON (`{ "frontmatter": ...`).

Run: `cargo run -p taliesin-server -- --help`
Expected: the `vocab` line appears in the COMMANDS list.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/query.rs crates/server/src/main.rs
git commit -m "feat(cli): add taliesin vocab command emitting the editor vocabulary JSON"
```

---

### Task 5: Pure completion-context detection + live scans (`src/complete.ts`)

**Files:**
- Create: `editor/vscode/src/complete.ts`
- Test: `editor/vscode/src/test/complete.test.ts`

**Interfaces:**
- Produces:
  - `type CompletionContext = { kind: "none" } | { kind: "frontmatter-key"; parent: string | null } | { kind: "cell-option" } | { kind: "div-class" } | { kind: "xref"; typed: string } | { kind: "cite" }`
  - `function detectContext(linePrefix: string, docPrefix: string): CompletionContext`
  - `function harvestAnchorIds(docText: string): string[]`
  - `function harvestBibKeys(bibText: string): string[]`
  - `function frontmatterBibPaths(docText: string): string[]`

- [ ] **Step 1: Write the failing test**

Create `editor/vscode/src/test/complete.test.ts`:

```ts
import { test } from "node:test";
import assert from "node:assert";
import {
  detectContext,
  harvestAnchorIds,
  harvestBibKeys,
  frontmatterBibPaths,
} from "../complete";

const FM_OPEN = "---\ntitle: T\n";

test("detects a top-level front-matter key position", () => {
  const ctx = detectContext("ti", FM_OPEN + "ti");
  assert.deepEqual(ctx, { kind: "frontmatter-key", parent: null });
});

test("detects a nested front-matter key under a known parent", () => {
  const doc = FM_OPEN + "execute:\n  ec";
  const ctx = detectContext("  ec", doc);
  assert.deepEqual(ctx, { kind: "frontmatter-key", parent: "execute" });
});

test("no front-matter completion once the block is closed", () => {
  const doc = "---\ntitle: T\n---\n\nBody ti";
  assert.deepEqual(detectContext("Body ti", doc), { kind: "none" });
});

test("no front-matter completion at a value position (after the colon)", () => {
  const doc = FM_OPEN + "author: ";
  assert.deepEqual(detectContext("author: ", doc), { kind: "none" });
});

test("detects a cell option after #| inside a code cell", () => {
  const doc = "```{python}\n#| ec";
  assert.deepEqual(detectContext("#| ec", doc), { kind: "cell-option" });
});

test("detects a cell option after //| in a js cell", () => {
  const doc = "```{js}\n//| na";
  assert.deepEqual(detectContext("//| na", doc), { kind: "cell-option" });
});

test("no cell-option completion for #| outside a code cell", () => {
  // The fence opened and closed before this line, so we are back in prose.
  const doc = "```{python}\nx = 1\n```\n\n#| ec";
  assert.deepEqual(detectContext("#| ec", doc), { kind: "none" });
});

test("detects a div class after :::{. and after ::: {.", () => {
  assert.deepEqual(detectContext(":::{.no", ":::{.no"), { kind: "div-class" });
  assert.deepEqual(detectContext("::: {.cal", "::: {.cal"), { kind: "div-class" });
});

test("detects an xref after @ and captures the typed prefix", () => {
  assert.deepEqual(detectContext("See @fig-", "See @fig-"), { kind: "xref", typed: "fig-" });
  assert.deepEqual(detectContext("See @", "See @"), { kind: "xref", typed: "" });
});

test("does not treat an email @ as an xref", () => {
  assert.deepEqual(detectContext("mail me at bob@", "mail me at bob@"), { kind: "none" });
});

test("detects a citation inside [@ ...", () => {
  assert.deepEqual(detectContext("see [@sm", "see [@sm"), { kind: "cite" });
  assert.deepEqual(detectContext("see [@a2020; @b", "see [@a2020; @b"), { kind: "cite" });
});

test("harvestAnchorIds pulls {#prefix-id} anchors and #sec- heading ids", () => {
  const doc = "## Intro {#sec-intro}\n\n![x](y){#fig-scree}\n\nplain {#not-a-ref}\n";
  const ids = harvestAnchorIds(doc);
  assert.ok(ids.includes("sec-intro"));
  assert.ok(ids.includes("fig-scree"));
  // Only cross-reference-prefixed ids are useful for @xref; a bare id is still harvested
  // (the provider filters by the typed prefix), so assert the two ref ids are present.
});

test("harvestBibKeys reads @type{key, entries", () => {
  const bib = "@article{smith2020, title={X}}\n@book{jones-2019, title={Y}}\n";
  assert.deepEqual(harvestBibKeys(bib).sort(), ["jones-2019", "smith2020"]);
});

test("frontmatterBibPaths reads a scalar and a list bibliography field", () => {
  assert.deepEqual(frontmatterBibPaths("---\nbibliography: refs.bib\n---\n"), ["refs.bib"]);
  const listed = frontmatterBibPaths("---\nbibliography:\n  - a.bib\n  - b.bib\n---\n");
  assert.deepEqual(listed.sort(), ["a.bib", "b.bib"]);
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd editor/vscode && npm run compile-tests && node --test out/test/complete.test.js`
Expected: FAIL (cannot find module `../complete`).

- [ ] **Step 3: Write the minimal implementation**

Create `editor/vscode/src/complete.ts`:

```ts
// Pure completion-context detection + best-effort live-candidate scans. No `vscode` import,
// so it stays in the fast `node:test` loop. The static vocabulary is Rust-authoritative
// (fetched via `taliesin vocab`); this file only decides WHICH list applies and harvests
// document-defined ids / .bib keys, which are suggestion-only (check remains the arbiter).

export type CompletionContext =
  | { kind: "none" }
  | { kind: "frontmatter-key"; parent: string | null }
  | { kind: "cell-option" }
  | { kind: "div-class" }
  | { kind: "xref"; typed: string }
  | { kind: "cite" };

// Front-matter parents whose immediate children have their own vocabulary.
const NESTED_PARENTS = ["execute", "listing", "about", "hero", "prose-lint", "theorems"];

// Are we inside the leading `---` front-matter block at `docPrefix`'s end?
function inFrontmatter(docPrefix: string): boolean {
  const lines = docPrefix.split("\n");
  if (lines.length === 0 || lines[0].trim() !== "---") return false;
  // Closed if any line AFTER the opener (before the current line) is a lone `---` or `...`.
  for (let i = 1; i < lines.length - 1; i++) {
    const t = lines[i].trim();
    if (t === "---" || t === "...") return false;
  }
  return true;
}

// Count ``` fence lines before the cursor; an odd count means we are inside a code cell.
function inCodeCell(docPrefix: string): boolean {
  const lines = docPrefix.split("\n");
  let fences = 0;
  // Exclude the current (last) line: the `#|` line itself is inside the cell the opener began.
  for (let i = 0; i < lines.length - 1; i++) {
    if (/^\s*```/.test(lines[i])) fences++;
  }
  return fences % 2 === 1;
}

// The nearest less-indented ancestor key (ending in `:`) above an indented current line.
function nestedParent(docPrefix: string): string | null {
  const lines = docPrefix.split("\n");
  const current = lines[lines.length - 1];
  const indent = current.length - current.trimStart().length;
  if (indent === 0) return null;
  for (let i = lines.length - 2; i >= 0; i--) {
    const line = lines[i];
    if (line.trim() === "") continue;
    const lineIndent = line.length - line.trimStart().length;
    if (lineIndent < indent) {
      const m = /^([\w-]+):/.exec(line.trim());
      const key = m ? m[1] : null;
      return key && NESTED_PARENTS.includes(key) ? key : null;
    }
  }
  return null;
}

export function detectContext(linePrefix: string, docPrefix: string): CompletionContext {
  // Citation FIRST: `[@` contains `@`, so it must win over the xref rule.
  if (/\[@[^\]]*$/.test(linePrefix)) return { kind: "cite" };

  // Cross-reference: `@` not preceded by a word char (so an email local-part is skipped).
  const xref = /(^|[^\w@])@([\w-]*)$/.exec(linePrefix);
  if (xref) return { kind: "xref", typed: xref[2] };

  // Fenced-div class: `:::{.` or `::: {.` then a partial class name.
  if (/:::\s*\{\.[\w-]*$/.test(linePrefix)) return { kind: "div-class" };

  // Cell option: a `#|` / `//|` / `%%|` directive line, key position, inside a code cell.
  if (/^\s*(#\||\/\/\||%%\|)\s*[\w-]*$/.test(linePrefix) && inCodeCell(docPrefix)) {
    return { kind: "cell-option" };
  }

  // Front-matter key: inside the `---` block, at a key position (only a partial word so far).
  if (inFrontmatter(docPrefix) && /^\s*[\w-]*$/.test(linePrefix)) {
    return { kind: "frontmatter-key", parent: nestedParent(docPrefix) };
  }

  return { kind: "none" };
}

// Harvest `{#id}` anchors (heading ids + figure/table/etc. labels) from the buffer, for
// @xref completion. Suggestion-only; the provider filters by the typed prefix.
export function harvestAnchorIds(docText: string): string[] {
  const ids = new Set<string>();
  const re = /\{#([\w-]+)\}/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(docText)) !== null) ids.add(m[1]);
  return [...ids];
}

// Harvest BibTeX citation keys (`@type{key,`) from a .bib file's text.
export function harvestBibKeys(bibText: string): string[] {
  const keys = new Set<string>();
  const re = /@\w+\s*\{\s*([^,\s}]+)\s*,/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(bibText)) !== null) keys.add(m[1]);
  return [...keys];
}

// Read the front-matter `bibliography:` field (scalar or list) as raw path strings.
export function frontmatterBibPaths(docText: string): string[] {
  const lines = docText.split("\n");
  if (lines[0]?.trim() !== "---") return [];
  const out: string[] = [];
  for (let i = 1; i < lines.length; i++) {
    const t = lines[i].trim();
    if (t === "---" || t === "...") break;
    const scalar = /^bibliography:\s*(.+)$/.exec(lines[i]);
    if (scalar && scalar[1].trim() !== "") {
      out.push(scalar[1].trim().replace(/^["']|["']$/g, ""));
      continue;
    }
    if (/^bibliography:\s*$/.test(lines[i])) {
      // A YAML list follows: subsequent `  - path` lines.
      for (let j = i + 1; j < lines.length; j++) {
        const item = /^\s*-\s*(.+)$/.exec(lines[j]);
        if (!item) break;
        out.push(item[1].trim().replace(/^["']|["']$/g, ""));
      }
    }
  }
  return out;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd editor/vscode && npm run compile-tests && node --test out/test/complete.test.js`
Expected: PASS (all tests).

- [ ] **Step 5: Type-check**

Run: `cd editor/vscode && npx tsc -p . --noEmit`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add editor/vscode/src/complete.ts editor/vscode/src/test/complete.test.ts
git commit -m "feat(companion): pure completion-context detection + live-candidate scans"
```

---

### Task 6: Completion provider wiring + registration + e2e (`src/completions.ts`)

**Files:**
- Create: `editor/vscode/src/completions.ts`
- Modify: `editor/vscode/src/extension.ts` (call `registerCompletions` from `activate`)
- Create: `editor/vscode/test-fixtures/complete.tmd`
- Modify: `editor/vscode/src/e2e/suite/integration.test.ts` (add a completion test)
- Modify: `editor/vscode/README.md` (F5-checklist addendum)

**Interfaces:**
- Consumes: `detectContext`, `harvestAnchorIds`, `harvestBibKeys`, `frontmatterBibPaths` (Task 5); the `taliesin vocab` JSON (Task 4).
- Produces: `function registerCompletions(context: vscode.ExtensionContext): void`

- [ ] **Step 1: Write the completion provider wiring**

Create `editor/vscode/src/completions.ts`:

```ts
import * as vscode from "vscode";
import { spawn } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import {
  detectContext,
  harvestAnchorIds,
  harvestBibKeys,
  frontmatterBibPaths,
} from "./complete";

interface Named {
  name: string;
  description: string;
}
interface Vocab {
  frontmatter: { keys: Named[]; nested: Record<string, Named[]> };
  cellOptions: Named[];
  calloutKinds: Named[];
  theoremKinds: Named[];
  divClasses: Named[];
  inputTypes: string[];
  xrefPrefixes: { prefix: string; label: string }[];
}

// Spawn `taliesin vocab` and parse its JSON. Rejects on spawn failure or bad JSON.
function fetchVocab(binary: string): Promise<Vocab> {
  return new Promise((resolve, reject) => {
    let stdout = "";
    const child = spawn(binary, ["vocab"]);
    child.on("error", (e) => reject(e));
    child.stdout?.on("data", (b) => (stdout += b.toString()));
    child.on("close", () => {
      try {
        resolve(JSON.parse(stdout) as Vocab);
      } catch (e) {
        reject(e);
      }
    });
  });
}

function item(label: string, detail: string, kind: vscode.CompletionItemKind): vscode.CompletionItem {
  const ci = new vscode.CompletionItem(label, kind);
  if (detail) ci.detail = detail;
  return ci;
}

export function registerCompletions(context: vscode.ExtensionContext): void {
  let cached: Promise<Vocab> | undefined;
  const binaryPath = () =>
    vscode.workspace.getConfiguration("qmdFast").get<string>("path", "qmd-fast");
  const vocab = () => (cached ??= fetchVocab(binaryPath()));

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("qmdFast.path")) cached = undefined; // re-fetch next request
    })
  );

  const provider: vscode.CompletionItemProvider = {
    async provideCompletionItems(document, position) {
      const linePrefix = document.getText(
        new vscode.Range(position.line, 0, position.line, position.character)
      );
      const docPrefix = document.getText(new vscode.Range(0, 0, position.line, position.character));
      const ctx = detectContext(linePrefix, docPrefix);
      if (ctx.kind === "none") return undefined;

      let v: Vocab;
      try {
        v = await vocab();
      } catch {
        return undefined; // no binary / bad vocab -> stay quiet, no completions
      }
      const K = vscode.CompletionItemKind;

      switch (ctx.kind) {
        case "frontmatter-key": {
          const list = ctx.parent ? v.frontmatter.nested[ctx.parent] ?? [] : v.frontmatter.keys;
          return list.map((n) => item(n.name, n.description, K.Property));
        }
        case "cell-option":
          return v.cellOptions.map((n) => item(n.name, n.description, K.Property));
        case "div-class": {
          const callouts = v.calloutKinds.map((n) =>
            item(`callout-${n.name}`, n.description, K.Class)
          );
          const theorems = v.theoremKinds.map((n) => item(n.name, n.description, K.Class));
          const divs = v.divClasses.map((n) => item(n.name, n.description, K.Class));
          return [...callouts, ...theorems, ...divs];
        }
        case "xref": {
          const prefixes = v.xrefPrefixes.map((p) =>
            item(`${p.prefix}-`, p.label, K.Reference)
          );
          const ids = harvestAnchorIds(document.getText())
            .filter((id) => ctx.typed === "" || id.startsWith(ctx.typed))
            .map((id) => item(id, "cross-reference target", K.Reference));
          return [...prefixes, ...ids];
        }
        case "cite": {
          const dir = path.dirname(document.fileName);
          const keys = new Set<string>();
          for (const rel of frontmatterBibPaths(document.getText())) {
            try {
              const text = fs.readFileSync(path.resolve(dir, rel), "utf8");
              for (const k of harvestBibKeys(text)) keys.add(k);
            } catch {
              /* missing/unreadable .bib -> skip */
            }
          }
          return [...keys].map((k) => item(k, "citation key", K.Reference));
        }
      }
      return undefined;
    },
  };

  context.subscriptions.push(
    vscode.languages.registerCompletionItemProvider(
      { language: "taliesin" },
      provider,
      "@",
      ".",
      "|",
      "-"
    )
  );
}
```

- [ ] **Step 2: Register it in `activate()`**

In `editor/vscode/src/extension.ts`, add the import:

```ts
import { registerCompletions } from "./completions";
```

and call it in `activate()` after `registerDiagnostics(context);`:

```ts
  registerDiagnostics(context);
  registerCompletions(context);
```

- [ ] **Step 3: Create the completion e2e fixture**

Create `editor/vscode/test-fixtures/complete.tmd`. The file must contain a Python code cell (for the `#|` trigger) and a `::: {.` div opener (for the div-class trigger). Write it with this exact structure (a fenced code block, then a div opener line):

Line 1: `---`
Line 2: `title: Completion Fixture`
Line 3: `---`
Line 4: (empty)
Line 5: ` ```{python} ` (a fence opener; three backticks then `{python}`, no surrounding spaces)
Line 6: `#|` (the directive marker, cursor goes right after it)
Line 7: `x = 1`
Line 8: ` ``` ` (three backticks, closing the fence)
Line 9: (empty)
Line 10: `::: {.` (the div opener, cursor goes right after the dot)

You can create it with a heredoc to get the backticks exactly right:

```bash
cat > editor/vscode/test-fixtures/complete.tmd <<'EOF'
---
title: Completion Fixture
---

```{python}
#|
x = 1
```

::: {.
EOF
```

- [ ] **Step 4: Write the failing e2e test**

In `editor/vscode/src/e2e/suite/integration.test.ts`, add the fixture constant near the others:

```ts
const COMPLETE_FIXTURE = path.join(REPO_ROOT, "editor/vscode/test-fixtures/complete.tmd");
```

and add this test inside the `suite(...)` block:

```ts
  test("offers cell-option and div-class completions", async () => {
    await vscode.workspace
      .getConfiguration("qmdFast")
      .update("path", QMD_FAST_BIN, vscode.ConfigurationTarget.Global);

    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(COMPLETE_FIXTURE));
    const text = doc.getText().split("\n");
    const cellLine = text.findIndex((l) => l.startsWith("#|"));
    const divLine = text.findIndex((l) => l.startsWith("::: {."));
    assert.ok(cellLine >= 0 && divLine >= 0, "fixture must contain a #| line and a ::: {. line");

    const cellPos = new vscode.Position(cellLine, 2); // right after `#|`
    const cellList = (await vscode.commands.executeCommand(
      "vscode.executeCompletionItemProvider",
      doc.uri,
      cellPos
    )) as vscode.CompletionList;
    const cellLabels = cellList.items.map((i) => labelText(i.label));
    assert.ok(cellLabels.includes("echo"), `cell options should include echo: ${cellLabels}`);

    const divPos = new vscode.Position(divLine, 6); // right after `::: {.`
    const divList = (await vscode.commands.executeCommand(
      "vscode.executeCompletionItemProvider",
      doc.uri,
      divPos
    )) as vscode.CompletionList;
    const divLabels = divList.items.map((i) => labelText(i.label));
    assert.ok(divLabels.includes("callout-note"), `div classes should include callout-note: ${divLabels}`);
    assert.ok(divLabels.includes("theorem"), `div classes should include theorem: ${divLabels}`);
  });
```

and add this helper at the bottom of the file (next to `waitFor`):

```ts
function labelText(label: string | vscode.CompletionItemLabel): string {
  return typeof label === "string" ? label : label.label;
}
```

- [ ] **Step 5: Build the binary and run the e2e suite**

Run: `cargo build -p taliesin-server`
Then: `cd editor/vscode && npm run test:e2e`
Expected: all e2e tests PASS, including the new completion test and the Task 2 diagnostics test.

- [ ] **Step 6: Run the pure unit tests + type-check**

Run: `cd editor/vscode && npm test && npx tsc -p . --noEmit`
Expected: all `node:test` PASS; no type errors.

- [ ] **Step 7: Update the README F5 checklist**

In `editor/vscode/README.md`, append this bullet under the existing F5 acceptance checklist (add it verbatim; adjust only the leading marker to match the list's `-` or `*` style):

```markdown
- Autocomplete fires inside front matter, after `#|` in a code cell, after `:::{.`, after `@`, and inside `[@ ]`, offering keys, cell options, callout/theorem/div classes, cross-reference prefixes, and citation keys from `taliesin vocab`.
```

- [ ] **Step 8: Commit**

```bash
git add editor/vscode/src/completions.ts editor/vscode/src/extension.ts \
  editor/vscode/test-fixtures/complete.tmd \
  editor/vscode/src/e2e/suite/integration.test.ts editor/vscode/README.md
git commit -m "feat(companion): autocomplete from the drift-proof taliesin vocab dump"
```

---

## Final verification (run before handing back)

- [ ] `cargo test` (whole workspace) PASS
- [ ] `cargo clippy --all-targets` clean
- [ ] `cargo fmt --check` clean
- [ ] `cd editor/vscode && npm test` PASS (node:test: `check`, `complete`, plus existing `paths`/`ports`/`grammar`)
- [ ] `cd editor/vscode && npx tsc -p . --noEmit` clean
- [ ] `cd editor/vscode && npm run test:e2e` PASS (registration + diagnostics + completions)
- [ ] Dispatch `rust-reviewer` + `corpus-verifier` on the Rust changes; re-read the diff yourself before any merge.

## Notes on scope & deferrals (from the spec)

- Diagnostics are save-triggered, whole-line, all-`Warning` in v1 (the core `Warning` layer has no severity/column). Richer `check` JSON (level/column/rule code + quick-fixes), live-buffer `check --stdin`, and hover/go-to-definition are Phase 3, not in this plan.
- No browser/chrome-devtools verification: these features render no HTML, so the regression net is the Rust golden-file/unit tests plus the extension's `node:test` + electron e2e against the `test-fixtures/` docs. This is the deliberate, spec-sanctioned deviation from the usual "pin a target corpus doc" rule.
