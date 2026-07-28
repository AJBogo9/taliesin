//! Position-encoding conversion at the LSP boundary.
//!
//! The `lsp` server works internally in Unicode **scalar** (`char`) offsets (so does
//! `lsp_nav`, `lsp_outline`, and the `check` diagnostics' `to_lsp`). The LSP wire, by
//! default and as the VS Code companion uses it, counts columns in **UTF-16 code units**.
//! Scalar and UTF-16 agree across the whole BMP (accents, CJK, Cyrillic, Arabic), so the
//! two conventions only diverge on astral characters (emoji, mathematical letters), which
//! occupy two UTF-16 code units but one scalar. These two helpers convert per line at the
//! server boundary: UTF-16 in on incoming positions, UTF-16 out on emitted `Position`s.

/// The 0-based line `line` (a `\n`-split buffer, 0-based) of `text`, or `""` past the end.
/// The server splits on `\n` everywhere positions are computed (`lsp_nav::classify_target`,
/// completion, `to_lsp`), so per-line conversion must split the same way.
///
/// A trailing `\r` is **not** part of the line. An editor treats CRLF as one terminator, so
/// a column the client sends never counts the `\r` and a column we emit must not either;
/// leaving it on made every end-of-line position in a CRLF buffer one column too long.
pub(crate) fn nth_line(text: &str, line: usize) -> &str {
    line_content(text.split('\n').nth(line).unwrap_or(""))
}

/// One line of a `\n`-split buffer, minus the `\r` of a CRLF terminator. See [`nth_line`].
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
}
