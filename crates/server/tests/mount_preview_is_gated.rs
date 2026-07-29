//! Item 149: a `mounts:` entry is served by `preview` and **not** wired into the static
//! build, so every link into it 404s in the deploy. `build` has always said so on stderr —
//! and both automated gates blessed the deploy anyway: `build --strict` exited 0 and
//! `check` printed "no problems found".
//!
//! That is the shape that costs the most: a warning nobody's CI can act on is a warning
//! that gets scrolled past. These pin the two gates.

use std::path::PathBuf;
use std::process::Command;

/// A site with one `mounts:` entry pointing at a real sibling project, plus the mounted
/// project itself. Returns the site root.
fn site_with_a_mount(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("tali-mount-gate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let site = base.join("site");
    let mounted = base.join("manual");
    std::fs::create_dir_all(&site).unwrap();
    std::fs::create_dir_all(&mounted).unwrap();
    std::fs::write(
        site.join("_site.yml"),
        "title: S\nmounts:\n  - at: manual\n    path: ../manual\n",
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

#[test]
fn build_strict_fails_on_a_mount_that_will_404() {
    let site = site_with_a_mount("build-strict");
    let out = site.join("_site");
    let (ok, _o, stderr) = run(&[
        "build",
        site.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--strict",
    ]);
    assert!(
        !ok,
        "--strict must not bless a build whose links 404: {stderr}"
    );
    assert!(
        stderr.contains("preview-only"),
        "and it still says which mount + how to build it: {stderr}"
    );
}

#[test]
fn a_plain_build_still_ships_and_still_warns() {
    // The gate is opt-in. Without `--strict` the mount warning stays a warning: previewing a
    // site with mounts is a legitimate workflow, and `build` writing the pages it *can* build
    // is the useful behaviour. Only the exit code changes under `--strict`.
    let site = site_with_a_mount("build-plain");
    let out = site.join("_site");
    let (ok, _o, stderr) = run(&[
        "build",
        site.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(ok, "a plain build still succeeds: {stderr}");
    assert!(stderr.contains("preview-only"), "and still warns: {stderr}");
    assert!(
        out.join("index.html").exists(),
        "the pages it can build are written"
    );
}

#[test]
fn check_reports_the_mount_instead_of_saying_no_problems_found() {
    // `check` is the pre-publish gate. It used to say "no problems found" on exactly the
    // project whose primary link 404s. It now reports it — as *advice*, because whether this
    // bites depends on whether you deploy the static build, and a site that is only ever
    // previewed is not broken. So the default exit stays 0 and `--strict` is the gate.
    let site = site_with_a_mount("check");
    let (ok, _o, stderr) = run(&["check", site.to_str().unwrap()]);
    assert!(
        !stderr.contains("no problems found"),
        "check no longer blesses a site whose mount will 404: {stderr}"
    );
    assert!(
        stderr.contains("TAL-MOUNT-PREVIEW"),
        "reported under its own stable code: {stderr}"
    );
    assert!(
        ok,
        "…but advice alone does not fail the default gate: {stderr}"
    );

    let (strict_ok, _o, strict_err) = run(&["check", site.to_str().unwrap(), "--strict"]);
    assert!(
        !strict_ok,
        "--strict is the gate that fails on it: {strict_err}"
    );
}

#[test]
fn check_json_carries_the_mount_diagnostic_with_a_suggestion_severity() {
    let site = site_with_a_mount("check-json");
    let (_ok, stdout, _e) = run(&["check", site.to_str().unwrap(), "--format", "json"]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    let d = parsed["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .find(|d| d["code"] == "TAL-MOUNT-PREVIEW")
        .unwrap_or_else(|| panic!("mount diagnostic present: {stdout}"));
    assert_eq!(d["severity"], "suggestion");
    assert!(
        d["message"].as_str().unwrap_or("").contains("manual"),
        "names the mount: {d}"
    );
}

#[test]
fn a_site_with_no_mounts_reports_nothing() {
    // The other half of the contract: this must not become a line every project prints.
    let base = std::env::temp_dir().join(format!("tali-mount-gate-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("_site.yml"), "title: S\n").unwrap();
    std::fs::write(base.join("index.tmd"), "---\ntitle: H\n---\n\nHi.\n").unwrap();
    let (ok, _o, stderr) = run(&["check", base.to_str().unwrap()]);
    assert!(ok, "clean site passes: {stderr}");
    assert!(
        !stderr.contains("TAL-MOUNT-PREVIEW"),
        "no mounts, no mount diagnostic: {stderr}"
    );
}
