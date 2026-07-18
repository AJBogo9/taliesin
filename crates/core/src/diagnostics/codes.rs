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
    // A DEFINITION no `@ref` can reach: a hidden cell's `label:` (the executor drops the
    // output that would carry the anchor) or a theorem id with no kind prefix. Distinct
    // from TAL-XREF-UNDEF, which is the reference site's complaint. Both messages embed
    // the author's own label and `classify` is first-hit-wins over the whole string, so
    // this MUST stay above the generic `math`/`bibliography`/`category ` needles below —
    // otherwise `fig-math-model` classifies as TAL-MATH and `tbl-bibliography-counts` as
    // TAL-CITE-BIB. Pinned by `an_unreferenceable_label_outranks_a_needle_in_its_own_label`.
    ("cannot be cross-referenced", "TAL-XREF-UNREF", WARNING),
    ("duplicate heading id", "TAL-DUP-ID", ERROR),
    ("broken in-page link", "TAL-ANCHOR", ERROR),
    ("broken link anchor", "TAL-LINK-ANCHOR", ERROR),
    ("broken link", "TAL-LINK", ERROR),
    // Assets + media.
    ("local asset not found", "TAL-ASSET", ERROR),
    ("local video not found", "TAL-MEDIA", ERROR),
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

// ---- `check --explain <CODE>`: turn a code into cause + canonical fix -----------------

/// The canonical, offline home a `docs_url` anchors into: a committed catalog generated from
/// [`EXPLANATIONS`] (`docs/DIAGNOSTICS.md`), so a per-code `#tal-…` anchor resolves on GitHub
/// without the tool shipping a production docs domain. See [`docs_url`].
pub const DIAGNOSTICS_DOC_URL: &str =
    "https://github.com/AJBogo9/taliesin/blob/main/docs/DIAGNOSTICS.md";

/// The prose expansion of one diagnostic code: what it means, why it fired, and the one
/// canonical edit that clears it (rustc `--explain` style). One per code, kept next to the
/// [`TABLE`] it explains so the two can't drift (a completeness test enforces the pairing).
#[derive(Debug, Clone, Copy)]
pub struct Explanation {
    pub code: &'static str,
    pub title: &'static str,
    pub cause: &'static str,
    pub fix: &'static str,
}

/// One entry per distinct code in [`TABLE`] plus [`GENERIC`]. Grounded in the real
/// validators (the wording of each cause/fix tracks the shipped diagnostic message). A code
/// added to `TABLE` without a row here fails `every_code_has_a_nonempty_explanation`.
const EXPLANATIONS: &[Explanation] = &[
    Explanation {
        code: GENERIC,
        title: "an uncatalogued diagnostic",
        cause: "This diagnostic has not been assigned a specific TAL-* family yet, so it \
                carries the generic code. The message text itself is the guide to what \
                tripped.",
        fix: "Read the message and its location, then act on what it names. If one kind of \
              problem keeps surfacing this way, that family is a candidate for its own code.",
    },
    Explanation {
        code: "TAL-FM-YAML",
        title: "the YAML front matter is malformed",
        cause: "The block between the opening and closing `---` is not valid YAML (an \
                unterminated quote, bad indentation, or a stray tab), so the strict parse \
                rejected it before any field could be read.",
        fix: "Fix the YAML at the reported line: close the quote, align the indentation, or \
              replace tabs with spaces. Every value after a parse error is lost, so the \
              parse must succeed first.",
    },
    Explanation {
        code: "TAL-FM-KEY",
        title: "an unknown key in front matter",
        cause: "A key in the document's front matter (or a nested `execute:`/`listing:`/\
                `hero:`/`theorems:`/`prose-lint:` block) is not in Taliesin's closed \
                vocabulary. It is a typo, or a key from another tool that Taliesin does not \
                implement, so it would be silently ignored.",
        fix: "Correct the key to the nearest valid name (`check --format json` carries a \
              `suggestion.replacement` for a near-miss), or remove it. The front-matter \
              reference lists every recognized key.",
    },
    Explanation {
        code: "TAL-FM-FORMAT",
        title: "an unknown `format:` value",
        cause: "The `format:` field names an output Taliesin does not produce. Taliesin \
                renders HTML only (`html`, `deck`); format names from other tools \
                (`revealjs`, `pdf`, `docx`) have no meaning here.",
        fix: "Use a format Taliesin supports, or drop the field to accept the default. HTML \
              is the only output target; a slide deck is `format: deck`.",
    },
    Explanation {
        code: "TAL-FM-UNSUPPORTED",
        title: "a recognized but unsupported key",
        cause: "This key is one Taliesin knows about but deliberately does not act on (for \
                example `csl:`), so leaving it in implies an effect that never happens.",
        fix: "Remove the key. The behavior it configures in another tool is not part of \
              Taliesin's HTML output.",
    },
    Explanation {
        code: "TAL-CALLOUT-KIND",
        title: "an unknown callout kind",
        cause: "A `::: {.callout-…}` block names a callout type that is not one of Taliesin's \
                kinds (`note`, `tip`, `important`, `warning`, `caution`), so it would render \
                as a plain fenced div with no callout styling.",
        fix: "Change the kind to a supported one (the message suggests the nearest match), \
              e.g. `::: {.callout-important}`.",
    },
    Explanation {
        code: "TAL-CELL-OPTION",
        title: "an unknown cell option",
        cause: "A `#|` / `//|` option line on a code cell uses a key Taliesin does not \
                recognize, so it has no effect on how the cell runs or renders.",
        fix: "Correct the option to a known one (the message suggests the nearest, e.g. \
              `labl` -> `label`), or remove it. See the cell-options reference.",
    },
    Explanation {
        code: "TAL-INPUT-TYPE",
        title: "an unknown input type",
        cause: "A reactive `{{< input >}}` (or `//| input`) declares a widget type Taliesin \
                does not provide, so no control can be built for it.",
        fix: "Use a supported input type (the message suggests the nearest, e.g. \
              `slidr` -> `slider`).",
    },
    Explanation {
        code: "TAL-XREF-UNDEF",
        title: "a cross-reference points at nothing",
        cause: "An @-reference (`@fig-…`, `@sec-…`, `@tbl-…`, `@thm-…`) names a label that no \
                figure, section, table, or theorem in the document defines, so it cannot \
                resolve to a number or a link.",
        fix: "Fix the reference to match a real label (the message suggests the nearest), or \
              add the label to the target you meant to point at.",
    },
    Explanation {
        code: "TAL-XREF-UNREF",
        title: "a label exists but cannot be referenced",
        cause: "A labeled float or theorem cannot be reached by any @ref: either a hidden \
                cell (`include: false`) drops the output that would carry the anchor, or a \
                theorem id is missing its kind prefix (`math-of-primes` rather than \
                `thm-math-of-primes`).",
        fix: "Make the anchor reachable: let the cell show its output, or rename the id with \
              the right prefix (`thm-`, `fig-`, …) so `@id` resolves.",
    },
    Explanation {
        code: "TAL-DUP-ID",
        title: "two headings share an id",
        cause: "Two headings produce the same slug id, so an in-page link or `@sec-` \
                reference to it is ambiguous and jumps to whichever comes first.",
        fix: "Give one heading an explicit distinct id (`## Title {#unique-id}`), or reword \
              it so the auto-generated slugs differ.",
    },
    Explanation {
        code: "TAL-ANCHOR",
        title: "a broken in-page link",
        cause: "A same-page link (`[text](#fragment)`) targets a `#fragment` that no heading \
                or anchor on this page defines, so the click goes nowhere.",
        fix: "Point the link at a real id on the page, or add `{#fragment}` to the heading \
              you meant.",
    },
    Explanation {
        code: "TAL-LINK-ANCHOR",
        title: "a link to a missing anchor on another page",
        cause: "A link to `other.html#fragment` (or `other.tmd#fragment`) resolves to a real \
                page, but that page has no such `#fragment`.",
        fix: "Fix the fragment to a real anchor on the target page, or add the id there.",
    },
    Explanation {
        code: "TAL-LINK",
        title: "a broken relative link",
        cause: "A relative link points at a file or page that does not exist. A `.tmd` link \
                is checked against the site's page registry (it rewrites to the built \
                `.html`); a link into a `mounts:` prefix is exempt.",
        fix: "Correct the path to an existing sibling document or asset. External `http(s)` \
              links and mount prefixes are not checked.",
    },
    Explanation {
        code: "TAL-ASSET",
        title: "a local asset was not found",
        cause: "An image or other local asset (`![](path)`, `src=…`) points at a file that is \
                not on disk relative to the document, so it would render broken.",
        fix: "Fix the path, or add the missing file. Remote `http(s)` assets are not checked.",
    },
    Explanation {
        code: "TAL-MEDIA",
        title: "a local video was not found",
        cause: "A `{{< video clip.mp4 >}}` (or similar) names a local media file that does \
                not exist relative to the document.",
        fix: "Correct the path or add the file. Remote media URLs are not checked.",
    },
    Explanation {
        code: "TAL-REACTIVE",
        title: "a broken reactive graph",
        cause: "An `{js}` reactive cell either reads an input that no cell or `{{< input >}}` \
                defines, or the cells form a dependency cycle so none can run.",
        fix: "Define the missing input (or fix its name; the message suggests the nearest), \
              or break the cycle so the graph is acyclic.",
    },
    Explanation {
        code: "TAL-A11Y-HEADING",
        title: "a heading level skips",
        cause: "The outline jumps a level (for example h2 straight to h4) with nothing in \
                between, which breaks the structure for screen readers and the table of \
                contents.",
        fix: "Add an intervening heading, or demote the skipping heading one level so the \
              outline is contiguous.",
    },
    Explanation {
        code: "TAL-A11Y-NAME",
        title: "an interactive element has no accessible name",
        cause: "A link or button (native `<a href>` / `<button>`, or a role=link/button/tab \
                element) has no text and no label, so assistive tech announces it as unnamed.",
        fix: "Give it visible text, or an `aria-label` / `title`. An icon-only control still \
              needs a name.",
    },
    Explanation {
        code: "TAL-A11Y-ALT",
        title: "an image is missing or has placeholder alt text",
        cause: "An image has no `alt` attribute, or its `alt` is a placeholder like \
                `image`/`photo` that describes nothing, so screen-reader users get no \
                information about it.",
        fix: "Add alt text that describes the image's content and purpose. Use `alt=\"\"` \
              only for a purely decorative image.",
    },
    Explanation {
        code: "TAL-CITE-BIB",
        title: "a citation without a bibliography",
        cause: "The document cites sources (`[@key]`) but no `bibliography:` resolves the \
                keys, or a bare `@key` outside brackets did not render as a citation, so the \
                reference cannot be looked up.",
        fix: "Add a `bibliography:` pointing at your `.bib` and make sure each key exists in \
              it. Wrap a citation you meant as one in brackets: `[@key]`.",
    },
    Explanation {
        code: "TAL-MATH",
        title: "a math expression could not be rendered",
        cause: "A `$…$` / `$$…$$` expression did not parse as valid KaTeX, so it cannot be \
                typeset and falls back to raw source.",
        fix: "Fix the LaTeX at the reported location: balance braces and `\\left`/`\\right`, \
              and use only macros KaTeX supports.",
    },
    Explanation {
        code: "TAL-CODE-LANG",
        title: "an unknown code language",
        cause: "A fenced code block names a language the highlighter does not know, so the \
                block renders as plain text with no syntax highlighting.",
        fix: "Use a recognized language tag, or leave the info string empty for an \
              unhighlighted block.",
    },
    Explanation {
        code: "TAL-CATEGORY",
        title: "a near-miss category splits the archive",
        cause: "A `categories:` value is a case-variant or typo of another category used \
                elsewhere on the site (`Statistics` vs `statistics`), so the listing filter \
                silently forks one topic into two chips.",
        fix: "Normalize the spelling to match the canonical category (the message names it), \
              so every post on the topic shares one chip.",
    },
];

/// The [`Explanation`] for `code`, case-insensitively (`tal-fm-key` == `TAL-FM-KEY`), or
/// `None` for a code with no catalogued explanation.
pub fn explain(code: &str) -> Option<&'static Explanation> {
    let want = code.to_ascii_uppercase();
    EXPLANATIONS.iter().find(|e| e.code == want)
}

/// Every distinct diagnostic code (the [`TABLE`] families plus [`GENERIC`]), sorted +
/// deduped. The candidate set for `--explain`'s did-you-mean, the `--explain` index, and the
/// completion of `check --explain <TAB>`.
pub fn all_codes() -> Vec<&'static str> {
    let mut codes: Vec<&'static str> = TABLE.iter().map(|(_, code, _)| *code).collect();
    codes.push(GENERIC);
    codes.sort_unstable();
    codes.dedup();
    codes
}

/// The canonical `docs_url` for a code: the committed catalog anchored by the lowercased
/// code (`…/DIAGNOSTICS.md#tal-fm-key`). Computed, so it can never drift from the code.
pub fn docs_url(code: &str) -> String {
    format!("{DIAGNOSTICS_DOC_URL}#{}", code.to_ascii_lowercase())
}

/// The `docs/DIAGNOSTICS.md` catalog, rendered from [`EXPLANATIONS`] in [`all_codes`] order.
/// One `## <CODE>` heading per code (so GitHub's heading anchors back [`docs_url`]) with its
/// title, cause, and fix. The committed file is drift-locked to this by
/// `diagnostics_md_matches_committed`.
pub fn diagnostics_markdown() -> String {
    let mut s = String::new();
    s.push_str("# Taliesin diagnostic codes\n\n");
    s.push_str(
        "Every diagnostic `taliesin check` reports carries a stable `TAL-*` code. This \
         catalog expands each into its cause and canonical fix; it is generated from the \
         code table in `crates/core/src/diagnostics/codes.rs`, so do not edit it by hand \
         (regenerate with `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib codes`). The \
         same text is available offline via `taliesin check --explain <CODE>`.\n\n",
    );
    for code in all_codes() {
        let e = explain(code).expect("all_codes is a subset of EXPLANATIONS");
        s.push_str(&format!(
            "## {}\n\n**{}**\n\n{}\n\nTo fix: {}\n\n",
            e.code, e.title, e.cause, e.fix
        ));
    }
    s
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
    fn an_unreferenceable_label_outranks_a_needle_in_its_own_label() {
        // These two messages embed the author's anchor VERBATIM, and `classify` scans the
        // whole string first-hit-wins — so a perfectly ordinary label that happens to
        // contain a later family's needle would hijack the code. Measured before the
        // needle existed: `fig-math-model` classified as TAL-MATH, and the theorem
        // warning (which had no family at all) did the same. Hostile labels on purpose.
        assert_eq!(
            classify(
                "figure label \u{201c}fig-math-model\u{201d} cannot be cross-referenced: \
                 `include: false` drops the cell's output, so nothing carries the anchor \
                 and `@fig-math-model` won't resolve"
            ),
            ("TAL-XREF-UNREF", WARNING)
        );
        assert_eq!(
            classify(
                "table label \u{201c}tbl-bibliography-counts\u{201d} cannot be cross-referenced: \
                 `include: false` drops the cell's output, so nothing carries the anchor \
                 and `@tbl-bibliography-counts` won't resolve"
            ),
            ("TAL-XREF-UNREF", WARNING)
        );
        // The theorem-prefix warning is the same family and was previously uncatalogued
        // (GENERIC/ERROR), so it hijacked the same way.
        assert_eq!(
            classify(
                "theorem id \u{201c}math-of-primes\u{201d} cannot be cross-referenced \
                 (`@math-of-primes` won't resolve); use `thm-math-of-primes`"
            ),
            ("TAL-XREF-UNREF", WARNING)
        );
    }

    #[test]
    fn a_broken_reference_and_an_unreachable_definition_are_different_families() {
        // Two sides of one mistake, and an agent triages them differently: one edits the
        // `@ref`, the other edits the cell that defines it.
        assert_eq!(
            classify("broken cross-reference: @fig-x (no such figure/section/\u{2026})").0,
            "TAL-XREF-UNDEF"
        );
        assert_eq!(
            classify(
                "figure label \u{201c}fig-x\u{201d} cannot be cross-referenced: `include: false` \
                 drops the cell's output, so nothing carries the anchor and `@fig-x` won't resolve"
            )
            .0,
            "TAL-XREF-UNREF"
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

    // ---- DX6: `check --explain <CODE>` catalog ---------------------------------------

    #[test]
    fn every_code_has_a_nonempty_explanation() {
        // The load-bearing drift guard: a new diagnostic family (a code added to TABLE) with
        // no EXPLANATIONS entry fails here, so `check --explain` can never learn a code it
        // can't explain. GENERIC is included: `TAL-CHECK` is a real emitted code.
        for code in all_codes() {
            let e = explain(code).unwrap_or_else(|| panic!("no explanation for {code}"));
            assert!(!e.title.is_empty(), "{code}: empty title");
            assert!(!e.cause.is_empty(), "{code}: empty cause");
            assert!(!e.fix.is_empty(), "{code}: empty fix");
            assert_eq!(e.code, code, "explanation.code echoes its key");
        }
    }

    #[test]
    fn no_orphan_explanations() {
        // Every explained code is a real code (in TABLE or GENERIC), and no code is
        // explained twice. Catches an entry left behind after a code is removed/renamed.
        let codes = all_codes();
        let mut seen = std::collections::BTreeSet::new();
        for e in EXPLANATIONS {
            assert!(
                codes.contains(&e.code),
                "{} is explained but is not a real code",
                e.code
            );
            assert!(seen.insert(e.code), "{} is explained twice", e.code);
        }
    }

    #[test]
    fn all_codes_is_sorted_deduped_and_holds_generic() {
        let codes = all_codes();
        assert!(codes.contains(&GENERIC), "all_codes includes TAL-CHECK");
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(codes, sorted, "all_codes is sorted + deduped");
    }

    #[test]
    fn explain_is_case_insensitive() {
        assert_eq!(
            explain("tal-fm-key").map(|e| e.code),
            explain("TAL-FM-KEY").map(|e| e.code)
        );
        assert_eq!(explain("TAL-FM-KEY").map(|e| e.code), Some("TAL-FM-KEY"));
        assert!(explain("TAL-NOPE").is_none());
    }

    #[test]
    fn docs_url_is_the_computed_lowercase_anchor() {
        assert_eq!(
            docs_url("TAL-XREF-UNREF"),
            format!("{DIAGNOSTICS_DOC_URL}#tal-xref-unref")
        );
        // Every code's url anchors into the same committed catalog.
        for code in all_codes() {
            assert!(docs_url(code).starts_with(DIAGNOSTICS_DOC_URL));
            assert!(docs_url(code).ends_with(&code.to_ascii_lowercase()));
        }
    }

    /// Assert `docs/DIAGNOSTICS.md` equals the generated catalog, OR rewrite it under
    /// `TALIESIN_BLESS=1` (mirrors `schema.rs::bless_or_assert`). `rel` is relative to the
    /// core crate root.
    #[test]
    fn diagnostics_md_matches_committed() {
        let rel = "../../docs/DIAGNOSTICS.md";
        let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel);
        let generated = diagnostics_markdown();
        if std::env::var("TALIESIN_BLESS").is_ok() {
            std::fs::write(&path, &generated).unwrap_or_else(|e| panic!("write {path}: {e}"));
            eprintln!("blessed {rel}");
        } else {
            let committed = std::fs::read_to_string(&path).unwrap_or_default();
            assert_eq!(
                generated, committed,
                "diagnostics catalog drift in {rel}; regenerate with \
                 `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib codes`"
            );
        }
    }
}
