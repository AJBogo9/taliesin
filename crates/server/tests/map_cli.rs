//! `taliesin map --format json` emits the whole-project outline in one read-only call:
//! the page list in nav/chapter order, nav + mounts, and the cross-reference graph. The
//! agent's one-call orientation for a project.

use std::process::Command;

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

fn map_json(path: &str) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["map", path, "--format", "json"])
        .output()
        .expect("run taliesin map");
    assert!(
        out.status.success(),
        "map failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("map emits valid json")
}

#[test]
fn map_book_lists_chapter_order_and_xref_graph() {
    let m = map_json(&corpus("demo-book"));
    assert_eq!(m["is_book"], true, "demo-book is a book");
    let urls: Vec<&str> = m["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["url"].as_str().unwrap())
        .collect();
    assert_eq!(
        urls,
        [
            "index.html",
            "intro.html",
            "methods.html",
            "results.html",
            "summary.html"
        ],
        "pages follow the chapters: order"
    );
    // The cross-reference graph is populated (demo-book cites @sec-methods/@sec-setup/@thm-kl),
    // and each target carries its defining url + number.
    let xref = &m["xref_targets"];
    assert!(xref["sec-methods"].is_object(), "xref graph populated: {m}");
    assert!(
        xref["sec-methods"]["url"]
            .as_str()
            .is_some_and(|u| u.ends_with(".html")),
        "an xref target names its page: {m}"
    );
}

#[test]
fn map_site_lists_nav_in_order() {
    let m = map_json(&corpus("tech-blog"));
    assert_eq!(m["is_book"], false, "tech-blog is a website");
    let nav: Vec<&str> = m["nav"]["left"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["text"].as_str())
        .collect();
    assert_eq!(
        nav,
        ["Blog", "Publications", "Projects", "CV"],
        "nav order preserved"
    );
}

#[test]
fn map_excludes_drafts_and_surfaces_mounts() {
    // No corpus site declares a draft or a mount, so build a throwaway project.
    let dir = std::env::temp_dir().join(format!("tali-map-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("_site.yml"),
        "title: Mapped\nmounts:\n  docs: ../docs\n",
    )
    .unwrap();
    std::fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nWelcome.\n").unwrap();
    std::fs::write(
        dir.join("wip.tmd"),
        "---\ntitle: WIP\ndraft: true\n---\n\nNot ready.\n",
    )
    .unwrap();

    let m = map_json(dir.to_str().unwrap());

    let urls: Vec<&str> = m["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["url"].as_str().unwrap())
        .collect();
    assert!(urls.contains(&"index.html"), "published page present: {m}");
    assert!(
        !urls.contains(&"wip.html"),
        "a draft: page is excluded from the map, as a build excludes it: {m}"
    );

    let mounts = m["mounts"].as_array().unwrap();
    assert_eq!(mounts.len(), 1, "the mount is surfaced: {m}");
    assert_eq!(mounts[0]["at"], "docs");
    assert_eq!(mounts[0]["path"], "../docs");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn map_rejects_a_single_file() {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["map", &corpus("reader/text-projection.tmd")])
        .output()
        .expect("run");
    assert!(!out.status.success(), "map on a file must fail");
}
