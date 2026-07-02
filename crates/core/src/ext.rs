//! The source-file extension vocabulary, defined once.
//!
//! Taliesin's native source extension is `.tmd`; `.qmd` (the Quarto spelling) stays
//! accepted so existing trees keep working unchanged. Every place that recognizes a
//! source file — the site page walker, the `check` walker, link rewriting, book
//! chapter naming, deck/embed href mapping — routes through here, so the accepted set
//! lives in one spot and the two spellings never drift apart.

use std::path::Path;

/// The native source extension (no leading dot). New scaffolding writes this.
pub const SOURCE_EXT: &str = "tmd";

/// Every accepted source extension (no leading dot), native first. A file is a
/// Taliesin source document iff its extension is one of these.
pub const ACCEPTED_SOURCE_EXTS: &[&str] = &["tmd", "qmd"];

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

/// Strip a trailing accepted source extension (`.tmd` / `.qmd`) from a path string,
/// returning the stem. `None` if the string has no accepted source extension — so a
/// non-source path (or a bare `"tmd"`) round-trips through callers unchanged.
pub fn strip_source_ext(path: &str) -> Option<&str> {
    ACCEPTED_SOURCE_EXTS
        .iter()
        .find_map(|e| path.strip_suffix(e).and_then(|p| p.strip_suffix('.')))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_both_spellings_rejects_others() {
        assert!(is_source_ext("tmd") && is_source_ext("qmd"));
        assert!(!is_source_ext("md") && !is_source_ext("html") && !is_source_ext(""));
        assert!(is_source_path(Path::new("a/b/index.tmd")));
        assert!(is_source_path(Path::new("post.qmd")));
        assert!(!is_source_path(Path::new("style.css")));
        assert!(!is_source_path(Path::new("noext")));
    }

    #[test]
    fn strips_only_a_real_trailing_source_ext() {
        assert_eq!(strip_source_ext("index.tmd"), Some("index"));
        assert_eq!(strip_source_ext("sub/index.qmd"), Some("sub/index"));
        assert_eq!(strip_source_ext("plain.html"), None);
        assert_eq!(strip_source_ext("x.notmd"), None); // not a `.tmd` boundary
        assert_eq!(strip_source_ext("tmd"), None); // bare, no dot
    }
}
