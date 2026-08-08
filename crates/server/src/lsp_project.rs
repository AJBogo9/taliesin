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
    pub path: PathBuf,
    /// 0-based line of the defining site.
    pub line: u32,
    /// The heading's text when the anchor sits on one; empty otherwise.
    pub title: String,
    /// The rendered section number for a numbered chapter heading; empty otherwise.
    pub number: String,
}

/// One heading on one page, for workspace symbols and the whole-book outline.
pub(crate) struct ProjectHeading {
    pub path: PathBuf,
    /// 0-based line of the heading.
    pub line: u32,
    pub level: u8,
    pub text: String,
}

/// One `@`-sigil reference, resolved or not. A use whose `id` matches no [`ProjectAnchor`] is
/// dangling, which is exactly what the References view groups.
pub(crate) struct ProjectUse {
    pub id: String,
    pub path: PathBuf,
    /// 0-based line and scalar column.
    pub line: u32,
    pub col: u32,
}

/// One walk's result.
pub(crate) struct ProjectScan {
    pub root: PathBuf,
    pub anchors: Vec<ProjectAnchor>,
    pub headings: Vec<ProjectHeading>,
    pub uses: Vec<ProjectUse>,
    /// The project's declared reading order — `_site.yml`'s top-level `chapters:` (the native
    /// schema is flat; `book: chapters:` is how it is written ABOUT, not in) resolved into page
    /// paths, parts flattened away. **Empty for a website**, which declares no order at all;
    /// that is the difference between "not in the list" and "there is no list", and the
    /// outline needs both. Drafts are included: you are editing them.
    pub reading_order: Vec<PathBuf>,
}

/// What a page looked like when it was last walked: enough to notice an edit without
/// watching the filesystem.
type Stamp = (PathBuf, Option<std::time::SystemTime>, u64);

/// The stat-validated memo described in the module docs, keyed by project root.
///
/// Keyed rather than single-entry because an editor routinely has files from more than one
/// project open at once (a chapter of the guide beside a corpus document), and workspace
/// symbols searches **every** open project. A single entry would re-walk both on every
/// keystroke of a Ctrl+T query, turning the memo into pure overhead exactly when it matters.
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

    /// The distinct project roots among `pages`, in a stable order.
    ///
    /// Workspace symbols needs this because the `workspace/symbol` request names **no file**:
    /// picking one open document to stand for "the project" silently searches whichever
    /// document happened to be first, which with two projects open is a coin flip. Answering
    /// for every open project is the only behaviour an author can predict.
    pub(crate) fn roots_of<'a>(pages: impl Iterator<Item = &'a Path>) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = pages
            .filter_map(|p| {
                p.parent()
                    .and_then(taliesin_core::site::enclosing_site_root_across_git)
            })
            .collect();
        roots.sort();
        roots.dedup();
        roots
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
    /// (`check::collect_file_diagnostics_in_site`), so that one place decides it: a deck and
    /// a `draft: true` chapter are both inside a project and are both linted standalone.
    ///
    /// `DraftMode::Exclude` matches `check`, which is the parity this whole path claims.
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

/// Every page of the project rooted at `root`, plus one number standing for the state of
/// every input its diagnostics depend on.
///
/// The caller is `workspace/diagnostic`, which has to name the pages **nobody has opened** —
/// that is the whole point of the pull model — and so cannot get them from the open-buffer
/// store. It is answered on a client poll (`vscode-languageclient` re-asks every 2 s), so the
/// two halves come back from **one** walk: asking for the pages and the stamp separately read
/// the directory tree twice per poll for no gain.
///
/// The stamp is deliberately **project-wide** rather than per-page. A cross-page `@sec-` makes
/// every page's answer depend on every other page and on `_site.yml`, so a per-page stamp would
/// answer `Unchanged` for chapter 12 after chapter 3 renamed the heading it links to. It is the
/// same `(path, mtime, len)` data [`stamps_for`] validates the memos with, so the report and
/// the caches cannot disagree about what counts as a change.
pub(crate) fn pages_and_stamp(root: &Path) -> (Vec<PathBuf>, u64) {
    let stamps = stamps_for(root);
    let mut acc = String::new();
    for (path, mtime, len) in &stamps {
        let nanos = mtime
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        acc.push_str(&format!("{}:{len}:{nanos}\n", path.display()));
    }
    // `stamps_for` puts `_site.yml` first and the pages after it; only the pages are pages.
    let pages = stamps.into_iter().skip(1).map(|(p, _, _)| p).collect();
    (pages, taliesin_core::hash::fnv1a(&acc))
}

/// `(path, mtime, len)` for every page, in `collect_pages` order so two runs compare equal.
///
/// `_site.yml` is stamped alongside the pages because the scan now carries the book's reading
/// order, which lives in that file and in no page. Without it, moving a chapter in `chapters:`
/// would leave the outline showing the old order until some unrelated page happened to change.
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

/// Read every page once and collect all three views of it. Includes are resolved first, so
/// an anchor authored in an `_includes/` partial belongs to whichever page includes it,
/// exactly as the render pipeline and `anchors_defined_elsewhere_in_project` both do.
fn walk(root: &Path) -> ProjectScan {
    let mut inputs = Vec::new();
    taliesin_core::site::collect_pages(root, &mut inputs);
    // The reading order comes from the one place that already resolves it — the same `Book`
    // the drawer and prev/next are built from — rather than a second reading of `chapters:`
    // that could disagree about nested parts or label overrides. `Include` because a draft
    // chapter is one you are in the middle of writing, and dropping it from the outline hides
    // exactly the page you have open.
    let reading_order = taliesin_core::Site::discover_with(root, taliesin_core::DraftMode::Include)
        .book
        .map(|b| {
            b.entries
                .iter()
                .filter(|e| e.part.is_none() && !e.url.is_empty())
                .map(|e| root.join(&e.rel))
                .collect()
        })
        .unwrap_or_default();
    let mut anchors = Vec::new();
    let mut headings = Vec::new();
    let mut uses = Vec::new();
    let mut seen: HashMap<String, ()> = HashMap::new();

    for input in inputs {
        let Ok(raw) = std::fs::read_to_string(&input) else {
            continue;
        };
        let base = input.parent().unwrap_or_else(|| Path::new("."));
        let (src, _) = taliesin_core::includes::resolve(&raw, base);

        for a in taliesin_core::site::scan_page_anchors(&src, None) {
            // First definition wins project-wide, matching `scan_xref_targets`. Two owners of
            // "which page defines `fig-x`" that disagreed would send F12 somewhere the built
            // page does not link to.
            if seen.insert(a.id.clone(), ()).is_none() {
                anchors.push(ProjectAnchor {
                    id: a.id,
                    path: input.clone(),
                    // `scan_page_anchors` reports a 1-based line; everything on the LSP wire
                    // is 0-based.
                    line: a.line.saturating_sub(1) as u32,
                    title: a.title,
                    number: a.number,
                });
            }
        }
        for s in crate::lsp_outline::sections(&src) {
            headings.push(ProjectHeading {
                path: input.clone(),
                line: s.start_line as u32,
                level: s.level,
                text: s.title,
            });
        }
        for (id, line, col) in crate::lsp_nav::xref_occurrences(&src) {
            uses.push(ProjectUse {
                id,
                path: input.clone(),
                line,
                col,
            });
        }
    }
    ProjectScan {
        root: root.to_path_buf(),
        anchors,
        headings,
        uses,
        reading_order,
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
    fn the_walk_collects_anchors_headings_and_uses_across_every_page() {
        let root = fixture("collect");
        let mut cache = ProjectCache::new();
        let scan = cache.get(&root.join("index.tmd")).unwrap();

        let mut ids: Vec<&str> = scan.anchors.iter().map(|a| a.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["fig-one", "sec-two"], "anchors from BOTH pages");

        assert!(
            scan.headings
                .iter()
                .any(|h| h.text == "Deeper" && h.level == 2),
            "headings come from every page, not just the one asked about"
        );

        let mut used: Vec<&str> = scan.uses.iter().map(|u| u.id.as_str()).collect();
        used.sort_unstable();
        assert_eq!(used, vec!["fig-gone", "fig-one", "sec-two"]);
        assert!(
            scan.uses.iter().any(|u| u.id == "fig-gone"),
            "a dangling use is collected, not dropped: the References view groups them"
        );
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
        assert_eq!(a.title, "Two");

        // A definition that is NOT on line 1, so an off-by-one cannot pass by coincidence.
        let f = scan.anchors.iter().find(|a| a.id == "fig-one").unwrap();
        assert_eq!(
            f.line, 2,
            "`{{#fig-one}}` sits on the third line of index.tmd"
        );
        let _ = std::fs::remove_dir_all(&root);
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
