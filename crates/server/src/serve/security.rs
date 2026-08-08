//! The preview's two request guards: the websocket origin check and the DNS-rebinding
//! `Host` check. The preview binds loopback only, so both are about a *local* peer or a
//! page in the author's own browser, never a remote one. `use super::*` reaches the axum
//! Router/middleware types and Arc from serve/mod.rs.

use super::*;

/// Whether a websocket upgrade carrying this `Origin` may connect, given the
/// request's `Host`. The control channel (restart kernel, etc.) lives on the
/// websocket, so a page on another site must not be able to open it against your
/// dev server (a browser always sends `Origin`, so this blocks cross-site driving
/// without affecting non-browser clients, which send none).
///
/// A loopback origin is trusted: the preview binds loopback only, so a page at another
/// local port (a second dev server, the editor companion) is a local peer of the author's
/// own machine.
pub(crate) fn origin_allowed(origin: Option<&str>, host: Option<&str>) -> bool {
    let Some(origin) = origin else {
        return true; // no Origin => not a browser => not a cross-site request
    };
    // The part after the scheme is the authority (host[:port]).
    let authority = origin.split_once("://").map_or(origin, |(_, rest)| rest);
    if Some(authority) == host {
        return true; // same origin
    }
    let host_only = authority.split(':').next().unwrap_or("");
    matches!(host_only, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

/// Apply [`origin_allowed`] to a request's headers; the websocket handler gates the
/// upgrade on this. It is the only thing stopping a page on another site from opening the
/// control channel and sending `restart_kernel`, so it must never become a no-op.
pub(crate) fn ws_origin_ok(headers: &axum::http::HeaderMap) -> bool {
    use axum::http::header::{HOST, ORIGIN};
    let origin = headers.get(ORIGIN).and_then(|v| v.to_str().ok());
    let host = headers.get(HOST).and_then(|v| v.to_str().ok());
    origin_allowed(origin, host)
}

/// Whether a request's `Host` header names THIS server: the standard DNS-rebinding
/// defense. The origin check compares `Origin` to `Host`, but under a rebinding attack
/// (a page at evil.example rebinds its DNS to 127.0.0.1 and reaches the loopback
/// preview) both headers are the attacker's domain and match, so the origin check alone
/// cannot see it. Validating `Host` against a fixed allowlist instead of the
/// (equally-attacker-controlled) `Origin` is what closes it. The preview binds loopback
/// only, so a loopback name is the whole allowlist. A missing `Host` is allowed: only a
/// browser can mount a rebind and it always sends one.
pub(crate) fn host_allowed(host: Option<&str>) -> bool {
    let Some(host) = host else {
        return true; // no Host => not a browser => can't be a rebinding attack
    };
    matches!(host_name(host), "localhost" | "127.0.0.1" | "::1")
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

/// Axum middleware enforcing [`host_allowed`], the DNS-rebinding defense. Unconditional:
/// a rebinding read works against the loopback preview (whose HTTP routes are otherwise
/// ungated), which is exactly the case it exists for.
pub(crate) async fn host_guard(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let allowed = {
        let host = req
            .headers()
            .get(axum::http::header::HOST)
            .and_then(|v| v.to_str().ok());
        host_allowed(host)
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

/// Wrap a router with the [`host_guard`]. Always installed: the loopback preview is
/// exactly what it protects.
pub(crate) fn with_host_guard(router: Router) -> Router {
    router.layer(axum::middleware::from_fn(host_guard))
}

#[cfg(test)]
mod tests {
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
        // Loopback is allowed regardless of port (a second local dev server, the
        // editor companion).
        assert!(origin_allowed(
            Some("http://127.0.0.1:9999"),
            Some("localhost:4388")
        ));
        // The attack: a malicious page open in your browser tries to drive your dev
        // server's control channel (`restart_kernel`). Blocked.
        assert!(!origin_allowed(
            Some("http://evil.example"),
            Some("localhost:4388")
        ));
        assert!(!origin_allowed(
            Some("https://evil.example:4388"),
            Some("localhost:4388")
        ));
        // A `null` origin (sandboxed iframe / file://) can't control the server.
        assert!(!origin_allowed(Some("null"), Some("localhost:4388")));
    }

    #[test]
    fn host_guard_allows_this_server_and_blocks_dns_rebinding() {
        // The author's own browser.
        assert!(host_allowed(Some("localhost:4388")));
        assert!(host_allowed(Some("127.0.0.1:4388")));
        assert!(host_allowed(Some("[::1]:4388")));
        assert!(host_allowed(Some("localhost"))); // no port
        // A non-browser client (curl with no -H) sends no Host and can't mount a rebind.
        assert!(host_allowed(None));
        // The DNS-rebinding attack: a page at evil.example rebinds it to 127.0.0.1 and
        // reaches the loopback preview with Host = evil.example. origin_allowed passes
        // (Origin == Host); the Host allowlist is the only thing that stops it. Blocked.
        assert!(!host_allowed(Some("evil.example:4388")));
        assert!(!host_allowed(Some("evil.example")));
        // A host that merely *contains* a loopback name is not loopback.
        assert!(!host_allowed(Some("127.0.0.1.evil.example:4388")));
        assert!(!host_allowed(Some("localhost.evil.example")));
    }
}
