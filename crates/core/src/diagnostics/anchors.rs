//! In-page anchor-link validation (`[text](#anchor)` targets that match no element id).

use super::helpers::collect_attr_values;
use crate::render::sourcepos_start_line as start_line;
use crate::render::{Block, Severity, Warning};

/// Same-page `href="#fragment"` values (without `#`) from MANUAL `<a>` links only.
/// `@fig-`/`@sec-`/`@tbl-` cross-references (anchors carrying `tali-xref`) are skipped:
/// they are validated by `validate_xrefs`, resolved cross-page by the site layer, and may
/// target an id emitted only by code-cell execution (which static `check` does not run).
/// Cross-page `href="page.html#x"` and empty `href="#"` are also skipped.
fn same_page_manual_fragments(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tag in crate::render::tags(html) {
        if !tag.name.eq_ignore_ascii_case("a") || tag.text.contains("tali-xref") {
            continue; // not a link, or a cross-reference validated separately
        }
        if let Some(href) = crate::render::attr_value(&tag, "href")
            && let Some(frag) = href.strip_prefix('#')
            && !frag.is_empty()
        {
            out.push(frag.to_string());
        }
    }
    out
}

/// In-page anchor links (`[text](#anchor)`) whose `#fragment` matches no element id on
/// the page — a broken jump that silently lands nowhere (or scrolls to the top). The
/// valid-target set is every `id="..."` the page emits, so it never false-flags a real
/// anchor. (`@fig-`/`@sec-` cross-references are covered separately by `validate_xrefs`.)
pub fn validate_internal_anchors(blocks: &[Block]) -> Vec<Warning> {
    // Static check never executes cells; a {python} or {js} cell can emit the target id at
    // runtime (e.g. `HTML('<div id="x">')`). Conservatively skip the manual-anchor check for
    // any doc with executable cells, so a green check stays a no-false-positive promise.
    if blocks.iter().any(|b| b.cells().next().is_some()) {
        return Vec::new();
    }
    let mut ids = std::collections::HashSet::new();
    for b in blocks {
        collect_attr_values(&b.html, "id", &mut ids);
    }
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for frag in same_page_manual_fragments(&b.html) {
            // Matched the way the browser matches a fragment: as written, then
            // percent-decoded, so `#%C3%BCber` finds `id="über"`.
            if ids.contains(frag.as_str())
                || ids.contains(crate::render::percent_decode(&frag).as_str())
            {
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
