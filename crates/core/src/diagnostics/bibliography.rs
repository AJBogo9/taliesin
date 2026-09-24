//! Bibliography-vs-citation mismatches: citations with no `bibliography:` declared.
//!
//! The inverse blind spot, a bare `@key` that never became a citation, is found by the
//! citation walk itself (`cite::render`), which already knows what prose is. It lived here
//! as a substring scan of the finished HTML until 2026-09-24, and read a key in an image's
//! `alt`, in inline code or in a comment as a bare one (audit G4).

use crate::render::{Block, Warning};
use std::path::Path;

/// Citations are present (`cite::process` appended the `tali-references` section), neither
/// the page nor its project declares a `bibliography:`, and **not one reference resolved**
/// — so every reference renders as a raw key with no diagnostic of its own.
///
/// Two conditions, and each rules out a different false positive:
///
/// - *Every row raw*, rather than "the front matter declares nothing". A page can now
///   inherit a project-wide `bibliography:` from `_site.yml`, declaring nothing of its own
///   while resolving every citation correctly; the old shape-only check called that "no
///   bibliography is declared" (regression caught by `corpus/shared-bib/index.tmd` in the
///   check-superset false-positive walk). Observing the outcome cannot be fooled by where
///   the bibliography came from, and it closes the mirror blind spot too — a declared `.bib`
///   that exists but is empty resolves nothing and used to pass silently.
/// - *Nothing is declared*, so one mistake does not draw two diagnostics: a declared file
///   that cannot be read is already `bibliography file not found`, which is the actionable
///   message. (A declared file missing only *some* keys is `broken citation` per key, and
///   does not reach here at all, since those rows resolve.) That includes the project's
///   `_site.yml`: a page inheriting it whose every citation is broken (one typo in a
///   one-citation post) already has its `broken citation`, and was also told to declare a
///   file its project declares (audit 2026-09-24, bibtex #8). A project declaration that
///   yields no entry at all is said as such instead: a page checked on its own hears about
///   the project's file nowhere else.
///
/// The raw-key marker is `cite::process`'s unresolved branch, the only place a reference row
/// contains a `<code>` element.
///
/// `base` is the page's directory, from which its project's `_site.yml` is found.
pub fn citations_without_bibliography(src: &str, blocks: &[Block], base: &Path) -> Vec<Warning> {
    let Some(refs) = blocks.iter().find(|b| b.id == "tali-references") else {
        return Vec::new();
    };
    let rows = refs.html.matches("class=\"csl-entry\">").count();
    let raw = refs.html.matches("</code></div>").count();
    if rows == 0 || raw < rows {
        return Vec::new();
    }
    let declares_bib = crate::frontmatter::front_matter_block(src)
        .and_then(|fm| serde_yaml::from_str::<serde_yaml::Value>(fm).ok())
        .and_then(|v| v.as_mapping().map(|m| m.get("bibliography").is_some()))
        .unwrap_or(false);
    if declares_bib {
        return Vec::new();
    }
    match crate::site::project_bibliography_has_entries(base) {
        Some(true) => Vec::new(),
        Some(false) => vec![Warning::new(
            "citations are present but no `bibliography:` entry could be read: the project's \
             `_site.yml` declares one and nothing in it loaded, so every reference renders as \
             a raw key",
        )],
        None => vec![Warning::new(
            "citations are present but no `bibliography:` is declared, so every reference renders as a raw key",
        )],
    }
}

// The `csl:` recognized-but-unsupported warning used to live here. It moved to
// `frontmatter::validate_unsupported_keys` so it fires on the RENDER path: this module is
// check-only (nothing under `serve/` calls it), so an author would have kept seeing the
// silence in the preview, which is the surface they actually read.
