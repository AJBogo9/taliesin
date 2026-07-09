//! Byte-exact `body_html()` snapshots for the hermetic `{js}` documents.
//!
//! `corpus.rs`'s `every_corpus_doc_renders_with_invariants` is *structural* only —
//! non-empty blocks, unique ids, ordered sourcepos — so a real regression in emitted
//! HTML (a broken scrolly wrapper, a dropped reactive cell shell) renders a
//! structurally valid document and passes. These snapshots pin the bytes.
//!
//! Scope is deliberately `{js}`: `exec.rs` maps only `python`/`r` to a kernel, so
//! these documents' cells never execute during a core render and the snapshots stay
//! hermetic — no Jupyter kernel, no CI kernel job. An `{r}`/`{python}` snapshot would
//! either need a kernel or would silently pin the "kernel unavailable" fallback.
//!
//! Snapshots are plain files under `tests/snapshots/`. Rewrite them after an
//! intentional change with:
//!
//! ```sh
//! UPDATE_SNAPSHOTS=1 cargo test -p taliesin-core --test body_html_snapshots
//! ```
//!
//! then read the diff before committing — an unreviewed snapshot update pins the bug.

mod common;
use common::corpus_dir;
use std::fs;
use std::path::PathBuf;

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.html"))
}

/// The first line at which `actual` and `expected` diverge, as a 1-based line number
/// plus both sides, so a failure names the drift instead of dumping two documents.
fn first_divergence(actual: &str, expected: &str) -> Option<(usize, String, String)> {
    let mut a = actual.lines();
    let mut e = expected.lines();
    let mut n = 0usize;
    loop {
        n += 1;
        match (a.next(), e.next()) {
            (None, None) => return None,
            (x, y) if x == y => continue,
            (x, y) => {
                return Some((
                    n,
                    x.unwrap_or("<end of output>").to_string(),
                    y.unwrap_or("<end of snapshot>").to_string(),
                ));
            }
        }
    }
}

fn assert_snapshot(name: &str, rel: &str) {
    let path = corpus_dir().join(rel);
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
    let doc = taliesin_core::render_document_with_includes(&src, path.parent().unwrap());
    let actual = doc.body_html();
    let snap = snapshot_path(name);

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

    if let Some((line, got, want)) = first_divergence(&actual, &expected) {
        panic!(
            "`{rel}` body_html drifted from its snapshot at line {line}\n\
             \x20 actual:   {got}\n\
             \x20 snapshot: {want}\n\n\
             If the change is intentional, rerun with UPDATE_SNAPSHOTS=1 and review the diff."
        );
    }
}

#[test]
fn reactive_graph() {
    assert_snapshot("reactive_graph", "reactive/graph.tmd");
}

#[test]
fn reactive_inputs() {
    assert_snapshot("reactive_inputs", "reactive/inputs.tmd");
}

/// A cell that throws: pins the error-shell HTML, not just that *something* rendered.
#[test]
fn reactive_js_error() {
    assert_snapshot("reactive_js_error", "reactive/js-error.tmd");
}

#[test]
fn explorable_scrolly() {
    assert_snapshot("explorable_scrolly", "explorable/scrolly.tmd");
}
