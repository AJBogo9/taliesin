# DX17 Phase 1 — `read --run` executed python/r visibility — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give a headless agent one command — `taliesin read --run <file.tmd>` — that executes python/r cells and reports what each produced (figure/table/output/error), as a human text projection and as `--format json`.

**Architecture:** A shared core classifier (`classify_exec_output`) turns an executed `tali-output` block's HTML into a structured `ExecOutput`. `render/text.rs`'s projector formats it as `[figure fig-x: produced, alt "…"]`-style lines. `read --run` (server) reuses `exec::Executor` exactly as `build` does (freeze-replay + kernel), then projects the executed block model as text or serializes a per-cell JSON summary.

**Tech Stack:** Rust (edition 2024). `taliesin-core` (render/text projection), `taliesin-server` bin crate (`query.rs` CLI, `exec.rs`, `freeze.rs`, `interpreter.rs`), `serde`/`serde_json` (server, already deps), tokio (already a dep; `Runtime::new()?.block_on`).

## Global Constraints

- **Read-only invariant:** never write source; the preview/CLI is a view. Executed output blocks already carry `data-block-id` + `data-sourcepos` (do not change `output_block`).
- **No exec/freeze/kernel machinery change:** reuse `exec::Executor` as `build` does; no freeze-key change, no kernel-lifecycle touch.
- **Backward compatible:** bare `read` (no `--run`) stays exactly as today (parse-only text projection + the existing "kernel cells projected as source" warning).
- **Offline:** no network in any path.
- **Corpus-plus-roadmap:** the feature ships pinned by `corpus/agent/executed-read.tmd` in this same change.
- **Server is a bin crate:** unit tests run with `cargo test -p taliesin-server --bin taliesin`; integration tests are separate crates under `crates/server/tests/` and invoke `env!("CARGO_BIN_EXE_taliesin")`.
- **`rustfmt` runs on save (PostToolUse hook); CI enforces `cargo fmt --check` + clippy.**
- **Flag name is `--run`** (not `--exec`/`-x`).

---

## File structure

- `crates/core/src/render/text.rs` (modify) — add `pub enum ExecOutput`, `pub fn classify_exec_output`, and the `project_block` arm; inline unit tests.
- `crates/core/src/render/mod.rs` (modify) — `pub use text::{ExecOutput, classify_exec_output};`.
- `crates/core/src/lib.rs` (modify) — add `ExecOutput, classify_exec_output` to the `pub use render::{…}` list.
- `crates/server/src/query.rs` (modify) — rewrite `cmd_read` (arg parsing, `--run` exec path, text + JSON projection, serde structs).
- `crates/server/src/main.rs` (modify) — `query::cmd_read(&args)`; add `READ_FLAGS`; wire `read` help if the other subcommands do.
- `corpus/agent/executed-read.tmd` (create) — the corpus pin doc.
- `corpus/README.md` (modify) — one-line entry.
- `crates/server/tests/read_run.rs` (create) — kernel-free CLI tests + kernel-gated executed-projection tests.
- `docs/guide/reference/` + `AGENTS.md` scaffold (modify) — a `read --run` line (last task).

---

## Task 1: Core `ExecOutput` classifier

**Files:**
- Modify: `crates/core/src/render/text.rs`
- Modify: `crates/core/src/render/mod.rs`
- Modify: `crates/core/src/lib.rs`
- Test: inline `#[cfg(test)] mod tests` in `crates/core/src/render/text.rs`

**Interfaces:**
- Produces:
  - `pub enum ExecOutput { Figure { fig_id: Option<String>, alt: Option<String> }, Table { tbl_id: Option<String> }, Stream(String), Error(String), Rich }`
  - `pub fn classify_exec_output(output_html: &str) -> Option<ExecOutput>` — `None` when `output_html` is not an executed output block (no `class="tali-output"`), else the classified kind.

Context — the executed block HTML (`output_block` in `crates/server/src/exec.rs`) is:
`<div class="tali-output" data-block-id="…" data-sourcepos="…">{inner}</div>`, where `{inner}` is:
- a labelled figure: `<figure id="fig-x" class="tali-figure …"><img …><figcaption>Figure&nbsp;N: caption</figcaption></figure>`
- a labelled table: `…<table id="tbl-x">…<caption>Table&nbsp;N: caption</caption>…</table>`
- else raw `render_outputs` HTML: `<pre class="tali-error">…</pre>`, `<pre class="tali-stream …">…</pre>`, `<img alt="output" src="data:image/png…">`/`<svg…>`, or other rich HTML.

Existing text.rs helpers to reuse: `first_attr(html, attr)` (reads an attribute value), `decode`/`visible` (HTML→text), `class_tag_span`/`leading_tag`. Read them before writing.

- [ ] **Step 1: Write the failing tests** (append to text.rs `mod tests`)

```rust
#[test]
fn classify_exec_output_none_for_non_output() {
    assert!(classify_exec_output("<p>hello</p>").is_none());
}

#[test]
fn classify_exec_output_labelled_figure() {
    let html = "<div class=\"tali-output\" data-block-id=\"b-out\" data-sourcepos=\"5:1-7:3\">\
        <figure id=\"fig-hist\" class=\"tali-figure tali-figure-center\">\
        <img alt=\"output\" src=\"data:image/png;base64,AAA\">\
        <figcaption>Figure&nbsp;2: A histogram of scores</figcaption></figure></div>";
    match classify_exec_output(html) {
        Some(ExecOutput::Figure { fig_id, alt }) => {
            assert_eq!(fig_id.as_deref(), Some("fig-hist"));
            assert_eq!(alt.as_deref(), Some("A histogram of scores"));
        }
        other => panic!("expected Figure, got {other:?}"),
    }
}

#[test]
fn classify_exec_output_unlabelled_image_is_a_figure() {
    let html = "<div class=\"tali-output\" data-block-id=\"b-out\" data-sourcepos=\"5:1-7:3\">\
        <img alt=\"output\" src=\"data:image/png;base64,AAA\"></div>";
    // An unlabelled plot is still "a figure produced" to the agent; the generic
    // alt="output" carries no caption, so alt is None.
    match classify_exec_output(html) {
        Some(ExecOutput::Figure { fig_id, alt }) => {
            assert!(fig_id.is_none());
            assert!(alt.is_none(), "generic alt=\"output\" must not surface as a caption");
        }
        other => panic!("expected Figure, got {other:?}"),
    }
}

#[test]
fn classify_exec_output_table_error_stream() {
    let tbl = "<div class=\"tali-output\" data-block-id=\"b\" data-sourcepos=\"1:1-1:1\">\
        <table id=\"tbl-a\"><caption>Table&nbsp;1: Counts</caption><tr><td>1</td></tr></table></div>";
    assert!(matches!(classify_exec_output(tbl),
        Some(ExecOutput::Table { tbl_id }) if tbl_id.as_deref() == Some("tbl-a")));

    let err = "<div class=\"tali-output\" data-block-id=\"b\" data-sourcepos=\"1:1-1:1\">\
        <pre class=\"tali-error\">Traceback\nValueError: bad value</pre></div>";
    assert!(matches!(classify_exec_output(err),
        Some(ExecOutput::Error(s)) if s == "ValueError: bad value"));

    let out = "<div class=\"tali-output\" data-block-id=\"b\" data-sourcepos=\"1:1-1:1\">\
        <pre class=\"tali-stream\">hello world\nsecond line</pre></div>";
    assert!(matches!(classify_exec_output(out),
        Some(ExecOutput::Stream(s)) if s == "hello world"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p taliesin-core --lib classify_exec_output`
Expected: FAIL to compile — `ExecOutput`/`classify_exec_output` not defined.

- [ ] **Step 3: Implement the enum + classifier** (add near the other html helpers in text.rs)

```rust
/// The kind of output an executed code cell produced, classified from the
/// rendered `tali-output` block. Shared by the text projection and `read`'s JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecOutput {
    /// A produced image/plot. `fig_id`/`alt` are set for a labelled figure cell
    /// (`#| label: fig-x` + `#| fig-cap:`); an unlabelled plot has both `None`.
    Figure { fig_id: Option<String>, alt: Option<String> },
    Table { tbl_id: Option<String> },
    /// stdout/stderr or plain text output (first non-empty line, trimmed).
    Stream(String),
    /// A cell error: the summary line (`EName: evalue`).
    Error(String),
    /// Any other rich output (e.g. an unlabelled HTML table/div).
    Rich,
}

/// Classify an executed output block's HTML. `None` if it is not a `tali-output`
/// block. Reads only the rendered HTML — it never reaches back into exec.
pub fn classify_exec_output(output_html: &str) -> Option<ExecOutput> {
    if !output_html.contains("class=\"tali-output\"") {
        return None;
    }
    // Work on the inner content (between the tali-output div's `>` and its close).
    let inner = output_html
        .find("class=\"tali-output\"")
        .and_then(|i| output_html[i..].find('>').map(|g| i + g + 1))
        .map(|start| output_html[start..].trim_end_matches("</div>"))
        .unwrap_or(output_html)
        .trim_start();

    if inner.starts_with("<figure") {
        let fig_id = first_attr(inner, "id");
        let alt = figcaption_caption(inner);
        return Some(ExecOutput::Figure { fig_id, alt });
    }
    if inner.starts_with("<img") || inner.starts_with("<svg") {
        // Unlabelled plot: a produced figure. The generic alt="output" is not a caption.
        let alt = first_attr(inner, "alt").filter(|a| a != "output");
        return Some(ExecOutput::Figure { fig_id: None, alt });
    }
    if inner.starts_with("<table") {
        return Some(ExecOutput::Table { tbl_id: first_attr(inner, "id") });
    }
    if inner.contains("class=\"tali-error\"") {
        return Some(ExecOutput::Error(error_summary(inner)));
    }
    if inner.contains("class=\"tali-stream") {
        let text = visible(inner);
        let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        return Some(ExecOutput::Stream(first.to_string()));
    }
    Some(ExecOutput::Rich)
}

/// The caption of a `figure_wrap` figcaption, i.e. the text after "Figure&nbsp;N: ".
/// `None` for a bare "Figure N" (unlabelled) figcaption.
fn figcaption_caption(figure_html: &str) -> Option<String> {
    let start = figure_html.find("<figcaption>")? + "<figcaption>".len();
    let end = figure_html[start..].find("</figcaption>")? + start;
    let text = decode(&figure_html[start..end]); // decodes &nbsp; etc.
    let (_, cap) = text.split_once(':')?;         // "Figure 2: caption" -> " caption"
    let cap = cap.trim();
    (!cap.is_empty()).then(|| cap.to_string())
}

/// The summary line of a baked `tali-error` pre: the last non-empty line of the
/// decoded text, which for both a no-traceback error and a traceback is `EName: evalue`.
fn error_summary(html: &str) -> String {
    let text = visible(html);
    text.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}
```

> Note: if `decode`/`visible` signatures differ (e.g. take the pre's inner only), adapt the call sites — read their definitions in text.rs first. The behavior asserted by the tests is the contract.

- [ ] **Step 4: Re-export from render + crate root**

In `crates/core/src/render/mod.rs`, beside the other `pub use text::…` (or after `mod text;`):
```rust
pub use text::{ExecOutput, classify_exec_output};
```
In `crates/core/src/lib.rs`, add to the `pub use render::{…}` list (keep alphabetical-ish with the neighbours):
```rust
    ExecOutput, classify_exec_output,
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p taliesin-core --lib classify_exec_output`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/text.rs crates/core/src/render/mod.rs crates/core/src/lib.rs
git commit -m "feat(core): classify_exec_output — structure an executed cell's output block"
```

---

## Task 2: Text projection arm for executed outputs

**Files:**
- Modify: `crates/core/src/render/text.rs` (`project_block`)
- Test: inline `mod tests` in `crates/core/src/render/text.rs`

**Interfaces:**
- Consumes: `classify_exec_output`, `ExecOutput` (Task 1).
- Produces: `project_block` now emits, for an executed output block, one of:
  - `[figure fig-x: produced, alt "caption"]` (labelled) / `[figure: produced (image)]` (unlabelled) / `[figure fig-x: produced]` (labelled, no caption)
  - `[table tbl-x: produced]` / `[table: produced]`
  - `[output: first line…]`
  - `[cell error: EName: evalue]`
  - `[output: produced]` (Rich)

- [ ] **Step 1: Write the failing test** (append to text.rs `mod tests`)

```rust
#[test]
fn project_block_renders_executed_outputs() {
    let fig = Block {
        id: "b-out".into(),
        sourcepos: "5:1-7:3".into(),
        source_file: None,
        html: "<div class=\"tali-output\" data-block-id=\"b-out\" data-sourcepos=\"5:1-7:3\">\
            <figure id=\"fig-hist\" class=\"tali-figure tali-figure-center\">\
            <img alt=\"output\" src=\"data:image/png;base64,AAA\">\
            <figcaption>Figure&nbsp;2: A histogram</figcaption></figure></div>".into(),
        cell: None,
    };
    assert_eq!(project_block(&fig), "[figure fig-hist: produced, alt \"A histogram\"]");

    let err = Block {
        id: "b2-out".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<div class=\"tali-output\" data-block-id=\"b2-out\" data-sourcepos=\"1:1-1:1\">\
            <pre class=\"tali-error\">ValueError: bad value</pre></div>".into(),
        cell: None,
    };
    assert_eq!(project_block(&err), "[cell error: ValueError: bad value]");
}
```
> Construct `Block` exactly as its public fields require (check `render/model.rs`; adapt field names if they differ).

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p taliesin-core --lib project_block_renders_executed_outputs`
Expected: FAIL — the executed output currently projects as generic `[image: …]`/visible text, not the `[figure … produced …]` form.

- [ ] **Step 3: Add the arm to `project_block`**

Near the top of `project_block`, before the generic image/visible handling, add:
```rust
    if let Some(kind) = classify_exec_output(&b.html) {
        return match kind {
            ExecOutput::Figure { fig_id: Some(id), alt: Some(a) } =>
                format!("[figure {id}: produced, alt \"{a}\"]"),
            ExecOutput::Figure { fig_id: Some(id), alt: None } =>
                format!("[figure {id}: produced]"),
            ExecOutput::Figure { fig_id: None, alt: Some(a) } =>
                format!("[figure: produced (image), alt \"{a}\"]"),
            ExecOutput::Figure { fig_id: None, alt: None } =>
                "[figure: produced (image)]".to_string(),
            ExecOutput::Table { tbl_id: Some(id) } => format!("[table {id}: produced]"),
            ExecOutput::Table { tbl_id: None } => "[table: produced]".to_string(),
            ExecOutput::Stream(s) => format!("[output: {s}]"),
            ExecOutput::Error(s) => format!("[cell error: {s}]"),
            ExecOutput::Rich => "[output: produced]".to_string(),
        };
    }
```
> `project_block`'s exact signature is `fn project_block(b: &Block) -> String` (text.rs:37). If it returns via a mutable `String`/push pattern rather than early return, adapt to that shape.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p taliesin-core --lib project_block_renders_executed_outputs`
Expected: PASS.

- [ ] **Step 5: Run the whole core suite** (guards the existing text-projection snapshot did not drift)

Run: `cargo test -p taliesin-core`
Expected: PASS (the `text_projection` snapshot is parse-only, so it must be unaffected).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/text.rs
git commit -m "feat(core): project executed cell outputs as [figure/table/output/cell error] lines"
```

---

## Task 3: `read --run` (text) — arg parsing + exec wiring

**Files:**
- Modify: `crates/server/src/query.rs` (`cmd_read`)
- Modify: `crates/server/src/main.rs` (`cmd_read(&args)` + `READ_FLAGS`)
- Test: `crates/server/tests/read_run.rs` (create; kernel-free cases)

**Interfaces:**
- Consumes: `taliesin_core::render_document_with_includes_rooted`, `crate::exec::Executor`, `crate::freeze::page_path`, `crate::interpreter::{resolve_python, resolve_r}`, `crate::serve::{unknown_flag_error, bad_format_error}`, `crate::log`.
- Produces: `pub(crate) fn cmd_read(args: &[String]) -> ExitCode`.

- [ ] **Step 1: Write the failing kernel-free integration tests** (create `crates/server/tests/read_run.rs`)

```rust
//! `taliesin read --run` executes python/r cells and projects what each produced.
//! Kernel-free cases (parsing, backward-compat, no-exec) run unconditionally; the
//! executed-projection cases are gated on TALIESIN_PYTHON (see the guard).

use std::process::Command;

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin")
}

#[test]
fn bare_read_is_unchanged_and_warns_about_kernel_cells() {
    // A doc with a python cell, read WITHOUT --run: cell projects as source, and the
    // "projected as source" warning fires on stderr.
    let out = run(&["read", &corpus("agent/executed-read.tmd")]);
    assert!(out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("projected as source"), "expected the no-run warning: {err}");
}

#[test]
fn read_rejects_an_unknown_flag() {
    let out = run(&["read", &corpus("agent/executed-read.tmd"), "--bogus"]);
    assert!(!out.status.success(), "unknown flag must fail");
}

#[test]
fn read_run_under_no_exec_projects_source_without_a_kernel() {
    // --run + TALIESIN_NO_EXEC: never touches a kernel; cells stay source, no crash.
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["read", "--run", &corpus("agent/executed-read.tmd")])
        .env("TALIESIN_NO_EXEC", "1")
        .output()
        .expect("run taliesin");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
}
```
> These depend on `corpus/agent/executed-read.tmd` (Task 5). Create a minimal throwaway version now if you implement Task 3 first, or reorder Task 5 before this step. Simplest: do Task 5 first, then this file.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p taliesin-server --test read_run bare_read_is_unchanged`
Expected: FAIL — `read` does not yet parse `--run`/reject unknown flags (and/or the corpus doc is missing).

- [ ] **Step 3: Rewrite `cmd_read`** in `crates/server/src/query.rs`

```rust
const READ_FLAGS: &[&str] = &["--run", "--format", "--json"];

pub(crate) fn cmd_read(args: &[String]) -> ExitCode {
    let mut path: Option<&str> = None;
    let mut run = false;
    let mut format = "human";
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--run" => run = true,
            "--format" => {
                if let Some(v) = it.next() {
                    format = v;
                }
            }
            "--json" => format = "json",
            s if s.starts_with("--") => {
                log::error(&crate::serve::unknown_flag_error(s, READ_FLAGS));
                return ExitCode::FAILURE;
            }
            s => {
                if path.is_none() {
                    path = Some(s);
                }
            }
        }
    }
    let Some(path) = path else {
        eprintln!("usage: taliesin read <file.tmd> [--run] [--format human|json]");
        return ExitCode::FAILURE;
    };
    if format != "human" && format != "json" {
        log::error(&crate::serve::bad_format_error(Some(format)));
        return ExitCode::FAILURE;
    }
    if let Some(msg) = directory_rejection(path, "read projects a single .tmd file") {
        log::error(&msg);
        return ExitCode::FAILURE;
    }
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            log::error(&format!("cannot read {path}: {e}"));
            return ExitCode::FAILURE;
        }
    };
    let p = Path::new(path);
    let base = p.parent().unwrap_or_else(|| Path::new("."));

    // Parse-only render is panic-guarded exactly as today.
    let mut doc = match crate::serve::guarded(|| {
        taliesin_core::render_document_with_includes_rooted(&src, base, Some(base))
    }) {
        Ok(d) => d,
        Err(panic) => {
            log::error(&format!("read panicked on {path}: {panic}"));
            return ExitCode::FAILURE;
        }
    };

    // `--run` executes; `--run` under TALIESIN_NO_EXEC never touches a kernel, so it is
    // effectively "not executed" for the report.
    let executed = run && std::env::var_os("TALIESIN_NO_EXEC").is_none();
    if run {
        let blocks = std::mem::take(&mut doc.blocks);
        doc.blocks = run_cells(blocks, base, p);
    } else {
        let kernel_cells = count_kernel_cells(&doc.blocks);
        if kernel_cells > 0 {
            log::warn(&format!(
                "read does not execute code cells ({kernel_cells} kernel cell{} projected \
                 as source; outputs will be absent). Use `read --run`, `build`, or `preview`.",
                if kernel_cells == 1 { "" } else { "s" }
            ));
        }
    }

    if format == "json" {
        print!("{}", read_json(path, &doc, executed));
    } else {
        print!("{}", doc.body_text());
    }
    ExitCode::SUCCESS
}
```
Add the helpers (same module):
```rust
fn count_kernel_cells(blocks: &[taliesin_core::Block]) -> usize {
    blocks
        .iter()
        .filter(|b| b.cell.as_ref().is_some_and(|c| matches!(c.lang.as_str(), "python" | "r")))
        .count()
}

/// Execute a single doc's cells, mirroring build's single-file exec (no HTML assembly).
/// Takes owned blocks and returns them with output blocks spliced in.
fn run_cells(blocks: Vec<taliesin_core::Block>, base: &Path, doc_path: &Path) -> Vec<taliesin_core::Block> {
    let stem = doc_path.file_stem().and_then(|s| s.to_str()).unwrap_or("doc");
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            log::error(&format!("cannot start async runtime: {e}"));
            return blocks;
        }
    };
    rt.block_on(async {
        let mut ex = crate::exec::Executor::with_freeze(crate::freeze::page_path(
            &base.join("_freeze"),
            stem,
        ))
        .in_dir(base);
        ex.set_interpreters(
            crate::interpreter::resolve_python(None, base),
            crate::interpreter::resolve_r(None, base),
        );
        let out = ex.run(blocks).await;
        if let Some(d) = ex.diagnostic() {
            log::warn(&d);
        }
        out
    })
}
```
> `read_json` is Task 4. To keep Task 3 compiling on its own, add a stub now and replace it in Task 4:
> `fn read_json(_p: &str, _d: &taliesin_core::RenderedDoc, _e: bool) -> String { String::new() }`
> (Task 3's tests never take the `--format json` path, so the stub is unreached there.)

In `crates/server/src/main.rs`, change the dispatch line:
```rust
        Some("read") => query::cmd_read(&args),
```
and, if `main.rs` keeps a per-subcommand flag/help table alongside `MAP_FLAGS`, add a `read` entry mirroring `map`.

- [ ] **Step 4: Create the corpus doc** (do Task 5 now if not yet done), then run:

Run: `cargo test -p taliesin-server --test read_run bare_read_is_unchanged read_rejects_an_unknown_flag read_run_under_no_exec`
Expected: PASS (3 kernel-free tests).

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/query.rs crates/server/src/main.rs crates/server/tests/read_run.rs
git commit -m "feat(read): --run executes python/r via build's exec path (text projection)"
```

---

## Task 4: `read --format json` structured projection

**Files:**
- Modify: `crates/server/src/query.rs` (serde structs + `read_json`)
- Test: `crates/server/tests/read_run.rs` (kernel-free JSON shape)

**Interfaces:**
- Consumes: `taliesin_core::{classify_exec_output, ExecOutput, RenderedDoc, Block}`.
- Produces: `fn read_json(path: &str, doc: &RenderedDoc, executed: bool) -> String`.

- [ ] **Step 1: Write the failing test** (append to `read_run.rs`)

```rust
#[test]
fn read_json_without_run_marks_cells_not_run() {
    let out = run(&["read", &corpus("agent/executed-read.tmd"), "--format", "json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(v["executed"], false);
    let cells = v["cells"].as_array().expect("cells array");
    assert!(!cells.is_empty(), "python/r cells are listed");
    assert!(cells.iter().all(|c| c["kind"] == "not-run"), "no --run -> not-run: {v}");
    assert!(v["text"].is_string(), "text projection included");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p taliesin-server --test read_run read_json_without_run`
Expected: FAIL — `read_json` returns empty / no `executed` field.

- [ ] **Step 3: Implement the structs + `read_json`** (in `query.rs`)

```rust
#[derive(serde::Serialize)]
struct ReadDoc<'a> {
    path: &'a str,
    executed: bool,
    cells: Vec<CellResult>,
    text: String,
}

#[derive(serde::Serialize)]
struct CellResult {
    id: String,
    lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    produced: bool,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    fig_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn read_json(path: &str, doc: &taliesin_core::RenderedDoc, executed: bool) -> String {
    use taliesin_core::ExecOutput;
    let mut cells = Vec::new();
    // Executable cells in document order; an output block (if any) immediately follows
    // its cell, so track the last-seen cell and attach the next tali-output to it.
    let mut pending: Option<(String, String)> = None; // (cell block id, lang)
    for b in &doc.blocks {
        if let Some(c) = &b.cell {
            // Flush a cell that had no following output block.
            if let Some((id, lang)) = pending.take() {
                cells.push(empty_or_not_run(id, lang, executed));
            }
            if matches!(c.lang.as_str(), "python" | "r") {
                pending = Some((b.id.clone(), c.lang.clone()));
                continue;
            }
        }
        if let (Some((id, lang)), Some(kind)) =
            (pending.as_ref(), taliesin_core::classify_exec_output(&b.html))
        {
            let (id, lang) = (id.clone(), lang.clone());
            pending = None;
            let r = match kind {
                ExecOutput::Figure { fig_id, alt } => CellResult {
                    id, lang, label: fig_id.clone(), produced: true, kind: "figure", fig_id, alt, error: None,
                },
                ExecOutput::Table { tbl_id } => CellResult {
                    id, lang, label: tbl_id, produced: true, kind: "table", fig_id: None, alt: None, error: None,
                },
                ExecOutput::Stream(_) => CellResult {
                    id, lang, label: None, produced: true, kind: "stream", fig_id: None, alt: None, error: None,
                },
                ExecOutput::Rich => CellResult {
                    id, lang, label: None, produced: true, kind: "rich", fig_id: None, alt: None, error: None,
                },
                ExecOutput::Error(msg) => CellResult {
                    id, lang, label: None, produced: false, kind: "error", fig_id: None, alt: None, error: Some(msg),
                },
            };
            cells.push(r);
        }
    }
    if let Some((id, lang)) = pending.take() {
        cells.push(empty_or_not_run(id, lang, executed));
    }
    let out = ReadDoc { path, executed, cells, text: doc.body_text() };
    format!("{}\n", serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".into()))
}

fn empty_or_not_run(id: String, lang: String, executed: bool) -> CellResult {
    CellResult {
        id, lang, label: None, produced: false,
        kind: if executed { "empty" } else { "not-run" },
        fig_id: None, alt: None, error: None,
    }
}
```
Replace the Task 3 `read_json` stub with this.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p taliesin-server --test read_run read_json_without_run`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/query.rs crates/server/tests/read_run.rs
git commit -m "feat(read): --format json per-cell executed-output summary"
```

---

## Task 5: Corpus pin doc

**Files:**
- Create: `corpus/agent/executed-read.tmd`
- Modify: `corpus/README.md`

- [ ] **Step 1: Create the doc**

`corpus/agent/executed-read.tmd`:
````markdown
---
title: Executed-output visibility
---

A doc an agent runs `read --run` against to confirm its cells produced output.

```{python}
#| label: fig-hist
#| fig-cap: A histogram of sampled scores
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
plt.hist([1, 2, 2, 3, 3, 3, 4, 4, 5])
plt.gca()
```

```{python}
print("rows:", 9)
```

```{python}
undefined_name + 1
```
````

- [ ] **Step 2: Add a README line** — under the corpus listing in `corpus/README.md`, add:
```
- `agent/executed-read.tmd` — a python figure/stream/error trio; pins `read --run`'s executed-output projection (DX17).
```

- [ ] **Step 3: Verify it satisfies the corpus invariants** (auto-discovered by `corpus.rs`, parse-only)

Run: `cargo test -p taliesin-core --test corpus`
Expected: PASS — clean front-matter, no unknown-key warnings, renders with unique block ids.

- [ ] **Step 4: Commit**

```bash
git add corpus/agent/executed-read.tmd corpus/README.md
git commit -m "corpus: agent/executed-read.tmd pins read --run executed output (DX17)"
```

---

## Task 6: Kernel-gated executed-projection integration test

**Files:**
- Modify: `crates/server/tests/read_run.rs`

**Interfaces:**
- Consumes: the built binary + `corpus/agent/executed-read.tmd`.

- [ ] **Step 1: Add the guard + executed tests** (append to `read_run.rs`)

```rust
/// `Some(python)` when a python kernel is configured, `None` to skip — unless
/// `TALIESIN_REQUIRE_KERNEL=1` (the CI kernel job), which makes a missing interpreter a
/// hard failure so this coverage cannot silently regress to zero.
fn python() -> Option<String> {
    match std::env::var("TALIESIN_PYTHON") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL=1 but TALIESIN_PYTHON is unset: read --run would go untested"
            );
            eprintln!("skipping: TALIESIN_PYTHON not set (no kernel)");
            None
        }
    }
}

#[test]
fn read_run_text_reports_figure_and_error() {
    let Some(py) = python() else { return };
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["read", "--run", &corpus("agent/executed-read.tmd")])
        .env("TALIESIN_PYTHON", &py)
        .output()
        .expect("run taliesin");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("[figure fig-hist: produced"), "figure not reported: {text}");
    assert!(text.contains("alt \"A histogram of sampled scores\""), "alt missing: {text}");
    assert!(text.contains("[cell error:"), "cell error not reported: {text}");
}

#[test]
fn read_run_json_reports_produced_and_error_kinds() {
    let Some(py) = python() else { return };
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["read", "--run", "--format", "json", &corpus("agent/executed-read.tmd")])
        .env("TALIESIN_PYTHON", &py)
        .output()
        .expect("run taliesin");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(v["executed"], true);
    let cells = v["cells"].as_array().unwrap();
    assert!(cells.iter().any(|c| c["kind"] == "figure" && c["fig_id"] == "fig-hist"),
        "figure cell missing: {v}");
    assert!(cells.iter().any(|c| c["kind"] == "error" && c["produced"] == false),
        "error cell missing: {v}");
}
```

- [ ] **Step 2: Run against the real kernel** (from the kernel-setup memory: the venv python)

Run: `TALIESIN_PYTHON=~/.local/share/qmd-venv/bin/python cargo test -p taliesin-server --test read_run`
Expected: PASS — all kernel-free + kernel-gated tests. (Without `TALIESIN_PYTHON` the two gated tests print "skipping".)

- [ ] **Step 3: Commit**

```bash
git add crates/server/tests/read_run.rs
git commit -m "test(read): kernel-gated read --run executed-projection (figure + error)"
```

---

## Task 7: Docs + backlog

**Files:**
- Modify: `docs/guide/reference/` (the agent/CLI surface page that documents `read`)
- Modify: the scaffolded `AGENTS.md` template (in `crates/server/src/cli.rs`, `new_files()` — the agent on-ramp)
- Modify: `notes/backlog.md`

- [ ] **Step 1: Find the read docs + AGENTS.md template**

Run: `grep -rn "taliesin read\|\\bread\\b.*source\|AGENTS.md" docs/guide/reference/ crates/server/src/cli.rs`
Add a sentence documenting `read --run` as the "did my python/r cell produce a figure?" check, and `--format json` for the structured per-cell result. Match the surrounding prose style.

- [ ] **Step 2: Strike DX17(a) from the backlog** — edit `notes/backlog.md` item 1 (DX17) to note (a) shipped and only (b) headless `{js}` remains; add an "Already shipped" line. (Do not renumber unless the item is fully removed — (b) remains, so leave item 1 as the reduced DX17(b).)

- [ ] **Step 3: Full verification** (the three CI gates)

Run:
```bash
cargo test -p taliesin-core
TALIESIN_PYTHON=~/.local/share/qmd-venv/bin/python cargo test -p taliesin-server --bin taliesin -- --test-threads=1
TALIESIN_PYTHON=~/.local/share/qmd-venv/bin/python cargo test -p taliesin-server --test read_run
cargo fmt --check && cargo clippy -p taliesin-core -p taliesin-server --bin taliesin
```
Expected: all PASS, fmt + clippy clean.

- [ ] **Step 4: Commit**

```bash
git add docs/ crates/server/src/cli.rs notes/backlog.md
git commit -m "docs(read): document read --run; strike DX17(a) from backlog"
```

---

## Self-review notes (author)

- **Spec coverage:** CLI surface (T3/T4), exec path (T3), text projection incl. exhaustive kind-map (T1/T2), JSON shape (T4), kernel-free unit tests (T1/T2) + corpus doc (T5) + kernel-gated integration test (T6), docs (T7). All spec sections map to a task.
- **Error shape:** honest summary string end-to-end (`ExecOutput::Error(String)` → `[cell error: …]` / json `error`), matching the spec's decision.
- **Ordering caveat:** Tasks 3/4 tests depend on the Task 5 corpus doc — do Task 5 before running Task 3/4's steps (noted inline). If executing strictly in order, create the corpus doc as the first action of Task 3.
- **Adapt-to-source flags:** `decode`/`visible` signatures and `project_block`'s return style are the two places to verify against text.rs before pasting; the asserted behavior is the contract.
