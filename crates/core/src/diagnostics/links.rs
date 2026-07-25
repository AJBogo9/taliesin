//! Local relative cross-file link existence validation.

use super::helpers::{is_local_ref, start_line, tag_attr};
use crate::render::{Block, Warning};
use std::path::Path;

/// Unique local link targets from MANUAL `<a href>` tags only, paired with their tag
/// span so the caller can locate each. Cross-reference links (`tali-xref`) are skipped
/// (validated by `validate_xrefs`); bare in-page `#fragment` links are skipped (validated
/// by [`super::anchors::validate_internal_anchors`]). Returns each `href` value verbatim
/// (path + optional `#frag`), so a caller can split the path from the fragment.
fn local_link_refs(html: &str) -> Vec<&str> {
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
            continue; // a cross-reference, validated separately
        }
        let Some(val) = tag_attr(tag, "href=\"") else {
            continue;
        };
        // A bare in-page anchor (`#frag`) is `validate_internal_anchors`'s job.
        if val.starts_with('#') {
            continue;
        }
        if is_local_ref(val) && !out.contains(&val) {
            out.push(val);
        }
    }
    out
}

/// Whether `path` (relative to a doc at `base`) is backed by a file on disk, accepting the
/// forms a site build produces: the literal file, a `.html` link whose source (any
/// extension in [`crate::ext::ACCEPTED_SOURCE_EXTS`]) exists, and a directory link (`x/` or
/// `x`) whose `x/index.<ext>`/`.html` exists. So a single-doc check doesn't false-flag an
/// intra-project `.html` link whose source page is present (the site build will emit the
/// `.html`).
fn link_target_exists(base: &Path, path: &str) -> bool {
    let join = |p: &str| base.join(p).is_file();
    if join(path) {
        return true;
    }
    // `x.html` → its source (the built page is produced from it), tried in every
    // accepted spelling (`x.tmd`).
    if let Some(stem) = path.strip_suffix(".html")
        && crate::ext::ACCEPTED_SOURCE_EXTS
            .iter()
            .any(|ext| join(&format!("{stem}.{ext}")))
    {
        return true;
    }
    // A directory link (`dir/` or `dir`) → that dir's index page (source or `.html`).
    let dir = path.trim_end_matches('/');
    crate::ext::ACCEPTED_SOURCE_EXTS
        .iter()
        .any(|ext| base.join(format!("{dir}/index.{ext}")).is_file())
        || base.join(format!("{dir}/index.html")).is_file()
}

/// Manual relative links (`[text](other.tmd)`, `[text](sub/page.html#x)`) whose local
/// **target file** does not exist under the doc base dir — a broken cross-file jump that
/// ships silently. External (`http(s)://`, `mailto:`, …) and absolute (`/…`) links are
/// out of scope (external links are never fetched — `check` stays offline + deterministic);
/// bare `#anchor` links and the in-page fragment are handled by
/// [`super::anchors::validate_internal_anchors`]. Cross-page `#fragment` resolution is a
/// site-registry job (the server's site path resolves anchors). A `.html` link whose source
/// exists on disk (any accepted extension) is accepted (see [`link_target_exists`]) so an
/// intra-project link to a yet-to-be-built page is not false-flagged — only a target with no
/// file *and* no source is broken.
pub fn validate_local_links(blocks: &[Block], base: &Path) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for val in local_link_refs(&b.html) {
            let path = &val[..val.find(['?', '#']).unwrap_or(val.len())];
            if path.is_empty() || path.starts_with('/') || link_target_exists(base, path) {
                continue;
            }
            // Nothing on disk backs it — but a document inside a site may legitimately link
            // a project that site MOUNTS, which resolves by URL prefix and so has no file
            // under this document's directory. Asked only here, on a link already about to
            // be reported, so the common path still costs nothing.
            if crate::site::link_targets_enclosing_mount(base, path) {
                continue;
            }
            let w = Warning::new(format!(
                "broken link: `{path}` (no such file under the document directory)"
            ));
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}
