# `qmd-fast check` CLI (design)

Date: 2026-06-26
Status: approved (brainstorm), pre-implementation
Feature branch: `feat/check-cli`
Pillar: BEYOND-QUARTO.md Pillar I (authoring intelligence / the validation moat) +
FEATURE-IDEAS.md #39.

## Summary

A `qmd-fast check <file|dir>` subcommand that renders a document or site in memory,
collects every located diagnostic that already flows through the warning channel
(schema/front-matter, cell options, callout/`.input`/`.scrolly`/`.panel-tabset`/
`.code-walkthrough` validation, broken `@xref`, unknown shortcodes,
bibliography-not-found, opt-in prose-lint) and reports them with a CI-gating exit
code. It turns the live, click-to-source diagnostics into a publish gate that runs
in CI or a pre-commit hook, without a second tool.

It is **static-only** (no code execution, no kernel: fast and deterministic) and a
**pure server/CLI addition** (no `crates/core` change): it reuses the existing
rendering + warning machinery.

## Goals

- `qmd-fast check <file.qmd>` and `qmd-fast check <dir>` (a site project) both work,
  mirroring the `build`/`preview` file-vs-dir dispatch.
- Collect all located diagnostics from the warning channel; report each as
  `path:line: message`.
- `--format human` (default, colorized) and `--format json` (machine-readable).
- **Exit 0** when clean, **exit 1** when any diagnostic is found (the CI gate).
- Document it in `usage()` and the CLI reference.

## Non-goals (v1, YAGNI)

- **No code execution.** Cell runtime errors are environment-dependent and stay in
  `build`/`preview`; `check` is the deterministic static gate. (No kernel, no tokio
  runtime needed.)
- **No SARIF** (deferred; it needs rule ids, which means a `Warning` field change).
- **No `rule`/category field** in the JSON v1 (keeps `Warning` untouched). JSON is
  `[{file, line, message}]`.
- **No `--fix`/autofix**, no severity levels, no `--max-warnings N`. One gate: any
  diagnostic fails.
- **No core change.** Pure `crates/server` addition reusing public APIs.

## Invariants honoured

- **Read-only:** renders in memory, writes nothing, executes nothing.
- **No `crates/core` change:** reuses `render_document_with_includes`,
  `cite::validate_xrefs`, `Site::discover`, `Site::render_page_doc_warned`, all
  already public.
- **HTML-only identity untouched:** `check` produces diagnostics, not an output
  format.
- **Offline, kernel-free, deterministic.**

## CLI surface

```
qmd-fast check <file.qmd|dir> [--format human|json]
```

- `--format human` (default): one `path:line: message` per diagnostic on stderr
  (colorized via the existing `log` module), then a summary line (`N problems` or
  `no problems found`).
- `--format json`: a single JSON array `[{ "file": string, "line": number|null,
  "message": string }]` on **stdout** (and nothing else on stdout, so it pipes
  cleanly); the human summary, if any, goes to stderr.
- Exit code: **0** = no diagnostics; **1** = one or more diagnostics found, OR a
  usage error (unreadable path / empty site, with a distinct message). Any non-zero
  fails CI, which is the desired behaviour.

Dispatched from `main()` as `Some("check") => cmd_check(&args)`.

## Architecture

A `Diagnostic` value object and a pure collector, then a thin command wrapper:

```rust
struct Diagnostic { file: String, line: Option<u32>, message: String }

/// Render `path` (a file or a site dir) in memory and return every located
/// diagnostic from the warning channel. No execution, no output written.
fn collect_diagnostics(path: &Path) -> Result<Vec<Diagnostic>, String>;
```

`collect_diagnostics`:

- **File** (`!path.is_dir()`):
  - `let doc = render_document_with_includes(&src, base);`
  - diagnostics = `doc.warnings` + `cite::validate_xrefs(&doc.blocks)` (the same
    broken-xref pass the preview runs for standalone docs).
  - each `Warning { message, file, line }` maps to a `Diagnostic` with
    `file = warning.file.unwrap_or(path-as-string)` and `line = warning.line`.
- **Dir** (`path.is_dir()`, a site):
  - `let site = Site::discover(path);`
  - `site.warnings` (config-level `Vec<String>`, no location) → `Diagnostic { file:
    "_site.yml"-or-root, line: None, message }`.
  - empty site (`site.pages.is_empty()`) → `Err("no .qmd pages found under …")`.
  - for each `page`: read its source, `render_document_with_includes`, then
    `site.render_page_doc_warned(page, doc)` and take the returned `Vec<Warning>`
    (this includes cross-page `@xref` resolution). Each warning → `Diagnostic` with
    `file = warning.file.unwrap_or(page.rel)`, `line = warning.line`.
  - (No `exec::Executor` step: blocks are not executed.)

`cmd_check(args)`:
- parse the positional `<path>` and `--format` (default `human`); an unknown format
  value is a usage error.
- `match collect_diagnostics(path)`:
  - `Err(msg)` → `log::error(msg)`, return `ExitCode::FAILURE`.
  - `Ok(diags)` → format (human to stderr / json to stdout), then
    `if diags.is_empty() { SUCCESS } else { FAILURE }`.

Formatting helpers (pure, testable):
- `fn format_human(diags) -> String` (the `path:line: message` lines; line shown as
  the number or omitted when `None`).
- `fn format_json(diags) -> String` (serialize the array; `serde_json` is already a
  core dev-dep, and the server already depends on serde via the workspace, so use a
  small manual JSON writer or `serde_json` if it is a server runtime dep, else hand-
  roll the array to avoid a new runtime dep). Decision: hand-roll a tiny JSON
  serializer for the three string/number fields to avoid adding a runtime dep; it is
  ~15 lines and escapes `"`, `\`, and control chars.

## Tests

In `crates/server/src/main.rs`'s `#[cfg(test)] mod tests` (where `mount_warnings`
is already tested), using a `TempProj`-style temp dir:

1. **File diagnostics**: a doc with a typo'd front-matter key (`titel:`) and a
   dangling `@fig-nope` → `collect_diagnostics` returns both, each with the file set
   and the typo carrying a line.
2. **Clean file**: a plain doc → empty.
3. **Site path**: a tiny `_site.yml` + two pages, one tripping a diagnostic →
   collected with the page's relative path; empty site dir → `Err`.
4. **JSON format**: `format_json` of a known diagnostic list parses back to the
   expected array (assert the exact string for one simple case, including a `null`
   line and an escaped quote in a message).
5. **Exit mapping**: empty → success, non-empty → failure (test the small mapping
   helper, or assert via the formatter + `is_empty`).
6. **Corpus integration** (optional, in a `crates/core`-adjacent or server test):
   `collect_diagnostics(corpus/diagnostics)` surfaces the deliberate schema + prose
   warnings (non-empty), and `collect_diagnostics` on a clean corpus doc is empty.

## Docs

- `usage()` gains a `check` line.
- `docs/guide/reference/cli.qmd` gains a `check` entry with a one-line CI snippet
  (e.g. `qmd-fast check . || exit 1` in a workflow step).

## Risks & mitigations

- **`render_page_doc_warned` expecting executed blocks**: it does cross-page xref
  resolution + `finish_blocks`, neither of which needs execution; passing the
  un-executed doc yields the static warnings correctly (the build path executes only
  to bake figure outputs, which `check` does not need).
- **Site config warnings have no location** (`Vec<String>`): reported with
  `line: None` and a `_site.yml` file label; acceptable (they are project-level).
- **JSON dep**: hand-rolled serializer avoids adding a server runtime dependency;
  unit-tested for escaping.
- **Exit-code conflation** (diagnostics vs usage error both exit 1): intentional, any
  non-zero fails CI; the message distinguishes them for a human.

## Out of scope follow-ups (recorded, not built)

- `--format sarif` (SARIF 2.1.0) for GitHub Code Scanning; needs a `rule`/ruleId,
  i.e. a `Warning` category field.
- `--max-warnings N`, severity levels, `--quiet`.
- Executing cells under a `--exec` flag to also report runtime cell errors.
- A11y output checks (the client-side audit) folded into `check`.
