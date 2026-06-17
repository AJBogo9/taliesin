//! The multi-page **site** dev server: a live preview of a whole website.
//!
//! It generalises the single-document [`crate::serve`] server to a project:
//!
//!   - the URL selects which page to render (navigation between pages is just a
//!     full page load, so navbar / prev-next links work with no SPA),
//!   - each page has its own block state, broadcast channel, and code executor,
//!     built lazily on first visit,
//!   - a save rebuilds only the affected page(s) and hot-reloads them in place;
//!     a `_quarto.yml` change re-discovers the site and reloads open tabs.
//!
//! Small HTTP/asset helpers + the embedded client are shared with [`crate::serve`].

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use notify::Watcher;
use qmd_fast_core::{Block, BlockOp, Page, Site, diff_blocks};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

use crate::serve::{
    CLIENT_JS, FAVICON, STATUS_CSS, bind_with_fallback, content_type, js_str, local_ip,
    open_in_browser, percent_decode, print_qr,
};

struct SiteApp {
    root: PathBuf,
    site: Mutex<Site>,
    pages: Mutex<HashMap<String, PageState>>,
    /// Page rel-paths queued for a (re)build by the executor worker.
    build_tx: mpsc::UnboundedSender<String>,
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
    blocks: Vec<Block>,
    diagnostics: Vec<Diag>,
    errored: bool,
}

#[derive(Clone, PartialEq)]
struct Diag {
    level: &'static str,
    message: String,
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
    let (build_tx, build_rx) = mpsc::unbounded_channel();
    let app = Arc::new(SiteApp {
        root: root.clone(),
        site: Mutex::new(site),
        pages: Mutex::new(HashMap::new()),
        build_tx,
    });

    spawn_builder(app.clone(), build_rx);
    spawn_watcher(app.clone());

    let router = Router::new()
        .route("/favicon.ico", get(favicon))
        .route("/ws", get(ws_handler))
        .fallback(page_or_asset)
        .with_state(app.clone());

    let (listener, addr) = bind_with_fallback(port, expose).await?;
    let port = addr.port();
    let local = format!("http://127.0.0.1:{port}");
    let network = expose
        .then(local_ip)
        .flatten()
        .map(|ip| format!("http://{ip}:{port}"));

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
    if open {
        open_in_browser(&local);
    }
    axum::serve(listener, router)
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
    let page = { app.site.lock().unwrap().page(&lookup).cloned() };
    if let Some(page) = page {
        return Html(ensure_and_render_page(&app, &page)).into_response();
    }
    serve_asset(&app.root, &path)
}

/// Serve a file under `root`, with path-traversal protection.
fn serve_asset(root: &Path, rel: &str) -> axum::response::Response {
    use axum::http::{StatusCode, header};
    let not_found = || (StatusCode::NOT_FOUND, "not found").into_response();
    let (Ok(base), Ok(full)) = (root.canonicalize(), root.join(rel).canonicalize()) else {
        return not_found();
    };
    if !full.starts_with(&base) || !full.is_file() {
        return not_found();
    }
    match std::fs::read(&full) {
        Ok(bytes) => ([(header::CONTENT_TYPE, content_type(&full))], bytes).into_response(),
        Err(_) => not_found(),
    }
}

/// Ensure the page has live state (creating it + queuing an execution build on
/// first visit), then render its full live HTML for the first paint.
fn ensure_and_render_page(app: &SiteApp, page: &Page) -> String {
    let rel = page.rel.clone();
    let created = {
        let mut pages = app.pages.lock().unwrap();
        if pages.contains_key(&rel) {
            false
        } else {
            let doc = render_markdown_only(page);
            let (tx, _) = broadcast::channel(256);
            pages.insert(rel.clone(), PageState { doc, tx });
            true
        }
    };
    if created {
        let _ = app.build_tx.send(rel.clone());
    }
    site_page_html(app, page)
}

/// A first-paint render without code execution (the worker fills outputs after).
fn render_markdown_only(page: &Page) -> PageDoc {
    let Ok(src) = std::fs::read_to_string(&page.input) else {
        return PageDoc {
            errored: true,
            ..Default::default()
        };
    };
    let base = page.input.parent().unwrap_or(Path::new("."));
    let doc = qmd_fast_core::render_document_with_includes(&src, base);
    PageDoc {
        title: doc.title,
        toc: doc.toc,
        theme_css: doc.theme_css,
        theme_default: doc.theme_default,
        blocks: doc.blocks,
        diagnostics: Vec::new(),
        errored: false,
    }
}

/// Build the full live HTML for a page: theme + base + site CSS, the SSR body
/// wrapped in the site chrome, and the preview client scoped to this page's ws.
fn site_page_html(app: &SiteApp, page: &Page) -> String {
    let (title, toc, theme_css, theme_default, body, ojs) = {
        let pages = app.pages.lock().unwrap();
        let ps = pages.get(&page.rel);
        match ps {
            Some(ps) => {
                let body = ps.doc.body_html();
                let ojs = body.contains("ojs-module-contents");
                (
                    ps.doc.title.clone(),
                    ps.doc.toc,
                    ps.doc.theme_css.clone(),
                    ps.doc.theme_default.clone(),
                    body,
                    ojs,
                )
            }
            None => (
                None,
                false,
                String::new(),
                String::new(),
                String::new(),
                false,
            ),
        }
    };
    let chrome = { app.site.lock().unwrap().page_chrome(page) };

    let (ojs_head, ojs_init) = if ojs {
        (qmd_fast_core::ojs_head(), qmd_fast_core::ojs_init())
    } else {
        (String::new(), String::new())
    };
    let (main_cls, toc_nav, toc_flag) = if toc {
        (
            "qmd-site-main has-toc",
            "<nav id=\"TOC\"></nav>",
            "window.QMD_TOC = true;",
        )
    } else {
        ("qmd-site-main", "", "")
    };
    let theme = if theme_css.trim().is_empty() {
        String::new()
    } else {
        format!("<style>{theme_css}</style>")
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
    let doc_global = format!(
        "window.QMD_DOC = {{ path: \"{}\", baseDir: \"{}\" }};",
        js_str(&doc_path.to_string_lossy()),
        js_str(&base_dir.to_string_lossy()),
    );
    let ws_path = format!("/ws?page={}", encode_query(&page.rel));
    // Body links (author `.qmd` references) -> `.html`; chrome links already are.
    let body = qmd_fast_core::site::rewrite_qmd_links(&body);
    let title_txt = title.unwrap_or_else(|| page.title.clone().unwrap_or_default());

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{title_txt}</title>
<link rel="icon" type="image/svg+xml" href="/favicon.ico" />
{theme_init}
{styles}
{site_styles}
{code_head}
{ojs_head}
{theme}
<style>{status_css}</style>
</head>
<body class="qmd-site">
{navbar}
<div class="{main_cls}">
<main id="qmd-root">{body}</main>
{toc_nav}
{prevnext}
</div>
{footer}
<div id="qmd-controls"></div>
<script>{doc_global} {toc_flag} window.QMD_SSR = true; window.QMD_WS_PATH = "{ws_path}";</script>
{code_scripts}
<script>
{js}
</script>
{ojs_init}
</body>
</html>
"#,
        theme_init = qmd_fast_core::theme_head(&theme_default),
        styles = qmd_fast_core::client_styles(),
        site_styles = qmd_fast_core::site_styles(),
        code_head = qmd_fast_core::code_head(),
        code_scripts = qmd_fast_core::code_scripts(),
        status_css = STATUS_CSS,
        navbar = chrome.navbar_html,
        prevnext = chrome.prevnext_html,
        footer = chrome.footer_html,
        js = CLIENT_JS,
    )
}

/// Minimal query-value encoding for a page rel in the ws URL (spaces only; `/`
/// and `-` are query-safe).
fn encode_query(s: &str) -> String {
    s.replace(' ', "%20")
}

// --- WebSocket ----------------------------------------------------------

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(q): Query<HashMap<String, String>>,
    State(app): State<Arc<SiteApp>>,
) -> impl IntoResponse {
    let rel = q.get("page").cloned().unwrap_or_default();
    ws.on_upgrade(move |socket| client_conn(socket, app, rel))
}

async fn client_conn(socket: WebSocket, app: Arc<SiteApp>, rel_or_url: String) {
    let (mut sink, mut stream) = socket.split();

    // Normalise the client's page key (it may send a url) to the source rel.
    let rel = {
        let site = app.site.lock().unwrap();
        match site.page(&rel_or_url) {
            Some(p) => p.rel.clone(),
            None => rel_or_url.clone(),
        }
    };

    let (snapshot, mut rx, created) = {
        let mut pages = app.pages.lock().unwrap();
        let created = !pages.contains_key(&rel);
        let ps = pages.entry(rel.clone()).or_insert_with(|| PageState {
            doc: PageDoc::default(),
            tx: broadcast::channel(256).0,
        });
        (full_render_json(&ps.doc), ps.tx.subscribe(), created)
    };
    if created {
        let _ = app.build_tx.send(rel.clone());
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
                        let pages = app.pages.lock().unwrap();
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
                Some(Ok(Message::Text(t))) => handle_client_msg(t.as_str()),
                Some(Ok(Message::Close(_))) | None => break,
                Some(Err(_)) => break,
                _ => {}
            },
        }
    }
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
    serde_json::json!({
        "type": "full_render",
        "title": d.title,
        "body_html": qmd_fast_core::site::rewrite_qmd_links(&d.body_html()),
        "diagnostics": diags_array(&d.diagnostics),
    })
    .to_string()
}

fn diags_array(diags: &[Diag]) -> Vec<serde_json::Value> {
    diags
        .iter()
        .map(|d| serde_json::json!({ "level": d.level, "message": d.message }))
        .collect()
}

fn diagnostics_json(diags: &[Diag]) -> String {
    serde_json::json!({ "type": "diagnostics", "messages": diags_array(diags) }).to_string()
}

fn error_json(message: &str) -> String {
    serde_json::json!({ "type": "error", "message": message }).to_string()
}

fn reload_json() -> String {
    serde_json::json!({ "type": "reload" }).to_string()
}

/// Like `serve::op_json`, but rewrites any author `.qmd` links in the block HTML
/// to their `.html` targets before it goes over the wire.
fn op_json(op: &BlockOp) -> String {
    use qmd_fast_core::site::rewrite_qmd_links;
    match op {
        BlockOp::Update { target_id, html } => serde_json::json!({
            "type": "update", "target_id": target_id, "html": rewrite_qmd_links(html)
        }),
        BlockOp::Insert { after_id, html } => serde_json::json!({
            "type": "insert", "after_id": after_id, "html": rewrite_qmd_links(html)
        }),
        BlockOp::Remove { target_id } => {
            serde_json::json!({"type": "remove", "target_id": target_id})
        }
    }
    .to_string()
}

// --- build worker -------------------------------------------------------

fn spawn_builder(app: Arc<SiteApp>, mut build_rx: mpsc::UnboundedReceiver<String>) {
    tokio::spawn(async move {
        let mut execs: HashMap<String, crate::exec::Executor> = HashMap::new();
        while let Some(rel) = build_rx.recv().await {
            build_page(&app, &rel, &mut execs).await;
        }
    });
}

/// Re-render a page's markdown, run its code cells (on the page's own executor),
/// then diff against its live blocks and broadcast the changes to its subscribers.
async fn build_page(app: &SiteApp, rel: &str, execs: &mut HashMap<String, crate::exec::Executor>) {
    let page = { app.site.lock().unwrap().page(rel).cloned() };
    let Some(page) = page else {
        return;
    };
    let Ok(src) = std::fs::read_to_string(&page.input) else {
        let mut pages = app.pages.lock().unwrap();
        if let Some(ps) = pages.get_mut(rel) {
            ps.doc.errored = true;
            let _ = ps
                .tx
                .send(error_json(&format!("cannot read {}", page.input.display())));
        }
        return;
    };
    let base = page.input.parent().unwrap_or(Path::new(".")).to_path_buf();
    let doc = qmd_fast_core::render_document_with_includes(&src, &base);

    let exec = execs
        .entry(rel.to_string())
        .or_insert_with(crate::exec::Executor::new);
    let blocks = exec.run(doc.blocks).await;
    let diags = page_diagnostics(&page.input, &base, exec);

    let mut pages = app.pages.lock().unwrap();
    let ps = pages.entry(rel.to_string()).or_insert_with(|| PageState {
        doc: PageDoc::default(),
        tx: broadcast::channel(256).0,
    });
    let recovered = std::mem::take(&mut ps.doc.errored);
    let ops = diff_blocks(&ps.doc.blocks, &blocks);
    let diags_changed = ps.doc.diagnostics != diags;
    ps.doc.title = doc.title;
    ps.doc.toc = doc.toc;
    ps.doc.theme_css = doc.theme_css;
    ps.doc.theme_default = doc.theme_default;
    ps.doc.blocks = blocks;
    ps.doc.diagnostics = diags;
    if recovered {
        let _ = ps.tx.send(full_render_json(&ps.doc));
    } else {
        for op in &ops {
            let _ = ps.tx.send(op_json(op));
        }
    }
    if diags_changed {
        let _ = ps.tx.send(diagnostics_json(&ps.doc.diagnostics));
    }
    if !ops.is_empty() {
        crate::log::update(ops.len());
    }
}

/// Per-page diagnostics: unresolved includes + kernel availability.
fn page_diagnostics(input: &Path, base: &Path, exec: &crate::exec::Executor) -> Vec<Diag> {
    let mut diags = Vec::new();
    if let Ok(src) = std::fs::read_to_string(input) {
        for dep in qmd_fast_core::includes::dependencies(&src, base) {
            if !dep.exists() {
                let shown = dep.strip_prefix(base).unwrap_or(&dep);
                diags.push(Diag {
                    level: "warning",
                    message: format!("include not found: {}", shown.display()),
                });
            }
        }
    }
    if let Some(message) = exec.diagnostic() {
        diags.push(Diag {
            level: "warning",
            message,
        });
    }
    diags
}

// --- file watching ------------------------------------------------------

fn spawn_watcher(app: Arc<SiteApp>) {
    let (sig_tx, mut sig_rx) = mpsc::unbounded_channel::<PathBuf>();
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
                    for p in ev.paths {
                        let _ = sig_tx.send(p);
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
            changed.insert(first);
            tokio::time::sleep(Duration::from_millis(80)).await;
            while let Ok(p) = sig_rx.try_recv() {
                changed.insert(p);
            }
            dispatch_changes(&app, &changed);
        }
    });
}

/// Map a batch of changed files to rebuilds: a `_quarto.yml` change re-discovers
/// the site and reloads open tabs; otherwise rebuild every *open* page whose
/// source or include set touches a changed file.
fn dispatch_changes(app: &SiteApp, changed: &HashSet<PathBuf>) {
    let changed_canon: HashSet<PathBuf> = changed
        .iter()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .collect();

    let config_changed = changed
        .iter()
        .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("_quarto.yml"));
    if config_changed {
        let new = Site::discover(&app.root);
        *app.site.lock().unwrap() = new;
        for ps in app.pages.lock().unwrap().values() {
            let _ = ps.tx.send(reload_json());
        }
        crate::log::update(0);
        return;
    }

    // Rebuild only pages that are open (have live state) and depend on a change.
    let open: Vec<String> = app.pages.lock().unwrap().keys().cloned().collect();
    let site = app.site.lock().unwrap();
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
            let _ = app.build_tx.send(rel);
        }
    }
}
