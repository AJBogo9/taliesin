//! `taliesin publish` end to end, without hitting Cloudflare: `--dry-run` builds + gates
//! and prints the exact wrangler command; a real publish fails fast when the API token
//! is absent.

use std::path::Path;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_taliesin")
}

#[test]
fn dry_run_builds_gates_and_prints_the_wrangler_command() {
    let out = std::env::temp_dir().join(format!("tali-pub-dry-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let res = Command::new(bin())
        .args(["publish", "corpus/demo-book", "--out"])
        .arg(&out)
        .arg("--dry-run")
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .output()
        .expect("run publish --dry-run");
    assert!(
        res.status.success(),
        "dry-run should succeed: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    // The passcode gate was injected into the built tree.
    let mw = out.join("functions").join("_middleware.js");
    let body = std::fs::read_to_string(&mw).expect("middleware injected");
    assert!(body.contains("export async function onRequest"), "{body}");
    assert!(body.contains("env.PASSWORD"), "{body}");
    assert!(body.contains("WWW-Authenticate"), "{body}");
    // The exact command is printed (project name = dir slug "demo-book").
    let stdout = String::from_utf8_lossy(&res.stdout);
    assert!(
        stdout.contains(
            "wrangler pages deploy . --project-name demo-book --branch production --commit-dirty=true"
        ),
        "stdout was: {stdout}"
    );
    // The site actually built.
    assert!(out.join("index.html").exists(), "site built to out");
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn real_publish_without_token_fails_fast() {
    let res = Command::new(bin())
        .args(["publish", "corpus/demo-book"])
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .env_remove("CLOUDFLARE_API_TOKEN")
        .output()
        .expect("run publish");
    assert!(!res.status.success(), "must fail without a token");
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("CLOUDFLARE_API_TOKEN"),
        "stderr should name the missing token: {stderr}"
    );
}

// Silence dead_code on the helper if only one test uses it in some configs.
#[allow(dead_code)]
fn _root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
