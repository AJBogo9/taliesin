//! The multi-page **site** dev server: a live preview of a whole website.
//!
//! It generalises the single-document [`crate::serve`] server to a project:
//!
//!   - the URL selects which page to render (navigation between pages is just a
//!     full page load, so navbar / prev-next links work with no SPA),
//!   - each page has its own block state, broadcast channel, and code executor,
//!     built lazily on first visit,
//!   - a save rebuilds only the affected page(s) and hot-reloads them in place;
//!     a `_site.yml` change re-discovers the site and reloads open tabs.
//!
//! Small HTTP/asset helpers + the embedded client are shared with [`crate::serve`].

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use notify::Watcher;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use taliesin_core::{Block, BlockOp, Page, Site, diff_blocks, needs_remount};
use tokio::sync::{broadcast, mpsc};

use crate::lint::{Diagnostic, diag_from};
use crate::protocol;
use crate::serve::{
    CLIENT_JS, FAVICON, STATUS_CSS, bind_with_fallback, js_str, open_in_browser, percent_decode,
    with_host_guard, with_identity, ws_origin_ok,
};

mod exec_pool;
use exec_pool::ExecPool;

/// The whole live site: one project, served through the per-page live path. One builder
/// task + one file watcher drive it.
struct SiteApp {
    /// The project being served.
    root: Arc<Project>,
    /// Page rel-paths queued for a (re)build by the executor worker.
    build_tx: mpsc::UnboundedSender<BuildMsg>,
    /// The bypass lane for pages that need no kernel (AP3-1). See [`SiteApp::queue_build`].
    fast_tx: mpsc::UnboundedSender<BuildMsg>,
    /// The OS pid of the code cell executing right now, or 0 when none is.
    ///
    /// Written by the executors the builder's [`ExecPool`] hands out; read here, on the
    /// websocket task. It exists because those two tasks are not the same task: "Restart
    /// kernel" arrives while the builder is blocked awaiting the very build it means to
    /// abort, and the builder is serial, so queueing alone can never reach a running cell
    /// (audit finding 01). What is sent is [`restart_stop`]'s call: SIGKILL for the
    /// requester's own kernel, SIGINT for a cell of another page.
    ///
    /// It is one pid for the whole pool, so the cell it names may belong to a page other
    /// than the one asking. That is deliberate and cannot be narrowed; what the page that
    /// loses the cell is told about it is [`ExecLane`]'s job (A17).
    interrupt: Arc<std::sync::atomic::AtomicU32>,
}

impl SiteApp {
    /// Queue a page rebuild on the lane that fits it (AP3-1).
    ///
    /// **The defect.** One builder task consumed the whole server's build queue, awaiting
    /// each page to completion. It serialized on the wrong predicate: a page with **no code
    /// cells** needs no kernel, yet it queued behind kernel work it would never use.
    /// Measured on a two-page preview, a cell-free page's trivial prose edit landed in
    /// **0.11 s** alone and **12.15 s** (110x) when an unrelated page was 1.2 s into a 12 s
    /// `{python}` cell.
    ///
    /// **Why not just parallelise the builder.** Serialization is what makes the
    /// task-owned `ExecPool` race-free, and `ExecPool` is under the M6a freeze. So there
    /// are two *serial* lanes, not concurrent executors: the exec lane owns the pool and is
    /// unchanged, and the fast lane owns nothing and never touches it. Neither lane gains
    /// any concurrency of its own.
    ///
    /// **Routing, and why it cannot race.** A page's lane is decided by what its LAST
    /// completed build found (`PageDoc::needs_kernel`, which starts `true` so an unbuilt
    /// page takes the safe lane). That flag is written only at the end of a build, so
    /// while a build of page P is in flight the flag still holds the value that routed it,
    /// and every queued message for P routes to the same lane. Both lanes being serial,
    /// P's builds stay totally ordered and the two lanes can never build P at once.
    ///
    /// The one cost is the edit that adds a page's *first* code cell: it routes to the fast
    /// lane, which renders, discovers cells, and hands the message to the exec lane —
    /// one wasted render, once, and the flag is right from then on.
    fn queue_build(&self, rel: String) {
        let cell_free = self
            .root
            .pages
            .lock()
            .get(&rel)
            .map(|ps| ps.doc.cell_free)
            .unwrap_or(false);
        let tx = if cell_free {
            &self.fast_tx
        } else {
            &self.build_tx
        };
        let _ = tx.send(BuildMsg::Build(rel));
    }

    /// Queue [`BuildMsg::IfMoved`] on the lane that fits the page, as [`queue_build`]
    /// routes. On the exec lane it waits behind the page's own build in flight, so it is
    /// judged against what that build left.
    ///
    /// [`queue_build`]: SiteApp::queue_build
    fn queue_if_moved(&self, rel: String, paths: Vec<PathBuf>) {
        let cell_free = self
            .root
            .pages
            .lock()
            .get(&rel)
            .map(|ps| ps.doc.cell_free)
            .unwrap_or(false);
        let tx = if cell_free {
            &self.fast_tx
        } else {
            &self.build_tx
        };
        let _ = tx.send(BuildMsg::IfMoved(rel, paths));
    }
}

/// The served project. Owns the live state the builder and router act on: the discovered
/// [`Site`], plus the live per-page block state + broadcast channels, created lazily on
/// first visit.
struct Project {
    dir: PathBuf,
    site: Mutex<Site>,
    pages: Mutex<HashMap<String, PageState>>,
    /// Who the serial exec lane is running cells for, and who lost a cell to someone
    /// else's kernel restart. See [`ExecLane`].
    exec_lane: Mutex<ExecLane>,
    /// Set when this project is one document previewed on its own (`preview <file.tmd>`
    /// with no ancestor `_site.yml`): the document it is scoped to.
    ///
    /// Load-bearing on **re-discovery**, not just at boot. A save that touches `_site.yml`
    /// or creates a `.tmd` re-runs discovery, and an unscoped re-run would quietly widen a
    /// one-document preview into "every `.tmd` in the parent directory" — the scoping would
    /// hold until the first save and then evaporate.
    scope: Option<PathBuf>,
    /// What discovery read of each source file the preview has asked it about, as of the
    /// discovery in force: a page's input maps to its
    /// [`discovery_digest`](taliesin_core::site::discovery_digest) (its front-matter block
    /// and its leading `# H1`), and a source file discovery found is no page (a partial, a
    /// book file `chapters:` does not list) maps to `None`.
    ///
    /// Front matter and the H1 are discovery input, not only render input. They are parsed
    /// once into [`Site::pages`] and the book's chapters, and every OTHER page renders its
    /// listing cards, nav labels, prev/next, drawer and chapter numbers out of that copy, so
    /// a save that only re-renders the edited page leaves all of them stale.
    ///
    /// This is what classifies a save, by what it changed rather than by how the editor
    /// wrote it ([`Project::what_moved`]): a page whose digest held is an edit in place
    /// whether the editor wrote the file in place or renamed a temp file over it. Deciding
    /// by event kind made every atomic save a possible page-set change (audit 2026-09-24
    /// C1, C9): it paid a whole re-discovery, and that re-discovery reseeded this record
    /// before anything asked whether the front matter moved, so an atomic `title:` edit
    /// never reached the listing that shows it.
    ///
    /// A digest rather than a re-discovery per save: discovery renders every page twice
    /// (the cross-reference harvest and the search index), and a body edit, nearly every
    /// save, leaves the digest untouched.
    records: Mutex<HashMap<PathBuf, Option<u64>>>,
}

/// What a batch of changed files moved of what discovery reads ([`Project::what_moved`]).
#[derive(Default)]
struct Moved {
    /// A page's source is gone, or a source file discovery never classified exists: the
    /// page set may have changed.
    page_set: bool,
    /// What discovery reads of a page that is still there changed.
    records: bool,
    /// The digest of every changed source file that exists, read BEFORE the re-discovery
    /// this batch may cause, so a save that lands during it is compared against what this
    /// batch saw rather than silently recorded as already seen.
    digests: HashMap<PathBuf, u64>,
}

/// The key a path is recorded under: its canonical form when it exists, else the path as
/// given (a deleted file does not canonicalize, and the watcher reports the path it had).
fn record_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// The [`discovery_digest`](taliesin_core::site::discovery_digest) of the source at `path`,
/// read as discovery reads it. An unreadable file digests as an empty one, which is what
/// discovery makes of it.
fn digest_of(path: &Path) -> u64 {
    let src = taliesin_core::includes::read_source(path).unwrap_or_default();
    taliesin_core::site::discovery_digest(&src)
}

impl Project {
    /// Re-discover this project the same way it was discovered, scope included.
    fn rediscover(&self) -> Site {
        match &self.scope {
            Some(file) => Site::discover_document(file),
            None => Site::discover_with(&self.dir, taliesin_core::DraftMode::Include),
        }
    }

    /// Record what discovery read of every page of the `Site` the server booted with.
    fn seed_records(&self) {
        let inputs: Vec<PathBuf> = self
            .site
            .lock()
            .pages
            .iter()
            .map(|p| record_key(&p.input))
            .collect();
        let mut records = self.records.lock();
        records.clear();
        for input in inputs {
            let digest = digest_of(&input);
            records.insert(input, Some(digest));
        }
    }

    /// What `changed` moved of what discovery reads. Reads each changed source file once;
    /// anything else (an `{{< include >}}` partial's `.md`, a `.bib`, an image) is no page
    /// and moves nothing here, unless it was a directory pages lived in.
    fn what_moved(&self, changed: &HashSet<PathBuf>) -> Moved {
        let records = self.records.lock();
        let mut moved = Moved::default();
        for path in changed {
            let key = record_key(path);
            if !taliesin_core::ext::is_source_path(path) {
                // A directory renamed away or deleted takes every page under it along. (One
                // renamed IN is replayed file by file by the watcher.)
                moved.page_set |= !key.exists()
                    && records
                        .iter()
                        .any(|(page, digest)| digest.is_some() && page.starts_with(&key));
                continue;
            }
            let exists = key.is_file();
            let digest = exists.then(|| digest_of(&key));
            if let Some(d) = digest {
                moved.digests.insert(key.clone(), d);
            }
            match (records.get(&key), digest) {
                // A page whose source is gone.
                (Some(Some(_)), None) => moved.page_set = true,
                (Some(Some(recorded)), Some(d)) => moved.records |= d != *recorded,
                // Discovery found this is no page, and no content can make it one: that is
                // decided by where the file is and by `chapters:`.
                (Some(None), _) => {}
                // A source file discovery has never classified may be a new page.
                (None, digest) => moved.page_set |= digest.is_some(),
            }
        }
        moved
    }

    /// Adopt a freshly discovered `site` and bring the record up to date with it. A page
    /// takes the digest `fresh` read before the discovery, or keeps the one it has; a page
    /// new to the record is read now; a changed source file that is no page is recorded as
    /// none. `forget_non_pages` drops every recorded "no page", for a `_site.yml` change,
    /// whose `chapters:` can make any file a page.
    fn adopt(&self, site: Site, fresh: &HashMap<PathBuf, u64>, forget_non_pages: bool) {
        let pages: HashSet<PathBuf> = site.pages.iter().map(|p| record_key(&p.input)).collect();
        *self.site.lock() = site;
        let mut records = self.records.lock();
        records.retain(|key, digest| match digest {
            Some(_) => pages.contains(key),
            None => !forget_non_pages && !pages.contains(key),
        });
        for key in &pages {
            match fresh.get(key) {
                Some(d) => {
                    records.insert(key.clone(), Some(*d));
                }
                None if !matches!(records.get(key), Some(Some(_))) => {
                    records.insert(key.clone(), Some(digest_of(key)));
                }
                None => {}
            }
        }
        for key in fresh.keys().filter(|key| !pages.contains(*key)) {
            records.insert(key.clone(), None);
        }
    }
}

/// The project source `rel` a client's `?page=` sub-key names, or `None` when it names no
/// page in this project.
///
/// The `None` case is load-bearing: the ws handler refuses such a connection instead of
/// creating a `PageState` for it. A `PageState` is a 256-slot broadcast ring that only a
/// save finding no tab on it evicts ([`watched_pages`]), so allocating one per unrecognized
/// key let any peer that can reach the socket
/// grow the map without bound by reconnecting with fresh garbage. Nothing is lost by
/// refusing: `build_page` already returns immediately for a key `Site::page` cannot resolve,
/// so the entry could only ever hold an empty document.
fn resolve_page_rel(project: &Project, sub: &str) -> Option<String> {
    project.site.lock().page(sub).map(|p| p.rel.clone())
}

/// What the serial exec lane is doing, as the websocket task needs to see it.
///
/// **Why it exists (A17).** [`SiteApp::interrupt`] is one pool-wide pid, so the
/// `restart_kernel` arm SIGINTs whatever cell is executing *anywhere* in the project. That
/// is deliberate: the exec lane is serial, so a page's own Restart is queued behind the
/// runaway build it is meant to abort, and the server-wide SIGINT is the only thing that
/// can reach it. Scoping the interrupt to the requesting page would restore that wedge for
/// every page except the one that owns the runaway cell, so it is not the fix.
///
/// What was wrong was the silence. The page that lost its cell was left holding a
/// `KeyboardInterrupt` traceback with nothing, anywhere, saying where it came from. So the
/// lane publishes whose cell is running, and a restart that takes someone else's says so
/// on that page.
///
/// The victim is deliberately **not** re-queued: its cell would simply run again and, if it
/// is the runaway that made the restart necessary, wedge the lane again. Re-running it is
/// the author's call, and the notice says how.
#[derive(Default)]
struct ExecLane {
    /// The page the exec builder is running cells for, empty when the lane is idle.
    page: String,
    /// `(victim, requester)`: a page whose running cell was SIGINTed so another page's
    /// kernel restart could go through, and the page that asked for that restart.
    interrupted_by: Option<(String, String)>,
}

impl ExecLane {
    /// The page whose kernel restart took `rel`'s running cell, if that is what happened,
    /// clearing it so it is reported once. Consumed by the victim's in-flight build, which
    /// is the build that shows the traceback.
    fn take_interrupt_for(&mut self, rel: &str) -> Option<String> {
        if self.interrupted_by.as_ref().is_some_and(|(v, _)| v == rel) {
            return self.interrupted_by.take().map(|(_, by)| by);
        }
        None
    }
}

/// The page a `restart_kernel` from `requester` is about to take a running cell from, when
/// that page is not the requester itself. `None` for the ordinary cases: nothing is
/// executing (`pid` 0), the lane is idle, or the requester's own cell is the one running —
/// aborting that is precisely what restarting your kernel means.
fn cross_page_victim(requester: &str, running: &str, pid: u32) -> Option<String> {
    (pid != 0 && !running.is_empty() && running != requester).then(|| running.to_string())
}

/// How "Restart kernel" stops the cell executing right now (`pid`), if one is.
#[derive(Debug, PartialEq, Eq)]
enum Stop {
    /// SIGKILL the kernel: it is the requester's own, about to be discarded anyway.
    Kill,
    /// SIGINT the cell: it belongs to another page (A17), whose kernel is not being
    /// discarded, so only its running cell is given up.
    Interrupt,
}

/// Which [`Stop`] a restart from the page whose cross-page victim is `victim` applies to
/// the running `pid`, or `None` when nothing is executing.
fn restart_stop(pid: u32, victim: Option<&str>) -> Option<Stop> {
    (pid != 0).then_some(match victim {
        None => Stop::Kill,
        Some(_) => Stop::Interrupt,
    })
}

/// What the page that lost a cell to `by`'s kernel restart says about it, next to the
/// traceback the interrupt left behind.
fn interrupted_notice(by: &str) -> String {
    format!(
        "a cell here was interrupted so the kernel restart requested on {by} could go \
         through — the dev server runs one page's cells at a time. Edit this page, or \
         restart its kernel, to run it again."
    )
}

/// A job for the executor worker: rebuild a page, restart its kernel first (the dev-menu
/// "Restart kernel" action) then rebuild, or rebuild it if one of the files it only looked
/// at is not as its last build left it ([`probes_moved`]).
enum BuildMsg {
    Build(String),
    Restart(String),
    IfMoved(String, Vec<PathBuf>),
}

struct PageState {
    doc: PageDoc,
    tx: broadcast::Sender<String>,
}

/// The live block state of one page (mirrors `serve::DocState`, per page).
#[derive(Default)]
struct PageDoc {
    /// The display-ready `<title>`: this page's resolved title (front matter, else its
    /// leading `# H1`) plus the site-name suffix, via `Site::page_title`. Empty before this
    /// page's first render, and for a page that resolves to no title at all (no corpus
    /// document does; the suffix is never applied to an empty title).
    ///
    /// Resolved HERE, by the producer, because it has two consumers that must not be able
    /// to disagree: the server-rendered `<title>` and every `full_render`, which the client
    /// assigns straight to `document.title`. This field used to hold the *raw* front-matter
    /// title and let each consumer finish the job; only one of them did, so the websocket
    /// clobbered a correct tab with a worse one on arrival. There is deliberately no raw
    /// title beside this: nothing in the live server wants one, and a second, subtly
    /// different title in reach is how the first one drifted.
    tab_title: String,
    toc: bool,
    /// The chrome's own markup for this page (SEO meta, feed links, the draft banner).
    /// No longer anything the AUTHOR wrote: the front-matter `include-*`/`css` family went
    /// on 2026-08-02 and `_site.yml`'s `head:` on 2026-08-18, so nothing merges here.
    includes: taliesin_core::render::PageIncludes,
    blocks: Vec<Block>,
    diagnostics: Vec<Diagnostic>,
    errored: bool,
    /// Monotonic body-render generation, bumped whenever this page's `blocks`
    /// change. Stamped into the page's SSR script (`window.TALIESIN_SSR_GEN`) and
    /// every `full_render`, so the client can tell a still-current SSR body from one
    /// the initial exec pass made stale before the websocket connected. Mirrors
    /// `serve::DocState::generation`; see [`protocol::full_render`].
    generation: u64,
    /// Whether this page's LAST completed build found no kernel-executing cell, so its
    /// next rebuild can take the bypass lane (AP3-1). See [`SiteApp::queue_build`] for why
    /// this is read from the last build rather than the current source, and why that cannot
    /// race. Deliberately `false` by default: an unbuilt page takes the safe lane.
    cell_free: bool,
    /// Every file this page's last render read or looked for, recorded by the read sites
    /// themselves ([`taliesin_core::reads`]) and keyed as [`record_key`] keys them: its
    /// source, each `{{< include >}}` it tried, each `.bib` it loaded (the project's shared
    /// one too), each image it measured or checked, each page a link of its points at. A
    /// change to any of them rebuilds the page ([`rebuild_project`]).
    reads: taliesin_core::reads::Reads,
    /// Each file this page's last build only looked at ([`Access::Probed`]: an image, a
    /// linked file), as that build left it: its [`stamp`] once the page's cells had run.
    ///
    /// [`Access::Probed`]: taliesin_core::reads::Access::Probed
    stamps: HashMap<PathBuf, Option<Stamp>>,
}

/// What a file looked like: its length and modification time.
type Stamp = (u64, std::time::SystemTime);

/// The [`Stamp`] of the file at `path`, `None` when there is none.
fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}

/// Whether one of `paths`, files page `rel` only looked at, is not as its last build left
/// it ([`PageDoc::stamps`]).
///
/// Asked instead of rebuilding outright, because such a file can be the page's own
/// output: a cell writes `gen.png` for the `![…](gen.png)` below it. Rebuilding the page
/// for that write ran its cells again, and a `#| cache: false` cell, which re-runs on
/// every build, wrote the file again: measured, 77 runs in 8 s of an idle preview. The
/// write lands before the build that made it ends, so that build's stamp already holds
/// it, and asking after the build is what tells the page's own write from a later one.
fn probes_moved(project: &Project, rel: &str, paths: &[PathBuf]) -> bool {
    let pages = project.pages.lock();
    let Some(ps) = pages.get(rel) else {
        return false;
    };
    paths
        .iter()
        .any(|p| ps.doc.stamps.get(p).is_none_or(|was| *was != stamp(p)))
}

impl PageDoc {
    fn body_html(&self) -> String {
        // Sized up front: a page body is ~290 KB of small blocks, so growing from empty
        // is ~19 reallocations and a copy of everything written so far each time.
        let mut s = String::with_capacity(self.blocks.iter().map(|b| b.html.len() + 1).sum());
        for b in &self.blocks {
            s.push_str(&b.html);
            s.push('\n');
        }
        s
    }
}

/// What `taliesin preview` was pointed at.
///
/// Both arms are served by this one server. There is no second, single-document server:
/// a `.tmd` is previewed as the project it belongs to, which is what makes its nav,
/// breadcrumbs and cross-page links work (and is what the VS Code companion has always
/// done — see `editor/vscode/src/extension.ts`, item 150).
pub enum Target {
    /// A directory: the whole project.
    Project(PathBuf),
    /// One `.tmd`: its enclosing `_site.yml` project opened at that page, or — with no
    /// ancestor `_site.yml` — a project of just that document.
    Document(PathBuf),
}

impl Target {
    /// A directory is a project; anything else is a document.
    pub fn at(path: PathBuf) -> Target {
        if path.is_dir() {
            Target::Project(path)
        } else {
            Target::Document(path)
        }
    }
}

/// Resolve a [`Target`] to the project to serve: its root, its discovered [`Site`], and
/// the document it is scoped to (`None` for a whole-project target).
///
/// A document inside a project is served as **that project**, not alone. Previewing the
/// file by itself produces an orphan — no nav, no breadcrumb, every cross-page link dead —
/// and the enclosing project is a fact about the tree (the nearest `_site.yml`), so there
/// is nothing here for the author to configure. Only a document with no ancestor
/// `_site.yml` gets a project of its own, scoped to it.
/// Returns `(root, site, scoped, doc)`. `scoped` is `Some` only for an out-of-project
/// document — it is what re-discovery must stay narrowed to. `doc` is the target document
/// in either case, which is what the browser opens at.
fn resolve_target(target: Target) -> std::io::Result<Resolved> {
    let (root, scope) = match target {
        Target::Project(dir) => (dir, None),
        Target::Document(file) => {
            // Keep the as-typed spelling for the refusal below, before canonicalizing:
            // `build` echoes the path exactly as the author typed it, and the two verbs
            // must answer the same refusal with the same-looking path.
            let typed = file.clone();
            // A missing document gets the one "cannot read" message every front door prints,
            // with its did-you-mean for a near-miss sibling (`build` answers the same typo
            // the same way).
            if let Err(e) = file.canonicalize() {
                return Err(std::io::Error::new(
                    e.kind(),
                    crate::lint::cannot_read(&typed, &e),
                ));
            }
            // Named by its canonical FOLDER, not resolved through a symlink: a linked page
            // belongs to the project its link sits in, where the site build publishes it.
            let file = taliesin_core::site::document_path(&file);
            if !file.is_file() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("no document at {}", file.display()),
                ));
            }
            // A document is a source document iff its extension is accepted
            // (`taliesin_core::ext::is_source_path`, the same vocabulary the site
            // walker discovers by): previewing a `note.md` would show a page no
            // `build <dir>` ever writes.
            if !taliesin_core::ext::is_source_path(&typed) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    crate::serve::not_a_source_error(&typed, "preview"),
                ));
            }
            let dir = file
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();
            match taliesin_core::site::enclosing_site_root(&dir) {
                // In a project: serve the project, open at this page.
                Some(root) => (root, Some(file)),
                // Not in a project: a project of exactly this document, rooted at its
                // directory so relative images/includes/assets resolve as they always did.
                None => (dir, Some(file)),
            }
        }
    };
    // Keep the as-typed root for the guard's error message, before it is made absolute:
    // `build` never canonicalizes the path it echoes back, so canonicalizing here first
    // would have the two verbs answer the same "not a project" question with two
    // different-looking paths for the same directory, and an absolute path is just noise
    // in a terminal for something the author typed relatively.
    let shown = root.clone();
    let root = root.canonicalize().unwrap_or(root);
    // A directory target is a project; a project is what `_site.yml` declares. Refuse before
    // binding a port, so the author gets the fix instead of a 404 page at `/` whose only link
    // points back at itself and which mounts neither the live client nor the dev menu.
    if scope.is_none() && !root.join("_site.yml").is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            crate::serve::not_a_project_error(&shown, "preview"),
        ));
    }
    // Only an OUT-of-project document narrows discovery. A document inside a project must
    // discover the whole project, or its nav and cross-page links would be the very orphan
    // this routing exists to prevent.
    let scoped = scope
        .as_deref()
        .filter(|f| {
            f.parent()
                .and_then(taliesin_core::site::enclosing_site_root)
                .is_none()
        })
        .map(|f| f.to_path_buf());
    let site = match &scoped {
        Some(file) => Site::discover_document(file),
        None => Site::discover_with(&root, taliesin_core::DraftMode::Include),
    };
    Ok(Resolved {
        root,
        site,
        scoped,
        doc: scope,
    })
}

impl Resolved {
    /// How this server is known to **other processes**: the root it answers
    /// [`crate::serve::IDENTITY_PATH`] with, and the incumbent it recognizes as itself.
    ///
    /// It is *what this server serves*, which for an out-of-project document is that
    /// document — it is a project of just that document — and **not** [`Resolved::root`],
    /// the directory the document happens to sit in. The two genuinely differ: that
    /// directory may hold unrelated `.tmd` files this server discovered nothing about and
    /// would 404, so answering with it claims pages that are not there.
    ///
    /// Publishing the directory once broke `taliesin run`, which looked a session up by the
    /// document and found nothing when the single-document server was folded in here. That
    /// verb went in Wave 13; the rule it exposed did not, because the incumbent check has the
    /// same shape: two previews of two unrelated loose documents in one directory must not
    /// recognize each other as the same server.
    ///
    /// `root` stays the filesystem base for serving assets and resolving includes; only
    /// the identity moves.
    fn session_key(&self) -> PathBuf {
        self.scoped.clone().unwrap_or_else(|| self.root.clone())
    }

    /// The warning for a document target its project does not publish, or `None`. Such a
    /// document (a partial, or a book chapter `chapters:` leaves out) has no page, so the
    /// preview opens the project's home page instead. It did that silently, leaving the
    /// author to find the document's own URL answering 404.
    fn unpublished_doc_warning(&self) -> Option<String> {
        let doc = self.doc.as_deref()?;
        if focus_url(&self.site, doc).is_some() {
            return None;
        }
        let shown = doc.strip_prefix(&self.root).unwrap_or(doc);
        Some(format!(
            "{} is not a page of this project (a partial, or a chapter `chapters:` does not \
             list), so the preview opens at the home page",
            shown.display()
        ))
    }
}

/// What [`resolve_target`] worked out about the thing being previewed.
#[derive(Debug)]
struct Resolved {
    root: PathBuf,
    site: Site,
    /// The document discovery is narrowed to, for an out-of-project single document.
    /// Carried onto [`Project::scope`] so a re-discovery cannot silently widen it.
    scoped: Option<PathBuf>,
    /// The document the browser should open at, in project and single-document cases alike.
    doc: Option<PathBuf>,
}

/// The URL a scoped document lives at. Used both to open the browser at it and to answer
/// the project root with it.
fn focus_url(site: &Site, file: &std::path::Path) -> Option<String> {
    let canon = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let file = canon(file);
    let same = |p: &std::path::Path| canon(p) == file;
    site.pages
        .iter()
        .find(|p| same(&p.input))
        .map(|p| p.url.clone())
}

/// Entry point for `taliesin preview <dir|file.tmd>`.
pub fn run(target: Target, port: u16, open: bool) -> std::io::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(serve(target, port, open));
    // `serve` returns on a shutdown signal (see `crate::serve::shutdown_signal`);
    // force the runtime down so the builder task that owns the kernels is dropped
    // promptly, running its teardown (the kernel SIGKILLs). Bounded so a wedged task
    // can't hang exit; the kills are synchronous.
    rt.shutdown_timeout(std::time::Duration::from_secs(5));
    result
}

async fn serve(target: Target, port: u16, open: bool) -> std::io::Result<()> {
    let start = std::time::Instant::now();
    // Preview shows drafts inline (nav/listings/prev-next, badged); build/publish exclude
    // them.
    let resolved = resolve_target(target)?;
    let session_key = resolved.session_key();
    let unpublished = resolved.unpublished_doc_warning();
    let Resolved {
        root,
        site,
        scoped,
        doc,
    } = resolved;
    // Where to point the browser: a document target opens at its own page rather than at
    // the project's home, so `preview chapter-7.tmd` shows chapter 7.
    let focus = doc.as_deref().and_then(|f| focus_url(&site, f));
    // Printed after the banner, below: the banner opens with a screen clear.
    let startup: Vec<taliesin_core::render::Warning> = site.warnings.clone();
    let page_count = site.pages.len();
    // A project with nothing to serve: `build <dir> --check-only` already exits 1 here,
    // while `preview` used to bind a port, 404 `/`, and boot a kernel for nothing. The two
    // front doors must agree.
    if page_count == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no .tmd pages found under {}", root.display()),
        ));
    }
    let (build_tx, build_rx) = mpsc::unbounded_channel();
    let (fast_tx, fast_rx) = mpsc::unbounded_channel();
    let app = Arc::new(SiteApp {
        root: Arc::new(Project {
            dir: root.clone(),
            site: Mutex::new(site),
            pages: Mutex::new(HashMap::new()),
            exec_lane: Mutex::new(ExecLane::default()),
            scope: scoped,
            records: Mutex::new(HashMap::new()),
        }),
        build_tx,
        fast_tx,
        interrupt: Arc::new(std::sync::atomic::AtomicU32::new(0)),
    });
    // Before the watcher can fire: the record has to describe the discovery the server
    // booted with, or the first front-matter edit of the session reads as "unchanged".
    app.root.seed_records();

    spawn_builder(app.clone(), build_rx);
    spawn_fast_builder(app.clone(), fast_rx);
    spawn_watcher(app.clone());

    let router = Router::new()
        .route("/favicon.ico", get(favicon))
        .route(taliesin_core::PREVIEW_MERMAID_PATH, get(mermaid_lib_js))
        .route("/search-index.js", get(search_index_js))
        .route("/ws", get(ws_handler))
        .fallback(page_or_asset)
        .with_state(app.clone());
    let router = with_identity(router, &session_key);
    let router = with_host_guard(router);

    let (listener, addr, replaced) = bind_with_fallback(port, &session_key)
        .await
        .map_err(|e| std::io::Error::new(e.kind(), format!("cannot listen on port {port}: {e}")))?;
    let requested = port;
    let port = addr.port();
    let local = format!("http://127.0.0.1:{port}");

    crate::log::clear_screen();
    crate::log::banner(taliesin_core::VERSION);
    crate::log::ready(&local, start.elapsed());
    crate::log::first_run_notice();
    crate::log::keys_hint();
    crate::log::watching(
        &root.display().to_string(),
        &format!("site, {page_count} pages"),
    );
    // After the banner, never before it: the soft clear pushes whatever came first up into
    // the scrollback, where the project's own diagnostics, the port fallback and the
    // takeover of an earlier preview used to go (audit 2026-09-24, WP2 and WP11 residuals).
    for line in &replaced {
        crate::log::warn(line);
    }
    // (Port 0 asks for any free port, so the one bound is not a fallback.)
    if requested != 0 && port != requested {
        crate::log::warn(&format!("port {requested} in use; using {port}"));
    }
    for w in &startup {
        crate::build::log_located(w, "_site.yml");
    }
    if let Some(w) = &unpublished {
        crate::log::warn(w);
    }
    if open {
        // A document target opens at its own page; a project target at its home.
        match &focus {
            Some(url) => open_in_browser(&format!("{local}/{url}")),
            None => open_in_browser(&local),
        }
    }
    // `into_make_service_with_connect_info` surfaces the peer address to the router.
    let server = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    );
    // Race the server against a shutdown signal so Ctrl-C/SIGTERM returns cleanly and
    // the runtime teardown in `run` can reap the warm pool + kernels (see
    // `crate::serve::shutdown_signal`).
    let outcome = tokio::select! {
        r = server => r.map_err(std::io::Error::other),
        _ = crate::serve::shutdown_signal() => {
            crate::log::kernel("shutting down (reaping kernels)");
            Ok(())
        }
    };
    outcome
}

// --- HTTP ---------------------------------------------------------------

async fn favicon() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        FAVICON,
    )
}

/// The full-text search index as a `search-index.js` script (assigns
/// `window.TALIESIN_SEARCH_INDEX`), lazy-loaded by the Cmd-K palette on first open. Served
/// as JS (not raw JSON) so the client can load it with a `<script>`, which also works
/// under file:// for a built book opened from disk.
/// Serve the vendored mermaid library so a diagram in **preview** needs no network
/// (OFF-2). The library is `include_str!`-compiled into the binary, so this reads nothing
/// from disk and cannot 404. Immutable-cached: the bytes only change when the binary does.
async fn mermaid_lib_js() -> impl IntoResponse {
    (
        [
            (
                axum::http::header::CONTENT_TYPE,
                "text/javascript; charset=utf-8",
            ),
            (
                axum::http::header::CACHE_CONTROL,
                "public, max-age=31536000, immutable",
            ),
        ],
        taliesin_core::mermaid_min_js(),
    )
}

async fn search_index_js(State(app): State<Arc<SiteApp>>) -> impl IntoResponse {
    let json = { app.root.site.lock().search_index_json.clone() };
    let json = if json.is_empty() {
        "[]".to_string()
    } else {
        json
    };
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/javascript; charset=utf-8",
        )],
        format!("window.TALIESIN_SEARCH_INDEX={json};"),
    )
        .into_response()
}

/// Resolve a `GET` or `HEAD` to a page (rendered live) or a static asset under the root.
async fn page_or_asset(
    State(app): State<Arc<SiteApp>>,
    method: axum::http::Method,
    uri: axum::http::Uri,
) -> axum::response::Response {
    // Reads only. The fallback answered every method as a GET, so a `POST` or a `DELETE`
    // got the page or the file (audit 2026-09-24, WP1 residual).
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return (
            axum::http::StatusCode::METHOD_NOT_ALLOWED,
            [(axum::http::header::ALLOW, "GET, HEAD")],
        )
            .into_response();
    }
    let path = percent_decode(uri.path().trim_start_matches('/'));
    let project = &app.root;
    let sub = path.as_str();
    let lookup = if sub.is_empty() {
        // For a single-document preview the document IS the root. `preview note.tmd`
        // serves a project of one page called `note.html`, and without this the bare
        // preview URL would resolve to an `index.html` that does not exist and answer
        // with the 404 page — for the one document the author asked to see. (The old
        // single-document server served it at `/`, and `--open` still opens its page
        // directly, so this is the path a hand-typed URL or a script takes.)
        project
            .scope
            .as_deref()
            .and_then(|f| {
                let site = project.site.lock();
                focus_url(&site, f)
            })
            .unwrap_or_else(|| "index.html".to_string())
    } else {
        sub.to_string()
    };
    // 1) A live page of this project.
    let page = { project.site.lock().page(&lookup).cloned() };
    if let Some(page) = page {
        return Html(ensure_and_render_page(&app, project, &page)).into_response();
    }
    // 2) The project's route-served search index (not written to disk in preview). For a
    //    mount this arrives as `/<prefix>/search-index.js`; without this Cmd-K search on a
    //    mounted page would 404.
    if lookup == "search-index.js" {
        let j = project.site.lock().search_index_json.clone();
        let j = if j.is_empty() { "[]".to_string() } else { j };
        return (
            [(
                axum::http::header::CONTENT_TYPE,
                "text/javascript; charset=utf-8",
            )],
            format!("window.TALIESIN_SEARCH_INDEX={j};"),
        )
            .into_response();
    }
    // 3) A static asset under this project's root, else this project's own 404 page
    //    (with a 404 status) so preview mirrors the deployed `404.html`.
    let asset = serve_asset(&project.dir, &lookup);
    if asset.status() == axum::http::StatusCode::NOT_FOUND {
        let html = { project.site.lock().render_404_page() };
        let html = format!("{html}{RECHECK_404_JS}");
        return (axum::http::StatusCode::NOT_FOUND, Html(html)).into_response();
    }
    asset
}

/// What the preview adds to the build's 404 page: a check, once a second, whether the page
/// it stands for exists now, reloading onto it when it does.
///
/// A tab lands on the 404 page when the page it was open on vanishes (renamed, deleted, or
/// deleted and written again in two saves, as `git` and some editors do), and the 404 page
/// carries no live client, so the tab stayed there after the page came back (audit
/// 2026-09-24 invalidation #13). A `HEAD` of the tab's own URL is the whole question, and it
/// keeps working across a restart of the preview.
const RECHECK_404_JS: &str = "<script>setInterval(()=>fetch(location.href,{method:'HEAD',\
    cache:'no-store'}).then(r=>{if(r.ok)location.reload()},()=>{}),1000);</script>\n";

/// Serve a file under `root`, with path-traversal protection.
fn serve_asset(root: &Path, rel: &str) -> axum::response::Response {
    crate::serve::serve_asset_from(root, rel)
}

/// Ensure the page has live state (creating it + queuing an execution build on
/// first visit), then render its full live HTML for the first paint.
fn ensure_and_render_page(app: &SiteApp, project: &Arc<Project>, page: &Page) -> String {
    let rel = page.rel.clone();
    if !project.pages.lock().contains_key(&rel) {
        // First-paint render (markdown + listing cards, no code execution yet);
        // done outside the pages lock since it needs the site lock for listings.
        let doc = {
            let site = project.site.lock();
            render_markdown_only(&site, page)
        };
        let (tx, _) = broadcast::channel(256);
        project
            .pages
            .lock()
            .entry(rel.clone())
            .or_insert(PageState { doc, tx });
        app.queue_build(rel.clone());
    }
    site_page_html(project, page)
}

/// A first-paint render without code execution (the worker fills outputs after): the one
/// page pass ([`crate::lint::PagePass`]) with no executor, so the page paints finished
/// exactly as the build finishes it (numbering, cross-references, `listing:` cards). The
/// caller holds the site lock across it.
fn render_markdown_only(site: &taliesin_core::Site, page: &Page) -> PageDoc {
    let (src, read) =
        taliesin_core::reads::record(|| taliesin_core::includes::read_source(&page.input));
    let Ok(src) = src else {
        return PageDoc {
            errored: true,
            reads: keyed(read),
            ..Default::default()
        };
    };
    let mut pass = crate::lint::PagePass::run_static(site, page, src, &page_label(page));
    taliesin_core::reads::merge(&mut pass.reads, read);
    PageDoc {
        // Resolved off the *finished* doc, exactly as the static build resolves it, so the
        // first paint, every `full_render`, and `_site/` cannot name one tab three ways.
        tab_title: site.page_title(page, &pass.doc),
        toc: pass.toc,
        includes: pass.doc.includes,
        blocks: pass.doc.blocks,
        diagnostics: pass.diags,
        errored: false,
        generation: 0, // first paint; the exec pass bumps it when it splices outputs
        // The first-paint render never runs cells, so it learns nothing about this page's
        // lane: leave it on the safe one until a real build reports back (AP3-1).
        cell_free: false,
        reads: keyed(pass.reads),
        stamps: HashMap::new(),
    }
}

/// `reads` keyed as the watcher's changed paths are ([`record_key`]), so the two compare.
fn keyed(reads: taliesin_core::reads::Reads) -> taliesin_core::reads::Reads {
    let mut out = taliesin_core::reads::Reads::new();
    let keyed = reads
        .into_iter()
        .map(|(path, access)| (record_key(&path), access));
    taliesin_core::reads::merge(&mut out, keyed.collect());
    out
}

/// Build the full live HTML for a page: theme + base + site CSS, the SSR body
/// wrapped in the site chrome, and the preview client scoped to this page's ws.
fn site_page_html(project: &Arc<Project>, page: &Page) -> String {
    // `tab_title` is the string the producer already resolved (`Site::page_title`) — the
    // very one the websocket re-asserts on connect, so the two cannot disagree. It is
    // deliberately NOT re-derived here if empty: that means the page has no live state at
    // all (the arm below has no body at all), and re-composing half the title
    // policy at a second call site is the exact shape of the bug this replaced.
    let live = {
        let pages = project.pages.lock();
        pages
            .get(&page.rel)
            .map(|ps| LivePage {
                tab_title: ps.doc.tab_title.clone(),
                toc: ps.doc.toc,
                body: ps.doc.body_html(),
                includes: ps.doc.includes.clone(),
                generation: ps.doc.generation,
            })
            .unwrap_or_default()
    };
    let frame = SiteFrame::of(&project.site.lock(), page);
    live_page_html(&frame, &project.dir, page, &live)
}

/// What a live page takes from its site, taken under the site lock and assembled without
/// it: the chrome it is wrapped in, and the dev menu's drafts row.
struct SiteFrame {
    chrome: taliesin_core::render::SiteCtx,
    drafts_global: String,
}

impl SiteFrame {
    fn of(site: &Site, page: &Page) -> SiteFrame {
        // Draft pages (preview only) power the dev-menu "Drafts" row. Root-absolute urls so a
        // link resolves from any page depth. A build ships neither this global nor the dev
        // menu.
        let items: Vec<String> = site
            .pages
            .iter()
            .filter(|p| p.draft)
            .map(|p| {
                format!(
                    "{{\"url\":\"/{}\",\"title\":\"{}\"}}",
                    js_str(&p.url),
                    js_str(p.title.as_deref().unwrap_or(&p.rel)),
                )
            })
            .collect();
        SiteFrame {
            chrome: site.page_chrome(page),
            drafts_global: format!("window.TALIESIN_DRAFTS=[{}];", items.join(",")),
        }
    }
}

/// What a live page takes from its own last build ([`PageDoc`]). All empty for a page with
/// no live state.
#[derive(Default)]
struct LivePage {
    tab_title: String,
    toc: bool,
    body: String,
    includes: taliesin_core::render::PageIncludes,
    generation: u64,
}

/// A digest of what a tab on `page` shows outside its `#tali-root`: the `<body>` of the live
/// page with its blocks and its generation left out (navbar, book drawer, pager, footer, the
/// draft banner, the dev menu's drafts row). That reaches an open tab only by a reload; the
/// blocks reach it as ops and the title as a `title` message.
///
/// The `<head>` is left out on purpose. It carries the page's own social meta, which follows
/// its `title:` and `description:`, and a tab cannot show it: counting it would reload the
/// page an author is typing a `title:` into, throwing away its live state, for nothing the
/// reader can see.
fn shell_digest(site: &Site, dir: &Path, page: &Page, doc: &PageDoc) -> u64 {
    let live = LivePage {
        toc: doc.toc,
        includes: doc.includes.clone(),
        ..LivePage::default()
    };
    let html = live_page_html(&SiteFrame::of(site, page), dir, page, &live);
    let body = taliesin_core::render::tags(&html)
        .find(|t| t.name.eq_ignore_ascii_case("body"))
        .map_or(0, |t| t.at);
    taliesin_core::hash::fnv1a(&html[body..])
}

/// The live page: `frame` around `live`, with the preview client.
fn live_page_html(frame: &SiteFrame, dir: &Path, page: &Page, live: &LivePage) -> String {
    let LivePage {
        tab_title,
        toc,
        body,
        includes: page_includes,
        generation,
    } = live;
    let (toc, generation) = (*toc, *generation);
    let chrome = &frame.chrome;
    // Site-level `format: html:` includes first, then this page's own front matter.
    let mut includes = chrome.includes.clone();
    includes.merge(page_includes);

    // The TOC rail is an empty landmark the client fills once it has the headings; the
    // wrapper class that reserves the column for it is `SiteCtx::layout`'s business, not
    // this path's.
    let (toc_nav, toc_flag) = if toc {
        (
            "<nav id=\"TOC\" aria-label=\"Table of contents\"></nav>",
            "window.TALIESIN_TOC = true;",
        )
    } else {
        ("", "")
    };

    // Absolute paths for click-to-source `vscode://file/…` links.
    let doc_path = page
        .input
        .canonicalize()
        .unwrap_or_else(|_| page.input.clone());
    let base_dir = page.input.parent().unwrap_or(Path::new("."));
    let base_dir = base_dir
        .canonicalize()
        .unwrap_or_else(|_| base_dir.to_path_buf());
    // `root` lets the locator resolve site-root-relative `data-tali-src` targets
    // (a card → its post's source, the navbar/footer → _site.yml, etc.).
    let doc_global = format!(
        "window.TALIESIN_DOC = {{ path: \"{}\", baseDir: \"{}\", root: \"{}\" }};",
        js_str(&doc_path.to_string_lossy()),
        js_str(&base_dir.to_string_lossy()),
        js_str(&dir.to_string_lossy()),
    );
    let ws_path = format!("/ws?page={}", encode_query(&page.rel));
    // Cross-page Cmd-K search: point the palette at the lazy-loaded `search-index.js`
    // (depth-relative, served at the root). Empty for a project with no index.
    let search_cfg = if chrome.search_index.is_empty() {
        String::new()
    } else {
        format!("{};", chrome.search_index)
    };
    // Body links (author `.tmd` references) -> `.html`; chrome links already are.
    let body = taliesin_core::site::rewrite_tmd_links(body);
    // The site's configured favicon (depth-relative); else the dev server's own.
    let favicon = if chrome.favicon.is_empty() {
        "<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.ico\" />".to_string()
    } else {
        taliesin_core::favicon_link(&chrome.favicon)
    };

    // The chrome around the page — book topbar + drawer, or navbar on top — comes from the
    // SAME shell the build calls (`SiteCtx::layout`), so the preview cannot paint a layout
    // the build does not. All this path decides is what goes INSIDE: the live `#tali-root`
    // mount the websocket client drives, and the empty `<nav id="TOC">` it fills. A book has
    // no right rail (item 76), so `toc` is false there and `toc_nav` is empty.
    let (body_class, layout) = chrome.layout(
        &format!("<main id=\"tali-root\">{body}</main>\n{toc_nav}\n"),
        toc,
    );

    // The live body: the site chrome + the mountable `#tali-root`, plus the dev-menu
    // mount. The websocket client drives everything after first paint.
    let body = format!("{layout}\n<div id=\"tali-controls\"></div>");
    let extra_head = format!("<style>{STATUS_CSS}</style>\n");
    let boot = protocol::boot_id();
    let drafts_global = &frame.drafts_global;
    let scripts_pre = format!(
        "<script>{doc_global} {toc_flag} {search_cfg} {drafts_global} window.TALIESIN_SSR = true; window.TALIESIN_SSR_GEN = {generation}; window.TALIESIN_BOOT = {boot}; window.TALIESIN_WS_PATH = \"{ws_path}\";</script>"
    );
    // The cross-page TOC scrollspy + Cmd-K search, then the websocket client.
    let scripts_post = format!(
        "<script>{toc_spy}</script>\n<script>{search_js}</script>\n<script>\n{CLIENT_JS}\n</script>",
        toc_spy = taliesin_core::TOC_SPY_JS,
        search_js = taliesin_core::SEARCH_JS,
    );
    taliesin_core::assemble_html_page(&taliesin_core::PageParts {
        // Live preview always ships everything (a doc can gain any construct on an edit).
        mode: taliesin_core::OutputMode::Preview,
        title: tab_title,
        favicon: &favicon,
        with_site_css: true,
        // A live page can gain math at any edit, so always ship the KaTeX styles.
        ship_katex: true,
        extra_head: &extra_head,
        body_class: &body_class,
        include_in_header: &includes.in_header,
        include_before_body: &includes.before_body,
        body: &body,
        scripts_pre: &scripts_pre,
        scripts_post: &scripts_post,
        include_after_body: &includes.after_body,
        ..taliesin_core::PageParts::defaults()
    })
}

/// Percent-encode a page rel as the `?page=` value of the ws URL: everything outside the
/// RFC 3986 unreserved set, keeping `/` (query-safe, and it is what makes a multi-page rel
/// readable in the url).
///
/// It encoded the space alone until 2026-08-13, and a doc comment claimed that was the
/// whole unsafe alphabet. It is not, and every miss is silent: `&` ends the parameter (axum
/// hands `client_conn` a truncated key), `+` decodes back as a space, `#` truncates the url
/// at the fragment, `%` starts an escape. A key that names no page is refused, so the page
/// renders at 200 with a green status pill while the client reconnects every second
/// forever. Non-ASCII goes out as its UTF-8 bytes rather than riding on the browser's own
/// normalisation, so the value on the wire is the same one this server built.
fn encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// --- WebSocket ----------------------------------------------------------

async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    State(app): State<Arc<SiteApp>>,
) -> axum::response::Response {
    if !ws_origin_ok(&headers) {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "cross-origin websocket refused",
        )
            .into_response();
    }
    let rel = q.get("page").cloned().unwrap_or_default();
    ws.max_message_size(crate::serve::MAX_WS_MESSAGE_BYTES)
        .on_upgrade(move |socket| client_conn(socket, app, rel))
        .into_response()
}

async fn client_conn(socket: WebSocket, app: Arc<SiteApp>, page_key: String) {
    let (mut sink, mut stream) = socket.split();

    // Normalise the client's page key to a source rel (the key may be a url).
    let project = app.root.clone();
    let rel = resolve_page_rel(&project, &page_key);

    // A `?page=` the owning project cannot resolve names no page at all, so there is
    // nothing to render, subscribe to, or rebuild — `build_page` already returns
    // immediately on such a key. Allocating a `PageState` for it anyway (a 256-slot
    // broadcast ring that only a save finding no tab on it evicts) let anyone who can reach this socket grow the
    // map without bound just by reconnecting with a fresh bogus key, clearable only by
    // restarting the preview. Refuse the key instead of allocating for it.
    let Some(rel) = rel else {
        let _ = sink
            .send(Message::Text(
                protocol::error(&format!("unknown page: {page_key}")).into(),
            ))
            .await;
        return;
    };

    let (snapshot, mut rx, created) = {
        let mut pages = project.pages.lock();
        let created = !pages.contains_key(&rel);
        let ps = pages.entry(rel.clone()).or_insert_with(|| PageState {
            doc: PageDoc::default(),
            tx: broadcast::channel(256).0,
        });
        (full_render_json(&ps.doc), ps.tx.subscribe(), created)
    };
    if created {
        app.queue_build(rel.clone());
    }
    if sink.send(Message::Text(snapshot.into())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            broadcasted = rx.recv() => match broadcasted {
                Ok(text) => {
                    if sink.send(Message::Text(text.into())).await.is_err() { break; }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let fr = {
                        let pages = project.pages.lock();
                        pages.get(&rel).map(|ps| full_render_json(&ps.doc))
                    };
                    if let Some(fr) = fr
                        && sink.send(Message::Text(fr.into())).await.is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(t))) => {
                    // The dev menu's "Restart kernel" action restarts this page's kernel.
                    if is_restart_kernel(t.as_str()) {
                        // Stop the running cell BEFORE queueing, or the Restart waits
                        // behind the very build it is meant to abort: the builder is
                        // serial and awaits each page to completion, so the queued message
                        // is not read until the runaway cell has already finished (audit
                        // finding 01). This page's own kernel is killed, not interrupted,
                        // or the build runs every later cell in it first (E6; see
                        // `restart_stop`). A pid of 0 means nothing is executing, and the
                        // queued Restart alone is then the whole action.
                        //
                        // The pid is pool-wide, so it may belong to ANOTHER page (A17).
                        // Decide that first and record it under the same lock that
                        // publishes it, so the victim's own in-flight build can say where
                        // its `KeyboardInterrupt` came from instead of just showing one.
                        let pid = app.interrupt.load(std::sync::atomic::Ordering::SeqCst);
                        let victim = {
                            let mut lane = app.root.exec_lane.lock();
                            let victim = cross_page_victim(&rel, &lane.page, pid);
                            if let Some(v) = &victim {
                                lane.interrupted_by = Some((v.clone(), rel.clone()));
                            }
                            victim
                        };
                        match restart_stop(pid, victim.as_deref()) {
                            Some(Stop::Kill) => crate::kernel::kill_pid(pid),
                            Some(Stop::Interrupt) => crate::kernel::interrupt_pid(pid),
                            None => {}
                        }
                        if let Some(v) = victim {
                            crate::log::kernel(&format!(
                                "interrupted the cell running on {v} so the kernel restart \
                                 requested on {rel} could go through"
                            ));
                        }
                        let _ = app
                            .build_tx
                            .send(BuildMsg::Restart(rel.clone()));
                    } else {
                        handle_client_msg(t.as_str());
                    }
                }
                Some(Ok(Message::Close(_))) | None => break,
                Some(Err(_)) => break,
                _ => {}
            },
        }
    }
}

/// Whether a client ws message is the dev-menu "Restart kernel" request.
fn is_restart_kernel(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v.get("type")?.as_str().map(str::to_string))
        .as_deref()
        == Some("restart_kernel")
}

fn handle_client_msg(text: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    if v.get("type").and_then(|t| t.as_str()) == Some("click_block") {
        let file = v
            .get("source_file")
            .and_then(|f| f.as_str())
            .unwrap_or("(primary)");
        let pos = v.get("sourcepos").and_then(|p| p.as_str()).unwrap_or("?");
        crate::log::source(&format!("{file}  {pos}"));
    }
}

// --- messages -----------------------------------------------------------

fn full_render_json(d: &PageDoc) -> String {
    use taliesin_core::site::rewrite_tmd_links;
    protocol::full_render(
        // The display-ready tab title, NOT the raw front-matter one: the client assigns
        // this straight to `document.title`, over the `<title>` we server-rendered. Null
        // (not "") for a page with no render yet, so the client keeps its own default.
        (!d.tab_title.is_empty()).then_some(d.tab_title.as_str()),
        &rewrite_tmd_links(&d.body_html()),
        d.generation,
        &d.diagnostics,
    )
}

/// Like the single-doc server's `op_json`, but rewrites any author `.tmd` links
/// in the block HTML to their `.html` targets before it goes over the wire.
fn op_json(op: &BlockOp, generation: u64) -> String {
    protocol::op(op, generation, taliesin_core::site::rewrite_tmd_links)
}

// --- build worker -------------------------------------------------------

fn spawn_builder(app: Arc<SiteApp>, mut build_rx: mpsc::UnboundedReceiver<BuildMsg>) {
    tokio::spawn(async move {
        // The project's one ExecPool. `exec_pool.rs` is used verbatim. Interpreters come
        // from the project's own `_site.yml`/root (python:, a project .venv, env, or
        // default), asked again before every job ([`repoint`]). The pool is owned by this
        // task and dropped on channel close (server shutdown), which kills every kernel it
        // holds.
        let project = app.root.clone();
        let py = resolve_python_for(&project);
        let mut pool = ExecPool::new(project.dir.join("_freeze"), py, app.interrupt.clone());
        while let Some(msg) = build_rx.recv().await {
            repoint(&mut pool, &project, &app.interrupt);
            match msg {
                BuildMsg::Build(rel) => {
                    build_on_exec_lane(&project, &rel, &mut pool).await;
                }
                BuildMsg::IfMoved(rel, paths) => {
                    if probes_moved(&project, &rel, &paths) {
                        build_on_exec_lane(&project, &rel, &mut pool).await;
                    }
                }
                BuildMsg::Restart(rel) => {
                    // Drop + respawn this page's kernel, then rebuild (re-executes every
                    // cell against the fresh kernel).
                    pool.restart(&rel);
                    build_on_exec_lane(&project, &rel, &mut pool).await;
                    // A fresh kernel means fresh outputs, including any `ojs_define`
                    // values. Reload the page so the `{js}` cells re-bind to the fresh
                    // `tali-define` blobs from a clean module scope.
                    if let Some(ps) = project.pages.lock().get(&rel) {
                        let _ = ps.tx.send(protocol::reload());
                    }
                }
            }
        }
    });
}

/// The interpreter `project` runs its cells with, resolved as of now (`_site.yml` `python:`,
/// the project's `.venv`, `TALIESIN_PYTHON`, an ancestor `.venv`, else `python3`).
fn resolve_python_for(project: &Project) -> crate::interpreter::Resolved {
    let site = project.site.lock();
    crate::interpreter::resolve_python(site.config.python.as_deref(), &project.dir)
}

/// Point the exec lane's `pool` at the interpreter the project resolves to now: a fresh pool
/// when it changed, the old one dropped with every kernel it holds.
///
/// It was resolved once, when the preview started, so a `python:` edited in `_site.yml`
/// never reached a kernel, not even through Restart kernel, and a `.venv` created while the
/// preview ran was ignored, though the guide says to fix the kernel and save (audit
/// 2026-09-24 C8, first-hour #9). Asked before every job on the lane, so a save or a Restart
/// kernel picks the change up; resolving is a handful of `exists` calls. The pool's warm
/// cap and eviction order are its own and untouched.
fn repoint(pool: &mut ExecPool, project: &Project, interrupt: &Arc<std::sync::atomic::AtomicU32>) {
    let python = resolve_python_for(project);
    // The new pool's first kernel says which interpreter it runs, and from where.
    if pool.python() != Some(python.path.as_path()) {
        *pool = ExecPool::new(project.dir.join("_freeze"), python, interrupt.clone());
    }
}

/// Build `rel` on the exec lane, publishing which page the lane is running cells for while
/// it does (A17). The websocket task reads that to tell whose cell the pool-wide interrupt
/// pid belongs to; see [`ExecLane`].
///
/// The clear afterwards also drops an interrupt notice this build never picked up — only
/// possible when the build returned before its diagnostics (an unresolvable or unreadable
/// page). A notice that outlived its build would surface on some later, unrelated rebuild
/// of that page, which is a worse lie than the silence it replaces.
async fn build_on_exec_lane(
    project: &Arc<Project>,
    rel: &str,
    pool: &mut ExecPool,
) -> BuildOutcome {
    project.exec_lane.lock().page = rel.to_string();
    let outcome = build_page_guarded(project, rel, Some(pool)).await;
    let mut lane = project.exec_lane.lock();
    lane.page.clear();
    lane.take_interrupt_for(rel);
    outcome
}

/// The bypass lane (AP3-1): rebuilds for pages whose last build found no kernel cell.
///
/// Serial, exactly like the exec builder — it just owns no `ExecPool` and can therefore
/// never wait on one. A page routed here that turns out to HAVE kernel cells (the edit
/// that adds the first one) is handed to the exec lane instead; that is the one wasted
/// render this design costs, and it happens once per page.
fn spawn_fast_builder(app: Arc<SiteApp>, mut fast_rx: mpsc::UnboundedReceiver<BuildMsg>) {
    tokio::spawn(async move {
        while let Some(msg) = fast_rx.recv().await {
            let project = app.root.clone();
            let rel = match msg {
                BuildMsg::Build(rel) | BuildMsg::Restart(rel) => rel,
                BuildMsg::IfMoved(rel, paths) if probes_moved(&project, &rel, &paths) => rel,
                BuildMsg::IfMoved(..) => continue,
            };
            if build_page_guarded(&project, &rel, None).await == BuildOutcome::NeedsKernel {
                let _ = app.build_tx.send(BuildMsg::Build(rel));
            }
        }
    });
}

/// On a page's FIRST build, put the pre-exec body on screen instead of leaving the reader
/// on a blank one until every cell has finished.
///
/// **The defect this closes (audit finding 02).** `build_page` renders the markdown, then
/// awaits `exec.run` for ALL cells, and only then publishes. A page the websocket reaches
/// before any build has state allocated by `client_conn` with no blocks in it, so the
/// opening snapshot is a `full_render` over an empty doc: measured at 20 s of bare navbar
/// on a page with one 25 s cell, with no spinner and no status, while the prose that needed
/// no kernel at all sat rendered in memory one statement above the await. Wave 11 recorded
/// the accepted cost of the warm-pool cut as "a `warming-kernel` state on the first cell";
/// what shipped was no state at all.
///
/// **A full publish only on a first build.** A warm edit already has a body on screen, and a
/// second full publish there would flash it away and back for nothing. A rebuild sends only
/// its edited cells instead ([`publish_edited_cells`]).
///
/// The cells go out as source, which is exactly what `--no-exec` already publishes, so the
/// shape is supported end to end. The post-exec publish is untouched, and the diff between
/// the two is what turns each cell's source into its output — `build_page` bumps the render
/// generation on that diff, which is the re-mount the client is already told to expect.
fn publish_pre_exec_body(project: &Arc<Project>, rel: &str, page: &Page, blocks: &[Block]) {
    // Finished exactly as the post-exec publish finishes them (numbering, cross-refs,
    // listing expansion), so this paint is the `--no-exec` render of the page rather than a
    // half-resolved one showing raw `@fig-` text. These warnings are recomputed against the
    // executed blocks below and are discarded here.
    let finished = || {
        let mut pre = blocks.to_vec();
        let mut discarded = Vec::new();
        let site = project.site.lock();
        site.finish_blocks(page, &mut pre, &mut discarded, None, None);
        pre
    };
    let edits = project
        .pages
        .lock()
        .get(rel)
        .filter(|ps| !ps.doc.blocks.is_empty())
        .map(|ps| edited_cells(&ps.doc.blocks, blocks, executable_cell));
    if let Some(edits) = edits {
        // A body is already on screen: this is a rebuild, not a first paint.
        if let Some(edits) = edits.filter(|e| !e.is_empty()) {
            publish_edited_cells(project, rel, &finished(), &edits);
        }
        return;
    }
    let pre = finished();
    let mut pages = project.pages.lock();
    // A page with no state was dropped while this build ran (see `build_page`'s publish).
    let Some(ps) = pages.get_mut(rel) else {
        return;
    };
    ps.doc.blocks = pre;
    let _ = ps.tx.send(full_render_json(&ps.doc));
}

/// On a rebuild, put each EDITED code cell on screen before it runs (audit E7).
///
/// An edit changes the cell's content hash, so its block id: the block the client holds
/// carries the old id, and the new one only arrived with the post-exec publish. The
/// executor streams the cell's `running` state and its live output under the NEW id, and
/// the client finds nowhere to show either (`openLiveOutput` looks the cell up by it), so
/// the one cell the author is watching showed its old source and output, with no badge,
/// until it finished. Only unedited downstream cells, whose ids survive, streamed.
///
/// So each edited cell goes out now as an `update` of the block it replaces, and its old
/// output is removed, since the cell is about to produce a new one and the live output
/// streams in its place. The page model is changed to match exactly what was sent, so the
/// post-exec diff starts from what the client really holds. Prose edits still wait for
/// the post-exec publish.
///
/// Edited cells are paired with the blocks they replace by order, and only when that is
/// unambiguous: the page has the same number of executable cells as before, and every pair
/// that differs is a new id replacing an id that is gone. Anything else (a cell added,
/// removed or moved) sends nothing here and leaves it all to the post-exec diff.
fn publish_edited_cells(
    project: &Arc<Project>,
    rel: &str,
    pre: &[Block],
    edits: &[(usize, usize)],
) {
    let mut pages = project.pages.lock();
    let Some(ps) = pages.get_mut(rel) else {
        return;
    };
    // The model may have moved since the pairing was made; pair again against it.
    if edited_cells(&ps.doc.blocks, pre, executable_cell).as_deref() != Some(edits) {
        return;
    }
    let mut ops = Vec::new();
    // Back to front, so removing an output block does not shift a later edit's index.
    for &(at, new) in edits.iter().rev() {
        let old = std::mem::replace(&mut ps.doc.blocks[at], pre[new].clone());
        let out = format!("{}-out", old.id);
        if ps.doc.blocks.get(at + 1).is_some_and(|b| b.id == out) {
            ps.doc.blocks.remove(at + 1);
            ops.push(BlockOp::Remove { target_id: out });
        }
        ops.push(BlockOp::Update {
            target_id: old.id,
            html: pre[new].html.clone(),
        });
    }
    ops.reverse();
    ps.doc.generation = ps.doc.generation.wrapping_add(1);
    let generation = ps.doc.generation;
    for op in &ops {
        let _ = ps.tx.send(op_json(op, generation));
    }
}

/// Whether `b` is a code cell the kernel executes (the cells that stream).
fn executable_cell(b: &Block) -> bool {
    b.cell
        .as_ref()
        .is_some_and(|c| taliesin_core::render::executes_to_kernel(&c.lang))
}

/// The `(index in on_screen, index in new)` pairs of code cells an edit replaced, or `None`
/// when the pairing is ambiguous (see [`publish_edited_cells`]). `cell` says which blocks
/// are executable cells.
fn edited_cells(
    on_screen: &[Block],
    new: &[Block],
    cell: impl Fn(&Block) -> bool,
) -> Option<Vec<(usize, usize)>> {
    let old: Vec<usize> = (0..on_screen.len())
        .filter(|&i| cell(&on_screen[i]))
        .collect();
    let now: Vec<usize> = (0..new.len()).filter(|&i| cell(&new[i])).collect();
    if old.len() != now.len() {
        return None;
    }
    let old_ids: std::collections::HashSet<&str> =
        on_screen.iter().map(|b| b.id.as_str()).collect();
    let new_ids: std::collections::HashSet<&str> = new.iter().map(|b| b.id.as_str()).collect();
    let mut edits = Vec::new();
    for (&o, &n) in old.iter().zip(&now) {
        let (was, is) = (&on_screen[o].id, &new[n].id);
        if was == is {
            continue;
        }
        if new_ids.contains(was.as_str()) || old_ids.contains(is.as_str()) {
            return None; // a move or a swap, not an edit in place
        }
        edits.push((o, n));
    }
    Some(edits)
}

/// Whether a rendered page needs no kernel, and so belongs on the bypass lane (AP3-1).
///
/// Asked of the RENDERED blocks, not of the source: this is exactly the set the executor
/// would run, `{{< include >}}` resolved and cell options applied, so the routing decision
/// and the work it routes around cannot disagree about what a cell is.
///
/// `executes_to_kernel` is the shared predicate the render pass and the executor already
/// agree on (`exec::tests::kernel_lang_agrees_with_cores_executable_set` pins them equal),
/// which is what makes a `{js}` page cell-free here: `{js}` runs in the browser, so a page
/// full of reactive cells needs the kernel lane exactly as much as a prose page does.
fn is_cell_free(blocks: &[Block]) -> bool {
    !blocks
        .iter()
        .flat_map(|b| b.cells())
        .any(|c| taliesin_core::render::executes_to_kernel(&c.lang))
}

/// What a build pass concluded about the page's lane.
#[derive(PartialEq, Eq, Clone, Copy)]
enum BuildOutcome {
    Done,
    /// Only ever returned by the bypass lane: this page has kernel cells after all, so the
    /// pass stopped before executing anything and the exec lane must take it.
    NeedsKernel,
}

/// Run [`build_page`], catching any panic in the render/exec path so one bad
/// page can't kill the shared builder task (which would silently stop hot-reload
/// for *every* page). The panic is logged and surfaced to that page's clients;
/// the next good save recovers.
async fn build_page_guarded(
    project: &Arc<Project>,
    rel: &str,
    pool: Option<&mut ExecPool>,
) -> BuildOutcome {
    use futures_util::FutureExt;
    let outcome = std::panic::AssertUnwindSafe(build_page(project, rel, pool))
        .catch_unwind()
        .await;
    match outcome {
        Ok(outcome) => outcome,
        Err(payload) => {
            let msg = crate::serve::panic_msg(&*payload);
            crate::log::error(&format!(
                "render panicked on {rel} (preview kept alive): {msg}"
            ));
            let mut pages = project.pages.lock();
            if let Some(ps) = pages.get_mut(rel) {
                ps.doc.errored = true;
                let _ = ps
                    .tx
                    .send(protocol::error(&format!("internal render error: {msg}")));
            }
            // A panicked pass says nothing about the page's lane; leave the routing flag
            // where it was rather than bouncing the page between queues.
            BuildOutcome::Done
        }
    }
}

/// Re-render a page's markdown, run its code cells (on the page's own executor),
/// then diff against its live blocks and broadcast the changes to its subscribers.
///
/// `pool` is `None` on the bypass lane (AP3-1), which owns no executor. A page routed
/// there that turns out to have kernel cells returns [`BuildOutcome::NeedsKernel`] without
/// publishing anything, and the exec lane rebuilds it.
async fn build_page(
    project: &Arc<Project>,
    rel: &str,
    pool: Option<&mut ExecPool>,
) -> BuildOutcome {
    let page = { project.site.lock().page(rel).cloned() };
    let Some(page) = page else {
        return BuildOutcome::Done;
    };
    let (src, mut reads) =
        taliesin_core::reads::record(|| taliesin_core::includes::read_source(&page.input));
    let Ok(src) = src else {
        let mut pages = project.pages.lock();
        if let Some(ps) = pages.get_mut(rel) {
            ps.doc.errored = true;
            // Still a dependency: writing the source again rebuilds the page.
            ps.doc.reads = keyed(reads);
            let _ = ps.tx.send(protocol::error(&format!(
                "cannot read {}",
                page.input.display()
            )));
        }
        return BuildOutcome::Done;
    };
    let base = page.input.parent().unwrap_or(Path::new(".")).to_path_buf();
    let label = page_label(&page);
    // THE page pass every verb runs. What it needs of the site is taken under the lock and
    // the lock released before the render; the cells run with no lock held.
    let render = {
        let site = project.site.lock();
        crate::lint::PageRender::of(&site, &page)
    };
    let mut pass = crate::lint::PagePass::begin(&render, &page, src, &label);

    // Which lane this page actually belongs on, decided from the rendered blocks rather
    // than a guess about the source: exactly the cells the executor would run.
    let cell_free = is_cell_free(&pass.doc.blocks);
    if pool.is_none() && !cell_free {
        // The bypass lane picked this page up (its last build had no cells) and the edit
        // has just added one. Publish nothing — the exec lane redoes this pass with a
        // pool — but record the lane so it is the last time this page comes here.
        if let Some(ps) = project.pages.lock().get_mut(rel) {
            ps.doc.cell_free = false;
        }
        return BuildOutcome::NeedsKernel;
    }
    let exec = pool.map(|pool| {
        let exec = pool.get(rel, &base);
        // Stream this page's code-cell execution progress (`build-state`) onto its own
        // broadcast, tagged with the page rel so the client knows which page it's about.
        // The page's `Sender` is created on first visit (before this build is queued), so
        // it's normally present; if it isn't yet, we just don't stream this pass.
        let tx = project.pages.lock().get(rel).map(|ps| ps.tx.clone());
        let sink: crate::exec::ProgressSink = tx.map(|tx| {
            std::sync::Arc::new(move |m: String| {
                let _ = tx.send(m);
            }) as std::sync::Arc<dyn Fn(String) + Send + Sync>
        });
        exec.set_progress(sink, Some(rel.to_string()));
        exec
    });
    let mut exec = exec;
    if let Some(exec) = exec.as_mut() {
        publish_pre_exec_body(project, rel, &page, &pass.doc.blocks);
        // A failed cell is not repeated among the diagnostics: the dev menu lists each one
        // from the page itself, clickable to the cell.
        let _ = pass.execute(exec).await;
    }
    // Finish the executed blocks exactly as the build does (numbering, cross-refs +
    // broken-ref warnings, listing/about expansion, post decoration). Queries the
    // whole site, so it needs the site lock.
    let tab_title = {
        let site = project.site.lock();
        pass.finish(&site, &page);
        // Re-resolved every build: an edit can add, change, or remove the front-matter
        // title or the leading `# H1` that names the tab.
        site.page_title(&page, &pass.doc)
    };
    let toc = pass.toc;
    let mut diags = std::mem::take(&mut pass.diags);
    // The kernel's availability, which the dev menu shows and the build does not repeat
    // (it reports a missing kernel as its own failure).
    if let Some(message) = exec.as_deref().and_then(|e| e.diagnostic()) {
        let notice = taliesin_core::render::Warning::new(message);
        diags.push(diag_from(&notice, &label));
    }
    // A cell of this page's may have been SIGINTed to let another page's kernel restart
    // through (A17). Read AFTER `exec.run`, which is what the interrupt aborts, so this is
    // the very build that shows the traceback — and the page says where it came from
    // instead of just showing one.
    if let Some(by) = project.exec_lane.lock().take_interrupt_for(rel) {
        let notice = taliesin_core::render::Warning::new(interrupted_notice(&by));
        diags.push(diag_from(&notice, &label));
    }
    // Cross-page links (this page only, and the pages it links to: the whole-site pass would
    // render every page on every save, PERF-1) and the project's own diagnostics, located
    // for this page: the client resolves a `file` from the page's own folder, so they climb
    // to the site root first. Scoped tightly under the site lock.
    {
        let site = project.site.lock();
        // The pages this one links to are read to judge its links, so they are dependencies.
        let (cross, read) =
            taliesin_core::reads::record(|| site.validate_cross_page_links_for(rel));
        taliesin_core::reads::merge(&mut reads, read);
        diags.extend(cross.iter().map(|w| diag_from(w, &label)));
        let config = format!("{}_site.yml", "../".repeat(rel.matches('/').count()));
        diags.extend(crate::lint::project_diagnostics(&site, &config));
    }
    taliesin_core::reads::merge(&mut reads, std::mem::take(&mut pass.reads));
    let reads = keyed(reads);
    // After the cells ran, and before the lock: the files this page only looked at, as it
    // leaves them ([`probes_moved`]).
    let stamps: HashMap<PathBuf, Option<Stamp>> = reads
        .iter()
        .filter(|(_, access)| **access == taliesin_core::reads::Access::Probed)
        .map(|(path, _)| (path.clone(), stamp(path)))
        .collect();
    let doc = pass.doc;

    let mut pages = project.pages.lock();
    // Every build is queued for a page that has state (a visit creates it first), so a
    // page without one had it dropped while this build ran: `reload_open_tabs` cleared it
    // for a re-discovered site, or nobody had the page open. Publishing would put the state
    // back, rendered against the defaults this build captured before the drop, and every
    // later GET would serve that stale body (audit 2026-09-24, invalidation #10). The next
    // visit renders it fresh instead.
    let Some(ps) = pages.get_mut(rel) else {
        return BuildOutcome::Done;
    };
    let recovered = std::mem::take(&mut ps.doc.errored);
    let ops = diff_blocks(&ps.doc.blocks, &doc.blocks);
    // A burst that aims at raw HTML the DOM does not hold as one element (a comment, a
    // wrapper's closing tag, its unclosed opening line) goes out as a full render.
    let remount = recovered || needs_remount(&ps.doc.blocks, &doc.blocks, &ops);
    let diags_changed = ps.doc.diagnostics != diags;
    // Compared BEFORE the assignment below overwrites it. The title is chrome, so it never
    // reaches the tab as a block op: a `title:`-only edit on a page that renders no title
    // block diffs to nothing, and even when it does render one, the body swapped while the
    // tab kept the old name.
    let title_changed = ps.doc.tab_title != tab_title;
    ps.doc.tab_title = tab_title;
    ps.doc.toc = toc;
    ps.doc.includes = doc.includes;
    // Bump the render generation only on a real body change (see serve::rebuild), so a
    // client that server-rendered this page pre-exec re-mounts to pick up the outputs.
    if !ops.is_empty() {
        ps.doc.generation = ps.doc.generation.wrapping_add(1);
    }
    ps.doc.blocks = doc.blocks;
    ps.doc.diagnostics = diags;
    ps.doc.reads = reads;
    ps.doc.stamps = stamps;
    // Broadcast sequencing (body, then theme, then diagnostics — theme/diags after the
    // body even on a recovery re-mount) is the shared contract in `protocol::Broadcast`.
    let generation = ps.doc.generation;
    let messages = protocol::Broadcast {
        ops: &ops,
        remount,
        title_changed,
        diags_changed,
    }
    .messages(
        || full_render_json(&ps.doc),
        |op| op_json(op, generation),
        || protocol::title(Some(&ps.doc.tab_title)),
        || protocol::diagnostics(&ps.doc.diagnostics),
    );
    for m in messages {
        let _ = ps.tx.send(m);
    }
    if !ops.is_empty() {
        crate::log::update(ops.len());
    }
    // Record which lane this page belongs on, for `SiteApp::queue_build` to read on the
    // NEXT save. Written last, after everything this pass publishes, so a routing decision
    // that sees the new value is always looking at a finished build (AP3-1).
    ps.doc.cell_free = cell_free;
    BuildOutcome::Done
}

/// The name the dev menu locates a page's own diagnostics by: its file name, which the
/// client resolves against the page's directory like every other diagnostic `file`.
fn page_label(page: &Page) -> String {
    page.input
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| page.rel.clone())
}

// --- file watching ------------------------------------------------------

fn spawn_watcher(app: Arc<SiteApp>) {
    let (sig_tx, mut sig_rx) = mpsc::unbounded_channel::<PathBuf>();
    let root = app.root.dir.clone();

    // Pump events through a channel so one thread owns the watcher and can register watches
    // for subdirectories that arrive after startup — the recursive-watch model added an
    // inotify descriptor per directory including `node_modules`/`.git`, which a large
    // project uses to exhaust `max_user_watches` and kill hot reload. The watches are
    // registered here, before this returns, so nothing saved after startup goes unseen.
    let (ev_tx, ev_rx) = std::sync::mpsc::channel::<notify::Event>();
    let mut watcher =
        match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                let _ = ev_tx.send(ev);
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                crate::log::error(&format!("file watcher unavailable: {e}"));
                return;
            }
        };
    // A non-recursive watch on every directory except the pruned generated/VCS trees.
    for dir in crate::serve::watch_tree(&root) {
        if let Err(e) = watcher.watch(&dir, notify::RecursiveMode::NonRecursive) {
            crate::log::warn(&format!("cannot watch {}: {e}", dir.display()));
        }
    }

    std::thread::spawn(move || {
        for ev in ev_rx {
            use notify::EventKind::{Create, Modify, Remove};
            if !matches!(ev.kind, Modify(_) | Create(_) | Remove(_)) {
                continue;
            }
            // A directory can arrive by being created or by a rename: renamed inside the
            // tree, or moved in from outside it. Both need watches of their own, since notify
            // drops a moved directory's watch and a moved-in one never had any. Missing the
            // rename left a renamed post folder 404ing and every later edit inside it unseen
            // until a restart (audit 2026-09-24 C3).
            let arrives = matches!(
                ev.kind,
                Create(_) | Modify(notify::event::ModifyKind::Name(_))
            );
            for p in &ev.paths {
                let is_dir = std::fs::symlink_metadata(p)
                    .map(|m| m.is_dir())
                    .unwrap_or(false);
                if arrives && is_dir && p.starts_with(&root) && !crate::serve::is_pruned_dir(p) {
                    for d in crate::serve::watch_tree(p) {
                        let _ = watcher.watch(&d, notify::RecursiveMode::NonRecursive);
                    }
                    // Files already inside the arriving dir were never reported (created
                    // before its watch existed, or moved in whole), so replay them as changes
                    // (a new `.tmd` may add a page) — a `git checkout` or a folder of pages
                    // otherwise wouldn't appear until an unrelated save.
                    for f in crate::serve::subtree_relevant_files(p) {
                        let _ = sig_tx.send(f);
                    }
                }
                // Ignore generated/VCS noise (esp. the executor's own `_freeze/` writes,
                // which would otherwise rebuild every run). Judged relative to the project
                // root: these are absolute event paths, and a project living under a
                // directory that happens to be called `_site` is not generated noise.
                if crate::serve::relevant_path(p, &root) && sig_tx.send(p.clone()).is_err() {
                    // Nothing is listening any more: the preview is shutting down.
                    return;
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some(first) = sig_rx.recv().await {
            let changed = gather(first, &mut sig_rx).await;
            // Guarded, like every other task that renders on the author's behalf. This one
            // was not: `dispatch_changes` re-discovers the project, re-derives the
            // cross-reference registry and rebuilds the search index, and a panic in any of
            // them unwound the only task draining `sig_rx`. The server stayed up and the
            // page stayed served, so the preview did not visibly die — it silently stopped
            // reacting to saves, which reads as "the tool is broken" rather than "this
            // document is broken". Reporting it keeps the failure attached to the edit.
            if let Err(msg) = crate::serve::guarded(|| dispatch_changes(&app, &changed)) {
                crate::log::error(&format!("rebuild failed: {msg}"));
            }
        }
    });
}

/// How long the watcher waits for a save's events to stop before acting on them. Every
/// editor's save, in place or by a rename over the old file (`sed -i` included), delivers
/// all its events within about a millisecond, measured on 2026-09-24 (audit perf #4).
const QUIET: Duration = Duration::from_millis(15);

/// The longest a batch waits for its events to stop: a stream that never pauses (a cell
/// writing a file in a loop) is acted on at this interval.
const MOST_QUIET: Duration = Duration::from_millis(250);

/// One save's changed paths: `first`, and whatever follows it until the events stop for
/// [`QUIET`] (or [`MOST_QUIET`] has passed).
///
/// It was a fixed 80 ms sleep after the first event, which was 80 to 92% of every save on
/// the author's own projects, and the floor that put every kind of save past 100 ms at
/// about 40 to 100 pages (audit 2026-09-24 F3). A save split across two batches costs a
/// second pass and nothing else: each batch is judged by what it changed.
async fn gather(first: PathBuf, rx: &mut mpsc::UnboundedReceiver<PathBuf>) -> HashSet<PathBuf> {
    let mut changed = HashSet::from([first]);
    let deadline = tokio::time::Instant::now() + MOST_QUIET;
    // Ends on a quiet `QUIET`, at the deadline, or when the watcher is gone.
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let Ok(Some(path)) = tokio::time::timeout(QUIET.min(left), rx.recv()).await else {
            break;
        };
        changed.insert(path);
        if left.is_zero() {
            break;
        }
    }
    changed
}

/// Which of the `open` pages actually cite one of `moved_anchors`, read from each open
/// page's already-cached rendered blocks (`PageState.doc.blocks`) via
/// [`taliesin_core::site::xref_anchors_in`] — no re-render, no project-wide reverse index.
/// A page with no live state (closed, or a race with its own first render) is skipped, not
/// force-included: it has no cached blocks to consult and isn't being served regardless.
///
/// Extracted out of [`rebuild_project`] so this selection — the replacement for the
/// deleted "Referenced by" reverse index — is unit-testable on its own, independent of the
/// watcher/lock/async machinery around it.
fn pages_citing_a_moved_anchor(
    pages: &HashMap<String, PageState>,
    open: &[String],
    moved_anchors: &HashSet<String>,
) -> Vec<String> {
    open.iter()
        .filter(|rel| {
            pages.get(rel.as_str()).is_some_and(|ps| {
                let referenced = taliesin_core::site::xref_anchors_in(&ps.doc.blocks);
                moved_anchors.iter().any(|a| referenced.contains(a))
            })
        })
        .cloned()
        .collect()
}

/// Rebuild one project's affected pages from a batch of changed files (already filtered
/// to this project by [`dispatch_changes`]): a `_site.yml` change, or a save that moves
/// the page set, re-discovers this project's site and reloads its open tabs; a save that
/// moves what discovery reads of a page re-discovers and rebuilds every open page;
/// otherwise rebuild every *open* page whose source or include set touches a changed file.
fn rebuild_project(app: &SiteApp, project: &Arc<Project>, changed: &HashSet<PathBuf>) {
    let config_changed = changed
        .iter()
        .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("_site.yml"));
    if config_changed {
        let new = project.rediscover();
        // A mid-edit save can leave `_site.yml` transiently malformed; re-discovering then
        // would replace the live site with the degraded default (losing nav/title/output).
        // Keep the last-good `Site` instead, and surface the parse error, so the preview
        // doesn't visibly collapse on every keystroke. The next valid save reloads cleanly.
        if let Some(w) = new
            .warnings
            .iter()
            .find(|w| taliesin_core::site::is_malformed_config_warning(w))
        {
            crate::log::warn(&format!("{w}; keeping the last-good _site.yml"));
            return;
        }
        project.adopt(new, &HashMap::new(), true);
        reload_open_tabs(project);
        return;
    }

    // The registry as it stands BEFORE anything below re-derives it — snapshotted here
    // because a re-discovery replaces the whole `Site` and that is one of the two ways it
    // moves. Both ways have to be compared against the same "before", or the rebuild
    // selection below silently doesn't apply to one of them.
    let targets_before = project.site.lock().xref_targets.clone();

    // What this batch changed of what discovery reads ([`Project::records`]), decided by
    // content: an editor that renames a temp file over the page, or a `git checkout` that
    // unlinks and recreates it, saved an existing page like any other editor.
    //
    // A page's front matter or leading `# H1` moved, so the project's view of that page
    // did: its listing card, its nav label, the prev/next either side of it, a book's
    // drawer label and every later chapter's number are all rendered by OTHER pages out of
    // `Site::pages`, which only discovery writes. Re-discovering rebuilds the
    // cross-reference registry and the search index as a side effect, so the
    // `refresh_xrefs` below is skipped then.
    //
    // A re-discovery (this, or a page added, renamed or deleted) reloads the tabs whose
    // chrome it moved and rebuilds every other open page. Discovery first, then the swap:
    // the site lock is not held across it.
    let moved = project.what_moved(changed);
    let rediscovered = moved.page_set || moved.records;
    // Rebuild only pages a tab is watching and that depend on a change.
    let mut open = watched_pages(project);
    if rediscovered {
        let before = shell_digests(project, &open);
        let new = project.rediscover();
        project.adopt(new, &moved.digests, false);
        // The chrome is outside `#tali-root`, where no block op reaches, so a tab whose
        // chrome moved (a book's drawer and pager naming a retitled chapter, a pager next
        // to a page added or removed) reloads, and so does a tab whose page is gone. Every
        // other tab keeps its DOM and its live state (an open `<details>`, a playing video,
        // a `{js}` widget) and takes what moved in its body as ops below. Reloading every
        // tab instead was correct but threw that state away, and rebuilding every tab alone
        // left each one on the old chrome with nothing sent at all (audit 2026-09-24 C5).
        let after = shell_digests(project, &open);
        let stale: Vec<String> = open
            .iter()
            .filter(|rel| after.get(*rel).is_none_or(|d| before.get(*rel) != Some(d)))
            .cloned()
            .collect();
        reload_tabs(project, &stale);
        open.retain(|rel| !stale.contains(rel));
    }

    let mut to_rebuild: Vec<String> = if rediscovered {
        // Every open page renders some part of the moved page's metadata or of the page
        // set, or could: a listing card, a nav label, a prev/next arrow. The set is the
        // pages a tab is watching, so this is a handful of renders on an edit that is rare
        // next to body edits, and it is the same shape as the moved-anchor rebuild below.
        open.clone()
    } else {
        // A page depends on every file its last render read or looked for
        // ([`PageDoc::reads`]), recorded by the read sites themselves rather than re-derived
        // from its source: two re-derivations (the include walk, the front-matter
        // `bibliography:`) plus a special case for `_site.yml`'s shared one each missed a
        // file the render read, so a shared `.bib` declared before it existed, an image
        // added or re-exported at a new size, never rebuilt the page that showed it (audit
        // 2026-09-24 C4, C7). A changed directory takes every file under it along.
        //
        // A file the page only looked at (an image) is asked about first: it can be the
        // page's own output ([`probes_moved`]).
        let changed: Vec<PathBuf> = changed.iter().map(|p| record_key(p)).collect();
        let mut read = Vec::new();
        let mut ask = Vec::new();
        let pages = project.pages.lock();
        for rel in &open {
            let Some(ps) = pages.get(rel.as_str()) else {
                continue;
            };
            let (text, probed): (Vec<_>, Vec<_>) = ps
                .doc
                .reads
                .iter()
                .filter(|(path, _)| changed.iter().any(|c| path.starts_with(c)))
                .partition(|(_, access)| **access == taliesin_core::reads::Access::Read);
            if !text.is_empty() {
                read.push(rel.clone());
            } else if !probed.is_empty() {
                let probed = probed.into_iter().map(|(path, _)| path.clone()).collect();
                ask.push((rel.clone(), probed));
            }
        }
        // Queued with the pages lock released: routing reads it.
        drop(pages);
        for (rel, probed) in ask {
            app.queue_if_moved(rel, probed);
        }
        read
    };
    // Re-derive the cross-reference registry FIRST: everything below reads it, and both its
    // producers ran only at discovery, so a warm preview froze every cross-page number at
    // startup. Measured on a live server: after inserting a figure above the referenced one,
    // `intro.html` served "Figure 1.2" while `methods.html` served "Figure 1.1" for that
    // same figure, and an anchor added after startup rendered as a dead same-page link.
    //
    // Gated on the CHANGED FILES, deliberately not on `to_rebuild`: that list is the *open
    // tabs* whose own sources moved, and registry staleness has nothing to do with which
    // tabs are open. Gating it there looked right and did nothing — with no tab open the
    // refresh never ran at all, and editing `intro.tmd` while only `methods.html` was open
    // left the registry rotting, which is the exact cross-page case this fixes. A cross-page
    // ref is precisely the dependency `to_rebuild` cannot see.
    //
    // A `.tmd` or `.md` only: an anchor can be created or renumbered by a page source or an
    // `{{< include >}}` partial, which is as often a `.md` as a `.tmd` (a `.md` partial was
    // missed until 2026-09-24, leaving every citing page one number off: audit C6), never by
    // a `.bib`/`.css`/image, which the dependency walk above also feeds us. A re-discovery above already rebuilt the
    // registry, so refreshing again would just burn the pass twice.
    //
    // Under the lock, unlike the per-page render below: this is the whole-site pass and the
    // pages rebuilt after it MUST see the fresh registry. A re-scan plus one render per page,
    // no code execution, so it is O(pages) on every save: 3.2ms on the largest real book
    // (`docs/guide`, 16 pages) re-measured 2026-08-27, ~0.2ms per page wall-clock across
    // cores, which extrapolates to ~0.2s at 200 heavy pages (it was 47.6ms / ~2.5s before
    // 1.1.0's render memos and concurrent harvest). `tools/live-edit-bench` carries the
    // number per project so this comment cannot drift the way its "27ms / 20 pages"
    // predecessor did.
    // `refresh_xrefs` is all-or-nothing about a render panic, so a bad page cannot leave the
    // registry un-numbered site-wide; the guard here is belt-and-braces for this task, which
    // (unlike `build_page`) has none of its own.
    //
    // NOT after a re-discovery above, which rebuilds the registry as a side effect, so
    // refreshing again would burn the whole pass twice.
    let touches_source = changed.iter().any(|p| {
        p.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("tmd") || e.eq_ignore_ascii_case("md"))
    });
    if touches_source && !rediscovered {
        let refreshed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            project.site.lock().refresh_xrefs();
        }));
        if refreshed.is_err() {
            crate::log::warn("cross-reference refresh panicked; numbers may be stale");
        }
    }

    // A moved target is a dependency the walk above CANNOT see, and re-deriving the registry
    // fixes nothing a reader can read without this: `methods.tmd` names no file that changed,
    // so it is absent from `to_rebuild` and keeps serving its cached body — measured with the
    // registry provably holding "1.2" while the open tab still showed "Figure 1.1".
    //
    // Deliberately OUTSIDE the gate on the refresh above, which is the subtler half of that
    // same measurement. The registry moves on BOTH paths, `refresh_xrefs` here and a
    // re-discovery above, so gating this on the refresh skips the reader-visible half
    // exactly when the save re-discovered. Reproduced when a delete+recreate still
    // re-discovered: it served "Figure 1.2" from `intro.html` while the open `methods.html`
    // tab sat on "Figure 1.1".
    //
    // There is no project-wide reverse index anymore (the "Referenced by" backlinks it
    // drove were deleted 2026-08-04), so this diffs the registry PER ANCHOR — which
    // targets actually moved (renumbered, moved to a different page, inserted, or
    // removed), not "the registry changed somewhere" — and asks each OPEN page's own
    // already-rendered blocks whether it cites one of them, via `xref_anchors_in`. That
    // reads `PageState.doc.blocks`, already in memory: no re-render, no revived
    // site-wide index, scoped to exactly the pages that were open. It works on a
    // FINISHED page's blocks (post cross-ref resolution, where a resolved cross-page
    // marker is gone) because `cite` always emits `href="#{anchor}"` and the
    // site-level rewrite only ever changes the prefix before `#`, never the anchor
    // itself — same-page, resolved cross-page, and still-unresolved links all recover
    // the same way. `moved_anchors` empty <=> the two registries are equal, so `moved`
    // means exactly what the old `site.xref_targets != targets_before` check meant.
    let moved_anchors: HashSet<String> = {
        let site = project.site.lock();
        site.xref_targets
            .iter()
            .filter(|(anchor, target)| targets_before.get(anchor.as_str()) != Some(*target))
            .map(|(anchor, _)| anchor.clone())
            .chain(
                targets_before
                    .keys()
                    .filter(|anchor| !site.xref_targets.contains_key(anchor.as_str()))
                    .cloned(),
            )
            .collect()
    };
    let moved = !moved_anchors.is_empty();
    if moved {
        // Never held alongside `site.lock()` above (the established lock order in this
        // file: release `site` before taking `pages`).
        let pages = project.pages.lock();
        for rel in pages_citing_a_moved_anchor(&pages, &open, &moved_anchors) {
            if !to_rebuild.contains(&rel) {
                to_rebuild.push(rel);
            }
        }
    }
    for rel in to_rebuild {
        app.queue_build(rel);
    }
    // The Cmd-K index is GLOBAL (one `search-index.js` for every tab), so a per-page
    // refresh keyed on the open tabs cannot keep it true: a renumbered figure would go stale
    // in the fragments of every page nobody happens to have open, and Cmd-K would surface a
    // snippet contradicting the page it links to. The index is rebuilt whole, and only on a
    // real anchor move; a prose edit reaches the palette on the next discovery.
    //
    // Last, and on a copy of the site rather than under its lock: it renders every page, and
    // the builds queued above, the pages a reader is watching, take that lock to render, so
    // their ops waited behind a whole-project pass. Only this task writes the `Site`, so the
    // copy is the site in force when it is put back.
    if moved {
        let mut site = project.site.lock().clone();
        site.rebuild_search_index();
        *project.site.lock() = site;
    }
}

/// The pages a tab is watching, after dropping the state of every other page.
///
/// A page's state outlives its tab: a GET creates one, and every page a reader ever opened
/// kept one. Rebuilding those on every save made a front-matter edit cost one render per
/// page ever visited (2544 ms after one visit of each of 221 pages, audit 2026-09-24
/// invalidation #9). A page nobody watches renders fresh on its next visit instead, as a
/// page never visited does; a build of it already queued finds no state and publishes
/// nothing (see `build_page`).
fn watched_pages(project: &Project) -> Vec<String> {
    let mut pages = project.pages.lock();
    pages.retain(|_, ps| ps.tx.receiver_count() > 0);
    pages.keys().cloned().collect()
}

/// Rebuild the project against a batch of changed files.
fn dispatch_changes(app: &SiteApp, changed: &HashSet<PathBuf>) {
    let project = app.root.clone();
    rebuild_project(app, &project, changed);
}

/// Reload every open tab ([`reload_tabs`]), after a `_site.yml` change.
fn reload_open_tabs(project: &Project) {
    let all: Vec<String> = project.pages.lock().keys().cloned().collect();
    reload_tabs(project, &all);
}

/// Reload the tabs open on `rels` and drop those pages' cached block state, so each reload
/// re-renders fresh against the re-discovered site. The reload message is delivered before
/// the channel's sender is dropped.
fn reload_tabs(project: &Project, rels: &[String]) {
    if rels.is_empty() {
        return;
    }
    let mut pages = project.pages.lock();
    for rel in rels {
        if let Some(ps) = pages.remove(rel) {
            let _ = ps.tx.send(protocol::reload());
        }
    }
    crate::log::update(0);
}

/// Each watched page's [`shell_digest`] against the site in force. A page the site no
/// longer has is left out.
fn shell_digests(project: &Project, open: &[String]) -> HashMap<String, u64> {
    // Each tab's own parts first, then the site: the two locks are never held together.
    let docs: Vec<(String, PageDoc)> = {
        let pages = project.pages.lock();
        open.iter()
            .filter_map(|rel| {
                let ps = pages.get(rel)?;
                let doc = PageDoc {
                    toc: ps.doc.toc,
                    includes: ps.doc.includes.clone(),
                    ..PageDoc::default()
                };
                Some((rel.clone(), doc))
            })
            .collect()
    };
    let site = project.site.lock();
    docs.into_iter()
        .filter_map(|(rel, doc)| {
            let page = site.page(&rel)?;
            Some((rel, shell_digest(&site, &project.dir, page, &doc)))
        })
        .collect()
}

#[cfg(test)]
mod protocol_contract {
    //! Locks the websocket message/op shapes the preview client consumes
    //! (web-client/client.js `@typedef` block). If a field name or `type` tag
    //! changes here, update the client's typedefs too — these are the two halves
    //! of one contract. The `serve.rs` producers are covered by a sibling test.
    use super::*;
    use crate::testutil::parse;
    use taliesin_core::{BlockOp, render_document};

    #[test]
    fn op_messages_match_client_contract() {
        let up = parse(op_json(
            &BlockOp::Update {
                target_id: "b1".into(),
                html: "<p>x</p>".into(),
            },
            7,
        ));
        assert_eq!(up["type"], "update");
        assert_eq!(up["target_id"], "b1");
        assert!(up.get("html").is_some());
        // Every op carries the resulting render generation so the client can track it
        // and skip a destructive re-mount on a byte-identical reconnect.
        assert_eq!(up["gen"], 7);

        let ins = parse(op_json(
            &BlockOp::Insert {
                after_id: Some("b1".into()),
                html: "<p>y</p>".into(),
            },
            7,
        ));
        assert_eq!(ins["type"], "insert");
        assert!(ins.get("after_id").is_some());
        assert!(ins.get("html").is_some());
        assert_eq!(ins["gen"], 7);

        let rm = parse(op_json(
            &BlockOp::Remove {
                target_id: "b2".into(),
            },
            7,
        ));
        assert_eq!(rm["type"], "remove");
        assert_eq!(rm["target_id"], "b2");
        assert_eq!(rm["gen"], 7);
    }

    #[test]
    fn set_meta_message_matches_client_contract() {
        // `set_meta` is the click-to-source mechanism and the most-emitted op by far:
        // live-edit-bench measures a real edit as 55 ops, 53 of them set_meta. It was
        // the one op with no shape test, so renaming a key here compiled, passed the
        // whole suite AND `tsc`, and silently degraded Ctrl-click to "opens at line 1"
        // for every line-shifted block. The client reads exactly these keys
        // (client.js `case "set_meta"`); they are the two halves of one contract.
        let sm = parse(op_json(
            &BlockOp::SetMeta {
                target_id: "b3".into(),
                sourcepos: "12:1-14:9".into(),
                source_file: Some("inc/part.tmd".into()),
                inner: vec!["13:1-13:4".into()],
            },
            7,
        ));
        assert_eq!(sm["type"], "set_meta");
        assert_eq!(sm["target_id"], "b3");
        assert_eq!(sm["gen"], 7);
        // The client feeds `sourcepos` straight to `data-sourcepos`, and `openSource`
        // parses it with /^(\d+):(\d+)/ — a rename lands the editor on line 1 instead.
        assert_eq!(sm["sourcepos"], "12:1-14:9");
        // `source_file` attributes an included block to its real file; a rename makes
        // click-to-source open the WRONG file.
        assert_eq!(sm["source_file"], "inc/part.tmd");
        // `inner` carries a container's inner positions, patched onto its descendants in
        // order; a rename makes the client resync on every shift above a container.
        assert_eq!(sm["inner"], serde_json::json!(["13:1-13:4"]));

        // A non-included block must emit source_file as JSON null (the client's
        // `if (msg.source_file)` is falsy for it and removes the attribute), not omit
        // the key and not emit the string "null".
        let plain = parse(op_json(
            &BlockOp::SetMeta {
                target_id: "b4".into(),
                sourcepos: "3:1-3:5".into(),
                source_file: None,
                inner: Vec::new(),
            },
            8,
        ));
        assert!(plain.get("source_file").is_some(), "key present");
        assert!(plain["source_file"].is_null(), "and is JSON null");
    }

    #[test]
    fn op_json_rewrites_tmd_links_in_block_html() {
        let up = parse(op_json(
            &BlockOp::Update {
                target_id: "b1".into(),
                html: "<a href=\"blog.tmd\">b</a>".into(),
            },
            1,
        ));
        assert_eq!(up["html"], "<a href=\"blog.html\">b</a>");
    }

    #[test]
    fn a_real_edit_serializes_to_one_update_with_links_rewritten() {
        // The full chain a previewing client receives: render two versions of a
        // page, diff them, and serialize. `tests/incremental.rs` covers render->
        // diff in core; this proves the serve-side serialization (incl. the
        // .tmd->.html rewrite that happens *in* op_json, not at render time).
        let v1 = render_document("Intro.\n\nSee [post](other.tmd).\n");
        let v2 = render_document("Intro.\n\nSee [the post](other.tmd) now.\n");
        let ops = diff_blocks(&v1.blocks, &v2.blocks);
        assert_eq!(ops.len(), 1, "one paragraph edit -> one op: {ops:?}");

        let msg = parse(op_json(&ops[0], 1));
        assert_eq!(msg["type"], "update");
        assert_eq!(
            msg["target_id"].as_str().unwrap(),
            v1.blocks[1].id.as_str(),
            "update must target the edited block's existing id"
        );
        let html = msg["html"].as_str().unwrap();
        assert!(
            html.contains("other.html"),
            "tmd link not rewritten: {html}"
        );
        assert!(!html.contains("other.tmd"), "raw .tmd link leaked: {html}");
    }

    #[test]
    fn full_render_title_is_the_display_ready_tab_title() {
        // The client assigns `full_render`'s title straight to `document.title`
        // (client.js `case "full_render"`), ABOVE its skipMount guard — so this field is
        // not "the doc's title", it is "what the tab must read", and whatever we put here
        // overwrites the `<title>` the server already rendered. It used to carry the raw
        // front-matter title, which quietly downgraded the tab two ways. Nobody caught it
        // by eye because a page with code cells self-heals (the exec pass's
        // `build-state: idle` restores baseTitle); the quiet prose chapters do not.
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");

        // 1. An inner page with a front-matter `title:` keeps the " · {site}" suffix that
        //    the server-rendered `<title>` applied. Without it /blog.html's tab dropped
        //    back to a bare "Blog" the moment the websocket connected.
        let site = Site::discover(&corpus.join("tech-blog"));
        let page = site
            .page("blog.tmd")
            .expect("corpus/tech-blog/blog.tmd")
            .clone();
        let fr = parse(full_render_json(&render_markdown_only(&site, &page)));
        assert_eq!(fr["title"], "Blog · Andreas Bogossian");

        // 2. A titleless page takes its leading `# H1` — the same fallback `Page.title`
        //    already resolves. The wire used to carry the front-matter title verbatim, so
        //    this was null and the client's `msg.title || "Taliesin"` literally tabbed the
        //    tool's name over the chapter's: 5 of corpus/demo-book's 6 chapters.
        let site = Site::discover(&corpus.join("demo-book"));
        let page = site
            .page("intro.tmd")
            .expect("corpus/demo-book/intro.tmd")
            .clone();
        let fr = parse(full_render_json(&render_markdown_only(&site, &page)));
        assert_eq!(fr["title"], "Introduction · A Short Demo Book");

        // 3. The home page stays bare (no "Name · Name"), i.e. the suffix policy is
        //    applied by `title_with_site_suffix`, not re-decided here.
        let page = site
            .page("index.tmd")
            .expect("corpus/demo-book/index.tmd")
            .clone();
        let fr = parse(full_render_json(&render_markdown_only(&site, &page)));
        assert_eq!(fr["title"], "Preface");
    }

    #[test]
    fn lifecycle_messages_match_client_contract() {
        let fr = parse(full_render_json(&PageDoc::default()));
        assert_eq!(fr["type"], "full_render");
        assert!(fr.get("title").is_some()); // present (null allowed)
        assert!(fr.get("body_html").is_some());
        assert!(fr["gen"].is_u64(), "full_render must carry a numeric gen");
        assert!(
            fr["boot"].is_u64(),
            "full_render must carry a numeric boot id"
        );
        assert!(fr["diagnostics"].is_array());

        let dg = parse(protocol::diagnostics(&[diag_from(
            &taliesin_core::render::Warning::new("x"),
            "p.tmd",
        )]));
        assert_eq!(dg["type"], "diagnostics");
        assert_eq!(dg["messages"][0]["level"], "warning");
        assert_eq!(dg["messages"][0]["message"], "x");

        let err = parse(protocol::error("boom"));
        assert_eq!(err["type"], "error");
        assert_eq!(err["message"], "boom");

        assert_eq!(parse(protocol::reload())["type"], "reload");
    }

    #[test]
    fn a_page_rel_survives_the_ws_query_intact() {
        // The ws url is the ONLY thing that tells the socket which page it is for, and a
        // rel that arrives changed names no page: `client_conn` refuses the key and
        // `client.js` reconnects every second forever behind a page that rendered 200 with
        // a green pill. So the encoding must be lossless for every character a filename
        // can hold, not for the space alone.
        //
        // Each of these is a distinct failure mode of the space-only encoding: `&` ends
        // the query parameter (axum sees `page=q`), `+` decodes back as a space, `#`
        // truncates the url at the fragment, and a bare `%` either eats the next two
        // characters or is malformed.
        for rel in [
            "q&a.tmd",
            "c++ notes.tmd",
            "100% done.tmd",
            "a#b.tmd",
            "posts/q&a/index.tmd",
            "café.tmd",
        ] {
            let encoded = encode_query(rel);
            assert!(
                !encoded.contains(['&', '+', '#', '?', ' ', '"', '<']),
                "`{rel}` still carries a character that changes the url: {encoded}"
            );
            assert_eq!(
                crate::serve::percent_decode(&encoded),
                rel,
                "`{rel}` must survive the round trip through the query"
            );
        }
        // A rel's own separators stay readable — they are query-safe and appear in every
        // multi-page url.
        assert_eq!(encode_query("posts/my-post.tmd"), "posts/my-post.tmd");
    }
}

#[cfg(test)]
mod project_tests {
    //! The per-page routing seam, pinned without a `Site`/kernel; the live wiring on top
    //! is browser-verified (no live-HTTP harness).
    use super::*;

    /// A17. `SiteApp::interrupt` is ONE pool-wide pid, and the `restart_kernel` arm SIGINTs
    /// whatever it holds. That is deliberate and load-bearing — the exec lane is serial, so
    /// when page A's runaway cell wedges the queue, page B's own Restart is queued behind
    /// that same build and only the server-wide SIGINT can unwedge it — but it means a
    /// restart on B can kill a cell running on A. Reproduced live: A's 45 s cell died with
    /// `KeyboardInterrupt` about 1 s after B sent `restart_kernel`, and A was left holding
    /// the traceback with nothing anywhere saying why.
    ///
    /// So the fix is not a page-equality check (that would restore the wedge): it is to
    /// name the collateral. This is the decision that separates "I aborted my own cell,
    /// which is what restart means" from "I took someone else's".
    #[test]
    fn a_restart_reports_only_a_cell_it_took_from_another_page() {
        assert_eq!(
            cross_page_victim("b.tmd", "a.tmd", 4242).as_deref(),
            Some("a.tmd"),
            "another page's cell died for this restart, and that must be said"
        );
        assert_eq!(
            cross_page_victim("a.tmd", "a.tmd", 4242),
            None,
            "aborting your OWN running cell is exactly what restarting your kernel means"
        );
        assert_eq!(
            cross_page_victim("b.tmd", "a.tmd", 0),
            None,
            "a pid of 0 means nothing was executing, so nothing was taken"
        );
        assert_eq!(
            cross_page_victim("b.tmd", "", 4242),
            None,
            "the exec lane is idle: the pid is stale, not another page's"
        );
    }

    /// E6: "Restart kernel" used to SIGINT the running cell even when it was the
    /// requester's own, whose kernel the restart discards anyway. The interrupt stopped
    /// that one cell and the build went on to run every cell after it, in the old kernel
    /// and against interrupted state, before the restart could start: minutes, with long
    /// cells. The requester's own kernel is killed, so the run fails fast; another page's
    /// cell is still only interrupted (A17), since its kernel is not being discarded.
    #[test]
    fn a_restart_kills_its_own_kernel_and_only_interrupts_another_pages() {
        assert_eq!(restart_stop(4242, None), Some(Stop::Kill));
        assert_eq!(restart_stop(4242, Some("a.tmd")), Some(Stop::Interrupt));
        assert_eq!(restart_stop(0, None), None, "nothing is executing");
    }

    /// The notice has to reach the page that lost the cell, on the very build that shows
    /// the traceback — and never on some later, unrelated rebuild of it.
    #[test]
    fn an_interrupt_notice_reaches_the_page_it_names_exactly_once() {
        let mut lane = ExecLane {
            page: "a.tmd".into(),
            interrupted_by: Some(("a.tmd".into(), "b.tmd".into())),
        };
        assert_eq!(
            lane.take_interrupt_for("c.tmd"),
            None,
            "a bystander page must not eat the notice"
        );
        assert_eq!(lane.take_interrupt_for("a.tmd").as_deref(), Some("b.tmd"));
        assert_eq!(
            lane.take_interrupt_for("a.tmd"),
            None,
            "and it is delivered once, not on every later build"
        );
        // The reader has to be able to act on it, so it names who took the cell and what
        // brings the output back.
        let notice = interrupted_notice("b.tmd");
        assert!(
            notice.contains("b.tmd"),
            "names the page that asked: {notice}"
        );
        assert!(
            notice.contains("restart"),
            "names what took the cell: {notice}"
        );
    }

    /// A first build must put the page on screen BEFORE it waits for the kernel.
    ///
    /// `build_page` renders the markdown, then awaits `exec.run` for ALL cells, and only
    /// then publishes. A page reached over the websocket has no state yet, so its opening
    /// snapshot is a `full_render` over an EMPTY doc and the reader watches a bare navbar
    /// for as long as the slowest cell takes — measured at 20 s on a page with one 25 s
    /// cell, no spinner, no status, while the prose that needs no kernel at all sat
    /// rendered in memory (audit finding 02, 2026-08-09).
    ///
    /// Driven through the broadcast channel rather than a real socket: the channel IS the
    /// publish mechanism and the websocket is a pipe onto it, and this crate has no
    /// live-HTTP harness (backlog item 10; wave 6 removed the browser net). Gated on a
    /// live kernel, because the defect only exists when a cell actually takes time.
    #[test]
    fn a_first_build_publishes_the_body_before_the_cells_finish() {
        if std::env::var_os("TALIESIN_PYTHON").is_none() {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 exercise the pre-exec publish; this run did not."
            );
            return;
        }
        let dir = std::env::temp_dir().join(format!("tali-preexec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        // The sleep must outlast the assertion deadline below by enough that "published
        // early" and "published at the end" cannot be confused for one another.
        //
        // The marker is CONCATENATED in the cell rather than written as one literal: a
        // pre-exec publish renders the cell as source, so a plain `print('CELL-OUTPUT')`
        // would put the needle on screen in the very paint that is supposed to prove the
        // output is absent, and the test would fail against a correct implementation.
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Slow\n---\n\nPROSE-BEFORE-THE-KERNEL\n\n\
             ```{python}\nimport time\ntime.sleep(8)\nprint('CELL' + '-' + 'OUTPUT')\n```\n",
        )
        .unwrap();

        let site = taliesin_core::site::Site::discover(&dir);
        let (tx, mut rx) = broadcast::channel(256);
        let mut pages = HashMap::new();
        // Exactly what `client_conn` allocates for a page the websocket reaches first:
        // a live channel over a doc with no blocks at all.
        pages.insert(
            "index.tmd".to_string(),
            PageState {
                doc: PageDoc::default(),
                tx,
            },
        );
        let project = Arc::new(Project {
            dir: dir.clone(),
            site: parking_lot::Mutex::new(site),
            pages: parking_lot::Mutex::new(pages),
            exec_lane: Mutex::new(ExecLane::default()),
            scope: None,
            records: Mutex::new(HashMap::new()),
        });

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let py = {
                let s = project.site.lock();
                crate::interpreter::resolve_python(s.config.python.as_deref(), &project.dir)
            };
            let mut pool = ExecPool::new(
                dir.join("_freeze"),
                py,
                Arc::new(std::sync::atomic::AtomicU32::new(0)),
            );
            let p = project.clone();
            let build =
                tokio::spawn(async move { build_page(&p, "index.tmd", Some(&mut pool)).await });

            let body = tokio::time::timeout(std::time::Duration::from_secs(4), async {
                loop {
                    let m = rx.recv().await.expect("the page channel stays open");
                    if m.contains("PROSE-BEFORE-THE-KERNEL") {
                        return m;
                    }
                }
            })
            .await
            .expect("the body must reach the client before the cell finishes");

            assert!(
                !body.contains("CELL-OUTPUT"),
                "this is the PRE-exec publish, so the cell is still source here: {body}"
            );
            build.await.unwrap();
        });

        // …and the finished build still carries the output, so the early publish added a
        // paint rather than replacing one.
        let final_body = project.pages.lock()["index.tmd"].doc.body_html();
        assert!(
            final_body.contains("CELL-OUTPUT"),
            "the post-exec publish must still splice the output in: {final_body}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// E7: the cell the author just edited must reach the page, as its new code block,
    /// BEFORE it runs. Its content hash (so its block id) changed with the edit, and the new
    /// block only arrived with the post-exec publish, so the client's `running` badge and
    /// live output (`openLiveOutput` looks the cell up by that id) had nothing to attach to:
    /// only unedited downstream cells streamed, never the one being watched. On a rebuild the
    /// edited cells now go out as `update` ops first, with the old output removed, so the
    /// live output streams into a block the client already holds.
    #[test]
    fn an_edited_cell_reaches_the_page_before_it_runs() {
        if std::env::var_os("TALIESIN_PYTHON").is_none() {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 exercise the pre-exec publish of an edited cell; this run did not."
            );
            return;
        }
        let dir = std::env::temp_dir().join(format!("tali-editedcell-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        let doc =
            |cell: &str| format!("---\ntitle: E\n---\n\nProse.\n\n```{{python}}\n{cell}\n```\n");
        std::fs::write(dir.join("index.tmd"), doc("print('v' + '1')")).unwrap();
        let site = taliesin_core::site::Site::discover(&dir);
        let (tx, _) = broadcast::channel(4096);
        let mut pages = HashMap::new();
        pages.insert(
            "index.tmd".to_string(),
            PageState {
                doc: PageDoc::default(),
                tx: tx.clone(),
            },
        );
        let project = Arc::new(Project {
            dir: dir.clone(),
            site: parking_lot::Mutex::new(site),
            pages: parking_lot::Mutex::new(pages),
            exec_lane: Mutex::new(ExecLane::default()),
            scope: None,
            records: Mutex::new(HashMap::new()),
        });
        let rt = tokio::runtime::Runtime::new().unwrap();
        let msgs: Vec<serde_json::Value> = rt.block_on(async {
            let py = {
                let s = project.site.lock();
                crate::interpreter::resolve_python(s.config.python.as_deref(), &project.dir)
            };
            let mut pool = ExecPool::new(
                dir.join("_freeze"),
                py,
                Arc::new(std::sync::atomic::AtomicU32::new(0)),
            );
            build_page(&project, "index.tmd", Some(&mut pool)).await;
            std::fs::write(
                dir.join("index.tmd"),
                doc("import time\nprint('EDITED' + '-RUN', flush=True)\ntime.sleep(1)"),
            )
            .unwrap();
            let mut rx = tx.subscribe();
            build_page(&project, "index.tmd", Some(&mut pool)).await;
            let mut out = Vec::new();
            while let Ok(m) = rx.try_recv() {
                out.push(serde_json::from_str(&m).unwrap());
            }
            out
        });
        let _ = std::fs::remove_dir_all(&dir);
        let running = msgs
            .iter()
            .position(|m| m["type"] == "cell-state" && m["state"] == "running")
            .expect("the edited cell ran");
        let cell_id = msgs[running]["cell_id"].as_str().unwrap().to_string();
        let shown = msgs.iter().position(|m| {
            m["type"] == "update"
                && m["html"]
                    .as_str()
                    .is_some_and(|h| h.contains(&format!("data-block-id=\"{cell_id}\"")))
        });
        assert!(
            shown.is_some_and(|i| i < running),
            "the edited cell's new block did not reach the page before it ran ({shown:?} vs \
             running at {running}), so its badge and live output had nothing to attach to"
        );
    }

    /// The pairing behind [`publish_edited_cells`]: an in-place edit pairs; a cell added,
    /// removed or moved does not, and then nothing is sent before the run.
    #[test]
    fn only_an_edit_in_place_pairs_a_new_cell_with_the_block_it_replaces() {
        let b = |id: &str, cell: bool| Block {
            id: id.into(),
            sourcepos: String::new(),
            source_file: None,
            html: String::new(),
            cell: cell.then(|| taliesin_core::render::Cell {
                lang: "python".into(),
                code: String::new(),
                figure: None,
                table: None,
                echo: true,
                include: true,
                cache: true,
                js: Default::default(),
            }),
            nested: Vec::new(),
        };
        let on_screen = [
            b("p", false),
            b("c1", true),
            b("c1-out", false),
            b("c2", true),
        ];
        let is_cell = |x: &Block| x.cell.is_some();
        assert_eq!(
            edited_cells(
                &on_screen,
                &[b("p", false), b("c1x", true), b("c2", true)],
                is_cell
            ),
            Some(vec![(1, 1)]),
            "c1 edited in place"
        );
        assert_eq!(
            edited_cells(
                &on_screen,
                &[b("p", false), b("c1", true), b("c2", true)],
                is_cell
            ),
            Some(vec![]),
            "nothing edited"
        );
        assert_eq!(
            edited_cells(
                &on_screen,
                &[b("c1", true), b("c2", true), b("c3", true)],
                is_cell
            ),
            None,
            "a cell added"
        );
        assert_eq!(
            edited_cells(&on_screen, &[b("c2", true), b("c1", true)], is_cell),
            None,
            "two cells swapped"
        );
    }

    /// The gate that decides whether a save touched what DISCOVERY reads.
    ///
    /// It has to answer both ways, and each answer costs something different. A missed
    /// change is the defect this exists for: a listed post's `title:` never reached the
    /// index listing, on save or on reload, so the preview contradicted `build` until the
    /// server was restarted. A false positive is a re-discovery on a keystroke, and
    /// discovery renders every page twice, so a body edit, which is nearly every edit, must
    /// not trip it.
    #[test]
    fn a_save_moves_the_record_only_when_what_discovery_reads_moved() {
        let dir = scratch("record");
        std::fs::create_dir_all(dir.join("posts")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nBody.\n").unwrap();
        let post = dir.join("posts/first.tmd");
        std::fs::write(
            &post,
            "---\ntitle: First\n---\n\n# First\n\nOriginal body.\n",
        )
        .unwrap();
        let (project, _app, _b, _f) = project_and_app(&dir);
        let changed: HashSet<PathBuf> = std::iter::once(post.clone()).collect();
        let moved = |project: &Project, changed: &HashSet<PathBuf>| {
            let m = project.what_moved(changed);
            (m.page_set, m.records)
        };

        // Nothing written yet: the record already describes what is on disk.
        assert_eq!(
            moved(&project, &changed),
            (false, false),
            "no edit, no move"
        );

        // A body edit leaves what discovery reads byte-identical.
        std::fs::write(&post, "---\ntitle: First\n---\n\n# First\n\nRewritten.\n").unwrap();
        assert_eq!(
            moved(&project, &changed),
            (false, false),
            "a body edit must not cost a re-discovery"
        );

        // The `title:` a listing card renders, and the heading a chapter is named by.
        std::fs::write(&post, "---\ntitle: Renamed\n---\n\n# First\n\nRewritten.\n").unwrap();
        assert_eq!(moved(&project, &changed), (false, true), "title: moved");
        std::fs::write(&post, "---\ntitle: First\n---\n\n# Second\n\nRewritten.\n").unwrap();
        assert_eq!(moved(&project, &changed), (false, true), "the H1 moved");

        // Once the re-discovery it caused is adopted, the same content is no move.
        let m = project.what_moved(&changed);
        project.adopt(project.rediscover(), &m.digests, false);
        assert_eq!(moved(&project, &changed), (false, false), "record updated");

        // A page that is gone may change the page set.
        std::fs::remove_file(&post).unwrap();
        assert_eq!(moved(&project, &changed), (true, false), "page deleted");

        // A source file discovery has never classified may be a new page, once; after the
        // re-discovery finds it is none, saving it costs nothing.
        let partial = dir.join("_includes/part.tmd");
        std::fs::create_dir_all(partial.parent().unwrap()).unwrap();
        std::fs::write(&partial, "---\ntitle: Not a page\n---\n\nx\n").unwrap();
        let saved: HashSet<PathBuf> = std::iter::once(partial.clone()).collect();
        assert_eq!(moved(&project, &saved), (true, false), "never classified");
        let m = project.what_moved(&saved);
        project.adopt(project.rediscover(), &m.digests, false);
        std::fs::write(&partial, "---\ntitle: Still not a page\n---\n\ny\n").unwrap();
        assert_eq!(
            moved(&project, &saved),
            (false, false),
            "an include partial is not a page of the site"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 D1. An HTML comment emits no element, so its block id is in the
    /// block list and nowhere in the DOM. A paragraph typed right under it arrived as an
    /// `Insert` anchored on that id; the client found no anchor and put the paragraph above
    /// the title. Such a burst must reach the client as one `full_render`, while an ordinary
    /// edit on the same page still travels as block ops.
    #[test]
    fn an_edit_anchored_on_a_comment_is_sent_as_a_full_render() {
        let dir = scratch("comment-anchor");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        let page = dir.join("index.tmd");
        let v1 = "---\ntitle: C\n---\n\nFirst.\n\n<!-- TODO -->\n\nLast.\n";
        std::fs::write(&page, v1).unwrap();
        let (project, _app, _b, _f) = project_and_app(&dir);
        open_page(&project, "index.tmd");
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(build_page(&project, "index.tmd", None));
        let mut rx = project.pages.lock()["index.tmd"].tx.subscribe();
        let mut burst_after = |src: &str| -> Vec<String> {
            std::fs::write(&page, src).unwrap();
            rt.block_on(build_page(&project, "index.tmd", None));
            std::iter::from_fn(|| rx.try_recv().ok())
                .map(|m| {
                    crate::testutil::parse(m)["type"]
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                })
                .collect()
        };

        let typed = v1.replace("<!-- TODO -->\n", "<!-- TODO -->\n\nNew paragraph.\n");
        let types = burst_after(&typed);
        assert!(
            types.iter().any(|t| t == "full_render") && !types.iter().any(|t| t == "insert"),
            "an insert anchored on a comment must be a full render: {types:?}"
        );

        let types = burst_after(&typed.replace("Last.", "Last, edited."));
        assert!(
            types.iter().any(|t| t == "update") && !types.iter().any(|t| t == "full_render"),
            "an ordinary paragraph edit stays a block op: {types:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // dos-pages: only a key the site actually resolves may reach the `PageState`
    // allocation. Everything else must come back `None`, which the ws handler refuses —
    // otherwise each bogus `?page=` permanently costs a 256-slot broadcast ring that only a
    // preview restart reclaims.
    //
    // This pins the *decision*; the socket path around it has no automated live-HTTP
    // harness (a known bin-crate gap, backlog item 10), so it was browser-verified instead.
    #[test]
    fn only_a_resolvable_page_key_gets_a_page_state() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/tarn");
        let project = Project {
            dir: dir.clone(),
            site: parking_lot::Mutex::new(taliesin_core::site::Site::discover(&dir)),
            pages: parking_lot::Mutex::new(HashMap::new()),
            exec_lane: Mutex::new(ExecLane::default()),
            scope: None,
            records: Mutex::new(HashMap::new()),
        };

        // A real page resolves, by source rel and by output url alike.
        assert_eq!(
            resolve_page_rel(&project, "install.tmd").as_deref(),
            Some("install.tmd")
        );
        assert_eq!(
            resolve_page_rel(&project, "install.html").as_deref(),
            Some("install.tmd"),
            "a url key normalises to the source rel"
        );

        // Everything a hostile or stale client can send resolves to nothing.
        for bogus in [
            "",
            "nope.tmd",
            "nope.html",
            "../../etc/passwd",
            "install.tmd/extra",
            "INSTALL.TMD",
            "a-fresh-key-every-reconnect-0001",
        ] {
            assert_eq!(
                resolve_page_rel(&project, bogus),
                None,
                "`{bogus}` names no page and must not earn a PageState"
            );
        }
        assert!(
            project.pages.lock().is_empty(),
            "resolving a key must never allocate"
        );
    }

    #[test]
    fn only_a_page_with_kernel_cells_takes_the_exec_lane() {
        // AP3-1's routing predicate. One builder task consumed the whole server's queue,
        // awaiting each page to completion, so it serialized on the wrong thing: a page
        // with no code cells needs no kernel, yet queued behind kernel work it would never
        // use. Measured on a two-page preview,
        // a cell-free page's prose edit landed in 0.11 s alone and 12.15 s (110x) when an
        // unrelated page was 1.2 s into a 12 s `{python}` cell.
        let render = |src: &str| {
            taliesin_core::render_document_scoped_with_site(src, Path::new("."), None, None).blocks
        };
        assert!(is_cell_free(&render("---\ntitle: T\n---\n\nJust prose.\n")));
        assert!(!is_cell_free(&render(
            "---\ntitle: T\n---\n\n```{python}\nprint(1)\n```\n"
        )));
        // `{js}` runs in the BROWSER, so a page full of reactive cells needs the kernel
        // lane exactly as much as a prose page does — which is most of what makes this
        // worth doing, since the explorable-explanation pages are the `{js}`-heavy ones.
        assert!(is_cell_free(&render(
            "---\ntitle: T\n---\n\n```{js}\nreturn 1;\n```\n"
        )));
        // A non-executing fenced block is not a cell at all.
        assert!(is_cell_free(&render(
            "---\ntitle: T\n---\n\n```python\nprint(1)\n```\n"
        )));
        // A hidden cell still runs, so it still needs the lane that can run it.
        assert!(!is_cell_free(&render(
            "---\ntitle: T\n---\n\n```{python}\n#| include: false\nprint(1)\n```\n"
        )));
        // A cell a `:::` container folded away runs too (item 210), and asking `b.cell`
        // alone here does not see it. Found by hand, not by this suite: the first build
        // went down the exec lane and worked, the page was then classified cell-free, and
        // every rebuild after it silently produced empty outputs — the same silent-drop the
        // whole item is about, reintroduced one predicate downstream of the fix. Every
        // container kind, because the bypass decision is per page, not per container.
        for wrapper in [
            ".callout-note",
            ".panel-tabset",
            ".column-page",
            "layout-ncol=2",
        ] {
            let src = format!(
                "---\ntitle: T\n---\n\n::: {{{wrapper}}}\n\n\
                 ```{{python}}\nprint(1)\n```\n\n:::\n"
            );
            assert!(
                !is_cell_free(&render(&src)),
                "a `{wrapper}` holding a {{python}} cell was routed to the lane that \
                 cannot run one"
            );
        }
        // …and a `{js}` cell in a container is still cell-free, for the same reason a
        // top-level one is: it runs in the browser.
        assert!(is_cell_free(&render(
            "---\ntitle: T\n---\n\n::: {.callout-note}\n\n```{js}\nreturn 1;\n```\n\n:::\n"
        )));
    }

    /// PT-2 on the preview's own path. `build notes/a.tmd` confines a document outside any
    /// project to its own folder (`render_single_doc`), refusing an include or a
    /// `bibliography:` that climbs to a sibling of the checkout. The preview rendered the
    /// same document through the project path with no root, which infers one from the
    /// nearest `.git`, so it showed the sibling's text the build drops and let an untrusted
    /// document read repo-local files through the preview. `include_root_parity.rs` pins
    /// the rule on `render_single_doc`; this pins that the preview calls it.
    #[test]
    fn a_loose_document_preview_resolves_includes_as_its_build_does() {
        let dir = std::env::temp_dir().join(format!("tali-loose-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        std::fs::write(dir.join(".git"), "").unwrap();
        std::fs::write(dir.join("sibling.tmd"), "SIBLING_SENTINEL\n").unwrap();
        let src = "---\ntitle: A\n---\n\n{{< include ../sibling.tmd >}}\n";
        std::fs::write(dir.join("notes/a.tmd"), src).unwrap();
        let file = dir.join("notes/a.tmd").canonicalize().unwrap();

        let site = taliesin_core::site::Site::discover_document(&file);
        let page = site
            .pages
            .first()
            .expect("the document is its own page")
            .clone();
        let preview = render_markdown_only(&site, &page);
        let body: String = preview
            .blocks
            .iter()
            .map(|b| format!("{}\n", b.html))
            .collect();
        let built = taliesin_core::render_single_doc(src, file.parent().unwrap());

        assert!(
            !body.contains("SIBLING_SENTINEL"),
            "the preview must refuse the climb the build refuses: {body}"
        );
        assert_eq!(body, built.body_html(), "one document, one render");
        assert!(
            preview
                .diagnostics
                .iter()
                .any(|d| d.message.contains("include not resolved")),
            "and say so: {:?}",
            preview
                .diagnostics
                .iter()
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The site-preview shell for one page of a corpus project, assembled the way the live
    /// server assembles it: a real `PageState` (so `toc` is the page's own answer, not a
    /// hand-set flag) behind a real `Project`.
    fn corpus_preview_page(project: &str, rel: &str) -> String {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus")
            .join(project);
        let site = taliesin_core::site::Site::discover(&dir);
        let page = site.page(rel).expect("corpus page").clone();
        let doc = render_markdown_only(&site, &page);
        let mut pages = HashMap::new();
        pages.insert(
            page.rel.clone(),
            PageState {
                doc,
                tx: tokio::sync::broadcast::channel(4).0,
            },
        );
        let project = Arc::new(Project {
            dir,
            site: parking_lot::Mutex::new(site),
            pages: parking_lot::Mutex::new(pages),
            exec_lane: Mutex::new(ExecLane::default()),
            scope: None,
            records: Mutex::new(HashMap::new()),
        });
        site_page_html(&project, &page)
    }

    /// One page of `site` as `build <dir>` writes it: rendered from `base`, finished, and
    /// wrapped in its chrome linking the shared `_assets/` (`Site::page_html_external`).
    fn built_site_page(
        site: &taliesin_core::site::Site,
        page: &taliesin_core::site::Page,
        base: &Path,
    ) -> String {
        let src = std::fs::read_to_string(&page.input).unwrap();
        let mut doc = taliesin_core::render_document_scoped_with_site(
            &src,
            base,
            None,
            Some(&site.render_defaults()),
        );
        let mut warnings = Vec::new();
        doc.toc = site.finish_blocks(page, &mut doc.blocks, &mut warnings, None, doc.toc_explicit);
        let assets = taliesin_core::ExternalAssets {
            app_css: "_assets/app.css",
            katex_css: "_assets/katex.css",
            app_js: "_assets/app.js",
            mermaid_js: "_assets/mermaid.js",
            jslibs_js: "_assets/jslibs.js",
            font_preload: "",
        };
        site.page_html_external(page, &doc, assets)
    }

    #[test]
    fn a_book_chapter_preview_gets_no_toc_rail() {
        // A book chapter has no rail at all (item 76), however long it is — the preview's
        // own assembler, not just the static build's, must honor that.
        let chapter = corpus_preview_page("tarn", "install.tmd");
        // The exact emitted mount, not `id="TOC"`: `client.js` is inlined verbatim and its
        // own source comments name the element, so the short needle matches on a page that
        // mounts nothing.
        assert!(
            !chapter.contains("<nav id=\"TOC\" aria-label=\"Table of contents\"></nav>"),
            "book chapter still ships a rail nav in the preview: {chapter}"
        );
        assert!(
            !chapter.contains("window.TALIESIN_TOC = true;"),
            "…and the client is not told to hydrate one: {chapter}"
        );
    }

    /// The chrome skeleton of a page: the `<body>` class, then every wrapper element's
    /// class in document order — which is all the site chrome IS. Read through
    /// `render::tags`/`attrs` rather than a substring scan, because a page's own prose may
    /// SHOW markup (`class="tali-site-main"` inside a code sample is text, not a wrapper).
    ///
    /// The `#TOC` rail is the ONE element the two paths legitimately spell differently — the
    /// build inlines the finished `<nav id="TOC" class="tali-toc">`, the preview mounts an
    /// empty landmark its client hydrates — so it is skipped, and what is compared for a
    /// TOC page is the wrapper class that reserves its column.
    fn chrome_skeleton(html: &str) -> Vec<String> {
        taliesin_core::render::tags(html)
            .filter(|t| matches!(t.name, "body" | "div" | "nav" | "main"))
            .filter_map(|t| {
                let attr = |name: &str| {
                    taliesin_core::render::attrs(&t)
                        .find(|a| a.name == name)
                        .map(|a| a.value.to_string())
                };
                if attr("id").as_deref() == Some("TOC") {
                    return None;
                }
                let class = attr("class")?;
                class
                    .split_whitespace()
                    .any(|c| c.starts_with("tali-") || c == "has-toc")
                    .then_some(format!("{}.{class}", t.name))
            })
            .collect()
    }

    /// The preview paints a page inside the SAME chrome the build does.
    ///
    /// FA16's actual subject. The `lang` test below pins the one value the hand-aligned twin
    /// used to invent (now inert on both sides, and asserted so); this pins the shell itself
    /// — where the navbar, the reading column, the TOC rail, the prev/next and the footer go. Both paths call `SiteCtx::layout` now, and
    /// what this guards is that they keep doing so. The CONTENTS are free to differ, as they
    /// must: the build renders `<main id="tali-main">` with the finished TOC, the preview
    /// mounts an empty `#tali-root` its websocket client drives.
    #[test]
    fn a_page_previews_inside_the_chrome_it_builds_inside() {
        // A book chapter (topbar + drawer + centred column, no rail), a website page
        // (navbar on top), and a page WITH a TOC rail, whose `has-toc` column class is the
        // conditional the two paths computed separately.
        for (project, rel) in [
            ("tarn", "install.tmd"),
            ("tech-blog", "index.tmd"),
            ("analyst", "methods.tmd"),
        ] {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus")
                .join(project);
            let site = taliesin_core::site::Site::discover(&dir);
            let page = site.page(rel).expect("corpus page").clone();
            let built = built_site_page(&site, &page, &dir);
            let preview = corpus_preview_page(project, rel);

            let (want, got) = (chrome_skeleton(&built), chrome_skeleton(&preview));
            // Anti-vacuity: a skeleton that reads as empty would make this pass forever.
            assert!(
                want.len() >= 2,
                "{project}/{rel}: the build's chrome parsed as {want:?}; the scan drifted"
            );
            assert_eq!(
                got, want,
                "{project}/{rel} previews inside different chrome than it builds inside"
            );
        }
    }

    /// A `lang:` in the front matter changes nothing, in either assembly.
    ///
    /// **The parser-side pin for the 2026-08-20 cut.** `lang:` was withdrawn, and
    /// withdrawing a construct means deleting the READ, not just the vocabulary entry --
    /// dropping a key from `KNOWN_KEYS` alone only makes it *diagnosed*, while a parser
    /// that still honours it goes on working. So this asserts the read is gone: a page
    /// declaring `lang: fi` paints `<html lang="en">` on BOTH paths.
    ///
    /// It is the same test that used to pin the opposite claim (Fable audit FA16: the live
    /// shell hardcoded `lang: "en"` while the build read the front matter, so a Finnish page
    /// previewed as English and built as Finnish). What made that possible was two
    /// assemblies each supplying their own value. Neither supplies one now -- both inherit
    /// the single `en` in `PageParts::defaults()` -- so the drift axis is closed
    /// structurally, and this test is what says so.
    #[test]
    fn a_declared_lang_is_inert_on_both_the_preview_and_the_build() {
        let dir = std::env::temp_dir().join(format!("tali-preview-lang-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: L\n").unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Etusivu\nlang: fi\n---\n\nTeksti.\n",
        )
        .unwrap();

        let site = taliesin_core::site::Site::discover(&dir);
        let page = site.page("index.tmd").expect("the page").clone();
        let doc = render_markdown_only(&site, &page);
        let mut pages = HashMap::new();
        pages.insert(
            page.rel.clone(),
            PageState {
                doc,
                tx: tokio::sync::broadcast::channel(4).0,
            },
        );
        let project = Arc::new(Project {
            dir: dir.clone(),
            site: parking_lot::Mutex::new(site),
            pages: parking_lot::Mutex::new(pages),
            exec_lane: Mutex::new(ExecLane::default()),
            scope: None,
            records: Mutex::new(HashMap::new()),
        });
        let preview = site_page_html(&project, &page);
        assert!(
            preview.contains(r#"<html lang="en""#) && !preview.contains(r#"lang="fi""#),
            "`lang:` names no read now, so the preview must paint the baseline `en`: {}",
            &preview[..preview.len().min(400)]
        );

        let built = built_site_page(&project.site.lock(), &page, &dir);
        assert!(
            built.contains(r#"<html lang="en""#) && !built.contains(r#"lang="fi""#),
            "the build must paint the same baseline, from the same const: {}",
            &built[..built.len().min(400)]
        );

        // ...and the withdrawn key draws the generic unknown-key diagnostic, so an author
        // who writes it is told rather than silently ignored.
        let src = std::fs::read_to_string(&page.input).unwrap();
        let ws = taliesin_core::frontmatter::validate_front_matter(&src);
        assert!(
            ws.iter()
                .any(|w| w.message.contains("unknown front-matter key `lang`")),
            "a withdrawn key must be diagnosed, not silently accepted: {ws:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unbuilt_page_routes_to_the_safe_lane() {
        // The routing flag is read from the LAST COMPLETED build, so its default decides
        // where a page goes before anything is known about it. `false` (= "not known to be
        // cell-free") must send it to the exec lane: the bypass lane cannot run a cell, and
        // guessing wrong there costs a wasted render, while guessing wrong the other way
        // would publish a page with its outputs missing.
        assert!(!PageDoc::default().cell_free);
    }

    /// Give `rel` the live state a visit gives it (`client_conn` allocates exactly this), so
    /// a build of it has somewhere to publish.
    fn open_page(project: &Project, rel: &str) {
        project.pages.lock().insert(
            rel.to_string(),
            PageState {
                doc: PageDoc::default(),
                tx: broadcast::channel(256).0,
            },
        );
    }

    /// A tab open on `rel` holding the page as it really renders: its cross-references in
    /// its blocks and the files it read in [`PageDoc::reads`], with a receiver on its
    /// channel.
    fn open_rendered(project: &Arc<Project>, rel: &str) -> broadcast::Receiver<String> {
        let page = project.site.lock().page(rel).cloned().unwrap();
        let doc = render_markdown_only(&project.site.lock(), &page);
        let (tx, rx) = broadcast::channel(256);
        project
            .pages
            .lock()
            .insert(rel.to_string(), PageState { doc, tx });
        rx
    }

    /// A tab open on `rel`: a state (as [`page_state_with_blocks`] makes it) and a receiver
    /// on its channel, which is what keeps the page rebuilt on a save. Hold the receiver for
    /// as long as the tab should count as open.
    fn watch(project: &Project, rel: &str) -> broadcast::Receiver<String> {
        let ps = page_state_with_blocks("<p>x</p>");
        let rx = ps.tx.subscribe();
        project.pages.lock().insert(rel.to_string(), ps);
        rx
    }

    fn page_state_with_blocks(html: &str) -> PageState {
        PageState {
            doc: PageDoc {
                blocks: vec![Block {
                    id: "x".into(),
                    sourcepos: String::new(),
                    source_file: None,
                    html: html.into(),
                    cell: None,
                    nested: Vec::new(),
                }],
                ..Default::default()
            },
            tx: tokio::sync::broadcast::channel(4).0,
        }
    }

    /// The regression this pins: deleting the "Referenced by" reverse index (2026-08-04)
    /// must not widen a moved-anchor rebuild to every open tab. `pages_citing_a_moved_anchor`
    /// is what `rebuild_project` now consults instead — this drives it directly with two
    /// open pages, one that cites the moved anchor and one that cites nothing cross-page,
    /// and asserts the referrer is selected and the bystander is not.
    ///
    /// **What this does and does not cover:** this pins the selection function in
    /// isolation (real `PageState`/`HashMap` shapes, hand-built blocks) — it does NOT
    /// drive `rebuild_project` itself or a live websocket session end to end (no test
    /// harness for that exists in this bin crate; see `project_tests`' own note on
    /// `only_a_resolvable_page_key_gets_a_page_state` for the same gap). The two are
    /// wired together by four lines at the call site (lock `pages`, call this, push what
    /// it returns) with no further logic of its own to hide a defect.
    #[test]
    fn pages_citing_a_moved_anchor_selects_the_referrer_not_the_bystander() {
        let mut pages = HashMap::new();
        pages.insert(
            "results.tmd".to_string(),
            page_state_with_blocks(
                r##"<p>It also leans on <a href="methods.html#thm-kl" class="tali-xref">Theorem&nbsp;2.1</a>.</p>"##,
            ),
        );
        pages.insert(
            "summary.tmd".to_string(),
            page_state_with_blocks("<p>No cross-page reference here at all.</p>"),
        );
        let open = vec!["results.tmd".to_string(), "summary.tmd".to_string()];
        let moved_anchors = HashSet::from(["thm-kl".to_string()]);

        let selected = pages_citing_a_moved_anchor(&pages, &open, &moved_anchors);

        assert_eq!(
            selected,
            vec!["results.tmd".to_string()],
            "only the page citing the moved anchor should be rebuilt"
        );
    }

    /// A page that cites SOME cross-page anchor, just not the one that moved, must also
    /// stay off the rebuild list — the old reverse index rebuilt on ANY cross-page
    /// reference when ANY target moved; the replacement is scoped to the anchor that
    /// actually moved.
    #[test]
    fn pages_citing_a_moved_anchor_ignores_a_page_that_cites_a_different_anchor() {
        let mut pages = HashMap::new();
        pages.insert(
            "results.tmd".to_string(),
            page_state_with_blocks(
                r##"<p>See <a href="methods.html#sec-setup" class="tali-xref">Section&nbsp;2.1</a>.</p>"##,
            ),
        );
        let open = vec!["results.tmd".to_string()];
        let moved_anchors = HashSet::from(["thm-kl".to_string()]);

        assert!(pages_citing_a_moved_anchor(&pages, &open, &moved_anchors).is_empty());
    }

    /// A `rel` with no live state (closed, or a not-yet-first-rendered race) is skipped
    /// rather than force-included — it has no cached blocks to consult.
    #[test]
    fn pages_citing_a_moved_anchor_skips_a_rel_with_no_page_state() {
        let pages: HashMap<String, PageState> = HashMap::new();
        let open = vec!["ghost.tmd".to_string()];
        let moved_anchors = HashSet::from(["thm-kl".to_string()]);

        assert!(pages_citing_a_moved_anchor(&pages, &open, &moved_anchors).is_empty());
    }

    /// A scratch project directory, canonicalized so the paths a test hands
    /// [`rebuild_project`] are in the same coordinate system as the ones discovery resolved
    /// (`changed_canon` and the dependency walk both canonicalize; on a platform where the
    /// temp dir is itself a symlink, an uncanonicalized root would never intersect).
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tali-rebuild-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(&d).unwrap()
    }

    /// A `Project` over `dir` as the live server builds one, plus the [`SiteApp`] around it
    /// with both build lanes' receivers handed back. No worker task is spawned: what a test
    /// of [`rebuild_project`] observes is which pages it QUEUED, not what a render produced.
    fn project_and_app(
        dir: &Path,
    ) -> (
        Arc<Project>,
        SiteApp,
        mpsc::UnboundedReceiver<BuildMsg>,
        mpsc::UnboundedReceiver<BuildMsg>,
    ) {
        let project = Arc::new(Project {
            dir: dir.to_path_buf(),
            site: Mutex::new(taliesin_core::site::Site::discover(dir)),
            pages: Mutex::new(HashMap::new()),
            exec_lane: Mutex::new(ExecLane::default()),
            scope: None,
            records: Mutex::new(HashMap::new()),
        });
        project.seed_records();
        let (build_tx, build_rx) = mpsc::unbounded_channel();
        let (fast_tx, fast_rx) = mpsc::unbounded_channel();
        let app = SiteApp {
            root: project.clone(),
            build_tx,
            fast_tx,
            interrupt: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        };
        (project, app, build_rx, fast_rx)
    }

    /// Every page `rebuild_project` queued, on either lane, sorted. `queue_build` routes on
    /// the page's last-known `cell_free` flag, so a test must drain both or it reads a
    /// rebuild that did happen as one that did not.
    fn queued(
        build_rx: &mut mpsc::UnboundedReceiver<BuildMsg>,
        fast_rx: &mut mpsc::UnboundedReceiver<BuildMsg>,
    ) -> Vec<String> {
        let mut out = Vec::new();
        for rx in [build_rx, fast_rx] {
            while let Ok(
                BuildMsg::Build(rel) | BuildMsg::Restart(rel) | BuildMsg::IfMoved(rel, _),
            ) = rx.try_recv()
            {
                out.push(rel);
            }
        }
        out.sort();
        out
    }

    /// Finding 16. `_site.yml`'s project-wide `bibliography:` is a render input of every
    /// page (`Site::render_defaults` lays it under each page's own), and it is named in no
    /// page's own source, so a dependency walk that read the PAGE could never see it: the
    /// open tab kept serving the citation it had, and a browser reload served the same one.
    /// The page's render reads the file, and what a render reads is what it depends on.
    #[test]
    fn a_shared_bibliography_save_rebuilds_the_pages_that_inherit_it() {
        let dir = scratch("shared-bib");
        std::fs::write(dir.join("_site.yml"), "title: T\nbibliography: refs.bib\n").unwrap();
        std::fs::write(
            dir.join("refs.bib"),
            "@article{k,\n title = {One},\n year = {2020}\n}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\n---\n\nAs shown in [@k].\n",
        )
        .unwrap();

        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        assert_eq!(
            project.site.lock().bibliography.len(),
            1,
            "the project must actually resolve its shared `.bib`, or this proves nothing"
        );
        let _tab = open_rendered(&project, "index.tmd");

        // The author fixes a wrong year and saves. Nothing else on disk moves.
        std::fs::write(
            dir.join("refs.bib"),
            "@article{k,\n title = {One},\n year = {2021}\n}\n",
        )
        .unwrap();
        let changed: HashSet<PathBuf> = std::iter::once(dir.join("refs.bib")).collect();
        rebuild_project(&app, &project, &changed);

        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string()],
            "the open page inherits the shared bibliography, so its save must rebuild it"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 invalidation #9. A page's state outlives its tab: every page a
    /// reader ever opened kept one, and a front-matter save rebuilt all of them, so a save
    /// took 453 ms with two pages visited and 2544 ms after one visit of each of 221 pages.
    /// A page nobody is watching is not rebuilt: its state is dropped, and its next visit
    /// renders it fresh, which is what a visit to a never-opened page does anyway.
    #[test]
    fn a_save_rebuilds_only_the_pages_a_tab_is_watching() {
        let dir = scratch("watched");
        std::fs::create_dir_all(dir.join("posts")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\nlisting:\n  contents: posts\n---\n\nPosts.\n",
        )
        .unwrap();
        let post = dir.join("posts/a.tmd");
        std::fs::write(&post, "---\ntitle: Old\n---\n\nBody.\n").unwrap();
        std::fs::write(dir.join("posts/b.tmd"), "---\ntitle: B\n---\n\nBody.\n").unwrap();

        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let _tab = watch(&project, "index.tmd");
        // Visited and left: a state with nobody on its channel.
        for rel in ["posts/a.tmd", "posts/b.tmd"] {
            project
                .pages
                .lock()
                .insert(rel.to_string(), page_state_with_blocks("<p>x</p>"));
        }

        std::fs::write(&post, "---\ntitle: New\n---\n\nBody.\n").unwrap();
        rebuild_project(&app, &project, &std::iter::once(post).collect());

        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string()],
            "the listing a tab shows is rebuilt, and no page nobody is watching"
        );
        let mut kept: Vec<String> = project.pages.lock().keys().cloned().collect();
        kept.sort();
        assert_eq!(
            kept,
            vec!["index.tmd".to_string()],
            "an unwatched page's state is dropped, so its next visit renders it fresh"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A page that does NOT read a `.bib` must stay off the rebuild list when it changes: a
    /// dependency, not a licence to rebuild every open tab on any save. Without
    /// this, the fix above would read as correct while quietly rebuilding the whole warm
    /// set on every image or stylesheet write.
    #[test]
    fn a_project_with_no_shared_bibliography_still_rebuilds_nothing_on_an_unrelated_save() {
        let dir = scratch("no-shared-bib");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(dir.join("refs.bib"), "@article{k,\n year = {2020}\n}\n").unwrap();
        std::fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nProse.\n").unwrap();

        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        assert!(project.site.lock().bibliography.is_empty());
        let _tab = open_rendered(&project, "index.tmd");

        let changed: HashSet<PathBuf> = std::iter::once(dir.join("refs.bib")).collect();
        rebuild_project(&app, &project, &changed);

        assert!(
            queued(&mut build_rx, &mut fast_rx).is_empty(),
            "a `.bib` this project declares nowhere is not a dependency of any page"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 C1. An editor that saves by writing a temp file and renaming it
    /// over the page (vim, JetBrains, gedit, `sed -i`), and a `git checkout`, deliver a
    /// front-matter edit as a rename or an unlink plus a create. The preview read those as a
    /// possible page-set change, re-discovered and reseeded its record before it asked
    /// whether the front matter moved, and so never rebuilt the listing that shows the
    /// post: the open tab kept the old card, and a fresh GET served the same stale body.
    #[test]
    fn an_atomic_save_of_a_listed_post_title_rebuilds_the_listing() {
        let dir = scratch("atomic-title");
        std::fs::create_dir_all(dir.join("posts")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\nlisting:\n  contents: posts\n---\n\nPosts.\n",
        )
        .unwrap();
        let post = dir.join("posts/a.tmd");
        std::fs::write(&post, "---\ntitle: Old\n---\n\nBody.\n").unwrap();

        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let _tab = watch(&project, "index.tmd");

        // gedit's save: a temp file beside the page, renamed over it.
        let tmp = dir.join("posts/.goutputstream-AB12CD");
        std::fs::write(&tmp, "---\ntitle: New\n---\n\nBody.\n").unwrap();
        std::fs::rename(&tmp, &post).unwrap();
        let changed: HashSet<PathBuf> = [tmp, post].into_iter().collect();
        rebuild_project(&app, &project, &changed);

        assert_eq!(
            project
                .site
                .lock()
                .page("posts/a.tmd")
                .unwrap()
                .title
                .as_deref(),
            Some("New")
        );
        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string()],
            "the listing shows the post's title, so it must be rebuilt"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 C2. Discovery reads a page's leading `# H1` as well as its front
    /// matter: the H1 names a book chapter in the drawer and the pager (before `title:`),
    /// its `.unnumbered` decides every later chapter's number, and it titles a website page
    /// that has no `title:`. The record hashed only the `---` block, so an in-place edit of
    /// the H1 never re-discovered, and every chapter's section, figure and equation numbers
    /// stayed wrong, in open tabs and on fresh GETs.
    #[test]
    fn an_in_place_edit_of_a_chapter_heading_rediscovers_the_book() {
        let dir = scratch("h1-edit");
        std::fs::write(
            dir.join("_site.yml"),
            "title: T\nchapters:\n  - intro.tmd\n  - methods.tmd\n",
        )
        .unwrap();
        let intro = dir.join("intro.tmd");
        std::fs::write(&intro, "# Introduction\n\nText.\n").unwrap();
        std::fs::write(dir.join("methods.tmd"), "# Methods\n\nText.\n").unwrap();

        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let mut tab = watch(&project, "methods.tmd");
        let chapter = |project: &Project| {
            let site = project.site.lock();
            site.chapter_for(site.page("methods.tmd").unwrap())
        };
        assert_eq!(chapter(&project), Some(2));

        std::fs::write(&intro, "# Introduction {.unnumbered}\n\nText.\n").unwrap();
        rebuild_project(&app, &project, &std::iter::once(intro).collect());

        assert_eq!(
            chapter(&project),
            Some(1),
            "the chapter after an unnumbered one is chapter 1"
        );
        assert_eq!(
            tab.try_recv().as_deref().unwrap_or(""),
            protocol::reload(),
            "its numbers and its drawer moved, so the open chapter reloads"
        );
        assert!(queued(&mut build_rx, &mut fast_rx).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 C5. A book's drawer and pager are chrome: they sit outside
    /// `#tali-root`, where no block op reaches. Retitling a chapter re-discovered the book
    /// and rebuilt every open chapter, whose bodies had not changed, so every tab kept the
    /// old label with nothing sent at all, while a comment claimed the tabs took the change
    /// as ops. After a re-discovery a tab whose chrome moved reloads, and one whose chrome
    /// held takes the change as ops, keeping its live state.
    #[test]
    fn a_rediscovery_reloads_exactly_the_tabs_whose_chrome_moved() {
        let dir = scratch("chrome-book");
        std::fs::write(
            dir.join("_site.yml"),
            "title: T\nchapters:\n  - intro.tmd\n  - methods.tmd\n",
        )
        .unwrap();
        let intro = dir.join("intro.tmd");
        std::fs::write(&intro, "# Introduction\n\nText.\n").unwrap();
        std::fs::write(dir.join("methods.tmd"), "# Methods\n\nText.\n").unwrap();
        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let mut tab = watch(&project, "methods.tmd");

        std::fs::write(&intro, "# Opening\n\nText.\n").unwrap();
        rebuild_project(&app, &project, &std::iter::once(intro).collect());

        assert_eq!(
            tab.try_recv().as_deref().unwrap_or(""),
            protocol::reload(),
            "the drawer and the pager on this chapter name the retitled one"
        );
        assert!(
            project.pages.lock().is_empty(),
            "so it re-renders on the reload"
        );
        assert!(queued(&mut build_rx, &mut fast_rx).is_empty());
        let _ = std::fs::remove_dir_all(&dir);

        // A website's listing shows the post's title in its body, which ops reach, and its
        // chrome does not name the post: it keeps its DOM. So does the post itself, whose
        // `<head>` meta follows its title but which no reader sees.
        let dir = scratch("chrome-site");
        std::fs::create_dir_all(dir.join("posts")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\nlisting:\n  contents: posts\n---\n\nPosts.\n",
        )
        .unwrap();
        let post = dir.join("posts/a.tmd");
        std::fs::write(&post, "---\ntitle: Old\n---\n\nBody.\n").unwrap();
        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let mut tab = watch(&project, "index.tmd");
        let mut own = watch(&project, "posts/a.tmd");

        std::fs::write(&post, "---\ntitle: New\n---\n\nBody.\n").unwrap();
        rebuild_project(&app, &project, &std::iter::once(post).collect());

        assert!(
            tab.try_recv().is_err(),
            "no reload for a tab whose chrome held"
        );
        assert!(own.try_recv().is_err(), "nor for the page being retitled");
        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string(), "posts/a.tmd".to_string()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 C6. An `{{< include >}}` partial can hold an anchor as well as a
    /// page can, and a partial is often a `.md`. The cross-reference registry was refreshed
    /// only for a `.tmd` save, so a figure removed from a `.md` partial left every page
    /// citing the figure after it one number off, in the tab and on a fresh GET, until some
    /// `.tmd` anywhere was saved.
    #[test]
    fn an_anchor_renumbered_in_a_md_partial_reaches_the_page_citing_it() {
        let dir = scratch("md-renumber");
        std::fs::create_dir_all(dir.join("_partials")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(
            dir.join("g.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        )
        .unwrap();
        let partial = dir.join("_partials/figpart.md");
        std::fs::write(&partial, "![Gamma](g.svg){#fig-gamma}\n").unwrap();
        std::fs::write(
            dir.join("figs.tmd"),
            "---\ntitle: Figures\n---\n\n{{< include _partials/figpart.md >}}\n\n\
             ![Alpha](g.svg){#fig-alpha}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\n---\n\nSee @fig-alpha.\n",
        )
        .unwrap();
        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let number =
            |project: &Project| project.site.lock().xref_targets["fig-alpha"].number.clone();
        assert_eq!(number(&project), "2");
        let _tab = open_rendered(&project, "index.tmd");

        std::fs::write(&partial, "No figure here any more.\n").unwrap();
        rebuild_project(&app, &project, &std::iter::once(partial).collect());

        assert_eq!(number(&project), "1");
        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string()],
            "the page citing the renumbered figure is rebuilt"
        );
        let index = project.site.lock().search_index_json.clone();
        assert!(
            index.contains("Figure 1") && !index.contains("Figure 2"),
            "and the search index says what the page now says: {index}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 C4. The natural order is to declare `bibliography: refs.bib` in
    /// `_site.yml`, then create `refs.bib`. Discovery dropped a declared file that did not
    /// exist yet, so no page depended on it: creating it rebuilt nothing, citations stayed
    /// raw keys, and the dev menu went on saying the file was not found while it existed,
    /// even after the page itself was rebuilt. A file a render looked for and did not find
    /// is a dependency like any other.
    #[test]
    fn a_shared_bibliography_created_after_it_was_declared_is_picked_up() {
        let dir = scratch("late-bib");
        std::fs::write(dir.join("_site.yml"), "title: T\nbibliography: refs.bib\n").unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\n---\n\nAs shown in [@k].\n",
        )
        .unwrap();
        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let _tab = open_rendered(&project, "index.tmd");

        std::fs::write(
            dir.join("refs.bib"),
            "@article{k,\n title = {Late Title},\n year = {2021}\n}\n",
        )
        .unwrap();
        rebuild_project(
            &app,
            &project,
            &std::iter::once(dir.join("refs.bib")).collect(),
        );
        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string()],
            "the page looked for the file, so creating it rebuilds the page"
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(build_page(&project, "index.tmd", None));
        let pages = project.pages.lock();
        let doc = &pages["index.tmd"].doc;
        assert!(
            doc.body_html().contains("Late Title"),
            "the citation resolves"
        );
        assert!(
            !doc.diagnostics
                .iter()
                .any(|d| d.message.contains("not found")),
            "and nothing says the file is missing: {:?}",
            doc.diagnostics
        );
        drop(pages);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 C7 (images #5). The render reads each local raster image it shows,
    /// for the `width`/`height` that reserve its box, and checks that every local asset
    /// exists, but no image was a dependency of its page: adding a missing image left its
    /// "not found" error on screen after a reload, and re-exporting a figure at a new size
    /// kept the old dimensions baked into the page, tab and fresh GET alike.
    #[test]
    fn adding_or_replacing_an_image_rebuilds_the_page_that_shows_it() {
        let dir = scratch("image");
        std::fs::create_dir_all(dir.join("img")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\n---\n\n![A picture](img/pic.png)\n",
        )
        .unwrap();
        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let _tab = open_rendered(&project, "index.tmd");
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
        let pic = dir.join("img/pic.png");

        // Added: the page reported it missing.
        std::fs::copy(corpus.join("diagnostics/logo.png"), &pic).unwrap();
        rebuild_project(&app, &project, &std::iter::once(pic.clone()).collect());
        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string()]
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(build_page(&project, "index.tmd", None));
        assert!(
            project.pages.lock()["index.tmd"]
                .doc
                .body_html()
                .contains("width=\"1\""),
            "the added image is measured"
        );

        // Replaced by a figure of another size.
        std::fs::copy(corpus.join("media/fit-small.png"), &pic).unwrap();
        rebuild_project(&app, &project, &std::iter::once(pic.clone()).collect());
        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string()]
        );
        assert!(
            probes_moved(&project, "index.tmd", &[record_key(&pic)]),
            "not as the last build left it, so the lane rebuilds"
        );
        rt.block_on(build_page(&project, "index.tmd", None));
        assert!(
            project.pages.lock()["index.tmd"]
                .doc
                .body_html()
                .contains("width=\"320\""),
            "the new size replaces the old one"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A figure a page's own cells write (`savefig("gen.png")`, shown as `![…](gen.png)`) is
    /// the page's output. With images a dependency (C7), rebuilding the page for it ran its
    /// cells again, and a `#| cache: false` cell re-runs on every build, so it wrote the file
    /// again: 77 runs in 8 s of an idle preview. A file the page only looked at is asked
    /// about after the page's build instead: the page's own write is already in what that
    /// build left, a later write by the author is not, and a file the page reads as text
    /// (its source, which an author edits while cells run) is rebuilt outright.
    #[test]
    fn a_file_the_pages_own_cells_wrote_does_not_rebuild_it() {
        let dir = scratch("own-output");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        let page = dir.join("index.tmd");
        std::fs::write(&page, "---\ntitle: Home\n---\n\n![Generated](gen.png)\n").unwrap();
        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let _tab = open_rendered(&project, "index.tmd");
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
        let figure = dir.join("gen.png");
        let saved =
            |path: &Path| -> HashSet<PathBuf> { std::iter::once(path.to_path_buf()).collect() };
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Written during the page's build, as its cells would: the build ends with it there.
        std::fs::copy(corpus.join("diagnostics/logo.png"), &figure).unwrap();
        rt.block_on(build_page(&project, "index.tmd", None));
        rebuild_project(&app, &project, &saved(&figure));
        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string()],
            "the page is asked about"
        );
        let figure_key = record_key(&figure);
        assert!(
            !probes_moved(&project, "index.tmd", std::slice::from_ref(&figure_key)),
            "the page's own output is no change to it"
        );

        // Replaced by the author afterwards: the page shows it, so it is rebuilt.
        std::thread::sleep(Duration::from_millis(20));
        std::fs::copy(corpus.join("media/fit-small.png"), &figure).unwrap();
        assert!(probes_moved(
            &project,
            "index.tmd",
            std::slice::from_ref(&figure_key)
        ));

        // The source is read as text, so saving it rebuilds the page with no question.
        std::fs::write(
            &page,
            "---\ntitle: Home\n---\n\n![Generated](gen.png)\n\nMore.\n",
        )
        .unwrap();
        rebuild_project(&app, &project, &saved(&page));
        let mut outright = Vec::new();
        for rx in [&mut build_rx, &mut fast_rx] {
            while let Ok(msg) = rx.try_recv() {
                outright.push(matches!(msg, BuildMsg::Build(rel) if rel == "index.tmd"));
            }
        }
        assert_eq!(outright, vec![true]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The loop above, live: a `#| cache: false` cell that writes the figure its page
    /// shows runs once per save, not once per build it causes. Gated on a live kernel.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_cell_writing_the_figure_its_page_shows_does_not_rebuild_forever() {
        if std::env::var_os("TALIESIN_PYTHON").is_none() {
            eprintln!(
                "SKIPPED (no live kernel): set TALIESIN_PYTHON to a python with ipykernel to \
                 exercise a cell that writes its page's figure; this run did not."
            );
            return;
        }
        let dir = scratch("own-output-live");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        let logo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/diagnostics/logo.png")
            .canonicalize()
            .unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            format!(
                "---\ntitle: Loop\n---\n\n```{{python}}\n#| cache: false\nimport os, shutil\n\
                 os.makedirs('_freeze', exist_ok=True)\n\
                 open('_freeze/runs.txt', 'a').write('x')\n\
                 shutil.copy({logo:?}, 'gen.png')\n```\n\n![Generated](gen.png)\n"
            ),
        )
        .unwrap();
        let runs = || {
            std::fs::read_to_string(dir.join("_freeze/runs.txt"))
                .map(|s| s.len())
                .unwrap_or(0)
        };
        let live = Live::start(&dir);
        let _tab = live.open("index.tmd");
        until("the cell's first run", || runs() >= 1);
        std::thread::sleep(Duration::from_secs(3));
        assert!(
            runs() <= 2,
            "the cell ran {} times in 3 s with nothing saved",
            runs()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 C9 (perf #3). Every rename-over save counted as a possible page-set
    /// change, so an atomic save of a body edit paid a whole re-discovery (every page read
    /// and rendered twice): 369 ms against 195 ms in place at 221 pages. A page that is
    /// still there, with what discovery reads of it unchanged, is an edit in place, however
    /// the editor wrote it.
    #[test]
    fn an_atomic_save_of_a_body_edit_does_not_rediscover() {
        let dir = scratch("atomic-body");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        let page = dir.join("index.tmd");
        std::fs::write(&page, "---\ntitle: Home\n---\n\n# Home\n\nOld body.\n").unwrap();

        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let _tab = open_rendered(&project, "index.tmd");
        // A mark only this `Site` carries: a re-discovery replaces it with one without.
        project
            .site
            .lock()
            .warnings
            .push(taliesin_core::render::Warning::new("KEPT"));

        // JetBrains' safe write: temp file, original moved aside, temp renamed over it.
        let tmp = dir.join("index.tmd___jb_tmp___");
        let old = dir.join("index.tmd___jb_old___");
        std::fs::write(&tmp, "---\ntitle: Home\n---\n\n# Home\n\nNew body.\n").unwrap();
        std::fs::rename(&page, &old).unwrap();
        std::fs::rename(&tmp, &page).unwrap();
        std::fs::remove_file(&old).unwrap();
        let changed: HashSet<PathBuf> = [tmp, old, page].into_iter().collect();
        rebuild_project(&app, &project, &changed);

        assert!(
            project
                .site
                .lock()
                .warnings
                .iter()
                .any(|w| w.message == "KEPT"),
            "a body edit re-discovered the project"
        );
        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string()],
            "and the page itself is still rebuilt"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Finding 17. A same-filesystem rename never creates and never removes: on
    /// Linux/inotify `mv a.tmd b.tmd` emits `Modify(Name(From))` on the old path,
    /// `Modify(Name(To))` on the new, then a `Modify(Name(Both))` carrying both. Read as an
    /// edit in place, it left the site listing the old page and 404ing the new URL until a
    /// restart. The old path is a page that is gone and the new one a source file discovery
    /// never classified, so `rebuild_project` re-discovers and serves the new page set: the
    /// tab left on the vanished page reloads (onto the 404 the build would give it), and the
    /// listing that links the page takes the new link as ops.
    #[test]
    fn renaming_a_page_moves_the_page_set_and_reloads_the_tab_left_on_it() {
        let dir = scratch("rename");
        std::fs::create_dir_all(dir.join("posts")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\nlisting:\n  contents: posts\n---\n\nPosts.\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("posts/notes.tmd"),
            "---\ntitle: Notes\n---\n\nProse.\n",
        )
        .unwrap();

        let (project, app, mut build_rx, mut fast_rx) = project_and_app(&dir);
        let mut left = watch(&project, "posts/notes.tmd");
        let mut listing = watch(&project, "index.tmd");

        std::fs::rename(dir.join("posts/notes.tmd"), dir.join("posts/journal.tmd")).unwrap();
        // Exactly what the watcher hands `dispatch_changes` for that rename: both paths.
        let changed: HashSet<PathBuf> =
            [dir.join("posts/notes.tmd"), dir.join("posts/journal.tmd")]
                .into_iter()
                .collect();
        rebuild_project(&app, &project, &changed);

        let site = project.site.lock();
        assert!(
            site.page("posts/journal.tmd").is_some(),
            "the new page is served"
        );
        assert!(
            site.page("posts/notes.tmd").is_none(),
            "the old page is not"
        );
        drop(site);
        assert_eq!(
            left.try_recv().as_deref().unwrap_or(""),
            protocol::reload(),
            "the tab on the vanished page must not go on showing it"
        );
        assert!(listing.try_recv().is_err(), "the listing's chrome held");
        assert_eq!(
            queued(&mut build_rx, &mut fast_rx),
            vec!["index.tmd".to_string()],
            "the listing is rebuilt with the new link, and the vanished page is not"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A live preview of a project with no HTTP in front of it: the real watcher and both
    /// real build lanes over a real [`Project`], wired as [`serve`] wires them. A test changes
    /// files on disk the way an editor does and reads what the preview then holds.
    struct Live {
        app: Arc<SiteApp>,
        rt: tokio::runtime::Runtime,
    }

    impl Live {
        fn start(dir: &Path) -> Live {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let _enter = rt.enter();
            let (build_tx, build_rx) = mpsc::unbounded_channel();
            let (fast_tx, fast_rx) = mpsc::unbounded_channel();
            let site = Site::discover_with(dir, taliesin_core::DraftMode::Include);
            let app = Arc::new(SiteApp {
                root: Arc::new(Project {
                    dir: dir.to_path_buf(),
                    site: Mutex::new(site),
                    pages: Mutex::new(HashMap::new()),
                    exec_lane: Mutex::new(ExecLane::default()),
                    scope: None,
                    records: Mutex::new(HashMap::new()),
                }),
                build_tx,
                fast_tx,
                interrupt: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            });
            app.root.seed_records();
            spawn_builder(app.clone(), build_rx);
            spawn_fast_builder(app.clone(), fast_rx);
            spawn_watcher(app.clone());
            drop(_enter);
            Live { app, rt }
        }

        /// Open a tab on `rel` as a browser does: the first paint, then a subscription to
        /// the page's channel. Hold the receiver for as long as the tab is open.
        fn open(&self, rel: &str) -> broadcast::Receiver<String> {
            let _enter = self.rt.enter();
            let project = &self.app.root;
            let page = project.site.lock().page(rel).cloned().expect("a page");
            ensure_and_render_page(&self.app, project, &page);
            project.pages.lock()[&page.rel].tx.subscribe()
        }

        /// The live body of `rel`, or empty when it has no live state.
        fn body(&self, rel: &str) -> String {
            let pages = self.app.root.pages.lock();
            pages
                .get(rel)
                .map(|ps| ps.doc.body_html())
                .unwrap_or_default()
        }

        fn has_page(&self, rel: &str) -> bool {
            self.app.root.site.lock().page(rel).is_some()
        }
    }

    /// Poll `done` until it holds, or panic naming `what` after ten seconds.
    fn until(what: &str, done: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !done() {
            assert!(std::time::Instant::now() < deadline, "timed out: {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Audit 2026-09-24 C3. A folder-per-post blog renames a post by renaming its folder
    /// (`mv`, the VS Code explorer and every file manager make the same call). The watcher
    /// dropped the event, since a directory has no file extension, and notify drops a moved
    /// directory's watch: the new URL 404ed, and every later edit inside the folder went
    /// unseen until a restart. The same held for a folder moved in from outside.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_renamed_or_moved_in_folder_is_served_and_watched() {
        let dir = scratch("folder");
        std::fs::create_dir_all(dir.join("posts/a-star")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\nlisting:\n  contents: posts\n---\n\nPosts.\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("posts/a-star/index.tmd"),
            "---\ntitle: Search\n---\n\nBody.\n",
        )
        .unwrap();
        let live = Live::start(&dir);
        let _tab = live.open("index.tmd");
        until("the first build", || {
            live.body("index.tmd").contains("Search")
        });

        std::fs::rename(dir.join("posts/a-star"), dir.join("posts/a-star-v2")).unwrap();
        until("the renamed folder's page is served", || {
            live.has_page("posts/a-star-v2/index.tmd") && !live.has_page("posts/a-star/index.tmd")
        });
        std::fs::write(
            dir.join("posts/a-star-v2/index.tmd"),
            "---\ntitle: Retitled\n---\n\nBody.\n",
        )
        .unwrap();
        until(
            "an edit inside the renamed folder reaches the listing",
            || live.body("index.tmd").contains("Retitled"),
        );

        let outside = scratch("folder-outside");
        std::fs::create_dir_all(outside.join("ext")).unwrap();
        std::fs::write(
            outside.join("ext/page.tmd"),
            "---\ntitle: Moved in\n---\n\nx\n",
        )
        .unwrap();
        std::fs::rename(outside.join("ext"), dir.join("posts/ext")).unwrap();
        until("a folder moved in from outside is served", || {
            live.has_page("posts/ext/page.tmd")
        });
        std::fs::write(
            dir.join("posts/ext/page.tmd"),
            "---\ntitle: Moved and edited\n---\n\nx\n",
        )
        .unwrap();
        until("and watched", || {
            live.body("index.tmd").contains("Moved and edited")
        });

        // Moved out (to the trash, say): the only event is the folder's own rename away.
        std::fs::rename(dir.join("posts/ext"), outside.join("ext")).unwrap();
        until("a folder moved out takes its page along", || {
            !live.has_page("posts/ext/page.tmd")
        });
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    /// Audit 2026-09-24 invalidation #12. The watcher dropped every event outside a list of
    /// extensions, which disagreed with what the pages read: a page linking a `.pdf` kept
    /// its "broken link" after the file was created. Whether a save matters is decided by
    /// what the pages read, so the list is gone.
    #[cfg(target_os = "linux")]
    #[test]
    fn creating_a_linked_file_of_any_kind_clears_its_broken_link() {
        let dir = scratch("linked-pdf");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\n---\n\nRead [the report](report.pdf).\n",
        )
        .unwrap();
        let live = Live::start(&dir);
        let _tab = live.open("index.tmd");
        let broken = || {
            let pages = live.app.root.pages.lock();
            pages.get("index.tmd").is_some_and(|ps| {
                ps.doc
                    .diagnostics
                    .iter()
                    .any(|d| d.message.contains("report.pdf"))
            })
        };
        until("the missing file is reported", broken);
        std::fs::write(dir.join("report.pdf"), b"%PDF-1.4\n").unwrap();
        until("creating it clears the report", || !broken());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 C8 and first-hour #9. The interpreter was resolved once, when the
    /// preview started, so a `python:` edited in `_site.yml` never reached a kernel, not
    /// even through Restart kernel, and a `.venv` created while the preview ran was ignored,
    /// though the guide says to fix the kernel and save. The exec lane re-resolves before
    /// every job and moves to a fresh pool when the answer changed.
    #[test]
    fn the_exec_lane_follows_the_interpreter_the_project_resolves_to_now() {
        let dir = scratch("python");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
        let (project, app, _b, _f) = project_and_app(&dir);
        let before = {
            let s = project.site.lock();
            crate::interpreter::resolve_python(s.config.python.as_deref(), &project.dir)
        };
        let mut pool = ExecPool::new(dir.join("_freeze"), before, app.interrupt.clone());

        // A `.venv` created while the preview runs.
        let venv = dir.join(".venv/bin/python");
        std::fs::create_dir_all(venv.parent().unwrap()).unwrap();
        std::fs::write(&venv, "").unwrap();
        repoint(&mut pool, &project, &app.interrupt);
        assert_eq!(pool.python(), Some(venv.as_path()));

        // `python:` set in `_site.yml`, adopted by the re-discovery its save causes.
        std::fs::write(
            dir.join("_site.yml"),
            "title: T\npython: /opt/py/bin/python\n",
        )
        .unwrap();
        *project.site.lock() = project.rediscover();
        repoint(&mut pool, &project, &app.interrupt);
        assert_eq!(pool.python(), Some(Path::new("/opt/py/bin/python")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 invalidation #13. A tab whose page vanishes reloads onto the 404
    /// page, which carries no live client: when the page came back (deleted and written
    /// again in two saves, as `git` and some editors do, or restored by hand) the tab stayed
    /// on the 404. The preview's 404 page asks again, once a second, whether the page it
    /// stands for is there, and reloads onto it when it is.
    #[test]
    fn the_previews_404_page_rechecks_the_page_it_stands_for() {
        let dir = scratch("404");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
        let (_project, app, _b, _f) = project_and_app(&dir);
        let app = Arc::new(app);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (status, body) = rt.block_on(async {
            let res = page_or_asset(
                State(app.clone()),
                axum::http::Method::GET,
                "/gone.html".parse().unwrap(),
            )
            .await;
            let status = res.status();
            let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap();
            (status, String::from_utf8_lossy(&bytes).into_owned())
        });
        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        let script = taliesin_core::render::tags(&body)
            .filter(|t| t.name.eq_ignore_ascii_case("script"))
            .map(|t| {
                let rest = &body[t.at + t.text.len()..];
                rest[..rest.find("</script>").unwrap_or(rest.len())].to_string()
            })
            .find(|js| js.contains("location.reload"));
        let script = script.expect("the 404 page carries a check that reloads it");
        assert!(
            script.contains("fetch(location.href"),
            "it asks about the page it stands for: {script}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 WP1 residual. The preview answered every HTTP method for a page or a
    /// static file, a `POST` or a `DELETE` included, as if it were a `GET`. It serves reads
    /// only: `GET` and `HEAD`, and `405` with an `Allow` header for anything else.
    #[test]
    fn the_preview_serves_pages_and_files_to_get_and_head_only() {
        use axum::http::{Method, StatusCode, header};
        let dir = scratch("methods");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
        std::fs::write(dir.join("style.css"), "body{}").unwrap();
        let (_project, app, _b, _f) = project_and_app(&dir);
        let app = Arc::new(app);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let answer = |method: Method, uri: &str| {
            let res = rt.block_on(page_or_asset(
                State(app.clone()),
                method,
                uri.parse().unwrap(),
            ));
            let allow = res.headers().get(header::ALLOW).cloned();
            (res.status(), allow)
        };
        for uri in ["/style.css", "/index.html"] {
            assert_eq!(answer(Method::GET, uri).0, StatusCode::OK, "{uri}");
            assert_eq!(answer(Method::HEAD, uri).0, StatusCode::OK, "{uri}");
            for method in [Method::POST, Method::PUT, Method::DELETE] {
                let (status, allow) = answer(method.clone(), uri);
                assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{method} {uri}");
                assert_eq!(
                    allow.as_ref().and_then(|a| a.to_str().ok()),
                    Some("GET, HEAD")
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 F3 (perf #4). The watcher slept a fixed 80 ms after the first event
    /// of every save, 80 to 92% of each save on the author's projects, while every editor's
    /// save finishes its events within about a millisecond. It now waits for its events to
    /// stop, so a save reaches its open page in the render's time plus a short quiet
    /// period.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_save_reaches_its_open_page_without_a_fixed_wait() {
        let dir = scratch("quiet");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        let page = dir.join("index.tmd");
        std::fs::write(&page, "---\ntitle: Home\n---\n\nFirst.\n").unwrap();
        let live = Live::start(&dir);
        let mut tab = live.open("index.tmd");
        until("the first build", || {
            live.body("index.tmd").contains("First.")
        });
        let drain = |tab: &mut broadcast::Receiver<String>| while tab.try_recv().is_ok() {};
        std::thread::sleep(Duration::from_millis(200));
        drain(&mut tab);

        let mut took = Vec::new();
        for i in 0..5 {
            let marker = format!("Saved {i}.");
            let started = std::time::Instant::now();
            std::fs::write(&page, format!("---\ntitle: Home\n---\n\n{marker}\n")).unwrap();
            until("the save reaches the page", || {
                live.body("index.tmd").contains(&marker)
            });
            took.push(started.elapsed());
            std::thread::sleep(Duration::from_millis(100));
            drain(&mut tab);
        }
        took.sort();
        let median = took[took.len() / 2];
        assert!(
            median < Duration::from_millis(60),
            "a save took {median:?} to reach its open page (all: {took:?})"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2026-09-24 invalidation #10. `reload_open_tabs` drops every page's state so the
    /// reload re-renders against the new site, and a page nobody has open has none. A build
    /// already in flight for such a page used to put a state back when it finished, carrying
    /// the render defaults it captured before the drop: measured, a `bibliography:` switched
    /// in `_site.yml` while a closed page's cell ran left that page serving the old citation
    /// on every later GET. Both publishing steps of a build must leave a dropped page alone.
    #[test]
    fn a_build_in_flight_does_not_bring_back_a_dropped_page_state() {
        let dir = scratch("resurrect");
        std::fs::write(dir.join("_site.yml"), "title: T\n").unwrap();
        std::fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nProse.\n").unwrap();
        let (project, _app, _b, _f) = project_and_app(&dir);
        let page = project.site.lock().page("index.tmd").cloned().unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(build_page(&project, "index.tmd", None));
        assert!(
            project.pages.lock().is_empty(),
            "the post-exec publish recreated the state of a page nobody has open"
        );
        publish_pre_exec_body(&project, "index.tmd", &page, &[]);
        assert!(
            project.pages.lock().is_empty(),
            "the pre-exec publish recreated the state of a page nobody has open"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Build one cell-free page of `files` on the bypass lane and return the dev menu's
    /// diagnostics exactly as the websocket carries them.
    fn wire_diagnostics(tag: &str, files: &[(&str, &str)], rel: &str) -> Vec<serde_json::Value> {
        let dir = std::env::temp_dir().join(format!("tali-wirediag-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (name, body) in files {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        let site =
            taliesin_core::site::Site::discover_with(&dir, taliesin_core::DraftMode::Include);
        let project = Arc::new(Project {
            dir: dir.clone(),
            site: parking_lot::Mutex::new(site),
            pages: parking_lot::Mutex::new(HashMap::new()),
            exec_lane: Mutex::new(ExecLane::default()),
            scope: None,
            records: Mutex::new(HashMap::new()),
        });
        open_page(&project, rel);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(build_page(&project, rel, None));
        let wire = protocol::diagnostics(&project.pages.lock()[rel].doc.diagnostics);
        let _ = std::fs::remove_dir_all(&dir);
        let v: serde_json::Value = serde_json::from_str(&wire).unwrap();
        v["messages"].as_array().cloned().unwrap_or_default()
    }

    /// The dev menu shows a defect at the severity its validator gave it. Every preview
    /// diagnostic was built with `Diagnostic::warn`, so a missing image the gate fails on
    /// arrived amber and the status dot never went red (audit 2026-09-24 B4, vestigial #4).
    #[test]
    fn an_error_reaches_the_dev_menu_as_an_error() {
        let msgs = wire_diagnostics(
            "severity",
            &[
                ("_site.yml", "title: T\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\n![a chart](nope.png)\n",
                ),
            ],
            "index.tmd",
        );
        let missing = msgs
            .iter()
            .find(|m| m["message"].as_str().unwrap_or("").contains("nope.png"))
            .unwrap_or_else(|| panic!("the missing image is reported: {msgs:?}"));
        assert_eq!(missing["level"], "error", "{missing}");
    }

    /// A project diagnostic in the dev menu is located, so the row is clickable, and its
    /// file resolves from the page's own folder (the client joins it onto `baseDir`). It
    /// was pinned on `_site.yml` with no line, so a config typo could not be clicked and
    /// a nested page would have resolved it in the wrong folder.
    #[test]
    fn a_project_diagnostic_is_clickable_from_a_nested_page() {
        let msgs = wire_diagnostics(
            "located",
            &[
                ("_site.yml", "title: T\ntitel: oops\n"),
                ("index.tmd", "---\ntitle: Home\n---\n\nHi.\n"),
                ("posts/p.tmd", "---\ntitle: P\n---\n\nBody.\n"),
            ],
            "posts/p.tmd",
        );
        let typo = msgs
            .iter()
            .find(|m| m["message"].as_str().unwrap_or("").contains("titel"))
            .unwrap_or_else(|| panic!("the config typo is reported: {msgs:?}"));
        assert_eq!(typo["file"], "../_site.yml", "{typo}");
        assert_eq!(typo["line"], 2, "{typo}");
    }
}

#[cfg(test)]
mod session_key_tests {
    //! What a server publishes as its identity, for the target forms that differ.
    //!
    //! [`Resolved::session_key`] is the name a second `preview` of the same project
    //! recognizes as itself (`bind_with_fallback`'s incumbent check, and the answer on
    //! [`crate::serve::IDENTITY_PATH`]). The case worth pinning is the loose document: the
    //! key must be the *document*, not the directory it happens to sit in, or a preview of
    //! one scratch file would claim every unrelated `.tmd` beside it. This was a two-sided
    //! pin until Wave 13 cut `taliesin run`, which owned the other derivation; the
    //! surviving side is the one the server actually publishes.
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tali-sesskey-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::canonicalize(&d).unwrap()
    }

    /// A document with no ancestor `_site.yml`: the case that broke. Both sides must
    /// answer the *document*, not the directory it sits in.
    #[test]
    fn an_out_of_project_document_keys_on_itself_on_both_sides() {
        let dir = tmp("loose");
        let doc = dir.join("scratch.tmd");
        std::fs::write(&doc, "---\ntitle: S\n---\n\nProse.\n").unwrap();

        let served = resolve_target(Target::at(doc.clone())).unwrap();
        assert_eq!(
            served.session_key(),
            doc,
            "the server must publish the document it serves, not {}",
            dir.display()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A document inside a project keys on the *project*, so every page of a book shares
    /// one session (one kernel set, one `_freeze/` writer) rather than one per chapter.
    #[test]
    fn a_document_inside_a_project_keys_on_the_project_on_both_sides() {
        let dir = tmp("project");
        std::fs::create_dir_all(dir.join("chapters")).unwrap();
        std::fs::write(dir.join("_site.yml"), "title: Book\n").unwrap();
        std::fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nProse.\n").unwrap();
        let ch = dir.join("chapters/ch9.tmd");
        std::fs::write(&ch, "---\ntitle: Nine\n---\n\nProse.\n").unwrap();

        let served = resolve_target(Target::at(ch.clone())).unwrap();
        assert_eq!(served.session_key(), dir, "a page keys on its project root");

        // And the project's own front door lands on that same key, so `preview <dir>` and
        // `preview <dir>/chapters/ch9.tmd` are one server, not two.
        let whole = resolve_target(Target::at(dir.clone())).unwrap();
        assert_eq!(whole.session_key(), dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A lone document with no ancestor `_site.yml` is legitimate and must keep resolving.
    /// Only the *directory* form is refused.
    #[test]
    fn a_loose_document_still_resolves() {
        let dir = tmp("loose-doc");
        let doc = dir.join("scratch.tmd");
        std::fs::write(&doc, "---\ntitle: S\n---\n\nProse.\n").unwrap();
        assert!(
            resolve_target(Target::at(doc)).is_ok(),
            "a lone document is not a project and needs none"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A document whose extension is not in `ACCEPTED_SOURCE_EXTS` is refused: the site
    /// walker will never discover a `note.md`, so previewing it would show a page no
    /// `build <dir>` ever writes.
    #[test]
    fn a_non_source_document_is_refused() {
        let dir = tmp("md-doc");
        let doc = dir.join("note.md");
        std::fs::write(&doc, "# A markdown note\n").unwrap();
        let err = resolve_target(Target::at(doc)).expect_err("a .md is not a source document");
        assert!(
            err.to_string().contains("not a Taliesin source document"),
            "says why: {err}"
        );
        assert!(
            err.to_string().contains(".tmd"),
            "names the accepted extension: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A document the project does not publish (a partial, or a book chapter `chapters:`
    /// leaves out) has no page to open, so the preview opens the project's home page. It did
    /// that without a word, and the document's own URL answered 404 (audit 2026-09-24,
    /// config-seam #16); the preview now names the document it could not open.
    #[test]
    fn a_document_the_project_does_not_publish_is_named() {
        let dir = tmp("unpublished");
        std::fs::create_dir_all(dir.join("ch")).unwrap();
        std::fs::create_dir_all(dir.join("_parts")).unwrap();
        std::fs::write(
            dir.join("_site.yml"),
            "title: B\nchapters:\n  - index.tmd\n  - ch/a.tmd\n",
        )
        .unwrap();
        for (file, title) in [("index.tmd", "Home"), ("ch/a.tmd", "A"), ("ch/b.tmd", "B")] {
            std::fs::write(
                dir.join(file),
                format!("---\ntitle: {title}\n---\n\nProse.\n"),
            )
            .unwrap();
        }
        std::fs::write(dir.join("_parts/p.tmd"), "A partial.\n").unwrap();

        for doc in ["ch/b.tmd", "_parts/p.tmd"] {
            let served = resolve_target(Target::at(dir.join(doc))).unwrap();
            let warning = served
                .unpublished_doc_warning()
                .unwrap_or_else(|| panic!("{doc} is not a page, and the preview must say so"));
            assert!(warning.contains(doc), "names the document: {warning}");
        }
        // A listed chapter, and the project itself, open where they should.
        let listed = resolve_target(Target::at(dir.join("ch/a.tmd"))).unwrap();
        assert_eq!(listed.unpublished_doc_warning(), None);
        let whole = resolve_target(Target::at(dir.clone())).unwrap();
        assert_eq!(whole.unpublished_doc_warning(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A symlinked page belongs to the project its link sits in, for every verb: the site
    /// build publishes it there, so the preview opens it there. The preview resolved the
    /// link to its target first and served the target's folder as a project of one
    /// document, with no nav, while the page's own URL answered 404 (audit 2026-09-24,
    /// config-seam #14).
    #[cfg(unix)]
    #[test]
    fn a_symlinked_page_is_previewed_in_the_project_of_its_link() {
        let dir = tmp("symlinked");
        std::fs::create_dir_all(dir.join("site/posts")).unwrap();
        std::fs::create_dir_all(dir.join("shared")).unwrap();
        std::fs::write(dir.join(".git"), "").unwrap();
        std::fs::write(dir.join("site/_site.yml"), "title: S\n").unwrap();
        std::fs::write(dir.join("site/index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
        std::fs::write(
            dir.join("shared/real.tmd"),
            "---\ntitle: Real\n---\n\nBody.\n",
        )
        .unwrap();
        std::os::unix::fs::symlink("../../shared/real.tmd", dir.join("site/posts/link.tmd"))
            .unwrap();

        let served = resolve_target(Target::at(dir.join("site/posts/link.tmd"))).unwrap();
        assert_eq!(
            served.root,
            dir.join("site"),
            "served as a page of its project"
        );
        assert_eq!(
            focus_url(&served.site, served.doc.as_deref().unwrap()).as_deref(),
            Some("posts/link.html"),
            "and opened at the URL the build publishes it at"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A directory with no `_site.yml` is not a project, and is refused before a port is bound.
    #[test]
    fn a_directory_without_site_yml_is_refused() {
        let dir = tmp("not-a-project");
        std::fs::write(dir.join("a.tmd"), "---\ntitle: A\n---\n\nProse.\n").unwrap();
        let err = resolve_target(Target::at(dir.clone())).expect_err("not a project");
        assert!(err.to_string().contains("no _site.yml"), "says why: {err}");
        assert!(
            err.to_string().contains("<page>.tmd"),
            "offers the fix: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
