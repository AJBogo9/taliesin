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

/// The pieces a caller supplies to [`assemble_html_page`]. Everything that
/// differs between the three HTML shells (the static build, the single-doc live
/// preview, and the multi-page site preview) lives here; the page skeleton +
/// `<head>` ordering live once in [`assemble_html_page`]. The empty defaults
/// (`extra_head`/`scripts_*` = `""`) reproduce the static build; the dev servers
/// fill those slots with their live machinery (dev menu, websocket client).
pub struct PageParts<'a> {
    /// Already HTML-escaped `<title>` text.
    pub title: &'a str,
    /// BCP-47 language tag for `<html lang>` (e.g. `en`); callers default to `en`.
    pub lang: &'a str,
    /// A pre-built `<link rel="icon" …>` (inlined data URI, a path, or a route).
    pub favicon: &'a str,
    /// `light`/`dark`/… default for the no-flash theme bootstrap script.
    pub theme_default: &'a str,
    /// Extension / site theme CSS, raw (wrapped in `<style>` only when non-empty).
    pub theme_css: &'a str,
    /// Also ship the multi-page site chrome CSS (navbar / footer / prev-next).
    pub with_site_css: bool,
    /// Ship the KaTeX stylesheet. The static build sets this only when the page
    /// has math; a live preview always sets it (a doc can gain math any edit).
    pub ship_katex: bool,
    /// Ship the Observable runtime (`<head>`) + its init script (after the body).
    pub has_ojs: bool,
    /// Preview-only `<head>` additions (the dev-menu CSS); `""` for the build.
    /// Non-empty values should end with a newline.
    pub extra_head: &'a str,
    /// A `<body>` attribute string including its leading space (e.g. ` class="…"`),
    /// or `""`.
    pub body_class: &'a str,
    pub include_in_header: &'a str,
    pub include_before_body: &'a str,
    /// The body region: chrome + content (build/site) or the live `#qmd-root`.
    pub body: &'a str,
    /// Scripts emitted *before* the shared enhancer registry (the static
    /// click-to-source logger, or the live `window.QMD_*` globals).
    pub scripts_pre: &'a str,
    /// Scripts emitted *after* it (the static `qmdEnhanceCode` call + TOC scripts,
    /// or the live websocket client).
    pub scripts_post: &'a str,
    pub include_after_body: &'a str,
}

/// Assemble a complete HTML page from its parts: the single source of truth for
/// the page skeleton (`<!DOCTYPE>`, the `<head>` ordering, the body frame, the
/// shared enhancer/OJS scripts) shared by the static build and both live-preview
/// servers. Keeping it here means a new meta tag, a bundled stylesheet, or a head
/// reordering happens once instead of in three hand-rolled templates.
pub fn assemble_html_page(p: &PageParts) -> String {
    let katex = if p.ship_katex {
        format!("\n<style>{KATEX_CSS}</style>")
    } else {
        String::new()
    };
    let site_css = if p.with_site_css { SITE_CSS } else { "" };
    let ojs_head_html = if p.has_ojs { ojs_head() } else { String::new() };
    let ojs_init_html = if p.has_ojs {
        format!("{}\n", ojs_init())
    } else {
        String::new()
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{title}</title>
{favicon}
{theme_init}
<style>{base}{dark}{site}</style>{katex}
{ojs_head}
{theme_css}
{include_in_header}
{extra_head}</head>
<body{body_class}>
{include_before_body}
{body}
{scripts_pre}
{code_scripts}
{scripts_post}
{ojs_init}{include_after_body}
</body>
</html>
"#,
        lang = escape_attr(p.lang),
        title = p.title,
        favicon = p.favicon,
        theme_init = theme_head(p.theme_default),
        base = BASE_CSS,
        dark = DARK_CSS,
        site = site_css,
        ojs_head = ojs_head_html,
        theme_css = theme_style(p.theme_css),
        include_in_header = p.include_in_header,
        extra_head = p.extra_head,
        body_class = p.body_class,
        include_before_body = p.include_before_body,
        body = p.body,
        scripts_pre = p.scripts_pre,
        code_scripts = code_scripts(),
        scripts_post = p.scripts_post,
        ojs_init = ojs_init_html,
        include_after_body = p.include_after_body,
    )
}

/// Static-page click-to-source: clicking a block logs its id + sourcepos to the
/// console (a no-server preview of click-to-source; the live server replaces this
/// with the editor wiring in `client.js`).
const STATIC_CLICK_TO_SOURCE: &str = r#"<script>
  // Click any block to see its source position in the console (a static preview
  // of click-to-source; the live server wires this to the editor).
  document.addEventListener('click', (e) => {
    const el = e.target.closest('[data-block-id]');
    document.querySelectorAll('.qmd-hl').forEach(n => n.classList.remove('qmd-hl'));
    if (!el) return;
    el.classList.add('qmd-hl');
    console.log('block', el.dataset.blockId, '@', el.dataset.sourcepos);
  });
</script>"#;

/// Run the client enhancers once on load (the static page has no websocket client
/// to call them after a mount).
const STATIC_ENHANCE: &str = "<script>document.addEventListener('DOMContentLoaded',function(){window.qmdEnhanceCode&&window.qmdEnhanceCode(document.body);});</script>";

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
    // Only ship the (large) KaTeX stylesheet when the page actually has math, and
    // the Observable runtime only when it has live cells (computed before `body`
    // is moved into the content layout below).
    let ship_katex = body.contains("class=\"katex");
    let has_ojs = has_ojs(&body);
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
    assemble_html_page(&PageParts {
        title: &t,
        lang: doc.lang.as_deref().unwrap_or("en"),
        favicon: &favicon,
        theme_default: &doc.theme_default,
        theme_css: &doc.theme_css,
        with_site_css: site.is_some(),
        ship_katex,
        has_ojs,
        extra_head: "",
        body_class: &body_class,
        include_in_header: &includes.in_header,
        include_before_body: &includes.before_body,
        body: &body_content,
        // The static page has no websocket client, so it logs click-to-source to
        // the console and runs the enhancers once on load itself.
        scripts_pre: STATIC_CLICK_TO_SOURCE,
        scripts_post: &format!("{STATIC_ENHANCE}\n{toc_script}"),
        include_after_body: &includes.after_body,
    })
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
