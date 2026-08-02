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
    /// keys within it, and entries **no page** cites.
    ///
    /// Read-only — it never edits a `.bib` and never changes what renders. Empty for a
    /// project with no `_site.yml` `bibliography:`, so it costs nothing to call
    /// unconditionally.
    ///
    /// The unused check is site-wide by necessity: scoping it per page would flag every
    /// shared entry on every page that happens not to cite it, which is the normal case and
    /// would make the lint useless. Citations are counted from each page's **source**
    /// ([`crate::cite::cited_keys_in_source`]) rather than from a render, so this stays a
    /// cheap read of files the caller has already discovered.
    pub fn validate_shared_bibliography(&self) -> Vec<Warning> {
        if self.bibliography.is_empty() {
            return Vec::new();
        }
        let text = self.shared_bibliography_text();
        let (bib, dup_warnings) = crate::cite::parse_bib_warned(&text);
        let mut out: Vec<Warning> = dup_warnings.into_iter().map(Warning::new).collect();

        // Every page, plus the decks: a deck is held out of `pages` but is built, served
        // and may cite. Missing one would report a cited entry as dead weight.
        let mut cited: Vec<String> = Vec::new();
        let sources = self
            .pages
            .iter()
            .map(|p| &p.input)
            .chain(self.decks.iter().map(|d| &d.input));
        for input in sources {
            let Ok(src) = std::fs::read_to_string(input) else {
                continue;
            };
            // Expand includes first, with the same base/root the page render uses: a shared
            // derivation lives in an `_includes/` partial, and its `[@key]` citations are
            // the page's. Scanning the unexpanded source would report every entry cited
            // only from a partial as dead weight.
            let base = input.parent().unwrap_or(&self.root);
            let (expanded, _, _) = crate::includes::resolve_warned_in(&src, base, None);
            for k in crate::cite::cited_keys_in_source(&expanded) {
                if !cited.contains(&k) {
                    cited.push(k);
                }
            }
        }
        let uncited = bib.uncited(&cited);
        if !uncited.is_empty() {
            out.push(Warning::new(crate::cite::uncited_message(&uncited)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::tests::write_site;

    const SHARED: &str = "@article{used,\n author = {A},\n title = {Used},\n year = {2020}\n}\n\
                          @article{dead,\n author = {B},\n title = {Dead},\n year = {2021}\n}\n";

    fn messages(w: &[Warning]) -> Vec<String> {
        w.iter().map(|x| x.message.clone()).collect()
    }

    #[test]
    fn an_entry_no_page_cites_is_reported_once_against_the_project() {
        let root = write_site(
            "shared-bib-dead",
            &[
                ("_site.yml", "title: T\nbibliography: refs.bib\n"),
                ("refs.bib", SHARED),
                ("index.tmd", "---\ntitle: A\n---\n\nSee [@used].\n"),
                ("other.tmd", "---\ntitle: B\n---\n\nNo citations here.\n"),
            ],
        );
        let w = messages(&Site::discover(&root).validate_shared_bibliography());
        assert_eq!(
            w.len(),
            1,
            "one diagnostic for the project, not one per page: {w:?}"
        );
        assert!(
            w[0].contains("`@dead`") && w[0].contains("never cited"),
            "the uncited entry is named: {w:?}"
        );
        assert!(
            !w[0].contains("`@used`"),
            "an entry cited by SOME page is in use: {w:?}"
        );
    }

    #[test]
    fn a_shared_entry_cited_by_only_one_page_is_not_reported() {
        // The scoping rule the whole site-wide pass exists for: judged per page, `used`
        // would be "unused" on `other.tmd` and the lint would fire on every real project.
        let root = write_site(
            "shared-bib-scope",
            &[
                ("_site.yml", "title: T\nbibliography: refs.bib\n"),
                (
                    "refs.bib",
                    "@article{used,\n author = {A},\n title = {Used},\n year = {2020}\n}\n",
                ),
                ("index.tmd", "---\ntitle: A\n---\n\nSee [@used].\n"),
                ("other.tmd", "---\ntitle: B\n---\n\nNothing cited.\n"),
            ],
        );
        assert!(
            Site::discover(&root)
                .validate_shared_bibliography()
                .is_empty(),
            "one citation anywhere in the project keeps a shared entry alive"
        );
    }

    #[test]
    fn a_citation_that_only_an_include_makes_still_counts() {
        // The false-positive this pass would otherwise manufacture: a shared derivation
        // lives in a partial, and its citations belong to the page that includes it.
        let root = write_site(
            "shared-bib-include",
            &[
                ("_site.yml", "title: T\nbibliography: refs.bib\n"),
                (
                    "refs.bib",
                    "@article{used,\n author = {A},\n title = {Used},\n year = {2020}\n}\n",
                ),
                (
                    "index.tmd",
                    "---\ntitle: A\n---\n\n{{< include _part.tmd >}}\n",
                ),
                ("_part.tmd", "Derivation, citing [@used].\n"),
            ],
        );
        let w = messages(&Site::discover(&root).validate_shared_bibliography());
        assert!(
            w.is_empty(),
            "an included citation is the page's own: {w:?}"
        );
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
