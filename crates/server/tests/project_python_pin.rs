//! A project's `_site.yml python:` governs **both** spellings of `build` inside it.
//!
//! `build <project>` and `preview` both resolve the interpreter through the project config;
//! `build <project>/posts/p.tmd` resolved it with `field: None` and so skipped the pin
//! entirely, falling through to `<root>/.venv`, `TALIESIN_PYTHON` or bare `python3`. Two
//! things break. The author gets a different interpreter from the one they pinned (and
//! `doctor`'s own fix line recommends setting exactly that key). And the interpreter's
//! identity seeds every cumulative freeze key, so the single-file build writes into the
//! project's `_freeze/<page>.json` — which it deliberately shares with the site build — under
//! a key the site build can never hit, and neither run ever restores the other's output.
//!
//! Needs no kernel: both candidate interpreters are paths that do not exist, so the build
//! fails identically either way and the only thing under test is WHICH one it chose. That is
//! read off the `<- used` marker in the resolution report the failure prints.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// The interpreter the project pins. Must not exist: the test is about the choice, not
/// about running anything.
const PINNED: &str = "/nonexistent/tali-site-pinned-python";
/// What `TALIESIN_PYTHON` offers instead — the value that used to win.
const FROM_ENV: &str = "/nonexistent/tali-env-python";

const CELL: &str = "---\ntitle: T\n---\n\n```{python}\nprint(1)\n```\n";

/// A project with a `python:` pin and one page in a subdirectory, so the single-file build
/// has to climb to find the project at all.
fn fixture(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-pypin-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("posts")).unwrap();
    // A boundary marker, so neither the upward `_site.yml` walk nor the upward `.venv` walk
    // can wander above the fixture and find a real one — that would resolve a working
    // interpreter and quietly turn every assertion here vacuous.
    fs::write(dir.join(".git"), b"").unwrap();
    fs::write(
        dir.join("_site.yml"),
        format!("title: S\npython: {PINNED}\n"),
    )
    .unwrap();
    fs::write(dir.join("posts").join("p.tmd"), CELL).unwrap();
    dir
}

fn taliesin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_taliesin"));
    // The loser: without the pin being read, this is what resolution lands on.
    c.env("TALIESIN_PYTHON", FROM_ENV);
    // Do not let a real freeze cache answer the cell and skip interpreter resolution.
    c.env("TALIESIN_NO_CACHE", "1");
    // A stray TALIESIN_NO_EXEC in the ambient env would skip the kernel entirely.
    c.env_remove("TALIESIN_NO_EXEC");
    c
}

/// The interpreter the run actually chose, read off the `<- used` marker in the ordered
/// resolution report the kernel failure prints.
fn interpreter_used(stderr: &str) -> String {
    stderr
        .lines()
        .find(|l| l.contains("<- used"))
        .unwrap_or_else(|| panic!("the failure must print the resolution report:\n{stderr}"))
        .to_string()
}

#[test]
fn a_single_file_build_inside_a_project_uses_its_python_pin() {
    let dir = fixture("single");
    let out = taliesin()
        .arg("build")
        .arg(dir.join("posts").join("p.tmd"))
        .arg("--stdout")
        .output()
        .expect("run taliesin");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let used = interpreter_used(&stderr);
    assert!(
        used.contains(PINNED),
        "`build <project>/posts/p.tmd` ignored the project's `python:` pin and resolved \
         its own interpreter: {used}\n{stderr}"
    );
}

#[test]
fn the_site_build_of_the_same_project_agrees() {
    // The control row, and the reason the case above is a defect rather than a policy: the
    // two verbs read one project and must land on one interpreter. A green run here with a
    // red run above is exactly the divergence; a red run here would mean the pin is not
    // honoured anywhere and the other test is asserting the wrong thing.
    let dir = fixture("site");
    let out = taliesin()
        .arg("build")
        .arg(&dir)
        .output()
        .expect("run taliesin");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let used = interpreter_used(&stderr);
    assert!(
        used.contains(PINNED),
        "the site build must honour `_site.yml python:`: {used}\n{stderr}"
    );
}
