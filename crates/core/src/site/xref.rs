//! Project-wide cross-reference registry: scan each page's source for `{#sec-}`/
//! `{#fig-}`/… anchors (+ a section number for numbered book sections) and rewrite
//! `data-qmd-xref`-marked links to the right page. `use super::*` reaches Page,
//! Book, section_number, Block.

use super::*;
use crate::render::parse_attrs;

/// Where a cross-referenceable anchor (`sec-x`, `fig-x`, …) lives in the project:
/// its page url and, for a numbered section, its number ("2.1"; empty otherwise).
/// `PartialEq` so the dev server can ask whether a refresh actually MOVED anything —
/// a cross-page ref is a dependency the file-level walk cannot see, so "did a target
/// move" is what tells it which open pages to re-render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    warnings: &mut Vec<String>,
) -> HashMap<String, XrefTarget> {
    let mut map: HashMap<String, XrefTarget> = HashMap::new();
    let mut warned: std::collections::HashSet<String> = std::collections::HashSet::new();
    for page in pages {
        let Ok(raw) = std::fs::read_to_string(&page.input) else {
            continue;
        };
        // Resolve `{{< include >}}` first, exactly like the render pipeline does, so
        // the section-number counters advance over included headings too (otherwise a
        // chapter built from includes numbers its sections differently here than in
        // the rendered page, and `@sec-` resolves to the wrong number).
        let base = page
            .input
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let (src, _) = crate::includes::resolve(&raw, base);
        let chapter = super::book::chapter_of(book, page);
        for (anchor, number, line) in scan_page_anchors(&src, chapter) {
            match map.entry(anchor) {
                std::collections::hash_map::Entry::Occupied(e) => {
                    // First definition wins project-wide; warn when a *different*
                    // page redefines the same label (within-page dups are warned by
                    // the per-page render). Otherwise `@x` silently links to whichever
                    // page was discovered first.
                    // Warn once per label (a page can define it twice, which would
                    // otherwise push the identical warning repeatedly). The message is
                    // located at the SECOND (redefining) anchor — the actionable one to
                    // remove/rename — in `file:line:` linter form, and names the first
                    // (winning) page so both sides of the collision are visible.
                    if e.get().url != page.url && warned.insert(e.key().clone()) {
                        warnings.push(format!(
                            "{}:{line}: duplicate cross-reference label \u{201c}{}\u{201d} \u{2014} already defined on {}; this page's anchor is ignored (the first definition wins)",
                            page.rel,
                            e.key(),
                            e.get().url
                        ));
                    }
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(XrefTarget {
                        url: page.url.clone(),
                        number,
                    });
                }
            }
        }
    }
    map
}
/// The ATX heading level of a content line (`## T` -> 2), or `None` if it is not a
/// heading. `#` runs must be followed by a space, so a `#hashtag` is not a heading.
fn heading_level_of(line: &str) -> Option<usize> {
    let level = line.bytes().take_while(|&b| b == b'#').count();
    ((1..=6).contains(&level) && line.as_bytes().get(level) == Some(&b' ')).then_some(level)
}

/// Every heading level in a page's source, in document order — the input
/// [`ChapterNumbering`] derives its base from.
fn heading_levels(src: &str) -> Vec<usize> {
    content_lines_numbered(src)
        .filter_map(|(_, t)| heading_level_of(t))
        .collect()
}

/// The `{#prefix-id}` cross-ref anchors in one page's source, paired with a section
/// number for `{#sec-}` headings in a numbered chapter (empty otherwise). Headings
/// are counted in order so an unlabeled section still advances the numbering.
fn scan_page_anchors(src: &str, chapter: Option<u32>) -> Vec<(String, String, usize)> {
    let mut out = Vec::new();
    // The numbering base is the shallowest heading below the chapter's own, so the whole
    // heading shape has to be known before the first anchor is numbered: pre-scan it.
    // `emits_title_block` is the renderer's own gate, so this scan and the rendered page
    // agree on whether a leading heading is the chapter's title or its first section.
    let levels: Vec<usize> = heading_levels(src);
    let mut numbering = chapter.map(|ch| {
        ChapterNumbering::new(
            ch,
            &levels,
            crate::render::emits_title_block(
                crate::frontmatter::front_matter_block(src).unwrap_or(""),
            ),
        )
    });
    for (line, t) in content_lines_numbered(src) {
        if let Some(level) = heading_level_of(t) {
            let number = numbering
                .as_mut()
                .map(|n| n.next(level))
                .unwrap_or_default();
            if let Some(id) = brace_id(t).filter(|id| is_ref_anchor(id)) {
                out.push((id, number, line));
            }
        } else if let Some(id) = brace_id(t).filter(|id| is_ref_anchor(id)) {
            out.push((id, String::new(), line)); // a figure/equation anchor: link, no number
        }
    }
    out
}
/// The `#id` from a `{…}` attribute block on a line, if any. Scans *every* brace
/// block on the line (so a split-brace heading `## T {.unnumbered} {#sec-x}` is
/// found, not only the first block) and parses each with the renderer's own
/// quote-aware [`parse_attrs`], so a `#` inside a quoted value (`title="see #x"`)
/// is never read as an id and the scan can't drift from what the renderer emits.
fn brace_id(line: &str) -> Option<String> {
    brace_blocks(line)
        .into_iter()
        .find_map(|block| parse_attrs(block).id().map(str::to_string))
}

/// The contents of each top-level `{…}` block on a line, quote-aware so a `}`
/// inside a quoted attribute value (`title="a } b"`) doesn't close the block early.
fn brace_blocks(line: &str) -> Vec<&str> {
    let bytes = line.as_bytes();
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i + 1;
            let mut j = start;
            let mut quote: Option<u8> = None;
            while j < bytes.len() {
                match quote {
                    Some(q) if bytes[j] == q => quote = None,
                    Some(_) => {}
                    None => match bytes[j] {
                        b'"' | b'\'' => quote = Some(bytes[j]),
                        b'}' => break,
                        _ => {}
                    },
                }
                j += 1;
            }
            blocks.push(&line[start..j]);
            i = j + 1;
        } else {
            i += 1;
        }
    }
    blocks
}
/// Whether an id is a cross-reference anchor (`sec-`, `fig-`, …).
pub(super) fn is_ref_anchor(id: &str) -> bool {
    [
        "sec-", "fig-", "tbl-", "eq-", "lst-", "thm-", "lem-", "cor-", "prp-", "def-", "exm-",
        "rem-",
    ]
    .iter()
    .any(|p| id.starts_with(p))
}
/// Every cross-page-reference anchor in a block's HTML: the value of each
/// `data-qmd-xref="…"` marker, in document order (with duplicates, if a block refers
/// to the same anchor twice). `cite` emits this marker *only* for a reference whose
/// target is not on the current page (`cite/render.rs`), so a marker is by
/// construction a cross-page reference — the raw material for the reverse
/// (anchor → referring pages) index.
pub(super) fn xref_markers_in(html: &str) -> Vec<&str> {
    let marker = "data-qmd-xref=\"";
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(rel) = html[pos..].find(marker) {
        let start = pos + rel + marker.len();
        let Some(len) = html[start..].find('"') else {
            break;
        };
        out.push(&html[start..start + len]);
        pos = start + len;
    }
    out
}
/// Resolve every cross-page xref marker in `blocks` against `targets`, as seen from
/// `current_url`. The ONE definition of "apply the registry to a rendered page", shared by
/// the page-render path ([`super::Site::resolve_cross_refs`]) and the search index
/// ([`super::search::page_fragment`]) — a single-doc render cannot know a cross-page
/// number, so anything that reads a rendered page's text must run this first or read a
/// bare "Figure". The search index used to skip it, and its snippets contradicted the very
/// pages they linked to.
pub(super) fn resolve_blocks(
    blocks: &mut [Block],
    targets: &HashMap<String, XrefTarget>,
    current_url: &str,
) {
    if targets.is_empty() {
        return;
    }
    let up = "../".repeat(current_url.matches('/').count());
    for b in blocks.iter_mut() {
        if b.html.contains("data-qmd-xref=\"") {
            b.html = rewrite_cross_refs(&b.html, targets, current_url, &up);
        }
    }
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
    // N"; a subsection keeps cite's "Section" label.
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
        "<a href=\"{up}{}#{anchor}\" class=\"tali-xref\">{label}{number}</a>",
        target.url
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brace_id_reads_an_id_in_a_later_brace_block() {
        // split-brace heading: `## Setup {.unnumbered} {#sec-setup}` — the id lives in
        // a brace block that is not the first one on the line.
        assert_eq!(
            brace_id("## Setup {.unnumbered} {#sec-setup}").as_deref(),
            Some("sec-setup")
        );
    }

    #[test]
    fn brace_id_ignores_a_hash_inside_a_quoted_attribute_value() {
        // a `#word` inside a quoted `title=` is prose, not an id — it must not invent a target.
        assert_eq!(
            brace_id(r#"::: {.theorem title="see #thm-ghost below"}"#),
            None
        );
    }

    #[test]
    fn brace_id_finds_the_real_id_after_a_quoted_value_containing_a_hash() {
        assert_eq!(
            brace_id(r#"::: {.theorem title="bound #thm-fake" #thm-real}"#).as_deref(),
            Some("thm-real")
        );
    }

    #[test]
    fn brace_id_still_handles_id_first_and_class_first_single_blocks() {
        assert_eq!(
            brace_id("## Setup {#sec-x .unnumbered}").as_deref(),
            Some("sec-x")
        );
        assert_eq!(brace_id("::: {.theorem #thm-x}").as_deref(), Some("thm-x"));
    }

    #[test]
    fn xref_markers_in_finds_every_cross_page_marker() {
        // Two cross-page markers on the block, plus a same-page `tali-xref` link with
        // no marker (must be ignored) and an ordinary link (ignored).
        let html = concat!(
            r##"See <a href="#fig-a" class="tali-xref" data-qmd-xref="fig-a">Figure</a> and "##,
            r##"<a href="#thm-b" class="tali-xref" data-qmd-xref="thm-b">Theorem</a>; "##,
            r##"local <a href="#sec-here" class="tali-xref">Section&nbsp;1</a> and "##,
            r##"<a href="https://x">out</a>."##,
        );
        assert_eq!(xref_markers_in(html), vec!["fig-a", "thm-b"]);
    }

    #[test]
    fn xref_markers_in_is_empty_without_markers() {
        assert_eq!(
            xref_markers_in(r##"<a href="#sec-x" class="tali-xref">Section</a>"##),
            Vec::<&str>::new()
        );
    }

    /// The bare-prefix list in `is_ref_anchor` must recognize every cross-reference prefix
    /// that `cite::XREF_LABELS` defines, so the two parallel lists cannot drift apart.
    #[test]
    fn xref_anchor_recognizes_every_cite_prefix() {
        for (prefix, _) in crate::cite::XREF_LABELS {
            assert!(
                super::is_ref_anchor(&format!("{prefix}-x")),
                "cite prefix `{prefix}` is not a recognized ref anchor"
            );
        }
    }
}
