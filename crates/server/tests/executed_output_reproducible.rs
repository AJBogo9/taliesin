//! AP8-1 (backlog item 15): an executed cell's stderr must not leak the kernel's
//! per-process temp path, so a doc with code cells builds **byte-reproducibly**.
//!
//! A Python warning (matplotlib's Agg `UserWarning`, or any `warnings.warn`) cites the
//! kernel cell file `<tmpdir>/ipykernel_<PID>/<HASH>.py`. The PID changes on every build
//! (each build spawns a fresh kernel), so before the fix two builds of the same doc
//! differed on that line — non-reproducible, and a local absolute path leaked into the
//! published HTML. This pins the fix end-to-end through a real kernel.
//!
//! Uses `warnings.warn` rather than a matplotlib figure on purpose: it reproduces the exact
//! same stderr-path leak with only `ipykernel` present (no matplotlib dependency), and it
//! emits NO figure — matplotlib's inline PNG bytes are themselves nondeterministic across
//! builds (see `parallel_build_determinism.rs`), which would mask the property under test.
//!
//! Gated on `TALIESIN_PYTHON` like the rest of the exec tests; `TALIESIN_REQUIRE_KERNEL=1`
//! (the CI kernel job) turns the skip into a hard failure so this coverage can't silently
//! regress to zero.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-ap8-{}-{name}", std::process::id()));
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
                "TALIESIN_REQUIRE_KERNEL=1 but TALIESIN_PYTHON is unset: the executed-output \
                 reproducibility pin would silently skip. Point TALIESIN_PYTHON at a python \
                 with ipykernel."
            );
            eprintln!("skipping: TALIESIN_PYTHON not set (no kernel)");
            None
        }
    }
}

/// Build `src` to `dest` with a fresh kernel every time (`TALIESIN_NO_CACHE=1`), returning
/// the produced HTML bytes.
fn build(src: &Path, dest: &Path, py: &str) -> Vec<u8> {
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
    fs::read(dest).expect("read built html")
}

#[test]
fn a_cell_warning_builds_byte_identically_twice_and_leaks_no_temp_path() {
    let Some(py) = python_or_skip() else {
        return;
    };
    let dir = tmp_dir("warn");
    let src = dir.join("warn.tmd");
    fs::write(
        &src,
        "---\ntitle: AP8-1 pin\n---\n\n\
         ```{python}\nimport warnings\nwarnings.warn(\"repro warning\")\nprint(\"done\")\n```\n",
    )
    .unwrap();

    let first = build(&src, &dir.join("a.html"), &py);
    let second = build(&src, &dir.join("b.html"), &py);
    let html = String::from_utf8(first.clone()).expect("html is utf-8");

    // The warning fired (so the test is not vacuous) and was normalized to a stable marker.
    assert!(
        html.contains("repro warning"),
        "the cell warning is missing — the exec path did not run:\n{html}"
    );
    assert!(
        html.contains("&lt;cell&gt;"),
        "the warning's cell path was not normalized to the `<cell>` marker:\n{html}"
    );
    // No per-process temp path leaked into the published HTML.
    assert!(
        !html.contains("ipykernel_") && !html.contains("/tmp/ipykernel"),
        "a non-deterministic `/tmp/ipykernel_<PID>/…` path leaked into the built HTML:\n{html}"
    );
    // The whole page is byte-identical across two builds with different kernel PIDs.
    assert_eq!(
        first,
        second,
        "two builds of the same code-cell doc differ ({} vs {} bytes) — the executed output \
         is not reproducible; a per-process value (kernel PID / temp path) reached the HTML",
        first.len(),
        second.len(),
    );

    let _ = fs::remove_dir_all(&dir);
}

/// exec #14: a warning or a traceback raised from a module published the absolute path of
/// the file that raised it: `/home/<user>/…/proj/helper.py:4: UserWarning` on stderr, and
/// `File ~/…/proj/helper.py:5` in the traceback (IPython already shortens `$HOME` there).
/// That is the author's home layout in a published page, and a build that differs by
/// machine. Paths inside the project now print relative to it, and the rest of `$HOME` as
/// `~`, in stderr and tracebacks alike.
#[test]
fn a_warning_or_traceback_from_a_project_module_publishes_no_home_path() {
    let Some(py) = python_or_skip() else {
        return;
    };
    let home = tmp_dir("home");
    let proj = home.join("proj");
    fs::create_dir_all(&proj).unwrap();
    fs::write(
        proj.join("helper.py"),
        "import warnings\n\ndef boom():\n    warnings.warn(\"helper warning\")\n    \
         raise ValueError(\"helper failed\")\n",
    )
    .unwrap();
    let src = proj.join("doc.tmd");
    fs::write(
        &src,
        "---\ntitle: exec 14 pin\n---\n\n```{python}\nimport helper\nhelper.boom()\n```\n",
    )
    .unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_taliesin"));
    cmd.arg("build").arg(&src).arg(proj.join("doc.html"));
    cmd.env("TALIESIN_PYTHON", &py);
    cmd.env("TALIESIN_NO_CACHE", "1");
    cmd.env("HOME", &home);
    let out = cmd.output().expect("run build");
    assert!(
        out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = fs::read_to_string(proj.join("doc.html")).expect("read built html");
    assert!(
        html.contains("helper warning") && html.contains("helper failed"),
        "the cell's warning and traceback must reach the page:\n{html}"
    );
    let abs = home.canonicalize().unwrap().display().to_string();
    for leak in [abs.as_str(), "~/proj"] {
        assert!(
            !html.contains(leak),
            "the page publishes the author's home path ({leak}):\n{html}"
        );
    }
    assert!(
        html.contains("helper.py"),
        "the path should stay, relative to the project:\n{html}"
    );
    let _ = fs::remove_dir_all(&home);
}
