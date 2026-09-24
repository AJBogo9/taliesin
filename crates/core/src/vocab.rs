//! The language server's static vocabulary: every closed-set construct taliesin recognizes,
//! with the human description its tooltip shows.
//!
//! Front-matter keys (top-level + nested) and their closed values, cell options, callout
//! kinds, structural div classes and attributes, input types, cross-reference prefixes, math
//! commands, cell languages. The lists are sourced from the SAME consts the validator uses,
//! so a completion can never drift from what the validator enforces. Human descriptions are
//! additive doc text authored here (the consts carry none), which `descriptions_present`
//! requires for every name.
//!
//! `lsp.rs` reads these typed tables in-process (`resolve_completion`, `xref_label`,
//! `frontmatter_key_doc`). They were one `serde_json::Value` until 2026-09-24, the dump
//! format of a `taliesin vocab` verb Wave 2 cut, and a string-keyed read of a key that had
//! left it answered `Null` in silence twice (`theoremKinds`, `frontmatterValues`).
//!
//! **This is the OFFERED subset, not the implemented set** — see `render::DIV_FEATURE_CLASSES`
//! and the validator consts for what the tool actually supports.

use crate::frontmatter::{EXECUTE_KEYS, HERO_ACTION_KEYS, HERO_KEYS, KNOWN_KEYS, LISTING_KEYS};

pub use crate::math_vocab::MathCommand;

/// One offered name and the description its tooltip shows.
pub type Named = (&'static str, &'static str);

/// `(name, description)` for each key in `names`, looking each description up in `desc`
/// (missing -> empty string, which the `descriptions_present` test forbids).
fn named(names: &[&'static str], desc: &[Named]) -> Vec<Named> {
    names
        .iter()
        .map(|n| {
            (
                *n,
                desc.iter().find(|(k, _)| k == n).map_or("", |(_, d)| *d),
            )
        })
        .collect()
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
        (
            "categories",
            "Tags for the page; each becomes a `<category>` in the Atom feed.",
        ),
        ("image", "Social-card and listing thumbnail image path."),
        ("image-alt", "Alt text for `image`."),
        (
            "draft",
            "`true` excludes the page from a site build, nav, and listings.",
        ),
        (
            "title-block-style",
            "`none` suppresses the visible title header.",
        ),
        (
            "toc",
            "Force the table of contents on or off (otherwise it is automatic).",
        ),
        ("bibliography", "Path(s) to `.bib` file(s) for citations."),
        ("execute", "Document-level code-cell execution defaults."),
        ("listing", "Auto-generated listing of child pages."),
        ("hero", "Landing-page hero block configuration."),
    ]
}

fn nested_key_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        // execute:
        ("cache", "Persist the cell's output in `_freeze/`."),
        // listing:
        ("contents", "Glob(s) of pages to include."),
        ("id", "Listing element id."),
        (
            "type",
            "Listing layout (`default` text rows, `list` rows with thumbnails).",
        ),
        ("max-items", "Maximum entries shown."),
        // hero:
        ("eyebrow", "Small label above the headline."),
        ("headline", "Hero headline."),
        ("lead", "Hero lead paragraph."),
        ("actions", "Call-to-action buttons."),
        // hero.actions[]:
        ("href", "Where the button links."),
        (
            "primary",
            "`true` styles this as the filled, primary button.",
        ),
        // theorems:
        // shared across blocks (hero.actions/listing reuse these):
        ("text", "The button's visible label."),
        ("title", "A human-readable name for this entry."),
        ("description", "A one-line description of this entry."),
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
        ("code-fold", "Collapse the code block (`true` or `show`)."),
        ("code-summary", "Summary label for a folded code block."),
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
    ]
}

/// Structural fenced-div classes offered to the editor. These are a subset of
/// `render::DIV_FEATURE_CLASSES` (the near-miss anchor for the div-class did-you-mean); the
/// `div_classes_are_a_subset_of_the_validator_vocab` test pins that so the two can't drift.
/// Keep in sync with the `.class` dispatch in `render/divs.rs`.
const DIV_CLASS_NAMES: &[&str] = &["column-margin", "column-page"];

/// The structural div classes offered, with their descriptions.
pub fn div_classes() -> Vec<Named> {
    named(
        DIV_CLASS_NAMES,
        &[
            ("column-margin", "Place content in the margin."),
            (
                "column-page",
                "Widen content past the text column, up to the page width.",
            ),
        ],
    )
}

/// Which classes read a given fenced-div attribute.
enum DivScope {
    /// A div carrying no feature class. `layout-ncol` also wins over the width escapes in
    /// the dispatch chain (it is tested second, right after the callout arm), but offering it
    /// on a `.column-page` would recommend silently replacing the escape with a grid — a
    /// footgun, not a feature — so it is offered only where it is the intended gesture.
    Generic,
    /// Every `callout-<kind>`.
    Callouts,
}

/// One offered fenced-div ATTRIBUTE (`key=value` inside `::: {…}`), and which classes
/// actually read it.
///
/// **The per-class narrowing is the whole point.** `render/divs.rs` dispatches on class in an
/// if-else chain, so an attribute is not a property of divs in general: `collapse=` is read
/// only inside the callout arm, and offering it on a `.column-page` would have the editor
/// recommend a no-op. Front matter had its own version of that failure until 2026-08-20,
/// when the one recognized-but-inert key (`csl:`) was withdrawn rather than kept
/// offered-but-excluded, and `UNSUPPORTED_KEYS` went with it.
///
/// `width` is deliberately ABSENT. `validate::validate_column_width` warns that the
/// equal-width grid ignores it, so completing it would recommend the exact thing `check`
/// flags.
pub struct DivAttribute {
    pub name: &'static str,
    pub description: &'static str,
    /// The value half as an LSP snippet body: `$1` for free text, `${1|a,b|}` for a closed set.
    value: &'static str,
    scope: &'static [DivScope],
    /// A value that must CHANGE the rendered HTML, for [`tests::every_div_attribute_is_live`].
    /// `icon` needs `false` exactly: any other value takes the default branch and renders
    /// identically, so a laxer probe would pass while proving nothing.
    ///
    /// Test-only, but it lives on the struct rather than in a side table so a new attribute
    /// cannot be added WITHOUT one — a missing probe would silently skip the liveness gate
    /// for exactly the entry nobody has checked yet.
    #[cfg_attr(not(test), allow(dead_code))]
    probe: &'static str,
}

/// The attribute vocabulary, derived from the `attrs.get(…)` dispatch in `render/divs.rs`.
/// An attribute no branch reads is a no-op the editor must not recommend, which
/// [`tests::every_div_attribute_is_live`] enforces by rendering every pair below.
const DIV_ATTRIBUTES: &[DivAttribute] = &[
    DivAttribute {
        name: "title",
        description: "Heading text for the box (else a leading heading, else the kind).",
        value: "$1",
        scope: &[DivScope::Callouts],
        probe: "T",
    },
    DivAttribute {
        name: "collapse",
        description: "Fold into a `<details>`: `true` starts closed, `false` starts open.",
        value: "${1|true,false|}",
        scope: &[DivScope::Callouts],
        probe: "true",
    },
    DivAttribute {
        name: "layout-ncol",
        description: "Lay the div's content out as an N-column grid.",
        value: "$1",
        scope: &[DivScope::Generic],
        probe: "3",
    },
];

impl DivAttribute {
    /// The class names this attribute is offered on. **Empty means "a div with no feature
    /// class"** ([`DivScope::Generic`]), which is how the editor reads it — no entry below
    /// mixes `Generic` with a named class, so the two readings cannot collide.
    pub fn classes(&self) -> Vec<String> {
        let mut out = Vec::new();
        for s in self.scope {
            match s {
                DivScope::Generic => {}
                DivScope::Callouts => out.extend(
                    crate::render::CALLOUT_KINDS
                        .iter()
                        .map(|k| format!("callout-{k}")),
                ),
            }
        }
        out
    }

    /// The completion's insert text: `name="value"`, the value an LSP snippet body.
    pub fn snippet(&self) -> String {
        format!("{}=\"{}\"", self.name, self.value)
    }
}

/// The fenced-div attributes offered, each narrowed to the classes that read it.
pub fn div_attributes() -> &'static [DivAttribute] {
    DIV_ATTRIBUTES
}

/// The languages offered for a ` ```{lang} ` cell, as `(name, description)`.
///
/// Two of these have behaviour and the rest are highlighting. The split is not cosmetic —
/// `render::executes_to_kernel` decides whether a cell can produce a numbered float, and the
/// completion marks the executed ones by asking it — so `kernel_languages_are_offered` pins
/// that every language it accepts is in this table.
pub const CELL_LANGUAGES: &[Named] = &[
    (
        "python",
        "Executed by a Jupyter kernel; output is spliced in.",
    ),
    (
        "js",
        "Reactive cell, run in the reader's browser (no kernel).",
    ),
    ("mermaid", "Diagram rendered at build time."),
    ("bash", "Highlighted only; not executed."),
    ("sql", "Highlighted only; not executed."),
    ("julia", "Highlighted only; not executed."),
    ("rust", "Highlighted only; not executed."),
];

/// The `@`-prefixes offered to an author and to an agent: [`XREF_LABELS`] entire, as
/// `(prefix, label)`, with no filter. It used to subtract a `RETIRED_XREF_PREFIXES` list of
/// seven theorem prefixes the renderer still resolved a label for but nothing could define a
/// target for; those tuples were deleted on 2026-08-18, so "what resolves" and "what is
/// offered" are the same slice.
///
/// [`XREF_LABELS`]: crate::cite::XREF_LABELS
pub fn xref_prefixes() -> &'static [Named] {
    crate::cite::XREF_LABELS
}

/// The nested front-matter blocks whose children have their own vocabulary, as `(path,
/// keys)`. `hero.actions` is a path into `hero:` (one entry of its `actions:` list), not a
/// parent word.
const NESTED: &[(&str, &[&str])] = &[
    ("execute", EXECUTE_KEYS),
    ("listing", LISTING_KEYS),
    ("hero", HERO_KEYS),
    ("hero.actions", HERO_ACTION_KEYS),
];

/// The front-matter keys offered at the top level (`parent` is `None`) or inside a nested
/// block (`"execute"`, `"hero.actions"`, …), or `None` for a parent with no vocabulary.
///
/// Every known key is offered. There was an exclusion here until 2026-08-20, for the one key
/// taliesin recognized but ignored (`csl:`) -- completing it would have been the tool
/// recommending a no-op. That key was withdrawn rather than kept inert, so the set and the
/// offer are the same thing again.
pub fn frontmatter_keys(parent: Option<&str>) -> Option<Vec<Named>> {
    match parent {
        None => Some(named(KNOWN_KEYS, frontmatter_key_descriptions())),
        Some(p) => NESTED
            .iter()
            .find(|(k, _)| *k == p)
            .map(|(_, keys)| named(keys, nested_key_descriptions())),
    }
}

/// The top-level keys whose immediate children have their own vocabulary (`execute`,
/// `listing`, `hero`).
pub fn nested_parents() -> impl Iterator<Item = &'static str> {
    NESTED.iter().map(|(k, _)| *k).filter(|k| !k.contains('.'))
}

const ON_OFF: &[Named] = &[("true", "Turn it on."), ("false", "Turn it off.")];

/// The front-matter keys whose value is a closed set, as `(key, values)`. A key is matched
/// by name at any depth, which is sound because each name here occurs once in the
/// vocabulary: `cache` only under `execute:`, `type` only under `listing:`, `primary` only in
/// a `hero.actions` entry.
const FRONTMATTER_VALUES: &[(&str, &[Named])] = &[
    ("toc", ON_OFF),
    ("draft", ON_OFF),
    ("cache", ON_OFF),
    ("primary", ON_OFF),
    (
        "title-block-style",
        &[("none", "Hide the visible title header.")],
    ),
    ("type", &[("list", "Rows with thumbnails.")]),
];

/// The closed value set of front-matter key `key`, or `&[]` when its value is free.
pub fn frontmatter_values(key: &str) -> &'static [Named] {
    FRONTMATTER_VALUES
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(&[], |(_, v)| *v)
}

/// The `#|` cell options offered, with their descriptions.
pub fn cell_options() -> Vec<Named> {
    named(crate::render::CELL_OPTION_KEYS, cell_option_descriptions())
}

/// The callout kinds offered (`note`, not `callout-note`), with their descriptions.
pub fn callout_kinds() -> Vec<Named> {
    named(crate::render::CALLOUT_KINDS, callout_descriptions())
}

/// The `{{< input type= >}}` control kinds.
pub fn input_types() -> &'static [&'static str] {
    crate::render::INPUT_TYPES
}

/// The math commands offered inside `$…$`. The one vocabulary taliesin does not own the
/// grammar of. It is authoritative anyway because KaTeX is IN the binary: `math_vocab`'s
/// `every_command_renders` renders each entry through `crate::math`, so an offered command
/// that KaTeX cannot parse fails the build instead of shipping a suggestion that renders as
/// a red error span for the reader.
pub fn math_commands() -> &'static [MathCommand] {
    crate::math_vocab::MATH_COMMANDS
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

    /// Every name carries a non-empty description, so a new validator const forces the
    /// author to add doc text here instead of silently shipping a blank tooltip.
    #[test]
    fn descriptions_present() {
        let mut lists: Vec<(String, Vec<Named>)> = vec![
            ("frontmatter".into(), frontmatter_keys(None).unwrap()),
            ("cellOptions".into(), cell_options()),
            ("calloutKinds".into(), callout_kinds()),
            ("divClasses".into(), div_classes()),
            (
                "divAttributes".into(),
                div_attributes()
                    .iter()
                    .map(|a| (a.name, a.description))
                    .collect(),
            ),
        ];
        for (parent, _) in NESTED {
            lists.push((parent.to_string(), frontmatter_keys(Some(parent)).unwrap()));
        }
        for (key, values) in FRONTMATTER_VALUES {
            lists.push((format!("{key}: values"), values.to_vec()));
        }
        for (where_, list) in lists {
            assert!(!list.is_empty(), "{where_} offers nothing");
            for (name, desc) in list {
                assert!(
                    !desc.is_empty(),
                    "empty description for `{name}` in {where_}"
                );
            }
        }
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

    /// Every language `executes_to_kernel` accepts must be OFFERED. That function decides
    /// whether a labelled cell can produce a numbered float, and the completion marks the
    /// executed languages by asking it, so a kernel language missing from the table would
    /// never be offered and nothing else could see it.
    #[test]
    fn kernel_languages_are_offered() {
        let lang = "python";
        assert!(
            crate::render::executes_to_kernel(lang),
            "`{lang}` is expected to be a kernel language"
        );
        assert!(
            CELL_LANGUAGES.iter().any(|(n, _)| *n == lang),
            "kernel language `{lang}` is not offered to the editor"
        );
    }

    /// Every nested parent is itself an offered top-level key, so a retired block (`about:`,
    /// `prose-lint:`, both still offered a nested vocabulary on 2026-08-17 while the linter
    /// squiggled them) cannot keep one.
    #[test]
    fn every_nested_parent_is_an_offered_key() {
        let parents: Vec<&str> = nested_parents().collect();
        assert_eq!(parents, ["execute", "listing", "hero"]);
        for p in parents {
            assert!(
                KNOWN_KEYS.contains(&p),
                "nested parent `{p}` is not a known key"
            );
        }
    }

    /// Every closed front-matter value set belongs to a live key and holds only values that
    /// key's reader accepts, so completion cannot offer a value the page then ignores.
    #[test]
    fn every_offered_front_matter_value_is_read() {
        let live =
            |k: &str| KNOWN_KEYS.contains(&k) || NESTED.iter().any(|(_, ks)| ks.contains(&k));
        for (key, values) in FRONTMATTER_VALUES {
            assert!(
                live(key),
                "`{key}:` offers values but is not a front-matter key"
            );
            if *values == ON_OFF {
                for (v, _) in *values {
                    let yaml = serde_yaml::Value::String(v.to_string());
                    assert!(
                        crate::frontmatter::value_bool(&yaml).is_some(),
                        "`{key}: {v}` does not read as a boolean"
                    );
                }
            }
        }
        let types: Vec<&str> = frontmatter_values("type").iter().map(|(v, _)| *v).collect();
        assert_eq!(
            types,
            crate::frontmatter::LISTING_TYPES,
            "`listing: type:` offers exactly what it reads"
        );
        // `title-block-style: none` is the value that hides the title header.
        let emits = |fm: &str| crate::render::emits_title_block(&format!("title: T\n{fm}"));
        assert!(emits("") && !emits("title-block-style: none\n"));
        assert_eq!(
            frontmatter_values("title"),
            &[],
            "a free-text key has no value set"
        );
    }

    /// Render one fenced div and return its HTML with `data-block-id` stripped.
    ///
    /// Stripping is load-bearing, not tidiness: `build_container` derives the block id from
    /// `format!("div:{}", span.attrs)`, so **every** attribute change perturbs the id. Compare
    /// the raw HTML and the liveness gate below passes for an attribute nothing reads —
    /// a vacuous test that looks like a strong one.
    fn div_html(class: &str, attrs: &str) -> String {
        let src = format!("::: {{.{class}{attrs}}}\nBody.\n:::\n");
        let html: String = crate::render::render_document(&src)
            .blocks
            .iter()
            .map(|b| b.html.as_str())
            .collect();
        let needle = " data-block-id=\"";
        let mut out = String::new();
        let mut rest = html.as_str();
        while let Some(i) = rest.find(needle) {
            out.push_str(&rest[..i]);
            let after = &rest[i + needle.len()..];
            match after.find('"') {
                Some(j) => rest = &after[j + 1..],
                None => {
                    rest = "";
                    break;
                }
            }
        }
        out.push_str(rest);
        out
    }

    /// **Every offered div attribute must actually do something on every class it is offered
    /// on.** This is what makes `DIV_ATTRIBUTES` authoritative rather than a wish list, the
    /// same role `math_vocab::every_command_renders` plays for math: an attribute the renderer
    /// ignores is a no-op the editor would be recommending, and the author would sit there
    /// wondering why `collapse="true"` did nothing to their lemma.
    ///
    /// It earned its place immediately — the first draft of this table gave `collapse` to
    /// every theorem kind, and this test showed only `proof` had a collapse branch (both are
    /// gone now, but the gate is what found it).
    #[test]
    fn every_div_attribute_is_live() {
        for a in DIV_ATTRIBUTES {
            let classes = a.classes();
            // An empty scope is `Generic`: a div with no feature class, where the dispatch
            // chain falls through to the generic arm.
            let targets = if classes.is_empty() {
                vec!["tali-probe-generic".to_string()]
            } else {
                classes
            };
            for class in targets {
                let plain = div_html(&class, "");
                let with = div_html(&class, &format!(" {}=\"{}\"", a.name, a.probe));
                assert_ne!(
                    plain, with,
                    "`{}=` is offered on `.{class}` but changes nothing there: \
                     the editor would be recommending a no-op",
                    a.name
                );
            }
        }
    }

    /// The negative direction, for the narrowings most easily got wrong. Without these
    /// the table could widen back to "every attribute on every div" and `every_div_attribute_is_live`
    /// would still pass — it only checks the pairs the table already claims.
    #[test]
    fn narrowed_div_attributes_are_no_ops_off_their_class() {
        for (class, attr, probe) in [
            // `layout-ncol` is offered on a plain div only: the callout arm is tested FIRST
            // in the dispatch chain, so it wins and the grid never fires.
            ("callout-note", "layout-ncol", "3"),
            // Callout attributes on a width escape: it falls through to the generic arm,
            // which reads neither.
            ("column-page", "collapse", "true"),
            ("column-page", "title", "T"),
        ] {
            assert_eq!(
                div_html(class, ""),
                div_html(class, &format!(" {attr}=\"{probe}\"")),
                "`{attr}=` is expected to be inert on `.{class}`; if this now works, \
                 widen DIV_ATTRIBUTES instead of deleting this case"
            );
        }
    }

    /// Every attribute names a real class. A scope pointing at a class the renderer does
    /// not dispatch on would be offered where it can never fire, and
    /// `every_div_attribute_is_live` would catch it only by the render diff — this says so
    /// directly.
    #[test]
    fn div_attribute_classes_are_real() {
        use crate::render::{CALLOUT_KINDS, DIV_FEATURE_CLASSES};
        for a in DIV_ATTRIBUTES {
            for class in a.classes() {
                let known = DIV_FEATURE_CLASSES.contains(&class.as_str())
                    || CALLOUT_KINDS
                        .iter()
                        .any(|k| class == format!("callout-{k}"));
                assert!(
                    known,
                    "`{}=` is offered on `.{class}`, which is not a class the renderer knows",
                    a.name
                );
            }
        }
    }
}
