//! `llms.txt` (a curated Markdown map — identity + linked page lists) and
//! `llms-full.txt` (every non-draft page as `taliesin read` projects it), so an assistant
//! can answer "who is this and what do they do?" from content the author wrote for humans.
//! Url-gated. The identity header is auto-derived from the home page's `hero:`.
//!
//! `llms-full.txt` has no text extractor of its own: it is [`crate::render::text`], the
//! same projection `read` and the search index use (Wave 1.5).

use super::{Page, Site};
use crate::render;
use std::collections::HashSet;

/// Normalise a nav href or page rel to a comparison key (`blog.tmd` / `blog.html` /
/// `blog/index` → `blog`).
fn nav_key(s: &str) -> &str {
    s.trim_end_matches(".tmd")
        .trim_end_matches(".html")
        .trim_end_matches("/index")
}

impl Site {
    /// The home (root `index`) page, if any.
    fn home_page(&self) -> Option<&Page> {
        self.pages.iter().find(|p| p.url == "index.html")
    }

    /// Pages in nav order (with their nav label), then any remaining discovered
    /// pages (label `None`). External nav links are skipped. Shared with `feed.rs`
    /// so feeds and the llms map deduplicate listings in the same (author-intended)
    /// order.
    pub(crate) fn nav_ordered(&self) -> Vec<(&Page, Option<&str>)> {
        let mut out: Vec<(&Page, Option<&str>)> = Vec::new();
        for item in self.config.nav.left.iter().chain(&self.config.nav.right) {
            let Some(href) = item.href.as_deref() else {
                continue;
            };
            if href.starts_with("http") {
                continue;
            }
            let want = nav_key(href);
            if let Some(p) = self.pages.iter().find(|p| nav_key(&p.rel) == want)
                && !out.iter().any(|(q, _)| q.rel == p.rel)
            {
                out.push((p, item.text.as_deref()));
            }
        }
        for p in &self.pages {
            if p.url == "404.html" {
                continue; // the error page is not site content
            }
            if !out.iter().any(|(q, _)| q.rel == p.rel) {
                out.push((p, None));
            }
        }
        out
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

    /// A `- [label](abs-url): description` line (description omitted if absent).
    fn llms_link_line(&self, label: &str, page: &Page) -> String {
        let link = self.abs_page_url(page).unwrap_or_default();
        match page.description.as_deref() {
            Some(d) => format!("- [{label}]({link}): {d}\n"),
            None => format!("- [{label}]({link})\n"),
        }
    }

    /// `llms.txt`: identity header + a section per distinct uncapped listing (titled by
    /// its host, deduped by contents so a re-listed collection like a CV's projects tail
    /// is not repeated) + a "Pages" section for standalone nav pages. `None` without `url:`.
    pub fn llms_txt(&self) -> Option<String> {
        self.canonical_base()?;
        let mut s = self.llms_header();
        let mut sink = Vec::new();
        let ordered = self.nav_ordered();
        let mut seen_contents: HashSet<String> = HashSet::new();
        let mut listed: HashSet<String> = HashSet::new();
        let mut section_hosts: HashSet<String> = HashSet::new();

        // Pass 1: one section per distinct uncapped listing; its items are "listed".
        for &(page, label) in &ordered {
            if page.url == "index.html" {
                continue; // the home page IS the identity header
            }
            let Some(spec) = page.listings.iter().find(|sp| {
                sp.max_items.is_none() && !seen_contents.contains(&Self::listing_prefix(page, sp))
            }) else {
                continue;
            };
            let items = self.collection(page, spec, &mut sink);
            if items.is_empty() {
                continue;
            }
            seen_contents.insert(Self::listing_prefix(page, spec));
            section_hosts.insert(page.rel.clone());
            let heading = label.or(page.title.as_deref()).unwrap_or("Posts");
            s.push_str(&format!("\n## {heading}\n"));
            for p in &items {
                listed.insert(p.rel.clone());
                s.push_str(&self.llms_link_line(p.title.as_deref().unwrap_or(&p.rel), p));
            }
        }

        // Pass 2: standalone titled pages (CV, Publications) → "## Pages".
        let mut pages_section = String::new();
        for &(page, label) in &ordered {
            if page.url == "index.html" || page.title.is_none() {
                continue;
            }
            if section_hosts.contains(&page.rel) || listed.contains(&page.rel) {
                continue;
            }
            let heading = label.or(page.title.as_deref()).unwrap_or(page.rel.as_str());
            pages_section.push_str(&self.llms_link_line(heading, page));
        }
        if !pages_section.is_empty() {
            s.push_str("\n## Pages\n");
            s.push_str(&pages_section);
        }
        Some(s)
    }

    /// `llms-full.txt`: the identity header, then every page's title + absolute URL + the
    /// page as `taliesin read` projects it. `None` without `url:`.
    ///
    /// **This is the `read` projection, not a second extractor** (Wave 1.5). The file
    /// exists so someone else's assistant can ingest the site, and on a technical site the
    /// code is the content — so headings, fenced code, resolved "Figure N" and display math
    /// as TeX all belong in it. The previous prose-only extractor (`page_prose` +
    /// `strip_katex` + `text_content`) was the second of the tool's text projections and is
    /// gone; `render::text` is the one recipe.
    ///
    /// Rendered WITHOUT execution, as before: a cell's source is content, its output is a
    /// build artifact.
    pub fn llms_full_txt(&self) -> Option<String> {
        self.canonical_base()?;
        let mut s = self.llms_header();
        for &(page, _) in &self.nav_ordered() {
            let Some(url) = self.abs_page_url(page) else {
                continue;
            };
            let title = page.title.as_deref().unwrap_or(&page.rel);
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let base = page.input.parent().unwrap_or(&self.root);
            let body = render::render_document_scoped_with_site(
                &src,
                base,
                self.chapter_for(page),
                Some(&self.render_defaults()),
            )
            .body_text();
            if body.trim().is_empty() {
                continue;
            }
            s.push_str(&format!("\n---\n\n## {title}\n{url}\n\n{body}\n"));
        }
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tests::write_site;

    /// Math must land in the dump exactly once. KaTeX emits every formula TWICE — a MathML
    /// `<annotation>` carrying the TeX source plus the visual glyph spans — so a naive tag
    /// strip publishes both, garbled and doubled. The old prose extractor solved that by
    /// deleting math outright; the shared `render::text` projection solves it by reading one
    /// of the two (display math as its TeX, inline math as its glyphs). Either way the
    /// invariant is the same, and it is the reason this fold could not use a hand-rolled
    /// `<`/`>` scan.
    #[test]
    fn llms_full_carries_math_once_not_doubled() {
        let root = write_site(
            "llmsmath",
            &[
                ("_site.yml", "title: S\nurl: https://ex.com\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\nEnergy $E=mc^2$ is conserved.\n\n$$a+b$$\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let full = site.llms_full_txt().expect("llms-full.txt with url:");
        assert!(
            !full.contains("annotation") && !full.contains("<math"),
            "KaTeX MathML leaked into the dump: {full}"
        );
        assert_eq!(
            full.matches("a+b").count(),
            1,
            "display math must appear exactly once (TeX or glyphs, never both): {full}"
        );
        assert!(full.contains("is conserved."), "prose kept: {full}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn llms_txt_leads_with_hero_identity_and_lists_posts() {
        let root = write_site(
            "llmsmap",
            &[
                (
                    "_site.yml",
                    "title: Andreas Bogossian\ndescription: ML from first principles\nurl: https://ex.com\n",
                ),
                (
                    "index.tmd",
                    "---\ntitle: Andreas Bogossian\nhero:\n  eyebrow: ML\n  headline: Machine learning, from the math up\n  lead: I build systems at the intersection of math and software.\nlisting:\n  contents: posts\n  max-items: 2\n---\n\n## Recent\n",
                ),
                (
                    "blog.tmd",
                    "---\ntitle: Blog\nlisting:\n  contents: posts\n  type: list\n---\n\n# Blog\n",
                ),
                (
                    "posts/a/index.tmd",
                    "---\ntitle: First Post\ndate: 2026-05-15\ndescription: A summary of the post.\n---\n\nBody.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let txt = site.llms_txt().expect("llms.txt with url:");
        assert!(
            txt.starts_with("# Andreas Bogossian"),
            "H1 = site title: {txt}"
        );
        assert!(
            txt.contains("> ML from first principles"),
            "tagline blockquote: {txt}"
        );
        assert!(
            txt.contains("Machine learning, from the math up"),
            "hero headline in About: {txt}"
        );
        assert!(
            txt.contains("intersection of math and software"),
            "hero lead in About: {txt}"
        );
        assert!(
            txt.contains("[First Post](https://ex.com/posts/a/): A summary of the post."),
            "post listed with absolute link + description: {txt}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `llms-full.txt` is the `read` projection, page by page (Wave 1.5). It carries the
    /// code, because the file exists so someone else's assistant can ingest the site and on
    /// a technical site the code IS the content. Drafts stay excluded.
    #[test]
    fn llms_full_is_the_read_projection_and_excludes_drafts() {
        let root = write_site(
            "llmsfull",
            &[
                ("_site.yml", "title: S\nurl: https://ex.com\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\nThe intro prose paragraph.\n\n```python\nvisible_code_token = 1\n```\n",
                ),
                (
                    "posts/draft/index.tmd",
                    "---\ntitle: WIP\ndraft: true\n---\n\nHidden draft prose.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let full = site.llms_full_txt().expect("llms-full.txt with url:");
        assert!(
            full.contains("The intro prose paragraph."),
            "prose kept: {full}"
        );
        assert!(
            full.contains("```python\nvisible_code_token = 1\n```"),
            "code fenced with its language, as `read` projects it: {full}"
        );
        assert!(
            !full.contains("Hidden draft prose"),
            "draft excluded: {full}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The R1 blocker, discharged. `llms-full.txt` could not adopt the shared projection
    /// while that projection left numeric character references raw: the old `text_content`
    /// decoded `&#8217;`/`&nbsp;` and `render::text::decode` did not, so folding would have
    /// published `it&#8217;s` where the site reads `it's`. `decode` now handles numeric
    /// refs, so one recipe serves `read`, the search index and this file.
    #[test]
    fn llms_full_decodes_numeric_character_references() {
        let root = write_site(
            "llmsents",
            &[
                ("_site.yml", "title: S\nurl: https://ex.com\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\nit&#8217;s fine&nbsp;here\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let full = site.llms_full_txt().expect("llms-full.txt with url:");
        assert!(
            full.contains("it\u{2019}s fine"),
            "the numeric reference must decode in the published dump: {full}"
        );
        assert!(
            !full.contains("&#8217;") && !full.contains("&#"),
            "a raw numeric entity leaked into llms-full.txt: {full}"
        );
        // `&nbsp;` survives as a real U+00A0 rather than being flattened to a space: the
        // author wrote a non-breaking space and the dump reports what the page says.
        assert!(full.contains("fine\u{a0}here"), "nbsp preserved: {full}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The word-boundary rule the deleted `text_content` existed to enforce, re-pinned at
    /// the artifact instead of at the helper. A listing card is ONE block whose HTML holds
    /// adjacent elements (title, date, reading time); a tag strip that leaves no boundary
    /// fused them in the real published file — "…alignment.17 March 20263 min read". This
    /// asserts the fused forms never come back, whichever projection produces the dump.
    #[test]
    fn llms_full_does_not_fuse_a_listing_cards_fields() {
        let root = write_site(
            "llmslisting",
            &[
                (
                    "_site.yml",
                    "title: S\nurl: https://ex.com\nnav:\n  left:\n    - href: index.tmd\n      text: Home\n",
                ),
                (
                    "index.tmd",
                    "---\ntitle: Home\nlisting:\n  contents: posts\n---\n\nRecent writing.\n",
                ),
                (
                    "posts/kl.tmd",
                    "---\ntitle: KL Divergence\ndate: 2026-03-17\ndescription: How to measure alignment.\n---\n\nBody prose.\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let full = site.llms_full_txt().expect("llms-full.txt with url:");
        assert!(
            !full.contains("alignment.17"),
            "listing description fused into the date: {full}"
        );
        assert!(
            !full.contains("20263"),
            "listing date fused into the reading time: {full}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn none_without_url() {
        let root = write_site(
            "llmsnourl",
            &[
                ("_site.yml", "title: S\n"),
                ("index.tmd", "---\ntitle: H\n---\n\nx\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(site.llms_txt().is_none());
        assert!(site.llms_full_txt().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
