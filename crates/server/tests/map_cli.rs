//! `taliesin map [--format json]` emits the whole-project outline in one read-only call:
//! the page list in nav/chapter order, nav + mounts, and the cross-reference graph. The
//! agent's one-call orientation for a project — and, on a single `.tmd`, the
//! cross-reference targets the retired `symbols` verb used to list.

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

/// A single `.tmd` is a project of one page, which is how `map` absorbed the retired
/// `symbols` verb: `xref_targets` on one file is exactly "what can I write after `@` in
/// this document".
///
/// The fixture is `corpus/reader/hovercards.tmd`, which carries both shapes an anchor can
/// take: a brace anchor (`## Why it works {#sec-why}`) and a *cell* label
/// (`%%| label: fig-flow`). The companion's completion once harvested only the first with a
/// `/\{#([\w-]+)\}/` regex, so cell-labeled figures, tables and listings — the majority of
/// the corpus's cross-reference targets — were invisible. Emitting the registry Rust
/// already builds is what keeps the two from drifting.
#[test]
fn map_of_one_file_lists_cell_labeled_and_brace_anchored_targets() {
    let m = map_json(&corpus("reader/hovercards.tmd"));
    let pages: Vec<&str> = m["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["rel"].as_str().unwrap())
        .collect();
    assert_eq!(
        pages,
        vec!["hovercards.tmd"],
        "a single file is a project of exactly that page: {pages:?}"
    );

    let targets = m["xref_targets"].as_object().expect("an xref_targets map");
    // The bug: a `%%| label: fig-flow` cell was invisible to the editor.
    assert!(
        targets.contains_key("fig-flow"),
        "cell-labeled target missing: {targets:?}"
    );
    // The no-regression: a brace anchor is still offered.
    assert!(
        targets.contains_key("sec-why"),
        "brace-anchored target missing: {targets:?}"
    );
    // Each target carries the number the registry assigned, so `@fig-flow` renders as
    // "Figure 1" and a completion can show it.
    assert_eq!(targets["fig-flow"]["number"], "1", "got: {targets:?}");
}

/// Reading one document must not boot a Jupyter kernel: an editor may call this on a
/// keystroke. `hovercards.tmd`'s `fig-flow` is a *cell* label and resolves with no Python
/// in sight — a cell's `label:` is registered while the block model is built, long before
/// the cell would run.
#[test]
fn map_of_one_file_is_parse_only() {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["map", &corpus("reader/hovercards.tmd"), "--format", "json"])
        .env("TALIESIN_PYTHON", "/nonexistent/python")
        .env("TALIESIN_R", "/nonexistent/R")
        .output()
        .expect("run taliesin map");
    assert!(
        out.status.success(),
        "map must not need an interpreter: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let m: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert!(
        m["xref_targets"]["fig-flow"].is_object(),
        "the cell label still resolves with no kernel available: {m}"
    );
}

/// `map` answers "what can I write after `@`", so it must not offer an anchor that `@` can
/// never resolve. A `.theorem` div registers whatever id it is given, but `cite` only links
/// an anchor whose prefix names a cross-reference kind, so a `::: {.theorem #pythagoras}`
/// is numbered and displayed yet is unreferenceable: `@pythagoras` stays literal text.
#[test]
fn an_anchor_that_cannot_be_referenced_is_not_a_target() {
    let dir = std::env::temp_dir().join(format!("tali-map-thm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("thm.tmd");
    std::fs::write(
        &doc,
        "---\ntitle: t\n---\n\n\
         ::: {.theorem #pythagoras title=\"Pythagoras\"}\nText.\n:::\n\n\
         ::: {.theorem #thm-good title=\"Good\"}\nText.\n:::\n",
    )
    .unwrap();

    let m = map_json(doc.to_str().unwrap());
    let targets = m["xref_targets"].as_object().unwrap();
    assert!(
        targets.contains_key("thm-good"),
        "a referenceable theorem is a target: {targets:?}"
    );
    assert!(
        !targets.contains_key("pythagoras"),
        "`@pythagoras` never resolves, so it must not be offered: {targets:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_format_is_a_hard_error_with_a_did_you_mean() {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["map", &corpus("reader/hovercards.tmd"), "--formt", "json"])
        .output()
        .expect("run");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a typo'd flag must not silently run with defaults"
    );
    assert!(
        err.contains("--format"),
        "expected a did-you-mean; got: {err}"
    );
}

#[test]
fn a_missing_path_prints_usage() {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("map")
        .output()
        .expect("run");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(err.contains("usage: taliesin map"), "got: {err}");
}
