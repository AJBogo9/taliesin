//! PL8: the page head advertises its generator and keeps mobile browser-chrome (`theme-color`)
//! in lockstep with the theme. The `generator` meta is static; the `theme-color` is created +
//! updated by the pre-paint theme script (reusing its `BG` map, so no hex is duplicated), so it
//! follows the in-page toggle, not only the OS scheme.

use taliesin_core::render_html_page;

#[test]
fn head_advertises_the_generator() {
    let page = render_html_page("# Hello\n\nBody.\n", "Hello");
    let head = &page[..page.find("</head>").expect("page has </head>")];
    assert!(
        head.contains(r#"<meta name="generator" content="Taliesin" />"#),
        "the head must advertise Taliesin as the generator; head:\n{head}"
    );
}

#[test]
fn head_carries_the_human_readable_generator_banner() {
    // C11: besides the machine `<meta name="generator">`, the head opens with a human-readable
    // ASCII-art banner (name + version + URL) so a developer reading view-source can find the
    // tool. It lives INSIDE <head> (an HTML comment), so the first byte stays the doctype.
    let page = render_html_page("# Hello\n\nBody.\n", "Hello");
    assert!(
        page.starts_with("<!DOCTYPE html>"),
        "the banner must not push the doctype off the first byte; page starts:\n{}",
        &page[..page.char_indices().nth(40).map_or(page.len(), |(i, _)| i)]
    );
    let head = &page[..page.find("</head>").expect("page has </head>")];
    assert!(
        head.contains(&format!("Taliesin v{}", taliesin_core::VERSION))
            && head.contains("https://taliesin.sh")
            && head.contains("block-modeled live HTML process"),
        "the head must carry the human generator banner (name + version + URL); head:\n{head}"
    );
}

#[test]
fn deck_head_carries_generator_meta_and_banner() {
    // A deck page previously shipped NEITHER the generator meta nor the banner; C11 adds both,
    // symmetric with the HTML page head.
    let page = render_html_page("---\ntitle: D\nformat: deck\n---\n\n## Slide\n", "D");
    let head = &page[..page.find("</head>").expect("deck has </head>")];
    assert!(
        head.contains(r#"<meta name="generator" content="Taliesin" />"#),
        "the deck head must advertise the generator meta; head:\n{head}"
    );
    assert!(
        head.contains(&format!("Taliesin v{}", taliesin_core::VERSION))
            && head.contains("https://taliesin.sh"),
        "the deck head must carry the human generator banner; head:\n{head}"
    );
}

#[test]
fn pre_paint_script_keeps_a_theme_color_meta_in_sync() {
    // The theme-color meta is created + set from the same `BG[mode]` the canvas uses, at the
    // one `apply()` choke point every theme change routes through — so it tracks the reader's
    // in-page toggle, not just the OS. Assert the wiring is present in the emitted head script.
    let page = render_html_page("# Hello\n\nBody.\n", "Hello");
    let head = &page[..page.find("</head>").expect("page has </head>")];
    assert!(
        head.contains(r#"meta[name="theme-color"]"#) && head.contains(r#"createElement("meta")"#),
        "the pre-paint script must create a theme-color meta; head:\n{head}"
    );
    assert!(
        head.contains(r#"mc.setAttribute("content", BG[mode]"#),
        "the theme-color meta's content must be set from the same BG[mode] as the canvas; head:\n{head}"
    );
}
