//! OpenGraph / Twitter-card `<meta>` tags for a site page, injected into the head via
//! `SiteCtx` so a shared link unfurls with the page's own title, description and image.
//! Plus the Atom autodiscovery `<link>` ([`feed_head`]), which is head markup for the same
//! reason and has nowhere better to live.
//!
//! **Reduced on 2026-08-08 to the five tags an unfurl actually needs**, from a block that
//! also carried `og:type`, `og:site_name`, `og:image:width`/`height`, `twitter:title`,
//! `twitter:description`, `twitter:image`, `<link rel="canonical">` and a schema.org
//! JSON-LD `@graph`. The rasterized social-card generator that fed `og:image` went in the
//! same cut, and the image now comes from the page's OWN front-matter `image:` — a key
//! that was already live (the listing/in-page thumbnail) and that eleven real documents
//! set. Nothing here is generated, so nothing here can drift from a file on disk.
//!
//! Everything is `url:`-gated: an absolute `og:url` and an absolute `og:image` are the
//! whole point, and a site with no `url:` cannot form either.

use super::{Page, Site};
use crate::escape_attr as esc;

fn meta(attr: &str, key: &str, val: &str) -> String {
    format!("\n<meta {attr}=\"{key}\" content=\"{}\">", esc(val))
}

/// The social meta block for `page` (leading-newline-separated tags, ready to append to the
/// head include).
///
/// `og:image` resolves the page's front-matter `image:`, which `discovery.rs` has already
/// stored site-root-relative (or left alone when it is an absolute/external URL), against
/// the site's canonical base. A page with no `image:` emits no `og:image`, and its
/// `twitter:card` degrades from `summary_large_image` to `summary` — which is the correct
/// shape for a link with no picture, not a defect.
pub(super) fn social_head(site: &Site, page: &Page) -> String {
    let cfg = &site.config;
    let title = page.title.as_deref().or(cfg.title.as_deref()).unwrap_or("");
    let desc = page.description.as_deref().or(cfg.description.as_deref());
    let base = site.canonical_base();
    // The clean directory URL, from `abs_page_url` and not a second hand-rolled copy of it.
    // This assembled the string itself until 2026-08-17 and so skipped the percent-encoding
    // the shared helper does: a page in a directory with a space shipped a raw space in its
    // `og:url` while the sitemap's `<loc>` for the same page was correctly encoded. Pages
    // are discovered from the filesystem and never slugified, so that path is reachable by
    // naming a folder the way people name folders.
    let page_url = site.abs_page_url(page);
    // The 404 page is not scraped content, so it advertises no image.
    let image = if page.url == "404.html" {
        None
    } else {
        page.card_image.as_deref().and_then(|img| {
            if img.starts_with("http://") || img.starts_with("https://") {
                Some(img.to_string())
            } else {
                base.map(|b| {
                    format!(
                        "{b}/{}",
                        super::feed::percent_encode_path(img.trim_start_matches('/'))
                    )
                })
            }
        })
    };

    let mut h = String::new();
    if let Some(d) = desc {
        // Not an OpenGraph tag, and kept anyway: it is the one line that decides what a
        // search result reads like, which is the same job `seo.rs`'s sitemap exists for.
        h.push_str(&meta("name", "description", d));
        h.push_str(&meta("property", "og:description", d));
    }
    if !title.is_empty() {
        h.push_str(&meta("property", "og:title", title));
    }
    if let Some(u) = &page_url {
        h.push_str(&meta("property", "og:url", u));
    }
    if let Some(img) = &image {
        h.push_str(&meta("property", "og:image", img));
    }
    h.push_str(&meta(
        "name",
        "twitter:card",
        if image.is_some() {
            "summary_large_image"
        } else {
            "summary"
        },
    ));
    h
}

/// `<link rel="alternate" type="application/atom+xml">` autodiscovery tags so a browser
/// or feed reader detects the site's Atom feed(s). One per feed the build writes (see
/// `Site::feed_index`); the href is absolute (feeds only exist when `url:` is set, so a
/// canonical base is always available here). Site-global — the same on every page — which
/// is why it takes no `page`. Empty when no feed is generated (no `url:`, or no dated
/// listing). Distinct from the human-facing footer feed link (which is relative).
pub(super) fn feed_head(site: &Site) -> String {
    let feeds = site.feed_index();
    if feeds.is_empty() {
        return String::new();
    }
    let base = site.canonical_base().unwrap_or("");
    let mut h = String::new();
    for (path, title) in feeds {
        h.push_str(&format!(
            "\n<link rel=\"alternate\" type=\"application/atom+xml\" title=\"{}\" href=\"{}\">",
            esc(&title),
            esc(&format!("{base}/{path}")),
        ));
    }
    h
}

#[cfg(test)]
mod tests {
    use crate::site::{Site, tests::write_site};

    /// The whole point of the block: a post unfurls with its own title, description, URL and
    /// picture. Driven through a real render rather than the emitter, because the head
    /// include is what a scraper actually reads.
    #[test]
    fn a_post_unfurls_with_its_own_title_description_url_and_image() {
        let root = write_site(
            "ogunfurl",
            &[
                (
                    "_site.yml",
                    "title: Journal\nurl: https://ex.com\ndescription: Site blurb.\n",
                ),
                (
                    "posts/one.tmd",
                    "---\ntitle: A Post\ndate: 2026-05-15\ndescription: Post blurb.\n\
                     image: thumb.webp\n---\n\nBody.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("posts/one.tmd").unwrap();

        for want in [
            r#"<meta property="og:title" content="A Post">"#,
            r#"<meta property="og:description" content="Post blurb.">"#,
            r#"<meta property="og:url" content="https://ex.com/posts/one.html">"#,
            r#"<meta property="og:image" content="https://ex.com/posts/thumb.webp">"#,
            r#"<meta name="twitter:card" content="summary_large_image">"#,
            r#"<meta name="description" content="Post blurb.">"#,
        ] {
            assert!(html.contains(want), "missing {want} in: {html}");
        }
        // The page's own description wins over the site's.
        assert!(
            !html.contains("Site blurb."),
            "the site description must not override the page's: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `og:url` and the sitemap's `<loc>` must be the SAME string for the same page.
    ///
    /// **The defect (Fable audit FA28).** `social_head` assembled the absolute URL itself
    /// instead of asking `abs_page_url`, so it skipped that helper's percent-encoding: a
    /// page in a directory with a space published a raw space in `og:url` (an invalid URL a
    /// scraper resolves however it likes) while the sitemap for the same page was correct.
    /// Pages are discovered from the filesystem and never slugified, so the only thing
    /// needed to reach it is a folder named the way people name folders.
    #[test]
    fn og_url_is_the_same_absolute_url_the_sitemap_publishes() {
        let root = write_site(
            "ogencode",
            &[
                (
                    "_site.yml",
                    "title: J
url: https://ex.com
",
                ),
                (
                    "my posts/one.tmd",
                    "---
title: P
image: my thumb.webp
---

Body.
",
                ),
            ],
        );
        let site = Site::discover(&root);
        let page = site
            .pages
            .iter()
            .find(|p| p.rel.contains("one"))
            .expect("the page was discovered");
        let want = site.abs_page_url(page).expect("an absolute url");
        assert!(
            want.contains("my%20posts"),
            "the shared helper encodes the space: {want}"
        );
        let html = site.render_page(&page.rel).unwrap();
        assert!(
            html.contains(&format!(r#"<meta property="og:url" content="{want}">"#)),
            "og:url must be the encoded one: {html}"
        );
        assert!(
            html.contains(r#"content="https://ex.com/my%20posts/my%20thumb.webp""#),
            "and so must og:image: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// An absolute `image:` is used as written. `discovery.rs` deliberately leaves an
    /// external URL alone rather than folding it into a relative path, and joining a base
    /// onto it here would undo that.
    #[test]
    fn an_absolute_image_is_not_rebased() {
        let root = write_site(
            "ogabsimage",
            &[
                ("_site.yml", "title: J\nurl: https://ex.com\n"),
                (
                    "abs.tmd",
                    "---\ntitle: Abs\nimage: https://cdn.example.com/card.png\n---\n\nx\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("abs.tmd").unwrap();
        assert!(
            html.contains(
                r#"<meta property="og:image" content="https://cdn.example.com/card.png">"#
            ),
            "an external image: is used verbatim: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// No `url:`, no absolute anything. `og:url` and a rebased `og:image` cannot be formed,
    /// and a relative one is worthless to a scraper — so the block degrades to the text card.
    #[test]
    fn without_a_site_url_there_is_no_og_url_and_no_rebased_image() {
        let root = write_site(
            "ognourl",
            &[
                ("_site.yml", "title: J\n"),
                ("p.tmd", "---\ntitle: P\nimage: thumb.webp\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("p.tmd").unwrap();
        assert!(!html.contains("og:url"), "no og:url without url:: {html}");
        assert!(
            !html.contains("og:image"),
            "no og:image without url:: {html}"
        );
        assert!(
            html.contains(r#"<meta name="twitter:card" content="summary">"#),
            "the card degrades to summary: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A page with no `image:` is the common case, and it must not advertise one.
    #[test]
    fn a_page_without_an_image_advertises_no_og_image() {
        let root = write_site(
            "ognoimage",
            &[
                ("_site.yml", "title: J\nurl: https://ex.com\n"),
                ("plain.tmd", "---\ntitle: Plain\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("plain.tmd").unwrap();
        assert!(!html.contains("og:image"), "no image, no og:image: {html}");
        assert!(
            html.contains(r#"<meta name="twitter:card" content="summary">"#),
            "the card degrades to summary: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The 404 page is generated, never shared, and (unlike every other page) is excluded
    /// from the sitemap too — so it advertises no image even if one were resolvable.
    #[test]
    fn the_error_page_advertises_no_image() {
        let root = write_site(
            "og404",
            &[
                ("_site.yml", "title: J\nurl: https://ex.com\n"),
                ("404.tmd", "---\ntitle: Gone\nimage: thumb.webp\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("404.tmd").unwrap();
        assert!(
            !html.contains("og:image"),
            "the 404 page advertises no image: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
