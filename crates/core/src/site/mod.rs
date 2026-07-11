//! Multi-page website project model.
//!
//! A *site* is a directory with one explicit root config (`_site.yml`) plus a
//! set of `.tmd` input pages. This module owns the
//! project-level concerns that the single-page path never had:
//!
//!   - parsing the root config (navbar / footer / title) into a typed [`SiteConfig`],
//!   - discovering input pages and mapping each to its output URL (`.tmd` → `.html`),
//!   - the page order used for book chapter prev/next navigation,
//!   - building the shared chrome (navbar, footer, book prev/next) injected into pages,
//!   - rewriting intra-site `.tmd` links to their built `.html` targets.
//!
//! Per the project's config decision there is **no `_metadata.yml` cascade**: the
//! root config is the single source of project-wide defaults and a page's own
//! front matter overrides it. Both `build` (static) and `serve` (live preview)
//! drive the site through [`Site::discover`] + [`Site::render_page`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::render::{self, Block, SiteCtx, Warning, block_heading_level, escape_attr as esc};

/// A deck referenced by a `{{< embed >}}` on some page: a standalone document
/// (not a chapter/page) that the build renders to its own self-contained `.html`
/// and the preview serves live, so the embedding iframe resolves.
#[derive(Debug, Clone)]
pub struct DeckRef {
    /// Absolute path to the deck's `.tmd` source.
    pub input: PathBuf,
    /// Output URL relative to the site root (`demo.tmd` → `demo.html`).
    pub url: String,
}

/// A single input page and where it lands in the built site.
#[derive(Debug, Clone)]
pub struct Page {
    /// Absolute path to the source `.tmd`.
    pub input: PathBuf,
    /// Path relative to the site root, e.g. `posts/em-algorithm/index.tmd`.
    pub rel: String,
    /// Output URL relative to the site root, e.g. `posts/em-algorithm/index.html`.
    pub url: String,
    /// Front-matter title (for nav labels + prev/next + listing cards).
    pub title: Option<String>,
    /// Front-matter `date` as written (ISO strings sort chronologically).
    pub date: Option<String>,
    /// Front-matter `description` (shown on a listing card).
    pub description: Option<String>,
    /// Front-matter `author`(s), for scholarly `citation_author` meta (Google Scholar).
    pub authors: Vec<String>,
    /// Front-matter `image`, resolved to a site-root-relative path (for cards).
    pub card_image: Option<String>,
    /// Front-matter `image-alt`: alt text for the listing card image (a11y). `None`
    /// falls back to empty alt (a decorative card image).
    pub card_image_alt: Option<String>,
    /// Front-matter `categories` (shown as badges on a card).
    pub categories: Vec<String>,
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
/// `template` (e.g. jolla) is kept as a class for styling; the layout is the
/// centered jolla style the corpus uses.
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
    /// Optional portrait beside the hero (`image:`), page-relative; when present the
    /// hero renders a two-column media layout (the blog homepage). Imageless heroes
    /// (the marketing site) are unaffected.
    pub image: Option<String>,
    /// Alt text for `image:` (`image-alt:`).
    pub image_alt: Option<String>,
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
    /// `type: grid` → card-grid layout; `list` and `default` are both a stacked
    /// list and differ only in `with_image` (below).
    pub grid: bool,
    /// Whether cards show their `image:` thumbnail: `grid` and `list`, not plain
    /// `default`. Lets a reading-first `list` keep the figure thumbnails while a
    /// formal text listing (e.g. a CV's projects) stays image-free.
    pub with_image: bool,
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
    /// The reverse of `xref_targets`: each cross-referenced anchor → the urls of the
    /// pages that reference it (deduped, in page order). Built during discovery from
    /// the same all-pages render as `harvest_xref_numbers`; drives the quiet
    /// "Referenced by" backlink line injected next to each target. Empty when nothing
    /// cross-references anything.
    pub backlinks: HashMap<String, Vec<String>>,
    /// Site-wide `format: html:` includes (header/body/css), resolved once at
    /// discovery relative to the site root and merged ahead of each page's own.
    pub includes: render::PageIncludes,
    /// Warnings gathered during discovery (bad config, etc.), surfaced by the
    /// caller (build logs / preview diagnostics).
    pub warnings: Vec<String>,
    /// Inlinable JSON of every page's title + anchored headings, so the Cmd-K
    /// palette searches the whole project (`window.TALIESIN_SEARCH_INDEX`). Assembled
    /// from `search_sections`; the dev server refreshes it per-edited-page via
    /// [`Site::refresh_search_for_page`] so live-preview search doesn't go stale.
    pub search_index_json: String,
    /// The per-page fragments `search_index_json` is assembled from — `(page rel, that
    /// page's JSON entries)` in page order — kept so an edited page's entries can be
    /// re-extracted without re-rendering the whole site.
    search_sections: Vec<(String, String)>,
    /// Inlinable JSON of every cross-reference anchor → the rendered HTML of the block
    /// that defines it, so hovering a CROSS-PAGE `.tali-xref` previews its target
    /// (`window.TALIESIN_HOVER_INDEX`). Built once at discovery; served as
    /// `hover-index.js` and lazy-loaded by `12-link-preview.js`. Empty when the project
    /// has no cross-reference targets.
    pub hover_index_json: String,
    /// Decks referenced by `{{< embed >}}` shortcodes across the pages (deduped).
    /// These aren't pages/chapters; the build renders each to its own `.html` and
    /// the preview serves them live so the embedding iframes resolve.
    pub decks: Vec<DeckRef>,
}

/// Compute a page's Cmd-K search fragment (its JSON entries, or `None` when the page is
/// excluded from search or unreadable) — renders the page's markdown once, no code
/// execution. A free function so the dev server can render it OFF the site lock (and
/// under a panic guard) before installing it via [`Site::install_search_fragment`].
pub fn page_search_fragment(page: &Page) -> Option<String> {
    search::page_fragment(page)
}

mod book;
mod card;
mod chrome;
pub use book::{Book, BookEntry};
use book::{book_pages, build_book};
pub use card::{CARD_DESIGN_VERSION, CARD_EXT, CARD_H, CARD_W, CardSpec, render_card};
mod backlinks;
mod categories;
mod feed;
mod hover;
mod llms;
mod meta;
mod search;
mod seo;
mod xref;
pub use xref::XrefTarget;
use xref::{rewrite_cross_refs, scan_xref_targets};
mod config;
mod frontmatter;
pub use config::*;
pub(crate) use frontmatter::*;
mod chapter;
use chapter::number_chapter_headings;
pub(crate) use chapter::section_number; // also used by xref.rs (via `use super::*`)
mod discovery;
use discovery::{discover_decks, website_pages};
/// Minimum number of `toc_entry_count` headings for a site-wide `toc: true` to render the
/// sidebar TOC (the auto-gate in [`Site::page_toc`]). Below this a page reads as one column.
const MIN_TOC_HEADINGS: usize = 3;
mod links;
pub use links::rewrite_qmd_links;
use links::{
    block_tag_has_id, collect_html_ids, href_matches_page, html_to_qmd, is_external_or_special,
    join_rel, join_rel_in_root, leading_tag_contains, manual_local_links, qmd_to_html,
    resolve_href, sourcepos_start_line,
};

impl Site {
    /// Discover the site rooted at `root`: parse `_site.yml`, enumerate input
    /// `.tmd` pages, and compute their output URLs + ordering.
    pub fn discover(root: &Path) -> Site {
        let mut warnings = Vec::new();
        let config = load_config(root, &mut warnings);

        // A book takes its page set + order from the explicit `chapters:` list;
        // a website discovers every `.tmd` and orders by path.
        let (mut pages, book) = if config.is_book {
            let book = build_book(root, &config);
            let pages = book_pages(root, &book, &mut warnings);
            (pages, Some(book))
        } else {
            (website_pages(root, &mut warnings), None)
        };

        // Decks referenced by `{{< embed >}}`. A website discovers *every* `.tmd` as a
        // page, so a deck that's only there to be embedded would otherwise also become
        // a navigable, chrome-wrapped page (and show up in nav/search). Drop those from
        // the page set: an embedded deck is served as a standalone deck, not a page.
        let decks = discover_decks(root, &pages, &mut warnings);
        pages.retain(|p| !decks.iter().any(|d| d.url == p.url));

        // A loose deck: a `format: revealjs` page that survived the embed retain
        // above, so it isn't referenced by `{{< embed >}}` anywhere. It would be
        // flattened into a chrome-wrapped article (no slides, no deck JS) with no
        // other signal — warn so the author embeds it or moves it out of the site.
        for p in &pages {
            if std::fs::read_to_string(&p.input).is_ok_and(|s| render::is_reveal_doc(&s)) {
                warnings.push(format!(
                    "{}: declares a revealjs deck but is a loose page in the site; it \
                     will render as a flat article. Reference it with {{{{< embed {} >}}}} \
                     from a page, or move it out of the site.",
                    p.rel, p.rel
                ));
            }
        }

        // A `mounts:` prefix that collides with a real page URL: the mounted sub-site
        // and the page share a route, so one silently shadows the other depending on
        // match order. Flag it rather than serve an unpredictable page.
        for m in &config.mounts {
            let at = m.at.trim_matches('/');
            if at.is_empty() {
                continue;
            }
            let prefix = format!("{at}/");
            if let Some(p) = pages
                .iter()
                .find(|p| p.url == at || p.url.starts_with(&prefix))
            {
                warnings.push(format!(
                    "mount `{}` collides with page `{}`: the mounted project and the page \
                     share a URL prefix and will shadow each other",
                    m.at, p.url
                ));
            }
        }

        // A `chapters:` entry naming a file that does not exist: the chapter is silently
        // skipped (its title falls back to the file stem, its body is empty), so a typo
        // in `_site.yml` drops a chapter with no signal.
        if let Some(book) = &book {
            for c in book.chapters() {
                if !root.join(&c.rel).exists() {
                    warnings.push(format!(
                        "chapter file not found: `{}` (listed in _site.yml `chapters:`)",
                        c.rel
                    ));
                }
            }
        }

        // A *relative* site-wide `image:` (the og/twitter social-card default) with no
        // `url:`: it can't be made absolute without the site URL, so og:image /
        // twitter:image are dropped for it. An absolute-URL `image:` needs no base and
        // still works, so it isn't flagged; nor is a per-page `image:`, which drives
        // listing cards that don't need an absolute URL.
        if config.url.is_none()
            && config
                .card_image
                .as_deref()
                .is_some_and(|img| !is_external_or_special(img))
        {
            warnings.push(
                "a relative `image:` is set in _site.yml but `url:` is not: the default \
                 social-card image (og:image / twitter:image) needs an absolute URL and \
                 is being suppressed. Set `url:`, or use an absolute image URL."
                    .to_string(),
            );
        }

        // Resolve the site-wide head/body/css includes once, relative to the site
        // root (where `_site.yml` and its referenced css/js files live).
        let includes = render::includes_from_parts(
            config.head.as_ref(),
            config.body_start.as_ref(),
            config.body_end.as_ref(),
            config.css.as_ref(),
            Some(root),
        );

        let xref_targets = scan_xref_targets(&pages, &book, &mut warnings);
        let search_sections = search::build_sections(&pages);
        let search_index_json = search::assemble(&search_sections);

        let mut site = Site {
            root: root.to_path_buf(),
            config,
            pages,
            book,
            xref_targets,
            backlinks: HashMap::new(),
            includes,
            warnings,
            search_index_json,
            search_sections,
            hover_index_json: String::new(),
            decks,
        };
        // Fill the cross-PAGE numbers the lightweight source-scan can't know — a figure /
        // equation / table / listing / theorem number is assigned only during render, so
        // `scan_xref_targets` left it empty. Harvesting here (not only in `build`) means the
        // live preview also renders "Theorem 1" / "Figure 2.3" for a cross-page ref instead
        // of a bare label. A pure render pass with no kernel execution, run once per
        // discover so build, preview, and `check` resolve numbers identically.
        site.harvest_xref_numbers();
        // Likewise once per discover: the cross-page hover-preview snippet index (its own
        // scoped render pass; independent of the numbers above).
        site.build_hover_index();
        site
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

    /// The output directory `build` writes to (default `_site`, or `_book` for a
    /// book).
    pub fn output_dir(&self) -> &str {
        self.config
            .output_dir
            .as_deref()
            .unwrap_or(if self.is_book() { "_book" } else { "_site" })
    }

    /// Whether the author supplies their own `404.tmd` (output URL `404.html`). When
    /// true the build must NOT clobber it with the built-in not-found template, and
    /// the page is kept out of the Cmd-K search index (a 404 is navigation chrome, not
    /// content). When false the build emits [`render_404_page`](Self::render_404_page).
    pub fn has_author_404(&self) -> bool {
        self.pages.iter().any(|p| p.url == "404.html")
    }

    /// Look up a page by its source rel-path or its output URL (`serve` accepts
    /// either an editor path or a browser request).
    pub fn page(&self, rel_or_url: &str) -> Option<&Page> {
        let needle = rel_or_url.trim_start_matches('/');
        self.pages
            .iter()
            .find(|p| p.rel == needle || p.url == needle)
    }

    /// Re-extract the Cmd-K search entries for the given page (accepts its `rel` or
    /// `url`, like [`Site::page`]) and reassemble the index, so a content edit in the
    /// live preview doesn't leave stale headings/prose in search (the static build
    /// always builds the index fresh). A no-op for an unknown page.
    ///
    /// This renders the page inline (holding whatever lock the caller holds). The dev
    /// server's hot path renders OFF the site lock and under a panic guard via
    /// [`page_search_fragment`] + [`Site::install_search_fragment`] instead; this
    /// convenience wrapper is for callers (tests) where that doesn't matter.
    pub fn refresh_search_for_page(&mut self, rel_or_url: &str) {
        let Some((rel, fragment)) = self
            .page(rel_or_url)
            .map(|page| (page.rel.clone(), search::page_fragment(page)))
        else {
            return;
        };
        self.install_search_fragment(&rel, fragment);
    }

    /// Install a freshly-computed search fragment for page `rel` (from
    /// [`page_search_fragment`]) and reassemble the index. Split from the render so the
    /// dev server can render the fragment off the site lock. A no-op for an unknown page
    /// that has no content to add.
    pub fn install_search_fragment(&mut self, rel: &str, fragment: Option<String>) {
        match self.search_sections.iter().position(|(r, _)| r == rel) {
            // Already indexed: replace its fragment, or drop it if the page now yields none.
            Some(pos) => match fragment {
                Some(frag) => self.search_sections[pos].1 = frag,
                None => {
                    self.search_sections.remove(pos);
                }
            },
            // Not previously indexed but now has content: recompute so page order holds.
            None if fragment.is_some() => {
                self.search_sections = search::build_sections(&self.pages);
            }
            None => return,
        }
        self.search_index_json = search::assemble(&self.search_sections);
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
        includes.in_header.push_str(&meta::jsonld_head(self, page));
        // The cross-page search index (+ how to resolve a result's page URL from
        // this page's depth). Empty when there are no entries; injected only where
        // the search palette also rides along (TOC pages).
        // Per-page site head JS: the depth-relative site root + this page's URL (used by
        // cross-page navigation — resolving a Cmd-K result to its page), plus the lazy
        // search-index URL. Empty when the project has no search index.
        let has_search = !self.search_index_json.is_empty() && self.search_index_json != "[]";
        let search_index = if !has_search {
            String::new()
        } else {
            // A script subresource (search-index.js) loads under file:// too, so Cmd-K
            // works from disk with no dev server.
            let up = "../".repeat(depth);
            format!(
                "window.TALIESIN_SITE_ROOT=\"{up}\";window.TALIESIN_PAGE_URL=\"{}\";\
                 window.TALIESIN_SEARCH_URL=\"{up}search-index.js\"",
                page.url
            )
        };
        // Cross-page hover-preview: point every page at the lazy hover-index.js and set the
        // site root so the client can resolve a snippet's rebased (root-relative) asset URLs.
        // Injected into the always-emitted head (unlike search, which rides only TOC pages)
        // because a cross-page ref can appear on any page. Idempotent with search's own
        // TALIESIN_SITE_ROOT on TOC pages (same value).
        if !self.hover_index_json.is_empty() {
            let up = "../".repeat(depth);
            includes.in_header.push_str(&format!(
                "<script>window.TALIESIN_SITE_ROOT=\"{up}\";\
                 window.TALIESIN_HOVER_URL=\"{up}hover-index.js\";</script>"
            ));
        }
        SiteCtx {
            // A book replaces the top navbar with a slim topbar + off-canvas chapter
            // drawer and uses chapter prev/next instead of the post "back to listing" link.
            navbar_html: if book {
                String::new()
            } else {
                self.navbar_html(page, depth)
            },
            footer_html: self.footer_html(depth),
            post_nav_html: if book {
                self.book_nav_html(page, depth)
            } else {
                self.listing_backlink_html(page, depth)
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
        // A numbered book chapter scopes its theorems to its chapter number
        // ("Theorem 2.3"); non-book / unnumbered pages pass None (continuous).
        let doc = render::render_document_with_includes_scoped(&src, base, self.chapter_for(page));
        Some(self.render_page_doc(page, doc))
    }

    /// Finish a page whose `doc.blocks` are already produced — and possibly
    /// code-executed (the static build runs cells, then calls this): apply the
    /// site front-matter expansion (`about:`/`listing:`), wrap in chrome, and
    /// rewrite intra-site `.tmd` links. Shared by `render_page` (no execution) and
    /// the executing `build` path so both emit identical chrome + links.
    pub fn render_page_doc(&self, page: &Page, doc: render::RenderedDoc) -> String {
        self.render_page_doc_warned(page, doc).0
    }

    /// Like [`render_page_doc`](Self::render_page_doc) but also returns the page's
    /// warnings (render warnings + broken cross-refs from `finish_blocks`), so the
    /// static `build` can print them to stderr instead of letting a broken site
    /// deploy silently.
    pub fn render_page_doc_warned(
        &self,
        page: &Page,
        mut doc: render::RenderedDoc,
    ) -> (String, Vec<Warning>) {
        doc.toc = self.page_toc(page, doc.toc_explicit, &doc.blocks);
        let mut warnings = std::mem::take(&mut doc.warnings);
        self.finish_blocks(page, &mut doc.blocks, &mut warnings);
        let ctx = self.page_chrome(page);
        let fallback = page.title.as_deref().unwrap_or("");
        let html = render::html_page_from_doc_in_site(&doc, fallback, &ctx);
        (rewrite_qmd_links(&html), warnings)
    }

    /// Static `check` cross-page link validation: for every page, resolve each manual
    /// relative `<a href>` against the project's **page registry** (the set of built
    /// `.html` urls) and the target page's id set, flagging (a) a link whose target page
    /// is not in the site, and (b) a `page.html#frag` whose `frag` is no id on that page.
    /// Returns `(page_rel, Warning)` so the caller can locate each to its source page.
    ///
    /// Read-only and offline: external/absolute links are skipped (never fetched — a
    /// network probe would make `check` nondeterministic). The anchor half is suppressed
    /// for a target page that runs executable cells (a cell can emit an id at runtime),
    /// mirroring `diagnostics::validate_internal_anchors`'s no-false-positive promise.
    pub fn validate_cross_page_links(&self) -> Vec<(String, Warning)> {
        // ONE render pass per page: build the id/cell registry AND capture every page's
        // outgoing local links at the same time, so the resolution scan below reuses the
        // captured links instead of rendering the whole site a SECOND time.
        struct LinkRef {
            path: String,
            frag: Option<String>,
            line: Option<u32>,
            source_file: Option<String>,
        }
        let mut ids_by_url: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
        let mut cells_by_url: HashMap<String, bool> = HashMap::new();
        let mut pages_links: Vec<(String, String, Vec<LinkRef>)> = Vec::new();
        for page in &self.pages {
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let base = page.input.parent().unwrap_or(&self.root);
            let doc = render::render_document_with_includes(&src, base);
            let mut ids = std::collections::HashSet::new();
            let mut links = Vec::new();
            for b in &doc.blocks {
                collect_html_ids(&b.html, &mut ids);
                let line = sourcepos_start_line(&b.sourcepos);
                for (path, frag) in manual_local_links(&b.html) {
                    links.push(LinkRef {
                        path: path.to_string(),
                        frag: frag.map(str::to_string),
                        line,
                        source_file: b.source_file.clone(),
                    });
                }
            }
            cells_by_url.insert(
                page.url.clone(),
                doc.blocks.iter().any(|b| b.cell.is_some()),
            );
            ids_by_url.insert(page.url.clone(), ids);
            pages_links.push((page.rel.clone(), page.url.clone(), links));
        }

        let mut out = Vec::new();
        for (rel, url, links) in &pages_links {
            for lk in links {
                let path = lk.path.as_str();
                let frag = lk.frag.as_deref();
                let line = lk.line;
                let source_file = &lk.source_file;
                // Resolve to a site-root-relative `.html` url. `.tmd`→`.html`, then
                // join against the page's directory. A link that climbs *above* the
                // site root (`../other-book/…`, a mounted sibling) is unresolvable
                // offline and deliberately skipped — only the marketing site that
                // mounts both books can resolve it, so flagging it here would be a
                // false positive (cross-book/mount links are written as relative
                // `.html` by design; see docs/ CLAUDE.md).
                let Some(target_url) = join_rel_in_root(url, &qmd_to_html(path)) else {
                    continue;
                };
                // A directory-style link (`dir/`) targets that dir's index.
                let target_url = if target_url.is_empty() || target_url.ends_with('/') {
                    format!("{target_url}index.html")
                } else {
                    target_url
                };
                let Some(target_ids) = ids_by_url.get(&target_url) else {
                    // A target outside the page registry is only "broken" if nothing
                    // on disk backs it: an `{{< embed >}}`-referenced deck (built +
                    // served but kept out of nav/registry) and any source file that
                    // exists under the root are legitimate targets.
                    // A target under a configured `mounts:` prefix resolves only when
                    // the mounted project is served (preview) or copied in (build) — it is
                    // not in this site's own page registry, so it is not "broken". (build
                    // separately warns these links are preview-only.) Matches the mount
                    // root (`docs`), its index (`docs/index.html`), and anything beneath it.
                    let under_mount = self.config.mounts.iter().any(|m| {
                        target_url == m.at
                            || target_url == format!("{}/index.html", m.at)
                            || target_url.starts_with(&format!("{}/", m.at))
                    });
                    if under_mount
                        || self.decks.iter().any(|d| d.url == target_url)
                        || self.root.join(&target_url).is_file()
                        || html_to_qmd(&target_url)
                            .iter()
                            .any(|p| self.root.join(p).is_file())
                    {
                        continue;
                    }
                    let w = Warning::new(format!(
                        "broken link: `{path}` resolves to `{target_url}`, which is no page in this site"
                    ));
                    out.push((
                        rel.clone(),
                        match line {
                            Some(l) => w.at(source_file.clone(), l),
                            None => w,
                        },
                    ));
                    continue;
                };
                // Anchor existence: only when the link carries a fragment, the target
                // page does not run cells (a cell can emit the id at runtime), and the
                // anchor is missing.
                if let Some(frag) = frag
                    && !frag.is_empty()
                    && !cells_by_url.get(&target_url).copied().unwrap_or(false)
                    && !target_ids.contains(frag)
                {
                    let w = Warning::new(format!(
                        "broken link anchor: `#{frag}` is no element id on `{target_url}`"
                    ));
                    out.push((
                        rel.clone(),
                        match line {
                            Some(l) => w.at(source_file.clone(), l),
                            None => w,
                        },
                    ));
                }
            }
        }
        out
    }

    /// Finish a page's blocks in place: chapter numbering, site-wide cross-ref
    /// resolution (+ broken-ref warnings), and site front-matter expansion
    /// (`about:`/`listing:`). The single block-finishing step shared by the static
    /// build, `render_page_doc`, and the live preview, so all three produce identical
    /// blocks (the preview used to skip `validate_xrefs`). `page_toc` is computed by
    /// the caller (it reads blocks but doesn't mutate them).
    pub fn finish_blocks(&self, page: &Page, blocks: &mut Vec<Block>, warnings: &mut Vec<Warning>) {
        self.number_chapter(page, blocks);
        self.resolve_cross_refs(blocks, &page.url);
        // Cross-refs that survived the site-wide resolution are genuinely broken.
        warnings.extend(crate::cite::validate_xrefs(blocks));
        // Reverse side of cross-refs: a quiet "Referenced by" line after each target
        // this page defines that other pages reference.
        self.attach_backlinks(blocks, &page.url);
        self.expand_page(page, blocks, warnings);
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
        // theme `--tali-*` vars so it tracks light/dark like the rest of the site.
        const NOT_FOUND_STYLE: &str = "\n<style>\n\
            .tali-404{min-height:60vh;display:flex;flex-direction:column;\
            align-items:center;justify-content:center;text-align:center;gap:.3rem}\n\
            .tali-404-code{font-family:var(--tali-font-head);\
            font-size:clamp(4.5rem,20vw,9rem);font-weight:800;line-height:.9;\
            letter-spacing:-.04em;color:var(--tali-accent)}\n\
            .tali-404 h1{margin:.4rem 0 0;font-size:1.5rem}\n\
            .tali-404 p{margin:.2rem 0;color:var(--tali-muted)}\n\
            .tali-404-home{display:inline-block;margin-top:1.4rem;font-weight:600}\n\
            </style>";

        let site_title = self.config.title.as_deref().unwrap_or("the site");
        let body = format!(
            "<div class=\"tali-404\">\n\
             <div class=\"tali-404-code\">404</div>\n\
             <h1>Page not found</h1>\n\
             <p>The page you’re looking for doesn’t exist or may have moved.</p>\n\
             <p><a class=\"tali-404-home\" href=\"/\">Back to {}</a></p>\n\
             </div>",
            crate::html_escape(site_title),
        );

        // Start from a default standalone doc (correct theme defaults + bundled
        // data-URI favicon), then swap in the one hand-built block.
        let mut doc = render::render_document("");
        doc.title = Some("Page not found".to_string());
        doc.includes.in_header.push_str(NOT_FOUND_STYLE);
        doc.blocks = vec![Block {
            id: "tali-404".to_string(),
            sourcepos: "1:1-1:1".to_string(),
            source_file: None,
            html: body,
            cell: None,
        }];
        render::render_doc_to_page(&doc, "Page not found", render::OutputMode::Build)
    }

    /// Whether a page shows a table of contents: its own front-matter `toc:` wins
    /// (an explicit `toc: false` suppresses it even when the site enables TOCs, and an
    /// explicit `toc: true` forces it on regardless of length); otherwise the site-wide
    /// `toc:` applies, but only to article pages with enough headings to warrant it — the
    /// page's rendered `blocks` are counted by `render::toc_entry_count`, and a page below
    /// [`MIN_TOC_HEADINGS`] (or a listing / about / hero page) reads as a single column
    /// instead of getting a near-empty TOC. Used by both the static build and live preview.
    pub fn page_toc(&self, page: &Page, doc_toc: Option<bool>, blocks: &[Block]) -> bool {
        doc_toc.unwrap_or_else(|| {
            self.config.toc.unwrap_or(false)
                && page.listings.is_empty()
                && page.about.is_none()
                && page.hero.is_none()
                // Auto-gate (NN/g: show a TOC only on long, chunkable pages): a site-wide
                // `toc: true` lands the sidebar TOC only when the page has enough sections.
                && render::toc_entry_count(blocks) >= MIN_TOC_HEADINGS
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

    /// Render-harvest: render each page once (scoped to its chapter) and fill in the
    /// CROSS-PAGE numbers the lightweight source-scan can't know — a figure / equation /
    /// table / listing / theorem number is assigned only during render, so
    /// `scan_xref_targets` left it empty. This enriches `xref_targets[anchor].number`
    /// (for those non-heading anchors), so a `@fig-x` / `@thm-x` to another page renders
    /// "Figure&nbsp;2.3" / "Theorem&nbsp;1" instead of a bare "Figure" / "Theorem".
    /// Called once by `discover`, so build AND the live preview resolve the same numbers.
    /// A pure render pass (no kernel execution), amortised across the discover it rides on.
    ///
    /// The same all-pages render also builds the reverse index [`Site::backlinks`]
    /// (anchor → referring pages) from each page's `data-qmd-xref` markers — cite emits
    /// that marker only for a reference whose target is on another page, so a marker is
    /// by construction a cross-page reference. Riding this existing render keeps it to
    /// no extra traversal.
    pub fn harvest_xref_numbers(&mut self) {
        // Collect during the `&self.pages` pass, then apply — keeps the borrows disjoint.
        let mut updates: Vec<(String, String)> = Vec::new();
        // (page url, that page's cross-page reference anchors) in site page order, so
        // each target's referrer list comes out in document order.
        let mut per_page: Vec<(String, Vec<String>)> = Vec::new();
        for page in &self.pages {
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let base = page.input.parent().unwrap_or(&self.root);
            let doc =
                render::render_document_with_includes_scoped(&src, base, self.chapter_for(page));
            let mut refs: Vec<String> = Vec::new();
            for b in &doc.blocks {
                if b.html.contains("data-qmd-xref=\"") {
                    refs.extend(
                        xref::xref_markers_in(&b.html)
                            .into_iter()
                            .map(str::to_string),
                    );
                }
            }
            per_page.push((page.url.clone(), refs));
            for (anchor, number) in doc.xref_numbers {
                // `sec-` numbers are the source-scan's job (chapter-hierarchical, and
                // correctly ABSENT on a non-book website). Harvesting the render's flat
                // per-page section counter here would fill an empty website target with
                // a bare "1", which `rewrite_one_xref` then mislabels "Chapter 1". Only
                // fig/eq/tbl/lst/thm need this render-time enrichment.
                if !number.is_empty() && !anchor.starts_with("sec-") {
                    updates.push((anchor, number));
                }
            }
        }
        for (anchor, number) in updates {
            if let Some(t) = self.xref_targets.get_mut(&anchor) {
                // Only fill a gap the source-scan left (fig/eq/tbl/lst/thm); a book
                // heading's section number is already authoritative from the scan.
                if t.number.is_empty() {
                    t.number = number;
                }
            }
        }
        self.backlinks = backlinks::build_backlink_index(&per_page, &self.xref_targets);
    }

    /// Discovery render-harvest: render each page that defines cross-reference targets
    /// once (scoped to its chapter, like [`harvest_xref_numbers`](Self::harvest_xref_numbers))
    /// and capture, per anchor, the rendered HTML of its defining block — the snippet the
    /// cross-page hover-preview card shows. Relative asset URLs are rebased site-root-relative
    /// so the snippet renders correctly on any viewing page. Runs inside `discover`, so the
    /// index is always populated (build, preview, and after a preview structural rebuild) with
    /// no extra call site. `hover::` does the per-anchor extraction + URL rebasing.
    fn build_hover_index(&mut self) {
        if self.xref_targets.is_empty() {
            return;
        }
        // Anchors grouped by their defining page's url, so each page renders at most once.
        let mut by_page: HashMap<&str, Vec<&str>> = HashMap::new();
        for (anchor, t) in &self.xref_targets {
            by_page
                .entry(t.url.as_str())
                .or_default()
                .push(anchor.as_str());
        }
        let mut entries: Vec<(String, String)> = Vec::new();
        for page in &self.pages {
            let Some(anchors) = by_page.get(page.url.as_str()) else {
                continue;
            };
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let base = page.input.parent().unwrap_or(&self.root);
            let mut doc =
                render::render_document_with_includes_scoped(&src, base, self.chapter_for(page));
            // Apply the book's chapter/section numbering so a hovered section heading
            // shows its number ("2.1"), matching the page it previews (the scoped render
            // alone doesn't prefix heading numbers — that's number_chapter_headings).
            self.number_chapter(page, &mut doc.blocks);
            for anchor in anchors {
                if let Some(snippet) = hover::extract_snippet(&doc.blocks, anchor) {
                    let snippet = hover::rewrite_snippet_urls(&snippet, &page.url);
                    entries.push((anchor.to_string(), snippet));
                }
            }
        }
        if entries.is_empty() {
            return;
        }
        // Stable order so the index is deterministic across builds.
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut out = String::from("{");
        for (i, (anchor, snippet)) in entries.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "\"{}\":\"{}\"",
                search::json_str(anchor),
                search::json_str(snippet)
            ));
        }
        out.push('}');
        self.hover_index_json = out;
    }

    /// This page's book chapter number, if it is a numbered chapter (None for a
    /// website page or an unnumbered preface). Drives both heading section numbering
    /// and `theorems: number-within: chapter` scoping, so they stay in lockstep.
    pub fn chapter_for(&self, page: &Page) -> Option<u32> {
        self.book.as_ref().and_then(|b| {
            b.entries
                .iter()
                .find(|e| e.rel == page.rel)
                .and_then(|e| e.number)
        })
    }

    /// Number a book chapter's headings in place (chapter N, then N.1, N.1.1 …),
    /// controlled by `number-sections`. A no-op for a website or an unnumbered
    /// preface. Called by both the static build and the live preview.
    pub fn number_chapter(&self, page: &Page, blocks: &mut [Block]) {
        if let Some(number) = self.chapter_for(page) {
            number_chapter_headings(blocks, number);
        }
    }

    // --- listings ---------------------------------------------------------

    /// Apply this page's site-level front-matter blocks to its rendered `blocks`,
    /// mutating in place: an `about:` profile replaces the title block, and each
    /// `listing:` expands into post cards. Both the static build and the live
    /// preview call this, so the results stay in the block model (mounted + diffed
    /// like any other block).
    pub fn expand_page(&self, page: &Page, blocks: &mut Vec<Block>, warnings: &mut Vec<Warning>) {
        // A `hero:` or `about:` block replaces the title block (they're alternative
        // page-header treatments; hero wins if a page somehow declares both).
        if let Some(hero) = &page.hero {
            set_title_block(blocks, self.hero_html(page, hero));
        } else if let Some(about) = &page.about {
            set_title_block(blocks, self.about_html(page, about));
        }
        for (li, spec) in page.listings.iter().enumerate() {
            let cards = self.listing_html(page, spec, warnings);
            match &spec.id {
                Some(id) => {
                    match blocks.iter().position(|b| block_tag_has_id(&b.html, id)) {
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

    /// The rel-path prefix a listing covers: `contents:` joined onto the hosting
    /// page's directory. `contents: .` on a root page resolves to the empty dir,
    /// which must match the whole project (an empty prefix), not `"/"` — which
    /// matched nothing, so the listing silently came up empty. A named subdir keeps
    /// its trailing slash so only that subtree matches. Shared by `collection()`
    /// (which pages a listing renders) and `listing_owner()` (which listing a page
    /// belongs to), so the two always agree on coverage.
    fn listing_prefix(host: &Page, spec: &ListingSpec) -> String {
        let dir = join_rel(&host.rel, spec.contents.trim_end_matches('/'));
        if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        }
    }

    /// The single listing page a `page` "belongs to" — the sole page whose
    /// **un-capped** `listing:` covers it — or `None` when zero or two-plus do. A
    /// `max-items`-capped listing is a *preview*, not the post's home, so it does not
    /// confer ownership: otherwise a "recent posts" preview on the home page would
    /// make every post read as ambiguous against its full listing page. Drives the
    /// bottom-of-post "back to listing" link.
    fn listing_owner(&self, page: &Page) -> Option<&Page> {
        // A titleless page never renders as a card, so it belongs to no listing
        // (mirrors `collection()` dropping it).
        page.title.as_ref()?;
        let mut owner: Option<&Page> = None;
        for host in &self.pages {
            // Skip the page itself, and any host with no `title:` — it can't render a
            // sensible "← <title>" label, so it isn't a listing citizen (symmetry with
            // the titleless-covered-page guard above).
            if host.rel == page.rel || host.title.is_none() {
                continue;
            }
            let covers = host.listings.iter().any(|spec| {
                spec.max_items.is_none() && page.rel.starts_with(&Self::listing_prefix(host, spec))
            });
            if !covers {
                continue;
            }
            if owner.is_some() {
                return None; // two-plus distinct owners → ambiguous, skip
            }
            owner = Some(host);
        }
        owner
    }

    /// The pages a listing covers: those under its `contents:` directory (relative
    /// to the hosting page), newest-first (or oldest-first), capped by `max-items`.
    fn collection(
        &self,
        host: &Page,
        spec: &ListingSpec,
        warnings: &mut Vec<Warning>,
    ) -> Vec<&Page> {
        let prefix = Self::listing_prefix(host, spec);
        let mut items: Vec<&Page> = Vec::new();
        for p in &self.pages {
            if p.rel == host.rel || !p.rel.starts_with(&prefix) {
                continue;
            }
            if p.title.is_none() {
                // A card needs a title to render, so a titleless post was silently
                // dropped from the listing — surface it rather than lose the post.
                warnings.push(Warning::new(format!(
                    "`{}` has no `title:` and is omitted from the listing on `{}`",
                    p.rel, host.rel
                )));
                continue;
            }
            items.push(p);
        }
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
    fn listing_html(&self, host: &Page, spec: &ListingSpec, warnings: &mut Vec<Warning>) -> String {
        let up = "../".repeat(host.url.matches('/').count());
        let layout = if spec.grid { "grid" } else { "default" };
        let items = self.collection(host, spec, warnings);
        let cards: String = items
            .iter()
            .map(|p| self.card_html(p, &up, spec.with_image))
            .collect();
        let grid = format!("<div class=\"tali-listing tali-listing-{layout}\">{cards}</div>");

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
            "<button class=\"tali-cat-chip tali-cat-active\" type=\"button\" data-cat=\"\">All</button>",
        );
        for (cat, n) in &counts {
            chips.push_str(&format!(
                "<button class=\"tali-cat-chip\" type=\"button\" data-cat=\"{c}\">{label}\
                 <span class=\"tali-cat-count\">{n}</span></button>",
                c = esc(cat),
                label = esc(cat),
            ));
        }
        format!(
            "<div class=\"tali-listing-wrap\">\
             <nav class=\"tali-cat-filter\" aria-label=\"Filter by category\">{chips}</nav>{grid}</div>"
        )
    }

    fn card_html(&self, p: &Page, up: &str, with_image: bool) -> String {
        let href = format!("{up}{}", p.url);
        let img = match (with_image, &p.card_image) {
            (true, Some(src)) => format!(
                "<img class=\"tali-card-img\" src=\"{up}{}\" alt=\"{}\" loading=\"lazy\">",
                esc(src),
                esc(p.card_image_alt.as_deref().unwrap_or(""))
            ),
            _ => String::new(),
        };
        let date = p
            .date
            .as_deref()
            .map(|d| format!("<div class=\"tali-card-date\">{}</div>", esc(d)))
            .unwrap_or_default();
        let title = esc(p.title.as_deref().unwrap_or(&p.rel));
        let desc = p
            .description
            .as_deref()
            .map(|d| format!("<p class=\"tali-card-desc\">{}</p>", esc(d)))
            .unwrap_or_default();
        // Each badge carries `data-cat` so a click on it toggles that category in
        // the filter; the filter also reads these badges to know a card's categories.
        let cats = if p.categories.is_empty() {
            String::new()
        } else {
            let badges: String = p
                .categories
                .iter()
                .map(|c| {
                    format!(
                        "<span class=\"tali-cat\" data-cat=\"{c}\">{c}</span>",
                        c = esc(c)
                    )
                })
                .collect();
            format!("<div class=\"tali-card-cats\">{badges}</div>")
        };
        // No delimited `data-categories` list: the client filter reads each card's
        // own `.tali-cat[data-cat]` badges (exact names), so a category name
        // containing a comma still matches.
        // `data-qmd-src` lets the click-to-source locator jump to the post's source
        // (it's site-root-relative; resolved client-side, inert in the static build).
        format!(
            "<a class=\"tali-card\" href=\"{href}\" data-qmd-src=\"{src}\">{img}\
             <div class=\"tali-card-body\">{date}<h3 class=\"tali-card-title\">{title}</h3>{desc}{cats}</div></a>",
            src = esc(&p.rel)
        )
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
        let src = esc(&page.rel);
        let headline = esc(&headline);
        let inner = format!("{eyebrow}<h1>{headline}</h1>{lead}{actions}");
        // Imageless hero (the marketing site): byte-identical to before the portrait
        // slot existed. With an image (the blog homepage): a two-column media layout.
        match hero.image.as_deref() {
            None => format!(
                "<header class=\"hero\" data-block-id=\"qmd-title-block\" data-qmd-src=\"{src}\">{inner}</header>"
            ),
            Some(image) => {
                let image = esc(image);
                let alt = esc(hero.image_alt.as_deref().unwrap_or(""));
                format!(
                    "<header class=\"hero hero-has-media\" data-block-id=\"qmd-title-block\" data-qmd-src=\"{src}\">\
                     <div class=\"hero-body\">{inner}</div>\
                     <img class=\"hero-media\" src=\"{image}\" alt=\"{alt}\"></header>"
                )
            }
        }
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
                    "<img class=\"tali-about-img\" src=\"{}\" alt=\"{}\">",
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
                        "<a class=\"tali-about-link\" href=\"{}\">{}</a>",
                        esc(href),
                        esc(label)
                    ))
                })
                .collect();
            format!("<div class=\"tali-about-links\">{items}</div>")
        };
        format!(
            "<header class=\"tali-about tali-about-{tpl}\" data-block-id=\"qmd-title-block\" data-qmd-src=\"{src}\">\
             {img}<h1 class=\"tali-about-name\">{name}</h1>{links}</header>",
            tpl = esc(&about.template),
            name = esc(&name),
            src = esc(&page.rel),
        )
    }

    // --- chrome -----------------------------------------------------------
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn website_pages_excludes_drafts() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("tali-draft-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
        fs::write(
            root.join("published.tmd"),
            "---\ntitle: Pub\n---\n\nPublished.\n",
        )
        .unwrap();
        fs::write(
            root.join("wip.tmd"),
            "---\ntitle: WIP\ndraft: true\n---\n\nWork in progress.\n",
        )
        .unwrap();

        let rels: Vec<String> = website_pages(&root, &mut Vec::new())
            .iter()
            .map(|p| p.rel.clone())
            .collect();
        assert!(rels.contains(&"index.tmd".to_string()), "kept: {rels:?}");
        assert!(
            rels.contains(&"published.tmd".to_string()),
            "kept: {rels:?}"
        );
        assert!(
            !rels.contains(&"wip.tmd".to_string()),
            "draft excluded: {rels:?}"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn absolute_image_url_is_not_mangled_into_a_relative_path() {
        // Batch 7: a page `image:` is the og:image / social-card source. When it's an
        // absolute URL, `join_rel` used to fold its scheme into a broken relative path
        // (`posts/https:/cdn.example.com/card.png`), breaking og:image + listing cards.
        // An external URL must pass through untouched; a local image still resolves
        // site-root-relative so a listing card on another page can link it.
        use std::fs;
        let root = std::env::temp_dir().join(format!("tali-absimg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("posts")).unwrap();
        fs::write(
            root.join("posts").join("a.tmd"),
            "---\ntitle: A\nimage: https://cdn.example.com/card.png\n---\n\nBody.\n",
        )
        .unwrap();
        fs::write(
            root.join("posts").join("b.tmd"),
            "---\ntitle: B\nimage: thumb.webp\n---\n\nBody.\n",
        )
        .unwrap();

        let pages = website_pages(&root, &mut Vec::new());
        let img = |rel: &str| {
            pages
                .iter()
                .find(|p| p.rel == rel)
                .and_then(|p| p.card_image.clone())
        };
        assert_eq!(
            img("posts/a.tmd").as_deref(),
            Some("https://cdn.example.com/card.png"),
            "an absolute image URL must pass through untouched"
        );
        assert_eq!(
            img("posts/b.tmd").as_deref(),
            Some("posts/thumb.webp"),
            "a local image stays resolved site-root-relative"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn absolute_page_image_is_verbatim_og_image_relative_joins_site_url() {
        // Batch 7: the discovery guard fixes listing cards, but the og:image builder in
        // `meta.rs` also prepended the site `url:` — so an absolute image became a broken
        // `{base}/https://…`. An absolute image must appear verbatim in og:/twitter:image
        // (and needs no `url:`); a relative image is joined onto the site url.
        use std::fs;
        let root = std::env::temp_dir().join(format!("tali-ogimg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("_site.yml"),
            "title: Demo\nurl: https://example.com\n",
        )
        .unwrap();
        fs::write(
            root.join("abs.tmd"),
            "---\ntitle: Abs\nimage: https://cdn.example.com/card.png\n---\n\nBody.\n",
        )
        .unwrap();
        fs::write(
            root.join("rel.tmd"),
            "---\ntitle: Rel\nimage: thumb.webp\n---\n\nBody.\n",
        )
        .unwrap();

        let site = Site::discover(&root);
        let head = |rel: &str| {
            let p = site.pages.iter().find(|p| p.rel == rel).expect("page");
            meta::social_head(&site, p)
        };
        assert!(
            head("abs.tmd")
                .contains(r#"property="og:image" content="https://cdn.example.com/card.png""#),
            "an absolute image passes through verbatim: {}",
            head("abs.tmd")
        );
        assert!(
            head("rel.tmd")
                .contains(r#"property="og:image" content="https://example.com/thumb.webp""#),
            "a relative image joins onto the site url: {}",
            head("rel.tmd")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn draft_yes_is_treated_as_draft_and_warns() {
        // Batch 5: `draft: yes` is a STRING in YAML 1.2 (not a bool), so it used to
        // slip through as draft=false and silently publish. It must be caught: excluded
        // like `draft: true` AND a warning to use canonical `true`.
        use std::fs;
        let root = std::env::temp_dir().join(format!("tali-draftyes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
        fs::write(
            root.join("wip.tmd"),
            "---\ntitle: WIP\ndraft: yes\n---\n\nStill cooking.\n",
        )
        .unwrap();

        let mut warnings = Vec::new();
        let rels: Vec<String> = website_pages(&root, &mut warnings)
            .iter()
            .map(|p| p.rel.clone())
            .collect();
        assert!(
            !rels.contains(&"wip.tmd".to_string()),
            "`draft: yes` must be excluded like `draft: true`: {rels:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("draft") && w.contains("YAML 1.2")),
            "a `draft: yes` page must warn to use `true`: {warnings:?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn author_404_is_honored_and_excluded_from_search() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("tali-404-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("_site.yml"), "title: Demo\n").unwrap();
        fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
        fs::write(
            root.join("404.tmd"),
            "---\ntitle: Lost\n---\n\n# Custom not found\n\nNope.\n",
        )
        .unwrap();

        let site = Site::discover(&root);
        assert!(
            site.has_author_404(),
            "a root 404.tmd is detected as the author's own 404 page"
        );
        // The author's 404 must never leak into the Cmd-K full-text index.
        assert!(
            !site.search_index_json.contains("\"u\":\"404.html\""),
            "404.html excluded from search: {}",
            site.search_index_json
        );
        // The real content page is still indexed.
        assert!(
            site.search_index_json.contains("\"u\":\"index.html\""),
            "index.html still indexed: {}",
            site.search_index_json
        );

        // A site with no 404.tmd reports false (the built-in template applies).
        let bare = std::env::temp_dir().join(format!("tali-no404-{}", std::process::id()));
        let _ = fs::remove_dir_all(&bare);
        fs::create_dir_all(&bare).unwrap();
        fs::write(bare.join("_site.yml"), "title: Demo\n").unwrap();
        fs::write(bare.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
        assert!(!Site::discover(&bare).has_author_404());

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&bare);
    }

    #[test]
    fn refresh_search_reflects_a_page_edit() {
        use std::fs;
        // A content edit in the live preview must not leave Cmd-K search frozen at the
        // page's load-time headings: after `refresh_search_for_page`, the index carries
        // the new heading and drops the old one.
        let root = write_site(
            "search-refresh",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\n# Original Heading\n\nHi.\n",
                ),
            ],
        );
        let mut site = Site::discover(&root);
        assert!(
            site.search_index_json.contains("Original Heading"),
            "the original heading is indexed at discovery: {}",
            site.search_index_json
        );

        fs::write(
            root.join("index.tmd"),
            "---\ntitle: Home\n---\n\n# Brand New Heading\n\nHi.\n",
        )
        .unwrap();
        // Resolve by url form too (exercises the rel-or-url lookup).
        site.refresh_search_for_page("index.html");
        assert!(
            site.search_index_json.contains("Brand New Heading"),
            "the edited heading is now indexed: {}",
            site.search_index_json
        );
        assert!(
            !site.search_index_json.contains("Original Heading"),
            "the stale heading is gone: {}",
            site.search_index_json
        );
        // An unknown page is a harmless no-op (doesn't panic or corrupt the index).
        let before = site.search_index_json.clone();
        site.refresh_search_for_page("does-not-exist");
        assert_eq!(before, site.search_index_json);

        let _ = fs::remove_dir_all(&root);
    }

    /// Write a throwaway site fixture (relative path → body) and return its root.
    pub(crate) fn write_site(tag: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        use std::fs;
        let root = std::env::temp_dir().join(format!("tali-omit-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for (rel, body) in files {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
        root
    }

    /// Render `rel` in `site` and return (html, render-warnings).
    fn render_page(site: &Site, rel: &str) -> (String, Vec<Warning>) {
        let page = site.pages.iter().find(|p| p.rel == rel).unwrap();
        let src = std::fs::read_to_string(&page.input).unwrap();
        let doc = crate::render::render_document_with_includes(&src, &site.root);
        site.render_page_doc_warned(page, doc)
    }

    #[test]
    fn website_cross_page_sec_ref_is_not_labelled_chapter() {
        // Batch 4 (Bug 2): a non-book website has no chapters, so a cross-page `@sec-`
        // must resolve to a bare "Section" link — never "Chapter&nbsp;1" (which happened
        // when harvest_xref_numbers filled the empty website target with the render's
        // flat per-page section counter, and rewrite read that whole number as a chapter).
        let root = write_site(
            "webxref",
            &[
                ("_site.yml", "title: Site\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\n# Home\n\nSee @sec-topic elsewhere.\n",
                ),
                (
                    "other.tmd",
                    "---\ntitle: Other\n---\n\n# Other\n\n## A topic {#sec-topic}\n\nHi.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        assert!(
            !site.is_book(),
            "a navbar-only site is a website, not a book"
        );
        let (html, _) = render_page(&site, "index.tmd");
        assert!(
            html.contains("other.html#sec-topic"),
            "cross-page @sec-topic should link to the other page: {html}"
        );
        assert!(
            !html.contains("Chapter&nbsp;1") && !html.contains(">Chapter"),
            "a website @sec- must not be mislabelled a Chapter: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn contents_dot_at_root_lists_siblings_and_warns_titleless() {
        let root = write_site(
            "dotlist",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\nlisting:\n  contents: \".\"\n---\n\n# Posts\n",
                ),
                ("a.tmd", "---\ntitle: Post A\n---\n\nA.\n"),
                ("b.tmd", "---\n# no title here\n---\n\nB.\n"),
            ],
        );
        let site = Site::discover(&root);
        let (html, warnings) = render_page(&site, "index.tmd");
        assert!(
            html.contains("Post A"),
            "root `contents: .` lists siblings: {html}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("b.tmd") && w.message.contains("no `title:`")),
            "titleless post warned: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn backlink_points_to_sole_uncapped_listing() {
        let root = write_site(
            "backlink-sole",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "blog.tmd",
                    "---\ntitle: Blog\nlisting:\n  contents: posts\n---\n\n# Blog\n",
                ),
                ("posts/one.tmd", "---\ntitle: One\n---\n\nOne.\n"),
                ("posts/two.tmd", "---\ntitle: Two\n---\n\nTwo.\n"),
            ],
        );
        let site = Site::discover(&root);
        let (post, _) = render_page(&site, "posts/one.tmd");
        assert!(
            post.contains("<nav class=\"tali-postnav tali-listing-backnav\"")
                && post.contains("href=\"../blog.html\"")
                && post.contains("</span> Blog</a>"),
            "sole un-capped listing should own the post: {post}"
        );
        // The listing page itself belongs to no listing → no backlink.
        let (blog, _) = render_page(&site, "blog.tmd");
        assert!(
            !blog.contains("<nav class=\"tali-postnav tali-listing-backnav\""),
            "the listing page should have no backlink"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_backlink_when_two_uncapped_listings_cover_the_post() {
        let root = write_site(
            "backlink-ambig",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "blog.tmd",
                    "---\ntitle: Blog\nlisting:\n  contents: posts\n---\n\n# Blog\n",
                ),
                (
                    "archive.tmd",
                    "---\ntitle: Archive\nlisting:\n  contents: posts\n---\n\n# Archive\n",
                ),
                ("posts/one.tmd", "---\ntitle: One\n---\n\nOne.\n"),
            ],
        );
        let site = Site::discover(&root);
        let (post, _) = render_page(&site, "posts/one.tmd");
        assert!(
            !post.contains("<nav class=\"tali-postnav tali-listing-backnav\""),
            "two un-capped owners are ambiguous → no backlink: {post}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn capped_preview_does_not_own_but_full_list_does() {
        // A Home page previews the newest post (max-items: 1); a Blog page lists all.
        // The capped preview must NOT count as an owner, so the post resolves uniquely
        // to the full Blog listing rather than reading as ambiguous.
        let root = write_site(
            "backlink-capped",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\nlisting:\n  contents: posts\n  max-items: 1\n---\n\n# Home\n",
                ),
                (
                    "blog.tmd",
                    "---\ntitle: Blog\nlisting:\n  contents: posts\n---\n\n# Blog\n",
                ),
                ("posts/one.tmd", "---\ntitle: One\n---\n\nOne.\n"),
                ("posts/two.tmd", "---\ntitle: Two\n---\n\nTwo.\n"),
            ],
        );
        let site = Site::discover(&root);
        let (post, _) = render_page(&site, "posts/one.tmd");
        assert!(
            post.contains("<nav class=\"tali-postnav tali-listing-backnav\"")
                && post.contains("</span> Blog</a>"),
            "capped preview should be excluded, leaving Blog as the sole owner: {post}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_backlink_when_only_a_capped_listing_covers_the_post() {
        let root = write_site(
            "backlink-cappedonly",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\nlisting:\n  contents: posts\n  max-items: 1\n---\n\n# Home\n",
                ),
                ("posts/one.tmd", "---\ntitle: One\n---\n\nOne.\n"),
                ("posts/two.tmd", "---\ntitle: Two\n---\n\nTwo.\n"),
            ],
        );
        let site = Site::discover(&root);
        let (post, _) = render_page(&site, "posts/one.tmd");
        assert!(
            !post.contains("<nav class=\"tali-postnav tali-listing-backnav\""),
            "a capped-only listing owns nothing → no backlink: {post}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_titleless_listing_host_is_not_an_owner() {
        // A listing page with no `title:` can't render a sensible "← <title>" label, so
        // it must not own posts — symmetry with the titleless-covered-page guard.
        let root = write_site(
            "backlink-titlelesshost",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "feed.tmd",
                    "---\nlisting:\n  contents: posts\n---\n\n# Feed\n",
                ),
                ("posts/one.tmd", "---\ntitle: One\n---\n\nOne.\n"),
            ],
        );
        let site = Site::discover(&root);
        let (post, _) = render_page(&site, "posts/one.tmd");
        assert!(
            !post.contains("<nav class=\"tali-postnav tali-listing-backnav\""),
            "a titleless listing host must not own the post: {post}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tmd_pages_are_discovered_with_html_urls() {
        // `.tmd` is the native (and only) source extension; the site walker must discover
        // every `.tmd` page in a project, and each page's built URL is still `.html`.
        let root = write_site(
            "tmd-native",
            &[
                ("_site.yml", "title: Demo\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\n[Next](page.tmd).\n"),
                (
                    "page.tmd",
                    "---\ntitle: Page\n---\n\nHi from a .tmd page.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let mut got: Vec<(String, String)> = site
            .pages
            .iter()
            .map(|p| (p.rel.clone(), p.url.clone()))
            .collect();
        got.sort();
        assert_eq!(
            got,
            vec![
                ("index.tmd".to_string(), "index.html".to_string()),
                ("page.tmd".to_string(), "page.html".to_string()),
            ],
            "both .tmd pages discovered with .html urls"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn listing_without_contents_warns() {
        let root = write_site(
            "nocontents",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\nlisting:\n  type: grid\n---\n\nHi.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        assert!(
            site.warnings
                .iter()
                .any(|w| w.contains("listing") && w.contains("contents")),
            "{:?}",
            site.warnings
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_chapter_file_warns() {
        let root = write_site(
            "missingch",
            &[
                (
                    "_site.yml",
                    "title: Book\nchapters:\n  - index.tmd\n  - missing.tmd\n",
                ),
                ("index.tmd", "---\ntitle: Intro\n---\n\n# Intro\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(
            site.warnings
                .iter()
                .any(|w| w.contains("missing.tmd") && w.contains("chapter file not found")),
            "{:?}",
            site.warnings
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mount_page_collision_warns() {
        let root = write_site(
            "mountcol",
            &[
                ("_site.yml", "title: Demo\nmounts:\n  docs: ../other\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nHi.\n"),
                ("docs/page.tmd", "---\ntitle: Doc\n---\n\nDoc.\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(
            site.warnings
                .iter()
                .any(|w| w.contains("mount") && w.contains("collides")),
            "{:?}",
            site.warnings
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn site_image_without_url_warns() {
        let root = write_site(
            "imgnourl",
            &[
                ("_site.yml", "title: Demo\nimage: card.png\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nHi.\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(
            site.warnings
                .iter()
                .any(|w| w.contains("image") && w.contains("url")),
            "{:?}",
            site.warnings
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn listing_card_emits_image_alt() {
        let root = write_site(
            "cardalt",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\nlisting:\n  contents: posts\n  type: grid\n---\n\n# Posts\n",
                ),
                (
                    "posts/p.tmd",
                    "---\ntitle: Post\nimage: pic.png\nimage-alt: A nice pic\n---\n\nBody.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let (html, _) = render_page(&site, "index.tmd");
        assert!(
            html.contains("alt=\"A nice pic\""),
            "card alt emitted: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn list_layout_shows_thumbnail_but_default_stays_text_only() {
        // `type: list` is a stacked (non-grid) layout that KEEPS the `image:` thumbnail
        // (reading-first feed); plain `type: default` is the same stacked layout WITHOUT
        // the thumbnail (a formal text list, e.g. a CV's projects). Both must differ only
        // in the image, and neither is the `grid` tile layout.
        let root = write_site(
            "listvsdefault",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "feed.tmd",
                    "---\ntitle: Feed\nlisting:\n  contents: posts\n  type: list\n---\n\n# Feed\n",
                ),
                (
                    "plain.tmd",
                    "---\ntitle: Plain\nlisting:\n  contents: posts\n  type: default\n---\n\n# Plain\n",
                ),
                (
                    "posts/p.tmd",
                    "---\ntitle: Post\nimage: pic.png\nimage-alt: A nice pic\n---\n\nBody.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let (feed, _) = render_page(&site, "feed.tmd");
        let (plain, _) = render_page(&site, "plain.tmd");
        // Match the emitted class ATTRIBUTE, not the inlined CSS rule names (the full
        // page bundles site.css, which mentions every class).
        // list: stacked layout, thumbnail present.
        assert!(
            feed.contains("class=\"tali-listing tali-listing-default\"")
                && !feed.contains("class=\"tali-listing tali-listing-grid\""),
            "list is a stacked (non-grid) layout: {feed}"
        );
        assert!(
            feed.contains("class=\"tali-card-img\"") && feed.contains("alt=\"A nice pic\""),
            "list keeps the thumbnail: {feed}"
        );
        // default: same stacked layout, NO thumbnail.
        assert!(
            plain.contains("class=\"tali-listing tali-listing-default\"")
                && !plain.contains("class=\"tali-card-img\""),
            "default stays text-only: {plain}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_numbers_cross_page_figure_refs() {
        let root = write_site(
            "xrefharvest",
            &[
                (
                    "_site.yml",
                    "title: Book\nchapters:\n  - a.tmd\n  - b.tmd\n",
                ),
                (
                    "a.tmd",
                    "---\ntitle: Alpha\n---\n\nSee @fig-plot for the result.\n",
                ),
                (
                    "b.tmd",
                    "---\ntitle: Beta\n---\n\n![A scatter plot](plot.png){#fig-plot}\n",
                ),
            ],
        );
        // The source-scan knows fig-plot's PAGE but not its NUMBER (figure numbers exist
        // only after render); `discover`'s harvest fills it, so the cross-page ref is
        // numbered in the live preview too, not only in the static build.
        let site = Site::discover(&root);
        let html = site.render_page("a.tmd").unwrap();
        assert!(
            html.contains("<a href=\"b.html#fig-plot\" class=\"tali-xref\">Figure&nbsp;1</a>"),
            "cross-page figure ref numbered after discover: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_numbers_cross_page_theorem_refs() {
        // A theorem is always a literal `::: {.theorem}` div in source, but its NUMBER is
        // assigned only during render. A cross-page `@thm-` ref must therefore show the
        // harvested number ("Theorem 1"), not a bare "Theorem" label — in the live
        // preview (plain `discover`, no explicit `harvest_xref_numbers`), not only in the
        // static build.
        let root = write_site(
            "xrefthm",
            &[
                (
                    "_site.yml",
                    "title: Book\nchapters:\n  - a.tmd\n  - b.tmd\n",
                ),
                (
                    "a.tmd",
                    "---\ntitle: Alpha\n---\n\nThe result rests on @thm-key.\n",
                ),
                (
                    "b.tmd",
                    "---\ntitle: Beta\n---\n\n::: {.theorem #thm-key}\nThe statement holds.\n:::\n",
                ),
            ],
        );
        // `discover` alone (what the live preview uses) must number the cross-page ref.
        let site = Site::discover(&root);
        let html = site.render_page("a.tmd").unwrap();
        assert!(
            html.contains("<a href=\"b.html#thm-key\" class=\"tali-xref\">Theorem&nbsp;1</a>"),
            "cross-page theorem ref numbered after discover: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
