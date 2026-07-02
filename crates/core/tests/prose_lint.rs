//! The prose-lint pin doc trips each rule (doubled / weasel / banned) and proves
//! markdown-awareness: code, math, link URLs, and fenced blocks are NOT flagged. Mirrors
//! `nested_validation.rs` (asserts the exact located warning set).

mod common;
use common::corpus_dir;
use std::fs;

fn warnings() -> Vec<taliesin_core::render::Warning> {
    let path = corpus_dir().join("diagnostics/prose.qmd");
    let src = fs::read_to_string(&path).unwrap();
    taliesin_core::render_document_with_includes(&src, path.parent().unwrap()).warnings
}

#[test]
fn prose_lint_pin_doc_trips_each_rule_and_skips_markdown() {
    let ws = warnings();
    let has = |needle: &str| ws.iter().any(|w| w.message == needle);
    assert!(has("repeated word `we`"), "doubled word: {ws:?}");
    assert!(
        has("weasel word `very` (consider cutting)"),
        "weasel: {ws:?}"
    );
    assert!(
        has("weasel word `really` (consider cutting)"),
        "weasel: {ws:?}"
    );
    assert!(has("banned term `utilize`"), "banned: {ws:?}");
    // markdown-awareness: the `utilize` in code / the link URL / the fence must NOT warn —
    // only the prose `utilize` on the "Please utilize" line does.
    let utilize_hits = ws
        .iter()
        .filter(|w| w.message == "banned term `utilize`")
        .count();
    assert_eq!(
        utilize_hits, 1,
        "only the prose `utilize` should warn: {ws:?}"
    );
    // every prose warning is located (carries a source line for click-to-source)
    assert!(
        ws.iter()
            .filter(|w| w.message.contains("word") || w.message.contains("term"))
            .all(|w| w.line.is_some()),
        "prose warnings must carry a line: {ws:?}"
    );
}
