//! The preview's request guards: the websocket origin check, the DNS-rebinding `Host`
//! check, and the Fetch Metadata check that stops another site including a response. The
//! preview binds loopback only, so all three are about a *local* peer or a page in the
//! author's own browser, never a remote one. `use super::*` reaches the axum
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
    // Same extraction as the `Host` check below — one spelling of "the host part of an
    // authority", so the two guards cannot disagree about what loopback is. A hand-rolled
    // `split(':').next()` lived here and could never return a string containing a colon,
    // which made the `::1` arm unreachable for every input: `[::1]:9999` came out as `"["`.
    matches!(
        host_name(authority),
        Some("localhost" | "127.0.0.1" | "::1")
    )
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
    matches!(host_name(host), Some("localhost" | "127.0.0.1" | "::1"))
}

/// The host portion of an authority (a `Host` header value, or an origin's part after the
/// scheme), dropping an optional `:port` and IPv6 brackets: `localhost:4388` ->
/// `localhost`, `[::1]:4388` -> `::1`, `192.168.1.5:4388` -> `192.168.1.5`. `None` when
/// anything but a `:port` of digits follows the host, so `[localhost]evil.example` and
/// `localhost:4388evil` name no host at all rather than a loopback one.
fn host_name(authority: &str) -> Option<&str> {
    let (host, port) = match authority.strip_prefix('[') {
        Some(rest) => {
            let (host, after) = rest.split_once(']')?; // `[::1]:4388` -> `::1`, `:4388`
            if after.is_empty() {
                (host, "")
            } else {
                (host, after.strip_prefix(':')?)
            }
        }
        None => authority.split_once(':').unwrap_or((authority, "")),
    };
    port.bytes().all(|b| b.is_ascii_digit()).then_some(host)
}

/// Why the preview refuses a request with these headers, or `None` to serve it: the one
/// decision [`host_guard`] enforces, kept pure so a test can put headers to it.
fn refusal(headers: &axum::http::HeaderMap) -> Option<&'static str> {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok());
    if !host_allowed(host) {
        return Some(
            "taliesin: refused (the Host header does not name this preview server; this is \
             the DNS-rebinding guard).",
        );
    }
    let metadata = |name| headers.get(name).and_then(|v| v.to_str().ok());
    if !fetch_site_allowed(metadata("sec-fetch-site"), metadata("sec-fetch-mode")) {
        return Some(
            "taliesin: refused (a page on another site may link to this preview, but not \
             load it as part of itself).",
        );
    }
    None
}

/// Whether a request's Fetch Metadata lets it be served. A page on another site, or on
/// another port of this one (`same-site`: a site ignores the port), may NAVIGATE here: a
/// link, a prefetch, or the editor companion's webview `<iframe>`, which is `cross-site`
/// because the webview is its own origin. It may not load a response as part of itself.
/// A browser runs a `<script src>` from any origin, and `/search-index.js` assigns every
/// page's text, drafts included, to a global the including page can read.
///
/// `same-origin` is the preview's own page and `none` the address bar. No header means no
/// browser, or one too old to send it. The websocket upgrade carries none either: the
/// origin check guards it.
fn fetch_site_allowed(site: Option<&str>, mode: Option<&str>) -> bool {
    !matches!(site, Some("cross-site" | "same-site")) || mode == Some("navigate")
}

/// Axum middleware enforcing [`refusal`], starting with [`host_allowed`], the DNS-rebinding
/// defense. Unconditional: a rebinding read works against the loopback preview (whose HTTP
/// routes are otherwise ungated), which is exactly the case it exists for.
pub(crate) async fn host_guard(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    match refusal(req.headers()) {
        None => next.run(req).await,
        Some(why) => (axum::http::StatusCode::FORBIDDEN, why).into_response(),
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
    fn the_origin_check_reads_a_bracketed_ipv6_authority_like_the_host_check_does() {
        // `authority.split(':').next()` can never yield a string containing a colon, so
        // the loopback allowlist's `::1` arm was unreachable for EVERY input, IPv6 or
        // not: `[::1]:9999` came out as `"["`. Both guards now extract the host the same
        // way (`host_name`), so "a loopback origin is trusted" is true as written and
        // there is only one spelling of "the host part of an authority" in this file.
        assert!(origin_allowed(
            Some("http://[::1]:9999"),
            Some("localhost:4388")
        ));
        assert!(origin_allowed(Some("http://[::1]"), Some("localhost:4388")));
        // And a non-loopback IPv6 literal is still refused — the bracket must not become
        // a blanket pass.
        assert!(!origin_allowed(
            Some("http://[2001:db8::1]:9999"),
            Some("localhost:4388")
        ));
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

    /// Another site may navigate to the preview but not load it as part of itself. A
    /// browser runs a `<script src>` from any origin, and `/search-index.js` assigns every
    /// page's text, drafts included, to a global the including page reads: a page served
    /// from `localhost:5128` read a draft's canary from a preview on `127.0.0.1` (audit
    /// 2026-09-24, Part H), and so did one on another port of `127.0.0.1` (`same-site`, since
    /// a site ignores the port). The header sets below are what Chrome sent, measured
    /// 2026-09-24 against a logging stand-in for the preview.
    #[test]
    fn another_site_may_navigate_to_the_preview_but_not_include_it() {
        use axum::http::{HeaderMap, HeaderName, HeaderValue};
        let refused = |metadata: &[(&'static str, &str)]| {
            let mut h = HeaderMap::new();
            h.insert(
                axum::http::header::HOST,
                HeaderValue::from_static("127.0.0.1:4321"),
            );
            for (name, value) in metadata {
                h.insert(
                    HeaderName::from_static(name),
                    HeaderValue::from_str(value).unwrap(),
                );
            }
            refusal(&h).is_some()
        };
        let fetch = |site, mode, dest| {
            refused(&[
                ("sec-fetch-site", site),
                ("sec-fetch-mode", mode),
                ("sec-fetch-dest", dest),
            ])
        };

        // The inclusion: a script or an image, from another site or another local port.
        assert!(
            fetch("cross-site", "no-cors", "script"),
            "cross-site <script src>"
        );
        assert!(
            fetch("same-site", "no-cors", "script"),
            "same-site <script src>"
        );
        assert!(
            fetch("cross-site", "no-cors", "image"),
            "cross-site <img src>"
        );
        assert!(fetch("cross-site", "cors", "empty"), "cross-site fetch()");

        // Navigations stay open: a link from another site, and the editor companion's
        // webview, whose `<iframe>` is cross-site because the webview is its own origin.
        assert!(
            !fetch("cross-site", "navigate", "document"),
            "a link from elsewhere"
        );
        assert!(
            !fetch("cross-site", "navigate", "iframe"),
            "the VS Code webview"
        );
        // The preview's own page and its parts, the address bar, and a prefetch (which
        // Chrome sends as `none` with `Sec-Purpose: prefetch`).
        assert!(!fetch("same-origin", "no-cors", "script"), "its own script");
        assert!(!fetch("same-origin", "cors", "empty"), "its own fetch()");
        assert!(
            !fetch("none", "navigate", "document"),
            "the address bar, a prefetch"
        );
        // No Fetch Metadata: curl, the takeover's identity probe, and the websocket
        // upgrade, which Chrome sends without it (the origin check guards that one).
        assert!(!refused(&[]), "a request with no Fetch Metadata");
    }

    /// An authority is a host and an optional `:port`, nothing else. The bracket branch
    /// kept whatever preceded `]` and ignored the rest, and the other branch kept whatever
    /// preceded the last `:`, so `[localhost]evil.example` and `localhost:4388evil` both
    /// read as loopback, in the `Host` check and the `Origin` check alike. No browser sends
    /// either (audit 2026-09-24, security info 10), so this is hygiene, not an exploit.
    #[test]
    fn an_authority_is_a_host_and_an_optional_port_and_nothing_else() {
        for bogus in [
            "[localhost]evil.example",
            "[::1]evil.example",
            "[::1]:4388evil",
            "localhost:4388evil",
            "localhost:evil.example",
        ] {
            assert!(!host_allowed(Some(bogus)), "Host {bogus}");
            assert!(
                !origin_allowed(Some(&format!("http://{bogus}")), Some("127.0.0.1:4388")),
                "Origin http://{bogus}"
            );
        }
        // The real spellings still pass, with and without a port.
        for real in [
            "localhost",
            "localhost:4388",
            "127.0.0.1:4388",
            "[::1]",
            "[::1]:4388",
        ] {
            assert!(host_allowed(Some(real)), "Host {real}");
            assert!(
                origin_allowed(Some(&format!("http://{real}")), Some("127.0.0.1:4388")),
                "Origin http://{real}"
            );
        }
    }
}
