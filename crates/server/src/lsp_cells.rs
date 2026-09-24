//! Fenced code regions in a `.tmd`, for embedded-language editor support.

/// One fenced code block's BODY (the fence lines themselves excluded).
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CellRegion {
    /// The language as the document spells it (`python`, `r`, `js`) — NOT an editor's
    /// language id. Mapping `js` to `javascript` is the client's job, because that name is
    /// VS Code's, and this server answers every editor.
    pub(crate) language: String,
    /// 0-based first and last body lines, inclusive. An empty body yields no region.
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
    /// Whether a kernel actually runs this fence: a top-level `{python}` cell, not a plain
    /// `python` display block, not a `{.python}` one, not one in a block quote or list item,
    /// and not `{bash}`.
    ///
    /// Here rather than in the editor because the answer is the render's
    /// (`render::executes_to_kernel`), and an editor deciding for itself would be a second
    /// copy of the executable-language set — the drift that puts a Run button above a
    /// fence nothing can run.
    pub(crate) executable: bool,
}

/// Every fenced code block in `text` that names a language.
///
/// Both spellings count: `{python}` (an executable cell) and a plain `python` info string (a
/// display block). Editor intelligence is useful in both, and the difference — whether the
/// kernel runs it — is not a difference in what the code *means*.
///
/// The fences are the ones core's line classifier finds (`render::rendered_lines`, the parse
/// the page renders from), and the language is the render's reading of the info string
/// (`render::code_lang`): a fence shown inside a longer one, in an HTML comment or in
/// indented code is not a fence, and one in a block quote is.
pub(crate) fn cell_regions(text: &str) -> Vec<CellRegion> {
    let lines: Vec<&str> = crate::lsp_pos::lines(text).collect();
    let class = taliesin_core::render::rendered_lines(text);
    class
        .fences
        .iter()
        .filter_map(|fence| {
            let language = taliesin_core::render::code_lang(&fence.info)?;
            // The body ends before the closing fence, or runs to where the block ends.
            let end = if fence.closed {
                fence.end
            } else {
                fence.end + 1
            };
            let end = end.min(lines.len());
            // Skip the leading `#|` / `//|` / `%%|` option block. These are Taliesin
            // directives, not code: the engine strips them before the cell ever reaches a
            // kernel (`render::strip_cell_options`), and handing them to a language server
            // would make it parse a syntax error instead of the code below.
            // `option_directive` is core's own predicate rather than a second reading of the
            // rule, and "leading only" matters: the same token further down is an ordinary
            // comment and stays.
            let mut start = fence.open + 1;
            while start < end && taliesin_core::render::option_directive(lines[start]).is_some() {
                start += 1;
            }
            let executable = class.line(fence.open).depth == 0
                && taliesin_core::render::is_executable_fence(&fence.info)
                && taliesin_core::render::executes_to_kernel(&language);
            (end > start).then(|| CellRegion {
                language,
                start_line: start,
                end_line: end - 1,
                executable,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_braced_kernel_languages_are_executable() {
        // The Run button hangs off this flag, so the distinction has to be exact: a
        // display block, a `{bash}` cell and a RETIRED cell language all look like code
        // and none of them runs. `{r}` is the retired row, kept here on purpose: it was
        // executable until 2026-08-08, so it is the case a stale executable-language list
        // would get wrong.
        // `{.python}` is the documented display-only spelling (`render::is_executable_fence`).
        let src = "```{python}\nx=1\n```\n\n```python\nx=1\n```\n\n```{bash}\nls\n```\n\n```{r}\nx<-1\n```\n\n```{.python}\nx=1\n```\n";
        let got: Vec<(String, bool)> = cell_regions(src)
            .into_iter()
            .map(|r| (r.language, r.executable))
            .collect();
        assert_eq!(
            got,
            vec![
                ("python".to_string(), true),
                ("python".to_string(), false),
                ("bash".to_string(), false),
                ("r".to_string(), false),
                ("python".to_string(), false),
            ],
            "executable must mean `a kernel runs this`, not `this is code`"
        );
    }

    fn langs(text: &str) -> Vec<(String, usize, usize)> {
        cell_regions(text)
            .into_iter()
            .map(|r| (r.language, r.start_line, r.end_line))
            .collect()
    }

    #[test]
    fn finds_every_cell_and_keeps_them_in_document_order() {
        let text = "```{python}\na\n```\n\n```{r}\nb\n```\n";
        assert_eq!(
            langs(text),
            vec![("python".into(), 1, 1), ("r".into(), 5, 5)]
        );
    }

    #[test]
    fn a_plain_language_fence_counts_too() {
        // Not executable, but the code still means the same thing to a language server.
        assert_eq!(
            langs("```python\nx = 1\n```\n"),
            vec![("python".into(), 1, 1)]
        );
    }

    #[test]
    fn cell_options_after_the_language_do_not_confuse_it() {
        assert_eq!(
            langs("```{python, echo=false}\nx = 1\n```\n"),
            vec![("python".into(), 1, 1)]
        );
    }

    #[test]
    fn a_tilde_fence_works_the_same() {
        assert_eq!(langs("~~~{r}\ny <- 1\n~~~\n"), vec![("r".into(), 1, 1)]);
    }

    #[test]
    fn a_fence_naming_no_language_is_skipped_as_a_unit() {
        // Its body is code: a ``` inside it must not be read as opening a block, or every
        // region after it shifts.
        let text = "```\nnot a language\n```\n\n```{python}\nx = 1\n```\n";
        assert_eq!(langs(text), vec![("python".into(), 5, 5)]);
    }

    // The rule `lsp_links` already follows: a cell shown INSIDE a longer fence is an example
    // of Taliesin syntax, not code to complete in.
    #[test]
    fn a_cell_quoted_inside_a_longer_fence_is_an_example() {
        let text = "````\n```{python}\nx = 1\n```\n````\n";
        assert_eq!(langs(text), Vec::new());
    }

    // `#|` (and `//|` in JS, `%%|` in mermaid) are Taliesin directives, not code. Handing
    // them to a language server means it parses a syntax error — in JS a leading `#|` breaks
    // the whole shadow buffer — and then offers nothing for the real code below.
    #[test]
    fn leading_option_lines_are_directives_not_code() {
        let text = "```{python}\n#| echo: false\n#| label: fig-x\nimport os\n```\n";
        assert_eq!(langs(text), vec![("python".into(), 3, 3)]);
    }

    #[test]
    fn a_cell_that_is_only_option_lines_has_no_code() {
        assert_eq!(langs("```{python}\n#| echo: false\n```\n"), Vec::new());
    }

    // Only the LEADING block is directives; the same token later is an ordinary comment and
    // stays, or the line numbers below it would shift.
    #[test]
    fn a_pipe_comment_below_the_code_is_just_a_comment() {
        let text = "```{python}\nimport os\n#| not an option\n```\n";
        assert_eq!(langs(text), vec![("python".into(), 1, 2)]);
    }

    #[test]
    fn an_empty_cell_has_no_body_to_offer() {
        assert_eq!(langs("```{python}\n```\n"), Vec::new());
    }

    #[test]
    fn finds_an_executable_cell_body_between_its_fences() {
        let text = "intro\n\n```{python}\nx = 1\ny = 2\n```\n\nafter\n";
        let regions = cell_regions(text);
        assert_eq!(regions.len(), 1, "one cell, got {regions:?}");
        assert_eq!(regions[0].language, "python");
        // Body only: the fence lines are not code the language server should see.
        assert_eq!((regions[0].start_line, regions[0].end_line), (3, 4));
    }
}
