//! The highlighting pin doc: every language it labels either emits `tali-hl-` scope
//! classes or stays plain **on purpose**, and `taliesin check` stays silent on all of
//! them. `ts` and `toml` are the load-bearing cases: syntect's bundled syntax set
//! carries neither, so both silently degraded to escaped plain text before the
//! `bat`-curated extras were loaded.

mod common;
use common::corpus_dir;
use std::fs;

fn doc_html() -> String {
    let path = corpus_dir().join("highlight.tmd");
    let src = fs::read_to_string(&path).unwrap();
    let doc = taliesin_core::render_document_with_includes(&src, path.parent().unwrap());
    doc.blocks.iter().map(|b| b.html.as_str()).collect()
}

/// The inner HTML of the `<code class="language-{lang}">` block.
fn code_body(html: &str, lang: &str) -> String {
    let open = format!("<code class=\"language-{lang}\">");
    let start = html
        .find(&open)
        .unwrap_or_else(|| panic!("no `{lang}` fence in the pin doc"))
        + open.len();
    let end = html[start..].find("</code>").expect("closed <code>") + start;
    html[start..end].to_string()
}

fn scope_spans(html: &str, lang: &str) -> usize {
    code_body(html, lang).matches("tali-hl-").count()
}

#[test]
fn bundled_languages_highlight() {
    let html = doc_html();
    for lang in ["rust", "python"] {
        assert!(
            scope_spans(&html, lang) > 0,
            "`{lang}` emitted no scope classes"
        );
    }
}

/// The capability this pins: syntect's defaults have no TypeScript and no TOML.
#[test]
fn typescript_and_toml_highlight() {
    let html = doc_html();
    for lang in ["ts", "toml"] {
        assert!(
            scope_spans(&html, lang) > 0,
            "`{lang}` degraded to plain text; the extra syntaxes are not loaded"
        );
    }
}

/// Scope classes are worthless if the stylesheet does not colour them.
#[test]
fn typescript_emits_scopes_the_stylesheet_colours() {
    let body = code_body(&doc_html(), "ts");
    for scope in ["tali-hl-comment", "tali-hl-keyword", "tali-hl-storage"] {
        assert!(body.contains(scope), "ts is missing `{scope}`: {body}");
    }
}

/// The third tier: a language *neither* set carries, filled by a grammar vendored into
/// the repo. Both of its tokens are pinned, because `ps1` reaches the syntax through the
/// grammar's own `file_extensions` rather than through `alias()`, so the two can break
/// independently.
#[test]
fn powershell_highlights_from_the_vendored_grammar() {
    let html = doc_html();
    for lang in ["powershell", "ps1"] {
        assert!(
            scope_spans(&html, lang) > 0,
            "`{lang}` degraded to plain text; the vendored PowerShell grammar is not \
             loaded (it is `.sublime-syntax` — syntect cannot read a `.tmLanguage`)"
        );
    }
}

/// Same rule as TypeScript's: scopes the stylesheet does not colour are worthless. The
/// palette is small and scope-name-based, so a grammar can parse perfectly and still
/// render as one flat colour.
#[test]
fn powershell_emits_scopes_the_stylesheet_colours() {
    let body = code_body(&doc_html(), "powershell");
    for scope in ["tali-hl-comment", "tali-hl-keyword", "tali-hl-variable"] {
        assert!(
            body.contains(scope),
            "powershell is missing `{scope}`: {body}"
        );
    }
}

#[test]
fn intentionally_plain_fences_emit_no_scopes() {
    let html = doc_html();
    for lang in ["text", "console"] {
        assert_eq!(
            scope_spans(&html, lang),
            0,
            "`{lang}` must render unhighlighted"
        );
    }
}

/// The pin doc must not itself trip the code-language validator: every token in it is
/// either a real syntax or an intentionally-plain one.
#[test]
fn the_pin_doc_reports_no_unknown_languages() {
    let path = corpus_dir().join("highlight.tmd");
    let src = fs::read_to_string(&path).unwrap();
    let doc = taliesin_core::render_document_with_includes(&src, path.parent().unwrap());
    let ws = taliesin_core::diagnostics::validate_code_languages(&doc.blocks);
    assert!(
        ws.is_empty(),
        "{:?}",
        ws.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}

/// Item 127 (2026-07-28). ` ```rust,ignore ` is the shared convention of mdBook, rustdoc,
/// Pandoc, Docusaurus and GitHub, and Taliesin took the whole info string as the language:
/// measured on `rust-lang/book`, **329 of 457** reported problems were this one shape, and on
/// one real chapter 11 of 18 code blocks rendered unstyled with `class="language-rust,ignore"`
/// — a comma is not a valid class token, so nothing could style it — while the diagnostic
/// called the ecosystem's standard form a spelling error.
///
/// The corpus had this shape **nowhere**, which is exactly why nothing caught it; the fixture
/// added to `highlight.tmd` is the other half of this pin.
#[test]
fn a_language_with_attributes_highlights_on_its_first_token() {
    let html = doc_html();
    // No emitted class may carry a comma: that is the defect in its most direct form, and it
    // is checked over the whole doc rather than one fence so a future attribute spelling
    // cannot reintroduce it somewhere else.
    assert!(
        !html.contains("class=\"language-rust,"),
        "an attribute must not leak into the class token"
    );
    // Needle the full opening tag, never the bare class name: this pin doc *explains* the
    // defect in prose, so `language-rust,ignore` legitimately appears on the page as inline
    // code. A substring assertion failed on exactly that, which is the inlined-needle trap in
    // miniature — the shape to assert is the tag, not the string.
    for bad in [
        "<code class=\"language-rust,ignore\">",
        "<code class=\"language-python should_panic\">",
        "<code class=\"language-rust,no_run,edition2021\">",
    ] {
        assert!(!html.contains(bad), "`{bad}` must not be emitted");
    }
    // And the fences are really there and really highlighted: `rust` appears three times in
    // the pin doc now (plain, `,ignore`, `,no_run,edition2021`) and `python` twice.
    assert_eq!(
        html.matches("<code class=\"language-rust\">").count(),
        3,
        "three rust fences, one plain and two attributed"
    );
    assert_eq!(
        html.matches("<code class=\"language-python\">").count(),
        2,
        "two python fences, one plain and one space-attributed"
    );
    // Non-vacuity: every one of those five blocks must carry scope classes, not just the
    // right class name. `code_body` finds the FIRST match, so assert on each occurrence.
    for (lang, count) in [("rust", 3), ("python", 2)] {
        let open = format!("<code class=\"language-{lang}\">");
        let mut cursor = 0usize;
        for n in 0..count {
            let start = html[cursor..].find(&open).expect("fence present") + cursor + open.len();
            let end = html[start..].find("</code>").expect("closed") + start;
            assert!(
                html[start..end].contains("tali-hl-"),
                "`{lang}` fence #{n} emitted no scope classes"
            );
            cursor = end;
        }
    }
}
