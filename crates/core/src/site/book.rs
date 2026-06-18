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
/// Resolve `book: chapters:` into the sidebar navigation: walk the ordered list
/// (chapter file names + `{ part, chapters }` groups), assigning each chapter a
/// running number (an unnumbered chapter — the `index.qmd` preface or one whose
/// H1 carries `.unnumbered`/`{-}` — is skipped in the count, like Quarto).
pub(super) fn build_book(root: &Path, config: &SiteConfig) -> Book {
    let mut entries = Vec::new();
    let mut num = 0u32;
    for ch in &config.chapters {
        if let Some(file) = ch.as_str() {
            push_chapter(root, file, &mut entries, &mut num);
        } else if let Some(map) = ch.as_mapping() {
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
                for c in seq.iter().filter_map(|v| v.as_str()) {
                    push_chapter(root, c, &mut entries, &mut num);
                }
            }
        }
    }
    Book {
        title: config.title.clone(),
        entries,
    }
}
/// Append one chapter entry, bumping the chapter counter unless it is unnumbered.
fn push_chapter(root: &Path, file: &str, entries: &mut Vec<BookEntry>, num: &mut u32) {
    let input = root.join(file);
    let rel = file.to_string();
    let (h1, unnumbered) = chapter_heading(&input);
    let title = h1
        .or_else(|| parse_front_matter(&input).title)
        .unwrap_or_else(|| rel.trim_end_matches(".qmd").to_string());
    // The `index.qmd` preface is unnumbered by convention, like Quarto.
    let number = if unnumbered || rel == "index.qmd" {
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
/// A chapter's title (its first `# H1` text, attributes stripped) and whether
/// that heading is unnumbered (`{.unnumbered}` / `{-}`).
fn chapter_heading(input: &Path) -> (Option<String>, bool) {
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
pub(super) fn book_pages(root: &Path, book: &Book) -> Vec<Page> {
    book.chapters()
        .into_iter()
        .map(|c| {
            let input = root.join(&c.rel);
            let fm = parse_front_matter(&input);
            Page {
                input,
                rel: c.rel.clone(),
                url: c.url.clone(),
                title: Some(c.title.clone()),
                date: fm.date,
                description: fm.description,
                card_image: None,
                categories: fm.categories,
                is_post: false,
                listings: fm.listings,
                about: fm.about,
                page_layout: fm.page_layout,
            }
        })
        .collect()
}
