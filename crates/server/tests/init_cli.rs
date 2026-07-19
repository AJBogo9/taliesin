//! `taliesin init [--template basic|site|book]` scaffolds a project that is correct on the
//! first save: it renders and `taliesin check` passes on it with no diagnostics.
//!
//! The template bytes are pinned unit-side against `corpus/scaffold-{site,book}/`
//! (`cli::init_template_tests`); this file exercises the CLI end-to-end — the `--template`
//! flag path, the shared onramp, refuse-to-overwrite, and a real `check` over the whole
//! project (so nav links and book chapters resolve, not just per-page front matter).

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    // A per-call sequence (not just name+pid) so two tests reusing a name don't share one dir
    // and clobber each other's scaffold when run in parallel.
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tali-init-{}-{}-{}", name, std::process::id(), seq));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin init");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn check_is_clean(path: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["check", path.to_str().unwrap()])
        .output()
        .expect("run taliesin check");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The whole point: every template must survive the tool's own preflight as a project.
#[test]
fn every_template_scaffolds_a_check_clean_project() {
    for (template, pages) in [
        ("basic", &["index.tmd"][..]),
        ("site", &["index.tmd", "about.tmd"][..]),
        ("book", &["index.tmd", "intro.tmd", "methods.tmd"][..]),
    ] {
        let dir = tmp(template);
        let (ok, _out, err) = run(&["init", dir.to_str().unwrap(), "--template", template]);
        assert!(
            ok,
            "`init --template {template}` should succeed; stderr: {err}"
        );

        // Config + the shared onramp are always written.
        assert!(
            dir.join("_site.yml").exists(),
            "{template}: _site.yml written"
        );
        assert!(
            dir.join("AGENTS.md").exists(),
            "{template}: AGENTS.md written"
        );
        assert!(
            dir.join(".taliesin/tali-site.schema.json").exists(),
            "{template}: schema wired"
        );
        for p in pages {
            assert!(dir.join(p).exists(), "{template}: {p} written");
        }

        let (clean, diagnostics) = check_is_clean(&dir);
        assert!(
            clean,
            "`taliesin check` must pass on a fresh `init --template {template}`, got:\n{diagnostics}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The `site` template is genuinely multi-page: it links a second page from the nav, which
/// only resolves when `check` sees the whole project (not per-page). The `book` template lists
/// its chapters. These are the properties a single-doc scaffold cannot have.
#[test]
fn site_links_a_second_page_and_book_lists_chapters() {
    let site = tmp("site-shape");
    run(&["init", site.to_str().unwrap(), "--template", "site"]);
    let cfg = std::fs::read_to_string(site.join("_site.yml")).unwrap();
    assert!(
        cfg.contains("nav:") && cfg.contains("about.tmd"),
        "site nav links about: {cfg}"
    );
    assert!(
        site.join("about.tmd").exists(),
        "site ships the linked page"
    );
    let _ = std::fs::remove_dir_all(&site);

    let book = tmp("book-shape");
    run(&["init", book.to_str().unwrap(), "--template", "book"]);
    let cfg = std::fs::read_to_string(book.join("_site.yml")).unwrap();
    assert!(
        cfg.contains("chapters:"),
        "book config declares chapters: {cfg}"
    );
    assert!(
        book.join("intro.tmd").exists() && book.join("methods.tmd").exists(),
        "book ships the listed chapters"
    );
    let _ = std::fs::remove_dir_all(&book);
}

/// `init` with no `--template` (non-interactive) keeps its historical default: the basic
/// one-page site, and nothing more.
#[test]
fn init_without_a_template_scaffolds_the_basic_site() {
    let dir = tmp("default");
    let (ok, _out, err) = run(&["init", dir.to_str().unwrap()]);
    assert!(ok, "bare `init` succeeds; stderr: {err}");
    assert!(
        dir.join("_site.yml").exists() && dir.join("index.tmd").exists(),
        "basic writes _site.yml + index.tmd"
    );
    assert!(!dir.join("about.tmd").exists(), "basic has no extra pages");
    let (clean, d) = check_is_clean(&dir);
    assert!(clean, "basic init must check clean:\n{d}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An unknown `--template` value is a hard error that names the nearest match (never a silent
/// fall-back to basic).
#[test]
fn an_unknown_template_is_rejected_with_a_hint() {
    let dir = tmp("bad");
    let (ok, _out, err) = run(&["init", dir.to_str().unwrap(), "--template", "sit"]);
    assert!(!ok, "an unknown template fails");
    assert!(
        err.contains("did you mean `site`?"),
        "hints the nearest: {err}"
    );
    // And it wrote nothing (a rejected template never half-scaffolds).
    assert!(
        !dir.join("_site.yml").exists(),
        "no partial scaffold on a bad template"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
