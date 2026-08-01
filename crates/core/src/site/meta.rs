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

/// The core OpenGraph + Twitter-card `<meta>` block from already-resolved fields. Shared by
/// the page path ([`social_head`]) and the embedded-deck path ([`deck_social_head`]) so both
/// emit the same tag shape by construction (no reimplement-next-door drift). `image`
/// present upgrades the Twitter card to `summary_large_image`; a `page_url` also emits the
/// `<link rel="canonical">`. All values are already absolute + url-gated by the caller.
fn emit_social(
    site_title: Option<&str>,
    title: &str,
    desc: Option<&str>,
    page_url: Option<&str>,
    image: Option<&str>,
    og_type: &str,
) -> String {
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
    if let Some(s) = site_title {
        h.push_str(&meta("property", "og:site_name", s));
    }
    if let Some(u) = page_url {
        h.push_str(&meta("property", "og:url", u));
    }
    if let Some(img) = image {
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
    if let Some(img) = image {
        h.push_str(&meta("name", "twitter:image", img));
    }
    if let Some(u) = page_url {
        h.push_str(&format!("\n<link rel=\"canonical\" href=\"{}\">", esc(u)));
    }
    h
}

/// The social/SEO meta block for an embedded deck (built off-`Page`, so it needs its own
/// entry point). `deck_url` is the deck's site-root-relative output URL (e.g. `talk.html`).
/// Url-gated exactly like a page: the absolute `og:url` + the branded `og:image` card appear
/// only when `_site.yml` sets `url:` (else the deck keeps a plain `summary` text card built
/// from its title/subtitle). `og:type` is `website` (a deck is not a dated article), and
/// there is no `citation_*` (a deck is not a scholarly document). The `og:image` derives from
/// the same [`card::deck_card_spec`] the build writes to disk, so URL and file agree.
pub(crate) fn deck_social_head(
    site: &Site,
    deck_url: &str,
    title: Option<&str>,
    lead: Option<&str>,
) -> String {
    let cfg = &site.config;
    let base = cfg.url.as_deref().map(|u| u.trim_end_matches('/'));
    let doc_title = title
        .filter(|s| !s.is_empty())
        .or(cfg.title.as_deref())
        .unwrap_or("");
    let page_url = base.map(|b| format!("{b}/{deck_url}"));
    let image = base.map(|b| {
        format!(
            "{b}/{}",
            card::card_rel_path(&card::deck_card_spec(site, title, lead))
        )
    });
    emit_social(
        cfg.title.as_deref(),
        doc_title,
        lead.filter(|s| !s.is_empty()),
        page_url.as_deref(),
        image.as_deref(),
        "website",
    )
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

    // The core OG/Twitter block is shared verbatim with a deck (`deck_social_head`) so the
    // two can't drift; the page-only `citation_*` scholar block is appended below.
    let mut h = emit_social(
        cfg.title.as_deref(),
        title,
        desc,
        page_url.as_deref(),
        image.as_deref(),
        og_type,
    );
    // Google Scholar / Highwire-Press `citation_*` meta so a scholarly post is indexable
    // by academic databases. Emitted only for an article (has a `date`) that names an
    // `author` — a blog post's shape, not a nav/landing page. `citation_pdf_url` is
    // intentionally absent (there is no PDF; the print-pdf track is deferred).
    if page.date.is_some() && !page.authors.is_empty() {
        if !title.is_empty() {
            h.push_str(&meta("name", "citation_title", title));
        }
        for author in &page.authors {
            h.push_str(&meta("name", "citation_author", &author.name));
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
    // An authorless site falls back to its title (an organisation-style byline). A declared
    // `author:` is used as written: reading a sequence as a scalar used to yield nothing, so
    // a multi-author site published its own title as the author.
    let authors: Vec<&str> = if site.config.authors.is_empty() {
        site.config.title.as_deref().into_iter().collect()
    } else {
        site.config
            .authors
            .iter()
            .map(|a| a.name.as_str())
            .collect()
    };
    // schema.org takes a lone Person or an array, so a single-author site is unchanged.
    let author_value = |authors: &[&str]| -> Option<Value> {
        match authors {
            [] => None,
            [one] => Some(json!({ "@type": "Person", "name": one })),
            many => Some(Value::Array(
                many.iter()
                    .map(|n| json!({ "@type": "Person", "name": n }))
                    .collect(),
            )),
        }
    };
    // The homepage's identity Person carries the site's singular `url` + `sameAs` socials,
    // so it names the primary (first) author rather than the whole list.
    let author = authors.first().copied().unwrap_or("");
    let data: Option<Value> = if page.date.is_some() {
        let url = site.abs_page_url(page).unwrap_or_default();
        // A dated post that declares a `bibliography:` is a cited/scholarly document, so it
        // gets `ScholarlyArticle` (richer for research crawlers + LLMs) rather than a plain
        // `BlogPosting`. Author-free, so a research post with no `author:` still upgrades.
        let kind = if page.has_bibliography {
            "ScholarlyArticle"
        } else {
            "BlogPosting"
        };
        let mut bp = json!({
            "@context": "https://schema.org",
            "@type": kind,
            "headline": page.title.as_deref().unwrap_or(""),
            "datePublished": page.date.as_deref().unwrap_or(""),
            "dateModified": page.date.as_deref().unwrap_or(""),
            "mainEntityOfPage": &url,
            "url": &url,
        });
        // The page's OWN authors, falling back to the site's — the same chain
        // `cite_this::resolve` documents, and the same one `citation_author` already
        // followed. This branch used to read the site config alone, so a page that named
        // its own authors still advertised the site owner as the author of the article:
        // the two metadata blocks on one page disagreed about who wrote it.
        let declared = if page.authors.is_empty() {
            &site.config.authors
        } else {
            &page.authors
        };
        if let Some(a) = person_value(declared) {
            bp["author"] = a;
        } else if let Some(a) = author_value(&authors) {
            bp["author"] = a;
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
            // Escape EVERY `<`, not just `</`: `<!--<script>` drives the HTML tokenizer
            // into script-data-double-escaped state, where the emitted `</script>` stops
            // closing the element, so the payload swallows the rest of the document and
            // the page renders blank. JSON structural syntax contains no `<`, so every
            // `<` here is inside a string value and `<` round-trips it exactly.
            // (`search.rs::json_str` neutralizes `<` the same way, per-char as it builds.)
            "\n<script type=\"application/ld+json\">{}</script>",
            v.to_string().replace('<', "\\u003c")
        ),
        None => String::new(),
    }
}

/// Declared authors as schema.org `Person` objects, carrying whatever each one declared:
/// `affiliation` as an `Organization` (item 184's third consumer), `url`, `email`, and an
/// ORCID as `sameAs` — which is the property a crawler resolves an identity through, and
/// the reason storing a bare ORCID string would be worth nothing.
///
/// `None` when nobody is named, so the caller keeps its site-title fallback.
fn person_value(authors: &[crate::author::Author]) -> Option<Value> {
    let people: Vec<Value> = authors
        .iter()
        .filter(|a| !a.name.trim().is_empty())
        .map(|a| {
            let mut p = json!({ "@type": "Person", "name": a.name });
            match a.affiliations.as_slice() {
                [] => {}
                [one] => p["affiliation"] = json!({ "@type": "Organization", "name": one }),
                many => {
                    p["affiliation"] = Value::Array(
                        many.iter()
                            .map(|n| json!({ "@type": "Organization", "name": n }))
                            .collect(),
                    )
                }
            }
            if let Some(u) = &a.url {
                p["url"] = json!(u);
            }
            if let Some(e) = &a.email {
                p["email"] = json!(e);
            }
            if let Some(o) = &a.orcid {
                // An ORCID identifies the person only if it is resolvable; a bare digit
                // string is not something a consumer can follow.
                let iri = if o.starts_with("http") {
                    o.clone()
                } else {
                    format!("https://orcid.org/{o}")
                };
                p["sameAs"] = json!(iri);
            }
            p
        })
        .collect();
    match people.len() {
        0 => None,
        1 => people.into_iter().next(),
        _ => Some(Value::Array(people)),
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
    fn a_structured_author_reaches_json_ld_with_its_affiliation() {
        // Item 184's third consumer. Also pins the fix that came with it: this branch read
        // the SITE config's authors, so a page naming its own authors still advertised the
        // site owner as the article's author — while `citation_author` on the same page
        // named the real ones. Two metadata blocks disagreeing about who wrote the page is
        // worse than either being absent.
        let root = write_site(
            "jsonldaffil",
            &[
                (
                    "_site.yml",
                    "title: Journal\nurl: https://ex.com\nauthor: \"Site Owner\"\n",
                ),
                (
                    "posts/a/index.tmd",
                    concat!(
                        "---\n",
                        "title: My Paper\n",
                        "date: 2026-05-15\n",
                        "author:\n",
                        "  - name: Ada Lovelace\n",
                        "    affiliation: Analytical Engine Institute\n",
                        "    orcid: 0000-0002-1825-0097\n",
                        "---\n\nx\n",
                    ),
                ),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("posts/a/index.tmd").unwrap();
        assert!(
            html.contains(r#""name":"Ada Lovelace""#),
            "the PAGE's author wins over the site's: {html}"
        );
        assert!(
            !html.contains(r#""@type":"Person","name":"Site Owner""#),
            "the site owner must not be published as this page's author: {html}"
        );
        assert!(
            html.contains(
                r#""affiliation":{"@type":"Organization","name":"Analytical Engine Institute"}"#
            ),
            "affiliation rides along as an Organization: {html}"
        );
        assert!(
            html.contains(r#""sameAs":"https://orcid.org/0000-0002-1825-0097""#),
            "a bare ORCID is published as a RESOLVABLE iri, or it identifies nobody: {html}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

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

    /// A dated post that declares a `bibliography:` is scholarly, so it upgrades to
    /// `ScholarlyArticle` (author-free — no `author:` needed).
    #[test]
    fn cited_post_emits_scholarly_article() {
        let root = write_site(
            "jsonldsci",
            &[
                ("_site.yml", "title: Blog\nurl: https://ex.com\n"),
                (
                    "references.bib",
                    "@book{k,\n title={T},\n author={A},\n year={2020}\n}\n",
                ),
                (
                    "posts/p/index.tmd",
                    "---\ntitle: A Study\ndate: 2026-04-14\nbibliography: references.bib\n---\n\nSee [@k].\n\n# References\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("posts/p/index.tmd").unwrap();
        assert!(
            html.contains(r#""@type":"ScholarlyArticle""#),
            "a bibliography-bearing dated post is a ScholarlyArticle, not a BlogPosting"
        );
        assert!(
            !html.contains(r#""@type":"BlogPosting""#),
            "it must not also carry BlogPosting"
        );
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
        // The footer social link must land in the Person JSON-LD `sameAs`, not merely appear
        // somewhere on the page: the footer chrome renders the same URL, so a plain
        // page-contains check stayed green even when `sameAs` was dropped from the JSON-LD.
        assert!(
            html.contains(r#""sameAs":["https://github.com/x"]"#),
            "footer social link must appear in the Person JSON-LD sameAs"
        );
        // An undated page is og:type=website. Only the dated=article branch was pinned
        // (in corpus.rs), so an always-"article" regression on this branch went unnoticed.
        assert!(
            html.contains(r#"property="og:type" content="website""#),
            "an undated page must be og:type=website, not article"
        );
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
    fn a_book_chapter_gets_its_own_distinct_og_card_not_one_site_wide() {
        // C-PUB-1 (PMF audit): "the amateur tell is one site-wide card." A website page
        // getting its own card is pinned (corpus.rs, tech-blog posts), but a BOOK routes
        // its chapters through the same `render_page` via the `is_book()` chrome branch,
        // and nothing pinned that a book chapter still emits og:image/twitter:card there,
        // nor that each chapter's card is DISTINCT. A future divergence of the book head
        // path could drop `social_head` with every website test still green.
        let root = write_site(
            "bookcards",
            &[
                (
                    "_site.yml",
                    "title: A Book\nurl: https://ex.com\nchapters:\n  - index.tmd\n  - methods.tmd\n",
                ),
                ("index.tmd", "---\ntitle: Introduction\n---\n\nWelcome.\n"),
                ("methods.tmd", "---\ntitle: Methodology\n---\n\nHow.\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(site.is_book(), "chapters: makes this a book");

        // Pull the og:image URL out of a rendered chapter, asserting it carries the
        // large-image twitter card on the way (a card page is summary_large_image, never
        // the imageless `summary`).
        let og_image = |rel: &str| -> String {
            let html = site.render_page(rel).expect("chapter renders");
            assert!(
                html.contains(r#"name="twitter:card" content="summary_large_image""#),
                "{rel}: a book chapter with a card must be summary_large_image, not summary"
            );
            let key = r#"property="og:image" content=""#;
            let i = html
                .find(key)
                .unwrap_or_else(|| panic!("{rel}: a book chapter must emit og:image:\n{html}"));
            let rest = &html[i + key.len()..];
            rest[..rest.find('"').expect("closing quote")].to_string()
        };
        let intro = og_image("index.tmd");
        let methods = og_image("methods.tmd");
        assert!(
            intro.starts_with("https://ex.com/og/") && methods.starts_with("https://ex.com/og/"),
            "each chapter's og:image is the branded build card (got {intro} / {methods})"
        );
        // The load-bearing assertion: the two chapters do NOT share one site-wide card.
        assert_ne!(
            intro, methods,
            "each chapter must get its OWN card (distinct hash), not one site-wide image"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn deck_social_head_is_a_large_image_card_when_url_is_set_and_degrades_without() {
        // C-PUB-1 deck residual: an embedded deck (built off-`Page`) gets its own rich
        // social meta via `deck_social_head` — a branded og:image card + og:url +
        // summary_large_image when `url:` is set, degrading to a plain `summary` text card
        // (title/description only) when it is not, exactly like a page.
        let with = write_site(
            "decksocial",
            &[
                ("_site.yml", "title: Talks\nurl: https://ex.com\n"),
                ("index.tmd", "---\ntitle: H\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&with);
        let h = super::deck_social_head(
            &site,
            "talk.html",
            Some("The EM algorithm"),
            Some("A worked talk."),
        );
        assert!(
            h.contains(r#"property="og:title" content="The EM algorithm""#),
            "{h}"
        );
        assert!(
            h.contains(r#"property="og:type" content="website""#),
            "a deck is not a dated article: {h}"
        );
        assert!(
            h.contains(r#"property="og:url" content="https://ex.com/talk.html""#),
            "{h}"
        );
        assert!(
            h.contains(r#"name="twitter:card" content="summary_large_image""#),
            "{h}"
        );
        // og:image points at the deck's OWN branded card (same spec the build writes).
        let rel = crate::site::card_rel_path(&crate::site::deck_card_spec(
            &site,
            Some("The EM algorithm"),
            Some("A worked talk."),
        ));
        assert!(
            h.contains(&format!(
                r#"property="og:image" content="https://ex.com/{rel}""#
            )),
            "{h}"
        );
        let _ = std::fs::remove_dir_all(&with);

        // No `url:` → no absolute card is possible; degrade to a plain summary text card
        // (still carrying the deck's title/description), and emit no og:url.
        let without = write_site(
            "decksocialno",
            &[
                ("_site.yml", "title: Talks\n"),
                ("index.tmd", "---\ntitle: H\n---\n\nx\n"),
            ],
        );
        let site2 = Site::discover(&without);
        let h2 = super::deck_social_head(&site2, "talk.html", Some("T"), None);
        assert!(!h2.contains("og:image"), "no url -> no card image: {h2}");
        assert!(
            h2.contains(r#"name="twitter:card" content="summary""#),
            "degrades to summary: {h2}"
        );
        assert!(!h2.contains("og:url"), "no url -> no og:url: {h2}");
        let _ = std::fs::remove_dir_all(&without);
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

    #[test]
    fn jsonld_neutralizes_every_angle_bracket_not_just_closing_script() {
        // `</` alone is not enough: `<!--<script>` drives the HTML tokenizer into
        // script-data-double-escaped state, where the emitted `</script>` no longer
        // closes the element. The ld+json then swallows the rest of the document and
        // the page renders blank, with the build reporting success.
        let root = write_site(
            "jsonldescape",
            &[
                (
                    "_site.yml",
                    "title: B\nurl: https://ex.com\ndescription: \"before <!--<script> after\"\n",
                ),
                ("index.tmd", "---\ntitle: H\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        let start = html.find("application/ld+json").expect("ld+json present");
        let block = &html[start..];
        let end = block.find("</script>").expect("ld+json script closes");
        let payload = &block[..end];
        assert!(
            !payload.contains('<'),
            "no raw `<` may survive into the ld+json payload: {payload}"
        );
        assert!(
            payload.contains("\\u003c"),
            "the `<` must be escaped, not dropped: {payload}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
