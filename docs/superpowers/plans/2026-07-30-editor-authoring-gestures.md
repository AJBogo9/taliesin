# Editor authoring gestures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the VS Code companion the three authoring gestures that are felt every day: paste
and drop (images, tables, URLs, BibTeX, CSV), rename-repairs-references in both directions, and
clickable `file:line:` locations in the dev-server log.

**Architecture:** Two new custom LSP requests (`taliesin/insertEdit`, `taliesin/renameFileEdits`)
compute every piece of `.tmd` knowledge in Rust; three new thin TypeScript modules own only the
VS Code gestures (clipboard bytes, `WorkspaceEdit` application, terminal-line matching). This
mirrors `taliesin/sectionEdit`, which exists for exactly this reason: a scan written in TypeScript
is a second copy of the vocabulary.

**Tech Stack:** Rust (edition 2024, `lsp-server` + `lsp-types`, `serde`), TypeScript
(`vscode-languageclient` over `taliesin lsp`), Mocha for both the unit and Extension Host suites.

**Spec:** [2026-07-30-editor-authoring-gestures-design.md](../specs/2026-07-30-editor-authoring-gestures-design.md)

## Global Constraints

Every task's requirements implicitly include this section.

- **`engines.vscode` must be `^1.97.0`.** Measured, not recalled: `registerDocumentPasteEditProvider`
  and `DocumentDropOrPasteEditKind` are absent from stable `@types/vscode@1.96.0` and present in
  `1.97.0`. The tree currently declares `^1.91.0` while resolving types at 1.125.0.
- **No em dashes or en dashes** in any prose this plan writes (`—`, `–`). Use commas, colons,
  parentheses, or a restructured sentence.
- **A new `taliesin/…` method needs a row in `docs/internals/extending.tmd` in the same commit.**
  Gate: `lsp::tests::the_internals_book_documents_every_taliesin_namespaced_method`
  (`crates/server/src/lsp.rs:4073`). It greps this module's own source for `"taliesin/…"` literals
  and requires each to appear in that file as `` `taliesin/name` ``.
- **A method name must be alphanumeric after the slash.** The census filter is
  `name.chars().all(char::is_alphanumeric)`, so `insertEdit` and `renameFileEdits` are fine and
  anything hyphenated would be silently skipped by the gate.
- **TypeScript never parses `.tmd`. Rust never touches the clipboard.**
- **Rename tests copy `corpus/tarn` into a temp directory and never mutate the corpus.** The corpus
  walker renders every corpus doc on every `cargo test`; an in-place edit poisons every later
  assertion.
- **`_site.yml` is edited as text, never re-serialized.** A YAML round-trip reformats the author's
  file and drops comments.
- **Verify each fix by mutation:** restore the bug, watch the named test fail. A green suite is not
  evidence.
- **stdout is the JSON-RPC wire in `taliesin lsp`.** Never `println!`; use `crate::log` (stderr).
- A `PostToolUse` hook runs `rustfmt` on every edited `.rs` file, so do not hand-format Rust.
- Final gate for the branch: `./tools/gates.sh`, plus
  `cd editor/vscode && npm run compile && npm test && npm run test:e2e`.

## File Structure

**Rust (`crates/server/src/`)**

| File | Responsibility |
|---|---|
| `lsp_insert.rs` **(new)** | `InsertEditParams` / `InsertEditResult` and the five insert kinds. Pure except for one directory read (the image counter) and one file read (the `.bib` duplicate check). |
| `lsp_rename_file.rs` **(new)** | `RenameFileEditsParams`, the inbound project walk and the outbound rebase. |
| `lsp.rs` **(modify)** | Two method constants, two dispatch arms, two census rows. |

**TypeScript (`editor/vscode/src/`)**

| File | Responsibility |
|---|---|
| `insert.ts` **(new)** | The paste and drop providers. Clipboard and file I/O only; every emitted string comes from `taliesin/insertEdit`. |
| `rename.ts` **(new)** | The `onWillRenameFiles` hook and `WorkspaceEdit` assembly. |
| `termlinks.ts` **(new)** | The terminal link provider. The only module here with knowledge of its own, which is why it gets a drift gate. |
| `extension.ts` **(modify)** | Register the three. |
| `package.json` **(modify)** | `engines.vscode` floor. |

**Tests**

| File | Responsibility |
|---|---|
| Inline `#[cfg(test)] mod tests` in each new Rust module | Unit coverage, following the convention in `lsp_edits.rs`. |
| `crates/server/tests/terminal_link_pattern.rs` **(new)** | The TS-regex-versus-Rust-format drift gate. Precedent: `release_targets.rs`, `site_build_script.rs`. |
| `editor/vscode/src/test/manifest.test.ts` **(modify)** | The engines floor gate. |
| `editor/vscode/src/e2e/suite/integration.test.ts` **(modify)** | Extension Host coverage for the gestures. |

**Docs**

| File | Responsibility |
|---|---|
| `docs/internals/extending.tmd` **(modify)** | One table row per new method (gated). |
| `docs/guide/using/writing.tmd` **(modify)** | The author-facing description of the gestures. |
| `notes/DETECTION-DEBT.md` **(modify)** | One row for the image-paste coverage gap named in Task 8. |
| `notes/backlog.md`, `notes/FEATURE-IDEAS.md` **(modify)** | Record the batch, delete nothing that is not shipped. |

---

### Task 1: Raise the VS Code API floor to 1.97

The prerequisite for every paste gesture, and it closes a latent inconsistency: the manifest
declares an engine older than the types it compiles against, so TypeScript happily accepts calls
to APIs that would be `undefined` at the declared minimum.

**Files:**
- Modify: `editor/vscode/package.json`
- Test: `editor/vscode/src/test/manifest.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing in code. Later tasks rely on `vscode.languages.registerDocumentPasteEditProvider`
  and `vscode.DocumentDropOrPasteEditKind` being legal to call.

- [ ] **Step 1: Write the failing test**

Append to `editor/vscode/src/test/manifest.test.ts`, inside the existing suite:

```ts
test("the declared engine is new enough for the APIs we call, and matches the types", () => {
  // `registerDocumentPasteEditProvider` and `DocumentDropOrPasteEditKind` are absent from
  // stable @types/vscode 1.96.0 and present in 1.97.0, so 1.97 is the real floor for the
  // paste gestures. Asserting the two agree is the point: the tree previously declared
  // ^1.91.0 while resolving types at 1.125.0, which let TypeScript bless a call that would
  // be `undefined` on the minimum VS Code we claim to support.
  const engine = pkg.engines.vscode as string;
  const types = pkg.devDependencies["@types/vscode"] as string;
  assert.strictEqual(engine, "^1.97.0", "engines.vscode must state the paste API floor");
  assert.strictEqual(types, engine, "@types/vscode must not be ahead of engines.vscode");
});
```

If `pkg` is not already in scope in that file, read the manifest the same way the neighbouring
tests do rather than inventing a second loader.

- [ ] **Step 2: Run it and watch it fail**

```bash
cd editor/vscode && npm run compile && npm test -- --grep "declared engine"
```

Expected: FAIL, `'^1.91.0' !== '^1.97.0'`.

- [ ] **Step 3: Bump both fields**

In `editor/vscode/package.json`: set `engines.vscode` to `^1.97.0` and
`devDependencies["@types/vscode"]` to `^1.97.0`.

- [ ] **Step 4: Reinstall types and confirm green**

```bash
cd editor/vscode && npm install && npm run compile && npm test -- --grep "declared engine"
```

Expected: PASS. `npm run compile` must stay clean: pinning the types to 1.97 removes API surface
the code may have been using unknowingly, and a new error here is a real finding, not noise.

- [ ] **Step 5: Mutation-verify**

Set `engines.vscode` back to `^1.91.0`, re-run, confirm RED, restore `^1.97.0`.

- [ ] **Step 6: Commit**

```bash
git add editor/vscode/package.json editor/vscode/package-lock.json editor/vscode/src/test/manifest.test.ts
git commit -m "chore(companion): declare the real VS Code API floor (1.97)

registerDocumentPasteEditProvider is absent from stable @types/vscode
1.96.0 and present in 1.97.0. The manifest declared ^1.91.0 while
resolving types at 1.125.0, so TypeScript accepted calls that would be
undefined on the minimum engine we claim. The new test pins the two
fields equal so they cannot drift apart again."
```

---

### Task 2: `taliesin/insertEdit` plus the image kind

**Files:**
- Create: `crates/server/src/lsp_insert.rs`
- Modify: `crates/server/src/lsp.rs` (module declaration, method constant, dispatch arm)
- Modify: `docs/internals/extending.tmd` (gated row)

**Interfaces:**
- Consumes: nothing.
- Produces, and later tasks depend on these exact names:
  - `pub(crate) struct InsertEditParams { text_document: lsp_types::TextDocumentIdentifier, kind: InsertKind, payload: String }`, `#[serde(rename_all = "camelCase")]`
  - `pub(crate) enum InsertKind { Image, HtmlTable, TsvTable, Bibtex, Dataset }`, `#[serde(rename_all = "camelCase")]`
  - `pub(crate) struct InsertEditResult { text: String, is_snippet: bool, write_file: Option<String>, append: Option<AppendEdit> }`, `#[serde(rename_all = "camelCase")]`
  - `pub(crate) struct AppendEdit { path: String, text: String }`, `#[serde(rename_all = "camelCase")]`
  - `pub(crate) fn insert_edit(doc: &std::path::Path, text: &str, params: &InsertEditParams) -> Result<InsertEditResult, String>`
  - `pub(crate) const INSERT_EDIT_METHOD: &str = "taliesin/insertEdit";` (in `lsp.rs`)

There is no `position` field. The client inserts the returned text where the gesture happened, so
the server never needs the cursor. `payload` carries, per kind: the clipboard mime type (`Image`),
the pasted text (`HtmlTable`, `TsvTable`, `Bibtex`), the dropped file's absolute path (`Dataset`).

- [ ] **Step 1: Write the failing tests**

Create `crates/server/src/lsp_insert.rs` with only the test module and the type declarations it
needs to compile, leaving `insert_edit` as `todo!()`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn params(kind: InsertKind, payload: &str) -> InsertEditParams {
        InsertEditParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: lsp_types::Url::parse("file:///tmp/x/bayes.tmd").unwrap(),
            },
            kind,
            payload: payload.to_owned(),
        }
    }

    #[test]
    fn an_image_paste_names_the_file_from_the_document_stem() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("bayes.tmd");
        std::fs::write(&doc, "# Bayes\n").unwrap();

        let r = insert_edit(&doc, "# Bayes\n", &params(InsertKind::Image, "image/png")).unwrap();

        assert_eq!(r.write_file.as_deref(), Some("bayes-01.png"));
        assert!(r.is_snippet, "the caption and label are tab stops");
        assert_eq!(
            r.text,
            "![${1:caption}](bayes-01.png){#fig-${2:label}}",
            "the canonical figure shape, beside the doc, with both tab stops"
        );
    }

    #[test]
    fn the_counter_skips_names_already_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();
        // Not 01 and not 02: a gap must not be reused, or two pastes in one session can
        // collide after the author deletes the first file.
        std::fs::write(dir.path().join("bayes-01.png"), "").unwrap();
        std::fs::write(dir.path().join("bayes-04.jpg"), "").unwrap();

        let r = insert_edit(&doc, "", &params(InsertKind::Image, "image/png")).unwrap();
        assert_eq!(r.write_file.as_deref(), Some("bayes-05.png"));
    }

    #[test]
    fn a_document_stem_that_is_not_a_safe_filename_is_slugged() {
        let dir = tempfile::tempdir().unwrap();
        // Spaces and parentheses in a filename force every later reference to escape them,
        // and `$` would be read as a snippet placeholder in the text we return.
        let doc = dir.path().join("Chapter 1 (draft).tmd");
        std::fs::write(&doc, "").unwrap();

        let r = insert_edit(&doc, "", &params(InsertKind::Image, "image/svg+xml")).unwrap();
        assert_eq!(r.write_file.as_deref(), Some("chapter-1-draft-01.svg"));
    }

    #[test]
    fn a_stem_that_slugs_to_nothing_falls_back_to_figure() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("___.tmd");
        std::fs::write(&doc, "").unwrap();

        let r = insert_edit(&doc, "", &params(InsertKind::Image, "image/png")).unwrap();
        assert_eq!(r.write_file.as_deref(), Some("figure-01.png"));
    }

    #[test]
    fn an_unknown_clipboard_mime_is_refused_rather_than_guessed() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("bayes.tmd");
        std::fs::write(&doc, "").unwrap();

        let err = insert_edit(&doc, "", &params(InsertKind::Image, "image/tiff")).unwrap_err();
        assert!(err.contains("image/tiff"), "the refusal names the mime: {err}");
    }
}
```

Check whether `tempfile` is already a dev-dependency of `crates/server`. If it is not, use the
same temp-directory helper the neighbouring tests use rather than adding a dependency.

- [ ] **Step 2: Run and watch it fail**

```bash
cargo test -p taliesin-server --bin taliesin lsp_insert 2>&1 | tail -20
```

Expected: FAIL. Either a compile error for the missing `insert_edit`, or a `todo!()` panic.

- [ ] **Step 3: Implement the types, the slug, the counter and the image kind**

```rust
//! `taliesin/insertEdit`: the text a companion gesture inserts.
//!
//! The gestures themselves (paste, drop) are VS Code APIs and live in TypeScript, but every
//! string they insert is `.tmd` vocabulary and is computed here, for the same reason
//! `lsp_edits::section_edit` exists: a figure shape, a pipe table or a citation key written in
//! the client is a second copy of knowledge this crate already owns.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InsertEditParams {
    pub(crate) text_document: lsp_types::TextDocumentIdentifier,
    pub(crate) kind: InsertKind,
    /// Per kind: the clipboard mime type for [`InsertKind::Image`], the pasted text for
    /// [`InsertKind::HtmlTable`] / [`InsertKind::TsvTable`] / [`InsertKind::Bibtex`], and the
    /// dropped file's absolute path for [`InsertKind::Dataset`].
    pub(crate) payload: String,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum InsertKind {
    Image,
    HtmlTable,
    TsvTable,
    Bibtex,
    Dataset,
}

#[derive(Debug, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InsertEditResult {
    /// What to insert at the gesture's position.
    pub(crate) text: String,
    /// `true` when `text` is a snippet with `${n:…}` tab stops rather than literal text.
    pub(crate) is_snippet: bool,
    /// A file the client must write before applying `text`, relative to the document's
    /// directory. Only the image paste sets it: the clipboard bytes never reach this crate.
    pub(crate) write_file: Option<String>,
    /// An append to a second file (the `.bib`), so the client can carry it as one undo.
    pub(crate) append: Option<AppendEdit>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppendEdit {
    pub(crate) path: String,
    pub(crate) text: String,
}

/// The extension a clipboard image is saved under. Refused rather than guessed: an unknown
/// mime saved as `.png` produces a file whose bytes contradict its name, which every later
/// tool reads as corruption.
fn image_extension(mime: &str) -> Result<&'static str, String> {
    match mime {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/svg+xml" => Ok("svg"),
        "image/webp" => Ok("webp"),
        "image/gif" => Ok("gif"),
        other => Err(format!("cannot paste {other}: unsupported image type")),
    }
}

/// A document stem reduced to a filename that needs no escaping anywhere it is later written:
/// in Markdown, in a shell, and in the snippet text this module returns (where a bare `$`
/// would be read as a placeholder).
fn slug(stem: &str) -> String {
    let mut out = String::new();
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_owned();
    if trimmed.is_empty() {
        "figure".to_owned()
    } else {
        trimmed
    }
}

/// The next free `<slug>-NN` in `dir`, ignoring the extension: a gap is never reused, so two
/// pastes in one session cannot collide after the author deletes the first file.
fn next_index(dir: &Path, slug: &str) -> u32 {
    let prefix = format!("{slug}-");
    let mut max = 0u32;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(rest) = name.strip_prefix(&prefix) else {
                continue;
            };
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(n) = digits.parse::<u32>() {
                max = max.max(n);
            }
        }
    }
    max + 1
}

pub(crate) fn insert_edit(
    doc: &Path,
    text: &str,
    params: &InsertEditParams,
) -> Result<InsertEditResult, String> {
    match params.kind {
        InsertKind::Image => image_paste(doc, &params.payload),
        // Filled in by later tasks.
        _ => Err("unsupported insert kind".to_owned()),
    }
}

fn image_paste(doc: &Path, mime: &str) -> Result<InsertEditResult, String> {
    let ext = image_extension(mime)?;
    let dir = doc.parent().ok_or("the document has no directory")?;
    let stem = doc.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let slug = slug(stem);
    let name = format!("{slug}-{:02}.{ext}", next_index(dir, &slug));
    Ok(InsertEditResult {
        // The caption and the label are the two things only the author can write, so both are
        // tab stops. `name` is slug-safe, so it cannot contain a `$` the snippet would read.
        text: format!("![${{1:caption}}]({name}){{#fig-${{2:label}}}}"),
        is_snippet: true,
        write_file: Some(name),
        append: None,
    })
}
```

Note the `text` parameter is unused in this task and is consumed by Tasks 4, 5 and 6. Silence the
warning with a leading underscore only if clippy demands it; do not remove the parameter.

- [ ] **Step 4: Run and confirm green**

```bash
cargo test -p taliesin-server --bin taliesin lsp_insert 2>&1 | tail -20
```

Expected: PASS, 5 tests.

- [ ] **Step 5: Wire the dispatch arm and the module**

In `crates/server/src/lsp.rs`, add `mod lsp_insert;` beside the other `lsp_*` module
declarations (check whether they are declared in `main.rs` instead and follow that), then the
constant beside `SECTION_EDIT_METHOD` around line 820:

```rust
/// The custom request behind the companion's paste and drop gestures. Namespaced for the same
/// reason as [`SECTION_EDIT_METHOD`]: it is not an LSP method. The gesture is a VS Code API and
/// has to live in the client, but the *text* it inserts is `.tmd` vocabulary and belongs here.
pub(crate) const INSERT_EDIT_METHOD: &str = "taliesin/insertEdit";
```

And the dispatch arm, immediately after the `SECTION_EDIT_METHOD` branch, following its shape
exactly (a refusal is a `RequestFailed` error whose message the companion shows):

```rust
} else if req.method == INSERT_EDIT_METHOD {
    let params: crate::lsp_insert::InsertEditParams = serde_json::from_value(req.params)?;
    let path = params
        .text_document
        .uri
        .to_file_path()
        .map_err(|()| "the document is not a file".to_owned());
    let text = docs.get(&params.text_document.uri).cloned().unwrap_or_default();
    match path.and_then(|p| crate::lsp_insert::insert_edit(&p, &text, &params)) {
        Ok(result) => lsp_server::Response {
            id: req.id,
            result: Some(serde_json::to_value(result)?),
            error: None,
        },
        // A refusal is a first-class answer, exactly as for `sectionEdit`: "this mime is not
        // an image we can save" is information the author should see, and `null` would read
        // as "nothing happened".
        Err(message) => lsp_server::Response {
            id: req.id,
            result: None,
            error: Some(lsp_server::ResponseError {
                code: -32803, // JSON-RPC RequestFailed
                message,
                data: None,
            }),
        },
    }
```

- [ ] **Step 6: Watch the docs gate fail, then satisfy it**

```bash
cargo test -p taliesin-server --bin taliesin the_internals_book_documents_every 2>&1 | tail -10
```

Expected: FAIL, "`taliesin/insertEdit` is served but docs/internals/extending.tmd never mentions
it". This is the gate proving itself; do not skip watching it go red.

Add a row to the method table in `docs/internals/extending.tmd` (beside the `taliesin/cellRegions`
and `taliesin/sectionEdit` rows around line 143), matching their voice and using no em dashes:

```
| `taliesin/insertEdit` | the text a paste or drop gesture inserts: a figure block for a pasted image, a pipe table for a pasted spreadsheet, `[@key]` plus a `.bib` append for a pasted BibTeX entry, a `{{< dataset >}}` card plus a loader cell for a dropped `.csv`. The gesture has to live in the client (it is a VS Code API), but the inserted text is `.tmd` vocabulary, so only the client's half is in TypeScript |
```

- [ ] **Step 7: Confirm green**

```bash
cargo test -p taliesin-server --bin taliesin lsp 2>&1 | tail -5
```

Expected: PASS, including the census gate.

- [ ] **Step 8: Commit**

```bash
git add crates/server/src/lsp_insert.rs crates/server/src/lsp.rs docs/internals/extending.tmd
git commit -m "feat(lsp): taliesin/insertEdit, with the image-paste kind

The first of two custom requests behind the companion's authoring
gestures. A pasted image is named from the slugged document stem with a
gap-skipping counter and written beside the doc, which is the corpus
convention (24 image refs beside the doc against 7 in a subdirectory),
and the inserted text is a snippet so the caption and the #fig- label
are tab stops.

An unknown clipboard mime is refused rather than saved as .png: a file
whose bytes contradict its extension reads as corruption downstream."
```

---

### Task 3: The table kinds

**Files:**
- Modify: `crates/server/src/lsp_insert.rs`

**Interfaces:**
- Consumes: `InsertEditResult`, `insert_edit` (Task 2).
- Produces: `InsertKind::HtmlTable` and `InsertKind::TsvTable` handled; no new public names.

Alignment is deliberately **all default** (`---` in every delimiter cell). `format_tables`
re-derives alignment from the delimiter row, and reading `align=` out of clipboard HTML is
speculative work for a shape no corpus document has.

- [ ] **Step 1: Write the failing tests**

Add to the test module in `crates/server/src/lsp_insert.rs`:

```rust
#[test]
fn a_pasted_tsv_grid_becomes_an_aligned_pipe_table() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    std::fs::write(&doc, "").unwrap();

    let tsv = "site\tdepth\ttemp\nnorth\t3\t7.1\nsouth\t12\t4.4\n";
    let r = insert_edit(&doc, "", &params(InsertKind::TsvTable, tsv)).unwrap();

    assert!(!r.is_snippet, "a table has nothing for the author to fill in");
    assert_eq!(
        r.text,
        "| site  | depth | temp |\n\
         | ----- | ----- | ---- |\n\
         | north | 3     | 7.1  |\n\
         | south | 12    | 4.4  |",
        "columns padded by lsp_format::format_tables"
    );
}

#[test]
fn a_cell_containing_a_pipe_is_escaped() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    std::fs::write(&doc, "").unwrap();

    // An unescaped pipe splits the cell, silently turning a 2-column row into 3. This is a
    // trap already recorded in LESSONS.md, which is why it gets its own test.
    let tsv = "expr\tmeaning\na|b\tunion\n";
    let r = insert_edit(&doc, "", &params(InsertKind::TsvTable, tsv)).unwrap();

    assert!(r.text.contains(r"a\|b"), "the pipe is escaped: {}", r.text);
    for line in r.text.lines() {
        let cells = line.replace(r"\|", "").matches('|').count();
        assert_eq!(cells, 3, "every row keeps 2 columns: {line}");
    }
}

#[test]
fn a_pasted_html_table_reads_th_td_and_decodes_entities() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    std::fs::write(&doc, "").unwrap();

    // The shape a spreadsheet actually puts on the clipboard: attributes on every tag,
    // inline markup inside cells, and &nbsp; for a blank cell.
    let html = "<table><tr><th align=\"left\">site</th><th>n</th></tr>\
                <tr><td style=\"x\"><b>north</b></td><td>&nbsp;</td></tr></table>";
    let r = insert_edit(&doc, "", &params(InsertKind::HtmlTable, html)).unwrap();

    let lines: Vec<&str> = r.text.lines().collect();
    assert_eq!(lines.len(), 3, "header, delimiter, one body row: {:?}", lines);
    assert!(lines[0].contains("site") && lines[0].contains('n'));
    assert!(lines[2].contains("north"), "cell markup stripped: {}", lines[2]);
    assert!(!r.text.contains("&nbsp;"), "entities decoded: {}", r.text);
}

#[test]
fn a_ragged_grid_is_refused_rather_than_silently_squared() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    std::fs::write(&doc, "").unwrap();

    // Tab-separated prose is not a table. Padding it to a rectangle invents cells the author
    // never wrote, so refuse and let the plain paste win.
    let err = insert_edit(&doc, "", &params(InsertKind::TsvTable, "a\tb\nc\n")).unwrap_err();
    assert!(err.contains("not a table"), "{err}");
}

#[test]
fn a_single_column_is_not_a_table() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    std::fs::write(&doc, "").unwrap();

    let err = insert_edit(&doc, "", &params(InsertKind::TsvTable, "alpha\nbeta\n")).unwrap_err();
    assert!(err.contains("not a table"), "{err}");
}
```

- [ ] **Step 2: Run and watch them fail**

```bash
cargo test -p taliesin-server --bin taliesin lsp_insert 2>&1 | tail -20
```

Expected: FAIL with "unsupported insert kind".

- [ ] **Step 3: Implement**

Add to `lsp_insert.rs`, and extend the `match` in `insert_edit` to route `HtmlTable` and
`TsvTable` to `table_paste`:

```rust
/// Rows of cells from tab-separated clipboard text.
fn tsv_rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').map(|c| c.trim().to_owned()).collect())
        .collect()
}

/// Rows of cells from clipboard HTML. A tolerant scanner rather than a parser: the input is
/// one `<table>` from a spreadsheet or a browser, not arbitrary documents, and the project
/// carries no HTML parser to reach for.
fn html_rows(html: &str) -> Vec<Vec<String>> {
    let lower = html.to_ascii_lowercase();
    let mut rows = Vec::new();
    let mut at = 0usize;
    while let Some(tr) = lower[at..].find("<tr").map(|i| at + i) {
        let end = lower[tr..]
            .find("</tr")
            .map(|i| tr + i)
            .unwrap_or(lower.len());
        let mut cells = Vec::new();
        let mut cell_at = tr;
        while let Some(open) = ["<td", "<th"]
            .iter()
            .filter_map(|t| lower[cell_at..end].find(t).map(|i| cell_at + i))
            .min()
        {
            // Skip past the tag's own attributes to the content.
            let Some(gt) = lower[open..end].find('>').map(|i| open + i + 1) else {
                break;
            };
            let close = ["</td", "</th"]
                .iter()
                .filter_map(|t| lower[gt..end].find(t).map(|i| gt + i))
                .min()
                .unwrap_or(end);
            cells.push(strip_tags(&html[gt..close]));
            cell_at = close + 1;
        }
        if !cells.is_empty() {
            rows.push(cells);
        }
        at = end + 1;
    }
    rows
}

/// Cell text with inline markup removed and the entities a spreadsheet emits decoded. `&amp;`
/// is decoded last, or `&amp;nbsp;` would turn into a space.
fn strip_tags(cell: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for ch in cell.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_owned()
}

fn table_paste(kind: InsertKind, payload: &str) -> Result<InsertEditResult, String> {
    let rows = match kind {
        InsertKind::HtmlTable => html_rows(payload),
        _ => tsv_rows(payload),
    };
    let width = rows.first().map(Vec::len).unwrap_or(0);
    // Two rows and two columns is the floor, and every row must agree on the column count.
    // Padding a ragged grid to a rectangle would invent cells the author never wrote.
    if rows.len() < 2 || width < 2 || rows.iter().any(|r| r.len() != width) {
        return Err("that does not look like a table (not a rectangular grid)".to_owned());
    }
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        let cells: Vec<String> = row.iter().map(|c| c.replace('|', r"\|")).collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
        if i == 0 {
            let delims: Vec<&str> = (0..width).map(|_| "---").collect();
            out.push_str(&format!("| {} |\n", delims.join(" | ")));
        }
    }
    // One aligner, shared with the format-document path, so a pasted table and a formatted
    // one are byte-identical.
    let aligned = apply_line_edits(&out, &crate::lsp_format::format_tables(&out));
    Ok(InsertEditResult {
        text: aligned.trim_end().to_owned(),
        is_snippet: false,
        write_file: None,
        append: None,
    })
}
```

`apply_line_edits` is the helper that turns `lsp_format::LineEdit`s into a new string. Look for an
existing one in `lsp_format.rs` or its tests and reuse it; only if there is none, write a private
one here and say so in a comment.

- [ ] **Step 4: Run and confirm green**

```bash
cargo test -p taliesin-server --bin taliesin lsp_insert 2>&1 | tail -20
```

Expected: PASS, 10 tests. If the padding in the first test does not match, print the actual
output and correct the *expected string* to what `format_tables` produces: the aligner is the
authority, not this plan's arithmetic.

- [ ] **Step 5: Mutation-verify the pipe escape**

Remove `.replace('|', r"\|")`, re-run, confirm `a_cell_containing_a_pipe_is_escaped` fails, restore.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/lsp_insert.rs
git commit -m "feat(lsp): insertEdit builds a pipe table from a pasted grid

HTML from a spreadsheet or TSV from plain text, aligned by the existing
lsp_format::format_tables so a pasted table and a formatted one are
byte-identical. Cells containing a pipe are escaped, which LESSONS.md
records as a live trap, and a ragged or single-column grid is refused
rather than squared: padding would invent cells the author never wrote."
```

---

### Task 4: The BibTeX kind

**Files:**
- Modify: `crates/server/src/lsp_insert.rs`

**Interfaces:**
- Consumes: `InsertEditResult`, `AppendEdit`, `insert_edit` (Task 2).
- Produces: `InsertKind::Bibtex` handled.

- [ ] **Step 1: Locate the existing bibliography resolution, do not invent it**

Before writing code, find how the render path resolves `bibliography:` from front matter and how
item 163's site-level merge works:

```bash
rg -n "bibliography" crates/core/src/site/bibliography.rs crates/core/src/frontmatter.rs | head -20
rg -n "SiteDefaults" crates/core/src --type rust | head -10
```

Reuse what you find. Write a local reader only if there is genuinely no accessor, and if you do,
say so in a comment naming what you looked for.

- [ ] **Step 2: Write the failing tests**

```rust
#[test]
fn a_pasted_bibtex_entry_appends_to_the_bib_and_inserts_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    let text = "---\nbibliography: refs.bib\n---\n\nBody.\n";
    std::fs::write(&doc, text).unwrap();
    std::fs::write(dir.path().join("refs.bib"), "@book{knuth1984,\n  title = {TeX},\n}\n").unwrap();

    let entry = "@article{bishop2006,\n  title = {Pattern Recognition},\n  year = {2006},\n}";
    let r = insert_edit(&doc, text, &params(InsertKind::Bibtex, entry)).unwrap();

    assert_eq!(r.text, "[@bishop2006]");
    assert!(!r.is_snippet);
    let append = r.append.expect("the entry is appended to the .bib");
    assert!(append.path.ends_with("refs.bib"), "{}", append.path);
    assert!(append.text.contains("bishop2006"));
}

#[test]
fn an_entry_already_in_the_bib_is_cited_but_not_appended_twice() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    let text = "---\nbibliography: refs.bib\n---\n\nBody.\n";
    std::fs::write(&doc, text).unwrap();
    std::fs::write(dir.path().join("refs.bib"), "@book{bishop2006,\n  title = {PR},\n}\n").unwrap();

    let entry = "@article{bishop2006,\n  title = {Pattern Recognition},\n}";
    let r = insert_edit(&doc, text, &params(InsertKind::Bibtex, entry)).unwrap();

    // parse_bib_warned lints duplicate keys, so appending one would make this gesture trip
    // the author's own diagnostic.
    assert_eq!(r.text, "[@bishop2006]");
    assert_eq!(r.append, None, "no second copy of the key");
}

#[test]
fn a_document_with_no_bibliography_still_gets_the_citation() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    let text = "# Bayes\n\nBody.\n";
    std::fs::write(&doc, text).unwrap();

    let r = insert_edit(&doc, text, &params(InsertKind::Bibtex, "@book{ab2020, title={X}}")).unwrap();

    // Creating the .bib, editing front matter and pasting is three coupled writes for the
    // least common case. `citations_without_bibliography` already reports the gap.
    assert_eq!(r.text, "[@ab2020]");
    assert_eq!(r.append, None);
}

#[test]
fn text_that_is_not_a_bibtex_entry_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    std::fs::write(&doc, "").unwrap();

    let err = insert_edit(&doc, "", &params(InsertKind::Bibtex, "@ not an entry")).unwrap_err();
    assert!(err.contains("BibTeX"), "{err}");
}
```

- [ ] **Step 3: Run and watch them fail**

```bash
cargo test -p taliesin-server --bin taliesin lsp_insert 2>&1 | tail -20
```

Expected: FAIL with "unsupported insert kind".

- [ ] **Step 4: Implement**

Route `InsertKind::Bibtex` to a `bibtex_paste(doc, text, payload)` that:

1. extracts the key with a scan for `@`, an identifier, `{`, then the key up to `,` or whitespace,
   returning `Err("that does not look like a BibTeX entry")` when the shape does not hold;
2. resolves the target `.bib` using what Step 1 found (front matter first, then the site-level
   key), returning `None` when there is none;
3. reads that `.bib` and returns `append: None` when the key is already present;
4. returns `text: format!("[@{key}]")`, `is_snippet: false`, and an `AppendEdit` whose `text`
   begins with a newline when the existing file does not end in one, so the append cannot glue
   two entries onto one line.

- [ ] **Step 5: Run and confirm green**

```bash
cargo test -p taliesin-server --bin taliesin lsp_insert 2>&1 | tail -20
```

Expected: PASS, 14 tests.

- [ ] **Step 6: Mutation-verify the duplicate guard**

Make the duplicate check always report "absent", re-run, confirm
`an_entry_already_in_the_bib_is_cited_but_not_appended_twice` fails, restore.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/lsp_insert.rs
git commit -m "feat(lsp): insertEdit appends a pasted BibTeX entry and cites it

The append travels as a separate edit so the client can apply it in the
same undo as the paste. A key already in the .bib is cited and not
appended: parse_bib_warned lints duplicate keys, so a second copy would
make the gesture trip the author's own diagnostic. A document with no
bibliography gets the citation and nothing else, leaving the existing
citations_without_bibliography diagnostic to report the gap."
```

---

### Task 5: The dataset kind

**Files:**
- Modify: `crates/server/src/lsp_insert.rs`

**Interfaces:**
- Consumes: `InsertEditResult`, `insert_edit` (Task 2), `crate::lsp_cells::cell_regions`.
- Produces: `InsertKind::Dataset` handled.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_dropped_csv_gets_a_dataset_card_and_a_loader_cell() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    let text = "# Bayes\n";
    std::fs::write(&doc, text).unwrap();
    std::fs::create_dir_all(dir.path().join("data")).unwrap();
    let csv = dir.path().join("data/measurements.csv");
    std::fs::write(&csv, "a,b\n1,2\n").unwrap();

    let r = insert_edit(&doc, text, &params(InsertKind::Dataset, csv.to_str().unwrap())).unwrap();

    assert!(
        r.text.contains("{{< dataset data/measurements.csv >}}"),
        "the card, with a doc-relative path: {}",
        r.text
    );
    // No cells at all, so the loader defaults to python.
    assert!(r.text.contains("```{python}"), "{}", r.text);
    assert!(r.text.contains("import pandas as pd"), "{}", r.text);
    assert!(r.text.contains(r#"pd.read_csv("data/measurements.csv")"#), "{}", r.text);
}

#[test]
fn the_loader_follows_the_documents_first_cell_language() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    // A bash fence first: not a cell language, so it must not decide the loader.
    let text = "```bash\nls\n```\n\n```{r}\nsummary(x)\n```\n";
    std::fs::write(&doc, text).unwrap();
    let csv = dir.path().join("m.csv");
    std::fs::write(&csv, "a\n1\n").unwrap();

    let r = insert_edit(&doc, text, &params(InsertKind::Dataset, csv.to_str().unwrap())).unwrap();

    assert!(r.text.contains("```{r}"), "{}", r.text);
    assert!(r.text.contains("library(readr)"), "{}", r.text);
    assert!(r.text.contains(r#"read_csv("m.csv")"#), "{}", r.text);
}

#[test]
fn an_import_the_document_already_has_is_not_repeated() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("bayes.tmd");
    let text = "```{python}\nimport pandas as pd\n```\n";
    std::fs::write(&doc, text).unwrap();
    let csv = dir.path().join("m.csv");
    std::fs::write(&csv, "a\n1\n").unwrap();

    let r = insert_edit(&doc, text, &params(InsertKind::Dataset, csv.to_str().unwrap())).unwrap();

    assert!(!r.text.contains("import pandas"), "already imported: {}", r.text);
    assert!(r.text.contains(r#"pd.read_csv("m.csv")"#), "{}", r.text);
}

#[test]
fn a_csv_outside_the_document_tree_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("proj/bayes.tmd");
    std::fs::create_dir_all(dir.path().join("proj")).unwrap();
    std::fs::write(&doc, "").unwrap();
    let csv = dir.path().join("elsewhere.csv");
    std::fs::write(&csv, "a\n1\n").unwrap();

    // A `../` dataset path is one the build cannot ship, so the editor must not emit it.
    let err = insert_edit(&doc, "", &params(InsertKind::Dataset, csv.to_str().unwrap()))
        .unwrap_err();
    assert!(err.contains("outside"), "{err}");
}
```

- [ ] **Step 2: Run and watch them fail**

```bash
cargo test -p taliesin-server --bin taliesin lsp_insert 2>&1 | tail -20
```

Expected: FAIL with "unsupported insert kind".

- [ ] **Step 3: Implement**

Route `InsertKind::Dataset` to `dataset_drop(doc, text, abs_path)` that:

1. computes the path relative to the document's directory, refusing with a message containing
   "outside" when that would need `../` (the build cannot ship such an asset);
2. picks the loader language as the **first** `cell_regions` entry whose `language` is one of
   `python`, `r`, `js`, defaulting to `python`. First rather than nearest, so the same document
   always gives the same answer no matter where the drop lands;
3. emits the card, a blank line, then a fenced cell:
   - `python`: `import pandas as pd` (only when `text` does not already contain `import pandas`)
     then `df = pd.read_csv("<rel>")`
   - `r`: `library(readr)` (only when `text` does not already contain `library(readr)`) then
     `df <- read_csv("<rel>")`
   - `js`: no import; `const df = await tali.csv("<rel>")` only if such a helper exists. Check
     first with `rg -n "csv" crates/core/assets/js/tali-js.js`; if there is none, refuse the `js`
     case with a message saying so rather than emitting a call to a function that does not exist.
4. returns `is_snippet: false`. There is no `datasets:` front-matter scaffold: `corpus/datasets.tmd`
   documents the block as optional (size and checksum are read off the file), and empty
   placeholders are noise a lint may flag.

- [ ] **Step 4: Run and confirm green**

```bash
cargo test -p taliesin-server --bin taliesin lsp_insert 2>&1 | tail -20
```

Expected: PASS, 18 tests.

- [ ] **Step 5: Mutation-verify the language choice**

Change "first cell language" to "last", re-run, confirm
`the_loader_follows_the_documents_first_cell_language` fails, restore.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/lsp_insert.rs
git commit -m "feat(lsp): insertEdit emits a dataset card and loader for a dropped CSV

The card plus a loader cell in the document's first cell language,
skipping an import the document already has. No datasets: front-matter
scaffold: corpus/datasets.tmd documents that block as optional, since
size and checksum are read off the file, and the keys it would add are
facts the author has and the editor does not.

A CSV outside the document tree is refused rather than emitted as a ../
path the build cannot ship."
```

---

### Task 6: The containment verdict for a dragged asset

**Files:**
- Modify: `crates/server/src/lsp_insert.rs`
- Modify: `crates/server/src/lsp.rs` (one more dispatch arm is **not** needed; extend `InsertKind`)

**Interfaces:**
- Consumes: `InsertEditResult`, `insert_edit` (Task 2).
- Produces: `InsertKind::Asset` added to the enum (serialized as `asset`), plus
  `InsertEditResult.outside: Option<String>` describing which containment rule the path breaks.

Add the field to `InsertEditResult` with `#[serde(skip_serializing_if = "Option::is_none")]` and
default `None`, so Tasks 2 to 5 need no edits.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_dragged_image_inside_the_tree_inserts_a_relative_figure() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("proj/media")).unwrap();
    let doc = dir.path().join("proj/bayes.tmd");
    std::fs::write(&doc, "").unwrap();
    std::fs::write(dir.path().join("proj/_site.yml"), "title: P\n").unwrap();
    let img = dir.path().join("proj/media/fit.png");
    std::fs::write(&img, "").unwrap();

    let r = insert_edit(&doc, "", &params(InsertKind::Asset, img.to_str().unwrap())).unwrap();

    assert_eq!(r.outside, None, "inside the project, nothing to warn about");
    assert!(r.is_snippet, "the caption and label are still the author's");
    assert!(r.text.contains("](media/fit.png)"), "{}", r.text);
}

#[test]
fn a_dragged_image_outside_the_doc_tree_says_so_and_still_offers_a_path() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("proj")).unwrap();
    let doc = dir.path().join("proj/bayes.tmd");
    std::fs::write(&doc, "").unwrap();
    std::fs::write(dir.path().join("proj/_site.yml"), "title: P\n").unwrap();
    let img = dir.path().join("outside/fit.png");
    std::fs::create_dir_all(img.parent().unwrap()).unwrap();
    std::fs::write(&img, "").unwrap();

    let r = insert_edit(&doc, "", &params(InsertKind::Asset, img.to_str().unwrap())).unwrap();

    // The verdict must come from the same rule copy_local_assets uses, or the editor blesses
    // a path the build then warns on, which is the bug class this gesture exists to prevent.
    let outside = r.outside.expect("an out-of-tree drag is reported");
    assert!(outside.contains("doc tree") || outside.contains("repository"), "{outside}");
    assert!(!r.text.is_empty(), "the client still has something to insert if the author insists");
}
```

- [ ] **Step 2: Run and watch them fail**

```bash
cargo test -p taliesin-server --bin taliesin lsp_insert 2>&1 | tail -20
```

Expected: FAIL, unknown variant `Asset` or "unsupported insert kind".

- [ ] **Step 3: Implement, reusing the build's own rule**

Read `copy_local_assets` at `crates/server/src/build.rs:715-740` first. It computes
`taliesin_core::includes::repo_boundary(base)` and then distinguishes two failures with two
different warnings: a path leaving the document tree, and a path resolving outside the
repository. Call the same functions and mirror both cases in the `outside` string. Do not write a
second containment rule.

The `text` is the same figure snippet the image paste emits, with the relative path substituted,
so a dragged figure and a pasted one are byte-identical apart from the filename.

- [ ] **Step 4: Run and confirm green**

```bash
cargo test -p taliesin-server --bin taliesin lsp_insert 2>&1 | tail -20
```

Expected: PASS, 20 tests.

- [ ] **Step 5: Mutation-verify**

Make `outside` always `None`, re-run, confirm
`a_dragged_image_outside_the_doc_tree_says_so_and_still_offers_a_path` fails, restore.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/lsp_insert.rs
git commit -m "feat(lsp): insertEdit answers whether a dragged asset is shippable

The verdict calls includes::repo_boundary and inside_repo, the same
functions copy_local_assets calls, and mirrors both of the cases it
warns about separately (outside the doc tree, outside the repository). A
second containment rule here would let the editor bless a path the build
then warns on, which is the bug class the gesture exists to prevent."
```

---

### Task 7: The TypeScript paste and drop providers

**Files:**
- Create: `editor/vscode/src/insert.ts`
- Modify: `editor/vscode/src/extension.ts`
- Test: `editor/vscode/src/test/insert.test.ts`

**Interfaces:**
- Consumes: `taliesin/insertEdit` and every kind from Tasks 2 to 6.
- Produces: `export function registerInsertProviders(context: vscode.ExtensionContext): void`,
  and `export function classifyPaste(mimes: readonly string[], hasSelection: boolean): InsertKind | "url" | null`
  as a pure exported function so the routing is unit-testable without an Extension Host.

- [ ] **Step 1: Write the failing unit tests for the pure routing**

Create `editor/vscode/src/test/insert.test.ts`:

```ts
import * as assert from "node:assert";
import { classifyPaste } from "../insert";

suite("paste routing", () => {
  test("an image mime routes to the image kind", () => {
    assert.strictEqual(classifyPaste(["image/png"], false), "image");
  });

  test("HTML wins over the plain-text fallback a spreadsheet also puts on the clipboard", () => {
    assert.strictEqual(classifyPaste(["text/html", "text/plain"], false), "htmlTable");
  });

  test("a URL only becomes a link when there is a selection to wrap", () => {
    assert.strictEqual(classifyPaste(["text/plain"], true), "url");
    // With no selection there is nothing to wrap, so the plain paste must win.
    assert.strictEqual(classifyPaste(["text/plain"], false), null);
  });

  test("an unknown mime routes nowhere", () => {
    assert.strictEqual(classifyPaste(["application/octet-stream"], true), null);
  });
});
```

`classifyPaste` deliberately does not decide `tsvTable` or `bibtex`: both need the pasted *text*,
which the provider inspects after the routing step. Keep that split, and note it in a comment.

- [ ] **Step 2: Run and watch it fail**

```bash
cd editor/vscode && npm run compile
```

Expected: FAIL, cannot find module `../insert`.

- [ ] **Step 3: Implement `insert.ts`**

Write the module with:

- `const INSERT_EDIT = "taliesin/insertEdit";` and the `InsertEditResult` interface mirroring
  Task 2's serialized shape (`text`, `isSnippet`, `writeFile`, `append`, `outside`).
- `classifyPaste` exactly as the tests require.
- One `vscode.DocumentPasteEditProvider` whose `provideDocumentPasteEdits` routes via
  `classifyPaste`, then for `text/plain` additionally sniffs BibTeX (`/^\s*@\w+\s*\{/`) and TSV
  (a tab present) before asking the server. A `DocumentPasteEdit` is constructed as
  `new vscode.DocumentPasteEdit(insertText, title, kind)` where `insertText` is a
  `vscode.SnippetString` when `result.isSnippet`, and `kind` is
  `vscode.DocumentDropOrPasteEditKind.Text.append("taliesin", "<name>")`.
- Registration metadata: `providedPasteEditKinds` listing the kinds used, and `pasteMimeTypes`
  listing `image/png`, `image/jpeg`, `image/svg+xml`, `image/webp`, `image/gif`, `text/html`,
  `text/plain`.
- **The HTML table edit is the default and the TSV table edit is not.** Express this with
  `yieldTo`: the TSV edit yields to `vscode.DocumentDropOrPasteEditKind.Text`, so plain-text
  paste wins unless the author picks "paste as table" from the paste-as menu. Plain text with
  tabs in it is not a table, and silently becoming one is worse than one extra keystroke.
- One `vscode.DocumentDropEditProvider` whose `provideDocumentDropEdits` reads `text/uri-list`,
  sends `dataset` for a `.csv` and `asset` otherwise, and on a non-null `result.outside` shows a
  `showWarningMessage` with two actions ("Copy into the document folder", "Insert path anyway"),
  copying the file beside the document when the first is chosen and then re-asking the server for
  the now-inside path.
- For `image`, write `result.writeFile` beside the document with
  `vscode.workspace.fs.writeFile` before returning the edit.
- For `result.append`, attach a `vscode.WorkspaceEdit` as `edit.additionalEdit` that inserts
  `append.text` at the end of `append.path`, so the whole gesture is one undo.
- Wrap every `sendRequest` in a `try`/`catch`: a refusal arrives as a `RequestFailed` error whose
  `message` is meant for the author. Return `undefined` from the provider (so the plain paste
  wins) and show the message only for a gesture the author explicitly chose, never for a
  speculative route.

Then call `registerInsertProviders(context);` from `activate` in `extension.ts`, beside
`registerCommands(context);`.

- [ ] **Step 4: Run and confirm green**

```bash
cd editor/vscode && npm run compile && npm test -- --grep "paste routing"
```

Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
git add editor/vscode/src/insert.ts editor/vscode/src/extension.ts editor/vscode/src/test/insert.test.ts
git commit -m "feat(companion): paste and drop providers for .tmd

Six gestures over one provider pair. Every inserted string comes from
taliesin/insertEdit, so this module owns only the parts VS Code owns:
clipboard bytes, the file write, the paste-as menu and the undo grouping.

The TSV table edit yields to plain text so a paste containing tabs does
not silently become a table; the HTML table edit, which had a real
<table> on the clipboard, is the default."
```

---

### Task 8: Extension Host coverage for the gestures, and the honest gap

**Files:**
- Modify: `editor/vscode/src/e2e/suite/integration.test.ts`
- Modify: `notes/DETECTION-DEBT.md`

**Interfaces:**
- Consumes: everything from Tasks 1 to 7.
- Produces: nothing.

A unit test proves the server answers and the routing is right. It cannot prove VS Code accepted
the provider, which is exactly how a provider registered against a stale `engines.vscode` fails.

- [ ] **Step 1: Add the drop test, which can be driven end to end**

Append to the existing suite in `integration.test.ts`:

```ts
test("dropping a CSV inserts a dataset card and a loader cell", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tali-drop-"));
  const doc = path.join(dir, "notes.tmd");
  fs.writeFileSync(doc, "# Notes\n");
  const csv = path.join(dir, "m.csv");
  fs.writeFileSync(csv, "a,b\n1,2\n");

  const opened = await vscode.workspace.openTextDocument(vscode.Uri.file(doc));
  const editor = await vscode.window.showTextDocument(opened);

  const dt = new vscode.DataTransfer();
  dt.set("text/uri-list", new vscode.DataTransferItem(vscode.Uri.file(csv).toString()));

  // executeDocumentDropEdits is not a command, so drive the registered provider the way
  // VS Code does: through the built-in drop command is not available to tests either, so
  // assert the provider is reachable by pasting the equivalent instead. See the note below.
  const edits = await vscode.commands.executeCommand<vscode.DocumentDropEdit[] | undefined>(
    "vscode.executeDocumentDropEditProvider",
    opened.uri,
    new vscode.Position(1, 0),
    dt
  );
  assert.ok(edits && edits.length > 0, "a provider answered the drop");
  const text = String(edits[0].insertText);
  assert.match(text, /\{\{< dataset m\.csv >\}\}/);
  assert.match(text, /```\{python\}/);
  editor.hide?.();
});
```

`vscode.executeDocumentDropEditProvider` may not exist as a command. **Verify first:**

```bash
cd editor/vscode && node -e "1" # placeholder; check the command list at runtime instead
```

In the Extension Host, log `(await vscode.commands.getCommands(true)).filter(c => c.includes('DropEdit'))`
from a scratch test and use whatever is actually there. If no such command exists, import the
provider factory from `../../../insert` and call `provideDocumentDropEdits` directly, and say in a
comment that this proves the provider's logic inside a real host but not that VS Code routed a
drop to it.

- [ ] **Step 2: Add the URL-over-selection test, which the real clipboard can drive**

```ts
test("pasting a URL over a selection makes a link", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tali-paste-"));
  const doc = path.join(dir, "notes.tmd");
  fs.writeFileSync(doc, "See the manual here.\n");

  const opened = await vscode.workspace.openTextDocument(vscode.Uri.file(doc));
  const editor = await vscode.window.showTextDocument(opened);
  // Select "manual".
  editor.selection = new vscode.Selection(new vscode.Position(0, 8), new vscode.Position(0, 14));

  await vscode.env.clipboard.writeText("https://taliesin.dev/guide");
  await vscode.commands.executeCommand("editor.action.clipboardPasteAction");

  // The paste is asynchronous through the provider; poll rather than sleep once.
  for (let i = 0; i < 40 && !opened.getText().includes("]("); i++) {
    await new Promise((r) => setTimeout(r, 50));
  }
  assert.match(opened.getText(), /\[manual\]\(https:\/\/taliesin\.dev\/guide\)/);
});
```

- [ ] **Step 3: Run the e2e suite**

```bash
cd editor/vscode && npm run compile && npm run test:e2e 2>&1 | tail -30
```

Expected: PASS for the new tests. If the two pre-existing list-continuation tests fail with "the
Enter keystroke was never delivered", that is the **known load-sensitivity**, not a regression:
re-run alternating baseline and branch pairs at low load before treating it as one.

- [ ] **Step 4: Record the gap that cannot be closed**

`vscode.env.clipboard` is text-only, so **no test can place an image on the clipboard** and drive
the image paste through VS Code's real paste pipeline. Add a row to `notes/DETECTION-DEBT.md`
matching the table's existing columns, saying: an image paste is exercised only by calling the
provider directly, so a break in VS Code's own routing of `image/*` to the provider would ship
silently; the harness change it needs is an Extension Host test that can seed binary clipboard
data, which the API does not expose today.

- [ ] **Step 5: Commit**

```bash
git add editor/vscode/src/e2e/suite/integration.test.ts notes/DETECTION-DEBT.md
git commit -m "test(companion): Extension Host coverage for the paste and drop gestures

A unit test can prove the server answers; only a real host proves VS Code
accepted the provider, which is how a provider registered against a stale
engines.vscode fails.

DETECTION-DEBT gains a row for the half that cannot be closed:
vscode.env.clipboard is text-only, so no test can put an image on the
clipboard, and the image paste is driven by calling the provider directly."
```

---

### Task 9: `taliesin/renameFileEdits`, the inbound half

**Files:**
- Create: `crates/server/src/lsp_rename_file.rs`
- Modify: `crates/server/src/lsp.rs` (constant, dispatch arm)
- Modify: `docs/internals/extending.tmd` (gated row)

**Interfaces:**
- Consumes: `taliesin_core::site::enclosing_site_root` (`crates/core/src/site/mod.rs:261`, public).
- Produces:
  - `pub(crate) struct RenameFileEditsParams { files: Vec<RenamedFile> }`, `#[serde(rename_all = "camelCase")]`
  - `pub(crate) struct RenamedFile { old_uri: lsp_types::Url, new_uri: lsp_types::Url }`, `#[serde(rename_all = "camelCase")]`
  - `pub(crate) struct FileEdits { uri: lsp_types::Url, edits: Vec<lsp_types::TextEdit> }`, `#[serde(rename_all = "camelCase")]`
  - `pub(crate) fn rename_file_edits(params: &RenameFileEditsParams) -> Vec<FileEdits>`
  - `pub(crate) const RENAME_FILE_EDITS_METHOD: &str = "taliesin/renameFileEdits";` (in `lsp.rs`)

- [ ] **Step 1: Write the failing tests against a temp copy of `corpus/tarn`**

Create `crates/server/src/lsp_rename_file.rs` with the types, `rename_file_edits` as `todo!()`, and:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    /// A real multi-chapter book with parts and a nested part, copied so the test can rename
    /// inside it. Never edit `corpus/` in place: the walker renders every corpus document on
    /// every `cargo test`, so an in-place edit poisons every later assertion.
    fn tarn_copy() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/tarn");
        let dst = dir.path().join("tarn");
        copy_tree(&src, &dst);
        (dir, dst)
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (f, t) = (e.path(), to.join(e.file_name()));
            // `_freeze` is a build artifact and can be large; skip it.
            if f.file_name().is_some_and(|n| n == "_freeze") {
                continue;
            }
            if f.is_dir() {
                copy_tree(&f, &t);
            } else {
                std::fs::copy(&f, &t).unwrap();
            }
        }
    }

    fn url(p: &Path) -> lsp_types::Url {
        lsp_types::Url::from_file_path(p).unwrap()
    }

    /// Apply the returned edits and read every touched file back, so the assertions are about
    /// resulting TEXT rather than about ranges. A range assertion passes while pointing at the
    /// wrong line.
    fn applied(edits: &[FileEdits]) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        for f in edits {
            let path = f.uri.to_file_path().unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
            // Later lines first, so an edit cannot shift the line an earlier edit targets.
            let mut sorted = f.edits.clone();
            sorted.sort_by_key(|e| std::cmp::Reverse(e.range.start.line));
            for e in sorted {
                let i = e.range.start.line as usize;
                let (a, b) = (e.range.start.character as usize, e.range.end.character as usize);
                let line = &lines[i];
                lines[i] = format!("{}{}{}", &line[..a], e.new_text, &line[b..]);
            }
            out.push((path, lines.join("\n")));
        }
        out
    }

    #[test]
    fn a_rename_rewrites_every_inbound_reference_in_the_project() {
        let (_tmp, root) = tarn_copy();
        // Pick a chapter that something else in the book actually references, and assert that
        // precondition: a test that renames an unreferenced file passes while proving nothing.
        let (old, referrers) = a_referenced_chapter(&root);
        assert!(!referrers.is_empty(), "precondition: the chapter is referenced somewhere");

        let new = old.with_file_name("renamed-chapter.tmd");
        let edits = rename_file_edits(&RenameFileEditsParams {
            files: vec![RenamedFile { old_uri: url(&old), new_uri: url(&new) }],
        });

        assert!(!edits.is_empty(), "the walk found the referrers");
        for (path, text) in applied(&edits) {
            let stem = old.file_stem().unwrap().to_str().unwrap();
            assert!(
                !text.contains(&format!("{stem}.tmd")) && !text.contains(&format!("{stem}.html")),
                "{} still references the old name",
                path.display()
            );
            assert!(text.contains("renamed-chapter"), "{} was rewritten", path.display());
        }
    }

    #[test]
    fn an_html_spelled_cross_page_link_is_rewritten_too() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
        std::fs::write(root.join("intro.tmd"), "# Intro\n").unwrap();
        // Cross-page links are authored as `.html`, so a rename that only handles the `.tmd`
        // spelling leaves a dead link and reports success.
        std::fs::write(root.join("two.tmd"), "See [intro](intro.html) and intro.tmd.\n").unwrap();

        let edits = rename_file_edits(&RenameFileEditsParams {
            files: vec![RenamedFile {
                old_uri: url(&root.join("intro.tmd")),
                new_uri: url(&root.join("overview.tmd")),
            }],
        });

        let (_, text) = applied(&edits).into_iter().next().expect("two.tmd was edited");
        assert!(text.contains("(overview.html)"), "the .html spelling: {text}");
        assert!(text.contains("overview.tmd"), "the .tmd spelling: {text}");
    }

    #[test]
    fn a_site_yml_entry_is_edited_as_text_and_keeps_its_comments() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let yml = "title: P\n# keep this comment\nchapters:\n  - intro.tmd   # and this one\n";
        std::fs::write(root.join("_site.yml"), yml).unwrap();
        std::fs::write(root.join("intro.tmd"), "# Intro\n").unwrap();

        let edits = rename_file_edits(&RenameFileEditsParams {
            files: vec![RenamedFile {
                old_uri: url(&root.join("intro.tmd")),
                new_uri: url(&root.join("overview.tmd")),
            }],
        });

        let (_, text) = applied(&edits)
            .into_iter()
            .find(|(p, _)| p.ends_with("_site.yml"))
            .expect("_site.yml was edited");
        assert!(text.contains("overview.tmd"));
        // A YAML round-trip would reformat the file and drop both comments.
        assert!(text.contains("# keep this comment"), "{text}");
        assert!(text.contains("# and this one"), "{text}");
    }

    #[test]
    fn a_document_under_no_site_yml_gets_no_inbound_walk() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // No `_site.yml`: item 70 rules that such a tree declares no boundary and that
        // inferring one is the wrong move, so there is nothing to walk.
        std::fs::write(root.join("a.tmd"), "See [b](b.html).\n").unwrap();
        std::fs::write(root.join("b.tmd"), "# B\n").unwrap();

        let edits = rename_file_edits(&RenameFileEditsParams {
            files: vec![RenamedFile {
                old_uri: url(&root.join("b.tmd")),
                new_uri: url(&root.join("c.tmd")),
            }],
        });

        assert!(edits.is_empty(), "no boundary means no inbound repair: {edits:?}");
    }
}
```

Write `a_referenced_chapter(&root)` as a helper that scans the copied book for a `.tmd` whose stem
appears in another file, returning the path and its referrers. **Do not hard-code a chapter name**:
`corpus/tarn`'s contents may change, and a hard-coded name turns a corpus edit into a mysterious
failure here. If no chapter is referenced by another, `assert!` with a message saying the fixture
no longer carries the shape, which is a real finding rather than a silent pass.

- [ ] **Step 2: Run and watch them fail**

```bash
cargo test -p taliesin-server --bin taliesin lsp_rename_file 2>&1 | tail -20
```

Expected: FAIL (`todo!()`).

- [ ] **Step 3: Implement the inbound walk**

`rename_file_edits` for each renamed file:

1. `taliesin_core::site::enclosing_site_root(old_path)`. `None` means return no edits for this
   file (item 70).
2. Walk the project for `.tmd` files plus the `_site.yml` itself. Reuse the page walk
   `site::anchors_defined_elsewhere_in_project` uses if it is reachable; otherwise a recursive
   directory read, skipping `_site`, `_book`, `_freeze` and dotted directories.
3. For each file, compute the old and new paths **relative to that file's own directory** and, for
   each of the two spellings (`.tmd` as written, and the `.html` the link form uses), emit a
   `TextEdit` per occurrence.
4. Occurrence matching is textual but must not match a longer name: require the character before
   the match to not be a path or word character, and the character after the extension to not be
   alphanumeric. A rename of `intro.tmd` must not touch `my-intro.tmd`.
5. Skip the renamed file itself: its own references are the outbound half (Task 10).

The `.html` spelling only applies when the renamed file is a `.tmd`. For the asset case (decision 5
in the spec) there is one spelling and no `_site.yml` chapter entry to consider.

- [ ] **Step 4: Run and confirm green**

```bash
cargo test -p taliesin-server --bin taliesin lsp_rename_file 2>&1 | tail -20
```

Expected: PASS, 4 tests.

- [ ] **Step 5: Wire the dispatch arm and the docs row**

Add `RENAME_FILE_EDITS_METHOD` beside `INSERT_EDIT_METHOD`, a dispatch arm returning
`serde_json::to_value(rename_file_edits(&params))` (no refusal path: an empty list is the correct
answer for "nothing to repair"), then watch the census gate fail and add the row:

```
| `taliesin/renameFileEdits` | the edits that keep a renamed or moved file's references correct: every `{{< include >}}`, `{{< embed >}}`, relative link and `_site.yml` entry pointing at it, plus the file's own outgoing relative references when it changed directory. A rename is a one-shot event, so this walks the project rather than maintaining an index |
```

- [ ] **Step 6: Confirm green**

```bash
cargo test -p taliesin-server --bin taliesin lsp 2>&1 | tail -5
```

- [ ] **Step 7: Mutation-verify the `.html` spelling**

Delete the `.html` spelling from the occurrence search, re-run, confirm
`an_html_spelled_cross_page_link_is_rewritten_too` fails, restore.

- [ ] **Step 8: Commit**

```bash
git add crates/server/src/lsp_rename_file.rs crates/server/src/lsp.rs docs/internals/extending.tmd
git commit -m "feat(lsp): taliesin/renameFileEdits, the inbound half

Renaming a .tmd rewrites every include, embed, relative link and
_site.yml entry pointing at it, found by walking the enclosing project.
Both link spellings are handled: cross-page links are authored as .html,
so handling only .tmd leaves a dead link and reports success.

_site.yml is edited as text, never re-serialized, so the author's
comments and formatting survive. A document under no _site.yml gets no
inbound walk at all: item 70 rules that such a tree declares no boundary
and that inferring one is the wrong move."
```

---

### Task 10: The outbound half

**Files:**
- Modify: `crates/server/src/lsp_rename_file.rs`

**Interfaces:**
- Consumes: Task 9's types and `rename_file_edits`.
- Produces: no new names; `rename_file_edits` now also returns edits for the moved file itself.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn moving_a_file_rebases_its_own_relative_references() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
    std::fs::create_dir_all(root.join("chapters")).unwrap();
    std::fs::create_dir_all(root.join("parts/one")).unwrap();
    std::fs::write(root.join("chapters/scree.png"), "").unwrap();
    let old = root.join("chapters/intro.tmd");
    std::fs::write(
        &old,
        "![A scree plot](scree.png){#fig-scree}\n\n{{< include ../_includes/x.tmd >}}\n",
    )
    .unwrap();

    let edits = rename_file_edits(&RenameFileEditsParams {
        files: vec![RenamedFile {
            old_uri: url(&old),
            new_uri: url(&root.join("parts/one/intro.tmd")),
        }],
    });

    let (_, text) = applied(&edits)
        .into_iter()
        .find(|(p, _)| p.ends_with("intro.tmd"))
        .expect("the moved file is edited");
    // chapters/ -> parts/one/ is two levels deeper plus a sibling hop.
    assert!(text.contains("](../../chapters/scree.png)"), "{text}");
    assert!(text.contains("{{< include ../../_includes/x.tmd >}}"), "{text}");
}

#[test]
fn an_in_place_rename_leaves_the_files_own_references_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
    std::fs::write(root.join("scree.png"), "").unwrap();
    let old = root.join("intro.tmd");
    std::fs::write(&old, "![S](scree.png){#fig-s}\n").unwrap();

    let edits = rename_file_edits(&RenameFileEditsParams {
        files: vec![RenamedFile {
            old_uri: url(&old),
            new_uri: url(&root.join("overview.tmd")),
        }],
    });

    // The directory did not change, so nothing inside the file broke. Rewriting it anyway
    // would churn the diff and risk breaking a path that was already correct.
    assert!(
        !edits.iter().any(|f| f.uri.to_file_path().unwrap().ends_with("intro.tmd")),
        "no outbound edits for a same-directory rename: {edits:?}"
    );
}

#[test]
fn an_absolute_or_external_reference_is_never_rebased() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("_site.yml"), "title: P\n").unwrap();
    std::fs::create_dir_all(root.join("a")).unwrap();
    std::fs::create_dir_all(root.join("b")).unwrap();
    let old = root.join("a/p.tmd");
    std::fs::write(
        &old,
        "[ext](https://example.org/x.png) [root](/logo.png) ![m](mailto:x@y.z)\n",
    )
    .unwrap();

    let edits = rename_file_edits(&RenameFileEditsParams {
        files: vec![RenamedFile { old_uri: url(&old), new_uri: url(&root.join("b/p.tmd")) }],
    });

    let moved = edits.iter().find(|f| f.uri.to_file_path().unwrap().ends_with("p.tmd"));
    assert!(
        moved.is_none_or(|f| f.edits.is_empty()),
        "a URL, a root-absolute path and a mailto: are not relative refs: {moved:?}"
    );
}
```

- [ ] **Step 2: Run and watch them fail**

```bash
cargo test -p taliesin-server --bin taliesin lsp_rename_file 2>&1 | tail -20
```

Expected: FAIL, the first two at least.

- [ ] **Step 3: Implement**

When `old.parent() != new.parent()`, scan the moved file's text for relative references and emit a
`TextEdit` per reference whose target resolves to a real path under the project:

- Markdown link and image targets: the `(...)` of `[...](...)` and `![...](...)`
- `{{< include … >}}`, `{{< embed … >}}` and `{{< dataset … >}}` arguments

Skip anything that is not a relative path: a scheme (`https:`, `mailto:`), a leading `/`, and a
bare fragment (`#sec-x`). For each kept reference, resolve it against the old directory and
re-express it relative to the new directory.

- [ ] **Step 4: Run and confirm green**

```bash
cargo test -p taliesin-server --bin taliesin lsp_rename_file 2>&1 | tail -20
```

Expected: PASS, 7 tests.

- [ ] **Step 5: Mutation-verify the same-directory guard**

Remove the `old.parent() != new.parent()` condition, re-run, confirm
`an_in_place_rename_leaves_the_files_own_references_alone` fails, restore.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/lsp_rename_file.rs
git commit -m "feat(lsp): renameFileEdits rebases the moved file's own references

A move across directories breaks the file's own images, includes and
links, and a repair that fixes only the inbound half tells the author it
handled the rename while leaving the other half broken.

Skipped for a same-directory rename, where nothing inside the file
changed, and never applied to a URL, a root-absolute path or a bare
fragment."
```

---

### Task 11: The `onWillRenameFiles` hook

**Files:**
- Create: `editor/vscode/src/rename.ts`
- Modify: `editor/vscode/src/extension.ts`
- Modify: `editor/vscode/src/e2e/suite/integration.test.ts`

**Interfaces:**
- Consumes: `taliesin/renameFileEdits` (Tasks 9 and 10).
- Produces: `export function registerRenameRepair(context: vscode.ExtensionContext): void`.

- [ ] **Step 1: Write the failing Extension Host test**

```ts
test("renaming a chapter repairs the references pointing at it", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tali-rename-"));
  fs.writeFileSync(path.join(dir, "_site.yml"), "title: P\n");
  fs.writeFileSync(path.join(dir, "intro.tmd"), "# Intro\n");
  fs.writeFileSync(path.join(dir, "two.tmd"), "See [intro](intro.html).\n");

  const oldUri = vscode.Uri.file(path.join(dir, "intro.tmd"));
  const newUri = vscode.Uri.file(path.join(dir, "overview.tmd"));

  // The real editor rename, so the assertion covers the hook, the request and the
  // WorkspaceEdit application rather than any one of them alone.
  const we = new vscode.WorkspaceEdit();
  we.renameFile(oldUri, newUri);
  assert.ok(await vscode.workspace.applyEdit(we), "the rename applied");

  for (let i = 0; i < 60; i++) {
    if (fs.readFileSync(path.join(dir, "two.tmd"), "utf8").includes("overview.html")) break;
    await new Promise((r) => setTimeout(r, 50));
  }
  assert.match(fs.readFileSync(path.join(dir, "two.tmd"), "utf8"), /\(overview\.html\)/);
});
```

Note in a comment that `applyEdit` with a `renameFile` does fire `onWillRenameFiles`, and if the
run shows it does not, fall back to `vscode.workspace.fs.rename` and record which one the editor
actually hooks.

- [ ] **Step 2: Run and watch it fail**

```bash
cd editor/vscode && npm run compile && npm run test:e2e 2>&1 | grep -A5 "repairs the references"
```

Expected: FAIL, `two.tmd` still says `intro.html`.

- [ ] **Step 3: Implement `rename.ts`**

```ts
import * as vscode from "vscode";
import { client } from "./client";

const RENAME_FILE_EDITS = "taliesin/renameFileEdits";

interface FileEdits {
  uri: string;
  edits: { range: { start: { line: number; character: number }; end: { line: number; character: number } }; newText: string }[];
}

/**
 * Repair the references around a renamed or moved file.
 *
 * The whole computation is `taliesin/renameFileEdits`: which references exist, which of the two
 * link spellings a page used, and where a `_site.yml` scalar sits are all `.tmd` knowledge, and
 * a scan here would be a second copy of it.
 *
 * `waitUntil` rather than a follow-up edit, so the repair lands with the rename and the author
 * undoes both together. There is deliberately no confirmation prompt: TypeScript's
 * `updateImportsOnFileMove.enabled` offers one because its repair can be wrong across a large
 * workspace, and Taliesin's is scoped to a declared project with one undo behind it.
 */
export function registerRenameRepair(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.workspace.onWillRenameFiles((event) => {
      const files = event.files.filter((f) => f.oldUri.scheme === "file");
      if (files.length === 0) return;
      event.waitUntil(repair(files));
    })
  );
}

async function repair(
  files: readonly { oldUri: vscode.Uri; newUri: vscode.Uri }[]
): Promise<vscode.WorkspaceEdit> {
  const edit = new vscode.WorkspaceEdit();
  const c = client();
  if (!c) return edit;
  let answer: FileEdits[];
  try {
    answer = await c.sendRequest<FileEdits[]>(RENAME_FILE_EDITS, {
      files: files.map((f) => ({ oldUri: f.oldUri.toString(), newUri: f.newUri.toString() })),
    });
  } catch (e) {
    // A rename must never fail because the repair did. Report and let the rename proceed.
    vscode.window.showWarningMessage(
      `Taliesin: could not update references (${String((e as Error).message || e)})`
    );
    return edit;
  }
  for (const file of answer) {
    const uri = vscode.Uri.parse(file.uri);
    for (const e of file.edits) {
      edit.replace(
        uri,
        new vscode.Range(
          new vscode.Position(e.range.start.line, e.range.start.character),
          new vscode.Position(e.range.end.line, e.range.end.character)
        ),
        e.newText
      );
    }
  }
  return edit;
}
```

`client()` is however `client.ts` already exposes the running `LanguageClient`. Check its exports
and use what is there rather than adding a second accessor.

Then call `registerRenameRepair(context);` from `activate`.

- [ ] **Step 4: Run and confirm green**

```bash
cd editor/vscode && npm run compile && npm run test:e2e 2>&1 | grep -A5 "repairs the references"
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add editor/vscode/src/rename.ts editor/vscode/src/extension.ts editor/vscode/src/e2e/suite/integration.test.ts
git commit -m "feat(companion): renaming a .tmd repairs its references

onWillRenameFiles with waitUntil, so the repair lands with the rename and
one undo reverses both. Every edit comes from taliesin/renameFileEdits;
this module decides nothing about .tmd.

No confirmation prompt: TypeScript's updateImportsOnFileMove offers one
because its repair spans a whole workspace, and this is scoped to a
declared project with one undo behind it. A failed repair warns and lets
the rename proceed rather than blocking it."
```

---

### Task 12: Terminal links, with the drift gate

**Files:**
- Create: `editor/vscode/src/termlinks.ts`
- Create: `crates/server/tests/terminal_link_pattern.rs`
- Modify: `editor/vscode/src/extension.ts`
- Test: `editor/vscode/src/test/termlinks.test.ts`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `export const DIAGNOSTIC_LINE = /^(\S+?\.tmd)(?::(\d+))?:\s/;` exported so both the unit test
    and the Rust drift gate can find one single source of the pattern.
  - `export function registerTerminalLinks(context: vscode.ExtensionContext): void`

- [ ] **Step 1: Write the failing unit tests**

```ts
import * as assert from "node:assert";
import { DIAGNOSTIC_LINE } from "../termlinks";

suite("terminal diagnostic links", () => {
  test("check's located form matches, and yields file and line", () => {
    const m = DIAGNOSTIC_LINE.exec("posts/intro.tmd:12: warning[TAL-XREF]: unresolved @fig-a");
    assert.ok(m);
    assert.strictEqual(m[1], "posts/intro.tmd");
    assert.strictEqual(m[2], "12");
  });

  test("check's unlocated form matches with no line", () => {
    const m = DIAGNOSTIC_LINE.exec("posts/intro.tmd: error[TAL-FM]: bad front matter");
    assert.ok(m);
    assert.strictEqual(m[1], "posts/intro.tmd");
    assert.strictEqual(m[2], undefined);
  });

  test("build's bare form matches", () => {
    const m = DIAGNOSTIC_LINE.exec("chapters/two.tmd:7: include not resolved");
    assert.ok(m);
    assert.strictEqual(m[2], "7");
  });

  test("prose that merely mentions a file is not a link", () => {
    assert.strictEqual(DIAGNOSTIC_LINE.exec("rendered posts/intro.tmd in 12ms"), null);
  });
});
```

- [ ] **Step 2: Run and watch it fail**

```bash
cd editor/vscode && npm run compile
```

Expected: FAIL, cannot find module `../termlinks`.

- [ ] **Step 3: Implement `termlinks.ts`**

Export `DIAGNOSTIC_LINE` and a provider whose `provideTerminalLinks` runs the pattern against
`context.line`, and whose `handleTerminalLink` opens the file at the line. Resolve the path
against the terminal's cwd when the companion created the terminal, else against each workspace
folder, and **return no link when more than one candidate exists**: a link that opens the wrong
file is worse than plain text. Register it from `activate`.

Note in a comment that the pattern is anchored at line start, which makes it correct whether or
not `TerminalLinkContext.line` arrives with the ANSI severity colour stripped. State in the test
which one the run showed.

- [ ] **Step 4: Write the Rust drift gate**

Create `crates/server/tests/terminal_link_pattern.rs`:

```rust
//! The companion's terminal-link pattern is the one piece of diagnostic-format knowledge that
//! lives in TypeScript, so it is the one piece that can drift. This reads the pattern out of
//! the TS source and runs it against strings produced by `check`'s own formatter: change either
//! side and this goes red.

#[test]
fn the_companion_pattern_matches_every_shape_the_tools_print() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ts = std::fs::read_to_string(root.join("../../editor/vscode/src/termlinks.ts")).unwrap();

    // One exported literal, so there is exactly one copy to keep honest.
    let line = ts
        .lines()
        .find(|l| l.contains("export const DIAGNOSTIC_LINE"))
        .expect("termlinks.ts must export DIAGNOSTIC_LINE");
    let body = line
        .split_once('/')
        .and_then(|(_, rest)| rest.rsplit_once('/'))
        .map(|(pat, _)| pat.to_owned())
        .expect("the export must be a regex literal");
    let re = regex::Regex::new(&body).expect("the TS pattern must be valid Rust regex too");

    // Exactly the three shapes emitted today: check.rs's located and unlocated forms, and
    // build.rs's bare `path:line: message`.
    for sample in [
        "posts/intro.tmd:12: warning[TAL-XREF]: unresolved @fig-a",
        "posts/intro.tmd: error[TAL-FM]: bad front matter",
        "chapters/two.tmd:7: include not resolved",
    ] {
        assert!(re.is_match(sample), "the companion pattern misses `{sample}`");
    }
    assert!(
        !re.is_match("rendered posts/intro.tmd in 12ms"),
        "the pattern must not link prose that merely names a file"
    );
}
```

If `regex` is not a dev-dependency of `crates/server`, do not add it: rewrite the assertions as
a hand-rolled check of the same three shapes, and say in a comment why (the project declines
dependencies it can do without). Better still, build the sample strings by calling `check`'s own
formatter if it is reachable from an integration test, which removes the hand-written samples
from the gate entirely. Try that first.

- [ ] **Step 5: Run both and confirm green**

```bash
cd editor/vscode && npm run compile && npm test -- --grep "terminal diagnostic"
cd /home/bogo/Documents/personal/taliesin && cargo test -p taliesin-server --test terminal_link_pattern
```

Expected: PASS on both.

- [ ] **Step 6: Mutation-verify the gate in both directions**

Add `:(\d+)` as mandatory in the TS pattern, re-run the Rust gate, confirm RED (the unlocated
form stops matching). Restore. Then change one sample in the gate to a shape nothing prints and
confirm it still passes, which shows the gate needs the samples to be real: replace that sample
with the formatter-derived string if you took that route.

- [ ] **Step 7: Commit**

```bash
git add editor/vscode/src/termlinks.ts editor/vscode/src/test/termlinks.test.ts editor/vscode/src/extension.ts crates/server/tests/terminal_link_pattern.rs
git commit -m "feat(companion): file:line in the dev-server log is clickable

Matches the three shapes the tools actually print, with no column group:
check.rs emits file:line: severity[CODE]: message and a form with no
line, build.rs emits file:line: message. The pattern is anchored at line
start so the ANSI severity colour cannot interfere.

The pattern is the one piece of diagnostic-format knowledge living in
TypeScript, so it gets a drift gate: a Rust test reads the exported
literal out of the TS source and runs it against the shapes check emits."
```

---

### Task 13: Author-facing docs, board bookkeeping, and the full gate run

**Files:**
- Modify: `docs/guide/using/writing.tmd`
- Modify: `notes/backlog.md`
- Modify: `notes/FEATURE-IDEAS.md`

**Interfaces:**
- Consumes: everything.
- Produces: nothing.

- [ ] **Step 1: Document the gestures for the author**

Add a section to `docs/guide/using/writing.tmd` covering all six paste and drop gestures, the
rename repair and the clickable log locations. Match the book's voice, use no em dashes, and state
the two deliberate limits so a reader does not file them as bugs: a document under no `_site.yml`
gets no inbound rename repair, and a pasted BibTeX entry does not create a bibliography.

Then confirm the prose gate is satisfied:

```bash
cargo test -p taliesin-core docs 2>&1 | tail -10
```

- [ ] **Step 2: Update the board**

In `notes/backlog.md`, add a "Now" bullet recording the batch (three items, what each shipped, the
four `FEATURE-IDEAS.md` corrections, and the DETECTION-DEBT row). In `notes/FEATURE-IDEAS.md`
Session 3, mark ideas 73, 84 and 82 as **SHIPPED 2026-07-30** with the same one-line reasons,
following how ideas 68 to 71 were marked. **Delete nothing that has not shipped**, and leave no
`[x]`.

- [ ] **Step 3: Run every gate**

```bash
cd /home/bogo/Documents/personal/taliesin
./tools/gates.sh 2>&1 | tail -40
cd editor/vscode && npm run compile && npm test && npm run test:e2e 2>&1 | tail -30
cd ../../web-client && npx -y -p typescript tsc -p jsconfig.json
cd ../crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json
```

Expected: `gates.sh` green with all interpreter canaries named and passing; both `tsc` runs silent.
`cargo test` aborts remaining binaries at the first failure, so **grep for FAILED rather than
trusting a total**, and re-run before believing a green summary.

- [ ] **Step 4: Commit**

```bash
git add docs/guide/using/writing.tmd notes/backlog.md notes/FEATURE-IDEAS.md
git commit -m "docs: the authoring gestures, and the board record

Six paste and drop gestures, the rename repair and the clickable log
locations, with the two deliberate limits stated so a reader does not
file them as defects: no inbound rename repair under a tree with no
_site.yml, and a pasted BibTeX entry does not create a bibliography."
```

- [ ] **Step 5: Report, do not push**

Push only when the author asks. Report: what shipped, the `gates.sh` output, the mutation checks
performed, and anything still red with its actual output.

---

## Self-Review

**Spec coverage.** Every spec section maps to a task: the engines floor to Task 1; A1 to Task 2;
A3 to Task 3; A5 to Task 4; A6 to Task 5; A2 to Tasks 6 and 7; A4 to Task 7; item B inbound to
Task 9 and outbound to Task 10, with its client half in Task 11; item C to Task 12. The gated
`docs/internals/extending.tmd` rows are inside Tasks 2 and 9, where the methods are added, because
the gate fails in that commit. The guide prose, the board bookkeeping and the full gate run are
Task 13. The spec's three "verify before building" items are resolved: the 1.97 floor is measured
and is a Global Constraint, `insertText` accepting a `SnippetString` is confirmed in the installed
`index.d.ts`, and the ANSI question is made moot by anchoring, with Task 12 recording which is
true. The DETECTION-DEBT row the spec's testing section implies is Task 8, Step 4.

**Placeholder scan.** No "TBD", no "handle edge cases", no "similar to Task N". Three steps
deliberately instruct the implementer to *locate* an existing API rather than giving a name I did
not verify (the bibliography resolver in Task 4, `apply_line_edits` in Task 3, the drop-provider
test command in Task 8). Each says what to search for and what to do if it is absent, which is a
verification instruction rather than a gap. Task 5's `js` case is explicitly conditional on a
helper existing, with "refuse rather than emit a call to a function that does not exist" as the
fallback.

**Type consistency.** `InsertEditParams`, `InsertKind`, `InsertEditResult`, `AppendEdit` and
`insert_edit` are declared once in Task 2 and used unchanged in Tasks 3 to 7. `InsertKind::Asset`
and `InsertEditResult.outside` are added in Task 6 with `skip_serializing_if`, so no earlier task
needs editing. `RenameFileEditsParams`, `RenamedFile`, `FileEdits` and `rename_file_edits` are
declared in Task 9 and extended, not renamed, in Task 10. The TypeScript `InsertEditResult`
interface in Task 7 mirrors Task 2's camelCase serialization, and `FileEdits` in Task 11 mirrors
Task 9's. `DIAGNOSTIC_LINE` has one definition, read by both its unit test and the Rust gate.
