//! Parse `.qmd` source with comrak (sourcepos-aware) and emit our own HTML.
//!
//! We deliberately do not use comrak's built-in HTML formatter: every
//! top-level AST node is treated as a "block" and gets its own root element
//! carrying `data-block-id` and `data-sourcepos`, which the dev server later
//! keys off for incremental block-swap and click-to-source.

use comrak::nodes::{AstNode, ListType, NodeValue};
use comrak::{Arena, Options, parse_document};
use std::collections::HashMap;

/// One top-level block: a stable id, its source position, and its HTML.
#[derive(Debug, Clone)]
pub struct Block {
    /// Content-hash id (`b-<hex>`), with a positional tiebreak (`-N`) for duplicates.
    pub id: String,
    /// comrak sourcepos as `startLine:startCol-endLine:endCol`.
    pub sourcepos: String,
    /// Rendered HTML for this block, root element carrying the data attributes.
    pub html: String,
}

/// A rendered document: its title (from front matter, if any) and ordered blocks.
#[derive(Debug, Clone)]
pub struct RenderedDoc {
    pub title: Option<String>,
    pub blocks: Vec<Block>,
}

impl RenderedDoc {
    /// Concatenated block HTML, one block per line.
    pub fn body_html(&self) -> String {
        let mut s = String::new();
        for b in &self.blocks {
            s.push_str(&b.html);
            s.push('\n');
        }
        s
    }
}

fn parse_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.front_matter_delimiter = Some("---".to_string());
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    // sourcepos is tracked on AST nodes during parsing; `render.sourcepos`
    // only affects comrak's own formatter, which we don't use.
    options
}

/// Parse `src` into ordered top-level blocks with stable ids + sourcepos.
pub fn render_document(src: &str) -> RenderedDoc {
    let arena = Arena::new();
    let options = parse_options();
    let root = parse_document(&arena, src, &options);

    let lines: Vec<&str> = src.lines().collect();
    let mut title: Option<String> = None;
    let mut blocks: Vec<Block> = Vec::new();
    let mut id_counts: HashMap<String, u32> = HashMap::new();

    for node in root.children() {
        let (sourcepos, block_src) = {
            let data = node.data.borrow();
            if let NodeValue::FrontMatter(fm) = &data.value {
                title = extract_title(fm);
                continue;
            }
            let sp = data.sourcepos;
            let sourcepos = format!(
                "{}:{}-{}:{}",
                sp.start.line, sp.start.column, sp.end.line, sp.end.column
            );
            (sourcepos, slice_lines(&lines, sp.start.line, sp.end.line))
        };

        let id = make_id(&block_src, &mut id_counts);
        let attrs = format!(" data-block-id=\"{id}\" data-sourcepos=\"{sourcepos}\"");
        let mut html = String::new();
        emit(node, &attrs, &mut html);
        blocks.push(Block { id, sourcepos, html });
    }

    RenderedDoc { title, blocks }
}

/// Render a complete, viewable HTML page (used by the one-shot CLI).
pub fn render_html_page(src: &str, fallback_title: &str) -> String {
    let doc = render_document(src);
    let title = doc.title.as_deref().unwrap_or(fallback_title);
    let mut t = String::new();
    escape_html(title, &mut t);
    PAGE_TEMPLATE
        .replace("{{TITLE}}", &t)
        .replace("{{BODY}}", &doc.body_html())
}

// --- emitter -------------------------------------------------------------

/// Emit `node`'s HTML, applying `attrs` to its root element (top-level only).
fn emit<'a>(node: &'a AstNode<'a>, attrs: &str, out: &mut String) {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Document => emit_children(node, out),
        NodeValue::FrontMatter(_) => {}
        NodeValue::Heading(h) => {
            let l = h.level;
            out.push_str(&format!("<h{l}{attrs}>"));
            emit_children(node, out);
            out.push_str(&format!("</h{l}>"));
        }
        NodeValue::Paragraph => {
            out.push_str(&format!("<p{attrs}>"));
            emit_children(node, out);
            out.push_str("</p>");
        }
        NodeValue::Text(t) => escape_html(t, out),
        NodeValue::SoftBreak => out.push('\n'),
        NodeValue::LineBreak => out.push_str("<br />\n"),
        NodeValue::Emph => wrap(node, "em", out),
        NodeValue::Strong => wrap(node, "strong", out),
        NodeValue::Strikethrough => wrap(node, "del", out),
        NodeValue::Code(c) => {
            out.push_str("<code>");
            escape_html(&c.literal, out);
            out.push_str("</code>");
        }
        NodeValue::CodeBlock(cb) => {
            let class = match code_lang(&cb.info) {
                Some(l) => format!(" class=\"language-{l}\""),
                None => String::new(),
            };
            out.push_str(&format!("<pre{attrs}><code{class}>"));
            escape_html(&cb.literal, out);
            out.push_str("</code></pre>");
        }
        NodeValue::HtmlBlock(hb) => {
            // Raw HTML block; wrap so it still carries block attrs for swapping.
            out.push_str(&format!("<div{attrs}>"));
            out.push_str(&hb.literal);
            out.push_str("</div>");
        }
        NodeValue::HtmlInline(h) => out.push_str(h),
        NodeValue::List(nl) => {
            let (tag, extra) = match nl.list_type {
                ListType::Bullet => ("ul", String::new()),
                ListType::Ordered if nl.start != 1 => ("ol", format!(" start=\"{}\"", nl.start)),
                ListType::Ordered => ("ol", String::new()),
            };
            out.push_str(&format!("<{tag}{attrs}{extra}>"));
            emit_children(node, out);
            out.push_str(&format!("</{tag}>"));
        }
        NodeValue::Item(_) => wrap(node, "li", out),
        NodeValue::BlockQuote => {
            out.push_str(&format!("<blockquote{attrs}>"));
            emit_children(node, out);
            out.push_str("</blockquote>");
        }
        NodeValue::ThematicBreak => out.push_str(&format!("<hr{attrs} />")),
        NodeValue::Link(l) => {
            out.push_str(&format!("<a href=\"{}\"", escape_attr(&l.url)));
            if !l.title.is_empty() {
                out.push_str(&format!(" title=\"{}\"", escape_attr(&l.title)));
            }
            out.push('>');
            emit_children(node, out);
            out.push_str("</a>");
        }
        NodeValue::Image(l) => {
            let mut alt = String::new();
            collect_text(node, &mut alt);
            out.push_str(&format!(
                "<img src=\"{}\" alt=\"{}\"",
                escape_attr(&l.url),
                escape_attr(&alt)
            ));
            if !l.title.is_empty() {
                out.push_str(&format!(" title=\"{}\"", escape_attr(&l.title)));
            }
            out.push_str(" />");
        }
        NodeValue::Table(_) => {
            out.push_str(&format!("<table{attrs}>"));
            emit_children(node, out);
            out.push_str("</table>");
        }
        NodeValue::TableRow(_) => wrap(node, "tr", out),
        NodeValue::TableCell => wrap(node, "td", out),
        // Unknown/unhandled wrappers degrade to their inner content.
        _ => emit_children(node, out),
    }
}

fn wrap<'a>(node: &'a AstNode<'a>, tag: &str, out: &mut String) {
    out.push('<');
    out.push_str(tag);
    out.push('>');
    emit_children(node, out);
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn emit_children<'a>(node: &'a AstNode<'a>, out: &mut String) {
    for c in node.children() {
        emit(c, "", out);
    }
}

fn collect_text<'a>(node: &'a AstNode<'a>, out: &mut String) {
    for c in node.children() {
        let recurse = {
            let d = c.data.borrow();
            match &d.value {
                NodeValue::Text(t) => {
                    out.push_str(t);
                    false
                }
                NodeValue::Code(code) => {
                    out.push_str(&code.literal);
                    false
                }
                _ => true,
            }
        };
        if recurse {
            collect_text(c, out);
        }
    }
}

// --- block ids -----------------------------------------------------------

/// Build a stable block id from its source text, with a positional tiebreak
/// so duplicate-content blocks still get distinct ids.
fn make_id(block_src: &str, counts: &mut HashMap<String, u32>) -> String {
    let hex = format!("{:016x}", fnv1a(block_src.trim()));
    let base = format!("b-{}", &hex[..12]);
    let n = counts.entry(base.clone()).or_insert(0);
    let id = if *n == 0 {
        base.clone()
    } else {
        format!("{base}-{n}")
    };
    *n += 1;
    id
}

/// 64-bit FNV-1a — a small, deterministic hash stable across runs and versions.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// --- helpers -------------------------------------------------------------

fn slice_lines(lines: &[&str], start: usize, end: usize) -> String {
    let s = start.saturating_sub(1);
    let e = end.min(lines.len());
    if s >= e {
        return String::new();
    }
    lines[s..e].join("\n")
}

/// Extract a `title:` from raw front matter. Lightweight scan, not a YAML parse.
fn extract_title(front_matter: &str) -> Option<String> {
    for line in front_matter.lines() {
        if let Some(rest) = line.trim().strip_prefix("title:") {
            let t = rest.trim().trim_matches(['"', '\'']).trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Language for a fenced block: `{python}`/`{.python}`/`{ojs}` -> "python"/"ojs",
/// plain ` ```rust ` -> "rust".
fn code_lang(info: &str) -> Option<String> {
    let info = info.trim();
    if info.is_empty() {
        return None;
    }
    let token = if let Some(inner) = info.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        inner.trim().trim_start_matches('.')
    } else {
        info
    };
    let lang = token.split_whitespace().next().unwrap_or("");
    (!lang.is_empty()).then(|| lang.to_string())
}

fn escape_html(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

fn escape_attr(s: &str) -> String {
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

const PAGE_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{{TITLE}}</title>
<style>
  body { max-width: 46rem; margin: 2rem auto; padding: 0 1rem;
         font: 17px/1.7 ui-serif, Georgia, "Times New Roman", serif; color: #1a1a1a; }
  h1, h2, h3, h4 { font-family: ui-sans-serif, system-ui, sans-serif; line-height: 1.25; }
  pre { background: #f5f5f5; padding: 1rem; border-radius: 6px; overflow: auto; font-size: .9em; }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
  blockquote { border-left: 3px solid #ddd; margin: 0 0 1rem; padding-left: 1rem; color: #555; }
  img { max-width: 100%; }
  [data-block-id] { scroll-margin-top: 1rem; }
  [data-block-id].qmd-hl { outline: 2px solid #4c8dff; outline-offset: 3px; border-radius: 3px; }
</style>
</head>
<body>
{{BODY}}
<script>
  // Phase 1 demo: click any block to see its source position in the console
  // (this previews the Phase 3 click-to-source feature).
  document.addEventListener('click', (e) => {
    const el = e.target.closest('[data-block-id]');
    document.querySelectorAll('.qmd-hl').forEach(n => n.classList.remove('qmd-hl'));
    if (!el) return;
    el.classList.add('qmd-hl');
    console.log('block', el.dataset.blockId, '@', el.dataset.sourcepos);
  });
</script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_and_paragraph_become_blocks() {
        let doc = render_document("# Title\n\nHello *world*.\n");
        assert_eq!(doc.blocks.len(), 2);
        assert!(doc.blocks[0].html.starts_with("<h1 "));
        assert!(doc.blocks[0].html.contains("data-sourcepos=\"1:1-"));
        assert!(doc.blocks[0].html.contains("data-block-id=\"b-"));
        assert!(doc.blocks[1].html.contains("<em>world</em>"));
    }

    #[test]
    fn ids_are_stable_across_runs_and_unique_for_duplicates() {
        let doc = render_document("Para.\n\nPara.\n");
        assert_eq!(doc.blocks.len(), 2);
        assert_ne!(doc.blocks[0].id, doc.blocks[1].id, "duplicate content must get a tiebreak");
        let again = render_document("Para.\n\nPara.\n");
        assert_eq!(doc.blocks[0].id, again.blocks[0].id, "ids must be stable across runs");
    }

    #[test]
    fn front_matter_title_extracted_and_not_a_block() {
        let doc = render_document("---\ntitle: \"My Post\"\nfoo: bar\n---\n\nBody.\n");
        assert_eq!(doc.title.as_deref(), Some("My Post"));
        assert_eq!(doc.blocks.len(), 1);
    }

    #[test]
    fn html_is_escaped_in_text() {
        let doc = render_document("a < b & c\n");
        assert!(doc.blocks[0].html.contains("a &lt; b &amp; c"));
    }

    #[test]
    fn qmd_code_cell_language_detected() {
        let doc = render_document("```{python}\nprint(1)\n```\n");
        assert!(doc.blocks[0].html.contains("<pre "));
        assert!(doc.blocks[0].html.contains("class=\"language-python\""));
    }
}
