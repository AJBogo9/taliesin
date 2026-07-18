# DX1 — Live Preview Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run the located static validators (broken links, missing images/assets/media, dup heading ids, dangling anchors, a11y, math, code-langs) plus cross-page link checking on **both** live-preview serve paths, feeding the dev-menu diagnostics list + badge that already exist.

**Architecture:** A new `crates/server/src/preview_diag.rs` bridge module converts the `check`-superset validators (`check::page_static_diagnostics`, `Site::validate_cross_page_links`, `Site::warnings`) into `protocol::Diagnostic`s. The single-doc serve (`serve/mod.rs::rebuild`) and the site serve (`serve_site/mod.rs::build_page`) each call the bridge and `extend` their existing `diags` vec. No client change (the `◇</>` badge + click-to-source list already consume `protocol::diagnostics`). Static lints run on **pre-execution** blocks; cross-page checking runs whole-site (~27 ms) filtered to the current page.

**Tech Stack:** Rust (edition 2024), `taliesin-server` bin crate, `taliesin-core` lib. In-crate `#[cfg(test)]` unit tests (the bin crate exposes no lib target, so `tests/*.rs` integration tests cannot see `pub(crate)` items — the pattern here is extract-pure-fn + in-crate unit test).

## Global Constraints

- Edition 2024, workspace resolver 3; shared deps via root `[workspace.dependencies]`.
- Invariant-safe: no new output format, no CDN, no preview write-back, `--tali-*` tokens only. The `data-block-id` + `data-sourcepos` block model and the `MAX_WARM_PAGES` + `exec_pool.rs` eviction freeze are untouched — this only reads already-rendered blocks and pushes onto the existing `protocol::diagnostics` transport.
- Static lints MUST run on **pre-execution** blocks: a cell-spliced matplotlib figure linted for alt-text would report a defect the author cannot fix in source (this is why `check` lints before executing).
- The serve helpers return `crate::protocol::Diagnostic` (the client wire type), NOT `crate::check::Diagnostic` (the CLI-JSON type). Do not reuse `check::diag_from` / `check::Diagnostic::new` — wrong type.
- `page_static_diagnostics` and `Scope` are `pub(crate)` in `check.rs`; reachable from a sibling module.
- Positive "corpus pin" is the clean-doc-yields-empty unit test, NOT a new corpus doc — a deliberately-broken corpus doc would fail the "corpus renders clean" regression net.
- A `PostToolUse` hook runs `rustfmt` on every edited `.rs`; keep the tree `cargo fmt`-clean. Branch is `dx1-live-preview-validation` (already created; spec committed at `b104719`).

## File Structure

- **Create** `crates/server/src/preview_diag.rs` — the bridge: `static_diagnostics`, `cross_page_diagnostics`, `site_config_diagnostics`, the private `located` converter, and all four in-crate unit tests. One responsibility: turn check-superset validators into `protocol::Diagnostic`s for the live preview.
- **Modify** `crates/server/src/main.rs` — register `mod preview_diag;`.
- **Modify** `crates/server/src/serve/mod.rs` — `rebuild` calls `preview_diag::static_diagnostics` (Standalone) on pre-exec blocks.
- **Modify** `crates/server/src/serve_site/mod.rs` — `build_page` calls `static_diagnostics` (InSite) pre-exec + `cross_page_diagnostics` + `site_config_diagnostics`.

---

### Task 1: `preview_diag` module — static-diagnostics bridge + tests

**Files:**
- Create: `crates/server/src/preview_diag.rs`
- Modify: `crates/server/src/main.rs` (add `mod preview_diag;` beside the other `mod` lines)
- Test: in-module `#[cfg(test)] mod tests` in `crates/server/src/preview_diag.rs`

**Interfaces:**
- Consumes: `crate::check::{Scope, page_static_diagnostics}` (`pub(crate)`), `crate::protocol::Diagnostic`, `taliesin_core::{Block, DocFormat, render::Warning}`.
- Produces: `pub(crate) fn static_diagnostics(src: &str, blocks: &[taliesin_core::Block], base: &Path, format: taliesin_core::DocFormat, scope: crate::check::Scope) -> Vec<crate::protocol::Diagnostic>` (Task 2 + 4 consume this).

- [ ] **Step 1: Register the module**

In `crates/server/src/main.rs`, add next to the existing `mod` declarations:

```rust
mod preview_diag;
```

- [ ] **Step 2: Write the module with the helper (test comes next; write the impl first so the crate compiles for the failing-test run)**

Create `crates/server/src/preview_diag.rs`:

```rust
//! Bridge: run the static validators (the `check` superset) over an already-rendered
//! preview document and convert them into `protocol::Diagnostic`s for the dev menu, so
//! the live preview and `check` cannot drift on what counts as a defect.
//!
//! Returns `protocol::Diagnostic` (the client wire type), using the exact
//! `Warning -> Diagnostic` mapping both serve paths already inline for render/xref
//! warnings. See docs/superpowers/specs/2026-07-18-dx1-live-preview-validation-design.md

use crate::check::{Scope, page_static_diagnostics};
use crate::protocol::Diagnostic;
use std::path::Path;

/// Located (file+line) when the warning carries a location, else attributed to "the
/// previewed document" (`file = None`, which the client resolves to the doc's path).
fn located(w: &taliesin_core::render::Warning) -> Diagnostic {
    let mut d = Diagnostic::warn(&w.message);
    if let Some(line) = w.line {
        d = d.at(w.file.clone(), line);
    }
    d
}

/// Static lints over an already-rendered preview doc's blocks. MUST be called on
/// **pre-execution** blocks (before the executor runs the code cells).
pub(crate) fn static_diagnostics(
    src: &str,
    blocks: &[taliesin_core::Block],
    base: &Path,
    format: taliesin_core::DocFormat,
    scope: Scope,
) -> Vec<Diagnostic> {
    page_static_diagnostics(src, blocks, base, format, scope)
        .iter()
        .map(located)
        .collect()
}
```

- [ ] **Step 3: Write the failing tests**

Append to `crates/server/src/preview_diag.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A fresh empty temp dir to use as the render base (no image files present, so a
    /// local-image reference is "missing").
    fn tmp_base(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tali-dx1-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn static_diagnostics_flag_a_missing_local_image() {
        let base = tmp_base("static-img");
        let src = "# Title\n\n![a chart](nope.png)\n";
        let doc =
            taliesin_core::render_document_with_includes_rooted(src, base.as_path(), Some(base.as_path()));
        let diags = static_diagnostics(src, &doc.blocks, base.as_path(), doc.format, Scope::Standalone);
        assert!(
            diags.iter().any(|d| d.message.contains("nope.png")),
            "expected a diagnostic naming the missing image, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn static_diagnostics_are_empty_for_a_clean_doc() {
        let base = tmp_base("static-clean");
        let src = "# Title\n\nJust a paragraph of plain prose, no links or images.\n";
        let doc =
            taliesin_core::render_document_with_includes_rooted(src, base.as_path(), Some(base.as_path()));
        let diags = static_diagnostics(src, &doc.blocks, base.as_path(), doc.format, Scope::Standalone);
        assert!(diags.is_empty(), "clean doc should lint clean, got: {diags:?}");
        let _ = std::fs::remove_dir_all(&base);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p taliesin-server preview_diag::tests -- --nocapture`
Expected: both PASS. (If `static_diagnostics_flag_a_missing_local_image` fails because the message text differs, read `crates/core/src/diagnostics/` for the missing-asset/media rule and assert on the substring it actually emits — the filename `nope.png` should appear regardless.)

- [ ] **Step 5: Mutation-check the missing-image test**

Temporarily change `Scope::Standalone` → return `Vec::new()` inside `static_diagnostics` (e.g. `let _ = page_static_diagnostics(...); return Vec::new();`), re-run the test, confirm it FAILS, then revert.

Run: `cargo test -p taliesin-server preview_diag::tests::static_diagnostics_flag_a_missing_local_image`
Expected after mutation: FAIL. After revert: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/server/src/preview_diag.rs crates/server/src/main.rs
git commit -m "feat(preview): static-diagnostics bridge for live preview (DX1)"
```

---

### Task 2: Wire single-doc `serve` to run static diagnostics

**Files:**
- Modify: `crates/server/src/serve/mod.rs` (`rebuild`, ~L1229-1258)

**Interfaces:**
- Consumes: `crate::preview_diag::static_diagnostics`, `crate::check::Scope::Standalone`.
- Produces: nothing new (behavioral wiring).

**Context:** `rebuild` already builds `diags` (kernel/include) then pushes `doc.warnings` and `validate_xrefs`. `render_doc(app)` reads the file internally but does not return `src`; `AppState` has `path: PathBuf`. Compute static diagnostics on `doc.blocks` **before** `executor.run(doc.blocks)` consumes it (partial move of only the `blocks` field; `doc.format` etc. stay usable).

- [ ] **Step 1: Insert the pre-exec computation and the extend**

In `crates/server/src/serve/mod.rs`, in `rebuild`, immediately after the `let Some(doc) = render_doc(app) else { ... };` block and **before** `let blocks = executor.run(doc.blocks).await;`, add:

```rust
    // Static lints (broken links, missing assets/media, dup ids, dangling anchors,
    // a11y, ...) on PRE-EXEC blocks, so a cell-generated figure isn't linted for alt
    // text. `src` is re-read (render_doc doesn't expose it); `base` is the doc's dir.
    let static_diags = {
        let src = std::fs::read_to_string(&app.path).unwrap_or_default();
        let base = app.path.parent().unwrap_or_else(|| Path::new("."));
        crate::preview_diag::static_diagnostics(
            &src,
            &doc.blocks,
            base,
            doc.format,
            crate::check::Scope::Standalone,
        )
    };
```

Then, immediately after the existing `let mut diags = compute_diagnostics(app, executor);` line, add:

```rust
    diags.extend(static_diags);
```

(`Path` is already imported at `serve/mod.rs:17` via `use std::path::{Path, PathBuf};`.)

- [ ] **Step 2: Verify it compiles clean**

Run: `cargo build -p taliesin-server && cargo clippy -p taliesin-server --all-targets -- -D warnings`
Expected: builds, no clippy findings. (Behavior is covered by Task 1's helper tests; the live-server wiring is verified in the browser in Task 5.)

- [ ] **Step 3: Confirm the helper tests still pass**

Run: `cargo test -p taliesin-server preview_diag::tests`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add crates/server/src/serve/mod.rs
git commit -m "feat(serve): run static diagnostics in single-doc preview (DX1)"
```

---

### Task 3: Cross-page + `_site.yml` bridge helpers + tests

**Files:**
- Modify: `crates/server/src/preview_diag.rs` (add two helpers + two tests)

**Interfaces:**
- Consumes: `taliesin_core::Site` (`validate_cross_page_links() -> Vec<(String, Warning)>`, `warnings: Vec<String>`), `taliesin_core::site::is_missing_config_warning`.
- Produces:
  - `pub(crate) fn cross_page_diagnostics(site: &taliesin_core::Site, page_rel: &str) -> Vec<crate::protocol::Diagnostic>`
  - `pub(crate) fn site_config_diagnostics(site: &taliesin_core::Site) -> Vec<crate::protocol::Diagnostic>`
  (Task 4 consumes both.)

- [ ] **Step 1: Add the two helpers**

In `crates/server/src/preview_diag.rs`, after `static_diagnostics`, add:

```rust
/// Cross-page relative-link + anchor existence for ONE page, resolved against the whole
/// site registry (the site-aware counterpart to `validate_local_links`, which `InSite`
/// omits). Runs the whole-site check (~27 ms) and keeps only this page's findings, so a
/// link broken by an edit to a *different* page refreshes when that page next rebuilds.
pub(crate) fn cross_page_diagnostics(
    site: &taliesin_core::Site,
    page_rel: &str,
) -> Vec<Diagnostic> {
    site.validate_cross_page_links()
        .into_iter()
        .filter(|(rel, _)| rel == page_rel)
        .map(|(_, w)| located(&w))
        .collect()
}

/// `_site.yml` config warnings (unknown keys / typos), attributed to the config file.
/// The missing-`_site.yml` advisory is dropped: a bare dir of `.tmd` is a valid project.
/// `protocol::Diagnostic` has no "file without line" constructor, so set `file` directly.
pub(crate) fn site_config_diagnostics(site: &taliesin_core::Site) -> Vec<Diagnostic> {
    site.warnings
        .iter()
        .filter(|m| !taliesin_core::site::is_missing_config_warning(m))
        .map(|m| {
            let mut d = Diagnostic::warn(m);
            d.file = Some("_site.yml".to_string());
            d
        })
        .collect()
}
```

- [ ] **Step 2: Write the failing tests**

In the `#[cfg(test)] mod tests` block in `crates/server/src/preview_diag.rs`, add a site-fixture helper and two tests:

```rust
    /// A temp multi-page site dir. `files` is (relative name, contents).
    fn tmp_site(tag: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tali-dx1-site-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        dir
    }

    #[test]
    fn cross_page_diagnostics_flag_a_broken_link_only_on_the_linking_page() {
        let dir = tmp_site(
            "xpage",
            &[
                ("index.tmd", "# Home\n\nSee [the other page](other.tmd#nope).\n"),
                ("other.tmd", "# Real Heading\n\nBody.\n"),
            ],
        );
        let site = taliesin_core::Site::discover(&dir);
        let index_rel = site
            .pages
            .iter()
            .find(|p| p.input.ends_with("index.tmd"))
            .expect("index page discovered")
            .rel
            .clone();
        let other_rel = site
            .pages
            .iter()
            .find(|p| p.input.ends_with("other.tmd"))
            .expect("other page discovered")
            .rel
            .clone();

        let on_index = cross_page_diagnostics(&site, &index_rel);
        assert!(
            !on_index.is_empty(),
            "index links a nonexistent anchor; expected a diagnostic, got none"
        );
        let on_other = cross_page_diagnostics(&site, &other_rel);
        assert!(
            on_other.is_empty(),
            "other.tmd has no broken outgoing link; expected none, got: {on_other:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn site_config_diagnostics_surface_an_unknown_key_attributed_to_site_yml() {
        let dir = tmp_site(
            "cfg",
            &[
                ("_site.yml", "title: My Site\ntitel: oops\n"),
                ("index.tmd", "# Home\n"),
            ],
        );
        let site = taliesin_core::Site::discover(&dir);
        // Precondition: discover must have typo-warned on the unknown `titel` key.
        assert!(
            !site.warnings.is_empty(),
            "fixture precondition: an unknown _site.yml key should warn; if not, use the \
             exact unknown-key form the config linter recognizes (site/config/mod.rs)"
        );
        let diags = site_config_diagnostics(&site);
        assert!(!diags.is_empty(), "expected the config warning surfaced as a diagnostic");
        assert!(
            diags.iter().all(|d| d.file.as_deref() == Some("_site.yml")),
            "config diagnostics must be attributed to _site.yml"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p taliesin-server preview_diag::tests -- --nocapture`
Expected: all four PASS.

- [ ] **Step 4: Mutation-check the cross-page test**

Temporarily change the filter in `cross_page_diagnostics` from `.filter(|(rel, _)| rel == page_rel)` to `.filter(|_| false)`, re-run, confirm `cross_page_diagnostics_flag_a_broken_link_only_on_the_linking_page` FAILS, then revert.

Run: `cargo test -p taliesin-server preview_diag::tests::cross_page_diagnostics_flag_a_broken_link_only_on_the_linking_page`
Expected after mutation: FAIL. After revert: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/server/src/preview_diag.rs
git commit -m "feat(preview): cross-page + _site.yml diagnostics bridge (DX1)"
```

---

### Task 4: Wire site `serve_site::build_page`

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs` (`build_page`, ~L895-951)

**Interfaces:**
- Consumes: `crate::preview_diag::{static_diagnostics, cross_page_diagnostics, site_config_diagnostics}`, `crate::check::Scope::InSite`.
- Produces: nothing new (behavioral wiring).

**Context:** In `build_page(app: &SiteApp, rel: &str, pool: &mut ExecPool)`, `src`/`base`/`doc`/`page` are local; `app.site` is `Mutex<Site>`; `page.rel` includes `.tmd`. The `finish_blocks` site-lock scope (~L935-943) is released before the diagnostics assembly at `let mut diags = page_diagnostics(&page.input, exec);` (~L944), so a new short lock scope there does not nest.

- [ ] **Step 1: Compute static diagnostics pre-exec**

In `build_page`, immediately **before** the line `doc.blocks = exec.run(std::mem::take(&mut doc.blocks)).await;`, add:

```rust
    // Static lints on PRE-EXEC blocks (InSite omits validate_local_links; the site-aware
    // cross-page check below covers those). Collected now, pushed after `diags` is built.
    let static_diags = crate::preview_diag::static_diagnostics(
        &src,
        &doc.blocks,
        &base,
        doc.format,
        crate::check::Scope::InSite,
    );
```

- [ ] **Step 2: Extend the diagnostics with static + cross-page + config**

Immediately after the existing `let mut diags = page_diagnostics(&page.input, exec);` line, add:

```rust
    diags.extend(static_diags);
    // Cross-page links (this page only) + `_site.yml` config warnings. `validate_cross_page_links`
    // re-renders the whole site (~27 ms), so scope the site lock tightly.
    {
        let site = app.site.lock();
        diags.extend(crate::preview_diag::cross_page_diagnostics(&site, rel));
        diags.extend(crate::preview_diag::site_config_diagnostics(&site));
    }
```

- [ ] **Step 3: Verify it compiles clean**

Run: `cargo build -p taliesin-server && cargo clippy -p taliesin-server --all-targets -- -D warnings`
Expected: builds, no clippy findings.

- [ ] **Step 4: Confirm the full unit suite still passes**

Run: `cargo test -p taliesin-server preview_diag::tests`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/server/src/serve_site/mod.rs
git commit -m "feat(serve_site): static + cross-page + _site.yml diagnostics in site preview (DX1)"
```

---

### Task 5: Full verification (suite + browser + fmt/clippy)

**Files:** none (verification only).

- [ ] **Step 1: Full workspace test + lint**

Run:
```bash
cargo test -p taliesin-core -p taliesin-server
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```
Expected: all green; `fmt --check` silent; clippy clean.

- [ ] **Step 2: Browser check — single-doc preview lights the badge**

Build the binary (`assets`/logic changed): `cargo build -p taliesin-server`. Copy a clean corpus doc to the scratchpad, inject a wrong image filename + a dangling `@fig-` ref (do NOT edit a committed corpus file — a broken corpus doc fails the regression net). Serve it and drive chrome-devtools MCP:

```bash
cp corpus/tech-blog/posts/<a-post>.tmd \
  /tmp/claude-1000/-home-bogo-Documents-personal-taliesin/08ebf233-740c-4daf-8b20-8f60fb3d9099/scratchpad/dx1-check.tmd
# edit the scratch copy: point an image at a missing file + add a dangling @fig- ref
cargo run -p taliesin-server -- preview <scratch>/dx1-check.tmd 4388
```
Verify: the collapsed `◇</>` button shows the amber count (badge lit); opening the dev menu lists the missing-image + dangling-ref rows; a located row is click-to-source. Screenshot + console via chrome-devtools MCP. (Recall from CLAUDE.md that `pkill -f 'taliesin preview'` kills the Bash tool's own shell — stop the server another way.)

- [ ] **Step 3: Browser check — site preview + cross-page**

Copy a small multi-page corpus site to the scratchpad, break one cross-page link (wrong slug or `#anchor`) and add an unknown `_site.yml` key:

```bash
cp -r corpus/<a-small-site> <scratch>/dx1-site   # or reuse corpus/embed
# break an intra-site link + add an unknown _site.yml key
cargo run -p taliesin-server -- preview <scratch>/dx1-site 4388
```
Verify on the broken page: badge lit, dev-menu lists the broken cross-page link + the `_site.yml` warning (attributed to `_site.yml`); the cross-page row is click-to-source. Confirm a *clean* sibling page shows no phantom cross-page warning. Screenshot + console.

- [ ] **Step 4: Final commit (if any verification tweaks were needed)**

```bash
cargo fmt
git add -A
git commit -m "test(preview): DX1 verification adjustments" || echo "nothing to commit"
```

---

## Notes for the implementer

- **Do not** add a new corpus doc for the positive case. The regression net asserts every `corpus/` doc renders clean; a broken doc would fail it. The positive pin is `static_diagnostics_are_empty_for_a_clean_doc`.
- **Do not** reuse `check::diag_from` or `check::Diagnostic::new` — those build `check::Diagnostic` (CLI-JSON: `code`/`severity`/`suggestion`). The serve paths push `protocol::Diagnostic`. The `located` converter is the correct, serve-consistent mapping.
- If `render_document_with_includes_rooted`'s arg types don't accept `&PathBuf`, pass `base.as_path()`.
- The incoming-cross-page-link staleness (breaking page B's link by editing page A shows on B only when B next rebuilds) is a **deliberate, owner-approved** limitation, not a bug; `build`/`check`/`publish` remain the backstop.
- After landing: delete DX1 from `notes/backlog.md` §6, note the scope-collapse (badge already existed; single-doc serve already had xrefs) in the closure record, and update `notes/AUDITS.md` if the DX-audit index tracks per-item status.
