# Editor DevX: E3 (column-accurate diagnostics) + E5 (outline + go-to-definition)

Date: 2026-07-21
Backlog: `notes/backlog.md` §A "Editor DevX / language-server initiative", items E3 + E5.
Audit: `notes/2026-07-21-vscode-devx-audit.md`.

## Why

The VS Code companion already surfaces diagnostics, hover, and completion, but two gaps
remain in the daily-authoring loop:

- **E3.** Diagnostics squiggle the **whole line**, because the `check` JSON carries no
  column. E1's quick-fix then *guesses* the bad token with a Levenshtein `suggestionSpan`
  heuristic (`check.ts`). Precise columns give exact squiggles and a guess-free quick-fix
  span.
- **E5.** `.tmd` files have **no document outline** (Outline view / breadcrumbs / sticky
  scroll / `Ctrl+Shift+O`) and **no go-to-definition** (`@fig-x` → figure, `{{< include
  x.tmd >}}` → file, `[@key]` → `.bib` entry). `taliesin symbols` exists but is not wired
  to a `DocumentSymbolProvider`/`DefinitionProvider`.

Both align with the **single-editing-surface** invariant: the editor is the only authoring
surface, so authoring intelligence lives here. Both are read-only; neither writes back to
the preview.

## Scope decisions (approved 2026-07-21)

- E3 columns cover the **front-matter key-typo family first** — the most common author typo,
  and the one place a column is genuinely cheap (the validators already locate the key line in
  the front-matter *source* via `block_key_line`/`nested_key_line`, so the key token's column is
  the line's indentation). Not a uniform sweep of every validator. The plumbing is additive
  (`col`/`end_col` default `None`), so other validators opt in later.
- **xref-typo columns are explicitly deferred:** `validate_xrefs` scans the *rendered HTML*
  (`data-qmd-xref` markers) and only has the block's start line, not the `@fig-x` token's source
  column. Re-deriving it from source is fragile (code-block false matches, first-occurrence
  ambiguity), so xref diagnostics stay whole-line + the existing `suggestionSpan` quick-fix
  fallback. `_site.yml` config-key typos are a natural later opt-in via the same span plumbing.
- E5 outline is **headings only** (figures/tables stay reachable via go-to-definition).
- E5 xref go-to-definition is **same-document only**; a cross-file xref degrades to "no
  definition" rather than jump to a guessed file.

## Non-goals

- No LSP server (that is E7, its own spec). These stay standalone providers.
- No new output format, no preview write-back, no network access.
- No uniform column retrofit of every validator; no cross-file xref navigation; no
  figures/tables in the outline tree.

---

## E3 — Column-accurate diagnostic ranges

### Data model (Rust)

`taliesin_core::render::Warning` (`crates/core/src/render/model.rs`) gains an optional
character span:

```rust
pub struct Warning {
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub col: Option<u32>,      // 1-based start column (Unicode scalar count on the line)
    pub end_col: Option<u32>,  // 1-based, exclusive: one past the token's last scalar
}
```

- `Warning::new` sets `col = end_col = None`.
- `at(file, line)` is unchanged (leaves the span `None`).
- New chainable builder: `fn span(mut self, col: u32, end_col: u32) -> Self` sets both. Used
  as `Warning::new(msg).at(file, line).span(c0, c1)`.

**Column convention:** 1-based, counted in Unicode scalar values from the start of the line,
half-open `[col, end_col)`. For ASCII tokens (every front-matter key, xref id, cite key, and
config key) scalar count == UTF-16 unit count == byte count, so the extension maps it to a
`vscode.Range` directly. Astral-plane characters *before* a flagged token on the same line
(vanishingly rare) are the one documented inaccuracy; it degrades to a slightly-off range,
never a panic.

### Diagnostic serialization (`crates/server/src/check.rs`)

`Diagnostic` gains `col: Option<u32>` and `end_col: Option<u32>`, populated from the
`Warning` via `diag_from`. Serialized with `#[serde(skip_serializing_if = "Option::is_none")]`
so:

- an un-columned diagnostic serializes **byte-identically** to today (existing JSON pins in
  `crates/server/tests/check_cli.rs` and `mcp_stdio.rs` stay green), and
- `--format human` is untouched (it prints its own `file:line:` lines, ignoring struct fields).

### Which validators get a span (first cut)

**Front-matter key typos** (`crates/core/src/frontmatter.rs`) — the unknown-top-level-key,
unknown-nested-key, unsupported-key, and `format:`-sub-key diagnostics, i.e. every diagnostic
whose flagged token *is a front-matter key*. These already locate the key line via
`block_key_line`/`nested_key_line`, which scan the front-matter source; extending those helpers
to also return the key's column is trivial (top-level keys are unindented, so col 1; nested keys
sit at their indentation, plus an optional `- ` list prefix). Front-matter keys and indentation
are ASCII, so scalar == byte == UTF-16 column, and the span is exact.

Front-matter **value** typos (`format: pdff`) keep pointing at the key line **without** a span
(whole-line), because their locator reports the key line, not the value token's column; giving
them a span would squiggle the key, not the bad value. Only the key-*is*-the-token cases get a
span. `_site.yml` config-key typos are a later opt-in via the same plumbing; xref typos are
deferred (see Scope decisions). The deliverable is "every front-matter key-typo diagnostic now
carries an exact span," a clean, testable boundary.

### Extension consumption

- `editor/vscode/src/check.ts`:
  - `CheckDiag` and `DiagShape` gain optional `col`/`endCol` (wire fields `col`/`end_col`,
    matching the existing snake_case→camel mapping for `docs_url`).
  - `parseCheckJson` reads and validates them (numbers only; a malformed one is dropped).
  - `toDiagnostics` carries them through, still clamping the line.
- `editor/vscode/src/diagnostics.ts` (the vscode wiring): when a shape has a valid span,
  build `new vscode.Range(line0, col0, line0, endCol0)` (0-based, clamped to the line length);
  otherwise keep the whole-line range.
- Quick-fix (`suggestion`): when a diagnostic has **both** a `suggestion` **and** a span, use
  the exact span for the `WorkspaceEdit` and **skip `suggestionSpan`**. When it has a
  suggestion but no span (older binary / un-columned family), fall back to `suggestionSpan`
  (unchanged). `suggestionSpan` and `editDistance` remain for that fallback; nothing is deleted.

### E3 tests (TDD)

- **Rust unit:** `validate_front_matter` over a known unknown-key typo (top-level and nested)
  carries the expected `col`/`end_col` on the key token (a `crates/core/src/frontmatter.rs` unit
  test, near the existing lint tests). Mutation check: return `col = None` from the span helper
  and watch the pin fail.
- **Rust CLI:** `crates/server/tests/check_cli.rs` (or `mcp_stdio.rs`) over a corpus doc with a
  front-matter key typo asserts the JSON carries `col`/`end_col`; a doc with only un-columned
  findings still serializes byte-identically (no `col` key).
- **Extension `node:test`** (`editor/vscode/src/test/check.test.ts`): `parseCheckJson` reads
  `col`/`end_col`; `toDiagnostics` preserves a span and clamps a line; a diagnostic with span +
  suggestion resolves its quick-fix from the span, not `suggestionSpan`.

---

## E5 — Document outline + go-to-definition (extension-only, no Rust change)

### DocumentSymbolProvider (outline)

New pure module `editor/vscode/src/outline.ts` (vscode-free, fast `node:test` loop, mirroring
`check.ts`/`hover.ts`):

```ts
export interface OutlineNode {
  title: string; level: number;      // 1..6
  startLine: number; endLine: number; // 0-based, inclusive of the section body
  children: OutlineNode[];
}
export function outline(text: string): OutlineNode[];
```

- Scan ATX headings `^(#{1,6})\s+(.*)$`.
- Skip lines inside fenced code blocks (track ` ``` ` / `~~~` open/close) and inside the
  leading `---` front-matter block (reuse the fence/front-matter helpers' logic).
- `title` = heading text with a trailing `{#id}`/`{.class}` attribute block and inline
  markdown emphasis stripped; empty title falls back to the raw heading text.
- `endLine` = the line before the next heading at the same or higher level (last heading runs
  to EOF), so folding and sticky scroll cover the whole section.
- Nest by level with a stack.

A thin `DocumentSymbolProvider` shell converts `OutlineNode` → `vscode.DocumentSymbol`
(`SymbolKind.String`, full range = `[startLine, endLine]`, selection range = the heading line).

### DefinitionProvider (go-to-definition)

Reuses E4's `classifyHover` (`hover.ts`), extended with a new target kind:

```ts
| { kind: "include"; path: string; start: number; end: number } // {{< include PATH >}} / {{< embed PATH >}}
```

`classifyHover` detects the `{{< include PATH >}}` / `{{< embed PATH >}}` shortcode and spans
the PATH token. Existing `xref`/`cite`/`frontmatter-key` kinds are unchanged.

A thin `DefinitionProvider` shell (`editor/vscode/src/definition-provider.ts`) resolves each:

- **include** → resolve `path` relative to the document's directory; return a `Location` at
  `(targetUri, 0:0)` if the file exists, else `undefined`. (No backend; pure path + `fs.existsSync`.)
- **xref** → pure `definitionSite(text, id)` in a new/extended helper: first occurrence of the
  bare id (`fig-x`) **not** immediately preceded by `@` (references are `@fig-x`; definitions
  are `{#fig-x}`, `label: fig-x`, `{{< ... id=fig-x >}}`). Return `{ line, col }` → a `Location`
  in the same document. `null` ⇒ `undefined` (cross-file or genuinely undefined: don't guess).
- **cite** → reuse `frontmatterBibPaths` to locate `.bib` files and a new `bibEntryOffset(text,
  key)` (a sibling of `bibEntryFor` returning the match offset). Convert the offset to a
  line/col over the `.bib` text; return a `Location` in the `.bib` `TextDocument`.

All three are offline, instant, and buffer/filesystem-only — no `taliesin` subprocess.

### Registration (`editor/vscode/src/extension.ts`)

`activate` gains `registerDocumentSymbols(context)` and `registerDefinitions(context)`
alongside the existing `registerDiagnostics`/`registerCompletions`/`registerHover`. Both are
scoped to `{ language: "taliesin" }`.

### E5 tests (TDD, `node:test`)

- `outline.test.ts`: nesting by level; skips headings inside a fenced code block and the
  front-matter block; strips a trailing `{#id}`; section `endLine` runs to the next same/higher
  heading.
- `definition.test.ts`: `definitionSite` finds a `{#fig-x}`/`label: fig-x` def and ignores the
  `@fig-x` reference; returns `null` for an undefined id; `bibEntryOffset` returns the entry
  offset and `null` for a missing key; `classifyHover` classifies an `include` PATH token and
  its `[start,end)`.

The provider shells (vscode-dependent) stay thin; correctness lives in the pure helpers, as
with the existing hover/completion providers.

---

## Order of work

1. E3 Rust: `Warning` span + `.span()` builder + `Diagnostic` fields + serialization (byte-safe).
2. E3 Rust: `block_key_span`/`nested_key_span` + populate the span for the front-matter
   key-typo diagnostics, with unit + CLI pins.
3. E3 extension: `check.ts` carry-through + `diagnostics.ts` range + exact-span quick-fix, with
   `check.test.ts` additions.
4. E5 outline: `outline.ts` + `DocumentSymbolProvider` shell + `outline.test.ts` + registration.
5. E5 go-to-def: `classifyHover` include kind + `definitionSite`/`bibEntryOffset` +
   `DefinitionProvider` shell + `definition.test.ts` + registration.

Steps 1–3 are one logical change (schema + consumer); 4 and 5 are independent extension-only
additions. Verify with `cargo test -p taliesin-core` + `cargo test -p taliesin-server` (three
kernel gates where relevant) and the extension `npm test` (node:test), plus a manual VS Code
sanity check of squiggle precision, outline, and F12.
