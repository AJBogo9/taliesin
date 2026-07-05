# Design: VS Code companion language features (diagnostics + completions)

Status: APPROVED (owner approved the design 2026-07-06). Branch: `vscode-language-features`.
Backlog items closed: *"Companion: surface `check`/prose-lint as editor diagnostics"* (the
"error highlight" ask) and a new sibling *"Companion: autocomplete"* (the "auto suggestion"
ask). Roadmap context: `BEYOND-QUARTO.md` Pillar II (editor companion); this is the
language-features layer that sits *beside* the already-built preview/cursor-sync (Phase 1),
independent of it.

## Why this, why now

The two asks map to real, backend-ready work:

- **Error highlight** → `taliesin check --format json <file>` already emits
  `[{file, line, message}]` today (`crates/server/src/check.rs:18-23,161-163,218-247`). No
  new rendering or validation engine is needed; this is *wiring* the existing checker into a
  VS Code `DiagnosticCollection`.
- **Auto suggestion** → the valid vocabulary already exists as validator consts in
  `crates/core` (front-matter `KNOWN_KEYS`, `CELL_OPTION_KEYS`, `CALLOUT_KINDS`,
  `THEOREM_KINDS`, `INPUT_TYPES`, the `xref_label` prefix map). Completions are *wiring* that
  vocabulary into a `CompletionItemProvider`.

Two properties make this the right next chunk rather than a Tier-2 owner-design call:

1. **The backend is already built** (checker + vocabulary), so the work is mostly wiring +
   tests, not a new subsystem.
2. **Independent of the one thing still blocked on the owner** — the companion's
   preview/cursor-sync is pending an F5 acceptance, but diagnostics and completions never
   touch the webview, the localhost iframe, or the relay. They are pure editor-side language
   features that shell out to the CLI, so they add daily value and carry no dependency on the
   preview relay working.

The current extension (`editor/vscode/`) has **zero** diagnostics/completion code
(`activate()` registers only `qmdFast.openPreview`) — this is a clean net-new layer.

## Approach: in-process providers shelling out to the CLI (not a full LSP)

- **A — In-process VS Code providers + CLI shell-out (CHOSEN).** A `CompletionItemProvider`
  and a `DiagnosticCollection` registered in `activate()`, backed by `taliesin check
  --format json` and a new `taliesin vocab` JSON dump. Matches the extension's existing
  architecture (it already spawns the binary for `preview`; no LSP deps today), reuses Rust
  validation/vocabulary rather than duplicating it, and tests with the existing `node:test`
  + `@vscode/test-electron` harnesses.
- **B — A real Language Server** (`vscode-languageclient` + a new `taliesin-lsp` crate).
  "Proper," enables incremental analysis / hover / go-to-def later, but a whole new
  subsystem and heavy for a single-author tool. **YAGNI, deferred** — nothing here forecloses
  it.
- **C — reimplement validation/vocab in TypeScript** — rejected up front by the owner's
  drift-proof choice (would fork the validator into a second implementation).

Decision: **A**.

## Architecture: one new "language features" layer, two phases

A net-new layer in `editor/vscode/src/`, registered next to the existing preview command.
Pure logic (JSON parsing, range mapping, completion-context detection) lives in
no-`vscode`-import modules so it stays in the fast `node:test` loop, mirroring how
`paths.ts` / `ports.ts` are structured today; only the thin registration wiring needs an
Electron e2e test.

- **Phase 1 — Diagnostics (error squiggles).** Zero Rust changes; ships first.
- **Phase 2 — Completions (autocomplete).** Adds a small drift-proof `taliesin vocab`
  command in Rust, then the completion provider.

The binary path for both comes from the existing `qmdFast.path` config (default
`qmd-fast` / the on-PATH launcher), read the same way `extension.ts` reads it for preview.

## Phase 1 — Diagnostics

### Modules
- `src/diagnostics.ts` — the impure wiring: spawn `taliesin check --format json <file>`,
  own a single `vscode.DiagnosticCollection`, and manage triggers + lifecycle.
- Pure helpers (no `vscode` import, `node:test`-covered):
  - `parseCheckJson(stdout): CheckDiag[] | {error: string}` — parse + validate the CLI's
    output shape, tolerating the `{"error": …}` variant.
  - `toDiagnostics(diags, lineCount): vscode.Diagnostic[]`-shaped data — map each
    `{file, line, message}` to a whole-line range + severity. (Returns plain objects the
    wiring turns into `vscode.Diagnostic`, so it stays `vscode`-free and testable.)

### Behavior
- **Triggers:** on open, on save, and on `qmdFast.path` config change. **Not** live-as-you-type
  in v1 — `check` reads from disk, so squiggles refresh on save. This matches the save-driven
  preview loop authors already use. (Live-buffer via a `check --stdin` mode is deliberate
  Phase 3, below.)
- **Range:** whole-line (the JSON carries a line, no column). Map comrak's 1-based line to a
  0-based VS Code range covering the whole line: `Range(line-1, 0, line-1, EOL)`. This reuses
  the `-1` line convention already in `extension.ts:49-54` / `paths.ts:parseSourcepos`.
- **Severity:** all `Warning` in v1 — that is what the core `Warning` layer actually models
  (`crates/core/src/render/model.rs:146-150` has no severity field). Honest v1 limitation;
  Phase 3 splits Error/Warning/Info.
- **Scope:** single active document only. Cross-page xref resolution needs a whole-site check;
  out of scope for the editor (would require `check <dir>` semantics and a project root).
- **Robustness:**
  - Missing binary / spawn error → clear the collection and stay quiet (no error spam); one
    optional one-time toast, gated so it never repeats per keystroke.
  - `{"error": …}` output → surface as a single document-level diagnostic (line 1) rather
    than dropping it.
  - Supersede an in-flight check when a newer save for the same document arrives (track the
    latest run per document URI; ignore stale results).
  - Non-zero exit is expected when diagnostics exist (the CLI returns `FAILURE` on findings) —
    parse stdout regardless of exit code; only treat spawn failure as an error.

### Tests
- `node:test`: `parseCheckJson` (valid array, `{error}`, malformed) + `toDiagnostics`
  (line→range, EOL clamp, empty).
- One `@vscode/test-electron` e2e: open a fixture `.tmd` with a typo'd front-matter key,
  assert exactly one diagnostic on the expected line with the did-you-mean message.

## Phase 2 — `taliesin vocab` + completions

### 2a. New `taliesin vocab` command (Rust, drift-proof)

A sibling of `taliesin schema`, in `crates/server/src/query.rs` (next to `cmd_schema`),
dispatched from `main.rs`. It prints one JSON blob **generated from the validator's own
consts** so completions can never drift from what `check` enforces:

```json
{
  "frontmatter": {
    "keys":   [{"name": "title", "description": "…"}],
    "nested": { "execute": [{"name":"echo","description":"…"}], "listing": [...], "about": [...],
                "hero": [...], "prose-lint": [...], "theorems": [...] }
  },
  "cellOptions":  [{"name": "echo", "description": "…"}],
  "calloutKinds": [{"name": "note", "description": "…"}],
  "theoremKinds": [{"name": "theorem", "description": "…"}],
  "divClasses":   [{"name": "panel-tabset", "description": "…"}, {"name":"column-margin", "…"}],
  "inputTypes":   ["slider","range","number","checkbox","text","select"],
  "xrefPrefixes": [{"prefix": "fig", "label": "Figure"}]
}
```

- **Source of truth:** the consts in `crates/core` — `frontmatter::KNOWN_KEYS` (+ the nested
  `EXECUTE_KEYS`/`LISTING_KEYS`/`ABOUT_KEYS`/`HERO_KEYS`/`PROSE_LINT_KEYS`/`THEOREM_KEYS`),
  `render::validate::{CELL_OPTION_KEYS, CALLOUT_KINDS, THEOREM_KINDS, INPUT_TYPES}`, and the
  prefix→label map behind `cite::render::xref_label`. These are `pub(crate)` today; expose
  them through a small `taliesin_core::vocab` module (a thin public surface that returns
  structured data), mirroring exactly how `schema.rs` already reads the same consts. No
  logic duplication — one authoritative list per construct.
- **Descriptions:** short one-line human descriptions authored once, in Rust, beside the
  vocab module (the consts carry none today). Names are drift-locked to the validator;
  descriptions are additive doc text. `divClasses` (`panel-tabset`, `code-walkthrough`,
  `scrolly`, `magic-move`, `.column-margin`/`.aside`/`.sidenote` aliases) have no single
  Rust const today — enumerate them explicitly in the vocab module with a comment pointing at
  their dispatch sites in `divs.rs` / `base.css` so the list has a named home.
- **Drift lock:** a golden-file test (`cargo test`, regen via an env flag such as
  `QMD_FAST_BLESS=1`) asserts the emitted JSON matches a committed golden file — the same
  pattern `schema.rs:192-208` uses. This catches unintended vocabulary changes.
- **xref-prefix note:** the prefix list is duplicated by hand today
  (`cite::render::xref_label` has the labels; `site::xref::is_ref_anchor` has a parallel bare
  list). Source `vocab` from `xref_label` (it carries the display labels) and leave a comment
  flagging the parallel `site::xref` list; optionally add a unit test asserting the two agree
  (bonus, low cost).

### 2b. Completion provider (`src/completions.ts`)

Registered for the `taliesin` language id. Spawns `taliesin vocab` once and caches the
parsed result (re-fetch on `qmdFast.path` change). Context detection is pure and
`node:test`-able; the provider wiring is thin.

| Context (detected from line + doc prefix)         | Completions offered                                   |
|---------------------------------------------------|-------------------------------------------------------|
| a key inside the leading `---` front-matter block | front-matter keys (nested keys when indented under a known parent like `execute:`) |
| after `#\|` / `//\|` / `%%\|` in a code cell      | cell-option keys                                      |
| after `:::{.` (or `::: {.`)                        | callout + theorem kinds + structural div classes      |
| `@` then a prefix (`fig-`, `sec-`, …)             | xref prefixes, then **live labels** scanned from the buffer |
| inside `[@ … ]`                                    | **citation keys** scanned from the front-matter `.bib` |

- **Static vocab** (keys / kinds / prefixes / div classes) is authoritative from the Rust
  `vocab` dump — never hand-listed in TS.
- **Live candidates** (label ids the doc defines, `.bib` keys) come from a lightweight
  regex scan — the open buffer for `{#<prefix>-<id>}` anchors + `{#sec-…}` heading ids, and
  the front-matter `bibliography:` `.bib` file(s) for `@type{key,` entries. This is
  acceptable because completions only *suggest*; `check` remains the arbiter of correctness,
  so a slightly-imperfect candidate list is low-stakes. The prefix *vocabulary* stays
  Rust-authoritative even though the ids are scanned.
- **Trigger characters:** `@`, `.`, `|`, `-` (plus normal invocation). Each completion item
  carries its description as detail/documentation where the vocab provides one.
- **Pure/testable split:** `detectContext(linePrefix, docPrefix)` and the candidate filters
  are pure (`node:test`); only `registerCompletionItemProvider` + the vocab spawn/cache need
  an e2e test.

### Tests
- `node:test`: `detectContext` across every row of the table above (including negatives:
  `@` inside an email, `#|` outside a cell, `.` outside a `:::{` attr) + the live-scan
  helpers (label harvest, `.bib` key harvest).
- One `@vscode/test-electron` e2e: trigger completion after `#|` in a fixture and assert the
  list contains `echo`/`eval`-class options; after `:::{.` assert it contains `note`.

## Testing & verification summary

- **Rust:** `cargo test -p taliesin-core` (vocab golden-file + unit) and
  `cargo test -p taliesin-server` (the `vocab` CLI surface); `rustfmt` (hook) + `clippy`;
  reviewed by `rust-reviewer` + `corpus-verifier`.
- **Extension:** `npm test` (node:test — pure modules) + `npm run test:e2e`
  (`@vscode/test-electron` — registration proofs) + `npx tsc` type-check.
- **No browser / chrome-devtools:** these features have no webview surface, so the corpus
  HTML arbiter does not cover them. The regression net *is* the Rust golden-file/unit tests
  plus the extension's node + electron tests, against small intentionally-broken/completable
  `.tmd` fixtures under the extension's test dir. This is a deliberate deviation from the
  usual "pin a target corpus doc" rule, justified: editor UX produces no rendered HTML to
  pin.
- README F5-checklist addendum for the two new features.

## Scope boundaries & invariants

- **Do-NOT-touch respected:** the exec/kernel zone is untouched (`check` executes no code,
  boots no kernel); the single-editing-surface invariant holds (diagnostics are read-only;
  completions insert at the cursor only on explicit user acceptance — ordinary editor
  behavior, never a preview-driven write-back).
- **Not the rebrand:** the manifest stays `qmd-fast-companion` / `qmdFast.path` / the
  `qmdFast.openPreview` command id. The Taliesin companion-manifest rebrand is a separate,
  deferred backlog item and is explicitly out of scope here (new code follows the existing
  ids so it folds cleanly into the rebrand later).
- **Language id:** features register for the existing `taliesin` language id (`.tmd`), the
  same one the grammar contributes.

## Explicitly deferred (Phase 3, not in this spec)

- **Richer `check` JSON** — add `level` (Error/Warning/Info), `col`/`end_col`, and a rule
  `code` to the CLI `Diagnostic`, threaded from the core `Warning` (which drops comrak's
  column at `diagnostics/helpers.rs`). Enables precise token-span squiggles + severity
  colors + did-you-mean **quick-fixes** (`CodeAction`) keyed by rule. A moderate cross-cutting
  change to the `Warning` call sites; sequenced after v1 lands.
- **Live-buffer diagnostics** — a `check --stdin --path <realpath>` mode so squiggles update
  on unsaved edits (read content from stdin, resolve relative includes/bib as if the doc
  lived at `<realpath>`), replacing the save-triggered v1.
- **Hover / go-to-definition** for xrefs and citations (would motivate approach B, a real
  LSP) — not now.

## Open decisions (none blocking)

None require an owner ruling before implementation. The two v1 simplifications
(save-triggered/whole-line/all-warning diagnostics; best-effort live completion candidates
with Rust-authoritative vocabulary) were both approved 2026-07-06.
