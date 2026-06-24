# Beyond Quarto Wave 1 (Substrate): locate-render-warnings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Carry source location through the render-warning channel so broken-citation, broken-cross-reference, and unknown-shortcode warnings become click-to-source in the preview panel, exactly like front-matter errors already are. This is the substrate the Wave 1 `nested-schema-validation` epic builds on.

**Architecture:** Today `RenderedDoc.warnings: Vec<String>` is flattened to unlocated `Diagnostic::warn(...)` in the servers. The `Diagnostic` type (`protocol.rs`) and the client (`client.js`) already render any diagnostic with a numeric `line` as a clickable jump-to-source, so no protocol or client change is needed. Task 1 introduces a located `Warning { message, file, line }` type in core, converts the channel to `Vec<Warning>` behavior-preservingly (every producer emits `Warning::new(msg)` with `line: None`, so the preview looks identical), and makes the server-side mapping generic: a `Warning` with a `line` becomes a clickable `Diagnostic`. Task 2 then sets real locations at the producers (broken cross-ref, unknown shortcode, broken citation), with no further consumer changes.

**Tech Stack:** Rust edition 2024 / resolver 3; `serde_json` (server only); integration tests under `crates/*/tests/` and in-file `#[cfg(test)]` modules.

## Global Constraints

- Rust edition 2024, resolver 3; no new runtime dependency.
- No em dashes or en dashes in any authored prose, comment, or commit message (commas/colons/parentheses).
- CI enforces `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Each task ends green on all three.
- INVARIANT SAFETY (read these): this change touches only the *warning channel*. It must NOT change the block model (`data-block-id`/`data-sourcepos`), the diff, the `:::` machine, the citation/xref HTML lowering, includes, numbering, or exec/freeze/kernel. `cite::process` must still transform block HTML exactly as before; only its *return type* (the warnings it emits) changes. The corpus-invariant tests in `crates/core/tests/corpus.rs` must stay green untouched.
- `Site::warnings` (the project-config warnings, `crates/core/src/site/mod.rs`) is a SEPARATE `Vec<String>` and is OUT OF SCOPE: do not change its type. Only `RenderedDoc.warnings` and the functions feeding it become `Warning`-typed. (Consequence: `crates/core/tests/config.rs` and `crates/core/tests/loose_deck.rs`, which read `Site::warnings`, do NOT churn.)
- A warning with `line: None` must render and behave EXACTLY as today (unlocated, not clickable). Behavior only changes for warnings that gain a real line in Task 2.
- The `file` field on a located warning is doc-base-relative (matches `Block.source_file`'s contract), or `None` to mean "the document being previewed" (the client falls back to the doc path). Never an absolute path.

---

### Task 1: introduce the located `Warning` type and convert the channel (behavior-preserving)

**Files:**
- Modify: `crates/core/src/render/model.rs` (add `Warning`; change `RenderedDoc.warnings` type)
- Modify: `crates/core/src/render/mod.rs` (accumulator + push sites + `load_bibliography`/`register_xref` signatures + the `RenderedDoc{}` literal + `expand_shortcodes` consumer)
- Modify: `crates/core/src/cite.rs` (`process` + `validate_xrefs` return `Vec<Warning>`; 2 unit tests)
- Modify: `crates/core/src/render/extension/mod.rs` (`expand_shortcodes` + the resolver helpers that take `&mut Vec<String>`)
- Modify: `crates/core/src/site/mod.rs` (`finish_blocks` + `render_page_doc_warned` signatures; keep `Site::warnings` as `Vec<String>`)
- Modify: `crates/server/src/serve.rs` + `crates/server/src/serve_site.rs` (map `Warning` to `Diagnostic`, generically clickable; `DocState.warnings`)
- Modify: `crates/server/src/main.rs` (build-log loops use `.message`)
- Modify (test churn): `crates/core/src/render/tests.rs`, `crates/core/tests/extensions.rs`, `crates/core/src/cite.rs` tests
- Test (new): a unit test for `Warning` in `crates/core/src/render/model.rs`

**Interfaces:**
- Produces: `qmd_fast_core::render::model::Warning` with `pub message: String`, `pub file: Option<String>`, `pub line: Option<u32>`; `Warning::new(impl Into<String>) -> Warning`; `Warning::at(self, file: Option<String>, line: u32) -> Warning`; `impl Display` (writes `message`). Re-exported wherever `RenderedDoc` is (so `qmd_fast_core::...::Warning` resolves for the server).
- Changes: `RenderedDoc.warnings: Vec<Warning>`; `cite::process(...) -> Vec<Warning>`; `cite::validate_xrefs(&[Block]) -> Vec<Warning>`; `load_bibliography(.., &mut Vec<Warning>)`; `register_xref(.., &mut Vec<Warning>, ..)`; `extension::expand_shortcodes(..) -> (String, Vec<Warning>)` and the `&mut Vec<String>` resolver params become `&mut Vec<Warning>`; `Site::finish_blocks(.., &mut Vec<Warning>)`; `Site::render_page_doc_warned(..) -> (String, Vec<Warning>)`.

- [ ] **Step 1: Write the failing unit test for `Warning`**

In `crates/core/src/render/model.rs`, add (or extend) an in-file test module at the end of the file:

```rust
#[cfg(test)]
mod warning_tests {
    use super::Warning;

    #[test]
    fn warning_new_is_unlocated_and_displays_its_message() {
        let w = Warning::new("broken citation: @x");
        assert_eq!(w.message, "broken citation: @x");
        assert_eq!(w.file, None);
        assert_eq!(w.line, None);
        assert_eq!(w.to_string(), "broken citation: @x");
    }

    #[test]
    fn warning_at_attaches_file_and_line() {
        let w = Warning::new("broken cross-reference: @fig-x").at(Some("intro.qmd".into()), 12);
        assert_eq!(w.file.as_deref(), Some("intro.qmd"));
        assert_eq!(w.line, Some(12));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p qmd-fast-core --lib warning_tests`
Expected: FAIL to compile (`Warning` does not exist).

- [ ] **Step 3: Add the `Warning` type**

In `crates/core/src/render/model.rs`, immediately above `pub struct RenderedDoc` (currently around line 124), add:

```rust
/// A non-fatal render warning, optionally carrying a click-to-source location.
/// When `line` is `Some`, the dev server renders it as a clickable diagnostic
/// (jump-to-source); `file` is doc-base-relative (matching `Block::source_file`)
/// or `None` for "the document being previewed". `line: None` is an unlocated
/// warning (logged + shown, not clickable), the same behavior bare strings had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl Warning {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), file: None, line: None }
    }

    /// Attach a click-to-source location.
    pub fn at(mut self, file: Option<String>, line: u32) -> Self {
        self.file = file;
        self.line = Some(line);
        self
    }
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
```

Then change the `warnings` field of `RenderedDoc` (currently `pub warnings: Vec<String>,` around line 161) to:

```rust
    pub warnings: Vec<Warning>,
```

- [ ] **Step 4: Run the `Warning` unit test (now compiles in isolation)**

Run: `cargo test -p qmd-fast-core --lib warning_tests 2>&1 | tail -20`
Expected: the two `warning_tests` PASS. (The wider crate will not yet compile because consumers still pass `String`; that is fixed in the next steps. If `--lib` fails to build the whole crate, proceed to Step 5 and treat Step 4's verification as "deferred until Step 8".)

- [ ] **Step 5: Convert the core producers to `Warning`**

Make these edits in `crates/core/src/render/mod.rs`:
- The accumulator (around line 160): `let mut warnings: Vec<Warning> = Vec::new();`
- Every `warnings.push(format!(...))` site (the duplicate-xref-label push inside `register_xref` around line 1273, and the `bibliography file not found` push in `load_bibliography` around line 890): wrap the `format!(...)` in `Warning::new(...)`, e.g. `warnings.push(Warning::new(format!("bibliography file not found: {tok}")));`
- `fn load_bibliography(field, base_dir, warnings: &mut Vec<Warning>)` (around line 868): change the param type.
- `fn register_xref(reg, warnings: &mut Vec<Warning>, anchor, number)` (around line 1266): change the param type.
- The `RenderedDoc { ... warnings, blocks }` literal (around line 565) needs no change (the local `warnings` is now `Vec<Warning>`).
- `doc.warnings.extend(shortcode_warnings)` (around line 93): unchanged in shape once `expand_shortcodes` returns `Vec<Warning>` (Step 7).
- Import `Warning` at the top of mod.rs alongside the other `model::` imports (find the existing `use ...model::{...}` / `use crate::render::model::...` and add `Warning`).

In `crates/core/src/cite.rs`:
- `pub fn process(blocks, bib, xrefs) -> Vec<Warning>` (around line 486): change the return type; the broken-citation push (around line 519) becomes `warnings.push(Warning::new(format!("broken citation: @{key} (not in the bibliography)")));`. Import `Warning` (`use crate::render::model::Warning;` or via the existing `Block` import path).
- `pub fn validate_xrefs(blocks: &[Block]) -> Vec<Warning>` (around line 546): the final `.map(|a| format!(...))` becomes `.map(|a| Warning::new(format!("broken cross-reference: @{a} (no such figure/section/…)")))`. (Location is added in Task 2; for now it stays unlocated to keep this step behavior-preserving.)
- `parse_bib_warned` (the duplicate-key warnings, around cite.rs:300) returns warnings consumed by `load_bibliography` via `warnings.extend(bib_warnings)`. Make `parse_bib_warned` return `Vec<Warning>` too (wrap its `format!` in `Warning::new`), OR convert at the boundary in `load_bibliography` (`warnings.extend(bib_warnings.into_iter().map(Warning::new))`). Prefer converting at the boundary to keep `parse_bib_warned` simple unless it already returns to multiple callers. Pick one and be consistent.

- [ ] **Step 6: Convert the extension resolver channel**

In `crates/core/src/render/extension/mod.rs`:
- `pub fn expand_shortcodes(...) -> (String, Vec<Warning>)` (the unknown-shortcode warning, around line 444): change the return type; wrap the pushed `format!(...)` in `Warning::new(...)` (Task 2 promotes its embedded line to a real `line`).
- The resolver helpers that currently take `&mut Vec<String>` (`resolve_format_extension`, `resolve_named_extensions`, and the manifest-key / not-found pushes around lines 95, 106, 166): change the param type to `&mut Vec<Warning>` and wrap their `format!(...)` pushes in `Warning::new(...)`.
- Import `Warning` (`use crate::render::model::Warning;`).

- [ ] **Step 7: Convert the site-path channel (keep `Site::warnings` as `Vec<String>`)**

In `crates/core/src/site/mod.rs`:
- `Site::finish_blocks(.., warnings: &mut Vec<Warning>)` (around line 348): change the param type; its internal `warnings.extend(crate::cite::validate_xrefs(blocks))` now extends with `Vec<Warning>` (matches).
- `Site::render_page_doc_warned(..) -> (String, Vec<Warning>)` (around line 328): the `std::mem::take(&mut doc.warnings)` now yields `Vec<Warning>`; update the return type.
- Do NOT change `Site::warnings` (the config warnings field) or `Site::discover`'s warnings: those stay `Vec<String>`.

- [ ] **Step 8: Build the core crate and fix remaining core compile errors**

Run: `cargo build -p qmd-fast-core 2>&1 | tail -30`
Fix any remaining type mismatches the compiler points at (there should be none beyond the sites above). Then run the `Warning` unit test from Step 1 to confirm green:
Run: `cargo test -p qmd-fast-core --lib warning_tests`
Expected: PASS.

- [ ] **Step 9: Fix the core test churn (mechanical: `w.contains(` to `w.message.contains(`)**

These test sites read `RenderedDoc.warnings` (now `Vec<Warning>`) and call `.contains(...)` on the element as if it were a string. At each, change `<var>.contains(` to `<var>.message.contains(` (and `w[0].contains(` to `w[0].message.contains(`). Exact locations (from the current tree):
- `crates/core/src/cite.rs`: the two return-asserting unit tests `broken_citation_warns_only_when_a_bib_exists` (around lines 849-867: `w[0].contains(...)`) and `validate_xrefs_flags_only_unresolved_markers` (around lines 869-891: `w[0].contains(...)`). Change both `w[0].contains(` to `w[0].message.contains(`.
- `crates/core/src/render/tests.rs`: lines around 1057, 1061, 1064, 1068, 1076, 1078 (the `doc.warnings` reads). Change each `.contains(` on a warning element to `.message.contains(`.
- `crates/core/tests/extensions.rs`: lines around 33, 37, 52, 54, 186, 190, 202, 204, 219, 221, 457, 459, 471, 475, 500 (the `doc.warnings.iter().any(|w| w.contains(...))` pattern). Change each `w.contains(` to `w.message.contains(`. Where a test asserts `doc.warnings.is_empty()`, leave it unchanged.

Do NOT touch `crates/core/tests/config.rs` or `crates/core/tests/loose_deck.rs` (they read `Site::warnings`, still `Vec<String>`).

Run: `cargo test -p qmd-fast-core 2>&1 | grep -E 'test result:|error\[' | grep -vE '0 failed' && echo FAILURES || echo "core green"`
Expected: `core green`.

- [ ] **Step 10: Update the server consumers (generic clickable mapping)**

In `crates/server/src/serve.rs`:
- `DocState.warnings` (around line 57): change `Vec<String>` to `Vec<qmd_fast_core::render::model::Warning>` (import or fully-qualify). It is written at init (around line 119) and in `rebuild` (around line 985) from `doc.warnings`, which now matches.
- The render-warning wrapping loop in `rebuild` (around lines 944-949) and the direct `validate_xrefs` loop (around lines 950-953): replace both with the generic mapping:

```rust
    // Render warnings (broken citation/cross-ref, missing bibliography/theme).
    // A warning that carries a line becomes a clickable jump-to-source row.
    for w in &doc.warnings {
        let mut d = Diagnostic::warn(&w.message);
        if let Some(line) = w.line {
            d = d.at(w.file.clone(), line);
        }
        diags.push(d);
    }
    // A standalone doc has no site to resolve cross-page refs, so any cross-ref
    // still marked unresolved is broken.
    for w in qmd_fast_core::cite::validate_xrefs(&blocks) {
        let mut d = Diagnostic::warn(&w.message);
        if let Some(line) = w.line {
            d = d.at(w.file.clone(), line);
        }
        diags.push(d);
    }
```

In `crates/server/src/serve_site.rs`:
- The live-path loop (around lines 816-818) and the non-live loop (around lines 328-333): replace each `Diagnostic::warn(w.clone())` / `.map(|w| Diagnostic::warn(w.clone()))` with the same located mapping (`Diagnostic::warn(&w.message)` plus `.at(w.file.clone(), line)` when `w.line` is `Some`).

In `crates/server/src/main.rs` (build logs, location is irrelevant in a stderr log):
- Single-doc render warnings (around lines 196-198): `for w in &doc.warnings { log::warn(&w.message); }`
- Single-doc broken-ref loop (around lines 211-213): `for w in qmd_fast_core::cite::validate_xrefs(&doc.blocks) { log::warn(&w.message); }`
- Site per-page render warnings (around lines 386-389): `for w in &warnings { log::warn(&format!("{}: {}", page.rel, w.message)); }`
- Leave the `frontmatter::lint` and `Site::warnings` (`site.warnings`) loops unchanged (still `Vec<String>`).

- [ ] **Step 11: Full gate**

Run: `cargo test --workspace 2>&1 | grep -E 'test result:|error\[' | grep -vE '0 failed' && echo FAILURES || echo "all green"`
Then: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: `all green`, fmt clean, clippy clean. No behavior change yet: every warning still has `line: None`, so the preview is identical to before.

- [ ] **Step 12: Commit**

```bash
git add crates/core/src/render/model.rs crates/core/src/render/mod.rs crates/core/src/cite.rs crates/core/src/render/extension/mod.rs crates/core/src/site/mod.rs crates/core/src/render/tests.rs crates/core/tests/extensions.rs crates/server/src/serve.rs crates/server/src/serve_site.rs crates/server/src/main.rs
git commit -m "refactor(warnings): carry an optional source location through the render-warning channel"
```

---

### Task 2: set real locations at the producers (make the warnings clickable)

With Task 1's generic server mapping in place, a warning becomes click-to-source the moment its producer sets a `line`. This task sets locations for the realistic cases: broken cross-reference (location already in scope), unknown shortcode (line already embedded in its message text), and broken citation (needs recording which block a key first appeared in). Missing-bibliography, theme-not-found, duplicate-bib-key, and extension-manifest warnings keep `line: None` (their location is not tracked today; out of scope).

**Files:**
- Modify: `crates/core/src/cite.rs` (`validate_xrefs` per-block location; `process` block-of-first-appearance for broken citations)
- Modify: `crates/core/src/render/extension/mod.rs` (`expand_shortcodes` sets the unknown-shortcode line)
- Test (new): `crates/core/tests/located_warnings.rs`

**Interfaces:**
- Consumes: `Block.sourcepos` (`startLine:startCol-endLine:endCol`) and `Block.source_file` from Task 1's `Warning`. A helper to parse the start line from a sourcepos string.

- [ ] **Step 1: Write the failing integration test**

Create `crates/core/tests/located_warnings.rs`:

```rust
mod common;
use common::TempProj;

/// A broken cross-reference warning carries the source line of the block that
/// contains the dangling `@ref`, so the dev panel can jump to it.
#[test]
fn broken_crossref_warning_is_located() {
    let proj = TempProj::new();
    let doc = qmd_fast_core::render_document_with_includes(
        "# Title\n\nIntro.\n\nSee @fig-nope for details.\n",
        &proj.0,
    );
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("@fig-nope"))
        .or_else(|| {
            // standalone docs surface broken xrefs via validate_xrefs at the server;
            // exercise that path directly so the test does not depend on the server.
            None
        });
    // Cross-refs in a standalone render are validated by `validate_xrefs`:
    let xref_warnings = qmd_fast_core::cite::validate_xrefs(&doc.blocks);
    let located = xref_warnings
        .iter()
        .find(|w| w.message.contains("@fig-nope"))
        .expect("a broken-crossref warning for @fig-nope");
    assert!(
        located.line.is_some(),
        "broken-crossref warning should carry a line, got: {located:?}"
    );
    let _ = w; // doc.warnings path may or may not include it depending on resolution stage
}

/// An unknown-shortcode warning carries the line where the shortcode appears.
#[test]
fn unknown_shortcode_warning_is_located() {
    let proj = TempProj::new();
    let doc = qmd_fast_core::render_document_with_includes(
        "# Title\n\nIntro.\n\n{{< videoo clip.mp4 >}}\n",
        &proj.0,
    );
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("videoo"))
        .expect("an unknown-shortcode warning for `videoo`");
    assert_eq!(w.line, Some(5), "shortcode is on line 5, got: {w:?}");
}
```

(Note: `TempProj` is the existing helper in `crates/core/tests/common/mod.rs`; `.0` is the base-dir `PathBuf`, matching the pattern in `extensions.rs`.)

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p qmd-fast-core --test located_warnings`
Expected: FAIL: `broken_crossref_warning_is_located` (line is `None`) and `unknown_shortcode_warning_is_located` (line is `None`), because Task 1 left all producers unlocated.

- [ ] **Step 3: Locate broken cross-references in `validate_xrefs`**

In `crates/core/src/cite.rs`, rewrite `validate_xrefs` so it captures the first block's location for each distinct broken anchor before dedup (replacing the `BTreeSet<String>` with an order-preserving map of anchor to its first location):

```rust
pub fn validate_xrefs(blocks: &[Block]) -> Vec<crate::render::model::Warning> {
    use crate::render::model::Warning;
    let marker = "data-qmd-xref=\"";
    // First occurrence wins for the reported location; dedup by anchor.
    let mut seen: std::collections::BTreeMap<String, (Option<String>, Option<u32>)> =
        std::collections::BTreeMap::new();
    for b in blocks {
        let loc = (b.source_file.clone(), sourcepos_start_line(&b.sourcepos));
        let mut rest = b.html.as_str();
        while let Some(i) = rest.find(marker) {
            rest = &rest[i + marker.len()..];
            let Some(end) = rest.find('"') else { break };
            let anchor = rest[..end].to_string();
            seen.entry(anchor).or_insert_with(|| loc.clone());
            rest = &rest[end..];
        }
    }
    seen.into_iter()
        .map(|(a, (file, line))| {
            let w = Warning::new(format!(
                "broken cross-reference: @{a} (no such figure/section/…)"
            ));
            match line {
                Some(l) => w.at(file, l),
                None => w,
            }
        })
        .collect()
}
```

Add the sourcepos start-line parser near the top of `cite.rs` (or reuse one if it already exists; search for an existing `sourcepos` parse first):

```rust
/// Parse the 1-based start line out of a `startLine:col-endLine:col` sourcepos.
/// Returns `None` for a generated block (empty sourcepos) or a malformed value.
fn sourcepos_start_line(sourcepos: &str) -> Option<u32> {
    sourcepos.split(':').next()?.parse::<u32>().ok().filter(|&l| l > 0)
}
```

- [ ] **Step 4: Locate the unknown-shortcode warning**

In `crates/core/src/render/extension/mod.rs`, the unknown-shortcode push currently embeds the line in the message (`"... at line {line_no} ..."`). Keep the message text as-is (an existing test asserts `contains("line 5")`), but also attach the real line:

```rust
warnings.push(
    Warning::new(format!(
        "unknown shortcode `{{{{< {name} >}}}}` at line {line_no} (no extension declares it; left as literal text)"
    ))
    .at(None, line_no as u32),
);
```

(`line_no` is the 1-based buffer line already in scope. `file: None` means "the previewed doc". Confirm `line_no`'s type and cast to `u32`; if it is already `u32`, drop the `as u32`.)

- [ ] **Step 5: Locate broken citations in `process` (block-of-first-appearance)**

In `crates/core/src/cite.rs` `process`, the per-key location is not tracked today. Record, during the block transform loop, the location of the block where each cite key first appears, then use it when emitting the broken-citation warning. Concretely: before transforming, scan each block's HTML for the cite markers the lowering will turn into keys is fragile; instead, capture location at the point the key is first registered. The simplest correct approach: in the `cite_key` closure (which already runs as keys are encountered in document order), also record the *current block's* location the first time a key is seen. Thread the current block's `(source_file, start_line)` into the closure via a `Cell`/`RefCell` updated in the `for b in blocks.iter_mut()` loop:

```rust
    let mut order: Vec<String> = Vec::new();
    let mut number: HashMap<String, usize> = HashMap::new();
    let mut key_loc: HashMap<String, (Option<String>, Option<u32>)> = HashMap::new();
    let cur_loc = std::cell::RefCell::new((None::<String>, None::<u32>));
    let mut cite_key = |key: &str| -> usize {
        // existing numbering logic ...
        if !number.contains_key(key) {
            key_loc.insert(key.to_string(), cur_loc.borrow().clone());
        }
        // existing: assign/lookup number, push to order, return it
        # /* keep the existing body; only the key_loc.insert line above is new */
    };
    for b in blocks.iter_mut() {
        *cur_loc.borrow_mut() = (b.source_file.clone(), sourcepos_start_line(&b.sourcepos));
        b.html = transform_html(&b.html, &mut cite_key, xrefs);
    }
```

Then in the broken-citation arm (around line 519), build the located warning from `key_loc`:

```rust
                if !bib.is_empty() {
                    let (file, line) = key_loc.get(key).cloned().unwrap_or((None, None));
                    let w = Warning::new(format!("broken citation: @{key} (not in the bibliography)"));
                    warnings.push(match line {
                        Some(l) => w.at(file, l),
                        None => w,
                    });
                }
```

NOTE: the exact shape of the existing `cite_key` closure and `transform_html` signature must be respected: if `transform_html` borrows `cite_key` as `FnMut(&str) -> usize`, the `RefCell` capture is the least-invasive way to feed it the current block. If this proves to fight the borrow checker (the closure already mutably borrows `order`/`number`), fall back to: do a second pass that, for each broken key, finds the first block whose HTML contains the rendered cite marker for that key, and reads that block's location. If you take the fallback, note it in the report. Do NOT change `transform_html`'s HTML output.

- [ ] **Step 6: Run the located-warnings test**

Run: `cargo test -p qmd-fast-core --test located_warnings`
Expected: PASS (both tests). The broken-xref carries a line; the unknown shortcode reports `line: Some(5)`.

- [ ] **Step 7: Confirm the existing unknown-shortcode message test still passes**

Run: `cargo test -p qmd-fast-core --test extensions unknown_shortcode_warns_with_its_name_and_line`
Expected: PASS (the message text, including `line 5`, is unchanged; we only added a structured `line`).

- [ ] **Step 8: Add a broken-citation location test**

Append to `crates/core/tests/located_warnings.rs`:

```rust
/// A broken citation (a `@key` with a bibliography present but no matching entry)
/// carries the line of the block where the citation appears.
#[test]
fn broken_citation_warning_is_located() {
    let proj = TempProj::new();
    proj.file("refs.bib", "@article{real, title={Real}, author={A}, year={2020}, journal={J}}\n");
    let doc = qmd_fast_core::render_document_with_includes(
        "---\nbibliography: refs.bib\n---\n\n# Title\n\nFirst para.\n\nSee [@missingkey] here.\n",
        &proj.0,
    );
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("@missingkey"))
        .expect("a broken-citation warning for @missingkey");
    assert!(w.line.is_some(), "broken-citation warning should carry a line, got: {w:?}");
}
```

(`TempProj::file(rel, content)` writes a file under the temp project, per `crates/core/tests/common/mod.rs`.)

Run: `cargo test -p qmd-fast-core --test located_warnings`
Expected: all three PASS. If `broken_citation_warning_is_located` cannot be made to pass without changing `transform_html` output, mark it `#[ignore]` with a comment and report the citation-location plumbing as deferred (xref + shortcode locations still deliver the feature).

- [ ] **Step 9: Full gate**

Run: `cargo test --workspace 2>&1 | grep -E 'test result:|error\[' | grep -vE '0 failed' && echo FAILURES || echo "all green"`
Then: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: `all green`, fmt clean, clippy clean.

- [ ] **Step 10: Manual smoke check (preview)**

Run: `cargo run -p qmd-fast-server -- build samples/deck.qmd >/dev/null 2>&1; echo "build ok"` to confirm nothing in the build path regressed. (A full preview click-through is optional; the located-warnings tests already assert the line is carried, and Task 1 confirmed the server maps a lined warning to a clickable diagnostic.)

- [ ] **Step 11: Commit**

```bash
git add crates/core/src/cite.rs crates/core/src/render/extension/mod.rs crates/core/tests/located_warnings.rs
git commit -m "feat(warnings): locate broken cross-refs, unknown shortcodes, and broken citations (click-to-source)"
```

---

## Self-Review

**Spec coverage:** locate-render-warnings delivers click-to-source for broken-citation, broken-cross-reference, and unknown-shortcode warnings (Task 2), on a located `Warning` channel (Task 1). Missing-bib / theme / duplicate-bib-key / extension-manifest warnings intentionally keep `line: None` (location not tracked today); that is documented in Task 2's intro. The substrate (`Warning` + generic server mapping) is what the `nested-schema-validation` epic will reuse.

**Placeholder scan:** Step 5 of Task 2 carries a deliberate fallback path (RefCell capture vs second-pass lookup) because the exact borrow shape of `cite_key`/`transform_html` cannot be fully determined without the implementer reading the live closure; both options are concrete and the report-it instruction is explicit. The `#` line in the Step 5 code block marks "keep the existing body" and must not be copied literally. Everything else is complete code.

**Type consistency:** `Warning` (message/file/line + `new`/`at`/`Display`) is defined in Task 1 and consumed identically in Task 2 and the servers. `cite::process` and `cite::validate_xrefs` return `Vec<Warning>` after Task 1; Task 2 only fills in locations. The server mapping is written once (Task 1, Step 10) and needs no change in Task 2 (producers setting a `line` is sufficient to make a warning clickable), which is the whole point of the generic mapping.

**Scope check:** Two tasks, each independently testable and committable. Task 1 is a behavior-preserving refactor (all existing tests pass after the mechanical `.message` churn; preview unchanged). Task 2 is the additive value with focused new tests. No block-model / diff / sourcepos / `:::` / includes / numbering / exec change. `Site::warnings` (config) deliberately stays `Vec<String>` to contain the blast radius.
