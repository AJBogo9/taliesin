//! `build` and `preview` render a *project*, and a project is what `_site.yml` declares.
//! A bare directory is refused with guidance, the same stance `read` already takes
//! (`read_of_a_non_site_directory_is_rejected_with_guidance` in `read_book.rs`).

use std::process::Command;

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

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn build_of_a_non_project_directory_is_rejected_with_guidance() {
    let (ok, _out, stderr) = run(&["build", &corpus("agent")]);
    assert!(!ok, "a bare directory (no _site.yml) must fail");
    assert!(stderr.contains("no _site.yml"), "says why: {stderr}");
    assert!(
        stderr.contains("<page>.tmd"),
        "offers the name-one-document fix: {stderr}"
    );
    assert!(
        stderr.contains("add a _site.yml"),
        "offers the make-it-a-project fix: {stderr}"
    );
}

#[test]
fn build_of_a_subdirectory_of_a_project_names_the_project() {
    let (ok, _out, stderr) = run(&["build", &corpus("tech-blog/posts")]);
    assert!(!ok, "a project subdirectory is not itself a project");
    assert!(
        stderr.contains("tech-blog") && stderr.contains("did you mean"),
        "leads with the enclosing project: {stderr}"
    );
}

#[test]
fn build_of_a_real_project_still_works() {
    let (ok, _out, stderr) = run(&["build", &corpus("shared-bib"), "--no-exec"]);
    assert!(
        ok,
        "a directory WITH _site.yml still builds; stderr: {stderr}"
    );
}

#[test]
fn preview_of_a_non_project_directory_is_rejected_with_guidance() {
    // Must fail before binding a port, so this returns rather than serving forever.
    let (ok, _out, stderr) = run(&["preview", &corpus("agent"), "4399"]);
    assert!(!ok, "a bare directory (no _site.yml) must fail");
    assert!(stderr.contains("no _site.yml"), "says why: {stderr}");
    assert!(
        stderr.contains("<page>.tmd"),
        "offers the name-one-document fix: {stderr}"
    );
}

/// The contract: for a document with no ancestor `_site.yml`, what `preview` serves and what
/// `build` writes carry the same chrome. This is the assertion the "Home" button bug failed.
#[test]
fn a_standalone_document_builds_without_site_chrome() {
    let out = std::env::temp_dir().join(format!(
        "tali-standalone-chrome-{}.html",
        std::process::id()
    ));
    let out_s = out.to_string_lossy().into_owned();
    let (ok, _o, stderr) = run(&[
        "build",
        &corpus("agent/executed-read.tmd"),
        &out_s,
        "--no-exec",
    ]);
    assert!(ok, "single-document build; stderr: {stderr}");
    let html = std::fs::read_to_string(&out).expect("built page");
    let _ = std::fs::remove_file(&out);

    for marker in [
        "tali-site-nav",
        "tali-nav-brand",
        "tali-nav-burger",
        "tali-site-footer",
    ] {
        assert!(
            !html.contains(marker),
            "a standalone document must carry no `{marker}`"
        );
    }
    // The reader affordances stay: they are personal, not project, chrome.
    assert!(html.contains("tali-theme-toggle"), "theme toggle survives");
}
