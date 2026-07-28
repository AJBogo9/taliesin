//! The source-file extension vocabulary, defined once.
//!
//! Taliesin's native and only source extension is `.tmd`. Every place that
//! recognizes a source file — the site page walker, the `check` walker, link
//! rewriting, book chapter naming, deck/embed href mapping — routes through here.

use std::path::Path;

/// Every accepted source extension (no leading dot). A file is a Taliesin source
/// document iff its extension is one of these.
pub const ACCEPTED_SOURCE_EXTS: &[&str] = &["tmd"];

/// Whether an extension string (no dot, as returned by [`Path::extension`]) names a
/// source document.
pub fn is_source_ext(ext: &str) -> bool {
    ACCEPTED_SOURCE_EXTS.contains(&ext)
}

/// Whether a path is a Taliesin source document, judged by its extension.
pub fn is_source_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(is_source_ext)
}

/// Strip a trailing accepted source extension (`.tmd`) from a path string,
/// returning the stem. `None` if the string has no accepted source extension — so a
/// non-source path (or a bare `"tmd"`) round-trips through callers unchanged.
pub fn strip_source_ext(path: &str) -> Option<&str> {
    ACCEPTED_SOURCE_EXTS
        .iter()
        .find_map(|e| path.strip_suffix(e).and_then(|p| p.strip_suffix('.')))
}

/// Document extensions a *migrated* project's links keep pointing at. Not "every extension
/// that is not `.tmd`": a link to `report.pdf` or `data.csv` names an asset the author means
/// to ship, and suggesting `report.tmd` for it would be noise.
///
/// Measured shapes (item 128): `.md` on `rust-lang/book`, `.qmd` on a real Quarto book. The
/// rest are the same family, listed so the first user carrying one is not the person who
/// discovers the gap.
const MIGRATED_DOC_EXTS: &[&str] = &["md", "markdown", "qmd", "rmd", "Rmd", "ipynb"];

/// Candidate accepted-source spellings of a link target whose extension is a document
/// extension from another tool: `creators.qmd` → `["creators.tmd"]`.
///
/// Renaming the sources is the one manual step a migration cannot avoid, and every internal
/// link then points at the old extension: **118 of 123** link errors on `rust-lang/book` and
/// **10 of 11** on a real Quarto book were this single shape, with `creators.qmd` reported
/// broken while `creators.tmd` sat in the same directory (item 128).
///
/// A *suggestion*, deliberately not a silent rewrite: a `.md` link may point at a real
/// shipped `.md` file (measured — the fixture for this carries one), and rewriting it would
/// break a link that works. Empty when the extension is absent, already accepted, or not a
/// document extension, so the caller can append the result unconditionally.
pub fn migrated_source_candidates(path: &str) -> Vec<String> {
    let Some((stem, ext)) = path.rsplit_once('.') else {
        return Vec::new();
    };
    // A path like `dir.v2/page` has a "." before the last segment, not an extension.
    if stem.is_empty() || ext.contains('/') || !MIGRATED_DOC_EXTS.contains(&ext) {
        return Vec::new();
    }
    ACCEPTED_SOURCE_EXTS
        .iter()
        .map(|e| format!("{stem}.{e}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrated_candidates_cover_the_measured_shapes_and_nothing_else() {
        // The two shapes real documents carry.
        assert_eq!(migrated_source_candidates("creators.qmd"), ["creators.tmd"]);
        assert_eq!(migrated_source_candidates("ch01.md"), ["ch01.tmd"]);
        // A nested path keeps its directories.
        assert_eq!(
            migrated_source_candidates("part/ch01.md"),
            ["part/ch01.tmd"]
        );
        // Not a document extension: an asset link must not be told to become a page.
        for keep in ["report.pdf", "data.csv", "logo.svg", "archive.tar.gz"] {
            assert!(
                migrated_source_candidates(keep).is_empty(),
                "`{keep}` must get no suggestion"
            );
        }
        // Already accepted, or built, or extensionless: nothing to suggest.
        for keep in ["page.tmd", "page.html", "page", "", ".md", "dir.v2/page"] {
            assert!(
                migrated_source_candidates(keep).is_empty(),
                "`{keep}` must get no suggestion"
            );
        }
    }

    #[test]
    fn accepts_tmd_only_rejects_the_retired_ext_and_others() {
        assert!(is_source_ext("tmd"));
        assert!(
            !is_source_ext("qmd"),
            ".qmd is no longer an accepted source extension"
        );
        assert!(!is_source_ext("md") && !is_source_ext("html") && !is_source_ext(""));
        assert!(is_source_path(Path::new("a/b/index.tmd")));
        assert!(!is_source_path(Path::new("a/b/index.qmd")));
    }

    #[test]
    fn strips_only_a_real_trailing_source_ext() {
        assert_eq!(strip_source_ext("index.tmd"), Some("index"));
        assert_eq!(strip_source_ext("plain.html"), None);
        assert_eq!(strip_source_ext("x.notmd"), None); // not a `.tmd` boundary
        assert_eq!(strip_source_ext("tmd"), None); // bare, no dot
        assert_eq!(
            strip_source_ext("sub/index.qmd"),
            None,
            ".qmd is no longer an accepted source extension"
        );
    }
}
