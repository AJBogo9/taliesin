//! The comrak AST -> HTML emitter: the inline/leaf-node `emit` switch plus its
//! helpers (wrap, children, lists, tables, raw HTML blocks, text collection).
//! Split out of the render module; `use super::*` reaches the shared block
//! model, the comrak node types, and the html-escaping / attribute helpers.

use super::*;

/// Emit `node`'s HTML, applying `attrs` to its root element (top-level only).
pub(super) fn emit<'a>(node: &'a AstNode<'a>, attrs: &str, out: &mut String) {
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
        NodeValue::CodeBlock(cb) if raw_block_format(&cb.info).as_deref() == Some("html") => {
            // Pandoc/Quarto raw passthrough: ```{=html} ... ``` is raw *output*,
            // not a code listing, so its body is emitted verbatim (block data
            // attrs injected into the leading tag, like any other raw HTML block).
            emit_html_block(&cb.literal, attrs, out);
        }
        NodeValue::CodeBlock(cb) => {
            let lang = code_lang(&cb.info);
            // Quarto cells (```{lang}) carry leading `#| key: val` option lines; drop them.
            let is_cell = cb.info.trim_start().starts_with('{');
            let fold = is_cell.then(|| code_fold(&cb.literal)).flatten();
            let literal = if is_cell {
                strip_cell_options(&cb.literal)
            } else {
                cb.literal.clone()
            };
            if lang.as_deref() == Some("mermaid") {
                // Diagram source for client-side mermaid.js. No <code> element,
                // so it skips syntax highlighting and the copy button.
                out.push_str(&format!("<pre{attrs} class=\"mermaid\">"));
                escape_html(&literal, out);
                out.push_str("</pre>");
            } else {
                let class = match &lang {
                    Some(l) => format!(" class=\"language-{l}\""),
                    None => String::new(),
                };
                // `code-fold` wraps the listing in a <details>; the block data
                // attrs move to the <details> so click-to-source still keys off it.
                let highlighted = crate::highlight::highlight(&literal, lang.as_deref());
                if let Some((open, summary)) = &fold {
                    let open_attr = if *open { " open" } else { "" };
                    out.push_str(&format!(
                        "<details{attrs} class=\"qmd-code-fold\"{open_attr}><summary>{}</summary><pre><code{class}>{highlighted}</code></pre></details>",
                        html_escape(summary)
                    ));
                } else {
                    // `code-line-numbers` wraps each line so a deck can highlight /
                    // step through them; absent, the code block is emitted unchanged.
                    match code_line_numbers(&cb.info, &cb.literal) {
                        Some(spec) => out.push_str(&format!(
                            "<pre{attrs} data-code-lines=\"{}\"><code{class}>{}</code></pre>",
                            escape_attr(&spec),
                            wrap_code_lines(&highlighted),
                        )),
                        None => out.push_str(&format!(
                            "<pre{attrs}><code{class}>{highlighted}</code></pre>"
                        )),
                    }
                }
            }
        }
        // Raw HTML in the body is passed through verbatim (not escaped): the
        // `.qmd` author is trusted. See the crate-level "Trust model" doc.
        NodeValue::HtmlBlock(hb) => emit_html_block(&hb.literal, attrs, out),
        NodeValue::HtmlInline(h) => out.push_str(h),
        NodeValue::Math(m) => out.push_str(&crate::math::render(&m.literal, m.display_math)),
        // `[^name]` reference → a superscript link to the gathered footnote section.
        NodeValue::FootnoteReference(r) => out.push_str(&format!(
            "<sup class=\"qmd-fnref\" id=\"fnref-{name}-{rn}\"><a href=\"#fn-{name}\">{ix}</a></sup>",
            name = escape_attr(&r.name),
            rn = r.ref_num,
            ix = r.ix,
        )),
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

/// A footnote definition as an `<li>` for the gathered footnotes `<ol>` (the render
/// loop collects these so they don't render in place; the `<ol>` auto-numbers them
/// in comrak's reference order). Carries a backlink to the first reference.
pub(crate) fn footnote_def_li<'a>(node: &'a AstNode<'a>, name: &str) -> String {
    let mut inner = String::new();
    emit_children(node, &mut inner);
    let n = escape_attr(name);
    format!(
        "<li id=\"fn-{n}\" class=\"qmd-fn\">{inner}<a class=\"qmd-fn-back\" href=\"#fnref-{n}-1\" aria-label=\"Back to content\">\u{21a9}\u{fe0e}</a></li>"
    )
}

/// The `code-line-numbers` spec for a code block: from a `{python}` cell option
/// (`#| code-line-numbers: "1|3-5"`) or a fenced attribute
/// (```` ```{.python code-line-numbers="1|3-5"} ````). `None` if absent. The spec
/// is `|`-separated steps, each a comma list of line numbers / `a-b` ranges /
/// `all` (e.g. `"1|3-5|all"`).
fn code_line_numbers(info: &str, literal: &str) -> Option<String> {
    if let Some(v) = cell_option(literal, "code-line-numbers") {
        return Some(v.trim_matches(['"', '\'']).to_string());
    }
    let rest = &info[info.find("code-line-numbers=")? + "code-line-numbers=".len()..];
    let mut chars = rest.chars();
    match chars.next()? {
        q @ ('"' | '\'') => rest[1..].find(q).map(|e| rest[1..1 + e].to_string()),
        _ => {
            let end = rest.find([' ', '}', '\t']).unwrap_or(rest.len());
            Some(rest[..end].to_string())
        }
    }
}

/// Wrap each source line of already-highlighted code HTML in `<span class="qhl-ln">`
/// so a deck can address individual lines. A highlight span left open across a
/// newline is closed at the line end and reopened at the next line's start, so each
/// line is self-contained. Lines are block-displayed (no trailing newline needed);
/// the copy button reads `innerText`, which still reconstructs the line breaks.
pub(crate) fn wrap_code_lines(html: &str) -> String {
    let mut lines: Vec<String> = vec![String::new()];
    let mut open: Vec<String> = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("</span>") {
            lines.last_mut().unwrap().push_str("</span>");
            open.pop();
            rest = after;
        } else if rest.starts_with("<span") {
            let end = rest.find('>').map(|e| e + 1).unwrap_or(rest.len());
            let (tag, after) = rest.split_at(end);
            lines.last_mut().unwrap().push_str(tag);
            open.push(tag.to_string());
            rest = after;
        } else if let Some(after) = rest.strip_prefix('\n') {
            let cur = lines.last_mut().unwrap();
            (0..open.len()).for_each(|_| cur.push_str("</span>"));
            lines.push(open.concat()); // reopen the still-open spans on the next line
            rest = after;
        } else {
            let n = rest.chars().next().unwrap().len_utf8();
            lines.last_mut().unwrap().push_str(&rest[..n]);
            rest = &rest[n..];
        }
    }
    // Drop trailing lines with no actual text (the source's final newline leaves a
    // line that is just the highlighter's closing tags).
    while lines.len() > 1 && !line_has_text(lines.last().unwrap()) {
        lines.pop();
    }
    lines
        .into_iter()
        .map(|l| format!("<span class=\"qhl-ln\">{l}</span>"))
        .collect()
}

/// Does an HTML line fragment contain any non-whitespace text outside of tags?
/// (Entities like `&lt;` count as text — they carry no literal `<`/`>`.)
/// Line-wrap the code inside a rendered `<pre><code>…</code></pre>` block (used for
/// magic-move blocks, which need addressable lines to morph between). Returns the
/// html unchanged if it isn't a code block or is already line-wrapped.
pub(crate) fn wrap_pre_lines(html: &str) -> String {
    if html.contains("class=\"qhl-ln\"") || !html.contains("<code") {
        return html.to_string();
    }
    let Some(cs) = html.find("<code") else {
        return html.to_string();
    };
    let Some(open_rel) = html[cs..].find('>') else {
        return html.to_string();
    };
    let open_end = cs + open_rel + 1;
    let Some(close) = html.rfind("</code>") else {
        return html.to_string();
    };
    format!(
        "{}{}{}",
        &html[..open_end],
        wrap_code_lines(&html[open_end..close]),
        &html[close..]
    )
}

fn line_has_text(s: &str) -> bool {
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag && !c.is_whitespace() => return true,
            _ => {}
        }
    }
    false
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

pub(crate) fn emit_children<'a>(node: &'a AstNode<'a>, out: &mut String) {
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
    if injectable && let Some(gt) = literal.find('>') {
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
