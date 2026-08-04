//! A book build emits a single `<book>.zip` at its output root (the offline "read this
//! book" download). The archive is the whole self-contained output packed into one file —
//! a delivery wrapper, not a new output format. The topbar's link to it was deleted
//! 2026-08-04 (visual minimalism pass, task 11), but `write_book_archive` (`build.rs`)
//! still runs for every book build, so the archive itself still ships; this test now pins
//! only that it is a valid, self-consistent ZIP.

use std::fs;
use std::process::Command;

#[test]
fn book_build_emits_a_valid_archive() {
    let dir = std::env::temp_dir().join(format!("tali-book-zip-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("sub")).unwrap();
    // A minimal book: top-level `chapters:` makes it a book; `index.tmd` → index.html.
    fs::write(
        dir.join("_site.yml"),
        "title: My Guide\ntoc: true\nchapters:\n  - index.tmd\n  - sub/two.tmd\n",
    )
    .unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Intro\n---\n\n# Intro\n\nHello.\n",
    )
    .unwrap();
    fs::write(
        dir.join("sub/two.tmd"),
        "---\ntitle: Two\n---\n\n# Two\n\nWorld.\n",
    )
    .unwrap();
    let out = dir.join("_book");

    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run build");
    let stderr = String::from_utf8_lossy(&res.stderr).to_string();

    // `archive_name()` slugs the title: "My Guide" → "my-guide.zip".
    let zip_path = out.join("my-guide.zip");
    let zip = fs::read(&zip_path).unwrap_or_default();
    let _ = fs::remove_dir_all(&dir);

    assert!(res.status.success(), "build failed: {stderr}");
    assert!(!zip.is_empty(), "book build did not emit my-guide.zip");
    // A valid ZIP begins with a local-file-header signature and ends with the end-of-central-
    // directory signature.
    assert_eq!(&zip[..4], b"PK\x03\x04", "not a ZIP (bad local header)");
    assert!(
        zip.windows(4).any(|w| w == b"PK\x05\x06"),
        "ZIP has no end-of-central-directory record"
    );
    // The entry names live uncompressed in the headers, so the raw bytes carry them: the book
    // entry point is packed, and the archive never contains itself.
    assert!(
        zip.windows(10).any(|w| w == b"index.html"),
        "archive must contain index.html"
    );
    assert!(
        !zip.windows(12).any(|w| w == b"my-guide.zip"),
        "archive must not pack itself"
    );
}
