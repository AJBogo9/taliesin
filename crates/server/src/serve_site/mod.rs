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
use taliesin_core::{Block, BlockOp, Page, Site, diff_blocks};
use tokio::sync::{broadcast, mpsc};

use crate::protocol::{self, Diagnostic};
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
    /// (audit finding 01). Signalling is [`crate::kernel::interrupt_pid`] — SIGINT, which
    /// ends the cell and leaves the warm kernel and every prior cell's state alive.
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

/// What the page that lost a cell to `by`'s kernel restart says about it, next to the
/// traceback the interrupt left behind.
fn interrupted_notice(by: &str) -> String {
    format!(
        "a cell here was interrupted so the kernel restart requested on {by} could go \
         through — the dev server runs one page's cells at a time. Edit this page, or \
         restart its kernel, to run it again."
    )
}

/// A job for the executor worker: rebuild a page, or restart its kernel first
/// (the dev-menu "Restart kernel" action) then rebuild.
enum BuildMsg {
    Build(String),
    Restart(String),
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
    /// The page's front-matter `lang:`, empty when it declares none.
    ///
    /// Resolved here by the producer for the same reason `tab_title` is: the live shell is
    /// a second assembly of the page and every value it does not carry, it invents. It
    /// invented this one until 2026-08-17, hardcoding `lang: "en"` with a comment saying
    /// "preview chrome defaults to English" — so a Finnish page previewed as English and
    /// built as Finnish, and the one place an author would notice the difference (a screen
    /// reader, a hyphenation dictionary) is not the preview.
    lang: String,
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
    let same = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()) == file;
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
        }),
        build_tx,
        fast_tx,
        interrupt: Arc::new(std::sync::atomic::AtomicU32::new(0)),
    });

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

    let (listener, addr) = bind_with_fallback(port, &session_key).await?;
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

/// Resolve a request to a page (rendered live) or a static asset under the root.
async fn page_or_asset(
    State(app): State<Arc<SiteApp>>,
    uri: axum::http::Uri,
) -> axum::response::Response {
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
        app.queue_build(rel.clone());
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
    // One shared finishing step (numbering, cross-refs + broken-ref warnings,
    // listing/about expansion, post decoration) so preview matches the build. It owns the
    // `toc` decision too, so the four callers cannot compute it at four different points.
    let mut warnings = std::mem::take(&mut doc.warnings);
    let toc = site.finish_blocks(
        page,
        &mut doc.blocks,
        &mut warnings,
        Some(&src),
        doc.toc_explicit,
    );
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
        lang: doc.lang.clone().unwrap_or_default(),
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
    // all (the arm below has no body at all), and re-composing half the title
    // policy at a second call site is the exact shape of the bug this replaced.
    let (tab_title, toc, lang, body, page_includes, generation) = {
        let pages = project.pages.lock();
        let ps = pages.get(&page.rel);
        match ps {
            Some(ps) => (
                ps.doc.tab_title.clone(),
                ps.doc.toc,
                ps.doc.lang.clone(),
                ps.doc.body_html(),
                ps.doc.includes.clone(),
                ps.doc.generation,
            ),
            None => (
                String::new(),
                false,
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
        js_str(&project.dir.to_string_lossy()),
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
    let body = taliesin_core::site::rewrite_tmd_links(&body);
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
        // The page's own `lang:`, exactly as the build reads it, falling back the same way
        // core's page builder does. Hardcoded to "en" until 2026-08-17.
        lang: if lang.is_empty() { "en" } else { &lang },
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
                        // SIGINT the running cell BEFORE queueing, or the Restart waits
                        // behind the very build it is meant to abort: the builder is
                        // serial and awaits each page to completion, so the queued message
                        // is not read until the runaway cell has already finished (audit
                        // finding 01). A pid of 0 means nothing is executing, and the
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
                        if pid != 0 {
                            crate::kernel::interrupt_pid(pid);
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
        // default). The pool is owned by this task and dropped on channel close (server
        // shutdown), which kills every kernel it holds.
        let project = app.root.clone();
        let py = {
            let s = project.site.lock();
            crate::interpreter::resolve_python(s.config.python.as_deref(), &project.dir)
        };
        let mut pool = ExecPool::new(project.dir.join("_freeze"), py, app.interrupt.clone());
        while let Some(msg) = build_rx.recv().await {
            match msg {
                BuildMsg::Build(rel) => {
                    build_on_exec_lane(&project, &rel, &mut pool).await;
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
            let (BuildMsg::Build(rel) | BuildMsg::Restart(rel)) = msg;
            let project = app.root.clone();
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
/// **Only on a first build.** A warm edit already has a body on screen, and a second full
/// publish there would flash it away and back for nothing.
///
/// The cells go out as source, which is exactly what `--no-exec` already publishes, so the
/// shape is supported end to end. The post-exec publish is untouched, and the diff between
/// the two is what turns each cell's source into its output — `build_page` bumps the render
/// generation on that diff, which is the re-mount the client is already told to expect.
fn publish_pre_exec_body(project: &Arc<Project>, rel: &str, page: &Page, blocks: &[Block]) {
    if project
        .pages
        .lock()
        .get(rel)
        .is_some_and(|ps| !ps.doc.blocks.is_empty())
    {
        return; // a body is already on screen: this is a rebuild, not a first paint
    }
    // Finished exactly as the post-exec publish finishes them (numbering, cross-refs,
    // listing expansion), so this paint is the `--no-exec` render of the page rather than a
    // half-resolved one showing raw `@fig-` text. These warnings are recomputed against the
    // executed blocks below and are discarded here.
    let mut pre = blocks.to_vec();
    let mut discarded = Vec::new();
    {
        let site = project.site.lock();
        site.finish_blocks(page, &mut pre, &mut discarded, None, None);
    }
    let mut pages = project.pages.lock();
    let ps = pages.entry(rel.to_string()).or_insert_with(|| PageState {
        doc: PageDoc::default(),
        tx: broadcast::channel(256).0,
    });
    ps.doc.blocks = pre;
    let _ = ps.tx.send(full_render_json(&ps.doc));
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
    let Ok(src) = std::fs::read_to_string(&page.input) else {
        let mut pages = project.pages.lock();
        if let Some(ps) = pages.get_mut(rel) {
            ps.doc.errored = true;
            let _ = ps.tx.send(protocol::error(&format!(
                "cannot read {}",
                page.input.display()
            )));
        }
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
    // Static lints on PRE-EXEC blocks (InSite omits validate_local_links; the site-aware
    // cross-page check below covers those). Collected now, pushed after `diags` is built.
    let static_diags = crate::preview_diag::static_diagnostics(
        &src,
        &doc.blocks,
        &base,
        crate::lint::Scope::InSite,
    );
    let mut exec = exec;
    if let Some(exec) = exec.as_mut() {
        publish_pre_exec_body(project, rel, &page, &doc.blocks);
        doc.blocks = exec.run(std::mem::take(&mut doc.blocks)).await;
    }
    // Finish the executed blocks exactly as the build does (numbering, cross-refs +
    // broken-ref warnings, listing/about expansion, post decoration). Queries the
    // whole site, so it needs the site lock.
    let mut warnings = doc.warnings.clone();
    let (toc, tab_title) = {
        let site = project.site.lock();
        let toc = site.finish_blocks(
            &page,
            &mut doc.blocks,
            &mut warnings,
            Some(&src),
            doc.toc_explicit,
        );
        (
            toc,
            // Re-resolved every build: an edit can add, change, or remove the front-matter
            // title or the leading `# H1` that names the tab.
            site.page_title(&page, &doc),
        )
    };
    let mut diags = page_diagnostics(&page.input, exec.as_deref());
    // A cell of this page's may have been SIGINTed to let another page's kernel restart
    // through (A17). Read AFTER `exec.run`, which is what the interrupt aborts, so this is
    // the very build that shows the traceback — and the page says where it came from
    // instead of just showing one.
    if let Some(by) = project.exec_lane.lock().take_interrupt_for(rel) {
        diags.push(Diagnostic::warn(interrupted_notice(&by)));
    }
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
    // Compared BEFORE the assignment below overwrites it. The title is chrome, so it never
    // reaches the tab as a block op: a `title:`-only edit on a page that renders no title
    // block diffs to nothing, and even when it does render one, the body swapped while the
    // tab kept the old name.
    let title_changed = ps.doc.tab_title != tab_title;
    ps.doc.tab_title = tab_title;
    ps.doc.toc = toc;
    ps.doc.lang = doc.lang.clone().unwrap_or_default();
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
    // `recovered` is the only remount trigger.
    let generation = ps.doc.generation;
    let messages = protocol::Broadcast {
        ops: &ops,
        remount: recovered,
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
    let roots: Vec<PathBuf> = vec![app.root.dir.clone()];

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
        // A non-recursive watch on every directory except the pruned generated/VCS trees.
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
                // which would otherwise rebuild every run). Judged relative to the project
                // root: these are absolute event paths, and a project living under a
                // directory that happens to be called `_site` is not generated noise.
                if roots.iter().any(|r| crate::serve::relevant_path(p, r)) {
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
    // pages rebuilt after it MUST see the fresh registry. A re-scan plus one render per page,
    // no code execution, so it is O(pages) on every save: 47.6ms on the largest real book
    // (`docs/guide`, 16 pages) re-measured 2026-08-18, and ~12.5ms per page on heavy pages,
    // which extrapolates to ~2.5s at 200 of them. `tools/live-edit-bench` carries the number
    // per project so this comment cannot drift the way its "27ms / 20 pages" predecessor did.
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
    // The Cmd-K index is GLOBAL (one `search-index.js` for every tab), so a per-page
    // refresh keyed on the open tabs cannot keep it true: a renumbered figure would go stale
    // in the fragments of every page nobody happens to have open, and Cmd-K would surface a
    // snippet contradicting the page it links to. The index is rebuilt whole, and only on a
    // real anchor move; a prose edit reaches the palette on the next discovery.
    if moved {
        project.site.lock().rebuild_search_index();
    }
    for rel in to_rebuild {
        app.queue_build(rel);
    }
}

/// Rebuild the project against a batch of changed files.
fn dispatch_changes(app: &SiteApp, changed: &HashSet<PathBuf>, structural: bool) {
    let project = app.root.clone();
    rebuild_project(app, &project, changed, structural);
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
        let render =
            |src: &str| taliesin_core::render_document_with_includes(src, Path::new(".")).blocks;
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
    /// FA16's actual subject. The `lang` test below pins one value the hand-aligned twin
    /// invented; this pins the shell itself — where the navbar, the reading column, the TOC
    /// rail, the prev/next and the footer go. Both paths call `SiteCtx::layout` now, and
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
            let built = {
                let src = std::fs::read_to_string(&page.input).unwrap();
                let doc = taliesin_core::render_document_scoped_with_site(
                    &src,
                    &dir,
                    None,
                    Some(&site.render_defaults()),
                );
                site.render_page_doc_warned(&page, doc).0
            };
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

    /// The preview honours a page's `lang:`, exactly as the build does.
    ///
    /// **The defect (Fable audit FA16).** The live shell is a hand-aligned twin of core's
    /// page assembly, and it passed a literal `lang: "en"` with a comment calling it a
    /// default. So a page declaring `lang: fi` previewed as English and built as Finnish:
    /// a divergence between the two assemblies with nothing structural to stop it, in the
    /// attribute a screen reader picks its voice from. Driven through the real preview
    /// assembler and compared against the real build path, so it pins the AGREEMENT and not
    /// one side's spelling.
    #[test]
    fn a_page_previews_with_the_lang_it_builds_with() {
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
        });
        let preview = site_page_html(&project, &page);
        assert!(
            preview.contains(r#"<html lang="fi""#),
            "the preview must honour the page's lang: {}",
            &preview[..preview.len().min(400)]
        );

        let built = {
            let site = project.site.lock();
            let src = std::fs::read_to_string(&page.input).unwrap();
            let doc = taliesin_core::render_document_scoped_with_site(
                &src,
                &dir,
                None,
                Some(&site.render_defaults()),
            );
            site.render_page_doc_warned(&page, doc).0
        };
        assert!(
            built.contains(r#"<html lang="fi""#),
            "the build side of the comparison must be the one being matched: {}",
            &built[..built.len().min(400)]
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
