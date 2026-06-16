//! Pragmatic citations and cross-references.
//!
//! Citations (`[@key]`, `[@key, locator]`, `[@a; @b]`) become numbered links to
//! an auto-generated References section, formatted from a parsed BibTeX file.
//! Cross-references (`@fig-x`, `@sec-x`, ...) become links to their anchor,
//! labelled by kind and, when the anchor's number is known (e.g. a static
//! `#fig-` figure), carrying it ("Figure 3"). This is not a full CSL engine —
//! numbering for *computed* figures would arrive with execution.
//!
//! Processing runs over the already-rendered block HTML, transforming only
//! plain-text runs (never inside tags, code, or math), so block sourcepos is
//! untouched. The only structural change is appending a References block.

use crate::render::Block;
use std::collections::HashMap;

/// A parsed BibTeX database.
#[derive(Default)]
pub struct Bibliography {
    entries: HashMap<String, HashMap<String, String>>,
}

impl Bibliography {
    /// Format one entry as a reference string (HTML). `None` if unknown.
    fn format(&self, key: &str) -> Option<String> {
        let e = self.entries.get(key)?;
        let mut parts: Vec<String> = Vec::new();
        if let Some(a) = e.get("author") {
            parts.push(esc(&format_authors(a)));
        }
        if let Some(t) = e.get("title") {
            parts.push(format!("<em>{}</em>", esc(t)));
        }
        let venue = e
            .get("journal")
            .or_else(|| e.get("booktitle"))
            .or_else(|| e.get("publisher"))
            .or_else(|| e.get("organization"));
        if let Some(v) = venue {
            parts.push(esc(v));
        }
        if let Some(y) = e.get("year") {
            parts.push(esc(y));
        }
        Some(parts.join(", "))
    }
}

/// Parse a BibTeX string into a [`Bibliography`]. Tolerant of `{...}`/`"..."`
/// values and brace nesting; ignores comments and `@string`/`@comment`.
pub fn parse_bib(text: &str) -> Bibliography {
    let mut entries = HashMap::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        i += 1;
        let kind = take_while(&chars, &mut i, |c| c.is_alphanumeric()).to_ascii_lowercase();
        skip_ws(&chars, &mut i);
        if i >= chars.len() || chars[i] != '{' {
            continue;
        }
        i += 1; // past '{'
        if kind == "comment" || kind == "string" || kind == "preamble" {
            skip_entry(&chars, &mut i);
            continue;
        }
        let key = take_while(&chars, &mut i, |c| c != ',' && c != '}')
            .trim()
            .to_string();
        let mut fields = HashMap::new();
        if i < chars.len() && chars[i] == ',' {
            i += 1;
        }
        loop {
            skip_ws(&chars, &mut i);
            if i >= chars.len() || chars[i] == '}' {
                break;
            }
            let name = take_while(&chars, &mut i, |c| c != '=' && c != '}' && c != ',')
                .trim()
                .to_ascii_lowercase();
            skip_ws(&chars, &mut i);
            if i >= chars.len() || chars[i] != '=' {
                break;
            }
            i += 1; // past '='
            skip_ws(&chars, &mut i);
            let value = read_value(&chars, &mut i);
            if !name.is_empty() {
                fields.insert(name, value);
            }
            skip_ws(&chars, &mut i);
            if i < chars.len() && chars[i] == ',' {
                i += 1;
            }
        }
        if i < chars.len() && chars[i] == '}' {
            i += 1;
        }
        if !key.is_empty() {
            entries.insert(key, fields);
        }
    }
    Bibliography { entries }
}

fn take_while(chars: &[char], i: &mut usize, pred: impl Fn(char) -> bool) -> String {
    let start = *i;
    while *i < chars.len() && pred(chars[*i]) {
        *i += 1;
    }
    chars[start..*i].iter().collect()
}

fn skip_ws(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i].is_whitespace() {
        *i += 1;
    }
}

fn skip_entry(chars: &[char], i: &mut usize) {
    let mut depth = 1;
    while *i < chars.len() && depth > 0 {
        match chars[*i] {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        *i += 1;
    }
}

/// Read a field value: `{...}` (brace-nested), `"..."`, or a bare token.
fn read_value(chars: &[char], i: &mut usize) -> String {
    let mut out = String::new();
    match chars.get(*i) {
        Some('{') => {
            let mut depth = 0;
            while *i < chars.len() {
                match chars[*i] {
                    '{' => {
                        depth += 1;
                        if depth > 1 {
                            out.push('{');
                        }
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            *i += 1;
                            break;
                        }
                        out.push('}');
                    }
                    c => out.push(c),
                }
                *i += 1;
            }
        }
        Some('"') => {
            *i += 1;
            while *i < chars.len() && chars[*i] != '"' {
                out.push(chars[*i]);
                *i += 1;
            }
            if *i < chars.len() {
                *i += 1;
            }
        }
        _ => {
            out = take_while(chars, i, |c| c != ',' && c != '}');
        }
    }
    normalize_ws(&out)
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "Bishop, Christopher M and Doe, Jane" -> "C. M. Bishop, J. Doe".
fn format_authors(raw: &str) -> String {
    raw.split(" and ")
        .map(|name| match name.split_once(',') {
            Some((last, first)) => {
                let initials: String = first
                    .split_whitespace()
                    .filter_map(|w| w.chars().next())
                    .map(|c| format!("{c}. "))
                    .collect();
                format!("{}{}", initials, last.trim())
            }
            None => name.trim().to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Cross-reference kind prefixes -> display label.
fn xref_label(prefix: &str) -> Option<&'static str> {
    match prefix {
        "fig" => Some("Figure"),
        "tbl" => Some("Table"),
        "sec" => Some("Section"),
        "eq" => Some("Equation"),
        "lst" => Some("Listing"),
        "thm" => Some("Theorem"),
        "def" => Some("Definition"),
        _ => None,
    }
}

/// Resolve citations + cross-references across `blocks`, appending a References
/// block when citations were found and the bibliography could format them.
/// `xrefs` maps a cross-reference anchor (e.g. `fig-scree`) to its resolved
/// number, so `@fig-scree` renders as a linked "Figure 3".
pub fn process(blocks: &mut Vec<Block>, bib: &Bibliography, xrefs: &HashMap<String, String>) {
    let mut order: Vec<String> = Vec::new();
    let mut number: HashMap<String, usize> = HashMap::new();
    let mut cite_key = |key: &str| -> usize {
        *number.entry(key.to_string()).or_insert_with(|| {
            order.push(key.to_string());
            order.len()
        })
    };

    for b in blocks.iter_mut() {
        b.html = transform_html(&b.html, &mut cite_key, xrefs);
    }

    if order.is_empty() {
        return;
    }
    let mut list = String::from(
        "<section class=\"qmd-references\" data-block-id=\"qmd-references\"><h2>References</h2>",
    );
    for (idx, key) in order.iter().enumerate() {
        let formatted = bib
            .format(key)
            .unwrap_or_else(|| format!("<code>{}</code>", esc(key)));
        list.push_str(&format!(
            "<div id=\"ref-{}\" class=\"csl-entry\">[{}] {}</div>",
            esc(key),
            idx + 1,
            formatted
        ));
    }
    list.push_str("</section>");
    blocks.push(Block {
        id: "qmd-references".to_string(),
        sourcepos: String::new(),
        source_file: None,
        html: list,
        cell: None,
    });
}

/// Walk HTML, transforming only plain-text runs (never inside tags or inside
/// `pre`/`code`/`script`/`style`/`annotation` elements).
fn transform_html(
    html: &str,
    cite_key: &mut impl FnMut(&str) -> usize,
    xrefs: &HashMap<String, String>,
) -> String {
    const SKIP: [&str; 5] = ["pre", "code", "script", "style", "annotation"];
    let mut out = String::with_capacity(html.len());
    let mut skip_depth = 0usize;
    let mut rest = html;
    while !rest.is_empty() {
        if rest.starts_with('<') {
            let end = rest.find('>').map(|e| e + 1).unwrap_or(rest.len());
            let tag = &rest[..end];
            let name: String = tag
                .trim_start_matches(['<', '/'])
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            if SKIP.contains(&name.as_str()) {
                if tag.starts_with("</") {
                    skip_depth = skip_depth.saturating_sub(1);
                } else if !tag.ends_with("/>") {
                    skip_depth += 1;
                }
            }
            out.push_str(tag);
            rest = &rest[end..];
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let text = &rest[..end];
            if skip_depth == 0 {
                out.push_str(&rewrite_text(text, cite_key, xrefs));
            } else {
                out.push_str(text);
            }
            rest = &rest[end..];
        }
    }
    out
}

/// Rewrite citations and cross-references within a plain-text run.
fn rewrite_text(
    text: &str,
    cite_key: &mut impl FnMut(&str) -> usize,
    xrefs: &HashMap<String, String>,
) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            if let Some(close) = chars[i + 1..].iter().position(|&c| c == ']') {
                let inner: String = chars[i + 1..i + 1 + close].iter().collect();
                if inner.contains('@') {
                    out.push_str(&render_citation_group(&inner, cite_key));
                    i += close + 2;
                    continue;
                }
            }
        } else if chars[i] == '@' {
            if let Some((label, anchor, len)) = parse_xref(&chars[i..]) {
                // A resolved number renders "Figure&nbsp;3"; otherwise just the label.
                let text = match xrefs.get(&anchor) {
                    Some(n) => format!("{label}&nbsp;{n}"),
                    None => label.to_string(),
                };
                out.push_str(&format!(
                    "<a href=\"#{anchor}\" class=\"qmd-xref\">{text}</a>"
                ));
                i += len;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// `@fig-x` -> ("Figure", "fig-x", consumed_len).
fn parse_xref(chars: &[char]) -> Option<(&'static str, String, usize)> {
    // chars[0] == '@'
    let rest: String = chars[1..].iter().collect();
    let prefix: String = rest
        .chars()
        .take_while(|c| c.is_ascii_lowercase())
        .collect();
    let label = xref_label(&prefix)?;
    let after = &rest[prefix.len()..];
    if !after.starts_with('-') {
        return None;
    }
    let ident: String = after[1..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if ident.is_empty() {
        return None;
    }
    let anchor = format!("{prefix}-{ident}");
    let consumed = 1 + prefix.len() + 1 + ident.len();
    Some((label, anchor, consumed))
}

/// Render `@a; @b, p. 5` style citation group content into `[1, 2, p. 5]`.
fn render_citation_group(inner: &str, cite_key: &mut impl FnMut(&str) -> usize) -> String {
    let mut rendered: Vec<String> = Vec::new();
    for item in inner.split(';') {
        let item = item.trim().trim_start_matches('-'); // `-@key` suppresses author (n/a for numeric)
        let Some(at) = item.find('@') else { continue };
        let after = &item[at + 1..];
        let key: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == ':')
            .collect();
        if key.is_empty() {
            continue;
        }
        let locator = after[key.len()..].trim().trim_start_matches(',').trim();
        let n = cite_key(&key);
        let mut piece = format!("<a href=\"#ref-{}\">{}</a>", esc(&key), n);
        if !locator.is_empty() {
            piece.push_str(&format!(", {}", esc(locator)));
        }
        rendered.push(piece);
    }
    if rendered.is_empty() {
        format!("[{}]", esc(inner))
    } else {
        format!("[{}]", rendered.join(", "))
    }
}

fn esc(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bib() -> Bibliography {
        parse_bib(
            "@book{bishop2006pattern,\n  title = {Pattern Recognition and Machine Learning},\n  author = {Bishop, Christopher M},\n  year = {2006},\n  publisher = {Springer}\n}\n",
        )
    }

    #[test]
    fn parses_and_formats_entry() {
        let b = bib();
        let f = b.format("bishop2006pattern").unwrap();
        assert!(f.contains("C. M. Bishop"), "got: {f}");
        assert!(f.contains("<em>Pattern Recognition and Machine Learning</em>"));
        assert!(f.contains("Springer") && f.contains("2006"));
    }

    #[test]
    fn citation_becomes_numbered_link_with_locator() {
        let b = bib();
        let mut blocks = vec![Block {
            id: "x".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<p>fails [@bishop2006pattern, chap. 9].</p>".into(),
            cell: None,
        }];
        process(&mut blocks, &b, &HashMap::new());
        assert!(
            blocks[0]
                .html
                .contains("[<a href=\"#ref-bishop2006pattern\">1</a>, chap. 9]")
        );
        // a References section was appended
        let refs = blocks.last().unwrap();
        assert!(refs.html.contains("id=\"ref-bishop2006pattern\""));
        assert!(refs.html.contains("[1] C. M. Bishop"));
    }

    #[test]
    fn crossref_becomes_labelled_link() {
        let b = Bibliography::default();
        let mut blocks = vec![Block {
            id: "x".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<p>see @fig-scree for details</p>".into(),
            cell: None,
        }];
        process(&mut blocks, &b, &HashMap::new());
        assert!(
            blocks[0]
                .html
                .contains("<a href=\"#fig-scree\" class=\"qmd-xref\">Figure</a>"),
            "got: {}",
            blocks[0].html
        );
        // no citations -> no References section
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn crossref_resolves_number_from_registry() {
        let mut xrefs = HashMap::new();
        xrefs.insert("fig-scree".to_string(), "3".to_string());
        let mut blocks = vec![Block {
            id: "x".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<p>see @fig-scree for the elbow</p>".into(),
            cell: None,
        }];
        process(&mut blocks, &Bibliography::default(), &xrefs);
        assert!(
            blocks[0]
                .html
                .contains("<a href=\"#fig-scree\" class=\"qmd-xref\">Figure&nbsp;3</a>"),
            "got: {}",
            blocks[0].html
        );
    }

    #[test]
    fn citations_inside_code_are_left_alone() {
        let b = bib();
        let mut blocks = vec![Block {
            id: "x".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<pre><code>x = [@bishop2006pattern]</code></pre>".into(),
            cell: None,
        }];
        process(&mut blocks, &b, &HashMap::new());
        assert!(
            blocks[0].html.contains("[@bishop2006pattern]"),
            "code was rewritten"
        );
        assert_eq!(blocks.len(), 1, "no citation should have been counted");
    }
}
