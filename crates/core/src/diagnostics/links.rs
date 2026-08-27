//! Local relative cross-file link existence validation.

use super::helpers::{is_local_ref, start_line};
use crate::render::{Block, Severity, Warning};
use std::path::Path;

/// Unique local link targets from MANUAL `<a href>` tags only, paired with their tag
/// span so the caller can locate each. Cross-reference links (`tali-xref`) are skipped
/// (validated by `validate_xrefs`); bare in-page `#fragment` links are skipped (validated
/// by [`super::anchors::validate_internal_anchors`]). Returns each `href` value verbatim
/// (path + optional `#frag`), so a caller can split the path from the fragment.
fn local_link_refs(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for tag in crate::render::tags(html) {
        if !tag.name.eq_ignore_ascii_case("a") || tag.text.contains("tali-xref") {
            continue; // not a link, or a cross-reference validated separately
        }
        let Some(val) = crate::render::attr_value(&tag, "href") else {
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
            // A migrated document's links keep the extension the old tool used, and the
            // answer is usually sitting right next to the link (item 128). Suggest, never
            // rewrite: a `.md` link may point at a real shipped `.md`.
            let hint = crate::ext::migrated_source_candidates(path)
                .into_iter()
                .find(|c| base.join(c).is_file())
                .map(|c| format!(" (did you mean `{c}`?)"))
                .unwrap_or_default();
            let w = Warning::new(format!(
                "broken link: `{path}` (no such file under the document directory){hint}"
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
