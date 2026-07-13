//! Byte-exact `body_text()` snapshot for the `taliesin read` text projection.
//!
//! `render/text.rs`'s unit tests prove individual constructs; this pins the *whole*
//! projection of a rich document (headings, a labelled figure with resolved "Figure N",
//! a callout labelled by kind, display math as raw TeX, a fenced code cell) so a
//! regression in the projector (a dropped block, a run-together callout, a leaked KaTeX
//! glyph) changes bytes here.
//!
//! Rewrite the snapshot after an intentional change with:
//!
//! ```sh
//! UPDATE_SNAPSHOTS=1 cargo test -p taliesin-core --test text_projection
//! ```
//!
//! then read the diff before committing, since an unreviewed update pins the bug.

mod common;
use common::corpus_dir;
use std::fs;
use std::path::PathBuf;

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.txt"))
}

#[test]
fn text_projection_of_a_rich_doc_is_pinned() {
    let rel = "reader/text-projection.tmd";
    let path = corpus_dir().join(rel);
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
    let doc = taliesin_core::render_document_with_includes(&src, path.parent().unwrap());
    let actual = doc.body_text();
    let snap = snapshot_path("text-projection");

    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        fs::create_dir_all(snap.parent().unwrap()).unwrap();
        fs::write(&snap, &actual).unwrap();
        return;
    }

    let expected = fs::read_to_string(&snap).unwrap_or_else(|_| {
        panic!(
            "missing snapshot {}\nrerun with UPDATE_SNAPSHOTS=1 to create it",
            snap.display()
        )
    });
    assert_eq!(
        actual, expected,
        "text projection drifted; rerun with UPDATE_SNAPSHOTS=1 and review the diff"
    );

    // The load-bearing behaviors, asserted directly so an accidental re-bless of a broken
    // projection is still caught: the figure resolves to its number, the callout is
    // labelled by kind, and the display math is raw TeX (not KaTeX glyphs).
    assert!(
        actual.contains("Figure 1: A scree plot"),
        "resolved figure number"
    );
    assert!(
        actual.contains("[note] Heads up"),
        "callout labelled by kind"
    );
    assert!(actual.contains("$$ E = mc^2 $$"), "display math as raw TeX");
    assert!(
        actual.contains("```python"),
        "code cell fenced with its language"
    );
}
