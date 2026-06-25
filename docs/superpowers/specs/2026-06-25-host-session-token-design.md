# Per-session access token for `--host` (LAN-snooping defense)

## Problem

`--host` binds `0.0.0.0`, so the live preview is reachable by anyone on the same LAN
(coffee-shop / office WiFi). Before this change the only gate was the WS `Origin`
check (`origin_allowed`), which stops a *cross-site* page from driving the control
channel but does nothing about a LAN peer who simply opens the URL: they could read
your unpublished draft and hit "Restart kernel". This is the last security item
before any OSS release.

## Threat model

A passive/active peer on the same LAN. The defense is a per-session secret that only
someone who can see your screen (to scan the QR) or whom you hand the URL to can
obtain. Not in scope: WAN exposure / tunnels (would want TLS + rate-limiting), or an
attacker who can already read your loopback traffic (they own the machine).

## Design

A per-session token (random UUID, regenerated every server start) gates **non-loopback**
access to **every** route — page, assets, `/ws`. Key properties:

- **Loopback is always exempt.** `ConnectInfo<SocketAddr>` gives the peer IP; if
  `ip().is_loopback()` the request is served with no token. So the author's localhost
  browsing, auto-open, bookmarks, and the localhost VS Code companion are **completely
  unchanged** — even with `--host` active.
- **The token rides the URL, then a cookie.** The advertised LAN URL/QR is
  `http://<ip>:<port>/?t=<token>`. The first load from the QR presents `?t=` → the
  guard serves it **and** sets `qmd_token=<token>; Path=/; SameSite=Lax`. Every
  subsequent same-origin request (assets, the `/ws` upgrade, reloads) carries the
  cookie automatically. **Result: zero client.js changes** — cookies thread the auth
  through the existing client untouched.
- **Fails closed.** A LAN peer with no/incorrect token gets `403` on everything,
  including `/ws` and the page itself, so the draft content is never served.
- **Off without `--host`.** No token is generated and the guard middleware is never
  installed, so loopback-only previews keep byte-for-byte prior behavior.

## Why gate all routes, not just `/ws`

The backlog framed this as "LAN-snooping defense". Gating only the control channel
(`/ws`) would still let a peer *read* the rendered draft over `/`. The threat is
reading the draft, so the page + assets are gated too.

## Implementation

`crates/server/src/serve.rs` (shared, reused by `serve_site.rs`):
- `new_session_token()` → random UUID (the `uuid` crate was already a dependency).
- `lan_access(peer_loopback, query, cookie, token) -> LanAccess{Allow,AllowSetCookie,Deny}`
  — the pure decision, unit-tested. `query_token`/`cookie_token` parse `t=` / `qmd_token=`.
- `lan_token_guard` — axum `from_fn` middleware applying `lan_access`, setting the cookie
  on an `AllowSetCookie`.
- `with_lan_guard(router, Option<token>)` — installs the layer only when a token exists.
- `lan_url(base, Option<token>)` — appends `/?t=<token>` for the QR/console URL.

Both servers: generate `token = expose.then(new_session_token)`, wrap the router with
`with_lan_guard`, advertise via `lan_url`, and serve with
`into_make_service_with_connect_info::<SocketAddr>()` (so the guard sees the peer IP).

## Verification

- Unit: `lan_access` matrix (loopback/allow, LAN-deny, wrong-token-deny, `?t=`→set-cookie
  incl. `page=…&t=…`, cookie→allow), `lan_url` with/without token, token uniqueness.
- End-to-end (curl against a real `--host` server, debug binary):
  loopback no-token `200`; LAN no-token `403`; LAN wrong-token `403`; LAN `?t=` `200`
  + `Set-Cookie`; LAN cookie `200`; LAN `/ws` no-token `403`; LAN `/favicon.ico`
  no-token `403`; the draft body ("Secret") is **absent** without the token, present
  with it.
- Browser (chrome-devtools, loopback): page renders, ws status `live`, no console
  errors, no cookie set (loopback exempt) — the author flow is intact under `--host`.
