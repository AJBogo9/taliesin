//! Structural editor commands for `.tmd`: move a section, change a heading's level.
//!
//! The four operations every outliner has (move up, move down, promote, demote) as **pure
//! text transforms of the `.tmd` buffer**, computed here and applied by the editor. This is
//! the legal replacement for the drag-to-reorder-slides gesture that was removed for
//! breaking the single-editing-surface rule: the source stays the one editing surface, the
//! preview stays a read-only view, and click-to-source keeps pointing one way.
//!
//! **Why in Rust rather than in the companion.** The segmentation is the whole feature: what
//! counts as a heading (not a `#` comment inside a `{python}` cell, not a `-` line in front
//! matter), where a section ends, and which of the headings around it is a *sibling* rather
//! than a parent. That knowledge already exists once, in `lsp_outline`, and this module
//! reuses it rather than re-deriving it — a second copy in TypeScript is exactly what the
//! LSP rewrite deleted.
//!
//! **Siblings only, never across a parent.** Moving down past a shallower heading would
//! silently re-parent the section (a `###` that walks past the next `##` changes which
//! chapter it belongs to), so that case is refused with a message instead. The transform an
//! author cannot see the consequence of is the one they will not notice went wrong.

use lsp_types::{Position, Range, TextEdit};

/// Which structural transform to apply at the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SectionOp {
    MoveUp,
    MoveDown,
    Promote,
    Demote,
}

/// `taliesin/sectionEdit` parameters: which buffer, where the cursor is, which transform.
///
/// The cursor is a parameter rather than server state because the server has none — LSP
/// notifies it of *text*, never of a selection.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SectionEditParams {
    pub text_document: lsp_types::TextDocumentIdentifier,
    pub position: Position,
    pub op: SectionOp,
}

/// The edits for one transform, plus where the cursor belongs once they land.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SectionEdit {
    pub edits: Vec<TextEdit>,
    /// Where to put the cursor after applying, or `None` to leave it where the editor's own
    /// edit tracking puts it.
    ///
    /// A move replaces a region that *contains* the cursor, and an editor's tracking has no
    /// way to know the author's line travelled with the section rather than staying at its
    /// offset in the region. Without this, one "move down" leaves the caret in whatever text
    /// slid up into its place and the second keypress moves a different section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Position>,
}

/// The edits that apply `op` to the section containing `cursor`, or a reason it cannot.
///
/// The error string is written to be shown to the author verbatim: it names the section, so
/// "this is the last one" arrives about a heading they recognize.
pub(crate) fn section_edit(
    text: &str,
    cursor: Position,
    op: SectionOp,
) -> Result<SectionEdit, String> {
    let sections = crate::lsp_outline::sections(text);
    let line = cursor.line as usize;
    // Sections tile from the first heading to EOF, so the last heading at or before the
    // cursor is the innermost section containing it. Put the cursor on the heading line
    // itself to act on that level rather than on the deepest one under it.
    let idx = sections
        .iter()
        .rposition(|s| s.start_line <= line)
        .ok_or_else(|| {
            "the cursor is above the first heading, so there is no section to move".to_string()
        })?;
    match op {
        SectionOp::MoveUp | SectionOp::MoveDown => move_section(text, &sections, idx, cursor, op),
        SectionOp::Promote | SectionOp::Demote => change_level(&sections, idx, op),
    }
}

/// Swap the section at `idx` with its previous or next sibling.
fn move_section(
    text: &str,
    sections: &[crate::lsp_outline::Section],
    idx: usize,
    cursor: Position,
    op: SectionOp,
) -> Result<SectionEdit, String> {
    let down = op == SectionOp::MoveDown;
    let level = sections[idx].level;
    let title = &sections[idx].title;
    // The nearest heading in that direction at the same level or shallower. A shallower one
    // is the parent boundary, which is where the move stops.
    let neighbour = if down {
        sections[idx + 1..]
            .iter()
            .position(|s| s.level <= level)
            .map(|p| idx + 1 + p)
    } else {
        sections[..idx].iter().rposition(|s| s.level <= level)
    };
    let Some(sib) = neighbour else {
        return Err(format!(
            "\"{title}\" is the {} section at this level in the document",
            if down { "last" } else { "first" }
        ));
    };
    if sections[sib].level < level {
        return Err(format!(
            "\"{title}\" is the {} section under \"{}\" — moving it further would change \
             which section it belongs to",
            if down { "last" } else { "first" },
            sections[sib].title
        ));
    }

    let lines: Vec<&str> = text.split('\n').collect();
    // `a` is the earlier of the two, `b` the later; they are adjacent because sections tile.
    let (a, b) = if down { (idx, sib) } else { (sib, idx) };
    let (a_start, a_end) = (sections[a].start_line, sections[a].end_line);
    let (b_start, b_end) = (sections[b].start_line, sections[b].end_line);

    // A section's extent runs to the line before the next heading, so it usually ends in the
    // blank line(s) that separate the two. Those belong to the *gap*, not to either section:
    // swapping them with their section would move the blank line to the end of the pair and
    // leave `text` and the next `## heading` on adjacent lines. Splitting body from gap keeps
    // the separation the author wrote, and keeps the line count identical either way.
    let a_body_end = last_non_blank(&lines, a_start, a_end);
    let b_body_end = last_non_blank(&lines, b_start, b_end);

    let mut swapped: Vec<&str> = Vec::with_capacity(b_end + 1 - a_start);
    swapped.extend_from_slice(&lines[b_start..=b_body_end]); // the later section's body
    swapped.extend_from_slice(&lines[a_body_end + 1..=a_end]); // the gap that separated them
    swapped.extend_from_slice(&lines[a_start..=a_body_end]); // the earlier section's body
    swapped.extend_from_slice(&lines[b_body_end + 1..=b_end]); // whatever trailed the pair

    // Where the moved section's heading ends up, and the cursor's offset into it.
    let new_start = if down {
        a_start + (b_body_end + 1 - b_start) + (a_end - a_body_end)
    } else {
        a_start
    };
    let moved_body_end = if down { a_body_end } else { b_body_end };
    let offset = if line_in(cursor, sections[idx].start_line, moved_body_end) {
        cursor.line as usize - sections[idx].start_line
    } else {
        // The cursor sat in the blank gap after the section rather than in its body: put it
        // on the heading, which is the line the author was acting on.
        0
    };

    let mut new_text = swapped.join("\n");
    // The range below stops at the last line's end-of-line column, which never includes the
    // `\r` of a CRLF terminator (an LSP column cannot address it). That `\r` therefore
    // survives the replacement, so the text must not carry a second one.
    if new_text.ends_with('\r') {
        new_text.pop();
    }

    Ok(SectionEdit {
        edits: vec![TextEdit {
            range: Range {
                start: Position {
                    line: a_start as u32,
                    character: 0,
                },
                end: Position {
                    line: b_end as u32,
                    character: line_end_utf16(&lines, b_end),
                },
            },
            new_text,
        }],
        cursor: Some(Position {
            line: (new_start + offset) as u32,
            character: cursor.character,
        }),
    })
}

/// Add or remove one `#` on the section at `idx` and every heading nested under it.
///
/// The subtree travels with the heading: promoting a `##` that owns three `###`s and leaving
/// the children behind would make them siblings of their own parent.
fn change_level(
    sections: &[crate::lsp_outline::Section],
    idx: usize,
    op: SectionOp,
) -> Result<SectionEdit, String> {
    let level = sections[idx].level;
    let title = &sections[idx].title;
    // The contiguous run of deeper headings after `idx`: its descendants, because sections
    // tile and a descendant is exactly a following heading deeper than this one.
    let depth = sections[idx + 1..]
        .iter()
        .take_while(|s| s.level > level)
        .count();
    let subtree = &sections[idx..=idx + depth];

    let promote = op == SectionOp::Promote;
    if promote && level == 1 {
        return Err(format!("\"{title}\" is already a top-level `#` heading"));
    }
    if !promote && let Some(deepest) = subtree.iter().find(|s| s.level >= 6) {
        return Err(format!(
            "\"{}\" is already a `######` heading, the deepest Markdown has",
            deepest.title
        ));
    }

    // One `#` at the start of the heading line: a deletion or an insertion, never a rewrite
    // of the line. That is what leaves the cursor where the author left it — an edit that
    // replaced the whole line would drag the caret to its end.
    let edits = subtree
        .iter()
        .map(|s| TextEdit {
            range: Range {
                start: Position {
                    line: s.start_line as u32,
                    character: 0,
                },
                end: Position {
                    line: s.start_line as u32,
                    character: if promote { 1 } else { 0 },
                },
            },
            new_text: if promote {
                String::new()
            } else {
                "#".to_string()
            },
        })
        .collect();
    Ok(SectionEdit {
        edits,
        cursor: None,
    })
}

/// True when `cursor`'s line is within `start..=end`.
fn line_in(cursor: Position, start: usize, end: usize) -> bool {
    let line = cursor.line as usize;
    line >= start && line <= end
}

/// The last non-blank line in `start..=end`, or `start` when the whole span is blank.
///
/// `start` is a heading line, so it is never blank and the fallback cannot lose text.
fn last_non_blank(lines: &[&str], start: usize, end: usize) -> usize {
    (start..=end)
        .rev()
        .find(|&i| !lines[i].trim().is_empty())
        .unwrap_or(start)
}

/// A line's end-of-line column in UTF-16 code units, which is what LSP positions count.
fn line_end_utf16(lines: &[&str], line: usize) -> u32 {
    crate::lsp_pos::line_end_utf16(lines.get(line).copied().unwrap_or_default()) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Apply `edits` to `text` the way an editor would, so a test asserts on the *result*
    /// rather than on a range arithmetic nobody can read.
    fn apply(text: &str, edits: &[TextEdit]) -> String {
        let mut sorted: Vec<&TextEdit> = edits.iter().collect();
        sorted.sort_by_key(|e| (e.range.start.line, e.range.start.character));
        let mut out = text.to_string();
        // Back to front, so an earlier edit's offsets are still the original ones.
        for e in sorted.iter().rev() {
            let start = byte_offset(text, e.range.start);
            let end = byte_offset(text, e.range.end);
            out.replace_range(start..end, &e.new_text);
        }
        out
    }

    fn byte_offset(text: &str, p: Position) -> usize {
        let mut offset = 0;
        for (i, line) in text.split('\n').enumerate() {
            if i == p.line as usize {
                let col = crate::lsp_pos::utf16_to_char(line, p.character as usize);
                return offset
                    + line
                        .char_indices()
                        .nth(col)
                        .map(|(b, _)| b)
                        .unwrap_or(line.len());
            }
            offset += line.len() + 1; // the `\n`
        }
        text.len()
    }

    fn at(line: u32) -> Position {
        Position { line, character: 0 }
    }

    fn edit(text: &str, line: u32, op: SectionOp) -> String {
        let result = section_edit(text, at(line), op).expect("the transform should apply");
        apply(text, &result.edits)
    }

    const TWO: &str = "## Alpha\n\nfirst body\n\n## Beta\n\nsecond body\n";

    #[test]
    fn move_down_swaps_a_section_with_its_next_sibling() {
        assert_eq!(
            edit(TWO, 0, SectionOp::MoveDown),
            "## Beta\n\nsecond body\n\n## Alpha\n\nfirst body\n"
        );
    }

    #[test]
    fn move_up_is_the_inverse_of_move_down() {
        let down = edit(TWO, 0, SectionOp::MoveDown);
        // The cursor lands on "## Alpha" again, which is now line 4.
        assert_eq!(edit(&down, 4, SectionOp::MoveUp), TWO);
    }

    #[test]
    fn a_move_from_anywhere_inside_the_section_moves_the_whole_section() {
        // Cursor in the body, not on the heading: same result as from the heading line.
        assert_eq!(
            edit(TWO, 2, SectionOp::MoveDown),
            edit(TWO, 0, SectionOp::MoveDown)
        );
    }

    #[test]
    fn moving_keeps_the_blank_lines_that_separated_the_two_sections() {
        // The fixture has to be uneven to mean anything: two blank lines after Alpha's body,
        // none after Beta's (it ends at EOF). Swapping the extents wholesale then glues the
        // moved heading onto "second" and strands both blanks at the end of the file. An
        // evenly-spaced fixture passes either way — the first draft of this test used one,
        // and the mutant survived it.
        let uneven = "## Alpha\n\nfirst\n\n\n## Beta\n\nsecond";
        assert_eq!(
            edit(uneven, 0, SectionOp::MoveDown),
            "## Beta\n\nsecond\n\n\n## Alpha\n\nfirst"
        );
        // The invariant behind it, which holds for every fixture: a move only reorders lines.
        for text in [TWO, uneven] {
            let moved = edit(text, 0, SectionOp::MoveDown);
            assert_eq!(
                moved.split('\n').count(),
                text.split('\n').count(),
                "a move must not change the line count: {moved:?}"
            );
        }
    }

    #[test]
    fn moving_a_section_takes_its_subtree_with_it() {
        let text = "## Alpha\n\n### Alpha one\n\nbody\n\n## Beta\n\nbeta body\n";
        assert_eq!(
            edit(text, 0, SectionOp::MoveDown),
            "## Beta\n\nbeta body\n\n## Alpha\n\n### Alpha one\n\nbody\n"
        );
    }

    #[test]
    fn moving_a_child_stays_inside_its_parent() {
        let text = "## Parent\n\n### One\n\na\n\n### Two\n\nb\n\n## Next\n";
        let moved = edit(text, 2, SectionOp::MoveDown);
        assert_eq!(
            moved,
            "## Parent\n\n### Two\n\nb\n\n### One\n\na\n\n## Next\n"
        );
    }

    #[test]
    fn a_last_child_refuses_to_move_past_its_parents_boundary() {
        // "### Two" is the last child of "## Parent": moving it down would re-parent it
        // under "## Next", which is a structural change the author did not ask for.
        let text = "## Parent\n\n### One\n\na\n\n### Two\n\nb\n\n## Next\n";
        let err = section_edit(text, at(6), SectionOp::MoveDown).expect_err("should refuse");
        assert!(err.contains("\"Two\""), "{err}");
        assert!(err.contains("\"Next\""), "{err}");
    }

    #[test]
    fn a_first_child_refuses_to_move_above_its_parent() {
        let text = "## Parent\n\n### One\n\na\n\n### Two\n\nb\n";
        let err = section_edit(text, at(2), SectionOp::MoveUp).expect_err("should refuse");
        assert!(err.contains("\"One\""), "{err}");
        assert!(err.contains("\"Parent\""), "{err}");
    }

    #[test]
    fn the_last_section_at_its_level_refuses_to_move_down() {
        let err = section_edit(TWO, at(4), SectionOp::MoveDown).expect_err("should refuse");
        assert!(err.contains("\"Beta\""), "{err}");
        assert!(err.contains("last"), "{err}");
    }

    #[test]
    fn a_cursor_above_the_first_heading_refuses_with_a_reason() {
        let text = "---\ntitle: T\n---\n\nlead paragraph\n\n## Alpha\n";
        let err = section_edit(text, at(4), SectionOp::MoveDown).expect_err("should refuse");
        assert!(err.contains("above the first heading"), "{err}");
    }

    #[test]
    fn the_cursor_travels_with_the_moved_section() {
        // Cursor on "first body" (line 2, offset 2 into the section). After the move the
        // section starts at line 4, so the cursor belongs on line 6 — still on "first body".
        let result = section_edit(
            TWO,
            Position {
                line: 2,
                character: 3,
            },
            SectionOp::MoveDown,
        )
        .unwrap();
        let cursor = result.cursor.expect("a move returns a cursor");
        assert_eq!((cursor.line, cursor.character), (6, 3));
        let moved = apply(TWO, &result.edits);
        assert_eq!(
            moved.split('\n').nth(cursor.line as usize),
            Some("first body")
        );
    }

    #[test]
    fn a_hash_inside_a_code_cell_is_not_a_heading() {
        // The reason this shares `lsp_outline`'s scan instead of grepping for `^#`: a Python
        // comment would otherwise split the section and the move would cut mid-cell.
        let text = "## Alpha\n\n```{python}\n## not a heading\nprint(1)\n```\n\n## Beta\n\nb\n";
        let moved = edit(text, 0, SectionOp::MoveDown);
        assert_eq!(
            moved,
            "## Beta\n\nb\n\n## Alpha\n\n```{python}\n## not a heading\nprint(1)\n```\n"
        );
    }

    #[test]
    fn a_section_ending_at_eof_without_a_trailing_newline_moves_whole() {
        let text = "## Alpha\n\na\n\n## Beta\n\nb"; // no trailing newline
        assert_eq!(
            edit(text, 0, SectionOp::MoveDown),
            "## Beta\n\nb\n\n## Alpha\n\na"
        );
    }

    #[test]
    fn a_crlf_buffer_keeps_its_terminators() {
        // The pair has to be followed by a shallower heading, so the last line the edit
        // replaces is a CRLF line in the middle of the buffer. An LSP column cannot address
        // the `\r` of a terminator, so that one stays behind when the range ends at the
        // line's end — and the replacement must not bring a second one. (With the pair at
        // EOF the last replaced line is the empty tail of the final `\n` and nothing
        // collides, which is why the first version of this test passed with the fix removed.)
        let text = "# Top\r\n\r\n## A\r\n\r\na\r\n\r\n## B\r\n\r\nb\r\n\r\n# Next\r\n";
        let moved = edit(text, 2, SectionOp::MoveDown);
        assert_eq!(
            moved,
            "# Top\r\n\r\n## B\r\n\r\nb\r\n\r\n## A\r\n\r\na\r\n\r\n# Next\r\n"
        );
        assert!(
            !moved.contains("\r\r"),
            "doubled carriage return: {moved:?}"
        );
    }

    #[test]
    fn promote_lifts_the_heading_and_its_descendants() {
        let text = "## Alpha\n\n### Child\n\nbody\n\n## Beta\n";
        assert_eq!(
            edit(text, 0, SectionOp::Promote),
            "# Alpha\n\n## Child\n\nbody\n\n## Beta\n"
        );
    }

    #[test]
    fn demote_pushes_the_heading_and_its_descendants_down_one() {
        let text = "## Alpha\n\n### Child\n\nbody\n\n## Beta\n";
        assert_eq!(
            edit(text, 0, SectionOp::Demote),
            "### Alpha\n\n#### Child\n\nbody\n\n## Beta\n"
        );
    }

    #[test]
    fn re_levelling_leaves_every_other_line_byte_identical() {
        let text = "## Alpha\n\nbody with `##` in it\n\n### Child\n\n## Beta\n";
        let demoted = edit(text, 0, SectionOp::Demote);
        let (before, after): (Vec<&str>, Vec<&str>) =
            (text.split('\n').collect(), demoted.split('\n').collect());
        assert_eq!(before.len(), after.len(), "line count changed");
        for (i, (b, a)) in before.iter().zip(&after).enumerate() {
            if i == 0 || i == 4 {
                continue; // the two headings in the subtree
            }
            assert_eq!(b, a, "line {i} changed but is outside the subtree");
        }
    }

    #[test]
    fn promoting_adopts_a_following_sibling_so_demote_is_not_its_inverse() {
        // Not a defect, and worth a pin because it looks like one. "Alpha" and "Beta" are
        // siblings; promoting Alpha to `#` leaves Beta a `##`, which now sits *under* Alpha.
        // Demoting Alpha then correctly takes its whole (larger) subtree down with it.
        // Nothing else could be right: the subtree is read from the text as it stands, not
        // from what it looked like before the previous command. Org-mode does the same.
        let text = "## Alpha\n\n### Child\n\n## Beta\n";
        let promoted = edit(text, 0, SectionOp::Promote);
        assert_eq!(promoted, "# Alpha\n\n## Child\n\n## Beta\n");
        assert_eq!(
            edit(&promoted, 0, SectionOp::Demote),
            "## Alpha\n\n### Child\n\n### Beta\n"
        );
        // With no sibling to adopt, the pair IS a round trip — which is what makes holding
        // the two keys safe in the common case.
        let alone = "## Alpha\n\n### Child\n\nbody\n";
        assert_eq!(
            edit(&edit(alone, 0, SectionOp::Promote), 0, SectionOp::Demote),
            alone
        );
    }

    #[test]
    fn promote_refuses_a_top_level_heading() {
        let err = section_edit("# Alpha\n\nbody\n", at(0), SectionOp::Promote)
            .expect_err("should refuse");
        assert!(err.contains("already a top-level"), "{err}");
    }

    #[test]
    fn demote_refuses_when_a_descendant_would_pass_level_six() {
        let text = "##### Deep\n\n###### Deeper\n\nbody\n";
        let err = section_edit(text, at(0), SectionOp::Demote).expect_err("should refuse");
        assert!(err.contains("Deeper"), "{err}");
        // The refusal is about the descendant, not the heading under the cursor: demoting
        // `#####` alone is legal, and only the `######` under it makes it impossible.
        assert!(err.contains("######"), "{err}");
    }

    #[test]
    fn a_deck_slide_is_a_section_so_the_same_command_reorders_slides() {
        // The corpus deck is the pin: `##` is the slide level (`render::deck::SLIDE_LEVEL`),
        // so "move section down" IS "move this slide later" with no deck-specific code —
        // and this is the operation the removed drag-to-reorder gesture used to do.
        let deck = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/deck.tmd"),
        )
        .expect("corpus/deck.tmd");
        let slides = |text: &str| -> Vec<String> {
            crate::lsp_outline::sections(text)
                .into_iter()
                .filter(|s| s.level == 2)
                .map(|s| s.title)
                .collect()
        };
        let before = slides(&deck);
        assert!(before.len() > 3, "the corpus deck should have slides");
        let first = crate::lsp_outline::sections(&deck)
            .into_iter()
            .find(|s| s.level == 2)
            .expect("a level-2 slide");
        let moved = edit(&deck, first.start_line as u32, SectionOp::MoveDown);

        let after = slides(&moved);
        assert_eq!(after.len(), before.len(), "a slide was lost or gained");
        assert_eq!(
            (&after[0], &after[1]),
            (&before[1], &before[0]),
            "the first two slides should have traded places"
        );
        assert_eq!(
            moved.split('\n').count(),
            deck.split('\n').count(),
            "reordering must not change the line count"
        );
        // Same bytes, reordered: nothing was rewritten on the way through.
        let sorted = |t: &str| {
            let mut v: Vec<&str> = t.split('\n').collect();
            v.sort_unstable();
            v.join("\n")
        };
        assert_eq!(sorted(&moved), sorted(&deck), "a line changed content");
    }
}
