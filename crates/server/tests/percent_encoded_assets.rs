//! A percent-encoded local asset reference (`![x](my%20image.png)`, the spelling VS Code
//! inserts when a file whose name has spaces is dragged into the editor) must survive a
//! portable `--out` build end to end. The dev server percent-decodes request paths and a
//! static host decodes URLs the same way, so the validator and the asset copier must
//! resolve the decoded file too: before the shared decode this shipped a folder that
//! 404'd the image ("0 assets", exit 0, plus a false "local asset not found" error) while
//! the preview showed the page intact.

use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-pct-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn taliesin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_taliesin"))
}

#[test]
fn a_percent_encoded_asset_ships_and_its_emitted_src_resolves() {
    let dir = tmp_dir("ship");
    fs::write(dir.join("my image.png"), "png-a").unwrap();
    fs::write(dir.join("other pic.png"), "png-b").unwrap();
    fs::write(
        dir.join("doc.tmd"),
        "---\ntitle: T\n---\n\n![spaced](my%20image.png)\n\n![control](<other pic.png>)\n",
    )
    .unwrap();
    let out_dir = dir.join("out");

    let out = taliesin()
        .arg("build")
        .arg(dir.join("doc.tmd"))
        .arg("--out")
        .arg(&out_dir)
        .output()
        .expect("run taliesin");
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(out.status.success(), "{stderr}");
    assert!(
        !stderr.contains("local asset not found"),
        "both files exist; the validator must decode the ref the way the preview does:\n{stderr}"
    );
    assert!(
        out_dir.join("my image.png").is_file(),
        "the percent-encoded ref bundles, under the decoded name a static host resolves"
    );
    assert!(
        out_dir.join("other pic.png").is_file(),
        "the angle-bracket control keeps working"
    );

    // Every local <img src> the built page emits resolves inside the folder. Read through
    // the walker (tags/attrs), never a substring scan: the inlined bundles build
    // `<img src="${e}"` out of string fragments, which only the walker knows is not markup.
    let html = fs::read_to_string(out_dir.join("index.html")).expect("read built page");
    let mut author_imgs = 0;
    for tag in taliesin_core::render::tags(&html) {
        if !tag.name.eq_ignore_ascii_case("img") {
            continue;
        }
        let Some(src) = taliesin_core::render::attrs(&tag)
            .find(|a| a.name.eq_ignore_ascii_case("src"))
            .map(|a| a.value)
        else {
            continue;
        };
        if src.starts_with("data:") {
            continue;
        }
        let on_disk = out_dir.join(taliesin_core::render::asset_fs_path(src));
        assert!(
            on_disk.is_file(),
            "emitted src `{src}` must resolve inside the portable folder"
        );
        author_imgs += 1;
    }
    assert_eq!(author_imgs, 2, "both author images are on the page");
}

#[test]
fn a_percent_encoded_linked_source_deploys_in_a_site_build() {
    // The third resolution surface: `deploy_referenced_sources` ships source files
    // pages link to (a `.md` download). A `%20`-spelled link must find the on-disk
    // file and ship it under its decoded name, like the validator and the copier.
    let dir = tmp_dir("site-src");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(dir.join("my notes.md"), "# notes\n").unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n[the notes](my%20notes.md)\n",
    )
    .unwrap();
    let out = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--no-exec")
        .output()
        .expect("run taliesin");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        dir.join("_site").join("my notes.md").is_file(),
        "the %20-spelled linked source ships under its decoded name"
    );
}
