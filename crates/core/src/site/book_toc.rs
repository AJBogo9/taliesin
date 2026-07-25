//! B2: the book landing-page auto-TOC.
//!
//! A whole-book **Contents** list rendered on a book's landing page (the hardcover
//! pattern), so a reader can jump straight to any chapter. It reuses the ordered
//! `Book.entries` the drawer (`chrome::sidebar_html`) and prev/next (`book_nav_html`)
//! already iterate, and joins each chapter to its `Page.description` for an optional
//! blurb. Additive: the per-page scrollspy TOC and the chapter drawer are untouched
//! (the 2026-07-06 "keep both nav surfaces" decision).
//!
//! It is a generated content block (`tali-book-toc`, no sourcepos) appended in
//! `finish_blocks`, exactly like `attach_backlinks`/`attach_cite_this` — NOT a chrome
//! slot, so it cannot collide with the drawer's `[data-tali-drawer-close]` markup. The
//! class prefix is deliberately `tali-btoc-*`, never the drawer's `.tali-book-chapter`.

use super::{Block, BookEntry, Page, Site};
use crate::render::escape_attr as esc;

/// Build the landing-page Contents `<nav>` from a book's entries. `current_url` is the
/// landing page's own url (its own entry is skipped — no self-link). `desc_of(rel)` yields
/// a chapter's `description:` blurb, if any. Returns `None` when no linkable chapter
/// remains (an empty book), so no empty nav is emitted.
pub(super) fn render_book_toc(
    entries: &[BookEntry],
    current_url: &str,
    desc_of: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    // A linkable chapter is a non-part row with a url that is not the landing itself.
    let is_chapter = |e: &BookEntry| e.part.is_none() && !e.url.is_empty() && e.url != current_url;
    if !entries.iter().any(is_chapter) {
        return None;
    }
    // Depth-relative prefix, like the drawer: the landing is at the site root
    // (`index.html`), so this is empty in practice, but keep the general form.
    let up = "../".repeat(current_url.matches('/').count());
    let mut s = String::from(
        "<nav class=\"tali-book-landing-toc\" aria-labelledby=\"tali-btoc-h\" \
         data-block-id=\"tali-book-toc\">\
         <h2 id=\"tali-btoc-h\" class=\"tali-btoc-title\">Contents</h2>\
         <ul class=\"tali-btoc-list\">",
    );
    for e in entries {
        if let Some(part) = &e.part {
            // A nested part is indented here too, so the landing Contents and the drawer
            // show the same structure (they are two renderers over one entry list).
            let nested = if e.depth > 0 {
                " tali-btoc-part-nested"
            } else {
                ""
            };
            s.push_str(&format!(
                "<li class=\"tali-btoc-part{nested}\">{}</li>",
                esc(part)
            ));
            continue;
        }
        if !is_chapter(e) {
            continue; // the landing's own entry (no self-link) or an empty-url row
        }
        let num = e
            .number
            .map(|n| format!("<span class=\"tali-btoc-num\">{n}</span> "))
            .unwrap_or_default();
        let (item_cls, badge) = if e.draft {
            (
                " tali-btoc-draft",
                " <span class=\"tali-draft-badge\">Draft</span>",
            )
        } else {
            ("", "")
        };
        let desc = desc_of(&e.rel)
            .filter(|d| !d.trim().is_empty())
            .map(|d| format!("<p class=\"tali-btoc-desc\">{}</p>", esc(&d)))
            .unwrap_or_default();
        // Same cost signal the drawer shows, from the same `words_label`, so the two
        // renderers over one entry list cannot print different numbers for one chapter.
        let words = super::book::words_label(e.words)
            .map(|w| format!("<span class=\"tali-btoc-words\">{w}</span>"))
            .unwrap_or_default();
        s.push_str(&format!(
            "<li class=\"tali-btoc-item{item_cls}\">\
             <a class=\"tali-btoc-link\" href=\"{up}{url}\">\
             {num}<span class=\"tali-btoc-chap\">{title}</span>{badge}{words}</a>{desc}</li>",
            url = e.url,
            title = esc(&e.title),
        ));
    }
    s.push_str("</ul></nav>");
    Some(s)
}

impl Site {
    /// Append the whole-book Contents list at the end of the book landing's content. A
    /// no-op unless this is a book AND this page is the landing (`index.html`). Mirrors
    /// `attach_cite_this`: a single-root generated block the incremental client mounts
    /// cleanly.
    pub(super) fn attach_book_toc(&self, page: &Page, blocks: &mut Vec<Block>) {
        let Some(book) = &self.book else {
            return;
        };
        if !self.is_home(page) {
            return;
        }
        let desc_of = |rel: &str| {
            self.pages
                .iter()
                .find(|p| p.rel == rel)
                .and_then(|p| p.description.clone())
        };
        if let Some(html) = render_book_toc(&book.entries, &page.url, desc_of) {
            blocks.push(Block {
                id: "tali-book-toc".to_string(),
                sourcepos: String::new(),
                source_file: None,
                html,
                cell: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(number: Option<u32>, title: &str, url: &str) -> BookEntry {
        BookEntry {
            depth: 0,
            part: None,
            number,
            title: title.to_string(),
            rel: format!("{}.tmd", url.trim_end_matches(".html")),
            url: url.to_string(),
            draft: false,
            words: 0,
        }
    }
    fn part(name: &str) -> BookEntry {
        BookEntry {
            part: Some(name.to_string()),
            ..Default::default()
        }
    }

    /// The demo-book shape: preface (unnumbered, the landing) → Intro (1) → part "Core"
    /// → Methods (2) → Results (3).
    fn demo_entries() -> Vec<BookEntry> {
        vec![
            chapter(None, "Preface", "index.html"),
            chapter(Some(1), "Introduction", "intro.html"),
            part("Core"),
            chapter(Some(2), "Methods", "methods.html"),
            chapter(Some(3), "Results", "results.html"),
        ]
    }

    #[test]
    fn lists_numbered_chapters_and_the_part_divider_in_order() {
        let html = render_book_toc(&demo_entries(), "index.html", |_| None).unwrap();
        assert!(
            html.contains("data-block-id=\"tali-book-toc\""),
            "got: {html}"
        );
        assert!(html.contains(">Contents<"));
        // Chapters link by .html with their number span.
        assert!(html.contains("href=\"intro.html\""));
        assert!(html.contains("<span class=\"tali-btoc-num\">2</span>"));
        assert!(html.contains(">Methods<"));
        // The part divider is present.
        assert!(html.contains("class=\"tali-btoc-part\">Core<"));
        // Order: Introduction before the "Core" part before Methods.
        let i_intro = html.find("intro.html").unwrap();
        let i_core = html.find(">Core<").unwrap();
        let i_methods = html.find("methods.html").unwrap();
        assert!(
            i_intro < i_core && i_core < i_methods,
            "entries out of order: {html}"
        );
    }

    #[test]
    fn skips_the_landings_own_entry_no_self_link() {
        let html = render_book_toc(&demo_entries(), "index.html", |_| None).unwrap();
        assert!(
            !html.contains("href=\"index.html\""),
            "the landing must not link to itself: {html}"
        );
    }

    #[test]
    fn includes_a_description_blurb_when_present_and_omits_it_otherwise() {
        let desc = |rel: &str| (rel == "methods.tmd").then(|| "How we did it.".to_string());
        let html = render_book_toc(&demo_entries(), "index.html", desc).unwrap();
        assert!(
            html.contains("<p class=\"tali-btoc-desc\">How we did it.</p>"),
            "the chapter with a description must show a blurb: {html}"
        );
        // Results has no description -> exactly one blurb in the whole list.
        assert_eq!(html.matches("tali-btoc-desc").count(), 1);
    }

    #[test]
    fn an_unnumbered_chapter_has_no_number_span() {
        let entries = vec![
            chapter(None, "Preface", "index.html"),
            chapter(None, "Foreword", "foreword.html"),
        ];
        let html = render_book_toc(&entries, "index.html", |_| None).unwrap();
        assert!(html.contains("href=\"foreword.html\""));
        assert!(
            !html.contains("tali-btoc-num"),
            "unnumbered chapter must have no number: {html}"
        );
    }

    #[test]
    fn returns_none_when_no_linkable_chapter_remains() {
        // Only the landing itself + a part divider: nothing to jump to.
        let entries = vec![chapter(None, "Preface", "index.html"), part("Empty")];
        assert!(render_book_toc(&entries, "index.html", |_| None).is_none());
    }

    #[test]
    fn escapes_titles_and_descriptions() {
        let entries = vec![
            chapter(None, "Preface", "index.html"),
            chapter(Some(1), "A <script> chapter", "x.html"),
        ];
        let html = render_book_toc(&entries, "index.html", |_| Some("a & b".to_string())).unwrap();
        assert!(!html.contains("<script>"), "raw markup leaked: {html}");
        assert!(html.contains("A &lt;script&gt; chapter"));
        assert!(html.contains("a &amp; b"));
    }

    #[test]
    fn draft_chapter_carries_a_badge() {
        let mut entries = demo_entries();
        entries.push(BookEntry {
            part: None,
            number: Some(4),
            title: "Appendix".into(),
            rel: "appendix.tmd".into(),
            url: "appendix.html".into(),
            draft: true,
            depth: 0,
            words: 0,
        });
        let html = render_book_toc(&entries, "index.html", |_| None).unwrap();
        assert!(
            html.contains("tali-btoc-draft"),
            "draft row class missing: {html}"
        );
        assert!(
            html.contains("tali-draft-badge"),
            "draft badge missing: {html}"
        );
    }
}
