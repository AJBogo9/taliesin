//! `taliesin read <file>` projects a rendered document to plain text on stdout, so an
//! agent can read what it made with no browser. Parse-only like `render`: a directory is
//! rejected, and the projection carries resolved cross-reference numbers.

use std::process::Command;

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin read");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn read_projects_a_document_to_plain_text() {
    let (ok, stdout, stderr) = run(&["read", &corpus("reader/text-projection.tmd")]);
    assert!(ok, "`read` should succeed; stderr: {stderr}");
    // Structured text: a heading keeps its level and a cross-reference is resolved.
    assert!(
        stdout.contains("### Overview"),
        "heading projected:\n{stdout}"
    );
    assert!(
        stdout.contains("Figure 1: A scree plot"),
        "resolved figure number:\n{stdout}"
    );
    // No HTML leaks into the projection.
    assert!(
        !stdout.contains("<figure"),
        "no raw HTML in the projection:\n{stdout}"
    );
}

#[test]
fn read_rejects_a_directory() {
    let (ok, _stdout, stderr) = run(&["read", &corpus("reader")]);
    assert!(!ok, "`read` on a directory must fail");
    assert!(!stderr.is_empty(), "it explains why: {stderr}");
}

#[test]
fn read_without_a_path_prints_usage() {
    let (ok, _stdout, stderr) = run(&["read"]);
    assert!(!ok);
    assert!(stderr.contains("usage: taliesin read"), "got: {stderr}");
}
