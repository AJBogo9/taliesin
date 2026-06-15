//! Parse `.qmd` source with comrak (sourcepos-aware) and emit our own HTML.
//!
//! We deliberately do not use comrak's built-in HTML formatter: every
//! top-level AST node is treated as a "block" and gets its own root element
//! carrying `data-block-id` and `data-sourcepos`, which the dev server later
//! keys off for incremental block-swap and click-to-source.

use comrak::nodes::{AstNode, ListType, NodeValue, TableAlignment};
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
    // Parse `$...$` (inline) and `$$...$$` (display) into Math nodes for KaTeX.
    options.extension.math_dollars = true;
    // sourcepos is tracked on AST nodes during parsing; `render.sourcepos`
    // only affects comrak's own formatter, which we don't use.
    options
}

/// Parse `src` into ordered top-level blocks with stable ids + sourcepos.
pub fn render_document(src: &str) -> RenderedDoc {
    let arena = Arena::new();
    let options = parse_options();
    // Quarto fenced divs (`:::`) aren't CommonMark; strip the fence markers in a
    // line-preserving pass so sourcepos line numbers stay exact and the inner
    // content parses as normal blocks. (Callout/layout styling is deferred.)
    let processed = preprocess(src);
    let root = parse_document(&arena, &processed, &options);

    let lines: Vec<&str> = processed.lines().collect();
    let mut title: Option<String> = None;
    let mut blocks: Vec<Block> = Vec::new();
    let mut id_counts: HashMap<String, u32> = HashMap::new();

    for node in root.children() {
        let (sourcepos, block_src, is_paragraph) = {
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
            let is_paragraph = matches!(data.value, NodeValue::Paragraph);
            (
                sourcepos,
                slice_lines(&lines, sp.start.line, sp.end.line),
                is_paragraph,
            )
        };

        let id = make_id(&block_src, &mut id_counts);
        let attrs = format!(" data-block-id=\"{id}\" data-sourcepos=\"{sourcepos}\"");
        let mut html = String::new();
        // Quarto/pandoc treat a bare `\begin{env}...\end{env}` block as display
        // math even without `$$`; comrak doesn't, so detect and render it here.
        if let Some(env) = is_paragraph.then(|| bare_math_env(&block_src)).flatten() {
            html.push_str(&format!("<div{attrs} class=\"qmd-math-block\">"));
            html.push_str(&crate::math::render(env, true));
            html.push_str("</div>");
        } else {
            emit(node, &attrs, &mut html);
        }
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
            // Quarto cells (```{lang}) carry leading `#| key: val` option lines; drop them.
            let is_cell = cb.info.trim_start().starts_with('{');
            let literal = if is_cell {
                strip_cell_options(&cb.literal)
            } else {
                cb.literal.clone()
            };
            out.push_str(&format!("<pre{attrs}><code{class}>"));
            escape_html(&literal, out);
            out.push_str("</code></pre>");
        }
        NodeValue::HtmlBlock(hb) => emit_html_block(&hb.literal, attrs, out),
        NodeValue::HtmlInline(h) => out.push_str(h),
        NodeValue::Math(m) => out.push_str(&crate::math::render(&m.literal, m.display_math)),
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
        NodeValue::Table(t) => emit_table(node, &t.alignments, attrs, out),
        // Rows/cells are emitted by emit_table; fall through harmlessly otherwise.
        NodeValue::TableRow(_) | NodeValue::TableCell => emit_children(node, out),
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

fn emit_table<'a>(node: &'a AstNode<'a>, aligns: &[TableAlignment], attrs: &str, out: &mut String) {
    out.push_str(&format!("<table{attrs}>"));
    let mut body_open = false;
    for row in node.children() {
        let is_header = matches!(row.data.borrow().value, NodeValue::TableRow(true));
        if is_header {
            out.push_str("<thead><tr>");
            emit_cells(row, aligns, "th", out);
            out.push_str("</tr></thead>");
        } else {
            if !body_open {
                out.push_str("<tbody>");
                body_open = true;
            }
            out.push_str("<tr>");
            emit_cells(row, aligns, "td", out);
            out.push_str("</tr>");
        }
    }
    if body_open {
        out.push_str("</tbody>");
    }
    out.push_str("</table>");
}

fn emit_cells<'a>(row: &'a AstNode<'a>, aligns: &[TableAlignment], tag: &str, out: &mut String) {
    for (i, cell) in row.children().enumerate() {
        let style = match aligns.get(i) {
            Some(TableAlignment::Left) => " style=\"text-align: left\"",
            Some(TableAlignment::Center) => " style=\"text-align: center\"",
            Some(TableAlignment::Right) => " style=\"text-align: right\"",
            _ => "",
        };
        out.push_str(&format!("<{tag}{style}>"));
        emit_children(cell, out);
        out.push_str(&format!("</{tag}>"));
    }
}

/// Emit a raw HTML block, injecting block `attrs` into its leading start tag
/// when one is present (e.g. `<div ...>`). Comments, closing tags, and other
/// fragments we can't safely annotate are emitted verbatim (no block id).
fn emit_html_block(literal: &str, attrs: &str, out: &mut String) {
    let lead = literal.trim_start();
    let injectable = !attrs.is_empty()
        && lead.starts_with('<')
        && !lead.starts_with("</")
        && !lead.starts_with("<!")
        && !lead.starts_with("<?");
    if injectable
        && let Some(gt) = literal.find('>')
    {
        let (open, rest) = literal.split_at(gt); // rest starts with '>'
        if let Some(open) = open.strip_suffix('/') {
            out.push_str(open);
            out.push_str(attrs);
            out.push('/');
        } else {
            out.push_str(open);
            out.push_str(attrs);
        }
        out.push_str(rest);
        return;
    }
    out.push_str(literal);
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

/// Blank out Quarto fenced-div markers (`::: {...}` / `:::`) without changing
/// the line count, so the inner content parses as ordinary blocks and every
/// other block's sourcepos line numbers stay valid against the original source.
fn preprocess(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for (i, line) in src.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if !is_fence_line(line.trim_start()) {
            out.push_str(line);
        }
    }
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A pandoc/Quarto fenced-div marker: 3+ colons, then nothing (close) or an
/// attribute block / class name (open).
fn is_fence_line(s: &str) -> bool {
    let colons = s.chars().take_while(|&c| c == ':').count();
    if colons < 3 {
        return false;
    }
    let rest = s[colons..].trim_start();
    rest.is_empty() || rest.starts_with('{') || rest.chars().next().is_some_and(char::is_alphabetic)
}

/// If a block's source is entirely a LaTeX math environment
/// (`\begin{env} ... \end{env}`), return it for display-math rendering.
fn bare_math_env(block_src: &str) -> Option<&str> {
    let t = block_src.trim();
    (t.starts_with("\\begin{") && t.contains("\\end{") && t.ends_with('}')).then_some(t)
}

/// Drop leading `#| key: val` option lines from a Quarto code cell.
fn strip_cell_options(literal: &str) -> String {
    let mut body = String::new();
    let mut skipping = true;
    for line in literal.lines() {
        if skipping && line.trim_start().starts_with("#|") {
            continue;
        }
        skipping = false;
        body.push_str(line);
        body.push('\n');
    }
    if !literal.ends_with('\n') {
        body.pop();
    }
    body
}

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
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css" crossorigin="anonymous" />
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

    #[test]
    fn table_uses_thead_th_and_tbody_td() {
        let doc = render_document("| A | B |\n|---|--:|\n| 1 | 2 |\n");
        let h = &doc.blocks[0].html;
        assert!(h.starts_with("<table "), "got: {h}");
        assert!(h.contains("<thead><tr><th>A</th><th"), "got: {h}");
        assert!(h.contains("<tbody><tr><td>1</td>"), "got: {h}");
        assert!(h.contains("text-align: right"), "alignment from |--:| missing: {h}");
    }

    #[test]
    fn fenced_divs_stripped_inner_content_kept() {
        let doc = render_document("::: {.callout-note}\n## Note\n\nBody.\n:::\n");
        assert_eq!(doc.blocks.len(), 2, "fence lines should not be blocks");
        assert!(doc.blocks[0].html.starts_with("<h2 "));
        assert!(!doc.body_html().contains(":::"));
        // line numbers preserved: the heading is on line 2 of the source.
        assert!(doc.blocks[0].html.contains("data-sourcepos=\"2:1-"));
    }

    #[test]
    fn cell_option_lines_are_dropped() {
        let doc = render_document("```{python}\n#| echo: false\n#| label: fig\nprint(1)\n```\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("print(1)"));
        assert!(!h.contains("#| echo"), "option lines should be stripped: {h}");
    }

    #[test]
    fn dollar_math_is_rendered_by_katex() {
        let doc = render_document("The value $x^2$ is positive.\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("katex"), "expected katex markup, got: {h}");
        assert!(!h.contains("$x^2$"), "raw dollar math should be consumed: {h}");
    }

    #[test]
    fn display_math_block_renders() {
        let doc = render_document("$$\n\\sum_{i=1}^n x_i\n$$\n");
        assert!(doc.body_html().contains("katex-display"), "got: {}", doc.body_html());
    }

    #[test]
    fn bare_latex_environment_renders_as_display_math() {
        let doc = render_document("\\begin{align*}\na &= b \\\\\nc &= d\n\\end{align*}\n");
        assert_eq!(doc.blocks.len(), 1);
        let h = &doc.blocks[0].html;
        // rendered as a display-math block (the raw TeX only survives inside
        // KaTeX's <annotation>, which is expected).
        assert!(h.contains("qmd-math-block"), "got: {h}");
        assert!(h.contains("katex-display"), "expected display math, got: {h}");
    }

    #[test]
    fn html_block_attrs_injected_into_leading_tag() {
        let doc = render_document("<div class=\"demo\">\nhi\n</div>\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("<div class=\"demo\" data-block-id="), "got: {h}");
        // the wrapper-div double-emit bug must not reappear
        assert!(!h.contains("<div data-block-id"), "should inject, not wrap: {h}");
    }
}
