//! `mounts:` in the static build (item 149, closed by Wave 6).
//!
//! A `mounts:` entry is another project served under a URL prefix. For a long time only
//! `preview` wired it: the static build rendered the parent's own pages and nothing else,
//! so every link into a mounted prefix 404'd in the deploy. That is not hypothetical — it
//! is how this project's own marketing site shipped with its primary call-to-action dead.
//!
//! The first fix was a diagnostic (`TAL-MOUNT-PREVIEW`) plus a shell script beside the site
//! that ran one `build … --out <out>/<at>` per mount. `build` does that itself now, so both
//! are gone and this file is the pin that replaced them: it was
//! `mount_preview_is_gated.rs`, asserting the warning fired; it now asserts the build
//! produces the tree the warning used to describe.
//!
//! The order claim below is inherited from the deleted `site_build_script.rs`, which pinned
//! it against the script. It is the one thing here that is easy to get wrong and silent
//! when wrong, so it outlived the file that held it.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A site with one `mounts:` entry pointing at a real sibling project, plus the mounted
/// project itself. Returns the site root.
fn site_with_a_mount(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("tali-mount-build-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let site = base.join("site");
    let mounted = base.join("manual");
    std::fs::create_dir_all(&site).unwrap();
    std::fs::create_dir_all(&mounted).unwrap();
    std::fs::write(
        site.join("_site.yml"),
        "title: S\nmounts:\n  manual: ../manual\n",
    )
    .unwrap();
    std::fs::write(
        site.join("index.tmd"),
        "---\ntitle: Home\n---\n\nRead the [manual](/manual/).\n",
    )
    .unwrap();
    std::fs::write(mounted.join("_site.yml"), "title: M\n").unwrap();
    std::fs::write(
        mounted.join("index.tmd"),
        "---\ntitle: Manual\n---\n\nThe manual.\n",
    )
    .unwrap();
    site
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn build(site: &Path, out: &Path) -> (bool, String) {
    let (ok, _o, stderr) = run(&[
        "build",
        site.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    (ok, stderr)
}

/// The feature: one `build` produces the parent **and** the tree behind its mount prefix,
/// so the link the parent writes has a file behind it.
#[test]
fn a_build_writes_the_mounted_project_under_its_prefix() {
    let site = site_with_a_mount("writes");
    let out = site.join("_site");
    let (ok, stderr) = build(&site, &out);

    assert!(ok, "the build must succeed: {stderr}");
    assert!(
        out.join("index.html").exists(),
        "the parent's own pages are written: {stderr}"
    );
    assert!(
        out.join("manual/index.html").exists(),
        "and the mount lands at <out>/<at>/, which is where the parent links: {stderr}"
    );
    let home = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(
        home.contains("/manual/"),
        "the parent really does link into the prefix, or this test proves nothing"
    );
}

/// The order claim, and the only way to observe it: build twice into the same directory.
///
/// The parent's `sweep_stale` deletes everything under the output it did not itself write,
/// and a mount's directory is neither dot- nor underscore-prefixed nor a symlink, so it is
/// not exempt. Build the mounts before the parent and the second build silently deletes
/// them — a green exit, a 404'ing tree, exactly the failure this whole feature exists to
/// end.
#[test]
fn a_rebuild_does_not_sweep_the_mount_back_out() {
    let site = site_with_a_mount("rebuild");
    let out = site.join("_site");

    let (ok, stderr) = build(&site, &out);
    assert!(ok, "first build: {stderr}");
    assert!(
        out.join("manual/index.html").exists(),
        "first build wrote it"
    );

    let (ok, stderr) = build(&site, &out);
    assert!(ok, "second build: {stderr}");
    assert!(
        out.join("manual/index.html").exists(),
        "the parent's sweep ran and the mount was rebuilt after it: {stderr}"
    );
}

/// `--strict` no longer fails a site merely for *having* mounts.
///
/// It used to, deliberately: the mount was genuinely missing from the artifact, so a strict
/// build was right to refuse. Now it is there, and a gate that still fired would be telling
/// the author to fix something the tool already did.
#[test]
fn strict_no_longer_fails_a_site_just_for_having_a_mount() {
    let site = site_with_a_mount("strict");
    let out = site.join("_site");
    let (ok, _o, stderr) = run(&[
        "build",
        site.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--strict",
    ]);
    assert!(
        ok,
        "a mount is part of the artifact now, not a hole in it: {stderr}"
    );
    assert!(
        out.join("manual/index.html").exists(),
        "and it is on disk: {stderr}"
    );
}

/// The retired diagnostic. It said "this mount is preview-only and its links will 404" and
/// sent the author off to write a shell script; both halves are now false. A stale diagnostic
/// is worse than none, so it is gone from the lint in both output formats.
#[test]
fn the_lint_no_longer_reports_a_mount_as_preview_only() {
    let site = site_with_a_mount("check");
    let (ok, _o, stderr) = run(&["build", site.to_str().unwrap(), "--check-only"]);
    assert!(ok, "a site with a buildable mount is clean: {stderr}");
    assert!(
        !stderr.contains("preview-only"),
        "the retired finding must not survive anywhere in the human output: {stderr}"
    );

    let (_ok, stdout, _e) = run(&[
        "build",
        site.to_str().unwrap(),
        "--check-only",
        "--format",
        "json",
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let messages: Vec<&str> = parsed["diagnostics"]
        .as_array()
        .map(|ds| ds.iter().filter_map(|d| d["message"].as_str()).collect())
        .unwrap_or_default();
    assert!(
        !messages.iter().any(|m| m.contains("preview-only")),
        "nor in the machine output: {stdout}"
    );
}

/// `mounts:` is a graph, not a tree, so the walk needs a cycle guard: a project that mounts
/// itself must build once and stop. Without one this recurses until the disk fills, and the
/// config that triggers it is a single plausible line.
#[test]
fn a_project_that_mounts_itself_terminates() {
    let base = std::env::temp_dir().join(format!("tali-mount-cycle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("_site.yml"), "title: S\nmounts:\n  self: .\n").unwrap();
    std::fs::write(base.join("index.tmd"), "---\ntitle: H\n---\n\nHi.\n").unwrap();
    let out = base.join("out");

    let (ok, stderr) = build(&base, &out);

    assert!(ok, "it still builds what it can: {stderr}");
    assert!(out.join("index.html").exists(), "the pages are written");
    assert!(
        !out.join("self").exists(),
        "and the self-mount was refused rather than recursed into: {stderr}"
    );
}

/// A mount refused by the containment rule (item 80) must not be built either. The refusal
/// exists to stop `mounts: { x: /etc }` from being served; building it would be the tool
/// performing the same escape one step later, and writing someone else's tree into the
/// deploy.
#[test]
fn a_mount_outside_the_boundary_is_not_built() {
    let base = std::env::temp_dir().join(format!("tali-mount-escape-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let site = base.join("deep/site");
    std::fs::create_dir_all(&site).unwrap();
    std::fs::write(
        site.join("_site.yml"),
        "title: S\nmounts:\n  escape: ../../..\n",
    )
    .unwrap();
    std::fs::write(site.join("index.tmd"), "---\ntitle: H\n---\n\nHi.\n").unwrap();
    let out = base.join("out");

    let (_ok, stderr) = build(&site, &out);

    assert!(
        !out.join("escape").exists(),
        "a refused mount gets no output directory: {stderr}"
    );
}

/// The other half of the contract: a project with no mounts must not gain a line, a
/// directory, or a slower build from any of this.
#[test]
fn a_site_with_no_mounts_is_unaffected() {
    // Deliberately NOT named `…-mount-…`: the build echoes its output path, so a fixture
    // directory carrying the word would satisfy the assertion below by accident.
    let base = std::env::temp_dir().join(format!("tali-plain-site-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("_site.yml"), "title: S\n").unwrap();
    std::fs::write(base.join("index.tmd"), "---\ntitle: H\n---\n\nHi.\n").unwrap();
    let out = base.join("_site");

    let (ok, stderr) = build(&base, &out);

    assert!(ok, "clean site builds: {stderr}");
    assert!(
        !stderr.contains("mount"),
        "and says nothing about mounts: {stderr}"
    );
}
