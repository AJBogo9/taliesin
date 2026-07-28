//! `textDocument/formatting` for `.tmd`: **pipe tables, and nothing else.**
//!
//! A document formatter for a prose format is mostly a way to lose an argument with the
//! author. Re-wrapping paragraphs fights every deliberate line break, reflowing lists fights
//! the nesting they chose, and normalizing emphasis markers rewrites bytes that render
//! identically. So this one has exactly one job — the one every markdown editor has had
//! since Markdown All in One and the one an author cannot reasonably do by hand — and every
//! other line comes back **byte-identical**.
//!
//! That is not a stylistic promise, it is the shape of the answer: the edits returned cover
//! only the line ranges of tables that actually change. A line outside one cannot be touched
//! because no edit ever names it.
//!
//! **Where it declines.** A table whose body row has MORE cells than its header is left
//! exactly as written. GFM ignores the extras when rendering, so "formatting" it would mean
//! either deleting the author's text or widening the delimiter row — and widening the
//! delimiter row past the header count stops it being a table at all. A malformed table is
//! the last place a formatter should be confident.

use crate::lsp_cells::code_line_mask;

/// A formatted table: replace `start_line..=end_line` (0-based, inclusive) with `new_text`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TableEdit {
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
    pub(crate) new_text: String,
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
pub(crate) fn format_tables(text: &str) -> Vec<TableEdit> {
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
fn table_at(lines: &[&str], code: &[bool], start: usize) -> Option<TableEdit> {
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
    Some(TableEdit {
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

    /// **Formatting must not move any line.** `notes/backlog.md` files `.tmd` format-on-save as
    /// an open question precisely because a source pretty-printer that reflows text changes
    /// which line a block starts on, and every `data-sourcepos` — so click-to-source, the
    /// block diff and live-state preservation all key off something that just moved.
    ///
    /// A table formatter is exempt by construction rather than by luck: a table's rows map
    /// one-to-one onto its lines, so the replacement has exactly the line count of the range
    /// it replaces. That is a property worth pinning, not assuming, because a future "collapse
    /// a blank row" or "split a long cell" would quietly break it.
    #[test]
    fn formatting_never_changes_the_line_count() {
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
}
