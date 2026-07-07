//! Server-robustness CLI behaviors, exercised end-to-end through the real binary (the
//! exit codes are `std::process::ExitCode`, opaque to a unit test, so these go through
//! `CARGO_BIN_EXE_taliesin`):
//!
//! - a malformed `_site.yml` is a `--strict` build problem (a silently-degraded site must
//!   not ship green), while a *missing* `_site.yml` is not;
//! - an unknown `--flag` is a hard error with a did-you-mean (not silently dropped);
//! - a value-less `--out` is a hard error (not a silent `<stem>.html` write).

use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-robust-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn taliesin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_taliesin"))
}

#[test]
fn malformed_site_yml_fails_strict_build() {
    let dir = tmp_dir("malformed");
    // Unterminated double-quoted scalar -> serde_yaml parse error -> degraded default site.
    fs::write(dir.join("_site.yml"), "title: \"unterminated\nfoo: bar\n").unwrap();
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nWelcome.\n").unwrap();
    let out = dir.join("_site");

    // Without --strict the build still writes (degraded), exit 0.
    let lenient = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run lenient build");
    let lenient_err = String::from_utf8_lossy(&lenient.stderr);
    assert!(
        lenient.status.success(),
        "a malformed config without --strict still builds (degraded): {lenient_err}"
    );
    assert!(
        lenient_err.contains("not valid YAML"),
        "the malformed config is still reported: {lenient_err}"
    );

    // With --strict the same malformed config must fail the build (non-zero exit).
    let strict = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--strict")
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run strict build");
    let strict_err = String::from_utf8_lossy(&strict.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !strict.status.success(),
        "a malformed _site.yml must fail --strict, stderr was:\n{strict_err}"
    );
    assert!(
        strict_err.contains("--strict") && strict_err.contains("problem"),
        "the strict failure names the problem count: {strict_err}"
    );
}

#[test]
fn missing_site_yml_does_not_fail_strict_build() {
    // A bare directory of `.tmd` pages (no `_site.yml`) is legitimate: --strict must not
    // fail just because the config is absent.
    let dir = tmp_dir("nofile");
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nWelcome.\n").unwrap();
    let out = dir.join("_site");
    let res = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--strict")
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run strict build");
    let err = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        res.status.success(),
        "a missing _site.yml must NOT fail --strict, stderr was:\n{err}"
    );
}

#[test]
fn single_doc_malformed_front_matter_fails_strict() {
    // Batch 5: a single-doc `build` used to skip yaml_error(), so a typo'd `---` block
    // built clean and passed --strict. It must now be a --strict problem.
    let dir = tmp_dir("singleyaml");
    let doc = dir.join("post.tmd");
    // `bad: : x` is a YAML syntax error (a mapping value that is itself a bare colon).
    fs::write(&doc, "---\ntitle: OK\nbad: : x\n---\n\nProse.\n").unwrap();

    // Lenient: still builds (degraded), exit 0, but reports the error.
    let lenient = taliesin()
        .arg("build")
        .arg(&doc)
        .output()
        .expect("lenient build");
    assert!(
        lenient.status.success(),
        "a malformed front-matter without --strict still builds"
    );

    let strict = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--strict")
        .output()
        .expect("strict build");
    let err = String::from_utf8_lossy(&strict.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !strict.status.success(),
        "malformed single-doc front-matter must fail --strict, stderr:\n{err}"
    );
}

#[test]
fn single_doc_embed_counts_toward_strict() {
    // Batch 7: an unresolved `{{< embed >}}` in a single-doc build warns (its target
    // isn't built beside the page), but the warning never counted toward `problems`, so
    // `--strict` passed green despite shipping a dead iframe. It must now fail --strict.
    let dir = tmp_dir("embedstrict");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\n{{< embed talk.tmd >}}\n").unwrap();

    // Lenient: still builds (the warning is non-fatal), exit 0.
    let lenient = taliesin()
        .arg("build")
        .arg(&doc)
        .output()
        .expect("lenient build");
    assert!(
        lenient.status.success(),
        "an embed warning without --strict still builds"
    );

    let strict = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--strict")
        .output()
        .expect("strict build");
    let err = String::from_utf8_lossy(&strict.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !strict.status.success(),
        "an unresolved single-doc embed must fail --strict, stderr:\n{err}"
    );
    assert!(
        err.contains("--strict") && err.contains("problem"),
        "the strict failure names the problem count: {err}"
    );
}

#[test]
fn build_rejects_unknown_flag_with_suggestion() {
    let dir = tmp_dir("badflag");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\nProse.\n").unwrap();
    let res = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--stict") // typo for --strict
        .output()
        .expect("run build");
    let err = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !res.status.success(),
        "an unknown flag must fail the build, stderr was:\n{err}"
    );
    assert!(
        err.contains("--stict") && err.contains("--strict"),
        "the error names the bad flag and suggests --strict: {err}"
    );
}

#[test]
fn build_rejects_value_less_out_flag() {
    let dir = tmp_dir("noout");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\nProse.\n").unwrap();
    // `--out` at the end of args (no directory value): a hard error, not a silent
    // `<stem>.html` write.
    let res = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--out")
        .output()
        .expect("run build");
    let err = String::from_utf8_lossy(&res.stderr);
    let html = dir.join("post.html");
    let wrote_default = html.exists();
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !res.status.success(),
        "a value-less --out must fail, stderr was:\n{err}"
    );
    assert!(
        err.contains("--out") && err.contains("requires"),
        "the error explains --out needs a value: {err}"
    );
    assert!(
        !wrote_default,
        "a value-less --out must not silently write the default <stem>.html"
    );
}

#[test]
fn check_rejects_unknown_flag_with_suggestion() {
    let dir = tmp_dir("checkflag");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\nProse.\n").unwrap();
    let res = taliesin()
        .arg("check")
        .arg(&doc)
        .arg("--formt") // typo for --format
        .arg("json")
        .output()
        .expect("run check");
    let err = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !res.status.success(),
        "an unknown check flag must fail, stderr was:\n{err}"
    );
    assert!(
        err.contains("--formt") && err.contains("--format"),
        "the error names the bad flag and suggests --format: {err}"
    );
}

#[test]
fn preview_rejects_unknown_flag_with_suggestion() {
    let dir = tmp_dir("previewflag");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\nProse.\n").unwrap();
    // A typo'd `--hots` (for --host) must fail fast, before the server binds a port.
    let res = taliesin()
        .arg("preview")
        .arg(&doc)
        .arg("--hots")
        .output()
        .expect("run preview");
    let err = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !res.status.success(),
        "an unknown preview flag must fail, stderr was:\n{err}"
    );
    assert!(
        err.contains("--hots") && err.contains("--host"),
        "the error names the bad flag and suggests --host: {err}"
    );
}
