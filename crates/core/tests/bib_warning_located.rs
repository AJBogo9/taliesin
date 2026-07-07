//! `.bib` diagnostics (a duplicate key, a missing file) point at the front
//! matter's `bibliography:` line so they are click-to-source, not unlocated.

use std::fs;
use std::path::PathBuf;

use taliesin_core::render_document_with_includes;

/// A throwaway dir under the system temp, unique per test name + process.
fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tali-bibwarn-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn duplicate_bib_key_warning_points_at_the_bibliography_line() {
    let dir = tmp("dupkey");
    // Two `@book` entries share the key `dup` — the second wins with a warning.
    fs::write(
        dir.join("refs.bib"),
        "@book{dup, title={First}, year={2001}}\n@book{dup, title={Second}, year={2002}}\n",
    )
    .unwrap();
    // `bibliography:` sits on source line 3 (line 1 = `---`, line 2 = `title:`).
    let src = "---\ntitle: T\nbibliography: refs.bib\n---\n\nSee [@dup].\n";
    let doc = render_document_with_includes(src, &dir);

    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("duplicate bibliography key"))
        .expect("expected a duplicate-key warning");
    assert_eq!(
        w.line,
        Some(3),
        "warning should point at `bibliography:` (line 3)"
    );
    assert_eq!(w.file, None, "location is in the previewed doc itself");
}

#[test]
fn missing_bib_file_warning_points_at_the_bibliography_line() {
    let dir = tmp("missing");
    // No refs.bib written: the declared file can't be read.
    let src = "---\ntitle: T\nbibliography: refs.bib\n---\n\nSee [@x].\n";
    let doc = render_document_with_includes(src, &dir);

    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("bibliography file not found"))
        .expect("expected a missing-file warning");
    assert_eq!(
        w.line,
        Some(3),
        "warning should point at `bibliography:` (line 3)"
    );
}
