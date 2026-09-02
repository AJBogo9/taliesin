//! Visible-text extraction for the Cmd-K search index.
//!
//! This was the `taliesin read` text projection — a screen-reader-like plain-text VIEW of a
//! whole document — until R6-11 (2026-08-09) removed it. Both consumers its doc comment
//! named were already gone: `site/llms.rs` with `llms-full.txt` in wave 4, and the `read`
//! verb in wave 2. What is left is the one walk that is still reached: the search index's,
//! which never went through the projection.
//!
//! Reuses mod.rs's private `strip_tags_separated`/`unescape_html` (a child module sees its
//! parent's privates) so the extraction stays identical to the TOC/slug path.

use super::*;

/// Decode already-stripped text: `&nbsp;` normalized to a space, numeric character
/// references resolved, then the named entities the renderer emits decoded exactly once.
/// The single home for this recipe — a caller that rewrites it by hand gets `&amp;lt;`
/// wrong (a chained `.replace` decodes it twice, to `<`).
///
/// **Numeric refs are decoded BEFORE the named ones**, for the same reason `&amp;` is
/// decoded last: a literal, double-encoded `&amp;#8217;` must survive as the text
/// `&#8217;`, and a numeric pass that ran after `&amp;`→`&` would eat it. Author sources
/// carry these (`&#8217;`, `&#x2019;`) wherever a typographic mark was written as an
/// escape; leaving them raw published `it&#8217;s` into the search index.
fn decode(stripped: &str) -> String {
    unescape_html(&decode_numeric(&stripped.replace("&nbsp;", " ")))
}

/// Resolve `&#NNN;` / `&#xHH;` character references. An unterminated, over-long, or
/// out-of-range reference is left exactly as written rather than guessed at.
fn decode_numeric(s: &str) -> String {
    if !s.contains("&#") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("&#") {
        out.push_str(&rest[..i]);
        let body = &rest[i + 2..];
        let hex = body.starts_with(['x', 'X']);
        let digits = if hex { &body[1..] } else { body };
        // Bounded: the longest legal code point is 7 decimal digits (0x10FFFF = 1114111).
        let len = digits
            .chars()
            .take(8)
            .take_while(|c| c.is_digit(if hex { 16 } else { 10 }))
            .count();
        let ch = (len > 0 && digits[len..].starts_with(';'))
            .then(|| u32::from_str_radix(&digits[..len], if hex { 16 } else { 10 }).ok())
            .flatten()
            .and_then(char::from_u32);
        match ch {
            Some(c) => {
                out.push(c);
                rest = &digits[len + 1..];
            }
            // Not a resolvable reference: emit the `&` and rescan from the `#`, so a
            // later valid reference in the same string is still found.
            None => {
                out.push('&');
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Visible text of a *run* of block HTML, for the cross-page search index: tags stripped
/// (KaTeX `<math>` MathML dropped), entities decoded, a space at every tag boundary and
/// whitespace collapsed, since the index reads many blocks as one string.
///
/// Sharing [`strip_tags_separated`] is what keeps the index honest: a hand-rolled `<`/`>`
/// scan indexes KaTeX's MathML *and* its raw-TeX `<annotation>` alongside the visible
/// glyphs, so every formula lands three times and leaks LaTeX into the index.
pub(crate) fn indexable_text(html: &str) -> String {
    decode(&strip_tags_separated(html))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Numeric character references decode, and a literal double-encoded one does not.
    #[test]
    fn decode_resolves_numeric_references_once() {
        assert_eq!(decode("it&#8217;s"), "it\u{2019}s");
        assert_eq!(decode("it&#x2019;s"), "it\u{2019}s");
        // A literal, double-encoded reference must survive as text, not decode twice.
        assert_eq!(decode("&amp;#8217;"), "&#8217;");
        // Unterminated / nonsense references are left exactly as written.
        assert_eq!(decode("&#nope;"), "&#nope;");
        assert_eq!(decode("a &#8217 b"), "a &#8217 b");
    }

    /// The index reads many blocks as one string, so a tag boundary must leave a space or
    /// two fields weld into one token — and KaTeX must contribute its glyphs once, not its
    /// MathML and raw TeX as well.
    #[test]
    fn indexable_text_separates_blocks_and_collapses_space() {
        assert_eq!(
            indexable_text("<h2>Title</h2><p>Body&nbsp;text</p>"),
            "Title Body text"
        );
        assert_eq!(indexable_text("<p>a   b\n\nc</p>"), "a b c");
    }

    /// A `{js}`/`{glsl}` cell ships its author source inside a `<script type="…">` in the
    /// page body, and a `<script>` body is CDATA, not text: nothing there is on the page. It
    /// was reaching the index anyway (measured live on gallery.taliesin.sh, where
    /// `descent.html`'s section text was dominated by its gradient-descent cell's source, so
    /// a query for `const` returned a snippet appearing nowhere on the page). `<math>` was
    /// already skipped for the same reason; this is the same rule applied to the other
    /// element class whose body is not visible text.
    #[test]
    fn indexable_text_skips_raw_text_element_bodies() {
        assert_eq!(
            indexable_text(
                "<div class=\"cell tali-js\"><div class=\"tali-js-out\"></div>\
                 <script type=\"text/javascript\" data-name=\"n\">const width = 640;\
                 </script></div><p>Visible.</p>"
            ),
            "Visible."
        );
        // `<style>` likewise: a scoped rule block is not prose.
        assert_eq!(
            indexable_text("<style>.a{color:red}</style><p>Text.</p>"),
            "Text."
        );
        // The tag boundary still separates, so the skip cannot weld neighbours together.
        assert_eq!(
            indexable_text("<p>One.</p><script>x</script><p>Two.</p>"),
            "One. Two."
        );
        // A `<script>` shown as a code SAMPLE is escaped text, not an element, and stays.
        assert_eq!(
            indexable_text("<pre><code>&lt;script&gt;kept&lt;/script&gt;</code></pre>"),
            "<script>kept</script>"
        );
    }
}
