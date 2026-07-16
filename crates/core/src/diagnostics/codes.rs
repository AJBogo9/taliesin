//! Stable diagnostic codes + severities for agent-matchable `check --format json` output.
//!
//! An agent (or CI rule) wants to match a diagnostic on a stable identifier and read a
//! machine-usable fix, not scrape prose. This catalog maps each diagnostic family to a
//! stable `TAL-<FAMILY>` code and a severity, and lifts the inline "did you mean `X`?"
//! hint into a structured replacement. It is derived from the diagnostic *message* at the
//! `check` boundary (not threaded through every validator), so `--format human` stays
//! byte-identical and no validator call site changes. The trade-off is that the mapping is
//! keyed on stable message prefixes; `check_cli.rs` pins representative codes so a wording
//! change that would silently reclassify a family fails a test.

/// Severity strings (an agent triages on these; `check` still exits non-zero on ANY
/// diagnostic regardless of severity — the severity only ranks them).
pub const ERROR: &str = "error";
pub const WARNING: &str = "warning";

/// The fallback code for a diagnostic whose family isn't catalogued yet. Non-empty and
/// stable, so `.diagnostics[].code` is always a usable string.
pub const GENERIC: &str = "TAL-CHECK";

/// `(message-substring, code, severity)`, matched in order (first hit wins), so a more
/// specific needle must precede a more general one (`broken link anchor` before
/// `broken link`).
const TABLE: &[(&str, &str, &str)] = &[
    // Front matter.
    ("not valid YAML", "TAL-FM-YAML", ERROR),
    ("valid YAML", "TAL-FM-YAML", ERROR),
    ("unknown front-matter key", "TAL-FM-KEY", WARNING),
    ("unknown execute key", "TAL-FM-KEY", WARNING),
    ("unknown listing key", "TAL-FM-KEY", WARNING),
    ("unknown about key", "TAL-FM-KEY", WARNING),
    ("unknown hero key", "TAL-FM-KEY", WARNING),
    ("unknown theorems key", "TAL-FM-KEY", WARNING),
    ("unknown prose-lint key", "TAL-FM-KEY", WARNING),
    ("unknown format", "TAL-FM-FORMAT", WARNING),
    // A recognized key taliesin reads and then ignores (`csl:`). Must precede the
    // citation needles below: the message names `bibliography`-adjacent concepts and
    // would otherwise classify as TAL-CITE-BIB.
    (
        "is recognized but not supported",
        "TAL-FM-UNSUPPORTED",
        WARNING,
    ),
    // Body constructs.
    ("unknown callout kind", "TAL-CALLOUT-KIND", WARNING),
    ("unknown cell option", "TAL-CELL-OPTION", WARNING),
    ("unknown input type", "TAL-INPUT-TYPE", WARNING),
    // Cross-references + links + anchors.
    ("broken cross-reference", "TAL-XREF-UNDEF", ERROR),
    ("duplicate heading id", "TAL-DUP-ID", ERROR),
    ("broken in-page link", "TAL-ANCHOR", ERROR),
    ("broken link anchor", "TAL-LINK-ANCHOR", ERROR),
    ("broken link", "TAL-LINK", ERROR),
    // Assets + media.
    ("local asset not found", "TAL-ASSET", ERROR),
    ("local video not found", "TAL-MEDIA", ERROR),
    ("local audio not found", "TAL-MEDIA", ERROR),
    // Reactive `{js}` graph.
    ("unknown reactive input", "TAL-REACTIVE", ERROR),
    ("reactive dependency cycle", "TAL-REACTIVE", ERROR),
    // Accessibility.
    ("heading level skips", "TAL-A11Y-HEADING", WARNING),
    ("has no accessible name", "TAL-A11Y-NAME", WARNING),
    ("missing alt text", "TAL-A11Y-ALT", WARNING),
    ("looks like a placeholder", "TAL-A11Y-ALT", WARNING),
    // Citations, math, code, categories.
    ("citations are present", "TAL-CITE-BIB", WARNING),
    ("bibliography", "TAL-CITE-BIB", WARNING),
    ("math", "TAL-MATH", WARNING),
    ("unknown code language", "TAL-CODE-LANG", WARNING),
    ("category ", "TAL-CATEGORY", WARNING),
];

/// The `(code, severity)` for a diagnostic message: the first catalogued family whose
/// substring the message contains, or `(GENERIC, ERROR)` when none match.
pub fn classify(message: &str) -> (&'static str, &'static str) {
    for (needle, code, severity) in TABLE {
        if message.contains(needle) {
            return (code, severity);
        }
    }
    (GENERIC, ERROR)
}

/// The replacement from an inline "did you mean `X`?" hint, e.g. `treme` -> `theme`,
/// `@fig-reslts` -> `@fig-results`. `None` when the message carries no such hint.
pub fn extract_suggestion(message: &str) -> Option<String> {
    let key = "did you mean `";
    let at = message.find(key)? + key.len();
    let rest = &message[at..];
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_families_with_stable_codes() {
        assert_eq!(
            classify("unknown front-matter key `treme` (did you mean `theme`?)"),
            ("TAL-FM-KEY", WARNING)
        );
        assert_eq!(
            classify("broken cross-reference: @fig-x (did you mean `@fig-y`?)"),
            ("TAL-XREF-UNDEF", ERROR)
        );
        assert_eq!(
            classify("image is missing alt text"),
            ("TAL-A11Y-ALT", WARNING)
        );
        assert_eq!(
            classify("local asset not found: `x.png`"),
            ("TAL-ASSET", ERROR)
        );
    }

    #[test]
    fn more_specific_needle_wins() {
        assert_eq!(classify("broken link anchor `#x`").0, "TAL-LINK-ANCHOR");
        assert_eq!(classify("broken link: `x.tmd`").0, "TAL-LINK");
    }

    #[test]
    fn unsupported_key_outranks_the_generic_citation_needles() {
        // The `csl:` message names citation concepts, so it would classify as TAL-CITE-BIB
        // if its needle were ordered after them. It is a front-matter defect (delete the
        // key), not a bibliography one, so pin both the code and the ordering.
        // Produced by the real rule at its real home (the render path), so this keeps
        // pinning the shipped message rather than a copy that could drift from it.
        let csl: Vec<_> = crate::frontmatter::validate_front_matter(
            "---\ntitle: T\ncsl: apa.csl\n---\n\nBody.\n",
        )
        .into_iter()
        .filter(|w| w.message.contains("is recognized but not supported"))
        .collect();
        assert_eq!(csl.len(), 1, "the csl warning: {csl:?}");
        assert_eq!(
            classify(&csl[0].message),
            ("TAL-FM-UNSUPPORTED", WARNING),
            "csl classifies as an unsupported front-matter key: {}",
            csl[0].message
        );
        // No replacement exists, so no structured suggestion may be lifted: an agent must
        // not be handed a fix to apply.
        assert_eq!(extract_suggestion(&csl[0].message), None);
    }

    #[test]
    fn uncatalogued_message_gets_a_stable_generic_code() {
        let (code, sev) = classify("something entirely new");
        assert_eq!((code, sev), (GENERIC, ERROR));
        assert!(!code.is_empty());
    }

    #[test]
    fn extracts_a_did_you_mean_replacement() {
        assert_eq!(
            extract_suggestion("unknown front-matter key `treme` (did you mean `theme`?)")
                .as_deref(),
            Some("theme")
        );
        assert_eq!(
            extract_suggestion(
                "broken cross-reference: @fig-reslts (did you mean `@fig-results`?)"
            )
            .as_deref(),
            Some("@fig-results")
        );
        assert_eq!(extract_suggestion("no hint here").as_deref(), None);
    }
}
