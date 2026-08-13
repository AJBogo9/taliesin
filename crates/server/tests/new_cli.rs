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

/// The same, from a chosen working directory. `new` with no `--dir` scaffolds relative to
/// the CWD, so the CWD is the input under test and cannot be left to whatever the harness
/// happens to run in.
fn run_in(cwd: &Path, args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .current_dir(cwd)
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

/// `new` accepts no `--json`/`--format`: both went on 2026-08-13.
///
/// They printed a `{kind, slug, created, preview}` receipt, appeared nowhere in the manual
/// (`grep -rn -- --json docs --include='*.tmd'` returned nothing, including `new`'s own row
/// in cli.tmd's command table), and `human` was a pure no-op. Meanwhile cli.tmd states that
/// `build --check-only --format json` is "the tool's one machine-readable surface" — there
/// were four. Deleting these made the manual true, which was the cheaper direction.
///
/// Not in `RETIRED_FLAGS`: that register is unscoped and `a_retired_flag_names_what_
/// happened_instead_of_guessing` asserts a retired flag is offered by NO live parser, while
/// `--json`/`--format` are still `build`'s and `doctor`'s. So this is a plain unknown flag,
/// and this test is what records the removal instead of a register entry.
#[test]
fn new_rejects_the_retired_json_flags() {
    let dir = tmp("json");
    for flag in ["--json", "--format"] {
        let (ok, stdout, stderr) = run(&[
            "new",
            "post",
            "my-post",
            "--dir",
            dir.to_str().unwrap(),
            flag,
        ]);
        assert!(!ok, "`new {flag}` must fail, not be silently dropped");
        assert!(
            stderr.contains(flag),
            "the error names the flag typed: {stderr}"
        );
        assert!(
            stdout.trim().is_empty(),
            "a rejected flag writes no receipt to stdout: {stdout}"
        );
    }
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

/// `new post` with no `--dir`, run where no project encloses the CWD, refuses instead of
/// writing an orphan.
///
/// This is the sequence the tool teaches itself, and it did not compose: `taliesin init
/// myblog` leaves you in the parent, and the homepage it writes says "Scaffold a dated post
/// with `taliesin new post my-first-post`" — with no `cd` and no `--dir`. Typed there, that
/// wrote `./posts/my-first-post/index.tmd` *beside* `myblog/`, in a directory with no
/// `_site.yml`, printed `built …` and exited 0. The post was invisible to the site, absent
/// from its listing, and the only way to notice was to look at the filesystem.
#[test]
fn new_post_outside_a_project_refuses_instead_of_writing_an_orphan() {
    let parent = tmp("orphan-parent");
    let project = parent.join("myblog");
    let (ok, ..) = run(&["init", project.to_str().unwrap()]);
    assert!(ok, "`init` should succeed");

    // Exactly what a new user types: the homepage's own instruction, from where `init`
    // left them.
    let (ok, stdout, stderr) = run_in(&parent, &["new", "post", "my-first-post"]);
    assert!(
        !ok,
        "scaffolding outside any project must fail, not exit 0:\nstdout: {stdout}"
    );
    assert!(
        !parent.join("posts").exists(),
        "no orphan directory may be written beside the project"
    );
    assert!(stderr.contains("no _site.yml"), "it says why: {stderr}");
    assert!(
        stderr.contains("--dir"),
        "it names the way forward: {stderr}"
    );

    // Inside the project, the same command still works.
    let (ok, _, stderr) = run_in(&project, &["new", "post", "my-first-post"]);
    assert!(ok, "`new post` inside a project still works: {stderr}");
    assert!(project.join("posts/my-first-post/index.tmd").exists());

    let _ = std::fs::remove_dir_all(&parent);
}
