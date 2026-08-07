# A project is what `_site.yml` declares: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `build` and `preview` refuse a directory that has no `_site.yml`, and make a standalone document render without site chrome so `preview <file>` matches `build <file>`.

**Architecture:** One shared error builder in the CLI layer, called from the two directory dispatch sites. One `standalone` flag on `Site`, set only by `Site::discover_single` when the scoped file's directory has no `_site.yml`, read at the single place that assembles site chrome.

**Tech Stack:** Rust (edition 2024, workspace resolver 3), `cargo test`. No new dependencies.

Spec: [`docs/superpowers/specs/2026-08-08-project-vs-standalone-document-design.md`](../specs/2026-08-08-project-vs-standalone-document-design.md).

## Global Constraints

- Rust edition 2024. Shared deps live in the root `[workspace.dependencies]`. This change adds none.
- Every emitted block keeps `data-block-id` + `data-sourcepos`; `crates/core/tests/corpus.rs` enforces it. Nothing here touches block emission.
- Do NOT touch `MAX_WARM_PAGES` or the LRU order in `crates/server/src/serve_site/exec_pool.rs`. That is the one standing freeze.
- `taliesin lsp` uses stdout as the JSON-RPC wire. Never `println!` in server code; use `crate::log` (stderr).
- A `PostToolUse` hook runs `rustfmt` on every edited `.rs`, so the tree stays `cargo fmt`-clean.
- `check`, `map` and `features` must keep working on a bare directory. The `read` refusal points at `taliesin map <path>`, so breaking `map` breaks that guidance.
- `read` already refuses bare directories (`crates/server/src/query.rs`). Do not change it; match its tone.

---

### Task 1: The shared refusal message

One builder, called by both verbs, so the two messages cannot drift.

**Files:**
- Modify: `crates/server/src/serve/mod.rs` (add beside `guarded` / `unknown_flag_error` / `bad_format_error`, which live at lines 672-693)
- Test: `crates/server/src/serve/mod.rs` (in the existing `#[cfg(test)] mod tests`; add one if absent)

**Interfaces:**
- Consumes: `taliesin_core::site::enclosing_site_root(&Path) -> Option<PathBuf>` (already public; walks up from a directory, stops at a `.git` boundary, and checks the starting directory first).
- Produces: `pub(crate) fn not_a_project_error(path: &Path, verb: &str) -> String`. Task 2 and Task 3 both call it. `verb` is `"build"` or `"preview"` and appears in the suggested command.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/server/src/serve/mod.rs`:

```rust
#[test]
fn not_a_project_error_names_both_fixes() {
    let dir = std::path::Path::new("corpus/agent");
    let msg = not_a_project_error(dir, "preview");
    assert!(
        msg.contains("no _site.yml"),
        "names the missing file: {msg}"
    );
    assert!(
        msg.contains("_site.yml") && msg.contains("add"),
        "offers the make-it-a-project fix: {msg}"
    );
    assert!(
        msg.contains("taliesin preview corpus/agent/"),
        "offers the name-one-document fix, with the verb: {msg}"
    );
}

#[test]
fn not_a_project_error_leads_with_an_enclosing_project() {
    // corpus/tech-blog/posts has no _site.yml of its own, but corpus/tech-blog does.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/tech-blog/posts");
    let msg = not_a_project_error(&root, "build");
    assert!(
        msg.contains("tech-blog"),
        "names the ancestor project: {msg}"
    );
    assert!(
        msg.contains("did you mean"),
        "leads with the ancestor as the likely intent: {msg}"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p taliesin-server not_a_project_error
```

Expected: FAIL, `cannot find function not_a_project_error in this scope`.

- [ ] **Step 3: Write the implementation**

Add to `crates/server/src/serve/mod.rs`:

```rust
/// The one message both `build` and `preview` print when handed a directory that is not a
/// project. A directory is a project, and a project is what `_site.yml` declares; without
/// one there is no nav to build, no title to brand with, and no page to serve at `/`.
///
/// When an ancestor IS a project, that is nearly always what the author meant (running the
/// verb on `corpus/tech-blog/posts` silently built eight posts as a detached site), so the
/// suggestion leads with it instead of the generic pair.
pub(crate) fn not_a_project_error(path: &Path, verb: &str) -> String {
    let shown = path.display();
    if let Some(root) = taliesin_core::site::enclosing_site_root(path) {
        return format!(
            "{shown} has no _site.yml.\n\
             its ancestor {root} is a project. did you mean:\n  \
             taliesin {verb} {root}",
            root = root.display()
        );
    }
    // `join` rather than string concatenation, so the suggestion reads
    // `corpus/agent/<page>.tmd` whether or not the author typed a trailing slash.
    format!(
        "{shown} has no _site.yml, so it is not a project.\n\
         to {verb} one document:   taliesin {verb} {example}\n\
         to make it a site or book: add a _site.yml",
        example = path.join("<page>.tmd").display()
    )
}
```

`use std::path::Path;` is already in scope in this module; confirm before adding it.

- [ ] **Step 4: Run the tests to verify they pass**

```sh
cargo test -p taliesin-server not_a_project_error
```

Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```sh
git add crates/server/src/serve/mod.rs
git commit -m "feat(cli): one refusal message for a directory that is not a project"
```

---

### Task 2: `build <dir>` refuses

**Files:**
- Modify: `crates/server/src/build.rs` (inside the `if Path::new(path).is_dir() {` block that opens at line 268, beside the existing `--bare` and `--stdout` rejections)
- Create: `crates/server/tests/project_required.rs`

**Interfaces:**
- Consumes: `crate::serve::not_a_project_error(&Path, &str) -> String` from Task 1.
- Produces: nothing later tasks depend on. The test file is extended by Task 3.

- [ ] **Step 1: Write the failing test**

Create `crates/server/tests/project_required.rs`:

```rust
//! `build` and `preview` render a *project*, and a project is what `_site.yml` declares.
//! A bare directory is refused with guidance, the same stance `read` already takes
//! (`read_of_a_non_site_directory_is_rejected_with_guidance` in `read_book.rs`).

use std::process::Command;

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn build_of_a_non_project_directory_is_rejected_with_guidance() {
    let (ok, _out, stderr) = run(&["build", &corpus("agent")]);
    assert!(!ok, "a bare directory (no _site.yml) must fail");
    assert!(stderr.contains("no _site.yml"), "says why: {stderr}");
    assert!(
        stderr.contains("<page>.tmd"),
        "offers the name-one-document fix: {stderr}"
    );
    assert!(
        stderr.contains("add a _site.yml"),
        "offers the make-it-a-project fix: {stderr}"
    );
}

#[test]
fn build_of_a_subdirectory_of_a_project_names_the_project() {
    let (ok, _out, stderr) = run(&["build", &corpus("tech-blog/posts")]);
    assert!(!ok, "a project subdirectory is not itself a project");
    assert!(
        stderr.contains("tech-blog") && stderr.contains("did you mean"),
        "leads with the enclosing project: {stderr}"
    );
}

#[test]
fn build_of_a_real_project_still_works() {
    let (ok, _out, stderr) = run(&["build", &corpus("shared-bib"), "--no-exec"]);
    assert!(ok, "a directory WITH _site.yml still builds; stderr: {stderr}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p taliesin-server --test project_required
```

Expected: the first two FAIL (the build succeeds today and prints only a warning); the third PASSES.

- [ ] **Step 3: Write the implementation**

In `crates/server/src/build.rs`, inside the `if Path::new(path).is_dir() {` block, **before** the existing `--bare` check so the project error wins over a flag error:

```rust
        // A directory is a project, and a project is what `_site.yml` declares. Without one
        // there is nothing to build: no nav, no title, no page at `/`. This is the stance
        // `read` already takes (`query.rs`); `build` used to warn and synthesize a website.
        if !Path::new(path).join("_site.yml").is_file() {
            log::error(&crate::serve::not_a_project_error(Path::new(path), "build"));
            return ExitCode::FAILURE;
        }
```

- [ ] **Step 4: Run the tests to verify they pass**

```sh
cargo test -p taliesin-server --test project_required
```

Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```sh
git add crates/server/src/build.rs crates/server/tests/project_required.rs
git commit -m "feat(build): refuse a directory that has no _site.yml"
```

---

### Task 3: `preview <dir>` refuses

Same rule, same message, so the two verbs cannot answer differently.

**Files:**
- Modify: `crates/server/src/serve_site/mod.rs` (`fn resolve_target(target: Target) -> std::io::Result<Resolved>`, which opens at line 419; the `Target::Project` path reaches `Site::discover_with(&root, …)` near line 453)
- Test: `crates/server/tests/project_required.rs` (extend)

**Interfaces:**
- Consumes: `crate::serve::not_a_project_error(&Path, &str) -> String` from Task 1.
- Produces: nothing later tasks depend on.

- [ ] **Step 1: Write the failing test**

Append to `crates/server/tests/project_required.rs`:

```rust
#[test]
fn preview_of_a_non_project_directory_is_rejected_with_guidance() {
    // Must fail before binding a port, so this returns rather than serving forever.
    let (ok, _out, stderr) = run(&["preview", &corpus("agent"), "4399"]);
    assert!(!ok, "a bare directory (no _site.yml) must fail");
    assert!(stderr.contains("no _site.yml"), "says why: {stderr}");
    assert!(
        stderr.contains("<page>.tmd"),
        "offers the name-one-document fix: {stderr}"
    );
}

```

Then add the authoritative pair as **unit** tests in the existing `#[cfg(test)] mod tests`
at the bottom of `crates/server/src/serve_site/mod.rs`, beside
`an_out_of_project_document_keys_on_itself_on_both_sides`. They use that module's existing
`tmp(name)` helper, and they test `resolve_target` directly rather than through a verb:

```rust
    /// A lone document with no ancestor `_site.yml` is legitimate and must keep resolving.
    /// Only the *directory* form is refused.
    #[test]
    fn a_loose_document_still_resolves() {
        let dir = tmp("loose-doc");
        let doc = dir.join("scratch.tmd");
        std::fs::write(&doc, "---\ntitle: S\n---\n\nProse.\n").unwrap();
        assert!(
            resolve_target(Target::at(doc)).is_ok(),
            "a lone document is not a project and needs none"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A directory with no `_site.yml` is not a project, and is refused before a port is bound.
    #[test]
    fn a_directory_without_site_yml_is_refused() {
        let dir = tmp("not-a-project");
        std::fs::write(dir.join("a.tmd"), "---\ntitle: A\n---\n\nProse.\n").unwrap();
        let err = resolve_target(Target::at(dir.clone())).expect_err("not a project");
        assert!(err.to_string().contains("no _site.yml"), "says why: {err}");
        assert!(err.to_string().contains("<page>.tmd"), "offers the fix: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
```

These two are the authoritative guard: they run in milliseconds and cannot hang. The
CLI-level test above additionally pins that the **exit code** is non-zero, which the unit
tests cannot see.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p taliesin-server a_directory_without_site_yml_is_refused a_loose_document_still_resolves
timeout 60 cargo test -p taliesin-server --test project_required preview_of_a_non_project
```

Expected: the two unit tests FAIL (`expect_err` panics, because resolution succeeds today).

**Run the CLI test under `timeout`, as shown.** Today `preview` warns and serves forever,
so that test does not fail, it hangs. A hang here IS the red state; do not interpret it as
a broken harness. Once Task 3 is implemented the process exits immediately and the timeout
is never reached.

- [ ] **Step 3: Write the implementation**

In `crates/server/src/serve_site/mod.rs`, inside `resolve_target`, immediately after
`let root = root.canonicalize().unwrap_or(root);` and **before** `Site::discover_with` is
reached:

```rust
    // A directory target is a project; a project is what `_site.yml` declares. Refuse before
    // binding a port, so the author gets the fix instead of a 404 page at `/` whose only link
    // points back at itself and which mounts neither the live client nor the dev menu.
    if scope.is_none() && !root.join("_site.yml").is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            crate::serve::not_a_project_error(&root, "preview"),
        ));
    }
```

Two details that are easy to get wrong:

- The local binding is `scope` (from `let (root, scope) = match target { … }`), **not**
  `scoped`, which is computed further down. `scope` is `None` only for `Target::Project`,
  which is exactly the directory case.
- `resolve_target` returns `std::io::Result<Resolved>`, so the error is a
  `std::io::Error`, matching the `ErrorKind::NotFound` construction a few lines above.
  `InvalidInput` is the right kind here: the directory exists, it just is not a project.

- [ ] **Step 4: Run the tests to verify they pass**

```sh
cargo test -p taliesin-server --test project_required
```

Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```sh
git add crates/server/src/serve_site/mod.rs crates/server/tests/project_required.rs
git commit -m "feat(preview): refuse a directory that has no _site.yml"
```

---

### Task 4: A standalone document renders no site chrome

The bug that started this: `preview <file>` emits a header with a self-linking "Home",
a burger over an empty nav, a search button, and a footer. `build <file>` emits none of
it. This makes preview match build.

**Files:**
- Modify: `crates/core/src/site/mod.rs` (the `Site` struct; `discover_scoped`; the `SiteCtx` assembly whose `navbar_html`/`footer_html` fields are set around lines 780-790)
- Create: `crates/core/tests/standalone_document_chrome.rs`

**Interfaces:**
- Consumes: `Site::discover_single(&Path) -> Site` (already public).
- Produces: `Site.standalone: bool` (public field). True only for a `discover_single` project whose root has no `_site.yml`.

- [ ] **Step 1: Write the failing test**

Create `crates/core/tests/standalone_document_chrome.rs`:

```rust
//! A document that belongs to no project gets no project chrome. `preview <file>` used to
//! wrap a lone .tmd in a site header carrying a brand link to itself labelled "Home", a
//! burger over an empty nav, a search button and a site footer, none of which
//! `build <file>` has ever emitted.

use std::path::Path;
use taliesin_core::Site; // re-exported at the crate root (`pub use site::{DraftMode, Page, Site}`)

fn corpus(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(rel)
}

#[test]
fn a_lone_document_is_marked_standalone() {
    let site = Site::discover_single(&corpus("agent/executed-read.tmd"));
    assert!(
        site.standalone,
        "corpus/agent has no _site.yml, so this document belongs to no project"
    );
}

#[test]
fn a_document_inside_a_project_is_not_standalone() {
    let site = Site::discover_single(&corpus("shared-bib/index.tmd"));
    assert!(
        !site.standalone,
        "corpus/shared-bib HAS an _site.yml, so its pages keep project chrome"
    );
}

#[test]
fn a_standalone_document_renders_no_site_header_or_footer() {
    let site = Site::discover_single(&corpus("agent/executed-read.tmd"));
    let page = site.pages.first().expect("the one scoped page");
    let ctx = site.page_chrome(page);
    assert_eq!(ctx.navbar_html, "", "no site navbar: {:?}", ctx.navbar_html);
    assert_eq!(ctx.footer_html, "", "no site footer: {:?}", ctx.footer_html);
}

#[test]
fn a_project_page_still_renders_its_header() {
    let site = Site::discover(&corpus("shared-bib"));
    let page = site.pages.first().expect("a page");
    let ctx = site.page_chrome(page);
    assert!(
        ctx.navbar_html.contains("tali-site-nav"),
        "a real project keeps its navbar: {:?}",
        ctx.navbar_html
    );
}
```

The chrome builder is `Site::page_chrome(&self, page: &Page) -> SiteCtx`
(`crates/core/src/site/mod.rs:736`). It takes the page only; the `depth` used by
`navbar_html` is derived inside. `SiteCtx` is `pub` in `crates/core/src/render/page.rs:73`,
so its `navbar_html` / `footer_html` fields are readable from an integration test.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p taliesin-core --test standalone_document_chrome
```

Expected: FAIL, `no field standalone on type Site`.

- [ ] **Step 3: Write the implementation**

Add the field to `pub struct Site` in `crates/core/src/site/mod.rs`, beside `book`:

```rust
    /// True when this is a one-document project synthesized by
    /// [`Site::discover_single`] for a file with no `_site.yml` anywhere above it.
    ///
    /// Such a document belongs to no project, so it gets no project chrome: the navbar
    /// would brand it "Home" and link to the page you are already on, the burger would
    /// open an empty nav, and the footer would credit a site that does not exist.
    /// `build <file>` has never emitted any of it; this is what makes `preview` agree.
    pub standalone: bool,
```

Set it in `discover_scoped`, where `config` is already loaded:

```rust
        let standalone = only.is_some() && !root.join("_site.yml").is_file();
```

and add `standalone,` to the `Site { … }` literal. There is exactly **one** such literal in
the whole workspace, `crates/core/src/site/mod.rs:520` (`let mut site = Site {`), so adding
a public field breaks exactly one construction site and nothing in the test crates.

Gate the chrome in the `SiteCtx` assembly (the `navbar_html:` / `footer_html:` fields):

```rust
            navbar_html: if book || self.standalone {
                String::new()
            } else {
                self.navbar_html(page, depth)
            },
            footer_html: if self.standalone {
                String::new()
            } else {
                self.footer_html(depth)
            },
```

- [ ] **Step 4: Run the tests to verify they pass**

```sh
cargo test -p taliesin-core --test standalone_document_chrome
```

Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```sh
git add crates/core/src/site/mod.rs crates/core/tests/standalone_document_chrome.rs
git commit -m "fix(preview): a document with no project renders no project chrome"
```

---

### Task 5: Pin that preview and build agree

The regression test that would have caught the original bug. Separate from Task 4 because
it crosses the crate boundary: it drives the real binary rather than the library.

**Files:**
- Test: `crates/server/tests/project_required.rs` (extend)

**Interfaces:**
- Consumes: the behaviour from Task 4. No new API.

- [ ] **Step 1: Write the failing test**

Append to `crates/server/tests/project_required.rs`:

```rust
/// The contract: for a document with no ancestor `_site.yml`, what `preview` serves and what
/// `build` writes carry the same chrome. This is the assertion the "Home" button bug failed.
#[test]
fn a_standalone_document_builds_without_site_chrome() {
    let out = std::env::temp_dir().join("tali-standalone-chrome.html");
    let out_s = out.to_string_lossy().into_owned();
    let (ok, _o, stderr) = run(&[
        "build",
        &corpus("agent/executed-read.tmd"),
        &out_s,
        "--no-exec",
    ]);
    assert!(ok, "single-document build; stderr: {stderr}");
    let html = std::fs::read_to_string(&out).expect("built page");
    let _ = std::fs::remove_file(&out);

    for marker in ["tali-site-nav", "tali-nav-brand", "tali-nav-burger", "tali-site-footer"] {
        assert!(
            !html.contains(marker),
            "a standalone document must carry no `{marker}`"
        );
    }
    // The reader affordances stay: they are personal, not project, chrome.
    assert!(html.contains("tali-theme-toggle"), "theme toggle survives");
}
```

- [ ] **Step 2: Run the test**

```sh
cargo test -p taliesin-server --test project_required a_standalone_document_builds
```

Expected: PASS immediately. `build <file>` already behaves correctly; this test exists to
stop it and `preview` diverging again. If it fails, Task 4 changed the single-file build
path, which it should not have.

- [ ] **Step 3: Verify it is a real guard (red-green)**

Temporarily revert Task 4's `navbar_html` gate (drop `|| self.standalone`), re-run the
Task 4 tests, and confirm `a_standalone_document_renders_no_site_header_or_footer` fails.
Restore the gate. A test that has never been red is not yet a regression test.

- [ ] **Step 4: Commit**

```sh
git add crates/server/tests/project_required.rs
git commit -m "test: pin that a standalone document's chrome matches between build and preview"
```

---

### Task 6: Update the documentation the change invalidates

**Files:**
- Modify: `CLAUDE.md` (the `src/serve_site/` bullet describing `preview <file.tmd>` resolution)
- Modify: `docs/guide/reference/` (the `build` / `preview` pages; find them with `grep -rln "preview" docs/guide/reference/`)
- Check: `crates/core/src/site/config/mod.rs` (`MISSING_CONFIG_PREFIX`, line 297)

- [ ] **Step 1: Check whether `MISSING_CONFIG_PREFIX` still has a caller**

```sh
grep -rn "MISSING_CONFIG_PREFIX\|no _site.yml at" crates/ --include='*.rs'
```

If `build` and `preview` no longer reach it and `check` suppresses it, remove the constant
and the code that emits the warning. If a caller remains, leave it and note which.

- [ ] **Step 2: Update `CLAUDE.md`**

The `serve_site/` bullet currently reads that a file "with no ancestor `_site.yml` … is a
project of just that document (`Site::discover_single`)". Keep that sentence (it is still
how the machinery works) and add that such a project renders **no** site chrome, so
`preview <file>` matches `build <file>`. In the same bullet, record that a directory
without `_site.yml` is now refused by both verbs.

- [ ] **Step 3: Update the guide**

In the `build` and `preview` reference pages, state that a directory target requires
`_site.yml`, and show the error's two fixes. Keep the prose in the guide's existing voice.

- [ ] **Step 4: Verify the docs still build**

```sh
cargo run -p taliesin-server -- build docs/guide --no-exec
```

Expected: exit 0. (`docs/guide` HAS an `_site.yml`, so it is unaffected by Task 2.)

- [ ] **Step 5: Commit**

```sh
git add CLAUDE.md docs/guide crates/core/src/site/config/mod.rs
git commit -m "docs: a directory needs _site.yml; a standalone document has no chrome"
```

---

### Task 7: Full gate

- [ ] **Step 1: Run the whole suite**

```sh
cargo test --workspace
```

Expected: PASS. Pay attention to `crates/core/tests/corpus.rs` and
`crates/server/tests/embed_site_build.rs`, which build corpus projects; all of those
directories have `_site.yml` and must be unaffected.

- [ ] **Step 2: Run the real gate**

```sh
./tools/gates.sh
```

Expected: green, with every `TALIESIN_REQUIRE_*` canary reporting `... ok`. This is the
only check that catches the drift gates living outside `taliesin-core`. Set
`TALIESIN_PYTHON` to a Python with `ipykernel` first (a `.venv` at the repo root is found
automatically by the interpreter resolver's ancestor search).

- [ ] **Step 3: Confirm the original complaint is fixed by hand**

```sh
cargo run -p taliesin-server -- preview corpus/agent            # expect: refusal + guidance
cargo run -p taliesin-server -- preview corpus/agent/executed-read.tmd 4321
curl -s http://127.0.0.1:4321/ | grep -c 'tali-site-nav'        # expect: 0
```

- [ ] **Step 4: Commit any fixes and open the PR**

```sh
git add -A && git commit -m "chore: gate fixes"
```

## Notes for the implementer

- **Do not "fix" `map` along the way.** `taliesin map corpus/agent/` prints
  `(untitled) (site) → _site`, which carries the same inferred-website assumption. It is
  recorded as a follow-up in the spec and is deliberately out of scope; `map` must keep
  working on bare directories because the `read` refusal points authors at it.
- **Do not fix `.tali-stretch` here.** A separate defect found during the corpus browser
  test: build-time AVIF optimization wraps `<img>` in `<picture>`, which breaks the deck's
  direct-child stretch CSS in `crates/core/assets/css/deck.css`. Separate spec.
- **`enclosing_site_root` checks the starting directory first.** That is harmless here
  because both call sites already know the directory has no `_site.yml`, so the first
  iteration always misses and the walk ascends. Do not "optimize" it by passing the parent.
- **Editing `assets/css/*` or `assets/js/*` needs a `cargo build` before the change shows
  up**, since they are `include_str!`-compiled. This change touches neither.
