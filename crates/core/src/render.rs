//! Parse `.qmd` source with comrak (sourcepos-aware) and emit our own HTML.
//!
//! We deliberately do not use comrak's built-in HTML formatter: every
//! top-level AST node is treated as a "block" and gets its own root element
//! carrying `data-block-id` and `data-sourcepos`, which the dev server later
//! keys off for incremental block-swap and click-to-source.

use crate::includes::LineOrigin;
use comrak::nodes::{AstNode, ListType, NodeList, NodeValue, TableAlignment};
use comrak::{Arena, Options, parse_document};
use std::collections::HashMap;
use std::path::Path;

/// One top-level block: a stable id, its source position, and its HTML.
#[derive(Debug, Clone)]
pub struct Block {
    /// Content-hash id (`b-<hex>`), with a positional tiebreak (`-N`) for duplicates.
    pub id: String,
    /// Sourcepos as `startLine:startCol-endLine:endCol`, relative to `source_file`.
    pub sourcepos: String,
    /// Origin file when the block came from an `{{< include >}}`d file
    /// (relative to the primary document's directory); `None` for the primary
    /// document. Drives click-to-source across files.
    pub source_file: Option<String>,
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
    // Smart typography (curly quotes, en/em dashes) to match Quarto/pandoc output.
    options.parse.smart = true;
    // sourcepos is tracked on AST nodes during parsing; `render.sourcepos`
    // only affects comrak's own formatter, which we don't use.
    options
}

/// Parse `src` into ordered top-level blocks with stable ids + sourcepos.
/// Does not resolve `{{< include >}}` (use [`render_document_with_includes`]).
pub fn render_document(src: &str) -> RenderedDoc {
    render_internal(src, None)
}

/// Like [`render_document`], but first expands `{{< include >}}` shortcodes
/// relative to `base_dir`, mapping each block back to its origin file.
pub fn render_document_with_includes(src: &str, base_dir: &Path) -> RenderedDoc {
    let (expanded, origins) = crate::includes::resolve(src, base_dir);
    render_internal(&expanded, Some(&origins))
}

/// Core render. When `origins` is provided (post-include expansion), each
/// block's sourcepos and `source_file` are translated back to the originating
/// file via the line-level source map.
fn render_internal(src: &str, origins: Option<&[LineOrigin]>) -> RenderedDoc {
    let arena = Arena::new();
    let options = parse_options();
    // Quarto fenced divs (`:::`) aren't CommonMark. Record their spans first,
    // then strip the fence markers in a line-preserving pass so sourcepos line
    // numbers stay exact and the inner content parses as normal blocks. The
    // recorded spans are used afterwards to wrap blocks back up as callouts etc.
    let spans = scan_div_spans(src);
    let processed = preprocess(src);
    let root = parse_document(&arena, &processed, &options);

    let lines: Vec<&str> = processed.lines().collect();
    let mut title: Option<String> = None;
    let mut flat: Vec<FlatBlock> = Vec::new();
    let mut id_counts: HashMap<String, u32> = HashMap::new();

    for node in root.children() {
        let (buf_start, sourcepos, source_file, block_src, is_paragraph) = {
            let data = node.data.borrow();
            if let NodeValue::FrontMatter(fm) = &data.value {
                title = extract_title(fm);
                continue;
            }
            let sp = data.sourcepos;
            // Translate the buffer line range back to the originating file/line.
            let (file, start_line) = map_origin(origins, sp.start.line);
            let (_, end_line) = map_origin(origins, sp.end.line);
            let sourcepos = format!(
                "{}:{}-{}:{}",
                start_line, sp.start.column, end_line, sp.end.column
            );
            let is_paragraph = matches!(data.value, NodeValue::Paragraph);
            (
                sp.start.line,
                sourcepos,
                file,
                slice_lines(&lines, sp.start.line, sp.end.line),
                is_paragraph,
            )
        };

        let id = make_id(&block_src, &mut id_counts);
        let file_attr = match &source_file {
            Some(f) => format!(" data-source-file=\"{}\"", escape_attr(f)),
            None => String::new(),
        };
        let attrs = format!(" data-block-id=\"{id}\" data-sourcepos=\"{sourcepos}\"{file_attr}");
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
        flat.push(FlatBlock {
            buf_start,
            block: Block { id, sourcepos, source_file, html },
        });
    }

    let blocks = group_divs(flat, &spans, origins, &mut id_counts);
    RenderedDoc { title, blocks }
}

/// A top-level block plus its line in the (post-include, post-blank) buffer,
/// used to group blocks back into fenced-div containers.
struct FlatBlock {
    buf_start: usize,
    block: Block,
}

/// Map a 1-based buffer line to its (origin file, origin line). Without a
/// source map, the file is the primary document and the line is unchanged.
fn map_origin(origins: Option<&[LineOrigin]>, buffer_line: usize) -> (Option<String>, usize) {
    match origins.and_then(|o| o.get(buffer_line.saturating_sub(1))) {
        Some(origin) => (origin.file.clone(), origin.line),
        None => (None, buffer_line),
    }
}

/// Render a complete, viewable HTML page (used by the one-shot CLI).
pub fn render_html_page(src: &str, fallback_title: &str) -> String {
    page_from_doc(&render_document(src), fallback_title)
}

/// Like [`render_html_page`], resolving `{{< include >}}` relative to `base_dir`.
pub fn render_html_page_with_includes(src: &str, base_dir: &Path, fallback_title: &str) -> String {
    page_from_doc(&render_document_with_includes(src, base_dir), fallback_title)
}

/// Self-contained KaTeX stylesheet (fonts inlined as data URIs at build time).
const KATEX_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/katex-inlined.css"));

fn page_from_doc(doc: &RenderedDoc, fallback_title: &str) -> String {
    let title = doc.title.as_deref().unwrap_or(fallback_title);
    let mut t = String::new();
    escape_html(title, &mut t);
    let body = doc.body_html();
    // Only ship the (large) KaTeX stylesheet when the page actually has math.
    let katex_css = if body.contains("class=\"katex") {
        format!("<style>{KATEX_CSS}</style>")
    } else {
        String::new()
    };
    PAGE_TEMPLATE
        .replace("{{TITLE}}", &t)
        .replace("{{KATEX_CSS}}", &katex_css)
        .replace("{{BODY}}", &body)
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
        NodeValue::List(nl) => emit_list(node, nl, attrs, out),
        NodeValue::Item(_) => emit_item(node, false, out),
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

fn emit_list<'a>(node: &'a AstNode<'a>, nl: &NodeList, attrs: &str, out: &mut String) {
    let (tag, extra) = match nl.list_type {
        ListType::Bullet => ("ul", String::new()),
        ListType::Ordered if nl.start != 1 => ("ol", format!(" start=\"{}\"", nl.start)),
        ListType::Ordered => ("ol", String::new()),
    };
    out.push_str(&format!("<{tag}{attrs}{extra}>"));
    for item in node.children() {
        emit_item(item, nl.tight, out);
    }
    out.push_str(&format!("</{tag}>"));
}

/// In a tight list, an item's direct paragraph renders as bare inline content
/// (no `<p>`); in a loose list it keeps its `<p>`. Nested lists recurse with
/// their own tightness.
fn emit_item<'a>(item: &'a AstNode<'a>, tight: bool, out: &mut String) {
    out.push_str("<li>");
    for child in item.children() {
        let is_paragraph = matches!(child.data.borrow().value, NodeValue::Paragraph);
        if tight && is_paragraph {
            emit_children(child, out);
        } else {
            emit(child, "", out);
        }
    }
    out.push_str("</li>");
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
        if parse_fence(line.trim_start()).is_none() {
            out.push_str(line);
        }
    }
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A pandoc/Quarto fenced-div marker: 3+ colons, then nothing (close) or an
/// attribute block / bare class name (open).
enum Fence {
    /// Opening fence; carries the raw attribute string (without the braces).
    Open(String),
    /// Closing fence (bare colons).
    Close,
}

fn parse_fence(s: &str) -> Option<Fence> {
    let colons = s.chars().take_while(|&c| c == ':').count();
    if colons < 3 {
        return None;
    }
    let rest = s[colons..].trim();
    if rest.is_empty() {
        Some(Fence::Close)
    } else if let Some(inner) = rest.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        Some(Fence::Open(inner.trim().to_string()))
    } else if rest.chars().next().is_some_and(char::is_alphabetic) {
        // bare `::: classname` -> treat the first word as a class
        Some(Fence::Open(format!(".{}", rest.split_whitespace().next().unwrap_or(""))))
    } else {
        None
    }
}

/// A fenced-div span in buffer-line space (1-based, inclusive of the markers).
struct DivSpan {
    open: usize,
    close: usize,
    /// Raw attribute string from the opening fence (e.g. `.callout-note title="X"`).
    attrs: String,
}

/// Find all fenced-div spans (stack-based, so nesting is handled). Sorted so
/// that for a shared opening line the outermost (latest close) comes first.
fn scan_div_spans(src: &str) -> Vec<DivSpan> {
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut spans: Vec<DivSpan> = Vec::new();
    for (i, line) in src.lines().enumerate() {
        match parse_fence(line.trim_start()) {
            Some(Fence::Open(attrs)) => stack.push((i + 1, attrs)),
            Some(Fence::Close) => {
                if let Some((open, attrs)) = stack.pop() {
                    spans.push(DivSpan { open, close: i + 1, attrs });
                }
            }
            None => {}
        }
    }
    spans.sort_by_key(|s| (s.open, std::cmp::Reverse(s.close)));
    spans
}

/// Parsed fenced-div attributes.
#[derive(Default)]
struct DivAttrs {
    classes: Vec<String>,
    id: Option<String>,
    kv: Vec<(String, String)>,
}

impl DivAttrs {
    fn get(&self, key: &str) -> Option<&str> {
        self.kv.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    fn callout_kind(&self) -> Option<&str> {
        self.classes
            .iter()
            .find_map(|c| c.strip_prefix("callout-"))
    }
}

/// Parse a fenced-div attribute string: `.class`, `#id`, and `key=val`
/// (value optionally quoted), whitespace-separated.
fn parse_attrs(s: &str) -> DivAttrs {
    let mut attrs = DivAttrs::default();
    for tok in tokenize_attrs(s) {
        if let Some(c) = tok.strip_prefix('.') {
            attrs.classes.push(c.to_string());
        } else if let Some(i) = tok.strip_prefix('#') {
            attrs.id = Some(i.to_string());
        } else if let Some((k, v)) = tok.split_once('=') {
            attrs.kv.push((k.to_string(), v.trim_matches(['"', '\'']).to_string()));
        } else if !tok.is_empty() {
            attrs.classes.push(tok.to_string());
        }
    }
    attrs
}

/// Split on whitespace, but keep quoted values (e.g. `title="a b"`) together.
fn tokenize_attrs(s: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in s.chars() {
        match quote {
            Some(q) => {
                cur.push(ch);
                if ch == q {
                    quote = None;
                }
            }
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                cur.push(ch);
            }
            None if ch.is_whitespace() => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}

/// Group flat top-level blocks back into fenced-div container blocks (callouts,
/// layout grids, generic divs), honoring nesting. Blocks inside a div become a
/// single container block whose HTML embeds them (they keep their own ids and
/// sourcepos, so click-to-source still works inside).
fn group_divs(
    flat: Vec<FlatBlock>,
    spans: &[DivSpan],
    origins: Option<&[LineOrigin]>,
    counts: &mut HashMap<String, u32>,
) -> Vec<Block> {
    struct Open<'a> {
        span: &'a DivSpan,
        inner: Vec<Block>,
    }
    let mut result: Vec<Block> = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut span_idx = 0;

    let push_block = |stack: &mut Vec<Open>, result: &mut Vec<Block>, b: Block| {
        match stack.last_mut() {
            Some(top) => top.inner.push(b),
            None => result.push(b),
        }
    };

    for (i, fb) in flat.iter().enumerate() {
        // Open every span that starts before this block and contains it.
        while span_idx < spans.len()
            && spans[span_idx].open < fb.buf_start
            && spans[span_idx].close > fb.buf_start
        {
            stack.push(Open { span: &spans[span_idx], inner: Vec::new() });
            span_idx += 1;
        }
        // Skip any spans that contain no blocks (degenerate/empty divs).
        while span_idx < spans.len() && spans[span_idx].close < fb.buf_start {
            span_idx += 1;
        }

        push_block(&mut stack, &mut result, fb.block.clone());

        // Close spans that end before the next block begins (innermost first).
        let next_start = flat.get(i + 1).map(|n| n.buf_start).unwrap_or(usize::MAX);
        while let Some(top) = stack.last() {
            if top.span.close < next_start {
                let done = stack.pop().unwrap();
                let container = build_container(done.span, done.inner, origins, counts);
                push_block(&mut stack, &mut result, container);
            } else {
                break;
            }
        }
    }
    // Close anything still open (e.g. unterminated div at EOF).
    while let Some(done) = stack.pop() {
        let container = build_container(done.span, done.inner, origins, counts);
        push_block(&mut stack, &mut result, container);
    }
    result
}

/// Render one fenced div as a container block: callouts, layout grids, or a
/// generic class div.
fn build_container(
    span: &DivSpan,
    mut inner: Vec<Block>,
    origins: Option<&[LineOrigin]>,
    counts: &mut HashMap<String, u32>,
) -> Block {
    let attrs = parse_attrs(&span.attrs);
    let id = make_id(&format!("div:{}", span.attrs), counts);
    let (file, open_line) = map_origin(origins, span.open);
    let (_, close_line) = map_origin(origins, span.close);
    let sourcepos = format!("{open_line}:1-{close_line}:3");
    let file_attr = match &file {
        Some(f) => format!(" data-source-file=\"{}\"", escape_attr(f)),
        None => String::new(),
    };
    let data = format!(" data-block-id=\"{id}\" data-sourcepos=\"{sourcepos}\"{file_attr}");

    let html = if let Some(kind) = attrs.callout_kind() {
        // Callout: use a `title="..."` attr, else a leading heading, else the kind.
        let title = match attrs.get("title") {
            Some(t) => html_escape(t),
            None if inner.first().is_some_and(|b| is_heading(&b.html)) => {
                strip_tags(&inner.remove(0).html)
            }
            None => capitalize(kind),
        };
        let body: String = inner.iter().map(|b| b.html.as_str()).collect();
        format!(
            "<div class=\"callout callout-{kind}\"{data}><div class=\"callout-title\">{title}</div><div class=\"callout-body\">{body}</div></div>"
        )
    } else if let Some(ncol) = attrs.get("layout-ncol").and_then(|n| n.parse::<u32>().ok()) {
        let body: String = inner.iter().map(|b| b.html.as_str()).collect();
        format!(
            "<div class=\"qmd-layout\" style=\"display:grid;grid-template-columns:repeat({ncol},minmax(0,1fr));gap:1rem\"{data}>{body}</div>"
        )
    } else {
        let mut class = attrs.classes.join(" ");
        if class.is_empty() {
            class.push_str("qmd-div");
        }
        let id_attr = match &attrs.id {
            Some(i) => format!(" id=\"{}\"", escape_attr(i)),
            None => String::new(),
        };
        let body: String = inner.iter().map(|b| b.html.as_str()).collect();
        format!("<div class=\"{class}\"{id_attr}{data}>{body}</div>")
    };

    Block { id, sourcepos, source_file: file, html }
}

fn is_heading(html: &str) -> bool {
    html.starts_with("<h") && html.as_bytes().get(2).is_some_and(u8::is_ascii_digit)
}

/// Strip HTML tags, returning the visible text (used for callout titles).
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn html_escape(s: &str) -> String {
    let mut out = String::new();
    escape_html(s, &mut out);
    out
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
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
{{KATEX_CSS}}
<style>
  body { max-width: 46rem; margin: 2rem auto; padding: 0 1rem;
         font: 17px/1.7 ui-serif, Georgia, "Times New Roman", serif; color: #1a1a1a; }
  h1, h2, h3, h4 { font-family: ui-sans-serif, system-ui, sans-serif; line-height: 1.25; }
  pre { background: #f5f5f5; padding: 1rem; border-radius: 6px; overflow: auto; font-size: .9em; }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
  blockquote { border-left: 3px solid #ddd; margin: 0 0 1rem; padding-left: 1rem; color: #555; }
  img { max-width: 100%; }
  table { border-collapse: collapse; }
  th, td { border: 1px solid #e3e3e3; padding: .35rem .6rem; }
  thead th { border-bottom: 2px solid #ccc; }
  .callout { border: 1px solid #e0e0e0; border-left-width: 4px; border-radius: 5px;
             margin: 1rem 0; overflow: hidden; }
  .callout-title { font-family: ui-sans-serif, system-ui, sans-serif; font-weight: 600;
                   padding: .5rem .9rem; background: #f6f6f6; }
  .callout-body { padding: .3rem .9rem; }
  .callout-body > :first-child { margin-top: .4rem; }
  .callout-note { border-left-color: #4c8dff; } .callout-note .callout-title { background: #eaf1ff; }
  .callout-tip { border-left-color: #2bb673; } .callout-tip .callout-title { background: #e7f7ef; }
  .callout-warning { border-left-color: #e0a800; } .callout-warning .callout-title { background: #fdf6e3; }
  .callout-important { border-left-color: #e0566b; } .callout-important .callout-title { background: #fdecef; }
  .callout-caution { border-left-color: #e8730c; } .callout-caution .callout-title { background: #fdefe3; }
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
    fn callout_wraps_content_using_leading_heading_as_title() {
        let doc = render_document("::: {.callout-note}\n## My Note\n\nBody text.\n:::\n");
        assert_eq!(doc.blocks.len(), 1, "the callout is one container block");
        let h = &doc.blocks[0].html;
        assert!(h.contains("class=\"callout callout-note\""), "got: {h}");
        assert!(h.contains("<div class=\"callout-title\">My Note</div>"), "got: {h}");
        assert!(!doc.body_html().contains(":::"));
        // inner content keeps its own sourcepos so click-to-source still works.
        assert!(h.contains("<p data-block-id"), "inner block lost its id: {h}");
        assert!(h.contains("Body text."));
    }

    #[test]
    fn callout_uses_explicit_title_and_default_title() {
        let titled = render_document("::: {.callout-tip title=\"Pro tip\"}\nDo this.\n:::\n");
        assert!(titled.blocks[0].html.contains("callout-tip"));
        assert!(titled.blocks[0].html.contains(">Pro tip</div>"), "got: {}", titled.blocks[0].html);

        let bare = render_document("::: {.callout-warning}\nBe careful.\n:::\n");
        assert!(bare.blocks[0].html.contains(">Warning</div>"), "got: {}", bare.blocks[0].html);
    }

    #[test]
    fn layout_ncol_div_becomes_grid() {
        let doc = render_document("::: {layout-ncol=2}\n![](a.png)\n\n![](b.png)\n:::\n");
        assert_eq!(doc.blocks.len(), 1);
        let h = &doc.blocks[0].html;
        assert!(h.contains("qmd-layout"), "got: {h}");
        assert!(h.contains("repeat(2,"), "got: {h}");
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

    // --- edge cases / robustness ---

    #[test]
    fn empty_and_whitespace_inputs_do_not_panic() {
        assert!(render_document("").blocks.is_empty());
        assert!(render_document("   \n\n\t\n").blocks.is_empty());
    }

    #[test]
    fn front_matter_only_yields_no_blocks() {
        let doc = render_document("---\ntitle: Only Meta\n---\n");
        assert_eq!(doc.title.as_deref(), Some("Only Meta"));
        assert!(doc.blocks.is_empty());
    }

    #[test]
    fn nested_lists_render_with_nesting() {
        let doc = render_document("- a\n    - b\n    - c\n- d\n");
        let h = &doc.blocks[0].html;
        assert!(h.starts_with("<ul "), "got: {h}");
        assert!(h.contains("<li>a<ul><li>b</li><li>c</li></ul></li>"), "got: {h}");
    }

    #[test]
    fn ordered_list_start_attribute_preserved() {
        let doc = render_document("3. third\n4. fourth\n");
        assert!(doc.blocks[0].html.starts_with("<ol "));
        assert!(doc.blocks[0].html.contains("start=\"3\""), "got: {}", doc.blocks[0].html);
    }

    #[test]
    fn links_images_and_blockquotes_render() {
        let link = render_document("[text](https://example.com \"t\")\n");
        assert!(link.blocks[0].html.contains("<a href=\"https://example.com\" title=\"t\">text</a>"));

        let img = render_document("![alt text](/img.png)\n");
        assert!(img.blocks[0].html.contains("<img src=\"/img.png\" alt=\"alt text\" />"));

        let quote = render_document("> quoted line\n");
        assert!(quote.blocks[0].html.starts_with("<blockquote "));
        assert!(quote.blocks[0].html.contains("quoted line"));
    }

    #[test]
    fn attribute_values_are_escaped() {
        let doc = render_document("[x](https://e.com?a=1&b=\"2\")\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("&amp;"), "ampersand should be escaped in href: {h}");
        assert!(h.contains("&quot;"), "quote should be escaped in href: {h}");
    }

    #[test]
    fn unicode_text_is_preserved() {
        let doc = render_document("naïve café — ψ ∈ ℂ, Σ over 𝒩\n");
        assert!(doc.blocks[0].html.contains("naïve café — ψ ∈ ℂ, Σ over 𝒩"));
    }

    #[test]
    fn special_chars_in_inline_code_are_escaped_not_interpreted() {
        let doc = render_document("use `a < b && c` here\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("<code>a &lt; b &amp;&amp; c</code>"), "got: {h}");
    }
}
