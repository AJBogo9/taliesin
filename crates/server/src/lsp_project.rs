//! What the enclosing `_site.yml` project contains, for the editor surfaces that reach past
//! the open buffer: cross-file go-to-definition, workspace symbols, and the sidebar's outline
//! and references views.
//!
//! **Why a walk behind a memo, and not an index.** Every consumer fires on a *user gesture*
//! (F12, Ctrl+T, opening a view, the Explorer asking for a decoration), never per keystroke,
//! so none of them needs a live index. An index would put file watching, invalidation and
//! background state into a component whose statelessness is why it is reliable. Instead the
//! walk is cached and validated by `stat`ing every page and comparing `(mtime, len)`: a stat
//! is orders of magnitude cheaper than the read-plus-parse it guards, and the failure mode is
//! "re-walked when it need not have", never "served stale data".
//!
//! The per-keystroke diagnostic path does NOT use [`ProjectCache`]. It keeps calling
//! `site::anchors_defined_elsewhere_in_project` behind the existing coalescing window.
//! It does use [`SiteCache`] below, which is the same stat-validated shape holding a
//! different thing (the page registry) for a caller with the opposite cost profile — see
//! the note there for the measurement that forced it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A cross-reference target defined somewhere in the project.
pub(crate) struct ProjectAnchor {
    pub id: String,
    /// The file the author wrote the anchor in — the page, or the `{{< include >}}` partial
    /// when the anchor lives there. Always paired with a line in THAT file's numbering.
    pub path: PathBuf,
    /// 0-based line of the defining site, in `path`'s own numbering (see [`origin_of`]).
    pub line: u32,
    /// The rendered section number for a numbered chapter heading; empty otherwise.
    pub number: String,
}

/// One walk's result.
pub(crate) struct ProjectScan {
    pub anchors: Vec<ProjectAnchor>,
}

/// What a page looked like when it was last walked: enough to notice an edit without
/// watching the filesystem.
type Stamp = (PathBuf, Option<std::time::SystemTime>, u64);

/// The stat-validated memo described in the module docs, keyed by project root.
///
/// Keyed rather than single-entry because an editor routinely has files from more than one
/// project open at once (a chapter of the guide beside a corpus document). A single entry
/// would re-walk both whenever the author moved between them, turning the memo into pure
/// overhead exactly when it matters.
pub(crate) struct ProjectCache {
    entries: HashMap<PathBuf, (ProjectScan, Vec<Stamp>)>,
    /// How many real walks have happened. Test-visible so the memo cannot be decoration.
    walks: usize,
}

impl ProjectCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            walks: 0,
        }
    }

    /// Walks completed. The `an_unchanged_project_is_not_re_walked` pin reads this; without
    /// it a memo that always missed would still pass every correctness test.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn walks(&self) -> usize {
        self.walks
    }

    /// The scan for `page`'s enclosing project, or `None` when it has no `_site.yml` above it.
    ///
    /// Uses the `.git`-crossing walk, matching `anchors_defined_elsewhere_in_project`: these
    /// two answer the same question for the same editor, and a page whose project one of them
    /// finds and the other does not would resolve a reference in the squiggle and not in F12.
    pub(crate) fn get(&mut self, page: &Path) -> Option<&ProjectScan> {
        let root = taliesin_core::site::enclosing_site_root_across_git(page.parent()?)?;
        let stamps = stamps_for(&root);
        let fresh = self
            .entries
            .get(&root)
            .is_some_and(|(_, seen)| *seen == stamps);
        if !fresh {
            self.entries.insert(root.clone(), (walk(&root), stamps));
            self.walks += 1;
        }
        self.entries.get(&root).map(|(scan, _)| scan)
    }
}

/// The enclosing project as a [`taliesin_core::Site`], memoized the same way and for the
/// opposite reason to [`ProjectCache`].
///
/// This one **does** sit on the per-keystroke diagnostic path, which is exactly why it has to
/// exist. Making the editor's buffer lint site-aware needs the page registry — only it knows
/// that `b.html` is a real page and which ids that page defines — and discovering it costs a
/// full walk: measured at **188 ms on `docs/guide`**, against 14 ms for the entire rest of the
/// lint. Discovering per publish would have made the fix for the missing diagnostic worse than
/// the missing diagnostic.
///
/// Validated by the same `(mtime, len)` stamps, so an edit to any page in the project (or to
/// `_site.yml`) rebuilds it and nothing serves a stale registry. The buffer being edited is
/// *not* read from disk — `validate_cross_page_links_for_src` takes the live text — so the
/// author's own unsaved typing never invalidates this.
///
/// **What it costs, measured over real stdio on `docs/guide` (release):** the first publish
/// for a project goes from 14 ms to 205 ms, and every publish after it is unchanged — typing
/// measured at ~134 ms against a ~130 ms baseline, of which 120 ms is the coalescing window
/// that already gated it. So the walk is paid once when a project's first buffer opens, which
/// is the one moment nobody is waiting on a keystroke.
pub(crate) struct SiteCache {
    entries: HashMap<PathBuf, (taliesin_core::Site, Vec<Stamp>)>,
    builds: usize,
}

impl SiteCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            builds: 0,
        }
    }

    /// Discoveries actually performed. Read by `an_unchanged_project_is_not_re_discovered`,
    /// without which a cache that always missed would still pass every correctness test.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn builds(&self) -> usize {
        self.builds
    }

    /// The project enclosing `page`, or `None` when no `_site.yml` sits above it.
    ///
    /// Whether `page` is a *page* of that project is settled by the caller
    /// (`lint::collect_file_diagnostics_in_site`), so that one place decides it: a deck and
    /// a `draft: true` chapter are both inside a project and are both linted standalone.
    ///
    /// `DraftMode::Exclude` matches `build --check-only`, the parity this whole path claims.
    pub(crate) fn get(&mut self, page: &Path) -> Option<&taliesin_core::Site> {
        let root = taliesin_core::site::enclosing_site_root_across_git(page.parent()?)?;
        let stamps = stamps_for(&root);
        let fresh = self
            .entries
            .get(&root)
            .is_some_and(|(_, seen)| *seen == stamps);
        if !fresh {
            self.entries
                .insert(root.clone(), (taliesin_core::Site::discover(&root), stamps));
            self.builds += 1;
        }
        self.entries.get(&root).map(|(site, _)| site)
    }
}

/// Where each publishable page of the project rooted at `root` is served: `rel` (the source
/// path, project-relative, POSIX) paired with `url`.
///
/// The companion's only structural question about a project — *where is this document
/// served?* — asked so the preview webview opens at the chapter the author is editing rather
/// than at the book's cover. It must be answered here and not in TypeScript: `.tmd`→`.html`,
/// book chapter numbering, `index` handling and the draft/embedded-deck exclusions all live
/// in Rust, and a second implementation over there is exactly what the LSP rewrite existed to
/// delete. This was `taliesin map --format json`, spawned once per preview, until Wave 2 cut
/// the verb; the same walk now answers in-process off [`SiteCache`].
///
/// `None` when `root` encloses no project, or a project with no pages — both of which the
/// client reads as "fall back to the single-file preview".
pub(crate) fn site_map(sites: &mut SiteCache, root: &Path) -> Option<serde_json::Value> {
    // `_site.yml` rather than `root` itself: `SiteCache::get` is keyed on a *page* and walks
    // up from its parent, the same idiom `workspace/symbol` uses at its call site.
    let site = sites.get(&root.join("_site.yml"))?;
    if site.pages.is_empty() {
        return None;
    }
    let pages: Vec<serde_json::Value> = site
        .pages
        .iter()
        .map(|p| serde_json::json!({ "rel": p.rel, "url": p.url }))
        .collect();
    Some(serde_json::json!({ "pages": pages }))
}

/// `(path, mtime, len)` for every page, in `collect_pages` order so two runs compare equal.
///
/// `_site.yml` is stamped alongside the pages because every rule that reads the project config
/// changes what a page's diagnostics should say without the page itself changing.
fn stamps_for(root: &Path) -> Vec<Stamp> {
    let mut inputs = vec![root.join("_site.yml")];
    taliesin_core::site::collect_pages(root, &mut inputs);
    inputs
        .into_iter()
        .map(|p| {
            let meta = std::fs::metadata(&p).ok();
            let mtime = meta.as_ref().and_then(|m| m.modified().ok());
            let len = meta.map(|m| m.len()).unwrap_or(0);
            (p, mtime, len)
        })
        .collect()
}

/// Read every page once and collect the project's cross-reference anchors. Includes are
/// resolved first, so an anchor authored in an `_includes/` partial is found through whichever
/// page includes it, exactly as the render pipeline and `anchors_defined_elsewhere_in_project`
/// do — and is then located back to the partial itself by [`origin_of`], because that is where
/// the author has to go to change it.
fn walk(root: &Path) -> ProjectScan {
    let mut inputs = Vec::new();
    taliesin_core::site::collect_pages(root, &mut inputs);
    let mut anchors = Vec::new();
    let mut seen: HashMap<String, ()> = HashMap::new();

    for input in inputs {
        let Ok(raw) = std::fs::read_to_string(&input) else {
            continue;
        };
        let base = input.parent().unwrap_or_else(|| Path::new("."));
        // The source map is kept, not discarded: `scan_page_anchors` counts lines in the
        // EXPANDED buffer, which is the page's own numbering only until the first
        // `{{< include >}}` shifts everything below it.
        let (src, origins) = taliesin_core::includes::resolve(&raw, base);

        for a in taliesin_core::site::scan_page_anchors(&src, None) {
            // First definition wins project-wide, matching `scan_xref_targets`. Two owners of
            // "which page defines `fig-x`" that disagreed would send F12 somewhere the built
            // page does not link to.
            if seen.insert(a.id.clone(), ()).is_none() {
                let (path, line) = origin_of(&input, base, &origins, a.line);
                anchors.push(ProjectAnchor {
                    id: a.id,
                    path,
                    // The source map reports a 1-based line; everything on the LSP wire
                    // is 0-based.
                    line: line.saturating_sub(1) as u32,
                    number: a.number,
                });
            }
        }
    }
    ProjectScan { anchors }
}

/// Map a 1-based **buffer** line of `page`'s expanded source back to the file and 1-based
/// line the author actually wrote it on.
///
/// This is the LSP-side half of the render pipeline's `map_origin`: a location handed to an
/// editor may only ever pair a file with a line in THAT file's numbering. A buffer line put
/// against the page's own path is not a near miss — any include shifts every line below it,
/// so F12 opened a real file at a line the anchor is not on (`corpus/single-page-report`
/// records `fig-model-hierarchical` at buffer line 131 of a 45-line `index.tmd`, which the
/// editor silently clamps to the end).
///
/// An anchor authored inside a partial belongs to that partial: the source map's label is
/// relative to the page's own directory (`data-source-file`'s contract), so it is joined back
/// onto `base` and normalized. With no map entry — an empty document, or a line past its end —
/// the buffer line IS the page's line, because there was nothing to expand.
fn origin_of(
    page: &Path,
    base: &Path,
    origins: &[taliesin_core::includes::LineOrigin],
    buffer_line: usize,
) -> (PathBuf, usize) {
    match origins.get(buffer_line.saturating_sub(1)) {
        Some(o) => (
            o.file.as_ref().map_or_else(
                || page.to_path_buf(),
                |f| taliesin_core::includes::absolutize(&base.join(f)),
            ),
            o.line,
        ),
        None => (page.to_path_buf(), buffer_line),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory following the house idiom (no `tempfile` dependency in this crate).
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tali-lspproj-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A two-page project: `index.tmd` defines `fig-one` and references `sec-two`;
    /// `ch/two.tmd` defines `sec-two` and references `fig-one` and a dangling `fig-gone`.
    fn fixture(name: &str) -> PathBuf {
        let root = scratch(name);
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::create_dir_all(root.join("ch")).unwrap();
        std::fs::write(
            root.join("index.tmd"),
            "# Index\n\n![p](i.png){#fig-one}\n\nSee @sec-two.\n",
        )
        .unwrap();
        std::fs::write(
            root.join("ch/two.tmd"),
            "# Two {#sec-two}\n\n## Deeper\n\nSee @fig-one and @fig-gone.\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn a_document_outside_any_site_project_scans_to_nothing() {
        let dir = scratch("solo");
        let solo = dir.join("solo.tmd");
        std::fs::write(&solo, "# Solo\n").unwrap();
        let mut cache = ProjectCache::new();
        assert!(
            cache.get(&solo).is_none(),
            "a standalone document has no project; every consumer must fall back silently"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_walk_finds_the_same_project_the_diagnostics_do() {
        // Both this walk and `anchors_defined_elsewhere_in_project` answer "what project is
        // this page in", for the same editor, and they must answer it identically: a page
        // whose project one finds and the other does not resolves a cross-page reference in
        // the squiggle but not under F12. The difference that made this possible is a `.git`
        // between the page and its `_site.yml`, so that is the fixture.
        let base = scratch("gitboundary");
        let inner = base.join("repo");
        std::fs::create_dir_all(inner.join(".git")).unwrap();
        std::fs::write(base.join("_site.yml"), "title: t\n").unwrap();
        std::fs::write(base.join("other.tmd"), "# Other {#sec-other}\n").unwrap();
        let page = inner.join("page.tmd");
        std::fs::write(&page, "See @sec-other.\n").unwrap();

        let by_diagnostics = taliesin_core::site::anchors_defined_elsewhere_in_project(&page);
        assert!(
            by_diagnostics.contains("sec-other"),
            "precondition: the diagnostic path sees across the `.git` boundary"
        );

        let mut cache = ProjectCache::new();
        let scan = cache
            .get(&page)
            .expect("the project walk must find the same project the diagnostics did");
        assert!(scan.anchors.iter().any(|a| a.id == "sec-other"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_walk_collects_anchors_across_every_page() {
        let root = fixture("collect");
        let mut cache = ProjectCache::new();
        let scan = cache.get(&root.join("index.tmd")).unwrap();

        let mut ids: Vec<&str> = scan.anchors.iter().map(|a| a.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["fig-one", "sec-two"], "anchors from BOTH pages");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_anchor_carries_the_page_and_line_that_define_it() {
        let root = fixture("locate");
        let mut cache = ProjectCache::new();
        let scan = cache.get(&root.join("index.tmd")).unwrap();
        let a = scan.anchors.iter().find(|a| a.id == "sec-two").unwrap();
        assert!(a.path.ends_with("ch/two.tmd"));
        assert_eq!(a.line, 0, "0-based line of the defining heading");

        // A definition that is NOT on line 1, so an off-by-one cannot pass by coincidence.
        let f = scan.anchors.iter().find(|a| a.id == "fig-one").unwrap();
        assert_eq!(
            f.line, 2,
            "`{{#fig-one}}` sits on the third line of index.tmd"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// An anchor's recorded location must be in the AUTHOR's coordinate system, not the
    /// post-include buffer's.
    ///
    /// `scan_page_anchors` counts lines in the expanded buffer, so the first
    /// `{{< include >}}` on a page shifts every anchor below it by the whole partial's
    /// length. Pairing that number with the page's own path — the mismatch the
    /// `source_file`/mapped-line rule forbids — sent F12 N lines past the heading, or to
    /// the clamped end of a file with nothing to signal it.
    #[test]
    fn an_anchor_below_an_include_reports_the_authors_own_file_and_line() {
        let root = scratch("include-lines");
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::create_dir_all(root.join("_includes")).unwrap();
        // Five lines, so the shift is unmistakable and no off-by-one can pass by luck.
        std::fs::write(
            root.join("_includes/intro.tmd"),
            "Some intro prose.\n\n## Included {#sec-included}\n\nMore prose.\n",
        )
        .unwrap();
        std::fs::write(
            root.join("chapter.tmd"),
            "# Chapter {#sec-chapter}\n\n{{< include _includes/intro.tmd >}}\n\n\
             ## After {#sec-after}\n",
        )
        .unwrap();

        let mut cache = ProjectCache::new();
        let scan = cache.get(&root.join("chapter.tmd")).unwrap();
        let at = |id: &str| scan.anchors.iter().find(|a| a.id == id).unwrap();

        // Above the include: unchanged, and the control that proves the fixture is walked.
        let before = at("sec-chapter");
        assert!(before.path.ends_with("chapter.tmd"));
        assert_eq!(before.line, 0, "an anchor above the include is untouched");

        // Below it: the partial is five lines where the directive was one, so the buffer
        // line is 9 while the author's is 5.
        let after = at("sec-after");
        assert!(
            after.path.ends_with("chapter.tmd"),
            "an anchor written in the page belongs to the page: {}",
            after.path.display()
        );
        assert_eq!(
            after.line, 4,
            "`## After` is the 5th line of chapter.tmd (0-based 4); a buffer line would be 8"
        );

        // Inside it: the anchor belongs to the partial that defines it, at the partial's
        // own line. Naming the page here would be a real, openable file at a wrong line.
        let inside = at("sec-included");
        assert!(
            inside.path.ends_with("_includes/intro.tmd"),
            "an anchor authored in a partial must resolve to the partial: {}",
            inside.path.display()
        );
        assert_eq!(
            inside.line, 2,
            "`## Included` is the 3rd line of the partial (0-based 2)"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The same property over a real project instead of a fixture: every anchor the walk
    /// records must actually be written at the file and line it names.
    ///
    /// `corpus/single-page-report` is the shape that motivated this — seven
    /// `{{< include >}}`s, with three `{#fig-}` anchors inside the third partial — so a
    /// regression through any other route than the one above still fails here.
    #[test]
    fn every_recorded_anchor_is_written_where_the_walk_says_it_is() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/single-page-report");
        let mut cache = ProjectCache::new();
        let scan = cache
            .get(&root.join("index.tmd"))
            .expect("the corpus report is a project");
        let mut checked = 0;
        for a in &scan.anchors {
            let body = std::fs::read_to_string(&a.path).unwrap_or_default();
            let line = body.lines().nth(a.line as usize).unwrap_or("");
            assert!(
                line.contains(&format!("{{#{}", a.id)),
                "`{}` is recorded at {}:{} , which reads {line:?}",
                a.id,
                a.path.display(),
                a.line + 1
            );
            checked += 1;
        }
        assert!(checked > 0, "no anchor in the report to check");
    }

    #[test]
    fn the_memo_re_walks_when_a_page_changes_on_disk() {
        // The one test that must not be vacuous: this memo is what the design chose INSTEAD
        // of a file watcher, so its invalidation is the whole risk.
        let root = fixture("invalidate");
        let probe = root.join("index.tmd");
        let mut cache = ProjectCache::new();
        assert!(
            !cache
                .get(&probe)
                .unwrap()
                .anchors
                .iter()
                .any(|a| a.id == "fig-late")
        );

        // Rewrite a page with a longer body, so both mtime and length differ.
        std::fs::write(
            root.join("ch/two.tmd"),
            "# Two {#sec-two}\n\n## Deeper\n\nSee @fig-one.\n\n![q](q.png){#fig-late}\n",
        )
        .unwrap();

        assert!(
            cache
                .get(&probe)
                .unwrap()
                .anchors
                .iter()
                .any(|a| a.id == "fig-late"),
            "the memo served a stale scan after a page changed on disk"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_new_page_appearing_invalidates_the_memo() {
        // Length and mtime of the EXISTING pages are unchanged here, so a memo keyed only on
        // the pages it already knew would miss this entirely.
        let root = fixture("newpage");
        let probe = root.join("index.tmd");
        let mut cache = ProjectCache::new();
        cache.get(&probe).unwrap();

        std::fs::write(root.join("three.tmd"), "# Three {#sec-three}\n").unwrap();
        assert!(
            cache
                .get(&probe)
                .unwrap()
                .anchors
                .iter()
                .any(|a| a.id == "sec-three"),
            "a page added to the project must invalidate the memo"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_unchanged_project_is_not_re_walked() {
        // The other half: if this never hits, the memo is decoration and every gesture pays
        // a full walk.
        let root = fixture("memo");
        let probe = root.join("index.tmd");
        let mut cache = ProjectCache::new();
        cache.get(&probe).unwrap();
        let first = cache.walks();
        cache.get(&probe).unwrap();
        assert_eq!(
            cache.walks(),
            first,
            "a second get on an unchanged project re-walked"
        );
        assert_eq!(first, 1, "the first get must actually have walked");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The SiteCache's whole reason to exist, and the one property no correctness test can
    /// see: this memo sits on the per-keystroke path, where a full `Site::discover` was
    /// measured at 188 ms against 14 ms for the entire rest of the lint. A cache that always
    /// missed would publish exactly the right diagnostics and make typing unusable.
    #[test]
    fn an_unchanged_project_is_not_re_discovered() {
        let root = fixture("site-memo");
        let probe = root.join("index.tmd");
        let mut cache = SiteCache::new();
        assert!(cache.get(&probe).is_some(), "the fixture is a project");
        assert_eq!(cache.builds(), 1, "the first get must actually discover");
        cache.get(&probe).unwrap();
        assert_eq!(
            cache.builds(),
            1,
            "a second get on an unchanged project re-discovered the whole site"
        );

        // And the other half, or the memo would serve a page registry that no longer
        // matches the tree: an external edit to a page the author is not typing in — which
        // is exactly what `didChangeWatchedFiles` now re-publishes for — must invalidate it.
        std::fs::write(root.join("ch/three.tmd"), "# Three {#sec-three}\n").unwrap();
        cache.get(&probe).unwrap();
        assert_eq!(
            cache.builds(),
            2,
            "a page added to the project must invalidate the site memo"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A document with no `_site.yml` above it has no project, and must be linted as the
    /// standalone document it is rather than borrowing an unrelated neighbour's registry.
    #[test]
    fn a_document_outside_any_project_has_no_site() {
        let dir = scratch("site-solo");
        let solo = dir.join("solo.tmd");
        std::fs::write(&solo, "# Solo\n").unwrap();
        let mut cache = SiteCache::new();
        assert!(cache.get(&solo).is_none());
        assert_eq!(cache.builds(), 0, "nothing to discover, nothing discovered");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
