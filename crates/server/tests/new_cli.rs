//! `taliesin new <post|page|deck> <slug>` scaffolds a document that is correct on the
//! first save: it renders, and `taliesin check` passes on it with no diagnostics.
//!
//! The blank-page tax was previously paid *outside* the tool, by a hand-written scaffolder
//! skill under `corpus/tech-blog/.claude/skills/new-post/` (since retired), which had rotted:
//! it emitted `.qmd` and said `quarto preview`. A scaffolder that lives outside the binary
//! cannot be checked against the binary's own vocabulary.
//!
//! What each `new` writes is pinned byte-for-byte by `corpus/scaffold/`, which the corpus
//! regression net renders and lints like any other document. So the scaffold cannot emit a
//! front-matter key the validator would reject: `cargo test -p taliesin-core` would fail.

use std::path::Path;
use std::process::Command;

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-new-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin new");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn check_is_clean(path: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["check", path.to_str().unwrap()])
        .output()
        .expect("run taliesin check");
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    (out.status.success(), err)
}

/// The whole point: what `new` writes must survive the tool's own preflight.
#[test]
fn every_scaffold_passes_check_with_no_diagnostics() {
    for (kind, slug, rel) in [
        ("post", "my-first-post", "posts/my-first-post/index.tmd"),
        ("page", "about", "about.tmd"),
        ("deck", "my-talk", "my-talk.tmd"),
        ("paper", "my-paper", "posts/my-paper/index.tmd"),
    ] {
        let dir = tmp(kind);
        let (ok, stdout, stderr) = run(&["new", kind, slug, "--dir", dir.to_str().unwrap()]);
        assert!(ok, "`new {kind}` should succeed; stderr: {stderr}");
        let written = dir.join(rel);
        assert!(
            written.exists(),
            "`new {kind}` writes {rel}; stdout: {stdout}"
        );

        let (clean, diagnostics) = check_is_clean(&written);
        assert!(
            clean,
            "`taliesin check` must pass on a fresh `new {kind}`, got:\n{diagnostics}"
        );
        // And it tells the author what to do next.
        assert!(
            stdout.contains("taliesin preview"),
            "`new {kind}` should print the preview hint; got: {stdout}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A `paper` scaffolds a citation-wired doc AND the `.bib` its `[@key]` resolves against,
/// so a research paper is check-clean on the first save (not just a blank page).
#[test]
fn a_paper_ships_its_bibliography_so_citations_resolve() {
    let dir = tmp("paper");
    let (ok, _, stderr) = run(&["new", "paper", "my-paper", "--dir", dir.to_str().unwrap()]);
    assert!(ok, "stderr: {stderr}");
    let index = dir.join("posts/my-paper/index.tmd");
    let bib = dir.join("posts/my-paper/references.bib");
    assert!(index.exists() && bib.exists(), "paper writes both files");
    let src = std::fs::read_to_string(&index).unwrap();
    assert!(
        src.contains("bibliography: [references.bib]"),
        "declares its bib"
    );
    assert!(src.contains("[@knuth1984literate]"), "cites a real key");
    // A worked example, not a blank page: a runnable figure cell whose Quarto-style cell options
    // (`#| label:`/`#| fig-cap:`) cross-reference automatically, plus display math.
    assert!(
        src.contains("```{python}"),
        "paper shows a runnable code cell"
    );
    assert!(
        src.contains("#| label: fig-"),
        "paper labels a figure the Quarto way"
    );
    assert!(src.contains("@fig-"), "paper cross-references its figure");
    assert!(src.contains("$$"), "paper shows display math");
    let (clean, diagnostics) = check_is_clean(&index);
    assert!(clean, "a fresh paper must check clean, got:\n{diagnostics}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--json` prints a machine receipt (`{kind, slug, created, preview}`) and nothing else on
/// stdout, so an agent knows exactly what it made and where.
#[test]
fn new_json_reports_what_it_made() {
    let dir = tmp("json");
    let (ok, stdout, stderr) = run(&[
        "new",
        "paper",
        "my-paper",
        "--dir",
        dir.to_str().unwrap(),
        "--json",
    ]);
    assert!(ok, "stderr: {stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout is pure JSON");
    assert_eq!(parsed["kind"], "paper");
    assert_eq!(parsed["slug"], "my-paper");
    let created = parsed["created"].as_array().expect("created array");
    assert_eq!(created.len(), 2, "paper creates index.tmd + references.bib");
    assert!(
        parsed["preview"]
            .as_str()
            .unwrap()
            .contains("taliesin preview"),
        "preview command present: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A post's date is today's, not a placeholder the author must remember to change.
#[test]
fn a_new_post_is_dated_today() {
    let dir = tmp("dated");
    let (ok, _, stderr) = run(&["new", "post", "dated", "--dir", dir.to_str().unwrap()]);
    assert!(ok, "stderr: {stderr}");
    let src = std::fs::read_to_string(dir.join("posts/dated/index.tmd")).unwrap();
    let date = src
        .lines()
        .find_map(|l| l.strip_prefix("date: "))
        .expect("a post carries a date");
    assert_eq!(date.len(), 10, "date is YYYY-MM-DD, got `{date}`");
    assert!(
        date.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 {
            c == '-'
        } else {
            c.is_ascii_digit()
        }),
        "date is YYYY-MM-DD, got `{date}`"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `init`'s refuse-before-overwrite discipline: never clobber the author's work.
#[test]
fn an_existing_file_is_never_overwritten() {
    let dir = tmp("clobber");
    let (ok, ..) = run(&["new", "page", "about", "--dir", dir.to_str().unwrap()]);
    assert!(ok);
    std::fs::write(dir.join("about.tmd"), "MY WORK").unwrap();

    let (ok2, _, stderr) = run(&["new", "page", "about", "--dir", dir.to_str().unwrap()]);
    assert!(!ok2, "a second `new page about` must fail");
    assert!(stderr.contains("already exists"), "got: {stderr}");
    assert_eq!(
        std::fs::read_to_string(dir.join("about.tmd")).unwrap(),
        "MY WORK",
        "the author's file must be untouched"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_kind_is_rejected_with_a_did_you_mean() {
    let dir = tmp("kind");
    let (ok, _, stderr) = run(&["new", "pots", "x", "--dir", dir.to_str().unwrap()]);
    assert!(!ok, "an unknown kind must fail");
    assert!(
        stderr.contains("post"),
        "expected a did-you-mean; got: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_slug_that_escapes_the_project_is_rejected() {
    let dir = tmp("escape");
    for slug in ["../evil", "a/b", ""] {
        let (ok, _, stderr) = run(&["new", "page", slug, "--dir", dir.to_str().unwrap()]);
        assert!(!ok, "slug `{slug}` must be rejected");
        assert!(!stderr.is_empty(), "slug `{slug}` should explain itself");
    }
    assert!(!dir.join("..").join("evil.tmd").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_argument_prints_usage() {
    let (ok, _, stderr) = run(&["new"]);
    assert!(!ok);
    assert!(stderr.contains("usage: taliesin new"), "got: {stderr}");
    let (ok2, _, stderr2) = run(&["new", "post"]);
    assert!(!ok2);
    assert!(stderr2.contains("usage: taliesin new"), "got: {stderr2}");
}
