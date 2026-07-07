//! taliesin's closed body vocabularies + did-you-mean validation for code-cell `#|`
//! options and `:::` callout kinds. Front-matter keys are validated in
//! `crate::frontmatter`; site-config keys in `crate::site::config`. Every validator is
//! purely diagnostic: an unrecognized key still renders exactly as before, plus one
//! located [`Warning`] (click-to-source in the dev panel).
//!
//! The vocabularies are taliesin's OWN. A key not in
//! the relevant set, whether a typo or a legacy term taliesin does not implement, is
//! reported as unknown (with the closest known key when within edit distance 2). This
//! is deliberate: taliesin is its own tool, not a compatibility shim.

use super::Warning;
use crate::frontmatter::unknown_key_message;

/// Cell options taliesin recognizes on a code cell's leading `#|` / `//|` / `%%|`
/// lines (the union across all cell languages; each is read in `cell_option` /
/// `parse_js_opts` / `code_fold` / `emit::code_line_numbers`).
pub(crate) const CELL_OPTION_KEYS: &[&str] = &[
    "echo",
    "include",
    "cache",
    "label",
    "fig-cap",
    "lst-cap",
    "tbl-cap",
    "fig-export",
    "code-fold",
    "code-summary",
    "code-line-numbers",
    "name",   // {js}
    "viewof", // {js}
    "input",  // {js}
];

/// Callout kinds taliesin recognizes (`::: {.callout-<kind>}`).
pub(crate) const CALLOUT_KINDS: &[&str] = &["note", "tip", "warning", "important", "caution"];

/// Theorem-environment kinds taliesin recognizes (`::: {.theorem}`, `::: {.proof}`, …).
/// Unlike callouts there is no namespace prefix, so this set IS the dispatch vocabulary:
/// a div whose class is one of these enters the theorem arm. `proof` is included but is
/// unnumbered + unreferenceable. A misspelled kind has no prefix to anchor a did-you-mean,
/// so it falls through to a plain div (see the design doc).
pub(crate) const THEOREM_KINDS: &[&str] = &[
    "theorem",
    "lemma",
    "corollary",
    "proposition",
    "definition",
    "example",
    "remark",
    "proof",
];

/// Input control types `.input type=` recognizes.
pub(crate) const INPUT_TYPES: &[&str] =
    &["slider", "range", "number", "checkbox", "text", "select"];

/// Enumerate a cell's leading option keys with each key's 0-based line offset within
/// `literal` (the fence body). Mirrors `cell_option`'s scan: only the contiguous
/// leading `#|` / `//|` / `%%|` block, stopping at the first code line.
pub(crate) fn cell_option_keys(literal: &str) -> Vec<(String, usize)> {
    let mut keys = Vec::new();
    for (i, line) in literal.lines().enumerate() {
        let Some(opt) = super::option_directive(line) else {
            break;
        };
        if let Some((k, _)) = opt.split_once(':') {
            keys.push((k.trim().to_string(), i));
        }
    }
    keys
}

/// Validate a code cell's `#|` options against [`CELL_OPTION_KEYS`]. `fence_line` is
/// the 1-based source line of the cell's opening fence (in `file`'s coordinates); an
/// option on the cell's i-th body line is at `fence_line + 1 + i`.
pub(crate) fn validate_cell_options(
    literal: &str,
    fence_line: usize,
    file: Option<String>,
) -> Vec<Warning> {
    cell_option_keys(literal)
        .into_iter()
        .filter(|(k, _)| !CELL_OPTION_KEYS.contains(&k.as_str()))
        .map(|(k, offset)| {
            let line = (fence_line + 1 + offset) as u32;
            Warning::new(unknown_key_message("cell option", &k, CELL_OPTION_KEYS))
                .at(file.clone(), line)
        })
        .collect()
}

/// Validate a callout kind (the `<kind>` in `.callout-<kind>`) against
/// [`CALLOUT_KINDS`]. `line` is the 1-based source line of the div's opening fence.
pub(crate) fn validate_callout_kind(
    kind: &str,
    line: usize,
    file: Option<String>,
) -> Option<Warning> {
    (!CALLOUT_KINDS.contains(&kind)).then(|| {
        Warning::new(unknown_key_message("callout kind", kind, CALLOUT_KINDS)).at(file, line as u32)
    })
}

/// Validate a `.code-walkthrough` container: warn (click-to-source) when it holds no
/// code block, since the sticky panel would render empty. `line` is the 1-based source
/// line of the div's opening fence. Purely diagnostic — the div still renders.
pub(crate) fn validate_walkthrough(
    has_code: bool,
    line: usize,
    file: Option<String>,
) -> Option<Warning> {
    (!has_code).then(|| {
        Warning::new(
            "`.code-walkthrough` has no code block to show in the sticky panel".to_string(),
        )
        .at(file, line as u32)
    })
}

/// Validate a `.panel-tabset` container: warn (click-to-source) when it has no headings,
/// so it would render no tabs. `line` is the 1-based source line of the opening fence.
/// Purely diagnostic — the div still renders its content.
pub(crate) fn validate_tabset(
    has_tabs: bool,
    line: usize,
    file: Option<String>,
) -> Option<Warning> {
    (!has_tabs).then(|| {
        Warning::new(
            "`.panel-tabset` has no headings, so it renders no tabs (add `##` headings)"
                .to_string(),
        )
        .at(file, line as u32)
    })
}

/// Validate a `.input` reactive-control container (located, click-to-source). Warns when
/// `name` is missing (the control can't feed the reactive graph), when `type` is unknown
/// (with a did-you-mean), or when a `select` has no `options`. Purely diagnostic — the
/// div still renders.
pub(crate) fn validate_input(
    name: Option<&str>,
    kind: Option<&str>,
    options: Option<&str>,
    line: usize,
    file: Option<String>,
) -> Vec<Warning> {
    let mut out = Vec::new();
    if name.unwrap_or("").trim().is_empty() {
        out.push(
            Warning::new("`.input` needs a `name=` to feed the reactive graph".to_string())
                .at(file.clone(), line as u32),
        );
    }
    if let Some(t) = kind
        && !INPUT_TYPES.contains(&t)
    {
        out.push(
            Warning::new(unknown_key_message("input type", t, INPUT_TYPES))
                .at(file.clone(), line as u32),
        );
    }
    if kind == Some("select") && options.unwrap_or("").trim().is_empty() {
        out.push(
            Warning::new("`.input type=select` needs `options=\"a,b,c\"`".to_string())
                .at(file, line as u32),
        );
    }
    out
}

/// Validate a `.scrolly` container (located, click-to-source). Warns when there is no
/// sticky stage block or no `.step` divs to scroll through. Purely diagnostic — it still
/// renders. Mirrors `validate_walkthrough`.
pub(crate) fn validate_scrolly(
    has_stage: bool,
    has_steps: bool,
    line: usize,
    file: Option<String>,
) -> Vec<Warning> {
    let mut out = Vec::new();
    if !has_stage {
        out.push(
            Warning::new(
                "`.scrolly` has no sticky stage (add a figure or `{js}` cell)".to_string(),
            )
            .at(file.clone(), line as u32),
        );
    }
    if !has_steps {
        out.push(
            Warning::new("`.scrolly` has no `.step` divs to scroll through".to_string())
                .at(file, line as u32),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_only_the_leading_option_block() {
        let lit = "#| echo: false\n#| labl: x\nprint(1)\n#| late: y\n";
        let keys: Vec<_> = cell_option_keys(lit).into_iter().collect();
        assert_eq!(keys, vec![("echo".to_string(), 0), ("labl".to_string(), 1)]);
    }

    #[test]
    fn flags_unknown_cell_option_with_did_you_mean_and_location() {
        // Fence is on file line 20, so the option on body line 1 is file line 22.
        let w = validate_cell_options("#| echo: false\n#| labl: x\n", 20, Some("p.tmd".into()));
        assert_eq!(w.len(), 1, "only `labl` is unknown, got: {w:?}");
        assert_eq!(
            w[0].message,
            "unknown cell option `labl` (did you mean `label`?)"
        );
        assert_eq!(w[0].file.as_deref(), Some("p.tmd"));
        assert_eq!(w[0].line, Some(22));
    }

    #[test]
    fn recognized_cell_options_are_silent() {
        let lit =
            "#| echo: false\n#| label: fig-x\n#| fig-cap: A\n#| code-fold: true\n//| name: n\n";
        assert!(
            validate_cell_options(lit, 1, None).is_empty(),
            "all keys recognized"
        );
    }

    #[test]
    fn unknown_callout_kind_is_flagged_and_located() {
        let w = validate_callout_kind("importnat", 7, None).expect("an unknown-kind warning");
        assert_eq!(
            w.message,
            "unknown callout kind `importnat` (did you mean `important`?)"
        );
        assert_eq!(w.line, Some(7));
        assert!(
            validate_callout_kind("note", 7, None).is_none(),
            "note is recognized"
        );
    }

    #[test]
    fn walkthrough_without_code_block_is_flagged_and_located() {
        let w = validate_walkthrough(false, 12, Some("w.tmd".into())).expect("a no-code warning");
        assert!(w.message.contains("no code block"), "got: {}", w.message);
        assert_eq!(w.line, Some(12));
        assert_eq!(w.file.as_deref(), Some("w.tmd"));
        assert!(
            validate_walkthrough(true, 12, None).is_none(),
            "silent when a code block is present"
        );
    }

    #[test]
    fn tabset_without_headings_is_flagged_and_located() {
        let w = validate_tabset(false, 4, Some("p.tmd".into())).expect("a no-tabs warning");
        assert!(w.message.contains("no headings"), "got: {}", w.message);
        assert_eq!(w.line, Some(4));
        assert_eq!(w.file.as_deref(), Some("p.tmd"));
        assert!(
            validate_tabset(true, 4, None).is_none(),
            "silent when headings are present"
        );
    }

    #[test]
    fn input_without_name_is_flagged() {
        let w = validate_input(None, Some("slider"), None, 4, Some("d.tmd".into()));
        assert_eq!(w.len(), 1);
        assert_eq!(
            w[0].message,
            "`.input` needs a `name=` to feed the reactive graph"
        );
        assert_eq!(w[0].line, Some(4));
    }

    #[test]
    fn input_unknown_type_has_did_you_mean() {
        let w = validate_input(Some("k"), Some("slidr"), None, 2, None);
        assert_eq!(w.len(), 1);
        assert_eq!(
            w[0].message,
            "unknown input type `slidr` (did you mean `slider`?)"
        );
    }

    #[test]
    fn input_select_without_options_is_flagged() {
        let w = validate_input(Some("c"), Some("select"), None, 9, None);
        assert_eq!(w.len(), 1);
        assert_eq!(
            w[0].message,
            "`.input type=select` needs `options=\"a,b,c\"`"
        );
    }

    #[test]
    fn input_valid_slider_is_clean() {
        assert!(validate_input(Some("k"), Some("slider"), None, 1, None).is_empty());
        assert!(validate_input(Some("c"), Some("select"), Some("a,b"), 1, None).is_empty());
    }

    #[test]
    fn scrolly_without_stage_is_flagged() {
        let w = validate_scrolly(false, true, 3, Some("s.tmd".into()));
        assert_eq!(w.len(), 1);
        assert!(
            w[0].message.contains("no sticky stage"),
            "got: {}",
            w[0].message
        );
        assert_eq!(w[0].line, Some(3));
    }

    #[test]
    fn scrolly_without_steps_is_flagged() {
        let w = validate_scrolly(true, false, 5, None);
        assert_eq!(w.len(), 1);
        assert!(w[0].message.contains("no `.step`"), "got: {}", w[0].message);
    }

    #[test]
    fn scrolly_complete_is_clean() {
        assert!(validate_scrolly(true, true, 1, None).is_empty());
    }
}
