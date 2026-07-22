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
pub(crate) use security::{
    lan_url, new_session_token, with_host_guard, with_lan_guard, ws_origin_ok,
};

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
    /// Deck chrome (front-matter `footer:`/`logo:`): a persistent per-slide footer text
    /// and corner logo image. Rendered into the initial deck page; a live edit to either
    /// is reflected on the next full page load (the overlay sits outside the diffed mount).
    footer: Option<String>,
    logo: Option<String>,
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
    /// Monotonic body-render generation, bumped whenever `blocks` change. Stamped
    /// into the SSR page (`window.TALIESIN_SSR_GEN`) and every `full_render`; the
    /// client compares the two to tell a still-current SSR body (skip the re-mount)
    /// from one made stale by a rebuild (e.g. the initial code-exec pass) that landed
    /// between the HTTP render and the websocket connect. See [`protocol::full_render`].
    generation: u64,
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
    let result = rt.block_on(serve(path, port, open, expose));
    // `serve` returns on a shutdown signal (see `shutdown_signal`); force the runtime
    // down so the spawned watcher task that owns the kernel is dropped promptly,
    // running its teardown (`Kernel`/`ForkserverDaemon` Drop = process/group SIGKILL).
    // Bounded so a wedged background task can't hang exit; the kills are synchronous,
    // so they still run even as the runtime is torn down.
    rt.shutdown_timeout(std::time::Duration::from_secs(5));
    result
}

/// Resolve when the process is asked to shut down: Ctrl-C (SIGINT) or SIGTERM. The
/// two dev servers race their `axum::serve` against this so `serve` **returns** on a
/// signal (rather than the process being hard-killed with kernels still live),
/// letting `run` tear the runtime down and drop the watcher/builder tasks that own
/// the kernels + warm pool — which runs their teardown Drops. We race (rather than
/// `axum`'s `with_graceful_shutdown`) because the preview holds a persistent
/// websocket that never closes on its own, so graceful shutdown would hang. Without
/// this, a Ctrl-C'd preview leaks the whole kernel/forkserver subtree.
pub(crate) async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            // Can't install the SIGTERM handler: fall back to Ctrl-C only.
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
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
            d.footer = doc.footer;
            d.logo = doc.logo;
            d.format = doc.format;
            d.toc = doc.toc;
            d.theme_css = doc.theme_css;
            d.theme_default = doc.theme_default;
            d.theme_is_custom = doc.theme_is_custom;
            d.includes = doc.includes;
            d.warnings = doc.warnings;
            d.blocks = doc.blocks;
        }
        // The file isn't there (yet). Serving a blank page in silence looked like a broken
        // renderer; every other command exits 1 on a missing path. Creating the file later
        // *does* work — the watcher picks it up — so say so rather than refuse.
        Ok(None) => crate::log::warn(&format!(
            "cannot read {} — serving an empty page; it will render as soon as the file exists",
            app.path.display()
        )),
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
    // Under --host, the bound LAN IP is a legitimate `Host`; in loopback mode only
    // loopback names are (the DNS-rebinding allowlist).
    let lan_ip: Option<Arc<str>> = expose
        .then(local_ip)
        .flatten()
        .map(|ip| Arc::from(ip.to_string()));

    let router = Router::new()
        .route("/", get(index))
        .route("/favicon.ico", get(favicon))
        .route("/ws", get(ws_handler))
        // Anything else is a static asset (images, etc.) resolved relative to the
        // document's directory, so figures display in the live preview.
        .fallback(static_asset)
        .with_state(app.clone());
    let router = with_lan_guard(router, token.clone());
    let router = with_host_guard(router, lan_ip);

    let (listener, addr) = bind_with_fallback(port, expose).await?;
    let port = addr.port();
    let local = format!("http://127.0.0.1:{port}");
    // With --host we bound 0.0.0.0; surface the LAN URL (and a QR for phones), with the
    // session token in `?t=` so the first load authenticates and sets the cookie.
    let network = expose
        .then(local_ip)
        .flatten()
        .map(|ip| lan_url(&format!("http://{ip}:{port}"), token.as_ref()));
    let (desc, narration) = {
        let d = app.doc.lock();
        let mut parts = vec![match d.format {
            DocFormat::Reveal => "deck",
            DocFormat::Html => "html",
        }];
        if d.toc {
            parts.push("toc");
        }
        // A deck's speaker notes are a spoken script; report its estimated length so a
        // recording author sees the runtime up front (None for a non-deck / no notes).
        (
            parts.join(", "),
            taliesin_core::script_summary(&d.body_html()),
        )
    };
    crate::log::clear_screen();
    crate::log::banner(taliesin_core::VERSION);
    crate::log::ready(&local, start.elapsed());
    if let Some(net) = &network {
        crate::log::network(net);
    } else if expose {
        crate::log::warn("--host set, but no LAN address was found");
    }
    crate::log::keys_hint();
    crate::log::watching(&app.path.display().to_string(), &desc);
    if let Some(n) = &narration {
        crate::log::deck_duration(n.total_secs, n.scripted, n.slides);
    }
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
    // the runtime teardown in `run` can reap the kernel (see `shutdown_signal`).
    tokio::select! {
        r = server => r.map_err(std::io::Error::other),
        _ = shutdown_signal() => {
            crate::log::kernel("shutting down (reaping kernel)");
            Ok(())
        }
    }
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
    // Single-document preview: confine includes/resources to the doc's own directory
    // (PT-2) so an untrusted `.tmd` opened from inside a larger checkout cannot climb
    // out to read a sibling repo-local file.
    Some(taliesin_core::render_document_with_includes_rooted(
        &src,
        &app.base_dir,
        Some(&app.base_dir),
    ))
}

// --- HTTP ---------------------------------------------------------------

async fn index(State(app): State<Arc<AppState>>) -> Html<String> {
    let (
        format,
        toc,
        theme_css,
        theme_default,
        theme_is_custom,
        includes,
        body,
        footer,
        logo,
        generation,
    ) = {
        let d = app.doc.lock();
        (
            d.format,
            d.toc,
            d.theme_css.clone(),
            d.theme_default.clone(),
            d.theme_is_custom,
            d.includes.clone(),
            d.body_html(),
            d.footer.clone(),
            d.logo.clone(),
            d.generation,
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
        footer: footer.as_deref(),
        logo: logo.as_deref(),
        generation,
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
    /// Deck chrome (front-matter `footer:`/`logo:`), rendered as a persistent overlay on
    /// a live deck's initial page. `None` for a non-deck doc or a deck without chrome.
    footer: Option<&'a str>,
    logo: Option<&'a str>,
    /// The render generation `body` was built at, stamped into
    /// `window.TALIESIN_SSR_GEN` so the client can tell a still-current SSR body from
    /// one a rebuild made stale before the websocket connected.
    generation: u64,
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
///
/// Decodes on the byte level: slicing `s` by byte offsets (`&s[i+1..i+3]`) panics when a
/// `%` is immediately followed by a raw multi-byte UTF-8 char (a crafted `GET /%€`), so the
/// two hex digits are read straight from the byte buffer instead.
pub(crate) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(hi) = hex_val(b[i + 1])
            && let Some(lo) = hex_val(b[i + 2])
        {
            out.push(hi << 4 | lo);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A single ASCII hex digit (`0-9`/`a-f`/`A-F`) as its 0-15 value, else `None`.
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
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
    .tali-dev-drafts { display: flex; flex-direction: column; gap: .2rem; margin-top: -.2rem; } \
    .tali-dev-drafts a { color: var(--tali-accent, #4c8dff); text-decoration: none; font-size: 12px; } \
    .tali-dev-drafts a:hover { text-decoration: underline; } \
    .tali-dev-ctl { display: inline-flex; align-items: center; gap: .4rem; text-align: left; cursor: pointer; \
      background: var(--tali-code-bg, #f5f5f5); color: var(--tali-fg, #111); \
      border: 1px solid var(--tali-border, #e0e0e0); border-radius: 6px; padding: .3rem .55rem; } \
    .tali-dev-ctl:hover { border-color: var(--tali-accent, #4c8dff); } \
    .tali-dev-card { display: block; max-width: 100%; height: auto; border-radius: 6px; \
      border: 1px solid var(--tali-border, #e0e0e0); } \
    .tali-dev-card[hidden] { display: none; } \
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
    /* The dev menu owns the bottom-left corner in preview; lift the reader \"Resume reading\" pill \
       above it so the two never overlap (base.css anchors the pill at bottom:1rem, ~1rem below the \
       dev button). Preview-only: a static build ships neither this rule nor the dev menu, so the \
       pill stays at its natural bottom:1rem there. */ \
    .tali-resume { bottom: 2.9rem; } \
    @media (max-width: 60rem) { body.tali-toc-sheet .tali-resume { bottom: 4.7rem; } } \
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
    [data-qmd-cell-source=\"cache\"] { border-left-color: color-mix(in srgb, #2bb673 40%, transparent); } \
    [data-qmd-cell-source=\"cache\"] .tali-cell-badge { opacity: .6; } \
    .tali-cell-badge { font: 11px/1 var(--tali-mono, monospace); opacity: .75; margin-right: 6px; } \
    @media (prefers-reduced-motion: no-preference) { \
      [data-qmd-cell-state=\"running\"] .tali-cell-badge { animation: tali-pulse 1s ease-in-out infinite; } \
      @keyframes tali-pulse { 50% { opacity: .35; } } \
    } \
    .tali-hint-nudge { position: absolute; bottom: calc(100% + .45rem); left: 0; width: 14rem; \
      display: flex; flex-direction: column; gap: .4rem; padding: .6rem .7rem; \
      font: 12px/1.4 ui-sans-serif, system-ui, sans-serif; \
      background: var(--tali-bg, #fff); color: var(--tali-fg, #111); \
      border: 1px solid var(--tali-border, #e0e0e0); border-radius: 9px; \
      box-shadow: 0 8px 28px rgba(0,0,0,.2); } \
    .tali-hint-nudge[hidden] { display: none; } \
    .tali-hint-nudge::after { content: \"\"; position: absolute; top: 100%; left: 1.1rem; \
      border: 6px solid transparent; border-top-color: var(--tali-bg, #fff); \
      filter: drop-shadow(0 1px 0 var(--tali-border, #e0e0e0)); } \
    .tali-hint-line { display: flex; align-items: baseline; gap: .35rem; color: var(--tali-fg, #111); } \
    .tali-hint-nudge kbd { font: 11px/1 ui-monospace, SFMono-Regular, Menlo, monospace; \
      padding: .1rem .3rem; border: 1px solid var(--tali-border, #d0d0d0); border-radius: 4px; \
      background: var(--tali-code-bg, #f5f5f5); color: var(--tali-fg, #111); } \
    .tali-hint-dismiss { align-self: flex-end; margin-top: .1rem; cursor: pointer; background: none; \
      border: none; padding: .15rem .2rem; color: var(--tali-accent, #4c8dff); \
      font: 600 12px ui-sans-serif, system-ui, sans-serif; } \
    .tali-hint-dismiss:hover { text-decoration: underline; } \
    @media (prefers-reduced-motion: no-preference) { \
      .tali-hint-nudge { animation: tali-hint-in .18s ease-out; } \
      @keyframes tali-hint-in { from { opacity: 0; transform: translateY(4px); } } \
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
    let scripts_pre = format!(
        "<script>{doc_global} {toc_flag} window.TALIESIN_SSR = true; window.TALIESIN_SSR_GEN = {}; window.TALIESIN_BOOT = {};</script>",
        ctx.generation,
        protocol::boot_id()
    );
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
        ..taliesin_core::PageParts::defaults()
    })
}

/// Live deck: the same preview client, but mounting sectioned slides into
/// `.tali-deck > .tali-slides` and (re)syncing the deck engine as blocks change. The
/// `TALIESIN_FORMAT` flag switches the client into deck mode.
fn deck_index_html(ctx: &PageCtx) -> String {
    let extra_head = format!("<style>{STATUS_CSS}</style>\n");
    let deck_overlay = taliesin_core::deck_overlay_html(ctx.footer, ctx.logo);
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
         <script>{doc_global} window.TALIESIN_FORMAT = \"deck\"; window.TALIESIN_SSR = true; window.TALIESIN_SSR_GEN = {generation}; window.TALIESIN_BOOT = {boot};</script>\n\
         {include_after_body}\n<script>\n{CLIENT_JS}\n</script>\n",
        deck_script = taliesin_core::deck_client_script(),
        code_scripts = taliesin_core::code_scripts(),
        include_after_body = ctx.includes.after_body,
        generation = ctx.generation,
        boot = protocol::boot_id(),
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
        // Persistent footer/logo overlay for the live deck's initial render (a live edit to
        // either shows on the next full page load; the overlay sits outside the diffed mount).
        deck_overlay: &deck_overlay,
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

/// A `. . .` pause paragraph (mirrors `deck.rs`'s `is_pause`): a `<p>` whose only text
/// content is the three-dot marker. Inserting/removing one regroups the following blocks
/// into `.fragment` steps, so it restructures a deck the same way a heading does.
fn is_pause_paragraph(html: &str) -> bool {
    let h = html.trim_end();
    h.starts_with("<p")
        && h.ends_with("</p>")
        && h.find('>')
            .is_some_and(|open| h[open + 1..h.len() - "</p>".len()].trim() == ". . .")
}

/// Whether inserting or removing a rendered block restructures a deck's slides: a
/// slide-starting heading (`<h1>`/`<h2>`, a `<section>` boundary), a `---` thematic
/// break (`<hr .../>`, which splits one slide into two), or a `. . .` pause paragraph.
/// All three are consumed by the slide model in `deck.rs` and never emitted as content,
/// so the flat block-swap can't apply them — the deck must fully re-mount.
fn is_slide_structural(html: &str) -> bool {
    let h = html.trim_start();
    h.starts_with("<h1") || h.starts_with("<h2") || h.starts_with("<hr") || is_pause_paragraph(h)
}

/// Whether a live-edit block op restructures a deck's slides (so it must fully re-mount
/// rather than apply incrementally). An Insert/Remove/Update that touches a slide
/// boundary (heading / `---` / `. . .`) adds, removes, splits, or merges a slide — the
/// flat block-swap can't restructure the server-built `<section>`s, and the projection
/// diff has no notion of slide grouping. A heading Update also re-slugs its `<section>`
/// id (which lives on the wrapper, not the swapped-in `<h2>`), so `#hash`/`@ref` to the
/// new title would otherwise resolve against the stale slug. In particular a plain
/// paragraph edited IN PLACE into a `---` or `. . .` (or back) is an Update, not an
/// Insert/Remove, so the Update arm must use the full `is_slide_structural` too — else
/// the live view silently diverges from a full render. `old_blocks` resolves a
/// Remove/Update target to its pre-edit html (looked up before `d.blocks` is replaced).
fn deck_op_is_structural(op: &BlockOp, old_blocks: &[Block]) -> bool {
    let old_html_is = |target_id: &String, pred: fn(&str) -> bool| {
        old_blocks
            .iter()
            .any(|b| &b.id == target_id && pred(&b.html))
    };
    match op {
        BlockOp::Insert { html, .. } => is_slide_structural(html),
        BlockOp::Remove { target_id } => old_html_is(target_id, is_slide_structural),
        BlockOp::Update { target_id, html } => {
            is_slide_structural(html) || old_html_is(target_id, is_slide_structural)
        }
        BlockOp::SetMeta { .. } => false,
    }
}

/// Whether a deck's front-matter title/subtitle changed between two renders. The deck
/// title slide is built from them by `slides_html`, OUTSIDE `doc.blocks`, so retitling
/// produces an empty block diff (nothing for the incremental swap to apply) and must force
/// a full re-mount (B3-15). Only a deck has a front-matter title slide — a regular HTML
/// page carries its title as an ordinary block, which the diff already handles.
fn deck_meta_changed(
    format: DocFormat,
    old_title: &Option<String>,
    old_subtitle: &Option<String>,
    new_title: &Option<String>,
    new_subtitle: &Option<String>,
) -> bool {
    matches!(format, DocFormat::Reveal) && (old_title != new_title || old_subtitle != new_subtitle)
}

// --- messages -----------------------------------------------------------

fn full_render_json(d: &DocState) -> String {
    protocol::full_render(
        d.title.as_deref(),
        &d.body_html(),
        d.generation,
        &d.diagnostics,
    )
}

/// Non-fatal issues with the current document: a framed front-matter parse error, and the
/// kernel state. Surfaced in the preview so the author sees them without watching the
/// terminal.
///
/// A missing `{{< include >}}` is deliberately *not* checked here. The render pass already
/// emits a located `IncludeWarning` on the directive's own line, which reaches this same
/// channel through `doc.warnings`; checking again produced two diagnostics for one defect,
/// and the extra one had no line to click.
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

fn op_json(op: &BlockOp, generation: u64) -> String {
    protocol::op(op, generation, |html| html.to_string())
}

// --- file watching ------------------------------------------------------

fn spawn_watcher(app: Arc<AppState>, mut signal_rx: mpsc::UnboundedReceiver<()>) {
    let signal_tx = app.kick.clone();
    let dirs = watch_dirs(&app);
    let base = app.base_dir.clone();

    // notify is synchronous; run it on its own thread. Events are pumped through a
    // channel so this thread OWNS the watcher and can register new watches on the fly
    // (a subdirectory created after startup) — the recursive-watch model registered an
    // inotify descriptor for every directory including `node_modules`/`.git`, which a
    // large project uses to exhaust `max_user_watches` and silently kill hot reload.
    std::thread::spawn(move || {
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
        // Register a NON-recursive watch on each pruned directory; each reports
        // create/modify/remove for its own direct children.
        for dir in &dirs {
            if let Err(e) = watcher.watch(dir, notify::RecursiveMode::NonRecursive) {
                crate::log::warn(&format!("cannot watch {}: {e}", dir.display()));
            }
        }
        // Blocking on the channel keeps both the watcher and this thread alive.
        for ev in ev_rx {
            if !matches!(
                ev.kind,
                notify::EventKind::Modify(_)
                    | notify::EventKind::Create(_)
                    | notify::EventKind::Remove(_)
            ) {
                continue;
            }
            // A newly-created in-tree subdirectory (and any non-pruned dirs it arrived
            // with) needs its own non-recursive watch, since the base-dir walk that
            // seeded the watch set ran once at startup.
            if matches!(ev.kind, notify::EventKind::Create(_)) {
                for p in &ev.paths {
                    let is_dir = std::fs::symlink_metadata(p)
                        .map(|m| m.is_dir())
                        .unwrap_or(false);
                    if is_dir && p.starts_with(&base) && !is_pruned_dir(p) {
                        for d in watch_tree(p) {
                            let _ = watcher.watch(&d, notify::RecursiveMode::NonRecursive);
                        }
                        // Files that already existed inside the new dir were created before
                        // its watch existed, so their events were missed — kick a rebuild if
                        // any is relevant (a new in-tree include folder, a `git checkout`).
                        if !subtree_relevant_files(p).is_empty() {
                            let _ = signal_tx.send(());
                        }
                    }
                }
            }
            if relevant_event(&ev) {
                let _ = signal_tx.send(());
            }
        }
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
        // Resolve this document's interpreters from its own directory (no _site.yml here,
        // so a `.venv` beside the doc / env / default), so a project-local venv beats a
        // stray global TALIESIN_PYTHON and the first kernel start logs which one ran.
        executor.set_interpreters(
            crate::interpreter::resolve_python(None, &app.base_dir),
            crate::interpreter::resolve_r(None, &app.base_dir),
        );
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

/// Generated/VCS directory names pruned from both the watch set (so notify never
/// registers an inotify descriptor inside them — a big `node_modules`/`.git` would
/// otherwise blow past `max_user_watches`) and the event filter. `_freeze` is skipped
/// so the executor's own cache writes don't kick a redundant rebuild on every run.
const SKIP_DIRS: &[&str] = &["_site", "_book", "_freeze", ".git", "node_modules"];

/// Whether a file-watch event touches something a re-render actually depends on:
/// a source/content/asset file, and not a build-output or VCS directory. Filters
/// out the noise (editor swap files, `_site/`/`_book/` output, `.git`)
/// that would otherwise trigger a wasteful 0-op rebuild on every unrelated save.
fn relevant_event(ev: &notify::Event) -> bool {
    ev.paths.iter().any(|p| relevant_path(p))
}

/// Whether a changed path should trigger a rebuild: a known source/asset extension,
/// not under a generated/VCS directory.
pub(crate) fn relevant_path(p: &Path) -> bool {
    const EXTS: &[&str] = &[
        "tmd", "md", "bib", "csl", "css", "scss", "yml", "yaml", "json", "js", "html", "svg",
        "png", "jpg", "jpeg", "webp", "gif",
    ];
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

/// Whether a directory should be pruned from the watch set by its own name — a
/// generated/VCS tree we never register an inotify watch inside.
pub(crate) fn is_pruned_dir(dir: &Path) -> bool {
    dir.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| SKIP_DIRS.contains(&n))
}

/// Every directory under `base` (inclusive) that we register a non-recursive watch on,
/// pruning generated/VCS subtrees (`node_modules`, `.git`, `_site`, `_book`, `_freeze`)
/// whole. notify's `Recursive` mode walks the *entire* tree and adds one inotify watch
/// descriptor per directory — a big `node_modules` alone can exhaust `max_user_watches`
/// and silently kill hot reload — so we enumerate only the directories a rebuild can
/// actually depend on and watch each non-recursively (which reports create/modify/remove
/// for its direct children, the same coverage recursive mode builds internally).
/// Symlinked directories are not followed, avoiding watch loops.
pub(crate) fn watch_tree(base: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        dirs.push(dir.clone());
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // `file_type()` uses the readdir/lstat type, so a symlink reads as a symlink
            // (not a dir) and is skipped — we descend only into real directories.
            let is_real_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_real_dir && !is_pruned_dir(&path) {
                stack.push(path);
            }
        }
    }
    dirs
}

/// The rebuild-relevant files directly inside a directory subtree (pruned like the watch
/// set). Used to backfill the watcher when a NEW directory appears with files already
/// inside it: those files were created before the directory's watch was registered, so
/// their create events were missed (notify's recursive mode used to emit them
/// automatically). The caller signals a rebuild if this is non-empty.
pub(crate) fn subtree_relevant_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in watch_tree(root) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) && relevant_path(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// Directories to watch (non-recursively, one inotify descriptor each): every directory
/// under the project base dir except the pruned generated/VCS trees, so a large
/// `node_modules`/`.git` can't exhaust the watch budget. A NEW in-tree subdirectory
/// created after startup is picked up dynamically in [`spawn_watcher`]. An include that
/// resolves OUTSIDE the base dir (a sibling file up the tree) can't be covered by the
/// base-dir walk and the watch set is fixed at startup, so we still register its dir
/// (so an out-of-tree include present now keeps refreshing) but warn once that an
/// out-of-tree sibling added later needs a manual reload — `relevant_path` still filters
/// every event.
fn watch_dirs(app: &AppState) -> Vec<PathBuf> {
    let mut dirs: HashSet<PathBuf> = watch_tree(&app.base_dir).into_iter().collect();
    if let Ok(src) = std::fs::read_to_string(&app.path) {
        for dep in taliesin_core::includes::dependencies(&src, &app.base_dir) {
            // In-tree includes are already covered by the base-dir walk.
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
    // Static lints (broken links, missing assets/media, dup ids, dangling anchors,
    // a11y, ...) on PRE-EXEC blocks, so a cell-generated figure isn't linted for alt
    // text. `src` is re-read (render_doc doesn't expose it); `base` is the doc's dir.
    let static_diags = {
        let src = std::fs::read_to_string(&app.path).unwrap_or_default();
        let base = app.path.parent().unwrap_or_else(|| Path::new("."));
        crate::preview_diag::static_diagnostics(
            &src,
            &doc.blocks,
            base,
            doc.format,
            crate::check::Scope::Standalone,
        )
    };
    let blocks = executor.run(doc.blocks).await;
    let mut diags = compute_diagnostics(app, executor);
    diags.extend(static_diags);
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
        // Detect a slide-restructuring edit on the RAW block diff: a `---`/`. . .` insert
        // is dropped from the slide-transformed projection below, so the projection can't
        // see it. A deck re-mounts fully when an insert/remove touches a slide boundary
        // (heading / `---` / `. . .`) or an update retitles a slide heading — its
        // `<section>`-grouped slides can't be restructured by flat block ops. Other
        // within-slide edits stay incremental. Computed before `d.blocks` is replaced, so
        // a Remove/Update can look up the old block's html.
        let raw_ops = taliesin_core::diff_blocks(&d.blocks, &blocks);
        let deck_structural = matches!(doc.format, DocFormat::Reveal)
            && raw_ops
                .iter()
                .any(|op| deck_op_is_structural(op, &d.blocks));
        // The ops the client applies incrementally: for a deck, diff the slide-transformed
        // projection (pause markers dropped, post-pause blocks carry `.fragment`) so a
        // within-slide text edit of a post-pause block ships the transformed html rather
        // than raw html that would strip its `.fragment`. A structural change re-mounts
        // (full_render), so these ops are unused in that case.
        let ops = if matches!(doc.format, DocFormat::Reveal) {
            taliesin_core::diff_blocks(
                &taliesin_core::deck_slide_blocks(&d.blocks),
                &taliesin_core::deck_slide_blocks(&blocks),
            )
        } else {
            raw_ops
        };
        // A deck's title slide is built from the front-matter title/subtitle (in
        // `slides_html`), *outside* `doc.blocks`, so retitling produces no block op and
        // the diff is empty. Force a full re-mount for a deck when either changes so the
        // title slide actually updates; the deck's JS preserves the current slide +
        // overview across the swap, exactly as it does for a structural change.
        let deck_meta_changed =
            deck_meta_changed(doc.format, &d.title, &d.subtitle, &doc.title, &doc.subtitle);
        // The tab title, for any format. A deck re-mounts (above) and its `full_render`
        // retitles the tab on the way through, so this only ever fires for a non-deck doc,
        // whose title reaches the tab through no other channel.
        let title_changed = d.title != doc.title;
        let diags_changed = d.diagnostics != diags;
        let theme_changed = d.theme_css != doc.theme_css;
        d.title = doc.title;
        d.subtitle = doc.subtitle;
        d.footer = doc.footer;
        d.logo = doc.logo;
        d.format = doc.format;
        d.toc = doc.toc;
        d.theme_css = doc.theme_css;
        d.theme_default = doc.theme_default;
        d.theme_is_custom = doc.theme_is_custom;
        d.includes = doc.includes;
        d.warnings = doc.warnings;
        // Bump the render generation when the pushed content actually changed, so a
        // no-op rebuild leaves a fresh SSR page's skip-the-remount check valid, while the
        // initial exec pass (which splices in output blocks) bumps it and forces any
        // client that server-rendered pre-exec to mount the outputs. A deck title/
        // subtitle edit changes the title slide (built outside `d.blocks`, so the diff is
        // empty) yet still needs a bump — otherwise its full_render carries an unchanged
        // gen and the client's reconnect-skip (same gen ⇒ byte-identical) would wrongly
        // suppress the re-mount that applies the new title slide.
        // `deck_structural` is checked too: a `---`/`. . .` insert re-mounts but produces
        // an empty slide-transformed `ops` (the marker/break is dropped from the
        // projection), so without this the gen wouldn't bump and the re-mount's full_render
        // would be suppressed as a same-gen no-op.
        if !ops.is_empty() || deck_meta_changed || deck_structural {
            d.generation = d.generation.wrapping_add(1);
        }
        d.blocks = blocks;
        d.diagnostics = diags;
        // Re-mount fully when recovering from an error (so every client clears its
        // overlay), a deck changed structurally, or a deck's title/subtitle changed (its
        // title slide lives outside the block model). The deck preserves its current
        // slide + overview across the swap (its JS state survives the rebuild). The
        // broadcast sequencing (body, then theme, then diagnostics — theme/diags after
        // the body even on a re-mount) is the shared contract in `protocol::Broadcast`.
        let generation = d.generation;
        let messages = protocol::Broadcast {
            ops: &ops,
            remount: recovered || deck_structural || deck_meta_changed,
            title_changed,
            theme_changed,
            diags_changed,
        }
        .messages(
            || full_render_json(&d),
            |op| op_json(op, generation),
            || protocol::title(d.title.as_deref()),
            || protocol::style(&d.theme_css),
            || protocol::diagnostics(&d.diagnostics),
        );
        // Broadcast under the lock so connecting clients can't interleave.
        for m in messages {
            let _ = app.tx.send(m);
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

/// One wording for a bad `--format` value, shared by every subcommand that takes
/// `--format`/`--json` (`build`/`publish`/`check`/`doctor`/`map`/`symbols`/`init`/`new`)
/// so the same mistake reads identically everywhere. `got` is the offending value, or
/// `None` when `--format` was given with nothing after it. No `error:` prefix — the caller
/// frames it exactly like `unknown_flag_error` (raw `eprintln!`, or `log::error` styles it).
pub(crate) fn bad_format_error(got: Option<&str>) -> String {
    format!(
        "--format expects human or json (got {})",
        got.unwrap_or("nothing")
    )
}

#[cfg(test)]
mod protocol_contract {
    //! The single-doc producers share the op/message contract that the preview
    //! client (web-client/client.js) consumes; the comprehensive shape test lives
    //! in serve_site.rs. This guards serve.rs's own `*_json` against drift.
    use super::*;
    use crate::testutil::parse;

    #[test]
    fn deck_structural_predicate_covers_heading_hr_pause_and_retitle() {
        // B3-13/B3-16: a deck re-mounts fully when an edit restructures its <section>s.
        // A slide boundary is not only a heading: a `---` (rendered `<hr .../>`) splits a
        // slide, and a `. . .` pause paragraph regroups the following blocks into fragments.
        assert!(is_slide_structural("<h2 data-block-id=\"a\">S</h2>"));
        assert!(is_slide_structural("<hr data-block-id=\"b\" />"));
        assert!(is_slide_structural("<p data-block-id=\"c\">. . .</p>"));
        assert!(!is_slide_structural("<p data-block-id=\"d\">Body.</p>"));

        let blk = |id: &str, html: &str| Block {
            id: id.into(),
            sourcepos: "1:1-1:3".into(),
            source_file: None,
            html: html.into(),
            cell: None,
        };
        let old = vec![blk("h_old", "<h2 data-block-id=\"h_old\">Old Title</h2>")];
        let ins = |html: &str| BlockOp::Insert {
            after_id: None,
            html: html.into(),
        };
        // Inserting a `---` / `. . .` / heading restructures; a plain paragraph doesn't.
        assert!(deck_op_is_structural(&ins("<hr />"), &old));
        assert!(deck_op_is_structural(&ins("<p>. . .</p>"), &old));
        assert!(!deck_op_is_structural(&ins("<p>plain</p>"), &old));
        // Retitling a slide (Update of a heading block) re-slugs its <section> id, whose
        // anchor lives on the wrapper not the swapped-in <h2> — so it must re-mount.
        assert!(deck_op_is_structural(
            &BlockOp::Update {
                target_id: "h_old".into(),
                html: "<h2 data-block-id=\"h_new\">New Title</h2>".into(),
            },
            &old,
        ));
        // A within-slide content edit (Update of a paragraph) stays incremental.
        let old_p = vec![blk("p1", "<p data-block-id=\"p1\">Old.</p>")];
        assert!(!deck_op_is_structural(
            &BlockOp::Update {
                target_id: "p1".into(),
                html: "<p data-block-id=\"p2\">New.</p>".into(),
            },
            &old_p,
        ));
        // Editing a paragraph IN PLACE into a `---` or `. . .` (or back) splits/merges a
        // slide — an Update, not an Insert/Remove — so it must also re-mount, or the live
        // view diverges from a full render (the projection diff can't restructure sections).
        assert!(deck_op_is_structural(
            &BlockOp::Update {
                target_id: "p1".into(),
                html: "<hr data-block-id=\"h\" />".into(),
            },
            &old_p,
        )); // para -> ---
        assert!(deck_op_is_structural(
            &BlockOp::Update {
                target_id: "p1".into(),
                html: "<p data-block-id=\"h\">. . .</p>".into(),
            },
            &old_p,
        )); // para -> pause
        let old_hr = vec![blk("hr1", "<hr data-block-id=\"hr1\" />")];
        assert!(deck_op_is_structural(
            &BlockOp::Update {
                target_id: "hr1".into(),
                html: "<p data-block-id=\"x\">now text</p>".into(),
            },
            &old_hr,
        )); // --- -> para (merge)
        // Removing a slide heading restructures; removing a paragraph doesn't.
        assert!(deck_op_is_structural(
            &BlockOp::Remove {
                target_id: "h_old".into()
            },
            &old
        ));
        assert!(!deck_op_is_structural(
            &BlockOp::Remove {
                target_id: "p1".into()
            },
            &old_p
        ));
    }

    #[test]
    fn deck_meta_changed_forces_remount_only_for_a_decks_title_or_subtitle() {
        // B3-15: a deck's title slide is built from the front-matter title/subtitle, OUTSIDE
        // doc.blocks, so a retitle yields an empty block diff and must force a full re-mount.
        let s = |x: &str| Some(x.to_string());
        // A deck whose title changed re-mounts...
        assert!(deck_meta_changed(
            DocFormat::Reveal,
            &s("Old"),
            &s("Sub"),
            &s("New"),
            &s("Sub")
        ));
        // ...and whose SUBTITLE changed (title unchanged) re-mounts too (an easy term to drop).
        assert!(deck_meta_changed(
            DocFormat::Reveal,
            &s("T"),
            &s("Old sub"),
            &s("T"),
            &s("New sub")
        ));
        // Adding or clearing a title (None <-> Some) counts as a change.
        assert!(deck_meta_changed(
            DocFormat::Reveal,
            &None,
            &None,
            &s("Added"),
            &None
        ));
        // No change -> no forced re-mount (the block diff alone drives the update).
        assert!(!deck_meta_changed(
            DocFormat::Reveal,
            &s("T"),
            &s("Sub"),
            &s("T"),
            &s("Sub")
        ));
        // A regular HTML page carries its title as an ordinary block, so a title edit there
        // must NOT force a deck-style re-mount — the format gate is load-bearing.
        assert!(!deck_meta_changed(
            DocFormat::Html,
            &s("Old"),
            &None,
            &s("New"),
            &None
        ));
    }

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
    fn watch_tree_prunes_generated_and_vcs_subtrees() {
        // notify's Recursive mode adds an inotify watch descriptor for EVERY directory
        // under the root; a big `node_modules` alone can exhaust `max_user_watches` and
        // silently kill hot reload. `watch_tree` must enumerate only the directories a
        // rebuild can depend on, pruning generated/VCS trees whole.
        let root = std::env::temp_dir().join(format!("tali-watchtree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in [
            "sub",
            "sub/deep",
            "node_modules/pkg",
            ".git/objects",
            "_site/assets",
            "_freeze",
        ] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let dirs: HashSet<PathBuf> = watch_tree(&root).into_iter().collect();
        assert!(dirs.contains(&root), "the base dir itself must be watched");
        assert!(dirs.contains(&root.join("sub")));
        assert!(dirs.contains(&root.join("sub/deep")));
        // The pruned trees (and everything under them) must be absent.
        assert!(!dirs.contains(&root.join("node_modules")));
        assert!(!dirs.contains(&root.join("node_modules/pkg")));
        assert!(!dirs.contains(&root.join(".git")));
        assert!(!dirs.contains(&root.join(".git/objects")));
        assert!(!dirs.contains(&root.join("_site")));
        assert!(!dirs.contains(&root.join("_site/assets")));
        assert!(!dirs.contains(&root.join("_freeze")));
        let _ = std::fs::remove_dir_all(&root);
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
    fn client_js_ships_the_preview_command_palette_hooks() {
        // DX8: the Cmd-K palette's preview-only actions (restart kernel, open source in the
        // editor) call these globals, which live in the PREVIEW client (client.js) — so they
        // exist only in a live preview, never a static build. JS is include_str!'d; this is
        // the drift guard that the hooks still ship.
        assert!(
            CLIENT_JS.contains("window.taliRestartKernel"),
            "client.js must expose window.taliRestartKernel for the command palette"
        );
        assert!(
            CLIENT_JS.contains("window.taliOpenPageSource"),
            "client.js must expose window.taliOpenPageSource for the command palette"
        );
    }

    #[test]
    fn client_and_status_css_ship_the_cache_legibility_surface() {
        // DX9: the ⚡ cached badge + the muted cached-cell border are include_str!'d JS/CSS,
        // so this drift guard keeps the render and the style in lockstep with the protocol's
        // `source: "cache"` tag. If the badge text or the CSS attr hook is renamed, the wire
        // stays "cache" and the surface goes silently blank — this fails first.
        assert!(
            CLIENT_JS.contains("⚡ cached"),
            "client.js must render the ⚡ cached badge for a cache replay"
        );
        assert!(
            CLIENT_JS.contains("data-qmd-cell-source"),
            "client.js must tag the block with its cache provenance"
        );
        assert!(
            STATUS_CSS.contains("data-qmd-cell-source=\"cache\""),
            "STATUS_CSS must style the cached-cell border distinctly from a fresh run"
        );
    }

    #[test]
    fn client_ships_the_card_preview_pane_hitting_the_identity_route() {
        // DX13: the dev-menu card pane fetches the current page's card from the identity-
        // keyed route, gated on the site preview's page global. If the route path or the
        // gate global is renamed on one side, the pane silently 404s / never appears — this
        // pins the client half against the serve_site `/og-preview` handler + STATUS_CSS.
        assert!(
            CLIENT_JS.contains("/og-preview?page="),
            "client.js must fetch the card from the /og-preview identity route"
        );
        assert!(
            CLIENT_JS.contains("TALIESIN_WS_PATH"),
            "client.js must gate the card pane on the site-preview page identity"
        );
        assert!(
            STATUS_CSS.contains(".tali-dev-card"),
            "STATUS_CSS must style the card-preview image"
        );
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
            footer: None,
            logo: None,
            generation: 0,
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
                footer: None,
                logo: None,
                generation: 0,
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
    fn preview_page_ships_first_run_hint_css_and_mount() {
        // DX2: the first-run nudge is built by client.js into #tali-controls and styled by
        // STATUS_CSS. Pin both on the assembled single-doc preview page so a future edit can't
        // silently drop the style (the nudge would then render unstyled) or the mount host.
        let includes = taliesin_core::render::PageIncludes::default();
        let ctx = PageCtx {
            format: DocFormat::Html,
            toc: false,
            theme_css: "",
            theme_default: "auto",
            theme_is_custom: false,
            doc_path: "/tmp/doc.tmd",
            base_dir: "/tmp",
            includes: &includes,
            body: "<h2 data-block-id=\"b\">S</h2>",
            footer: None,
            logo: None,
            generation: 0,
        };
        let html = blog_index_html(&ctx);
        assert!(
            html.contains(".tali-hint-nudge"),
            "preview page head must ship the first-run nudge CSS (STATUS_CSS)"
        );
        assert!(
            html.contains("id=\"tali-controls\""),
            "preview page must ship the #tali-controls host the nudge mounts into"
        );
    }

    #[test]
    fn ops_and_full_render_match_client_contract() {
        let up = parse(op_json(
            &BlockOp::Update {
                target_id: "b".into(),
                html: "h".into(),
            },
            3,
        ));
        assert_eq!(up["type"], "update");
        assert_eq!(up["target_id"], "b");
        assert!(up.get("html").is_some());
        // The resulting render generation rides on every op so the client can track it
        // and skip the destructive re-mount on a byte-identical reconnect.
        assert_eq!(up["gen"], 3);

        let ins = parse(op_json(
            &BlockOp::Insert {
                after_id: Some("b".into()),
                html: "h".into(),
            },
            3,
        ));
        assert_eq!(ins["type"], "insert");
        assert!(ins.get("after_id").is_some());
        assert_eq!(ins["gen"], 3);

        let rm = parse(op_json(
            &BlockOp::Remove {
                target_id: "b".into(),
            },
            3,
        ));
        assert_eq!(rm["type"], "remove");
        assert_eq!(rm["gen"], 3);

        let fr = parse(full_render_json(&DocState::default()));
        assert_eq!(fr["type"], "full_render");
        assert!(fr.get("body_html").is_some());
        assert!(fr["gen"].is_u64(), "full_render must carry a numeric gen");
        // The per-process boot id lets the client force a re-mount on a reconnect to a
        // restarted server (whose gen counter reset), not skip it and show stale source.
        assert!(
            fr["boot"].is_u64(),
            "full_render must carry a numeric boot id"
        );
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

#[cfg(test)]
mod percent_decode_tests {
    use super::percent_decode;

    #[test]
    fn decodes_ascii_and_multibyte_escapes() {
        assert_eq!(percent_decode("%20"), " ");
        assert_eq!(percent_decode("a%2Fb"), "a/b");
        // A percent-encoded multi-byte char round-trips.
        assert_eq!(percent_decode("caf%C3%A9"), "café");
    }

    #[test]
    fn does_not_panic_on_percent_before_raw_multibyte_char() {
        // A `%` immediately followed by a *raw* (un-encoded) multi-byte UTF-8 char used
        // to slice `&s[i+1..i+3]` across a char boundary and panic — a crafted request
        // path like `GET /%€` would crash the handler. Decoding must operate on bytes
        // so it can't split a char: the un-decodable `%` is left literal.
        assert_eq!(percent_decode("%€"), "%€");
        assert_eq!(percent_decode("a%€b"), "a%€b");
        // A trailing `%` with fewer than two following bytes must not decode or panic.
        assert_eq!(percent_decode("%9"), "%9");
        assert_eq!(percent_decode("done%"), "done%");
        // A `%` followed by a non-hex ASCII pair is left literal (not mis-parsed).
        assert_eq!(percent_decode("%zz"), "%zz");
    }
}
