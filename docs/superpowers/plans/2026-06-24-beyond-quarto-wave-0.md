# Beyond Quarto Wave 0 (Integrity & Foundation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pay the correctness debt left by DROP-QUARTO so the repo is honest and public-ready: a real version + colophon, a truthful `THIRD_PARTY.md` that cannot silently rot, and docs that no longer describe deleted Quarto machinery.

**Architecture:** Three independent tasks, each landing its own committed, testable deliverable. Task 1 (version-stamp) touches only the server crate's CLI + a new build script. Task 2 (third-party-truth) rewrites one doc and adds a grep regression test in the core test crate. Task 3 (prune-and-fix-stale-docs) annotates two front-matter keys and fixes three doc files, guarded by lightweight "stale phrase is gone" assertions. No task touches the block model, the diff, sourcepos, exec/freeze/kernel, or any Do-NOT-touch machinery.

**Tech Stack:** Rust (edition 2024, workspace resolver 3), hand-rolled CLI arg parsing (no clap), `include_str!`-bundled assets, integration tests under `crates/*/tests/` using `env!("CARGO_MANIFEST_DIR")` + `env!("CARGO_BIN_EXE_qmd-fast")`.

## Global Constraints

- Rust edition 2024, workspace resolver 3. Shared deps live in the root `[workspace.dependencies]`; do not add a dependency to a crate's own `[dependencies]` if it belongs at the workspace level. (Wave 0 adds no new runtime crate dependency.)
- Writing style: never use em dashes or en dashes in any prose authored here (docs, comments, commit messages). Use commas, colons, parentheses, or restructured sentences.
- Every `.rs` file is auto-formatted by a `rustfmt` hook; CI enforces `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. Each task must end green on all three.
- Corpus-plus-roadmap discipline: do not make any change that regresses a `corpus/` document into a new warning. Specifically, `title-block-banner` and `site-url` are used by corpus front matter and must keep parsing warning-free.
- The binary is named `qmd-fast`; the single source of truth for the version is `qmd_fast_core::VERSION` (`crates/core/src/lib.rs:48`, `env!("CARGO_PKG_VERSION")`), inherited from the root `[workspace.package] version`.

---

### Task 1: version-stamp (real version + git-SHA colophon)

`--version`/`-V` already dispatch and print `qmd-fast {VERSION}` (`crates/server/src/main.rs:29-32`); the usage banner prints it too (`main.rs:638-640`). The only gaps: the workspace version is `0.0.0`, and there is no build colophon (git SHA). This task bumps the version to `0.1.0` and adds a short SHA via a new server-crate build script, surfaced in both `--version` and the usage banner.

**Files:**
- Modify: `Cargo.toml:6` (workspace version `0.0.0` -> `0.1.0`)
- Create: `crates/server/build.rs`
- Modify: `crates/server/src/main.rs:29-32` (`--version` arm) and `main.rs:638-640` (`usage()`)
- Test: `crates/server/tests/version.rs`

**Interfaces:**
- Consumes: `qmd_fast_core::VERSION: &str` (already exists).
- Produces: a compile-time env var `QMD_FAST_GIT_SHA` (set by the new build script, read in `main.rs` via `env!("QMD_FAST_GIT_SHA")`); the `--version` stdout line now reads `qmd-fast 0.1.0 (<sha-or-unknown>)`.

- [ ] **Step 1: Write the failing test**

Create `crates/server/tests/version.rs`:

```rust
use std::process::Command;

/// `--version` prints the bumped semver plus a parenthesized build colophon.
#[test]
fn version_flag_prints_semver_and_colophon() {
    let out = Command::new(env!("CARGO_BIN_EXE_qmd-fast"))
        .arg("--version")
        .output()
        .expect("the qmd-fast binary should run");
    assert!(out.status.success(), "exit status: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with("qmd-fast 0.1.0 ("),
        "expected `qmd-fast 0.1.0 (<sha>)`, got: {stdout:?}"
    );
    assert!(
        stdout.trim_end().ends_with(')'),
        "colophon should be wrapped in parentheses, got: {stdout:?}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p qmd-fast-server --test version`
Expected: FAIL. Before the version bump and colophon, stdout is `qmd-fast 0.0.0` with no parenthesized SHA, so `starts_with("qmd-fast 0.1.0 (")` fails.

- [ ] **Step 3: Bump the workspace version**

In `Cargo.toml`, change line 6 inside `[workspace.package]`:

```toml
version = "0.1.0"
```

- [ ] **Step 4: Add the build script that emits the git SHA**

Create `crates/server/build.rs`:

```rust
//! Emit a short git SHA so the CLI can print a build colophon. Falls back to
//! "unknown" outside a git checkout (e.g. a packaged crate), so the build never
//! fails for lack of git.
use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=QMD_FAST_GIT_SHA={sha}");
    // Re-run when the checked-out commit moves.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
}
```

- [ ] **Step 5: Surface the SHA in `--version` and the usage banner**

In `crates/server/src/main.rs`, change the `--version` arm (lines 29-32) from:

```rust
        Some("--version" | "-V") => {
            println!("qmd-fast {}", qmd_fast_core::VERSION);
            ExitCode::SUCCESS
        }
```

to:

```rust
        Some("--version" | "-V") => {
            println!("qmd-fast {} ({})", qmd_fast_core::VERSION, env!("QMD_FAST_GIT_SHA"));
            ExitCode::SUCCESS
        }
```

And in `usage()` (line 639), change:

```rust
    println!("qmd-fast {}", qmd_fast_core::VERSION);
```

to:

```rust
    println!("qmd-fast {} ({})", qmd_fast_core::VERSION, env!("QMD_FAST_GIT_SHA"));
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p qmd-fast-server --test version`
Expected: PASS. (Also run `cargo run -p qmd-fast-server -- --version` to eyeball, e.g. `qmd-fast 0.1.0 (cdadb9d)`.)

- [ ] **Step 7: Verify the existing version test still holds + fmt/clippy**

Run: `cargo test -p qmd-fast-core version_is_present && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS / clean. (`crates/core/src/lib.rs:54-57` only asserts `!VERSION.is_empty()`, still true.)

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/server/build.rs crates/server/src/main.rs crates/server/tests/version.rs
git commit -m "feat(cli): bump version to 0.1.0 + git-SHA build colophon"
```

---

### Task 2: third-party-truth (truthful THIRD_PARTY.md + a rot-proof test)

`THIRD_PARTY.md` still lists reveal.js + highlight.js (both deleted) and omits the vendored d3 (`d3.min.js`, ISC, v7.9.0) and Observable Plot (`plot.umd.min.js`, ISC, v0.6.16) that actually ship; mermaid is the sole CDN dependency (`crates/core/src/render/mod.rs:970`). This task rewrites the file to reality and adds a grep test that fails if any vendored `assets/js` file is undocumented or a removed dependency reappears.

**Files:**
- Modify: `THIRD_PARTY.md` (full rewrite)
- Test: `crates/core/tests/third_party.rs`
- Create: `deny.toml` (repo root)
- Modify: `.github/workflows/ci.yml` (add a `cargo deny` step to the existing `audit` job)

**Interfaces:**
- Consumes: the files under `crates/core/assets/js/` and the root `THIRD_PARTY.md`.
- Produces: a regression contract that `THIRD_PARTY.md` names every vendored JS asset by filename (`d3.min.js`, `plot.umd.min.js`) and never names a removed dependency (`reveal.js`, `highlight.js`).

- [ ] **Step 1: Write the failing test**

Create `crates/core/tests/third_party.rs`:

```rust
use std::path::Path;

/// qmd-fast's OWN (MIT) bundled scripts. Everything else in `assets/js/` is a
/// vendored third party that MUST be attributed by filename in THIRD_PARTY.md.
/// Adding a new vendored lib without documenting it fails `vendored_js_is_attributed`.
const OWN_JS: &[&str] = &["code-enhance.js", "deck.js", "mermaid.js", "qmd-js.js"];

fn third_party_md() -> String {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(core.join("../../THIRD_PARTY.md"))
        .expect("THIRD_PARTY.md should exist at the repo root")
}

#[test]
fn vendored_js_is_attributed() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc = third_party_md();
    let js_dir = core.join("assets/js");
    for entry in std::fs::read_dir(&js_dir).expect("assets/js should exist") {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        if !name.ends_with(".js") || OWN_JS.contains(&name.as_str()) {
            continue;
        }
        assert!(
            doc.contains(&name),
            "vendored asset `{name}` is not attributed in THIRD_PARTY.md \
             (document it, or add it to OWN_JS if it is qmd-fast's own)"
        );
    }
}

#[test]
fn removed_deps_are_not_listed() {
    let doc = third_party_md();
    for gone in ["reveal.js", "highlight.js"] {
        assert!(
            !doc.contains(gone),
            "THIRD_PARTY.md still lists removed dependency `{gone}`"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p qmd-fast-core --test third_party`
Expected: FAIL on both tests. `vendored_js_is_attributed` fails because the current doc lacks `d3.min.js`/`plot.umd.min.js`; `removed_deps_are_not_listed` fails because it still contains `reveal.js` and `highlight.js`.

- [ ] **Step 3: Rewrite THIRD_PARTY.md to reality**

Overwrite `THIRD_PARTY.md` with:

```markdown
# Third-party components

qmd-fast itself is MIT licensed (see [LICENSE](LICENSE)). It builds on the
following third-party work.

## Vendored (redistributed with this project)

Bundled so the tool works fully offline.

- **KaTeX** (MIT, Copyright (c) 2013-2020 Khan Academy and other contributors).
  The stylesheet and WOFF2 fonts under `crates/core/assets/katex/` render math
  offline. License: <https://github.com/KaTeX/KaTeX/blob/main/LICENSE>.
- **D3** (`crates/core/assets/js/d3.min.js`, ISC, v7.9.0, Copyright 2010-2023
  Mike Bostock). The plotting primitive used by `{js}` cells. License:
  <https://github.com/d3/d3/blob/main/LICENSE>.
- **Observable Plot** (`crates/core/assets/js/plot.umd.min.js`, ISC, v0.6.16,
  Copyright 2020-2023 Observable, Inc.). The high-level chart library for `{js}`
  cells; depends on the vendored D3 above. License:
  <https://github.com/observablehq/plot/blob/main/LICENSE>.

The other scripts under `crates/core/assets/js/` (`code-enhance.js`, `deck.js`,
`mermaid.js`, `qmd-js.js`) are qmd-fast's own (MIT).

## Loaded at runtime from a CDN (not redistributed here)

- **Mermaid** (MIT, diagrams). Pulled lazily from jsDelivr by the bundled
  `mermaid.js` loader only on pages that contain a `mermaid` block. This is the
  sole CDN dependency.

## Build dependencies

The Rust crates in `Cargo.lock` (comrak, axum, tokio, syntect, etc.) are fetched
by Cargo at build time under their own licenses (predominantly MIT, Apache-2.0,
and ISC). They are not redistributed in this repository. `deny.toml` pins the
allowed license set and CI runs `cargo deny check`.

## Note on Quarto

qmd-fast is an independent reimplementation of a subset of Quarto's `.qmd` ->
HTML behavior, not a copy of Quarto's source. "Quarto" is a trademark of its
owner; this project is not affiliated with or endorsed by it.
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p qmd-fast-core --test third_party`
Expected: PASS (both tests).

- [ ] **Step 5: Add `deny.toml` pinning the allowed license set**

Create `deny.toml` at the repo root:

```toml
# cargo-deny configuration. Run `cargo deny check`.
# Keeps the dependency tree to redistribution-friendly licenses and flags
# advisories, mirroring the licenses we actually ship (MIT/Apache/ISC/BSD).
[advisories]
version = 2

[licenses]
version = 2
allow = [
    "MIT",
    "Apache-2.0",
    "ISC",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Zlib",
    "CC0-1.0",
]
confidence-threshold = 0.9

[bans]
multiple-versions = "warn"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

- [ ] **Step 6: Verify cargo-deny locally if available**

Run: `cargo deny --version >/dev/null 2>&1 && cargo deny check || echo "cargo-deny not installed locally; CI will enforce"`
Expected: either `cargo deny check` passes (advisories/licenses/bans/sources OK), or the not-installed notice. If it runs and a license is rejected, add that exact SPDX id to the `allow` list (do not widen with a wildcard).

- [ ] **Step 7: Wire cargo-deny into CI**

In `.github/workflows/ci.yml`, inside the existing `audit` job (which already installs `cargo-audit` and runs `cargo audit`), add a cargo-deny install + check after the `cargo audit` step. Append these two steps to the `audit` job's `steps:` list (match the file's existing indentation):

```yaml
      - uses: taiki-e/install-action@cargo-deny
      - run: cargo deny check
```

- [ ] **Step 8: Run the full gate**

Run: `cargo test --workspace && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS / clean.

- [ ] **Step 9: Commit**

```bash
git add THIRD_PARTY.md crates/core/tests/third_party.rs deny.toml .github/workflows/ci.yml
git commit -m "docs(licensing): correct THIRD_PARTY.md + rot-proof test + cargo-deny"
```

---

### Task 3: prune-and-fix-stale-docs

Fix docs that contradict the post-DROP-QUARTO reality and a test (`quarto_shaped_config_is_no_longer_parsed_and_warns`, `crates/core/tests/config.rs:69`): `docs/guide/reference/configuration.qmd` still says a Quarto-shaped config "still works unchanged", `docs/internals/sites.qmd` still describes the deleted `site/config/quarto.rs` shim and lists "feeds", and `CLAUDE.md` still lists `RSS (feed.rs)` though `feed.rs` is gone and no RSS is produced. Annotate (do NOT remove) the two tolerated-but-unimplemented keys `title-block-banner` and `site-url`, which the corpus uses; their "recognized but not honored" disposition belongs to the Wave 1 validation epic.

**Files:**
- Modify: `crates/core/src/frontmatter.rs:33,49` (comments only) and its `#[cfg(test)] mod tests`
- Test: `crates/core/tests/stale_docs.rs`
- Modify: `docs/guide/reference/configuration.qmd` (lines 5-8 + 101-108)
- Modify: `docs/internals/sites.qmd` (lines 9-12 + 34-48)
- Modify: `CLAUDE.md:57`

**Interfaces:**
- Consumes: `qmd_fast_core::frontmatter::lint(src: &str) -> Vec<String>` (existing).
- Produces: a guard test that the stale phrases stay gone, and a unit test pinning that the two corpus-used keys never warn.

- [ ] **Step 1: Write the failing unit test (keys must not warn)**

In `crates/core/src/frontmatter.rs`, inside the existing `#[cfg(test)] mod tests` block (near the other `lint` tests around line 188), add:

```rust
    #[test]
    fn tolerated_unimplemented_keys_do_not_warn() {
        // `title-block-banner` and `site-url` are used by corpus front matter
        // (tech-blog, bayesian-book) but read by no code yet. They must stay in
        // KNOWN_KEYS so the corpus never warns; their "recognized but not
        // honored" disposition is the Wave 1 validation epic's job, not removal.
        let w = lint("---\ntitle: X\ntitle-block-banner: false\nsite-url: https://example.com\n---\n");
        assert!(w.is_empty(), "tolerated keys must not warn, got: {w:?}");
    }
```

- [ ] **Step 2: Run it to confirm it already passes (guard, not red)**

Run: `cargo test -p qmd-fast-core --lib frontmatter::tests::tolerated_unimplemented_keys_do_not_warn`
Expected: PASS. (Both keys are already in `KNOWN_KEYS` at `frontmatter.rs:33,49`; this test pins that intent so a future "prune" cannot silently regress the corpus. This is a guard test, so it is green from the start by design.)

- [ ] **Step 3: Annotate the two keys in KNOWN_KEYS**

In `crates/core/src/frontmatter.rs`, replace line 33:

```rust
    "title-block-banner",
```

with:

```rust
    // Tolerated but not yet honored: corpus front matter sets these, but no code
    // reads them. Kept so valid corpus docs do not warn; a future validation pass
    // may add a "recognized but not honored" note (see BEYOND-QUARTO.md Wave 1).
    "title-block-banner",
```

and replace line 49:

```rust
    "site-url",
```

with:

```rust
    "site-url", // tolerated but not honored (see the note above); used by corpus.
```

- [ ] **Step 4: Write the failing stale-docs guard test**

Create `crates/core/tests/stale_docs.rs`:

```rust
use std::path::Path;

fn read(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// These phrases describe machinery deleted in DROP-QUARTO and must not return.
#[test]
fn docs_do_not_claim_quarto_config_still_works() {
    let cfg = read("docs/guide/reference/configuration.qmd");
    assert!(
        !cfg.contains("still works"),
        "configuration.qmd still claims a Quarto config works"
    );
    assert!(
        !cfg.contains("Coming from a Quarto config?"),
        "configuration.qmd still has the stale Quarto-config callout"
    );
}

#[test]
fn internals_do_not_describe_the_deleted_shim() {
    let sites = read("docs/internals/sites.qmd");
    assert!(
        !sites.contains("site/config/quarto.rs"),
        "sites.qmd still describes the deleted quarto.rs shim"
    );
}

#[test]
fn claude_md_does_not_list_feed_rs() {
    let claude = read("CLAUDE.md");
    assert!(
        !claude.contains("feed.rs"),
        "CLAUDE.md still lists the deleted feed.rs"
    );
}
```

- [ ] **Step 5: Run it to verify it fails**

Run: `cargo test -p qmd-fast-core --test stale_docs`
Expected: FAIL on all three (the stale phrases are still present).

- [ ] **Step 6: Fix `docs/guide/reference/configuration.qmd`**

Replace lines 5-8:

```
Two places hold configuration: a document's **YAML front matter** (the `---`
block at the top), and a project's **`_site.yml`** (for a site or book). The
project config uses a flat, native schema; a Quarto-shaped config still works
unchanged (see the note at the end).
```

with:

```
Two places hold configuration: a document's **YAML front matter** (the `---`
block at the top), and a project's **`_site.yml`** (for a site or book). The
project config uses a flat, native schema. A Quarto-shaped nested config (a
`project:` / `website:` / `book:` / `format:` block) is no longer parsed: its
unknown top-level keys warn, and its nested values are ignored. Flatten it (see
the note at the end).
```

Then replace the callout at lines 101-108:

```
::: {.callout-note}
## Coming from a Quarto config?

Leave it as-is — the nested `project:` / `website:` / `book:` / `format:` shape is
detected and still renders. Or flatten it: lift `website: title:` → `title:`,
`format: html: toc:` → `toc:`, `website: navbar: left:` → `nav:`,
`page-footer:` → `footer:`, and `project: output-dir:` → `output:`.
:::
```

with:

```
::: {.callout-note}
## Coming from a Quarto config?

The nested Quarto shape is no longer parsed, so flatten it: lift
`website: title:` to `title:`, `format: html: toc:` to `toc:`,
`website: navbar: left:` to `nav:`, `page-footer:` to `footer:`, and
`project: output-dir:` to `output:`. Any leftover nested top-level key
(`project:`, `website:`, `book:`) will warn as unknown.
:::
```

- [ ] **Step 7: Fix `docs/internals/sites.qmd`**

Replace the intro fragment at lines 11-12 (the trailing "and feeds."):

```
rewriting, cross-page references, search, and feeds.
```

with:

```
rewriting, cross-page references, and search.
```

Then replace the whole stale section at lines 34-47:

```
## The config model, and the Quarto shim

The typed `SiteConfig` is qmd-fast's **native, flat** schema (title, nav, footer,
`mounts:`, …). Config is deliberately **two levels with no cascade**: the root
`_site.yml` sets project-wide defaults and a page's own front matter overrides
them, full stop. There is no `_metadata.yml` directory merge, the single most
confusing part of Quarto's model.

Existing Quarto-shaped configs still load, through one isolated shim
(`site/config/quarto.rs`) that translates the nested
`project:` / `website:` / `book:` / `format:` layout into the native `SiteConfig`.
It is **isolated on purpose**: to drop Quarto support entirely you delete that one
file and the single dispatch branch that calls it, and the native path and every
downstream consumer keep working.
```

with:

```
## The config model

The typed `SiteConfig` is qmd-fast's **native, flat** schema (title, nav, footer,
`mounts:`, ...). Config is deliberately **two levels with no cascade**: the root
`_site.yml` sets project-wide defaults and a page's own front matter overrides
them, full stop. There is no `_metadata.yml` directory merge, the single most
confusing part of Quarto's model.

The schema is a **closed set**: an unknown top-level key warns with a "did you
mean" hint (`config/mod.rs` `validate_keys`). A Quarto-shaped nested config
(`project:` / `website:` / `book:` / `format:`) is no longer accepted: those keys
warn as unknown and their nested values are ignored, so a Quarto config must be
flattened to the native shape.
```

- [ ] **Step 8: Fix `CLAUDE.md` file map**

Replace line 57:

```
                   front-matter parse (frontmatter.rs), books (book.rs), RSS (feed.rs),
```

with:

```
                   front-matter parse (frontmatter.rs), books (book.rs),
```

- [ ] **Step 9: Run the guard test to verify it passes**

Run: `cargo test -p qmd-fast-core --test stale_docs`
Expected: PASS (all three).

- [ ] **Step 10: Run the contradiction-source test + full gate**

Run: `cargo test -p qmd-fast-core quarto_shaped_config_is_no_longer_parsed_and_warns && cargo test --workspace && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS / clean. (The docs now agree with the behavior that test enforces.)

- [ ] **Step 11: Rebuild the docs books to confirm they still render**

Run: `cargo run -p qmd-fast-server -- build docs/guide >/dev/null && cargo run -p qmd-fast-server -- build docs/internals >/dev/null && echo OK`
Expected: `OK` with no error output (the edited `.qmd` prose still renders; this is the corpus-plus-roadmap "docs are dogfooded" check).

- [ ] **Step 12: Commit**

```bash
git add crates/core/src/frontmatter.rs crates/core/tests/stale_docs.rs docs/guide/reference/configuration.qmd docs/internals/sites.qmd CLAUDE.md
git commit -m "docs: remove stale Quarto-compat + feed.rs claims; annotate tolerated keys"
```

---

## Self-Review

**Spec coverage (against BEYOND-QUARTO.md Wave 0):**
- `prune-and-fix-stale-docs` -> Task 3 (configuration.qmd, sites.qmd, CLAUDE.md; the two keys kept + annotated, not pruned, because the corpus uses them, with a `tolerated_unimplemented_keys_do_not_warn` guard).
- `third-party-truth` (= backlog #5) -> Task 2 (rewrite + grep test + deny.toml + CI).
- `version-stamp` -> Task 1 (version bump + colophon; `--version` plumbing already existed, so the task is correctly scoped to the bump + SHA).
- Wave 1 items (`locate-render-warnings`, `nested-schema-validation`, `jsonschema-for-config`) are deliberately NOT in this plan; they are the next dedicated plan and depend on a located-diagnostics substrate that should land and be reviewed first.

**Placeholder scan:** No TBD/TODO. Every code step shows full content. The one conditional step (Step 6 of Task 2, `cargo deny` local run) is genuinely conditional on a local tool install and states the exact fallback, not a vague "handle errors".

**Type/string consistency:** `QMD_FAST_GIT_SHA` is spelled identically in `build.rs` (emit) and `main.rs` (`env!`). `OWN_JS` in Task 2 lists exactly the four own scripts the research confirmed (`code-enhance.js`, `deck.js`, `mermaid.js`, `qmd-js.js`); the vendored pair the rewritten doc names (`d3.min.js`, `plot.umd.min.js`) are exactly what the `vendored_js_is_attributed` test scans for. The stale-phrase assertions in Task 3 match the exact strings removed from each doc.

**Scope check:** Three independent, individually committable tasks. No task touches the block model, diff, sourcepos, exec/freeze/kernel, the `:::` machine, cite/includes/numbering, or any Do-NOT-touch machinery. No corpus document is regressed into a new warning.
