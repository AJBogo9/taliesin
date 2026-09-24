//! Book-chapter section numbering: each section heading of a numbered chapter carries its
//! number (`N`, `N.1`, `N.1.1`) as a `tali-section-number` span, and a `@sec-` reference
//! to it reads the same number. The render calls [`number_sections`] with the chapter
//! number the orchestrator passes in; nothing else numbers a heading.

use super::*;

/// Number a book chapter's section headings in place, and set every `sec-` number in the
/// page's cross-reference `registry` to the number its heading shows.
///
/// **One list, one pass.** The number a heading shows, the number a same-page `@sec-` reads
/// and the number a cross-page `@sec-` reads (the site harvests this page's registry) all
/// come from here. They were three sites until 2026-09-24, an HTML walk, the render's AST
/// registry and a source scan, and each had its own idea of what a heading is: a setext
/// heading was not one to the scan, a quoted one was one to the HTML walk only, and a
/// heading a callout took for its title was one to the registry and the scan only. A link
/// reading "1.3" then landed on a heading reading "1.2".
///
/// `sections` holds the block ids of the page's section headings, the top-level heading
/// nodes the render's walk emitted (so a heading inside a `:::` div counts, and one in a
/// block quote or list item, one marked `.unnumbered`, raw `<h2>` HTML and cell output do
/// not). Read from the page as FOLDED, through the one tag walker, so a heading a callout
/// consumed for its title is gone: it shows no number, takes none, and its `@sec-` reads a
/// bare "Section".
///
/// `has_title_block`: the render emits a front-matter title block, which carries the
/// chapter number itself and demotes every body heading one level.
pub(crate) fn number_sections(
    blocks: &mut [Block],
    sections: &std::collections::HashSet<String>,
    chapter: u32,
    has_title_block: bool,
    registry: &mut HashMap<String, String>,
) {
    // (block index, offset just past the heading's opening tag, level, its `id`), in
    // document order.
    let mut sites: Vec<(usize, usize, usize, Option<String>)> = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        for t in crate::render::tags(&b.html) {
            let Some(level) = block_heading_level(t.text) else {
                continue;
            };
            let is_section = crate::render::attr_value(&t, "data-block-id")
                .is_some_and(|id| sections.contains(id.as_ref()));
            if is_section {
                let id = crate::render::attr_value(&t, "id").map(|id| id.into_owned());
                sites.push((i, t.at + t.text.len(), usize::from(level), id));
            }
        }
    }
    let levels: Vec<usize> = sites.iter().map(|s| s.2).collect();
    let mut numbering = ChapterNumbering::new(chapter, &levels, has_title_block);
    let numbers: Vec<String> = levels.iter().map(|&l| numbering.next(l)).collect();
    let shown: HashMap<&str, &str> = sites
        .iter()
        .zip(&numbers)
        .filter_map(|(s, n)| Some((s.3.as_deref()?, n.as_str())))
        .collect();
    // A `sec-` label on no section heading (a callout took it for its title) has no number
    // to read, so it reads the bare "Section", as the page shows it.
    for (anchor, number) in registry.iter_mut() {
        if anchor.starts_with("sec-") {
            *number = shown
                .get(anchor.as_str())
                .map(|n| n.to_string())
                .unwrap_or_default();
        }
    }
    // Spliced back to front, so an earlier insertion cannot shift a later offset.
    for ((b, at, _, _), number) in sites.iter().zip(&numbers).rev() {
        blocks[*b]
            .html
            .insert_str(*at, &section_number_span(number));
    }
}

/// Assigns section numbers to one chapter's section headings, in document order.
///
/// The rule: the chapter's own heading, its front-matter title block, else its first
/// heading when nothing above it is shallower, carries the bare chapter number "N".
/// Sections then count from the shallowest level *below* it, so a chapter rooted at
/// `###` numbers "N.1", not "N.0.1", and a titled chapter, whose body headings the title
/// block demoted one level, numbers its first `##` "N.1" too.
pub(crate) struct ChapterNumbering {
    chapter: u32,
    /// The heading level that counter slot 0 corresponds to.
    base: usize,
    counters: [u32; 5],
    /// Whether the chapter's own heading has been consumed. A title block counts as
    /// already consumed: it carries the chapter number itself.
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
/// heading's opening tag. One spelling, shared with the title block's chapter number.
pub(crate) fn section_number_span(number: &str) -> String {
    format!("<span class=\"tali-section-number\">{number}</span> ")
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The number a heading shows, the number a same-page `@sec-` reads and the number a
    /// cross-page `@sec-` reads must be one number, whatever precedes the heading. They came
    /// from three sites (an HTML walk, the render's AST registry, a source scan) that each
    /// had their own idea of what a heading is (audit 2026-09-24, B5): a setext heading and
    /// `##\t` were headings to the first two and not the scan; a heading quoted or in a list
    /// item, and a raw `<h2>`, were headings to the HTML walk only; a heading that became a
    /// callout's title was a heading to the registry and the scan only.
    ///
    /// A section is a heading the document's outline holds: a top-level heading, inside a
    /// `:::` div or not. A heading in a quote or list item is not one (it cannot even carry
    /// an `{#id}`), nor is raw HTML, nor a heading a callout took for its title.
    #[test]
    fn every_reader_of_a_section_number_reads_the_same_number() {
        let cases = [
            ("nothing", "", "1.2"),
            ("setext", "Setext\n------\n", "1.3"),
            ("tab after the hashes", "##\tTabbed\n", "1.3"),
            (
                "a column-page div",
                "::: {.column-page}\n## In a div\n:::\n",
                "1.3",
            ),
            // `.unnumbered` is the class a book chapter's own H1 already reads: on a section
            // it takes no number and advances no counter (audit 2026-09-24, WP13 leftover).
            ("an unnumbered heading", "## Aside {.unnumbered}\n", "1.2"),
            ("a block quote", "> ## Quoted\n", "1.2"),
            ("a list item", "- ## Listed\n", "1.2"),
            ("raw html", "<h2>Raw</h2>\n", "1.2"),
            ("indented code", "    ## Indented\n", "1.2"),
            ("a comment", "<!--\n## Old\n-->\n", "1.2"),
            (
                "a callout title",
                "::: {.callout-note}\n## Consumed {#sec-consumed}\n\nBody.\n:::\n",
                "1.2",
            ),
        ];
        let mut wrong = Vec::new();
        for titled in [false, true] {
            for (shape, before, want) in cases {
                let head = if titled {
                    "---\ntitle: One\n---\n\n"
                } else {
                    "# One\n\n"
                };
                let root = crate::site::tests::write_site(
                    &format!("secagree-{titled}-{}", shape.replace(' ', "-")),
                    &[
                        (
                            "_site.yml",
                            "title: BK\nchapters:\n  - index.tmd\n  - one.tmd\n  - two.tmd\n",
                        ),
                        ("index.tmd", "---\ntitle: Pre\n---\n\nHi.\n"),
                        (
                            "one.tmd",
                            &format!(
                                "{head}See @sec-after.\n\n## First {{#sec-first}}\n\nA.\n\n\
                                 {before}\nB.\n\n## After {{#sec-after}}\n\nC.\n"
                            ),
                        ),
                        ("two.tmd", "# Two\n\nSee @sec-after.\n"),
                    ],
                );
                let site = Site::discover(&root);
                let one = site.render_page("one.tmd").expect("renders");
                let two = site.render_page("two.tmd").expect("renders");
                let _ = std::fs::remove_dir_all(&root);
                let at = one.find("id=\"sec-after\"").expect("the heading exists");
                let body = &one[at..];
                let shown = body[body.find('>').unwrap() + 1..]
                    .strip_prefix("<span class=\"tali-section-number\">")
                    .and_then(|s| s.split('<').next())
                    .unwrap_or("none")
                    .to_string();
                let link = |html: &str| -> String {
                    html.split("#sec-after\" class=\"tali-xref\">")
                        .nth(1)
                        .and_then(|s| s.split('<').next())
                        .unwrap_or("unresolved")
                        .replace("&nbsp;", " ")
                };
                let got = (shown, link(&one), link(&two));
                let expected = (
                    want.to_string(),
                    format!("Section {want}"),
                    format!("Section {want}"),
                );
                if got != expected {
                    wrong.push(format!(
                        "titled={titled} {shape}: {got:?}, want {expected:?}"
                    ));
                }
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    /// Number `levels` in document order, the way [`number_sections`] does.
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
        // …and the rule is relative to the base, so the same shape one level shallower
        // numbers exactly the same.
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
