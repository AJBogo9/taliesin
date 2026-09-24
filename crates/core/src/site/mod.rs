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
//! drive the site through [`Site::discover`] + [`Site::page_chrome`].

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
    /// Front-matter `date` as written, for display. Anything that orders or stamps by it
    /// reads [`Page::day`] instead.
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
    /// `draft: true` in front matter. `false` for every published page; `true` only for a
    /// draft surfaced in `DraftMode::Include` (preview). Drives the DRAFT badge/banner; a
    /// built page is always `false`, so those affordances are inert in a build.
    pub draft: bool,
}

impl Page {
    /// The calendar day `date:` names (`crate::frontmatter::calendar_date`), `None` when it
    /// names none. The one reading of the date that the listing order, the Atom feed and the
    /// sitemap's `<lastmod>` share: the raw string sorted an un-padded `2026-1-5` above
    /// `2026-01-20`, and free text above every real date.
    pub(crate) fn day(&self) -> Option<(u32, u32, u32)> {
        self.date
            .as_deref()
            .and_then(crate::frontmatter::calendar_date)
    }
}

/// A `hero:` front-matter block: the headline + lead + call-to-action band at the
/// top of a landing/home page. Authored entirely in YAML, so a landing page needs
/// no bespoke HTML — it renders into the framework's `.hero` primitive. Reusable
/// for a product page, a researcher's homepage, or a lab/group site.
#[derive(Debug, Clone)]
pub struct HeroSpec {
    /// Short italic line above the headline (`eyebrow:`); optional.
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
    /// `primary: true` renders the filled accent button; otherwise the outline style.
    pub primary: bool,
}

/// A `listing:` front-matter block: a request to render a list of cards for the
/// documents under `contents`.
#[derive(Debug, Clone)]
pub struct ListingSpec {
    /// Optional target id (`listing: { id: x }`) → fills `::: {#x}`; else appended.
    pub id: Option<String>,
    /// The directory whose pages are listed (relative to the hosting page).
    pub contents: String,
    /// Whether cards show their `image:` thumbnail: `type: list`, not the plain default.
    /// Lets a reading-first `list` keep the figure thumbnails while a formal text listing
    /// (e.g. a CV's projects) stays image-free. `type: grid` was cut on 2026-09-24.
    pub with_image: bool,
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
    /// The project-wide `bibliography:` (`_site.yml`), resolved once at discovery against
    /// the site root: absolute `.bib` paths, in declaration order, a file not written yet
    /// included (see `bibliography::resolve_shared`). Empty for a project that declares
    /// none. Laid **under** each page's own `bibliography:`, so a
    /// page can override a shared entry (`site::bibliography`).
    pub bibliography: Vec<PathBuf>,
    /// Diagnostics about the project itself, gathered during discovery: its `_site.yml`,
    /// a page's front matter as discovery reads it, a label two pages define. Each is
    /// located, relative to the site root: `file` is `_site.yml` or a page's `rel` (a
    /// partial is joined onto its page's folder), `line` is that file's own line, and
    /// `severity` is the validator's. Every verb reports them exactly as it reports a
    /// page's render warnings. They were strings with the location baked into the text,
    /// which `--check-only` pinned on `_site.yml` with no line, a writing build logged as
    /// advice, and `--format json` dropped (audit 2026-09-24 NEW-A, NEW-B).
    pub warnings: Vec<Warning>,
    /// How many of the leading entries in `warnings` came from parsing `_site.yml`
    /// itself, rather than from page discovery. See [`config_warnings`](Self::config_warnings).
    config_warning_count: usize,
    /// Inlinable JSON of every page's title + anchored headings, so the Cmd-K
    /// palette searches the whole project (`window.TALIESIN_SEARCH_INDEX`). Assembled
    /// from per-page fragments; the dev server rebuilds it whole whenever a cross-reference
    /// anchor moves, so a snippet never contradicts the page it links to.
    pub search_index_json: String,
    /// Rel paths of `draft: true` pages dropped in `DraftMode::Exclude` (empty in
    /// `Include`). Drives the build's "N drafts not published" report.
    pub excluded_drafts: Vec<String>,
    /// True when this site is discovered for ONE document ([`Site::discover_document`]): a
    /// document with no project, or one page of a project
    /// built on its own (`build <file>`). Either way nothing else of the project is
    /// published beside it.
    ///
    /// Such a document gets no project navigation: for one with no project the navbar
    /// would brand it "Home" and link to the page you are already on, the burger would open
    /// an empty nav, and the footer would credit a site that does not exist; for a page
    /// built alone every link in them names a page that build does not write. And a book
    /// chapter built alone keeps its table of contents ([`Site::page_toc`]): the book's
    /// chapter drawer that stands in for it is not there either.
    pub standalone: bool,
}

mod book;
mod chrome;
pub use book::{Book, BookEntry};
use book::{book_pages, build_book, chapter_heading};
mod bibliography;
pub(crate) use bibliography::{project_bibliography_has_entries, shared_for_single_doc};
mod fanout;
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
pub(crate) use chapter::{number_sections, section_number_span};
mod discovery;
// `collect_pages` is not called here: `xref.rs` reaches it through this binding (a
// private `use` is still visible to a descendant module), so the project-wide anchor
// scan walks exactly the page set discovery does.
pub use discovery::{collect_pages, discovery_digest};
use discovery::{website_page, website_pages};
/// Minimum number of `toc_entry_count` headings for a site-wide `toc: true` to render the
/// sidebar TOC (the auto-gate in [`Site::page_toc`]). Below this a page reads as one column.
const MIN_TOC_HEADINGS: usize = 3;
/// The author's own not-found page: `404.tmd` at the project root, built to `404.html`,
/// which a static host serves for every unknown path.
fn is_not_found_page(page: &Page) -> bool {
    page.url == "404.html"
}

mod links;
#[cfg(test)]
mod poison_tests;
pub use links::rewrite_tmd_links;
use links::{
    block_tag_has_id, collect_html_ids, href_matches_page, html_to_tmd, is_external_or_special,
    join_rel, join_rel_in_root, manual_local_links, resolve_href, root_absolute_urls,
    sourcepos_start_line, tmd_to_html,
};

/// A document's path as every verb names it: its folder canonicalized, its own name kept. A
/// symlinked page keeps the name its link has in the project, which is the page the site
/// walker discovers; canonicalizing the whole path resolved the link and put the page in
/// whatever folder its target happens to sit in (see [`Site::discover_document`]).
pub fn document_path(file: &Path) -> PathBuf {
    let dir = file
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let dir = dir
        .canonicalize()
        .unwrap_or_else(|_| crate::includes::absolutize(dir));
    match file.file_name() {
        Some(name) => dir.join(name),
        None => dir,
    }
}

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
    /// The diagnostics `_site.yml` itself produced, in order — the subset of
    /// [`warnings`](Self::warnings) a caller may report as "the config is wrong".
    ///
    /// It exists because `doctor`'s `config` row asked the question by message text, with
    /// [`is_malformed_config_warning`], and so printed `✓ _site.yml is valid` on a file
    /// `build --check-only` rejected with two errors: an unknown key, a typo'd key and the
    /// scheme-less `url:` warning are all config defects that are not YAML parse failures.
    /// The whole-project answer is still `build <dir> --check-only`; this is only the
    /// narrower question of whether the *config file* is clean.
    pub fn config_warnings(&self) -> &[Warning] {
        &self.warnings[..self.config_warning_count]
    }

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

    /// The project a document named on its own belongs to, scoped to that one document:
    /// THE discovery every verb that is handed a `.tmd` rather than a directory starts from
    /// (`build <file>`, `build <file> --check-only`, and `preview <file>` for a document with
    /// no project). The project is the nearest `_site.yml` above the document's folder; with
    /// none, the folder is a project of just that document. Scoping to the one file (rather
    /// than discovering the whole directory) is the point: a scratch note must not pull
    /// thirty unrelated siblings into its page, and must not parse them to find that out.
    ///
    /// **The folder is canonicalized, not the file**, which is what the site walker keeps:
    /// a symlinked page belongs to the project its link sits in, as the site build that
    /// publishes it there says. Resolving the link first made the preview serve the TARGET's
    /// folder as a project of one document, with no nav, while the page's own URL answered
    /// 404 (audit 2026-09-24, config-seam #14).
    ///
    /// The published view of the project around it ([`DraftMode::Exclude`]): a book chapter
    /// built alone carries the number the published book gives it, and the document itself
    /// is its one page whatever its own `draft:` says, since it was named.
    pub fn discover_document(file: &Path) -> Site {
        let file = document_path(file);
        let dir = file.parent().unwrap_or_else(|| Path::new("."));
        let root = enclosing_site_root(dir).unwrap_or_else(|| dir.to_path_buf());
        Self::discover_scoped(&root, DraftMode::Exclude, Some(&file))
    }

    /// [`discover_with`](Self::discover_with), optionally narrowed to one document
    /// (see [`discover_document`](Self::discover_document)). The narrowing happens before
    /// cross-references and the search index are computed, so every downstream artifact is
    /// built from the scoped page set rather than filtered afterwards; a single-page `build`
    /// inside a project gets that project's config (its `python:` pin) without paying the
    /// whole project's two render passes (+80 ms on `docs/guide`, 16 pages, release,
    /// measured 2026-09-02) for a page set it would throw away.
    pub fn discover_scoped(root: &Path, drafts: DraftMode, only: Option<&Path>) -> Site {
        let mut site = Self::registry(root, drafts, only);
        site.xref_targets = scan_xref_targets(&site.pages, &mut site.warnings);
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

    /// The project at `root` as its page registry (published view, drafts excluded):
    /// config, pages, book, shared bibliography, discovery warnings and the cross-page
    /// targets the source scan finds, with no render pass and no search index. For the
    /// language server, whose buffer lint and `siteMap` read nothing else, and which
    /// rediscovers on every save of any page: the full [`discover`](Self::discover) renders
    /// every page twice, measured at 309 ms for a 500-page book (audit 2026-09-24, F2).
    ///
    /// The target scan stays: the buffer lint runs the page pass every verb runs, which
    /// resolves `@sec-`/`@fig-` against `xref_targets`, so without it every valid
    /// cross-page reference read as broken in the editor. Numbers are not needed there
    /// (they come from the harvest render), only which targets exist.
    pub fn discover_registry(root: &Path) -> Site {
        let mut site = Self::registry(root, DraftMode::Exclude, None);
        site.xref_targets = scan_xref_targets(&site.pages, &mut site.warnings);
        xref::add_cell_label_targets(&site.pages, &mut site.xref_targets);
        site
    }

    /// Everything [`discover_scoped`](Self::discover_scoped) builds before its
    /// whole-project passes.
    fn registry(root: &Path, drafts: DraftMode, only: Option<&Path>) -> Site {
        let mut warnings = Vec::new();
        let mut excluded_drafts = Vec::new();
        let config = load_config(root, &mut warnings);
        // Everything `load_config` just pushed is a diagnostic about `_site.yml` itself, and
        // it runs first, so the config warnings are exactly this prefix of `warnings`. Kept
        // as a length because a caller that wants only those (`doctor`'s `config` row) had
        // no way to ask. See `config_warnings`.
        let config_warning_count = warnings.len();

        // A book takes its page set + order from the explicit `chapters:` list;
        // a website discovers every `.tmd` and orders by path.
        //
        // Scoped to one document, the page set is that document and nothing else is read,
        // so xrefs and search are built from it rather than filtered after the fact, and no
        // other page's front matter is parsed to find that out: a loose note beside 4000
        // others parsed all 4000 and reported each one's problems against it (leads
        // site/mod.rs:340). A book still parses its chapters (their order numbers this one)
        // but keeps only this page's diagnostics; a file that is no chapter is a page of its
        // own, so the one document named is always the one page.
        let book = config
            .is_book
            .then(|| build_book(root, &config, drafts, &mut excluded_drafts));
        let pages = match (only, &book) {
            (None, Some(book)) => book_pages(root, book, &mut warnings),
            (None, None) => website_pages(root, drafts, &mut warnings, &mut excluded_drafts),
            (Some(only), book) => {
                let want = only.canonicalize().unwrap_or_else(|_| only.to_path_buf());
                let same =
                    |p: &Page| p.input.canonicalize().unwrap_or_else(|_| p.input.clone()) == want;
                let mut sink = Vec::new();
                let chapter = book
                    .as_ref()
                    .map(|book| book_pages(root, book, &mut sink))
                    .and_then(|pages| pages.into_iter().find(same));
                let page = match chapter {
                    Some(page) => {
                        let rel = Some(page.rel.as_str());
                        warnings.extend(sink.into_iter().filter(|w| w.file.as_deref() == rel));
                        page
                    }
                    None => website_page(root, only.to_path_buf(), &mut warnings),
                };
                vec![page]
            }
        };

        // A `chapters:` entry naming a file that does not exist: the chapter is silently
        // skipped (its title falls back to the file stem, its body is empty), so a typo
        // in `_site.yml` drops a chapter with no signal.
        if let Some(book) = &book {
            for c in book.chapters() {
                if !root.join(&c.rel).exists() {
                    warnings.push(config_warning(
                        chapter_line(root, &c.rel),
                        Severity::Error,
                        format!(
                            "chapter file not found: `{}` (listed in _site.yml `chapters:`)",
                            c.rel
                        ),
                    ));
                }
            }
        }

        // Once, and against the site root: a project-wide `.bib` path is written
        // relative to `_site.yml`, not to whichever page happens to be rendering, and a bad
        // one should be reported once rather than on every page.
        let bibliography = bibliography::resolve_shared(root, &config.bibliography, &mut warnings);

        let standalone = only.is_some();

        Site {
            root: root.to_path_buf(),
            config,
            pages,
            book,
            xref_targets: HashMap::new(),
            bibliography,
            warnings,
            config_warning_count,
            // `discover_scoped` builds these once the registry's numbers exist: the search
            // index READS `xref_targets`, so building it here (as it used to) indexed every
            // cross-page `@fig-` before a single number had been harvested.
            search_index_json: String::new(),
            excluded_drafts,
            standalone,
        }
    }

    /// Rebuild the whole Cmd-K index from the pages' current sources, against the CURRENT
    /// registry. Separate from `discover` so the ordering requirement above has one name,
    /// and so [`refresh_xrefs`](Self::refresh_xrefs) can be followed by it — which the dev
    /// server does whenever a target MOVES. Whole-index, because the index is GLOBAL (one
    /// `search-index.js` for every tab): a per-page refresh keyed on the open tabs leaves a
    /// renumbered figure stale in the fragments of pages nobody has open, which is the
    /// exact snippet-contradicts-its-target defect this index ordering exists to prevent.
    pub fn rebuild_search_index(&mut self) {
        // The fragments are dropped once assembled. They were kept on `Site` for a per-page
        // refresh nothing calls, and holding them kept each rebuild's strings alive in the
        // allocator arenas of the threads that built them.
        let sections = search::build_sections(
            &self.pages,
            &self.book,
            &self.xref_targets,
            Some(&self.render_defaults()),
        );
        self.search_index_json = search::assemble(&sections);
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
        self.pages.iter().any(is_not_found_page)
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
    /// (`page_html_external`), which reaches the same helper through `SiteCtx`.
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
        // `PageIncludes` is now purely the chrome's own carrier for markup that belongs in
        // the `<head>` or at the top of the body — per-page OpenGraph / Twitter-card / SEO
        // meta so a shared link renders a rich preview, the feed links, the draft banner.
        // It had one AUTHOR-configured source, `_site.yml`'s `head:`, cut 2026-08-18 at zero
        // adoption; nothing an author writes reaches this any more.
        let mut includes = render::PageIncludes::default();
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
            // The author's 404 is served at any depth, so it names the root absolutely
            // (see `root_absolute_urls`, which does the same for its attributes).
            let up = if is_not_found_page(page) {
                "/".to_string()
            } else {
                "../".repeat(depth)
            };
            format!(
                "window.TALIESIN_SITE_ROOT=\"{up}\";window.TALIESIN_PAGE_URL=\"{}\";\
                 window.TALIESIN_SEARCH_URL=\"{up}search-index.js\"",
                search::json_str(&page.url)
            )
        };
        SiteCtx {
            // A book replaces the top navbar with a slim topbar + off-canvas chapter
            // drawer, and closes each chapter with a prev/next pager.
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
            // Empty outside a book: a website page's back-to-listing link is a block at the
            // top of the page (`expand_page`), not chrome under it.
            post_nav_html: self.book_nav_html(page, depth),
            book_sidebar: book.then(|| self.sidebar_html(page, depth)),
            includes,
            favicon,
            search_index,
            // The `<title>` suffix names the site on inner tabs; the root index stays bare.
            site_name: self.config.title.clone().unwrap_or_default(),
            is_home: self.is_home(page),
        }
    }

    /// The page HTML for a page whose blocks are already FINISHED ([`Self::finish_blocks`]),
    /// wrapped in its chrome and linking the shared `_assets/` bundle: what the site build
    /// writes once the page pass is done with the page.
    pub fn page_html_external(
        &self,
        page: &Page,
        doc: &render::RenderedDoc,
        assets: render::ExternalAssets,
    ) -> String {
        let ctx = self.page_chrome(page);
        let fallback = page.title.as_deref().unwrap_or("");
        let html = render::render_doc_to_page(
            doc,
            fallback,
            Some(&ctx),
            "",
            render::AssetMode::External(assets),
        );
        let html = rewrite_tmd_links(&html);
        // The host serves the author's 404 for any unknown path, at any depth, so its
        // depth-relative URLs (assets, navbar, favicon, the author's own links) would
        // resolve against the directory the reader mistyped.
        if is_not_found_page(page) {
            return root_absolute_urls(&html);
        }
        html
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
        let src = crate::includes::read_source(&page.input).ok()?;
        self.page_link_facts_from_src(page, &src)
    }

    /// [`page_link_facts`](Self::page_link_facts) over source already in hand — an editor
    /// buffer, which has no file to read.
    fn page_link_facts_from_src(&self, page: &Page, src: &str) -> Option<PageLinkFacts> {
        let base = page.input.parent().unwrap_or(&self.root);
        let doc = render::render_document_scoped_with_site(src, base, None, None);
        let mut ids = std::collections::HashSet::new();
        let mut links = Vec::new();
        for b in &doc.blocks {
            collect_html_ids(&b.html, &mut ids);
            let line = sourcepos_start_line(&b.sourcepos);
            for (path, frag) in manual_local_links(&b.html) {
                links.push(LinkRef {
                    path,
                    frag,
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

    /// Whether a link target that is no page (site-root-relative, as [`Self::link_target_url`]
    /// returns it) is a file the build publishes: `None` when no file is there, `Some(false)`
    /// for one the build never ships (a `.`-prefixed path, a symlink out of the checkout).
    /// The one publication rule, [`crate::includes::publishable`], so a link the gate
    /// accepts is a file the referenced-file pass deploys.
    pub(crate) fn raw_file_target(&self, target: &str) -> Option<bool> {
        let on_disk = crate::render::asset_fs_path(target);
        let file = self.root.join(&on_disk);
        crate::reads::probe(&file);
        if !file.is_file() {
            return None;
        }
        let reach = crate::includes::Reach::Referenced;
        Some(
            crate::includes::publishable(&self.root, &self.root, Path::new(&on_disk), reach)
                .is_ok(),
        )
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
                    // Above the site root. In a project that may be a mounted sibling, so it
                    // is left alone; a document built on its own has none, and the link names
                    // a file beside it on disk, which is dead in its page when it is missing.
                    let dir = Path::new(rel).parent().unwrap_or(Path::new(""));
                    if self.standalone && !self.root.join(dir).join(path).exists() {
                        let w = Warning::new(format!(
                            "broken link: `{path}` (no such file under the document directory)"
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
                    continue;
                };
                let Some(target_ids) = ids_by_url.get(target_url.as_str()) else {
                    // A target outside the page registry is only "broken" if nothing
                    // on disk backs it: a raw source file that exists under the root
                    // (`notes.md`, `data.csv`, an `_downloads/` PDF) is a legitimate target,
                    // and the build ships it via `deploy_referenced_sources`. Judged by the
                    // rule that build ships by (`includes::publishable`): a file in a
                    // `.`-prefixed folder exists and is never published, so a link to it is
                    // dead in the deploy.
                    if let Some(published) = self.raw_file_target(&target_url) {
                        if published {
                            continue;
                        }
                        let w = Warning::new(format!(
                            "broken link: `{path}` resolves to `{target_url}`, a file the build \
                             never publishes (a `.`-prefixed path, or a symlink out of the \
                             checkout)"
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
                        } else if self.standalone {
                            // A site discovered for one document builds that page alone
                            // (`build <file>`, a lone document's own project): the target may
                            // well be a page of the project, just not of this build.
                            format!("`{src}` is not built with this document")
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
                // The fragment matches as written or percent-decoded, the browser's two tries.
                if let Some(frag) = frag
                    && !frag.is_empty()
                    && !cells_by_url
                        .get(target_url.as_str())
                        .copied()
                        .unwrap_or(false)
                    && !target_ids.contains(frag)
                    && !target_ids.contains(&render::percent_decode(frag))
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

    /// Finish a page's blocks in place: site-wide cross-ref resolution (+ broken-ref
    /// warnings), and site front-matter expansion
    /// (`listing:`). The single block-finishing step shared by the page pass every
    /// verb runs and the live preview, so both produce identical blocks (the preview
    /// used to skip `validate_xrefs`). `page_toc` is computed by
    /// the caller (it reads blocks but doesn't mutate them).
    /// `src` is the page's own source text when the caller holds it, used only to narrow a
    /// broken-cross-reference warning from the whole line to the `@anchor` itself. `None` is
    /// a correct answer, not a gap: the warning keeps its whole-line span (which is what
    /// `build`'s `file:line` output shows anyway), and the render entry points here are
    /// handed a finished `RenderedDoc` rather than the text it came from.
    ///
    /// **Returns the page's `toc` flag, so the four callers cannot disagree about WHEN it
    /// is computed** (Fable audit FA17). `page_toc` reads the block list, and three callers
    /// asked it before this ran while `serve_site::build_page` asked it after. That was
    /// benign only by accident of the gate's own short-circuit (`page_toc` consults the
    /// blocks only for a page with no `listing:` and no `hero:`, which was then exactly the
    /// page `expand_page` left alone), i.e. by a coincidence between two functions that do not
    /// know about each other. Returning it here retires the question instead of restating
    /// the convention in four comments.
    pub fn finish_blocks(
        &self,
        page: &Page,
        blocks: &mut Vec<Block>,
        warnings: &mut Vec<Warning>,
        src: Option<&str>,
        toc_explicit: Option<bool>,
    ) -> bool {
        self.resolve_cross_refs(blocks, &page.url);
        // Cross-refs that survived the site-wide resolution are genuinely broken.
        warnings.extend(crate::cite::validate_xrefs(blocks, src));
        self.expand_page(page, blocks, warnings, src);
        self.page_toc(page, toc_explicit, blocks)
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
        // A special page is a short document, set by the reading surface's own elements and
        // nothing else: no type scale of its own. The centred box is its only layout, injected
        // into the head and reading the theme `--tali-*` vars like the rest of the site.
        const NOT_FOUND_STYLE: &str = "\n<style>\n\
            .tali-404{min-height:60vh;display:flex;flex-direction:column;\
            align-items:center;justify-content:center;text-align:center;gap:.3rem}\n\
            .tali-404 h1{margin:.4rem 0 0}\n\
            .tali-404 p{margin:.2rem 0;color:var(--tali-muted)}\n\
            .tali-404-home{display:inline-block;margin-top:1.4rem}\n\
            </style>";

        let site_title = self.config.title.as_deref().unwrap_or("the site");
        let body = format!(
            "<div class=\"tali-404\">\n\
             <h1>Page not found</h1>\n\
             <p>Nothing is published at this address.</p>\n\
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
            None,
            "",
            render::AssetMode::Inline { mermaid_src: "" },
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
        render::render_doc_to_page(
            &self.not_found_doc(),
            "Page not found",
            None,
            "",
            render::AssetMode::External(assets),
        )
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
        // The drawer this rule leans on is a book's chrome, which a chapter built on its
        // own does not have.
        if self.is_book() && !self.standalone {
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
    /// number depends only on its own page + chapter). That is a cost decision and it is
    /// INSTRUMENTED, not asserted: `tools/live-edit-bench` measures this pass per project and
    /// publishes the row (re-measured 2026-08-27 — `docs/guide` 16 pages / 3.2 ms,
    /// `docs/internals` 6 pages / 1.6 ms, `corpus/tech-blog` 17 pages / 4.6 ms; ~12x faster
    /// than the 2026-08-18 figures this line used to carry, from the render memos and the
    /// concurrent harvest below). Buying it back with incremental invalidation would have to
    /// re-derive the scan's project-wide "first definition wins" ordering to know whether a
    /// dropped anchor should fall through to another page's definition, which is not worth it
    /// at these sizes. A page-SET change re-runs `discover` anyway.
    ///
    /// **Still O(pages) on EVERY save**, at ~0.2 ms per page — but that is now wall clock
    /// across `available_parallelism()` workers (`fanout::map_ordered`), not per-core cost,
    /// so the same 200-page extrapolation is ~0.2 s here against the ~2.5 s it gave before.
    /// A single-core machine gets the memos but not the fan-out. The gate is deliberately
    /// absent because a wall clock measures the machine — but the extrapolation is the thing
    /// to check before assuming the published one-document warm-edit figure describes a
    /// book-sized save.
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
        let scanned = scan_xref_targets(&self.pages, &mut discarded);
        let prev_targets = std::mem::replace(&mut self.xref_targets, scanned);
        let harvested = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.harvest_xref_numbers();
        }));
        if harvested.is_err() {
            self.xref_targets = prev_targets;
        }
    }

    /// Render-harvest: render each page once (scoped to its chapter) and fill in the
    /// CROSS-PAGE facts the lightweight source-scan can't know — a section / figure /
    /// equation / table / listing number is assigned only during render, so
    /// `scan_xref_targets` left it empty. This enriches `xref_targets[anchor].number`, so a
    /// `@fig-x` to another page renders "Figure&nbsp;2.3" instead of a bare "Figure", and a
    /// `@sec-x` reads the number its heading shows (the render numbers a chapter's sections
    /// once, for the heading and every reference alike: `number_sections`).
    ///
    /// It also *inserts* an anchor the scan cannot see at all: a float labelled by a
    /// cell directive (`#| label: fig-x`, `%%| label:`) is inside a fence the scan skips
    /// and is not a brace id, so the render is the only thing that knows it exists.
    /// Reusing the render's own registry — rather than teaching the scan to parse cell
    /// options — keeps one source of truth, so the two cannot drift on which fences
    /// count as cells (the same reason `xref::brace_id` reuses `parse_attrs`).
    /// It names them too: a target's `title` is the text its heading shows on this render
    /// (`xref::heading_titles`), which is what an unnumbered cross-page `@sec-` reads.
    ///
    /// Called once by `discover`, so build AND the live preview resolve the same numbers.
    /// A pure render pass (no kernel execution), amortised across the discover it rides on.
    /// The render is `render::render_numbers_scoped_with_site`, which typesets nothing this
    /// discards.
    pub fn harvest_xref_numbers(&mut self) {
        // Collect during the `&self.pages` pass, then apply — keeps the borrows disjoint.
        // (anchor, number, defining page url) — the url is needed because an anchor the
        // source-scan cannot see is *inserted* here, not just enriched.
        let defaults = self.render_defaults();
        // Concurrent across pages, but collected back in PAGE order: the duplicate-label
        // rule below is "first definition wins", so completion order would let the winner
        // depend on which page rendered fastest. See `fanout::map_ordered`.
        let per_page = fanout::map_ordered(&self.pages, |page| {
            let Ok(src) = crate::includes::read_source(&page.input) else {
                return (Vec::new(), Vec::new());
            };
            let base = page.input.parent().unwrap_or(&self.root);
            let chapter = self.chapter_for(page);
            let doc = render::render_numbers_scoped_with_site(&src, base, chapter, Some(&defaults));
            let mut mine: Vec<(String, String, String)> = Vec::new();
            for (anchor, number) in doc.xref_numbers {
                // These three conditions gate an INSERT, not just an enrich, so each has
                // to hold on its own rather than lean on the entry already existing:
                //
                // A `sec-` number is taken from a numbered chapter only: a non-book
                // website has no section numbering, and harvesting the render's flat
                // per-page section counter would fill an empty website target with a bare
                // "1", which `rewrite_one_xref` then mislabels "Chapter 1".
                //
                // `is_ref_anchor` keeps parity with the scan (`xref.rs`), because the
                // render registry is LOOSER: the table-caption path registers any id, so
                // `: cap {#my-table}` arrives here. `@my-table` can never resolve (cite
                // rejects an unknown prefix), so admitting it would advertise a phantom
                // target in `taliesin map --format json` and build it a hover card.
                //
                // An empty number means the render assigned none, so there is nothing to
                // enrich the target with and nothing worth inserting one for.
                if !number.is_empty()
                    && (chapter.is_some() || !anchor.starts_with("sec-"))
                    && xref::is_ref_anchor(&anchor)
                {
                    mine.push((anchor, number, page.url.clone()));
                }
            }
            let titles: Vec<(String, String, String)> = xref::heading_titles(&doc.blocks)
                .into_iter()
                .map(|(anchor, title)| (anchor, title, page.url.clone()))
                .collect();
            (mine, titles)
        });
        let (updates, titles): (Vec<_>, Vec<_>) = per_page.into_iter().unzip();
        let updates: Vec<(String, String, String)> = updates.into_iter().flatten().collect();
        // Whether a label defined on two pages is already reported. The source-scan warns
        // for the anchors IT can see, so the check below covers only the ones it can't (a
        // cell label), and re-checking the list keeps a scan-warned duplicate from being
        // announced twice — and a third definition from announcing a fourth time.
        // Matching the curly-quoted anchor makes it exact, so `fig-a` never matches a
        // warning about `fig-abc`.
        let dup_reported = |warnings: &[Warning], anchor: &str| {
            let quoted = format!("\u{201c}{anchor}\u{201d}");
            warnings.iter().any(|w| {
                w.message.contains("duplicate cross-reference label") && w.message.contains(&quoted)
            })
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
                            // and the second that redefines it — and locate it on the second.
                            let mut w = Warning::new(format!(
                                "duplicate cross-reference label \u{201c}{}\u{201d} defined on both {} and {}; using {}",
                                e.key(),
                                e.get().url,
                                url,
                                e.get().url
                            ))
                            .severity(Severity::Error);
                            w.file = self
                                .pages
                                .iter()
                                .find(|p| p.url == url)
                                .map(|p| p.rel.clone());
                            self.warnings.push(w);
                        }
                        continue;
                    }
                    // Only fill a gap: the source-scan numbers nothing, so this is every
                    // number a target has.
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
        // A title only names a target this page defines: it never creates one, and a
        // duplicate on another page names nothing (the link goes to the first).
        for (anchor, title, url) in titles.into_iter().flatten() {
            if let Some(t) = self.xref_targets.get_mut(&anchor)
                && t.url == url
            {
                t.title = title;
            }
        }
    }

    /// This page's book chapter number, if it is a numbered chapter (None for a
    /// website page or an unnumbered preface). The render scoped to it numbers the
    /// sections, the floats and the theorems alike, so all three stay in lockstep.
    /// There is no key to turn numbering on: a chapter is numbered iff this gives it a
    /// number, i.e. it is a `chapters:` entry that is not the `index` preface and whose
    /// H1 carries no `.unnumbered` (see `book.rs`).
    pub fn chapter_for(&self, page: &Page) -> Option<u32> {
        book::chapter_of(&self.book, page)
    }

    // --- listings ---------------------------------------------------------

    /// Apply this page's site-level blocks to its rendered `blocks`, mutating in place: a
    /// `hero:` block replaces the title block, each `listing:` expands into post cards, and
    /// a page that one listing owns opens with a link back to it. Both the static build and
    /// the live preview call this, so the results stay in the block model (mounted + diffed
    /// like any other block). `src` is the page's source when the caller holds it, which
    /// locates a listing's diagnostics at the front-matter line that wrote them.
    pub fn expand_page(
        &self,
        page: &Page,
        blocks: &mut Vec<Block>,
        warnings: &mut Vec<Warning>,
        src: Option<&str>,
    ) {
        // A `hero:` block replaces the title block (a landing-page header treatment).
        if let Some(hero) = &page.hero {
            set_title_block(blocks, self.hero_html(page, hero));
        }
        for (li, spec) in page.listings.iter().enumerate() {
            let at = spec
                .id
                .as_ref()
                .and_then(|id| blocks.iter().position(|b| block_tag_has_id(&b.html, id)));
            // Where the cards land decides whether they are a block at all. Injected into an
            // author's `::: {#id}` container they belong to THAT block, which already carries
            // its own `data-block-id`; every other placement makes them a block of their own,
            // which needs one or the diff's `Update` reaches nothing (see `listing_block`).
            let adopted = at.is_some_and(|i| blocks[i].html.contains("</div>"));
            let id = listing_block_id(li, &spec.contents);
            let cards = self.listing_html(page, spec, (!adopted).then_some(id.as_str()), warnings);
            match at {
                // A `::: {#id}` container → inject the cards inside it.
                Some(i) if adopted => {
                    let pos = blocks[i].html.rfind("</div>").unwrap();
                    blocks[i].html.insert_str(pos, &cards);
                }
                // An anchor (e.g. an auto-slugged heading sharing the id) → cards go right
                // after it. An empty `::: {#id}` is a container, so it takes the arm above.
                Some(i) => blocks.insert(i + 1, listing_block(id, cards)),
                // No target at all → append so the listing still renders, and say so when
                // the author named one: the cards landing at the foot of the page is the
                // only other sign.
                None => {
                    if let Some(want) = &spec.id {
                        let mut w = Warning::new(format!(
                            "the listing on `{}` has `id: {want}`, but no element on the page \
                             has that id, so its cards were added at the end; put a \
                             `::: {{#{want}}}` block where they belong",
                            page.rel
                        ));
                        // The file line of the `id:` that names it: the block starts on the
                        // line after the opening `---`.
                        w.line = src
                            .and_then(crate::frontmatter::front_matter_block)
                            .and_then(|b| crate::frontmatter::value_line(b, Some("id"), want))
                            .map(|l| l as u32 + 1);
                        warnings.push(w);
                    }
                    blocks.push(listing_block(id, cards));
                }
            }
        }
        // The back-to-listing link opens the page, above the title. A book has none: each
        // chapter closes with the prev/next pager instead (`book_nav_html`).
        if !self.is_book()
            && let Some(backnav) = self.listing_backnav_block(page)
        {
            blocks.insert(0, backnav);
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
    /// "back to listing" link a post opens with.
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
    /// to the hosting page), **always newest-first**, capped by `max-items`. There is no
    /// order to configure: `sort:` was retired on 2026-08-02 and both the reference page
    /// and the register say so, so the one boolean it used to set is gone too — an Atom
    /// feed's order is not a document's to choose (`feed_hosts` calls this).
    fn collection(
        &self,
        host: &Page,
        spec: &ListingSpec,
        warnings: &mut Vec<Warning>,
    ) -> Vec<&Page> {
        let prefix = Self::listing_prefix(host, spec);
        // A `contents:` that names no directory (a typo, a glob) can list nothing. An
        // existing directory with no pages in it yet is a new blog, and stays silent.
        if !self.root.join(&prefix).is_dir() {
            warnings.push(
                Warning::new(format!(
                    "the listing on `{}` has `contents: {}`, but there is no such directory \
                     beside the page, so it lists nothing",
                    host.rel, spec.contents
                ))
                .severity(Severity::Error),
            );
        }
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
        // Order by calendar day (a page whose date names none sorts oldest), then by the
        // date as written (a time on the same day), then by rel; then reverse: newest
        // first, unconditionally.
        items.sort_by(|a, b| (a.day(), &a.date, &a.rel).cmp(&(b.day(), &b.date, &b.rel)));
        items.reverse();
        if let Some(n) = spec.max_items {
            items.truncate(n);
        }
        items
    }

    /// Render a listing's cards. `host` fixes the link/image depth so cards on a
    /// nested page still resolve. `block_id` is `Some` exactly when the `<ul>` is going to
    /// BE a block (see `expand_page`), and `None` when it is adopted into one that already
    /// carries an id — two `data-block-id`s in one block would give the client a second
    /// element to resolve an op against.
    fn listing_html(
        &self,
        host: &Page,
        spec: &ListingSpec,
        block_id: Option<&str>,
        warnings: &mut Vec<Warning>,
    ) -> String {
        let up = "../".repeat(host.url.matches('/').count());
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
        // The id goes LAST so the tag still opens with the semantics the a11y pin needles.
        let id_attr = block_id
            .map(|id| format!(" data-block-id=\"{}\"", esc(id)))
            .unwrap_or_default();
        format!(
            "<ul role=\"list\" class=\"tali-listing tali-listing-default\"{id_attr}>{cards}</ul>"
        )
    }

    fn card_html(&self, p: &Page, up: &str, with_image: bool) -> String {
        let href = format!("{up}{}", esc(&p.url));
        // A post with an `image:` shows it; a post without simply does not. The monogram
        // placeholder that used to fill the empty slot went on 2026-08-15 with spec §9's cut
        // #12: it existed to keep a text-only post ALIGNED beside its imaged neighbours in a
        // card grid, and a ruled list has no such alignment to keep.
        let img = match (with_image, &p.card_image) {
            (true, Some(src)) => format!(
                "<img class=\"tali-card-img\" src=\"{up}{}\" alt=\"{}\" loading=\"lazy\">",
                esc(src),
                esc(p.card_image_alt.as_deref().unwrap_or(""))
            ),
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
        // The category badges went on 2026-08-15 (spec §9 cut #12). `categories:` itself is
        // NOT retired and must not be: `feed.rs` still emits one `<category>` per entry in
        // the Atom feed, which is where a tag does real work. What is gone is the chip row —
        // and with it the last consumer of `data-cat`, an attribute already inert since the
        // listing category filter was deleted on 2026-08-04.
        // A draft card is badged so it reads as unpublished in a listing (preview only —
        // a built listing never contains a draft, so this is inert in `build`).
        let draft_badge = if p.draft {
            "<span class=\"tali-draft-badge\">Draft</span>"
        } else {
            ""
        };
        // `data-tali-src` lets the click-to-source locator jump to the post's source
        // (it's site-root-relative; resolved client-side, inert in the static build).
        // The title is an <h2>: the only heading before the cards is the page's (often
        // sr-only) <h1>, so an <h3> here skipped a level on every listing page — and the
        // heading-skip lint cannot see it, because the whole listing is ONE <ul> block
        // (T12, 2026-09-01). The stylesheet keys off `.tali-card-title`, never the tag,
        // so the rendering is unchanged.
        //
        // The thumbnail goes AFTER the body: the whole card is one link, whose accessible
        // name is its text in DOM order, and emitting the image first opened every card's
        // name with its alt text. `site.css` draws the image after the body already
        // (`order: 1`), so the move changes nothing on screen.
        format!(
            "<li class=\"tali-listing-item\"><a class=\"tali-card\" href=\"{href}\" data-tali-src=\"{src}\">\
             <div class=\"tali-card-body\">{draft_badge}{date}<h2 class=\"tali-card-title\">{title}</h2>{desc}</div>{img}</a></li>",
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
                    // Scheme-checked like a markdown link: `javascript:` is blanked.
                    format!(
                        "<a class=\"{cls}\" href=\"{}\">{}</a>",
                        esc(render::safe_url(&a.href, false)),
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

/// The block id a standalone listing takes. `index` is the listing's position on the page,
/// so two listings of the same `contents:` don't collide (which would break the diff).
///
/// Deliberately NOT a hash of the cards: a listed post's front-matter edit has to leave the
/// id alone so the diff can pair the two renders and emit one `Update` instead of a
/// remove-plus-insert that discards the element. That stability is exactly why the id must
/// also reach the emitted `<ul>` — `listing_html` interpolates this same string.
fn listing_block_id(index: usize, contents: &str) -> String {
    format!("listing-{index}-{}", contents.replace('/', "-"))
}

/// A synthetic block wrapping a listing card set (id-less listing, or no placeholder).
/// `cards_html` comes from `listing_html` carrying this same `id` as its `data-block-id`:
/// the block model's id and the element's must be one string, or `diff_blocks` aims an
/// `Update` at an element the client's `querySelector('[data-block-id=…]')` cannot find and
/// the op is dropped in silence.
fn listing_block(id: String, cards_html: String) -> Block {
    Block {
        id,
        sourcepos: String::new(),
        source_file: None,
        html: cards_html,
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

/// Walk a raw `.tmd` source's *content* lines: those comrak reads as markdown, so not the
/// front matter, not code (fenced or indented) and not raw HTML (a comment, `<pre>`,
/// `<script>`). Each yielded line is already `trim_start`ed, paired with its 1-based source
/// line number so a scan can point a diagnostic at exactly where an anchor lives. The
/// skeleton of the raw-source anchor scan, [`xref::scan_page_anchors`], so a `{#sec-x}`
/// inside front matter, a code sample or a heading the author commented out is never taken
/// for an anchor. It does NOT resolve `{{< include >}}`: the caller does, so an anchor in a
/// partial belongs to its page. Lines are split as comrak splits them, so a raw file with a
/// lone `\r` still lines up with its classification.
pub(super) fn content_lines_numbered(src: &str) -> impl Iterator<Item = (usize, &str)> {
    let lines = crate::render::rendered_lines(src);
    crate::lines::split(src)
        .enumerate()
        .filter(move |(i, _)| lines.line(*i).kind.is_markdown())
        .map(|(i, line)| (i + 1, line.trim_start()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Where [`Site::render_page`] links the shared bundle: the site build's `_assets/`, at
    /// the root depth (a test reads the markup, never these hrefs' depth).
    pub(crate) const TEST_ASSETS: render::ExternalAssets<'static> = render::ExternalAssets {
        app_css: "_assets/app.css",
        katex_css: "_assets/katex.css",
        app_js: "_assets/app.js",
        mermaid_js: "_assets/mermaid.js",
        jslibs_js: "_assets/jslibs.js",
        font_preload: "",
    };

    impl Site {
        /// Test convenience: one page (by rel-path or URL) as the site build writes it,
        /// rendered, finished and wrapped in its chrome linking `_assets/`. `None` when it is
        /// no page of this site.
        pub(crate) fn render_page(&self, rel_or_url: &str) -> Option<String> {
            self.render_page_warned(rel_or_url).map(|(html, _)| html)
        }

        /// [`Self::render_page`] with the page's warnings (the render's and the finish's).
        pub(crate) fn render_page_warned(
            &self,
            rel_or_url: &str,
        ) -> Option<(String, Vec<Warning>)> {
            let page = self.page(rel_or_url)?;
            let src = crate::includes::read_source(&page.input).ok()?;
            let base = page.input.parent().unwrap_or(&self.root);
            let doc = render::render_document_scoped_with_site(
                &src,
                base,
                self.chapter_for(page),
                Some(&self.render_defaults()),
            );
            Some(self.finish_page(page, doc, TEST_ASSETS))
        }

        /// The site build's last two steps for a rendered `doc`: [`Site::finish_blocks`],
        /// then [`Site::page_html_external`] linking `assets`.
        pub(crate) fn finish_page(
            &self,
            page: &Page,
            mut doc: render::RenderedDoc,
            assets: render::ExternalAssets,
        ) -> (String, Vec<Warning>) {
            let mut warnings = std::mem::take(&mut doc.warnings);
            doc.toc =
                self.finish_blocks(page, &mut doc.blocks, &mut warnings, None, doc.toc_explicit);
            (self.page_html_external(page, &doc, assets), warnings)
        }
    }

    #[test]
    fn content_lines_skips_front_matter_and_fenced_code() {
        // The skeleton of the raw-source anchor scan: front matter (even a `#`-looking
        // line in it) and fenced code (```/~~~, even a `# comment` inside) are dropped; the
        // real headings + prose survive, trim_start'ed. A `{#id}` in either region must
        // never read as an anchor.
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
        let lines: Vec<&str> = content_lines_numbered(src).map(|(_, t)| t).collect();
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
                // The title is the text the heading SHOWS, not its markdown source.
                (
                    "iter.tmd",
                    "# Using the `Iterator` trait &amp; *friends*\n\nx\n",
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
        assert_eq!(
            title_of("iter.tmd").as_deref(),
            Some("Using the Iterator trait & friends")
        );
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

    /// A file on disk is a legitimate link target only if the build publishes it. A link
    /// into a `.`-prefixed folder was excused because the file existed, and the build never
    /// ships one, so the deploy 404'd under a clean gate. An `_`-prefixed folder's file is
    /// referenced, so it ships and the link is fine.
    #[test]
    fn a_link_to_a_file_the_build_never_publishes_is_broken() {
        let root = write_site(
            "private-link",
            &[
                ("_site.yml", "title: T\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\n[notes](.notes/x.pdf) and [slides](_downloads/y.pdf).\n",
                ),
                (".notes/x.pdf", "x"),
                ("_downloads/y.pdf", "y"),
            ],
        );
        let msgs: Vec<String> = Site::discover(&root)
            .validate_cross_page_links()
            .into_iter()
            .map(|(_rel, w)| w.message)
            .collect();
        assert!(
            msgs.iter().any(|m| m.contains(".notes/x.pdf")),
            "a link into a dot folder is dead in the deploy: {msgs:?}"
        );
        assert!(
            !msgs.iter().any(|m| m.contains("y.pdf")),
            "a referenced `_downloads/` file ships: {msgs:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// An `&` in a page's file name, in an anchor and in a query is ordinary text to the
    /// reader's browser, and the cross-page check read all three still entity-encoded: a
    /// working link to `R&D.tmd` was reported three times as "resolves to `R&amp;D.html`,
    /// which is no page in this site", failing the publish gate.
    #[test]
    fn a_link_to_a_page_with_an_ampersand_in_its_name_resolves() {
        let root = write_site(
            "amp-link",
            &[
                ("_site.yml", "title: S\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\n[page](R&D.tmd) [section](R&D.tmd#q&a) \
                     [query](R&D.tmd?x=1&y=2) [gone](R&D.tmd#nope&x)\n",
                ),
                ("R&D.tmd", "---\ntitle: RD\n---\n\n## Q&A {#q&a}\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let msgs: Vec<String> = site
            .validate_cross_page_links()
            .into_iter()
            .map(|(_rel, w)| w.message)
            .collect();
        assert_eq!(msgs.len(), 1, "only the missing anchor: {msgs:?}");
        assert!(msgs[0].contains("`#nope&x`"), "{msgs:?}");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A `%20` in a link is how a file name with a space is spelled in a URL, and the
    /// browser decodes it; a fragment matches an id as written or percent-decoded. The
    /// cross-page check and the nav check compared the encoded text, so every link below
    /// failed the publish gate while working in the browser.
    #[test]
    fn a_percent_encoded_link_resolves_like_the_browser_resolves_it() {
        let root = write_site(
            "pct-link",
            &[
                (
                    "_site.yml",
                    "title: S\nnav:\n  left:\n    - text: Notes\n      href: my%20notes.tmd\n",
                ),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\n[file](my%20file.txt) [notes](my%20notes.tmd) \
                     [uber](my%20notes.tmd#%C3%BCber) [gone](my%20notes.tmd#nope)\n",
                ),
                (
                    "my notes.tmd",
                    "---\ntitle: N\n---\n\n## Über {#über}\n\nx\n",
                ),
                ("my file.txt", "x\n"),
            ],
        );
        let site = Site::discover(&root);
        let msgs: Vec<String> = site
            .validate_cross_page_links()
            .into_iter()
            .map(|(_rel, w)| w.message)
            .collect();
        assert_eq!(msgs.len(), 1, "only the missing anchor: {msgs:?}");
        assert!(msgs[0].contains("`#nope`"), "{msgs:?}");
        let nav: Vec<String> = site
            .validate_chrome_links()
            .into_iter()
            .map(|w| w.message)
            .collect();
        assert!(nav.is_empty(), "the nav link works: {nav:?}");

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

    /// `listing: sort:` was retired on 2026-08-02 and the register answers a leftover with
    /// "newest first is the only order now, so delete the key" — but `parse_listing_spec`
    /// went on reading it until 2026-08-13, so `sort: "date asc"` really did reverse the
    /// cards while the tool said the key did not exist. The register entry cannot say the
    /// parser stopped consuming it; this does (the sibling of
    /// `parse_hero_ignores_the_retired_image_keys`).
    ///
    /// Both surfaces, because `feed_hosts` calls the same `collection`: a document claim is
    /// at least the author's to make, but the Atom feed's order is not, and it was coming
    /// out oldest-first too.
    #[test]
    fn a_retired_listing_sort_cannot_reverse_the_cards_or_the_feed() {
        let root = write_site(
            "listsort",
            &[
                ("_site.yml", "title: T\nurl: https://example.com\n"),
                (
                    "index.tmd",
                    "---\ntitle: Blog\nlisting:\n  contents: posts\n  sort: \"date asc\"\n---\n\nx\n",
                ),
                (
                    "posts/old.tmd",
                    "---\ntitle: Older\ndate: 2026-01-01\n---\nx\n",
                ),
                (
                    "posts/new.tmd",
                    "---\ntitle: Newer\ndate: 2026-06-01\n---\nx\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let host = site.pages.iter().find(|p| p.rel == "index.tmd").unwrap();
        let spec = host.listings.first().expect("the listing parses");
        let mut sink = Vec::new();
        let titles: Vec<&str> = site
            .collection(host, spec, &mut sink)
            .iter()
            .map(|p| p.title.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(
            titles,
            vec!["Newer", "Older"],
            "newest first is the only order; a retired `sort:` must not reverse it"
        );
        let feeds = site.feed_hosts();
        let (_, _, dated) = feeds.first().expect("the dated listing gets a feed");
        assert_eq!(
            dated
                .iter()
                .map(|p| p.title.as_deref().unwrap_or(""))
                .collect::<Vec<_>>(),
            vec!["Newer", "Older"],
            "the Atom feed is newest-first regardless of any front-matter key"
        );
        assert!(
            !format!("{spec:?}").contains("asc"),
            "a retired `sort:` must not reach ListingSpec: {spec:?}"
        );
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
                .any(|w| w.message.contains("draft") && w.message.contains("YAML 1.2")),
            "a `draft: yes` page must warn to use `true`: {warnings:?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// A `draft:` value that is neither a bool nor one of YAML 1.1's bool words (`1`, `y`,
    /// `x`, `[true]`) fell back to "not a draft", so the page was published, listed and put
    /// in the feed with no diagnostic. It is held back now (a flag the tool cannot read must
    /// fail safe) and reported; a null `draft:` is simply unset.
    #[test]
    fn an_unreadable_draft_flag_holds_the_page_back_and_warns() {
        let root = write_site(
            "draftunreadable",
            &[
                ("_site.yml", "title: T\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nHome.\n"),
                ("one.tmd", "---\ntitle: One\ndraft: 1\n---\n\nx\n"),
                ("y.tmd", "---\ntitle: Y\ndraft: y\n---\n\nx\n"),
                ("list.tmd", "---\ntitle: L\ndraft: [true]\n---\n\nx\n"),
                ("null.tmd", "---\ntitle: N\ndraft: ~\n---\n\nx\n"),
                ("no.tmd", "---\ntitle: No\ndraft: false\n---\n\nx\n"),
            ],
        );
        let mut warnings = Vec::new();
        let rels: Vec<String> =
            website_pages(&root, DraftMode::Exclude, &mut warnings, &mut Vec::new())
                .iter()
                .map(|p| p.rel.clone())
                .collect();
        for held in ["one.tmd", "y.tmd", "list.tmd"] {
            assert!(
                !rels.contains(&held.to_string()),
                "{held} held back: {rels:?}"
            );
            assert!(
                warnings
                    .iter()
                    .any(|w| w.file.as_deref() == Some(held)
                        && w.message.contains("not a boolean")),
                "{held} reported: {warnings:?}"
            );
        }
        for kept in ["null.tmd", "no.tmd"] {
            assert!(
                rels.contains(&kept.to_string()),
                "{kept} published: {rels:?}"
            );
            assert!(
                !warnings.iter().any(|w| w.file.as_deref() == Some(kept)),
                "{kept} is not reported: {warnings:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A file whose lines end in a lone CR (a classic-Mac tool, pasted terminal output) is
    /// one line to `str::lines`, while comrak and the render path split it. Discovery read
    /// such a file raw, found no front matter, and PUBLISHED a `draft: true` page, listed it
    /// and indexed its body. Every raw `.tmd` reader now goes through
    /// `includes::read_source`, which normalizes line endings the way the render path does.
    #[test]
    fn a_lone_cr_file_is_read_like_any_other() {
        let root = write_site(
            "lonecr",
            &[
                ("_site.yml", "title: T\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nHome.\n"),
                (
                    "posts/d.tmd",
                    "---\rtitle: Draft\rdraft: true\r---\r\rSecret draft body.\r",
                ),
                ("posts/p.tmd", "---\rtitle: Kept\r---\r\rPublished.\r"),
            ],
        );
        let site = Site::discover(&root);
        assert!(
            !site.pages.iter().any(|p| p.rel == "posts/d.tmd"),
            "a lone-CR `draft: true` is a draft: {:?}",
            site.pages.iter().map(|p| &p.rel).collect::<Vec<_>>()
        );
        let kept = site.pages.iter().find(|p| p.rel == "posts/p.tmd").unwrap();
        assert_eq!(kept.title.as_deref(), Some("Kept"));
        let _ = std::fs::remove_dir_all(&root);

        // A book chapter's title fallback reads the file too.
        let root = write_site(
            "lonecrbook",
            &[
                ("_site.yml", "title: B\nchapters:\n  - one.tmd\n"),
                (
                    "one.tmd",
                    "---\rdescription: x\r---\r\r# The first chapter\r\rText.\r",
                ),
            ],
        );
        let site = Site::discover(&root);
        let chapters = site.book.as_ref().unwrap().chapters();
        assert_eq!(chapters[0].title, "The first chapter");
        let _ = std::fs::remove_dir_all(&root);
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

    /// The host serves an author's `404.html` for ANY unknown path, at any depth, so every
    /// URL in it must be root-absolute, as the generated 404's already are. Relative ones
    /// resolve against the directory the reader mistyped: on the live blog on 2026-09-23,
    /// `/a/b/zz` rendered without its stylesheet, with a navbar of dead links. Every other
    /// page keeps its relative URLs, which the portable `file://` build depends on.
    #[test]
    fn author_404_links_everything_from_the_site_root() {
        use std::fs;
        let root = std::env::temp_dir().join(format!("tali-404-abs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("_site.yml"),
            "title: Demo\nurl: https://example.com\nfavicon: icon.svg\nlogo: logo.svg\n\
             nav:\n  left:\n  - text: Blog\n    href: blog.tmd\n\
             footer:\n  right:\n  - { icon: rss, href: blog.xml }\n",
        )
        .unwrap();
        fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
        fs::write(root.join("blog.tmd"), "---\ntitle: Blog\n---\n\nPosts.\n").unwrap();
        fs::write(
            root.join("404.tmd"),
            "---\ntitle: Lost\n---\n\nTry the [blog](blog.tmd), [home](/), [top](#top) \
             or [elsewhere](https://example.com/x).\n\n<img src=\"./lost.png\" alt=\"\">\n",
        )
        .unwrap();
        let site = Site::discover(&root);
        let ext = || render::ExternalAssets {
            app_css: "_assets/app.1.css",
            katex_css: "_assets/katex.2.css",
            app_js: "_assets/app.3.js",
            mermaid_js: "_assets/mermaid.4.js",
            jslibs_js: "_assets/jslibs.5.js",
            font_preload: "_assets/font.6.woff2",
        };
        // Rendered the way `build <dir>` renders each page.
        let built = |rel: &str| {
            let page = site.page(rel).expect("page");
            let src = fs::read_to_string(&page.input).unwrap();
            let doc = render::render_document_scoped_with_site(
                &src,
                &root,
                site.chapter_for(page),
                Some(&site.render_defaults()),
            );
            site.finish_page(page, doc, ext()).0
        };
        let urls = |html: &str| {
            let mut out = Vec::new();
            for t in render::tags(html) {
                for a in render::attrs(&t) {
                    if ["href", "src", "poster", "srcset"]
                        .iter()
                        .any(|n| a.name.eq_ignore_ascii_case(n))
                    {
                        out.push(a.value.to_string());
                    }
                }
            }
            out
        };

        let not_found = built("404.tmd");
        let relative: Vec<String> = urls(&not_found)
            .into_iter()
            .filter(|v| !v.starts_with('/') && !links::is_external_or_special(v))
            .collect();
        assert!(
            relative.is_empty(),
            "relative URLs in 404.html: {relative:?}"
        );
        for expected in [
            "/_assets/app.1.css",
            "/_assets/app.3.js",
            "/_assets/font.6.woff2",
            "/icon.svg",
            "/logo.svg",
            "/blog.html",
            "/blog.xml",
            "/lost.png",
            "#top",
            "https://example.com/x",
        ] {
            assert!(
                urls(&not_found).iter().any(|v| v == expected),
                "404.html should carry {expected}: {:?}",
                urls(&not_found)
            );
        }
        // Cmd-K resolves a result against the site root it is handed, not an attribute.
        assert!(
            not_found.contains("window.TALIESIN_SITE_ROOT=\"/\"")
                && not_found.contains("window.TALIESIN_SEARCH_URL=\"/search-index.js\""),
            "the 404's search globals must be root-absolute"
        );

        // Any other page is untouched: still relative, so the build opens from disk.
        let home = built("index.tmd");
        assert!(urls(&home).iter().any(|v| v == "_assets/app.1.css"));
        assert!(urls(&home).iter().any(|v| v == "blog.html"));
        assert!(home.contains("window.TALIESIN_SITE_ROOT=\"\""));

        let _ = fs::remove_dir_all(&root);
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

    /// The harvest renders every page on every save and keeps only the numbers and the
    /// heading text, so a paragraph's math and a block's code are typeset for nothing: they
    /// were most of a harvest render, and past a memo's capacity every save redid the whole
    /// project's (10.7 s per save at 9,693 math expressions, audit 2026-09-24, F1). A
    /// heading's math is still typeset, because its text names an unnumbered `@sec-` link.
    /// Witnessed through the memos: what nothing typeset is not in them.
    #[test]
    fn the_harvest_numbers_a_page_without_typesetting_its_body_math() {
        let root = write_site(
            "harvest-math",
            &[
                (
                    "_site.yml",
                    "title: B\nchapters:\n  - index.tmd\n  - one.tmd\n",
                ),
                ("index.tmd", "# Preface {.unnumbered}\n\nSee @sec-probe.\n"),
                (
                    "one.tmd",
                    "# One\n\n## The $x_{h7731}$ case {#sec-probe}\n\n\
                     Body $y_{b7731}$ text.\n\n$$ z_{e7731} $$ {#eq-probe}\n\n\
                     ```python\nprobe_7731 = 1\n```\n\n\
                     ```{python}\n#| label: lst-probe\n#| lst-cap: A listing.\nlisted_7731 = 2\n```\n",
                ),
            ],
        );
        let mut site = Site::discover_registry(&root);
        site.harvest_xref_numbers();
        assert_eq!(site.xref_targets["eq-probe"].number, "1.1");
        assert_eq!(site.xref_targets["sec-probe"].number, "1.1");
        assert_eq!(site.xref_targets["lst-probe"].number, "1.1");
        for code in ["probe_7731 = 1\n", "listed_7731 = 2\n"] {
            assert!(
                !crate::highlight::is_memoized(code, "python"),
                "the harvest highlighted `{code}`, which only the served page shows"
            );
        }
        assert!(
            crate::math::is_memoized("x_{h7731}", false),
            "a heading's math names its section, so the harvest typesets it"
        );
        for (latex, display) in [("y_{b7731}", false), ("z_{e7731}", true)] {
            assert!(
                !crate::math::is_memoized(latex, display),
                "the harvest typeset `{latex}`, which only the served page shows"
            );
        }
        // The title is what the served page's heading reads, math included.
        let page = site.page("one.tmd").unwrap();
        let src = std::fs::read_to_string(&page.input).unwrap();
        let full = render::render_document_scoped_with_site(
            &src,
            &root,
            site.chapter_for(page),
            Some(&site.render_defaults()),
        );
        assert_eq!(
            site.xref_targets["sec-probe"].title,
            xref::heading_titles(&full.blocks)["sec-probe"]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The harvest's render skips typesetting, so it must still agree with the served render
    /// on everything the harvest keeps: every number and every heading title, on every page
    /// of every project here (the corpus and both books).
    #[test]
    fn the_numbers_render_agrees_with_the_full_render_on_every_real_page() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut roots = vec![repo.join("docs/guide"), repo.join("docs/internals")];
        let mut stack = vec![repo.join("corpus")];
        while let Some(dir) = stack.pop() {
            if dir.join("_site.yml").is_file() {
                roots.push(dir.clone());
            }
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                if entry.path().is_dir() {
                    stack.push(entry.path());
                }
            }
        }
        let mut pages = 0;
        for root in roots {
            let site = Site::discover_registry(&root);
            let defaults = site.render_defaults();
            for page in &site.pages {
                let Ok(src) = crate::includes::read_source(&page.input) else {
                    continue;
                };
                let base = page.input.parent().unwrap();
                let chapter = site.chapter_for(page);
                let full =
                    render::render_document_scoped_with_site(&src, base, chapter, Some(&defaults));
                let numbers =
                    render::render_numbers_scoped_with_site(&src, base, chapter, Some(&defaults));
                assert_eq!(
                    numbers.xref_numbers,
                    full.xref_numbers,
                    "{}",
                    page.input.display()
                );
                assert_eq!(
                    xref::heading_titles(&numbers.blocks),
                    xref::heading_titles(&full.blocks),
                    "{}",
                    page.input.display()
                );
                pages += 1;
            }
        }
        assert!(pages > 50, "only {pages} pages compared");
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
    fn a_document_s_discovery_scopes_the_project_to_that_document() {
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
        let site = Site::discover_document(&root.join("note.tmd"));
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
        let site = Site::discover_document(&root.join("note.tmd"));
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

    /// A `listing:` whose `contents:` names no directory (a typo, a glob) renders no
    /// cards, and nothing said so. An existing directory with no pages in it yet is a new
    /// blog, not a mistake, and stays silent.
    #[test]
    fn a_listing_whose_contents_names_no_directory_is_diagnosed() {
        let root = write_site(
            "listingcontents",
            &[
                ("_site.yml", "title: T\n"),
                (
                    "typo.tmd",
                    "---\ntitle: Typo\nlisting:\n  contents: post\n---\n\nx\n",
                ),
                (
                    "glob.tmd",
                    "---\ntitle: Glob\nlisting:\n  contents: \"posts/*.tmd\"\n---\n\nx\n",
                ),
                (
                    "ok.tmd",
                    "---\ntitle: Ok\nlisting:\n  contents: posts\n---\n\nx\n",
                ),
                ("posts/a.tmd", "---\ntitle: A\n---\n\nx\n"),
                (
                    "empty.tmd",
                    "---\ntitle: Empty\nlisting:\n  contents: drafts\n---\n\nx\n",
                ),
                ("drafts/.keep", ""),
            ],
        );
        let site = Site::discover(&root);
        for (rel, contents) in [("typo.tmd", "post"), ("glob.tmd", "posts/*.tmd")] {
            let (_, warnings) = render_page(&site, rel);
            assert!(
                warnings
                    .iter()
                    .any(|w| w.message.contains(&format!("`contents: {contents}`"))
                        && w.message.contains("no such directory")
                        && w.severity == Severity::Error),
                "{rel}: {warnings:?}"
            );
        }
        for rel in ["ok.tmd", "empty.tmd"] {
            let (_, warnings) = render_page(&site, rel);
            assert!(
                !warnings
                    .iter()
                    .any(|w| w.message.contains("no such directory")),
                "{rel}: {warnings:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Render `rel` in `site` and return (html, render-warnings).
    fn render_page(site: &Site, rel: &str) -> (String, Vec<Warning>) {
        site.render_page_warned(rel).expect("a page of the site")
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
        let doc = crate::render::render_document_scoped_with_site(&src, &site.root, None, None);
        let ext = render::ExternalAssets {
            app_css: "_assets/app.a.css",
            katex_css: "_assets/katex.b.css",
            app_js: "_assets/app.c.js",
            mermaid_js: "_assets/mermaid.d.js",
            jslibs_js: "_assets/jslibs.e.js",
            font_preload: "",
        };
        let (html, _w) = site.finish_page(page, doc, ext);
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

    /// A `listing:` carrying an `id:` renders its cards AT that id, not appended to the end
    /// of the page. Two page shapes reach two different arms of `expand_page`'s target lookup
    /// and both must place the cards before the prose that follows the target: a `::: {#id}`
    /// container with content in it, and a bare `::: {#id}` placeholder, which emits no block
    /// at all, so the id is the auto-slugged heading's and the cards become the block after it.
    ///
    /// `corpus/tech-blog/index.tmd` was the only thing in the tree exercising either arm until
    /// it dropped its `recent-posts` listing on 2026-08-14, so this is the witness now. It pins
    /// placement relative to the target, not which arm ran.
    #[test]
    fn an_id_listing_lands_at_its_target() {
        let root = write_site(
            "listing-id-target",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "filled.tmd",
                    "---\ntitle: Filled\nlisting:\n  id: picks\n  contents: posts\n---\n\n## Selected\n\n::: {#picks}\nPicked by hand.\n:::\n\nTrailing paragraph.\n",
                ),
                (
                    "anchored.tmd",
                    "---\ntitle: Anchored\nlisting:\n  id: recent-posts\n  contents: posts\n---\n\n## Recent Posts\n\nTrailing paragraph.\n",
                ),
                // The guide's blog recipe verbatim: an EMPTY `::: {#recent}` under a heading
                // whose slug is something else. It used to emit no element, so the cards were
                // appended past "View all posts" with no diagnostic.
                (
                    "recipe.tmd",
                    "---\ntitle: Recipe\nlisting:\n  id: recent\n  contents: posts\n---\n\n## Recent posts\n\n::: {#recent}\n:::\n\n[View all posts](blog.tmd)\n",
                ),
                (
                    "missing.tmd",
                    "---\ntitle: Missing\nlisting:\n  id: nowhere\n  contents: posts\n---\n\nBody.\n",
                ),
                ("posts/one.tmd", "---\ntitle: One\n---\n\nOne.\n"),
                ("posts/two.tmd", "---\ntitle: Two\n---\n\nTwo.\n"),
            ],
        );
        let site = Site::discover(&root);

        // Container shape: cards land after the container's own prose and before the page's.
        let (filled, _) = render_page(&site, "filled.tmd");
        let at = |hay: &str, needle: &str| -> usize {
            hay.find(needle)
                .unwrap_or_else(|| panic!("missing {needle:?} in: {hay}"))
        };
        assert!(
            at(&filled, "id=\"picks\"") < at(&filled, "Picked by hand.")
                && at(&filled, "Picked by hand.") < at(&filled, "class=\"tali-card\"")
                && at(&filled, "class=\"tali-card\"") < at(&filled, "Trailing paragraph."),
            "cards must render at the #picks target, not appended past the page: {filled}"
        );
        // Adopted into the author's block, the cards are NOT a block of their own, so the
        // `<ul>` must carry no `data-block-id`: a second one inside the container's block
        // gives the client another element to resolve an op's target against.
        assert!(
            filled.contains("<ul role=\"list\" class=\"tali-listing tali-listing-default\">"),
            "an adopted listing must not carry its own block id: {filled}"
        );

        // Placeholder shape: an empty `::: {#recent}` is still an element, so the cards are
        // adopted into it, ahead of the link that follows, and nothing warns.
        let (recipe, recipe_warnings) = render_page(&site, "recipe.tmd");
        let div = at(&recipe, "id=\"recent\"");
        assert!(
            div < at(&recipe, "class=\"tali-card\"")
                && at(&recipe, "class=\"tali-card\"") < at(&recipe, "View all posts"),
            "cards must render inside the empty #recent div, before the link: {recipe}"
        );
        assert!(
            recipe_warnings
                .iter()
                .all(|w| !w.message.contains("listing")),
            "a listing that found its target draws no listing warning: {recipe_warnings:?}"
        );

        // No target at all: the cards still render (appended), and the author is told.
        let (_, missing_warnings) = render_page(&site, "missing.tmd");
        assert!(
            missing_warnings.iter().any(
                |w| w.message.contains("`id: nowhere`") && w.message.contains("::: {#nowhere}")
            ),
            "a listing id that names nothing must warn: {missing_warnings:?}"
        );

        // Anchor shape: the id is the heading's own slug and no div exists, so the cards
        // follow the heading, still ahead of the trailing prose.
        let (anchored, _) = render_page(&site, "anchored.tmd");
        assert!(
            at(&anchored, "id=\"recent-posts\"") < at(&anchored, "class=\"tali-card\"")
                && at(&anchored, "class=\"tali-card\"") < at(&anchored, "Trailing paragraph."),
            "cards must render at the #recent-posts anchor: {anchored}"
        );
        // The other arm: an anchor is not a container, so these cards ARE their own block and
        // must carry the id `diff_blocks` will aim an `Update` at.
        assert!(
            anchored.contains(
                "<ul role=\"list\" class=\"tali-listing tali-listing-default\" \
                 data-block-id=\"listing-0-posts\">"
            ),
            "a listing placed after an anchor is its own block and needs its id: {anchored}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A standalone listing is a block of its own, so its element must carry the
    /// `data-block-id` the incremental update aims at it.
    ///
    /// `listing_block`'s id is deliberately stable across a content change — it is the
    /// listing's position plus its `contents:`, not a hash of the cards — so editing a listed
    /// post's front matter leaves the id alone and `diff_blocks` emits
    /// `Update { target_id: "listing-0-posts" }`. The client resolves every op through
    /// `querySelector('[data-block-id=…]')`, and the `<ul>` carried none: the op matched
    /// nothing, was dropped without a word, and the open index page went on showing the old
    /// card until the reader reloaded.
    ///
    /// Both halves read the id back through the tag walker and compare it to the block
    /// model's own, so the two can never drift into agreeing only by spelling.
    #[test]
    fn a_standalone_listing_block_is_targetable_by_the_op_that_updates_it() {
        let root = write_site(
            "listingblockid",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\nlisting:\n  contents: posts\n---\n\n# Home\n",
                ),
                (
                    "posts/a.tmd",
                    "---\ntitle: First\ndate: 2026-01-01\n---\n\nBody.\n",
                ),
            ],
        );
        // The page's finished blocks, as the preview and the build both see them.
        let blocks_of = |root: &Path| -> Vec<Block> {
            let site = Site::discover(root);
            let page = site.pages.iter().find(|p| p.rel == "index.tmd").unwrap();
            let src = std::fs::read_to_string(&page.input).unwrap();
            let mut doc =
                crate::render::render_document_scoped_with_site(&src, &site.root, None, None);
            let mut warnings = Vec::new();
            site.finish_blocks(page, &mut doc.blocks, &mut warnings, Some(&src), None);
            doc.blocks
        };

        let old = blocks_of(&root);
        let listing = old
            .iter()
            .find(|b| b.id.starts_with("listing-"))
            .expect("the page has a listing block");
        assert_eq!(
            crate::render::attr_values(&listing.html, "data-block-id").collect::<Vec<_>>(),
            vec![listing.id.as_str()],
            "the listing's own element must carry its block id, once: {}",
            listing.html
        );

        // End to end: a listed post is renamed, so the cards change while the listing block's
        // id does not — exactly the Update the client has to be able to find.
        std::fs::write(
            root.join("posts/a.tmd"),
            "---\ntitle: Renamed\ndate: 2026-01-01\n---\n\nBody.\n",
        )
        .unwrap();
        let new = blocks_of(&root);
        let ops = crate::diff::diff_blocks(&old, &new);
        let html = ops
            .iter()
            .find_map(|op| match op {
                crate::diff::BlockOp::Update { target_id, html } if *target_id == listing.id => {
                    Some(html)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no Update aimed at the listing block: {ops:?}"));
        assert!(
            crate::render::attr_values(html, "data-block-id").any(|v| v == listing.id),
            "the op's html must carry the id the op targets, or `elById` resolves nothing \
             and the stale cards stay on screen: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `max-items:` caps how many cards a listing renders, keeping the newest. (What the cap
    /// does to listing OWNERSHIP is a separate rule, pinned by
    /// `capped_preview_does_not_own_but_full_list_does`.)
    #[test]
    fn max_items_caps_the_cards_a_listing_renders() {
        let root = write_site(
            "listing-maxitems",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "all.tmd",
                    "---\ntitle: All\nlisting:\n  contents: posts\n---\n\n# All\n",
                ),
                (
                    "recent.tmd",
                    "---\ntitle: Recent\nlisting:\n  contents: posts\n  max-items: 2\n---\n\n# Recent\n",
                ),
                (
                    "posts/oldest.tmd",
                    "---\ntitle: Oldest\ndate: 2026-01-01\n---\n\nBody.\n",
                ),
                (
                    "posts/middle.tmd",
                    "---\ntitle: Middle\ndate: 2026-02-01\n---\n\nBody.\n",
                ),
                (
                    "posts/newest.tmd",
                    "---\ntitle: Newest\ndate: 2026-03-01\n---\n\nBody.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let (all, _) = render_page(&site, "all.tmd");
        assert_eq!(
            all.matches("class=\"tali-card\"").count(),
            3,
            "an uncapped listing renders every post: {all}"
        );
        let (recent, _) = render_page(&site, "recent.tmd");
        assert_eq!(
            recent.matches("class=\"tali-card\"").count(),
            2,
            "max-items: 2 must cap the cards: {recent}"
        );
        assert!(
            recent.contains("Newest") && recent.contains("Middle") && !recent.contains("Oldest"),
            "the cap keeps the newest two and drops the oldest: {recent}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The back-to-listing link's opening tag, shared by every assertion below so a negative
    /// one ("no backlink here") cannot pass vacuously after the markup changes.
    const BACKNAV: &str = "<nav class=\"tali-listing-backnav\"";

    #[test]
    fn the_backlink_leads_the_page_above_its_title() {
        // The link is the page's first block, so it sits in the reading column's text track
        // above the title. It used to trail `<main>` in the chrome's post-nav slot, a sibling
        // of the reading grid rather than a child of it, which put it at the window's edge.
        let root = write_site(
            "backlink-top",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "blog.tmd",
                    "---\ntitle: Blog\nlisting:\n  contents: posts\n---\n\n# Blog\n",
                ),
                ("posts/one.tmd", "---\ntitle: One\n---\n\nOne.\n"),
            ],
        );
        let site = Site::discover(&root);
        let page = site
            .pages
            .iter()
            .find(|p| p.rel == "posts/one.tmd")
            .unwrap();
        let src = std::fs::read_to_string(&page.input).unwrap();
        let mut doc = crate::render::render_document_scoped_with_site(&src, &site.root, None, None);
        site.finish_blocks(page, &mut doc.blocks, &mut Vec::new(), None, None);
        assert_eq!(
            doc.blocks.first().map(|b| b.id.as_str()),
            Some("tali-backnav"),
            "the backlink is a block, first in the page, so the preview mounts it too"
        );
        let (post, _) = render_page(&site, "posts/one.tmd");
        let main = post.find("<main id=\"tali-main\"").expect("a <main>");
        let nav = post.find(BACKNAV).expect("a backlink");
        let title = post
            .find("<header class=\"tali-title-block\"")
            .expect("a title block");
        assert!(
            main < nav && nav < title,
            "the backlink must open <main>, above the title: {post}"
        );
        assert!(
            !post.contains("tali-postnav"),
            "a website post leaves the bottom post-nav slot empty: {post}"
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
            post.contains(BACKNAV)
                && post.contains("href=\"../blog.html\"")
                && post.contains("</span> Blog</a>"),
            "sole un-capped listing should own the post: {post}"
        );
        // The listing page itself belongs to no listing → no backlink.
        let (blog, _) = render_page(&site, "blog.tmd");
        assert!(
            !blog.contains(BACKNAV),
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
            !post.contains(BACKNAV),
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
            post.contains(BACKNAV) && post.contains("</span> Blog</a>"),
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
            !post.contains(BACKNAV),
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
            !post.contains(BACKNAV),
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
                    "---\ntitle: Home\nlisting:\n  type: list\n---\n\nHi.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        assert!(
            site.warnings
                .iter()
                .any(|w| w.message.contains("listing") && w.message.contains("contents")),
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
                .any(|w| w.message.contains("missing.tmd")
                    && w.message.contains("chapter file not found")),
            "{:?}",
            site.warnings
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A card is one link, and a link's accessible name is its text in DOM order, so the
    /// thumbnail's alt, emitted first, opened the name of every card ("A cover 1 September
    /// 2026 Post One ..."). The image is emitted after the card body. `site.css` already
    /// draws it there (`.tali-card-img { order: 1 }`), so nothing moves on screen.
    #[test]
    fn a_listing_card_names_its_post_before_its_thumbnail() {
        let root = write_site(
            "cardorder",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\nlisting:\n  contents: posts\n  type: list\n---\n\n# Posts\n",
                ),
                (
                    "posts/p.tmd",
                    "---\ntitle: Post One\nimage: pic.png\nimage-alt: A cover\n---\n\nBody.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let (html, _) = render_page(&site, "index.tmd");
        let card = &html[html.find("class=\"tali-card\"").expect("a card")..];
        let (title, img) = (
            card.find("tali-card-title").expect("title"),
            card.find("tali-card-img").expect("thumbnail"),
        );
        assert!(
            title < img,
            "the post's title comes before the thumbnail: {card}"
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
                    "---\ntitle: Home\nlisting:\n  contents: posts\n  type: list\n---\n\n# Posts\n",
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
        // `type: list` KEEPS the `image:` thumbnail (reading-first feed); plain
        // `type: default` is the same layout WITHOUT the thumbnail (a formal text list,
        // e.g. a CV's projects). The two differ only in the image.
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
            feed.contains("class=\"tali-listing tali-listing-default\""),
            "list is the one listing layout: {feed}"
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
                    "---\ntitle: Blog\nlisting:\n  contents: posts\n  type: list\n---\n\n# Blog\n",
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

        // Needle the opening tag up to the class, not a bare class name: every page inlines
        // the whole stylesheet, which names `.tali-listing`, so a bare `contains` passes on
        // any page. The tag is left open here because a standalone listing also carries its
        // `data-block-id` (see `a_standalone_listing_block_is_targetable_by_the_op_that_…`).
        assert!(
            blog.contains("<ul role=\"list\" class=\"tali-listing tali-listing-default\""),
            "the listing container must be a <ul>: {blog}"
        );
        assert!(
            !blog.contains("<div class=\"tali-listing"),
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
    fn listing_card_titles_are_h2_so_the_outline_never_skips_a_level() {
        // A listing page's only preceding heading is its (often sr-only) page <h1>, so an
        // <h3> card title skipped a level on every listing page — and the heading-skip
        // lint structurally cannot see it: the whole listing is ONE <ul> block, and the
        // lint only reads a block that *starts* with a heading. Cards emit <h2>. Assert
        // through the tag walker: the inlined stylesheet also spells `.tali-card-title`,
        // so a substring scan is not evidence about the markup.
        let root = write_site(
            "cardheading",
            &[
                ("_site.yml", "title: Demo\n"),
                (
                    "blog.tmd",
                    "---\ntitle: Blog\ntitle-block-style: none\nlisting:\n  contents: posts\n---\n\nIntro.\n",
                ),
                (
                    "posts/a.tmd",
                    "---\ntitle: A\ndate: 2026-01-01\n---\n\nBody.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let (blog, _) = render_page(&site, "blog.tmd");
        let card_titles_at = |level: &str| {
            crate::render::tags(&blog)
                .filter(|t| t.name.eq_ignore_ascii_case(level))
                .filter(|t| {
                    crate::render::attrs(t).any(|a| {
                        a.name.eq_ignore_ascii_case("class")
                            && a.value
                                .split_ascii_whitespace()
                                .any(|c| c == "tali-card-title")
                    })
                })
                .count()
        };
        assert_eq!(
            card_titles_at("h2"),
            1,
            "the card title must be an <h2>: {blog}"
        );
        assert_eq!(
            card_titles_at("h3"),
            0,
            "no card title may remain an <h3>: {blog}"
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

    /// The per-page cross-page link check the preview runs on every save judges only the
    /// page asked about, so a broken link is reported on the page that carries it and not
    /// on the page it points at. (It lived beside the preview's bridge module until that
    /// module went, with the preview's second diagnostic type.)
    #[test]
    fn a_broken_cross_page_link_is_reported_only_on_the_linking_page() {
        let root = write_site(
            "xpage-for",
            &[
                ("_site.yml", "title: T\n"),
                (
                    "index.tmd",
                    "# Home\n\nSee [the other page](other.tmd#nope).\n",
                ),
                ("other.tmd", "# Real Heading\n\nBody.\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(
            !site.validate_cross_page_links_for("index.tmd").is_empty(),
            "index links a nonexistent anchor"
        );
        assert!(
            site.validate_cross_page_links_for("other.tmd").is_empty(),
            "other.tmd has no broken outgoing link"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A document built on its own has no sibling book beside it to link into: a link out of
    /// its folder names a file on disk, and a missing one is dead in the page the build
    /// writes. The project rule skips a link above the root (it may point into a mounted
    /// sibling), which for one document's own project would have dropped the check the
    /// single-file gate always ran.
    #[test]
    fn a_lone_document_s_link_out_of_its_folder_is_checked() {
        let root = write_site(
            "lone-climb",
            &[
                (
                    "doc/a.tmd",
                    "# A\n\n[there](../there.pdf) and [gone](../gone.pdf)\n",
                ),
                ("there.pdf", "%PDF"),
            ],
        );
        let site = Site::discover_document(&root.join("doc/a.tmd"));
        let broken: Vec<String> = site
            .validate_cross_page_links_for("a.tmd")
            .into_iter()
            .map(|w| w.message)
            .collect();
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert!(broken[0].contains("../gone.pdf"), "{broken:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A book chapter built on its own carries the number the published book gives it: a
    /// draft chapter ahead of it is not in the book, so it does not count.
    #[test]
    fn a_chapter_built_alone_carries_its_published_number() {
        let root = write_site(
            "chapter-alone",
            &[
                (
                    "_site.yml",
                    "title: B\nchapters:\n  - index.tmd\n  - a.tmd\n  - b.tmd\n",
                ),
                ("index.tmd", "# Preface\n"),
                ("a.tmd", "---\ndraft: true\n---\n\n# A\n"),
                ("b.tmd", "# B\n"),
            ],
        );
        let book = Site::discover(&root);
        let alone = Site::discover_document(&root.join("b.tmd"));
        let number = |site: &Site| site.chapter_for(site.page("b.tmd").unwrap());
        assert_eq!(number(&book), Some(1), "the published book skips the draft");
        assert_eq!(number(&alone), number(&book));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A project never contains another: `build <parent>` publishes a nested project's pages
    /// as its own, under its own chrome, and ignores the nested `_site.yml`. Saying so is the
    /// honest minimum (nested projects were cut); it was silent (config-seam #18).
    #[test]
    fn a_nested_project_s_config_is_reported() {
        let root = write_site(
            "nested",
            &[
                ("_site.yml", "title: Outer\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nHi.\n"),
                ("sub/_site.yml", "title: Inner\n"),
                ("sub/s.tmd", "---\ntitle: S\n---\n\nS.\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(
            site.warnings
                .iter()
                .any(|w| loc(w).contains("sub/_site.yml")),
            "{:?}",
            site.warnings
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A site warning as `file:line: message`, the form every verb prints it in.
    fn loc(w: &Warning) -> String {
        let file = w.file.as_deref().unwrap_or("_site.yml");
        match w.line {
            Some(l) => format!("{file}:{l}: {}", w.message),
            None => format!("{file}: {}", w.message),
        }
    }

    /// Every project diagnostic is located at the file and line that wrote it: a config key
    /// in `_site.yml`, a page's front matter in that page. They were strings with whatever
    /// location the producer baked into the text, which is how a page's `draft:` came to be
    /// reported against `_site.yml` with no line (audit 2026-09-24 NEW-A).
    #[test]
    fn site_warnings_are_located_where_they_were_written() {
        let root = write_site(
            "located",
            &[
                ("_site.yml", "title: T\ntitel: oops\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nHi.\n"),
                ("posts/p.tmd", "---\ntitle: P\ndraft: maybe\n---\n\nBody.\n"),
            ],
        );
        let site = Site::discover_with(&root, DraftMode::Include);
        let found = |needle: &str| {
            site.warnings
                .iter()
                .map(loc)
                .find(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("no warning mentions {needle}: {:?}", site.warnings))
        };
        assert!(
            found("titel").starts_with("_site.yml:2: "),
            "{}",
            found("titel")
        );
        assert!(
            found("draft").starts_with("posts/p.tmd:3: "),
            "{}",
            found("draft")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A duplicate label written below an `{{< include >}}` is reported at the line the
    /// author wrote it on. The scan counted lines of the include-expanded buffer, so it
    /// named a line past the end of a short page (leads xref.rs:72).
    #[test]
    fn a_duplicate_label_below_an_include_is_located_at_its_own_line() {
        let part: String = (1..=20).map(|i| format!("filler {i}\n\n")).collect();
        let root = write_site(
            "dup-after-include",
            &[
                ("_site.yml", "title: T\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\n## First {#sec-dup}\n",
                ),
                (
                    "posts/one/index.tmd",
                    "---\ntitle: One\n---\n\n{{< include _part.tmd >}}\n\n## Again {#sec-dup}\n",
                ),
                ("posts/one/_part.tmd", &part),
            ],
        );
        let site = Site::discover(&root);
        let dup = site
            .warnings
            .iter()
            .map(loc)
            .find(|l| l.contains("duplicate cross-reference label"))
            .unwrap_or_else(|| panic!("the duplicate is reported: {:?}", site.warnings));
        assert!(dup.starts_with("posts/one/index.tmd:7: "), "{dup}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Previewing or building one loose document reads that document and nothing else: a
    /// sibling's front matter is not parsed, so a sibling's problem is not reported against
    /// it (leads site/mod.rs:340, measured at 4000 sibling warnings for one note).
    #[test]
    fn a_single_document_project_reads_no_sibling() {
        let root = write_site(
            "no-siblings",
            &[
                ("note.tmd", "---\ntitle: Note\n---\n\nThe note.\n"),
                (
                    "other.tmd",
                    "---\ntitle: Other\ndraft: maybe\n---\n\nUnrelated.\n",
                ),
            ],
        );
        let site = Site::discover_document(&root.join("note.tmd"));
        assert!(
            site.warnings.iter().all(|w| !loc(w).contains("other.tmd")),
            "{:?}",
            site.warnings
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The warning for a listing whose `id:` matches nothing names the line that wrote the
    /// `id:`, so it is clickable like every other page diagnostic (audit 2026-09-24, WP5
    /// residual).
    #[test]
    fn a_listing_id_that_matches_nothing_is_located() {
        let src = "---\ntitle: Home\nlisting:\n  - contents: posts\n    id: nowhere\n---\n\nHi.\n";
        let root = write_site(
            "listing-id",
            &[
                ("_site.yml", "title: T\n"),
                ("index.tmd", src),
                ("posts/a.tmd", "---\ntitle: A\n---\n\nA.\n"),
            ],
        );
        let site = Site::discover(&root);
        let page = site.page("index.tmd").unwrap().clone();
        let mut doc = render::render_document_scoped_with_site(
            src,
            &root,
            None,
            Some(&site.render_defaults()),
        );
        let mut warnings = Vec::new();
        site.finish_blocks(&page, &mut doc.blocks, &mut warnings, Some(src), None);
        let w = warnings
            .iter()
            .find(|w| w.message.contains("id: nowhere"))
            .unwrap_or_else(|| panic!("the listing id is reported: {warnings:?}"));
        assert_eq!(w.line, Some(5), "{w:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A chrome link carrying a `?query` is resolved without it, as a body link is: a nav
    /// entry to a draft page is dead in the deploy however it is spelled. With the query
    /// kept, the `.tmd` source on disk passed it as a raw file (audit 2026-09-24, WP9
    /// residual).
    #[test]
    fn a_chrome_link_with_a_query_is_judged_by_its_page() {
        let root = write_site(
            "chrome-query",
            &[
                (
                    "_site.yml",
                    "title: T\nnav:\n  - { text: Wip, href: \"wip.tmd?v=1\" }\n",
                ),
                ("index.tmd", "---\ntitle: Home\n---\n\nHi.\n"),
                ("wip.tmd", "---\ntitle: Wip\ndraft: true\n---\n\nNot yet.\n"),
            ],
        );
        let site = Site::discover(&root);
        let broken = site.validate_chrome_links();
        assert!(
            broken.iter().any(|w| w.message.contains("wip.tmd?v=1")),
            "{broken:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
