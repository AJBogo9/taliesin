//! Backlog item 210: a code cell nested inside a `:::` fenced div must execute, and its
//! output must land **inside** the container rather than after it.
//!
//! The defect: `build_container` (`render/divs.rs`) folds a div's children into one HTML
//! string and returns a composite `Block` whose `cell` is `None`, while
//! `Executor::run_through` (`server/src/exec.rs`) only scans **top-level** blocks for a
//! cell to run. So a `{python}` cell inside a `.callout-note`, a `layout-ncol` grid or a
//! `.column-page` was dead source: it rendered, and it never ran. Silent, and it
//! contradicts the tool's core promise — a first user who puts a cell in a callout
//! concludes execution is broken.
//!
//! Placement is half the fix and is pinned here too. Splicing the output as a sibling
//! *after* the container would be visibly wrong for a two-column `layout-ncol` grid (both
//! cells' outputs stacked below the grid instead of under the cell that produced each),
//! so the renderer leaves an empty `data-tali-out-for` slot after each folded cell and the
//! executor fills that in place.
//!
//! This is an EXECUTION pin, so it lives here and not in `corpus/`: the corpus walker
//! renders every document on every `cargo test` but never runs a cell, so a corpus doc
//! would pay the render cost and exercise nothing. `corpus/nested-cells.tmd` pins the
//! render half (the slots and the collected cells); this pins that they actually run.
//!
//! Gated on `TALIESIN_PYTHON` like the rest of the exec tests; `TALIESIN_REQUIRE_KERNEL=1`
//! turns the skip into a hard failure so the coverage can't silently regress to zero.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-210-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The python interpreter to test against, or `None` to skip (unless the CI canary is set,
/// in which case a missing interpreter is a hard failure so the gap can't hide).
fn python_or_skip() -> Option<String> {
    match std::env::var("TALIESIN_PYTHON") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => {
            assert!(
                std::env::var_os("TALIESIN_REQUIRE_KERNEL").is_none(),
                "TALIESIN_REQUIRE_KERNEL=1 but TALIESIN_PYTHON is unset: the nested-cell \
                 execution pin would silently skip. Point TALIESIN_PYTHON at a python with \
                 ipykernel."
            );
            eprintln!("skipping: TALIESIN_PYTHON not set (no kernel)");
            None
        }
    }
}

fn build(src: &Path, dest: &Path, py: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(src)
        .arg(dest)
        .env("TALIESIN_PYTHON", py)
        .env("TALIESIN_NO_CACHE", "1")
        .output()
        .expect("run build");
    assert!(
        out.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    fs::read_to_string(dest).expect("read built html")
}

/// The slice of `html` between the container's opening tag (matched by `open_marker`) and
/// the marker that follows the container. Used to assert an output landed INSIDE a given
/// container rather than merely somewhere on the page.
fn between<'a>(html: &'a str, open_marker: &str, end_marker: &str) -> &'a str {
    let start = html
        .find(open_marker)
        .unwrap_or_else(|| panic!("no {open_marker:?} in page:\n{html}"));
    let rest = &html[start..];
    let end = rest
        .find(end_marker)
        .unwrap_or_else(|| panic!("no {end_marker:?} after {open_marker:?}"));
    &rest[..end]
}

#[test]
fn a_cell_inside_a_callout_runs_and_its_output_lands_inside_the_callout() {
    let Some(py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("callout");
    let src = dir.join("doc.tmd");
    fs::write(
        &src,
        "---\ntitle: Nested\n---\n\n\
         ::: {.callout-note}\n\
         Some prose first.\n\n\
         ```{python}\n\
         print(\"CELL-IN\" + \"-CALLOUT\")\n\
         ```\n\
         :::\n\n\
         After the callout.\n",
    )
    .unwrap();
    let html = build(&src, &dir.join("out.html"), &py);

    assert!(
        html.contains("CELL-IN-CALLOUT"),
        "the cell inside the callout never executed:\n{html}"
    );
    // …and its output is inside the callout body, not spliced in after the container.
    let callout = between(
        &html,
        "class=\"callout callout-note\"",
        "After the callout.",
    );
    assert!(
        callout.contains("CELL-IN-CALLOUT"),
        "output landed outside the callout it belongs to:\n{callout}"
    );
}

#[test]
fn each_cell_of_a_two_column_grid_runs_its_output_into_its_own_column() {
    let Some(py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("grid");
    let src = dir.join("doc.tmd");
    fs::write(
        &src,
        "---\ntitle: Grid\n---\n\n\
         ::: {layout-ncol=2}\n\
         ```{python}\n\
         print(\"COL-\" + \"ONE\")\n\
         ```\n\n\
         ```{python}\n\
         print(\"COL-\" + \"TWO\")\n\
         ```\n\
         :::\n\n\
         After the grid.\n",
    )
    .unwrap();
    let html = build(&src, &dir.join("out.html"), &py);

    assert!(
        html.contains("COL-ONE"),
        "the first column's cell never ran:\n{html}"
    );
    assert!(
        html.contains("COL-TWO"),
        "the second column's cell never ran:\n{html}"
    );
    // Both outputs are INSIDE the grid, not spliced in as siblings after it — which is
    // what the defect produced, and what would stack both under the grid instead of
    // under the cell that made each.
    let grid = between(&html, "class=\"tali-layout\"", "After the grid.");
    assert!(
        grid.contains("COL-ONE") && grid.contains("COL-TWO"),
        "an output landed outside the grid it belongs to:\n{grid}"
    );
    // And each output sits in ITS OWN column, which is an ORDERING claim: the grid runs
    // cell1-source, slot1, cell2-source, slot2 — so the SECOND cell's `<pre>` falls
    // between the two outputs. Both slots stacked at the end of the grid (the shape a
    // sibling splice produces) would leave nothing between them.
    let one = grid.find("COL-ONE").expect("the first column's output");
    let two = grid.find("COL-TWO").expect("the second column's output");
    assert!(one < two, "the outputs are out of document order:\n{grid}");
    assert!(
        grid[one..two].contains("<pre"),
        "the second cell's source is not between the two outputs, so both outputs landed \
         together instead of each under its own cell:\n{grid}"
    );
}

#[test]
fn a_nested_cell_shares_the_kernel_with_the_top_level_cells_around_it() {
    let Some(py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("order");
    let src = dir.join("doc.tmd");
    // The nested cell sits between two top-level ones and must run in document order
    // against the same warm kernel: it reads what the first defined and the third reads
    // what it defined. A nested cell excluded from the run (the defect) makes the third
    // cell raise `NameError`, and one run out of document order makes the second.
    fs::write(
        &src,
        "---\ntitle: Order\n---\n\n\
         ```{python}\n\
         a = 1\n\
         ```\n\n\
         ::: {.callout-tip}\n\
         ```{python}\n\
         b = a + 1\n\
         ```\n\
         :::\n\n\
         ```{python}\n\
         print(f\"SUM={a + b}\")\n\
         ```\n",
    )
    .unwrap();
    let html = build(&src, &dir.join("out.html"), &py);

    assert!(
        !html.contains("NameError"),
        "a nested cell was skipped or ran out of document order:\n{html}"
    );
    assert!(html.contains("SUM=3"), "expected SUM=3 in:\n{html}");
}
