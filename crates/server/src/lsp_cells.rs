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
    /// Whether a kernel actually runs this fence: `{python}`/`{r}`, not a plain `python`
    /// display block and not `{bash}`.
    ///
    /// Here rather than in the editor because the answer is
    /// [`crate::exec::kernel_lang`]'s, and an editor deciding for itself would be a second
    /// copy of the executable-language set — the drift that puts a Run button above a
    /// fence nothing can run.
    pub(crate) executable: bool,
}

/// Every fenced code block in `text` that names a language.
///
/// Both spellings count: `{python}` (an executable cell) and a plain `python` info string (a
/// display block). Editor intelligence is useful in both, and the difference — whether the
/// kernel runs it — is not a difference in what the code *means*.
pub(crate) fn cell_regions(text: &str) -> Vec<CellRegion> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((marker, width, info)) = fence_open(lines[i]) else {
            i += 1;
            continue;
        };
        let close = close_line(&lines, i + 1, marker, width).unwrap_or(lines.len());
        // Skip the leading `#|` / `//|` / `%%|` option block. These are Taliesin directives,
        // not code — the engine strips them before the cell ever reaches a kernel
        // (`render::strip_cell_options`), and handing them to a language server would make
        // it parse a syntax error instead of the code below. `option_directive` is core's
        // own predicate rather than a second reading of the rule, and "leading only" matters:
        // the same token further down is an ordinary comment and stays.
        let mut body_start = i + 1;
        while body_start < close
            && taliesin_core::render::option_directive(lines[body_start]).is_some()
        {
            body_start += 1;
        }
        // A fence with no language still has to be skipped as a unit: its contents are code,
        // and a ``` inside it would otherwise be read as opening a block of its own.
        if let Some(language) = language_of(info)
            && close > body_start
        {
            let executable = is_braced(info)
                && crate::exec::kernel_lang(&language.to_ascii_lowercase()).is_some();
            out.push(CellRegion {
                language,
                start_line: body_start,
                end_line: close - 1,
                executable,
            });
        }
        i = close + 1;
    }
    out
}

/// One `true` per line of `text`: is this line part of a fenced code block (the fence lines
/// themselves included)?
///
/// Shared with the table formatter so "what is code" has one implementation here rather than
/// a second regex somewhere else — the same reason `cell_regions` reuses core's
/// `option_directive`. A pipe table shown *inside* a fence is an example of one, and
/// reformatting it would rewrite documentation about tables into a table.
pub(crate) fn code_line_mask(text: &str) -> Vec<bool> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut mask = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        let Some((marker, width, _)) = fence_open(lines[i]) else {
            i += 1;
            continue;
        };
        // An unclosed fence runs to the end of the document, which is also how the renderer
        // treats it — so an author mid-edit does not get their prose reformatted.
        let close = close_line(&lines, i + 1, marker, width).unwrap_or(lines.len() - 1);
        for m in mask.iter_mut().take(close + 1).skip(i) {
            *m = true;
        }
        i = close + 1;
    }
    mask
}

/// `(marker char, run width, info string)` for a line that opens a fence, else `None`.
fn fence_open(line: &str) -> Option<(char, usize, &str)> {
    let t = line.trim_start();
    let marker = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let width = t.chars().take_while(|c| *c == marker).count();
    if width < 3 {
        return None;
    }
    Some((marker, width, &t[width..]))
}

/// The index of the fence that closes a block opened with `width` × `marker`: a fence of at
/// least that width, of the same character, carrying no info string of its own.
fn close_line(lines: &[&str], from: usize, marker: char, width: usize) -> Option<usize> {
    (from..lines.len()).find(|&i| {
        matches!(fence_open(lines[i]), Some((m, w, info))
            if m == marker && w >= width && info.trim().is_empty())
    })
}

/// The language named by a fence's info string: `{python}`, `{python, echo=false}` and a
/// bare `python` all name `python`. `None` when the fence names nothing.
/// Is this info string the `{lang}` (executable cell) spelling rather than a plain
/// `lang` display block? The brace is the whole difference the engine reads.
fn is_braced(info: &str) -> bool {
    info.trim_start().starts_with('{')
}

fn language_of(info: &str) -> Option<String> {
    let t = info.trim();
    let name = match t.strip_prefix('{') {
        Some(inner) => inner.split([',', '}', ' ', '\t']).next().unwrap_or(""),
        None => t.split_whitespace().next().unwrap_or(""),
    };
    let name = name.trim();
    // A language is a bare word. This rejects `{=html}`-style passthrough and the empty
    // info string of a closing fence.
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '+')
    {
        return None;
    }
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_braced_kernel_languages_are_executable() {
        // The Run button hangs off this flag, so the distinction has to be exact: a
        // display block and a `{bash}` cell both look like code and neither runs.
        let src = "```{python}\nx=1\n```\n\n```python\nx=1\n```\n\n                   ```{bash}\nls\n```\n\n```{r}\nx<-1\n```\n";
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
                ("r".to_string(), true),
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
