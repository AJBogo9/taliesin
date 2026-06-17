//! `_extensions/` format-extension resolution: a deck that selects a reveal theme
//! extension via `format: <ext>-revealjs` gets the extension's contributed theme +
//! includes injected (the mechanism behind liquid-glass-revealjs).

use std::fs;
use std::path::Path;

fn fixture(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
}

#[test]
fn format_extension_injects_theme_and_includes() {
    let dir = fixture("deck-ext");
    let src = fs::read_to_string(dir.join("slides.qmd")).expect("read slides.qmd");
    let html = qmd_fast_core::render_html_page_with_includes(&src, &dir, "Glass Deck");

    // Renders as a reveal deck (the `-revealjs` base format is detected).
    assert!(
        html.contains("<div class=\"reveal\">"),
        "should render as a reveal deck"
    );
    // contributed theme: glass.css inlined as <style>.
    assert!(
        html.contains(".reveal .slides section h2 { color: #2bd4a0; }"),
        "extension theme css not inlined"
    );
    // contributed include-in-header (file: glass-head.html), inside <head>.
    let head = &html[..html.find("</head>").expect("has </head>")];
    assert!(
        head.contains(r#"<meta name="glass-ext" content="active">"#),
        "extension include-in-header not injected into <head>"
    );
    // contributed include-after-body (file: glass-init.html).
    assert!(
        html.contains("window.__glassExt = true;"),
        "extension include-after-body not injected"
    );
}

#[test]
fn plain_format_without_extension_is_untouched() {
    // A bare `format: revealjs` has no extension prefix, so nothing extra is pulled.
    let src = "---\ntitle: T\nformat: revealjs\n---\n\n## S\n";
    let html = qmd_fast_core::render_html_page_with_includes(src, &fixture("deck-ext"), "T");
    assert!(html.contains("<div class=\"reveal\">"));
    assert!(
        !html.contains("glass-ext"),
        "a non-extension format must not pull extension includes"
    );
}
