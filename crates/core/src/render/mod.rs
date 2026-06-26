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

mod model;
pub(crate) use model::CellRole;
pub use model::{
    Block, Cell, CellFigure, CellTable, DocFormat, JsOpts, PageIncludes, RenderedDoc, Warning,
};

fn parse_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.front_matter_delimiter = Some("---".to_string());
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    // Parse `$...$` (inline) and `$$...$$` (display) into Math nodes for KaTeX.
    options.extension.math_dollars = true;
    // `[^1]` references + `[^1]: …` definitions; comrak moves definitions to the
    // document end in reference order, which we gather into a footnotes section.
    options.extension.footnotes = true;
    // Smart typography (curly quotes, en/em dashes) to match Quarto/pandoc output.
    options.parse.smart = true;
    // sourcepos is tracked on AST nodes during parsing; `render.sourcepos`
    // only affects comrak's own formatter, which we don't use.
    options
}

/// Parse `src` into ordered top-level blocks with stable ids + sourcepos.
/// Does not resolve `{{< include >}}` (use [`render_document_with_includes`]).
mod deck;
pub use deck::{DeckParts, assemble_deck_page, deck_client_script, slides_html};
// `deck_theme_head` is used inside `deck.rs` (the deck builder) and by the unit
// tests; it's not part of the public API, so it's only pulled into scope here for
// the tests rather than re-exported.
#[cfg(test)]
use deck::deck_theme_head;
mod extension;
pub use extension::embed_targets;
use extension::{resolve_format_extension, resolve_named_extensions};
mod divs;
mod validate;
pub(crate) use divs::parse_attrs;
use divs::{group_divs, parse_pandoc_attrs, preprocess, scan_div_spans};
mod emit;
use emit::emit;
// emit_children is re-exported so the sibling figure module reaches it via `super`.
pub(crate) use emit::emit_children;
mod figure;
use figure::{emit_figure, emit_mermaid_figure, figure_parts};
mod theme;
// Used only by the page builders; kept crate-internal, not part of the public API.
pub(crate) use theme::theme_head;
mod page;
use page::page_from_doc;
pub use page::{
    PageParts, SiteCtx, assemble_html_page, favicon_link, html_page_from_doc_in_site,
    render_doc_to_page,
};
use theme::{detect_theme, resolve_theme, resolve_theme_layers, theme_default_mode, theme_style};

/// Render a `.qmd` source string into the `RenderedDoc` block model: the parse
/// step only (no code execution, no page chrome). The dev server diffs these
/// block lists for incremental updates; the CLI wraps the result in a page.
///
/// ```
/// let doc = qmd_fast_core::render_document("# Title\n\nHello *world*.\n");
/// assert_eq!(doc.title, None); // no front-matter title
/// assert_eq!(doc.blocks.len(), 2); // the heading + the paragraph
/// assert!(doc.blocks[0].html.contains("<h1"));
/// ```
pub fn render_document(src: &str) -> RenderedDoc {
    render_internal(src, None, None)
}

/// Like [`render_document`], but first expands `{{< include >}}` shortcodes
/// relative to `base_dir`, mapping each block back to its origin file, and
/// resolves citations/cross-references against the doc's bibliography.
pub fn render_document_with_includes(src: &str, base_dir: &Path) -> RenderedDoc {
    let (expanded, origins, include_warnings) = crate::includes::resolve_warned(src, base_dir);
    // Declarative shortcodes (`{{< name args >}}`) from the active format
    // extension expand after includes, line-preserving so `origins` stays valid.
    // A `{{< name >}}` that no extension/built-in declares is left verbatim but
    // reported, so a typo'd shortcode doesn't ship silently as literal text.
    let (expanded, shortcode_warnings) = extension::expand_shortcodes(&expanded, Some(base_dir));
    let mut doc = render_internal(&expanded, Some(&origins), Some(base_dir));
    // An include that couldn't be expanded (unsafe path, cycle, unreadable) leaves
    // its `{{< include … >}}` directive literal in the output; surface it as a
    // located, click-to-source diagnostic on the same channel as broken refs so it
    // shows in build/preview/`check` instead of shipping silently.
    doc.warnings.extend(include_warnings.into_iter().map(|iw| {
        // `iw.line` is always >= 1 (constructed as `idx + 1` in includes.rs), so the
        // warning is always located on the directive line.
        Warning::new(format!(
            "include not resolved ({}): {{{{< include {} >}}}}",
            iw.reason, iw.target
        ))
        .at(iw.file, iw.line as u32)
    }));
    doc.warnings.extend(shortcode_warnings);
    doc
}

/// Core render. Runs the actual work on a worker thread with a large stack:
/// deeply nested input (blockquotes / lists) drives deep recursion in the Markdown
/// parser and block emission, which on the default ~8 MB stack overflows and
/// **aborts the whole process** (a single pathological document would crash `build`
/// or take down the live preview server) at ~3000 levels. A big stack absorbs any
/// realistic nesting; a panic is propagated to the caller unchanged.
fn render_internal(
    src: &str,
    origins: Option<&[LineOrigin]>,
    base_dir: Option<&Path>,
) -> RenderedDoc {
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || render_internal_impl(src, origins, base_dir))
        {
            Ok(handle) => match handle.join() {
                Ok(doc) => doc,
                Err(payload) => std::panic::resume_unwind(payload),
            },
            // Spawning the big-stack worker can fail under a strict address-space
            // limit (e.g. `ulimit -v`). Fall back to rendering inline on the current
            // (default-stack) thread rather than panicking — same as before this guard.
            Err(_) => render_internal_impl(src, origins, base_dir),
        }
    })
}

fn render_internal_impl(
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
    let mut lang: Option<String> = None;
    let mut format = DocFormat::Html;
    let mut toc_explicit: Option<bool> = None;
    // `title-block-style: none` keeps `title` (drives `<title>`, OpenGraph, nav)
    // but skips the visible `<h1>` header (nav landing pages don't need it).
    let mut hide_title_block = false;
    let mut theme: Option<String> = None;
    let mut bib_field: Option<String> = None;
    let mut includes = PageIncludes::default();
    // Whether a format/named extension contributed head/body markup (e.g. a reveal
    // theme extension like liquid-glass): such a theme owns the deck's colours.
    let mut ext_contributes = false;
    // Non-fatal render warnings (missing/broken extension, bibliography, theme),
    // collected through the whole render and surfaced in the dev menu / build log.
    let mut warnings: Vec<Warning> = Vec::new();
    // Validate the document's front matter against qmd-fast's vocabulary (top-level
    // keys + the nested execute/listing/about/hero children); located warnings flow to
    // the dev panel as click-to-source diagnostics, the same channel as broken refs.
    warnings.extend(crate::frontmatter::validate_front_matter(src));
    // Opt-in prose lint (front-matter `prose-lint:`): markdown-aware, diagnostic-only,
    // located via map_origin like every other warning.
    if let Some(cfg) =
        crate::prose::config(crate::frontmatter::front_matter_block(src).unwrap_or(""))
    {
        for (line, msg) in crate::prose::lint(src, &cfg) {
            let (file, mapped) = map_origin(origins, line);
            warnings.push(Warning::new(msg).at(file, mapped as u32));
        }
    }
    // Document-level cell defaults from a front-matter `execute:` block; a cell's
    // own `#| echo`/`#| include`/`#| cache` overrides these.
    let mut exec_echo = true;
    let mut exec_include = true;
    let mut exec_cache = true;
    let mut flat: Vec<FlatBlock> = Vec::new();
    // Footnote definitions, rendered as `<li>`s and gathered into a section at the
    // end (comrak moves them here in reference order); see below the loop.
    let mut footnote_items: Vec<String> = Vec::new();
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
        // Footnote definitions are gathered into a section at the end (comrak has
        // already moved them here, in reference order) — not rendered in place.
        let fn_name = match &node.data.borrow().value {
            NodeValue::FootnoteDefinition(fd) => Some(fd.name.clone()),
            _ => None,
        };
        if let Some(name) = fn_name {
            footnote_items.push(emit::footnote_def_li(node, &name));
            continue;
        }
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
                lang = extract_field(fm, "lang");
                bib_field = extract_field(fm, "bibliography");
                format = detect_format(fm);
                toc_explicit = detect_toc(fm);
                hide_title_block = detect_title_block_hidden(fm);
                theme = detect_theme(fm);
                // Extensions contribute first (the `format:` one, then each
                // `extensions: [..]` entry), and the doc's own front matter
                // appends/overrides last.
                let (fmt_inc, fmt_theme_base) =
                    resolve_format_extension(fm, base_dir, &mut warnings);
                let named_inc = resolve_named_extensions(fm, base_dir, &mut warnings);
                ext_contributes = fmt_inc.has_markup() || named_inc.has_markup();
                includes = fmt_inc;
                includes.merge(&named_inc);
                includes.merge(&resolve_doc_includes(fm, base_dir));
                // A format extension's `theme: [dark|light, …]` selects the
                // built-in base mode when the doc itself named no `theme:` (the
                // extension owns the look, matching Quarto).
                if theme.is_none() {
                    theme = fmt_theme_base.map(String::from);
                }
                (exec_echo, exec_include, exec_cache) = detect_execute_defaults(fm);
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
                    code_lang(&cb.info).map(|lang| {
                        let js = parse_js_opts(&cb.literal, &lang);
                        Cell {
                            lang,
                            code: strip_cell_options(&cb.literal),
                            figure: None,
                            table: None,
                            echo: cell_flag_or(&cb.literal, "echo", exec_echo),
                            include: cell_flag_or(&cb.literal, "include", exec_include),
                            cache: cell_flag_or(&cb.literal, "cache", exec_cache),
                            fig_export: cell_option(&cb.literal, "fig-export").map(str::to_string),
                            js,
                        }
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
                    let tbl_cap = cell_option(&cb.literal, "tbl-cap");
                    let is_fig = label.is_some_and(|l| l.starts_with("fig-")) || fig_cap.is_some();
                    let is_lst = label.is_some_and(|l| l.starts_with("lst-")) || lst_cap.is_some();
                    let is_tbl = label.is_some_and(|l| l.starts_with("tbl-")) || tbl_cap.is_some();
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
                    } else if is_tbl {
                        Some(CellRole::Table {
                            anchor: label.filter(|l| l.starts_with("tbl-")).map(str::to_string),
                            caption: tbl_cap.map(str::to_string),
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            };
            // Validate this code cell's `#|` options against qmd-fast's vocabulary
            // (a typo or a Quarto-only key becomes a located, click-to-source warning;
            // the cell still renders unchanged).
            if cell.is_some()
                && let NodeValue::CodeBlock(cb) = &data.value
            {
                warnings.extend(validate::validate_cell_options(
                    &cb.literal,
                    start_line,
                    file.clone(),
                ));
            }
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
        let file_attr = source_file_attr(source_file.as_deref());
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
            register_xref(&mut xref_registry, &mut warnings, id, sec_count.to_string());
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
        // `sourcepos` MUST stay in `L:C-L:C` form: reverse cursor-sync (client.js
        // `highlightAtLine`) matches it against `^(\d+):\d+-(\d+):\d+$` and silently
        // skips any block it can't parse. Corpus-enforced by
        // `reverse_sync_sourcepos_is_total` (tests/corpus.rs). A generated block with no
        // source position uses an EMPTY sourcepos (omitted), never a degenerate one.
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
            register_xref(
                &mut xref_registry,
                &mut warnings,
                &anchor,
                eq_count.to_string(),
            );
            html.push_str(&emit_equation(&latex, &anchor, &attrs, eq_count));
        } else if let Some(fig) = is_paragraph.then(|| figure_parts(node)).flatten() {
            // Standalone image -> a numbered `<figure>`; register `#fig-` ids so
            // `@fig-x` cross-references resolve to the number.
            fig_count += 1;
            if let Some(fid) = fig.attrs.id.as_deref().filter(|i| i.starts_with("fig-")) {
                register_xref(
                    &mut xref_registry,
                    &mut warnings,
                    fid,
                    fig_count.to_string(),
                );
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
                        register_xref(&mut xref_registry, &mut warnings, a, fig_count.to_string());
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
                        "js" => html.push_str(&emit_js_figure(
                            &code,
                            &id,
                            cell.as_ref().map(|c| &c.js),
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
                            register_xref(
                                &mut xref_registry,
                                &mut warnings,
                                a,
                                lst_count.to_string(),
                            );
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
                CellRole::Table { anchor, caption } => {
                    // The table is the cell's executed output (e.g. a pandas
                    // DataFrame), so tag the cell and let the executor inject the
                    // caption/id; the number is assigned in document order by
                    // `apply_table_captions` below. The source renders now (or is
                    // hidden by `echo:false`/`include:false`), like a figure cell.
                    if let Some(c) = cell.as_mut() {
                        c.table = Some(CellTable {
                            anchor: anchor.clone(),
                            caption: caption.clone(),
                            number: 0,
                        });
                    }
                    if cell.as_ref().is_some_and(|c| !c.echo || !c.include) {
                        html.push_str(&hidden_cell(&attrs));
                    } else {
                        emit(node, &attrs, &mut html);
                    }
                }
            }
        } else if let Some(c) = cell.as_ref().filter(|c| c.lang == "js") {
            // Native interactive `{js}` cell: the qmd-js enhancer runs it
            // client-side (no Observable runtime).
            html.push_str(&emit_js_cell(&c.code, &id, &c.js, &attrs));
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
        // A deck heading may set section-level attrs (`## T {background-image="..."}`,
        // `{auto-animate=true}`): emit them as data-* on the heading so the slide model
        // can hoist them onto the wrapping `<section>`.
        if heading_level.is_some() && format == DocFormat::Reveal {
            let section_attrs = heading_section_attrs(&block_src);
            if !section_attrs.is_empty() {
                html = apply_heading_bg(&html, &section_attrs);
            }
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

    let mut blocks = group_divs(flat, &spans, origins, &mut id_counts, &mut warnings);
    // Pandoc table captions (`: caption {#tbl-x}` after a table) are numbered and
    // folded into the table's `<caption>`; registers `tbl-x` for `@tbl-` refs.
    apply_table_captions(&mut blocks, &mut xref_registry, &mut warnings);
    let bib = load_bibliography(bib_field.as_deref(), base_dir, &mut warnings);
    warnings.extend(crate::cite::process(&mut blocks, &bib, &xref_registry));
    // Gather the footnote definitions (collected above, in comrak's reference order)
    // into one footnotes section, appended after any References.
    if !footnote_items.is_empty() {
        let inner = footnote_items.join("");
        blocks.push(Block {
            id: "qmd-footnotes".to_string(),
            sourcepos: String::new(),
            source_file: None,
            html: format!(
                "<section class=\"footnotes\" role=\"doc-endnotes\" data-block-id=\"qmd-footnotes\"><hr><ol>{inner}</ol></section>"
            ),
            cell: None,
        });
    }
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
    let theme_css = resolve_theme(theme.as_deref(), base_dir, &mut warnings);
    let theme_default = theme_default_mode(theme.as_deref()).to_string();
    let theme_is_custom = ext_contributes || !theme_css.trim().is_empty();
    RenderedDoc {
        title,
        subtitle,
        lang,
        description,
        format,
        // Standalone default: a TOC only when the page asked for one. The site
        // path overrides this via `page_toc` using `toc_explicit`.
        toc: toc_explicit.unwrap_or(false),
        toc_explicit,
        theme_css,
        theme_default,
        theme_is_custom,
        includes,
        warnings,
        blocks,
    }
}

/// OpenGraph / Twitter-card / SEO `<meta>` for a standalone document, from its own
/// front matter. A single file has no site URL, so there's no canonical/og:url or
/// absolute image — just the text tags that make a shared link or search result
/// meaningful. (Site pages get the richer, URL-aware set from `site::meta`.)
fn social_meta_head(title: Option<&str>, description: Option<&str>) -> String {
    let meta = |attr: &str, key: &str, val: &str| {
        format!("\n<meta {attr}=\"{key}\" content=\"{}\">", escape_attr(val))
    };
    let mut h = String::new();
    if let Some(d) = description.filter(|s| !s.is_empty()) {
        h.push_str(&meta("name", "description", d));
        h.push_str(&meta("property", "og:description", d));
        h.push_str(&meta("name", "twitter:description", d));
    }
    h.push_str(&meta("property", "og:type", "article"));
    if let Some(t) = title.filter(|s| !s.is_empty()) {
        h.push_str(&meta("property", "og:title", t));
        h.push_str(&meta("name", "twitter:title", t));
    }
    h.push_str(&meta("name", "twitter:card", "summary"));
    h
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
    // Containment: an absolute path or one escaping the project root is refused
    // (path-traversal guard), reported the same as a missing file.
    let path = match base_dir {
        Some(dir) => crate::includes::safe_join(dir, rel),
        None => Path::new(rel).is_relative().then(|| PathBuf::from(rel)),
    };
    match path.and_then(|p| std::fs::read_to_string(&p).ok()) {
        Some(s) => s,
        None => format!(
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

/// Whether a document's front matter selects a revealjs deck. Reads only the
/// front matter (no full parse), so site discovery can cheaply flag a loose deck
/// dropped into a website — which would otherwise be flattened into an article.
pub fn is_reveal_doc(src: &str) -> bool {
    crate::frontmatter::front_matter_block(src)
        .is_some_and(|fm| detect_format(fm) == DocFormat::Reveal)
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
            // `format: revealjs` (or a list `[html, revealjs]`): match a format
            // *name*, not any substring, so a theme/filename that merely contains
            // "revealjs" (e.g. `theme: my-revealjs.css`) can't flip an HTML doc to
            // a deck.
            return if inline.split(['[', ']', ',', ' ']).any(is_reveal_format) {
                DocFormat::Reveal
            } else {
                DocFormat::Html
            };
        }
        // Block form: the sub-keys are format *names* (`html:`, `revealjs:`,
        // `liquid-glass-revealjs:`). Match the key, never a value substring.
        for sub in &lines[i + 1..] {
            if sub.trim().is_empty() {
                continue;
            }
            if !sub.starts_with(char::is_whitespace) {
                break;
            }
            let key = sub.trim().split(':').next().unwrap_or("");
            if is_reveal_format(key) {
                return DocFormat::Reveal;
            }
        }
        return DocFormat::Html;
    }
    DocFormat::Html
}

/// Whether a `format:` name selects a revealjs deck: `revealjs` itself or an
/// extension variant `<ext>-revealjs` (e.g. `liquid-glass-revealjs`).
fn is_reveal_format(name: &str) -> bool {
    let n = name.trim().trim_matches(['"', '\'']);
    n == "revealjs" || n.ends_with("-revealjs")
}

/// Load and merge the bibliography file(s) named in the front matter, resolved
/// relative to `base_dir`. Returns an empty bibliography when none is found
/// (citations still de-leak; cross-references still resolve).
fn load_bibliography(
    field: Option<&str>,
    base_dir: Option<&Path>,
    warnings: &mut Vec<Warning>,
) -> crate::cite::Bibliography {
    let (Some(field), Some(base)) = (field, base_dir) else {
        return crate::cite::Bibliography::default();
    };
    let mut text = String::new();
    for tok in field.split([',', '[', ']', ' ']) {
        let tok = tok.trim().trim_matches(['"', '\'']);
        if !tok.ends_with(".bib") {
            continue;
        }
        match crate::includes::safe_join(base, tok).and_then(|p| std::fs::read_to_string(&p).ok()) {
            Some(content) => {
                text.push_str(&content);
                text.push('\n');
            }
            // An explicitly named `.bib` that can't be read (or escapes the project
            // root) is a typo worth flagging: citations would otherwise just
            // silently fail to resolve.
            None => warnings.push(Warning::new(format!("bibliography file not found: {tok}"))),
        }
    }
    let (bib, bib_warnings) = crate::cite::parse_bib_warned(&text);
    warnings.extend(bib_warnings.into_iter().map(Warning::new));
    bib
}

/// A top-level block plus its line in the (post-include, post-blank) buffer,
/// used to group blocks back into fenced-div containers.
struct FlatBlock {
    buf_start: usize,
    block: Block,
}

/// The ` data-source-file="…"` attribute for a block from an included file (empty
/// for a primary-document block). Preserves the click-to-source invariant for both
/// leaf blocks and fenced-div containers.
fn source_file_attr(file: Option<&str>) -> String {
    match file {
        Some(f) => format!(" data-source-file=\"{}\"", escape_attr(f)),
        None => String::new(),
    }
}

/// The ` id="…"` attribute for an optional anchor (empty when `None`). Shared by the
/// figure/listing/div emitters that put a `#fig-`/`#lst-`/`#id` anchor on a wrapper.
pub(crate) fn id_attr(id: Option<&str>) -> String {
    match id {
        Some(i) => format!(" id=\"{}\"", escape_attr(i)),
        None => String::new(),
    }
}

/// Map a 1-based buffer line to its (origin file, origin line). Without a
/// source map, the file is the primary document and the line is unchanged.
fn map_origin(origins: Option<&[LineOrigin]>, buffer_line: usize) -> (Option<String>, usize) {
    match origins.and_then(|o| o.get(buffer_line.saturating_sub(1))) {
        Some(origin) => (origin.file.clone(), origin.line),
        None => (None, buffer_line),
    }
}

/// Render a complete, viewable HTML page (used by the one-shot CLI). The
/// front-matter `title:` becomes the document `<title>`; `fallback_title` is
/// used when the source declares none.
///
/// ```
/// let html = qmd_fast_core::render_html_page("---\ntitle: Demo\n---\n\nHi.\n", "fallback");
/// assert!(html.contains("<title>Demo</title>"));
/// assert!(html.contains("Hi."));
/// ```
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
/// highlight). Emitted by the page builders in `page.rs`/`deck.rs`; KaTeX rides
/// along when the page has (or, in a live preview, may gain) math.
const BASE_CSS: &str = include_str!("../../assets/css/base.css");

// mermaid (pinned) is the one asset still loaded from a CDN rather than bundled:
// it's large (~3 MB) and only needed when a diagram is actually present, so it's
// lazy-loaded by `mermaid.js` (the self-registering enhancer) the first time a
// `{mermaid}` block appears. It's a client-side presentation layer, so it never
// affects the block model or the diff. NOTE: this is the sole exception to the
// "self-contained / offline" guarantee — a built page with a mermaid diagram needs
// network at view time. (Syntax highlighting is server-side; the deck engine and
// KaTeX are all bundled offline.)
const MERMAID: &str = "https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.min.js";

/// The client enhancers: the `window.qmdEnhancers` registry + built-ins (copy
/// buttons, lightbox, link-preview, category-filter) in code-enhance.js, then the
/// self-registering mermaid module (which lazy-loads the mermaid library on first
/// use). Emitted after the registry so it is defined when mermaid registers.
/// Syntax highlighting arrives already done from the server. Callers invoke
/// `window.qmdEnhanceCode(root)` after (re)mounting; it is idempotent.
pub fn code_scripts() -> String {
    let mermaid = MERMAID_JS.replace("{{MERMAID}}", MERMAID);
    format!(
        "<script>{CODE_ENHANCE_JS}</script>\n<script>{mermaid}</script>\n<script>{QMD_JS}</script>\n<script>{WALKTHROUGH_JS}</script>\n<script>{TABSET_JS}</script>\n<script>{SCROLLY_JS}</script>"
    )
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

// Native interactive `{js}` cells: vendored d3 + Observable Plot (UMD globals) the
// cells draw with, shipped only when a page has `{js}` cells. The small enhancer (`qmd-js.js`)
// ships unconditionally in `code_scripts()` (it registers and no-ops without cells,
// like mermaid); only these heavy libs are gated on `has_js_cells`.
const D3_JS: &str = include_str!("../../assets/js/d3.min.js");
const PLOT_JS: &str = include_str!("../../assets/js/plot.umd.min.js");
const QMD_JS: &str = include_str!("../../assets/js/qmd-js.js");

/// `<head>` assets for native `{js}` cells: vendored d3 + Observable Plot. Emit
/// only when a page actually has `{js}` cells (gated on [`has_js_cells`]). The
/// enhancer itself rides in [`code_scripts`].
pub(crate) fn js_cell_head() -> String {
    format!("<script>{D3_JS}</script>\n<script>{PLOT_JS}</script>")
}

/// True if a rendered body contains native `{js}` cells (gates the Plot/d3 libs).
pub fn has_js_cells(body: &str) -> bool {
    body.contains("application/qmd-js")
}

const CODE_ENHANCE_JS: &str = include_str!("../../assets/js/code-enhance.js");
const MERMAID_JS: &str = include_str!("../../assets/js/mermaid.js");
/// Scroll-driven line-range highlighter for `::: {.code-walkthrough}`. Registers
/// through `qmdEnhancers`, no-ops without a walkthrough (like mermaid/qmd-js), so it
/// rides unconditionally in [`code_scripts`].
const WALKTHROUGH_JS: &str = include_str!("../../assets/js/walkthrough.js");
/// ARIA tabs interaction for `::: {.panel-tabset}` (click + arrow-key tab switching).
/// Registers through `qmdEnhancers`, no-ops without a tabset, rides in [`code_scripts`].
const TABSET_JS: &str = include_str!("../../assets/js/tabset.js");
/// Scroll-driven sticky-stage scenes for `::: {.scrolly}`. Registers through `qmdEnhancers`,
/// no-ops without a `.scrolly`, rides in [`code_scripts`].
const SCROLLY_JS: &str = include_str!("../../assets/js/scrolly.js");

/// Heading level (1–6) for a block whose root element is `<hN ...>`/`<hN>`.
pub(crate) fn block_heading_level(html: &str) -> Option<u8> {
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
    dedup_with_suffix(base, counts)
}

/// Make `base` unique within `counts`: the first occurrence is `base`, each repeat
/// gets a `-N` suffix (`base`, `base-1`, `base-2`, …). Shared by heading-slug and
/// block-id deduplication.
fn dedup_with_suffix(base: String, counts: &mut HashMap<String, u32>) -> String {
    let n = counts.entry(base.clone()).or_insert(0);
    let out = if *n == 0 {
        base.clone()
    } else {
        format!("{base}-{n}")
    };
    *n += 1;
    out
}

/// The line of a heading block that carries a trailing `{...}` attribute. For an
/// ATX heading (`## Title {#id}`) that's the whole line; for a setext heading
/// (`Title {#id}` above a `===`/`---` rule) the attribute sits on the text line, so
/// return that, not the underline. Only called when the block is a heading.
fn heading_attr_line(block_src: &str) -> &str {
    let trimmed = block_src.trim_end();
    if let Some((above, rule)) = trimmed.rsplit_once('\n') {
        let r = rule.trim();
        if !r.is_empty() && (r.bytes().all(|b| b == b'=') || r.bytes().all(|b| b == b'-')) {
            // Setext underline: the attribute is on the last text line above it.
            return above.trim_end().lines().last().unwrap_or(above).trim_end();
        }
    }
    trimmed
}

/// A trailing Pandoc attribute on a heading line (`## Title {#id .class}`).
/// Returns `(text_without_attr, explicit_id)`, or `None` when there is no attr.
fn parse_heading_attr(block_src: &str) -> Option<(String, Option<String>)> {
    let line = heading_attr_line(block_src);
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

/// Per-slide background data attributes from a heading's trailing `{...}` —
/// `background-color`/`-image`/`-gradient`/`-size`/`-position`/`-repeat`/`-opacity`
/// become ` data-background-*="..."`. Empty if the heading has no `{...}` or no
/// background keys.
fn heading_section_attrs(block_src: &str) -> String {
    let line = heading_attr_line(block_src);
    let Some(open) = line.rfind('{') else {
        return String::new();
    };
    if !line.ends_with('}') {
        return String::new();
    }
    let attrs = divs::parse_attrs(&line[open + 1..line.len() - 1]);
    let mut out = String::new();
    for (k, v) in &attrs.kv {
        if k.starts_with("background") {
            out.push_str(&format!(" data-{}=\"{}\"", k, escape_attr(v)));
        }
    }
    // `auto-animate` (as `auto-animate=true` or a bare class) marks the slide so the
    // deck engine tweens matched elements into the next auto-animate slide.
    let auto = attrs.kv.iter().any(|(k, _)| k == "auto-animate")
        || attrs.classes.iter().any(|c| c == "auto-animate");
    if auto {
        out.push_str(" data-auto-animate=\"\"");
    }
    out
}

/// Strip a heading's trailing `{...}` (if still present) and inject `bg_data` into
/// its opening tag, so `## T {background-image="x"}` -> `<h2 data-background-image="x">T</h2>`.
fn apply_heading_bg(html: &str, bg_data: &str) -> String {
    let stripped = strip_heading_attr(html);
    match stripped.find('>') {
        Some(gt) => format!("{}{}{}", &stripped[..gt], bg_data, &stripped[gt..]),
        None => stripped,
    }
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
    const TAGS: [&str; 11] = [
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
    ];
    let trimmed = html.trim_end();
    for tag in TAGS {
        let Some(prefix) = trimmed.strip_suffix(tag) else {
            continue;
        };
        // A trailing hard-break backslash sits right before the block's closing tag
        // (`text\</p>`): CommonMark keeps it literal, Pandoc/Quarto drop it. Anchored
        // to the block end, so raw-HTML content containing `\</p>` mid-block (not a
        // hardbreak) is left untouched — the previous global replace could corrupt it.
        let stripped = prefix
            .strip_suffix('\\')
            .or_else(|| prefix.strip_suffix("\\ "));
        return match stripped {
            Some(p) => format!("{p}{tag}{}", &html[trimmed.len()..]),
            None => html.to_string(),
        };
    }
    html.to_string()
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
/// Register a cross-reference anchor → number, keeping the **first** definition and
/// warning on a duplicate label. Otherwise a repeated `{#fig-x}`/`{#sec-x}` silently
/// took the *last* number while the `#fig-x` anchor pointed at the *first* element —
/// so `@fig-x` and the link target disagreed, with no diagnostic.
fn register_xref(
    reg: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
    anchor: &str,
    number: String,
) {
    if reg.contains_key(anchor) {
        warnings.push(Warning::new(format!(
            "duplicate cross-reference label \u{201c}{anchor}\u{201d} (using the first definition)"
        )));
    } else {
        reg.insert(anchor.to_string(), number);
    }
}

fn apply_table_captions(
    blocks: &mut Vec<Block>,
    xrefs: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
) {
    let mut tbl_count = 0u32;
    let mut i = 0;
    while i < blocks.len() {
        // A code cell whose executed output is a numbered table (`#| label: tbl-x`):
        // assign its number in document order (so it interleaves correctly with
        // Markdown tables) and register the xref. The executor injects the matching
        // caption/id into the output using `cell.table.number`.
        if let Some(t) = blocks[i].cell.as_mut().and_then(|c| c.table.as_mut()) {
            tbl_count += 1;
            t.number = tbl_count;
            if let Some(a) = &t.anchor {
                register_xref(xrefs, warnings, a, tbl_count.to_string());
            }
            i += 1;
            continue;
        }
        // A Markdown table directly followed by a `: caption {#tbl-x}` paragraph.
        if i + 1 < blocks.len()
            && blocks[i].html.starts_with("<table")
            && let Some((caption_html, id)) = parse_table_caption(&blocks[i + 1].html)
        {
            tbl_count += 1;
            if let Some(id) = &id {
                register_xref(xrefs, warnings, id, tbl_count.to_string());
            }
            let sep = if caption_html.is_empty() { "" } else { ": " };
            let id_attr = id_attr(id.as_deref());
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
            // Descend: nest a <ul> inside the open parent <li>. When heading levels
            // are skipped (e.g. h1 -> h3, or the first heading is deeper than the
            // base) there is no <li> to hold the next <ul>, so emit a filler <li> —
            // a <ul> may only contain <li>, never another <ul> directly.
            while level < lvl {
                if !open_li {
                    out.push_str("<li>");
                }
                out.push_str("<ul>");
                level += 1;
                open_li = false; // the freshly opened <ul> has no <li> yet
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
    dedup_with_suffix(base, counts)
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

/// Index of the `>` that closes an element's opening tag, skipping any `>` inside
/// a quoted attribute value (so `<a title="a>b">` returns the *final* `>`, not the
/// one in the title). `None` if the tag is unterminated. Used by the string-surgery
/// helpers that splice a class/attribute into an already-emitted opening tag — a
/// naive `find('>')` would split inside an attribute value.
pub(crate) fn tag_end(html: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (i, &b) in html.as_bytes().iter().enumerate() {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'>' => return Some(i),
                _ => {}
            },
        }
    }
    None
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

/// A numbered figure/listing caption: `"<Label>&nbsp;<num>"`, with `": <caption>"`
/// appended (HTML-escaped) when a non-empty caption is given. Shared by the
/// figure, listing, mermaid, and `{js}`-figure emitters.
fn numbered_caption(label: &str, num: usize, caption: Option<&str>) -> String {
    match caption.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("{label}&nbsp;{num}: {}", html_escape(c)),
        None => format!("{label}&nbsp;{num}"),
    }
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

/// If `line` is a leading cell-option directive, return the content after the pipe.
/// Recognizes `#|` (most langs), `//|` (JS), `%%|` (mermaid), each tolerating optional
/// whitespace between the comment marker and the pipe (`# |`, `// |`, `%% |`) — Quarto
/// accepts the spaced form, so the corpus may use it (e.g. `posts/pca-geometry`).
/// Returns `None` for a plain comment or code line. This is the single primitive every
/// option parser keys off (`cell_option`, `strip_cell_options`, `validate`).
pub(crate) fn option_directive(line: &str) -> Option<&str> {
    let t = line.trim_start();
    for marker in ["#", "//", "%%"] {
        if let Some(rest) = t.strip_prefix(marker) {
            return rest.trim_start_matches([' ', '\t']).strip_prefix('|');
        }
    }
    None
}

/// Read a leading `#| key: value` cell option (returns the unquoted value).
/// Only scans the contiguous leading option block, stopping at the first code
/// line. See [`option_directive`] for the recognized prefixes.
fn cell_option<'a>(literal: &'a str, key: &str) -> Option<&'a str> {
    for line in literal.lines() {
        let Some(opt) = option_directive(line) else {
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
///   cache: false
/// ```
///
/// Returns `(echo, include, cache)`, each defaulting to `true`. Per-cell `#|`
/// options override these. (`eval`/`output`/`warning` are not yet honoured.)
fn detect_execute_defaults(front_matter: &str) -> (bool, bool, bool) {
    // Apply one `key: value` pair from an `execute:` mapping (shared by the block
    // and the inline flow form).
    fn apply_kv(k: &str, v: &str, echo: &mut bool, include: &mut bool, cache: &mut bool) {
        let v = v.trim().trim_matches(['"', '\'']);
        match k.trim() {
            "echo" => *echo = v != "false",
            "include" => *include = v != "false",
            "cache" => *cache = v != "false",
            _ => {}
        }
    }

    let (mut echo, mut include, mut cache) = (true, true, true);
    let mut in_block = false;
    for line in front_matter.lines() {
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();
        if !in_block {
            if indent == 0
                && let Some(rest) = t.strip_prefix("execute:")
            {
                let rest = rest.trim();
                if let Some(inner) = rest.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    // Flow form on one line: `execute: {echo: false, cache: false}`.
                    for pair in inner.split(',') {
                        if let Some((k, v)) = pair.split_once(':') {
                            apply_kv(k, v, &mut echo, &mut include, &mut cache);
                        }
                    }
                } else if rest.is_empty() {
                    in_block = true; // block form: indented lines follow
                }
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
            apply_kv(k, v, &mut echo, &mut include, &mut cache);
        }
    }
    (echo, include, cache)
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

/// Drop leading cell-option lines (`#|` for most languages, `//|` for JS,
/// `%%|` for mermaid; see [`option_directive`] for the spaced forms too).
fn strip_cell_options(literal: &str) -> String {
    let mut body = String::new();
    let mut skipping = true;
    for line in literal.lines() {
        if skipping && option_directive(line).is_some() {
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

/// Language for a fenced block: `{python}`/`{.python}`/`{js}` -> "python"/"js",
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

/// Minimal standard-alphabet base64 (mirrors `build.rs`); used to inline the
/// favicon as a `data:` URI (see [`page::favicon_link`]).
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

/// Parse the native `{js}` cell options (`//| name:`/`//| viewof:`/`//| input:`)
/// from the raw fence body. Empty for every other language.
fn parse_js_opts(literal: &str, lang: &str) -> JsOpts {
    if lang != "js" {
        return JsOpts::default();
    }
    JsOpts {
        name: cell_option(literal, "name").map(str::to_string),
        viewof: cell_option(literal, "viewof").map(str::to_string),
        inputs: cell_option(literal, "input")
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Emit a native interactive `{js}` cell: an output target div plus an
/// `application/qmd-js` script carrying the author source verbatim (only `</script`
/// escaped, so it is readable in devtools — no base64). The `data-*` attrs tell the
/// `qmd-js` enhancer how to wire the cell (shared-scope name, named input, re-run
/// inputs). Block data attrs ride on the wrapper for click-to-source.
fn emit_js_cell(src: &str, block_id: &str, js: &JsOpts, block_attrs: &str) -> String {
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
        "<div{block_attrs} class=\"cell qmd-js-cell\"><div class=\"qmd-js-out\" id=\"{target}\"></div>\
         <script type=\"application/qmd-js\"{data}>{safe_src}</script></div>"
    )
}

/// Wrap a native `{js}` cell in a numbered `<figure>` (for `label: fig-x` js cells,
/// e.g. a Three.js scene). The block attrs + `#fig-` anchor ride on the figure.
fn emit_js_figure(
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

/// Make already-entity-escaped HTML *text* (e.g. a rendered caption with its tags
/// stripped via [`strip_tags`]) safe inside a double-quoted attribute. Existing
/// entities are valid in an attribute value, so only the `"` needs escaping —
/// running [`escape_attr`] here would double-escape `&` (`&amp;` -> `&amp;amp;`).
pub(crate) fn escape_attr_from_html(s: &str) -> String {
    s.replace('"', "&quot;")
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

// The deck engine (deck.css/deck.js) is bundled into the page like KaTeX —
// decks render with no network, the same as every other format.

#[cfg(test)]
mod tests;
