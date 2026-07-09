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
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Prefix on every emitted scope class, so they can't collide with page CSS and
/// are easy to target (`.tali-hl-keyword`, `.tali-hl-string`, …).
const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "tali-hl-" };

/// syntect's bundled syntaxes. `_newlines` is the variant the line-based
/// `ClassedHTMLGenerator` expects.
fn bundled() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The `bat`-curated extras, consulted **only** when [`bundled`] has no syntax for
/// a token. This is not a superset of syntect's defaults: it is a separate curated
/// set whose definitions for shared languages (Rust, Python, JavaScript, JSON,
/// HTML, YAML) emit *different* scope spans. Preferring it wholesale would silently
/// re-highlight every existing code block in every document, so the bundled set
/// always wins and the extras supply only what it lacks: TypeScript and TOML.
///
/// Loaded lazily, so a document that uses neither never pays to deserialize it.
fn extras() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(two_face::syntax::extra_newlines)
}

/// Resolve a token to its syntax **and the set that owns it** (the generator must be
/// given the owning set, since a `SyntaxReference` indexes into it).
fn resolve(token: &str) -> Option<(&'static SyntaxReference, &'static SyntaxSet)> {
    if let Some(s) = bundled().find_syntax_by_token(token) {
        return Some((s, bundled()));
    }
    extras().find_syntax_by_token(token).map(|s| (s, extras()))
}

/// Map a markdown language token to a token the syntax sets know.
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
    INTENTIONALLY_PLAIN.contains(&lang) || resolve(alias(lang)).is_some()
}

/// Highlight a fenced code block's `code` for `lang`, returning the inner HTML for
/// a `<code>` element: text is HTML-escaped and wrapped in `<span class="tali-hl-…">`
/// scope spans. An unknown/missing language (or any highlighter error) falls back
/// to plain escaped text, so output is always valid and never panics.
pub fn highlight(code: &str, lang: Option<&str>) -> String {
    let Some((syntax, ss)) = lang.map(alias).and_then(resolve) else {
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

    /// Adding the extras must not re-highlight a language syntect already knew.
    ///
    /// Comparing `SyntaxReference::name` is **not** enough: the extras carry their own
    /// "Rust"/"Python"/… definitions under identical names but different contexts, so a
    /// name check passes while the emitted bytes drift. Assert instead that every
    /// established token resolves *into the bundled set itself*.
    #[test]
    fn established_languages_still_come_from_the_bundled_set() {
        for token in [
            "rust", "rs", "bash", "sh", "zsh", "yaml", "yml", "js", "ojs", "markdown", "python",
            "py", "json", "css", "html", "r", "bibtex", "diff", "sql", "c",
        ] {
            let (_, set) = resolve(alias(token)).unwrap_or_else(|| panic!("`{token}` unresolved"));
            assert!(
                std::ptr::eq(set, bundled()),
                "`{token}` now resolves into the extras set; its highlighting would drift"
            );
        }
    }

    /// The bytes, not just the provenance: highlighting must match what the bundled
    /// set alone produces. These six are the languages whose extras definition differs.
    #[test]
    fn established_languages_emit_unchanged_bytes() {
        let ss = bundled();
        for (token, code) in [
            ("rust", "pub fn f(x: u32) -> u32 { x + 1 } // c\n"),
            ("python", "def f(x):\n    return 'a' # c\n"),
            ("js", "const a = 1; // c\n"),
            ("json", "{\"a\": 1}\n"),
            ("html", "<p class=\"x\">hi</p>\n"),
            ("yaml", "a: 1 # c\n"),
        ] {
            let syntax = ss.find_syntax_by_token(alias(token)).unwrap();
            let mut hl = ClassedHTMLGenerator::new_with_class_style(syntax, ss, CLASS_STYLE);
            for line in LinesWithEndings::from(code) {
                hl.parse_html_for_line_which_includes_newline(line).unwrap();
            }
            assert_eq!(
                highlight(code, Some(token)),
                hl.finalize(),
                "`{token}` highlighting drifted from the bundled set"
            );
        }
    }

    /// The extras are the only place TypeScript and TOML can come from.
    #[test]
    fn typescript_and_toml_come_from_the_extras() {
        for token in ["ts", "typescript", "toml"] {
            assert!(bundled().find_syntax_by_token(alias(token)).is_none());
            let (_, set) = resolve(alias(token)).expect("resolves via extras");
            assert!(std::ptr::eq(set, extras()));
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
