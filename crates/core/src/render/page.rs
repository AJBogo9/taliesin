//! Full HTML-page assembly: turn a RenderedDoc into a standalone page (the
//! PAGE_TEMPLATE shell, site-chrome wiring, favicon). Split out of mod.rs;
//! `use super::*` reaches the render pipeline + the bundled-asset accessors.

use super::*;

pub(crate) fn page_from_doc(doc: &RenderedDoc, fallback_title: &str, mode: OutputMode) -> String {
    html_page_from_doc(doc, fallback_title, mode)
}

/// Render an already-built [`RenderedDoc`] into a standalone HTML page (no site
/// chrome). Lets the `build` CLI run code cells first and then emit the page from
/// the executed blocks; the in-process [`render_html_page`] path stays unchanged.
/// `mode` decides how much optional machinery ships (see [`OutputMode`]).
pub fn render_doc_to_page(doc: &RenderedDoc, fallback_title: &str, mode: OutputMode) -> String {
    page_from_doc(doc, fallback_title, mode)
}

/// Like [`render_doc_to_page`] but links the shared `_assets/` files instead of inlining the
/// framework. For a chrome-less page emitted *inside* a multi-page build — today only
/// `404.html`, which is not one of the site's pages and so never passes through
/// [`html_page_from_doc_in_site_external`].
///
/// The caller owns the href form. Every other page in a build gets depth-relative hrefs;
/// this one must be handed **root-absolute** ones, because a static host serves it for any
/// unknown path at any depth and a `../` prefix would resolve against the wrong directory.
pub fn render_doc_to_page_external(
    doc: &RenderedDoc,
    fallback_title: &str,
    assets: ExternalAssets,
) -> String {
    html_page_inner(
        doc,
        fallback_title,
        None,
        OutputMode::Build,
        AssetMode::External(assets),
    )
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
    /// The site's own name (`_site.yml` `title:`), for the `<title>` suffix; `""` if
    /// unset. Every inner page's `<title>` becomes "{page} · {site}" so a browser tab /
    /// search result names both — see [`title_with_site_suffix`].
    pub site_name: String,
    /// This page is the site's root index (`index.html`); its `<title>` stays the bare
    /// site name (no " · {site}" suffix).
    pub is_home: bool,
}

/// Apply the site-name `<title>` suffix policy: an inner page becomes "{title} · {site}"
/// so each browser tab / search result names both the page and the site. The home (root
/// index) and any page already titled exactly the site name stay bare — never "Name ·
/// Name", never a suffix on an empty title or a standalone (no-site) doc. `title` is the
/// already-resolved page `<title>`; the returned string is still unescaped. Shared by the
/// static build (`html_page_inner`) and the live site preview so both tabs agree.
pub fn title_with_site_suffix(title: &str, site_name: &str, is_home: bool) -> String {
    let name = site_name.trim();
    if name.is_empty() || is_home || title.is_empty() || title == name {
        title.to_string()
    } else {
        format!("{title} · {name}")
    }
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
    /// click-to-source logger, or the live `window.TALIESIN_*` globals).
    pub scripts_pre: &'a str,
    /// Scripts emitted *after* it (the static `taliEnhanceCode` call + TOC scripts,
    /// or the live websocket client).
    pub scripts_post: &'a str,
    pub include_after_body: &'a str,
    /// How framework CSS/JS is delivered (inline blobs, or links to `_assets/`).
    pub assets: AssetMode<'a>,
}

impl<'a> PageParts<'a> {
    /// Every field at a safe default so a construction site sets only what it varies and
    /// ends with `..PageParts::defaults()`. Adding a new `PageParts` field is then a
    /// one-line edit here instead of at every hand-rolled call site (the three live/build
    /// assemblers used to drift, which once shipped a title-consistency bug).
    pub fn defaults() -> PageParts<'a> {
        PageParts {
            mode: OutputMode::Build,
            title: "",
            lang: "en",
            favicon: "",
            theme_default: "",
            theme_css: "",
            with_site_css: false,
            ship_katex: false,
            extra_head: "",
            body_class: "",
            include_in_header: "",
            include_before_body: "",
            body: "",
            scripts_pre: "",
            scripts_post: "",
            include_after_body: "",
            assets: AssetMode::Inline,
        }
    }
}

/// Assemble a complete HTML page from its parts: the single source of truth for
/// the page skeleton (`<!DOCTYPE>`, the `<head>` ordering, the body frame, the
/// shared enhancer scripts) shared by the static build and both live-preview
/// servers. Keeping it here means a new meta tag, a bundled stylesheet, or a head
/// reordering happens once instead of in three hand-rolled templates.
pub fn assemble_html_page(p: &PageParts) -> String {
    let bare = p.mode == OutputMode::Bare;
    // The pre-paint theme bootstrap is JS; bare output is script-free.
    let theme_init = if bare {
        String::new()
    } else {
        theme_head(p.theme_default)
    };
    // Bare's guarantee is zero `<script>`: suppress every script source — the passed-in
    // pre/post scripts, the enhancer bundle, and (above) the theme bootstrap + js head.
    let scripts_pre = if bare { "" } else { p.scripts_pre };
    let scripts_post = if bare { "" } else { p.scripts_post };
    // Skip-to-content link: the first focusable thing in the body, so a keyboard /
    // screen-reader user can jump past the chrome to the reading region. Emitted
    // server-side (works with JS off) whenever the body carries the focusable
    // `<main id="tali-main">` (build + site pages always do; the live `#tali-root`
    // mount does not — the runtime `taliInitSkipLink` synthesizes the pair there).
    // Bare output is link-only chrome but keeps the skip link (it's pure HTML/CSS).
    // A second link to the in-page TOC, on pages that have one (AP7-5). The TOC is a
    // sticky sidebar visible the whole time, but it is emitted AFTER the reading column so
    // it lands at tab stop 56 of 62 on a chapter: a keyboard user has to traverse a
    // 10,000 px chapter — every heading anchor and every code copy button — to put focus
    // in a list of links that never left the screen. Screen-reader users already had a way
    // (the `doc-toc` landmark is in the rotor); this is for keyboard-only users who are
    // not running AT, where the skip link is the only mechanism there is. Ordered after
    // "Skip to content" because reading is the common intent.
    let skip_link = match (
        p.body.contains("id=\"tali-main\""),
        p.body.contains("id=\"TOC\""),
    ) {
        (true, true) => {
            "<a class=\"tali-skip\" href=\"#tali-main\">Skip to content</a>\n\
             <a class=\"tali-skip tali-skip-toc\" href=\"#TOC\">Skip to table of contents</a>\n"
        }
        (true, false) => "<a class=\"tali-skip\" href=\"#tali-main\">Skip to content</a>\n",
        _ => "",
    };
    // The head CSS block + framework script tags differ by asset mode; the body frame,
    // skip link, theme bootstrap, and passed-in pre/post scripts are identical.
    // The enhancer registry, emitted in <head> AHEAD of `{include_in_header}`.
    //
    // `01-registry.js` defines `window.taliEnhancers` / `taliEnhanceCode`, and the documented
    // way to ship an extension enhancer is a `<script>` in the project's `_site.yml` `head:`
    // (docs/internals/extending.tmd). That markup lands in `<head>`, so the registry has to be
    // defined by then or the extension's `window.taliEnhancers.register(fn)` throws at parse.
    //
    // It used to be emitted in the body beside `code_scripts`, which was correct while the
    // documented route was the front-matter `include-after-body` (a body slot). That family was
    // retired on 2026-08-02 leaving `head:` as the only route, which runs EARLIER — so the
    // registry moved up to stay ahead of it. `01-registry.js` opens with
    // `if (window.taliEnhancers) return;`, so the later bundled copy (deferred app.js, or the
    // inline bundle) still no-ops exactly as it did before.
    //
    // Bare output ships no scripts at all, so it gets none of this.
    let enhancer_registry = if bare {
        String::new()
    } else {
        format!("<script>{REGISTRY_JS}</script>\n")
    };
    let (style_block, katex_block, js_head_html, framework_scripts) = match &p.assets {
        AssetMode::Inline => {
            let site_css = if p.with_site_css { SITE_CSS } else { "" };
            // Bare output carries no `[data-theme]` script, so the JS-keyed dark layer
            // never matches: drop the dark token override + dark recolours from the main
            // sheet and append flattened CSS-only theming instead.
            let (tokens_dark, dark) = if bare {
                ("", "")
            } else {
                (TOKENS_DARK_CSS, DARK_CSS)
            };
            let bare_theme = if bare {
                bare_theme_css(p.theme_default)
            } else {
                String::new()
            };
            let style_block = format!(
                "<style>{FONTS_CSS}{TOKENS_CSS}{tokens_dark}{BASE_CSS}{dark}{site_css}{bare_theme}</style>"
            );
            let katex_block = if p.ship_katex {
                format!("\n<style>{KATEX_CSS}</style>")
            } else {
                String::new()
            };
            // Native `{js}` cells need the vendored d3 + Plot libs in <head>; the
            // enhancer itself rides in code_scripts(). Gated on the rendered body (no
            // PageParts flag). Bare drops `{js}` entirely (its script blocks are
            // stripped from the body too).
            let js_head_html = if !bare && has_js_cells(p.body) {
                js_cell_head()
            } else {
                String::new()
            };
            let framework_scripts = code_scripts_for(p.body, p.mode);
            (style_block, katex_block, js_head_html, framework_scripts)
        }
        AssetMode::External(a) => {
            // Item 150: the body face is its own file here, so start its fetch beside the
            // stylesheet instead of after it parses. Emitted BEFORE the sheet — a preload
            // that follows the request it is meant to beat buys nothing. `crossorigin` is
            // required even same-origin: fonts are fetched in CORS mode, and a preload
            // whose mode disagrees is fetched twice.
            let font_preload = if a.font_preload.is_empty() {
                String::new()
            } else {
                format!(
                    "<link rel=\"preload\" as=\"font\" type=\"font/woff2\" href=\"{}\" crossorigin>",
                    a.font_preload
                )
            };
            let style_block = format!(
                "{font_preload}<link rel=\"stylesheet\" href=\"{}\">",
                a.app_css
            );
            let katex_block = if p.ship_katex {
                format!("\n<link rel=\"stylesheet\" href=\"{}\">", a.katex_css)
            } else {
                String::new()
            };
            let js_head_html = if !bare && has_js_cells(p.body) {
                format!("<script src=\"{}\" defer></script>", a.jslibs_js)
            } else {
                String::new()
            };
            let mermaid = if has_mermaid(p.body) {
                format!("\n<script src=\"{}\" defer></script>", a.mermaid_js)
            } else {
                String::new()
            };
            // The `{js}`-cell runtime stays INLINE even in External mode, exactly as on the
            // inline path. It runs each cell via `new AsyncFunction(..., src)`, and a dynamic
            // `import()` in that body resolves relative to the SCRIPT that ran the constructor:
            // inlined here it anchors to the page, so a cell's `import("./helper.js")` resolves
            // page-relative; folded into the shared `/_assets/app.js` it would wrongly resolve
            // against `/_assets/` (a 404). That page-relative anchoring is the ONLY reason it is
            // inline: it registers with `window.taliEnhancers` directly at parse (the registry is
            // emitted inline just below, ahead of the deferred app.js, so it already exists by the
            // time this runs), and the deferred jslibs (d3/Plot) have executed by the
            // DOMContentLoaded mount, so the cells still see `window.d3` / `Plot`.
            //
            // Gated on [`has_client_cells`], NOT on `{js}` alone: every registered
            // client-side language runs through this one runtime, so a second language
            // added to the registry needs it just as much. A language with its own
            // enhancer follows it inline (and must, since it would call
            // `window.taliJs.registerLanguage` on the object this script has just defined).
            let tali_js_inline = if !bare && has_client_cells(p.body) {
                format!("\n<script>{TALIESIN_JS}</script>")
            } else {
                String::new()
            };
            // The registry itself is emitted in <head> (see `enhancer_registry` above), so this
            // is just the deferred bundle. app.js stays deferred (non-blocking): when it runs
            // after parse, its own bundled `01-registry` copy hits
            // `if (window.taliEnhancers) return;` and no-ops, while its feature scripts (02-16)
            // register into the already-created list. On DOMContentLoaded, STATIC_ENHANCE calls
            // `taliEnhanceCode` = registry.run, running every registered enhancer (core +
            // tali-js + any extension); the deferred jslibs (d3/Plot) have executed by then, so
            // `{js}` cells still run correctly.
            let framework_scripts = format!(
                "<script src=\"{}\" defer></script>{tali_js_inline}{mermaid}",
                a.app_js
            );
            (style_block, katex_block, js_head_html, framework_scripts)
        }
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
{GENERATOR_BANNER}<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<meta name="referrer" content="no-referrer" />
<meta name="generator" content="Taliesin" />
<title>{title}</title>
{favicon}
{theme_init}
{style_block}{katex_block}
{js_head}
{theme_css}
{enhancer_registry}{include_in_header}
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
        style_block = style_block,
        katex_block = katex_block,
        js_head = js_head_html,
        theme_css = theme_style(p.theme_css),
        enhancer_registry = enhancer_registry,
        include_in_header = p.include_in_header,
        extra_head = p.extra_head,
        body_class = p.body_class,
        include_before_body = p.include_before_body,
        body = p.body,
        scripts_pre = scripts_pre,
        code_scripts = framework_scripts,
        scripts_post = scripts_post,
        include_after_body = p.include_after_body,
    )
}

/// CSS-only theming for `--bare` output (no `[data-theme]` script). The dark layer is
/// the palette override (`tokens-dark.css`) plus the recoloured scopes/boxes (`dark.css`),
/// both uniformly `html[data-theme="dark"]`-prefixed, so rewriting that prefix to `:root`
/// yields a flat dark layer: emitted unconditionally for a forced dark theme, wrapped
/// in a `prefers-color-scheme: dark` media query for an unforced (`auto`) theme so it
/// follows the OS. A forced light theme needs nothing (tokens.css `:root` is light).
fn bare_theme_css(default_mode: &str) -> String {
    let dark = format!("{TOKENS_DARK_CSS}{DARK_CSS}").replace("html[data-theme=\"dark\"]", ":root");
    match default_mode {
        "dark" => dark,
        "light" => String::new(),
        _ => format!("@media (prefers-color-scheme: dark){{{dark}}}"),
    }
}

/// Run the client enhancers once on load (the static page has no websocket client
/// to call them after a mount).
const STATIC_ENHANCE: &str = "<script>document.addEventListener('DOMContentLoaded',function(){window.taliEnhanceCode&&window.taliEnhanceCode(document.body);});</script>";

fn html_page_from_doc(doc: &RenderedDoc, fallback_title: &str, mode: OutputMode) -> String {
    html_page_inner(doc, fallback_title, None, mode, AssetMode::Inline)
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
    html_page_inner(
        doc,
        fallback_title,
        Some(site),
        OutputMode::Build,
        AssetMode::Inline,
    )
}

/// Like [`html_page_from_doc_in_site`] but links the shared `_assets/` files instead of
/// inlining the framework CSS/JS. Used by the multi-page `build <dir>` path.
pub fn html_page_from_doc_in_site_external(
    doc: &RenderedDoc,
    fallback_title: &str,
    site: &SiteCtx,
    assets: ExternalAssets,
) -> String {
    html_page_inner(
        doc,
        fallback_title,
        Some(site),
        OutputMode::Build,
        AssetMode::External(assets),
    )
}

/// Resolve the `<title>` text, in order of how deliberately the author chose it:
///
/// 1. the document's own front-matter `title:`;
/// 2. in a site, the page's authored title (an `_site.yml` `chapters:` `text:` override
///    deliberately differs from the chapter's heading: "Methodology" vs `# Methods`);
/// 3. the document's leading `# H1`, which is what the author called the page;
/// 4. the caller's last resort, which standalone is only the file stem.
///
/// Steps 2 and 3 swap on `in_site`, and that is the whole subtlety: standalone the
/// fallback is a filename, so any heading beats it; in a site the fallback is an authored
/// title, so it beats the heading. Before step 3 existed, a front-matter-less document
/// rendered `<title>the-file-stem</title>` standalone and `<title></title>` in a site,
/// where `og:title` then quietly borrowed the site's own name.
/// `pub(super)` only so the print assembler (`render/print.rs`) resolves a `<title>` through
/// the SAME policy rather than growing a fourth copy of it — the drift this module's
/// `site_page_title` doc comment warns about. Visibility only; no behaviour change.
pub(super) fn resolve_title(doc: &RenderedDoc, fallback_title: &str, in_site: bool) -> String {
    let fallback = (!fallback_title.is_empty()).then_some(fallback_title);
    let h1 = leading_h1_text(&doc.blocks);
    let ranked = if in_site {
        [fallback, h1.as_deref()]
    } else {
        [h1.as_deref(), fallback]
    };
    doc.title
        .as_deref()
        .into_iter()
        .chain(ranked.into_iter().flatten())
        .next()
        .unwrap_or("")
        .to_string()
}

/// The display-ready `<title>` for a page in a site: [`resolve_title`]'s ranking, then the
/// site-name suffix ([`title_with_site_suffix`]). The whole title policy behind one call,
/// because it has three consumers that MUST agree — the static build (`html_page_inner`),
/// the live preview's server-rendered `<title>`, and the `full_render` websocket message,
/// which the client assigns straight to `document.title`.
///
/// They didn't agree. The websocket sent the doc's raw front-matter title, so on arrival it
/// overwrote a correct server-rendered tab with a worse one: `/blog.html` lost its " ·
/// {site}" suffix, and a titleless chapter (no front-matter `title:`, only an `# H1`) went
/// all the way down to the client's own "Taliesin" default. Composing the two halves per
/// caller is what let a caller compose only one of them, so don't: call this.
///
/// Takes `site_name`/`is_home` rather than a [`SiteCtx`] so a caller that only wants a
/// title needn't build the whole page chrome to get one; [`Site::page_title`] is the
/// entry point for those.
pub(crate) fn site_page_title(
    doc: &RenderedDoc,
    fallback_title: &str,
    site_name: &str,
    is_home: bool,
) -> String {
    let resolved = resolve_title(doc, fallback_title, true);
    title_with_site_suffix(&resolved, site_name, is_home)
}

fn html_page_inner(
    doc: &RenderedDoc,
    fallback_title: &str,
    site: Option<&SiteCtx>,
    mode: OutputMode,
    assets: AssetMode,
) -> String {
    // In a site, name the site on every inner tab ("{page} · {site}"); the home + any
    // page already titled the site name stay bare (see `title_with_site_suffix`).
    let resolved = match site {
        Some(s) => site_page_title(doc, fallback_title, &s.site_name, s.is_home),
        None => resolve_title(doc, fallback_title, false),
    };
    let title = resolved.as_str();
    let mut t = String::new();
    escape_html(title, &mut t);
    let body = doc.body_html();
    // Bare output must contain zero `<script>`; a `{js}` cell's runtime payload is a
    // `<script type="application/tali-js">` in the body, so strip those (the cell is
    // inert without its browser runtime; the build warns separately).
    let body = if mode == OutputMode::Bare {
        strip_tali_js_scripts(&body)
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
    // **A book renders no rail** (item 76). Every assembler already routes through
    // `Site::page_toc`, which returns false for a book, so this is belt-and-braces — but
    // the book branch below now emits a one-column grid unconditionally, and a `SiteCtx`
    // assembled some other way with `doc.toc` still set would drop the nav into a layout
    // that has no track for it. Gating at the source keeps that unrepresentable.
    let toc = if doc.toc && !site.is_some_and(|s| s.book_sidebar.is_some()) {
        // `tabindex="-1"`, like `<main>`, so the skip link below can move focus INTO the
        // landmark rather than merely near it (AP7-5). Not a tab stop.
        toc_html(&doc.blocks).replacen(
            "<nav id=\"TOC\"",
            "<nav id=\"TOC\" tabindex=\"-1\" aria-label=\"Table of contents\"",
            1,
        )
    } else {
        String::new()
    };
    // The scrollspy + mobile TOC sheet ride along only on pages that HAVE a TOC.
    let spy_script = if toc.is_empty() {
        String::new()
    } else {
        match &assets {
            AssetMode::Inline => toc_scripts(),
            // External: the shared toc JS is in app.js.
            AssetMode::External(_) => String::new(),
        }
    };
    // Cmd-K is deliberately NOT gated on this page's TOC. Its button is part of the site
    // chrome and renders on every page, so gating the runtime + index on `toc` left any
    // chapter under `MIN_TOC_HEADINGS` advertising a palette that opened with nothing in
    // it — invisible to the author, because the preview injects both unconditionally.
    // Ship it wherever the trigger exists: a site page (chrome button, TOC or not) or a
    // standalone page with a TOC (which is where a bare doc gets its ⌘K hint).
    let search_script = if toc.is_empty() && site.is_none() {
        String::new()
    } else {
        let index = site
            .map(|s| s.search_index.as_str())
            .filter(|s| !s.is_empty())
            .map(|idx| format!("<script>{idx}</script>\n"))
            .unwrap_or_default();
        match &assets {
            // Inline: the per-page index (if any) followed by the palette runtime.
            AssetMode::Inline => format!("{index}{}", search_scripts()),
            // External: the palette runtime is in app.js; keep only the per-page index.
            AssetMode::External(_) => index,
        }
    };
    let toc_script = format!("{search_script}{spy_script}");
    // The reading region is always a focusable `<main id="tali-main">`, emitted
    // server-side so the skip-to-content link (added in `assemble_html_page`) and
    // keyboard "skip the chrome" work with JS off. `tabindex="-1"` lets the skip link
    // move focus into it without making it a tab stop. The runtime `taliInitSkipLink`
    // no-ops when this server markup is present (it only synthesizes the pair on the
    // live `#tali-root` mount, which has no `<main>`).
    // A dated post is a self-contained, syndicatable unit, so its reading content is an
    // `<article>` landmark (PA-M2) — the title block, body, and footnotes are all the article.
    // An undated page (listing / section / generic) stays plain `<main>` content. `<article>`
    // is `display:block` like the content it wraps, and no CSS selector targets `main`'s direct
    // children on an article page (the one such rule is gated to listing pages), so this is a
    // semantic-only change with no layout effect.
    let reading = if doc.is_article {
        format!("<article>\n{body}</article>\n")
    } else {
        body
    };
    // Content first (left, wide column), TOC second (right, sticky column).
    let (mut body_class, content) = if toc.is_empty() {
        (
            String::new(),
            format!("<main id=\"tali-main\" tabindex=\"-1\">\n{reading}</main>\n"),
        )
    } else {
        (
            " class=\"has-toc\"".to_string(),
            format!("<main id=\"tali-main\" tabindex=\"-1\">\n{reading}</main>\n{toc}\n"),
        )
    };
    // Site mode: body becomes a full-width flex column (navbar, a centred content
    // wrapper, footer) so the footer sits at the bottom of short pages and the
    // chrome lines up with the reading column. The `has-toc` grid moves onto the
    // wrapper, leaving the body free to be the flex shell.
    let body_content = match site {
        // Book: a centred reading column (content + optional TOC) under a sticky topbar;
        // the chapter list is an off-canvas drawer, with prev/next-chapter under the column.
        Some(s) if s.book_sidebar.is_some() => {
            body_class = " class=\"tali-book-body\"".to_string();
            // `chrome` = the sticky topbar + the off-canvas chapter drawer; the reading
            // content centres in `.tali-book-main` (the same ~70ch measure as a blog post).
            // One column, always: a book has no right rail (item 76), so there is no
            // content+TOC grid to widen into and no empty track to reserve against it.
            format!(
                "{chrome}\n<div class=\"tali-book-main\">\n\
                 <div class=\"tali-book-inner\">\n{content}</div>\n{post_nav}</div>\n{footer}\n",
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
            doc.is_article,
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
        assets,
        ..PageParts::defaults()
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

pub(super) fn default_favicon() -> String {
    format!(
        "<link rel=\"icon\" type=\"image/svg+xml\" href=\"data:image/svg+xml;base64,{}\" />",
        base64_encode(FAVICON_SVG.as_bytes())
    )
}

/// Remove every client-side cell's `<script type="application/tali-…">…</script>` payload
/// from a rendered body, leaving the empty output container behind. Used only for `--bare`
/// output, whose contract is zero `<script>`; a client-side cell is inert without its
/// browser runtime. The author source escapes any `</script` to `<\/script` (see
/// [`emit_client_cell`]), so the first `</script>` after the opening tag is always the
/// real terminator.
///
/// Driven off the [`client_lang`] registry rather than a literal, so registering a
/// language cannot leave a `<script>` in output whose whole contract is having none.
fn strip_tali_js_scripts(body: &str) -> String {
    CLIENT_LANGS.iter().fold(body.to_string(), |acc, lang| {
        strip_scripts_of_type(&acc, lang.mime)
    })
}

fn strip_scripts_of_type(body: &str, mime: &str) -> String {
    let open = format!("<script type=\"{mime}\"");
    const CLOSE: &str = "</script>";
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find(&open) {
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

#[cfg(test)]
mod tests {
    use super::*;

    // Same base.css marker literal Task 1 confirmed present (see render/tests.rs).
    const MARKER_BASE: &str = ".tali-reader-seg";
    // A literal unique to tali-js.js (the `{js}`-cell runtime); its presence in the page
    // proves the runtime shipped INLINE (in External mode all other framework JS is a
    // `<script src=...>` link, so raw runtime text in the page == an inline `<script>`).
    const MARKER_TALIJS: &str = "tali-js cell error:";

    #[test]
    fn page_parts_defaults_assemble_a_minimal_page() {
        // The churn-killer: a caller sets only what it varies, the rest comes from defaults().
        let html = assemble_html_page(&PageParts {
            title: "MinimalT",
            body: "<p>hello-min</p>",
            ..PageParts::defaults()
        });
        assert!(html.contains("hello-min"), "body must render: {html}");
        assert!(html.contains("MinimalT"), "title must render");
    }

    #[test]
    fn external_inlines_js_cell_runtime_page_relative() {
        let assemble = |body: &str| {
            let ext = ExternalAssets {
                app_css: "_assets/app.aaaa.css",
                katex_css: "_assets/katex.bbbb.css",
                app_js: "_assets/app.cccc.js",
                mermaid_js: "_assets/mermaid.dddd.js",
                jslibs_js: "_assets/jslibs.eeee.js",
                font_preload: "",
            };
            assemble_html_page(&PageParts {
                mode: OutputMode::Build,
                title: "T",
                lang: "en",
                favicon: "",
                theme_default: "dark",
                theme_css: "",
                with_site_css: true,
                ship_katex: false,
                extra_head: "",
                body_class: "",
                include_in_header: "",
                include_before_body: "",
                body,
                scripts_pre: "",
                scripts_post: "",
                include_after_body: "",
                assets: AssetMode::External(ext),
            })
        };
        // A page WITH a `{js}` cell: the runtime ships inline so `new AsyncFunction`'s
        // `import()` anchors to the page (not `/_assets/`), yet app.js + jslibs stay external.
        let js_html = assemble(
            "<main id=\"tali-main\"><script type=\"application/tali-js\">1</script></main>",
        );
        assert!(
            js_html.contains(MARKER_TALIJS),
            "{{js}}-cell runtime must be inlined on a {{js}} page in External mode"
        );
        // The inline runtime is a bare `<script>` (no `src`, no `defer`): the whole
        // `<script>{TALIESIN_JS}</script>` block is present verbatim.
        assert!(
            js_html.contains(&format!("<script>{TALIESIN_JS}</script>")),
            "the runtime must be a bare inline <script>, not src/defer"
        );
        // The external, deferred app.js (the enhancers) is STILL linked alongside it.
        assert!(
            js_html.contains("<script src=\"_assets/app.cccc.js\" defer></script>"),
            "external app.js must still be linked"
        );
        // The heavy d3/Plot libs stay externalized + deferred (they do no relative import()).
        assert!(js_html.contains("src=\"_assets/jslibs.eeee.js\" defer"));

        // A page WITHOUT `{js}` cells does NOT inline the runtime (but still links app.js).
        let prose_html = assemble("<main id=\"tali-main\"><p>prose only</p></main>");
        assert!(
            !prose_html.contains(MARKER_TALIJS),
            "no {{js}}-cell runtime on a {{js}}-free page"
        );
        assert!(
            prose_html.contains("<script src=\"_assets/app.cccc.js\" defer></script>"),
            "app.js is always linked"
        );
    }

    #[test]
    fn external_assets_link_instead_of_inlining() {
        let ext = ExternalAssets {
            app_css: "_assets/app.aaaa.css",
            katex_css: "_assets/katex.bbbb.css",
            app_js: "_assets/app.cccc.js",
            mermaid_js: "_assets/mermaid.dddd.js",
            jslibs_js: "_assets/jslibs.eeee.js",
            font_preload: "",
        };
        let body = "<main id=\"tali-main\"><span class=\"katex\">x</span>\
                    <pre class=\"mermaid\">g</pre>\
                    <script type=\"application/tali-js\">1</script></main>";
        let html = assemble_html_page(&PageParts {
            mode: OutputMode::Build,
            title: "T",
            lang: "en",
            favicon: "",
            theme_default: "dark",
            theme_css: "",
            with_site_css: true,
            ship_katex: true,
            extra_head: "",
            body_class: "",
            include_in_header: "",
            include_before_body: "",
            body,
            scripts_pre: "",
            scripts_post: "",
            include_after_body: "",
            assets: AssetMode::External(ext),
        });
        // Links, not inlined framework CSS.
        assert!(html.contains("<link rel=\"stylesheet\" href=\"_assets/app.aaaa.css\">"));
        assert!(html.contains("href=\"_assets/katex.bbbb.css\""));
        assert!(
            !html.contains(MARKER_BASE),
            "framework CSS must not be inlined in External mode"
        );
        // Scripts as deferred external refs.
        assert!(html.contains("<script src=\"_assets/app.cccc.js\" defer></script>"));
        assert!(html.contains("src=\"_assets/mermaid.dddd.js\" defer"));
        assert!(html.contains("src=\"_assets/jslibs.eeee.js\" defer"));
    }

    #[test]
    fn external_omits_conditional_links_when_absent() {
        let ext = ExternalAssets {
            app_css: "a.css",
            katex_css: "k.css",
            app_js: "a.js",
            mermaid_js: "m.js",
            jslibs_js: "j.js",
            font_preload: "",
        };
        let html = assemble_html_page(&PageParts {
            mode: OutputMode::Build,
            title: "T",
            lang: "en",
            favicon: "",
            theme_default: "dark",
            theme_css: "",
            with_site_css: true,
            ship_katex: false,
            extra_head: "",
            body_class: "",
            include_in_header: "",
            include_before_body: "",
            body: "<main id=\"tali-main\"><p>prose only</p></main>",
            scripts_pre: "",
            scripts_post: "",
            include_after_body: "",
            assets: AssetMode::External(ext),
        });
        assert!(html.contains("href=\"a.css\""), "app.css always linked");
        assert!(html.contains("src=\"a.js\" defer"), "app.js always linked");
        // Note: a bare `"k.css"` substring check would false-positive on the unrelated
        // theme-bootstrap script's own comment mentioning "dark.css" (which contains
        // "k.css"), so this checks the actual href attribute.
        assert!(
            !html.contains("href=\"k.css\""),
            "no katex link on a math-free page"
        );
        assert!(
            !html.contains("m.js"),
            "no mermaid link on a diagram-free page"
        );
        assert!(
            !html.contains("j.js"),
            "no jslibs link on a {{js}}-free page"
        );
    }
}
