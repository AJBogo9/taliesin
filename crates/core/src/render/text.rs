//! Text projection: a deterministic, screen-reader-like plain-text VIEW of a rendered
//! document, for an agent (or a blind author) that wants to read what it made without a
//! browser and without HTML noise.
//!
//! This is a **view, not an output format** (HTML stays the only build target). It walks
//! the already-built block model and turns each block's *rendered* HTML — post-citation,
//! so `@fig-`/`@sec-`/`[@key]` are already resolved to "Figure 3" / "Section 2.1" — into
//! structured text: headings keep their level, figures show their resolved "Figure N"
//! caption and image alt, callouts are labelled by kind, code cells/blocks are fenced with
//! their language, display math is emitted as raw TeX, and everything else projects to its
//! visible text.
//!
//! Reuses mod.rs's private `strip_tags`/`unescape_html` (a child module sees its parent's
//! privates) so the visible-text extraction stays identical to the TOC/slug path.

use super::*;

/// Project a document's block model to structured plain text. Blocks are separated by a
/// blank line; the result ends with a single trailing newline.
pub(crate) fn project(blocks: &[Block]) -> String {
    project_with_js(blocks, &std::collections::HashMap::new())
}

/// Like [`project`], but appends a caller-supplied `[js: …]` observation line after each
/// `{js}` cell whose block id is a key in `js_lines`. A `{js}` cell runs in the *browser*,
/// so there is no server-side output block to project (unlike an executed python/r cell);
/// the server observes it headlessly (DX17b) and hands the formatted line here, and core
/// only interleaves it at the cell's position — the same place an executed python/r output
/// block projects. Non-`{js}` cells and cells absent from the map project unchanged, so
/// `project(blocks)` (empty map) is byte-identical to the pre-DX17b projection.
pub(crate) fn project_with_js(
    blocks: &[Block],
    js_lines: &std::collections::HashMap<String, String>,
) -> String {
    let mut out = String::new();
    for b in blocks {
        let mut piece = project_block(b).trim_end_matches('\n').to_string();
        if piece.trim().is_empty() {
            continue;
        }
        if b.cell.as_ref().is_some_and(|c| c.lang == "js")
            && let Some(line) = js_lines.get(&b.id)
        {
            piece.push('\n');
            piece.push_str(line);
        }
        out.push_str(&piece);
        out.push_str("\n\n");
    }
    // Exactly one trailing newline.
    format!("{}\n", out.trim_end())
}

/// Project a single block. Order matters: a code cell is detected before its HTML, a
/// heading/figure/callout/pre by its leading element, everything else by visible text.
fn project_block(b: &Block) -> String {
    // A code cell: fence its source with the language, whether or not it executed. (In
    // parse-only `read` there is no output to show; the source is what the agent wrote.)
    if let Some(cell) = &b.cell {
        return fenced_code(&cell.lang, &cell.code);
    }

    let html = b.html.trim_start();

    // An executed cell's output block (DX17): report what it produced, so a headless agent
    // can tell its figure/table baked, its cell printed, or its cell errored. Must precede
    // the figure/visible arms below (a produced figure's `<figure>` is nested in the
    // `tali-output` div and would otherwise project as plain "Figure N: …" text).
    if let Some(kind) = classify_exec_output(html) {
        return match kind {
            ExecOutput::Figure {
                fig_id: Some(id),
                alt: Some(a),
            } => format!("[figure {id}: produced, alt \"{a}\"]"),
            ExecOutput::Figure {
                fig_id: Some(id),
                alt: None,
            } => format!("[figure {id}: produced]"),
            ExecOutput::Figure {
                fig_id: None,
                alt: Some(a),
            } => format!("[figure: produced (image), alt \"{a}\"]"),
            ExecOutput::Figure {
                fig_id: None,
                alt: None,
            } => "[figure: produced (image)]".to_string(),
            ExecOutput::Table { tbl_id: Some(id) } => format!("[table {id}: produced]"),
            ExecOutput::Table { tbl_id: None } => "[table: produced]".to_string(),
            ExecOutput::Stream(s) => format!("[output: {s}]"),
            ExecOutput::Error(s) => format!("[cell error: {s}]"),
            ExecOutput::Rich => "[output: produced]".to_string(),
        };
    }

    // Heading: keep the level as `#`s.
    if let Some(level) = block_heading_level(html) {
        return format!("{} {}", "#".repeat(level as usize), visible(html));
    }

    // Display math: emit the raw TeX (KaTeX stores it in an `<annotation>`). Inline `$…$`
    // uses `katex` (not `katex-display`) and falls through to its visible glyphs.
    if html.contains("katex-display")
        && let Some(tex) = annotation_tex(html)
    {
        return format!("$$ {tex} $$");
    }

    // A plain fenced code block (non-cell): fence the code text with its language.
    if leading_tag(html) == Some("pre") {
        let lang = code_lang(html).unwrap_or_default();
        return fenced_code(&lang, &decode_code(html));
    }

    // A figure: the figcaption already reads "Figure N: caption"; add the image alt so an
    // agent knows the alt text it shipped.
    if leading_tag(html) == Some("figure") {
        let mut s = visible(html);
        if let Some(alt) = first_attr(html, "alt").filter(|a| !a.is_empty()) {
            s.push_str(&format!("\n[image: {alt}]"));
        }
        return s;
    }

    // A callout: label it by kind, then its title (if any) and body on their own lines, so
    // an agent reads "[note] Heads up" instead of the title running into the body.
    if let Some(kind) = callout_kind(html) {
        return project_callout(html, kind);
    }

    // A list: project each top-level <li> on its own line so items don't run together
    // (`…reference.Returns —…`). Ordered lists count; a nested list indents two spaces.
    if matches!(leading_tag(html), Some("ul") | Some("ol")) {
        return project_list(html, 0);
    }

    // A scrolly / code-walkthrough: project each `.step`'s narration as its own paragraph
    // so adjacent steps don't merge across the boundary (`…in the middle.Which way…`). The
    // `scrolly-steps` container carries the token `scrolly-steps`, not `step`, so matching
    // the exact opening `<div class="step"` never mistakes the container for a step.
    if html.contains("<div class=\"step\"") {
        return project_steps(html);
    }

    // Input control(s): `[input] label = value`, one line per control, so a control's
    // label and value don't fuse (`step size (η)0.12`). `class="tali-input"` (closing
    // quote included) matches only the control wrapper, not `tali-input-label`/`-out`.
    if html.contains("class=\"tali-input\"") {
        return project_inputs(html);
    }

    let text = visible(html);
    // A bare image (no figure/caption) has no visible text; surface its alt so the block
    // isn't silently dropped.
    if text.is_empty()
        && let Some(alt) = first_attr(html, "alt")
    {
        return format!("[image: {alt}]");
    }
    text
}

/// A fenced code block: ` ```lang\n<code>\n``` ` (blank lang omits the info string).
fn fenced_code(lang: &str, code: &str) -> String {
    format!("```{lang}\n{}\n```", code.trim_end_matches('\n'))
}

/// Decode already-stripped text: `&nbsp;` normalized to a space, then the entities the
/// renderer emits decoded exactly once. The single home for this recipe — [`visible`]
/// and [`indexable_text`] differ only in how they strip and trim, never in how they
/// decode, and a caller that rewrites it by hand gets `&amp;lt;` wrong (a chained
/// `.replace` decodes it twice, to `<`).
fn decode(stripped: &str) -> String {
    unescape_html(&stripped.replace("&nbsp;", " "))
}

/// Visible text of a block's HTML: tags stripped (KaTeX `<math>` MathML dropped),
/// `&nbsp;` normalized to a space, entities decoded. Identical extraction to the
/// TOC/slug path, so what `read` shows matches what a heading's slug/label sees.
fn visible(html: &str) -> String {
    decode(&strip_tags(html)).trim().to_string()
}

/// Visible text of a *run* of block HTML, for the cross-page search index: the same
/// extraction as [`visible`] — so an indexed snippet reads exactly like the page it
/// points at — plus a space at every tag boundary and collapsed whitespace, since the
/// index reads many blocks as one string.
///
/// Sharing [`strip_tags_inner`] is what keeps the index honest: a hand-rolled `<`/`>`
/// scan indexes KaTeX's MathML *and* its raw-TeX `<annotation>` alongside the visible
/// glyphs, so every formula lands three times and leaks LaTeX into the index.
pub(crate) fn indexable_text(html: &str) -> String {
    decode(&strip_tags_separated(html))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Decode a `<pre><code>` block's text: strip the highlight spans, decode entities. Keeps
/// interior newlines (unlike [`visible`], which trims) so code lines survive.
fn decode_code(html: &str) -> String {
    unescape_html(&strip_tags(html))
}

/// The kind of a callout div (`class="callout callout-{kind}…"`), e.g. `note`.
fn callout_kind(html: &str) -> Option<&str> {
    let key = "callout callout-";
    let at = html.find(key)? + key.len();
    let rest = &html[at..];
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))?;
    Some(&rest[..end]).filter(|s| !s.is_empty())
}

/// `[kind] title` then the body on its own line, from a callout's `callout-title` /
/// `callout-body` sub-elements (the title runs up to the body's opening tag; the body runs
/// to the end, since the trailing `</div>`s carry no text).
fn project_callout(html: &str, kind: &str) -> String {
    let body_span = class_tag_span(html, "callout-body");
    let title = class_tag_span(html, "callout-title")
        .map(|(_, gt)| {
            let end = body_span
                .map(|(lt, _)| lt)
                .unwrap_or(html.len())
                .max(gt + 1);
            visible(&html[gt + 1..end])
        })
        .filter(|s| !s.is_empty());
    let body = body_span
        .map(|(_, gt)| visible(&html[gt + 1..]))
        .unwrap_or_default();
    let head = match title {
        Some(t) => format!("[{kind}] {t}"),
        None => format!("[{kind}]"),
    };
    if body.is_empty() {
        head
    } else {
        format!("{head}\n{body}")
    }
}

/// Project a `<ul>`/`<ol>` list block to one line per item, nested lists indented two
/// spaces per level. `indent` is the current nesting depth (0 at the top). Each item keeps
/// its visible inline text (bold/links/code stripped); an ordered list counts from `start`.
fn project_list(html: &str, indent: usize) -> String {
    let ordered = leading_tag(html) == Some("ol");
    // `<ol start="N">` begins at N; a bare `<ol>`/`<ul>` at 1.
    let mut n: usize = first_attr(html, "start")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let pad = "  ".repeat(indent);
    let mut lines = Vec::new();
    for item in top_level_li_inner(html) {
        // Split off a trailing nested list: the text before it is this item; the nested
        // list (if any) recurses one level deeper.
        let (own, nested) = split_nested_list(item);
        let marker = if ordered {
            let m = format!("{n}.");
            n += 1;
            m
        } else {
            "-".to_string()
        };
        lines.push(format!("{pad}{marker} {}", visible(own)));
        if let Some(nested) = nested {
            let sub = project_list(nested, indent + 1);
            if !sub.is_empty() {
                lines.push(sub);
            }
        }
    }
    lines.join("\n")
}

/// The inner HTML of each TOP-LEVEL `<li>` in a list block. `<li>` is emitted
/// attribute-free (`emit.rs::emit_item`), so the literal `<li>`/`</li>` delimit items;
/// nested lists' `<li>`s are matched by depth so a nested item is not taken as top-level.
fn top_level_li_inner(html: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut pos = 0;
    while let Some(rel) = html[pos..].find("<li>") {
        let start = pos + rel + "<li>".len();
        let mut depth = 1usize;
        let mut i = start;
        loop {
            let open = html[i..].find("<li>");
            let close = html[i..].find("</li>");
            match (open, close) {
                (Some(o), Some(c)) if o < c => {
                    depth += 1;
                    i += o + "<li>".len();
                }
                (_, Some(c)) => {
                    depth -= 1;
                    if depth == 0 {
                        items.push(&html[start..i + c]);
                        i += c + "</li>".len();
                        break;
                    }
                    i += c + "</li>".len();
                }
                _ => {
                    // Malformed (no matching close): take the rest and stop.
                    items.push(&html[start..]);
                    i = html.len();
                    break;
                }
            }
        }
        pos = i;
    }
    items
}

/// Split a `<li>`'s inner HTML at its first nested list (`<ul`/`<ol`): the leading part is
/// the item's own content; the trailing part (if any) is the nested list to recurse into.
fn split_nested_list(item: &str) -> (&str, Option<&str>) {
    let at = match (item.find("<ul"), item.find("<ol")) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    match at {
        Some(at) => (&item[..at], Some(&item[at..])),
        None => (item, None),
    }
}

/// Project a stepped block (a `.scrolly`'s `scrolly-steps`, or a `.code-walkthrough`) so
/// each `.step`'s visible text is its own paragraph, blank-line separated.
fn project_steps(html: &str) -> String {
    step_inners(html)
        .into_iter()
        .map(visible)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The inner HTML of each `<div class="step"…>` in a stepped block, matched depth-aware
/// over `<div`/`</div>` so a nested `<div>` inside a step does not close it early.
fn step_inners(html: &str) -> Vec<&str> {
    let open = "<div class=\"step\"";
    let mut steps = Vec::new();
    let mut pos = 0;
    while let Some(rel) = html[pos..].find(open) {
        let tag_start = pos + rel;
        let Some(gt) = html[tag_start..].find('>') else {
            break;
        };
        let start = tag_start + gt + 1;
        let mut depth = 1usize;
        let mut i = start;
        loop {
            let next_open = html[i..].find("<div");
            let next_close = html[i..].find("</div>");
            match (next_open, next_close) {
                (Some(o), Some(c)) if o < c => {
                    depth += 1;
                    i += o + "<div".len();
                }
                (_, Some(c)) => {
                    depth -= 1;
                    if depth == 0 {
                        steps.push(&html[start..i + c]);
                        i += c + "</div>".len();
                        break;
                    }
                    i += c + "</div>".len();
                }
                _ => {
                    steps.push(&html[start..]);
                    i = html.len();
                    break;
                }
            }
        }
        pos = i;
    }
    steps
}

/// Project a `{{< input >}}` block's control(s) as `[input] <label> = <value>`, one line
/// per control (a block can hold several). Label = the `.tali-input-label` text; value =
/// the `.tali-input-out` `<output>` text.
fn project_inputs(html: &str) -> String {
    let open = "<div class=\"tali-input\"";
    let mut lines = Vec::new();
    let mut pos = 0;
    while let Some(rel) = html[pos..].find(open) {
        let start = pos + rel;
        // This control runs to the next control, or to the block end.
        let next = html[start + open.len()..]
            .find(open)
            .map(|r| start + open.len() + r)
            .unwrap_or(html.len());
        let chunk = &html[start..next];
        let label = class_text(chunk, "tali-input-label").unwrap_or_default();
        let value = class_text(chunk, "tali-input-out").unwrap_or_default();
        lines.push(format!("[input] {label} = {value}"));
        pos = next;
    }
    lines.join("\n")
}

/// The visible text of the first element whose opening tag carries `class_token`. The
/// label/output spans hold plain text with no nested element, so the text runs from the
/// tag's `>` to the next `<` (its closing tag).
fn class_text(html: &str, class_token: &str) -> Option<String> {
    let (_, gt) = class_tag_span(html, class_token)?;
    let inner_start = gt + 1;
    let end = html[inner_start..]
        .find('<')
        .map(|r| inner_start + r)
        .unwrap_or(html.len());
    Some(visible(&html[inner_start..end]))
}

/// `(index of '<' opening the element, index of '>' closing its opening tag)` for the first
/// element whose opening tag carries `class_token`. Quote-aware via [`tag_end`].
fn class_tag_span(html: &str, class_token: &str) -> Option<(usize, usize)> {
    let at = html.find(class_token)?;
    let lt = html[..at].rfind('<')?;
    let gt = lt + tag_end(&html[lt..])?;
    Some((lt, gt))
}

/// The lowercased name of the first HTML tag in `html`, or `None` if it doesn't start with
/// a tag.
fn leading_tag(html: &str) -> Option<&str> {
    let rest = html.strip_prefix('<')?;
    let end = rest.find(|c: char| !c.is_ascii_alphanumeric())?;
    Some(&rest[..end])
}

/// The `language-x` class of a `<pre>`/`<code>` fenced block → `x`.
fn code_lang(html: &str) -> Option<String> {
    let at = html.find("language-")?;
    let rest = &html[at + "language-".len()..];
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '+' || c == '#'))
        .unwrap_or(rest.len());
    Some(rest[..end].to_string()).filter(|s| !s.is_empty())
}

/// The raw TeX inside the first `<annotation encoding="application/x-tex">…</annotation>`
/// (KaTeX's source-of-truth for the equation), entity-decoded.
fn annotation_tex(html: &str) -> Option<String> {
    let key = "encoding=\"application/x-tex\">";
    let at = html.find(key)? + key.len();
    let rest = &html[at..];
    let end = rest.find("</annotation>")?;
    Some(unescape_html(rest[..end].trim()))
}

/// The value of the first `attr="…"` occurrence in `html` (entity-decoded). Used for the
/// image `alt`, which [`strip_tags`] drops as an attribute.
fn first_attr(html: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let at = html.find(&needle)? + needle.len();
    let rest = &html[at..];
    let end = rest.find('"')?;
    Some(unescape_html(rest[..end].trim()))
}

/// The kind of output an executed code cell produced, classified from its rendered
/// `tali-output` block. Shared by the text projection ([`project_block`]) and `read`'s
/// `--format json`. Reads only the rendered HTML — it never reaches back into exec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecOutput {
    /// A produced image/plot. `fig_id`/`alt` are set for a labelled figure cell
    /// (`#| label: fig-x` + `#| fig-cap:`); an unlabelled plot has both `None`.
    Figure {
        fig_id: Option<String>,
        alt: Option<String>,
    },
    Table {
        tbl_id: Option<String>,
    },
    /// stdout/stderr or plain text output (first non-empty line, trimmed).
    Stream(String),
    /// A cell error: the summary line (`EName: evalue`).
    Error(String),
    /// Any other rich output (e.g. an unlabelled HTML `<div>` widget).
    Rich,
}

/// Classify an executed output block's HTML (`output_block` in `crates/server/src/exec.rs`
/// emits `<div class="tali-output" …>{inner}</div>`). `None` if it is not such a block.
pub fn classify_exec_output(output_html: &str) -> Option<ExecOutput> {
    let at = output_html.find("class=\"tali-output\"")?;
    let start = output_html[at..].find('>')? + at + 1;
    let inner = output_html[start..]
        .strip_suffix("</div>")
        .unwrap_or(&output_html[start..])
        .trim_start();

    if inner.starts_with("<figure") {
        return Some(ExecOutput::Figure {
            fig_id: first_attr(inner, "id"),
            alt: figcaption_caption(inner),
        });
    }
    if inner.starts_with("<img") || inner.starts_with("<svg") {
        // An unlabelled plot is still "a figure produced" to the agent; the generic
        // alt="output" carries no caption.
        let alt = first_attr(inner, "alt").filter(|a| a != "output");
        return Some(ExecOutput::Figure { fig_id: None, alt });
    }
    // A table cell (`table_wrap`) or a rich DataFrame table, possibly `<div>`-wrapped.
    // Error/stream outputs are `<pre>` with escaped text, so they never match a raw `<table`.
    if let Some(tpos) = inner.find("<table") {
        return Some(ExecOutput::Table {
            tbl_id: first_attr(&inner[tpos..], "id"),
        });
    }
    if inner.contains("class=\"tali-error\"") {
        return Some(ExecOutput::Error(error_summary(inner)));
    }
    if inner.contains("class=\"tali-stream") {
        let text = visible(inner);
        let first = text
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim();
        return Some(ExecOutput::Stream(first.to_string()));
    }
    Some(ExecOutput::Rich)
}

/// The caption of a `figure_wrap` figcaption (the text after "Figure&nbsp;N: "), or `None`
/// for a bare "Figure N" (an unlabelled figure).
fn figcaption_caption(figure_html: &str) -> Option<String> {
    let start = figure_html.find("<figcaption>")? + "<figcaption>".len();
    let end = figure_html[start..].find("</figcaption>")? + start;
    let text = decode(&figure_html[start..end]);
    let (_, cap) = text.split_once(':')?;
    let cap = cap.trim();
    (!cap.is_empty()).then(|| cap.to_string())
}

/// The summary line of a baked `tali-error` pre: the last non-empty line of the decoded
/// text, which for a no-traceback error and a traceback alike is `EName: evalue`.
fn error_summary(html: &str) -> String {
    visible(html)
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_src(src: &str) -> String {
        let doc = crate::render_document(src);
        project(&doc.blocks)
    }

    /// The MVP: a heading keeps its `#`, and a labelled figure projects its *resolved*
    /// "Figure 1: cap" (proving the projector reads post-citation `block.html`, not the AST).
    #[test]
    fn projects_heading_and_resolved_figure() {
        let out = project_src("# Heading\n\n![cap](x.png){#fig-a}\n");
        assert!(out.contains("# Heading"), "heading kept its level:\n{out}");
        assert!(
            out.contains("Figure 1: cap"),
            "figure caption resolved to its number:\n{out}"
        );
    }

    /// A code cell fences its source with the language.
    #[test]
    fn projects_code_cell_fenced() {
        let out = project_src("```{python}\nprint(1)\n```\n");
        assert!(
            out.contains("```python\nprint(1)\n```"),
            "code cell fenced with lang:\n{out}"
        );
    }

    /// Display math projects to its raw TeX, not the KaTeX glyph soup.
    #[test]
    fn projects_display_math_as_tex() {
        let out = project_src("$$H_0 = 1$$\n");
        assert!(out.contains("H_0 = 1"), "raw TeX preserved:\n{out}");
    }

    /// A callout is labelled by kind with its title and body separated, not run together.
    #[test]
    fn projects_callout_labelled_by_kind() {
        let out = project_src("::: {.callout-note}\n## Heads up\n\nBe careful.\n:::\n");
        assert!(
            out.contains("[note] Heads up"),
            "callout kind + title labelled:\n{out}"
        );
        assert!(out.contains("Be careful."), "callout body kept:\n{out}");
    }

    // --- DX17: executed-output classification + projection ---

    #[test]
    fn classify_exec_output_none_for_non_output() {
        assert!(classify_exec_output("<p>hello</p>").is_none());
    }

    #[test]
    fn classify_exec_output_labelled_figure() {
        let html = "<div class=\"tali-output\" data-block-id=\"b-out\" data-sourcepos=\"5:1-7:3\">\
            <figure id=\"fig-hist\" class=\"tali-figure tali-figure-center\">\
            <img alt=\"output\" src=\"data:image/png;base64,AAA\">\
            <figcaption>Figure&nbsp;2: A histogram of scores</figcaption></figure></div>";
        match classify_exec_output(html) {
            Some(ExecOutput::Figure { fig_id, alt }) => {
                assert_eq!(fig_id.as_deref(), Some("fig-hist"));
                assert_eq!(alt.as_deref(), Some("A histogram of scores"));
            }
            other => panic!("expected Figure, got {other:?}"),
        }
    }

    #[test]
    fn classify_exec_output_unlabelled_image_is_a_figure() {
        let html = "<div class=\"tali-output\" data-block-id=\"b-out\" data-sourcepos=\"5:1-7:3\">\
            <img alt=\"output\" src=\"data:image/png;base64,AAA\"></div>";
        match classify_exec_output(html) {
            Some(ExecOutput::Figure { fig_id, alt }) => {
                assert!(fig_id.is_none());
                assert!(
                    alt.is_none(),
                    "generic alt=\"output\" must not surface as a caption"
                );
            }
            other => panic!("expected Figure, got {other:?}"),
        }
    }

    #[test]
    fn classify_exec_output_table_error_stream() {
        let tbl = "<div class=\"tali-output\" data-block-id=\"b\" data-sourcepos=\"1:1-1:1\">\
            <table id=\"tbl-a\"><caption>Table&nbsp;1: Counts</caption><tr><td>1</td></tr></table></div>";
        assert!(matches!(classify_exec_output(tbl),
            Some(ExecOutput::Table { tbl_id }) if tbl_id.as_deref() == Some("tbl-a")));

        let err = "<div class=\"tali-output\" data-block-id=\"b\" data-sourcepos=\"1:1-1:1\">\
            <pre class=\"tali-error\">Traceback\nValueError: bad value</pre></div>";
        assert!(matches!(classify_exec_output(err),
            Some(ExecOutput::Error(s)) if s == "ValueError: bad value"));

        let out = "<div class=\"tali-output\" data-block-id=\"b\" data-sourcepos=\"1:1-1:1\">\
            <pre class=\"tali-stream\">hello world\nsecond line</pre></div>";
        assert!(matches!(classify_exec_output(out),
            Some(ExecOutput::Stream(s)) if s == "hello world"));
    }

    #[test]
    fn project_block_renders_executed_outputs() {
        let fig = Block {
            id: "b-out".into(),
            sourcepos: "5:1-7:3".into(),
            source_file: None,
            html: "<div class=\"tali-output\" data-block-id=\"b-out\" data-sourcepos=\"5:1-7:3\">\
                <figure id=\"fig-hist\" class=\"tali-figure tali-figure-center\">\
                <img alt=\"output\" src=\"data:image/png;base64,AAA\">\
                <figcaption>Figure&nbsp;2: A histogram</figcaption></figure></div>"
                .into(),
            cell: None,
        };
        assert_eq!(
            project_block(&fig),
            "[figure fig-hist: produced, alt \"A histogram\"]"
        );

        let err = Block {
            id: "b2-out".into(),
            sourcepos: "1:1-1:1".into(),
            source_file: None,
            html: "<div class=\"tali-output\" data-block-id=\"b2-out\" data-sourcepos=\"1:1-1:1\">\
                <pre class=\"tali-error\">ValueError: bad value</pre></div>"
                .into(),
            cell: None,
        };
        assert_eq!(project_block(&err), "[cell error: ValueError: bad value]");
    }

    // --- DX17b: headless {js} observation line interleave ---

    #[test]
    fn project_with_js_appends_the_line_after_the_cell_and_is_a_no_op_when_empty() {
        let doc = crate::render_document("Intro.\n\n```{js}\nreturn 1;\n```\n");
        let js_id = doc
            .blocks
            .iter()
            .find(|b| b.cell.as_ref().is_some_and(|c| c.lang == "js"))
            .expect("a js cell block")
            .id
            .clone();

        let mut lines = std::collections::HashMap::new();
        lines.insert(js_id, "[js: produced, <svg 320×200>]".to_string());
        let out = project_with_js(&doc.blocks, &lines);
        assert!(
            out.contains("```js\nreturn 1;\n```\n[js: produced, <svg 320×200>]"),
            "the observation line sits directly under the cell fence:\n{out}"
        );

        // An empty map must reproduce the pre-DX17b projection exactly (no stray line).
        let empty = std::collections::HashMap::new();
        assert_eq!(project(&doc.blocks), project_with_js(&doc.blocks, &empty));
        assert!(
            !project(&doc.blocks).contains("[js:"),
            "no observation line without a map entry"
        );
    }

    #[test]
    fn project_with_js_ignores_a_line_keyed_to_a_non_js_block() {
        // A stray key that isn't a {js} cell id must not leak into the projection.
        let doc = crate::render_document("# H\n\nbody\n");
        let mut lines = std::collections::HashMap::new();
        lines.insert(doc.blocks[0].id.clone(), "[js: produced]".to_string());
        assert_eq!(project(&doc.blocks), project_with_js(&doc.blocks, &lines));
    }

    // --- item 19: structure-preserving projection (lists, steps, inputs) ---

    #[test]
    fn projects_list_items_on_separate_lines() {
        let out = project_src("- **name**: the column to reference.\n- **Returns**: an `Expr`.\n");
        assert!(
            out.contains("- name: the column to reference."),
            "first item on its own line:\n{out}"
        );
        assert!(
            out.contains("- Returns: an Expr"),
            "second item separated:\n{out}"
        );
        assert!(
            !out.contains("reference.Returns"),
            "adjacent list items must not fuse:\n{out}"
        );
    }

    #[test]
    fn projects_ordered_and_nested_lists() {
        let out = project_src("1. first\n2. second\n   - nested a\n   - nested b\n");
        assert!(out.contains("1. first"), "ordered marker:\n{out}");
        assert!(out.contains("2. second"), "ordered counts up:\n{out}");
        assert!(
            out.contains("  - nested a"),
            "nested item indented two spaces:\n{out}"
        );
        assert!(
            out.contains("  - nested b"),
            "nested item indented two spaces:\n{out}"
        );
    }

    #[test]
    fn projects_scrolly_steps_as_separate_paragraphs() {
        let src = "::: {.scrolly}\n::: {.step}\nThe landscape. High on the wall.\n:::\n\n\
                   ::: {.step}\nWhich way is downhill. The gradient points across.\n:::\n:::\n";
        let out = project_src(src);
        assert!(
            out.contains("The landscape. High on the wall."),
            "step 1 text:\n{out}"
        );
        assert!(
            out.contains("Which way is downhill."),
            "step 2 text:\n{out}"
        );
        assert!(
            !out.contains("wall.Which"),
            "steps must not merge across their boundary:\n{out}"
        );
    }

    #[test]
    fn projects_input_controls_as_label_equals_value() {
        // `{{< input >}}` is a declarative shortcode, expanded by the includes pass, so
        // render through it (bare `render_document` leaves the shortcode as prose).
        let src = "{{< input name=\"lr\" type=\"slider\" min=\"0\" max=\"1\" \
                   step=\"0.01\" value=\"0.12\" label=\"step size\" >}}\n";
        let doc = crate::render_document_with_includes(src, std::path::Path::new("."));
        let out = project(&doc.blocks);
        assert!(
            out.contains("[input] step size = 0.12"),
            "input label = value:\n{out}"
        );
        assert!(
            !out.contains("size0.12"),
            "label and value must not fuse:\n{out}"
        );
    }
}
