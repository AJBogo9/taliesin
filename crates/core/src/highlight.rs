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

/// The syntax set, loaded once: syntect's defaults plus the `bat`-curated extras.
/// The extras are why `typescript` and `toml` highlight at all: syntect's bundled
/// set carries neither, so both rendered as plain text before. `_newlines` is the
/// variant the line-based `ClassedHTMLGenerator` expects.
fn syntaxes() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(two_face::syntax::extra_newlines)
}

/// Map a markdown language token to a token the syntax set knows.
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

/// Tokens that deliberately render unhighlighted: console transcripts, sample
/// output, plain prose. They are not typos, so [`known_language`] accepts them
/// even though most resolve to no syntax.
const INTENTIONALLY_PLAIN: [&str; 6] = ["text", "txt", "plain", "console", "output", "none"];

/// Whether a fenced block's language token resolves to a syntax, or is a token we
/// render plainly on purpose. `false` means the fence silently degrades to escaped
/// text, nearly always a typo (`pyton`) or a language we carry no syntax for.
///
/// A pure query: it cannot change what [`highlight`] emits.
pub fn known_language(lang: &str) -> bool {
    INTENTIONALLY_PLAIN.contains(&lang) || syntaxes().find_syntax_by_token(alias(lang)).is_some()
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

    /// The docs use `ts` (22 blocks) and `toml` (8). syntect's bundled set carries
    /// neither, so both degraded to plain text until the `two-face` extras landed.
    #[test]
    fn typescript_and_toml_highlight() {
        let ts = highlight("const x: number = 1;\n", Some("ts"));
        assert!(ts.contains("tali-hl-"), "ts not highlighted: {ts}");
        let tsx = highlight("const x: number = 1;\n", Some("typescript"));
        assert!(
            tsx.contains("tali-hl-"),
            "typescript not highlighted: {tsx}"
        );
        let toml = highlight("[deps]\nx = 1\n", Some("toml"));
        assert!(toml.contains("tali-hl-"), "toml not highlighted: {toml}");
    }

    /// Swapping the syntax set must not silently re-resolve a language the corpus
    /// already renders: that would drift every affected document's HTML.
    #[test]
    fn established_languages_resolve_to_the_same_syntax() {
        let ss = syntaxes();
        for (token, expected) in [
            ("rust", "Rust"),
            ("rs", "Rust"),
            ("bash", "Bourne Again Shell (bash)"),
            ("sh", "Bourne Again Shell (bash)"),
            ("zsh", "Bourne Again Shell (bash)"),
            ("yaml", "YAML"),
            ("yml", "YAML"),
            ("js", "JavaScript"),
            ("ojs", "JavaScript"),
            ("markdown", "Markdown"),
            ("python", "Python"),
            ("py", "Python"),
            ("json", "JSON"),
            ("css", "CSS"),
            ("html", "HTML"),
            ("r", "R"),
            ("bibtex", "BibTeX"),
        ] {
            let got = ss
                .find_syntax_by_token(alias(token))
                .map(|s| s.name.as_str());
            assert_eq!(got, Some(expected), "`{token}` re-resolved");
        }
    }

    #[test]
    fn known_language_accepts_real_and_intentionally_plain_tokens() {
        for good in ["python", "py", "ts", "toml", "rust", "bibtex"] {
            assert!(known_language(good), "`{good}` should be known");
        }
        for plain in INTENTIONALLY_PLAIN {
            assert!(known_language(plain), "`{plain}` should be accepted");
        }
        for bad in ["pyton", "rustlang", "no-such-lang"] {
            assert!(!known_language(bad), "`{bad}` should be unknown");
        }
    }

    /// `text` stays plain: `known_language` accepts it, but nothing highlights it.
    #[test]
    fn intentionally_plain_tokens_still_render_plain() {
        assert_eq!(highlight("a < b", Some("text")), "a &lt; b");
        assert!(known_language("text"));
    }
}
