# Editor Scope Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish Taliesin's VS Code / LSP surface by building ideas 72, 75, 76, 77, 78, 79, 80 and 85, on one shared project-walk substrate, so no editor-surface idea remains open except 86 (filed as backlog 175(d), blocked on output streaming).

**Architecture:** One new Rust module, `crates/server/src/lsp_project.rs`, walks the enclosing `_site.yml` project and caches the result behind a stat-validated memo (compare `(path, mtime, len)` per page; re-walk on mismatch). Every project-wide surface reads that one snapshot, so cross-file definition, workspace symbols, the sidebar and the References view cannot disagree with each other. The TypeScript side adds only what LSP has no concept of: TreeViews, a task provider, a file-decoration provider, a status bar item and the language-model registrations.

**Tech Stack:** Rust (edition 2024, `lsp-server` + `lsp-types`), TypeScript (`vscode-languageclient` 10.x, esbuild), `node --test` and mocha for TS unit tests, `@vscode/test-electron` for e2e.

**Spec:** [2026-07-30-editor-scope-completion-design.md](../specs/2026-07-30-editor-scope-completion-design.md). Read it before Task 1; it records why ideas 67, 74 and 83 are cut and must not be rebuilt.

## Global Constraints

- **Single editing surface.** Every surface here is read-only navigation or a diagnostic. Nothing in this plan may write to a `.tmd` except through an author gesture that already exists. No drag-to-reorder in the sidebar, ever.
- **Editor intelligence lives in Rust.** If it can be an LSP request, it is one. TypeScript holds only TreeViews, tasks, decorations, the status bar, and LM registrations.
- **Do-NOT-touch:** `MAX_WARM_PAGES` and the deterministic LRU order in `crates/server/src/serve_site/exec_pool.rs`. Nothing in this plan goes near it.
- **`taliesin lsp` stdout is the JSON-RPC wire.** Never `println!` from LSP code. Use `crate::log` (stderr).
- **Engine floor:** `engines.vscode` is `^1.97.0` and `@types/vscode` is pinned **exactly** `1.97.0` (no caret). Task 13 moves both together to `^1.101.0` / `1.101.0`. A caret on the types resolves to the latest and lets `tsc` bless APIs the minimum engine lacks; that is the bug the exact pin exists to prevent.
- **Measured API floors** (do not re-derive, do not guess): `registerMcpServerDefinitionProvider` is **absent** from `@types/vscode@1.100.0` and **present** in `1.101.0`. `LanguageModelTool` is present at `1.97.0`.
- **No em dashes or en dashes** in prose added by this plan (docs, comments, commit messages). Use commas, colons or parentheses.
- **A `PostToolUse` hook runs `rustfmt`** on every edited `.rs`, so the tree stays `cargo fmt` clean automatically.
- **Verify every fix by mutation**, not by a green suite: restore the bug, watch the named test fail, restore the fix. Revert a mutation by **inverse edit**, never `git checkout` (that restores from HEAD and deletes uncommitted implementation).
- **`cargo test` aborts remaining binaries at the first failure**, so a total is not trustworthy until a clean re-run. Grep for `FAILED`, never for the absence of a string.
- **Bound memory:** heavy cargo runs are charged to the VS Code snap scope and have killed the desktop. Use `CARGO_BUILD_JOBS=4`.

## File Structure

**Rust, `crates/core`:**
- Modify `src/site/xref.rs`: make the anchor scanner and its result type reachable from outside the module; delete the duplicate `enclosing_site_root`.
- Modify `src/site/discovery.rs:128`: `collect_pages` from `pub(super)` to `pub`.
- Modify `src/site/mod.rs`: re-export the scanner.

**Rust, `crates/server`:**
- Create `src/lsp_project.rs`: the walk, its types, the stat-validated memo. One responsibility: "what does the enclosing project contain".
- Modify `src/lsp_nav.rs`: add `xref_occurrences` beside `anchor_occurrences`.
- Modify `src/lsp.rs`: capabilities, dispatch, cross-file definition and hover, two custom requests.
- Modify `src/main.rs`: declare the new module.

**TypeScript, `editor/vscode/src`:**
- Create `sidebar.ts` (idea 77), `tasks.ts` (80), `decorations.ts` (78), `statusbar.ts` (79), `lmtools.ts` (85).
- Modify `extension.ts` (registration only), `client.ts` (document selector), `package.json`, `src/test/manifest.test.ts`.

**Docs:**
- `docs/internals/extending.tmd`: a table row per new capability (a drift gate fails without it).
- `docs/guide/using/preview.tmd`: what an author sees.

---

### Task 1: `xref_occurrences`, the scan-all sibling

**Files:**
- Modify: `crates/server/src/lsp_nav.rs` (add after `anchor_occurrences`, which ends at `:563`)
- Test: `crates/server/src/lsp_nav.rs` (the inline `mod tests`, beside `anchor_occurrences_covers_definition_and_references_only` at `:1027`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pub(crate) fn xref_occurrences(text: &str) -> Vec<(String, u32, u32)>` returning `(id, line, col)` in scalar (not UTF-16) offsets, one entry per `@`-sigil reference whose id has a known cross-reference prefix.

**Why this exists:** `anchor_occurrences(text, id)` searches for **one known id**, so it can answer "what links to this target" but never "what points at nothing". The References view has to group dangling references, whose ids are by definition absent from the anchor set.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `crates/server/src/lsp_nav.rs`:

```rust
    #[test]
    fn xref_occurrences_finds_every_reference_and_skips_definitions_and_citations() {
        // A definition, two references, a bare-fragment link, a citation, and an email-like
        // `@` that is not a reference site.
        let text = "![p](i.png){#fig-scree}\n\nSee @fig-scree and @sec-intro.\n\n\
                    [back](#fig-scree), cite [@knuth1984], mail a@b-c.com\n";
        let hits = xref_occurrences(text);
        assert_eq!(
            hits,
            vec![
                ("fig-scree".to_string(), 2, 4),
                ("sec-intro".to_string(), 2, 19),
            ],
            "only the two `@`-sigil references, not the `{{#fig-scree}}` definition, not the \
             `](#fig-scree)` link, not the citation, not the address"
        );
    }

    #[test]
    fn xref_occurrences_reports_a_dangling_reference_anchor_occurrences_cannot_find() {
        // The whole reason this function exists: `fig-missing` is defined nowhere, so no
        // caller could have known to ask `anchor_occurrences` for it.
        let text = "See @fig-missing.\n";
        assert_eq!(
            xref_occurrences(text),
            vec![("fig-missing".to_string(), 0, 5)]
        );
    }

    #[test]
    fn xref_occurrences_agrees_with_anchor_occurrences_on_every_id_it_reports() {
        // Containment, NOT equality: `anchor_occurrences` also matches the `{#id}` definition
        // and the `](#id)` bare fragment, neither of which is a *use*. Written as an equality
        // this fails on the first document that references an anchor it also defines, and a
        // test relaxed on first contact teaches nothing.
        let text = "# Scree {#fig-scree}\n\nSee @fig-scree, again @fig-scree.\n\n\
                    [x](#fig-scree) and [@fig-scree]\n";
        for (id, line, col) in xref_occurrences(text) {
            let known = anchor_occurrences(text, &id);
            assert!(
                known.iter().any(|&(l, c, _)| (l, c) == (line, col)),
                "xref_occurrences reported {id} at {line}:{col}, which anchor_occurrences \
                 does not see: the two scanners disagree about what a reference is"
            );
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin xref_occurrences
```

Expected: FAIL to compile, `cannot find function 'xref_occurrences' in this scope`.

- [ ] **Step 3: Write the implementation**

Insert immediately after `anchor_occurrences` in `crates/server/src/lsp_nav.rs`:

```rust
/// Every `@`-sigil cross-reference **use** in the document, as `(id, line, col)` in scalar
/// offsets.
///
/// The scan-all sibling of [`anchor_occurrences`], which searches for one *known* id and so
/// structurally cannot see a **dangling** reference: a reference whose target is defined
/// nowhere has an id no caller could have thought to ask for, and grouping those is the whole
/// job of the sidebar's References view.
///
/// It shares [`is_anchor_site`] and [`is_xref_id_char`] with `anchor_occurrences` rather than
/// re-deciding what a reference looks like. A second scanner free to disagree with rename
/// about what an anchor is, is the trap document highlight avoided by reusing this one.
/// Definitions (`{#id}`) and bare-fragment links (`](#id)`) are **not** uses and are excluded
/// here even though `anchor_occurrences` matches them, so the relation between the two is
/// containment, not equality.
pub(crate) fn xref_occurrences(text: &str) -> Vec<(String, u32, u32)> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = Vec::new();
    let mut i = 1; // an id at offset 0 has no room for the `@` before it
    while i < n {
        // `is_anchor_site` accepts `@`, `#` and `label:` sigils; a *use* is the `@` one only.
        if chars[i - 1] == '@' && is_anchor_site(&chars, i) {
            let mut j = i;
            while j < n && is_xref_id_char(chars[j]) {
                j += 1;
            }
            if j > i {
                let id: String = chars[i..j].iter().collect();
                // An `@word` with no cross-reference kind prefix is prose, not a reference.
                if taliesin_core::cite::is_xref_anchor(&id) {
                    let (line, col) = offset_to_line_col(&chars, i);
                    out.push((id, line, col));
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin xref_occurrences
```

Expected: PASS, 3 tests.

- [ ] **Step 5: Verify by mutation**

Change `if taliesin_core::cite::is_xref_anchor(&id) {` to `if true {`. Re-run. Expected: `xref_occurrences_finds_every_reference_and_skips_definitions_and_citations` FAILS (the `a@b-c.com` address is now reported). Restore by inverse edit.

Then change `chars[i - 1] == '@' && is_anchor_site(&chars, i)` to `is_anchor_site(&chars, i)`. Re-run. Expected: the same test FAILS (the `{#fig-scree}` definition is now reported as a use). Restore by inverse edit.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/lsp_nav.rs
git commit -m "feat(lsp): xref_occurrences, the scan-all sibling that can see a dangling ref"
```

---

### Task 2: Open the core walk, and delete the duplicated project-root finder

**Files:**
- Modify: `crates/core/src/site/xref.rs` (`ScannedAnchor` at `:227`, `scan_page_anchors` at `:240`, the private `enclosing_site_root` at `:139`)
- Modify: `crates/core/src/site/discovery.rs:128` (`collect_pages`)
- Modify: `crates/core/src/site/mod.rs` (re-exports, near the existing `pub use book::{Book, BookEntry};` at `:209`)
- Test: `crates/core/src/site/xref.rs` (inline tests)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces, all reachable as `taliesin_core::site::…`:
  - `pub struct ScannedAnchor { pub id: String, pub number: String, pub title: String, pub line: usize }`
  - `pub fn scan_page_anchors(src: &str, chapter: Option<u32>) -> Vec<ScannedAnchor>`
  - `pub fn collect_pages(dir: &Path, out: &mut Vec<PathBuf>)`
  - `pub fn enclosing_site_root(start: &Path) -> Option<PathBuf>` (already public at `site/mod.rs:261`; this task removes its private twin)

**The cleanup, and its stop condition:** `enclosing_site_root` exists twice, `site/mod.rs:261` (public) and `site/xref.rs:139` (private). `lsp_project` needs project-root discovery and would be the third caller against a duplicated definition. **Before deleting either, prove they agree.** If they do not, stop and report which behaviour is correct; a silent merge of two subtly different root-finders is worse than the duplication.

- [ ] **Step 1: Write the failing agreement test**

Add to the `mod tests` block in `crates/core/src/site/xref.rs`:

```rust
    #[test]
    fn the_two_enclosing_site_root_implementations_agree_before_one_is_deleted() {
        // Guards the consolidation in this task. Both copies must answer identically for:
        // a page directly in the root, a page in a subdirectory, a page with no `_site.yml`
        // anywhere above it, and the root directory itself passed as the start.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::create_dir_all(root.join("part")).unwrap();
        std::fs::write(root.join("index.tmd"), "# i\n").unwrap();
        std::fs::write(root.join("part/ch.tmd"), "# c\n").unwrap();

        let orphan = tempfile::tempdir().unwrap();
        std::fs::write(orphan.path().join("solo.tmd"), "# s\n").unwrap();

        for probe in [
            root.join("index.tmd"),
            root.join("part/ch.tmd"),
            root.to_path_buf(),
            orphan.path().join("solo.tmd"),
        ] {
            assert_eq!(
                enclosing_site_root(&probe),
                super::enclosing_site_root(&probe),
                "the xref.rs and mod.rs copies disagree for {}",
                probe.display()
            );
        }
    }
```

Note: inside `xref.rs`, the bare name resolves to the private local copy and `super::enclosing_site_root` to the `mod.rs` one. If `tempfile` is not already a dev-dependency of `taliesin-core`, add it to `[dev-dependencies]` from the workspace table.

- [ ] **Step 2: Run it**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-core the_two_enclosing_site_root
```

Expected: PASS. **If it FAILS, stop this task and report the divergence.** That is a finding, not an obstacle.

- [ ] **Step 3: Delete the duplicate and widen the three items**

In `crates/core/src/site/xref.rs`:
1. Delete the private `fn enclosing_site_root` at `:139` and the agreement test from Step 1 (it has served its purpose and cannot compile once one side is gone). Point the call inside `anchors_defined_elsewhere_in_project` at `super::enclosing_site_root`.
2. Change `struct ScannedAnchor` to `pub struct ScannedAnchor` and each of its four fields to `pub`.
3. Change `fn scan_page_anchors` to `pub fn scan_page_anchors`.

In `crates/core/src/site/discovery.rs:128`, change `pub(super) fn collect_pages` to `pub fn collect_pages`.

In `crates/core/src/site/mod.rs`, beside the existing re-exports:

```rust
pub use discovery::collect_pages;
pub use xref::{ScannedAnchor, scan_page_anchors};
```

Add doc comments to the two newly public items explaining that they are the substrate the editor's project walk reads, so a later reader knows why they are public.

- [ ] **Step 4: Verify the crate still builds and nothing regressed**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-core 2>&1 | tail -30
CARGO_BUILD_JOBS=4 cargo clippy -p taliesin-core --all-targets -- -D warnings
```

Expected: no `FAILED` lines, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/site/
git commit -m "refactor(core): expose the anchor scanner, fold the duplicated enclosing_site_root"
```

---

### Task 3: `lsp_project.rs`, the stat-validated project walk

**Files:**
- Create: `crates/server/src/lsp_project.rs`
- Modify: `crates/server/src/main.rs` (add `mod lsp_project;` beside the other `mod lsp_*;` lines)
- Test: `crates/server/src/lsp_project.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `taliesin_core::site::{collect_pages, enclosing_site_root, scan_page_anchors, ScannedAnchor}` (Task 2); `crate::lsp_nav::xref_occurrences` (Task 1); `crate::lsp_outline::sections`.
- Produces:

```rust
pub(crate) struct ProjectAnchor { pub id: String, pub path: PathBuf, pub line: u32, pub title: String, pub number: String }
pub(crate) struct ProjectHeading { pub path: PathBuf, pub line: u32, pub level: u8, pub text: String }
pub(crate) struct ProjectUse { pub id: String, pub path: PathBuf, pub line: u32, pub col: u32 }
pub(crate) struct ProjectScan { pub root: PathBuf, pub anchors: Vec<ProjectAnchor>, pub headings: Vec<ProjectHeading>, pub uses: Vec<ProjectUse> }
pub(crate) struct ProjectCache { /* private */ }
impl ProjectCache {
    pub(crate) fn new() -> Self;
    pub(crate) fn get(&mut self, page: &Path) -> Option<&ProjectScan>;
}
```

`get` returns `None` for a document with no enclosing `_site.yml`. That is the standalone-document case: every consumer must degrade to today's document-local behaviour, silently.

**Why a memo and not an index:** every consumer fires on a user gesture (F12, Ctrl+T, opening a view, an Explorer refresh), never per keystroke, so this needs a walk. Validation is a `stat` per page, orders of magnitude below the read-plus-parse it guards. Correctness degrades to "re-walks more often than needed", never to "serves stale data". This is what replaces idea 74's file watcher.

- [ ] **Step 1: Write the failing tests**

Create `crates/server/src/lsp_project.rs` containing only the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A two-page project: `index.tmd` defines `fig-one` and references `sec-two`;
    /// `ch/two.tmd` defines `sec-two` and references `fig-one` and a dangling `fig-gone`.
    fn fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::create_dir_all(root.join("ch")).unwrap();
        std::fs::write(
            root.join("index.tmd"),
            "# Index\n\n![p](i.png){#fig-one}\n\nSee @sec-two.\n",
        )
        .unwrap();
        std::fs::write(
            root.join("ch/two.tmd"),
            "# Two {#sec-two}\n\n## Deeper\n\nSee @fig-one and @fig-gone.\n",
        )
        .unwrap();
        tmp
    }

    #[test]
    fn a_document_outside_any_site_project_scans_to_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let solo = tmp.path().join("solo.tmd");
        std::fs::write(&solo, "# Solo\n").unwrap();
        let mut cache = ProjectCache::new();
        assert!(
            cache.get(&solo).is_none(),
            "a standalone document has no project; every consumer must fall back silently"
        );
    }

    #[test]
    fn the_walk_collects_anchors_headings_and_uses_across_every_page() {
        let tmp = fixture();
        let mut cache = ProjectCache::new();
        let scan = cache.get(&tmp.path().join("index.tmd")).unwrap();

        let mut ids: Vec<&str> = scan.anchors.iter().map(|a| a.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["fig-one", "sec-two"], "anchors from BOTH pages");

        assert!(
            scan.headings.iter().any(|h| h.text == "Deeper" && h.level == 2),
            "headings come from every page, not just the one asked about"
        );

        let mut used: Vec<&str> = scan.uses.iter().map(|u| u.id.as_str()).collect();
        used.sort_unstable();
        assert_eq!(used, vec!["fig-gone", "fig-one", "sec-two"]);
        assert!(
            scan.uses.iter().any(|u| u.id == "fig-gone"),
            "a dangling use is collected, not dropped: the References view groups them"
        );
    }

    #[test]
    fn an_anchor_carries_the_page_and_line_that_define_it() {
        let tmp = fixture();
        let mut cache = ProjectCache::new();
        let scan = cache.get(&tmp.path().join("index.tmd")).unwrap();
        let a = scan.anchors.iter().find(|a| a.id == "sec-two").unwrap();
        assert!(a.path.ends_with("ch/two.tmd"));
        assert_eq!(a.line, 0, "0-based line of the defining heading");
        assert_eq!(a.title, "Two");
    }

    #[test]
    fn the_memo_re_walks_when_a_page_changes_on_disk() {
        // The one test that must not be vacuous: this memo is what the design chose INSTEAD
        // of a file watcher, so its invalidation is the whole risk.
        let tmp = fixture();
        let probe = tmp.path().join("index.tmd");
        let mut cache = ProjectCache::new();
        assert!(!cache.get(&probe).unwrap().anchors.iter().any(|a| a.id == "fig-late"));

        // Rewrite a page with a longer body, so both mtime and length differ.
        std::fs::write(
            tmp.path().join("ch/two.tmd"),
            "# Two {#sec-two}\n\n## Deeper\n\nSee @fig-one.\n\n![q](q.png){#fig-late}\n",
        )
        .unwrap();

        assert!(
            cache.get(&probe).unwrap().anchors.iter().any(|a| a.id == "fig-late"),
            "the memo served a stale scan after a page changed on disk"
        );
    }

    #[test]
    fn an_unchanged_project_is_not_re_walked() {
        // The other half: if this never hits, the memo is decoration and every gesture pays
        // a full walk.
        let tmp = fixture();
        let probe = tmp.path().join("index.tmd");
        let mut cache = ProjectCache::new();
        cache.get(&probe).unwrap();
        let first = cache.walks();
        cache.get(&probe).unwrap();
        assert_eq!(cache.walks(), first, "a second get on an unchanged project re-walked");
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Add `mod lsp_project;` to `crates/server/src/main.rs`, then:

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin lsp_project
```

Expected: FAIL to compile, `cannot find type 'ProjectCache'`.

- [ ] **Step 3: Write the implementation**

Write the module body above the test block in `crates/server/src/lsp_project.rs`:

```rust
//! What the enclosing `_site.yml` project contains, for the editor surfaces that reach past
//! the open buffer: cross-file go-to-definition, workspace symbols, and the sidebar's outline
//! and references views.
//!
//! **Why a walk behind a memo, and not an index.** Every consumer fires on a *user gesture*
//! (F12, Ctrl+T, opening a view, the Explorer asking for a decoration), never per keystroke,
//! so none of them needs a live index. An index would put file watching, invalidation and
//! background state into a component whose statelessness is why it is reliable. Instead the
//! walk is cached and validated by `stat`ing every page and comparing `(mtime, len)`: a stat
//! is orders of magnitude cheaper than the read-plus-parse it guards, and the failure mode is
//! "re-walked when it need not have", never "served stale data".
//!
//! The per-keystroke diagnostic path does NOT use this. It keeps calling
//! `site::anchors_defined_elsewhere_in_project` behind the existing coalescing window.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A cross-reference target defined somewhere in the project.
pub(crate) struct ProjectAnchor {
    pub id: String,
    pub path: PathBuf,
    /// 0-based line of the defining site.
    pub line: u32,
    /// The heading's text when the anchor sits on one; empty otherwise.
    pub title: String,
    /// The rendered section number for a numbered chapter heading; empty otherwise.
    pub number: String,
}

/// One heading on one page, for workspace symbols and the whole-book outline.
pub(crate) struct ProjectHeading {
    pub path: PathBuf,
    /// 0-based line of the heading.
    pub line: u32,
    pub level: u8,
    pub text: String,
}

/// One `@`-sigil reference, resolved or not. A use whose `id` matches no [`ProjectAnchor`] is
/// dangling, which is exactly what the References view groups.
pub(crate) struct ProjectUse {
    pub id: String,
    pub path: PathBuf,
    /// 0-based line and scalar column.
    pub line: u32,
    pub col: u32,
}

/// One walk's result.
pub(crate) struct ProjectScan {
    pub root: PathBuf,
    pub anchors: Vec<ProjectAnchor>,
    pub headings: Vec<ProjectHeading>,
    pub uses: Vec<ProjectUse>,
}

/// What a page looked like when it was last walked: enough to notice an edit without
/// watching the filesystem.
type Stamp = (PathBuf, Option<std::time::SystemTime>, u64);

/// The single-entry, stat-validated memo described in the module docs.
pub(crate) struct ProjectCache {
    entry: Option<(ProjectScan, Vec<Stamp>)>,
    /// How many real walks have happened. Test-visible so the memo cannot be decoration.
    walks: usize,
}

impl ProjectCache {
    pub(crate) fn new() -> Self {
        Self { entry: None, walks: 0 }
    }

    /// Walks completed. The `an_unchanged_project_is_not_re_walked` pin reads this; without
    /// it a memo that always missed would still pass every correctness test.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn walks(&self) -> usize {
        self.walks
    }

    /// The scan for `page`'s enclosing project, or `None` when it has no `_site.yml` above it.
    pub(crate) fn get(&mut self, page: &Path) -> Option<&ProjectScan> {
        let root = taliesin_core::site::enclosing_site_root(page)?;
        let stamps = stamps_for(&root);
        let fresh = match &self.entry {
            Some((scan, seen)) => scan.root == root && *seen == stamps,
            None => false,
        };
        if !fresh {
            self.entry = Some((walk(&root), stamps));
            self.walks += 1;
        }
        self.entry.as_ref().map(|(scan, _)| scan)
    }
}

/// `(path, mtime, len)` for every page, in `collect_pages` order so two runs compare equal.
fn stamps_for(root: &Path) -> Vec<Stamp> {
    let mut inputs = Vec::new();
    taliesin_core::site::collect_pages(root, &mut inputs);
    inputs
        .into_iter()
        .map(|p| {
            let meta = std::fs::metadata(&p).ok();
            let mtime = meta.as_ref().and_then(|m| m.modified().ok());
            let len = meta.map(|m| m.len()).unwrap_or(0);
            (p, mtime, len)
        })
        .collect()
}

/// Read every page once and collect all three views of it. Includes are resolved first, so
/// an anchor authored in an `_includes/` partial belongs to whichever page includes it,
/// exactly as the render pipeline and `anchors_defined_elsewhere_in_project` both do.
fn walk(root: &Path) -> ProjectScan {
    let mut inputs = Vec::new();
    taliesin_core::site::collect_pages(root, &mut inputs);
    let mut anchors = Vec::new();
    let mut headings = Vec::new();
    let mut uses = Vec::new();
    let mut seen: HashMap<String, ()> = HashMap::new();

    for input in inputs {
        let Ok(raw) = std::fs::read_to_string(&input) else {
            continue;
        };
        let base = input.parent().unwrap_or_else(|| Path::new("."));
        let (src, _) = taliesin_core::includes::resolve(&raw, base);

        for a in taliesin_core::site::scan_page_anchors(&src, None) {
            // First definition wins project-wide, matching `scan_xref_targets`. Two owners of
            // "which page defines `fig-x`" that disagreed would send F12 somewhere the built
            // page does not link to.
            if seen.insert(a.id.clone(), ()).is_none() {
                anchors.push(ProjectAnchor {
                    id: a.id,
                    path: input.clone(),
                    line: a.line.saturating_sub(1) as u32, // scan_page_anchors is 1-based
                    title: a.title,
                    number: a.number,
                });
            }
        }
        for s in crate::lsp_outline::sections(&src) {
            headings.push(ProjectHeading {
                path: input.clone(),
                line: s.start_line as u32,
                level: s.level,
                text: s.title,
            });
        }
        for (id, line, col) in crate::lsp_nav::xref_occurrences(&src) {
            uses.push(ProjectUse { id, path: input.clone(), line, col });
        }
    }
    ProjectScan { root: root.to_path_buf(), anchors, headings, uses }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin lsp_project
```

Expected: PASS, 5 tests. If `a.line` is off by one, check `scan_page_anchors`'s doc comment: it documents a **1-based** line, and `an_anchor_carries_the_page_and_line_that_define_it` pins the 0-based conversion.

- [ ] **Step 5: Verify by mutation**

Replace the body of `stamps_for` with `Vec::new()`. Re-run. Expected: `the_memo_re_walks_when_a_page_changes_on_disk` FAILS (every stamp list is now equal, so nothing ever invalidates). Restore by inverse edit.

Then change `if !fresh {` to `if true {`. Re-run. Expected: `an_unchanged_project_is_not_re_walked` FAILS. Restore by inverse edit.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/lsp_project.rs crates/server/src/main.rs
git commit -m "feat(lsp): a stat-validated project walk (replaces the parked project-index idea)"
```

---

### Task 4: Idea 75, cross-file go-to-definition and hover

**Files:**
- Modify: `crates/server/src/lsp.rs` (`resolve_definition` at `:785`, its `Target::Xref` arm at `:829`; `resolve_hover` at `:892`, its `Target::Xref` arm)
- Test: `crates/server/src/lsp.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `crate::lsp_project::{ProjectCache, ProjectScan}` (Task 3).
- Produces: nothing new for later tasks. `resolve_definition` and `resolve_hover` each gain a `&mut ProjectCache` parameter; the cache is owned by the request loop beside the existing `memo: &mut crate::lsp_memo::RenderMemo`.

**The gap this closes** is documented at `lsp.rs:695` ("an undefined xref, a cross-file ref, a missing include/bib") and at `:829` ("cross-file refs get nothing").

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/server/src/lsp.rs`. Follow the existing temp-project idiom used by `// Write a doc + its .bib to a temp dir so the server can resolve across files.` at `:2956`.

```rust
    #[test]
    fn go_to_definition_resolves_an_xref_defined_on_another_page() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::write(root.join("two.tmd"), "# Two {#sec-two}\n").unwrap();
        let here = root.join("one.tmd");
        let text = "# One\n\nSee @sec-two.\n";
        std::fs::write(&here, text).unwrap();

        let uri = lsp_types::Url::from_file_path(&here).unwrap();
        let mut docs = std::collections::HashMap::new();
        docs.insert(uri.clone(), text.to_string());
        let mut cache = crate::lsp_project::ProjectCache::new();

        // Cursor inside `sec-two` on line 2.
        let params = goto_params(&uri, 2, 6);
        let found = resolve_definition(&docs, &mut cache, &params)
            .expect("a cross-page xref must resolve; this is the lsp.rs:829 gap");
        let lsp_types::GotoDefinitionResponse::Scalar(loc) = found else {
            panic!("expected a single location");
        };
        assert!(loc.uri.to_file_path().unwrap().ends_with("two.tmd"));
        assert_eq!(loc.range.start.line, 0);
    }

    #[test]
    fn a_local_definition_still_wins_over_the_project_walk() {
        // The project scan must never override the buffer: an unsaved edit is ahead of disk,
        // and jumping to the on-disk copy of the page you are typing in is worse than useless.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::write(root.join("other.tmd"), "# Other {#sec-x}\n").unwrap();
        let here = root.join("one.tmd");
        let text = "# Local {#sec-x}\n\nSee @sec-x.\n";
        std::fs::write(&here, text).unwrap();

        let uri = lsp_types::Url::from_file_path(&here).unwrap();
        let mut docs = std::collections::HashMap::new();
        docs.insert(uri.clone(), text.to_string());
        let mut cache = crate::lsp_project::ProjectCache::new();

        let params = goto_params(&uri, 2, 6);
        let lsp_types::GotoDefinitionResponse::Scalar(loc) =
            resolve_definition(&docs, &mut cache, &params).unwrap()
        else {
            panic!("expected a single location");
        };
        assert_eq!(loc.uri, uri, "the buffer's own definition must win");
    }

    #[test]
    fn an_xref_defined_nowhere_still_resolves_to_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("_site.yml"), "title: t\n").unwrap();
        let here = tmp.path().join("one.tmd");
        let text = "See @sec-nowhere.\n";
        std::fs::write(&here, text).unwrap();
        let uri = lsp_types::Url::from_file_path(&here).unwrap();
        let mut docs = std::collections::HashMap::new();
        docs.insert(uri.clone(), text.to_string());
        let mut cache = crate::lsp_project::ProjectCache::new();
        assert!(resolve_definition(&docs, &mut cache, &goto_params(&uri, 0, 6)).is_none());
    }

    #[test]
    fn hover_on_a_cross_page_xref_names_the_page_that_defines_it() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::write(root.join("two.tmd"), "# Two {#sec-two}\n").unwrap();
        let here = root.join("one.tmd");
        let text = "See @sec-two.\n";
        std::fs::write(&here, text).unwrap();
        let uri = lsp_types::Url::from_file_path(&here).unwrap();
        let mut docs = std::collections::HashMap::new();
        docs.insert(uri.clone(), text.to_string());
        let mut cache = crate::lsp_project::ProjectCache::new();

        let hover = resolve_hover(&docs, &mut cache, &hover_params(&uri, 0, 6)).unwrap();
        let lsp_types::HoverContents::Markup(m) = hover.contents else {
            panic!("expected markup");
        };
        assert!(m.value.contains("two.tmd"), "got {:?}", m.value);
    }
```

Add the two small helpers beside them if the test module has no equivalent yet:

```rust
    fn goto_params(uri: &lsp_types::Url, line: u32, character: u32) -> lsp_types::GotoDefinitionParams {
        lsp_types::GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                position: lsp_types::Position { line, character },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }
    }

    fn hover_params(uri: &lsp_types::Url, line: u32, character: u32) -> lsp_types::HoverParams {
        lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                position: lsp_types::Position { line, character },
            },
            work_done_progress_params: Default::default(),
        }
    }
```

- [ ] **Step 2: Run them to verify they fail**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin cross_page -- --include-ignored
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin another_page
```

Expected: FAIL to compile (`resolve_definition` takes 2 arguments, not 3).

- [ ] **Step 3: Thread the cache and add the fallback**

1. Add `cache: &mut crate::lsp_project::ProjectCache` as the second parameter of both `resolve_definition` and `resolve_hover`.
2. In `handle_request`, take a `cache: &mut crate::lsp_project::ProjectCache` parameter beside `memo`, and pass it through at the `GotoDefinition::METHOD` and `HoverRequest::METHOD` arms.
3. In the message loop that owns `memo`, construct `let mut project = crate::lsp_project::ProjectCache::new();` alongside it and pass `&mut project` into `handle_request`.
4. Replace the `Target::Xref` arm of `resolve_definition` (`:829`) with:

```rust
        // `@fig-x` → its definition. The open buffer wins: it is ahead of the on-disk copy,
        // and an unsaved anchor must not send the author to yesterday's file. Only when the
        // buffer does not define it does the project walk answer, which is what closes the
        // cross-file half of the gap this function's doc comment names.
        Target::Xref { id, .. } => match crate::lsp_nav::definition_site(text, &id) {
            Some((line, col)) => Location::new(
                uri.clone(),
                point(text, line, col, col + id.chars().count() as u32),
            ),
            None => {
                let here = uri.to_file_path().ok()?;
                let anchor = cache.get(&here)?.anchors.iter().find(|a| a.id == id)?;
                let body = std::fs::read_to_string(&anchor.path).ok()?;
                Location::new(
                    Url::from_file_path(&anchor.path).ok()?,
                    point(&body, anchor.line, 0, 0),
                )
            }
        },
```

5. In `resolve_hover`'s `Target::Xref` arm, when `xref_number` returns `None`, fall back to the project scan and render the page as well as the label:

```rust
        Target::Xref { id, start, end } => {
            let label = xref_label(&id)?;
            match xref_number(uri, text, &id) {
                Some(number) => markup(format!("**{label} {number}** — `@{id}`"), start, end),
                // Defined on another page: the number belongs to that page's render, which
                // this kernel-free path does not have, so name the page instead of nothing.
                None => {
                    let here = uri.to_file_path().ok()?;
                    let anchor = cache.get(&here)?.anchors.iter().find(|a| a.id == id)?;
                    let page = anchor
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let shown = if anchor.number.is_empty() {
                        format!("**{label}** `@{id}`\n\nDefined in `{page}`")
                    } else {
                        format!("**{label} {}** `@{id}`\n\nDefined in `{page}`", anchor.number)
                    };
                    markup(shown, start, end)
                }
            }
        }
```

Keep the existing em dash in the untouched same-page string; do not introduce a new one in the added branch.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin lsp:: 2>&1 | tail -20
```

Expected: no `FAILED`; the four new tests pass along with the existing lsp suite.

- [ ] **Step 5: Verify by mutation**

Swap the match arms so the project scan is consulted first and the buffer second. Re-run. Expected: `a_local_definition_still_wins_over_the_project_walk` FAILS. Restore by inverse edit.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/lsp.rs
git commit -m "feat(lsp): cross-file xref go-to-definition and hover (idea 75)"
```

---

### Task 5: Idea 76, workspace symbols

**Files:**
- Modify: `crates/server/src/lsp.rs` (`server_capabilities` at `:21`, `handle_request` dispatch chain at `:438`)
- Modify: `docs/internals/extending.tmd` (the capability table; a drift gate fails without the row)
- Test: `crates/server/src/lsp.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `crate::lsp_project::ProjectCache` (Task 3), already threaded into `handle_request` by Task 4.
- Produces: the `workspace/symbol` handler. No new types.

**A note on matching:** case-insensitive substring only. VS Code applies its own fuzzy ranking on top of whatever the server returns, and a second ranking here would fight it.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn workspace_symbols_reach_headings_and_anchors_on_every_page() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::write(root.join("index.tmd"), "# Introduction\n").unwrap();
        std::fs::write(
            root.join("two.tmd"),
            "# Scree Plots\n\n![p](i.png){#fig-scree}\n",
        )
        .unwrap();
        let mut cache = crate::lsp_project::ProjectCache::new();

        let hits = workspace_symbols(&mut cache, root, "scree");
        let names: Vec<&str> = hits.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Scree Plots"), "heading on another page: {names:?}");
        assert!(names.contains(&"fig-scree"), "anchor on another page: {names:?}");
        assert!(!names.contains(&"Introduction"), "non-matching symbol leaked in");
    }

    #[test]
    fn workspace_symbol_matching_ignores_case() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::write(root.join("index.tmd"), "# Introduction\n").unwrap();
        let mut cache = crate::lsp_project::ProjectCache::new();
        assert_eq!(workspace_symbols(&mut cache, root, "INTRO").len(), 1);
    }

    #[test]
    fn an_empty_query_returns_every_symbol_rather_than_none() {
        // VS Code opens Ctrl+T with an empty query and expects a browsable list, not silence.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::write(root.join("index.tmd"), "# A\n\n## B\n").unwrap();
        let mut cache = crate::lsp_project::ProjectCache::new();
        assert_eq!(workspace_symbols(&mut cache, root, "").len(), 2);
    }

    #[test]
    fn the_capabilities_advertise_workspace_symbols() {
        let caps = serde_json::to_value(server_capabilities()).unwrap();
        assert_eq!(caps["workspaceSymbolProvider"], true);
    }
```

- [ ] **Step 2: Run them to verify they fail**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin workspace_symbol
```

Expected: FAIL to compile, `cannot find function 'workspace_symbols'`.

- [ ] **Step 3: Implement**

In `server_capabilities`, beside `document_symbol_provider`:

```rust
        // Ctrl+T across the whole book. `documentSymbol` is per-file, which for a 25-chapter
        // project means the author has to already know which file holds the heading they want.
        // Answered by a project walk, not an index: the request fires on a gesture, not on
        // every keystroke.
        workspace_symbol_provider: Some(OneOf::Left(true)),
```

Add the resolver near `resolve_definition`:

```rust
/// Every heading and cross-reference anchor in `page`'s project whose name contains `query`,
/// case-insensitively. An empty query returns everything, because that is the state Ctrl+T
/// opens in and an empty list there reads as "this project has no symbols".
///
/// Ranking is deliberately absent: VS Code applies its own fuzzy sort to whatever comes back,
/// and a second ranking here would fight it.
fn workspace_symbols(
    cache: &mut crate::lsp_project::ProjectCache,
    page: &std::path::Path,
    query: &str,
) -> Vec<lsp_types::SymbolInformation> {
    use lsp_types::{Location, Position, Range, SymbolKind, Url};
    let needle = query.to_lowercase();
    let Some(scan) = cache.get(page) else {
        return Vec::new();
    };
    let matches = |name: &str| needle.is_empty() || name.to_lowercase().contains(&needle);
    let at = |path: &std::path::Path, line: u32| {
        Url::from_file_path(path).ok().map(|uri| {
            Location::new(
                uri,
                Range::new(Position::new(line, 0), Position::new(line, 0)),
            )
        })
    };

    let mut out = Vec::new();
    for h in &scan.headings {
        if matches(&h.text)
            && let Some(location) = at(&h.path, h.line)
        {
            out.push(symbol(h.text.clone(), SymbolKind::MODULE, location, None));
        }
    }
    for a in &scan.anchors {
        if matches(&a.id)
            && let Some(location) = at(&a.path, a.line)
        {
            let detail = (!a.title.is_empty()).then(|| a.title.clone());
            out.push(symbol(a.id.clone(), SymbolKind::KEY, location, detail));
        }
    }
    out
}

/// `SymbolInformation` is deprecated in `lsp-types` in favour of `WorkspaceSymbol`, but the
/// struct literal still has to be spelled out, and the deprecated field must be set. Kept in
/// one helper so the `#[allow]` sits in exactly one place.
#[allow(deprecated)]
fn symbol(
    name: String,
    kind: lsp_types::SymbolKind,
    location: lsp_types::Location,
    container_name: Option<String>,
) -> lsp_types::SymbolInformation {
    lsp_types::SymbolInformation {
        name,
        kind,
        tags: None,
        deprecated: None,
        location,
        container_name,
    }
}
```

In `handle_request`, add an arm beside the `DocumentSymbolRequest::METHOD` one at `:537`:

```rust
    } else if req.method == WorkspaceSymbolRequest::METHOD {
        let params: lsp_types::WorkspaceSymbolParams = serde_json::from_value(req.params)?;
        // The project is discovered from any open document: the request itself names no file,
        // and every open `.tmd` in one window belongs to the same project in practice. An
        // empty editor answers nothing, which is correct rather than an error.
        let anchor = docs.keys().find_map(|u| u.to_file_path().ok());
        let found = anchor
            .map(|p| workspace_symbols(cache, &p, &params.query))
            .unwrap_or_default();
        Some(serde_json::to_value(found)?)
```

Add `WorkspaceSymbolRequest` to the `use lsp_types::request::{…}` list at `:444`.

- [ ] **Step 4: Add the documentation row**

In `docs/internals/extending.tmd`, add a row to the capability table matching the existing column shape:

```
| `workspaceSymbol` | Ctrl+T to any heading or cross-reference anchor in the project, answered by a stat-validated walk of every page rather than a live index. |
```

The gate strips the `Provider` suffix and looks for `` | `workspaceSymbol` ``, so the backticked name must match exactly.

- [ ] **Step 5: Run to verify they pass**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin workspace_symbol
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin the_internals_capability_table
```

Expected: PASS on both.

- [ ] **Step 6: Verify by mutation**

Delete the `workspaceSymbol` row from `docs/internals/extending.tmd`. Re-run the capability-table test. Expected: FAIL. Restore it.

Then change `needle.is_empty() ||` to `false ||`. Re-run. Expected: `an_empty_query_returns_every_symbol_rather_than_none` FAILS. Restore by inverse edit.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/lsp.rs docs/internals/extending.tmd
git commit -m "feat(lsp): workspace symbols across the project (idea 76)"
```

---

### Task 6: Two custom requests for the sidebar

**Files:**
- Modify: `crates/server/src/lsp.rs` (method constants beside `RENAME_FILE_EDITS_METHOD` at `:883`, dispatch beside `CELL_REGIONS_METHOD` at `:547`)
- Test: `crates/server/src/lsp.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `crate::lsp_project::ProjectCache` (Task 3).
- Produces, for Task 7 to call over the language client:
  - `taliesin/projectOutline`, params `{ "uri": string }`, result:
    ```json
    { "root": "/abs/root",
      "pages": [ { "path": "/abs/p.tmd", "headings": [ { "line": 0, "level": 1, "text": "A" } ] } ],
      "floats": [ { "id": "fig-x", "path": "/abs/p.tmd", "line": 4, "title": "", "number": "2.1" } ] }
    ```
  - `taliesin/projectRefs`, params `{ "uri": string }`, result:
    ```json
    { "root": "/abs/root",
      "targets": [ { "id": "fig-x", "resolved": true, "definedIn": "/abs/p.tmd", "definedLine": 4,
                     "uses": [ { "path": "/abs/q.tmd", "line": 9, "col": 4 } ] } ] }
    ```

`resolved` is `false` and `definedIn` is `null` for a dangling target. Both requests answer `null` for a document with no enclosing project, and the client renders an empty view rather than an error.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn project_outline_lists_every_page_with_its_headings_and_floats() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::write(
            root.join("index.tmd"),
            "# One\n\n## Deeper\n\n![p](i.png){#fig-a}\n",
        )
        .unwrap();
        let mut cache = crate::lsp_project::ProjectCache::new();
        let out = project_outline(&mut cache, &root.join("index.tmd")).unwrap();

        assert_eq!(out["pages"].as_array().unwrap().len(), 1);
        let headings = out["pages"][0]["headings"].as_array().unwrap();
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[1]["text"], "Deeper");
        assert_eq!(headings[1]["level"], 2);
        assert_eq!(out["floats"][0]["id"], "fig-a");
    }

    #[test]
    fn project_refs_groups_uses_by_target_and_flags_the_dangling_one() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("_site.yml"), "title: t\n").unwrap();
        std::fs::write(root.join("a.tmd"), "# A {#sec-a}\n").unwrap();
        std::fs::write(root.join("b.tmd"), "See @sec-a and @sec-gone.\n").unwrap();
        let mut cache = crate::lsp_project::ProjectCache::new();
        let refs = project_refs(&mut cache, &root.join("b.tmd")).unwrap();

        let targets = refs["targets"].as_array().unwrap();
        let resolved = targets.iter().find(|t| t["id"] == "sec-a").unwrap();
        assert_eq!(resolved["resolved"], true);
        assert_eq!(resolved["uses"].as_array().unwrap().len(), 1);

        let dangling = targets.iter().find(|t| t["id"] == "sec-gone").unwrap();
        assert_eq!(dangling["resolved"], false);
        assert!(dangling["definedIn"].is_null());
    }

    #[test]
    fn both_project_requests_answer_none_outside_a_project() {
        let tmp = tempfile::tempdir().unwrap();
        let solo = tmp.path().join("solo.tmd");
        std::fs::write(&solo, "# Solo\n\nSee @sec-x.\n").unwrap();
        let mut cache = crate::lsp_project::ProjectCache::new();
        assert!(project_outline(&mut cache, &solo).is_none());
        assert!(project_refs(&mut cache, &solo).is_none());
    }

    #[test]
    fn every_custom_method_constant_is_namespaced() {
        // The existing census test already enforces this shape; this pins the two new ones
        // explicitly so a rename cannot quietly drop the prefix.
        assert!(PROJECT_OUTLINE_METHOD.starts_with("taliesin/"));
        assert!(PROJECT_REFS_METHOD.starts_with("taliesin/"));
    }
```

- [ ] **Step 2: Run them to verify they fail**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin project_outline project_refs
```

Expected: FAIL to compile.

- [ ] **Step 3: Implement**

Beside the other method constants:

```rust
/// The custom request behind the sidebar's whole-book Outline and Figures views. Namespaced
/// for the same reason as [`CELL_REGIONS_METHOD`]: it is not an LSP method. `workspace/symbol`
/// is the closest standard method and deliberately answers a *flat, queried* list, which is
/// the wrong shape for a tree the author browses.
pub(crate) const PROJECT_OUTLINE_METHOD: &str = "taliesin/projectOutline";

/// The custom request behind the sidebar's References view: every cross-reference target with
/// the uses pointing at it, dangling ones included. Namespaced for the same reason.
pub(crate) const PROJECT_REFS_METHOD: &str = "taliesin/projectRefs";
```

The two resolvers, near `workspace_symbols`:

```rust
/// The whole-book outline plus the numbered-float index, as the sidebar's TreeViews want it:
/// grouped by page and in reading order, not flattened. `None` outside a project.
fn project_outline(
    cache: &mut crate::lsp_project::ProjectCache,
    page: &std::path::Path,
) -> Option<serde_json::Value> {
    let scan = cache.get(page)?;
    let mut pages: Vec<serde_json::Value> = Vec::new();
    for h in &scan.headings {
        let path = h.path.to_string_lossy().into_owned();
        let row = serde_json::json!({ "line": h.line, "level": h.level, "text": h.text });
        match pages
            .iter_mut()
            .find(|p| p["path"].as_str() == Some(path.as_str()))
        {
            Some(p) => p["headings"].as_array_mut()?.push(row),
            None => pages.push(serde_json::json!({ "path": path, "headings": [row] })),
        }
    }
    let floats: Vec<serde_json::Value> = scan
        .anchors
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "path": a.path.to_string_lossy(),
                "line": a.line,
                "title": a.title,
                "number": a.number,
            })
        })
        .collect();
    Some(serde_json::json!({
        "root": scan.root.to_string_lossy(),
        "pages": pages,
        "floats": floats,
    }))
}

/// Every cross-reference target with the uses pointing at it. A target with no definition is
/// reported with `resolved: false` rather than omitted: grouping dangling references is the
/// reason `lsp_nav::xref_occurrences` exists at all.
fn project_refs(
    cache: &mut crate::lsp_project::ProjectCache,
    page: &std::path::Path,
) -> Option<serde_json::Value> {
    let scan = cache.get(page)?;
    let mut ids: Vec<&str> = scan.anchors.iter().map(|a| a.id.as_str()).collect();
    for u in &scan.uses {
        if !ids.contains(&u.id.as_str()) {
            ids.push(&u.id);
        }
    }
    ids.sort_unstable();
    ids.dedup();

    let targets: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| {
            let defined = scan.anchors.iter().find(|a| a.id == *id);
            let uses: Vec<serde_json::Value> = scan
                .uses
                .iter()
                .filter(|u| u.id == *id)
                .map(|u| serde_json::json!({
                    "path": u.path.to_string_lossy(),
                    "line": u.line,
                    "col": u.col,
                }))
                .collect();
            serde_json::json!({
                "id": id,
                "resolved": defined.is_some(),
                "definedIn": defined.map(|d| d.path.to_string_lossy().into_owned()),
                "definedLine": defined.map(|d| d.line),
                "uses": uses,
            })
        })
        .collect();
    Some(serde_json::json!({ "root": scan.root.to_string_lossy(), "targets": targets }))
}
```

Dispatch, beside the `CELL_REGIONS_METHOD` arm:

```rust
    } else if req.method == PROJECT_OUTLINE_METHOD || req.method == PROJECT_REFS_METHOD {
        #[derive(serde::Deserialize)]
        struct ProjectParams {
            uri: lsp_types::Url,
        }
        let params: ProjectParams = serde_json::from_value(req.params)?;
        let answer = params.uri.to_file_path().ok().and_then(|p| {
            if req.method == PROJECT_OUTLINE_METHOD {
                project_outline(cache, &p)
            } else {
                project_refs(cache, &p)
            }
        });
        // `null` rather than an error for a document outside any project: the sidebar renders
        // an empty view, which is the honest answer for a standalone document.
        Some(answer.unwrap_or(serde_json::Value::Null))
```

- [ ] **Step 4: Run to verify they pass**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin project_outline project_refs every_custom_method
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin lsp:: 2>&1 | tail -15
```

Expected: PASS, and the existing `"taliesin/…"` census test at `:4142` still green (it enumerates every namespaced literal in the file, so the two new constants are counted automatically).

- [ ] **Step 5: Verify by mutation**

In `project_refs`, change the `ids` seeding loop so dangling uses are skipped (delete the `for u in &scan.uses` block). Re-run. Expected: `project_refs_groups_uses_by_target_and_flags_the_dangling_one` FAILS. Restore by inverse edit.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/lsp.rs
git commit -m "feat(lsp): taliesin/projectOutline and taliesin/projectRefs for the sidebar"
```

---

### Task 7: Idea 77, the sidebar (three TreeViews)

**Files:**
- Create: `editor/vscode/src/sidebar.ts`
- Modify: `editor/vscode/src/extension.ts` (one registration call in `activate`)
- Modify: `editor/vscode/package.json` (`contributes.viewsContainers`, `contributes.views`, one command)
- Test: create `editor/vscode/src/test/sidebar.test.ts`

**Interfaces:**
- Consumes: `taliesin/projectOutline` and `taliesin/projectRefs` (Task 6); `languageClient()` from `client.ts`.
- Produces: `export function registerSidebar(context: vscode.ExtensionContext): void`, plus two pure functions the unit test drives without a VS Code host:
  - `export function outlineTree(reply: OutlineReply): TreeRow[]`
  - `export function refsTree(reply: RefsReply): TreeRow[]`

**Scope, and what is deliberately absent:** three views only. Whole-book **Outline**, **References** (uses grouped by target, dangling grouped separately), and **Figures & tables**. The idea's bibliography view is cut (`check` already reports unresolved citations) and its kernel panel belongs to Task 10. **No drag-to-reorder, no rename-in-tree, no editing of any kind**: this is read-only navigation, and a write path here is the removed slide-reorder mistake in a new costume.

- [ ] **Step 1: Write the failing tests**

Create `editor/vscode/src/test/sidebar.test.ts`. Model it on the existing pure-function tests in `map.test.ts`, which run under `node --test` with no VS Code host:

```ts
import { test } from "node:test";
import assert from "node:assert";
import { outlineTree, refsTree } from "../sidebar";

test("the outline tree nests headings under their page and by level", () => {
  const rows = outlineTree({
    root: "/r",
    pages: [
      { path: "/r/index.tmd", headings: [
        { line: 0, level: 1, text: "One" },
        { line: 4, level: 2, text: "Deeper" },
      ] },
    ],
    floats: [],
  });
  assert.strictEqual(rows.length, 1, "one page row");
  assert.strictEqual(rows[0].label, "index.tmd");
  assert.strictEqual(rows[0].children[0].label, "One");
  assert.strictEqual(rows[0].children[0].children[0].label, "Deeper",
    "a level-2 heading nests under the level-1 above it");
});

test("a heading that skips a level still nests rather than being dropped", () => {
  // `# A` then `### C` is legal Markdown and common in real documents. A tree builder that
  // only accepts level+1 silently loses C, which is worse than showing it one level shallow.
  const rows = outlineTree({
    root: "/r",
    pages: [{ path: "/r/a.tmd", headings: [
      { line: 0, level: 1, text: "A" },
      { line: 2, level: 3, text: "C" },
    ] }],
    floats: [],
  });
  assert.strictEqual(rows[0].children[0].children[0].label, "C");
});

test("the references tree separates resolved targets from dangling ones", () => {
  const rows = refsTree({
    root: "/r",
    targets: [
      { id: "sec-a", resolved: true, definedIn: "/r/a.tmd", definedLine: 0,
        uses: [{ path: "/r/b.tmd", line: 9, col: 4 }] },
      { id: "sec-gone", resolved: false, definedIn: null, definedLine: null,
        uses: [{ path: "/r/b.tmd", line: 10, col: 4 }] },
    ],
  });
  const groups = rows.map((r) => r.label);
  assert.deepStrictEqual(groups, ["Resolved (1)", "Dangling (1)"]);
  assert.strictEqual(rows[1].children[0].label, "sec-gone");
});

test("a target nobody references is not listed as dangling", () => {
  // An anchor defined and never used is normal, not an error. Only a USE with no definition
  // is dangling, and conflating the two would fill the view with false alarms.
  const rows = refsTree({
    root: "/r",
    targets: [
      { id: "sec-unused", resolved: true, definedIn: "/r/a.tmd", definedLine: 0, uses: [] },
    ],
  });
  assert.deepStrictEqual(rows.map((r) => r.label), ["Resolved (1)", "Dangling (0)"]);
});

test("an empty reply renders empty views rather than throwing", () => {
  assert.deepStrictEqual(outlineTree({ root: "", pages: [], floats: [] }), []);
  assert.deepStrictEqual(
    refsTree({ root: "", targets: [] }).map((r) => r.label),
    ["Resolved (0)", "Dangling (0)"]
  );
});
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd editor/vscode && npm run compile && node --test out/test/sidebar.test.js
```

Expected: FAIL, `Cannot find module '../sidebar'`. (Check `package.json`'s `scripts` for the exact compile/test invocation the repo already uses and match it.)

- [ ] **Step 3: Implement `sidebar.ts`**

Write `editor/vscode/src/sidebar.ts` with:

1. The reply types mirroring Task 6's JSON exactly (`OutlineReply`, `RefsReply`, `TreeRow`). `TreeRow` is `{ label: string; description?: string; path?: string; line?: number; children: TreeRow[] }`.
2. `outlineTree(reply)`: one row per page labelled with its basename, headings nested by level using a stack. **When a heading's level jumps by more than one, attach it to the deepest open ancestor rather than dropping it**, which the second test pins.
3. `refsTree(reply)`: exactly two group rows, `Resolved (n)` and `Dangling (n)`, always both present so the view never looks broken when one side is empty. A target is dangling when `resolved === false`; a resolved target with zero uses stays under Resolved.
4. `floatsTree(reply)` for the third view, one row per float labelled `` `${id}` `` with the number as `description`.
5. A `vscode.TreeDataProvider<TreeRow>` wrapping each, and `registerSidebar(context)` which:
   - creates the three `TreeView`s against the view ids declared in `package.json`,
   - refreshes on `vscode.workspace.onDidSaveTextDocument` filtered to `.tmd`, and on `vscode.window.onDidChangeActiveTextEditor`,
   - sends the two requests via `languageClient()?.sendRequest("taliesin/projectOutline", { uri })`, guarding for an undefined client (the server may not have started),
   - gives each leaf row a `command` of `vscode.open` with a `Position` selection so clicking navigates. **This is navigation only; no row may offer an edit.**

Add to `package.json`:

```json
"viewsContainers": {
  "activitybar": [
    { "id": "taliesin", "title": "Taliesin", "icon": "media/sidebar.svg" }
  ]
},
"views": {
  "taliesin": [
    { "id": "taliesin.outline", "name": "Outline" },
    { "id": "taliesin.references", "name": "References" },
    { "id": "taliesin.floats", "name": "Figures & Tables" }
  ]
}
```

Create `media/sidebar.svg` as a monochrome 24x24 icon (VS Code recolours it). Remember the standing trap: an SVG `<style>` block is XML, so a bare `<` or `&` in its CSS kills the file. Prefer plain attributes over a `<style>` block.

In `extension.ts`, add `registerSidebar(context);` beside `registerTerminalLinks(context);` with a one-line comment saying it is read-only navigation over the project walk.

- [ ] **Step 4: Run to verify they pass**

```bash
cd editor/vscode && npm run compile && node --test out/test/sidebar.test.js
cd editor/vscode && npx -y -p typescript tsc -p . --noEmit
```

Expected: 5 tests pass, `tsc` clean.

- [ ] **Step 5: Verify by mutation**

In `outlineTree`, change the level-jump handling to `if (h.level !== top.level + 1) continue;`. Re-run. Expected: `a heading that skips a level still nests rather than being dropped` FAILS. Restore by inverse edit.

- [ ] **Step 6: Commit**

```bash
git add editor/vscode/src/sidebar.ts editor/vscode/src/test/sidebar.test.ts editor/vscode/src/extension.ts editor/vscode/package.json editor/vscode/media/sidebar.svg
git commit -m "feat(companion): a read-only Taliesin sidebar, three views (idea 77)"
```

---

### Task 8: Idea 80, task provider and problem matchers

**Files:**
- Create: `editor/vscode/src/tasks.ts`
- Modify: `editor/vscode/src/extension.ts`, `editor/vscode/package.json` (`contributes.taskDefinitions`, `contributes.problemMatchers`)
- Test: create `editor/vscode/src/test/tasks.test.ts`

**Interfaces:**
- Consumes: `projectRootFor` from `paths.ts`.
- Produces: `export function registerTasks(context: vscode.ExtensionContext): void` and the pure `export function taskSpecs(root: string): TaskSpec[]` where `TaskSpec = { name: string; args: string[] }`.

**The format, measured:** `check` prints `path:line: severity[CODE]: message` and `build` prints `path:line:`. **There is no column.** A `problemMatcher` pattern requiring `:col` matches nothing, which is the exact mis-transcription the ideas file already corrected once.

- [ ] **Step 1: Write the failing tests**

```ts
import { test } from "node:test";
import assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import { taskSpecs } from "../tasks";

const manifest = JSON.parse(
  fs.readFileSync(path.join(__dirname, "..", "..", "package.json"), "utf8")
);

test("three tasks are offered for a project root", () => {
  const names = taskSpecs("/r").map((t) => t.name);
  assert.deepStrictEqual(names, ["check", "build", "build --out"]);
});

test("the check task targets the project root, not a single file", () => {
  const check = taskSpecs("/r").find((t) => t.name === "check")!;
  assert.deepStrictEqual(check.args, ["check", "/r"]);
});

test("the problem matcher pattern matches real check output and has no column group", () => {
  // Real output, copied from `taliesin check`: `path:line: severity[CODE]: message`.
  const matcher = manifest.contributes.problemMatchers.find(
    (m: { name: string }) => m.name === "taliesin"
  );
  assert.ok(matcher, "package.json must contribute a `taliesin` problem matcher");
  const re = new RegExp(matcher.pattern.regexp);
  const line = "using/formats.tmd:12: WARNING[TAL0042]: unresolved cross-reference";
  const m = re.exec(line);
  assert.ok(m, "the pattern does not match real `check` output");
  assert.strictEqual(m[matcher.pattern.file], "using/formats.tmd");
  assert.strictEqual(m[matcher.pattern.line], "12");
  assert.strictEqual(m[matcher.pattern.severity], "WARNING");
  assert.strictEqual(m[matcher.pattern.message], "unresolved cross-reference");
  assert.strictEqual(
    matcher.pattern.column,
    undefined,
    "check output carries no column; a pattern that requires one matches nothing"
  );
});

test("the problem matcher does not match a line that only looks like a diagnostic", () => {
  const matcher = manifest.contributes.problemMatchers.find(
    (m: { name: string }) => m.name === "taliesin"
  );
  const re = new RegExp(matcher.pattern.regexp);
  assert.strictEqual(re.exec("Checked 25 pages, 0 problems"), null);
  assert.strictEqual(re.exec("  at some/file.rs:12: something"), null);
});
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd editor/vscode && npm run compile && node --test out/test/tasks.test.js
```

Expected: FAIL, `Cannot find module '../tasks'`.

- [ ] **Step 3: Implement**

`tasks.ts` exports `taskSpecs(root)` returning exactly:

```ts
[
  { name: "check", args: ["check", root] },
  { name: "build", args: ["build", root] },
  { name: "build --out", args: ["build", root, "--out", "_site"] },
]
```

and `registerTasks(context)` calling `vscode.tasks.registerTaskProvider("taliesin", { provideTasks, resolveTask })`. `provideTasks` finds the project root for the active document via `projectRootFor`, builds a `vscode.Task` per spec with a `ShellExecution` of the configured binary, sets `task.group` (`Build` for the two builds, `Test` for check), and attaches `problemMatchers: ["$taliesin"]`.

In `package.json`:

```json
"taskDefinitions": [
  { "type": "taliesin", "required": ["command"],
    "properties": { "command": { "type": "string", "description": "check, build, or build --out" } } }
],
"problemMatchers": [
  {
    "name": "taliesin",
    "owner": "taliesin",
    "fileLocation": ["autoDetected", "${workspaceFolder}"],
    "pattern": {
      "regexp": "^([^\\s:][^:]*):(\\d+): (ERROR|WARNING|SUGGESTION)\\[([^\\]]+)\\]: (.*)$",
      "file": 1, "line": 2, "severity": 3, "code": 4, "message": 5
    }
  }
]
```

The leading `[^\s:]` is what makes the fourth test pass: it rejects the indented `  at some/file.rs:12:` shape.

Register in `extension.ts` beside the others.

- [ ] **Step 4: Run to verify they pass**

```bash
cd editor/vscode && npm run compile && node --test out/test/tasks.test.js
```

Expected: 4 tests pass.

- [ ] **Step 5: Verify against real output, not just the fixture**

```bash
cd /home/bogo/Documents/personal/taliesin
./target/release/taliesin check docs/guide 2>&1 | head -5
```

Take a real line from that output and confirm by hand that the regexp above matches it. If `check` currently reports zero problems on the guide, introduce a temporary broken `@fig-nope` reference in a scratch copy under the scratchpad to generate one; do not commit it. **A matcher tested only against a hand-written fixture is a matcher tested against your assumption of the format.**

- [ ] **Step 6: Verify by mutation**

Add `"column": 3` to the pattern and shift the group indices. Re-run. Expected: the third test FAILS on the column assertion. Restore by inverse edit.

- [ ] **Step 7: Commit**

```bash
git add editor/vscode/src/tasks.ts editor/vscode/src/test/tasks.test.ts editor/vscode/src/extension.ts editor/vscode/package.json
git commit -m "feat(companion): task provider + problem matcher, check lands in Problems (idea 80)"
```

---

### Task 9: Idea 78, file decorations from project check status

**Files:**
- Create: `editor/vscode/src/decorations.ts`
- Modify: `editor/vscode/src/extension.ts`, `editor/vscode/package.json` (one setting)
- Test: create `editor/vscode/src/test/decorations.test.ts`

**Interfaces:**
- Consumes: `check --format json`, whose shape is `{ diagnostics: [{ code, docs_url, severity, file, line, message, suggestion? }], environment: [...] }`.
- Produces: `export function registerDecorations(context: vscode.ExtensionContext): void` and the pure `export function worstByFile(json: CheckJson, root: string): Map<string, Severity>` where `Severity = "ERROR" | "WARNING" | "SUGGESTION"`, keyed by absolute path.

**Scope:** the check-status dot only. The idea's `⚡ fully cached` and never-executed-cell badges need freeze-key machinery from `exec`, and this component is deliberately kernel-free. Task 14 files that as detection debt rather than dropping it silently.

**Cost, measured:** `check docs/guide` (25 pages) is **369 ms**. Run it debounced on save, never on keystroke, and never more than one at a time.

- [ ] **Step 1: Write the failing tests**

```ts
import { test } from "node:test";
import assert from "node:assert";
import { worstByFile } from "../decorations";

const json = {
  diagnostics: [
    { severity: "WARNING", file: "a.tmd", line: 3, code: "TAL1", message: "w" },
    { severity: "ERROR", file: "a.tmd", line: 9, code: "TAL2", message: "e" },
    { severity: "SUGGESTION", file: "b.tmd", line: 1, code: "TAL3", message: "s" },
  ],
  environment: [],
};

test("a file's worst severity wins over its others", () => {
  const worst = worstByFile(json, "/r");
  assert.strictEqual(worst.get("/r/a.tmd"), "ERROR",
    "a.tmd has both a WARNING and an ERROR; the badge must show the worse one");
  assert.strictEqual(worst.get("/r/b.tmd"), "SUGGESTION");
});

test("severity order does not depend on the order diagnostics arrive in", () => {
  // The same two diagnostics, reversed. A naive last-write-wins map gets this wrong.
  const reversed = { diagnostics: [...json.diagnostics].reverse(), environment: [] };
  assert.strictEqual(worstByFile(reversed, "/r").get("/r/a.tmd"), "ERROR");
});

test("a clean project decorates nothing", () => {
  assert.strictEqual(worstByFile({ diagnostics: [], environment: [] }, "/r").size, 0);
});

test("an already-absolute file path is not re-rooted", () => {
  const abs = { diagnostics: [
    { severity: "ERROR", file: "/elsewhere/c.tmd", line: 1, code: "T", message: "m" },
  ], environment: [] };
  assert.ok(worstByFile(abs, "/r").has("/elsewhere/c.tmd"),
    "joining a root onto an absolute path produces a key nothing in the Explorer matches");
});
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd editor/vscode && npm run compile && node --test out/test/decorations.test.js
```

Expected: FAIL, module not found.

- [ ] **Step 3: Implement**

`worstByFile` ranks `ERROR > WARNING > SUGGESTION` via an explicit rank map and keeps the max per file, resolving each `file` against `root` with `path.resolve(root, file)` (which leaves an absolute path alone, satisfying the fourth test).

`registerDecorations(context)` registers a `vscode.FileDecorationProvider` whose `provideFileDecoration(uri)` looks the path up in the current map and returns a `vscode.FileDecoration` with badge `!` / `▲` / `·` and a `tooltip`, or `undefined`. It refreshes by spawning `taliesin check <root> --format json` (using the configured binary path) on activation and on `onDidSaveTextDocument` for `.tmd`, **debounced to at most one run in flight**, then fires the provider's `onDidChangeFileDecorations` emitter with the union of the previously and newly decorated URIs (a file that just went clean must lose its badge, which firing only for current keys would miss).

Add a setting so it can be turned off:

```json
"taliesin.explorerBadges": {
  "type": "boolean", "default": true,
  "description": "Badge .tmd files in the Explorer with their worst `taliesin check` severity."
}
```

- [ ] **Step 4: Run to verify they pass**

```bash
cd editor/vscode && npm run compile && node --test out/test/decorations.test.js
```

Expected: 4 tests pass.

- [ ] **Step 5: Verify by mutation**

Replace the rank comparison with plain assignment (`worst.set(key, d.severity)` unconditionally). Re-run. Expected: `severity order does not depend on the order diagnostics arrive in` FAILS. Restore by inverse edit.

- [ ] **Step 6: Commit**

```bash
git add editor/vscode/src/decorations.ts editor/vscode/src/test/decorations.test.ts editor/vscode/src/extension.ts editor/vscode/package.json
git commit -m "feat(companion): Explorer badges from project check status (idea 78)"
```

---

### Task 10: Idea 79, status bar

**Files:**
- Create: `editor/vscode/src/statusbar.ts`
- Modify: `editor/vscode/src/extension.ts`, `editor/vscode/package.json` (one command)
- Test: create `editor/vscode/src/test/statusbar.test.ts`

**Interfaces:**
- Consumes: `PreviewRegistry` from `previews.ts` (it already knows which previews are live and on which port); the decoration map from Task 9 is **not** consumed, the count is recomputed from the same check JSON to keep the modules independent.
- Produces: `export function registerStatusBar(context, previews: PreviewRegistry): void` and the pure `export function statusText(state: StatusState): string` where `StatusState = { previewPort: number | null; problems: number | null }`.

**Deliberately absent: live kernel and cache state.** The webview relay carries exactly four message types on purpose (`tali-goto`, `tali-page` up; `tali-cursor`, `tali-navigate` down). Widening it is a protocol decision to make on its own merits, not a side effect of a status bar. Task 14 files this as detection debt.

- [ ] **Step 1: Write the failing tests**

```ts
import { test } from "node:test";
import assert from "node:assert";
import { statusText } from "../statusbar";

test("a running preview shows its port", () => {
  assert.strictEqual(statusText({ previewPort: 4388, problems: 0 }), "$(book) Taliesin :4388");
});

test("problems are shown when there are any", () => {
  assert.strictEqual(statusText({ previewPort: 4388, problems: 3 }), "$(book) Taliesin :4388 · 3 problems");
});

test("one problem is not pluralised", () => {
  assert.strictEqual(statusText({ previewPort: null, problems: 1 }), "$(book) Taliesin · 1 problem");
});

test("an unknown problem count is omitted rather than shown as zero", () => {
  // `null` means check has not run yet. Rendering that as "0 problems" claims a clean
  // project the extension has not verified.
  assert.strictEqual(statusText({ previewPort: null, problems: null }), "$(book) Taliesin");
});
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd editor/vscode && npm run compile && node --test out/test/statusbar.test.js
```

Expected: FAIL, module not found.

- [ ] **Step 3: Implement**

`statusText` composes the pieces above exactly. `registerStatusBar` creates a `vscode.StatusBarItem` (alignment `Left`, priority 100), sets `command` to `taliesin.openPreview` so clicking opens or focuses the preview, updates on active-editor change and on save, and hides the item entirely when the active document is not a `.tmd` and no preview is live.

- [ ] **Step 4: Run to verify they pass**

```bash
cd editor/vscode && npm run compile && node --test out/test/statusbar.test.js
```

Expected: 4 tests pass.

- [ ] **Step 5: Verify by mutation**

Change the null-problems branch to render `0 problems`. Re-run. Expected: the fourth test FAILS. Restore by inverse edit.

- [ ] **Step 6: Commit**

```bash
git add editor/vscode/src/statusbar.ts editor/vscode/src/test/statusbar.test.ts editor/vscode/src/extension.ts editor/vscode/package.json
git commit -m "feat(companion): status bar for preview state and project problems (idea 79)"
```

---

### Task 11: Idea 72, document colour provider

**Files:**
- Modify: `crates/server/src/lsp.rs` (capabilities, dispatch), create `crates/server/src/lsp_color.rs`
- Modify: `crates/server/src/main.rs` (`mod lsp_color;`)
- Modify: `editor/vscode/src/client.ts` (document selector at `:62`)
- Modify: `docs/internals/extending.tmd` (capability row)
- Test: `crates/server/src/lsp_color.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pub(crate) fn colors(text: &str) -> Vec<(u32, u32, u32, [f32; 4])>` returning `(line, start_col, end_col, [r, g, b, a])` in 0.0 to 1.0 components, and `pub(crate) fn presentation(color: [f32; 4]) -> String` returning the hex spelling.

**Selector change:** the client currently binds only `language: "taliesin"`. `_site.yml` is where the tokens are authored, so add `{ scheme: "file", pattern: "**/_site.yml" }`. **The server must answer every other request with an empty result for a YAML document** rather than trying to render it as `.tmd`; verify that before shipping, since a YAML file reaching `render_buffer` is a new input shape.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hex_token_value_is_offered_as_a_swatch() {
        let text = "theme:\n  --tali-accent: #3b5bdb\n";
        let found = colors(text);
        assert_eq!(found.len(), 1);
        let (line, start, end, rgba) = found[0];
        assert_eq!((line, start, end), (1, 17, 24));
        assert!((rgba[0] - 0.231).abs() < 0.01, "r was {}", rgba[0]);
        assert_eq!(rgba[3], 1.0);
    }

    #[test]
    fn a_non_colour_custom_property_is_not_a_swatch() {
        // `--tali-maxw: 46rem` is a length. Offering a colour picker on it would let one
        // click replace a width with `#000000`.
        assert!(colors("theme:\n  --tali-maxw: 46rem\n").is_empty());
    }

    #[test]
    fn a_three_digit_hex_expands_correctly() {
        let (_, _, _, rgba) = colors("  --tali-x: #fff\n")[0];
        assert_eq!(rgba, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn a_property_that_is_not_ours_is_left_alone() {
        // Only `--tali-*` is Taliesin's vocabulary. Painting swatches on every CSS custom
        // property in a YAML file would claim ownership of things this tool does not own.
        assert!(colors("  --other-accent: #3b5bdb\n").is_empty());
    }

    #[test]
    fn a_presentation_round_trips_to_the_same_colour() {
        let hex = presentation([0.231, 0.357, 0.855, 1.0]);
        assert_eq!(hex, "#3b5bda", "got {hex}");
        assert!(!colors(&format!("  --tali-x: {hex}\n")).is_empty());
    }
}
```

Compute the expected value in the last test by hand from the rounding you implement; if it differs, fix the assertion to the true value rather than loosening it to a range.

- [ ] **Step 2: Run to verify they fail**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin lsp_color
```

Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`lsp_color.rs` scans line by line for `--tali-<name>:` followed by a value, accepts `#rgb` and `#rrggbb` only (a named colour or `oklch()` has no unambiguous picker round trip, so it is skipped, not guessed), and returns scalar column offsets. `presentation` formats back to `#rrggbb`.

In `lsp.rs`, advertise:

```rust
        // Swatches for `--tali-*` colour tokens in `_site.yml` and front matter. Narrow on
        // purpose: only our own token prefix, and only hex values, because a picker that
        // rewrote `46rem` into `#000000` would be worse than no picker.
        color_provider: Some(lsp_types::ColorProviderCapability::Simple(true)),
```

Dispatch `DocumentColor::METHOD` and `ColorPresentationRequest::METHOD` in `handle_request`, converting scalar columns to UTF-16 with `crate::lsp_pos::char_to_utf16` exactly as the other handlers do.

In `client.ts`, extend `documentSelector`:

```ts
      // `--tali-*` colour tokens are authored in `_site.yml`, so the colour provider needs to
      // see that file. Everything else the server answers is `.tmd`-only and returns empty for
      // a YAML document.
      { scheme: "file", pattern: "**/_site.yml" },
```

Add the capability row to `docs/internals/extending.tmd`:

```
| `color` | Swatches and a picker for `--tali-*` hex tokens in `_site.yml` and front matter. Hex only: a named colour or an `oklch()` value has no unambiguous round trip through a picker. |
```

- [ ] **Step 4: Confirm a YAML document does not break the other handlers**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin lsp:: 2>&1 | tail -15
```

Then add one test asserting that `_site.yml` content produces no diagnostics and no panic through the publish path, since that path is now reachable for a non-`.tmd` buffer for the first time.

- [ ] **Step 5: Run everything and verify it passes**

```bash
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin lsp_color
CARGO_BUILD_JOBS=4 cargo test -p taliesin-server --bin taliesin the_internals_capability_table
```

Expected: PASS on both.

- [ ] **Step 6: Verify by mutation**

Remove the `--tali-` prefix check so every custom property is scanned. Re-run. Expected: `a_property_that_is_not_ours_is_left_alone` FAILS. Restore by inverse edit.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/lsp_color.rs crates/server/src/lsp.rs crates/server/src/main.rs editor/vscode/src/client.ts docs/internals/extending.tmd
git commit -m "feat(lsp): colour swatches for --tali-* tokens (idea 72)"
```

---

### Task 12: Idea 85a, language model tools at the current floor

**Files:**
- Create: `editor/vscode/src/lmtools.ts`
- Modify: `editor/vscode/src/extension.ts`, `editor/vscode/package.json` (`contributes.languageModelTools`)
- Test: create `editor/vscode/src/test/lmtools.test.ts`

**Interfaces:**
- Consumes: the existing `taliesin mcp` tool set, which is already built: `check`, `read`, `symbols`, `map`, `vocab`, `build` (`crates/server/src/mcp.rs:56-81`).
- Produces: `export function registerLmTools(context: vscode.ExtensionContext): void` and the pure `export const LM_TOOLS: { name: string; cli: string[] }[]`.

**`LanguageModelTool` exists at the pinned `@types/vscode@1.97.0`** (measured: 43 occurrences), so this half needs no floor change. Task 13 owns the bump.

- [ ] **Step 1: Write the failing tests**

```ts
import { test } from "node:test";
import assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import { LM_TOOLS } from "../lmtools";

const EXT_ROOT = path.join(__dirname, "..", "..");
const manifest = JSON.parse(fs.readFileSync(path.join(EXT_ROOT, "package.json"), "utf8"));

test("every registered tool is declared in the manifest", () => {
  const declared = new Set(
    (manifest.contributes.languageModelTools ?? []).map((t: { name: string }) => t.name)
  );
  for (const tool of LM_TOOLS) {
    assert.ok(declared.has(tool.name), `${tool.name} is registered in code but not declared`);
  }
});

test("every declared tool is registered in code", () => {
  // The other direction. A manifest entry with no implementation is a tool the model can
  // call and get nothing from, which is worse than not offering it.
  const registered = new Set(LM_TOOLS.map((t) => t.name));
  for (const t of manifest.contributes.languageModelTools ?? []) {
    assert.ok(registered.has(t.name), `${t.name} is declared but never registered`);
  }
});

test("the tool set matches what the MCP server actually exposes", () => {
  // Drift gate against the Rust side: `mcp.rs` is the owner of this list.
  const rust = fs.readFileSync(
    path.join(EXT_ROOT, "..", "..", "crates/server/src/mcp.rs"), "utf8"
  );
  const exposed = [...rust.matchAll(/name:\s*"([a-z_]+)"/g)].map((m) => m[1]);
  for (const tool of LM_TOOLS) {
    const sub = tool.cli[0];
    assert.ok(exposed.includes(sub), `${sub} is offered here but mcp.rs does not expose it`);
  }
});

test("no tool shells out to a subcommand that writes", () => {
  // `build` writes `_site/`. A model calling it unprompted is a surprise write, so the LM
  // surface is read-only even though the MCP server offers more.
  for (const tool of LM_TOOLS) {
    assert.ok(!tool.cli.includes("build"), `${tool.name} would write to disk`);
    assert.ok(!tool.cli.includes("publish"), `${tool.name} would publish`);
  }
});
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd editor/vscode && npm run compile && node --test out/test/lmtools.test.js
```

Expected: FAIL, module not found.

- [ ] **Step 3: Implement**

`LM_TOOLS` lists the four **read-only** tools:

```ts
export const LM_TOOLS = [
  { name: "taliesin_check", cli: ["check"] },
  { name: "taliesin_read", cli: ["read"] },
  { name: "taliesin_symbols", cli: ["symbols"] },
  { name: "taliesin_map", cli: ["map"] },
];
```

`build` is deliberately excluded, which the fourth test pins. `vocab` may be added if `mcp.rs` still exposes it; the third test will tell you.

`registerLmTools` calls `vscode.lm.registerTool(name, { invoke })` for each, where `invoke` spawns the configured binary with `[...cli, target, "--format", "json"]` and returns a `vscode.LanguageModelToolResult` with one `LanguageModelTextPart`.

Declare each in `package.json` under `contributes.languageModelTools` with `name`, `displayName`, `modelDescription`, `canBeReferencedInPrompt: true`, `toolReferenceName`, and an `inputSchema`.

- [ ] **Step 4: Run to verify they pass**

```bash
cd editor/vscode && npm run compile && node --test out/test/lmtools.test.js
cd editor/vscode && npx -y -p typescript tsc -p . --noEmit
```

Expected: 4 tests pass, `tsc` clean at the pinned 1.97.0 types.

- [ ] **Step 5: Verify by mutation**

Add `{ name: "taliesin_build", cli: ["build"] }` to `LM_TOOLS`. Re-run. Expected: two tests FAIL (the manifest-declaration gate and the writes gate). Restore by inverse edit.

- [ ] **Step 6: Commit**

```bash
git add editor/vscode/src/lmtools.ts editor/vscode/src/test/lmtools.test.ts editor/vscode/src/extension.ts editor/vscode/package.json
git commit -m "feat(companion): register taliesin's read-only tools as VS Code LM tools (idea 85a)"
```

---

### Task 13: Idea 85b, MCP server definition provider behind the floor bump

**Files:**
- Modify: `editor/vscode/package.json` (`engines.vscode`, `devDependencies.@types/vscode`, `contributes.mcpServerDefinitionProviders`)
- Modify: `editor/vscode/src/lmtools.ts` (add the provider)
- Modify: `editor/vscode/src/test/manifest.test.ts:482-492` (the engines pin test)

**Interfaces:**
- Consumes: nothing from earlier tasks beyond `lmtools.ts`.
- Produces: `export function registerMcpProvider(context: vscode.ExtensionContext): void`.

**Do this task last.** It changes a global constraint, so a failure here must not be able to block anything above it.

- [ ] **Step 1: Update the engines pin test first**

Rewrite the assertions in `editor/vscode/src/test/manifest.test.ts:482-492`:

```ts
test("the declared engine is new enough for the APIs we call, and the types pin that floor", () => {
  const engine = manifest.engines.vscode;
  const types = manifest.devDependencies["@types/vscode"];
  // Measured, not recalled: `registerMcpServerDefinitionProvider` and
  // `McpStdioServerDefinition` are absent from stable @types/vscode 1.100.0 and present in
  // 1.101.0, so 1.101 is the real floor once the MCP provider exists. (The previous floor,
  // 1.97, was set the same way by the paste API.)
  assert.strictEqual(engine, "^1.101.0", "engines.vscode must state the MCP provider floor");
  assert.strictEqual(
    types,
    "1.101.0",
    "@types/vscode must be pinned to the engine floor, with no caret: a range resolves to the " +
      "latest types and lets tsc bless APIs the minimum engine does not have"
  );
});
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd editor/vscode && npm run compile && node --test out/test/manifest.test.js
```

Expected: FAIL, `Expected values to be strictly equal: '^1.97.0' !== '^1.101.0'`.

- [ ] **Step 3: Move both fields and reinstall**

```bash
cd editor/vscode
python3 - <<'PY'
import json
p = json.load(open("package.json"))
p["engines"]["vscode"] = "^1.101.0"
p["devDependencies"]["@types/vscode"] = "1.101.0"
json.dump(p, open("package.json", "w"), indent=2)
open("package.json", "a").write("\n")
PY
npm install
python3 -c "import json;print(json.load(open('node_modules/@types/vscode/package.json'))['version'])"
```

Expected: prints `1.101.0`. If it prints anything else, stop: the pin did not take and every later `tsc` result is meaningless.

- [ ] **Step 4: Add the provider**

In `lmtools.ts`:

```ts
/**
 * Advertise `taliesin mcp` to VS Code, so the MCP server the project already ships is
 * discovered instead of hand-registered in user config.
 *
 * Needs VS Code 1.101: `registerMcpServerDefinitionProvider` is absent from the 1.100 API
 * surface. That measurement is what set the engine floor, and `manifest.test.ts` pins it.
 */
export function registerMcpProvider(context: vscode.ExtensionContext): void {
  const binary = vscode.workspace.getConfiguration("taliesin").get<string>("path", "taliesin");
  const didChange = new vscode.EventEmitter<void>();
  context.subscriptions.push(
    didChange,
    vscode.lm.registerMcpServerDefinitionProvider("taliesin", {
      onDidChangeMcpServerDefinitions: didChange.event,
      provideMcpServerDefinitions: async () => [
        new vscode.McpStdioServerDefinition("Taliesin", binary, ["mcp"]),
      ],
    })
  );
}
```

Declare it in `package.json`:

```json
"mcpServerDefinitionProviders": [
  { "id": "taliesin", "label": "Taliesin" }
]
```

Call `registerMcpProvider(context)` from `extension.ts`.

- [ ] **Step 5: Run to verify everything passes**

```bash
cd editor/vscode && npm run compile && node --test out/test/manifest.test.js
cd editor/vscode && npx -y -p typescript tsc -p . --noEmit
```

Expected: manifest tests pass; `tsc` clean **against the 1.101.0 types**, which is the point of the pin.

- [ ] **Step 6: Verify by mutation**

Set `devDependencies["@types/vscode"]` to `"^1.101.0"` (caret) and re-run the manifest test. Expected: FAIL with the no-caret message. Restore to the exact pin and re-run `npm install`.

- [ ] **Step 7: Commit**

```bash
git add editor/vscode/package.json editor/vscode/package-lock.json editor/vscode/src/lmtools.ts editor/vscode/src/extension.ts editor/vscode/src/test/manifest.test.ts
git commit -m "feat(companion): advertise taliesin mcp to VS Code, floor to 1.101 (idea 85b)"
```

---

### Task 14: Close the scope, in the repo's own records

**Files:**
- Modify: `notes/FEATURE-IDEAS.md` (Session 3 entries 67, 72, 74, 75, 76, 77, 78, 79, 80, 83, 85)
- Modify: `notes/backlog.md` ("Now" section)
- Modify: `notes/DETECTION-DEBT.md` (two new rows)
- Modify: `docs/guide/using/preview.tmd` (what an author sees)
- Test: the full gate suite

**Interfaces:** none. This is the record-keeping the project's own rules require, and the standing rule is explicit: **delete an item when it lands, never leave a `[x]`.**

- [ ] **Step 1: Mark the shipped ideas and record the three cuts**

In `notes/FEATURE-IDEAS.md` Session 3, prefix 72, 75, 76, 77, 78, 79, 80 and 85 with `SHIPPED 2026-07-30.` in the style ideas 68-71 and 82/84 already use. For the three cuts, replace the entry body with the reason, so it is not re-proposed:

- **67**: cut because idea 75 removed its last justification. Once go-to-definition resolves across pages, "locally defined versus defined elsewhere" is no longer information the author needs painted into the buffer. Note that its math-delimiter parenthetical was already obsolete.
- **74**: cut, not deferred. Every surface it was to enable fires on a gesture, not a keystroke, so a walk suffices; `lsp_project.rs` is a stat-validated memo with no watcher and no invalidation protocol. Record that this is the second time the "re-cost against 74" rule paid out, after idea 84.
- **83**: cut because its premise was rot. `client.js:1833` and `:1879` already navigate the standalone browser to `vscode://file<abs>:<line>:<col>`.

Also correct the Cluster C preamble, which currently reads "Every item here requires closing fact 1 and fact 2 first". That is now false.

- [ ] **Step 2: Update the backlog**

In `notes/backlog.md`, add one "Now" entry for the batch, carrying only what a later session needs: the measured MCP floor, the walk-not-index re-costing paying out a second time, and the containment-not-equality shape of the scanner-agreement pin. Delete nothing else; none of these ideas had a numbered backlog item except through 175(d), which stays.

- [ ] **Step 3: File the two detection-debt rows**

In `notes/DETECTION-DEBT.md`, following the existing row format:

1. **Cell cache and execution state are invisible to the editor.** The `⚡ fully cached` and never-executed-cell badges of idea 78 need freeze-key machinery from `exec`; the LSP and the decoration provider are both kernel-free by design. Nothing today would detect a wrong badge, because no badge is emitted.
2. **Live kernel state is unobservable from the extension host.** The webview relay carries four message types on purpose, and none reports kernel liveness, so the status bar cannot show it and no test can assert it.

- [ ] **Step 4: Document what the author sees**

In `docs/guide/using/preview.tmd`, extend the capability section with the sidebar's three views, Ctrl+T across the book, the Explorer badges (and the `taliesin.explorerBadges` setting that turns them off), the tasks now offered in the Run Task list, and the colour swatches in `_site.yml`.

- [ ] **Step 5: Run every gate**

```bash
cd /home/bogo/Documents/personal/taliesin
CARGO_BUILD_JOBS=4 ./tools/gates.sh 2>&1 | tail -40
```

This is the only invocation that proves the interpreter-gated suites actually ran rather than skipping silently. **Read its output; do not grep for the absence of a failure string.** Then the two type-checks and the companion suites:

```bash
cd web-client && npx -y -p typescript tsc -p jsconfig.json
cd crates/core/assets/js && npx -y -p typescript tsc -p jsconfig.json
cd editor/vscode && npm test && npm run test:e2e
```

The e2e suite is **load-sensitive**: two list-continuation tests fail at load ~6-7 on `main` as well as on a branch. If they fail, run an alternating baseline/branch pair at low load before treating it as a regression.

- [ ] **Step 6: Confirm the shipped extension actually works**

A green unit suite does not prove VS Code accepted a provider, and a stale `.vsix` has silently shipped a fixed bug before. Package, install, reload, and check by hand: the sidebar's three views populate on `docs/guide`, Ctrl+T finds a heading from another chapter, F12 on a cross-page `@sec-` jumps to the right file, the Explorer shows a badge, and Run Task lists the three Taliesin tasks.

- [ ] **Step 7: Commit**

```bash
git add notes/ docs/guide/using/preview.tmd
git commit -m "docs: close the editor scope in the ideas file, backlog and detection debt"
```

---

## Self-Review

**Spec coverage.** Substrate 3.1 → Task 3; 3.1.1 duplicate root finder → Task 2; 3.1.2 `xref_occurrences` → Task 1; idea 75 → Task 4; 76 → Task 5; 72 → Task 11; the two custom requests → Task 6; 77 → Task 7; 80 → Task 8; 78 → Task 9; 79 → Task 10; 85a → Task 12; 85b and the floor bump → Task 13; the three cuts, detection debt and docs → Task 14. Spec section 4's capability-table gate is exercised in Tasks 5 and 11; `manifest.test.ts` in Task 13; the memo invalidation pin in Task 3; the scanner-agreement pin in Task 1; mutation verification in every task.

**Type consistency.** `ProjectCache::get(&Path) -> Option<&ProjectScan>` is defined in Task 3 and consumed with that exact signature in Tasks 4, 5 and 6. `xref_occurrences(&str) -> Vec<(String, u32, u32)>` is defined in Task 1 and consumed in Task 3's `walk`. The `taliesin/projectOutline` and `taliesin/projectRefs` payloads defined in Task 6 match the `OutlineReply` and `RefsReply` shapes consumed in Task 7, field for field (`root`, `pages[].path`, `pages[].headings[].{line,level,text}`, `floats[].{id,path,line,title,number}`; `targets[].{id,resolved,definedIn,definedLine,uses[].{path,line,col}}`). `LM_TOOLS` is defined in Task 12 and extended in Task 13.

**One known ordering hazard.** Task 4 changes the signatures of `resolve_definition`, `resolve_hover` and `handle_request`. Tasks 5, 6 and 11 all add dispatch arms that take `cache`. **Do not run Tasks 5, 6 or 11 before Task 4**, or the `cache` parameter will not exist yet.
