# Pre-open-source security + supply-chain audit, 2026-07-17

> **Remediation status (2026-07-17, same day).** All four real findings are fixed on local
> `main`, plus the trivial supply-chain items:
> - **PT-1** symlink escape — `safe_join` canonicalizes + re-checks (`e981f20`).
> - **NET-1** DNS-rebinding — positive `Host`-header allowlist middleware (`2fe4add`).
> - **OUT-1** URL-scheme sanitizer on the markdown link/image path (`99ffdc8`).
> - **PT-2** single-doc includes/resources confined to an explicit root (`9359a2c`): a new
>   `safe_join_in(base, rel, root)`; preview/build/render/read/map/check pass the doc's own
>   dir; the site path keeps the `_site.yml`-bounded walk. Threaded fix (not the deferred
>   "accept + document" option), per owner decision. A standalone doc can no longer read
>   above its own folder (a refused include surfaces the located "escapes the project root"
>   warning).
> - **DEP-02** `anyhow` bumped; **DEP-01** `scc` unsound advisory gated + documented in
>   `deny.toml` (`unsound = "all"`, `78b26e7`). `SECURITY.md` added; `/home/bogo` paths
>   scrubbed (`127038c`).
>
> **Deferred to the pre-public cleanup (repo goes public ~2026-08, owner may start fresh):**
> the remaining info/hardening items below (DEP-03 mermaid bump, CMD-01 boot log, dos-rich /
> dos-ws-size caps, dos-pages eviction) and the `oss-*` checklist (notes/ prune decision).
> `dos-pages` and the notes/ prune are **not** done. `check` also confines now (single-doc).

Report-only. A multi-agent audit (7 read-only finders, one per attack class; each finding
independently adversarially verified) run before making the repo public. Threat model:
**A1** a malicious web page the author visits while a preview runs, **A2** an untrusted `.tmd`
project the author opens/builds, **A3** a LAN attacker under `--host`, **A4** a reader of the
built/published static HTML. Cell code execution is BY DESIGN and disclosed, so "cells run
code" is not counted as a vuln.

## Headline

**Nothing security-critical blocks the release, and the existing posture is genuinely strong.**
Every finding a finder rated *medium* was **downgraded to low or info under adversarial
verification** (each verifier read the actual source; one built a repro). No critical/high/medium
survived. The dev server already has a real security model: loopback-always / LAN-token-gated
access, a per-session UUID token (`HttpOnly`, `SameSite=Lax`), a websocket origin check that
handles the tricky `--host` localhost-origin case, terminal-injection defanging, `deny.toml` +
`cargo-audit` in CI, and a HTTP-asset path that already canonicalizes against traversal.

What remains is **four genuine low-severity hardening items** and a set of info/hygiene notes.
The single most vulnerability-shaped item is the path-traversal pair (PT-1/PT-2), because it
defeats a guard (`safe_join`) whose own comment says it blocks `{{< include /etc/passwd >}}`,
and the fix already exists ten modules away.

## Verified findings, ranked by corrected severity

| id | title | finder → verified | verdict | area |
|----|-------|------|---------|------|
| PT-1 | `safe_join` follows symlinks: arbitrary-file read inlined into HTML | medium → **low** | confirmed (reachable) | `includes.rs` |
| PT-2 | `containment_root` walks up to any ancestor `.git`/`_site.yml`, widening read scope | low → **low** | confirmed (scoped down) | `includes.rs` |
| NET-1 | DNS-rebinding bypasses the ws origin check (no Host allowlist) | medium → **low** | confirmed (reachable) | `serve/security.rs` |
| dos-pages | `serve_site` `app.pages` grows unbounded from bogus ws `?page=` keys | low → **low** | confirmed (A3 only; amplifier refuted) | `serve_site/mod.rs` |
| OUT-1 | Markdown link/image URLs not scheme-sanitized (`javascript:`/`data:`) | low → **info** | refuted as current vuln; **keep as preventive** | `render/emit.rs` |
| CMD-01 | Doc-controlled interpreter path (`_site.yml python:/r:`) spawned as a program | medium → **info** | within by-design exec boundary | `interpreter.rs` |
| DEP-01 | `scc 3.7.3` double-free (RUSTSEC-2026-0205) in graph, path unreachable | low → **info** | confirmed unreachable | `Cargo.lock` |
| DEP-02 | `anyhow 1.0.102` unsound (RUSTSEC-2026-0190), not compiled in | info → **info** | confirmed not in build graph | `Cargo.lock` |
| DEP-03 | Vendored `mermaid 11.4.1` aging; strict-mode default holds | low → **info** | provenance verified | `assets/js` |
| dos-rich | Rich cell outputs not byte-capped (unlike stream) | low → **info** | within A2 exec blast radius | `kernel.rs` |
| NET-2 | LAN token rides cleartext HTTP under `--host` | low → **info** | accepted plaintext-LAN property | `serve/security.rs` |
| oss-1/3/4/5 | Release hygiene: no SECURITY.md; `/home/bogo` paths; notes/ publish; sourcepos in HTML | low/info → **info** | housekeeping | repo |
| NET-3 | Non-constant-time token compare | info → **refuted** | not realistically exploitable | `serve/security.rs` |
| dos-yaml | serde_yaml "billion-laughs" alias bomb | medium → **refuted (empirically)** | libyaml rejects it in ~30ms | `frontmatter.rs` |
| dos-ws-size / oss-2 | No ws message-size cap; no CONTRIBUTING.md | info → **refuted** | hardening nit / completeness | server / repo |

## The four real low-severity items (worth fixing before/around going public)

### PT-1 — `safe_join` confines lexically only, so an in-tree symlink escapes it
`safe_join` ([includes.rs:340](../crates/core/src/includes.rs#L340)) rejects absolute paths and
`..` **textually** (`normalize()` does no filesystem access, no symlink resolution), then hands
the result straight to a symlink-following read: `{{< include >}}` ([includes.rs:130](../crates/core/src/includes.rs#L130)),
the `.bib` bibliography ([render/mod.rs:951](../crates/core/src/render/mod.rs#L951)), and
front-matter `css:` / `include-in-header:` / `include-before-body:` / `include-after-body:`
([render/doc_includes.rs:124-127](../crates/core/src/render/doc_includes.rs#L124)). A malicious
project ships `theme.css` as a symlink to a fixed absolute path (e.g. `/etc/passwd`), references it
with `css: theme.css`; `safe_join` sees a lexical in-root path (no `..`, passes `starts_with`), and
the read follows the link and **inlines the bytes verbatim into the page head**. It fires during
plain render, so it survives `--no-exec` (the "preview an untrusted doc as source" mode), and on
`build` the secret lands in the shipped static HTML (A4).

Verifier adjustment (medium → low): in the default exec-on path, previewing an untrusted project
already runs cells (RCE dominates); the unique window is untrusted-project + `--no-exec` + a symlink
+ preview-or-publish. A symlink cannot expand `~`, so reliable exfil is limited to fixed absolute
paths, not `~/.ssh/id_rsa` (needs the victim's username). Real, defeats a named guard, low.

**Fix (cheap, and asymmetric with existing code):** after `safe_join` resolves, `canonicalize()`
the target and the root and re-check `starts_with` before any read, exactly as
`serve_asset_from` ([serve/mod.rs:431](../crates/server/src/serve/mod.rs#L431)) already does for
HTTP assets. The HTTP path is symlink-safe; the render-time inlining path is not.

### PT-2 — `containment_root` walks up to a distant ancestor marker
`containment_root` ([includes.rs:374](../crates/core/src/includes.rs#L374)) sets `safe_join`'s
boundary to the nearest ancestor containing `.git` or `_site.yml`, walking parents unbounded. The
author does not choose this root; it is inferred from the filesystem above the untrusted doc. If a
high ancestor carries a marker, the root widens and pure `../` traversal to files under that root
is permitted (no symlink needed).

Verifier scoping (kept low): the headline "`~/.git` balloons to `$HOME`" is mostly a misconception
(the standard bare-repo dotfiles pattern creates no `~/.git`; absent a marker the root falls back to
the doc's own dir and `../../escape` is refused, pinned at [includes.rs:555](../crates/core/src/includes.rs#L555)).
The realistic case is an untrusted doc dropped **inside an existing checkout** reading a sibling
repo-local file. Modest escalation, low.

**Fix:** bound the containment root to the CLI-invoked project / the doc's own directory, never a
distant inferred marker. Preserve legitimate `../_includes/` under an explicitly-invoked root (the
design comment at [includes.rs:364](../crates/core/src/includes.rs#L364) shows the walk is
intentional for that).

### NET-1 — DNS-rebinding bypasses the ws origin check (no Host allowlist)
`origin_allowed` ([serve/security.rs:30](../crates/server/src/serve/security.rs#L30)) decides
same-origin by `Some(authority) == host`, comparing `Origin` to the equally-attacker-controlled
`Host`; the loopback name-check ([security.rs:37](../crates/server/src/serve/security.rs#L37)) is
applied to `authority`, never to `Host`, and there is **no Host allowlist anywhere**. A page at
`evil.com` that rebinds its DNS to `127.0.0.1` becomes "same-origin" with the loopback preview:
`Origin` and `Host` are both `evil.com:PORT`, so the ws check passes and `client_conn` pushes a
full document snapshot; the ws control channel (kernel restart) is reachable. Holds in default and
`--host` modes (a rebound connection is genuinely loopback, so the LAN token guard allows it).

Verifier framing correction (medium → low): the **draft read** happens via a plain HTTP `GET`,
which never passes through `origin_allowed` at all (HTTP is ungated) — that is a generic
no-Host-allowlist loopback-server property, not an origin-check failure. The disclosed material is
the author's *own* draft and project tree on the author's *own* machine (no third-party secret, no
write, no RCE beyond by-design cells; the kernel-restart nuisance is documented as accepted at
[serve/mod.rs:817](../crates/server/src/serve/mod.rs#L817)), and the precondition chain is heavy.
Low.

**Fix:** a positive Host-header allowlist on every request (ws upgrade + HTTP): accept only
`{127.0.0.1, localhost, ::1}[:port]` in loopback mode and the bound LAN IP under `--host`. This is
the standard DNS-rebinding defense and closes the ungated HTTP read the origin check never covered.

### dos-pages — unbounded `app.pages` growth from bogus `?page=` keys
On each ws connect, an unrecognized `?page=` value becomes a fresh `PageState` (a 256-slot broadcast
ring) that is never evicted ([serve_site/mod.rs:726](../crates/server/src/serve_site/mod.rs#L726));
only a full `_site.yml` rebuild clears the map. A token-holding LAN peer opening many connections
with distinct keys grows the map without bound. Already logged at
[notes/AUDITS.md](AUDITS.md). Verifier refuted the "build-queue amplifier" half (`build_page`
early-returns for an unknown key, so the queued build is a no-op) and narrowed the reachable
adversary to **A3 only** (the origin gate blocks A1). Low.

**Fix:** only allocate/queue for a `rel` that `site.page()` resolves, or cap+evict idle entries on
disconnect.

## Info / hardening (no adversary reaches these today, but cheap wins for a public release)

- **OUT-1 (preventive, most relevant to going public):** Taliesin emits its own `<a href>`/`<img src>`
  through `escape_attr` only, dropping comrak's safe-mode URL sanitizer, so `[x](javascript:...)` and
  `data:text/html` URLs survive ([render/emit.rs:109](../crates/core/src/render/emit.rs#L109),
  [:122](../crates/core/src/render/emit.rs#L122)). Not exploitable under the single-author model
  (author-authored, subsumed by raw-HTML passthrough + cell exec), but a **safe-default regression**
  that becomes live XSS the moment any not-fully-authored markdown is rendered (a third-party README,
  an unaudited `{{< include >}}`, any future comment/multi-tenant feature). Add an allowlist scheme
  sanitizer + a corpus pin **before** any hosted/multi-author feature ships.
- **DEP-02 (trivial):** `cargo update -p anyhow` bumps the stale lock entry 1.0.102 → 1.0.103
  (already in the local cache) and clears the `cargo audit` warning. `anyhow` is not even in the
  compiled graph (only wasm tooling depends on it).
- **DEP-01:** add `RUSTSEC-2026-0205` (scc double-free) to `deny.toml`'s ignore block with a
  one-line rationale (rides in via `zeromq`, panicking-comparator path unreachable because keys are
  non-panicking `Vec<u8>` identities on loopback-only self-spawned peers). No fixed `scc` exists yet.
  This makes `cargo audit` clean again.
- **DEP-03:** bump vendored `mermaid.min.js` off the ~13-month-old 11.4.1 pin (update
  `THIRD_PARTY.md` to match) and explicitly set `securityLevel: 'strict'` in `mermaid.js` so the
  safe default is not silently dependent on the library. Provenance of all four bundles (d3 7.9.0,
  plot 0.6.16, mermaid 11.4.1, katex) verified against `THIRD_PARTY.md` (test-enforced).
- **CMD-01:** log the fully-resolved interpreter path at warm-pool boot (the eager
  `warm_pool_for_preview` spawn has no startup line before the first page executes; the kernel-start
  path already logs it). Keeps a `_site.yml`-injected interpreter visible. Not a boundary violation
  (naming your interpreter is not an escalation over running code against it, per the universal
  dev-tool trust model).
- **dos-rich / dos-ws-size:** cap cumulative rich-output bytes (mirror the stream cap at
  [kernel.rs:816](../crates/server/src/kernel.rs#L816)) and set a small `max_message_size` on both
  ws upgrades. Both are robustness nits inside the A2 exec blast radius / trusted-peer channel.
- **oss-1/3/4:** add a root `SECURITY.md` (private disclosure channel + supported versions); scrub
  the 4 tracked `/home/bogo/...` absolute paths from `docs/superpowers/*` (username already public via
  git author metadata, so low); and consciously decide whether to prune `notes/` + `docs/superpowers/`
  open-bug/roadmap files before flipping public (no secret is exposed, the `--host` token design doc
  discloses only a per-session UUID mechanism, but it is a curated bug roadmap).
- **oss-5:** built HTML carries doc-relative `data-source-file`/`data-sourcepos` (by-design for
  click-to-source, not absolute paths). Optional build flag to strip them from static output; no
  action required.

## False leads (recorded honestly, per this project's "trust the symptom, re-derive the cause" rule)

- **dos-yaml (billion-laughs): REFUTED empirically.** The finder claimed serde_yaml has no
  alias-expansion cap and a nested-alias bomb OOMs `check`/`build`. The verifier **built the repro the
  finding asked for** against the workspace-locked serde_yaml 0.9.34 / unsafe-libyaml 0.2.11: an
  8-level×9 and a 14-level×10 bomb both return `Err("repetition limit exceeded")` in 13-32ms at ~31MB
  RSS. The guard lives in libyaml, so grepping Taliesin's own source for it (correctly) finds nothing;
  the finder mistook "no guard in app code" for "no guard at all." Only residual: serde_yaml is
  unmaintained (RUSTSEC-2024-0320, already in `deny.toml`), a hygiene item, not a DoS.
- **NET-3 (constant-time token):** the `==` compare is non-constant-time, but recovering a
  session-scoped 122-bit UUID via LAN timing (signal nanoseconds, jitter hundreds of microseconds,
  token regenerated each restart) is infeasible. Defense-in-depth note only.
- **dos-ws-size / oss-2:** transient bounded allocation from a trusted peer; missing contributor doc.
  Neither is a vuln.

## Already fixed / not a release blocker (triage of prior-audit items)

The verifier confirmed against current source that several prior-audit issues are **already
remediated**: the `interp_id` pipeline-hang (`probe_version` now async with `kill_on_drop` under a
10s timeout, [exec.rs:1024](../crates/server/src/exec.rs#L1024)), the `interp_id` empty-version
memoize (only `Some` is cached), the `percent_decode` slice-panic (now byte-based,
[serve/mod.rs:474](../crates/server/src/serve/mod.rs#L474)), the freeze cache is bounded
(MAX_ENTRIES=1024 + LRU), and include expansion is cycle-guarded. The remaining warm-pool / budget
items (`MAX_WARM_PAGES`, refill-goes-dark, container RAM probe) are **not A1-A4 reachable** (their
triggers are fork races and cgroup mismatches, not adversary input), so they are robustness backlog,
not security release blockers.

## Recommendation

The security gate for open-sourcing is essentially clear. When remediation happens, a sensible order:

1. **PT-1 + PT-2** together (one `includes.rs` change: canonicalize + bound the root). Most
   vulnerability-shaped, cheap, defeats a named guard, and touches the published-artifact leak vector.
2. **NET-1** Host-allowlist middleware (standard DNS-rebinding defense; small, well-understood).
3. **DEP-02 + DEP-01** (bump `anyhow`, ignore `scc` with rationale) so `cargo audit` is clean.
4. **OUT-1** URL scheme sanitizer + corpus pin — do this **before** any multi-author/hosted feature,
   not after.
5. The `oss-*` release-checklist items (SECURITY.md, path scrub, notes/ prune) whenever the repo
   actually flips public.

Provenance: workflow `taliesin-release-security-audit` (27 agents: 7 finders + 20 adversarial
verifiers, ~1.48M tokens, 2026-07-17). `cargo deny check` passes (advisories/bans/licenses/sources
all OK); `cargo audit` warns on 5 allowed advisories, 2 of which (`anyhow`, `scc`) are addressed
above.
