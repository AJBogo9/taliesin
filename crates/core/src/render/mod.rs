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
use std::path::{Path, PathBuf};

/// An executable Quarto code cell (```` ```{lang} ````), exposed so the dev
/// server can run it against a kernel.
#[derive(Debug, Clone)]
pub struct Cell {
    pub lang: String,
    pub code: String,
    /// When this cell's output should be a numbered `<figure>` (`#| label: fig-x`
    /// + `#| fig-cap:`), the executor wraps the rendered output accordingly.
    pub figure: Option<CellFigure>,
    /// `#| echo: false` hides the source (the cell still runs, output still shows).
    pub echo: bool,
    /// `#| include: false` hides both source and output, but the cell still runs
    /// (so downstream cells see its kernel state).
    pub include: bool,
}

/// Metadata for wrapping a code cell's executed output as a numbered figure.
#[derive(Debug, Clone)]
pub struct CellFigure {
    /// `#fig-…` anchor (when labelled), so `@fig-x` cross-references resolve.
    pub anchor: Option<String>,
    pub caption: Option<String>,
    pub number: usize,
}

/// A code cell's cross-reference role from its `label`/`*-cap` options.
enum CellRole {
    /// `label: fig-x` / `fig-cap` -> a numbered figure (the cell's output).
    Figure {
        anchor: Option<String>,
        caption: Option<String>,
    },
    /// `label: lst-x` / `lst-cap` -> a numbered listing (the cell's source);
    /// `fold` carries `code-fold` (start-open, summary) so a folded listing works.
    Listing {
        anchor: Option<String>,
        caption: Option<String>,
        fold: Option<(bool, String)>,
    },
}

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
    /// Present when this block is an executable code cell.
    pub cell: Option<Cell>,
}

/// The output format the document targets, taken from its front matter
/// `format:` key. Drives which page scaffold (and live client) is emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocFormat {
    /// A standard HTML page (blog post, book): the default.
    #[default]
    Html,
    /// A reveal.js slide deck (`format: revealjs` / `*-revealjs`).
    Reveal,
}

/// A rendered document: front-matter metadata plus ordered blocks.
#[derive(Debug, Clone)]
pub struct RenderedDoc {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub format: DocFormat,
    /// Whether this doc shows a table of contents. For a standalone render this
    /// is the front-matter `toc:` (default off); inside a site it is recomputed
    /// from [`SiteCtx::page_toc`] so the site default can apply.
    pub toc: bool,
    /// The page's explicit front-matter `toc:` as a tri-state: `Some(true/false)`
    /// when set, `None` when absent. The site uses this so an explicit `toc: false`
    /// overrides the site-wide default (a plain `bool` can't tell "off" from "unset").
    pub toc_explicit: Option<bool>,
    /// Resolved custom theme CSS (`.css`/`_extensions/`), empty for the built-in
    /// light/dark themes. Inlined after the base stylesheet.
    pub theme_css: String,
    /// Default theme mode for the resolver script: `"dark"`/`"light"` force it,
    /// `"auto"` follows the OS `prefers-color-scheme`.
    pub theme_default: String,
    /// Resolved `include-in-header`/`include-before-body`/`include-after-body` +
    /// `css` from the doc's front matter, injected into the page template.
    pub includes: PageIncludes,
    pub blocks: Vec<Block>,
}

/// Ready-to-inject markup from the `include-in-header` / `include-before-body` /
/// `include-after-body` / `css` front-matter (and site `format: html:`) keys.
/// Each string is already resolved (inline `text:` or a referenced file's
/// contents; `css` files wrapped in `<style>`), so the template just drops it in.
#[derive(Debug, Clone, Default)]
pub struct PageIncludes {
    pub in_header: String,
    pub before_body: String,
    pub after_body: String,
    /// Files a format extension contributes via `format-resources` (e.g. a reveal
    /// plugin's `.js`). Absolute source paths; the build copies each next to the
    /// output page (by file name) so the deck's `<script src="...">` resolves, and
    /// the preview serves them from the `_extensions/` tree.
    pub resources: Vec<PathBuf>,
}

impl PageIncludes {
    /// Append `other` after `self` (site-level first, then the page's own).
    pub fn merge(&mut self, other: &PageIncludes) {
        for (dst, src) in [
            (&mut self.in_header, &other.in_header),
            (&mut self.before_body, &other.before_body),
            (&mut self.after_body, &other.after_body),
        ] {
            if !src.is_empty() {
                dst.push_str(src);
            }
        }
        self.resources.extend(other.resources.iter().cloned());
    }
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
mod reveal;
pub use reveal::{reveal_client_head, reveal_client_script, slides_html};
mod divs;
pub(crate) use divs::parse_attrs;
use divs::{group_divs, parse_pandoc_attrs, preprocess, scan_div_spans};
mod emit;
use emit::emit;
// emit_children is re-exported so the sibling figure module reaches it via `super`.
pub(crate) use emit::emit_children;
mod figure;
use figure::{emit_figure, emit_mermaid_figure, figure_parts};
mod theme;
pub use theme::theme_head;
use theme::{detect_theme, resolve_theme, resolve_theme_layers, theme_default_mode, theme_style};

pub fn render_document(src: &str) -> RenderedDoc {
    render_internal(src, None, None)
}

/// Like [`render_document`], but first expands `{{< include >}}` shortcodes
/// relative to `base_dir`, mapping each block back to its origin file, and
/// resolves citations/cross-references against the doc's bibliography.
pub fn render_document_with_includes(src: &str, base_dir: &Path) -> RenderedDoc {
    let (expanded, origins) = crate::includes::resolve(src, base_dir);
    render_internal(&expanded, Some(&origins), Some(base_dir))
}

/// Core render. When `origins` is provided (post-include expansion), each
/// block's sourcepos and `source_file` are translated back to the originating
/// file via the line-level source map. `base_dir` (when known) is used to
/// locate the bibliography for citation resolution.
fn render_internal(
    src: &str,
    origins: Option<&[LineOrigin]>,
    base_dir: Option<&Path>,
) -> RenderedDoc {
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
    let mut subtitle: Option<String> = None;
    let mut date: Option<String> = None;
    let mut author: Option<String> = None;
    let mut description: Option<String> = None;
    let mut format = DocFormat::Html;
    let mut toc_explicit: Option<bool> = None;
    // `title-block-style: none` keeps `title` (drives `<title>`, OpenGraph, nav)
    // but skips the visible `<h1>` header (nav landing pages don't need it).
    let mut hide_title_block = false;
    let mut theme: Option<String> = None;
    let mut bib_field: Option<String> = None;
    let mut includes = PageIncludes::default();
    // Document-level cell defaults from a front-matter `execute:` block; a cell's
    // own `#| echo`/`#| include` overrides these.
    let mut exec_echo = true;
    let mut exec_include = true;
    let mut flat: Vec<FlatBlock> = Vec::new();
    let mut id_counts: HashMap<String, u32> = HashMap::new();
    // Heading anchor slugs (deduped) and the cross-reference number registry
    // (figures + equations), both used for `@sec-x`/`@fig-x`/`@eq-x` and the TOC.
    let mut heading_slugs: HashMap<String, u32> = HashMap::new();
    let mut fig_count: usize = 0;
    let mut eq_count: usize = 0;
    let mut lst_count: usize = 0;
    let mut sec_count: usize = 0;
    let mut xref_registry: HashMap<String, String> = HashMap::new();

    for node in root.children() {
        let (
            buf_start,
            sourcepos,
            source_file,
            block_src,
            is_paragraph,
            heading_level,
            mut cell,
            cell_role,
        ) = {
            let data = node.data.borrow();
            if let NodeValue::FrontMatter(fm) = &data.value {
                title = extract_field(fm, "title");
                subtitle = extract_field(fm, "subtitle");
                date = extract_field(fm, "date");
                author = extract_field(fm, "author");
                description = extract_field(fm, "description");
                bib_field = extract_field(fm, "bibliography");
                format = detect_format(fm);
                toc_explicit = detect_toc(fm);
                hide_title_block = detect_title_block_hidden(fm);
                theme = detect_theme(fm);
                // A format extension (`format: <ext>-revealjs`) contributes its
                // includes/theme first; the doc's own front matter appends/overrides.
                includes = resolve_format_extension(fm, base_dir);
                includes.merge(&resolve_doc_includes(fm, base_dir));
                (exec_echo, exec_include) = detect_execute_defaults(fm);
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
            let heading_level = match &data.value {
                NodeValue::Heading(h) => Some(h.level),
                _ => None,
            };
            // Executable Quarto cell: ```{lang} ... ``` (lang detected, options stripped).
            let cell = match &data.value {
                NodeValue::CodeBlock(cb) if cb.info.trim_start().starts_with('{') => {
                    code_lang(&cb.info).map(|lang| Cell {
                        lang,
                        code: strip_cell_options(&cb.literal),
                        figure: None,
                        echo: cell_flag_or(&cb.literal, "echo", exec_echo),
                        include: cell_flag_or(&cb.literal, "include", exec_include),
                    })
                }
                _ => None,
            };
            // A code cell labelled/captioned as a figure (`label: fig-x` / `fig-cap`)
            // or a listing (`label: lst-x` / `lst-cap`) becomes a numbered, anchored
            // `<figure>`/listing so `@fig-x` / `@lst-x` resolve.
            let cell_role = match &data.value {
                NodeValue::CodeBlock(cb) if cell.is_some() => {
                    let label = cell_option(&cb.literal, "label");
                    let fig_cap = cell_option(&cb.literal, "fig-cap");
                    let lst_cap = cell_option(&cb.literal, "lst-cap");
                    let is_fig = label.is_some_and(|l| l.starts_with("fig-")) || fig_cap.is_some();
                    let is_lst = label.is_some_and(|l| l.starts_with("lst-")) || lst_cap.is_some();
                    if is_fig {
                        Some(CellRole::Figure {
                            anchor: label.filter(|l| l.starts_with("fig-")).map(str::to_string),
                            caption: fig_cap.map(str::to_string),
                        })
                    } else if is_lst {
                        Some(CellRole::Listing {
                            anchor: label.filter(|l| l.starts_with("lst-")).map(str::to_string),
                            caption: lst_cap.map(str::to_string),
                            fold: code_fold(&cb.literal),
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            };
            (
                sp.start.line,
                sourcepos,
                file,
                slice_lines(&lines, sp.start.line, sp.end.line),
                is_paragraph,
                heading_level,
                cell,
                cell_role,
            )
        };

        let id = make_id(&block_src, &mut id_counts);
        let file_attr = match &source_file {
            Some(f) => format!(" data-source-file=\"{}\"", escape_attr(f)),
            None => String::new(),
        };
        // A heading gets a stable, deduped anchor id (HTML docs only — reveal
        // decks put the slug on the wrapping `<section>` instead, so adding it
        // here too would duplicate the id in the DOM).
        // A heading may carry a Pandoc/Quarto attribute (`## Title {#sec-x}`): use
        // an explicit `#id` as the anchor (else a slug of the cleaned text), and
        // strip the attribute from the rendered heading below.
        let h_attr = heading_level.and_then(|_| parse_heading_attr(&block_src));
        // A heading labelled `{#sec-x}` is numbered so `@sec-x` resolves to "Section N"
        // (sequential over labelled headings; full hierarchical numbering is a
        // separate `number-sections` feature).
        if let Some((_, Some(id))) = &h_attr
            && id.starts_with("sec-")
        {
            sec_count += 1;
            xref_registry.insert(id.clone(), sec_count.to_string());
        }
        let id_attr = match heading_level {
            Some(_) if format == DocFormat::Html => {
                let id = match &h_attr {
                    Some((_, Some(id))) => id.clone(),
                    Some((clean, None)) => dedup_slug(clean, &mut heading_slugs),
                    None => dedup_slug(&block_src, &mut heading_slugs),
                };
                format!(" id=\"{}\"", escape_attr(&id))
            }
            _ => String::new(),
        };
        let attrs =
            format!("{id_attr} data-block-id=\"{id}\" data-sourcepos=\"{sourcepos}\"{file_attr}");
        let mut html = String::new();
        // Quarto/pandoc treat a bare `\begin{env}...\end{env}` block as display
        // math even without `$$`; comrak doesn't, so detect and render it here.
        if let Some(env) = is_paragraph.then(|| bare_math_env(&block_src)).flatten() {
            html.push_str(&format!("<div{attrs} class=\"qmd-math-block\">"));
            html.push_str(&crate::math::render(env, true));
            html.push_str("</div>");
        } else if let Some((latex, anchor)) = is_paragraph
            .then(|| labelled_display_eq(&block_src))
            .flatten()
        {
            // `$$ ... $$ {#eq-x}` -> a numbered display equation; register the
            // `#eq-` id so `@eq-x` cross-references resolve to "Equation N".
            eq_count += 1;
            xref_registry.insert(anchor.clone(), eq_count.to_string());
            html.push_str(&emit_equation(&latex, &anchor, &attrs, eq_count));
        } else if let Some(fig) = is_paragraph.then(|| figure_parts(node)).flatten() {
            // Standalone image -> a numbered `<figure>`; register `#fig-` ids so
            // `@fig-x` cross-references resolve to the number.
            fig_count += 1;
            if let Some(fid) = fig.attrs.id.as_deref().filter(|i| i.starts_with("fig-")) {
                xref_registry.insert(fid.to_string(), fig_count.to_string());
            }
            html.push_str(&emit_figure(&fig, &attrs, fig_count));
        } else if let Some(role) = &cell_role {
            // A labelled/captioned code cell -> a numbered, anchored figure/listing.
            let lang = cell.as_ref().map(|c| c.lang.clone()).unwrap_or_default();
            let code = cell.as_ref().map(|c| c.code.clone()).unwrap_or_default();
            match role {
                CellRole::Figure { anchor, caption } => {
                    fig_count += 1;
                    if let Some(a) = anchor {
                        xref_registry.insert(a.clone(), fig_count.to_string());
                    }
                    match lang.as_str() {
                        // Client-rendered outputs are known now, so wrap them here.
                        "mermaid" => html.push_str(&emit_mermaid_figure(
                            &code,
                            anchor.as_deref(),
                            caption.as_deref(),
                            &attrs,
                            fig_count,
                        )),
                        "ojs" => html.push_str(&emit_ojs_figure(
                            &code,
                            &id,
                            anchor.as_deref(),
                            caption.as_deref(),
                            &attrs,
                            fig_count,
                        )),
                        // Python/R: the source renders now; tag the cell so the
                        // executor wraps the (later) output in the numbered figure.
                        _ => {
                            if let Some(c) = cell.as_mut() {
                                c.figure = Some(CellFigure {
                                    anchor: anchor.clone(),
                                    caption: caption.clone(),
                                    number: fig_count,
                                });
                            }
                            // `echo: false` hides the code but keeps the figure
                            // tagging, so the executed output still becomes Figure N.
                            if cell.as_ref().is_some_and(|c| !c.echo || !c.include) {
                                html.push_str(&hidden_cell(&attrs));
                            } else {
                                emit(node, &attrs, &mut html);
                            }
                        }
                    }
                }
                CellRole::Listing {
                    anchor,
                    caption,
                    fold,
                } => {
                    // A listing exists to show source, so only `include: false`
                    // (hide everything) suppresses it.
                    if cell.as_ref().is_some_and(|c| !c.include) {
                        html.push_str(&hidden_cell(&attrs));
                    } else {
                        lst_count += 1;
                        if let Some(a) = anchor {
                            xref_registry.insert(a.clone(), lst_count.to_string());
                        }
                        html.push_str(&emit_code_listing(
                            &code,
                            &lang,
                            anchor.as_deref(),
                            caption.as_deref(),
                            fold.as_ref(),
                            &attrs,
                            lst_count,
                        ));
                    }
                }
            }
        } else if let Some(c) = cell.as_ref().filter(|c| c.lang == "ojs") {
            // Live Observable cell: a placeholder the vendored runtime executes
            // client-side, instead of a static highlighted listing.
            html.push_str(&emit_ojs_cell(&c.code, &id, &attrs));
        } else if cell.as_ref().is_some_and(|c| !c.echo || !c.include) {
            // `echo: false` / `include: false`: keep the block so the executor still
            // runs it, but hide its source.
            html.push_str(&hidden_cell(&attrs));
        } else if h_attr.is_some() {
            // Heading with a Pandoc attribute: render it, then drop the trailing
            // `{#id ...}` text comrak left behind (it isn't CommonMark).
            emit(node, &attrs, &mut html);
            html = strip_heading_attr(&html);
        } else {
            emit(node, &attrs, &mut html);
            // Apply Pandoc attribute blocks trailing a link (`[t](u){.btn}`) onto
            // the `<a>`, dropping the literal `{...}` comrak left as text.
            html = apply_link_attrs(&html);
            // Drop a stray trailing `\` (a hard break at the end of a block): strict
            // CommonMark leaves it literal, but Pandoc/Quarto drop it. Match Pandoc.
            html = strip_trailing_hardbreak(&html);
        }
        flat.push(FlatBlock {
            buf_start,
            block: Block {
                id,
                sourcepos,
                source_file,
                html,
                cell,
            },
        });
    }

    let mut blocks = group_divs(flat, &spans, origins, &mut id_counts);
    // Pandoc table captions (`: caption {#tbl-x}` after a table) are numbered and
    // folded into the table's `<caption>`; registers `tbl-x` for `@tbl-` refs.
    apply_table_captions(&mut blocks, &mut xref_registry);
    let bib = load_bibliography(bib_field.as_deref(), base_dir);
    crate::cite::process(&mut blocks, &bib, &xref_registry);
    // A visible title block (HTML only; reveal builds its own title slide). It is
    // a generated block (no sourcepos), so it rides the block model + diff like
    // the References section.
    if format == DocFormat::Html
        && !hide_title_block
        && let Some(tb) = title_block_html(
            title.as_deref(),
            subtitle.as_deref(),
            author.as_deref(),
            date.as_deref(),
            description.as_deref(),
        )
    {
        blocks.insert(
            0,
            Block {
                id: "qmd-title-block".to_string(),
                sourcepos: String::new(),
                source_file: None,
                html: tb,
                cell: None,
            },
        );
    }
    let theme_css = resolve_theme(theme.as_deref(), base_dir);
    let theme_default = theme_default_mode(theme.as_deref()).to_string();
    RenderedDoc {
        title,
        subtitle,
        format,
        // Standalone default: a TOC only when the page asked for one. The site
        // path overrides this via `page_toc` using `toc_explicit`.
        toc: toc_explicit.unwrap_or(false),
        toc_explicit,
        theme_css,
        theme_default,
        includes,
        blocks,
    }
}

/// Build the visible title-block header from front-matter metadata (title +
/// optional subtitle/description and an author · date meta line). Returns `None`
/// without a title. Carries `data-block-id` so it lives in the block model.
fn title_block_html(
    title: Option<&str>,
    subtitle: Option<&str>,
    author: Option<&str>,
    date: Option<&str>,
    description: Option<&str>,
) -> Option<String> {
    let title = title?;
    let mut h = String::from(
        "<header class=\"qmd-title-block\" data-block-id=\"qmd-title-block\"><h1 class=\"title\">",
    );
    h.push_str(&html_escape(title));
    h.push_str("</h1>");
    if let Some(s) = subtitle.filter(|s| !s.is_empty()) {
        h.push_str(&format!("<p class=\"subtitle\">{}</p>", html_escape(s)));
    }
    if let Some(d) = description.filter(|s| !s.is_empty()) {
        h.push_str(&format!("<p class=\"description\">{}</p>", html_escape(d)));
    }
    let meta: Vec<String> = [author, date]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .map(|s| format!("<span>{}</span>", html_escape(s)))
        .collect();
    if !meta.is_empty() {
        h.push_str(&format!(
            "<div class=\"qmd-title-meta\">{}</div>",
            meta.join("")
        ));
    }
    h.push_str("</header>");
    Some(h)
}

/// Resolve the `include-in-header`/`include-before-body`/`include-after-body` +
/// `css` keys from a doc's front-matter YAML into ready-to-inject markup, reading
/// referenced files relative to `base_dir`.
fn resolve_doc_includes(front_matter: &str, base_dir: Option<&Path>) -> PageIncludes {
    // comrak hands us the block *with* its `---` fences; strip them so serde_yaml
    // sees a single document (the bare `---` would otherwise read as a separator).
    let body = {
        let mut lines: Vec<&str> = front_matter.lines().collect();
        while lines.first().is_some_and(|l| l.trim().is_empty()) {
            lines.remove(0);
        }
        if lines.first().is_some_and(|l| l.trim() == "---") {
            lines.remove(0);
        }
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        if lines.last().is_some_and(|l| l.trim() == "---") {
            lines.pop();
        }
        lines.join("\n")
    };
    let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&body) else {
        return PageIncludes::default();
    };
    includes_from_parts(
        v.get("include-in-header"),
        v.get("include-before-body"),
        v.get("include-after-body"),
        v.get("css"),
        base_dir,
    )
}

/// The raw `format:` key (`glass-revealjs`, `revealjs`, `html`, …) — inline value
/// or the first block sub-key. Used to spot a format-extension reference.
fn detect_format_name(front_matter: &str) -> Option<String> {
    let lines: Vec<&str> = front_matter.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some(rest) = line.trim_end().strip_prefix("format:") else {
            continue;
        };
        let inline = rest.trim();
        if !inline.is_empty() {
            return Some(inline.trim_matches(['"', '\'']).to_string());
        }
        // Block form: the first indented sub-key is the format name.
        for sub in &lines[i + 1..] {
            if sub.trim().is_empty() {
                continue;
            }
            if !sub.starts_with(char::is_whitespace) {
                break; // dedented out of the block without a sub-key
            }
            let key = sub.trim().trim_end_matches(':').trim_matches(['"', '\'']);
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
        return None;
    }
    None
}

/// If the doc's `format:` names a format extension (`<ext>-revealjs`/`<ext>-html`),
/// load `_extensions/<ext>/_extension.yml` and resolve the includes + theme its
/// `contributes: formats: <base>:` block injects, with files resolved relative to
/// the extension's own directory. Empty when there's no such extension.
fn resolve_format_extension(front_matter: &str, base_dir: Option<&Path>) -> PageIncludes {
    let Some(fmt) = detect_format_name(front_matter) else {
        return PageIncludes::default();
    };
    // Recognized base formats; the part before `-<base>` is the extension name.
    let Some((ext, base)) = ["revealjs", "html"]
        .iter()
        .find_map(|b| fmt.strip_suffix(&format!("-{b}")).map(|e| (e, *b)))
    else {
        return PageIncludes::default();
    };
    let (Some(dir), false) = (base_dir, ext.is_empty()) else {
        return PageIncludes::default();
    };
    let ext_dir = dir.join("_extensions").join(ext);
    let Ok(text) = std::fs::read_to_string(ext_dir.join("_extension.yml")) else {
        return PageIncludes::default();
    };
    let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return PageIncludes::default();
    };
    let Some(cfg) = v
        .get("contributes")
        .and_then(|c| c.get("formats"))
        .and_then(|f| f.get(base))
    else {
        return PageIncludes::default();
    };
    let mut inc = includes_from_parts(
        cfg.get("include-in-header"),
        cfg.get("include-before-body"),
        cfg.get("include-after-body"),
        cfg.get("css"),
        Some(&ext_dir),
    );
    // The contributed `theme:` CSS layers, inlined ahead of the header so the deck's
    // own front matter can still override. (`.scss` layers need a compiler we don't
    // ship yet, so only `.css` is inlined; named base themes are handled elsewhere.)
    let theme = resolve_theme_layers(cfg.get("theme"), &ext_dir);
    if !theme.is_empty() {
        inc.in_header = format!("{theme}{}", inc.in_header);
    }
    // `format-resources` (a scalar or list of file names relative to the extension)
    // are copied verbatim next to the output so an injected `<script src="x.js">`
    // resolves at runtime, rather than inlined.
    if let Some(res) = cfg.get("format-resources") {
        for name in res
            .as_sequence()
            .map(|s| s.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_else(|| res.as_str().into_iter().collect())
        {
            inc.resources.push(ext_dir.join(name));
        }
    }
    inc
}

/// Build [`PageIncludes`] from already-located YAML values for each key. Shared by
/// the single-doc front-matter path and the site `format: html:` path (which keep
/// these as typed `serde_yaml::Value` fields). `css` files are wrapped in `<style>`
/// and placed ahead of the header text so an author stylesheet can override ours.
pub fn includes_from_parts(
    in_header: Option<&serde_yaml::Value>,
    before_body: Option<&serde_yaml::Value>,
    after_body: Option<&serde_yaml::Value>,
    css: Option<&serde_yaml::Value>,
    base_dir: Option<&Path>,
) -> PageIncludes {
    let mut head = resolve_include_value(css, base_dir, true);
    head.push_str(&resolve_include_value(in_header, base_dir, false));
    PageIncludes {
        in_header: head,
        before_body: resolve_include_value(before_body, base_dir, false),
        after_body: resolve_include_value(after_body, base_dir, false),
        resources: Vec::new(),
    }
}

/// Resolve one include value: a path string (file contents), a `{text: …}` or
/// `{file: …}` map, or a list of those. `css == true` wraps each resolved chunk
/// in a `<style>` block; otherwise the markup is injected verbatim.
fn resolve_include_value(
    v: Option<&serde_yaml::Value>,
    base_dir: Option<&Path>,
    css: bool,
) -> String {
    let mut out = String::new();
    if let Some(v) = v {
        resolve_include_into(v, base_dir, css, &mut out);
    }
    out
}

fn resolve_include_into(
    v: &serde_yaml::Value,
    base_dir: Option<&Path>,
    css: bool,
    out: &mut String,
) {
    use serde_yaml::Value;
    match v {
        Value::String(s) => append_include(&read_include_file(base_dir, s), css, out),
        Value::Mapping(_) => {
            if let Some(Value::String(t)) = v.get("text") {
                append_include(t, css, out);
            } else if let Some(Value::String(f)) = v.get("file") {
                append_include(&read_include_file(base_dir, f), css, out);
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                resolve_include_into(item, base_dir, css, out);
            }
        }
        _ => {}
    }
}

fn append_include(content: &str, css: bool, out: &mut String) {
    if css {
        out.push_str("<style>\n");
        out.push_str(content);
        out.push_str("\n</style>\n");
    } else {
        out.push_str(content);
        out.push('\n');
    }
}

/// Read an include/css file relative to the doc (or site root). A missing file is
/// reported as an HTML comment rather than aborting the render (warn, don't reject).
fn read_include_file(base_dir: Option<&Path>, rel: &str) -> String {
    let path = match base_dir {
        Some(dir) => dir.join(rel),
        None => PathBuf::from(rel),
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => format!(
            "<!-- qmd-fast: include file not found: {} -->",
            esc_comment(rel)
        ),
    }
}

/// Sanitize a path for an HTML comment (no `--`, which would close the comment).
fn esc_comment(s: &str) -> String {
    s.replace("--", "__")
}

/// The built-in dark theme, scoped to `html[data-theme="dark"]` so it can be
/// flipped at runtime (the toggle / OS preference set the attribute). Always
/// shipped alongside the light `:root` base. The `:root` light vars plus this
/// block are the reference template for a community theme.
const DARK_CSS: &str = include_str!("../../assets/css/dark.css");

/// The front-matter `toc:` setting (typically under `format: html:`) as a
/// tri-state: `Some(true)`/`Some(false)` when the page sets it, `None` when
/// absent. A lightweight scan, matching the corpus book's usage. Returning
/// `Option` lets a site distinguish an explicit `toc: false` (which overrides
/// the site default) from an unset toc (which inherits it).
fn detect_toc(front_matter: &str) -> Option<bool> {
    front_matter.lines().find_map(|l| {
        let t = l.trim();
        match t.strip_prefix("toc:").map(str::trim) {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        }
    })
}

/// `title-block-style: none` suppresses the visible title-block header while
/// keeping the `title` metadata (Quarto-compatible). Used by nav landing pages
/// (Blog/Projects/Publications) where a big `<h1>` repeats the navbar.
fn detect_title_block_hidden(front_matter: &str) -> bool {
    front_matter.lines().any(|l| {
        let t = l.trim();
        t.strip_prefix("title-block-style:").map(str::trim) == Some("none")
    })
}

/// Detect the output format from raw front matter. A reveal.js deck declares a
/// `format:` whose inline value or indented sub-keys name a revealjs variant
/// (`revealjs`, `liquid-glass-revealjs`, …). Everything else is a standard page.
fn detect_format(front_matter: &str) -> DocFormat {
    let lines: Vec<&str> = front_matter.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        // Only consider the top-level `format:` key, not nested ones.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some(rest) = line.trim_end().strip_prefix("format:") else {
            continue;
        };
        let inline = rest.trim();
        if !inline.is_empty() {
            // `format: revealjs`
            return reveal_if(inline.contains("revealjs"));
        }
        // Block form: scan the indented sub-keys until the block dedents.
        for sub in &lines[i + 1..] {
            if sub.trim().is_empty() {
                continue;
            }
            if !sub.starts_with(char::is_whitespace) {
                break;
            }
            if sub.contains("revealjs") {
                return DocFormat::Reveal;
            }
        }
        return DocFormat::Html;
    }
    DocFormat::Html
}

fn reveal_if(cond: bool) -> DocFormat {
    if cond {
        DocFormat::Reveal
    } else {
        DocFormat::Html
    }
}

/// Load and merge the bibliography file(s) named in the front matter, resolved
/// relative to `base_dir`. Returns an empty bibliography when none is found
/// (citations still de-leak; cross-references still resolve).
fn load_bibliography(field: Option<&str>, base_dir: Option<&Path>) -> crate::cite::Bibliography {
    let (Some(field), Some(base)) = (field, base_dir) else {
        return crate::cite::Bibliography::default();
    };
    let mut text = String::new();
    for tok in field.split([',', '[', ']', ' ']) {
        let tok = tok.trim().trim_matches(['"', '\'']);
        if tok.ends_with(".bib")
            && let Ok(content) = std::fs::read_to_string(base.join(tok))
        {
            text.push_str(&content);
            text.push('\n');
        }
    }
    crate::cite::parse_bib(&text)
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
    page_from_doc(
        &render_document_with_includes(src, base_dir),
        fallback_title,
    )
}

/// Self-contained KaTeX stylesheet (fonts inlined as data URIs at build time).
const KATEX_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/katex-inlined.css"));

/// Base document styling (typography, tables, callouts, references, block
/// highlight). Shared by the one-shot page and the live preview client.
const BASE_CSS: &str = include_str!("../../assets/css/base.css");

/// `<style>` block(s) for the live preview client: base styling plus the
/// (self-contained) KaTeX stylesheet, since a live doc may gain math at any edit.
pub fn client_styles() -> String {
    format!("<style>{BASE_CSS}{DARK_CSS}</style>\n<style>{KATEX_CSS}</style>")
}

/// The multi-page site chrome CSS (navbar / footer / prev-next), wrapped in a
/// `<style>`. The live site preview ships this on top of [`client_styles`];
/// the static build folds it into the page template directly.
pub fn site_styles() -> String {
    format!("<style>{SITE_CSS}</style>")
}

// highlight.js + mermaid (pinned) served from jsDelivr — the dev server runs
// locally with network access, like reveal.js. Both are client-side presentation
// layers, so they never affect the block model or the diff. mermaid is loaded
// lazily (only when a diagram is actually present).
const MERMAID: &str = "https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.min.js";

/// `<head>` stylesheet link for code syntax highlighting (the highlight.js theme).
pub fn code_head() -> String {
    // Syntax highlighting is done server-side (see `crate::highlight`); the colors
    // live in the base stylesheet, so there's no highlighter CSS to load.
    String::new()
}

/// Defines `window.qmdEnhanceCode(root)`, which gives each code block a copy button
/// and renders any `<pre class="mermaid">` diagrams (lazy-loading mermaid.js on
/// first use). Syntax highlighting is no longer done here — code arrives already
/// highlighted from the server. Callers invoke it after (re)mounting content; it is
/// idempotent (skips already-processed blocks).
pub fn code_scripts() -> String {
    let js = CODE_ENHANCE_JS.replace("{{MERMAID}}", MERMAID);
    format!("<script>{js}</script>")
}

/// The canonical TOC scrollspy (highlights the section under the navbar). Shared
/// so the static build and the live preview behave identically: the static build
/// inlines this once (it auto-inits on load); the preview also ships it and calls
/// `window.qmdInitTocSpy()` after each TOC rebuild. Emitted only on TOC pages.
pub const TOC_SPY_JS: &str = include_str!("../../../../web-client/toc-spy.js");

pub fn toc_scripts() -> String {
    format!("<script>{TOC_SPY_JS}</script>\n<script>{SEARCH_JS}</script>")
}

/// Cmd/Ctrl-K command palette to search the document's headings. Rides along on
/// pages that have a table of contents (the long ones: the book, a paper), where
/// jumping between sections matters most.
pub const SEARCH_JS: &str = include_str!("../../../../web-client/search.js");

// Quarto's Observable runtime (vendored, v0.0.18 — not published to any CDN, so
// unlike hljs/reveal it must ship with us). It self-installs `window._ojs` on
// load and drives cells via `interpretFromScriptTags()`. Loaded as a module so
// execution is deferred until <body> exists (the bundle touches document.body).
const OJS_RUNTIME: &str = include_str!("../../assets/ojs/quarto-ojs-runtime.min.js");
const OJS_CSS: &str = include_str!("../../assets/ojs/quarto-ojs.css");

/// `<head>` assets for Observable cells: the runtime CSS + the runtime bundle.
/// Emit only when a page actually has `{ojs}` cells.
pub fn ojs_head() -> String {
    format!("<style>{OJS_CSS}</style>\n<script type=\"module\">{OJS_RUNTIME}</script>")
}

/// The init script (run after the bundle + after cells are in the DOM): point
/// the module resolver at the doc dir and interpret every `ojs-module-contents`
/// script. Exposed as `window.qmdRunOJS()` so the live client can call it after
/// (re)mounting blocks; also invoked once on load for the one-shot page.
pub fn ojs_init() -> String {
    OJS_INIT.to_string()
}

const OJS_INIT: &str = include_str!("../../assets/js/ojs-init.html");

/// True if a rendered body contains live Observable cells (gates the OJS assets).
pub fn has_ojs(body: &str) -> bool {
    body.contains("ojs-module-contents")
}

const CODE_ENHANCE_JS: &str = include_str!("../../assets/js/code-enhance.js");

fn page_from_doc(doc: &RenderedDoc, fallback_title: &str) -> String {
    match doc.format {
        DocFormat::Reveal => reveal::reveal_page_from_doc(doc, fallback_title),
        DocFormat::Html => html_page_from_doc(doc, fallback_title),
    }
}

/// Render an already-built [`RenderedDoc`] into a standalone HTML page (no site
/// chrome). Lets the `build` CLI run code cells first and then emit the page from
/// the executed blocks; the in-process [`render_html_page`] path stays unchanged.
pub fn render_doc_to_page(doc: &RenderedDoc, fallback_title: &str) -> String {
    page_from_doc(doc, fallback_title)
}

/// Shared chrome for a page rendered inside a multi-page site: pre-built navbar,
/// footer, and post prev/next HTML. Built by `qmd_fast_core::site` and injected
/// around the page body. Empty fields render nothing.
#[derive(Debug, Clone, Default)]
pub struct SiteCtx {
    pub navbar_html: String,
    pub footer_html: String,
    pub post_nav_html: String,
    /// `page-layout: full` — widen the content column (for listing indexes).
    pub wide: bool,
    /// Site-level `format: html:` includes (header/body/css from `_quarto.yml`),
    /// merged ahead of each page's own front-matter includes.
    pub includes: PageIncludes,
    /// Site `favicon:` resolved to a path relative to this page's depth (empty if
    /// none configured), emitted as `<link rel="icon">`.
    pub favicon: String,
}

fn html_page_from_doc(doc: &RenderedDoc, fallback_title: &str) -> String {
    html_page_inner(doc, fallback_title, None)
}

/// Like [`html_page_from_doc`], but wraps the page body in the site chrome
/// (navbar above, prev/next + footer below) and ships the site CSS. The
/// single-page path ([`html_page_from_doc`]) is unchanged (`site == None`).
pub fn html_page_from_doc_in_site(
    doc: &RenderedDoc,
    fallback_title: &str,
    site: &SiteCtx,
) -> String {
    html_page_inner(doc, fallback_title, Some(site))
}

fn html_page_inner(doc: &RenderedDoc, fallback_title: &str, site: Option<&SiteCtx>) -> String {
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
    // Only ship the Observable runtime + init when the page has live OJS cells.
    let (ojs_head_html, ojs_init_html) = if has_ojs(&body) {
        (ojs_head(), ojs_init())
    } else {
        (String::new(), String::new())
    };
    // With `toc: true`, lay the content beside a sticky table of contents.
    let toc = if doc.toc {
        toc_html(&doc.blocks)
    } else {
        String::new()
    };
    // The scrollspy script rides along only on pages that actually have a TOC.
    let toc_script = if toc.is_empty() {
        String::new()
    } else {
        toc_scripts()
    };
    // Content first (left, wide column), TOC second (right, sticky column).
    let (mut body_class, content) = if toc.is_empty() {
        (String::new(), body)
    } else {
        (
            " class=\"has-toc\"".to_string(),
            format!("<main>\n{body}</main>\n{toc}\n"),
        )
    };
    // Site mode: body becomes a full-width flex column (navbar, a centred content
    // wrapper, footer) so the footer sits at the bottom of short pages and the
    // chrome lines up with the reading column. The `has-toc` grid moves onto the
    // wrapper, leaving the body free to be the flex shell.
    let body_content = match site {
        Some(s) => {
            let mut main_cls = String::from("qmd-site-main");
            if !toc.is_empty() {
                main_cls.push_str(" has-toc");
            }
            if s.wide {
                main_cls.push_str(" qmd-wide");
            }
            body_class = " class=\"qmd-site\"".to_string();
            format!(
                "{nav}\n<div class=\"{main_cls}\">\n{content}{post_nav}</div>\n{footer}\n",
                nav = s.navbar_html,
                post_nav = s.post_nav_html,
                footer = s.footer_html,
            )
        }
        None => content,
    };
    let base_css = match site {
        Some(_) => format!("{BASE_CSS}{DARK_CSS}{SITE_CSS}"),
        None => format!("{BASE_CSS}{DARK_CSS}"),
    };
    // Site-level `format: html:` includes (from `_quarto.yml`) apply to every page
    // first; the page's own front-matter includes follow.
    let includes = match site {
        Some(s) => {
            let mut merged = s.includes.clone();
            merged.merge(&doc.includes);
            merged
        }
        None => doc.includes.clone(),
    };
    let favicon = match site {
        Some(s) if !s.favicon.is_empty() => favicon_link(&s.favicon),
        _ => String::new(),
    };
    PAGE_TEMPLATE
        .replace("{{TITLE}}", &t)
        .replace("{{FAVICON}}", &favicon)
        .replace("{{THEME_INIT}}", &theme_head(&doc.theme_default))
        .replace("{{KATEX_CSS}}", &katex_css)
        .replace("{{BASE_CSS}}", &base_css)
        .replace("{{THEME_CSS}}", &theme_style(&doc.theme_css))
        .replace("{{CODE_HEAD}}", &code_head())
        .replace("{{OJS_HEAD}}", &ojs_head_html)
        .replace("{{INCLUDE_IN_HEADER}}", &includes.in_header)
        .replace("{{BODY_CLASS}}", &body_class)
        .replace("{{INCLUDE_BEFORE_BODY}}", &includes.before_body)
        .replace("{{BODY}}", &body_content)
        .replace("{{CODE_SCRIPTS}}", &code_scripts())
        .replace("{{TOC_SCRIPT}}", &toc_script)
        .replace("{{OJS_INIT}}", &ojs_init_html)
        .replace("{{INCLUDE_AFTER_BODY}}", &includes.after_body)
}

/// A `<link rel="icon">` for the given href, with a `type` inferred from the
/// extension (svg/png/x-icon) so SVG favicons render. Shared by the static build
/// and the live preview.
pub fn favicon_link(href: &str) -> String {
    let ty = match href
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("svg") => " type=\"image/svg+xml\"",
        Some("png") => " type=\"image/png\"",
        Some("ico") => " type=\"image/x-icon\"",
        _ => "",
    };
    let mut h = String::new();
    escape_html(href, &mut h);
    format!("<link rel=\"icon\"{ty} href=\"{h}\" />")
}

/// Heading level (1–6) for a block whose root element is `<hN ...>`/`<hN>`.
fn block_heading_level(html: &str) -> Option<u8> {
    let b = html.as_bytes();
    if b.len() >= 4 && b[0] == b'<' && b[1] == b'h' && b[2].is_ascii_digit() {
        let lvl = b[2] - b'0';
        if (1..=6).contains(&lvl) && matches!(b[3], b' ' | b'>') {
            return Some(lvl);
        }
    }
    None
}

/// A reveal slide separator: a thematic break (`<hr ...>`).
/// GitHub-style slug for a heading's visible text, used as the `<section>` id.
fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            if pending_dash {
                out.push('-');
                pending_dash = false;
            }
            out.extend(ch.to_lowercase());
        } else if !out.is_empty() {
            pending_dash = true;
        }
    }
    out
}

/// A deduped heading anchor slug; a repeated slug gets a `-N` suffix (Quarto).
/// `block_src` is the heading's markdown line; `slugify` ignores the leading
/// `#`s and markup, yielding the visible-text slug.
fn dedup_slug(block_src: &str, counts: &mut HashMap<String, u32>) -> String {
    let base = slugify(block_src);
    let base = if base.is_empty() {
        "section".to_string()
    } else {
        base
    };
    let n = counts.entry(base.clone()).or_insert(0);
    let slug = if *n == 0 {
        base.clone()
    } else {
        format!("{base}-{n}")
    };
    *n += 1;
    slug
}

/// A trailing Pandoc attribute on a heading line (`## Title {#id .class}`).
/// Returns `(text_without_attr, explicit_id)`, or `None` when there is no attr.
fn parse_heading_attr(block_src: &str) -> Option<(String, Option<String>)> {
    let line = block_src.trim_end();
    let open = line.rfind('{')?;
    if !line.ends_with('}') {
        return None;
    }
    let inner = &line[open + 1..line.len() - 1];
    // Require an id or class so we don't strip a heading that merely ends in `}`.
    if !(inner.starts_with('#') || inner.starts_with('.')) {
        return None;
    }
    let id = inner
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix('#'))
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    Some((line[..open].trim_end().to_string(), id))
}

/// Remove the trailing `{...}` attribute comrak leaves as literal text inside a
/// rendered heading (heading attributes aren't CommonMark, so it doesn't consume
/// them). Only called when [`parse_heading_attr`] found one.
fn strip_heading_attr(html: &str) -> String {
    let Some(close) = html.rfind("</h") else {
        return html.to_string();
    };
    let inner = &html[..close];
    if inner.trim_end().ends_with('}')
        && let Some(open) = inner.rfind('{')
    {
        return format!("{}{}", inner[..open].trim_end(), &html[close..]);
    }
    html.to_string()
}

/// Apply a Pandoc/Quarto attribute block trailing a link — `<a ...>text</a>{.btn #id}`
/// — onto the `<a>` (merging classes + setting an id) and drop the literal `{...}`
/// comrak leaves as text. The inline analogue of [`strip_heading_attr`]; only
/// `.class`/`#id` blocks are consumed (anything else, e.g. `{x}`, is left as-is).
fn apply_link_attrs(html: &str) -> String {
    if !html.contains("</a>") {
        return html.to_string();
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find("</a>") {
        let end = pos + "</a>".len();
        out.push_str(&rest[..end]);
        let after = &rest[end..];
        let trimmed = after.trim_start();
        if let Some(body) = trimmed.strip_prefix('{')
            && let Some(close) = body.find('}')
            && let Some((classes, id)) = parse_pandoc_attrs(&body[..close])
        {
            inject_attrs_into_last_tag(&mut out, "<a ", &classes, id.as_deref());
            rest = &body[close + 1..];
            continue;
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Drop a literal backslash left right before a block-closing tag — a trailing
/// `\` hard break that comrak keeps (CommonMark) but Pandoc/Quarto drop (e.g. a
/// CV line ending `2025–2027 \`). Scoped to block closers so inline `\` is untouched.
fn strip_trailing_hardbreak(html: &str) -> String {
    let mut out = html.to_string();
    for tag in [
        "</p>",
        "</li>",
        "</h1>",
        "</h2>",
        "</h3>",
        "</h4>",
        "</h5>",
        "</h6>",
        "</blockquote>",
        "</td>",
        "</dd>",
    ] {
        if out.contains('\\') {
            out = out.replace(&format!("\\{tag}"), tag);
            out = out.replace(&format!("\\ {tag}"), tag);
        }
    }
    out
}

/// Insert `class`/`id` attributes into the last opening `tag` already written to
/// `out` (e.g. the `<a ` that precedes a just-emitted `</a>`).
fn inject_attrs_into_last_tag(out: &mut String, tag: &str, classes: &[String], id: Option<&str>) {
    let Some(start) = out.rfind(tag) else {
        return;
    };
    let Some(rel_gt) = out[start..].find('>') else {
        return;
    };
    let gt = start + rel_gt;
    let mut ins = String::new();
    if !classes.is_empty() {
        ins.push_str(&format!(" class=\"{}\"", escape_attr(&classes.join(" "))));
    }
    if let Some(i) = id {
        ins.push_str(&format!(" id=\"{}\"", escape_attr(i)));
    }
    out.insert_str(gt, &ins);
}

/// Fold Pandoc table captions into their tables. A `: caption {#tbl-x}` paragraph
/// directly after a table becomes the table's numbered `<caption>` ("Table N"),
/// the table gains the `#tbl-x` id, and `tbl-x` is registered so `@tbl-x` resolves.
fn apply_table_captions(blocks: &mut Vec<Block>, xrefs: &mut HashMap<String, String>) {
    let mut tbl_count = 0u32;
    let mut i = 0;
    while i + 1 < blocks.len() {
        if blocks[i].html.starts_with("<table")
            && let Some((caption_html, id)) = parse_table_caption(&blocks[i + 1].html)
        {
            tbl_count += 1;
            if let Some(id) = &id {
                xrefs.insert(id.clone(), tbl_count.to_string());
            }
            let sep = if caption_html.is_empty() { "" } else { ": " };
            let id_attr = id
                .as_deref()
                .map(|x| format!(" id=\"{}\"", escape_attr(x)))
                .unwrap_or_default();
            // Insert the `id` on the <table> and a <caption> as its first child.
            let table = &blocks[i].html;
            let gt = table.find('>').unwrap_or(0) + 1;
            let open = table[..gt].replacen("<table", &format!("<table{id_attr}"), 1);
            blocks[i].html = format!(
                "{open}<caption>Table&nbsp;{tbl_count}{sep}{caption_html}</caption>{}",
                &table[gt..],
            );
            blocks.remove(i + 1); // the caption paragraph is now folded in
        }
        i += 1;
    }
}

/// Parse a table-caption paragraph (`<p ...>: caption {#tbl-x}</p>`): returns the
/// caption's inner HTML (markers stripped) and an explicit `#id`, or `None`.
fn parse_table_caption(p_html: &str) -> Option<(String, Option<String>)> {
    if !(p_html.starts_with("<p ") || p_html.starts_with("<p>")) {
        return None; // a paragraph (not <pre>, etc.)
    }
    let gt = p_html.find('>')?;
    let body = p_html[gt + 1..].strip_suffix("</p>")?.trim_start();
    let body = body.strip_prefix(':')?.trim(); // the `: caption` marker
    Some(match parse_heading_attr(body) {
        Some((clean, id)) => (clean.trim().to_string(), id),
        None => (body.to_string(), None),
    })
}

// --- figures -------------------------------------------------------------

// --- table of contents ---------------------------------------------------

/// Build a `<nav id="TOC">` from the document's heading blocks (levels 1–3),
/// linking to their anchor ids. Empty when the doc has no anchored headings.
fn toc_html(blocks: &[Block]) -> String {
    let mut items: Vec<(u8, String, String)> = Vec::new();
    for b in blocks {
        if let (Some(level), Some(id)) = (block_heading_level(&b.html), extract_attr(&b.html, "id"))
            && level <= 3
        {
            items.push((level, id, strip_tags(&b.html)));
        }
    }
    if items.is_empty() {
        return String::new();
    }
    let base = items.iter().map(|(l, _, _)| *l).min().unwrap();
    let mut out = String::from("<nav id=\"TOC\" class=\"qmd-toc\" role=\"doc-toc\"><ul>");
    let mut level = base;
    let mut open_li = false;
    for (lvl, id, text) in &items {
        let lvl = (*lvl).max(base);
        if lvl > level {
            // Descend: open nested lists inside the still-open parent <li>.
            while level < lvl {
                out.push_str("<ul>");
                level += 1;
            }
        } else {
            if open_li {
                out.push_str("</li>");
            }
            while level > lvl {
                out.push_str("</ul></li>");
                level -= 1;
            }
        }
        out.push_str(&format!(
            "<li><a href=\"#{}\">{}</a>",
            escape_attr(id),
            html_escape(text)
        ));
        open_li = true;
    }
    if open_li {
        out.push_str("</li>");
    }
    while level > base {
        out.push_str("</ul></li>");
        level -= 1;
    }
    out.push_str("</ul></nav>");
    out
}

/// Read the value of an HTML attribute from a start tag (e.g. `id="..."`).
fn extract_attr(html: &str, name: &str) -> Option<String> {
    let needle = format!(" {name}=\"");
    let start = html.find(&needle)? + needle.len();
    let end = html[start..].find('"')? + start;
    Some(html[start..end].to_string())
}

// --- emitter -------------------------------------------------------------

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

/// Parsed fenced-div attributes. Kept here (rather than in the `divs` submodule)
/// because the sibling `figure` module reads its fields via descendant access.
#[derive(Default)]
pub(crate) struct DivAttrs {
    classes: Vec<String>,
    id: Option<String>,
    kv: Vec<(String, String)>,
}

impl DivAttrs {
    fn get(&self, key: &str) -> Option<&str> {
        self.kv
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
    fn callout_kind(&self) -> Option<&str> {
        self.classes.iter().find_map(|c| c.strip_prefix("callout-"))
    }
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

/// Escape a string for HTML *text* content (`&`, `<`, `>`). For attribute values
/// (which also need `"`), use [`escape_attr`]. Shared with the server crate's
/// executor/kernel output rendering so escaping is defined once.
pub fn html_escape(s: &str) -> String {
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

/// A Quarto-labelled display equation: `$$ ... $$ {#eq-x}`. Returns the LaTeX
/// body and the `eq-x` anchor. Only `#eq-`-prefixed labels qualify (other
/// attribute blocks after `$$` are left to the normal math path).
fn labelled_display_eq(block_src: &str) -> Option<(String, String)> {
    let t = block_src.trim();
    let body = t.strip_prefix("$$")?;
    let close = body.rfind("$$")?;
    let (latex, after) = body.split_at(close);
    let attr = after.strip_prefix("$$")?.trim();
    let inner = attr.strip_prefix('{')?.strip_suffix('}')?;
    let anchor = inner
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix('#'))?;
    anchor
        .starts_with("eq-")
        .then(|| (latex.trim().to_string(), anchor.to_string()))
}

/// Render a numbered display equation: the KaTeX body plus a right-aligned
/// `(N)` number, carrying the `#eq-` id so `@eq-x` cross-refs link to it.
fn emit_equation(latex: &str, anchor: &str, block_attrs: &str, num: usize) -> String {
    format!(
        "<div id=\"{anchor}\"{block_attrs} class=\"qmd-eqn\">\
         <span class=\"qmd-eqn-body\">{}</span>\
         <span class=\"qmd-eqn-number\">({num})</span></div>",
        crate::math::render(latex, true)
    )
}

/// Read a leading `#| key: value` cell option (returns the unquoted value).
/// Only scans the contiguous leading option block, stopping at the first code
/// line. Recognizes `#|` (most langs), `//|` (OJS/JS), and `%%|` (mermaid).
fn cell_option<'a>(literal: &'a str, key: &str) -> Option<&'a str> {
    for line in literal.lines() {
        let t = line.trim_start();
        let Some(opt) = t
            .strip_prefix("#|")
            .or_else(|| t.strip_prefix("//|"))
            .or_else(|| t.strip_prefix("%%|"))
        else {
            break;
        };
        if let Some((k, v)) = opt.split_once(':')
            && k.trim() == key
        {
            return Some(v.trim().trim_matches(['"', '\'']));
        }
    }
    None
}

/// A boolean cell option (`#| echo: false`) that falls back to a document default
/// (from `execute:`) when the cell doesn't set it. Only an explicit `false` turns
/// it off, so Quarto's `echo: fenced` etc. still count as "shown".
fn cell_flag_or(literal: &str, key: &str, default: bool) -> bool {
    match cell_option(literal, key) {
        Some("false") => false,
        Some(_) => true,
        None => default,
    }
}

/// Document-level cell defaults from a front-matter `execute:` block:
///
/// ```yaml
/// execute:
///   echo: false
///   include: false
/// ```
///
/// Returns `(echo, include)`, each defaulting to `true`. Per-cell `#|` options
/// override these. (`eval`/`output`/`warning`/`cache` are not yet honoured.)
fn detect_execute_defaults(front_matter: &str) -> (bool, bool) {
    let (mut echo, mut include) = (true, true);
    let mut in_block = false;
    for line in front_matter.lines() {
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();
        if !in_block {
            if indent == 0 && t.starts_with("execute:") {
                in_block = true;
            }
            continue;
        }
        if t.is_empty() {
            continue;
        }
        if indent == 0 {
            break; // dedent ends the block
        }
        if let Some((k, v)) = t.split_once(':') {
            let v = v.trim().trim_matches(['"', '\'']);
            match k.trim() {
                "echo" => echo = v != "false",
                "include" => include = v != "false",
                _ => {}
            }
        }
    }
    (echo, include)
}

/// A code cell whose source is suppressed (`#| echo: false` / `#| include: false`)
/// still needs a block in the list so the executor runs it and the output can be
/// placed after it; render it as an empty hidden marker carrying the data attrs.
fn hidden_cell(attrs: &str) -> String {
    format!("<div{attrs} class=\"qmd-cell-hidden\" hidden></div>")
}

/// If a cell sets `code-fold`, return `(start_open, summary)`. `true` folds
/// (starts closed), `show` folds but starts open; `code-summary` overrides the
/// "Code" label.
fn code_fold(literal: &str) -> Option<(bool, String)> {
    let v = cell_option(literal, "code-fold")?;
    if v != "true" && v != "show" {
        return None;
    }
    let summary = cell_option(literal, "code-summary")
        .unwrap_or("Code")
        .to_string();
    Some((v == "show", summary))
}

/// Drop leading Quarto cell-option lines (`#|` for most languages, `//|` for
/// OJS/JS, `%%|` for mermaid).
fn strip_cell_options(literal: &str) -> String {
    let mut body = String::new();
    let mut skipping = true;
    for line in literal.lines() {
        let t = line.trim_start();
        if skipping && (t.starts_with("#|") || t.starts_with("//|") || t.starts_with("%%|")) {
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

/// Extract a top-level `key:` value from raw front matter. Lightweight scan,
/// not a YAML parse; returns the inline value (empty for block/list values).
fn extract_field(front_matter: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    for line in front_matter.lines() {
        // top-level keys only (not indented sub-keys)
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some(rest) = line.trim().strip_prefix(&prefix) {
            let v = rest.trim().trim_matches(['"', '\'']).trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Language for a fenced block: `{python}`/`{.python}`/`{ojs}` -> "python"/"ojs",
/// plain ` ```rust ` -> "rust". Pandoc raw-output attributes (`{=html}`,
/// `{=latex}`, ...) are not languages and return `None`.
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
    if lang.is_empty() || lang.starts_with('=') {
        return None;
    }
    Some(lang.to_string())
}

/// The raw output format of a Pandoc passthrough fence: `{=html}` -> "html".
fn raw_block_format(info: &str) -> Option<String> {
    info.trim()
        .strip_prefix("{=")
        .and_then(|s| s.strip_suffix('}'))
        .map(|f| f.trim().to_ascii_lowercase())
        .filter(|f| !f.is_empty())
}

/// Minimal standard-alphabet base64 (mirrors `build.rs`); encodes the OJS
/// module-contents JSON the way the runtime's `base64ToStr` (base64 → UTF-8)
/// expects.
fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        s.push(T[(n >> 18 & 63) as usize] as char);
        s.push(T[(n >> 12 & 63) as usize] as char);
        s.push(if chunk.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}

/// Serialize a string as a JSON string literal (quoted + escaped).
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Emit a live Observable cell: an output target div (id == cellName) plus a
/// base64 `ojs-module-contents` script the runtime interprets into it. The block
/// data attrs ride on the wrapper so click-to-source still keys off the block.
fn emit_ojs_cell(src: &str, block_id: &str, block_attrs: &str) -> String {
    let cell_name = format!("ojs-cell-{block_id}");
    let json = format!(
        "{{\"contents\":[{{\"methodName\":\"interpret\",\"cellName\":{},\"inline\":false,\"source\":{}}}]}}",
        json_string(&cell_name),
        json_string(src),
    );
    let b64 = base64_encode(json.as_bytes());
    // A pure named declaration (`foo = …`) feeds other cells and shouldn't display
    // its inspector value; tag it so the vendored OJS CSS hides the output (viewof
    // and bare-expression cells stay visible). Mirrors Quarto's `nodetype`.
    let nodetype = if ojs_is_declaration(src) {
        " nodetype=\"declaration\""
    } else {
        ""
    };
    // The vendored Observable runtime walks up to an ancestor with class `cell`
    // to render a cell error (and bails with a crash if it finds none). The extra
    // class costs nothing for healthy cells and lets errors degrade to an inline
    // callout instead of an uncaught `locatePreDiv` TypeError.
    format!(
        "<div{block_attrs}{nodetype} class=\"cell ojs-cell\"><div id=\"{cell_name}\"></div>\
         <script type=\"ojs-module-contents\">{b64}</script></div>"
    )
}

/// Wrap a live OJS cell in a numbered `<figure>` (for `label: fig-x` OJS cells,
/// e.g. a Three.js scene). The block attrs + `#fig-` anchor ride on the figure.
fn emit_ojs_figure(
    src: &str,
    block_id: &str,
    anchor: Option<&str>,
    caption: Option<&str>,
    block_attrs: &str,
    num: usize,
) -> String {
    let cell = emit_ojs_cell(src, block_id, "");
    let id_attr = anchor
        .map(|a| format!(" id=\"{}\"", escape_attr(a)))
        .unwrap_or_default();
    let figcap = match caption.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("Figure&nbsp;{num}: {}", html_escape(c)),
        None => format!("Figure&nbsp;{num}"),
    };
    format!(
        "<figure{block_attrs}{id_attr} class=\"qmd-figure qmd-figure-center\">\
         {cell}<figcaption>{figcap}</figcaption></figure>"
    )
}

/// Render a labelled code cell's source as a numbered listing (`@lst-x`),
/// caption above the code. The block attrs + `#lst-` anchor ride on the wrapper.
fn emit_code_listing(
    code: &str,
    lang: &str,
    anchor: Option<&str>,
    caption: Option<&str>,
    fold: Option<&(bool, String)>,
    block_attrs: &str,
    num: usize,
) -> String {
    let id_attr = anchor
        .map(|a| format!(" id=\"{}\"", escape_attr(a)))
        .unwrap_or_default();
    let class = if lang.is_empty() {
        String::new()
    } else {
        format!(" class=\"language-{lang}\"")
    };
    let code_html = crate::highlight::highlight(code, (!lang.is_empty()).then_some(lang));
    let figcap = match caption.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("Listing&nbsp;{num}: {}", html_escape(c)),
        None => format!("Listing&nbsp;{num}"),
    };
    // `code-fold` collapses the listing's source behind its summary.
    let code_html = match fold {
        Some((open, summary)) => format!(
            "<details class=\"qmd-code-fold\"{}><summary>{}</summary><pre><code{class}>{code_html}</code></pre></details>",
            if *open { " open" } else { "" },
            html_escape(summary),
        ),
        None => format!("<pre><code{class}>{code_html}</code></pre>"),
    };
    format!(
        "<div{block_attrs}{id_attr} class=\"qmd-listing\">\
         <figcaption class=\"qmd-listing-caption\">{figcap}</figcaption>{code_html}</div>"
    )
}

/// Heuristic: does this `{ojs}` cell start with a named declaration whose value
/// shouldn't be shown — `name = …`, `function name(…)`, `async function name(…)`,
/// or `class Name`? `viewof`/`mutable`/`import` and bare expressions
/// (``md`…` ``, `Plot.plot(…)`, `{ … }`) are displayed.
fn ojs_is_declaration(src: &str) -> bool {
    let Some(line) = src
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("//"))
    else {
        return false;
    };
    for kw in ["viewof", "mutable", "import"] {
        if let Some(rest) = line.strip_prefix(kw)
            && rest.starts_with(char::is_whitespace)
        {
            return false;
        }
    }
    // `function name`, `async function name`, `class Name` define a name too.
    let head = line
        .strip_prefix("async ")
        .map(str::trim_start)
        .unwrap_or(line);
    for kw in ["function", "class"] {
        if let Some(rest) = head.strip_prefix(kw)
            && rest.starts_with(char::is_whitespace)
        {
            return true;
        }
    }
    let id_len = line
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$')
        .count();
    if id_len == 0 {
        return false;
    }
    let rest = line[id_len..].trim_start();
    rest.starts_with('=') && !rest.starts_with("==") && !rest.starts_with("=>")
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

/// Escape a string for an HTML *attribute* value (`&`, `<`, `>`, `"`). For text
/// content, use [`html_escape`].
pub fn escape_attr(s: &str) -> String {
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

/// Multi-page site chrome: a sticky theme-aware navbar, a slim footer, and post
/// prev/next nav. Only shipped when a page renders inside a site (see
/// [`html_page_from_doc_in_site`]); all of it is driven by `--qmd-*` vars so a
/// theme extension restyles it for free. Deliberately leaner than Quarto's
/// Bootstrap chrome (no banner, no search bar, no feed).
const SITE_CSS: &str = include_str!("../../assets/css/site.css");

const PAGE_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{{TITLE}}</title>
{{FAVICON}}
{{THEME_INIT}}
{{KATEX_CSS}}
<style>{{BASE_CSS}}</style>
{{CODE_HEAD}}
{{OJS_HEAD}}
{{THEME_CSS}}
{{INCLUDE_IN_HEADER}}
</head>
<body{{BODY_CLASS}}>
{{INCLUDE_BEFORE_BODY}}
{{BODY}}
<script>
  // Click any block to see its source position in the console (a static preview
  // of click-to-source; the live server wires this to the editor).
  document.addEventListener('click', (e) => {
    const el = e.target.closest('[data-block-id]');
    document.querySelectorAll('.qmd-hl').forEach(n => n.classList.remove('qmd-hl'));
    if (!el) return;
    el.classList.add('qmd-hl');
    console.log('block', el.dataset.blockId, '@', el.dataset.sourcepos);
  });
</script>
{{CODE_SCRIPTS}}
<script>document.addEventListener('DOMContentLoaded',function(){window.qmdEnhanceCode&&window.qmdEnhanceCode(document.body);});</script>
{{TOC_SCRIPT}}
{{OJS_INIT}}
{{INCLUDE_AFTER_BODY}}
</body>
</html>
"#;

// reveal.js (pinned to 5.1.0) is served from jsDelivr — the dev server runs
// locally with network access; only KaTeX is bundled for true offline use.

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
        assert_ne!(
            doc.blocks[0].id, doc.blocks[1].id,
            "duplicate content must get a tiebreak"
        );
        let again = render_document("Para.\n\nPara.\n");
        assert_eq!(
            doc.blocks[0].id, again.blocks[0].id,
            "ids must be stable across runs"
        );
    }

    #[test]
    fn front_matter_title_extracted_and_rendered_as_title_block() {
        let doc = render_document("---\ntitle: \"My Post\"\nfoo: bar\n---\n\nBody.\n");
        assert_eq!(doc.title.as_deref(), Some("My Post"));
        // A generated title block is prepended, then the body paragraph.
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.blocks[0].id, "qmd-title-block");
        assert!(
            doc.blocks[0]
                .html
                .contains("<h1 class=\"title\">My Post</h1>"),
            "got: {}",
            doc.blocks[0].html
        );
        assert!(doc.blocks[1].html.contains("Body."));
    }

    #[test]
    fn title_block_style_none_keeps_title_metadata_but_drops_visible_block() {
        let doc = render_document("---\ntitle: \"Blog\"\ntitle-block-style: none\n---\n\nIntro.\n");
        // Metadata title is preserved (drives `<title>`, OpenGraph, nav)...
        assert_eq!(doc.title.as_deref(), Some("Blog"));
        // ...but no visible title-block header is emitted, only the body.
        assert!(
            !doc.blocks.iter().any(|b| b.id == "qmd-title-block"),
            "expected no title block, got ids: {:?}",
            doc.blocks.iter().map(|b| &b.id).collect::<Vec<_>>()
        );
        assert_eq!(doc.blocks.len(), 1);
        assert!(doc.blocks[0].html.contains("Intro."));
    }

    #[test]
    fn title_block_includes_subtitle_date_and_description() {
        let doc = render_document(
            "---\ntitle: T\nsubtitle: S\ndate: 2026-05-15\nauthor: A\ndescription: D\n---\n\nx\n",
        );
        let h = &doc.blocks[0].html;
        assert!(h.contains("class=\"qmd-title-block\""));
        assert!(h.contains("<p class=\"subtitle\">S</p>"), "got: {h}");
        assert!(h.contains("<p class=\"description\">D</p>"), "got: {h}");
        assert!(
            h.contains("<span>A</span>") && h.contains("<span>2026-05-15</span>"),
            "got: {h}"
        );
    }

    #[test]
    fn reveal_deck_has_no_html_title_block() {
        // The deck builds its own title slide; no `qmd-title-block` block.
        let doc = render_document("---\ntitle: T\nformat: revealjs\n---\n\n## Slide\n");
        assert!(!doc.blocks.iter().any(|b| b.id == "qmd-title-block"));
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
        assert!(
            h.contains("text-align: right"),
            "alignment from |--:| missing: {h}"
        );
    }

    #[test]
    fn callout_wraps_content_using_leading_heading_as_title() {
        let doc = render_document("::: {.callout-note}\n## My Note\n\nBody text.\n:::\n");
        assert_eq!(doc.blocks.len(), 1, "the callout is one container block");
        let h = &doc.blocks[0].html;
        assert!(h.contains("class=\"callout callout-note\""), "got: {h}");
        assert!(
            h.contains("<div class=\"callout-title\">My Note</div>"),
            "got: {h}"
        );
        assert!(!doc.body_html().contains(":::"));
        // inner content keeps its own sourcepos so click-to-source still works.
        assert!(
            h.contains("<p data-block-id"),
            "inner block lost its id: {h}"
        );
        assert!(h.contains("Body text."));
    }

    #[test]
    fn callout_uses_explicit_title_and_default_title() {
        let titled = render_document("::: {.callout-tip title=\"Pro tip\"}\nDo this.\n:::\n");
        assert!(titled.blocks[0].html.contains("callout-tip"));
        assert!(
            titled.blocks[0].html.contains(">Pro tip</div>"),
            "got: {}",
            titled.blocks[0].html
        );

        let bare = render_document("::: {.callout-warning}\nBe careful.\n:::\n");
        assert!(
            bare.blocks[0].html.contains(">Warning</div>"),
            "got: {}",
            bare.blocks[0].html
        );
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
    fn mermaid_block_emits_pre_mermaid_without_code() {
        // Both the executable cell form and a plain fence become a mermaid pre.
        for src in [
            "```{mermaid}\nflowchart LR\n  A --> B\n```\n",
            "```mermaid\nflowchart LR\n  A --> B\n```\n",
        ] {
            let doc = render_document(src);
            let h = &doc.blocks[0].html;
            assert!(h.contains("<pre data-block-id"), "got: {h}");
            assert!(h.contains("class=\"mermaid\""), "got: {h}");
            assert!(
                !h.contains("<code"),
                "mermaid must not wrap a <code> element: {h}"
            );
            assert!(h.contains("flowchart LR"), "got: {h}");
            assert!(
                h.contains("A --&gt; B"),
                "diagram source should be escaped: {h}"
            );
        }
    }

    #[test]
    fn labelled_mermaid_becomes_numbered_referenceable_figure() {
        let doc = render_document(
            "See @fig-flow.\n\n```{mermaid}\n%%| label: fig-flow\n%%| fig-cap: \"The pipeline\"\nflowchart LR\n  A --> B\n```\n",
        );
        let body = doc.body_html();
        // the diagram is wrapped in a numbered figure with the #fig- anchor
        assert!(
            body.contains("id=\"fig-flow\""),
            "figure anchor missing: {body}"
        );
        assert!(
            body.contains("class=\"qmd-figure"),
            "mermaid not wrapped in a figure: {body}"
        );
        assert!(
            body.contains("<pre class=\"mermaid\">"),
            "diagram pre missing: {body}"
        );
        assert!(
            body.contains("<figcaption>Figure&nbsp;1: The pipeline</figcaption>"),
            "got: {body}"
        );
        // the `%%|` option lines are stripped from the diagram source
        assert!(!body.contains("%%|"), "mermaid cell options leaked: {body}");
        // and `@fig-flow` resolves to the numbered link
        assert!(
            body.contains("<a href=\"#fig-flow\" class=\"qmd-xref\">Figure&nbsp;1</a>"),
            "cross-reference did not resolve: {body}"
        );
    }

    #[test]
    fn unlabelled_mermaid_stays_a_bare_diagram() {
        // No label/fig-cap -> not a figure, not numbered (stays a plain pre).
        let doc = render_document("```{mermaid}\nflowchart LR\n  A --> B\n```\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("<pre data-block-id"), "got: {h}");
        assert!(
            !h.contains("qmd-figure"),
            "unlabelled mermaid should not be a figure: {h}"
        );
        assert!(
            !h.contains("figcaption"),
            "unlabelled mermaid should have no caption: {h}"
        );
    }

    #[test]
    fn cell_option_lines_are_dropped() {
        let doc = render_document("```{python}\n#| warning: false\nprint(1)\n```\n");
        let h = &doc.blocks[0].html;
        // (code is highlighted, so its text is split across scope spans)
        assert!(strip_tags(h).contains("print(1)"));
        assert!(!h.contains("#|"), "option lines should be stripped: {h}");

        // OJS cells become live placeholders; their `//|` options are stripped
        // before the source is base64-encoded into an ojs-module-contents script.
        let ojs = render_document("```{ojs}\n//| echo: false\nx = 1\n```\n");
        let oh = &ojs.blocks[0].html;
        assert!(
            oh.contains("class=\"cell ojs-cell\""),
            "ojs cell should be a live placeholder: {oh}"
        );
        assert!(
            oh.contains("ojs-module-contents"),
            "ojs cell missing module-contents: {oh}"
        );
        assert!(
            !oh.contains("//| echo"),
            "option lines should be stripped: {oh}"
        );
    }

    #[test]
    fn echo_and_include_false_hide_source_but_keep_the_cell() {
        // echo:false hides the source; the cell stays (so the executor still runs
        // it) and its output (added by the executor) is unaffected.
        let echo = render_document("```{python}\n#| echo: false\nprint(1)\n```\n");
        let b = &echo.blocks[0];
        assert!(
            b.cell.is_some(),
            "cell metadata must survive so the executor runs it"
        );
        assert!(
            b.cell.as_ref().unwrap().include,
            "echo:false keeps include true"
        );
        assert!(
            !b.html.contains("print(1)"),
            "echo:false must hide the source: {}",
            b.html
        );
        assert!(
            b.html.contains("qmd-cell-hidden"),
            "expected a hidden marker: {}",
            b.html
        );

        // include:false hides the source too and flags the cell so the executor
        // suppresses its output.
        let inc = render_document("```{python}\n#| include: false\nprint(1)\n```\n");
        let b = &inc.blocks[0];
        assert!(b.cell.is_some());
        assert!(
            !b.cell.as_ref().unwrap().include,
            "include:false must be recorded on the cell"
        );
        assert!(
            !b.html.contains("print(1)"),
            "include:false must hide the source: {}",
            b.html
        );

        // A plain cell still shows its source.
        let plain = render_document("```{python}\nprint(1)\n```\n");
        assert!(
            strip_tags(&plain.blocks[0].html).contains("print(1)"),
            "default cell shows source"
        );
    }

    #[test]
    fn execute_block_sets_document_cell_defaults() {
        // `execute: echo: false` hides every cell's source by default.
        let doc =
            render_document("---\nexecute:\n  echo: false\n---\n\n```{python}\nprint(1)\n```\n");
        let cell = doc
            .blocks
            .iter()
            .find(|b| b.cell.is_some())
            .expect("a code cell");
        assert!(
            !cell.html.contains("print(1)"),
            "execute.echo:false should hide source by default: {}",
            cell.html
        );

        // A per-cell `#| echo: true` overrides the document default.
        let doc2 = render_document(
            "---\nexecute:\n  echo: false\n---\n\n```{python}\n#| echo: true\nprint(1)\n```\n",
        );
        let cell2 = doc2
            .blocks
            .iter()
            .find(|b| b.cell.is_some())
            .expect("a code cell");
        assert!(
            strip_tags(&cell2.html).contains("print(1)"),
            "per-cell echo:true must override the execute default: {}",
            cell2.html
        );
    }

    #[test]
    fn explicit_heading_id_is_applied_and_stripped() {
        let doc = render_document("## Methods {#sec-methods}\n\nText.\n");
        let h = &doc.blocks[0].html;
        assert!(
            h.contains("id=\"sec-methods\""),
            "explicit id not applied: {h}"
        );
        assert!(
            !h.contains('{'),
            "the {{#id}} attribute leaked into the heading: {h}"
        );
        assert!(
            h.contains(">Methods</h2>"),
            "heading text wrong after strip: {h}"
        );

        // A heading without an attribute still gets a slug id.
        let plain = render_document("## My Heading\n");
        assert!(
            plain.blocks[0].html.contains("id=\"my-heading\""),
            "slug id missing: {}",
            plain.blocks[0].html
        );
    }

    #[test]
    fn sec_label_makes_at_sec_resolve_to_a_number() {
        let doc = render_document("## Methods {#sec-methods}\n\nSee @sec-methods.\n");
        let body = doc.body_html();
        assert!(
            body.contains("id=\"sec-methods\""),
            "heading id missing: {body}"
        );
        assert!(
            body.contains("class=\"qmd-xref\">Section&nbsp;1</a>"),
            "@sec-methods did not resolve to a numbered Section link: {body}"
        );
    }

    #[test]
    fn table_caption_is_numbered_folded_and_referenceable() {
        let doc = render_document(
            "| a | b |\n|---|---|\n| 1 | 2 |\n\n: My caption {#tbl-data}\n\nSee @tbl-data.\n",
        );
        let body = doc.body_html();
        assert!(
            body.contains("<table id=\"tbl-data\""),
            "table did not get the explicit id: {body}"
        );
        assert!(
            body.contains("<caption>Table&nbsp;1: My caption</caption>"),
            "caption not folded/numbered into the table: {body}"
        );
        assert!(
            !body.contains("{#tbl-data}") && !body.contains(">: My caption"),
            "the caption paragraph leaked instead of folding into the table: {body}"
        );
        assert!(
            body.contains("class=\"qmd-xref\">Table&nbsp;1</a>"),
            "@tbl-data did not resolve to a number: {body}"
        );
    }

    #[test]
    fn ojs_cell_emits_live_placeholder_and_classifies_declarations() {
        // A named declaration is hidden (nodetype="declaration"); a viewof and a
        // bare expression stay visible.
        let decl = render_document("```{ojs}\nsignalX = [1, 2, 3]\n```\n");
        assert!(decl.blocks[0].html.contains("class=\"cell ojs-cell\""));
        assert!(
            decl.blocks[0].html.contains("nodetype=\"declaration\""),
            "named decl should be hidden"
        );
        assert!(
            decl.blocks[0]
                .html
                .contains("<script type=\"ojs-module-contents\">")
        );

        let view = render_document("```{ojs}\nviewof n = Inputs.range([0, 9])\n```\n");
        assert!(
            !view.blocks[0].html.contains("nodetype=\"declaration\""),
            "viewof must stay visible"
        );

        let expr = render_document("```{ojs}\nPlot.lineY([1, 2, 3]).plot()\n```\n");
        assert!(
            !expr.blocks[0].html.contains("nodetype=\"declaration\""),
            "expression must stay visible"
        );
    }

    #[test]
    fn ojs_declaration_classifier() {
        assert!(ojs_is_declaration("foo = 1 + 2"));
        assert!(ojs_is_declaration("// a comment\nbar = {\n  return 3;\n}"));
        assert!(ojs_is_declaration("function makeScene(a, b) { return a; }"));
        assert!(ojs_is_declaration(
            "async function makeScene3D(build, invalidation) { return 0; }"
        ));
        assert!(ojs_is_declaration("class Particle { constructor() {} }"));
        assert!(!ojs_is_declaration("viewof x = Inputs.button()"));
        assert!(!ojs_is_declaration("import {a} from \"./x.js\""));
        assert!(!ojs_is_declaration("md`hello ${name}`"));
        assert!(!ojs_is_declaration("a == b"));
        assert!(!ojs_is_declaration("x => x + 1"));
        assert!(!ojs_is_declaration("{ const y = 1; return y; }"));
    }

    #[test]
    fn dollar_math_is_rendered_by_katex() {
        let doc = render_document("The value $x^2$ is positive.\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("katex"), "expected katex markup, got: {h}");
        assert!(
            !h.contains("$x^2$"),
            "raw dollar math should be consumed: {h}"
        );
    }

    #[test]
    fn display_math_block_renders() {
        let doc = render_document("$$\n\\sum_{i=1}^n x_i\n$$\n");
        assert!(
            doc.body_html().contains("katex-display"),
            "got: {}",
            doc.body_html()
        );
    }

    #[test]
    fn bare_latex_environment_renders_as_display_math() {
        let doc = render_document("\\begin{align*}\na &= b \\\\\nc &= d\n\\end{align*}\n");
        assert_eq!(doc.blocks.len(), 1);
        let h = &doc.blocks[0].html;
        // rendered as a display-math block (the raw TeX only survives inside
        // KaTeX's <annotation>, which is expected).
        assert!(h.contains("qmd-math-block"), "got: {h}");
        assert!(
            h.contains("katex-display"),
            "expected display math, got: {h}"
        );
    }

    #[test]
    fn html_block_attrs_injected_into_leading_tag() {
        let doc = render_document("<div class=\"demo\">\nhi\n</div>\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("<div class=\"demo\" data-block-id="), "got: {h}");
        // the wrapper-div double-emit bug must not reappear
        assert!(
            !h.contains("<div data-block-id"),
            "should inject, not wrap: {h}"
        );
    }

    // --- edge cases / robustness ---

    #[test]
    fn empty_and_whitespace_inputs_do_not_panic() {
        assert!(render_document("").blocks.is_empty());
        assert!(render_document("   \n\n\t\n").blocks.is_empty());
    }

    #[test]
    fn front_matter_only_yields_just_the_title_block() {
        let doc = render_document("---\ntitle: Only Meta\n---\n");
        assert_eq!(doc.title.as_deref(), Some("Only Meta"));
        // Only the generated title block (no body content).
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(doc.blocks[0].id, "qmd-title-block");
    }

    #[test]
    fn front_matter_without_title_yields_no_blocks() {
        // No title -> no title block, and no body -> empty.
        assert!(render_document("---\nfoo: bar\n---\n").blocks.is_empty());
    }

    #[test]
    fn reveal_deck_injects_includes_and_theme() {
        let src = "---\n\
            format: revealjs\n\
            include-in-header:\n  text: |\n    <meta name=\"deck\" content=\"1\">\n\
            include-after-body:\n  text: |\n    <script>window.__deck=1</script>\n\
            ---\n\n## Slide\n";
        let page = render_html_page(src, "deck");
        assert!(
            page.contains("<div class=\"reveal\">"),
            "should render as a reveal deck"
        );
        let head = &page[..page.find("</head>").expect("has </head>")];
        assert!(
            head.contains("<meta name=\"deck\" content=\"1\">"),
            "include-in-header not injected into the deck <head>"
        );
        assert!(
            page.contains("<script>window.__deck=1</script>"),
            "include-after-body not injected into the deck"
        );
    }

    #[test]
    fn front_matter_include_text_injected_at_head_and_body() {
        let src = "---\n\
            title: T\n\
            include-in-header:\n  text: |\n    <meta name=\"x\" content=\"y\">\n\
            include-before-body:\n  text: |\n    <div id=\"top-banner\"></div>\n\
            include-after-body:\n  text: |\n    <script>window.__after=1</script>\n\
            ---\n\nBody.\n";
        let page = render_html_page(src, "fallback");
        let head = &page[..page.find("</head>").expect("has </head>")];
        assert!(
            head.contains("<meta name=\"x\" content=\"y\">"),
            "include-in-header not injected into <head>"
        );
        // before-body lands ahead of the rendered body paragraph.
        let banner = page.find("top-banner").expect("before-body injected");
        let body_para = page.find("Body.").expect("body present");
        assert!(
            banner < body_para,
            "include-before-body must precede the body"
        );
        assert!(
            page.contains("<script>window.__after=1</script>"),
            "include-after-body not injected"
        );
    }

    #[test]
    fn nested_lists_render_with_nesting() {
        let doc = render_document("- a\n    - b\n    - c\n- d\n");
        let h = &doc.blocks[0].html;
        assert!(h.starts_with("<ul "), "got: {h}");
        assert!(
            h.contains("<li>a<ul><li>b</li><li>c</li></ul></li>"),
            "got: {h}"
        );
    }

    #[test]
    fn ordered_list_start_attribute_preserved() {
        let doc = render_document("3. third\n4. fourth\n");
        assert!(doc.blocks[0].html.starts_with("<ol "));
        assert!(
            doc.blocks[0].html.contains("start=\"3\""),
            "got: {}",
            doc.blocks[0].html
        );
    }

    #[test]
    fn links_images_and_blockquotes_render() {
        let link = render_document("[text](https://example.com \"t\")\n");
        assert!(
            link.blocks[0]
                .html
                .contains("<a href=\"https://example.com\" title=\"t\">text</a>")
        );

        let img = render_document("![alt text](/img.png)\n");
        assert!(
            img.blocks[0]
                .html
                .contains("<img src=\"/img.png\" alt=\"alt text\" />")
        );

        let quote = render_document("> quoted line\n");
        assert!(quote.blocks[0].html.starts_with("<blockquote "));
        assert!(quote.blocks[0].html.contains("quoted line"));
    }

    #[test]
    fn attribute_values_are_escaped() {
        let doc = render_document("[x](https://e.com?a=1&b=\"2\")\n");
        let h = &doc.blocks[0].html;
        assert!(
            h.contains("&amp;"),
            "ampersand should be escaped in href: {h}"
        );
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

    // --- reveal.js / slides ---

    #[test]
    fn reveal_format_detected_from_front_matter() {
        // Nested block form (the corpus shape): `format:` with a *-revealjs subkey.
        let deck = render_document(
            "---\nformat:\n  liquid-glass-revealjs:\n    slide-number: true\n---\n\n## A\n",
        );
        assert_eq!(deck.format, DocFormat::Reveal);
        // Inline form.
        let inline = render_document("---\nformat: revealjs\n---\n\n## A\n");
        assert_eq!(inline.format, DocFormat::Reveal);
        // A normal post is Html, even if a nested non-format key mentions revealjs.
        let post = render_document("---\ntitle: Post\nformat: html\n---\n\nHi.\n");
        assert_eq!(post.format, DocFormat::Html);
    }

    #[test]
    fn deck_splits_into_title_slide_and_one_section_per_heading() {
        let doc = render_document(
            "---\ntitle: Deck\nsubtitle: A subtitle\nformat: revealjs\n---\n\n## First\n\nHello.\n\n## Second\n\nWorld.\n",
        );
        let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
        // Title slide from front matter.
        assert!(slides.contains("id=\"title-slide\""), "got: {slides}");
        assert!(
            slides.contains("<h1 class=\"title\">Deck</h1>"),
            "got: {slides}"
        );
        assert!(
            slides.contains("<p class=\"subtitle\">A subtitle</p>"),
            "got: {slides}"
        );
        // One <section> per h2, id slugged from the heading text.
        assert!(
            slides.contains("<section id=\"first\" class=\"slide level2\">"),
            "got: {slides}"
        );
        assert!(
            slides.contains("<section id=\"second\" class=\"slide level2\">"),
            "got: {slides}"
        );
        // Heading keeps its block id inside the section (block-swap/click-to-source).
        assert!(
            slides.contains("<h2 data-block-id="),
            "heading lost its block id: {slides}"
        );
        // title + two content slides, no nesting.
        assert_eq!(slides.matches("<section").count(), 3, "got: {slides}");
    }

    #[test]
    fn thematic_break_starts_a_new_slide_and_is_not_emitted() {
        let doc = render_document("---\nformat: revealjs\n---\n\nOne.\n\n---\n\nTwo.\n");
        let slides = slides_html(None, None, &doc.blocks);
        assert!(
            !slides.contains("<hr"),
            "the --- delimiter must not render: {slides}"
        );
        assert_eq!(slides.matches("<section").count(), 2, "got: {slides}");
    }

    #[test]
    fn h1_wraps_following_h2s_in_a_vertical_stack() {
        let doc =
            render_document("---\nformat: revealjs\n---\n\n# Part One\n\nIntro.\n\n## A\n\n## B\n");
        let slides = slides_html(None, None, &doc.blocks);
        // Outer wrapper section, then the h1 lead slide, then the two h2 children.
        assert!(
            slides.contains("<section>\n<section id=\"part-one\" class=\"slide level1\">"),
            "got: {slides}"
        );
        assert!(
            slides.contains("<section id=\"a\" class=\"slide level2\">"),
            "got: {slides}"
        );
        assert!(
            slides.contains("<section id=\"b\" class=\"slide level2\">"),
            "got: {slides}"
        );
        // 1 wrapper + lead + 2 children = 4 sections.
        assert_eq!(slides.matches("<section").count(), 4, "got: {slides}");
    }

    #[test]
    fn reveal_page_carries_revealjs_scaffolding() {
        let page = render_html_page(
            "---\ntitle: D\nformat: revealjs\n---\n\n## Slide\n",
            "fallback",
        );
        assert!(page.contains("class=\"reveal\""));
        assert!(page.contains("class=\"slides\""));
        assert!(page.contains("reveal.js@5.1.0"));
        assert!(page.contains("Reveal.initialize("));
    }

    // --- books: heading anchors, figures, toc ---

    #[test]
    fn headings_get_deduped_anchor_ids() {
        let doc = render_document("# Intro\n\nbody\n\n# Intro\n");
        assert!(
            doc.blocks[0].html.starts_with("<h1 id=\"intro\""),
            "got: {}",
            doc.blocks[0].html
        );
        // a repeated heading slug is deduped with a -N suffix.
        let last = doc.blocks.last().unwrap();
        assert!(last.html.contains("id=\"intro-1\""), "got: {}", last.html);
    }

    #[test]
    fn reveal_headings_have_no_id_to_avoid_duplicating_section_ids() {
        // In a deck the slug lives on the wrapping <section>, so the heading must
        // not also carry it (that would be a duplicate id in the DOM).
        let doc = render_document("---\nformat: revealjs\n---\n\n## A Slide\n");
        let h = doc
            .blocks
            .iter()
            .find(|b| b.html.starts_with("<h2"))
            .unwrap();
        assert!(
            !h.html.contains(" id=\""),
            "reveal heading should not carry an id: {}",
            h.html
        );
    }

    #[test]
    fn standalone_image_becomes_a_numbered_figure() {
        let doc = render_document(
            "![Scree plot](scree.png){#fig-scree width=50% fig-align=\"center\"}\n",
        );
        let h = &doc.blocks[0].html;
        assert!(h.starts_with("<figure"), "got: {h}");
        assert!(h.contains("id=\"fig-scree\""), "got: {h}");
        assert!(
            h.contains("class=\"qmd-figure qmd-figure-center\""),
            "got: {h}"
        );
        assert!(h.contains("<img src=\"scree.png\""), "got: {h}");
        assert!(h.contains("style=\"width:50%\""), "got: {h}");
        assert!(
            h.contains("<figcaption>Figure&nbsp;1: Scree plot</figcaption>"),
            "got: {h}"
        );
        assert!(!h.contains("{#fig-"), "the attribute block leaked: {h}");
        // the figure still carries the block model attributes.
        assert!(
            h.contains("data-block-id=") && h.contains("data-sourcepos="),
            "got: {h}"
        );
    }

    #[test]
    fn inline_image_in_a_sentence_stays_inline() {
        let doc = render_document("See ![logo](l.png) for the mark.\n");
        let h = &doc.blocks[0].html;
        assert!(h.starts_with("<p "), "got: {h}");
        assert!(h.contains("<img src=\"l.png\""), "got: {h}");
        assert!(
            !h.contains("<figure"),
            "a non-standalone image must not become a figure: {h}"
        );
    }

    #[test]
    fn toc_page_lists_headings_with_anchor_links() {
        let page = render_html_page(
            "---\ntitle: Doc\nformat:\n  html:\n    toc: true\n---\n\n# A\n\ntext\n\n## B\n",
            "fb",
        );
        assert!(page.contains("id=\"TOC\""), "missing TOC nav");
        assert!(
            page.contains("<body class=\"has-toc\">"),
            "missing toc layout class"
        );
        assert!(
            page.contains("<a href=\"#a\">A</a>"),
            "missing TOC entry for A: {page}"
        );
        assert!(
            page.contains("<a href=\"#b\">B</a>"),
            "missing nested TOC entry for B"
        );
    }

    #[test]
    fn no_toc_when_not_requested() {
        let page = render_html_page("---\ntitle: Doc\n---\n\n# A\n", "fb");
        // (the `#TOC`/`has-toc` CSS rules are always present; assert on markup.)
        assert!(
            !page.contains("<nav id=\"TOC\""),
            "TOC nav should be absent without toc: true"
        );
        assert!(
            !page.contains("<body class=\"has-toc\">"),
            "toc layout should be off"
        );
    }

    #[test]
    fn detect_toc_is_tristate_so_explicit_false_can_override_a_site_default() {
        // Unset, on, and off must be distinguishable: a plain bool can't tell an
        // explicit `toc: false` (which should beat the site default) from "unset".
        assert_eq!(detect_toc("title: X\n"), None);
        assert_eq!(detect_toc("title: X\ntoc: true\n"), Some(true));
        assert_eq!(detect_toc("title: X\ntoc: false\n"), Some(false));
        // `toc-depth:`/`toc-title:` are not the `toc:` key and must not match.
        assert_eq!(detect_toc("toc-depth: 2\ntoc-title: Contents\n"), None);
    }

    #[test]
    fn theme_dark_default_drives_data_theme_resolver() {
        // Built-in dark no longer inlines a per-page override; it sets the default
        // mode, and the always-shipped dark CSS is selected at runtime by data-theme.
        let dark = render_document("---\ntheme: dark\n---\n\nx\n");
        assert!(
            dark.theme_css.is_empty(),
            "built-in dark should not inline override CSS"
        );
        assert_eq!(dark.theme_default, "dark");

        let page = render_html_page("---\ntheme: dark\n---\n\nx\n", "fb");
        assert!(
            page.contains("html[data-theme=\"dark\"]"),
            "scoped dark CSS not shipped"
        );
        assert!(page.contains("--qmd-bg: #16181d"), "dark vars missing");
        assert!(
            page.contains("var DEFAULT = \"dark\""),
            "resolver default should be dark"
        );

        // No theme -> auto (follow OS); light -> light. No inlined override either way.
        let plain = render_document("---\ntitle: x\n---\n\nx\n");
        assert!(plain.theme_css.is_empty());
        assert_eq!(plain.theme_default, "auto");
        assert_eq!(
            render_document("---\ntheme: light\n---\n\nx\n").theme_default,
            "light"
        );
    }

    #[test]
    fn theme_list_takes_first_entry() {
        // `theme: [dark, custom.scss]` (Quarto list form) selects the base.
        let d = render_document("---\ntheme: [dark, custom.scss]\n---\n\nx\n");
        assert_eq!(
            d.theme_default, "dark",
            "first list entry (dark) should win"
        );
    }
}
