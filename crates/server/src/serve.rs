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
use qmd_fast_core::{Block, BlockOp, DocFormat, RenderedDoc};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

pub(crate) const CLIENT_JS: &str = include_str!("../../../web-client/client.js");
/// The preview tab's favicon (an original block-model mark; SVG, so it's tiny
/// and self-contained).
pub(crate) const FAVICON: &str = include_str!("../../../web-client/favicon.svg");

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
    includes: qmd_fast_core::render::PageIncludes,
    /// Non-fatal render warnings (missing `bibliography:`/`theme:` file), surfaced
    /// in the dev menu + terminal.
    warnings: Vec<qmd_fast_core::render::Warning>,
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

/// Entry point for `qmd-fast serve <file> [port] [--open]`.
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
    });

    // Initial render.
    if let Some(doc) = render_doc(&app) {
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
    crate::log::banner(qmd_fast_core::VERSION);
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

/// Whether a websocket upgrade carrying this `Origin` may connect, given the
/// request's `Host`. The control channel (restart kernel, etc.) lives on the
/// websocket, so a page on another site must not be able to open it against your
/// dev server (a browser always sends `Origin`, so this blocks cross-site driving
/// without affecting non-browser clients, which send none).
pub(crate) fn origin_allowed(origin: Option<&str>, host: Option<&str>) -> bool {
    let Some(origin) = origin else {
        return true; // no Origin => not a browser => not a cross-site request
    };
    // The part after the scheme is the authority (host[:port]).
    let authority = origin.split_once("://").map_or(origin, |(_, rest)| rest);
    if Some(authority) == host {
        return true; // same origin (covers the LAN case: phone dials the Host it sees)
    }
    let host_only = authority.split(':').next().unwrap_or("");
    matches!(host_only, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

/// Apply [`origin_allowed`] to a request's headers; both websocket handlers gate
/// the upgrade on this.
pub(crate) fn ws_origin_ok(headers: &axum::http::HeaderMap) -> bool {
    use axum::http::header::{HOST, ORIGIN};
    let origin = headers.get(ORIGIN).and_then(|v| v.to_str().ok());
    let host = headers.get(HOST).and_then(|v| v.to_str().ok());
    origin_allowed(origin, host)
}

// --- LAN access token (`--host` only) -------------------------------------------
// `--host` binds 0.0.0.0, so anyone on the LAN can reach the preview. A per-session
// token, carried in the QR / printed LAN URL, gates *non-loopback* access: a snooper
// on the same network can't read your draft or drive the control channel without it.
// Loopback (the author's own machine; the localhost VS Code companion) is always
// allowed and never needs the token, so the local workflow is unchanged. Without
// `--host` there is no token and the guard is never installed at all.

/// A fresh per-session access token (a random UUID).
pub(crate) fn new_session_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// What the [`lan_token_guard`] does with a request.
pub(crate) enum LanAccess {
    /// Serve it (loopback peer, or a valid `qmd_token` cookie already present).
    Allow,
    /// Serve it and set the session cookie (a valid `?t=` token — e.g. the first load
    /// from the QR), so later same-origin asset/ws requests authenticate by cookie and
    /// no longer need the token in the URL.
    AllowSetCookie,
    /// Reject: a non-loopback peer with no/incorrect token.
    Deny,
}

/// The `t=` value of a URL query string, if present.
fn query_token(query: &str) -> Option<&str> {
    query.split('&').find_map(|kv| kv.strip_prefix("t="))
}

/// The `qmd_token` value of a `Cookie` header, if present.
fn cookie_token(cookie: &str) -> Option<&str> {
    cookie
        .split(';')
        .find_map(|kv| kv.trim_start().strip_prefix("qmd_token="))
}

/// Decide LAN access for one request. Loopback is always allowed; a LAN peer must
/// present the token in the `?t=` query (→ set a cookie) or the `qmd_token` cookie.
pub(crate) fn lan_access(
    peer_loopback: bool,
    query: Option<&str>,
    cookie: Option<&str>,
    token: &str,
) -> LanAccess {
    if peer_loopback {
        return LanAccess::Allow;
    }
    if query.and_then(query_token) == Some(token) {
        return LanAccess::AllowSetCookie;
    }
    if cookie.and_then(cookie_token) == Some(token) {
        return LanAccess::Allow;
    }
    LanAccess::Deny
}

/// Axum middleware enforcing [`lan_access`]. Installed on the router only when a token
/// exists (i.e. `--host`), so a loopback-only preview keeps its exact prior behavior.
/// Reads the peer address from `ConnectInfo`, so the router must be served with
/// `into_make_service_with_connect_info::<SocketAddr>()`.
pub(crate) async fn lan_token_guard(
    token: Arc<str>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let peer_loopback = req
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .is_some_and(|ci| ci.0.ip().is_loopback());
    let query = req.uri().query().map(str::to_owned);
    let cookie = req
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    match lan_access(peer_loopback, query.as_deref(), cookie.as_deref(), &token) {
        LanAccess::Deny => (
            axum::http::StatusCode::FORBIDDEN,
            "qmd-fast: this --host preview needs its session link. Scan the QR code or \
             open the printed LAN URL (it carries the access token).",
        )
            .into_response(),
        LanAccess::Allow => next.run(req).await,
        LanAccess::AllowSetCookie => {
            let mut resp = next.run(req).await;
            // Session-scoped: a new token each server start, so a stale cookie just
            // fails closed and the author re-scans. SameSite=Lax + Path=/ so the cookie
            // rides every same-origin asset/ws request from the page.
            let value = format!("qmd_token={token}; Path=/; SameSite=Lax; Max-Age=86400");
            if let Ok(hv) = axum::http::HeaderValue::from_str(&value) {
                resp.headers_mut()
                    .append(axum::http::header::SET_COOKIE, hv);
            }
            resp
        }
    }
}

/// Wrap a router with the LAN token guard for a `--host` session. Returns the router
/// unchanged when there is no token (loopback-only preview).
pub(crate) fn with_lan_guard(router: Router, token: Option<Arc<str>>) -> Router {
    match token {
        None => router,
        Some(token) => router.layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let token = token.clone();
                async move { lan_token_guard(token, req, next).await }
            },
        )),
    }
}

/// The LAN URL to advertise (QR + console): the base plus the session token in `?t=`
/// when one exists, so the first load authenticates and sets the cookie.
pub(crate) fn lan_url(base: &str, token: Option<&Arc<str>>) -> String {
    match token {
        Some(t) => format!("{base}/?t={t}"),
        None => base.to_string(),
    }
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
    Some(qmd_fast_core::render_document_with_includes(
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
    includes: &'a qmd_fast_core::render::PageIncludes,
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

/// Serve a static file a page references: first a plain file under `base`, else a
/// format extension's resource (looked up by file name in `_extensions/*/`, so an
/// injected `<script src="plugin.js">` resolves in preview the same way the build
/// copies it next to the page). Shared by the single-doc and site servers.
pub(crate) fn serve_asset_from(base: &Path, rel: &str) -> axum::response::Response {
    use axum::http::{StatusCode, header};
    let not_found = || (StatusCode::NOT_FOUND, "not found").into_response();
    let serve = |full: &Path| match std::fs::read(full) {
        Ok(bytes) => ([(header::CONTENT_TYPE, content_type(full))], bytes).into_response(),
        Err(_) => not_found(),
    };
    if let (Ok(root), Ok(full)) = (base.canonicalize(), base.join(rel).canonicalize())
        && full.starts_with(&root)
        && full.is_file()
    {
        return serve(&full);
    }
    match find_in_extensions(base, rel) {
        Some(p) => serve(&p),
        None => not_found(),
    }
}

/// Find a file by its bare name anywhere under `base/_extensions/` (where a
/// format extension's `format-resources` live, possibly in a subdir like
/// `assets/`). `build` copies those resources flat next to the page, so the
/// preview must resolve a bare `<script src="x.js">` regardless of which subdir
/// it sits in. Only the file name is used, so a path can't traverse out.
fn find_in_extensions(base: &Path, rel: &str) -> Option<PathBuf> {
    let name = Path::new(rel).file_name()?;
    find_file_named(&base.join("_extensions"), name)
}

/// Depth-first search for a file named `name` under `dir`: files at each level
/// are matched before descending, so a top-level resource still wins.
fn find_file_named(dir: &Path, name: &std::ffi::OsStr) -> Option<PathBuf> {
    fn walk(
        dir: &Path,
        name: &std::ffi::OsStr,
        seen: &mut std::collections::HashSet<PathBuf>,
    ) -> Option<PathBuf> {
        // Visit each directory at most once (by canonical path) so a symlink cycle
        // can't recurse forever.
        if let Ok(canon) = dir.canonicalize()
            && !seen.insert(canon)
        {
            return None;
        }
        let mut subdirs = Vec::new();
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                subdirs.push(path);
            } else if path.file_name() == Some(name) {
                return Some(path);
            }
        }
        subdirs.iter().find_map(|sub| walk(sub, name, seen))
    }
    walk(dir, name, &mut std::collections::HashSet::new())
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
    #qmd-controls.qmd-dev { position: fixed; bottom: .6rem; left: .6rem; z-index: 9999; \
      font: 12px ui-sans-serif, system-ui, sans-serif; } \
    .qmd-dev-toggle { display: inline-flex; align-items: center; gap: .4rem; cursor: pointer; \
      background: var(--qmd-bg, #fff); color: var(--qmd-muted, #888); \
      border: 1px solid var(--qmd-border, #e0e0e0); border-radius: 999px; padding: .25rem .6rem; \
      box-shadow: 0 1px 6px rgba(0,0,0,.12); } \
    .qmd-dev-toggle:hover { color: var(--qmd-fg, #111); } \
    .qmd-dev-toggle.qmd-dev-alert { border-color: #d9a23a; color: #d9a23a; } \
    .qmd-dev-glyph { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; letter-spacing: -1px; } \
    .qmd-dev-count { min-width: 1rem; padding: 0 .3rem; border-radius: 999px; background: #d9a23a; color: #fff; \
      font-weight: 700; font-size: 11px; line-height: 1.3; text-align: center; } \
    .qmd-dev-count[hidden] { display: none; } \
    .qmd-dev-dot { width: .5rem; height: .5rem; border-radius: 50%; background: var(--qmd-muted, #888); flex: none; } \
    .qmd-dev-dot[data-state=\"live\"] { background: #3fb950; } \
    .qmd-dev-dot[data-state=\"warn\"] { background: #d9a23a; } \
    .qmd-dev-dot[data-state=\"error\"] { background: #e5534b; } \
    .qmd-dev-panel { position: absolute; bottom: calc(100% + .45rem); left: 0; min-width: 13rem; \
      display: flex; flex-direction: column; gap: .5rem; padding: .65rem; \
      background: var(--qmd-bg, #fff); color: var(--qmd-fg, #111); \
      border: 1px solid var(--qmd-border, #e0e0e0); border-radius: 9px; box-shadow: 0 8px 28px rgba(0,0,0,.2); } \
    .qmd-dev-panel[hidden] { display: none; } \
    .qmd-dev-row { display: flex; justify-content: space-between; gap: 1rem; color: var(--qmd-muted, #888); } \
    .qmd-dev-row .qmd-dev-label { font-weight: 600; } \
    #qmd-wordcount { font-variant-numeric: tabular-nums; } \
    .qmd-dev-ctl { display: inline-flex; align-items: center; gap: .4rem; text-align: left; cursor: pointer; \
      background: var(--qmd-code-bg, #f5f5f5); color: var(--qmd-fg, #111); \
      border: 1px solid var(--qmd-border, #e0e0e0); border-radius: 6px; padding: .3rem .55rem; } \
    .qmd-dev-ctl:hover { border-color: var(--qmd-accent, #4c8dff); } \
    .qmd-dev-theme svg { width: 14px; height: 14px; } \
    #qmd-diagnostics { display: none; flex-direction: column; gap: .3rem; max-width: 22rem; } \
    #qmd-diagnostics .qmd-diag { padding: .3rem .5rem; border-radius: 6px; background: var(--qmd-code-bg, #f5f5f5); \
      border: 1px solid var(--qmd-border, #e0e0e0); line-height: 1.35; } \
    #qmd-diagnostics .qmd-diag-error { border-left: 3px solid #e5534b; } \
    #qmd-diagnostics .qmd-diag-warning { border-left: 3px solid #d9a23a; } \
    #qmd-diagnostics .qmd-diag-loc { cursor: pointer; text-align: left; width: 100%; font: inherit; color: inherit; } \
    #qmd-diagnostics .qmd-diag-loc:hover { border-color: var(--qmd-accent, #4c8dff); } \
    #qmd-diagnostics .qmd-diag-loc::after { content: \"  \\2192 source\"; color: var(--qmd-muted, #888); font-size: 11px; } \
    #qmd-diagnostics .qmd-diag-frame { margin: .35rem 0 0; padding: .35rem .45rem; border-radius: 4px; overflow-x: auto; \
      background: var(--qmd-bg, #fff); white-space: pre; font: 11px/1.45 ui-monospace, SFMono-Regular, Menlo, monospace; } \
    #qmd-cell-errors { flex-direction: column; gap: .3rem; max-width: 22rem; } \
    .qmd-cellerr { text-align: left; cursor: pointer; font: 12px ui-sans-serif, system-ui, sans-serif; \
      color: var(--qmd-fg, #111); background: var(--qmd-code-bg, #f5f5f5); border: 1px solid var(--qmd-border, #e0e0e0); \
      border-left: 3px solid #e5534b; border-radius: 6px; padding: .3rem .5rem; \
      white-space: nowrap; overflow: hidden; text-overflow: ellipsis; } \
    .qmd-cellerr:hover { border-color: #e5534b; } \
    @media (max-width: 60rem) { body.qmd-toc-sheet #qmd-controls.qmd-dev { bottom: 2.4rem; } }";

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
    // `QMD_TOC` flag switches the client into that mode. `qmd-toc-sheet` opts the
    // live page into the mobile pull-up-sheet TOC (the static export keeps the
    // plain stacked-top TOC).
    let (body_class, toc_nav, toc_flag) = if ctx.toc {
        (
            " class=\"has-toc qmd-toc-sheet\"",
            "<nav id=\"TOC\"></nav>\n\
             <div id=\"qmd-toc-backdrop\"></div>\n\
             <button id=\"qmd-toc-handle\" type=\"button\" aria-label=\"Contents\">\
             <span id=\"qmd-toc-cur\"></span><span class=\"qmd-toc-grip\"></span></button>",
            "window.QMD_TOC = true;",
        )
    } else {
        ("", "", "")
    };
    // Absolute paths so click-to-source can build `vscode://file/…` links.
    let doc_global = format!(
        "window.QMD_DOC = {{ path: \"{}\", baseDir: \"{}\" }};",
        js_str(ctx.doc_path),
        js_str(ctx.base_dir),
    );
    // The live body: a mountable `#qmd-root`, the live TOC nav, and the dev-menu
    // mount. The websocket client drives everything after the first paint.
    let body = format!(
        "<main id=\"qmd-root\">{}</main>\n{toc_nav}\n<div id=\"qmd-controls\"></div>",
        ctx.body
    );
    let extra_head = format!("<style>{STATUS_CSS}</style>\n");
    let scripts_pre = format!("<script>{doc_global} {toc_flag} window.QMD_SSR = true;</script>");
    let scripts_post = format!("<script>\n{CLIENT_JS}\n</script>");
    qmd_fast_core::assemble_html_page(&qmd_fast_core::PageParts {
        title: "qmd-fast",
        // The preview page chrome is English ("qmd-fast"); the built artifact honours
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
/// `.qmd-deck > .qmd-slides` and (re)syncing the deck engine as blocks change. The
/// `QMD_FORMAT` flag switches the client into deck mode.
fn deck_index_html(ctx: &PageCtx) -> String {
    let extra_head = format!("<style>{STATUS_CSS}</style>\n");
    // Absolute paths so click-to-source can build `vscode://file/…` links. The
    // single-doc page sets this in its scripts_pre; the deck has none, so the tail
    // carries it — without it, `openSource` bails (no QMD_DOC) and click-to-source
    // silently does nothing on slides.
    let doc_global = format!(
        "window.QMD_DOC = {{ path: \"{}\", baseDir: \"{}\" }};",
        js_str(ctx.doc_path),
        js_str(ctx.base_dir),
    );
    // The live deck tail: the deck engine, the enhancers, the `QMD_*` flags, the
    // doc's after-body include (an extension plugin's `<script src>` + registration,
    // which must run after the engine and before the client initializes it), then
    // the websocket client last.
    let tail = format!(
        "{deck_script}\n{code_scripts}\n\
         <script>{doc_global} window.QMD_FORMAT = \"deck\"; window.QMD_SSR = true;</script>\n\
         {include_after_body}\n<script>\n{CLIENT_JS}\n</script>\n",
        deck_script = qmd_fast_core::deck_client_script(),
        code_scripts = qmd_fast_core::code_scripts(),
        include_after_body = ctx.includes.after_body,
    );
    qmd_fast_core::assemble_deck_page(&qmd_fast_core::DeckParts {
        title: "qmd-fast",
        // The preview page chrome is English ("qmd-fast"); the built artifact honours
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
        slides_attr: " id=\"qmd-root\"",
        slides: ctx.body,
        // The dev-menu host (the floating `</>` button), same as the single-doc
        // page: `client.js`'s buildDevMenu fills it with the live status dot,
        // click-to-source toggle, and restart-kernel control. (Was a bare
        // `#qmd-status` node, which only showed an orphaned "live" label.)
        after_deck: "<div id=\"qmd-controls\"></div>\n",
        tail: &tail,
    })
}

// --- WebSocket ----------------------------------------------------------

async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    State(app): State<Arc<AppState>>,
) -> axum::response::Response {
    if !ws_origin_ok(&headers) {
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
        if let Some((message, line)) = qmd_fast_core::frontmatter::yaml_error(&src) {
            diags.push(
                Diagnostic::error(message)
                    .at(None, line)
                    .with_frame(code_frame(&src, line)),
            );
        }
        for dep in qmd_fast_core::includes::dependencies(&src, &app.base_dir) {
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
        let mut executor = crate::exec::Executor::with_freeze(freeze_path);
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
/// out the noise (editor swap files, `_site/`/`_book/` output, `.git`/`.quarto`)
/// that would otherwise trigger a wasteful 0-op rebuild on every unrelated save.
fn relevant_event(ev: &notify::Event) -> bool {
    ev.paths.iter().any(|p| relevant_path(p))
}

/// Whether a changed path should trigger a rebuild: a known source/asset extension,
/// not under a generated/VCS directory. `_freeze` is skipped so the executor's own
/// cache writes don't kick a redundant rebuild on every run.
pub(crate) fn relevant_path(p: &Path) -> bool {
    const EXTS: &[&str] = &[
        "qmd", "md", "bib", "csl", "css", "scss", "yml", "yaml", "json", "js", "html", "svg",
        "png", "jpg", "jpeg", "webp", "gif",
    ];
    const SKIP_DIRS: &[&str] = &[
        "_site",
        "_book",
        "_freeze",
        ".git",
        ".quarto",
        "node_modules",
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
    for w in qmd_fast_core::cite::validate_xrefs(&blocks) {
        let mut d = Diagnostic::warn(&w.message);
        if let Some(line) = w.line {
            d = d.at(w.file.clone(), line);
        }
        diags.push(d);
    }
    let ops = {
        let mut d = app.doc.lock();
        let recovered = std::mem::take(&mut d.errored);
        let ops = qmd_fast_core::diff_blocks(&d.blocks, &blocks);
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
            // CSS-only change (a theme/`.css` edit, content unchanged): hot-swap the
            // theme `<style>` in place instead of reloading, so scroll + the current
            // slide survive. Only when no block ops also went out this pass.
            if theme_changed && ops.is_empty() {
                let _ = app.tx.send(protocol::style(&d.theme_css));
            }
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

#[cfg(test)]
mod protocol_contract {
    //! The single-doc producers share the op/message contract that the preview
    //! client (web-client/client.js) consumes; the comprehensive shape test lives
    //! in serve_site.rs. This guards serve.rs's own `*_json` against drift.
    use super::*;
    use crate::testutil::parse;

    #[test]
    fn style_message_carries_css_for_hot_swap() {
        let m = parse(protocol::style(":root{--qmd-accent:#f00}"));
        assert_eq!(m["type"], "style");
        assert_eq!(m["css"], ":root{--qmd-accent:#f00}");
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
        // The deck page has no scripts_pre, so its tail must inject QMD_DOC — without
        // it, client.js's openSource bails (no doc) and click-to-source is dead on
        // slides, even though every block carries data-block-id/sourcepos.
        let includes = qmd_fast_core::render::PageIncludes::default();
        let ctx = PageCtx {
            format: DocFormat::Reveal,
            toc: false,
            theme_css: "",
            theme_default: "auto",
            theme_is_custom: false,
            doc_path: "/tmp/deck.qmd",
            base_dir: "/tmp",
            includes: &includes,
            body: "<section><h2>S</h2></section>",
        };
        let html = deck_index_html(&ctx);
        assert!(
            html.contains("window.QMD_DOC = { path: \"/tmp/deck.qmd\", baseDir: \"/tmp\" }"),
            "deck page must carry QMD_DOC for click-to-source"
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

#[cfg(test)]
mod extension_assets {
    //! The preview's `find_in_extensions` fallback resolves a format extension's
    //! `format-resources` by *bare file name*, mirroring how `build` copies them
    //! flat next to the output page.
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn tmp() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "qmd-srv-ext-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn resolves_top_level_resource_by_bare_name_and_strips_paths() {
        let root = tmp();
        fs::create_dir_all(root.join("_extensions/deck")).unwrap();
        fs::write(root.join("_extensions/deck/plugin.js"), "// x").unwrap();

        // A bare `<script src="plugin.js">` resolves into the extension dir.
        assert_eq!(
            find_in_extensions(&root, "plugin.js"),
            Some(root.join("_extensions/deck/plugin.js"))
        );
        // Only the file name is used, so a path can't traverse out.
        assert_eq!(
            find_in_extensions(&root, "../../etc/plugin.js"),
            Some(root.join("_extensions/deck/plugin.js"))
        );
        // A miss is None (not a panic).
        assert_eq!(find_in_extensions(&root, "missing.js"), None);

        // A *subdir* resource (e.g. `format-resources: [assets/x.css]`, which
        // `build` copies flat as `x.css`) resolves too, by bare name — so preview
        // and build agree on what an injected `src="x.css"` points at.
        fs::create_dir_all(root.join("_extensions/deck/assets")).unwrap();
        fs::write(root.join("_extensions/deck/assets/x.css"), "/* */").unwrap();
        assert_eq!(
            find_in_extensions(&root, "x.css"),
            Some(root.join("_extensions/deck/assets/x.css"))
        );

        let _ = fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod security {
    use super::*;

    #[test]
    fn origin_check_allows_same_origin_and_blocks_cross_site() {
        // No Origin header (curl / websocat — not a browser) can't be a cross-site
        // request, so it's allowed.
        assert!(origin_allowed(None, Some("localhost:4388")));
        // A same-origin browser connection is allowed.
        assert!(origin_allowed(
            Some("http://localhost:4388"),
            Some("localhost:4388")
        ));
        // The `--host` LAN case: the phone's Origin is the Host it dialed -> allowed.
        assert!(origin_allowed(
            Some("http://192.168.1.5:4388"),
            Some("192.168.1.5:4388")
        ));
        // Loopback is allowed regardless of port (a second local dev server).
        assert!(origin_allowed(
            Some("http://127.0.0.1:9999"),
            Some("localhost:4388")
        ));
        // The attack: a malicious page open in your browser tries to drive your dev
        // server's control channel. Blocked.
        assert!(!origin_allowed(
            Some("http://evil.example"),
            Some("localhost:4388")
        ));
        assert!(!origin_allowed(
            Some("https://evil.example:4388"),
            Some("192.168.1.5:4388")
        ));
        // A `null` origin (sandboxed iframe / file://) can't control the server.
        assert!(!origin_allowed(Some("null"), Some("localhost:4388")));
    }

    #[test]
    fn lan_token_gates_non_loopback_only() {
        let tok = "abc123";
        // The author's own machine (loopback peer) is always allowed, token or not —
        // so localhost browsing and the editor companion are unaffected by `--host`.
        assert!(matches!(
            lan_access(true, None, None, tok),
            LanAccess::Allow
        ));
        // A LAN peer with no token is rejected (the snooping defense).
        assert!(matches!(
            lan_access(false, None, None, tok),
            LanAccess::Deny
        ));
        // A LAN peer with the wrong token is rejected.
        assert!(matches!(
            lan_access(false, Some("t=nope"), Some("qmd_token=nope"), tok),
            LanAccess::Deny
        ));
        // First load from the QR carries `?t=<token>` -> allowed, and we set the cookie.
        assert!(matches!(
            lan_access(false, Some("t=abc123"), None, tok),
            LanAccess::AllowSetCookie
        ));
        // A `?t=` among other params still authenticates (e.g. `?page=x&t=abc123`).
        assert!(matches!(
            lan_access(false, Some("page=intro.qmd&t=abc123"), None, tok),
            LanAccess::AllowSetCookie
        ));
        // Subsequent same-origin asset/ws requests carry the cookie -> allowed.
        assert!(matches!(
            lan_access(false, None, Some("qmd_token=abc123"), tok),
            LanAccess::Allow
        ));
        // A cookie among other cookies still authenticates.
        assert!(matches!(
            lan_access(false, None, Some("other=1; qmd_token=abc123"), tok),
            LanAccess::Allow
        ));
    }

    #[test]
    fn lan_url_appends_token_only_when_present() {
        let tok: Arc<str> = Arc::from("abc123");
        assert_eq!(
            lan_url("http://192.168.1.5:4388", Some(&tok)),
            "http://192.168.1.5:4388/?t=abc123"
        );
        assert_eq!(
            lan_url("http://192.168.1.5:4388", None),
            "http://192.168.1.5:4388"
        );
    }

    #[test]
    fn session_tokens_are_unique() {
        assert_ne!(new_session_token(), new_session_token());
    }
}
