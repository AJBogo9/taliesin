//! Position-encoding conversion at the LSP boundary.
//!
//! The `lsp` server works internally in Unicode **scalar** (`char`) offsets (so does
//! `lsp_nav`, `lsp_outline`, and the `check` diagnostics' `to_lsp`). The LSP wire, by
//! default and as the VS Code companion uses it, counts columns in **UTF-16 code units**.
//! Scalar and UTF-16 agree across the whole BMP (accents, CJK, Cyrillic, Arabic), so the
//! two conventions only diverge on astral characters (emoji, mathematical letters), which
//! occupy two UTF-16 code units but one scalar. These two helpers convert per line at the
//! server boundary: UTF-16 in on incoming positions, UTF-16 out on emitted `Position`s.

/// The lines of `text`, ended the way **CommonMark** ends a line — at `\r\n`, at `\n`, or at a
/// lone `\r` — with the terminator excluded. The one line model the whole LSP counts in.
///
/// It has to be this one because the line number in a diagnostic is comrak's, and comrak
/// follows CommonMark. `text.split('\n')` was the instrument at seven sites until 2026-08-13,
/// and it agrees with comrak on every buffer with no lone `\r` and disagrees on every buffer
/// that has one — pasted terminal output is the realistic source. One stray CR desynced the
/// two indexes for the rest of the file: F12 landed on the wrong line, hover answered about a
/// neighbour, and a whole-line squiggle whose line number ran past the shorter `\n`-split
/// clamped to the last line and collapsed to zero width. The client agrees with CommonMark
/// too (VS Code's text model and `vscode-languageserver-textdocument` both end a line at a
/// lone `\r`), so this is the model all three sides already shared and only we did not.
///
/// Trailing behaviour is `str::split`'s, **not** `str::lines`': a buffer ending in a
/// terminator yields a final empty line. That line is one the editor's cursor can sit on, and
/// every `last`/`line_count` clamp in the server is written against its being there.
pub(crate) fn lines(text: &str) -> Lines<'_> {
    Lines { rest: Some(text) }
}

/// The iterator [`lines`] returns.
pub(crate) struct Lines<'a> {
    /// The text after the last terminator consumed, or `None` once the final line was yielded.
    rest: Option<&'a str>,
}

impl<'a> Iterator for Lines<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let rest = self.rest?;
        match rest.find(['\n', '\r']) {
            Some(i) => {
                // CRLF is one terminator, so it must not yield an empty line between the two.
                let skip = if rest[i..].starts_with("\r\n") { 2 } else { 1 };
                self.rest = Some(&rest[i + skip..]);
                Some(&rest[..i])
            }
            None => {
                self.rest = None;
                Some(rest)
            }
        }
    }
}

/// The 0-based line `line` of `text` (see [`lines`]), or `""` past the end.
pub(crate) fn nth_line(text: &str, line: usize) -> &str {
    lines(text).nth(line).unwrap_or("")
}

/// One line minus the `\r` of a CRLF terminator, for a line obtained without [`lines`] (which
/// excludes it already).
///
/// An editor treats CRLF as one terminator, so a column the client sends never counts the
/// `\r` and a column we emit must not either; leaving it on made every end-of-line position
/// in a CRLF buffer one column too long.
pub(crate) fn line_content(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

/// The end-of-line column of `line`, in UTF-16 code units — what an LSP `Range` that spans a
/// whole line ends at. CRLF-aware via [`line_content`], which is the whole reason it is a
/// named function rather than an inline `.chars().map(char::len_utf16).sum()` in each of the
/// four places that wanted it.
pub(crate) fn line_end_utf16(line: &str) -> usize {
    line_content(line).chars().map(char::len_utf16).sum()
}

/// Convert a UTF-16 code-unit offset into `line` to a scalar (`char`) offset. An offset past
/// the line's end clamps to the char length; an offset that lands inside an astral char's
/// surrogate pair rounds up to the next char boundary.
pub(crate) fn utf16_to_char(line: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (char_idx, ch) in line.chars().enumerate() {
        if units >= utf16 {
            return char_idx;
        }
        units += ch.len_utf16();
    }
    line.chars().count()
}

/// Convert a scalar (`char`) offset into `line` to a UTF-16 code-unit offset. An offset past
/// the line's end clamps to the line's UTF-16 length.
pub(crate) fn char_to_utf16(line: &str, char_col: usize) -> usize {
    line.chars().take(char_col).map(char::len_utf16).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    // An astral character (2 UTF-16 units, 1 scalar) is where the two conventions diverge.
    const EMOJI: &str = "😀ab"; // 😀 = U+1F600, 2 UTF-16 units
    const MATH: &str = "x𝐀y"; // 𝐀 = U+1D400 MATHEMATICAL BOLD CAPITAL A, 2 UTF-16 units

    #[test]
    fn char_to_utf16_advances_two_units_per_astral_char() {
        // 😀(2) a(1) b(1): char boundaries land at UTF-16 offsets 0,2,3,4.
        assert_eq!(char_to_utf16(EMOJI, 0), 0);
        assert_eq!(char_to_utf16(EMOJI, 1), 2); // after the emoji
        assert_eq!(char_to_utf16(EMOJI, 2), 3); // after 'a'
        assert_eq!(char_to_utf16(EMOJI, 3), 4); // after 'b'
        // Astral in the middle: x(1) 𝐀(2) y(1) -> 0,1,3,4.
        assert_eq!(char_to_utf16(MATH, 1), 1); // before the math letter
        assert_eq!(char_to_utf16(MATH, 2), 3); // after the math letter
    }

    #[test]
    fn utf16_to_char_collapses_two_units_per_astral_char() {
        assert_eq!(utf16_to_char(EMOJI, 0), 0);
        assert_eq!(utf16_to_char(EMOJI, 2), 1); // start of 'a'
        assert_eq!(utf16_to_char(EMOJI, 3), 2); // start of 'b'
        assert_eq!(utf16_to_char(EMOJI, 4), 3); // end of line
        assert_eq!(utf16_to_char(MATH, 3), 2); // start of 'y'
    }

    #[test]
    fn a_utf16_offset_inside_a_surrogate_pair_rounds_to_a_char_boundary() {
        // Offset 1 sits between the two UTF-16 units of 😀; round up to char 1 ('a').
        assert_eq!(utf16_to_char(EMOJI, 1), 1);
    }

    #[test]
    fn bmp_text_is_an_identity_both_directions() {
        // Every BMP scalar is exactly one UTF-16 unit, so the conversion is a no-op:
        // this is why all realistic natural-language text already navigates correctly.
        let bmp = "héllo café 你好"; // accents + CJK, all BMP
        for c in 0..=bmp.chars().count() {
            assert_eq!(char_to_utf16(bmp, c), c, "char_to_utf16 identity at {c}");
            assert_eq!(utf16_to_char(bmp, c), c, "utf16_to_char identity at {c}");
        }
    }

    #[test]
    fn round_trips_through_both_conversions() {
        for line in [EMOJI, MATH, "plain", "你好😀world"] {
            for c in 0..=line.chars().count() {
                assert_eq!(
                    utf16_to_char(line, char_to_utf16(line, c)),
                    c,
                    "round-trip at {c} of {line:?}"
                );
            }
        }
    }

    #[test]
    fn nth_line_splits_on_newline() {
        assert_eq!(nth_line("a\n😀b\nc", 1), "😀b");
        assert_eq!(nth_line("a\nb", 9), "");
    }

    fn split(text: &str) -> Vec<&str> {
        lines(text).collect()
    }

    /// The defect this helper exists for: comrak ends a line at a lone `\r` and
    /// `text.split('\n')` does not, so one pasted CR desynced every later line index.
    #[test]
    fn a_lone_cr_ends_a_line() {
        assert_eq!(split("para one\rpara two"), ["para one", "para two"]);
        assert_eq!(split("a\r\rb"), ["a", "", "b"]);
        assert_eq!(nth_line("a\rb\rc", 2), "c");
    }

    #[test]
    fn crlf_is_one_terminator_and_is_not_part_of_the_line() {
        assert_eq!(split("a\r\nb\r\n"), ["a", "b", ""]);
        assert_eq!(nth_line("# H\r\n\r\ntext\r\n", 0), "# H");
        assert_eq!(line_end_utf16(nth_line("# H\r\n", 0)), 3);
    }

    /// Every buffer with no lone `\r` must index exactly as `split('\n')` did, including the
    /// empty line a trailing terminator leaves behind: the `last`/`line_count` clamps at the
    /// call sites are all written against that line being counted.
    #[test]
    fn without_a_lone_cr_it_is_the_old_newline_split_exactly() {
        for text in ["", "a", "a\n", "a\nb", "a\n\nb\n", "\n", "a\r\n"] {
            let old: Vec<&str> = text.split('\n').map(line_content).collect();
            assert_eq!(split(text), old, "line model changed for {text:?}");
        }
    }

    #[test]
    fn mixed_terminators_in_one_buffer_each_end_one_line() {
        assert_eq!(split("a\r\nb\nc\rd"), ["a", "b", "c", "d"]);
    }
}
