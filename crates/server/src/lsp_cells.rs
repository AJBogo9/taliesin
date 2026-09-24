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
    /// The scope the body runs in, when it is not the document's: `tali-js.js` compiles each
    /// `{js}` cell the render runs as its own function over `render::JS_CELL_PARAMS`, so a
    /// name declared in another cell is out of scope and a top-level `return` or `await` is
    /// legal. An editor that projects every cell into one file writes `open` on the line
    /// above the body (the fence or an option line) and `close` on the line below it (the
    /// closing fence, or one past the end of an unterminated block), so no line moves.
    /// `None` for a `{python}` cell (one kernel, shared state) and for a display fence.
    ///
    /// Here rather than in the editor because which fences run is the render's answer, and
    /// the parameter list is the runtime's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) wrap: Option<CellWrap>,
}

/// The text that encloses a cell body in the scope it runs in (see [`CellRegion::wrap`]).
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct CellWrap {
    pub(crate) open: String,
    pub(crate) close: String,
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
/// Whether the render runs `fence` as a cell (and so reads its leading options): top level,
/// braced (`{lang}`, not `{.lang}`), with a language. The one statement of that rule in the
/// server; `cell_regions` wraps such a `{js}` cell, and completion offers options in one.
pub(crate) fn is_rendered_cell(
    class: &taliesin_core::lines::Lines,
    fence: &taliesin_core::lines::Fence,
) -> bool {
    class.line(fence.open).depth == 0
        && taliesin_core::render::is_executable_fence(&fence.info)
        && taliesin_core::render::code_lang(&fence.info).is_some()
}

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
            // A `{js}` cell the render runs: top level, braced, the browser's language.
            let wrap = (is_rendered_cell(&class, fence)
                && taliesin_core::render::is_client_lang(&language))
            .then(|| CellWrap {
                open: format!(
                    "(async function ({}) {{",
                    taliesin_core::render::JS_CELL_PARAMS.join(", ")
                ),
                close: "});".to_string(),
            });
            (end > start).then(|| CellRegion {
                language,
                start_line: start,
                end_line: end - 1,
                wrap,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_js_cell_is_wrapped_in_the_function_the_runtime_compiles_it_into() {
        // `tali-js.js` compiles each `{js}` cell as its own AsyncFunction over
        // `JS_CELL_PARAMS`, so a name declared in one cell is out of scope in the next, and
        // a `return` or `await` at the top of a cell is legal.
        let params = taliesin_core::render::JS_CELL_PARAMS.join(", ");
        let got: Vec<(String, Option<CellWrap>)> =
            cell_regions("```{js}\n//| echo: false\nreturn tali;\n```\n")
                .into_iter()
                .map(|r| (r.language, r.wrap))
                .collect();
        assert_eq!(
            got,
            vec![(
                "js".to_string(),
                Some(CellWrap {
                    open: format!("(async function ({params}) {{"),
                    close: "});".to_string(),
                })
            )]
        );
    }

    #[test]
    fn only_a_js_cell_the_render_runs_is_wrapped() {
        // A `{python}` cell shares one kernel with the others, so its scope is the
        // document's. A plain `js` fence, a `{.js}` one and a `{js}` fence in a block quote
        // are samples the page shows and never runs. A `{js}` cell in a `:::` div runs.
        let src = "```{python}\nx=1\n```\n\n```js\nx\n```\n\n```{.js}\nx\n```\n\n\
                   > ```{js}\n> x\n> ```\n\n::: {.callout-note}\n```{js}\nx\n```\n:::\n";
        let got: Vec<(String, bool)> = cell_regions(src)
            .into_iter()
            .map(|r| (r.language, r.wrap.is_some()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("python".to_string(), false),
                ("js".to_string(), false),
                ("js".to_string(), false),
                ("js".to_string(), false),
                ("js".to_string(), true),
            ]
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
