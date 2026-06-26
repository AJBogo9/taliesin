//! Static document-lint validators for `qmd-fast check` (the "check-superset").
//!
//! Each takes the rendered block model (and, where needed, the doc base dir) and
//! returns located [`Warning`]s on the same click-to-source channel as the other
//! diagnostics, so `check` becomes a true preflight superset of build/preview and a
//! green `check` means the document is publishable. Read-only static analysis only.

use crate::render::{Block, Warning};
use std::path::Path;

/// 1-based start line from a block's `sourcepos` (`"startLine:col-..."`), if positive.
fn start_line(sourcepos: &str) -> Option<u32> {
    sourcepos
        .split(':')
        .next()?
        .parse::<u32>()
        .ok()
        .filter(|&l| l > 0)
}

/// The `id="..."` attribute of a heading block (`<h1>`..`<h6>`), or None for a
/// non-heading block or a heading with no id. Reads only the opening tag and matches
/// the ` id="` attribute specifically (so `data-block-id="..."` does not false-match).
fn heading_id(html: &str) -> Option<&str> {
    let level_ok = html.as_bytes().get(2).is_some_and(|c| c.is_ascii_digit());
    if !(html.starts_with("<h") && level_ok) {
        return None;
    }
    let tag_end = html.find('>')?;
    let head = &html[..tag_end];
    let i = head.find(" id=\"")? + 5;
    let rest = &head[i..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Two headings that emit the same `id` (e.g. a repeated explicit `{#id}`) produce an
/// invalid duplicate DOM id, so anchors, the TOC, and cross-references silently jump to
/// the first. Auto-slugged ids are already deduped, so a duplicate here is an explicit-id
/// collision the renderer does not catch.
pub fn validate_duplicate_heading_ids(blocks: &[Block]) -> Vec<Warning> {
    use std::collections::HashSet;
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for b in blocks {
        let Some(id) = heading_id(&b.html) else {
            continue;
        };
        if !seen.insert(id) {
            let w = Warning::new(format!(
                "duplicate heading id `{id}`: an earlier heading already uses it, so anchors, the TOC, and cross-references jump to the first"
            ));
            out.push(match start_line(&b.sourcepos) {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}

/// Every value of attribute `attr` (e.g. `"id=\""`) found in `html`, appended to `out`.
fn collect_attr_values<'a>(
    html: &'a str,
    attr: &str,
    out: &mut std::collections::HashSet<&'a str>,
) {
    let mut i = 0;
    while let Some(pos) = html[i..].find(attr) {
        let start = i + pos + attr.len();
        let Some(len) = html[start..].find('"') else {
            break;
        };
        out.insert(&html[start..start + len]);
        i = start + len;
    }
}

/// Same-page `href="#fragment"` values (without `#`) from MANUAL `<a>` links only.
/// `@fig-`/`@sec-`/`@tbl-` cross-references (anchors carrying `qmd-xref`) are skipped:
/// they are validated by `validate_xrefs`, resolved cross-page by the site layer, and may
/// target an id emitted only by code-cell execution (which static `check` does not run).
/// Cross-page `href="page.html#x"` and empty `href="#"` are also skipped.
fn same_page_manual_fragments(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find("<a ") {
        let tag_start = i + pos;
        let Some(rel_end) = html[tag_start..].find('>') else {
            break;
        };
        let tag = &html[tag_start..tag_start + rel_end];
        i = tag_start + rel_end + 1;
        if tag.contains("qmd-xref") {
            continue; // a cross-reference, not a manual in-page link
        }
        let Some(hpos) = tag.find("href=\"") else {
            continue;
        };
        let vstart = hpos + "href=\"".len();
        let Some(vlen) = tag[vstart..].find('"') else {
            continue;
        };
        if let Some(frag) = tag[vstart..vstart + vlen].strip_prefix('#')
            && !frag.is_empty()
        {
            out.push(frag);
        }
    }
    out
}

/// In-page anchor links (`[text](#anchor)`) whose `#fragment` matches no element id on
/// the page — a broken jump that silently lands nowhere (or scrolls to the top). The
/// valid-target set is every `id="..."` the page emits, so it never false-flags a real
/// anchor. (`@fig-`/`@sec-` cross-references are covered separately by `validate_xrefs`.)
pub fn validate_internal_anchors(blocks: &[Block]) -> Vec<Warning> {
    // Static check never executes cells; a {python}/{r}/{js} cell can emit the target id at
    // runtime (e.g. `HTML('<div id="x">')`). Conservatively skip the manual-anchor check for
    // any doc with executable cells, so a green check stays a no-false-positive promise.
    if blocks.iter().any(|b| b.cell.is_some()) {
        return Vec::new();
    }
    let mut ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for b in blocks {
        collect_attr_values(&b.html, "id=\"", &mut ids);
    }
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for frag in same_page_manual_fragments(&b.html) {
            if ids.contains(frag) {
                continue;
            }
            let w = Warning::new(format!(
                "broken in-page link: #{frag} (no element with that id on this page)"
            ));
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}

/// Whether `v` is a local file reference, i.e. not external, an in-page anchor, a data
/// URI, or a non-file scheme. (Mirrors the asset-bundling heuristic in the build path.)
fn is_local_ref(v: &str) -> bool {
    !v.is_empty()
        && !v.starts_with('#')
        && !v.starts_with("//")
        && !v.contains("://")
        && !v.starts_with("data:")
        && !v.starts_with("mailto:")
        && !v.starts_with("tel:")
        && !v.starts_with("vscode:")
        && !v.starts_with("javascript:")
}

/// Unique local `src="..."` values from `<img>` tags only. Restricted to images on
/// purpose: `<audio>`/`<video>`/`<source>` refs are frequently generated by code
/// execution or are streamed/unvendored heavy media, which a static (no-execution) check
/// cannot resolve — checking them would false-flag. Links (`href=`) are out of scope.
fn local_img_refs(html: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find("<img ") {
        let tag_start = i + pos;
        let Some(rel_end) = html[tag_start..].find('>') else {
            break;
        };
        let tag = &html[tag_start..tag_start + rel_end];
        i = tag_start + rel_end + 1;
        let Some(spos) = tag.find("src=\"") else {
            continue;
        };
        let vstart = spos + "src=\"".len();
        let Some(vlen) = tag[vstart..].find('"') else {
            continue;
        };
        let val = &tag[vstart..vstart + vlen];
        if is_local_ref(val) && !out.contains(&val) {
            out.push(val);
        }
    }
    out
}

/// Citations are present (`cite::process` appended the `qmd-references` section) but the
/// front matter declares no `bibliography:`, so every reference renders as a raw key with
/// no diagnostic today. (A declared-but-missing bibliography file is a separate warning.)
pub fn citations_without_bibliography(src: &str, blocks: &[Block]) -> Vec<Warning> {
    let has_citations = blocks.iter().any(|b| b.id == "qmd-references");
    if !has_citations {
        return Vec::new();
    }
    let declares_bib = crate::frontmatter::front_matter_block(src)
        .and_then(|fm| serde_yaml::from_str::<serde_yaml::Value>(fm).ok())
        .and_then(|v| v.as_mapping().map(|m| m.get("bibliography").is_some()))
        .unwrap_or(false);
    if declares_bib {
        return Vec::new();
    }
    vec![Warning::new(
        "citations are present but no `bibliography:` is declared, so every reference renders as a raw key",
    )]
}

/// Local `<img src>` references (`![](img.png)`, raw `<img>`) whose target file does not
/// exist under the doc base dir — a broken image that ships silently today. Absolute
/// (`/...`) and external refs are out of scope; audio/video are skipped (see
/// [`local_img_refs`]: a static check cannot resolve generated/streamed media).
pub fn validate_local_assets(blocks: &[Block], base: &Path) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for val in local_img_refs(&b.html) {
            let path = &val[..val.find(['?', '#']).unwrap_or(val.len())];
            if path.is_empty() || path.starts_with('/') || base.join(path).is_file() {
                continue;
            }
            let w = Warning::new(format!(
                "local asset not found: `{path}` (no such file under the document directory)"
            ));
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}
