//! The dev server's **shared layer**: the pieces every serving path needs, and the
//! CLI-wide error helpers that grew up next to them.
//!
//! HTTP + asset plumbing (`serve_asset_from`, `content_type`, `percent_decode`), the
//! bundled client + favicon + dev-menu CSS, port binding and the single-instance takeover
//! probe, the LAN/host/identity guards in [`security`], the shutdown signal, the file-watch
//! predicates, and `guarded`/`panic_msg`/`unknown_flag_error`/`bad_format_error`.
//!
//! **There is no server here.** The live preview is [`crate::serve_site`], for a project
//! and for a single document alike — Wave 1.1 folded the single-document server away, since
//! serving one `.tmd` on its own produced an orphan page (no nav, no breadcrumb, dead
//! cross-page links) from a second, drifting copy of the same machinery. What is left is
//! the layer that copy shared with the site server, kept at this path so every
//! `crate::serve::` import across the crate still resolves.

use axum::Router;
use axum::response::IntoResponse;
use axum::routing::get;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
// `security` is a child module on `use super::*`, so its `Arc<str>` tokens resolve here.
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) const CLIENT_JS: &str = include_str!("../../../../web-client/client.js");
/// The preview tab's favicon (an original block-model mark; SVG, so it's tiny
/// and self-contained).
pub(crate) const FAVICON: &str = include_str!("../../../../web-client/favicon.svg");

mod security;
// Re-exported at `crate::serve::*` because serve_site.rs imports several of these.
pub(crate) use security::{
    lan_url, new_session_token, with_host_guard, with_lan_guard, ws_origin_ok,
};

async fn unix_signal(kind: tokio::signal::unix::SignalKind) {
    match tokio::signal::unix::signal(kind) {
        Ok(mut sig) => {
            sig.recv().await;
        }
        Err(_) => std::future::pending::<()>().await,
    }
}

/// Resolve when the process is asked to shut down: Ctrl-C (SIGINT), SIGTERM, or
/// SIGHUP. The two dev servers race their `axum::serve` against this so `serve`
/// **returns** on a signal (rather than the process being hard-killed with kernels
/// still live), letting `run` tear the runtime down and drop the watcher/builder
/// tasks that own the kernels + warm pool — which runs their teardown Drops. We race
/// (rather than `axum`'s `with_graceful_shutdown`) because the preview holds a
/// persistent websocket that never closes on its own, so graceful shutdown would
/// hang. Without this, a Ctrl-C'd preview leaks the whole kernel/forkserver subtree.
///
/// SIGHUP is here because closing a terminal tab is the most common way a dev server
/// dies, and its default disposition *terminates the process*, skipping this teardown
/// entirely and leaking exactly the subtree the SIGINT/SIGTERM paths are careful to
/// reap.
pub(crate) async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = unix_signal(tokio::signal::unix::SignalKind::terminate());
    #[cfg(unix)]
    let hangup = unix_signal(tokio::signal::unix::SignalKind::hangup());
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    #[cfg(not(unix))]
    let hangup = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
        _ = hangup => {}
    }
}

pub(crate) const IDENTITY_PATH: &str = "/__taliesin";

/// The path a session accepts run requests on (`taliesin run`'s only control seam).
///
/// Under `/__taliesin` so it shares the identity endpoint's namespace and can never
/// collide with a page path: a project is free to have a page called `run`.
pub(crate) const RUN_PATH: &str = "/__taliesin/run";

/// The path a session accepts interrupt requests on (`taliesin run --interrupt`, and the
/// Ctrl-C that a run installs for itself). Same namespace as [`RUN_PATH`], same reason.
pub(crate) const INTERRUPT_PATH: &str = "/__taliesin/interrupt";

/// What an interrupt request carries: which document's run to stop.
#[derive(serde::Deserialize)]
pub(crate) struct InterruptReq {
    pub(crate) file: String,
}

fn canonical(root: &Path) -> PathBuf {
    std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

/// Attach the identity route. The answer is fixed for the process's lifetime, so it is
/// rendered once here rather than per request.
pub(crate) fn with_identity(router: Router, root: &Path) -> Router {
    let body = serde_json::json!({
        "root": canonical(root).to_string_lossy(),
        "pid": std::process::id(),
        "version": taliesin_core::VERSION,
    })
    .to_string();
    router.route(
        IDENTITY_PATH,
        get(move || {
            let body = body.clone();
            async move {
                (
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    body,
                )
            }
        }),
    )
}

/// A preview already listening on `port`, as it describes itself.
struct Incumbent {
    port: u16,
    root: PathBuf,
    pid: i32,
}

/// Is a live session for `root` listening on `port`?
///
/// The proof `taliesin run` requires before sending anything to a port a hint file named.
/// Deliberately the SAME check `bind_with_fallback` already makes about an incumbent, and
/// for the same reason: a hint file survives SIGKILL, ports get recycled, and any local
/// user can bind loopback, so the port holder must both claim this root and be confirmed
/// by the OS ([`is_sibling_preview`] reads `/proc/<pid>/exe`) to be another instance of
/// this binary. A pid-liveness check alone would be satisfied by a recycled pid.
pub(crate) async fn session_owns(port: u16, root: &Path) -> bool {
    let want = canonical(root);
    matches!(identify(port).await, Some(i) if i.root == want && is_sibling_preview(i.pid))
}

/// How long a port holder gets to answer the identity probe. Generous, because a
/// machine under load still has to be able to answer: a probe that gives up early reads
/// as "someone else's port" and silently stacks a second preview, the exact outcome this
/// is here to prevent. Bounded, because a port held by something that accepts
/// connections and never replies must not stall startup.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Accept a reported pid only if signalling it could mean one process. Any local user
/// can bind a loopback port, so this number is untrusted input on its way to `kill`, and
/// the non-positive range is where it gets dangerous: `kill(-1, ...)` signals *every*
/// process the user can reach, `kill(-N, ...)` a whole process group, and `0` the
/// caller's own group. 1 is init. None of those is ever a preview.
fn plausible_pid(raw: i64) -> Option<i32> {
    (2..=i64::from(i32::MAX))
        .contains(&raw)
        .then_some(raw as i32)
}

/// Strip the marker the kernel appends to `/proc/*/exe` once the binary behind it has
/// been replaced. Rebuilding while a preview runs is routine here (the `taliesin`
/// launcher rebuilds on source change), and that preview is still a preview.
#[cfg(target_os = "linux")]
fn without_deleted_marker(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    let stripped: &str = s.strip_suffix(" (deleted)").unwrap_or(&s);
    PathBuf::from(stripped)
}

/// Confirm against the OS that `pid` is another instance of *this binary*, instead of
/// taking the port holder's word for it. `/proc/<pid>/exe` answers both halves at once:
/// it names the executable, and reading it for a process owned by another user fails
/// outright, so a hostile responder cannot borrow this preview's privileges to signal
/// something it could not signal itself.
#[cfg(target_os = "linux")]
fn is_sibling_preview(pid: i32) -> bool {
    let (Ok(mine), Ok(theirs)) = (
        std::env::current_exe(),
        std::fs::read_link(format!("/proc/{pid}/exe")),
    ) else {
        return false;
    };
    without_deleted_marker(&mine) == without_deleted_marker(&theirs)
}

/// No cheap portable equivalent of the `/proc` check, so elsewhere the root match and
/// [`plausible_pid`] are what stand between a responder and a SIGTERM. The residual
/// exposure is a same-user process being terminated, which such an attacker could do
/// directly anyway.
#[cfg(not(target_os = "linux"))]
fn is_sibling_preview(_pid: i32) -> bool {
    true
}

/// Ask whatever holds `port` to identify itself. `None` when nothing answers, when the
/// answer isn't a taliesin preview's, or when it doesn't reply within [`PROBE_TIMEOUT`].
/// In each of those cases the port belongs to something we must leave alone.
async fn identify(port: u16) -> Option<Incumbent> {
    let ask = async {
        let mut sock = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .ok()?;
        let req =
            format!("GET {IDENTITY_PATH} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
        sock.write_all(req.as_bytes()).await.ok()?;
        let mut raw = Vec::new();
        sock.read_to_end(&mut raw).await.ok()?;
        let raw = String::from_utf8(raw).ok()?;
        let (_head, body) = raw.split_once("\r\n\r\n")?;
        let v: serde_json::Value = serde_json::from_str(body).ok()?;
        Some(Incumbent {
            port,
            root: PathBuf::from(v["root"].as_str()?),
            pid: plausible_pid(v["pid"].as_i64()?)?,
        })
    };
    tokio::time::timeout(PROBE_TIMEOUT, ask)
        .await
        .ok()
        .flatten()
}

async fn try_bind(
    host: [u8; 4],
    port: u16,
) -> std::io::Result<(tokio::net::TcpListener, SocketAddr)> {
    let addr = SocketAddr::from((host, port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    // Report the *bound* address: with port 0 ("any free port") the OS assigns the
    // real port, so the requested `addr` would still read `:0`.
    let bound = listener.local_addr().unwrap_or(addr);
    Ok((listener, bound))
}

/// Bind `port` for a preview of `root`. Binds 0.0.0.0 (LAN-reachable) with `expose`,
/// else loopback only.
///
/// Three outcomes, in order. The port is free, and we take it. Or the port (or one in
/// the fallback range) is held by a preview of *this same root*, which we replace:
/// previewing one root twice is never what was meant, and the surplus instances are
/// not harmless: each keeps its own file watcher and kernel subtree re-executing the
/// same sources, on a port nobody is looking at. Or the port belongs to something
/// else, and we fall back to the next free one, so a second project can be previewed
/// alongside the first.
pub(crate) async fn bind_with_fallback(
    port: u16,
    expose: bool,
    root: &Path,
) -> std::io::Result<(tokio::net::TcpListener, SocketAddr)> {
    let host = if expose { [0, 0, 0, 0] } else { [127, 0, 0, 1] };
    let mut last_err = match try_bind(host, port).await {
        Ok(bound) => return Ok(bound),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => Some(e),
        Err(e) => return Err(e),
    };

    // Taken. Sweep the whole fallback range rather than just `port`: launches from
    // before this behavior existed could have stacked several previews of this root.
    // Probe concurrently: a port held by something that accepts connections but never
    // answers costs the full timeout, and ten of those in series would stall startup.
    // Both halves of the filter matter: the root match says the incumbent is redundant,
    // and `is_sibling_preview` says the pid it handed us is really its own, since a
    // responder that simply names a pid must not have it signalled on its say-so.
    let root = canonical(root);
    let mine: Vec<Incumbent> =
        futures::future::join_all((port..=port.saturating_add(9)).map(identify))
            .await
            .into_iter()
            .flatten()
            .filter(|i| i.root == root && is_sibling_preview(i.pid))
            .collect();

    if !mine.is_empty() {
        for inc in &mine {
            crate::log::warn(&format!(
                "port {}: replacing an existing preview of this project (pid {})",
                inc.port, inc.pid
            ));
            // SAFETY: SIGTERM to a pid that just identified itself, over loopback, as a
            // preview of the very root we are about to serve, i.e. this user's own server.
            // SIGTERM rather than SIGKILL so it runs its kernel-reaping teardown.
            unsafe { libc::kill(inc.pid, libc::SIGTERM) };
        }
        // Wait for the canonical port, but only when it was one of ours. If something
        // else holds it, the fallback scan below is already the right answer and there
        // is nothing to wait for.
        if mine.iter().any(|i| i.port == port) {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match try_bind(host, port).await {
                    Ok(bound) => return Ok(bound),
                    Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                        if Instant::now() >= deadline {
                            last_err = Some(e);
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    for p in port.saturating_add(1)..=port.saturating_add(9) {
        match try_bind(host, p).await {
            Ok(bound) => {
                crate::log::warn(&format!("port {port} in use; using {p}"));
                return Ok(bound);
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
    .tali-dev-sections { display: flex; flex-direction: column; gap: .1rem; max-width: 22rem; \
      max-height: 14rem; overflow-y: auto; } \
    .tali-dev-sections .tali-section-row { display: flex; gap: .5rem; justify-content: space-between; \
      width: 100%; text-align: left; font: inherit; font-size: 12px; line-height: 1.35; \
      cursor: pointer; background: none; border: 0; padding: .1rem .35rem; \
      color: var(--tali-muted, #888); } \
    .tali-dev-sections .tali-section-row:hover { color: var(--tali-fg, #111); \
      background: var(--tali-code-bg, #f5f5f5); border-radius: 4px; } \
    .tali-dev-sections .tali-section-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } \
    .tali-dev-sections .tali-section-meta { flex: none; font-variant-numeric: tabular-nums; } \
    .tali-dev-sections .tali-section-meta[data-tali-op=\"warn\"] { color: #d9a23a; } \
    .tali-dev-sections .tali-section-meta[data-tali-op=\"error\"] { color: #e5534b; } \
    .tali-dev-sections .tali-section-empty { padding: .1rem .35rem; color: var(--tali-muted, #888); font-size: 12px; } \
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
    [data-tali-cell-state] { border-left: 3px solid transparent; padding-left: 8px; } \
    [data-tali-cell-state=\"queued\"] { border-left-color: color-mix(in srgb, currentColor 30%, transparent); opacity: .7; } \
    [data-tali-cell-state=\"running\"] { border-left-color: #4c8dff; } \
    [data-tali-cell-state=\"done\"] { border-left-color: #2bb673; } \
    [data-tali-cell-state=\"error\"] { border-left-color: #cc3333; } \
    [data-tali-cell-source=\"cache\"] { border-left-color: color-mix(in srgb, #2bb673 40%, transparent); } \
    [data-tali-cell-source=\"cache\"] .tali-cell-badge { opacity: .6; } \
    .tali-cell-badge { font: 11px/1 var(--tali-mono, monospace); opacity: .75; margin-right: 6px; } \
    @media (prefers-reduced-motion: no-preference) { \
      [data-tali-cell-state=\"running\"] .tali-cell-badge { animation: tali-pulse 1s ease-in-out infinite; } \
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

pub(crate) const MAX_WS_MESSAGE_BYTES: usize = 64 * 1024;

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

const SKIP_DIRS: &[&str] = &["_site", "_book", "_freeze", ".git", "node_modules"];

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
    //! The protocol messages this shared layer still produces (`style`, `diagnostics`),
    //! plus the watch predicates. The op/full_render shape contract the preview client
    //! consumes is pinned in `serve_site`, next to the producers that survived Wave 1.1.
    use super::*;
    use crate::protocol::{self, Diagnostic};
    use crate::testutil::parse;
    use std::collections::HashSet;

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

    /// PA-B3, the preview's half. The mobile TOC sheet is a dimming modal, so Tab belongs
    /// inside it — the same shared trap the lightbox and Cmd-K use. The preview keeps its own
    /// copy of the sheet (it rebuilds the TOC live), so the static build's `toc-sheet.js` fix
    /// does not reach it; `render::tests` pins that copy. The trap must be released when a
    /// resize turns the sheet back into the desktop sidebar, or Tab stays confined to a panel
    /// nobody opened.
    #[test]
    fn client_js_traps_focus_in_the_mobile_toc_sheet() {
        // The bare name is also in the feature-detect guard and the comment, so pin the CALL
        // (deleting it left `contains("taliFocusTrap")` passing — found by mutation).
        assert!(
            CLIENT_JS.contains("taliFocusTrap(tocEl, f)"),
            "the preview's TOC sheet must reuse the shared modal focus trap"
        );
        assert!(
            CLIENT_JS.contains("if (!isSheetMode()) dropTocTrap()"),
            "the preview's TOC sheet must release the trap on a resize out of sheet mode"
        );
    }

    /// The preview REBUILDS `<nav id="TOC">` on every change while the build emits it from
    /// `render::toc_items`, so the same selection rule is written twice, in two languages,
    /// with nothing forcing them to agree. They did not: the client took `h1,h2,h3` by tag
    /// while the build takes a window of two levels below the shallowest heading PRESENT, so
    /// every page whose sections start below `<h1>` (any page with a title block) lost its
    /// third level in the preview only — the author tunes navigation against a TOC readers
    /// never see, and the suite is green either way.
    ///
    /// Pinned as a needle pair, not as an equivalence test: `buildToc` closes over `root` and
    /// `tocEl` inside client.js's single IIFE, so it cannot be called from Node without
    /// splitting the bundle (which `js-modularization` deliberately did not do). The Rust
    /// half of the rule is pinned behaviourally by `render::tests::
    /// toc_filter_is_relative_to_the_shallowest_heading`.
    #[test]
    fn the_previews_toc_uses_the_same_relative_window_as_the_build() {
        assert!(
            !CLIENT_JS.contains("h1[id], h2[id], h3[id]"),
            "the preview must not select TOC headings by absolute tag"
        );
        assert!(
            CLIENT_JS.contains("lvl(h) - base <= 2"),
            "the preview must filter to two levels below the shallowest anchored heading"
        );
        assert!(
            CLIENT_JS.contains("h4[id],h5[id],h6[id]"),
            "…which needs the deeper headings selected in the first place"
        );
    }

    /// `buildToc` was the one place the client re-serialized DOM values back into markup:
    /// a heading's *decoded* `id` was interpolated into an `href` inside a string that was
    /// then assigned to `innerHTML`. Two consequences, one per layer — any `{#id}` carrying
    /// `"`/`<`/`&` corrupted the nav markup (and executed, in preview), and the resulting
    /// fragment stopped matching the anchor the heading actually carries, which is the
    /// client half of `render::tests::toc_href_matches_an_explicit_heading_id_containing_an_entity`.
    ///
    /// Needled rather than executed for the reason the test above gives: `buildToc` closes
    /// over `root`/`tocEl` inside client.js's single IIFE and cannot be called from Node.
    /// The behavioural check is a browser one.
    #[test]
    fn the_previews_toc_is_built_from_dom_nodes_never_innerhtml() {
        let toc = CLIENT_JS
            .split("const buildToc")
            .nth(1)
            .expect("client.js defines buildToc");
        let body = &toc[..toc.find("\n  };").expect("buildToc has a body")];
        // Needle the ASSIGNMENT, not the bare word: the comment inside `buildToc` names
        // `innerHTML` to explain why it is gone, and a substring test on the name alone
        // fails on its own fixture's prose.
        assert!(
            !body.contains("innerHTML ="),
            "buildToc re-serializes DOM text into HTML: {body}"
        );
        assert!(
            body.contains("setAttribute(\"href\", \"#\" + h.id)"),
            "the TOC href must take the id verbatim via setAttribute: {body}"
        );
        assert!(
            body.contains("a.textContent = h.textContent"),
            "the TOC label must be assigned as text, never parsed as markup: {body}"
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
            CLIENT_JS.contains("data-tali-cell-source"),
            "client.js must tag the block with its cache provenance"
        );
        assert!(
            STATUS_CSS.contains("data-tali-cell-source=\"cache\""),
            "STATUS_CSS must style the cached-cell border distinctly from a fresh run"
        );
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

#[cfg(test)]
mod takeover_tests {
    use super::plausible_pid;

    #[test]
    fn only_a_pid_that_means_one_process_survives_the_takeover_check() {
        // The pid arrives over a loopback socket any local user can answer on, and its
        // next stop is `kill`. The whole non-positive range is a broadcast in disguise:
        // -1 signals every process the user can reach, -N a process group, 0 the
        // caller's own group. Letting one through turns "replace the old preview" into
        // "SIGTERM the session".
        assert_eq!(plausible_pid(-1), None, "kill(-1) signals everything");
        assert_eq!(plausible_pid(-4321), None, "kill(-N) signals a group");
        assert_eq!(plausible_pid(0), None, "kill(0) signals our own group");
        assert_eq!(plausible_pid(1), None, "1 is init, never a preview");
        // Out of range for a pid_t, so a cast would wrap into a live process.
        assert_eq!(plausible_pid(i64::from(i32::MAX) + 1), None);
        assert_eq!(plausible_pid(i64::MAX), None);
        // An ordinary pid still passes.
        assert_eq!(plausible_pid(2), Some(2));
        assert_eq!(plausible_pid(31_337), Some(31_337));
        assert_eq!(plausible_pid(i64::from(i32::MAX)), Some(i32::MAX));
    }
}
