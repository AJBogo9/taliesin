//! Regression test for the relative-path `{{< include >}}` silent-drop bug.
//!
//! When `qmd-fast build/render` is given a *relative* path
//! (`corpus/posts/pca-geometry/index.qmd`), the document's base dir is the
//! relative parent `corpus/posts/pca-geometry`. The include resolver's
//! containment check used to walk the parents of that *relative* path, which hit
//! an empty component before reaching the absolute repo root that holds `.git`,
//! fell back to the doc dir itself, and then rejected the legitimate
//! `../../_includes/three-scene.qmd` include as "escaping" that fake root — so the
//! include was silently dropped and the literal directive leaked into the HTML.
//!
//! The fix absolutizes the base dir before the parent-walk. This test pins it: a
//! relative base must resolve the include (HTML contains the scene definition) and
//! must NOT leave the literal `{{< include` directive behind.

mod common;
use common::TempProj;

use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Process-wide cwd lock: these tests mutate the process working directory, which
/// is shared, so they must not run concurrently with one another.
fn cwd_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// Restore the previous cwd when dropped, even on panic.
struct CwdGuard(std::path::PathBuf);
impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// The repo root (parent of `corpus/`), which holds `.git`.
fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn relative_base_resolves_include_against_repo_root() {
    let _lock = cwd_lock();
    let prev = std::env::current_dir().expect("cwd");
    let _restore = CwdGuard(prev);

    std::env::set_current_dir(repo_root()).expect("cd to repo root");

    // Exactly the CLI's `build corpus/posts/.../index.qmd` shape: a relative doc
    // path whose parent is the relative base.
    let rel_doc = Path::new("corpus/posts/pca-geometry/index.qmd");
    let src = std::fs::read_to_string(rel_doc).expect("read corpus doc");
    let base = rel_doc.parent().unwrap();

    let html = qmd_fast_core::render_html_page_with_includes(&src, base, "pca");

    // The include (`../../_includes/three-scene.qmd`) defines `makeScene3D`; it must
    // be present, proving the include was expanded rather than dropped.
    assert!(
        html.contains("makeScene3D"),
        "relative-base include should resolve and emit the scene definition"
    );
    // And the literal directive must NOT leak into the output (comrak escapes `<`
    // to `&lt;`, so match the surviving, un-escaped middle of the directive).
    assert!(
        !html.contains("include ../../_includes/three-scene.qmd"),
        "the literal `{{{{< include … >}}}}` directive must not leak into the HTML"
    );
}

#[test]
fn unresolvable_include_warns_instead_of_silently_dropping() {
    // A project with a `.git` marker so the containment root is the project root.
    let proj = TempProj::new();
    proj.file(".git", "");
    proj.file(
        "post/index.qmd",
        "---\ntitle: T\n---\n\nIntro.\n\n{{< include ../../../etc/passwd >}}\n\n\
         {{< include missing.qmd >}}\n",
    );

    let src = std::fs::read_to_string(proj.0.join("post/index.qmd")).unwrap();
    let base = proj.0.join("post");
    let doc = qmd_fast_core::render_document_with_includes(&src, &base);

    let msgs: Vec<&str> = doc.warnings.iter().map(|w| w.message.as_str()).collect();
    // The escaping path is refused, located, and reported (not silently dropped).
    let escaping = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("etc/passwd"))
        .expect("a warning for the escaping include");
    assert!(
        escaping.line.is_some(),
        "include warning should be click-to-source (carry a line): {escaping:?}"
    );
    // The missing-file include is also reported.
    assert!(
        msgs.iter().any(|m| m.contains("missing.qmd")),
        "a warning for the not-found include, got: {msgs:?}"
    );
}
