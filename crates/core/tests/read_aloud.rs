//! Structural contract for the read-aloud reader enhancer. The enhancer (client-side
//! JS, browser-verified) walks the rendered DOM, so the *server output* of its pin doc
//! must contain the structures it keys off: a highlighted code block (`<code
//! class="language-…">`, whose lines the enhancer ranges over client-side), a numbered
//! figure (`<figcaption>`), a display equation (`.katex-display`), and a captioned table
//! (`<caption>`). If a future render change drops one of these, read-aloud silently stops
//! announcing/stepping it — this test makes that a hard failure.

mod common;
use common::corpus_dir;
use std::fs;

fn body() -> String {
    let path = corpus_dir().join("reader/read-aloud.qmd");
    let src = fs::read_to_string(&path).unwrap();
    taliesin_core::render_document_with_includes(&src, path.parent().unwrap()).body_html()
}

#[test]
fn pin_doc_renders_structures_read_aloud_walks() {
    let html = body();
    assert!(
        html.contains("language-python") && html.contains("<pre"),
        "code block must render a highlighted <pre><code class=language-python> to line-step over"
    );
    assert!(
        html.contains("<figcaption>Figure&nbsp;1:"),
        "figure must render a numbered figcaption for the announce step"
    );
    assert!(
        html.contains("katex-display"),
        "display equation must render as .katex-display for the announce step"
    );
    assert!(
        html.contains("<caption>Table&nbsp;1:"),
        "captioned table must render a numbered <caption> for the announce step"
    );
}
