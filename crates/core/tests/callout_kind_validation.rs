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

/// A callout kind this tool does not define still RENDERS unchanged, and only warns: the
/// render pipeline is purely diagnostic even for `important`/`caution`, which it did define
/// until 2026-08-03. The page keeps the class the author wrote; their own CSS can style it.
#[test]
fn a_callout_kind_the_tool_does_not_define_warns_and_still_renders() {
    let proj = TempProj::new();
    let src = "# T\n\nIntro.\n\n::: {.callout-important}\nBody.\n:::\n";
    let doc = taliesin_core::render_document_with_includes(src, &proj.0);
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("callout kind"))
        .expect("a warning for the undefined callout kind");
    assert!(
        w.message.starts_with("unknown callout kind `important`"),
        "got: {}",
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

/// A div class and a callout kind are author text inside an attribute, so they are escaped
/// like every other attribute value. They were interpolated raw, so a `"` in either wrote
/// a second attribute: `::: {.callout-note"onclick="alert(1)}` shipped a live `onclick`,
/// the shape the fence-language fix closed at its own site. The generated title a callout
/// takes from its kind is escaped too, so `<b>` in a kind is text, not an element.
#[test]
fn a_quote_in_a_div_class_or_callout_kind_never_writes_an_attribute() {
    let proj = TempProj::new();
    let src = "# T\n\n::: {.callout-note\"onclick=\"alert(1)}\nBody.\n:::\n\n\
               ::: {.callout-x<b>y</b>}\nBody.\n:::\n\n\
               ::: {.callout-note\"onclick=\"alert(3) collapse=\"true\"}\nBody.\n:::\n\n\
               ::: {.a\"onmouseover=\"alert(2)}\nx\n:::\n";
    let doc = taliesin_core::render_document_with_includes(src, &proj.0);
    let html = doc.body_html();
    let mut classes = Vec::new();
    for tag in taliesin_core::render::tags(&html) {
        for a in taliesin_core::render::attrs(&tag) {
            assert!(
                !a.name.starts_with("on"),
                "an event handler was written: {}",
                tag.text
            );
            if a.name == "class" && a.value.contains('"') {
                classes.push(a.value.to_string());
            }
        }
        assert_ne!(tag.name, "b", "a kind became markup: {html}");
    }
    assert_eq!(
        classes,
        vec![
            "callout callout-note\"onclick=\"alert(1)",
            "callout callout-note\"onclick=\"alert(3) callout-collapse",
            "a\"onmouseover=\"alert(2)",
        ],
        "each class reads back exactly as the author wrote it"
    );
}
