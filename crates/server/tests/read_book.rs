//! `taliesin read` is book-aware: a chapter inside a `_site.yml` project resolves its
//! chapter-scoped numbering and cross-page references (item 19, book scoping), reusing
//! `corpus/course/` (em.tmd = Chapter 3; mle.tmd = Chapter 2).

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
fn read_of_a_book_chapter_resolves_scoped_and_cross_page_refs() {
    let (ok, stdout, stderr) = run(&["read", &corpus("course/em.tmd")]);
    assert!(ok, "`read` should succeed; stderr: {stderr}");
    // Same-chapter theorem is chapter-scoped (em is Chapter 3): "Theorem 3.1", not "Theorem 1".
    assert!(
        stdout.contains("Theorem 3.1"),
        "chapter-scoped number:\n{stdout}"
    );
    // Cross-page refs resolve: @thm-consistency (mle, ch 2) -> "Theorem 2.1";
    // @sec-mle (mle's chapter H1) -> "Chapter 2".
    assert!(
        stdout.contains("Theorem 2.1"),
        "cross-page theorem resolved:\n{stdout}"
    );
    assert!(
        stdout.contains("Chapter 2"),
        "cross-page section reads Chapter 2:\n{stdout}"
    );
    // The pre-fix bug: bare "Recall Theorem from Section" must be gone.
    assert!(
        !stdout.contains("Recall Theorem from Section"),
        "cross-page refs are no longer bare:\n{stdout}"
    );
}
