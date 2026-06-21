//! Standalone-image figures: detect an image-only paragraph as a `<figure>`,
//! emit the numbered figure / figcaption, and the mermaid-diagram variant.
//! Split out of the render module; `use super::*` reaches the shared AST types
//! and helpers (AstNode, NodeValue, emit_children, parse_pandoc_attrs, html
//! escaping).

use super::*;

/// A standalone-image paragraph recognized as a figure.
pub(super) struct FigureParts {
    url: String,
    /// Rendered inline HTML of the caption (the image's alt content).
    caption: String,
    pub(super) attrs: DivAttrs,
}

/// If `node` is a paragraph that is a single image, optionally followed by a
/// `{#id key=val}` attribute block, return its figure parts. Any other content
/// in the paragraph (stray text, a link, a second image) disqualifies it, so it
/// falls through to ordinary inline-image rendering.
pub(super) fn figure_parts<'a>(node: &'a AstNode<'a>) -> Option<FigureParts> {
    let mut image: Option<&'a AstNode<'a>> = None;
    let mut attr_str: Option<String> = None;
    for child in node.children() {
        let d = child.data.borrow();
        match &d.value {
            NodeValue::Image(_) => {
                if image.is_some() {
                    return None;
                }
                drop(d);
                image = Some(child);
            }
            NodeValue::SoftBreak | NodeValue::LineBreak => {}
            NodeValue::Text(t) => {
                let t = t.trim();
                if t.is_empty() {
                    continue;
                }
                match t.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    Some(a) if attr_str.is_none() => attr_str = Some(a.trim().to_string()),
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
    let image = image?;
    let url = match &image.data.borrow().value {
        NodeValue::Image(link) => link.url.clone(),
        _ => return None,
    };
    let mut caption = String::new();
    emit_children(image, &mut caption);
    let attrs = parse_attrs(attr_str.as_deref().unwrap_or(""));
    let has_fig_id = attrs.id.as_deref().is_some_and(|i| i.starts_with("fig-"));
    // A bare image with neither a caption nor a `#fig-` id is decorative.
    if caption.trim().is_empty() && !has_fig_id {
        return None;
    }
    Some(FigureParts {
        url,
        caption,
        attrs,
    })
}

/// Render a recognized figure as a numbered `<figure>` carrying the block data
/// attributes, honoring `width=` and `fig-align=`.
pub(super) fn emit_figure(fig: &FigureParts, block_attrs: &str, num: usize) -> String {
    let id_attr = id_attr(fig.attrs.id.as_deref());
    let align_class = match fig.attrs.get("fig-align") {
        Some("left") => " qmd-figure-left",
        Some("right") => " qmd-figure-right",
        _ => " qmd-figure-center",
    };
    let style = match fig.attrs.get("width") {
        Some(w) => format!(" style=\"width:{}\"", escape_attr(w)),
        None => String::new(),
    };
    // `alt` is the caption HTML with tags stripped: it already carries valid
    // entities, so only quote-escape it (escape_attr would double-escape `&`).
    let alt = strip_tags(&fig.caption);
    format!(
        "<figure{block_attrs}{id_attr} class=\"qmd-figure{align_class}\">\
         <img src=\"{}\" alt=\"{}\"{style} />\
         <figcaption>Figure&nbsp;{num}: {}</figcaption></figure>",
        escape_attr(&fig.url),
        escape_attr_from_html(&alt),
        fig.caption,
    )
}

/// Render a labelled/captioned `{mermaid}` cell as a numbered `<figure>` wrapping
/// the diagram `<pre>`, carrying the block attrs and (when labelled) the `#fig-`
/// anchor so `@fig-x` cross-references resolve and click-to-zoom still works.
pub(super) fn emit_mermaid_figure(
    code: &str,
    anchor: Option<&str>,
    caption: Option<&str>,
    block_attrs: &str,
    num: usize,
) -> String {
    let id_attr = id_attr(anchor);
    let mut diagram = String::new();
    escape_html(code, &mut diagram);
    let figcap = numbered_caption("Figure", num, caption);
    format!(
        "<figure{block_attrs}{id_attr} class=\"qmd-figure qmd-figure-center\">\
         <pre class=\"mermaid\">{diagram}</pre>\
         <figcaption>{figcap}</figcaption></figure>"
    )
}
