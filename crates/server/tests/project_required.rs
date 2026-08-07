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
