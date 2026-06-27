//! Multi-page website project model.
//!
//! A *site* is a directory with one explicit root config (`_site.yml`, kept for
//! Quarto compatibility) plus a set of `.qmd` input pages. This module owns the
//! project-level concerns that the single-page path never had:
//!
//!   - parsing the root config (navbar / footer / title) into a typed [`SiteConfig`],
//!   - discovering input pages and mapping each to its output URL (`.qmd` → `.html`),
//!   - the page order used for book chapter prev/next navigation,
//!   - building the shared chrome (navbar, footer, book prev/next) injected into pages,
//!   - rewriting intra-site `.qmd` links to their built `.html` targets.
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
    /// Discover the site rooted at `root`: parse `_site.yml`, enumerate input
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
                String::new()
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
        doc.toc = self.page_toc(page, doc.toc_explicit);
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
        // Build the registry once: url -> (ids, has_executable_cells).
        let mut ids_by_url: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
        let mut cells_by_url: HashMap<String, bool> = HashMap::new();
        for page in &self.pages {
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let base = page.input.parent().unwrap_or(&self.root);
            let doc = render::render_document_with_includes(&src, base);
            let mut ids = std::collections::HashSet::new();
            for b in &doc.blocks {
                collect_html_ids(&b.html, &mut ids);
            }
            cells_by_url.insert(
                page.url.clone(),
                doc.blocks.iter().any(|b| b.cell.is_some()),
            );
            ids_by_url.insert(page.url.clone(), ids);
        }

        let mut out = Vec::new();
        for page in &self.pages {
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let base = page.input.parent().unwrap_or(&self.root);
            let doc = render::render_document_with_includes(&src, base);
            for b in &doc.blocks {
                let line = sourcepos_start_line(&b.sourcepos);
                for (path, frag) in manual_local_links(&b.html) {
                    // Resolve to a site-root-relative `.html` url. `.qmd`→`.html`, then
                    // join against the page's directory. A link that climbs *above* the
                    // site root (`../other-book/…`, a mounted sibling) is unresolvable
                    // offline and deliberately skipped — only the marketing site that
                    // mounts both books can resolve it, so flagging it here would be a
                    // false positive (cross-book/mount links are written as relative
                    // `.html` by design; see docs/ CLAUDE.md).
                    let Some(target_url) = join_rel_in_root(&page.url, &qmd_to_html(path)) else {
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
                        if self.decks.iter().any(|d| d.url == target_url)
                            || self.root.join(&target_url).is_file()
                            || self.root.join(html_to_qmd(&target_url)).is_file()
                        {
                            continue;
                        }
                        let w = Warning::new(format!(
                            "broken link: `{path}` resolves to `{target_url}`, which is no page in this site"
                        ));
                        out.push((
                            page.rel.clone(),
                            match line {
                                Some(l) => w.at(b.source_file.clone(), l),
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
                            page.rel.clone(),
                            match line {
                                Some(l) => w.at(b.source_file.clone(), l),
                                None => w,
                            },
                        ));
                    }
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
        self.expand_page(page, blocks);
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
        render::render_doc_to_page(&doc, "Page not found", render::OutputMode::Build)
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
        // the filter; the filter also reads these badges to know a card's categories.
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
        // No delimited `data-categories` list: the client filter reads each card's
        // own `.qmd-cat[data-cat]` badges (exact names), so a category name
        // containing a comma still matches.
        // `data-qmd-src` lets the click-to-source locator jump to the post's source
        // (it's site-root-relative; resolved client-side, inert in the static build).
        format!(
            "<a class=\"qmd-card\" href=\"{href}\" data-qmd-src=\"{src}\">{img}\
             <div class=\"qmd-card-body\">{date}<h3 class=\"qmd-card-title\">{title}</h3>{desc}{cats}</div></a>",
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

/// Prefix each heading in a book chapter with its section number: the chapter's
/// `# H1` becomes "N", and the deeper headings count within it ("N.1", "N.1.1"),
/// emitted as a `qmd-section-number` span.
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

/// Insert a `qmd-section-number` span just after a heading's opening tag.
fn prefix_heading_number(html: &str, number: &str) -> String {
    match html.find('>') {
        Some(i) => format!(
            "{}<span class=\"qmd-section-number\">{number}</span> {}",
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
        .filter_map(|input| {
            let rel = rel_str(root, &input);
            let url = qmd_to_html(&rel);
            let fm = parse_front_matter(&input);
            // `draft: true` excludes the page from the build entirely — and, because
            // listings + prev/next nav derive from `self.pages`, from those too.
            if fm.draft {
                return None;
            }
            // `image` is relative to the page's own directory; store it
            // site-root-relative so a listing card on another page can link it.
            let card_image = fm.image.map(|img| join_rel(&rel, &img));
            Some(Page {
                input,
                rel,
                url,
                title: fm.title,
                date: fm.date,
                description: fm.description,
                card_image,
                categories: fm.categories,
                listings: fm.listings,
                about: fm.about,
                hero: fm.hero,
                page_layout: fm.page_layout,
            })
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
        // Expand `{{< include >}}` first: an `{{< embed >}}` living inside an included
        // partial must be discovered too (else the deck flattens to an article + leaks
        // into search). The embed path stays relative to the embedding page.
        let base = page.input.parent().unwrap_or(root);
        let (src, _origins) = crate::includes::resolve(&src, base);
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
    // `.qmd`→`.html` on the path component, fragment preserved (a non-`.qmd` path
    // round-trips unchanged through `qmd_to_html`).
    qmd_href(val)
}

/// Whether a block's *leading element tag* carries `id="x"` (so a `::: {#x}`
/// placeholder matches, but a code sample or prose that merely contains the text
/// `id="x"` in its body does not).
fn block_tag_has_id(html: &str, id: &str) -> bool {
    let needle = format!("id=\"{id}\"");
    // Quote-aware tag end, so a raw-HTML placeholder whose leading tag has a `>`
    // inside an attribute value (e.g. `<div title="a > b" id="x">`) is handled.
    match crate::render::tag_end(html) {
        Some(gt) => html[..gt].contains(&needle),
        None => html.contains(&needle),
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

/// Like [`join_rel`] but returns `None` when `target` climbs *above* the file's directory
/// (`../` past the site root). A root-escaping link points at a sibling project/mount the
/// single-site registry can't see, so the cross-page link checker skips it rather than
/// false-flag a legitimate cross-book link.
fn join_rel_in_root(from_rel: &str, target: &str) -> Option<String> {
    if target.starts_with('/') {
        return Some(target.trim_start_matches('/').to_string());
    }
    let dir = from_rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut parts: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?; // None when the link climbs above the site root
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

/// `.html`→`.qmd` on a url path (`x.html` → `x.qmd`), so the checker can test whether a
/// link target is backed by a source file on disk. A non-`.html` path round-trips.
fn html_to_qmd(url: &str) -> String {
    match url.strip_suffix(".html") {
        Some(stem) => format!("{stem}.qmd"),
        None => url.to_string(),
    }
}

/// The 1-based start line from a block's `sourcepos` (`"startLine:col-…"`), if positive.
/// A local copy of `diagnostics::start_line` (that one is private to its module); used to
/// locate cross-page link warnings to their source line.
fn sourcepos_start_line(sourcepos: &str) -> Option<u32> {
    sourcepos
        .split(':')
        .next()?
        .parse::<u32>()
        .ok()
        .filter(|&l| l > 0)
}

/// Every `id="…"` value in a block's HTML, added to `out` (the page's anchor set for the
/// cross-page link check). Plain substring scan, matching how `search`/`diagnostics` read ids.
fn collect_html_ids(html: &str, out: &mut std::collections::HashSet<String>) {
    let needle = "id=\"";
    let mut i = 0;
    while let Some(pos) = html[i..].find(needle) {
        let start = i + pos + needle.len();
        let Some(len) = html[start..].find('"') else {
            break;
        };
        out.insert(html[start..start + len].to_string());
        i = start + len;
    }
}

/// Manual relative `<a href>` links in a block's HTML, as `(path, Option<fragment>)`.
/// External (`http(s)://`, `//`, `mailto:`, `tel:`), data-URI, empty, bare in-page
/// `#frag`, and cross-reference (`qmd-xref`) links are skipped — the cross-page checker
/// only resolves intra-site file links (anchors handled per target page). The path keeps
/// its authored form (`other.qmd`, `../sec/page.html`); the fragment is split off.
fn manual_local_links(html: &str) -> Vec<(&str, Option<&str>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find("<a ") {
        let tag_start = i + pos;
        let Some(rel_end) = html[tag_start..].find('>') else {
            break;
        };
        let tag = &html[tag_start..tag_start + rel_end];
        i = tag_start + rel_end + 1;
        if tag.contains("qmd-xref") {
            continue;
        }
        let Some(hpos) = tag.find("href=\"") else {
            continue;
        };
        let vstart = hpos + "href=\"".len();
        let Some(vlen) = tag[vstart..].find('"') else {
            continue;
        };
        let val = &tag[vstart..vstart + vlen];
        // Skip external / non-file / bare-anchor links.
        if val.is_empty()
            || val.starts_with('#')
            || val.starts_with("//")
            || val.contains("://")
            || val.starts_with("data:")
            || val.starts_with("mailto:")
            || val.starts_with("tel:")
            || val.starts_with("vscode:")
        {
            continue;
        }
        let (path, frag) = match val.split_once('#') {
            Some((p, f)) => (p, Some(f)),
            None => (val, None),
        };
        if !path.is_empty() {
            out.push((path, frag));
        }
    }
    out
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

    #[test]
    fn join_rel_in_root_resolves_and_rejects_escapes() {
        // In-site sibling + nested resolve to a site-root-relative url.
        assert_eq!(
            join_rel_in_root("posts/x/index.html", "../y/index.html").as_deref(),
            Some("posts/y/index.html")
        );
        assert_eq!(
            join_rel_in_root("index.html", "about.html").as_deref(),
            Some("about.html")
        );
        assert_eq!(
            join_rel_in_root("index.html", "/abs.html").as_deref(),
            Some("abs.html")
        );
        // A link climbing ABOVE the site root (a sibling book / mount) is rejected, so the
        // cross-page checker skips it rather than false-flag a legitimate cross-book link.
        assert_eq!(
            join_rel_in_root("index.html", "../internals/index.html"),
            None
        );
        assert_eq!(
            join_rel_in_root("guide/index.html", "../../escape.html"),
            None
        );
    }

    #[test]
    fn manual_local_links_skips_external_anchor_and_xref() {
        let html = r##"<a href="other.qmd">o</a> <a href="page.html#sec">p</a> <a href="https://x.com">e</a> <a href="#top">t</a> <a href="x.html" class="qmd-xref">r</a>"##;
        let links = manual_local_links(html);
        assert_eq!(links, vec![("other.qmd", None), ("page.html", Some("sec"))]);
    }

    #[test]
    fn website_pages_excludes_drafts() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("qmd-draft-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("index.qmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
        fs::write(
            root.join("published.qmd"),
            "---\ntitle: Pub\n---\n\nPublished.\n",
        )
        .unwrap();
        fs::write(
            root.join("wip.qmd"),
            "---\ntitle: WIP\ndraft: true\n---\n\nWork in progress.\n",
        )
        .unwrap();

        let rels: Vec<String> = website_pages(&root).iter().map(|p| p.rel.clone()).collect();
        assert!(rels.contains(&"index.qmd".to_string()), "kept: {rels:?}");
        assert!(
            rels.contains(&"published.qmd".to_string()),
            "kept: {rels:?}"
        );
        assert!(
            !rels.contains(&"wip.qmd".to_string()),
            "draft excluded: {rels:?}"
        );

        let _ = fs::remove_dir_all(&root);
    }
}
