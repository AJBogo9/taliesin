//! The project-wide `bibliography:` — a `.bib` shared by every page of a site, declared
//! once in `_site.yml` instead of retyped in each post's front matter.
//!
//! Two things live here, and the split is the whole design:
//!
//! - **Resolution** ([`resolve_shared`]) happens once, at `Site::discover`, against the
//!   site root. Doing it per page would report the same bad path N times and would make
//!   "relative to what?" depend on which page happened to be rendering.
//! - **The hygiene check** ([`Site::validate_shared_bibliography`]) reads the shared files
//!   once and reports what is wrong inside them (a duplicate key, an entry never closed, a
//!   key no citation can name, an undefined `@string` macro, a file that is not UTF-8)
//!   against `_site.yml`, where they are declared. Reported per page, one mistake would
//!   print once per page, which is why a page render drops the shared layer's diagnostics.
//!
//! Both read the files through `cite::read_bib_files`, the one `.bib` reader.

use super::Site;
use crate::render::{Severity, Warning};
use std::path::{Path, PathBuf};

/// Resolve `_site.yml`'s `bibliography:` entries against the site root, dropping (with a
/// warning) any that a page-level `bibliography:` would also refuse. Returns the absolute
/// paths, in declaration order.
///
/// A file that does not exist yet is kept: whether it exists is a fact about the disk when a
/// page renders, not when the project was discovered. Dropping it here meant no render ever
/// looked for it, so the preview never learned the author had created it (the natural order
/// is to declare `bibliography: refs.bib`, then write `refs.bib`), and the "not found" this
/// used to say stayed on screen after the file existed (audit 2026-09-24 C4). The one `.bib`
/// reader reports a missing file when it is read, in `validate_shared_bibliography`.
///
/// The messages match `render::load_bibliography`'s word for word: one bad `.bib` path
/// should read the same whether it was written in a page or in the project config.
///
/// Each warning is located at `_site.yml`'s `bibliography:` key.
pub(super) fn resolve_shared(
    root: &Path,
    declared: &[String],
    warnings: &mut Vec<Warning>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut warn = |message: String| {
        let line = super::config::read_site_yml(root)
            .ok()
            .and_then(|text| super::config::key_line(&text, "bibliography"));
        warnings.push(super::config::config_warning(
            line,
            Severity::Warning,
            message,
        ));
    };
    for path in declared {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        if !path.ends_with(".bib") {
            warn(format!(
                "bibliography `{path}` ignored: only BibTeX (`.bib`) is supported"
            ));
            continue;
        }
        // The site root is both the base and the containment boundary, the same pair
        // `Site::discover` uses for the head/body/css includes: a project-wide config key
        // may not point outside the project.
        match crate::includes::try_join_in(root, path, Some(root)) {
            Ok(p) => out.push(p),
            Err(crate::includes::Refused::OutsideRoot) => warn(format!(
                "bibliography `{path}` is outside the project root and was not read"
            )),
            Err(crate::includes::Refused::SymlinkOutsideRepo) => warn(format!(
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
    let declared = declared_at(root);
    if declared.is_empty() {
        return Vec::new();
    }
    resolve_shared(root, &declared, &mut Vec::new())
}

/// What a document in `doc_dir` inherits from its project's `_site.yml` `bibliography:`:
/// `None` when the project declares none, else whether any entry could be read from it.
/// `Some(false)` is a declaration that yields nothing (a missing or non-UTF-8 file, an
/// empty one), which a page checked on its own has no other way to hear about.
pub(crate) fn project_bibliography_has_entries(doc_dir: &Path) -> Option<bool> {
    let root = crate::includes::single_doc_root(doc_dir);
    let declared = declared_at(&root);
    if declared.is_empty() {
        return None;
    }
    let files: Vec<(String, PathBuf)> = resolve_shared(&root, &declared, &mut Vec::new())
        .into_iter()
        .map(|p| (String::new(), p))
        .collect();
    let mut bib = crate::cite::Bibliography::default();
    crate::cite::read_bib_files(&mut bib, &files, &mut Default::default());
    Some(!bib.is_empty())
}

/// The `bibliography:` entries of the `_site.yml` at `root`, as written. Empty when there
/// is none, or it declares none.
fn declared_at(root: &Path) -> Vec<String> {
    let Ok(text) = super::config::read_site_yml(root) else {
        return Vec::new();
    };
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return Vec::new();
    };
    crate::site::frontmatter::string_list(value.get("bibliography"))
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

    /// Site-wide hygiene for the shared `.bib`, reported against `_site.yml`: whatever the
    /// one `.bib` reader finds in the files (a duplicate key, an entry never closed, a key no
    /// citation can name, an undefined `@string` macro, a file that is not UTF-8), each
    /// named by file and, inside a file, by line.
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
        // Named relative to the project root, the way `_site.yml` declared them
        // (`resolve_shared` joined each onto the absolutized root).
        let root = crate::includes::absolutize(&self.root);
        let files: Vec<(String, PathBuf)> = self
            .bibliography
            .iter()
            .map(|p| {
                let name = p.strip_prefix(&root).unwrap_or(p);
                (name.display().to_string(), p.clone())
            })
            .collect();
        let mut bib = crate::cite::Bibliography::default();
        crate::cite::read_bib_files(&mut bib, &files, &mut Default::default())
            .into_iter()
            .map(Warning::new)
            .collect()
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

    /// The shared files are read one by one, so an entry left unclosed at the end of
    /// `a.bib` cannot eat the first entry of `b.bib`; the project check names the file. The
    /// files used to be concatenated into one text first (audit 2026-09-24 G3).
    #[test]
    fn an_unclosed_entry_in_one_shared_file_does_not_swallow_the_next_file() {
        let root = write_site(
            "shared-bib-unclosed",
            &[
                ("_site.yml", "title: T\nbibliography: [a.bib, b.bib]\n"),
                (
                    "a.bib",
                    "@article{a1, title={From a}, year={2001}}\n\
                     @article{a2, title={Unclosed}, year={2002}\n",
                ),
                ("b.bib", "@article{b1, title={First in b}, year={2003}}\n"),
                ("index.tmd", "---\ntitle: A\n---\n\nSee [@b1].\n"),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").expect("renders");
        assert!(html.contains("First in b"), "b1 resolves:\n{html}");
        let w = messages(&site.validate_shared_bibliography());
        assert!(
            w.iter()
                .any(|m| m.contains("a.bib") && m.contains("a2") && m.contains("not closed")),
            "{w:?}"
        );
    }

    /// A page's `.bib` can use an `@string` macro the project's shared `.bib` defines, as
    /// `\bibliography{shared,page}` shares macros across its files (the `IEEEabrv.bib`
    /// pattern). The two layers used to be parsed as separate texts, so the page printed the
    /// macro's name, `jn`, as the journal (audit 2026-09-24, bibtex #20).
    #[test]
    fn a_page_bib_can_use_a_string_macro_the_shared_bib_defines() {
        let root = write_site(
            "shared-bib-macro",
            &[
                ("_site.yml", "title: T\nbibliography: shared.bib\n"),
                ("shared.bib", "@string{jn = {Shared Journal}}\n"),
                (
                    "page.bib",
                    "@article{p1, title={Page entry}, journal=jn, year={2005}}\n",
                ),
                (
                    "index.tmd",
                    "---\ntitle: A\nbibliography: page.bib\n---\n\nSee [@p1].\n",
                ),
            ],
        );
        let site = Site::discover(&root);
        let html = site.render_page("index.tmd").expect("renders");
        assert!(html.contains("<em>Shared Journal</em>"), "{html}");
        // The same page opened on its own reads the same layers in the same order.
        let src = std::fs::read_to_string(root.join("index.tmd")).unwrap();
        let doc = crate::render_single_doc(&src, &root);
        assert!(doc.body_html().contains("<em>Shared Journal</em>"));
        assert!(
            !doc.warnings
                .iter()
                .any(|w| w.message.contains("not defined")),
            "{:?}",
            doc.warnings
        );
    }

    /// A shared `.bib` that is not UTF-8 was skipped in silence, leaving every page's
    /// citations as raw keys with no diagnostic that named the file (audit 2026-09-24,
    /// bibtex #9). The project check reports it.
    #[test]
    fn a_shared_bib_that_is_not_utf8_is_reported_against_the_project() {
        let root = write_site(
            "shared-bib-latin1",
            &[
                ("_site.yml", "title: T\nbibliography: refs.bib\n"),
                ("index.tmd", "---\ntitle: A\n---\n\nSee [@k].\n"),
            ],
        );
        std::fs::write(
            root.join("refs.bib"),
            b"@article{k, author={M\xfcller, Hans}, title={T}, year={2020}}\n",
        )
        .unwrap();
        let w = messages(&Site::discover(&root).validate_shared_bibliography());
        assert!(
            w.iter()
                .any(|m| m.contains("refs.bib") && m.contains("not valid UTF-8")),
            "{w:?}"
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

    /// Each bad declaration is said once, not once per page, which is why resolution
    /// happens at discovery rather than in the render pass. A file that is not there is the
    /// exception to saying it at discovery: it is kept, because a page's render will look
    /// for it (and the preview then rebuilds the page when it appears, audit 2026-09-24 C4),
    /// and the hygiene check reports it, against the disk as it is when a verb runs.
    #[test]
    fn a_shared_bib_path_that_does_not_resolve_warns_once() {
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
        assert_eq!(
            site.bibliography,
            vec![root.join("missing.bib")],
            "only the file not written yet is kept"
        );
        let w: Vec<&str> = site
            .warnings
            .iter()
            .map(|m| m.message.as_str())
            .filter(|m| m.contains("bibliography"))
            .collect();
        assert_eq!(w.len(), 2, "one diagnostic per refused declaration: {w:?}");
        assert!(
            w.iter().any(|m| m.contains("outside the project root")),
            "{w:?}"
        );
        assert!(w.iter().any(|m| m.contains("only BibTeX")), "{w:?}");
        let missing = messages(&site.validate_shared_bibliography());
        assert_eq!(
            missing,
            vec!["bibliography file not found: missing.bib".to_string()]
        );
    }
}
