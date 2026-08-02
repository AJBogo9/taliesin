//! Book project model: chapter navigation (`Book`/`BookEntry` from
//! `book: chapters:`) and the per-chapter page set. `use super::*` reaches Page,
//! parse_front_matter, tmd_to_html, etc.

use super::*;

/// A page's book chapter number, if it is a numbered chapter (`None` for a website page,
/// the `index` preface, or an unnumbered entry). The one lookup behind every number a
/// reader sees: `Site::chapter_for` delegates here, and the two passes that run before a
/// `Site` exists (`scan_xref_targets`, `search::build_sections`) call it directly, so the
/// registry, the search index, and the rendered page cannot disagree about a chapter.
pub(super) fn chapter_of(book: &Option<Book>, page: &Page) -> Option<u32> {
    book.as_ref().and_then(|b| {
        b.entries
            .iter()
            .find(|e| e.rel == page.rel)
            .and_then(|e| e.number)
    })
}

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
    /// `draft: true` front matter on the chapter file. Only ever `true` in
    /// `DraftMode::Include` (a draft chapter is dropped entirely in `Exclude`).
    pub draft: bool,
    /// Nesting level of a part header: 0 for a top-level `{ part: }`, 1 for one nested
    /// inside another, and so on. Always 0 for a chapter entry. Lets the drawer indent a
    /// sub-part instead of flattening it into its parent.
    pub depth: u8,
    /// The chapter's prose length in words ([`crate::prose::word_count`] over the
    /// include-expanded source), shown in the drawer and the landing Contents so the
    /// "which chapter do I open" decision is made with its cost visible. 0 for a part
    /// header. **Prose only:** front matter, fenced code and `:::` fences are excluded,
    /// which is the same selection `lint` and the reading-time estimate use — so
    /// a code-heavy chapter is understated, and the label says "words", never a time.
    pub words: usize,
}
/// A chapter's prose length as the visible text both nav surfaces print: `"1,204 words"`.
/// `None` for a chapter with no prose at all (a figure-only or stub chapter), where
/// "0 words" reads as a bug rather than as information.
///
/// **Words, never a time.** `word_count` excludes fenced code and math, so a code-heavy
/// chapter is understated — and reading code is slower than reading prose, so a "~N min"
/// label would be wrong in the same direction, twice over, on exactly the chapters this
/// tool exists for. A word count states what was measured and claims nothing about the
/// reader. Absolute units, never a normalised bar: a bar's full width is 8 words in one
/// book and 4,077 in another, which misleads exactly where it claims to help.
pub(super) fn words_label(words: usize) -> Option<String> {
    if words == 0 {
        return None;
    }
    let digits = words.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    Some(format!(
        "{grouped} word{}",
        if words == 1 { "" } else { "s" }
    ))
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
pub(super) fn build_book(
    root: &Path,
    config: &SiteConfig,
    mode: DraftMode,
    excluded: &mut Vec<String>,
) -> Book {
    let mut entries = Vec::new();
    let mut num = 0u32;
    push_group(
        root,
        &config.chapters,
        0,
        &mut entries,
        &mut num,
        mode,
        excluded,
    );
    Book {
        title: config.title.clone(),
        entries,
    }
}

/// Walk one `chapters:` list in order, appending its chapters and part headers.
///
/// **Recurses into a nested `{ part:, chapters: }` group.** The inner loop used to call
/// [`push_chapter_entry`] and discard its `false` — the signal that the value was some
/// other shape — so a part nested inside a part silently deleted itself AND every chapter
/// under it, with `check` still exiting 0. (The outer loop always did check that return;
/// only the inner one dropped it.)
fn push_group(
    root: &Path,
    list: &[serde_yaml::Value],
    depth: u8,
    entries: &mut Vec<BookEntry>,
    num: &mut u32,
    mode: DraftMode,
    excluded: &mut Vec<String>,
) {
    for ch in list {
        if push_chapter_entry(root, ch, entries, num, mode, excluded) {
            continue;
        }
        // Not a chapter ⇒ a `{ part:, chapters: }` group header + its inner entries.
        if let Some(map) = ch.as_mapping() {
            let part = map
                .get("part")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let header_idx = entries.len();
            entries.push(BookEntry {
                part: Some(part),
                depth,
                ..Default::default()
            });
            if let Some(seq) = map.get("chapters").and_then(|v| v.as_sequence()) {
                push_group(
                    root,
                    seq,
                    depth.saturating_add(1),
                    entries,
                    num,
                    mode,
                    excluded,
                );
            }
            // Every chapter in this part was a draft and got dropped: drop the now-empty
            // part header too, rather than leaving an orphan heading over nothing in the
            // drawer. (Drafting a whole part is a natural authoring state.) Runs after the
            // recursion, so an outer part whose only content was an all-draft inner part
            // collapses too.
            if entries.len() == header_idx + 1 {
                entries.pop();
            }
        }
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
    mode: DraftMode,
    excluded: &mut Vec<String>,
) -> bool {
    if let Some(file) = value.as_str() {
        push_chapter(root, file, None, entries, num, mode, excluded);
        return true;
    }
    if let Some(map) = value.as_mapping()
        && let Some(file) = map.get("file").and_then(|v| v.as_str())
    {
        let label = map.get("text").and_then(|v| v.as_str());
        push_chapter(root, file, label, entries, num, mode, excluded);
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
    mode: DraftMode,
    excluded: &mut Vec<String>,
) {
    let input = root.join(file);
    let rel = file.to_string();
    let src = std::fs::read_to_string(&input).unwrap_or_default();
    let (h1, unnumbered) = chapter_heading_in(&src);
    // The cost signal, from the same read. `word_count`'s contract is an include-EXPANDED
    // source: a chapter assembled from partials would otherwise be measured at the length
    // of its own directive lines, so the longest chapters in a book would read as the
    // shortest. Expanding here costs one pass over the (already-read) source plus a read of
    // each partial, at discovery, once per build.
    let base_dir = input.parent().unwrap_or(root);
    let words = crate::prose::word_count(&crate::includes::resolve(&src, base_dir).0);
    // Parse once: needed for the draft gate and (below) the title fallback. Throwaway
    // warnings: `book_pages` re-parses this file with the real sink, so a
    // listing-without-contents warning here would just duplicate it.
    let fm = parse_front_matter(&input, file, &mut Vec::new());
    // A draft chapter is dropped in the published view (recorded so the build can report
    // it) — no entry, no number bump, so the book renumbers as if it weren't listed. In
    // the preview view it stays, tagged, and is numbered in context.
    if fm.draft && mode == DraftMode::Exclude {
        excluded.push(rel);
        return;
    }
    let title = label
        .map(str::to_string)
        .or(h1)
        .or(fm.title)
        .unwrap_or_else(|| {
            crate::ext::strip_source_ext(&rel)
                .unwrap_or(&rel)
                .to_string()
        });
    // The `index.tmd` preface is unnumbered by convention.
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
        url: tmd_to_html(&rel),
        rel,
        draft: fm.draft,
        depth: 0, // only a part header nests; a chapter is always a leaf
        words,
    });
}
/// A page's leading `# H1` text (attributes stripped) and whether that heading is
/// unnumbered (`{.unnumbered}` / `{-}`). Used for a book chapter's title fallback and,
/// via the `.0`, for a titleless website page's title ([`discovery::website_pages`]).
pub(super) fn chapter_heading(input: &Path) -> (Option<String>, bool) {
    let Ok(src) = std::fs::read_to_string(input) else {
        return (None, false);
    };
    chapter_heading_in(&src)
}
/// [`chapter_heading`] against an already-read source, so `push_chapter` (which also needs
/// the text for its prose count) reads each chapter file once rather than twice.
fn chapter_heading_in(src: &str) -> (Option<String>, bool) {
    // `content_lines` skips front matter + fenced code (so a `# comment` inside ```yaml/```sh
    // isn't mistaken for the chapter's H1); take the first real `# ` heading.
    for t in content_lines(src) {
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
                hero: fm.hero,
                page_layout: fm.page_layout,
                has_bibliography: fm.has_bibliography,
                draft: c.draft,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a book from an inline `chapters:` list against a temp dir of empty chapters.
    fn book_of(yaml: &str, files: &[&str]) -> Book {
        let dir = std::env::temp_dir().join(format!(
            "tali-book-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for f in files {
            std::fs::write(dir.join(f), format!("# {f}\n")).unwrap();
        }
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let config = SiteConfig {
            chapters: value
                .get("chapters")
                .and_then(|v| v.as_sequence())
                .cloned()
                .unwrap_or_default(),
            ..Default::default()
        };
        let book = build_book(&dir, &config, DraftMode::Include, &mut Vec::new());
        std::fs::remove_dir_all(&dir).ok();
        book
    }

    /// Build a book whose chapter files carry the given bodies (not the empty `# a.tmd`
    /// stub `book_of` writes), so a test can assert on their contents.
    fn book_of_bodies(yaml: &str, files: &[(&str, &str)]) -> Book {
        let dir = std::env::temp_dir().join(format!(
            "tali-book-body-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for (f, body) in files {
            let path = dir.join(f);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, body).unwrap();
        }
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let config = SiteConfig {
            chapters: value
                .get("chapters")
                .and_then(|v| v.as_sequence())
                .cloned()
                .unwrap_or_default(),
            ..Default::default()
        };
        let book = build_book(&dir, &config, DraftMode::Include, &mut Vec::new());
        std::fs::remove_dir_all(&dir).ok();
        book
    }

    /// The count reads the **include-expanded** source, not the raw file. A chapter that is
    /// mostly `{{< include >}}` would otherwise be advertised as a few words long, which is
    /// exactly backwards: the transcluded prose is the chapter.
    ///
    /// Pinned here since 2026-08-03. It used to be caught in `tarn.rs` by comparing the nav
    /// against the `skim` projection (which counted the raw source), and that projection was
    /// retired — a cross-surface drift test cannot outlive the surface it compared against,
    /// so the property it was really protecting is asserted directly instead.
    #[test]
    fn a_chapters_prose_length_counts_what_its_includes_bring_in() {
        let book = book_of_bodies(
            "chapters:\n  - a.tmd\n",
            &[
                (
                    "a.tmd",
                    "---\ntitle: \"Host\"\n---\n\none two\n\n{{< include _part.tmd >}}\n",
                ),
                ("_part.tmd", "three four five six\n"),
            ],
        );
        let chapter = book
            .entries
            .iter()
            .find(|e| e.part.is_none())
            .expect("one chapter");
        assert_eq!(
            chapter.words, 6,
            "the included prose counts toward the chapter's advertised length"
        );
    }

    #[test]
    fn a_chapters_prose_length_excludes_code_and_front_matter() {
        // The cost signal in the drawer and the landing Contents is `prose::word_count`,
        // the same selection `lint` and the reading-time estimate use — so
        // front matter, fenced code and `:::` fences are all out. Counting raw words would
        // report a code-heavy chapter as the longest thing in the book.
        let book = book_of_bodies(
            "chapters:\n  - a.tmd\n",
            &[(
                "a.tmd",
                "---\ntitle: \"Counted out\"\nsubtitle: \"also out\"\n---\n\n\
                 # Heading\n\n\
                 one two three four five\n\n\
                 ```python\nnot counted at all in here friend\n```\n\n\
                 six seven\n",
            )],
        );
        assert_eq!(
            book.chapters()[0].words,
            8,
            "`Heading` (1) + 5 + 2 prose words, with front matter and the fence excluded"
        );
    }

    #[test]
    fn an_include_built_chapters_prose_counts_what_the_reader_will_read() {
        // `prose::word_count`'s contract is an include-EXPANDED source. A chapter assembled
        // from partials is otherwise reported at the length of its own directive lines,
        // i.e. the longest chapters in a book read as the shortest.
        let book = book_of_bodies(
            "chapters:\n  - a.tmd\n",
            &[
                ("a.tmd", "# Assembled\n\n{{< include _part.tmd >}}\n"),
                ("_part.tmd", "one two three four five six seven eight\n"),
            ],
        );
        assert_eq!(
            book.chapters()[0].words,
            9,
            "`# Assembled` (1) + the 8 words the include pulls in"
        );
    }

    #[test]
    fn a_nested_part_group_keeps_its_chapters() {
        // The regression: the inner loop called `push_chapter_entry` and threw away its
        // `false`, so a part nested inside a part deleted ITSELF and every chapter under
        // it — and `check` still exited 0. Two chapters went in; two must come out.
        let book = book_of(
            "chapters:\n  - a.tmd\n  - part: Outer\n    chapters:\n      - b.tmd\n      - part: Inner\n        chapters:\n          - c.tmd\n",
            &["a.tmd", "b.tmd", "c.tmd"],
        );
        let chapters: Vec<&str> = book.chapters().iter().map(|c| c.rel.as_str()).collect();
        assert_eq!(
            chapters,
            ["a.tmd", "b.tmd", "c.tmd"],
            "a chapter under a nested part must survive"
        );
        let parts: Vec<(&str, u8)> = book
            .entries
            .iter()
            .filter_map(|e| e.part.as_deref().map(|p| (p, e.depth)))
            .collect();
        assert_eq!(
            parts,
            [("Outer", 0), ("Inner", 1)],
            "both part headers present, the inner one one level deeper"
        );
    }

    #[test]
    fn a_nested_part_still_numbers_chapters_in_reading_order() {
        // Numbering must run straight through the nesting, not restart per part.
        let book = book_of(
            "chapters:\n  - part: One\n    chapters:\n      - a.tmd\n      - part: Two\n        chapters:\n          - b.tmd\n  - c.tmd\n",
            &["a.tmd", "b.tmd", "c.tmd"],
        );
        let nums: Vec<Option<u32>> = book.chapters().iter().map(|c| c.number).collect();
        assert_eq!(nums, [Some(1), Some(2), Some(3)]);
    }
}
