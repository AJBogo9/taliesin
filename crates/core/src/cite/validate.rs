//! Static cross-reference validation: flag `data-qmd-xref` markers left unresolved.

use super::sourcepos_start_line;
use crate::render::{Block, Warning};

/// Scan rendered blocks for cross-references left unresolved — the `data-qmd-xref`
/// markers `cite` emits when an `@fig-`/`@sec-`/… anchor isn't in the local (and,
/// for a site, cross-page) registry. One warning per distinct broken anchor. Run
/// AFTER any site-wide cross-ref resolution so genuine cross-page refs aren't flagged.
pub fn validate_xrefs(blocks: &[Block]) -> Vec<Warning> {
    let marker = "data-qmd-xref=\"";
    // First occurrence wins for the reported location; dedup by anchor.
    let mut seen: std::collections::BTreeMap<String, (Option<String>, Option<u32>)> =
        std::collections::BTreeMap::new();
    for b in blocks {
        let loc = (b.source_file.clone(), sourcepos_start_line(&b.sourcepos));
        let mut rest = b.html.as_str();
        while let Some(i) = rest.find(marker) {
            rest = &rest[i + marker.len()..];
            let Some(end) = rest.find('"') else { break };
            let anchor = rest[..end].to_string();
            seen.entry(anchor).or_insert_with(|| loc.clone());
            rest = &rest[end..];
        }
    }
    seen.into_iter()
        .map(|(a, (file, line))| {
            let w = Warning::new(format!(
                "broken cross-reference: @{a} (no such figure/section/\u{2026})"
            ));
            match line {
                Some(l) => w.at(file, l),
                None => w,
            }
        })
        .collect()
}
