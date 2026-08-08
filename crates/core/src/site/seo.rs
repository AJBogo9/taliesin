//! `sitemap.xml` + `robots.txt` — crawler directives, url-gated. Pure functions of
//! the discovered model; `build.rs` writes the files.

use super::Site;
use crate::escape_attr as esc;

impl Site {
    /// `sitemap.xml`: one `<url>` per built HTML page (absolute clean `<loc>`,
    /// `<lastmod>` from `date:` when present). `None` without `url:`. Drafts are
    /// already excluded from `self.pages`.
    pub fn sitemap(&self) -> Option<String> {
        self.canonical_base()?;
        let mut s = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
        s.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        for p in &self.pages {
            if p.url == "404.html" {
                continue; // never sitemap the error page
            }
            let Some(loc) = self.abs_page_url(p) else {
                continue;
            };
            s.push_str("  <url>\n");
            s.push_str(&format!("    <loc>{}</loc>\n", esc(&loc)));
            // `<lastmod>` is W3C Datetime, so the raw `date:` cannot go in: `esc` is an XML
            // escaper, not a date normalizer, and a human `date: May 15, 2026` used to ship
            // verbatim into a machine-read file. A date we cannot parse omits `<lastmod>`
            // (and warns at its front-matter line) — the `<url>` stays, since the page
            // exists and only its date is unknown.
            //
            // Not `feed::rfc3339`: sitemaps.org takes the date-only W3C form, and we do not
            // know a time — the feed's `T00:00:00Z` is there because Atom REQUIRES a full
            // timestamp, a constraint that does not travel here. The two share the
            // validator, not the format.
            if let Some((y, m, d)) = p
                .date
                .as_deref()
                .and_then(crate::frontmatter::calendar_date)
            {
                s.push_str(&format!("    <lastmod>{y:04}-{m:02}-{d:02}</lastmod>\n"));
            }
            s.push_str("  </url>\n");
        }
        s.push_str("</urlset>\n");
        Some(s)
    }

    /// `robots.txt`: allow-all (welcomes AI crawlers) + the sitemap reference.
    /// `None` without `url:`.
    pub fn robots(&self) -> Option<String> {
        let base = self.canonical_base()?;
        Some(format!(
            "User-agent: *\nAllow: /\nSitemap: {base}/sitemap.xml\n"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tests::write_site;

    #[test]
    fn sitemap_lists_pages_with_absolute_locs_and_lastmod() {
        let root = write_site(
            "sitemapgen",
            &[
                ("_site.yml", "title: S\nurl: https://ex.com\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nHi.\n"),
                (
                    "posts/a/index.tmd",
                    "---\ntitle: P\ndate: 2026-05-15\n---\n\nx\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let sm = site.sitemap().expect("sitemap emitted with url:");
        assert!(sm.contains(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#));
        assert!(
            sm.contains("<loc>https://ex.com/</loc>"),
            "home clean loc: {sm}"
        );
        assert!(
            sm.contains("<loc>https://ex.com/posts/a/</loc>"),
            "post loc: {sm}"
        );
        assert!(
            sm.contains("<lastmod>2026-05-15</lastmod>"),
            "post lastmod: {sm}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `<lastmod>` is read by machines and must be W3C Datetime, but it used to be
    /// `esc(date)` — an XML escaper, not a date normalizer — so whatever the author typed
    /// shipped verbatim under a green `check`. A date we cannot parse now omits `<lastmod>`
    /// rather than emitting a lie; the `<url>` itself stays listed, since the page exists
    /// and only its date is unknown.
    #[test]
    fn sitemap_lastmod_normalizes_or_is_omitted_never_invented() {
        let root = write_site(
            "sitemaplastmod",
            &[
                ("_site.yml", "title: S\nurl: https://ex.com\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nHi.\n"),
                (
                    "posts/human/index.tmd",
                    "---\ntitle: H\ndate: \"May 15, 2026\"\n---\n\nx\n",
                ),
                (
                    "posts/unpadded/index.tmd",
                    "---\ntitle: U\ndate: 2026-5-15\n---\n\nx\n",
                ),
                (
                    "posts/timed/index.tmd",
                    "---\ntitle: T\ndate: \"2026-05-15T09:30:00Z\"\n---\n\nx\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let sm = site.sitemap().expect("sitemap emitted with url:");
        // The `<lastmod>` of one `<url>`, by its path — asserting on the whole document
        // would let a date land under the wrong page and still pass.
        let lastmod_of = |path: &str| -> Option<String> {
            let (_, after) = sm.split_once(&format!("<loc>https://ex.com{path}</loc>"))?;
            let entry = after.split("</url>").next()?;
            let (_, m) = entry.split_once("<lastmod>")?;
            Some(m.split("</lastmod>").next()?.to_string())
        };
        assert!(
            !sm.contains("May 15, 2026"),
            "a human date must never reach <lastmod>: {sm}"
        );
        assert_eq!(
            lastmod_of("/posts/human/"),
            None,
            "an unparseable date omits <lastmod> rather than inventing one: {sm}"
        );
        // Un-padded normalizes (it names one unambiguous day), and a timestamped date
        // keeps only its calendar half: `<lastmod>` is a date, and we know no time.
        assert_eq!(
            lastmod_of("/posts/unpadded/").as_deref(),
            Some("2026-05-15")
        );
        assert_eq!(lastmod_of("/posts/timed/").as_deref(), Some("2026-05-15"));
        // Every page is still listed, datable or not.
        for loc in ["/posts/human/", "/posts/unpadded/", "/posts/timed/"] {
            assert!(
                sm.contains(&format!("<loc>https://ex.com{loc}</loc>")),
                "{loc} still sitemapped: {sm}"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn absolute_urls_percent_encode_an_unsafe_path_segment() {
        // A page directory can carry a space (pages are discovered from the filesystem,
        // never slugified). The shared absolute URL must be percent-encoded, or the
        // sitemap `<loc>` ships a raw space (an invalid sitemap URL — `esc` is an XML
        // escaper, not a URL escaper) and llms.txt ships a broken CommonMark link.
        let root = write_site(
            "seoencode",
            &[
                ("_site.yml", "title: S\nurl: https://ex.com\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nHi.\n"),
                (
                    "posts/two words/index.tmd",
                    "---\ntitle: Two Words\ndate: 2026-05-15\n---\n\nx\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let sm = site.sitemap().expect("sitemap emitted with url:");
        assert!(
            sm.contains("<loc>https://ex.com/posts/two%20words/</loc>"),
            "the space must be percent-encoded in <loc>: {sm}"
        );
        assert!(
            !sm.contains("two words"),
            "no raw space may reach the sitemap XML: {sm}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn robots_allows_all_and_points_at_the_sitemap() {
        let root = write_site(
            "robotsgen",
            &[
                ("_site.yml", "title: S\nurl: https://ex.com/\n"),
                ("index.tmd", "---\ntitle: H\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let r = site.robots().expect("robots emitted with url:");
        assert!(r.contains("User-agent: *"));
        assert!(r.contains("Allow: /"));
        assert!(
            r.contains("Sitemap: https://ex.com/sitemap.xml"),
            "sitemap ref: {r}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn none_without_url() {
        let root = write_site(
            "seonourl",
            &[
                ("_site.yml", "title: S\n"),
                ("index.tmd", "---\ntitle: H\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(site.sitemap().is_none());
        assert!(site.robots().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
