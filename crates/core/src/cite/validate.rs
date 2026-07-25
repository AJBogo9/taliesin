//! Static cross-reference validation: flag `data-tali-xref` markers left unresolved.

use super::sourcepos_start_line;
use crate::render::{Block, Warning};
use std::collections::BTreeSet;

/// Scan rendered blocks for cross-references left unresolved — the `data-tali-xref`
/// markers `cite` emits when an `@fig-`/`@sec-`/… anchor isn't in the local (and,
/// for a site, cross-page) registry. One warning per distinct broken anchor. Run
/// AFTER any site-wide cross-ref resolution so genuine cross-page refs aren't flagged.
///
/// A broken anchor that is a near-miss of one the page defines carries a did-you-mean.
/// Renaming a label is the commonest way an author silently breaks their own document.
pub fn validate_xrefs(blocks: &[Block]) -> Vec<Warning> {
    let marker = "data-tali-xref=\"";
    let anchors = local_anchors(blocks);
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
            let w = Warning::new(match suggest(&a, &anchors) {
                Some(near) => format!("broken cross-reference: @{a} (did you mean `@{near}`?)"),
                None => format!("broken cross-reference: @{a} (no such figure/section/\u{2026})"),
            });
            match line {
                Some(l) => w.at(file, l),
                None => w,
            }
        })
        .collect()
}

/// Every cross-reference anchor the page itself defines, read back off the rendered
/// HTML. The anchor registry lives in `render` and is gone by the time this runs, but
/// each target still carries its `id="fig-x"`, so the vocabulary is recoverable.
///
/// The leading space is load-bearing: `data-block-id="…"` ends in `id="`, and an
/// unanchored search would harvest every block's content hash as an anchor.
fn local_anchors(blocks: &[Block]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for b in blocks {
        let mut rest = b.html.as_str();
        while let Some(i) = rest.find(" id=\"") {
            rest = &rest[i + 5..];
            let Some(end) = rest.find('"') else { break };
            if super::is_xref_anchor(&rest[..end]) {
                out.insert(rest[..end].to_string());
            }
            rest = &rest[end..];
        }
    }
    out
}

/// The nearest same-kind anchor to a broken one, e.g. `fig-reslts` -> `fig-results`.
///
/// Matching is on the *stem* (the part after the kind prefix), for two reasons: a
/// broken `@fig-x` must never suggest a `@sec-y` (the label would read "Figure" and
/// link to a section), and the shared prefix would otherwise pad every name past the
/// short-name guard, so `fig-a` would suggest `fig-b`.
fn suggest(broken: &str, anchors: &BTreeSet<String>) -> Option<String> {
    let (kind, stem) = broken.split_once('-')?;
    let siblings = anchors
        .iter()
        .filter_map(|a| a.split_once('-'))
        .filter(|(k, _)| *k == kind)
        .map(|(_, s)| s);
    super::nearest(stem, siblings).map(|near| format!("{kind}-{near}"))
}
