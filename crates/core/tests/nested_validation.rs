//! Pins taliesin's schema validators to corpus/diagnostics/typos.tmd: rendering it
//! must produce exactly the expected click-to-source "unknown key" warnings, one per
//! deliberately-misspelled key (front-matter top-level + nested, callout kind, cell
//! option). This is the corpus pin for the nested-schema-validation epic.
mod common;
use common::corpus_dir;

#[test]
fn typos_doc_warns_exactly_on_each_unknown_key() {
    let dir = corpus_dir().join("diagnostics");
    let src = std::fs::read_to_string(dir.join("typos.tmd")).unwrap();
    let doc = taliesin_core::render_document_with_includes(&src, &dir);
    let msgs: Vec<&str> = doc.warnings.iter().map(|w| w.message.as_str()).collect();

    let expected = [
        "unknown front-matter key `treme` (did you mean `theme`?)",
        "unknown execute key `cach` (did you mean `cache`?)",
        "unknown listing key `max-itemz` (did you mean `max-items`?)",
        "unknown callout kind `warnign` (did you mean `warning`?)",
        "unknown cell option `labl` (did you mean `label`?)",
        "unknown div class `column-margn` (did you mean `column-margin`?)",
        // No near neighbour, so no guess: `theorems` is four edits from `theme`, and a
        // wrong rename is worse than none. An unknown PARENT key also takes its whole
        // nested block with it — one warning, not one per child, because the author
        // deletes the block rather than each line of it.
        "unknown front-matter key `theorems`",
    ];
    for e in expected {
        assert!(
            msgs.contains(&e),
            "missing warning:\n  {e}\ngot:\n{msgs:#?}"
        );
    }
    // No EXTRA "unknown ..." warnings beyond the pinned ones.
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

    // `csl:` is RECOGNIZED but deliberately inert, which is a third category again: not a
    // typo (nothing to rename) and not a removal (it was never honored). Its message does
    // not start with "unknown", so it sits outside the count above on purpose.
    let unsupported = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("is recognized but not supported"))
        .expect("`csl:` must draw the recognized-but-unsupported diagnostic");
    assert!(
        unsupported.message.contains("`csl:`") && unsupported.line.is_some(),
        "the unsupported-key warning names the key and is located: {unsupported:?}"
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
        .find(|w| w.message.contains("`warnign`"))
        .unwrap();
    assert!(
        callout.line.is_some(),
        "callout warning should be located: {callout:?}"
    );
    let div = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("`column-margn`"))
        .unwrap();
    assert!(
        div.line.is_some(),
        "div-class warning should be located: {div:?}"
    );

    // PL2: an empty `::: {.input name="k"}` div (reaching for a div instead of the shortcode)
    // renders nothing and is dropped — it must warn, located, pointing at the shortcode.
    let empty = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("empty `.input`"))
        .expect("an empty `.input` feature div must warn");
    assert!(
        empty.line.is_some() && empty.message.contains("{{< input"),
        "empty-div warning is located + points at the shortcode: {empty:?}"
    );
}
