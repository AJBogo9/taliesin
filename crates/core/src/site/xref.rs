//! Project-wide cross-reference registry: scan each page's source for `{#sec-}`/
//! `{#fig-}`/… anchors (+ a section number for numbered book sections) and rewrite
//! `data-tali-xref`-marked links to the right page. `use super::*` reaches Page,
//! Book, section_number, Block.

use super::*;
use crate::render::parse_attrs;
use std::collections::BTreeSet;

/// Where a cross-referenceable anchor (`sec-x`, `fig-x`, …) lives in the project:
/// its page url and, for a numbered section, its number ("2.1"; empty otherwise).
/// `PartialEq` so the dev server can ask whether a refresh actually MOVED anything —
/// a cross-page ref is a dependency the file-level walk cannot see, so "did a target
/// move" is what tells it which open pages to re-render. `title` is part of that
/// equality *on purpose*: it is rendered into every referring page, so editing a
/// heading's text has to re-render the pages that name it, exactly as moving it does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XrefTarget {
    pub url: String,
    pub number: String,
    /// The target heading's own text, for an anchor that sits on a heading line;
    /// empty otherwise (a figure/equation anchor, or a cell label harvested from a
    /// render). Carried so an unnumbered cross-page `@sec-` can name what it points
    /// at instead of rendering the bare word "Section" — see [`rewrite_one_xref`].
    pub title: String,
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
        for ScannedAnchor {
            id,
            number,
            title,
            line,
        } in scan_page_anchors(&src, chapter)
        {
            match map.entry(id) {
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
                        title,
                    });
                }
            }
        }
    }
    map
}
/// Every cross-reference anchor defined by the *other* `.tmd` files of the site
/// project that contains `page` — empty when `page` has no ancestor `_site.yml`, i.e.
/// when it really is a standalone document.
///
/// This exists because per-document validation is a **scope mismatch**, not a bug: a
/// page inside a site legitimately refers across pages, so `check <dir>` and the built
/// page resolve every such reference while a per-document check (the editor's language
/// server, and `check <file.tmd>`) reported every one of them as a broken
/// cross-reference. An author who trusts that squiggle deletes a working reference; one
/// who learns to ignore it stops reading the diagnostics that matter.
///
/// Deliberately cheap — sources are read and scanned line by line, and **nothing is
/// rendered** — because the caller is the every-keystroke editor path where
/// [`Site::discover`](super::Site::discover) (three whole-project render passes) is out
/// of the question. That is also why a cell's `#| label:` is read directly rather than
/// harvested from a render the way [`super::Site::harvest_xref_numbers`] must: a
/// *number* needs the render, a *name* does not, and a name is all this answers.
///
/// The page's own anchors are excluded: an editor buffer is ahead of its file on disk,
/// and a reference the buffer resolves locally emits no marker to validate anyway.
pub fn anchors_defined_elsewhere_in_project(page: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(root) = enclosing_site_root(page) else {
        return out;
    };
    let own = page.canonicalize().ok();
    let mut inputs = Vec::new();
    super::collect_pages(&root, &mut inputs);
    for input in inputs {
        if own.is_some() && input.canonicalize().ok() == own {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&input) else {
            continue;
        };
        // Resolve includes, exactly as the scan above does: an anchor authored in an
        // `_includes/` partial belongs to whichever page includes it, and the walk
        // skips `_`-prefixed directories, so it is reachable only this way.
        let base = input.parent().unwrap_or_else(|| Path::new("."));
        let (src, _) = crate::includes::resolve(&raw, base);
        out.extend(scan_page_anchors(&src, None).into_iter().map(|a| a.id));
        out.extend(cell_label_anchors(&src));
    }
    out
}

/// The nearest ancestor directory of `page` holding a `_site.yml`, or `None`. Starts at
/// the file's own directory, so a page IS in the project its own `_site.yml` roots.
fn enclosing_site_root(page: &Path) -> Option<PathBuf> {
    let abs = page
        .canonicalize()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(page));
    let mut cur = abs.parent()?;
    loop {
        if cur.join("_site.yml").is_file() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
        if cur.as_os_str().is_empty() {
            return None;
        }
    }
}

/// The cross-reference anchors a page's code cells define through `#| label: fig-x`.
/// [`scan_page_anchors`] cannot see these — a cell option lives inside a fence, which
/// [`content_lines_numbered`] skips by design — so they are read here, from inside the
/// fences, using the renderer's own directive primitive.
fn cell_label_anchors(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_code = false;
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            continue;
        }
        let Some(opt) = crate::render::option_directive(line) else {
            continue;
        };
        if let Some((k, v)) = opt.split_once(':')
            && k.trim() == "label"
        {
            let id = v.trim().trim_matches(['"', '\'']);
            if is_ref_anchor(id) {
                out.push(id.to_string());
            }
        }
    }
    out
}

/// The ATX heading level of a content line (`## T` -> 2), or `None` if it is not a
/// heading. `#` runs must be followed by a space, so a `#hashtag` is not a heading.
fn heading_level_of(line: &str) -> Option<usize> {
    let level = line.bytes().take_while(|&b| b == b'#').count();
    ((1..=6).contains(&level) && line.as_bytes().get(level) == Some(&b' ')).then_some(level)
}

/// The display text of a heading line: its `#` run, its `{…}` attribute blocks and
/// its inline `` ` ``/`*` delimiters removed. Plain text, not HTML — the caller
/// escapes it, so a heading containing `<` or `&` cannot inject markup into the
/// referring page's link label.
///
/// Only the two delimiters that actually occur in the repo's anchored headings are
/// stripped. `_` is deliberately left alone: it is far likelier to be a `snake_case`
/// identifier than an emphasis marker in a heading, and mangling one is worse than
/// leaving the other.
fn heading_title(line: &str) -> String {
    let after_hashes = line.trim_start_matches('#').trim_start();
    let mut text = String::with_capacity(after_hashes.len());
    let mut depth = 0usize;
    for c in after_hashes.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '`' | '*' if depth == 0 => {}
            _ if depth == 0 => text.push(c),
            _ => {}
        }
    }
    text.trim().to_string()
}

/// Every heading level in a page's source, in document order — the input
/// [`ChapterNumbering`] derives its base from.
fn heading_levels(src: &str) -> Vec<usize> {
    content_lines_numbered(src)
        .filter_map(|(_, t)| heading_level_of(t))
        .collect()
}

/// One cross-referenceable anchor as the source scan sees it.
struct ScannedAnchor {
    id: String,
    /// Section number for a `{#sec-}` heading in a numbered chapter; empty otherwise.
    number: String,
    /// The heading's own text when the anchor sits on a heading line; empty otherwise.
    title: String,
    /// 1-based source line, for the duplicate-label warning.
    line: usize,
}

/// The `{#prefix-id}` cross-ref anchors in one page's source, paired with a section
/// number for `{#sec-}` headings in a numbered chapter (empty otherwise). Headings
/// are counted in order so an unlabeled section still advances the numbering.
fn scan_page_anchors(src: &str, chapter: Option<u32>) -> Vec<ScannedAnchor> {
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
                out.push(ScannedAnchor {
                    id,
                    number,
                    title: heading_title(t),
                    line,
                });
            }
        } else if let Some(id) = brace_id(t).filter(|id| is_ref_anchor(id)) {
            // a figure/equation anchor: link, no number, no heading to name it by
            out.push(ScannedAnchor {
                id,
                number: String::new(),
                title: String::new(),
                line,
            });
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
/// `data-tali-xref="…"` marker, in document order (with duplicates, if a block refers
/// to the same anchor twice). `cite` emits this marker *only* for a reference whose
/// target is not on the current page (`cite/render.rs`), so a marker is by
/// construction a cross-page reference — the raw material for the reverse
/// (anchor → referring pages) index.
pub(super) fn xref_markers_in(html: &str) -> Vec<&str> {
    let marker = "data-tali-xref=\"";
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
        if b.html.contains("data-tali-xref=\"") {
            b.html = rewrite_cross_refs(&b.html, targets, current_url, &up);
        }
    }
}

/// Rewrite the `data-tali-xref`-marked links in one block's HTML: a marker whose
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
    let marker = "data-tali-xref=\"";
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
    // What qualifies the kind word. A number when the project has one — but a website
    // has no section numbering, so a cross-page `@sec-` there had nothing to add and
    // rendered as the bare word "Section", a sentence-breaking dead end ("…as set out
    // in Section."). Name the target instead: the heading's own text says which section
    // it is, which is what the number would have said. Numbers still win where they
    // exist, so a book is untouched.
    let qualifier = if !target.number.is_empty() {
        format!("&nbsp;{}", target.number)
    } else if !target.title.is_empty() {
        format!("&nbsp;\u{201c}{}\u{201d}", esc(&target.title))
    } else {
        String::new()
    };
    format!(
        "<a href=\"{up}{}#{anchor}\" class=\"tali-xref\">{label}{qualifier}</a>",
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

    /// A website has no section numbering, so a cross-page `@sec-` has no number to
    /// carry. It must name its target rather than render the bare kind word.
    #[test]
    fn an_unnumbered_cross_page_sec_is_labelled_with_its_heading_title() {
        let targets = HashMap::from([(
            "sec-model".to_string(),
            XrefTarget {
                url: "index.html".to_string(),
                number: String::new(),
                title: "Is the canary still slower?".to_string(),
            },
        )]);
        let link =
            r##"<a href="#sec-model" class="tali-xref" data-tali-xref="sec-model">Section</a>"##;
        assert_eq!(
            rewrite_one_xref(link, &targets, "methods.html", ""),
            "<a href=\"index.html#sec-model\" class=\"tali-xref\">Section&nbsp;\u{201c}Is the \
             canary still slower?\u{201d}</a>"
        );
    }

    /// A number, where the project has one, still wins: a book is untouched by the
    /// title fallback above.
    #[test]
    fn a_numbered_cross_page_sec_still_renders_its_number_not_its_title() {
        let targets = HashMap::from([(
            "sec-setup".to_string(),
            XrefTarget {
                url: "methods.html".to_string(),
                number: "2.1".to_string(),
                title: "Setting up".to_string(),
            },
        )]);
        let link =
            r##"<a href="#sec-setup" class="tali-xref" data-tali-xref="sec-setup">Section</a>"##;
        assert_eq!(
            rewrite_one_xref(link, &targets, "intro.html", ""),
            "<a href=\"methods.html#sec-setup\" class=\"tali-xref\">Section&nbsp;2.1</a>"
        );
    }

    /// The title is plain text from the source line, so it is escaped on the way into
    /// the referring page — a heading may legitimately contain `&` or `<`.
    #[test]
    fn a_heading_title_is_escaped_into_the_referring_page() {
        let targets = HashMap::from([(
            "sec-ab".to_string(),
            XrefTarget {
                url: "a.html".to_string(),
                number: String::new(),
                title: "Tom & Jerry <live>".to_string(),
            },
        )]);
        let link = r##"<a href="#sec-ab" class="tali-xref" data-tali-xref="sec-ab">Section</a>"##;
        let out = rewrite_one_xref(link, &targets, "b.html", "");
        assert!(
            out.contains("Tom &amp; Jerry &lt;live&gt;") && !out.contains("<live>"),
            "the heading text must be escaped: {out}"
        );
    }

    #[test]
    fn heading_title_drops_the_hashes_attributes_and_inline_delimiters() {
        assert_eq!(
            heading_title("## Is the canary still slower? {#sec-model}"),
            "Is the canary still slower?"
        );
        // The one anchored heading in the repo with inline code: the delimiters go, the
        // identifier stays.
        assert_eq!(
            heading_title("### How `draft:` filtering works {#sec-draft-filtering}"),
            "How draft: filtering works"
        );
        // A split-brace heading drops BOTH blocks, not only the last.
        assert_eq!(
            heading_title("## Setup {.unnumbered} {#sec-setup}"),
            "Setup"
        );
        // `_` survives: a heading is likelier to hold a snake_case identifier than an
        // emphasis pair.
        assert_eq!(
            heading_title("## The p95_ms column {#sec-p95}"),
            "The p95_ms column"
        );
    }

    #[test]
    fn xref_markers_in_finds_every_cross_page_marker() {
        // Two cross-page markers on the block, plus a same-page `tali-xref` link with
        // no marker (must be ignored) and an ordinary link (ignored).
        let html = concat!(
            r##"See <a href="#fig-a" class="tali-xref" data-tali-xref="fig-a">Figure</a> and "##,
            r##"<a href="#thm-b" class="tali-xref" data-tali-xref="thm-b">Theorem</a>; "##,
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
