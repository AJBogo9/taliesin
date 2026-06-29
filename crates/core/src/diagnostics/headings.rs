//! Duplicate heading-id detection.

use super::helpers::start_line;
use crate::render::{Block, Warning};

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
