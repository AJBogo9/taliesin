# Editor DevX E3 + E5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (chosen: inline) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `.tmd` front-matter key-typo diagnostics exact column ranges + a guess-free quick-fix (E3), and add a document outline + go-to-definition to the VS Code companion (E5).

**Architecture:** E3 threads an optional `[col, end_col)` character span through `Warning` → `check.rs::Diagnostic` → JSON → the extension's squiggle + quick-fix, populated where the column is cheap (front-matter keys). E5 is extension-only: a pure heading scanner drives a `DocumentSymbolProvider`, and E4's `classifyHover` (+ two new pure helpers) drives a `DefinitionProvider` for `@xref`/`{{< include >}}`/`[@cite]`, resolved from the buffer + `.bib` with no backend call.

**Tech Stack:** Rust (edition 2024, `taliesin-core` + `taliesin-server`), serde_json; TypeScript VS Code extension with `node:test` pure-function tests.

## Global Constraints

- No new output format, no preview write-back, no network access (offline invariant).
- JSON must stay **byte-identical** for un-columned diagnostics: new `Diagnostic` fields use `#[serde(skip_serializing_if = "Option::is_none")]`.
- Columns are **1-based Unicode-scalar** counts, half-open `[col, end_col)`; front-matter keys/indentation are ASCII so scalar == byte == UTF-16.
- `--format human` output is unchanged.
- Extension pure helpers stay `vscode`-import-free (fast `node:test` loop), mirroring `check.ts`/`hover.ts`; provider shells are thin.
- Run Rust with the three kernel gates where relevant; `rustfmt` runs on save (keep the tree fmt-clean). Commit after each task.

---

### Task 1: E3 — front-matter key-typo columns (Rust core)

**Files:**
- Modify: `crates/core/src/render/model.rs` (Warning: `col`/`end_col` + `.span()`)
- Modify: `crates/core/src/frontmatter.rs` (`block_key_span`/`nested_key_span`; attach span to key-typo warnings)
- Test: `crates/core/src/frontmatter.rs` (`#[cfg(test)]` unit tests, alongside existing lint tests)

**Interfaces:**
- Produces: `Warning { …, col: Option<u32>, end_col: Option<u32> }`; `Warning::span(col: u32, end_col: u32) -> Warning`; `frontmatter::block_key_span(block, key) -> Option<(u32,u32,u32)>` (line, col, end_col, 1-based); `nested_key_span(block, parent, key) -> Option<(u32,u32,u32)>`.

- [ ] **Step 1: Write the failing test** (append to `frontmatter.rs` tests)

```rust
#[test]
fn unknown_top_level_key_carries_a_column_span() {
    let src = "---\ntitle: X\ntreme: darkly\n---\n";
    let w = validate_front_matter(src);
    let d = w.iter().find(|w| w.message.contains("`treme`")).expect("treme flagged");
    assert_eq!(d.line, Some(3));
    assert_eq!(d.col, Some(1)); // `treme` starts at column 1
    assert_eq!(d.end_col, Some(6)); // one past the 5-char key
}

#[test]
fn unknown_nested_key_carries_an_indented_column_span() {
    let src = "---\ntitle: X\nexecute:\n  eccho: false\n---\n";
    let w = validate_front_matter(src);
    let d = w.iter().find(|w| w.message.contains("`eccho`")).expect("eccho flagged");
    assert_eq!(d.line, Some(4));
    assert_eq!(d.col, Some(3)); // 2-space indent -> column 3
    assert_eq!(d.end_col, Some(8));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p taliesin-core --lib frontmatter::tests::unknown 2>&1 | tail -20`
Expected: FAIL — `col`/`end_col` are not fields of `Warning` (compile error), or the values are `None`.

- [ ] **Step 3: Add the span to `Warning`** (`crates/core/src/render/model.rs`)

```rust
pub struct Warning {
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    /// 1-based start column (Unicode-scalar count on the line); `None` = whole-line.
    pub col: Option<u32>,
    /// 1-based, exclusive end column; set together with `col`.
    pub end_col: Option<u32>,
}
```

In `Warning::new`, add `col: None, end_col: None`. Add the builder:

```rust
    /// Attach a `[col, end_col)` character span on the located line (1-based, exclusive end).
    pub fn span(mut self, col: u32, end_col: u32) -> Self {
        self.col = Some(col);
        self.end_col = Some(end_col);
        self
    }
```

- [ ] **Step 4: Add span locators + attach them** (`crates/core/src/frontmatter.rs`)

Refactor the two line-locators to compute a span, keeping the `_line` names delegating so non-span callers are untouched:

```rust
/// (line, col, end_col) of a top-level `key:` in the front-matter block, all 1-based.
/// Top-level keys are unindented, so `col` is 1. Column is a scalar count; keys are ASCII.
pub(crate) fn block_key_span(block: &str, key: &str) -> Option<(u32, u32, u32)> {
    block.lines().enumerate().find_map(|(i, line)| {
        let t = line.trim_start();
        (line.len() == t.len() && key_matches(t, key))
            .then(|| (i as u32 + 2, 1, 1 + key.chars().count() as u32))
    })
}

pub(crate) fn block_key_line(block: &str, key: &str) -> Option<u32> {
    block_key_span(block, key).map(|(l, _, _)| l)
}

/// (line, col, end_col) of a nested `key:` under `parent:` — column follows the indentation
/// and an optional `- ` list prefix. All 1-based, scalar columns (indentation is ASCII).
fn nested_key_span(block: &str, parent: &str, key: &str) -> Option<(u32, u32, u32)> {
    let mut in_block = false;
    for (i, line) in block.lines().enumerate() {
        let t = line.trim_start();
        let at_top = line.len() == t.len();
        if !in_block {
            if at_top && key_matches(t, parent) {
                in_block = true;
            }
            continue;
        }
        if at_top {
            break;
        }
        let indent = line.len() - t.len();
        let (prefix, body) = match t.strip_prefix("- ") {
            Some(rest) => (2 + (rest.len() - rest.trim_start().len()), rest.trim_start()),
            None => (0, t),
        };
        if key_matches(body, key) {
            let col = indent as u32 + prefix as u32 + 1;
            return Some((i as u32 + 2, col, col + key.chars().count() as u32));
        }
    }
    None
}

fn nested_key_line(block: &str, parent: &str, key: &str) -> Option<u32> {
    nested_key_span(block, parent, key).map(|(l, _, _)| l)
}
```

Add a span-aware `located` sibling and use it for the key-typo call sites:

```rust
/// A `Warning` located at a `[col,end_col)` span, from a `(line, col, end_col)` locator.
fn located_span(message: String, span: Option<(u32, u32, u32)>) -> Warning {
    match span {
        Some((l, c, e)) => Warning::new(message).at(None, l).span(c, e),
        None => Warning::new(message),
    }
}
```

Update the **unknown-key** sites to carry the span:
- top-level unknown key (in `validate_front_matter`): replace `let line = block_key_line(block, key); out.push(located(unknown_key_message(...), line));` with `out.push(located_span(unknown_key_message("front-matter key", key, KNOWN_KEYS), block_key_span(block, key)));`
- nested unknown key (in `validate_nested`): replace `let line = nested_key_line(block, parent, key); out.push(located(unknown_key_message(what, key, allowed), line));` with `out.push(located_span(unknown_key_message(what, key, allowed), nested_key_span(block, parent, key)));`
- unsupported keys (`validate_unsupported_keys`) and `format:` sub-keys (`validate_format_subkeys`): same pattern, swap `block_key_line`→`block_key_span` / `nested_key_line`→`nested_key_span` and `located`→`located_span`.

Leave **value** diagnostics (`validate_format_value`, `validate_page_layout_value`, `validate_date_value`, `validate_theorem_values`) on `located`/`block_key_line` (no span — they point at the key line, not the value token).

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p taliesin-core --lib frontmatter 2>&1 | tail -20`
Expected: PASS (new span tests + existing frontmatter tests green).

- [ ] **Step 6: Mutation check**

Temporarily make `block_key_span` return `col=0,end_col=0` (or the delegating `_line` return `None` span); re-run — the two new tests fail. Revert.

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/render/model.rs crates/core/src/frontmatter.rs
git commit -m "feat(core): front-matter key-typo warnings carry a column span (E3)"
```

---

### Task 2: E3 — Diagnostic col/end_col + JSON serialization (CLI)

**Files:**
- Modify: `crates/server/src/check.rs` (`Diagnostic` fields + `diag_from`)
- Test: `crates/server/tests/check_cli.rs`

**Interfaces:**
- Consumes: `Warning.col`/`Warning.end_col` (Task 1).
- Produces: JSON diagnostic objects with optional `col`/`end_col` integer keys (present only when the warning carried a span).

- [ ] **Step 1: Write the failing test** (append to `check_cli.rs`)

```rust
#[test]
fn check_json_front_matter_typo_carries_a_column_span() {
    let (_ok, stdout, _e) = run(&["check", &corpus("diagnostics/typos.tmd"), "--format", "json"]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let diags = parsed["diagnostics"].as_array().expect("diagnostics array");
    let treme = diags
        .iter()
        .find(|d| d["message"].as_str().is_some_and(|m| m.contains("`treme`")))
        .expect("treme diagnostic present");
    assert_eq!(treme["col"], 1);
    assert_eq!(treme["end_col"], 6);
    // An un-columned finding (a broken xref) must NOT carry the keys (byte-stable JSON).
    let xref = diags
        .iter()
        .find(|d| d["code"] == "TAL-XREF-UNDEF")
        .expect("xref diagnostic present");
    assert!(xref.get("col").is_none(), "un-columned diag omits col: {xref}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p taliesin-server --test check_cli check_json_front_matter_typo 2>&1 | tail -20`
Expected: FAIL — `treme["col"]` is null (field not serialized yet).

- [ ] **Step 3: Add the fields + populate** (`crates/server/src/check.rs`)

In `struct Diagnostic`, after `line`:

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    col: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_col: Option<u32>,
```

In `Diagnostic::new`, set `col: None, end_col: None`. In `diag_from`, after building via `Diagnostic::new(...)`, copy the span:

```rust
pub(crate) fn diag_from(w: &taliesin_core::render::Warning, fallback_file: &str) -> Diagnostic {
    let mut d = Diagnostic::new(
        w.file.clone().unwrap_or_else(|| fallback_file.to_string()),
        w.line,
        w.message.clone(),
    );
    d.col = w.col;
    d.end_col = w.end_col;
    d
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p taliesin-server --test check_cli 2>&1 | tail -20`
Expected: PASS (new + existing check_cli tests green).

- [ ] **Step 5: Full check_cli + mcp_stdio regression** (byte-stability guard)

Run: `cargo test -p taliesin-server --test check_cli --test mcp_stdio 2>&1 | tail -20`
Expected: PASS — existing JSON pins unaffected (un-columned diags omit the keys).

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/check.rs crates/server/tests/check_cli.rs
git commit -m "feat(check): serialize col/end_col for columned diagnostics (E3)"
```

---

### Task 3: E3 — extension precise squiggle + guess-free quick-fix

**Files:**
- Modify: `editor/vscode/src/check.ts` (`CheckDiag`/`DiagShape` col fields; `fixSpan` helper)
- Modify: `editor/vscode/src/diagnostics.ts` (precise Range; quick-fix uses span)
- Test: `editor/vscode/src/test/check.test.ts`

**Interfaces:**
- Consumes: JSON `col`/`end_col` (Task 2).
- Produces: `CheckDiag`/`DiagShape` gain `col?: number; endCol?: number`; `fixSpan(entry: { replacement: string; span?: { start: number; end: number } }, lineText: string): { start: number; end: number } | null`.

- [ ] **Step 1: Write the failing tests** (append to `check.test.ts`)

```ts
import { parseCheckJson, toDiagnostics, suggestionSpan, fixSpan } from "../check";

test("parseCheckJson reads col/end_col when present", () => {
  const out = parseCheckJson(
    JSON.stringify({ diagnostics: [{ file: "a.tmd", line: 3, message: "unknown key `treme`", col: 1, end_col: 6 }] })
  );
  assert.equal(out.kind, "diags");
  assert.equal((out as any).diags[0].col, 1);
  assert.equal((out as any).diags[0].endCol, 6);
});

test("toDiagnostics carries the column span through", () => {
  const out = parseCheckJson(
    JSON.stringify({ diagnostics: [{ file: "a.tmd", line: 3, message: "x", col: 1, end_col: 6 }] })
  );
  const shapes = toDiagnostics(out, 10);
  assert.equal(shapes[0].col, 1);
  assert.equal(shapes[0].endCol, 6);
});

test("fixSpan prefers an exact span over the edit-distance guess", () => {
  // With a span, the bad token's location is known: no suggestionSpan guess.
  assert.deepEqual(fixSpan({ replacement: "theme", span: { start: 0, end: 5 } }, "treme: dark"), {
    start: 0,
    end: 5,
  });
});

test("fixSpan falls back to suggestionSpan when no span is present", () => {
  assert.deepEqual(fixSpan({ replacement: "theme" }, "treme: dark"), suggestionSpan("treme: dark", "theme"));
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd editor/vscode && npm run compile-tests && node --test out/test/check.test.js 2>&1 | tail -20`
Expected: FAIL — `fixSpan` is not exported; `col`/`endCol` undefined.

- [ ] **Step 3: Carry col/endCol + add `fixSpan`** (`check.ts`)

In `CheckDiag` add `col?: number; endCol?: number;` (wire fields `col`/`end_col`). In `parseCheckJson`'s mapper, after the suggestion block:

```ts
        if (typeof d.col === "number") diag.col = d.col;
        if (typeof d.end_col === "number") diag.endCol = d.end_col;
```

In `DiagShape` add `col?: number; endCol?: number;`. In `toDiagnostics`'s mapper, after `suggestion`:

```ts
    if (d.col !== undefined) shape.col = d.col;
    if (d.endCol !== undefined) shape.endCol = d.endCol;
```

Append the pure quick-fix resolver:

```ts
// The [start,end) span a "did you mean" fix should overwrite. When the diagnostic carried an
// exact column span (E3), use it verbatim (guess-free); otherwise fall back to locating the
// token by edit distance (older binary / un-columned family). 0-based columns on the line.
export function fixSpan(
  entry: { replacement: string; span?: { start: number; end: number } },
  lineText: string
): { start: number; end: number } | null {
  return entry.span ?? suggestionSpan(lineText, entry.replacement);
}
```

- [ ] **Step 4: Wire the extension** (`diagnostics.ts`)

Change the `suggestionOf` value type to carry the optional span, and import `fixSpan`:

```ts
import { parseCheckJson, toDiagnostics, fixSpan } from "./check";
const suggestionOf = new WeakMap<vscode.Diagnostic, { replacement: string; span?: { start: number; end: number } }>();
```

In the code-action provider, replace the `suggestionSpan(...)` call:

```ts
          for (const diag of ctx.diagnostics) {
            const entry = suggestionOf.get(diag);
            if (!entry) continue;
            const line = diag.range.start.line;
            const span = fixSpan(entry, document.lineAt(line).text);
            if (!span) continue;
            const action = new vscode.CodeAction(`Change to \`${entry.replacement}\``, vscode.CodeActionKind.QuickFix);
            action.edit = new vscode.WorkspaceEdit();
            action.edit.replace(document.uri, new vscode.Range(line, span.start, line, span.end), entry.replacement);
            action.diagnostics = [diag];
            action.isPreferred = true;
            actions.push(action);
          }
```

In `refresh`, build a precise range + store the span with the suggestion:

```ts
      const line0 = Math.max(0, Math.min(s.line0, doc.lineCount - 1));
      const lineLen = doc.lineAt(line0).text.length;
      const range =
        s.col !== undefined && s.endCol !== undefined
          ? new vscode.Range(line0, Math.min(s.col - 1, lineLen), line0, Math.min(s.endCol - 1, lineLen))
          : doc.lineAt(line0).range; // whole-line when no column
      const d = new vscode.Diagnostic(range, s.message, severityOf(s.severity));
      d.source = "taliesin check";
      if (s.code) d.code = s.docsUrl ? { value: s.code, target: vscode.Uri.parse(s.docsUrl) } : s.code;
      if (s.suggestion) {
        const span = s.col !== undefined && s.endCol !== undefined ? { start: s.col - 1, end: s.endCol - 1 } : undefined;
        suggestionOf.set(d, { replacement: s.suggestion.replacement, span });
      }
      return d;
```

(Remove the now-unused `suggestionSpan` import from `diagnostics.ts`; it lives in `check.ts`.)

- [ ] **Step 5: Run tests + type-check**

Run: `cd editor/vscode && npm run compile-tests && node --test out/test/*.test.js 2>&1 | tail -25`
Expected: PASS (new + existing node:tests).

- [ ] **Step 6: Commit**

```bash
git add editor/vscode/src/check.ts editor/vscode/src/diagnostics.ts editor/vscode/src/test/check.test.ts
git commit -m "feat(companion): precise squiggle + guess-free quick-fix from column span (E3)"
```

---

### Task 4: E5 — document outline (DocumentSymbolProvider)

**Files:**
- Create: `editor/vscode/src/outline.ts` (pure `outline(text)`)
- Create: `editor/vscode/src/outline-provider.ts` (thin `DocumentSymbolProvider` shell)
- Modify: `editor/vscode/src/extension.ts` (register)
- Test: `editor/vscode/src/test/outline.test.ts`

**Interfaces:**
- Produces: `outline(text: string): OutlineNode[]` where `OutlineNode = { title: string; level: number; startLine: number; endLine: number; children: OutlineNode[] }` (0-based lines, `endLine` inclusive of the section body); `registerDocumentSymbols(context: vscode.ExtensionContext): void`.

- [ ] **Step 1: Write the failing tests** (`outline.test.ts`)

```ts
import { test } from "node:test";
import assert from "node:assert";
import { outline } from "../outline";

test("nests headings by level", () => {
  const t = "# A\n\ntext\n\n## B\n\n## C\n";
  const tree = outline(t);
  assert.equal(tree.length, 1);
  assert.equal(tree[0].title, "A");
  assert.deepEqual(tree[0].children.map((c) => c.title), ["B", "C"]);
});

test("a section runs to just before the next same-or-higher heading", () => {
  const t = "# A\nl1\n## B\nl3\n# C\n"; // lines 0..4
  const tree = outline(t);
  assert.equal(tree[0].endLine, 3); // A's body ends before `# C`
  assert.equal(tree[0].children[0].endLine, 3); // B ends before `# C`
  assert.equal(tree[1].startLine, 4);
});

test("ignores headings inside a fenced code block", () => {
  const t = "# Real\n\n```\n# not a heading\n```\n";
  const tree = outline(t);
  assert.deepEqual(tree.map((n) => n.title), ["Real"]);
});

test("ignores the front-matter block and strips a trailing {#id}", () => {
  const t = "---\ntitle: X\n# fake\n---\n# Intro {#sec-intro}\n";
  const tree = outline(t);
  assert.deepEqual(tree.map((n) => n.title), ["Intro"]);
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd editor/vscode && npm run compile-tests && node --test out/test/outline.test.js 2>&1 | tail -20`
Expected: FAIL — module `../outline` not found.

- [ ] **Step 3: Implement `outline.ts`**

```ts
// Pure document-outline extraction for `.tmd`: the ATX-heading tree. No `vscode` import, so it
// stays in the fast `node:test` loop (mirrors check.ts/hover.ts). Skips headings inside fenced
// code blocks and the leading `---` front-matter block; strips a trailing attribute block.
export interface OutlineNode {
  title: string;
  level: number; // 1..6
  startLine: number; // 0-based
  endLine: number; // 0-based, inclusive of the section body
  children: OutlineNode[];
}

interface Flat {
  title: string;
  level: number;
  line: number;
}

// Heading text minus a trailing `{#id}`/`{.class}` attribute block and inline emphasis markers.
function cleanTitle(raw: string): string {
  const noAttr = raw.replace(/\s*\{[^}]*\}\s*$/, "").trim();
  const noEmph = noAttr.replace(/[*_`]/g, "").trim();
  return noEmph || raw.trim();
}

function headings(text: string): Flat[] {
  const lines = text.split("\n");
  const out: Flat[] = [];
  let inFence = false;
  let fence = "";
  let start = 0;
  // Skip a leading `---` front-matter block.
  if (lines[0]?.trim() === "---") {
    for (let i = 1; i < lines.length; i++) {
      const t = lines[i].trim();
      if (t === "---" || t === "...") {
        start = i + 1;
        break;
      }
    }
  }
  for (let i = start; i < lines.length; i++) {
    const line = lines[i];
    const fenceOpen = /^\s*(```+|~~~+)/.exec(line);
    if (fenceOpen) {
      const marker = fenceOpen[1][0];
      if (!inFence) {
        inFence = true;
        fence = marker;
      } else if (fence === marker) {
        inFence = false;
      }
      continue;
    }
    if (inFence) continue;
    const m = /^(#{1,6})\s+(.*)$/.exec(line);
    if (m) out.push({ title: cleanTitle(m[2]), level: m[1].length, line: i });
  }
  return out;
}

export function outline(text: string): OutlineNode[] {
  const flat = headings(text);
  const lineCount = text.split("\n").length;
  const roots: OutlineNode[] = [];
  const stack: OutlineNode[] = [];
  for (let i = 0; i < flat.length; i++) {
    const h = flat[i];
    // endLine: the line before the next heading at the same or higher level (else EOF).
    let end = lineCount - 1;
    for (let j = i + 1; j < flat.length; j++) {
      if (flat[j].level <= h.level) {
        end = flat[j].line - 1;
        break;
      }
    }
    const node: OutlineNode = { title: h.title, level: h.level, startLine: h.line, endLine: end, children: [] };
    while (stack.length && stack[stack.length - 1].level >= h.level) stack.pop();
    if (stack.length) stack[stack.length - 1].children.push(node);
    else roots.push(node);
    stack.push(node);
  }
  return roots;
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd editor/vscode && npm run compile-tests && node --test out/test/outline.test.js 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Implement the provider shell + register** (`outline-provider.ts`)

```ts
import * as vscode from "vscode";
import { outline, OutlineNode } from "./outline";
import { isSourceFile } from "./paths";

function toSymbol(doc: vscode.TextDocument, n: OutlineNode): vscode.DocumentSymbol {
  const lastLine = Math.max(0, doc.lineCount - 1);
  const start = Math.min(n.startLine, lastLine);
  const end = Math.min(Math.max(n.endLine, n.startLine), lastLine);
  const full = new vscode.Range(start, 0, end, doc.lineAt(end).text.length);
  const selection = new vscode.Range(start, 0, start, doc.lineAt(start).text.length);
  const sym = new vscode.DocumentSymbol(n.title || "(untitled)", "", vscode.SymbolKind.String, full, selection);
  sym.children = n.children.map((c) => toSymbol(doc, c));
  return sym;
}

export function registerDocumentSymbols(context: vscode.ExtensionContext): void {
  const provider: vscode.DocumentSymbolProvider = {
    provideDocumentSymbols(document) {
      if (!isSourceFile(document.fileName)) return [];
      return outline(document.getText()).map((n) => toSymbol(document, n));
    },
  };
  context.subscriptions.push(
    vscode.languages.registerDocumentSymbolProvider({ language: "taliesin" }, provider)
  );
}
```

In `extension.ts`: `import { registerDocumentSymbols } from "./outline-provider";` and call `registerDocumentSymbols(context);` in `activate`.

- [ ] **Step 6: Type-check + full node:test**

Run: `cd editor/vscode && npx -y -p typescript tsc -p jsconfig.json 2>&1 | tail; npm run compile-tests && node --test out/test/*.test.js 2>&1 | tail -15`
Expected: type-check clean; all node:tests pass.

- [ ] **Step 7: Commit**

```bash
git add editor/vscode/src/outline.ts editor/vscode/src/outline-provider.ts editor/vscode/src/extension.ts editor/vscode/src/test/outline.test.ts
git commit -m "feat(companion): document outline (DocumentSymbolProvider) for .tmd (E5)"
```

---

### Task 5: E5 — go-to-definition (DefinitionProvider)

**Files:**
- Modify: `editor/vscode/src/hover.ts` (add `include` target kind to `classifyHover`; add `definitionSite` + `bibEntryOffset`)
- Create: `editor/vscode/src/definition-provider.ts` (thin `DefinitionProvider` shell)
- Modify: `editor/vscode/src/extension.ts` (register)
- Test: `editor/vscode/src/test/definition.test.ts`

**Interfaces:**
- Consumes: `classifyHover` (E4), `frontmatterBibPaths` (complete.ts), `bibEntryFor` (hover.ts).
- Produces: `HoverTarget` gains `{ kind: "include"; path: string; start: number; end: number }`; `definitionSite(text: string, id: string): { line: number; col: number } | null` (0-based line, 0-based col); `bibEntryOffset(bibText: string, key: string): number | null`; `registerDefinitions(context: vscode.ExtensionContext): void`.

- [ ] **Step 1: Write the failing tests** (`definition.test.ts`)

```ts
import { test } from "node:test";
import assert from "node:assert";
import { classifyHover, definitionSite, bibEntryOffset } from "../hover";

test("classifyHover classifies an include path token", () => {
  const line = "{{< include chapters/intro.tmd >}}";
  const t = classifyHover(line, 0, line.indexOf("chapters") + 2);
  assert.equal(t.kind, "include");
  assert.equal((t as any).path, "chapters/intro.tmd");
});

test("definitionSite finds a {#fig-x} definition and ignores the @fig-x reference", () => {
  const text = "See @fig-scree below.\n\n![Scree](s.png){#fig-scree}\n";
  const site = definitionSite(text, "fig-scree");
  assert.deepEqual(site, { line: 2, col: text.split("\n")[2].indexOf("fig-scree") });
});

test("definitionSite finds a `label: fig-x` cell definition", () => {
  const text = "```{python}\n#| label: fig-plot\nplot()\n```\n";
  const site = definitionSite(text, "fig-plot");
  assert.equal(site?.line, 1);
});

test("definitionSite returns null for an undefined id", () => {
  assert.equal(definitionSite("only @fig-x here\n", "fig-missing"), null);
});

test("bibEntryOffset returns the entry offset and null for a missing key", () => {
  const bib = "@article{smith20,\n  title = {T},\n}\n@book{jones19, title={B}}\n";
  assert.equal(bibEntryOffset(bib, "jones19"), bib.indexOf("@book"));
  assert.equal(bibEntryOffset(bib, "nope"), null);
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd editor/vscode && npm run compile-tests && node --test out/test/definition.test.js 2>&1 | tail -20`
Expected: FAIL — `definitionSite`/`bibEntryOffset` not exported; `include` kind absent.

- [ ] **Step 3: Extend `hover.ts`**

Add to the `HoverTarget` union:

```ts
  // `{{< include PATH >}}` / `{{< embed PATH >}}`. `path` is the target; [start,end) spans it.
  | { kind: "include"; path: string; start: number; end: number }
```

In `classifyHover`, before the `return { kind: "none" }`, add include detection (after the front-matter block):

```ts
  {
    const re = /\{\{<\s*(?:include|embed)\s+([^\s>]+)/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(lineText)) !== null) {
      const start = m.index + m[0].length - m[1].length;
      const end = start + m[1].length;
      if (covers(start, end, char)) return { kind: "include", path: m[1], start, end };
    }
  }
```

Append the two pure resolvers:

```ts
// The 0-based {line, col} where cross-reference id `id` is DEFINED in this document: the first
// occurrence preceded by `#` (`{#fig-x}`) or `label:` (a `#| label: fig-x` cell), never `@id`
// (a reference). null when undefined here (e.g. defined in another file) — the caller then
// offers no definition rather than jump to a guess.
export function definitionSite(text: string, id: string): { line: number; col: number } | null {
  const esc = id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(`(?:#|label:\\s*)(${esc})(?![\\w-])`, "g");
  const m = re.exec(text);
  if (!m) return null;
  const idOffset = m.index + m[0].length - m[1].length;
  const before = text.slice(0, idOffset);
  const line = before.split("\n").length - 1;
  const col = idOffset - (before.lastIndexOf("\n") + 1);
  return { line, col };
}

// The byte offset of the BibTeX entry `@type{key,` for `key` in `bibText`, or null if absent.
// Sibling of `bibEntryFor`; the caller converts the offset to a document position.
export function bibEntryOffset(bibText: string, key: string): number | null {
  const esc = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(`@\\w+\\s*\\{\\s*${esc}\\s*,`, "g");
  const m = re.exec(bibText);
  return m ? m.index : null;
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd editor/vscode && npm run compile-tests && node --test out/test/definition.test.js 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Implement the provider shell + register** (`definition-provider.ts`)

```ts
import * as vscode from "vscode";
import * as fs from "node:fs";
import * as path from "node:path";
import { classifyHover, definitionSite, bibEntryOffset } from "./hover";
import { frontmatterBibPaths } from "./complete";
import { isSourceFile } from "./paths";

export function registerDefinitions(context: vscode.ExtensionContext): void {
  const provider: vscode.DefinitionProvider = {
    provideDefinition(document, position) {
      if (!isSourceFile(document.fileName)) return undefined;
      const text = document.getText();
      const target = classifyHover(text, position.line, position.character);
      const dir = path.dirname(document.fileName);

      switch (target.kind) {
        case "include": {
          const abs = path.resolve(dir, target.path);
          if (!fs.existsSync(abs)) return undefined;
          return new vscode.Location(vscode.Uri.file(abs), new vscode.Position(0, 0));
        }
        case "xref": {
          const site = definitionSite(text, target.id);
          if (!site) return undefined;
          return new vscode.Location(document.uri, new vscode.Position(site.line, site.col));
        }
        case "cite": {
          for (const rel of frontmatterBibPaths(text)) {
            try {
              const abs = path.resolve(dir, rel);
              const bib = fs.readFileSync(abs, "utf8");
              const off = bibEntryOffset(bib, target.key);
              if (off !== null) {
                const before = bib.slice(0, off);
                const line = before.split("\n").length - 1;
                const col = off - (before.lastIndexOf("\n") + 1);
                return new vscode.Location(vscode.Uri.file(abs), new vscode.Position(line, col));
              }
            } catch {
              /* missing/unreadable .bib -> try the next */
            }
          }
          return undefined;
        }
      }
      return undefined;
    },
  };
  context.subscriptions.push(
    vscode.languages.registerDefinitionProvider({ language: "taliesin" }, provider)
  );
}
```

In `extension.ts`: `import { registerDefinitions } from "./definition-provider";` and call `registerDefinitions(context);` in `activate`.

- [ ] **Step 6: Type-check + full node:test**

Run: `cd editor/vscode && npx -y -p typescript tsc -p jsconfig.json 2>&1 | tail; npm run compile-tests && node --test out/test/*.test.js 2>&1 | tail -15`
Expected: type-check clean; all node:tests pass.

- [ ] **Step 7: Commit**

```bash
git add editor/vscode/src/hover.ts editor/vscode/src/definition-provider.ts editor/vscode/src/extension.ts editor/vscode/src/test/definition.test.ts
git commit -m "feat(companion): go-to-definition for @xref, {{< include >}}, [@cite] (E5)"
```

---

### Task 6: Full regression + manual VS Code sanity

- [ ] **Step 1: Rust suite (three kernel gates)**

Run: `cargo test -p taliesin-core -p taliesin-server --test-threads=1 2>&1 | tail -25` (add the kernel env gates if available in this environment).
Expected: PASS (or only the known load-sensitive/concurrency-race flakes, re-run filtered).

- [ ] **Step 2: Extension type-check + tests + manifest gate**

Run: `cd editor/vscode && npx -y -p typescript tsc -p jsconfig.json && npm test 2>&1 | tail -25`
Expected: type-check clean; all node:tests pass (incl. `manifest.test.ts`).

- [ ] **Step 3: Manual sanity (documented, not automated)**

Build the binary (`cargo build`), open a `.tmd` with a front-matter typo in VS Code with the companion: (a) the squiggle covers only the key token, (b) the quick-fix replaces exactly the token, (c) the Outline view shows the heading tree, (d) F12 on `@fig-x`/`{{< include >}}`/`[@key]` jumps to the definition. Note results in the PR/summary.

- [ ] **Step 4: Update the backlog** — remove E3 + E5 from `notes/backlog.md` §A (mark shipped in the "Already shipped" anti-rot list), commit.

---

## Self-Review

- **Spec coverage:** E3 data model → Task 1; Diagnostic/JSON → Task 2; extension squiggle+quick-fix → Task 3; E5 outline → Task 4; E5 go-to-def (all three link types) → Task 5; regression + backlog → Task 6. All spec sections mapped.
- **Placeholders:** none — every code step shows the code; every run step shows the command + expected result.
- **Type consistency:** `Warning.col/end_col` (Task 1) consumed in `diag_from` (Task 2); JSON `col`/`end_col` → `CheckDiag.col/endCol` → `DiagShape.col/endCol` → `fixSpan` (Task 3); `OutlineNode` shape identical in Task 4's helper + provider; `HoverTarget` `include` kind + `definitionSite`/`bibEntryOffset` (Task 5) match the provider's `switch`.
