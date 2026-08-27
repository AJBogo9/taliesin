//! Duplicate heading-id detection.

use super::helpers::start_line;
use crate::render::{Block, Severity, Warning};

/// The `id` attribute of a heading block (`<h1>`..`<h6>`), or None for a non-heading block
/// or a heading with no id. Reads the block's leading tag only, through the one walker, so
/// `data-block-id="…"` is not an `id` (it is a different attribute NAME, which is what the
/// leading-space needle here used to approximate) and a `>` inside an attribute value does
/// not end the tag early.
fn heading_id(html: &str) -> Option<&str> {
    super::helpers::heading_level(html)?;
    crate::render::attr_value(&crate::render::tags(html).next()?, "id")
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
            ))
            .severity(Severity::Error);
            out.push(match start_line(&b.sourcepos) {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}
