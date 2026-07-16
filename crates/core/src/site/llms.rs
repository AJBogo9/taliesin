//! `llms.txt` (a curated Markdown map — identity + linked page lists) and
//! `llms-full.txt` (every non-draft page's clean prose), so an assistant can answer
//! "who is this and what do they do?" from content the author wrote for humans.
//! Url-gated. The identity header is auto-derived from the home page's `hero:`.

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

    /// `llms-full.txt`: the identity header, then every page's title + absolute URL +
    /// clean prose (code cells and math excluded). `None` without `url:`.
    pub fn llms_full_txt(&self) -> Option<String> {
        self.canonical_base()?;
        let mut s = self.llms_header();
        for &(page, _) in &self.nav_ordered() {
            let Some(url) = self.abs_page_url(page) else {
                continue;
            };
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
    /// code cells and plain code blocks (`<pre>`) and math regions; strip tags + decode
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
                continue; // a plain (non-cell) code block
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
            let mut depth = 1usize;
            let mut i = start + marker.len();
            while i < out.len() && depth > 0 {
                if out[i..].starts_with("<span") {
                    depth += 1;
                    i += 5;
                } else if out[i..].starts_with("</span>") {
                    depth -= 1;
                    i += 7;
                } else {
                    i += (1..=4).find(|n| out.is_char_boundary(i + n)).unwrap_or(1);
                }
            }
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
            // A space at every tag so text from adjacent blocks/inlines stays
            // word-separated (the trailing whitespace-collapse tidies the runs).
            // Without it a title block emits "KL DivergenceHow to measure...
            // alignment.17 March 20263 min read".
            '<' => {
                depth += 1;
                no_tags.push(' ');
            }
            '>' => depth = depth.saturating_sub(1),
            c if depth == 0 => no_tags.push(c),
            _ => {}
        }
    }
    // Decode `&amp;` LAST so a literal, double-encoded `&amp;lt;` isn't turned into `<`.
    let decoded = no_tags
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#8217;", "\u{2019}")
        .replace("&amp;", "&");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tests::write_site;

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

    #[test]
    fn llms_full_has_prose_skips_code_and_excludes_drafts() {
        let root = write_site(
            "llmsfull",
            &[
                ("_site.yml", "title: S\nurl: https://ex.com\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\nThe intro prose paragraph.\n\n```python\nsecret_code_token = 1\n```\n",
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
        assert!(!full.contains("secret_code_token"), "code skipped: {full}");
        assert!(
            !full.contains("Hidden draft prose"),
            "draft excluded: {full}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn text_content_strips_tags_and_decodes_entities() {
        assert_eq!(
            text_content("<p>Two &amp; three &lt; four</p>"),
            "Two & three < four"
        );
        assert_eq!(text_content("<h2>A&nbsp;B</h2>"), "A B");
        // A literal, double-encoded `&lt;` must decode once, not collapse to `<`.
        assert_eq!(text_content("<p>&amp;lt;</p>"), "&lt;");
    }

    #[test]
    fn text_content_separates_adjacent_blocks() {
        // Stripping a tag must leave a word boundary behind: without one, every
        // post in the real `llms-full.txt` fused its title into its description
        // and its date into its reading time ("...alignment.17 March 20263 min read").
        assert_eq!(
            text_content("<h3>KL Divergence</h3><p>How to measure it.</p>"),
            "KL Divergence How to measure it."
        );
        assert_eq!(
            text_content("<span>17 March 2026</span><span>3 min read</span>"),
            "17 March 2026 3 min read"
        );
        // Accepted trade-off (same call `search.rs::section_text` makes): the date +
        // reading-time fusion above is two *inline* `<span>`s, so a block-level-only
        // rule would still emit "20263 min read". Separating at every tag fixes that
        // and costs mid-word inline emphasis, which degrades to readable words rather
        // than to a fake number.
        assert_eq!(text_content("<p>re<em>st</em>art</p>"), "re st art");
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
