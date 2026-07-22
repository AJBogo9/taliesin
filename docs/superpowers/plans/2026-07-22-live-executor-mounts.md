# Live-executor mounts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a mounted sub-project serve through the same live path as the root site, so its `{python}`/`{r}` cells execute live in `taliesin preview <site>` under the mount's `/prefix/` (F-04 full fix).

**Architecture:** Extract the per-project live machinery (`site` + `pages`) into one `Project` struct; the root site and each mount become `Arc<Project>`. One builder task holds one `ExecPool` per project (each with its own `_freeze` + interpreters); `exec_pool.rs` is used verbatim. The router resolves the project by URL prefix, then runs the existing per-page path against it. The watcher watches every project root.

**Tech Stack:** Rust (edition 2024), axum, tokio, `notify` file watcher, ZMQ Jupyter kernels. Bin crate `taliesin-server`, module `crates/server/src/serve_site/mod.rs` (+ `exec_pool.rs`).

## Global Constraints

- **Do-NOT-touch freeze:** `crates/server/src/serve_site/exec_pool.rs` — `MAX_WARM_PAGES` (=6) and the LRU eviction order. The file is **used, never edited**. One `ExecPool` instance per project is additive use, not a retune.
- **Single editing surface:** the preview is read-only; nothing here writes back to source.
- **Offline / no-CDN / no new runtime deps** where avoidable; `--tali-*` tokens only for any CSS.
- **Rust edition 2024, workspace resolver 3.** A `PostToolUse` hook runs `rustfmt` on every edited `.rs`; CI enforces `cargo fmt` + `clippy -D warnings`.
- **Three test gates (CI sets all three):** `TALIESIN_REQUIRE_NODE=1`; `TALIESIN_R=R TALIESIN_REQUIRE_R=1`; `TALIESIN_PYTHON=<venv> TALIESIN_REQUIRE_KERNEL=1`. Interpreter for local runs: `~/.local/share/qmd-venv/bin/python` (ipykernel 7.3.0).
- **Interpreter identity for warm-pool matching:** compare `interpreter::Resolved.path` (a `PathBuf`).
- **Verify by mutation; commit per task; do NOT push (author pushes); do NOT move `main`.** Work stays on branch `worktree-live-executor-mounts`.
- **`cargo test` aborts remaining binaries at the first failure; re-run before trusting a total. If an `exec` probe test flakes, `--test-threads=1` before blaming the change.**

## File structure

- `crates/server/src/serve_site/mod.rs` — the whole change. Introduces `Project`, `ProjectKey`, `MountPoint`, `resolve_project`, `classify_change`; reshapes `SiteApp`, `BuildMsg`, `spawn_builder`, `build_page(_guarded)`, `page_or_asset`, `ws_handler`/`client_conn`, `og_card_preview`, `spawn_watcher`. In-module `#[cfg(test)]` unit tests for the two pure helpers.
- `crates/server/src/serve_site/exec_pool.rs` — **unchanged** (constructed once per project by the builder).
- Browser-verify only (no new corpus doc; the existing `corpus/course` mount is the target).

## Interface summary (locked here, referenced by tasks)

```rust
// key "" = root; else the mount prefix, e.g. "gallery/course"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(super) struct ProjectKey(pub(super) String);

pub(super) struct Project {
    key: ProjectKey,
    root: PathBuf,
    site: Mutex<Site>,
    pages: Mutex<HashMap<String, PageState>>,
}

struct MountPoint { prefix: String, project: Arc<Project> }

// Longest-prefix match. Returns the owning project + the path with the prefix stripped.
fn resolve_project<'a>(app: &'a SiteApp, path: &'a str) -> (&'a Arc<Project>, &'a str);

// Pure: map a changed file's rel-from-a-project-root to (key, page-rel), or None.
// Used by the watcher; unit-tested without I/O.
fn classify_change(projects: &[(ProjectKey, &Site)], abs: &Path, roots: &[(ProjectKey, PathBuf)]) -> Option<(ProjectKey, String)>;

enum BuildMsg { Build(ProjectKey, String), Restart(ProjectKey, String) }
```

---

### Task 1: `ProjectKey` + pure `resolve_project` prefix resolver

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs` (add types + fn + in-module test)

**Interfaces:**
- Produces: `ProjectKey(String)`; `fn project_for<'a>(root: &'a Arc<Project>, mounts: &'a [MountPoint], path: &'a str) -> (&'a Arc<Project>, &'a str)` — a pure form taking the parts (so it is testable without a full `SiteApp`). `resolve_project(app, path)` is a thin wrapper used in later tasks.

- [ ] **Step 1: Write the failing test.** Add near the bottom of `mod.rs`, inside a new `#[cfg(test)] mod project_tests { use super::*; ... }` (or the existing test module if present):

```rust
#[cfg(test)]
mod project_tests {
    use super::*;

    // Build a bare Project with just a key + root (site/pages unused by the resolver).
    fn proj(key: &str, root: &str) -> Arc<Project> {
        Arc::new(Project {
            key: ProjectKey(key.to_string()),
            root: PathBuf::from(root),
            site: Mutex::new(Site::default()),
            pages: Mutex::new(HashMap::new()),
        })
    }

    #[test]
    fn prefix_resolution_picks_the_owning_project() {
        let root = proj("", "/site");
        let mounts = vec![
            MountPoint { prefix: "gallery/course".into(), project: proj("gallery/course", "/c") },
            MountPoint { prefix: "docs/guide".into(), project: proj("docs/guide", "/g") },
        ];
        // Unprefixed → root, path unchanged.
        let (p, sub) = project_for(&root, &mounts, "features.html");
        assert_eq!(p.key.0, ""); assert_eq!(sub, "features.html");
        // Exact prefix (the mount landing) → mount, empty sub-path.
        let (p, sub) = project_for(&root, &mounts, "gallery/course");
        assert_eq!(p.key.0, "gallery/course"); assert_eq!(sub, "");
        // Nested under a prefix → mount, prefix stripped (leading slash gone).
        let (p, sub) = project_for(&root, &mounts, "gallery/course/em.html");
        assert_eq!(p.key.0, "gallery/course"); assert_eq!(sub, "em.html");
        // A path that only shares a leading segment but not the full prefix → root.
        let (p, sub) = project_for(&root, &mounts, "gallery/other.html");
        assert_eq!(p.key.0, ""); assert_eq!(sub, "gallery/other.html");
    }
}
```

- [ ] **Step 2: Run it, verify it fails to compile** (types/fn absent).

Run: `cargo test -p taliesin-server --bin taliesin project_tests 2>&1 | tail -20`
Expected: FAIL — `cannot find type Project`/`function project_for`.

- [ ] **Step 3: Add the types + resolver.** Place the `ProjectKey`/`Project`/`MountPoint` definitions near the current `MountedSite` (mod.rs:55-60) and the resolver above `page_or_asset`:

```rust
/// A servable project: the root site (`key == ""`) or a mounted sub-project
/// (`key == its /prefix/`). Owns the per-project live state the builder + router act on.
pub(super) struct Project {
    key: ProjectKey,
    root: PathBuf,
    site: Mutex<Site>,
    pages: Mutex<HashMap<String, PageState>>,
}

/// A mounted sub-project served under `prefix` (e.g. `gallery/course`).
struct MountPoint {
    prefix: String,
    project: Arc<Project>,
}

/// `""` = the root project; otherwise the mount prefix.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(super) struct ProjectKey(pub(super) String);

/// Longest-prefix match of `path` against the mounts; returns the owning project and
/// `path` with the mount prefix (and its trailing `/`) stripped. Root when nothing matches.
fn project_for<'a>(
    root: &'a Arc<Project>,
    mounts: &'a [MountPoint],
    path: &'a str,
) -> (&'a Arc<Project>, &'a str) {
    let mut best: Option<&'a MountPoint> = None;
    for m in mounts {
        let hit = path == m.prefix || path.strip_prefix(&m.prefix).is_some_and(|r| r.starts_with('/'));
        if hit && best.is_none_or(|b| m.prefix.len() > b.prefix.len()) {
            best = Some(m);
        }
    }
    match best {
        Some(m) => {
            let sub = path.strip_prefix(&m.prefix).unwrap_or("");
            (&m.project, sub.strip_prefix('/').unwrap_or(sub))
        }
        None => (root, path),
    }
}

/// The [`SiteApp`] wrapper used by the router handlers.
fn resolve_project<'a>(app: &'a SiteApp, path: &'a str) -> (&'a Arc<Project>, &'a str) {
    project_for(&app.root, &app.mounts, path)
}
```

Note: `SiteApp` does not yet have `root`/`mounts: Vec<MountPoint>` — Task 2 reshapes it. For Task 1 the test calls `project_for` directly (not `resolve_project`), so it compiles against the types alone. Add `resolve_project` now but expect it unused until Task 4 (allow with `#[allow(dead_code)]` on `resolve_project` and on `Project` fields not yet read, removed as later tasks consume them).

- [ ] **Step 4: Run the test, verify it passes.**

Run: `cargo test -p taliesin-server --bin taliesin project_tests 2>&1 | tail -20`
Expected: PASS (1 test).

- [ ] **Step 5: Commit.**

```bash
git add crates/server/src/serve_site/mod.rs
git commit -m "feat(serve_site): ProjectKey + longest-prefix project resolver (unit-tested)"
```

---

### Task 2: Extract the `Project` struct (pure refactor, root only)

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs` — `SiteApp`, `serve()`, and every handler/builder/watcher reference to `app.site` / `app.pages` / `app.root`.

**Interfaces:**
- Consumes: `Project`, `MountPoint`, `ProjectKey` (Task 1).
- Produces: `SiteApp { root: Arc<Project>, mounts: Vec<MountPoint>, build_tx, loopback_bound }`. Every consumer now reaches state via a `&Project`.

This task has **no behavior change** — the safety net is "compiles + full suite green". Mounts stay statically served for now (the old `MountedSite` render path is preserved by keeping it working against `MountPoint.project.site` until Task 4 deletes it).

- [ ] **Step 1: Reshape `SiteApp`.** Replace the `site`/`pages`/`mounts` fields (mod.rs:40-53):

```rust
struct SiteApp {
    root: Arc<Project>,
    mounts: Vec<MountPoint>,
    build_tx: mpsc::UnboundedSender<BuildMsg>,
    loopback_bound: bool,
}
```

- [ ] **Step 2: Build the root `Project` + `MountPoint`s in `serve()`** (mod.rs:129-186). Replace the `mounts` discovery + `SiteApp` construction so each mount becomes an `Arc<Project>`:

```rust
let root_project = Arc::new(Project {
    key: ProjectKey(String::new()),
    root: root.clone(),
    site: Mutex::new(site),
    pages: Mutex::new(HashMap::new()),
});
let mounts: Vec<MountPoint> = root_project
    .site.lock().config.mounts.clone().into_iter()
    .filter_map(|m| {
        let mroot = root.join(&m.path);
        let mroot = mroot.canonicalize().unwrap_or(mroot);
        if !mroot.is_dir() {
            crate::log::warn(&format!("mount '{}': no directory at {}", m.at, mroot.display()));
            return None;
        }
        crate::log::watching(&mroot.display().to_string(), &format!("mounted at /{}/", m.at));
        let msite = Site::discover_with(&mroot, taliesin_core::DraftMode::Include);
        Some(MountPoint {
            prefix: m.at.clone(),
            project: Arc::new(Project {
                key: ProjectKey(m.at),
                root: mroot,
                site: Mutex::new(msite),
                pages: Mutex::new(HashMap::new()),
            }),
        })
    })
    .collect();
let page_count = root_project.site.lock().pages.len();
// ... keep the "page_count == 0 && mounts.is_empty()" guard ...
let app = Arc::new(SiteApp { root: root_project, mounts, build_tx, loopback_bound: !expose });
```

- [ ] **Step 3: Rewire the root handlers to `app.root`.** Mechanical, one call site at a time (each `app.site` → `app.root.site`, `app.pages` → `app.root.pages`, `&app.root` [PathBuf] → `&app.root.root`):
  - `og_card` (275): `app.site.lock()` → `app.root.site.lock()`.
  - `og_card_preview` (300): same. (Task 4 makes it project-aware.)
  - `search_index_js` (322) / `hover_index_js` (342): `app.site.lock()` → `app.root.site.lock()`.
  - `favicon` (266): no state — unchanged.
  - `page_or_asset` (360): the root page/deck/asset/404 branches use `app.root.site` / `app.root.root`; the mount loop still iterates `app.mounts` but now reads `m.project.site` (rename `m.site`→`m.project.site.lock()`, `m.root`→`m.project.root`, `m.at`→`m.prefix`). Keep it static for this task.
  - `ensure_and_render_page` / `render_markdown_only` / `site_page_html` (469-660): take/read `app.root.site` + `app.root.pages`. (They already take `&SiteApp`; switch internal field access.)
  - `ws_handler`/`client_conn` (728-808): `app.site`→`app.root.site`, `app.pages`→`app.root.pages`.
  - `spawn_builder`/`build_page(_guarded)`/`page_diagnostics` (856-1055): `app.site`→`app.root.site`, `app.pages`→`app.root.pages`, `app.root`(PathBuf join `_freeze`)→`app.root.root`.
  - `spawn_watcher` (1091+): `app.site`→`app.root.site`; the watch tree is still `watch_tree(&app.root.root)` for now (Task 5 adds mounts).
  - `render_404_page` call in `page_or_asset` (458): `app.root.site.lock()`.

- [ ] **Step 4: Verify it compiles + the full suite is green** (no behavior change).

Run: `cargo build -p taliesin-server 2>&1 | tail -5 && cargo test -p taliesin-core 2>&1 | tail -8 && cargo test -p taliesin-server --bin taliesin 2>&1 | tail -8`
Expected: builds clean; both suites PASS (same counts as before the task).

- [ ] **Step 5: Commit.**

```bash
git add crates/server/src/serve_site/mod.rs
git commit -m "refactor(serve_site): extract Project struct; SiteApp holds root + mounts as Projects"
```

---

### Task 3: Per-project `ExecPool` + `BuildMsg(ProjectKey)` in the builder

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs` — `BuildMsg`, `spawn_builder`, `build_page`, `build_page_guarded`.

**Interfaces:**
- Consumes: `Project`, `ProjectKey`, `SiteApp.root`/`.mounts`.
- Produces: `BuildMsg::Build(ProjectKey, String)` / `::Restart(ProjectKey, String)`; `build_page(project: &Arc<Project>, rel: &str, pool: &mut ExecPool)`; a builder that owns `HashMap<ProjectKey, ExecPool>` and looks the project up by key.

Still no mount execution yet (only the root sends build messages until Task 4), so the suite stays green with unchanged root behavior.

- [ ] **Step 1: Widen `BuildMsg`** (mod.rs:64-67):

```rust
enum BuildMsg {
    Build(ProjectKey, String),
    Restart(ProjectKey, String),
}
```

- [ ] **Step 2: Add a project lookup on `SiteApp`:**

```rust
impl SiteApp {
    fn project(&self, key: &ProjectKey) -> Option<&Arc<Project>> {
        if key.0.is_empty() { return Some(&self.root); }
        self.mounts.iter().find(|m| m.prefix == key.0).map(|m| &m.project)
    }
}
```

- [ ] **Step 3: Rework `spawn_builder`** (mod.rs:856-894) to a per-project pool map. Resolve each project's interpreters from its own root; boot ONE warm pool for the root's Python and share it only with projects whose `python.path` matches:

```rust
fn spawn_builder(app: Arc<SiteApp>, mut build_rx: mpsc::UnboundedReceiver<BuildMsg>) {
    tokio::spawn(async move {
        // Resolve every project's interpreters from ITS OWN _site.yml/root.
        let mut specs: Vec<(ProjectKey, PathBuf, crate::interpreter::Resolved, crate::interpreter::Resolved)> = Vec::new();
        {
            let py = {
                let s = app.root.site.lock();
                crate::interpreter::resolve_python(s.config.python.as_deref(), &app.root.root)
            };
            let r = {
                let s = app.root.site.lock();
                crate::interpreter::resolve_r(s.config.r.as_deref(), &app.root.root)
            };
            specs.push((ProjectKey::default(), app.root.root.clone(), py, r));
            for m in &app.mounts {
                let (py, r) = {
                    let s = m.project.site.lock();
                    (
                        crate::interpreter::resolve_python(s.config.python.as_deref(), &m.project.root),
                        crate::interpreter::resolve_r(s.config.r.as_deref(), &m.project.root),
                    )
                };
                specs.push((ProjectKey(m.prefix.clone()), m.project.root.clone(), py, r));
            }
        }
        // One forkserver, for the root's Python; shared with matching-interpreter projects.
        let root_py = specs[0].2.clone();
        let warm_pool = crate::warm_pool::warm_pool_for_preview(&root_py).await;
        let mut pools: HashMap<ProjectKey, ExecPool> = HashMap::new();
        for (key, root, py, r) in specs {
            let wp = (py.path == root_py.path).then(|| warm_pool.clone()).flatten();
            pools.insert(key, ExecPool::new(root.join("_freeze"), wp, py, r));
        }
        while let Some(msg) = build_rx.recv().await {
            match msg {
                BuildMsg::Build(key, rel) => {
                    if let (Some(project), Some(pool)) = (app.project(&key), pools.get_mut(&key)) {
                        let project = project.clone();
                        build_page_guarded(&project, &rel, pool).await;
                    }
                }
                BuildMsg::Restart(key, rel) => {
                    if let (Some(project), Some(pool)) = (app.project(&key).cloned(), pools.get_mut(&key)) {
                        pool.restart(&rel);
                        build_page_guarded(&project, &rel, pool).await;
                        if let Some(ps) = project.pages.lock().get(&rel) {
                            let _ = ps.tx.send(protocol::reload());
                        }
                    }
                }
            }
        }
    });
}
```

- [ ] **Step 4: Change `build_page`/`build_page_guarded`/`page_diagnostics` to take `&Arc<Project>`** (mod.rs:900-1055). Replace `app: &SiteApp` with `project: &Arc<Project>`; inside, `app.site`→`project.site`, `app.pages`→`project.pages`, `app.site.lock().page(rel)` unchanged (now `project.site.lock().page(rel)`), `&base` work_dir unchanged. `pool.get(rel, &base)` unchanged. Everywhere `app.build_tx.send(BuildMsg::Build(rel))` in the page lifecycle becomes `app.build_tx.send(BuildMsg::Build(project_key.clone(), rel))` — but those callers are in `ensure_and_render_page`/`client_conn`, updated in Task 4. For THIS task, the only sender is the root; update `ensure_and_render_page` + `client_conn` + watcher to send `BuildMsg::Build(ProjectKey::default(), rel)` (root key) so it compiles and root behavior is identical.

- [ ] **Step 5: Verify build + suite green.**

Run: `cargo build -p taliesin-server 2>&1 | tail -5 && cargo test -p taliesin-server --bin taliesin 2>&1 | tail -8 && cargo test -p taliesin-core 2>&1 | tail -6`
Expected: clean build; suites PASS unchanged.

- [ ] **Step 6: Commit.**

```bash
git add crates/server/src/serve_site/mod.rs
git commit -m "refactor(serve_site): per-project ExecPool map; BuildMsg carries ProjectKey"
```

---

### Task 4: Route mount requests through the live per-page path

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs` — `page_or_asset`, `ws_handler`/`client_conn`, `og_card_preview`, `ensure_and_render_page`, `site_page_html`.

**Interfaces:**
- Consumes: `resolve_project`, per-project builder (Task 3).
- Produces: mounted pages that flow through `ensure_and_render_page` (live executor + ws), replacing the static `m.site.render_page` branch.

This is the task that flips on mount execution. Verified by browser (Task 6); the suite stays green (root unchanged; mount routing has no unit-reachable seam beyond `resolve_project`, already tested).

- [ ] **Step 1: Project-scope the page lifecycle fns.** Change `ensure_and_render_page(project: &Arc<Project>, page: &Page, build_tx: &mpsc::UnboundedSender<BuildMsg>)`, `render_markdown_only` (already takes `&Site`), and `site_page_html(project: &Arc<Project>, page, ...)`. In `ensure_and_render_page`, queue `build_tx.send(BuildMsg::Build(project.key.clone(), rel))`. In `site_page_html`, emit the preview client's `?page=` key as the **prefixed** page path (root: unchanged rel/url; mount: `"<prefix>/<url>"`) so the client's ws connects under the mount. Add a helper:

```rust
fn ws_page_key(project: &Project, page_url_or_rel: &str) -> String {
    if project.key.0.is_empty() { page_url_or_rel.to_string() }
    else { format!("{}/{}", project.key.0, page_url_or_rel) }
}
```

- [ ] **Step 2: Rewrite `page_or_asset`** (mod.rs:360-462) to resolve the project first, then run the existing root logic against it, and **delete** the `for m in &app.mounts { ... m.site.render_page ... }` static branch (399-453):

```rust
async fn page_or_asset(State(app): State<Arc<SiteApp>>, uri: axum::http::Uri) -> axum::response::Response {
    let path = uri.path().trim_start_matches('/');
    let (project, sub) = resolve_project(&app, path);
    // 1) a live page of this project
    let page = { project.site.lock().page(sub).cloned() };
    if let Some(page) = page {
        return Html(ensure_and_render_page(project, &page, &app.build_tx)).into_response();
    }
    // 2) a deck of this project (self-contained on the fly) — reuse the existing deck branch,
    //    reading project.site + project.root instead of app.site/app.root.
    // 3) route-served aggregates for this project:
    let lookup = if sub.is_empty() { "index.html" } else { sub };
    if lookup == "search-index.js" { /* project.site.search_index_json */ }
    if lookup == "hover-index.js"  { /* project.site.hover_index_json  */ }
    // 4) a static asset under this project's root
    let asset = serve_asset(&project.root, sub);
    if asset.status() == axum::http::StatusCode::NOT_FOUND {
        // 5) this project's own 404
        let html = { project.site.lock().render_404_page() };
        return (axum::http::StatusCode::NOT_FOUND, Html(html)).into_response();
    }
    asset
}
```

(Fold the existing deck-render code and the search/hover `<script>` bodies — currently duplicated between the root path and the mount branch — into this single project-scoped path. The mount branch's bespoke copies are deleted, since the root path now serves every project.)

- [ ] **Step 3: Project-scope `ws_handler`/`client_conn`** (mod.rs:728-808). Resolve the project from the `?page=` key, normalise to that project's rel, subscribe to `project.pages`, and send project-keyed build/restart:

```rust
async fn client_conn(socket: WebSocket, app: Arc<SiteApp>, page_key: String) {
    let (project, sub) = { let (p, s) = resolve_project(&app, &page_key); (p.clone(), s.to_string()) };
    let rel = {
        let site = project.site.lock();
        site.page(&sub).map(|p| p.rel.clone()).unwrap_or(sub)
    };
    // ... entry into project.pages (was app.pages) ...
    if created { let _ = app.build_tx.send(BuildMsg::Build(project.key.clone(), rel.clone())); }
    // ... restart branch: BuildMsg::Restart(project.key.clone(), rel.clone()) ...
}
```

- [ ] **Step 4: Project-scope `og_card_preview`** (mod.rs:300): resolve the project from `?page=`, read `project.site`.

- [ ] **Step 5: Verify build + suite green.**

Run: `cargo build -p taliesin-server 2>&1 | tail -5 && cargo test -p taliesin-server --bin taliesin 2>&1 | tail -8`
Expected: clean build; PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/server/src/serve_site/mod.rs
git commit -m "feat(serve_site): serve mounted pages through the live per-page path (executor + ws)"
```

---

### Task 5: Watch every project root + pure `classify_change`

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs` — `spawn_watcher`, add pure `classify_change` + its unit test.

**Interfaces:**
- Consumes: `Project`, `ProjectKey`.
- Produces: `fn classify_change(roots: &[(ProjectKey, PathBuf)], abs: &Path) -> Option<(ProjectKey, PathBuf)>` — the pure "which project + relative path does this changed file belong to" (longest-root match). The watcher then resolves the page rel via that project's `Site`.

- [ ] **Step 1: Write the failing unit test** (in the `project_tests` module from Task 1):

```rust
#[test]
fn classify_change_attributes_a_file_to_its_deepest_project_root() {
    let roots = vec![
        (ProjectKey(String::new()), PathBuf::from("/site")),
        (ProjectKey("gallery/course".into()), PathBuf::from("/site/../corpus/course")),
    ];
    // Canonicalise-free: use paths that are already prefix-clean for the test.
    let roots = vec![
        (ProjectKey(String::new()), PathBuf::from("/site")),
        (ProjectKey("gallery/course".into()), PathBuf::from("/corpus/course")),
    ];
    let _ = roots;
    let roots = [
        (ProjectKey("gallery/course".into()), PathBuf::from("/corpus/course")),
        (ProjectKey(String::new()), PathBuf::from("/site")),
    ];
    // A file under the mount root → the mount, rel from that root.
    assert_eq!(
        classify_change(&roots, Path::new("/corpus/course/em.tmd")),
        Some((ProjectKey("gallery/course".into()), PathBuf::from("em.tmd")))
    );
    // A file under the site root → root.
    assert_eq!(
        classify_change(&roots, Path::new("/site/features.tmd")),
        Some((ProjectKey(String::new()), PathBuf::from("features.tmd")))
    );
    // A file under neither → None.
    assert_eq!(classify_change(&roots, Path::new("/elsewhere/x.tmd")), None);
}
```

(Delete the scratch `let roots` lines when writing — keep only the final `let roots = [ ... ]`.)

- [ ] **Step 2: Run it, verify it fails** (`classify_change` absent).

Run: `cargo test -p taliesin-server --bin taliesin classify_change 2>&1 | tail -20`
Expected: FAIL — cannot find function `classify_change`.

- [ ] **Step 3: Implement `classify_change`** (pure, longest-root match):

```rust
/// Attribute a changed absolute path to the project whose root is its deepest ancestor,
/// returning that project's key + the path relative to its root. `None` if under no root.
fn classify_change(roots: &[(ProjectKey, PathBuf)], abs: &Path) -> Option<(ProjectKey, PathBuf)> {
    roots
        .iter()
        .filter_map(|(k, root)| abs.strip_prefix(root).ok().map(|rel| (k.clone(), rel.to_path_buf())))
        .max_by_key(|(k, _)| k.0.len().max(if k.0.is_empty() { 0 } else { k.0.len() }))
        // Deepest root wins: prefer the LONGEST matched root path, not the key length.
}
```

Correction — rank by matched **root path length**, not key length (a nested mount root is longer):

```rust
fn classify_change(roots: &[(ProjectKey, PathBuf)], abs: &Path) -> Option<(ProjectKey, PathBuf)> {
    roots
        .iter()
        .filter_map(|(k, root)| {
            abs.strip_prefix(root)
                .ok()
                .map(|rel| (root.as_os_str().len(), k.clone(), rel.to_path_buf()))
        })
        .max_by_key(|(len, _, _)| *len)
        .map(|(_, k, rel)| (k, rel))
}
```

- [ ] **Step 4: Run it, verify it passes.**

Run: `cargo test -p taliesin-server --bin taliesin classify_change 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Wire the watcher** (mod.rs:1091+). Build the `roots` vec (root + every mount root, canonicalised), extend the watch set to the union of `watch_tree` over all project roots, and on each event use `classify_change` to get `(key, rel_from_root)`, then look up the page rel in that project's `Site` and send `BuildMsg::Build(key, page_rel)`. Reuse the existing `.tmd`/`.md`/`.bib`/`.csl`/`.css` filtering per project; late-created subdir watches register under whichever root contains them.

- [ ] **Step 6: Verify build + suite green.**

Run: `cargo build -p taliesin-server 2>&1 | tail -5 && cargo test -p taliesin-server --bin taliesin 2>&1 | tail -8`
Expected: clean build; PASS (incl. the two new pure tests).

- [ ] **Step 7: Commit.**

```bash
git add crates/server/src/serve_site/mod.rs
git commit -m "feat(serve_site): watch every project root; pure classify_change (unit-tested)"
```

---

### Task 6: Browser-verify the mount executes live (the F-04 repro)

**Files:** none (verification only). Uses the existing `corpus/course` mount.

- [ ] **Step 1: Build the release binary** (assets are `include_str!`-compiled; a preview needs a current binary).

Run: `cargo build -p taliesin-server 2>&1 | tail -3`

- [ ] **Step 2: Serve the marketing site on a distinct port** (avoid the parallel session's ports; kill by PID afterward, never `pkill -f`).

Run (background): `TALIESIN_PYTHON=~/.local/share/qmd-venv/bin/python ./target/debug/taliesin preview site 4399 &` then record `$!`.

- [ ] **Step 3: Drive chrome-devtools MCP** to `http://127.0.0.1:4399/gallery/course/em.html`. Wait for the websocket exec pass. **Assert:** the `{python}` cell (`corpus/course/em.tmd:57`) shows its **executed output** (a rendered result/plot or value), NOT bare source and NOT a "kernel unavailable" notice. Capture `list_console_messages` → **0 errors**. Screenshot.

- [ ] **Step 4: Repeat at three viewports** (~390×844, ~1440×900, ~900×1440), light + dark. Confirm no horizontal overflow and the output persists.

- [ ] **Step 5: Hot-reload check.** Touch a trivial edit in `corpus/course/em.tmd` (e.g. a word in prose), confirm the mounted page live-updates without a manual refresh. Revert the edit.

- [ ] **Step 6: Restart-kernel check.** Open the mounted page's dev menu → "Restart kernel"; confirm the cell re-executes.

- [ ] **Step 7: Kill the server by PID** (`kill <recorded pid>`), not `pkill`.

- [ ] **Step 8: Record the verification** in the findings/notes (Task 7). No commit (verification only), unless a screenshot is worth attaching to the notes doc.

---

### Task 7: Finalize — gates, backlog/notes, wrap

**Files:**
- Modify: `notes/backlog.md` (item 16 F-04 → fixed; item 10 "mounts: live serve untested" note), the demand-probe program memory pointer is updated separately.

- [ ] **Step 1: Full gates green** (run the whole thing under the CI env, `--test-threads=1` if an exec probe flakes):

```bash
TALIESIN_REQUIRE_NODE=1 TALIESIN_R=R TALIESIN_REQUIRE_R=1 \
TALIESIN_PYTHON=~/.local/share/qmd-venv/bin/python TALIESIN_REQUIRE_KERNEL=1 \
cargo test -p taliesin-core -p taliesin-server 2>&1 | tail -20
cargo fmt --check && cargo clippy --workspace -- -D warnings 2>&1 | tail -5
```
Expected: all PASS; fmt clean; clippy clean.

- [ ] **Step 2: Confirm `exec_pool.rs` is byte-unchanged** (the freeze):

Run: `git diff --stat origin/main -- crates/server/src/serve_site/exec_pool.rs`
Expected: empty (no lines).

- [ ] **Step 3: Update `notes/backlog.md`:** mark item 16 **F-04 fixed** (mounted kernel cells now execute live in preview); note the live-executor-mount engine landed; and record that item 10's "`mounts:` live serve" now has live-exec coverage via browser-verify (the automated live-HTTP/exec test remains a filed follow-up given the bin-crate has no serve-test harness).

- [ ] **Step 4: Commit the notes.**

```bash
git add notes/backlog.md
git commit -m "docs(backlog): F-04 fixed (live-executor mounts); item 10 mount-serve note"
```

- [ ] **Step 5: Hand back to the author.** Summarize what landed, the `exec_pool.rs`-unchanged proof, the browser evidence, and the filed follow-up (automated live-exec test). Do NOT push; do NOT move `main`. Then proceed to sub-project 2 (analyst artifact) as a separate spec.

## Self-review

- **Spec coverage:** §3 Approach A → Tasks 1-5; §4 component model → Tasks 1-2; §5 executor/warm-pool → Task 3; §6 routing → Task 4; §7 watcher → Task 5; §8 error handling → reuses `build_page_guarded` (Task 3-4), missing-mount-dir warn preserved (Task 2 Step 2); §9 testing → Tasks 1/5 (unit) + Task 6 (browser) + follow-up note (Task 7); §10 success criteria → Tasks 6-7; §11 guardrails → Global Constraints + Task 7 Step 2. Covered.
- **Placeholder scan:** the `page_or_asset` rewrite (Task 4 Step 2) uses `/* ... */` to point at existing code blocks to fold in rather than reproduce ~60 lines verbatim; every NEW unit (types, resolver, builder, classify_change, tests) is shown in full. The deck-render fold is an existing block relocated, not new code.
- **Type consistency:** `ProjectKey(String)` (field `.0`), `Project { key, root, site, pages }`, `MountPoint { prefix, project }`, `BuildMsg::{Build,Restart}(ProjectKey, String)`, `project_for`/`resolve_project`, `classify_change(roots, abs)` — consistent across tasks.
- **Freeze:** `exec_pool.rs` never in a task's file list; Task 7 Step 2 asserts it byte-unchanged.
