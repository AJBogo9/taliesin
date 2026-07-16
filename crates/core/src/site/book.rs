//! Book project model: chapter navigation (`Book`/`BookEntry` from
//! `book: chapters:`) and the per-chapter page set. `use super::*` reaches Page,
//! parse_front_matter, qmd_to_html, etc.

use super::*;

/// The resolved book navigation: the sidebar order (parts + chapters) plus the
/// chapter title/number/url for each chapter page. Present only for a book.
#[derive(Debug, Clone, Default)]
pub struct Book {
    pub title: Option<String>,
    /// Sidebar entries in order; a part header has `part: Some` and no `url`.
    pub entries: Vec<BookEntry>,
}
/// One sidebar row: a part header (`part` set, `url` empty) or a chapter (`url`
/// set, `number` = its chapter number, `None` for an unnumbered preface).
#[derive(Debug, Clone, Default)]
pub struct BookEntry {
    pub part: Option<String>,
    pub number: Option<u32>,
    pub title: String,
    pub rel: String,
    pub url: String,
}
impl Book {
    /// The chapters in reading order (part headers dropped), for prev/next.
    pub(super) fn chapters(&self) -> Vec<&BookEntry> {
        self.entries
            .iter()
            .filter(|e| e.part.is_none() && !e.url.is_empty())
            .collect()
    }
}
/// Resolve `book: chapters:` into the sidebar navigation: walk the ordered list,
/// assigning each chapter a running number (an unnumbered chapter — the `index.tmd`
/// preface or one whose H1 carries `.unnumbered`/`{-}` — is skipped in the count).
/// Each list entry is one of three shapes: a bare path string
/// (`- intro.tmd`), a `{ file:, text: }` chapter with a label override, or a
/// `{ part:, chapters: }` group whose inner list takes the same string-or-`{file,text}`
/// chapter shapes.
pub(super) fn build_book(root: &Path, config: &SiteConfig) -> Book {
    let mut entries = Vec::new();
    let mut num = 0u32;
    for ch in &config.chapters {
        if push_chapter_entry(root, ch, &mut entries, &mut num) {
            continue;
        }
        // Not a chapter ⇒ a `{ part:, chapters: }` group header + its inner chapters.
        if let Some(map) = ch.as_mapping() {
            let part = map
                .get("part")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            entries.push(BookEntry {
                part: Some(part),
                ..Default::default()
            });
            if let Some(seq) = map.get("chapters").and_then(|v| v.as_sequence()) {
                for c in seq {
                    push_chapter_entry(root, c, &mut entries, &mut num);
                }
            }
        }
    }
    Book {
        title: config.title.clone(),
        entries,
    }
}
/// Push one chapter from a list entry that is either a bare path string or a
/// `{ file:, text: }` mapping (the `text:` overrides the sidebar label). Returns
/// `true` when it consumed `value` as a chapter, `false` if it is some other shape
/// (e.g. a `{ part: }` group) the caller must handle.
fn push_chapter_entry(
    root: &Path,
    value: &serde_yaml::Value,
    entries: &mut Vec<BookEntry>,
    num: &mut u32,
) -> bool {
    if let Some(file) = value.as_str() {
        push_chapter(root, file, None, entries, num);
        return true;
    }
    if let Some(map) = value.as_mapping()
        && let Some(file) = map.get("file").and_then(|v| v.as_str())
    {
        let label = map.get("text").and_then(|v| v.as_str());
        push_chapter(root, file, label, entries, num);
        return true;
    }
    false
}
/// Append one chapter entry, bumping the chapter counter unless it is unnumbered.
/// `label` (from a `{ file:, text: }` entry) overrides the sidebar label; without
/// it the label falls back to the first `# H1`, then front-matter `title:`, then
/// the file stem.
fn push_chapter(
    root: &Path,
    file: &str,
    label: Option<&str>,
    entries: &mut Vec<BookEntry>,
    num: &mut u32,
) {
    let input = root.join(file);
    let rel = file.to_string();
    let (h1, unnumbered) = chapter_heading(&input);
    let title = label
        .map(str::to_string)
        .or(h1)
        // Throwaway warnings: `book_pages` re-parses this file with the real sink, so a
        // listing-without-contents warning here would just duplicate it.
        .or_else(|| parse_front_matter(&input, file, &mut Vec::new()).title)
        .unwrap_or_else(|| {
            crate::ext::strip_source_ext(&rel)
                .unwrap_or(&rel)
                .to_string()
        });
    // The `index.{tmd,qmd}` preface is unnumbered by convention.
    let number = if unnumbered || crate::ext::strip_source_ext(&rel) == Some("index") {
        None
    } else {
        *num += 1;
        Some(*num)
    };
    entries.push(BookEntry {
        part: None,
        number,
        title,
        url: qmd_to_html(&rel),
        rel,
    });
}
/// A page's leading `# H1` text (attributes stripped) and whether that heading is
/// unnumbered (`{.unnumbered}` / `{-}`). Used for a book chapter's title fallback and,
/// via the `.0`, for a titleless website page's title ([`discovery::website_pages`]).
pub(super) fn chapter_heading(input: &Path) -> (Option<String>, bool) {
    let Ok(src) = std::fs::read_to_string(input) else {
        return (None, false);
    };
    let mut in_fm = false;
    let mut in_code = false;
    for (i, line) in src.lines().enumerate() {
        let t = line.trim_start();
        if i == 0 && t == "---" {
            in_fm = true;
            continue;
        }
        if in_fm {
            if t == "---" {
                in_fm = false;
            }
            continue;
        }
        // Skip fenced code blocks so a `# comment` inside ```yaml/```sh isn't
        // mistaken for the chapter's H1.
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        if let Some(rest) = t.strip_prefix("# ") {
            let unnumbered = rest.contains(".unnumbered") || rest.contains("{-}");
            let title = rest.split('{').next().unwrap_or(rest).trim().to_string();
            return (Some(title), unnumbered);
        }
    }
    (None, false)
}
/// A book's pages: one [`Page`] per chapter, in reading order.
pub(super) fn book_pages(root: &Path, book: &Book, warnings: &mut Vec<String>) -> Vec<Page> {
    book.chapters()
        .into_iter()
        .map(|c| {
            let input = root.join(&c.rel);
            let fm = parse_front_matter(&input, &c.rel, warnings);
            Page {
                input,
                rel: c.rel.clone(),
                url: c.url.clone(),
                title: Some(c.title.clone()),
                date: fm.date,
                description: fm.description,
                authors: fm.authors,
                card_image: None,
                card_image_alt: None,
                categories: fm.categories,
                listings: fm.listings,
                about: fm.about,
                hero: fm.hero,
                page_layout: fm.page_layout,
                has_bibliography: fm.has_bibliography,
                draft: false,
            }
        })
        .collect()
}
