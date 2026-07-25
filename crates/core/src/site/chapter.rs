//! Book-chapter section numbering: prefix each heading in a numbered chapter with its
//! section number (`N`, `N.1`, `N.1.1`) as a `tali-section-number` span. Pure block-HTML
//! surgery driven by the chapter number the orchestrator passes in.

use super::*;

/// Prefix each heading in a book chapter with its section number: the chapter's
/// `# H1` becomes "N", and the deeper headings count within it ("N.1", "N.1.1"),
/// emitted as a `tali-section-number` span.
pub(super) fn number_chapter_headings(blocks: &mut [Block], chapter: u32) {
    let has_title_block = blocks.iter().any(|b| is_title_block(&b.html));
    // The numbering base is the shallowest heading BELOW the chapter's own heading, so
    // it has to be known before the first heading is numbered: pre-scan the levels.
    let levels: Vec<usize> = blocks
        .iter()
        .filter(|b| !is_title_block(&b.html))
        .filter_map(|b| heading_level(&b.html))
        .collect();
    let mut numbering = ChapterNumbering::new(chapter, &levels, has_title_block);
    for b in blocks.iter_mut() {
        if is_title_block(&b.html) {
            // The chapter's title-block header is the visible chapter heading; give it the
            // bare chapter number ("N") for continuity, without advancing the h2+ counters.
            b.html = prefix_title_number(&b.html, &chapter.to_string());
        } else if let Some(level) = heading_level(&b.html) {
            let number = numbering.next(level);
            b.html = prefix_heading_number(&b.html, &number);
        }
    }
}

/// Assigns section numbers to one chapter's headings, in document order.
///
/// **Three sites number the same chapter independently** and a link reading "6.1.1"
/// must land on a heading reading "6.1.1": the rendered heading
/// ([`number_chapter_headings`], over emitted HTML), the render-time `@sec-` registry
/// (`render/mod.rs`, over the AST), and the project-wide source scan (`site/xref.rs`,
/// over raw lines). They share this type so the *rule* cannot drift even though their
/// inputs cannot be made identical.
///
/// The rule: the chapter's own heading — its front-matter title block, else its first
/// heading when nothing above it is shallower — carries the bare chapter number "N".
/// Sections then count from the shallowest level *below* it, so a chapter rooted at
/// `###` numbers "N.1", not "N.0.1".
///
/// Note the emitted-HTML site sees levels one deeper than the two source-side sites
/// whenever a title block was emitted (that same gate demotes every body heading).
/// Deriving the base per-site rather than hardcoding `h2` is what makes the slot
/// (`level - base`) come out equal on both sides of that shift.
pub(crate) struct ChapterNumbering {
    chapter: u32,
    /// The heading level that counter slot 0 corresponds to.
    base: usize,
    counters: [u32; 5],
    /// Whether the chapter's own heading has been consumed. A title block counts as
    /// already consumed: [`prefix_title_number`] numbers it separately.
    chapter_heading_seen: bool,
}

impl ChapterNumbering {
    /// `levels`: every heading level in the chapter, in document order, excluding a
    /// front-matter title block (pass `has_title_block` for that instead).
    pub(crate) fn new(chapter: u32, levels: &[usize], has_title_block: bool) -> Self {
        // Without a title block, a leading `# H1` is the chapter's own title, so sections
        // start below it. Specifically an h1: a chapter that opens at `##` has no title
        // heading at all (its `##`s are all sections, numbered N.1, N.2 …), which is what
        // `same_page_sec_ref_uses_hierarchical_number_in_a_chapter` pins.
        let leads_with_chapter_heading = !has_title_block && levels.first() == Some(&1);
        let body_from = usize::from(leads_with_chapter_heading);
        let base = levels[body_from..]
            .iter()
            .copied()
            .min()
            // A chapter whose only heading IS its title has no sections to number; any
            // base does, so keep the conventional h2.
            .unwrap_or(2);
        Self {
            chapter,
            base,
            counters: [0; 5],
            chapter_heading_seen: !leads_with_chapter_heading,
        }
    }

    /// The number for the next heading in document order: "N" for the chapter's own
    /// heading, else "N.c1.…ck" with the counters carried and reset on a shallower
    /// heading.
    pub(crate) fn next(&mut self, level: usize) -> String {
        if !self.chapter_heading_seen {
            self.chapter_heading_seen = true;
            return self.chapter.to_string();
        }
        let i = level.saturating_sub(self.base).min(self.counters.len() - 1);
        self.counters[i] += 1;
        for c in &mut self.counters[i + 1..] {
            *c = 0;
        }
        let mut parts = vec![self.chapter.to_string()];
        parts.extend(self.counters[..=i].iter().map(u32::to_string));
        parts.join(".")
    }
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

    /// Number `levels` in document order, the way one call site would.
    fn number(chapter: u32, levels: &[usize], has_title_block: bool) -> Vec<String> {
        let mut n = ChapterNumbering::new(chapter, levels, has_title_block);
        levels.iter().map(|&l| n.next(l)).collect()
    }

    #[test]
    fn a_titled_chapters_demoted_sections_do_not_gain_a_zero() {
        // The regression: a `title:` chapter has every body heading demoted one level,
        // so the emitted levels are h3/h4 for an author's `##`/`###`. Numbering them
        // against a hardcoded h2 base produced "4.0.1" / "4.0.1.1" on 31 of 32 dogfood
        // chapters, and a `@sec-` link resolved from the SOURCE levels read "4.1".
        assert_eq!(
            number(4, &[3, 4, 4, 3], true),
            ["4.1", "4.1.1", "4.1.2", "4.2"]
        );
        // …and the source-side sites, one level shallower, must agree exactly.
        assert_eq!(
            number(4, &[2, 3, 3, 2], true),
            number(4, &[3, 4, 4, 3], true)
        );
    }

    #[test]
    fn an_untitled_chapters_own_h1_takes_the_bare_chapter_number() {
        // No title block: the leading `#` IS the chapter heading, so it reads "7" and
        // sections count below it. This is the one shape that was already correct.
        assert_eq!(
            number(7, &[1, 2, 3, 2], false),
            ["7", "7.1", "7.1.1", "7.2"]
        );
    }

    #[test]
    fn a_chapter_rooted_deeper_than_h2_still_starts_at_one() {
        // `###`-rooted titled chapter (emitted h4/h5): the base is the shallowest body
        // heading, not h2, so it numbers "N.1" rather than "N.0.0.1".
        assert_eq!(number(2, &[4, 5, 4], true), ["2.1", "2.1.1", "2.2"]);
    }

    #[test]
    fn an_untitled_chapter_rooted_at_h2_has_no_title_heading() {
        // No title block and no leading `#`: there is no chapter heading in the document
        // at all (the number comes from the book's chapter list), so every `##` is a
        // section. Pinned end-to-end by `same_page_sec_ref_uses_hierarchical_number_in_a_chapter`.
        assert_eq!(
            number(2, &[2, 3, 3, 2], false),
            ["2.1", "2.1.1", "2.1.2", "2.2"]
        );
    }

    #[test]
    fn a_titled_chapter_may_carry_a_body_h1() {
        // A titled chapter whose body opens with `#` (demoted to h2): the title block
        // owns "N", so the body `#` is a section, not a second chapter heading.
        assert_eq!(number(9, &[2, 3, 2], true), ["9.1", "9.1.1", "9.2"]);
    }

    #[test]
    fn a_deeper_heading_resets_the_counters_below_it() {
        assert_eq!(
            number(1, &[2, 3, 3, 2, 3], true),
            ["1.1", "1.1.1", "1.1.2", "1.2", "1.2.1"]
        );
    }

    #[test]
    fn numbering_survives_a_chapter_with_no_sections_at_all() {
        assert_eq!(number(5, &[], true), Vec::<String>::new());
        assert_eq!(number(5, &[1], false), ["5"]);
    }

    #[test]
    fn a_second_h1_after_the_chapter_heading_is_a_section() {
        // Two sibling `#`s with no title block: the first is the chapter, the rest count.
        assert_eq!(number(3, &[1, 1, 2], false), ["3", "3.1", "3.1.1"]);
    }

    #[test]
    fn the_base_absorbs_a_uniformly_deep_chapter() {
        // Every section at h6 under an h1 chapter heading: the base is the shallowest
        // BODY heading (6), so these are still first-level sections, not slot-5 ones.
        assert_eq!(number(1, &[1, 6, 6], false), ["1", "1.1", "1.2"]);
    }

    #[test]
    fn levels_deeper_than_the_counter_array_clamp_instead_of_panicking() {
        // A genuine 6-level jump (base 1, a heading at h6) is slot 5, one past the last
        // counter: clamp to the last slot rather than indexing out of bounds.
        assert_eq!(number(1, &[1, 1, 6], false), ["1", "1.1", "1.1.0.0.0.1"]);
    }
}
