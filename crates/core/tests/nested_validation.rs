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
        "unknown execute key `eccho` (did you mean `echo`?)",
        "unknown listing key `max-itemz` (did you mean `max-items`?)",
        "unknown callout kind `importnat` (did you mean `important`?)",
        "unknown cell option `labl` (did you mean `label`?)",
        "unknown div class `fragmnet` (did you mean `fragment`?)",
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
    let div = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("`fragmnet`"))
        .unwrap();
    assert!(
        div.line.is_some(),
        "div-class warning should be located: {div:?}"
    );

    // PL7: a `.step lines=` carrying a `|` (the deck `code-line-numbers=` step separator) is a
    // silent no-op — the step's comma-only parser focuses zero lines — so it must warn, located.
    let step = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("step separator"))
        .expect("a `.step lines=` using `|` must warn");
    assert!(
        step.line.is_some(),
        "step-lines warning should be located: {step:?}"
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
