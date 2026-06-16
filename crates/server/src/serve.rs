//! The long-running dev server: serves the preview client over HTTP, pushes
//! the document over a websocket, and watches the source files. On each change
//! it re-renders, diffs against the previous block list, and broadcasts only
//! the changed blocks so the browser updates in place.

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use notify::Watcher;
use qmd_fast_core::{Block, BlockOp, DocFormat, RenderedDoc};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

const CLIENT_JS: &str = include_str!("../../../web-client/client.js");
/// The preview tab's favicon (an original block-model mark; SVG, so it's tiny
/// and self-contained).
const FAVICON: &str = include_str!("../../../web-client/favicon.svg");

struct AppState {
    path: PathBuf,
    base_dir: PathBuf,
    doc: Mutex<DocState>,
    tx: broadcast::Sender<String>,
}

#[derive(Default)]
struct DocState {
    title: Option<String>,
    subtitle: Option<String>,
    format: DocFormat,
    toc: bool,
    theme_css: String,
    theme_default: String,
    blocks: Vec<Block>,
    diagnostics: Vec<Diagnostic>,
}

/// A non-fatal issue with the current document, surfaced in the preview so the
/// author isn't left guessing why something looks wrong.
#[derive(Clone, PartialEq)]
struct Diagnostic {
    level: &'static str, // "warning" | "error"
    message: String,
}

impl DocState {
    /// The mountable body: a reveal deck is assembled into `<section>` slides;
    /// a normal doc is just its blocks concatenated. Either way the individual
    /// blocks keep their ids, so incremental ops apply the same to both.
    fn body_html(&self) -> String {
        match self.format {
            DocFormat::Reveal => qmd_fast_core::slides_html(
                self.title.as_deref(),
                self.subtitle.as_deref(),
                &self.blocks,
            ),
            DocFormat::Html => {
                let mut s = String::new();
                for b in &self.blocks {
                    s.push_str(&b.html);
                    s.push('\n');
                }
                s
            }
        }
    }
}

/// Entry point for `qmd-fast serve <file> [port]`.
pub fn run(path: PathBuf, port: u16) -> std::io::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(serve(path, port))
}

async fn serve(path: PathBuf, port: u16) -> std::io::Result<()> {
    let base_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let (tx, _rx) = broadcast::channel(256);
    let app = Arc::new(AppState {
        path,
        base_dir,
        doc: Mutex::new(DocState::default()),
        tx,
    });

    // Initial render.
    if let Some(doc) = render_doc(&app) {
        let mut d = app.doc.lock().unwrap();
        d.title = doc.title;
        d.subtitle = doc.subtitle;
        d.format = doc.format;
        d.toc = doc.toc;
        d.theme_css = doc.theme_css;
        d.theme_default = doc.theme_default;
        d.blocks = doc.blocks;
    }

    spawn_watcher(app.clone());

    let router = Router::new()
        .route("/", get(index))
        .route("/favicon.ico", get(favicon))
        .route("/ws", get(ws_handler))
        // Anything else is a static asset (images, etc.) resolved relative to the
        // document's directory, so figures display in the live preview.
        .fallback(static_asset)
        .with_state(app.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let desc = {
        let d = app.doc.lock().unwrap();
        let mut parts = vec![match d.format {
            DocFormat::Reveal => "reveal",
            DocFormat::Html => "html",
        }];
        if d.toc {
            parts.push("toc");
        }
        parts.join(", ")
    };
    crate::log::banner(qmd_fast_core::VERSION);
    crate::log::ready(&format!("http://{addr}"));
    crate::log::watching(&app.path.display().to_string(), &desc);
    axum::serve(listener, router)
        .await
        .map_err(std::io::Error::other)
}

fn render_doc(app: &AppState) -> Option<RenderedDoc> {
    let src = std::fs::read_to_string(&app.path).ok()?;
    Some(qmd_fast_core::render_document_with_includes(
        &src,
        &app.base_dir,
    ))
}

// --- HTTP ---------------------------------------------------------------

async fn index(State(app): State<Arc<AppState>>) -> Html<String> {
    let (format, toc, theme_css, theme_default, ojs) = {
        let d = app.doc.lock().unwrap();
        let ojs = d
            .blocks
            .iter()
            .any(|b| b.html.contains("ojs-module-contents"));
        (
            d.format,
            d.toc,
            d.theme_css.clone(),
            d.theme_default.clone(),
            ojs,
        )
    };
    // Absolute doc + base-dir paths so the browser can build `vscode://file/…`
    // links for click-to-source (canonicalized; fall back to the raw paths).
    let doc_path = app.path.canonicalize().unwrap_or_else(|_| app.path.clone());
    let base_dir = app
        .base_dir
        .canonicalize()
        .unwrap_or_else(|_| app.base_dir.clone());
    let ctx = PageCtx {
        format,
        toc,
        theme_css: &theme_css,
        theme_default: &theme_default,
        ojs,
        doc_path: &doc_path.to_string_lossy(),
        base_dir: &base_dir.to_string_lossy(),
    };
    Html(index_html(&ctx))
}

/// Everything the preview index template needs from the current document.
struct PageCtx<'a> {
    format: DocFormat,
    toc: bool,
    theme_css: &'a str,
    theme_default: &'a str,
    ojs: bool,
    doc_path: &'a str,
    base_dir: &'a str,
}

/// The preview favicon (also satisfies the browser's implicit `/favicon.ico`
/// request, so the tab gets an icon and the console stays free of a 404).
async fn favicon() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        FAVICON,
    )
}

/// Serve a static file (image, etc.) resolved relative to the document's
/// directory, with path-traversal protection so only files under `base_dir`
/// are reachable.
async fn static_asset(
    State(app): State<Arc<AppState>>,
    uri: axum::http::Uri,
) -> axum::response::Response {
    use axum::http::{StatusCode, header};
    let rel = percent_decode(uri.path().trim_start_matches('/'));
    let not_found = || (StatusCode::NOT_FOUND, "not found").into_response();
    let (Ok(base), Ok(full)) = (
        app.base_dir.canonicalize(),
        app.base_dir.join(&rel).canonicalize(),
    ) else {
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

/// Guess a content type from a file extension (covers the asset types a doc
/// references; defaults to a generic binary type).
fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("pdf") => "application/pdf",
        Some("mp4") => "video/mp4",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Minimal percent-decoding for request paths (so `%20` etc. in filenames work).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn index_html(ctx: &PageCtx) -> String {
    match ctx.format {
        DocFormat::Reveal => reveal_index_html(),
        DocFormat::Html => blog_index_html(ctx),
    }
}

/// Status pill + the bottom-left control bar (theme + click-to-source toggles).
const STATUS_CSS: &str = "#qmd-controls { position: fixed; bottom: .5rem; left: .5rem; z-index: 9999; \
    display: flex; align-items: center; gap: .4rem; \
    font: 12px ui-sans-serif, system-ui, sans-serif; } \
    #qmd-controls .qmd-ctl { background: var(--qmd-bg, #fff); color: var(--qmd-muted, #888); \
    border: 1px solid var(--qmd-border, #e0e0e0); border-radius: 5px; padding: .15rem .5rem; \
    cursor: pointer; line-height: 1.4; } \
    #qmd-controls .qmd-ctl:hover { color: var(--qmd-fg, #111); } \
    #qmd-controls .qmd-ctl[aria-pressed=\"false\"] { opacity: .55; } \
    #qmd-status { color: var(--qmd-muted, #888); padding: .15rem .35rem; }";

/// Minimal JS-string escape for embedding a filesystem path in a `\"...\"` literal.
fn js_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn blog_index_html(ctx: &PageCtx) -> String {
    // The Observable runtime + init, only when the doc has live `{ojs}` cells.
    // `client.js` calls `window.qmdRunOJS()` after the first full mount.
    let (ojs_head, ojs_init) = if ctx.ojs {
        (qmd_fast_core::ojs_head(), qmd_fast_core::ojs_init())
    } else {
        (String::new(), String::new())
    };
    // With a TOC, lay the content beside a sticky `<nav id="TOC">` (the client
    // rebuilds its entries from the mounted headings, so it stays live). The
    // `QMD_TOC` flag switches the client into that mode.
    let (body_attr, toc_nav, toc_flag) = if ctx.toc {
        (
            " class=\"has-toc\"",
            "<nav id=\"TOC\"></nav>",
            "window.QMD_TOC = true;",
        )
    } else {
        ("", "", "")
    };
    // Custom theme CSS (if any) comes after the base styles so its rules win.
    let theme = if ctx.theme_css.trim().is_empty() {
        String::new()
    } else {
        format!("<style>{}</style>", ctx.theme_css)
    };
    // Absolute paths so click-to-source can build `vscode://file/…` links.
    let doc_global = format!(
        "window.QMD_DOC = {{ path: \"{}\", baseDir: \"{}\" }};",
        js_str(ctx.doc_path),
        js_str(ctx.base_dir),
    );
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>qmd-fast</title>
<link rel="icon" type="image/svg+xml" href="/favicon.ico" />
{theme_init}
{styles}
{code_head}
{ojs_head}
{theme}
<style>{status_css}</style>
</head>
<body{body_attr}>
<main id="qmd-root"></main>
{toc_nav}
<div id="qmd-controls"></div>
<script>{doc_global} {toc_flag}</script>
{code_scripts}
<script>
{js}
</script>
{ojs_init}
</body>
</html>
"#,
        theme_init = qmd_fast_core::theme_head(ctx.theme_default),
        styles = qmd_fast_core::client_styles(),
        code_head = qmd_fast_core::code_head(),
        code_scripts = qmd_fast_core::code_scripts(),
        status_css = STATUS_CSS,
        js = CLIENT_JS,
    )
}

/// Live reveal.js deck: the same preview client, but mounting sectioned slides
/// into `.reveal > .slides` and (re)syncing reveal as blocks change. The
/// `QMD_FORMAT` flag switches the client into deck mode.
fn reveal_index_html() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
<title>qmd-fast</title>
<link rel="icon" type="image/svg+xml" href="/favicon.ico" />
{head}
{code_head}
<style>{status_css}</style>
</head>
<body>
<div class="reveal">
<div class="slides" id="qmd-root"></div>
</div>
<div id="qmd-status">connecting…</div>
{reveal_script}
{code_scripts}
<script>window.QMD_FORMAT = "reveal";</script>
<script>
{js}
</script>
</body>
</html>
"#,
        head = qmd_fast_core::reveal_client_head(),
        code_head = qmd_fast_core::code_head(),
        code_scripts = qmd_fast_core::code_scripts(),
        status_css = STATUS_CSS,
        reveal_script = qmd_fast_core::reveal_client_script(),
        js = CLIENT_JS,
    )
}

// --- WebSocket ----------------------------------------------------------

async fn ws_handler(ws: WebSocketUpgrade, State(app): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| client_conn(socket, app))
}

async fn client_conn(socket: WebSocket, app: Arc<AppState>) {
    let (mut sink, mut stream) = socket.split();

    // Subscribe and snapshot under the same lock the watcher uses, so we never
    // miss or double-apply an op straddling the initial render.
    let (snapshot, mut rx) = {
        let d = app.doc.lock().unwrap();
        let rx = app.tx.subscribe();
        (full_render_json(&d), rx)
    };
    if sink.send(Message::Text(snapshot.into())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            broadcasted = rx.recv() => match broadcasted {
                Ok(text) => {
                    if sink.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                // Fell behind: re-sync with a fresh full render.
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let fr = full_render_json(&app.doc.lock().unwrap());
                    if sink.send(Message::Text(fr.into())).await.is_err() {
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

fn full_render_json(d: &DocState) -> String {
    serde_json::json!({
        "type": "full_render",
        "title": d.title,
        "body_html": d.body_html(),
        "diagnostics": diags_array(&d.diagnostics),
    })
    .to_string()
}

fn diags_array(diags: &[Diagnostic]) -> Vec<serde_json::Value> {
    diags
        .iter()
        .map(|d| serde_json::json!({ "level": d.level, "message": d.message }))
        .collect()
}

fn diagnostics_json(diags: &[Diagnostic]) -> String {
    serde_json::json!({ "type": "diagnostics", "messages": diags_array(diags) }).to_string()
}

fn error_json(message: &str) -> String {
    serde_json::json!({ "type": "error", "message": message }).to_string()
}

/// Non-fatal issues with the current document: includes that don't resolve, and
/// the kernel state. Surfaced in the preview so the author sees them without
/// watching the terminal.
fn compute_diagnostics(app: &AppState, executor: &crate::exec::Executor) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    if let Ok(src) = std::fs::read_to_string(&app.path) {
        for dep in qmd_fast_core::includes::dependencies(&src, &app.base_dir) {
            if !dep.exists() {
                let shown = dep.strip_prefix(&app.base_dir).unwrap_or(&dep);
                diags.push(Diagnostic {
                    level: "warning",
                    message: format!("include not found: {}", shown.display()),
                });
            }
        }
    }
    if let Some(message) = executor.diagnostic() {
        diags.push(Diagnostic {
            level: "warning",
            message,
        });
    }
    diags
}

fn op_json(op: &BlockOp) -> String {
    match op {
        BlockOp::Update { target_id, html } => {
            serde_json::json!({"type": "update", "target_id": target_id, "html": html})
        }
        BlockOp::Insert { after_id, html } => {
            serde_json::json!({"type": "insert", "after_id": after_id, "html": html})
        }
        BlockOp::Remove { target_id } => {
            serde_json::json!({"type": "remove", "target_id": target_id})
        }
    }
    .to_string()
}

// --- file watching ------------------------------------------------------

fn spawn_watcher(app: Arc<AppState>) {
    let (signal_tx, mut signal_rx) = mpsc::unbounded_channel::<()>();
    let dirs = watch_dirs(&app);

    // notify is synchronous; run it on its own thread and forward events.
    std::thread::spawn(move || {
        let mut watcher =
            match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res {
                    if matches!(
                        ev.kind,
                        notify::EventKind::Modify(_)
                            | notify::EventKind::Create(_)
                            | notify::EventKind::Remove(_)
                    ) {
                        let _ = signal_tx.send(());
                    }
                }
            }) {
                Ok(w) => w,
                Err(e) => {
                    crate::log::error(&format!("file watcher unavailable: {e}"));
                    return;
                }
            };
        for dir in &dirs {
            if let Err(e) = watcher.watch(dir, notify::RecursiveMode::Recursive) {
                crate::log::warn(&format!("cannot watch {}: {e}", dir.display()));
            }
        }
        std::thread::park(); // keep the watcher alive
    });

    // Debounce bursts of save events, then re-render, execute, and broadcast a diff.
    tokio::spawn(async move {
        let mut executor = crate::exec::Executor::new();
        // Initial execution pass: markdown is already live; this fills in outputs
        // (and starts the warm kernel) shortly after the page loads.
        rebuild(&app, &mut executor).await;
        while signal_rx.recv().await.is_some() {
            tokio::time::sleep(Duration::from_millis(80)).await;
            while signal_rx.try_recv().is_ok() {}
            rebuild(&app, &mut executor).await;
        }
    });
}

/// Directories to watch: the primary doc's directory plus the directory of any
/// included file (so out-of-tree includes still trigger refreshes).
fn watch_dirs(app: &AppState) -> Vec<PathBuf> {
    let mut dirs: HashSet<PathBuf> = HashSet::new();
    dirs.insert(app.base_dir.clone());
    if let Ok(src) = std::fs::read_to_string(&app.path) {
        for dep in qmd_fast_core::includes::dependencies(&src, &app.base_dir) {
            if let Some(parent) = dep.parent() {
                dirs.insert(parent.to_path_buf());
            }
        }
    }
    dirs.into_iter().collect()
}

/// Re-render markdown, execute code cells (changed + downstream), then diff the
/// assembled block list against the live state and broadcast the changes.
async fn rebuild(app: &AppState, executor: &mut crate::exec::Executor) {
    let Some(doc) = render_doc(app) else {
        let _ = app
            .tx
            .send(error_json(&format!("cannot read {}", app.path.display())));
        return;
    };
    let blocks = executor.run(doc.blocks).await;
    let diags = compute_diagnostics(app, executor);
    let ops = {
        let mut d = app.doc.lock().unwrap();
        let ops = qmd_fast_core::diff_blocks(&d.blocks, &blocks);
        let diags_changed = d.diagnostics != diags;
        d.title = doc.title;
        d.subtitle = doc.subtitle;
        d.format = doc.format;
        d.toc = doc.toc;
        d.theme_css = doc.theme_css;
        d.theme_default = doc.theme_default;
        d.blocks = blocks;
        d.diagnostics = diags;
        // Broadcast under the lock so connecting clients can't interleave.
        for op in &ops {
            let _ = app.tx.send(op_json(op));
        }
        if diags_changed {
            let _ = app.tx.send(diagnostics_json(&d.diagnostics));
        }
        ops.len()
    };
    if ops > 0 {
        crate::log::update(ops);
    }
}
