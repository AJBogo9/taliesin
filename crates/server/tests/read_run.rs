//! `taliesin read --run` executes python/r cells and projects what each produced, so a
//! headless agent can tell its figure/table baked, its cell printed, or its cell errored.
//!
//! Kernel-free cases (parsing, backward-compat, no-exec) run unconditionally. The
//! executed-projection cases are gated on `TALIESIN_PYTHON`; `TALIESIN_REQUIRE_KERNEL=1`
//! (the CI kernel job) turns the skip into a hard failure so this coverage can't silently
//! regress to zero.

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
    // A doc with python cells, read WITHOUT --run: cells project as source and the
    // "projected as source" warning fires on stderr.
    let out = run(&["read", &corpus("agent/executed-read.tmd")]);
    assert!(out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("projected as source"),
        "expected the no-run warning: {err}"
    );
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
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn read_json_without_run_marks_cells_not_run() {
    let out = run(&[
        "read",
        &corpus("agent/executed-read.tmd"),
        "--format",
        "json",
    ]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(v["executed"], false);
    let cells = v["cells"].as_array().expect("cells array");
    assert!(!cells.is_empty(), "python cells are listed");
    assert!(
        cells.iter().all(|c| c["kind"] == "not-run"),
        "no --run -> not-run: {v}"
    );
    assert!(v["text"].is_string(), "text projection included");
}

/// `Some(python)` when a python kernel is configured, `None` to skip — unless
/// `TALIESIN_REQUIRE_KERNEL=1`, which makes a missing interpreter a hard failure.
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
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("[figure fig-hist: produced"),
        "figure not reported: {text}"
    );
    assert!(
        text.contains("alt \"A histogram of sampled scores\""),
        "alt missing: {text}"
    );
    assert!(
        text.contains("[cell error:"),
        "cell error not reported: {text}"
    );
}

#[test]
fn read_run_json_reports_produced_and_error_kinds() {
    let Some(py) = python() else { return };
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args([
            "read",
            "--run",
            "--format",
            "json",
            &corpus("agent/executed-read.tmd"),
        ])
        .env("TALIESIN_PYTHON", &py)
        .output()
        .expect("run taliesin");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(v["executed"], true);
    let cells = v["cells"].as_array().unwrap();
    assert!(
        cells
            .iter()
            .any(|c| c["kind"] == "figure" && c["fig_id"] == "fig-hist"),
        "figure cell missing: {v}"
    );
    assert!(
        cells
            .iter()
            .any(|c| c["kind"] == "error" && c["produced"] == false),
        "error cell missing: {v}"
    );
}
