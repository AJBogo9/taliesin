//! Multi-page website project model.
//!
//! A *site* is a directory with one explicit root config (`_quarto.yml`, kept for
//! Quarto compatibility) plus a set of `.qmd` input pages. This module owns the
//! project-level concerns that the single-page path never had:
//!
//!   - parsing the root config (navbar / footer / title) into a typed [`SiteConfig`],
//!   - discovering input pages and mapping each to its output URL (`.qmd` → `.html`),
//!   - the page order used for post prev/next navigation,
//!   - building the shared chrome (navbar, footer, prev/next) injected into pages,
//!   - rewriting intra-site `.qmd` links to their built `.html` targets.
//!
//! Per the project's config decision there is **no `_metadata.yml` cascade**: the
//! root config is the single source of project-wide defaults and a page's own
//! front matter overrides it. Both `build` (static) and `serve` (live preview)
//! drive the site through [`Site::discover`] + [`Site::render_page`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::render::{self, Block, SiteCtx, block_heading_level, escape_attr as esc};

/// A single input page and where it lands in the built site.
#[derive(Debug, Clone)]
pub struct Page {
    /// Absolute path to the source `.qmd`.
    pub input: PathBuf,
    /// Path relative to the site root, e.g. `posts/em-algorithm/index.qmd`.
    pub rel: String,
    /// Output URL relative to the site root, e.g. `posts/em-algorithm/index.html`.
    pub url: String,
    /// Front-matter title (for nav labels + prev/next + listing cards).
    pub title: Option<String>,
    /// Front-matter `date` as written (ISO strings sort chronologically).
    pub date: Option<String>,
    /// Front-matter `description` (shown on a listing card).
    pub description: Option<String>,
    /// Front-matter `image`, resolved to a site-root-relative path (for cards).
    pub card_image: Option<String>,
    /// Front-matter `categories` (shown as badges on a card).
    pub categories: Vec<String>,
    /// Whether the page lives under `posts/` (drives prev/next navigation).
    pub is_post: bool,
    /// `listing:` blocks declared on this page (the blog index, projects, etc.).
    pub listings: Vec<ListingSpec>,
    /// `about:` profile block, if this page declares one (the homepage).
    pub about: Option<AboutSpec>,
    /// `page-layout:` (`full` widens the content column; default reading width).
    pub page_layout: Option<String>,
}

/// An `about:` front-matter block: a profile header (image + name + links). The
/// `template` (jolla, trestles, …) is kept as a class for styling; the layout is
/// the centered jolla style the corpus uses.
#[derive(Debug, Clone)]
pub struct AboutSpec {
    pub template: String,
    pub image: Option<String>,
    pub image_alt: Option<String>,
    pub links: Vec<NavItem>,
}

/// A `listing:` front-matter block: a request to render a grid/list of cards for
/// the documents under `contents`.
#[derive(Debug, Clone)]
pub struct ListingSpec {
    /// Optional target id (`listing: { id: x }`) → fills `::: {#x}`; else appended.
    pub id: Option<String>,
    /// The directory whose pages are listed (relative to the hosting page).
    pub contents: String,
    /// `type: grid` → card grid; otherwise a stacked default list.
    pub grid: bool,
    /// Newest-first when true (`sort: "date desc"`, the default).
    pub sort_desc: bool,
    /// `max-items:` cap, if any.
    pub max_items: Option<usize>,
    /// `categories: true` → render a category filter chip row above the cards.
    pub categories: bool,
}

/// A discovered multi-page site: the root config plus its input pages.
#[derive(Debug, Clone)]
pub struct Site {
    pub root: PathBuf,
    pub config: SiteConfig,
    pub pages: Vec<Page>,
    /// Resolved book navigation when `project: type: book`; `None` for a website.
    pub book: Option<Book>,
    /// Project-wide cross-reference targets (`sec-`/`fig-`/… anchor → page + number),
    /// so a `@sec-x` on one page resolves to its section on another (the book case).
    pub xref_targets: HashMap<String, XrefTarget>,
    /// Site-wide `format: html:` includes (header/body/css), resolved once at
    /// discovery relative to the site root and merged ahead of each page's own.
    pub includes: render::PageIncludes,
    /// Warnings gathered during discovery (bad config, etc.), surfaced by the
    /// caller (build logs / preview diagnostics).
    pub warnings: Vec<String>,
    /// Inlinable JSON of every page's title + anchored headings, so the Cmd-K
    /// palette searches the whole project (`window.QMD_SEARCH_INDEX`). Built once
    /// at discovery.
    pub search_index_json: String,
}

mod book;
pub use book::{Book, BookEntry};
use book::{book_pages, build_book};
mod feed;
mod search;
mod xref;
pub use xref::XrefTarget;
use xref::{rewrite_cross_refs, scan_xref_targets};
mod config;
pub use config::*;

impl Site {
    /// Discover the site rooted at `root`: parse `_quarto.yml`, enumerate input
    /// `.qmd` pages, and compute their output URLs + ordering.
    pub fn discover(root: &Path) -> Site {
        let mut warnings = Vec::new();
        let config = load_config(root, &mut warnings);

        // A book takes its page set + order from the explicit `chapters:` list;
        // a website discovers every `.qmd` and orders by path.
        let (pages, book) = if config.is_book {
            let book = build_book(root, &config);
            (book_pages(root, &book), Some(book))
        } else {
            (website_pages(root), None)
        };

        // Resolve the site-wide head/body/css includes once, relative to the site
        // root (where `_quarto.yml` and its referenced css/js files live).
        let includes = render::includes_from_parts(
            config.head.as_ref(),
            config.body_start.as_ref(),
            config.body_end.as_ref(),
            config.css.as_ref(),
            Some(root),
        );

        let xref_targets = scan_xref_targets(&pages, &book);
        let search_index_json = search::build_index_json(&pages);

        Site {
            root: root.to_path_buf(),
            config,
            pages,
            book,
            xref_targets,
            includes,
            warnings,
            search_index_json,
        }
    }

    /// Whether this project is a book (`project: type: book`).
    pub fn is_book(&self) -> bool {
        self.book.is_some()
    }

    /// The site's RSS 2.0 feed (a website with a configured `url:` and at least
    /// one post), or `None`. Written to `feed.xml` by the build and served by the
    /// preview.
    pub fn rss_feed(&self) -> Option<String> {
        feed::rss(self)
    }

    /// Cheap check that a feed will be produced — gates the discovery `<link>` and
    /// the `feed.xml` route without rebuilding the whole feed.
    fn feed_enabled(&self) -> bool {
        !self.is_book()
            && self.config.url.as_deref().is_some_and(|u| !u.is_empty())
            && self.pages.iter().any(|p| p.is_post)
    }

    /// The output directory `build` writes to (default `_site`, or `_book` for a
    /// book, matching Quarto).
    pub fn output_dir(&self) -> &str {
        self.config
            .output_dir
            .as_deref()
            .unwrap_or(if self.is_book() { "_book" } else { "_site" })
    }

    /// Look up a page by its source rel-path or its output URL (`serve` accepts
    /// either an editor path or a browser request).
    pub fn page(&self, rel_or_url: &str) -> Option<&Page> {
        let needle = rel_or_url.trim_start_matches('/');
        self.pages
            .iter()
            .find(|p| p.rel == needle || p.url == needle)
    }

    /// Build the chrome (navbar, footer, post-nav) for a page, with links
    /// resolved relative to that page's depth. Shared by the static build and the
    /// live preview so both render identical navigation.
    pub fn page_chrome(&self, page: &Page) -> SiteCtx {
        let depth = page.url.matches('/').count(); // links are relative to the page
        let favicon = match &self.config.favicon {
            Some(f) if !f.is_empty() => format!("{}{}", "../".repeat(depth), f),
            _ => String::new(),
        };
        let book = self.is_book();
        // Auto-discovery for the RSS feed: a root-relative `<link>` in the head so
        // feed readers (and the browser) find `feed.xml` from any page depth.
        let mut includes = self.includes.clone();
        if self.feed_enabled() {
            let title = self.config.title.as_deref().unwrap_or("RSS");
            includes.in_header.push_str(&format!(
                "\n<link rel=\"alternate\" type=\"application/rss+xml\" title=\"{}\" href=\"{}feed.xml\">",
                crate::escape_attr(title),
                "../".repeat(depth),
            ));
        }
        // The cross-page search index (+ how to resolve a result's page URL from
        // this page's depth). Empty when there are no entries; injected only where
        // the search palette also rides along (TOC pages).
        let search_index = if self.search_index_json.is_empty() || self.search_index_json == "[]" {
            String::new()
        } else {
            format!(
                "window.QMD_SEARCH_INDEX={};window.QMD_SITE_ROOT=\"{}\";window.QMD_PAGE_URL=\"{}\"",
                self.search_index_json,
                "../".repeat(depth),
                page.url
            )
        };
        SiteCtx {
            // A book replaces the top navbar with a left chapter sidebar and uses
            // chapter prev/next instead of the post "back to listing" link.
            navbar_html: if book {
                String::new()
            } else {
                self.navbar_html(page, depth)
            },
            footer_html: self.footer_html(depth),
            post_nav_html: if book {
                self.book_nav_html(page, depth)
            } else {
                self.post_nav_html(page, depth)
            },
            book_sidebar: book.then(|| self.sidebar_html(page, depth)),
            wide: page.page_layout.as_deref() == Some("full"),
            includes,
            favicon,
            search_index,
        }
    }

    /// Render a single page (by rel-path or URL) into a full HTML document with
    /// the site chrome (navbar, footer, prev/next) and intra-site links rewritten
    /// to their `.html` targets. Returns `None` if the page isn't part of the site.
    pub fn render_page(&self, rel_or_url: &str) -> Option<String> {
        let page = self.page(rel_or_url)?;
        let src = std::fs::read_to_string(&page.input).ok()?;
        let base = page.input.parent().unwrap_or(&self.root);
        let doc = render::render_document_with_includes(&src, base);
        Some(self.render_page_doc(page, doc))
    }

    /// Finish a page whose `doc.blocks` are already produced — and possibly
    /// code-executed (the static build runs cells, then calls this): apply the
    /// site front-matter expansion (`about:`/`listing:`), wrap in chrome, and
    /// rewrite intra-site `.qmd` links. Shared by `render_page` (no execution) and
    /// the executing `build` path so both emit identical chrome + links.
    pub fn render_page_doc(&self, page: &Page, mut doc: render::RenderedDoc) -> String {
        doc.toc = self.page_toc(page, doc.toc_explicit);
        self.number_chapter(page, &mut doc.blocks);
        self.resolve_cross_refs(&mut doc.blocks, &page.url);
        self.expand_page(page, &mut doc.blocks);
        let ctx = self.page_chrome(page);
        let fallback = page.title.as_deref().unwrap_or("");
        let html = render::html_page_from_doc_in_site(&doc, fallback, &ctx);
        rewrite_qmd_links(&html)
    }

    /// Whether a page shows a table of contents: its own front-matter `toc:` wins
    /// (an explicit `toc: false` suppresses it even when the site enables TOCs);
    /// otherwise the site-wide `format: html: toc:` applies, but only to article
    /// pages — a listing or about page would otherwise get a TOC built from its card
    /// titles. Used by both the static build and the live preview.
    pub fn page_toc(&self, page: &Page, doc_toc: Option<bool>) -> bool {
        doc_toc.unwrap_or_else(|| {
            self.config.toc.unwrap_or(false) && page.listings.is_empty() && page.about.is_none()
        })
    }

    /// Resolve cross-*page* references in place: a `@sec-x` whose anchor lives on
    /// another page (left marked `data-qmd-xref` by `cite`) is rewritten to link to
    /// that page and carry its number ("Section 2.1"). Same-page refs were already
    /// resolved by `cite`; an anchor unknown project-wide is left as a label link.
    /// Called by both the static build and the live preview.
    pub fn resolve_cross_refs(&self, blocks: &mut [Block], current_url: &str) {
        if self.xref_targets.is_empty() {
            return;
        }
        let up = "../".repeat(current_url.matches('/').count());
        for b in blocks.iter_mut() {
            if b.html.contains("data-qmd-xref=\"") {
                b.html = rewrite_cross_refs(&b.html, &self.xref_targets, current_url, &up);
            }
        }
    }

    /// Number a book chapter's headings in place (chapter N, then N.1, N.1.1 …),
    /// like Quarto's `number-sections`. A no-op for a website or an unnumbered
    /// preface. Called by both the static build and the live preview.
    pub fn number_chapter(&self, page: &Page, blocks: &mut [Block]) {
        if let Some(book) = &self.book
            && let Some(number) = book
                .entries
                .iter()
                .find(|e| e.rel == page.rel)
                .and_then(|e| e.number)
        {
            number_chapter_headings(blocks, number);
        }
    }

    // --- listings ---------------------------------------------------------

    /// Apply this page's site-level front-matter blocks to its rendered `blocks`,
    /// mutating in place: an `about:` profile replaces the title block, and each
    /// `listing:` expands into post cards. Both the static build and the live
    /// preview call this, so the results stay in the block model (mounted + diffed
    /// like any other block).
    pub fn expand_page(&self, page: &Page, blocks: &mut Vec<Block>) {
        if let Some(about) = &page.about {
            let html = self.about_html(page, about);
            match blocks.iter_mut().find(|b| b.id == "qmd-title-block") {
                Some(tb) => tb.html = html,
                None => blocks.insert(
                    0,
                    Block {
                        id: "qmd-title-block".to_string(),
                        sourcepos: String::new(),
                        source_file: None,
                        html,
                        cell: None,
                    },
                ),
            }
        }
        for spec in &page.listings {
            let cards = self.listing_html(page, spec);
            match &spec.id {
                Some(id) => {
                    let needle = format!("id=\"{}\"", id);
                    match blocks.iter().position(|b| b.html.contains(&needle)) {
                        // A `::: {#id}` container → inject the cards inside it.
                        Some(i) if blocks[i].html.contains("</div>") => {
                            let pos = blocks[i].html.rfind("</div>").unwrap();
                            blocks[i].html.insert_str(pos, &cards);
                        }
                        // An anchor (e.g. an auto-slugged heading sharing the id, since
                        // an empty fenced div emits no block) → cards go right after it.
                        Some(i) => blocks.insert(i + 1, listing_block(&spec.contents, &cards)),
                        // No target at all → append so the listing still renders.
                        None => blocks.push(listing_block(&spec.contents, &cards)),
                    }
                }
                None => blocks.push(listing_block(&spec.contents, &cards)),
            }
        }
    }

    /// The pages a listing covers: those under its `contents:` directory (relative
    /// to the hosting page), newest-first (or oldest-first), capped by `max-items`.
    fn collection(&self, host: &Page, spec: &ListingSpec) -> Vec<&Page> {
        let prefix = format!(
            "{}/",
            join_rel(&host.rel, spec.contents.trim_end_matches('/'))
        );
        let mut items: Vec<&Page> = self
            .pages
            .iter()
            .filter(|p| p.rel != host.rel && p.title.is_some() && p.rel.starts_with(&prefix))
            .collect();
        // Order by date (string-ISO sorts chronologically), tiebreak on rel.
        items.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.rel.cmp(&b.rel)));
        if spec.sort_desc {
            items.reverse();
        }
        if let Some(n) = spec.max_items {
            items.truncate(n);
        }
        items
    }

    /// Render a listing's cards. `host` fixes the link/image depth so cards on a
    /// nested page still resolve.
    fn listing_html(&self, host: &Page, spec: &ListingSpec) -> String {
        let up = "../".repeat(host.url.matches('/').count());
        let layout = if spec.grid { "grid" } else { "default" };
        let items = self.collection(host, spec);
        let cards: String = items
            .iter()
            .map(|p| self.card_html(p, &up, spec.grid))
            .collect();
        let grid = format!("<div class=\"qmd-listing qmd-listing-{layout}\">{cards}</div>");

        if !spec.categories {
            return grid;
        }
        // `categories: true` → a filter chip row above the cards: every category
        // across the listing, with a count, sorted (the client enhancer wires the
        // multi-select filtering; an "All" chip clears it).
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for p in &items {
            for c in &p.categories {
                *counts.entry(c.as_str()).or_default() += 1;
            }
        }
        if counts.is_empty() {
            return grid;
        }
        let mut chips = String::from(
            "<button class=\"qmd-cat-chip qmd-cat-active\" type=\"button\" data-cat=\"\">All</button>",
        );
        for (cat, n) in &counts {
            chips.push_str(&format!(
                "<button class=\"qmd-cat-chip\" type=\"button\" data-cat=\"{c}\">{label}\
                 <span class=\"qmd-cat-count\">{n}</span></button>",
                c = esc(cat),
                label = esc(cat),
            ));
        }
        format!(
            "<div class=\"qmd-listing-wrap\">\
             <nav class=\"qmd-cat-filter\" aria-label=\"Filter by category\">{chips}</nav>{grid}</div>"
        )
    }

    fn card_html(&self, p: &Page, up: &str, grid: bool) -> String {
        let href = format!("{up}{}", p.url);
        let img = match (grid, &p.card_image) {
            (true, Some(src)) => format!(
                "<img class=\"qmd-card-img\" src=\"{up}{}\" alt=\"\" loading=\"lazy\">",
                esc(src)
            ),
            _ => String::new(),
        };
        let date = p
            .date
            .as_deref()
            .map(|d| format!("<div class=\"qmd-card-date\">{}</div>", esc(d)))
            .unwrap_or_default();
        let title = esc(p.title.as_deref().unwrap_or(&p.rel));
        let desc = p
            .description
            .as_deref()
            .map(|d| format!("<p class=\"qmd-card-desc\">{}</p>", esc(d)))
            .unwrap_or_default();
        // Each badge carries `data-cat` so a click on it toggles that category in
        // the filter; the card carries `data-categories` for the filter to match.
        let cats = if p.categories.is_empty() {
            String::new()
        } else {
            let badges: String = p
                .categories
                .iter()
                .map(|c| {
                    format!(
                        "<span class=\"qmd-cat\" data-cat=\"{c}\">{c}</span>",
                        c = esc(c)
                    )
                })
                .collect();
            format!("<div class=\"qmd-card-cats\">{badges}</div>")
        };
        let data_cats = if p.categories.is_empty() {
            String::new()
        } else {
            format!(" data-categories=\"{}\"", esc(&p.categories.join(",")))
        };
        // `data-qmd-src` lets the click-to-source locator jump to the post's source
        // (it's site-root-relative; resolved client-side, inert in the static build).
        format!(
            "<a class=\"qmd-card\" href=\"{href}\" data-qmd-src=\"{src}\"{data_cats}>{img}\
             <div class=\"qmd-card-body\">{date}<h3 class=\"qmd-card-title\">{title}</h3>{desc}{cats}</div></a>",
            src = esc(&p.rel)
        )
    }

    // --- about ------------------------------------------------------------

    /// Render an `about:` profile header (replaces the title block on a page that
    /// declares one). Centered jolla-style: round image, name, optional links. The
    /// `image` is relative to the page itself, so it's emitted as-is.
    fn about_html(&self, page: &Page, about: &AboutSpec) -> String {
        let name = page.title.clone().unwrap_or_default();
        let img = about
            .image
            .as_deref()
            .map(|src| {
                let alt = about.image_alt.as_deref().unwrap_or("");
                format!(
                    "<img class=\"qmd-about-img\" src=\"{}\" alt=\"{}\">",
                    esc(src),
                    esc(alt)
                )
            })
            .unwrap_or_default();
        let links = if about.links.is_empty() {
            String::new()
        } else {
            let items: String = about
                .links
                .iter()
                .filter_map(|l| {
                    let href = l.href.as_deref()?;
                    let label = l.text.as_deref().or(l.icon.as_deref()).unwrap_or(href);
                    Some(format!(
                        "<a class=\"qmd-about-link\" href=\"{}\">{}</a>",
                        esc(href),
                        esc(label)
                    ))
                })
                .collect();
            format!("<div class=\"qmd-about-links\">{items}</div>")
        };
        format!(
            "<header class=\"qmd-about qmd-about-{tpl}\" data-block-id=\"qmd-title-block\" data-qmd-src=\"{src}\">\
             {img}<h1 class=\"qmd-about-name\">{name}</h1>{links}</header>",
            tpl = esc(&about.template),
            name = esc(&name),
            src = esc(&page.rel),
        )
    }

    // --- chrome -----------------------------------------------------------

    /// The site navbar: a brand (site title → home) plus the configured left/right
    /// item groups. `depth` is the current page's path depth so links resolve
    /// relative to it (a post two levels deep prefixes `../../`).
    fn navbar_html(&self, current: &Page, depth: usize) -> String {
        let up = "../".repeat(depth);
        let brand_text = self
            .config
            .title
            .clone()
            .unwrap_or_else(|| "Home".to_string());
        let mut s = String::from(
            "<header class=\"qmd-site-nav\" data-qmd-src=\"_quarto.yml\"><nav class=\"qmd-nav-inner\">",
        );
        s.push_str(&format!(
            "<a class=\"qmd-nav-brand\" href=\"{up}index.html\">{}</a>",
            esc(&brand_text)
        ));
        // A hidden checkbox toggles the mobile menu with no JS dependency.
        s.push_str(
            "<input type=\"checkbox\" id=\"qmd-nav-toggle\" class=\"qmd-nav-toggle\" hidden>",
        );
        s.push_str("<label for=\"qmd-nav-toggle\" class=\"qmd-nav-burger\" aria-label=\"Menu\"><span></span><span></span><span></span></label>");
        s.push_str("<div class=\"qmd-nav-links\">");
        for it in &self.config.nav.left {
            s.push_str(&self.nav_link(it, current, &up));
        }
        // Everything after the spacer is pushed to the far right of the bar.
        s.push_str("<span class=\"qmd-nav-spacer\"></span>");
        for it in &self.config.nav.right {
            s.push_str(&self.nav_link(it, current, &up));
        }
        // A real, shipped light/dark toggle (wired by theme_head; works in `build`
        // too). Dev-only tools live in the floating dev menu, not the navbar.
        s.push_str(
            "<button class=\"qmd-theme-toggle\" type=\"button\" data-qmd-theme-toggle \
             aria-label=\"Toggle theme\"></button>",
        );
        s.push_str("</div></nav></header>");
        s
    }

    fn nav_link(&self, it: &NavItem, current: &Page, up: &str) -> String {
        let Some(href) = it.href.as_deref() else {
            return String::new();
        };
        let label = it.text.as_deref().unwrap_or(href);
        let target = resolve_href(href, up);
        // Active when this nav item points at the current page.
        let active = href_matches_page(href, current);
        let cls = if active {
            " class=\"qmd-nav-link qmd-nav-active\" aria-current=\"page\""
        } else {
            " class=\"qmd-nav-link\""
        };
        // `icon:` shorthand renders a bundled SVG; otherwise the (escaped) label.
        let content = it
            .icon
            .as_deref()
            .and_then(social_icon)
            .unwrap_or_else(|| esc(label));
        // `data-label` carries the text so the CSS can reserve the bold (active)
        // width, keeping the navbar from shifting when the active item bolds.
        format!(
            "<a{cls} href=\"{}\" data-label=\"{}\">{}</a>",
            target,
            esc(label),
            content
        )
    }

    /// The slim site footer. Footer item text is treated as raw HTML (icon SVGs),
    /// per the trusted-source model. A configured `.xml` link resolves to the
    /// generated `feed.xml`.
    fn footer_html(&self, depth: usize) -> String {
        let Some(footer) = &self.config.footer else {
            return String::new();
        };
        let up = "../".repeat(depth);
        let feed = self.feed_enabled();
        let group = |items: &[NavItem]| -> String {
            let mut g = String::new();
            for it in items {
                // `icon:` shorthand → a bundled SVG; otherwise the text is raw HTML
                // (the trusted-source model, so an inline `<svg>` still works).
                let (content, aria) = match it.icon.as_deref().and_then(social_icon) {
                    Some(svg) => {
                        let label = it.text.as_deref().or(it.icon.as_deref()).unwrap_or("link");
                        (svg, format!(" aria-label=\"{}\"", esc(label)))
                    }
                    None => (it.text.clone().unwrap_or_default(), String::new()),
                };
                match it.href.as_deref() {
                    // A configured `.xml` link (e.g. Quarto's `blog.xml`) points to
                    // the generated feed.xml — or is dropped when there's no feed.
                    Some(h) if h.ends_with(".xml") => {
                        if !feed {
                            continue;
                        }
                        g.push_str(&format!(
                            "<a class=\"qmd-foot-item\"{aria} href=\"{up}feed.xml\">{content}</a>"
                        ));
                    }
                    Some(h) => {
                        g.push_str(&format!(
                            "<a class=\"qmd-foot-item\"{aria} href=\"{}\">{content}</a>",
                            resolve_href(h, &up)
                        ));
                    }
                    None => g.push_str(&format!("<span class=\"qmd-foot-item\">{content}</span>")),
                }
            }
            g
        };
        format!(
            "<footer class=\"qmd-site-footer\" data-qmd-src=\"_quarto.yml\"><div class=\"qmd-foot-inner\">\
             <div class=\"qmd-foot-left\">{}</div>\
             <div class=\"qmd-foot-center\">{}</div>\
             <div class=\"qmd-foot-right\">{}</div>\
             </div></footer>",
            group(&footer.left),
            group(&footer.center),
            group(&footer.right),
        )
    }
}

/// The bundled social glyphs for the `icon:` shorthand (Bootstrap Icons, inline
/// SVG using `currentColor`), so a footer/nav link is `{ icon: github, href: … }`
/// instead of a raw `<svg>` blob. `None` for an unknown name (the caller falls
/// back to `text`).
fn social_icon(name: &str) -> Option<String> {
    let paths = match name.to_ascii_lowercase().as_str() {
        "github" => {
            "<path d=\"M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8\"/>"
        }
        "linkedin" => {
            "<path d=\"M0 1.146C0 .513.526 0 1.175 0h13.65C15.474 0 16 .513 16 1.146v13.708c0 .633-.526 1.146-1.175 1.146H1.175C.526 16 0 15.487 0 14.854zm4.943 12.248V6.169H2.542v7.225zm-1.2-8.212c.837 0 1.358-.554 1.358-1.248-.015-.709-.52-1.248-1.342-1.248S2.4 3.226 2.4 3.934c0 .694.521 1.248 1.327 1.248zm4.908 8.212V9.359c0-.216.016-.432.08-.586.173-.431.568-.878 1.232-.878.869 0 1.216.662 1.216 1.634v3.865h2.401V9.25c0-2.22-1.184-3.252-2.764-3.252-1.274 0-1.845.7-2.165 1.193v.025h-.016l.016-.025V6.169h-2.4c.03.678 0 7.225 0 7.225z\"/>"
        }
        "rss" => {
            "<path d=\"M14 1a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H2a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1zM2 0a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V2a2 2 0 0 0-2-2z\"/><path d=\"M5.5 12a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0m-3-8.5a1 1 0 0 1 1-1c5.523 0 10 4.477 10 10a1 1 0 1 1-2 0 8 8 0 0 0-8-8 1 1 0 0 1-1-1m0 4a1 1 0 0 1 1-1 6 6 0 0 1 6 6 1 1 0 1 1-2 0 4 4 0 0 0-4-4 1 1 0 0 1-1-1\"/>"
        }
        "x" | "twitter" => {
            "<path d=\"M12.6.75h2.454l-5.36 6.142L16 15.25h-4.937l-3.867-5.07-4.425 5.07H.316l5.733-6.57L0 .75h5.063l3.495 4.633L12.601.75Zm-.86 13.028h1.36L4.323 2.145H2.865z\"/>"
        }
        "mastodon" => {
            "<path d=\"M11.19 12.195c2.016-.24 3.77-1.475 3.99-2.603.348-1.778.32-4.339.32-4.339 0-3.47-2.286-4.488-2.286-4.488C12.062.238 10.083.017 8.027 0h-.05C5.92.017 3.942.238 2.79.765c0 0-2.285 1.017-2.285 4.488l-.002.662c-.004.64-.007 1.35.011 2.091.083 3.394.626 6.74 3.78 7.57 1.454.383 2.703.463 3.709.408 1.823-.1 2.847-.647 2.847-.647l-.06-1.317s-1.303.41-2.767.36c-1.45-.05-2.98-.156-3.215-1.928a3.6 3.6 0 0 1-.033-.496s1.424.346 3.228.428c1.103.05 2.137-.064 3.188-.189zm1.613-2.47H11.13v-4.08c0-.859-.364-1.295-1.091-1.295-.804 0-1.207.517-1.207 1.541v2.233H7.168V5.89c0-1.024-.403-1.541-1.207-1.541-.727 0-1.091.436-1.091 1.296v4.079H3.197V5.522c0-.859.22-1.541.66-2.046.456-.505 1.052-.764 1.793-.764.856 0 1.504.328 1.933.983L8 4.39l.417-.695c.429-.655 1.077-.983 1.934-.983.74 0 1.336.259 1.791.764.442.505.661 1.187.661 2.046z\"/>"
        }
        "bluesky" => {
            "<path d=\"M3.468 1.948C5.303 3.325 7.276 6.117 8 7.615c.725-1.498 2.697-4.29 4.532-5.667C13.855.956 16 .186 16 2.632c0 .489-.28 4.105-.444 4.692-.572 2.04-2.653 2.561-4.504 2.246 3.236.551 4.06 2.375 2.281 4.2-3.376 3.464-4.852-.87-5.23-1.98-.07-.204-.103-.3-.103-.218 0-.082-.033.014-.102.218-.379 1.11-1.855 5.444-5.231 1.98-1.778-1.825-.955-3.65 2.28-4.2-1.85.315-3.932-.205-4.503-2.246C.28 6.737 0 3.12 0 2.632 0 .186 2.145.955 3.468 1.948\"/>"
        }
        "email" | "mail" => {
            "<path d=\"M.05 3.555A2 2 0 0 1 2 2h12a2 2 0 0 1 1.95 1.555L8 8.414zM0 4.697v7.104l5.803-3.558zM6.761 8.83l-6.57 4.027A2 2 0 0 0 2 14h12a2 2 0 0 0 1.808-1.144l-6.57-4.027L8 9.586zm3.436-.586L16 11.801V4.697z\"/>"
        }
        _ => return None,
    };
    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" fill=\"currentColor\" viewBox=\"0 0 16 16\" aria-hidden=\"true\">{paths}</svg>"
    ))
}

impl Site {
    /// Prev/next navigation between posts (chronological). Non-posts get nothing.
    /// Bottom-of-post navigation: a single "back to the listing" button (replaces
    /// prev/next). Links to the listing page that covers this post, preferring the
    /// most complete one (the full blog over a homepage "recent posts" excerpt).
    fn post_nav_html(&self, current: &Page, depth: usize) -> String {
        if !current.is_post {
            return String::new();
        }
        // The listing page covering this post with the largest collection.
        let mut best: Option<(&Page, usize)> = None;
        for page in &self.pages {
            if page.rel == current.rel {
                continue;
            }
            for spec in &page.listings {
                let coll = self.collection(page, spec);
                if coll.iter().any(|p| p.rel == current.rel)
                    && best.is_none_or(|(_, n)| coll.len() > n)
                {
                    best = Some((page, coll.len()));
                }
            }
        }
        let Some((blog, _)) = best else {
            return String::new();
        };
        let up = "../".repeat(depth);
        let target = format!("{up}{}", blog.url);
        let label = blog.title.as_deref().unwrap_or("Blog");
        format!(
            "<nav class=\"qmd-postnav\"><a class=\"qmd-back-link\" href=\"{target}\">\
             <span class=\"qmd-back-glyph\">\u{2190}</span> Back to {}</a></nav>",
            esc(label)
        )
    }

    /// The book's left sidebar: the title, then the ordered chapters (part
    /// headers interspersed), each prefixed with its number, the current chapter
    /// highlighted.
    fn sidebar_html(&self, current: &Page, depth: usize) -> String {
        let Some(book) = &self.book else {
            return String::new();
        };
        let up = "../".repeat(depth);
        let mut s = String::from("<nav class=\"qmd-book-sidebar\" data-qmd-src=\"_quarto.yml\">");
        // Sidebar header: book title (links home) + a light/dark toggle. A book has
        // no top navbar, so without this the toggle (wired by theme_head) has no home.
        s.push_str("<div class=\"qmd-book-sidebar-head\">");
        if let Some(t) = &book.title {
            s.push_str(&format!(
                "<a class=\"qmd-book-brand\" href=\"{up}index.html\">{}</a>",
                esc(t)
            ));
        }
        s.push_str(
            "<button class=\"qmd-theme-toggle\" type=\"button\" data-qmd-theme-toggle \
             aria-label=\"Toggle light/dark theme\"></button>",
        );
        s.push_str("</div>");
        s.push_str("<ul class=\"qmd-book-chapters\">");
        for e in &book.entries {
            if let Some(part) = &e.part {
                s.push_str(&format!("<li class=\"qmd-book-part\">{}</li>", esc(part)));
                continue;
            }
            let active = e.rel == current.rel;
            let cls = if active {
                "qmd-book-chapter qmd-book-active"
            } else {
                "qmd-book-chapter"
            };
            let aria = if active { " aria-current=\"page\"" } else { "" };
            let num = e
                .number
                .map(|n| format!("<span class=\"qmd-chap-num\">{n}</span> "))
                .unwrap_or_default();
            s.push_str(&format!(
                "<li><a class=\"{cls}\" href=\"{up}{}\"{aria}>{num}{}</a></li>",
                e.url,
                esc(&e.title)
            ));
        }
        s.push_str("</ul></nav>");
        s
    }

    /// Bottom-of-chapter prev/next navigation between book chapters.
    fn book_nav_html(&self, current: &Page, depth: usize) -> String {
        let Some(book) = &self.book else {
            return String::new();
        };
        let chapters = book.chapters();
        let Some(idx) = chapters.iter().position(|c| c.rel == current.rel) else {
            return String::new();
        };
        let up = "../".repeat(depth);
        let label = |e: &BookEntry| match e.number {
            Some(n) => format!("{n}  {}", esc(&e.title)),
            None => esc(&e.title),
        };
        let prev = idx.checked_sub(1).and_then(|i| chapters.get(i)).copied();
        let next = chapters.get(idx + 1).copied();
        if prev.is_none() && next.is_none() {
            return String::new();
        }
        let left = prev
            .map(|p| {
                format!(
                    "<a class=\"qmd-book-prev\" href=\"{up}{}\">\
                     <span class=\"qmd-back-glyph\">\u{2190}</span> {}</a>",
                    p.url,
                    label(p)
                )
            })
            .unwrap_or_default();
        let right = next
            .map(|n| {
                format!(
                    "<a class=\"qmd-book-next\" href=\"{up}{}\">{} \
                     <span class=\"qmd-fwd-glyph\">\u{2192}</span></a>",
                    n.url,
                    label(n)
                )
            })
            .unwrap_or_default();
        format!(
            "<nav class=\"qmd-postnav qmd-book-postnav\">{left}\
             <span class=\"qmd-nav-spacer\"></span>{right}</nav>"
        )
    }
}

/// Prefix each heading in a book chapter with its section number: the chapter's
/// `# H1` becomes "N", and the deeper headings count within it ("N.1", "N.1.1"),
/// emitted as a `header-section-number` span so it reads like Quarto.
fn number_chapter_headings(blocks: &mut [Block], chapter: u32) {
    let mut counters = [0u32; 5]; // counters[0] = h2, [1] = h3, …
    for b in blocks.iter_mut() {
        if let Some(level) = heading_level(&b.html) {
            let number = section_number(chapter, level, &mut counters);
            b.html = prefix_heading_number(&b.html, &number);
        }
    }
}

/// The number for a heading at `level` within chapter `chapter`: the chapter's H1
/// is "N", a level-`k` heading is "N.c2.…ck", with `counters` (h2..h6) carried and
/// reset on a shallower heading. Shared by the render-time numbering and the
/// source scan that builds the cross-reference registry, so they never diverge.
fn section_number(chapter: u32, level: usize, counters: &mut [u32; 5]) -> String {
    if level <= 1 {
        return chapter.to_string();
    }
    let i = (level - 2).min(counters.len() - 1);
    counters[i] += 1;
    for c in &mut counters[i + 1..] {
        *c = 0;
    }
    let mut parts = vec![chapter.to_string()];
    parts.extend(counters[..=i].iter().map(u32::to_string));
    parts.join(".")
}

/// The heading level (1–6) of a block whose root element is `<hN …>`, else `None`.
/// Delegates to the render crate's parser so the two never diverge.
fn heading_level(html: &str) -> Option<usize> {
    block_heading_level(html).map(usize::from)
}

/// Insert a `header-section-number` span just after a heading's opening tag.
fn prefix_heading_number(html: &str, number: &str) -> String {
    match html.find('>') {
        Some(i) => format!(
            "{}<span class=\"header-section-number\">{number}</span> {}",
            &html[..=i],
            &html[i + 1..]
        ),
        None => html.to_string(),
    }
}

/// A website's pages: every `.qmd` under `root` (path-ordered), each mapped to a
/// [`Page`] from its front matter.
fn website_pages(root: &Path) -> Vec<Page> {
    let mut inputs = Vec::new();
    collect_pages(root, &mut inputs);
    inputs.sort();
    let mut pages: Vec<Page> = inputs
        .into_iter()
        .map(|input| {
            let rel = rel_str(root, &input);
            let url = qmd_to_html(&rel);
            let fm = parse_front_matter(&input);
            // `image` is relative to the page's own directory; store it
            // site-root-relative so a listing card on another page can link it.
            let card_image = fm.image.map(|img| join_rel(&rel, &img));
            let is_post = rel.starts_with("posts/");
            Page {
                input,
                rel,
                url,
                title: fm.title,
                date: fm.date,
                description: fm.description,
                card_image,
                categories: fm.categories,
                is_post,
                listings: fm.listings,
                about: fm.about,
                page_layout: fm.page_layout,
            }
        })
        .collect();
    pages.sort_by(|a, b| a.rel.cmp(&b.rel));
    pages
}

/// Recursively collect input `.qmd` pages under `dir`, skipping `_`-prefixed
/// directories (`_includes`, `_freeze`, `_site`, …) and dotfiles.
fn collect_pages(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            collect_pages(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("qmd") {
            out.push(p);
        }
    }
}

/// Path of `p` relative to `root`, using `/` separators.
fn rel_str(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Map a `.qmd` rel-path to its built `.html` URL (`x.qmd` → `x.html`).
fn qmd_to_html(rel: &str) -> String {
    match rel.strip_suffix(".qmd") {
        Some(stem) => format!("{stem}.html"),
        None => rel.to_string(),
    }
}

/// Resolve a config/author href for emission from a page at `up` depth: leave
/// external/absolute/anchor links alone, map intra-site `.qmd` to `.html`, and
/// prefix in-tree relative links with the page's `../` depth.
fn resolve_href(href: &str, up: &str) -> String {
    if href.starts_with('#')
        || href.starts_with("//")
        || href.contains("://")
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
    {
        return href.to_string();
    }
    // Site-absolute (`/blog.qmd`): qmd→html, keep absolute.
    if let Some(rest) = href.strip_prefix('/') {
        return format!("/{}", qmd_href(rest));
    }
    // Relative: qmd→html, prefix with the page's depth.
    format!("{up}{}", qmd_href(href))
}

/// `.qmd`→`.html` on an href, preserving any `#fragment`.
fn qmd_href(href: &str) -> String {
    let (path, frag) = match href.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (href, None),
    };
    let mapped = qmd_to_html(path);
    match frag {
        Some(f) => format!("{mapped}#{f}"),
        None => mapped,
    }
}

/// Whether a navbar `href` points at `page` (so the item renders active).
fn href_matches_page(href: &str, page: &Page) -> bool {
    let h = href.trim_start_matches('/');
    let target = qmd_to_html(h);
    target == page.url || h == page.rel
}

/// Rewrite every intra-site `.qmd` link in rendered HTML to its `.html` target,
/// preserving the author's relative/absolute prefix and `#fragment`. External
/// links, data URIs, and non-`.qmd` paths are untouched.
pub fn rewrite_qmd_links(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find("href=\"") {
        let val_start = pos + 6;
        out.push_str(&rest[..val_start]);
        let after = &rest[val_start..];
        let Some(end) = after.find('"') else {
            rest = after;
            break;
        };
        let val = &after[..end];
        out.push_str(&rewrite_one_href(val));
        out.push('"');
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn rewrite_one_href(val: &str) -> String {
    // Only touch in-site links (skip external/anchor/data); rewrite the `.qmd`
    // path component, keeping prefix + fragment intact.
    if val.starts_with('#')
        || val.starts_with("//")
        || val.contains("://")
        || val.starts_with("data:")
        || val.starts_with("mailto:")
        || val.starts_with("tel:")
        || val.starts_with("vscode:")
    {
        return val.to_string();
    }
    let (path, frag) = match val.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (val, None),
    };
    if !path.ends_with(".qmd") {
        return val.to_string();
    }
    let mapped = qmd_to_html(path);
    match frag {
        Some(f) => format!("{mapped}#{f}"),
        None => mapped,
    }
}

/// The front-matter fields discovery needs for nav labels, prev/next, and the
/// listing cards. Parsed once per page from the `---` block.
#[derive(Default)]
struct FrontInfo {
    title: Option<String>,
    date: Option<String>,
    description: Option<String>,
    image: Option<String>,
    categories: Vec<String>,
    listings: Vec<ListingSpec>,
    about: Option<AboutSpec>,
    page_layout: Option<String>,
}

/// Parse a page's `---` front-matter block (YAML) into the fields discovery
/// needs. Tolerant: a missing or malformed block just yields defaults.
fn parse_front_matter(path: &Path) -> FrontInfo {
    let Ok(src) = std::fs::read_to_string(path) else {
        return FrontInfo::default();
    };
    let Some(block) = front_matter_block(&src) else {
        return FrontInfo::default();
    };
    let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(block) else {
        return FrontInfo::default();
    };
    FrontInfo {
        title: scalar(val.get("title")),
        date: scalar(val.get("date")),
        description: scalar(val.get("description")),
        image: scalar(val.get("image")),
        categories: string_list(val.get("categories")),
        listings: parse_listings(val.get("listing")),
        about: parse_about(val.get("about")),
        page_layout: scalar(val.get("page-layout")),
    }
}

/// Parse an `about:` mapping into a profile spec (template + image + links).
fn parse_about(v: Option<&serde_yaml::Value>) -> Option<AboutSpec> {
    let map = match v? {
        serde_yaml::Value::Mapping(_) => v?,
        _ => return None,
    };
    let links = match map.get("links") {
        Some(serde_yaml::Value::Sequence(seq)) => seq
            .iter()
            .map(|it| NavItem {
                text: scalar(it.get("text")),
                href: scalar(it.get("href")),
                icon: scalar(it.get("icon")),
            })
            .filter(|n| n.href.is_some())
            .collect(),
        _ => Vec::new(),
    };
    Some(AboutSpec {
        template: scalar(map.get("template")).unwrap_or_else(|| "jolla".to_string()),
        image: scalar(map.get("image")),
        image_alt: scalar(map.get("image-alt")),
        links,
    })
}

/// The text between the leading `---` and the next `---` (the YAML front matter),
/// or `None` if the document doesn't open with a front-matter fence.
fn front_matter_block(src: &str) -> Option<&str> {
    let rest = src.strip_prefix("---")?;
    // Tolerate `---\n` (and a leading BOM/whitespace already stripped by caller).
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

/// A YAML scalar (string/number/bool) as a display string.
fn scalar(v: Option<&serde_yaml::Value>) -> Option<String> {
    match v? {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// A YAML value that is either a single scalar or a sequence of scalars → a list
/// of strings (used for `categories`).
fn string_list(v: Option<&serde_yaml::Value>) -> Vec<String> {
    match v {
        Some(serde_yaml::Value::Sequence(seq)) => {
            seq.iter().filter_map(|x| scalar(Some(x))).collect()
        }
        Some(other) => scalar(Some(other)).into_iter().collect(),
        None => Vec::new(),
    }
}

/// Parse a `listing:` value: a single map, or a sequence of maps (cv.qmd).
fn parse_listings(v: Option<&serde_yaml::Value>) -> Vec<ListingSpec> {
    match v {
        Some(serde_yaml::Value::Sequence(seq)) => {
            seq.iter().filter_map(parse_listing_spec).collect()
        }
        Some(map @ serde_yaml::Value::Mapping(_)) => parse_listing_spec(map).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn parse_listing_spec(v: &serde_yaml::Value) -> Option<ListingSpec> {
    // `contents` is what makes a listing renderable; without it there's nothing
    // to list (and we only support a single directory string for now).
    let contents = scalar(v.get("contents"))?;
    let sort_desc = scalar(v.get("sort"))
        .map(|s| !s.contains("asc"))
        .unwrap_or(true);
    let max_items = v
        .get("max-items")
        .and_then(serde_yaml::Value::as_u64)
        .map(|n| n as usize);
    Some(ListingSpec {
        id: scalar(v.get("id")),
        contents,
        grid: scalar(v.get("type")).as_deref() == Some("grid"),
        sort_desc,
        max_items,
        categories: v
            .get("categories")
            .and_then(|c| c.as_bool())
            .unwrap_or(false),
    })
}

/// A synthetic block wrapping a listing's cards — used for an id-less listing or
/// when no `::: {#id}` placeholder exists. Generated, so it carries no sourcepos
/// (the corpus invariant test skips empty-sourcepos blocks, like References).
fn listing_block(contents: &str, cards_html: &str) -> Block {
    Block {
        id: format!("listing-{}", contents.replace('/', "-")),
        sourcepos: String::new(),
        source_file: None,
        html: cards_html.to_string(),
        cell: None,
    }
}

/// Resolve `target` (a path relative to the file at `from_rel`) to a site-root-
/// relative path: e.g. (`posts/em/index.qmd`, `thumbnail.webp`) → `posts/em/thumbnail.webp`.
fn join_rel(from_rel: &str, target: &str) -> String {
    if target.starts_with('/') {
        return target.trim_start_matches('/').to_string();
    }
    let dir = match from_rel.rsplit_once('/') {
        Some((d, _)) => d,
        None => "",
    };
    let mut parts: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qmd_urls_map_to_html() {
        assert_eq!(qmd_to_html("blog.qmd"), "blog.html");
        assert_eq!(qmd_to_html("index.qmd"), "index.html");
        assert_eq!(
            qmd_to_html("posts/em-algorithm/index.qmd"),
            "posts/em-algorithm/index.html"
        );
        assert_eq!(qmd_to_html("style.css"), "style.css");
    }

    #[test]
    fn link_rewrite_preserves_prefix_and_fragment() {
        let html = r##"<a href="blog.qmd">b</a> <a href="../KL-divergence/index.qmd#sec-x">k</a> <a href="/projects.qmd">p</a> <a href="https://x.com/a.qmd">ext</a> <a href="#local">l</a>"##;
        let out = rewrite_qmd_links(html);
        assert!(out.contains("href=\"blog.html\""));
        assert!(out.contains("href=\"../KL-divergence/index.html#sec-x\""));
        assert!(out.contains("href=\"/projects.html\""));
        assert!(
            out.contains("href=\"https://x.com/a.qmd\""),
            "external untouched"
        );
        assert!(out.contains("href=\"#local\""), "anchor untouched");
    }

    #[test]
    fn resolve_href_handles_depth_and_externals() {
        assert_eq!(resolve_href("blog.qmd", "../../"), "../../blog.html");
        assert_eq!(resolve_href("/blog.qmd", "../"), "/blog.html");
        assert_eq!(resolve_href("https://x.com", "../"), "https://x.com");
        assert_eq!(resolve_href("#top", "../"), "#top");
    }
}
