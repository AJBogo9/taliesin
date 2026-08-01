//! The `links:` resource row — Paper / arXiv / Code / Data under the byline — plus the
//! `venue:` and `award:` badges that sit beside it.
//!
//! This is the one element every fork of the academic project-page template keeps, and the
//! reason is that a research page's job is to hand a reader the artefacts. Rendered by
//! `render::resource_links_html` into the title block; `venue:` additionally feeds Google
//! Scholar's `citation_conference_title` and an `arxiv.org` entry feeds `citation_arxiv_id`
//! (`site/meta.rs`), so the row a reader clicks and the metadata a crawler reads come from
//! one declaration.
//!
//! **A bare URL is the whole spelling.** The label and icon are inferred from the URL,
//! because an author who writes
//!
//! ```yaml
//! links:
//!   - https://arxiv.org/abs/2501.01234
//!   - https://github.com/me/project
//! ```
//!
//! has already said everything: `arxiv.org` is arXiv and `github.com` is code, and asking
//! them to restate it as `{text: arXiv, href: …}` is asking them to keep two things in sync
//! that cannot disagree. The map form stays available for the cases inference cannot reach
//! (a lab-hosted supplement, a project-specific label), and an unrecognised host falls back
//! to its own hostname rather than to a guess — an honest label needs no override.
//!
//! Deliberately NOT `hero:`: that block replaces the title block wholesale and has no icon
//! concept, so putting the row there would make a paper page choose between its byline and
//! its links.

/// The sub-keys a `links:` entry may carry, on the [`crate::author::AUTHOR_KEYS`] precedent:
/// a typo is worth a warning, because the symptom is a link that never appears.
pub(crate) const LINK_KEYS: &[&str] = &["text", "href", "icon"];

/// One resource link. `icon` is a bundled glyph name (see `render::resource_icon`), never a
/// URL or a font reference — a CDN icon font would break the offline guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceLink {
    pub text: String,
    pub href: String,
    pub icon: String,
}

/// The three front-matter keys that make up the resource row, travelling together because
/// they render as one band and are meaningless apart from it.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Resources<'a> {
    pub links: &'a [ResourceLink],
    pub venue: Option<&'a str>,
    pub award: Option<&'a str>,
}

impl Resources<'_> {
    /// Whether the page declared any of the three. A page that declared none emits no row
    /// at all, so its title block is byte-for-byte the one it always emitted.
    pub(crate) fn is_empty(&self) -> bool {
        self.links.is_empty()
            && self.venue.is_none_or(|v| v.trim().is_empty())
            && self.award.is_none_or(|a| a.trim().is_empty())
    }
}

/// The label + icon inferred from a URL. Only hosts whose meaning is unambiguous are
/// claimed; everything else falls through to the hostname, which is always true even when
/// it is not pretty.
fn infer(href: &str) -> (String, String) {
    let lower = href.to_ascii_lowercase();
    // The host, minus scheme and any `www.`, up to the first `/`.
    let host = lower
        .split_once("://")
        .map_or(lower.as_str(), |(_, rest)| rest)
        .trim_start_matches("www.")
        .split('/')
        .next()
        .unwrap_or("")
        .to_string();
    let by_host = match host.as_str() {
        "arxiv.org" => Some(("arXiv", "paper")),
        "github.com" | "gitlab.com" | "codeberg.org" => Some(("Code", "code")),
        "huggingface.co" | "zenodo.org" | "osf.io" => Some(("Data", "data")),
        "doi.org" | "dx.doi.org" => Some(("DOI", "paper")),
        "youtube.com" | "youtu.be" | "vimeo.com" => Some(("Video", "video")),
        _ => None,
    };
    if let Some((text, icon)) = by_host {
        return (text.to_string(), icon.to_string());
    }
    // A PDF is a paper whoever hosts it. Checked after the host table so an arXiv PDF
    // link still reads "arXiv".
    let path_is_pdf = lower
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .ends_with(".pdf");
    if path_is_pdf {
        return ("Paper".to_string(), "paper".to_string());
    }
    // Nothing recognised: the host is the honest label. An empty host (a relative path
    // like `supplement.zip`) has no better answer than a generic one.
    if host.is_empty() {
        ("Link".to_string(), "link".to_string())
    } else {
        (host, "link".to_string())
    }
}

/// Parse a `links:` value: a sequence of bare URLs and/or `{text:, href:, icon:}` maps. A
/// lone entry (not a sequence) is accepted too, so a one-link page needs no list syntax.
///
/// An entry with no `href:` is skipped with a warning rather than rendered as a dead label:
/// a link that goes nowhere is worse than no link.
pub(crate) fn parse(v: Option<&serde_yaml::Value>) -> (Vec<ResourceLink>, Vec<String>) {
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

fn push_one(v: &serde_yaml::Value, warnings: &mut Vec<String>, out: &mut Vec<ResourceLink>) {
    match v {
        serde_yaml::Value::Mapping(map) => {
            let (mut text, mut href, mut icon) = (None, None, None);
            for (k, val) in map {
                let Some(key) = k.as_str() else { continue };
                match key {
                    "text" => text = scalar(val),
                    "href" => href = scalar(val),
                    "icon" => icon = scalar(val),
                    other => warnings.push(crate::frontmatter::unknown_key_message(
                        "links key",
                        other,
                        LINK_KEYS,
                    )),
                }
            }
            let Some(href) = href else {
                warnings.push(
                    "a `links:` entry has no `href:`, so there is nothing to link to; the \
                     entry is ignored"
                        .to_string(),
                );
                return;
            };
            let (inferred_text, inferred_icon) = infer(&href);
            out.push(ResourceLink {
                text: text.unwrap_or(inferred_text),
                icon: icon.unwrap_or(inferred_icon),
                href,
            });
        }
        other => {
            if let Some(href) = scalar(other) {
                let (text, icon) = infer(&href);
                out.push(ResourceLink { text, href, icon });
            }
        }
    }
}

fn scalar(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// The arXiv identifier named by an `arxiv.org` link, for `citation_arxiv_id`.
///
/// Derived rather than declared: an author who links their preprint has already written the
/// id, and a separate `arxiv:` key would be a second copy of it. Handles both the modern
/// `2501.01234` and the legacy `math/0309136` form, and drops a `v2`-style version suffix —
/// Scholar indexes the paper, not the revision.
pub(crate) fn arxiv_id(links: &[ResourceLink]) -> Option<String> {
    links.iter().find_map(|l| {
        let lower = l.href.to_ascii_lowercase();
        let rest = lower
            .split_once("arxiv.org/abs/")
            .or_else(|| lower.split_once("arxiv.org/pdf/"))
            .map(|(_, r)| r)?;
        let id = rest
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .trim_end_matches('/')
            .trim_end_matches(".pdf");
        // A trailing `vN` is a revision, not part of the identifier.
        let id = match id.rsplit_once('v') {
            Some((base, ver)) if !ver.is_empty() && ver.chars().all(|c| c.is_ascii_digit()) => base,
            _ => id,
        };
        (!id.is_empty()).then(|| id.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(yaml: &str) -> (Vec<ResourceLink>, Vec<String>) {
        let v: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        parse(v.get("links"))
    }

    #[test]
    fn a_bare_url_infers_its_label_and_icon() {
        // The whole point of the key: no author should have to type "arXiv" next to an
        // arxiv.org URL.
        let (l, w) = parse_str(concat!(
            "links:\n",
            "  - https://arxiv.org/abs/2501.01234\n",
            "  - https://github.com/me/project\n",
            "  - https://example.org/paper.pdf\n",
        ));
        assert!(w.is_empty(), "no warnings: {w:?}");
        assert_eq!(
            l.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(),
            vec!["arXiv", "Code", "Paper"]
        );
        assert_eq!(
            l.iter().map(|x| x.icon.as_str()).collect::<Vec<_>>(),
            vec!["paper", "code", "paper"]
        );
    }

    #[test]
    fn an_unrecognised_host_labels_itself_rather_than_guessing() {
        // The fallback has to be TRUE, not clever: a wrong guess is worse than a hostname,
        // and it is what makes the override optional rather than mandatory.
        let (l, _) = parse_str("links:\n  - https://www.mylab.ac.uk/supplement\n");
        assert_eq!(l[0].text, "mylab.ac.uk", "www. is stripped: {l:?}");
        assert_eq!(l[0].icon, "link");
    }

    #[test]
    fn an_arxiv_pdf_still_reads_as_arxiv_not_as_a_generic_paper() {
        // Ordering trap: the `.pdf` rule would otherwise shadow the host table.
        let (l, _) = parse_str("links:\n  - https://arxiv.org/pdf/2501.01234v2\n");
        assert_eq!(l[0].text, "arXiv");
    }

    #[test]
    fn the_map_form_overrides_the_inference() {
        let (l, w) = parse_str(
            "links:\n  - { text: Supplementary, href: https://github.com/me/x, icon: data }\n",
        );
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(
            l[0].text, "Supplementary",
            "text: wins over the inferred Code"
        );
        assert_eq!(l[0].icon, "data", "icon: wins over the inferred code");
        assert_eq!(l[0].href, "https://github.com/me/x");
    }

    #[test]
    fn a_map_with_only_text_keeps_the_inferred_icon() {
        // Half an override is still an override of only that half.
        let (l, _) = parse_str("links:\n  - { text: Our repo, href: https://github.com/me/x }\n");
        assert_eq!(l[0].text, "Our repo");
        assert_eq!(
            l[0].icon, "code",
            "the icon is still inferred from the host"
        );
    }

    #[test]
    fn an_entry_with_no_href_is_dropped_with_a_warning() {
        let (l, w) = parse_str("links:\n  - { text: Nowhere }\n");
        assert!(l.is_empty(), "no dead link is rendered: {l:?}");
        assert_eq!(w.len(), 1, "and it says so: {w:?}");
    }

    #[test]
    fn a_typo_in_a_sub_key_warns_instead_of_vanishing() {
        let (l, w) = parse_str("links:\n  - { txt: Paper, href: https://x.org/a.pdf }\n");
        assert_eq!(l.len(), 1, "the link still renders");
        assert_eq!(w.len(), 1, "one warning: {w:?}");
        assert!(w[0].contains("txt") && w[0].contains("text"), "{}", w[0]);
    }

    #[test]
    fn a_single_link_needs_no_list_syntax() {
        let (l, _) = parse_str("links: https://github.com/me/x\n");
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].text, "Code");
    }

    #[test]
    fn the_arxiv_id_is_derived_from_the_link_not_declared() {
        let id = |yaml: &str| arxiv_id(&parse_str(yaml).0);
        assert_eq!(
            id("links:\n  - https://arxiv.org/abs/2501.01234\n"),
            Some("2501.01234".into())
        );
        // A revision suffix is not part of the identifier.
        assert_eq!(
            id("links:\n  - https://arxiv.org/abs/2501.01234v3\n"),
            Some("2501.01234".into())
        );
        // The legacy `archive/number` form.
        assert_eq!(
            id("links:\n  - https://arxiv.org/abs/math/0309136\n"),
            Some("math/0309136".into())
        );
        // A PDF link names the same paper.
        assert_eq!(
            id("links:\n  - https://arxiv.org/pdf/2501.01234.pdf\n"),
            Some("2501.01234".into())
        );
        // ...and no arXiv link invents no id.
        assert_eq!(id("links:\n  - https://github.com/me/x\n"), None);
    }
}
