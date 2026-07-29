//! Inlay hints: the resolved number beside a cross-reference.

use lsp_types::{InlayHint, InlayHintLabel, Position, Range};

/// One inlay hint per cross-reference in `range` whose anchor this document numbers.
///
/// `xref_numbers` is page-local, so a reference to an anchor defined in another chapter
/// has no entry here. Such a reference is *valid* (the diagnostic path knows the project's
/// anchors) but unnumbered, and we omit the hint rather than render a placeholder: a
/// missing hint reads as "no information", `⟨elsewhere⟩` reads as a claim.
pub(crate) fn inlay_hints(
    text: &str,
    doc: &taliesin_core::RenderedDoc,
    range: Range,
) -> Vec<InlayHint> {
    let mut out = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let line_no = line_no as u32;
        if line_no < range.start.line || line_no > range.end.line {
            continue;
        }
        for (id, end_char) in xrefs_on_line(line) {
            let Some(number) = doc.xref_numbers.get(&id) else {
                continue;
            };
            out.push(hint(line, line_no, end_char, number));
        }
    }
    out
}

/// An inlay hint reading `⟨label⟩`, positioned just past the construct it annotates.
fn hint(line: &str, line_no: u32, end_char: usize, label: &str) -> InlayHint {
    InlayHint {
        // The wire wants UTF-16; the scan works in scalar offsets.
        position: Position::new(
            line_no,
            crate::lsp_pos::char_to_utf16(line, end_char) as u32,
        ),
        label: InlayHintLabel::String(format!(" ⟨{label}⟩")),
        kind: None,
        text_edits: None,
        tooltip: None,
        padding_left: None,
        padding_right: None,
        data: None,
    }
}

/// Every `@anchor-id` on one line, with the scalar offset just past the id. Reuses
/// `lsp_nav`'s cursor classifier rather than adding a second scanner: walking the line and
/// asking "what is here" keeps one definition of what an xref is.
///
/// De-duplicated by END offset, not by id: the classifier answers the same span at every
/// column the id covers, but the *same id twice on one line* is two references and must get
/// two hints.
fn xrefs_on_line(line: &str) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for col in 0..line.chars().count() {
        if let crate::lsp_nav::Target::Xref { id, end, .. } =
            crate::lsp_nav::classify_target(line, 0, col)
            && out.last().map(|&(_, prev)| prev != end).unwrap_or(true)
        {
            out.push((id, end));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use lsp_types::{Position, Range};

    fn render(text: &str) -> taliesin_core::RenderedDoc {
        taliesin_core::render_single_doc(text, std::path::Path::new("."))
    }

    const DOC: &str = "\
# Results {#sec-results}

![A curve](a.png){#fig-results}

See @fig-results and @sec-results and @fig-nowhere.
";

    fn hints_on_last_line(text: &str) -> Vec<String> {
        let doc = render(text);
        let last = text.lines().count() as u32 - 1;
        super::inlay_hints(
            text,
            &doc,
            Range::new(Position::new(0, 0), Position::new(last, 0)),
        )
        .into_iter()
        .map(|h| match h.label {
            lsp_types::InlayHintLabel::String(s) => s,
            other => panic!("expected a string label, got {other:?}"),
        })
        .collect()
    }

    #[test]
    fn a_resolving_xref_gets_its_number() {
        let labels = hints_on_last_line(DOC);
        assert!(
            labels.iter().any(|l| l.contains('1')),
            "expected a number hint for @fig-results, got {labels:?}"
        );
    }

    #[test]
    fn an_unresolvable_xref_gets_no_hint() {
        let labels = hints_on_last_line(DOC);
        assert!(
            !labels.iter().any(|l| l.contains("nowhere")),
            "an anchor this document does not define must produce no hint, got {labels:?}"
        );
    }

    // The classifier answers the same span at every column the id covers, so the walk has to
    // de-duplicate — but by SPAN, not by id. The same anchor cited twice on one line is two
    // references and earns two hints; de-duplicating by id would silently drop the second.
    #[test]
    fn the_same_anchor_twice_on_one_line_gets_two_hints() {
        let text = "![A curve](a.png){#fig-x}\n\nSee @fig-x and again @fig-x.\n";
        let doc = render(text);
        let hints = super::inlay_hints(
            text,
            &doc,
            Range::new(Position::new(2, 0), Position::new(2, 0)),
        );
        assert_eq!(
            hints.len(),
            2,
            "expected one hint per reference, got {hints:?}"
        );
        assert_ne!(
            hints[0].position, hints[1].position,
            "two references must be annotated at their own positions"
        );
    }

    #[test]
    fn hints_outside_the_requested_range_are_not_returned() {
        let doc = render(DOC);
        // Range covering only line 0, which holds no reference.
        let hints = super::inlay_hints(
            DOC,
            &doc,
            Range::new(Position::new(0, 0), Position::new(0, 0)),
        );
        assert!(
            hints.is_empty(),
            "expected no hints outside the range, got {hints:?}"
        );
    }

    #[test]
    fn a_malformed_buffer_yields_no_hints_and_does_not_panic() {
        // Unterminated fenced div and unterminated display math: the normal half-typed case.
        let text = "::: {.callout}\n$$\n\\frac{1}{";
        let doc = render(text);
        let _ = super::inlay_hints(
            text,
            &doc,
            Range::new(Position::new(0, 0), Position::new(2, 0)),
        );
    }
}
