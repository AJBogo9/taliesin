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

use super::{CellRole, Warning};
use crate::frontmatter::{closest, unknown_key_message};
use std::collections::HashMap;

/// Cell options taliesin recognizes on a code cell's leading `#|` / `//|` / `%%|`
/// lines (the union across all cell languages; each is read in `cell_option` /
/// `parse_js_opts` / `code_fold`).
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
    "name",   // {js}
    "viewof", // {js}
    "input",  // {js}
];

/// Options only a client-language cell reads (`parse_js_opts` answers empty for every
/// other language), so each is inert on a kernel or display cell.
const CLIENT_ONLY: &[&str] = &["name", "viewof", "input"];

/// Options a client-language cell never reads, so each is inert on one. A running client
/// cell hands its source to the browser before the `echo` check, and the Figure and Table
/// arms read only `include` for a non-kernel language. `include` is NOT here: it hides a
/// `lst-`/`tbl-` cell of any language. (`echo` on a listing is inert in every language, and
/// a client table cell never runs, so each has its own reason in `inert_option_message`.)
/// `cache` has a rule of its own: it keys `_freeze/`, which only a cell a kernel runs reaches.
const NOT_ON_CLIENT: &[&str] = &["echo"];

/// The label prefixes the cell-role test in `render_internal` reads. Any other `label:`
/// feeds no anchor, number, element id or freeze key. `pub` for completion, which offers
/// only these after `#| label:`.
pub const NUMBERED_LABEL_PREFIXES: &[&str] = &["fig-", "lst-", "tbl-"];

/// Whether known option `key` can act on a `{lang}` cell: false for `cache` on a cell no
/// kernel runs, a `CLIENT_ONLY` key off a client language and a `NOT_ON_CLIENT` key on one.
/// The language half of `inert_option_message`, and what completion offers a cell by, so
/// the offer and the warning cannot disagree. The role half (`echo` on a listing) stays in
/// the lint: the role is not known while the key is being typed.
pub fn option_acts_on_language(key: &str, lang: &str) -> bool {
    if key == "cache" {
        return super::executes_to_kernel(lang);
    }
    let inert = if super::is_client_lang(lang) {
        NOT_ON_CLIENT
    } else {
        CLIENT_ONLY
    };
    !inert.contains(&key)
}

/// Callout kinds taliesin recognizes (`::: {.callout-<kind>}`).
///
/// Five kinds shrank to three on 2026-08-03 (visual minimalism pass, task 12):
/// `important` and `caution` were a coloured box with a different word in it, the same
/// as `warning`, and no reader could reliably tell the three apart. A document that still
/// writes one gets the same unknown-kind diagnostic as any other name this tool does not
/// define.
pub(crate) const CALLOUT_KINDS: &[&str] = &["note", "tip", "warning"];

/// Structural feature classes a `:::` fenced div can carry. This is **not** a closed
/// vocabulary — a div may carry any custom class (styled by the author's own CSS) — so this list
/// only anchors the *did-you-mean* (`validate_div_class`); it never rejects. Keep in sync with the
/// `.class` dispatch in `render/divs.rs`; a `vocab.rs` test pins `vocab::div_classes()`'s names
/// as a subset.
///
/// Two width escapes, and that is the whole set. The narrative widgets
/// (`panel-tabset`/`code-walkthrough`/`scrolly`/`step`) and the five theorem kinds were
/// withdrawn on 2026-08-08; a leftover fence carrying one is a custom class like any
/// other, styled by whatever CSS the author brings.
pub(crate) const DIV_FEATURE_CLASSES: &[&str] = &["column-margin", "column-page"];

/// Input control types `.input type=` recognizes.
///
/// Every one is a plain form field, and that is the whole set on purpose. Two structural
/// controls (`animate`, a play/pause/step/reset tick, and `point`, a draggable 2-D
/// coordinate) were retired on 2026-08-03: neither had a use outside its own fixture, and
/// each carried a special case through the emitter, the a11y markup and the URL-state
/// serializer. A document that wants a frame pump drives one from a `{js}` cell.
/// `range` was retired on 2026-08-08: it was a second spelling of `slider` and nothing but
/// its own fixture used it, so it bought a synonym in five registration sites.
pub(crate) const INPUT_TYPES: &[&str] = &["slider", "number", "checkbox", "text", "select"];

/// Enumerate a cell's leading options, each as its key, its unquoted value (as
/// `cell_option` reads it) and its 0-based line offset within `literal` (the fence body).
/// Mirrors `cell_option`'s scan: only the contiguous leading `#|` / `//|` / `%%|` block,
/// stopping at the first code line.
pub fn cell_option_keys(literal: &str) -> Vec<(String, &str, usize)> {
    let mut keys = Vec::new();
    for (i, line) in literal.lines().enumerate() {
        let Some(opt) = super::option_directive(line) else {
            break;
        };
        if let Some((k, v)) = opt.split_once(':') {
            keys.push((k.trim().to_string(), v.trim().trim_matches(['"', '\'']), i));
        }
    }
    keys
}

/// Validate a code cell's `#|` options against [`CELL_OPTION_KEYS`], and report a known
/// option that does nothing on this cell: one its `lang` or its `role` never reads, a
/// `label` no cross-reference can reach, or a key set again after its first line.
/// `role` is the role `render_internal` resolved for the cell. `fence_line` is the 1-based
/// source line of the cell's opening fence (in `file`'s coordinates); an option on the
/// cell's i-th body line is at `fence_line + 1 + i`.
///
/// `cell_option` reads the FIRST line that sets a key, so only that line is judged, and
/// each later line setting a known key is reported with the line that is read. An unknown
/// key is reported as unknown on every line, since none of them is read.
pub(crate) fn validate_cell_options(
    literal: &str,
    lang: &str,
    role: Option<&CellRole>,
    fence_line: usize,
    file: Option<String>,
) -> Vec<Warning> {
    let mut read_at: HashMap<String, u32> = HashMap::new();
    cell_option_keys(literal)
        .into_iter()
        .filter_map(|(k, value, offset)| {
            let line = (fence_line + 1 + offset) as u32;
            let message = if !CELL_OPTION_KEYS.contains(&k.as_str()) {
                // A suggestion the author accepts must not draw the inert warning next.
                let acting: Vec<&str> = CELL_OPTION_KEYS
                    .iter()
                    .copied()
                    .filter(|key| option_acts_on_language(key, lang))
                    .collect();
                unknown_key_message("cell option", &k, &acting)
            } else if let Some(first) = read_at.get(&k) {
                format!("repeated `{k}:`: only the first, on line {first}, is read")
            } else {
                read_at.insert(k.clone(), line);
                inert_option_message(&k, value, lang, role)?
            };
            Some(Warning::new(message).at(file.clone(), line))
        })
        .collect()
}

/// Why known option `key`, read as `value`, does nothing on a `{lang}` cell in `role`, or
/// `None` when it acts.
fn inert_option_message(
    key: &str,
    value: &str,
    lang: &str,
    role: Option<&CellRole>,
) -> Option<String> {
    let client = super::is_client_lang(lang);
    if key == "label" {
        return (!NUMBERED_LABEL_PREFIXES.iter().any(|p| value.starts_with(p))).then(|| {
            format!(
                "`label: {value}` makes no anchor: only fig-, lst- and tbl- labels are numbered"
            )
        });
    }
    if key == "echo" {
        match role {
            // The Listing arm reads only `include`, whatever the language.
            Some(CellRole::Listing { .. }) => {
                return Some(
                    "`echo` has no effect on a listing: a listing always shows its source \
                     (`include: false` hides it)"
                        .to_string(),
                );
            }
            // The Table arm runs, and reads `echo` for, only a kernel language. A client
            // table cell keeps its source instead, which only `include: false` hides.
            Some(CellRole::Table { .. }) if client => {
                return Some(format!(
                    "`echo` has no effect on a `{{{lang}}}` table cell: it never runs, so it \
                     always shows its source (`include: false` hides it)"
                ));
            }
            _ => {}
        }
    }
    if !option_acts_on_language(key, lang) {
        let why = if key == "cache" {
            "only a kernel cell's output is kept in `_freeze/`".to_string()
        } else if !client {
            "it is a `{js}` cell option".to_string()
        } else {
            format!("a running `{{{lang}}}` cell never shows its source")
        };
        return Some(format!(
            "`{key}` has no effect on a `{{{lang}}}` cell: {why}"
        ));
    }
    None
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

/// A misspelled feature `:::` class → a located "did you mean". Fired from the generic-div
/// fall-through in `build_container` (the classes that matched no feature arm). Only a *near-miss*
/// of a known feature class ([`DIV_FEATURE_CLASSES`], edit distance ≤ 2) warns:
/// an exactly-known class (a legit generic like `.column-page`) and a genuine custom class
/// (far from every known name) both stay silent, since div classes are an *open* vocabulary. At
/// most one warning per div (the first offending class), and purely diagnostic — the div still
/// renders with its given class. `line` is the 1-based source line of the opening fence.
pub(crate) fn validate_div_class(
    classes: &[String],
    line: usize,
    file: Option<String>,
) -> Option<Warning> {
    let known = DIV_FEATURE_CLASSES;
    classes.iter().find_map(|c| {
        if known.contains(&c.as_str()) {
            return None;
        }
        closest(c, known).map(|s| {
            Warning::new(format!("unknown div class `{c}` (did you mean `{s}`?)"))
                .at(file.clone(), line as u32)
        })
    })
}

/// Validate a fenced div that turned out EMPTY (no blocks between its `:::` fences). An empty
/// GENERIC div is harmless (it's dropped), but an empty div that names a real feature — a
/// `.input` reactive control, a `.callout-*`, a `.column-page` escape — is almost
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
        c == "input" || c.starts_with("callout-") || DIV_FEATURE_CLASSES.contains(&c)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_only_the_leading_option_block() {
        let lit = "#| echo: false\n#| labl: x\nprint(1)\n#| late: y\n";
        let keys: Vec<_> = cell_option_keys(lit).into_iter().collect();
        assert_eq!(
            keys,
            vec![
                ("echo".to_string(), "false", 0),
                ("labl".to_string(), "x", 1)
            ]
        );
    }

    fn listing() -> CellRole {
        CellRole::Listing {
            anchor: None,
            caption: None,
            fold: None,
        }
    }

    fn table() -> CellRole {
        CellRole::Table {
            anchor: None,
            caption: None,
        }
    }

    #[test]
    fn flags_unknown_cell_option_with_did_you_mean_and_location() {
        // Fence is on file line 20, so the option on body line 1 is file line 22.
        let w = validate_cell_options(
            "#| echo: false\n#| labl: x\n",
            "python",
            None,
            20,
            Some("p.tmd".into()),
        );
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
        let py = "#| echo: false\n#| include: false\n#| cache: false\n#| label: fig-x\n\
                  #| fig-cap: A\n#| code-fold: true\n#| code-summary: S\n";
        assert!(
            validate_cell_options(py, "python", None, 1, None).is_empty(),
            "every key acts on a {{python}} cell"
        );
        let reactive = "//| name: n\n//| viewof: v\n//| input: a, b\n";
        assert!(
            validate_cell_options(reactive, "js", None, 1, None).is_empty(),
            "every reactive key acts on a running {{js}} cell"
        );
        let lst = "//| label: lst-x\n//| lst-cap: A\n//| include: false\n//| code-fold: true\n";
        assert!(
            validate_cell_options(lst, "js", Some(&listing()), 1, None).is_empty(),
            "every key acts on a {{js}} listing"
        );
    }

    /// `echo` and `cache` are never read for a client-language cell: the Figure and Table
    /// arms read only `include` for a non-kernel language, the plain arm hands the source to
    /// the browser before the echo check, and `cache` keys `_freeze/`, which only a kernel
    /// cell reaches.
    #[test]
    fn echo_and_cache_on_a_client_cell_warn_located() {
        let w = validate_cell_options(
            "//| echo: false\n//| cache: false\nlet x = 1;\n",
            "js",
            None,
            10,
            Some("p.tmd".into()),
        );
        let got: Vec<_> = w.iter().map(|w| (w.message.as_str(), w.line)).collect();
        assert_eq!(
            got,
            vec![
                (
                    "`echo` has no effect on a `{js}` cell: a running `{js}` cell never shows \
                     its source",
                    Some(11)
                ),
                (
                    "`cache` has no effect on a `{js}` cell: only a kernel cell's output is \
                     kept in `_freeze/`",
                    Some(12)
                ),
            ]
        );
        assert!(w.iter().all(|w| w.file.as_deref() == Some("p.tmd")
            && w.severity == crate::render::Severity::Warning));
    }

    /// The Listing arm reads only `include`, for every language, so `echo` on a listing is
    /// inert whether the cell is `{js}`, `{python}` or a display fence. The reason names the
    /// listing, not the language, because the language is not why.
    #[test]
    fn echo_on_a_listing_warns_with_the_listing_reason_in_any_language() {
        for lang in ["js", "python", "bash"] {
            let w = validate_cell_options(
                "#| label: lst-a\n#| echo: false\n",
                lang,
                Some(&listing()),
                1,
                None,
            );
            let got: Vec<_> = w.iter().map(|w| (w.message.as_str(), w.line)).collect();
            assert_eq!(
                got,
                [(
                    "`echo` has no effect on a listing: a listing always shows its source \
                     (`include: false` hides it)",
                    Some(3)
                )],
                "for {{{lang}}}"
            );
        }
    }

    /// The Table arm reads `echo` only for a kernel language. A `{js}` table cell never
    /// runs there: it keeps its source, which only `include: false` hides, so the reason is
    /// the table's, not the running cell's.
    #[test]
    fn echo_on_a_js_table_cell_warns_with_the_table_reason() {
        let w = validate_cell_options(
            "//| label: tbl-a\n//| echo: false\n",
            "js",
            Some(&table()),
            1,
            None,
        );
        let got: Vec<_> = w.iter().map(|w| (w.message.as_str(), w.line)).collect();
        assert_eq!(
            got,
            [(
                "`echo` has no effect on a `{js}` table cell: it never runs, so it always shows \
                 its source (`include: false` hides it)",
                Some(3)
            )]
        );
    }

    /// A `{python}` table cell runs on the kernel and hides its source on `echo: false` (the
    /// Table arm's kernel branch), so `echo` acts there and draws nothing.
    #[test]
    fn echo_on_a_python_table_cell_is_silent() {
        let lit = "#| label: tbl-a\n#| echo: false\n";
        assert!(validate_cell_options(lit, "python", Some(&table()), 1, None).is_empty());
    }

    /// The reactive options are read only by `parse_js_opts`, which answers empty for every
    /// language but `{js}`, kernel and display cells alike.
    #[test]
    fn reactive_options_on_a_non_client_cell_warn() {
        for lang in ["python", "mermaid", "bash"] {
            let w = validate_cell_options(
                "#| name: n\n#| viewof: v\n#| input: a\n",
                lang,
                None,
                1,
                None,
            );
            let got: Vec<_> = w.iter().map(|w| (w.message.clone(), w.line)).collect();
            assert_eq!(
                got,
                ["name", "viewof", "input"]
                    .iter()
                    .zip(2..)
                    .map(|(k, line)| (
                        format!(
                            "`{k}` has no effect on a `{{{lang}}}` cell: it is a `{{js}}` cell \
                             option"
                        ),
                        Some(line)
                    ))
                    .collect::<Vec<_>>(),
                "for {{{lang}}}"
            );
        }
    }

    /// A deliberate false negative. `include` on a plain `{js}` cell is inert (the plain
    /// client arm never reads it), but it acts on a `{js}` listing or table cell, and the
    /// validator stays silent on the plain case rather than re-derive each arm's reading.
    /// `include_false_hides_a_js_listing_and_is_not_reported` pins the case where it acts.
    #[test]
    fn include_on_a_plain_client_cell_is_a_deliberate_false_negative() {
        assert!(validate_cell_options("//| include: false\n", "js", None, 1, None).is_empty());
    }

    /// The language half of the rule is the one question completion asks too (a cell is
    /// offered only the keys that act on its language), so the offer and the warning agree.
    #[test]
    fn the_language_rule_is_the_one_completion_asks() {
        let inert = |lang| {
            CELL_OPTION_KEYS
                .iter()
                .copied()
                .filter(|k| !option_acts_on_language(k, lang))
                .collect::<Vec<_>>()
        };
        assert_eq!(inert("js"), ["echo", "cache"]);
        assert_eq!(inert("python"), ["name", "viewof", "input"]);
        // Nothing runs a display language, so nothing keeps its output in `_freeze/`.
        assert_eq!(inert("mermaid"), ["cache", "name", "viewof", "input"]);
        for lang in ["js", "python", "mermaid"] {
            for key in CELL_OPTION_KEYS {
                let value = if *key == "label" { "fig-a" } else { "x" };
                assert_eq!(
                    option_acts_on_language(key, lang),
                    inert_option_message(key, value, lang, None).is_none(),
                    "`{key}` on {{{lang}}}"
                );
            }
        }
    }

    /// Only a `fig-`/`lst-`/`tbl-` label is read (the cell-role test in `render_internal`);
    /// any other label feeds no anchor, number, element id or freeze key.
    #[test]
    fn a_label_without_a_numbered_prefix_warns() {
        for (lit, shown) in [
            ("#| label: setup\n", "setup"),
            ("#| label: \"setup\"\n", "setup"),
            ("#| echo: false\n#| label: sec-intro\n", "sec-intro"),
        ] {
            let w = validate_cell_options(lit, "python", None, 4, None);
            assert_eq!(w.len(), 1, "for {lit:?}: {w:?}");
            assert_eq!(
                w[0].message,
                format!(
                    "`label: {shown}` makes no anchor: only fig-, lst- and tbl- labels are numbered"
                )
            );
            assert_eq!(w[0].line, Some(4 + lit.lines().count() as u32));
        }
        for label in ["fig-a", "lst-a", "tbl-a"] {
            let lit = format!("#| label: {label}\n");
            assert!(
                validate_cell_options(&lit, "python", None, 1, None).is_empty(),
                "{label} is numbered"
            );
        }
    }

    /// `cell_option` reads the FIRST line that sets a key, so only that line's value is
    /// judged, and every later line setting the same key is reported, located, with the line
    /// that is read. A bare first label warns and the `fig-` one after it is the repeat; a
    /// `fig-` label first is read, so the bare one after it is only a repeat.
    #[test]
    fn a_repeated_key_is_judged_on_its_first_line_and_reported_on_each_later_one() {
        let bare = "`label: setup` makes no anchor: only fig-, lst- and tbl- labels are numbered";
        let repeated = "repeated `label:`: only the first, on line 11, is read";
        for (lit, want) in [
            (
                "#| label: setup\n#| label: fig-z\n",
                vec![(bare, Some(11)), (repeated, Some(12))],
            ),
            (
                "#| label: fig-z\n#| label: setup\n",
                vec![(repeated, Some(12))],
            ),
        ] {
            let w = validate_cell_options(lit, "python", None, 10, Some("p.tmd".into()));
            let got: Vec<_> = w.iter().map(|w| (w.message.as_str(), w.line)).collect();
            assert_eq!(got, want, "for {lit:?}");
            assert!(w.iter().all(|w| w.file.as_deref() == Some("p.tmd")));
        }

        // Each later line is reported, and each names the first line, not the one before it.
        let w = validate_cell_options(
            "#| echo: false\n#| cache: false\n#| echo: true\n#| echo: false\n",
            "python",
            None,
            1,
            None,
        );
        let got: Vec<_> = w.iter().map(|w| (w.message.as_str(), w.line)).collect();
        let echo = "repeated `echo:`: only the first, on line 2, is read";
        assert_eq!(got, [(echo, Some(4)), (echo, Some(5))]);
    }

    /// No line of an unknown key is read, so "only the first is read" would be false for one:
    /// each line draws the unknown-key warning instead.
    #[test]
    fn a_repeated_unknown_key_is_unknown_on_every_line() {
        let w = validate_cell_options("#| labl: a\n#| labl: b\n", "python", None, 1, None);
        let got: Vec<_> = w.iter().map(|w| (w.message.as_str(), w.line)).collect();
        let unknown = "unknown cell option `labl` (did you mean `label`?)";
        assert_eq!(got, [(unknown, Some(2)), (unknown, Some(3))]);
    }

    #[test]
    fn validate_div_class_suggests_near_miss_only() {
        let s = |c: &str| vec![c.to_string()];
        // A near-miss of a feature class → located "did you mean".
        let w = validate_div_class(&s("column-margn"), 3, None).expect("a near-miss warning");
        assert_eq!(
            w.message,
            "unknown div class `column-margn` (did you mean `column-margin`?)"
        );
        assert_eq!(w.line, Some(3));
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
            validate_div_class(
                &["column-margn".to_string(), "column-pag".to_string()],
                3,
                None
            )
            .into_iter()
            .count(),
            1
        );
    }

    /// Is `name` used as a class SELECTOR anywhere in `css`?
    ///
    /// A plain `css.contains(".column")` is the obvious spelling and it is wrong: it also
    /// matches `.column-margin`, the surviving class, so a register entry named `column`
    /// would report its own successor as a leftover. Measured on 2026-08-08 — that is one
    /// false positive out of the eight entries, i.e. the naive derivation fails today, not
    /// hypothetically. Requiring a non-identifier character after the name is what makes
    /// `.column` and `.column-margin` different selectors.
    fn css_has_class_selector(css: &str, name: &str) -> bool {
        let needle = format!(".{name}");
        css.match_indices(&needle).any(|(at, _)| {
            !css[at + needle.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
    }

    /// The live `:::` vocabulary is exactly the width escapes and the callout kinds.
    ///
    /// What is left of the retired-name tombstone this used to be: the registers it derived
    /// from went on 2026-08-17 (the author's FD2 ruling — Taliesin answers for its own
    /// vocabulary, not for the tools a name came from), so a withdrawn class is now an
    /// unknown class like any other. The half that still earns its place is the live list:
    /// a class silently re-added here starts dispatching a feature again, and the two
    /// `assert_eq!`s below are what make that a test failure rather than a surprise.
    ///
    /// `css_has_class_selector` keeps its vacuity control: a helper that always answered
    /// `false` would make the surviving CSS check below meaningless.
    #[test]
    fn the_live_div_vocabulary_is_the_width_escapes_and_the_callout_kinds() {
        let base = crate::render::base_css();

        // Vacuity control: the helper must find a selector that IS there, and must not
        // confuse a prefix for the whole name (the case the naive `contains` gets wrong).
        assert!(
            css_has_class_selector(base, "column-margin"),
            "the CSS selector check finds nothing, so every assertion below is vacuous"
        );
        assert!(
            !css_has_class_selector(base, "column"),
            "`.column` must not match `.column-margin` — the derivation is too loose"
        );

        assert_eq!(
            DIV_FEATURE_CLASSES,
            &["column-margin", "column-page"],
            "the live div-class vocabulary should be exactly the two width escapes"
        );
        assert_eq!(
            CALLOUT_KINDS,
            &["note", "tip", "warning"],
            "callout vocabulary should be exactly 3"
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
        // Other feature classes warn generically (callout/width escape), no shortcode hint.
        for c in ["callout-note", "column-page", "column-margin"] {
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
}
