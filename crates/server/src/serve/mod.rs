//! The dev server's **shared layer**: the pieces every serving path needs, and the
//! CLI-wide error helpers that grew up next to them.
//!
//! HTTP + asset plumbing (`serve_asset_from`, `content_type`, `percent_decode`), the
//! bundled client + favicon + dev-menu CSS, port binding and the single-instance takeover
//! probe, the origin/host/identity guards in [`security`], the shutdown signal, the file-watch
//! predicates, `parse_args` (the argv grammar every verb shares), and
//! `guarded`/`panic_msg`/`bad_format_error`.
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
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) const CLIENT_JS: &str = include_str!("../../../../web-client/client.js");
/// The preview tab's favicon (an original block-model mark; SVG, so it's tiny
/// and self-contained).
pub(crate) const FAVICON: &str = include_str!("../../../../web-client/favicon.svg");

mod security;
// Re-exported at `crate::serve::*` because serve_site.rs imports several of these.
pub(crate) use security::{with_host_guard, ws_origin_ok};

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
/// tasks that own the kernels, which runs their teardown Drops. We race
/// (rather than `axum`'s `with_graceful_shutdown`) because the preview holds a
/// persistent websocket that never closes on its own, so graceful shutdown would
/// hang. Without this, a Ctrl-C'd preview leaks the whole kernel subtree.
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

/// How long a port holder gets to answer the identity probe. Generous, because a
/// machine under load still has to be able to answer: a probe that gives up early reads
/// as "someone else's port" and silently stacks a second preview, the exact outcome this
/// is here to prevent. Bounded, because a port held by something that accepts
/// connections and never replies must not stall startup.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// The most of a port holder's reply the probe reads. A real answer is under 300 bytes;
/// the timeout bounds how long a holder can talk, not how much, and a holder streaming
/// bytes forever grew the probe past 2 GB in about a second, ten probes at once.
const PROBE_READ_CAP: u64 = 64 * 1024;

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

/// Confirm against the OS that `pid` owns the socket listening on `port`, instead of
/// taking the port holder's word for it: the pid it names must be the process that
/// answered. `/proc/<pid>/fd` answers both halves at once. It lists the process's
/// sockets, and reading it for a process owned by another user fails outright, so a
/// hostile responder cannot borrow this preview's privileges to signal something it could
/// not signal itself.
///
/// This compared `/proc/<pid>/exe` with this binary until 2026-09-24, which let a holder
/// name ANY process of this binary (the author's own `taliesin lsp`, another project's
/// preview) and have it terminated. Holding the port is the fact the takeover rests on.
#[cfg(target_os = "linux")]
fn holds_the_port(pid: i32, port: u16) -> bool {
    // `/proc/net/tcp`: after a header line, one socket per line, fields `sl local_address
    // rem_address st ... inode`, the address as `HEXIP:HEXPORT` (the IP as the in-memory
    // word, so native-endian) and `0A` meaning LISTEN. A preview binds IPv4 loopback only,
    // so the IPv4 table is the whole search, and only a socket on 127.0.0.1 or 0.0.0.0 can
    // have answered a probe sent to 127.0.0.1: one on another address with the same port
    // number belongs to someone the probe never reached.
    let Ok(table) = std::fs::read_to_string("/proc/net/tcp") else {
        return false;
    };
    let listening: Vec<String> = table
        .lines()
        .skip(1)
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            let (hex_ip, hex_port) = f.get(1)?.split_once(':')?;
            let inode = f.get(9)?;
            let ip = std::net::Ipv4Addr::from(u32::from_str_radix(hex_ip, 16).ok()?.to_ne_bytes());
            let listens_here = u16::from_str_radix(hex_port, 16).ok()? == port
                && (ip == std::net::Ipv4Addr::LOCALHOST || ip.is_unspecified())
                && *f.get(3)? == "0A";
            listens_here.then(|| format!("socket:[{inode}]"))
        })
        .collect();
    let Ok(fds) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
        return false;
    };
    fds.flatten().any(|fd| {
        std::fs::read_link(fd.path())
            .is_ok_and(|target| listening.iter().any(|l| target.as_os_str() == l.as_str()))
    })
}

/// No cheap portable equivalent of the `/proc` check, so elsewhere the root match and
/// [`plausible_pid`] are what stand between a responder and a SIGTERM. Nothing ties the
/// pid to the port there, so a holder on another account can name any process of the
/// user running the preview.
#[cfg(not(target_os = "linux"))]
fn holds_the_port(_pid: i32, _port: u16) -> bool {
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
        (&mut sock)
            .take(PROBE_READ_CAP)
            .read_to_end(&mut raw)
            .await
            .ok()?;
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

/// Bind `port` for a preview of `root`, on loopback only.
///
/// Three outcomes, in order. The port is free, and we take it. Or the port (or one in
/// the fallback range) is held by a preview of *this same root*, which we replace:
/// previewing one root twice is never what was meant, and the surplus instances are
/// not harmless: each keeps its own file watcher and kernel subtree re-executing the
/// same sources, on a port nobody is looking at. Or the port belongs to something
/// else, and we fall back to the next free one, so a second project can be previewed
/// alongside the first.
///
/// Also returns what the caller should say about a replaced preview. The caller prints it
/// after its banner: printed here, it went before the banner's screen clear and into the
/// scrollback (audit 2026-09-24, WP11 residual).
pub(crate) async fn bind_with_fallback(
    port: u16,
    root: &Path,
) -> std::io::Result<(tokio::net::TcpListener, SocketAddr, Vec<String>)> {
    let host = [127, 0, 0, 1];
    let mut last_err = match try_bind(host, port).await {
        Ok((listener, addr)) => return Ok((listener, addr, Vec::new())),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => Some(e),
        Err(e) => return Err(e),
    };

    // Taken. Sweep the whole fallback range rather than just `port`: launches from
    // before this behavior existed could have stacked several previews of this root.
    // Probe concurrently: a port held by something that accepts connections but never
    // answers costs the full timeout, and ten of those in series would stall startup.
    // Both halves of the filter matter: the root match says the incumbent is redundant,
    // and `holds_the_port` says the pid it handed us is the process that answered, since
    // a responder that simply names a pid must not have it signalled on its say-so.
    let root = canonical(root);
    let mine: Vec<Incumbent> =
        futures_util::future::join_all((port..=port.saturating_add(9)).map(identify))
            .await
            .into_iter()
            .flatten()
            .filter(|i| i.root == root && holds_the_port(i.pid, i.port))
            .collect();

    let replaced: Vec<String> = mine
        .iter()
        .map(|inc| {
            format!(
                "port {}: replacing an existing preview of this project (pid {})",
                inc.port, inc.pid
            )
        })
        .collect();
    if !mine.is_empty() {
        for inc in &mine {
            // SAFETY: SIGTERM to the pid that owns the port which just identified itself,
            // over loopback, as a preview of the very root we are about to serve, i.e. this
            // user's own server.
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
                    Ok((listener, addr)) => return Ok((listener, addr, replaced)),
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
            // The caller says so, after its banner: printed here, it went before the screen
            // clear and into the scrollback.
            Ok((listener, addr)) => return Ok((listener, addr, replaced)),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => last_err = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("no free port")))
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

/// Serve the file `rel` (a decoded request path) names under the project directory `base`.
///
/// Through the build's own publication rule ([`taliesin_core::includes::publishable`], as a
/// reference), so the preview serves what a build ships and nothing else it happens to be
/// able to read: no `../` climb out of the project and no symlink out of the repository,
/// and no `.`-prefixed path, which the build never publishes. That last clause is what
/// keeps `preview ~/scratch.tmd`, whose project is its parent directory, from handing
/// `.git/config` and every other dotfile under it to any local process that asks.
pub(crate) fn serve_asset_from(base: &Path, rel: &str) -> axum::response::Response {
    use axum::http::{StatusCode, header};
    use taliesin_core::includes::{Reach, publishable};
    let not_found = || (StatusCode::NOT_FOUND, "not found").into_response();
    let Ok(below) = publishable(base, base, Path::new(rel), Reach::Referenced) else {
        return not_found();
    };
    let full = base.join(below);
    if !full.is_file() {
        return not_found();
    }
    match std::fs::read(&full) {
        Ok(bytes) => ([(header::CONTENT_TYPE, content_type(&full))], bytes).into_response(),
        Err(_) => not_found(),
    }
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
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
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
/// Delegates to [`taliesin_core::render::percent_decode`] — the ONE decode, also behind
/// the local-asset validator's and the build copier's shared resolution step
/// (`render::asset_fs_path`) — so what the preview serves and what a build ships can
/// never disagree about which file `my%20image.png` names. See there for the byte-level
/// details (an invalid or char-splitting escape stays literal).
pub(crate) fn percent_decode(s: &str) -> String {
    taliesin_core::render::percent_decode(s)
}

pub(crate) const STATUS_CSS: &str = "\
    /* The preview dev UI, on the theme's own mono at the machine voice's size. \
\
       The FACE and the SIZE are on the container; the uppercase and the tracking are NOT, \
       and that split is spec §4's rule rather than a preference. Two things in here are not \
       generated labels: a diagnostic MESSAGE is a sentence (tracked uppercase turns it into \
       shouting, which is the same correction §4 records for captions reading as terminal \
       output), and `.tali-dev-drafts a` holds a draft page's `title:` — the AUTHOR's own \
       words, which take the serif and no tracking at all. The voice attaches to the labels \
       below, one at a time. */ \
    #tali-controls.tali-dev { position: fixed; bottom: .6rem; left: .6rem; z-index: 9999; \
      font: 400 .78rem/1.4 var(--tali-font-mono); } \
    .tali-dev-row .tali-dev-label, .tali-dev-count, .tali-cell-badge, .tali-diag-kind { \
      text-transform: uppercase; letter-spacing: .053em; } \
    /* The kind as a word. It replaced a `✗ `/`⚠ ` prefix, and it is load-bearing rather \
       than ornamental: without it the only thing separating an error row from a warning \
       row is the colour of its left rule, which is colour as the sole cue (WCAG 1.4.1). */ \
    .tali-diag-kind { display: block; font-size: .72rem; color: var(--tali-muted); \
      margin-bottom: .15rem; } \
    .tali-diag-error .tali-diag-kind { color: var(--tali-status-error); } \
    .tali-diag-warning .tali-diag-kind { color: var(--tali-status-warn); } \
    /* `font: inherit` because a `<button>` inherits NO font: without it the `</>` glyph — the \
       one the console hint names — rendered in the UA's Arial on a surface that had just been \
       put on the theme's own mono. Found by rendering; the family was declared correctly on \
       the container one rule above, which is exactly what a stylesheet read cannot catch. */ \
    .tali-dev-toggle { display: inline-flex; align-items: center; gap: .4rem; cursor: pointer; \
      font: inherit; \
      background: var(--tali-bg); color: var(--tali-muted); \
      border: 1px solid var(--tali-border); border-radius: var(--tali-radius); padding: .25rem .6rem; } \
    .tali-dev-toggle:hover { color: var(--tali-fg); } \
    .tali-dev-toggle.tali-dev-alert { border-color: var(--tali-status-warn); color: var(--tali-status-warn); } \
    .tali-dev-glyph { letter-spacing: -1px; text-transform: none; } \
    .tali-dev-count { min-width: 1rem; padding: 0 .3rem; border-radius: var(--tali-radius); \
      background: var(--tali-status-warn); color: var(--tali-bg); \
      font-size: 11px; line-height: 1.3; text-align: center; } \
    .tali-dev-count[hidden] { display: none; } \
    .tali-dev-dot { width: .5rem; height: .5rem; border-radius: 50%; background: var(--tali-muted); flex: none; } \
    .tali-dev-dot[data-state=\"live\"] { background: var(--tali-status-live); } \
    .tali-dev-dot[data-state=\"warn\"] { background: var(--tali-status-warn); } \
    .tali-dev-dot[data-state=\"error\"] { background: var(--tali-status-error); } \
    .tali-dev-panel { position: absolute; bottom: calc(100% + .45rem); left: 0; min-width: 13rem; \
      display: flex; flex-direction: column; gap: .5rem; padding: .65rem; \
      background: var(--tali-bg); color: var(--tali-fg); \
      border: 1px solid var(--tali-border); border-radius: var(--tali-radius); } \
    .tali-dev-panel[hidden] { display: none; } \
    .tali-dev-row { display: flex; justify-content: space-between; gap: 1rem; color: var(--tali-muted); } \
    .tali-dev-row .tali-dev-label { color: var(--tali-fg); } \
    #tali-wordcount { font-variant-numeric: tabular-nums; } \
    .tali-dev-drafts { display: flex; flex-direction: column; gap: .2rem; margin-top: -.2rem; } \
    /* A draft's `title:` is the AUTHOR's own words inside the tool's own panel, so it \
       takes the serif and no tracking — spec §4's rule, and the reason the voice is on the \
       labels above rather than on the container. */ \
    .tali-dev-drafts a { color: var(--tali-fg); text-decoration: none; \
      font: var(--tali-font-body); font-size: .82rem; letter-spacing: normal; } \
    .tali-dev-drafts a:hover { text-decoration: underline; } \
    .tali-dev-ctl { display: inline-flex; align-items: center; gap: .4rem; text-align: left; cursor: pointer; \
      font: inherit; background: var(--tali-code-bg); color: var(--tali-fg); \
      border: 1px solid var(--tali-border); border-radius: var(--tali-radius); padding: .3rem .55rem; } \
    .tali-dev-ctl:hover { border-color: var(--tali-fg); } \
    .tali-dev-theme svg { width: 14px; height: 14px; } \
    #tali-diagnostics { display: none; flex-direction: column; gap: .3rem; max-width: 22rem; } \
    #tali-diagnostics .tali-diag { padding: .3rem .5rem; border-radius: var(--tali-radius); background: var(--tali-code-bg); \
      border: 1px solid var(--tali-border); line-height: 1.35; } \
    #tali-diagnostics .tali-diag-error { border-left: 2px solid var(--tali-status-error); } \
    #tali-diagnostics .tali-diag-warning { border-left: 2px solid var(--tali-status-warn); } \
    #tali-diagnostics .tali-diag-loc { cursor: pointer; text-align: left; width: 100%; font: inherit; color: inherit; } \
    #tali-diagnostics .tali-diag-loc:hover { border-color: var(--tali-fg); } \
    /* The one glyph left in the dev chrome, and it is a `content` string rather than an \
       emoji: U+2192. The vendored mono subset does not carry it, so it falls back to a \
       system mono for this arrow alone until Plan 4 re-vendors the face. Deliberate, not an \
       oversight — the alternative is the word `source` twice. */ \
    #tali-diagnostics .tali-diag-loc::after { content: \"  \\2192 source\"; color: var(--tali-muted); } \
    #tali-diagnostics .tali-diag-frame { margin: .35rem 0 0; padding: .35rem .45rem; border-radius: var(--tali-radius); overflow-x: auto; \
      background: var(--tali-bg); white-space: pre; font: 11px/1.45 var(--tali-font-mono); } \
    #tali-cell-errors { flex-direction: column; gap: .3rem; max-width: 22rem; } \
    .tali-cellerr { text-align: left; cursor: pointer; font: inherit; \
      color: var(--tali-fg); background: var(--tali-code-bg); border: 1px solid var(--tali-border); \
      border-left: 2px solid var(--tali-status-error); border-radius: var(--tali-radius); padding: .3rem .5rem; \
      white-space: nowrap; overflow: hidden; text-overflow: ellipsis; } \
    .tali-cellerr:hover { border-color: var(--tali-status-error); } \
    /* The progress chip is appended to `document.body`, NOT inside `#tali-controls`, so it \
       cannot inherit the machine voice from the rule at the top of this sheet — it would \
       inherit the page's serif instead. Written out for that reason. */ \
    #tali-progress { position: fixed; bottom: 12px; right: 12px; z-index: 9999; \
      display: flex; align-items: center; gap: 6px; \
      font: 400 .78rem/1.4 var(--tali-font-mono); \
      padding: 5px 10px; border-radius: var(--tali-radius); \
      background: var(--tali-bg); color: var(--tali-fg); \
      border: 1px solid color-mix(in srgb, currentColor 20%, transparent); \
      cursor: default; user-select: none; } \
    #tali-progress[data-state=\"busy\"] { cursor: pointer; } \
    #tali-progress[data-state=\"warming\"] { border-color: color-mix(in srgb, var(--tali-status-warn) 55%, transparent); } \
    #tali-progress[data-state=\"error\"] { cursor: pointer; border-color: var(--tali-status-error); } \
    /* An idle chip is quiet by COLOUR, not by `opacity`: spec §3 bans opacity as text \
       dimming outright, because it is a colour nobody chose and nothing scores. */ \
    #tali-progress[data-state=\"idle\"] { color: var(--tali-muted); } \
    .tali-prog-dot { width: .5rem; height: .5rem; border-radius: 50%; flex: none; \
      background: var(--tali-muted); } \
    #tali-progress[data-state=\"busy\"] .tali-prog-dot { background: var(--tali-fg); } \
    #tali-progress[data-state=\"warming\"] .tali-prog-dot { background: var(--tali-status-warn); } \
    #tali-progress[data-state=\"idle\"] .tali-prog-dot { background: var(--tali-status-live); } \
    #tali-progress[data-state=\"error\"] .tali-prog-dot { background: var(--tali-status-error); } \
    @media (prefers-reduced-motion: no-preference) { \
      #tali-progress[data-state=\"busy\"] .tali-prog-dot, \
      #tali-progress[data-state=\"warming\"] .tali-prog-dot { \
        animation: tali-dot-pulse 1.2s ease-in-out infinite; } \
      @keyframes tali-dot-pulse { 0%,100% { opacity:1; } 50% { opacity:.3; } } \
    } \
    .tali-prog-label { white-space: nowrap; } \
    .tali-prog-bar { display: inline-block; width: 48px; height: 4px; border-radius: var(--tali-radius); \
      background: color-mix(in srgb, currentColor 15%, transparent); flex: none; } \
    .tali-prog-fill { display: block; height: 100%; border-radius: var(--tali-radius); \
      background: var(--tali-fg); transition: width var(--tali-dur) linear; } \
    [data-tali-cell-state] { border-left: 2px solid transparent; padding-left: 8px; } \
    /* No `opacity` on a queued cell: it dimmed the AUTHOR's own rendered output — a figure, \
       a table — to say something about the tool's schedule. The border says it instead. */ \
    [data-tali-cell-state=\"queued\"] { border-left-color: color-mix(in srgb, currentColor 30%, transparent); } \
    [data-tali-cell-state=\"running\"] { border-left-color: var(--tali-fg); } \
    [data-tali-cell-state=\"done\"] { border-left-color: var(--tali-status-live); } \
    [data-tali-cell-state=\"error\"] { border-left-color: var(--tali-status-error); } \
    [data-tali-cell-source=\"cache\"] { border-left-color: color-mix(in srgb, var(--tali-status-live) 40%, transparent); } \
    /* The badge is muted by COLOUR. A cached replay used to be dimmed FURTHER on top of \
       that, which stacked two unscored alphas; the badge now says `cached` in words, so the \
       distinction is carried by the label rather than by how faint it is. */ \
    .tali-cell-badge { font: 400 11px/1 var(--tali-font-mono); text-transform: uppercase; \
      letter-spacing: .053em; color: var(--tali-muted); margin-right: 6px; } \
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

const SKIP_DIRS: &[&str] = &["_site", "_book", "_freeze", "node_modules"];

/// Whether a path component names a tree the watcher leaves alone: a generated one, or a
/// `.`-prefixed one (`.git`, a project's `.venv`), which discovery and the build skip too.
/// A project's `.venv` held 2,334 of the real tech-blog's 2,372 watched directories, and
/// grew by 190 with each `pip install` (audit 2026-09-24 invalidation #14).
fn skipped_name(name: &str) -> bool {
    name.starts_with('.') || SKIP_DIRS.contains(&name)
}

/// Whether a file event under `root` should reach the rebuild: any path outside the
/// generated and `.`-prefixed trees. What it then rebuilds is decided by what the pages
/// read (`serve_site`'s `PageDoc::reads`), not by the file's extension: a list of
/// extensions let `.css` and `.json` through, which nothing reads, and dropped a `.pdf` a
/// page links, whose "broken link" then outlived the file's creation (audit 2026-09-24
/// invalidation #12).
///
/// **The skip-dir scan runs on the path RELATIVE to `root`**, and that is load-bearing.
/// The watcher hands this absolute event paths, so a whole-path scan asked whether any
/// *ancestor* of the project happened to be called `_site`/`_book`/`_freeze`/`.git`/
/// `node_modules` — and vetoed every event in a project that merely lived under one, which
/// is hot reload dead for that project, silently, with the page still serving 200. Inside a
/// normally-located project the two readings agree, which is why it went unseen.
/// A path that is not under `root` keeps the whole-path scan, so a caller with no
/// meaningful root loses no vetting.
pub(crate) fn relevant_path(p: &Path, root: &Path) -> bool {
    !p.strip_prefix(root).unwrap_or(p).components().any(|c| {
        matches!(c, std::path::Component::Normal(name) if name.to_str().is_some_and(skipped_name))
    })
}

/// Whether a directory should be pruned from the watch set by its own name — a generated
/// or `.`-prefixed tree we never register an inotify watch inside.
pub(crate) fn is_pruned_dir(dir: &Path) -> bool {
    dir.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(skipped_name)
}

/// Every directory under `base` (inclusive) that we register a non-recursive watch on,
/// pruning generated and `.`-prefixed subtrees (`node_modules`, `_site`, `_book`,
/// `_freeze`, `.git`, `.venv`) whole. notify's `Recursive` mode walks the *entire* tree and adds one inotify watch
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
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) && relevant_path(&path, root)
            {
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
/// commands (`build`, including `--check-only`) call core rendering directly with no
/// async rebuild loop to absorb a panic, so without this a malformed doc that panics the
/// renderer crashes the CLI with a raw backtrace + abort instead of a located error and a
/// non-zero exit. `AssertUnwindSafe` is sound here: a panic mid-`f` is surfaced and the
/// caller returns immediately, so no half-updated state is observed afterward.
pub(crate) fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|p| panic_msg(&*p))
}

/// Read a verb's argv (the tokens after the verb) by the one grammar every verb shares, so
/// no two verbs disagree about what a token is. Four parsers followed four rules until
/// 2026-09-24: `doctor -jsn` was a directory, `preview -o x.tmd` a path, `--format=json` an
/// unknown flag, and an extra positional was dropped by three verbs and kept by the fourth.
///
/// - Any token that starts with `-` is a flag, never a positional. A dash-named file is
///   still reachable as `./-x.tmd`.
/// - `--flag=value` means `--flag value`. A flag's value is the part after `=`, else the
///   next token unless that token is itself a flag ([`FlagValue::take`]).
/// - An unknown flag is refused with a did-you-mean over the verb's own flags (`known`), and
///   a known flag given an `=value` it does not take is refused too.
/// - At most `max` positionals: an extra one is refused, never silently dropped.
///
/// `on_flag` sees each flag and returns whether it knows it. Returns the positionals in
/// order; an error is a message with no `error:` prefix, for `log::error` to frame.
pub(crate) fn parse_args<'a>(
    verb: &str,
    args: &'a [String],
    known: &[&'static str],
    max: usize,
    mut on_flag: impl FnMut(&'a str, &mut FlagValue<'a, '_>) -> Result<bool, String>,
) -> Result<Vec<&'a str>, String> {
    let mut positionals = Vec::new();
    let mut next = 0;
    while let Some(token) = args.get(next).map(String::as_str) {
        next += 1;
        if !token.starts_with('-') {
            if positionals.len() == max {
                // Hung under `crate::log`'s 10-column tag gutter, like `not_a_project_error`.
                return Err(
                    format!("unexpected argument `{token}`\n{}", crate::usage_line(verb))
                        .replace('\n', "\n          "),
                );
            }
            positionals.push(token);
            continue;
        }
        let (name, inline) = match token.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (token, None),
        };
        let mut value = FlagValue {
            inline,
            args,
            next: &mut next,
            taken: false,
        };
        if !on_flag(name, &mut value)? {
            return Err(unknown_flag_error(verb, name, known));
        }
        if inline.is_some() && !value.taken {
            return Err(format!("`{name}` takes no value"));
        }
    }
    Ok(positionals)
}

/// Where a flag's value comes from, for [`parse_args`]'s `on_flag`.
pub(crate) struct FlagValue<'a, 'n> {
    inline: Option<&'a str>,
    args: &'a [String],
    next: &'n mut usize,
    taken: bool,
}

impl<'a> FlagValue<'a, '_> {
    /// The flag's value: the part after `=`, else the next token unless that token is itself
    /// a flag, in which case it stays a flag and the value is `None`.
    pub(crate) fn take(&mut self) -> Option<&'a str> {
        self.taken = true;
        if let Some(v) = self.inline {
            return (!v.is_empty()).then_some(v);
        }
        let v = self
            .args
            .get(*self.next)
            .map(String::as_str)
            .filter(|t| !t.starts_with('-'))?;
        *self.next += 1;
        Some(v)
    }
}

/// Build a hard-error message for an unrecognized flag: "did you mean `--strict`?" when a
/// known flag is within edit distance 2 (`closest`), or is the one flag the typed name
/// extends or abbreviates (`--output` for `--out`, three edits away); otherwise a pointer
/// to the verb's `--help`, which lists its flags. [`parse_args`] is the one caller, so a
/// typo'd flag fails loudly the same way on every verb. `known` is each verb's own
/// accepted long-flag set. No `error:` prefix, so the caller frames it.
fn unknown_flag_error(verb: &str, flag: &str, known: &[&'static str]) -> String {
    match taliesin_core::closest(flag, known)
        .or_else(|| extends_or_abbreviates(flag, known.iter().copied()))
    {
        Some(s) => format!("unknown flag `{flag}` (did you mean `{s}`?)"),
        None => format!("unknown flag `{flag}` (run `taliesin {verb} --help` for its flags)"),
    }
}

/// The one candidate a typed name extends or abbreviates, for the cases edit distance
/// cannot see: `preview-site` is five edits from `preview`, and `--output` three from
/// `--out`. The verb and flag did-you-means both consult it, only after `closest` declines.
///
/// A name shorter than two characters is not a signal, and ambiguity yields nothing rather
/// than a coin flip: picking a winner when two candidates match would teach a rule that is
/// not real.
pub(crate) fn extends_or_abbreviates<'c>(
    typed: &str,
    candidates: impl IntoIterator<Item = &'c str>,
) -> Option<&'c str> {
    if typed.len() < 2 {
        return None;
    }
    let mut hits = candidates
        .into_iter()
        .filter(|c| typed.starts_with(c) || c.starts_with(typed));
    let first = hits.next()?;
    hits.next().is_none().then_some(first)
}

/// One wording for a bad `--format` value, shared by every subcommand that takes
/// `--format`/`--json` (`build`/`doctor`) so the same mistake reads
/// identically everywhere. `got` is the offending value, or
/// `None` when `--format` was given with nothing after it. No `error:` prefix — the caller
/// frames it exactly like `unknown_flag_error` (`log::error` styles it).
pub(crate) fn bad_format_error(got: Option<&str>) -> String {
    format!(
        "--format expects human or json (got {})",
        got.unwrap_or("nothing")
    )
}

/// Render `root` (an ancestor of `path`, as returned by
/// [`taliesin_core::site::enclosing_site_root`]) in the same spelling `path` itself was
/// given in, rather than `enclosing_site_root`'s always-absolute answer.
///
/// `enclosing_site_root` canonicalizes internally (`walk_up_for_site_yml` starts with
/// `start.canonicalize().ok()?`), so `root` carries no memory of whether `path` was typed
/// relatively — the "not a project" message would otherwise mix a relative subject with
/// an absolute suggested fix. But `root` is, by construction, some number of directory
/// levels above `path`'s own canonical form, and that level count is a fact about the
/// real filesystem tree, not about either path's spelling — so popping that many trailing
/// components off `path` AS TYPED is a plausible reconstruction of the ancestor in the
/// caller's own spelling. An absolute `path` in still comes out absolute; that is simply
/// what popping components off an absolute path yields.
///
/// That reconstruction is only a GUESS, though: `climbed` is counted in the canonical
/// (symlink-resolved) tree, and popping it off the typed path is only the same operation
/// when no symlink sits between `path` and `root`. When one does — e.g. `path`'s last
/// component is itself a symlink into a differently-deep real tree — the two structures
/// diverge silently: popping still lands on *some* syntactically valid directory, just
/// not `root`, and if that wrong directory happens to have its own `_site.yml` the
/// suggested command would build an unrelated project with no error at all. So the
/// candidate is never trusted on arithmetic alone: it is re-canonicalized and required to
/// name the exact same directory as `root` (both are canonical at that point, so a plain
/// `Path` equality is the right test — no further resolution needed) before it is
/// returned.
///
/// Falls back to `root` (today's absolute rendering — uglier, but always correct)
/// whenever recovering the typed spelling isn't safely possible: `path` failing to
/// canonicalize (e.g. a race, or a permissions error), a zero-level climb (neither
/// caller reaches here without one, but an unchanged `path` would misname itself as its
/// own ancestor), `path` running out of components to pop, the popped candidate itself
/// failing to canonicalize, or — the symlink case above — the popped candidate
/// canonicalizing to somewhere other than `root`.
fn ancestor_as_typed(path: &Path, root: &Path) -> PathBuf {
    let Ok(canon) = path.canonicalize() else {
        return root.to_path_buf();
    };
    let climbed = canon
        .components()
        .count()
        .saturating_sub(root.components().count());
    if climbed == 0 {
        return root.to_path_buf();
    }
    let mut shown = path.to_path_buf();
    for _ in 0..climbed {
        match shown.parent() {
            Some(p) if !p.as_os_str().is_empty() => shown = p.to_path_buf(),
            _ => return root.to_path_buf(),
        }
    }
    // Prove the reconstruction rather than trust the arithmetic: a symlink anywhere
    // between `path` and `root` can make `climbed` (counted in the canonical tree) not
    // correspond to the number of components popped off the typed one, landing on a
    // syntactically valid but semantically wrong directory. Re-canonicalizing `shown`
    // and requiring it match `root` exactly turns that unbounded wrong-answer class into
    // a bounded, honest fallback.
    match shown.canonicalize() {
        Ok(reconstructed) if reconstructed == root => shown,
        _ => root.to_path_buf(),
    }
}

/// The one message both `build` and `preview` print when handed a directory that is not a
/// project. A directory is a project, and a project is what `_site.yml` declares; without
/// one there is no nav to build, no title to brand with, and no page to serve at `/`.
///
/// When an ancestor IS a project, that is nearly always what the author meant (running the
/// verb on `corpus/tech-blog/posts` silently built eight posts as a detached site), so the
/// suggestion leads with it instead of the generic pair.
pub(crate) fn not_a_project_error(path: &Path, verb: &str) -> String {
    let shown = path.display();
    let body = if let Some(root) = taliesin_core::site::enclosing_site_root(path) {
        let root = ancestor_as_typed(path, &root);
        format!(
            "{shown} has no _site.yml.\n\
             its ancestor {root} is a project. did you mean:\n  \
             taliesin {verb} {root}",
            root = root.display()
        )
    } else {
        // `join` rather than string concatenation, so the suggestion reads
        // `corpus/agent/<page>.tmd` whether or not the author typed a trailing slash.
        let label1 = format!("to {verb} one document:");
        let label2 = "to make it a site or book:";
        // Right-pad the shorter label so both suggested commands start in the same
        // column regardless of the verb's length: a hardcoded gap only lined up for
        // `preview` (24 chars) and drifted for `build` (22 chars).
        let width = label1.chars().count().max(label2.chars().count());
        format!(
            "{shown} has no _site.yml, so it is not a project.\n\
             {label1:<width$} taliesin {verb} {example}\n\
             {label2:<width$} add a _site.yml",
            example = path.join("<page>.tmd").display()
        )
    };
    // Hang every continuation line under `crate::log`'s 10-column tag gutter
    // ("  " + a 7-wide tag + " "), so a multi-line error reads as one block instead of
    // half a message sitting flush against the left margin (same treatment as
    // `exec.rs`'s `kernel_failure_report`, and for the same reason). Done here, once,
    // rather than at each call site, so `build` and `preview` cannot drift or forget it.
    body.replace('\n', "\n          ")
}

/// The one message both `build` and `preview` print when handed a single file whose
/// extension is not an accepted source extension. The site walker discovers `.tmd` only
/// ([`taliesin_core::ext::ACCEPTED_SOURCE_EXTS`]), so a `note.md` the single-file verbs
/// happily rendered was still invisible to `build <dir>` — absent from its output at
/// exit 0. Derived from that same constant, so the two verbs and the walker can never
/// disagree about what a source document is.
///
/// Two lines: name the file, then the accepted extension and the rename that gets there. A
/// `.tmd` is Markdown, so a Markdown file only needs the new name; the hint names no other
/// tool, since Taliesin answers for its own vocabulary and nothing else.
pub(crate) fn not_a_source_error(path: &Path, verb: &str) -> String {
    let exts = taliesin_core::ext::ACCEPTED_SOURCE_EXTS;
    let accepted = exts
        .iter()
        .map(|e| format!(".{e}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{shown} is not a Taliesin source document.\n\
         taliesin {verb} takes a {accepted} file: if it is Markdown, rename it to {renamed}.",
        shown = path.display(),
        renamed = path.with_extension(exts[0]).display()
    )
    // Same gutter hang as `not_a_project_error`, for the same reason.
    .replace('\n', "\n          ")
}

#[cfg(test)]
mod protocol_contract {
    //! The protocol messages this shared layer still produces (`diagnostics`), plus the
    //! watch predicates. `style` rode here too until `theme:` was cut on 2026-08-17. The op/full_render shape contract the preview client
    //! consumes is pinned in `serve_site`, next to the producers that survived Wave 1.1.
    use super::*;
    use crate::lint::Diagnostic;
    use crate::protocol;
    use crate::testutil::parse;
    use std::collections::HashSet;

    #[test]
    fn relevant_path_watches_tmd_edits() {
        // `.tmd` is the native (and only) source extension; a watcher blind to it would
        // silently never rebuild on a `.tmd` edit — the core edit loop would be broken.
        let root = Path::new("/tmp");
        assert!(relevant_path(Path::new("/tmp/doc.tmd"), root));
        // A directory has no extension, and renaming or deleting one takes its pages along.
        assert!(relevant_path(Path::new("/tmp/posts/a-star"), root));
        // Whether a file matters is what the pages read, not its extension: a page can link
        // a `.pdf` or a `.csv`, and creating it clears that link's "broken" (audit
        // 2026-09-24 invalidation #12). A `.`-prefixed path is read by nothing.
        assert!(relevant_path(Path::new("/tmp/files/report.pdf"), root));
        assert!(!relevant_path(Path::new("/tmp/.venv/lib/site.py"), root));
        assert!(!relevant_path(
            Path::new("/tmp/.goutputstream-AB12CD"),
            root
        ));
        // The generated/VCS trees are still vetoed — that is what keeps the executor's own
        // `_freeze/` writes from rebuilding every run.
        assert!(!relevant_path(Path::new("/tmp/_freeze/doc.tmd"), root));
        assert!(!relevant_path(
            Path::new("/tmp/sub/node_modules/a.js"),
            root
        ));
    }

    #[test]
    fn the_skip_dir_scan_is_relative_to_the_project_root() {
        // The scan walked the WHOLE path, while the watcher hands it an absolute event
        // path. A project that merely lives under a directory named `_site`/`_book`/
        // `_freeze`/`.git`/`node_modules` therefore had every one of its own events
        // vetoed: watches were registered, events arrived, and hot reload never fired
        // again — total and silent, with the page still serving 200.
        let root = Path::new("/home/me/_site/blog");
        assert!(
            relevant_path(&root.join("post.tmd"), root),
            "an ancestor's name is not this project's business"
        );
        assert!(relevant_path(&root.join("sub/post.tmd"), root));
        // Inside the project the veto still stands, on the same names.
        assert!(!relevant_path(&root.join("_freeze/post.tmd"), root));
        assert!(!relevant_path(&root.join(".git/COMMIT_EDITMSG.md"), root));
        // A path from outside the project keeps the whole-path scan (nothing to be
        // relative to), so an unrooted caller loses no vetting.
        assert!(!relevant_path(Path::new("/elsewhere/_site/post.tmd"), root));
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
            ".venv/lib/python3.12/site-packages",
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
        // A `.`-prefixed tree is no page's and nothing a page reads, as discovery and the
        // build agree; a project's own `.venv` held 2,334 of the real tech-blog's 2,372
        // watched directories (audit 2026-09-24 invalidation #14).
        assert!(!dirs.contains(&root.join(".venv")));
        assert!(!dirs.contains(&root.join(".venv/lib")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn located_diagnostic_serializes_file_line_and_frame() {
        let d = Diagnostic::new("doc.tmd".into(), Some(3), "bad yaml".into())
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

    /// The preview REBUILDS `<nav id="TOC">` on every change while the build emits it from
    /// `render::toc_items`, so the same selection rule is written twice, in two languages,
    /// with nothing forcing them to agree. They did not: the client took `h1,h2,h3` by tag
    /// while the build takes a window of two levels below the shallowest heading PRESENT, so
    /// every page whose sections start below `<h1>` (any page with a title block) lost its
    /// third level in the preview only — the author tunes navigation against a TOC readers
    /// never see, and the suite is green either way.
    ///
    /// **The rule has TWO halves and only the level window was pinned here.** The other is
    /// WHICH headings are candidates at all: `toc_items` iterates the block list, so a
    /// heading folded into a `:::` container block (one block whose html happens to contain
    /// headings) is not a candidate, while the client's `querySelectorAll` descended into it
    /// and listed it. Measured 2026-08-13: a `## Inside a width escape` inside a
    /// `.column-page` gave the preview 4 entries against the build's 3. The client now takes
    /// `root`'s element children, which IS the block list (`<main id="tali-root">{blocks}
    /// </main>`, and every op appends/replaces at that level).
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
            CLIENT_JS.contains("/^H[1-6]$/.test(h.tagName)"),
            "…which needs every heading level a candidate in the first place"
        );
        assert!(
            CLIENT_JS.contains("[...root.children]"),
            "and TOP-LEVEL blocks only, or a heading inside a `:::` container is in the \
             preview's TOC and absent from the build's"
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
        // DX9: the `cached` badge + the muted cached-cell border are include_str!'d JS/CSS,
        // so this drift guard keeps the render and the style in lockstep with the protocol's
        // `source: "cache"` tag. If the badge text or the CSS attr hook is renamed, the wire
        // stays "cache" and the surface goes silently blank — this fails first.
        //
        // The needle was the literal `⚡ cached` until 2026-08-15, when spec §8 took the emoji
        // out of the dev chrome. What this test is about is the DISTINCTION between a replay
        // and a fresh run, not the pictogram that used to carry it, so the needle is the word:
        // a badge renamed or blanked still reddens this.
        assert!(
            CLIENT_JS.contains("\"cached\""),
            "client.js must render the `cached` badge for a cache replay"
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

    #[test]
    fn not_a_project_error_names_both_fixes() {
        let dir = std::path::Path::new("corpus/agent");
        let msg = not_a_project_error(dir, "preview");
        assert!(
            msg.contains("no _site.yml"),
            "names the missing file: {msg}"
        );
        assert!(
            msg.contains("_site.yml") && msg.contains("add"),
            "offers the make-it-a-project fix: {msg}"
        );
        assert!(
            msg.contains("taliesin preview corpus/agent/"),
            "offers the name-one-document fix, with the verb: {msg}"
        );
        // Every continuation line hangs under `crate::log`'s 10-column tag gutter
        // ("  " + a 7-wide tag + " "), so a multi-line error reads as one block instead
        // of half a message sitting flush against the left margin (`log::error` prints
        // `"  {tag:<7} {msg}"`, and `msg.replace('\n', ...)` never runs on the first line).
        for cont in msg.lines().skip(1) {
            assert!(
                cont.starts_with("          "),
                "continuation line must hang under the 10-column gutter: {cont:?} in {msg:?}"
            );
        }
    }

    #[test]
    fn not_a_source_error_names_the_file_and_the_accepted_extension() {
        let msg = not_a_source_error(std::path::Path::new("notes/note.md"), "build");
        assert!(msg.contains("notes/note.md"), "names the file: {msg}");
        // Derived from the walker's own vocabulary, so the single-file verbs and the
        // site walker can never disagree about what a source document is.
        for ext in taliesin_core::ext::ACCEPTED_SOURCE_EXTS {
            assert!(
                msg.contains(&format!(".{ext}")),
                "names the accepted extension .{ext}: {msg}"
            );
        }
        // The fix, not only the rule: a stranger with a Markdown file was told what the
        // tool takes and left to work out the rename (audit 2026-09-24, first-hour #13).
        assert!(
            msg.contains("rename it to notes/note.tmd"),
            "names the rename that makes it a source document: {msg}"
        );
        assert_eq!(
            msg.lines().count(),
            2,
            "two lines, the refusal and its fix: {msg}"
        );
        // Same gutter-hang treatment as `not_a_project_error`, above.
        for cont in msg.lines().skip(1) {
            assert!(
                cont.starts_with("          "),
                "continuation line must hang under the 10-column gutter: {cont:?} in {msg:?}"
            );
        }
    }

    #[test]
    fn not_a_project_error_leads_with_an_enclosing_project() {
        // corpus/tech-blog/posts has no _site.yml of its own, but corpus/tech-blog does.
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/tech-blog/posts");
        let msg = not_a_project_error(&root, "build");
        assert!(
            msg.contains("tech-blog"),
            "names the ancestor project: {msg}"
        );
        assert!(
            msg.contains("did you mean"),
            "leads with the ancestor as the likely intent: {msg}"
        );
        // Same gutter-hang treatment as the other branch, above.
        for cont in msg.lines().skip(1) {
            assert!(
                cont.starts_with("          "),
                "continuation line must hang under the 10-column gutter: {cont:?} in {msg:?}"
            );
        }
    }

    #[test]
    fn not_a_project_error_names_the_ancestor_in_the_same_spelling_as_the_subject() {
        // Regression: `enclosing_site_root` canonicalizes internally, so its answer is
        // always absolute regardless of how `path` was spelled — without
        // `ancestor_as_typed` this message mixed a relative subject with an absolute
        // suggested fix (and told the reader to run a 50-character absolute command
        // where `taliesin build corpus/tech-blog` would do). Relative in the cwd this
        // test binary runs from (`crates/server`, so `../..` is the repo root) exercises
        // exactly the caller-supplied-a-relative-path case; `..` in the middle also
        // proves popping trailing components doesn't get confused by it.
        let dir = std::path::Path::new("../../corpus/tech-blog/posts");
        let msg = not_a_project_error(dir, "build");
        assert!(
            msg.contains("its ancestor ../../corpus/tech-blog is a project"),
            "the ancestor must be spelled the same way the subject was: {msg}"
        );
        assert!(
            msg.contains("taliesin build ../../corpus/tech-blog"),
            "the suggested command must use the same spelling: {msg}"
        );
        assert!(
            !msg.contains(env!("CARGO_MANIFEST_DIR")),
            "must not fall back to an absolute path when the subject was relative: {msg}"
        );
    }

    #[test]
    fn ancestor_as_typed_recovers_a_relative_spelling() {
        let subject = std::path::Path::new("../../corpus/tech-blog/posts");
        let root = taliesin_core::site::enclosing_site_root(subject)
            .expect("corpus/tech-blog is a project");
        assert!(
            root.is_absolute(),
            "enclosing_site_root always canonicalizes: {root:?}"
        );
        let shown = ancestor_as_typed(subject, &root);
        assert_eq!(shown, std::path::Path::new("../../corpus/tech-blog"));
    }

    #[test]
    fn ancestor_as_typed_leaves_an_absolute_subject_absolute() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let subject = manifest.join("../../corpus/tech-blog/posts");
        let root = taliesin_core::site::enclosing_site_root(&subject)
            .expect("corpus/tech-blog is a project");
        let shown = ancestor_as_typed(&subject, &root);
        assert!(
            shown.is_absolute(),
            "an absolute subject must stay absolute, not be left canonicalized-relative: {shown:?}"
        );
        assert_eq!(shown, manifest.join("../../corpus/tech-blog"));
    }

    #[test]
    fn ancestor_as_typed_handles_a_trailing_slash_without_a_dangling_separator() {
        let subject = std::path::Path::new("../../corpus/tech-blog/posts/");
        let root = taliesin_core::site::enclosing_site_root(subject)
            .expect("corpus/tech-blog is a project");
        let shown = ancestor_as_typed(subject, &root);
        assert_eq!(shown, std::path::Path::new("../../corpus/tech-blog"));
        assert!(
            !shown.display().to_string().ends_with('/'),
            "no dangling trailing separator: {shown:?}"
        );
    }

    #[test]
    fn ancestor_as_typed_falls_back_to_root_when_the_subject_cannot_canonicalize() {
        // A path that does not exist (a race with the caller's own `.is_dir()` check, or
        // a permissions error) must not panic or unwrap — a worse-looking (absolute)
        // message beats a crash.
        let bogus = std::path::Path::new("/definitely/does/not/exist/so/canonicalize/fails");
        let root = std::path::Path::new("/some/real/ancestor");
        assert_eq!(ancestor_as_typed(bogus, root), root);
    }

    #[test]
    fn ancestor_as_typed_does_not_echo_the_subject_back_as_its_own_ancestor() {
        // Neither caller reaches `ancestor_as_typed` without `enclosing_site_root` having
        // climbed at least one level (both only call it after confirming `path` itself
        // has no `_site.yml`), but guard the zero-level case anyway: if `root` and
        // `path`'s own canonical form were equal, popping zero components off `path`
        // would misname it as its own ancestor.
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let same = manifest.canonicalize().expect("manifest dir exists");
        assert_eq!(ancestor_as_typed(manifest, &same), same);
    }

    #[test]
    #[cfg(unix)]
    fn ancestor_as_typed_falls_back_when_a_symlink_breaks_the_correspondence() {
        // `climbed` is counted in the CANONICAL tree; popping it off the TYPED path only
        // recovers the right directory when no symlink sits between them. Build a layout
        // where it doesn't:
        //
        //   <tmp>/real/a/b/leaf/          no _site.yml
        //   <tmp>/real/_site.yml          the TRUE ancestor project, 3 real levels up
        //   <tmp>/fake/tech-blog/posts -> <tmp>/real/a/b/leaf   (symlink)
        //
        // Typed subject: `<tmp>/fake/tech-blog/posts`. Its canonical form (through the
        // symlink) is `<tmp>/real/a/b/leaf`, so `enclosing_site_root` climbs 3 levels to
        // `<tmp>/real`. Popping 3 components off the TYPED path instead lands on
        // `<tmp>` itself — a real, existing directory that is NOT the ancestor. Without
        // the canonicalize-and-compare guard this function would silently hand back
        // `<tmp>`, and if `<tmp>` happened to carry its own `_site.yml` the suggested
        // command would build an unrelated project with no error at all.
        use std::os::unix::fs::symlink;

        let base = std::env::temp_dir().join(format!(
            "tali-ancestor-symlink-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("real/a/b/leaf")).unwrap();
        std::fs::write(base.join("real/_site.yml"), b"title: T\n").unwrap();
        std::fs::create_dir_all(base.join("fake/tech-blog")).unwrap();
        symlink(
            base.join("real/a/b/leaf"),
            base.join("fake/tech-blog/posts"),
        )
        .unwrap();

        let subject = base.join("fake/tech-blog/posts");
        let root = taliesin_core::site::enclosing_site_root(&subject)
            .expect("real/_site.yml is an ancestor through the symlink");
        let true_ancestor = base.join("real").canonicalize().unwrap();
        assert_eq!(
            root, true_ancestor,
            "sanity: the walk finds the real project"
        );

        let shown = ancestor_as_typed(&subject, &root);
        assert_eq!(
            shown, root,
            "a symlink-broken reconstruction must fall back to the absolute (but \
             correct) root, not a syntactically valid but wrong directory: {shown:?}"
        );
        // Specifically must NOT be the wrong-but-plausible directory the naive pop
        // would have produced.
        assert_ne!(
            shown, base,
            "must not silently name the wrong ancestor {base:?} as if it were correct"
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod argv_tests {
    use super::parse_args;

    const KNOWN: &[&str] = &["--flag", "--value"];

    /// The grammar, in one place, on a stand-in verb with one switch and one valued flag.
    /// Returns `(positionals, flag seen, value)` or the error.
    fn parse(tokens: &[&str]) -> Result<(Vec<String>, bool, Option<String>), String> {
        let args: Vec<String> = tokens.iter().map(|s| s.to_string()).collect();
        let (mut flag, mut value) = (false, None);
        let positionals = parse_args("build", &args, KNOWN, 2, |name, v| {
            match name {
                "--flag" => flag = true,
                "--value" => value = Some(v.take().ok_or("--value needs a value")?.to_string()),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok((
            positionals.iter().map(|s| s.to_string()).collect(),
            flag,
            value,
        ))
    }

    #[test]
    fn every_verb_reads_a_token_the_same_way() {
        // Positionals in order, flags anywhere.
        let (pos, flag, _) = parse(&["a", "--flag", "b"]).unwrap();
        assert_eq!((pos, flag), (vec!["a".to_string(), "b".to_string()], true));

        // Any leading dash is a flag: a single-dash token is never a path.
        let err = parse(&["a", "-o"]).unwrap_err();
        assert!(err.contains("unknown flag `-o`"), "{err}");
        // An unknown flag names the verb's nearest one.
        let err = parse(&["--flga"]).unwrap_err();
        assert!(err.contains("did you mean `--flag`"), "{err}");

        // A value is the part after `=`, else the next token unless that is itself a flag.
        assert_eq!(parse(&["--value=x"]).unwrap().2.as_deref(), Some("x"));
        assert_eq!(parse(&["--value", "x"]).unwrap().2.as_deref(), Some("x"));
        let err = parse(&["--value", "--flag"]).unwrap_err();
        assert!(
            err.contains("needs a value"),
            "a flag is not a value: {err}"
        );
        let err = parse(&["--value="]).unwrap_err();
        assert!(
            err.contains("needs a value"),
            "an empty `=` is no value: {err}"
        );
        // A switch given a value it does not take is refused rather than half-read.
        let err = parse(&["--flag=yes"]).unwrap_err();
        assert!(err.contains("`--flag` takes no value"), "{err}");

        // At most `max` positionals: the extra one is named, with the verb's usage line.
        let err = parse(&["a", "b", "c"]).unwrap_err();
        assert!(err.contains("unexpected argument `c`"), "{err}");
        assert!(err.contains(&crate::usage_line("build")), "{err}");
    }

    /// `build --output x.html` got a bare "unknown flag `--output`": it is three edits from
    /// `--out`, past the did-you-mean's reach, though it plainly extends it (audit
    /// 2026-09-24, first-hour #15). The verb names already answer a name that extends or
    /// abbreviates one of theirs (`preview-site`); flags now take the same rule. With no
    /// suggestion at all, the error says where the flags are listed.
    #[test]
    fn an_unknown_flag_that_extends_a_known_one_suggests_it() {
        let err = parse(&["--values-file"]).unwrap_err();
        assert!(err.contains("did you mean `--value`"), "{err}");
        let err = parse(&["--fl"]).unwrap_err();
        assert!(err.contains("did you mean `--flag`"), "{err}");
        // Ambiguous is no answer: `--` opens both flags.
        let err = parse(&["--"]).unwrap_err();
        assert!(!err.contains("did you mean"), "{err}");
        // Nothing close: point at the verb's own help instead of guessing.
        let err = parse(&["-o"]).unwrap_err();
        assert!(
            !err.contains("did you mean") && err.contains("`taliesin build --help`"),
            "{err}"
        );
    }
}

#[cfg(test)]
mod content_type_tests {
    use super::content_type;
    use std::path::Path;

    /// A static page or text file a project ships displays in the preview as it does on
    /// any static host. With no arm for either, `widget.html` came back as
    /// `application/octet-stream`, so the browser downloaded it and an `<iframe>` of it
    /// broke in the preview only.
    #[test]
    fn a_static_page_and_a_text_file_display_instead_of_downloading() {
        for (name, want) in [
            ("widget.html", "text/html; charset=utf-8"),
            ("OLD.HTM", "text/html; charset=utf-8"),
            ("notes.txt", "text/plain; charset=utf-8"),
        ] {
            assert_eq!(content_type(Path::new(name)), want, "{name}");
        }
        // Anything unknown still falls back to the generic binary type.
        assert_eq!(
            content_type(Path::new("blob.bin")),
            "application/octet-stream"
        );
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
mod static_asset_tests {
    use super::serve_asset_from;
    use axum::http::StatusCode;
    use std::fs;

    /// The preview's static handler applies the build's publication rule. It served every
    /// file under its root, and a loose document's root is its parent directory, so
    /// `preview ~/scratch.tmd` handed `.git/config` and every other dotfile under `$HOME`
    /// to any local process. A `.`-prefixed component is private everywhere, including
    /// through a symlink; an `_`-prefixed folder is not, because a page may reference an
    /// image there and the build ships it.
    #[test]
    #[cfg(unix)]
    fn the_preview_never_serves_a_private_path() {
        let root = std::env::temp_dir().join(format!("tali-static-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in [".git", ".private", "_images", "img"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        for f in [
            ".git/config",
            ".private/key",
            ".env",
            "_images/hero.png",
            "img/a.png",
        ] {
            fs::write(root.join(f), b"x").unwrap();
        }
        std::os::unix::fs::symlink(".git", root.join("vendor")).unwrap();
        let status = |rel: &str| serve_asset_from(&root, rel).status();

        for private in [
            ".git/config",
            ".private/key",
            ".env",
            "img/../.env",
            "vendor/config",
        ] {
            assert_eq!(status(private), StatusCode::NOT_FOUND, "{private}");
        }
        assert_eq!(status("img/a.png"), StatusCode::OK);
        assert_eq!(status("_images/hero.png"), StatusCode::OK);
        let _ = fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod takeover_tests {
    use super::{PROBE_TIMEOUT, identify, plausible_pid};
    use std::time::Instant;

    /// The takeover signals a pid only if that pid owns the socket listening on the port
    /// that answered. Being some process of this user, or even of this binary, is not it.
    #[test]
    #[cfg(target_os = "linux")]
    fn only_the_process_listening_on_the_port_holds_it() {
        use super::holds_the_port;
        let me = std::process::id() as i32;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(holds_the_port(me, port), "this process listens on {port}");

        // Another live process of this user, holding no socket at all.
        let mut other = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        let other_holds = holds_the_port(other.id() as i32, port);
        let _ = other.kill();
        let _ = other.wait();
        assert!(
            !other_holds,
            "a process that does not listen on {port} does not hold it"
        );

        // The same port number on another address is not the port the probe reached: a
        // probe to 127.0.0.1 is never answered from 127.0.0.2.
        let elsewhere = std::net::TcpListener::bind("127.0.0.2:0").unwrap();
        let other_port = elsewhere.local_addr().unwrap().port();
        assert!(
            !holds_the_port(me, other_port),
            "a listener on 127.0.0.2:{other_port} does not hold 127.0.0.1:{other_port}"
        );

        // A port that is no longer listening is held by nobody.
        drop(listener);
        assert!(
            !holds_the_port(me, port),
            "{port} closed, so no one holds it"
        );
    }

    /// Any local process can hold a port in the fallback range and answer the identity
    /// probe, so the reply is untrusted input, and its size is part of that. A holder that
    /// streams bytes forever drove the probe past a 2 GB memory cap in about a second (audit
    /// 2026-09-24, A4), and ten probes run at once. The probe must stop reading at a cap far
    /// above a real answer (under 300 bytes) and give up at once, not at the timeout.
    #[tokio::test]
    async fn the_identity_probe_stops_reading_an_endless_reply() {
        use std::io::{Read, Write};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU64, Ordering};

        // Bounded, so a regression costs this test BUDGET bytes of memory rather than
        // whatever loopback can deliver before the probe's timeout.
        const BUDGET: u64 = 256 << 20;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let sent = Arc::new(AtomicU64::new(0));
        let counter = sent.clone();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = listener.accept() else {
                return;
            };
            let _ = s.read(&mut [0u8; 1024]);
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n");
            let chunk = vec![b' '; 1 << 16];
            while counter.load(Ordering::SeqCst) < BUDGET {
                if s.write_all(&chunk).is_err() {
                    return;
                }
                counter.fetch_add(chunk.len() as u64, Ordering::SeqCst);
            }
            // Hold the connection open: only a read cap, never an EOF, may end the probe.
            std::thread::sleep(PROBE_TIMEOUT * 2);
        });

        let started = Instant::now();
        assert!(
            identify(port).await.is_none(),
            "a stream of spaces is not a preview"
        );
        let took = started.elapsed();
        let streamed = sent.load(Ordering::SeqCst);
        assert!(
            streamed < 48 << 20,
            "the probe kept reading an endless reply: {} MiB went into it",
            streamed >> 20
        );
        assert!(
            took < PROBE_TIMEOUT / 2,
            "the probe waited out its timeout ({took:?}) instead of stopping at its read cap"
        );
    }

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
