//! Conservative, dependency-free minifiers for the build-time shared asset bundle. CSS
//! collapses whitespace + strips comments (string/`url()`-aware). JS strips comments and
//! blank-line indentation but PRESERVES newlines (ASI-safe) and never mangles tokens; it
//! runs only on Taliesin's own hand-written JS (vendored `*.min.js` bypass it entirely).

// `not(test)`-gated: this module's own unit tests call these fns, so under
// `--all-targets` the test-target build always uses them (dead_code never fires
// there) while the plain bin build does not (Task 5 has not wired them in yet).
// Gating on `not(test)` keeps the expectation meaningful (and self-clearing once
// Task 5 adds a real, non-test caller) without it being permanently unfulfilled
// in the test-target build.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired up by the shared-asset-bundle build step")
)]
pub fn minify_css(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut last_was_space = false;
    while i < b.len() {
        // string literal: copy verbatim
        if b[i] == b'"' || b[i] == b'\'' {
            let q = b[i];
            out.push(q as char);
            i += 1;
            while i < b.len() {
                out.push(b[i] as char);
                if b[i] == b'\\' && i + 1 < b.len() {
                    out.push(b[i + 1] as char);
                    i += 2;
                    continue;
                }
                if b[i] == q {
                    i += 1;
                    break;
                }
                i += 1;
            }
            last_was_space = false;
            continue;
        }
        // url(...) : copy verbatim through the matching ')'
        if src[i..].starts_with("url(") {
            let end = src[i..].find(')').map(|e| i + e + 1).unwrap_or(b.len());
            out.push_str(&src[i..end]);
            i = end;
            last_was_space = false;
            continue;
        }
        // comment
        if src[i..].starts_with("/*") {
            let end = src[i + 2..]
                .find("*/")
                .map(|e| i + 2 + e + 2)
                .unwrap_or(b.len());
            i = end;
            continue;
        }
        if b[i].is_ascii_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
            i += 1;
            continue;
        }
        out.push(b[i] as char);
        last_was_space = false;
        i += 1;
    }
    out.trim().to_string()
}

// `not(test)`-gated for the same reason as `minify_css` above.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired up by the shared-asset-bundle build step")
)]
pub fn minify_js(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let stripped = strip_comments(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue; // drop blank (or now-blank) lines
        }
        out.push_str(trimmed);
        out.push('\n'); // preserve the statement newline (ASI)
    }
    out
}

/// Remove `//` line comments and self-contained (opening and closing on the same
/// line) `/* ... */` block comments from `line`, honoring `"`/`'`/backtick strings.
/// A block comment that does not close on this line is left intact (rare in our
/// sources; it stays on its own line, so leaving it is safe and simple). A removed
/// block comment is replaced with a single space so adjacent tokens never fuse
/// (e.g. `return/*x*/5` must stay two tokens, not become `return5`).
fn strip_comments(line: &str) -> String {
    let b = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let mut in_str: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = in_str {
            out.push(c as char);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1] as char);
                i += 2;
                continue;
            }
            if c == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        if c == b'"' || c == b'\'' || c == b'`' {
            in_str = Some(c);
            out.push(c as char);
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            break; // rest of the line is a line comment
        }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            if let Some(close) = line[i + 2..].find("*/") {
                i += 2 + close + 2;
                out.push(' ');
                continue;
            }
            out.push_str(&line[i..]); // unterminated on this line: leave intact
            break;
        }
        out.push(c as char);
        i += 1;
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
}
