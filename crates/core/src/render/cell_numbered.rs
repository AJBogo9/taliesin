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

/// The generated half of a caption — `Figure 3`, `Table 2`, `Listing 7` — wrapped so CSS can
/// address it separately from the sentence beside it.
///
/// This is the ONE part of a caption the tool wrote; everything after the colon is the
/// author's own sentence and stays in the serif. Without the wrapper the two are one flat
/// string and the choice is between a whole caption in mono — which reads as terminal output,
/// the correction spec §4 records from a render — and no machine voice at all on the one word
/// that is the tool's. Shared by the figure, listing, mermaid and `{js}`-figure emitters here
/// and by the executed-table captions in `crates/server`.
pub fn caption_label(label: &str, num: &str) -> String {
    format!("<span class=\"tali-caption-label\">{label}&nbsp;{num}</span>")
}

/// A numbered figure/listing caption: the label span, with `": <caption>"` appended
/// (rendered as inline markdown) when a non-empty caption is given. Shared by the figure,
/// listing, mermaid, and `{js}`-figure emitters.
pub(crate) fn numbered_caption(label: &str, num: &str, caption: Option<&str>) -> String {
    let head = caption_label(label, num);
    match caption.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("{head}: {}", caption_inline_html(c)),
        None => head,
    }
}

/// Emit a client-side cell (`{js}`, `{glsl}`, …): an output target div plus a
/// `<script type="{lang.mime}">` carrying the author source verbatim (only `</script`
/// escaped, so it is readable in devtools — no base64). The `data-*` attrs tell the
/// client registry how to wire the cell (shared-scope name, named input, re-run
/// inputs). Block data attrs ride on the wrapper for click-to-source.
///
/// **This is the one wrapper contract every registered language shares** (see
/// [`client_lang`]): same target-div-plus-script shape, same `data-*` vocabulary, so the
/// client's language registry, the teardown hook and the reactive graph are all written
/// once against the shape rather than per language.
pub(super) fn emit_client_cell(
    lang: &ClientLang,
    src: &str,
    block_id: &str,
    js: &JsOpts,
    block_attrs: &str,
) -> String {
    // The target id keeps the `tali-js-` prefix for every language: it is a DOM id, not a
    // language tag, and renaming it per language would fork the one selector the preview
    // client, the screenshot harness and `strip_client_scripts` all key off.
    let target = format!("tali-js-{block_id}");
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
    let (mime, class) = (lang.mime, lang.class);
    format!(
        "<div{block_attrs} class=\"cell {class}\"><div class=\"tali-js-out\" id=\"{target}\"></div>\
         <script type=\"{mime}\"{data}>{safe_src}</script></div>"
    )
}

/// The float identity a numbered client figure carries. Bundled rather than passed
/// positionally: with the language spec, the source, the block id, the cell options and
/// the block attrs already in the signature, three more `Option<&str>`/`&str` in a row is
/// where a call site starts silently transposing the anchor and the caption.
pub(super) struct FloatLabel<'a> {
    pub anchor: Option<&'a str>,
    pub caption: Option<&'a str>,
    pub num: &'a str,
}

/// Wrap a client-side cell in a numbered `<figure>` (for `label: fig-x` cells, e.g. a
/// Three.js scene or a `{glsl}` shader). The block attrs + `#fig-` anchor ride on the
/// figure.
pub(super) fn emit_client_figure(
    lang: &ClientLang,
    src: &str,
    block_id: &str,
    js: Option<&JsOpts>,
    block_attrs: &str,
    float: &FloatLabel<'_>,
) -> String {
    let default = JsOpts::default();
    let cell = emit_client_cell(lang, src, block_id, js.unwrap_or(&default), "");
    let id_attr = id_attr(float.anchor);
    let figcap = numbered_caption("Figure", float.num, float.caption);
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
    fold: Option<&CodeFold>,
    block_attrs: &str,
    num: &str,
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
        Some(f) => format!(
            "<details class=\"tali-code-fold\"{}>{}<pre><code{class}>{code_html}</code></pre></details>",
            if f.open { " open" } else { "" },
            f.summary_html(),
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
