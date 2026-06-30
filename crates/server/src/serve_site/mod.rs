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
use qmd_fast_core::{Block, BlockOp, Page, Site, diff_blocks};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

use crate::protocol::{self, Diagnostic};
use crate::serve::{
    CLIENT_JS, FAVICON, STATUS_CSS, bind_with_fallback, js_str, lan_url, local_ip,
    new_session_token, open_in_browser, percent_decode, print_qr, with_lan_guard, ws_origin_ok,
};

mod exec_pool;
use exec_pool::ExecPool;

struct SiteApp {
    root: PathBuf,
    site: Mutex<Site>,
    pages: Mutex<HashMap<String, PageState>>,
    /// Page rel-paths queued for a (re)build by the executor worker.
    build_tx: mpsc::UnboundedSender<BuildMsg>,
    /// `mounts:` — other qmd-fast projects (e.g. a docs `book`) served under a URL
    /// prefix, so a site's link to `/docs` resolves in `preview` (not just `build`).
    /// Discovered once; pages render on request (content edits show on refresh).
    mounts: Vec<MountedSite>,
}

/// A mounted sub-project: serve `site` (rooted at `root`) under the `/at/` prefix.
struct MountedSite {
    at: String,
    root: PathBuf,
    site: Site,
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
    title: Option<String>,
    toc: bool,
    theme_css: String,
    theme_default: String,
    /// The page's own front-matter `include-*`/`css` (merged after the site's).
    includes: qmd_fast_core::render::PageIncludes,
    blocks: Vec<Block>,
    diagnostics: Vec<Diagnostic>,
    errored: bool,
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

/// Entry point for `qmd-fast preview <dir>` when the path is a site project.
pub fn run(root: PathBuf, port: u16, open: bool, expose: bool) -> std::io::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(serve(root, port, open, expose))
}

async fn serve(root: PathBuf, port: u16, open: bool, expose: bool) -> std::io::Result<()> {
    let start = std::time::Instant::now();
    let root = root.canonicalize().unwrap_or(root);
    let site = Site::discover(&root);
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
            let msite = Site::discover(&mroot);
            Some(MountedSite {
                at: m.at,
                root: mroot,
                site: msite,
            })
        })
        .collect();
    let (build_tx, build_rx) = mpsc::unbounded_channel();
    let app = Arc::new(SiteApp {
        root: root.clone(),
        site: Mutex::new(site),
        pages: Mutex::new(HashMap::new()),
        build_tx,
        mounts,
    });

    spawn_builder(app.clone(), build_rx);
    spawn_watcher(app.clone());

    // With --host the whole site is LAN-reachable; gate non-loopback access behind a
    // per-session token threaded into the LAN URL/QR (loopback stays token-free).
    let token: Option<Arc<str>> = expose.then(|| Arc::from(new_session_token()));

    let router = Router::new()
        .route("/favicon.ico", get(favicon))
        .route("/search.json", get(search_json))
        .route("/ws", get(ws_handler))
        .fallback(page_or_asset)
        .with_state(app.clone());
    let router = with_lan_guard(router, token.clone());

    let (listener, addr) = bind_with_fallback(port, expose).await?;
    let port = addr.port();
    let local = format!("http://127.0.0.1:{port}");
    let network = expose
        .then(local_ip)
        .flatten()
        .map(|ip| lan_url(&format!("http://{ip}:{port}"), token.as_ref()));

    crate::log::clear_screen();
    crate::log::banner(qmd_fast_core::VERSION);
    crate::log::ready(&local, start.elapsed());
    if let Some(net) = &network {
        crate::log::network(net);
    } else if expose {
        crate::log::warn("--host set, but no LAN address was found");
    }
    crate::log::watching(
        &root.display().to_string(),
        &format!("site, {page_count} pages"),
    );
    if let Some(net) = &network {
        print_qr(net);
    }
    if expose && std::env::var_os("QMD_FAST_NO_EXEC").is_none() {
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
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(std::io::Error::other)
}

// --- HTTP ---------------------------------------------------------------

async fn favicon() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        FAVICON,
    )
}

/// The full-text search index, lazy-loaded by the Cmd-K palette on first open.
async fn search_json(State(app): State<Arc<SiteApp>>) -> impl IntoResponse {
    let json = { app.site.lock().search_index_json.clone() };
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )],
        if json.is_empty() {
            "[]".to_string()
        } else {
            json
        },
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
        let doc = qmd_fast_core::render_document_with_includes(&src, &base);
        let stem = deck
            .url
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".html"))
            .unwrap_or("deck");
        return Html(qmd_fast_core::render_doc_to_page(
            &doc,
            stem,
            qmd_fast_core::OutputMode::Preview,
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
            // mounted-book page fetches `/<mount>/search.json` → 404.
            let json_ct = "application/json; charset=utf-8";
            if lookup == "search.json" {
                let j = m.site.search_index_json.clone();
                let body = if j.is_empty() { "[]".to_string() } else { j };
                return ([(axum::http::header::CONTENT_TYPE, json_ct)], body).into_response();
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
                let doc = qmd_fast_core::render_document_with_includes(&src, &base);
                let stem = deck
                    .url
                    .rsplit('/')
                    .next()
                    .and_then(|f| f.strip_suffix(".html"))
                    .unwrap_or("deck");
                return Html(qmd_fast_core::render_doc_to_page(
                    &doc,
                    stem,
                    qmd_fast_core::OutputMode::Preview,
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
fn render_markdown_only(site: &qmd_fast_core::Site, page: &Page) -> PageDoc {
    let Ok(src) = std::fs::read_to_string(&page.input) else {
        return PageDoc {
            errored: true,
            ..Default::default()
        };
    };
    let base = page.input.parent().unwrap_or(Path::new("."));
    let doc =
        qmd_fast_core::render_document_with_includes_scoped(&src, base, site.chapter_for(page));
    let mut blocks = doc.blocks;
    let toc = site.page_toc(page, doc.toc_explicit, &blocks);
    // One shared finishing step (numbering, cross-refs + broken-ref warnings,
    // listing/about expansion, post decoration) so preview matches the build.
    let mut warnings = doc.warnings;
    site.finish_blocks(page, &mut blocks, &mut warnings);
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
        title: doc.title,
        toc,
        theme_css: doc.theme_css,
        theme_default: doc.theme_default,
        includes: doc.includes,
        blocks,
        diagnostics,
        errored: false,
    }
}

/// Build the full live HTML for a page: theme + base + site CSS, the SSR body
/// wrapped in the site chrome, and the preview client scoped to this page's ws.
fn site_page_html(app: &SiteApp, page: &Page) -> String {
    let (title, toc, theme_css, theme_default, body, page_includes) = {
        let pages = app.pages.lock();
        let ps = pages.get(&page.rel);
        match ps {
            Some(ps) => (
                ps.doc.title.clone(),
                ps.doc.toc,
                ps.doc.theme_css.clone(),
                ps.doc.theme_default.clone(),
                ps.doc.body_html(),
                ps.doc.includes.clone(),
            ),
            None => (
                None,
                false,
                String::new(),
                String::new(),
                String::new(),
                Default::default(),
            ),
        }
    };
    let chrome = { app.site.lock().page_chrome(page) };
    // Site-level `format: html:` includes first, then this page's own front matter.
    let mut includes = chrome.includes.clone();
    includes.merge(&page_includes);

    let (base_cls, toc_nav, toc_flag) = if toc {
        (
            "qmd-site-main has-toc",
            "<nav id=\"TOC\" aria-label=\"Table of contents\"></nav>",
            "window.QMD_TOC = true;",
        )
    } else {
        ("qmd-site-main", "", "")
    };
    let main_cls = if chrome.wide {
        format!("{base_cls} qmd-wide")
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
        "window.QMD_DOC = {{ path: \"{}\", baseDir: \"{}\", root: \"{}\" }};",
        js_str(&doc_path.to_string_lossy()),
        js_str(&base_dir.to_string_lossy()),
        js_str(&app.root.to_string_lossy()),
    );
    let ws_path = format!("/ws?page={}", encode_query(&page.rel));
    // Cross-page Cmd-K search: point the palette at the lazy-loaded `search.json`
    // (depth-relative, served at the root). Empty for a project with no index.
    let search_cfg = if chrome.search_index.is_empty() {
        String::new()
    } else {
        format!("{};", chrome.search_index)
    };
    // Body links (author `.qmd` references) -> `.html`; chrome links already are.
    let body = qmd_fast_core::site::rewrite_qmd_links(&body);
    let title_txt = title.unwrap_or_else(|| page.title.clone().unwrap_or_default());
    // The site's configured favicon (depth-relative); else the dev server's own.
    let favicon = if chrome.favicon.is_empty() {
        "<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.ico\" />".to_string()
    } else {
        qmd_fast_core::favicon_link(&chrome.favicon)
    };

    // A book lays out a sticky topbar + off-canvas chapter drawer over a centred reading
    // column (the live `#qmd-root` + TOC), with prev/next-chapter under it; a website keeps
    // the navbar-on-top layout. (Kept structurally identical to the build path in page.rs.)
    let (body_class, layout) = match chrome.book_sidebar.as_deref() {
        Some(sidebar) => {
            // Keep this layout byte-aligned with the build path (`render/page.rs` book
            // branch): a sticky topbar + off-canvas chapter drawer (`sidebar`), then the
            // reading content centred in `.qmd-book-main`, widened to the content+TOC grid
            // only when the chapter carries a TOC.
            let main_cls = if toc_nav.is_empty() {
                "qmd-book-main"
            } else {
                "qmd-book-main has-toc"
            };
            let inner_cls = if toc_nav.is_empty() {
                "qmd-book-inner"
            } else {
                "qmd-book-inner has-toc"
            };
            (
                "qmd-book-body",
                format!(
                    "{sidebar}\n<div class=\"{main_cls}\">\n\
                     <div class=\"{inner_cls}\">\n<main id=\"qmd-root\">{body}</main>\n{toc_nav}\n</div>\n\
                     {post_nav}</div>\n{footer}",
                    post_nav = chrome.post_nav_html,
                    footer = chrome.footer_html,
                ),
            )
        }
        None => (
            "qmd-site",
            format!(
                "{navbar}\n<div class=\"{main_cls}\">\n<main id=\"qmd-root\">{body}</main>\n\
                 {toc_nav}\n{post_nav}\n</div>\n{footer}",
                navbar = chrome.navbar_html,
                post_nav = chrome.post_nav_html,
                footer = chrome.footer_html,
            ),
        ),
    };

    // The live body: the site chrome + the mountable `#qmd-root`, plus the
    // dev-menu mount. The websocket client drives everything after first paint.
    let body = format!("{layout}\n<div id=\"qmd-controls\"></div>");
    let extra_head = format!("<style>{STATUS_CSS}</style>\n");
    let scripts_pre = format!(
        "<script>{doc_global} {toc_flag} {search_cfg} window.QMD_SSR = true; window.QMD_WS_PATH = \"{ws_path}\";</script>"
    );
    // The cross-page TOC scrollspy + Cmd-K search, then the websocket client.
    let scripts_post = format!(
        "<script>{toc_spy}</script>\n<script>{search_js}</script>\n<script>\n{CLIENT_JS}\n</script>",
        toc_spy = qmd_fast_core::TOC_SPY_JS,
        search_js = qmd_fast_core::SEARCH_JS,
    );
    qmd_fast_core::assemble_html_page(&qmd_fast_core::PageParts {
        // Live preview always ships everything (a doc can gain any construct on an edit).
        mode: qmd_fast_core::OutputMode::Preview,
        title: &title_txt,
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
    if !ws_origin_ok(&headers) {
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
    use qmd_fast_core::site::rewrite_qmd_links;
    protocol::full_render(
        d.title.as_deref(),
        &rewrite_qmd_links(&d.body_html()),
        &d.diagnostics,
    )
}

/// Like the single-doc server's `op_json`, but rewrites any author `.qmd` links
/// in the block HTML to their `.html` targets before it goes over the wire.
fn op_json(op: &BlockOp) -> String {
    protocol::op(op, qmd_fast_core::site::rewrite_qmd_links)
}

// --- build worker -------------------------------------------------------

fn spawn_builder(app: Arc<SiteApp>, mut build_rx: mpsc::UnboundedReceiver<BuildMsg>) {
    tokio::spawn(async move {
        // Boot one process-wide warm pool of Python kernels so the first edit on any
        // page is near-instant. Owned by this builder task: it lives for the server's
        // lifetime and is dropped when the build channel closes (server shutdown),
        // which kills the forkserver daemon + idle kernels. If `QMD_FAST_PYTHON` is
        // unset or the forkserver can't boot, `WarmPool::new` returns an inert pool
        // and every page cold-starts — no regression.
        let warm_pool = crate::warm_pool::warm_pool_for_preview().await;
        let mut pool = ExecPool::new(app.root.join("_freeze"), warm_pool);
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
    let doc = qmd_fast_core::render_document_with_includes_scoped(&src, &base, chapter);

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
    let mut blocks = exec.run(doc.blocks).await;
    // Finish the executed blocks exactly as the build does (numbering, cross-refs +
    // broken-ref warnings, listing/about expansion, post decoration). Queries the
    // whole site, so it needs the site lock.
    let mut warnings = doc.warnings.clone();
    let toc = {
        let site = app.site.lock();
        site.finish_blocks(&page, &mut blocks, &mut warnings);
        site.page_toc(&page, doc.toc_explicit, &blocks)
    };
    let mut diags = page_diagnostics(&page.input, &base, exec);
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
    let ops = diff_blocks(&ps.doc.blocks, &blocks);
    let diags_changed = ps.doc.diagnostics != diags;
    let theme_changed = ps.doc.theme_css != doc.theme_css;
    ps.doc.title = doc.title;
    ps.doc.toc = toc;
    ps.doc.theme_css = doc.theme_css;
    ps.doc.theme_default = doc.theme_default;
    ps.doc.includes = doc.includes;
    ps.doc.blocks = blocks;
    ps.doc.diagnostics = diags;
    if recovered {
        let _ = ps.tx.send(full_render_json(&ps.doc));
    } else {
        for op in &ops {
            let _ = ps.tx.send(op_json(op));
        }
    }
    // A theme/`.css` edit: hot-swap the theme style in place (no reload). Sent AFTER the
    // if/else (not only on the incremental path), so a save that both changes the theme
    // and triggers a full re-mount (error recovery) still applies the new theme — the
    // re-mounted HTML carries the old `<style>` body.
    if theme_changed {
        let _ = ps.tx.send(protocol::style(&ps.doc.theme_css));
    }
    if diags_changed {
        let _ = ps.tx.send(protocol::diagnostics(&ps.doc.diagnostics));
    }
    if !ops.is_empty() {
        crate::log::update(ops.len());
    }
}

/// Per-page diagnostics: unresolved includes + kernel availability.
fn page_diagnostics(input: &Path, base: &Path, exec: &crate::exec::Executor) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    if let Ok(src) = std::fs::read_to_string(input) {
        // Broken front matter: a located, framed error (same as the single-doc server).
        // (Front-matter key warnings now arrive via `doc.warnings` from the render pass.)
        if let Some((message, line)) = qmd_fast_core::frontmatter::yaml_error(&src) {
            diags.push(
                Diagnostic::error(message)
                    .at(None, line)
                    .with_frame(crate::serve::code_frame(&src, line)),
            );
        }
        for dep in qmd_fast_core::includes::dependencies(&src, base) {
            if !dep.exists() {
                let shown = dep.strip_prefix(base).unwrap_or(&dep);
                diags.push(Diagnostic::warn(format!(
                    "include not found: {}",
                    shown.display()
                )));
            }
        }
    }
    if let Some(message) = exec.diagnostic() {
        diags.push(Diagnostic::warn(message));
    }
    diags
}

// --- file watching ------------------------------------------------------

/// One debounced file-change signal: the path plus whether it is *structural* (a
/// `.qmd` created or removed, which may change the site's page set).
struct Change {
    path: PathBuf,
    structural: bool,
}

fn is_qmd(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("qmd") | Some("md")
    )
}

fn spawn_watcher(app: Arc<SiteApp>) {
    let (sig_tx, mut sig_rx) = mpsc::unbounded_channel::<Change>();
    let root = app.root.clone();

    std::thread::spawn(move || {
        let mut watcher =
            match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res
                    && matches!(
                        ev.kind,
                        notify::EventKind::Modify(_)
                            | notify::EventKind::Create(_)
                            | notify::EventKind::Remove(_)
                    )
                {
                    // A created/removed file may change the page set (vs. an in-place
                    // edit, which only rebuilds the page).
                    let structural = matches!(
                        ev.kind,
                        notify::EventKind::Create(_) | notify::EventKind::Remove(_)
                    );
                    for p in ev.paths {
                        // Ignore generated/VCS noise (esp. the executor's own
                        // `_freeze/` writes, which would otherwise rebuild every run).
                        if crate::serve::relevant_path(&p) {
                            let _ = sig_tx.send(Change {
                                path: p,
                                structural,
                            });
                        }
                    }
                }
            }) {
                Ok(w) => w,
                Err(e) => {
                    crate::log::error(&format!("file watcher unavailable: {e}"));
                    return;
                }
            };
        if let Err(e) = watcher.watch(&root, notify::RecursiveMode::Recursive) {
            crate::log::warn(&format!("cannot watch {}: {e}", root.display()));
        }
        std::thread::park();
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

/// Map a batch of changed files to rebuilds: a `_site.yml` change (or a `.qmd`
/// added/removed that changes the page set) re-discovers the site and reloads open
/// tabs; otherwise rebuild every *open* page whose source or include set touches a
/// changed file. `structural` is set when the batch created/removed a `.qmd`.
fn dispatch_changes(app: &SiteApp, changed: &HashSet<PathBuf>, structural: bool) {
    let changed_canon: HashSet<PathBuf> = changed
        .iter()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .collect();

    let config_changed = changed
        .iter()
        .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("_site.yml"));
    if config_changed {
        let new = Site::discover(&app.root);
        // A mid-edit save can leave `_site.yml` transiently malformed; re-discovering then
        // would replace the live site with the degraded default (losing nav/title/output).
        // Keep the last-good `Site` instead, and surface the parse error, so the preview
        // doesn't visibly collapse on every keystroke. The next valid save reloads cleanly.
        if let Some(w) = new
            .warnings
            .iter()
            .find(|w| qmd_fast_core::site::is_malformed_config_warning(w))
        {
            crate::log::warn(&format!("{w}; keeping the last-good _site.yml"));
            return;
        }
        *app.site.lock() = new;
        reload_open_tabs(app);
        return;
    }

    // A `.qmd` was created/removed: re-discover, and if the page set actually changed
    // (new/renamed/deleted page, not just an editor's save-via-rename of an existing
    // one) reload open tabs so nav + listings refresh. Otherwise fall through to the
    // normal per-page rebuild against the refreshed site.
    if structural {
        let new = Site::discover(&app.root);
        let set_changed = page_rels(&new) != page_rels(&app.site.lock());
        *app.site.lock() = new;
        if set_changed {
            reload_open_tabs(app);
            return;
        }
    }

    // Rebuild only pages that are open (have live state) and depend on a change.
    let open: Vec<String> = app.pages.lock().keys().cloned().collect();
    let site = app.site.lock();
    for rel in open {
        let Some(page) = site.page(&rel) else {
            continue;
        };
        let mut deps: HashSet<PathBuf> = HashSet::new();
        deps.insert(
            page.input
                .canonicalize()
                .unwrap_or_else(|_| page.input.clone()),
        );
        if let Ok(src) = std::fs::read_to_string(&page.input) {
            let base = page.input.parent().unwrap_or(Path::new("."));
            for dep in qmd_fast_core::includes::dependencies(&src, base) {
                deps.insert(dep.canonicalize().unwrap_or(dep));
            }
        }
        if deps.intersection(&changed_canon).next().is_some() {
            let _ = app.build_tx.send(BuildMsg::Build(rel));
        }
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

/// The site's page identifiers, sorted — to tell whether a `.qmd` add/remove actually
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
    use qmd_fast_core::{BlockOp, render_document};

    #[test]
    fn op_messages_match_client_contract() {
        let up = parse(op_json(&BlockOp::Update {
            target_id: "b1".into(),
            html: "<p>x</p>".into(),
        }));
        assert_eq!(up["type"], "update");
        assert_eq!(up["target_id"], "b1");
        assert!(up.get("html").is_some());

        let ins = parse(op_json(&BlockOp::Insert {
            after_id: Some("b1".into()),
            html: "<p>y</p>".into(),
        }));
        assert_eq!(ins["type"], "insert");
        assert!(ins.get("after_id").is_some());
        assert!(ins.get("html").is_some());

        let rm = parse(op_json(&BlockOp::Remove {
            target_id: "b2".into(),
        }));
        assert_eq!(rm["type"], "remove");
        assert_eq!(rm["target_id"], "b2");
    }

    #[test]
    fn op_json_rewrites_qmd_links_in_block_html() {
        let up = parse(op_json(&BlockOp::Update {
            target_id: "b1".into(),
            html: "<a href=\"blog.qmd\">b</a>".into(),
        }));
        assert_eq!(up["html"], "<a href=\"blog.html\">b</a>");
    }

    #[test]
    fn a_real_edit_serializes_to_one_update_with_links_rewritten() {
        // The full chain a previewing client receives: render two versions of a
        // page, diff them, and serialize. `tests/incremental.rs` covers render->
        // diff in core; this proves the serve-side serialization (incl. the
        // .qmd->.html rewrite that happens *in* op_json, not at render time).
        let v1 = render_document("Intro.\n\nSee [post](other.qmd).\n");
        let v2 = render_document("Intro.\n\nSee [the post](other.qmd) now.\n");
        let ops = diff_blocks(&v1.blocks, &v2.blocks);
        assert_eq!(ops.len(), 1, "one paragraph edit -> one op: {ops:?}");

        let msg = parse(op_json(&ops[0]));
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
        assert!(!html.contains("other.qmd"), "raw .qmd link leaked: {html}");
    }

    #[test]
    fn lifecycle_messages_match_client_contract() {
        let fr = parse(full_render_json(&PageDoc::default()));
        assert_eq!(fr["type"], "full_render");
        assert!(fr.get("title").is_some()); // present (null allowed)
        assert!(fr.get("body_html").is_some());
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
