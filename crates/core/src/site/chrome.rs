//! Site chrome: the navbar, footer, bottom-of-post nav, and book sidebar +
//! within-chapter nav HTML, plus the bundled social-icon glyphs. Split out of
//! mod.rs; `page_chrome()` there calls these per page (`use super::*` reaches
//! Page/NavItem + the link helpers).

use super::*;

/// Magnifier glyph for the search control (single-quoted attrs so it embeds in a
/// double-quoted Rust string; `currentColor` so it inherits the control's colour).
const SEARCH_ICON: &str = "<svg width='15' height='15' viewBox='0 0 16 16' fill='none' stroke='currentColor' stroke-width='1.6' aria-hidden='true'><circle cx='7' cy='7' r='4.5'/><path d='M10.5 10.5 14 14' stroke-linecap='round'/></svg>";

/// Tiny idempotent script that makes the mobile burger keyboard- and
/// screen-reader-operable: it toggles the button's `aria-expanded` and a
/// `.tali-nav-open` class on the `.tali-nav-links` menu (the CSS reveals the menu on
/// that class instead of the old `:checked` selector), and closes the menu on
/// Escape or when a nav link is followed. The `data-nav-wired` guard makes it safe
/// to re-run when the live preview re-injects the navbar on hot reload.
const NAV_TOGGLE_SCRIPT: &str = "<script>(function(){var b=document.getElementById('tali-nav-toggle'),m=document.getElementById('tali-nav-links');if(!b||!m||b.dataset.navWired)return;b.dataset.navWired='1';function set(o){b.setAttribute('aria-expanded',o?'true':'false');m.classList.toggle('tali-nav-open',o);}b.addEventListener('click',function(){set(b.getAttribute('aria-expanded')!=='true');});m.addEventListener('click',function(e){if(e.target.closest('a'))set(false);});document.addEventListener('keydown',function(e){if(e.key==='Escape'&&b.getAttribute('aria-expanded')==='true'){set(false);b.focus();}});})();</script>";

/// Same shape as [`NAV_TOGGLE_SCRIPT`], for the BOOK chapter drawer. A book is laid out
/// as one centred reading column (the same measure as a blog post); the chapter list is
/// not a permanent rail but an off-canvas drawer summoned from the topbar's "Chapters"
/// button at every width. This wires that button: toggle `aria-expanded` + reveal the
/// `#tali-book-drawer` overlay (which starts `hidden`), move focus into it on open, and
/// close it on Escape, on a backdrop / close-button click (`[data-tali-drawer-close]`), or
/// after a chapter link is followed (restoring focus to the opener). `data-drawer-wired`
/// keeps it idempotent across hot-reload re-injects.
///
/// It also **locks page scroll** while open (MOB-5). On a phone the panel covers 93% of the
/// viewport over a backdrop, and without the lock a swipe meant for the chapter list scrolled
/// the article underneath it (measured: 328px), so dismissing the drawer returned the reader
/// somewhere they never chose. `overflow: hidden` on the root element is the lock; the panel's
/// own `overscroll-behavior: contain` (site.css) stops a scroll *inside* the list from
/// chaining out at either end. Restoring to `''` hands the value back to the stylesheet
/// rather than freezing whatever was inline.
///
/// *Not verified on real WebKit* — iOS Safari is known to honour a root `overflow: hidden`
/// less completely than Chromium, and the 2026-07-26 round was Chromium emulation only.
const BOOK_DRAWER_SCRIPT: &str = "<script>(function(){var b=document.getElementById('tali-book-drawer-btn'),d=document.getElementById('tali-book-drawer');if(!b||!d||b.dataset.drawerWired)return;b.dataset.drawerWired='1';var panel=d.querySelector('.tali-book-drawer-panel')||d,release=null;function set(o){d.hidden=!o;b.setAttribute('aria-expanded',o?'true':'false');document.documentElement.style.overflow=o?'hidden':'';if(o){var f=d.querySelector('.tali-book-chapter[aria-current]')||d.querySelector('.tali-book-chapter,a,button');if(window.taliFocusTrap){release=window.taliFocusTrap(panel,f);}else if(f){f.focus();}}else if(release){release();release=null;}else{b.focus();}}b.addEventListener('click',function(){set(d.hidden);});d.addEventListener('click',function(e){if(e.target.closest('[data-tali-drawer-close]')||e.target.closest('a'))set(false);});document.addEventListener('keydown',function(e){if(e.key==='Escape'&&!d.hidden)set(false);});})();</script>";

/// A search control that opens the Cmd-K palette. It carries `data-tali-search`,
/// which `web-client/search.js` wires (by click delegation) to open the same
/// palette the keyboard shortcut does. Rendered in the navbar (websites) and the
/// book topbar.
fn search_button() -> String {
    // The kbd is a shortcut hint, not part of the label: aria-hidden keeps it out of the
    // accessible name (WCAG 2.5.3 Label-in-Name). The icon-only button names itself with
    // aria-label.
    format!(
        "<button class='tali-search-btn' type='button' data-tali-search aria-label='Search' \
         aria-keyshortcuts='Control+K Meta+K'>{SEARCH_ICON}\
         <kbd class='tali-search-kbd' aria-hidden='true'>\u{2318}K</kbd></button>"
    )
}

/// Resolve a `_site.yml` asset path (`logo:` and `favicon:`) for a page whose depth prefix
/// is `up`. A project-relative path gets the `../` climb back to the site root; a
/// site-absolute (`/brand.svg`) or external (`https://…`, `//cdn/…`) source is left exactly
/// as written, since prefixing those produces a path that resolves nowhere. Never
/// `.tmd`→`.html` rewritten: this is an image, not a page link, so `resolve_href` is the
/// wrong helper.
///
/// `favicon:` shipped before this guard existed and prefixed unconditionally, so
/// `favicon: /brand.svg` on a nested page emitted `..//brand.svg` and 404'd — which is the
/// one case an author writes a site-absolute path *for*. Both keys name a project asset,
/// so both go through here.
pub(super) fn site_asset_href(src: &str, up: &str) -> String {
    if src.starts_with('/') || src.starts_with("//") || src.contains("://") {
        src.to_string()
    } else {
        format!("{up}{src}")
    }
}

impl Site {
    /// The content of a brand link (`.tali-nav-brand` on a website, `.tali-book-brand` in
    /// both book slots): the configured `logo:` as an `<img>`, else the escaped brand text.
    ///
    /// **One image slot, no knobs.** The logo *replaces* the wordmark rather than sitting
    /// beside it — a logo file almost always already carries the name, and "both" would
    /// immediately need a second key to turn the text off. Size and position are the
    /// stylesheet's job (`.tali-brand-logo` caps the height against the bar), so a branded
    /// project is one `logo:` line and nothing else.
    ///
    /// `text` names the link either way: it is the visible label without a logo, and the
    /// image's `alt` with one, so the link keeps an accessible name. A blank/absent project
    /// title falls back to `Home` (what the website brand already prints) rather than
    /// shipping `alt=""` on an image that *is* the link.
    fn brand_content(&self, text: &str, up: &str) -> String {
        let Some(src) = self
            .config
            .logo
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            return esc(text);
        };
        let label = match text.trim() {
            "" => "Home",
            t => t,
        };
        format!(
            "<img class=\"tali-brand-logo\" src=\"{}\" alt=\"{}\" />",
            esc(&site_asset_href(src, up)),
            esc(label)
        )
    }

    /// The site navbar: a brand (site title → home) plus the configured left/right
    /// item groups. `depth` is the current page's path depth so links resolve
    /// relative to it (a post two levels deep prefixes `../../`).
    pub(super) fn navbar_html(&self, current: &Page, depth: usize) -> String {
        let up = "../".repeat(depth);
        let brand_text = self
            .config
            .title
            .clone()
            .unwrap_or_else(|| "Home".to_string());
        let mut s = String::from(
            "<header class=\"tali-site-nav\" data-tali-src=\"_site.yml\"><nav class=\"tali-nav-inner\" aria-label=\"Primary\">",
        );
        s.push_str(&format!(
            "<a class=\"tali-nav-brand\" href=\"{up}{}\">{}</a>",
            self.site_home_url(),
            self.brand_content(&brand_text, &up)
        ));
        // A real, focusable button toggles the mobile menu, so keyboard and
        // screen-reader users can open it (the old display:none checkbox + an
        // unfocusable, role-less label was a WCAG 2.1.1 failure). `aria-expanded`
        // reflects open/closed; `aria-controls` points at the menu it reveals. The
        // tiny inline script below wires the click + Escape-to-close; CSS hides the
        // button above 640px so the desktop bar is unchanged.
        s.push_str(
            "<button type=\"button\" class=\"tali-nav-burger\" id=\"tali-nav-toggle\" \
             aria-label=\"Menu\" aria-expanded=\"false\" aria-controls=\"tali-nav-links\">\
             <span></span><span></span><span></span></button>",
        );
        s.push_str("<div class=\"tali-nav-links\" id=\"tali-nav-links\">");
        for it in &self.config.nav.left {
            s.push_str(&self.nav_link(it, current, &up));
        }
        // Everything after the spacer is pushed to the far right of the bar.
        s.push_str("<span class=\"tali-nav-spacer\"></span>");
        for it in &self.config.nav.right {
            s.push_str(&self.nav_link(it, current, &up));
        }
        // A visible search control (opens the Cmd-K palette); search + social links collapse
        // into the burger menu on mobile. Dev-only tools live in the floating dev menu.
        s.push_str(&search_button());
        s.push_str("</div>");
        s.push_str("</nav></header>");
        // Wire the burger button: toggle `aria-expanded` + a `.tali-nav-open` class
        // the CSS shows the menu on, and close on Escape / link click. Idempotent
        // (a `dataset.navWired` guard) so re-running it (live hot-reload re-injects the
        // navbar) never double-binds. Inlined here, in the navbar's own HTML, to
        // keep this fix inside the two owned files (no new asset, no other JS).
        s.push_str(NAV_TOGGLE_SCRIPT);
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
        // `icon:` shorthand renders a bundled SVG; otherwise the (escaped) label.
        let icon = it.icon.as_deref().and_then(social_icon);
        let mut classes = String::from("tali-nav-link");
        if icon.is_some() {
            classes.push_str(" tali-nav-icon");
        }
        if active {
            classes.push_str(" tali-nav-active");
        }
        let aria = if active { " aria-current=\"page\"" } else { "" };
        // `data-label` carries the text so the CSS can reserve the bold (active)
        // width, keeping the navbar from shifting when the active item bolds. An
        // icon link has no text to bold (and for `{ icon, href }` the `label` is the
        // URL), so it gets an empty `data-label` (no width reservation) + an
        // accessible name from the icon name.
        let (data_label, name_attr) = match &icon {
            Some(_) => (
                String::new(),
                format!(
                    " aria-label=\"{}\"",
                    esc(it.icon.as_deref().unwrap_or("link"))
                ),
            ),
            None => (esc(label), String::new()),
        };
        let content = icon.unwrap_or_else(|| esc(label));
        format!(
            "<a class=\"{classes}\"{aria}{name_attr} href=\"{}\" data-label=\"{}\">{}</a>",
            target, data_label, content
        )
    }

    /// Resolve every `nav:` / `footer:` `href` in `_site.yml` against the page registry,
    /// the way [`Site::validate_cross_page_links`] resolves the links in a page body.
    ///
    /// **This is the highest-leverage broken link a project can have and it was the only
    /// class no validator saw**: cross-page validation harvests links out of rendered page
    /// *bodies* (`page_link_facts_from_src`), and chrome hrefs never pass through a page
    /// body — `chrome.rs` emits them straight from the config, onto every page. A single
    /// typo shipped site-wide with `--check-only --strict` green.
    ///
    /// The two collection sites are [`Self::nav_link`] and [`Self::footer_html`]'s group
    /// closure, and this walks the same items they render, so it judges what actually
    /// ships: an item with no `href` is skipped (nav drops it, the footer renders a
    /// `<span>`), and a local `.xml` in the FOOTER is skipped when no feed is generated
    /// because the footer drops that too. The navbar has no such drop rule, which is why a
    /// feed link there is reported when `url:` is unset — nothing else would have said so.
    ///
    /// Located to `_site.yml`, which is the file the author has to edit.
    pub fn validate_chrome_links(&self) -> Vec<Warning> {
        let published: std::collections::HashSet<&str> =
            self.pages.iter().map(|p| p.url.as_str()).collect();
        // Feeds are generated, not pages. `feed_hosts` is the same source `atom_feeds`
        // writes from, so this set is exactly what the build emits (empty without `url:`).
        let feeds: std::collections::HashSet<String> = self
            .feed_hosts()
            .into_iter()
            .map(|(_, path, _)| path)
            .collect();

        let nav = self.config.nav.left.iter().chain(&self.config.nav.right);
        let footer = self
            .config
            .footer
            .iter()
            .flat_map(|f| f.left.iter().chain(&f.center).chain(&f.right));

        let mut out = Vec::new();
        for (item, in_footer) in nav.map(|i| (i, false)).chain(footer.map(|i| (i, true))) {
            // No href: nav_link returns "" and footer_html renders a <span>. Nothing links.
            let Some(href) = item.href.as_deref() else {
                continue;
            };
            // Exactly what `resolve_href` passes through untouched — not this site's to
            // resolve, and never fetched (a network probe would make the gate
            // nondeterministic).
            if href.starts_with('#')
                || href.starts_with("//")
                || href.contains("://")
                || href.starts_with("mailto:")
                || href.starts_with("tel:")
            {
                continue;
            }
            let path = href.split('#').next().unwrap_or(href);
            if path.is_empty() {
                continue;
            }
            // A site-absolute `/about.tmd` and a relative `about.tmd` name the same page:
            // chrome hrefs are written from the site root, and `resolve_href` supplies each
            // page's own `../` climb.
            let rooted = path.strip_prefix('/').unwrap_or(path);
            let Some(target) = self.link_target_url("index.html", rooted) else {
                continue; // climbs above the root; unresolvable offline, as for body links
            };
            if published.contains(target.as_str()) || feeds.contains(&target) {
                continue;
            }
            // The footer drops a local `.xml` when no feed is generated, so there is no
            // link on the page to be broken.
            if in_footer && target.ends_with(".xml") {
                continue;
            }
            // A raw file on disk: the same judgement the body-link resolver makes.
            if self.root.join(&target).is_file() {
                continue;
            }
            let where_ = if in_footer { "footer" } else { "nav" };
            out.push(
                Warning::new(format!(
                    "broken {where_} link: `{href}` resolves to `{target}`, which is no page \
                     in this site — and `_site.yml` chrome ships on every page"
                ))
                .severity(Severity::Error)
                .at(Some("_site.yml".to_string()), 1),
            );
        }
        out
    }

    /// The slim site footer. Footer item text is treated as raw HTML (icon SVGs),
    /// per the trusted-source model. A configured local `.xml` link is dropped
    /// (this build generates no RSS feed).
    pub(super) fn footer_html(&self, depth: usize) -> String {
        let Some(footer) = &self.config.footer else {
            return String::new();
        };
        let up = "../".repeat(depth);
        // With `url:` set, the build generates the feed (`blog.xml` etc.), so a local
        // `.xml` footer link is honest and kept; without it, no feed exists so it is dropped.
        let url_set = self.config.url.is_some();
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
                    // A configured *local* `.xml` link (e.g. `/blog.xml`) is dropped ONLY
                    // when no feed is generated — i.e. `url:` is unset. With `url:` set the
                    // build emits the feed, so the link is honest and kept. An external
                    // `.xml` URL (http/protocol-relative) is left alone — it's some other
                    // resource, not this site's feed.
                    Some(h)
                        if h.ends_with(".xml")
                            && !url_set
                            && !(h.starts_with("http://")
                                || h.starts_with("https://")
                                || h.starts_with("//")) =>
                    {
                        continue;
                    }
                    Some(h) => {
                        g.push_str(&format!(
                            "<a class=\"tali-foot-item\"{aria} href=\"{}\">{content}</a>",
                            resolve_href(h, &up)
                        ));
                    }
                    None => g.push_str(&format!("<span class=\"tali-foot-item\">{content}</span>")),
                }
            }
            g
        };
        format!(
            "<footer class=\"tali-site-footer\" data-tali-src=\"_site.yml\"><div class=\"tali-foot-inner\">\
             <div class=\"tali-foot-left\">{}</div>\
             <div class=\"tali-foot-center\">{}</div>\
             <div class=\"tali-foot-right\">{}</div>\
             </div></footer>",
            group(&footer.left),
            group(&footer.center),
            group(&footer.right),
        )
    }
}

impl Site {
    /// The book's brand link, for both slots that carry one (the sticky topbar and the
    /// drawer's head). Empty when the book is unbranded — no `title:` and no `logo:` —
    /// because a bare "Home" wordmark where a book's name belongs is noise, unlike a
    /// website navbar, which is a row of links that needs an anchored left edge.
    ///
    /// **A `logo:` alone is enough.** The gate used to be `title:` only, which predates
    /// `logo:`: a book that configured a logo and no title emitted no brand at all, though
    /// the logo *is* the brand and a missing title costs only the accessible label — which
    /// `brand_content` already falls back to "Home" for. One helper rather than two copies,
    /// since the slot is emitted twice and fixing one is exactly the shape of the bug
    /// `book_brand_renders_the_logo_in_both_the_topbar_and_the_drawer_head` pins against.
    fn book_brand_html(&self, title: Option<&String>, up: &str) -> String {
        let has_logo = self
            .config
            .logo
            .as_deref()
            .is_some_and(|l| !l.trim().is_empty());
        if title.is_none() && !has_logo {
            return String::new();
        }
        format!(
            "<a class=\"tali-book-brand\" href=\"{up}{}\">{}</a>",
            self.book_home_url(),
            self.brand_content(title.map(String::as_str).unwrap_or(""), up)
        )
    }

    /// Where the site brand points: the website's home page.
    ///
    /// The website twin of [`Self::book_home_url`], and the same hazard. A directory-walked
    /// website has no required entry file — `corpus/debug/` is four pages named
    /// `sorting`/`leetcode`/`dp`/`custom-view` and nothing else — so a hardcoded
    /// `index.html` was a dead brand link on every page of it. Prefer a real `index.html`
    /// (every other site in the repo has one, and keeps exactly its old behaviour),
    /// otherwise fall back to the first page in the site's own order.
    fn site_home_url(&self) -> String {
        if self.pages.iter().any(|p| p.url == "index.html") {
            return "index.html".to_string();
        }
        self.pages
            .first()
            .map(|p| p.url.clone())
            .unwrap_or_else(|| "index.html".to_string())
    }

    /// Where the book brand points: the book's home page.
    ///
    /// **Not always `index.html`.** `chapters:` is an ordered list of files the author
    /// names, and nothing requires the first to be `index.tmd`; a book that starts with
    /// `alpha.tmd` builds no `index.html` at all, so a hardcoded `index.html` was a dead
    /// link in both brand slots on every page of it (measured on `corpus/theorem-book/`).
    /// Prefer a real index chapter when one exists — that keeps every book that already
    /// starts with `index.tmd` exactly as it was — and otherwise fall back to the first
    /// chapter, which IS the book's front door when there is no index.
    fn book_home_url(&self) -> String {
        let Some(book) = &self.book else {
            return "index.html".to_string();
        };
        let chapters = book.chapters();
        if chapters.iter().any(|c| c.url == "index.html") {
            return "index.html".to_string();
        }
        chapters
            .first()
            .map(|c| c.url.clone())
            .unwrap_or_else(|| "index.html".to_string())
    }

    /// The book chrome: a slim sticky topbar (a "Chapters" drawer launcher, the title
    /// linking home, a search button, and the light/dark toggle) followed by the chapter
    /// list inside an off-canvas drawer. A book reads as one centred column, so the chapter
    /// list is summoned, not a permanent rail. (Returned together from one method because
    /// the page assembler threads a single `book_sidebar` string; the topbar is `.tali-book-
    /// topbar`, never the website `.tali-site-nav`.)
    pub(super) fn sidebar_html(&self, current: &Page, depth: usize) -> String {
        let Some(book) = &self.book else {
            return String::new();
        };
        let up = "../".repeat(depth);
        let mut s = String::new();
        // --- slim sticky topbar: Chapters launcher · brand · search · Settings gear ---
        s.push_str(
            "<header class=\"tali-book-topbar\" data-tali-src=\"_site.yml\">\
             <div class=\"tali-book-topbar-inner\">",
        );
        s.push_str(
            "<button type=\"button\" class=\"tali-book-drawer-btn\" id=\"tali-book-drawer-btn\" \
             aria-label=\"Chapters\" aria-haspopup=\"dialog\" aria-expanded=\"false\" \
             aria-controls=\"tali-book-drawer\">\
             <svg width='16' height='16' viewBox='0 0 16 16' fill='none' stroke='currentColor' \
             stroke-width='1.6' stroke-linecap='round' aria-hidden='true'>\
             <path d='M2 4h12M2 8h12M2 12h12'/></svg><span>Chapters</span></button>",
        );
        s.push_str(&self.book_brand_html(book.title.as_ref(), &up));
        s.push_str("<span class=\"tali-nav-spacer\"></span>");
        // A search button, opening the same Cmd-K palette. The light/dark toggle that used
        // to sit here became the reader Settings gear, and the gear was removed on
        // 2026-08-13: a page follows the reader's device and offers no override.
        s.push_str(&search_button());
        s.push_str("</div></header>");
        // --- the chapter drawer: an off-canvas overlay summoned from the topbar ---
        s.push_str(
            "<div class=\"tali-book-drawer\" id=\"tali-book-drawer\" hidden>\
             <div class=\"tali-book-drawer-backdrop\" data-tali-drawer-close></div>\
             <div class=\"tali-book-drawer-panel\" role=\"dialog\" aria-label=\"Chapters\">",
        );
        // The `tali-book-sidebar` nav (kept for the chapter list + its aria-label) now lives
        // inside the drawer panel rather than a left rail.
        s.push_str(
            "<nav class=\"tali-book-sidebar\" data-tali-src=\"_site.yml\" \
             aria-label=\"Chapters\">",
        );
        s.push_str("<div class=\"tali-book-sidebar-head\">");
        s.push_str(&self.book_brand_html(book.title.as_ref(), &up));
        s.push_str(
            "<button type=\"button\" class=\"tali-book-drawer-close\" data-tali-drawer-close \
             aria-label=\"Close chapters\">\u{2715}</button>",
        );
        s.push_str("</div>");
        s.push_str("<ul class=\"tali-book-chapters\" id=\"tali-book-chapters\">");
        for e in &book.entries {
            if let Some(part) = &e.part {
                // A nested part is indented rather than flattened into its parent, so the
                // drawer shows the structure the author actually declared.
                let nested = if e.depth > 0 {
                    " tali-book-part-nested"
                } else {
                    ""
                };
                s.push_str(&format!(
                    "<li class=\"tali-book-part{nested}\">{}</li>",
                    esc(part)
                ));
                continue;
            }
            let active = e.rel == current.rel;
            let cls = if active {
                "tali-book-chapter tali-book-active"
            } else {
                "tali-book-chapter"
            };
            let aria = if active { " aria-current=\"page\"" } else { "" };
            let num = e
                .number
                .map(|n| format!("<span class=\"tali-chap-num\">{n}</span> "))
                .unwrap_or_default();
            // A draft chapter (preview only — a built book never contains one) is marked
            // in the drawer so it reads as unpublished.
            let draft_tag = if e.draft {
                " <span class=\"tali-draft-badge\">draft</span>"
            } else {
                ""
            };
            // The per-chapter word count went on 2026-08-15 with spec §9's cut #12, and the
            // whole chain went with it: the label, the `Chapter::words` field, and the
            // include-expanding `word_count` pass this ran over every chapter at discovery.
            s.push_str(&format!(
                "<li><a class=\"{cls}\" href=\"{up}{}\"{aria}>{num}{}{draft_tag}</a></li>",
                e.url,
                esc(&e.title)
            ));
        }
        s.push_str("</ul>");
        s.push_str("</nav></div></div>");
        // Wire the Chapters drawer (keyboard + SR operable; idempotent on hot reload).
        s.push_str(BOOK_DRAWER_SCRIPT);
        s
    }

    /// Bottom-of-chapter prev/next navigation between book chapters.
    pub(super) fn book_nav_html(&self, current: &Page, depth: usize) -> String {
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
                    "<a class=\"tali-book-prev\" href=\"{up}{}\">\
                     <span class=\"tali-back-glyph\">\u{2190}</span> {}</a>",
                    p.url,
                    label(p)
                )
            })
            .unwrap_or_default();
        let right = next
            .map(|n| {
                format!(
                    "<a class=\"tali-book-next\" href=\"{up}{}\">{} \
                     <span class=\"tali-fwd-glyph\">\u{2192}</span></a>",
                    n.url,
                    label(n)
                )
            })
            .unwrap_or_default();
        format!(
            "<nav class=\"tali-postnav tali-book-postnav\" aria-label=\"Pagination\">{left}\
             <span class=\"tali-nav-spacer\"></span>{right}</nav>"
        )
    }

    /// Bottom-of-post "back to listing" link on a website page: returns to the single
    /// listing page this page belongs to (e.g. "← Blog"), or empty when it belongs to
    /// none or is ambiguously covered by several. Non-book pages only — books fill the
    /// same slot with [`book_nav_html`](Self::book_nav_html).
    pub(super) fn listing_backlink_html(&self, page: &Page, depth: usize) -> String {
        let Some(owner) = self.listing_owner(page) else {
            return String::new();
        };
        let up = "../".repeat(depth);
        // The arrow is decorative (the `<nav>` label carries the direction), so it is
        // hidden from the accessibility tree; a screen reader reads just the title.
        format!(
            "<nav class=\"tali-postnav tali-listing-backnav\" aria-label=\"Back to listing\">\
             <a class=\"tali-back-link\" href=\"{up}{}\">\
             <span class=\"tali-back-glyph\" aria-hidden=\"true\">\u{2190}</span> {}</a></nav>",
            owner.url,
            esc(owner.title.as_deref().unwrap_or_default())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::{Site, tests::write_site};

    #[test]
    fn footer_honors_local_xml_feed_link_when_url_set() {
        let root = write_site(
            "footerfeed",
            &[
                (
                    "_site.yml",
                    "title: Blog\nurl: https://ex.com\nfooter:\n  right:\n    - { icon: rss, href: blog.xml }\n",
                ),
                ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        assert!(
            html.contains("href=\"blog.xml\""),
            "feed link honored with url: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn footer_still_drops_local_xml_without_url() {
        let root = write_site(
            "footerfeednourl",
            &[
                (
                    "_site.yml",
                    "title: Blog\nfooter:\n  right:\n    - { icon: rss, href: blog.xml }\n",
                ),
                ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        assert!(
            !html.contains("blog.xml"),
            "no feed generated → link dropped: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    // --- `logo:` — the brand image, on all three brand slots (item 74) -----------------
    //
    // Decks have carried a front-matter `logo:` since the deck-chrome overlay
    // (`render::deck::deck_overlay_html`); the website navbar and the book topbar/drawer
    // were text-only, so a branded book or site was impossible. The same key name now
    // reaches all three, and the brand link is the only place it lands: one image slot,
    // no size/position sub-keys (`.tali-brand-logo` in site.css owns the sizing).
    //
    // Needle the WHOLE `<a …><img …></a>` construct in these, never a bare `logo`
    // substring: every page inlines the entire CSS + JS payload, so a loose `contains`
    // is satisfied by the stylesheet rule alone and passes on a page that renders no
    // logo at all.

    fn site_with(name: &str, config: &str) -> String {
        let root = write_site(
            name,
            &[
                ("_site.yml", config),
                ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        let _ = std::fs::remove_dir_all(&root);
        html
    }

    #[test]
    fn website_brand_renders_the_configured_logo_inside_the_brand_link() {
        let html = site_with("brandlogo", "title: Acme Research\nlogo: brand.svg\n");
        assert!(
            html.contains(
                "<a class=\"tali-nav-brand\" href=\"index.html\">\
                 <img class=\"tali-brand-logo\" src=\"brand.svg\" alt=\"Acme Research\" /></a>"
            ),
            "the navbar brand must wrap the configured logo, with the site title as its \
             alt (the link's accessible name):\n{html}"
        );
        // The wordmark is REPLACED, not doubled: a logo file already carries the name, and
        // emitting both would immediately need a second key to turn the text off.
        assert!(
            !html.contains(">Acme Research</a>"),
            "the brand text must not also render beside the logo:\n{html}"
        );
    }

    #[test]
    fn website_brand_falls_back_to_the_title_text_without_a_logo() {
        let html = site_with("brandnologo", "title: Acme Research\n");
        assert!(
            html.contains("<a class=\"tali-nav-brand\" href=\"index.html\">Acme Research</a>"),
            "with no `logo:` the brand stays exactly the escaped title text:\n{html}"
        );
        assert!(
            !html.contains("<img class=\"tali-brand-logo\""),
            "an unconfigured project must emit no brand image at all:\n{html}"
        );
    }

    #[test]
    fn book_brand_renders_the_logo_in_both_the_topbar_and_the_drawer_head() {
        // `.tali-book-brand` is emitted TWICE (the sticky topbar and the drawer's head).
        // Fixing one and leaving the other is the shape of the bug this pins against.
        let root = write_site(
            "bookbrandlogo",
            &[
                (
                    "_site.yml",
                    "title: Field Manual\nlogo: brand.svg\nchapters:\n  - index.tmd\n  - two.tmd\n",
                ),
                ("index.tmd", "---\ntitle: One\n---\n\nx\n"),
                ("two.tmd", "---\ntitle: Two\n---\n\ny\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        let _ = std::fs::remove_dir_all(&root);
        let brand = "<a class=\"tali-book-brand\" href=\"index.html\">\
                     <img class=\"tali-brand-logo\" src=\"brand.svg\" alt=\"Field Manual\" /></a>";
        assert_eq!(
            html.matches(brand).count(),
            2,
            "both book brand slots (topbar + drawer head) must carry the logo:\n{html}"
        );
    }

    /// The book brand links to the book's HOME, and a book's home is not always
    /// `index.html`. `chapters:` is an ordered list of files the author names, and nothing
    /// requires the first to be `index.tmd` — `corpus/theorem-book/` declares `alpha.tmd`
    /// then `beta.tmd`, so its build emitted no `index.html` at all and the title in the
    /// topbar (and the drawer head, both slots) was a dead link on every page. Every other
    /// book in the repo happened to start with `index.tmd`, which is why it survived: the
    /// bug needs a book that simply named its chapters something else.
    #[test]
    fn the_book_brand_links_to_the_first_chapter_when_there_is_no_index() {
        let root = write_site(
            "booknoindex",
            &[
                (
                    "_site.yml",
                    "title: Counters\nchapters:\n  - alpha.tmd\n  - beta.tmd\n",
                ),
                ("alpha.tmd", "---\ntitle: Alpha\n---\n\nx\n"),
                ("beta.tmd", "---\ntitle: Beta\n---\n\ny\n"),
            ],
        );
        let site = Site::discover(&root);
        let beta = site.render_page("beta.tmd").unwrap();
        let _ = std::fs::remove_dir_all(&root);

        assert!(
            !beta.contains("<a class=\"tali-book-brand\" href=\"index.html\">"),
            "no chapter emits index.html, so the brand must not point at it:\n{beta}"
        );
        assert_eq!(
            beta.matches("<a class=\"tali-book-brand\" href=\"alpha.html\">")
                .count(),
            2,
            "both brand slots point at the first chapter, the book's actual home:\n{beta}"
        );
    }

    /// The website twin: a directory-walked site has no required entry file, so a site
    /// whose pages are simply named something else got a dead brand link on every page.
    /// `corpus/debug/` is exactly that (`sorting`/`leetcode`/`dp`/`custom-view`, no
    /// `index.tmd`) and every one of its four pages linked "home" to a 404.
    #[test]
    fn the_site_brand_links_to_the_first_page_when_there_is_no_index() {
        let root = write_site(
            "sitenoindex",
            &[
                ("_site.yml", "title: Exhibit\n"),
                ("alpha.tmd", "---\ntitle: Alpha\n---\n\nx\n"),
                ("beta.tmd", "---\ntitle: Beta\n---\n\ny\n"),
            ],
        );
        let site = Site::discover(&root);
        let beta = site.render_page("beta.tmd").unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            !beta.contains("<a class=\"tali-nav-brand\" href=\"index.html\">"),
            "no page emits index.html, so the brand must not point at it:\n{beta}"
        );
        assert!(
            beta.contains("<a class=\"tali-nav-brand\" href=\"alpha.html\">"),
            "the brand points at the first page, the site's actual home:\n{beta}"
        );
    }

    /// The other half: a book that DOES start with `index.tmd` keeps linking there, so the
    /// fix above is a fallback rather than a behaviour change for every existing book.
    #[test]
    fn the_book_brand_still_prefers_a_real_index_chapter() {
        let root = write_site(
            "bookwithindex",
            &[
                (
                    "_site.yml",
                    "title: Manual\nchapters:\n  - index.tmd\n  - two.tmd\n",
                ),
                ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
                ("two.tmd", "---\ntitle: Two\n---\n\ny\n"),
            ],
        );
        let site = Site::discover(&root);
        let two = site.render_page("two.tmd").unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            two.matches("<a class=\"tali-book-brand\" href=\"index.html\">")
                .count(),
            2,
            "an index chapter stays the home:\n{two}"
        );
    }

    #[test]
    fn a_logo_resolves_relative_to_the_page_depth_but_leaves_an_absolute_src_alone() {
        // Same depth rule `favicon:` uses: a project-relative path is written from the
        // site root, so a nested page has to climb back out or the image 404s in `_site/`.
        let root = write_site(
            "brandlogodepth",
            &[
                ("_site.yml", "title: Acme\nlogo: brand.svg\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
                ("posts/deep.tmd", "---\ntitle: Deep\n---\n\ny\n"),
            ],
        );
        let site = Site::discover(&root);
        let deep = site.render_page("posts/deep.tmd").unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            deep.contains("<img class=\"tali-brand-logo\" src=\"../brand.svg\" alt=\"Acme\" />"),
            "a page one level down must climb back to the logo:\n{deep}"
        );
        // A site-absolute or external source is written as the author meant it: prefixing
        // `../` there produces a path that resolves nowhere.
        assert_eq!(site_asset_href("/brand.svg", "../"), "/brand.svg");
        assert_eq!(
            site_asset_href("https://cdn.example/brand.svg", "../"),
            "https://cdn.example/brand.svg"
        );
        assert_eq!(
            site_asset_href("//cdn.example/b.svg", "../"),
            "//cdn.example/b.svg"
        );
    }

    #[test]
    fn a_titleless_book_with_a_logo_still_brands_on_the_logo_alone() {
        // Item 77 residual: both `.tali-book-brand` slots were gated on `book.title`, a
        // gate that predates `logo:` — so a book that configured a logo and no title got
        // no brand link at all, while a website in the same shape brands fine (its brand
        // always renders, falling back to "Home"). The logo IS the brand; a missing title
        // only costs the *label*, which is what the "Home" fallback is for.
        let root = write_site(
            "booklogonotitle",
            &[
                ("_site.yml", "logo: brand.svg\nchapters:\n  - index.tmd\n"),
                ("index.tmd", "---\ntitle: One\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        let _ = std::fs::remove_dir_all(&root);
        let brand = "<a class=\"tali-book-brand\" href=\"index.html\">\
                     <img class=\"tali-brand-logo\" src=\"brand.svg\" alt=\"Home\" /></a>";
        assert_eq!(
            html.matches(brand).count(),
            2,
            "both book brand slots must render the logo without a title:\n{html}"
        );
    }

    #[test]
    fn a_book_with_neither_title_nor_logo_emits_no_brand_link() {
        // The negative control, and a deliberate non-change: a bare "Home" wordmark where
        // a book's name belongs is noise, so an unbranded book still shows nothing. Without
        // this, the fix above passes on a topbar that brands unconditionally.
        let root = write_site(
            "booknobrand",
            &[
                ("_site.yml", "chapters:\n  - index.tmd\n"),
                ("index.tmd", "---\ntitle: One\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        let _ = std::fs::remove_dir_all(&root);
        // The full opening tag, not the bare class: `site.css` ships `.tali-book-brand`
        // rules and the page inlines the whole stylesheet, so the short needle matches on
        // a page that renders no brand at all.
        assert!(
            !html.contains("<a class=\"tali-book-brand\""),
            "an unbranded book must emit no brand link:\n{html}"
        );
    }

    #[test]
    fn a_favicon_resolves_by_depth_but_leaves_a_site_absolute_or_external_one_alone() {
        // Item 77 residual: `favicon:` predates `logo:`'s guard and prefixed
        // unconditionally, so `favicon: /brand.svg` on a nested page emitted
        // `../brand.svg` and 404'd — the author writes a site-absolute path precisely
        // BECAUSE they want one path that works from every depth. Both keys name a
        // project asset, so both resolve the same way.
        let root = write_site(
            "favicondepth",
            &[
                ("_site.yml", "title: Acme\nfavicon: icon.svg\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
                ("posts/deep.tmd", "---\ntitle: Deep\n---\n\ny\n"),
            ],
        );
        let site = Site::discover(&root);
        let deep = site.page("posts/deep.tmd").unwrap().clone();
        let home = site.page("index.tmd").unwrap().clone();
        let _ = std::fs::remove_dir_all(&root);
        // The project-relative case is unchanged: still climbs out per page depth.
        assert_eq!(site.page_chrome(&home).favicon, "icon.svg");
        assert_eq!(site.page_chrome(&deep).favicon, "../icon.svg");

        for (written, expect_deep) in [
            ("/brand.svg", "/brand.svg"),
            ("https://cdn.example/i.png", "https://cdn.example/i.png"),
            ("//cdn.example/i.png", "//cdn.example/i.png"),
        ] {
            let root = write_site(
                &format!("faviconabs{}", written.len()),
                &[
                    (
                        "_site.yml",
                        &format!("title: Acme\nfavicon: \"{written}\"\n"),
                    ),
                    ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
                    ("posts/deep.tmd", "---\ntitle: Deep\n---\n\ny\n"),
                ],
            );
            let site = Site::discover(&root);
            let deep = site.page("posts/deep.tmd").unwrap().clone();
            let _ = std::fs::remove_dir_all(&root);
            assert_eq!(
                site.page_chrome(&deep).favicon,
                expect_deep,
                "`favicon: {written}` must survive a nested page unprefixed"
            );
        }
    }

    #[test]
    fn a_blank_title_still_leaves_the_logo_link_an_accessible_name() {
        // The logo IS the link's content, so `alt=""` would leave a link with no
        // accessible name at all — the failure a decorative empty alt is correct for
        // everywhere else (the deck overlay's standalone logo included).
        let html = site_with("brandlogonotitle", "title: \"  \"\nlogo: brand.svg\n");
        assert!(
            html.contains("<img class=\"tali-brand-logo\" src=\"brand.svg\" alt=\"Home\" />"),
            "a blank project title must fall back to a real name, never `alt=\"\"`:\n{html}"
        );
        assert!(
            !html.contains("class=\"tali-brand-logo\" src=\"brand.svg\" alt=\"\""),
            "a meaningful brand image must never ship an empty alt:\n{html}"
        );
    }

    #[test]
    fn search_button_hides_the_shortcut_hint_from_its_name() {
        // WCAG 2.5.3: the visible ⌘K kbd must not pollute the button's accessible name.
        let b = search_button();
        assert!(
            b.contains("<kbd class='tali-search-kbd' aria-hidden='true'>"),
            "the shortcut hint kbd must be aria-hidden: {b}"
        );
        // The icon-only button still names itself.
        assert!(
            b.contains("aria-label='Search'"),
            "icon-only button keeps its label: {b}"
        );
    }

    // --- the chapter drawer is a modal, and must behave like one (MOB-5) --------------
    //
    // Re-derived from source, then measured in a browser, because most of the filed finding
    // had already been fixed: `role="dialog"` has been on the panel since 2369d80
    // (2026-07-07), `BOOK_DRAWER_SCRIPT` already calls `window.taliFocusTrap`, and the trap
    // itself both SETS `aria-modal` on open and REMOVES it on release — the correct
    // lifecycle, since a closed dialog is not modal. A static `aria-modal="true"` was tried
    // and reverted: the trap's release stripped it, leaving it present on load and absent
    // after the first close.
    //
    // The audit's "focus stays on `.tali-book-body`" was nonetheless a REAL symptom with a
    // wrong cause: it was the per-chapter section-outline hydration re-parenting the
    // focused chapter link (moving an element in the DOM blurs it), not the dialog markup.
    // That outline was deleted 2026-08-04 (visual minimalism pass), taking the symptom,
    // its fix, and this file's pin of it with it.

    fn book_page_html() -> String {
        let root = write_site(
            "bookdrawermodal",
            &[
                (
                    "_site.yml",
                    "title: A Book\nchapters:\n  - index.tmd\n  - two.tmd\n",
                ),
                ("index.tmd", "---\ntitle: One\n---\n\nx\n"),
                ("two.tmd", "---\ntitle: Two\n---\n\ny\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        let _ = std::fs::remove_dir_all(&root);
        html
    }

    #[test]
    fn book_drawer_panel_is_a_dialog_whose_modality_the_focus_trap_owns() {
        let html = book_page_html();
        // Needle the WHOLE tag, not a bare attribute: every page inlines the entire CSS + JS
        // payload, so a substring is satisfied by a script that merely mentions the name.
        assert!(
            html.contains(
                "<div class=\"tali-book-drawer-panel\" role=\"dialog\" aria-label=\"Chapters\">"
            ),
            "drawer panel is not a named role=dialog:\n{html}"
        );
        // Modality is the trap's job, so the script must actually hand it the PANEL (handing
        // it the outer container would trap against the backdrop and mark the wrong node).
        assert!(
            BOOK_DRAWER_SCRIPT.contains("release=window.taliFocusTrap(panel,f)"),
            "the drawer must route through taliFocusTrap, which is what supplies aria-modal \
             and the Tab confinement:\n{BOOK_DRAWER_SCRIPT}"
        );
    }

    #[test]
    fn book_drawer_locks_page_scroll_while_it_is_open() {
        // Measured on a 390x844 phone: with the drawer open, `scrollBy(0, 400)` moved the
        // article behind it by 328px, so a swipe meant for the chapter list moved the
        // chapter and dismissing returned the reader somewhere they did not choose.
        let js = BOOK_DRAWER_SCRIPT;
        assert!(
            js.contains("documentElement.style.overflow=o?'hidden':''"),
            "the drawer sets no scroll lock, so the page scrolls behind it:\n{js}"
        );
    }

    /// A `_site.yml` href is emitted on EVERY page, so a broken one is the highest-leverage
    /// broken link a project can have — and it was the only link class no validator saw,
    /// because cross-page validation harvests links from rendered page bodies.
    #[test]
    fn a_broken_nav_or_footer_href_is_reported() {
        let root = write_site(
            "chromelinks",
            &[
                (
                    "_site.yml",
                    "title: Site\nnav:\n  left:\n    - { text: Gone, href: missing.tmd }\n\
                     footer:\n  right:\n    - { text: Nope, href: nope.tmd }\n",
                ),
                ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let ws = site.validate_chrome_links();
        let joined = ws
            .iter()
            .map(|w| w.message.clone())
            .collect::<Vec<_>>()
            .join(" | ");

        assert!(joined.contains("missing.tmd"), "nav href: {joined}");
        assert!(joined.contains("nope.tmd"), "footer href: {joined}");
        assert_eq!(ws.len(), 2, "one per broken href: {joined}");
        assert!(
            ws.iter().all(|w| w.file.as_deref() == Some("_site.yml")),
            "located to the file the author must edit: {ws:?}"
        );
    }

    /// The three ways a chrome href is legitimately not a page. Each of these shipped in a
    /// real `_site.yml` before the validator existed, so each is a false positive the
    /// validator would otherwise invent.
    #[test]
    fn a_working_nav_or_footer_href_is_not_reported() {
        let root = write_site(
            "chromelinks-ok",
            &[
                (
                    "_site.yml",
                    "title: Site\nurl: https://ex.com\n\
                     nav:\n  left:\n    - { text: About, href: about.tmd }\n\
                     \x20   - { text: Code, href: \"https://github.com/x/y\" }\n\
                     \x20   - { text: Mail, href: \"mailto:a@b.c\" }\n\
                     \x20   - { text: Top, href: \"#top\" }\n\
                     \x20   - { text: Data, href: data.csv }\n\
                     \x20   - { text: NoHref }\n",
                ),
                ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
                ("about.tmd", "---\ntitle: About\n---\n\nx\n"),
                ("data.csv", "a,b\n"),
            ],
        );
        let site = Site::discover(&root);
        let ws = site.validate_chrome_links();
        assert!(
            ws.is_empty(),
            "a real page, an absolute URL, a mailto, a bare fragment, a raw asset on disk \
             and an href-less item are all fine: {ws:?}"
        );
    }

    /// Feeds are generated, not pages, so a feed link resolves against what the build
    /// emits. With no `url:` no feed is written at all — and the navbar, unlike the
    /// footer, has no drop rule, so it ships a link to a file that will not exist.
    #[test]
    fn a_feed_href_is_judged_against_the_feeds_the_build_writes() {
        let files = |cfg: &str| {
            vec![
                ("_site.yml".to_string(), cfg.to_string()),
                (
                    "index.tmd".to_string(),
                    "---\ntitle: Home\nlisting:\n  contents: posts\n  feed: true\n---\n\nx\n"
                        .to_string(),
                ),
                (
                    "posts/a.tmd".to_string(),
                    "---\ntitle: A\ndate: 2026-01-01\n---\n\nx\n".to_string(),
                ),
            ]
        };
        fn borrow(v: &[(String, String)]) -> Vec<(&str, &str)> {
            v.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
        }

        let with_url = files(
            "title: Blog\nurl: https://ex.com\nnav:\n  left:\n    - { text: RSS, href: index.xml }\n",
        );
        let root = write_site("chromefeed-on", &borrow(&with_url));
        let ws = Site::discover(&root).validate_chrome_links();
        assert!(ws.is_empty(), "the build writes index.xml: {ws:?}");

        let no_url = files("title: Blog\nnav:\n  left:\n    - { text: RSS, href: index.xml }\n");
        let root = write_site("chromefeed-off", &borrow(&no_url));
        let ws = Site::discover(&root).validate_chrome_links();
        assert_eq!(
            ws.len(),
            1,
            "without `url:` no feed is written, so the nav link 404s: {ws:?}"
        );

        // The footer DROPS a local `.xml` when no feed is generated, so there is nothing
        // to report there — flagging it would name a link the page does not carry.
        let footer =
            files("title: Blog\nfooter:\n  right:\n    - { icon: rss, href: index.xml }\n");
        let root = write_site("chromefeed-footer", &borrow(&footer));
        let ws = Site::discover(&root).validate_chrome_links();
        assert!(
            ws.is_empty(),
            "the footer drops it, so it is not broken: {ws:?}"
        );
    }
}
