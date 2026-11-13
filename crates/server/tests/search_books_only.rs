//! Search lives in books only (owner ruling 2026-11-13). A website build carries no search
//! button, no `search-index.js`, no search URL global and no Cmd-K binding; a book build
//! keeps all four.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-searchbook-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(root: &Path) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(root)
        .output()
        .expect("run build");
    assert!(
        out.status.success(),
        "build failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The shared `_assets/app.<hash>.js`, where the Cmd-K binding ships.
fn app_js(out: &Path) -> String {
    let dir = out.join("_assets");
    for e in fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with("app.") && name.ends_with(".js") {
            return fs::read_to_string(p).unwrap();
        }
    }
    panic!("no app.js under {}", dir.display());
}

fn body(label: &str) -> String {
    format!("---\ntitle: T\n---\n\nProse about kangaroos.\n\n## Head {{#sec-{label}}}\n\nBody.\n")
}

#[test]
fn a_website_build_has_no_search() {
    let root = tmp_dir("web");
    fs::write(
        root.join("_site.yml"),
        "title: W\nnav:\n  left:\n    - text: Two\n      href: two.tmd\n",
    )
    .unwrap();
    fs::write(root.join("index.tmd"), body("a")).unwrap();
    fs::write(root.join("two.tmd"), body("b")).unwrap();
    build(&root);
    let site = root.join("_site");
    assert!(
        !site.join("search-index.js").exists(),
        "a website wrote search-index.js"
    );
    for page in ["index.html", "two.html"] {
        let html = fs::read_to_string(site.join(page)).unwrap();
        assert!(
            !html.contains("tali-search-btn"),
            "{page} has a search button"
        );
        assert!(
            !html.contains("TALIESIN_SEARCH_URL"),
            "{page} has a search URL global"
        );
        assert!(
            !html.contains("data-tali-search"),
            "{page} has a search control"
        );
    }
    let js = app_js(&site);
    assert!(
        !js.contains("taliOpenSearch"),
        "a website's app.js ships the Cmd-K palette"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_book_build_keeps_search() {
    let root = tmp_dir("book");
    fs::write(
        root.join("_site.yml"),
        "title: B\nchapters:\n  - index.tmd\n  - two.tmd\n",
    )
    .unwrap();
    fs::write(root.join("index.tmd"), body("a")).unwrap();
    fs::write(root.join("two.tmd"), body("b")).unwrap();
    build(&root);
    let site = root.join("_book");
    let idx =
        fs::read_to_string(site.join("search-index.js")).expect("a book writes search-index.js");
    assert!(idx.contains("kangaroos"), "the index carries the prose");
    let html = fs::read_to_string(site.join("index.html")).unwrap();
    assert!(
        html.contains("tali-search-btn"),
        "a book keeps its search button"
    );
    assert!(
        html.contains("TALIESIN_SEARCH_URL"),
        "a book keeps its search URL global"
    );
    assert!(
        app_js(&site).contains("taliOpenSearch"),
        "a book keeps the Cmd-K palette"
    );
    let _ = fs::remove_dir_all(&root);
}
