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
use std::path::Path;

/// An executable Quarto code cell (```` ```{lang} ````), exposed so the dev
/// server can run it against a kernel.
#[derive(Debug, Clone)]
pub struct Cell {
    pub lang: String,
    pub code: String,
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
    pub blocks: Vec<Block>,
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
fn render_internal(src: &str, origins: Option<&[LineOrigin]>, base_dir: Option<&Path>) -> RenderedDoc {
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
    let mut format = DocFormat::Html;
    let mut toc = false;
    let mut bib_field: Option<String> = None;
    let mut flat: Vec<FlatBlock> = Vec::new();
    let mut id_counts: HashMap<String, u32> = HashMap::new();
    // Heading anchor slugs (deduped) and the figure number registry, both used
    // for cross-references (`@sec-x`/`@fig-x`) and the table of contents.
    let mut heading_slugs: HashMap<String, u32> = HashMap::new();
    let mut fig_count: usize = 0;
    let mut fig_registry: HashMap<String, String> = HashMap::new();

    for node in root.children() {
        let (buf_start, sourcepos, source_file, block_src, is_paragraph, heading_level, cell) = {
            let data = node.data.borrow();
            if let NodeValue::FrontMatter(fm) = &data.value {
                title = extract_field(fm, "title");
                subtitle = extract_field(fm, "subtitle");
                bib_field = extract_field(fm, "bibliography");
                format = detect_format(fm);
                toc = detect_toc(fm);
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
                    })
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
        let id_attr = match heading_level {
            Some(_) if format == DocFormat::Html => {
                format!(" id=\"{}\"", escape_attr(&dedup_slug(&block_src, &mut heading_slugs)))
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
        } else if let Some(fig) = is_paragraph.then(|| figure_parts(node)).flatten() {
            // Standalone image -> a numbered `<figure>`; register `#fig-` ids so
            // `@fig-x` cross-references resolve to the number.
            fig_count += 1;
            if let Some(fid) = fig.attrs.id.as_deref().filter(|i| i.starts_with("fig-")) {
                fig_registry.insert(fid.to_string(), fig_count.to_string());
            }
            html.push_str(&emit_figure(&fig, &attrs, fig_count));
        } else {
            emit(node, &attrs, &mut html);
        }
        flat.push(FlatBlock {
            buf_start,
            block: Block { id, sourcepos, source_file, html, cell },
        });
    }

    let mut blocks = group_divs(flat, &spans, origins, &mut id_counts);
    let bib = load_bibliography(bib_field.as_deref(), base_dir);
    crate::cite::process(&mut blocks, &bib, &fig_registry);
    RenderedDoc { title, subtitle, format, toc, blocks }
}

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
    if cond { DocFormat::Reveal } else { DocFormat::Html }
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
    page_from_doc(&render_document_with_includes(src, base_dir), fallback_title)
}

/// Self-contained KaTeX stylesheet (fonts inlined as data URIs at build time).
const KATEX_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/katex-inlined.css"));

/// Base document styling (typography, tables, callouts, references, block
/// highlight). Shared by the one-shot page and the live preview client.
const BASE_CSS: &str = r#"
  body { max-width: 46rem; margin: 2rem auto; padding: 0 1rem;
         font: 17px/1.7 ui-serif, Georgia, "Times New Roman", serif; color: #1a1a1a; }
  h1, h2, h3, h4 { font-family: ui-sans-serif, system-ui, sans-serif; line-height: 1.25; }
  pre { position: relative; background: #f5f5f5; padding: 1rem; border-radius: 6px; overflow: auto; font-size: .9em; }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
  .qmd-copy { position: absolute; top: .45rem; right: .45rem; padding: .1rem .45rem;
              font: 600 11px/1.4 ui-sans-serif, system-ui, sans-serif; color: #555;
              background: #fff; border: 1px solid #d4d4d4; border-radius: 5px;
              cursor: pointer; opacity: 0; transition: opacity .12s ease; }
  pre:hover .qmd-copy, .qmd-copy:focus-visible { opacity: 1; }
  .qmd-copy:hover { color: #111; border-color: #999; }
  .qmd-copy.qmd-copied { color: #2bb673; border-color: #2bb673; }
  pre.mermaid { background: transparent; padding: .5rem 0; text-align: center; overflow: visible; }
  pre.mermaid svg { max-width: 100%; height: auto; }
  blockquote { border-left: 3px solid #ddd; margin: 0 0 1rem; padding-left: 1rem; color: #555; }
  img { max-width: 100%; }
  table { border-collapse: collapse; }
  th, td { border: 1px solid #e3e3e3; padding: .35rem .6rem; }
  thead th { border-bottom: 2px solid #ccc; }
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
  .qmd-xref { text-decoration: none; }
  .qmd-references .csl-entry { margin: .4rem 0; padding-left: 2.2rem; text-indent: -2.2rem; }
  .qmd-output { margin: 0 0 1rem; }
  .qmd-output > pre { background: #fbfbfb; border-left: 3px solid #e3e3e3; }
  .qmd-output img { display: block; max-width: 100%; }
  .qmd-stderr { border-left-color: #e0a800 !important; background: #fdf6e3 !important; }
  .qmd-error { border-left-color: #e0566b !important; background: #fdecef !important; color: #862033; }
  [data-block-id] { scroll-margin-top: 1rem; }
  [data-block-id].qmd-hl { outline: 2px solid #4c8dff; outline-offset: 3px; border-radius: 3px; }
  figure.qmd-figure { margin: 1.5rem 0; }
  figure.qmd-figure img { max-width: 100%; height: auto; }
  figure.qmd-figure figcaption { font-size: .9em; color: #555; margin-top: .5rem; }
  .qmd-figure-center { text-align: center; }
  .qmd-figure-right { text-align: right; }
  /* toc layout: content beside a sticky table of contents on wide screens */
  body.has-toc { max-width: 72rem; display: grid; align-items: start; gap: 2.5rem;
                 grid-template-columns: minmax(0, 46rem) 14rem; justify-content: center; }
  body.has-toc > main { min-width: 0; }
  #TOC { position: sticky; top: 2rem; max-height: 92vh; overflow: auto;
         font: 14px/1.5 ui-sans-serif, system-ui, sans-serif; }
  #TOC ul { list-style: none; margin: .2rem 0; padding-left: .9rem; }
  #TOC > ul { padding-left: 0; }
  #TOC a { color: #666; text-decoration: none; }
  #TOC a:hover { color: #1a1a1a; }
  @media (max-width: 60rem) {
    body.has-toc { display: block; }
    #TOC { position: static; max-height: none; border-bottom: 1px solid #eee;
           margin-bottom: 1.5rem; padding-bottom: 1rem; }
  }
"#;

/// `<style>` block(s) for the live preview client: base styling plus the
/// (self-contained) KaTeX stylesheet, since a live doc may gain math at any edit.
pub fn client_styles() -> String {
    format!("<style>{BASE_CSS}</style>\n<style>{KATEX_CSS}</style>")
}

// highlight.js + mermaid (pinned) served from jsDelivr — the dev server runs
// locally with network access, like reveal.js. Both are client-side presentation
// layers, so they never affect the block model or the diff. mermaid is loaded
// lazily (only when a diagram is actually present).
const HLJS: &str = "https://cdn.jsdelivr.net/npm/@highlightjs/cdn-assets@11.11.1";
const MERMAID: &str = "https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.min.js";

/// `<head>` stylesheet link for code syntax highlighting (the highlight.js theme).
pub fn code_head() -> String {
    format!("<link rel=\"stylesheet\" href=\"{HLJS}/styles/github.min.css\" />")
}

/// Scripts that load highlight.js and define `window.qmdEnhanceCode(root)`,
/// which syntax-highlights every language-tagged `<pre><code>` under `root`,
/// gives each code block a copy button, and renders any `<pre class="mermaid">`
/// diagrams (lazy-loading mermaid.js on first use). Callers invoke it after
/// (re)mounting content; it is idempotent (skips already-processed blocks).
pub fn code_scripts() -> String {
    let js = CODE_ENHANCE_JS.replace("{{MERMAID}}", MERMAID);
    format!("<script src=\"{HLJS}/highlight.min.js\"></script>\n<script>{js}</script>")
}

const CODE_ENHANCE_JS: &str = r#"
window.qmdEnhanceCode = function (root) {
  if (!root) return;
  root.querySelectorAll('pre > code').forEach(function (code) {
    var pre = code.parentElement;
    if (pre.dataset.enhanced) return;
    pre.dataset.enhanced = '1';
    if (window.hljs && /language-/.test(code.className)) {
      try { window.hljs.highlightElement(code); } catch (e) {}
    }
    var btn = document.createElement('button');
    btn.className = 'qmd-copy';
    btn.type = 'button';
    btn.setAttribute('aria-label', 'Copy code');
    btn.textContent = 'Copy';
    btn.addEventListener('click', function () {
      navigator.clipboard.writeText(code.innerText).then(function () {
        btn.textContent = 'Copied';
        btn.classList.add('qmd-copied');
        setTimeout(function () { btn.textContent = 'Copy'; btn.classList.remove('qmd-copied'); }, 1200);
      });
    });
    pre.appendChild(btn);
  });
  qmdRenderMermaid(root);
};

function qmdRenderMermaid(root) {
  var pending = root.querySelectorAll('pre.mermaid:not([data-processed])');
  if (!pending.length) return;
  if (window.mermaid) {
    try { window.mermaid.run({ nodes: pending }); } catch (e) {}
    return;
  }
  if (window.__qmdMermaidLoading) return; // its onload will sweep the whole doc
  window.__qmdMermaidLoading = true;
  var s = document.createElement('script');
  s.src = '{{MERMAID}}';
  s.onload = function () {
    try {
      window.mermaid.initialize({ startOnLoad: false });
      window.mermaid.run({ nodes: document.querySelectorAll('pre.mermaid:not([data-processed])') });
    } catch (e) {}
  };
  document.head.appendChild(s);
}
"#;

fn page_from_doc(doc: &RenderedDoc, fallback_title: &str) -> String {
    match doc.format {
        DocFormat::Reveal => reveal_page_from_doc(doc, fallback_title),
        DocFormat::Html => html_page_from_doc(doc, fallback_title),
    }
}

fn html_page_from_doc(doc: &RenderedDoc, fallback_title: &str) -> String {
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
    // With `toc: true`, lay the content beside a sticky table of contents.
    let toc = if doc.toc { toc_html(&doc.blocks) } else { String::new() };
    // Content first (left, wide column), TOC second (right, sticky column).
    let (body_class, body_content) = if toc.is_empty() {
        (String::new(), body)
    } else {
        (" class=\"has-toc\"".to_string(), format!("<main>\n{body}</main>\n{toc}\n"))
    };
    PAGE_TEMPLATE
        .replace("{{TITLE}}", &t)
        .replace("{{KATEX_CSS}}", &katex_css)
        .replace("{{BASE_CSS}}", BASE_CSS)
        .replace("{{CODE_HEAD}}", &code_head())
        .replace("{{BODY_CLASS}}", &body_class)
        .replace("{{BODY}}", &body_content)
        .replace("{{CODE_SCRIPTS}}", &code_scripts())
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
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n\
         <meta charset=\"utf-8\" />\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no\" />\n\
         <title>{t}</title>\n{links}{katex_css}<style>{REVEAL_EXTRA_CSS}</style>\n{code_head}\n\
         </head>\n<body>\n<div class=\"reveal\">\n<div class=\"slides\">\n{slides}</div>\n</div>\n\
         {script}\n<script>\n  Reveal.initialize({{ hash: true, slideNumber: 'c/t', center: false }});\n</script>\n\
         {code_scripts}\n\
         <script>document.addEventListener('DOMContentLoaded',function(){{window.qmdEnhanceCode&&window.qmdEnhanceCode(document.body);}});</script>\n\
         </body>\n</html>\n",
        links = reveal_stylesheet_links(),
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
    Stack { lead: SlideBuf, children: Vec<SlideBuf> },
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
            cur = Some(SlideBuf { level: 0, from_rule: true, id: None, blocks: Vec::new() });
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
    let base = if base.is_empty() { "section".to_string() } else { base };
    let n = counts.entry(base.clone()).or_insert(0);
    let slug = if *n == 0 { base.clone() } else { format!("{base}-{n}") };
    *n += 1;
    slug
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
    Some(FigureParts { url, caption, attrs })
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
        NodeValue::CodeBlock(cb) => {
            let lang = code_lang(&cb.info);
            // Quarto cells (```{lang}) carry leading `#| key: val` option lines; drop them.
            let is_cell = cb.info.trim_start().starts_with('{');
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
                out.push_str(&format!("<pre{attrs}><code{class}>"));
                escape_html(&literal, out);
                out.push_str("</code></pre>");
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
    if injectable
        && let Some(gt) = literal.find('>')
    {
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
        Some(Fence::Open(format!(".{}", rest.split_whitespace().next().unwrap_or(""))))
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
                    spans.push(DivSpan { open, close: i + 1, attrs });
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
        self.kv.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    fn callout_kind(&self) -> Option<&str> {
        self.classes
            .iter()
            .find_map(|c| c.strip_prefix("callout-"))
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
            attrs.kv.push((k.to_string(), v.trim_matches(['"', '\'']).to_string()));
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

    let push_block = |stack: &mut Vec<Open>, result: &mut Vec<Block>, b: Block| {
        match stack.last_mut() {
            Some(top) => top.inner.push(b),
            None => result.push(b),
        }
    };

    for (i, fb) in flat.iter().enumerate() {
        // Open every span that starts before this block and contains it.
        while span_idx < spans.len()
            && spans[span_idx].open < fb.buf_start
            && spans[span_idx].close > fb.buf_start
        {
            stack.push(Open { span: &spans[span_idx], inner: Vec::new() });
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
        format!(
            "<div class=\"callout callout-{kind}\"{data}><div class=\"callout-title\">{title}</div><div class=\"callout-body\">{body}</div></div>"
        )
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

    Block { id, sourcepos, source_file: file, html, cell: None }
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

fn html_escape(s: &str) -> String {
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

/// Drop leading Quarto cell-option lines (`#|` for most languages, `//|` for OJS/JS).
fn strip_cell_options(literal: &str) -> String {
    let mut body = String::new();
    let mut skipping = true;
    for line in literal.lines() {
        let t = line.trim_start();
        if skipping && (t.starts_with("#|") || t.starts_with("//|")) {
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
/// plain ` ```rust ` -> "rust".
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
    (!lang.is_empty()).then(|| lang.to_string())
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

const PAGE_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{{TITLE}}</title>
{{KATEX_CSS}}
<style>{{BASE_CSS}}</style>
{{CODE_HEAD}}
</head>
<body{{BODY_CLASS}}>
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
        assert_ne!(doc.blocks[0].id, doc.blocks[1].id, "duplicate content must get a tiebreak");
        let again = render_document("Para.\n\nPara.\n");
        assert_eq!(doc.blocks[0].id, again.blocks[0].id, "ids must be stable across runs");
    }

    #[test]
    fn front_matter_title_extracted_and_not_a_block() {
        let doc = render_document("---\ntitle: \"My Post\"\nfoo: bar\n---\n\nBody.\n");
        assert_eq!(doc.title.as_deref(), Some("My Post"));
        assert_eq!(doc.blocks.len(), 1);
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
        assert!(h.contains("text-align: right"), "alignment from |--:| missing: {h}");
    }

    #[test]
    fn callout_wraps_content_using_leading_heading_as_title() {
        let doc = render_document("::: {.callout-note}\n## My Note\n\nBody text.\n:::\n");
        assert_eq!(doc.blocks.len(), 1, "the callout is one container block");
        let h = &doc.blocks[0].html;
        assert!(h.contains("class=\"callout callout-note\""), "got: {h}");
        assert!(h.contains("<div class=\"callout-title\">My Note</div>"), "got: {h}");
        assert!(!doc.body_html().contains(":::"));
        // inner content keeps its own sourcepos so click-to-source still works.
        assert!(h.contains("<p data-block-id"), "inner block lost its id: {h}");
        assert!(h.contains("Body text."));
    }

    #[test]
    fn callout_uses_explicit_title_and_default_title() {
        let titled = render_document("::: {.callout-tip title=\"Pro tip\"}\nDo this.\n:::\n");
        assert!(titled.blocks[0].html.contains("callout-tip"));
        assert!(titled.blocks[0].html.contains(">Pro tip</div>"), "got: {}", titled.blocks[0].html);

        let bare = render_document("::: {.callout-warning}\nBe careful.\n:::\n");
        assert!(bare.blocks[0].html.contains(">Warning</div>"), "got: {}", bare.blocks[0].html);
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
        for src in ["```{mermaid}\nflowchart LR\n  A --> B\n```\n", "```mermaid\nflowchart LR\n  A --> B\n```\n"] {
            let doc = render_document(src);
            let h = &doc.blocks[0].html;
            assert!(h.contains("<pre data-block-id"), "got: {h}");
            assert!(h.contains("class=\"mermaid\""), "got: {h}");
            assert!(!h.contains("<code"), "mermaid must not wrap a <code> element: {h}");
            assert!(h.contains("flowchart LR"), "got: {h}");
            assert!(h.contains("A --&gt; B"), "diagram source should be escaped: {h}");
        }
    }

    #[test]
    fn cell_option_lines_are_dropped() {
        let doc = render_document("```{python}\n#| echo: false\n#| label: fig\nprint(1)\n```\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("print(1)"));
        assert!(!h.contains("#| echo"), "option lines should be stripped: {h}");

        // OJS/JS cells use `//|` option syntax
        let ojs = render_document("```{ojs}\n//| echo: false\nx = 1\n```\n");
        assert!(ojs.blocks[0].html.contains("x = 1"));
        assert!(!ojs.blocks[0].html.contains("//| echo"), "got: {}", ojs.blocks[0].html);
    }

    #[test]
    fn dollar_math_is_rendered_by_katex() {
        let doc = render_document("The value $x^2$ is positive.\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("katex"), "expected katex markup, got: {h}");
        assert!(!h.contains("$x^2$"), "raw dollar math should be consumed: {h}");
    }

    #[test]
    fn display_math_block_renders() {
        let doc = render_document("$$\n\\sum_{i=1}^n x_i\n$$\n");
        assert!(doc.body_html().contains("katex-display"), "got: {}", doc.body_html());
    }

    #[test]
    fn bare_latex_environment_renders_as_display_math() {
        let doc = render_document("\\begin{align*}\na &= b \\\\\nc &= d\n\\end{align*}\n");
        assert_eq!(doc.blocks.len(), 1);
        let h = &doc.blocks[0].html;
        // rendered as a display-math block (the raw TeX only survives inside
        // KaTeX's <annotation>, which is expected).
        assert!(h.contains("qmd-math-block"), "got: {h}");
        assert!(h.contains("katex-display"), "expected display math, got: {h}");
    }

    #[test]
    fn html_block_attrs_injected_into_leading_tag() {
        let doc = render_document("<div class=\"demo\">\nhi\n</div>\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("<div class=\"demo\" data-block-id="), "got: {h}");
        // the wrapper-div double-emit bug must not reappear
        assert!(!h.contains("<div data-block-id"), "should inject, not wrap: {h}");
    }

    // --- edge cases / robustness ---

    #[test]
    fn empty_and_whitespace_inputs_do_not_panic() {
        assert!(render_document("").blocks.is_empty());
        assert!(render_document("   \n\n\t\n").blocks.is_empty());
    }

    #[test]
    fn front_matter_only_yields_no_blocks() {
        let doc = render_document("---\ntitle: Only Meta\n---\n");
        assert_eq!(doc.title.as_deref(), Some("Only Meta"));
        assert!(doc.blocks.is_empty());
    }

    #[test]
    fn nested_lists_render_with_nesting() {
        let doc = render_document("- a\n    - b\n    - c\n- d\n");
        let h = &doc.blocks[0].html;
        assert!(h.starts_with("<ul "), "got: {h}");
        assert!(h.contains("<li>a<ul><li>b</li><li>c</li></ul></li>"), "got: {h}");
    }

    #[test]
    fn ordered_list_start_attribute_preserved() {
        let doc = render_document("3. third\n4. fourth\n");
        assert!(doc.blocks[0].html.starts_with("<ol "));
        assert!(doc.blocks[0].html.contains("start=\"3\""), "got: {}", doc.blocks[0].html);
    }

    #[test]
    fn links_images_and_blockquotes_render() {
        let link = render_document("[text](https://example.com \"t\")\n");
        assert!(link.blocks[0].html.contains("<a href=\"https://example.com\" title=\"t\">text</a>"));

        let img = render_document("![alt text](/img.png)\n");
        assert!(img.blocks[0].html.contains("<img src=\"/img.png\" alt=\"alt text\" />"));

        let quote = render_document("> quoted line\n");
        assert!(quote.blocks[0].html.starts_with("<blockquote "));
        assert!(quote.blocks[0].html.contains("quoted line"));
    }

    #[test]
    fn attribute_values_are_escaped() {
        let doc = render_document("[x](https://e.com?a=1&b=\"2\")\n");
        let h = &doc.blocks[0].html;
        assert!(h.contains("&amp;"), "ampersand should be escaped in href: {h}");
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
        let deck = render_document("---\nformat:\n  liquid-glass-revealjs:\n    slide-number: true\n---\n\n## A\n");
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
        assert!(slides.contains("<h1 class=\"title\">Deck</h1>"), "got: {slides}");
        assert!(slides.contains("<p class=\"subtitle\">A subtitle</p>"), "got: {slides}");
        // One <section> per h2, id slugged from the heading text.
        assert!(slides.contains("<section id=\"first\" class=\"slide level2\">"), "got: {slides}");
        assert!(slides.contains("<section id=\"second\" class=\"slide level2\">"), "got: {slides}");
        // Heading keeps its block id inside the section (block-swap/click-to-source).
        assert!(slides.contains("<h2 data-block-id="), "heading lost its block id: {slides}");
        // title + two content slides, no nesting.
        assert_eq!(slides.matches("<section").count(), 3, "got: {slides}");
    }

    #[test]
    fn thematic_break_starts_a_new_slide_and_is_not_emitted() {
        let doc = render_document("---\nformat: revealjs\n---\n\nOne.\n\n---\n\nTwo.\n");
        let slides = slides_html(None, None, &doc.blocks);
        assert!(!slides.contains("<hr"), "the --- delimiter must not render: {slides}");
        assert_eq!(slides.matches("<section").count(), 2, "got: {slides}");
    }

    #[test]
    fn h1_wraps_following_h2s_in_a_vertical_stack() {
        let doc = render_document("---\nformat: revealjs\n---\n\n# Part One\n\nIntro.\n\n## A\n\n## B\n");
        let slides = slides_html(None, None, &doc.blocks);
        // Outer wrapper section, then the h1 lead slide, then the two h2 children.
        assert!(slides.contains("<section>\n<section id=\"part-one\" class=\"slide level1\">"), "got: {slides}");
        assert!(slides.contains("<section id=\"a\" class=\"slide level2\">"), "got: {slides}");
        assert!(slides.contains("<section id=\"b\" class=\"slide level2\">"), "got: {slides}");
        // 1 wrapper + lead + 2 children = 4 sections.
        assert_eq!(slides.matches("<section").count(), 4, "got: {slides}");
    }

    #[test]
    fn reveal_page_carries_revealjs_scaffolding() {
        let page = render_html_page("---\ntitle: D\nformat: revealjs\n---\n\n## Slide\n", "fallback");
        assert!(page.contains("class=\"reveal\""));
        assert!(page.contains("class=\"slides\""));
        assert!(page.contains("reveal.js@5.1.0"));
        assert!(page.contains("Reveal.initialize("));
    }

    // --- books: heading anchors, figures, toc ---

    #[test]
    fn headings_get_deduped_anchor_ids() {
        let doc = render_document("# Intro\n\nbody\n\n# Intro\n");
        assert!(doc.blocks[0].html.starts_with("<h1 id=\"intro\""), "got: {}", doc.blocks[0].html);
        // a repeated heading slug is deduped with a -N suffix.
        let last = doc.blocks.last().unwrap();
        assert!(last.html.contains("id=\"intro-1\""), "got: {}", last.html);
    }

    #[test]
    fn reveal_headings_have_no_id_to_avoid_duplicating_section_ids() {
        // In a deck the slug lives on the wrapping <section>, so the heading must
        // not also carry it (that would be a duplicate id in the DOM).
        let doc = render_document("---\nformat: revealjs\n---\n\n## A Slide\n");
        let h = doc.blocks.iter().find(|b| b.html.starts_with("<h2")).unwrap();
        assert!(!h.html.contains(" id=\""), "reveal heading should not carry an id: {}", h.html);
    }

    #[test]
    fn standalone_image_becomes_a_numbered_figure() {
        let doc = render_document("![Scree plot](scree.png){#fig-scree width=50% fig-align=\"center\"}\n");
        let h = &doc.blocks[0].html;
        assert!(h.starts_with("<figure"), "got: {h}");
        assert!(h.contains("id=\"fig-scree\""), "got: {h}");
        assert!(h.contains("class=\"qmd-figure qmd-figure-center\""), "got: {h}");
        assert!(h.contains("<img src=\"scree.png\""), "got: {h}");
        assert!(h.contains("style=\"width:50%\""), "got: {h}");
        assert!(h.contains("<figcaption>Figure&nbsp;1: Scree plot</figcaption>"), "got: {h}");
        assert!(!h.contains("{#fig-"), "the attribute block leaked: {h}");
        // the figure still carries the block model attributes.
        assert!(h.contains("data-block-id=") && h.contains("data-sourcepos="), "got: {h}");
    }

    #[test]
    fn inline_image_in_a_sentence_stays_inline() {
        let doc = render_document("See ![logo](l.png) for the mark.\n");
        let h = &doc.blocks[0].html;
        assert!(h.starts_with("<p "), "got: {h}");
        assert!(h.contains("<img src=\"l.png\""), "got: {h}");
        assert!(!h.contains("<figure"), "a non-standalone image must not become a figure: {h}");
    }

    #[test]
    fn toc_page_lists_headings_with_anchor_links() {
        let page = render_html_page(
            "---\ntitle: Doc\nformat:\n  html:\n    toc: true\n---\n\n# A\n\ntext\n\n## B\n",
            "fb",
        );
        assert!(page.contains("id=\"TOC\""), "missing TOC nav");
        assert!(page.contains("<body class=\"has-toc\">"), "missing toc layout class");
        assert!(page.contains("<a href=\"#a\">A</a>"), "missing TOC entry for A: {page}");
        assert!(page.contains("<a href=\"#b\">B</a>"), "missing nested TOC entry for B");
    }

    #[test]
    fn no_toc_when_not_requested() {
        let page = render_html_page("---\ntitle: Doc\n---\n\n# A\n", "fb");
        // (the `#TOC`/`has-toc` CSS rules are always present; assert on markup.)
        assert!(!page.contains("<nav id=\"TOC\""), "TOC nav should be absent without toc: true");
        assert!(!page.contains("<body class=\"has-toc\">"), "toc layout should be off");
    }
}
