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

use crate::render::{
    self, Block, Severity, SiteCtx, Warning, block_heading_level, escape_attr as esc,
};

/// Whether discovery keeps `draft: true` pages (`Include`, the preview view) or drops
/// them from the page set (`Exclude`, the published view: build/publish/check/map).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftMode {
    Exclude,
    Include,
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
    /// Front-matter `image`, resolved to a site-root-relative path (for cards).
    pub card_image: Option<String>,
    /// Front-matter `image-alt`: alt text for the listing card image (a11y). `None`
    /// falls back to empty alt (a decorative card image).
    pub card_image_alt: Option<String>,
    /// Front-matter `categories` (shown as badges on a card).
    pub categories: Vec<String>,
    /// `listing:` blocks declared on this page (the blog index, projects, etc.).
    pub listings: Vec<ListingSpec>,
    /// `hero:` landing block (headline + lead + CTAs), if declared. Replaces the
    /// title block.
    pub hero: Option<HeroSpec>,
    /// `page-layout:` (`full` widens the content column; default reading width).
    pub page_layout: Option<String>,
    /// `draft: true` in front matter. `false` for every published page; `true` only for a
    /// draft surfaced in `DraftMode::Include` (preview). Drives the DRAFT badge/banner; a
    /// built page is always `false`, so those affordances are inert in a build.
    pub draft: bool,
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
    /// The project-wide `bibliography:` (`_site.yml`), resolved once at discovery against
    /// the site root: readable absolute `.bib` paths, in declaration order. Empty for a
    /// project that declares none. Laid **under** each page's own `bibliography:`, so a
    /// page can override a shared entry (`site::bibliography`).
    pub bibliography: Vec<PathBuf>,
    /// Warnings gathered during discovery (bad config, etc.), surfaced by the
    /// caller (build logs / preview diagnostics).
    pub warnings: Vec<String>,
    /// Inlinable JSON of every page's title + anchored headings, so the Cmd-K
    /// palette searches the whole project (`window.TALIESIN_SEARCH_INDEX`). Assembled
    /// from `search_sections`; the dev server rebuilds it whole whenever a cross-reference
    /// anchor moves, so a snippet never contradicts the page it links to.
    pub search_index_json: String,
    /// The per-page fragments `search_index_json` is assembled from — `(page rel, that
    /// page's JSON entries)` in page order — kept so an edited page's entries can be
    /// re-extracted without re-rendering the whole site.
    search_sections: Vec<(String, String)>,
    /// Rel paths of `draft: true` pages dropped in `DraftMode::Exclude` (empty in
    /// `Include`). Drives the build's "N drafts not published" report.
    pub excluded_drafts: Vec<String>,
    /// True when this is a one-document project synthesized by
    /// [`Site::discover_single`] because the file's own parent directory has no
    /// `_site.yml`. The check is local to that one directory, not a walk up the
    /// tree: a caller that invokes `discover_single` on a file already nested
    /// inside a real project still gets `standalone: true` here (harmlessly, e.g.
    /// `crates/server/src/query.rs`'s `map`, which never reads this field).
    /// `preview`/`build` only reach `discover_single` after their own ancestor
    /// walk ([`enclosing_site_root`]) found no `_site.yml` anywhere above the
    /// file, which is what makes the field mean "no project at all" for them.
    ///
    /// Such a document belongs to no project, so it gets no project chrome: the navbar
    /// would brand it "Home" and link to the page you are already on, the burger would
    /// open an empty nav, and the footer would credit a site that does not exist.
    /// `build <file>` has never emitted any of it; this is what makes `preview` agree.
    pub standalone: bool,
}

mod book;
mod chrome;
pub use book::{Book, BookEntry};
use book::{book_pages, build_book, chapter_heading};
mod bibliography;
pub(crate) use bibliography::shared_for_single_doc;
mod feed;
mod meta;
mod search;
mod seo;
mod xref;
use xref::scan_xref_targets;
pub use xref::{
    ScannedAnchor, XrefTarget, anchors_defined_elsewhere_in_project, scan_page_anchors,
    xref_anchors_in,
};
mod config;
mod frontmatter;
pub use config::*;
pub(crate) use frontmatter::*;
mod chapter;
pub(crate) use chapter::ChapterNumbering;
use chapter::number_chapter_headings; // also used by xref.rs (via `use super::*`)
mod discovery;
// `collect_pages` is not called here: `xref.rs` reaches it through this binding (a
// private `use` is still visible to a descendant module), so the project-wide anchor
// scan walks exactly the page set discovery does.
pub use discovery::collect_pages;
use discovery::website_pages;
/// Minimum number of `toc_entry_count` headings for a site-wide `toc: true` to render the
/// sidebar TOC (the auto-gate in [`Site::page_toc`]). Below this a page reads as one column.
const MIN_TOC_HEADINGS: usize = 3;
mod links;
pub use links::rewrite_tmd_links;
use links::{
    block_tag_has_id, collect_html_ids, href_matches_page, html_to_tmd, is_external_or_special,
    join_rel, join_rel_in_root, manual_local_links, resolve_href, sourcepos_start_line,
    tmd_to_html,
};

/// Walk up from `start` (a directory) for an enclosing `_site.yml`, stopping at a `.git`
/// boundary or the filesystem root, so a tool handed ONE file can still find the project it
/// belongs to. Returns the directory holding the `_site.yml`, if any. The `.git` stop keeps
/// the walk from climbing out of the project the file lives in.
pub fn enclosing_site_root(start: &Path) -> Option<PathBuf> {
    walk_up_for_site_yml(start, true)
}

/// The same walk, but climbing **past** a `.git` boundary.
///
/// This exists because the two behaviours were separately implemented and silently differed
/// for a year: `xref.rs` carried its own copy with no `.git` stop. Measured on a fixture with
/// an `_site.yml` above a `.git`, one answered the project and the other answered `None`. The
/// bodies are now one function and the difference is this parameter, so it is a choice a
/// reader can see rather than a divergence nobody knew about.
///
/// The unbounded form is what [`xref::anchors_defined_elsewhere_in_project`] wants: it runs on
/// the editor's every-keystroke diagnostic path, where wrongly deciding a page has no project
/// turns every legitimate cross-page reference into a broken-reference squiggle, which is the
/// exact harm that function was written to stop.
///
/// Public for the same reason: the editor's project walk (`lsp_project`) has to answer "what
/// project is this page in" the *same* way the diagnostics do. Two answers inside one editor
/// session means a reference that resolves in the squiggle and not under F12.
pub fn enclosing_site_root_across_git(start: &Path) -> Option<PathBuf> {
    walk_up_for_site_yml(start, false)
}

/// The one walk both spellings share. `stop_at_git` chooses whether a `.git` directory ends
/// the climb (guarding against an unrelated ancestor `_site.yml` adopting the document) or is
/// walked through.
fn walk_up_for_site_yml(start: &Path, stop_at_git: bool) -> Option<PathBuf> {
    let mut dir = start.canonicalize().ok()?;
    loop {
        if dir.join("_site.yml").is_file() {
            return Some(dir);
        }
        if stop_at_git && dir.join(".git").exists() {
            return None;
        }
        dir = dir.parent()?.to_path_buf();
    }
}

/// One outgoing local link found in a rendered page, kept with enough context to locate a
/// warning back to the source line that wrote it.
struct LinkRef {
    path: String,
    frag: Option<String>,
    line: Option<u32>,
    source_file: Option<String>,
}

/// Everything one render of a page contributes to cross-page link validation: the ids it
/// defines (link targets), whether it runs cells (a cell can emit an id at runtime, so its
/// anchors are never reported missing), and the links it points outward.
struct PageLinkFacts {
    rel: String,
    url: String,
    ids: std::collections::HashSet<String>,
    has_cells: bool,
    links: Vec<LinkRef>,
}

impl Site {
    /// Discover the site rooted at `root` (published view): parse `_site.yml`, enumerate
    /// input `.tmd` pages, and compute their output URLs + ordering. `draft: true` pages
    /// are excluded and recorded in [`Site::excluded_drafts`]. Used by build/publish/
    /// check/map/query.
    pub fn discover(root: &Path) -> Site {
        Self::discover_with(root, DraftMode::Exclude)
    }

    /// Like [`discover`](Self::discover) but with an explicit [`DraftMode`]: `Include`
    /// keeps `draft: true` pages in the page set (tagged `Page.draft`) so the live
    /// **preview** shows them in nav/listings/prev-next; `Exclude` is the published view.
    pub fn discover_with(root: &Path, drafts: DraftMode) -> Site {
        Self::discover_scoped(root, drafts, None)
    }

    /// The project for a single `.tmd` previewed on its own: its parent directory carrying
    /// exactly that one document, plus whatever that document `{{< embed >}}`s.
    ///
    /// This is what `taliesin preview <file.tmd>` builds when the file has no ancestor
    /// `_site.yml`. Scoping to the one file (rather than discovering the whole parent
    /// directory) is the point: previewing a scratch note must not pull thirty unrelated
    /// siblings into the nav, and must not parse them to find that out.
    pub fn discover_single(file: &Path) -> Site {
        let root = file.parent().unwrap_or_else(|| Path::new("."));
        Self::discover_scoped(root, DraftMode::Include, Some(file))
    }

    /// [`discover_with`](Self::discover_with), optionally narrowed to one document
    /// (see [`discover_single`](Self::discover_single)). The narrowing happens before
    /// cross-references and the search index are computed, so every downstream artifact is
    /// built from the scoped page set rather than filtered afterwards.
    fn discover_scoped(root: &Path, drafts: DraftMode, only: Option<&Path>) -> Site {
        let mut warnings = Vec::new();
        let mut excluded_drafts = Vec::new();
        let config = load_config(root, &mut warnings);

        // A book takes its page set + order from the explicit `chapters:` list;
        // a website discovers every `.tmd` and orders by path.
        let (mut pages, book) = if config.is_book {
            let book = build_book(root, &config, drafts, &mut excluded_drafts);
            let pages = book_pages(root, &book, &mut warnings);
            (pages, Some(book))
        } else {
            (
                website_pages(root, drafts, &mut warnings, &mut excluded_drafts),
                None,
            )
        };

        // Scoped to one document: drop every other page BEFORE xrefs/search are computed,
        // so they are built from the one page and not filtered after the fact.
        if let Some(only) = only {
            let want = only.canonicalize().unwrap_or_else(|_| only.to_path_buf());
            pages.retain(|p| p.input.canonicalize().unwrap_or_else(|_| p.input.clone()) == want);
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

        // Resolve the site-wide `head:` include once, relative to the site root (where
        // `_site.yml` and any file it references live).
        let includes = render::includes_from_parts(
            config.head.as_ref(),
            Some(root),
            // The site root is the explicit containment boundary (equivalent to the
            // `_site.yml`-marker walk, but not dependent on it): a head include stays
            // inside the project.
            Some(root),
        );

        // Likewise once, and against the site root: a project-wide `.bib` path is written
        // relative to `_site.yml`, not to whichever page happens to be rendering, and a bad
        // one should be reported once rather than on every page.
        let bibliography = bibliography::resolve_shared(root, &config.bibliography, &mut warnings);

        let xref_targets = scan_xref_targets(&pages, &book, &mut warnings);

        let standalone = only.is_some() && !root.join("_site.yml").is_file();

        let mut site = Site {
            root: root.to_path_buf(),
            config,
            pages,
            book,
            xref_targets,
            includes,
            bibliography,
            warnings,
            // Both are built below, once the registry's numbers exist: the search index
            // READS `xref_targets`, so building it here (as it used to) indexed every
            // cross-page `@fig-` before a single number had been harvested.
            search_index_json: String::new(),
            search_sections: Vec::new(),
            excluded_drafts,
            standalone,
        };
        // Fill the cross-PAGE numbers the lightweight source-scan can't know — a figure /
        // equation / table / listing / theorem number is assigned only during render, so
        // `scan_xref_targets` left it empty. Harvesting here (not only in `build`) means the
        // live preview also renders "Theorem 2.1" / "Figure 2.3" for a cross-page ref instead
        // of a bare label. A pure render pass with no kernel execution, run once per
        // discover so build, preview, and `check` resolve numbers identically.
        site.harvest_xref_numbers();
        // LAST, and the ordering is load-bearing: the Cmd-K index resolves each page's
        // cross-page refs against `xref_targets`, so it has to run after the harvest above
        // has put the numbers there. Built before it, every cross-page `@fig-` was indexed
        // as a bare "Figure" and the snippet contradicted the page it linked to.
        site.rebuild_search_index();
        site
    }

    /// Rebuild the whole Cmd-K index from the pages' current sources, against the CURRENT
    /// registry. Separate from `discover` so the ordering requirement above has one name,
    /// and so [`refresh_xrefs`](Self::refresh_xrefs) can be followed by it — which the dev
    /// server does whenever a target MOVES. Whole-index, because the index is GLOBAL (one
    /// `search-index.js` for every tab): a per-page refresh keyed on the open tabs leaves a
    /// renumbered figure stale in the fragments of pages nobody has open, which is the
    /// exact snippet-contradicts-its-target defect this index ordering exists to prevent.
    pub fn rebuild_search_index(&mut self) {
        self.search_sections = search::build_sections(
            &self.pages,
            &self.book,
            &self.xref_targets,
            Some(&self.render_defaults()),
        );
        self.search_index_json = search::assemble(&self.search_sections);
    }

    /// Whether this project is a book (`project: type: book`).
    pub fn is_book(&self) -> bool {
        self.book.is_some()
    }

    /// The output directory `build` writes to: `_site`, or `_book` for a book.
    ///
    /// Not configurable. The `output:` key was retired on 2026-08-02 because both configs
    /// that set it wrote the value this returns anyway; `build --out <dir>` is the way to
    /// put the build somewhere else, and it does not need the config's permission.
    pub fn output_dir(&self) -> &str {
        if self.is_book() { "_book" } else { "_site" }
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

    /// The page whose source file is `input`, or `None` when this project publishes no such
    /// page — which is a real answer, not a lookup failure: a deck held out of `pages`, a
    /// `draft: true` chapter, an `_includes/` partial and a file in a sibling directory all
    /// land here, and none of them may be linted as a page of this site.
    ///
    /// Compared by canonical path, because the caller's path and `Page.input` reach this
    /// from different directions (a CLI argument as typed, an editor's absolute URI, a root
    /// joined during discovery) and `a/../b.tmd` is the same file as `b.tmd`.
    pub fn page_for_input(&self, input: &Path) -> Option<&Page> {
        let want = input.canonicalize().ok()?;
        self.pages
            .iter()
            .find(|p| p.input.canonicalize().ok().as_deref() == Some(&want))
    }

    /// This page is the site's root index: its `<title>` stays the bare site name (no
    /// " · {site}" suffix). One definition, so [`page_chrome`](Self::page_chrome)'s
    /// `SiteCtx` and [`page_title`](Self::page_title) cannot disagree about which page
    /// is home.
    fn is_home(&self, page: &Page) -> bool {
        page.url == "index.html"
    }

    /// The display-ready `<title>` for one of this site's pages: the doc's own title, else
    /// this page's authored title, else its leading `# H1`, plus the site-name suffix.
    ///
    /// Exists so the live preview can resolve a tab title on every rebuild WITHOUT building
    /// the page chrome: `page_chrome` renders the navbar, footer, an O(chapters) book
    /// sidebar, the social/JSON-LD meta and a `PageIncludes` clone, and the preview would
    /// throw all of it away to read two scalars — under the site lock, which page serving
    /// and `/search-index.js` also wait on. Resolves identically to the static build
    /// (`render_page_doc_warned`), which reaches the same helper through `SiteCtx`.
    pub fn page_title(&self, page: &Page, doc: &render::RenderedDoc) -> String {
        render::site_page_title(
            doc,
            page.title.as_deref().unwrap_or(""),
            self.config.title.as_deref().unwrap_or(""),
            self.is_home(page),
        )
    }

    /// Build the chrome (navbar, footer, post-nav) for a page, with links
    /// resolved relative to that page's depth. Shared by the static build and the
    /// live preview so both render identical navigation.
    pub fn page_chrome(&self, page: &Page) -> SiteCtx {
        let depth = page.url.matches('/').count(); // links are relative to the page
        // Same resolution `logo:` uses (`chrome::site_asset_href`): climb to the site root
        // for a project-relative path, leave a site-absolute or external one as written.
        let favicon = match &self.config.favicon {
            Some(f) if !f.is_empty() => chrome::site_asset_href(f, &"../".repeat(depth)),
            _ => String::new(),
        };
        let book = self.is_book();
        // Per-page OpenGraph / Twitter-card / SEO meta, so a shared link renders a
        // rich preview. Injected via the head include (no render/mod.rs change).
        let mut includes = self.includes.clone();
        // A draft page (only reachable in preview — a built page is never `draft`) gets a
        // quiet top-of-body banner so the author knows it won't publish. Read-only view
        // affordance; no source write-back.
        if page.draft {
            includes.before_body.insert_str(
                0,
                "<div class=\"tali-draft-banner\" role=\"status\">Draft: not published</div>",
            );
        }
        includes.in_header.push_str(&meta::social_head(self, page));
        includes.in_header.push_str(&meta::feed_head(self));
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
        SiteCtx {
            // A book replaces the top navbar with a slim topbar + off-canvas chapter
            // drawer and uses chapter prev/next instead of the post "back to listing" link.
            navbar_html: if book || self.standalone {
                String::new()
            } else {
                self.navbar_html(page, depth)
            },
            footer_html: if self.standalone {
                String::new()
            } else {
                self.footer_html(depth)
            },
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
            // The `<title>` suffix names the site on inner tabs; the root index stays bare.
            site_name: self.config.title.clone().unwrap_or_default(),
            is_home: self.is_home(page),
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
        let doc = render::render_document_scoped_with_site(
            &src,
            base,
            self.chapter_for(page),
            Some(&self.render_defaults()),
        );
        Some(self.render_page_doc(page, doc))
    }

    /// Finish a page whose `doc.blocks` are already produced — and possibly
    /// code-executed (the static build runs cells, then calls this): apply the
    /// site front-matter expansion (`listing:`), wrap in chrome, and
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
        // Inline single-file page build: no `_assets/`, and no book archive alongside it.
        let ctx = self.page_chrome(page);
        let fallback = page.title.as_deref().unwrap_or("");
        let html = render::html_page_from_doc_in_site(&doc, fallback, &ctx);
        (rewrite_tmd_links(&html), warnings)
    }

    /// Render a page linking the shared `_assets/` bundle (the multi-page build path).
    /// Identical to [`Self::render_page_doc_warned`] except for the asset delivery.
    pub fn render_page_doc_external(
        &self,
        page: &Page,
        mut doc: render::RenderedDoc,
        assets: render::ExternalAssets,
    ) -> (String, Vec<Warning>) {
        doc.toc = self.page_toc(page, doc.toc_explicit, &doc.blocks);
        let mut warnings = std::mem::take(&mut doc.warnings);
        self.finish_blocks(page, &mut doc.blocks, &mut warnings);
        let ctx = self.page_chrome(page);
        let fallback = page.title.as_deref().unwrap_or("");
        let html = render::html_page_from_doc_in_site_external(&doc, fallback, &ctx, assets);
        (rewrite_tmd_links(&html), warnings)
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
    #[allow(rustdoc::private_intra_doc_links)]
    pub fn validate_cross_page_links(&self) -> Vec<(String, Warning)> {
        let facts: Vec<PageLinkFacts> = self
            .pages
            .iter()
            .filter_map(|p| self.page_link_facts(p))
            .collect();
        self.resolve_link_warnings(&facts, &facts)
    }

    /// [`validate_cross_page_links`](Self::validate_cross_page_links) for ONE page, rendering
    /// only that page and the pages it actually links to.
    ///
    /// The live preview re-runs this on every save of a site/book page, and it used to call
    /// the whole-site version and throw away every other page's findings — an
    /// O(pages x blocks-per-page) render of the entire site to keep the warnings of one page
    /// (AP1/PERF-1: ~30 ms per pass on the 17-page `tech-blog`, extrapolating to ~350 ms at
    /// 200 pages, with no cliff to notice it by). A page's own links can only be validated
    /// against the pages they point at, so that is all this renders: one page plus a handful.
    ///
    /// Semantics are identical to filtering the whole-site result to `page_rel`. It renders
    /// every *registered* page this one links to, so "no ids for this url" still means
    /// exactly "not a page in this site" — the distinction the broken-link branch turns on.
    pub fn validate_cross_page_links_for(&self, page_rel: &str) -> Vec<Warning> {
        self.cross_page_links_for(page_rel, None)
    }

    /// [`validate_cross_page_links_for`](Self::validate_cross_page_links_for) judging `src`
    /// as the source page's content instead of the file on disk.
    ///
    /// This is what the editor needs and the disk-reading form cannot give it: an unsaved
    /// buffer is the only version that exists, so linting the saved file would report a link
    /// the author already deleted and miss the one they just typed. Only the SOURCE page is
    /// substituted — the pages it points at are read from disk, which is also what the
    /// preview does, and is right for the same reason: another page's unsaved buffer is not
    /// something this side can see.
    pub fn validate_cross_page_links_for_src(&self, page_rel: &str, src: &str) -> Vec<Warning> {
        self.cross_page_links_for(page_rel, Some(src))
    }

    fn cross_page_links_for(&self, page_rel: &str, src: Option<&str>) -> Vec<Warning> {
        let Some(page) = self.page(page_rel) else {
            return Vec::new();
        };
        let Some(source) = (match src {
            Some(src) => self.page_link_facts_from_src(page, src),
            None => self.page_link_facts(page),
        }) else {
            return Vec::new();
        };
        // The source page first (so it is also its own link target, for a `self.html#frag`),
        // then one render per distinct registered page it points at.
        let mut seen: std::collections::HashSet<String> =
            std::iter::once(source.url.clone()).collect();
        let targets: Vec<String> = source
            .links
            .iter()
            .filter_map(|lk| self.link_target_url(&source.url, &lk.path))
            .filter(|url| seen.insert(url.clone()))
            .collect();
        let mut rendered = vec![source];
        for url in targets {
            if let Some(target) = self.pages.iter().find(|p| p.url == url)
                && let Some(facts) = self.page_link_facts(target)
            {
                rendered.push(facts);
            }
        }
        self.resolve_link_warnings(&rendered[..1], &rendered)
            .into_iter()
            .map(|(_, w)| w)
            .collect()
    }

    /// Render one page once and take everything cross-page link validation needs from it:
    /// the element ids it defines, whether it runs cells, and its outgoing local links.
    /// One render, not three passes, so the ids and the links cannot disagree.
    fn page_link_facts(&self, page: &Page) -> Option<PageLinkFacts> {
        let src = std::fs::read_to_string(&page.input).ok()?;
        self.page_link_facts_from_src(page, &src)
    }

    /// [`page_link_facts`](Self::page_link_facts) over source already in hand — an editor
    /// buffer, which has no file to read.
    fn page_link_facts_from_src(&self, page: &Page, src: &str) -> Option<PageLinkFacts> {
        let base = page.input.parent().unwrap_or(&self.root);
        let doc = render::render_document_with_includes(src, base);
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
        Some(PageLinkFacts {
            rel: page.rel.clone(),
            url: page.url.clone(),
            has_cells: doc.blocks.iter().any(|b| b.cells().next().is_some()),
            ids,
            links,
        })
    }

    /// Resolve a page-relative link `path` (from the page at `from_url`) to a site-root
    /// relative `.html` url. `None` for a link that climbs above the site root.
    fn link_target_url(&self, from_url: &str, path: &str) -> Option<String> {
        // `.tmd`→`.html`, then join against the linking page's directory. A link that
        // climbs *above* the site root (`../other-book/…`, a mounted sibling) is
        // unresolvable offline and deliberately skipped — only the marketing site that
        // mounts both books can resolve it, so flagging it here would be a false positive
        // (cross-book/mount links are written as relative `.html` by design).
        let target_url = join_rel_in_root(from_url, &tmd_to_html(path))?;
        // A directory-style link (`dir/`) targets that dir's index.
        Some(if target_url.is_empty() || target_url.ends_with('/') {
            format!("{target_url}index.html")
        } else {
            target_url
        })
    }

    /// Judge every link carried by `sources` against the id/cell registry `rendered`
    /// supplies. Split from the render so the whole-site and single-page entry points share
    /// one copy of the resolution rules and cannot drift.
    fn resolve_link_warnings(
        &self,
        sources: &[PageLinkFacts],
        rendered: &[PageLinkFacts],
    ) -> Vec<(String, Warning)> {
        let ids_by_url: HashMap<&str, &std::collections::HashSet<String>> =
            rendered.iter().map(|f| (f.url.as_str(), &f.ids)).collect();
        let cells_by_url: HashMap<&str, bool> = rendered
            .iter()
            .map(|f| (f.url.as_str(), f.has_cells))
            .collect();

        let mut out = Vec::new();
        for source in sources {
            let (rel, url, links) = (&source.rel, &source.url, &source.links);
            for lk in links {
                let path = lk.path.as_str();
                let frag = lk.frag.as_deref();
                let line = lk.line;
                let source_file = &lk.source_file;
                let Some(target_url) = self.link_target_url(url, path) else {
                    continue;
                };
                let Some(target_ids) = ids_by_url.get(target_url.as_str()) else {
                    // A prefix another project supplies in the composed deploy
                    // (`_site.yml`'s `external-prefixes:`). This project cannot see those
                    // pages and never will; `tools/build-site.sh --check` resolves them for
                    // real, against the composed output, and is what pre-push and
                    // `tools/gates.sh` run. Without this the marketing site's own
                    // pre-publish gate was permanently red on 11 links that all resolve,
                    // which trains an author to ignore the one command that would catch a
                    // real one.
                    if self
                        .config
                        .external_prefixes
                        .iter()
                        .any(|p| target_url == *p || target_url.starts_with(&format!("{p}/")))
                    {
                        continue;
                    }
                    // A target outside the page registry is only "broken" if nothing
                    // on disk backs it: a raw source file that exists under the root
                    // (`notes.md`, `data.csv`) is a legitimate target, and the build ships
                    // it via `deploy_referenced_sources`.
                    if self.root.join(&target_url).is_file() {
                        continue;
                    }
                    // What must NOT excuse it: the *source* of a page discovery held back.
                    // This arm used to accept any target whose `.tmd` sat on disk, and a
                    // draft's `.tmd` sits on disk by definition — so the one link class
                    // guaranteed to 404 in the deploy was the one the gate waved through,
                    // silently, under `--strict`. It can only ever fire for an unpublished
                    // page: a published one is in `ids_by_url` and never reaches here.
                    // The reason is named, because the author is looking at a file that
                    // exists and would otherwise read "no page in this site" as a bug in
                    // the tool.
                    if let Some(src) = html_to_tmd(&target_url)
                        .into_iter()
                        .find(|p| self.root.join(p).is_file())
                    {
                        let why = if self.excluded_drafts.contains(&src) {
                            format!("`{src}` is a draft, so no page is built for it")
                        } else {
                            format!("this project does not publish `{src}`")
                        };
                        let w = Warning::new(format!(
                            "broken link: `{path}` resolves to `{target_url}`, but {why}"
                        ))
                        .severity(Severity::Error);
                        out.push((
                            rel.clone(),
                            match line {
                                Some(l) => w.at(source_file.clone(), l),
                                None => w,
                            },
                        ));
                        continue;
                    }
                    // The registry already knows the answer for the commonest broken link
                    // there is: a migrated document's links keep the old tool's extension
                    // while the renamed source sits in the same directory (item 128).
                    // Probed against `target_url` (site-root-relative, so a link from a
                    // subdirectory resolves the way the site resolves it) but *shown* as the
                    // spelling the author wrote, which is what they have to edit.
                    let renamed_source_exists = crate::ext::migrated_source_candidates(&target_url)
                        .into_iter()
                        .any(|c| self.root.join(c).is_file());
                    let hint = match crate::ext::migrated_source_candidates(path).first() {
                        Some(shown) if renamed_source_exists => {
                            format!(" (did you mean `{shown}`?)")
                        }
                        _ => String::new(),
                    };
                    let w = Warning::new(format!(
                        "broken link: `{path}` resolves to `{target_url}`, which is no page in this site{hint}"
                    ))
                    .severity(Severity::Error);
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
                    && !cells_by_url
                        .get(target_url.as_str())
                        .copied()
                        .unwrap_or(false)
                    && !target_ids.contains(frag)
                {
                    let w = Warning::new(format!(
                        "broken link anchor: `#{frag}` is no element id on `{target_url}`"
                    ))
                    .severity(Severity::Error);
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
    /// (`listing:`). The single block-finishing step shared by the static
    /// build, `render_page_doc`, and the live preview, so all three produce identical
    /// blocks (the preview used to skip `validate_xrefs`). `page_toc` is computed by
    /// the caller (it reads blocks but doesn't mutate them).
    pub fn finish_blocks(&self, page: &Page, blocks: &mut Vec<Block>, warnings: &mut Vec<Warning>) {
        self.number_chapter(page, blocks);
        self.resolve_cross_refs(blocks, &page.url);
        // Cross-refs that survived the site-wide resolution are genuinely broken.
        warnings.extend(crate::cite::validate_xrefs(blocks));
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
    ///
    /// The document itself; the two renderers below differ only in how the framework
    /// CSS/JS is delivered.
    fn not_found_doc(&self) -> render::RenderedDoc {
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
            nested: Vec::new(),
        }];
        doc
    }

    /// The built-in `404.html`, inlining the framework. See [`not_found_doc`](Self::not_found_doc)
    /// for what the page is and why its links are root-absolute.
    pub fn render_404_page(&self) -> String {
        render::render_doc_to_page(
            &self.not_found_doc(),
            "Page not found",
            render::OutputMode::Build,
        )
    }

    /// The same page for a multi-page `build <dir>`, linking the shared `_assets/` bundle
    /// instead of inlining it. This is the one page in a build that was still assembled by
    /// the inline renderer, so a site of ~26 KB pages shipped a **356 KB** 404.
    ///
    /// The hrefs the caller passes must be **root-absolute** (`/_assets/…`), for the same
    /// reason the home link is: the host serves this one file for any unknown path, so a
    /// depth-relative href resolves against whatever directory the reader guessed at. That
    /// makes the page's existing root-deploy assumption load-bearing for its styling as well
    /// as its home link — on a project-subpath deploy it degrades to unstyled rather than
    /// merely mislinking. The page keeps its own `<style>` block inline either way, so the
    /// layout survives even if the stylesheet does not resolve.
    pub fn render_404_page_external(&self, assets: render::ExternalAssets) -> String {
        render::render_doc_to_page_external(&self.not_found_doc(), "Page not found", assets)
    }

    /// Whether a page shows a table of contents: its own front-matter `toc:` wins (an
    /// explicit `toc: false` suppresses it, an explicit `toc: true` forces it on regardless
    /// of length); otherwise it is **automatic**, and an article page earns one by being
    /// long enough — the page's rendered `blocks` are counted by `render::toc_entry_count`,
    /// and a page below [`MIN_TOC_HEADINGS`] (or a listing / hero page) reads as a single
    /// column instead of getting a near-empty TOC. Used by the static build and preview alike.
    ///
    /// **There is no site-wide `toc:` any more** (retired 2026-08-02). It was a switch in
    /// front of a gate that already answers the same question per page, and it answered it
    /// worse: it could only turn TOCs off for pages that warranted one, or on for a whole
    /// project regardless. The page-level key stays because a page really can know better
    /// than a heading count — a long reference table wants no rail, a short landing essay
    /// might want one.
    ///
    /// **A book never shows one** (item 76, owner ruling 2026-07-27, reversing the
    /// 2026-07-06 "keep both nav surfaces" decision). A book already has an in-chapter
    /// outline that is *strictly more detailed* than the rail: the Chapters drawer
    /// auto-expands the current chapter and lists it to h3, where the rail listed h2 only.
    /// The gate is here, ahead of `doc_toc`, on purpose — a page-level `toc: true` must not
    /// be a hidden way to reinstate a removed surface, and putting it here keeps every
    /// assembler (both static builds, both previews) on one decision instead of four.
    /// What is lost is scrollspy; the ruling accepts that.
    pub fn page_toc(&self, page: &Page, doc_toc: Option<bool>, blocks: &[Block]) -> bool {
        if self.is_book() {
            return false;
        }
        doc_toc.unwrap_or_else(|| {
            page.listings.is_empty()
                && page.hero.is_none()
                // The gate (NN/g: show a TOC only on long, chunkable pages).
                && render::toc_entry_count(blocks) >= MIN_TOC_HEADINGS
        })
    }

    /// Resolve cross-*page* references in place: a `@sec-x` whose anchor lives on
    /// another page (left marked `data-tali-xref` by `cite`) is rewritten to link to
    /// that page and carry its number ("Section 2.1"). Same-page refs were already
    /// resolved by `cite`; an anchor unknown project-wide is left as a label link.
    /// Called by both the static build and the live preview.
    pub fn resolve_cross_refs(&self, blocks: &mut [Block], current_url: &str) {
        xref::resolve_blocks(blocks, &self.xref_targets, current_url);
    }

    /// Re-derive the whole cross-reference registry from the pages' current sources — the
    /// source scan *and* the render-harvest that numbers the floats — so a warm preview's
    /// cross-page refs track edits instead of freezing at discovery. Both producers ran only
    /// in `discover`, which left `intro.html` serving "Figure 1.2" while `methods.html`
    /// served "Figure 1.1" for that same figure, and left an anchor added after startup
    /// permanently unknown (it rendered as a dead same-page link, silently).
    ///
    /// Whole-registry rather than per-page, which the numbers alone would allow (a float's
    /// number depends only on its own page + chapter): MEASURED at 27ms for the largest real
    /// book (`docs/guide`, 20 pages; `docs/internals` 17ms, `corpus/demo-book` 0.5ms), which
    /// is not worth buying with incremental invalidation — that would have to re-derive the
    /// scan's project-wide "first definition wins" ordering to know whether a dropped anchor
    /// should fall through to another page's definition. A page-SET change re-runs `discover`
    /// anyway.
    ///
    /// Deliberately does NOT rebuild the hover index (its own second render pass over the
    /// targets): it is equally frozen today, so leaving it is no regression, and doubling
    /// this cost for hover cards wants its own measurement.
    /// ALL-OR-NOTHING: a render panic restores the previous registry. The harvest renders
    /// EVERY page, so a panic partway leaves `xref_targets` holding the raw scan map — every
    /// float number empty, every cell-labelled anchor missing — and one bad page would
    /// silently un-number cross-page refs on all the good ones, site-wide. Stale-but-numbered
    /// beats un-numbered, and the caller can't distinguish them from outside.
    pub fn refresh_xrefs(&mut self) {
        // Only the SCAN's duplicate-label warnings are dropped: `self.warnings` is
        // discovery-scoped (the server logs it once at startup and never re-reads it), so
        // re-appending them per save would grow it and surface nothing. The harvest still
        // pushes its own, once per anchor — its `dup_reported` guard reads `self.warnings`,
        // which this never clears, so it stays idempotent across refreshes.
        let mut discarded = Vec::new();
        let scanned = scan_xref_targets(&self.pages, &self.book, &mut discarded);
        let prev_targets = std::mem::replace(&mut self.xref_targets, scanned);
        let harvested = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.harvest_xref_numbers();
        }));
        if harvested.is_err() {
            self.xref_targets = prev_targets;
        }
    }

    /// Render-harvest: render each page once (scoped to its chapter) and fill in the
    /// CROSS-PAGE facts the lightweight source-scan can't know — a figure / equation /
    /// table / listing / theorem number is assigned only during render, so
    /// `scan_xref_targets` left it empty. This enriches `xref_targets[anchor].number`
    /// (for those non-heading anchors), so a `@fig-x` / `@thm-x` to another page renders
    /// "Figure&nbsp;2.3" / "Theorem&nbsp;2.1" instead of a bare "Figure" / "Theorem".
    ///
    /// It also *inserts* an anchor the scan cannot see at all: a float labelled by a
    /// cell directive (`#| label: fig-x`, `%%| label:`) is inside a fence the scan skips
    /// and is not a brace id, so the render is the only thing that knows it exists.
    /// Reusing the render's own registry — rather than teaching the scan to parse cell
    /// options — keeps one source of truth, so the two cannot drift on which fences
    /// count as cells (the same reason `xref::brace_id` reuses `parse_attrs`).
    /// Called once by `discover`, so build AND the live preview resolve the same numbers.
    /// A pure render pass (no kernel execution), amortised across the discover it rides on.
    pub fn harvest_xref_numbers(&mut self) {
        // Collect during the `&self.pages` pass, then apply — keeps the borrows disjoint.
        // (anchor, number, defining page url) — the url is needed because an anchor the
        // source-scan cannot see is *inserted* here, not just enriched.
        let mut updates: Vec<(String, String, String)> = Vec::new();
        let defaults = self.render_defaults();
        for page in &self.pages {
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let base = page.input.parent().unwrap_or(&self.root);
            let doc = render::render_document_scoped_with_site(
                &src,
                base,
                self.chapter_for(page),
                Some(&defaults),
            );
            for (anchor, number) in doc.xref_numbers {
                // These three conditions gate an INSERT, not just an enrich, so each has
                // to hold on its own rather than lean on the entry already existing:
                //
                // `sec-` numbers are the source-scan's job (chapter-hierarchical, and
                // correctly ABSENT on a non-book website). Harvesting the render's flat
                // per-page section counter here would fill an empty website target with
                // a bare "1", which `rewrite_one_xref` then mislabels "Chapter 1". Only
                // fig/eq/tbl/lst/thm need this render-time enrichment.
                //
                // `is_ref_anchor` keeps parity with the scan (`xref.rs`), because the
                // render registry is LOOSER: the table-caption path registers any id, so
                // `: cap {#my-table}` arrives here. `@my-table` can never resolve (cite
                // rejects an unknown prefix), so admitting it would advertise a phantom
                // target in `taliesin map --format json` and build it a hover card.
                //
                // An empty number means the render assigned none (an unnumbered theorem),
                // and its `::: {.theorem #thm-x}` opener is scan-visible anyway.
                if !number.is_empty() && !anchor.starts_with("sec-") && xref::is_ref_anchor(&anchor)
                {
                    updates.push((anchor, number, page.url.clone()));
                }
            }
        }
        // Whether a label defined on two pages is already reported. The source-scan warns
        // for the anchors IT can see, so the check below covers only the ones it can't (a
        // cell label), and re-checking the list keeps a scan-warned duplicate from being
        // announced twice — and a third definition from announcing a fourth time.
        // Matching the curly-quoted anchor makes it exact, so `fig-a` never matches a
        // warning about `fig-abc`.
        let dup_reported = |warnings: &[String], anchor: &str| {
            let quoted = format!("\u{201c}{anchor}\u{201d}");
            warnings
                .iter()
                .any(|w| w.contains("duplicate cross-reference label") && w.contains(&quoted))
        };
        for (anchor, number, url) in updates {
            match self.xref_targets.entry(anchor) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    // A definition on a page other than the one the target points at is a
                    // duplicate. Report it (unless it already is), and take nothing from
                    // it: its number belongs to a page this link does not go to, and
                    // harvesting it would render "Figure 2" on a link to a page where the
                    // figure reads "Figure 1" — contradicting the warning's own "using …".
                    if e.get().url != url {
                        if !dup_reported(&self.warnings, e.key()) {
                            // A cell-labelled anchor (`#| label:`) has no source line to point
                            // at (it's harvested from the rendered block, not the source scan),
                            // so name BOTH colliding pages instead — the first (winning) page
                            // and the second that redefines it.
                            self.warnings.push(format!(
                                "duplicate cross-reference label \u{201c}{}\u{201d} defined on both {} and {}; using {}",
                                e.key(),
                                e.get().url,
                                url,
                                e.get().url
                            ));
                        }
                        continue;
                    }
                    // Only fill a gap the source-scan left (fig/eq/tbl/lst/thm); a book
                    // heading's section number is already authoritative from the scan.
                    if e.get().number.is_empty() {
                        e.get_mut().number = number;
                    }
                }
                // An anchor the source-scan structurally cannot see: a float labelled by
                // a CELL directive (`#| label: fig-x`) lives inside a fence, which the
                // scan skips, and is not a `{#fig-x}` brace id either. Only the render
                // knows it, so this is its one chance to become a cross-page target —
                // enriching alone silently dropped it and `@fig-x` from another page
                // stayed a bare "Figure" pointing at a dead same-page anchor.
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(XrefTarget {
                        url,
                        number,
                        title: String::new(),
                    });
                }
            }
        }
    }

    /// This page's book chapter number, if it is a numbered chapter (None for a
    /// website page or an unnumbered preface). Drives heading section numbering, float
    /// numbering, and theorem numbering alike, so all three stay in lockstep.
    pub fn chapter_for(&self, page: &Page) -> Option<u32> {
        book::chapter_of(&self.book, page)
    }

    /// Number a book chapter's headings in place (chapter N, then N.1, N.1.1 …).
    /// There is no key to turn this on: a chapter is numbered iff `chapter_for` gives
    /// it a number, i.e. it is a `chapters:` entry that is not the `index` preface and
    /// whose H1 carries no `.unnumbered`/`{-}` (see `book.rs`). A no-op for a website
    /// or an unnumbered preface. Called by both the static build and the live preview.
    pub fn number_chapter(&self, page: &Page, blocks: &mut [Block]) {
        if let Some(number) = self.chapter_for(page) {
            number_chapter_headings(blocks, number);
        }
    }

    // --- listings ---------------------------------------------------------

    /// Apply this page's site-level front-matter blocks to its rendered `blocks`,
    /// mutating in place: a `hero:` block replaces the title block, and each
    /// `listing:` expands into post cards. Both the static build and the live
    /// preview call this, so the results stay in the block model (mounted + diffed
    /// like any other block).
    pub fn expand_page(&self, page: &Page, blocks: &mut Vec<Block>, warnings: &mut Vec<Warning>) {
        // A `hero:` block replaces the title block (a landing-page header treatment).
        if let Some(hero) = &page.hero {
            set_title_block(blocks, self.hero_html(page, hero));
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
                warnings.push(
                    Warning::new(format!(
                        "`{}` has no `title:` and is omitted from the listing on `{}`",
                        p.rel, host.rel
                    ))
                    .severity(Severity::Error),
                );
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
        // A real `<ul>`, so assistive tech announces "list, N items" and offers list
        // navigation (PA-M3). The cards stay `<a>`s inside `<li>`s: putting
        // `role="listitem"` on the anchor would replace its link role, which is worse
        // than the defect being fixed.
        // The explicit `role="list"` is not redundant: WebKit strips list semantics from a
        // `<ul>` whose `list-style` is `none`, which is exactly what the card layout sets,
        // so without it VoiceOver announces nothing even though Chrome's tree is correct.
        // (AP6 compared Firefox and Chromium only, so this browser was never measured.)
        format!("<ul role=\"list\" class=\"tali-listing tali-listing-{layout}\">{cards}</ul>")
    }

    fn card_html(&self, p: &Page, up: &str, with_image: bool) -> String {
        let href = format!("{up}{}", p.url);
        let img = match (with_image, &p.card_image) {
            (true, Some(src)) => format!(
                "<img class=\"tali-card-img\" src=\"{up}{}\" alt=\"{}\" loading=\"lazy\">",
                esc(src),
                esc(p.card_image_alt.as_deref().unwrap_or(""))
            ),
            // This listing shows thumbnails, but this post has no `image:`. Reserve the
            // same slot with a monogram placeholder so a mixed listing keeps its rhythm:
            // in the row layout the body still starts at the thumbnail column, and in the
            // grid the card is a proper card, not a stretched void beside its neighbours.
            (true, None) => {
                let title = p.title.as_deref().unwrap_or(&p.rel);
                let initial = title
                    .chars()
                    .find(|c| c.is_alphanumeric())
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_default();
                format!(
                    "<div class=\"tali-card-noimg\" aria-hidden=\"true\">{}</div>",
                    esc(&initial)
                )
            }
            _ => String::new(),
        };
        // A `<time datetime>` (PA-M1) so the card date stays machine-readable; the class keeps
        // its styling (the CSS targets `.tali-card-date`, not a `<div>`).
        let date = p
            .date
            .as_deref()
            .map(|d| crate::render::time_html(d, "tali-card-date"))
            .unwrap_or_default();
        let title = esc(p.title.as_deref().unwrap_or(&p.rel));
        let desc = p
            .description
            .as_deref()
            .map(|d| format!("<p class=\"tali-card-desc\">{}</p>", esc(d)))
            .unwrap_or_default();
        // Each badge still carries `data-cat` (the exact category name), a leftover of the
        // listing category filter deleted 2026-08-04 (visual minimalism pass) — the badge
        // is inert display now; nothing reads `data-cat` at runtime.
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
        // A draft card is badged so it reads as unpublished in a listing (preview only —
        // a built listing never contains a draft, so this is inert in `build`).
        let draft_badge = if p.draft {
            "<span class=\"tali-draft-badge\">Draft</span>"
        } else {
            ""
        };
        // `data-tali-src` lets the click-to-source locator jump to the post's source
        // (it's site-root-relative; resolved client-side, inert in the static build).
        format!(
            "<li class=\"tali-listing-item\"><a class=\"tali-card\" href=\"{href}\" data-tali-src=\"{src}\">{img}\
             <div class=\"tali-card-body\">{draft_badge}{date}<h3 class=\"tali-card-title\">{title}</h3>{desc}{cats}</div></a></li>",
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
        // The hero banner is type, not a figure. `hero.image:`/`image-alt:` were retired on
        // 2026-08-02 and the two-column portrait layout they drove was deleted on
        // 2026-08-08; this is byte-identical to the emission that predated the slot.
        format!(
            "<header class=\"hero\" data-block-id=\"tali-title-block\" data-tali-src=\"{src}\">{inner}</header>"
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
        nested: Vec::new(),
    }
}

/// Set the page's title-block content to `html` (a `hero:` header): reuse
/// the existing `tali-title-block` so source-mapping + diffing are preserved, or
/// insert it at the top if the page has no title block.
fn set_title_block(blocks: &mut Vec<Block>, html: String) {
    match blocks.iter_mut().find(|b| b.id == "tali-title-block") {
        Some(tb) => tb.html = html,
        None => blocks.insert(
            0,
            Block {
                id: "tali-title-block".to_string(),
                sourcepos: String::new(),
                source_file: None,
                html,
                cell: None,
                nested: Vec::new(),
            },
        ),
    }
}

/// Walk a raw `.tmd` source's *content* lines: those outside the leading front-matter
/// block and outside fenced code (` ``` `/`~~~`). Each yielded line is already
/// `trim_start`ed. This is the skeleton both raw-source scanners share —
/// [`xref::scan_page_anchors`] (heading `{#id}` anchors + section numbers) and
/// [`book::chapter_heading`] (a chapter's leading `# H1`) — so a `#` inside front matter
/// or a `# comment` inside a code fence is never mistaken for a heading in either. It does
/// NOT resolve `{{< include >}}`: that stays a deliberate caller choice (the xref scan
/// resolves includes first so section numbers advance over included headings; chapter-title
/// detection reads the file raw). A refactor of two ~identical pre-scans into one, not a
/// behavior change.
pub(super) fn content_lines(src: &str) -> impl Iterator<Item = &str> {
    content_lines_numbered(src).map(|(_, t)| t)
}

/// [`content_lines`] paired with each line's 1-based source line number, so a scan can point
/// a diagnostic at exactly where an anchor lives.
pub(super) fn content_lines_numbered(src: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut in_front_matter = false;
    let mut in_code = false;
    src.lines().enumerate().filter_map(move |(i, line)| {
        let t = line.trim_start();
        if i == 0 && t == "---" {
            in_front_matter = true;
            return None;
        }
        if in_front_matter {
            in_front_matter = t != "---";
            return None;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            return None;
        }
        if in_code {
            return None;
        }
        Some((i + 1, t))
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn content_lines_skips_front_matter_and_fenced_code() {
        // The skeleton both raw-source scanners (xref anchors, chapter titles) now share:
        // front matter (even a `#`-looking line in it) and fenced code (```/~~~, even a
        // `# comment` inside) are dropped; the real headings + prose survive, trim_start'ed.
        // A `#` in either region must never read as a heading in either scanner.
        let src = concat!(
            "---\n",
            "title: X\n",
            "# not a heading (front matter)\n",
            "---\n",
            "\n",
            "# Real H1\n",
            "```yaml\n",
            "# fake heading in a fence\n",
            "```\n",
            "text\n",
            "~~~\n",
            "## also fake in a tilde fence {#sec-fake}\n",
            "~~~\n",
            "## Real H2 {#sec-x}\n",
        );
        let lines: Vec<&str> = content_lines(src).collect();
        assert!(lines.contains(&"# Real H1"), "real H1 survives: {lines:?}");
        assert!(
            lines.contains(&"## Real H2 {#sec-x}"),
            "real H2 survives: {lines:?}"
        );
        assert!(lines.contains(&"text"), "prose survives: {lines:?}");
        assert!(
            !lines
                .iter()
                .any(|l| l.contains("fake") || l.contains("front matter")),
            "no front-matter or in-fence line may leak: {lines:?}"
        );
    }

    #[test]
    fn a_titleless_website_page_falls_back_to_its_leading_h1() {
        // A website page with no front-matter `title:` but a leading `# H1` takes the H1
        // as its title, so <title>, og:title, listing cards, nav, and search — all of which
        // read `Page.title` — agree. (A website page resolves title-first; a BOOK chapter
        // resolves `text:` -> `# H1` -> `title:`, because a chapter has a nav label distinct
        // from its page title; a chapter that sets both deliberately shows two names.)
        let root = write_site(
            "h1title",
            &[
                ("_site.yml", "title: My Site\nurl: https://ex.com\n"),
                (
                    "about.tmd",
                    "---\ndescription: About me.\n---\n\n# About the author\n\nHi.\n",
                ),
                // Front matter still wins when a `title:` is present (H1 differs).
                (
                    "explicit.tmd",
                    "---\ntitle: Explicit\n---\n\n# A different heading\n\nx\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let title_of = |rel: &str| {
            site.pages
                .iter()
                .find(|p| p.rel == rel)
                .and_then(|p| p.title.clone())
        };
        assert_eq!(title_of("about.tmd").as_deref(), Some("About the author"));
        assert_eq!(title_of("explicit.tmd").as_deref(), Some("Explicit"));
        // og:title now uses the H1 (not the site name), and the <title> agrees with it.
        let html = site.render_page("about.tmd").unwrap();
        assert!(
            html.contains(r#"property="og:title" content="About the author""#),
            "og:title should be the H1, not the site name"
        );
        assert!(
            html.contains("<title>About the author · My Site</title>"),
            "the <title> and og:title agree"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

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

        // Exclude (the published view): the draft is dropped AND recorded.
        let mut excluded = Vec::new();
        let rels: Vec<String> =
            website_pages(&root, DraftMode::Exclude, &mut Vec::new(), &mut excluded)
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
        assert_eq!(excluded, vec!["wip.tmd".to_string()], "draft recorded");

        // Include (the preview view): the draft is kept, tagged, and nothing is recorded.
        let mut excluded2 = Vec::new();
        let pages = website_pages(&root, DraftMode::Include, &mut Vec::new(), &mut excluded2);
        let wip = pages
            .iter()
            .find(|p| p.rel == "wip.tmd")
            .expect("draft kept in Include");
        assert!(wip.draft, "the draft page is tagged in Include");
        assert!(excluded2.is_empty(), "Include records no exclusions");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_is_published_view_discover_with_include_is_preview_view() {
        let root = write_site(
            "draftmode",
            &[
                ("_site.yml", "title: T\n"),
                ("live.tmd", "---\ntitle: Live\n---\nbody\n"),
                ("wip.tmd", "---\ntitle: WIP\ndraft: true\n---\nbody\n"),
            ],
        );

        let published = Site::discover(&root); // == discover_with(Exclude)
        assert!(published.pages.iter().any(|p| p.rel == "live.tmd"));
        assert!(
            !published.pages.iter().any(|p| p.rel == "wip.tmd"),
            "draft absent from the published set"
        );
        assert_eq!(published.excluded_drafts, vec!["wip.tmd".to_string()]);

        let preview = Site::discover_with(&root, DraftMode::Include);
        let wip = preview
            .pages
            .iter()
            .find(|p| p.rel == "wip.tmd")
            .expect("draft present in preview");
        assert!(wip.draft, "the draft page is tagged");
        assert!(
            preview.excluded_drafts.is_empty(),
            "Include excludes nothing"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A link to a page discovery held back is a 404 in the deploy, and it was the one
    /// broken link the gate excused: `resolve_link_warnings` accepted any target whose
    /// `.tmd` sat on disk, and a draft's `.tmd` sits on disk by definition.
    #[test]
    fn a_link_to_an_unpublished_page_is_broken() {
        let root = write_site(
            "unpublished-link",
            &[
                ("_site.yml", "title: T\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\n[the post](posts/a.tmd) and [a note](notes.md).\n",
                ),
                (
                    "posts/a.tmd",
                    "---\ntitle: A\ndraft: true\n---\n\nSecret.\n",
                ),
                ("notes.md", "raw source, deliberately shipped\n"),
            ],
        );
        let site = Site::discover(&root);
        let msgs: Vec<String> = site
            .validate_cross_page_links()
            .into_iter()
            .map(|(_rel, w)| w.message)
            .collect();
        let joined = msgs.join("\n");

        let hit = msgs
            .iter()
            .find(|m| m.contains("posts/a.tmd"))
            .unwrap_or_else(|| panic!("a link to a draft must be reported:\n{joined}"));
        assert!(
            hit.contains("draft"),
            "and must say WHY, since the file the author linked is right there on disk: {hit}"
        );
        assert!(
            !joined.contains("notes.md"),
            "a raw source file on disk is still a legitimate target:\n{joined}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The same hatch, strictly worse: a book publishes only what `chapters:` lists, and an
    /// unlisted `.tmd` beside it produces no "N drafts not published" line either, so
    /// nothing anywhere told the author the link was dead.
    #[test]
    fn a_link_to_a_page_no_chapter_list_names_is_broken() {
        let root = write_site(
            "unlisted-link",
            &[
                ("_site.yml", "title: B\nchapters:\n  - index.tmd\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\n[stray](stray.tmd)\n"),
                ("stray.tmd", "---\ntitle: Stray\n---\n\nNot listed.\n"),
            ],
        );
        let site = Site::discover(&root);
        let msgs: Vec<String> = site
            .validate_cross_page_links()
            .into_iter()
            .map(|(_rel, w)| w.message)
            .collect();
        let joined = msgs.join("\n");

        let hit = msgs
            .iter()
            .find(|m| m.contains("stray.tmd"))
            .unwrap_or_else(|| panic!("a link to an unlisted page must be reported:\n{joined}"));
        assert!(
            hit.contains("does not publish"),
            "the reason is the chapter list, not a missing file: {hit}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn book_drafts_excluded_renumber_contiguously_include_numbers_in_context() {
        let root = write_site(
            "bookdraft",
            &[
                (
                    "_site.yml",
                    "title: B\nchapters:\n  - one.tmd\n  - wip.tmd\n  - two.tmd\n",
                ),
                ("one.tmd", "# One\n"),
                ("wip.tmd", "---\ndraft: true\n---\n# WIP\n"),
                ("two.tmd", "# Two\n"),
            ],
        );

        let published = Site::discover(&root);
        assert!(!published.pages.iter().any(|p| p.rel == "wip.tmd"));
        assert_eq!(published.excluded_drafts, vec!["wip.tmd".to_string()]);
        let book = published.book.as_ref().unwrap();
        // Chapters renumber contiguously: One=1, Two=2 (no gap where WIP was).
        let nums: Vec<u32> = book.chapters().iter().filter_map(|c| c.number).collect();
        assert_eq!(nums, vec![1, 2]);
        assert!(!book.chapters().iter().any(|c| c.rel == "wip.tmd"));

        let preview = Site::discover_with(&root, DraftMode::Include);
        let pbook = preview.book.as_ref().unwrap();
        let pchapters = pbook.chapters();
        let wip = pchapters
            .iter()
            .find(|c| c.rel == "wip.tmd")
            .expect("draft chapter present in preview");
        assert!(wip.draft);
        assert_eq!(
            wip.number,
            Some(2),
            "numbered in context (One=1, WIP=2, Two=3)"
        );
        assert!(preview.pages.iter().any(|p| p.rel == "wip.tmd" && p.draft));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_part_whose_chapters_are_all_drafts_drops_its_header() {
        // Drafting a whole part is a natural authoring state ("Part III is still WIP").
        // The published drawer must not keep an orphan heading over nothing.
        let root = write_site(
            "ghostpart",
            &[
                (
                    "_site.yml",
                    "title: B\nchapters:\n  - index.tmd\n  - part: Ghost\n    chapters:\n      - wipa.tmd\n      - wipb.tmd\n  - part: Real\n    chapters:\n      - live.tmd\n",
                ),
                ("index.tmd", "# Preface\n"),
                ("wipa.tmd", "---\ndraft: true\n---\n# WIP A\n"),
                ("wipb.tmd", "---\ndraft: true\n---\n# WIP B\n"),
                ("live.tmd", "# Live\n"),
            ],
        );

        let published = Site::discover(&root);
        let parts: Vec<String> = published
            .book
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .filter_map(|e| e.part.clone())
            .collect();
        assert_eq!(
            parts,
            vec!["Real".to_string()],
            "the all-draft part header is dropped; a part with a live chapter stays"
        );

        // In preview both parts stand (neither is empty there).
        let preview = Site::discover_with(&root, DraftMode::Include);
        let pparts: Vec<String> = preview
            .book
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .filter_map(|e| e.part.clone())
            .collect();
        assert_eq!(pparts, vec!["Ghost".to_string(), "Real".to_string()]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn listing_card_shows_draft_badge_only_for_drafts() {
        let root = write_site(
            "cardbadge",
            &[
                ("_site.yml", "title: T\n"),
                ("live.tmd", "---\ntitle: Live\n---\nx\n"),
                ("wip.tmd", "---\ntitle: WIP\ndraft: true\n---\nx\n"),
            ],
        );
        let site = Site::discover_with(&root, DraftMode::Include);
        let live = site.pages.iter().find(|p| p.rel == "live.tmd").unwrap();
        let wip = site.pages.iter().find(|p| p.rel == "wip.tmd").unwrap();
        assert!(
            site.card_html(wip, "", false).contains("tali-draft-badge"),
            "a draft card carries the badge"
        );
        assert!(
            !site.card_html(live, "", false).contains("tali-draft-badge"),
            "a published card has no badge"
        );
        let _ = std::fs::remove_dir_all(&root);
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

        let pages = website_pages(&root, DraftMode::Exclude, &mut Vec::new(), &mut Vec::new());
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
        let rels: Vec<String> =
            website_pages(&root, DraftMode::Exclude, &mut warnings, &mut Vec::new())
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

    /// A two-chapter book whose `methods` chapter cross-references a figure defined in
    /// `intro` — the smallest shape that exercises the xref registry's whole seam.
    fn xref_book(tag: &str) -> std::path::PathBuf {
        write_site(
            tag,
            &[
                (
                    "_site.yml",
                    "title: B\nchapters:\n  - index.tmd\n  - intro.tmd\n  - methods.tmd\n",
                ),
                ("index.tmd", "---\ntitle: Home\n---\n\nWelcome.\n"),
                (
                    "intro.tmd",
                    "---\ntitle: Intro\n---\n\n![The structure.](a.svg){#fig-structure}\n",
                ),
                (
                    "methods.tmd",
                    "---\ntitle: Methods\n---\n\n## Setup {#sec-setup}\n\n\
                     Refines the overview from @fig-structure into steps.\n",
                ),
            ],
        )
    }

    /// A cross-page `@fig-` was indexed WITHOUT its number, so the Cmd-K snippet
    /// contradicted the page it points at ("…from Figure into…" vs the page's "…from
    /// Figure 1.1 into…") and the number was unsearchable. An ORDERING fact, not a text
    /// bug: `build_sections` ran at discovery *before* `harvest_xref_numbers` filled the
    /// numbers, and the per-page render it uses leaves a cross-page ref as an unresolved
    /// marker link — only the site-level pass rewrites it.
    #[test]
    fn search_index_carries_a_cross_page_xref_number() {
        let root = xref_book("xref-search");
        let site = Site::discover(&root);
        assert!(
            site.search_index_json.contains("Figure 1.1"),
            "the index must carry the number the page shows: {}",
            site.search_index_json
        );
        assert!(
            !site.search_index_json.contains("from Figure into"),
            "a bare label means the marker was never resolved: {}",
            site.search_index_json
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The other end of the same seam: the registry is filled ONLY in `discover`, so a warm
    /// preview's cross-page numbers freeze at startup. Measured on a live server before this
    /// existed: after inserting a figure above `fig-structure`, `intro.html` served
    /// "Figure 1.2" while `methods.html` served "Figure 1.1" for the same figure — one
    /// preview contradicting itself — and an anchor created after startup stayed unknown
    /// forever, rendering as a dead same-page link.
    #[test]
    fn refresh_xrefs_reflects_a_renumber_and_a_new_anchor() {
        use std::fs;
        let root = xref_book("xref-refresh");
        let mut site = Site::discover(&root);
        assert_eq!(site.xref_targets["fig-structure"].number, "1.1");
        assert!(!site.xref_targets.contains_key("fig-new"));

        // Insert a figure ABOVE the referenced one: `fig-structure` becomes 1.2, and
        // `fig-new` is an anchor the registry has never seen.
        fs::write(
            root.join("intro.tmd"),
            "---\ntitle: Intro\n---\n\n![A new first.](a.svg){#fig-new}\n\n\
             ![The structure.](a.svg){#fig-structure}\n",
        )
        .unwrap();
        site.refresh_xrefs();
        assert_eq!(
            site.xref_targets["fig-structure"].number, "1.2",
            "the renumber must reach the registry"
        );
        assert_eq!(
            site.xref_targets.get("fig-new").map(|t| t.url.as_str()),
            Some("intro.html"),
            "an anchor born after startup must become resolvable"
        );
        // A dropped anchor must LEAVE the registry, or a stale target outlives its source
        // and `@fig-structure` keeps resolving to a figure that no longer exists.
        fs::write(
            root.join("intro.tmd"),
            "---\ntitle: Intro\n---\n\n![A new first.](a.svg){#fig-new}\n",
        )
        .unwrap();
        site.refresh_xrefs();
        assert!(
            !site.xref_targets.contains_key("fig-structure"),
            "a deleted anchor must not linger: {:?}",
            site.xref_targets
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// A refresh must be all-or-nothing about the numbers. The harvest renders EVERY page, so
    /// a panic partway would otherwise leave the raw scan map behind — floats un-numbered
    /// site-wide — and one bad page would silently strip the numbers off every good page's
    /// cross-page refs. Stale-but-numbered beats un-numbered.
    #[test]
    fn a_refresh_that_cannot_complete_keeps_the_previous_registry() {
        use std::fs;
        let root = xref_book("xref-panic");
        let mut site = Site::discover(&root);
        assert_eq!(site.xref_targets["fig-structure"].number, "1.1");

        // Make the harvest's render unreachable for every page (the sources are gone), which
        // is the closest a test can get to "the pass did not complete" without a panic: the
        // numbers it would re-derive are simply not there to find.
        for p in ["index.tmd", "intro.tmd", "methods.tmd"] {
            fs::remove_file(root.join(p)).unwrap();
        }
        site.refresh_xrefs();
        // Unreadable sources mean the scan sees no anchors at all — the registry empties
        // rather than keeping numbers it can no longer justify. What must NOT happen is a
        // registry that still lists `fig-structure` with its number stripped to "".
        if let Some(t) = site.xref_targets.get("fig-structure") {
            assert!(
                !t.number.is_empty(),
                "a listed target must never lose its number: {t:?}"
            );
        }
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

    /// `preview <file.tmd>` on a document with no ancestor `_site.yml` gets a project of
    /// exactly that document — not its whole parent directory. Thirty unrelated notes next
    /// to it must not become nav entries (nor be parsed to discover that they are not).
    #[test]
    fn discover_single_scopes_the_project_to_one_document() {
        let root = write_site(
            "single",
            &[
                ("note.tmd", "---\ntitle: Note\n---\n\nThe note.\n"),
                ("other.tmd", "---\ntitle: Other\n---\n\nUnrelated.\n"),
                (
                    "deep/third.tmd",
                    "---\ntitle: Third\n---\n\nAlso unrelated.\n",
                ),
            ],
        );
        let site = Site::discover_single(&root.join("note.tmd"));
        assert_eq!(
            site.pages.iter().map(|p| &p.rel).collect::<Vec<_>>(),
            vec!["note.tmd"],
            "only the previewed document is a page"
        );
        // Discovering the directory instead is what this must NOT do.
        assert_eq!(
            Site::discover(&root).pages.len(),
            3,
            "the fixture has three"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A single-document project has exactly one page and no `index.tmd`, so the server
    /// must answer the bare preview URL with that document. This pins the fact the routing
    /// depends on: the one page's URL is NOT `index.html`, so a root request that falls
    /// through to the usual lookup finds nothing and serves the 404 page — for the one
    /// document the author asked to see. (Caught by a browser test that previews a
    /// `.tmd` and fetches `/`; a gate that only `tools/gates.sh` runs.)
    #[test]
    fn a_single_document_project_has_no_index_page_to_answer_the_root_with() {
        let root = write_site(
            "singleroot",
            &[("note.tmd", "---\ntitle: Note\n---\n\nBody.\n")],
        );
        let site = Site::discover_single(&root.join("note.tmd"));
        assert_eq!(site.pages.len(), 1);
        assert_eq!(
            site.pages[0].url, "note.html",
            "the document keeps its own URL; the server maps the root onto it"
        );
        assert!(
            site.page("index.html").is_none(),
            "nothing answers `index.html`, which is why the root needs the mapping"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Render `rel` in `site` and return (html, render-warnings).
    fn render_page(site: &Site, rel: &str) -> (String, Vec<Warning>) {
        let page = site.pages.iter().find(|p| p.rel == rel).unwrap();
        let src = std::fs::read_to_string(&page.input).unwrap();
        let doc = crate::render::render_document_with_includes(&src, &site.root);
        site.render_page_doc_warned(page, doc)
    }

    #[test]
    fn external_site_render_keeps_search_index_inline_drops_shared_toc_js() {
        // A literal from web-client/toc-spy.js: stable + unique enough that its
        // presence proves the shared scrollspy code got re-inlined.
        const MARKER_TOC_SPY: &str = "taliInitTocSpy";
        let root = write_site(
            "ext-toc",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\ntoc: true\n---\n\n# Home\n\n## Alpha\n\nHi.\n\n\
                     ## Beta\n\nBye.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let page = site.pages.iter().find(|p| p.rel == "index.tmd").unwrap();
        let src = std::fs::read_to_string(&page.input).unwrap();
        let doc = crate::render::render_document_with_includes(&src, &site.root);
        let ext = render::ExternalAssets {
            app_css: "_assets/app.a.css",
            katex_css: "_assets/katex.b.css",
            app_js: "_assets/app.c.js",
            mermaid_js: "_assets/mermaid.d.js",
            jslibs_js: "_assets/jslibs.e.js",
            font_preload: "",
        };
        let (html, _w) = site.render_page_doc_external(page, doc, ext);
        // app.js is linked (carries the toc/search code now).
        assert!(
            html.contains("src=\"_assets/app.c.js\" defer"),
            "app.js should be linked: {html}"
        );
        // The shared toc-spy code is NOT inlined again (it now lives in app.js).
        assert!(
            !html.contains(MARKER_TOC_SPY),
            "toc-spy code must not be re-inlined: {html}"
        );
        // The per-page search index (inline bootstrap) is still present.
        assert!(
            html.contains("TALIESIN_SEARCH_URL"),
            "the per-page search index bootstrap should stay inline: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
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
        // AN-5: "a bare Section" was the *other* half of the same defect. With no number
        // to carry, the link named nothing at all and the sentence read "See Section
        // elsewhere." It names its target instead — the information the number would
        // have carried, in the form a website can supply it.
        assert!(
            html.contains(
                "<a href=\"other.html#sec-topic\" class=\"tali-xref\">Section&nbsp;\u{201c}A \
                 topic\u{201d}</a>"
            ),
            "an unnumbered cross-page @sec- must name its heading: {html}"
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
        // it must not own posts — symmetry with the titleless-covered-page guard. The host
        // has neither a `title:` nor a leading `# H1` (which would now supply the title).
        let root = write_site(
            "backlink-titlelesshost",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "feed.tmd",
                    "---\nlisting:\n  contents: posts\n---\n\nA feed with no heading.\n",
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

    /// PA-M3: a listing is a real list, so assistive tech announces "list, N items" and
    /// offers list navigation. It used to be a `<div>` of `<a>` cards — visually a grid,
    /// semantically a pile — while the book chapter list and the TOC next to it were
    /// correct `<ul>`s.
    ///
    /// The cards stay `<a>`s inside `<li>`s rather than becoming `role="listitem"`
    /// themselves: a role on the anchor would REPLACE its link semantics, trading one
    /// a11y defect for a worse one.
    #[test]
    fn a_listing_is_a_list_so_at_can_announce_and_navigate_it() {
        let root = write_site(
            "listingsemantics",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "blog.tmd",
                    "---\ntitle: Blog\nlisting:\n  contents: posts\n  type: grid\n---\n\n# Blog\n",
                ),
                (
                    "posts/a.tmd",
                    "---\ntitle: A\ndate: 2026-01-01\n---\n\nBody.\n",
                ),
                (
                    "posts/b.tmd",
                    "---\ntitle: B\ndate: 2026-01-02\n---\n\nBody.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let (blog, _) = render_page(&site, "blog.tmd");

        // Needle the full opening tag: every page inlines the whole stylesheet, which
        // names `.tali-listing`, so a bare class-name `contains` passes on any page.
        assert!(
            blog.contains("<ul role=\"list\" class=\"tali-listing tali-listing-grid\">"),
            "the listing container must be a <ul>: {blog}"
        );
        assert!(
            !blog.contains("<div class=\"tali-listing tali-listing-grid\">"),
            "the old <div> container must be gone: {blog}"
        );
        // The explicit role is load-bearing, not belt-and-braces: `list-style: none` (which
        // the card layout sets) makes WebKit drop list semantics entirely.
        assert!(
            blog.contains("role=\"list\""),
            "the <ul> must keep an explicit role=list for WebKit: {blog}"
        );
        // Each card is wrapped, and the anchor keeps its own semantics.
        let items = blog.matches("<li class=\"tali-listing-item\">").count();
        assert_eq!(items, 2, "each of the 2 posts must be one <li>: {blog}");
        assert!(
            blog.contains("<li class=\"tali-listing-item\"><a class=\"tali-card\""),
            "the card anchor must sit INSIDE its <li>: {blog}"
        );
        assert!(
            !blog.contains("role=\"listitem\""),
            "cards must not take a listitem role, which would replace their link role: {blog}"
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
        // b.tmd is chapter 2, so its first figure is "2.1": the harvest must carry the
        // chapter-scoped number across pages, which is the whole point of scoping (a flat
        // "Figure 1" here would collide with chapter 1's own first figure).
        let site = Site::discover(&root);
        let html = site.render_page("a.tmd").unwrap();
        assert!(
            html.contains("<a href=\"b.html#fig-plot\" class=\"tali-xref\">Figure&nbsp;2.1</a>"),
            "cross-page figure ref numbered after discover: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
