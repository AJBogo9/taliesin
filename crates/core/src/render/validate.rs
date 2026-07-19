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
use crate::frontmatter::{closest, unknown_key_message};

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

/// Structural + deck feature classes a `:::` fenced div can carry. This is **not** a closed
/// vocabulary — a div may carry any custom class (styled by the author's own CSS) — so this list
/// only anchors the *did-you-mean* (`validate_div_class`); it never rejects. Keep in sync with the
/// `.class` dispatch in `render/divs.rs` and the deck classes in `assets/css/deck.css`; a
/// `vocab.rs` test pins `vocab::div_classes()`'s names as a subset.
pub(crate) const DIV_FEATURE_CLASSES: &[&str] = &[
    "panel-tabset",
    "code-walkthrough",
    "scrolly",
    "magic-move",
    "step",
    "column-margin",
    "aside",
    "sidenote",
    "marginnote",
    "fragment",
    "incremental",
    // Fragment EFFECT modifiers: real styled classes (`deck.css`) that ride alongside
    // `.fragment` (`::: {.fragment .fade-out}`), so a typo in the effect (`.fade-ot`,
    // `.hihglight`) is exactly a deck author's fiddly mistake. Anchored here for the
    // did-you-mean; like `.fragment`/`.incremental` they are deck-authoring modifiers, so
    // (matching that family) they are NOT offered in the editor vocab (`vocab::DIV_CLASS_NAMES`).
    "fade-out",
    "highlight",
    "notes",
    "columns",
    "column",
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

/// A misspelled feature/theorem `:::` class → a located "did you mean". Fired from the generic-div
/// fall-through in `build_container` (the classes that matched no feature arm). Only a *near-miss*
/// of a known feature class ([`DIV_FEATURE_CLASSES`] ∪ [`THEOREM_KINDS`], edit distance ≤ 2) warns:
/// an exactly-known class (a legit generic like `.aside`/`.fragment`) and a genuine custom class
/// (far from every known name) both stay silent, since div classes are an *open* vocabulary. At
/// most one warning per div (the first offending class), and purely diagnostic — the div still
/// renders with its given class. `line` is the 1-based source line of the opening fence.
pub(crate) fn validate_div_class(
    classes: &[String],
    line: usize,
    file: Option<String>,
) -> Option<Warning> {
    let known: Vec<&'static str> = DIV_FEATURE_CLASSES
        .iter()
        .copied()
        .chain(THEOREM_KINDS.iter().copied())
        .collect();
    classes.iter().find_map(|c| {
        if known.contains(&c.as_str()) {
            return None;
        }
        closest(c, &known).map(|s| {
            Warning::new(format!("unknown div class `{c}` (did you mean `{s}`?)"))
                .at(file.clone(), line as u32)
        })
    })
}

/// Validate a `.column` div's `width=` (located, click-to-source). Column widths are ignored:
/// a `.columns` grid lays its `.column` children out EQUAL-width, so a reveal/Quarto
/// `::: {.column width="70%"}` habit silently does nothing. Warn and name the equal-width
/// behaviour + the fixed-count knob. `None` when the div is not a `.column` or carries no
/// non-empty `width=`. `line` is the 1-based source line of the opening fence.
pub(crate) fn validate_column_width(
    classes: &[String],
    width: Option<&str>,
    line: usize,
    file: Option<String>,
) -> Option<Warning> {
    let width = width.map(str::trim).filter(|w| !w.is_empty())?;
    (classes.iter().any(|c| c == "column")).then(|| {
        Warning::new(format!(
            "`.column width=\"{width}\"` is ignored: a `.columns` grid lays its columns out \
             equal-width. Remove `width=`, or use `::: {{.columns ncol=N}}` (or `{{layout-ncol=N}}`) \
             for a fixed column count."
        ))
        .at(file, line as u32)
    })
}

/// Validate a fenced div that turned out EMPTY (no blocks between its `:::` fences). An empty
/// GENERIC div is harmless (it's dropped), but an empty div that names a real feature — a
/// `.input` reactive control, a `.callout-*`, a `.panel-tabset`, a theorem, … — is almost
/// always a mistake: the feature renders nothing, silently. Warn (located, click-to-source),
/// with a pointed hint for `.input` (whose real form is the `{{< input >}}` shortcode, not a
/// div — the exact confusion this closes). `None` when the empty div carries no known feature
/// class, so a genuinely-empty custom/plain div stays silent (open vocabulary). `line` is the
/// 1-based source line of the opening fence.
pub(crate) fn validate_empty_feature_div(
    classes: &[String],
    line: usize,
    file: Option<String>,
) -> Option<Warning> {
    let feature = classes.iter().find(|c| {
        let c = c.as_str();
        c == "input"
            || c.starts_with("callout-")
            || DIV_FEATURE_CLASSES.contains(&c)
            || THEOREM_KINDS.contains(&c)
    })?;
    let hint = if feature == "input" {
        " — the reactive input control is the `{{< input name=\"…\" >}}` shortcode, not a `:::` div"
    } else {
        ""
    };
    Some(
        Warning::new(format!(
            "empty `.{feature}` block: no content between the `:::` fences, so it renders nothing{hint}"
        ))
        .at(file, line as u32),
    )
}

/// Validate a `.step lines=` value (located, click-to-source). The `|` is the STEP separator
/// of a deck/listing `code-line-numbers="1|2-3"` spec, but a `.step` is already one step, so
/// its own `lines=` is parsed as comma-separated ranges only (`walkthrough.js`/`scrolly.js`).
/// A `|` therefore matches neither a range nor a number and silently focuses zero lines — a
/// deck author's muscle-memory trap. Purely diagnostic — the step still renders. `line` is the
/// 1-based source line of the div's opening fence.
pub(crate) fn validate_step_lines(
    spec: &str,
    line: usize,
    file: Option<String>,
) -> Option<Warning> {
    spec.contains('|').then(|| {
        Warning::new(format!(
            "`.step lines=\"{spec}\"` uses `|` (the step separator for a deck's \
             `code-line-numbers=`), but a `.step`'s own `lines=` focuses one step and takes \
             comma-separated ranges only (e.g. `3-5,8`), so the `|` groups highlight nothing. \
             Split the pipe groups into separate `.step` blocks."
        ))
        .at(file, line as u32)
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
    fn validate_div_class_suggests_near_miss_only() {
        let s = |c: &str| vec![c.to_string()];
        // A near-miss of a feature class → located "did you mean".
        let w = validate_div_class(&s("fragmnet"), 3, None).expect("a near-miss warning");
        assert_eq!(
            w.message,
            "unknown div class `fragmnet` (did you mean `fragment`?)"
        );
        assert_eq!(w.line, Some(3));
        // A near-miss of a THEOREM kind (the case validate.rs's own comment calls out).
        assert!(
            validate_div_class(&s("theorm"), 3, None).is_some(),
            "a misspelled theorem kind should be caught"
        );
        // An exactly-known class is legit → silent.
        assert!(
            validate_div_class(&s("aside"), 3, None).is_none(),
            "known class is silent"
        );
        // A genuine custom class (far from every known name) → silent (open vocabulary).
        assert!(
            validate_div_class(&s("my-widget"), 3, None).is_none(),
            "a far custom class must not warn"
        );
        // Only the first offending class warns (no pile-up).
        assert_eq!(
            validate_div_class(&s("fragmnet"), 3, None)
                .into_iter()
                .count(),
            1
        );
    }

    #[test]
    fn fragment_effect_modifiers_are_did_you_mean_anchors() {
        // PL9: `.fade-out`/`.highlight` are real styled fragment effects (`deck.css`), so a
        // typo in the effect modifier alongside a legit `.fragment` must draw a did-you-mean
        // instead of silently rendering a plain fragment. Before this, the effect names weren't
        // known anchors, so `.fade-ot`/`.hihglight` matched nothing and stayed silent.
        let div = |classes: &[&str]| classes.iter().map(|c| c.to_string()).collect::<Vec<_>>();
        let w = validate_div_class(&div(&["fragment", "fade-ot"]), 5, None)
            .expect("a mistyped fragment effect warns");
        assert_eq!(
            w.message,
            "unknown div class `fade-ot` (did you mean `fade-out`?)"
        );
        assert!(
            validate_div_class(&div(&["fragment", "hihglight"]), 5, None)
                .is_some_and(|w| w.message.contains("did you mean `highlight`?")),
            "a mistyped `.highlight` effect is caught too"
        );
        // The correct effect spellings are legit → silent (known anchors, not typos).
        assert!(
            validate_div_class(&div(&["fragment", "fade-out"]), 5, None).is_none(),
            "the real `.fade-out` effect is silent"
        );
        assert!(
            validate_div_class(&div(&["fragment", "highlight"]), 5, None).is_none(),
            "the real `.highlight` effect is silent"
        );
    }

    #[test]
    fn validate_column_width_warns_only_on_a_column_with_a_width() {
        let div = |classes: &[&str]| classes.iter().map(|c| c.to_string()).collect::<Vec<_>>();
        // PL3: a `.column width=` is silently equalized — warn, echoing the width + naming the fix.
        let w = validate_column_width(&div(&["column"]), Some("70%"), 3, None)
            .expect("`.column width=` warns");
        assert!(
            w.message.contains("width=\"70%\"") && w.message.contains("equal-width"),
            "echoes the width + names the behaviour: {}",
            w.message
        );
        // Silent: a `.column` with no width, an empty width, or a non-`.column` div with a width.
        assert!(validate_column_width(&div(&["column"]), None, 3, None).is_none());
        assert!(validate_column_width(&div(&["column"]), Some("  "), 3, None).is_none());
        assert!(validate_column_width(&div(&["columns"]), Some("70%"), 3, None).is_none());
    }

    #[test]
    fn validate_empty_feature_div_warns_by_class_and_points_input_at_the_shortcode() {
        let div = |classes: &[&str]| classes.iter().map(|c| c.to_string()).collect::<Vec<_>>();
        // PL2: an empty `.input` div is the reach-for-a-div-instead-of-the-shortcode trap; warn
        // and point at the shortcode.
        let w = validate_empty_feature_div(&div(&["input"]), 4, Some("d.tmd".into()))
            .expect("empty .input warns");
        assert!(
            w.message.contains("empty `.input`") && w.message.contains("{{< input"),
            "names the class + points at the shortcode: {}",
            w.message
        );
        assert_eq!(w.line, Some(4));
        // Other feature classes warn generically (callout/tabset/theorem), no shortcode hint.
        for c in ["callout-note", "panel-tabset", "theorem", "scrolly"] {
            let w = validate_empty_feature_div(&div(&[c]), 1, None)
                .unwrap_or_else(|| panic!("empty .{c} should warn"));
            assert!(
                w.message.contains(&format!("empty `.{c}`")) && !w.message.contains("{{< input")
            );
        }
        // A plain/custom empty div (no known feature class) stays silent — open vocabulary.
        assert!(validate_empty_feature_div(&div(&["my-widget"]), 1, None).is_none());
        assert!(validate_empty_feature_div(&div(&[]), 1, None).is_none());
    }

    #[test]
    fn validate_step_lines_warns_only_on_the_pipe_step_separator() {
        // PL7: a `|` in a `.step lines=` is a deck `code-line-numbers=` habit that the step's
        // comma-only parser focuses to zero lines — warn, located.
        let w = validate_step_lines("1|2-3", 7, Some("d.tmd".into())).expect("`|` warns");
        assert!(
            w.message.contains("step separator") && w.message.contains("lines=\"1|2-3\""),
            "names the separator + echoes the spec: {}",
            w.message
        );
        assert_eq!(w.line, Some(7), "located at the fence line");
        // The valid grammars — comma-separated ranges/numbers, a plain range, `all` — are silent.
        assert!(validate_step_lines("3-5,8", 7, None).is_none(), "commas ok");
        assert!(validate_step_lines("6-8", 7, None).is_none(), "a range ok");
        assert!(validate_step_lines("all", 7, None).is_none(), "`all` ok");
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
