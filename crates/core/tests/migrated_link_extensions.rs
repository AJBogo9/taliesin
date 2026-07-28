//! Item 128 (2026-07-28). A migrated document's internal links keep the extension the old
//! tool used, and both link validators reported them as flatly broken while the renamed
//! source sat in the same directory.
//!
//! Measured: **118 of 123** link errors on `rust-lang/book` (`.md`) and **10 of 11** on a real
//! Quarto book (`.qmd`) were this one shape — `creators.qmd` reported broken with
//! `creators.tmd` a page in the same site. That is a triage pass a stranger pays before any
//! real work starts, on a project that is otherwise fine (item 133's pricing).
//!
//! The deliverable is a **suggestion, not a rewrite**, and the third row below is why: a `.md`
//! link may point at a real shipped `.md` file, so rewriting would break a working link. The
//! suggestion also lifts into the structured `suggestion.replacement` an agent or an editor
//! quick fix applies, which is where the value actually lands.

use std::path::{Path, PathBuf};

fn tmp(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("tali-migrated-links-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A project whose links carry foreign extensions, with the renamed sources present and one
/// genuinely-shipped `.md` beside them.
fn fixture(tag: &str) -> PathBuf {
    let dir = tmp(tag);
    std::fs::write(dir.join("_site.yml"), "title: Migrated\n").unwrap();
    std::fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Intro\n---\n\nSee [creators](creators.qmd) and [one](ch01.md),\n\
         plus [notes](notes.md) which is a real file, and [gone](nowhere.qmd).\n",
    )
    .unwrap();
    for page in ["creators", "ch01"] {
        std::fs::write(
            dir.join(format!("{page}.tmd")),
            format!("---\ntitle: {page}\n---\n\nHi.\n"),
        )
        .unwrap();
    }
    std::fs::write(dir.join("notes.md"), "# real markdown\n").unwrap();
    dir
}

fn single_doc_warnings(dir: &Path) -> Vec<String> {
    let path = dir.join("index.tmd");
    let src = std::fs::read_to_string(&path).unwrap();
    let doc = taliesin_core::render_single_doc(&src, dir);
    taliesin_core::diagnostics::validate_local_links(&doc.blocks, dir)
        .iter()
        .map(|w| w.message.clone())
        .collect()
}

#[test]
fn a_single_doc_check_suggests_the_renamed_source() {
    let dir = fixture("single");
    let msgs = single_doc_warnings(&dir);
    let joined = msgs.join("\n");

    for (broken, suggested) in [("creators.qmd", "creators.tmd"), ("ch01.md", "ch01.tmd")] {
        let hit = msgs
            .iter()
            .find(|m| m.contains(broken))
            .unwrap_or_else(|| panic!("`{broken}` must still be reported broken:\n{joined}"));
        assert!(
            hit.contains(&format!("did you mean `{suggested}`?")),
            "`{broken}` must point at the renamed source: {hit}"
        );
    }

    // A real shipped `.md` is NOT broken, which is the reason this is a suggestion rather
    // than a rewrite. If this row ever fails, the fix is wrong, not this test.
    assert!(
        !joined.contains("notes.md"),
        "a `.md` link backed by a real file must not be reported at all:\n{joined}"
    );

    // A foreign-extension link with no renamed source behind it stays a bare error: a
    // suggestion nothing backs would be a guess.
    let gone = msgs
        .iter()
        .find(|m| m.contains("nowhere.qmd"))
        .expect("a genuinely missing target is still reported");
    assert!(
        !gone.contains("did you mean"),
        "no suggestion may be invented for a target that does not exist: {gone}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_site_check_suggests_the_renamed_source_too() {
    // The two validators are separate code paths answering the same question — the site one
    // resolves through the page registry — so both need the row, or a migration is still a
    // wall of bare errors whichever command the newcomer runs.
    let dir = fixture("site");
    let site = taliesin_core::Site::discover(&dir);
    let msgs: Vec<String> = site
        .validate_cross_page_links()
        .into_iter()
        .map(|(_rel, w)| w.message)
        .collect();
    let joined = msgs.join("\n");
    for (broken, suggested) in [("creators.qmd", "creators.tmd"), ("ch01.md", "ch01.tmd")] {
        let hit = msgs
            .iter()
            .find(|m| m.contains(broken))
            .unwrap_or_else(|| panic!("`{broken}` must be reported:\n{joined}"));
        assert!(
            hit.contains(&format!("did you mean `{suggested}`?")),
            "the site check must suggest the renamed source: {hit}"
        );
    }
    assert!(
        !joined.contains("notes.md"),
        "a real shipped `.md` is not broken in a site either:\n{joined}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A link from a subdirectory must resolve the way the *site* resolves it, not relative to the
/// project root: probing the wrong base would silently drop the suggestion exactly where a
/// real book (chapters in folders) needs it.
#[test]
fn the_suggestion_survives_a_link_from_a_subdirectory() {
    let dir = tmp("nested");
    std::fs::write(dir.join("_site.yml"), "title: Nested\n").unwrap();
    std::fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nHome.\n").unwrap();
    std::fs::create_dir_all(dir.join("part")).unwrap();
    std::fs::write(
        dir.join("part/one.tmd"),
        "---\ntitle: One\n---\n\nNext: [two](two.qmd).\n",
    )
    .unwrap();
    std::fs::write(dir.join("part/two.tmd"), "---\ntitle: Two\n---\n\nHi.\n").unwrap();

    let site = taliesin_core::Site::discover(&dir);
    let msgs: Vec<String> = site
        .validate_cross_page_links()
        .into_iter()
        .map(|(_rel, w)| w.message)
        .collect();
    let hit = msgs
        .iter()
        .find(|m| m.contains("two.qmd"))
        .unwrap_or_else(|| panic!("the nested link must be reported: {msgs:?}"));
    assert!(
        hit.contains("did you mean `two.tmd`?"),
        "shown as the author wrote it (`two.qmd`, not `part/two.qmd`): {hit}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
