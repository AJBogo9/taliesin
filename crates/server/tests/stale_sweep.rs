//! A `_site` build mirrors the source tree, so a page or asset removed/renamed in the
//! source must not linger in the output across rebuilds. The build sweeps the stale
//! files it no longer produces — but leaves the dot/underscore deploy metadata it never
//! emits (`.nojekyll`, `_headers`, a `.git` worktree), which the author placed on purpose.

use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-sweep-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(dir: &std::path::Path, out: &std::path::Path) {
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(dir)
        .arg("--out")
        .arg(out)
        .output()
        .expect("run build");
    assert!(
        res.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&res.stderr)
    );
}

#[test]
fn rebuild_sweeps_removed_page_and_asset_but_keeps_deploy_metadata() {
    let dir = tmp_dir("removed");
    let out = dir.join("out");
    fs::write(dir.join("_site.yml"), "title: Sweep probe\n").unwrap();
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
    fs::create_dir_all(dir.join("posts")).unwrap();
    fs::write(
        dir.join("posts").join("old.tmd"),
        "---\ntitle: Old\n---\n\nGoing away.\n",
    )
    .unwrap();
    fs::write(dir.join("logo.png"), b"not-a-real-png").unwrap();

    build(&dir, &out);
    assert!(
        out.join("posts").join("old.html").exists(),
        "first build writes the page"
    );
    assert!(
        out.join("logo.png").exists(),
        "first build mirrors the asset"
    );
    assert!(out.join("index.html").exists());

    // Deploy metadata the build never emits (dot/underscore) must survive a rebuild.
    fs::write(out.join(".nojekyll"), "").unwrap();
    fs::write(out.join("_headers"), "/*\n  X-Frame-Options: DENY\n").unwrap();

    // Remove a page + an asset from the source, then rebuild.
    fs::remove_file(dir.join("posts").join("old.tmd")).unwrap();
    fs::remove_file(dir.join("logo.png")).unwrap();
    build(&dir, &out);

    assert!(
        !out.join("posts").join("old.html").exists(),
        "a removed page must be swept from the output"
    );
    assert!(
        !out.join("logo.png").exists(),
        "a removed asset must be swept from the output"
    );
    assert!(
        !out.join("posts").exists(),
        "an emptied output directory is pruned"
    );
    assert!(
        out.join("index.html").exists(),
        "a surviving page is still built"
    );
    assert!(
        out.join(".nojekyll").exists(),
        ".nojekyll (deploy metadata) must survive the sweep"
    );
    assert!(
        out.join("_headers").exists(),
        "_headers (deploy metadata) must survive the sweep"
    );

    let _ = fs::remove_dir_all(&dir);
}
