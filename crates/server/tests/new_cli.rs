//! `taliesin new post <slug>` scaffolds a document that is correct on the first save: it
//! renders, and `taliesin check` passes on it with no diagnostics.
//!
//! The blank-page tax was previously paid *outside* the tool, by a hand-written scaffolder
//! skill under `corpus/tech-blog/.claude/skills/new-post/` (since retired), which had rotted:
//! it emitted `.qmd` and said `quarto preview`. A scaffolder that lives outside the binary
//! cannot be checked against the binary's own vocabulary.
//!
//! What `new` writes was also pinned byte-for-byte by `corpus/scaffold/` until Wave 8. This
//! file is what remains, and it is the stronger half: it runs the real binary and then the
//! real `check`, so the scaffold cannot emit a front-matter key the validator would reject.

use std::path::Path;
use std::process::Command;

fn tmp(name: &str) -> std::path::PathBuf {
    // A per-call sequence, not just `(name, pid)`: two tests reusing a name (e.g. the
    // scaffold-matrix loop's `tmp("paper")` and `a_paper_ships_...`'s `tmp("paper")`)
    // otherwise share ONE dir and clobber each other's scaffold when run in parallel —
    // a load-sensitive collision that fails `check` on a half-written or deleted file.
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tali-new-{}-{}-{}", name, std::process::id(), seq));
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
        .args(["build", path.to_str().unwrap(), "--check-only"])
        .output()
        .expect("run the lint");
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    (out.status.success(), err)
}

/// The whole point: what `new` writes must survive the tool's own preflight.
#[test]
fn every_scaffold_passes_check_with_no_diagnostics() {
    let dir = tmp("post");
    let (ok, stdout, stderr) = run(&[
        "new",
        "post",
        "my-first-post",
        "--dir",
        dir.to_str().unwrap(),
    ]);
    assert!(ok, "`new post` should succeed; stderr: {stderr}");
    let written = dir.join("posts/my-first-post/index.tmd");
    assert!(
        written.exists(),
        "`new post` writes posts/<slug>/index.tmd; stdout: {stdout}"
    );

    let (clean, diagnostics) = check_is_clean(&written);
    assert!(
        clean,
        "`taliesin check` must pass on a fresh `new post`, got:\n{diagnostics}"
    );
    // And it tells the author what to do next.
    assert!(
        stdout.contains("taliesin preview"),
        "`new post` should print the preview hint; got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A kind this verb used to scaffold says what to do instead, rather than falling through to
/// "unknown kind": every one of them is edit-distance 3 or more from `post`, so the
/// did-you-mean rule cannot see them and would have answered a removal with silence.
#[test]
fn a_retired_kind_says_what_to_do_instead() {
    let dir = tmp("retired-kind");
    for (kind, needle) in [
        ("page", "front matter"),
        ("paper", "bibliography:"),
        ("deck", "slide-deck engine"),
    ] {
        let (ok, _, stderr) = run(&["new", kind, "x", "--dir", dir.to_str().unwrap()]);
        assert!(!ok, "`new {kind}` must fail");
        assert!(
            stderr.contains(needle),
            "`new {kind}` should say what replaces it; got: {stderr}"
        );
        assert!(
            !stderr.contains("did you mean"),
            "a removal is not a misspelling; got: {stderr}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--json` prints a machine receipt (`{kind, slug, created, preview}`) and nothing else on
/// stdout, so an agent knows exactly what it made and where.
#[test]
fn new_json_reports_what_it_made() {
    let dir = tmp("json");
    let (ok, stdout, stderr) = run(&[
        "new",
        "post",
        "my-post",
        "--dir",
        dir.to_str().unwrap(),
        "--json",
    ]);
    assert!(ok, "stderr: {stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout is pure JSON");
    assert_eq!(parsed["kind"], "post");
    assert_eq!(parsed["slug"], "my-post");
    let created = parsed["created"].as_array().expect("created array");
    assert_eq!(created.len(), 1, "a post creates one index.tmd");
    assert!(
        created[0].as_str().unwrap().ends_with("index.tmd"),
        "created names the file: {stdout}"
    );
    assert!(
        parsed["preview"]
            .as_str()
            .unwrap()
            .contains("taliesin preview"),
        "preview command present: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--draft` marks a scaffold `draft: true` (held out of the published build) and the doc still
/// checks clean; without the flag, no `draft:` key appears (the default scaffold is unchanged).
#[test]
fn new_post_draft_marks_it_a_draft_and_stays_clean() {
    let dir = tmp("draft");
    let (ok, _, stderr) = run(&[
        "new",
        "post",
        "wip",
        "--draft",
        "--dir",
        dir.to_str().unwrap(),
    ]);
    assert!(ok, "stderr: {stderr}");
    let src = std::fs::read_to_string(dir.join("posts/wip/index.tmd")).unwrap();
    assert!(
        src.contains("draft: true"),
        "--draft sets draft: true:\n{src}"
    );
    let (clean, diags) = check_is_clean(&dir.join("posts/wip/index.tmd"));
    assert!(clean, "a fresh --draft post must check clean:\n{diags}");

    // Default (no flag) stays draft-free, so the corpus mirror + existing scaffolds are unchanged.
    let (ok2, ..) = run(&["new", "post", "published", "--dir", dir.to_str().unwrap()]);
    assert!(ok2);
    let plain = std::fs::read_to_string(dir.join("posts/published/index.tmd")).unwrap();
    assert!(
        !plain.contains("draft:"),
        "no --draft → no draft key:\n{plain}"
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
    let (ok, ..) = run(&["new", "post", "about", "--dir", dir.to_str().unwrap()]);
    assert!(ok);
    let page = dir.join("posts/about/index.tmd");
    std::fs::write(&page, "MY WORK").unwrap();

    let (ok2, _, stderr) = run(&["new", "post", "about", "--dir", dir.to_str().unwrap()]);
    assert!(!ok2, "a second `new post about` must fail");
    assert!(stderr.contains("already exists"), "got: {stderr}");
    assert_eq!(
        std::fs::read_to_string(&page).unwrap(),
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
        let (ok, _, stderr) = run(&["new", "post", slug, "--dir", dir.to_str().unwrap()]);
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

/// A scaffold, inside a site or out of one, gets the same plain "Preview it:" line. The
/// deck kind was the one exception (a deck was a component of a page, not a page), and it
/// went with the slide-deck engine on 2026-08-08 — so "one kind of advice" is now the whole
/// rule, and this is what notices if a second exception creeps back in.
#[test]
fn a_scaffold_keeps_the_plain_preview_advice() {
    let loose = tmp("post-loose");
    let (ok, stdout, stderr) = run(&["new", "post", "solo", "--dir", loose.to_str().unwrap()]);
    assert!(ok, "{stderr}");
    assert!(
        stdout.contains("Preview it:") && !stdout.contains("embed"),
        "a post outside a site previews directly, with no embed advice:\n{stdout}"
    );

    let site = tmp("post-in-site");
    let (ok_init, ..) = run(&["init", site.to_str().unwrap()]);
    assert!(ok_init);
    let (ok2, stdout2, stderr2) = run(&["new", "post", "hello", "--dir", site.to_str().unwrap()]);
    assert!(ok2, "{stderr2}");
    assert!(
        stdout2.contains("Preview it:") && !stdout2.contains("embed"),
        "a post IS a page of the site, so its advice is unchanged:\n{stdout2}"
    );
    let _ = std::fs::remove_dir_all(&loose);
    let _ = std::fs::remove_dir_all(&site);
}
