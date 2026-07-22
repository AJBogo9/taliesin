# Structure-preserving, book-aware `taliesin read` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `taliesin read`'s text projection keep structured blocks separated (lists, scrolly/steps, input controls) and make `read` book-aware (chapter-scoped numbering + cross-page ref resolution, per-page and whole-directory).

**Architecture:** Two independent halves. (A) Three new pure arms in `crates/core/src/render/text.rs::project_block`, each HTML-in → text-out, before the generic `visible()` fallthrough. (B) `crates/server/src/query.rs` gains enclosing-site discovery so a single page auto-scopes like the site build (reusing `render_document_with_includes_scoped` + `Site::number_chapter` + `Site::resolve_cross_refs`, the exact sequence `site/search.rs::page_fragment` already uses), plus a whole-directory read.

**Tech Stack:** Rust (edition 2024), `taliesin-core` lib + `taliesin-server` bin, `serde`/`serde_json`, cargo test.

## Global Constraints

- **No em dashes / en dashes** in any authored prose, fixture, or test data (user rule). Use commas, colons, parentheses.
- **Block-model invariant:** every emitted block carries `data-block-id` + `data-sourcepos`. The new arms only READ `block.html`; they must not alter block structure.
- **`read` is a view, not an output format.** HTML stays the only build target. No new dependency.
- **A `rustfmt` PostToolUse hook runs on every edited `.rs` file**, so the tree stays `cargo fmt`-clean.
- **Three test gates** (CI sets all three): `TALIESIN_REQUIRE_NODE=1`, `TALIESIN_R=R TALIESIN_REQUIRE_R=1`, `TALIESIN_PYTHON=… TALIESIN_REQUIRE_KERNEL=1`. `cargo test` aborts remaining binaries at first failure; re-run before trusting a total. If an `exec` probe test flakes, `--test-threads=1` before blaming your change.
- **Branch:** `structure-preserving-read` (already created; the spec is committed there). Commit per task; do NOT push (author pushes).
- **Verify by mutation:** after each test passes, restore the bug and watch the named test fail.

Exact resolved numbers for the book fixture (`corpus/course/`, verified against source): `em.tmd` is **Chapter 3** (`@thm-elbo` → "Theorem 3.1"); `mle.tmd` is **Chapter 2** (`@thm-consistency` → "Theorem 2.1", `@sec-mle` → "Chapter 2"). `em.tmd:3` reads `Recall @thm-consistency from @sec-mle` and must project to `Recall Theorem 2.1 from Chapter 2`.

---

## Part A — Structure-preserving projection (`crates/core/src/render/text.rs`)

All three arms are inserted into `project_block` between the callout arm and the final
`let text = visible(html);` fallthrough (currently text.rs:128-130). Insertion anchor:

```rust
    // A callout: label it by kind, then its title (if any) and body on their own lines, so
    // an agent reads "[note] Heads up" instead of the title running into the body.
    if let Some(kind) = callout_kind(html) {
        return project_callout(html, kind);
    }

    // <<< NEW ARMS GO HERE (Tasks 1, 2, 3) >>>

    let text = visible(html);
```

### Task 1: List projection arm

**Files:**
- Modify: `crates/core/src/render/text.rs` (add arm in `project_block` + `project_list`/`top_level_li_inner`/`split_nested_list` helpers + unit tests)

**Interfaces:**
- Produces: `fn project_list(html: &str, indent: usize) -> String` (used only within text.rs)
- Consumes existing helpers in text.rs: `leading_tag(&str) -> Option<&str>`, `first_attr(&str, &str) -> Option<String>`, `visible(&str) -> String`.

- [ ] **Step 1: Write the failing tests** (append to `mod tests` in text.rs)

```rust
    #[test]
    fn projects_list_items_on_separate_lines() {
        let out = project_src("- **name**: the column to reference.\n- **Returns**: an `Expr`.\n");
        assert!(
            out.contains("- name: the column to reference."),
            "first item on its own line:\n{out}"
        );
        assert!(out.contains("- Returns: an Expr"), "second item separated:\n{out}");
        assert!(
            !out.contains("reference.Returns"),
            "adjacent list items must not fuse:\n{out}"
        );
    }

    #[test]
    fn projects_ordered_and_nested_lists() {
        let out = project_src("1. first\n2. second\n   - nested a\n   - nested b\n");
        assert!(out.contains("1. first"), "ordered marker:\n{out}");
        assert!(out.contains("2. second"), "ordered counts up:\n{out}");
        assert!(out.contains("  - nested a"), "nested item indented two spaces:\n{out}");
        assert!(out.contains("  - nested b"), "nested item indented two spaces:\n{out}");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p taliesin-core --lib render::text::tests::projects_list -- --nocapture`
Expected: FAIL (items fused, e.g. `reference.Returns`).

- [ ] **Step 3: Add the arm + helpers**

In `project_block`, at the NEW-ARMS anchor:

```rust
    // A list: project each top-level <li> on its own line so items don't run together
    // (`…reference.Returns —…`). Ordered lists count; a nested list indents two spaces.
    if matches!(leading_tag(html), Some("ul") | Some("ol")) {
        return project_list(html, 0);
    }
```

Add these helpers (near the other block helpers, e.g. after `project_callout`):

```rust
/// Project a `<ul>`/`<ol>` list block to one line per item, nested lists indented two
/// spaces per level. `indent` is the current nesting depth (0 at the top). Each item keeps
/// its visible inline text (bold/links/code stripped); an ordered list counts from `start`.
fn project_list(html: &str, indent: usize) -> String {
    let ordered = leading_tag(html) == Some("ol");
    // `<ol start="N">` begins at N; a bare `<ol>`/`<ul>` at 1.
    let mut n: usize = first_attr(html, "start")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let pad = "  ".repeat(indent);
    let mut lines = Vec::new();
    for item in top_level_li_inner(html) {
        // Split off a trailing nested list: the text before it is this item; the nested
        // list (if any) recurses one level deeper.
        let (own, nested) = split_nested_list(item);
        let marker = if ordered {
            let m = format!("{n}.");
            n += 1;
            m
        } else {
            "-".to_string()
        };
        lines.push(format!("{pad}{marker} {}", visible(own)));
        if let Some(nested) = nested {
            let sub = project_list(nested, indent + 1);
            if !sub.is_empty() {
                lines.push(sub);
            }
        }
    }
    lines.join("\n")
}

/// The inner HTML of each TOP-LEVEL `<li>` in a list block. `<li>` is emitted
/// attribute-free (`emit.rs::emit_item`), so the literal `<li>`/`</li>` delimit items;
/// nested lists' `<li>`s are matched by depth so a nested item is not taken as top-level.
fn top_level_li_inner(html: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut pos = 0;
    while let Some(rel) = html[pos..].find("<li>") {
        let start = pos + rel + "<li>".len();
        let mut depth = 1usize;
        let mut i = start;
        loop {
            let open = html[i..].find("<li>");
            let close = html[i..].find("</li>");
            match (open, close) {
                (Some(o), Some(c)) if o < c => {
                    depth += 1;
                    i += o + "<li>".len();
                }
                (_, Some(c)) => {
                    depth -= 1;
                    if depth == 0 {
                        items.push(&html[start..i + c]);
                        i += c + "</li>".len();
                        break;
                    }
                    i += c + "</li>".len();
                }
                _ => {
                    // Malformed (no matching close): take the rest and stop.
                    items.push(&html[start..]);
                    i = html.len();
                    break;
                }
            }
        }
        pos = i;
    }
    items
}

/// Split a `<li>`'s inner HTML at its first nested list (`<ul`/`<ol`): the leading part is
/// the item's own content; the trailing part (if any) is the nested list to recurse into.
fn split_nested_list(item: &str) -> (&str, Option<&str>) {
    let at = match (item.find("<ul"), item.find("<ol")) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    match at {
        Some(at) => (&item[..at], Some(&item[at..])),
        None => (item, None),
    }
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p taliesin-core --lib render::text::tests::projects_list -- --nocapture`
Expected: PASS (both tests).

- [ ] **Step 5: Mutation-check**

Temporarily change `lines.join("\n")` to `lines.join("")` in `project_list`, run the tests, confirm `projects_list_items_on_separate_lines` fails, then revert.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/text.rs
git commit -m "feat(read): project list items on separate lines (item 17 F-03)"
```

### Task 2: Stepped-div (scrolly / code-walkthrough) projection arm

**Files:**
- Modify: `crates/core/src/render/text.rs` (add arm + `project_steps`/`step_inners` + unit test)

**Interfaces:**
- Produces: `fn project_steps(html: &str) -> String` (text.rs-internal)
- Consumes: `visible(&str) -> String`.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn projects_scrolly_steps_as_separate_paragraphs() {
        let src = "::: {.scrolly}\n::: {.step}\nThe landscape. High on the wall.\n:::\n\n\
                   ::: {.step}\nWhich way is downhill. The gradient points across.\n:::\n:::\n";
        let out = project_src(src);
        assert!(out.contains("The landscape. High on the wall."), "step 1 text:\n{out}");
        assert!(out.contains("Which way is downhill."), "step 2 text:\n{out}");
        assert!(
            !out.contains("wall.Which"),
            "steps must not merge across their boundary:\n{out}"
        );
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p taliesin-core --lib render::text::tests::projects_scrolly -- --nocapture`
Expected: FAIL (`wall.Which` present).

- [ ] **Step 3: Add the arm + helpers**

In `project_block`, directly after the list arm:

```rust
    // A scrolly / code-walkthrough: project each `.step`'s narration as its own paragraph
    // so adjacent steps don't merge across the boundary (`…in the middle.Which way…`). The
    // `scrolly-steps` container carries the token `scrolly-steps`, not `step`, so matching
    // the exact opening `<div class="step"` never mistakes the container for a step.
    if html.contains("<div class=\"step\"") {
        return project_steps(html);
    }
```

Add:

```rust
/// Project a stepped block (a `.scrolly`'s `scrolly-steps`, or a `.code-walkthrough`) so
/// each `.step`'s visible text is its own paragraph, blank-line separated.
fn project_steps(html: &str) -> String {
    step_inners(html)
        .into_iter()
        .map(visible)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The inner HTML of each `<div class="step"…>` in a stepped block, matched depth-aware
/// over `<div`/`</div>` so a nested `<div>` inside a step does not close it early.
fn step_inners(html: &str) -> Vec<&str> {
    let open = "<div class=\"step\"";
    let mut steps = Vec::new();
    let mut pos = 0;
    while let Some(rel) = html[pos..].find(open) {
        let tag_start = pos + rel;
        let Some(gt) = html[tag_start..].find('>') else { break };
        let start = tag_start + gt + 1;
        let mut depth = 1usize;
        let mut i = start;
        loop {
            let next_open = html[i..].find("<div");
            let next_close = html[i..].find("</div>");
            match (next_open, next_close) {
                (Some(o), Some(c)) if o < c => {
                    depth += 1;
                    i += o + "<div".len();
                }
                (_, Some(c)) => {
                    depth -= 1;
                    if depth == 0 {
                        steps.push(&html[start..i + c]);
                        i += c + "</div>".len();
                        break;
                    }
                    i += c + "</div>".len();
                }
                _ => {
                    steps.push(&html[start..]);
                    i = html.len();
                    break;
                }
            }
        }
        pos = i;
    }
    steps
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p taliesin-core --lib render::text::tests::projects_scrolly -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Mutation-check**

Temporarily change `.join("\n\n")` to `.join("")` in `project_steps`, confirm the test fails, then revert.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/text.rs
git commit -m "feat(read): project scrolly/step narrations as separate paragraphs (item 18 F-01)"
```

### Task 3: Input-control (`{{< input >}}`) projection arm

**Files:**
- Modify: `crates/core/src/render/text.rs` (add arm + `project_inputs`/`class_text` + unit test)

**Interfaces:**
- Produces: `fn project_inputs(html: &str) -> String`, `fn class_text(html: &str, class_token: &str) -> Option<String>` (text.rs-internal)
- Consumes: `class_tag_span(&str, &str) -> Option<(usize, usize)>` (existing, text.rs:222), `visible`.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn projects_input_controls_as_label_equals_value() {
        let src = "{{< input name=\"lr\" type=\"slider\" min=\"0\" max=\"1\" \
                   step=\"0.01\" value=\"0.12\" label=\"step size\" >}}\n";
        let out = project_src(src);
        assert!(out.contains("[input] step size = 0.12"), "input label = value:\n{out}");
        assert!(!out.contains("size0.12"), "label and value must not fuse:\n{out}");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p taliesin-core --lib render::text::tests::projects_input -- --nocapture`
Expected: FAIL (`size0.12` present).

- [ ] **Step 3: Add the arm + helpers**

In `project_block`, directly after the steps arm:

```rust
    // Input control(s): `[input] label = value`, one line per control, so a control's
    // label and value don't fuse (`step size (η)0.12`). `class="tali-input"` (closing
    // quote included) matches only the control wrapper, not `tali-input-label`/`-out`.
    if html.contains("class=\"tali-input\"") {
        return project_inputs(html);
    }
```

Add:

```rust
/// Project a `{{< input >}}` block's control(s) as `[input] <label> = <value>`, one line
/// per control (a block can hold several). Label = the `.tali-input-label` text; value =
/// the `.tali-input-out` `<output>` text.
fn project_inputs(html: &str) -> String {
    let open = "<div class=\"tali-input\"";
    let mut lines = Vec::new();
    let mut pos = 0;
    while let Some(rel) = html[pos..].find(open) {
        let start = pos + rel;
        // This control runs to the next control, or to the block end.
        let next = html[start + open.len()..]
            .find(open)
            .map(|r| start + open.len() + r)
            .unwrap_or(html.len());
        let chunk = &html[start..next];
        let label = class_text(chunk, "tali-input-label").unwrap_or_default();
        let value = class_text(chunk, "tali-input-out").unwrap_or_default();
        lines.push(format!("[input] {label} = {value}"));
        pos = next;
    }
    lines.join("\n")
}

/// The visible text of the first element whose opening tag carries `class_token`. The
/// label/output spans hold plain text with no nested element, so the text runs from the
/// tag's `>` to the next `<` (its closing tag).
fn class_text(html: &str, class_token: &str) -> Option<String> {
    let (_, gt) = class_tag_span(html, class_token)?;
    let inner_start = gt + 1;
    let end = html[inner_start..]
        .find('<')
        .map(|r| inner_start + r)
        .unwrap_or(html.len());
    Some(visible(&html[inner_start..end]))
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p taliesin-core --lib render::text::tests::projects_input -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Mutation-check**

Temporarily change `"[input] {label} = {value}"` to `"[input] {label}{value}"`, confirm the test fails, then revert.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/text.rs
git commit -m "feat(read): project input controls as [input] label = value (item 18 F-01)"
```

### Task 4: Extend the corpus fixture + re-bless the whole-doc snapshot

**Files:**
- Modify: `corpus/reader/text-projection.tmd` (append a list, a scrolly, an input)
- Modify: `crates/core/tests/snapshots/text-projection.txt` (re-blessed)
- Modify: `crates/core/tests/text_projection.rs` (add direct asserts)

**Interfaces:**
- Consumes: the three arms from Tasks 1-3 (must land first).

- [ ] **Step 1: Extend the fixture** — append to `corpus/reader/text-projection.tmd`:

```markdown

## Structured blocks {#sec-structured}

The projection keeps these apart instead of fusing them:

- **name**: the column to reference.
- **Returns**: an `Expr` you can compare and combine.

{{< input name="lr" type="slider" min="0.01" max="0.35" step="0.01" value="0.12" label="step size" >}}

::: {.scrolly}
::: {.step}
The landscape. Descent begins high on the steep left wall of the valley.
:::

::: {.step}
Which way is downhill. The negative gradient points almost straight across.
:::
:::
```

- [ ] **Step 2: Re-bless the snapshot**

Run: `UPDATE_SNAPSHOTS=1 cargo test -p taliesin-core --test text_projection`
Then inspect the diff (an unreviewed re-bless pins the bug):

Run: `git diff crates/core/tests/snapshots/text-projection.txt`
Expected: the appended section shows `- name: the column to reference.` and `- Returns: …` on separate lines, `[input] step size = 0.12`, and the two step narrations as separate paragraphs (no `wall.Which`, no `reference.Returns`, no `size0.12`).

- [ ] **Step 3: Add direct asserts** — in `text_projection.rs`, after the existing `assert!` block (near line 65), add:

```rust
    // Structured blocks stay separated (item 19): list items on their own lines, an input
    // control as `label = value`, and scrolly steps as distinct paragraphs.
    assert!(
        actual.contains("- name: the column to reference."),
        "list item on its own line"
    );
    assert!(!actual.contains("reference.Returns"), "list items must not fuse");
    assert!(
        actual.contains("[input] step size = 0.12"),
        "input control as label = value"
    );
    assert!(!actual.contains("wall.Which"), "scrolly steps must not merge");
```

- [ ] **Step 4: Run the snapshot test (no UPDATE) to verify it passes**

Run: `cargo test -p taliesin-core --test text_projection`
Expected: PASS.

- [ ] **Step 5: Confirm the single-file `read` CLI test still passes**

Run: `cargo test -p taliesin-server --test read_cli`
Expected: PASS (the appended fixture content does not disturb `### Overview` / `Figure 1` asserts).

- [ ] **Step 6: Commit**

```bash
git add corpus/reader/text-projection.tmd crates/core/tests/snapshots/text-projection.txt crates/core/tests/text_projection.rs
git commit -m "test(read): pin structured-block projection in the corpus fixture (item 19)"
```

---

## Part B — Book-aware read (`crates/server/src/query.rs`)

### Task 5: Enclosing-site discovery + single-page auto-scope

**Files:**
- Modify: `crates/server/src/query.rs` (add `enclosing_site_root` + `scoped_site_doc`; wire into `cmd_read`)

**Interfaces:**
- Produces: `fn enclosing_site_root(start: &Path) -> Option<PathBuf>`, `fn scoped_site_doc(path: &Path, src: &str) -> Option<taliesin_core::RenderedDoc>`
- Consumes (all pub in `taliesin_core`): `Site::discover_with(&Path, DraftMode)`, `Site::chapter_for(&Page) -> Option<u32>`, `Site::number_chapter(&Page, &mut [Block])`, `Site::resolve_cross_refs(&mut [Block], &str)`, `render_document_with_includes_scoped(&str, &Path, Option<u32>)`, `DraftMode`, `Page`.

- [ ] **Step 1: Add a `use` for `PathBuf`** — the file already has `use std::path::Path;` (query.rs:17). Change it to:

```rust
use std::path::{Path, PathBuf};
```

- [ ] **Step 2: Add the two helpers** (place after `count_kernel_cells`, before `run_cells`):

```rust
/// Walk up from `start` (a directory) for an enclosing `_site.yml`, stopping at a `.git`
/// boundary or the filesystem root, so `read` of a file inside a book/site can render it
/// the way the site does. Returns the directory that holds the `_site.yml`, if any.
fn enclosing_site_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.canonicalize().ok()?;
    loop {
        if dir.join("_site.yml").is_file() {
            return Some(dir);
        }
        // Don't climb out of the repo/project the file lives in.
        if dir.join(".git").exists() {
            return None;
        }
        dir = dir.parent()?.to_path_buf();
    }
}

/// Render a single file the way its enclosing site would: chapter-scoped numbering
/// (`@thm-elbo` → "Theorem 3.1") plus cross-page reference resolution (`@thm-consistency`
/// on another page → "Theorem 2.1"). Returns `None` when the file is not part of a
/// discoverable site (the caller then does today's standalone render). Reuses the exact
/// sequence `site/search.rs::page_fragment` is proven on, plus heading numbering.
fn scoped_site_doc(path: &Path, src: &str) -> Option<taliesin_core::RenderedDoc> {
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let root = enclosing_site_root(base)?;
    let site = taliesin_core::Site::discover_with(&root, taliesin_core::DraftMode::Include);
    let canon = path.canonicalize().ok()?;
    let page = site
        .pages
        .iter()
        .find(|p| p.input.canonicalize().ok().as_deref() == Some(canon.as_path()))?;
    crate::serve::guarded(|| {
        let mut doc =
            taliesin_core::render_document_with_includes_scoped(src, base, site.chapter_for(page));
        site.number_chapter(page, &mut doc.blocks);
        site.resolve_cross_refs(&mut doc.blocks, &page.url);
        doc
    })
    .ok()
}
```

- [ ] **Step 3: Wire auto-scope into `cmd_read`** — replace the standalone render block (query.rs:136-145, the `let mut doc = match crate::serve::guarded(...) { ... };`) with:

```rust
    // Auto-scope: if this file lives in a site (an enclosing `_site.yml`), render it as the
    // site does (chapter numbering + cross-page refs), so a book chapter reads "Theorem
    // 3.1" / "Chapter 2", not a bare "Theorem". A standalone `.tmd` falls back unchanged.
    let mut doc = match scoped_site_doc(p, &src) {
        Some(d) => d,
        None => match crate::serve::guarded(|| {
            taliesin_core::render_document_with_includes_rooted(&src, base, Some(base))
        }) {
            Ok(d) => d,
            Err(panic) => {
                log::error(&format!("read panicked on {path}: {panic}"));
                return ExitCode::FAILURE;
            }
        },
    };
```

- [ ] **Step 4: Write the failing integration test** — create `crates/server/tests/read_book.rs`:

```rust
//! `taliesin read` is book-aware: a chapter inside a `_site.yml` project resolves its
//! chapter-scoped numbering and cross-page references (item 19, book scoping), reusing
//! `corpus/course/` (em.tmd = Chapter 3; mle.tmd = Chapter 2).

use std::process::Command;

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin read");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn read_of_a_book_chapter_resolves_scoped_and_cross_page_refs() {
    let (ok, stdout, stderr) = run(&["read", &corpus("course/em.tmd")]);
    assert!(ok, "`read` should succeed; stderr: {stderr}");
    // Same-chapter theorem is chapter-scoped (em is Chapter 3): "Theorem 3.1", not "Theorem 1".
    assert!(stdout.contains("Theorem 3.1"), "chapter-scoped number:\n{stdout}");
    // Cross-page refs resolve: @thm-consistency (mle, ch 2) → "Theorem 2.1";
    // @sec-mle (mle's chapter H1) → "Chapter 2".
    assert!(stdout.contains("Theorem 2.1"), "cross-page theorem resolved:\n{stdout}");
    assert!(stdout.contains("Chapter 2"), "cross-page section reads Chapter 2:\n{stdout}");
    // The pre-fix bug: bare "Recall Theorem from Section" must be gone.
    assert!(
        !stdout.contains("Recall Theorem from Section"),
        "cross-page refs are no longer bare:\n{stdout}"
    );
}
```

- [ ] **Step 5: Run to verify it fails, then passes**

First confirm the OLD behavior would fail. Since Steps 2-3 already added the fix, run:
Run: `cargo build -p taliesin-server --bin taliesin && cargo test -p taliesin-server --test read_book read_of_a_book_chapter`
Expected: PASS. (If iterating TDD-strictly, stash Step 3's wiring, run to see FAIL with bare "Theorem", then restore.)

- [ ] **Step 6: Mutation-check**

In `scoped_site_doc`, temporarily comment out `site.resolve_cross_refs(...)`, rebuild, run the test, confirm it fails on the `Theorem 2.1` / `Chapter 2` assert, then restore.

- [ ] **Step 7: Confirm the standalone-doc read is unchanged**

Run: `cargo test -p taliesin-server --test read_cli`
Expected: PASS (`corpus/reader` has no enclosing `_site.yml`, so auto-scope is a no-op there).

- [ ] **Step 8: Commit**

```bash
git add crates/server/src/query.rs crates/server/tests/read_book.rs
git commit -m "feat(read): auto-scope an in-book page (chapter numbers + cross-page refs) (item 16 F-02)"
```

### Task 6: Whole-directory book/site read

**Files:**
- Modify: `crates/server/src/query.rs` (route a directory to `cmd_read_dir`; add `cmd_read_dir` + `DirPage`/`dir_human`/`dir_json`)

**Interfaces:**
- Consumes: `enclosing_site_root` is NOT used here (the dir IS the root); uses `Site::discover_with`, `chapter_for`, `number_chapter`, `resolve_cross_refs`, `RenderedDoc::body_text`, existing `count_kernel_cells`.
- The `read <dir>` route replaces the `directory_rejection` guard in `cmd_read` (a non-site dir keeps a helpful error).

- [ ] **Step 1: Route a directory in `cmd_read`** — replace the `directory_rejection` block (query.rs:122-125) with:

```rust
    // A directory that is a site reads as a whole book; a bare directory keeps the
    // single-file guidance below (via `cmd_read_dir`).
    if Path::new(path).is_dir() {
        return cmd_read_dir(path, format, run);
    }
```

- [ ] **Step 2: Add `cmd_read_dir` + support types/fns** (place after `cmd_read`, before `js_cell_ids`):

```rust
/// One page's projection in a whole-directory read.
struct DirPage {
    rel: String,
    title: Option<String>,
    chapter: Option<u32>,
    text: String,
}

/// `taliesin read <dir>`: project a whole book/site to text, page by page in
/// chapter/nav order, each scoped (chapter numbering + cross-page refs) exactly as a single
/// in-book page read is. Parse-only (whole-book execution is out of scope); `--run` is
/// rejected with a pointer to per-page `--run`.
fn cmd_read_dir(path: &str, format: &str, run: bool) -> ExitCode {
    let dir = Path::new(path);
    // Only a discoverable site (an `_site.yml`) reads as a whole book; a bare directory
    // keeps the single-file guidance and points at `map` for the outline.
    if !dir.join("_site.yml").is_file() {
        log::error(&format!(
            "read projects a .tmd file or a site directory, but {path} has no _site.yml. \
             For a project outline use `taliesin map {path}`; to read one page use \
             `taliesin read {path}/<page>.tmd`."
        ));
        return ExitCode::FAILURE;
    }
    if run {
        log::error(
            "read --run executes one page at a time; run it on a single .tmd file. A \
             whole-directory read is parse-only.",
        );
        return ExitCode::FAILURE;
    }
    let site = taliesin_core::Site::discover_with(dir, taliesin_core::DraftMode::Include);
    if site.pages.is_empty() {
        log::error(&format!("no .tmd pages found under {path}"));
        return ExitCode::FAILURE;
    }
    let mut kernel_cells = 0usize;
    let pages: Vec<DirPage> = site
        .pages
        .iter()
        .filter_map(|page| {
            let src = std::fs::read_to_string(&page.input).ok()?;
            let base = page.input.parent().unwrap_or_else(|| Path::new("."));
            let doc = crate::serve::guarded(|| {
                let mut d = taliesin_core::render_document_with_includes_scoped(
                    &src,
                    base,
                    site.chapter_for(page),
                );
                site.number_chapter(page, &mut d.blocks);
                site.resolve_cross_refs(&mut d.blocks, &page.url);
                d
            })
            .ok()?;
            kernel_cells += count_kernel_cells(&doc.blocks);
            Some(DirPage {
                rel: page.rel.clone(),
                title: page.title.clone(),
                chapter: site.chapter_for(page),
                text: doc.body_text(),
            })
        })
        .collect();
    if kernel_cells > 0 {
        log::warn(&format!(
            "read does not execute code cells ({kernel_cells} kernel cell{} across the book \
             projected as source). Use `build` or `preview` to run them.",
            if kernel_cells == 1 { "" } else { "s" }
        ));
    }
    if format == "json" {
        print!("{}", dir_json(path, &pages));
    } else {
        print!("{}", dir_human(&pages));
    }
    ExitCode::SUCCESS
}

/// The concatenated human projection: each page under a `===== rel (Chapter N) =====`
/// header (the `(Chapter N)` clause only for a numbered chapter), blank-line separated.
fn dir_human(pages: &[DirPage]) -> String {
    let mut out = String::new();
    for p in pages {
        match p.chapter {
            Some(n) => out.push_str(&format!("===== {} (Chapter {n}) =====\n\n", p.rel)),
            None => out.push_str(&format!("===== {} =====\n\n", p.rel)),
        }
        out.push_str(p.text.trim_end());
        out.push_str("\n\n");
    }
    format!("{}\n", out.trim_end())
}

#[derive(serde::Serialize)]
struct ReadDir<'a> {
    path: &'a str,
    pages: Vec<DirPageJson<'a>>,
}

#[derive(serde::Serialize)]
struct DirPageJson<'a> {
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chapter: Option<u32>,
    text: &'a str,
}

/// The machine form of a whole-directory read: `{path, pages:[{path,title,chapter,text}]}`.
fn dir_json(path: &str, pages: &[DirPage]) -> String {
    let out = ReadDir {
        path,
        pages: pages
            .iter()
            .map(|p| DirPageJson {
                path: &p.rel,
                title: p.title.as_deref(),
                chapter: p.chapter,
                text: &p.text,
            })
            .collect(),
    };
    format!(
        "{}\n",
        serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string())
    )
}
```

- [ ] **Step 3: Write the failing tests** — append to `crates/server/tests/read_book.rs`:

```rust
#[test]
fn read_of_a_book_directory_projects_every_chapter() {
    let (ok, stdout, stderr) = run(&["read", &corpus("course")]);
    assert!(ok, "`read <dir>` should succeed; stderr: {stderr}");
    assert!(
        stdout.contains("===== em.tmd (Chapter 3) ====="),
        "per-page header with chapter:\n{stdout}"
    );
    assert!(stdout.contains("Theorem 3.1"), "scoped refs in the whole-book read:\n{stdout}");
    assert!(stdout.contains("Maximum likelihood"), "the mle chapter is present:\n{stdout}");
}

#[test]
fn read_run_on_a_directory_is_rejected() {
    let (ok, _out, stderr) = run(&["read", "--run", &corpus("course")]);
    assert!(!ok, "`read --run` on a directory must fail");
    assert!(
        stderr.contains("one page at a time"),
        "it explains why: {stderr}"
    );
}

#[test]
fn read_of_a_non_site_directory_is_rejected_with_guidance() {
    let (ok, _out, stderr) = run(&["read", &corpus("reader")]);
    assert!(!ok, "a bare directory (no _site.yml) must fail");
    assert!(stderr.contains("no _site.yml"), "it points at map/per-page: {stderr}");
}
```

- [ ] **Step 4: Build + run to verify pass**

Run: `cargo build -p taliesin-server --bin taliesin && cargo test -p taliesin-server --test read_book`
Expected: PASS (all four tests: the Task-5 one plus these three).

- [ ] **Step 5: Confirm the existing `read_rejects_a_directory` test still passes**

The old `read_cli.rs::read_rejects_a_directory` runs `read corpus/reader` and asserts failure. `corpus/reader` has no `_site.yml`, so `cmd_read_dir` errors → the test still passes, but its assertion is now on the new message. Verify:
Run: `cargo test -p taliesin-server --test read_cli`
Expected: PASS. If `read_rejects_a_directory` asserted specific old-message text, update its assert to `stderr.contains("no _site.yml")` (it currently only checks `!stderr.is_empty()`, so no change needed — confirm).

- [ ] **Step 6: Manually spot-check the human output**

Run: `cargo run -q -p taliesin-server -- read corpus/course | head -40`
Expected: `===== index.tmd =====` first, then numbered chapters with `===== <rel> (Chapter N) =====` headers and resolved refs.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/query.rs crates/server/tests/read_book.rs
git commit -m "feat(read): project a whole book/site directory (item 16 F-02)"
```

### Task 7: Full-suite verification + backlog bookkeeping

**Files:**
- Modify: `notes/backlog.md` (remove item 19 + the folded 16 F-02 / 17 F-03 / 18 F-01 sub-findings)
- Modify: `notes/AUDITS.md` only if it references item 19 (check; likely not)

- [ ] **Step 1: Type-check nothing new on the JS side** (no client change), then run the full core + server suites under the three gates:

```bash
TALIESIN_REQUIRE_NODE=1 cargo test -p taliesin-core
TALIESIN_PYTHON="$TALIESIN_PYTHON" TALIESIN_REQUIRE_KERNEL=1 \
  TALIESIN_R=R TALIESIN_REQUIRE_R=1 cargo test -p taliesin-server
```

Expected: PASS. If an `exec` probe flakes, re-run that binary with `--test-threads=1` before blaming this change (see Global Constraints).

- [ ] **Step 2: Confirm search (`indexable_text`) did not drift** — the new arms live in `project_block`, which the Cmd-K index does not call. Verify no search snapshot moved:

```bash
cargo test -p taliesin-core --test tarn 2>&1 | tail -5   # tarn pins built HTML + search-indexable text
```

Expected: PASS (search output unchanged).

- [ ] **Step 3: Update the backlog** — in `notes/backlog.md`: delete section-A item 19 entirely; in item 16 delete the F-02 bullet (mark it landed via git), in item 17 delete the F-03 bullet, in item 18 delete the F-01 bullet. Update the "Next session: start here" numbered list (item 19 was step 2). Leave item 16 F-03 (embed/code-walkthrough) and the other findings intact.

- [ ] **Step 4: Commit the bookkeeping**

```bash
git add notes/backlog.md
git commit -m "docs(backlog): land item 19 (structure-preserving, book-aware read)"
```

- [ ] **Step 5: Report** the branch state (`git log --oneline main..structure-preserving-read`) and that the tree is green, for the author's merge decision. Do NOT push or fast-forward `main` unless asked.

---

## Self-Review

**Spec coverage:**
- Spec A.1 lists → Task 1. A.2 stepped divs → Task 2. A.3 inputs → Task 3. ✓
- Spec B single-page auto-scope → Task 5. B whole-dir (human + json, `--run` rejected) → Task 6. ✓
- Spec "minimal pair, not finish_blocks" → Tasks 5/6 call `number_chapter` + `resolve_cross_refs` only. ✓
- Spec testing (unit per arm, re-blessed snapshot, integration, mutation-check) → Tasks 1-3 (unit + mutation), Task 4 (snapshot), Tasks 5-6 (integration + mutation). ✓
- Spec risk "indexable_text unchanged" → Task 7 Step 2. ✓
- Spec non-goal (embed chrome, item 16 F-03) → untouched; Task 7 Step 3 leaves it. ✓

**Placeholder scan:** No TBD/TODO; every code step shows complete code; every run step shows the command + expected result. ✓

**Type consistency:** `project_list(html, indent)` defined Task 1, recursed Task 1. `project_steps`/`step_inners` Task 2. `project_inputs`/`class_text` Task 3 (uses existing `class_tag_span`). `enclosing_site_root`/`scoped_site_doc` Task 5, `cmd_read_dir`/`DirPage`/`dir_human`/`dir_json`/`ReadDir`/`DirPageJson` Task 6. `Site` methods (`discover_with`/`chapter_for`/`number_chapter`/`resolve_cross_refs`) verified `pub` in `crates/core/src/site/mod.rs`; `DraftMode`/`Page`/`Site`/`render_document_with_includes_scoped` verified exported from `taliesin_core` (`lib.rs:54,58`). ✓
