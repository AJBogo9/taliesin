//! A conservative, dependency-free CSS minifier for the build-time shared asset bundle.
//! Fully UTF-8 correct: it iterates by `char` and copies non-ASCII content (e.g. `·`, `×`,
//! `…`, `→`, `é`) through untouched, inside strings and out. It collapses whitespace and
//! strips comments, string- and `url()`-aware.
//!
//! **The JS half was cut on 2026-08-08 with the rest of the web-platform ops.** It was a
//! 235-line stateful tokenizer (ASI-safe newline preservation, regex-vs-division
//! disambiguation, nested template interpolation) verified against an acorn token-stream
//! oracle over Node — the one place in this repository where a silent mis-tokenization
//! could ship a broken script that no page visibly failed on. The CSS half carries roughly
//! three quarters of the measured gzipped saving on its own and has the opposite failure
//! mode: a broken stylesheet is visible on the first page you look at. `build.rs` now ships
//! the hand-written JS verbatim, which changes every `_assets/app.<hash>.js` URL and
//! nothing else.

/// Would `prev` and `next` tokenize as ONE token if written adjacent? Used to decide
/// whether dropping a CSS comment between them needs a space to preserve tokenization.
fn css_tokens_would_merge(prev: char, next: char) -> bool {
    // CSS ident continuation: letters, digits, `-`, `_`, and anything non-ASCII.
    let ident = |c: char| c.is_alphanumeric() || c == '-' || c == '_' || (c as u32) >= 0x80;
    // `0` + `auto`, `50` + `px`, `a` + `b`
    if ident(prev) && ident(next) {
        return true;
    }
    // a number absorbing a fractional part: `0` + `.5`, or `.` + `5`
    if prev.is_ascii_digit() && next == '.' {
        return true;
    }
    if prev == '.' && next.is_ascii_digit() {
        return true;
    }
    false
}

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
        // block comment: drop it entirely, but never let its removal FUSE the tokens it
        // separated. Browser-verified: `margin:0/* x */auto` computes the same as
        // `margin:0 auto` (marginLeft 910px), while the fused `margin:0auto` is invalid
        // and silently drops to 0px. Fusion also runs the other way: `50/**/px` is an
        // invalid <number><ident> width originally, and fusing it to `50px` would
        // silently make a broken declaration work.
        //
        // A space is only correct where the tokens would actually merge. `.a/* c */.b`
        // must stay `.a.b` (a compound selector; browser-verified that it matches an
        // element with both classes and NOT a descendant), because `.` cannot continue
        // the `a` ident, so no merge happens and a space would change the meaning.
        if chars[i..].starts_with(&['/', '*']) {
            let mut j = i + 2;
            while j + 1 < n && !(chars[j] == '*' && chars[j + 1] == '/') {
                j += 1;
            }
            i = if j + 1 < n { j + 2 } else { n };
            if let (Some(prev), Some(&next)) = (out.chars().last(), chars.get(i))
                && css_tokens_would_merge(prev, next)
            {
                out.push(' ');
                last_was_space = true;
            }
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
    fn css_empty_input_is_empty_output() {
        assert_eq!(minify_css(""), "");
    }

    #[test]
    fn css_comment_between_value_tokens_keeps_them_apart() {
        // Dropping a comment must not FUSE two tokens. Browser-verified: `margin:0 auto`
        // and `margin:0/* reset */auto` both compute marginLeft 910px (centred), while
        // `margin:0auto` computes 0px — the declaration is invalid and silently dropped.
        // So emitting `0auto` here silently un-centres the element.
        let out = minify_css(".x{margin:0/* reset */auto}");
        assert!(!out.contains("/*"), "comment stripped: {out}");
        assert!(
            out.contains("0 auto"),
            "value tokens stay separate tokens: {out}"
        );
        // The mirror case: `50/**/px` is <number><ident> = an invalid width originally,
        // so it must STAY invalid. Fusing it to `50px` would silently make a broken
        // declaration work, which is just as much a behaviour change.
        let out = minify_css(".x{width:50/* c */px}");
        assert!(out.contains("50 px"), "no accidental token fusion: {out}");
        // A number can absorb a following `.5`: fusing here turns two values
        // (0 vertical, .5em horizontal) into one (.5em on all four sides).
        let out = minify_css(".x{margin:0/* c */.5em}");
        assert!(
            out.contains("0 .5em"),
            "a number must not absorb `.5`: {out}"
        );
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
    fn css_preserves_non_ascii_bytes_exactly() {
        // The exact multi-byte run must survive byte-for-byte inside a content:"..." string.
        let run = "· × … → ↵ é";
        let out = minify_css(&format!(".x::after{{content:\"{run}\"}}"));
        assert!(out.contains(run), "non-ASCII CSS content mangled: {out:?}");
        // No Latin-1 mojibake artifact (the old `byte as char` bug produced e.g. "Â·").
        assert!(!out.contains('Â'), "mojibake leaked into CSS: {out:?}");
    }
}
