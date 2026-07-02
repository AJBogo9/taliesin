mod common;
use common::TempProj;

/// A broken cross-reference warning carries the source line of the block that
/// contains the dangling `@ref`, so the dev panel can jump to it.
#[test]
fn broken_crossref_warning_is_located() {
    let proj = TempProj::new();
    let doc = taliesin_core::render_document_with_includes(
        "# Title\n\nIntro.\n\nSee @fig-nope for details.\n",
        &proj.0,
    );
    // Standalone docs surface broken xrefs via `validate_xrefs` (the server runs it
    // after site-wide resolution); exercise that path directly here.
    let xref_warnings = taliesin_core::cite::validate_xrefs(&doc.blocks);
    let located = xref_warnings
        .iter()
        .find(|w| w.message.contains("@fig-nope"))
        .expect("a broken-crossref warning for @fig-nope");
    assert!(
        located.line.is_some(),
        "broken-crossref warning should carry a line, got: {located:?}"
    );
}

/// An unknown-shortcode warning carries the line where the shortcode appears.
#[test]
fn unknown_shortcode_warning_is_located() {
    let proj = TempProj::new();
    let doc = taliesin_core::render_document_with_includes(
        "# Title\n\nIntro.\n\n{{< videoo clip.mp4 >}}\n",
        &proj.0,
    );
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("videoo"))
        .expect("an unknown-shortcode warning for `videoo`");
    assert_eq!(w.line, Some(5), "shortcode is on line 5, got: {w:?}");
}

/// A broken citation (a `@key` with a bibliography present but no matching entry)
/// carries the line of the block where the citation appears.
#[test]
fn broken_citation_warning_is_located() {
    let proj = TempProj::new();
    proj.file(
        "refs.bib",
        "@article{real, title={Real}, author={A}, year={2020}, journal={J}}\n",
    );
    let doc = taliesin_core::render_document_with_includes(
        "---\nbibliography: refs.bib\n---\n\n# Title\n\nFirst para.\n\nSee [@missingkey] here.\n",
        &proj.0,
    );
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("@missingkey"))
        .expect("a broken-citation warning for @missingkey");
    assert!(
        w.line.is_some(),
        "broken-citation warning should carry a line, got: {w:?}"
    );
}
