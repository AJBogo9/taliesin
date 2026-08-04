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
    "code-fold",
    "code-summary",
    "code-line-numbers",
    "name",   // {js}
    "viewof", // {js}
    "input",  // {js}
    "trace",  // {python}/{js} inside `::: {.debug}`
];

/// Callout kinds taliesin recognizes (`::: {.callout-<kind>}`).
///
/// Five kinds shrank to three on 2026-08-03 (visual minimalism pass, task 12):
/// `important` and `caution` were a coloured box with a different word in it, the same
/// as `warning`, and no reader could reliably tell the three apart. `important` and
/// `caution` are registered in `frontmatter::RETIRED_KEYS` (scope `"callout kind"`) so a
/// document that still writes one gets a located removal note instead of silence.
pub(crate) const CALLOUT_KINDS: &[&str] = &["note", "tip", "warning"];

/// Theorem-environment kinds taliesin recognizes (`::: {.theorem}`, `::: {.proof}`, …).
/// Unlike callouts there is no namespace prefix, so this set IS the dispatch vocabulary:
/// a div whose class is one of these enters the theorem arm. `proof` is included but is
/// unnumbered + unreferenceable. A misspelled kind has no prefix to anchor a did-you-mean,
/// so it falls through to a plain div (see the design doc).
///
/// Eight kinds shrank to five on 2026-08-03 (visual minimalism pass, task 14): `example`,
/// `proposition` and `remark` were never cross-referenced by any document in the tree. All
/// three are registered in [`RETIRED_DIV_CLASSES`] below — the same silent-fallthrough risk
/// the doc comment above already names, so without that entry a leftover
/// `.example`/`.proposition`/`.remark` gets no diagnostic at all.
pub(crate) const THEOREM_KINDS: &[&str] = &["theorem", "lemma", "corollary", "definition", "proof"];

/// Structural + deck feature classes a `:::` fenced div can carry. This is **not** a closed
/// vocabulary — a div may carry any custom class (styled by the author's own CSS) — so this list
/// only anchors the *did-you-mean* (`validate_div_class`); it never rejects. Keep in sync with the
/// `.class` dispatch in `render/divs.rs` and the deck classes in `assets/css/deck.css`; a
/// `vocab.rs` test pins `vocab::div_classes()`'s names as a subset.
pub(crate) const DIV_FEATURE_CLASSES: &[&str] = &[
    "panel-tabset",
    "code-walkthrough",
    "scrolly",
    "debug",
    "magic-move",
    "step",
    "column-margin",
    "column-page",
    "column-screen",
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
];

/// Input control types `.input type=` recognizes.
///
/// Every one is a plain form field, and that is the whole set on purpose. Two structural
/// controls (`animate`, a play/pause/step/reset tick, and `point`, a draggable 2-D
/// coordinate) were retired on 2026-08-03: neither had a use outside its own fixture, and
/// each carried a special case through the emitter, the a11y markup and the URL-state
/// serializer. A document that wants a frame pump drives one from a `{js}` cell.
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
/// an exactly-known class (a legit generic like `.fragment`) and a genuine custom class
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
        // A class we REMOVED is still unknown, so it still warns — but a removal and a
        // misspelling are different mistakes and the author has to be told which one they
        // made. Checked ahead of the did-you-mean, and unconditionally: once `.columns`
        // and `.column` were both withdrawn nothing survived within edit distance 2 of
        // either, so the search below would answer a retired class with SILENCE.
        if let Some((_, note)) = RETIRED_DIV_CLASSES.iter().find(|(retired, _)| retired == c) {
            return Some(
                Warning::new(format!("unknown div class `{c}`: {note}"))
                    .at(file.clone(), line as u32),
            );
        }
        closest(c, &known).map(|s| {
            Warning::new(format!("unknown div class `{c}` (did you mean `{s}`?)"))
                .at(file.clone(), line as u32)
        })
    })
}

/// Div classes taliesin USED to dispatch on, as `(class, what to do instead)`.
///
/// The div sibling of `frontmatter::RETIRED_KEYS`, and it exists for the same reason plus a
/// sharper one: div classes are an OPEN vocabulary, so a withdrawn class does not merely get
/// the wrong hint, it gets no diagnostic at all — it reads as a custom class the author
/// styles themselves, and the page silently loses its layout.
///
/// **No entry is phrased as a did-you-mean, even where a successor exists**, exactly as
/// `RETIRED_KEYS` documents: `codes::extract_suggestion` lifts that phrase into a structured
/// fix an agent applies mechanically, and none of these are mechanical renames. Rewriting
/// `::: {.columns}` + `.column` children as `{layout-ncol=N}` deletes the child fences and
/// moves the declaration from the class to an attribute, so a blind rename would trade a
/// warning for a document that is broken in a new way.
pub(crate) const RETIRED_DIV_CLASSES: &[(&str, &str)] = &[
    (
        "columns",
        "it was removed on 2026-08-02. `{layout-ncol=N}` was always the same grid and is \
         now the only spelling, so the wrapper becomes `::: {layout-ncol=2}` and its \
         `.column` children become plain blocks separated by a blank line",
    ),
    (
        "column",
        "it was removed on 2026-08-02 with `.columns`. Under `{layout-ncol=N}` each direct \
         child block is already a column, so the child fences go away entirely rather than \
         being renamed",
    ),
    (
        "aside",
        "it was removed on 2026-08-03. `.column-margin` is the only margin spelling now",
    ),
    (
        "sidenote",
        "it was removed on 2026-08-03. `.column-margin` is the only margin spelling now",
    ),
    (
        "marginnote",
        "it was removed on 2026-08-03. `.column-margin` is the only margin spelling now",
    ),
    (
        "example",
        "it was removed on 2026-08-03. Rewrite it as `.definition` or a plain \
         `::: {.callout-note}`, whichever fits the content",
    ),
    (
        "proposition",
        "it was removed on 2026-08-03. `.theorem` is the closest surviving numbered kind \
         now — both render in the same `plain` style",
    ),
    (
        "remark",
        "it was removed on 2026-08-03. A plain `::: {.callout-note}` is the closest \
         surviving spelling now",
    ),
];

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

/// Validate a `::: {.debug}` container. Purely diagnostic: the div still renders, and a
/// reader gets a plain code panel rather than a blank box.
///
/// `named` is whether the div carried `name=`. An unnamed `.debug` is legal (the stepper
/// works; it just cannot be addressed from a `{js}` view cell), so it is NOT a warning
/// here. The unaddressable-view case is caught where the `//| input:` edge is resolved.
pub(crate) fn validate_debug(
    traced_cells: usize,
    has_code: bool,
    _named: bool,
    line: usize,
    file: Option<String>,
) -> Vec<Warning> {
    let mut out = Vec::new();
    if !has_code {
        out.push(
            Warning::new("`.debug` has no code block to step through".to_string())
                .at(file.clone(), line as u32),
        );
    } else if traced_cells == 0 {
        out.push(
            Warning::new(
                "`.debug` has no traced cell: mark one with `#| trace: true` \
                 (or `//| trace: true` for a `{js}` generator)"
                    .to_string(),
            )
            .at(file.clone(), line as u32),
        );
    } else if traced_cells > 1 {
        out.push(
            Warning::new(
                "`.debug` has more than one traced cell; only the first is traced".to_string(),
            )
            .at(file, line as u32),
        );
    }
    out
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
            validate_div_class(&s("column-margin"), 3, None).is_none(),
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

    /// Margin-content spellings went 4 -> 1 on 2026-08-03 (visual minimalism pass, task
    /// 13): `.aside` and `.marginnote` had zero uses in the tree, `.sidenote` had one
    /// (migrated to `.column-margin`). Each retired spelling must warn with the removal
    /// note — not silence (div classes are an open vocabulary, so silence is what a
    /// PLAIN removal from `DIV_FEATURE_CLASSES` alone would produce) and not a
    /// did-you-mean (a removal and a misspelling are different mistakes). This is the
    /// direct pin that `RETIRED_DIV_CLASSES` actually carries all three entries;
    /// `crates/core/tests/retired_names.rs` pins the same three through the full render
    /// pipeline, since `RETIRED_DIV_CLASSES` itself is `pub(crate)` and unreachable from
    /// an external test.
    #[test]
    fn margin_aliases_warn_with_the_removal_note_not_a_did_you_mean() {
        let s = |c: &str| vec![c.to_string()];
        for gone in ["aside", "sidenote", "marginnote"] {
            let w = validate_div_class(&s(gone), 3, None)
                .unwrap_or_else(|| panic!("`.{gone}` must warn, div classes are open vocabulary"));
            assert_eq!(
                w.message,
                format!(
                    "unknown div class `{gone}`: it was removed on 2026-08-03. \
                     `.column-margin` is the only margin spelling now"
                )
            );
            assert!(
                !w.message.contains("did you mean"),
                "a retired class is not a did-you-mean: {}",
                w.message
            );
        }
    }

    /// Theorem kinds went 8 -> 5 on 2026-08-03 (visual minimalism pass, task 14): `example`,
    /// `proposition` and `remark` were never cross-referenced by any document in the tree.
    /// Unlike a callout kind, a theorem kind carries no namespace prefix — `THEOREM_KINDS`
    /// IS the dispatch vocabulary — so without a `RETIRED_DIV_CLASSES` entry a leftover
    /// `.example`/`.proposition`/`.remark` falls through to a plain, unnumbered,
    /// unreferenceable div with no diagnostic at all. This is the direct pin that
    /// `RETIRED_DIV_CLASSES` actually carries all three entries;
    /// `crates/core/tests/retired_names.rs` pins the same three through the full render
    /// pipeline, since `RETIRED_DIV_CLASSES` itself is `pub(crate)` and unreachable from
    /// an external test.
    #[test]
    fn theorem_kind_aliases_warn_with_the_removal_note_not_a_did_you_mean() {
        let s = |c: &str| vec![c.to_string()];
        for gone in ["example", "proposition", "remark"] {
            let w = validate_div_class(&s(gone), 3, None)
                .unwrap_or_else(|| panic!("`.{gone}` must warn, div classes are open vocabulary"));
            assert!(
                w.message.starts_with(&format!(
                    "unknown div class `{gone}`: it was removed on 2026-08-03."
                )),
                "expected a removal note for `.{gone}`, got: {}",
                w.message
            );
            assert!(
                !w.message.contains("did you mean"),
                "a retired class is not a did-you-mean: {}",
                w.message
            );
        }
        // Removing a kind also removes it from the dispatch vocabulary itself: the three
        // cut kinds must not survive in THEOREM_KINDS (else `theorem_kind()` would still
        // route them into the theorem arm, and this whole retirement would be a no-op).
        for gone in ["example", "proposition", "remark"] {
            assert!(
                !THEOREM_KINDS.contains(&gone),
                "`.{gone}` must no longer be a recognized theorem kind"
            );
        }
        assert_eq!(
            THEOREM_KINDS,
            &["theorem", "lemma", "corollary", "definition", "proof"],
            "theorem vocabulary should be exactly 5"
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
        let w = validate_callout_kind("warnign", 7, None).expect("an unknown-kind warning");
        assert_eq!(
            w.message,
            "unknown callout kind `warnign` (did you mean `warning`?)"
        );
        assert_eq!(w.line, Some(7));
        assert!(
            validate_callout_kind("note", 7, None).is_none(),
            "note is recognized"
        );
    }

    /// `important` and `caution` were retired 2026-08-03 (visual minimalism pass, task
    /// 12): three kinds cover the distinctions a reader can actually decode. A document
    /// that still names one of the two cut kinds must be told it was REMOVED, not offered
    /// a did-you-mean (that phrasing is reserved for mechanical renames, and `warning`/
    /// `note` are judgement calls, not renames) — same rule `RETIRED_KEYS`'s own doc
    /// comment states for front-matter keys.
    #[test]
    fn retired_callout_kind_gives_the_removal_note_not_a_did_you_mean() {
        for kind in ["important", "caution"] {
            let w = validate_callout_kind(kind, 7, None)
                .unwrap_or_else(|| panic!("`{kind}` must still warn"));
            assert!(
                w.message
                    .starts_with(&format!("unknown callout kind `{kind}`: it was removed")),
                "expected a removal note for `{kind}`, got: {}",
                w.message
            );
            assert!(
                !w.message.contains("did you mean"),
                "a retired kind must not be phrased as a did-you-mean: {}",
                w.message
            );
        }
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

    #[test]
    fn debug_diagnostics_cover_every_authoring_mistake() {
        // No traced cell: the block would render a dead code panel.
        let w = validate_debug(0, true, true, 7, Some("a.tmd".into()));
        assert_eq!(w.len(), 1, "expected exactly one warning, got {w:?}");
        assert!(
            w[0].message.contains("trace: true"),
            "must name the fix: {}",
            w[0].message
        );
        assert_eq!(w[0].line, Some(7), "must be click-to-source locatable");

        // Two traced cells: only the first is traced, so say so.
        let w = validate_debug(2, true, true, 3, None);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].message.contains("only the first"), "{}", w[0].message);

        // No code block at all.
        let w = validate_debug(0, false, true, 3, None);
        assert!(
            w.iter().any(|x| x.message.contains("no code block")),
            "a bodyless .debug must warn: {w:?}"
        );

        // The healthy case is silent.
        assert!(validate_debug(1, true, true, 3, None).is_empty());
        assert!(
            validate_debug(1, true, false, 3, None).is_empty(),
            "an unnamed .debug is legal (it just cannot be addressed from a view cell)"
        );
    }
}
