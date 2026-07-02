//! Book-chapter section numbering: prefix each heading in a numbered chapter with its
//! section number (`N`, `N.1`, `N.1.1`) as a `tali-section-number` span. Pure block-HTML
//! surgery driven by the chapter number the orchestrator passes in.

use super::*;

/// Prefix each heading in a book chapter with its section number: the chapter's
/// `# H1` becomes "N", and the deeper headings count within it ("N.1", "N.1.1"),
/// emitted as a `tali-section-number` span.
pub(super) fn number_chapter_headings(blocks: &mut [Block], chapter: u32) {
    let mut counters = [0u32; 5]; // counters[0] = h2, [1] = h3, …
    for b in blocks.iter_mut() {
        if is_title_block(&b.html) {
            // The chapter's title-block header is the visible chapter heading; give it the
            // bare chapter number ("N") for continuity, without advancing the h2+ counters.
            b.html = prefix_title_number(&b.html, &chapter.to_string());
        } else if let Some(level) = heading_level(&b.html) {
            let number = section_number(chapter, level, &mut counters);
            b.html = prefix_heading_number(&b.html, &number);
        }
    }
}

/// The number for a heading at `level` within chapter `chapter`: the chapter's H1
/// is "N", a level-`k` heading is "N.c2.…ck", with `counters` (h2..h6) carried and
/// reset on a shallower heading. Shared by the render-time numbering and the
/// source scan that builds the cross-reference registry, so they never diverge.
pub(crate) fn section_number(chapter: u32, level: usize, counters: &mut [u32; 5]) -> String {
    if level <= 1 {
        return chapter.to_string();
    }
    let i = (level - 2).min(counters.len() - 1);
    counters[i] += 1;
    for c in &mut counters[i + 1..] {
        *c = 0;
    }
    let mut parts = vec![chapter.to_string()];
    parts.extend(counters[..=i].iter().map(u32::to_string));
    parts.join(".")
}

/// The heading level (1–6) of a block whose root element is `<hN …>`, else `None`.
/// Delegates to the render crate's parser so the two never diverge.
fn heading_level(html: &str) -> Option<usize> {
    block_heading_level(html).map(usize::from)
}

/// Insert a `tali-section-number` span just after a heading's opening tag.
fn prefix_heading_number(html: &str, number: &str) -> String {
    match html.find('>') {
        Some(i) => format!(
            "{}<span class=\"tali-section-number\">{number}</span> {}",
            &html[..=i],
            &html[i + 1..]
        ),
        None => html.to_string(),
    }
}

/// Whether a block is the front-matter `title:` block (a `<header class="tali-title-block">`,
/// not a markdown heading — so `heading_level` never sees it as an `<h1>`).
fn is_title_block(html: &str) -> bool {
    html.contains("class=\"tali-title-block\"")
}

/// Number a numbered chapter's TITLE: insert the chapter number just inside the
/// title block's `<h1 class="title">`, so the chapter reads "N Title" and its `N.1`
/// subsections no longer look like numbers appearing from nowhere.
fn prefix_title_number(html: &str, number: &str) -> String {
    let marker = "<h1 class=\"title\">";
    match html.find(marker) {
        Some(i) => {
            let at = i + marker.len();
            format!(
                "{}<span class=\"tali-section-number\">{number}</span> {}",
                &html[..at],
                &html[at..]
            )
        }
        None => html.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_the_chapter_title_block() {
        let title = "<header class=\"tali-title-block\" data-block-id=\"qmd-title-block\">\
            <h1 class=\"title\">Executable content</h1></header>";
        assert_eq!(
            prefix_title_number(title, "3"),
            "<header class=\"tali-title-block\" data-block-id=\"qmd-title-block\">\
             <h1 class=\"title\"><span class=\"tali-section-number\">3</span> Executable content</h1></header>"
        );
    }

    #[test]
    fn detects_the_title_block_but_not_a_heading() {
        assert!(is_title_block(
            "<header class=\"tali-title-block\"><h1 class=\"title\">T</h1></header>"
        ));
        assert!(!is_title_block("<h2 id=\"x\">A section</h2>"));
    }
}
