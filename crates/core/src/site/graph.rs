//! Project reference graph: nodes = pages, edges = a cross-page connection (a
//! `@sec-`/`@fig-`/… cross-reference, or a prose `.qmd`/`.html` link, from one page to
//! another). Built by a lightweight SOURCE scan at discovery (no render), so it works in
//! both the live preview and a static build; emitted as JSON for the `graph.js` client to
//! draw as an interactive force-directed map. Read-only: it navigates, never writes.
//! `use super::*` reaches Page, XrefTarget, the link helpers, includes.

use super::*;
use std::collections::HashSet;

/// The project's page-to-page reference graph as JSON: `{"nodes":[{"u":url,"t":title,
/// "c":true?}...],"edges":[{"s":from,"t":to}...]}` (`c` marks a chapter/book node). Empty
/// `{"nodes":[],"edges":[]}` for a single page or when nothing links across pages.
pub(super) fn reference_graph_json(
    pages: &[Page],
    xref_targets: &HashMap<String, XrefTarget>,
) -> String {
    let urls: HashSet<&str> = pages.iter().map(|p| p.url.as_str()).collect();
    // Deduped directed page→page edges, in discovery order for stable output.
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut edges: Vec<(String, String)> = Vec::new();
    for page in pages {
        let Ok(raw) = std::fs::read_to_string(&page.input) else {
            continue;
        };
        let base = page
            .input
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let (src, _) = crate::includes::resolve(&raw, base);
        for target in scan_outgoing(&src, page, xref_targets, &urls) {
            if target != page.url && seen.insert((page.url.clone(), target.clone())) {
                edges.push((page.url.clone(), target));
            }
        }
    }
    // Only include nodes that participate in at least one edge OR are the sole page — a
    // fully-disconnected page adds nothing to a *reference* graph and just clutters it.
    let connected: HashSet<&str> = edges
        .iter()
        .flat_map(|(s, t)| [s.as_str(), t.as_str()])
        .collect();
    let mut nodes = String::from("[");
    let mut first = true;
    for p in pages {
        if !connected.contains(p.url.as_str()) {
            continue;
        }
        if !first {
            nodes.push(',');
        }
        first = false;
        let title = p.title.as_deref().unwrap_or(&p.rel);
        nodes.push_str(&format!(
            "{{\"u\":{},\"t\":{}}}",
            json_str(&p.url),
            json_str(title)
        ));
    }
    nodes.push(']');
    let mut edges_json = String::from("[");
    for (i, (s, t)) in edges.iter().enumerate() {
        if i > 0 {
            edges_json.push(',');
        }
        edges_json.push_str(&format!("{{\"s\":{},\"t\":{}}}", json_str(s), json_str(t)));
    }
    edges_json.push(']');
    format!("{{\"nodes\":{nodes},\"edges\":{edges_json}}}")
}

/// The distinct OTHER-page urls that `page`'s source links to: a cross-page `@ref`
/// (resolved via `xref_targets`) or a prose `.qmd`/`.html` link (resolved against the
/// page registry). Front matter + fenced code are skipped, and a `@` mid-word
/// (`bob@host`) is ignored, mirroring the xref/citation scanners.
fn scan_outgoing(
    src: &str,
    page: &Page,
    xref_targets: &HashMap<String, XrefTarget>,
    urls: &HashSet<&str>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut in_front_matter = false;
    let mut in_code = false;
    for (i, line) in src.lines().enumerate() {
        let t = line.trim_start();
        if i == 0 && t == "---" {
            in_front_matter = true;
            continue;
        }
        if in_front_matter {
            in_front_matter = t != "---";
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        collect_xref_refs(line, xref_targets, &mut out);
        collect_link_targets(line, page, urls, &mut out);
    }
    out
}

/// Push the target page url of each cross-page `@<ref-anchor>` on `line`.
fn collect_xref_refs(
    line: &str,
    xref_targets: &HashMap<String, XrefTarget>,
    out: &mut Vec<String>,
) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' {
            // Word boundary: an `@` glued to a preceding word (`bob@host`) is not a ref.
            let boundary = i == 0
                || matches!(
                    bytes[i - 1],
                    b' ' | b'\t' | b'(' | b'[' | b'{' | b';' | b',' | b'-'
                );
            if boundary {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && is_key_byte(bytes[j]) {
                    j += 1;
                }
                // `.`/`:` are valid key chars, so a ref ending a sentence (`@sec-x.`)
                // over-reads; trim trailing sentence punctuation before the lookup.
                let anchor = line[start..j].trim_end_matches(['.', ',', ';', ':']);
                if is_ref_anchor(anchor)
                    && let Some(t) = xref_targets.get(anchor)
                {
                    out.push(t.url.clone());
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
}

/// Push the target page url of each markdown link `](path)` on `line` whose path
/// resolves to another page in the site (a `.qmd` source or a built `.html`).
fn collect_link_targets(line: &str, page: &Page, urls: &HashSet<&str>, out: &mut Vec<String>) {
    let mut rest = line;
    while let Some(open) = rest.find("](") {
        let after = &rest[open + 2..];
        let Some(close) = after.find(')') else { break };
        let raw = after[..close].split_whitespace().next().unwrap_or("");
        rest = &after[close + 1..];
        // Strip a #fragment / ?query and skip external / anchor-only / non-page links.
        let path = &raw[..raw.find(['#', '?']).unwrap_or(raw.len())];
        if path.is_empty() || path.starts_with('#') || path.contains("://") {
            continue;
        }
        let as_html = qmd_to_html(path);
        if let Some(url) = join_rel_in_root(&page.url, &as_html)
            && urls.contains(url.as_str())
        {
            out.push(url);
        }
    }
}

fn is_key_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.' | b'+' | b'/')
}

/// Minimal JSON string escaping (`"`, `\`, control chars) for the emitted graph.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tgt(url: &str) -> XrefTarget {
        XrefTarget {
            url: url.to_string(),
            number: String::new(),
        }
    }

    #[test]
    fn scan_finds_xref_and_link_edges_but_not_glued_at_or_code() {
        let mut xrefs = HashMap::new();
        xrefs.insert("fig-plot".to_string(), tgt("b.html"));
        let urls: HashSet<&str> = ["a.html", "b.html", "c.html"].into_iter().collect();
        let page = Page {
            input: std::path::PathBuf::from("a.qmd"),
            rel: "a.qmd".into(),
            url: "a.html".into(),
            title: Some("A".into()),
            date: None,
            description: None,
            authors: vec![],
            card_image: None,
            card_image_alt: None,
            categories: vec![],
            listings: vec![],
            about: None,
            hero: None,
            page_layout: None,
        };
        // `@fig-plot.` ends a sentence (trailing period must be trimmed, not read as
        // part of the anchor); `bob@host` is not a ref; a ref in a code fence is skipped.
        let src = "See @fig-plot. Also [chapter C](c.qmd).\n\nMail bob@host is not a ref.\n\n```\n@fig-plot in code is skipped\n```\n";
        let mut got = scan_outgoing(src, &page, &xrefs, &urls);
        got.sort();
        assert_eq!(got, vec!["b.html".to_string(), "c.html".to_string()]);
    }

    #[test]
    fn graph_json_drops_disconnected_pages() {
        // A page with no cross-page reference is omitted from a *reference* graph.
        let xrefs = HashMap::new();
        let pages: Vec<Page> = vec![];
        let json = reference_graph_json(&pages, &xrefs);
        assert_eq!(json, "{\"nodes\":[],\"edges\":[]}");
    }
}
