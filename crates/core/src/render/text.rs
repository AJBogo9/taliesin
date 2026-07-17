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
    let mut out = String::new();
    for b in blocks {
        let piece = project_block(b);
        let piece = piece.trim_end_matches('\n');
        if piece.trim().is_empty() {
            continue;
        }
        out.push_str(piece);
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
}
