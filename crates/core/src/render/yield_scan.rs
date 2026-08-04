//! Map each `yield` in a `{js}` cell to its source line, so the debugger's cursor can
//! point at the statement that produced the current frame.
//!
//! Deliberately a SCANNER, not a parser: pulling a JavaScript parser into the crate to
//! learn one line number per `yield` is not a trade this project makes. The scanner's
//! contract is therefore conservative in one direction only, and that asymmetry is the
//! whole safety design: a `yield` it fails to recognise costs a cursor position, while a
//! `yield` it invents inside a string would corrupt the author's cell. So it refuses.

/// Lexer states. Regex literals matter only because one may contain the word `yield`.
#[derive(PartialEq)]
enum S {
    Code,
    Single,
    Double,
    Template,
    LineComment,
    BlockComment,
    Regex,
}

/// Rewrite `yield EXPR` into `yield __at(LINE, EXPR)`. Returns `None` when the scan
/// cannot complete (an unterminated literal or comment), in which case the caller ships
/// the cell unmodified and the cursor stays parked.
pub(crate) fn stamp_yields(src: &str) -> Option<String> {
    let b = src.as_bytes();
    let (mut st, mut line, mut i) = (S::Code, 1usize, 0usize);
    // (byte offset of the yielded expression's first byte, line): recorded AFTER
    // skipping the whitespace that follows the `yield` keyword, so the splice below
    // inserts `__at(LINE, ` right against the expression rather than leaving a stray
    // double space where the keyword's own trailing space used to be.
    let mut sites: Vec<(usize, usize)> = Vec::new();
    let mut prev_sig = 0u8; // last significant code byte, for the regex-vs-divide call

    while i < b.len() {
        if b[i] == b'\n' {
            line += 1;
            if st == S::LineComment {
                st = S::Code;
            }
            // An unterminated single/double quoted string cannot span a raw newline.
            // A template literal legitimately can (JS allows multi-line templates), and
            // a block comment can too, so only these two states bail here.
            if st == S::Single || st == S::Double {
                return None;
            }
            i += 1;
            continue;
        }
        match st {
            S::Code => {
                // Enter a literal or comment.
                if b[i] == b'\'' {
                    st = S::Single;
                } else if b[i] == b'"' {
                    st = S::Double;
                } else if b[i] == b'`' {
                    st = S::Template;
                } else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
                    st = S::LineComment;
                } else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
                    st = S::BlockComment;
                } else if b[i] == b'/'
                    && matches!(
                        prev_sig,
                        0 | b'='
                            | b'('
                            | b','
                            | b':'
                            | b'['
                            | b'!'
                            | b'&'
                            | b'|'
                            | b'?'
                            | b'{'
                            | b'}'
                            | b';'
                    )
                {
                    st = S::Regex;
                } else if b[i..].starts_with(b"yield")
                    && !is_ident(if i == 0 { 0 } else { b[i - 1] })
                    && !is_ident(*b.get(i + 5).unwrap_or(&0))
                {
                    // The keyword matched; find where the yielded expression actually
                    // starts by skipping inline whitespace (not a newline: `yield` then
                    // a bare newline is a valueless yield under JS's ASI rules, so there
                    // is nothing to stamp). A bare `yield;` is the same case by a
                    // different route. Either way, no site is recorded rather than
                    // splicing `__at(` in front of nothing.
                    let mut j = i + 5;
                    while j < b.len() && matches!(b[j], b' ' | b'\t') {
                        j += 1;
                    }
                    if j < b.len() && b[j] != b'\n' && b[j] != b';' {
                        sites.push((j, line));
                    }
                    i += 5;
                    prev_sig = b'd';
                    continue;
                }
                if !b[i].is_ascii_whitespace() {
                    prev_sig = b[i];
                }
            }
            S::Single if b[i] == b'\'' => st = S::Code,
            S::Double if b[i] == b'"' => st = S::Code,
            S::Template if b[i] == b'`' => st = S::Code,
            S::Regex if b[i] == b'/' => st = S::Code,
            S::BlockComment if b[i] == b'*' && b.get(i + 1) == Some(&b'/') => {
                st = S::Code;
                i += 1;
            }
            _ => {}
        }
        // A backslash escapes the next byte inside any literal.
        if matches!(st, S::Single | S::Double | S::Template | S::Regex) && b[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    if st != S::Code && st != S::LineComment {
        return None; // unterminated: refuse rather than guess
    }
    if sites.is_empty() {
        return Some(src.to_string());
    }
    Some(splice(src, &sites))
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// Insert `__at(LINE, ` at each recorded expression start and its matching `)` at the
/// expression's end. Sites are visited in reverse (highest offset first): each site's
/// own insertions only ever touch byte offsets at or after that site's start, and every
/// still-pending site sits strictly before it, so earlier offsets never shift out from
/// under an insertion that hasn't happened yet.
fn splice(src: &str, sites: &[(usize, usize)]) -> String {
    let b = src.as_bytes();
    let mut out = src.to_string();
    for &(start, line) in sites.iter().rev() {
        // A site whose expression end can't be found at bracket depth zero is skipped
        // on its own (a partial cursor beats no adapter), and every OTHER site still
        // gets stamped.
        if let Some(end) = expr_end(b, start) {
            out.insert(end, ')');
            out.insert_str(start, &format!("__at({line}, "));
        }
    }
    out
}

/// The byte offset just past a yielded expression: scans forward from `start` at the
/// same bracket depth for the first `;` or newline, staying alert to a nested string/
/// template so a terminator-looking byte inside one of those (`yield "a;b";`) is not
/// mistaken for the expression's end. `None` when the brackets never return to depth
/// zero or a string/template never closes before EOF.
fn expr_end(b: &[u8], start: usize) -> Option<usize> {
    #[derive(PartialEq)]
    enum Q {
        Code,
        Single,
        Double,
        Template,
    }
    let mut st = Q::Code;
    let mut depth = 0i32;
    let mut i = start;
    while i < b.len() {
        match st {
            Q::Code => match b[i] {
                b'\'' => st = Q::Single,
                b'"' => st = Q::Double,
                b'`' => st = Q::Template,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' if depth > 0 => depth -= 1,
                b';' | b'\n' if depth == 0 => return Some(i),
                _ => {}
            },
            Q::Single if b[i] == b'\'' => st = Q::Code,
            Q::Double if b[i] == b'"' => st = Q::Code,
            Q::Template if b[i] == b'`' => st = Q::Code,
            _ => {}
        }
        if matches!(st, Q::Single | Q::Double | Q::Template) && b[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_each_yield_with_its_source_line() {
        let src = "function* f(a) {\n  yield a;\n  a.push(1);\n  yield a;\n}\n";
        let out = stamp_yields(src).expect("plain source must scan");
        assert!(out.contains("yield __at(2, a);"), "{out}");
        assert!(out.contains("yield __at(4, a);"), "{out}");
    }

    #[test]
    fn leaves_yield_alone_inside_strings_templates_and_comments() {
        let src =
            "const s = 'yield x';\n// yield x\n/* yield x */\nconst t = `yield ${x}`;\nyield v;\n";
        let out = stamp_yields(src).expect("must scan");
        assert_eq!(
            out.matches("__at(").count(),
            1,
            "only the real yield is stamped:\n{out}"
        );
        assert!(out.contains("yield __at(5, v);"), "{out}");
    }

    /// A scan it cannot finish must REFUSE, not guess. A mis-stamp inside a string would
    /// corrupt the author's cell; no stamp only costs the line cursor.
    #[test]
    fn refuses_rather_than_guessing_on_an_unterminated_literal() {
        assert!(stamp_yields("const s = 'unterminated\nyield v;\n").is_none());
        assert!(stamp_yields("/* unterminated\nyield v;\n").is_none());
    }

    #[test]
    fn a_regex_literal_containing_the_word_yield_is_not_stamped() {
        let src = "const r = /yield/g;\nyield v;\n";
        let out = stamp_yields(src).expect("must scan");
        assert_eq!(out.matches("__at(").count(), 1, "{out}");
    }

    /// A bare `yield;` (or `yield` immediately followed by a newline, the same
    /// statement under ASI) has no expression to wrap; stamping it would splice
    /// `__at(` in front of nothing and break the statement. Regression for the
    /// whitespace-skip in the keyword match: without it, the naive site offset would
    /// sit on the space right after `yield`, one byte before the value.
    #[test]
    fn a_bare_yield_with_no_value_is_left_alone() {
        let src = "function* f() {\n  yield;\n  yield\n  x();\n}\n";
        let out = stamp_yields(src).expect("must scan");
        assert!(!out.contains("__at("), "nothing to stamp here:\n{out}");
        assert_eq!(out, src, "a bodyless yield stays byte-identical");
    }

    /// The splice must not leave a double space or a missing one: the keyword's own
    /// trailing space is preserved and `__at(` sits directly against the expression.
    #[test]
    fn the_splice_keeps_exactly_one_space_after_the_keyword() {
        let out = stamp_yields("yield   a;\n").expect("must scan");
        assert_eq!(out, "yield   __at(1, a);\n");
    }
}
