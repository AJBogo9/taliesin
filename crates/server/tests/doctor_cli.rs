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

/// The audit reports the Python interpreter unconditionally (no document needed). Run in a
/// clean dir with the kernel env vars removed for determinism.
///
/// It asserted a third line, `env`, until 2026-08-10: that row printed the active
/// conda/virtualenv at a hard-coded ✓ no environment could change, and its content is already
/// in the python line's provenance and `.venv` search trail.
#[test]
fn doctor_reports_the_interpreter_unconditionally() {
    let dir = tmp("basic");
    let out = taliesin()
        .arg("doctor")
        .arg(&dir)
        .env_remove("TALIESIN_PYTHON")
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
    assert!(
        checks.iter().any(|c| c["name"] == "python"),
        "json has a `python` check: {v}"
    );
    // `status` is the field an agent branches on, so it has to be the agreed vocabulary and
    // not merely present. Asserted across every row rather than pinning one row's exact
    // value: the only row whose value was knowable in advance here was `env`, and it was
    // knowable precisely because it could never be anything but "ok" — which is why that row
    // was deleted on 2026-08-10. What an agent needs is that every status it reads is one of
    // the three it knows how to branch on.
    for c in checks {
        assert!(c["name"].is_string(), "every check is named: {c}");
        let status = c["status"].as_str().unwrap_or("");
        assert!(
            matches!(status, "ok" | "warn" | "error"),
            "unknown status {status:?} in {c}"
        );
    }
    // `ok` is the exit code in JSON form: it must agree with the process's own verdict.
    assert_eq!(
        v["ok"].as_bool(),
        Some(out.status.success()),
        "the json verdict and the exit code must agree: {v}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The status column is a glyph, and a piped run carries no colour. Both halves are easy to
/// lose silently: the glyph is the only thing that makes the human output scannable, and
/// escape codes in a piped stream are noise in whatever reads it (the NO_COLOR convention is
/// "and stdout is a terminal", not "or").
#[test]
fn the_human_report_is_glyph_marked_and_uncoloured_when_piped() {
    let dir = tmp("glyphs");
    let out = taliesin()
        .arg("doctor")
        .arg(&dir)
        .env_remove("NO_COLOR")
        .output()
        .expect("run doctor");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Any of the three, not specifically ✓. This asserted a ✓ and passed only because the
    // deleted `env` row was hard-coded to one; with that row gone the surviving rows depend
    // on the machine, and pinning ✓ would make the test assert "this box has a working
    // ipykernel" instead of "the report is glyph-marked", which is the property.
    assert!(
        stdout.contains('✓') || stdout.contains('⚠') || stdout.contains('✗'),
        "every line carries its status glyph:\n{stdout}"
    );
    assert!(
        !stdout.contains('\u{1b}'),
        "a piped run must not emit ANSI escapes:\n{stdout:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// An unrecognized flag is an error, not a directory. Silently treating `--jsonn` as the
/// project path audits the wrong place and exits 0, which reads as "your environment is fine".
#[test]
fn an_unknown_flag_is_rejected() {
    let out = taliesin()
        .arg("doctor")
        .arg("--jsonn")
        .output()
        .expect("run doctor");
    assert!(!out.status.success(), "an unknown flag must exit non-zero");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--jsonn"),
        "the error should name the flag: {stderr}"
    );
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

/// `--format json` carries a package manifest, not only the interpreter checks.
///
/// `doctor` audited interpreter *presence* — "ipykernel MISSING" — and never "which pandas",
/// which is the half that decides whether a document reproduces on another machine. It is
/// also the axis `_freeze/`'s cumulative key structurally cannot see: an in-place
/// `pip install --upgrade` leaves every key unchanged.
///
/// Skipped rather than failed when no interpreter answers: this is a real probe of the
/// machine it runs on, and a CI box with no Python is not a regression.
#[test]
fn doctor_json_carries_a_package_manifest() {
    let dir = tmp("packages");
    let out = taliesin()
        .args(["doctor", dir.to_str().unwrap(), "--format", "json"])
        .output()
        .expect("run doctor");
    let text = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    let packages = v["packages"]
        .as_object()
        .expect("a `packages` object, even when empty");
    if packages.is_empty() {
        eprintln!("no interpreter answered here; the manifest shape is still asserted above");
        return;
    }
    for (lang, m) in packages {
        assert!(
            m["digest"].as_str().is_some_and(|d| d.len() == 16),
            "{lang}: a 16-hex-digit digest, which is what `_freeze/` records: {m}"
        );
        let listed = m["packages"].as_object().expect("name → version");
        assert!(
            !listed.is_empty(),
            "{lang}: an interpreter that answered must list something — an empty manifest \
             would hash the same as a different empty one and report a false match"
        );
        for (name, version) in listed {
            assert!(
                version.is_string(),
                "{lang}: {name} has no version, which is the whole thing being recorded"
            );
        }
    }
}
