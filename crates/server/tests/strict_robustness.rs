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

/// The confidence gap: `check` chained ten static validators that ran nowhere else, so a
/// `build --strict` exited 0 while shipping a broken `<img>`. A green `--strict` reads as
/// "safe to ship", so it must fail on exactly what `check` fails on.
#[test]
fn strict_build_fails_on_everything_check_fails_on() {
    let dir = tmp_dir("strict-superset");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(
        dir.join("about.tmd"),
        "---\ntitle: About\n---\n\n## Team {#team}\n\nHi.\n",
    )
    .unwrap();
    // Five defects `build --strict` used to miss entirely or count without locating:
    // a duplicate heading id, a broken in-page anchor, a missing image, a link to a page
    // that does not exist, and an anchor that does not exist on a page that does.
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n## A {#dup}\n\n## B {#dup}\n\n\
         See [anchor](#nope) and ![img](missing.png).\n\
         A [cross-page](ghost.tmd) link and a [bad anchor](about.tmd#nope).\n",
    )
    .unwrap();

    let out = dir.join("_out");
    let check = taliesin().arg("check").arg(&dir).output().unwrap();
    assert!(!check.status.success(), "check must fail on this site");

    let strict = taliesin()
        .args(["build"])
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .arg("--strict")
        .output()
        .unwrap();
    assert!(
        !strict.status.success(),
        "build --strict must fail on what check fails on; stderr:\n{}",
        String::from_utf8_lossy(&strict.stderr)
    );

    // Every diagnostic `check` reports is reported by the build, verbatim and located.
    let check_msgs: Vec<String> = String::from_utf8_lossy(&check.stderr)
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.contains(" problem"))
        .map(|l| l.trim().to_string())
        .collect();
    assert_eq!(
        check_msgs.len(),
        5,
        "expected 5 findings, got {check_msgs:?}"
    );
    let build_err = String::from_utf8_lossy(&strict.stderr).to_string();
    for m in &check_msgs {
        assert!(
            build_err.contains(m.as_str()),
            "build --strict omitted `{m}`\nbuild stderr:\n{build_err}"
        );
    }

    // Without `--strict` the page is still written and the build succeeds: the lints warn,
    // they do not gate an ordinary build.
    let plain = taliesin()
        .args(["build"])
        .arg(&dir)
        .arg("--out")
        .arg(dir.join("_out2"))
        .output()
        .unwrap();
    assert!(plain.status.success(), "a plain build still succeeds");
    assert!(
        dir.join("_out2/index.html").is_file(),
        "and still writes the page"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// The same wiring on the single-document path, whose validator set differs by one rule
/// (`validate_local_links` runs standalone but not in a site, where a `.tmd` link rewrites
/// to `.html` and only the page registry knows the real url).
#[test]
fn strict_single_doc_build_fails_on_a_missing_image() {
    let dir = tmp_dir("strict-single");
    let doc = dir.join("doc.tmd");
    fs::write(&doc, "---\ntitle: T\n---\n\n![img](missing.png)\n").unwrap();

    let check = taliesin().arg("check").arg(&doc).output().unwrap();
    assert!(!check.status.success());

    let strict = taliesin()
        .args(["build"])
        .arg(&doc)
        .arg(dir.join("out.html"))
        .arg("--strict")
        .output()
        .unwrap();
    assert!(
        !strict.status.success(),
        "single-doc --strict must fail on a missing image"
    );
    let err = String::from_utf8_lossy(&strict.stderr);
    assert!(err.contains("missing.png"), "names the asset: {err}");
    assert!(err.contains("doc:5:"), "located to its line: {err}");

    let _ = fs::remove_dir_all(&dir);
}

/// The `Scope::InSite` carve-out. An intra-site `[x](other.tmd)` link is legitimate: it
/// rewrites to `other.html` at build time. Running the single-doc link rule on site pages
/// would report every internal link as broken, so a correct site must build green.
#[test]
fn strict_site_build_does_not_flag_a_working_intra_site_link() {
    let dir = tmp_dir("strict-intrasite");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n[About](about.tmd)\n",
    )
    .unwrap();
    fs::write(dir.join("about.tmd"), "---\ntitle: About\n---\n\nHi.\n").unwrap();

    let strict = taliesin()
        .args(["build"])
        .arg(&dir)
        .arg("--out")
        .arg(dir.join("_out"))
        .arg("--strict")
        .output()
        .unwrap();
    assert!(
        strict.status.success(),
        "a correct site must build green under --strict; stderr:\n{}",
        String::from_utf8_lossy(&strict.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

/// The audit's original reproduction, isolated: a site whose *only* defect is a missing
/// image. Nothing else fails it, so this is the test that actually pins the per-page
/// static-validator wiring (a broken cross-page link would fail the build by itself).
#[test]
fn strict_site_build_fails_on_a_missing_image_alone() {
    let dir = tmp_dir("strict-missing-img");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n![missing](does-not-exist.png)\n",
    )
    .unwrap();

    let out = dir.join("_out");
    let strict = taliesin()
        .args(["build"])
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .arg("--strict")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&strict.stderr);
    assert!(
        !strict.status.success(),
        "build --strict shipped a broken <img> with exit 0; stderr:\n{err}"
    );
    assert!(err.contains("does-not-exist.png"), "names the asset: {err}");
    assert!(err.contains("index.tmd:5:"), "located to its line: {err}");
    let _ = fs::remove_dir_all(&dir);
}
