//! Atom 1.0 syndication feeds, one per uncapped dated listing, plus the shared
//! absolute-URL and RFC-3339 date helpers the other discoverability artifacts reuse.
//! All gated on `config.url`. Pure functions of the discovered model; `build.rs`
//! writes the files.

use super::{Page, Site};
use crate::escape_attr as esc;
use std::collections::HashSet;

impl Site {
    /// The canonical origin (`config.url` with any trailing slash trimmed). A blank
    /// `url:` is treated as unset (it would yield relative artifact URLs).
    pub(crate) fn canonical_base(&self) -> Option<&str> {
        self.config
            .url
            .as_deref()
            .map(|u| u.trim_end_matches('/'))
            .filter(|u| !u.is_empty())
    }

    /// A page's absolute clean (directory) URL, e.g. `https://site/posts/x/`.
    /// An index page is served at its directory, matching the `og:url`/canonical
    /// logic in `meta.rs`. `None` without `url:`.
    pub(crate) fn abs_page_url(&self, page: &Page) -> Option<String> {
        let base = self.canonical_base()?;
        let clean = page.url.strip_suffix("index.html").unwrap_or(&page.url);
        Some(format!("{base}/{clean}"))
    }

    /// One Atom feed per **uncapped, dated** listing (`(relative_path, xml)`).
    /// Path = the listing page's url with `.html` → `.xml` (`blog.html` → `blog.xml`).
    /// A capped listing (a homepage teaser) and an undated listing get no feed.
    /// Empty without `url:`.
    pub fn atom_feeds(&self) -> Vec<(String, String)> {
        let Some(base) = self.canonical_base() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut sink = Vec::new(); // collection warnings are surfaced during page render
        // Dedupe by the RESOLVED listing prefix (not the raw `contents:` string) in nav
        // order, so a collection re-listed elsewhere (e.g. the CV's projects tail) does
        // not spawn a second feed, while two listings in different directories that share
        // a `contents:` value (distinct collections) each still get one.
        let mut seen: HashSet<String> = HashSet::new();
        for &(page, _) in &self.nav_ordered() {
            let Some(spec) = page.listings.iter().find(|sp| {
                sp.max_items.is_none() && !seen.contains(&Self::listing_prefix(page, sp))
            }) else {
                continue; // capped teaser, or its collection already fed → no feed
            };
            let dated: Vec<&Page> = self
                .collection(page, spec, &mut sink)
                .into_iter()
                .filter(|p| p.date.is_some())
                .collect();
            if dated.is_empty() {
                continue;
            }
            seen.insert(Self::listing_prefix(page, spec));
            let path = page.url.replace(".html", ".xml");
            out.push((path.clone(), self.build_atom(page, &dated, &path, base)));
        }
        out
    }

    /// Build the Atom XML for one listing's dated items (already newest-first).
    fn build_atom(&self, host: &Page, items: &[&Page], feed_path: &str, base: &str) -> String {
        let feed_url = format!("{base}/{feed_path}");
        let title = esc(host
            .title
            .as_deref()
            .or(self.config.title.as_deref())
            .unwrap_or("Feed"));
        // RFC 4287 requires an author at feed or entry level; fall back to the site
        // origin so the feed always carries a non-empty one.
        let author = self
            .config
            .author
            .as_ref()
            .and_then(|a| a.as_str())
            .or(self.config.title.as_deref())
            .filter(|a| !a.is_empty())
            .unwrap_or(base);
        // Feed `updated` = newest entry's date (items are already date-sorted desc, but
        // take the max defensively).
        let updated = items
            .iter()
            .filter_map(|p| p.date.as_deref())
            .filter_map(rfc3339)
            .max()
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
        let mut s = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
        s.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");
        s.push_str(&format!("  <title>{title}</title>\n"));
        s.push_str(&format!("  <id>{}</id>\n", esc(&feed_url)));
        s.push_str(&format!(
            "  <link rel=\"self\" href=\"{}\"/>\n",
            esc(&feed_url)
        ));
        s.push_str(&format!(
            "  <link rel=\"alternate\" href=\"{}\"/>\n",
            esc(&self.abs_page_url(host).unwrap_or_default())
        ));
        s.push_str(&format!("  <updated>{updated}</updated>\n"));
        if !author.is_empty() {
            s.push_str(&format!(
                "  <author><name>{}</name></author>\n",
                esc(author)
            ));
        }
        s.push_str("  <generator>Taliesin</generator>\n");
        for p in items.iter().copied() {
            let link = self.abs_page_url(p).unwrap_or_default();
            let when = p
                .date
                .as_deref()
                .and_then(rfc3339)
                .unwrap_or_else(|| updated.clone());
            s.push_str("  <entry>\n");
            s.push_str(&format!(
                "    <title>{}</title>\n",
                esc(p.title.as_deref().unwrap_or(""))
            ));
            s.push_str(&format!("    <id>{}</id>\n", esc(&link)));
            s.push_str(&format!(
                "    <link rel=\"alternate\" href=\"{}\"/>\n",
                esc(&link)
            ));
            s.push_str(&format!("    <updated>{when}</updated>\n"));
            s.push_str(&format!("    <published>{when}</published>\n"));
            for c in &p.categories {
                s.push_str(&format!("    <category term=\"{}\"/>\n", esc(c)));
            }
            if let Some(d) = p.description.as_deref() {
                s.push_str(&format!("    <summary>{}</summary>\n", esc(d)));
            }
            s.push_str("  </entry>\n");
        }
        s.push_str("</feed>\n");
        s
    }
}

/// An ISO date (`2026-05-15`) → RFC-3339 (`2026-05-15T00:00:00Z`). A value that
/// already carries a time is returned unchanged; anything that is not `YYYY-MM-DD`
/// (nor `…T…`) yields `None`.
pub(crate) fn rfc3339(date: &str) -> Option<String> {
    let d = date.trim();
    if d.contains('T') {
        return Some(d.to_string());
    }
    let parts: Vec<&str> = d.split('-').collect();
    if parts.len() == 3
        && parts[0].len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        Some(format!("{d}T00:00:00Z"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tests::write_site;

    #[test]
    fn rfc3339_normalizes_a_date_only_string() {
        assert_eq!(
            rfc3339("2026-05-15").as_deref(),
            Some("2026-05-15T00:00:00Z")
        );
        assert_eq!(
            rfc3339("2026-05-15T09:30:00Z").as_deref(),
            Some("2026-05-15T09:30:00Z")
        );
        assert_eq!(rfc3339("not-a-date"), None);
    }

    #[test]
    fn atom_feed_emitted_per_uncapped_dated_listing_with_absolute_links() {
        let root = write_site(
            "feedgen",
            &[
                ("_site.yml", "title: Blog\nurl: https://ex.com/\n"),
                (
                    "blog.tmd",
                    "---\ntitle: Blog\nlisting:\n  contents: posts\n  type: list\n---\n\n# Blog\n",
                ),
                (
                    "home.tmd",
                    "---\ntitle: Home\nlisting:\n  contents: posts\n  max-items: 2\n---\n\n# Home\n",
                ),
                (
                    "posts/a/index.tmd",
                    "---\ntitle: First Post\ndate: 2026-05-15\ndescription: A summary.\ncategories: [rust]\n---\n\nBody.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let feeds = site.atom_feeds();
        let paths: Vec<&str> = feeds.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"blog.xml"), "blog feed emitted: {paths:?}");
        assert!(
            !paths.iter().any(|p| p.starts_with("home")),
            "capped listing has no feed: {paths:?}"
        );
        let (_, xml) = feeds.iter().find(|(p, _)| p == "blog.xml").unwrap();
        assert!(
            xml.contains(r#"<feed xmlns="http://www.w3.org/2005/Atom">"#),
            "atom root: {xml}"
        );
        assert!(
            xml.contains("<title>First Post</title>"),
            "entry title: {xml}"
        );
        assert!(
            xml.contains(r#"href="https://ex.com/posts/a/""#),
            "absolute entry link: {xml}"
        );
        assert!(
            xml.contains("<summary>A summary.</summary>"),
            "summary from description: {xml}"
        );
        assert!(xml.contains("2026-05-15T00:00:00Z"), "rfc3339 date: {xml}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_feeds_without_url() {
        let root = write_site(
            "feednourl",
            &[
                ("_site.yml", "title: Blog\n"),
                (
                    "blog.tmd",
                    "---\ntitle: Blog\nlisting:\n  contents: posts\n---\n\n# Blog\n",
                ),
                ("posts/a.tmd", "---\ntitle: P\ndate: 2026-01-01\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(site.atom_feeds().is_empty(), "no url: → no feeds");
        let _ = std::fs::remove_dir_all(&root);
    }
}
