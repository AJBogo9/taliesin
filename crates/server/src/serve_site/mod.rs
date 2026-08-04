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
use axum::extract::{Path as AxumPath, Query, State};
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
use taliesin_core::{Block, BlockOp, Page, Site, diff_blocks};
use tokio::sync::{broadcast, mpsc};

use crate::protocol::{self, Diagnostic};
use crate::serve::{
    CLIENT_JS, FAVICON, STATUS_CSS, bind_with_fallback, js_str, lan_url, local_ip,
    new_session_token, open_in_browser, percent_decode, print_qr, with_host_guard, with_identity,
    with_lan_guard, ws_origin_ok,
};

mod exec_pool;
use exec_pool::ExecPool;

/// The whole live site: the root project plus any mounted sub-projects, all served
/// through the same per-page live path. One builder task + one file watcher drive every
/// project; a request routes to a project by URL prefix (see [`match_mount`]).
struct SiteApp {
    /// The root site (URL prefix `""`).
    root: Arc<Project>,
    /// `mounts:` — other taliesin projects (e.g. a docs `book` or a gallery exhibit),
    /// each served under a URL prefix. Every mount is a full [`Project`]: its pages
    /// execute live and hot-reload exactly like the root's, so a mounted `{python}`/`{r}`
    /// cell runs in `preview` (not just in a static `build`).
    mounts: Vec<MountPoint>,
    /// Page rel-paths queued for a (re)build by the executor worker.
    build_tx: mpsc::UnboundedSender<BuildMsg>,
    /// The bypass lane for pages that need no kernel (AP3-1). See [`SiteApp::queue_build`].
    fast_tx: mpsc::UnboundedSender<BuildMsg>,
    /// Whether the server is loopback-bound (i.e. not `--host`). Gates whether a
    /// loopback *origin* may open the control-channel ws (see [`origin_allowed`]).
    loopback_bound: bool,
}

impl SiteApp {
    /// The project a build message / request targets: the root (key `""`) or a mount by
    /// its prefix. `None` if a stale key names a mount that is no longer present.
    fn project(&self, key: &ProjectKey) -> Option<&Arc<Project>> {
        if key.0.is_empty() {
            Some(&self.root)
        } else {
            self.mounts
                .iter()
                .find(|m| m.prefix == key.0)
                .map(|m| &m.project)
        }
    }

    /// Queue a page rebuild on the lane that fits it (AP3-1).
    ///
    /// **The defect.** One builder task consumed the whole server's build queue — root and
    /// every mount alike — awaiting each page to completion. It serialized on the wrong
    /// predicate: a page with **no code cells** needs no kernel, yet it queued behind
    /// kernel work it would never use. Measured on a two-page preview with a warm pool, a
    /// cell-free page's trivial prose edit landed in **0.11 s** alone and **12.15 s**
    /// (110x) when an unrelated page was 1.2 s into a 12 s `{python}` cell. That is the
    /// normal shape of this tool's own site, which `mounts:` both dogfood books beside a
    /// corpus that has genuinely slow cells.
    ///
    /// **Why not just parallelise the builder.** Serialization is what makes the shared
    /// warm pool and the task-owned `ExecPool` race-free, and `ExecPool` is under the M6a
    /// freeze. So there are two *serial* lanes, not concurrent executors: the exec lane
    /// owns every pool and is unchanged, and the fast lane owns nothing and never touches
    /// a pool. Neither lane gains any concurrency of its own.
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
    fn queue_build(&self, key: ProjectKey, rel: String) {
        let cell_free = self
            .project(&key)
            .and_then(|p| p.pages.lock().get(&rel).map(|ps| ps.doc.cell_free))
            .unwrap_or(false);
        let tx = if cell_free {
            &self.fast_tx
        } else {
            &self.build_tx
        };
        let _ = tx.send(BuildMsg::Build(key, rel));
    }
}

/// A project's routing + build identity: `""` for the root site, otherwise the mount
/// prefix (e.g. `gallery/course`). The builder keys each project's [`ExecPool`] by it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
struct ProjectKey(String);

/// A servable project: the root site or a mounted sub-project. Owns the per-project live
/// state the builder and router act on — the discovered [`Site`], plus the live per-page
/// block state + broadcast channels, created lazily on first visit.
struct Project {
    key: ProjectKey,
    dir: PathBuf,
    site: Mutex<Site>,
    pages: Mutex<HashMap<String, PageState>>,
    /// Per-page handles for stopping a run in flight. Lives on the project (not the exec
    /// pool) because the pool is owned by the build task, and the interrupt endpoint is a
    /// request handler that must reach a run without waiting on that task's lock.
    runs: crate::run_control::RunRegistry,
    /// Set when this project is one document previewed on its own (`preview <file.tmd>`
    /// with no ancestor `_site.yml`): the document it is scoped to.
    ///
    /// Load-bearing on **re-discovery**, not just at boot. A save that touches `_site.yml`
    /// or creates a `.tmd` re-runs discovery, and an unscoped re-run would quietly widen a
    /// one-document preview into "every `.tmd` in the parent directory" — the scoping would
    /// hold until the first save and then evaporate.
    scope: Option<PathBuf>,
}

impl Project {
    /// Re-discover this project the same way it was discovered, scope included.
    fn rediscover(&self) -> Site {
        match &self.scope {
            Some(file) => Site::discover_single(file),
            None => Site::discover_with(&self.dir, taliesin_core::DraftMode::Include),
        }
    }
}

/// A mounted sub-project served under `prefix` (e.g. `gallery/course`).
struct MountPoint {
    prefix: String,
    project: Arc<Project>,
}

/// Longest-prefix match of a request `path` against mount `prefixes` (each like
/// `gallery/course`). Returns the winning mount index and `path` with that prefix (and
/// its trailing `/`) removed; `None` when nothing matches, i.e. the request belongs to
/// the root project. Pure — this is the routing seam, unit-tested without any
/// `Site`/kernel; the live per-page wiring on top is browser-verified.
fn match_mount<'a>(prefixes: &[String], path: &'a str) -> Option<(usize, &'a str)> {
    let mut best: Option<(usize, usize)> = None; // (index, prefix byte-len)
    for (i, p) in prefixes.iter().enumerate() {
        let hit = path == p || path.strip_prefix(p).is_some_and(|r| r.starts_with('/'));
        if hit && best.is_none_or(|(_, len)| p.len() > len) {
            best = Some((i, p.len()));
        }
    }
    best.map(|(i, _)| {
        let sub = path.strip_prefix(&prefixes[i]).unwrap_or("");
        (i, sub.strip_prefix('/').unwrap_or(sub))
    })
}

/// Resolve a request `path` to the project that owns it (root or a mount) and the path
/// with the mount prefix stripped. The root project is the fallback when no mount matches.
fn resolve_project<'a>(app: &'a SiteApp, path: &'a str) -> (&'a Arc<Project>, &'a str) {
    let prefixes: Vec<String> = app.mounts.iter().map(|m| m.prefix.clone()).collect();
    match match_mount(&prefixes, path) {
        Some((i, sub)) => (&app.mounts[i].project, sub),
        None => (&app.root, path),
    }
}

/// The project source `rel` a client's `?page=` sub-key names, or `None` when it names no
/// page in this project.
///
/// The `None` case is load-bearing: the ws handler refuses such a connection instead of
/// creating a `PageState` for it. A `PageState` is a 256-slot broadcast ring and nothing
/// evicts it, so allocating one per unrecognized key let any peer that can reach the socket
/// grow the map without bound by reconnecting with fresh garbage. Nothing is lost by
/// refusing: `build_page` already returns immediately for a key `Site::page` cannot resolve,
/// so the entry could only ever hold an empty document.
fn resolve_page_rel(project: &Project, sub: &str) -> Option<String> {
    project.site.lock().page(sub).map(|p| p.rel.clone())
}

/// The `?page=` websocket key for a page: prefixed by the project's mount so the ws
/// handler routes the client back to the owning project (the root uses the bare rel).
fn ws_page_key(project: &Project, rel: &str) -> String {
    if project.key.0.is_empty() {
        rel.to_string()
    } else {
        format!("{}/{}", project.key.0, rel)
    }
}

/// Attribute a changed absolute path to the project whose root is its deepest ancestor,
/// returning that project's key + the path relative to that root. `None` if under no
/// project root. Pure — the watcher's routing seam; unit-tested without any I/O.
fn classify_change(roots: &[(ProjectKey, PathBuf)], abs: &Path) -> Option<(ProjectKey, PathBuf)> {
    roots
        .iter()
        .filter_map(|(key, root)| {
            abs.strip_prefix(root)
                .ok()
                .map(|rel| (root.as_os_str().len(), key.clone(), rel.to_path_buf()))
        })
        .max_by_key(|(len, _, _)| *len)
        .map(|(_, key, rel)| (key, rel))
}

/// A job for the executor worker: rebuild a page, or restart its kernel first
/// (the dev-menu "Restart kernel" action) then rebuild.
enum BuildMsg {
    Build(ProjectKey, String),
    Restart(ProjectKey, String),
    /// An explicit run from `taliesin run` (the editor's Run Cell): build this page with
    /// the executor capped to `scope`, then announce completion on the page's broadcast
    /// tagged with `run_id` so the requesting client knows its own run finished.
    ///
    /// It rides the same queue as an ordinary rebuild rather than jumping it, because the
    /// queue is what keeps one page's builds totally ordered. A run that overtook a
    /// pending save would execute stale source.
    Run {
        key: ProjectKey,
        rel: String,
        scope: crate::runspec::RunScope,
        run_id: String,
        /// The [`crate::run_control::RunControl`] epoch when this run was **requested**, not
        /// when it starts. The build lane is serialized, so a run can wait behind the
        /// session's own startup pass; a Ctrl-C during that pass must invalidate this
        /// message too, and only an epoch taken at request time can say so.
        epoch: u64,
    },
}

struct PageState {
    doc: PageDoc,
    tx: broadcast::Sender<String>,
}

/// How far a build may execute, and who is waiting to hear that it finished.
///
/// An ordinary rebuild is [`RunRequest::none`]: uncapped, nobody waiting. A `taliesin run`
/// carries a cap and a `run_id`, and the difference is confined to this one struct so the
/// two paths cannot drift into two different builds.
#[derive(Clone)]
struct RunRequest {
    scope: crate::runspec::RunScope,
    /// `Some` only for an explicit run, i.e. exactly when a client is blocked waiting for
    /// a terminal `run-done`.
    run_id: Option<String>,
    /// The run-control epoch this run was **requested** at; `None` for a rebuild nobody
    /// asked for (a file save), which is never pre-emptively cancelled.
    epoch: Option<u64>,
}

impl RunRequest {
    /// An ordinary rebuild: whole document, nobody waiting.
    fn none() -> Self {
        Self {
            scope: crate::runspec::RunScope::All,
            run_id: None,
            epoch: None,
        }
    }
}

/// Guarantees a waiting `taliesin run` gets exactly one terminal message.
///
/// `build_page` has several early returns and can panic; a client blocked on the page
/// broadcast has no other way to learn the run is over, and a missing terminal message
/// reads to it as a hang. So the announcement is a drop guard: whatever path leaves the
/// build, the client is answered. Armed only when a `run_id` is present, so an ordinary
/// rebuild costs nothing and broadcasts nothing new.
struct RunAnnounce {
    project: Arc<Project>,
    rel: String,
    run_id: Option<String>,
}

impl RunAnnounce {
    fn new(project: &Arc<Project>, rel: &str, run_id: Option<String>) -> Self {
        Self {
            project: project.clone(),
            rel: rel.to_string(),
            run_id,
        }
    }

    /// Announce the outcome and disarm.
    fn finish(&mut self, status: &str, message: Option<&str>) {
        let Some(run_id) = self.run_id.take() else {
            return;
        };
        if let Some(ps) = self.project.pages.lock().get(&self.rel) {
            let _ = ps.tx.send(protocol::run_done(
                Some(&self.rel),
                &run_id,
                status,
                message,
            ));
        }
    }

    /// Disarm WITHOUT announcing: this pass is handing the run to another lane, which
    /// will answer instead. Announcing here would tell the client the run finished while
    /// its cells are still queued.
    fn defer(&mut self) {
        self.run_id = None;
    }
}

impl Drop for RunAnnounce {
    fn drop(&mut self) {
        // Still armed at drop means an exit nobody accounted for (a panic unwinding
        // through, or a future early return added later). Answer rather than hang.
        self.finish("error", Some("run ended without completing"));
    }
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
    theme_css: String,
    theme_default: String,
    /// The page's own front-matter `include-*`/`css` (merged after the site's).
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
}

impl PageDoc {
    fn body_html(&self) -> String {
        let mut s = String::new();
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
            let file = file.canonicalize().unwrap_or(file);
            if !file.is_file() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("no document at {}", file.display()),
                ));
            }
            match taliesin_core::site::enclosing_site_root(&file) {
                // In a project: serve the project, open at this page.
                Some(root) => (root, Some(file)),
                // Not in a project: a project of exactly this document, rooted at its
                // directory so relative images/includes/assets resolve as they always did.
                None => (
                    file.parent()
                        .unwrap_or(std::path::Path::new("."))
                        .to_path_buf(),
                    Some(file),
                ),
            }
        }
    };
    let root = root.canonicalize().unwrap_or(root);
    // Only an OUT-of-project document narrows discovery. A document inside a project must
    // discover the whole project, or its nav and cross-page links would be the very orphan
    // this routing exists to prevent.
    let scoped = scope
        .as_deref()
        .filter(|f| taliesin_core::site::enclosing_site_root(f).is_none())
        .map(|f| f.to_path_buf());
    let site = match &scoped {
        Some(file) => Site::discover_single(file),
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
    /// [`crate::serve::IDENTITY_PATH`] with, the incumbent it recognizes as itself, and the
    /// name its session hint is filed under.
    ///
    /// It is *what this server serves*, which for an out-of-project document is that
    /// document — it is a project of just that document — and **not** [`Resolved::root`],
    /// the directory the document happens to sit in. The two genuinely differ: that
    /// directory may hold unrelated `.tmd` files this server discovered nothing about and
    /// would 404, so answering with it claims pages that are not there.
    ///
    /// Publishing the directory is what broke `taliesin run` when the single-document
    /// server was folded in here. That server published the document (`serve/mod.rs`'s
    /// `app.path`, pre-`e6a99ec4`) and [`crate::run_cmd`] still asks for the document, so a
    /// run found no hint, started a session, and then could not find that one either: 45
    /// seconds to report that a live server it had just spawned was not there.
    /// `crates/server/tests/run_session_discovery.rs` pins both halves.
    ///
    /// `root` stays the filesystem base for serving assets and resolving includes; only
    /// the identity moves.
    fn session_key(&self) -> PathBuf {
        self.scoped.clone().unwrap_or_else(|| self.root.clone())
    }
}

/// What [`resolve_target`] worked out about the thing being previewed.
struct Resolved {
    root: PathBuf,
    site: Site,
    /// The document discovery is narrowed to, for an out-of-project single document.
    /// Carried onto [`Project::scope`] so a re-discovery cannot silently widen it.
    scoped: Option<PathBuf>,
    /// The document the browser should open at, in project and single-document cases alike.
    doc: Option<PathBuf>,
}

/// The URL a scoped document lives at: its page, or the deck it is. Used both to open the
/// browser at it and to answer the project root with it.
fn focus_url(site: &Site, file: &std::path::Path) -> Option<String> {
    let same = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()) == file;
    site.pages
        .iter()
        .find(|p| same(&p.input))
        .map(|p| p.url.clone())
        .or_else(|| {
            site.decks
                .iter()
                .find(|d| same(&d.input))
                .map(|d| d.url.clone())
        })
}

/// Entry point for `taliesin preview <dir|file.tmd>`.
pub fn run(
    target: Target,
    port: u16,
    open: bool,
    expose: bool,
    headless: bool,
) -> std::io::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(serve(target, port, open, expose, headless));
    // `serve` returns on a shutdown signal (see `crate::serve::shutdown_signal`);
    // force the runtime down so the builder task that owns the warm pool + kernels is
    // dropped promptly, running its teardown (the forkserver group-kill + kernel
    // SIGKILLs). Bounded so a wedged task can't hang exit; the kills are synchronous.
    rt.shutdown_timeout(std::time::Duration::from_secs(5));
    result
}

async fn serve(
    target: Target,
    port: u16,
    open: bool,
    expose: bool,
    headless: bool,
) -> std::io::Result<()> {
    let start = std::time::Instant::now();
    // Preview shows drafts inline (nav/listings/prev-next, badged); build/publish exclude
    // them. See `docs/superpowers/specs/2026-07-16-draft-aware-preview-design.md`.
    let resolved = resolve_target(target)?;
    let session_key = resolved.session_key();
    let Resolved {
        root,
        site,
        scoped,
        doc,
    } = resolved;
    // Where to point the browser: a document target opens at its own page rather than at
    // the project's home, so `preview chapter-7.tmd` shows chapter 7.
    let focus = doc.as_deref().and_then(|f| focus_url(&site, f));
    for w in &site.warnings {
        // "no _site.yml at …" is a finding about a *project*. When the author asked to see
        // one document, its absence is the expected case — it is precisely what put us on
        // the single-document path — so reporting it reads as a fault where there is none.
        if scoped.is_some() && taliesin_core::site::is_missing_config_warning(w) {
            continue;
        }
        crate::log::warn(w);
    }
    let page_count = site.pages.len();
    // Discover any `mounts:` sub-projects (e.g. a docs book under /docs) once. Each becomes
    // a full `Project`, so its pages execute live + hot-reload under its URL prefix.
    let mounts: Vec<MountPoint> = site
        .config
        .mounts
        .clone()
        .into_iter()
        .filter_map(|m| {
            // Containment (item 80) is enforced once in `load_config`, so a refusal here can
            // only come from a mount that did not travel through it. Kept anyway: this is
            // the call site that turns a path into a live HTTP root plus an executor, and it
            // must not be the one place that trusts the string.
            let mroot = match m.resolve(&root) {
                Ok(p) => p,
                Err(why) => {
                    crate::log::warn(&m.refusal_warning(&root, why));
                    return None;
                }
            };
            let mroot = mroot.canonicalize().unwrap_or(mroot);
            if !mroot.is_dir() {
                crate::log::warn(&format!(
                    "mount '{}': no directory at {}",
                    m.at,
                    mroot.display()
                ));
                return None;
            }
            crate::log::watching(
                &mroot.display().to_string(),
                &format!("mounted at /{}/", m.at),
            );
            let msite = Site::discover_with(&mroot, taliesin_core::DraftMode::Include);
            Some(MountPoint {
                prefix: m.at.clone(),
                project: Arc::new(Project {
                    key: ProjectKey(m.at),
                    dir: mroot,
                    site: Mutex::new(msite),
                    pages: Mutex::new(HashMap::new()),
                    runs: Default::default(),
                    // A mount is always a whole project; only the root can be a single
                    // document, and a single document declares no `mounts:`.
                    scope: None,
                }),
            })
        })
        .collect();
    // A project with nothing to serve: `check <dir>` already exits 1 here, while `preview`
    // used to bind a port, 404 `/`, and boot the kernel pool for nothing. The two front
    // doors must agree. A page-less root that only `mounts:` sub-projects is legitimate —
    // it is how a docs container is previewed — so it is not empty.
    if page_count == 0 && mounts.is_empty() && site.decks.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no .tmd pages found under {}", root.display()),
        ));
    }
    let (build_tx, build_rx) = mpsc::unbounded_channel();
    let (fast_tx, fast_rx) = mpsc::unbounded_channel();
    let app = Arc::new(SiteApp {
        root: Arc::new(Project {
            key: ProjectKey(String::new()),
            dir: root.clone(),
            site: Mutex::new(site),
            pages: Mutex::new(HashMap::new()),
            runs: Default::default(),
            scope: scoped,
        }),
        mounts,
        build_tx,
        fast_tx,
        loopback_bound: !expose,
    });

    spawn_builder(app.clone(), build_rx);
    spawn_fast_builder(app.clone(), fast_rx);
    spawn_watcher(app.clone());

    // With --host the whole site is LAN-reachable; gate non-loopback access behind a
    // per-session token threaded into the LAN URL/QR (loopback stays token-free).
    let token: Option<Arc<str>> = expose.then(|| Arc::from(new_session_token()));
    // Under --host, the bound LAN IP is a legitimate `Host`; in loopback mode only
    // loopback names are (the DNS-rebinding allowlist).
    let lan_ip: Option<Arc<str>> = expose
        .then(local_ip)
        .flatten()
        .map(|ip| Arc::from(ip.to_string()));

    let router = Router::new()
        .route("/favicon.ico", get(favicon))
        .route(taliesin_core::PREVIEW_MERMAID_PATH, get(mermaid_lib_js))
        .route("/search-index.js", get(search_index_js))
        .route("/ws", get(ws_handler))
        .route(crate::serve::RUN_PATH, axum::routing::post(run_handler))
        .route(
            crate::serve::INTERRUPT_PATH,
            axum::routing::post(interrupt_handler),
        )
        .route("/og/{name}", get(og_card))
        .route("/og-preview", get(og_card_preview))
        .fallback(page_or_asset)
        .with_state(app.clone());
    let router = with_identity(router, &session_key);
    let router = with_lan_guard(router, token.clone());
    let router = with_host_guard(router, lan_ip);

    let (listener, addr) = bind_with_fallback(port, expose, &session_key).await?;
    let port = addr.port();
    // Publish where this project's session is listening, so `taliesin run` attaches to
    // THIS server rather than starting a second one. A second server would mean a second
    // kernel set and a second `_freeze/` writer for one project, which is how stale output
    // gets published. Written after the bind, so the port recorded is the port we got
    // (`bind_with_fallback` may have stepped past a busy one).
    crate::session::write_hint(&session_key, port);
    let local = format!("http://127.0.0.1:{port}");
    let network = expose
        .then(local_ip)
        .flatten()
        .map(|ip| lan_url(&format!("http://{ip}:{port}"), token.as_ref()));

    // A session has nobody watching its console: the screen clear, banner, QR and hints
    // are for a preview you launched and are looking at. Announce one line instead, so
    // `taliesin run` still leaves something greppable behind if it misbehaves.
    if headless {
        crate::log::info(&format!("session listening on {local}"));
    } else {
        crate::log::clear_screen();
        crate::log::banner(taliesin_core::VERSION);
        crate::log::ready(&local, start.elapsed());
        if let Some(net) = &network {
            crate::log::network(net);
        } else if expose {
            crate::log::warn("--host set, but no LAN address was found");
        }
        crate::log::first_run_notice();
        crate::log::keys_hint();
        crate::log::watching(
            &root.display().to_string(),
            &format!("site, {page_count} pages"),
        );
        if let Some(net) = &network {
            print_qr(net);
        }
        if expose && std::env::var_os("TALIESIN_NO_EXEC").is_none() {
            crate::log::warn(
                "code cells run on this machine; only serve documents you trust over --host \
                 (pass --no-exec to preview as source)",
            );
        }
    }
    if open {
        // A document target opens at its own page; a project target at its home.
        match &focus {
            Some(url) => open_in_browser(&format!("{local}/{url}")),
            None => open_in_browser(&local),
        }
    }
    // `into_make_service_with_connect_info` surfaces the peer address to the LAN guard
    // (loopback detection); harmless when no guard is installed.
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
    // Clean exit: retract the hint so the next `taliesin run` starts a session instead of
    // dialling a dead port. A SIGKILL skips this, which is exactly why a client proves the
    // hint with the identity handshake rather than trusting it.
    crate::session::clear_hint(&session_key);
    outcome
}

// --- HTTP ---------------------------------------------------------------

async fn favicon() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        FAVICON,
    )
}

/// Serve a preview OG card: find the page whose card hash matches `name` and render it
/// on demand (so the shared og:image tag is never a dead link during preview).
async fn og_card(
    State(app): State<Arc<SiteApp>>,
    AxumPath(name): AxumPath<String>,
) -> impl IntoResponse {
    let want = format!("og/{name}");
    let bytes = {
        let site = app.root.site.lock();
        site.pages.iter().find_map(|page| {
            let spec = taliesin_core::site::card_spec(&site, page);
            (taliesin_core::site::card_rel_path(&spec) == want)
                .then(|| taliesin_core::site::render_card(&spec))
        })
    };
    match bytes {
        Some(b) => ([(axum::http::header::CONTENT_TYPE, "image/png")], b).into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

/// Serve the branded OG social card for the CURRENT preview page (DX13), keyed by the
/// page's source `rel` (or output `url`) via [`taliesin_core::Site::page`] rather than the
/// content hash the shared `/og/{name}` route uses. The dev-menu card pane hits this so an
/// author can see their card without a build — and, unlike the hash route, it works before
/// `_site.yml` sets a `url:` (no card hash is ever surfaced without one, so `/og/{name}` is
/// unreachable then). Preview-only; the render is pure + offline (bundled font).
async fn og_card_preview(
    State(app): State<Arc<SiteApp>>,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let page_key = q.get("page").cloned().unwrap_or_default();
    let (project, sub) = resolve_project(&app, &page_key);
    let bytes = {
        let site = project.site.lock();
        site.page(sub).map(|page| {
            let spec = taliesin_core::site::card_spec(&site, page);
            taliesin_core::site::render_card(&spec)
        })
    };
    match bytes {
        Some(b) => ([(axum::http::header::CONTENT_TYPE, "image/png")], b).into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
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

/// Resolve a request to a page (rendered live) or a static asset under the root.
async fn page_or_asset(
    State(app): State<Arc<SiteApp>>,
    uri: axum::http::Uri,
) -> axum::response::Response {
    let path = percent_decode(uri.path().trim_start_matches('/'));
    // Route to the owning project (root or a mount) by URL prefix. A mount now serves
    // through the SAME live per-page path as the root, so its `{python}`/`{r}` cells
    // execute live in preview (replacing the old static pre-exec render of a mount).
    let (project, sub) = resolve_project(&app, &path);
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
    // 3) A deck referenced by `{{< embed >}}` (a standalone document, not a page/chapter):
    //    render it self-contained on the fly so the embedding iframe resolves in preview.
    let deck = { project.site.lock().deck(&lookup).cloned() };
    if let Some(deck) = deck
        && let Ok(src) = std::fs::read_to_string(&deck.input)
    {
        let base = deck.input.parent().unwrap_or(&project.dir).to_path_buf();
        // A deck is a page of the project too: it inherits the project-wide
        // `bibliography:`, and `Site::validate_shared_bibliography` counts a deck's
        // citations, so the two must agree about what a deck can resolve.
        let defaults = { project.site.lock().render_defaults() };
        let doc =
            taliesin_core::render_document_scoped_with_site(&src, &base, None, Some(&defaults));
        let stem = deck
            .url
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".html"))
            .unwrap_or("deck");
        // Same rewrite the page path applies below: a deck's author `.tmd` references
        // must resolve to `.html` here too, or preview disagrees with the build about
        // where a deck's links point (preview happens to serve the rendered page for a
        // `.tmd` URL, which is exactly what masks the build's version of this bug).
        let html = taliesin_core::site::rewrite_tmd_links(&taliesin_core::render_doc_to_page(
            &doc,
            stem,
            taliesin_core::OutputMode::Preview,
        ));
        // Click-to-source, which is one of the three load-bearing goals and was dead on
        // every deck served here: `client.js`'s `openSource` bails without `TALIESIN_DOC`,
        // so a deck's blocks carried `data-block-id`/`data-sourcepos` that nothing could
        // act on. The page path above injects this; the deck path never did.
        let doc_path = deck
            .input
            .canonicalize()
            .unwrap_or_else(|_| deck.input.clone());
        let doc_global = format!(
            "<script>window.TALIESIN_DOC = {{ path: \"{}\", baseDir: \"{}\", root: \"{}\" }};</script>",
            js_str(&doc_path.to_string_lossy()),
            js_str(&base.to_string_lossy()),
            js_str(&project.dir.to_string_lossy()),
        );
        return Html(match html.rfind("</body>") {
            Some(i) => format!("{}{doc_global}{}", &html[..i], &html[i..]),
            None => format!("{html}{doc_global}"),
        })
        .into_response();
    }
    // 4) A static asset under this project's root, else this project's own 404 page
    //    (with a 404 status) so preview mirrors the deployed `404.html`.
    let asset = serve_asset(&project.dir, &lookup);
    if asset.status() == axum::http::StatusCode::NOT_FOUND {
        let html = { project.site.lock().render_404_page() };
        return (axum::http::StatusCode::NOT_FOUND, Html(html)).into_response();
    }
    asset
}

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
        app.queue_build(project.key.clone(), rel.clone());
    }
    site_page_html(project, page)
}

/// A first-paint render without code execution (the worker fills outputs after).
/// Listing cards are expanded here so the blog index paints with its posts.
fn render_markdown_only(site: &taliesin_core::Site, page: &Page) -> PageDoc {
    let Ok(src) = std::fs::read_to_string(&page.input) else {
        return PageDoc {
            errored: true,
            ..Default::default()
        };
    };
    let base = page.input.parent().unwrap_or(Path::new("."));
    let mut doc = taliesin_core::render_document_scoped_with_site(
        &src,
        base,
        site.chapter_for(page),
        Some(&site.render_defaults()),
    );
    let toc = site.page_toc(page, doc.toc_explicit, &doc.blocks);
    // One shared finishing step (numbering, cross-refs + broken-ref warnings,
    // listing/about expansion, post decoration) so preview matches the build.
    let mut warnings = std::mem::take(&mut doc.warnings);
    site.finish_blocks(page, &mut doc.blocks, &mut warnings);
    // Resolved off the *finished* doc, exactly as the static build resolves it
    // (`Site::render_page_doc_warned`), so the first paint, every `full_render`, and
    // `_site/` cannot name one tab three ways.
    let tab_title = site.page_title(page, &doc);
    let diagnostics = warnings
        .iter()
        .map(|w| {
            let mut d = Diagnostic::warn(&w.message);
            if let Some(line) = w.line {
                d = d.at(w.file.clone(), line);
            }
            d
        })
        .collect();
    PageDoc {
        tab_title,
        toc,
        theme_css: doc.theme_css,
        theme_default: doc.theme_default,
        includes: doc.includes,
        blocks: doc.blocks,
        diagnostics,
        errored: false,
        generation: 0, // first paint; the exec pass bumps it when it splices outputs
        // The first-paint render never runs cells, so it learns nothing about this page's
        // lane: leave it on the safe one until a real build reports back (AP3-1).
        cell_free: false,
    }
}

/// Build the full live HTML for a page: theme + base + site CSS, the SSR body
/// wrapped in the site chrome, and the preview client scoped to this page's ws.
fn site_page_html(project: &Arc<Project>, page: &Page) -> String {
    // `tab_title` is the string the producer already resolved (`Site::page_title`) — the
    // very one the websocket re-asserts on connect, so the two cannot disagree. It is
    // deliberately NOT re-derived here if empty: that means the page has no live state at
    // all (the arm below has no body and no theme either), and re-composing half the title
    // policy at a second call site is the exact shape of the bug this replaced.
    let (tab_title, toc, theme_css, theme_default, body, page_includes, generation) = {
        let pages = project.pages.lock();
        let ps = pages.get(&page.rel);
        match ps {
            Some(ps) => (
                ps.doc.tab_title.clone(),
                ps.doc.toc,
                ps.doc.theme_css.clone(),
                ps.doc.theme_default.clone(),
                ps.doc.body_html(),
                ps.doc.includes.clone(),
                ps.doc.generation,
            ),
            None => (
                String::new(),
                false,
                String::new(),
                String::new(),
                String::new(),
                Default::default(),
                0,
            ),
        }
    };
    let chrome = { project.site.lock().page_chrome(page) };
    // Site-level `format: html:` includes first, then this page's own front matter.
    let mut includes = chrome.includes.clone();
    includes.merge(&page_includes);

    let (base_cls, toc_nav, toc_flag) = if toc {
        (
            "tali-site-main has-toc",
            "<nav id=\"TOC\" aria-label=\"Table of contents\"></nav>",
            "window.TALIESIN_TOC = true;",
        )
    } else {
        ("tali-site-main", "", "")
    };
    let main_cls = if chrome.wide {
        format!("{base_cls} tali-wide")
    } else {
        base_cls.to_string()
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
        js_str(&project.dir.to_string_lossy()),
    );
    let ws_path = format!(
        "/ws?page={}",
        encode_query(&ws_page_key(project, &page.rel))
    );
    // Cross-page Cmd-K search: point the palette at the lazy-loaded `search-index.js`
    // (depth-relative, served at the root). Empty for a project with no index.
    let search_cfg = if chrome.search_index.is_empty() {
        String::new()
    } else {
        format!("{};", chrome.search_index)
    };
    // Body links (author `.tmd` references) -> `.html`; chrome links already are.
    let body = taliesin_core::site::rewrite_tmd_links(&body);
    // The site's configured favicon (depth-relative); else the dev server's own.
    let favicon = if chrome.favicon.is_empty() {
        "<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.ico\" />".to_string()
    } else {
        taliesin_core::favicon_link(&chrome.favicon)
    };

    // A book lays out a sticky topbar + off-canvas chapter drawer over a centred reading
    // column (the live `#tali-root` + TOC), with prev/next-chapter under it; a website keeps
    // the navbar-on-top layout. (Kept structurally identical to the build path in page.rs.)
    let (body_class, layout) = match chrome.book_sidebar.as_deref() {
        Some(sidebar) => {
            // Keep this layout byte-aligned with the build path (`render/page.rs` book
            // branch): a sticky topbar + off-canvas chapter drawer (`sidebar`), then the
            // reading content centred in `.tali-book-main`. One column, always — a book
            // has no right rail (item 76), so `toc_nav` is unreachable here (`page_toc`
            // returns false for a book) and is deliberately not interpolated: the preview
            // must not paint a surface the build does not.
            (
                "tali-book-body",
                format!(
                    "{sidebar}\n<div class=\"tali-book-main\">\n\
                     <div class=\"tali-book-inner\">\n<main id=\"tali-root\">{body}</main>\n</div>\n\
                     {post_nav}</div>\n{footer}",
                    post_nav = chrome.post_nav_html,
                    footer = chrome.footer_html,
                ),
            )
        }
        None => (
            "tali-site",
            format!(
                "{navbar}\n<div class=\"{main_cls}\">\n<main id=\"tali-root\">{body}</main>\n\
                 {toc_nav}\n{post_nav}\n</div>\n{footer}",
                navbar = chrome.navbar_html,
                post_nav = chrome.post_nav_html,
                footer = chrome.footer_html,
            ),
        ),
    };

    // The live body: the site chrome + the mountable `#tali-root`, plus the dev-menu
    // mount. The websocket client drives everything after first paint.
    let body = format!("{layout}\n<div id=\"tali-controls\"></div>");
    let extra_head = format!("<style>{STATUS_CSS}</style>\n");
    let boot = protocol::boot_id();
    // Draft pages (preview only) power the dev-menu "Drafts" row. Root-absolute urls so a
    // link resolves from any page depth. A build ships neither this global nor the dev menu.
    let drafts_global = {
        let site = project.site.lock();
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
        format!("window.TALIESIN_DRAFTS=[{}];", items.join(","))
    };
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
        title: &tab_title,
        // Preview chrome defaults to English; the built `_site/` honours each
        // page's front-matter `lang:` via the core page builder.
        lang: "en",
        favicon: &favicon,
        theme_default: &theme_default,
        theme_css: &theme_css,
        with_site_css: true,
        // A live page can gain math at any edit, so always ship the KaTeX styles.
        ship_katex: true,
        extra_head: &extra_head,
        body_class: &format!(" class=\"{body_class}\""),
        include_in_header: &includes.in_header,
        include_before_body: &includes.before_body,
        body: &body,
        scripts_pre: &scripts_pre,
        scripts_post: &scripts_post,
        include_after_body: &includes.after_body,
        ..taliesin_core::PageParts::defaults()
    })
}

/// Minimal query-value encoding for a page rel in the ws URL (spaces only; `/`
/// and `-` are query-safe).
fn encode_query(s: &str) -> String {
    s.replace(' ', "%20")
}

// --- WebSocket ----------------------------------------------------------

async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    State(app): State<Arc<SiteApp>>,
) -> axum::response::Response {
    if !ws_origin_ok(&headers, app.loopback_bound) {
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

/// `POST /__taliesin/run`: execute part of a page and stream the result as NDJSON.
///
/// Loopback only, unconditionally. A run is on-demand code execution, and while the
/// preview already executes this project's cells on save, that is the author's own editor
/// driving it. Under `--host` the LAN token gates *viewing*; it must not also hand a
/// visitor a trigger. `taliesin run` is always local, so nothing legitimate is lost.
async fn run_handler(
    axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<SocketAddr>,
    State(app): State<Arc<SiteApp>>,
    axum::Json(req): axum::Json<crate::runspec::RunReq>,
) -> axum::response::Response {
    if !peer.ip().is_loopback() {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "runs are accepted from loopback only",
        )
            .into_response();
    }
    // `--no-exec` means "never run this document's code". A run request is the most
    // explicit possible ask, and refusing it plainly beats appearing to succeed while
    // executing nothing.
    if crate::exec::exec_disabled() {
        return (
            axum::http::StatusCode::CONFLICT,
            "this session was started with --no-exec",
        )
            .into_response();
    }

    let want = std::fs::canonicalize(&req.file).unwrap_or_else(|_| PathBuf::from(&req.file));
    let Some((project, rel)) = page_for_input(&app, &want) else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            format!("{} is not a page of this session's project", req.file),
        )
            .into_response();
    };

    let scope = req.scope();
    let run_id = uuid::Uuid::new_v4().to_string();

    // Subscribe BEFORE queueing, or a fast build can finish between the two and the
    // client waits forever for a `run-done` that was broadcast into an empty room.
    let rx = {
        let mut pages = project.pages.lock();
        pages
            .entry(rel.clone())
            .or_insert_with(|| PageState {
                doc: PageDoc::default(),
                tx: broadcast::channel(256).0,
            })
            .tx
            .subscribe()
    };
    // The exec lane unconditionally: a run request asserts the page has cells, and the
    // bypass lane owns no executor.
    let _ = app.build_tx.send(BuildMsg::Run {
        key: project.key.clone(),
        rel: rel.clone(),
        scope,
        run_id: run_id.clone(),
        epoch: project.runs.control(&rel).epoch(),
    });

    let body = axum::body::Body::from_stream(crate::runspec::event_stream(rx, Some(rel), run_id));
    (
        [(axum::http::header::CONTENT_TYPE, "application/x-ndjson")],
        body,
    )
        .into_response()
}

/// `POST /__taliesin/interrupt`: stop the run in flight on a page, keeping its kernel.
///
/// Loopback only, matching the run endpoint: stopping someone's computation is no more a
/// visitor's business than starting it. Deliberately **not** gated on `--no-exec` (such a
/// session has nothing running, so "nothing to interrupt" is the honest answer, and a 409
/// would read as a failure to stop something) and deliberately not an error when idle: the
/// author who hits Ctrl-C a second after the last cell finished must see a quiet no-op.
async fn interrupt_handler(
    axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<SocketAddr>,
    State(app): State<Arc<SiteApp>>,
    axum::Json(req): axum::Json<crate::serve::InterruptReq>,
) -> axum::response::Response {
    if !peer.ip().is_loopback() {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "interrupts are accepted from loopback only",
        )
            .into_response();
    }
    let want = std::fs::canonicalize(&req.file).unwrap_or_else(|_| PathBuf::from(&req.file));
    let Some((project, rel)) = page_for_input(&app, &want) else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            format!("{} is not a page of this session's project", req.file),
        )
            .into_response();
    };
    let lang = project.runs.existing(&rel).and_then(|c| c.cancel());
    axum::Json(serde_json::json!({
        "interrupted": lang.is_some(),
        "lang": lang,
    }))
    .into_response()
}

/// The project + page rel owning source file `input`, or `None` when no project serves it.
///
/// Compares canonicalized paths rather than strings so a symlinked or `..`-laden argument
/// still finds its page.
fn page_for_input(app: &Arc<SiteApp>, input: &Path) -> Option<(Arc<Project>, String)> {
    for project in std::iter::once(&app.root).chain(app.mounts.iter().map(|m| &m.project)) {
        let hit = {
            let site = project.site.lock();
            site.pages
                .iter()
                .find(|p| {
                    std::fs::canonicalize(&p.input)
                        .map(|c| c == input)
                        .unwrap_or(false)
                })
                .map(|p| p.rel.clone())
        };
        if let Some(rel) = hit {
            return Some((project.clone(), rel));
        }
    }
    None
}

async fn client_conn(socket: WebSocket, app: Arc<SiteApp>, page_key: String) {
    let (mut sink, mut stream) = socket.split();

    // Route the client's page key (possibly `<mount-prefix>/<rel-or-url>`) to its owning
    // project, then normalise to that project's source rel (the key may be a url).
    let (project, rel) = {
        let (project, sub) = resolve_project(&app, &page_key);
        let project = project.clone();
        let rel = resolve_page_rel(&project, sub);
        (project, rel)
    };

    // A `?page=` the owning project cannot resolve names no page at all, so there is
    // nothing to render, subscribe to, or rebuild — `build_page` already returns
    // immediately on such a key. Allocating a `PageState` for it anyway (a 256-slot
    // broadcast ring that is never evicted) let anyone who can reach this socket grow the
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
        app.queue_build(project.key.clone(), rel.clone());
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
                        let _ = app
                            .build_tx
                            .send(BuildMsg::Restart(project.key.clone(), rel.clone()));
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
        // One ExecPool per project (root + each mount), so a mounted page executes on its
        // OWN _freeze + interpreters. `exec_pool.rs` is used verbatim, once per project.
        // Resolve each project's interpreters from ITS OWN _site.yml/root (python:/r:, a
        // project .venv, env, or default). Boot a single forkserver warm pool for the root's
        // Python and share it only with projects whose interpreter matches (a mismatched
        // mount cold-starts — no forkserver-per-mount). The pools + warm pool are owned by
        // this task and dropped on channel close (server shutdown), which kills every
        // forkserver daemon + idle kernel. If `TALIESIN_PYTHON` is unset or the forkserver
        // can't boot, `warm_pool_for_preview` is inert and every page cold-starts.
        let mut specs: Vec<(
            Arc<Project>,
            crate::interpreter::Resolved,
            crate::interpreter::Resolved,
        )> = Vec::new();
        for project in std::iter::once(&app.root).chain(app.mounts.iter().map(|m| &m.project)) {
            let (py, r) = {
                let s = project.site.lock();
                (
                    crate::interpreter::resolve_python(s.config.python.as_deref(), &project.dir),
                    crate::interpreter::resolve_r(s.config.r.as_deref(), &project.dir),
                )
            };
            specs.push((project.clone(), py, r));
        }
        let root_py = specs[0].1.clone();
        let warm_pool = crate::warm_pool::warm_pool_for_preview(&root_py).await;
        let mut pools: HashMap<ProjectKey, ExecPool> = HashMap::new();
        for (project, py, r) in &specs {
            let wp = if py.path == root_py.path {
                warm_pool.clone()
            } else {
                None
            };
            pools.insert(
                project.key.clone(),
                ExecPool::new(project.dir.join("_freeze"), wp, py.clone(), r.clone()),
            );
        }
        while let Some(msg) = build_rx.recv().await {
            match msg {
                BuildMsg::Build(key, rel) => {
                    let project = app.project(&key).cloned();
                    if let (Some(project), Some(pool)) = (project, pools.get_mut(&key)) {
                        build_page_guarded(&project, &rel, Some(pool), RunRequest::none()).await;
                    }
                }
                BuildMsg::Run {
                    key,
                    rel,
                    scope,
                    run_id,
                    epoch,
                } => {
                    let project = app.project(&key).cloned();
                    if let (Some(project), Some(pool)) = (project, pools.get_mut(&key)) {
                        build_page_guarded(
                            &project,
                            &rel,
                            Some(pool),
                            RunRequest {
                                scope,
                                run_id: Some(run_id.clone()),
                                epoch: Some(epoch),
                            },
                        )
                        .await;
                    } else {
                        // The page or its project vanished between request and dequeue.
                        // The client is waiting on a terminal message for `run_id` and
                        // would otherwise hang until its own timeout, so answer here too:
                        // every path out of a queued run must produce exactly one.
                        if let Some(project) = app.project(&key).cloned()
                            && let Some(ps) = project.pages.lock().get(&rel)
                        {
                            let _ = ps.tx.send(protocol::run_done(
                                Some(&rel),
                                &run_id,
                                "error",
                                Some("no executor for this project"),
                            ));
                        }
                    }
                }
                BuildMsg::Restart(key, rel) => {
                    // Drop + respawn this page's kernel, then rebuild (re-executes every
                    // cell against the fresh kernel).
                    let project = app.project(&key).cloned();
                    if let (Some(project), Some(pool)) = (project, pools.get_mut(&key)) {
                        pool.restart(&rel);
                        build_page_guarded(&project, &rel, Some(pool), RunRequest::none()).await;
                        // A fresh kernel means fresh outputs — including any `ojs_define`
                        // values. Reload the page so the `{js}` cells re-bind to the fresh
                        // `tali-define` blobs from a clean module scope.
                        if let Some(ps) = project.pages.lock().get(&rel) {
                            let _ = ps.tx.send(protocol::reload());
                        }
                    }
                }
            }
        }
    });
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
            // A `Run` never reaches this lane: `run_page` routes it to the exec lane
            // unconditionally, because a run request is a statement that the page HAS
            // cells. Matching it here anyway keeps the enum exhaustive rather than
            // silently dropping a run if that routing ever changes.
            let (key, rel, req) = match msg {
                BuildMsg::Build(key, rel) | BuildMsg::Restart(key, rel) => {
                    (key, rel, RunRequest::none())
                }
                BuildMsg::Run {
                    key,
                    rel,
                    scope,
                    run_id,
                    epoch,
                } => (
                    key,
                    rel,
                    RunRequest {
                        scope,
                        run_id: Some(run_id),
                        epoch: Some(epoch),
                    },
                ),
            };
            let Some(project) = app.project(&key).cloned() else {
                continue;
            };
            if build_page_guarded(&project, &rel, None, req.clone()).await
                == BuildOutcome::NeedsKernel
            {
                let _ = app.build_tx.send(match req.run_id {
                    Some(run_id) => BuildMsg::Run {
                        key,
                        rel,
                        scope: req.scope,
                        run_id,
                        // Preserve the ORIGINAL request epoch across the re-queue: this is
                        // the same run, bounced to the exec lane, and re-reading the epoch
                        // here would launder away a cancel that arrived in between.
                        epoch: req.epoch.unwrap_or_default(),
                    },
                    None => BuildMsg::Build(key, rel),
                });
            }
        }
    });
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
    !blocks.iter().any(|b| {
        b.cell
            .as_ref()
            .is_some_and(|c| taliesin_core::render::executes_to_kernel(&c.lang))
    })
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
    req: RunRequest,
) -> BuildOutcome {
    use futures_util::FutureExt;
    let run_id = req.run_id.clone();
    let outcome = std::panic::AssertUnwindSafe(build_page(project, rel, pool, req))
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
                // A panic is the one path where `build_page` cannot have announced the
                // run itself. Without this the client waits out its whole timeout on a
                // run that is already over, and reports a hang instead of the panic.
                if let Some(run_id) = &run_id {
                    let _ = ps.tx.send(protocol::run_done(
                        Some(rel),
                        run_id,
                        "error",
                        Some(&format!("internal render error: {msg}")),
                    ));
                }
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
    req: RunRequest,
) -> BuildOutcome {
    // Every early return below must still answer a waiting `taliesin run`, so the
    // announcement is a guard that fires on drop rather than a line repeated at each
    // exit. `finish` disarms it once the real outcome is known.
    let mut announce = RunAnnounce::new(project, rel, req.run_id.clone());
    let page = { project.site.lock().page(rel).cloned() };
    let Some(page) = page else {
        announce.finish("error", Some("no such page in this project"));
        return BuildOutcome::Done;
    };
    let Ok(src) = std::fs::read_to_string(&page.input) else {
        let mut pages = project.pages.lock();
        if let Some(ps) = pages.get_mut(rel) {
            ps.doc.errored = true;
            let _ = ps.tx.send(protocol::error(&format!(
                "cannot read {}",
                page.input.display()
            )));
        }
        announce.finish(
            "error",
            Some(&format!("cannot read {}", page.input.display())),
        );
        return BuildOutcome::Done;
    };
    let base = page.input.parent().unwrap_or(Path::new(".")).to_path_buf();
    let (chapter, site_defaults) = {
        let site = project.site.lock();
        (site.chapter_for(&page), site.render_defaults())
    };
    let mut doc =
        taliesin_core::render_document_scoped_with_site(&src, &base, chapter, Some(&site_defaults));

    // Which lane this page actually belongs on, decided from the rendered blocks rather
    // than a guess about the source: exactly the cells the executor would run.
    let cell_free = is_cell_free(&doc.blocks);
    if pool.is_none() && !cell_free {
        // The bypass lane picked this page up (its last build had no cells) and the edit
        // has just added one. Publish nothing — the exec lane redoes this pass with a
        // pool — but record the lane so it is the last time this page comes here.
        if let Some(ps) = project.pages.lock().get_mut(rel) {
            ps.doc.cell_free = false;
        }
        // The exec lane redoes this pass and answers the client there.
        announce.defer();
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
        // Hand this page's executor the same control the interrupt endpoint looks up, so a
        // cancel raised there is a cancel this run reads. Re-set every build because the
        // pool may have evicted and rebuilt the executor since the last one.
        exec.set_run_control(project.runs.control(rel));
        exec
    });
    // Static lints on PRE-EXEC blocks (InSite omits validate_local_links; the site-aware
    // cross-page check below covers those). Collected now, pushed after `diags` is built.
    let static_diags = crate::preview_diag::static_diagnostics(
        &src,
        &doc.blocks,
        &base,
        doc.format,
        crate::check::Scope::InSite,
    );
    // Resolve the run's cap against the RENDERED blocks — the same list the executor is
    // about to walk — so `--cell 3` and the engine cannot disagree about which fence is
    // cell 3, and no second parse of the source is needed.
    let cap = crate::runspec::resolve(req.scope, &doc.blocks);
    if cap == crate::runspec::Resolved::Unresolvable {
        // Refuse rather than widen. Silently promoting "no cell there" into a whole-document
        // run is the worst available answer when a cell takes twenty minutes.
        announce.finish("error", Some("no code cell at that position"));
        return BuildOutcome::Done;
    }
    let until_block = match cap {
        crate::runspec::Resolved::Cap(i) => Some(i),
        _ => None,
    };
    let mut exec = exec;
    if let Some(exec) = exec.as_mut() {
        doc.blocks = exec
            .run_through(std::mem::take(&mut doc.blocks), until_block, req.epoch)
            .await;
    }
    // Finish the executed blocks exactly as the build does (numbering, cross-refs +
    // broken-ref warnings, listing/about expansion, post decoration). Queries the
    // whole site, so it needs the site lock.
    let mut warnings = doc.warnings.clone();
    let (toc, tab_title) = {
        let site = project.site.lock();
        site.finish_blocks(&page, &mut doc.blocks, &mut warnings);
        (
            site.page_toc(&page, doc.toc_explicit, &doc.blocks),
            // Re-resolved every build: an edit can add, change, or remove the front-matter
            // title or the leading `# H1` that names the tab.
            site.page_title(&page, &doc),
        )
    };
    let mut diags = page_diagnostics(&page.input, exec.as_deref());
    diags.extend(static_diags);
    // Cross-page links (this page only) + `_site.yml` config warnings. `validate_cross_page_links`
    // re-renders the whole site (~27 ms), so scope the site lock tightly.
    {
        let site = project.site.lock();
        diags.extend(crate::preview_diag::cross_page_diagnostics(&site, rel));
        diags.extend(crate::preview_diag::site_config_diagnostics(&site));
    }
    for w in &warnings {
        let mut d = Diagnostic::warn(&w.message);
        if let Some(line) = w.line {
            d = d.at(w.file.clone(), line);
        }
        diags.push(d);
    }

    let mut pages = project.pages.lock();
    let ps = pages.entry(rel.to_string()).or_insert_with(|| PageState {
        doc: PageDoc::default(),
        tx: broadcast::channel(256).0,
    });
    let recovered = std::mem::take(&mut ps.doc.errored);
    let ops = diff_blocks(&ps.doc.blocks, &doc.blocks);
    let diags_changed = ps.doc.diagnostics != diags;
    let theme_changed = ps.doc.theme_css != doc.theme_css;
    // Compared BEFORE the assignment below overwrites it. The title is chrome, so it never
    // reaches the tab as a block op: a `title:`-only edit on a page that renders no title
    // block diffs to nothing, and even when it does render one, the body swapped while the
    // tab kept the old name.
    let title_changed = ps.doc.tab_title != tab_title;
    ps.doc.tab_title = tab_title;
    ps.doc.toc = toc;
    ps.doc.theme_css = doc.theme_css;
    ps.doc.theme_default = doc.theme_default;
    ps.doc.includes = doc.includes;
    // Bump the render generation only on a real body change (see serve::rebuild), so a
    // client that server-rendered this page pre-exec re-mounts to pick up the outputs.
    if !ops.is_empty() {
        ps.doc.generation = ps.doc.generation.wrapping_add(1);
    }
    ps.doc.blocks = doc.blocks;
    ps.doc.diagnostics = diags;
    // Broadcast sequencing (body, then theme, then diagnostics — theme/diags after the
    // body even on a recovery re-mount) is the shared contract in `protocol::Broadcast`.
    // A site page never restructures a deck, so `recovered` is the only remount trigger.
    let generation = ps.doc.generation;
    let messages = protocol::Broadcast {
        ops: &ops,
        remount: recovered,
        title_changed,
        theme_changed,
        diags_changed,
    }
    .messages(
        || full_render_json(&ps.doc),
        |op| op_json(op, generation),
        || protocol::title(Some(&ps.doc.tab_title)),
        || protocol::style(&ps.doc.theme_css),
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
    // Announced only after this pass has published everything, so a client that exits the
    // moment it sees `run-done` cannot race ahead of the outputs it was waiting for.
    drop(pages);
    announce.finish("ok", None);
    BuildOutcome::Done
}

/// Per-page diagnostics: a framed front-matter parse error + kernel availability.
///
/// A missing `{{< include >}}` is deliberately *not* checked here. The render pass already
/// emits a located `IncludeWarning` on the directive's own line, which reaches this same
/// channel through `doc.warnings`; checking again produced two diagnostics for one defect,
/// and the extra one had no line to click.
/// `exec` is `None` on the bypass lane (AP3-1), which has no executor — and needs none:
/// the only thing it contributes is the kernel-availability notice, which is about cells
/// this page does not have.
fn page_diagnostics(input: &Path, exec: Option<&crate::exec::Executor>) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    if let Ok(src) = std::fs::read_to_string(input) {
        // Broken front matter: a located, framed error (same as the single-doc server).
        // (Front-matter key warnings now arrive via `doc.warnings` from the render pass.)
        if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(&src) {
            diags.push(
                Diagnostic::error(message)
                    .at(None, line)
                    .with_frame(crate::serve::code_frame(&src, line)),
            );
        }
    }
    if let Some(message) = exec.and_then(|e| e.diagnostic()) {
        diags.push(Diagnostic::warn(message));
    }
    diags
}

// --- file watching ------------------------------------------------------

/// One debounced file-change signal: the path plus whether it is *structural* (a
/// `.tmd` created or removed, which may change the site's page set).
struct Change {
    path: PathBuf,
    structural: bool,
}

fn is_tmd(p: &Path) -> bool {
    // Native `.tmd` source docs, plus `.md` (watched for includes).
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("tmd") | Some("md")
    )
}

fn spawn_watcher(app: Arc<SiteApp>) {
    let (sig_tx, mut sig_rx) = mpsc::unbounded_channel::<Change>();
    // Watch the root site AND every mounted sub-project's dir, so an edit to a mounted
    // page hot-reloads it exactly like a root page.
    let roots: Vec<PathBuf> = std::iter::once(app.root.dir.clone())
        .chain(app.mounts.iter().map(|m| m.project.dir.clone()))
        .collect();

    std::thread::spawn(move || {
        // Pump events through a channel so this thread owns the watcher and can register
        // watches for subdirectories created after startup — the recursive-watch model
        // added an inotify descriptor per directory including `node_modules`/`.git`,
        // which a large project uses to exhaust `max_user_watches` and kill hot reload.
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
        // A non-recursive watch on every directory except the pruned generated/VCS trees,
        // across the root site and every mounted sub-project.
        for base in &roots {
            for dir in crate::serve::watch_tree(base) {
                if let Err(e) = watcher.watch(&dir, notify::RecursiveMode::NonRecursive) {
                    crate::log::warn(&format!("cannot watch {}: {e}", dir.display()));
                }
            }
        }
        for ev in ev_rx {
            if !matches!(
                ev.kind,
                notify::EventKind::Modify(_)
                    | notify::EventKind::Create(_)
                    | notify::EventKind::Remove(_)
            ) {
                continue;
            }
            // A created/removed file may change the page set (vs. an in-place edit, which
            // only rebuilds the page).
            let structural = matches!(
                ev.kind,
                notify::EventKind::Create(_) | notify::EventKind::Remove(_)
            );
            for p in &ev.paths {
                // A newly-created in-tree subdirectory needs its own non-recursive watch.
                if matches!(ev.kind, notify::EventKind::Create(_)) {
                    let is_dir = std::fs::symlink_metadata(p)
                        .map(|m| m.is_dir())
                        .unwrap_or(false);
                    if is_dir
                        && roots.iter().any(|r| p.starts_with(r))
                        && !crate::serve::is_pruned_dir(p)
                    {
                        for d in crate::serve::watch_tree(p) {
                            let _ = watcher.watch(&d, notify::RecursiveMode::NonRecursive);
                        }
                        // Files that already existed inside the new dir were created before
                        // its watch existed, so their events were missed. Replay them as
                        // structural changes (a new `.tmd` may add a page) — a `git checkout`
                        // or a new-folder-with-pages otherwise wouldn't appear until an
                        // unrelated save.
                        for f in crate::serve::subtree_relevant_files(p) {
                            let _ = sig_tx.send(Change {
                                path: f,
                                structural: true,
                            });
                        }
                    }
                }
                // Ignore generated/VCS noise (esp. the executor's own `_freeze/` writes,
                // which would otherwise rebuild every run).
                if crate::serve::relevant_path(p) {
                    let _ = sig_tx.send(Change {
                        path: p.clone(),
                        structural,
                    });
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some(first) = sig_rx.recv().await {
            let mut changed: HashSet<PathBuf> = HashSet::new();
            let mut structural = first.structural && is_tmd(&first.path);
            changed.insert(first.path);
            tokio::time::sleep(Duration::from_millis(80)).await;
            while let Ok(c) = sig_rx.try_recv() {
                structural |= c.structural && is_tmd(&c.path);
                changed.insert(c.path);
            }
            dispatch_changes(&app, &changed, structural);
        }
    });
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
/// to this project by [`dispatch_changes`]): a `_site.yml` change (or a `.tmd`
/// added/removed that changes the page set) re-discovers this project's site and reloads
/// its open tabs; otherwise rebuild every *open* page whose source or include set touches
/// a changed file. `structural` is set when the batch created/removed a `.tmd`.
fn rebuild_project(
    app: &SiteApp,
    project: &Arc<Project>,
    changed: &HashSet<PathBuf>,
    structural: bool,
) {
    let changed_canon: HashSet<PathBuf> = changed
        .iter()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .collect();

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
        *project.site.lock() = new;
        reload_open_tabs(project);
        return;
    }

    // The registry as it stands BEFORE anything below re-derives it — snapshotted here
    // because `structural` re-discovers (replacing the whole `Site`) and that is one of the
    // two ways it moves. Both ways have to be compared against the same "before", or the
    // rebuild selection below silently doesn't apply to one of them.
    let targets_before = project.site.lock().xref_targets.clone();

    // A `.tmd` was created/removed: re-discover, and if the page set actually changed
    // (new/renamed/deleted page, not just an editor's save-via-rename of an existing
    // one) reload open tabs so nav + listings refresh. Otherwise fall through to the
    // normal per-page rebuild against the refreshed site.
    if structural {
        let new = project.rediscover();
        let set_changed = page_rels(&new) != page_rels(&project.site.lock());
        *project.site.lock() = new;
        if set_changed {
            reload_open_tabs(project);
            return;
        }
    }

    // A deck is served by rendering it on request, not from live per-page block state, so
    // it owns no entry in `project.pages` and the dependency scan below — which walks
    // `site.page(rel)` — cannot see it. Without this, editing a deck produced no feedback
    // at all: not a block update, not a reload, nothing.
    //
    // A full reload is the right answer rather than an oversight. The block-level path
    // wants live state per document, and a deck has none here; decks are frozen, so the
    // fix is to make the existing mechanism reach them, not to build a second live path.
    // Every deck in a project already behaved this way — now the standalone one does too.
    let deck_changed = {
        let site = project.site.lock();
        site.decks.iter().any(|d| {
            changed_canon.contains(&d.input.canonicalize().unwrap_or_else(|_| d.input.clone()))
        })
    };
    if deck_changed {
        reload_open_tabs(project);
        return;
    }

    // Rebuild only pages that are open (have live state) and depend on a change.
    let open: Vec<String> = project.pages.lock().keys().cloned().collect();
    let mut to_rebuild: Vec<String> = {
        let site = project.site.lock();
        open.iter()
            .filter(|rel| {
                let Some(page) = site.page(rel) else {
                    return false;
                };
                let mut deps: HashSet<PathBuf> = HashSet::new();
                deps.insert(
                    page.input
                        .canonicalize()
                        .unwrap_or_else(|_| page.input.clone()),
                );
                if let Ok(src) = std::fs::read_to_string(&page.input) {
                    let base = page.input.parent().unwrap_or(Path::new("."));
                    for dep in taliesin_core::includes::dependencies(&src, base) {
                        deps.insert(dep.canonicalize().unwrap_or(dep));
                    }
                    // A page also depends on the resources its front matter names. Without
                    // these, a `.bib`/`.csl`/`.css` edit was a watched event that matched no
                    // page, so the preview kept rendering the stale citation.
                    for dep in taliesin_core::includes::resource_dependencies(&src, base) {
                        deps.insert(dep.canonicalize().unwrap_or(dep));
                    }
                }
                deps.intersection(&changed_canon).next().is_some()
            })
            .cloned()
            .collect()
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
    // `.tmd` only: an anchor can be created or renumbered by a page source or an
    // `{{< include >}}` partial (both `.tmd`), never by a `.bib`/`.css`/image, which the
    // dependency walk above also feeds us. `structural` already re-discovered, which rebuilds
    // the registry, so refreshing again would just burn the pass twice.
    //
    // Under the lock, unlike the per-page render below: this is the whole-site pass and the
    // pages rebuilt after it MUST see the fresh registry. It costs 27ms on the largest real
    // book (`docs/guide`, 20 pages) — a re-scan plus one render per page, no code execution.
    // `refresh_xrefs` is all-or-nothing about a render panic, so a bad page cannot leave the
    // registry un-numbered site-wide; the guard here is belt-and-braces for this task, which
    // (unlike `build_page`) has none of its own.
    //
    // NOT when `structural`: that path re-discovered above, which rebuilds the registry as a
    // side effect, so refreshing again would burn the whole pass twice.
    let touches_source = changed
        .iter()
        .any(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("tmd")));
    if touches_source && !structural {
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
    // Deliberately OUTSIDE the `!structural` gate above, which is the subtler half of that
    // same measurement. The registry moves on BOTH paths — `refresh_xrefs` here, `discover`
    // there — so gating this on the refresh skips the reader-visible half exactly when a
    // `.tmd` is created/removed rather than written in place. Reproduced: delete+recreate
    // (a `git checkout`, or any editor that unlinks before writing) served "Figure 1.2" from
    // `intro.html` while the open `methods.html` tab sat on "Figure 1.1".
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
    // The Cmd-K index is GLOBAL — one `search-index.js` for every tab — so the per-page
    // refresh below, keyed on the open tabs being rebuilt, cannot keep it true: a renumbered
    // figure would go stale in the fragments of every page nobody happens to have open, and
    // Cmd-K would surface a snippet contradicting the page it links to. That is the defect
    // the discovery-time ordering exists to prevent, so it must not come back on the warm
    // path. Only on a real move (a prose edit still refreshes one page's fragment below).
    if moved {
        project.site.lock().rebuild_search_index();
    }
    // Cloned once, after the refresh and before the loop: the fragment render below runs OFF
    // the lock (see the note there) but must resolve against the registry the served pages
    // use, or Cmd-K indexes a bare "Figure" for text the page shows as "Figure 1.1".
    let xref_targets = project.site.lock().xref_targets.clone();
    // Refresh the cross-page Cmd-K index for each rebuilt page, so a re-fetch of
    // `/search-index.js` (the client re-fetches on each palette open in preview) reflects
    // the edit's new headings/prose instead of staying frozen at discovery. Per-page so a
    // big book re-renders one page, not the whole site. The fragment is rendered OFF the
    // site lock and under a panic guard: this runs on the unguarded dispatch task (unlike
    // `build_page`, which has its own guard), and a full render under the lock would stall
    // every other site-lock reader (page serving, `/search-index.js`) on each save.
    for rel in &to_rebuild {
        // Read the chapter under the same brief lock as the page clone: the RENDER stays
        // off-lock (the point of this split), and the index gets the same numbers the page
        // shows ("Theorem 2.1", not "Theorem 1").
        let found = {
            let site = project.site.lock();
            site.page(rel)
                .map(|p| (p.clone(), site.chapter_for(p), site.render_defaults()))
        };
        let Some((page, chapter, site_defaults)) = found else {
            continue;
        };
        let computed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            taliesin_core::site::page_search_fragment(
                &page,
                chapter,
                &xref_targets,
                Some(&site_defaults),
            )
        }));
        // A render panic keeps the last-good fragment (don't wipe the page from search).
        if let Ok(fragment) = computed {
            app.root
                .site
                .lock()
                .install_search_fragment(&page.rel, fragment);
        }
    }
    for rel in to_rebuild {
        app.queue_build(project.key.clone(), rel);
    }
}

/// Fan a batch of changed files out to the projects that own them (root + mounts), then
/// rebuild each affected project independently against its own site/pages/freeze. A file
/// under a mount's dir rebuilds that mount's page; a root file rebuilds the root's.
fn dispatch_changes(app: &SiteApp, changed: &HashSet<PathBuf>, structural: bool) {
    let roots: Vec<(ProjectKey, PathBuf)> =
        std::iter::once((app.root.key.clone(), app.root.dir.clone()))
            .chain(
                app.mounts
                    .iter()
                    .map(|m| (m.project.key.clone(), m.project.dir.clone())),
            )
            .collect();
    // Group changed files by the project whose root is their deepest ancestor.
    let mut by_project: HashMap<ProjectKey, HashSet<PathBuf>> = HashMap::new();
    for p in changed {
        let canon = p.canonicalize().unwrap_or_else(|_| p.clone());
        if let Some((key, _)) = classify_change(&roots, &canon) {
            by_project.entry(key).or_default().insert(p.clone());
        }
    }
    for (key, project_changed) in by_project {
        if let Some(project) = app.project(&key) {
            let project = project.clone();
            rebuild_project(app, &project, &project_changed, structural);
        }
    }
}

/// Reload every open tab and drop its cached block state, so the reload re-renders
/// fresh against the (re-discovered) site — used after a `_site.yml` or page-set
/// change. The reload message is delivered before each channel's sender is dropped.
fn reload_open_tabs(project: &Arc<Project>) {
    let mut pages = project.pages.lock();
    for ps in pages.values() {
        let _ = ps.tx.send(protocol::reload());
    }
    pages.clear();
    crate::log::update(0);
}

/// The site's page identifiers, sorted — to tell whether a `.tmd` add/remove actually
/// changed the page set (vs. an editor save-via-rename of an existing page).
fn page_rels(site: &Site) -> Vec<String> {
    let mut v: Vec<String> = site.pages.iter().map(|p| p.rel.clone()).collect();
    v.sort();
    v
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

        // A non-included block must emit source_file as JSON null (the client's
        // `if (msg.source_file)` is falsy for it and removes the attribute), not omit
        // the key and not emit the string "null".
        let plain = parse(op_json(
            &BlockOp::SetMeta {
                target_id: "b4".into(),
                sourcepos: "3:1-3:5".into(),
                source_file: None,
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

        let dg = parse(protocol::diagnostics(&[Diagnostic::warn("x")]));
        assert_eq!(dg["type"], "diagnostics");
        assert_eq!(dg["messages"][0]["level"], "warning");
        assert_eq!(dg["messages"][0]["message"], "x");

        let err = parse(protocol::error("boom"));
        assert_eq!(err["type"], "error");
        assert_eq!(err["message"], "boom");

        assert_eq!(parse(protocol::reload())["type"], "reload");
    }
}

#[cfg(test)]
mod card_preview {
    //! DX13: the dev-menu card pane renders the current page's OG card on demand via
    //! `/og-preview?page=<rel>`, keyed by page identity (`Site::page`) so it works BEFORE
    //! `_site.yml` sets a `url:` — the exact case the hash-keyed `/og/{name}` route can't
    //! serve (no card hash is ever surfaced without a `url:`). This pins the handler's core
    //! composition against a url-less corpus site; the axum plumbing has no live-HTTP harness.
    use super::*;

    #[test]
    fn card_renders_by_page_identity_without_a_configured_url() {
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
        let site = Site::discover(&corpus.join("demo-book"));
        // The fixture sets no `url:`, so the hash route would surface nothing here — the
        // preview route must still render, which is the whole point of keying by identity.
        assert!(
            site.config.url.is_none(),
            "fixture must have no url: set, or it doesn't exercise the url-less path"
        );
        let page = site.page("intro.tmd").expect("corpus/demo-book/intro.tmd");
        let spec = taliesin_core::site::card_spec(&site, page);
        let png = taliesin_core::site::render_card(&spec);
        // A real PNG (magic bytes) of non-trivial size: the card actually rendered.
        assert!(
            png.starts_with(&[0x89, b'P', b'N', b'G']),
            "og-preview did not return PNG bytes"
        );
        assert!(
            png.len() > 1000,
            "suspiciously small card ({} bytes) — likely a blank/failed render",
            png.len()
        );
    }
}

#[cfg(test)]
mod project_tests {
    //! The routing seam that lets a mounted sub-project be served under its URL prefix.
    //! `match_mount` is the pure core (no `Site`/kernel), so prefix resolution is pinned
    //! here; the live per-page wiring on top is browser-verified (no live-HTTP harness).
    use super::*;

    #[test]
    fn match_mount_picks_the_longest_matching_prefix() {
        let prefixes = vec!["gallery/course".to_string(), "docs/guide".to_string()];
        // Unprefixed → None (the root project serves it).
        assert_eq!(match_mount(&prefixes, "features.html"), None);
        // Exact prefix (the mount landing) → that mount, empty sub-path (caller maps to index).
        assert_eq!(match_mount(&prefixes, "gallery/course"), Some((0, "")));
        // Nested under a prefix → that mount, prefix + leading slash stripped.
        assert_eq!(
            match_mount(&prefixes, "gallery/course/em.html"),
            Some((0, "em.html"))
        );
        // Shares only a leading segment, not the whole prefix → None (root).
        assert_eq!(match_mount(&prefixes, "gallery/other.html"), None);
        // A deeper mount prefix wins over a shorter one that also matches.
        let nested = vec!["gallery".to_string(), "gallery/course".to_string()];
        assert_eq!(
            match_mount(&nested, "gallery/course/em.html"),
            Some((1, "em.html"))
        );
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
            key: ProjectKey(String::new()),
            dir: dir.clone(),
            site: parking_lot::Mutex::new(taliesin_core::site::Site::discover(&dir)),
            pages: parking_lot::Mutex::new(HashMap::new()),
            runs: Default::default(),
            scope: None,
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
    fn classify_change_attributes_a_file_to_its_deepest_project_root() {
        let roots = [
            (
                ProjectKey("gallery/course".into()),
                PathBuf::from("/corpus/course"),
            ),
            (ProjectKey(String::new()), PathBuf::from("/site")),
        ];
        // A file under the mount root → the mount, path relative to that root.
        assert_eq!(
            classify_change(&roots, Path::new("/corpus/course/em.tmd")),
            Some((ProjectKey("gallery/course".into()), PathBuf::from("em.tmd")))
        );
        // A file under the site root → the root project.
        assert_eq!(
            classify_change(&roots, Path::new("/site/features.tmd")),
            Some((ProjectKey(String::new()), PathBuf::from("features.tmd")))
        );
        // A file under neither project root → None.
        assert_eq!(classify_change(&roots, Path::new("/elsewhere/x.tmd")), None);
        // A nested project root wins over an ancestor root (deepest match).
        let nested = [
            (ProjectKey(String::new()), PathBuf::from("/site")),
            (ProjectKey("sub".into()), PathBuf::from("/site/sub")),
        ];
        assert_eq!(
            classify_change(&nested, Path::new("/site/sub/p.tmd")),
            Some((ProjectKey("sub".into()), PathBuf::from("p.tmd")))
        );
    }

    #[test]
    fn only_a_page_with_kernel_cells_takes_the_exec_lane() {
        // AP3-1's routing predicate. One builder task consumed the whole server's queue,
        // root and every mount alike, awaiting each page to completion — so it serialized
        // on the wrong thing: a page with no code cells needs no kernel, yet queued behind
        // kernel work it would never use. Measured on a two-page preview with a warm pool,
        // a cell-free page's prose edit landed in 0.11 s alone and 12.15 s (110x) when an
        // unrelated page was 1.2 s into a 12 s `{python}` cell.
        let render =
            |src: &str| taliesin_core::render_document_with_includes(src, Path::new(".")).blocks;
        assert!(is_cell_free(&render("---\ntitle: T\n---\n\nJust prose.\n")));
        assert!(!is_cell_free(&render(
            "---\ntitle: T\n---\n\n```{python}\nprint(1)\n```\n"
        )));
        assert!(!is_cell_free(&render(
            "---\ntitle: T\n---\n\n```{r}\nprint(1)\n```\n"
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
            key: ProjectKey(String::new()),
            dir,
            site: parking_lot::Mutex::new(site),
            pages: parking_lot::Mutex::new(pages),
            runs: Default::default(),
            scope: None,
        });
        site_page_html(&project, &page)
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

    #[test]
    fn an_unbuilt_page_routes_to_the_safe_lane() {
        // The routing flag is read from the LAST COMPLETED build, so its default decides
        // where a page goes before anything is known about it. `false` (= "not known to be
        // cell-free") must send it to the exec lane: the bypass lane cannot run a cell, and
        // guessing wrong there costs a wasted render, while guessing wrong the other way
        // would publish a page with its outputs missing.
        assert!(!PageDoc::default().cell_free);
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
}

#[cfg(test)]
mod session_key_tests {
    //! The two derivations of a session's key, pinned against each other.
    //!
    //! [`Resolved::session_key`] is what the server publishes; `session::session_key_for`
    //! is what `taliesin run` looks up. They live in different modules and run in different
    //! processes, and when they disagreed the symptom was a run waiting out its full start
    //! timeout on a session that was up and answering. Nothing but this test connects them,
    //! so it drives the real production expressions on both sides rather than restating
    //! either rule.
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
        assert_eq!(
            crate::session::session_key_for(&doc),
            served.session_key(),
            "`taliesin run` would look up a key the server never wrote"
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
        assert_eq!(
            crate::session::session_key_for(&ch),
            served.session_key(),
            "the run client and the server must name the same session"
        );

        // And the project's own front door lands on that same key, so `preview <dir>` and
        // `run <dir>/chapters/ch9.tmd` are one session, not two.
        let whole = resolve_target(Target::at(dir.clone())).unwrap();
        assert_eq!(whole.session_key(), dir);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
