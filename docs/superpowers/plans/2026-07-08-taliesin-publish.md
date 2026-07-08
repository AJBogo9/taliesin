# `taliesin publish` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `taliesin publish <project>` subcommand that builds a Taliesin site/book, injects a shared-passcode gate, and deploys it to Cloudflare Pages via Wrangler direct upload.

**Architecture:** A thin subcommand in the server crate orchestrates three existing-or-new pieces: (1) the existing site build (`run_site_build`, extracted from `build_site` so `publish` can learn success + reuse the output dir), (2) a bundled `functions/_middleware.js` HTTP Basic-Auth gate written into the built tree, (3) a shell-out to `wrangler pages deploy`. A closed `publish:` block in `_site.yml` names the Cloudflare project (default: a slug of the directory name). The passcode lives only as a Cloudflare secret, never in the repo. Flow is one-way (source, build, deploy); nothing writes back to source.

**Tech Stack:** Rust (edition 2024), `std::process::Command` for the Wrangler shell-out (no new crate deps), `serde_yaml` (already used) for config, a small ESM Cloudflare Pages Function in JavaScript, `node:test` for the Function's unit test.

## Global Constraints

- Rust edition 2024, workspace resolver 3. No new runtime crate dependencies (publish uses only `std`).
- A `PostToolUse` hook runs `rustfmt` on edited `.rs` files; the tree must stay `cargo fmt`-clean (CI enforces).
- Never use em dashes or en dashes in any code comment, user-facing string, or doc prose. Use commas, colons, parentheses, or hyphens.
- The block-model invariants and the byte-identical parallel site build are load-bearing. The `build_site_async` refactor here changes only its return type and the runtime-creation site; it must NOT change page-build ordering, per-page freeze isolation, or output bytes. `crates/server/tests/parallel_build_determinism.rs` must stay green.
- Minimal config: `publish:` is optional; zero config (dir-name slug) is the default path.
- One-way flow only. `publish` never writes to the source tree (it writes to the build output dir and deploys from there).
- The `.tmd` extension and `_site.yml` config file names are fixed.

---

### Task 1: `publish:` config parsing + validation (core crate)

**Files:**
- Modify: `crates/core/src/site/config/mod.rs` (add `PublishConfig`, `SiteConfig.publish`, `NATIVE_KEYS` entry, `PUBLISH_KEYS`, `validate_publish`, `publish_from`, wire into `parse_native`)
- Test: same file's `#[cfg(test)] mod config_tests`

**Interfaces:**
- Produces (re-exported to `crate::site::*` via the existing `pub use config::*;` at `crates/core/src/site/mod.rs:189`):
  - `pub struct PublishConfig { pub provider: Option<String>, pub project: Option<String> }` (derives `Debug, Clone, Default`)
  - `SiteConfig.publish: Option<PublishConfig>`
  - `pub(crate) const PUBLISH_KEYS: &[&str] = &["provider", "project"]`

- [ ] **Step 1: Write the failing tests**

Add to `mod config_tests` in `crates/core/src/site/config/mod.rs`:

```rust
    #[test]
    fn publish_block_parses_provider_and_project() {
        let dir = tmp("publish-ok");
        std::fs::write(
            dir.join("_site.yml"),
            "title: Book\npublish:\n  provider: cloudflare\n  project: my-book\n",
        )
        .unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        let publish = cfg.publish.expect("publish block parsed");
        assert_eq!(publish.provider.as_deref(), Some("cloudflare"));
        assert_eq!(publish.project.as_deref(), Some("my-book"));
        assert!(
            !warnings.iter().any(|w| w.contains("unknown")),
            "a valid publish block must not warn: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_publish_key_warns_with_did_you_mean() {
        // A typo in a publish key silently drops the setting, so it must warn.
        let w = cfg_warnings("publish:\n  provder: cloudflare\n");
        assert!(
            w.iter()
                .any(|w| w.contains("publish key `provder`") && w.contains("`provider`")),
            "publish key typo: {w:?}"
        );
    }

    #[test]
    fn absent_publish_block_is_none() {
        let dir = tmp("publish-absent");
        std::fs::write(dir.join("_site.yml"), "title: Book\n").unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        assert!(cfg.publish.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p taliesin-core --lib publish_block_parses_provider_and_project unknown_publish_key_warns_with_did_you_mean absent_publish_block_is_none`
Expected: FAIL to compile (`PublishConfig`, `cfg.publish` do not exist yet).

- [ ] **Step 3: Add the `PublishConfig` struct + `SiteConfig` field**

In `crates/core/src/site/config/mod.rs`, after the `Mount` struct (around line 63), add:

```rust
/// `publish:` — where `taliesin publish` deploys this project. Optional; when absent,
/// publish falls back to a slug of the project directory name. The passcode is never
/// stored here (it lives only as a Cloudflare Pages secret).
#[derive(Debug, Clone, Default)]
pub struct PublishConfig {
    /// Deploy provider. Only `cloudflare` is recognized today.
    pub provider: Option<String>,
    /// Cloudflare Pages project name (overrides the dir-name slug default).
    pub project: Option<String>,
}
```

In `struct SiteConfig`, after the `mounts` field (line 54), add:

```rust
    /// `publish:` deploy target for `taliesin publish` (absent unless configured).
    pub publish: Option<PublishConfig>,
```

- [ ] **Step 4: Register the key + validation + parser**

In `NATIVE_KEYS` (line 96), add `"publish",` after `"mounts",`.

After the `MOUNT_ITEM_KEYS` const (line 123), add:

```rust
/// The keys of the `publish:` block (`{ provider, project }`).
pub(crate) const PUBLISH_KEYS: &[&str] = &["provider", "project"];
```

In `validate_keys`, in the `match key { ... }` (after the `"mounts" =>` arm, line 244), add:

```rust
            "publish" => validate_publish(v, warnings),
```

After the `validate_mounts` fn (line 317), add:

```rust
/// Validate the `publish:` mapping's keys against [`PUBLISH_KEYS`]. A typo silently
/// drops a setting (publish would fall back to a default), so it warns.
fn validate_publish(v: &serde_yaml::Value, warnings: &mut Vec<String>) {
    let serde_yaml::Value::Mapping(m) = v else {
        return;
    };
    for k in m.keys().filter_map(|k| k.as_str()) {
        if !PUBLISH_KEYS.contains(&k) {
            warnings.push(format!(
                "_site.yml: unknown publish key `{k}`{}",
                did_you_mean(k, PUBLISH_KEYS)
            ));
        }
    }
}

/// Parse the `publish:` mapping into [`PublishConfig`] (a non-mapping value yields None).
fn publish_from(v: Option<&serde_yaml::Value>) -> Option<PublishConfig> {
    let pv = v?;
    if !pv.is_mapping() {
        return None;
    }
    let s = |k: &str| pv.get(k).and_then(|x| x.as_str()).map(str::to_string);
    Some(PublishConfig {
        provider: s("provider"),
        project: s("project"),
    })
}
```

In `parse_native`, in the `SiteConfig { ... }` literal (after `mounts: mounts_from(...)`, line 185), add:

```rust
        publish: publish_from(value.get("publish")),
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p taliesin-core --lib publish_block_parses_provider_and_project unknown_publish_key_warns_with_did_you_mean absent_publish_block_is_none`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/site/config/mod.rs
git commit -m "feat(site): parse + validate a publish: config block"
```

---

### Task 2: Regenerate the `_site.yml` JSON Schema for `publish` (core crate)

**Files:**
- Modify: `crates/core/src/schema.rs` (`site_config_schema` override for `publish`)
- Modify (blessed, generated): `crates/core/assets/schema/tali-site.schema.json`

**Interfaces:**
- Consumes: `PUBLISH_KEYS` from Task 1 (via `crate::site::PUBLISH_KEYS`).

- [ ] **Step 1: Verify the schema drift test currently fails after Task 1**

Task 1 added `publish` to `NATIVE_KEYS`, so the generated schema now includes a `publish` property but the committed golden file does not.

Run: `cargo test -p taliesin-core --lib site_schema_matches_committed`
Expected: FAIL with "schema drift in assets/schema/tali-site.schema.json".

- [ ] **Step 2: Add the closed `publish` sub-schema to the generator**

In `crates/core/src/schema.rs`, at the top of `mod generate`, extend the import (line 24):

```rust
    use crate::site::{NATIVE_KEYS, PUBLISH_KEYS};
```

In `site_config_schema()` (line 120), before the final `json!({ ... })`, add:

```rust
        // publish: a closed { provider, project } block. `provider` is an enum (only
        // cloudflare today); `project` is the Cloudflare Pages project name.
        let publish = closed_object(
            PUBLISH_KEYS,
            &[
                (
                    "provider",
                    json!({ "type": "string", "enum": ["cloudflare"] }),
                ),
                ("project", json!({ "type": "string" })),
            ],
        );
```

Then change the final `properties(...)` call (line 158) to include the override:

```rust
            "properties": properties(
                NATIVE_KEYS,
                &[("toc", boolean()), ("chapters", chapters), ("publish", publish)],
            ),
```

- [ ] **Step 3: Bless the golden file**

Run: `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib site_schema_matches_committed`
Expected: PASS, prints `blessed assets/schema/tali-site.schema.json`.

- [ ] **Step 4: Verify the drift test passes without bless**

Run: `cargo test -p taliesin-core --lib schema`
Expected: PASS (all schema tests, including `schemas_are_structurally_sane` which checks every `NATIVE_KEYS` key is present).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/schema.rs crates/core/assets/schema/tali-site.schema.json
git commit -m "feat(schema): close the publish: block in the _site.yml schema"
```

---

### Task 3: The passcode gate Function + its unit test (server crate)

**Files:**
- Create: `crates/server/src/assets/_middleware.js` (the Cloudflare Pages Function, bundled via `include_str!`)
- Create: `crates/server/src/assets/package.json` (marks the dir ESM so `node --test` can import the `.js` Function; not deployed)
- Create: `crates/server/src/assets/_middleware.test.mjs` (`node:test` unit test)

**Interfaces:**
- Produces: `crates/server/src/assets/_middleware.js` exporting `async function onRequest(context)` (Cloudflare Pages middleware signature). Consumed by Task 5 via `include_str!("assets/_middleware.js")`.

- [ ] **Step 1: Write the failing test**

Create `crates/server/src/assets/_middleware.test.mjs`:

```js
import { test } from "node:test";
import assert from "node:assert/strict";
import { onRequest } from "./_middleware.js";

function ctx({ auth, password } = {}) {
  const headers = new Headers();
  if (auth) headers.set("Authorization", auth);
  return {
    request: new Request("https://example.pages.dev/", { headers }),
    env: password === undefined ? {} : { PASSWORD: password },
    next: async () => new Response("SECRET", { status: 200 }),
  };
}
const basic = (user, pass) =>
  "Basic " + Buffer.from(`${user}:${pass}`).toString("base64");

test("missing PASSWORD secret fails closed (503)", async () => {
  const res = await onRequest(ctx({ auth: basic("x", "hunter2") }));
  assert.equal(res.status, 503);
});

test("no Authorization header returns 401 with WWW-Authenticate", async () => {
  const res = await onRequest(ctx({ password: "hunter2" }));
  assert.equal(res.status, 401);
  assert.match(res.headers.get("WWW-Authenticate"), /Basic realm="draft"/);
});

test("wrong passcode returns 401", async () => {
  const res = await onRequest(ctx({ password: "hunter2", auth: basic("x", "nope") }));
  assert.equal(res.status, 401);
});

test("correct passcode calls next() and serves content", async () => {
  const res = await onRequest(
    ctx({ password: "hunter2", auth: basic("anyuser", "hunter2") }),
  );
  assert.equal(res.status, 200);
  assert.equal(await res.text(), "SECRET");
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `node --test crates/server/src/assets/_middleware.test.mjs`
Expected: FAIL (cannot resolve `./_middleware.js` / module not found).

- [ ] **Step 3: Create the ESM marker**

Create `crates/server/src/assets/package.json`:

```json
{
  "private": true,
  "type": "module"
}
```

- [ ] **Step 4: Write the Function**

Create `crates/server/src/assets/_middleware.js`:

```js
// Cloudflare Pages Functions middleware: gate the whole site behind a shared passcode
// (HTTP Basic Auth). The passcode is the `PASSWORD` environment secret set on the Pages
// project (`wrangler pages secret put PASSWORD`), never stored in the repo. This file is
// injected into the build output by `taliesin publish`.
export async function onRequest(context) {
  const { request, env, next } = context;
  const expected = env.PASSWORD;
  // Fail closed: if the secret is unset, never serve ungated content.
  if (!expected) {
    return new Response("Site not configured: missing PASSWORD secret.", {
      status: 503,
    });
  }
  const header = request.headers.get("Authorization") || "";
  const [scheme, encoded] = header.split(" ");
  if (scheme === "Basic" && encoded) {
    let decoded = "";
    try {
      decoded = atob(encoded);
    } catch {
      decoded = "";
    }
    // "user:pass"; compare only the password (a shared passcode has no per-user
    // identity). Constant-time compare to avoid a timing oracle.
    const pass = decoded.slice(decoded.indexOf(":") + 1);
    if (timingSafeEqual(pass, expected)) {
      return next();
    }
  }
  return new Response("Authentication required.", {
    status: 401,
    headers: { "WWW-Authenticate": 'Basic realm="draft", charset="UTF-8"' },
  });
}

// Length-independent constant-time string compare (both encoded to bytes first).
function timingSafeEqual(a, b) {
  const enc = new TextEncoder();
  const ab = enc.encode(a);
  const bb = enc.encode(b);
  const len = Math.max(ab.length, bb.length);
  let diff = ab.length ^ bb.length;
  for (let i = 0; i < len; i++) {
    diff |= (ab[i] ?? 0) ^ (bb[i] ?? 0);
  }
  return diff === 0;
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `node --test crates/server/src/assets/_middleware.test.mjs`
Expected: PASS (4 tests, `# pass 4`).

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/assets/_middleware.js crates/server/src/assets/package.json crates/server/src/assets/_middleware.test.mjs
git commit -m "feat(publish): bundled shared-passcode Pages Function + node test"
```

---

### Task 4: Extract `run_site_build` returning a success bool (server crate)

**Files:**
- Modify: `crates/server/src/build.rs` (`build_site_async` return type; new `run_site_build`; `build_site` becomes a thin mapper; extract `warn_strict`)

**Interfaces:**
- Produces: `pub(crate) fn run_site_build(root: &Path, out_override: Option<&str>, strict: bool, jobs: Option<usize>) -> bool` (true = the site built; false = a hard failure or a `--strict` problem). Consumed by Task 5.
- Consumes: nothing new.

- [ ] **Step 1: Add the `warn_strict` helper and route `strict_exit` through it**

In `crates/server/src/build.rs`, replace the body of `strict_exit` (lines 225-234) with:

```rust
fn strict_exit(code: ExitCode, strict_fail: bool, problems: usize) -> ExitCode {
    if strict_fail {
        warn_strict(problems);
        return ExitCode::FAILURE;
    }
    code
}

/// Log the `--strict` failure summary (shared by the single-doc and site build paths).
fn warn_strict(problems: usize) {
    log::error(&format!(
        "--strict: {problems} problem{} (cell error or located warning); failing the build",
        if problems == 1 { "" } else { "s" }
    ));
}
```

- [ ] **Step 2: Change `build_site_async` to return `bool`**

Change its signature (line 802) from `-> ExitCode` to `-> bool`.

Replace the three early failure returns:
- Line 820 `return ExitCode::FAILURE;` (no pages) becomes `return false;`
- Line 830 `return ExitCode::FAILURE;` (cannot create out) becomes `return false;`
- Line 844 `return ExitCode::FAILURE;` (in-place refuse) becomes `return false;`

Replace the tail (line 1089) `strict_exit(ExitCode::SUCCESS, strict && problems > 0, problems)` with:

```rust
    let strict_fail = strict && problems > 0;
    if strict_fail {
        warn_strict(problems);
    }
    !strict_fail
```

- [ ] **Step 3: Replace `build_site` with `run_site_build` + a thin mapper**

Replace the whole `build_site` fn (lines 780-800) with:

```rust
/// Run a directory (site/book) build to disk, returning whether it succeeded. Shared by
/// `cmd_build`'s directory branch and `publish` (which needs the success signal, not just
/// an opaque `ExitCode`, plus the freedom to keep working with the output dir afterward).
pub(crate) fn run_site_build(
    root: &Path,
    out_override: Option<&str>,
    strict: bool,
    jobs: Option<usize>,
) -> bool {
    // Executing code cells needs the async kernel, so the whole site build runs on a
    // tokio runtime (mirrors the preview server's setup). A multi-thread runtime so
    // concurrent page builds (each its own kernel) actually overlap on the CPU.
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log::error(&format!("cannot start runtime: {e}"));
            return false;
        }
    };
    rt.block_on(build_site_async(root, out_override, strict, jobs))
}

fn build_site(root: &Path, out_override: Option<&str>, strict: bool, jobs: Option<usize>) -> ExitCode {
    if run_site_build(root, out_override, strict, jobs) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
```

- [ ] **Step 4: Run the build regression tests**

Run: `cargo test -p taliesin-server --test parallel_build_determinism --test stale_sweep --test strict_robustness`
Expected: PASS (the parallel build stays byte-identical; strict still fails on problems; stale sweep unchanged).

- [ ] **Step 5: Verify the crate still compiles + a broad build test run**

Run: `cargo test -p taliesin-server`
Expected: PASS (no `ExitCode`/`bool` mismatch; `build_site` still used by `cmd_build`'s dir branch).

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/build.rs
git commit -m "refactor(build): extract run_site_build returning a success bool"
```

---

### Task 5: The `publish` subcommand (server crate)

**Files:**
- Create: `crates/server/src/publish.rs`
- Create: `crates/server/tests/publish.rs` (integration: dry-run + missing-token)

**Interfaces:**
- Consumes: `crate::build::run_site_build` (Task 4); `taliesin_core::Site` + `Site.config.publish` + `Site::output_dir()` (Task 1); `include_str!("assets/_middleware.js")` (Task 3); `crate::serve::unknown_flag_error` (`crates/server/src/serve/mod.rs:1307`); `crate::log`.
- Produces: `pub(crate) fn cmd_publish(args: &[String]) -> ExitCode`.

- [ ] **Step 1: Write the failing unit tests (arg parsing + slug)**

Create `crates/server/src/publish.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_lowercases_and_dashes_non_alphanumerics() {
        assert_eq!(slug("FL-Weather"), "fl-weather");
        assert_eq!(slug("invertible speech"), "invertible-speech");
        assert_eq!(slug("My_Book!!"), "my-book");
        assert_eq!(slug("a---b"), "a-b");
        assert_eq!(slug("...."), "");
    }

    fn argv(rest: &[&str]) -> Vec<String> {
        let mut v = vec!["taliesin".to_string(), "publish".to_string()];
        v.extend(rest.iter().map(|s| s.to_string()));
        v
    }

    #[test]
    fn parses_path_and_flags() {
        let a = argv(&["book", "--project-name", "my-book", "--dry-run", "--strict"]);
        let p = parse_publish_args(&a).expect("parse");
        assert_eq!(p.path, "book");
        assert_eq!(p.project_name, Some("my-book"));
        assert!(p.dry_run);
        assert!(p.strict);
        assert_eq!(p.out_dir, None);
    }

    #[test]
    fn missing_path_is_an_error() {
        assert!(parse_publish_args(&argv(&["--dry-run"])).is_err());
    }

    #[test]
    fn unknown_flag_is_an_error() {
        let err = parse_publish_args(&argv(&["book", "--projct-name", "x"])).unwrap_err();
        assert!(err.contains("--projct-name"), "{err}");
    }

    #[test]
    fn project_name_flag_requires_a_value() {
        assert!(parse_publish_args(&argv(&["book", "--project-name"])).is_err());
    }
}
```

- [ ] **Step 2: Run the unit tests to verify they fail**

Run: `cargo test -p taliesin-server --lib publish`
Expected: FAIL to compile (`slug`, `parse_publish_args`, `PublishArgs` do not exist).

- [ ] **Step 3: Implement arg parsing + slug**

Add above the test module in `crates/server/src/publish.rs`:

```rust
//! The `publish` subcommand: build a site/book and deploy it to Cloudflare Pages
//! (Wrangler direct upload) behind a shared passcode.
//!
//! **What:** `publish <dir>` builds the project (reusing the site build), writes a
//! bundled `functions/_middleware.js` HTTP Basic-Auth gate into the output tree, then
//! runs `wrangler pages deploy` from that tree. One-way: it never writes to the source.
//!
//! **How to use:** `main()` dispatches `publish` to [`cmd_publish`].
//!
//! **One-time setup (per repo, documented, not automated here):**
//! `wrangler pages project create <name> --production-branch production` and
//! `wrangler pages secret put PASSWORD --project-name <name>`, with `CLOUDFLARE_API_TOKEN`
//! (and `CLOUDFLARE_ACCOUNT_ID`) in the environment.

use crate::log;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The Cloudflare Pages Function that gates the deployed site behind a shared passcode.
/// Written into `<out>/functions/_middleware.js` at publish time.
const MIDDLEWARE_JS: &str = include_str!("assets/_middleware.js");

/// Fixed production-branch label used at project-create time and at every deploy, so a
/// deploy is always a *production* deploy (stable `<name>.pages.dev`) regardless of the
/// source repo's current git branch.
const PRODUCTION_BRANCH: &str = "production";

/// Long flags `publish` accepts (drives the unknown-flag did-you-mean).
const PUBLISH_FLAGS: &[&str] = &["--project-name", "--out", "--strict", "--dry-run"];

/// Parsed `publish` argv (pure; no I/O), so the positional/flag rules are unit-testable.
#[derive(Debug)]
struct PublishArgs<'a> {
    path: &'a str,
    project_name: Option<&'a str>,
    out_dir: Option<&'a str>,
    strict: bool,
    dry_run: bool,
}

/// Parse `publish` argv (`args[2..]`). The first positional is the project dir.
fn parse_publish_args(args: &[String]) -> Result<PublishArgs<'_>, String> {
    let mut positionals: Vec<&str> = Vec::new();
    let mut project_name: Option<&str> = None;
    let mut out_dir: Option<&str> = None;
    let mut strict = false;
    let mut dry_run = false;
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--project-name" => match it.next().map(|s| s.as_str()) {
                Some(v) if !v.starts_with("--") => project_name = Some(v),
                _ => return Err("error: --project-name requires a value (e.g. --project-name my-book)".to_string()),
            },
            "--out" | "--dir" => match it.next().map(|s| s.as_str()) {
                Some(v) if !v.starts_with("--") => out_dir = Some(v),
                _ => return Err(format!("error: {a} requires a directory value (e.g. {a} out)")),
            },
            "--strict" => strict = true,
            "--dry-run" => dry_run = true,
            s if s.starts_with("--") => {
                return Err(format!(
                    "error: {}",
                    crate::serve::unknown_flag_error(s, PUBLISH_FLAGS)
                ));
            }
            s => positionals.push(s),
        }
    }
    let path = positionals.first().copied().ok_or_else(|| {
        "usage: taliesin publish <dir> [--project-name <name>] [--out <dir>] [--strict] [--dry-run]"
            .to_string()
    })?;
    Ok(PublishArgs {
        path,
        project_name,
        out_dir,
        strict,
        dry_run,
    })
}

/// Slugify a directory name into a Cloudflare Pages project name: lowercase, runs of
/// non-alphanumerics collapse to one `-`, trimmed of leading/trailing `-`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}
```

- [ ] **Step 4: Run the unit tests to verify they pass**

Run: `cargo test -p taliesin-server --lib publish`
Expected: PASS (slug + arg-parse tests).

- [ ] **Step 5: Implement `cmd_publish` + the middleware injection**

Add to `crates/server/src/publish.rs`, after `slug` (before the test module):

```rust
/// Write the passcode gate into `<out>/functions/_middleware.js`. Called AFTER the build
/// (the build's stale-sweep would otherwise delete the `functions/` dir, which is neither
/// dot- nor underscore-prefixed); re-injected on every publish.
fn inject_gate(out: &Path) -> std::io::Result<()> {
    let dir = out.join("functions");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("_middleware.js"), MIDDLEWARE_JS)
}

/// `publish <dir>`: build the site, inject the passcode gate, deploy to Cloudflare Pages.
pub(crate) fn cmd_publish(args: &[String]) -> ExitCode {
    let PublishArgs {
        path,
        project_name,
        out_dir,
        strict,
        dry_run,
    } = match parse_publish_args(args) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };

    let root = Path::new(path);
    if !root.is_dir() {
        log::error(&format!(
            "publish builds a site or book (a directory with _site.yml); `{path}` is not a directory. \
             For a single document, use `taliesin build {path}` and host the output yourself."
        ));
        return ExitCode::FAILURE;
    }

    // Fail fast (before the build) when a real deploy is missing its credential.
    if !dry_run && std::env::var_os("CLOUDFLARE_API_TOKEN").is_none() {
        log::error(
            "CLOUDFLARE_API_TOKEN is not set (a non-interactive deploy needs it). \
             Create a token with the Cloudflare Pages:Edit permission, then export \
             CLOUDFLARE_API_TOKEN (and CLOUDFLARE_ACCOUNT_ID). Use --dry-run to build without deploying.",
        );
        return ExitCode::FAILURE;
    }

    // Discover the site once to resolve the project name + the output dir.
    let site = taliesin_core::Site::discover(root);
    if let Some(publish) = &site.config.publish
        && let Some(provider) = &publish.provider
        && provider != "cloudflare"
    {
        log::error(&format!(
            "publish provider `{provider}` is not supported (only `cloudflare`)."
        ));
        return ExitCode::FAILURE;
    }

    let dir_name = root
        .canonicalize()
        .ok()
        .and_then(|c| c.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let project = project_name
        .map(str::to_string)
        .or_else(|| site.config.publish.as_ref().and_then(|p| p.project.clone()))
        .unwrap_or_else(|| slug(&dir_name));
    if project.is_empty() {
        log::error(
            "cannot derive a Cloudflare project name from the directory; \
             pass --project-name <name> or set publish.project in _site.yml.",
        );
        return ExitCode::FAILURE;
    }

    // Resolve the output dir the same way the build does, so we can inject + deploy from it.
    let out: PathBuf = out_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(site.output_dir()));

    // Build (reuses the full site build, including its own discover + strict handling).
    if !crate::build::run_site_build(root, out.to_str(), strict, None) {
        return ExitCode::FAILURE;
    }
    let out = out.canonicalize().unwrap_or(out);

    // Inject the passcode gate into the freshly built tree.
    if let Err(e) = inject_gate(&out) {
        log::error(&format!(
            "cannot write the passcode gate to {}: {e}",
            out.join("functions/_middleware.js").display()
        ));
        return ExitCode::FAILURE;
    }

    let cmd = format!(
        "wrangler pages deploy . --project-name {project} --branch {PRODUCTION_BRANCH} --commit-dirty=true"
    );
    if dry_run {
        log::info(&format!("built + gated {} (not deployed)", out.display()));
        println!("would run (cwd {}): {cmd}", out.display());
        return ExitCode::SUCCESS;
    }

    log::info(&format!("deploying {} to Cloudflare Pages ({project})", out.display()));
    let status = std::process::Command::new("wrangler")
        .current_dir(&out)
        .args([
            "pages",
            "deploy",
            ".",
            "--project-name",
            &project,
            "--branch",
            PRODUCTION_BRANCH,
            "--commit-dirty=true",
        ])
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("published: https://{project}.pages.dev");
            ExitCode::SUCCESS
        }
        Ok(s) => {
            log::error(&format!("wrangler exited with status {s}"));
            ExitCode::FAILURE
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::error(
                "wrangler was not found on PATH. Install it (npm install -g wrangler) and \
                 run the one-time setup: `wrangler pages project create <name> \
                 --production-branch production` then `wrangler pages secret put PASSWORD \
                 --project-name <name>`.",
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            log::error(&format!("cannot run wrangler: {e}"));
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 6: Write the failing integration tests**

Create `crates/server/tests/publish.rs`:

```rust
//! `taliesin publish` end to end, without hitting Cloudflare: `--dry-run` builds + gates
//! and prints the exact wrangler command; a real publish fails fast when the API token
//! is absent.

use std::path::Path;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_taliesin")
}

#[test]
fn dry_run_builds_gates_and_prints_the_wrangler_command() {
    let out = std::env::temp_dir().join(format!("tali-pub-dry-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let res = Command::new(bin())
        .args(["publish", "corpus/demo-book", "--out"])
        .arg(&out)
        .arg("--dry-run")
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .output()
        .expect("run publish --dry-run");
    assert!(
        res.status.success(),
        "dry-run should succeed: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    // The passcode gate was injected into the built tree.
    let mw = out.join("functions").join("_middleware.js");
    let body = std::fs::read_to_string(&mw).expect("middleware injected");
    assert!(body.contains("export async function onRequest"), "{body}");
    assert!(body.contains("env.PASSWORD"), "{body}");
    assert!(body.contains("WWW-Authenticate"), "{body}");
    // The exact command is printed (project name = dir slug "demo-book").
    let stdout = String::from_utf8_lossy(&res.stdout);
    assert!(
        stdout.contains(
            "wrangler pages deploy . --project-name demo-book --branch production --commit-dirty=true"
        ),
        "stdout was: {stdout}"
    );
    // The site actually built.
    assert!(out.join("index.html").exists(), "site built to out");
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn real_publish_without_token_fails_fast() {
    let res = Command::new(bin())
        .args(["publish", "corpus/demo-book"])
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .env_remove("CLOUDFLARE_API_TOKEN")
        .output()
        .expect("run publish");
    assert!(!res.status.success(), "must fail without a token");
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("CLOUDFLARE_API_TOKEN"),
        "stderr should name the missing token: {stderr}"
    );
}

// Silence dead_code on the helper if only one test uses it in some configs.
#[allow(dead_code)]
fn _root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
```

Note: `corpus/demo-book` is resolved relative to the workspace root; the tests set `current_dir` to the workspace root (`CARGO_MANIFEST_DIR` for the server crate is `crates/server`, so `../..` is the workspace root).

- [ ] **Step 7: Register the module so it compiles**

This step is completed in Task 6 (adding `mod publish;` to `main.rs`). To run these tests now, temporarily add `mod publish;` to `crates/server/src/main.rs` after `mod protocol;` (Task 6 confirms/keeps it).

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p taliesin-server --lib publish && cargo test -p taliesin-server --test publish`
Expected: PASS. (`dry_run_builds_...` builds `corpus/demo-book`; with no kernel, cells emit as source and the build still succeeds. `real_publish_without_token_fails_fast` fails fast before building.)

- [ ] **Step 9: Commit**

```bash
git add crates/server/src/publish.rs crates/server/tests/publish.rs crates/server/src/main.rs
git commit -m "feat(publish): build + gate + wrangler deploy subcommand"
```

---

### Task 6: Wire `publish` into the CLI (dispatch, help, usage) (server crate)

**Files:**
- Modify: `crates/server/src/main.rs` (`mod publish;`, dispatch arm, `COMMANDS`, `usage()`, `subcommand_help`, microcopy test)

**Interfaces:**
- Consumes: `publish::cmd_publish` (Task 5).

- [ ] **Step 1: Add the failing microcopy assertion**

In `crates/server/src/main.rs`, in `cli_microcopy_tests::subcommand_help_covers_documented_commands`, add `"publish"` to the iterated array (line 265-267):

```rust
        for cmd in [
            "preview", "build", "check", "render", "schema", "vocab", "blocks", "init", "publish",
        ] {
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p taliesin-server --lib subcommand_help_covers_documented_commands`
Expected: FAIL (`help for `publish`` panics: no dedicated help page yet).

- [ ] **Step 3: Register the module + dispatch**

In `crates/server/src/main.rs`, add after `mod protocol;` (line 16):

```rust
mod publish;
```

(If Task 5 Step 7 already added it, leave the single declaration.)

Add a dispatch arm after the `build` arm (line 41):

```rust
        Some("publish") => publish::cmd_publish(&args),
```

Add `"publish"` to `COMMANDS` (line 77-80):

```rust
const COMMANDS: &[&str] = &[
    "render", "build", "blocks", "schema", "vocab", "check", "init", "serve", "preview", "dev",
    "publish", "help",
];
```

- [ ] **Step 4: Add the usage line + focused help**

In `usage()`, after the `build` block (after line 114), add:

```rust
    println!("  publish <dir> [--project-name <name>] [--out <dir>] [--strict] [--dry-run]");
    println!("                             build a site/book + deploy it to Cloudflare Pages");
    println!("                             behind a shared passcode (Wrangler direct upload);");
    println!("                             --dry-run builds + gates + prints the deploy command");
```

In `subcommand_help`, add an arm before `_ => return None` (line 238):

```rust
        "publish" => {
            "taliesin publish <dir> [--project-name <name>] [--out <dir>] [--strict] [--dry-run]\n\
             \n\
             Build a site or book and deploy it to Cloudflare Pages (Wrangler direct\n\
             upload) behind a shared passcode. One-way: it never writes to your source.\n\
             The passcode lives only as a Cloudflare secret, never in your repo.\n\
             \n\
             Flags:\n\
             \x20 --project-name <name>  Cloudflare Pages project (default: the dir-name slug)\n\
             \x20 --out <dir>            build output dir (default: the project's _site/_book)\n\
             \x20 --strict               fail before deploying if the build has warnings\n\
             \x20 --dry-run              build + inject the gate, print the deploy command,\n\
             \x20                        do not deploy\n\
             \n\
             One-time setup (per repo):\n\
             \x20 export CLOUDFLARE_API_TOKEN=...   (also CLOUDFLARE_ACCOUNT_ID)\n\
             \x20 wrangler pages project create <name> --production-branch production\n\
             \x20 wrangler pages secret put PASSWORD --project-name <name>\n\
             \n\
             Example:\n\
             \x20 taliesin publish . --dry-run\n"
        }
```

- [ ] **Step 5: Run the microcopy + dispatch tests to verify they pass**

Run: `cargo test -p taliesin-server --lib subcommand_help_covers_documented_commands closest_command_suggests_nearest`
Expected: PASS.

- [ ] **Step 6: Manually verify the help renders**

Run: `cargo run -p taliesin-server -- publish --help`
Expected: prints the publish synopsis, flags, one-time setup, and example; exit 0.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/main.rs
git commit -m "feat(cli): wire the publish subcommand into dispatch + help"
```

---

### Task 7: Document `publish` in the User Guide (docs)

**Files:**
- Modify: `docs/guide/reference/cli.tmd` (the "Publishing & sharing" section, lines 145-181)

**Interfaces:** none (docs prose).

- [ ] **Step 1: Replace the "no built-in publish command yet" paragraph**

In `docs/guide/reference/cli.tmd`, replace the final paragraph of the "Publishing & sharing" section (lines 178-181, beginning "Taliesin has no built-in `publish` command yet") with:

```markdown
### `taliesin publish`: one command to a passcode-gated URL

For a private draft you want a supervisor or co-author to read at a stable URL,
`taliesin publish <dir>` builds the site or book, gates it behind a shared passcode,
and deploys it to [Cloudflare Pages](https://pages.cloudflare.com/) (free, works with a
private repo). The passcode is stored as a Cloudflare secret, never in your repo, and the
flow is one-way: publish never writes back to your source.

One-time setup per project (needs [Wrangler](https://developers.cloudflare.com/workers/wrangler/),
Cloudflare's CLI, and a Cloudflare API token with the Pages:Edit permission):

```sh
export CLOUDFLARE_API_TOKEN=...      # also CLOUDFLARE_ACCOUNT_ID
wrangler pages project create my-book --production-branch production
wrangler pages secret put PASSWORD --project-name my-book   # type the passcode once
```

Then, every time you want to update the live copy:

```sh
taliesin publish .                   # build, gate, deploy; prints https://my-book.pages.dev
taliesin publish . --dry-run         # build + gate + print the deploy command, no deploy
```

The Cloudflare project name defaults to a slug of the directory name; override it with
`--project-name` or a `publish:` block in `_site.yml`:

```yaml
publish:
  provider: cloudflare
  project: my-book
```

Your readers open the URL, type the passcode once in the browser's password prompt (no
account needed), and see the current build. A shared passcode keeps casual strangers out;
it is forwardable and is not per-person access control, which is the right level for a
work-in-progress draft to a supervisor. Note that a book using live Mermaid diagrams
still loads Mermaid from a CDN at view time (everything else is offline); this is harmless
behind the gate.
```

- [ ] **Step 2: Verify the guide still renders clean**

Run: `cargo run -p taliesin-server -- check docs/guide`
Expected: exit 0 (no broken links or diagnostics introduced by the edit).

- [ ] **Step 3: Commit**

```bash
git add docs/guide/reference/cli.tmd
git commit -m "docs(guide): document taliesin publish"
```

---

### Task 8: Final verification + backlog update

**Files:**
- Modify: `notes/backlog.md` (mark the `taliesin publish` item shipped)

- [ ] **Step 1: Full workspace test, clippy, fmt**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all PASS. (Also re-run the Function test: `node --test crates/server/src/assets/_middleware.test.mjs`.)

- [ ] **Step 2: End-to-end dry-run against a real project**

Run: `cargo run -p taliesin-server -- publish docs/guide --dry-run --out /tmp/tali-pub-guide`
Expected: builds the guide, prints `would run (cwd /tmp/tali-pub-guide): wrangler pages deploy . --project-name guide --branch production --commit-dirty=true`, exit 0. Confirm `/tmp/tali-pub-guide/functions/_middleware.js` exists.

- [ ] **Step 3: Update the backlog**

In `notes/backlog.md`, replace the `taliesin publish` bullet (the one dated "Researched 2026-07-07 for the private research-paper-draft workflow", currently under Tier 3) with a one-line shipped note:

```markdown
- **`taliesin publish` — SHIPPED 2026-07-08.** Build + shared-passcode gate
  (`functions/_middleware.js`) + `wrangler pages deploy` to Cloudflare Pages; project name
  defaults to a dir-name slug, override via `publish:` in `_site.yml`. Passcode is a
  Cloudflare secret (never in git); one-way flow. Spec/plan under `docs/superpowers/`.
  Follow-ups (not built): optional `--init` wrapper for the one-time `wrangler` setup;
  email-allowlist (Cloudflare Access) mode.
```

- [ ] **Step 4: Commit**

```bash
git add notes/backlog.md
git commit -m "chore(backlog): mark taliesin publish shipped"
```

---

## Self-Review

**1. Spec coverage.** Every spec section maps to a task:
- Command surface (`publish`, `--strict`, `--dry-run`) → Tasks 5, 6. Added `--project-name` and `--out` (test isolation + override); documented.
- Per-run flow (build → inject → preflight → deploy → URL) → Task 5 `cmd_publish`.
- `--branch production --commit-dirty=true` (production-deploy correctness) → Task 5 (`PRODUCTION_BRANCH`), asserted in Task 5 Step 6 and Task 8 Step 2.
- Zero-config default (dir slug) + `publish:` closed block → Tasks 1, 5.
- One-time setup documented, not automated → Tasks 6 (help) + 7 (guide).
- Passcode gate `functions/_middleware.js` (fail-closed, constant-time, 401) → Task 3, behavior pinned by the node test.
- Code locations (main.rs, publish.rs, config, schema, bundled asset) → Tasks 1-6.
- Tests (dry-run, config, slug, middleware, preflight) → Tasks 1, 3, 5.
- Injection after build so stale-sweep is unaffected → Task 5 `inject_gate` (called post-build), documented in the fn comment.

**2. Placeholder scan.** No TBD/TODO; every code step shows complete code; every run step states the exact command and expected result.

**3. Type consistency.** `run_site_build(root, out_override: Option<&str>, strict, jobs: Option<usize>) -> bool` is defined in Task 4 and called in Task 5 with `out.to_str()` (Option<&str>) and `None` (jobs). `PublishConfig { provider, project }` and `SiteConfig.publish` (Task 1) are read in Task 5 as `site.config.publish.as_ref()...`. `PUBLISH_KEYS` (Task 1) consumed in Task 2. `MIDDLEWARE_JS = include_str!("assets/_middleware.js")` (Task 5) matches the file created in Task 3. `cmd_publish(args: &[String]) -> ExitCode` (Task 5) matches the dispatch arm (Task 6). `slug`/`parse_publish_args`/`PublishArgs` names match between the implementation and the unit tests.

One coordination note flagged for the executor: Task 5 Step 7 adds `mod publish;` to `main.rs` so its tests compile; Task 6 Step 3 keeps exactly one such declaration (do not duplicate it).
