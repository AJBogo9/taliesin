# Beyond Quarto Wave 1 (Epic): nested-schema-validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Push qmd-fast's closed-vocabulary validation all the way down: every `#|` cell option, every `:::` callout kind, and every immediate child of the nested `execute:` / `listing:` / `about:` / `hero:` front-matter blocks is checked against qmd-fast's OWN recognized key set, with a click-to-source "unknown key (did you mean …?)" warning for anything else, riding the located `Warning` channel landed in Wave 1.

**Architecture:** qmd-fast has its own vocabulary, defined independently of Quarto (per the author's directive: "totally leave Quarto behind"). A key not in the relevant closed set, whether a typo or a Quarto term qmd-fast does not implement, is reported as unknown. Three validation surfaces, all emitting located `Warning`s that flow to `doc.warnings` (already mapped to clickable diagnostics by Wave 1): (1) cell options, validated in the render loop where each code cell is built; (2) callout kinds, validated in `build_container` via a threaded `&mut Vec<Warning>`; (3) front-matter top-level + nested config keys, validated by a new `frontmatter::validate_front_matter` that render calls, with membership decided by a real YAML parse and best-effort line location. The corpus is first purged of every Quarto-only key it currently uses (all no-ops for qmd-fast output), so the regression net stays green, then pinned by `corpus/diagnostics/typos.qmd` plus a test asserting the exact warning set.

**Tech Stack:** Rust edition 2024 / resolver 3; `serde_yaml` (already a dependency, used by the existing front-matter linter); integration tests under `crates/core/tests/`, in-file `#[cfg(test)]` unit tests.

## Global Constraints

- Rust edition 2024, resolver 3; no new runtime dependency (`serde_yaml` is already present).
- No em dashes or en dashes in any authored prose, comment, doc, or commit message. Use commas, colons, parentheses, or restructured sentences.
- CI enforces `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Each task ends green on all three.
- INVARIANT SAFETY (read before every task): this epic is READ-ONLY with respect to output. The validators only push `Warning`s; they must NOT change the emitted HTML for any block, must NOT touch the block model (`data-block-id` / `data-sourcepos` / `data-source-file`), the diff, the `:::` three-pass machine's output, the citation/xref/includes/numbering passes, or exec/freeze/kernel. An unrecognized cell option, callout kind, or config key still renders EXACTLY as it does today; only an extra warning is added. The corpus-invariant tests in `crates/core/tests/corpus.rs` (block ids, sourcepos, order) must stay green.
- The `Warning` type (from Wave 1) lives in `crates/core/src/render/model.rs`: `pub struct Warning { pub message: String, pub file: Option<String>, pub line: Option<u32> }`, with `Warning::new(impl Into<String>) -> Warning` and `Warning::at(self, file: Option<String>, line: u32) -> Warning`. It is re-exported from `mod.rs` (`pub use model::{… Warning}`) so `render/` submodules see it via `use super::*`, and as `qmd_fast_core::render::model::Warning` for the server crate. A warning with `line: None` renders unlocated (not clickable), the same as before; a `line: Some(n)` warning is click-to-source.
- `file` on a located warning is doc-base-relative (matches `Block::source_file`) or `None` for "the document being previewed". Never an absolute path.
- The site-config validator (`crate::site::config::validate_keys`, `NATIVE_KEYS`) and its `Site::warnings: Vec<String>` channel are OUT OF SCOPE and unchanged. This epic touches only per-document validation (`doc.warnings: Vec<Warning>`).

## File Structure

- `crates/core/src/render/validate.rs` (NEW): qmd-fast's body vocabularies (`CELL_OPTION_KEYS`, `CALLOUT_KINDS`) plus `cell_option_keys`, `validate_cell_options`, `validate_callout_kind`. One responsibility: validating body-level keys against the closed sets, emitting located `Warning`s.
- `crates/core/src/render/mod.rs` (MODIFY): declare `mod validate;`; call `validate::validate_cell_options` per code cell in the render loop; thread `&mut warnings` into `group_divs`; call `frontmatter::validate_front_matter(src)` once.
- `crates/core/src/render/divs.rs` (MODIFY): thread `&mut Vec<Warning>` through `group_divs` + `build_container`; validate the callout kind in the callout arm.
- `crates/core/src/frontmatter.rs` (MODIFY): reconcile `KNOWN_KEYS` (drop the two genuinely-unhonored Quarto keys, fix the stale "not honored" comment); add the nested key sets + `validate_front_matter` (located, top-level + nested); add the shared `unknown_key_message` helper; replace `lint`.
- `crates/server/src/serve.rs`, `serve_site.rs`, `main.rs` (MODIFY): remove the now-redundant standalone `frontmatter::lint(&src)` loops (front-matter warnings now arrive via `doc.warnings`).
- `corpus/**` (MODIFY): purge every Quarto-only key (no-op edits).
- `corpus/diagnostics/typos.qmd` (NEW): the pin doc exercising all three validators.
- `crates/core/tests/corpus.rs` (MODIFY): update the front-matter cleanliness test; add the render-based all-surface guard.
- `crates/core/tests/nested_validation.rs` (NEW): the exact-warning pin test.

---

### Task 1: Corpus hygiene — purge non-qmd-fast keys (no-op edits, output unchanged)

This lands FIRST so the validators in Tasks 2-4 hit an already-clean corpus and the regression net stays green. Every key removed here is **unread by qmd-fast** (verified: `fig-width`/`fig-height`/`message`/`warning` are never honored; `title-block-banner`/`site-url`/`sort-ui`/`filter-ui`/`feed`/`fields` are never read by the listing/front-matter parsers), and cell-option lines are stripped by `strip_cell_options` before emission regardless, so removing them produces byte-identical rendered HTML.

**Files:** see the per-edit list below (all under `corpus/`).

- [ ] **Step 1: Snapshot an affected doc's output (to prove the edits are no-ops)**

```bash
cargo run -p qmd-fast-server -- render corpus/bayesian-book/subsections/_data-description.qmd > /tmp/qmd_before_datadesc.html 2>/dev/null; echo "snapshot ok"
```

(`render` runs the full pipeline; if the cell needs a kernel it renders as source, which is fine for a structural diff.)

- [ ] **Step 2: Remove non-recognized CELL OPTIONS (`message`, `fig-width`, `fig-height`)**

Delete these exact lines (the `#| message:` / `#| fig-width:` / `#| fig-height:` option lines):
- `corpus/posts/em-algorithm/index.qmd`: line 161 `#| message: false`
- `corpus/tech-blog/posts/em-algorithm/index.qmd`: line 161 `#| message: false` (this file is byte-identical to the one above)
- `corpus/bayesian-book/subsections/_partial-pooling-model.qmd`: the `#| fig-width:` and `#| fig-height:` lines (at 34, 35, 49, 50)
- `corpus/bayesian-book/subsections/_data-description.qmd`: the `#| fig-width:` and `#| fig-height:` lines (at 94, 95, 123, 124, 148, 149, 241, 242)
- `corpus/bayesian-book/subsections/_no-pooling-model.qmd`: the `#| fig-width:` and `#| fig-height:` lines (at 74, 75)

Use Edit per line (the surrounding `#| label:` / `#| echo:` lines stay). After editing, confirm none remain:

```bash
grep -rnE '^\s*#\|\s*(message|fig-width|fig-height)\s*:' corpus --include='*.qmd' || echo "no non-recognized cell options remain"
```

- [ ] **Step 3: Remove non-recognized `execute:` sub-keys (`warning`, `message`)**

In `corpus/bayesian-book/index.qmd`, the `execute:` block is:

```yaml
execute:
  warning: false
  message: false
  cache: true
```

Delete the `  warning: false` and `  message: false` lines, leaving:

```yaml
execute:
  cache: true
```

- [ ] **Step 4: Remove non-recognized TOP-LEVEL keys (`title-block-banner`, `site-url`)**

Delete these lines (keep `title-block-style: none` everywhere it appears, it IS honored):
- `corpus/tech-blog/index.qmd`: line 8 `title-block-banner: false`
- `corpus/tech-blog/blog.qmd`: line 14 `title-block-banner: false`
- `corpus/tech-blog/projects.qmd`: line 13 `title-block-banner: false`
- `corpus/tech-blog/cv.qmd`: line 5 `title-block-banner: false`
- `corpus/bayesian-book/index.qmd`: line 6 `site-url: https://ajbogo9.github.io/bayesian-fatality-analysis`

Confirm:

```bash
grep -rnE '^(title-block-banner|site-url):' corpus --include='*.qmd' || echo "no non-recognized top-level keys remain"
```

- [ ] **Step 5: Remove non-recognized `listing:` sub-keys (`sort-ui`, `filter-ui`, `feed`, `fields`)**

Delete these indented sub-key lines from each `listing:` block:
- `corpus/tech-blog/blog.qmd`: `  sort-ui: false`, `  filter-ui: false`, `  feed: true`, `  fields: [image, title, description, date, categories]`
- `corpus/tech-blog/projects.qmd`: `  sort-ui: false`, `  filter-ui: false`, `  fields: [image, title, description, categories]`
- `corpus/tech-blog/index.qmd`: `  fields: [image, title, description, date, categories]`
- `corpus/tech-blog/cv.qmd`: `    fields: [title, description, categories, date]`, `    sort-ui: false`, `    filter-ui: false` (note the deeper indent: these are under a sequence item `- id: cv-projects`)

Keep `contents`, `id`, `sort`, `type`, `max-items`, `categories`. Confirm:

```bash
grep -rnE '^\s*(sort-ui|filter-ui|feed|fields)\s*:' corpus --include='*.qmd' || echo "no non-recognized listing keys remain"
```

- [ ] **Step 6: Verify output is unchanged and the corpus still renders**

```bash
cargo run -p qmd-fast-server -- render corpus/bayesian-book/subsections/_data-description.qmd > /tmp/qmd_after_datadesc.html 2>/dev/null
diff /tmp/qmd_before_datadesc.html /tmp/qmd_after_datadesc.html && echo "OUTPUT IDENTICAL (edits are no-ops)" || echo "OUTPUT CHANGED — investigate"
cargo test -p qmd-fast-core 2>&1 | grep -E 'test result:' | grep -vE '0 failed' && echo FAILURES || echo "core green"
```

Expected: `OUTPUT IDENTICAL` and `core green`.

- [ ] **Step 7: Commit**

```bash
git add corpus
git commit -m "chore(corpus): purge Quarto-only keys qmd-fast does not honor (no-op cleanup)"
```

---

### Task 2: Cell-option validator (`validate.rs` + render-loop hook)

**Files:**
- Create: `crates/core/src/render/validate.rs`
- Modify: `crates/core/src/render/mod.rs` (declare `mod validate;`; call the validator in the render loop)
- Modify: `crates/core/src/frontmatter.rs` (add the shared `unknown_key_message` helper)
- Test: in-file `#[cfg(test)]` in `validate.rs`; new `crates/core/tests/cell_option_validation.rs`

**Interfaces:**
- Produces: `render::validate::CELL_OPTION_KEYS: &[&str]`; `render::validate::CALLOUT_KINDS: &[&str]` (used in Task 3); `render::validate::cell_option_keys(literal: &str) -> Vec<(String, usize)>`; `render::validate::validate_cell_options(literal: &str, fence_line: usize, file: Option<String>) -> Vec<Warning>`; `render::validate::validate_callout_kind(kind: &str, line: usize, file: Option<String>) -> Option<Warning>` (used in Task 3).
- Produces: `crate::frontmatter::unknown_key_message(what: &str, key: &str, candidates: &[&'static str]) -> String` (the single shared message format, reused by Tasks 3 and 4).
- Consumes: `crate::frontmatter::closest(key, candidates) -> Option<&'static str>` (already `pub(crate)`), `super::Warning`.

- [ ] **Step 1: Add the shared `unknown_key_message` helper to `frontmatter.rs`**

In `crates/core/src/frontmatter.rs`, immediately after the `closest` function (around line 142), add:

```rust
/// Build an "unknown <what> `<key>`" message, appending "(did you mean `X`?)" when a
/// known candidate is within edit distance 2. The single message format shared by the
/// front-matter, cell-option, callout, and nested-config validators.
pub(crate) fn unknown_key_message(what: &str, key: &str, candidates: &[&'static str]) -> String {
    match closest(key, candidates) {
        Some(s) => format!("unknown {what} `{key}` (did you mean `{s}`?)"),
        None => format!("unknown {what} `{key}`"),
    }
}
```

- [ ] **Step 2: Write the failing unit tests for the cell-option validator**

Create `crates/core/src/render/validate.rs` with ONLY the test module first (so the test fails to compile, proving the functions are absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_only_the_leading_option_block() {
        let lit = "#| echo: false\n#| labl: x\nprint(1)\n#| late: y\n";
        let keys: Vec<_> = cell_option_keys(lit).into_iter().map(|(k, i)| (k, i)).collect();
        assert_eq!(keys, vec![("echo".to_string(), 0), ("labl".to_string(), 1)]);
    }

    #[test]
    fn flags_unknown_cell_option_with_did_you_mean_and_location() {
        // Fence is on file line 20, so the option on body line 1 is file line 22.
        let w = validate_cell_options("#| echo: false\n#| labl: x\n", 20, Some("p.qmd".into()));
        assert_eq!(w.len(), 1, "only `labl` is unknown, got: {w:?}");
        assert_eq!(
            w[0].message,
            "unknown cell option `labl` (did you mean `label`?)"
        );
        assert_eq!(w[0].file.as_deref(), Some("p.qmd"));
        assert_eq!(w[0].line, Some(22));
    }

    #[test]
    fn recognized_cell_options_are_silent() {
        let lit = "#| echo: false\n#| label: fig-x\n#| fig-cap: A\n#| code-fold: true\n//| name: n\n";
        assert!(validate_cell_options(lit, 1, None).is_empty(), "all keys recognized");
    }

    #[test]
    fn unknown_callout_kind_is_flagged_and_located() {
        let w = validate_callout_kind("importnat", 7, None).expect("an unknown-kind warning");
        assert_eq!(w.message, "unknown callout kind `importnat` (did you mean `important`?)");
        assert_eq!(w.line, Some(7));
        assert!(validate_callout_kind("note", 7, None).is_none(), "note is recognized");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p qmd-fast-core --lib validate 2>&1 | tail -20`
Expected: compile failure (`cell_option_keys` / `validate_cell_options` / `validate_callout_kind` not found).

- [ ] **Step 4: Implement `validate.rs`**

Prepend the implementation above the test module in `crates/core/src/render/validate.rs`:

```rust
//! qmd-fast's closed body vocabularies + did-you-mean validation for code-cell `#|`
//! options and `:::` callout kinds. Front-matter keys are validated in
//! `crate::frontmatter`; site-config keys in `crate::site::config`. Every validator is
//! purely diagnostic: an unrecognized key still renders exactly as before, plus one
//! located [`Warning`] (click-to-source in the dev panel).
//!
//! The vocabularies are qmd-fast's OWN, defined independently of Quarto. A key not in
//! the relevant set, whether a typo or a Quarto term qmd-fast does not implement, is
//! reported as unknown (with the closest known key when within edit distance 2). This
//! is deliberate: qmd-fast is its own tool, not a Quarto runtime.

use super::Warning;
use crate::frontmatter::unknown_key_message;

/// Cell options qmd-fast recognizes on a code cell's leading `#|` / `//|` / `%%|`
/// lines (the union across all cell languages; each is read in `cell_option` /
/// `parse_js_opts` / `code_fold` / `emit::code_line_numbers`).
pub(crate) const CELL_OPTION_KEYS: &[&str] = &[
    "echo",
    "include",
    "cache",
    "label",
    "fig-cap",
    "lst-cap",
    "tbl-cap",
    "fig-export",
    "code-fold",
    "code-summary",
    "code-line-numbers",
    "name",   // {js}
    "viewof", // {js}
    "input",  // {js}
];

/// Callout kinds qmd-fast recognizes (`::: {.callout-<kind>}`).
pub(crate) const CALLOUT_KINDS: &[&str] = &["note", "tip", "warning", "important", "caution"];

/// Enumerate a cell's leading option keys with each key's 0-based line offset within
/// `literal` (the fence body). Mirrors `cell_option`'s scan: only the contiguous
/// leading `#|` / `//|` / `%%|` block, stopping at the first code line.
pub(crate) fn cell_option_keys(literal: &str) -> Vec<(String, usize)> {
    let mut keys = Vec::new();
    for (i, line) in literal.lines().enumerate() {
        let t = line.trim_start();
        let Some(opt) = t
            .strip_prefix("#|")
            .or_else(|| t.strip_prefix("//|"))
            .or_else(|| t.strip_prefix("%%|"))
        else {
            break;
        };
        if let Some((k, _)) = opt.split_once(':') {
            keys.push((k.trim().to_string(), i));
        }
    }
    keys
}

/// Validate a code cell's `#|` options against [`CELL_OPTION_KEYS`]. `fence_line` is
/// the 1-based source line of the cell's opening fence (in `file`'s coordinates); an
/// option on the cell's i-th body line is at `fence_line + 1 + i`.
pub(crate) fn validate_cell_options(
    literal: &str,
    fence_line: usize,
    file: Option<String>,
) -> Vec<Warning> {
    cell_option_keys(literal)
        .into_iter()
        .filter(|(k, _)| !CELL_OPTION_KEYS.contains(&k.as_str()))
        .map(|(k, offset)| {
            let line = (fence_line + 1 + offset) as u32;
            Warning::new(unknown_key_message("cell option", &k, CELL_OPTION_KEYS)).at(file.clone(), line)
        })
        .collect()
}

/// Validate a callout kind (the `<kind>` in `.callout-<kind>`) against
/// [`CALLOUT_KINDS`]. `line` is the 1-based source line of the div's opening fence.
pub(crate) fn validate_callout_kind(kind: &str, line: usize, file: Option<String>) -> Option<Warning> {
    (!CALLOUT_KINDS.contains(&kind))
        .then(|| Warning::new(unknown_key_message("callout kind", kind, CALLOUT_KINDS)).at(file, line as u32))
}
```

- [ ] **Step 5: Declare the module and run the unit tests**

In `crates/core/src/render/mod.rs`, add `mod validate;` alongside the other submodule declarations (next to `mod divs;` around line 50).

Run: `cargo test -p qmd-fast-core --lib validate 2>&1 | tail -20`
Expected: the four `validate::tests` PASS. (Clippy may warn that `CALLOUT_KINDS` / `validate_callout_kind` are unused until Task 3; that is fine for `cargo test`. If `-D warnings` clippy is run now it will flag dead code, so do not run clippy until Task 3 wires the callout path. The full clippy gate is in Task 3's Step 5 and the final gate.)

- [ ] **Step 6: Write the failing integration test for the render-loop hook**

Create `crates/core/tests/cell_option_validation.rs`:

```rust
mod common;
use common::TempProj;

/// A typo'd cell option produces a located, click-to-source warning; the cell still
/// renders. (No kernel needed: the cell renders as source, and validation runs in the
/// render pass regardless of execution.)
#[test]
fn typo_cell_option_warns_with_location() {
    let proj = TempProj::new();
    let src = "# Title\n\nIntro.\n\n```{python}\n#| eccho: false\nprint(1)\n```\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &proj.0);
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("`eccho`"))
        .expect("a warning for the misspelled cell option");
    assert_eq!(w.message, "unknown cell option `eccho` (did you mean `echo`?)");
    // The fence ```{python} is on line 5, so the option (next line) is line 6.
    assert_eq!(w.line, Some(6), "got: {w:?}");
}

/// A cell using only recognized options is silent.
#[test]
fn recognized_cell_options_do_not_warn() {
    let proj = TempProj::new();
    let src = "# T\n\n```{python}\n#| echo: false\n#| label: fig-x\n#| fig-cap: Cap\nprint(1)\n```\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &proj.0);
    assert!(
        !doc.warnings.iter().any(|w| w.message.contains("cell option")),
        "no cell-option warnings expected, got: {:?}",
        doc.warnings
    );
}
```

Run: `cargo test -p qmd-fast-core --test cell_option_validation`
Expected: FAIL (`typo_cell_option_warns_with_location`: no such warning yet, the render loop does not call the validator).

- [ ] **Step 7: Wire the validator into the render loop**

In `crates/core/src/render/mod.rs`, inside `render_internal_impl`'s per-node block, the code cell is detected and `cell` / `cell_role` are built inside the borrow scope, just before the tuple is returned (the tuple starts around line 302 with `(sp.start.line, ...)`). `file` and `start_line` are already bound (around line 237: `let (file, start_line) = map_origin(origins, sp.start.line);`). Insert this statement AFTER `let cell_role = match … ;` (around line 301) and BEFORE the tuple expression:

```rust
            // Validate this code cell's `#|` options against qmd-fast's vocabulary
            // (a typo or a Quarto-only key becomes a located, click-to-source warning;
            // the cell still renders unchanged).
            if cell.is_some()
                && let NodeValue::CodeBlock(cb) = &data.value
            {
                warnings.extend(validate::validate_cell_options(&cb.literal, start_line, file.clone()));
            }
```

(`file.clone()` is used because `file` is moved into the tuple on the next line. `start_line` is the cell fence's originating file line, so the option location resolves in the correct file even for `{{< include >}}`d cells.)

- [ ] **Step 8: Run the integration test**

Run: `cargo test -p qmd-fast-core --test cell_option_validation`
Expected: both PASS.

- [ ] **Step 9: Gate (fmt + clippy + core tests)**

Run: `cargo test -p qmd-fast-core 2>&1 | grep -E 'test result:' | grep -vE '0 failed' && echo FAILURES || echo "core green"`
Then: `cargo fmt --all -- --check`
Expected: `core green`, fmt clean. (Defer full `-D warnings` clippy to Task 3, which removes the temporary dead-code on `CALLOUT_KINDS`.)

- [ ] **Step 10: Commit**

```bash
git add crates/core/src/render/validate.rs crates/core/src/render/mod.rs crates/core/src/frontmatter.rs crates/core/tests/cell_option_validation.rs
git commit -m "feat(validate): warn on unknown #| cell options with did-you-mean (click-to-source)"
```

---

### Task 3: Callout-kind validator (thread warnings through `group_divs` / `build_container`)

**Files:**
- Modify: `crates/core/src/render/divs.rs` (`group_divs` + `build_container` signatures; validate the kind in the callout arm)
- Modify: `crates/core/src/render/mod.rs` (pass `&mut warnings` to the `group_divs` call)
- Test: new `crates/core/tests/callout_kind_validation.rs`

**Interfaces:**
- Changes: `group_divs(flat, spans, origins, counts, warnings: &mut Vec<Warning>) -> Vec<Block>`; `build_container(span, inner, origins, counts, warnings: &mut Vec<Warning>) -> Block`.
- Consumes: `super::validate::validate_callout_kind` (Task 2), `super::Warning` (via `use super::*`).

- [ ] **Step 1: Write the failing integration test**

Create `crates/core/tests/callout_kind_validation.rs`:

```rust
mod common;
use common::TempProj;

/// An unknown callout kind warns (located at the div's opening fence) and still
/// renders with its given class (no render change).
#[test]
fn unknown_callout_kind_warns_and_still_renders() {
    let proj = TempProj::new();
    let src = "# T\n\nIntro.\n\n::: {.callout-importnat}\nBody.\n:::\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &proj.0);
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("callout kind"))
        .expect("a warning for the unknown callout kind");
    assert_eq!(w.message, "unknown callout kind `importnat` (did you mean `important`?)");
    assert_eq!(w.line, Some(5), "located at the opening fence line, got: {w:?}");
    // Render is unchanged: the class is still emitted verbatim.
    assert!(doc.body_html().contains("callout-importnat"), "callout still renders");
}

/// A recognized callout kind is silent.
#[test]
fn recognized_callout_kind_does_not_warn() {
    let proj = TempProj::new();
    let src = "# T\n\n::: {.callout-tip}\nUse the thing.\n:::\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &proj.0);
    assert!(
        !doc.warnings.iter().any(|w| w.message.contains("callout kind")),
        "no callout-kind warning expected, got: {:?}",
        doc.warnings
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p qmd-fast-core --test callout_kind_validation`
Expected: FAIL (`unknown_callout_kind_warns_and_still_renders`: no warning yet).

- [ ] **Step 3: Thread `&mut Vec<Warning>` through `group_divs` and `build_container`**

In `crates/core/src/render/divs.rs`:

Change the `group_divs` signature (around line 215) to add the parameter:

```rust
pub(crate) fn group_divs(
    flat: Vec<FlatBlock>,
    spans: &[DivSpan],
    origins: Option<&[LineOrigin]>,
    counts: &mut HashMap<String, u32>,
    warnings: &mut Vec<Warning>,
) -> Vec<Block> {
```

Pass `warnings` at both `build_container` call sites inside `group_divs` (around lines 259 and 268):

```rust
                let container = build_container(done.span, done.inner, origins, counts, warnings);
```

```rust
        let container = build_container(done.span, done.inner, origins, counts, warnings);
```

Change the `build_container` signature (around line 276) to add the parameter:

```rust
fn build_container(
    span: &DivSpan,
    mut inner: Vec<Block>,
    origins: Option<&[LineOrigin]>,
    counts: &mut HashMap<String, u32>,
    warnings: &mut Vec<Warning>,
) -> Block {
```

In `build_container`'s callout arm (the `if let Some(kind) = attrs.callout_kind() {` at line 291), validate the kind as the FIRST statement inside the arm, before building `title`/`body`/`html` (which are all unchanged). `file` and `open_line` are already bound around line 284 (`let (file, open_line) = map_origin(origins, span.open);`):

```rust
    let html = if let Some(kind) = attrs.callout_kind() {
        // Validate the kind against qmd-fast's callout vocabulary (an unknown kind
        // warns, click-to-source, and still renders with its given class).
        if let Some(w) = super::validate::validate_callout_kind(kind, open_line, file.clone()) {
            warnings.push(w);
        }
        // Callout: use a `title="..."` attr, else a leading heading, else the kind.
        let title = match attrs.get("title") {
```

(`file.clone()` because `file` is moved into the returned `Block { … source_file: file, … }`.)

- [ ] **Step 4: Update the `group_divs` call in `mod.rs`**

In `crates/core/src/render/mod.rs` (around line 520):

```rust
    let mut blocks = group_divs(flat, &spans, origins, &mut id_counts, &mut warnings);
```

- [ ] **Step 5: Run the test and the full clippy gate**

Run: `cargo test -p qmd-fast-core --test callout_kind_validation`
Expected: both PASS.
Then: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean (the `CALLOUT_KINDS` / `validate_callout_kind` dead-code from Task 2 is now used).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/divs.rs crates/core/src/render/mod.rs crates/core/tests/callout_kind_validation.rs
git commit -m "feat(validate): warn on unknown callout kinds with did-you-mean (click-to-source)"
```

---

### Task 4: Front-matter validator (reconcile `KNOWN_KEYS` + nested config + located + wiring)

**Files:**
- Modify: `crates/core/src/frontmatter.rs` (reconcile `KNOWN_KEYS`; add nested key sets; replace `lint` with `validate_front_matter` returning located `Vec<Warning>`; located helpers; update in-file tests)
- Modify: `crates/core/src/render/mod.rs` (call `validate_front_matter(src)` once)
- Modify: `crates/server/src/serve.rs`, `crates/server/src/serve_site.rs`, `crates/server/src/main.rs` (remove the now-redundant standalone `frontmatter::lint(&src)` loops)
- Modify: `crates/core/tests/corpus.rs` (update `every_corpus_doc_has_clean_front_matter` to use `validate_front_matter`, skip a `diagnostics` dir)

**Interfaces:**
- Changes: `frontmatter::lint(src: &str) -> Vec<String>` is REMOVED; replaced by `frontmatter::validate_front_matter(src: &str) -> Vec<Warning>` (top-level + nested `execute:`/`listing:`/`about:`/`hero:` immediate children, each best-effort located).
- Produces: `frontmatter::EXECUTE_KEYS`, `LISTING_KEYS`, `ABOUT_KEYS`, `HERO_KEYS` (private consts).
- Consumes: `crate::render::model::Warning` (imported in `frontmatter.rs`), the Task 2 `unknown_key_message`, the existing `front_matter_block` / `closest`.

- [ ] **Step 1: Reconcile `KNOWN_KEYS` (drop the two genuinely-unhonored Quarto keys; fix the stale comment)**

In `crates/core/src/frontmatter.rs`, `KNOWN_KEYS` (around lines 10-53). `include-in-header` / `include-before-body` / `include-after-body` (read by `render::resolve_doc_includes`) and `title-block-style` (read by `render::detect_title_block_hidden`) ARE honored, so they stay but move out of the "tolerated but not honored" group. `title-block-banner` and `site-url` are read by NO code, so remove them. Replace the block of lines from `    // Output / format / theme` through `    "site-url", // …` with:

```rust
    // Output / format / theme
    "format",
    "theme",
    "css",
    "extensions",
    "page-layout",
    // Title block: `title-block-style: none` is honored (suppresses the visible
    // header); see `render::detect_title_block_hidden`.
    "title-block-style",
    // Per-document head/body injection, honored by `render::resolve_doc_includes`.
    "include-in-header",
    "include-before-body",
    "include-after-body",
    // Table of contents
    "toc",
    // Citations
    "bibliography",
    "csl",
    // Execution
    "execute",
    // Listings / project pages
    "listing",
    "about",
    "hero",
```

Also update the top-of-file doc comment so it no longer says nested keys "are not" linted (around lines 13-14): change "Only top-level keys are linted; nested keys (under `format:`, `execute:`, `about:`, `listing:`, `hero:`) are not." to "Top-level keys plus the immediate children of `execute:` / `listing:` / `about:` / `hero:` are linted; `format:` sub-keys are not (an extension owns them)."

- [ ] **Step 2: Add the nested key sets**

In `crates/core/src/frontmatter.rs`, after `KNOWN_KEYS` (around line 53), add qmd-fast's nested vocabularies (the immediate children each parser reads):

```rust
/// `execute:` sub-keys qmd-fast honors (document-level cell defaults; see
/// `render::detect_execute_defaults`).
const EXECUTE_KEYS: &[&str] = &["echo", "include", "cache"];

/// `listing:` sub-keys qmd-fast honors (see `site::frontmatter::parse_listing_spec`).
const LISTING_KEYS: &[&str] = &["contents", "id", "sort", "type", "max-items", "categories"];

/// `about:` sub-keys qmd-fast honors (see `site::frontmatter::parse_about`).
const ABOUT_KEYS: &[&str] = &["template", "image", "image-alt", "links"];

/// `hero:` sub-keys qmd-fast honors (see `site::frontmatter::parse_hero`).
const HERO_KEYS: &[&str] = &["eyebrow", "headline", "lead", "actions"];
```

- [ ] **Step 3: Add the `Warning` import**

At the top of `crates/core/src/frontmatter.rs`, add:

```rust
use crate::render::model::Warning;
```

- [ ] **Step 4: Write the failing unit tests for `validate_front_matter`**

Replace the in-file `mod tests` block of `frontmatter.rs` so the `lint`-based tests become `validate_front_matter`-based and new nested cases are added. The full replacement `#[cfg(test)] mod tests` is:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn msgs(src: &str) -> Vec<String> {
        validate_front_matter(src).into_iter().map(|w| w.message).collect()
    }

    #[test]
    fn flags_top_level_typo_with_suggestion_and_location() {
        let w = validate_front_matter("---\ntreme: darkly\ntitle: X\n---\n\nbody\n");
        assert_eq!(w.len(), 1, "got: {w:?}");
        assert_eq!(w[0].message, "unknown front-matter key `treme` (did you mean `theme`?)");
        assert_eq!(w[0].line, Some(2), "`treme` is on file line 2");
    }

    #[test]
    fn flags_unknown_execute_child() {
        let m = msgs("---\ntitle: X\nexecute:\n  eccho: false\n  cache: true\n---\n");
        assert_eq!(m, vec!["unknown execute key `eccho` (did you mean `echo`?)"]);
    }

    #[test]
    fn flags_unknown_listing_child_in_a_mapping_and_a_sequence() {
        let m = msgs("---\ntitle: X\nlisting:\n  contents: posts\n  max-itemz: 3\n---\n");
        assert_eq!(m, vec!["unknown listing key `max-itemz` (did you mean `max-items`?)"]);
        // A sequence of listings (cv.qmd shape) validates each item.
        let m2 = msgs("---\ntitle: X\nlisting:\n  - contents: a\n    sort-uii: false\n---\n");
        assert_eq!(m2, vec!["unknown listing key `sort-uii`"]);
    }

    #[test]
    fn flags_unknown_about_and_hero_children() {
        let a = msgs("---\ntitle: X\nabout:\n  template: jolla\n  imagee: me.png\n---\n");
        assert_eq!(a, vec!["unknown about key `imagee` (did you mean `image`?)"]);
        let h = msgs("---\ntitle: X\nhero:\n  headlin: Hi\n---\n");
        assert_eq!(h, vec!["unknown hero key `headlin` (did you mean `headline`?)"]);
    }

    #[test]
    fn clean_doc_with_nested_blocks_has_no_warnings() {
        let w = validate_front_matter(
            "---\ntitle: X\ntoc: true\nexecute:\n  echo: false\n  cache: true\nlisting:\n  contents: posts\n  type: grid\nabout:\n  template: jolla\n  links:\n    - text: GH\n      href: https://x\n---\n\nx\n",
        );
        assert!(w.is_empty(), "got: {w:?}");
    }

    #[test]
    fn format_subkeys_are_not_linted() {
        // `format:` is owned by extensions; its children must not warn.
        let w = validate_front_matter("---\ntitle: X\nformat:\n  html:\n    toc: true\n    anything: 1\n---\n");
        assert!(w.is_empty(), "got: {w:?}");
    }

    #[test]
    fn invalid_yaml_yields_no_lint_warnings() {
        // The YAML parse error is reported separately by `yaml_error`.
        assert!(validate_front_matter("---\ntitle: X\n: : :\n---\n").is_empty());
    }

    #[test]
    fn no_front_matter_yields_no_warnings() {
        assert!(validate_front_matter("# Heading\n\ntext\n").is_empty());
        assert!(validate_front_matter("").is_empty());
    }

    #[test]
    fn dropped_quarto_keys_now_warn() {
        let m = msgs("---\ntitle: X\ntitle-block-banner: false\nsite-url: https://x\n---\n");
        assert!(m.iter().any(|w| w.contains("`title-block-banner`")), "got: {m:?}");
        assert!(m.iter().any(|w| w.contains("`site-url`")), "got: {m:?}");
    }

    #[test]
    fn honored_keys_do_not_warn() {
        let w = validate_front_matter(
            "---\ntitle: X\ntitle-block-style: none\ninclude-in-header:\n  text: \"<meta>\"\n---\n",
        );
        assert!(w.is_empty(), "honored keys must not warn, got: {w:?}");
    }

    // The YAML-parse-error locator is unchanged.
    #[test]
    fn yaml_error_reports_the_file_line() {
        let (msg, line) = yaml_error("---\ntitle: ok\nbad: : x\n---\n\nbody\n").expect("an error");
        assert!(msg.contains("not valid YAML"), "got: {msg}");
        assert_eq!(line, 3);
    }

    #[test]
    fn yaml_error_none_when_valid_or_absent() {
        assert!(yaml_error("---\ntitle: X\n---\n\nbody\n").is_none());
        assert!(yaml_error("no front matter\n").is_none());
    }
}
```

Run: `cargo test -p qmd-fast-core --lib frontmatter 2>&1 | tail -20`
Expected: compile failure (`validate_front_matter` not defined yet).

- [ ] **Step 5: Replace `lint` with `validate_front_matter` + location helpers**

In `crates/core/src/frontmatter.rs`, DELETE the existing `pub fn lint(...)` (around lines 55-83) and the `closest_known` helper (around lines 127-130, it was only used by `lint`). Add, in their place (keep `closest`, `levenshtein`, `yaml_error`, `front_matter_block`, `unknown_key_message`):

```rust
/// Validate a document's front matter against qmd-fast's vocabulary: every unknown
/// top-level key, plus every unknown immediate child of the nested `execute:`,
/// `listing:`, `about:`, and `hero:` blocks. Membership is decided by a real YAML
/// parse (so structure, lists, nested maps, never causes a false positive); each
/// warning is best-effort located (click-to-source) at the offending key's source
/// line. Empty when there is no front matter, it is not a mapping, or it fails to
/// parse (the parse error is reported separately by [`yaml_error`]).
pub fn validate_front_matter(src: &str) -> Vec<Warning> {
    let Some(block) = front_matter_block(src) else {
        return Vec::new();
    };
    if block.trim().is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(block) else {
        return Vec::new();
    };
    let Some(map) = value.as_mapping() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for key in map.keys().filter_map(|k| k.as_str()) {
        if !KNOWN_KEYS.contains(&key) {
            let line = block_key_line(block, key);
            out.push(located(unknown_key_message("front-matter key", key, KNOWN_KEYS), line));
        }
    }
    validate_nested(map, "execute", "execute key", EXECUTE_KEYS, block, &mut out);
    validate_nested(map, "about", "about key", ABOUT_KEYS, block, &mut out);
    validate_nested(map, "hero", "hero key", HERO_KEYS, block, &mut out);
    // `listing:` is one mapping or a sequence of mappings (cv.qmd).
    match map.get("listing") {
        Some(serde_yaml::Value::Mapping(m)) => {
            validate_child_keys(m, "listing", "listing key", LISTING_KEYS, block, &mut out)
        }
        Some(serde_yaml::Value::Sequence(seq)) => {
            for item in seq {
                if let Some(m) = item.as_mapping() {
                    validate_child_keys(m, "listing", "listing key", LISTING_KEYS, block, &mut out);
                }
            }
        }
        _ => {}
    }
    out
}

/// A `Warning` for `message`, located when `line` is `Some` (file `None` = the
/// previewed doc, the client falls back to its path).
fn located(message: String, line: Option<u32>) -> Warning {
    match line {
        Some(l) => Warning::new(message).at(None, l),
        None => Warning::new(message),
    }
}

/// Validate the immediate children of a single nested mapping block.
fn validate_nested(
    map: &serde_yaml::Mapping,
    parent: &str,
    what: &str,
    allowed: &[&'static str],
    block: &str,
    out: &mut Vec<Warning>,
) {
    if let Some(serde_yaml::Value::Mapping(m)) = map.get(parent) {
        validate_child_keys(m, parent, what, allowed, block, out);
    }
}

fn validate_child_keys(
    m: &serde_yaml::Mapping,
    parent: &str,
    what: &str,
    allowed: &[&'static str],
    block: &str,
    out: &mut Vec<Warning>,
) {
    for key in m.keys().filter_map(|k| k.as_str()) {
        if !allowed.contains(&key) {
            let line = nested_key_line(block, parent, key);
            out.push(located(unknown_key_message(what, key, allowed), line));
        }
    }
}

/// The 1-based SOURCE-FILE line of a top-level front-matter key (best-effort). The
/// block starts on the file line after the opening `---`, so block line index `i` is
/// file line `i + 2`. `None` if the key is not on its own line (e.g. a flow mapping).
fn block_key_line(block: &str, key: &str) -> Option<u32> {
    block.lines().enumerate().find_map(|(i, line)| {
        let t = line.trim_start();
        (line.len() == t.len() && key_matches(t, key)).then_some(i as u32 + 2)
    })
}

/// The 1-based SOURCE-FILE line of an immediate child `key` under top-level
/// `parent:` (best-effort). Scans from `parent:` to the next indent-0 key, matching
/// `key:` at any indent (including a leading `- ` sequence item).
fn nested_key_line(block: &str, parent: &str, key: &str) -> Option<u32> {
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
            break; // dedent ends the parent block
        }
        let body = t.strip_prefix("- ").map(str::trim_start).unwrap_or(t);
        if key_matches(body, key) {
            return Some(i as u32 + 2);
        }
    }
    None
}

/// Does `text` start with `key` immediately followed by `:` (a YAML key)?
fn key_matches(text: &str, key: &str) -> bool {
    text.strip_prefix(key).is_some_and(|rest| rest.starts_with(':'))
}
```

Run: `cargo test -p qmd-fast-core --lib frontmatter 2>&1 | tail -30`
Expected: all `frontmatter::tests` PASS.

- [ ] **Step 6: Call `validate_front_matter` from the render pipeline**

In `crates/core/src/render/mod.rs`, in `render_internal_impl`, just after the warnings accumulator is created (around line 162, `let mut warnings: Vec<Warning> = Vec::new();`), add:

```rust
    // Validate the document's front matter against qmd-fast's vocabulary (top-level
    // keys + the nested execute/listing/about/hero children); located warnings flow to
    // the dev panel as click-to-source diagnostics, the same channel as broken refs.
    warnings.extend(crate::frontmatter::validate_front_matter(src));
```

(`src` is `render_internal_impl`'s input; the front matter is at the top, so its lines are unaffected by `{{< include >}}` splicing.)

- [ ] **Step 7: Remove the now-redundant server-side `lint` loops**

Front-matter warnings now arrive via `doc.warnings` (which Wave 1 already maps to clickable diagnostics in both servers and logs in `main.rs`). Remove the standalone `frontmatter::lint(&src)` loops so they do not double-report. Leave the `frontmatter::yaml_error` located-parse-error handling untouched.

- In `crates/server/src/serve.rs` (around line 768): delete the `for message in qmd_fast_core::frontmatter::lint(&src) { … }` loop that pushes diagnostics.
- In `crates/server/src/serve_site.rs` (around line 877): delete the equivalent `for message in qmd_fast_core::frontmatter::lint(&src) { … }` loop.
- In `crates/server/src/main.rs` (around lines 181 and 375): delete the two `for w in qmd_fast_core::frontmatter::lint(src/&src) { log::warn(…) }` loops.

Build the server to confirm no dangling references:
Run: `cargo build -p qmd-fast-server 2>&1 | tail -20`
Expected: builds (no `lint` references remain).

- [ ] **Step 8: Update the corpus front-matter cleanliness test**

In `crates/core/tests/corpus.rs`, the `every_corpus_doc_has_clean_front_matter` test (around lines 43-60) calls `frontmatter::lint`. Update it to use `validate_front_matter` (now returning `Vec<Warning>`), read `.message`, and skip a future `diagnostics` dir:

```rust
#[test]
fn every_corpus_doc_has_clean_front_matter() {
    // qmd-fast's front-matter validator must not warn on any real document: a warning
    // here means the allowlist is missing a key the corpus legitimately uses.
    // corpus/diagnostics/ is exempt (it deliberately holds typo'd keys).
    let mut files = Vec::new();
    collect_qmd(&corpus_dir(), &mut files);
    let mut offenders = Vec::new();
    for f in &files {
        if f.components().any(|c| c.as_os_str() == "diagnostics") {
            continue;
        }
        let src = fs::read_to_string(f).unwrap();
        for w in qmd_fast_core::frontmatter::validate_front_matter(&src) {
            let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
            offenders.push(format!("{label}: {}", w.message));
        }
    }
    assert!(
        offenders.is_empty(),
        "front-matter validator warned on corpus docs:\n{}",
        offenders.join("\n")
    );
}
```

- [ ] **Step 9: Full gate**

Run: `cargo test --workspace 2>&1 | grep -E 'test result:|error\[' | grep -vE '0 failed' && echo FAILURES || echo "all green"`
Then: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: `all green`, fmt clean, clippy clean. (Task 1 already purged the corpus, so `every_corpus_doc_has_clean_front_matter` passes with the reconciled `KNOWN_KEYS`.)

- [ ] **Step 10: Commit**

```bash
git add crates/core/src/frontmatter.rs crates/core/src/render/mod.rs crates/server/src/serve.rs crates/server/src/serve_site.rs crates/server/src/main.rs crates/core/tests/corpus.rs
git commit -m "feat(validate): located front-matter + nested execute/listing/about/hero key validation"
```

---

### Task 5: The pin doc + exact-warning test + corpus-wide guard

**Files:**
- Create: `corpus/diagnostics/typos.qmd`
- Create: `crates/core/tests/nested_validation.rs`
- Modify: `crates/core/tests/corpus.rs` (add the render-based all-surface guard)
- Modify: `corpus/README.md` (document the diagnostics dir)

**Interfaces:**
- Consumes: all three validators (cell options, callout kinds, front-matter nested) and `render_document_with_includes`.

- [ ] **Step 1: Create the pin doc**

Create `corpus/diagnostics/typos.qmd` (each misspelled key produces exactly one click-to-source warning; the doc renders fine):

```markdown
---
title: "Diagnostics: typo coverage"
treme: darkly
execute:
  eccho: false
listing:
  contents: posts
  max-itemz: 3
---

# Typo coverage

This document deliberately uses misspelled keys to pin qmd-fast's schema validators.
Each unknown key below produces one click-to-source warning; rendering is unaffected.

::: {.callout-importnat}
This callout names a kind qmd-fast does not define.
:::

```{python}
#| labl: fig-demo
print("hello from a cell whose option key is misspelled")
```
```

- [ ] **Step 2: Write the failing exact-warning pin test**

Create `crates/core/tests/nested_validation.rs`:

```rust
//! Pins qmd-fast's schema validators to corpus/diagnostics/typos.qmd: rendering it
//! must produce exactly the expected click-to-source "unknown key" warnings, one per
//! deliberately-misspelled key (front-matter top-level + nested, callout kind, cell
//! option). This is the corpus pin for the nested-schema-validation epic.
mod common;
use common::corpus_dir;

#[test]
fn typos_doc_warns_exactly_on_each_unknown_key() {
    let dir = corpus_dir().join("diagnostics");
    let src = std::fs::read_to_string(dir.join("typos.qmd")).unwrap();
    let doc = qmd_fast_core::render_document_with_includes(&src, &dir);
    let msgs: Vec<&str> = doc.warnings.iter().map(|w| w.message.as_str()).collect();

    let expected = [
        "unknown front-matter key `treme` (did you mean `theme`?)",
        "unknown execute key `eccho` (did you mean `echo`?)",
        "unknown listing key `max-itemz` (did you mean `max-items`?)",
        "unknown callout kind `importnat` (did you mean `important`?)",
        "unknown cell option `labl` (did you mean `label`?)",
    ];
    for e in expected {
        assert!(msgs.contains(&e), "missing warning:\n  {e}\ngot:\n{msgs:#?}");
    }
    // No EXTRA "unknown ..." warnings beyond the five pinned ones.
    let unknown = doc.warnings.iter().filter(|w| w.message.starts_with("unknown ")).count();
    assert_eq!(unknown, expected.len(), "unexpected unknown-key warnings:\n{msgs:#?}");

    // The body validators are click-to-source (located at the offending line).
    let cell = doc.warnings.iter().find(|w| w.message.contains("`labl`")).unwrap();
    assert!(cell.line.is_some(), "cell-option warning should be located: {cell:?}");
    let callout = doc.warnings.iter().find(|w| w.message.contains("`importnat`")).unwrap();
    assert!(callout.line.is_some(), "callout warning should be located: {callout:?}");
}
```

Run: `cargo test -p qmd-fast-core --test nested_validation`
Expected: PASS (all three validators from Tasks 2-4 fire on the pin doc).

- [ ] **Step 3: Add the render-based all-surface corpus guard**

In `crates/core/tests/corpus.rs`, add a new test that renders every corpus doc and asserts no "unknown …" validation warning (this catches cell-option and callout warnings, which the front-matter-only test does not), skipping the diagnostics pin:

```rust
#[test]
fn every_corpus_doc_emits_no_unknown_key_warnings() {
    // qmd-fast has its own closed vocabulary: every real corpus doc must use only
    // recognized cell options, callout kinds, and config keys, so the validators stay
    // silent. corpus/diagnostics/ is exempt (its exact warnings are pinned in
    // crates/core/tests/nested_validation.rs).
    let mut files = Vec::new();
    collect_qmd(&corpus_dir(), &mut files);
    let mut offenders = Vec::new();
    for f in &files {
        if f.components().any(|c| c.as_os_str() == "diagnostics") {
            continue;
        }
        let src = fs::read_to_string(f).unwrap();
        let base = f.parent().unwrap();
        let doc = qmd_fast_core::render_document_with_includes(&src, base);
        for w in doc.warnings.iter().filter(|w| w.message.starts_with("unknown ")) {
            let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
            offenders.push(format!("{label}: {}", w.message));
        }
    }
    assert!(
        offenders.is_empty(),
        "validator warned on corpus docs (clean the doc or extend the vocabulary):\n{}",
        offenders.join("\n")
    );
}
```

Run: `cargo test -p qmd-fast-core --test corpus`
Expected: PASS (Task 1 purged every Quarto-only key, so the guard is green).

- [ ] **Step 4: Document the diagnostics dir in the corpus README**

In `corpus/README.md`, add a short row/section noting the new directory. Match the file's existing format; the content to convey:

```markdown
- **diagnostics/** — docs that deliberately trip qmd-fast's schema validators
  (`typos.qmd`: a misspelled key in each surface, front-matter top-level + nested,
  callout kind, cell option). Pinned by `crates/core/tests/nested_validation.rs`,
  which asserts the exact click-to-source warnings, and exempted from the corpus
  "clean vocabulary" guards.
```

- [ ] **Step 5: Full gate**

Run: `cargo test --workspace 2>&1 | grep -E 'test result:|error\[' | grep -vE '0 failed' && echo FAILURES || echo "all green"`
Then: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: `all green`, fmt clean, clippy clean.

- [ ] **Step 6: Manual smoke check (preview the pin doc, confirm click-to-source)**

Run: `cargo run -p qmd-fast-server -- build corpus/diagnostics/typos.qmd /tmp/typos.html >/tmp/typos.log 2>&1; echo "--- build log ---"; cat /tmp/typos.log`
Expected: the build log lists each "unknown …" warning. (Optional: `qmd-fast preview corpus/diagnostics/typos.qmd` and confirm the dev panel shows five warnings, each clicking to the right source line.)

- [ ] **Step 7: Commit**

```bash
git add corpus/diagnostics/typos.qmd crates/core/tests/nested_validation.rs crates/core/tests/corpus.rs corpus/README.md
git commit -m "test(validate): pin corpus/diagnostics/typos.qmd + corpus-wide clean-vocabulary guard"
```

---

## Self-Review

**Spec coverage** (BEYOND-QUARTO.md Pillar I `nested-schema-validation`):
- "Closed key sets + `closest()` did-you-mean for `#|` / `//|` cell options" → Task 2 (`CELL_OPTION_KEYS`, `validate_cell_options`, render-loop hook; covers `#|`/`//|`/`%%|`).
- "callout kinds (`callout_kind` strips `callout-` off any class)" → Task 3 (`CALLOUT_KINDS`, `validate_callout_kind`, threaded through `build_container`; render unchanged).
- "nested config blocks (`execute:` / `listing:` / `about:` / `hero:` + front-matter sub-keys)" → Task 4 (`validate_front_matter`, the four nested sets, the reconciled `KNOWN_KEYS`).
- "Built on `locate-render-warnings` so every diagnostic is click-to-source" → all validators emit `Warning` with `line`/`file`; cell + callout are exactly located, front-matter is best-effort located; everything flows through `doc.warnings`, which Wave 1 maps to clickable diagnostics.
- "Pin: `corpus/diagnostics/typos.qmd` + a sibling test asserting exact warnings" → Task 5.
- "Closes the now-moot backlog P2" → the per-document config path is now validated against qmd-fast's own closed vocabulary.
- The author's clarifying directive ("totally leave Quarto behind; only my framework's keys are recognized; Quarto terminology is flagged as an error") supersedes the spec's "recognized-but-not-honored" sub-goal: there is a SINGLE closed set per surface, and Task 1 purges the corpus of every Quarto-only key so the regression net holds.

**Placeholder scan:** No TBD/TODO/"add validation"/"handle edge cases". Every code step shows complete code. Task 1's per-line deletions are enumerated with file + line. The one best-effort element (front-matter `block_key_line`/`nested_key_line` location) is fully implemented with a defined graceful fallback (`line: None` renders unlocated, never wrong), and the pin test asserts located-ness only for the exactly-located cell + callout surfaces.

**Type consistency:** `Warning` (message/file/line, `new`/`at`) is the Wave 1 type, used identically across all tasks. `unknown_key_message(what, key, candidates) -> String` is defined in Task 2 and consumed by Tasks 2-4. `validate_cell_options` / `validate_callout_kind` signatures defined in Task 2 match their call sites in Tasks 2-3. `group_divs` / `build_container` gain `warnings: &mut Vec<Warning>` consistently (Task 3). `frontmatter::lint -> Vec<String>` is fully removed and replaced by `validate_front_matter -> Vec<Warning>`; every caller (3 server files + corpus test) is updated in Task 4. The nested sets (`EXECUTE_KEYS`/`LISTING_KEYS`/`ABOUT_KEYS`/`HERO_KEYS`) match the keys the live parsers read (`detect_execute_defaults`, `parse_listing_spec`, `parse_about`, `parse_hero`).

**Scope check:** Five independently testable, independently committable tasks. Task 1 is a no-op corpus purge (verified output-identical). Tasks 2-4 each add one validation surface with TDD; the corpus stays green because Task 1 ran first. Task 5 pins the behavior. No block-model / diff / sourcepos / `:::`-output / cite / includes / numbering / exec change: every validator only appends a `Warning`. `Site::warnings` and `site::config::validate_keys` are untouched.

**Known follow-ups (out of scope, noted not silently dropped):** `jsonschema-for-config` (the next Wave 1 item) will generate a schema from these same consts. `callout-kind-contract` (Wave 3) will turn `CALLOUT_KINDS` into a render contract (icons/tokens, fall-back-to-note); this epic deliberately leaves rendering unchanged. `about:`/`hero:` deep-child validation (the keys inside `links:` / `actions:` items) is left for a later pass; only immediate children are validated here. The marketing `site/` and `docs/` trees are not in the corpus regression net; if they use purged Quarto keys they will now warn in preview (correct, per the clean-break directive) and can be cleaned on demand.
