//! Server-side syntax highlighting (syntect), done at render time.
//!
//! Code is colored in Rust rather than by a client-side highlight.js pass. Three
//! wins, matching how KaTeX is already handled (render in Rust, ship offline):
//!
//!   - the exported HTML is self-contained — no CDN dependency for a built post's
//!     own highlighting,
//!   - the first paint (SSR) is already highlighted — no flash of plain code, and
//!     nothing to re-run on the client after each block update,
//!   - highlighting moves into the corpus-tested Rust half.
//!
//! We emit syntect's *scope classes* (each prefixed `tali-hl-`) rather than inline
//! colors, and map them to a palette in CSS with a `[data-theme=dark]` override,
//! so the light/dark toggle restyles code with no re-highlight.

use std::sync::OnceLock;
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Prefix on every emitted scope class, so they can't collide with page CSS and
/// are easy to target (`.tali-hl-keyword`, `.tali-hl-string`, …).
const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "tali-hl-" };

/// The default syntax set, loaded once. `_newlines` is the variant the line-based
/// `ClassedHTMLGenerator` expects.
fn syntaxes() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Map a Quarto/markdown language token to a token syntect knows.
fn alias(lang: &str) -> &str {
    match lang {
        "ojs" | "js" => "javascript",
        "ts" => "typescript",
        "sh" | "shell" | "zsh" => "bash",
        "py" => "python",
        "rs" => "rust",
        "yml" => "yaml",
        other => other,
    }
}

/// Highlight a fenced code block's `code` for `lang`, returning the inner HTML for
/// a `<code>` element: text is HTML-escaped and wrapped in `<span class="tali-hl-…">`
/// scope spans. An unknown/missing language (or any highlighter error) falls back
/// to plain escaped text, so output is always valid and never panics.
pub fn highlight(code: &str, lang: Option<&str>) -> String {
    let ss = syntaxes();
    let Some(syntax) = lang.map(alias).and_then(|t| ss.find_syntax_by_token(t)) else {
        return crate::render::html_escape(code);
    };
    let mut hl = ClassedHTMLGenerator::new_with_class_style(syntax, ss, CLASS_STYLE);
    for line in LinesWithEndings::from(code) {
        if hl.parse_html_for_line_which_includes_newline(line).is_err() {
            return crate::render::html_escape(code);
        }
    }
    hl.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_language_emits_scope_classes() {
        let html = highlight("def f():\n    return 1\n", Some("python"));
        assert!(
            html.contains("tali-hl-"),
            "no scope classes emitted: {html}"
        );
        // the keyword `def` should be wrapped as a keyword scope
        assert!(html.contains("tali-hl-keyword"), "keyword not highlighted");
    }

    #[test]
    fn unknown_language_is_plain_escaped() {
        let html = highlight("a < b && c", Some("no-such-lang"));
        assert_eq!(html, "a &lt; b &amp;&amp; c");
        assert!(!html.contains("tali-hl-"));
    }

    #[test]
    fn no_language_is_plain_escaped() {
        assert_eq!(highlight("x < y", None), "x &lt; y");
    }
}
