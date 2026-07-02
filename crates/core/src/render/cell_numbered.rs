//! Numbered code-cell emission: a `{js}` cell, a `{js}` cell wrapped in a numbered
//! `<figure>`, and a labelled code cell rendered as a numbered listing, plus the
//! shared numbered-caption builder.
//!
//! These emit the `<figure>`/listing/`{js}` HTML that carries data-block-id +
//! data-sourcepos — but they receive the orchestrator-built `block_attrs` string as an
//! ARG and interpolate it verbatim; they never construct the data-attrs themselves, so
//! the block-id/sourcepos invariants are owned entirely by the caller. Counting + xref
//! registration stay in render_internal_impl. Uses mod.rs's html_escape/escape_attr/
//! id_attr helpers via `use super::*` (the render child-sees-parent convention).

use super::*;

/// Render a caption STRING (from a `fig-cap:`/`lst-cap:` cell option) as inline
/// markdown, so `$...$` math, `*emphasis*`, and `` `code` `` render the same way an
/// image-alt caption does (via `emit_children`) instead of surviving as literal
/// escaped text. Parses with the shared options (`math_dollars` on) and emits the
/// first paragraph's inline children. Falls back to a plain escape if the input
/// somehow parses without a paragraph.
fn caption_inline_html(caption: &str) -> String {
    let arena = Arena::new();
    let options = parse_options();
    let root = parse_document(&arena, caption, &options);
    for child in root.children() {
        if matches!(child.data.borrow().value, NodeValue::Paragraph) {
            let mut out = String::new();
            emit_children(child, &mut out);
            return out;
        }
    }
    html_escape(caption)
}

/// A numbered figure/listing caption: `"<Label>&nbsp;<num>"`, with `": <caption>"`
/// appended (rendered as inline markdown) when a non-empty caption is given. Shared
/// by the figure, listing, mermaid, and `{js}`-figure emitters.
pub(crate) fn numbered_caption(label: &str, num: usize, caption: Option<&str>) -> String {
    match caption.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("{label}&nbsp;{num}: {}", caption_inline_html(c)),
        None => format!("{label}&nbsp;{num}"),
    }
}

/// Emit a native interactive `{js}` cell: an output target div plus an
/// `application/qmd-js` script carrying the author source verbatim (only `</script`
/// escaped, so it is readable in devtools — no base64). The `data-*` attrs tell the
/// `qmd-js` enhancer how to wire the cell (shared-scope name, named input, re-run
/// inputs). Block data attrs ride on the wrapper for click-to-source.
pub(super) fn emit_js_cell(src: &str, block_id: &str, js: &JsOpts, block_attrs: &str) -> String {
    let target = format!("qmd-js-{block_id}");
    let mut data = format!(" data-target=\"{target}\"");
    if let Some(n) = js.name.as_deref() {
        data.push_str(&format!(" data-name=\"{}\"", escape_attr(n)));
    }
    if let Some(v) = js.viewof.as_deref() {
        data.push_str(&format!(" data-viewof=\"{}\"", escape_attr(v)));
    }
    if !js.inputs.is_empty() {
        data.push_str(&format!(
            " data-inputs=\"{}\"",
            escape_attr(&js.inputs.join(","))
        ));
    }
    // `</script` is the only sequence that can terminate the script element; escape
    // it so author source carrying it (e.g. in a template literal) stays intact.
    let safe_src = src.replace("</script", "<\\/script");
    format!(
        "<div{block_attrs} class=\"cell tali-js-cell\"><div class=\"tali-js-out\" id=\"{target}\"></div>\
         <script type=\"application/qmd-js\"{data}>{safe_src}</script></div>"
    )
}

/// Wrap a native `{js}` cell in a numbered `<figure>` (for `label: fig-x` js cells,
/// e.g. a Three.js scene). The block attrs + `#fig-` anchor ride on the figure.
pub(super) fn emit_js_figure(
    src: &str,
    block_id: &str,
    js: Option<&JsOpts>,
    anchor: Option<&str>,
    caption: Option<&str>,
    block_attrs: &str,
    num: usize,
) -> String {
    let default = JsOpts::default();
    let cell = emit_js_cell(src, block_id, js.unwrap_or(&default), "");
    let id_attr = id_attr(anchor);
    let figcap = numbered_caption("Figure", num, caption);
    format!(
        "<figure{block_attrs}{id_attr} class=\"tali-figure tali-figure-center\">\
         {cell}<figcaption>{figcap}</figcaption></figure>"
    )
}

/// Render a labelled code cell's source as a numbered listing (`@lst-x`),
/// caption above the code. The block attrs + `#lst-` anchor ride on the wrapper.
pub(super) fn emit_code_listing(
    code: &str,
    lang: &str,
    anchor: Option<&str>,
    caption: Option<&str>,
    fold: Option<&(bool, String)>,
    block_attrs: &str,
    num: usize,
) -> String {
    let id_attr = id_attr(anchor);
    let class = if lang.is_empty() {
        String::new()
    } else {
        format!(" class=\"language-{lang}\"")
    };
    let code_html = crate::highlight::highlight(code, (!lang.is_empty()).then_some(lang));
    let figcap = numbered_caption("Listing", num, caption);
    // `code-fold` collapses the listing's source behind its summary.
    let code_html = match fold {
        Some((open, summary)) => format!(
            "<details class=\"tali-code-fold\"{}><summary>{}</summary><pre><code{class}>{code_html}</code></pre></details>",
            if *open { " open" } else { "" },
            html_escape(summary),
        ),
        None => format!("<pre><code{class}>{code_html}</code></pre>"),
    };
    // A `<figure>` (not a `<div>`): a `<figcaption>` is only valid inside a `<figure>`,
    // and a numbered code listing IS a captioned float (same float semantics as a figure).
    // `.tali-listing` already zeroes the UA figure margin, so the element swap is style-neutral.
    format!(
        "<figure{block_attrs}{id_attr} class=\"tali-listing\">\
         <figcaption class=\"tali-listing-caption\">{figcap}</figcaption>{code_html}</figure>"
    )
}
