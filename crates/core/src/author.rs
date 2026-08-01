//! Structured `author:` front matter.
//!
//! One declaration feeds every consumer that has to say something about a person: the
//! visible byline (`render/mod.rs`), the Scholar `citation_author` /
//! `citation_author_institution` tags and JSON-LD `Person`/`affiliation`
//! (`site/meta.rs`), the Atom feed's `<author>` (`site/feed.rs`), and the three citation
//! formats in the "Cite this" box (`site/cite_this.rs`). Before this existed each of
//! those read a flat `Vec<String>`, so an affiliation had nowhere to live at all.
//!
//! **Every older spelling stays valid**, which is not politeness — `author:` is a scalar
//! in every document in the corpus, and a parse that broke them would take the byline off
//! each one silently (the byline is generated furniture, so no document's source mentions
//! it):
//!
//! ```yaml
//! author: Ada Lovelace                    # scalar
//! author: ["Ada Lovelace", "Grace Hopper"]  # list of scalars
//! author:                                 # structured
//!   - name: Ada Lovelace
//!     affiliation: Analytical Engine Institute
//!     orcid: 0000-0002-1825-0097
//!     url: https://example.org/ada
//!     equal: true
//!   - name: Charles Babbage
//!     affiliation: [Analytical Engine Institute, Somewhere Else]
//! ```
//!
//! **Affiliations are named where they are used, and numbered here.** The convention this
//! form is modelled on (Distill, and every LaTeX class) makes the author write an index —
//! `affiliation: 1` against a separate `affiliations:` list. That was the shape proposed
//! when the item was filed, and it is rejected deliberately: an index is a second thing to
//! keep in sync, reordering the list silently re-attributes every author, and it buys a
//! reader nothing the string does not. Repeating the institution's name instead is
//! impossible to get wrong, and the superscript numbers a reader sees are derived from
//! first appearance (see [`affiliation_index`]) — same output, one less way to be wrong,
//! and no new top-level key. This is the project's own "perfect the default before adding
//! a knob" rule applied to a shape that was already written down.

/// The sub-keys a structured author entry may carry. A typo here is worth a warning
/// rather than silence: the value is dropped, and the only symptom is an affiliation
/// that never appears on the page.
pub(crate) const AUTHOR_KEYS: &[&str] = &[
    "name",
    "affiliation",
    "orcid",
    "url",
    "email",
    "equal",
    "contribution",
];

/// One declared author. `name` is the only required part; everything else is absent in
/// the scalar spellings above and stays absent rather than being invented.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Author {
    pub name: String,
    pub affiliations: Vec<String>,
    pub orcid: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
    /// `equal: true` — an equal-contribution marker, the one piece of author metadata a
    /// paper page carries that is about the *authorship* rather than the person.
    pub equal: bool,
    /// `contribution:` — what this person actually did ("Designed the study and wrote the
    /// analysis"). Rendered as an Author Contributions entry in the appendix.
    ///
    /// A sub-key rather than a top-level `contributions:` map keyed by name, because a map
    /// would have to match a name string back to an author and would silently drop the
    /// entry when the two spellings differed. Declared beside the name, it cannot miss.
    pub contribution: Option<String>,
}

impl Author {
    /// A bare name, the shape every scalar spelling produces.
    pub(crate) fn named(name: impl Into<String>) -> Self {
        Author {
            name: name.into(),
            ..Default::default()
        }
    }
}

impl From<&str> for Author {
    fn from(name: &str) -> Self {
        Author::named(name)
    }
}

/// Parse an `author:` value in any of its spellings. Unknown sub-keys are reported
/// through `warnings` (located by the caller) instead of being dropped in silence.
///
/// A mapping with no `name:` is skipped rather than emitted as an anonymous author: an
/// empty byline entry would render as a stray separator, and the warning says which key
/// was expected.
pub(crate) fn parse(v: Option<&serde_yaml::Value>) -> (Vec<Author>, Vec<String>) {
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    match v {
        None => {}
        Some(serde_yaml::Value::Sequence(seq)) => {
            for item in seq {
                push_one(item, &mut warnings, &mut out);
            }
        }
        Some(other) => push_one(other, &mut warnings, &mut out),
    }
    (out, warnings)
}

fn push_one(v: &serde_yaml::Value, warnings: &mut Vec<String>, out: &mut Vec<Author>) {
    match v {
        serde_yaml::Value::Mapping(map) => {
            let mut a = Author::default();
            for (k, val) in map {
                let Some(key) = k.as_str() else { continue };
                match key {
                    "name" => a.name = scalar(val).unwrap_or_default(),
                    "orcid" => a.orcid = scalar(val),
                    "url" => a.url = scalar(val),
                    "email" => a.email = scalar(val),
                    "equal" => {
                        a.equal = val
                            .as_bool()
                            .or_else(|| val.as_str().and_then(crate::frontmatter::yaml_bool_word))
                            == Some(true);
                    }
                    "affiliation" => a.affiliations = string_list(val),
                    "contribution" => a.contribution = scalar(val),
                    other => warnings.push(crate::frontmatter::unknown_key_message(
                        "author key",
                        other,
                        AUTHOR_KEYS,
                    )),
                }
            }
            if a.name.trim().is_empty() {
                warnings.push(
                    "an `author:` entry has no `name:`, so there is nothing to put in the \
                     byline; the entry is ignored"
                        .to_string(),
                );
                return;
            }
            out.push(a);
        }
        // A scalar author. Kept verbatim — `cite_this` is what knows how to split a
        // `Family, Given` or an ` and `-joined list, and it must keep seeing the raw
        // string to do it.
        other => {
            if let Some(s) = scalar(other) {
                out.push(Author::named(s));
            }
        }
    }
}

fn scalar(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn string_list(v: &serde_yaml::Value) -> Vec<String> {
    match v {
        serde_yaml::Value::Sequence(seq) => seq.iter().filter_map(scalar).collect(),
        other => scalar(other).into_iter().collect(),
    }
}

/// The page's distinct affiliations, in first-appearance order. The position of an
/// affiliation in this list (1-based) is the superscript a reader sees beside a name, so
/// two authors who type the same institution share a number without being told to.
pub(crate) fn affiliation_index(authors: &[Author]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in authors {
        for aff in &a.affiliations {
            if !out.iter().any(|s| s == aff) {
                out.push(aff.clone());
            }
        }
    }
    out
}

/// The 1-based marker numbers for one author's affiliations, against [`affiliation_index`].
pub(crate) fn marks(author: &Author, index: &[String]) -> Vec<usize> {
    author
        .affiliations
        .iter()
        .filter_map(|aff| index.iter().position(|s| s == aff).map(|i| i + 1))
        .collect()
}

/// Just the names, for the consumers that only ever wanted a `Vec<String>` (the Atom
/// feed's `<author>`, `cite_this`'s name splitter).
pub(crate) fn names(authors: &[Author]) -> Vec<String> {
    authors.iter().map(|a| a.name.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(yaml: &str) -> (Vec<Author>, Vec<String>) {
        let v: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        parse(v.get("author"))
    }

    #[test]
    fn every_older_spelling_still_parses() {
        // The compatibility contract. `author:` is a scalar in every corpus document, so
        // a regression here takes the byline off all of them at once — and silently,
        // since a byline is generated and no document's source mentions it.
        let (a, w) = parse_str("author: Ada Lovelace\n");
        assert_eq!(a, vec![Author::named("Ada Lovelace")]);
        assert!(w.is_empty());

        let (a, _) = parse_str("author: [\"Ada Lovelace\", \"Grace Hopper\"]\n");
        assert_eq!(
            a,
            vec![Author::named("Ada Lovelace"), Author::named("Grace Hopper")]
        );

        // The ` & `-joined spelling stays ONE raw string here: splitting it is
        // `cite_this::parse_authors`'s job and it needs the original.
        let (a, _) = parse_str("author: \"Ada Lovelace & Grace Hopper\"\n");
        assert_eq!(a, vec![Author::named("Ada Lovelace & Grace Hopper")]);
    }

    #[test]
    fn a_structured_entry_carries_its_affiliation_and_ids() {
        let (a, w) = parse_str(concat!(
            "author:\n",
            "  - name: Ada Lovelace\n",
            "    affiliation: Analytical Engine Institute\n",
            "    orcid: 0000-0002-1825-0097\n",
            "    url: https://example.org/ada\n",
            "    equal: true\n",
        ));
        assert!(w.is_empty(), "no warnings: {w:?}");
        assert_eq!(
            a,
            vec![Author {
                name: "Ada Lovelace".into(),
                affiliations: vec!["Analytical Engine Institute".into()],
                orcid: Some("0000-0002-1825-0097".into()),
                url: Some("https://example.org/ada".into()),
                email: None,
                equal: true,
                contribution: None,
            }]
        );
    }

    #[test]
    fn affiliations_number_themselves_by_first_appearance() {
        // The reason there is no `affiliations:` key and no hand-written index: two
        // authors who name the same institution share a number because the STRING is the
        // same, with nothing to keep in sync and no way to reorder into a wrong answer.
        let (a, _) = parse_str(concat!(
            "author:\n",
            "  - name: A\n",
            "    affiliation: [Inst One, Inst Two]\n",
            "  - name: B\n",
            "    affiliation: Inst One\n",
            "  - name: C\n",
        ));
        let idx = affiliation_index(&a);
        assert_eq!(idx, vec!["Inst One".to_string(), "Inst Two".to_string()]);
        assert_eq!(marks(&a[0], &idx), vec![1, 2]);
        assert_eq!(marks(&a[1], &idx), vec![1], "same string, same number");
        assert_eq!(marks(&a[2], &idx), Vec::<usize>::new(), "no affiliation");
    }

    #[test]
    fn a_typo_in_a_sub_key_warns_instead_of_vanishing() {
        // `affiliaton:` would otherwise be dropped in silence, and the only symptom is an
        // institution that never appears — on a page whose author cannot see why.
        let (a, w) = parse_str("author:\n  - name: Ada\n    affiliaton: Somewhere\n");
        assert_eq!(a, vec![Author::named("Ada")]);
        assert_eq!(w.len(), 1, "one warning: {w:?}");
        assert!(
            w[0].contains("affiliaton") && w[0].contains("affiliation"),
            "the warning should name the typo AND the intended key: {}",
            w[0]
        );
    }

    #[test]
    fn an_entry_with_no_name_is_dropped_with_a_warning() {
        let (a, w) = parse_str("author:\n  - affiliation: Somewhere\n");
        assert!(a.is_empty(), "no anonymous author is invented: {a:?}");
        assert_eq!(w.len(), 1, "and it says so: {w:?}");
    }

    #[test]
    fn yaml_one_dot_one_booleans_reach_equal() {
        // `equal: yes` is a plain STRING to serde_yaml (which follows YAML 1.2), exactly
        // the trap `draft: yes` already documents in site/frontmatter.rs.
        let (a, _) = parse_str("author:\n  - name: Ada\n    equal: yes\n");
        assert!(a[0].equal, "`equal: yes` must not be read as false");
    }
}
