//! Cross-page search index: every page's title + its anchored headings, built
//! once at discovery and inlined as `window.QMD_SEARCH_INDEX` so the Cmd-K
//! palette searches the whole book/site (not just the current page). Heading
//! level only — full-text search is a separate, heavier feature. `use super::*`
//! reaches Page + the render entry point.

use super::*;

/// Build the inlinable JSON index for `pages`: one `{u,p,i,l,t}` object per page
/// title and per anchored heading (`u`rl, `p`age title, anchor `i`d, `l`evel,
/// heading `t`ext). Renders each page's markdown once (no code execution) so the
/// anchor ids exactly match what the served pages emit.
pub(super) fn build_index_json(pages: &[Page]) -> String {
    let mut out = String::from("[");
    let mut first = true;
    for page in pages {
        let Ok(src) = std::fs::read_to_string(&page.input) else {
            continue;
        };
        let base = page.input.parent().unwrap_or_else(|| Path::new("."));
        let doc = render::render_document_with_includes(&src, base);
        let page_title = page
            .title
            .clone()
            .or(doc.title)
            .unwrap_or_else(|| page.url.clone());

        // The page itself (jump to its top), then each anchored heading.
        let mut push = |id: &str, level: u8, title: &str| {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str(&format!(
                "{{\"u\":\"{}\",\"p\":\"{}\",\"i\":\"{}\",\"l\":{},\"t\":\"{}\"}}",
                json_str(&page.url),
                json_str(&page_title),
                json_str(id),
                level,
                json_str(title),
            ));
        };
        push("", 0, &page_title);
        let body: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
        for (level, id, text) in headings_in(&body) {
            if !text.is_empty() {
                push(&id, level, &text);
            }
        }
    }
    out.push(']');
    out
}

/// Scan rendered HTML for `<h1..6 id="…">text</hN>`, returning `(level, id, text)`
/// for each anchored heading (the title block's `<h1 class="title">` has no id,
/// so it is skipped — the page-title entry covers it).
fn headings_in(html: &str) -> Vec<(u8, String, String)> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(p) = rest.find("<h") {
        rest = &rest[p..];
        let level = rest
            .as_bytes()
            .get(2)
            .map(|b| b.wrapping_sub(b'0'))
            .filter(|l| (1..=6).contains(l));
        let Some(level) = level else {
            rest = &rest[2..];
            continue;
        };
        let Some(gt) = rest.find('>') else { break };
        let open_tag = &rest[..gt];
        let id = open_tag
            .split_once("id=\"")
            .and_then(|(_, a)| a.split_once('"').map(|(id, _)| id.to_string()));
        let close = format!("</h{level}>");
        let body = &rest[gt + 1..];
        let Some(end) = body.find(&close) else {
            rest = body;
            continue;
        };
        if let Some(id) = id {
            out.push((level, id, strip_tags(&body[..end])));
        }
        rest = &body[end + close.len()..];
    }
    out
}

/// Plain text from a heading's inner HTML: drop tags, decode the few entities the
/// renderer emits, collapse whitespace.
fn strip_tags(html: &str) -> String {
    let mut s = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => s.push(c),
            _ => {}
        }
    }
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escape a string for a JSON value inlined inside a `<script>` (so `</script>`
/// in a heading can't break out, and control chars stay valid JSON).
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"), // neutralize a stray </script>
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
