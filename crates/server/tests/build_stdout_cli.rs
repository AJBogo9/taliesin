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

    // A site has many pages and no one page to put on stdout.
    let (ok, _out, err) = build(&[dir.to_str().unwrap(), "--stdout"]);
    assert!(!ok, "--stdout on a directory must fail");
    assert!(err.contains("many"), "got: {err}");

    let _ = fs::remove_dir_all(&dir);
}

/// `build --stdout` is a single self-contained page in Build + Inline asset mode, the same
/// shape `build <file> out.html` produces, so it must degrade a `{pyodide}` cell the same
/// way (item 158). The retired `render` verb did not: it printed a live wrapper with no
/// `<meta name="tali-pyodide-index">` for the enhancer to boot from, so the only thing a
/// reader could ever see there was an error box, and the author's Python source, which
/// lives inside that `<script>`, was invisible.
///
/// **The needle is the full opening tag, deliberately.** Every Taliesin page inlines the
/// whole JS bundle, and `pyodide.js` contains the bare string `application/tali-pyodide` in
/// its `registerLanguage` call, so a `contains("application/tali-pyodide")` here is a claim
/// about the bundle and is true on a correctly degraded page.
///
/// Gated on the feature: it asserts that a wrapper which WAS emitted gets degraded. With
/// the runtime compiled out no wrapper is emitted in the first place (the cell takes the
/// `emit`-as-source arm at render time), so feature-off this test would pass without
/// exercising the degrade path at all: green for the wrong reason.
///
/// **`--no-exec` is deliberately NOT passed, for that same reason.** It reaches the identical
/// non-exercising state by a second route: measured 2026-08-03, `--stdout --no-exec` emits
/// `<code class="language-pyodide">` straight from the render-time source arm, while a plain
/// `--stdout` emits the live wrapper and `degrade_pyodide_cells` rewrites it to
/// `<code class="language-python">`. Only the second path is the one under test. A `{pyodide}`
/// cell's runtime is the browser, so an executing build still needs no kernel here.
#[cfg(feature = "pyodide")]
#[test]
fn stdout_degrades_a_pyodide_cell_to_visible_source_like_a_single_file_build() {
    let dir = std::env::temp_dir().join(format!("tali-stdout-{}-pyodide", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("doc.tmd");
    fs::write(
        &doc,
        "---\ntitle: Browser Python\n---\n\n```{pyodide}\nimport numpy as np\nnp.arange(3).tolist()\n```\n",
    )
    .unwrap();

    let (ok, html, stderr) = build(&[doc.to_str().unwrap(), "--stdout"]);
    let _ = fs::remove_dir_all(&dir);

    assert!(ok, "build --stdout exited non-zero: {stderr}");
    // Known-positive first: without it every assertion below is satisfied by an empty page.
    assert!(
        html.contains("<!DOCTYPE html>") && html.contains("Browser Python"),
        "the build produced a page at all: {html:.200}"
    );
    assert!(
        !html.contains("<script type=\"application/tali-pyodide\""),
        "a stdout page has no runtime directory to boot from, so it must not ship a live \
         `{{pyodide}}` wrapper: the reader would get an error box and the source would be \
         invisible"
    );
    assert!(
        html.contains("<code class=\"language-python\">"),
        "the cell must degrade to a visible python listing, the same shape `build` produces"
    );
    // `arange`, not `np.arange(3)`: server-side highlighting splits the source into
    // `<span>`-wrapped tokens, so the multi-token literal never appears contiguously.
    assert!(
        html.contains("arange"),
        "the author's source must remain VISIBLE, not merely stripped"
    );
}
