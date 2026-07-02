use std::process::Command;

/// `--version` prints the bumped semver plus a parenthesized build colophon.
#[test]
fn version_flag_prints_semver_and_colophon() {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("--version")
        .output()
        .expect("the taliesin binary should run");
    assert!(out.status.success(), "exit status: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Assert against the crate version rather than a hard-coded string, so a
    // version bump never breaks this test.
    let expected = format!("taliesin {} (", env!("CARGO_PKG_VERSION"));
    assert!(
        stdout.starts_with(&expected),
        "expected `{expected}<sha>)`, got: {stdout:?}"
    );
    assert!(
        stdout.trim_end().ends_with(')'),
        "colophon should be wrapped in parentheses, got: {stdout:?}"
    );
}
