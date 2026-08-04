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
            // Pandoc raw passthrough: ```{=html} ... ``` is raw *output*,
            // not a code listing, so its body is emitted verbatim (block data
            // attrs injected into the leading tag, like any other raw HTML block).
            emit_html_block(&cb.literal, attrs, out);
        }
        NodeValue::CodeBlock(cb) => {
            let lang = code_lang(&cb.info);
            // code cells (```{lang}) carry leading `#| key: val` option lines; drop them.
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
                // Mark a CELL's source listing, so the reader's "show/hide code" control
                // has something to target. `is_cell` was computed here already and thrown
                // away: nothing in the built HTML distinguished ```{python} from ```python,
                // and the preview's `data-tali-cell-state` is added at runtime by client.js,
                // so a built page carried no marker at all.
                //
                // The distinction is the whole point of the control. A plain fence is prose
                // the author wrote to be read; a cell's source is the computation behind an
                // output, which is exactly what per-cell `echo:` already governs. The reader
                // switch is that same axis, owned by the reader.
                //
                // Carries the language rather than being a bare flag, because the code
                // download (`.tali-repro`) groups a page's cells by it.
                let cell_attr = if is_cell {
                    format!(
                        " data-tali-cell=\"{}\"",
                        escape_attr(lang.as_deref().unwrap_or(""))
                    )
                } else {
                    String::new()
                };
                // `code-fold` wraps the listing in a <details>; the block data
                // attrs move to the <details> so click-to-source still keys off it.
                let highlighted = crate::highlight::highlight(&literal, lang.as_deref());
                // `::: {.debug}` marks its stepped cell `#| trace: true`; the marker rides
                // on the `<pre>` (folded or not) so `divs.rs`'s `is_traced_cell` can find
                // it without re-scanning the (already-stripped) source.
                let trace_attr = match cell_option(&cb.literal, "trace") {
                    Some("true") => " data-tali-trace=\"1\"",
                    _ => "",
                };
                // A traced `{js}` cell has no live `<script type="application/tali-js">`
                // (`mod.rs` routes it to this plain-source arm instead, so the panel
                // stays a single root element; see the comment there), so `debug.js` has
                // nothing to run unless the RUNNABLE source rides along too. It cannot
                // just re-read `<code>`'s highlighted text at the displayed line numbers:
                // the cursor needs `yield` rewritten to `yield __at(N, ...)`, which the
                // reader must never see, so the stamped text goes in a data attribute
                // instead of the visible listing. `stamp_yields` refuses (returns `None`)
                // whenever it cannot scan confidently; the ORIGINAL text ships in that
                // case, still valid JS, just with no line stamps, so the cursor stays
                // parked rather than the cell breaking.
                let js_src_attr = if lang.as_deref() == Some("js") && !trace_attr.is_empty() {
                    let stamped = stamp_yields(&literal).unwrap_or_else(|| literal.clone());
                    format!(" data-tali-js-src=\"{}\"", escape_attr(&stamped))
                } else {
                    String::new()
                };
                if let Some((open, summary)) = &fold {
                    let open_attr = if *open { " open" } else { "" };
                    out.push_str(&format!(
                        "<details{attrs}{cell_attr} class=\"tali-code-fold\"{open_attr}><summary>{}</summary><pre{trace_attr}{js_src_attr}><code{class}>{highlighted}</code></pre></details>",
                        html_escape(summary)
                    ));
                } else {
                    // `code-line-numbers` wraps each line so a deck can highlight /
                    // step through them; absent, the code block is emitted unchanged.
                    match code_line_numbers(&cb.info, &cb.literal) {
                        Some(spec) => out.push_str(&format!(
                            "<pre{attrs}{cell_attr}{trace_attr}{js_src_attr} data-code-lines=\"{}\"><code{class}>{}</code></pre>",
                            escape_attr(&spec),
                            wrap_code_lines(&highlighted),
                        )),
                        None => out.push_str(&format!(
                            "<pre{attrs}{cell_attr}{trace_attr}{js_src_attr}><code{class}>{highlighted}</code></pre>"
                        )),
                    }
                }
            }
        }
        // Raw HTML in the body is passed through verbatim (not escaped): the
        // `.tmd` author is trusted. See the crate-level "Trust model" doc.
        NodeValue::HtmlBlock(hb) => emit_html_block(&hb.literal, attrs, out),
        NodeValue::HtmlInline(h) => out.push_str(h),
        NodeValue::Math(m) => out.push_str(&crate::math::render(&m.literal, m.display_math)),
        // `[^name]` reference → a superscript link to the note. The link is a `doc-noteref`
        // (PA-M8) so AT announces it as a note reference, not a bare number. The note's own
        // text is spliced in right after this `<sup>` by the render loop (see
        // `footnote_sidenote`), which reconstructs this markup to find the splice point —
        // so the two must stay one format string.
        NodeValue::FootnoteReference(r) => {
            out.push_str(&footnote_ref_markup(&r.name, r.ref_num, r.ix))
        }
        NodeValue::List(nl) => emit_list(node, nl, attrs, out),
        NodeValue::Item(_) => emit_item(node, false, out),
        NodeValue::BlockQuote => {
            out.push_str(&format!("<blockquote{attrs}>"));
            emit_children(node, out);
            out.push_str("</blockquote>");
        }
        NodeValue::ThematicBreak => out.push_str(&format!("<hr{attrs} />")),
        NodeValue::Link(l) => {
            out.push_str(&format!(
                "<a href=\"{}\"",
                escape_attr(safe_url(&l.url, false))
            ));
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
                escape_attr(safe_url(&l.url, true)),
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

/// Neutralize script-bearing URL schemes in a markdown link/image destination.
/// Taliesin renders comrak's AST with raw-HTML passthrough, which also disables
/// comrak's own safe-mode URL filter, so this restores that safe default on the
/// markdown path. Relative paths, fragments, and the ordinary web schemes pass
/// through unchanged; a blocked scheme collapses to an empty string (an inert
/// `href`/`src`). `allow_data_image` additionally permits inline raster `data:image/*`
/// payloads (legitimate for `<img>`), which stay blocked for `<a href>`, where
/// `data:` is an XSS vector. The `.tmd` author can still emit any URL via raw HTML;
/// this only guards not-fully-authored markdown (an include, a third-party README).
pub(crate) fn safe_url(url: &str, allow_data_image: bool) -> &str {
    match url_scheme_lc(url) {
        // No scheme: relative path, absolute path, `#fragment`, `?query`, or a
        // protocol-relative `//host` URL. None can introduce script; keep as-is.
        None => url,
        Some(scheme) => match scheme.as_str() {
            "http" | "https" | "mailto" | "tel" | "ftp" => url,
            "data" if allow_data_image && is_safe_data_image(url) => url,
            _ => "",
        },
    }
}

/// The URL's scheme, lowercased, or `None` if it has none. ASCII whitespace and C0
/// control characters are skipped while scanning, mirroring how browsers strip them
/// before resolving a scheme, so `java\tscript:` is still recognized as `javascript`.
fn url_scheme_lc(url: &str) -> Option<String> {
    let mut scheme = String::new();
    for &b in url.as_bytes() {
        match b {
            b':' => return (!scheme.is_empty()).then_some(scheme),
            // path / query / fragment starts before any `:` → no scheme
            b'/' | b'?' | b'#' => return None,
            // browsers drop these before scheme resolution; ignore them
            b if b <= 0x20 => continue,
            b if b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.') => {
                // a scheme must begin with a letter
                if scheme.is_empty() && !b.is_ascii_alphabetic() {
                    return None;
                }
                scheme.push(b.to_ascii_lowercase() as char);
            }
            // any other byte cannot appear in a scheme → treat as schemeless
            _ => return None,
        }
    }
    None
}

/// Whether a `data:` URL is an inline *raster* image (`png`/`gif`/`jpeg`/`webp`/`avif`).
/// `data:image/svg+xml` is deliberately excluded: SVG can carry script. The prefix is
/// matched after stripping ASCII whitespace/control chars so a padded `data:\timage/…`
/// cannot slip past.
fn is_safe_data_image(url: &str) -> bool {
    let norm: String = url
        .bytes()
        .filter(|b| *b > 0x20)
        .map(|b| b.to_ascii_lowercase() as char)
        .collect();
    const RASTER: [&str; 6] = [
        "data:image/png",
        "data:image/gif",
        "data:image/jpeg",
        "data:image/jpg",
        "data:image/webp",
        "data:image/avif",
    ];
    RASTER.iter().any(|p| norm.starts_with(p))
}

/// A footnote definition as the margin sidenote spliced in right after its own
/// reference (owner ruling 2026-08-01: margin placement is the DEFAULT on a wide
/// screen, not a `footnotes:` knob). Returns the markup and whether anything had to be
/// flattened to produce it.
///
/// **Phrasing content only, and that is a hard constraint rather than a style choice.**
/// The note is spliced immediately after the `<sup>`, and a reference can sit anywhere
/// inline — in a paragraph, a heading, a table cell, a list item. A `<p>`, `<ul>` or
/// `<pre>` in that position makes the HTML parser close the enclosing element early,
/// which leaves the block with *two* root elements: that breaks the one-root-element
/// invariant every block carries, and with it the block swap that addresses a block by
/// id. So each paragraph child contributes its INLINE content (joined with `<br>`), and
/// any other child is flattened to its text with `flattened = true` so the caller can
/// warn rather than silently reshaping the author's note.
///
/// The sidenote is the locatable unit: it carries the definition's own `data-sourcepos`
/// (+ `data-source-file` when the definition came from an `{{< include >}}`d file) AND a
/// `data-block-id`, because client.js `locatable()` resolves a Ctrl-click with
/// `closest("[data-tali-src], [data-block-id]")`. Without the id the click walks up to
/// the *referencing* block and lands on that block's line — silently the wrong line,
/// which is worse than no-op. The id is namespaced `fn-…`, which cannot collide with a
/// content-hashed block id (`make_id` emits `b-…`).
pub(crate) fn footnote_sidenote<'a>(
    node: &'a AstNode<'a>,
    name: &str,
    ix: u32,
    sourcepos: &str,
    source_file: Option<&str>,
) -> (String, bool) {
    let mut inner = String::new();
    let mut flattened = false;
    for child in node.children() {
        let is_paragraph = matches!(child.data.borrow().value, NodeValue::Paragraph);
        if !inner.is_empty() {
            inner.push_str("<br>");
        }
        if is_paragraph {
            emit_children(child, &mut inner);
        } else {
            flattened = true;
            let mut text = String::new();
            collect_text(child, &mut text);
            escape_html(text.trim(), &mut inner);
        }
    }
    let n = escape_attr(name);
    let file_attr = source_file_attr(source_file);
    (
        format!(
            "<span class=\"tali-sidenote\" id=\"fn-{n}\" role=\"doc-footnote\" \
             data-block-id=\"fn-{n}\" data-sourcepos=\"{sourcepos}\"{file_attr}>\
             <span class=\"tali-sidenote-num\">{ix}</span>{inner}</span>"
        ),
        flattened,
    )
}

/// The exact `<sup>` markup [`emit_node`] produces for one footnote reference. The
/// splice that puts a note beside its reference matches on this string rather than
/// parsing the emitted HTML: both ends are generated here, so an exact reconstruction
/// cannot mis-target the way a substring search for `fnref-` could.
pub(crate) fn footnote_ref_markup(name: &str, ref_num: u32, ix: u32) -> String {
    format!(
        "<sup class=\"tali-fnref\" id=\"fnref-{name}-{ref_num}\"><a role=\"doc-noteref\" href=\"#fn-{name}\">{ix}</a></sup>",
        name = escape_attr(name),
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

/// Wrap each source line of already-highlighted code HTML in `<span class="tali-hl-ln">`
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
        .map(|l| format!("<span class=\"tali-hl-ln\">{l}</span>"))
        .collect()
}

/// Does an HTML line fragment contain any non-whitespace text outside of tags?
/// (Entities like `&lt;` count as text — they carry no literal `<`/`>`.)
/// Line-wrap the code inside a rendered `<pre><code>…</code></pre>` block (used for
/// magic-move blocks, which need addressable lines to morph between). Returns the
/// html unchanged if it isn't a code block or is already line-wrapped.
pub(crate) fn wrap_pre_lines(html: &str) -> String {
    if html.contains("class=\"tali-hl-ln\"") || !html.contains("<code") {
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
    // A header cell announces which column it labels (PA-M6): `scope="col"` so a screen
    // reader can pair each data cell with its header. Body cells carry no scope.
    let scope = if tag == "th" { " scope=\"col\"" } else { "" };
    for (i, cell) in row.children().enumerate() {
        let style = match aligns.get(i) {
            Some(TableAlignment::Left) => " style=\"text-align: left\"",
            Some(TableAlignment::Center) => " style=\"text-align: center\"",
            Some(TableAlignment::Right) => " style=\"text-align: right\"",
            _ => "",
        };
        out.push_str(&format!("<{tag}{scope}{style}>"));
        emit_children(cell, out);
        out.push_str(&format!("</{tag}>"));
    }
}

/// Emit a raw HTML block, injecting block `attrs` into its leading start tag
/// when one is present (e.g. `<div ...>`). Comments, closing tags, and other
/// fragments we can't safely annotate are emitted verbatim (no block id).
///
/// A literal with SEVERAL top-level roots (three `{{< input >}}` controls on
/// consecutive lines, say — comrak makes those one HTML block) is wrapped in a
/// single `<div>` that carries the attrs instead. A block must have exactly one
/// root element: the preview client mounts an incoming block with
/// `template.content.firstElementChild`, so injecting the id into the first of N
/// roots half-applies every op — `update` swaps in root 1 and silently drops the
/// rest (the id changes, so the op *looks* applied while the DOM keeps the old
/// content), and `remove` strands roots 2..N in the page forever. That would make
/// the preview disagree with what `build` publishes, which is the one thing the
/// block model exists to prevent. `crates/core/tests/block_single_root.rs` asserts it
/// for every document in the corpus.
fn emit_html_block(literal: &str, attrs: &str, out: &mut String) {
    let lead = literal.trim_start();
    let injectable = !attrs.is_empty()
        && lead.starts_with('<')
        && !lead.starts_with("</")
        && !lead.starts_with("<!")
        && !lead.starts_with("<?");
    if injectable && !is_single_root(literal) {
        out.push_str(&format!("<div{attrs} class=\"tali-html-block\">"));
        out.push_str(literal.trim_end());
        out.push_str("</div>");
        return;
    }
    if injectable && let Some(gt) = tag_end(literal) {
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

/// Elements with no end tag, and elements whose body is raw text (a `<` inside a
/// `<script>` is text, not a tag). Both shapes break a naive depth count.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
const RAW_TEXT_ELEMENTS: &[&str] = &["script", "style", "textarea", "title"];

/// Whether `literal` is one top-level node: one element (closed or not), with only
/// whitespace around it. Two sibling elements, or an element plus loose text, are
/// not — see [`emit_html_block`], which wraps those.
///
/// A deliberately small scanner rather than a parser: the input is a raw HTML block
/// from a `.tmd`, and the question is only "how many nodes would `firstElementChild`
/// have to choose between". An *unclosed* root (an author opening a `<div>` in one
/// block and closing it in a later one, which `corpus/layout/dense-output.tmd` does)
/// counts as one root, so that idiom keeps today's behaviour.
fn is_single_root(literal: &str) -> bool {
    let b = literal.as_bytes();
    let mut i = 0;
    let mut depth = 0usize;
    let mut roots = 0usize;
    let mut in_top_text = false;
    while i < b.len() {
        if b[i] != b'<' {
            // A run of loose top-level text is a root of its own: the client takes the
            // first element *child*, so text beside an element is dropped just as surely.
            if !b[i].is_ascii_whitespace() && depth == 0 && !in_top_text {
                in_top_text = true;
                roots += 1;
            }
            i += 1;
            continue;
        }
        if literal[i..].starts_with("<!--") {
            i = literal[i + 4..]
                .find("-->")
                .map(|r| i + 4 + r + 3)
                .unwrap_or(b.len());
            continue;
        }
        if literal[i..].starts_with("<!") || literal[i..].starts_with("<?") {
            i = literal[i..].find('>').map(|r| i + r + 1).unwrap_or(b.len());
            continue;
        }
        let closing = literal[i..].starts_with("</");
        let name_start = if closing { i + 2 } else { i + 1 };
        if !literal[name_start..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            if depth == 0 && !in_top_text {
                in_top_text = true; // a bare `<` in prose, not the start of a tag
                roots += 1;
            }
            i += 1;
            continue;
        }
        let name: String = literal[name_start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect::<String>()
            .to_ascii_lowercase();
        // This tag's `>`, skipping any inside a quoted attribute value (`alt="a > b"`,
        // an SVG `d=` path, an inline handler).
        let mut j = name_start + name.len();
        let mut quote: Option<u8> = None;
        while j < b.len() {
            match (quote, b[j]) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), _) => {}
                (None, c @ (b'"' | b'\'')) => quote = Some(c),
                (None, b'>') => break,
                (None, _) => {}
            }
            j += 1;
        }
        let self_closing = j > 0 && b[j - 1] == b'/';
        let end = (j + 1).min(b.len());
        in_top_text = false;
        if closing {
            depth = depth.saturating_sub(1);
            i = end;
            continue;
        }
        if depth == 0 {
            roots += 1;
            if roots > 1 {
                return false;
            }
        }
        if VOID_ELEMENTS.contains(&name.as_str()) || self_closing {
            i = end;
            continue;
        }
        depth += 1;
        if RAW_TEXT_ELEMENTS.contains(&name.as_str()) {
            // Resume ON the close tag (the loop pops it), so a sibling after a
            // `<script>` is not mistaken for a second root.
            let close = format!("</{name}");
            i = match literal[end..].find(&close) {
                Some(rel) => end + rel,
                None => b.len(),
            };
            continue;
        }
        i = end;
    }
    roots <= 1
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
