//! The feature-adoption report: which constructs a document uses, and which constructs no
//! document uses (backlog item 202).
//!
//! **Why.** The tool could not answer *what does this document use*, and — the half that
//! matters more — could not answer *which capabilities is nothing exercising*. Taliesin's
//! scope rule is corpus-plus-roadmap: every capability ships pinned by a corpus document.
//! Nothing checked that. Producing the 2026-08-01 feature-value audit's adoption table took
//! a session of `grep`; this makes it one command, and makes the policy self-checking.
//!
//! **Why this is not instrumented into the render.** The obvious design is a recording sink
//! threaded through the render passes, since those are what expand a construct. Two things
//! rule it out. `validate::validate_div_class` is called only from `build_container`'s
//! *generic* arm, so the validator walk never sees a div that matched a feature class and is
//! not the free recording point it looks like; recording would need a new sink at every
//! dispatch arm, each one a place to forget. And warm-server block-level incremental render
//! is the tool's moat, so taxing every render to serve a report nobody runs while editing is
//! the wrong trade. Instead this module REUSES the existing parsers ([`divs::scan_div_attrs`],
//! [`divs::scan_code_fences`], [`extension::scan_shortcodes`],
//! [`crate::frontmatter::front_matter_block`]) from outside the render path. Nothing here
//! changes `RenderedDoc`, the block model, or the four text projections.
//!
//! **What it reports is what the AUTHOR wrote**, which can differ from which dispatch arm
//! won: a div carrying a feature class AND an attribute that outranks it in the dispatch
//! chain counts as both, though only one of them shaped the HTML. For an adoption report
//! that is the right answer (the author reached for both), and it is stated here so it is
//! not read later as a bug.
//!
//! **The catalogue is read from the validator consts, never re-declared** — see
//! [`catalogue`] for why `vocab.rs` is the wrong source.

use std::collections::{BTreeMap, BTreeSet};

/// One closed vocabulary of constructs, as the report groups them.
#[derive(Debug, Clone)]
pub struct Group {
    /// Human heading (`"div classes"`).
    pub name: &'static str,
    /// Stable machine key (`"div-classes"`), the JSON object key.
    pub slug: &'static str,
    /// Every construct in this vocabulary that taliesin implements, sorted.
    pub known: Vec<String>,
}

/// Every construct taliesin implements, grouped.
///
/// **Sourced from the validator consts, not from [`crate::vocab`].** `vocab.rs` is the
/// *offered-completions* projection, not the implemented set: `vocab::DIV_CLASS_NAMES` holds
/// 8 entries where `render::DIV_FEATURE_CLASSES` holds 13, because `fragment`,
/// `incremental`, `notes`, `fade-out` and `highlight` are implemented and deliberately not
/// offered. Building the report on `vocab.rs` would not merely undercount, it would report a
/// live feature as unused when it is only unsuggested. `crate::vocab` is consulted only for
/// the two vocabularies it genuinely owns (cell languages, div attributes), via accessors
/// that read the same tables the completions do.
///
/// `csl` stays in the front-matter group though it is recognized-but-ignored
/// ([`crate::frontmatter::UNSUPPORTED_KEYS`]): a report that silently dropped it would make
/// "no document uses `csl`" indistinguishable from "`csl` is not a key", and the first is a
/// fact about adoption while the second is false.
pub fn catalogue() -> Vec<Group> {
    use crate::frontmatter::{
        EXECUTE_KEYS, HERO_ACTION_KEYS, HERO_KEYS, KNOWN_KEYS, LISTING_KEYS, THEOREM_KEYS,
    };
    use crate::render::{
        CALLOUT_KINDS, CELL_OPTION_KEYS, DIV_FEATURE_CLASSES, INPUT_TYPES, THEOREM_KINDS,
    };

    // Nested keys are qualified by their parent, because the same word means different
    // things in two maps: top-level `categories:` tags a page for listings while
    // `listing.categories` toggles a filter widget. An unqualified list would silently
    // merge such pairs and report one adoption number for two features. `hero.actions` is
    // qualified twice over (`hero.actions.text`) for the same reason.
    let nested: Vec<String> = [
        ("execute", EXECUTE_KEYS),
        ("listing", LISTING_KEYS),
        ("hero", HERO_KEYS),
        ("hero.actions", HERO_ACTION_KEYS),
        ("theorems", THEOREM_KEYS),
    ]
    .iter()
    .flat_map(|(parent, keys)| keys.iter().map(move |k| format!("{parent}.{k}")))
    .collect();

    vec![
        group("front-matter keys", "frontmatter-keys", KNOWN_KEYS),
        Group {
            name: "front-matter sub-keys",
            slug: "frontmatter-subkeys",
            known: sorted(nested),
        },
        group("div classes", "div-classes", DIV_FEATURE_CLASSES),
        Group {
            name: "div attributes",
            slug: "div-attributes",
            known: sorted(crate::vocab::div_attribute_names()),
        },
        group("callout kinds", "callout-kinds", CALLOUT_KINDS),
        group("theorem kinds", "theorem-kinds", THEOREM_KINDS),
        Group {
            name: "cell languages",
            slug: "cell-languages",
            known: sorted(crate::vocab::cell_language_names()),
        },
        group("cell options", "cell-options", CELL_OPTION_KEYS),
        group(
            "shortcodes",
            "shortcodes",
            crate::render::extension::SHORTCODE_NAMES,
        ),
        Group {
            name: "shortcode arguments",
            slug: "shortcode-args",
            known: sorted(crate::render::extension::shortcode_argument_names()),
        },
        group("input types", "input-types", INPUT_TYPES),
        Group {
            name: "cross-reference kinds",
            slug: "xref-kinds",
            known: sorted(crate::cite::xref_prefixes()),
        },
    ]
}

fn group(name: &'static str, slug: &'static str, known: &[&str]) -> Group {
    Group {
        name,
        slug,
        known: sorted(known.iter().copied()),
    }
}

fn sorted<T: AsRef<str>>(items: impl IntoIterator<Item = T>) -> Vec<String> {
    let set: BTreeSet<String> = items.into_iter().map(|s| s.as_ref().to_owned()).collect();
    set.into_iter().collect()
}

/// What one document uses: group slug → the construct names it writes.
///
/// Only names that are in [`catalogue`] are recorded. A custom div class (the div vocabulary
/// is deliberately open) or an unknown front-matter key is a `check` diagnostic, not a
/// feature, and counting it here would make the "unused" tail unreadable.
#[derive(Debug, Default, Clone)]
pub struct DocFeatures {
    pub used: BTreeMap<&'static str, BTreeSet<String>>,
}

impl DocFeatures {
    fn add(&mut self, slug: &'static str, name: impl Into<String>) {
        self.used.entry(slug).or_default().insert(name.into());
    }

    /// Total constructs used, across every group.
    pub fn count(&self) -> usize {
        self.used.values().map(BTreeSet::len).sum()
    }
}

/// Scan one document's source for every catalogued construct it uses.
///
/// Parse-only and filesystem-free: no render, no kernel, no include resolution. An
/// `{{< include >}}`d fragment is scanned as its own document when the walk reaches it, so a
/// construct is attributed to the file that actually contains it rather than to every file
/// that transcludes it.
pub fn scan(src: &str) -> DocFeatures {
    let cat = catalogue();
    let known = |slug: &str| -> BTreeSet<String> {
        cat.iter()
            .find(|g| g.slug == slug)
            .map(|g| g.known.iter().cloned().collect())
            .unwrap_or_default()
    };
    let mut out = DocFeatures::default();

    scan_frontmatter(
        src,
        &mut out,
        &known("frontmatter-keys"),
        &known("frontmatter-subkeys"),
    );
    scan_divs(
        src,
        &mut out,
        &known("div-classes"),
        &known("div-attributes"),
    );
    scan_cells(
        src,
        &mut out,
        &known("cell-languages"),
        &known("cell-options"),
    );
    scan_shortcode_uses(
        src,
        &mut out,
        &known("shortcodes"),
        &known("shortcode-args"),
        &known("input-types"),
    );
    scan_xrefs(src, &mut out, &known("xref-kinds"));
    out
}

/// Front-matter keys, counted **inside the YAML block** rather than grepped from the body.
///
/// This is the trap the 2026-08-01 audit recorded: `docs/guide/reference/frontmatter.tmd`
/// names all 33 keys in its prose and tables, so a body grep would score the page that
/// *documents* every key as the page that *uses* every key, and the unused column would come
/// back empty for the wrong reason.
fn scan_frontmatter(
    src: &str,
    out: &mut DocFeatures,
    known_top: &BTreeSet<String>,
    known_nested: &BTreeSet<String>,
) {
    let Some(fm) = crate::frontmatter::front_matter_block(src) else {
        return;
    };
    let Ok(serde_yaml::Value::Mapping(map)) = serde_yaml::from_str::<serde_yaml::Value>(fm) else {
        return;
    };
    for (k, v) in &map {
        let Some(key) = k.as_str() else { continue };
        if known_top.contains(key) {
            out.add("frontmatter-keys", key);
        }
        // A nested map's children (`execute:`, `hero:`, …), plus `datasets:`, which is a
        // SEQUENCE of maps rather than a map, so its children live one level deeper.
        let children: Vec<&serde_yaml::Mapping> = match v {
            serde_yaml::Value::Mapping(m) => vec![m],
            serde_yaml::Value::Sequence(seq) => seq.iter().filter_map(|i| i.as_mapping()).collect(),
            _ => Vec::new(),
        };
        for child in children {
            for sub in child.keys().filter_map(|s| s.as_str()) {
                let qualified = format!("{key}.{sub}");
                if known_nested.contains(&qualified) {
                    out.add("frontmatter-subkeys", qualified);
                }
            }
        }
    }
}

fn scan_divs(
    src: &str,
    out: &mut DocFeatures,
    known_classes: &BTreeSet<String>,
    known_attrs: &BTreeSet<String>,
) {
    let (classes, attrs) = crate::render::scan_div_attrs(src);
    for c in classes {
        // A callout is `.callout-<kind>`: record the class family once and the kind
        // separately, so "how many documents use callouts at all" and "does anybody use
        // `caution`" are both answerable.
        if let Some(kind) = c.strip_prefix("callout-") {
            out.add("callout-kinds", kind);
            continue;
        }
        if known_classes.contains(&c) {
            out.add("div-classes", &c);
        }
        // Theorem kinds ARE div classes (no namespace prefix), so a `.theorem` is both.
        if crate::render::THEOREM_KINDS.contains(&c.as_str()) {
            out.add("theorem-kinds", c);
        }
    }
    for a in attrs {
        if known_attrs.contains(&a) {
            out.add("div-attributes", a);
        }
    }
}

fn scan_cells(
    src: &str,
    out: &mut DocFeatures,
    known_langs: &BTreeSet<String>,
    known_opts: &BTreeSet<String>,
) {
    for (info, body) in crate::render::scan_code_fences(src) {
        // `{python}` (executed) and `python` (highlighted only) are the same language for
        // adoption purposes. `lang,attr` info strings are real in the wild (734 occurrences
        // across the two external documents the R11 audit read), so stop at the comma.
        let lang = info
            .trim_start_matches('{')
            .split([',', ' ', '}'])
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if known_langs.contains(&lang) {
            out.add("cell-languages", lang);
        }
        for (key, _) in crate::render::cell_option_keys(&body) {
            if known_opts.contains(&key) {
                out.add("cell-options", key);
            }
        }
        // An option may also be written as a FENCE ATTRIBUTE (`{.python code-line-numbers=…}`)
        // instead of a `#|` body line, and the renderer reads both spellings (see
        // `emit.rs::line_number_spec`). Reading only the body made `code-line-numbers` report
        // as used by nobody, because the fence attribute is the ONLY spelling the documents
        // that use it write (`corpus/deck.tmd`, `docs/guide/demo.tmd`).
        //
        // Splitting on whitespace is enough to recover the key even when a quoted value
        // contains spaces: the key is always attached to the token that opens the pair, and
        // the leftover fragments match no catalogued option. The leading `.` of `{.python}`
        // is stripped so the language is never read as an option name.
        for tok in info.split([' ', '\t', '{', '}']) {
            let key = tok
                .split_once('=')
                .map(|(k, _)| k)
                .unwrap_or(tok)
                .trim_start_matches('.');
            if known_opts.contains(key) {
                out.add("cell-options", key);
            }
        }
    }
}

fn scan_shortcode_uses(
    src: &str,
    out: &mut DocFeatures,
    known_names: &BTreeSet<String>,
    known_args: &BTreeSet<String>,
    known_types: &BTreeSet<String>,
) {
    for (name, args) in crate::render::extension::scan_shortcodes(src) {
        if !known_names.contains(&name) {
            continue;
        }
        out.add("shortcodes", &name);
        // An argument is `key=value` OR a bare flag that takes no value (`{{< video
        // tour.mp4 controls >}}`). Reading only the `=` form meant `video.controls` and
        // `video.audio` could never be reported used by any document however many wrote
        // them — a vacuous "unused", which is the one column this report exists for.
        // A positional argument (the source path) yields no catalogued name and is dropped
        // by the membership check below, as before.
        for key in args
            .iter()
            .map(|a| a.split_once('=').map(|(k, _)| k).unwrap_or(a.as_str()))
        {
            let qualified = format!("{name}.{key}");
            if known_args.contains(&qualified) {
                out.add("shortcode-args", qualified);
            }
        }
        if name == "input" {
            for t in args
                .iter()
                .filter_map(|a| a.strip_prefix("type="))
                .map(|t| t.trim_matches(['"', '\'']).to_owned())
            {
                if known_types.contains(&t) {
                    out.add("input-types", t);
                }
            }
        }
    }
}

/// Which cross-reference kinds the document *writes* (`@fig-scree`), which is the adoption
/// question. Whether the target resolves is `check`'s job, not this report's.
fn scan_xrefs(src: &str, out: &mut DocFeatures, known: &BTreeSet<String>) {
    let bytes = src.as_bytes();
    for (i, _) in src.match_indices('@') {
        // Skip an escaped `\@` and an email-ish `x@y`: a reference marker opens a word.
        if i > 0 && !matches!(bytes[i - 1], b' ' | b'\n' | b'\t' | b'(' | b'[' | b'\r') {
            continue;
        }
        let rest = &src[i + 1..];
        let Some(dash) = rest.find('-') else { continue };
        let prefix = &rest[..dash];
        if !prefix.is_empty()
            && prefix.chars().all(|c| c.is_ascii_lowercase())
            && known.contains(prefix)
        {
            out.add("xref-kinds", prefix);
        }
    }
}

/// One construct's adoption: which documents use it.
#[derive(Debug, Clone)]
pub struct FeatureAdoption {
    pub name: String,
    /// Document paths, as the caller labelled them, in the caller's order.
    pub documents: Vec<String>,
}

/// One group's adoption, with its own denominator.
#[derive(Debug, Clone)]
pub struct GroupAdoption {
    pub name: &'static str,
    pub slug: &'static str,
    pub features: Vec<FeatureAdoption>,
}

impl GroupAdoption {
    pub fn known(&self) -> usize {
        self.features.len()
    }
    pub fn used(&self) -> usize {
        self.features
            .iter()
            .filter(|f| !f.documents.is_empty())
            .count()
    }
    pub fn unused(&self) -> usize {
        self.known() - self.used()
    }
}

/// The whole report: every catalogued construct, with the documents that use it.
#[derive(Debug, Clone)]
pub struct Adoption {
    pub documents: usize,
    pub groups: Vec<GroupAdoption>,
}

impl Adoption {
    /// Aggregate per-document scans into the report.
    ///
    /// `docs` is `(label, scan)` in the order the caller wants them reported: a project's
    /// `chapters:`/nav order, or sorted paths for a bare directory walk.
    pub fn build(docs: &[(String, DocFeatures)]) -> Adoption {
        let groups = catalogue()
            .into_iter()
            .map(|g| GroupAdoption {
                name: g.name,
                slug: g.slug,
                features: g
                    .known
                    .into_iter()
                    .map(|name| FeatureAdoption {
                        documents: docs
                            .iter()
                            .filter(|(_, f)| f.used.get(g.slug).is_some_and(|s| s.contains(&name)))
                            .map(|(label, _)| label.clone())
                            .collect(),
                        name,
                    })
                    .collect(),
            })
            .collect();
        Adoption {
            documents: docs.len(),
            groups,
        }
    }

    /// Constructs no document uses, and the total catalogue size.
    pub fn unused_totals(&self) -> (usize, usize) {
        (
            self.groups.iter().map(GroupAdoption::unused).sum(),
            self.groups.iter().map(GroupAdoption::known).sum(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gate that keeps the report honest: every construct name [`scan`] can emit must be
    /// in [`catalogue`], or a document would use a feature the report cannot show. Driven by
    /// a document that writes one construct from every group.
    const EVERY_GROUP: &str = r#"---
title: T
csl: x.csl
execute:
  cache: false
hero:
  headline: H
---

::: {.column-margin}
A margin note.
:::

::: {.callout-tip}
A tip.
:::

::: {.theorem}
A theorem.
:::

::: {layout-ncol=2}
Two up.
:::

```{python}
#| label: fig-scree
#| echo: false
print(1)
```

{{< video clip.mp4 poster=still.png >}}

{{< input type=slider name=rate >}}

See @fig-scree.
"#;

    #[test]
    fn every_scanned_name_is_in_the_catalogue() {
        let f = scan(EVERY_GROUP);
        let cat = catalogue();
        assert!(f.count() > 0, "the fixture must scan as something");
        for (slug, names) in &f.used {
            let g = cat
                .iter()
                .find(|g| g.slug == *slug)
                .unwrap_or_else(|| panic!("scan emitted group `{slug}`, which has no catalogue"));
            for n in names {
                assert!(
                    g.known.contains(n),
                    "scan emitted `{n}` in group `{slug}`, which the catalogue does not list"
                );
            }
        }
    }

    /// The fixture really does exercise every group, so the test above cannot pass by
    /// scanning nothing. This is the positive control: without it, a `scan` that returned
    /// an empty map would satisfy `every_scanned_name_is_in_the_catalogue` vacuously.
    #[test]
    fn the_fixture_reaches_every_group() {
        let f = scan(EVERY_GROUP);
        for g in catalogue() {
            assert!(
                f.used.contains_key(g.slug),
                "group `{}` is unreached by the fixture, so nothing pins its scanner",
                g.slug
            );
        }
    }

    #[test]
    fn it_scans_what_the_author_wrote() {
        let f = scan(EVERY_GROUP);
        let got = |slug: &str| -> Vec<String> {
            f.used
                .get(slug)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default()
        };
        assert_eq!(got("frontmatter-keys"), ["csl", "execute", "hero", "title"]);
        assert_eq!(
            got("frontmatter-subkeys"),
            ["execute.cache", "hero.headline"]
        );
        // `.theorem` is a THEOREM kind, not a div feature class: theorem kinds have no
        // namespace prefix, so that set IS its own dispatch vocabulary.
        assert_eq!(got("div-classes"), ["column-margin"]);
        assert_eq!(got("div-attributes"), ["layout-ncol"]);
        assert_eq!(got("callout-kinds"), ["tip"]);
        assert_eq!(got("theorem-kinds"), ["theorem"]);
        assert_eq!(got("cell-languages"), ["python"]);
        assert_eq!(got("cell-options"), ["echo", "label"]);
        assert_eq!(got("shortcodes"), ["input", "video"]);
        assert_eq!(got("shortcode-args"), ["video.poster"]);
        assert_eq!(got("input-types"), ["slider"]);
        assert_eq!(got("xref-kinds"), ["fig"]);
    }

    /// A shortcode argument written as a BARE FLAG is a use. `{{< video >}}`'s `controls`
    /// and `audio` take no value, so a scan that reads only `key=value` pairs can never
    /// report them however many documents write them — an "unused" that is vacuous rather
    /// than measured. `corpus/media/screencast.tmd` writes both, on purpose, and the
    /// report still called them unused, which is exactly the input a cut decision reads.
    #[test]
    fn a_bare_flag_shortcode_argument_counts_as_a_use() {
        let f = scan("---\ntitle: T\n---\n\n{{< video tour.mp4 controls >}}\n");
        let args = f.used.get("shortcode-args").unwrap();
        assert!(
            args.contains("video.controls"),
            "a bare flag is a use of that argument: {args:?}"
        );
        // The valued form keeps working, and a flag written BEFORE the path still counts
        // (the flags are not positional).
        let f = scan("---\ntitle: T\n---\n\n{{< video audio tour.mp4 captions=t.vtt >}}\n");
        let args = f.used.get("shortcode-args").unwrap();
        assert!(
            args.contains("video.audio") && args.contains("video.captions"),
            "bare and valued arguments are both uses: {args:?}"
        );
    }

    /// A cell option written as a FENCE ATTRIBUTE is a use. `code-line-numbers` is read by
    /// the renderer from either spelling (`emit.rs::line_number_spec`), but the scan read
    /// only the `#|` body lines — so the deck idiom `{.python code-line-numbers="1|2-3"}`,
    /// which is the ONLY spelling `corpus/deck.tmd` and `docs/guide/demo.tmd` use, reported
    /// as nobody using the feature at all.
    #[test]
    fn a_fence_attribute_cell_option_counts_as_a_use() {
        let f =
            scan("---\ntitle: T\n---\n\n```{.python code-line-numbers=\"1|2-3\"}\nx = 1\n```\n");
        let opts = f.used.get("cell-options").unwrap();
        assert!(
            opts.contains("code-line-numbers"),
            "a fence-attribute option is a use: {opts:?}"
        );
        // A word in the info string that is not a catalogued option is not invented as one,
        // and the language itself is never mistaken for an option.
        assert!(
            !opts.contains("python") && opts.len() == 1,
            "only catalogued options are counted: {opts:?}"
        );
    }

    /// A key named only in prose is not a use. This is the audit's recorded trap, and it is
    /// what makes the reference page that documents 33 keys report as using none of them.
    #[test]
    fn a_key_named_in_the_body_is_not_a_use() {
        let doc = "---\ntitle: T\n---\n\nThe `doi:` key takes a DOI, and `venue:` a venue.\n";
        let f = scan(doc);
        let keys = f.used.get("frontmatter-keys").unwrap();
        assert!(keys.contains("title"), "the real key is counted: {keys:?}");
        assert!(
            !keys.contains("doi") && !keys.contains("venue"),
            "a key named in prose must not count as a use: {keys:?}"
        );
    }

    /// A construct shown as an EXAMPLE (a fenced block or an inline code span) is not a use.
    /// Without this the guide's own reference pages read as the heaviest users in the repo.
    #[test]
    fn a_construct_shown_as_an_example_is_not_a_use() {
        let doc = "---\ntitle: T\n---\n\nWrite `{{< video clip.mp4 >}}` like this:\n\n\
                   ````\n{{< embed deck.tmd >}}\n\n::: {.scrolly}\n:::\n````\n";
        let f = scan(doc);
        assert!(
            !f.used.contains_key("shortcodes"),
            "a shortcode in backticks or a fence is an example: {:?}",
            f.used.get("shortcodes")
        );
        assert!(
            !f.used.contains_key("div-classes"),
            "a div inside a fenced block is an example: {:?}",
            f.used.get("div-classes")
        );
    }

    #[test]
    fn adoption_counts_documents_per_feature() {
        let a = Adoption::build(&[
            ("one.tmd".into(), scan("---\ntitle: T\n---\n")),
            ("two.tmd".into(), scan("---\ntitle: T\nsubtitle: S\n---\n")),
        ]);
        assert_eq!(a.documents, 2);
        let fm = a
            .groups
            .iter()
            .find(|g| g.slug == "frontmatter-keys")
            .unwrap();
        let title = fm.features.iter().find(|f| f.name == "title").unwrap();
        assert_eq!(title.documents, ["one.tmd", "two.tmd"]);
        let subtitle = fm.features.iter().find(|f| f.name == "subtitle").unwrap();
        assert_eq!(subtitle.documents, ["two.tmd"]);
        // The half the report exists for: a catalogued key nobody set is present and empty,
        // never absent, or "unused" and "not a feature" would look the same.
        let logo = fm.features.iter().find(|f| f.name == "logo").unwrap();
        assert!(logo.documents.is_empty());
        assert!(fm.unused() > 0 && fm.used() == 2);
    }

    /// The shortcode scan walks a line looking for `{{<`. It must advance by whole UTF-8
    /// characters: advancing one *byte* at a time lands the cursor mid-codepoint on any
    /// line that carries both a shortcode and a non-ASCII character, and the next
    /// `line[i..]` slice panics. Ordinary prose triggers it (an arrow, an em dash, angle
    /// quotes), and it aborted `taliesin features` on 3 of the 25 `docs/guide` pages
    /// (`reference/cli.tmd`, `reference/frontmatter.tmd`, `using/preview.tmd`).
    ///
    /// Each case below is a real line from one of those pages, or a reduction of one.
    #[test]
    fn shortcode_scan_survives_non_ascii_on_the_same_line() {
        let cases = [
            // reference/cli.tmd:41: the arrow sits inside an inline code span.
            "links resolve against the page registry (`.tmd`→`.html`), so a `{{< embed >}}` deck is left alone.",
            // reference/frontmatter.tmd:57: an em dash before the shortcode.
            "out of scope for `check`, ignored on an `{{< embed >}}` target",
            // using/preview.tmd:131: angle quotes after the shortcode.
            "an `{{< include >}}` reads `⟨42 lines⟩`.",
            // The non-ASCII character immediately before, inside and after the braces.
            "→{{< embed deck.tmd >}}",
            "{{< embed deck.tmd >}}→",
            "{{< video clip.mp4 >}} ⟨caption⟩",
            // Multi-byte characters of every UTF-8 width, including an astral-plane emoji.
            "é {{< embed deck.tmd >}} € 𝄞 🎉",
        ];
        for line in cases {
            // The bug was a panic, so merely returning is the assertion that matters.
            let _ = scan(line);
        }

        // Not just "does not panic": the shortcode is still *found* past the non-ASCII.
        let f = scan("prose → more prose {{< embed deck.tmd >}}\n");
        assert_eq!(
            f.used
                .get("shortcodes")
                .map(|s| s.iter().collect::<Vec<_>>()),
            Some(vec![&"embed".to_string()]),
            "the embed after a non-ASCII char must still be detected: {:?}",
            f.used
        );

        // And the inline-code discipline still holds around non-ASCII: a shortcode shown
        // as an example in backticks is an example, not a use.
        let f = scan("→ `{{< embed deck.tmd >}}` ←\n");
        assert!(
            !f.used.contains_key("shortcodes"),
            "a backticked shortcode is an example, even next to non-ASCII: {:?}",
            f.used
        );
    }
}
