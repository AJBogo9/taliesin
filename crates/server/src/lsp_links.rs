//! Document-wide scan for the file paths a `.tmd` points at: the `{{< include PATH >}}` and
//! `{{< embed PATH >}}` arguments. Feeds `textDocument/documentLink`.
//!
//! This is what [`crate::lsp_nav::classify_target`] cannot do. That answers "what is under
//! THIS cursor", which resolves a click once you already suspect a target is there — but an
//! affordance has to be painted over every path in the buffer at once, before any click. A
//! go-to-definition nobody knows to try is invisible.
//!
//! The scan mirrors the engine instead of re-deriving it, because a link that disagrees with
//! the renderer is worse than no link: it promises a jump nothing will ever make.
//!   - a shortcode inside a fenced block or an inline code span is an EXAMPLE and stays
//!     literal (`render::extension::expand_shortcodes` + `expand_in_line`,
//!     `includes::resolve_into`)
//!   - `{{< include >}}` must own its whole trimmed line, and its path may be quoted
//!     (`includes::parse_include`)
//!   - `{{< embed >}}`'s path is the first argument that is not `key=value`
//!     (`render::extension::embed_path` + `is_named_arg`)
//!
//! Columns are scalar character indices, like the rest of the `lsp_*` modules; `lsp_pos`
//! converts them to UTF-16 at the wire boundary.

use crate::lsp_complete::Shortcode;

/// One navigable shortcode path: `[start, end)` are scalar columns on `line` spanning the
/// path token's text, quotes excluded.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PathLink {
    pub(crate) line: u32,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) path: String,
    pub(crate) shortcode: Shortcode,
}

/// One tokenized shortcode argument. `start`/`end` are the RAW scalar span (quotes included,
/// so a caller can highlight it); `value` is the engine's view of the token.
struct Token {
    start: usize,
    end: usize,
    value: String,
}

/// Tokenize `chars[from, to)` the way `render::extension::tokenize_args` does: a quote
/// toggles quoting rather than delimiting, so whitespace inside quotes does not split and
/// `title="A deck"` stays one token. Splitting naively on whitespace lets a quoted `title=`
/// leak its second word into the positional slot, which would link `deck"` instead of the
/// actual path.
fn tokenize(chars: &[char], from: usize, to: usize) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    let mut quote: Option<char> = None;
    let mut start: Option<usize> = None;
    let mut value = String::new();
    for (i, &ch) in chars.iter().enumerate().take(to).skip(from) {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    value.push(ch);
                }
            }
            None if ch == '"' || ch == '\'' => {
                start.get_or_insert(i);
                quote = Some(ch);
            }
            None if ch.is_whitespace() => {
                if let Some(s) = start.take()
                    && !value.is_empty()
                {
                    out.push(Token {
                        start: s,
                        end: i,
                        value: std::mem::take(&mut value),
                    });
                }
                value.clear();
                start = None;
            }
            None => {
                start.get_or_insert(i);
                value.push(ch);
            }
        }
    }
    if let Some(s) = start
        && !value.is_empty()
    {
        out.push(Token {
            start: s,
            end: to,
            value,
        });
    }
    out
}

/// `[start, end)` with one layer of surrounding quotes removed, so a link underlines the
/// path rather than the quotes around it.
fn unquote(chars: &[char], start: usize, end: usize) -> (usize, usize) {
    if end >= start + 2
        && (chars[start] == '"' || chars[start] == '\'')
        && chars[end - 1] == chars[start]
    {
        (start + 1, end - 1)
    } else {
        (start, end)
    }
}

/// The include link on this line, if any. `includes::parse_include` strips `{{<`/`>}}` off
/// the TRIMMED LINE, so the directive must own its line — a mid-sentence `{{< include >}}` is
/// never expanded and must not be linked.
fn include_link(chars: &[char], line: u32) -> Option<PathLink> {
    let lead = chars.iter().take_while(|c| c.is_whitespace()).count();
    let trail = chars.iter().rev().take_while(|c| c.is_whitespace()).count();
    let end = chars.len().checked_sub(trail)?;
    if end < lead + 6 {
        return None;
    }
    if chars[lead..lead + 3] != ['{', '{', '<'] || chars[end - 3..end] != ['>', '}', '}'] {
        return None;
    }
    let toks = tokenize(chars, lead + 3, end - 3);
    if toks.len() != 2 || toks[0].value != "include" {
        return None;
    }
    let (s, e) = unquote(chars, toks[1].start, toks[1].end);
    if toks[1].value.is_empty() {
        return None;
    }
    Some(PathLink {
        line,
        start: s,
        end: e,
        path: toks[1].value.clone(),
        shortcode: Shortcode::Include,
    })
}

/// Every navigable `{{< include >}}` path in `text`, in reading order.
pub(crate) fn path_links(text: &str) -> Vec<PathLink> {
    let mut out = Vec::new();
    let mut in_code = false;
    for (i, line) in text.split('\n').enumerate() {
        let t = line.trim_start();
        // A fence line toggles code state and is never itself a shortcode (mirrors
        // `expand_shortcodes`, which toggles on either marker).
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code || !line.contains("{{<") {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        if let Some(inc) = include_link(&chars, i as u32) {
            out.push(inc);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(text: &str) -> Vec<String> {
        path_links(text).into_iter().map(|l| l.path).collect()
    }

    #[test]
    fn finds_an_include_and_spans_exactly_the_path_token() {
        let text = "intro\n\n{{< include _includes/setup.tmd >}}\n";
        let links = path_links(text);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].line, 2);
        assert_eq!(links[0].path, "_includes/setup.tmd");
        assert_eq!(links[0].shortcode, Shortcode::Include);
        let line: Vec<char> = "{{< include _includes/setup.tmd >}}".chars().collect();
        let span: String = line[links[0].start..links[0].end].iter().collect();
        assert_eq!(span, "_includes/setup.tmd");
    }

    #[test]
    fn strips_quotes_around_an_include_path_and_spans_the_inside() {
        let line = r#"{{< include "a b/part.tmd" >}}"#;
        let links = path_links(&format!("{line}\n"));
        assert_eq!(links[0].path, "a b/part.tmd");
        let chars: Vec<char> = line.chars().collect();
        let span: String = chars[links[0].start..links[0].end].iter().collect();
        assert_eq!(span, "a b/part.tmd");
    }

    #[test]
    fn a_shortcode_inside_a_fenced_block_is_an_example() {
        let text = "```\n{{< include real.tmd >}}\n```\n\n{{< include after.tmd >}}\n";
        assert_eq!(paths(text), vec!["after.tmd"]);
    }

    #[test]
    fn a_tilde_fence_also_suppresses_links() {
        assert!(path_links("~~~\n{{< include x.tmd >}}\n~~~\n").is_empty());
    }

    #[test]
    fn a_shortcode_inside_an_inline_code_span_is_an_example() {
        assert!(path_links("Write `{{< include part.tmd >}}` to splice it in.\n").is_empty());
    }

    #[test]
    fn an_include_must_own_its_whole_line() {
        // `includes::parse_include` strips the fences off the TRIMMED LINE, so a mid-sentence
        // include is never expanded; linking it would promise a jump the renderer never makes.
        assert!(path_links("See {{< include part.tmd >}} for details.\n").is_empty());
    }

    #[test]
    fn an_indented_include_is_still_a_link() {
        let links = path_links("   {{< include part.tmd >}}\n");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].start, "   {{< include ".chars().count());
    }

    #[test]
    fn shortcodes_that_take_no_path_yield_no_link() {
        assert!(
            path_links("{{< video clip.mp4 >}}\n{{< input type=slider name=x >}}\n").is_empty()
        );
    }

    #[test]
    fn an_unterminated_shortcode_yields_no_link() {
        assert!(path_links("{{< include broken.tmd\n").is_empty());
    }

    #[test]
    fn a_document_with_no_shortcodes_produces_no_links() {
        assert!(path_links("# Title\n\nJust prose.\n").is_empty());
    }

    #[test]
    fn columns_are_scalar_indices_not_bytes() {
        // An astral char inside the path must shift the span's end by ONE scalar per char,
        // not by its four UTF-8 bytes; `lsp_pos` converts to UTF-16 at the wire, and a byte
        // index here would land mid-character. The astral char goes IN the path because an
        // include owns its whole line, so nothing may precede the directive.
        let line = "{{< include 😀/d.tmd >}}";
        let links = path_links(&format!("{line}\n"));
        assert_eq!(links.len(), 1);
        let chars: Vec<char> = line.chars().collect();
        let span: String = chars[links[0].start..links[0].end].iter().collect();
        assert_eq!(span, "😀/d.tmd");
    }
}
