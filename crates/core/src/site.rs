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

use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::render::{self, Block, SiteCtx};

/// The root project config, parsed from `_quarto.yml`. Only the subset qmd-fast
/// understands is modelled; unknown keys are ignored (Quarto compatibility —
/// a real config carries far more than this foundation consumes).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SiteConfig {
    #[serde(default)]
    pub project: ProjectSection,
    #[serde(default)]
    pub website: WebsiteSection,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectSection {
    /// Where `build` writes the site (default `_site`).
    #[serde(default, rename = "output-dir")]
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WebsiteSection {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "site-url")]
    pub site_url: Option<String>,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub navbar: Navbar,
    #[serde(default, rename = "page-footer")]
    pub page_footer: Option<Footer>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Navbar {
    #[serde(default)]
    pub left: Vec<NavItem>,
    #[serde(default)]
    pub right: Vec<NavItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Footer {
    #[serde(default)]
    pub left: Vec<NavItem>,
    #[serde(default)]
    pub center: Vec<NavItem>,
    #[serde(default)]
    pub right: Vec<NavItem>,
}

/// A navbar/footer entry. `text` is the label (plain text in the navbar, allowed
/// raw HTML in the footer for icon SVGs); `href` is its link.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NavItem {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub href: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
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
}

/// A discovered multi-page site: the root config plus its input pages.
#[derive(Debug, Clone)]
pub struct Site {
    pub root: PathBuf,
    pub config: SiteConfig,
    pub pages: Vec<Page>,
    /// Warnings gathered during discovery (bad config, etc.), surfaced by the
    /// caller (build logs / preview diagnostics).
    pub warnings: Vec<String>,
}

impl Site {
    /// Discover the site rooted at `root`: parse `_quarto.yml`, enumerate input
    /// `.qmd` pages, and compute their output URLs + ordering.
    pub fn discover(root: &Path) -> Site {
        let mut warnings = Vec::new();
        let config = load_config(root, &mut warnings);

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

        Site {
            root: root.to_path_buf(),
            config,
            pages,
            warnings,
        }
    }

    /// The output directory `build` writes to (default `_site`).
    pub fn output_dir(&self) -> &str {
        self.config.project.output_dir.as_deref().unwrap_or("_site")
    }

    /// Look up a page by its source rel-path or its output URL (`serve` accepts
    /// either an editor path or a browser request).
    pub fn page(&self, rel_or_url: &str) -> Option<&Page> {
        let needle = rel_or_url.trim_start_matches('/');
        self.pages
            .iter()
            .find(|p| p.rel == needle || p.url == needle)
    }

    /// Posts ordered oldest→newest (by `date`, falling back to rel path), the
    /// order used for prev/next navigation.
    fn posts_chronological(&self) -> Vec<&Page> {
        let mut posts: Vec<&Page> = self.pages.iter().filter(|p| p.is_post).collect();
        posts.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.rel.cmp(&b.rel)));
        posts
    }

    /// Build the chrome (navbar, footer, prev/next) for a page, with links
    /// resolved relative to that page's depth. Shared by the static build and the
    /// live preview so both render identical navigation.
    pub fn page_chrome(&self, page: &Page) -> SiteCtx {
        let depth = page.url.matches('/').count(); // links are relative to the page
        SiteCtx {
            navbar_html: self.navbar_html(page, depth),
            footer_html: self.footer_html(depth),
            prevnext_html: self.prevnext_html(page, depth),
            wide: page.page_layout.as_deref() == Some("full"),
        }
    }

    /// Render a single page (by rel-path or URL) into a full HTML document with
    /// the site chrome (navbar, footer, prev/next) and intra-site links rewritten
    /// to their `.html` targets. Returns `None` if the page isn't part of the site.
    pub fn render_page(&self, rel_or_url: &str) -> Option<String> {
        let page = self.page(rel_or_url)?;
        let src = std::fs::read_to_string(&page.input).ok()?;
        let base = page.input.parent().unwrap_or(&self.root);
        let mut doc = render::render_document_with_includes(&src, base);
        self.expand_page(page, &mut doc.blocks);
        let ctx = self.page_chrome(page);
        let fallback = page.title.as_deref().unwrap_or("");
        let html = render::html_page_from_doc_in_site(&doc, fallback, &ctx);
        Some(rewrite_qmd_links(&html))
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
        let mut s = format!("<div class=\"qmd-listing qmd-listing-{layout}\">");
        for p in self.collection(host, spec) {
            s.push_str(&self.card_html(p, &up, spec.grid));
        }
        s.push_str("</div>");
        s
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
        let cats = if p.categories.is_empty() {
            String::new()
        } else {
            let badges: String = p
                .categories
                .iter()
                .map(|c| format!("<span class=\"qmd-cat\">{}</span>", esc(c)))
                .collect();
            format!("<div class=\"qmd-card-cats\">{badges}</div>")
        };
        format!(
            "<a class=\"qmd-card\" href=\"{href}\">{img}\
             <div class=\"qmd-card-body\">{date}<h3 class=\"qmd-card-title\">{title}</h3>{desc}{cats}</div></a>"
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
            "<header class=\"qmd-about qmd-about-{tpl}\" data-block-id=\"qmd-title-block\">\
             {img}<h1 class=\"qmd-about-name\">{name}</h1>{links}</header>",
            tpl = esc(&about.template),
            name = esc(&name),
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
            .website
            .title
            .clone()
            .unwrap_or_else(|| "Home".to_string());
        let mut s = String::from("<header class=\"qmd-site-nav\"><nav class=\"qmd-nav-inner\">");
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
        for it in &self.config.website.navbar.left {
            s.push_str(&self.nav_link(it, current, &up));
        }
        // Everything after the spacer is pushed to the far right of the bar.
        s.push_str("<span class=\"qmd-nav-spacer\"></span>");
        for it in &self.config.website.navbar.right {
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
        format!("<a{cls} href=\"{}\">{}</a>", target, esc(label))
    }

    /// The slim site footer. Footer item text is treated as raw HTML (icon SVGs),
    /// per the trusted-source model. The RSS/feed link is dropped (unused).
    fn footer_html(&self, depth: usize) -> String {
        let Some(footer) = &self.config.website.page_footer else {
            return String::new();
        };
        let up = "../".repeat(depth);
        let group = |items: &[NavItem]| -> String {
            let mut g = String::new();
            for it in items {
                let text = it.text.clone().unwrap_or_default();
                match it.href.as_deref() {
                    // Drop the RSS/Atom feed link — deliberately unsupported.
                    Some(h) if h.ends_with(".xml") => continue,
                    Some(h) => {
                        g.push_str(&format!(
                            "<a class=\"qmd-foot-item\" href=\"{}\">{text}</a>",
                            resolve_href(h, &up)
                        ));
                    }
                    None => g.push_str(&format!("<span class=\"qmd-foot-item\">{text}</span>")),
                }
            }
            g
        };
        format!(
            "<footer class=\"qmd-site-footer\"><div class=\"qmd-foot-inner\">\
             <div class=\"qmd-foot-left\">{}</div>\
             <div class=\"qmd-foot-center\">{}</div>\
             <div class=\"qmd-foot-right\">{}</div>\
             </div></footer>",
            group(&footer.left),
            group(&footer.center),
            group(&footer.right),
        )
    }

    /// Prev/next navigation between posts (chronological). Non-posts get nothing.
    fn prevnext_html(&self, current: &Page, depth: usize) -> String {
        if !current.is_post {
            return String::new();
        }
        let posts = self.posts_chronological();
        let Some(i) = posts.iter().position(|p| p.rel == current.rel) else {
            return String::new();
        };
        let up = "../".repeat(depth);
        let link = |p: &Page, dir: &str, glyph: &str| -> String {
            let label = p.title.as_deref().unwrap_or(&p.rel);
            let target = format!("{up}{}", p.url);
            format!(
                "<a class=\"qmd-prevnext-link qmd-pn-{dir}\" href=\"{target}\">\
                 <span class=\"qmd-pn-dir\">{glyph}</span>\
                 <span class=\"qmd-pn-title\">{}</span></a>",
                esc(label)
            )
        };
        let mut s = String::from("<nav class=\"qmd-prevnext\">");
        if i > 0 {
            s.push_str(&link(posts[i - 1], "prev", "← Previous"));
        } else {
            s.push_str("<span></span>");
        }
        if i + 1 < posts.len() {
            s.push_str(&link(posts[i + 1], "next", "Next →"));
        } else {
            s.push_str("<span></span>");
        }
        s.push_str("</nav>");
        s
    }
}

/// Load + parse `_quarto.yml` at `root`, tolerating malformed sections (warn,
/// don't reject — Quarto configs carry keys/shapes we don't model).
fn load_config(root: &Path, warnings: &mut Vec<String>) -> SiteConfig {
    let path = root.join("_quarto.yml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        warnings.push(format!("no _quarto.yml at {}", root.display()));
        return SiteConfig::default();
    };
    // Parse to a generic value first, then deserialize each known section on its
    // own, so one unfamiliar section can't sink the whole config.
    let root_val: serde_yaml::Value = match serde_yaml::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!("_quarto.yml is not valid YAML: {e}"));
            return SiteConfig::default();
        }
    };
    let mut cfg = SiteConfig::default();
    if let Some(v) = root_val.get("project").cloned() {
        match serde_yaml::from_value(v) {
            Ok(p) => cfg.project = p,
            Err(e) => warnings.push(format!("ignoring malformed `project` config: {e}")),
        }
    }
    if let Some(v) = root_val.get("website").cloned() {
        match serde_yaml::from_value(v) {
            Ok(w) => cfg.website = w,
            Err(e) => warnings.push(format!("ignoring malformed `website` config: {e}")),
        }
    }
    cfg
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

/// Minimal HTML-text escape for nav labels (the project-wide `html_escape` in §6
/// will subsume this; kept local for now).
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
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
}
