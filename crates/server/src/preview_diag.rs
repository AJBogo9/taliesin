//! Bridge: run the static validators (the `check` superset) over an already-rendered
//! preview document and convert them into `protocol::Diagnostic`s for the dev menu, so
//! the live preview and `check` cannot drift on what counts as a defect.
//!
//! Returns `protocol::Diagnostic` (the client wire type), using the exact
//! `Warning -> Diagnostic` mapping both serve paths already inline for render/xref
//! warnings. See docs/superpowers/specs/2026-07-18-dx1-live-preview-validation-design.md

use crate::check::{Scope, page_static_diagnostics};
use crate::protocol::Diagnostic;
use std::path::Path;

/// Located (file+line) when the warning carries a location, else attributed to "the
/// previewed document" (`file = None`, which the client resolves to the doc's path).
fn located(w: &taliesin_core::render::Warning) -> Diagnostic {
    let mut d = Diagnostic::warn(&w.message);
    if let Some(line) = w.line {
        d = d.at(w.file.clone(), line);
    }
    d
}

/// Static lints over an already-rendered preview doc's blocks. MUST be called on
/// **pre-execution** blocks (before the executor runs the code cells).
pub(crate) fn static_diagnostics(
    src: &str,
    blocks: &[taliesin_core::Block],
    base: &Path,
    format: taliesin_core::DocFormat,
    scope: Scope,
) -> Vec<Diagnostic> {
    page_static_diagnostics(src, blocks, base, format, scope)
        .iter()
        .map(located)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A fresh empty temp dir to use as the render base (no image files present, so a
    /// local-image reference is "missing").
    fn tmp_base(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tali-dx1-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn static_diagnostics_flag_a_missing_local_image() {
        let base = tmp_base("static-img");
        let src = "# Title\n\n![a chart](nope.png)\n";
        let doc = taliesin_core::render_document_with_includes_rooted(
            src,
            base.as_path(),
            Some(base.as_path()),
        );
        let diags = static_diagnostics(
            src,
            &doc.blocks,
            base.as_path(),
            doc.format,
            Scope::Standalone,
        );
        assert!(
            diags.iter().any(|d| d.message.contains("nope.png")),
            "expected a diagnostic naming the missing image, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn static_diagnostics_are_empty_for_a_clean_doc() {
        let base = tmp_base("static-clean");
        let src = "# Title\n\nJust a paragraph of plain prose, no links or images.\n";
        let doc = taliesin_core::render_document_with_includes_rooted(
            src,
            base.as_path(),
            Some(base.as_path()),
        );
        let diags = static_diagnostics(
            src,
            &doc.blocks,
            base.as_path(),
            doc.format,
            Scope::Standalone,
        );
        assert!(
            diags.is_empty(),
            "clean doc should lint clean, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
