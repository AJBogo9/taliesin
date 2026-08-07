//! Black-box CLI coverage for `build --stdout`: the page on stdout instead of in a file.
//!
//! Wave 5 folded the `render` verb into this flag — `render <f>` was `build <f> --stdout
//! --no-exec` with a second code path — and this file is that verb's coverage (C7 in the
//! 2026-07-17 reduction map) re-aimed at the survivor. The `blocks` verb's listing went
//! with it; what mattered about it, that every emitted block carries `data-block-id` +
//! `data-sourcepos`, is asserted here on the page itself, which is where a consumer reads
//! them.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn write_doc(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-stdout-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("doc.tmd");
    fs::write(
        &doc,
        "---\ntitle: CLI Doc\n---\n\n# Heading\n\nA paragraph.\n",
    )
    .unwrap();
    doc
}

fn build(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .args(args)
        .output()
        .expect("run taliesin build");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn stdout_prints_a_full_html_page_and_writes_no_file() {
    let doc = write_doc("page");
    let dir = doc.parent().unwrap().to_path_buf();
    let (ok, html, stderr) = build(&[doc.to_str().unwrap(), "--stdout", "--no-exec"]);

    assert!(ok, "build --stdout exited non-zero: {stderr}");
    assert!(html.contains("<!DOCTYPE html>"), "a full page: {html:.200}");
    assert!(
        html.contains("<html lang=\"en\">"),
        "html element: {html:.200}"
    );
    assert!(html.contains("CLI Doc"), "the title is rendered");
    // The block model is present: every emitted block carries these. This is what the
    // retired `blocks` listing existed to show.
    assert!(html.contains("data-block-id="), "block ids present");
    assert!(html.contains("data-sourcepos="), "sourcepos present");
    assert!(html.contains("A paragraph."), "body content rendered");
    // The heading is on line 5 of the fixture, so a real `L:C-L:C` span reaches the page.
    assert!(html.contains("5:1-"), "a body block carries its sourcepos");

    // Nothing was written: `--stdout` is the whole output. A default `build` would have
    // left `doc.html` beside the source.
    assert!(
        !dir.join("doc.html").exists(),
        "--stdout must not also write a file"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// The human log stays on stderr so the HTML pipes cleanly — the property that makes
/// `build … --stdout > page.html` a usable replacement for `render … > page.html`.
#[test]
fn only_the_page_reaches_stdout() {
    let doc = write_doc("clean");
    let (ok, html, _stderr) = build(&[doc.to_str().unwrap(), "--stdout", "--no-exec"]);
    assert!(ok);
    assert!(
        html.trim_start().starts_with("<!DOCTYPE html>"),
        "stdout opens with the page, with no log line ahead of it: {:.120}",
        html
    );
    let _ = fs::remove_dir_all(doc.parent().unwrap());
}

/// Two flags that each claim stdout, or each claim a destination, are a contradiction the
/// CLI must name rather than silently resolve: one of them would otherwise lose its output
/// (or, for `--format json`, interleave two streams on one fd).
#[test]
fn stdout_conflicts_are_loud() {
    let doc = write_doc("conflict");
    let path = doc.to_str().unwrap().to_string();
    let dir = doc.parent().unwrap().to_path_buf();

    for (args, needle) in [
        (vec![path.as_str(), "out.html", "--stdout"], "Pick one"),
        (vec![path.as_str(), "--stdout", "--out", "dist"], "Pick one"),
        (
            vec![path.as_str(), "--stdout", "--format", "json"],
            "both write to stdout",
        ),
    ] {
        let (ok, _out, err) = build(&args);
        assert!(!ok, "`{args:?}` must fail");
        assert!(err.contains(needle), "`{args:?}` said: {err}");
    }

    // A site has many pages and no one page to put on stdout. `--stdout` on a directory is
    // rejected for that reason specifically, so this needs a real project (`_site.yml`) --
    // otherwise the newer, more fundamental "not a project" guard wins instead (by design:
    // see `project_required.rs`), masking the check this test means to exercise.
    fs::write(dir.join("_site.yml"), "title: Conflict probe\n").unwrap();
    let (ok, _out, err) = build(&[dir.to_str().unwrap(), "--stdout"]);
    assert!(!ok, "--stdout on a directory must fail");
    assert!(err.contains("many"), "got: {err}");

    let _ = fs::remove_dir_all(&dir);
}
