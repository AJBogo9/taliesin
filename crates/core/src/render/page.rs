//! Full HTML-page assembly: turn a RenderedDoc into a standalone page (the
//! PAGE_TEMPLATE shell, site-chrome wiring, favicon). Split out of mod.rs;
//! `use super::*` reaches the render pipeline + the bundled-asset accessors.

use super::*;

pub(crate) fn page_from_doc(doc: &RenderedDoc, fallback_title: &str) -> String {
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
    /// A book's left chapter sidebar (Some only for `project: type: book`); when
    /// set, the page uses the book layout (sidebar | content | TOC) instead of the
    /// website layout (navbar on top).
    pub book_sidebar: Option<String>,
    /// `page-layout: full` — widen the content column (for listing indexes).
    pub wide: bool,
    /// Site-level `format: html:` includes (header/body/css from `_quarto.yml`),
    /// merged ahead of each page's own front-matter includes.
    pub includes: PageIncludes,
    /// Site `favicon:` resolved to a path relative to this page's depth (empty if
    /// none configured), emitted as `<link rel="icon">`.
    pub favicon: String,
    /// JS that sets `window.QMD_SEARCH_INDEX` (+ `QMD_SITE_ROOT`/`QMD_PAGE_URL`)
    /// for the whole-project Cmd-K search; empty for a single doc. Injected next to
    /// the search script on TOC pages.
    pub search_index: String,
}

fn html_page_from_doc(doc: &RenderedDoc, fallback_title: &str) -> String {
    html_page_inner(doc, fallback_title, None)
}

/// Like `html_page_from_doc`, but wraps the page body in the site chrome
/// (navbar above, prev/next + footer below) and ships the site CSS. The
/// single-page path (`html_page_from_doc`) is unchanged (`site == None`).
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
    // The scrollspy + search scripts ride along only on pages with a TOC. In a
    // site/book, prepend the cross-page search index so Cmd-K searches everything.
    let toc_script = if toc.is_empty() {
        String::new()
    } else {
        match site
            .map(|s| s.search_index.as_str())
            .filter(|s| !s.is_empty())
        {
            Some(idx) => format!("<script>{idx}</script>\n{}", toc_scripts()),
            None => toc_scripts(),
        }
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
        // Book: a left chapter sidebar beside the reading area (content + TOC),
        // with prev/next-chapter navigation under it.
        Some(s) if s.book_sidebar.is_some() => {
            body_class = " class=\"qmd-book-body\"".to_string();
            let inner_cls = if toc.is_empty() {
                "qmd-book-inner"
            } else {
                "qmd-book-inner has-toc"
            };
            format!(
                "<div class=\"qmd-book\">\n{sidebar}\n<div class=\"qmd-book-main\">\n\
                 <div class=\"{inner_cls}\">\n{content}</div>\n{post_nav}</div>\n</div>\n{footer}\n",
                sidebar = s.book_sidebar.as_deref().unwrap_or(""),
                post_nav = s.post_nav_html,
                footer = s.footer_html,
            )
        }
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
    let mut includes = match site {
        Some(s) => {
            let mut merged = s.includes.clone();
            merged.merge(&doc.includes);
            merged
        }
        None => doc.includes.clone(),
    };
    // A standalone doc has no site chrome, so emit its OpenGraph/SEO meta here from
    // its own front matter. Site pages already carry richer per-page meta via the
    // chrome includes, so this only runs off-site (`site` is `None`).
    if site.is_none() {
        includes.in_header.push_str(&social_meta_head(
            doc.title.as_deref(),
            doc.description.as_deref(),
        ));
    }
    let favicon = match site {
        Some(s) if !s.favicon.is_empty() => favicon_link(&s.favicon),
        // No configured favicon (a book, or any project that sets none): fall back
        // to the bundled qmd-fast mark so the tab has an icon and no /favicon.ico 404.
        _ => default_favicon(),
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

/// The bundled qmd-fast mark (the block-model glyph), inlined as a base64 SVG data
/// URI — the default favicon when a project configures none.
const FAVICON_SVG: &str = include_str!("../../../../web-client/favicon.svg");

fn default_favicon() -> String {
    format!(
        "<link rel=\"icon\" type=\"image/svg+xml\" href=\"data:image/svg+xml;base64,{}\" />",
        base64_encode(FAVICON_SVG.as_bytes())
    )
}
