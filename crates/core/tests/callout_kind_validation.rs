mod common;
use common::TempProj;

/// An unknown callout kind warns (located at the div's opening fence) and still
/// renders with its given class (no render change).
#[test]
fn unknown_callout_kind_warns_and_still_renders() {
    let proj = TempProj::new();
    let src = "# T\n\nIntro.\n\n::: {.callout-warnign}\nBody.\n:::\n";
    let doc = taliesin_core::render_document_with_includes(src, &proj.0);
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("callout kind"))
        .expect("a warning for the unknown callout kind");
    assert_eq!(
        w.message,
        "unknown callout kind `warnign` (did you mean `warning`?)"
    );
    assert_eq!(
        w.line,
        Some(5),
        "located at the opening fence line, got: {w:?}"
    );
    // Render is unchanged: the class is still emitted verbatim.
    assert!(
        doc.body_html().contains("callout-warnign"),
        "callout still renders"
    );
}

/// A retired callout kind (`important`/`caution`, cut 2026-08-03) warns with the
/// removal note rather than a did-you-mean, and still renders unchanged — the render
/// pipeline is purely diagnostic even for a construct that once existed.
#[test]
fn retired_callout_kind_warns_with_removal_note_and_still_renders() {
    let proj = TempProj::new();
    let src = "# T\n\nIntro.\n\n::: {.callout-important}\nBody.\n:::\n";
    let doc = taliesin_core::render_document_with_includes(src, &proj.0);
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("callout kind"))
        .expect("a warning for the retired callout kind");
    assert!(
        w.message
            .starts_with("unknown callout kind `important`: it was removed"),
        "got: {}",
        w.message
    );
    assert!(
        !w.message.contains("did you mean"),
        "a retired kind is not a did-you-mean: {}",
        w.message
    );
    assert_eq!(w.line, Some(5), "located at the opening fence line");
    assert!(
        doc.body_html().contains("callout-important"),
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
