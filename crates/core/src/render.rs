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
    /// Whether the front matter requested a table of contents (`toc: true`).
    pub toc: bool,
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
    let mut toc = false;
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
                toc = detect_toc(fm);
                theme = detect_theme(fm);
                includes = resolve_doc_includes(fm, base_dir);
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
        toc,
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

/// A theme is an extension that ships CSS. Two minimal themes are built in
/// (`light` is the default `:root`, `dark` overrides it); any other name
/// resolves to a `.css`/`.scss` file or an installed `_extensions/<name>/`
/// bundle, both relative to the document. Returns the override CSS to inline
/// after the base stylesheet (empty for the default light theme).
fn resolve_theme(theme: Option<&str>, base_dir: Option<&Path>) -> String {
    let Some(name) = theme else {
        return String::new();
    };
    match name {
        // Built-in light/dark are always shipped (DARK_CSS) and selected at
        // runtime via `data-theme` (toggle / OS), so no per-page override CSS.
        "light" | "default" | "dark" => String::new(),
        path if path.ends_with(".css") || path.ends_with(".scss") => base_dir
            .and_then(|b| std::fs::read_to_string(b.join(path)).ok())
            .unwrap_or_default(),
        // An installed extension bundle: `_extensions/<name>/theme.css`.
        ext => base_dir
            .and_then(|b| {
                std::fs::read_to_string(b.join("_extensions").join(ext).join("theme.css")).ok()
            })
            .unwrap_or_default(),
    }
}

/// The default theme mode for the resolver script: an explicit `dark`/`light`
/// from front matter forces that mode; otherwise `auto` follows the OS
/// (`prefers-color-scheme`). Custom CSS themes don't force a built-in mode.
fn theme_default_mode(theme: Option<&str>) -> &'static str {
    match theme {
        Some("dark") => "dark",
        Some("light") | Some("default") => "light",
        _ => "auto",
    }
}

/// Inline `<head>` script (runs before paint, so no flash): set
/// `<html data-theme>` from the saved choice, else the front-matter default,
/// else the OS `prefers-color-scheme`. Also defines `qmdSetTheme`/`qmdGetThemePref`
/// for the preview toggle and keeps `auto` in sync with OS changes.
pub fn theme_head(default_mode: &str) -> String {
    format!(
        r#"<script>
(function(){{
  var DEFAULT = "{default_mode}";
  var mq = window.matchMedia && matchMedia("(prefers-color-scheme: dark)");
  function pref(){{ try {{ return localStorage.getItem("qmd-theme") || DEFAULT; }} catch(e) {{ return DEFAULT; }} }}
  function apply(){{
    var p = pref();
    var mode = p === "auto" ? ((mq && mq.matches) ? "dark" : "light") : p;
    document.documentElement.setAttribute("data-theme", mode);
    // Let theme-dependent renderers (e.g. mermaid, whose SVG colours are baked at
    // render time) re-render. Fires on toggle and on OS change while in `auto`.
    try {{ window.dispatchEvent(new CustomEvent("qmd:themechange", {{ detail: {{ mode: mode }} }})); }} catch(e) {{}}
  }}
  apply();
  if (mq && mq.addEventListener) mq.addEventListener("change", function(){{ if (pref() === "auto") apply(); }});
  window.qmdSetTheme = function(p){{ try {{ localStorage.setItem("qmd-theme", p); }} catch(e) {{}} apply(); }};
  window.qmdGetThemePref = function(){{ return pref(); }};
  // Wire any `[data-qmd-theme-toggle]` button (the site navbar's, or the dev
  // menu's on a single doc): cycle auto -> light -> dark, icon reflects state.
  // Shipped here (not in the preview client) so the toggle works in `build` too.
  var ICONS = {{ auto: "{auto_icon}", light: "{sun_icon}", dark: "{moon_icon}" }};
  var ORDER = ["auto", "light", "dark"];
  window.qmdWireThemeToggles = function(){{
    var btns = document.querySelectorAll("[data-qmd-theme-toggle]");
    for (var i = 0; i < btns.length; i++) {{
      (function(btn){{
        if (btn.getAttribute("data-wired")) return;
        btn.setAttribute("data-wired", "1");
        function sync(){{ var p = pref(); btn.innerHTML = ICONS[p] || ICONS.auto;
          btn.setAttribute("aria-label", "Theme: " + p + " (click to cycle light / dark / auto)"); }}
        btn.addEventListener("click", function(){{ window.qmdSetTheme(ORDER[(ORDER.indexOf(pref()) + 1) % 3]); sync(); }});
        window.addEventListener("qmd:themechange", sync);
        sync();
      }})(btns[i]);
    }}
  }};
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", window.qmdWireThemeToggles);
  else window.qmdWireThemeToggles();
}})();
</script>"#,
        auto_icon = THEME_ICON_AUTO,
        sun_icon = THEME_ICON_SUN,
        moon_icon = THEME_ICON_MOON,
    )
}

// Monochrome theme-toggle icons (single-quoted attrs so they embed in JS double
// quotes; `currentColor` so they inherit the control's colour).
const THEME_ICON_SUN: &str = "<svg width='15' height='15' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round'><circle cx='12' cy='12' r='4'/><path d='M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4'/></svg>";
const THEME_ICON_MOON: &str = "<svg width='15' height='15' viewBox='0 0 24 24' fill='currentColor'><path d='M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z'/></svg>";
const THEME_ICON_AUTO: &str = "<svg width='15' height='15' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2'><circle cx='12' cy='12' r='9'/><path d='M12 3a9 9 0 0 0 0 18z' fill='currentColor' stroke='none'/></svg>";

/// Detect the `theme:` front-matter value (top-level or nested under `format:`).
fn detect_theme(front_matter: &str) -> Option<String> {
    front_matter.lines().find_map(|line| {
        let v = line.trim().strip_prefix("theme:")?.trim();
        // Take the first name from a scalar or a `[a, b]` list (Quarto allows a
        // list; the first entry is the base theme, the rest are SCSS layers).
        let v = v.trim_start_matches('[').split([',', ']']).next()?.trim();
        let v = v.trim_matches(['"', '\'']).trim();
        (!v.is_empty()).then(|| v.to_string())
    })
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
const DARK_CSS: &str = r#"
  html[data-theme="dark"] {
    --qmd-bg: #16181d; --qmd-fg: #e6e6e6; --qmd-muted: #9aa0aa; --qmd-accent: #6ea8ff;
    --qmd-link: #6ea8ff; --qmd-code-bg: #21242b; --qmd-border: #363a44;
    --qmd-edge-shadow: rgba(255, 255, 255, .14); --qmd-flash: rgba(110, 168, 255, .22);
    color-scheme: dark; /* native form controls (OJS inputs) + scrollbars render dark */
  }
  html[data-theme="dark"] .qmd-copy { background: #21242b; color: #c8ccd4; border-color: #3a3f4b; }
  html[data-theme="dark"] .qmd-copy:hover { color: #fff; border-color: #5a606b; }
  html[data-theme="dark"] .callout-note .callout-title { background: #1b2330; }
  html[data-theme="dark"] .callout-tip .callout-title { background: #15241c; }
  html[data-theme="dark"] .callout-warning .callout-title { background: #2a2415; }
  html[data-theme="dark"] .callout-important .callout-title { background: #2a1820; }
  html[data-theme="dark"] .callout-caution .callout-title { background: #2a2015; }
  html[data-theme="dark"] pre.mermaid { background: transparent; }
  /* server-side syntax-highlight scope classes (syntect), recoloured for dark */
  html[data-theme="dark"] .qhl-comment { color: #8b949e; }
  html[data-theme="dark"] .qhl-string { color: #a5d6ff; }
  html[data-theme="dark"] .qhl-keyword, html[data-theme="dark"] .qhl-storage { color: #ff7b72; }
  html[data-theme="dark"] .qhl-constant, html[data-theme="dark"] .qhl-support { color: #79c0ff; }
  html[data-theme="dark"] .qhl-entity { color: #d2a8ff; }
  html[data-theme="dark"] .qhl-variable { color: #ffa657; }
  /* code-cell output, warnings, and errors: dark equivalents of the light boxes */
  html[data-theme="dark"] .qmd-output > pre { background: #1b1e24; border-left-color: var(--qmd-border); }
  html[data-theme="dark"] .qmd-stderr { border-left-color: #d0a215 !important; background: #2a2415 !important; color: #e8dcc0; }
  html[data-theme="dark"] .qmd-error { border-left-color: #e0566b !important; background: #2a1820 !important; color: #f2b8c2; }
"#;

/// `toc: true` requested anywhere in the front matter (typically under
/// `format: html:`). A lightweight scan, matching the corpus book's usage.
fn detect_toc(front_matter: &str) -> bool {
    front_matter.lines().any(|l| {
        let t = l.trim();
        t.strip_prefix("toc:").map(str::trim) == Some("true")
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
const BASE_CSS: &str = r#"
  :root {
    --qmd-bg: #ffffff; --qmd-fg: #1a1a1a; --qmd-muted: #555; --qmd-accent: #4c8dff;
    --qmd-link: #2563eb; --qmd-code-bg: #f5f5f5; --qmd-border: #e3e3e3;
    --qmd-edge-shadow: rgba(0, 0, 0, .16); --qmd-flash: rgba(76, 141, 255, .18);
    color-scheme: light;
    --qmd-font-body: 17px/1.7 ui-serif, Georgia, "Times New Roman", serif;
    --qmd-font-head: ui-sans-serif, system-ui, sans-serif;
    --qmd-font-mono: ui-monospace, SFMono-Regular, Menlo, monospace;
    --qmd-maxw: 46rem;
  }
  html { scroll-behavior: smooth; }
  body { max-width: var(--qmd-maxw); margin: 2rem auto; padding: 0 1rem;
         font: var(--qmd-font-body); color: var(--qmd-fg); background: var(--qmd-bg);
         overflow-wrap: break-word; }
  a { color: var(--qmd-link); }
  /* button-style links (Pandoc `[text](url){.btn .btn-outline-* .btn-sm}`) */
  a.btn { display: inline-block; font: 500 .9rem var(--qmd-font-head); text-decoration: none;
    padding: .4rem .95rem; border-radius: 7px; border: 1px solid var(--qmd-accent);
    color: var(--qmd-accent); transition: background .12s ease, color .12s ease; }
  a.btn:hover { background: var(--qmd-accent); color: #fff; }
  a.btn-outline-secondary { border-color: var(--qmd-border); color: var(--qmd-muted); }
  a.btn-outline-secondary:hover { background: var(--qmd-fg); color: var(--qmd-bg); border-color: var(--qmd-fg); }
  a.btn-sm { font-size: .8rem; padding: .25rem .7rem; }
  h1, h2, h3, h4 { font-family: var(--qmd-font-head); line-height: 1.25; }
  .qmd-title-block { margin: 0 0 2rem; padding-bottom: 1rem; border-bottom: 1px solid var(--qmd-border); }
  .qmd-title-block .title { margin: 0 0 .3rem; font-size: 2.1rem; line-height: 1.15; }
  .qmd-title-block .subtitle { margin: .2rem 0; font-size: 1.15rem; color: var(--qmd-muted); font-weight: 400; }
  .qmd-title-block .description { margin: .4rem 0; color: var(--qmd-fg); }
  .qmd-title-block .qmd-title-meta { margin-top: .6rem; display: flex; flex-wrap: wrap; gap: .25rem 1rem;
    font: 14px var(--qmd-font-head); color: var(--qmd-muted); }
  pre { position: relative; padding: 1rem; border-radius: 6px; overflow: auto; font-size: .9em;
        /* same scroll-shadow trick as wide tables: bg-coloured covers (local) ride
           with the content and mask the edge shadows (scroll, pinned to the box)
           until a long line actually has more to scroll toward. */
        background-color: var(--qmd-code-bg);
        background-image:
          linear-gradient(to right, var(--qmd-code-bg) 30%, transparent),
          linear-gradient(to left, var(--qmd-code-bg) 30%, transparent),
          radial-gradient(farthest-side at left, var(--qmd-edge-shadow), transparent),
          radial-gradient(farthest-side at right, var(--qmd-edge-shadow), transparent);
        background-position: left center, right center, left center, right center;
        background-repeat: no-repeat;
        background-size: 38px 100%, 38px 100%, 13px 100%, 13px 100%;
        background-attachment: local, local, scroll, scroll; }
  code { font-family: var(--qmd-font-mono); }
  /* the <pre> is the single horizontal scroll container the scroll-shadow keys off,
     so the inner <code> must not introduce its own scroll box. */
  pre > code { display: block; overflow: visible; background: transparent; padding: 0; }
  /* server-side syntax-highlight scope classes (syntect), light palette (GitHub-ish);
     the dark overrides live in DARK_CSS so the theme toggle restyles code with no
     re-highlight. Unmapped scopes inherit the default code colour. */
  .qhl-comment { color: #6e7781; font-style: italic; }
  .qhl-string { color: #0a3069; }
  .qhl-keyword, .qhl-storage { color: #cf222e; }
  .qhl-constant, .qhl-support { color: #0550ae; }
  .qhl-entity { color: #8250df; }
  .qhl-variable { color: #953800; }
  /* inline code (not the scrollable <pre> kind): a subtle tinted chip so it reads
     as code in prose instead of blending in as same-colour monospace; .875em pulls
     the mono glyphs down to the serif x-height. It also breaks rather than overflowing. */
  :not(pre) > code { background: var(--qmd-code-bg); padding: .1em .35em; border-radius: 4px;
    font-size: .875em; overflow-wrap: anywhere; }
  .qmd-copy { position: absolute; top: .45rem; right: .45rem;
              display: inline-flex; align-items: center; justify-content: center;
              padding: .28rem; line-height: 0; color: #555;
              background: #fff; border: 1px solid #d4d4d4; border-radius: 5px; cursor: pointer;
              opacity: 0; transition: opacity .12s ease, color .12s ease, border-color .12s ease; }
  .qmd-copy svg { display: block; width: 14px; height: 14px; }
  pre:hover .qmd-copy, .qmd-copy:focus-visible { opacity: 1; }
  .qmd-copy:hover { color: #111; border-color: #999; }
  .qmd-copy.qmd-copied { color: #2bb673; border-color: #2bb673; }
  pre.mermaid { background: transparent; padding: .5rem 0; text-align: center; overflow: visible; }
  pre.mermaid svg { max-width: 100%; height: auto; }
  blockquote { border-left: 3px solid var(--qmd-border); margin: 0 0 1rem; padding-left: 1rem; color: var(--qmd-muted); }
  img { max-width: 100%; }
  /* embedded media (OJS/Three.js canvases, SVG figures, video, embeds) is clamped
     to the page width so a fixed-size canvas can't force a horizontal scroll on
     mobile; max-width clamps even against an author width: rule. */
  canvas, svg, video, iframe { max-width: 100%; }
  /* a wide table scrolls within its own box (max-content up to the page width)
     rather than stretching the page and forcing a horizontal scroll on mobile.
     The layered background is a scroll shadow: bg-coloured covers (background-
     attachment: local) ride with the content and mask the edge shadows (scroll,
     pinned to the box) until there is actually more to scroll toward. */
  table { border-collapse: collapse; display: block; width: max-content;
          max-width: 100%; overflow-x: auto;
          background:
            linear-gradient(to right, var(--qmd-bg) 30%, transparent) left center,
            linear-gradient(to left, var(--qmd-bg) 30%, transparent) right center,
            radial-gradient(farthest-side at left, var(--qmd-edge-shadow), transparent) left center,
            radial-gradient(farthest-side at right, var(--qmd-edge-shadow), transparent) right center;
          background-repeat: no-repeat;
          background-size: 38px 100%, 38px 100%, 13px 100%, 13px 100%;
          background-attachment: local, local, scroll, scroll; }
  th, td { border: 1px solid var(--qmd-border); padding: .35rem .6rem; }
  table caption { caption-side: top; font-size: .9em; color: var(--qmd-muted); padding-bottom: .4rem; text-align: left; }
  thead th { border-bottom: 2px solid var(--qmd-border); }
  .callout { border: 1px solid #e0e0e0; border-left-width: 4px; border-radius: 5px;
             margin: 1rem 0; overflow: hidden; }
  .callout-title { font-family: ui-sans-serif, system-ui, sans-serif; font-weight: 600;
                   padding: .5rem .9rem; background: #f6f6f6; }
  .callout-body { padding: .3rem .9rem; }
  .callout-body > :first-child { margin-top: .4rem; }
  .callout-note { border-left-color: #4c8dff; } .callout-note .callout-title { background: #eaf1ff; }
  .callout-tip { border-left-color: #2bb673; } .callout-tip .callout-title { background: #e7f7ef; }
  .callout-warning { border-left-color: #e0a800; } .callout-warning .callout-title { background: #fdf6e3; }
  .callout-important { border-left-color: #e0566b; } .callout-important .callout-title { background: #fdecef; }
  .callout-caution { border-left-color: #e8730c; } .callout-caution .callout-title { background: #fdefe3; }
  .callout-collapse > details > summary.callout-title { cursor: pointer; }
  .callout-collapse > details[open] > summary.callout-title { margin-bottom: 0; }
  details.qmd-code-fold > summary { cursor: pointer; color: var(--qmd-muted);
    font: 600 .85em var(--qmd-font-head); padding: .15rem 0; margin-bottom: .35rem; }
  details.qmd-code-fold > summary:hover { color: var(--qmd-fg); }
  .qmd-xref { text-decoration: none; }
  .qmd-references .csl-entry { margin: .4rem 0; padding-left: 2.2rem; text-indent: -2.2rem; }
  .qmd-output { margin: 0 0 1rem; }
  .qmd-output > pre { background: #fbfbfb; border-left: 3px solid #e3e3e3; }
  .qmd-output img { display: block; max-width: 100%; }
  .qmd-stderr { border-left-color: #e0a800 !important; background: #fdf6e3 !important; }
  .qmd-error { border-left-color: #e0566b !important; background: #fdecef !important; color: #862033; }
  [data-block-id] { scroll-margin-top: 1rem; }
  [data-block-id].qmd-hl { outline: 2px solid var(--qmd-accent); outline-offset: 3px; border-radius: 3px; }
  /* live-edit feedback: a block that just (re)rendered pulses an accent tint, so
     the eye (or a phone) lands on what changed. reduced-motion collapses it away. */
  @keyframes qmd-flash { from { background-color: var(--qmd-flash); } to { background-color: transparent; } }
  [data-block-id].qmd-flash { animation: qmd-flash .8s ease-out; border-radius: 4px; }
  /* single-click click-to-source feedback: the clicked block's accent outline
     fades out (a transient "you clicked here", not a persistent box). The plain
     .qmd-hl above stays reserved for the editor cursor sync, which persists. */
  @keyframes qmd-hl-fade { from { outline-color: var(--qmd-accent); } to { outline-color: transparent; } }
  [data-block-id].qmd-hl-flash { outline: 2px solid var(--qmd-accent); outline-offset: 3px;
    border-radius: 3px; animation: qmd-hl-fade .7s ease-out forwards; }
  figure.qmd-figure { margin: 1.5rem 0; }
  figure.qmd-figure img { max-width: 100%; height: auto; }
  figure.qmd-figure figcaption { font-size: .9em; color: var(--qmd-muted); margin-top: .5rem; }
  .qmd-listing { margin: 1.5rem 0; }
  .qmd-listing-caption { font-size: .9em; color: var(--qmd-muted); margin-bottom: .35rem; }
  .qmd-figure-center { text-align: center; }
  .qmd-figure-right { text-align: right; }
  /* numbered display equation: body centered, (N) right-aligned */
  .qmd-eqn { display: grid; grid-template-columns: 1fr auto; align-items: center; column-gap: 1rem; }
  .qmd-eqn .qmd-eqn-body { min-width: 0; }
  .qmd-eqn .qmd-eqn-body .katex-display { margin: 0; }
  .qmd-eqn .qmd-eqn-number { color: var(--qmd-muted); font-variant-numeric: tabular-nums; white-space: nowrap; }
  /* a display equation wider than the column scrolls horizontally inside its own
     box instead of overflowing the page (the #1 mobile breakage for math docs) */
  .katex-display { overflow-x: auto; overflow-y: hidden; padding: .3rem 0; }
  /* toc layout: content beside a sticky table of contents on wide screens */
  body.has-toc { max-width: 72rem; display: grid; align-items: start; gap: 2.5rem;
                 grid-template-columns: minmax(0, 46rem) 14rem; justify-content: center; }
  body.has-toc > main { min-width: 0; }
  #TOC { position: sticky; top: 2rem; max-height: 92vh; overflow: auto;
         font: 14px/1.5 ui-sans-serif, system-ui, sans-serif; }
  #TOC ul { list-style: none; margin: .2rem 0; padding-left: .9rem; }
  #TOC > ul { padding-left: 0; }
  /* the transparent left border reserves the accent rail's space, so toggling the
     active entry never shifts the text; the client's scrollspy sets .qmd-toc-active */
  #TOC a { display: block; padding: .12rem 0 .12rem .6rem; border-left: 2px solid transparent;
           color: var(--qmd-muted); text-decoration: none; }
  #TOC a:hover { color: var(--qmd-fg); }
  #TOC a.qmd-toc-active { color: var(--qmd-fg); border-left-color: var(--qmd-accent); }
  /* mobile pull-up-sheet chrome: hidden on desktop, revealed in the sheet media query */
  #qmd-toc-handle, #qmd-toc-backdrop { display: none; }
  @media (max-width: 60rem) {
    /* keep the fixed control bar / diagnostics from covering the last line */
    body { padding-bottom: 2.75rem; }
    /* sheet pages lift the control bar above the bottom TOC handle, so clear more */
    body.qmd-toc-sheet { padding-bottom: 4.4rem; }
    /* Static export (no JS): stack the TOC above the content. Keep it a grid so
       the minmax(0,1fr) track clamps <main> to the viewport, and `order` lifts
       the TOC (which follows <main> in the DOM) up instead of stranding it at the
       bottom. The live preview opts out via .qmd-toc-sheet and uses the sheet. */
    body.has-toc:not(.qmd-toc-sheet) { grid-template-columns: minmax(0, 1fr); max-width: var(--qmd-maxw); gap: 0; }
    body.has-toc:not(.qmd-toc-sheet) > #TOC { order: -1; position: static; max-height: 45vh; overflow: auto;
           border-bottom: 1px solid var(--qmd-border); margin-bottom: 1.5rem; padding-bottom: 1rem; }

    /* Live preview: a quiet pull-up handle opens the TOC as a bottom sheet. */
    body.has-toc.qmd-toc-sheet { display: block; max-width: var(--qmd-maxw); }
    /* the nav becomes the sheet: off-screen until .qmd-toc-open slides it up */
    body.qmd-toc-sheet #TOC { position: fixed; inset: auto 0 0 0; z-index: 10001; margin: 0;
      max-height: 72vh; padding: .25rem .6rem calc(1.4rem + env(safe-area-inset-bottom, 0px));
      background: var(--qmd-bg); border-radius: 16px 16px 0 0; box-shadow: 0 -8px 30px rgba(0, 0, 0, .22);
      overflow: auto; overscroll-behavior: contain;
      transform: translateY(100%); transition: transform .3s cubic-bezier(.2, .8, .2, 1); }
    body.qmd-toc-sheet #TOC::before { content: ""; display: block; width: 40px; height: 5px;
      margin: .15rem auto .6rem; border-radius: 3px; background: var(--qmd-border); }
    body.qmd-toc-sheet #TOC a { padding-top: .45rem; padding-bottom: .45rem; }
    body.qmd-toc-sheet.qmd-toc-open #TOC { transform: translateY(0); }
    /* dim backdrop behind the open sheet */
    body.qmd-toc-sheet #qmd-toc-backdrop { display: block; position: fixed; inset: 0; z-index: 10000;
      background: rgba(0, 0, 0, .42); opacity: 0; pointer-events: none; transition: opacity .25s; }
    body.qmd-toc-sheet.qmd-toc-open #qmd-toc-backdrop { opacity: 1; pointer-events: auto; }
    /* the resting handle: a quiet grabber you drag up or tap */
    body.qmd-toc-sheet #qmd-toc-handle { display: flex; flex-direction: column; align-items: center;
      position: fixed; left: 50%; transform: translateX(-50%); z-index: 9997;
      bottom: calc(.45rem + env(safe-area-inset-bottom, 0px)); padding: .4rem 1.3rem .5rem; gap: .3rem;
      border: 0; border-radius: 13px; background: color-mix(in srgb, var(--qmd-bg) 72%, transparent);
      -webkit-backdrop-filter: blur(7px); backdrop-filter: blur(7px); box-shadow: 0 1px 7px rgba(0, 0, 0, .16);
      cursor: grab; touch-action: none; -webkit-user-select: none; user-select: none; }
    body.qmd-toc-sheet.qmd-toc-open #qmd-toc-handle { display: none; }
    #qmd-toc-handle .qmd-toc-grip { width: 42px; height: 5px; border-radius: 3px; background: var(--qmd-muted); opacity: .6; }
    /* current-section chip: hidden at rest, flashes in while scrolling, then fades */
    #qmd-toc-cur { position: absolute; bottom: calc(100% + .35rem); left: 50%;
      transform: translateX(-50%) translateY(5px); max-width: 72vw; white-space: nowrap;
      overflow: hidden; text-overflow: ellipsis; font: 600 12px ui-sans-serif, system-ui, sans-serif;
      color: var(--qmd-muted); background: var(--qmd-bg); border: 1px solid var(--qmd-border);
      padding: .2rem .55rem; border-radius: 9px; box-shadow: 0 2px 8px rgba(0, 0, 0, .12);
      opacity: 0; pointer-events: none; transition: opacity .2s ease, transform .2s ease; }
    #qmd-toc-handle.qmd-show-label #qmd-toc-cur { opacity: 1; transform: translateX(-50%) translateY(0); }
    @keyframes qmdPeekHint { 0%, 100% { bottom: calc(.45rem + env(safe-area-inset-bottom, 0px)); }
      50% { bottom: calc(1.3rem + env(safe-area-inset-bottom, 0px)); } }
    body.qmd-toc-sheet #qmd-toc-handle.qmd-hint { animation: qmdPeekHint 1.15s ease-in-out 2; }
    /* custom multi-column layouts collapse to one column (inline style ⇒ !important).
       minmax(0,1fr), not 1fr: the 0 floor lets wide children (figures/images) shrink
       to the column instead of forcing their intrinsic width and overflowing the page. */
    .qmd-layout { grid-template-columns: minmax(0, 1fr) !important; }
  }
  @media (max-width: 30rem) {
    body { padding-left: .85rem; padding-right: .85rem; font-size: 16px; }
    .qmd-title-block .title { font-size: 1.7rem; }
    .qmd-title-block .subtitle { font-size: 1.05rem; }
  }
  /* honor reduced-motion: collapse transitions/animations (the TOC sheet slide,
     the handle hint bounce, fades) to near-instant, and drop smooth scrolling */
  @media (prefers-reduced-motion: reduce) {
    *, *::before, *::after { animation-duration: .001ms !important; animation-iteration-count: 1 !important;
      transition-duration: .001ms !important; scroll-behavior: auto !important; }
  }
  /* print: drop the live chrome, force a light palette, flow content full width
     with the contents list lifted to the top, and avoid clipping/awkward breaks.
     Placed last so it outranks the width-query rules that also match on paper. */
  @media print {
    html[data-theme="dark"], :root {
      --qmd-bg: #fff; --qmd-fg: #111; --qmd-muted: #555; --qmd-border: #ccc;
      --qmd-code-bg: #f5f5f5; --qmd-link: #0645ad; --qmd-accent: #0645ad;
    }
    #qmd-controls, #qmd-diagnostics, #qmd-toc-handle, #qmd-toc-backdrop, .qmd-copy { display: none !important; }
    body { max-width: none !important; margin: 0 !important; }
    body.has-toc { display: grid !important; grid-template-columns: 1fr !important; gap: 0 !important; }
    body.qmd-toc-sheet #TOC, body.has-toc > #TOC {
      position: static !important; transform: none !important; order: -1 !important;
      max-height: none !important; overflow: visible !important; z-index: auto !important;
      background: transparent !important; box-shadow: none !important; border-radius: 0 !important;
      margin: 0 0 1.5rem !important; padding: 0 0 1rem !important;
      border-bottom: 1px solid var(--qmd-border) !important; }
    #TOC::before { display: none !important; }
    pre { white-space: pre-wrap !important; overflow: visible !important; }
    table { display: table !important; overflow: visible !important; background: none !important; }
    .katex-display { overflow: visible !important; }
    pre, figure, table, blockquote, .callout, .qmd-eqn { break-inside: avoid; }
    h2, h3, h4 { break-after: avoid; }
  }
"#;

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

// Quarto's Observable runtime (vendored, v0.0.18 — not published to any CDN, so
// unlike hljs/reveal it must ship with us). It self-installs `window._ojs` on
// load and drives cells via `interpretFromScriptTags()`. Loaded as a module so
// execution is deferred until <body> exists (the bundle touches document.body).
const OJS_RUNTIME: &str = include_str!("../assets/ojs/quarto-ojs-runtime.min.js");
const OJS_CSS: &str = include_str!("../assets/ojs/quarto-ojs.css");

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

const OJS_INIT: &str = r#"<script type="module">
// Tracks each Python `ojs_define` value we've bound into the live module
// (name -> JSON of the last-bound value) so a later arrival redefines rather
// than double-defines (the runtime rejects a name defined more than once).
window.__qmdOjsDefined = new Map();

// Bind Python `ojs_define` values into the live OJS module. On a cold first load
// the `{python}` cell hasn't executed yet, so its `<script type="ojs-define">` is
// absent when the OJS cells first interpret; the values then arrive via a websocket
// block op. Feeding them in (define the first time, redefine on change) makes the
// Observable runtime reactively recompute the cells that referenced them — so the
// figure binds without a page reload. `allowPendingGlobals` (set before the initial
// interpret) keeps a not-yet-defined reference pending instead of hard-erroring in
// the gap. Call with the freshly-mounted node after an op (or no arg for the doc).
window.qmdBindOjsDefines = function (scope) {
  var ojs = window._ojs, conn = ojs && ojs.ojsConnector;
  if (!conn || !window.__qmdOjsRan) return;
  var scripts = (scope && scope.querySelectorAll ? scope : document)
    .querySelectorAll('script[type="ojs-define"]');
  scripts.forEach(function (s) {
    var parsed;
    try { parsed = JSON.parse(s.textContent); } catch (e) { return; }
    (parsed.contents || []).forEach(function (entry) {
      var name = entry.name, value = entry.value, key = JSON.stringify(value);
      if (window.__qmdOjsDefined.get(name) === key) return;  // unchanged
      try {
        if (window.__qmdOjsDefined.has(name)) {
          conn.mainModule.redefine(name, [], function () { return value; });
        } else {
          conn.define(name)(value);
        }
        window.__qmdOjsDefined.set(name, key);
      } catch (e) { /* keep the prior binding rather than break the cell */ }
    });
  });
};

// Interpret every OJS cell once per page load. The runtime rejects re-defining a
// variable, so re-interpreting an edited cell needs a reload (the client triggers
// one); non-OJS edits hot-update via block ops and leave OJS cells untouched.
window.qmdRunOJS = function () {
  if (window.__qmdOjsRan) return;
  if (!window._ojs || !window._ojs.runtime) return;        // bundle not ready yet
  if (!document.querySelector('script[type="ojs-module-contents"]')) return; // no cells yet
  window.__qmdOjsRan = true;
  window._ojs.selfContained = false;
  if (window._ojs.ojsConnector) window._ojs.ojsConnector.allowPendingGlobals = true;
  window._ojs.paths.runtimeToDoc = ".";                    // page is served from the doc dir
  window._ojs.paths.runtimeToRoot = ".";
  window._ojs.paths.docToRoot = ".";
  // ojs-define scripts present now are bound by interpretFromScriptTags itself;
  // record them so a later arrival of the same name redefines, not double-defines.
  document.querySelectorAll('script[type="ojs-define"]').forEach(function (s) {
    try {
      JSON.parse(s.textContent).contents.forEach(function (e) {
        window.__qmdOjsDefined.set(e.name, JSON.stringify(e.value));
      });
    } catch (e) {}
  });
  window._ojs.runtime.interpretFromScriptTags();
};
window.qmdRunOJS();                                         // one-shot page: cells already present
</script>"#;

/// True if a rendered body contains live Observable cells (gates the OJS assets).
pub fn has_ojs(body: &str) -> bool {
    body.contains("ojs-module-contents")
}

const CODE_ENHANCE_JS: &str = r#"
window.qmdEnhanceCode = function (root) {
  if (!root) return;
  root.querySelectorAll('pre > code').forEach(function (code) {
    var pre = code.parentElement;
    if (pre.dataset.enhanced) return;
    pre.dataset.enhanced = '1';
    // (Code is highlighted server-side; the client only adds the copy button.)
    // GitHub/Claude-style copy glyph (Octicons copy), swapping to a check on success.
    var copyIcon = '<svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0 0 1 0 1.5h-1.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-1.5a.75.75 0 0 1 1.5 0v1.5A1.75 1.75 0 0 1 9.25 16h-7.5A1.75 1.75 0 0 1 0 14.25Z"></path><path d="M5 1.75C5 .784 5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0 1 14.25 11h-7.5A1.75 1.75 0 0 1 5 9.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"></path></svg>';
    var checkIcon = '<svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L1.22 8.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z"></path></svg>';
    var btn = document.createElement('button');
    btn.className = 'qmd-copy';
    btn.type = 'button';
    btn.setAttribute('aria-label', 'Copy code');
    btn.innerHTML = copyIcon;
    btn.addEventListener('click', function () {
      var text = code.innerText;
      var ok = function () {
        btn.innerHTML = checkIcon;
        btn.classList.add('qmd-copied');
        btn.setAttribute('aria-label', 'Copied');
        setTimeout(function () { btn.innerHTML = copyIcon; btn.classList.remove('qmd-copied'); btn.setAttribute('aria-label', 'Copy code'); }, 1200);
      };
      // navigator.clipboard only exists in a secure context; over --host (plain http
      // on the LAN, e.g. a phone) fall back to a hidden-textarea execCommand copy so
      // the button still copies and confirms with the check.
      var legacy = function () {
        try {
          var ta = document.createElement('textarea');
          ta.value = text; ta.setAttribute('readonly', '');
          ta.style.position = 'fixed'; ta.style.top = '0'; ta.style.opacity = '0';
          document.body.appendChild(ta); ta.select();
          var done = document.execCommand('copy'); document.body.removeChild(ta);
          return done;
        } catch (e) { return false; }
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text).then(ok, function () { if (legacy()) ok(); });
      } else if (legacy()) {
        ok();
      }
    });
    pre.appendChild(btn);
    // The button is absolutely positioned inside the <pre>, which is the horizontal
    // scroll container, so it would scroll away with the code. Counter-translate it by
    // the scroll offset to keep it pinned to the visible top-right corner.
    pre.addEventListener('scroll', function () {
      btn.style.transform = pre.scrollLeft ? 'translateX(' + pre.scrollLeft + 'px)' : '';
    }, { passive: true });
  });
  qmdRenderMermaid(root);
  qmdInitLightbox();
  qmdInitLinkPreview();
  qmdInitCategoryFilter(root);
};

// Native category filter for `listing: { categories: true }`: the server emits a
// chip row (`.qmd-cat-filter`) above the card grid and tags each card with
// `data-categories`. Clicking a chip — or a category tag on a card — toggles it
// (multi-select, OR semantics); an empty `data-cat` ("All") clears the filter.
// Works in the static build and the live preview; idempotent per filter.
function qmdInitCategoryFilter(root) {
  (root || document).querySelectorAll('.qmd-cat-filter').forEach(function (filter) {
    if (filter.dataset.qmdCat) return;
    filter.dataset.qmdCat = '1';
    var wrap = filter.closest('.qmd-listing-wrap');
    var listing = wrap && wrap.querySelector('.qmd-listing');
    if (!listing) return;
    var selected = new Set();
    var catsOf = function (card) {
      var raw = card.getAttribute('data-categories');
      return raw ? raw.split(',') : [];
    };
    var apply = function () {
      listing.querySelectorAll('.qmd-card').forEach(function (card) {
        var show = selected.size === 0 || catsOf(card).some(function (c) { return selected.has(c); });
        card.style.display = show ? '' : 'none';
      });
      filter.querySelectorAll('.qmd-cat-chip').forEach(function (chip) {
        var c = chip.getAttribute('data-cat');
        chip.classList.toggle('qmd-cat-active', c === '' ? selected.size === 0 : selected.has(c));
      });
      listing.querySelectorAll('.qmd-cat[data-cat]').forEach(function (tag) {
        tag.classList.toggle('qmd-cat-on', selected.has(tag.getAttribute('data-cat')));
      });
    };
    var toggle = function (cat) {
      if (cat === '') selected.clear();
      else if (selected.has(cat)) selected.delete(cat);
      else selected.add(cat);
      apply();
    };
    filter.addEventListener('click', function (e) {
      var chip = e.target.closest('.qmd-cat-chip');
      if (chip) toggle(chip.getAttribute('data-cat') || '');
    });
    // A category tag on a card toggles its filter instead of opening the post.
    listing.addEventListener('click', function (e) {
      var tag = e.target.closest('.qmd-cat[data-cat]');
      if (!tag) return;
      e.preventDefault();
      e.stopPropagation();
      toggle(tag.getAttribute('data-cat'));
    });
    apply();
  });
}

// Full-screen viewer for figure images AND mermaid diagrams. Set up once; uses
// event delegation in the capture phase so a click opens the lightbox WITHOUT
// triggering the block-level click/double-click handlers (highlight,
// click-to-source). Images are shown via <img>; mermaid SVGs are cloned live
// (so <foreignObject> labels keep rendering, which an <img> would drop). Modifier
// clicks pass through (new tab, reveal alt-zoom). Dismiss: backdrop, Esc, or x.
function qmdInitLightbox() {
  if (window.__qmdLightbox) return;
  window.__qmdLightbox = true;

  var style = document.createElement('style');
  style.textContent =
    'figure img,pre.mermaid{cursor:zoom-in}' +
    '#qmd-lightbox{position:fixed;inset:0;z-index:2147483000;display:none;flex-direction:column;' +
    'align-items:center;justify-content:center;gap:.9rem;padding:2rem;box-sizing:border-box;' +
    'background:rgba(10,12,16,.9);cursor:zoom-out;opacity:0;transition:opacity .15s ease}' +
    '#qmd-lightbox.open{display:flex;opacity:1}' +
    '#qmd-lightbox img{max-width:93vw;max-height:86vh;object-fit:contain;cursor:default;' +
    'background:var(--qmd-bg,#fff);border-radius:4px;box-shadow:0 10px 50px rgba(0,0,0,.5)}' +
    '#qmd-lightbox .qmd-lb-svg{display:none;width:92vw;max-width:1400px;max-height:86vh;overflow:auto;' +
    'cursor:default;background:var(--qmd-bg,#fff);border-radius:4px;padding:1.2rem;box-sizing:border-box;' +
    'box-shadow:0 10px 50px rgba(0,0,0,.5)}' +
    '#qmd-lightbox .qmd-lb-svg svg{display:block;width:100%;height:auto;max-width:100%}' +
    '#qmd-lightbox .qmd-lb-cap{color:#e8e8e8;font:14px ui-sans-serif,system-ui,sans-serif;' +
    'text-align:center;max-width:93vw}' +
    '#qmd-lightbox .qmd-lb-cap:empty{display:none}' +
    '#qmd-lightbox .qmd-lb-close{position:fixed;top:.6rem;right:1rem;color:#fff;background:none;' +
    'border:0;font-size:2.2rem;line-height:1;cursor:pointer;opacity:.75}' +
    '#qmd-lightbox .qmd-lb-close:hover{opacity:1}';
  document.head.appendChild(style);

  var box = document.createElement('div');
  box.id = 'qmd-lightbox';
  box.setAttribute('role', 'dialog');
  box.innerHTML = '<button class="qmd-lb-close" aria-label="Close">×</button>' +
    '<img alt=""><div class="qmd-lb-svg"></div><div class="qmd-lb-cap"></div>';
  document.body.appendChild(box);
  var lbImg = box.querySelector('img');
  var lbSvg = box.querySelector('.qmd-lb-svg');
  var lbCap = box.querySelector('.qmd-lb-cap');

  function openImg(srcImg) {
    lbSvg.style.display = 'none'; lbSvg.innerHTML = '';
    lbImg.style.display = '';
    lbImg.src = srcImg.currentSrc || srcImg.src;
    lbImg.alt = srcImg.alt || '';
    var fig = srcImg.closest('figure');
    var fc = fig && fig.querySelector('figcaption');
    lbCap.textContent = fc ? fc.textContent : (srcImg.alt || '');
    box.classList.add('open');
    document.documentElement.style.overflow = 'hidden'; // lock scroll behind the lightbox
  }
  function openMermaid(pre) {
    var svg = pre.querySelector('svg');
    if (!svg) return; // not rendered yet
    lbImg.style.display = 'none'; lbImg.removeAttribute('src');
    var clone = svg.cloneNode(true);
    clone.removeAttribute('width'); clone.removeAttribute('height');
    clone.style.maxWidth = 'none';
    lbSvg.innerHTML = ''; lbSvg.appendChild(clone);
    lbSvg.style.display = 'block';
    // Show the figure's caption in the zoom too (empty -> hidden by CSS).
    var fig = pre.closest('figure');
    var fc = fig && fig.querySelector('figcaption');
    lbCap.textContent = fc ? fc.textContent : '';
    box.classList.add('open');
    document.documentElement.style.overflow = 'hidden'; // lock scroll behind the lightbox
  }
  function close() {
    box.classList.remove('open');
    document.documentElement.style.overflow = ''; // restore page scroll
    lbImg.removeAttribute('src');
    lbSvg.innerHTML = '';
  }

  var unmodified = function (e) {
    return !e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey;
  };
  document.addEventListener('click', function (e) {
    if (!e.target.closest) return;
    if (e.target.closest('figure img') && unmodified(e)) {
      e.preventDefault(); e.stopPropagation(); openImg(e.target);
    } else {
      var pre = e.target.closest('pre.mermaid');
      if (pre && pre.querySelector('svg') && unmodified(e)) {
        e.preventDefault(); e.stopPropagation(); openMermaid(pre);
      }
    }
  }, true);
  // Keep a double-click on a figure/diagram from reaching click-to-source.
  document.addEventListener('dblclick', function (e) {
    if (e.target.closest && e.target.closest('figure img, pre.mermaid')) {
      e.preventDefault(); e.stopPropagation();
    }
  }, true);
  box.addEventListener('click', function (e) {
    if (e.target !== lbImg && !lbSvg.contains(e.target)) close();
  });
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape' && box.classList.contains('open')) close();
  });
}

// mermaid bakes colours into the SVG at run() time, so a diagram can't be
// recoloured by CSS when the theme flips — it has to be re-rendered. The config is
// CSS-driven so a theme extension can style diagrams with no JS: set
// `--qmd-mermaid-theme` (a mermaid theme name; defaults to dark/default by mode),
// and optionally `--qmd-mermaid-{bg,node,node-border,text,line}` to tune colours
// (most effective with `--qmd-mermaid-theme: base`). Each diagram's source is
// stashed (dataset.src) so a later `qmd:themechange` can restore and re-run it.
function qmdMermaidConfig() {
  var cs = getComputedStyle(document.documentElement);
  var get = function (n) { return cs.getPropertyValue(n).trim(); };
  var dark = document.documentElement.getAttribute('data-theme') === 'dark';
  var cfg = { startOnLoad: false, theme: get('--qmd-mermaid-theme') || (dark ? 'dark' : 'default') };
  var map = {
    background: '--qmd-mermaid-bg',
    primaryColor: '--qmd-mermaid-node',
    primaryBorderColor: '--qmd-mermaid-node-border',
    primaryTextColor: '--qmd-mermaid-text',
    lineColor: '--qmd-mermaid-line',
  };
  var vars = {};
  for (var key in map) { var v = get(map[key]); if (v) { vars[key] = v; } }
  if (Object.keys(vars).length) { cfg.themeVariables = vars; }
  return cfg;
}
function qmdRunMermaid(nodes) {
  try {
    window.mermaid.initialize(qmdMermaidConfig());
    window.mermaid.run({ nodes: nodes });
  } catch (e) {}
}
// Quarto-style hover preview for internal links: hovering a citation, a cross
// reference, or a section link pops up a small card previewing its target (the
// reference entry, the figure + caption, the equation, the section heading + its
// first lines). Server-rendered, so the clone needs no re-running (math is already
// KaTeX HTML). Set up once via event delegation, so it survives block swaps;
// table-of-contents links are skipped (navigational, not worth a popup).
function qmdInitLinkPreview() {
  if (window.__qmdLinkPreview) return;
  window.__qmdLinkPreview = true;

  var style = document.createElement('style');
  style.textContent =
    '#qmd-link-preview{position:fixed;z-index:2147482000;max-width:min(440px,90vw);max-height:50vh;' +
    'overflow:auto;background:var(--qmd-bg,#fff);color:var(--qmd-fg,#111);' +
    'border:1px solid var(--qmd-border,#e0e0e0);border-radius:8px;box-shadow:0 6px 30px rgba(0,0,0,.22);' +
    'padding:.7rem .9rem;font-size:.9rem;line-height:1.45;opacity:0;transform:translateY(3px);' +
    'transition:opacity .12s ease,transform .12s ease;pointer-events:none;visibility:hidden;}' +
    '#qmd-link-preview.open{opacity:1;transform:none;pointer-events:auto;visibility:visible;}' +
    '#qmd-link-preview > :first-child{margin-top:0;}#qmd-link-preview > :last-child{margin-bottom:0;}' +
    '#qmd-link-preview img{max-width:100%;height:auto;}#qmd-link-preview figure{margin:0;}' +
    '#qmd-link-preview .qmd-lp-head{font-weight:600;}';
  document.head.appendChild(style);

  var card = document.createElement('div');
  card.id = 'qmd-link-preview';
  card.setAttribute('role', 'tooltip');
  document.body.appendChild(card);

  var showTimer = null, hideTimer = null;

  function eligible(a) {
    if (!a) return false;
    var href = a.getAttribute('href') || '';
    if (href.charAt(0) !== '#' || href.length < 2) return false;
    return !a.closest('#TOC') && !a.closest('#qmd-link-preview');
  }
  // Build the preview body for a target element. A heading shows itself plus the
  // following block(s) up to the next heading; anything else is cloned whole.
  function buildPreview(target) {
    if (/^H[1-6]$/.test(target.tagName)) {
      var frag = document.createElement('div');
      var head = document.createElement('div');
      head.className = 'qmd-lp-head';
      head.textContent = target.textContent;
      frag.appendChild(head);
      var n = target.nextElementSibling, added = 0;
      while (n && added < 2 && !/^H[1-6]$/.test(n.tagName) && !n.id) {
        frag.appendChild(n.cloneNode(true));
        added++; n = n.nextElementSibling;
      }
      return frag;
    }
    return target.cloneNode(true);
  }
  function place(link) {
    var r = link.getBoundingClientRect();
    var cw = card.offsetWidth, ch = card.offsetHeight;
    var left = Math.min(Math.max(8, r.left), window.innerWidth - cw - 8);
    var top = r.top - ch - 8;             // prefer above the link
    if (top < 8) top = r.bottom + 8;      // flip below when there is no room
    card.style.left = left + 'px';
    card.style.top = Math.max(8, top) + 'px';
  }
  function show(link) {
    var id = decodeURIComponent((link.getAttribute('href') || '').slice(1));
    var target = id && document.getElementById(id);
    if (!target) return;
    var body = buildPreview(target);
    if (!body || !body.textContent.trim()) return;
    card.innerHTML = '';
    card.appendChild(body);
    card.classList.add('open');
    place(link);
  }
  function scheduleShow(link) {
    clearTimeout(hideTimer); clearTimeout(showTimer);
    showTimer = setTimeout(function () { show(link); }, 140);
  }
  function hide() { clearTimeout(showTimer); card.classList.remove('open'); }
  function scheduleHide() { clearTimeout(hideTimer); hideTimer = setTimeout(hide, 160); }

  document.addEventListener('mouseover', function (e) {
    var a = e.target.closest && e.target.closest("a[href^='#']");
    if (eligible(a)) scheduleShow(a);
  });
  document.addEventListener('mouseout', function (e) {
    var a = e.target.closest && e.target.closest("a[href^='#']");
    if (a && eligible(a)) {
      var to = e.relatedTarget;
      if (to && to.closest && to.closest('#qmd-link-preview')) return; // moving into the card
      scheduleHide();
    }
  });
  card.addEventListener('mouseenter', function () { clearTimeout(hideTimer); });
  card.addEventListener('mouseleave', scheduleHide);
  window.addEventListener('scroll', hide, true);
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') hide(); });
}

function qmdRenderMermaid(root) {
  var pending = root.querySelectorAll('pre.mermaid:not([data-processed])');
  if (!pending.length) return;
  // Keep the source text so the diagram survives a theme-driven re-render.
  pending.forEach(function (p) { if (p.dataset.src == null) p.dataset.src = p.textContent; });
  if (window.mermaid) { qmdRunMermaid(pending); return; }
  if (window.__qmdMermaidLoading) return; // its onload will sweep the whole doc
  window.__qmdMermaidLoading = true;
  var s = document.createElement('script');
  s.src = '{{MERMAID}}';
  s.onload = function () {
    qmdRunMermaid(document.querySelectorAll('pre.mermaid:not([data-processed])'));
  };
  document.head.appendChild(s);
}
// Re-render every diagram from its stashed source under the new theme.
function qmdReRenderMermaid() {
  if (!window.mermaid) return; // not loaded yet => first render will use the theme
  var all = document.querySelectorAll('pre.mermaid');
  if (!all.length) return;
  all.forEach(function (p) {
    if (p.dataset.src == null) return;
    p.textContent = p.dataset.src;
    p.removeAttribute('data-processed');
  });
  qmdRunMermaid(document.querySelectorAll('pre.mermaid:not([data-processed])'));
}
window.addEventListener('qmd:themechange', qmdReRenderMermaid);
"#;

fn page_from_doc(doc: &RenderedDoc, fallback_title: &str) -> String {
    match doc.format {
        DocFormat::Reveal => reveal_page_from_doc(doc, fallback_title),
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
    pub prevnext_html: String,
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
                "{nav}\n<div class=\"{main_cls}\">\n{content}{prevnext}</div>\n{footer}\n",
                nav = s.navbar_html,
                prevnext = s.prevnext_html,
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

/// Wrap resolved theme override CSS in a `<style>` (empty string when there is
/// no override, i.e. the default light theme).
fn theme_style(theme_css: &str) -> String {
    if theme_css.trim().is_empty() {
        String::new()
    } else {
        format!("<style>{theme_css}</style>")
    }
}

fn reveal_page_from_doc(doc: &RenderedDoc, fallback_title: &str) -> String {
    let title = doc.title.as_deref().unwrap_or(fallback_title);
    let mut t = String::new();
    escape_html(title, &mut t);
    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    // Only ship the (large) KaTeX stylesheet when the deck actually has math.
    let katex_css = if slides.contains("class=\"katex") {
        format!("<style>{KATEX_CSS}</style>\n")
    } else {
        String::new()
    };
    // A custom `theme:` CSS layer and the `include-*` front-matter apply to decks
    // just like HTML pages — a deck (or an installed reveal theme extension) can
    // restyle reveal and inject head/body markup. `theme` comes after reveal's own
    // stylesheets so it overrides them; the css folded into `include-in-header`
    // follows last.
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n\
         <meta charset=\"utf-8\" />\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no\" />\n\
         <title>{t}</title>\n{links}{katex_css}<style>{REVEAL_EXTRA_CSS}</style>\n{code_head}\n{theme}{in_header}\
         </head>\n<body>\n{before_body}<div class=\"reveal\">\n<div class=\"slides\">\n{slides}</div>\n</div>\n\
         {script}\n<script>\n  Reveal.initialize({{ hash: true, slideNumber: 'c/t', center: false }});\n</script>\n\
         {code_scripts}\n\
         <script>document.addEventListener('DOMContentLoaded',function(){{window.qmdEnhanceCode&&window.qmdEnhanceCode(document.body);}});</script>\n\
         {after_body}</body>\n</html>\n",
        links = reveal_stylesheet_links(),
        theme = theme_style(&doc.theme_css),
        in_header = doc.includes.in_header,
        before_body = doc.includes.before_body,
        after_body = doc.includes.after_body,
        script = reveal_library_script(),
        code_head = code_head(),
        code_scripts = code_scripts(),
    )
}

/// A jsDelivr URL for a file under reveal.js's `dist/`, with the version pinned
/// in one place so the one-shot page and the live client never diverge.
fn reveal_cdn(path: &str) -> String {
    format!("https://cdn.jsdelivr.net/npm/reveal.js@5.1.0/dist/{path}")
}

/// The reveal.js core + theme `<link rel="stylesheet">` tags.
fn reveal_stylesheet_links() -> String {
    ["reset.css", "reveal.css", "theme/white.css"]
        .iter()
        .map(|f| format!("<link rel=\"stylesheet\" href=\"{}\" />\n", reveal_cdn(f)))
        .collect()
}

/// The reveal.js library `<script>` tag (must load before any `Reveal` call).
fn reveal_library_script() -> String {
    format!("<script src=\"{}\"></script>", reveal_cdn("reveal.js"))
}

/// `<head>` markup for the live reveal.js deck client: reveal stylesheets plus
/// the bundled KaTeX stylesheet (a live deck may gain math on any edit) and the
/// slide tweaks. The blog [`client_styles`] body CSS is deliberately omitted —
/// it would fight reveal's own layout.
pub fn reveal_client_head() -> String {
    format!(
        "{links}<style>{KATEX_CSS}</style>\n<style>{REVEAL_EXTRA_CSS}</style>",
        links = reveal_stylesheet_links(),
    )
}

/// The reveal.js library `<script>` for the live deck client; load it before
/// the preview client so `Reveal` is defined when the deck mounts.
pub fn reveal_client_script() -> String {
    reveal_library_script()
}

// --- reveal.js slide model ----------------------------------------------

/// Quarto's default `slide-level`: headings at this level start a new slide;
/// headings above it (h1) open a vertical stack of sub-slides.
const SLIDE_LEVEL: u8 = 2;

/// One slide's contents: the heading level that opened it (0 when opened by a
/// `---` break or leading content), an optional id slug, and the inner block
/// HTML (each block keeps its own `data-block-id`/`data-sourcepos`).
#[derive(Clone)]
struct SlideBuf {
    level: u8,
    from_rule: bool,
    id: Option<String>,
    blocks: Vec<String>,
}

/// A top-level (horizontal) slide, optionally carrying vertical sub-slides.
enum Top {
    Slide(SlideBuf),
    Stack {
        lead: SlideBuf,
        children: Vec<SlideBuf>,
    },
}

/// Build the inner HTML of reveal's `<div class="slides">`: an optional title
/// slide from front matter, then one `<section>` per slide. Blocks are grouped
/// into slides by heading level (`SLIDE_LEVEL`) and `---` breaks, with h1s
/// wrapping their h2s as a vertical stack.
pub fn slides_html(title: Option<&str>, subtitle: Option<&str>, blocks: &[Block]) -> String {
    let mut out = String::new();
    if let Some(title) = title {
        out.push_str("<section id=\"title-slide\" class=\"quarto-title-block center\">\n<h1 class=\"title\">");
        escape_html(title, &mut out);
        out.push_str("</h1>\n");
        if let Some(sub) = subtitle {
            out.push_str("<p class=\"subtitle\">");
            escape_html(sub, &mut out);
            out.push_str("</p>\n");
        }
        out.push_str("</section>\n");
    }
    for top in group_slides(blocks) {
        render_top(&top, &mut out);
    }
    out
}

/// Split blocks into flat slides at slide-level headings and `---` breaks,
/// then nest h2 slides under any preceding h1 as a vertical stack.
fn group_slides(blocks: &[Block]) -> Vec<Top> {
    let flat = split_slides(blocks);
    let mut tops: Vec<Top> = Vec::new();
    let mut i = 0;
    while i < flat.len() {
        let opens_stack = flat[i].level != 0 && flat[i].level < SLIDE_LEVEL && !flat[i].from_rule;
        if opens_stack {
            let lead = flat[i].clone();
            i += 1;
            let mut children = Vec::new();
            // Gather following slides as vertical children until the next
            // above-slide-level heading or a `---` break pops the stack.
            while i < flat.len() {
                let c = &flat[i];
                let breaks = c.from_rule || (c.level != 0 && c.level < SLIDE_LEVEL);
                if breaks {
                    break;
                }
                children.push(flat[i].clone());
                i += 1;
            }
            if children.is_empty() {
                tops.push(Top::Slide(lead));
            } else {
                tops.push(Top::Stack { lead, children });
            }
        } else {
            tops.push(Top::Slide(flat[i].clone()));
            i += 1;
        }
    }
    tops
}

/// First pass: a new slide begins at any heading with level <= `SLIDE_LEVEL` or
/// at a `---` break (whose `<hr>` is dropped). Deeper headings and other blocks
/// accrete onto the current slide. Empty slides (e.g. back-to-back breaks) are
/// dropped.
fn split_slides(blocks: &[Block]) -> Vec<SlideBuf> {
    let mut slides: Vec<SlideBuf> = Vec::new();
    let mut cur: Option<SlideBuf> = None;
    for b in blocks {
        if is_slide_break(&b.html) {
            slides.extend(cur.take());
            cur = Some(SlideBuf {
                level: 0,
                from_rule: true,
                id: None,
                blocks: Vec::new(),
            });
            continue; // the `<hr>` is the delimiter, not content
        }
        if let Some(level) = block_heading_level(&b.html)
            && level <= SLIDE_LEVEL
        {
            slides.extend(cur.take());
            cur = Some(SlideBuf {
                level,
                from_rule: false,
                id: Some(slugify(&strip_tags(&b.html))),
                blocks: vec![b.html.clone()],
            });
            continue;
        }
        match &mut cur {
            Some(s) => s.blocks.push(b.html.clone()),
            None => {
                cur = Some(SlideBuf {
                    level: 0,
                    from_rule: false,
                    id: None,
                    blocks: vec![b.html.clone()],
                })
            }
        }
    }
    slides.extend(cur);
    slides.retain(|s| !s.blocks.is_empty());
    slides
}

fn render_top(top: &Top, out: &mut String) {
    match top {
        Top::Slide(s) => render_section(s, out),
        Top::Stack { lead, children } => {
            out.push_str("<section>\n");
            render_section(lead, out);
            for c in children {
                render_section(c, out);
            }
            out.push_str("</section>\n");
        }
    }
}

fn render_section(s: &SlideBuf, out: &mut String) {
    out.push_str("<section");
    if let Some(id) = s.id.as_deref().filter(|id| !id.is_empty()) {
        out.push_str(&format!(" id=\"{}\"", escape_attr(id)));
    }
    if s.level != 0 {
        out.push_str(&format!(" class=\"slide level{}\"", s.level));
    } else {
        out.push_str(" class=\"slide\"");
    }
    out.push_str(">\n");
    for b in &s.blocks {
        out.push_str(b);
        out.push('\n');
    }
    out.push_str("</section>\n");
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
fn is_slide_break(html: &str) -> bool {
    html.starts_with("<hr")
}

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

/// Parse a `.class #id` attribute block. Returns `None` unless every token is a
/// `.class` or `#id` (so non-attribute braces are left untouched).
fn parse_pandoc_attrs(s: &str) -> Option<(Vec<String>, Option<String>)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut classes = Vec::new();
    let mut id = None;
    for tok in s.split_whitespace() {
        if let Some(c) = tok.strip_prefix('.').filter(|c| !c.is_empty()) {
            classes.push(c.to_string());
        } else if let Some(i) = tok.strip_prefix('#').filter(|i| !i.is_empty()) {
            id = Some(i.to_string());
        } else {
            return None;
        }
    }
    Some((classes, id))
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

/// A standalone-image paragraph recognized as a figure.
struct FigureParts {
    url: String,
    /// Rendered inline HTML of the caption (the image's alt content).
    caption: String,
    attrs: DivAttrs,
}

/// If `node` is a paragraph that is a single image, optionally followed by a
/// `{#id key=val}` attribute block, return its figure parts. Any other content
/// in the paragraph (stray text, a link, a second image) disqualifies it, so it
/// falls through to ordinary inline-image rendering.
fn figure_parts<'a>(node: &'a AstNode<'a>) -> Option<FigureParts> {
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
fn emit_figure(fig: &FigureParts, block_attrs: &str, num: usize) -> String {
    let id_attr = match &fig.attrs.id {
        Some(i) => format!(" id=\"{}\"", escape_attr(i)),
        None => String::new(),
    };
    let align_class = match fig.attrs.get("fig-align") {
        Some("left") => " qmd-figure-left",
        Some("right") => " qmd-figure-right",
        _ => " qmd-figure-center",
    };
    let style = match fig.attrs.get("width") {
        Some(w) => format!(" style=\"width:{}\"", escape_attr(w)),
        None => String::new(),
    };
    let alt = strip_tags(&fig.caption);
    format!(
        "<figure{block_attrs}{id_attr} class=\"qmd-figure{align_class}\">\
         <img src=\"{}\" alt=\"{}\"{style} />\
         <figcaption>Figure&nbsp;{num}: {}</figcaption></figure>",
        escape_attr(&fig.url),
        escape_attr(&alt),
        fig.caption,
    )
}

/// Render a labelled/captioned `{mermaid}` cell as a numbered `<figure>` wrapping
/// the diagram `<pre>`, carrying the block attrs and (when labelled) the `#fig-`
/// anchor so `@fig-x` cross-references resolve and click-to-zoom still works.
fn emit_mermaid_figure(
    code: &str,
    anchor: Option<&str>,
    caption: Option<&str>,
    block_attrs: &str,
    num: usize,
) -> String {
    let id_attr = match anchor {
        Some(a) => format!(" id=\"{}\"", escape_attr(a)),
        None => String::new(),
    };
    let mut diagram = String::new();
    escape_html(code, &mut diagram);
    let figcap = match caption.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("Figure&nbsp;{num}: {}", html_escape(c)),
        None => format!("Figure&nbsp;{num}"),
    };
    format!(
        "<figure{block_attrs}{id_attr} class=\"qmd-figure qmd-figure-center\">\
         <pre class=\"mermaid\">{diagram}</pre>\
         <figcaption>{figcap}</figcaption></figure>"
    )
}

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

/// Emit `node`'s HTML, applying `attrs` to its root element (top-level only).
fn emit<'a>(node: &'a AstNode<'a>, attrs: &str, out: &mut String) {
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
                    out.push_str(&format!(
                        "<pre{attrs}><code{class}>{highlighted}</code></pre>"
                    ));
                }
            }
        }
        NodeValue::HtmlBlock(hb) => emit_html_block(&hb.literal, attrs, out),
        NodeValue::HtmlInline(h) => out.push_str(h),
        NodeValue::Math(m) => out.push_str(&crate::math::render(&m.literal, m.display_math)),
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

fn wrap<'a>(node: &'a AstNode<'a>, tag: &str, out: &mut String) {
    out.push('<');
    out.push_str(tag);
    out.push('>');
    emit_children(node, out);
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn emit_children<'a>(node: &'a AstNode<'a>, out: &mut String) {
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

/// Blank out Quarto fenced-div markers (`::: {...}` / `:::`) without changing
/// the line count, so the inner content parses as ordinary blocks and every
/// other block's sourcepos line numbers stay valid against the original source.
fn preprocess(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for (i, line) in src.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if parse_fence(line.trim_start()).is_none() {
            out.push_str(line);
        }
    }
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A pandoc/Quarto fenced-div marker: 3+ colons, then nothing (close) or an
/// attribute block / bare class name (open).
enum Fence {
    /// Opening fence; carries the raw attribute string (without the braces).
    Open(String),
    /// Closing fence (bare colons).
    Close,
}

fn parse_fence(s: &str) -> Option<Fence> {
    let colons = s.chars().take_while(|&c| c == ':').count();
    if colons < 3 {
        return None;
    }
    let rest = s[colons..].trim();
    if rest.is_empty() {
        Some(Fence::Close)
    } else if let Some(inner) = rest.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        Some(Fence::Open(inner.trim().to_string()))
    } else if rest.chars().next().is_some_and(char::is_alphabetic) {
        // bare `::: classname` -> treat the first word as a class
        Some(Fence::Open(format!(
            ".{}",
            rest.split_whitespace().next().unwrap_or("")
        )))
    } else {
        None
    }
}

/// A fenced-div span in buffer-line space (1-based, inclusive of the markers).
struct DivSpan {
    open: usize,
    close: usize,
    /// Raw attribute string from the opening fence (e.g. `.callout-note title="X"`).
    attrs: String,
}

/// Find all fenced-div spans (stack-based, so nesting is handled). Sorted so
/// that for a shared opening line the outermost (latest close) comes first.
fn scan_div_spans(src: &str) -> Vec<DivSpan> {
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut spans: Vec<DivSpan> = Vec::new();
    for (i, line) in src.lines().enumerate() {
        match parse_fence(line.trim_start()) {
            Some(Fence::Open(attrs)) => stack.push((i + 1, attrs)),
            Some(Fence::Close) => {
                if let Some((open, attrs)) = stack.pop() {
                    spans.push(DivSpan {
                        open,
                        close: i + 1,
                        attrs,
                    });
                }
            }
            None => {}
        }
    }
    spans.sort_by_key(|s| (s.open, std::cmp::Reverse(s.close)));
    spans
}

/// Parsed fenced-div attributes.
#[derive(Default)]
struct DivAttrs {
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

/// Parse a fenced-div attribute string: `.class`, `#id`, and `key=val`
/// (value optionally quoted), whitespace-separated.
fn parse_attrs(s: &str) -> DivAttrs {
    let mut attrs = DivAttrs::default();
    for tok in tokenize_attrs(s) {
        if let Some(c) = tok.strip_prefix('.') {
            attrs.classes.push(c.to_string());
        } else if let Some(i) = tok.strip_prefix('#') {
            attrs.id = Some(i.to_string());
        } else if let Some((k, v)) = tok.split_once('=') {
            attrs
                .kv
                .push((k.to_string(), v.trim_matches(['"', '\'']).to_string()));
        } else if !tok.is_empty() {
            attrs.classes.push(tok.to_string());
        }
    }
    attrs
}

/// Split on whitespace, but keep quoted values (e.g. `title="a b"`) together.
fn tokenize_attrs(s: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in s.chars() {
        match quote {
            Some(q) => {
                cur.push(ch);
                if ch == q {
                    quote = None;
                }
            }
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                cur.push(ch);
            }
            None if ch.is_whitespace() => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}

/// Group flat top-level blocks back into fenced-div container blocks (callouts,
/// layout grids, generic divs), honoring nesting. Blocks inside a div become a
/// single container block whose HTML embeds them (they keep their own ids and
/// sourcepos, so click-to-source still works inside).
fn group_divs(
    flat: Vec<FlatBlock>,
    spans: &[DivSpan],
    origins: Option<&[LineOrigin]>,
    counts: &mut HashMap<String, u32>,
) -> Vec<Block> {
    struct Open<'a> {
        span: &'a DivSpan,
        inner: Vec<Block>,
    }
    let mut result: Vec<Block> = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut span_idx = 0;

    let push_block =
        |stack: &mut Vec<Open>, result: &mut Vec<Block>, b: Block| match stack.last_mut() {
            Some(top) => top.inner.push(b),
            None => result.push(b),
        };

    for (i, fb) in flat.iter().enumerate() {
        // Open every span that starts before this block and contains it.
        while span_idx < spans.len()
            && spans[span_idx].open < fb.buf_start
            && spans[span_idx].close > fb.buf_start
        {
            stack.push(Open {
                span: &spans[span_idx],
                inner: Vec::new(),
            });
            span_idx += 1;
        }
        // Skip any spans that contain no blocks (degenerate/empty divs).
        while span_idx < spans.len() && spans[span_idx].close < fb.buf_start {
            span_idx += 1;
        }

        push_block(&mut stack, &mut result, fb.block.clone());

        // Close spans that end before the next block begins (innermost first).
        let next_start = flat.get(i + 1).map(|n| n.buf_start).unwrap_or(usize::MAX);
        while let Some(top) = stack.last() {
            if top.span.close < next_start {
                let done = stack.pop().unwrap();
                let container = build_container(done.span, done.inner, origins, counts);
                push_block(&mut stack, &mut result, container);
            } else {
                break;
            }
        }
    }
    // Close anything still open (e.g. unterminated div at EOF).
    while let Some(done) = stack.pop() {
        let container = build_container(done.span, done.inner, origins, counts);
        push_block(&mut stack, &mut result, container);
    }
    result
}

/// Render one fenced div as a container block: callouts, layout grids, or a
/// generic class div.
fn build_container(
    span: &DivSpan,
    mut inner: Vec<Block>,
    origins: Option<&[LineOrigin]>,
    counts: &mut HashMap<String, u32>,
) -> Block {
    let attrs = parse_attrs(&span.attrs);
    let id = make_id(&format!("div:{}", span.attrs), counts);
    let (file, open_line) = map_origin(origins, span.open);
    let (_, close_line) = map_origin(origins, span.close);
    let sourcepos = format!("{open_line}:1-{close_line}:3");
    let file_attr = match &file {
        Some(f) => format!(" data-source-file=\"{}\"", escape_attr(f)),
        None => String::new(),
    };
    let data = format!(" data-block-id=\"{id}\" data-sourcepos=\"{sourcepos}\"{file_attr}");

    let html = if let Some(kind) = attrs.callout_kind() {
        // Callout: use a `title="..."` attr, else a leading heading, else the kind.
        let title = match attrs.get("title") {
            Some(t) => html_escape(t),
            None if inner.first().is_some_and(|b| is_heading(&b.html)) => {
                strip_tags(&inner.remove(0).html)
            }
            None => capitalize(kind),
        };
        let body: String = inner.iter().map(|b| b.html.as_str()).collect();
        // `collapse="true"` makes the callout a native <details> (starts closed);
        // `collapse="false"` is collapsible but starts open.
        match attrs.get("collapse") {
            Some(v) => {
                let open = if v == "false" { " open" } else { "" };
                format!(
                    "<div class=\"callout callout-{kind} callout-collapse\"{data}><details{open}><summary class=\"callout-title\">{title}</summary><div class=\"callout-body\">{body}</div></details></div>"
                )
            }
            None => format!(
                "<div class=\"callout callout-{kind}\"{data}><div class=\"callout-title\">{title}</div><div class=\"callout-body\">{body}</div></div>"
            ),
        }
    } else if let Some(ncol) = attrs.get("layout-ncol").and_then(|n| n.parse::<u32>().ok()) {
        let body: String = inner.iter().map(|b| b.html.as_str()).collect();
        format!(
            "<div class=\"qmd-layout\" style=\"display:grid;grid-template-columns:repeat({ncol},minmax(0,1fr));gap:1rem\"{data}>{body}</div>"
        )
    } else {
        let mut class = attrs.classes.join(" ");
        if class.is_empty() {
            class.push_str("qmd-div");
        }
        let id_attr = match &attrs.id {
            Some(i) => format!(" id=\"{}\"", escape_attr(i)),
            None => String::new(),
        };
        let body: String = inner.iter().map(|b| b.html.as_str()).collect();
        format!("<div class=\"{class}\"{id_attr}{data}>{body}</div>")
    };

    Block {
        id,
        sourcepos,
        source_file: file,
        html,
        cell: None,
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

pub(crate) fn html_escape(s: &str) -> String {
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

fn escape_attr(s: &str) -> String {
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
const SITE_CSS: &str = r#"
  /* Site shell: a full-width flex column so the footer sits at the bottom of the
     viewport even on short pages. The navbar, content, and footer each centre
     their own inner box on the reading column, so they all line up. */
  body.qmd-site { max-width: none; margin: 0; padding: 0;
    min-height: 100vh; display: flex; flex-direction: column; }

  /* sticky theme-aware navbar (spans full width as a flex child; inner centres) */
  .qmd-site-nav { position: sticky; top: 0; z-index: 50;
    background: color-mix(in srgb, var(--qmd-bg) 88%, transparent);
    -webkit-backdrop-filter: saturate(1.4) blur(9px); backdrop-filter: saturate(1.4) blur(9px);
    border-bottom: 1px solid var(--qmd-border); }
  .qmd-nav-inner { position: relative; max-width: var(--qmd-maxw); margin: 0 auto;
    padding: .6rem 1rem; display: flex; align-items: center; gap: 1.1rem;
    font: 500 15px/1 var(--qmd-font-head); }
  .qmd-nav-brand { color: var(--qmd-fg); font-weight: 700; font-size: 1.02rem; text-decoration: none;
    letter-spacing: -0.01em; white-space: nowrap; }
  .qmd-nav-brand:hover { color: var(--qmd-accent); }
  .qmd-nav-links { display: flex; align-items: center; gap: .35rem; flex: 1; }
  .qmd-nav-spacer { flex: 1 1 auto; }
  .qmd-nav-link { color: var(--qmd-muted); text-decoration: none; padding: .35rem .6rem;
    border-radius: 6px; transition: color .12s ease, background .12s ease; }
  .qmd-nav-link:hover { color: var(--qmd-fg); background: var(--qmd-code-bg); }
  .qmd-nav-active { color: var(--qmd-fg); font-weight: 600; }
  .qmd-nav-active:hover { background: transparent; }
  /* real (shipped) light/dark toggle: a subtle icon, not a dev-style button */
  .qmd-theme-toggle { display: inline-flex; align-items: center; justify-content: center;
    width: 2rem; height: 2rem; padding: 0; border: 0; border-radius: 7px; cursor: pointer;
    background: transparent; color: var(--qmd-muted); transition: color .12s ease, background .12s ease; }
  .qmd-theme-toggle:hover { color: var(--qmd-fg); background: var(--qmd-code-bg); }
  .qmd-theme-toggle svg { display: block; }
  .qmd-nav-burger { display: none; }
  .qmd-nav-toggle { display: none; }

  /* the reading column; grows to fill the column so the footer is pushed down */
  .qmd-site-main { flex: 1 0 auto; width: 100%; max-width: var(--qmd-maxw);
    margin: 0 auto; padding: 2rem 1rem; }
  .qmd-site-main > main { min-width: 0; }
  /* `page-layout: full` widens the column (the blog/projects card indexes) */
  .qmd-site-main.qmd-wide { max-width: 60rem; }
  /* a page with a TOC widens into a two-column grid (content + sticky sidebar) */
  .qmd-site-main.has-toc { max-width: 72rem; display: grid; align-items: start;
    gap: 2.5rem; grid-template-columns: minmax(0, 46rem) 14rem; }
  .qmd-site-main.has-toc > .qmd-prevnext { grid-column: 1 / -1; }

  /* prev/next between posts */
  .qmd-prevnext { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem;
    margin: 3.5rem 0 0; padding-top: 1.5rem; border-top: 1px solid var(--qmd-border); }
  .qmd-prevnext-link { display: flex; flex-direction: column; gap: .25rem; text-decoration: none;
    padding: .8rem 1rem; border: 1px solid var(--qmd-border); border-radius: 8px;
    color: var(--qmd-fg); transition: border-color .12s ease, background .12s ease; }
  .qmd-prevnext-link:hover { border-color: var(--qmd-accent); background: var(--qmd-code-bg); }
  .qmd-pn-next { text-align: right; align-items: flex-end; }
  .qmd-pn-dir { font: 600 .8rem var(--qmd-font-head); color: var(--qmd-muted); }
  .qmd-pn-title { font-weight: 600; }

  /* slim footer pinned to the bottom of the flex column (icons = author raw HTML) */
  .qmd-site-footer { flex-shrink: 0; border-top: 1px solid var(--qmd-border); }
  .qmd-foot-inner { max-width: var(--qmd-maxw); margin: 0 auto; padding: 1.4rem 1rem;
    display: flex; align-items: center; gap: 1.25rem; flex-wrap: wrap;
    font: 14px var(--qmd-font-head); color: var(--qmd-muted); }
  .qmd-foot-left, .qmd-foot-center, .qmd-foot-right { display: inline-flex; align-items: center; gap: 1.1rem; }
  .qmd-foot-left:empty, .qmd-foot-center:empty, .qmd-foot-right:empty { display: none; }
  .qmd-foot-center { flex: 1 1 auto; justify-content: center; }
  .qmd-foot-right { margin-left: auto; }
  .qmd-foot-item { color: var(--qmd-muted); text-decoration: none; display: inline-flex; align-items: center; gap: .35rem; }
  .qmd-foot-item:hover { color: var(--qmd-fg); }
  .qmd-foot-item svg { width: 16px; height: 16px; }

  /* listing: post cards (a `grid` of cards, or a stacked `default` list) */
  .qmd-listing { margin: 1.75rem 0; }
  .qmd-listing-grid { display: grid; gap: 1.25rem;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 15rem), 1fr)); }
  .qmd-listing-default { display: flex; flex-direction: column; gap: 1rem; }
  .qmd-card { display: flex; flex-direction: column; text-decoration: none; color: var(--qmd-fg);
    border: 1px solid var(--qmd-border); border-radius: 10px; overflow: hidden; background: var(--qmd-bg);
    transition: border-color .15s ease, transform .15s ease, box-shadow .15s ease; }
  .qmd-card:hover { border-color: var(--qmd-accent); transform: translateY(-2px);
    box-shadow: 0 6px 20px var(--qmd-edge-shadow); }
  .qmd-listing-default .qmd-card { flex-direction: row; align-items: stretch; }
  .qmd-card-img { width: 100%; aspect-ratio: 16 / 9; object-fit: cover; display: block;
    background: var(--qmd-code-bg); }
  .qmd-listing-default .qmd-card-img { width: 12rem; aspect-ratio: 4 / 3; flex: none; }
  .qmd-card-body { padding: .85rem 1rem 1rem; display: flex; flex-direction: column; gap: .35rem; }
  .qmd-card-date { font: 600 .72rem var(--qmd-font-head); color: var(--qmd-muted);
    text-transform: uppercase; letter-spacing: .04em; }
  .qmd-card-title { font: 600 1.08rem/1.25 var(--qmd-font-head); margin: 0; }
  .qmd-card-desc { margin: 0; font-size: .92rem; color: var(--qmd-muted); line-height: 1.5; }
  .qmd-card-cats { display: flex; flex-wrap: wrap; gap: .35rem; margin-top: .25rem; }
  .qmd-cat { font: 500 .72rem var(--qmd-font-head); color: var(--qmd-muted);
    background: var(--qmd-code-bg); border: 1px solid var(--qmd-border);
    border-radius: 999px; padding: .1rem .55rem; transition: background .12s ease, color .12s ease, border-color .12s ease; }
  /* a card's category tag doubles as a filter toggle */
  .qmd-cat[data-cat] { cursor: pointer; }
  .qmd-cat[data-cat]:hover { color: var(--qmd-fg); border-color: var(--qmd-muted); }
  .qmd-cat.qmd-cat-on { background: var(--qmd-accent); border-color: var(--qmd-accent); color: #fff; }

  /* category filter chip row (listing: categories: true) */
  .qmd-cat-filter { display: flex; flex-wrap: wrap; gap: .45rem; margin: 0 0 1.5rem; }
  .qmd-cat-chip { display: inline-flex; align-items: center; gap: .35rem; cursor: pointer;
    font: 500 .8rem var(--qmd-font-head); color: var(--qmd-muted);
    background: var(--qmd-code-bg); border: 1px solid var(--qmd-border); border-radius: 999px;
    padding: .25rem .7rem; transition: background .12s ease, color .12s ease, border-color .12s ease; }
  .qmd-cat-chip:hover { color: var(--qmd-fg); border-color: var(--qmd-muted); }
  .qmd-cat-chip.qmd-cat-active { background: var(--qmd-accent); border-color: var(--qmd-accent); color: #fff; }
  .qmd-cat-count { font-size: .82em; opacity: .65; font-variant-numeric: tabular-nums; }
  .qmd-cat-chip.qmd-cat-active .qmd-cat-count { opacity: .85; }

  /* about: a centered profile header (jolla), replacing the title block */
  .qmd-about { display: flex; flex-direction: column; align-items: center; text-align: center;
    gap: .85rem; margin: 1rem 0 2.5rem; padding-bottom: 1.5rem;
    border-bottom: 1px solid var(--qmd-border); }
  .qmd-about-img { width: 9rem; height: 9rem; border-radius: 50%; object-fit: cover;
    border: 1px solid var(--qmd-border); }
  .qmd-about-name { margin: 0; font-size: 2rem; line-height: 1.15; }
  .qmd-about-links { display: flex; flex-wrap: wrap; gap: .6rem; justify-content: center; }
  .qmd-about-link { font: 500 .9rem var(--qmd-font-head); text-decoration: none;
    color: var(--qmd-fg); border: 1px solid var(--qmd-border); border-radius: 999px;
    padding: .3rem .9rem; transition: border-color .12s ease, color .12s ease; }
  .qmd-about-link:hover { border-color: var(--qmd-accent); color: var(--qmd-accent); }

  @media (max-width: 640px) {
    .qmd-listing-default .qmd-card { flex-direction: column; }
    .qmd-listing-default .qmd-card-img { width: 100%; aspect-ratio: 16 / 9; }
    .qmd-nav-burger { display: flex; flex-direction: column; gap: 4px; cursor: pointer;
      margin-left: auto; padding: .45rem; }
    .qmd-nav-burger span { display: block; width: 22px; height: 2px; border-radius: 2px;
      background: var(--qmd-fg); transition: transform .15s ease, opacity .15s ease; }
    .qmd-nav-links { display: none; position: absolute; top: 100%; left: 0; right: 0;
      flex-direction: column; align-items: stretch; gap: 0; flex: none;
      background: var(--qmd-bg); border-bottom: 1px solid var(--qmd-border);
      box-shadow: 0 8px 20px rgba(0,0,0,.12); padding: .35rem 0; }
    .qmd-nav-toggle:checked ~ .qmd-nav-links { display: flex; }
    .qmd-nav-spacer { display: none; }
    .qmd-nav-link { padding: .7rem 1.15rem; border-radius: 0; }
    .qmd-nav-controls { padding: .5rem 1.15rem; }
    .qmd-site-main.has-toc { grid-template-columns: minmax(0, 1fr); }
    .qmd-prevnext { grid-template-columns: 1fr; }
    .qmd-pn-next { text-align: left; align-items: flex-start; }
  }
"#;

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
{{OJS_INIT}}
{{INCLUDE_AFTER_BODY}}
</body>
</html>
"#;

// reveal.js (pinned to 5.1.0) is served from jsDelivr — the dev server runs
// locally with network access; only KaTeX is bundled for true offline use.

/// Slide-specific tweaks layered over the reveal theme (left-aligned content,
/// centered title slide, readable code/math).
const REVEAL_EXTRA_CSS: &str = r#"
  .reveal .slides { text-align: left; }
  .reveal section.quarto-title-block, .reveal h1.title { text-align: center; }
  .reveal .subtitle { text-align: center; opacity: .75; font-style: italic; }
  .reveal pre { position: relative; width: 100%; box-shadow: none; font-size: .55em; }
  .reveal pre code { max-height: none; }
  .reveal code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
  .reveal .qmd-copy { position: absolute; top: .3em; right: .3em; padding: .1em .4em;
                      font: 600 .5em ui-sans-serif, system-ui, sans-serif; color: #444;
                      background: #fff; border: 1px solid #ccc; border-radius: 5px; cursor: pointer; }
  .reveal .qmd-copy.qmd-copied { color: #2bb673; border-color: #2bb673; }
  .reveal pre.mermaid { background: transparent; padding: 0; text-align: center; }
  .reveal .katex-display { margin: .4em 0; }
  [data-block-id].qmd-hl { outline: 2px solid #4c8dff; outline-offset: 3px; }
"#;

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
