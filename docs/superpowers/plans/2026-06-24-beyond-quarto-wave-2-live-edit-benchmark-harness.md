# Beyond Quarto Wave 2: live-edit-benchmark-harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A committed `tools/live-edit-bench` crate that measures qmd-fast's live-edit moat through the real `render_document_with_includes` -> `diff_blocks` seam (cold render, warm edit-above render+diff, emitted payload vs full HTML, and DOM preservation at the diff level), with a CI-safe regression gate and a binary that emits a markdown table + JSON for the hero demo.

**Architecture:** A new workspace crate `tools/live-edit-bench` (so no bench code ships in the `qmd-fast` binary). `src/lib.rs` is the single source of truth: `measure_live_edit` renders a doc cold, applies a deterministic in-memory edit (insert a paragraph above the cells/collapsible), renders the edited source, and diffs, returning a `LiveEditMetrics`. `src/main.rs` runs it on a real corpus doc and emits artifacts. `tests/regression.rs` asserts the deterministic invariants (a collapse callout below the edit gets a `SetMeta`, never an `Update`; the op set is small; payload is far smaller than the full HTML). The end-to-end browser proof is captured live (not committed), via the existing `tools/record-demo` / chrome-devtools.

**Tech Stack:** Rust edition 2024 / resolver 3; new crate deps `qmd-fast-core` (workspace path), `serde` (derive), `serde_json` (artifact emit). All confined to the tools crate; the shipped binary is unaffected.

## Global Constraints

- Rust edition 2024, resolver 3. The new crate uses `version.workspace`/`edition.workspace`/`license.workspace` and sets `publish = false`.
- No new dependency on `crates/core` or `crates/server`. `serde`/`serde_json` live only in `tools/live-edit-bench/Cargo.toml`, so the `qmd-fast` binary gains nothing.
- No em dashes or en dashes in any authored prose, comment, doc, or commit message. Use commas, colons, parentheses, or restructured sentences.
- CI enforces `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Each task ends green on all three. (The new crate joins the workspace, so it is fmt/clippy/test gated.)
- INVARIANT SAFETY: pure measurement. The bench edits an in-memory `String` copy of the source, NEVER the file on disk; it reads only block `id` / `sourcepos` / `html`; it changes NOTHING in `crates/core` or `crates/server` (the render, diff, exec paths are untouched). The only edits outside the new crate are the root `Cargo.toml` workspace member list and `.gitignore`.
- No Jupyter kernel and no browser in the committed bench: `render_document_with_includes` does not execute cells (it renders them as source), so no kernel is needed; the DOM proof is asserted at the diff level (`SetMeta`), and the browser proof is captured live, out of band.
- The regression gate asserts deterministic invariants (op structure, payload ratio, `dom_preserved`), NEVER absolute timings (machine-dependent).

## File Structure

- `tools/live-edit-bench/Cargo.toml` (NEW): the crate manifest (workspace member, `publish = false`).
- `tools/live-edit-bench/src/lib.rs` (NEW): `LiveEditMetrics`, `measure_live_edit`, `op_payload_bytes`, `markdown_report`, plus `#[cfg(test)]` unit tests. One responsibility: measuring one live edit through the core seam.
- `tools/live-edit-bench/src/main.rs` (NEW): the binary, runs `measure_live_edit` on `corpus/posts/em-algorithm/index.qmd` and emits a markdown table (stdout) + `RESULTS.json`.
- `tools/live-edit-bench/tests/regression.rs` (NEW): the CI-safe gate.
- `tools/live-edit-bench/RESULTS.md` (NEW, committed snapshot): an indicative markdown table from one run (machine-noted).
- `Cargo.toml` (MODIFY): add `tools/live-edit-bench` to `[workspace] members`.
- `.gitignore` (MODIFY): ignore `tools/live-edit-bench/RESULTS.json` (a runtime artifact; the committed snapshot is `RESULTS.md`).

---

### Task 1: The measurement crate + the CI-safe regression gate

**Files:**
- Create: `tools/live-edit-bench/Cargo.toml`
- Create: `tools/live-edit-bench/src/lib.rs`
- Create: `tools/live-edit-bench/tests/regression.rs`
- Modify: `Cargo.toml` (workspace members)

**Interfaces:**
- Produces: `live_edit_bench::LiveEditMetrics` (a `pub struct` with the fields below, `#[derive(Debug, Clone, serde::Serialize)]`); `live_edit_bench::measure_live_edit(doc_label: &str, src: &str, base: &std::path::Path, edit: impl Fn(&str) -> String) -> LiveEditMetrics`; `live_edit_bench::markdown_report(m: &LiveEditMetrics) -> String`.
- Consumes: `qmd_fast_core::{render_document_with_includes, diff_blocks, BlockOp}`; `RenderedDoc::body_html()` and `RenderedDoc::blocks` (a `Vec<Block>` with `pub id: String`, `pub sourcepos: String`, `pub source_file: Option<String>`, `pub html: String`).

- [ ] **Step 1: Create the crate manifest**

Create `tools/live-edit-bench/Cargo.toml`:

```toml
[package]
name = "live-edit-bench"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
publish = false

[dependencies]
qmd-fast-core = { workspace = true }
serde = { workspace = true }
serde_json = "1"
```

- [ ] **Step 2: Add the crate to the workspace**

In the root `Cargo.toml`, change the `[workspace] members` line:

```toml
members = ["crates/core", "crates/server", "tools/live-edit-bench"]
```

- [ ] **Step 3: Write the failing regression tests**

Create `tools/live-edit-bench/tests/regression.rs`:

```rust
use live_edit_bench::measure_live_edit;
use std::path::Path;

/// A synthetic doc: a paragraph and a collapsible callout (which renders as a
/// `<details>`). Inserting a paragraph ABOVE everything shifts the callout's line
/// numbers but not its content, so the diff must patch it in place (`SetMeta`),
/// never replace it (`Update`), which is what keeps its open/closed DOM state alive.
const SYNTHETIC: &str = "\
# Title

First paragraph above the callout.

::: {.callout-note collapse=\"true\"}
## Note
Body of the collapsible note.
:::

More text after the callout.
";

#[test]
fn edit_above_preserves_the_collapsible_dom_node() {
    let m = measure_live_edit(
        "synthetic",
        SYNTHETIC,
        Path::new("."),
        |s| s.replace("First paragraph", "A freshly typed line.\n\nFirst paragraph"),
    );
    assert!(
        m.dom_preserved,
        "the <details> block below the edit should get a SetMeta (same DOM node), got metrics: {m:?}"
    );
    assert_eq!(
        m.update_count, 0,
        "no block below the edit should be re-rendered (no Update), got: {m:?}"
    );
    assert!(m.insert_count >= 1, "the new paragraph is an Insert, got: {m:?}");
    assert!(m.set_meta_count >= 1, "shifted blocks below are SetMeta, got: {m:?}");
}

/// On a real corpus doc, the warm-edit payload (the BlockOps sent over the wire) is
/// far smaller than the full page HTML a reload re-sends. (em-algorithm has a
/// `collapse=\"true\"` callout and `{python}` cells, which render as source here.)
#[test]
fn warm_edit_payload_is_far_smaller_than_full_render() {
    let doc = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/posts/em-algorithm/index.qmd");
    let src = std::fs::read_to_string(doc).expect("read em-algorithm corpus doc");
    let base = Path::new(doc).parent().unwrap();
    let m = measure_live_edit(
        "em-algorithm",
        &src,
        base,
        |s| s.replace(
            "Let's start from a practical example.",
            "A freshly typed opening line.\n\nLet's start from a practical example.",
        ),
    );
    assert!(
        m.edit_payload_bytes * 10 < m.full_html_bytes,
        "payload {} should be far below full html {} (ratio guard), metrics: {m:?}",
        m.edit_payload_bytes, m.full_html_bytes
    );
    assert!(m.dom_preserved, "the collapse callout below the edit should survive, got: {m:?}");
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p live-edit-bench --test regression 2>&1 | tail -20`
Expected: compile failure (`live_edit_bench::measure_live_edit` / `LiveEditMetrics` do not exist yet).

- [ ] **Step 5: Implement `src/lib.rs`**

Create `tools/live-edit-bench/src/lib.rs`:

```rust
//! Measure qmd-fast's live-edit moat through the real core seam
//! (`render_document_with_includes` -> `diff_blocks`): cold render, a warm
//! edit-above render+diff, the emitted `BlockOp` payload vs the full page HTML, and
//! DOM preservation at the diff level (a `<details>` / cell block below the edit gets
//! a `SetMeta`, not an `Update`). Pure measurement: it edits an in-memory copy of the
//! source, never the file, and reads only block id / sourcepos / html.

use qmd_fast_core::{BlockOp, diff_blocks, render_document_with_includes};
use std::path::Path;
use std::time::Instant;

/// One live edit's measurements. Times are nanoseconds and machine-dependent (the
/// regression gate asserts the deterministic structural fields, not the times).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LiveEditMetrics {
    pub doc: String,
    pub cold_render_ns: u128,
    pub warm_edit_ns: u128, // render(edited) + diff
    pub diff_ns: u128,
    pub op_count: usize,
    pub set_meta_count: usize, // DOM-preserving ops
    pub update_count: usize,   // DOM-replacing ops
    pub insert_count: usize,
    pub remove_count: usize,
    pub full_html_bytes: usize,    // body_html().len(): what a full reload re-sends
    pub edit_payload_bytes: usize, // the BlockOp wire payload for the edit
    pub dom_preserved: bool,       // a <details> block below the edit kept its node
}

/// The wire-payload size of one op: a faithful proxy of the server's JSON message
/// (the variable-length html / ids / sourcepos plus a small fixed envelope).
fn op_payload_bytes(op: &BlockOp) -> usize {
    const ENVELOPE: usize = 32; // {"type":"...","target_id":"..."} scaffolding
    match op {
        BlockOp::Update { target_id, html } => ENVELOPE + target_id.len() + html.len(),
        BlockOp::Insert { after_id, html } => {
            ENVELOPE + after_id.as_deref().map_or(0, str::len) + html.len()
        }
        BlockOp::Remove { target_id } => ENVELOPE + target_id.len(),
        BlockOp::SetMeta { target_id, sourcepos, source_file } => {
            ENVELOPE + target_id.len() + sourcepos.len() + source_file.as_deref().map_or(0, str::len)
        }
    }
}

/// Render `src` cold, apply `edit` (a deterministic source transform), render the
/// edited source, and diff. `edit` should change text ABOVE the cells/collapsible so
/// the blocks below shift their line numbers (yielding `SetMeta`s).
pub fn measure_live_edit(
    doc_label: &str,
    src: &str,
    base: &Path,
    edit: impl Fn(&str) -> String,
) -> LiveEditMetrics {
    let t = Instant::now();
    let cold = render_document_with_includes(src, base);
    let cold_render_ns = t.elapsed().as_nanos();
    let full_html_bytes = cold.body_html().len();

    let edited = edit(src);
    let t = Instant::now();
    let new_doc = render_document_with_includes(&edited, base);
    let render_ns = t.elapsed().as_nanos();

    let t = Instant::now();
    let ops = diff_blocks(&cold.blocks, &new_doc.blocks);
    let diff_ns = t.elapsed().as_nanos();

    let (mut set_meta_count, mut update_count, mut insert_count, mut remove_count) = (0, 0, 0, 0);
    let mut edit_payload_bytes = 0;
    let mut set_meta_ids = std::collections::HashSet::new();
    for op in &ops {
        edit_payload_bytes += op_payload_bytes(op);
        match op {
            BlockOp::SetMeta { target_id, .. } => {
                set_meta_count += 1;
                set_meta_ids.insert(target_id.clone());
            }
            BlockOp::Update { .. } => update_count += 1,
            BlockOp::Insert { .. } => insert_count += 1,
            BlockOp::Remove { .. } => remove_count += 1,
        }
    }
    // DOM preserved: a `<details>` block (a collapse callout, the stateful element the
    // moat is about) below the edit kept its identity, so it got a `SetMeta` rather
    // than being re-rendered. False when the doc has no such element.
    let dom_preserved = new_doc
        .blocks
        .iter()
        .any(|b| b.html.contains("<details") && set_meta_ids.contains(&b.id));

    LiveEditMetrics {
        doc: doc_label.to_string(),
        cold_render_ns,
        warm_edit_ns: render_ns + diff_ns,
        diff_ns,
        op_count: ops.len(),
        set_meta_count,
        update_count,
        insert_count,
        remove_count,
        full_html_bytes,
        edit_payload_bytes,
        dom_preserved,
    }
}

/// A human-readable markdown table for one measurement (printed by the binary and
/// snapshotted into `RESULTS.md`). Times shown in microseconds for readability.
pub fn markdown_report(m: &LiveEditMetrics) -> String {
    let us = |ns: u128| ns as f64 / 1000.0;
    let ratio = if m.edit_payload_bytes == 0 {
        0.0
    } else {
        m.full_html_bytes as f64 / m.edit_payload_bytes as f64
    };
    format!(
        "## live-edit benchmark: `{doc}`\n\n\
         | metric | value |\n\
         |---|---|\n\
         | cold full render | {cold:.1} us |\n\
         | warm edit (render + diff) | {warm:.1} us |\n\
         | diff only | {diff:.1} us |\n\
         | ops emitted | {ops} (insert {ins}, set_meta {sm}, update {up}, remove {rm}) |\n\
         | full page HTML | {html} bytes |\n\
         | warm-edit payload | {payload} bytes |\n\
         | payload shrink vs full reload | {ratio:.0}x smaller |\n\
         | open `<details>` survives as same DOM node | {dom} |\n",
        doc = m.doc,
        cold = us(m.cold_render_ns),
        warm = us(m.warm_edit_ns),
        diff = us(m.diff_ns),
        ops = m.op_count,
        ins = m.insert_count,
        sm = m.set_meta_count,
        up = m.update_count,
        rm = m.remove_count,
        html = m.full_html_bytes,
        payload = m.edit_payload_bytes,
        ratio = ratio,
        dom = if m.dom_preserved { "yes" } else { "no" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_payload_bytes_sums_html_plus_envelope() {
        let update = BlockOp::Update { target_id: "b-1".into(), html: "<p>hi</p>".into() };
        assert_eq!(op_payload_bytes(&update), 32 + 3 + 9);
        let set_meta = BlockOp::SetMeta {
            target_id: "b-2".into(),
            sourcepos: "5:1-7:3".into(),
            source_file: None,
        };
        assert_eq!(op_payload_bytes(&set_meta), 32 + 3 + 7);
    }

    #[test]
    fn markdown_report_renders_the_headline_rows() {
        let m = LiveEditMetrics {
            doc: "x".into(), cold_render_ns: 1000, warm_edit_ns: 1200, diff_ns: 50,
            op_count: 3, set_meta_count: 2, update_count: 0, insert_count: 1, remove_count: 0,
            full_html_bytes: 10000, edit_payload_bytes: 100, dom_preserved: true,
        };
        let md = markdown_report(&m);
        assert!(md.contains("100x smaller"), "ratio row: {md}");
        assert!(md.contains("survives as same DOM node | yes"), "dom row: {md}");
    }
}
```

- [ ] **Step 6: Run the tests (gate + unit)**

Run: `cargo test -p live-edit-bench 2>&1 | grep -E 'test result:|error\[' | grep -vE '0 failed' && echo FAILURES || echo "bench crate green"`
Expected: `bench crate green` (both regression tests + both unit tests pass). If `edit_above_preserves_the_collapsible_dom_node` fails on `dom_preserved`, STOP and report: it would mean a collapse callout below an edit is NOT getting a `SetMeta`, a real moat finding, not a test bug.

- [ ] **Step 7: Workspace gate**

Run: `cargo test --workspace 2>&1 | grep -E 'test result:|error\[' | grep -vE '0 failed' && echo FAILURES || echo "all green"`
Then: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: `all green`, fmt clean, clippy clean.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml tools/live-edit-bench/Cargo.toml tools/live-edit-bench/src/lib.rs tools/live-edit-bench/tests/regression.rs
git commit -m "feat(bench): measure the live-edit moat (render+diff+payload+DOM-preservation)"
```

---

### Task 2: The artifact-emitting binary + committed snapshot

**Files:**
- Create: `tools/live-edit-bench/src/main.rs`
- Create: `tools/live-edit-bench/RESULTS.md`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: `live_edit_bench::{measure_live_edit, markdown_report, LiveEditMetrics}` (from Task 1).

- [ ] **Step 1: Implement the binary**

Create `tools/live-edit-bench/src/main.rs`:

```rust
//! Run the live-edit benchmark on a real corpus doc and emit artifacts: a markdown
//! table to stdout (snapshotted into RESULTS.md) and RESULTS.json (the raw metrics
//! the hero demo cites). Pure measurement, it never writes the corpus doc.

use live_edit_bench::{markdown_report, measure_live_edit};
use std::path::Path;

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let doc = format!("{manifest}/../../corpus/posts/em-algorithm/index.qmd");
    let src = std::fs::read_to_string(&doc).expect("read the em-algorithm corpus doc");
    let base = Path::new(&doc).parent().expect("doc has a parent dir");

    let m = measure_live_edit(
        "corpus/posts/em-algorithm/index.qmd",
        &src,
        base,
        |s| {
            s.replace(
                "Let's start from a practical example.",
                "A freshly typed opening line.\n\nLet's start from a practical example.",
            )
        },
    );

    print!("{}", markdown_report(&m));

    let json = serde_json::to_string_pretty(&m).expect("serialize metrics");
    let out = format!("{manifest}/RESULTS.json");
    std::fs::write(&out, json + "\n").expect("write RESULTS.json");
    eprintln!("wrote {out}");
}
```

- [ ] **Step 2: Ignore the JSON runtime artifact**

In `.gitignore`, add:

```
tools/live-edit-bench/RESULTS.json
```

- [ ] **Step 3: Run the binary and confirm it emits a sane report**

Run: `cargo run -p live-edit-bench 2>/dev/null | tee /tmp/bench-report.md`
Expected: a markdown table with non-zero `full page HTML` bytes, a small `warm-edit payload`, a large `payload shrink` ratio, and `open <details> survives as same DOM node | yes`. Then confirm the JSON is valid:
Run: `python3 -m json.tool tools/live-edit-bench/RESULTS.json >/dev/null && echo "RESULTS.json is valid"`
Expected: `RESULTS.json is valid`.

- [ ] **Step 4: Commit the indicative markdown snapshot**

Create `tools/live-edit-bench/RESULTS.md` with a one-line machine note followed by the captured table:

```markdown
# live-edit benchmark results (indicative)

> Indicative numbers from one run; absolute times vary by machine. Regenerate with
> `cargo run -p live-edit-bench`. The structural rows (op counts, payload ratio, DOM
> preservation) are the deterministic, gated invariants.

<PASTE THE EXACT MARKDOWN TABLE PRINTED BY `cargo run -p live-edit-bench` HERE>
```

Replace the `<PASTE ...>` line with the actual table from Step 3's `/tmp/bench-report.md` (the `## live-edit benchmark: ...` block).

- [ ] **Step 5: Gate**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Then: `cargo test --workspace 2>&1 | grep -E 'test result:|error\[' | grep -vE '0 failed' && echo FAILURES || echo "all green"`
Expected: fmt clean, clippy clean, `all green`.

- [ ] **Step 6: Commit**

```bash
git add tools/live-edit-bench/src/main.rs tools/live-edit-bench/RESULTS.md .gitignore
git commit -m "feat(bench): emit markdown + JSON artifacts; commit an indicative snapshot"
```

---

## Self-Review

**Spec coverage** (the design at `docs/superpowers/specs/2026-06-24-live-edit-benchmark-harness-design.md`):
- "Cold full render, warm edit, payload, DOM preservation" -> Task 1 `measure_live_edit` (the four measurements) + the `LiveEditMetrics` fields.
- "New `tools/live-edit-bench` crate, no bench code in the binary" -> Task 1 Steps 1-2 (the crate + workspace member; nothing in core/server changes).
- "Single source of truth lib + emit bin + CI-safe tests" -> lib (Task 1 Step 5), bin (Task 2), tests (Task 1 Step 3).
- "Deterministic edit inserting a paragraph above" -> the `edit` closures in the tests + the bin (`replace(...)` that inserts a paragraph).
- "DOM preservation at the diff level (SetMeta not Update)" -> `dom_preserved` + the `update_count == 0` assertion.
- "Payload vs full HTML" -> `edit_payload_bytes` / `full_html_bytes` + the ratio assertion + the markdown ratio row.
- "Emit markdown + JSON for the hero demo" -> Task 2 (`markdown_report` to stdout + `RESULTS.json` + the committed `RESULTS.md`).
- "Honesty note (no render speedup; the win is no-startup + payload + DOM)" -> captured in the lib doc comment + the markdown rows (which show cold ~ warm and the payload shrink), to be reflected in `RESULTS.md`.
- Out-of-scope items (kernel exec, committed browser harness, Quarto run) are honored: no task adds them. The live browser proof is done by the controller after merge, not a subagent task.

**Placeholder scan:** No TBD/TODO. The one fill-in is Task 2 Step 4's `<PASTE ...>` (the captured table), which is an explicit, necessary capture of generated output, not a left-behind placeholder; the surrounding RESULTS.md prose is complete. Every code step shows complete code.

**Type consistency:** `LiveEditMetrics` fields are defined once (Task 1 Step 5) and used identically in the tests (Task 1 Step 3), the unit tests, `markdown_report`, and the bin (Task 2). `measure_live_edit(doc_label, src, base, edit)` has the same 4-arg signature at every call site (the two regression tests, the two unit tests do not call it, the bin). `op_payload_bytes` is private and tested in the lib's `#[cfg(test)]`. `markdown_report` and `measure_live_edit` are the only `pub` fns consumed by the bin.

**Scope check:** Two independently testable, independently committable tasks. Task 1 is the measured library + the regression gate (the deterministic core); Task 2 is the artifact-emitting binary + snapshot. No change to `crates/core` or `crates/server` beyond the workspace member list. The crate's deps (`serde`/`serde_json`) do not reach the shipped binary.
