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
            if let Some(d) = p.date.as_deref() {
                s.push_str(&format!("    <lastmod>{}</lastmod>\n", esc(d)));
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
