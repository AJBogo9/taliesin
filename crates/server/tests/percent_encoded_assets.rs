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
        let on_disk = out_dir.join(taliesin_core::render::asset_fs_path(&src));
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

/// A 1x1 RGBA PNG, so the build's size pass has a real header to read.
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x02, 0x01, 0xa1, 0xd4, 0xd6, 0xe6, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// The entity twin of the percent case: an `&` in a file name reaches the finished page as
/// `R&amp;D.png`, and the walker handed that encoded text to every reader. The folder
/// shipped without the image (only `plain.png` was copied), the gate called it missing, the
/// image lost its intrinsic size, and the LCP hint moved on to the second image.
#[test]
fn an_ampersand_in_an_asset_name_ships_with_its_size_and_the_lcp_hint() {
    let dir = tmp_dir("amp");
    fs::create_dir_all(dir.join("img")).unwrap();
    fs::write(dir.join("img/R&D.png"), PNG_1X1).unwrap();
    fs::write(dir.join("img/plain.png"), PNG_1X1).unwrap();
    fs::write(
        dir.join("doc.tmd"),
        "---\ntitle: T\n---\n\n![Chart of spend](img/R&D.png)\n\n![A second chart](img/plain.png)\n",
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
    assert!(!stderr.contains("local asset not found"), "{stderr}");
    assert!(
        out_dir.join("img/R&D.png").is_file(),
        "the image ships under its own name"
    );

    let html = fs::read_to_string(out_dir.join("index.html")).expect("read built page");
    let first = taliesin_core::render::tags(&html)
        .find(|t| t.name.eq_ignore_ascii_case("img"))
        .expect("an image on the page");
    let get = |name: &str| {
        taliesin_core::render::attrs(&first)
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.value.to_string())
    };
    assert_eq!(get("src").as_deref(), Some("img/R&D.png"), "{}", first.text);
    assert_eq!(get("width").as_deref(), Some("1"), "{}", first.text);
    assert_eq!(
        get("fetchpriority").as_deref(),
        Some("high"),
        "{}",
        first.text
    );
}
