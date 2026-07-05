//! The long-running dev server: serves the preview client over HTTP, pushes
//! the document over a websocket, and watches the source files. On each change
//! it re-renders, diffs against the previous block list, and broadcasts only
//! the changed blocks so the browser updates in place.

use crate::protocol::{self, Diagnostic};
use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use notify::Watcher;
use parking_lot::Mutex;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use taliesin_core::{Block, BlockOp, DocFormat, RenderedDoc};
use tokio::sync::{broadcast, mpsc};

pub(crate) const CLIENT_JS: &str = include_str!("../../../../web-client/client.js");
/// The preview tab's favicon (an original block-model mark; SVG, so it's tiny
/// and self-contained).
pub(crate) const FAVICON: &str = include_str!("../../../../web-client/favicon.svg");

mod security;
// Re-exported at `crate::serve::*` because serve_site.rs imports several of these.
pub(crate) use security::{lan_url, new_session_token, with_lan_guard, ws_origin_ok};

struct AppState {
    path: PathBuf,
    base_dir: PathBuf,
    doc: Mutex<DocState>,
    tx: broadcast::Sender<String>,
    /// Set by the dev-menu "Restart kernel" action; the rebuild loop honours it on
    /// its next pass (restart the kernel, then re-render).
    restart_kernel: AtomicBool,
    /// Wakes the rebuild loop (the file watcher and the restart action both kick it).
    kick: mpsc::UnboundedSender<()>,
    /// Whether the server is loopback-bound (i.e. not `--host`). Gates whether a
    /// loopback *origin* may open the control-channel ws (see [`security::origin_allowed`]).
    loopback_bound: bool,
}

#[derive(Default)]
struct DocState {
    title: Option<String>,
    subtitle: Option<String>,
    format: DocFormat,
    toc: bool,
    theme_css: String,
    theme_default: String,
    /// Whether a custom/extension theme owns the colours (decks skip built-in
    /// light/dark management when so).
    theme_is_custom: bool,
    /// The doc's front-matter `include-*`/`css` (and any format-extension theme),
    /// injected into the page head/body so a single-doc preview matches the build.
    includes: taliesin_core::render::PageIncludes,
    /// Non-fatal render warnings (missing `bibliography:`/`theme:` file), surfaced
    /// in the dev menu + terminal.
    warnings: Vec<taliesin_core::render::Warning>,
    blocks: Vec<Block>,
    diagnostics: Vec<Diagnostic>,
    /// True while the last render failed, so the next success can re-mount fully
    /// (clearing the client's error overlay even when the diff is empty).
    errored: bool,
}

impl DocState {
    /// The mountable body: a reveal deck is assembled into `<section>` slides;
    /// a normal doc is just its blocks concatenated. Either way the individual
    /// blocks keep their ids, so incremental ops apply the same to both.
    fn body_html(&self) -> String {
        match self.format {
            DocFormat::Reveal => taliesin_core::slides_html(
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

/// Entry point for `taliesin serve <file> [port] [--open]`.
pub fn run(path: PathBuf, port: u16, open: bool, expose: bool) -> std::io::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(serve(path, port, open, expose))
}

async fn serve(path: PathBuf, port: u16, open: bool, expose: bool) -> std::io::Result<()> {
    let start = std::time::Instant::now();
    let base_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let (tx, _rx) = broadcast::channel(256);
    let (kick, kick_rx) = mpsc::unbounded_channel();
    let app = Arc::new(AppState {
        path,
        base_dir,
        doc: Mutex::new(DocState::default()),
        tx,
        restart_kernel: AtomicBool::new(false),
        kick,
        loopback_bound: !expose,
    });

    // Initial render. Guard it like the rebuild loop (`rebuild_guarded`): a
    // pathological document that panics the renderer must not crash startup before
    // the server can show the error. On a panic, surface it as a diagnostic (the
    // connect snapshot carries it) and mark the doc errored so the first good save
    // recovers with a full re-mount.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render_doc(&app))) {
        Ok(Some(doc)) => {
            let mut d = app.doc.lock();
            d.title = doc.title;
            d.subtitle = doc.subtitle;
            d.format = doc.format;
            d.toc = doc.toc;
            d.theme_css = doc.theme_css;
            d.theme_default = doc.theme_default;
            d.theme_is_custom = doc.theme_is_custom;
            d.includes = doc.includes;
            d.warnings = doc.warnings;
            d.blocks = doc.blocks;
        }
        Ok(None) => {}
        Err(payload) => {
            let msg = panic_msg(&*payload);
            crate::log::error(&format!(
                "render panicked on initial load (preview kept alive): {msg}"
            ));
            let mut d = app.doc.lock();
            d.errored = true;
            d.diagnostics = vec![Diagnostic::error(format!("internal render error: {msg}"))];
        }
    }

    spawn_watcher(app.clone(), kick_rx);

    // With --host the preview is LAN-reachable; gate non-loopback access behind a
    // per-session token threaded into the LAN URL/QR (loopback stays token-free).
    let token: Option<Arc<str>> = expose.then(|| Arc::from(new_session_token()));

    let router = Router::new()
        .route("/", get(index))
        .route("/favicon.ico", get(favicon))
        .route("/ws", get(ws_handler))
        // Anything else is a static asset (images, etc.) resolved relative to the
        // document's directory, so figures display in the live preview.
        .fallback(static_asset)
        .with_state(app.clone());
    let router = with_lan_guard(router, token.clone());

    let (listener, addr) = bind_with_fallback(port, expose).await?;
    let port = addr.port();
    let local = format!("http://127.0.0.1:{port}");
    // With --host we bound 0.0.0.0; surface the LAN URL (and a QR for phones), with the
    // session token in `?t=` so the first load authenticates and sets the cookie.
    let network = expose
        .then(local_ip)
        .flatten()
        .map(|ip| lan_url(&format!("http://{ip}:{port}"), token.as_ref()));
    let desc = {
        let d = app.doc.lock();
        let mut parts = vec![match d.format {
            DocFormat::Reveal => "deck",
            DocFormat::Html => "html",
        }];
        if d.toc {
            parts.push("toc");
        }
        parts.join(", ")
    };
    crate::log::clear_screen();
    crate::log::banner(taliesin_core::VERSION);
    crate::log::ready(&local, start.elapsed());
    if let Some(net) = &network {
        crate::log::network(net);
    } else if expose {
        crate::log::warn("--host set, but no LAN address was found");
    }
    crate::log::watching(&app.path.display().to_string(), &desc);
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

/// Bind `port`, falling back to the next few ports if it's in use (so a second
/// `serve` doesn't just fail). Binds 0.0.0.0 (LAN-reachable) with `expose`, else
/// loopback only. Logs the substitution when it happens.
pub(crate) async fn bind_with_fallback(
    port: u16,
    expose: bool,
) -> std::io::Result<(tokio::net::TcpListener, SocketAddr)> {
    let host = if expose { [0, 0, 0, 0] } else { [127, 0, 0, 1] };
    let mut last_err = None;
    for p in port..=port.saturating_add(9) {
        let addr = SocketAddr::from((host, p));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                if p != port {
                    crate::log::warn(&format!("port {port} in use; using {p}"));
                }
                // Report the *bound* address: with port 0 ("any free port") the OS
                // assigns the real port, so the requested `addr` would still read `:0`.
                let bound = listener.local_addr().unwrap_or(addr);
                return Ok((listener, bound));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => last_err = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("no free port")))
}

/// The machine's primary LAN IP, found by asking the OS which local address it
/// would route an outbound packet from. No packet is sent, so this works offline;
/// returns `None` when there is no route (e.g. no network interface).
pub(crate) fn local_ip() -> Option<std::net::IpAddr> {
    let sock = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    sock.connect(("8.8.8.8", 80)).ok()?;
    sock.local_addr().ok().map(|a| a.ip())
}

/// Print a scannable QR code (terminal half-blocks) for `url`, so the preview can
/// be opened on a phone on the same network without typing the address.
pub(crate) fn print_qr(url: &str) {
    let Ok(code) = qrcode::QrCode::new(url.as_bytes()) else {
        return;
    };
    let art = code
        .render::<qrcode::render::unicode::Dense1x2>()
        .quiet_zone(true)
        .build();
    eprintln!("{art}");
}

/// Open `url` in the default browser (best effort; ignores failure).
pub(crate) fn open_in_browser(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn render_doc(app: &AppState) -> Option<RenderedDoc> {
    let src = std::fs::read_to_string(&app.path).ok()?;
    Some(taliesin_core::render_document_with_includes(
        &src,
        &app.base_dir,
    ))
}

// --- HTTP ---------------------------------------------------------------

async fn index(State(app): State<Arc<AppState>>) -> Html<String> {
    let (format, toc, theme_css, theme_default, theme_is_custom, includes, body) = {
        let d = app.doc.lock();
        (
            d.format,
            d.toc,
            d.theme_css.clone(),
            d.theme_default.clone(),
            d.theme_is_custom,
            d.includes.clone(),
            d.body_html(),
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
        theme_is_custom,
        doc_path: &doc_path.to_string_lossy(),
        base_dir: &base_dir.to_string_lossy(),
        includes: &includes,
        body: &body,
    };
    Html(index_html(&ctx))
}

/// Everything the preview index template needs from the current document.
struct PageCtx<'a> {
    format: DocFormat,
    toc: bool,
    theme_css: &'a str,
    theme_default: &'a str,
    theme_is_custom: bool,
    doc_path: &'a str,
    base_dir: &'a str,
    /// The doc's front-matter `include-*`/`css` + format-extension theme, injected
    /// into the page head/body (so a single-doc deck's theme + plugin appear live).
    includes: &'a taliesin_core::render::PageIncludes,
    /// The rendered body, server-rendered into the page so content shows on the
    /// first paint (the websocket then only drives live updates).
    body: &'a str,
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
    serve_asset_from(
        &app.base_dir,
        &percent_decode(uri.path().trim_start_matches('/')),
    )
}

/// Serve a static file a page references (an image, a stylesheet, …): a plain file
/// under `base`, contained by the canonical root. Shared by the single-doc and site
/// servers.
pub(crate) fn serve_asset_from(base: &Path, rel: &str) -> axum::response::Response {
    use axum::http::{StatusCode, header};
    let not_found = || (StatusCode::NOT_FOUND, "not found").into_response();
    if let (Ok(root), Ok(full)) = (base.canonicalize(), base.join(rel).canonicalize())
        && full.starts_with(&root)
        && full.is_file()
    {
        return match std::fs::read(&full) {
            Ok(bytes) => ([(header::CONTENT_TYPE, content_type(&full))], bytes).into_response(),
            Err(_) => not_found(),
        };
    }
    not_found()
}

/// Guess a content type from a file extension (covers the asset types a doc
/// references; defaults to a generic binary type).
pub(crate) fn content_type(path: &Path) -> &'static str {
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
pub(crate) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn index_html(ctx: &PageCtx) -> String {
    match ctx.format {
        DocFormat::Reveal => deck_index_html(ctx),
        DocFormat::Html => blog_index_html(ctx),
    }
}

/// The floating dev menu (preview-only): a collapsed corner button with a live
/// status dot, expanding to a panel of dev tools (status, word count, click-to-
/// source toggle, diagnostics, and on a single doc a theme toggle). All
/// preview-only — none of this ships in `build`.
pub(crate) const STATUS_CSS: &str = "\
    #tali-controls.tali-dev { position: fixed; bottom: .6rem; left: .6rem; z-index: 9999; \
      font: 12px ui-sans-serif, system-ui, sans-serif; } \
    .tali-dev-toggle { display: inline-flex; align-items: center; gap: .4rem; cursor: pointer; \
      background: var(--tali-bg, #fff); color: var(--tali-muted, #888); \
      border: 1px solid var(--tali-border, #e0e0e0); border-radius: 999px; padding: .25rem .6rem; \
      box-shadow: 0 1px 6px rgba(0,0,0,.12); } \
    .tali-dev-toggle:hover { color: var(--tali-fg, #111); } \
    .tali-dev-toggle.tali-dev-alert { border-color: #d9a23a; color: #d9a23a; } \
    .tali-dev-glyph { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; letter-spacing: -1px; } \
    .tali-dev-count { min-width: 1rem; padding: 0 .3rem; border-radius: 999px; background: #d9a23a; color: #fff; \
      font-weight: 700; font-size: 11px; line-height: 1.3; text-align: center; } \
    .tali-dev-count[hidden] { display: none; } \
    .tali-dev-dot { width: .5rem; height: .5rem; border-radius: 50%; background: var(--tali-muted, #888); flex: none; } \
    .tali-dev-dot[data-state=\"live\"] { background: #3fb950; } \
    .tali-dev-dot[data-state=\"warn\"] { background: #d9a23a; } \
    .tali-dev-dot[data-state=\"error\"] { background: #e5534b; } \
    .tali-dev-panel { position: absolute; bottom: calc(100% + .45rem); left: 0; min-width: 13rem; \
      display: flex; flex-direction: column; gap: .5rem; padding: .65rem; \
      background: var(--tali-bg, #fff); color: var(--tali-fg, #111); \
      border: 1px solid var(--tali-border, #e0e0e0); border-radius: 9px; box-shadow: 0 8px 28px rgba(0,0,0,.2); } \
    .tali-dev-panel[hidden] { display: none; } \
    .tali-dev-row { display: flex; justify-content: space-between; gap: 1rem; color: var(--tali-muted, #888); } \
    .tali-dev-row .tali-dev-label { font-weight: 600; } \
    #tali-wordcount { font-variant-numeric: tabular-nums; } \
    .tali-dev-ctl { display: inline-flex; align-items: center; gap: .4rem; text-align: left; cursor: pointer; \
      background: var(--tali-code-bg, #f5f5f5); color: var(--tali-fg, #111); \
      border: 1px solid var(--tali-border, #e0e0e0); border-radius: 6px; padding: .3rem .55rem; } \
    .tali-dev-ctl:hover { border-color: var(--tali-accent, #4c8dff); } \
    .tali-dev-theme svg { width: 14px; height: 14px; } \
    #tali-diagnostics { display: none; flex-direction: column; gap: .3rem; max-width: 22rem; } \
    #tali-diagnostics .tali-diag { padding: .3rem .5rem; border-radius: 6px; background: var(--tali-code-bg, #f5f5f5); \
      border: 1px solid var(--tali-border, #e0e0e0); line-height: 1.35; } \
    #tali-diagnostics .tali-diag-error { border-left: 3px solid #e5534b; } \
    #tali-diagnostics .tali-diag-warning { border-left: 3px solid #d9a23a; } \
    #tali-diagnostics .tali-diag-loc { cursor: pointer; text-align: left; width: 100%; font: inherit; color: inherit; } \
    #tali-diagnostics .tali-diag-loc:hover { border-color: var(--tali-accent, #4c8dff); } \
    #tali-diagnostics .tali-diag-loc::after { content: \"  \\2192 source\"; color: var(--tali-muted, #888); font-size: 11px; } \
    #tali-diagnostics .tali-diag-frame { margin: .35rem 0 0; padding: .35rem .45rem; border-radius: 4px; overflow-x: auto; \
      background: var(--tali-bg, #fff); white-space: pre; font: 11px/1.45 ui-monospace, SFMono-Regular, Menlo, monospace; } \
    #tali-cell-errors { flex-direction: column; gap: .3rem; max-width: 22rem; } \
    .tali-cellerr { text-align: left; cursor: pointer; font: 12px ui-sans-serif, system-ui, sans-serif; \
      color: var(--tali-fg, #111); background: var(--tali-code-bg, #f5f5f5); border: 1px solid var(--tali-border, #e0e0e0); \
      border-left: 3px solid #e5534b; border-radius: 6px; padding: .3rem .5rem; \
      white-space: nowrap; overflow: hidden; text-overflow: ellipsis; } \
    .tali-cellerr:hover { border-color: #e5534b; } \
    @media (max-width: 60rem) { body.tali-toc-sheet #tali-controls.tali-dev { bottom: 2.4rem; } } \
    #tali-progress { position: fixed; bottom: 12px; right: 12px; z-index: 9999; \
      display: flex; align-items: center; gap: 6px; \
      font: 12px/1.4 ui-sans-serif, system-ui, sans-serif; padding: 5px 10px; border-radius: 6px; \
      background: var(--tali-bg, #fff); color: var(--tali-fg, #222); \
      border: 1px solid color-mix(in srgb, currentColor 20%, transparent); \
      box-shadow: 0 1px 6px rgba(0,0,0,.10); cursor: default; user-select: none; } \
    #tali-progress[data-state=\"busy\"] { cursor: pointer; } \
    #tali-progress[data-state=\"warming\"] { border-color: color-mix(in srgb, #d9a23a 55%, transparent); } \
    #tali-progress[data-state=\"error\"] { cursor: pointer; border-color: #e5534b; } \
    #tali-progress[data-state=\"idle\"] { opacity: .65; } \
    .tali-prog-dot { width: .5rem; height: .5rem; border-radius: 50%; flex: none; \
      background: var(--tali-muted, #aaa); } \
    #tali-progress[data-state=\"busy\"] .tali-prog-dot { background: #4c8dff; } \
    #tali-progress[data-state=\"warming\"] .tali-prog-dot { background: #d9a23a; } \
    #tali-progress[data-state=\"idle\"] .tali-prog-dot { background: #3fb950; } \
    #tali-progress[data-state=\"error\"] .tali-prog-dot { background: #e5534b; } \
    @media (prefers-reduced-motion: no-preference) { \
      #tali-progress[data-state=\"busy\"] .tali-prog-dot, \
      #tali-progress[data-state=\"warming\"] .tali-prog-dot { \
        animation: tali-dot-pulse 1.2s ease-in-out infinite; } \
      @keyframes tali-dot-pulse { 0%,100% { opacity:1; } 50% { opacity:.3; } } \
    } \
    .tali-prog-label { white-space: nowrap; } \
    .tali-prog-bar { display: inline-block; width: 48px; height: 4px; border-radius: 2px; \
      background: color-mix(in srgb, currentColor 15%, transparent); flex: none; } \
    .tali-prog-fill { display: block; height: 100%; border-radius: 2px; \
      background: #4c8dff; transition: width .15s linear; } \
    [data-qmd-cell-state] { border-left: 3px solid transparent; padding-left: 8px; } \
    [data-qmd-cell-state=\"queued\"] { border-left-color: color-mix(in srgb, currentColor 30%, transparent); opacity: .7; } \
    [data-qmd-cell-state=\"running\"] { border-left-color: #4c8dff; } \
    [data-qmd-cell-state=\"done\"] { border-left-color: #2bb673; } \
    [data-qmd-cell-state=\"error\"] { border-left-color: #cc3333; } \
    .tali-cell-badge { font: 11px/1 var(--tali-mono, monospace); opacity: .75; margin-right: 6px; } \
    @media (prefers-reduced-motion: no-preference) { \
      [data-qmd-cell-state=\"running\"] .tali-cell-badge { animation: tali-pulse 1s ease-in-out infinite; } \
      @keyframes tali-pulse { 50% { opacity: .35; } } \
    }";

/// Minimal JS-string escape for embedding a filesystem path in a `\"...\"` literal.
pub(crate) fn js_str(s: &str) -> String {
    // Escape for a double-quoted JS string literal embedded in an inline <script>:
    // also neutralize `</script>` (escape `<`), newlines, and the U+2028/U+2029
    // separators that are illegal raw in a JS string. A path with these is unusual
    // but shouldn't be able to break out of the literal or the script tag.
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '<' => out.push_str("\\x3c"), // splits a literal `</script>`
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out
}

fn blog_index_html(ctx: &PageCtx) -> String {
    // With a TOC, lay the content beside a sticky `<nav id="TOC">` (the client
    // rebuilds its entries from the mounted headings, so it stays live). The
    // `TALIESIN_TOC` flag switches the client into that mode. `tali-toc-sheet` opts the
    // live page into the mobile pull-up-sheet TOC (the static export keeps the
    // plain stacked-top TOC).
    let (body_class, toc_nav, toc_flag) = if ctx.toc {
        (
            " class=\"has-toc tali-toc-sheet\"",
            "<nav id=\"TOC\" aria-label=\"Table of contents\"></nav>\n\
             <div id=\"tali-toc-backdrop\"></div>\n\
             <button id=\"tali-toc-handle\" type=\"button\" aria-label=\"Contents\">\
             <span id=\"tali-toc-cur\"></span><span class=\"tali-toc-grip\"></span></button>",
            "window.TALIESIN_TOC = true;",
        )
    } else {
        ("", "", "")
    };
    // Absolute paths so click-to-source can build `vscode://file/…` links.
    let doc_global = format!(
        "window.TALIESIN_DOC = {{ path: \"{}\", baseDir: \"{}\" }};",
        js_str(ctx.doc_path),
        js_str(ctx.base_dir),
    );
    // The live body: a mountable `#tali-root`, the live TOC nav, and the dev-menu
    // mount. The websocket client drives everything after the first paint.
    let body = format!(
        "<main id=\"tali-root\">{}</main>\n{toc_nav}\n<div id=\"tali-controls\"></div>",
        ctx.body
    );
    let extra_head = format!("<style>{STATUS_CSS}</style>\n");
    let scripts_pre =
        format!("<script>{doc_global} {toc_flag} window.TALIESIN_SSR = true;</script>");
    // With a TOC, load the shared scrollspy (toc-spy.js) ahead of the client so
    // `window.taliInitTocSpy` is defined when client.js rebuilds the nav and calls it
    // after every edit — that drives the active-section highlight and the read-state
    // marks. Without this the TOC sits inert in single-doc preview (scrollspy + read
    // state only worked in the static build / site preview). Search (Cmd-K) stays out:
    // the client doesn't re-index it on a live edit, so its index would go stale.
    let toc_spy = if ctx.toc {
        format!("<script>\n{}\n</script>\n", taliesin_core::TOC_SPY_JS)
    } else {
        String::new()
    };
    let scripts_post = format!("{toc_spy}<script>\n{CLIENT_JS}\n</script>");
    taliesin_core::assemble_html_page(&taliesin_core::PageParts {
        // Live preview always ships everything (a doc can gain any construct on an edit).
        mode: taliesin_core::OutputMode::Preview,
        title: "taliesin",
        // The preview page chrome is English ("taliesin"); the built artifact honours
        // the doc's front-matter `lang:`.
        lang: "en",
        favicon: "<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.ico\" />",
        theme_default: ctx.theme_default,
        theme_css: ctx.theme_css,
        with_site_css: false,
        // A live doc can gain math at any edit, so always ship the KaTeX styles.
        ship_katex: true,
        extra_head: &extra_head,
        body_class,
        include_in_header: &ctx.includes.in_header,
        include_before_body: &ctx.includes.before_body,
        body: &body,
        scripts_pre: &scripts_pre,
        scripts_post: &scripts_post,
        include_after_body: &ctx.includes.after_body,
    })
}

/// Live deck: the same preview client, but mounting sectioned slides into
/// `.tali-deck > .tali-slides` and (re)syncing the deck engine as blocks change. The
/// `TALIESIN_FORMAT` flag switches the client into deck mode.
fn deck_index_html(ctx: &PageCtx) -> String {
    let extra_head = format!("<style>{STATUS_CSS}</style>\n");
    // Absolute paths so click-to-source can build `vscode://file/…` links. The
    // single-doc page sets this in its scripts_pre; the deck has none, so the tail
    // carries it — without it, `openSource` bails (no TALIESIN_DOC) and click-to-source
    // silently does nothing on slides.
    let doc_global = format!(
        "window.TALIESIN_DOC = {{ path: \"{}\", baseDir: \"{}\" }};",
        js_str(ctx.doc_path),
        js_str(ctx.base_dir),
    );
    // The live deck tail: the deck engine, the enhancers, the `QMD_*` flags, the
    // doc's after-body include (an extension plugin's `<script src>` + registration,
    // which must run after the engine and before the client initializes it), then
    // the websocket client last.
    let tail = format!(
        "{deck_script}\n{code_scripts}\n\
         <script>{doc_global} window.TALIESIN_FORMAT = \"deck\"; window.TALIESIN_SSR = true;</script>\n\
         {include_after_body}\n<script>\n{CLIENT_JS}\n</script>\n",
        deck_script = taliesin_core::deck_client_script(),
        code_scripts = taliesin_core::code_scripts(),
        include_after_body = ctx.includes.after_body,
    );
    taliesin_core::assemble_deck_page(&taliesin_core::DeckParts {
        title: "taliesin",
        // The preview page chrome is English ("taliesin"); the built artifact honours
        // the doc's front-matter `lang:`.
        lang: "en",
        favicon: "<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.ico\" />",
        theme_default: ctx.theme_default,
        theme_is_custom: ctx.theme_is_custom,
        theme_css: ctx.theme_css,
        // A live deck can gain math at any edit, so always ship the KaTeX styles.
        ship_katex: true,
        extra_head: &extra_head,
        include_in_header: &ctx.includes.in_header,
        include_before_body: &ctx.includes.before_body,
        slides_attr: " id=\"tali-root\"",
        slides: ctx.body,
        // The dev-menu host (the floating `</>` button), same as the single-doc
        // page: `client.js`'s buildDevMenu fills it with the live status dot,
        // click-to-source toggle, and restart-kernel control. (Was a bare
        // `#tali-status` node, which only showed an orphaned "live" label.)
        after_deck: "<div id=\"tali-controls\"></div>\n",
        tail: &tail,
    })
}

// --- WebSocket ----------------------------------------------------------

async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    State(app): State<Arc<AppState>>,
) -> axum::response::Response {
    if !ws_origin_ok(&headers, app.loopback_bound) {
        return (
            axum::http::StatusCode::FORBIDDEN,
            "cross-origin websocket refused",
        )
            .into_response();
    }
    ws.on_upgrade(move |socket| client_conn(socket, app))
        .into_response()
}

async fn client_conn(socket: WebSocket, app: Arc<AppState>) {
    let (mut sink, mut stream) = socket.split();

    // Subscribe and snapshot under the same lock the watcher uses, so we never
    // miss or double-apply an op straddling the initial render.
    let (snapshot, mut rx) = {
        let d = app.doc.lock();
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
                    let fr = full_render_json(&app.doc.lock());
                    if sink.send(Message::Text(fr.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(t))) => handle_client_msg(t.as_str(), &app),
                Some(Ok(Message::Close(_))) | None => break,
                Some(Err(_)) => break,
                _ => {}
            },
        }
    }
}

fn handle_client_msg(text: &str, app: &AppState) {
    // Control messages are accepted from any connected client without auth. Under
    // the tool's trust model this is intentional: it serves *one author's own*
    // documents, and `--host` is an opt-in convenience for previewing on the
    // author's own devices over a trusted LAN. The only state these messages reach
    // is the author's local render/kernel (worst case, a LAN peer triggers a kernel
    // restart). Multi-tenant/hosted use would need per-connection authorization here.
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("click_block") => {
            let file = v
                .get("source_file")
                .and_then(|f| f.as_str())
                .unwrap_or("(primary)");
            let pos = v.get("sourcepos").and_then(|p| p.as_str()).unwrap_or("?");
            crate::log::source(&format!("{file}  {pos}"));
        }
        Some("restart_kernel") => {
            // Flag the restart and wake the rebuild loop, which drops + respawns
            // the kernel and re-renders (broadcasting the refreshed outputs).
            app.restart_kernel.store(true, Ordering::Relaxed);
            let _ = app.kick.send(());
        }
        _ => {}
    }
}

/// Whether a rendered block is a slide-starting heading (`<h1>`/`<h2>`), i.e. a deck
/// section boundary. Used to tell a slide-level change (add/remove a slide)
/// from a within-slide content edit.
fn is_slide_heading(html: &str) -> bool {
    let h = html.trim_start();
    h.starts_with("<h1") || h.starts_with("<h2")
}

// --- messages -----------------------------------------------------------

fn full_render_json(d: &DocState) -> String {
    protocol::full_render(d.title.as_deref(), &d.body_html(), &d.diagnostics)
}

/// Non-fatal issues with the current document: includes that don't resolve, and
/// the kernel state. Surfaced in the preview so the author sees them without
/// watching the terminal.
fn compute_diagnostics(app: &AppState, executor: &crate::exec::Executor) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    if let Ok(src) = std::fs::read_to_string(&app.path) {
        // A broken front matter is worth pointing AT: a located, framed error that
        // jumps to the bad line on click. (Front-matter key warnings now arrive via
        // `doc.warnings` from the render pass, so they are not re-collected here.)
        if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(&src) {
            diags.push(
                Diagnostic::error(message)
                    .at(None, line)
                    .with_frame(code_frame(&src, line)),
            );
        }
        for dep in taliesin_core::includes::dependencies(&src, &app.base_dir) {
            if !dep.exists() {
                let shown = dep.strip_prefix(&app.base_dir).unwrap_or(&dep);
                diags.push(Diagnostic::warn(format!(
                    "include not found: {}",
                    shown.display()
                )));
            }
        }
    }
    if let Some(message) = executor.diagnostic() {
        diags.push(Diagnostic::warn(message));
    }
    diags
}

/// A small code frame for a located diagnostic: up to two lines of context around
/// the 1-based `line`, each prefixed with its number, the offending line marked
/// `>`. Shown inline (monospace) in the dev panel. Shared with the site server.
pub(crate) fn code_frame(src: &str, line: u32) -> String {
    let lines: Vec<&str> = src.lines().collect();
    if lines.is_empty() || line == 0 {
        return String::new();
    }
    let l = (line as usize).min(lines.len());
    let start = l.saturating_sub(2).max(1);
    let end = (l + 2).min(lines.len());
    let mut out = String::new();
    for n in start..=end {
        let mark = if n as u32 == line { '>' } else { ' ' };
        out.push_str(&format!("{mark} {n:>3} | {}\n", lines[n - 1]));
    }
    out
}

fn op_json(op: &BlockOp) -> String {
    protocol::op(op, |html| html.to_string())
}

// --- file watching ------------------------------------------------------

fn spawn_watcher(app: Arc<AppState>, mut signal_rx: mpsc::UnboundedReceiver<()>) {
    let signal_tx = app.kick.clone();
    let dirs = watch_dirs(&app);

    // notify is synchronous; run it on its own thread and forward events.
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
                    && relevant_event(&ev)
                {
                    let _ = signal_tx.send(());
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
        // Persistent execution cache for this doc (`_freeze/<stem>.json` beside the
        // source), so a preview restart warms from disk instead of re-executing.
        let stem = app
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document");
        let freeze_path = crate::freeze::page_path(&app.base_dir.join("_freeze"), stem);
        let mut executor = crate::exec::Executor::with_freeze(freeze_path).in_dir(&app.base_dir);
        // Stream code-cell execution progress (`build-state`) onto the session
        // broadcast, so the previewing client can show "warming kernel" / "cell k/N".
        // Single-doc => no page key. Cloning the `Sender` is cheap; each progress
        // message is just sent like any other render op.
        {
            let tx = app.tx.clone();
            let sink: crate::exec::ProgressSink = Some(std::sync::Arc::new(move |m| {
                let _ = tx.send(m);
            }));
            executor.set_progress(sink, None);
        }
        // Initial execution pass: markdown is already live; this fills in outputs
        // (and starts the warm kernel) shortly after the page loads.
        rebuild_guarded(&app, &mut executor).await;
        while signal_rx.recv().await.is_some() {
            tokio::time::sleep(Duration::from_millis(80)).await;
            while signal_rx.try_recv().is_ok() {}
            // Honour a pending dev-menu "Restart kernel" before this rebuild.
            let restarted = app.restart_kernel.swap(false, Ordering::Relaxed);
            if restarted {
                crate::log::kernel("restart requested (dev menu)");
                executor.restart_kernel();
            }
            rebuild_guarded(&app, &mut executor).await;
            // A fresh kernel means fresh outputs — including any `ojs_define`
            // values. Reload so the `{js}` cells re-bind to the fresh `qmd-define`
            // blobs from a clean module scope.
            if restarted {
                let _ = app.tx.send(protocol::reload());
            }
        }
    });
}

/// Whether a file-watch event touches something a re-render actually depends on:
/// a source/content/asset file, and not a build-output or VCS directory. Filters
/// out the noise (editor swap files, `_site/`/`_book/` output, `.git`)
/// that would otherwise trigger a wasteful 0-op rebuild on every unrelated save.
fn relevant_event(ev: &notify::Event) -> bool {
    ev.paths.iter().any(|p| relevant_path(p))
}

/// Whether a changed path should trigger a rebuild: a known source/asset extension,
/// not under a generated/VCS directory. `_freeze` is skipped so the executor's own
/// cache writes don't kick a redundant rebuild on every run.
pub(crate) fn relevant_path(p: &Path) -> bool {
    const EXTS: &[&str] = &[
        "tmd", "md", "bib", "csl", "css", "scss", "yml", "yaml", "json", "js", "html", "svg",
        "png", "jpg", "jpeg", "webp", "gif",
    ];
    const SKIP_DIRS: &[&str] = &["_site", "_book", "_freeze", ".git", "node_modules"];
    let ext_ok = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()));
    let in_skip_dir = p.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| SKIP_DIRS.contains(&s))
    });
    ext_ok && !in_skip_dir
}

/// Directories to watch: the project base dir (recursively — the site server's model),
/// which covers every in-tree include, including ones added after startup. An include that
/// resolves OUTSIDE the base dir (a sibling file up the tree) can't be covered by the
/// recursive base-dir watch and the watch set is fixed at startup, so we still register its
/// dir (so an out-of-tree include present now keeps refreshing) but warn once that an
/// out-of-tree sibling needs a manual reload — `relevant_path` still filters every event.
fn watch_dirs(app: &AppState) -> Vec<PathBuf> {
    let mut dirs: HashSet<PathBuf> = HashSet::new();
    // Recursive watch of the base dir is the primary mechanism: it covers any in-tree
    // include without enumerating each one, so a NEW in-tree include is picked up too.
    dirs.insert(app.base_dir.clone());
    if let Ok(src) = std::fs::read_to_string(&app.path) {
        for dep in taliesin_core::includes::dependencies(&src, &app.base_dir) {
            // In-tree includes are already covered by the recursive base-dir watch.
            if dep.starts_with(&app.base_dir) {
                continue;
            }
            // Out-of-tree (a sibling above base_dir): register its dir so a currently-present
            // one keeps refreshing, but warn — a sibling added later won't be auto-watched.
            if let Some(parent) = dep.parent() {
                dirs.insert(parent.to_path_buf());
            }
            crate::log::warn(&format!(
                "include lives outside the preview root: {}; edits to it refresh only while it \
                 exists now — a sibling added later needs a manual reload",
                dep.display()
            ));
        }
    }
    dirs.into_iter().collect()
}

/// Re-render markdown, execute code cells (changed + downstream), then diff the
/// assembled block list against the live state and broadcast the changes.
async fn rebuild(app: &AppState, executor: &mut crate::exec::Executor) {
    let Some(doc) = render_doc(app) else {
        app.doc.lock().errored = true;
        let _ = app.tx.send(protocol::error(&format!(
            "cannot read {}",
            app.path.display()
        )));
        return;
    };
    let blocks = executor.run(doc.blocks).await;
    let mut diags = compute_diagnostics(app, executor);
    // Render warnings (missing bibliography/theme file, broken citation) ride
    // alongside the include/kernel diagnostics into the dev menu.
    // A warning that carries a line becomes a clickable jump-to-source row.
    for w in &doc.warnings {
        let mut d = Diagnostic::warn(&w.message);
        if let Some(line) = w.line {
            d = d.at(w.file.clone(), line);
        }
        diags.push(d);
    }
    // A standalone doc has no site to resolve cross-page refs, so any cross-ref
    // still marked unresolved is broken.
    for w in taliesin_core::cite::validate_xrefs(&blocks) {
        let mut d = Diagnostic::warn(&w.message);
        if let Some(line) = w.line {
            d = d.at(w.file.clone(), line);
        }
        diags.push(d);
    }
    let ops = {
        let mut d = app.doc.lock();
        let recovered = std::mem::take(&mut d.errored);
        let ops = taliesin_core::diff_blocks(&d.blocks, &blocks);
        // A deck re-mounts fully only when an insert/remove touches a slide HEADING
        // (add / remove a slide in the source): its `<section>`-grouped slides can't
        // be restructured by flat block ops. Content edits within a slide (inserting a
        // paragraph, re-titling, editing text) stay incremental. Computed before
        // `d.blocks` is replaced, so a Remove can look up the old block's html.
        let deck_structural = matches!(doc.format, DocFormat::Reveal)
            && ops.iter().any(|op| match op {
                BlockOp::Insert { html, .. } => is_slide_heading(html),
                BlockOp::Remove { target_id } => d
                    .blocks
                    .iter()
                    .any(|b| &b.id == target_id && is_slide_heading(&b.html)),
                // A content edit or a pure position-metadata shift never restructures
                // slides (no <section> added or removed).
                BlockOp::Update { .. } | BlockOp::SetMeta { .. } => false,
            });
        let diags_changed = d.diagnostics != diags;
        let theme_changed = d.theme_css != doc.theme_css;
        d.title = doc.title;
        d.subtitle = doc.subtitle;
        d.format = doc.format;
        d.toc = doc.toc;
        d.theme_css = doc.theme_css;
        d.theme_default = doc.theme_default;
        d.theme_is_custom = doc.theme_is_custom;
        d.includes = doc.includes;
        d.warnings = doc.warnings;
        d.blocks = blocks;
        d.diagnostics = diags;
        // Broadcast under the lock so connecting clients can't interleave.
        if recovered || deck_structural {
            // Re-mount fully when recovering from an error (so every client clears its
            // overlay) or a deck changed structurally. The deck preserves its current
            // slide + overview across the swap (its JS state survives the DOM rebuild).
            let _ = app.tx.send(full_render_json(&d));
        } else {
            for op in &ops {
                let _ = app.tx.send(op_json(op));
            }
        }
        // A theme/`.css` edit: hot-swap the theme `<style>` in place. Sent AFTER the
        // if/else (not only on the incremental path), so a save that both changes the
        // theme and triggers a full re-mount (error recovery, a deck restructure) still
        // applies the new theme — the re-mounted HTML carries the old `<style>` body.
        if theme_changed {
            let _ = app.tx.send(protocol::style(&d.theme_css));
        }
        if diags_changed {
            let _ = app.tx.send(protocol::diagnostics(&d.diagnostics));
        }
        ops.len()
    };
    if ops > 0 {
        crate::log::update(ops);
    }
}

/// Run a [`rebuild`], but catch any panic in the render/exec path so one bad
/// document can't kill the rebuild task (which would silently stop hot-reload
/// for the rest of the session). The panic is logged and surfaced to the client
/// as an error diagnostic; the next good save recovers. `parking_lot` mutexes
/// mean a panic mid-update releases the lock cleanly rather than poisoning it.
async fn rebuild_guarded(app: &AppState, executor: &mut crate::exec::Executor) {
    use futures_util::FutureExt;
    let outcome = std::panic::AssertUnwindSafe(rebuild(app, executor))
        .catch_unwind()
        .await;
    if let Err(payload) = outcome {
        let msg = panic_msg(&*payload);
        crate::log::error(&format!("render panicked (preview kept alive): {msg}"));
        app.doc.lock().errored = true;
        let _ = app
            .tx
            .send(protocol::error(&format!("internal render error: {msg}")));
    }
}

/// Best-effort human string from a caught panic payload (`Box<dyn Any>`).
pub(crate) fn panic_msg(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

/// Run a synchronous `f` under [`std::panic::catch_unwind`], turning a panic into a clean
/// `Err(message)` (via [`panic_msg`]) instead of aborting the process. The one-shot
/// commands (`build`/`check`/`render`/`blocks`) call core rendering directly with no
/// async rebuild loop to absorb a panic, so without this a malformed doc that panics the
/// renderer crashes the CLI with a raw backtrace + abort instead of a located error and a
/// non-zero exit. `AssertUnwindSafe` is sound here: a panic mid-`f` is surfaced and the
/// caller returns immediately, so no half-updated state is observed afterward.
pub(crate) fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|p| panic_msg(&*p))
}

/// Build a hard-error message for an unrecognized `--flag`, appending a `closest`-based
/// "did you mean `--strict`?" when a known flag is within edit distance 2. Shared by the
/// `build`/`check`/`serve` flag parsers so a typo'd flag fails loudly instead of being
/// silently dropped. `known` is each parser's own accepted long-flag set. No `error:`
/// prefix — the caller frames it (raw `eprintln!` adds `error: `; `log::error` styles it).
pub(crate) fn unknown_flag_error(flag: &str, known: &[&'static str]) -> String {
    match taliesin_core::closest(flag, known) {
        Some(s) => format!("unknown flag `{flag}` (did you mean `{s}`?)"),
        None => format!("unknown flag `{flag}`"),
    }
}

#[cfg(test)]
mod protocol_contract {
    //! The single-doc producers share the op/message contract that the preview
    //! client (web-client/client.js) consumes; the comprehensive shape test lives
    //! in serve_site.rs. This guards serve.rs's own `*_json` against drift.
    use super::*;
    use crate::testutil::parse;

    #[test]
    fn relevant_path_watches_tmd_edits() {
        // `.tmd` is the native (and only) source extension; a watcher blind to it would
        // silently never rebuild on a `.tmd` edit — the core edit loop would be broken.
        assert!(relevant_path(Path::new("/tmp/doc.tmd")));
        // `.qmd` is no longer a source extension: a `.qmd` edit must not trigger a rebuild.
        assert!(!relevant_path(Path::new("/tmp/doc.qmd")));
        assert!(!relevant_path(Path::new("/tmp/doc.txt")));
    }

    #[test]
    fn style_message_carries_css_for_hot_swap() {
        let m = parse(protocol::style(":root{--tali-accent:#f00}"));
        assert_eq!(m["type"], "style");
        assert_eq!(m["css"], ":root{--tali-accent:#f00}");
    }

    #[test]
    fn located_diagnostic_serializes_file_line_and_frame() {
        let d = Diagnostic::error("bad yaml")
            .at(None, 3)
            .with_frame("> 3 | x\n".into());
        let m = parse(protocol::diagnostics(&[d]));
        assert_eq!(m["messages"][0]["level"], "error");
        assert_eq!(m["messages"][0]["line"], 3);
        assert!(m["messages"][0]["frame"].as_str().unwrap().contains("> 3"));
    }

    #[test]
    fn reveal_index_carries_qmd_doc_for_click_to_source() {
        // The deck page has no scripts_pre, so its tail must inject TALIESIN_DOC — without
        // it, client.js's openSource bails (no doc) and click-to-source is dead on
        // slides, even though every block carries data-block-id/sourcepos.
        let includes = taliesin_core::render::PageIncludes::default();
        let ctx = PageCtx {
            format: DocFormat::Reveal,
            toc: false,
            theme_css: "",
            theme_default: "auto",
            theme_is_custom: false,
            doc_path: "/tmp/deck.tmd",
            base_dir: "/tmp",
            includes: &includes,
            body: "<section><h2>S</h2></section>",
        };
        let html = deck_index_html(&ctx);
        assert!(
            html.contains("window.TALIESIN_DOC = { path: \"/tmp/deck.tmd\", baseDir: \"/tmp\" }"),
            "deck page must carry TALIESIN_DOC for click-to-source"
        );
    }

    #[test]
    fn blog_index_ships_toc_scrollspy_when_toc_enabled() {
        // The single-doc live preview must load toc-spy.js when the doc has a TOC, so
        // scrollspy highlighting + read-state TOC work in `taliesin preview <file>`
        // (client.js rebuilds the nav, then calls window.taliInitTocSpy). The `qmd-read:`
        // storage key is unique to toc-spy.js — client.js only *calls* taliInitTocSpy —
        // so it discriminates "script loaded" from "script merely referenced".
        let includes = taliesin_core::render::PageIncludes::default();
        let mk = |toc| {
            let ctx = PageCtx {
                format: DocFormat::Html,
                toc,
                theme_css: "",
                theme_default: "auto",
                theme_is_custom: false,
                doc_path: "/tmp/doc.tmd",
                base_dir: "/tmp",
                includes: &includes,
                body: "<h2 id=\"s\" data-block-id=\"b\">S</h2>",
            };
            blog_index_html(&ctx)
        };
        assert!(
            mk(true).contains("qmd-read:"),
            "a TOC preview must load toc-spy.js so scrollspy + read-state work live"
        );
        assert!(
            !mk(false).contains("qmd-read:"),
            "a no-TOC preview should not load the TOC scrollspy"
        );
    }

    #[test]
    fn ops_and_full_render_match_client_contract() {
        let up = parse(op_json(&BlockOp::Update {
            target_id: "b".into(),
            html: "h".into(),
        }));
        assert_eq!(up["type"], "update");
        assert_eq!(up["target_id"], "b");
        assert!(up.get("html").is_some());

        let ins = parse(op_json(&BlockOp::Insert {
            after_id: Some("b".into()),
            html: "h".into(),
        }));
        assert_eq!(ins["type"], "insert");
        assert!(ins.get("after_id").is_some());

        let rm = parse(op_json(&BlockOp::Remove {
            target_id: "b".into(),
        }));
        assert_eq!(rm["type"], "remove");

        let fr = parse(full_render_json(&DocState::default()));
        assert_eq!(fr["type"], "full_render");
        assert!(fr.get("body_html").is_some());
        assert!(fr["diagnostics"].is_array());
    }

    #[tokio::test]
    async fn rebuild_guard_catches_panic_and_recovers_message() {
        // The exact mechanism `rebuild_guarded` relies on: a panic inside the
        // awaited render future is caught (not propagated, so the rebuild loop
        // survives) and its message is recovered for the error diagnostic.
        use futures_util::FutureExt;
        let outcome = std::panic::AssertUnwindSafe(async { panic!("render boom") })
            .catch_unwind()
            .await;
        let payload = outcome.expect_err("the panic must be caught, not propagated");
        assert_eq!(panic_msg(&*payload), "render boom");
    }
}
