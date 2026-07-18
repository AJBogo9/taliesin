//! Custom-theme resolution: a `theme:` pointing at a sibling `.css`/`.scss` file, or at an
//! installed `_extensions/<name>/theme.css` bundle, has that CSS read from disk and inlined
//! into the page (after the base stylesheet). The negative case (a missing theme file warns)
//! is pinned in the render unit tests, but the POSITIVE file-read path had no test — and the
//! existing negative test passes even with `base_dir = None`, since the `.css` branch
//! short-circuits to the not-found warning before any read, so a broken/no-op reader would
//! still render clean. This is that net (C3), pinned against `corpus/theme-css/`.

use taliesin_core::{render_document_with_includes, render_html_page_with_includes};

mod common;
use common::{TempProj, corpus_dir};

use std::fs;

/// The corpus `theme: brand.css` doc: its sibling stylesheet is read from disk and inlined,
/// both onto the RenderedDoc (`theme_css`) and into the assembled page's `<style id="qmd-theme">`.
#[test]
fn a_custom_css_theme_file_is_read_from_disk_and_inlined() {
    let dir = corpus_dir().join("theme-css");
    let src = fs::read_to_string(dir.join("index.tmd")).expect("corpus theme-css/index.tmd");

    let doc = render_document_with_includes(&src, &dir);
    assert!(
        doc.warnings.is_empty(),
        "a readable theme file must not warn: {:?}",
        doc.warnings
    );
    assert!(
        doc.theme_css.contains("CORPUS-C3-THEME-MARKER"),
        "the custom .css file's contents must be inlined onto the RenderedDoc; got: {:?}",
        doc.theme_css
    );

    // End to end: the assembled page wraps that CSS in <style id="qmd-theme"> inside <head>.
    let page = render_html_page_with_includes(&src, &dir, "Custom theme");
    let head = &page[..page.find("</head>").expect("page has </head>")];
    assert!(
        head.contains("<style id=\"qmd-theme\">") && head.contains("CORPUS-C3-THEME-MARKER"),
        "the custom theme CSS must be inlined into the page <head>"
    );
}

/// An installed `_extensions/<name>/theme.css` bundle resolves by bare name and inlines its CSS
/// (no sidecar path, no warning). Synthetic, since the bundle branch is an internal resolution
/// route rather than a user-authored file worth a permanent corpus artifact.
#[test]
fn an_installed_extension_theme_bundle_is_read_and_inlined() {
    let d = TempProj::new();
    d.file(
        "_extensions/house/theme.css",
        ".x { color: #654321 } /* CORPUS-C3-EXT-MARKER */\n",
    );
    let doc = render_document_with_includes("---\ntitle: X\ntheme: house\n---\n\nHi.\n", &d.0);
    assert!(
        doc.theme_css.contains("CORPUS-C3-EXT-MARKER"),
        "an installed _extensions/<name>/theme.css must be read and inlined; got: {:?}",
        doc.theme_css
    );
    assert!(
        doc.warnings.is_empty(),
        "a resolved extension theme must not warn: {:?}",
        doc.warnings
    );
}
