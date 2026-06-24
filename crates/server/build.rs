//! Emit a short git SHA so the CLI can print a build colophon. Falls back to
//! "unknown" outside a git checkout (e.g. a packaged crate), so the build never
//! fails for lack of git.
use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=QMD_FAST_GIT_SHA={sha}");
    // Re-run when the checked-out commit moves.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
}
