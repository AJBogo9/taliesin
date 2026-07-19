//! Parse `.tmd` source with comrak (sourcepos-aware) and emit our own HTML.
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

mod model;
pub(crate) use model::CellRole;
pub use model::{
    AssetMode, Block, Cell, CellFigure, CellTable, DocFormat, ExternalAssets, JsOpts, OutputMode,
    PageIncludes, RenderedDoc, Warning,
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
    // Smart typography (curly quotes, en/em dashes) to match Pandoc output.
    options.parse.smart = true;
    // sourcepos is tracked on AST nodes during parsing; `render.sourcepos`
    // only affects comrak's own formatter, which we don't use.
    options
}

/// Parse `src` into ordered top-level blocks with stable ids + sourcepos.
/// Does not resolve `{{< include >}}` (use [`render_document_with_includes`]).
mod doc_includes;
pub use doc_includes::includes_from_parts;
use doc_includes::resolve_doc_includes;
mod fm_extract;
pub(crate) use fm_extract::bibliography_paths;
pub use fm_extract::is_reveal_doc;
use fm_extract::{
    Numbered, TheoremConfig, detect_format, detect_title_block_hidden, detect_toc, extract_field,
    parse_theorem_config,
};
mod cell_extract;
pub(crate) use cell_extract::option_directive;
use cell_extract::{
    cell_flag_or, cell_option, code_fold, code_lang, detect_execute_defaults, hidden_cell,
    is_executable_fence, parse_js_opts, slice_lines, strip_cell_options,
};
mod cell_numbered;
pub(crate) use cell_numbered::numbered_caption;
use cell_numbered::{emit_code_listing, emit_js_cell, emit_js_figure};
mod deck;
pub use deck::{DeckParts, assemble_deck_page, deck_client_script, deck_slide_blocks, slides_html};
// `deck_theme_head` is used inside `deck.rs` (the deck builder) and by the unit
// tests; it's not part of the public API, so it's only pulled into scope here for
// the tests rather than re-exported.
#[cfg(test)]
use deck::deck_theme_head;
mod extension;
pub use extension::embed_targets;
mod divs;
mod validate;
pub(crate) use divs::parse_attrs;
use divs::{group_divs, parse_pandoc_attrs, preprocess, scan_div_spans};

// Re-exported for the editor vocabulary dump (crate::vocab), which sources completion
// vocabulary from the SAME consts the validator enforces so the two cannot drift.
pub(crate) use validate::{CALLOUT_KINDS, CELL_OPTION_KEYS, INPUT_TYPES, THEOREM_KINDS};
// Only the `vocab.rs` drift test reads this outside `render`; the validator uses it directly.
#[cfg(test)]
pub(crate) use validate::DIV_FEATURE_CLASSES;

mod emit;
use emit::emit;
// emit_children is re-exported so the sibling figure module reaches it via `super`.
pub(crate) use emit::emit_children;
pub(crate) use emit::safe_url;
mod figure;
use figure::{emit_figure, emit_mermaid_figure, figure_parts};
// Text projection (`taliesin read`): a plain-text VIEW of the block model, not an output
// format. Crate-internal; reached via `RenderedDoc::body_text()`.
mod text;
// The search index's text extraction, shared with the `read`/TOC/slug path above rather
// than re-derived in `site/` (where a weaker copy silently indexed KaTeX three times).
pub(crate) use text::indexable_text;
mod theme;
// Used only by the page builders; kept crate-internal, not part of the public API.
pub(crate) use theme::theme_head;
mod page;
use page::page_from_doc;
pub use page::{
    PageParts, SiteCtx, assemble_html_page, favicon_link, html_page_from_doc_in_site,
    html_page_from_doc_in_site_external, render_doc_to_page, title_with_site_suffix,
};
// Crate-internal: `Site::page_title` is the entry point for resolving a page's tab title.
pub(crate) use page::site_page_title;
use theme::{detect_theme, resolve_theme, theme_default_mode, theme_style};

/// Render a `.tmd` source string into the `RenderedDoc` block model: the parse
/// step only (no code execution, no page chrome). The dev server diffs these
/// block lists for incremental updates; the CLI wraps the result in a page.
///
/// `title` is the front-matter `title:` only. A leading `# H1` titles the *page* (see
/// [`render_doc_to_page`]) but is not folded in here, so a site can still prefer an
/// authored `_site.yml` chapter title over the heading text.
///
/// ```
/// let doc = taliesin_core::render_document("# Title\n\nHello *world*.\n");
/// assert_eq!(doc.title, None); // no front-matter title
/// assert_eq!(doc.blocks.len(), 2); // the heading + the paragraph
/// assert!(doc.blocks[0].html.contains("<h1"));
/// ```
pub fn render_document(src: &str) -> RenderedDoc {
    render_internal(src, None, None, None, None)
}

/// The languages Taliesin executes against a warm kernel, whose *output block* can
/// therefore carry a figure/table anchor. This is the canonical set: the render pass
/// reserves a `@fig-`/`@tbl-` number only for a lang that will actually produce the
/// float, and `taliesin-server`'s `exec::kernel_lang` (which does the running) is
/// drift-locked to it by a test. A lang that is neither executed here nor emitted at
/// render time (mermaid/`{js}`) — `{bash}`, `{sql}`, `{julia}`, … — produces no float,
/// so labelling one as a figure/table must NOT burn a number or register a phantom
/// anchor.
pub fn executes_to_kernel(lang: &str) -> bool {
    matches!(lang, "python" | "r")
}

/// Like [`render_document`], but first expands `{{< include >}}` shortcodes
/// relative to `base_dir`, mapping each block back to its origin file, and
/// resolves citations/cross-references against the doc's bibliography.
pub fn render_document_with_includes(src: &str, base_dir: &Path) -> RenderedDoc {
    render_document_with_includes_scoped(src, base_dir, None)
}

/// Like [`render_document_with_includes`] but with an optional book chapter number, so a
/// numbered chapter renders "Figure 2.3" / "Theorem 2.3". Only the site book path passes
/// `Some(n)`; everything else is `None` (continuous numbering).
pub fn render_document_with_includes_scoped(
    src: &str,
    base_dir: &Path,
    chapter: Option<u32>,
) -> RenderedDoc {
    render_doc_with_includes_impl(src, base_dir, chapter, None)
}

/// Like [`render_document_with_includes`], but confining `{{< include >}}` and the
/// front-matter resource paths (`css:` / `bibliography:` / `csl:` / `include-*`) to an
/// explicit containment `root` (see [`crate::includes::safe_join_in`]) instead of the
/// inferred `.git`/`_site.yml` walk. First-party single-document preview/build passes the
/// invoked doc's own directory here, so an untrusted document dropped inside a larger
/// checkout cannot climb out of it to read a sibling repo-local file. `None` keeps the
/// walk (the multi-page site path and the corpus's loose `../../_includes/` fixture).
pub fn render_document_with_includes_rooted(
    src: &str,
    base_dir: &Path,
    root: Option<&Path>,
) -> RenderedDoc {
    render_doc_with_includes_impl(src, base_dir, None, root)
}

fn render_doc_with_includes_impl(
    src: &str,
    base_dir: &Path,
    chapter: Option<u32>,
    root: Option<&Path>,
) -> RenderedDoc {
    let (expanded, origins, include_warnings) =
        crate::includes::resolve_warned_in(src, base_dir, root);
    // Declarative shortcodes (`{{< embed >}}` / `{{< video >}}` / `{{< input >}}`)
    // expand after includes, line-preserving so `origins` stays valid. A `{{< name >}}`
    // that no built-in declares is left verbatim but reported, so a typo'd shortcode
    // doesn't ship silently as literal text.
    let (expanded, shortcode_warnings) = extension::expand_shortcodes(&expanded);
    let mut doc = render_internal(&expanded, Some(&origins), Some(base_dir), root, chapter);
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
    include_root: Option<&Path>,
    chapter: Option<u32>,
) -> RenderedDoc {
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                render_internal_impl(src, origins, base_dir, include_root, chapter)
            }) {
            Ok(handle) => match handle.join() {
                Ok(doc) => doc,
                Err(payload) => std::panic::resume_unwind(payload),
            },
            // Spawning the big-stack worker can fail under a strict address-space
            // limit (e.g. `ulimit -v`). Fall back to rendering inline on the current
            // (default-stack) thread rather than panicking — same as before this guard.
            Err(_) => render_internal_impl(src, origins, base_dir, include_root, chapter),
        }
    })
}

fn render_internal_impl(
    src: &str,
    origins: Option<&[LineOrigin]>,
    base_dir: Option<&Path>,
    include_root: Option<&Path>,
    chapter: Option<u32>,
) -> RenderedDoc {
    let arena = Arena::new();
    let options = parse_options();
    // fenced divs (`:::`) aren't CommonMark. Record their spans first,
    // then strip the fence markers in a line-preserving pass so sourcepos line
    // numbers stay exact and the inner content parses as normal blocks. The
    // recorded spans are used afterwards to wrap blocks back up as callouts etc.
    let (spans, unclosed_fences) = scan_div_spans(src);
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
    let mut bib_paths: Vec<String> = Vec::new();
    let mut includes = PageIncludes::default();
    // Non-fatal render warnings (a missing `bibliography:`/`theme:` file, …),
    // collected through the whole render and surfaced in the dev menu / build log.
    let mut warnings: Vec<Warning> = Vec::new();
    // Validate the document's front matter against taliesin's vocabulary (top-level
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
    // An unterminated `:::` fence drops its wrapper silently (the callout/columns never
    // form and the content renders unfenced) — surface it as a click-to-source warning.
    for open in unclosed_fences {
        let (file, mapped) = map_origin(origins, open);
        warnings.push(
            Warning::new(
                "unterminated `:::` fenced div: add a closing `:::` \u{2014} the block is \
                 rendered without its wrapper",
            )
            .at(file, mapped as u32),
        );
    }
    // Document-level cell defaults from a front-matter `execute:` block; a cell's
    // own `#| echo`/`#| include`/`#| cache` overrides these.
    let mut exec_echo = true;
    let mut exec_include = true;
    let mut exec_cache = true;
    let mut theorem_config = TheoremConfig::default();
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
    // Hierarchical section counters (h2..h6) for a book chapter, advanced over EVERY
    // heading in document order so a `{#sec-x}` registers the same number the heading
    // visibly shows via `number_chapter_headings` (they share `section_number`).
    let mut sec_counters = [0u32; 5];
    let mut xref_registry: HashMap<String, String> = HashMap::new();

    for node in root.children() {
        // Footnote definitions are gathered into a section at the end (comrak has
        // already moved them here, in reference order) — not rendered in place.
        let fn_def = {
            let data = node.data.borrow();
            match &data.value {
                // Keep the definition's OWN source range: comrak has moved the node to
                // the document end, but its sourcepos still points at where the author
                // wrote it, which is the line click-to-source must land on.
                NodeValue::FootnoteDefinition(fd) => {
                    let sp = data.sourcepos;
                    let (file, start_line) = map_origin(origins, sp.start.line);
                    let (_, end_line) = map_origin(origins, sp.end.line);
                    Some((
                        fd.name.clone(),
                        format!(
                            "{}:{}-{}:{}",
                            start_line, sp.start.column, end_line, sp.end.column
                        ),
                        file,
                    ))
                }
                _ => None,
            }
        };
        if let Some((name, sourcepos, file)) = fn_def {
            footnote_items.push(emit::footnote_def_li(
                node,
                &name,
                &sourcepos,
                file.as_deref(),
            ));
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
                bib_paths = bibliography_paths(fm);
                format = detect_format(fm);
                toc_explicit = detect_toc(fm);
                hide_title_block = detect_title_block_hidden(fm);
                theme = detect_theme(fm);
                includes = resolve_doc_includes(fm, base_dir, include_root);
                (exec_echo, exec_include, exec_cache) = detect_execute_defaults(fm);
                theorem_config = parse_theorem_config(fm);
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
            // Executable code cell: ```{lang} ... ``` (lang detected, options stripped).
            // A LEADING DOT (```{.python}) is the documented display-only form, so it is
            // NOT a cell — see `is_executable_fence`.
            let cell = match &data.value {
                NodeValue::CodeBlock(cb) if is_executable_fence(&cb.info) => code_lang(&cb.info)
                    .map(|lang| {
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
                    }),
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
            // Validate this code cell's `#|` options against taliesin's vocabulary
            // (a typo or a legacy key becomes a located, click-to-source warning;
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
        // A heading may carry a Pandoc attribute (`## Title {#sec-x}`): use
        // an explicit `#id` as the anchor (else a slug of the cleaned text), and
        // strip the attribute from the rendered heading below.
        let h_attr = heading_level.and_then(|_| parse_heading_attr(&block_src));
        // Advance the hierarchical section counters over EVERY heading (in a book
        // chapter), so a labelled `{#sec-x}` registers the same number its heading
        // will visibly show — even when earlier, unlabelled headings sit between them.
        // Outside a chapter there is no hierarchy: keep the flat sequential counter.
        let hierarchical_number = heading_level.and_then(|level| {
            chapter.map(|ch| crate::site::section_number(ch, level as usize, &mut sec_counters))
        });
        // A heading labelled `{#sec-x}` is numbered so `@sec-x` resolves to "Section N":
        // the chapter-hierarchical number ("2.2") in a book, else a flat sequential one.
        if let Some((_, Some(id))) = &h_attr
            && id.starts_with("sec-")
        {
            let number = match &hierarchical_number {
                Some(n) => n.clone(),
                None => {
                    sec_count += 1;
                    sec_count.to_string()
                }
            };
            register_xref(
                &mut xref_registry,
                &mut warnings,
                id,
                number,
                source_file.as_deref(),
                buf_start as u32,
            );
        }
        let id_attr = match heading_level {
            Some(_) if format == DocFormat::Html => {
                let id = match &h_attr {
                    // An explicit `{#id}` must be deduped too: two same explicit ids
                    // (or an explicit id colliding with an autoslug) would otherwise
                    // emit DUPLICATE element ids, silently breaking in-page anchors
                    // and `@sec-` refs. Route it through the SAME `heading_slugs` map
                    // the autoslug path uses (so explicit-vs-autoslug collisions are
                    // caught), and warn on the duplicate.
                    Some((_, Some(id))) => {
                        let deduped = dedup_with_suffix(id.clone(), &mut heading_slugs);
                        if &deduped != id {
                            warnings.push(
                                Warning::new(format!(
                                    "duplicate heading id \u{201c}{id}\u{201d} (using \u{201c}{deduped}\u{201d}; in-page links to \u{201c}{id}\u{201d} may not resolve)"
                                ))
                                .at(source_file.clone(), buf_start as u32),
                            );
                        }
                        deduped
                    }
                    Some((clean, None)) => {
                        dedup_slug(&strip_math_for_slug(clean), &mut heading_slugs)
                    }
                    None => dedup_slug(&strip_math_for_slug(&block_src), &mut heading_slugs),
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
        // Pandoc treats a bare `\begin{env}...\end{env}` block as display
        // math even without `$$`; comrak doesn't, so detect and render it here.
        if let Some(env) = is_paragraph.then(|| bare_math_env(&block_src)).flatten() {
            html.push_str(&format!("<div{attrs} class=\"tali-math-block\">"));
            html.push_str(&crate::math::render(env, true));
            html.push_str("</div>");
        } else if let Some((latex, anchor)) = is_paragraph
            .then(|| labelled_display_eq(&block_src))
            .flatten()
        {
            // `$$ ... $$ {#eq-x}` -> a numbered display equation; register the
            // `#eq-` id so `@eq-x` cross-references resolve to "Equation N".
            eq_count += 1;
            let eq_num = float_number(chapter, eq_count);
            register_xref(
                &mut xref_registry,
                &mut warnings,
                &anchor,
                eq_num.clone(),
                source_file.as_deref(),
                buf_start as u32,
            );
            html.push_str(&emit_equation(&latex, &anchor, &attrs, &eq_num));
        } else if let Some(fig) = is_paragraph.then(|| figure_parts(node)).flatten() {
            // Standalone image -> a numbered `<figure>`; register `#fig-` ids so
            // `@fig-x` cross-references resolve to the number.
            fig_count += 1;
            let fig_num = float_number(chapter, fig_count);
            if let Some(fid) = fig.attrs.id.as_deref().filter(|i| i.starts_with("fig-")) {
                register_xref(
                    &mut xref_registry,
                    &mut warnings,
                    fid,
                    fig_num.clone(),
                    source_file.as_deref(),
                    buf_start as u32,
                );
            }
            html.push_str(&emit_figure(&fig, &attrs, &fig_num));
        } else if let Some(role) = &cell_role {
            // A labelled/captioned code cell -> a numbered, anchored figure/listing.
            let lang = cell.as_ref().map(|c| c.lang.clone()).unwrap_or_default();
            let code = cell.as_ref().map(|c| c.code.clone()).unwrap_or_default();
            match role {
                CellRole::Figure { anchor, caption } => {
                    // Register from what will EXIST, like the `Listing` arm below — not
                    // from what the label declares. A figure materializes only when it is
                    // emitted here at render time (mermaid/`{js}`) OR the cell executes
                    // against a kernel (python/r) with its output kept (`include: true`);
                    // `include: false` drops that output block outright (`exec.rs`).
                    //
                    // Any OTHER lang — `{bash}`, `{sql}`, `{julia}`, … — is neither
                    // emitted here nor executed, so it has no figure for ANY value of
                    // `include`. Registering an anchor + burning a number for one would
                    // point `@fig-x` at a "Figure N" no element carries and shift every
                    // later figure down by one. `executes_to_kernel` is the canonical
                    // executable set (`exec::kernel_lang` is drift-locked to it).
                    let include = cell.as_ref().is_none_or(|c| c.include);
                    let emitted_at_render_time = matches!(lang.as_str(), "mermaid" | "js");
                    if !(emitted_at_render_time || (executes_to_kernel(&lang) && include)) {
                        if let Some(a) = anchor {
                            warnings.push(if include {
                                unreferenceable_nonexec_label(
                                    "figure",
                                    a,
                                    &lang,
                                    source_file.clone(),
                                    buf_start,
                                )
                            } else {
                                unreferenceable_hidden_label(
                                    "figure",
                                    a,
                                    source_file.clone(),
                                    buf_start,
                                )
                            });
                        }
                        // A visible non-executable cell keeps its source (the author wants
                        // to show the code), just not wrapped as a numbered figure. An
                        // `include: false` cell still RUNS (if executable) but its output
                        // is dropped, so hide the source.
                        if include {
                            emit(node, &attrs, &mut html);
                        } else {
                            html.push_str(&hidden_cell(&attrs));
                        }
                    } else {
                        fig_count += 1;
                        let fig_num = float_number(chapter, fig_count);
                        if let Some(a) = anchor {
                            register_xref(
                                &mut xref_registry,
                                &mut warnings,
                                a,
                                fig_num.clone(),
                                source_file.as_deref(),
                                buf_start as u32,
                            );
                        }
                        match lang.as_str() {
                            // Client-rendered outputs are known now, so wrap them here.
                            "mermaid" => html.push_str(&emit_mermaid_figure(
                                &code,
                                anchor.as_deref(),
                                caption.as_deref(),
                                &attrs,
                                &fig_num,
                            )),
                            "js" => html.push_str(&emit_js_figure(
                                &code,
                                &id,
                                cell.as_ref().map(|c| &c.js),
                                anchor.as_deref(),
                                caption.as_deref(),
                                &attrs,
                                &fig_num,
                            )),
                            // Python/R: the source renders now; tag the cell so the
                            // executor wraps the (later) output in the numbered figure.
                            _ => {
                                if let Some(c) = cell.as_mut() {
                                    c.figure = Some(CellFigure {
                                        anchor: anchor.clone(),
                                        caption: caption.clone(),
                                        number: fig_num.clone(),
                                    });
                                }
                                // `echo: false` hides the code but keeps the figure
                                // tagging, so the executed output still becomes Figure N.
                                // (`include: false` never reaches here — it is handled
                                // above, where the figure is known not to materialize.)
                                if cell.as_ref().is_some_and(|c| !c.echo) {
                                    html.push_str(&hidden_cell(&attrs));
                                } else {
                                    emit(node, &attrs, &mut html);
                                }
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
                        let lst_num = float_number(chapter, lst_count);
                        if let Some(a) = anchor {
                            register_xref(
                                &mut xref_registry,
                                &mut warnings,
                                a,
                                lst_num.clone(),
                                source_file.as_deref(),
                                buf_start as u32,
                            );
                        }
                        html.push_str(&emit_code_listing(
                            &code,
                            &lang,
                            anchor.as_deref(),
                            caption.as_deref(),
                            fold.as_ref(),
                            &attrs,
                            &lst_num,
                        ));
                    }
                }
                CellRole::Table { anchor, caption } => {
                    // The table is the cell's executed output (e.g. a pandas
                    // DataFrame), so tag the cell and let the executor inject the
                    // caption/id; the number is assigned in document order by
                    // `apply_table_captions` below. The source renders now (or is
                    // hidden by `echo: false`), like a figure cell.
                    //
                    // A table has no render-time emission path at all — unlike a figure,
                    // which mermaid/`{js}` can produce here — so it materializes ONLY when
                    // the cell executes (python/r) with its output kept. `include: false`
                    // or a non-executable lang (`{bash}`, `{sql}`, …) means no `<table>`
                    // and no anchor; leaving `c.table` unset is what keeps
                    // `apply_table_captions` from numbering and registering a phantom.
                    let include = cell.as_ref().is_none_or(|c| c.include);
                    if executes_to_kernel(&lang) && include {
                        if let Some(c) = cell.as_mut() {
                            c.table = Some(CellTable {
                                anchor: anchor.clone(),
                                caption: caption.clone(),
                                // Filled in document order by `apply_table_captions`.
                                number: String::new(),
                            });
                        }
                        if cell.as_ref().is_some_and(|c| !c.echo) {
                            html.push_str(&hidden_cell(&attrs));
                        } else {
                            emit(node, &attrs, &mut html);
                        }
                    } else {
                        if let Some(a) = anchor {
                            warnings.push(if include {
                                unreferenceable_nonexec_label(
                                    "table",
                                    a,
                                    &lang,
                                    source_file.clone(),
                                    buf_start,
                                )
                            } else {
                                unreferenceable_hidden_label(
                                    "table",
                                    a,
                                    source_file.clone(),
                                    buf_start,
                                )
                            });
                        }
                        // A visible non-executable cell keeps its source; an
                        // `include: false` cell hides it (the executor drops the output).
                        if include {
                            emit(node, &attrs, &mut html);
                        } else {
                            html.push_str(&hidden_cell(&attrs));
                        }
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
            // CommonMark leaves it literal, but Pandoc drops it. Match Pandoc.
            html = strip_trailing_hardbreak(&html);
        }
        // One <h1> per page: when this render emits a visible title block, demote every
        // body heading one level so sections nest beneath the title. The gate mirrors the
        // title-block insertion condition exactly (Html, not hidden, titled). Decks
        // (Reveal) and books (untitled, numbered chapters) never satisfy it, so their
        // slide-break and section-numbering machinery is never entered.
        if let Some(level) = heading_level
            && format == DocFormat::Html
            && !hide_title_block
            && title.is_some()
        {
            html = demote_heading_html(&html, level);
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
    apply_table_captions(&mut blocks, &mut xref_registry, &mut warnings, chapter);
    // Theorem environments: number per-kind in document order + register #thm-/#lem-/…
    // anchors. Must run before cite::process resolves @thm-/@lem-/… references.
    number_theorems(
        &mut blocks,
        &mut xref_registry,
        &mut warnings,
        &theorem_config,
        chapter,
    );
    let bib_line = crate::frontmatter::bibliography_line(src);
    let bib = load_bibliography(&bib_paths, base_dir, include_root, bib_line, &mut warnings);
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
    // Reading-time estimate, shown only for a dated post (the same gate as og:type=article).
    // Prose words / 200 wpm, rounded to whole minutes (min 1), matching the client's live
    // count; `src` is include-expanded, so an included file's prose is included.
    let read_time = date.as_deref().filter(|d| !d.is_empty()).map(|_| {
        let mins = ((crate::prose::word_count(src) + 100) / 200).max(1);
        format!("{mins} min read")
    });
    if format == DocFormat::Html
        && !hide_title_block
        && let Some(tb) = title_block_html(
            title.as_deref(),
            subtitle.as_deref(),
            author.as_deref(),
            date.as_deref(),
            description.as_deref(),
            read_time.as_deref(),
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
    let theme_css = resolve_theme(theme.as_deref(), base_dir, include_root, &mut warnings);
    let theme_default = theme_default_mode(theme.as_deref()).to_string();
    let theme_is_custom = !theme_css.trim().is_empty();
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
        xref_numbers: xref_registry,
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

/// Humanize an ISO `YYYY-MM-DD` date for display ("2026-04-14" → "14 April 2026"),
/// day un-padded. Any value that isn't a valid plain `YYYY-MM-DD` (a free-form date the
/// author wrote, or one carrying a time) is returned unchanged, so nothing an author
/// typed is ever mangled. Shared by the post title block and the listing cards; the
/// machine-readable ISO form is emitted separately (JSON-LD, `citation_*`, the feed).
pub(crate) fn humanize_date(date: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    // A date carrying a time prints verbatim: humanizing it would silently drop the time
    // the author wrote. That `T` rule is this function's own — `calendar_date` answers only
    // "which day is this", which the feed and sitemap ask of the same value.
    if !date.contains('T')
        && let Some((y, month, day)) = crate::frontmatter::calendar_date(date)
    {
        return format!("{day} {} {y}", MONTHS[month as usize - 1]);
    }
    date.to_string()
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
    read_time: Option<&str>,
) -> Option<String> {
    let title = title?;
    let mut h = String::from(
        "<header class=\"tali-title-block\" data-block-id=\"qmd-title-block\"><h1 class=\"title\">",
    );
    h.push_str(&html_escape(title));
    h.push_str("</h1>");
    if let Some(s) = subtitle.filter(|s| !s.is_empty()) {
        h.push_str(&format!("<p class=\"subtitle\">{}</p>", html_escape(s)));
    }
    if let Some(d) = description.filter(|s| !s.is_empty()) {
        h.push_str(&format!("<p class=\"description\">{}</p>", html_escape(d)));
    }
    let author_span = author
        .filter(|s| !s.is_empty())
        .map(|s| format!("<span>{}</span>", html_escape(s)));
    // The date is humanized for display ("2026-04-14" → "14 April 2026"); a value the
    // author wrote that isn't a plain ISO date is shown verbatim (never mangled).
    let date_span = date
        .filter(|s| !s.is_empty())
        .map(|s| format!("<span>{}</span>", html_escape(&humanize_date(s))));
    // A subtle reading-time estimate ("N min read"), only when the caller supplies one
    // (a dated post). Rides the same muted meta line as author · date.
    let read_span = read_time
        .filter(|s| !s.is_empty())
        .map(|s| format!("<span class=\"tali-read-time\">{}</span>", html_escape(s)));
    let meta: Vec<String> = [author_span, date_span, read_span]
        .into_iter()
        .flatten()
        .collect();
    if !meta.is_empty() {
        h.push_str(&format!(
            "<div class=\"tali-title-meta\">{}</div>",
            meta.join("")
        ));
    }
    h.push_str("</header>");
    Some(h)
}

/// The built-in dark theme, scoped to `html[data-theme="dark"]` so it can be
/// flipped at runtime (the toggle / OS preference set the attribute). Always
/// shipped alongside the light `:root` base. The `:root` light vars plus this
/// block are the reference template for a community theme.
const DARK_CSS: &str = include_str!("../../assets/css/dark.css");

/// Load and merge the bibliography file(s) named in the front matter, resolved
/// relative to `base_dir`. Returns an empty bibliography when none is found
/// (citations still de-leak; cross-references still resolve).
fn load_bibliography(
    paths: &[String],
    base_dir: Option<&Path>,
    root: Option<&Path>,
    bib_line: Option<u32>,
    warnings: &mut Vec<Warning>,
) -> crate::cite::Bibliography {
    let Some(base) = base_dir else {
        return crate::cite::Bibliography::default();
    };
    // Point every `.bib` diagnostic at the front-matter `bibliography:` line (the
    // .bib is an external file with no in-doc position of its own), so it is
    // click-to-source rather than an unlocated warning.
    let locate = |w: Warning| match bib_line {
        Some(l) => w.at(None, l),
        None => w,
    };
    let mut text = String::new();
    for path in paths {
        let path = path.trim();
        // Only `.bib` is supported; a differently-suffixed path (a stray token or an
        // unsupported CSL-JSON/YAML) is skipped rather than mis-read — but warn, since it
        // would otherwise silently fail to resolve any of its citations.
        if !path.ends_with(".bib") {
            warnings.push(locate(Warning::new(format!(
                "bibliography `{path}` ignored: only BibTeX (`.bib`) is supported"
            ))));
            continue;
        }
        match crate::includes::safe_join_in(base, path, root)
            .and_then(|p| std::fs::read_to_string(&p).ok())
        {
            Some(content) => {
                text.push_str(&content);
                text.push('\n');
            }
            // An explicitly named `.bib` that can't be read (or escapes the project
            // root) is a typo worth flagging: citations would otherwise just
            // silently fail to resolve.
            None => warnings.push(locate(Warning::new(format!(
                "bibliography file not found: {path}"
            )))),
        }
    }
    let (bib, bib_warnings) = crate::cite::parse_bib_warned(&text);
    warnings.extend(bib_warnings.into_iter().map(|m| locate(Warning::new(m))));
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
/// let html = taliesin_core::render_html_page("---\ntitle: Demo\n---\n\nHi.\n", "fallback");
/// assert!(html.contains("<title>Demo</title>"));
/// assert!(html.contains("Hi."));
/// ```
pub fn render_html_page(src: &str, fallback_title: &str) -> String {
    // The in-process full-page API ships everything (like a preview); the static
    // `build`/`render` CLI opts into content-gating via `render_doc_to_page`.
    page_from_doc(&render_document(src), fallback_title, OutputMode::Preview)
}

/// Like [`render_html_page`], resolving `{{< include >}}` relative to `base_dir`.
pub fn render_html_page_with_includes(src: &str, base_dir: &Path, fallback_title: &str) -> String {
    page_from_doc(
        &render_document_with_includes(src, base_dir),
        fallback_title,
        OutputMode::Preview,
    )
}

/// Self-contained KaTeX stylesheet (fonts inlined as data URIs at build time).
const KATEX_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/katex-inlined.css"));

/// The owned body typeface: Newsreader `@font-face` rules with the two variable woff2
/// faces (roman + italic) inlined as data URIs at build time (see `build.rs`). Emitted
/// ahead of the base stylesheet so `--tali-font-body` resolves to the loaded face.
pub(crate) const FONTS_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/fonts-inlined.css"));

/// The owned design tokens (light + sepia palette, fonts, geometry, motion),
/// `include_str!`'d ahead of BOTH `base.css` (the page) and `deck.css` (the deck) so
/// the palette is declared exactly once. See `tokens.css`. The dark palette override
/// is `TOKENS_DARK_CSS` (kept separate so a `--bare` page can flatten just that layer).
pub(crate) const TOKENS_CSS: &str = include_str!("../../assets/css/tokens.css");
/// The dark palette override (keyed on `html[data-theme="dark"]` + the deck's
/// `html.tali-deck-dark`), shared by the page and the deck. See `TOKENS_CSS`.
pub(crate) const TOKENS_DARK_CSS: &str = include_str!("../../assets/css/tokens-dark.css");

/// Base document styling (typography, tables, callouts, references, block
/// highlight). Emitted by the page builders in `page.rs`/`deck.rs`; KaTeX rides
/// along when the page has (or, in a live preview, may gain) math.
const BASE_CSS: &str = include_str!("../../assets/css/base.css");

// mermaid (pinned) is loaded as a separate script rather than bundled: the library is
// large (~2.8 MB) and only needed when a diagram is actually present, so it's lazy-loaded
// by `mermaid.js` (the self-registering enhancer) the first time a `{mermaid}` block
// appears. It's a client-side presentation layer, so it never affects the block model or
// the diff. (Syntax highlighting is server-side; the deck engine and KaTeX are bundled
// offline.)
//
// OFFLINE: a static Build with a diagram INLINES the vendored library (`MERMAID_MIN_JS`,
// ~2.5 MB, content-gated to pages that actually have a `pre.mermaid`), so a `--out` doc /
// book renders diagrams with zero network. The live Preview instead keeps the lazy loader
// pointed at the CDN default below (inlining 2.5 MB on every save would bloat the payload,
// and dev-time network is fine); `TALIESIN_MERMAID_URL` overrides that Preview/loader URL
// (e.g. to a self-hosted copy) and is also the loader's never-reached Build fallback if the
// inlined global somehow isn't present. Either way a load failure is *visible* (a
// `[data-mermaid-error]` banner), never a silent blank.
const MERMAID_DEFAULT: &str = "https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.min.js";

/// The URL the lazy mermaid loader fetches the diagram library from: the
/// `TALIESIN_MERMAID_URL` override when set (and non-empty), else the pinned CDN default.
fn mermaid_url() -> String {
    match std::env::var("TALIESIN_MERMAID_URL") {
        Ok(u) if !u.trim().is_empty() => u,
        _ => MERMAID_DEFAULT.to_string(),
    }
}

/// The client enhancers: the `window.taliEnhancers` registry + built-ins (copy
/// buttons, lightbox, link-preview, category-filter) in code-enhance.js, then the
/// self-registering mermaid module (which lazy-loads the mermaid library on first
/// use). Emitted after the registry so it is defined when mermaid registers.
/// Syntax highlighting arrives already done from the server. Callers invoke
/// `window.taliEnhanceCode(root)` after (re)mounting; it is idempotent.
pub fn code_scripts() -> String {
    code_scripts_for("", OutputMode::Preview)
}

/// The client enhancer scripts, content-gated by [`OutputMode`]. `code-enhance.js`
/// (copy buttons / lightbox / link-preview + the whole reader menu + skip-link and
/// keyboard a11y) rides on every non-bare page, since every page benefits. The
/// DOM-specific enhancers (mermaid, `{js}`, walkthrough, tabset, scrolly) ship
/// unconditionally in [`OutputMode::Preview`] (a doc can gain any construct on an
/// edit, same reasoning as the always-on KaTeX/d3 in preview) but only when their
/// target DOM is present in a static [`OutputMode::Build`]. [`OutputMode::Bare`]
/// ships nothing (the zero-`<script>` contract).
pub fn code_scripts_for(body: &str, mode: OutputMode) -> String {
    if mode == OutputMode::Bare {
        return String::new();
    }
    let mermaid_present = body.contains("class=\"mermaid\"");
    // A static Build inlines the vendored mermaid library (it sets `globalThis.mermaid`,
    // which the loader below short-circuits on) so a diagram renders FULLY OFFLINE — no
    // CDN, no external request. Preview keeps just the lean lazy loader (dev-time network
    // is fine, and inlining 2.5 MB on every save would bloat the payload). The loader's
    // `{{MERMAID}}` CDN URL stays as a never-reached fallback (window.mermaid is already
    // set), so a stripped/edited inline still degrades gracefully rather than blank.
    let mermaid_lib = if mode == OutputMode::Build && mermaid_present {
        format!("\n<script>{MERMAID_MIN_JS}</script>")
    } else {
        String::new()
    };
    let mermaid = format!(
        "{mermaid_lib}\n<script>{}</script>",
        MERMAID_JS.replace("{{MERMAID}}", &mermaid_url())
    );
    // In Preview every gate is open; in Build a gate opens only when the rendered
    // body carries that enhancer's DOM marker.
    let gate = |present: bool, script: &str| -> String {
        if mode == OutputMode::Preview || present {
            format!("\n<script>{script}</script>")
        } else {
            String::new()
        }
    };
    format!(
        "<script>{CODE_ENHANCE_JS}</script>{mermaid_s}{qmdjs_s}{walk_s}{tabset_s}{scrolly_s}",
        mermaid_s = if mode == OutputMode::Preview || mermaid_present {
            mermaid.clone()
        } else {
            String::new()
        },
        qmdjs_s = gate(has_js_cells(body), TALIESIN_JS),
        walk_s = gate(body.contains("code-walkthrough"), WALKTHROUGH_JS),
        tabset_s = gate(body.contains("panel-tabset"), TABSET_JS),
        scrolly_s = gate(body.contains("tali-scrolly"), SCROLLY_JS),
    )
}

/// The canonical TOC scrollspy (highlights the section under the navbar). Shared
/// so the static build and the live preview behave identically: the static build
/// inlines this once (it auto-inits on load); the preview also ships it and calls
/// `window.taliInitTocSpy()` after each TOC rebuild. Emitted only on TOC pages.
pub const TOC_SPY_JS: &str = include_str!("../../../../web-client/toc-spy.js");

/// Mobile pull-up TOC sheet for static builds (self-inits on load). The live preview
/// drives its own copy from client.js, so this ships only in the static build path
/// (`toc_scripts`), never the preview, to avoid double-wiring the sheet.
pub const TOC_SHEET_JS: &str = include_str!("../../../../web-client/toc-sheet.js");

pub fn toc_scripts() -> String {
    format!(
        "<script>{TOC_SPY_JS}</script>\n<script>{TOC_SHEET_JS}</script>\n<script>{SEARCH_JS}</script>"
    )
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
const TALIESIN_JS: &str = include_str!("../../assets/js/qmd-js.js");

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

// `code-enhance.js` is authored as ordered per-feature fragments under
// `assets/js/code-enhance/` and concatenated (in filename order) into one inline
// `<script>`. The numeric prefix IS the load order: the registry IIFE (`01`) must
// define `window.taliEnhancers` before the registration block (`09`) runs, and every
// built-in's `function` declaration is hoisted within this single concatenated
// script, so `09` can forward-reference features defined in later fragments. The
// `code_enhance_bundle_matches_fragments_in_order` test (tests.rs) guards that this
// list stays complete + in load order (no separators — the fragments tile exactly).
const CODE_ENHANCE_JS: &str = concat!(
    include_str!("../../assets/js/code-enhance/01-registry.js"),
    include_str!("../../assets/js/code-enhance/02-anchor-links.js"),
    include_str!("../../assets/js/code-enhance/03-focus-mode.js"),
    include_str!("../../assets/js/code-enhance/04-focus-trap.js"),
    include_str!("../../assets/js/code-enhance/06-skip-link.js"),
    include_str!("../../assets/js/code-enhance/07-keyboard.js"),
    include_str!("../../assets/js/code-enhance/08-copy-buttons.js"),
    include_str!("../../assets/js/code-enhance/09-register.js"),
    include_str!("../../assets/js/code-enhance/10-category-filter.js"),
    include_str!("../../assets/js/code-enhance/11-lightbox.js"),
    include_str!("../../assets/js/code-enhance/12-link-preview.js"),
    include_str!("../../assets/js/code-enhance/13-reader-menu.js"),
    include_str!("../../assets/js/code-enhance/14-reader-prefs.js"),
    include_str!("../../assets/js/code-enhance/15-reading-progress.js"),
    include_str!("../../assets/js/code-enhance/16-scroll-a11y.js"),
    include_str!("../../assets/js/code-enhance/17-cite-box.js"),
);
/// The enhancer registry (`window.taliEnhancers` + `taliEnhanceCode`) on its own, so the
/// External build path can emit it INLINE at parse (before any `include-after-body`
/// extension script that calls `taliEnhancers.register`), while the shared app.js stays
/// deferred. This double-includes `01-registry.js` at compile time (once here, once inside
/// `CODE_ENHANCE_JS`'s `concat!` above); keep BOTH copies, since dropping it from
/// `CODE_ENHANCE_JS` would drift the Inline path's byte output. The IIFE is idempotent
/// (`if (window.taliEnhancers) return;`), so app.js's bundled copy no-ops on its later run.
const REGISTRY_JS: &str = include_str!("../../assets/js/code-enhance/01-registry.js");
const MERMAID_JS: &str = include_str!("../../assets/js/mermaid.js");
/// The vendored Mermaid library (pinned mermaid@11.4.1, ~2.5 MB; sets `globalThis.mermaid`).
/// Inlined into a static Build page that has a diagram so it renders with no CDN; the
/// live Preview keeps the lazy loader instead (see `code_scripts_for`).
const MERMAID_MIN_JS: &str = include_str!("../../assets/js/mermaid.min.js");
/// Scroll-driven line-range highlighter for `::: {.code-walkthrough}`. Registers
/// through `taliEnhancers`, no-ops without a walkthrough (like mermaid/qmd-js), so it
/// rides unconditionally in [`code_scripts`].
const WALKTHROUGH_JS: &str = include_str!("../../assets/js/walkthrough.js");
/// ARIA tabs interaction for `::: {.panel-tabset}` (click + arrow-key tab switching).
/// Registers through `taliEnhancers`, no-ops without a tabset, rides in [`code_scripts`].
const TABSET_JS: &str = include_str!("../../assets/js/tabset.js");
/// Scroll-driven sticky-stage scenes for `::: {.scrolly}`. Registers through `taliEnhancers`,
/// no-ops without a `.scrolly`, rides in [`code_scripts`].
const SCROLLY_JS: &str = include_str!("../../assets/js/scrolly.js");

/// The raw framework CSS a non-bare site page inlines in its main `<style>` (fonts +
/// tokens + base + dark + site chrome). Exposed so the multi-page build can externalize it
/// into one content-hashed `_assets/app.<hash>.css` instead of inlining a copy per page.
pub fn shared_site_css() -> String {
    format!("{FONTS_CSS}{TOKENS_CSS}{TOKENS_DARK_CSS}{BASE_CSS}{DARK_CSS}{SITE_CSS}")
}

/// The KaTeX stylesheet (base64 fonts inlined), for the externalized `katex.<hash>.css`.
pub fn katex_css() -> &'static str {
    KATEX_CSS
}

/// All of Taliesin's OWN page JS, concatenated for the always-on `app.<hash>.js`. Each
/// piece is separated by a bare `;` on its own line so concatenation is ASI-safe. The
/// big vendored libs (mermaid, d3, Plot) are deliberately excluded (their own files), and
/// so is the `{js}`-cell runtime (`TALIESIN_JS`): it runs each cell via `new
/// AsyncFunction(..., src)`, whose dynamic `import()` resolves the specifier relative to
/// the SCRIPT that called the constructor. Folded into the shared `/_assets/app.js`, a
/// cell's `import("./helper.js")` would wrongly resolve against `/_assets/` (a 404), so the
/// External page keeps that runtime INLINE instead (see `assemble_html_page`), anchoring
/// the resolution base to the page itself.
pub fn core_enhance_js() -> String {
    [
        CODE_ENHANCE_JS,
        WALKTHROUGH_JS,
        TABSET_JS,
        SCROLLY_JS,
        TOC_SPY_JS,
        TOC_SHEET_JS,
        SEARCH_JS,
    ]
    .join("\n;\n")
}

/// The vendored mermaid library plus its loader (CDN placeholder already resolved), for
/// the conditional `mermaid.<hash>.js`. Ships only on pages that have a diagram, so the
/// loader's never-reached CDN fallback stays off prose pages.
pub fn mermaid_bundle_js() -> String {
    format!(
        "{MERMAID_MIN_JS}\n;\n{}",
        MERMAID_JS.replace("{{MERMAID}}", &mermaid_url())
    )
}

/// The vendored d3 + Observable Plot globals for the conditional `jslibs.<hash>.js`
/// (ships only on pages with `{js}` cells).
pub fn js_cell_libs_js() -> String {
    format!("{D3_JS}\n;\n{PLOT_JS}")
}

/// True if a rendered body contains a mermaid diagram (gates the mermaid file link).
pub fn has_mermaid(body: &str) -> bool {
    body.contains("class=\"mermaid\"")
}

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

/// Remove `$…$` / `$$…$$` math spans from a heading's markdown text before slugging, so
/// KaTeX/LaTeX (`$H_0$`) doesn't leak into the anchor id (`…-h-0`). Mirrors comrak's
/// `math_dollars` rule closely enough for a slug: an opening `$` is not followed by
/// whitespace and its matching closing `$` is not preceded by whitespace, `\$` is a
/// literal dollar (not a delimiter), and a span never crosses a newline. So a real math
/// span drops out while a lone/currency `$` (e.g. `$5 and $10`, which comrak also leaves
/// as text) stays put.
fn strip_math_for_slug(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        // A backslash escape (`\$`, `\\`, …) is literal: copy the pair, never a delimiter.
        if chars[i] == '\\' {
            out.push(chars[i]);
            if let Some(&next) = chars.get(i + 1) {
                out.push(next);
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if chars[i] == '$' {
            let display = chars.get(i + 1) == Some(&'$');
            let open_len = if display { 2 } else { 1 };
            let body = i + open_len;
            // A span needs a body; inline `$…$` also forbids whitespace right after the
            // opening `$` (display `$$…$$` allows it, matching comrak). If a valid close
            // exists, drop the whole span; otherwise the `$` is literal (fall through).
            let open_ok = body < chars.len() && (display || !chars[body].is_whitespace());
            if open_ok && let Some(close) = math_close(&chars, body, display) {
                i = close + open_len; // skip delimiters + body
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Index of the matching closing `$` (inline) or first `$` of the closing `$$` (display)
/// for a math span whose body starts at `start`; `None` if the opening `$` isn't a math
/// delimiter after all. Mirrors comrak's inline scan: the close is the FIRST unescaped
/// `$`, and if that `$` is preceded by whitespace or followed by an ASCII digit the span
/// is abandoned (the opening `$` stays literal) rather than reaching for a later `$` —
/// else a lone/currency `$` would greedily swallow the text up to some later real span.
fn math_close(chars: &[char], start: usize, display: bool) -> Option<usize> {
    let mut j = start;
    while j < chars.len() {
        match chars[j] {
            '\n' => return None,
            // `\` escapes the next char (`\$` is a literal dollar, never a delimiter).
            '\\' => {
                j += 2;
                continue;
            }
            // Display close: the first `$$` (comrak is lenient about adjacency here).
            '$' if display => {
                if chars.get(j + 1) == Some(&'$') {
                    return Some(j);
                }
            }
            // Inline close: THIS first unescaped `$` is the only candidate. `j > start`
            // (the caller guarantees `chars[start]` is neither whitespace nor `$`), so
            // `j - 1` is in range.
            '$' => {
                let ok = !chars[j - 1].is_whitespace()
                    && !chars.get(j + 1).is_some_and(|c| c.is_ascii_digit());
                return ok.then_some(j);
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// A deduped heading anchor slug; a repeated slug gets a `-N` suffix.
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
    // An explicit `{#id}` on a slide heading becomes the `<section>` anchor (the slide
    // model reads `data-slide-anchor`), so `@sec-x` into a deck resolves instead of the
    // text-slug id winning and leaving a dead link.
    if let Some(id) = &attrs.id {
        out.push_str(&format!(" data-slide-anchor=\"{}\"", escape_attr(id)));
    }
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

/// Apply a Pandoc attribute block trailing a link — `<a ...>text</a>{.btn #id}`
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
/// `\` hard break that comrak keeps (CommonMark) but Pandoc drops (e.g. a
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
        // (`text\</p>`): CommonMark keeps it literal, Pandoc drops it. Anchored
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
/// A float's displayed number: chapter-scoped ("2.3") inside a numbered book chapter,
/// else the flat count ("3"). Figures/tables/equations/listings each keep their own
/// counter and scope it to the chapter, so two chapters no longer both open with a
/// "Figure 1" and a cross-chapter `@fig-` ref is unambiguous.
///
/// There is no knob: outside a numbered chapter there is simply no chapter to scope to,
/// so numbering stays flat — the same rule `section_number` already follows. Theorems
/// call this too, so a chapter cannot show "Figure 2.3" beside "Theorem 5".
fn float_number(chapter: Option<u32>, n: usize) -> String {
    match chapter {
        Some(ch) => format!("{ch}.{n}"),
        None => n.to_string(),
    }
}

/// Register a cross-reference anchor → number, keeping the **first** definition and
/// warning on a duplicate label. Otherwise a repeated `{#fig-x}`/`{#sec-x}` silently
/// took the *last* number while the `#fig-x` anchor pointed at the *first* element —
/// so `@fig-x` and the link target disagreed, with no diagnostic.
fn register_xref(
    reg: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
    anchor: &str,
    number: String,
    file: Option<&str>,
    line: u32,
) {
    if reg.contains_key(anchor) {
        // Locate the DUPLICATE (this second definition), like the "duplicate heading id"
        // warning beside it — an unlocated duplicate-label warning half-reproduces the
        // Quarto flaw D53 critiques.
        warnings.push(
            Warning::new(format!(
                "duplicate cross-reference label \u{201c}{anchor}\u{201d} (using the first definition)"
            ))
            .at(file.map(str::to_string), line),
        );
    } else {
        reg.insert(anchor.to_string(), number);
    }
}

/// The 1-based start line of a `L:C-L:C` sourcepos, or 0 when it carries none (a generated
/// block with an empty sourcepos — not click-to-source anyway, and `locatable()` requires
/// a `[1-9]` line, so 0 reads as "no location").
fn sourcepos_start_line(sp: &str) -> u32 {
    sp.split(':')
        .next()
        .and_then(|l| l.parse().ok())
        .unwrap_or(0)
}

/// A labelled cell whose output the executor will never emit (`#| include: false`) has
/// nothing to carry its anchor, so the label is unreachable. Warn at the cell, mirroring
/// the theorem-prefix warning: the reference site's own "broken cross-reference: @fig-x"
/// reads as a lie to an author looking straight at the `label: fig-x` they wrote, and an
/// unreferenced one would otherwise die silently. `kind` names the construct ("figure",
/// "table") so the message says which label it means.
fn unreferenceable_hidden_label(
    kind: &str,
    anchor: &str,
    file: Option<String>,
    line: usize,
) -> Warning {
    Warning::new(format!(
        "{kind} label \u{201c}{anchor}\u{201d} cannot be cross-referenced: `include: false` drops \
         the cell's output, so nothing carries the anchor and `@{anchor}` won't resolve"
    ))
    .at(file, line as u32)
}

/// A labelled figure/table cell whose language Taliesin never runs (`{bash}`, `{sql}`,
/// `{julia}`, …) can't produce the output the label points at. Unlike the `include:
/// false` case, the source still shows; the label just cannot resolve, so say why (a
/// listing label `lst-*` would work, since a listing IS the source).
fn unreferenceable_nonexec_label(
    kind: &str,
    anchor: &str,
    lang: &str,
    file: Option<String>,
    line: usize,
) -> Warning {
    Warning::new(format!(
        "{kind} label \u{201c}{anchor}\u{201d} cannot be cross-referenced: `{{{lang}}}` is not \
         executed, so it produces no {kind} output and `@{anchor}` won't resolve"
    ))
    .at(file, line as u32)
}

/// Assign continuous, per-kind theorem numbers in document order (Theorem 1, 2, …;
/// Lemma 1, 2, … independently), fill each theorem's number slot, and register its
/// `#thm-`/`#lem-`/… anchor so `@thm-x` resolves. Runs after `apply_table_captions`
/// and before `cite::process`. `proof` carries no `data-qmd-theorem-kind`, so it is
/// skipped (unnumbered, unreferenceable). Top-level theorems only — a theorem nested
/// inside another container is embedded in the parent block's HTML (same limitation as
/// table captions). The container id is read from the OPENING tag only (via `tag_end`)
/// so a child block's `id=` is never mistaken for the theorem anchor.
/// Every theorem div inside `html`, in document order, as `(kind, id)`. Scans the
/// whole string (not just the opening tag) so a `::: {.theorem}` nested inside another
/// fenced div — which collapses into the parent's one block — is still found, numbered,
/// and registered as a ref target. Each `data-qmd-theorem-kind` occurrence is paired
/// with the `id` on its own opening `<div>`, bounded to that tag so a sibling div's id
/// can't leak in.
fn theorem_divs(html: &str) -> Vec<(String, Option<String>)> {
    const NEEDLE: &str = "data-qmd-theorem-kind=\"";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = html[from..].find(NEEDLE) {
        let attr_pos = from + rel;
        let kind_start = attr_pos + NEEDLE.len();
        let Some(kind_end) = html[kind_start..].find('"').map(|i| kind_start + i) else {
            break;
        };
        let kind = html[kind_start..kind_end].to_string();
        // The theorem's own opening tag is the nearest `<div` before its kind attr.
        let div_start = html[..attr_pos].rfind("<div").unwrap_or(attr_pos);
        let open_tag = tag_end(&html[div_start..])
            .map(|i| &html[div_start..div_start + i + 1])
            .unwrap_or(&html[div_start..]);
        out.push((kind, extract_attr(open_tag, "id")));
        from = kind_end;
    }
    out
}

fn number_theorems(
    blocks: &mut [Block],
    xrefs: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
    config: &TheoremConfig,
    chapter: Option<u32>,
) {
    let mut counts: HashMap<String, u32> = HashMap::new();
    // For `numbered: unless-unique`, a kind is numbered only if it occurs more than once;
    // pre-count occurrences per counter-key.
    let mut totals: HashMap<String, u32> = HashMap::new();
    if config.numbered() == Numbered::UnlessUnique {
        for b in blocks.iter() {
            for (kind, _) in theorem_divs(&b.html) {
                *totals
                    .entry(config.counter_key(&kind).to_string())
                    .or_insert(0) += 1;
            }
        }
    }
    for b in blocks.iter_mut() {
        // Collect every theorem in this block up front; the number slots are then filled
        // left-to-right in the same order, so nested theorems interleave by document order.
        for (kind, id) in theorem_divs(&b.html) {
            // Shared-group kinds collapse to one counter key; the visible label stays
            // per-kind (only the number is shared).
            let key = config.counter_key(&kind).to_string();
            let n = {
                let c = counts.entry(key.clone()).or_insert(0);
                *c += 1;
                *c
            };
            // Whether to show a number: `numbered: false` never; `unless-unique` only when the
            // kind occurs more than once; otherwise yes.
            let show_number = match config.numbered() {
                Numbered::Yes => true,
                Numbered::No => false,
                Numbered::UnlessUnique => totals.get(&key).copied().unwrap_or(0) > 1,
            };
            // A numbered book chapter scopes the number ("Theorem 2.3"); anywhere else it
            // is flat ("Theorem 3"). Same rule, same helper, as every float — so a chapter
            // cannot show "Figure 2.3" beside "Theorem 5". There is no opt-in: scoping is
            // a property of being in a numbered chapter, which the renderer already knows.
            let display = if !show_number {
                String::new()
            } else {
                float_number(chapter, n as usize)
            };
            // An unnumbered theorem leaves the slot empty (no &nbsp;) and is not a ref target.
            let slot = if display.is_empty() {
                String::new()
            } else {
                format!("&nbsp;{display}")
            };
            b.html = b.html.replacen(
                "<span class=\"tali-theorem-number\"></span>",
                &format!("<span class=\"tali-theorem-number\">{slot}</span>"),
                1,
            );
            // Register the anchor even when unnumbered (`display` empty): an id'd theorem is a
            // valid same-page ref target that resolves to a bare label, not a broken ref.
            // But only if the id carries a cross-reference kind prefix. A theorem gets its
            // number from the `.theorem` class alone, so `#pythagoras` is numbered yet
            // `@pythagoras` never resolves (`parse_xref` bails when the prefix names no xref
            // kind) — the div's own id path, unlike figures/tables, never gated on the prefix,
            // so this was silently unreferenceable and `check` said nothing. Warn instead, and
            // suggest the kind's prefix (theorem -> `thm-`, lemma -> `lem-`, …).
            if let Some(id) = id {
                if crate::cite::is_xref_anchor(&id) {
                    register_xref(
                        xrefs,
                        warnings,
                        &id,
                        display,
                        b.source_file.as_deref(),
                        sourcepos_start_line(&b.sourcepos),
                    );
                } else {
                    let hint = crate::cite::xref_prefix_for_label(divs::theorem_meta(&kind).0)
                        .map(|p| format!("; use `{p}-{id}`"))
                        .unwrap_or_default();
                    warnings.push(Warning::new(format!(
                        "theorem id \u{201c}{id}\u{201d} cannot be cross-referenced (`@{id}` won't resolve){hint}"
                    )));
                }
            }
        }
    }
}

fn apply_table_captions(
    blocks: &mut Vec<Block>,
    xrefs: &mut HashMap<String, String>,
    warnings: &mut Vec<Warning>,
    chapter: Option<u32>,
) {
    let mut tbl_count = 0usize;
    let mut i = 0;
    while i < blocks.len() {
        // The current block's location, captured before any mutable borrow of it, so a
        // duplicate-label warning can point at it (click-to-source).
        let bfile = blocks[i].source_file.clone();
        let bline = sourcepos_start_line(&blocks[i].sourcepos);
        // A code cell whose executed output is a numbered table (`#| label: tbl-x`):
        // assign its number in document order (so it interleaves correctly with
        // Markdown tables) and register the xref. The executor injects the matching
        // caption/id into the output using `cell.table.number`.
        if let Some(t) = blocks[i].cell.as_mut().and_then(|c| c.table.as_mut()) {
            tbl_count += 1;
            let num = float_number(chapter, tbl_count);
            t.number = num.clone();
            if let Some(a) = &t.anchor {
                register_xref(xrefs, warnings, a, num, bfile.as_deref(), bline);
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
            let tbl_num = float_number(chapter, tbl_count);
            if let Some(id) = &id {
                // The `{#tbl-x}` label is authored on the caption paragraph (blocks[i+1]),
                // so locate the warning there.
                register_xref(
                    xrefs,
                    warnings,
                    id,
                    tbl_num.clone(),
                    blocks[i + 1].source_file.as_deref(),
                    sourcepos_start_line(&blocks[i + 1].sourcepos),
                );
            }
            let sep = if caption_html.is_empty() { "" } else { ": " };
            let id_attr = id_attr(id.as_deref());
            // Insert the `id` on the <table> and a <caption> as its first child.
            let table = &blocks[i].html;
            let gt = table.find('>').unwrap_or(0) + 1;
            let open = table[..gt].replacen("<table", &format!("<table{id_attr}"), 1);
            blocks[i].html = format!(
                "{open}<caption>Table&nbsp;{tbl_num}{sep}{caption_html}</caption>{}",
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
/// The headings the on-page TOC shows: those carrying an anchor `id`, within two levels
/// of the shallowest heading present (`level - base <= 2`). `toc_entry_count` and
/// `toc_html` share this so their filters cannot drift, and so a title-demoted page
/// (whose sections start at `<h2>`) still surfaces three levels instead of two.
fn toc_items(blocks: &[Block]) -> Vec<(u8, String, String)> {
    let all: Vec<(u8, String, String)> = blocks
        .iter()
        .filter_map(|b| {
            Some((
                block_heading_level(&b.html)?,
                extract_attr(&b.html, "id")?,
                strip_tags(&b.html),
            ))
        })
        .collect();
    let Some(base) = all.iter().map(|(l, _, _)| *l).min() else {
        return Vec::new();
    };
    // `base` is the minimum, so `*l >= base` always: the subtraction never underflows.
    all.into_iter().filter(|(l, _, _)| *l - base <= 2).collect()
}

/// How many entries the table of contents would list (exactly the set [`toc_html`]
/// renders). The site auto-gates the "on this page" TOC on this count — a short /
/// sequential page reads as one column; only long, chunkable pages earn the sidebar
/// TOC (NN/g).
pub(crate) fn toc_entry_count(blocks: &[Block]) -> usize {
    toc_items(blocks).len()
}
fn toc_html(blocks: &[Block]) -> String {
    let items = toc_items(blocks);
    if items.is_empty() {
        return String::new();
    }
    let base = items.iter().map(|(l, _, _)| *l).min().unwrap();
    let mut out = String::from("<nav id=\"TOC\" class=\"tali-toc\" role=\"doc-toc\"><ul>");
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
            // `text` is `strip_tags` output — already HTML-safe (entities intact from
            // the rendered heading), so it must NOT be `html_escape`'d again (that
            // turned `&amp;` into `&amp;amp;`). See `toc_html`'s `strip_tags` call above.
            "<li><a href=\"#{}\">{text}</a>",
            escape_attr(id),
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
    // Block ids and the freeze cache key share one hashing scheme; see `crate::hash`.
    let hex = format!("{:016x}", crate::hash::fnv1a(block_src.trim()));
    let base = format!("b-{}", &hex[..12]);
    dedup_with_suffix(base, counts)
}

/// Demote a heading block's visible tag one level (`<hN>` -> `<h{N+1}>`, clamped at
/// `<h6>`), leaving its attributes, `id`, `data-block-id`, `data-sourcepos` and text
/// untouched. Used when a page renders a title-block `<h1 class="title">` so its body
/// sections nest beneath the single page title: one `<h1>` per page (a11y + SEO).
fn demote_heading_html(html: &str, level: u8) -> String {
    let to = (level + 1).min(6);
    if to == level {
        return html.to_string();
    }
    // `html` is `<hN...>...</hN>`: rewrite only the opening tag name (at index 0) and the
    // lone closing tag. Heading text has its `<`/`>` escaped to entities, so the literal
    // `</hN>` appears exactly once (the real closing tag).
    html.replacen(&format!("<h{level}"), &format!("<h{to}"), 1)
        .replacen(&format!("</h{level}>"), &format!("</h{to}>"), 1)
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
    /// The `#id` parsed from the attribute block, if any. `pub(crate)` so the
    /// cross-page xref scanner (`site::xref`) reuses this one quote-aware parser
    /// instead of re-implementing brace scanning and drifting from the renderer.
    pub(crate) fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn get(&self, key: &str) -> Option<&str> {
        self.kv
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
    fn callout_kind(&self) -> Option<&str> {
        self.classes.iter().find_map(|c| c.strip_prefix("callout-"))
    }
    /// The first class that names a theorem-environment kind, or `None`.
    fn theorem_kind(&self) -> Option<&str> {
        self.classes
            .iter()
            .map(String::as_str)
            .find(|c| validate::THEOREM_KINDS.contains(c))
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

/// Strip HTML tags, returning the visible text (callout/tabset titles, TOC entries,
/// figure alt-text, deck slugs). Quote-aware, like [`tag_end`]: a `>` inside a quoted
/// attribute value (e.g. KaTeX's `<span title="a>b">`) does NOT end the tag, so the
/// visible text isn't truncated mid-attribute.
///
/// KaTeX-aware: with the default `htmlAndMathml` output, KaTeX renders inline math
/// three times — the MathML semantic text, a raw-TeX `<annotation>`, and the visible
/// `katex-html` glyphs. Emitting all three triples a heading's TOC label / slug and
/// leaks LaTeX (`$H_0$` → `H0H_0H0`). So the whole `<math>…</math>` subtree is dropped,
/// leaving only the visible `katex-html` glyphs (`H0`).
fn strip_tags(html: &str) -> String {
    strip_tags_inner(html, false)
}

/// [`strip_tags`], but with a space at every tag boundary, so text from *adjacent
/// blocks* stays word-separated when a run of block HTML is read as one string (the
/// search index's case: `<p>First.</p><p>Second.</p>` must not fuse into
/// "First.Second."). The TOC/slug path must NOT do this — a space there would split
/// `<em>Fig</em>ure` into two words and change every slug.
fn strip_tags_separated(html: &str) -> String {
    strip_tags_inner(html, true)
}

fn strip_tags_inner(html: &str, separate: bool) -> String {
    let mut out = String::new();
    let mut skip_math = 0usize; // depth of `<math>` subtrees whose text is dropped
    let mut chars = html.chars();
    while let Some(ch) = chars.next() {
        if ch == '<' {
            if separate {
                out.push(' ');
            }
            // Consume the tag body up to the closing `>` (quote-aware: a `>` inside a
            // quoted attribute value does not end the tag).
            let mut tag = String::new();
            let mut quote: Option<char> = None;
            for c in chars.by_ref() {
                match quote {
                    Some(q) => {
                        if c == q {
                            quote = None;
                        }
                        tag.push(c);
                    }
                    None => match c {
                        '"' | '\'' => {
                            quote = Some(c);
                            tag.push(c);
                        }
                        '>' => break,
                        _ => tag.push(c),
                    },
                }
            }
            // Enter/exit the KaTeX `<math>` MathML subtree (depth-tracked for safety).
            let body = tag.trim_start();
            let is_close = body.starts_with('/');
            let name: String = body
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect();
            if name == "math" {
                if is_close {
                    skip_math = skip_math.saturating_sub(1);
                } else if !tag.trim_end().ends_with('/') {
                    skip_math += 1;
                }
            }
        } else if skip_math == 0 {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

/// The plain text of a document's leading `# H1`, when that H1 is the document's *first*
/// block. A later or deeper heading is a section, not the document's name, so only the
/// first block qualifies.
///
/// The returned text is decoded, not HTML: `<title>` escapes its input, so a heading
/// carrying `&amp;` would otherwise ship as `&amp;amp;`.
fn leading_h1_text(blocks: &[Block]) -> Option<String> {
    let first = blocks.first()?;
    (block_heading_level(&first.html)? == 1)
        .then(|| unescape_html(&strip_tags(&first.html)))
        .filter(|t| !t.is_empty())
}

/// Reverse [`escape_html`]: decode the entities the renderer itself emits (`&amp;`,
/// `&lt;`, `&gt;`, `&quot;`, `&#39;`). Not a general HTML entity decoder — it exists so
/// text lifted back out of emitted HTML can be re-escaped exactly once.
fn unescape_html(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let decoded = [
            ("&amp;", '&'),
            ("&lt;", '<'),
            ("&gt;", '>'),
            ("&quot;", '"'),
            ("&#39;", '\''),
        ]
        .into_iter()
        .find(|(ent, _)| tail.starts_with(ent));
        match decoded {
            Some((ent, ch)) => {
                out.push(ch);
                rest = &tail[ent.len()..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
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

/// A labelled display equation: `$$ ... $$ {#eq-x}`. Returns the LaTeX
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
fn emit_equation(latex: &str, anchor: &str, block_attrs: &str, num: &str) -> String {
    format!(
        "<div id=\"{anchor}\"{block_attrs} class=\"tali-eqn\">\
         <span class=\"tali-eqn-body\">{}</span>\
         <span class=\"tali-eqn-number\">({num})</span></div>",
        crate::math::render(latex, true)
    )
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
/// [`html_page_from_doc_in_site`]); all of it is driven by `--tali-*` vars so a
/// theme extension restyles it for free. Deliberately leaner than a full
/// Bootstrap chrome (no banner, no search bar, no feed).
const SITE_CSS: &str = include_str!("../../assets/css/site.css");

// The deck engine (deck.css/deck.js) is bundled into the page like KaTeX —
// decks render with no network, the same as every other format.

#[cfg(test)]
mod tests;
