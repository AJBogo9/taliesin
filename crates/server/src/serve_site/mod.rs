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
    new_session_token, open_in_browser, percent_decode, print_qr, with_host_guard, with_lan_guard,
    ws_origin_ok,
};

mod exec_pool;
use exec_pool::ExecPool;

struct SiteApp {
    root: PathBuf,
    site: Mutex<Site>,
    pages: Mutex<HashMap<String, PageState>>,
    /// Page rel-paths queued for a (re)build by the executor worker.
    build_tx: mpsc::UnboundedSender<BuildMsg>,
    /// `mounts:` — other taliesin projects (e.g. a docs `book`) served under a URL
    /// prefix, so a site's link to `/docs` resolves in `preview` (not just `build`).
    /// Discovered once; pages render on request (content edits show on refresh).
    mounts: Vec<MountedSite>,
    /// Whether the server is loopback-bound (i.e. not `--host`). Gates whether a
    /// loopback *origin* may open the control-channel ws (see [`origin_allowed`]).
    loopback_bound: bool,
}

/// A mounted sub-project: serve `site` (rooted at `root`) under the `/at/` prefix.
struct MountedSite {
    at: String,
    root: PathBuf,
    site: Site,
}

/// Longest-prefix match of a request `path` against mount `prefixes` (each like
/// `gallery/course`). Returns the winning mount index and `path` with that prefix (and
/// its trailing `/`) removed; `None` when nothing matches, i.e. the request belongs to
/// the root project. Pure — this is the routing seam, unit-tested without any
/// `Site`/kernel. Wired into project resolution when the `Project` struct lands.
#[allow(dead_code)]
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

/// Entry point for `taliesin preview <dir>` when the path is a site project.
pub fn run(root: PathBuf, port: u16, open: bool, expose: bool) -> std::io::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(serve(root, port, open, expose));
    // `serve` returns on a shutdown signal (see `crate::serve::shutdown_signal`);
    // force the runtime down so the builder task that owns the warm pool + kernels is
    // dropped promptly, running its teardown (the forkserver group-kill + kernel
    // SIGKILLs). Bounded so a wedged task can't hang exit; the kills are synchronous.
    rt.shutdown_timeout(std::time::Duration::from_secs(5));
    result
}

async fn serve(root: PathBuf, port: u16, open: bool, expose: bool) -> std::io::Result<()> {
    let start = std::time::Instant::now();
    let root = root.canonicalize().unwrap_or(root);
    // Preview shows drafts inline (nav/listings/prev-next, badged); build/publish exclude
    // them. See `docs/superpowers/specs/2026-07-16-draft-aware-preview-design.md`.
    let site = Site::discover_with(&root, taliesin_core::DraftMode::Include);
    for w in &site.warnings {
        crate::log::warn(w);
    }
    let page_count = site.pages.len();
    // Discover any `mounts:` sub-projects (e.g. a docs book under /docs) once.
    let mounts: Vec<MountedSite> = site
        .config
        .mounts
        .clone()
        .into_iter()
        .filter_map(|m| {
            let mroot = root.join(&m.path);
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
            Some(MountedSite {
                at: m.at,
                root: mroot,
                site: msite,
            })
        })
        .collect();
    // A project with nothing to serve: `check <dir>` already exits 1 here, while `preview`
    // used to bind a port, 404 `/`, and boot the kernel pool for nothing. The two front
    // doors must agree. A page-less root that only `mounts:` sub-projects is legitimate —
    // it is how a docs container is previewed — so it is not empty.
    if page_count == 0 && mounts.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no .tmd pages found under {}", root.display()),
        ));
    }
    let (build_tx, build_rx) = mpsc::unbounded_channel();
    let app = Arc::new(SiteApp {
        root: root.clone(),
        site: Mutex::new(site),
        pages: Mutex::new(HashMap::new()),
        build_tx,
        mounts,
        loopback_bound: !expose,
    });

    spawn_builder(app.clone(), build_rx);
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
        .route("/search-index.js", get(search_index_js))
        .route("/hover-index.js", get(hover_index_js))
        .route("/ws", get(ws_handler))
        .route("/og/{name}", get(og_card))
        .route("/og-preview", get(og_card_preview))
        .fallback(page_or_asset)
        .with_state(app.clone());
    let router = with_lan_guard(router, token.clone());
    let router = with_host_guard(router, lan_ip);

    let (listener, addr) = bind_with_fallback(port, expose).await?;
    let port = addr.port();
    let local = format!("http://127.0.0.1:{port}");
    let network = expose
        .then(local_ip)
        .flatten()
        .map(|ip| lan_url(&format!("http://{ip}:{port}"), token.as_ref()));

    crate::log::clear_screen();
    crate::log::banner(taliesin_core::VERSION);
    crate::log::ready(&local, start.elapsed());
    if let Some(net) = &network {
        crate::log::network(net);
    } else if expose {
        crate::log::warn("--host set, but no LAN address was found");
    }
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
    if open {
        open_in_browser(&local);
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
    tokio::select! {
        r = server => r.map_err(std::io::Error::other),
        _ = crate::serve::shutdown_signal() => {
            crate::log::kernel("shutting down (reaping kernels)");
            Ok(())
        }
    }
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
        let site = app.site.lock();
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
    let rel = q.get("page").cloned().unwrap_or_default();
    let bytes = {
        let site = app.site.lock();
        site.page(&rel).map(|page| {
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
async fn search_index_js(State(app): State<Arc<SiteApp>>) -> impl IntoResponse {
    let json = { app.site.lock().search_index_json.clone() };
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

/// The cross-page hover-preview snippet index as a `hover-index.js` script (assigns
/// `window.TALIESIN_HOVER_INDEX`), lazy-loaded by `12-link-preview.js` on the first
/// cross-page hover. Served as JS (not JSON) so a `<script>` load works under file://.
async fn hover_index_js(State(app): State<Arc<SiteApp>>) -> impl IntoResponse {
    let json = { app.site.lock().hover_index_json.clone() };
    let json = if json.is_empty() {
        "{}".to_string()
    } else {
        json
    };
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/javascript; charset=utf-8",
        )],
        format!("window.TALIESIN_HOVER_INDEX={json};"),
    )
        .into_response()
}

/// Resolve a request to a page (rendered live) or a static asset under the root.
async fn page_or_asset(
    State(app): State<Arc<SiteApp>>,
    uri: axum::http::Uri,
) -> axum::response::Response {
    let path = percent_decode(uri.path().trim_start_matches('/'));
    let lookup = if path.is_empty() {
        "index.html".to_string()
    } else {
        path.clone()
    };
    let page = { app.site.lock().page(&lookup).cloned() };
    if let Some(page) = page {
        return Html(ensure_and_render_page(&app, &page)).into_response();
    }
    // A deck referenced by `{{< embed >}}` (a standalone document, not a page/
    // chapter): render it self-contained on the fly so the embedding iframe resolves
    // in preview, mirroring what `build` writes.
    let deck = { app.site.lock().deck(&lookup).cloned() };
    if let Some(deck) = deck
        && let Ok(src) = std::fs::read_to_string(&deck.input)
    {
        let base = deck.input.parent().unwrap_or(&app.root).to_path_buf();
        let doc = taliesin_core::render_document_with_includes(&src, &base);
        let stem = deck
            .url
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".html"))
            .unwrap_or("deck");
        return Html(taliesin_core::render_doc_to_page(
            &doc,
            stem,
            taliesin_core::OutputMode::Preview,
        ))
        .into_response();
    }
    // A `mounts:` sub-project (e.g. the docs book under /docs): render the requested
    // page from it on the fly (so its links resolve in preview, mirroring the
    // single-tree build), or serve one of its assets.
    for m in &app.mounts {
        let sub = if path == m.at {
            Some("")
        } else {
            match path.strip_prefix(&m.at) {
                Some(r) if r.starts_with('/') => Some(&r[1..]),
                _ => None,
            }
        };
        if let Some(sub) = sub {
            let lookup = if sub.is_empty() { "index.html" } else { sub };
            // The mounted site's search + feed are route-served (not written to disk
            // in preview), exactly like the parent's. Without this, Cmd-K search on a
            // mounted-book page loads `/<mount>/search-index.js` → 404.
            if lookup == "search-index.js" {
                let j = m.site.search_index_json.clone();
                let j = if j.is_empty() { "[]".to_string() } else { j };
                let js_ct = "text/javascript; charset=utf-8";
                let body = format!("window.TALIESIN_SEARCH_INDEX={j};");
                return ([(axum::http::header::CONTENT_TYPE, js_ct)], body).into_response();
            }
            if lookup == "hover-index.js" {
                let j = m.site.hover_index_json.clone();
                let j = if j.is_empty() { "{}".to_string() } else { j };
                let js_ct = "text/javascript; charset=utf-8";
                let body = format!("window.TALIESIN_HOVER_INDEX={j};");
                return ([(axum::http::header::CONTENT_TYPE, js_ct)], body).into_response();
            }
            if let Some(html) = m.site.render_page(lookup) {
                return Html(html).into_response();
            }
            // A deck embedded by a mounted page (e.g. `/docs/guide/tour.html`):
            // render it self-contained on the fly, mirroring the parent's deck
            // branch above. Without this the embedding iframe 404s in preview.
            if let Some(deck) = m.site.deck(lookup)
                && let Ok(src) = std::fs::read_to_string(&deck.input)
            {
                let base = deck.input.parent().unwrap_or(&m.root).to_path_buf();
                let doc = taliesin_core::render_document_with_includes(&src, &base);
                let stem = deck
                    .url
                    .rsplit('/')
                    .next()
                    .and_then(|f| f.strip_suffix(".html"))
                    .unwrap_or("deck");
                return Html(taliesin_core::render_doc_to_page(
                    &doc,
                    stem,
                    taliesin_core::OutputMode::Preview,
                ))
                .into_response();
            }
            return serve_asset(&m.root, lookup);
        }
    }
    // Nothing matched. If it isn't an existing asset either, serve the site's own
    // 404 page (with a 404 status) so preview mirrors the deployed `404.html`.
    let asset = serve_asset(&app.root, &path);
    if asset.status() == axum::http::StatusCode::NOT_FOUND {
        let html = { app.site.lock().render_404_page() };
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
fn ensure_and_render_page(app: &SiteApp, page: &Page) -> String {
    let rel = page.rel.clone();
    if !app.pages.lock().contains_key(&rel) {
        // First-paint render (markdown + listing cards, no code execution yet);
        // done outside the pages lock since it needs the site lock for listings.
        let doc = {
            let site = app.site.lock();
            render_markdown_only(&site, page)
        };
        let (tx, _) = broadcast::channel(256);
        app.pages
            .lock()
            .entry(rel.clone())
            .or_insert(PageState { doc, tx });
        let _ = app.build_tx.send(BuildMsg::Build(rel.clone()));
    }
    site_page_html(app, page)
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
    let mut doc =
        taliesin_core::render_document_with_includes_scoped(&src, base, site.chapter_for(page));
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
    }
}

/// Build the full live HTML for a page: theme + base + site CSS, the SSR body
/// wrapped in the site chrome, and the preview client scoped to this page's ws.
fn site_page_html(app: &SiteApp, page: &Page) -> String {
    // `tab_title` is the string the producer already resolved (`Site::page_title`) — the
    // very one the websocket re-asserts on connect, so the two cannot disagree. It is
    // deliberately NOT re-derived here if empty: that means the page has no live state at
    // all (the arm below has no body and no theme either), and re-composing half the title
    // policy at a second call site is the exact shape of the bug this replaced.
    let (tab_title, toc, theme_css, theme_default, body, page_includes, generation) = {
        let pages = app.pages.lock();
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
    // Live preview: no book archive on disk, so no offline-download link (it would 404).
    let chrome = { app.site.lock().page_chrome(page, false) };
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
    // `root` lets the locator resolve site-root-relative `data-qmd-src` targets
    // (a card → its post's source, the navbar/footer → _site.yml, etc.).
    let doc_global = format!(
        "window.TALIESIN_DOC = {{ path: \"{}\", baseDir: \"{}\", root: \"{}\" }};",
        js_str(&doc_path.to_string_lossy()),
        js_str(&base_dir.to_string_lossy()),
        js_str(&app.root.to_string_lossy()),
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
    let body = taliesin_core::site::rewrite_qmd_links(&body);
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
            // reading content centred in `.tali-book-main`, widened to the content+TOC grid
            // only when the chapter carries a TOC.
            let main_cls = if toc_nav.is_empty() {
                "tali-book-main"
            } else {
                "tali-book-main has-toc"
            };
            let inner_cls = if toc_nav.is_empty() {
                "tali-book-inner"
            } else {
                "tali-book-inner has-toc"
            };
            (
                "tali-book-body",
                format!(
                    "{sidebar}\n<div class=\"{main_cls}\">\n\
                     <div class=\"{inner_cls}\">\n<main id=\"tali-root\">{body}</main>\n{toc_nav}\n</div>\n\
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

    // The live body: the site chrome + the mountable `#tali-root`, plus the
    // dev-menu mount. The websocket client drives everything after first paint.
    let body = format!("{layout}\n<div id=\"tali-controls\"></div>");
    let extra_head = format!("<style>{STATUS_CSS}</style>\n");
    let boot = protocol::boot_id();
    // Draft pages (preview only) power the dev-menu "Drafts" row. Root-absolute urls so a
    // link resolves from any page depth. A build ships neither this global nor the dev menu.
    let drafts_global = {
        let site = app.site.lock();
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
    ws.on_upgrade(move |socket| client_conn(socket, app, rel))
        .into_response()
}

async fn client_conn(socket: WebSocket, app: Arc<SiteApp>, rel_or_url: String) {
    let (mut sink, mut stream) = socket.split();

    // Normalise the client's page key (it may send a url) to the source rel.
    let rel = {
        let site = app.site.lock();
        match site.page(&rel_or_url) {
            Some(p) => p.rel.clone(),
            None => rel_or_url.clone(),
        }
    };

    let (snapshot, mut rx, created) = {
        let mut pages = app.pages.lock();
        let created = !pages.contains_key(&rel);
        let ps = pages.entry(rel.clone()).or_insert_with(|| PageState {
            doc: PageDoc::default(),
            tx: broadcast::channel(256).0,
        });
        (full_render_json(&ps.doc), ps.tx.subscribe(), created)
    };
    if created {
        let _ = app.build_tx.send(BuildMsg::Build(rel.clone()));
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
                        let pages = app.pages.lock();
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
                        let _ = app.build_tx.send(BuildMsg::Restart(rel.clone()));
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
    use taliesin_core::site::rewrite_qmd_links;
    protocol::full_render(
        // The display-ready tab title, NOT the raw front-matter one: the client assigns
        // this straight to `document.title`, over the `<title>` we server-rendered. Null
        // (not "") for a page with no render yet, so the client keeps its own default.
        (!d.tab_title.is_empty()).then_some(d.tab_title.as_str()),
        &rewrite_qmd_links(&d.body_html()),
        d.generation,
        &d.diagnostics,
    )
}

/// Like the single-doc server's `op_json`, but rewrites any author `.tmd` links
/// in the block HTML to their `.html` targets before it goes over the wire.
fn op_json(op: &BlockOp, generation: u64) -> String {
    protocol::op(op, generation, taliesin_core::site::rewrite_qmd_links)
}

// --- build worker -------------------------------------------------------

fn spawn_builder(app: Arc<SiteApp>, mut build_rx: mpsc::UnboundedReceiver<BuildMsg>) {
    tokio::spawn(async move {
        // Boot one process-wide warm pool of Python kernels so the first edit on any
        // page is near-instant. Owned by this builder task: it lives for the server's
        // lifetime and is dropped when the build channel closes (server shutdown),
        // which kills the forkserver daemon + idle kernels. If `TALIESIN_PYTHON` is
        // unset or the forkserver can't boot, `WarmPool::new` returns an inert pool
        // and every page cold-starts — no regression.
        // Resolve the project's interpreters once (from _site.yml python:/r:, a project
        // .venv, env, or default) against the site root, so every page executor and the
        // warm pool agree on which interpreter runs. Read the config under the site lock.
        let (py, r) = {
            let site = app.site.lock();
            (
                crate::interpreter::resolve_python(site.config.python.as_deref(), &app.root),
                crate::interpreter::resolve_r(site.config.r.as_deref(), &app.root),
            )
        };
        let warm_pool = crate::warm_pool::warm_pool_for_preview(&py).await;
        let mut pool = ExecPool::new(app.root.join("_freeze"), warm_pool, py, r);
        while let Some(msg) = build_rx.recv().await {
            match msg {
                BuildMsg::Build(rel) => build_page_guarded(&app, &rel, &mut pool).await,
                BuildMsg::Restart(rel) => {
                    // Drop + respawn this page's kernel, then rebuild (re-executes
                    // every cell against the fresh kernel).
                    pool.restart(&rel);
                    build_page_guarded(&app, &rel, &mut pool).await;
                    // A fresh kernel means fresh outputs — including any `ojs_define`
                    // values. Reload the page so the `{js}` cells re-bind to the
                    // fresh `qmd-define` blobs from a clean module scope.
                    if let Some(ps) = app.pages.lock().get(&rel) {
                        let _ = ps.tx.send(protocol::reload());
                    }
                }
            }
        }
    });
}

/// Run [`build_page`], catching any panic in the render/exec path so one bad
/// page can't kill the shared builder task (which would silently stop hot-reload
/// for *every* page). The panic is logged and surfaced to that page's clients;
/// the next good save recovers.
async fn build_page_guarded(app: &SiteApp, rel: &str, pool: &mut ExecPool) {
    use futures_util::FutureExt;
    let outcome = std::panic::AssertUnwindSafe(build_page(app, rel, pool))
        .catch_unwind()
        .await;
    if let Err(payload) = outcome {
        let msg = crate::serve::panic_msg(&*payload);
        crate::log::error(&format!(
            "render panicked on {rel} (preview kept alive): {msg}"
        ));
        let mut pages = app.pages.lock();
        if let Some(ps) = pages.get_mut(rel) {
            ps.doc.errored = true;
            let _ = ps
                .tx
                .send(protocol::error(&format!("internal render error: {msg}")));
        }
    }
}

/// Re-render a page's markdown, run its code cells (on the page's own executor),
/// then diff against its live blocks and broadcast the changes to its subscribers.
async fn build_page(app: &SiteApp, rel: &str, pool: &mut ExecPool) {
    let page = { app.site.lock().page(rel).cloned() };
    let Some(page) = page else {
        return;
    };
    let Ok(src) = std::fs::read_to_string(&page.input) else {
        let mut pages = app.pages.lock();
        if let Some(ps) = pages.get_mut(rel) {
            ps.doc.errored = true;
            let _ = ps.tx.send(protocol::error(&format!(
                "cannot read {}",
                page.input.display()
            )));
        }
        return;
    };
    let base = page.input.parent().unwrap_or(Path::new(".")).to_path_buf();
    let chapter = app.site.lock().chapter_for(&page);
    let mut doc = taliesin_core::render_document_with_includes_scoped(&src, &base, chapter);

    let exec = pool.get(rel, &base);
    // Stream this page's code-cell execution progress (`build-state`) onto its own
    // broadcast, tagged with the page rel so the client knows which page it's about.
    // The page's `Sender` is created on first visit (before this build is queued), so
    // it's normally present; if it isn't yet, we just don't stream this pass.
    {
        let tx = app.pages.lock().get(rel).map(|ps| ps.tx.clone());
        let sink: crate::exec::ProgressSink = tx.map(|tx| {
            std::sync::Arc::new(move |m: String| {
                let _ = tx.send(m);
            }) as std::sync::Arc<dyn Fn(String) + Send + Sync>
        });
        exec.set_progress(sink, Some(rel.to_string()));
    }
    // Static lints on PRE-EXEC blocks (InSite omits validate_local_links; the site-aware
    // cross-page check below covers those). Collected now, pushed after `diags` is built.
    let static_diags = crate::preview_diag::static_diagnostics(
        &src,
        &doc.blocks,
        &base,
        doc.format,
        crate::check::Scope::InSite,
    );
    doc.blocks = exec.run(std::mem::take(&mut doc.blocks)).await;
    // Finish the executed blocks exactly as the build does (numbering, cross-refs +
    // broken-ref warnings, listing/about expansion, post decoration). Queries the
    // whole site, so it needs the site lock.
    let mut warnings = doc.warnings.clone();
    let (toc, tab_title) = {
        let site = app.site.lock();
        site.finish_blocks(&page, &mut doc.blocks, &mut warnings);
        (
            site.page_toc(&page, doc.toc_explicit, &doc.blocks),
            // Re-resolved every build: an edit can add, change, or remove the front-matter
            // title or the leading `# H1` that names the tab.
            site.page_title(&page, &doc),
        )
    };
    let mut diags = page_diagnostics(&page.input, exec);
    diags.extend(static_diags);
    // Cross-page links (this page only) + `_site.yml` config warnings. `validate_cross_page_links`
    // re-renders the whole site (~27 ms), so scope the site lock tightly.
    {
        let site = app.site.lock();
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

    let mut pages = app.pages.lock();
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
}

/// Per-page diagnostics: a framed front-matter parse error + kernel availability.
///
/// A missing `{{< include >}}` is deliberately *not* checked here. The render pass already
/// emits a located `IncludeWarning` on the directive's own line, which reaches this same
/// channel through `doc.warnings`; checking again produced two diagnostics for one defect,
/// and the extra one had no line to click.
fn page_diagnostics(input: &Path, exec: &crate::exec::Executor) -> Vec<Diagnostic> {
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
    if let Some(message) = exec.diagnostic() {
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

fn is_qmd(p: &Path) -> bool {
    // Native `.tmd` source docs, plus `.md` (watched for includes).
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("tmd") | Some("md")
    )
}

fn spawn_watcher(app: Arc<SiteApp>) {
    let (sig_tx, mut sig_rx) = mpsc::unbounded_channel::<Change>();
    let root = app.root.clone();

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
        for dir in crate::serve::watch_tree(&root) {
            if let Err(e) = watcher.watch(&dir, notify::RecursiveMode::NonRecursive) {
                crate::log::warn(&format!("cannot watch {}: {e}", dir.display()));
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
                    if is_dir && p.starts_with(&root) && !crate::serve::is_pruned_dir(p) {
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
            let mut structural = first.structural && is_qmd(&first.path);
            changed.insert(first.path);
            tokio::time::sleep(Duration::from_millis(80)).await;
            while let Ok(c) = sig_rx.try_recv() {
                structural |= c.structural && is_qmd(&c.path);
                changed.insert(c.path);
            }
            dispatch_changes(&app, &changed, structural);
        }
    });
}

/// Map a batch of changed files to rebuilds: a `_site.yml` change (or a `.tmd`
/// added/removed that changes the page set) re-discovers the site and reloads open
/// tabs; otherwise rebuild every *open* page whose source or include set touches a
/// changed file. `structural` is set when the batch created/removed a `.tmd`.
fn dispatch_changes(app: &SiteApp, changed: &HashSet<PathBuf>, structural: bool) {
    let changed_canon: HashSet<PathBuf> = changed
        .iter()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .collect();

    let config_changed = changed
        .iter()
        .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("_site.yml"));
    if config_changed {
        let new = Site::discover_with(&app.root, taliesin_core::DraftMode::Include);
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
        *app.site.lock() = new;
        reload_open_tabs(app);
        return;
    }

    // The registry as it stands BEFORE anything below re-derives it — snapshotted here
    // because `structural` re-discovers (replacing the whole `Site`) and that is one of the
    // two ways it moves. Both ways have to be compared against the same "before", or the
    // rebuild selection below silently doesn't apply to one of them.
    let xrefs_before = {
        let site = app.site.lock();
        (site.xref_targets.clone(), site.backlinks.clone())
    };

    // A `.tmd` was created/removed: re-discover, and if the page set actually changed
    // (new/renamed/deleted page, not just an editor's save-via-rename of an existing
    // one) reload open tabs so nav + listings refresh. Otherwise fall through to the
    // normal per-page rebuild against the refreshed site.
    if structural {
        let new = Site::discover_with(&app.root, taliesin_core::DraftMode::Include);
        let set_changed = page_rels(&new) != page_rels(&app.site.lock());
        *app.site.lock() = new;
        if set_changed {
            reload_open_tabs(app);
            return;
        }
    }

    // Rebuild only pages that are open (have live state) and depend on a change.
    let open: Vec<String> = app.pages.lock().keys().cloned().collect();
    let mut to_rebuild: Vec<String> = {
        let site = app.site.lock();
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
            app.site.lock().refresh_xrefs();
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
    // Referrers are unioned across BEFORE and AFTER, because `build_backlink_index` drops a
    // marker whose anchor is not a known target: DELETING `{#fig-structure}` unlists its
    // referrer from the new index, so an after-only read would never rebuild the one page
    // whose link just went dead. Only on a real move, and only over pages already OPEN, so
    // this adds a tab's worth of renders, never a site's.
    // The selection is any-cross-page-ref, not this-anchor: a page referencing ANYTHING
    // cross-page is rebuilt when ANY target moves. Rebuilds are idempotent (cells replay
    // from their cumulative hashes, and the diff yields no ops for unchanged blocks), and
    // over open tabs the precision is not worth an anchor-diff.
    let (targets_before, backlinks_before) = xrefs_before;
    let moved = {
        let site = app.site.lock();
        let moved = site.xref_targets != targets_before;
        if moved {
            let referring: HashSet<&str> = site
                .backlinks
                .values()
                .chain(backlinks_before.values())
                .flatten()
                .map(String::as_str)
                .collect();
            for rel in &open {
                if !to_rebuild.contains(rel)
                    && site
                        .page(rel)
                        .is_some_and(|p| referring.contains(p.url.as_str()))
                {
                    to_rebuild.push(rel.clone());
                }
            }
        }
        moved
    };
    // The Cmd-K index is GLOBAL — one `search-index.js` for every tab — so the per-page
    // refresh below, keyed on the open tabs being rebuilt, cannot keep it true: a renumbered
    // figure would go stale in the fragments of every page nobody happens to have open, and
    // Cmd-K would surface a snippet contradicting the page it links to. That is the defect
    // the discovery-time ordering exists to prevent, so it must not come back on the warm
    // path. Only on a real move (a prose edit still refreshes one page's fragment below).
    if moved {
        app.site.lock().rebuild_search_index();
    }
    // Cloned once, after the refresh and before the loop: the fragment render below runs OFF
    // the lock (see the note there) but must resolve against the registry the served pages
    // use, or Cmd-K indexes a bare "Figure" for text the page shows as "Figure 1.1".
    let xref_targets = app.site.lock().xref_targets.clone();
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
            let site = app.site.lock();
            site.page(rel).map(|p| (p.clone(), site.chapter_for(p)))
        };
        let Some((page, chapter)) = found else {
            continue;
        };
        let computed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            taliesin_core::site::page_search_fragment(&page, chapter, &xref_targets)
        }));
        // A render panic keeps the last-good fragment (don't wipe the page from search).
        if let Ok(fragment) = computed {
            app.site.lock().install_search_fragment(&page.rel, fragment);
        }
    }
    for rel in to_rebuild {
        let _ = app.build_tx.send(BuildMsg::Build(rel));
    }
}

/// Reload every open tab and drop its cached block state, so the reload re-renders
/// fresh against the (re-discovered) site — used after a `_site.yml` or page-set
/// change. The reload message is delivered before each channel's sender is dropped.
fn reload_open_tabs(app: &SiteApp) {
    let mut pages = app.pages.lock();
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
        // live-edit-bench measures a real edit as 55 ops, 54 of them set_meta. It was
        // the one op with no shape test, so renaming a key here compiled, passed the
        // whole suite AND `tsc`, and silently degraded Alt-click to "opens at line 1"
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
    fn op_json_rewrites_qmd_links_in_block_html() {
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
            "qmd link not rewritten: {html}"
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
}
