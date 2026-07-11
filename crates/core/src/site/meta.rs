//! OpenGraph / Twitter-card / SEO `<meta>` tags for a site page, injected into the
//! head via `SiteCtx` so a shared link renders a rich preview. Per-page title,
//! description, canonical URL, and card image come from the page's front matter,
//! falling back to the site config. og:url and canonical need the site `url:` to be
//! absolute; og:image does too for a site-root-relative image, but an already-absolute
//! `image:` URL is emitted verbatim without one.

use super::{Page, Site, is_external_or_special};
use crate::escape_attr as esc;
use serde_json::{Value, json};

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
    // back to the site-wide default (`image:`, or `open-graph: image:`). An
    // already-absolute image URL (a CDN-hosted card) is used verbatim and works even
    // without a configured `url:`; a site-root-relative image is joined onto `base`,
    // which needs `url:` to exist.
    let image = page
        .card_image
        .as_deref()
        .or(cfg.card_image.as_deref())
        .and_then(|img| {
            if is_external_or_special(img) {
                Some(img.to_string())
            } else {
                base.map(|b| format!("{b}/{}", img.trim_start_matches('/')))
            }
        });
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
    // Google Scholar / Highwire-Press `citation_*` meta so a scholarly post is indexable
    // by academic databases. Emitted only for an article (has a `date`) that names an
    // `author` — a blog post's shape, not a nav/landing page. `citation_pdf_url` is
    // intentionally absent (there is no PDF; the print-pdf track is deferred).
    if page.date.is_some() && !page.authors.is_empty() {
        if !title.is_empty() {
            h.push_str(&meta("name", "citation_title", title));
        }
        for author in &page.authors {
            h.push_str(&meta("name", "citation_author", author));
        }
        if let Some(d) = page.date.as_deref() {
            h.push_str(&meta("name", "citation_publication_date", d));
        }
        // The site title is the closest thing to a journal/venue name.
        if let Some(journal) = cfg.title.as_deref() {
            h.push_str(&meta("name", "citation_journal_title", journal));
        }
        if let Some(u) = &page_url {
            h.push_str(&meta("name", "citation_public_url", u));
        }
    }
    h
}

/// schema.org JSON-LD for `page`, url-gated: a post (`date:` present) → `BlogPosting`;
/// the root index page → a `WebSite` + `Person` `@graph`. Empty string otherwise (no
/// `url:`, or a non-post inner page). Injected into the head beside `social_head`, so
/// it also appears in the live preview. All string values are JSON-escaped by
/// `serde_json`; `</` is additionally escaped so a description can't break the script.
pub(super) fn jsonld_head(site: &Site, page: &Page) -> String {
    let Some(base) = site.canonical_base() else {
        return String::new();
    };
    let author = site
        .config
        .author
        .as_ref()
        .and_then(|a| a.as_str())
        .or(site.config.title.as_deref())
        .unwrap_or("");
    let data: Option<Value> = if page.date.is_some() {
        let url = site.abs_page_url(page).unwrap_or_default();
        let mut bp = json!({
            "@context": "https://schema.org",
            "@type": "BlogPosting",
            "headline": page.title.as_deref().unwrap_or(""),
            "datePublished": page.date.as_deref().unwrap_or(""),
            "dateModified": page.date.as_deref().unwrap_or(""),
            "mainEntityOfPage": &url,
            "url": &url,
        });
        if !author.is_empty() {
            bp["author"] = json!({ "@type": "Person", "name": author });
        }
        if let Some(d) = page.description.as_deref() {
            bp["description"] = json!(d);
        }
        if let Some(img) = page
            .card_image
            .as_deref()
            .or(site.config.card_image.as_deref())
        {
            let abs = if is_external_or_special(img) {
                img.to_string()
            } else {
                format!("{base}/{}", img.trim_start_matches('/'))
            };
            bp["image"] = json!(abs);
        }
        Some(bp)
    } else if page.url == "index.html" {
        let website = json!({
            "@type": "WebSite",
            "name": site.config.title.as_deref().unwrap_or(""),
            "url": base,
            "description": site.config.description.as_deref().unwrap_or(""),
        });
        let mut person = json!({
            "@type": "Person",
            "name": author,
            "url": base,
        });
        let same_as = footer_social_links(site);
        if !same_as.is_empty() {
            person["sameAs"] = json!(same_as);
        }
        Some(json!({ "@context": "https://schema.org", "@graph": [website, person] }))
    } else {
        None
    };
    match data {
        Some(v) => format!(
            "\n<script type=\"application/ld+json\">{}</script>",
            v.to_string().replace("</", "<\\/")
        ),
        None => String::new(),
    }
}

/// Absolute social URLs from footer items that carry an `icon:` (the Person `sameAs`).
fn footer_social_links(site: &Site) -> Vec<String> {
    let Some(f) = site.config.footer.as_ref() else {
        return Vec::new();
    };
    f.left
        .iter()
        .chain(&f.center)
        .chain(&f.right)
        .filter(|it| it.icon.is_some())
        .filter_map(|it| it.href.clone())
        .filter(|h| h.starts_with("http"))
        .collect()
}

#[cfg(test)]
mod jsonld_tests {
    use crate::site::{Site, tests::write_site};

    #[test]
    fn post_emits_blogposting() {
        let root = write_site(
            "jsonldpost",
            &[
                ("_site.yml", "title: Blog\nurl: https://ex.com\n"),
                (
                    "posts/a/index.tmd",
                    "---\ntitle: My Post\ndate: 2026-05-15\ndescription: About things.\n---\n\nx\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("posts/a/index.tmd").unwrap();
        assert!(
            html.contains(r#""@type":"BlogPosting""#),
            "BlogPosting present"
        );
        assert!(html.contains(r#""headline":"My Post""#));
        assert!(html.contains(r#""datePublished":"2026-05-15""#));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn home_emits_website_and_person() {
        let root = write_site(
            "jsonldhome",
            &[
                (
                    "_site.yml",
                    "title: Andreas Bogossian\nurl: https://ex.com\nfooter:\n  right:\n    - { icon: github, href: https://github.com/x }\n",
                ),
                ("index.tmd", "---\ntitle: Andreas Bogossian\n---\n\nHi.\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        assert!(html.contains(r#""@type":"WebSite""#), "WebSite present");
        assert!(html.contains(r#""@type":"Person""#), "Person present");
        assert!(
            html.contains(r#""name":"Andreas Bogossian""#),
            "person name = title fallback"
        );
        assert!(html.contains("https://github.com/x"), "sameAs from footer");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_jsonld_without_url() {
        let root = write_site(
            "jsonldnourl",
            &[
                ("_site.yml", "title: B\n"),
                ("index.tmd", "---\ntitle: H\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        assert!(
            !html.contains("application/ld+json"),
            "no JSON-LD without url:"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
