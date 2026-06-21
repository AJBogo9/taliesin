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

/// A deck referenced by a `{{< embed >}}` on some page: a standalone document
/// (not a chapter/page) that the build renders to its own self-contained `.html`
/// and the preview serves live, so the embedding iframe resolves.
#[derive(Debug, Clone)]
pub struct DeckRef {
    /// Absolute path to the deck's `.qmd` source.
    pub input: PathBuf,
    /// Output URL relative to the site root (`demo.qmd` → `demo.html`).
    pub url: String,
}

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
    /// `hero:` landing block (headline + lead + CTAs), if declared. Mutually
    /// exclusive with `about:` (both replace the title block).
    pub hero: Option<HeroSpec>,
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

/// A `hero:` front-matter block: the headline + lead + call-to-action band at the
/// top of a landing/home page. Authored entirely in YAML, so a landing page needs
/// no bespoke HTML — it renders into the framework's `.hero` primitive. Reusable
/// for a product page, a researcher's homepage, or a lab/group site.
#[derive(Debug, Clone)]
pub struct HeroSpec {
    /// Small uppercase kicker above the headline (`eyebrow:`); optional.
    pub eyebrow: Option<String>,
    /// The big headline; falls back to the page `title:` when omitted.
    pub headline: Option<String>,
    /// The supporting sentence under the headline (`lead:`); optional.
    pub lead: Option<String>,
    /// Call-to-action buttons (`actions:` — a list of `{text, href, primary}`).
    pub actions: Vec<HeroAction>,
}

/// One `hero:` call-to-action button.
#[derive(Debug, Clone)]
pub struct HeroAction {
    pub text: String,
    pub href: String,
    /// `primary: true` (or `class: primary`) renders the filled accent button;
    /// otherwise the outline style.
    pub primary: bool,
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
    /// Decks referenced by `{{< embed >}}` shortcodes across the pages (deduped).
    /// These aren't pages/chapters; the build renders each to its own `.html` and
    /// the preview serves them live so the embedding iframes resolve.
    pub decks: Vec<DeckRef>,
}

mod book;
mod chrome;
pub use book::{Book, BookEntry};
use book::{book_pages, build_book};
mod feed;
mod meta;
mod search;
mod xref;
pub use xref::XrefTarget;
use xref::{rewrite_cross_refs, scan_xref_targets};
mod config;
mod frontmatter;
pub use config::*;
pub(crate) use frontmatter::*;

impl Site {
    /// Discover the site rooted at `root`: parse `_quarto.yml`, enumerate input
    /// `.qmd` pages, and compute their output URLs + ordering.
    pub fn discover(root: &Path) -> Site {
        let mut warnings = Vec::new();
        let config = load_config(root, &mut warnings);

        // A book takes its page set + order from the explicit `chapters:` list;
        // a website discovers every `.qmd` and orders by path.
        let (mut pages, book) = if config.is_book {
            let book = build_book(root, &config);
            (book_pages(root, &book), Some(book))
        } else {
            (website_pages(root), None)
        };

        // Decks referenced by `{{< embed >}}`. A website discovers *every* `.qmd` as a
        // page, so a deck that's only there to be embedded would otherwise also become
        // a navigable, chrome-wrapped page (and show up in nav/search). Drop those from
        // the page set: an embedded deck is served as a standalone deck, not a page.
        let decks = discover_decks(root, &pages, &mut warnings);
        pages.retain(|p| !decks.iter().any(|d| d.url == p.url));

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
            decks,
        }
    }

    /// A deck referenced by an `{{< embed >}}`, looked up by its output URL (what a
    /// browser requests). Used by the preview to render embedded decks on the fly.
    pub fn deck(&self, url: &str) -> Option<&DeckRef> {
        let needle = url.trim_start_matches('/');
        self.decks.iter().find(|d| d.url == needle)
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
        // Per-page OpenGraph / Twitter-card / SEO meta, so a shared link renders a
        // rich preview. Injected via the head include (no render/mod.rs change).
        let mut includes = self.includes.clone();
        includes.in_header.push_str(&meta::social_head(self, page));
        // Auto-discovery for the RSS feed: a root-relative `<link>` in the head so
        // feed readers (and the browser) find `feed.xml` from any page depth.
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
            // Point the client at the lazy-loaded `search.json` (fetched on first
            // open) instead of inlining the full-text index into every page.
            let up = "../".repeat(depth);
            format!(
                "window.QMD_SEARCH_URL=\"{up}search.json\";window.QMD_SITE_ROOT=\"{up}\";window.QMD_PAGE_URL=\"{}\"",
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
        let mut warnings = std::mem::take(&mut doc.warnings);
        self.finish_blocks(page, &mut doc.blocks, &mut warnings);
        doc.warnings = warnings;
        let ctx = self.page_chrome(page);
        let fallback = page.title.as_deref().unwrap_or("");
        let html = render::html_page_from_doc_in_site(&doc, fallback, &ctx);
        rewrite_qmd_links(&html)
    }

    /// Finish a page's blocks in place: chapter numbering, site-wide cross-ref
    /// resolution (+ broken-ref warnings), site front-matter expansion
    /// (`about:`/`listing:`), and post decoration (reading-time / category badges).
    /// The single block-finishing step shared by the static build, `render_page_doc`,
    /// and the live preview, so all three produce identical blocks (the preview used
    /// to skip `validate_xrefs` + `decorate_post`). `page_toc` is computed by the
    /// caller (it reads blocks but doesn't mutate them).
    pub fn finish_blocks(&self, page: &Page, blocks: &mut Vec<Block>, warnings: &mut Vec<String>) {
        self.number_chapter(page, blocks);
        self.resolve_cross_refs(blocks, &page.url);
        // Cross-refs that survived the site-wide resolution are genuinely broken.
        warnings.extend(crate::cite::validate_xrefs(blocks));
        self.expand_page(page, blocks);
        self.decorate_post(page, blocks);
    }

    /// A self-contained `404.html` for the static build. A static host (GitHub
    /// Pages, Netlify, Cloudflare Pages, …) serves this one file for *any* unknown
    /// path, at any depth, while the browser keeps the bad URL in the address bar —
    /// so every link in it must be **root-absolute** (`/…`), never the
    /// depth-relative links the rest of the site uses (those would resolve against
    /// the wrong directory). To keep that absolute-link surface tiny the page is
    /// deliberately minimal: no navbar, an inlined favicon (data URI), and a single
    /// `/` home link. The base/site CSS is inlined into every page already, so the
    /// page stays on-theme with no relative dependency.
    ///
    /// Absolute `/` assumes a **root deploy** (custom domain or `user.github.io`); a
    /// project-subpath deploy (`user.github.io/repo/`) would need a base path the
    /// config doesn't model yet. Served (with a 404 status) by the live preview's
    /// fallback too, so preview matches production.
    pub fn render_404_page(&self) -> String {
        // Scoped styling for the centred 404 body, injected into the head. Uses the
        // theme `--qmd-*` vars so it tracks light/dark like the rest of the site.
        const NOT_FOUND_STYLE: &str = "\n<style>\n\
            .qmd-404{min-height:60vh;display:flex;flex-direction:column;\
            align-items:center;justify-content:center;text-align:center;gap:.3rem}\n\
            .qmd-404-code{font-family:var(--qmd-font-head);\
            font-size:clamp(4.5rem,20vw,9rem);font-weight:800;line-height:.9;\
            letter-spacing:-.04em;color:var(--qmd-accent)}\n\
            .qmd-404 h1{margin:.4rem 0 0;font-size:1.5rem}\n\
            .qmd-404 p{margin:.2rem 0;color:var(--qmd-muted)}\n\
            .qmd-404-home{display:inline-block;margin-top:1.4rem;font-weight:600}\n\
            </style>";

        let site_title = self.config.title.as_deref().unwrap_or("the site");
        let body = format!(
            "<div class=\"qmd-404\">\n\
             <div class=\"qmd-404-code\">404</div>\n\
             <h1>Page not found</h1>\n\
             <p>The page you’re looking for doesn’t exist or may have moved.</p>\n\
             <p><a class=\"qmd-404-home\" href=\"/\">Back to {}</a></p>\n\
             </div>",
            crate::html_escape(site_title),
        );

        // Start from a default standalone doc (correct theme defaults + bundled
        // data-URI favicon), then swap in the one hand-built block.
        let mut doc = render::render_document("");
        doc.title = Some("Page not found".to_string());
        doc.includes.in_header.push_str(NOT_FOUND_STYLE);
        doc.blocks = vec![Block {
            id: "qmd-404".to_string(),
            sourcepos: "1:1-1:1".to_string(),
            source_file: None,
            html: body,
            cell: None,
        }];
        render::render_doc_to_page(&doc, "Page not found")
    }

    /// Whether a page shows a table of contents: its own front-matter `toc:` wins
    /// (an explicit `toc: false` suppresses it even when the site enables TOCs);
    /// otherwise the site-wide `format: html: toc:` applies, but only to article
    /// pages — a listing or about page would otherwise get a TOC built from its card
    /// titles. Used by both the static build and the live preview.
    pub fn page_toc(&self, page: &Page, doc_toc: Option<bool>) -> bool {
        doc_toc.unwrap_or_else(|| {
            self.config.toc.unwrap_or(false)
                && page.listings.is_empty()
                && page.about.is_none()
                && page.hero.is_none()
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
        // A `hero:` or `about:` block replaces the title block (they're alternative
        // page-header treatments; hero wins if a page somehow declares both).
        if let Some(hero) = &page.hero {
            set_title_block(blocks, self.hero_html(page, hero));
        } else if let Some(about) = &page.about {
            set_title_block(blocks, self.about_html(page, about));
        }
        for (li, spec) in page.listings.iter().enumerate() {
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
                        Some(i) => blocks.insert(i + 1, listing_block(li, &spec.contents, &cards)),
                        // No target at all → append so the listing still renders.
                        None => blocks.push(listing_block(li, &spec.contents, &cards)),
                    }
                }
                None => blocks.push(listing_block(li, &spec.contents, &cards)),
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

    // --- post decoration + category archives ------------------------------

    /// Add a reading-time estimate and category links to a *post's* title block
    /// (a no-op for listing/about/book pages). Both ride in the existing title
    /// block, so they mount + diff like any other content.
    fn decorate_post(&self, page: &Page, blocks: &mut [Block]) {
        if !page.is_post {
            return;
        }
        let words: usize = blocks
            .iter()
            .filter(|b| b.id != "qmd-title-block")
            .map(|b| html_word_count(&b.html))
            .sum();
        let mins = words.div_ceil(200).max(1);
        let read = format!("<span class=\"qmd-read-time\">{mins} min read</span>");
        let cats = if page.categories.is_empty() {
            String::new()
        } else {
            let up = "../".repeat(page.url.matches('/').count());
            let links: String = page
                .categories
                .iter()
                .map(|c| {
                    format!(
                        "<a class=\"qmd-cat\" href=\"{up}categories/{}/\">{}</a>",
                        slugify(c),
                        esc(c)
                    )
                })
                .collect();
            format!("<div class=\"qmd-post-cats\">{links}</div>")
        };
        if let Some(tb) = blocks.iter_mut().find(|b| b.id == "qmd-title-block") {
            inject_title_extras(&mut tb.html, &read, &cats);
        }
    }

    /// Every post category mapped to its posts, newest-first. The basis of the
    /// per-tag archive pages.
    pub fn category_index(&self) -> std::collections::BTreeMap<String, Vec<&Page>> {
        let mut m: std::collections::BTreeMap<String, Vec<&Page>> = Default::default();
        for p in self.pages.iter().filter(|p| p.is_post) {
            for c in &p.categories {
                m.entry(c.clone()).or_default().push(p);
            }
        }
        for v in m.values_mut() {
            v.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.rel.cmp(&b.rel)));
        }
        m
    }

    /// Render the archive page for one category `slug` (a grid of its posts with
    /// full site chrome), or `None` if no category slugs to it. Used by the build
    /// (writes `categories/<slug>/index.html`) and the preview route.
    pub fn render_category_page(&self, slug: &str) -> Option<String> {
        let index = self.category_index();
        let (cat, posts) = index.iter().find(|(c, _)| slugify(c) == slug)?;
        let url = format!("categories/{slug}/index.html");
        let n = posts.len();
        let synth = Page {
            input: self.root.join(&url),
            rel: format!("categories/{slug}/index.qmd"),
            url,
            title: Some(format!("Tagged: {cat}")),
            date: None,
            description: Some(format!(
                "{n} post{} tagged \u{201c}{cat}\u{201d}",
                if n == 1 { "" } else { "s" }
            )),
            card_image: None,
            categories: Vec::new(),
            is_post: false,
            listings: Vec::new(),
            about: None,
            hero: None,
            page_layout: None,
        };
        let grid: String = posts
            .iter()
            .map(|p| self.card_html(p, "../../", true))
            .collect();
        // Render a minimal source so the doc gets the right theme/includes defaults,
        // then append the card grid and wrap it in chrome like any other page.
        let src = format!(
            "---\ntitle: {}\n---\n",
            yaml_quote(&format!("Tagged: {cat}"))
        );
        let mut doc = render::render_document_with_includes(&src, &self.root);
        doc.blocks.push(Block {
            id: "qmd-cat-archive".to_string(),
            sourcepos: String::new(),
            source_file: None,
            html: format!("<div class=\"qmd-listing qmd-listing-grid\">{grid}</div>"),
            cell: None,
        });
        Some(self.render_page_doc(&synth, doc))
    }

    /// All category archive pages as `(url, html)`, for the static build.
    pub fn category_pages(&self) -> Vec<(String, String)> {
        self.category_index()
            .keys()
            .filter_map(|cat| {
                let slug = slugify(cat);
                self.render_category_page(&slug)
                    .map(|html| (format!("categories/{slug}/index.html"), html))
            })
            .collect()
    }

    // --- hero ---------------------------------------------------------------

    /// Render a `hero:` landing header (eyebrow + headline + lead + CTA buttons)
    /// into the framework's `.hero` primitive — no bespoke HTML on the page. The
    /// headline falls back to the page `title:`. Replaces the title block.
    fn hero_html(&self, page: &Page, hero: &HeroSpec) -> String {
        let headline = hero
            .headline
            .clone()
            .or_else(|| page.title.clone())
            .unwrap_or_default();
        let eyebrow = hero
            .eyebrow
            .as_deref()
            .map(|e| format!("<div class=\"hero-eyebrow\">{}</div>", esc(e)))
            .unwrap_or_default();
        let lead = hero
            .lead
            .as_deref()
            .map(|l| format!("<p class=\"hero-lead\">{}</p>", esc(l)))
            .unwrap_or_default();
        let actions = if hero.actions.is_empty() {
            String::new()
        } else {
            let items: String = hero
                .actions
                .iter()
                .map(|a| {
                    let cls = if a.primary {
                        "btn btn-primary btn-lg"
                    } else {
                        "btn btn-lg"
                    };
                    format!(
                        "<a class=\"{cls}\" href=\"{}\">{}</a>",
                        esc(&a.href),
                        esc(&a.text)
                    )
                })
                .collect();
            format!("<div class=\"hero-actions\">{items}</div>")
        };
        format!(
            "<header class=\"hero\" data-block-id=\"qmd-title-block\" data-qmd-src=\"{src}\">\
             {eyebrow}<h1>{headline}</h1>{lead}{actions}</header>",
            src = esc(&page.rel),
            headline = esc(&headline),
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
}

/// A URL-safe slug for a category name (`"Machine Learning"` → `"machine-learning"`):
/// lowercase ASCII alphanumerics, every other run collapsed to a single `-`.
fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// Approximate word count of rendered HTML, skipping tag markup (for reading time).
fn html_word_count(html: &str) -> usize {
    let (mut words, mut in_tag, mut in_word) = (0usize, false, false);
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                in_word = false;
            }
            '>' => in_tag = false,
            _ if in_tag => {}
            c if c.is_whitespace() => in_word = false,
            _ => {
                if !in_word {
                    words += 1;
                    in_word = true;
                }
            }
        }
    }
    words
}

/// Double-quote + escape a string for a YAML scalar (synthetic front matter).
fn yaml_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Splice a reading-time span into a title block's `qmd-title-meta` (creating the
/// meta if absent) and append a category-links block before `</header>`.
fn inject_title_extras(html: &mut String, read: &str, cats: &str) {
    if let Some(meta) = html.find("class=\"qmd-title-meta\">") {
        if let Some(rel) = html[meta..].find("</div>") {
            html.insert_str(meta + rel, read);
        }
    } else if let Some(end) = html.rfind("</header>") {
        html.insert_str(end, &format!("<div class=\"qmd-title-meta\">{read}</div>"));
    }
    if !cats.is_empty()
        && let Some(end) = html.rfind("</header>")
    {
        html.insert_str(end, cats);
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
                hero: fm.hero,
                page_layout: fm.page_layout,
            }
        })
        .collect();
    pages.sort_by(|a, b| a.rel.cmp(&b.rel));
    pages
}

/// Resolve every `{{< embed PATH >}}` across the pages to a deduped [`DeckRef`].
/// The path is written relative to the embedding page, so it's mapped to a
/// site-root-relative path via [`join_rel`]; a target that isn't a file is warned
/// about and skipped (the embed iframe would otherwise 404).
fn discover_decks(root: &Path, pages: &[Page], warnings: &mut Vec<String>) -> Vec<DeckRef> {
    let mut decks: Vec<DeckRef> = Vec::new();
    for page in pages {
        let Ok(src) = std::fs::read_to_string(&page.input) else {
            continue;
        };
        for target in crate::render::embed_targets(&src) {
            let rel = join_rel(&page.rel, &target);
            let url = qmd_to_html(&rel);
            if decks.iter().any(|d| d.url == url) {
                continue;
            }
            let input = root.join(&rel);
            if input.is_file() {
                decks.push(DeckRef { input, url });
            } else {
                warnings.push(format!("{}: embedded deck not found: {target}", page.rel));
            }
        }
    }
    decks
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

/// A synthetic block wrapping a listing card set (id-less listing, or no placeholder).
/// `index` is the listing's position on the page, so two listings of the same
/// `contents:` don't collide on `data-block-id` (which would break the diff).
fn listing_block(index: usize, contents: &str, cards_html: &str) -> Block {
    Block {
        id: format!("listing-{index}-{}", contents.replace('/', "-")),
        sourcepos: String::new(),
        source_file: None,
        html: cards_html.to_string(),
        cell: None,
    }
}

/// Set the page's title-block content to `html` (a `hero:`/`about:` header): reuse
/// the existing `qmd-title-block` so source-mapping + diffing are preserved, or
/// insert it at the top if the page has no title block.
fn set_title_block(blocks: &mut Vec<Block>, html: String) {
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
