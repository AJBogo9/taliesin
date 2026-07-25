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
