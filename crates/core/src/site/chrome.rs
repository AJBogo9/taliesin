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
/// close it on Escape, on a backdrop / close-button click (`[data-qmd-drawer-close]`), or
/// after a chapter link is followed (restoring focus to the opener). `data-drawer-wired`
/// keeps it idempotent across hot-reload re-injects.
const BOOK_DRAWER_SCRIPT: &str = "<script>(function(){var b=document.getElementById('tali-book-drawer-btn'),d=document.getElementById('tali-book-drawer');if(!b||!d||b.dataset.drawerWired)return;b.dataset.drawerWired='1';function set(o){d.hidden=!o;b.setAttribute('aria-expanded',o?'true':'false');if(o){var f=d.querySelector('.tali-book-chapter')||d.querySelector('a,button');if(f)f.focus();}else{b.focus();}}b.addEventListener('click',function(){set(d.hidden);});d.addEventListener('click',function(e){if(e.target.closest('[data-qmd-drawer-close]')||e.target.closest('a'))set(false);});document.addEventListener('keydown',function(e){if(e.key==='Escape'&&!d.hidden)set(false);});})();</script>";

/// A search control that opens the Cmd-K palette. It carries `data-qmd-search`,
/// which `web-client/search.js` wires (by click delegation) to open the same
/// palette the keyboard shortcut does. Rendered in the navbar (websites) and the
/// book sidebar; `full` widens it with a label for the sidebar.
fn search_button(full: bool) -> String {
    let (cls, label) = if full {
        (
            "tali-search-btn tali-search-full",
            "<span class='tali-search-label'>Search the book</span>",
        )
    } else {
        ("tali-search-btn", "")
    };
    format!(
        "<button class='{cls}' type='button' data-qmd-search aria-label='Search' \
         aria-keyshortcuts='Control+K Meta+K'>{SEARCH_ICON}{label}\
         <kbd class='tali-search-kbd'>\u{2318}K</kbd></button>"
    )
}

/// A cog — the reader Settings gear glyph (Feather "settings"). Single-quoted attrs so it can
/// also live verbatim in the floating-launcher copy in `code-enhance/13-reader-menu.js`.
const SETTINGS_ICON: &str = "<svg width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><circle cx='12' cy='12' r='3'/><path d='M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z'/></svg>";

/// The reader Settings gear that docks in the navbar / book topbar. `taliInitReaderMenu`
/// (code-enhance) wires it via `[data-tali-settings]` click delegation and mounts the
/// Theme / Focus / Keyboard popover it opens; on a chrome-less single doc the same enhancer
/// instead creates a floating copy. Server-rendered so the icon shows before JS runs.
fn settings_button() -> String {
    format!(
        "<button class='tali-nav-settings' type='button' data-tali-settings \
         aria-label='Settings' aria-expanded='false'>{SETTINGS_ICON}</button>"
    )
}

impl Site {
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
            "<header class=\"tali-site-nav\" data-qmd-src=\"_site.yml\"><nav class=\"tali-nav-inner\" aria-label=\"Primary\">",
        );
        s.push_str(&format!(
            "<a class=\"tali-nav-brand\" href=\"{up}index.html\">{}</a>",
            esc(&brand_text)
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
        s.push_str(&search_button(false));
        s.push_str("</div>");
        // The reader Settings gear (theme / focus / shortcuts) sits OUTSIDE the collapsing
        // links so it stays visible top-right at every width, beside the burger on mobile.
        s.push_str(&settings_button());
        s.push_str("</nav></header>");
        // Wire the burger button: toggle `aria-expanded` + a `.tali-nav-open` class
        // the CSS shows the menu on, and close on Escape / link click. Idempotent
        // (a `data-wired` guard) so re-running it (live hot-reload re-injects the
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

    /// The slim site footer. Footer item text is treated as raw HTML (icon SVGs),
    /// per the trusted-source model. A configured local `.xml` link is dropped
    /// (this build generates no RSS feed).
    pub(super) fn footer_html(&self, depth: usize) -> String {
        let Some(footer) = &self.config.footer else {
            return String::new();
        };
        let up = "../".repeat(depth);
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
                    // A configured *local* `.xml` link (e.g. `/blog.xml`)
                    // is dropped: this build generates no RSS feed. An external
                    // `.xml` URL (http/protocol-relative) is left alone — it's some
                    // other resource, not this site's feed.
                    Some(h)
                        if h.ends_with(".xml")
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
            "<footer class=\"tali-site-footer\" data-qmd-src=\"_site.yml\"><div class=\"tali-foot-inner\">\
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
            "<header class=\"tali-book-topbar\" data-qmd-src=\"_site.yml\">\
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
        if let Some(t) = &book.title {
            s.push_str(&format!(
                "<a class=\"tali-book-brand\" href=\"{up}index.html\">{}</a>",
                esc(t)
            ));
        }
        s.push_str("<span class=\"tali-nav-spacer\"></span>");
        // A search button (opens the same Cmd-K palette) + the reader Settings gear (theme /
        // focus / shortcuts). The gear replaces the old light/dark toggle that lived here.
        s.push_str(&search_button(false));
        s.push_str(&settings_button());
        s.push_str("</div></header>");
        // --- the chapter drawer: an off-canvas overlay summoned from the topbar ---
        s.push_str(
            "<div class=\"tali-book-drawer\" id=\"tali-book-drawer\" hidden>\
             <div class=\"tali-book-drawer-backdrop\" data-qmd-drawer-close></div>\
             <div class=\"tali-book-drawer-panel\">",
        );
        // The `tali-book-sidebar` nav (kept for the chapter list + its aria-label) now lives
        // inside the drawer panel rather than a left rail.
        s.push_str(
            "<nav class=\"tali-book-sidebar\" data-qmd-src=\"_site.yml\" aria-label=\"Chapters\">",
        );
        s.push_str("<div class=\"tali-book-sidebar-head\">");
        if let Some(t) = &book.title {
            s.push_str(&format!(
                "<a class=\"tali-book-brand\" href=\"{up}index.html\">{}</a>",
                esc(t)
            ));
        }
        s.push_str(
            "<button type=\"button\" class=\"tali-book-drawer-close\" data-qmd-drawer-close \
             aria-label=\"Close chapters\">\u{2715}</button>",
        );
        s.push_str("</div>");
        s.push_str("<ul class=\"tali-book-chapters\" id=\"tali-book-chapters\">");
        for e in &book.entries {
            if let Some(part) = &e.part {
                s.push_str(&format!("<li class=\"tali-book-part\">{}</li>", esc(part)));
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
            s.push_str(&format!(
                "<li><a class=\"{cls}\" href=\"{up}{}\"{aria}>{num}{}</a></li>",
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
