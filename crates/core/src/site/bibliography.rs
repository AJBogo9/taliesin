//! The project-wide `bibliography:` — a `.bib` shared by every page of a site, declared
//! once in `_site.yml` instead of retyped in each post's front matter.
//!
//! Two things live here, and the split is the whole design:
//!
//! - **Resolution** ([`resolve_shared`]) happens once, at `Site::discover`, against the
//!   site root. Doing it per page would report the same bad path N times and would make
//!   "relative to what?" depend on which page happened to be rendering.
//! - **The unused-entry lint** ([`Site::validate_shared_bibliography`]) is a *site-wide*
//!   pass, because a shared entry cited by one page is used even though every other page
//!   leaves it alone. The per-page mirror of this check (`cite::process`) is deliberately
//!   scoped to what the page itself declared, for the same reason.
//!
//! Neither touches the BibTeX parser or the CSL formatter.

use super::Site;
use crate::render::Warning;
use std::path::{Path, PathBuf};

/// Resolve `_site.yml`'s `bibliography:` entries against the site root, dropping (with a
/// warning) any that a page-level `bibliography:` would also refuse. Returns the readable
/// absolute paths, in declaration order.
///
/// The messages match `render::load_bibliography`'s word for word: one bad `.bib` path
/// should read the same whether it was written in a page or in the project config.
pub(super) fn resolve_shared(
    root: &Path,
    declared: &[String],
    warnings: &mut Vec<String>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in declared {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        if !path.ends_with(".bib") {
            warnings.push(format!(
                "bibliography `{path}` ignored: only BibTeX (`.bib`) is supported"
            ));
            continue;
        }
        // The site root is both the base and the containment boundary, the same pair
        // `Site::discover` uses for the head/body/css includes: a project-wide config key
        // may not point outside the project.
        match crate::includes::try_join_in(root, path, Some(root)) {
            Ok(p) if p.is_file() => out.push(p),
            Ok(_) => warnings.push(format!("bibliography file not found: {path}")),
            Err(crate::includes::Refused::OutsideRoot) => warnings.push(format!(
                "bibliography `{path}` is outside the project root and was not read"
            )),
            Err(crate::includes::Refused::SymlinkOutsideRepo) => warnings.push(format!(
                "bibliography `{path}` is a symlink whose target is outside the project \
                 repository and was not read"
            )),
        }
    }
    out
}

/// The project-wide `bibliography:` for a document opened **directly** (`preview post.tmd`,
/// `check post.tmd`, the LSP), read from the `_site.yml` at `root` — the same marker
/// [`crate::includes::single_doc_root`] already walked to when it chose the containment root.
/// Empty when `root` holds no `_site.yml`, or none that declares the key.
///
/// Without this a site page renders as two different documents depending on how it was
/// invoked: `preview <dir>` resolves its shared citations and `preview <page.tmd>` shows raw
/// keys. That is the same defect the single-document containment root was unified to kill
/// (PP-3), and previewing one post of a series is the workflow the shared key exists for.
///
/// `_site.yml`'s **own** diagnostics are dropped here on purpose. A bad project path is a
/// project-level mistake belonging to a project-level check; surfacing it as a warning on
/// whichever page happens to be open would attribute it to the wrong file.
pub(crate) fn shared_for_single_doc(root: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(root.join("_site.yml")) else {
        return Vec::new();
    };
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return Vec::new();
    };
    let declared = crate::site::frontmatter::string_list(value.get("bibliography"));
    if declared.is_empty() {
        return Vec::new();
    }
    resolve_shared(root, &declared, &mut Vec::new())
}

impl Site {
    /// What every page of this project inherits from `_site.yml`: the project-wide
    /// `bibliography:`. One value, so a render call site names the project once instead of
    /// listing its policies (and so the next project-wide key does not widen six signatures
    /// again).
    ///
    /// Cheap but not free (it clones the resolved paths), so a loop over pages should bind
    /// it once rather than call it per page.
    pub fn render_defaults(&self) -> crate::render::SiteDefaults {
        crate::render::SiteDefaults {
            bibliography: self.bibliography.clone(),
        }
    }

    /// The project-wide bibliography as text, concatenated in declaration order. Empty
    /// when `_site.yml` declares none, which is every project that predates the key.
    ///
    /// Read per page render rather than parsed once and shared: a [`crate::cite::Bibliography`]
    /// is built per document (the page's own entries are laid over this one), and the files
    /// are a handful of kilobytes next to a full markdown render.
    pub fn shared_bibliography_text(&self) -> String {
        let mut text = String::new();
        for p in &self.bibliography {
            if let Ok(content) = std::fs::read_to_string(p) {
                text.push_str(&content);
                text.push('\n');
            }
        }
        text
    }

    /// Site-wide hygiene for the shared `.bib`, reported against `_site.yml`: duplicate
    /// keys within it.
    ///
    /// Read-only — it never edits a `.bib` and never changes what renders. Empty for a
    /// project with no `_site.yml` `bibliography:`, so it costs nothing to call
    /// unconditionally.
    ///
    /// It also reported entries **no page** cites until 2026-08-20. That half read every
    /// page's source and expanded its includes, on every call, to answer a question whose
    /// answer never affected a rendered page — an uncited `.bib` entry produces no defect a
    /// reader can see, because the References list holds only cited keys and the `.bib`
    /// itself is unpublished source. The duplicate-key check stays: it names two entries
    /// that disagree, and the build silently uses the last one.
    pub fn validate_shared_bibliography(&self) -> Vec<Warning> {
        if self.bibliography.is_empty() {
            return Vec::new();
        }
        let text = self.shared_bibliography_text();
        let (_bib, dup_warnings) = crate::cite::parse_bib_warned(&text);
        dup_warnings.into_iter().map(Warning::new).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tests::write_site;

    fn messages(w: &[Warning]) -> Vec<String> {
        w.iter().map(|x| x.message.clone()).collect()
    }

    #[test]
    fn a_duplicate_key_within_the_shared_file_is_reported_against_the_project() {
        let root = write_site(
            "shared-bib-dup",
            &[
                ("_site.yml", "title: T\nbibliography: refs.bib\n"),
                (
                    "refs.bib",
                    "@article{k,\n title = {One},\n year = {2020}\n}\n\
                     @article{k,\n title = {Two},\n year = {2021}\n}\n",
                ),
                ("index.tmd", "---\ntitle: A\n---\n\nSee [@k].\n"),
            ],
        );
        let w = messages(&Site::discover(&root).validate_shared_bibliography());
        assert!(
            w.iter().any(|m| m.contains("duplicate bibliography key")),
            "a duplicate inside the shared file is the project's problem: {w:?}"
        );
    }

    #[test]
    fn a_project_declaring_no_bibliography_is_never_linted() {
        let root = write_site(
            "shared-bib-absent",
            &[
                ("_site.yml", "title: T\n"),
                ("index.tmd", "---\ntitle: A\n---\n\nProse.\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(
            site.bibliography.is_empty(),
            "nothing declared, nothing resolved"
        );
        assert!(site.validate_shared_bibliography().is_empty());
    }

    #[test]
    fn a_shared_bib_path_that_does_not_resolve_warns_once_at_discovery() {
        let root = write_site(
            "shared-bib-bad-path",
            &[
                (
                    "_site.yml",
                    "title: T\nbibliography: [missing.bib, ../escape.bib, notes.txt]\n",
                ),
                ("index.tmd", "---\ntitle: A\n---\n\nProse.\n"),
                ("other.tmd", "---\ntitle: B\n---\n\nProse.\n"),
            ],
        );
        let site = Site::discover(&root);
        assert!(site.bibliography.is_empty(), "none of the three resolves");
        let w: Vec<&String> = site
            .warnings
            .iter()
            .filter(|m| m.contains("bibliography"))
            .collect();
        // Three declarations, three diagnostics — and each exactly once, not once per page,
        // which is the reason resolution happens here rather than in the render pass.
        assert_eq!(w.len(), 3, "one diagnostic per bad declaration: {w:?}");
        assert!(w.iter().any(|m| m.contains("not found")), "{w:?}");
        assert!(
            w.iter().any(|m| m.contains("outside the project root")),
            "{w:?}"
        );
        assert!(w.iter().any(|m| m.contains("only BibTeX")), "{w:?}");
    }
}
