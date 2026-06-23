//! OpenGraph / Twitter-card / SEO `<meta>` tags for a site page, injected into the
//! head via `SiteCtx` so a shared link renders a rich preview. Per-page title,
//! description, canonical URL, and card image come from the page's front matter,
//! falling back to the site config. The absolute-URL tags (og:url, og:image,
//! canonical) are emitted only when a `url:` is configured.

use super::{Page, Site};
use crate::escape_attr as esc;

fn meta(attr: &str, key: &str, val: &str) -> String {
    format!("\n<meta {attr}=\"{key}\" content=\"{}\">", esc(val))
}

/// The social/SEO meta block for `page` (leading-newline-separated tags, ready to
/// append to the head include).
pub(super) fn social_head(site: &Site, page: &Page) -> String {
    let cfg = &site.config;
    let title = page.title.as_deref().or(cfg.title.as_deref()).unwrap_or("");
    let desc = page.description.as_deref().or(cfg.description.as_deref());
    let base = cfg.url.as_deref().map(|u| u.trim_end_matches('/'));
    // Use the clean directory URL for canonical/og:url: an index page is served at
    // its directory (`/posts/x/`), not `/posts/x/index.html`.
    let clean_url = page.url.strip_suffix("index.html").unwrap_or(&page.url);
    let page_url = base.map(|b| format!("{b}/{clean_url}"));
    // Card image, made absolute (social scrapers require absolute image URLs). Falls
    // back to the site-wide default (`image:` / Quarto `open-graph: image:`).
    let image = page
        .card_image
        .as_deref()
        .or(cfg.card_image.as_deref())
        .zip(base)
        .map(|(img, b)| format!("{b}/{}", img.trim_start_matches('/')));
    let og_type = if page.date.is_some() {
        "article"
    } else {
        "website"
    };

    let mut h = String::new();
    if let Some(d) = desc {
        h.push_str(&meta("name", "description", d));
    }
    h.push_str(&meta("property", "og:type", og_type));
    if !title.is_empty() {
        h.push_str(&meta("property", "og:title", title));
    }
    if let Some(d) = desc {
        h.push_str(&meta("property", "og:description", d));
    }
    if let Some(s) = cfg.title.as_deref() {
        h.push_str(&meta("property", "og:site_name", s));
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
    if !title.is_empty() {
        h.push_str(&meta("name", "twitter:title", title));
    }
    if let Some(d) = desc {
        h.push_str(&meta("name", "twitter:description", d));
    }
    if let Some(img) = &image {
        h.push_str(&meta("name", "twitter:image", img));
    }
    if let Some(u) = &page_url {
        h.push_str(&format!("\n<link rel=\"canonical\" href=\"{}\">", esc(u)));
    }
    h
}
