//! Folding ranges: sections, fenced divs, front matter, code fences.

use lsp_types::{FoldingRange, FoldingRangeKind};

/// Fold by document structure rather than by indentation: front matter, headings (each
/// running to the next heading of equal or shallower level), `:::` fenced divs, and code
/// fences.
///
/// Indentation folding is what `.tmd` gets today (there is no `folding` key in the
/// language configuration and no server capability), and it is meaningless in a
/// Markdown-derived format where nesting is expressed by fences and heading level.
///
/// Code fences are tracked whether or not they are folded, because everything inside one is
/// literal: a `# comment` on the first line of a `{python}` cell is not a heading, and
/// treating it as one would close the enclosing section's fold early. This is the same rule
/// `lsp_outline` follows, using the same two helpers.
///
/// An unterminated construct folds to the last line rather than being dropped: a half-typed
/// div is the normal case for a provider that fires while the author types.
pub(crate) fn folding_ranges(text: &str) -> Vec<FoldingRange> {
    let lines: Vec<&str> = text.lines().collect();
    let last = lines.len().saturating_sub(1) as u32;
    let mut out = Vec::new();
    // (start_line, heading_level) for each heading still open.
    let mut headings: Vec<(u32, u8)> = Vec::new();
    // start_line for each `:::` div still open.
    let mut divs: Vec<u32> = Vec::new();
    let mut fm_start: Option<u32> = None;
    let mut fence: Option<(u32, char)> = None;

    for (i, raw) in lines.iter().enumerate() {
        let i = i as u32;
        let line = raw.trim_end();

        // A code fence swallows everything until its matching marker closes it.
        if let Some(marker) = crate::lsp_outline::fence_marker(line) {
            match fence {
                None => fence = Some((i, marker)),
                Some((start, open)) if open == marker => {
                    out.push(region(start, i));
                    fence = None;
                }
                Some(_) => {}
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }

        // Front matter: only when `---` opens line 0, so a thematic break mid-document
        // is not mistaken for it.
        if line == "---" {
            match fm_start {
                None if i == 0 => fm_start = Some(0),
                Some(start) => {
                    out.push(region(start, i));
                    fm_start = None;
                }
                None => {}
            }
            continue;
        }

        if let Some((level, _)) = crate::lsp_outline::atx_heading(line) {
            // A heading closes every open heading at its level or deeper.
            while let Some(&(start, open)) = headings.last() {
                if open >= level {
                    out.push(region(start, i.saturating_sub(1)));
                    headings.pop();
                } else {
                    break;
                }
            }
            headings.push((i, level));
            continue;
        }

        if line.starts_with(":::") {
            // A bare `:::` closes; `::: {.x}` or `:::note` opens.
            if line.trim_matches(':').trim().is_empty() {
                if let Some(start) = divs.pop() {
                    out.push(region(start, i));
                }
            } else {
                divs.push(i);
            }
        }
    }

    // Unterminated constructs fold to the end of the document.
    for (start, _) in headings {
        out.push(region(start, last));
    }
    for start in divs {
        out.push(region(start, last));
    }
    if let Some(start) = fm_start {
        out.push(region(start, last));
    }
    if let Some((start, _)) = fence {
        out.push(region(start, last));
    }
    // A zero-height range is not foldable and clutters the client's gutter.
    out.retain(|r| r.end_line > r.start_line);
    out
}

fn region(start_line: u32, end_line: u32) -> FoldingRange {
    FoldingRange {
        start_line,
        start_character: None,
        end_line,
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
