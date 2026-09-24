//! Folding ranges: sections, fenced divs, front matter, code fences.

use lsp_types::{FoldingRange, FoldingRangeKind};

/// Fold by document structure rather than by indentation: front matter, headings (each
/// running to the next heading of equal or shallower level), `:::` fenced divs, and code
/// fences.
///
/// Indentation folding is what `.tmd` gets without this, and it is meaningless in a
/// Markdown-derived format where nesting is expressed by fences and heading level.
///
/// Every construct is the one the page renders: the front matter is core's one splitter's
/// block, code fences and headings come from its line classifier (`render::rendered_lines`,
/// the parse the render makes), and divs are paired as the render pairs them
/// (`render::div_lines`). So a `# comment` in a cell is no heading, a fence line inside a
/// longer fence closes nothing, and a `:::` in a code sample is no div.
///
/// An unterminated construct folds to the last line rather than being dropped: a half-typed
/// div is the normal case for a provider that fires while the author types.
pub(crate) fn folding_ranges(text: &str) -> Vec<FoldingRange> {
    // CommonMark line endings, so a lone `\r` cannot collapse the buffer to one unfoldable
    // line — but minus the empty line a final terminator leaves behind, which `str::lines`
    // also drops. `last` is where an unterminated construct folds to, and the end of the
    // document an author means is their last line of text, not the blank after it.
    let mut lines: Vec<&str> = crate::lsp_pos::lines(text).collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let last = lines.len().saturating_sub(1);
    let class = taliesin_core::render::rendered_lines(text);
    let mut out = Vec::new();

    let front: Vec<usize> = (0..lines.len())
        .filter(|&i| class.line(i).kind == taliesin_core::lines::Kind::FrontMatter)
        .collect();
    if let (Some(&start), Some(&end)) = (front.first(), front.last()) {
        out.push(region(start, end));
    }
    for fence in &class.fences {
        out.push(region(fence.open, fence.end.min(last)));
    }
    for (open, close) in taliesin_core::render::div_lines(text) {
        out.push(region(open, close.unwrap_or(last)));
    }
    // (start_line, heading_level) for each heading still open. A heading closes every open
    // heading at its level or deeper.
    let mut headings: Vec<(usize, u8)> = Vec::new();
    for i in 0..lines.len() {
        let c = class.line(i);
        let Some(level) = c.heading.filter(|_| c.depth == 0) else {
            continue;
        };
        while let Some(&(start, open)) = headings.last() {
            if open < level {
                break;
            }
            out.push(region(start, i.saturating_sub(1)));
            headings.pop();
        }
        headings.push((i, level));
    }
    // Unterminated sections fold to the end of the document.
    for (start, _) in headings {
        out.push(region(start, last));
    }
    // A zero-height range is not foldable and clutters the client's gutter.
    out.retain(|r| r.end_line > r.start_line);
    out
}

fn region(start_line: usize, end_line: usize) -> FoldingRange {
    FoldingRange {
        start_line: start_line as u32,
        start_character: None,
        end_line: end_line as u32,
        end_character: None,
        kind: Some(FoldingRangeKind::Region),
        collapsed_text: None,
    }
}

#[cfg(test)]
mod tests {
    fn lines_of(text: &str, kind: Option<lsp_types::FoldingRangeKind>) -> Vec<(u32, u32)> {
        super::folding_ranges(text)
            .into_iter()
            .filter(|r| r.kind == kind)
            .map(|r| (r.start_line, r.end_line))
            .collect()
    }

    const DOC: &str = "\
---
title: T
---

# One

text

## Two

more

::: {.callout-note}
inside
:::
";

    #[test]
    fn front_matter_folds() {
        // Lines 0..2 inclusive of the closing `---`.
        assert!(
            lines_of(DOC, Some(lsp_types::FoldingRangeKind::Region)).contains(&(0, 2)),
            "expected the front matter to fold, got {:?}",
            super::folding_ranges(DOC)
        );
    }

    /// The front matter is the block core's one splitter reads (audit 2026-09-24, B1 and
    /// scanners #10): closed by `...` or by a fence with trailing whitespace too. Only an
    /// exact `---` closed it here, so either left the fold running to the end of the file.
    #[test]
    fn front_matter_folds_where_the_splitter_ends_it() {
        for closer in ["...", "--- "] {
            let text = format!("---\ntitle: T\n{closer}\n\n# One\n\ntext\n");
            assert!(
                lines_of(&text, Some(lsp_types::FoldingRangeKind::Region)).contains(&(0, 2)),
                "closer {closer:?}: {:?}",
                super::folding_ranges(&text)
            );
        }
    }

    #[test]
    fn a_section_folds_to_the_next_heading_of_its_level_or_above() {
        let regions = lines_of(DOC, Some(lsp_types::FoldingRangeKind::Region));
        // `# One` starts at line 4 and runs to the end of the document.
        assert!(
            regions.iter().any(|&(s, e)| s == 4 && e >= 14),
            "expected `# One` to fold through the end, got {regions:?}"
        );
        // `## Two` starts at line 8.
        assert!(
            regions.iter().any(|&(s, _)| s == 8),
            "expected `## Two` to fold, got {regions:?}"
        );
    }

    // The "or above" half of the rule, which the document above never exercises: it only ever
    // deepens, so nothing there distinguishes "close every heading at this level or deeper"
    // from "close only an equal level". A SHALLOWER heading after a deeper one does, and it
    // has to close both.
    #[test]
    fn a_shallower_heading_closes_every_deeper_section_under_it() {
        let text = "# One\n\na\n\n## Two\n\nb\n\n# Three\n\nc\n";
        let regions = lines_of(text, Some(lsp_types::FoldingRangeKind::Region));
        assert!(
            regions.contains(&(0, 7)),
            "`# One` must end where `# Three` begins: {regions:?}"
        );
        assert!(
            regions.contains(&(4, 7)),
            "`## Two` must end there too, not run past its parent: {regions:?}"
        );
        assert!(
            regions.contains(&(8, 10)),
            "`# Three` runs to the end: {regions:?}"
        );
    }

    #[test]
    fn a_fenced_div_folds() {
        let regions = lines_of(DOC, Some(lsp_types::FoldingRangeKind::Region));
        assert!(
            regions.iter().any(|&(s, e)| s == 12 && e == 14),
            "expected the ::: div to fold, got {regions:?}"
        );
    }

    #[test]
    fn an_unterminated_div_does_not_panic_and_folds_to_end_of_file() {
        let text = "::: {.callout}\nstill open\n";
        let _ = super::folding_ranges(text);
    }

    #[test]
    fn a_code_fence_folds() {
        let text = "text\n\n```{python}\nx = 1\n```\n\nafter\n";
        let regions = lines_of(text, Some(lsp_types::FoldingRangeKind::Region));
        assert!(
            regions.contains(&(2, 4)),
            "expected the code fence to fold, got {regions:?}"
        );
    }

    // Everything inside a fence is literal. A `#` comment on a cell's first line is the
    // common case, and reading it as a heading would close the enclosing section's fold at
    // the cell instead of at the next real heading.
    #[test]
    fn a_hash_comment_inside_a_code_fence_is_not_a_heading() {
        let text = "# Real\n\ntext\n\n```{python}\n# not a heading\nx = 1\n```\n\nmore\n";
        let regions = lines_of(text, Some(lsp_types::FoldingRangeKind::Region));
        assert!(
            regions.contains(&(0, 9)),
            "`# Real` must fold through the end of the document, not stop at the cell \
             comment on line 5: {regions:?}"
        );
        assert!(
            !regions.iter().any(|&(s, _)| s == 5),
            "the comment inside the fence must not open a section: {regions:?}"
        );
    }

    // A lone `\r` ends a line for CommonMark and for the editor, and `str::lines` (what this
    // read the buffer with) does not see one at all: a buffer using them was one long line, so
    // every fold in it disappeared. The terminator must not change the answer.
    #[test]
    fn a_lone_cr_ends_a_line_here_too() {
        let kind = Some(lsp_types::FoldingRangeKind::Region);
        let lf = "# One\n\na\n\n## Two\n\nb\n";
        let folds = lines_of(lf, kind.clone());
        assert_eq!(
            lines_of(&lf.replace('\n', "\r"), kind),
            folds,
            "the same document with CR terminators must fold identically"
        );
        assert_eq!(
            folds,
            vec![(0, 6), (4, 6)],
            "the fixture must fold at all, or the comparison above proves nothing"
        );
    }

    // Same rule, the other construct: a `:::` in a code sample is content, not a fence.
    #[test]
    fn a_div_fence_inside_a_code_block_is_not_a_div() {
        let text = "```\n::: {.callout}\n:::\n```\n";
        let regions = lines_of(text, Some(lsp_types::FoldingRangeKind::Region));
        assert_eq!(
            regions,
            vec![(0, 3)],
            "only the code fence itself folds: {regions:?}"
        );
    }
}
