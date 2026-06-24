use std::process::Command;

/// `--version` prints the bumped semver plus a parenthesized build colophon.
#[test]
fn version_flag_prints_semver_and_colophon() {
    let out = Command::new(env!("CARGO_BIN_EXE_qmd-fast"))
        .arg("--version")
        .output()
        .expect("the qmd-fast binary should run");
    assert!(out.status.success(), "exit status: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with("qmd-fast 0.1.0 ("),
        "expected `qmd-fast 0.1.0 (<sha>)`, got: {stdout:?}"
    );
    assert!(
        stdout.trim_end().ends_with(')'),
        "colophon should be wrapped in parentheses, got: {stdout:?}"
    );
}
