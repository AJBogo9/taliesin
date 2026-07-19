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
