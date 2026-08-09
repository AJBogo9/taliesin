//! Lane C `.bib` rendering fixes, pinned against real corpus documents:
//!   1. LaTeX accents -> Unicode        (Müller / Schölkopf / Erdős / Rényi)
//!   2. corporate / brace-protected authors render whole
//!   3. `@string` macro resolution + substitution
//!   4. `@inbook` / `@incollection` render `booktitle` + `pages`
//!   5. a manual `# References` heading suppresses the auto one (no duplicate)
//!
//! Plus a HARD byte-stable guard: an existing IEEE corpus citation (the
//! `em-algorithm` post's References block) must remain BYTE-IDENTICAL, so the
//! fixes above can't silently perturb already-correct output.

use std::fs;

use taliesin_core::render_document_with_includes;

mod common;
use common::corpus_dir;

/// Render a corpus post (resolving its includes) and return the body HTML.
fn render_post(rel: &str) -> String {
    let path = corpus_dir().join(rel);
    let base = path.parent().unwrap();
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
    render_document_with_includes(&src, base).body_html()
}

/// Slice out the single `<section class="tali-references">…</section>` block.
fn references_section(html: &str) -> &str {
    let start = html
        .find("<section class=\"tali-references\"")
        .expect("no References section rendered");
    let end = html[start..]
        .find("</section>")
        .map(|e| start + e + "</section>".len())
        .expect("unterminated References section");
    &html[start..end]
}

// ---------------------------------------------------------------------------
// HARD MERGE GATE: existing IEEE output is byte-stable.
// ---------------------------------------------------------------------------

/// The `em-algorithm` post cites a single `@book` with no accents, no `@string`,
/// no manual heading — i.e. output untouched by any Lane C fix. Snapshot it
/// EXACTLY so a regression in the accent/string/heading paths is caught.
#[test]
fn ieee_corpus_reference_output_is_byte_stable() {
    let html = render_post("tech-blog/posts/em-algorithm/index.tmd");
    let refs = references_section(&html);
    assert_eq!(
        refs,
        "<section class=\"tali-references\" data-block-id=\"tali-references\">\
         <h2>References</h2>\
         <div id=\"ref-bishop2006pattern\" class=\"csl-entry\">\
         [1] C. M. Bishop, <em>Pattern Recognition and Machine Learning</em>. Springer, 2006.\
         </div></section>",
        "IEEE corpus citation output changed (byte-stable gate)"
    );
}

/// A second byte-stable gate over entries that exercise the author paths the
/// single-`em-algorithm` snapshot does NOT: a single-brace `{First Last}` author
/// (`{Umar Jamil}` -> "U. Jamil", must keep initializing) and a double-brace
/// corporate author (`{{Wikipedia contributors}}` -> rendered whole). These two
/// posts caught a real regression during this lane, so they are pinned exactly.
#[test]
fn single_and_double_brace_author_corpus_output_is_byte_stable() {
    let elbo = references_section(&render_post(
        "tech-blog/posts/evidence-lower-bound/index.tmd",
    ))
    .to_string();
    assert_eq!(
        elbo,
        "<section class=\"tali-references\" data-block-id=\"tali-references\"><h2>References</h2>\
         <div id=\"ref-bishop2006pattern\" class=\"csl-entry\">\
         [1] C. M. Bishop, <em>Pattern Recognition and Machine Learning</em>. Springer, 2006.</div>\
         <div id=\"ref-jamil2023vae\" class=\"csl-entry\">\
         [2] U. Jamil, \u{201c}Variational Autoencoder - Model, ELBO, loss function and maths \
         explained easily!,\u{201d} 2023. [Online]. Available: \
         <a href=\"https://www.youtube.com/watch?v=iwEzwTTalbg\">\
         https://www.youtube.com/watch?v=iwEzwTTalbg</a>. YouTube video, accessed March 31, 2026.\
         </div></section>",
        "single-brace First-Last author output changed (byte-stable gate)"
    );

    let kw = references_section(&render_post(
        "tech-blog/posts/Kruskal-Wallis-test/index.tmd",
    ))
    .to_string();
    assert_eq!(
        kw,
        "<section class=\"tali-references\" data-block-id=\"tali-references\"><h2>References</h2>\
         <div id=\"ref-wiki_anova\" class=\"csl-entry\">\
         [1] Wikipedia contributors, \u{201c}Analysis of variance,\u{201d} 2025. [Online]. \
         Available: <a href=\"https://en.wikipedia.org/wiki/Analysis_of_variance\">\
         https://en.wikipedia.org/wiki/Analysis_of_variance</a>. Accessed: 2026-04-25.</div>\
         <div id=\"ref-bobbitt2020dunns\" class=\"csl-entry\">\
         [2] Z. Bobbitt, \u{201c}Dunn's Test for Multiple Comparisons,\u{201d} 2020. [Online]. \
         Available: <a href=\"https://www.statology.org/dunns-test/\">\
         https://www.statology.org/dunns-test/</a>. Accessed: 2026-04-25.</div></section>",
        "double-brace corporate author output changed (byte-stable gate)"
    );
}

// ---------------------------------------------------------------------------
// Corpus pin: cite-coverage exercises every fix in one rendered document.
// ---------------------------------------------------------------------------

#[test]
fn cite_coverage_corpus_doc_renders_all_fixes() {
    let html = render_post("posts/cite-coverage/index.tmd");
    let refs = references_section(&html);

    // Fix 1: LaTeX accents -> composed Unicode in author names.
    assert!(
        refs.contains("[1] K. Müller and B. Schölkopf,"),
        "accents not composed: {refs}"
    );
    assert!(
        refs.contains("[2] P. Erdős and A. Rényi,"),
        "Erdős/Rényi accents not composed: {refs}"
    );
    // No raw TeX cruft leaked into the section.
    assert!(!refs.contains('\\'), "backslash leaked: {refs}");
    assert!(!refs.contains("{\\"), "brace+macro leaked: {refs}");

    // Fix 2: corporate author renders whole, never initialised.
    assert!(
        refs.contains("[3] World Health Organization,"),
        "corporate author split/initialised: {refs}"
    );
    assert!(!refs.contains("W. H. Organization"), "got: {refs}");

    // Fix 3: `@string{springer = "Springer"}` substituted into book + chapter.
    assert!(
        refs.contains("<em>Pattern Recognition and Machine Learning</em>. Springer, 2006."),
        "string macro not substituted in book: {refs}"
    );

    // Fix 4: `@incollection` keeps booktitle (italic, "in …") + pages.
    assert!(
        refs.contains(
            "[5] Y. Bengio, \u{201c}Practical Recommendations for Gradient-Based Training,\u{201d} \
             in <em>Neural Networks: Tricks of the Trade</em>, Springer, 2012, pp. 437\u{2013}478."
        ),
        "@incollection booktitle/pages missing: {refs}"
    );

    // Fix 5: the auto <h2>References</h2> is suppressed (manual heading present),
    // and exactly one "References" heading exists in the whole document.
    assert!(
        !refs.contains("<h2>References</h2>"),
        "auto References heading should be suppressed: {refs}"
    );
    // The manual `# References` is body content on a titled page, so heading demotion
    // (#11) renders it as <h2> (one <h1> per page) — the same level as the auto heading.
    assert!(
        html.contains("<h2 id=\"references\""),
        "manual References heading missing: {html}"
    );
    let heading_count = html.matches(">References<").count();
    assert_eq!(
        heading_count, 1,
        "expected exactly one References heading, found {heading_count}"
    );

    // Fix 6: a `\url{...}` macro in `howpublished` unwraps to a bare URL (underscores
    // intact), rendered as an Available link — no backslash leaks (asserted above).
    assert!(
        refs.contains(
            "[6] Wikipedia contributors, \u{201c}Analysis of Variance,\u{201d} 2025. \
             [Online]. Available: \
             <a href=\"https://en.wikipedia.org/wiki/Analysis_of_variance\">"
        ),
        "\\url{{}} not unwrapped to a bare URL: {refs}"
    );

    // Fix 7: a quoted single-brace author `"{Ada Lovelace}"` initialises (a person),
    // it is NOT treated as a literal corporate name.
    assert!(
        refs.contains("[7] A. Lovelace,"),
        "quoted single-brace author not initialised: {refs}"
    );
    assert!(!refs.contains("Ada Lovelace,"), "rendered whole: {refs}");

    // Fix 8: a cite key with a `.` resolves (shared cite/bib key charset), so it is
    // numbered in the References, not truncated / dropped as a broken key.
    assert!(
        refs.contains("[8] J. Smith,"),
        "dotted cite key `smith.2020a` did not resolve: {refs}"
    );

    // Fix 9: the front matter loads its bibliography via the INLINE-SEQUENCE form
    // (`bibliography: [references.bib]`); if the seq-parsing path regressed, the bib
    // would not load and every citation above would be a raw key, failing this test.

    // Fix 10: a PAREN-delimited `@inproceedings` renders its booktitle (italic, "in …")
    // + pages — the commonest CS/ML type, previously dropped by the misc fallback.
    assert!(
        refs.contains(
            "[9] A. Vaswani and N. Shazeer, \u{201c}Attention Is All You Need,\u{201d} \
             in <em>Advances in Neural Information Processing Systems</em>, 2017, \
             pp. 5998\u{2013}6008."
        ),
        "@inproceedings booktitle/pages missing or paren entry misparsed: {refs}"
    );

    // Fix 11: the brace entry FOLLOWING the paren-delimited one still resolves — a
    // regression in the paren close-delimiter would cascade-drop every entry past it.
    assert!(
        refs.contains("[10] Y. LeCun,"),
        "entry after a paren-delimited one was cascade-dropped: {refs}"
    );
}
