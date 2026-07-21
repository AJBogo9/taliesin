//! A built deck must keep the same offline-build contract as a built HTML page:
//! a Mermaid diagram ships the vendored library inlined, not a live CDN dependency.
//! `deck_page_from_doc` used to hardcode `OutputMode::Preview` regardless of the
//! caller's mode, so a built deck never inlined the library and stayed one of
//! four formats that could reach out to a CDN at runtime.

use std::fs;
use std::process::Command;

#[test]
fn built_deck_with_mermaid_inlines_the_library() {
    let dir = std::env::temp_dir().join(format!("tali-deck-offline-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("slides.tmd");
    fs::write(
        &doc,
        "---\ntitle: Slides\nformat: deck\n---\n\n## A\n\n```mermaid\nflowchart LR\n  A --> B\n```\n",
    )
    .unwrap();
    let out = dir.join("slides.html");

    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(&doc)
        .output()
        .expect("run build");
    let html = fs::read_to_string(&out).unwrap_or_default();
    let stderr = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);

    assert!(res.status.success(), "build failed: {stderr}");
    assert!(
        html.contains("globalThis.mermaid"),
        "built deck must inline the vendored mermaid library, not fetch it from a CDN"
    );
}

#[test]
fn built_deck_carries_a_favicon() {
    // A built page falls back to the bundled taliesin mark when no favicon is
    // configured, so its tab has an icon and the browser never 404s `/favicon.ico`.
    // A built deck must keep the same contract: it used to ship no `<link rel="icon">`.
    let dir = std::env::temp_dir().join(format!("tali-deck-favicon-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("slides.tmd");
    fs::write(
        &doc,
        "---\ntitle: Slides\nformat: deck\n---\n\n## A\n\nHi\n",
    )
    .unwrap();
    let out = dir.join("slides.html");

    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(&doc)
        .output()
        .expect("run build");
    let html = fs::read_to_string(&out).unwrap_or_default();
    let stderr = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);

    assert!(res.status.success(), "build failed: {stderr}");
    assert!(
        html.contains("<link rel=\"icon\""),
        "a built deck must carry a favicon link (else a blank tab + a 404 on /favicon.ico)"
    );
}
