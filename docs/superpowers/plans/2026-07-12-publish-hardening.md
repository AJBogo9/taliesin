# Publish Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `taliesin publish` safe-by-default: strict checks on unless opted out, and a supported way to deploy a genuinely public (un-gated) site.

**Architecture:** All changes are on the `publish` command surface. Add a `gate: Option<bool>` config field (core), then in the `publish` CLI add a `--public` flag and a `--no-strict` flag, flip the strict default to `true`, and make gate injection conditional with a loud PUBLIC warning. No change to the gate mechanism, the build pipeline, or Wrangler.

**Tech Stack:** Rust (edition 2024), `serde_yaml` for config, the existing `taliesin` server binary + its integration test harness (`CARGO_BIN_EXE_taliesin`, `--dry-run`).

## Global Constraints

- Rust edition 2024, workspace resolver 3.
- Gate must **fail closed**: any ambiguity in resolving `gate` defaults to gated (passcode on). Only an explicit `--public` or explicit `gate: false` deploys public.
- No em dashes / en dashes in any user-facing string or log line (use a colon).
- A `PostToolUse` hook runs `rustfmt` on edited `.rs` files; keep the tree `cargo fmt`-clean.
- Spec: [docs/superpowers/specs/2026-07-12-publish-hardening-design.md](../specs/2026-07-12-publish-hardening-design.md).
- Branch: `publish-hardening` (already created off `main`).

## File Structure

- `crates/core/src/site/config/mod.rs` — add `gate: Option<bool>` to `PublishConfig`, parse it, add `"gate"` to `PUBLISH_KEYS` for the typo-lint. (~4 edits + 1 test.)
- `crates/server/src/publish.rs` — add `public` + flip `strict` default in `PublishArgs`, parse `--public`/`--no-strict`, resolve the gate decision + warning in `cmd_publish`, update usage/flags. (Unit tests in-file.)
- `crates/server/tests/publish.rs` — integration `--dry-run` tests for gate on/off and strict default.

---

### Task 1: Config `gate` field + typo-lint

**Files:**
- Modify: `crates/core/src/site/config/mod.rs` (struct `PublishConfig` ~L80, `PUBLISH_KEYS` ~L150, `publish_from` ~L379)
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `PublishConfig.gate: Option<bool>` — `Some(false)` = un-gated, `Some(true)`/`None` = gated. Consumed by Task 3.

- [ ] **Step 1: Write the failing test**

Add to the tests module in `crates/core/src/site/config/mod.rs`:

```rust
    #[test]
    fn publish_gate_false_parses() {
        let dir = tmp("publish-gate");
        std::fs::write(
            dir.join("_site.yml"),
            "title: Book\npublish:\n  provider: cloudflare\n  gate: false\n",
        )
        .unwrap();
        let mut warnings = Vec::new();
        let cfg = load_config(&dir, &mut warnings);
        let publish = cfg.publish.expect("publish block parsed");
        assert_eq!(publish.gate, Some(false));
        assert!(
            !warnings.iter().any(|w| w.contains("unknown")),
            "a valid gate must not warn: {warnings:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_gate_typo_warns() {
        let w = cfg_warnings("publish:\n  gat: false\n");
        assert!(
            w.iter().any(|w| w.contains("publish key `gat`") && w.contains("`gate`")),
            "gate typo did-you-mean: {w:?}"
        );
    }
```

Note: this mirrors the existing `publish_block_parses_provider_and_project` test exactly — `load_config(&dir, &mut warnings)` fills `warnings` by reference and returns the config; `cfg_warnings(yaml)` is the existing helper that returns just the warning vec.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p taliesin-core --lib publish_gate_false_parses unknown_gate_typo_warns`
Expected: FAIL — `gate` field does not exist / no `gate` did-you-mean.

- [ ] **Step 3: Add the field, parse, and key**

In `PublishConfig` (~L80), add the field:

```rust
pub struct PublishConfig {
    /// Deploy provider. Only `cloudflare` is recognized today.
    pub provider: Option<String>,
    /// Cloudflare Pages project name (overrides the dir-name slug default).
    pub project: Option<String>,
    /// Passcode gate. Absent or `true` = gated (the safe default); `false` = public.
    pub gate: Option<bool>,
}
```

In `PUBLISH_KEYS` (~L150), add `"gate"`:

```rust
pub(crate) const PUBLISH_KEYS: &[&str] = &["provider", "project", "gate"];
```

In `publish_from` (~L379), parse the bool:

```rust
    let s = |k: &str| pv.get(k).and_then(|x| x.as_str()).map(str::to_string);
    Some(PublishConfig {
        provider: s("provider"),
        project: s("project"),
        gate: pv.get("gate").and_then(|x| x.as_bool()),
    })
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-core --lib publish_gate_false_parses unknown_gate_typo_warns`
Expected: PASS. Also run the existing publish-config tests to confirm no regression:
Run: `cargo test -p taliesin-core --lib publish`
Expected: PASS (all).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/site/config/mod.rs
git commit -m "feat(config): publish.gate field + typo-lint (public opt-out)"
```

---

### Task 2: `publish` argv — `--public`, `--no-strict`, strict-by-default

**Files:**
- Modify: `crates/server/src/publish.rs` (`PublishArgs` ~L33, `PUBLISH_FLAGS` ~L29, `parse_publish_args` ~L42, usage string ~L80)
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `PublishArgs { path, project_name, out_dir, strict: bool (default true), dry_run, public: bool }`. Consumed by Task 3's `cmd_publish`.

- [ ] **Step 1: Write the failing tests**

Add to the tests module in `crates/server/src/publish.rs`:

```rust
    #[test]
    fn strict_is_the_default() {
        let p = parse_publish_args(&argv(&["book"])).expect("parse");
        assert!(p.strict, "publish must be strict by default");
        assert!(!p.public);
    }

    #[test]
    fn no_strict_opts_out_and_public_opts_in() {
        let p = parse_publish_args(&argv(&["book", "--no-strict", "--public"])).expect("parse");
        assert!(!p.strict);
        assert!(p.public);
    }

    #[test]
    fn strict_flags_are_last_wins() {
        let a = parse_publish_args(&argv(&["book", "--no-strict", "--strict"])).expect("parse");
        assert!(a.strict, "--strict after --no-strict wins");
        let b = parse_publish_args(&argv(&["book", "--strict", "--no-strict"])).expect("parse");
        assert!(!b.strict, "--no-strict after --strict wins");
    }

    #[test]
    fn public_typo_still_did_you_means() {
        let err = parse_publish_args(&argv(&["book", "--publik"])).unwrap_err();
        assert!(err.contains("--publik"), "{err}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p taliesin-server --bin taliesin strict_is_the_default no_strict_opts_out_and_public_opts_in strict_flags_are_last_wins public_typo_still_did_you_means`
Expected: FAIL — no `public` field, `strict` defaults false, unknown `--no-strict`/`--public`.

(If `--bin taliesin` is not the right selector, use `cargo test -p taliesin-server` and filter by test name; the binary crate's unit tests run under the package.)

- [ ] **Step 3: Add fields, flags, default, usage**

In `PUBLISH_FLAGS` (~L29) add the two new flags:

```rust
const PUBLISH_FLAGS: &[&str] = &[
    "--project-name",
    "--out",
    "--strict",
    "--no-strict",
    "--public",
    "--dry-run",
];
```

In `PublishArgs` (~L33) add `public`:

```rust
struct PublishArgs<'a> {
    path: &'a str,
    project_name: Option<&'a str>,
    out_dir: Option<&'a str>,
    strict: bool,
    dry_run: bool,
    public: bool,
}
```

In `parse_publish_args` (~L42), default `strict = true`, add `public`, and the two match arms:

```rust
    let mut project_name: Option<&str> = None;
    let mut out_dir: Option<&str> = None;
    let mut strict = true; // publish is strict by default; --no-strict opts out
    let mut dry_run = false;
    let mut public = false;
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // ...existing --project-name / --out arms unchanged...
            "--strict" => strict = true,
            "--no-strict" => strict = false,
            "--public" => public = true,
            "--dry-run" => dry_run = true,
            // ...existing unknown-flag + positional arms unchanged...
        }
    }
```

Update the usage string (~L80) and the returned struct:

```rust
    let path = positionals.first().copied().ok_or_else(|| {
        "usage: taliesin publish <dir> [--project-name <name>] [--out <dir>] [--public] [--no-strict] [--dry-run]"
            .to_string()
    })?;
    Ok(PublishArgs {
        path,
        project_name,
        out_dir,
        strict,
        dry_run,
        public,
    })
```

Also update the existing `parses_path_and_flags` test: it passes `--strict` and asserts `p.strict` is true, which still holds; add `assert!(!p.public);` is optional. Leave it if it still compiles (the struct gained a field but the test builds via `parse_publish_args`, not a literal, so it is unaffected).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-server strict_is_the_default no_strict_opts_out_and_public_opts_in strict_flags_are_last_wins public_typo_still_did_you_means parses_path_and_flags`
Expected: PASS (all).

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/publish.rs
git commit -m "feat(publish): --public + --no-strict flags; strict by default"
```

---

### Task 3: Gate decision + PUBLIC warning in `cmd_publish` (+ integration)

**Files:**
- Modify: `crates/server/src/publish.rs` (`cmd_publish` ~L119: destructure `public`, resolve gate, conditional `inject_gate`, dry-run message)
- Test: `crates/server/tests/publish.rs`

**Interfaces:**
- Consumes: `PublishArgs.public` (Task 2), `PublishConfig.gate` (Task 1).
- Produces: no new type; the observable contract is "gate written iff gated", asserted by the integration tests.

- [ ] **Step 1: Write the failing integration tests**

Add to `crates/server/tests/publish.rs`:

```rust
#[test]
fn public_flag_skips_the_gate_and_warns() {
    let out = std::env::temp_dir().join(format!("tali-pub-public-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let res = Command::new(bin())
        .args(["publish", "corpus/demo-book", "--out"])
        .arg(&out)
        .args(["--public", "--dry-run"])
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .output()
        .expect("run publish --public --dry-run");
    assert!(
        res.status.success(),
        "public dry-run should succeed: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    // No passcode gate injected.
    assert!(
        !out.join("functions").join("_middleware.js").exists(),
        "--public must not inject the gate"
    );
    // A loud PUBLIC warning was printed.
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("PUBLIC"),
        "must warn that the site is public: {stderr}"
    );
    // The site still built.
    assert!(out.join("index.html").exists(), "site built to out");
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn gate_false_config_skips_the_gate() {
    // A throwaway one-page site with publish.gate: false.
    let root = std::env::temp_dir().join(format!("tali-pub-gatecfg-{}", std::process::id()));
    let out = std::env::temp_dir().join(format!("tali-pub-gatecfg-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("_site.yml"),
        "title: Pub\npublish:\n  gate: false\n",
    )
    .unwrap();
    std::fs::write(root.join("index.tmd"), "---\ntitle: Home\n---\n\nHi.\n").unwrap();
    let res = Command::new(bin())
        .args(["publish"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .arg("--dry-run")
        .output()
        .expect("run publish --dry-run");
    assert!(
        res.status.success(),
        "dry-run should succeed: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    assert!(
        !out.join("functions").join("_middleware.js").exists(),
        "publish.gate: false must not inject the gate"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}
```

The existing `dry_run_builds_gates_and_prints_the_wrangler_command` test is the gated-by-default half: keep it as-is (it asserts the gate IS written on a plain dry-run). Together with `public_flag_skips_the_gate_and_warns` it is the gate-the-gate mutation check: an unconditional `inject_gate` fails the second test, an always-skipped one fails the first.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p taliesin-server --test publish public_flag_skips_the_gate_and_warns gate_false_config_skips_the_gate`
Expected: FAIL — the gate is currently injected unconditionally, so `_middleware.js` exists and no PUBLIC warning is printed.

- [ ] **Step 3: Make gate injection conditional + warn**

In `cmd_publish` (~L119), destructure `public` from the parsed args:

```rust
    let PublishArgs {
        path,
        project_name,
        out_dir,
        strict,
        dry_run,
        public,
    } = match parse_publish_args(args) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };
```

After the site is discovered (the `let site = taliesin_core::Site::discover(root);` line ~L154), resolve the gate decision (precedence: `--public` > `publish.gate` > default gated):

```rust
    // Precedence: --public wins, else publish.gate: in _site.yml, else gated (safe default).
    let gated = if public {
        false
    } else {
        site.config
            .publish
            .as_ref()
            .and_then(|p| p.gate)
            .unwrap_or(true)
    };
    if !gated {
        log::warn("publishing WITHOUT a passcode gate: this site will be PUBLIC");
    }
```

Then wrap the existing `inject_gate` call (~L194) in the condition:

```rust
    // Inject the passcode gate into the freshly built tree (unless deploying public).
    if gated {
        if let Err(e) = inject_gate(&out) {
            log::error(&format!(
                "cannot write the passcode gate to {}: {e}",
                out.join("functions/_middleware.js").display()
            ));
            return ExitCode::FAILURE;
        }
    }
```

Update the dry-run success message (~L206) to reflect the decision rather than always saying "gated":

```rust
    if dry_run {
        let gate_note = if gated { "gated" } else { "PUBLIC (no gate)" };
        log::info(&format!("built + {gate_note} {} (not deployed)", out.display()));
        println!("would run (cwd {}): {cmd}", out.display());
        return ExitCode::SUCCESS;
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p taliesin-server --test publish`
Expected: PASS (all four: the two new ones + the two originals, including the still-gated default).

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/publish.rs crates/server/tests/publish.rs
git commit -m "feat(publish): skip the passcode gate for --public / gate: false, warn loudly"
```

---

### Task 4: Strict-by-default is enforced on deploy (integration)

**Files:**
- Test: `crates/server/tests/publish.rs`

**Interfaces:**
- Consumes: the strict-by-default default from Task 2 + the existing `run_site_build(strict)` path (`cmd_publish` returns `FAILURE` when `run_site_build` returns `false`).

- [ ] **Step 1: Write the failing test**

Add to `crates/server/tests/publish.rs` a helper that writes a site with a broken cross-page link, and two assertions:

```rust
/// Write a throwaway one-page site whose only page links a page that does not exist,
/// so the strict check has a real problem to fail on. Returns the site root.
fn broken_site(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("tali-pub-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("_site.yml"), "title: Broken\n").unwrap();
    std::fs::write(
        root.join("index.tmd"),
        "---\ntitle: Home\n---\n\nSee [the missing page](nonexistent.tmd).\n",
    )
    .unwrap();
    root
}

#[test]
fn strict_by_default_fails_a_broken_site() {
    let root = broken_site("strict");
    let out = std::env::temp_dir().join(format!("tali-pub-strict-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let res = Command::new(bin())
        .args(["publish"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .arg("--dry-run")
        .output()
        .expect("run publish --dry-run");
    assert!(
        !res.status.success(),
        "a broken cross-ref must fail the strict-by-default deploy: {}",
        String::from_utf8_lossy(&res.stdout)
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn no_strict_lets_a_broken_site_through() {
    let root = broken_site("nostrict");
    let out = std::env::temp_dir().join(format!("tali-pub-nostrict-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let res = Command::new(bin())
        .args(["publish"])
        .arg(&root)
        .arg("--out")
        .arg(&out)
        .args(["--no-strict", "--dry-run"])
        .output()
        .expect("run publish --no-strict --dry-run");
    assert!(
        res.status.success(),
        "--no-strict must let the broken site build + (dry) deploy: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}
```

- [ ] **Step 2: Run tests to verify the pair behaves**

Run: `cargo test -p taliesin-server --test publish strict_by_default_fails_a_broken_site no_strict_lets_a_broken_site_through`
Expected: Both PASS immediately (Task 2 already flipped the default; this task is the regression pin). If `strict_by_default_fails_a_broken_site` does NOT fail the build, investigate whether a broken intra-site link is actually counted as a strict problem by `run_site_build`; if not, switch the fixture to a broken cross-ref (`See @sec-nope.`) which `validate_xrefs` flags, and re-run. Document which construct was used.

- [ ] **Step 3: Confirm the whole publish suite is green**

Run: `cargo test -p taliesin-server --test publish`
Expected: PASS (all).

- [ ] **Step 4: Commit**

```bash
git add crates/server/tests/publish.rs
git commit -m "test(publish): pin strict-by-default + --no-strict escape"
```

---

### Task 5: Full verification + docs sweep

**Files:**
- Modify (if present): any `publish` reference in `docs/guide/` that lists its flags or says it always gates. Search first.

- [ ] **Step 1: Search for docs that describe publish flags / gating**

Run: `grep -rn "publish" docs/guide docs/internals --include=*.tmd | grep -i "gate\|strict\|passcode\|--"`
Expected: a short list. For each hit that enumerates publish flags or states the gate is unconditional, add `--public` / `--no-strict` and note the gate is default-on but opt-out. Keep edits minimal and in the existing voice; do not reformat surrounding lines.

- [ ] **Step 2: Full workspace test + lint**

Run: `cargo test -p taliesin-core -p taliesin-server`
Expected: PASS.
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.
Run: `cargo fmt --check`
Expected: clean.

- [ ] **Step 3: Commit any doc edits**

```bash
git add docs/
git commit -m "docs(publish): document --public + --no-strict"
```

(Skip the commit if the grep found nothing to change.)

---

## Self-Review Notes

- **Spec coverage:** #15 gate opt-out → Tasks 1 (config), 3 (flag + decision + warning). #16 strict default → Tasks 2 (default flip + flag), 4 (enforcement pin). Precedence rule → Task 3 `gated` resolution. Typo-lint → Task 1. Gate-the-gate → Task 3 (the two opposing gate tests). Docs follow-up → Task 5. The `deploy`-skill retirement is out of scope per the spec (a later cleanup), so no task.
- **Placeholder scan:** none — every code step shows the code; Task 4 Step 2 names the exact fallback fixture if the primary one does not trip strict.
- **Type consistency:** `PublishConfig.gate: Option<bool>` (Task 1) is read in Task 3 as `.and_then(|p| p.gate).unwrap_or(true)`; `PublishArgs.public: bool` (Task 2) is destructured in Task 3; `gated: bool` is local to `cmd_publish`. Consistent throughout.
