//! Editor vocabulary dump for the VS Code companion's autocomplete.
//!
//! Emits, as one JSON blob, every closed-set body construct taliesin recognizes:
//! front-matter keys (top-level + nested), cell options, callout/theorem kinds,
//! structural div classes, input types, and cross-reference prefixes. The lists are
//! sourced from the SAME consts the validator and `check` use, so completions can never
//! drift from what `check` enforces. Human descriptions are additive doc text authored
//! here (the consts carry none). Golden-file-locked like `schema.rs`: regenerate ONLY via
//! `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib vocab`, never hand-edit.

use serde_json::{Value, json};

/// The committed vocabulary JSON, bundled so the `taliesin vocab` CLI can print it verbatim
/// (no runtime generation), exactly as `schema.rs` bundles the schemas.
pub const VOCAB_JSON: &str = include_str!("../assets/vocab/tali-vocab.json");

/// `[{ "name", "description" }]` for each key in `names`, looking each description up in
/// `desc` (missing -> empty string, which the `descriptions_present` test forbids).
fn named(names: &[&str], desc: &[(&str, &str)]) -> Value {
    Value::Array(
        names
            .iter()
            .map(|n| {
                let d = desc
                    .iter()
                    .find(|(k, _)| k == n)
                    .map(|(_, d)| *d)
                    .unwrap_or("");
                json!({ "name": n, "description": d })
            })
            .collect(),
    )
}

fn frontmatter_key_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        ("title", "The document or page title."),
        ("subtitle", "A secondary title shown under the title."),
        ("author", "Author name(s)."),
        ("date", "Publication date."),
        (
            "description",
            "Short summary used for listings and social cards.",
        ),
        ("lang", "Content language (BCP-47), for example `en`."),
        ("categories", "Tags used to group the page in listings."),
        ("image", "Social-card and listing thumbnail image path."),
        ("image-alt", "Alt text for `image`."),
        (
            "format",
            "Output format (for example `deck`); an extension owns its sub-keys.",
        ),
        ("theme", "Named theme or theme overrides."),
        ("css", "Extra CSS file(s) to include."),
        ("page-layout", "Page width and layout mode."),
        (
            "draft",
            "`true` excludes the page from a site build, nav, and listings.",
        ),
        (
            "title-block-style",
            "`none` suppresses the visible title header.",
        ),
        ("include-in-header", "Raw HTML injected into `<head>`."),
        (
            "include-before-body",
            "Raw HTML injected at the top of the body.",
        ),
        (
            "include-after-body",
            "Raw HTML injected at the end of the body.",
        ),
        ("toc", "Show a table of contents."),
        ("bibliography", "Path(s) to `.bib` file(s) for citations."),
        ("execute", "Document-level code-cell execution defaults."),
        ("listing", "Auto-generated listing of child pages."),
        ("hero", "Landing-page hero block configuration."),
        (
            "prose-lint",
            "Enable prose linting (`true` or `{ banned: [...] }`).",
        ),
        ("theorems", "Theorem-environment numbering configuration."),
    ]
}

fn nested_key_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        // execute:
        ("echo", "Show the cell's source code."),
        ("include", "Include the cell's output."),
        ("cache", "Persist the cell's output in `_freeze/`."),
        // listing:
        ("contents", "Glob(s) of pages to include."),
        ("id", "Listing element id."),
        ("sort", "Sort field and order."),
        (
            "type",
            "Listing layout (`default` text rows, `grid` cards, `list` rows with thumbnails).",
        ),
        ("max-items", "Maximum entries shown."),
        ("categories", "Show a category filter."),
        // hero:
        ("image", "Hero portrait image path."),
        ("eyebrow", "Small label above the headline."),
        ("headline", "Hero headline."),
        ("lead", "Hero lead paragraph."),
        ("actions", "Call-to-action buttons."),
        // prose-lint:
        ("banned", "Words and phrases to flag."),
        // theorems:
        ("shared", "Kinds that share one counter."),
        (
            "numbered",
            "Whether or when to number (`true`, `false`, `unless-unique`).",
        ),
        // shared across blocks (hero/listing reuse these):
        ("image-alt", "Alt text for the image."),
    ]
}

fn cell_option_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        ("echo", "Show the cell's source code."),
        ("include", "Include the cell's output."),
        ("cache", "Persist the cell's output in `_freeze/`."),
        ("label", "Cross-reference id (for example `fig-scree`)."),
        ("fig-cap", "Figure caption."),
        ("lst-cap", "Listing (code) caption."),
        ("tbl-cap", "Table caption."),
        ("fig-export", "Export the figure as a file."),
        ("code-fold", "Collapse the code block (`true` or `show`)."),
        ("code-summary", "Summary label for a folded code block."),
        ("code-line-numbers", "Show or highlight code line numbers."),
        (
            "name",
            "Reactive `{js}` cell name that other cells can depend on.",
        ),
        ("viewof", "Bind a `{js}` input control to this name."),
        ("input", "Reactive `{js}` inputs this cell depends on."),
    ]
}

fn callout_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        ("note", "Informational callout."),
        ("tip", "Helpful tip callout."),
        ("warning", "Warning callout."),
        ("important", "Important callout."),
        ("caution", "Caution callout."),
    ]
}

fn theorem_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        ("theorem", "Numbered theorem."),
        ("lemma", "Numbered lemma."),
        ("corollary", "Numbered corollary."),
        ("proposition", "Numbered proposition."),
        ("definition", "Numbered definition."),
        ("example", "Numbered example."),
        ("remark", "Numbered remark."),
        ("proof", "Proof block (unnumbered)."),
    ]
}

/// Structural fenced-div classes offered to the editor. These are a subset of
/// `render::DIV_FEATURE_CLASSES` (the near-miss anchor for the div-class did-you-mean); the
/// `div_classes_are_a_subset_of_the_validator_vocab` test pins that so the two can't drift.
/// Keep in sync with the `.class` dispatch in `render/divs.rs` and the aliases in `base.css`.
const DIV_CLASS_NAMES: &[&str] = &[
    "panel-tabset",
    "code-walkthrough",
    "scrolly",
    "magic-move",
    "step",
    "column-margin",
    "aside",
    "sidenote",
    "marginnote",
];

fn div_classes() -> Value {
    named(
        DIV_CLASS_NAMES,
        &[
            (
                "panel-tabset",
                "Tabbed panel; each `##` heading becomes a tab.",
            ),
            ("code-walkthrough", "Step-through narrated code."),
            ("scrolly", "Scroll-driven storytelling section."),
            ("magic-move", "Animated code diff between steps."),
            (
                "step",
                "A step inside a code-walkthrough or scrolly (line focus or stage state).",
            ),
            ("column-margin", "Place content in the margin."),
            ("aside", "Margin aside (alias of `column-margin`)."),
            ("sidenote", "Margin sidenote (alias of `column-margin`)."),
            ("marginnote", "Margin note (alias of sidenote)."),
        ],
    )
}

fn xref_prefixes() -> Value {
    Value::Array(
        crate::cite::XREF_LABELS
            .iter()
            .map(|(prefix, label)| json!({ "prefix": prefix, "label": label }))
            .collect(),
    )
}

/// Suggested VALUES for the front-matter keys that have a small, useful closed set, as
/// `key -> [{name, description}]`. This is a completion aid, not a validated gate: an
/// unrecognized `format:` value just renders as HTML, and a `theme:` may instead name an
/// extension theme or a CSS file. Sourced from the recognizers so it can't drift from
/// behaviour: `render::fm_extract::is_reveal_format` (`deck`, else HTML) and
/// `render::theme::resolve_theme` (the `dark`/`light`/`default` built-ins).
fn frontmatter_value_vocab() -> Value {
    json!({
        "format": [
            { "name": "html", "description": "Standard HTML page (the default)." },
            { "name": "deck", "description": "Slide deck on taliesin's own engine." },
        ],
        "theme": [
            { "name": "dark", "description": "Built-in dark theme." },
            { "name": "light", "description": "Built-in light theme." },
        ],
    })
}

/// Build the vocabulary JSON from the validator's consts.
pub fn vocab() -> Value {
    use crate::frontmatter::{
        EXECUTE_KEYS, HERO_KEYS, KNOWN_KEYS, LISTING_KEYS, PROSE_LINT_KEYS, THEOREM_KEYS,
        UNSUPPORTED_KEYS,
    };
    use crate::render::{CALLOUT_KINDS, CELL_OPTION_KEYS, INPUT_TYPES, THEOREM_KINDS};

    // A key taliesin recognizes but ignores (`csl:`) must not be OFFERED: completing it
    // is the tool recommending a no-op. It stays in KNOWN_KEYS so the did-you-mean can't
    // mis-suggest `css` (see `frontmatter::UNSUPPORTED_KEYS`), and
    // `diagnostics::csl_recognized_but_unsupported` warns if an author writes it anyway.
    let offered: Vec<&str> = KNOWN_KEYS
        .iter()
        .copied()
        .filter(|k| !UNSUPPORTED_KEYS.contains(k))
        .collect();

    let nested_desc = nested_key_descriptions();
    json!({
        "frontmatter": {
            "keys": named(&offered, frontmatter_key_descriptions()),
            "nested": {
                "execute": named(EXECUTE_KEYS, nested_desc),
                "listing": named(LISTING_KEYS, nested_desc),
                "hero": named(HERO_KEYS, nested_desc),
                "prose-lint": named(PROSE_LINT_KEYS, nested_desc),
                "theorems": named(THEOREM_KEYS, nested_desc),
            }
        },
        "cellOptions": named(CELL_OPTION_KEYS, cell_option_descriptions()),
        "calloutKinds": named(CALLOUT_KINDS, callout_descriptions()),
        "theoremKinds": named(THEOREM_KINDS, theorem_descriptions()),
        "divClasses": div_classes(),
        "inputTypes": Value::Array(INPUT_TYPES.iter().map(|t| json!(t)).collect()),
        "xrefPrefixes": xref_prefixes(),
        "frontmatterValues": frontmatter_value_vocab(),
    })
}

/// Deterministic pretty JSON with a trailing newline (so the committed file ends cleanly),
/// matching `schema::generate::to_pretty_json`.
pub fn to_pretty_json() -> String {
    let mut s = serde_json::to_string_pretty(&vocab()).expect("vocab serializes");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The editor's offered div classes must all be near-miss anchors for the div-class
    /// did-you-mean, so a class a user is told exists also gets a "did you mean" when typo'd.
    /// Pins `DIV_CLASS_NAMES ⊆ render::DIV_FEATURE_CLASSES` — add a new class to both or neither.
    #[test]
    fn div_classes_are_a_subset_of_the_validator_vocab() {
        for name in DIV_CLASS_NAMES {
            assert!(
                crate::render::DIV_FEATURE_CLASSES.contains(name),
                "`{name}` is offered by vocab::div_classes() but missing from \
                 render::DIV_FEATURE_CLASSES (typos of it won't get a did-you-mean)"
            );
        }
    }

    /// Assert the generated JSON equals the committed file, OR (under `TALIESIN_BLESS=1`)
    /// rewrite the committed file from the generator. Mirrors `schema.rs`.
    #[test]
    fn vocab_matches_committed() {
        let generated = to_pretty_json();
        if std::env::var("TALIESIN_BLESS").is_ok() {
            let path = format!(
                "{}/assets/vocab/tali-vocab.json",
                env!("CARGO_MANIFEST_DIR")
            );
            std::fs::write(&path, &generated).unwrap_or_else(|e| panic!("write {path}: {e}"));
            eprintln!("blessed assets/vocab/tali-vocab.json");
        } else {
            assert_eq!(
                generated, VOCAB_JSON,
                "vocab drift; regenerate with `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib vocab`"
            );
        }
    }

    /// Every name carries a non-empty description, so a new validator const forces the
    /// author to add doc text here instead of silently shipping a blank tooltip.
    #[test]
    fn descriptions_present() {
        fn check_named(v: &Value, where_: &str) {
            for item in v.as_array().unwrap() {
                let name = item["name"].as_str().unwrap();
                let desc = item["description"].as_str().unwrap();
                assert!(
                    !desc.is_empty(),
                    "empty description for `{name}` in {where_}"
                );
            }
        }
        let v = vocab();
        check_named(&v["frontmatter"]["keys"], "frontmatter.keys");
        for parent in ["execute", "listing", "hero", "prose-lint", "theorems"] {
            check_named(&v["frontmatter"]["nested"][parent], parent);
        }
        check_named(&v["cellOptions"], "cellOptions");
        check_named(&v["calloutKinds"], "calloutKinds");
        check_named(&v["theoremKinds"], "theoremKinds");
        check_named(&v["divClasses"], "divClasses");
        for key in ["format", "theme"] {
            check_named(
                &v["frontmatterValues"][key],
                &format!("frontmatterValues.{key}"),
            );
        }
    }

    /// The value vocab is a completion aid keyed by front-matter key. Pin its content (not
    /// just via the golden file, which a bless could empty): `format` must offer both
    /// recognized names and `theme` both built-ins, mirroring `is_reveal_format` /
    /// `resolve_theme`. Removing a value fails here.
    #[test]
    fn frontmatter_values_offer_format_and_theme() {
        let v = vocab();
        let names = |k: &str| {
            v["frontmatterValues"][k]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["name"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        let format = names("format");
        assert!(
            format.contains(&"html".to_string()) && format.contains(&"deck".to_string()),
            "format values must offer html + deck: {format:?}"
        );
        let theme = names("theme");
        assert!(
            theme.contains(&"dark".to_string()) && theme.contains(&"light".to_string()),
            "theme values must offer dark + light: {theme:?}"
        );
    }

    /// The reverse of `descriptions_present`: every entry in `frontmatter_key_descriptions`
    /// must map to a real `KNOWN_KEY`. The vocab keys are `KNOWN_KEYS` looked up in that
    /// table, so a description for a key NOT in `KNOWN_KEYS` (a retired/renamed key) is dead:
    /// never emitted, never seen by `descriptions_present`, and so accumulates silently.
    #[test]
    fn every_frontmatter_description_maps_to_a_known_key() {
        use crate::frontmatter::KNOWN_KEYS;
        for (key, _) in frontmatter_key_descriptions() {
            assert!(
                KNOWN_KEYS.contains(key),
                "`frontmatter_key_descriptions` carries `{key}`, which is not a KNOWN_KEY: \
                 a retired key leaves dead, never-emitted doc text here"
            );
        }
    }

    /// The bundled string parses as JSON (catches an empty or corrupt committed file).
    #[test]
    fn bundled_vocab_is_valid_json() {
        serde_json::from_str::<Value>(VOCAB_JSON).expect("bundled vocab is valid JSON");
    }
}
