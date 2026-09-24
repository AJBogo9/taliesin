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

/// Decode already-stripped text: `&nbsp;` normalized to a space (a reader types a space),
/// then every character reference decoded exactly once by [`unescape_html`], the one
/// decoder. A caller that rewrites this by hand gets `&amp;lt;` wrong (a chained
/// `.replace` decodes it twice, to `<`). Author sources carry numeric references
/// (`&#8217;`, `&#x2019;`) wherever a typographic mark was written as an escape; leaving
/// them raw published `it&#8217;s` into the search index.
fn decode(stripped: &str) -> String {
    unescape_html(&stripped.replace("&nbsp;", " "))
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

/// The text of ONE heading, for a search result's title: the TOC entry's own extraction
/// ([`strip_tags`], no boundary at any tag), decoded and whitespace-collapsed like
/// [`indexable_text`]. A heading is one run of text, so a tag inside it separates nothing:
/// read with a boundary at each one, `$H_0$` came out `H 0` beside a TOC reading `H0`.
pub(crate) fn heading_text(html: &str) -> String {
    decode(&strip_tags(html))
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

    /// A tag boundary separates fields and blocks; it never parts text from punctuation
    /// that touches it on the page. Every numbered caption read "Figure 1 : The caption"
    /// against a page reading "Figure 1: The caption", and "(<em>x</em>)" read "( x )".
    #[test]
    fn a_boundary_never_parts_text_from_the_punctuation_it_touches() {
        assert_eq!(
            indexable_text(
                "<figcaption><span class=\"tali-caption-label\">Figure&nbsp;1</span>: \
                 The caption.</figcaption>"
            ),
            "Figure 1: The caption."
        );
        assert_eq!(
            indexable_text("<p>Some (<em>emph</em>), then <a href=\"#x\">a link</a>.</p>"),
            "Some (emph), then a link."
        );
        // Fields and blocks still separate, and an existing space is not doubled.
        assert_eq!(
            indexable_text("<p>First.</p><p>Second.</p><div><span>A</span><span>B</span></div>"),
            "First. Second. A B"
        );
        assert_eq!(
            indexable_text("<p>x</p><script>y</script><p>(z)</p>"),
            "x (z)"
        );
    }

    /// A `<` that opens no tag is text, as the walker ([`tags`]) and the browser read it:
    /// an unescaped `a < b` in a raw-HTML block shows on the page. Read as the start of a
    /// tag, it hid everything up to the next `>`.
    #[test]
    fn a_lt_that_opens_no_tag_is_text() {
        let html = "<p>If 1 < 2 and 3 > 2, then <b>so</b>.</p>";
        assert_eq!(strip_tags(html), "If 1 < 2 and 3 > 2, then so.");
        assert_eq!(
            indexable_text("<p>If 1 < 2 and 3 > 2.</p><p>Next.</p>"),
            "If 1 < 2 and 3 > 2. Next."
        );
        // A comment, a closing tag and a doctype are still markup.
        assert_eq!(strip_tags("<!DOCTYPE html><p>a<!-- b -->c</p>"), "ac");
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
