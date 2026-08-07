//! `textDocument/selectionRange`: expand-selection by *document* structure.
//!
//! Without this an editor falls back to expand-by-brackets, which is the same mistake
//! indentation folding is: it is a code editor's model of nesting applied to prose, where the
//! brackets are `[link](target)` and `{#sec-id}` and expanding by them selects nothing an
//! author was reaching for.
//!
//! The ladder is **word → sentence → paragraph → each enclosing structural block → the whole
//! document**. The structural half is not a second segmentation: it is
//! [`crate::lsp_fold::folding_ranges`], the same front matter / heading section / `:::` div /
//! code fence extents the gutter already folds by. An author who has folded this document
//! knows exactly which block each press of the key will land on, because it is the one they
//! can see the fold arrow for.

use lsp_types::{Position, Range, SelectionRange};

/// One `SelectionRange` chain per requested position, in the same order.
///
/// A position outside the buffer still gets an answer (the whole document), because the
/// request arrives with whatever the editor's cursors were when the key was pressed and a
/// short reply desynchronises them from their cursors.
pub(crate) fn selection_ranges(text: &str, positions: &[Position]) -> Vec<SelectionRange> {
    positions.iter().map(|p| chain(text, *p)).collect()
}

/// The innermost-to-outermost ladder for one cursor, linked into `parent` chain order.
fn chain(text: &str, pos: Position) -> SelectionRange {
    let lines: Vec<&str> = text.split('\n').collect();
    let line = pos.line as usize;
    let cursor = crate::lsp_pos::utf16_to_char(
        lines.get(line).copied().unwrap_or(""),
        pos.character as usize,
    );

    let mut ladder: Vec<Range> = Vec::new();
    if let Some((start, end)) = word_at(lines.get(line).copied().unwrap_or(""), cursor) {
        ladder.push(single_line(&lines, line, start, end));
    }
    let para = paragraph_at(&lines, line);
    if let Some((s, e)) = para
        && let Some(sentence) = sentence_at(&lines, s, e, line, cursor)
    {
        ladder.push(sentence);
    }
    if let Some((s, e)) = para {
        ladder.push(whole_lines(&lines, s, e));
    }
    // The structural blocks containing this line, innermost first. `folding_ranges` is
    // line-granular and unsorted, so the sort by width is what orders div inside section
    // inside document.
    let mut blocks: Vec<(u32, u32)> = crate::lsp_fold::folding_ranges(text)
        .into_iter()
        .filter(|r| r.start_line as usize <= line && line <= r.end_line as usize)
        .map(|r| (r.start_line, r.end_line))
        .collect();
    blocks.sort_by_key(|(s, e)| (e - s, *s));
    for (s, e) in blocks {
        ladder.push(whole_lines(&lines, s as usize, e as usize));
    }
    let last = lines.len().saturating_sub(1);
    ladder.push(whole_lines(&lines, 0, last));

    // A rung that does not strictly grow is a rung the author presses the key on and sees
    // nothing happen. Drop it rather than send a parent equal to its child, which the spec
    // forbids and clients handle inconsistently.
    ladder.dedup();
    let mut kept: Vec<Range> = Vec::new();
    for r in ladder {
        match kept.last() {
            Some(prev) if !strictly_contains(&r, prev) => continue,
            _ => kept.push(r),
        }
    }
    let mut node: Option<Box<SelectionRange>> = None;
    for range in kept.into_iter().rev() {
        node = Some(Box::new(SelectionRange {
            range,
            parent: node,
        }));
    }
    // `kept` always holds at least the whole-document rung, so the unwrap cannot fire; an
    // empty document is one line and still yields it.
    *node.unwrap_or_else(|| {
        Box::new(SelectionRange {
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
            parent: None,
        })
    })
}

/// Does `outer` cover `inner` and differ from it?
fn strictly_contains(outer: &Range, inner: &Range) -> bool {
    outer.start <= inner.start && inner.end <= outer.end && outer != inner
}

/// The maximal word-ish run under `cursor`, as scalar `[start, end)` columns. Hyphens and
/// underscores are inside a word here, because in this format the thing under the cursor is
/// as often `fig-scree` or `sec-intro` as it is an English word, and selecting `fig` alone is
/// never what was wanted.
fn word_at(line: &str, cursor: usize) -> Option<(usize, usize)> {
    let chars: Vec<char> = line.chars().collect();
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
    let n = chars.len();
    let mut start = cursor.min(n);
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = start;
    while end < n && is_word(chars[end]) {
        end += 1;
    }
    (end > start && cursor <= end).then_some((start, end))
}

/// The inclusive line extent of the paragraph containing `line`: the run of non-blank lines
/// around it, stopping at anything the *structural* rungs own (a heading, a `:::` fence, a
/// code fence) so the two layers do not both claim the same text.
fn paragraph_at(lines: &[&str], line: usize) -> Option<(usize, usize)> {
    let boundary = |l: &str| {
        let t = l.trim();
        t.is_empty()
            || t.starts_with(":::")
            || t == "---"
            || crate::lsp_outline::fence_marker(l).is_some()
            || crate::lsp_outline::atx_heading(l).is_some()
    };
    let cur = lines.get(line)?;
    if boundary(cur) {
        return None;
    }
    let mut start = line;
    while start > 0 && !boundary(lines[start - 1]) {
        start -= 1;
    }
    let mut end = line;
    while end + 1 < lines.len() && !boundary(lines[end + 1]) {
        end += 1;
    }
    Some((start, end))
}

/// The sentence under the cursor inside the paragraph `[first, last]`.
///
/// Boundaries are `.`/`!`/`?` followed by whitespace or end of paragraph — deliberately the
/// naive rule, because the alternative is an abbreviation dictionary and the cost of getting
/// it wrong here is one extra press of the expand key, not a wrong render.
fn sentence_at(
    lines: &[&str],
    first: usize,
    last: usize,
    line: usize,
    cursor: usize,
) -> Option<Range> {
    // Flatten the paragraph, remembering which (line, col) each char came from, so the
    // answer can span the soft line breaks a paragraph is written across.
    let mut flat: Vec<char> = Vec::new();
    let mut origin: Vec<(usize, usize)> = Vec::new();
    let mut at = None;
    for l in first..=last {
        let row: Vec<char> = lines.get(l).copied().unwrap_or("").chars().collect();
        for (c, ch) in row.iter().enumerate() {
            if l == line && c == cursor {
                at = Some(flat.len());
            }
            flat.push(*ch);
            origin.push((l, c));
        }
        if l == line && cursor >= row.len() {
            at = Some(flat.len());
        }
        if l < last {
            flat.push(' ');
            origin.push((l, row.len()));
        }
    }
    let at = at?.min(flat.len().saturating_sub(1));
    let ends: Vec<usize> = (0..flat.len())
        .filter(|&i| {
            matches!(flat[i], '.' | '!' | '?') && flat.get(i + 1).is_none_or(|c| c.is_whitespace())
        })
        .collect();
    let start = ends
        .iter()
        .rev()
        .find(|&&i| i < at)
        .map(|&i| {
            let mut s = i + 1;
            while flat.get(s).is_some_and(|c| c.is_whitespace()) {
                s += 1;
            }
            s
        })
        .unwrap_or(0);
    let end = ends
        .iter()
        .find(|&&i| i >= at)
        .map(|&i| i + 1)
        .unwrap_or(flat.len());
    if start >= end || end > origin.len() {
        return None;
    }
    let (sl, sc) = origin[start];
    let (el, ec) = origin[end - 1];
    Some(Range::new(
        Position::new(sl as u32, to_utf16(lines, sl, sc)),
        Position::new(el as u32, to_utf16(lines, el, ec + 1)),
    ))
}

/// A range covering lines `[first, last]` in full.
fn whole_lines(lines: &[&str], first: usize, last: usize) -> Range {
    let last = last.min(lines.len().saturating_sub(1));
    let end_col = lines
        .get(last)
        .map_or(0, |l| crate::lsp_pos::line_end_utf16(l)) as u32;
    Range::new(
        Position::new(first as u32, 0),
        Position::new(last as u32, end_col),
    )
}

/// A range on one line, from scalar columns.
fn single_line(lines: &[&str], line: usize, start: usize, end: usize) -> Range {
    Range::new(
        Position::new(line as u32, to_utf16(lines, line, start)),
        Position::new(line as u32, to_utf16(lines, line, end)),
    )
}

fn to_utf16(lines: &[&str], line: usize, col: usize) -> u32 {
    crate::lsp_pos::char_to_utf16(lines.get(line).copied().unwrap_or(""), col) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flatten one chain into the `(start_line, start_col, end_line, end_col)` rungs it
    /// offers, innermost first.
    fn rungs(text: &str, line: u32, character: u32) -> Vec<(u32, u32, u32, u32)> {
        let mut out = Vec::new();
        let mut node = Some(Box::new(chain(text, Position::new(line, character))));
        while let Some(n) = node {
            out.push((
                n.range.start.line,
                n.range.start.character,
                n.range.end.line,
                n.range.end.character,
            ));
            node = n.parent;
        }
        out
    }

    const DOC: &str = "\
# Intro

The scree slope is steep. It is also long.

## Method

::: {.callout-note}
Careful here.
:::
";

    #[test]
    fn the_first_rung_is_the_word_and_the_second_is_its_sentence() {
        // Line 2, on `scree` (cols 4..9).
        let got = rungs(DOC, 2, 5);
        assert_eq!(got[0], (2, 4, 2, 9), "the word: {got:?}");
        assert_eq!(
            got[1],
            (2, 0, 2, 25),
            "then the sentence, stopping at the full stop: {got:?}"
        );
    }

    #[test]
    fn a_hyphenated_reference_id_is_one_word() {
        // Selecting `fig` out of `@fig-scree` is never what the author reached for.
        let text = "See @fig-scree for the shape.\n";
        assert_eq!(rungs(text, 0, 8)[0], (0, 5, 0, 14));
    }

    #[test]
    fn the_paragraph_rung_covers_the_soft_wrapped_lines() {
        let text = "one two\nthree four\n\nnext\n";
        let got = rungs(text, 0, 1);
        assert!(
            got.contains(&(0, 0, 1, 10)),
            "expected the two-line paragraph: {got:?}"
        );
    }

    #[test]
    fn the_structural_rungs_come_from_the_folds_the_author_can_see() {
        // Inside the callout on line 7: the div, then the `## Method` section, then the doc.
        let got = rungs(DOC, 7, 3);
        assert!(
            got.contains(&(6, 0, 8, 3)),
            "the ::: div should be a rung: {got:?}"
        );
        assert!(
            got.iter().any(|r| r.0 == 4),
            "the enclosing `## Method` section should be a rung: {got:?}"
        );
        let last = got.last().copied().unwrap();
        assert_eq!(
            last.0, 0,
            "the outermost rung is the whole document: {got:?}"
        );
    }

    #[test]
    fn every_rung_strictly_contains_the_one_below_it() {
        for (line, col) in [(0u32, 2u32), (2, 5), (4, 3), (7, 3), (9, 0)] {
            let got = rungs(DOC, line, col);
            for pair in got.windows(2) {
                let (inner, outer) = (pair[0], pair[1]);
                assert!(
                    (outer.0, outer.1) <= (inner.0, inner.1)
                        && (inner.2, inner.3) <= (outer.2, outer.3)
                        && inner != outer,
                    "at {line}:{col}, {outer:?} must strictly contain {inner:?} — a rung that \
                     does not grow is a keypress that does nothing"
                );
            }
        }
    }

    #[test]
    fn a_position_past_the_end_still_answers() {
        // The editor sends whatever its cursors were; a short reply desyncs them.
        let got = selection_ranges(DOC, &[Position::new(999, 0), Position::new(2, 5)]);
        assert_eq!(got.len(), 2, "one chain per requested position");
    }

    /// UTF-16 is the wire unit, so a non-ASCII character earlier on the line shifts every
    /// column after it. This is the trap the whole `lsp_pos` boundary exists for.
    #[test]
    fn columns_are_utf16_units_not_scalars() {
        let text = "🌄🌄 scree here.\n";
        // `scree` starts at scalar col 3, which is UTF-16 col 5 (two astral chars = 4 units).
        assert_eq!(rungs(text, 0, 6)[0], (0, 5, 0, 10));
    }
}
