//! In-page anchor-link validation (`[text](#anchor)` targets that match no element id).

use super::helpers::{collect_attr_values, start_line};
use crate::render::{Block, Severity, Warning};

/// Same-page `href="#fragment"` values (without `#`) from MANUAL `<a>` links only.
/// `@fig-`/`@sec-`/`@tbl-` cross-references (anchors carrying `tali-xref`) are skipped:
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
        if tag.contains("tali-xref") {
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
    if blocks.iter().any(|b| b.cells().next().is_some()) {
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
            ))
            .severity(Severity::Error);
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}
