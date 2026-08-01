//! B1: reader-facing "Cite this" export.
//!
//! The **outward** direction: a page's OWN front-matter (`title` + `author` + `date`,
//! plus the site `url`/`title`) serialized into BibTeX, CSL-JSON and RIS, and rendered
//! into a `.tali-cite-this` box appended at the end of the page. This is distinct from
//! [`crate::cite`], which is **inbound** (a `.bib` the page *cites* → IEEE HTML).
//!
//! Output is deterministic — no "Accessed:" date or build timestamp — so the static
//! build stays byte-identical and the freeze cache is never invalidated by it.
//!
//! The render gate (owner ruling 2026-07-18, *site-author fallback*): the box renders
//! iff the page has a non-empty title, a valid `YYYY-MM-DD` date, and at least one
//! author resolved from the page's `author:` **or**, failing that, the site's `author:`.
//! The chain stops at the site author — it never falls back to the site *title*, so the
//! byline is always a real author name or the box is simply absent (degrade to nothing).

use super::{Page, Site, SiteConfig};
use crate::render::{Block, escape_attr as esc};
use serde::Serialize;

/// One parsed author name: an optional given part + a required family part.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Author {
    pub given: Option<String>,
    pub family: String,
}

/// Resolved citation inputs for one page. Built by [`resolve`]; `None` there means the
/// page fails the render gate (missing author/title/date) and no box is emitted.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CiteMeta {
    pub title: String,
    pub authors: Vec<Author>,
    pub year: u32,
    pub month: u32,
    pub day: u32,
    /// Canonical absolute page URL, when the site sets `url:`.
    pub url: Option<String>,
    /// The site title, as the container/venue ("published on X").
    pub venue: Option<String>,
    /// A page that declares a `bibliography:` is treated as a scholarly article (drives
    /// the CSL `type`/RIS `TY`), mirroring the JSON-LD `ScholarlyArticle`/`BlogPosting`
    /// choice in `meta.rs`.
    pub scholarly: bool,
}

/// Split one raw author string into a given/family [`Author`].
///
/// `Family, Given` (a comma) is taken as-is; otherwise the last whitespace token is the
/// family and the rest is the given part. A single token is family-only. Best-effort,
/// matching how `cite::author` reasons about `and`-lists.
pub(crate) fn split_name(raw: &str) -> Author {
    let raw = raw.trim();
    if let Some((family, given)) = raw.split_once(',') {
        let given = given.trim();
        return Author {
            given: (!given.is_empty()).then(|| given.to_string()),
            family: family.trim().to_string(),
        };
    }
    match raw.rsplit_once(char::is_whitespace) {
        Some((given, family)) => {
            let given = given.trim();
            Author {
                given: (!given.is_empty()).then(|| given.to_string()),
                family: family.trim().to_string(),
            }
        }
        None => Author {
            given: None,
            family: raw.to_string(),
        },
    }
}

/// Parse a page/site `authors` list into [`Author`]s, splitting any one entry that joins
/// several names with ` & `, ` and `, or `;` (front matter allows `author: "A & B"`).
pub(crate) fn parse_authors(raw: &[String]) -> Vec<Author> {
    raw.iter()
        .flat_map(|entry| split_author_list(entry))
        .map(|s| split_name(&s))
        .filter(|a| !a.family.is_empty())
        .collect()
}

/// Split one front-matter `author` scalar into individual names on ` & `, ` and `, `;`.
/// A comma is *not* a separator here — it marks `Family, Given` inside one name.
fn split_author_list(entry: &str) -> Vec<String> {
    let mut parts = vec![entry.to_string()];
    for sep in [" & ", " and ", ";"] {
        parts = parts
            .iter()
            .flat_map(|p| p.split(sep))
            .map(|s| s.trim().to_string())
            .collect();
    }
    parts.into_iter().filter(|s| !s.is_empty()).collect()
}

/// Resolve a page + site config (+ canonical url) into [`CiteMeta`], applying the render
/// gate. `None` when the page lacks a title, a valid date, or any author.
pub(crate) fn resolve(page: &Page, config: &SiteConfig, url: Option<String>) -> Option<CiteMeta> {
    let title = page
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())?
        .to_string();
    let (year, month, day) = crate::frontmatter::calendar_date(page.date.as_deref()?)?;
    // Site-author fallback (owner ruling 2026-07-18): page `author:` else site `author:`.
    // The chain stops here — never the site *title* — so the byline is a real name.
    let declared: &[crate::author::Author] = if page.authors.is_empty() {
        &config.authors
    } else {
        &page.authors
    };
    let authors = parse_authors(&crate::author::names(declared));
    if authors.is_empty() {
        return None;
    }
    let venue = config
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from);
    Some(CiteMeta {
        title,
        authors,
        year,
        month,
        day,
        url,
        venue,
        scholarly: page.has_bibliography,
    })
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// ASCII-fold + lowercase + keep only alphanumerics (accents decompose away: `Müller` →
/// `muller`, `Erdős` → `erdos`). Shared by the cite key and its title word.
fn ascii_alnum_lower(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfkd()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

/// The first title word for the cite key: skip a leading article (`the`/`a`/`an`), take
/// the first alphanumeric run of the next word (`The EM-algorithm` → `em`).
fn first_title_word_key(title: &str) -> String {
    for word in title.split_whitespace() {
        if matches!(word.to_lowercase().as_str(), "the" | "a" | "an") {
            continue;
        }
        let run: String = word.chars().take_while(|c| c.is_alphanumeric()).collect();
        let key = ascii_alnum_lower(&run);
        if !key.is_empty() {
            return key;
        }
    }
    String::new()
}

/// A stable BibTeX-style cite key: `<family><year><first-title-word>`, ASCII, lowercased.
fn cite_key(m: &CiteMeta) -> String {
    let family = ascii_alnum_lower(&m.authors[0].family);
    format!("{family}{}{}", m.year, first_title_word_key(&m.title))
}

/// Escape the LaTeX specials that can appear in a front-matter string.
fn bibtex_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '\\' => out.push_str("\\textbackslash{}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            _ => out.push(c),
        }
    }
    out
}

fn bibtex_author(a: &Author) -> String {
    let family = bibtex_escape(&a.family);
    match &a.given {
        Some(g) => format!("{family}, {}", bibtex_escape(g)),
        None => family,
    }
}

/// Serialize to a BibTeX `@misc` entry.
pub(crate) fn to_bibtex(m: &CiteMeta) -> String {
    let authors = m
        .authors
        .iter()
        .map(bibtex_author)
        .collect::<Vec<_>>()
        .join(" and ");
    let mut fields = vec![
        format!("author = {{{authors}}}"),
        format!("title = {{{{{}}}}}", bibtex_escape(&m.title)),
        format!("year = {{{}}}", m.year),
        format!("month = {{{}}}", MONTHS[(m.month as usize - 1).min(11)]),
    ];
    if let Some(url) = &m.url {
        // `howpublished = {\url{...}}`; concatenated to keep the braces readable.
        let mut hp = String::from("howpublished = {\\url{");
        hp.push_str(url);
        hp.push_str("}}");
        fields.push(hp);
    }
    format!("@misc{{{},\n  {}\n}}", cite_key(m), fields.join(",\n  "))
}

#[derive(Serialize)]
struct CslName<'a> {
    family: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    given: Option<&'a str>,
}

#[derive(Serialize)]
struct CslIssued {
    #[serde(rename = "date-parts")]
    date_parts: Vec<Vec<u32>>,
}

#[derive(Serialize)]
struct CslItem<'a> {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'a str,
    author: Vec<CslName<'a>>,
    issued: CslIssued,
    #[serde(rename = "container-title", skip_serializing_if = "Option::is_none")]
    container_title: Option<&'a str>,
    #[serde(rename = "URL", skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
}

/// Serialize to a one-element CSL-JSON array.
pub(crate) fn to_csl_json(m: &CiteMeta) -> String {
    let item = CslItem {
        id: cite_key(m),
        kind: if m.scholarly {
            "article-journal"
        } else {
            "post-weblog"
        },
        title: &m.title,
        author: m
            .authors
            .iter()
            .map(|a| CslName {
                family: &a.family,
                given: a.given.as_deref(),
            })
            .collect(),
        issued: CslIssued {
            date_parts: vec![vec![m.year, m.month, m.day]],
        },
        container_title: m.venue.as_deref(),
        url: m.url.as_deref(),
    };
    serde_json::to_string_pretty(&vec![item]).unwrap_or_default()
}

fn ris_author(a: &Author) -> String {
    match &a.given {
        Some(g) => format!("{}, {}", a.family, g),
        None => a.family.clone(),
    }
}

/// RIS is a line-oriented format: a value may not carry a newline.
fn ris_value(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

/// Serialize to an RIS record.
pub(crate) fn to_ris(m: &CiteMeta) -> String {
    let mut lines = vec![format!(
        "TY  - {}",
        if m.scholarly { "JOUR" } else { "BLOG" }
    )];
    for a in &m.authors {
        lines.push(format!("AU  - {}", ris_author(a)));
    }
    lines.push(format!("TI  - {}", ris_value(&m.title)));
    lines.push(format!("PY  - {}", m.year));
    lines.push(format!("DA  - {:04}/{:02}/{:02}", m.year, m.month, m.day));
    if let Some(url) = &m.url {
        lines.push(format!("UR  - {url}"));
    }
    if let Some(v) = &m.venue {
        lines.push(format!("T2  - {}", ris_value(v)));
    }
    lines.push("ER  -".to_string());
    lines.join("\n")
}

/// The rendered `.tali-cite-this` box: a heading, three format tabs, the three
/// serialized citations (each in a `<pre>` the client reads via `.textContent`), and
/// copy/download buttons. All serialized text is HTML-escaped so nothing in the citation
/// can break out of its `<pre>`. Progressive enhancement: with no JS the reader still
/// sees (and can select) the default BibTeX citation.
pub(crate) fn cite_block_html(m: &CiteMeta) -> String {
    // (data-format key, tab label, download filename, serialized text)
    let formats = [
        ("bibtex", "BibTeX", "citation.bib", to_bibtex(m)),
        ("csl", "CSL-JSON", "citation.json", to_csl_json(m)),
        ("ris", "RIS", "citation.ris", to_ris(m)),
    ];

    let mut tabs = String::from(
        "<div class=\"tali-cite-tabs\" role=\"tablist\" aria-label=\"Citation format\">",
    );
    let mut panes = String::new();
    for (i, (key, label, file, text)) in formats.iter().enumerate() {
        let selected = i == 0;
        // A roving `tabindex`: a tablist is ONE stop in the tab sequence (the selected tab),
        // and the arrow keys move within it — emitted server-side as well as maintained by
        // `17-cite-box.js`, so the shape is right before any JS runs. Without it all three
        // formats were their own Tab stop, and with JS off they were focusable controls that
        // could not do anything.
        let tabindex = if selected { "0" } else { "-1" };
        tabs.push_str(&format!(
            "<button type=\"button\" class=\"tali-cite-tab\" role=\"tab\" \
             data-format=\"{key}\" aria-selected=\"{selected}\" tabindex=\"{tabindex}\">{label}</button>"
        ));
        panes.push_str(&format!(
            "<pre class=\"tali-cite-out\" role=\"tabpanel\" data-format=\"{key}\" \
             data-filename=\"{file}\"{hidden}>{text}</pre>",
            hidden = if selected { "" } else { " hidden" },
            text = esc(text),
        ));
    }
    tabs.push_str("</div>");

    format!(
        "<aside class=\"tali-cite-this\" data-block-id=\"tali-cite-this\" \
         aria-labelledby=\"tali-cite-this-h\">\
         <p id=\"tali-cite-this-h\" class=\"tali-cite-this-title\">Cite this</p>\
         {tabs}{panes}\
         <div class=\"tali-cite-actions\">\
         <button type=\"button\" class=\"tali-cite-copy\">Copy</button>\
         <button type=\"button\" class=\"tali-cite-download\">Download</button>\
         </div></aside>"
    )
}

impl Site {
    /// Append the reader-facing "Cite this" box at the end of a page's content, when the
    /// page carries enough metadata (see [`resolve`]). Called last in `finish_blocks`, so
    /// the static build and the live preview inject identically; a single-root generated
    /// block (`tali-cite-this`, no sourcepos) so the incremental client mounts it cleanly.
    /// A no-op when the gate fails — the box degrades to nothing, never an empty shell.
    pub(super) fn attach_cite_this(&self, page: &Page, blocks: &mut Vec<Block>) {
        let Some(meta) = resolve(page, &self.config, self.abs_page_url(page)) else {
            return;
        };
        blocks.push(Block {
            id: "tali-cite-this".to_string(),
            sourcepos: String::new(),
            source_file: None,
            html: cite_block_html(&meta),
            cell: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn page(title: Option<&str>, date: Option<&str>, authors: &[&str]) -> Page {
        Page {
            input: PathBuf::from("x.tmd"),
            rel: "x.tmd".into(),
            url: "posts/em-algorithm/index.html".into(),
            title: title.map(Into::into),
            date: date.map(Into::into),
            description: None,
            authors: authors
                .iter()
                .map(|s| crate::author::Author::named(*s))
                .collect(),
            card_image: None,
            card_image_alt: None,
            categories: vec![],
            listings: vec![],
            hero: None,
            page_layout: None,
            has_bibliography: false,
            doi: None,
            draft: false,
        }
    }

    fn em_meta() -> CiteMeta {
        CiteMeta {
            title: "The EM-algorithm".into(),
            authors: vec![Author {
                given: Some("Andreas".into()),
                family: "Bogossian".into(),
            }],
            year: 2026,
            month: 4,
            day: 14,
            url: Some("https://andreasbogossian.com/posts/em-algorithm/".into()),
            venue: Some("Andreas Bogossian".into()),
            scholarly: false,
        }
    }

    #[test]
    fn name_splits_given_from_family() {
        assert_eq!(
            split_name("Andreas Bogossian"),
            Author {
                given: Some("Andreas".into()),
                family: "Bogossian".into()
            }
        );
    }

    #[test]
    fn name_in_comma_form_is_taken_as_is() {
        assert_eq!(
            split_name("Bogossian, Andreas"),
            Author {
                given: Some("Andreas".into()),
                family: "Bogossian".into()
            }
        );
    }

    #[test]
    fn single_token_name_is_family_only() {
        assert_eq!(
            split_name("Aristotle"),
            Author {
                given: None,
                family: "Aristotle".into()
            }
        );
    }

    #[test]
    fn one_entry_with_ampersand_splits_into_two_authors() {
        assert_eq!(
            parse_authors(&["Andreas Bogossian & [REDACTED-NAME]".into()]),
            vec![
                Author {
                    given: Some("Andreas".into()),
                    family: "Bogossian".into()
                },
                Author {
                    given: Some("[REDACTED-NAME]".into()),
                    family: "[REDACTED-NAME]".into()
                },
            ]
        );
    }

    #[test]
    fn cite_key_is_family_year_first_title_word_skipping_articles() {
        assert_eq!(cite_key(&em_meta()), "bogossian2026em");
    }

    #[test]
    fn bibtex_golden_single_author() {
        let expected = "\
@misc{bogossian2026em,
  author = {Bogossian, Andreas},
  title = {{The EM-algorithm}},
  year = {2026},
  month = {apr},
  howpublished = {\\url{https://andreasbogossian.com/posts/em-algorithm/}}
}";
        assert_eq!(to_bibtex(&em_meta()), expected);
    }

    #[test]
    fn bibtex_multi_author_and_missing_url() {
        let mut m = em_meta();
        m.authors.push(Author {
            given: Some("[REDACTED-NAME]".into()),
            family: "[REDACTED-NAME]".into(),
        });
        m.url = None;
        let expected = "\
@misc{bogossian2026em,
  author = {Bogossian, Andreas and [REDACTED-NAME]},
  title = {{The EM-algorithm}},
  year = {2026},
  month = {apr}
}";
        assert_eq!(to_bibtex(&m), expected);
    }

    #[test]
    fn bibtex_escapes_latex_specials_in_title() {
        let mut m = em_meta();
        m.title = "Cost & Value: 50% of #1".into();
        assert!(
            to_bibtex(&m).contains("title = {{Cost \\& Value: 50\\% of \\#1}}"),
            "got: {}",
            to_bibtex(&m)
        );
    }

    #[test]
    fn bibtex_escapes_the_backslash_tilde_and_caret_arms_too() {
        // The test above covers the `& % $ # _ { }` arm, whose members all escape the same
        // way. These three expand to control sequences instead, and deleting ANY of the
        // three arms survived the whole suite (mutation-found). An unescaped `\`, `~` or
        // `^` does not merely look wrong: it makes the exported .bib fail to compile.
        let mut m = em_meta();
        m.title = "Big-O ~ n^2 on C:\\drive".into();
        let bib = to_bibtex(&m);
        assert!(bib.contains("\\textbackslash{}"), "got: {bib}");
        assert!(bib.contains("\\textasciitilde{}"), "got: {bib}");
        assert!(bib.contains("\\textasciicircum{}"), "got: {bib}");
    }

    #[test]
    fn csl_json_golden_single_author() {
        let expected = "\
[
  {
    \"id\": \"bogossian2026em\",
    \"type\": \"post-weblog\",
    \"title\": \"The EM-algorithm\",
    \"author\": [
      {
        \"family\": \"Bogossian\",
        \"given\": \"Andreas\"
      }
    ],
    \"issued\": {
      \"date-parts\": [
        [
          2026,
          4,
          14
        ]
      ]
    },
    \"container-title\": \"Andreas Bogossian\",
    \"URL\": \"https://andreasbogossian.com/posts/em-algorithm/\"
  }
]";
        assert_eq!(to_csl_json(&em_meta()), expected);
    }

    #[test]
    fn csl_type_is_article_journal_when_scholarly() {
        let mut m = em_meta();
        m.scholarly = true;
        assert!(to_csl_json(&m).contains("\"type\": \"article-journal\""));
    }

    #[test]
    fn ris_golden_single_author() {
        let expected = "\
TY  - BLOG
AU  - Bogossian, Andreas
TI  - The EM-algorithm
PY  - 2026
DA  - 2026/04/14
UR  - https://andreasbogossian.com/posts/em-algorithm/
T2  - Andreas Bogossian
ER  -";
        assert_eq!(to_ris(&em_meta()), expected);
    }

    #[test]
    fn ris_type_is_jour_when_scholarly() {
        let mut m = em_meta();
        m.scholarly = true;
        assert!(to_ris(&m).starts_with("TY  - JOUR\n"));
    }

    #[test]
    fn resolve_needs_title_date_and_author() {
        let cfg = SiteConfig::default();
        assert!(resolve(&page(None, Some("2026-04-14"), &["A B"]), &cfg, None).is_none());
        assert!(resolve(&page(Some("T"), None, &["A B"]), &cfg, None).is_none());
        assert!(resolve(&page(Some("T"), Some("2026-04-14"), &[]), &cfg, None).is_none());
        // A year-only / invalid date is not a citable date.
        assert!(resolve(&page(Some("T"), Some("2026"), &["A B"]), &cfg, None).is_none());
    }

    /// The venue is the site title, and a site with no usable title has no venue rather than
    /// an empty one. An empty `container-title` is worse than a missing one: it reaches the
    /// exported BibTeX/RIS and the JSON-LD as a field the reader's reference manager shows as
    /// blank, and `to_bibtex` emits the key on the strength of `Some`, not of its contents.
    #[test]
    fn a_site_with_no_usable_title_has_no_venue() {
        let with = |title: Option<&str>| {
            let cfg = SiteConfig {
                title: title.map(Into::into),
                authors: vec!["Ada Lovelace".into()],
                ..Default::default()
            };
            resolve(&page(Some("T"), Some("2026-04-14"), &[]), &cfg, None)
                .expect("resolves")
                .venue
        };
        assert_eq!(
            with(Some("Andreas Bogossian")).as_deref(),
            Some("Andreas Bogossian")
        );
        assert_eq!(with(None), None, "no site title at all");
        assert_eq!(with(Some("")), None, "an empty title is not a venue");
        assert_eq!(with(Some("   ")), None, "nor is a whitespace one");
        // And the title that survives is trimmed, so the venue never carries stray padding.
        assert_eq!(with(Some("  Padded  ")).as_deref(), Some("Padded"));
    }

    #[test]
    fn resolve_falls_back_to_site_author_not_site_title() {
        // No site author yet: the site title is NOT a valid author fallback.
        let cfg = SiteConfig {
            title: Some("Andreas Bogossian".into()),
            ..Default::default()
        };
        assert!(resolve(&page(Some("T"), Some("2026-04-14"), &[]), &cfg, None).is_none());
        // With a site author, the box resolves with that byline.
        let cfg = SiteConfig {
            title: Some("Andreas Bogossian".into()),
            authors: vec!["Ada Lovelace".into()],
            ..Default::default()
        };
        let m = resolve(&page(Some("T"), Some("2026-04-14"), &[]), &cfg, None).unwrap();
        assert_eq!(
            m.authors,
            vec![Author {
                given: Some("Ada".into()),
                family: "Lovelace".into()
            }]
        );
    }

    #[test]
    fn the_render_gate_is_exactly_title_and_date_and_some_author() {
        // The whole gate as a truth table, pinned as one unit BEFORE the author parser is
        // restructured (item 184). Every other test here checks one axis; the risk this
        // guards is different — a change to how authors are *parsed* silently moving the
        // boundary of which pages emit a cite box at all. A page that gains or loses the
        // box is a visible, permanent change to published output, and nothing else would
        // catch it: the box is generated furniture, so no corpus document's source
        // mentions it.
        let site_author = SiteConfig {
            title: Some("A Site".into()),
            authors: vec!["Ada Lovelace".into()],
            ..Default::default()
        };
        let no_site_author = SiteConfig {
            title: Some("A Site".into()),
            ..Default::default()
        };
        /// label, title, date, page authors, site has an author, expect a box.
        type Case = (
            &'static str,
            Option<&'static str>,
            Option<&'static str>,
            &'static [&'static str],
            bool,
            bool,
        );
        let cases: &[Case] = &[
            (
                "everything present",
                Some("T"),
                Some("2026-04-14"),
                &["Grace Hopper"],
                false,
                true,
            ),
            (
                "page author, site author too",
                Some("T"),
                Some("2026-04-14"),
                &["Grace Hopper"],
                true,
                true,
            ),
            (
                "no page author, site author",
                Some("T"),
                Some("2026-04-14"),
                &[],
                true,
                true,
            ),
            (
                "no author anywhere",
                Some("T"),
                Some("2026-04-14"),
                &[],
                false,
                false,
            ),
            (
                "no title",
                None,
                Some("2026-04-14"),
                &["Grace Hopper"],
                true,
                false,
            ),
            (
                "empty title",
                Some("  "),
                Some("2026-04-14"),
                &["Grace Hopper"],
                true,
                false,
            ),
            ("no date", Some("T"), None, &["Grace Hopper"], true, false),
            (
                "unparseable date",
                Some("T"),
                Some("someday"),
                &["Grace Hopper"],
                true,
                false,
            ),
            // Measured, and NOT what it looks like: `author: ""` is a one-element list, so
            // the "page has no author" branch never runs and the site author is never
            // reached; the empty name is then dropped and the box disappears. An explicit
            // blank SUPPRESSES the box rather than falling through to the site. Pinned as
            // the behaviour that ships, not the behaviour anyone would predict.
            (
                "blank page author does NOT fall through to site",
                Some("T"),
                Some("2026-04-14"),
                &[""],
                true,
                false,
            ),
            (
                "blank page author, no site author",
                Some("T"),
                Some("2026-04-14"),
                &[""],
                false,
                false,
            ),
        ];
        for (label, title, date, authors, site_has_author, expect_box) in cases {
            let cfg = if *site_has_author {
                &site_author
            } else {
                &no_site_author
            };
            let got = resolve(&page(*title, *date, authors), cfg, None).is_some();
            assert_eq!(
                got, *expect_box,
                "render gate changed for the `{label}` case: emitted a cite box = {got}"
            );
        }
    }

    #[test]
    fn cite_block_html_embeds_all_three_formats_escaped() {
        let html = cite_block_html(&em_meta());
        assert!(html.contains("class=\"tali-cite-this\""), "got: {html}");
        assert!(html.contains("data-block-id=\"tali-cite-this\""));
        assert!(html.contains(">Cite this<"));
        assert!(html.contains("role=\"tablist\""));
        assert!(html.contains("data-format=\"bibtex\""));
        assert!(html.contains("data-format=\"csl\""));
        assert!(html.contains("data-format=\"ris\""));
        // The serialized citation text is embedded.
        assert!(html.contains("@misc{bogossian2026em,"));
        assert!(html.contains("TY  - BLOG"));
    }

    #[test]
    fn cite_block_html_escapes_angle_brackets_in_serialized_text() {
        let mut m = em_meta();
        m.title = "A <script> study".into();
        let html = cite_block_html(&m);
        assert!(
            !html.contains("<script>"),
            "raw script tag leaked into the block"
        );
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn resolve_prefers_page_author_over_site_author() {
        let cfg = SiteConfig {
            authors: vec!["Ada Lovelace".into()],
            ..Default::default()
        };
        let m = resolve(
            &page(Some("T"), Some("2026-04-14"), &["Alan Turing"]),
            &cfg,
            None,
        )
        .unwrap();
        assert_eq!(
            m.authors,
            vec![Author {
                given: Some("Alan".into()),
                family: "Turing".into()
            }]
        );
    }

    /// PA-B5: a tablist is ONE stop in the tab sequence. All three format tabs used to be
    /// their own stop, so Tab walked through BibTeX / CSL-JSON / RIS one at a time on the way
    /// past the box — and with JS off they were focusable controls that did nothing at all.
    /// Emitted server-side, not only maintained by `17-cite-box.js`, so the shape is correct
    /// before any script runs.
    #[test]
    fn the_format_tablist_is_one_tab_stop() {
        let html = cite_block_html(&em_meta());
        assert_eq!(
            html.matches("tabindex=\"0\"").count(),
            1,
            "exactly one tab is in the tab sequence: {html}"
        );
        assert_eq!(
            html.matches("tabindex=\"-1\"").count(),
            2,
            "the unselected tabs are reachable by arrow key, not by Tab: {html}"
        );
        // The one stop is the SELECTED tab, not just any of them.
        assert!(
            html.contains("aria-selected=\"true\" tabindex=\"0\""),
            "the selected tab is the one in the tab sequence: {html}"
        );
    }
}
