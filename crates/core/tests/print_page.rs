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
    let html = print_page_from_doc(
        &doc("# Hi\n\ntext\n"),
        "fallback",
        Paper::A4,
        Path::new("."),
    );
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
    let html = print_page_from_doc(&doc("# Hi\n"), "fallback", Paper::A4, Path::new("."));
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
    let html = print_page_from_doc(&doc("# Hi\n"), "fallback", Paper::A4, Path::new("."));
    let config = html.find("window.PagedConfig").expect("config present");
    let lib = html.find("@license Paged.js").expect("polyfill present");
    assert!(
        config < lib,
        "PagedConfig must be declared BEFORE the polyfill loads, or paged.js never sees it"
    );
}

#[test]
fn the_paper_size_reaches_the_at_page_rule() {
    let a4 = print_page_from_doc(&doc("# Hi\n"), "f", Paper::A4, Path::new("."));
    assert!(a4.contains("size: 210mm 297mm"), "A4 size must reach @page");

    let letter = print_page_from_doc(&doc("# Hi\n"), "f", Paper::Letter, Path::new("."));
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
        Path::new("."),
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

#[test]
fn a_document_with_figures_gets_a_generated_list_of_figures() {
    let src = "---\ntitle: T\n---\n\n\
               ![Alpha caption](a.png){#fig-alpha}\n\n\
               ![Omega caption](b.png){#fig-omega}\n";
    let html = print_page_from_doc(&doc(src), "f", Paper::A4, Path::new("."));
    assert!(
        html.contains("<nav class=\"tali-lof\""),
        "a document with figures must get a list-of-figures nav"
    );
    let a = html.find("href=\"#fig-alpha\"").expect("fig-alpha listed");
    let b = html.find("href=\"#fig-omega\"").expect("fig-omega listed");
    assert!(a < b, "the list must follow document order");
    assert!(
        html.contains("Alpha caption"),
        "each entry carries its caption text"
    );
}

/// An empty "List of Figures" heading on a document that has none is a defect, not a
/// degenerate case.
///
/// **Needles the full emitted tag, and this one is not pedantry:** the bare string
/// `tali-lof` now appears in `print.css`, which every print page inlines whole, so
/// `!contains("tali-lof")` asserts something about the stylesheet rather than the document
/// and fails on a page that correctly renders no list. That is the inlined-asset trap firing
/// in its NEGATIVE direction — the same way it bit the reader-affordances batch.
#[test]
fn a_document_without_figures_gets_no_list_of_figures() {
    let html = print_page_from_doc(&doc("# Hi\n\njust text\n"), "f", Paper::A4, Path::new("."));
    assert!(
        !html.contains("<nav class=\"tali-lof\""),
        "no figures means no list"
    );
    assert!(
        !html.contains("<h2>List of Figures</h2>"),
        "no figures means no heading"
    );
}

/// The LoF is a GENERATED block. The reader-affordances batch found one leaking into four
/// text projections across three modules, so this pins that the print-only block cannot
/// reach the normal page at all — it is excluded structurally, and this proves it.
#[test]
fn the_generated_list_of_figures_never_reaches_the_normal_page() {
    let d = doc("---\ntitle: T\n---\n\n![Alpha caption](a.png){#fig-alpha}\n");
    let normal =
        taliesin_core::render_doc_to_page(&d, "fallback", taliesin_core::OutputMode::Build);
    // The full tag, not the bare class: a stylesheet rule named `.tali-lof` landing in
    // base.css someday must not turn this pin red, and must not mask a real leak either.
    assert!(
        !normal.contains("<nav class=\"tali-lof\""),
        "the print-only list of figures leaked onto the normal built page"
    );
    // And the text projection `taliesin read`/`skim`, the search index and llms-full.txt
    // all derive from — the four surfaces the reader-affordances batch found leaking.
    assert!(
        !d.body_text().contains("List of Figures"),
        "the print-only list of figures leaked into the text projection"
    );
}

/// Paragraph-breaking and hyphenation policy. Asserted on the emitted sheet rather than the
/// PDF because these are continuous properties with no discrete signal in extracted text —
/// the live gate covers the things that DO show up there (heads, folios, page refs).
#[test]
fn the_print_sheet_sets_widow_orphan_and_hyphenation_policy() {
    let html = print_page_from_doc(&doc("# Hi\n\ntext\n"), "f", Paper::A4, Path::new("."));
    for needle in ["orphans: 3", "widows: 3", "hyphens: auto"] {
        assert!(
            html.contains(needle),
            "print.css must set `{needle}` — a paged document without it breaks paragraphs \
             across pages badly"
        );
    }
}

/// A float split across a page boundary is the most visible paging defect there is.
#[test]
fn the_print_sheet_keeps_floats_off_page_boundaries() {
    let html = print_page_from_doc(&doc("# Hi\n"), "f", Paper::A4, Path::new("."));
    assert!(
        html.contains("break-inside: avoid"),
        "figures, tables, code blocks and callouts must not split across pages"
    );
    assert!(
        html.contains("break-after: avoid"),
        "a heading stranded at the foot of a page must be pulled to the next"
    );
}

/// The print page is written to a TEMP directory, so every relative URL in the document
/// resolves against the wrong root unless a `<base href>` says otherwise.
///
/// **This shipped broken and was caught only by looking at a PDF**, not by a test: the
/// figures rendered their ALT TEXT where the image should have been. Every live gate written
/// before this one referenced an image file that did not exist, so nothing ever loaded and
/// nothing ever 404'd visibly.
#[test]
fn relative_urls_resolve_against_the_documents_own_directory() {
    let html = print_page_from_doc(
        &doc("---\ntitle: T\n---\n\n![Cap](pic.png){#fig-a}\n"),
        "f",
        Paper::A4,
        Path::new("/tmp"),
    );
    assert!(
        html.contains("<base href=\"file:///tmp/\">"),
        "the print page must carry a <base href> at the document's own directory"
    );
    // `<base>` only affects URLs that FOLLOW it, so it has to precede the content.
    let base = html.find("<base href=").expect("base present");
    let img = html.find("pic.png").expect("image present");
    assert!(base < img, "<base> must precede the URLs it governs");
}
