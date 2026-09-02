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
        .flat_map(|b| heading_sites(&b.html))
        .map(|(_, level)| level)
        .collect();
    let mut numbering = ChapterNumbering::new(chapter, &levels, has_title_block);
    for b in blocks.iter_mut() {
        if is_title_block(&b.html) {
            // The chapter's title-block header is the visible chapter heading; give it the
            // bare chapter number ("N") for continuity, without advancing the h2+ counters.
            b.html = prefix_title_number(&b.html, &chapter.to_string());
            continue;
        }
        let sites = heading_sites(&b.html);
        if sites.is_empty() {
            continue;
        }
        // Numbers are taken in document order and spliced back to front, so an earlier
        // insertion cannot shift a later offset.
        let numbers: Vec<String> = sites.iter().map(|&(_, l)| numbering.next(l)).collect();
        for (&(at, _), number) in sites.iter().zip(&numbers).rev() {
            b.html.insert_str(at, &section_number_span(number));
        }
    }
}

/// Every heading inside one block's emitted HTML: the byte offset just past its opening
/// tag's `>` (where the number span goes) and its level, in document order.
///
/// **Not `block_heading_level(&b.html)`, which only asks about a block's ROOT element.** A
/// `:::` container concatenates its children into one `html`, so a `## Beta {#sec-beta}`
/// inside a `.column-page` or a `layout-ncol` grid is a heading in the middle of a `<div>`
/// block and answered `None` — it drew no number and advanced no counter, while the
/// render-time `@sec-` registry (which walks the AST, before any folding) went on counting
/// it. The two sites then disagreed by one for the rest of the chapter: `@sec-beta` read
/// "Section 1.2" and landed on an unnumbered heading, while the NEXT heading visibly
/// displayed "1.2". Those two sites must agree — a link reading "6.1.1" has to land on a
/// heading reading "6.1.1" — which is the whole reason they share [`ChapterNumbering`].
///
/// Read through `render::tags`, so a heading spelled inside a `<script>` body or shown as
/// escaped text in a code sample is not mistaken for one (the walker knows tag from text and
/// skips raw-text element bodies); and each tag's level comes from `block_heading_level`, so
/// what counts as a heading still has exactly one definition.
fn heading_sites(html: &str) -> Vec<(usize, usize)> {
    crate::render::tags(html)
        .filter_map(|t| {
            let level = block_heading_level(t.text)?;
            Some((t.at + t.text.len(), usize::from(level)))
        })
        .collect()
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

/// The `tali-section-number` span, trailing space included, as it is spliced in just after a
/// heading's opening tag. One spelling, shared with [`prefix_title_number`].
fn section_number_span(number: &str) -> String {
    format!("<span class=\"tali-section-number\">{number}</span> ")
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
                "{}{}{}",
                &html[..at],
                section_number_span(number),
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
        let title = "<header class=\"tali-title-block\" data-block-id=\"tali-title-block\">\
            <h1 class=\"title\">Executable content</h1></header>";
        assert_eq!(
            prefix_title_number(title, "3"),
            "<header class=\"tali-title-block\" data-block-id=\"tali-title-block\">\
             <h1 class=\"title\"><span class=\"tali-section-number\">3</span> Executable content</h1></header>"
        );
    }

    /// A heading inside a `:::` container is a heading in the MIDDLE of a container block's
    /// html, so the root-element test could not see it: it drew no number and advanced no
    /// counter, while the render-time `@sec-` registry (which walks the AST, before folding)
    /// went on counting it. The two then disagreed by one for the rest of the chapter.
    ///
    /// Measured before the fix, on exactly this book: Beta rendered with no number at all
    /// while `@sec-beta` read "Section 1.2", and Gamma visibly displayed "1.2" while
    /// `@sec-gamma` read "Section 1.3". A reader following the link for Beta landed on an
    /// unnumbered heading, with a different heading on the page showing the number they
    /// clicked. This asserts the lockstep in both directions, per heading.
    #[test]
    fn a_heading_inside_a_container_is_numbered_in_document_order() {
        let root = crate::site::tests::write_site(
            "chapterfolded",
            &[
                (
                    "_site.yml",
                    "title: BK\nchapters:\n  - index.tmd\n  - two.tmd\n",
                ),
                ("index.tmd", "---\ntitle: Intro\n---\n\nHi.\n"),
                (
                    "two.tmd",
                    "---\ntitle: Two\n---\n\nSee @sec-beta and @sec-gamma.\n\n\
                     ## Alpha {#sec-alpha}\n\nA.\n\n\
                     ::: {.column-page}\n## Beta {#sec-beta}\n\nB.\n:::\n\n\
                     ## Gamma {#sec-gamma}\n\nG.\n",
                ),
            ],
        );
        let html = Site::discover(&root)
            .render_page("two.tmd")
            .expect("renders");
        // The number a heading VISIBLY shows, by its anchor.
        let shown = |id: &str| -> String {
            let at = html
                .find(&format!("id=\"{id}\""))
                .unwrap_or_else(|| panic!("heading {id} exists: {html}"));
            let after = &html[at..];
            let body = &after[after.find('>').expect("the tag closes") + 1..];
            body.strip_prefix("<span class=\"tali-section-number\">")
                .unwrap_or_else(|| panic!("{id} is unnumbered: {body:.120}"))
                .split('<')
                .next()
                .unwrap()
                .to_string()
        };
        // The number an `@sec-` link RESOLVES to, by its href.
        let resolved = |id: &str| -> String {
            let at = html
                .find(&format!("href=\"#{id}\" class=\"tali-xref\""))
                .unwrap_or_else(|| panic!("xref to {id} resolved: {html}"));
            html[at..]
                .split_once('>')
                .and_then(|(_, r)| r.split('<').next())
                .expect("link text")
                .replace("&nbsp;", " ")
        };
        assert_eq!(shown("sec-alpha"), "1.1");
        assert_eq!(shown("sec-beta"), "1.2", "the folded heading is numbered");
        assert_eq!(
            shown("sec-gamma"),
            "1.3",
            "and the folded heading advanced the counter, so Gamma is not also 1.2"
        );
        assert_eq!(resolved("sec-beta"), "Section 1.2");
        assert_eq!(resolved("sec-gamma"), "Section 1.3");
        let _ = std::fs::remove_dir_all(&root);
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
