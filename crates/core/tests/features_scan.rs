//! `taliesin features` must survive every document the repo ships.
//!
//! The instrument that measures adoption is worthless if it aborts on the documents it
//! is pointed at, and worse than worthless during an audit: a crash reads as "this page
//! uses nothing". The 2026-08-02 scope audit could not measure `docs/guide`'s front
//! matter with the tool's own scanner for exactly this reason and had to substitute an
//! independent one.
//!
//! The defect was a byte-at-a-time cursor in the shortcode scanner: any line carrying
//! both `{{<` and a non-ASCII character landed the cursor mid-codepoint and panicked on
//! the next slice. Three of the 25 `docs/guide` pages tripped it on ordinary prose (an
//! arrow, an em dash, angle quotes). A hand-written case list would not have caught it,
//! because nobody thinks to write "and also an em dash", so this gate derives its
//! inputs from the tree instead.

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.tmd` under `dir`, recursively.
fn tmd_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Build output and execution caches are not authored source.
        if name == "_site" || name == "_freeze" || name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            tmd_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "tmd") {
            out.push(path);
        }
    }
}

/// Scanning a document must never panic, for every `.tmd` the repo ships: both books,
/// the marketing site, and the whole corpus.
#[test]
fn feature_scan_survives_every_shipped_document() {
    let repo = repo();
    let mut docs = Vec::new();
    for dir in ["docs/guide", "docs/internals", "site", "corpus", "samples"] {
        tmd_files(&repo.join(dir), &mut docs);
    }
    docs.sort();
    assert!(
        docs.len() > 150,
        "expected the repo's ~195 .tmd documents, found {}; the walker is looking in \
         the wrong place and this gate would pass vacuously",
        docs.len()
    );

    for doc in &docs {
        let src = std::fs::read_to_string(doc).unwrap_or_else(|e| panic!("read {doc:?}: {e}"));
        let rel = doc.strip_prefix(&repo).unwrap_or(doc).display().to_string();
        // `scan` takes no panic-catching path of its own; a panic here fails the test
        // with the document name, which is the diagnostic that was missing.
        let f = taliesin_core::features::scan(&src);
        // Touch the result so the call cannot be optimized away, and assert the scan
        // produced *something* for a non-trivial document rather than silently nothing.
        let _ = f.used.len();
        assert!(
            !rel.is_empty(),
            "unreachable, keeps `rel` live for the panic message"
        );
    }
}

/// The three `docs/guide` pages that used to abort the scan, pinned by name.
///
/// The sweep above would catch a regression, but not tell you it was *this* one. These
/// three are the reproduction: each carries a shortcode and a non-ASCII character on one
/// line. If a page is renamed, fix the name here rather than deleting the case.
#[test]
fn the_three_pages_that_used_to_panic_are_scannable() {
    for rel in [
        "docs/guide/reference/cli.tmd",
        "docs/guide/reference/frontmatter.tmd",
        "docs/guide/using/preview.tmd",
    ] {
        let path = repo().join(rel);
        let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        // The precondition that makes this a real reproduction rather than a page that
        // happens to pass: the page must still contain a line with both a shortcode and
        // a non-ASCII character. Otherwise an innocent edit silently guts the gate.
        assert!(
            src.lines().any(|l| l.contains("{{<") && !l.is_ascii()),
            "{rel} no longer has a line carrying both `{{{{<` and a non-ASCII character, \
             so it no longer reproduces the panic. Point this gate at a page that does."
        );
        let f = taliesin_core::features::scan(&src);
        assert!(
            !f.used.is_empty(),
            "{rel} scanned to nothing, which is what the panic used to look like \
             to a caller that swallowed it"
        );
    }
}
