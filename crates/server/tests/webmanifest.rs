//! A site build emits `manifest.webmanifest` plus app icons at its output root, so a
//! reader can install the site or book as an app. Packaging only: there is no service
//! worker and no offline claim (the book `.zip` owns offline).

use std::fs;
use std::process::Command;

/// Build a throwaway project and return `(output dir, stderr)`. The caller deletes the
/// source tree; the output lives inside it, so read what you need before dropping it.
fn build(name: &str, files: &[(&str, &str)]) -> (std::path::PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("tali-manifest-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for (rel, body) in files {
        let dest = dir.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(dest, body).unwrap();
    }
    let out = dir.join("_out");
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run build");
    let stderr = String::from_utf8_lossy(&res.stderr).to_string();
    assert!(res.status.success(), "build failed: {stderr}");
    (out, stderr)
}

#[test]
fn a_website_build_emits_a_manifest_and_the_bundled_icons() {
    // No `url:` — unlike sitemap.xml and the feeds, the manifest must not be gated on it,
    // because every URL inside a manifest is relative to the manifest itself.
    let (out, _) = build(
        "site",
        &[
            ("_site.yml", "title: My Site\ndescription: Hello there\n"),
            ("index.tmd", "---\ntitle: Home\n---\n\n# Home\n\nHi.\n"),
        ],
    );
    let manifest = fs::read_to_string(out.join("manifest.webmanifest")).unwrap_or_default();
    let icon_192 = fs::read(out.join("icon-192.png")).unwrap_or_default();
    let icon_512 = fs::read(out.join("icon-512.png")).unwrap_or_default();
    let maskable = fs::read(out.join("icon-maskable-512.png")).unwrap_or_default();
    let sitemap = out.join("sitemap.xml").exists();
    let _ = fs::remove_dir_all(out.parent().unwrap());

    assert!(
        manifest.contains("\"name\":\"My Site\""),
        "manifest missing or unnamed: {manifest}"
    );
    assert!(
        manifest.contains("\"display\":\"standalone\""),
        "{manifest}"
    );
    assert!(
        manifest.contains("\"description\":\"Hello there\""),
        "{manifest}"
    );
    assert!(
        !sitemap,
        "sitemap.xml needs `url:`; its absence is what proves the manifest is not gated on it"
    );
    // Real PNGs, not empty placeholders.
    assert_eq!(
        icon_192.get(..4),
        Some(&b"\x89PNG"[..]),
        "icon-192.png is not a PNG"
    );
    assert_eq!(
        icon_512.get(..4),
        Some(&b"\x89PNG"[..]),
        "icon-512.png is not a PNG"
    );
    assert_eq!(
        maskable.get(..4),
        Some(&b"\x89PNG"[..]),
        "icon-maskable-512.png is not a PNG"
    );
}

#[test]
fn a_book_build_emits_one_too_and_names_it_from_the_title() {
    let (out, _) = build(
        "book",
        &[
            ("_site.yml", "title: My Guide\nchapters:\n  - index.tmd\n"),
            ("index.tmd", "---\ntitle: Intro\n---\n\n# Intro\n\nHello.\n"),
        ],
    );
    let manifest = fs::read_to_string(out.join("manifest.webmanifest")).unwrap_or_default();
    let _ = fs::remove_dir_all(out.parent().unwrap());
    assert!(manifest.contains("\"name\":\"My Guide\""), "{manifest}");
}

/// A 1x1 PNG: the build copies icon bytes, it never decodes them.
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

#[test]
fn author_icons_win_and_suppress_the_bundled_set() {
    let dir = std::env::temp_dir().join(format!("tali-manifest-own-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("_site.yml"), "title: Mine\n").unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n# Home\n\nHi.\n",
    )
    .unwrap();
    fs::write(dir.join("icon-192.png"), PNG_1X1).unwrap();
    fs::write(dir.join("icon-512.png"), PNG_1X1).unwrap();
    let out = dir.join("_out");
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run build");
    let stderr = String::from_utf8_lossy(&res.stderr).to_string();
    let manifest = fs::read_to_string(out.join("manifest.webmanifest")).unwrap_or_default();
    let shipped_192 = fs::read(out.join("icon-192.png")).unwrap_or_default();
    let maskable_exists = out.join("icon-maskable-512.png").exists();
    let _ = fs::remove_dir_all(&dir);

    assert!(res.status.success(), "build failed: {stderr}");
    assert_eq!(
        shipped_192, PNG_1X1,
        "the author's icon must not be overwritten by the bundled mark"
    );
    assert!(
        !maskable_exists,
        "an author set without a maskable file must not gain a bundled one (no mixed brands)"
    );
    assert!(!manifest.contains("maskable"), "{manifest}");
}

#[test]
fn an_incomplete_author_set_falls_back_to_the_bundled_icons() {
    // Only a 512: Chrome needs a 192 too, so a partial set must not be used at all.
    let (out, _) = build(
        "partial",
        &[
            ("_site.yml", "title: Partial\n"),
            ("index.tmd", "---\ntitle: Home\n---\n\n# Home\n\nHi.\n"),
            ("icon-512.png", "not-a-real-png-but-mirrored"),
        ],
    );
    let icon_192 = fs::read(out.join("icon-192.png")).unwrap_or_default();
    let manifest = fs::read_to_string(out.join("manifest.webmanifest")).unwrap_or_default();
    let _ = fs::remove_dir_all(out.parent().unwrap());
    assert_eq!(
        icon_192.get(..4),
        Some(&b"\x89PNG"[..]),
        "an incomplete author set must fall back to the bundled icons"
    );
    assert!(manifest.contains("maskable"), "{manifest}");
}
