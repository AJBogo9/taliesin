//! Static cross-reference validation: flag `data-tali-xref` markers left unresolved.

use super::{sourcepos_end_line, sourcepos_start_line, token_span};
use crate::render::{Block, Severity, Warning};
use std::collections::BTreeSet;

/// Scan rendered blocks for cross-references left unresolved — the `data-tali-xref`
/// markers `cite` emits when an `@fig-`/`@sec-`/… anchor isn't in the local (and,
/// for a site, cross-page) registry. One warning per distinct broken anchor. Run
/// AFTER any site-wide cross-ref resolution so genuine cross-page refs aren't flagged.
///
/// A broken anchor that is a near-miss of one the page defines carries a did-you-mean.
/// Renaming a label is the commonest way an author silently breaks their own document.
pub fn validate_xrefs(blocks: &[Block], src: Option<&str>) -> Vec<Warning> {
    validate_xrefs_known_elsewhere(blocks, &BTreeSet::new(), src)
}

/// [`validate_xrefs`], plus a set of anchors known to be defined **on other pages of
/// the same project**, which are therefore not broken and must not be reported.
///
/// This is the per-document/whole-project scope seam. A single-document check has no
/// page registry, so every legitimate cross-page reference looked broken to it: the
/// editor drew red squiggles on correct content that `check <dir>` and the built page
/// both resolved. The caller supplies what it can see of the project (see
/// [`crate::site::anchors_defined_elsewhere_in_project`]); an anchor that exists
/// nowhere in it is still an error, so a genuinely broken reference is still caught in
/// the editor, which is the only place worth catching it.
pub fn validate_xrefs_known_elsewhere(
    blocks: &[Block],
    known_elsewhere: &BTreeSet<String>,
    src: Option<&str>,
) -> Vec<Warning> {
    let marker = "data-tali-xref=\"";
    let anchors = local_anchors(blocks);
    // First occurrence wins for the reported location; dedup by anchor.
    type Loc = (Option<String>, Option<u32>, Option<u32>);
    let mut seen: std::collections::BTreeMap<String, Loc> = std::collections::BTreeMap::new();
    for b in blocks {
        let loc = (
            b.source_file.clone(),
            sourcepos_start_line(&b.sourcepos),
            sourcepos_end_line(&b.sourcepos),
        );
        let mut rest = b.html.as_str();
        while let Some(i) = rest.find(marker) {
            rest = &rest[i + marker.len()..];
            let Some(end) = rest.find('"') else { break };
            let anchor = rest[..end].to_string();
            if !known_elsewhere.contains(&anchor) {
                seen.entry(anchor).or_insert_with(|| loc.clone());
            }
            rest = &rest[end..];
        }
    }
    seen.into_iter()
        .map(|(a, (file, line, end))| {
            let w = Warning::new(match suggest(&a, &anchors) {
                Some(near) => format!("broken cross-reference: @{a} (did you mean `@{near}`?)"),
                None => format!("broken cross-reference: @{a} (no such figure/section/\u{2026})"),
            })
            .severity(Severity::Error);
            let Some(l) = line else { return w };
            // The exact `@anchor`, not the whole line. Only for the primary document: a
            // block from an included file numbers its lines in a file `src` is not.
            match (file.is_none(), src) {
                (true, Some(s)) => {
                    // An anchor's own character set (`parse_xref`), not the cite key's.
                    let anchor_char = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
                    match token_span(s, l, end.unwrap_or(l), &format!("@{a}"), anchor_char) {
                        Some((tl, col, end_col)) => w.at(None, tl).span(col, end_col),
                        None => w.at(None, l),
                    }
                }
                _ => w.at(file, l),
            }
        })
        .collect()
}

/// Every cross-reference anchor the page itself defines, read back off the rendered
/// HTML. The anchor registry lives in `render` and is gone by the time this runs, but
/// each target still carries its `id="fig-x"`, so the vocabulary is recoverable.
///
/// Read through [`crate::render::attr_values`], so `id` is matched as an attribute NAME
/// (`data-block-id="…"` is a different one, which the leading-space needle here used to
/// approximate) and an author's single-quoted `<div id='fig-x'>` counts as the anchor it is.
fn local_anchors(blocks: &[Block]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for b in blocks {
        out.extend(
            crate::render::attr_values(&b.html, "id")
                .filter(|v| super::is_xref_anchor(v))
                .map(str::to_string),
        );
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
