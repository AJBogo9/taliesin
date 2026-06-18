//! Project-wide cross-reference registry: scan each page's source for `{#sec-}`/
//! `{#fig-}`/… anchors (+ a section number for numbered book sections) and rewrite
//! `data-qmd-xref`-marked links to the right page. `use super::*` reaches Page,
//! Book, section_number, Block.

use super::*;

/// Where a cross-referenceable anchor (`sec-x`, `fig-x`, …) lives in the project:
/// its page url and, for a numbered section, its number ("2.1"; empty otherwise).
#[derive(Debug, Clone, Default)]
pub struct XrefTarget {
    pub url: String,
    pub number: String,
}
/// Scan every page's source for cross-referenceable anchors (`{#sec-x}` headings,
/// `{#fig-x}`/`{#eq-x}`/… on other lines), recording each anchor's page url and —
/// for a numbered book section — its number. A lightweight source pass (no render),
/// so cross-page `@ref`s resolve without a second execution. First definition wins.
pub(super) fn scan_xref_targets(
    pages: &[Page],
    book: &Option<Book>,
) -> HashMap<String, XrefTarget> {
    let mut map = HashMap::new();
    for page in pages {
        let Ok(src) = std::fs::read_to_string(&page.input) else {
            continue;
        };
        let chapter = book.as_ref().and_then(|b| {
            b.entries
                .iter()
                .find(|e| e.rel == page.rel)
                .and_then(|e| e.number)
        });
        for (anchor, number) in scan_page_anchors(&src, chapter) {
            map.entry(anchor).or_insert(XrefTarget {
                url: page.url.clone(),
                number,
            });
        }
    }
    map
}
/// The `{#prefix-id}` cross-ref anchors in one page's source, paired with a section
/// number for `{#sec-}` headings in a numbered chapter (empty otherwise). Headings
/// are counted in order so an unlabeled section still advances the numbering.
fn scan_page_anchors(src: &str, chapter: Option<u32>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut counters = [0u32; 5];
    let mut in_front_matter = false;
    let mut in_code = false;
    for (i, line) in src.lines().enumerate() {
        let t = line.trim_start();
        if i == 0 && t == "---" {
            in_front_matter = true;
            continue;
        }
        if in_front_matter {
            in_front_matter = t != "---";
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let level = t.bytes().take_while(|&b| b == b'#').count();
        let is_heading = (1..=6).contains(&level) && t.as_bytes().get(level) == Some(&b' ');
        if is_heading {
            let number = chapter
                .map(|ch| section_number(ch, level, &mut counters))
                .unwrap_or_default();
            if let Some(id) = brace_id(t).filter(|id| is_ref_anchor(id)) {
                out.push((id, number));
            }
        } else if let Some(id) = brace_id(t).filter(|id| is_ref_anchor(id)) {
            out.push((id, String::new())); // a figure/equation anchor: link, no number
        }
    }
    out
}
/// The id from a `{#id …}` attribute block on a line, if any (up to a space, `.`,
/// or `}`).
fn brace_id(line: &str) -> Option<String> {
    let start = line.find("{#")? + 2;
    let rest = &line[start..];
    let end = rest.find([' ', '.', '}']).unwrap_or(rest.len());
    let id = &rest[..end];
    (!id.is_empty()).then(|| id.to_string())
}
/// Whether an id is a Quarto cross-reference anchor (`sec-`, `fig-`, …).
fn is_ref_anchor(id: &str) -> bool {
    ["sec-", "fig-", "tbl-", "eq-", "lst-", "thm-", "def-"]
        .iter()
        .any(|p| id.starts_with(p))
}
/// Rewrite the `data-qmd-xref`-marked links in one block's HTML: a marker whose
/// anchor is a known cross-page target becomes a link to that page (with its
/// number); an unknown anchor is left as the bare-label link `cite` emitted.
pub(super) fn rewrite_cross_refs(
    html: &str,
    targets: &HashMap<String, XrefTarget>,
    current_url: &str,
    up: &str,
) -> String {
    let mut out = String::with_capacity(html.len());
    let mut pos = 0;
    while let Some(rel) = html[pos..].find("<a href=\"#") {
        let start = pos + rel;
        out.push_str(&html[pos..start]);
        let Some(close) = html[start..].find("</a>") else {
            break;
        };
        let end = start + close + "</a>".len();
        out.push_str(&rewrite_one_xref(
            &html[start..end],
            targets,
            current_url,
            up,
        ));
        pos = end;
    }
    out.push_str(&html[pos..]);
    out
}
/// Rewrite one `<a …>` if it is a cross-page xref marker; else return it unchanged.
fn rewrite_one_xref(
    link: &str,
    targets: &HashMap<String, XrefTarget>,
    current_url: &str,
    up: &str,
) -> String {
    let marker = "data-qmd-xref=\"";
    let (Some(ms), Some(gt)) = (link.find(marker), link.find('>')) else {
        return link.to_string();
    };
    let astart = ms + marker.len();
    let Some(alen) = link[astart..].find('"') else {
        return link.to_string();
    };
    let anchor = &link[astart..astart + alen];
    let Some(target) = targets.get(anchor).filter(|t| t.url != current_url) else {
        return link.to_string(); // same page or unknown → leave cite's label link
    };
    // A `@sec-` to a chapter (a whole-number section number, no dot) reads "Chapter
    // N" like Quarto; a subsection keeps cite's "Section" label.
    let label = if anchor.starts_with("sec-")
        && !target.number.is_empty()
        && !target.number.contains('.')
    {
        "Chapter"
    } else {
        &link[gt + 1..link.len() - "</a>".len()]
    };
    let number = if target.number.is_empty() {
        String::new()
    } else {
        format!("&nbsp;{}", target.number)
    };
    format!(
        "<a href=\"{up}{}#{anchor}\" class=\"qmd-xref\">{label}{number}</a>",
        target.url
    )
}
