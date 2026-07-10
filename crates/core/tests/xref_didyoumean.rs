//! Pins the did-you-mean suggestions on broken references to
//! `corpus/diagnostics/refs.tmd`: a near-miss `@fig-`/`@sec-` cross-reference and a
//! near-miss `[@key]` citation each name the target the author meant, while a
//! reference nothing resembles keeps its plain message.
//!
//! The two halves reach the author by different routes, which is why both are asserted
//! here: a broken citation is warned by `render` (it lands in `doc.warnings`), whereas a
//! broken cross-reference is warned by `cite::validate_xrefs`, which the servers call
//! separately once cross-page resolution has had its chance.
mod common;
use common::corpus_dir;

fn messages(warnings: &[taliesin_core::render::Warning]) -> Vec<&str> {
    warnings.iter().map(|w| w.message.as_str()).collect()
}

#[test]
fn near_miss_references_suggest_the_intended_target() {
    let dir = corpus_dir().join("diagnostics");
    let src = std::fs::read_to_string(dir.join("refs.tmd")).unwrap();
    let doc = taliesin_core::render_document_with_includes(&src, &dir);

    let xrefs = taliesin_core::cite::validate_xrefs(&doc.blocks);
    let xref_msgs = messages(&xrefs);
    for expected in [
        "broken cross-reference: @fig-reslts (did you mean `@fig-results`?)",
        "broken cross-reference: @sec-summry (did you mean `@sec-summary`?)",
        "broken cross-reference: @fig-nonexistent (no such figure/section/\u{2026})",
    ] {
        assert!(
            xref_msgs.contains(&expected),
            "missing warning:\n  {expected}\ngot:\n{xref_msgs:#?}"
        );
    }
    assert_eq!(xrefs.len(), 3, "unexpected xref warnings:\n{xref_msgs:#?}");

    let cite_msgs = messages(&doc.warnings);
    for expected in [
        "broken citation: @bishop2006patern (did you mean `@bishop2006pattern`?)",
        "broken citation: @nosuchkey (not in the bibliography)",
    ] {
        assert!(
            cite_msgs.contains(&expected),
            "missing warning:\n  {expected}\ngot:\n{cite_msgs:#?}"
        );
    }

    // Every suggestion is click-to-source: the point is to jump to the typo.
    for w in xrefs.iter().filter(|w| w.message.contains("did you mean")) {
        assert!(w.line.is_some(), "unlocated suggestion: {}", w.message);
    }
}
