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

#[test]
fn non_bib_bibliography_is_flagged_not_silently_ignored() {
    let dir = tmp("nonbib");
    // A CSL-YAML/JSON path (not `.bib`) is unsupported; it must warn (located at the
    // `bibliography:` line) rather than silently resolving no citations.
    let src = "---\ntitle: T\nbibliography: refs.yaml\n---\n\nSee [@x].\n";
    let doc = render_document_with_includes(src, &dir);

    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("only BibTeX"))
        .expect("expected a non-.bib warning");
    assert!(
        w.message.contains("refs.yaml"),
        "names the path: {}",
        w.message
    );
    assert_eq!(
        w.line,
        Some(3),
        "warning should point at `bibliography:` (line 3)"
    );
}

/// Two `.bib` files are two files: an entry left unclosed at the end of the first cannot
/// swallow the first entry of the second, and the diagnostic names the file it is in.
/// The page's files used to be concatenated and parsed as one text (audit 2026-09-24 G3),
/// so `b1` vanished and the only message was "broken citation: @b1".
#[test]
fn an_unclosed_entry_is_confined_to_its_own_file_and_reported_there() {
    let dir = tmp("unclosed");
    fs::write(
        dir.join("a.bib"),
        "@article{a1, title={From a}, year={2001}}\n@article{a2, title={Unclosed}, year={2002}\n",
    )
    .unwrap();
    fs::write(
        dir.join("b.bib"),
        "@article{b1, title={First in b}, year={2003}}\n",
    )
    .unwrap();
    let src = "---\ntitle: T\nbibliography: [a.bib, b.bib]\n---\n\nSee [@a2] and [@b1].\n";
    let doc = render_document_with_includes(src, &dir);
    let html = doc.body_html();
    assert!(html.contains("First in b"), "b1 resolves:\n{html}");
    assert!(
        !doc.warnings
            .iter()
            .any(|w| w.message.contains("broken citation")),
        "{:?}",
        doc.warnings
    );
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("not closed"))
        .unwrap_or_else(|| panic!("no unclosed-entry warning: {:?}", doc.warnings));
    assert!(
        w.message.contains("a.bib") && w.message.contains("a2"),
        "{}",
        w.message
    );
    assert_eq!(w.line, Some(3), "located at `bibliography:`");
}
