# Live-executor mounts (F-04 full fix)

**Date:** 2026-07-22
**Status:** design approved; spec under review
**Sub-project 1 of 2** in the persona-4 (analyst) demand-probe work. Sub-project 2
(the `corpus/analyst/` artifact + `analyst.rs` pin + `/gallery/analyst` exhibit +
findings doc) is a separate spec, built + verified *after* this lands.

## 1. Context & motivation

A Taliesin **site** (`taliesin preview <dir>`) can `mounts:` other Taliesin projects
under a URL prefix, so a link to `/docs/guide` resolves in live preview, not only in a
static build. The marketing site uses this for the docs books and the `/gallery/<name>`
persona exhibits.

Today a mount is served **statically**. `serve_site::page_or_asset` (mod.rs:399-453)
answers a mounted-page request by calling `MountedSite::site.render_page(lookup)` — a
pre-execution render. Client-side `{js}` cells still run (the render emits the qmd-js
runtime), but a **kernel** cell (`{python}` / `{r}`) never executes in the mount: it
renders as bare source, with no output and no "kernel unavailable" notice, so it looks
like dead source. This is finding **F-04** from the course-author persona, re-confirmed
kernel-only by the interactive-explainer persona.

The static `build` is unaffected and correct: `build` does not wire mounts either (it
emits a per-mount "build it yourself with `--out`" warning), and the gallery's build
script runs exactly those per-mount builds, each executing its own cells at build time.
**F-04 is purely the live-preview gap**, and it is the natural prerequisite of the
persona-4 analyst report, which is heavy `{python}`+`{r}` and is exhibited as a mount.

## 2. Goal & non-goals

**Goal.** A mounted sub-project serves through the *same* live path as the root site:
each mounted page gets a per-page `Executor` + kernel, streams execution over its own
websocket, and hot-reloads on edits to the mount's source. A `{python}`/`{r}` cell in a
mount executes live in `preview` under its `/prefix/`, exactly as it would if the mount
were previewed standalone.

**Non-goals (out of scope for this sub-project):**

- **Static `build` wiring of mounts.** Unchanged. The gallery build script already
  builds each mount with `--out`; auto-wiring the static build is a separate backlog
  item. This work is **live preview only**.
- **Retuning `MAX_WARM_PAGES` or the LRU eviction order.** The standing freeze
  (`serve_site/exec_pool.rs`). `exec_pool.rs` is used unchanged; the file is not edited.
- **The single-editing-surface invariant.** Preview stays a read-only view; nothing here
  writes back to source.
- **A per-mount warm pool / forkserver.** A mount whose interpreter differs from the
  root cold-starts; only the root boots a forkserver. (See §5.)
- **The analyst artifact.** Sub-project 2.

## 3. Approach (A, of the three considered)

Three approaches were weighed (see the brainstorming record). **Approach A**, chosen:

- **A — per-project `ExecPool`, `exec_pool.rs` untouched (CHOSEN).** Keep one builder
  task, but hold one `ExecPool` per project (root + each mount). The frozen file is
  constructed once per project; its LRU logic is used verbatim, never edited.
- B — full `ProjectRuntime` extraction with a builder task + warm pool per project.
  Cleanest abstraction, largest churn on the 72 KB moat file, N tasks at shutdown.
  Rejected: more risk than the yield warrants.
- C — one shared pool keyed by `(project, rel)`. Rejected: forces per-entry
  `freeze_dir`/interpreters *inside* `exec_pool.rs`, i.e. editing the exact frozen logic
  the standing freeze says breaks silently and is not test-guarded.

## 4. Component model

Extract the per-project live machinery into one struct, so "root site" and "each mount"
are the same kind of thing:

```
Project {
    root:  PathBuf,
    site:  Mutex<Site>,
    pages: Mutex<HashMap<rel, PageState>>,   // per-project; no cross-mount key collision
    key:   ProjectKey,                        // "" = root; else the mount prefix (e.g. "gallery/course")
}

SiteApp {
    root:     Arc<Project>,
    mounts:   Vec<MountPoint { prefix: String, project: Arc<Project> }>,
    build_tx: mpsc::UnboundedSender<BuildMsg>,   // one channel → one builder task
    loopback_bound: bool,
}

enum BuildMsg { Build(ProjectKey, String), Restart(ProjectKey, String) }  // now carries the project
```

`PageState` / `PageDoc` are unchanged. `MountedSite { at, root, site }` is replaced by
`MountPoint { prefix, project: Arc<Project> }` (the `Site` moves inside `Project`).

The single builder task owns `HashMap<ProjectKey, ExecPool>` — one pool per project, each
constructed with **that project's own** `_freeze/` dir and resolved interpreters. This is
additive *use* of `exec_pool.rs`, not a change to it.

**`ProjectKey`** is the mount prefix string, `""` for the root. (A newtype over `String`
for clarity; ordering not required.)

## 5. Executor / kernel / warm pool

- The builder resolves each project's interpreters from **its own** `_site.yml`
  (`resolve_python` / `resolve_r` against the mount root — the same calls the root uses),
  and builds that project's `ExecPool::new(project.root.join("_freeze"), warm_pool_opt,
  py, r)`.
- **Warm pool:** the builder boots one forkserver for the **root's** Python, as today. A
  mount whose resolved Python **matches** the root's shares that warm pool; a mount with a
  **different** interpreter gets `warm_pool = None` and cold-starts. Matching is on the
  resolved interpreter identity already computed by `interpreter::Resolved`. For the
  gallery every project uses the default interpreter, so all mounts warm-start.
  *(Rationale: a forkserver per mount is heavy and low-yield; cold-start is correct and
  already the fallback whenever the pool is inert.)*
- `MAX_WARM_PAGES` is **per pool** and unchanged. Worst-case resident kernels rise to
  `(1 + N_mounts) × MAX_WARM_PAGES`, bounded in practice by lazy kernel spawn (a kernel is
  only started when a visited page actually has kernel cells). Documented, accepted.
- Teardown is unchanged in shape: the one builder task owns every pool + the warm pool and
  drops them on channel close (server shutdown), running the existing forkserver group-kill
  + kernel SIGKILLs.

## 6. Data flow / routing

The router handlers resolve the **project by URL prefix** first, then run the existing
per-page logic against that `Project` instead of the hard-coded root.

- **Prefix resolution.** A helper `resolve_project(app, path) -> (&Project, sub_path)`:
  longest-prefix match over `app.mounts` (a mounted page path starts with `"<prefix>/"`
  or equals `"<prefix>"`); no match → the root project, `sub_path == path`. Pure and
  unit-tested.
- **`page_or_asset`.** Resolve the project, strip the prefix, resolve the `Page` within
  that project's `Site`, and flow through the **same** `ensure_and_render_page` →
  `site_page_html` path the root uses. The static mount branch (mod.rs:399-453) that
  called `m.site.render_page(lookup)` is **deleted**; deck-embed, `search-index.js`,
  `hover-index.js`, asset, and 404 fallbacks are handled by the shared per-project path
  (each already exists for the root; they become project-scoped).
- **`site_page_html`** emits the preview client with a **project-qualified `?page=` key**
  (the full URL path, prefix included) so a mounted page's client connects to the right
  project's ws.
- **`ws_handler` / `client_conn`.** Resolve the project from the `?page=` key's prefix,
  normalise the key to that project's source `rel`, subscribe to *that* project's
  `PageState`, and send `Build`/`Restart` with the project key.
- **Route-served aggregates** (`/search-index.js`, `/hover-index.js`, `/favicon.ico`,
  `/og/{name}`) resolve the project by prefix (root when unprefixed) and read from that
  project's `Site`. The current mount-branch special-cases for search/hover indexes
  collapse into this shared path.

## 7. Watcher / hot-reload

`spawn_watcher` watches **each project's root** (root + every mount dir). A changed file
maps to `(ProjectKey, rel)` via the owning project's `Site::page`, and queues
`BuildMsg::Build(key, rel)`. A mount edit hot-reloads its mounted page exactly like a
root edit. The existing per-directory non-recursive watch model (and its late-created
subdir handling) is reused per project root; the watch set is the union over projects.

## 8. Error handling (reuses existing nets)

- **Per-page panic guard.** `build_page_guarded` already isolates one bad page from the
  shared builder; it becomes project-scoped (the guard + the error broadcast target the
  right project's `PageState`). One panicking mounted page cannot stop hot-reload for the
  root or other mounts.
- **Missing mount dir.** Already warned + skipped at discovery; unchanged.
- **Interpreter mismatch.** Cold-start, never a crash (§5).
- **Kernel-unavailable.** A mount with no kernel available shows the existing located
  "kernel unavailable" diagnostic on its own page — the very notice F-04 was missing —
  because the mount now runs the real per-page diagnostics path.

## 9. Testing

The server is a bin crate with no live-HTTP serve test today (backlog item 10 records the
gap). This work adds coverage at two levels, with a browser floor:

1. **Pure unit tests (guaranteed):**
   - `resolve_project`: root (unprefixed), exact prefix (`"gallery/course"`), nested
     (`"gallery/course/ch1.html"`), longest-prefix precedence, and a non-matching path →
     root.
   - watcher path → `(ProjectKey, rel)` mapping for a file under the root vs. under a
     mount dir.
   These are TDD'd (written failing first) and mutation-checked.
2. **Live-HTTP integration test (goal; kernel-gated):** spawn the site server on an
   ephemeral port, GET a mounted `{python}` page, wait for execution, and assert the
   executed output appears in the served/broadcast HTML (not bare source). Gated like
   `read_run.rs` (skips without a kernel; a `TALIESIN_REQUIRE_KERNEL` canary in CI).
   **This also closes backlog item 10's "`mounts:` live serve untested" gap.** If the
   bin-crate harness proves impractical (port binding, async lifecycle), the unit +
   browser floor stands and the live-HTTP test is filed as a follow-up rather than
   blocking the feature.
3. **Browser-verify (the acceptance floor; `preview` skill + chrome-devtools):**
   `taliesin preview site` on a distinct port → navigate to the course chapter `em.tmd`,
   whose `{python}` cell renders at `/gallery/course/em.html` → confirm the cell output
   renders **live** (not dead source), 0 console errors, across the viewport matrix
   (mobile ~390, laptop-landscape ~1440, laptop-portrait ~900). This is the exact F-04
   repro from the course-author findings doc. The **existing course mount** is the
   verification target, so this sub-project needs no new corpus doc.
4. **Regression:** the full `taliesin-core` + server suites stay green (fmt, clippy `-D`,
   the three test gates).

## 10. Deliverables & success criteria

1. A mounted `{python}`/`{r}` page **executes live** in `taliesin preview <site>`, with
   hot-reload on mount-source edits and a working "Restart kernel" for the mounted page.
2. `exec_pool.rs` is byte-unchanged; `MAX_WARM_PAGES` and the LRU order are untouched.
3. The root site's behavior is unchanged (root is just the `""`-prefix project); the full
   suite is green.
4. Unit tests for `resolve_project` + watcher mapping; the live-HTTP kernel-gated test
   (or a filed follow-up if impractical); browser-verified against the course mount.
5. Backlog item 10's "`mounts:` live serve untested" gap is closed if the live-HTTP test
   lands; item 16 F-04 / the program notes are updated to "fixed."

## 11. Guardrails (standing constraints this honors)

- **Do-NOT-touch freeze:** `exec_pool.rs` (`MAX_WARM_PAGES` + LRU) is used, not edited.
- **Single editing surface:** preview stays read-only.
- **Offline / no-CDN / `--tali-*`:** no asset or network change.
- **Verify by mutation; branch per feature; ff-merge locally; author pushes.**
- **Parallel session:** built in an isolated worktree off `origin/main`
  (`worktree-live-executor-mounts`); `main` is not moved.
