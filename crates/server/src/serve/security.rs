//! LAN-exposure security: origin checking + the per-session access token that gates
//! non-loopback (`--host`) requests. Loopback is always allowed; a remote request must
//! present the session token (via `?t=` -> cookie). `use super::*` reaches the axum
//! Router/middleware types and Arc from serve/mod.rs.

use super::*;

/// Whether a websocket upgrade carrying this `Origin` may connect, given the
/// request's `Host`. The control channel (restart kernel, etc.) lives on the
/// websocket, so a page on another site must not be able to open it against your
/// dev server (a browser always sends `Origin`, so this blocks cross-site driving
/// without affecting non-browser clients, which send none).
///
/// `allow_loopback_origins` is `true` for a loopback-bound preview (the default),
/// where a page at another local port (a second dev server, the editor companion)
/// is a trusted local peer. It is `false` under `--host`: there a page at
/// `http://localhost:X` open in the author's *own* browser is a loopback *peer*
/// (so the LAN token guard waves it through) yet may be hostile, so it must be
/// same-origin to drive the control channel.
pub(crate) fn origin_allowed(
    origin: Option<&str>,
    host: Option<&str>,
    allow_loopback_origins: bool,
) -> bool {
    let Some(origin) = origin else {
        return true; // no Origin => not a browser => not a cross-site request
    };
    // The part after the scheme is the authority (host[:port]).
    let authority = origin.split_once("://").map_or(origin, |(_, rest)| rest);
    if Some(authority) == host {
        return true; // same origin (covers the LAN case: phone dials the Host it sees)
    }
    if !allow_loopback_origins {
        return false; // --host: only same-origin drives the ws
    }
    let host_only = authority.split(':').next().unwrap_or("");
    matches!(host_only, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

/// Apply [`origin_allowed`] to a request's headers; both websocket handlers gate
/// the upgrade on this. `allow_loopback_origins` is the server's `loopback_bound`
/// flag (false under `--host`).
pub(crate) fn ws_origin_ok(headers: &axum::http::HeaderMap, allow_loopback_origins: bool) -> bool {
    use axum::http::header::{HOST, ORIGIN};
    let origin = headers.get(ORIGIN).and_then(|v| v.to_str().ok());
    let host = headers.get(HOST).and_then(|v| v.to_str().ok());
    origin_allowed(origin, host, allow_loopback_origins)
}

/// Whether a request's `Host` header names THIS server: the standard DNS-rebinding
/// defense. The origin check compares `Origin` to `Host`, but under a rebinding attack
/// (a page at evil.example rebinds its DNS to 127.0.0.1 and reaches the loopback
/// preview) both headers are the attacker's domain and match, so the origin check alone
/// cannot see it. Validating `Host` against a fixed allowlist instead of the
/// (equally-attacker-controlled) `Origin` is what closes it. Loopback names are always
/// this server; under `--host` the bound LAN IP is too (the phone dials it). A missing
/// `Host` is allowed: only a browser can mount a rebind and it always sends one.
pub(crate) fn host_allowed(host: Option<&str>, lan_ip: Option<&str>) -> bool {
    let Some(host) = host else {
        return true; // no Host => not a browser => can't be a rebinding attack
    };
    let name = host_name(host);
    matches!(name, "localhost" | "127.0.0.1" | "::1") || lan_ip == Some(name)
}

/// The host portion of a `Host` header value, dropping an optional `:port` and IPv6
/// brackets: `localhost:4388` -> `localhost`, `[::1]:4388` -> `::1`, `192.168.1.5:4388`
/// -> `192.168.1.5`.
fn host_name(host: &str) -> &str {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest); // `[::1]:4388` -> `::1`
    }
    host.rsplit_once(':').map_or(host, |(h, _)| h)
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
    /// Serve it (loopback peer, or a valid `tali_token` cookie already present).
    Allow,
    /// Serve it and set the session cookie (a valid `?t=` token — e.g. the first load
    /// from the QR), so later same-origin asset/ws requests authenticate by cookie and
    /// no longer need the token in the URL.
    AllowSetCookie,
    /// Reject: a non-loopback peer with no/incorrect token.
    Deny,
}

/// The `Set-Cookie` value for the session token. Session-scoped: a new token each
/// server start, so a stale cookie just fails closed and the author re-scans.
/// `HttpOnly` (the page never needs to read it from JS — it authenticates by riding
/// requests), `SameSite=Lax` + `Path=/` so it rides every same-origin asset/ws
/// request from the page.
fn session_cookie(token: &str) -> String {
    format!("tali_token={token}; Path=/; SameSite=Lax; HttpOnly; Max-Age=86400")
}

/// The `t=` value of a URL query string, if present.
fn query_token(query: &str) -> Option<&str> {
    query.split('&').find_map(|kv| kv.strip_prefix("t="))
}

/// The `tali_token` value of a `Cookie` header, if present.
fn cookie_token(cookie: &str) -> Option<&str> {
    cookie
        .split(';')
        .find_map(|kv| kv.trim_start().strip_prefix("tali_token="))
}

/// Decide LAN access for one request. Loopback is always allowed; a LAN peer must
/// present the token in the `?t=` query (→ set a cookie) or the `tali_token` cookie.
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
            "taliesin: this --host preview needs its session link. Scan the QR code or \
             open the printed LAN URL (it carries the access token).",
        )
            .into_response(),
        LanAccess::Allow => next.run(req).await,
        LanAccess::AllowSetCookie => {
            let mut resp = next.run(req).await;
            let value = session_cookie(&token);
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

/// Axum middleware enforcing [`host_allowed`], the DNS-rebinding defense. Unlike the LAN
/// token guard this is installed in BOTH modes: a rebinding read works even against the
/// default loopback preview (whose HTTP routes are otherwise ungated), so the guard is
/// unconditional. `lan_ip` is `Some` only under `--host`, adding the bound LAN IP to the
/// allowlist.
pub(crate) async fn host_guard(
    lan_ip: Option<Arc<str>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let allowed = {
        let host = req
            .headers()
            .get(axum::http::header::HOST)
            .and_then(|v| v.to_str().ok());
        host_allowed(host, lan_ip.as_deref())
    };
    if allowed {
        next.run(req).await
    } else {
        (
            axum::http::StatusCode::FORBIDDEN,
            "taliesin: refused (the Host header does not name this preview server; this is \
             the DNS-rebinding guard).",
        )
            .into_response()
    }
}

/// Wrap a router with the [`host_guard`]. Always installed (loopback previews need it
/// too), so unlike [`with_lan_guard`] it takes no `Option<Router>` short-circuit.
pub(crate) fn with_host_guard(router: Router, lan_ip: Option<Arc<str>>) -> Router {
    router.layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let lan_ip = lan_ip.clone();
            async move { host_guard(lan_ip, req, next).await }
        },
    ))
}

/// The LAN URL to advertise (QR + console): the base plus the session token in `?t=`
/// when one exists, so the first load authenticates and sets the cookie.
pub(crate) fn lan_url(base: &str, token: Option<&Arc<str>>) -> String {
    match token {
        Some(t) => format!("{base}/?t={t}"),
        None => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_check_allows_same_origin_and_blocks_cross_site() {
        // A loopback-bound preview (the default): loopback origins are trusted local
        // peers.
        let loopback_bound = true;
        // No Origin header (curl / websocat — not a browser) can't be a cross-site
        // request, so it's allowed.
        assert!(origin_allowed(None, Some("localhost:4388"), loopback_bound));
        // A same-origin browser connection is allowed.
        assert!(origin_allowed(
            Some("http://localhost:4388"),
            Some("localhost:4388"),
            loopback_bound
        ));
        // The `--host` LAN case: the phone's Origin is the Host it dialed -> allowed.
        assert!(origin_allowed(
            Some("http://192.168.1.5:4388"),
            Some("192.168.1.5:4388"),
            loopback_bound
        ));
        // Loopback is allowed regardless of port (a second local dev server).
        assert!(origin_allowed(
            Some("http://127.0.0.1:9999"),
            Some("localhost:4388"),
            loopback_bound
        ));
        // The attack: a malicious page open in your browser tries to drive your dev
        // server's control channel. Blocked.
        assert!(!origin_allowed(
            Some("http://evil.example"),
            Some("localhost:4388"),
            loopback_bound
        ));
        assert!(!origin_allowed(
            Some("https://evil.example:4388"),
            Some("192.168.1.5:4388"),
            loopback_bound
        ));
        // A `null` origin (sandboxed iframe / file://) can't control the server.
        assert!(!origin_allowed(
            Some("null"),
            Some("localhost:4388"),
            loopback_bound
        ));
    }

    #[test]
    fn host_mode_drops_the_loopback_origin_blanket_allow() {
        // Under `--host` (LAN-bound), only same-origin drives the ws. The phone (its
        // Origin is the LAN Host it dialed) still connects...
        assert!(origin_allowed(
            Some("http://192.168.1.5:4388"),
            Some("192.168.1.5:4388"),
            false
        ));
        // ...and a non-browser client (no Origin) is unaffected.
        assert!(origin_allowed(None, Some("192.168.1.5:4388"), false));
        // But a page at http://localhost:X open in the author's OWN browser is a
        // loopback peer (so the LAN token guard waves it through) — the origin check
        // is the only thing that stops it driving the control channel. Blocked.
        assert!(!origin_allowed(
            Some("http://localhost:9999"),
            Some("192.168.1.5:4388"),
            false
        ));
        assert!(!origin_allowed(
            Some("http://127.0.0.1:9999"),
            Some("192.168.1.5:4388"),
            false
        ));
    }

    #[test]
    fn session_cookie_is_httponly_and_scoped() {
        let c = session_cookie("abc123");
        assert!(c.contains("tali_token=abc123"));
        assert!(c.contains("HttpOnly"), "cookie must be HttpOnly: {c}");
        assert!(c.contains("SameSite=Lax"));
        assert!(c.contains("Path=/"));
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
            lan_access(false, Some("t=nope"), Some("tali_token=nope"), tok),
            LanAccess::Deny
        ));
        // First load from the QR carries `?t=<token>` -> allowed, and we set the cookie.
        assert!(matches!(
            lan_access(false, Some("t=abc123"), None, tok),
            LanAccess::AllowSetCookie
        ));
        // A `?t=` among other params still authenticates (e.g. `?page=x&t=abc123`).
        assert!(matches!(
            lan_access(false, Some("page=intro.tmd&t=abc123"), None, tok),
            LanAccess::AllowSetCookie
        ));
        // Subsequent same-origin asset/ws requests carry the cookie -> allowed.
        assert!(matches!(
            lan_access(false, None, Some("tali_token=abc123"), tok),
            LanAccess::Allow
        ));
        // A cookie among other cookies still authenticates.
        assert!(matches!(
            lan_access(false, None, Some("other=1; tali_token=abc123"), tok),
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

    #[test]
    fn host_guard_allows_this_server_and_blocks_dns_rebinding() {
        // Loopback preview (default): no LAN IP, only loopback names are this server.
        let no_lan: Option<&str> = None;
        // The author's own browser.
        assert!(host_allowed(Some("localhost:4388"), no_lan));
        assert!(host_allowed(Some("127.0.0.1:4388"), no_lan));
        assert!(host_allowed(Some("[::1]:4388"), no_lan));
        assert!(host_allowed(Some("localhost"), no_lan)); // no port
        // A non-browser client (curl with no -H) sends no Host and can't mount a rebind.
        assert!(host_allowed(None, no_lan));
        // The DNS-rebinding attack: a page at evil.example rebinds it to 127.0.0.1 and
        // reaches the loopback preview with Host = evil.example. origin_allowed passes
        // (Origin == Host); the Host allowlist is the only thing that stops it. Blocked.
        assert!(!host_allowed(Some("evil.example:4388"), no_lan));
        assert!(!host_allowed(Some("evil.example"), no_lan));
        // A host that merely *contains* a loopback name is not loopback.
        assert!(!host_allowed(Some("127.0.0.1.evil.example:4388"), no_lan));
        assert!(!host_allowed(Some("localhost.evil.example"), no_lan));

        // --host: the bound LAN IP is additionally a legitimate Host (the phone dials
        // it); the attacker domain and other LAN IPs still are not.
        let lan = Some("192.168.1.5");
        assert!(host_allowed(Some("192.168.1.5:4388"), lan));
        assert!(host_allowed(Some("127.0.0.1:4388"), lan)); // author still uses localhost
        assert!(!host_allowed(Some("evil.example:4388"), lan));
        assert!(!host_allowed(Some("192.168.1.99:4388"), lan));
    }
}
