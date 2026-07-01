//! Cross-page **full-text** search index: every page's title + each anchored
//! heading, each carrying the plain-text body of its section so Cmd-K matches
//! prose, not just headings. Built once at discovery; written to `search-index.js`
//! and lazy-loaded by the client on first open (so it never bloats every page).
//! `use super::*` reaches Page + the render entry point.

use super::*;

/// Per-section body text is capped so one long section (or a big code listing)
/// can't blow up the index; matches/snippets come from the section's start.
const BODY_CAP: usize = 1500;

/// Build the JSON index for `pages`: one `{u,p,i,l,t,b}` object per page title and
/// per anchored heading — `u`rl, `p`age title, anchor `i`d, `l`evel, heading
/// `t`ext, and the section `b`ody text. Renders each page's markdown once (no code
/// execution) so the anchor ids match what the served pages emit.
pub(super) fn build_index_json(pages: &[Page]) -> String {
    let mut out = String::from("[");
    let mut first = true;
    for page in pages {
        // The author's own 404 page (output URL `404.html`) is navigation chrome, not
        // content: keep it out of the full-text index so a search never surfaces it.
        if page.url == "404.html" {
            continue;
        }
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

        let mut push = |id: &str, level: u8, title: &str, body: &str| {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str(&format!(
                "{{\"u\":\"{}\",\"p\":\"{}\",\"i\":\"{}\",\"l\":{},\"t\":\"{}\",\"b\":\"{}\"}}",
                json_str(&page.url),
                json_str(&page_title),
                json_str(id),
                level,
                json_str(title),
                json_str(body),
            ));
        };

        let body: String = doc.blocks.iter().map(|b| b.html.as_str()).collect();
        let hs = headings_with_pos(&body);
        // The page itself: jump to its top; body = the intro before the first heading.
        let intro_end = hs.first().map(|h| h.3).unwrap_or(body.len());
        push("", 0, &page_title, &section_text(&body[..intro_end]));
        // Each anchored heading: body = text from its close to the next heading's open.
        for (idx, (level, id, title, _open, close_end)) in hs.iter().enumerate() {
            if title.is_empty() {
                continue;
            }
            let sec_end = hs.get(idx + 1).map(|n| n.3).unwrap_or(body.len());
            let sec_body = section_text(body.get(*close_end..sec_end).unwrap_or(""));
            push(id, *level, title, &sec_body);
        }
    }
    out.push(']');
    out
}

/// Scan rendered HTML for `<h1..6 id="…">text</hN>`, returning, per anchored
/// heading, `(level, id, text, open_byte, close_end_byte)` — the byte span lets
/// the caller slice each section's body (heading-close → next heading-open).
fn headings_with_pos(html: &str) -> Vec<(u8, String, String, usize, usize)> {
    let mut out = Vec::new();
    let mut pos = 0; // byte offset of `rest` within `html`
    let mut rest = html;
    while let Some(p) = rest.find("<h") {
        pos += p;
        rest = &rest[p..];
        let open_start = pos;
        let level = rest
            .as_bytes()
            .get(2)
            .map(|b| b.wrapping_sub(b'0'))
            .filter(|l| (1..=6).contains(l));
        let Some(level) = level else {
            pos += 2;
            rest = &rest[2..];
            continue;
        };
        let Some(gt) = rest.find('>') else { break };
        let open_tag = &rest[..gt];
        let id = open_tag
            .split_once("id=\"")
            .and_then(|(_, a)| a.split_once('"').map(|(id, _)| id.to_string()));
        let close = format!("</h{level}>");
        let inner = &rest[gt + 1..];
        let Some(end) = inner.find(&close) else {
            pos += gt + 1;
            rest = inner;
            continue;
        };
        let close_end = pos + gt + 1 + end + close.len();
        if let Some(id) = id {
            out.push((
                level,
                id,
                section_text(&inner[..end]),
                open_start,
                close_end,
            ));
        }
        let advance = gt + 1 + end + close.len();
        pos += advance;
        rest = &rest[advance..];
    }
    out
}

/// Plain text from inner HTML: drop tags, decode the few entities the renderer
/// emits, collapse whitespace, and cap the length (so one section can't dominate
/// the index).
fn section_text(html: &str) -> String {
    let mut s = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            // A space at every tag so text from adjacent blocks/inlines stays
            // word-separated (the trailing whitespace-collapse tidies the runs).
            '<' => {
                in_tag = true;
                s.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => s.push(c),
            _ => {}
        }
    }
    let text = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    match text.char_indices().nth(BODY_CAP) {
        Some((i, _)) => text[..i].to_string(),
        None => text,
    }
}

/// Escape a string for a JSON value inlined inside a `<script>` (so `</script>`
/// in content can't break out, and control chars stay valid JSON). Returns the
/// escaped body without surrounding quotes.
pub(super) fn json_str(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_text_separates_blocks_decodes_and_collapses() {
        let html = "<p>First.</p><p>Second &amp; third.</p>";
        assert_eq!(section_text(html), "First. Second & third.");
    }

    #[test]
    fn section_text_caps_length() {
        let long = format!("<p>{}</p>", "x ".repeat(2000));
        assert!(section_text(&long).chars().count() <= BODY_CAP);
    }

    #[test]
    fn headings_with_pos_yields_spans_for_full_text_sections() {
        let html = "<h2 id=\"a\">Alpha</h2><p>body of a</p><h3 id=\"b\">Beta</h3><p>body of b</p>";
        let hs = headings_with_pos(html);
        assert_eq!(hs.len(), 2);
        assert_eq!(
            (hs[0].0, hs[0].1.as_str(), hs[0].2.as_str()),
            (2, "a", "Alpha")
        );
        // The span between heading a's close and heading b's open is a's section.
        assert_eq!(section_text(&html[hs[0].4..hs[1].3]), "body of a");
    }
}
