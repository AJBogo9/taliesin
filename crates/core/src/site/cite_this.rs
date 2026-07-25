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
    let raw_authors: &[String] = if page.authors.is_empty() {
        &config.authors
    } else {
        &page.authors
    };
    let authors = parse_authors(raw_authors);
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
        tabs.push_str(&format!(
            "<button type=\"button\" class=\"tali-cite-tab\" role=\"tab\" \
             data-format=\"{key}\" aria-selected=\"{selected}\">{label}</button>"
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
            authors: authors.iter().map(|s| s.to_string()).collect(),
            card_image: None,
            card_image_alt: None,
            categories: vec![],
            listings: vec![],
            hero: None,
            page_layout: None,
            has_bibliography: false,
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
}
