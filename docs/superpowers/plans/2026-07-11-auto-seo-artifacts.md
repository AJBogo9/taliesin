# Auto SEO + discoverability artifacts — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a site sets `url:`, `taliesin build <dir>` additionally emits Atom feeds, `sitemap.xml`, `robots.txt`, `llms.txt`, `llms-full.txt`, and per-page JSON-LD, all auto-derived from existing page/config data (zero SEO effort for the author).

**Architecture:** New pure `Site` methods (`crates/core/src/site/`) compute each artifact string from the already-discovered model; `build.rs` writes them in the aux-file zone and adds them to the stale-sweep `keep` set. JSON-LD rides the existing per-page head path (`meta.rs`), so it also appears in live preview. Everything is gated on `config.url`.

**Tech Stack:** Rust (edition 2024), `serde_json` (already a workspace dep) for JSON-LD, the existing `escape_attr`/`html_escape` helpers for XML, `Site`/`Page`/`SiteConfig` model.

## Global Constraints

- **Gate:** every artifact is emitted only when `config.url.is_some()`. No new config key.
- **Invariants:** offline/zero-CDN (static same-origin files only); single-editing-surface (read-only derivations, no write-back); HTML-only output (these are *sidecars* like `search-index.js`, not a new document format); minimal-config.
- **Corpus is the regression net:** every corpus assertion matches an emitted **string** (feed XML / file content), never an inlined-CSS/JS substring (the "gate the gate" lesson: a test that can't fail is worse than none).
- **Edition 2024, `cargo fmt`-clean** (a PostToolUse hook runs rustfmt; CI enforces it).
- **Draft pages are already excluded** at discovery (`discovery.rs:19-21`), so `self.pages` is draft-free — no per-artifact draft filtering needed.
- Spec: `docs/superpowers/specs/2026-07-11-auto-seo-artifacts-design.md`.

---

### Task 1: Atom feeds + shared URL/date helpers (`feed.rs`)

**Files:**
- Create: `crates/core/src/site/feed.rs`
- Modify: `crates/core/src/site/mod.rs` (add `mod feed;` and re-export nothing — methods hang off `impl Site`)
- Test: unit tests inside `feed.rs`

**Interfaces:**
- Produces (used by Tasks 2-5):
  - `Site::canonical_base(&self) -> Option<&str>` — `config.url` minus a trailing slash.
  - `Site::abs_page_url(&self, page: &Page) -> Option<String>` — absolute clean (directory) URL of a page.
  - `Site::atom_feeds(&self) -> Vec<(String, String)>` — `(relative_path, xml)` per uncapped dated listing.
  - `feed::rfc3339(date: &str) -> Option<String>` — ISO date → RFC-3339.

- [ ] **Step 1: Register the module.** In `crates/core/src/site/mod.rs`, add `mod feed;` beside the other `mod` declarations (near `mod meta;` / `mod chrome;`).

- [ ] **Step 2: Write the failing tests** in `crates/core/src/site/feed.rs`:

```rust
//! Atom 1.0 syndication feeds, one per uncapped dated listing, plus the shared
//! absolute-URL and RFC-3339 date helpers the other discoverability artifacts reuse.
//! All gated on `config.url`. Pure functions of the discovered model; `build.rs`
//! writes the files.

use super::{Page, Site};
use crate::escape_attr as esc;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tests::write_site;

    #[test]
    fn rfc3339_normalizes_a_date_only_string() {
        assert_eq!(rfc3339("2026-05-15").as_deref(), Some("2026-05-15T00:00:00Z"));
        assert_eq!(rfc3339("2026-05-15T09:30:00Z").as_deref(), Some("2026-05-15T09:30:00Z"));
        assert_eq!(rfc3339("not-a-date"), None);
    }

    #[test]
    fn atom_feed_emitted_per_uncapped_dated_listing_with_absolute_links() {
        let root = write_site(
            "feedgen",
            &[
                ("_site.yml", "title: Blog\nurl: https://ex.com/\n"),
                ("blog.tmd", "---\ntitle: Blog\nlisting:\n  contents: posts\n  type: list\n---\n\n# Blog\n"),
                ("home.tmd", "---\ntitle: Home\nlisting:\n  contents: posts\n  max-items: 2\n---\n\n# Home\n"),
                ("posts/a.tmd", "---\ntitle: First Post\ndate: 2026-05-15\ndescription: A summary.\ncategories: [rust]\n---\n\nBody.\n"),
            ],
        );
        let site = Site::discover(&root);
        let feeds = site.atom_feeds();
        // One feed for the uncapped blog listing; the capped `home` teaser gets none.
        let paths: Vec<&str> = feeds.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"blog.xml"), "blog feed emitted: {paths:?}");
        assert!(!paths.iter().any(|p| p.starts_with("home")), "capped listing has no feed: {paths:?}");
        let (_, xml) = feeds.iter().find(|(p, _)| p == "blog.xml").unwrap();
        assert!(xml.contains(r#"<feed xmlns="http://www.w3.org/2005/Atom">"#), "atom root: {xml}");
        assert!(xml.contains("<title>First Post</title>"), "entry title: {xml}");
        assert!(xml.contains(r#"href="https://ex.com/posts/a/""#), "absolute entry link: {xml}");
        assert!(xml.contains("<summary>A summary.</summary>"), "summary from description: {xml}");
        assert!(xml.contains("2026-05-15T00:00:00Z"), "rfc3339 date: {xml}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_feeds_without_url() {
        let root = write_site(
            "feednourl",
            &[
                ("_site.yml", "title: Blog\n"),
                ("blog.tmd", "---\ntitle: Blog\nlisting:\n  contents: posts\n---\n\n# Blog\n"),
                ("posts/a.tmd", "---\ntitle: P\ndate: 2026-01-01\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(site.atom_feeds().is_empty(), "no url: → no feeds");
        let _ = std::fs::remove_dir_all(&root);
    }
}
```

Note: `write_site` is the existing test helper in `mod.rs`'s `tests` module. Confirm it is `pub(crate)` or `pub(super)`; if it is only `pub(self)`/private to the `tests` module, add `pub(crate)` to its definition so `feed.rs`'s tests can call `crate::site::tests::write_site`.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p taliesin-core --lib site::feed 2>&1 | tail -20`
Expected: FAIL to compile ("cannot find function `rfc3339`" / "no method `atom_feeds`").

- [ ] **Step 4: Implement the helpers + feed builder** in `feed.rs` (above the `#[cfg(test)]` block):

```rust
impl Site {
    /// The canonical origin (`config.url` with any trailing slash trimmed).
    pub(crate) fn canonical_base(&self) -> Option<&str> {
        self.config.url.as_deref().map(|u| u.trim_end_matches('/'))
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
    pub(crate) fn atom_feeds(&self) -> Vec<(String, String)> {
        let Some(base) = self.canonical_base() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut sink = Vec::new(); // collection warnings are surfaced elsewhere
        for page in &self.pages {
            for spec in &page.listings {
                if spec.max_items.is_some() {
                    continue; // capped teaser → not a feed source
                }
                let dated: Vec<&Page> = self
                    .collection(page, spec, &mut sink)
                    .into_iter()
                    .filter(|p| p.date.is_some())
                    .collect();
                if dated.is_empty() {
                    continue;
                }
                let path = page.url.replace(".html", ".xml");
                out.push((path.clone(), self.build_atom(page, &dated, &path, base)));
            }
        }
        out
    }

    fn build_atom(&self, host: &Page, items: &[&Page], feed_path: &str, base: &str) -> String {
        let feed_url = format!("{base}/{feed_path}");
        let title = esc(host.title.as_deref().or(self.config.title.as_deref()).unwrap_or("Feed"));
        let author = self
            .config
            .author
            .as_ref()
            .and_then(|a| a.as_str())
            .or(self.config.title.as_deref())
            .unwrap_or("");
        // Feed `updated` = newest entry's date (items are already date-sorted).
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
        s.push_str(&format!("  <link rel=\"self\" href=\"{}\"/>\n", esc(&feed_url)));
        s.push_str(&format!("  <link rel=\"alternate\" href=\"{}\"/>\n", esc(&self.abs_page_url(host).unwrap_or_default())));
        s.push_str(&format!("  <updated>{updated}</updated>\n"));
        if !author.is_empty() {
            s.push_str(&format!("  <author><name>{}</name></author>\n", esc(author)));
        }
        s.push_str("  <generator>Taliesin</generator>\n");
        for p in items {
            let link = self.abs_page_url(p).unwrap_or_default();
            let when = p.date.as_deref().and_then(rfc3339).unwrap_or_else(|| updated.clone());
            s.push_str("  <entry>\n");
            s.push_str(&format!("    <title>{}</title>\n", esc(p.title.as_deref().unwrap_or(""))));
            s.push_str(&format!("    <id>{}</id>\n", esc(&link)));
            s.push_str(&format!("    <link rel=\"alternate\" href=\"{}\"/>\n", esc(&link)));
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
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        Some(format!("{d}T00:00:00Z"))
    } else {
        None
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p taliesin-core --lib site::feed 2>&1 | tail -20`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/site/feed.rs crates/core/src/site/mod.rs
git commit -m "feat(site): Atom feed generation per dated listing (url-gated)"
```

---

### Task 2: sitemap.xml + robots.txt (`seo.rs`)

**Files:**
- Create: `crates/core/src/site/seo.rs`
- Modify: `crates/core/src/site/mod.rs` (add `mod seo;`)
- Test: unit tests inside `seo.rs`

**Interfaces:**
- Consumes: `Site::canonical_base`, `Site::abs_page_url` (Task 1), `feed::rfc3339` — sitemap `lastmod` uses the raw ISO date (no time), so `rfc3339` is not needed here.
- Produces: `Site::sitemap(&self) -> Option<String>`, `Site::robots(&self) -> Option<String>`.

- [ ] **Step 1: Register the module.** Add `mod seo;` in `mod.rs`.

- [ ] **Step 2: Write the failing tests** in `crates/core/src/site/seo.rs`:

```rust
//! `sitemap.xml` + `robots.txt` — crawler directives, url-gated. Pure functions of
//! the discovered model.

use super::Site;
use crate::escape_attr as esc;

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
                ("posts/a.tmd", "---\ntitle: P\ndate: 2026-05-15\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let sm = site.sitemap().expect("sitemap emitted with url:");
        assert!(sm.contains(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#));
        assert!(sm.contains("<loc>https://ex.com/</loc>"), "home clean loc: {sm}");
        assert!(sm.contains("<loc>https://ex.com/posts/a/</loc>"), "post loc: {sm}");
        assert!(sm.contains("<lastmod>2026-05-15</lastmod>"), "post lastmod: {sm}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn robots_allows_all_and_points_at_the_sitemap() {
        let root = write_site(
            "robotsgen",
            &[("_site.yml", "title: S\nurl: https://ex.com/\n"), ("index.tmd", "---\ntitle: H\n---\n\nx\n")],
        );
        let site = Site::discover(&root);
        let r = site.robots().expect("robots emitted with url:");
        assert!(r.contains("User-agent: *"));
        assert!(r.contains("Allow: /"));
        assert!(r.contains("Sitemap: https://ex.com/sitemap.xml"), "sitemap ref: {r}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn none_without_url() {
        let root = write_site("seonourl", &[("_site.yml", "title: S\n"), ("index.tmd", "---\ntitle: H\n---\n\nx\n")]);
        let site = Site::discover(&root);
        assert!(site.sitemap().is_none());
        assert!(site.robots().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
```

- [ ] **Step 3: Run to verify fail**

Run: `cargo test -p taliesin-core --lib site::seo 2>&1 | tail -20`
Expected: FAIL to compile ("no method `sitemap`").

- [ ] **Step 4: Implement** in `seo.rs` (above the test module):

```rust
impl Site {
    /// `sitemap.xml`: one `<url>` per built HTML page (absolute clean `<loc>`,
    /// `<lastmod>` from `date:` when present). `None` without `url:`. Drafts are
    /// already excluded from `self.pages`.
    pub(crate) fn sitemap(&self) -> Option<String> {
        self.canonical_base()?;
        let mut s = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
        s.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        for p in &self.pages {
            let Some(loc) = self.abs_page_url(p) else { continue };
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
    pub(crate) fn robots(&self) -> Option<String> {
        let base = self.canonical_base()?;
        Some(format!("User-agent: *\nAllow: /\nSitemap: {base}/sitemap.xml\n"))
    }
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p taliesin-core --lib site::seo 2>&1 | tail -20`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/site/seo.rs crates/core/src/site/mod.rs
git commit -m "feat(site): sitemap.xml + robots.txt generation (url-gated)"
```

---

### Task 3: llms.txt + llms-full.txt (`llms.rs`)

**Files:**
- Create: `crates/core/src/site/llms.rs`
- Modify: `crates/core/src/site/mod.rs` (add `mod llms;`)
- Test: unit tests inside `llms.rs`

**Interfaces:**
- Consumes: `Site::canonical_base`, `Site::abs_page_url` (Task 1); `render::render_document_with_includes_scoped` (as `render_page` uses at `mod.rs:508`); `Page.hero`, `Page.listings`, `Page.description`, `Page.title`, `Page.cell` (on blocks).
- Produces: `Site::llms_txt(&self) -> Option<String>`, `Site::llms_full_txt(&self) -> Option<String>`.

- [ ] **Step 1: Register the module.** Add `mod llms;` in `mod.rs`.

- [ ] **Step 2: Write the failing tests** in `crates/core/src/site/llms.rs`:

```rust
//! `llms.txt` (a curated Markdown map — identity + linked page lists) and
//! `llms-full.txt` (every non-draft page's clean prose), so an assistant can answer
//! "who is this and what do they do?" from content the author wrote for humans.
//! Url-gated. The identity header is auto-derived from the home page's `hero:`.

use super::{Page, Site};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tests::write_site;

    #[test]
    fn llms_txt_leads_with_hero_identity_and_lists_posts() {
        let root = write_site(
            "llmsmap",
            &[
                ("_site.yml", "title: Andreas Bogossian\ndescription: ML from first principles\nurl: https://ex.com\n"),
                ("index.tmd", "---\ntitle: Andreas Bogossian\nhero:\n  eyebrow: ML\n  headline: Machine learning, from the math up\n  lead: I build systems at the intersection of math and software.\nlisting:\n  contents: posts\n  max-items: 2\n---\n\n## Recent\n"),
                ("blog.tmd", "---\ntitle: Blog\nlisting:\n  contents: posts\n  type: list\n---\n\n# Blog\n"),
                ("posts/a.tmd", "---\ntitle: First Post\ndate: 2026-05-15\ndescription: A summary of the post.\n---\n\nBody.\n"),
            ],
        );
        let site = Site::discover(&root);
        let txt = site.llms_txt().expect("llms.txt with url:");
        assert!(txt.starts_with("# Andreas Bogossian"), "H1 = site title: {txt}");
        assert!(txt.contains("> ML from first principles"), "tagline blockquote: {txt}");
        assert!(txt.contains("Machine learning, from the math up"), "hero headline in About: {txt}");
        assert!(txt.contains("intersection of math and software"), "hero lead in About: {txt}");
        assert!(
            txt.contains("[First Post](https://ex.com/posts/a/): A summary of the post."),
            "post listed with absolute link + description: {txt}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn llms_full_has_prose_skips_code_and_excludes_drafts() {
        let root = write_site(
            "llmsfull",
            &[
                ("_site.yml", "title: S\nurl: https://ex.com\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nThe intro prose paragraph.\n\n```python\nsecret_code_token = 1\n```\n"),
                ("posts/draft.tmd", "---\ntitle: WIP\ndraft: true\n---\n\nHidden draft prose.\n"),
            ],
        );
        let site = Site::discover(&root);
        let full = site.llms_full_txt().expect("llms-full.txt with url:");
        assert!(full.contains("The intro prose paragraph."), "prose kept: {full}");
        assert!(!full.contains("secret_code_token"), "code cell skipped: {full}");
        assert!(!full.contains("Hidden draft prose"), "draft excluded: {full}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn text_content_strips_tags_and_decodes_entities() {
        assert_eq!(text_content("<p>Two &amp; three &lt; four</p>"), "Two & three < four");
        assert_eq!(text_content("<h2>A&nbsp;B</h2>"), "A B");
    }

    #[test]
    fn none_without_url() {
        let root = write_site("llmsnourl", &[("_site.yml", "title: S\n"), ("index.tmd", "---\ntitle: H\n---\n\nx\n")]);
        let site = Site::discover(&root);
        assert!(site.llms_txt().is_none());
        assert!(site.llms_full_txt().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
```

- [ ] **Step 3: Run to verify fail**

Run: `cargo test -p taliesin-core --lib site::llms 2>&1 | tail -20`
Expected: FAIL to compile.

- [ ] **Step 4: Implement** in `llms.rs` (above the test module). The identity header composes from the home hero; prose extraction renders each page and keeps non-code blocks:

```rust
use crate::render;

impl Site {
    /// The home (root `index`) page, if any.
    fn home_page(&self) -> Option<&Page> {
        self.pages.iter().find(|p| p.url == "index.html")
    }

    /// The identity header shared by both llms files: `# title`, a `> tagline`
    /// blockquote (description → hero eyebrow → hero headline), and an About
    /// paragraph from the home hero (headline + lead). Each part omitted if absent.
    fn llms_header(&self) -> String {
        let title = self.config.title.as_deref().unwrap_or("Site");
        let mut s = format!("# {title}\n");
        let hero = self.home_page().and_then(|p| p.hero.as_ref());
        let tagline = self
            .config
            .description
            .as_deref()
            .or_else(|| hero.and_then(|h| h.eyebrow.as_deref()))
            .or_else(|| hero.and_then(|h| h.headline.as_deref()));
        if let Some(t) = tagline {
            s.push_str(&format!("\n> {t}\n"));
        }
        if let Some(h) = hero {
            let about: Vec<&str> = [h.headline.as_deref(), h.lead.as_deref()]
                .into_iter()
                .flatten()
                .collect();
            if !about.is_empty() {
                s.push_str(&format!("\n{}\n", about.join(" ")));
            }
        }
        s
    }

    /// `llms.txt`: the curated map. Header + one section per dated listing (named
    /// after its page title) + a "Pages" section for remaining top-level nav pages.
    /// `None` without `url:`.
    pub(crate) fn llms_txt(&self) -> Option<String> {
        self.canonical_base()?;
        let mut s = self.llms_header();
        let mut sink = Vec::new();
        let mut listed: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for page in &self.pages {
            for spec in &page.listings {
                let items: Vec<&Page> = self.collection(page, spec, &mut sink);
                if items.is_empty() {
                    continue;
                }
                let section = page.title.as_deref().unwrap_or("Posts");
                s.push_str(&format!("\n## {section}\n"));
                for p in &items {
                    listed.insert(p.rel.as_str());
                    let link = self.abs_page_url(p).unwrap_or_default();
                    let title = p.title.as_deref().unwrap_or(&p.rel);
                    match p.description.as_deref() {
                        Some(d) => s.push_str(&format!("- [{title}]({link}): {d}\n")),
                        None => s.push_str(&format!("- [{title}]({link})\n")),
                    }
                }
            }
        }
        // Remaining top-level nav pages (CV, Publications, …) that no listing covered.
        let mut pages_section = String::new();
        for item in self.config.nav.items.iter() {
            let Some(href) = item.href.as_deref() else { continue };
            let rel = href.trim_end_matches(".tmd");
            if let Some(p) = self.pages.iter().find(|p| p.rel.trim_end_matches(".tmd").trim_end_matches("/index") == rel && !listed.contains(p.rel.as_str())) {
                let link = self.abs_page_url(p).unwrap_or_default();
                let label = item.text.as_deref().or(p.title.as_deref()).unwrap_or(rel);
                match p.description.as_deref() {
                    Some(d) => pages_section.push_str(&format!("- [{label}]({link}): {d}\n")),
                    None => pages_section.push_str(&format!("- [{label}]({link})\n")),
                }
            }
        }
        if !pages_section.is_empty() {
            s.push_str("\n## Pages\n");
            s.push_str(&pages_section);
        }
        Some(s)
    }

    /// `llms-full.txt`: the identity header, then every page's title + absolute URL +
    /// clean prose (code cells and math excluded). `None` without `url:`.
    pub(crate) fn llms_full_txt(&self) -> Option<String> {
        self.canonical_base()?;
        let mut s = self.llms_header();
        for page in &self.pages {
            let Some(url) = self.abs_page_url(page) else { continue };
            let title = page.title.as_deref().unwrap_or(&page.rel);
            let prose = self.page_prose(page);
            if prose.trim().is_empty() {
                continue;
            }
            s.push_str(&format!("\n---\n\n## {title}\n{url}\n\n{prose}\n"));
        }
        Some(s)
    }

    /// Render a page to its block model and extract readable prose: skip executable
    /// code cells (`block.cell.is_some()`) and math regions; strip tags + decode
    /// entities on the rest. Re-renders without execution (cell outputs aren't prose).
    fn page_prose(&self, page: &Page) -> String {
        let Ok(src) = std::fs::read_to_string(&page.input) else {
            return String::new();
        };
        let base = page.input.parent().unwrap_or(&self.root);
        let doc = render::render_document_with_includes_scoped(&src, base, self.chapter_for(page));
        let mut parts = Vec::new();
        for b in &doc.blocks {
            if b.cell.is_some() {
                continue; // executable code cell → not prose
            }
            let html = strip_katex(&b.html);
            if html.trim_start().starts_with("<pre") {
                continue; // a non-cell code block
            }
            let t = text_content(&html);
            if !t.is_empty() {
                parts.push(t);
            }
        }
        parts.join("\n\n")
    }
}

/// Remove balanced `<span class="katex-display">…</span>` and `<span class="katex">…</span>`
/// regions so math produces no duplicated/garbled text (v1 omits math). Accounts for
/// nested `<span>`s via depth matching.
fn strip_katex(html: &str) -> String {
    let mut out = html.to_string();
    for marker in [r#"<span class="katex-display">"#, r#"<span class="katex">"#] {
        while let Some(start) = out.find(marker) {
            let after = start + marker.len();
            let mut depth = 1usize;
            let mut i = after;
            let bytes = out.as_bytes();
            while i < out.len() && depth > 0 {
                if out[i..].starts_with("<span") {
                    depth += 1;
                    i += 5;
                } else if out[i..].starts_with("</span>") {
                    depth -= 1;
                    i += 7;
                } else {
                    // advance one char (utf-8 safe)
                    i += (1..=4).find(|n| out.is_char_boundary(i + n)).unwrap_or(1);
                }
            }
            let _ = bytes;
            out.replace_range(start..i.min(out.len()), " ");
        }
    }
    out
}

/// Visible text of an HTML fragment: strip tags, decode the common entities, and
/// collapse whitespace. (`a11y::strip_tags` strips tags but does not decode entities.)
pub(crate) fn text_content(html: &str) -> String {
    let mut no_tags = String::with_capacity(html.len());
    let mut depth = 0u32;
    for ch in html.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if depth == 0 => no_tags.push(c),
            _ => {}
        }
    }
    let decoded = no_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#8217;", "\u{2019}");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}
```

Note on `self.config.nav.items`: confirm the navbar field name in `crates/core/src/site/config/mod.rs` (`Navbar` struct). If the field is not `items`, adjust the loop accessor to the real one (e.g. `self.config.nav.<field>`). If a titleless helper is needed, the `Page.title` fallback already handles it.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p taliesin-core --lib site::llms 2>&1 | tail -20`
Expected: PASS (4 tests). If `&amp;`-decode ordering double-decodes (it must decode `&amp;` in a way that does not re-trigger), verify the test `text_content_strips_tags_and_decodes_entities` passes as written; the ordering above decodes each entity once over the tag-stripped string, which is correct.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/site/llms.rs crates/core/src/site/mod.rs
git commit -m "feat(site): llms.txt + llms-full.txt (identity auto-derived from the home hero)"
```

---

### Task 4: JSON-LD structured data (`meta.rs::jsonld_head`)

**Files:**
- Modify: `crates/core/src/site/meta.rs` (add `jsonld_head`)
- Modify: `crates/core/src/site/mod.rs:445` (wire it into the head, right after `social_head`)
- Test: unit tests inside `meta.rs`

**Interfaces:**
- Consumes: `Site::canonical_base`, `Site::abs_page_url` (Task 1); `serde_json` (`json!`, `Value`); `config.footer` (`Footer{left,center,right: Vec<NavItem>}`), `config.author`, `config.title`, `Page.date`, `Page.title`, `Page.description`, `Page.card_image`.
- Produces: `meta::jsonld_head(site: &Site, page: &Page) -> String`.

- [ ] **Step 1: Write the failing tests** in `crates/core/src/site/meta.rs` (add to its existing `#[cfg(test)] mod tests`, or create one):

```rust
#[cfg(test)]
mod jsonld_tests {
    use crate::site::{tests::write_site, Site};

    #[test]
    fn post_emits_blogposting() {
        let root = write_site(
            "jsonldpost",
            &[
                ("_site.yml", "title: Blog\nurl: https://ex.com\n"),
                ("posts/a.tmd", "---\ntitle: My Post\ndate: 2026-05-15\ndescription: About things.\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("posts/a.tmd").unwrap();
        assert!(html.contains(r#""@type":"BlogPosting""#), "BlogPosting: {}", &html[..html.len().min(3000)]);
        assert!(html.contains(r#""headline":"My Post""#));
        assert!(html.contains(r#""datePublished":"2026-05-15""#));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn home_emits_website_and_person() {
        let root = write_site(
            "jsonldhome",
            &[
                ("_site.yml", "title: Andreas Bogossian\nurl: https://ex.com\nfooter:\n  right:\n    - { icon: github, href: https://github.com/x }\n"),
                ("index.tmd", "---\ntitle: Andreas Bogossian\n---\n\nHi.\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        assert!(html.contains(r#""@type":"WebSite""#), "WebSite present");
        assert!(html.contains(r#""@type":"Person""#), "Person present");
        assert!(html.contains(r#""name":"Andreas Bogossian""#), "person name = title fallback");
        assert!(html.contains("https://github.com/x"), "sameAs from footer");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_jsonld_without_url() {
        let root = write_site("jsonldnourl", &[("_site.yml", "title: B\n"), ("index.tmd", "---\ntitle: H\n---\n\nx\n")]);
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        assert!(!html.contains("application/ld+json"), "no JSON-LD without url:");
        let _ = std::fs::remove_dir_all(&root);
    }
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p taliesin-core --lib site::meta::jsonld 2>&1 | tail -20`
Expected: FAIL (JSON-LD absent from the rendered head).

- [ ] **Step 3: Implement `jsonld_head`** in `meta.rs`:

```rust
use serde_json::{json, Value};

/// schema.org JSON-LD for `page`, url-gated: a post (`date:` present) → `BlogPosting`;
/// the root index page → a `WebSite` + `Person` `@graph`. Empty otherwise.
pub(super) fn jsonld_head(site: &Site, page: &Site_Page_alias) -> String {
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
        let mut bp = json!({
            "@context": "https://schema.org",
            "@type": "BlogPosting",
            "headline": page.title.as_deref().unwrap_or(""),
            "datePublished": page.date.as_deref().unwrap_or(""),
            "dateModified": page.date.as_deref().unwrap_or(""),
            "mainEntityOfPage": site.abs_page_url(page).unwrap_or_default(),
            "url": site.abs_page_url(page).unwrap_or_default(),
        });
        if !author.is_empty() {
            bp["author"] = json!({ "@type": "Person", "name": author });
        }
        if let Some(d) = page.description.as_deref() {
            bp["description"] = json!(d);
        }
        if let Some(img) = page.card_image.as_deref() {
            let abs = if img.starts_with("http") { img.to_string() } else { format!("{base}/{}", img.trim_start_matches('/')) };
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
        Some(v) => format!("\n<script type=\"application/ld+json\">{}</script>", v),
        None => String::new(),
    }
}

/// Absolute social URLs from footer items that carry an `icon:` (the `sameAs` set).
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
```

Fix the two placeholders when implementing: replace `Site_Page_alias` with the real `Page` type (import it: `use super::{Page, Site}` — `meta.rs` already has `use super::{Page, Site, is_external_or_special};`, so the signature is `page: &Page`). `serde_json::Value`'s `Display` (`{}`) emits compact, correctly-escaped JSON — safe to drop inside the `<script>` (it contains no `</script>` because all string values are JSON-escaped and schema text has no literal `</script>`).

- [ ] **Step 4: Wire it into the head.** In `crates/core/src/site/mod.rs`, right after line 445 (`includes.in_header.push_str(&meta::social_head(self, page));`), add:

```rust
        includes.in_header.push_str(&meta::jsonld_head(self, page));
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p taliesin-core --lib site::meta 2>&1 | tail -20`
Expected: PASS (3 new tests + existing meta tests).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/site/meta.rs crates/core/src/site/mod.rs
git commit -m "feat(site): BlogPosting/WebSite/Person JSON-LD in the page head (url-gated)"
```

---

### Task 5: Wire the build to write the artifacts (`build.rs`)

**Files:**
- Modify: `crates/server/src/build.rs` (aux-file zone ~1104-1156)
- Test: covered by the corpus pins in Task 7 (a build-integration behavior; the unit layer is Tasks 1-3).

**Interfaces:**
- Consumes: `Site::atom_feeds`, `Site::sitemap`, `Site::robots`, `Site::llms_txt`, `Site::llms_full_txt`. These are `pub(crate)`; `build.rs` is in a *different crate* (`taliesin-server`), so they must be reachable. **Make each of the five `Site` methods `pub`** (not `pub(crate)`) — they are the build's public surface, mirroring `render_page`/`search_index_json` which are already `pub`. Update the `pub(crate)` in Tasks 1-3 to `pub` for these five entry methods (keep the internal helpers `canonical_base`/`abs_page_url`/`page_prose`/`build_atom` as `pub(crate)`/private).

- [ ] **Step 1: Add the writes** in `build.rs`, immediately after the `404.html` block and before the `keep` set is assembled. Use a small local helper to DRY the write + log:

```rust
    // SEO + discoverability sidecars: emitted only when `url:` is set (absolute URLs
    // are mandatory for feeds/sitemap/JSON-LD). All are auto-derived from the site's
    // own content; the author writes nothing SEO-specific.
    let mut seo_written: Vec<PathBuf> = Vec::new();
    if site.config.url.is_some() {
        let mut emit = |rel: &str, body: String| {
            match std::fs::write(out.join(rel), body) {
                Ok(()) => seo_written.push(PathBuf::from(rel)),
                Err(e) => log::warn(&format!("cannot write {rel}: {e}")),
            }
        };
        for (path, xml) in site.atom_feeds() {
            emit(&path, xml);
        }
        if let Some(x) = site.sitemap() {
            emit("sitemap.xml", x);
        }
        if let Some(x) = site.robots() {
            emit("robots.txt", x);
        }
        if let Some(x) = site.llms_txt() {
            emit("llms.txt", x);
        }
        if let Some(x) = site.llms_full_txt() {
            emit("llms-full.txt", x);
        }
    }
```

- [ ] **Step 2: Keep the artifacts from the stale sweep.** In the `keep` set assembly (after the `hover-index.js` insert), add:

```rust
    keep.extend(seo_written.iter().cloned());
```

- [ ] **Step 3: (Optional) surface them in the build summary line.** If the summary `format!` near the end lists `search`/`not_found`, append a note like `format!("  ·  {} SEO file(s)", seo_written.len())` when non-empty. Keep it minimal; skip if it complicates the existing summary.

- [ ] **Step 4: Build to verify it compiles + emits.**

Run:
```bash
cargo build -p taliesin-server 2>&1 | tail -3
TALIESIN_NO_EXEC=1 ./target/debug/taliesin build corpus/tech-blog --out /tmp/seo-check 2>&1 | tail -2
ls /tmp/seo-check/*.xml /tmp/seo-check/*.txt /tmp/seo-check/blog.xml
```
Expected: `blog.xml`, `projects.xml`, `sitemap.xml`, `robots.txt`, `llms.txt`, `llms-full.txt` present.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/build.rs crates/core/src/site/feed.rs crates/core/src/site/seo.rs crates/core/src/site/llms.rs
git commit -m "feat(build): emit SEO + LLM sidecars into the site build (url-gated)"
```

---

### Task 6: Footer feed link un-drop (`chrome.rs`) + re-add the config item

**Files:**
- Modify: `crates/core/src/site/chrome.rs` (footer `.xml` drop condition ~186-197)
- Modify: `corpus/tech-blog/_site.yml` (re-add the `rss` footer item)
- Test: unit test in `chrome.rs` (or `mod.rs` site tests)

**Interfaces:**
- Consumes: `self.config.url` inside `footer_html`. The `rss` glyph **already exists** in `social_icon` (`chrome.rs:391`) — nothing to add there.

- [ ] **Step 1: Write the failing test** (add to `chrome.rs`'s test module, or `mod.rs` site tests). Render a site *with* `url:` and a footer `{ icon: rss, href: blog.xml }`; assert the link is present (not dropped):

```rust
    #[test]
    fn footer_honors_local_xml_feed_link_when_url_set() {
        let root = write_site(
            "footerfeed",
            &[
                ("_site.yml", "title: Blog\nurl: https://ex.com\nfooter:\n  right:\n    - { icon: rss, href: blog.xml }\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        assert!(html.contains("href=\"blog.xml\""), "feed link honored with url: {html:.400}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn footer_still_drops_local_xml_without_url() {
        let root = write_site(
            "footerfeednourl",
            &[
                ("_site.yml", "title: Blog\nfooter:\n  right:\n    - { icon: rss, href: blog.xml }\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").unwrap();
        assert!(!html.contains("blog.xml"), "no feed generated → link dropped: {html:.400}");
        let _ = std::fs::remove_dir_all(&root);
    }
```

Adjust the `{html:.400}` width syntax to a plain `{html}` if the precision formatter is not valid for `&String` here.

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p taliesin-core --lib footer_honors 2>&1 | tail -15`
Expected: FAIL (the link is currently dropped unconditionally).

- [ ] **Step 3: Implement the conditional drop.** In `chrome.rs::footer_html` (the `group` closure), change the `.xml` drop arm (currently ~186-197) to only drop when `self.config.url` is **unset** (no feed will be generated). The closure already borrows `&self` via `self.config`; if it does not, capture `let url_set = self.config.url.is_some();` before the closure and reference it:

```rust
                    // A configured *local* `.xml` link (a feed) is dropped ONLY when no
                    // feed is generated — i.e. `url:` is unset. With `url:` set the build
                    // emits `blog.xml` etc., so the link is honest and kept.
                    Some(h)
                        if h.ends_with(".xml")
                            && !url_set
                            && !(h.starts_with("http://")
                                || h.starts_with("https://")
                                || h.starts_with("//")) =>
                    {
                        continue;
                    }
```

Define `url_set` just inside `footer_html` before the `group` closure: `let url_set = self.config.url.is_some();` and make the closure capture it (`move`/reference as the existing closure does). If the closure is `let group = |items: &[NavItem]| { … };`, add `url_set` to its captured environment (it already captures `up`).

- [ ] **Step 4: Re-add the footer item.** In `corpus/tech-blog/_site.yml`, add an `rss` item to the footer `right:` group pointing at the generated feed. Current footer:

```yaml
  - { icon: linkedin, href: https://www.linkedin.com/in/andreas-bogossian/ }
  - { icon: github, href: https://github.com/AJBogo9 }
```

Add (place it last so RSS sits after the socials, or first — author's taste; last is fine):

```yaml
  - { icon: rss, href: blog.xml }
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p taliesin-core --lib footer 2>&1 | tail -15`
Expected: PASS (both new tests).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/site/chrome.rs corpus/tech-blog/_site.yml
git commit -m "feat(site): honor a local .xml footer feed link once url: generates the feed"
```

---

### Task 7: Corpus pins + build spot-check (`tech_blog.rs`)

**Files:**
- Modify: `crates/core/tests/tech_blog.rs`
- Test: this task *is* the corpus regression net.

**Interfaces:**
- Consumes: the `pub` `Site` methods from Tasks 1-5 (`atom_feeds`, `sitemap`, `robots`, `llms_txt`, `llms_full_txt`) + `render_page`. tech-blog has `url: https://andreasbogossian.com`.

- [ ] **Step 1: Write the corpus assertions** — a new test in `crates/core/tests/tech_blog.rs`. Every assertion matches an emitted string:

```rust
/// The real blog has `url:` set, so `build` emits the discoverability sidecars. Pins
/// each artifact against the actual corpus (the regression net) — matching emitted
/// strings, never inlined-CSS/JS substrings ("gate the gate").
#[test]
fn seo_and_llm_artifacts_are_generated_for_the_blog() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let base = "https://andreasbogossian.com";

    // Atom feeds: one per uncapped dated listing (blog + projects), none for the
    // homepage's capped `recent-posts` teaser.
    let feeds = site.atom_feeds();
    let paths: Vec<&str> = feeds.iter().map(|(p, _)| p.as_str()).collect();
    assert!(paths.contains(&"blog.xml"), "blog feed: {paths:?}");
    assert!(paths.contains(&"projects.xml"), "projects feed: {paths:?}");
    assert!(!paths.iter().any(|p| p.starts_with("index")), "no feed for the capped homepage teaser: {paths:?}");
    let (_, blog_xml) = feeds.iter().find(|(p, _)| p == "blog.xml").unwrap();
    assert!(blog_xml.contains(r#"<feed xmlns="http://www.w3.org/2005/Atom">"#), "atom root");
    assert!(blog_xml.contains(&format!(r#"href="{base}/posts/"#)), "absolute post links: {blog_xml:.600}");
    assert!(blog_xml.matches("<entry>").count() >= 5, "an entry per post");

    // sitemap + robots.
    let sitemap = site.sitemap().expect("sitemap");
    assert!(sitemap.contains(&format!("<loc>{base}/</loc>")), "home in sitemap");
    assert!(sitemap.contains(&format!("<loc>{base}/posts/em-algorithm/</loc>")), "a post in sitemap");
    let robots = site.robots().expect("robots");
    assert!(robots.contains(&format!("Sitemap: {base}/sitemap.xml")), "robots names sitemap");

    // llms.txt: identity from the home hero + linked posts.
    let llms = site.llms_txt().expect("llms.txt");
    assert!(llms.starts_with("# Andreas Bogossian"), "identity H1: {llms:.200}");
    assert!(llms.contains(&format!("]({base}/posts/")), "posts linked absolutely");
    // llms-full.txt: real prose, no draft.
    let full = site.llms_full_txt().expect("llms-full.txt");
    assert!(full.contains("Andreas Bogossian"), "identity header");
    assert!(full.len() > 2000, "carries real page prose, got {} bytes", full.len());

    // JSON-LD in the rendered pages.
    let post = site.render_page("posts/em-algorithm/index.tmd").expect("post renders");
    assert!(post.contains(r#""@type":"BlogPosting""#), "BlogPosting on a post");
    let home = site.render_page("index.tmd").expect("home renders");
    assert!(home.contains(r#""@type":"WebSite""#) && home.contains(r#""@type":"Person""#), "WebSite+Person on home");

    // Footer feed link honored (url: is set).
    let blog = site.render_page("blog.tmd").expect("blog renders");
    assert!(blog.contains("href=\"blog.xml\""), "footer feed link honored");
}
```

Adjust the `{…:.600}`/`{…:.200}` precision to plain `{…}` if the formatter rejects it for `&String`/`&str` (use `&x[..x.len().min(N)]` instead if a truncated print is wanted).

- [ ] **Step 2: Run to verify pass**

Run: `cargo test -p taliesin-core --test tech_blog seo_and_llm_artifacts 2>&1 | tail -20`
Expected: PASS. If `projects.xml` is absent, confirm `corpus/tech-blog/projects.tmd` items carry `date:` (they should — projects are dated); if not, the projects feed is legitimately skipped and the assertion should be relaxed to blog-only with a comment.

- [ ] **Step 3: Full suite + fmt**

Run:
```bash
cargo test -p taliesin-core 2>&1 | grep -E "test result" | grep -v "0 failed" || echo "all green"
cargo fmt -p taliesin-core -- --check && echo "fmt clean"
```
Expected: all suites green; fmt clean.

- [ ] **Step 4: Browser/CLI spot-check the emitted files.**

Run:
```bash
cargo build -p taliesin-server 2>&1 | tail -2
rm -rf /tmp/seo-check && TALIESIN_NO_EXEC=1 ./target/debug/taliesin build corpus/tech-blog --out /tmp/seo-check 2>&1 | tail -2
echo "--- llms.txt head ---"; head -15 /tmp/seo-check/llms.txt
echo "--- blog.xml head ---"; head -20 /tmp/seo-check/blog.xml
echo "--- robots ---"; cat /tmp/seo-check/robots.txt
echo "--- files ---"; ls /tmp/seo-check/*.xml /tmp/seo-check/*.txt
```
Expected: `llms.txt` opens with `# Andreas Bogossian` + the hero identity; `blog.xml` is valid Atom; all six files present. Eyeball `llms.txt`/`llms-full.txt` for clean prose (no code-cell source, no garbled math).

- [ ] **Step 5: Commit**

```bash
git add crates/core/tests/tech_blog.rs
git commit -m "test(corpus): pin the SEO + LLM artifacts against the real blog"
```

---

## Self-Review

**Spec coverage** — every spec section maps to a task:
- Atom feeds per uncapped dated listing → Task 1. sitemap/robots → Task 2. llms.txt/llms-full.txt (identity from hero, drafts excluded, prose extraction) → Task 3. JSON-LD BlogPosting/WebSite/Person → Task 4. build wiring + keep-set → Task 5. Footer un-drop (+ `_site.yml`) → Task 6. Corpus pins + spot-check → Task 7. Zero-effort principle → realized by all artifacts deriving from existing fields (no new config).
- Trigger gate (`url:`) → asserted in every task's "no url:" test. Draft exclusion → Task 3 test + upstream discovery. XML/JSON escaping → `esc`/`serde_json`. Person `name` fallback to title → Task 4 test. `rss` icon already present → Task 6 note.

**Placeholder scan:** `Site_Page_alias` in Task 4 is called out explicitly as a fix-on-implement (real type `Page`); the `nav.items` field name in Task 3 and the `{x:.N}` format-precision are flagged with verification notes. No `TODO`/`TBD` left as silent gaps.

**Type consistency:** `atom_feeds`/`sitemap`/`robots`/`llms_txt`/`llms_full_txt` are the five `pub` entry methods used identically in Tasks 5 and 7; `canonical_base`/`abs_page_url` (Task 1) are consumed by Tasks 2-4; `text_content`/`strip_katex` are local to Task 3; `jsonld_head` (Task 4) is wired at `mod.rs:445` next to `social_head`. `rfc3339` is defined in Task 1 and reused in Task 1 only (sitemap uses the raw ISO date). Names are consistent across tasks.

**Scope:** one coherent feature (discoverability artifacts, one trigger, one data source), 7 independently-testable tasks. Not decomposed further.
