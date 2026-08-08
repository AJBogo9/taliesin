//! The language server's static vocabulary: every closed-set construct taliesin recognizes,
//! with the human description its tooltip shows.
//!
//! Front-matter keys (top-level + nested), cell options, callout/theorem kinds, structural
//! div classes, input types, cross-reference prefixes, math commands. The lists are sourced
//! from the SAME consts the validator uses, so a completion can never drift from what the
//! validator enforces. Human descriptions are additive doc text authored here (the consts
//! carry none), which `descriptions_present` requires for every name.
//!
//! [`vocab`] builds this as one `serde_json::Value` and `lsp.rs` reads keys out of it —
//! `resolve_completion`, `xref_label`, `frontmatter_key_doc`, the math picker's table.
//! It used to be dumped verbatim by a `taliesin vocab` verb and golden-locked against a
//! committed `tali-vocab.json`; Wave 2 cut both. The JSON shape stays because it is what the
//! wire carries, not because anything is written to disk.
//!
//! **This is the OFFERED subset, not the implemented set** — see `render::DIV_FEATURE_CLASSES`
//! and the validator consts for what the tool actually supports.

use serde_json::{Value, json};

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
            "footer",
            "Deck-only: persistent footer text shown on every slide.",
        ),
        ("logo", "Deck-only: logo image shown in a slide corner."),
        (
            "format",
            "Output format (for example `deck`); an extension owns its sub-keys.",
        ),
        ("theme", "Named theme or theme overrides."),
        ("page-layout", "Page width and layout mode."),
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
        ("theorems", "Theorem-environment counter configuration."),
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
            "Listing layout (`default` text rows, `grid` cards, `list` rows with thumbnails).",
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
        ("shared", "Kinds that share one counter."),
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
    ]
}

fn theorem_descriptions() -> &'static [(&'static str, &'static str)] {
    &[
        ("theorem", "Numbered theorem."),
        ("lemma", "Numbered lemma."),
        ("corollary", "Numbered corollary."),
        ("definition", "Numbered definition."),
        ("proof", "Proof block (unnumbered)."),
    ]
}

/// Structural fenced-div classes offered to the editor. These are a subset of
/// `render::DIV_FEATURE_CLASSES` (the near-miss anchor for the div-class did-you-mean); the
/// `div_classes_are_a_subset_of_the_validator_vocab` test pins that so the two can't drift.
/// Keep in sync with the `.class` dispatch in `render/divs.rs`.
const DIV_CLASS_NAMES: &[&str] = &[
    "panel-tabset",
    "code-walkthrough",
    "scrolly",
    "magic-move",
    "step",
    "column-margin",
    "column-page",
    "column-screen",
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
            (
                "column-page",
                "Widen content past the text column, up to the page width.",
            ),
            (
                "column-screen",
                "Widen content to the full width of the screen.",
            ),
        ],
    )
}

/// Which classes read a given fenced-div attribute.
enum DivScope {
    /// A div carrying no feature class. `layout-ncol` also wins over most feature classes in
    /// the dispatch chain (it is tested second, right after the callout arm), but offering it
    /// on a `.step` would recommend silently replacing the step with a grid — a footgun, not
    /// a feature — so it is offered only where it is the intended gesture.
    Generic,
    /// Every `callout-<kind>`.
    Callouts,
    /// Every theorem kind, `proof` included.
    Theorems,
    /// One literal class name.
    Class(&'static str),
}

/// One offered fenced-div ATTRIBUTE (`key=value` inside `::: {…}`), and which classes
/// actually read it.
///
/// **The per-class narrowing is the whole point.** `render/divs.rs` dispatches on class in an
/// if-else chain, so an attribute is not a property of divs in general: `state=` is read only
/// inside the `.step` arm, and `collapse=` reaches a theorem only through the `proof` arm —
/// a `::: {.lemma collapse="true"}` renders exactly like a `::: {.lemma}`. Offering the union
/// would have the editor recommend a no-op, which is the same failure `UNSUPPORTED_KEYS`
/// exists to prevent for front matter.
///
/// `width` is deliberately ABSENT. `validate::validate_column_width` warns that the
/// equal-width grid ignores it, so completing it would recommend the exact thing `check`
/// flags.
struct DivAttribute {
    name: &'static str,
    description: &'static str,
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
        scope: &[DivScope::Callouts, DivScope::Theorems],
        probe: "T",
    },
    DivAttribute {
        name: "collapse",
        description: "Fold into a `<details>`: `true` starts closed, `false` starts open.",
        value: "${1|true,false|}",
        // Callouts, and of the theorem kinds only `proof` — the numbered arm has no
        // collapse branch at all.
        scope: &[DivScope::Callouts, DivScope::Class("proof")],
        probe: "true",
    },
    DivAttribute {
        name: "icon",
        description: "`false` hides the callout's kind icon.",
        value: "${1|false|}",
        scope: &[DivScope::Callouts],
        probe: "false",
    },
    DivAttribute {
        name: "appearance",
        description: "Callout presentation variant (default boxed).",
        value: "${1|simple,minimal|}",
        scope: &[DivScope::Callouts],
        probe: "simple",
    },
    DivAttribute {
        name: "layout-ncol",
        description: "Lay the div's content out as an N-column grid.",
        value: "$1",
        scope: &[DivScope::Generic],
        probe: "3",
    },
    DivAttribute {
        name: "lines",
        description: "Lines this walkthrough step focuses, for example `1,4-6`.",
        value: "$1",
        scope: &[DivScope::Class("step")],
        probe: "1",
    },
    DivAttribute {
        name: "state",
        description: "Stage state this scrolly step activates.",
        value: "$1",
        scope: &[DivScope::Class("step")],
        probe: "a",
    },
    DivAttribute {
        name: "name",
        description: "Reactive name a `{js}` cell reads the active step's state from.",
        value: "$1",
        scope: &[DivScope::Class("scrolly")],
        probe: "n",
    },
];

impl DivAttribute {
    /// The class names this attribute is offered on. **Empty means "a div with no feature
    /// class"** ([`DivScope::Generic`]), which is how the editor reads it — no entry below
    /// mixes `Generic` with a named class, so the two readings cannot collide.
    fn classes(&self) -> Vec<String> {
        let mut out = Vec::new();
        for s in self.scope {
            match s {
                DivScope::Generic => {}
                DivScope::Callouts => out.extend(
                    crate::render::CALLOUT_KINDS
                        .iter()
                        .map(|k| format!("callout-{k}")),
                ),
                DivScope::Theorems => out.extend(
                    crate::render::THEOREM_KINDS
                        .iter()
                        .map(|k| (*k).to_string()),
                ),
                DivScope::Class(c) => out.push((*c).to_string()),
            }
        }
        out
    }
}

fn div_attributes() -> Value {
    Value::Array(
        DIV_ATTRIBUTES
            .iter()
            .map(|a| {
                json!({
                    "name": a.name,
                    "description": a.description,
                    "snippet": format!("{}=\"{}\"", a.name, a.value),
                    "classes": a.classes(),
                })
            })
            .collect(),
    )
}

/// The languages offered for a ` ```{lang} ` cell, as `(name, description)`.
///
/// Two of these have behaviour and the rest are highlighting. The split is not cosmetic —
/// `executes_to_kernel` decides whether a cell can produce a numbered float — so
/// `kernel_languages_are_marked_as_executed` pins the executed pair against that function
/// rather than against this table's prose.
const CELL_LANGUAGES: &[(&str, &str)] = &[
    (
        "python",
        "Executed by a Jupyter kernel; output is spliced in.",
    ),
    ("r", "Executed by an IRkernel; output is spliced in."),
    (
        "js",
        "Reactive cell, run in the reader's browser (no kernel).",
    ),
    (
        "glsl",
        "Fragment shader drawn to a live canvas in the reader's browser (no kernel).",
    ),
    ("mermaid", "Diagram rendered at build time."),
    ("bash", "Highlighted only; not executed."),
    ("sql", "Highlighted only; not executed."),
    ("julia", "Highlighted only; not executed."),
    ("rust", "Highlighted only; not executed."),
];

fn cell_languages() -> Value {
    Value::Array(
        CELL_LANGUAGES
            .iter()
            .map(|(name, description)| {
                json!({
                    "name": name,
                    "description": description,
                    "executes": crate::render::executes_to_kernel(name),
                })
            })
            .collect(),
    )
}

/// The `@`-prefixes offered to an author and to an agent. Retired prefixes are filtered
/// out: the renderer still resolves a *label* for `prp`/`exm`/`rem` so a leftover `@prp-a`
/// draws `TAL-XREF-UNDEF` instead of passing through silently, but nothing can define one
/// of those targets since the `proposition`/`example`/`remark` environments were retired on
/// 2026-08-03. Offering them told `AGENTS.md`'s reader to write a reference that is
/// guaranteed to be broken.
fn xref_prefixes() -> Value {
    Value::Array(
        crate::cite::XREF_LABELS
            .iter()
            .filter(|(prefix, _)| !crate::cite::RETIRED_XREF_PREFIXES.contains(prefix))
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
        EXECUTE_KEYS, HERO_ACTION_KEYS, HERO_KEYS, KNOWN_KEYS, LISTING_KEYS, THEOREM_KEYS,
        UNSUPPORTED_KEYS,
    };
    use crate::render::{CALLOUT_KINDS, CELL_OPTION_KEYS, INPUT_TYPES, THEOREM_KINDS};

    // A key taliesin recognizes but ignores (`csl:`) must not be OFFERED: completing it
    // is the tool recommending a no-op. It stays in KNOWN_KEYS so an author who writes it
    // is told the honest thing (see `frontmatter::UNSUPPORTED_KEYS`), via
    // `diagnostics::csl_recognized_but_unsupported`.
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
                "hero.actions": named(HERO_ACTION_KEYS, nested_desc),
                "theorems": named(THEOREM_KEYS, nested_desc),
            }
        },
        "cellOptions": named(CELL_OPTION_KEYS, cell_option_descriptions()),
        "calloutKinds": named(CALLOUT_KINDS, callout_descriptions()),
        "theoremKinds": named(THEOREM_KINDS, theorem_descriptions()),
        "divClasses": div_classes(),
        "divAttributes": div_attributes(),
        "inputTypes": Value::Array(INPUT_TYPES.iter().map(|t| json!(t)).collect()),
        "xrefPrefixes": xref_prefixes(),
        "frontmatterValues": frontmatter_value_vocab(),
        // The one vocabulary taliesin does not own the grammar of. It is authoritative
        // anyway because KaTeX is IN the binary: `math_vocab`'s `every_command_renders`
        // renders each entry through `crate::math`, so an offered command that KaTeX
        // cannot parse fails the build instead of shipping a suggestion that renders as a
        // red error span for the reader.
        "mathCommands": crate::math_vocab::math_commands(),
        "cellLanguages": cell_languages(),
    })
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
        for parent in ["execute", "listing", "hero", "hero.actions", "theorems"] {
            check_named(&v["frontmatter"]["nested"][parent], parent);
        }
        check_named(&v["cellOptions"], "cellOptions");
        check_named(&v["calloutKinds"], "calloutKinds");
        check_named(&v["theoremKinds"], "theoremKinds");
        check_named(&v["divClasses"], "divClasses");
        check_named(&v["divAttributes"], "divAttributes");
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

    /// Every language `executes_to_kernel` accepts must be OFFERED, and must be the only
    /// ones marked `executes`. That function decides whether a labelled cell can produce a
    /// numbered float, so a completion that gets the split wrong teaches an author to label
    /// a `{bash}` cell `fig-…` and wait for a figure that never arrives.
    #[test]
    fn kernel_languages_are_marked_as_executed() {
        use crate::render::executes_to_kernel;
        let v = cell_languages();
        for entry in v.as_array().unwrap() {
            let name = entry["name"].as_str().unwrap();
            assert_eq!(
                entry["executes"].as_bool().unwrap(),
                executes_to_kernel(name),
                "`{name}`'s `executes` flag disagrees with render::executes_to_kernel"
            );
        }
        // The reverse direction: a kernel language missing from the list would never be
        // offered, and the loop above could not see it.
        for lang in ["python", "r"] {
            assert!(
                executes_to_kernel(lang),
                "`{lang}` is expected to be a kernel language"
            );
            assert!(
                CELL_LANGUAGES.iter().any(|(n, _)| *n == lang),
                "kernel language `{lang}` is not offered to the editor"
            );
        }
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
    /// It earned its place immediately — the first draft of this table gave `collapse` to all
    /// eight theorem kinds, and this test showed only `proof` has a collapse branch.
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

    /// The negative direction, for the three narrowings most easily got wrong. Without these
    /// the table could widen back to "every attribute on every div" and `every_div_attribute_is_live`
    /// would still pass — it only checks the pairs the table already claims.
    #[test]
    fn narrowed_div_attributes_are_no_ops_off_their_class() {
        for (class, attr, probe) in [
            // A numbered theorem has no collapse branch; only `proof` does.
            ("lemma", "collapse", "true"),
            // Step attributes on a callout: the callout arm wins and never looks at them.
            ("callout-note", "state", "a"),
            ("callout-note", "lines", "1"),
        ] {
            assert_eq!(
                div_html(class, ""),
                div_html(class, &format!(" {attr}=\"{probe}\"")),
                "`{attr}=` is expected to be inert on `.{class}`; if this now works, \
                 widen DIV_ATTRIBUTES instead of deleting this case"
            );
        }
    }

    /// Every attribute names a real class. A `DivScope::Class` pointing at a class the
    /// renderer does not dispatch on would be offered where it can never fire, and
    /// `every_div_attribute_is_live` would catch it only by the render diff — this says so
    /// directly, and covers the aliases too.
    #[test]
    fn div_attribute_classes_are_real() {
        use crate::render::{CALLOUT_KINDS, DIV_FEATURE_CLASSES, THEOREM_KINDS};
        for a in DIV_ATTRIBUTES {
            for class in a.classes() {
                let known = DIV_FEATURE_CLASSES.contains(&class.as_str())
                    || THEOREM_KINDS.contains(&class.as_str())
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
