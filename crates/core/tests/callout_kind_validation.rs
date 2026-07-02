mod common;
use common::TempProj;

/// An unknown callout kind warns (located at the div's opening fence) and still
/// renders with its given class (no render change).
#[test]
fn unknown_callout_kind_warns_and_still_renders() {
    let proj = TempProj::new();
    let src = "# T\n\nIntro.\n\n::: {.callout-importnat}\nBody.\n:::\n";
    let doc = taliesin_core::render_document_with_includes(src, &proj.0);
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("callout kind"))
        .expect("a warning for the unknown callout kind");
    assert_eq!(
        w.message,
        "unknown callout kind `importnat` (did you mean `important`?)"
    );
    assert_eq!(
        w.line,
        Some(5),
        "located at the opening fence line, got: {w:?}"
    );
    // Render is unchanged: the class is still emitted verbatim.
    assert!(
        doc.body_html().contains("callout-importnat"),
        "callout still renders"
    );
}

/// A recognized callout kind is silent.
#[test]
fn recognized_callout_kind_does_not_warn() {
    let proj = TempProj::new();
    let src = "# T\n\n::: {.callout-tip}\nUse the thing.\n:::\n";
    let doc = taliesin_core::render_document_with_includes(src, &proj.0);
    assert!(
        !doc.warnings
            .iter()
            .any(|w| w.message.contains("callout kind")),
        "no callout-kind warning expected, got: {:?}",
        doc.warnings
    );
}
