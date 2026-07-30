//! A build whose document has executable cells but no usable kernel **fails**.
//!
//! It used to emit every code cell as source, print a warning, and exit 0. For a book
//! whose entire value is executed output that is the worst possible outcome to report as
//! success: the site looks complete and every result is missing. `--strict` could catch
//! it, but opt-in is the wrong default — other toolchains for executable documents
//! (Quarto, nbconvert, Jupyter Book) error rather than silently degrade.
//!
//! The opt-out is `--no-exec`, which already means exactly "render code cells as source,
//! deliberately". Preview is untouched: a dev server must keep running and show the
//! diagnostic, so only `build` is fatal.
//!
//! Needs no kernel: the failure under test is that the interpreter cannot be launched, so
//! pointing `TALIESIN_PYTHON` at a path that does not exist is the whole fixture.

use std::fs;
use std::process::{Command, Output};

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-krq-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // A boundary marker, so the upward `.venv` walk cannot wander out of the fixture and
    // find a real venv somewhere above /tmp — that would resolve a working interpreter
    // and quietly turn every assertion here vacuous.
    fs::write(dir.join(".git"), b"").unwrap();
    dir
}

/// `taliesin` with a `python` that cannot possibly launch, and nothing else changed.
fn taliesin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_taliesin"));
    c.env("TALIESIN_PYTHON", "/nonexistent/taliesin-test-python");
    // Do not let a real freeze cache answer the cell and skip the kernel entirely.
    c.env("TALIESIN_NO_CACHE", "1");
    // A stray TALIESIN_NO_EXEC in the ambient env is the one value that would make the
    // "it fails" tests pass for the wrong reason.
    c.env_remove("TALIESIN_NO_EXEC");
    c
}

const CELL: &str = "---\ntitle: T\n---\n\n```{python}\nprint(1)\n```\n";

fn run(cmd: &mut Command) -> (Output, String) {
    let out = cmd.output().expect("run taliesin");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    (out, stderr)
}

#[test]
fn a_single_doc_build_with_python_cells_and_no_kernel_fails() {
    let dir = tmp_dir("single");
    let doc = dir.join("doc.tmd");
    fs::write(&doc, CELL).unwrap();

    let (out, stderr) = run(taliesin().arg("build").arg(&doc));
    assert!(
        !out.status.success(),
        "a doc whose cells could not run must not report success:\n{stderr}"
    );
}

#[test]
fn a_site_build_with_python_cells_and_no_kernel_fails() {
    let dir = tmp_dir("site");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(dir.join("index.tmd"), CELL).unwrap();

    let (out, stderr) = run(taliesin().arg("build").arg(&dir));
    assert!(
        !out.status.success(),
        "a site whose cells could not run must not report success:\n{stderr}"
    );
}

#[test]
fn the_failure_names_every_source_it_searched_in_precedence_order() {
    let dir = tmp_dir("report");
    let doc = dir.join("doc.tmd");
    fs::write(&doc, CELL).unwrap();

    let (_, stderr) = run(taliesin().arg("build").arg(&doc));
    // Scope to the ordered report itself. Searching all of stderr would find
    // "TALIESIN_PYTHON" in the earlier `kernel unavailable` warning and compare positions
    // across two different messages, which says nothing about the report's order.
    let at = stderr
        .find("interpreter resolution, in order:")
        .unwrap_or_else(|| panic!("the failure must print the resolution order:\n{stderr}"));
    let report = &stderr[at..];
    let idx = |needle: &str| {
        report
            .find(needle)
            .unwrap_or_else(|| panic!("the failure must name {needle:?}:\n{stderr}"))
    };
    // Every source, in the order resolution actually applied them. Order is the contract:
    // a report that lists them out of order teaches the reader the wrong precedence.
    assert!(idx("_site.yml python:") < idx("<project>/.venv"));
    assert!(idx("<project>/.venv") < idx("TALIESIN_PYTHON"));
    assert!(idx("TALIESIN_PYTHON") < idx("ancestor .venv"));
    assert!(idx("ancestor .venv") < idx("python3"));
    // Where the upward walk stopped, so a wrong pick is diagnosable without reading source.
    assert!(
        stderr.contains("stopped at"),
        "the failure must say where the upward search stopped:\n{stderr}"
    );
    // And it names the opt-out rather than leaving the author stuck.
    assert!(
        stderr.contains("--no-exec"),
        "the failure must name the opt-out:\n{stderr}"
    );
}

#[test]
fn no_exec_is_the_opt_out_and_still_succeeds() {
    let dir = tmp_dir("noexec");
    let doc = dir.join("doc.tmd");
    fs::write(&doc, CELL).unwrap();

    let (out, stderr) = run(taliesin().arg("build").arg(&doc).arg("--no-exec"));
    assert!(
        out.status.success(),
        "--no-exec is a deliberate source-only render, not a failure:\n{stderr}"
    );
}

#[test]
fn a_document_with_no_executable_cells_is_unaffected() {
    // The failure is "has executable cells AND no kernel". Prose-only documents never
    // needed a kernel, so a missing one is not their problem — without this the change
    // would fail every build on a machine with no Python at all.
    let dir = tmp_dir("prose");
    let doc = dir.join("doc.tmd");
    fs::write(&doc, "---\ntitle: T\n---\n\nJust prose, and a non-executable fence:\n\n```python\nprint(1)\n```\n").unwrap();

    let (out, stderr) = run(taliesin().arg("build").arg(&doc));
    assert!(
        out.status.success(),
        "a document with no executable cells must still build:\n{stderr}"
    );
}
