//! Citation + cross-reference HTML processing: walk block HTML, rewrite `[@key]`
//! citation groups and `@fig-`/`@sec-`/… cross-references, and append a References
//! section. Transforms only plain-text runs (never tags/code/math), so block
//! sourcepos is untouched.

use super::{Bibliography, sourcepos_start_line};
use crate::render::{Block, Warning, escape_attr as esc};
use std::collections::HashMap;

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
/// Returns one warning per citation key not in the (non-empty) bibliography, for
/// the dev server's diagnostics. Empty when every citation resolves (or there's no
/// bibliography at all, in which case the missing-file case is reported elsewhere).
pub fn process(
    blocks: &mut Vec<Block>,
    bib: &Bibliography,
    xrefs: &HashMap<String, String>,
) -> Vec<Warning> {
    let mut order: Vec<String> = Vec::new();
    let mut number: HashMap<String, usize> = HashMap::new();
    // Track the block location where each cite key is first seen, for located warnings.
    // (file, line) pair per key; the RefCell lets the closure capture it alongside order/number.
    type KeyLocMap = HashMap<String, (Option<String>, Option<u32>)>;
    let key_loc: std::cell::RefCell<KeyLocMap> = std::cell::RefCell::new(HashMap::new());
    let cur_loc: std::cell::RefCell<(Option<String>, Option<u32>)> =
        std::cell::RefCell::new((None, None));
    let mut cite_key = |key: &str| -> usize {
        let n = *number.entry(key.to_string()).or_insert_with(|| {
            order.push(key.to_string());
            order.len()
        });
        // Record the block location the first time this key appears.
        key_loc
            .borrow_mut()
            .entry(key.to_string())
            .or_insert_with(|| cur_loc.borrow().clone());
        n
    };

    for b in blocks.iter_mut() {
        *cur_loc.borrow_mut() = (b.source_file.clone(), sourcepos_start_line(&b.sourcepos));
        b.html = transform_html(&b.html, &mut cite_key, xrefs);
    }
    let key_loc = key_loc.into_inner();

    if order.is_empty() {
        return Vec::new();
    }
    // If the author already wrote a `# References` / `# Bibliography` heading, render
    // the reference list under it instead of emitting a second "References" heading.
    let has_manual_heading = blocks.iter().any(|b| is_manual_references_heading(&b.html));
    let mut warnings: Vec<Warning> = Vec::new();
    let mut list =
        String::from("<section class=\"qmd-references\" data-block-id=\"qmd-references\">");
    if !has_manual_heading {
        list.push_str("<h2>References</h2>");
    }
    for (idx, key) in order.iter().enumerate() {
        let formatted = match bib.format(key) {
            Some(f) => f,
            None => {
                // A cited key with no entry is a broken citation — but only flag it
                // when a bibliography exists (else every cite would warn before one
                // is set up; the missing-file case is its own warning).
                if !bib.is_empty() {
                    let (file, line) = key_loc.get(key).cloned().unwrap_or((None, None));
                    let w =
                        Warning::new(format!("broken citation: @{key} (not in the bibliography)"));
                    warnings.push(match line {
                        Some(l) => w.at(file, l),
                        None => w,
                    });
                }
                format!("<code>{}</code>", esc(key))
            }
        };
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
    warnings
}

/// Whether a block is a manual heading (`<h1>`…`<h6>`) whose visible text is exactly
/// "References" or "Bibliography" (case-insensitive). Such a heading means the author
/// is placing the reference list themselves, so the auto section drops its own
/// `<h2>References</h2>` to avoid a duplicate heading. Matches only a heading block
/// (not, say, a paragraph that merely mentions "references").
fn is_manual_references_heading(html: &str) -> bool {
    let t = html.trim_start();
    // Must open with an <h1>..<h6> tag.
    let bytes = t.as_bytes();
    if bytes.len() < 4
        || bytes[0] != b'<'
        || (bytes[1] | 0x20) != b'h'
        || !bytes[2].is_ascii_digit()
    {
        return false;
    }
    // Strip every tag, leaving the text content; then compare case-insensitively.
    let mut text = String::new();
    let mut in_tag = false;
    for c in t.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text.trim().to_ascii_lowercase();
    text == "references" || text == "bibliography"
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
    // Once a scan from some `[` finds no `]` to its right, no later `[` can either
    // (the remaining text only shrinks), so stop re-scanning to the end. Without
    // this, a run of N unmatched `[` is O(N^2) (one full scan per `[`).
    let mut no_close = false;
    while i < chars.len() {
        if chars[i] == '[' && !no_close {
            match chars[i + 1..].iter().position(|&c| c == ']') {
                Some(close) => {
                    let inner = &chars[i + 1..i + 1 + close];
                    if inner.contains(&'@') {
                        let inner: String = inner.iter().collect();
                        out.push_str(&render_citation_group(&inner, cite_key, xrefs));
                        i += close + 2;
                        continue;
                    }
                }
                None => no_close = true,
            }
        } else if chars[i] == '@'
            && let Some((label, anchor, len)) = parse_xref(&chars[i..])
        {
            // A locally-resolved number renders "Figure&nbsp;3". An anchor not in
            // this document's registry may live on another page: emit it with a
            // `data-qmd-xref` marker so a site can resolve it to that page (and its
            // number); if nothing resolves it, it degrades to a bare-label link.
            out.push_str(&xref_anchor_link(&anchor, label, xrefs));
            i += len;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Characters allowed in a citation key. BibTeX keys permit far more than
/// alphanumerics (e.g. `smith.2020`, `doe+roe`, `path/key`); the reference parser
/// must accept the same set the bib parser does, or `[@smith.2020]` truncates to
/// `smith` and falsely warns "broken citation".
fn is_cite_key_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_' | ':' | '.' | '+' | '/')
}

/// A cross-reference link to `anchor`, labelled by kind. A locally-resolved number
/// renders "Figure&nbsp;3"; an anchor unknown to this document's registry emits a
/// `data-qmd-xref` marker (so a site can resolve it cross-page) and degrades to a
/// bare-label link. Shared by the bracketed (`[@fig-x]`) and bare (`@fig-x`) paths.
fn xref_anchor_link(anchor: &str, label: &str, xrefs: &HashMap<String, String>) -> String {
    let (text, marker) = match xrefs.get(anchor) {
        Some(n) => (format!("{label}&nbsp;{n}"), String::new()),
        None => (
            label.to_string(),
            format!(" data-qmd-xref=\"{}\"", esc(anchor)),
        ),
    };
    format!(
        "<a href=\"#{}\" class=\"qmd-xref\"{marker}>{text}</a>",
        esc(anchor)
    )
}

/// If `key` is a cross-reference key (`fig-x`, `tbl-x`, …), render it as a cross-ref
/// link (so `[@fig-x]` is a cross-ref, not a citation). `None` for ordinary keys.
fn xref_link(key: &str, xrefs: &HashMap<String, String>) -> Option<String> {
    let (prefix, ident) = key.split_once('-')?;
    let label = xref_label(prefix)?;
    if ident.is_empty() {
        return None;
    }
    Some(xref_anchor_link(key, label, xrefs))
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

/// Render `@a; @b, p. 5` style citation group content into `[1, 2, p. 5]`. A
/// cross-reference key inside the brackets (`[@fig-x]`) renders as a cross-ref link,
/// not a citation.
fn render_citation_group(
    inner: &str,
    cite_key: &mut impl FnMut(&str) -> usize,
    xrefs: &HashMap<String, String>,
) -> String {
    let mut rendered: Vec<String> = Vec::new();
    for item in inner.split(';') {
        let item = item.trim().trim_start_matches('-'); // `-@key` suppresses author (n/a for numeric)
        let Some(at) = item.find('@') else { continue };
        let after = &item[at + 1..];
        let key: String = after.chars().take_while(|&c| is_cite_key_char(c)).collect();
        if key.is_empty() {
            continue;
        }
        // A cross-reference key (`fig-`, `tbl-`, …) is a cross-ref, not a citation.
        if let Some(link) = xref_link(&key, xrefs) {
            rendered.push(link);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_text_leaves_unmatched_and_non_citation_brackets_literal() {
        let xrefs = HashMap::new();
        let mut key = |_: &str| 1usize;
        // A run of '[' with no closing ']' is emitted verbatim (this is also the
        // O(n^2)-pathological input the scan must not choke on).
        assert_eq!(
            rewrite_text("[[[[ no close here", &mut key, &xrefs),
            "[[[[ no close here"
        );
        // A bracket group without '@' is not a citation; the brackets stay.
        assert_eq!(
            rewrite_text("see [ref 12] here", &mut key, &xrefs),
            "see [ref 12] here"
        );
        // A real citation is still rewritten.
        let out = rewrite_text("see [@bishop2006pattern]", &mut key, &xrefs);
        assert!(
            out.contains("<a") && !out.contains("[@"),
            "citation not rewritten: {out}"
        );
    }
}
