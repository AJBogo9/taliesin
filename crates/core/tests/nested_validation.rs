//! Pins qmd-fast's schema validators to corpus/diagnostics/typos.qmd: rendering it
//! must produce exactly the expected click-to-source "unknown key" warnings, one per
//! deliberately-misspelled key (front-matter top-level + nested, callout kind, cell
//! option). This is the corpus pin for the nested-schema-validation epic.
mod common;
use common::corpus_dir;

#[test]
fn typos_doc_warns_exactly_on_each_unknown_key() {
    let dir = corpus_dir().join("diagnostics");
    let src = std::fs::read_to_string(dir.join("typos.qmd")).unwrap();
    let doc = taliesin_core::render_document_with_includes(&src, &dir);
    let msgs: Vec<&str> = doc.warnings.iter().map(|w| w.message.as_str()).collect();

    let expected = [
        "unknown front-matter key `treme` (did you mean `theme`?)",
        "unknown execute key `eccho` (did you mean `echo`?)",
        "unknown listing key `max-itemz` (did you mean `max-items`?)",
        "unknown callout kind `importnat` (did you mean `important`?)",
        "unknown cell option `labl` (did you mean `label`?)",
    ];
    for e in expected {
        assert!(
            msgs.contains(&e),
            "missing warning:\n  {e}\ngot:\n{msgs:#?}"
        );
    }
    // No EXTRA "unknown ..." warnings beyond the five pinned ones.
    let unknown = doc
        .warnings
        .iter()
        .filter(|w| w.message.starts_with("unknown "))
        .count();
    assert_eq!(
        unknown,
        expected.len(),
        "unexpected unknown-key warnings:\n{msgs:#?}"
    );

    // The body validators are click-to-source (located at the offending line).
    let cell = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("`labl`"))
        .unwrap();
    assert!(
        cell.line.is_some(),
        "cell-option warning should be located: {cell:?}"
    );
    let callout = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("`importnat`"))
        .unwrap();
    assert!(
        callout.line.is_some(),
        "callout warning should be located: {callout:?}"
    );
}
