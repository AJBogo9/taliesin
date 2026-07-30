//! `textDocument/formatting` for `.tmd`: **pipe tables, and whitespace that renders as
//! nothing.**
//!
//! A document formatter for a prose format is mostly a way to lose an argument with the
//! author. Re-wrapping paragraphs fights every deliberate line break, reflowing lists fights
//! the nesting they chose, and normalizing emphasis markers rewrites bytes that render
//! identically. **None of that is here and none of it is coming**: the corpus measured
//! 86 of 174 documents hand-wrapped and 379 prose lines past 100 columns, so there is not
//! even a house style for a re-wrapper to enforce, and the companion contributes
//! `editor.wordWrap: on` precisely so a long logical line is a *display* question.
//!
//! What it does have is two jobs, and the second is admissible for a reason the first one
//! cannot claim: **every whitespace edit here renders to byte-identical HTML.** Trailing
//! spaces that are not a hard break, a run of three or more blank lines, a missing or
//! doubled final newline — the parser already ignores all of them, so normalizing them is
//! not a style opinion, it is deleting bytes with no meaning. That claim is *pinned*, over
//! every document in `corpus/` and `docs/`, by
//! `formatting_the_whole_corpus_renders_identical_html`: format the file, render both, and
//! the HTML must match apart from the line numbers.
//!
//! **The line numbers are allowed to move**, which is the thing this module used to promise
//! they never would. Collapsing blank lines changes the line count, so every
//! `data-sourcepos` below it moves — and that is handled, by `BlockOp::SetMeta`, which
//! patches a shifted block's position metadata while leaving its content and its live DOM
//! state (video playback, `{js}` widgets, open `<details>`) alone. The table formatter's
//! one-to-one property is still pinned separately, because it is still true of tables.
//!
//! **Where it declines.** A table whose body row has MORE cells than its header is left
//! exactly as written. GFM ignores the extras when rendering, so "formatting" it would mean
//! either deleting the author's text or widening the delimiter row — and widening the
//! delimiter row past the header count stops it being a table at all. A malformed table is
//! the last place a formatter should be confident. Whitespace declines in the same spirit:
//! anything inside a fence, anything indented far enough to *be* an indented code block, and
//! any trailing run of two or more spaces (which is a hard line break, not an accident).

use crate::lsp_cells::code_line_mask;

/// One whole-line replacement: `start_line..=end_line` (0-based, inclusive) becomes
/// `new_text`. Line-granular by design — every edit this module emits either rewrites whole
/// lines or nothing, which is what makes "no line outside an edit can change" checkable.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LineEdit {
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
    pub(crate) new_text: String,
}

/// Everything "Format Document" does: tables, then whitespace, as one sorted,
/// non-overlapping edit list.
///
/// The two passes cannot collide because the whitespace pass skips every line a table edit
/// covers — a reformatted row already comes back with its trailing space normalized, and two
/// edits naming the same line is a protocol error, not a merge.
pub(crate) fn format_edits(text: &str) -> Vec<LineEdit> {
    let tables = format_tables(text);
    let mut out = whitespace_edits(text, &tables);
    out.extend(tables);
    out.sort_by_key(|e| e.start_line);
    out
}

/// The whitespace normalizations, all of them render-identical: trailing whitespace that is
/// not a hard break, runs of three or more blank lines, and the final newline.
fn whitespace_edits(text: &str, tables: &[LineEdit]) -> Vec<LineEdit> {
    let lines: Vec<&str> = text.split('\n').collect();
    let code = code_line_mask(text);
    let in_table = |line: usize| {
        tables
            .iter()
            .any(|t| line >= t.start_line && line <= t.end_line)
    };
    // A line indented four spaces (or a tab) may BE an indented code block, where trailing
    // spaces and interior blank lines are content. Telling one from a list continuation needs
    // the block structure, and guessing wrong rewrites code — so the whole class is declined.
    let indented = |line: usize| {
        let l: &str = lines[line];
        l.starts_with("    ") || l.starts_with('\t')
    };
    let skip = |line: usize| code[line] || in_table(line) || indented(line);

    let mut out = Vec::new();

    // (1) Trailing whitespace, line by line.
    for (i, line) in lines.iter().enumerate() {
        if skip(i) {
            continue;
        }
        // Whether a trailing two-space run on this line can be a hard break at all: a break
        // needs a following line in the same paragraph. At the end of a block (next line
        // blank, or this the last line) the renderer discards it, so it is noise like any
        // other trailing space. Getting this wrong in the other direction only ever *keeps*
        // two spaces, which changes nothing.
        let continues = lines
            .get(i + 1)
            .is_some_and(|next| !next.trim().is_empty() && !code[i + 1]);
        let trimmed = trim_trailing(line, continues);
        if trimmed != *line {
            out.push(LineEdit {
                start_line: i,
                end_line: i,
                new_text: trimmed,
            });
        }
    }

    // (2) Blank-line runs of three or more collapse to one. Skipped next to an indented
    // block, where a blank line can be part of the code.
    let blank = |i: usize| lines[i].trim().is_empty() && !code[i];
    let mut i = 0;
    while i < lines.len() {
        if !blank(i) {
            i += 1;
            continue;
        }
        let mut end = i;
        while end + 1 < lines.len() && blank(end + 1) {
            end += 1;
        }
        let run = end - i + 1;
        let touches_indented =
            (i > 0 && indented(i - 1)) || (end + 1 < lines.len() && indented(end + 1));
        // The final run is the file's trailing newlines; (3) owns that one.
        let at_eof = end + 1 == lines.len();
        if run >= 3 && !touches_indented && !at_eof {
            // Drop the edits from (1) that this run swallows, or two edits would name one line.
            out.retain(|e| e.start_line < i || e.start_line > end);
            out.push(LineEdit {
                start_line: i,
                end_line: end,
                new_text: String::new(),
            });
        }
        i = end + 1;
    }

    // (3) Exactly one final newline: `split('\n')` gives one trailing empty element for a
    // file that ends in a newline, so "tidy" is exactly one empty last line.
    let last = lines.len() - 1;
    // Inside an unclosed fence the trailing blank lines are part of the code block.
    if !code[last] {
        // A raw HTML block is passed through verbatim, so a newline added at the end of one
        // lands in the *output*: `corpus/posts/em-algorithm/index.tmd` ends in a `-->` with no
        // final newline and rendered `-->\n`; adding the newline rendered `-->\n\n`. Harmless
        // in a browser, but the whole licence for this pass is that the rendered bytes do not
        // move, so this case declines rather than argues. (Found by the corpus gate, not by
        // reasoning about it.)
        //
        // Two shapes, because they end differently. A `<div>` block ends at a blank line, so
        // walking back to one and asking whether that block opened with `<` finds it. A
        // **comment** ends only at `-->` and can contain blank lines, so it needs its own
        // scan — the walk-back lands somewhere in the middle of the commented-out text and
        // sees ordinary prose.
        let raw_html_tail = {
            let mut start = last;
            while start > 0 && !lines[start - 1].trim().is_empty() {
                start -= 1;
            }
            lines[start].trim_start().starts_with('<')
        };
        let comment_tail = lines[last].trim_end().ends_with("-->") || {
            let mut inside = false;
            for (i, line) in lines.iter().enumerate() {
                if code[i] {
                    continue;
                }
                let mut rest: &str = line;
                loop {
                    let marker = if inside { "-->" } else { "<!--" };
                    match rest.find(marker) {
                        Some(at) => {
                            inside = !inside;
                            rest = &rest[at + marker.len()..];
                        }
                        None => break,
                    }
                }
            }
            inside
        };
        if !lines[last].is_empty()
            && !lines[last].trim().is_empty()
            && !raw_html_tail
            && !comment_tail
        {
            // No final newline at all: give the last line one.
            out.retain(|e| e.start_line != last);
            out.push(LineEdit {
                start_line: last,
                end_line: last,
                new_text: format!("{}\n", trim_trailing(lines[last], false)),
            });
        } else {
            // One or more blank lines at the end: keep a single empty one.
            let mut first = last;
            while first > 0 && lines[first - 1].trim().is_empty() && !code[first - 1] {
                first -= 1;
            }
            if first < last {
                out.retain(|e| e.start_line < first);
                out.push(LineEdit {
                    start_line: first,
                    end_line: last,
                    new_text: String::new(),
                });
            }
        }
    }

    out.sort_by_key(|e| e.start_line);
    out
}

/// A line with meaningless trailing whitespace removed.
///
/// Two or more trailing spaces are a **hard line break** when a line follows them in the same
/// paragraph (`continues`), so those survive — normalized to exactly two, since a third space
/// adds nothing a parser can see. A single trailing space, any run containing a tab (which
/// CommonMark does not accept for a break), and any run at the end of a block go entirely. A
/// line of nothing but whitespace becomes empty: a blank line is a blank line either way.
fn trim_trailing(line: &str, continues: bool) -> String {
    let body = line.trim_end();
    if body.is_empty() {
        return String::new();
    }
    let tail = &line[body.len()..];
    if continues && tail.len() >= 2 && tail.bytes().all(|b| b == b' ') {
        format!("{body}  ")
    } else {
        body.to_string()
    }
}

/// Column alignment, as the delimiter row spells it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Align {
    None,
    Left,
    Center,
    Right,
}

/// Every table in `text` whose formatting would change, as one edit each.
///
/// Tables that are already formatted produce no edit at all, so "Format Document" on a tidy
/// file is a no-op rather than a diff full of identical lines.
pub(crate) fn format_tables(text: &str) -> Vec<LineEdit> {
    let lines: Vec<&str> = text.split('\n').collect();
    let code = code_line_mask(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        if code[i] || code[i + 1] {
            i += 1;
            continue;
        }
        let Some(edit) = table_at(&lines, &code, i) else {
            i += 1;
            continue;
        };
        let next = edit.end_line + 1;
        if replaced_lines(&lines, edit.start_line, edit.end_line) != edit.new_text {
            out.push(edit);
        }
        i = next;
    }
    out
}

/// The original text of an inclusive line range, joined the way an edit replacing it must be.
fn replaced_lines(lines: &[&str], start: usize, end: usize) -> String {
    lines[start..=end].join("\n")
}

/// A table starting at `start` (its header row), or `None`.
fn table_at(lines: &[&str], code: &[bool], start: usize) -> Option<LineEdit> {
    let header = split_row(lines[start]);
    if header.len() < 2 || !lines[start].contains('|') {
        return None;
    }
    let delims = split_row(lines[start + 1]);
    // GFM makes this the definition of a table: the delimiter row must have exactly as many
    // cells as the header. Anything else is a paragraph that happens to contain pipes.
    if delims.len() != header.len() || !delims.iter().all(|c| is_delimiter_cell(c)) {
        return None;
    }
    let ncol = header.len();
    let aligns: Vec<Align> = delims.iter().map(|c| align_of(c)).collect();

    // The body runs while lines keep looking like rows: a pipe, not code, not blank.
    let mut rows = vec![header];
    let mut end = start + 1;
    let mut j = start + 2;
    while j < lines.len() && !code[j] && lines[j].contains('|') && !lines[j].trim().is_empty() {
        let row = split_row(lines[j]);
        // Declining, not truncating — see the module docs.
        if row.len() > ncol {
            return None;
        }
        rows.push(row);
        end = j;
        j += 1;
    }

    // Widths span the header and every body row. A minimum of three keeps `:-:` legal for a
    // centred column and keeps a one-character column from rendering as a bare `-`.
    let mut widths = vec![3usize; ncol];
    for row in &rows {
        for (c, cell) in row.iter().enumerate() {
            widths[c] = widths[c].max(display_width(cell));
        }
    }

    let indent: String = lines[start]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let mut new_text = String::new();
    for (n, row) in rows.iter().enumerate() {
        if n > 0 {
            new_text.push('\n');
        }
        new_text.push_str(&indent);
        new_text.push_str(&render_row(row, &widths, &aligns, ncol));
        // The delimiter row sits between the header and the first body row.
        if n == 0 {
            new_text.push('\n');
            new_text.push_str(&indent);
            new_text.push_str(&render_delims(&widths, &aligns));
        }
    }
    Some(LineEdit {
        start_line: start,
        end_line: end,
        new_text,
    })
}

/// `| a   | b   |`, each cell padded to its column and set by its alignment. A row shorter
/// than the header is padded with empty cells, which is what GFM renders anyway.
fn render_row(row: &[String], widths: &[usize], aligns: &[Align], ncol: usize) -> String {
    let mut s = String::from("|");
    for c in 0..ncol {
        let empty = String::new();
        let cell = row.get(c).unwrap_or(&empty);
        s.push(' ');
        s.push_str(&pad(cell, widths[c], aligns[c]));
        s.push_str(" |");
    }
    s
}

fn render_delims(widths: &[usize], aligns: &[Align]) -> String {
    let mut s = String::from("|");
    for (w, a) in widths.iter().zip(aligns) {
        let w = *w;
        let bar = match a {
            Align::None => "-".repeat(w),
            Align::Left => format!(":{}", "-".repeat(w - 1)),
            Align::Right => format!("{}:", "-".repeat(w - 1)),
            Align::Center => format!(":{}:", "-".repeat(w - 2)),
        };
        s.push(' ');
        s.push_str(&bar);
        s.push_str(" |");
    }
    s
}

fn pad(cell: &str, width: usize, align: Align) -> String {
    let short = width.saturating_sub(display_width(cell));
    match align {
        Align::Right => format!("{}{cell}", " ".repeat(short)),
        Align::Center => {
            let left = short / 2;
            format!("{}{cell}{}", " ".repeat(left), " ".repeat(short - left))
        }
        _ => format!("{cell}{}", " ".repeat(short)),
    }
}

/// Column width in characters.
///
/// Deliberately NOT grapheme- or East-Asian-width-aware. Alignment here is cosmetic — the
/// rendered HTML is identical however the source is padded — so the cost of being wrong is a
/// ragged column in the editor, and the cost of being right is a new dependency in a tool
/// that vendors its assets to stay offline.
fn display_width(cell: &str) -> usize {
    cell.chars().count()
}

/// Split a row on unescaped `|`, dropping the optional leading and trailing delimiters and
/// trimming each cell.
///
/// The escape rule is the only piece of GFM this file has to restate, and it is the one that
/// matters: `\|` is a literal pipe inside a cell, so a naive `split('|')` turns one cell into
/// two and the formatter silently corrupts the table it was asked to tidy.
fn split_row(line: &str) -> Vec<String> {
    let t = line.trim();
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut escaped = false;
    let mut ended_with_delim = false;
    for c in t.chars() {
        if escaped {
            cur.push(c);
            escaped = false;
            ended_with_delim = false;
            continue;
        }
        match c {
            '\\' => {
                cur.push('\\');
                escaped = true;
                ended_with_delim = false;
            }
            '|' => {
                cells.push(std::mem::take(&mut cur).trim().to_string());
                ended_with_delim = true;
            }
            _ => {
                cur.push(c);
                ended_with_delim = false;
            }
        }
    }
    cells.push(cur.trim().to_string());
    if ended_with_delim {
        cells.pop();
    }
    if t.starts_with('|') && !cells.is_empty() {
        cells.remove(0);
    }
    cells
}

/// `---`, `:--`, `--:`, `:-:` — at least one dash, optional colons at either end.
fn is_delimiter_cell(cell: &str) -> bool {
    let body = cell
        .strip_prefix(':')
        .unwrap_or(cell)
        .strip_suffix(':')
        .unwrap_or_else(|| cell.strip_prefix(':').unwrap_or(cell));
    !body.is_empty() && body.chars().all(|c| c == '-')
}

fn align_of(cell: &str) -> Align {
    match (cell.starts_with(':'), cell.ends_with(':')) {
        (true, true) => Align::Center,
        (true, false) => Align::Left,
        (false, true) => Align::Right,
        (false, false) => Align::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Apply every edit to `text`, so a test asserts on the DOCUMENT rather than on an edit
    /// list — which is what actually reaches the author's buffer.
    fn formatted(text: &str) -> String {
        let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        for edit in format_tables(text).into_iter().rev() {
            let replacement: Vec<String> = edit.new_text.split('\n').map(str::to_string).collect();
            lines.splice(edit.start_line..=edit.end_line, replacement);
        }
        lines.join("\n")
    }

    #[test]
    fn ragged_columns_are_aligned_and_alignment_markers_survive() {
        let src = "| a | long header | c |\n|:--|--:|:-:|\n| 1 | 2 | 3 |\n";
        // Cells are set the way the delimiter row says: left, right, centred.
        assert_eq!(
            formatted(src),
            "| a   | long header |  c  |\n\
             | :-- | ----------: | :-: |\n\
             | 1   |           2 |  3  |\n"
        );
    }

    /// The promise the whole module rests on: a document with no table comes back unchanged,
    /// and one with a table changes only the table's lines.
    #[test]
    fn nothing_outside_a_table_is_touched() {
        let prose =
            "---\ntitle: T\n---\n\n# Heading\n\nA paragraph with a | pipe in it.\n\n- a list\n";
        assert_eq!(formatted(prose), prose, "no table, no edits");
        assert!(format_tables(prose).is_empty());

        let mixed = "Before.\n\n|a|b|\n|-|-|\n|1|2|\n\nAfter.\n";
        assert_eq!(
            formatted(mixed),
            "Before.\n\n| a   | b   |\n| --- | --- |\n| 1   | 2   |\n\nAfter.\n"
        );
    }

    /// A table already formatted must produce NO edit. Otherwise format-on-save rewrites the
    /// same bytes on every save and every one of them lands in the undo stack.
    #[test]
    fn an_already_formatted_table_produces_no_edit() {
        let src = "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n";
        assert!(format_tables(src).is_empty(), "{:?}", format_tables(src));
    }

    /// The escape rule. A naive `split('|')` reads three cells here and the formatter would
    /// write the table back with the author's text redistributed into the wrong columns.
    #[test]
    fn an_escaped_pipe_stays_inside_its_cell() {
        assert_eq!(split_row(r"| a \| b | c |"), vec![r"a \| b", "c"]);
        let src = "| expr | means |\n|-|-|\n| a \\| b | or |\n";
        assert_eq!(
            formatted(src),
            "| expr   | means |\n| ------ | ----- |\n| a \\| b | or    |\n"
        );
    }

    /// A table inside a fence is an EXAMPLE of a table. `code_line_mask` is shared with the
    /// cell scanner precisely so this cannot drift from what the renderer treats as code.
    #[test]
    fn a_table_inside_a_code_fence_is_left_alone() {
        let src = "Docs:\n\n```markdown\n|a|b|\n|-|-|\n|1|2|\n```\n";
        assert_eq!(formatted(src), src);
        assert!(format_tables(src).is_empty());
    }

    /// Declining is a feature: a row with more cells than the header cannot be formatted
    /// without deleting text or breaking the table, so it is returned untouched.
    #[test]
    fn a_row_with_more_cells_than_the_header_is_declined() {
        let src = "|a|b|\n|-|-|\n|1|2|3|\n";
        assert!(format_tables(src).is_empty());
        assert_eq!(formatted(src), src);
    }

    /// A short row is padded, because that is what GFM renders anyway — the cells exist, they
    /// are just empty.
    #[test]
    fn a_short_row_is_padded_to_the_column_count() {
        assert_eq!(
            formatted("|a|b|\n|-|-|\n|1|\n"),
            "| a   | b   |\n| --- | --- |\n| 1   |     |\n"
        );
    }

    /// Rows need neither leading nor trailing pipes in GFM; the formatter normalizes to both.
    #[test]
    fn a_row_without_outer_pipes_is_still_a_row() {
        assert_eq!(
            formatted("a | b\n--- | ---\n1 | 2\n"),
            "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n"
        );
    }

    /// Two pipes in a paragraph are not a table: without a delimiter row there is nothing to
    /// format, and treating it as one would wreck ordinary prose.
    #[test]
    fn pipes_without_a_delimiter_row_are_prose() {
        assert!(format_tables("a | b\nc | d\n").is_empty());
    }

    /// An indented table keeps its indentation, so a table inside a `:::` div or a list item
    /// does not jump to column zero.
    #[test]
    fn indentation_is_preserved() {
        assert_eq!(
            formatted("  |a|b|\n  |-|-|\n  |1|2|\n"),
            "  | a   | b   |\n  | --- | --- |\n  | 1   | 2   |\n"
        );
    }

    /// **Table formatting must not move any line.** A table's rows map one-to-one onto its
    /// lines, so the replacement has exactly the line count of the range it replaces. That is
    /// a property worth pinning, not assuming, because a future "collapse a blank row" or
    /// "split a long cell" would quietly break it.
    ///
    /// It is deliberately scoped to `format_tables` and no longer claims anything about the
    /// document formatter as a whole. Collapsing a run of blank lines *does* change the line
    /// count, and the reason that is now allowed is `BlockOp::SetMeta`: a block whose content
    /// is unchanged and whose lines moved has its `data-sourcepos` patched in place, keeping
    /// click-to-source correct and its live DOM state alive. What guards the whitespace pass
    /// instead is `formatting_the_whole_corpus_renders_identical_html`.
    #[test]
    fn table_formatting_never_changes_the_line_count() {
        for src in [
            "|a|b|\n|-|-|\n|1|2|\n",
            "  |a|b|\n  |-|-|\n  |1|\n",
            "text\n\n|a|long header|c|\n|:-|-:|:-:|\n|1|2|3|\n|4|5|6|\n\nmore\n",
            "a | b\n--- | ---\n1 | 2\n",
        ] {
            let before = src.split('\n').count();
            let after = formatted(src).split('\n').count();
            assert_eq!(
                before, after,
                "line count moved, which moves every data-sourcepos below it: {src:?}"
            );
            // And per edit, so a multi-table document cannot cancel two errors out.
            for edit in format_tables(src) {
                assert_eq!(
                    edit.new_text.split('\n').count(),
                    edit.end_line - edit.start_line + 1,
                    "edit replaces {} lines with a different number: {edit:?}",
                    edit.end_line - edit.start_line + 1
                );
            }
        }
    }

    #[test]
    fn two_tables_in_one_document_both_format() {
        let src = "|a|b|\n|-|-|\n|1|2|\n\ntext\n\n|c|d|\n|-|-|\n|3|4|\n";
        assert_eq!(format_tables(src).len(), 2);
        assert_eq!(
            formatted(src),
            "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n\ntext\n\n\
             | c   | d   |\n| --- | --- |\n| 3   | 4   |\n"
        );
    }

    // --- The whitespace pass (backlog item 166) ------------------------------------------
    //
    // Every case below is a byte the parser already ignores. The class-level proof that this
    // is true is `formatting_the_whole_corpus_renders_identical_html` at the end; these pin
    // the individual rules, and above all the ones where the formatter must NOT act.

    /// Apply the whole document formatter, the way the editor does.
    fn whole(text: &str) -> String {
        let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        for edit in format_edits(text).into_iter().rev() {
            let replacement: Vec<String> = edit.new_text.split('\n').map(str::to_string).collect();
            lines.splice(edit.start_line..=edit.end_line, replacement);
        }
        lines.join("\n")
    }

    #[test]
    fn trailing_whitespace_goes_but_a_hard_break_stays() {
        assert_eq!(whole("text   \n"), "text\n");
        assert_eq!(whole("text \n"), "text\n");
        assert_eq!(whole("text\t\n"), "text\n");
        // Two trailing spaces are a hard line break, which renders as a `<br>`: keeping them
        // is the difference between formatting the file and editing the document.
        assert_eq!(whole("line  \nnext\n"), "line  \nnext\n");
        // Three or more is that same break plus noise; the break survives, the noise does not.
        assert_eq!(whole("line    \nnext\n"), "line  \nnext\n");
        // A run with a tab is not a break at all (CommonMark counts spaces only).
        assert_eq!(whole("line \t\nnext\n"), "line\nnext\n");
        // A line of nothing but spaces is a blank line either way.
        assert_eq!(whole("a\n   \nb\n"), "a\n\nb\n");
    }

    #[test]
    fn a_tidy_document_produces_no_edits_at_all() {
        // "Format Document" on a clean file must be a no-op, or every save shows a diff.
        for src in [
            "# Title\n\ntext\n",
            "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n",
            "para\n\nline  \nbreak\n",
        ] {
            assert_eq!(format_edits(src), Vec::new(), "{src:?} should be untouched");
        }
    }

    #[test]
    fn formatting_is_idempotent() {
        for src in [
            "a   \n\n\n\n\nb\t\n\n\n",
            "|a|b   |\n|-|-|\n|1|2|\n\n\n\nx",
            "text",
            "\n\n\n",
        ] {
            let once = whole(src);
            assert_eq!(
                format_edits(&once),
                Vec::new(),
                "a second pass still wants to change {src:?} -> {once:?}"
            );
        }
    }

    #[test]
    fn three_or_more_blank_lines_collapse_to_one() {
        assert_eq!(whole("a\n\n\n\nb\n"), "a\n\nb\n");
        // Two blank lines are left alone: they render identically, but so does one, and the
        // formatter's job stops at bytes with no meaning rather than bytes it would prefer.
        assert_eq!(whole("a\n\n\nb\n"), "a\n\n\nb\n");
    }

    #[test]
    fn the_file_ends_with_exactly_one_newline() {
        assert_eq!(whole("text"), "text\n");
        assert_eq!(whole("text\n\n\n"), "text\n");
        assert_eq!(whole("text\n"), "text\n");
    }

    #[test]
    fn nothing_inside_a_fence_is_touched() {
        // Trailing spaces in a `{python}` cell are the author's code, and a blank run inside a
        // fence is part of the output. `code_line_mask` is shared with the table pass so
        // "what is code" has one definition.
        let src = "```{python}\nx = 1   \n\n\n\n\nprint(x)\n```\n";
        assert_eq!(format_edits(src), Vec::new(), "a fence must be untouched");
    }

    #[test]
    fn an_indented_code_block_is_declined() {
        // Four-space indentation may BE a code block, where a trailing space and an interior
        // blank line are both content. Telling that from a list continuation needs the block
        // structure, so the whole class is left alone.
        let src = "para\n\n    code   \n\n\n\n    more\n";
        assert_eq!(
            format_edits(src),
            Vec::new(),
            "indented code must be untouched"
        );
    }

    #[test]
    fn a_table_line_is_formatted_once_not_twice() {
        // Both passes have an opinion about `|a|b|   `: the table pass rewrites the row (and
        // drops the trailing space on the way), the whitespace pass would trim it. Two edits
        // naming one line is a protocol error, so the whitespace pass yields.
        let src = "|a|b|   \n|-|-|\n|1|2|\n";
        let edits = format_edits(src);
        assert_eq!(edits.len(), 1, "one edit per line: {edits:?}");
        assert_eq!(whole(src), "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n");
    }

    #[test]
    fn edits_are_sorted_and_never_overlap() {
        let src = "a   \n\n\n\n\nb  \t\n|x|y|\n|-|-|\n|1|2|   \n\n\n\nc";
        let edits = format_edits(src);
        assert!(edits.len() >= 3, "expected several edits, got {edits:?}");
        for pair in edits.windows(2) {
            assert!(
                pair[0].end_line < pair[1].start_line,
                "overlapping or unsorted edits: {:?} then {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    /// **The claim that licenses the whole whitespace pass**, over every document in the
    /// corpus and both dogfooded books: formatting a file changes no rendered *content*.
    ///
    /// Two attributes are masked, and both are metadata rather than content.
    /// `data-sourcepos` moves because a collapsed blank run shifts every line below it —
    /// that is exactly the difference `BlockOp::SetMeta` exists to carry.
    /// `data-block-id` is a content hash **of the block's source**, so trimming a trailing
    /// space re-ids that one block even though it renders identically; the cost is that the
    /// block remounts (losing live DOM state inside it) on an explicit Format Document, which
    /// is a price the author asked for by running the command.
    ///
    /// Everything else — every tag, every attribute, every character of text — must be
    /// byte-identical. A re-wrapping formatter could not pass this test at all, which is
    /// precisely why this is the gate and not a promise in a comment.
    #[test]
    fn formatting_the_whole_corpus_renders_identical_html() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let docs = collect_tmd(&root.join("corpus"))
            .into_iter()
            .chain(collect_tmd(&root.join("docs")))
            .collect::<Vec<_>>();
        assert!(
            docs.len() > 100,
            "only {} documents found; the walk stopped working and this gate would pass \
             vacuously",
            docs.len()
        );

        let mut changed = 0;
        for path in &docs {
            let text = std::fs::read_to_string(path).unwrap();
            if format_edits(&text).is_empty() {
                continue;
            }
            changed += 1;
            let uri = lsp_types::Url::from_file_path(path).unwrap();
            let Some(before) = crate::lsp::render_buffer(&uri, &text) else {
                continue; // a document this build cannot render is not this test's business
            };
            let after = crate::lsp::render_buffer(&uri, &whole(&text))
                .expect("a formatted document must still render");
            let (a, b) = (
                mask_metadata(&before.body_html()),
                mask_metadata(&after.body_html()),
            );
            if a != b {
                // A localized report: these documents render to hundreds of kilobytes, and an
                // assert_eq! of two of them says nothing a reader can act on.
                let at = a
                    .char_indices()
                    .zip(b.char_indices())
                    .find(|((_, x), (_, y))| x != y)
                    .map(|((i, _), _)| i)
                    .unwrap_or(a.len().min(b.len()));
                let window = |s: &str| {
                    let start = s[..at.min(s.len())]
                        .char_indices()
                        .rev()
                        .nth(120)
                        .map_or(0, |(i, _)| i);
                    let end = s[at.min(s.len())..]
                        .char_indices()
                        .nth(120)
                        .map_or(s.len(), |(i, _)| at + i);
                    s[start..end].to_string()
                };
                panic!(
                    "formatting changed the rendered HTML of {}\n  before: {:?}\n  after:  {:?}",
                    path.display(),
                    window(&a),
                    window(&b)
                );
            }
        }
        // If the corpus were already tidy everywhere, the loop above would compare nothing.
        assert!(
            changed > 0,
            "no document in the corpus needed formatting, so nothing was actually compared"
        );
    }

    fn collect_tmd(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            // `_freeze` is cached cell output, `_site`/`_book` are build products.
            if path.is_dir() {
                if !name.to_string_lossy().starts_with('_') {
                    out.extend(collect_tmd(&path));
                }
            } else if path.extension().is_some_and(|e| e == "tmd") {
                out.push(path);
            }
        }
        out
    }

    /// Position metadata and content-hash identity blanked, so two renders compare on content
    /// alone. The attributes themselves stay, so a block that appeared or vanished is still a
    /// difference this can see.
    ///
    /// Block ids are masked wherever they appear rather than attribute by attribute: a
    /// `b-<hash>` is *referenced* as well as declared (`data-section-end` names the block a
    /// section ends on), and chasing each referencing attribute as the gate finds it is how a
    /// test ends up describing the renderer's attribute list instead of its content.
    fn mask_metadata(html: &str) -> String {
        mask_block_ids(&mask_attr(html, "data-sourcepos=\""))
    }

    /// Every `b-<hex…>` token replaced by `b-…`.
    fn mask_block_ids(html: &str) -> String {
        let mut out = String::with_capacity(html.len());
        let mut rest = html;
        while let Some(at) = rest.find("b-") {
            let (head, tail) = rest.split_at(at + 2);
            let hex = tail.chars().take_while(|c| c.is_ascii_hexdigit()).count();
            out.push_str(head);
            if hex >= 6 {
                out.push('…');
                rest = &tail[hex..];
            } else {
                rest = tail;
            }
        }
        out.push_str(rest);
        out
    }

    fn mask_attr(html: &str, attr: &str) -> String {
        let mut out = String::with_capacity(html.len());
        let mut rest = html;
        while let Some(at) = rest.find(attr) {
            let (head, tail) = rest.split_at(at + attr.len());
            out.push_str(head);
            match tail.find('"') {
                Some(end) => {
                    out.push_str("…\"");
                    rest = &tail[end + 1..];
                }
                None => {
                    rest = tail;
                    break;
                }
            }
        }
        out.push_str(rest);
        out
    }
}
