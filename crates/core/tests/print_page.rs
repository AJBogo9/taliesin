//! The print/PDF track's page assembler (backlog 159).
//!
//! The assembler is a PURE function, so it is fully testable with no browser. The live CDP
//! loop is gated separately in `crates/server/tests/print_pdf.rs`.
//!
//! **Every assertion here needles a FULL emitted tag or a distinctive literal, never a bare
//! substring of the bundle.** A Taliesin page inlines its whole CSS/JS payload, so a
//! whole-page `contains("paged")` would pass on a page that paginates nothing — the
//! inlined-asset needle trap, which has fired in both directions on this codebase before.

use std::path::Path;
use taliesin_core::RenderedDoc;
use taliesin_core::render::print::{Paper, print_page_from_doc};

/// `RenderedDoc` derives only `Debug, Clone` — there is no `Default` — so build one the way
/// the product does, through the real single-doc render.
fn doc(src: &str) -> RenderedDoc {
    taliesin_core::render_single_doc(src, Path::new("."))
}

#[test]
fn the_print_page_inlines_the_pagedjs_polyfill() {
    let html = print_page_from_doc(&doc("# Hi\n\ntext\n"), "fallback", Paper::A4);
    // The bundle's own @license banner: distinctive, and present only if the real library
    // was inlined rather than a stub or a link.
    assert!(
        html.contains("@license Paged.js v0.4.3"),
        "the polyfill body must be inlined verbatim"
    );
    assert!(
        !html.contains("<script src=\"http"),
        "offline guarantee: the print page must fetch nothing"
    );
}

#[test]
fn the_print_page_stamps_a_completion_attribute_via_pagedconfig() {
    let html = print_page_from_doc(&doc("# Hi\n"), "fallback", Paper::A4);
    assert!(
        html.contains("window.PagedConfig"),
        "must declare PagedConfig so paged.js calls back when pagination finishes"
    );
    // The full assignment, not just the attribute name. An attribute that is never set
    // would hang the CDP driver rather than fail it, which is the worse failure.
    assert!(
        html.contains("dataset.taliPaged = 'done'"),
        "PagedConfig.after must stamp the flag the driver waits on"
    );
}

/// Ordering is load-bearing: paged.js reads `window.PagedConfig` when it loads, so a config
/// declared *after* the library is never seen and the driver waits forever.
#[test]
fn the_pagedconfig_hook_precedes_the_polyfill() {
    let html = print_page_from_doc(&doc("# Hi\n"), "fallback", Paper::A4);
    let config = html.find("window.PagedConfig").expect("config present");
    let lib = html.find("@license Paged.js").expect("polyfill present");
    assert!(
        config < lib,
        "PagedConfig must be declared BEFORE the polyfill loads, or paged.js never sees it"
    );
}

#[test]
fn the_paper_size_reaches_the_at_page_rule() {
    let a4 = print_page_from_doc(&doc("# Hi\n"), "f", Paper::A4);
    assert!(a4.contains("size: 210mm 297mm"), "A4 size must reach @page");

    let letter = print_page_from_doc(&doc("# Hi\n"), "f", Paper::Letter);
    assert!(
        letter.contains("size: 8.5in 11in"),
        "Letter size must reach @page"
    );
    assert!(
        !letter.contains("210mm 297mm"),
        "a Letter render must not also carry the A4 size"
    );
}

#[test]
fn paper_parses_the_three_supported_names_and_rejects_others() {
    assert_eq!(Paper::parse("a4"), Some(Paper::A4));
    assert_eq!(Paper::parse("A4"), Some(Paper::A4));
    assert_eq!(Paper::parse("letter"), Some(Paper::Letter));
    assert_eq!(Paper::parse("a5"), Some(Paper::A5));
    assert_eq!(Paper::parse("foolscap"), None);
    assert_eq!(Paper::default(), Paper::A4);
}

/// The document's own `lang` must reach `<html lang>`: `hyphens: auto` silently does nothing
/// without it, because the browser has no dictionary to pick. A hyphenation rule with no
/// lang is a rule that never fires.
#[test]
fn the_document_language_reaches_the_html_element() {
    let html = print_page_from_doc(
        &doc("---\ntitle: T\nlang: fi\n---\n\ntext\n"),
        "f",
        Paper::A4,
    );
    assert!(
        html.contains("lang=\"fi\""),
        "front-matter lang must reach <html lang>"
    );
}

/// The print artifact is terminal output. It must never be confused for a built page, and
/// the polyfill must never leak onto one.
#[test]
fn the_polyfill_is_absent_from_the_normal_built_page() {
    let d = doc("---\ntitle: T\n---\n\ntext\n");
    let normal =
        taliesin_core::render_doc_to_page(&d, "fallback", taliesin_core::OutputMode::Build);
    assert!(
        !normal.contains("@license Paged.js"),
        "the print-only polyfill leaked onto the normal built page"
    );
}
