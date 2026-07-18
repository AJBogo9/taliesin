//! `taliesin doctor` end-to-end: a standalone environment self-audit that runs unconditionally
//! (not gated on a doc's languages, unlike `check`'s Environment section). The severity model
//! (✓ ready / ⚠ fixable / ✗ configured-and-broken) and the exit code go through the real
//! binary, so these shell out through `CARGO_BIN_EXE_taliesin`.
use std::process::Command;

fn taliesin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_taliesin"))
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("tali-doctor-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The audit reports the Python + R interpreter and the active-env line, unconditionally
/// (no document needed). Run in a clean dir with the kernel env vars removed for determinism.
#[test]
fn doctor_reports_interpreters_and_active_env() {
    let dir = tmp("basic");
    let out = taliesin()
        .arg("doctor")
        .arg(&dir)
        .env_remove("TALIESIN_PYTHON")
        .env_remove("TALIESIN_R")
        .output()
        .expect("run doctor");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("python"),
        "reports a python line:\n{stdout}"
    );
    assert!(
        stdout.contains("ipykernel"),
        "names the python kernel pkg (present / MISSING / install fix):\n{stdout}"
    );
    assert!(
        stdout.contains("IRkernel"),
        "names the R kernel pkg:\n{stdout}"
    );
    assert!(
        stdout.contains("env"),
        "reports the active-env line:\n{stdout}"
    );
    assert!(
        stdout.contains("cells will"),
        "prints a readiness summary:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--format json` emits a parseable `{ok, checks:[{name,status,...}]}` for an agent.
#[test]
fn doctor_json_lists_the_checks() {
    let dir = tmp("json");
    let out = taliesin()
        .arg("doctor")
        .arg(&dir)
        .args(["--format", "json"])
        .env_remove("TALIESIN_PYTHON")
        .output()
        .expect("run doctor --format json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is valid JSON");
    assert!(v["ok"].is_boolean(), "has a top-level ok: {v}");
    let checks = v["checks"].as_array().expect("checks array");
    for name in ["python", "r", "env"] {
        assert!(
            checks.iter().any(|c| c["name"] == name),
            "json has a `{name}` check: {v}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A configured-but-broken interpreter (a bad `TALIESIN_PYTHON`) is a hard failure: exit
/// non-zero, with a ✗ naming the failure. (An absent *default* interpreter would only warn.)
#[test]
fn doctor_fails_on_a_broken_configured_python() {
    let dir = tmp("broken");
    let out = taliesin()
        .arg("doctor")
        .arg(&dir)
        .env("TALIESIN_PYTHON", "/nonexistent/tali/python-xyz")
        .output()
        .expect("run doctor");
    assert!(
        !out.status.success(),
        "a broken TALIESIN_PYTHON must exit non-zero"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains('✗') || stdout.contains("cannot run"),
        "shows the interpreter error:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
