//! Bridge: run the static validators (the `build --check-only` superset) over an
//! already-rendered preview document for the dev menu, so the live preview and the
//! pre-publish gate cannot drift on what counts as a defect. (`check` was a verb until
//! wave 9 retired it into that flag.)
//!
//! Returns [`crate::lint::Diagnostic`], the one diagnostic type, through the one
//! `Warning -> Diagnostic` mapping ([`crate::lint::diag_from`]), so each keeps the severity
//! its validator set. `label` is the previewed page's file name: a warning with no `file`
//! of its own is about that page, and the client resolves every `file` against the page's
//! own directory.

use crate::lint::{Diagnostic, Scope, diag_from, page_static_diagnostics};
use std::path::Path;

/// Static lints over an already-rendered preview doc's blocks. MUST be called on
/// **pre-execution** blocks (before the executor runs the code cells).
pub(crate) fn static_diagnostics(
    src: &str,
    blocks: &[taliesin_core::Block],
    base: &Path,
    scope: Scope,
    label: &str,
) -> Vec<Diagnostic> {
    page_static_diagnostics(src, blocks, base, scope)
        .iter()
        .map(|w| diag_from(w, label))
        .collect()
}

/// Cross-page relative-link + anchor existence for ONE page (the site-aware counterpart to
/// `validate_local_links`, which `InSite` omits). A link broken by an edit to a *different*
/// page refreshes when that page next rebuilds.
///
/// Scoped, not filtered. This used to run the whole-site check and discard every other
/// page's findings, so each save of any page in a site or book paid a full-site render pass
/// to keep one page's warnings (PERF-1). It renders the page plus the pages it links to
/// instead, which is the same answer for a fraction of the work — and, unlike the old
/// version, work that does not grow with the size of the book.
pub(crate) fn cross_page_diagnostics(
    site: &taliesin_core::Site,
    page_rel: &str,
    label: &str,
) -> Vec<Diagnostic> {
    site.validate_cross_page_links_for(page_rel)
        .iter()
        .map(|w| diag_from(w, label))
        .collect()
}

/// `_site.yml` config warnings (unknown keys / typos), attributed to the config file.
/// The missing-`_site.yml` advisory is dropped: a bare dir of `.tmd` is a valid project.
pub(crate) fn site_config_diagnostics(site: &taliesin_core::Site) -> Vec<Diagnostic> {
    site.warnings
        .iter()
        .filter(|m| !taliesin_core::site::is_missing_config_warning(m))
        .map(|m| {
            diag_from(
                &taliesin_core::render::Warning::new(m.as_str()),
                "_site.yml",
            )
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
        let doc = taliesin_core::render_single_doc(src, base.as_path());
        let diags =
            static_diagnostics(src, &doc.blocks, base.as_path(), Scope::Standalone, "d.tmd");
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
        let doc = taliesin_core::render_single_doc(src, base.as_path());
        let diags =
            static_diagnostics(src, &doc.blocks, base.as_path(), Scope::Standalone, "d.tmd");
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

        let on_index = cross_page_diagnostics(&site, &index_rel, "index.tmd");
        assert!(
            !on_index.is_empty(),
            "index links a nonexistent anchor; expected a diagnostic, got none"
        );
        let on_other = cross_page_diagnostics(&site, &other_rel, "other.tmd");
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
            diags.iter().all(|d| d.file == "_site.yml"),
            "config diagnostics must be attributed to _site.yml"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
