//! OpenGraph / Twitter-card / SEO `<meta>` tags for a site page, injected into the
//! head via `SiteCtx` so a shared link renders a rich preview. Per-page title,
//! description, and canonical URL come from the page's front matter, falling back to
//! the site config. The social-card image is always the build-generated card
//! (`card::card_url`), never the page's own `image:` (that stays the in-page/listing
//! thumbnail). og:url, canonical, and og:image all need the site `url:` to be absolute,
//! so they're all `None` without it.

use super::card;
use super::{Page, Site};
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
    // Card image: the build-generated, branded OG card (absolute). Url-gated exactly
    // like the sidecars — `card_url` is Some only when `url:` is set, and `base` is Some
    // in the same case. The page's own `image:` stays the in-page/listing thumbnail.
    let image = card::card_url(site, page).and_then(|rel| base.map(|b| format!("{b}{rel}")));
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
        if let Some(rel) = card::card_url(site, page) {
            bp["image"] = json!(format!("{base}{rel}"));
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
    fn social_image_is_the_generated_card_not_the_page_image() {
        let root = write_site(
            "cardsocial",
            &[
                ("_site.yml", "title: Blog\nurl: https://ex.com\n"),
                (
                    "posts/a/index.tmd",
                    "---\ntitle: My Post\ndate: 2026-05-15\ndescription: About.\nimage: fig.webp\n---\n\nx\n",
                ),
            ],
        );
        let site = crate::site::Site::discover(&root);
        let post = site
            .pages
            .iter()
            .find(|p| p.url.contains("posts/a"))
            .unwrap();
        let rel = crate::site::card_rel_path(&crate::site::card_spec(&site, post));
        let html = site.render_page("posts/a/index.tmd").unwrap();
        assert!(
            html.contains(&format!(
                r#"property="og:image" content="https://ex.com/{rel}""#
            )),
            "og:image = card"
        );
        assert!(
            html.contains(&format!(
                r#"name="twitter:image" content="https://ex.com/{rel}""#
            )),
            "twitter:image = card"
        );
        assert!(
            !html.contains("fig.webp"),
            "the page image: is not the social card"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn error_page_emits_no_card_image() {
        // Finding A: the build skips the 404 card, so social_head must not point og:image at a
        // card file that is never written.
        let root = write_site(
            "card404",
            &[
                ("_site.yml", "title: Blog\nurl: https://ex.com\n"),
                ("index.tmd", "---\ntitle: H\n---\n\nx\n"),
                ("404.tmd", "---\ntitle: Not found\n---\n\nnope\n"),
            ],
        );
        let site = crate::site::Site::discover(&root);
        // Sanity: the 404 page is discovered with url "404.html" (matches the build skip + the gate).
        assert!(
            site.pages.iter().any(|p| p.url == "404.html"),
            "404 page discovered"
        );
        let html = site.render_page("404.tmd").unwrap();
        assert!(!html.contains("og:image"), "404 emits no og:image");
        assert!(
            !html.contains("twitter:image"),
            "404 emits no twitter:image"
        );
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
