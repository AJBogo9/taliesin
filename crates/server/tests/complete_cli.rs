//! The hidden `__complete` subcommand drives shell completion: it prints candidate lines
//! then a trailing `:<directive>` line. These tests invoke the real binary the way a shim
//! does, so they cover dispatch wiring + the wire protocol end to end.

use std::process::Command;

/// Run `taliesin __complete <words…>` in `cwd`, returning (candidate values, directive).
fn complete(cwd: &std::path::Path, words: &[&str]) -> (Vec<String>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("__complete")
        .args(words)
        .current_dir(cwd)
        .output()
        .expect("run taliesin __complete");
    assert!(out.status.success(), "exit: {:?}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    let mut values = Vec::new();
    let mut directive = String::new();
    for line in text.lines() {
        if let Some(d) = line.strip_prefix(':') {
            directive = d.to_string();
        } else {
            values.push(line.split('\t').next().unwrap_or("").to_string());
        }
    }
    (values, directive)
}

#[test]
fn completes_subcommands_and_suppresses_files() {
    let (values, directive) = complete(std::path::Path::new("."), &[""]);
    assert!(
        values.contains(&"preview".to_string()),
        "offers preview: {values:?}"
    );
    assert!(
        values.contains(&"build".to_string()),
        "offers build: {values:?}"
    );
    assert_eq!(directive, "4", "NoFileComp for subcommand completion");
}
