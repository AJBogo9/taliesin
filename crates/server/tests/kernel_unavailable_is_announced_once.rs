//! A missing interpreter is announced ONCE per run, and the one line it prints is the
//! actionable one.
//!
//! It used to print twice on `build <file>` (and on the since-retired `read --run`): a terse
//! line at the point of failure (`python kernel unavailable (<err>); cells render as source only`) and then the
//! full `Executor::diagnostic()` line at the caller. The terse one said strictly less — no
//! interpreter path, no env var, no `doctor` pointer — so it was pure repetition. A *site*
//! build had the mirror defect: it printed only the terse form, once per page, and never
//! the actionable half at all.
//!
//! Needs no kernel: the failure under test is that the interpreter cannot be launched, so
//! pointing `TALIESIN_PYTHON` at a path that does not exist is the whole fixture.

use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-kua-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `taliesin` with a `python` that cannot possibly launch, and nothing else changed.
fn taliesin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_taliesin"));
    c.env("TALIESIN_PYTHON", "/nonexistent/taliesin-test-python");
    // Do not let a real freeze cache answer the cell and skip the kernel entirely.
    c.env("TALIESIN_NO_CACHE", "1");
    c
}

const CELL: &str = "---\ntitle: T\n---\n\n```{python}\nprint(1)\n```\n";

fn stderr_of(cmd: &mut Command) -> String {
    let out = cmd.output().expect("run taliesin");
    String::from_utf8_lossy(&out.stderr).to_string()
}

/// Lines that announce the unavailable kernel — not the per-cell "this cell did not run"
/// diagnostic, which is a different fact reported per cell on purpose.
fn announcements(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|l| l.contains("kernel unavailable ("))
        .collect()
}

#[test]
fn a_single_doc_build_announces_the_missing_kernel_exactly_once() {
    let dir = tmp_dir("single");
    let doc = dir.join("doc.tmd");
    fs::write(&doc, CELL).unwrap();

    let stderr = stderr_of(taliesin().arg("build").arg(&doc));
    let lines = announcements(&stderr);
    assert_eq!(
        lines.len(),
        1,
        "one announcement, not two; got {lines:#?}\nfull stderr:\n{stderr}"
    );
    // And it is the actionable form, not the terse one that was printed alongside it.
    let line = lines[0];
    for needle in [
        "/nonexistent/taliesin-test-python",
        "TALIESIN_PYTHON",
        "taliesin doctor",
    ] {
        assert!(
            line.contains(needle),
            "the surviving line must be the actionable one (missing {needle:?}): {line}"
        );
    }
}

/// A site build states it once for the whole run, not once per page. The interpreter and
/// its error cannot differ between pages of one build, so repeating it is noise — and the
/// per-page repeat is what made the site path print the *terse* line three times while
/// never printing the actionable one.
#[test]
fn a_site_build_announces_the_missing_kernel_once_for_the_whole_run() {
    let dir = tmp_dir("site");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    for page in ["index", "second", "third"] {
        fs::write(
            dir.join(format!("{page}.tmd")),
            format!("---\ntitle: {page}\n---\n\n```{{python}}\nprint(\"{page}\")\n```\n"),
        )
        .unwrap();
    }

    let stderr = stderr_of(taliesin().arg("build").arg(&dir));
    let lines = announcements(&stderr);
    assert_eq!(
        lines.len(),
        1,
        "three pages, one announcement; got {lines:#?}\nfull stderr:\n{stderr}"
    );
    // The run is still reported as affected — deduplicating the cause must not hide that
    // the build shipped code cells as source.
    assert!(
        stderr.contains("emitted as source") || stderr.contains("render as source"),
        "the build still says its cells did not run:\n{stderr}"
    );
}
