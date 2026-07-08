# `taliesin publish` — passcode-gated static deploy — design

Date: 2026-07-08
Status: approved, ready to implement
Branch: `taliesin-publish`

## Motivation

The author keeps in-progress explainer books in separate **private** repositories
(`fl-weather`, `invertible-speech-disentanglement`) and wants each one reachable at a
stable, **passcode-protected** URL so supervisors can watch the current state of the
work without the drafts being public and without needing an account.

`taliesin build <dir>` already produces a self-contained static tree; "publishing" is
only the last mile: getting that tree behind a gated, auto-updating URL. This is the
backlog item *"`taliesin publish`: build, push HTML to a deploy branch, host
auto-deploys"* (researched 2026-07-07 for exactly this private-draft workflow), refined
here. The manual recipe (`docs/guide/reference/cli.tmd` "Publishing & sharing") already
covers arbitrary static hosts; this automates the one repeated workflow the author has.

## Decisions (settled during brainstorming)

- **What:** several separate books, each its own repo, each its own gated URL.
- **Gate:** a single **shared passcode** per site (not email allowlist), typed once into
  a native browser Basic-Auth prompt, no account required.
- **Host / mechanism:** **Cloudflare Pages via Wrangler direct upload**
  (`wrangler pages deploy`). Chosen over pushing built HTML to a git deploy branch so
  that nothing generated ever enters the private research repos' history, and the
  passcode Function deploys automatically. Cloudflare Pages was pre-selected (free,
  private-repo-capable, deploy-without-its-own-build, gateable).
- **Altitude:** a thin `taliesin publish` subcommand that orchestrates
  build → inject a bundled passcode gate → shell out to `wrangler`. The binary does
  **not** reimplement Cloudflare's API or manage project/secret lifecycle.

### Verified facts (Cloudflare docs, 2026-07-08)

- `wrangler pages deploy <dir>` **does** compile a `functions/` folder when it exists
  *where the command is run*. (Only dashboard drag-and-drop cannot; Wrangler can — this
  is stated explicitly in the Direct Upload docs.) So a `functions/_middleware.js`
  passcode gate is compatible with direct upload.
- The production URL `<project>.pages.dev` is stable and always serves the latest
  production deployment, so a bookmarked URL stays fresh across re-publishes.
- Non-interactive auth: `CLOUDFLARE_API_TOKEN` (Pages:Edit) + `CLOUDFLARE_ACCOUNT_ID`
  in the environment.
- Reference for the shared-password middleware pattern:
  `github.com/garrison/cloudflare-pages-shared-password` (functions/_middleware +
  Workers basic-auth example).

## Scope

- **In:** a `publish` subcommand that builds a Taliesin **site/book** project, writes a
  bundled `functions/_middleware.js` into the output tree, and runs
  `wrangler pages deploy`. A closed `publish:` config block in `_site.yml`. Docs for the
  one-time setup. Corpus/unit tests that need no network.
- **Out (deliberate):**
  - Base-URL / subpath handling — each book is its own Pages project served at a root,
    so the known subpath gap (absolute links + built-in 404 assume a root deploy) never
    arises.
  - Auto-building `mounts:` into the deployed tree (the books here are standalone
    projects, not the mount-composed marketing site).
  - Email-allowlist / Cloudflare Access mode (shared passcode chosen).
  - Cloudflare **project creation** and **secret setting** from the binary (one-time,
    manual/documented — keeps CF account lifecycle out of the tool).
  - Any write-back to source (single-editing-surface invariant: flow is one-way,
    source → build → deploy).
  - Single-`.tmd`-file publish (the use case is books/sites; single-doc stays a manual
    `build` + host).

## Design

### 1. Command surface

```
taliesin publish <project>            # build the site + deploy to its Cloudflare Pages target
taliesin publish <project> --strict   # refuse to deploy if the build reports warnings
taliesin publish <project> --dry-run  # build + inject the gate, PRINT the wrangler command, do not deploy
```

`<project>` must be a directory (a site or a book). Publishing a single `.tmd` file is
an error with a message pointing at `build` + the manual host recipe.

Per-run flow (`cmd_publish`):

1. Resolve config (§2); compute the Cloudflare project name.
2. Build the site into its normal output dir (`_site` / `_book`, or `--out`), reusing
   the existing `build_site` path unchanged. `--strict` threads through to the existing
   strict-exit logic.
3. Write the bundled `functions/_middleware.js` (§4) into `<out>/functions/_middleware.js`,
   **after** the build (so the build's stale-sweep never sees or deletes it).
4. Preflight: `wrangler` on `PATH`, and `CLOUDFLARE_API_TOKEN` set. On failure, exit
   non-zero with an actionable message (what's missing + the one-time-setup commands).
   `--dry-run` skips this and just prints the command.
5. Run `wrangler pages deploy . --project-name <name> --branch <prod> --commit-dirty=true`
   with `cwd = <out>`, where `<prod>` is a fixed production-branch label (`production`).
   Rationale: `wrangler pages deploy` otherwise infers the deployment branch from the
   ambient git repo — and a branch that doesn't match the project's production branch
   yields a *preview* deploy at an unstable per-deploy hostname, not the stable
   `<name>.pages.dev`. Passing `--branch <prod>` explicitly forces a production deploy
   regardless of the repo's current branch; `--commit-dirty=true` suppresses the
   uncommitted-changes prompt so the run is non-interactive. `cwd = <out>` is what makes
   Wrangler pick up `functions/`. Stream output; propagate the exit code.
6. On success, print the production URL (`https://<name>.pages.dev`).

### 2. Configuration (zero by default)

Per the "perfect the default" rule, the Cloudflare project name **defaults to a slug of
the project directory name** (lowercased, non-alphanumerics → `-`, e.g. `FL-Weather`
→ `fl-weather`). No config is needed in the common case.

Override via a closed block in `_site.yml`:

```yaml
publish:
  provider: cloudflare      # required if the block is present; only accepted value for now
  project: my-custom-name   # optional; defaults to the dir slug
```

- Add `publish` to `NATIVE_KEYS` (`crates/core/src/site/config`) and a `PUBLISH_KEYS`
  closed child set (`provider`, `project`) validated with the existing `closest()`
  did-you-mean machinery, so typos are click-to-source warnings.
- Regenerate the `qmd-site.schema.json` golden file (drift-locked test).
- The **passcode is never in config or git** — it lives only as a Cloudflare Pages
  secret.

A `--project-name <name>` CLI flag overrides both config and the default slug (useful
for a one-off).

### 3. One-time setup (documented; not in the binary)

Three commands per repo, once:

```sh
export CLOUDFLARE_API_TOKEN=...     # token with Pages:Edit
export CLOUDFLARE_ACCOUNT_ID=...
wrangler pages project create fl-weather --production-branch production
wrangler pages secret put PASSWORD --project-name fl-weather   # type the passcode; stored as a CF secret
```

`publish` assumes these are done and fails clearly if not. Deliberately excluded from
the binary to avoid growing Cloudflare-account lifecycle logic. (A future optional
`taliesin publish --init` convenience wrapper is possible but out of scope for v1.)

### 4. The passcode gate

A bundled asset `assets/functions/_middleware.js`, embedded via `include_str!` and
written into the build tree at publish time. Behavior:

- Read `env.PASSWORD` (the Cloudflare Pages secret). If it is unset, **fail closed**
  (return 503/500, never serve ungated) rather than allowing all traffic.
- Parse the `Authorization: Basic` header; base64-decode; **constant-time compare** the
  password component against `env.PASSWORD`. The username component is not checked (a
  shared passcode has no per-user identity).
- On missing/mismatch: `401` with `WWW-Authenticate: Basic realm="draft"`.
- On match: `return context.next()` (serve the static asset).

`pages.dev` is HTTPS, so the credential is encrypted in transit. Security posture is
stated honestly in docs: a shared passcode keeps casual strangers out, is forwardable,
and is not real per-person access control — acceptable for a supervisor draft.

### 5. Where it lives in the code

- `crates/server/src/main.rs`: new `Some("publish") => publish::cmd_publish(&args)` arm
  + help text.
- `crates/server/src/publish.rs` (new): arg parsing, config resolution, slug, build
  reuse, middleware injection, wrangler shell-out, messaging.
- `crates/core/assets/functions/_middleware.js` (new bundled asset).
- `crates/core/src/site/config`: `publish` in `NATIVE_KEYS` + `PUBLISH_KEYS` +
  `PublishConfig { provider, project }`.
- Schema golden file regenerated.

### 6. Tests (corpus-plus-roadmap discipline, all network-free)

- `publish --dry-run` on a corpus site (e.g. `corpus/demo-book`): asserts the tree
  builds, `functions/_middleware.js` is present and byte-equal to the bundled asset, and
  the printed command is exactly
  `wrangler pages deploy . --project-name <expected-slug> --branch production --commit-dirty=true`.
- Config: a `publish:` block parses into `PublishConfig`; an unknown child key
  (`provder:`) produces the expected did-you-mean warning; the schema golden file
  matches.
- Slug: directory-name → project-name slug cases (spaces, capitals, punctuation).
- Middleware: a small self-contained JS test of the auth check (unset secret → fail
  closed; missing header → 401; wrong pass → 401; correct → next), runnable without
  Cloudflare.
- Preflight: with `wrangler` absent / token unset, `publish` (non-dry-run) exits
  non-zero with the actionable message (no deploy attempted).

Corpus pin: `corpus/publish/` is unnecessary — publish adds no new *rendered* output;
the dry-run + config tests are the regression net. (If a pin is wanted, a `publish:`
block on an existing corpus site's `_site.yml` exercises the config path.)

## Caveats

- **Mermaid** diagrams load from a CDN at view time (the one non-offline dependency).
  Harmless behind the gate; books without Mermaid are fully self-contained. Noted in docs.
- **FL-weather must be a Taliesin (`.tmd`) project** for `build` to succeed. If it is
  still a Quarto book, migrating it is a separate prerequisite (existing backlog item),
  not part of this feature.
- Wrangler + a Cloudflare account/token are prerequisites the tool cannot provide; the
  error path must make that obvious.

## Invariants held

- One-way flow (source → build → deploy); no preview write-back, no source mutation.
- HTML-only output; publish deploys the existing build artifact, adds no new format.
- Do-NOT-touch machinery untouched; publish reuses `build_site` and rides the config
  closed-key + schema seams. The `functions/` dir is written post-build so the
  stale-sweep contract is unaffected.
- Minimal config: zero-config default (slug), one optional closed block.
