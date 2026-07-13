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
    // Point --out at a scratch dir so this test's blast radius never depends on the very
    // ordering it checks: if the token fail-fast ever regressed to run after the build,
    // the build would write here (a temp dir), not into the tracked corpus directory.
    let out = std::env::temp_dir().join(format!("tali-pub-notoken-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let res = Command::new(bin())
        .args(["publish", "corpus/demo-book", "--out"])
        .arg(&out)
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
    // The fail-fast happens before any build, so nothing should have been written.
    assert!(
        !out.exists(),
        "token fail-fast must happen before the build (no output dir)"
    );
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn public_flag_skips_the_gate_and_warns() {
    let out = std::env::temp_dir().join(format!("tali-pub-public-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let res = Command::new(bin())
        .args(["publish", "corpus/demo-book", "--out"])
        .arg(&out)
        .args(["--public", "--dry-run"])
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .output()
        .expect("run publish --public --dry-run");
    assert!(
        res.status.success(),
        "public dry-run should succeed: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    // No passcode gate injected.
    assert!(
        !out.join("functions").join("_middleware.js").exists(),
        "--public must not inject the gate"
    );
    // A loud PUBLIC warning was printed.
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("PUBLIC"),
        "must warn that the site is public: {stderr}"
    );
    // The site still built.
    assert!(out.join("index.html").exists(), "site built to out");
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn gate_false_config_skips_the_gate() {
    // A throwaway one-page site with publish.gate: false.
    let root = std::env::temp_dir().join(format!("tali-pub-gatecfg-{}", std::process::id()));
    let out = std::env::temp_dir().join(format!("tali-pub-gatecfg-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("_site.yml"),
        "title: Pub\npublish:\n  gate: false\n",
    )
    .unwrap();
    std::fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
    let res = Command::new(bin())
        .args(["publish"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .arg("--dry-run")
        .output()
        .expect("run publish --dry-run");
    assert!(
        res.status.success(),
        "dry-run should succeed: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    assert!(
        !out.join("functions").join("_middleware.js").exists(),
        "publish.gate: false must not inject the gate"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

/// Write a throwaway one-page site whose only page links a page that does not exist,
/// so the strict check has a real problem to fail on. Returns the site root.
fn broken_site(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("tali-pub-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("_site.yml"), "title: Broken\n").unwrap();
    std::fs::write(
        root.join("index.tmd"),
        "---\ntitle: Home\n---\n\nSee [the missing page](nonexistent.tmd).\n",
    )
    .unwrap();
    root
}

#[test]
fn strict_by_default_fails_a_broken_site() {
    let root = broken_site("strict");
    let out = std::env::temp_dir().join(format!("tali-pub-strict-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let res = Command::new(bin())
        .args(["publish"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .arg("--dry-run")
        .output()
        .expect("run publish --dry-run");
    assert!(
        !res.status.success(),
        "a broken cross-ref must fail the strict-by-default deploy: {}",
        String::from_utf8_lossy(&res.stdout)
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn no_strict_lets_a_broken_site_through() {
    let root = broken_site("nostrict");
    let out = std::env::temp_dir().join(format!("tali-pub-nostrict-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let res = Command::new(bin())
        .args(["publish"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .args(["--no-strict", "--dry-run"])
        .output()
        .expect("run publish --no-strict --dry-run");
    assert!(
        res.status.success(),
        "--no-strict must let the broken site build + (dry) deploy: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

// Silence dead_code on the helper if only one test uses it in some configs.
#[allow(dead_code)]
fn _root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
