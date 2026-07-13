//! Conservative, dependency-free minifiers for the build-time shared asset bundle. Both are
//! fully UTF-8 correct: they iterate by `char` and copy non-ASCII content (e.g. `·`, `×`, `…`,
//! `→`, `é`) through untouched, inside strings and out. CSS collapses whitespace + strips
//! comments (string/`url()`-aware). JS runs a SINGLE stateful pass over the whole source (not
//! per line), so cross-line constructs (multi-line template literals, multi-line block
//! comments) are handled correctly; it strips comments and blank-line indentation but
//! PRESERVES every statement newline (ASI-safe) and never mangles tokens. It runs only on
//! Taliesin's own hand-written JS (vendored `*.min.js` bypass it entirely).

pub fn minify_css(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut last_was_space = false;
    while i < n {
        let c = chars[i];
        // string literal: copy verbatim (quotes, escapes, and any non-ASCII content)
        if c == '"' || c == '\'' {
            let q = c;
            out.push(q);
            i += 1;
            while i < n {
                let d = chars[i];
                out.push(d);
                if d == '\\' && i + 1 < n {
                    out.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                if d == q {
                    i += 1;
                    break;
                }
                i += 1;
            }
            last_was_space = false;
            continue;
        }
        // url(...) : copy verbatim through the matching ')'. NOTE: this stops at the FIRST
        // ')'. Safe today because our CSS urls are base64 `data:` URIs with no ')'; a future
        // quoted `url("...)...")` would need revisiting.
        if chars[i..].starts_with(&['u', 'r', 'l', '(']) {
            let mut j = i;
            while j < n && chars[j] != ')' {
                j += 1;
            }
            if j < n {
                j += 1; // include the ')'
            }
            for &ch in &chars[i..j] {
                out.push(ch);
            }
            i = j;
            last_was_space = false;
            continue;
        }
        // block comment: drop it entirely
        if chars[i..].starts_with(&['/', '*']) {
            let mut j = i + 2;
            while j + 1 < n && !(chars[j] == '*' && chars[j + 1] == '/') {
                j += 1;
            }
            i = if j + 1 < n { j + 2 } else { n };
            continue;
        }
        if c.is_ascii_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
            i += 1;
            continue;
        }
        out.push(c);
        last_was_space = false;
        i += 1;
    }
    out.trim().to_string()
}

/// State of the single-pass JS scanner.
#[derive(PartialEq, Clone, Copy)]
enum Js {
    Normal,
    LineComment,  // `//` ... end of line
    BlockComment, // `/* ... */`
    Single,       // '...'
    Double,       // "..."
    Template,     // `...`
    Regex,        // /.../  (regex literal)
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// Decide whether a `/` (not `//` or `/*`) begins a regex literal rather than division.
/// Conservative but correct for our own JS: a regex starts only when the previous
/// significant (non-whitespace, non-comment) token cannot end an expression, i.e. it is
/// start-of-input, a newline boundary, the `return` keyword, or one of the listed punctuators.
fn regex_context(prev: Option<char>, last_word: &str, newline_boundary: bool) -> bool {
    if prev.is_none() || newline_boundary || last_word == "return" {
        return true;
    }
    matches!(
        prev,
        Some('(' | ',' | '=' | ':' | '[' | '!' | '&' | '|' | '?' | '{' | '}' | ';')
    )
}

/// Pop trailing ASCII spaces/tabs/CR from the current output line (never below `line_start`).
fn trim_trailing_ws(out: &mut String, line_start: usize) {
    while out.len() > line_start {
        match out.as_bytes()[out.len() - 1] {
            b' ' | b'\t' | b'\r' => {
                out.pop();
            }
            _ => break,
        }
    }
}

pub fn minify_js(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut state = Js::Normal;
    // Index into `out` where the current output line begins (for blank-line dropping and
    // trailing-whitespace trimming). Advanced past every '\n' pushed, in any state, so a
    // multi-line string/template's interior whitespace is never trimmed as if it were code.
    let mut line_start = 0usize;
    // Regex-context detection: the previous significant (non-ws, non-comment) token.
    let mut prev_significant: Option<char> = None;
    let mut last_word = String::new(); // trailing run of identifier chars, to spot `return`
    let mut in_word = false; // currently extending `last_word` (reset by any boundary)
    let mut newline_boundary = true; // start-of-input counts as a newline boundary
    let mut regex_class = false; // inside a `[...]` char class of a regex literal

    while i < n {
        let c = chars[i];
        match state {
            Js::Normal => {
                if c == '\n' {
                    // Statement newline (ASI-critical): drop trailing indentation, then keep
                    // the newline only when the line has real content (drop blank lines).
                    trim_trailing_ws(&mut out, line_start);
                    if out.len() > line_start {
                        out.push('\n');
                        line_start = out.len();
                    }
                    newline_boundary = true;
                    in_word = false;
                    i += 1;
                    continue;
                }
                if c == ' ' || c == '\t' || c == '\r' {
                    // Strip leading indentation; keep interior spaces verbatim (a later
                    // newline trims any trailing run). Whitespace ends a word but does not
                    // clear it, so `return /re/` still sees the `return` keyword.
                    if out.len() > line_start {
                        out.push(c);
                    }
                    in_word = false;
                    i += 1;
                    continue;
                }
                if c == '/' && i + 1 < n && chars[i + 1] == '/' {
                    state = Js::LineComment;
                    i += 2;
                    continue;
                }
                if c == '/' && i + 1 < n && chars[i + 1] == '*' {
                    state = Js::BlockComment;
                    i += 2;
                    continue;
                }
                if c == '"' || c == '\'' || c == '`' {
                    out.push(c);
                    state = match c {
                        '"' => Js::Double,
                        '\'' => Js::Single,
                        _ => Js::Template,
                    };
                    i += 1;
                    continue;
                }
                if c == '/' {
                    out.push(c);
                    if regex_context(prev_significant, &last_word, newline_boundary) {
                        state = Js::Regex;
                        regex_class = false;
                    } else {
                        // division: an ordinary token that can end an expression
                        prev_significant = Some('/');
                        last_word.clear();
                        newline_boundary = false;
                        in_word = false;
                    }
                    i += 1;
                    continue;
                }
                // ordinary significant char
                out.push(c);
                prev_significant = Some(c);
                newline_boundary = false;
                if is_ident_char(c) {
                    if !in_word {
                        last_word.clear();
                        in_word = true;
                    }
                    last_word.push(c);
                } else {
                    last_word.clear();
                    in_word = false;
                }
                i += 1;
            }
            Js::LineComment => {
                // Consume to (but not including) the newline; the Normal arm handles the '\n'
                // so the statement newline and blank-line logic stay in one place.
                if c == '\n' {
                    state = Js::Normal;
                    continue;
                }
                i += 1;
            }
            Js::BlockComment => {
                if c == '*' && i + 1 < n && chars[i + 1] == '/' {
                    // Replace the whole comment (even multi-line) with a single space so
                    // adjacent tokens never fuse (`return/*x*/5` -> `return 5`). Skip the
                    // space when it would only add leading/duplicate whitespace.
                    // GUARD: collapsing a MULTI-LINE `/* ... */` to one space also drops the
                    // newline(s) it spanned, which could change meaning where a newline was
                    // ASI-significant (e.g. a `return` on its own line before the comment). This
                    // is currently unreachable because no JS fed to `core_enhance_js()` has a
                    // multi-line block comment; a guard note so that stays a deliberate choice.
                    let need_space = match out.as_bytes().last() {
                        None => false,
                        Some(&b) => b != b' ' && b != b'\t' && b != b'\n',
                    };
                    if need_space {
                        out.push(' ');
                    }
                    state = Js::Normal;
                    i += 2;
                    continue;
                }
                i += 1;
            }
            Js::Single | Js::Double | Js::Template => {
                let quote = match state {
                    Js::Single => '\'',
                    Js::Double => '"',
                    _ => '`',
                };
                if c == '\\' && i + 1 < n {
                    // Escape: copy the backslash and the next char verbatim (handles \" \` and
                    // a `\`-escaped line continuation).
                    out.push(c);
                    let e = chars[i + 1];
                    out.push(e);
                    if e == '\n' {
                        line_start = out.len();
                    }
                    i += 2;
                    continue;
                }
                out.push(c);
                if c == '\n' {
                    // Interior newline (only reachable in a template): significant, copied
                    // verbatim; advance line_start so its content is never trimmed.
                    line_start = out.len();
                } else if c == quote {
                    state = Js::Normal;
                    prev_significant = Some(quote);
                    last_word.clear();
                    in_word = false;
                    newline_boundary = false;
                }
                i += 1;
            }
            Js::Regex => {
                if c == '\\' && i + 1 < n {
                    out.push(c);
                    out.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                out.push(c);
                match c {
                    '[' => regex_class = true,
                    ']' => regex_class = false,
                    '/' if !regex_class => {
                        state = Js::Normal;
                        prev_significant = Some('/');
                        last_word.clear();
                        in_word = false;
                        newline_boundary = false;
                    }
                    '\n' => line_start = out.len(),
                    _ => {}
                }
                i += 1;
            }
        }
    }
    // Flush a final line lacking a trailing newline (mirrors the old per-line loop, which
    // emitted one '\n' per surviving line).
    if state == Js::Normal {
        trim_trailing_ws(&mut out, line_start);
        if out.len() > line_start {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_strips_comments_and_collapses_space_but_spares_strings() {
        let out = minify_css("/* c */ a  {  color : red ;  }\n.b{content:\"  keep  \"}");
        assert!(!out.contains("/*"), "comment stripped");
        // Whitespace runs collapse everywhere OUTSIDE string literals; the string's
        // own internal spacing (checked below) is deliberately opaque, so it is
        // excluded here rather than asserting a whole-output invariant that the
        // preserved string content would trivially violate.
        let (before_string, _) = out.split_once('"').expect("string literal present");
        assert!(
            !before_string.contains("  "),
            "runs of space collapsed outside strings"
        );
        assert!(out.contains("\"  keep  \""), "string content preserved");
    }

    #[test]
    fn css_preserves_data_uri_in_url() {
        let src = ".x{background:url(data:image/png;base64,AA  BB)}";
        // Whitespace inside url(...) with a data: URI must not be mangled.
        assert!(minify_css(src).contains("data:image/png;base64,AA  BB"));
    }

    #[test]
    fn js_preserves_newlines_for_asi_and_strips_comments() {
        let src = "let a = 1 // trailing\n/* block */\nlet b = 2\n";
        let out = minify_js(src);
        assert!(!out.contains("//"), "line comment stripped");
        assert!(!out.contains("/*"), "block comment stripped");
        // Newlines between statements are preserved (ASI safety).
        assert!(out.matches('\n').count() >= 1, "statement newline kept");
    }

    #[test]
    fn js_does_not_strip_comment_markers_inside_strings_or_regex() {
        assert!(minify_js("let u = \"http://x\"\n").contains("http://x"));
        assert!(minify_js("let re = /a\\/\\/b/\n").contains("/a\\/\\/b/"));
    }

    // Extra edge cases beyond the brief's four pinned tests.

    #[test]
    fn css_empty_input_is_empty_output() {
        assert_eq!(minify_css(""), "");
    }

    #[test]
    fn js_empty_input_is_empty_output() {
        assert_eq!(minify_js(""), "");
    }

    #[test]
    fn css_comment_adjacent_to_selector_does_not_leave_stray_space() {
        // A comment butted right up against selector tokens (no surrounding
        // whitespace) must not introduce a space when it is dropped.
        let out = minify_css(".a/* c */.b{color:red}");
        assert!(!out.contains("/*"), "comment stripped");
        assert!(
            out.contains(".a.b{color:red}"),
            "no stray space where comment was: {out}"
        );
    }

    #[test]
    fn js_template_literal_with_interpolation_is_left_intact() {
        // `${...}` inside a backtick string, including a `//`-looking sequence
        // inside the template body, must survive as opaque string content.
        let src = "let s = `a${b}//c`\n";
        let out = minify_js(src);
        assert!(
            out.contains("`a${b}//c`"),
            "template literal preserved verbatim: {out}"
        );
    }

    #[test]
    fn js_division_not_mistaken_for_comment() {
        // A single `/` (division) adjacent to another token must not be
        // treated as the start of a comment.
        let out = minify_js("let x = a / b\n");
        assert!(out.contains("a / b"), "division preserved: {out}");
    }

    // --- Finding 1: UTF-8 safety (no `byte as char` mojibake) ---

    #[test]
    fn css_preserves_non_ascii_bytes_exactly() {
        // The exact multi-byte run must survive byte-for-byte inside a content:"..." string.
        let run = "· × … → ↵ é";
        let out = minify_css(&format!(".x::after{{content:\"{run}\"}}"));
        assert!(out.contains(run), "non-ASCII CSS content mangled: {out:?}");
        // No Latin-1 mojibake artifact (the old `byte as char` bug produced e.g. "Â·").
        assert!(!out.contains('Â'), "mojibake leaked into CSS: {out:?}");
    }

    #[test]
    fn js_preserves_non_ascii_bytes_exactly() {
        // Non-ASCII inside a JS string AND immediately before a `//` line comment that itself
        // contains non-ASCII: the string run must survive byte-for-byte, the comment must go.
        let run = "a · b × c … d → e ↵ f é";
        let src = format!("var s = '{run}' // drop this · × …\nvar t = \"→\"\n");
        let out = minify_js(&src);
        assert!(
            out.contains(&format!("'{run}'")),
            "string run mangled: {out:?}"
        );
        assert!(out.contains("\"→\""), "second string mangled: {out:?}");
        assert!(
            !out.contains("//"),
            "line comment (with non-ASCII) not stripped: {out:?}"
        );
        assert!(!out.contains("drop this"), "comment body survived: {out:?}");
        assert!(!out.contains('Â'), "mojibake leaked into JS: {out:?}");
    }

    // --- Finding 2: single stateful cross-line pass ---

    #[test]
    fn js_multiline_template_continuation_with_double_slash_survives() {
        // A template literal spanning physical lines whose CONTINUATION line contains `//`
        // must keep that content: the old per-line stripper reset state each line and ate it.
        let src = "var t = `line1 http://ok\ncontinued // not a comment\nend`\nvar x = 1\n";
        let out = minify_js(src);
        assert!(
            out.contains("continued // not a comment"),
            "template continuation with // lost: {out:?}"
        );
        assert!(
            out.contains("line1 http://ok"),
            "template line 1 lost: {out:?}"
        );
        assert!(out.contains("end`"), "template close lost: {out:?}");
        assert!(
            out.contains("var x = 1"),
            "trailing statement lost: {out:?}"
        );
    }

    #[test]
    fn js_multiline_block_comment_with_double_slash_fully_removed() {
        // A `/* ... */` spanning lines whose interior has `//` must be removed whole, leaving
        // a balanced result: no stray `/*` (the old per-line code swallowed the closing `*/`).
        let src = "before\n/* multi\n// interior line\ncomment */after\ndone\n";
        let out = minify_js(src);
        assert!(!out.contains("/*"), "stray /* left: {out:?}");
        assert!(!out.contains("*/"), "stray */ left: {out:?}");
        assert!(!out.contains("interior"), "comment body survived: {out:?}");
        assert!(
            out.contains("before"),
            "content before comment lost: {out:?}"
        );
        assert!(out.contains("after"), "content after comment lost: {out:?}");
        assert!(out.contains("done"), "content after comment lost: {out:?}");
    }

    #[test]
    fn js_block_comment_replaced_with_space_avoids_token_fusion() {
        // `return/*c*/x` must become `return x`, never `returnx`.
        let out = minify_js("return/*c*/x\n");
        assert!(
            out.contains("return x"),
            "tokens fused or space missing: {out:?}"
        );
        assert!(!out.contains("returnx"), "tokens fused: {out:?}");
    }

    #[test]
    fn js_regex_vs_division_disambiguation() {
        // A regex literal containing `//` is preserved verbatim...
        let out1 = minify_js("var re = /a\\/\\/b/\n");
        assert!(
            out1.contains("/a\\/\\/b/"),
            "regex with // not preserved: {out1:?}"
        );
        // ...and a chain of divisions is not mistaken for a regex (which would swallow tokens).
        let out2 = minify_js("var q = a / b / c\n");
        assert!(
            out2.contains("a / b / c"),
            "division misread as regex: {out2:?}"
        );
        // A regex right after `return` (keyword => expression position) is a regex, and its
        // `/`-looking interior must not terminate it early via the char class.
        let out3 = minify_js("return /[a/b]/.test(x)\n");
        assert!(
            out3.contains("/[a/b]/.test(x)"),
            "return-position regex broke: {out3:?}"
        );
    }

    /// Independent reference: count `delim` in `src` everywhere EXCEPT inside `//`/`/* */`
    /// comments. String literals ('/"/`) are honored (a comment marker inside a string is not
    /// a comment) and their delimiters ARE counted, because `minify_js` keeps strings verbatim.
    /// Regex literals are treated as ordinary text, which is sound for this asset: its only
    /// regex (`/^H[1-6]$/`) contains no `{}()`, quote, or `//`.
    fn count_code_delims(src: &str, delim: char) -> usize {
        let chars: Vec<char> = src.chars().collect();
        let n = chars.len();
        let mut i = 0;
        let mut count = 0usize;
        // 0 normal, 1 line comment, 2 block comment, 3 string (quote in `q`)
        let mut state = 0u8;
        let mut q = '"';
        while i < n {
            let c = chars[i];
            match state {
                1 => {
                    if c == '\n' {
                        state = 0;
                    }
                    i += 1;
                }
                2 => {
                    if c == '*' && i + 1 < n && chars[i + 1] == '/' {
                        state = 0;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                3 => {
                    if c == '\\' && i + 1 < n {
                        if chars[i + 1] == delim {
                            count += 1; // kept verbatim in output => must be counted
                        }
                        i += 2;
                        continue;
                    }
                    if c == delim {
                        count += 1;
                    }
                    if c == q {
                        state = 0;
                    }
                    i += 1;
                }
                _ => {
                    if c == '/' && i + 1 < n && chars[i + 1] == '/' {
                        state = 1;
                        i += 2;
                        continue;
                    }
                    if c == '/' && i + 1 < n && chars[i + 1] == '*' {
                        state = 2;
                        i += 2;
                        continue;
                    }
                    if c == '"' || c == '\'' || c == '`' {
                        state = 3;
                        q = c;
                        i += 1;
                        continue;
                    }
                    if c == delim {
                        count += 1;
                    }
                    i += 1;
                }
            }
        }
        count
    }

    #[test]
    fn js_preserves_delimiter_balance_on_real_asset() {
        // Sanity check on the project's own hand-written enhancer JS (it SHIPS to readers):
        // minify_js must neither add nor drop any {, }, (, ) that live in code, strings, or
        // regex literals; only comment content (which itself contains parens/braces here) is
        // removed. A regex/comment misfire that swallowed a run would unbalance this.
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../core/assets/js/code-enhance"
        );
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .expect("code-enhance dir readable")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "js"))
            .collect();
        paths.sort();
        assert!(!paths.is_empty(), "found no enhancer JS to check");
        let mut src = String::new();
        for p in &paths {
            src.push_str(&std::fs::read_to_string(p).expect("read enhancer js"));
            src.push('\n');
        }
        let out = minify_js(&src);
        for delim in ['{', '}', '(', ')'] {
            assert_eq!(
                out.matches(delim).count(),
                count_code_delims(&src, delim),
                "delimiter {delim:?} count changed vs comments-removed input"
            );
        }
        assert_eq!(
            out.matches('{').count(),
            out.matches('}').count(),
            "braces unbalanced after minify"
        );
        assert_eq!(
            out.matches('(').count(),
            out.matches(')').count(),
            "parens unbalanced after minify"
        );
    }

    #[test]
    fn js_preserves_non_ascii_on_real_shipping_asset() {
        // Directly proves Finding 1 on a real asset that ships to readers: the reading-progress
        // enhancer's string literals carry `·`, `→`, and `×`, which must survive minification
        // byte-for-byte (the old `byte as char` path mojibake'd them into Latin-1 junk).
        let src = include_str!("../../core/assets/js/code-enhance/15-reading-progress.js");
        let out = minify_js(src);
        for run in ["Resume reading · ", "% →", "'×'"] {
            assert!(
                out.contains(run),
                "non-ASCII run {run:?} not preserved in minified asset"
            );
        }
        assert!(!out.contains('Â'), "mojibake leaked into a shipped asset");
    }
}
