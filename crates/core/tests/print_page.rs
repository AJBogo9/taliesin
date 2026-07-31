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

/// Scrape the `<img …>` tags that carry a `src=`, out of an assembled page.
///
/// The `src=` filter is not cosmetic. A Taliesin page inlines its whole CSS/JS payload, and
/// that payload contains bare `<img>` literals of its own — so an unfiltered scrape returns
/// 8 tags for a two-figure document and any count assertion becomes a claim about the
/// bundle. This is the inlined-asset needle trap; it has fired on this file before.
fn img_tags(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find("<img") {
        rest = &rest[i..];
        let end = match rest.find('>') {
            Some(e) => e,
            None => break,
        };
        let tag = &rest[..=end];
        if tag.contains(" src=\"") {
            out.push(tag.to_string());
        }
        rest = &rest[end + 1..];
    }
    out
}

/// **The hang that cost a whole debugging session.** `loading="lazy"` is a *scrolling*
/// optimization: Chrome only starts the fetch when the image nears the viewport. A
/// paginated rendering never scrolls, so a lazy image far down the document is never
/// requested — `img.complete` stays `false` forever and neither `onload` nor `onerror` ever
/// fires. The assembler's start hook waits on exactly those events, so pagination never
/// began: measured in the real headless driver, `.pagedjs_page` count `0` with the polyfill
/// loaded and fonts settled.
///
/// It is also wrong even when it does not hang: an unloaded image has no intrinsic size, so
/// the chunker would lay it out at zero height and paginate around a figure that is not
/// there.
#[test]
fn the_print_page_loads_every_image_eagerly() {
    // A base directory with REAL images: the annotator reads intrinsic size off disk and
    // annotates nothing when it cannot, so a fixture of fake paths would make the negative
    // assertion below vacuous.
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/print");
    let src = "---\ntitle: T\n---\n\n![One](scree.png){#fig-a}\n\n![Two](recon.png){#fig-b}\n";
    let rendered = taliesin_core::render_single_doc(src, &base);

    // POSITIVE CONTROL: the rendered body really does carry a lazy image for this fixture.
    // Without this the assertion below passes just as happily on a page with no images.
    let screen = rendered.body_html();
    assert!(
        img_tags(&screen)
            .iter()
            .any(|t| t.contains(r#"loading="lazy""#)),
        "fixture is inert: the rendered body emitted no lazy image, so the print-page \
         assertion would prove nothing.\ntags: {:?}",
        img_tags(&screen)
    );

    let html = print_page_from_doc(&rendered, "fallback", Paper::A4, &base);
    let tags = img_tags(&html);
    let lazy: Vec<&String> = tags
        .iter()
        .filter(|t| t.contains(r#"loading="lazy""#))
        .collect();
    assert!(
        lazy.is_empty(),
        "these images would never load in a paginated render, wedging pagination: {lazy:?}"
    );
    // And the images must still be there — stripping the tag entirely would also pass above.
    assert_eq!(
        tags.len(),
        2,
        "both figures must survive into the print page"
    );
}

/// The running head must name the section in effect when the page BEGINS.
///
/// A drift pin, deliberately: the semantics are proved live in
/// `crates/server/tests/print_pdf.rs`, which renders real running heads and would catch the
/// keyword being unsupported (paged.js renders an unknown one as an empty margin box rather
/// than failing). What that gate cannot see is the difference between `start` and the
/// `first` default, because both produce a head on the pages it inspects. Dropping the
/// keyword would silently go back to naming a section that opens on the page's last line.
#[test]
fn the_running_head_names_the_section_in_effect_at_the_page_start() {
    let html = print_page_from_doc(
        &doc("# T\n\n## S\n\ntext\n"),
        "fallback",
        Paper::A4,
        Path::new("."),
    );
    assert!(
        html.contains("content: string(tali-section, start);"),
        "the @top-center rule must ask for the section in effect at the page start"
    );
}

/// Every `__TALI_*` placeholder in `print.css` must be substituted.
///
/// **This is the pin that was missing.** `__TALI_MAX_FLOAT_H__` was computed by
/// `Paper::max_float_height`, threaded through `print_page_from_doc`, and documented at
/// length as a load-bearing hang fix — while the rule that consumes it was not in the
/// stylesheet at all. The substitution silently replaced nothing, for every render, and no
/// test noticed because each one asserted on a value it expected rather than on the absence
/// of an unsubstituted token.
#[test]
fn no_unsubstituted_placeholder_survives_into_the_print_page() {
    for paper in [Paper::A4, Paper::Letter, Paper::A5] {
        let html = print_page_from_doc(&doc("# Hi\n\ntext\n"), "f", paper, Path::new("."));
        assert!(
            !html.contains("__TALI_"),
            "an unsubstituted placeholder reached the {} print page — either the CSS rule \
             that consumes it was removed, or a new one was added without a substitution",
            paper.name()
        );
    }
}

/// The per-paper figure cap must actually reach the stylesheet, at the right value.
/// `break-inside: avoid` on `figure` means an uncapped oversized figure can neither break nor
/// fit: measured, it bleeds past both page margins and strands blank pages ahead of it.
#[test]
fn the_figure_height_cap_reaches_the_stylesheet_per_paper() {
    let a4 = print_page_from_doc(&doc("# Hi\n"), "f", Paper::A4, Path::new("."));
    assert!(
        a4.contains("max-height: 190mm"),
        "the A4 figure cap must reach the print stylesheet"
    );
    let letter = print_page_from_doc(&doc("# Hi\n"), "f", Paper::Letter, Path::new("."));
    assert!(
        letter.contains("max-height: 175mm"),
        "the Letter figure cap must reach the print stylesheet"
    );
    assert!(
        !letter.contains("max-height: 190mm"),
        "a Letter render must not also carry the A4 cap"
    );
}
