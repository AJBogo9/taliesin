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

/// Cross-page relative-link + anchor existence for ONE page, resolved against the whole
/// site registry (the site-aware counterpart to `validate_local_links`, which `InSite`
/// omits). Runs the whole-site check (~27 ms) and keeps only this page's findings, so a
/// link broken by an edit to a *different* page refreshes when that page next rebuilds.
pub(crate) fn cross_page_diagnostics(
    site: &taliesin_core::Site,
    page_rel: &str,
) -> Vec<Diagnostic> {
    site.validate_cross_page_links()
        .into_iter()
        .filter(|(rel, _)| rel == page_rel)
        .map(|(_, w)| located(&w))
        .collect()
}

/// `_site.yml` config warnings (unknown keys / typos), attributed to the config file.
/// The missing-`_site.yml` advisory is dropped: a bare dir of `.tmd` is a valid project.
/// `protocol::Diagnostic` has no "file without line" constructor, so set `file` directly.
pub(crate) fn site_config_diagnostics(site: &taliesin_core::Site) -> Vec<Diagnostic> {
    site.warnings
        .iter()
        .filter(|m| !taliesin_core::site::is_missing_config_warning(m))
        .map(|m| {
            let mut d = Diagnostic::warn(m);
            d.file = Some("_site.yml".to_string());
            d
        })
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

    /// A temp multi-page site dir. `files` is (relative name, contents).
    fn tmp_site(tag: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tali-dx1-site-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        dir
    }

    #[test]
    fn cross_page_diagnostics_flag_a_broken_link_only_on_the_linking_page() {
        let dir = tmp_site(
            "xpage",
            &[
                (
                    "index.tmd",
                    "# Home\n\nSee [the other page](other.tmd#nope).\n",
                ),
                ("other.tmd", "# Real Heading\n\nBody.\n"),
            ],
        );
        let site = taliesin_core::Site::discover(dir.as_path());
        let index_rel = site
            .pages
            .iter()
            .find(|p| p.input.ends_with("index.tmd"))
            .expect("index page discovered")
            .rel
            .clone();
        let other_rel = site
            .pages
            .iter()
            .find(|p| p.input.ends_with("other.tmd"))
            .expect("other page discovered")
            .rel
            .clone();

        let on_index = cross_page_diagnostics(&site, &index_rel);
        assert!(
            !on_index.is_empty(),
            "index links a nonexistent anchor; expected a diagnostic, got none"
        );
        let on_other = cross_page_diagnostics(&site, &other_rel);
        assert!(
            on_other.is_empty(),
            "other.tmd has no broken outgoing link; expected none, got: {:?}",
            on_other.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn site_config_diagnostics_surface_an_unknown_key_attributed_to_site_yml() {
        let dir = tmp_site(
            "cfg",
            &[
                ("_site.yml", "title: My Site\ntitel: oops\n"),
                ("index.tmd", "# Home\n"),
            ],
        );
        let site = taliesin_core::Site::discover(dir.as_path());
        // Precondition: discover must have typo-warned on the unknown `titel` key.
        assert!(
            !site.warnings.is_empty(),
            "fixture precondition: an unknown _site.yml key should warn; if not, use the \
             exact unknown-key form the config linter recognizes (site/config/mod.rs)"
        );
        let diags = site_config_diagnostics(&site);
        assert!(
            !diags.is_empty(),
            "expected the config warning surfaced as a diagnostic"
        );
        assert!(
            diags.iter().all(|d| d.file.as_deref() == Some("_site.yml")),
            "config diagnostics must be attributed to _site.yml"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
