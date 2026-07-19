//! The interactive `new`/`init` wizard MUST NOT fire outside a human TTY. Every test here
//! drives the binary with stdin redirected from `/dev/null` (not a terminal), the way CI, a
//! pipe, or an agent does, and asserts the historical non-interactive behavior is unchanged:
//!
//!   * `new` with a missing kind/slug prints its usage line and fails (it does not block on a
//!     prompt), and
//!   * `init` with no `--template` scaffolds the basic one-page site (it does not prompt).
//!
//! If the stdin-is-a-terminal gate ever regressed, the wizard would try to prompt on
//! `/dev/null` and this observable behavior would change — which is what makes these a real
//! guard, not a tautology (mutation-checked by removing the gate).

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn tmp(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tali-wiz-{}-{}-{}", name, std::process::id(), seq));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Run the binary with a non-terminal stdin, so the wizard's TTY gate is exercised the way it
/// is in CI. Setting `/dev/null` explicitly (rather than inheriting) also means this suite can
/// never hang on a prompt even when a developer runs `cargo test` from a terminal.
fn run_non_tty(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("run taliesin");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn new_without_args_is_a_usage_error_when_not_a_tty() {
    let (ok, _out, err) = run_non_tty(&["new"]);
    assert!(
        !ok,
        "bare `new` with no TTY must fail, not prompt; stderr: {err}"
    );
    assert!(
        err.contains("usage:") && err.contains("new"),
        "prints the usage line rather than prompting: {err}"
    );
}

#[test]
fn new_with_a_kind_but_no_slug_is_a_usage_error_when_not_a_tty() {
    let (ok, _out, err) = run_non_tty(&["new", "post"]);
    assert!(
        !ok,
        "`new post` with no slug + no TTY must fail, not prompt; stderr: {err}"
    );
    assert!(err.contains("usage:"), "prints the usage line: {err}");
}

#[test]
fn init_without_a_template_scaffolds_basic_when_not_a_tty() {
    let dir = tmp("init-non-tty");
    let (ok, _out, err) = run_non_tty(&["init", dir.to_str().unwrap()]);
    assert!(
        ok,
        "bare `init` with no TTY must scaffold, not prompt; stderr: {err}"
    );
    assert!(
        dir.join("_site.yml").exists() && dir.join("index.tmd").exists(),
        "scaffolded the basic site"
    );
    // Basic, not a wizard-selected template: no extra pages.
    assert!(
        !dir.join("about.tmd").exists(),
        "did not silently pick the site template"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
