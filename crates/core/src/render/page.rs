//! Full HTML-page assembly: turn a RenderedDoc into a standalone page (the
//! PAGE_TEMPLATE shell, site-chrome wiring, favicon). Split out of mod.rs;
//! `use super::*` reaches the render pipeline + the bundled-asset accessors.

use super::*;

pub(crate) fn page_from_doc(doc: &RenderedDoc, fallback_title: &str, mode: OutputMode) -> String {
    match doc.format {
        // A deck assembles its own page (deck.js + native `.tali-deck`); `mode` still
        // gates the enhancer scripts (e.g. inlining Mermaid offline for Build) so a
        // built deck keeps the same offline contract as an HTML page. `--bare` on a
        // deck is refused at the CLI.
        DocFormat::Reveal => deck::deck_page_from_doc(doc, fallback_title, mode),
        DocFormat::Html => html_page_from_doc(doc, fallback_title, mode),
    }
}

/// Render an already-built [`RenderedDoc`] into a standalone HTML page (no site
/// chrome). Lets the `build` CLI run code cells first and then emit the page from
/// the executed blocks; the in-process [`render_html_page`] path stays unchanged.
/// `mode` decides how much optional machinery ships (see [`OutputMode`]).
pub fn render_doc_to_page(doc: &RenderedDoc, fallback_title: &str, mode: OutputMode) -> String {
    page_from_doc(doc, fallback_title, mode)
}

/// Shared chrome for a page rendered inside a multi-page site: pre-built navbar,
/// footer, and post prev/next HTML. Built by `taliesin_core::site` and injected
/// around the page body. Empty fields render nothing.
#[derive(Debug, Clone, Default)]
pub struct SiteCtx {
    pub navbar_html: String,
    pub footer_html: String,
    pub post_nav_html: String,
    /// A book's chapter chrome — the sticky `.tali-book-topbar` + the off-canvas chapter
    /// drawer (Some only for a book project); when set, the page uses the centred book
    /// reading column instead of the website layout (navbar on top). (Field name kept for
    /// stability; it no longer holds a left sidebar.)
    pub book_sidebar: Option<String>,
    /// `page-layout: full` — widen the content column (for listing indexes).
    pub wide: bool,
    /// Site-level `format: html:` includes (header/body/css from `_site.yml`),
    /// merged ahead of each page's own front-matter includes.
    pub includes: PageIncludes,
    /// Site `favicon:` resolved to a path relative to this page's depth (empty if
    /// none configured), emitted as `<link rel="icon">`.
    pub favicon: String,
    /// JS that sets `window.TALIESIN_SEARCH_INDEX` (+ `TALIESIN_SITE_ROOT`/`TALIESIN_PAGE_URL`)
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
    /// How the page is emitted: live `Preview` (ship everything), static `Build`
    /// (content-gate enhancers), or `Bare` (zero `<script>`, CSS-only theming). The
    /// live servers set `Preview`; the build CLI threads `Build`/`Bare`.
    pub mode: OutputMode,
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
    /// Preview-only `<head>` additions (the dev-menu CSS); `""` for the build.
    /// Non-empty values should end with a newline.
    pub extra_head: &'a str,
    /// A `<body>` attribute string including its leading space (e.g. ` class="…"`),
    /// or `""`.
    pub body_class: &'a str,
    pub include_in_header: &'a str,
    pub include_before_body: &'a str,
    /// The body region: chrome + content (build/site) or the live `#tali-root`.
    pub body: &'a str,
    /// Scripts emitted *before* the shared enhancer registry (the static
    /// click-to-source logger, or the live `window.QMD_*` globals).
    pub scripts_pre: &'a str,
    /// Scripts emitted *after* it (the static `taliEnhanceCode` call + TOC scripts,
    /// or the live websocket client).
    pub scripts_post: &'a str,
    pub include_after_body: &'a str,
}

/// Assemble a complete HTML page from its parts: the single source of truth for
/// the page skeleton (`<!DOCTYPE>`, the `<head>` ordering, the body frame, the
/// shared enhancer scripts) shared by the static build and both live-preview
/// servers. Keeping it here means a new meta tag, a bundled stylesheet, or a head
/// reordering happens once instead of in three hand-rolled templates.
pub fn assemble_html_page(p: &PageParts) -> String {
    let bare = p.mode == OutputMode::Bare;
    let katex = if p.ship_katex {
        format!("\n<style>{KATEX_CSS}</style>")
    } else {
        String::new()
    };
    let site_css = if p.with_site_css { SITE_CSS } else { "" };
    // Bare output carries no `[data-theme]` script, so the JS-keyed dark layer never
    // matches: drop it from the main sheet and append CSS-only theming instead.
    let dark = if bare { "" } else { DARK_CSS };
    let bare_theme = if bare {
        bare_theme_css(p.theme_default)
    } else {
        String::new()
    };
    // The pre-paint theme bootstrap is JS; bare output is script-free.
    let theme_init = if bare {
        String::new()
    } else {
        theme_head(p.theme_default)
    };
    // Native `{js}` cells need the vendored d3 + Plot libs in <head>; the enhancer
    // itself rides in code_scripts(). Gated on the rendered body (no PageParts flag).
    // Bare drops `{js}` entirely (its script blocks are stripped from the body too).
    let js_head_html = if !bare && has_js_cells(p.body) {
        js_cell_head()
    } else {
        String::new()
    };
    // Bare's guarantee is zero `<script>`: suppress every script source — the passed-in
    // pre/post scripts, the enhancer bundle, and (above) the theme bootstrap + js head.
    let scripts_pre = if bare { "" } else { p.scripts_pre };
    let scripts_post = if bare { "" } else { p.scripts_post };
    let code_scripts = code_scripts_for(p.body, p.mode);
    // Skip-to-content link: the first focusable thing in the body, so a keyboard /
    // screen-reader user can jump past the chrome to the reading region. Emitted
    // server-side (works with JS off) whenever the body carries the focusable
    // `<main id="tali-main">` (build + site pages always do; the live `#tali-root`
    // mount does not — the runtime `taliInitSkipLink` synthesizes the pair there).
    // Bare output is link-only chrome but keeps the skip link (it's pure HTML/CSS).
    let skip_link = if p.body.contains("id=\"tali-main\"") {
        "<a class=\"tali-skip\" href=\"#tali-main\">Skip to content</a>\n"
    } else {
        ""
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<meta name="referrer" content="no-referrer" />
<title>{title}</title>
{favicon}
{theme_init}
<style>{base}{dark}{site}{bare_theme}</style>{katex}
{js_head}
{theme_css}
{include_in_header}
{extra_head}</head>
<body{body_class}>
{skip_link}{include_before_body}
{body}
{scripts_pre}
{code_scripts}
{scripts_post}
{include_after_body}
</body>
</html>
"#,
        lang = escape_attr(p.lang),
        title = p.title,
        favicon = p.favicon,
        theme_init = theme_init,
        base = BASE_CSS,
        dark = dark,
        site = site_css,
        bare_theme = bare_theme,
        js_head = js_head_html,
        theme_css = theme_style(p.theme_css),
        include_in_header = p.include_in_header,
        extra_head = p.extra_head,
        body_class = p.body_class,
        include_before_body = p.include_before_body,
        body = p.body,
        scripts_pre = scripts_pre,
        code_scripts = code_scripts,
        scripts_post = scripts_post,
        include_after_body = p.include_after_body,
    )
}

/// CSS-only theming for `--bare` output (no `[data-theme]` script). `dark.css` is
/// uniformly `html[data-theme="dark"]`-prefixed, so rewriting that prefix to `:root`
/// yields a flat dark layer: emitted unconditionally for a forced dark theme, wrapped
/// in a `prefers-color-scheme: dark` media query for an unforced (`auto`) theme so it
/// follows the OS. A forced light theme needs nothing (base.css `:root` is light).
fn bare_theme_css(default_mode: &str) -> String {
    let dark = DARK_CSS.replace("html[data-theme=\"dark\"]", ":root");
    match default_mode {
        "dark" => dark,
        "light" => String::new(),
        _ => format!("@media (prefers-color-scheme: dark){{{dark}}}"),
    }
}

/// Run the client enhancers once on load (the static page has no websocket client
/// to call them after a mount).
const STATIC_ENHANCE: &str = "<script>document.addEventListener('DOMContentLoaded',function(){window.taliEnhanceCode&&window.taliEnhanceCode(document.body);});</script>";

/// Mobile pull-up-sheet chrome for a static TOC page: a dim backdrop + a grabber handle
/// (with a current-section chip). Body-level and `position: fixed`, revealed by CSS only
/// at the sheet breakpoint (`<= 60rem`); `toc-sheet.js` wires the drag/tap/keyboard.
const TOC_SHEET_MARKUP: &str = "<div id=\"tali-toc-backdrop\"></div>\n\
     <button id=\"tali-toc-handle\" type=\"button\" aria-label=\"Contents\">\
     <span id=\"tali-toc-cur\"></span><span class=\"tali-toc-grip\"></span></button>\n";

fn html_page_from_doc(doc: &RenderedDoc, fallback_title: &str, mode: OutputMode) -> String {
    html_page_inner(doc, fallback_title, None, mode)
}

/// Like `html_page_from_doc`, but wraps the page body in the site chrome
/// (navbar above, prev/next + footer below) and ships the site CSS. The
/// single-page path (`html_page_from_doc`) is unchanged (`site == None`).
///
/// Every caller is a static-build context (the `build` CLI, the 404 page, mounted
/// sub-site serving, `check`'s discard); the live site preview assembles its own
/// `PageParts` directly. So this content-gates enhancers like any other build.
/// `--bare` is single-doc only, so a site never reaches `OutputMode::Bare`.
pub fn html_page_from_doc_in_site(
    doc: &RenderedDoc,
    fallback_title: &str,
    site: &SiteCtx,
) -> String {
    html_page_inner(doc, fallback_title, Some(site), OutputMode::Build)
}

fn html_page_inner(
    doc: &RenderedDoc,
    fallback_title: &str,
    site: Option<&SiteCtx>,
    mode: OutputMode,
) -> String {
    let title = doc.title.as_deref().unwrap_or(fallback_title);
    let mut t = String::new();
    escape_html(title, &mut t);
    let body = doc.body_html();
    // Bare output must contain zero `<script>`; a `{js}` cell's runtime payload is a
    // `<script type="application/qmd-js">` in the body, so strip those (the cell is
    // inert without its browser runtime; the build warns separately).
    let body = if mode == OutputMode::Bare {
        strip_qmd_js_scripts(&body)
    } else {
        body
    };
    // Only ship the (large) KaTeX stylesheet when the page actually has math
    // (computed before `body` is moved into the content layout below).
    let ship_katex = body.contains("class=\"katex");
    // With `toc: true`, lay the content beside a sticky table of contents. Name the
    // TOC landmark so a screen reader's landmark list distinguishes it from the other
    // `<nav>`s (navbar / post-nav). `toc_html` already gives it `role="doc-toc"`; the
    // accessible name is added here (its builder lives in mod.rs).
    let toc = if doc.toc {
        toc_html(&doc.blocks).replacen(
            "<nav id=\"TOC\"",
            "<nav id=\"TOC\" aria-label=\"Table of contents\"",
            1,
        )
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
    // The reading region is always a focusable `<main id="tali-main">`, emitted
    // server-side so the skip-to-content link (added in `assemble_html_page`) and
    // keyboard "skip the chrome" work with JS off. `tabindex="-1"` lets the skip link
    // move focus into it without making it a tab stop. The runtime `taliInitSkipLink`
    // no-ops when this server markup is present (it only synthesizes the pair on the
    // live `#tali-root` mount, which has no `<main>`).
    // Content first (left, wide column), TOC second (right, sticky column).
    let (mut body_class, content) = if toc.is_empty() {
        (
            String::new(),
            format!("<main id=\"tali-main\" tabindex=\"-1\">\n{body}</main>\n"),
        )
    } else {
        (
            " class=\"has-toc\"".to_string(),
            format!("<main id=\"tali-main\" tabindex=\"-1\">\n{body}</main>\n{toc}\n"),
        )
    };
    // Site mode: body becomes a full-width flex column (navbar, a centred content
    // wrapper, footer) so the footer sits at the bottom of short pages and the
    // chrome lines up with the reading column. The `has-toc` grid moves onto the
    // wrapper, leaving the body free to be the flex shell.
    let mut body_content = match site {
        // Book: a centred reading column (content + optional TOC) under a sticky topbar;
        // the chapter list is an off-canvas drawer, with prev/next-chapter under the column.
        Some(s) if s.book_sidebar.is_some() => {
            body_class = " class=\"tali-book-body\"".to_string();
            let main_cls = if toc.is_empty() {
                "tali-book-main"
            } else {
                "tali-book-main has-toc"
            };
            let inner_cls = if toc.is_empty() {
                "tali-book-inner"
            } else {
                "tali-book-inner has-toc"
            };
            // `chrome` = the sticky topbar + the off-canvas chapter drawer; the reading
            // content centres in `.tali-book-main` (the same ~70ch measure as a blog post),
            // widening to the content+TOC grid only when the chapter carries a TOC.
            format!(
                "{chrome}\n<div class=\"{main_cls}\">\n\
                 <div class=\"{inner_cls}\">\n{content}</div>\n{post_nav}</div>\n{footer}\n",
                chrome = s.book_sidebar.as_deref().unwrap_or(""),
                post_nav = s.post_nav_html,
                footer = s.footer_html,
            )
        }
        Some(s) => {
            let mut main_cls = String::from("tali-site-main");
            if !toc.is_empty() {
                main_cls.push_str(" has-toc");
            }
            if s.wide {
                main_cls.push_str(" tali-wide");
            }
            body_class = " class=\"tali-site\"".to_string();
            format!(
                "{nav}\n<div class=\"{main_cls}\">\n{content}{post_nav}</div>\n{footer}\n",
                nav = s.navbar_html,
                post_nav = s.post_nav_html,
                footer = s.footer_html,
            )
        }
        None => content,
    };
    // On a TOC page, ship the mobile pull-up-sheet chrome so the "on this page" TOC can
    // become a bottom sheet on narrow screens instead of stranding at the very bottom of
    // the chapter. Progressive enhancement: the handle/backdrop are hidden by default and
    // `toc-sheet.js` ADDS `tali-toc-sheet` to the body at runtime, then wires the drag/tap
    // — so with JS off the TOC still degrades to the in-flow layout (never off-screen and
    // unreachable). Desktop is unaffected (the sheet CSS lives in the `<= 60rem` query).
    if !toc.is_empty() {
        body_content.push_str(TOC_SHEET_MARKUP);
    }
    // Site-level `format: html:` includes (from `_site.yml`) apply to every page
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
        // to the bundled taliesin mark so the tab has an icon and no /favicon.ico 404.
        _ => default_favicon(),
    };
    assemble_html_page(&PageParts {
        mode,
        title: &t,
        lang: doc.lang.as_deref().unwrap_or("en"),
        favicon: &favicon,
        theme_default: &doc.theme_default,
        theme_css: &doc.theme_css,
        with_site_css: site.is_some(),
        ship_katex,
        extra_head: "",
        body_class: &body_class,
        include_in_header: &includes.in_header,
        include_before_body: &includes.before_body,
        body: &body_content,
        // A static page is a read-only view with no editor bridge, so it ships no
        // click-to-source handler (that would draw a dead `.tali-hl` outline on every
        // click); it only runs the enhancers once on load. Click-to-source is a
        // live-preview-only feature (client.js wires it to the editor).
        scripts_pre: "",
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

/// The bundled taliesin mark (the block-model glyph), inlined as a base64 SVG data
/// URI — the default favicon when a project configures none.
const FAVICON_SVG: &str = include_str!("../../../../web-client/favicon.svg");

fn default_favicon() -> String {
    format!(
        "<link rel=\"icon\" type=\"image/svg+xml\" href=\"data:image/svg+xml;base64,{}\" />",
        base64_encode(FAVICON_SVG.as_bytes())
    )
}

/// Remove `<script type="application/qmd-js">…</script>` blocks (a `{js}` cell's
/// runtime payload) from a rendered body, leaving the empty output container behind.
/// Used only for `--bare` output, whose contract is zero `<script>`; a `{js}` cell is
/// inert without its browser runtime. The author source escapes any `</script` to
/// `<\/script` (see `emit_js_cell`), so the first `</script>` after the opening tag
/// is always the real terminator.
fn strip_qmd_js_scripts(body: &str) -> String {
    const OPEN: &str = "<script type=\"application/qmd-js\"";
    const CLOSE: &str = "</script>";
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        match after.find(CLOSE) {
            Some(end) => rest = &after[end + CLOSE.len()..],
            None => {
                // Unterminated (shouldn't happen given the escaping): keep the rest.
                out.push_str(after);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}
