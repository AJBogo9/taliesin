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
                    // `yield* EXPR` (generator delegation) is a DIFFERENT construct from
                    // `yield EXPR`: the grammar treats a `*` right after `yield` (mod
                    // inline whitespace) as the delegation marker, never as a value to
                    // stamp. Recording a site here used to land ON the `*` (its own
                    // start-of-expression heuristic sees `*` as "not whitespace, not `;`,
                    // not a newline" and stamps it), producing `yield__at(N, * expr)`: the
                    // splice below inserts `__at(` flush against `yield` with no space
                    // between them (there was none in the source, `yield*`), merging the
                    // two into the single identifier `yield__at` and leaving a bare `*` as
                    // the first thing inside its own argument list: a guaranteed syntax
                    // error, exactly the "a stamp it should not have emitted" case this
                    // scanner exists to rule out. Refuse the site instead: a missed cursor
                    // costs a line highlight, never a corrupted cell.
                    if j < b.len() && b[j] != b'*' && b[j] != b'\n' && b[j] != b';' {
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

    // --- adversarial cases (fix round 1 review) --------------------------------------
    // Five cases a reviewer asked to be run and pinned. None of them found a stamp the
    // scanner should not have made: a MISS costs a cursor position (acceptable, the
    // documented contract), a wrong stamp would corrupt a cell (never acceptable). All
    // five come back either correctly stamped once or a clean refusal.

    /// A `yield` inside a NESTED expression inside a template literal's `${...}` is
    /// inside `S::Template` the whole time (the scanner does not recurse into `${...}`
    /// as a second code region): a MISS, not a wrong stamp. Safe by construction, since
    /// Template-state bytes are never checked against the `yield` keyword at all.
    #[test]
    fn adversarial_yield_nested_inside_a_template_expression_is_missed_not_corrupted() {
        let src = "const s = `x ${ (function*(){ yield 1; })() } y`;\n";
        let out = stamp_yields(src).expect("the outer template closes, so this scans");
        assert_eq!(out, src, "nothing stamped, nothing rewritten");
    }

    /// An escaped quote immediately before a real `yield` must not fool the string
    /// scanner into closing early (which would leave the rest of the line, including
    /// the real string close and the `yield`, misparsed).
    #[test]
    fn adversarial_escaped_quote_before_a_real_yield_still_stamps_it_once() {
        let src = "const s = 'it\\'s'; yield v;\n";
        let out = stamp_yields(src).expect("must scan");
        assert_eq!(out.matches("__at(").count(), 1, "{out}");
        assert!(out.contains("yield __at(1, v);"), "{out}");
    }

    /// A regex containing quote characters must not make the scanner think it is
    /// inside a string (which would then swallow the rest of the line looking for a
    /// closing quote that is not there, and could refuse or misplace the next yield).
    #[test]
    fn adversarial_regex_containing_quote_characters_still_stamps_the_real_yield_once() {
        let src = "const r = /['\"]/g;\nyield v;\n";
        let out = stamp_yields(src).expect("must scan");
        assert_eq!(out.matches("__at(").count(), 1, "{out}");
        assert!(out.contains("yield __at(2, v);"), "{out}");
    }

    /// Division must not be mistaken for a regex open (the previous significant byte
    /// is `a`, an identifier, not an operator), so the following `yield` on the next
    /// line is reached in `S::Code` and stamped normally.
    #[test]
    fn adversarial_division_is_not_mistaken_for_a_regex_open() {
        let src = "const q = a / b;\nyield q;\n";
        let out = stamp_yields(src).expect("must scan");
        assert_eq!(out.matches("__at(").count(), 1, "{out}");
        assert!(out.contains("yield __at(2, q);"), "{out}");
    }

    /// An unterminated template literal refuses, the same as an unterminated block
    /// comment: `S::Template` survives a raw newline (real multi-line templates are
    /// legal JS), so only running out of input while still inside one triggers the
    /// final unterminated check.
    #[test]
    fn adversarial_unterminated_template_literal_refuses() {
        assert!(stamp_yields("const s = `oops\nyield v;\n").is_none());
    }

    // --- yield* delegation (fix round 2: a real CRITICAL, not a hypothetical) ---------
    // Regression pins for a stamp the scanner used to emit ON the `*` of a `yield*`
    // delegation: `is_ident(b'*')` is false, so the keyword match accepted `yield*`, and
    // the site-recording guard only rejected a newline or `;`, so it recorded a site at
    // the `*` itself. Splicing `__at(` there merges flush against `yield` (there was no
    // space to preserve, the source read `yield*`) into the single identifier
    // `yield__at`, with a bare `*` as the first token in what was meant to be its
    // argument list: a guaranteed `SyntaxError`. `yield*` is the idiomatic delegation
    // form for exactly the recursive algorithms (quicksort, mergesort, tree traversals)
    // this feature targets, so this is not an edge case.

    /// The textbook case: `yield* recurse(...)`, no space before the `*`.
    #[test]
    fn a_yield_star_delegation_with_no_space_is_left_alone() {
        let src = "function* sort(a, lo, mid) {\n  yield* sort(a, lo, mid);\n}\n";
        let out = stamp_yields(src).expect("must scan");
        assert!(!out.contains("__at("), "nothing to stamp here:\n{out}");
        assert_eq!(out, src, "yield* stays byte-identical");
        // The specific corruption a reviewer measured on a real build: `yield` and
        // `__at` must never merge into one identifier.
        assert!(!out.contains("yield__at"), "{out}");
    }

    /// The same delegation with whitespace between `yield` and `*` (`yield *
    /// recurse(...)`): the grammar still parses this as delegation (a `*` right after
    /// `yield`, mod inline whitespace, is never ordinary multiplication of a bodyless
    /// yield), so the scanner must reach the same "do not stamp" conclusion by finding
    /// the `*` only after skipping the inline whitespace.
    #[test]
    fn a_yield_star_delegation_with_a_space_before_the_star_is_left_alone() {
        let src = "function* f(a) {\n  yield * recurse(a);\n}\n";
        let out = stamp_yields(src).expect("must scan");
        assert!(!out.contains("__at("), "nothing to stamp here:\n{out}");
        assert_eq!(out, src, "yield * stays byte-identical");
    }

    /// A `yield*` delegation followed, on a LATER line, by an ordinary `yield` must not
    /// have its own refusal leak forward: only the delegation site is skipped, the plain
    /// `yield` after it is still stamped normally.
    #[test]
    fn a_plain_yield_after_a_yield_star_is_still_stamped() {
        let src = "function* f(a) {\n  yield* recurse(a);\n  yield a;\n}\n";
        let out = stamp_yields(src).expect("must scan");
        assert_eq!(out.matches("__at(").count(), 1, "{out}");
        assert!(out.contains("yield __at(3, a);"), "{out}");
        assert!(!out.contains("yield*__at("), "{out}");
    }
}
