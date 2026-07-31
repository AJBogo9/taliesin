//! Black-box CLI coverage for the `render` and `blocks` subcommands (C7 in the
//! 2026-07-17 reduction map): both were exercised only indirectly, with no integration
//! test. `render` prints a one-shot full HTML page to stdout; `blocks` prints the
//! block-id + sourcepos debug listing. Both are static (they never execute code cells).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn write_doc(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-rbcli-{}-{name}", std::process::id()));
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

#[test]
fn render_prints_a_full_html_page_to_stdout() {
    let doc = write_doc("render");
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("render")
        .arg(&doc)
        .output()
        .expect("run render");
    let html = String::from_utf8_lossy(&out.stdout);
    let _ = fs::remove_dir_all(doc.parent().unwrap());
    assert!(out.status.success(), "render exited non-zero");
    assert!(html.contains("<!DOCTYPE html>"), "a full page: {html:.200}");
    assert!(
        html.contains("<html lang=\"en\">"),
        "html element: {html:.200}"
    );
    assert!(html.contains("CLI Doc"), "the title is rendered");
    // The block model is present: every emitted block carries these.
    assert!(html.contains("data-block-id="), "block ids present");
    assert!(html.contains("data-sourcepos="), "sourcepos present");
    assert!(html.contains("A paragraph."), "body content rendered");
}

#[test]
fn blocks_lists_block_ids_and_sourcepos() {
    let doc = write_doc("blocks");
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("blocks")
        .arg(&doc)
        .output()
        .expect("run blocks");
    let text = String::from_utf8_lossy(&out.stdout);
    let _ = fs::remove_dir_all(doc.parent().unwrap());
    assert!(out.status.success(), "blocks exited non-zero");
    assert!(
        text.contains("id") && text.contains("sourcepos"),
        "listing header: {text}"
    );
    assert!(
        text.contains("tali-title-block"),
        "the generated title block is listed: {text}"
    );
    assert!(
        text.contains("b-"),
        "a content block's content-hash id is listed: {text}"
    );
    // A real `L:C-L:C` sourcepos for the body (the heading is on line 5).
    assert!(
        text.contains("5:1-"),
        "a body block carries its sourcepos: {text}"
    );
}

/// `render` is a single self-contained page in Build + Inline asset mode, the same shape
/// `build <file> out.html` produces, so it must degrade a `{pyodide}` cell the same way
/// (item 158). It did not: it printed a live wrapper with no `<meta name="tali-pyodide-index">`
/// for the enhancer to boot from, so the only thing a reader could ever see there was an error
/// box, and the author's Python source, which lives inside that `<script>`, was invisible.
///
/// **The needle is the full opening tag, deliberately.** Every Taliesin page inlines the whole
/// JS bundle, and `pyodide.js` contains the bare string `application/tali-pyodide` in its
/// `registerLanguage` call, so a `contains("application/tali-pyodide")` here is a claim about
/// the bundle and is true on a correctly degraded page. Measured on this document:
/// `<script type="application/tali-pyodide"` occurs 0 times after the fix and 2 times before it.
#[test]
fn render_degrades_a_pyodide_cell_to_visible_source_like_a_single_file_build() {
    let dir = std::env::temp_dir().join(format!("tali-rbcli-{}-pyodide", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("doc.tmd");
    fs::write(
        &doc,
        "---\ntitle: Browser Python\n---\n\n```{pyodide}\nimport numpy as np\nnp.arange(3).tolist()\n```\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("render")
        .arg(&doc)
        .output()
        .expect("run render");
    let html = String::from_utf8_lossy(&out.stdout).into_owned();
    let _ = fs::remove_dir_all(&dir);

    assert!(out.status.success(), "render exited non-zero");
    // Known-positive first: without it every assertion below is satisfied by an empty page.
    assert!(
        html.contains("<!DOCTYPE html>") && html.contains("Browser Python"),
        "render produced a page at all: {html:.200}"
    );
    assert!(
        !html.contains("<script type=\"application/tali-pyodide\""),
        "a `render` page has no runtime to boot, so it must not ship a live `{{pyodide}}` \
         wrapper: the reader would get an error box and the source would be invisible"
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
