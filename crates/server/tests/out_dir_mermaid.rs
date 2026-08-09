//! `build <file.tmd> --out <dir>` writes a PORTABLE FOLDER, and the vendored mermaid
//! library belongs in it as a sibling file rather than inside the page.
//!
//! The single-file `build <file.tmd>` inlines the library on purpose: one file that renders
//! a diagram with zero network is the whole point of that spelling. `--out <dir>` has a
//! different contract -- `index.html` plus the assets it references -- so paying 3.5 MB
//! inside the HTML buys nothing there. Measured before this file existed: a 2-node diagram
//! took the `--out` page from 230,751 B to 3,803,736 B, a 16.5x blow-up on a mode that was
//! already allowed to put the bytes beside the page.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// The vendored library's esbuild wrapper name: present iff the library itself is there.
const LIB_MARKER: &str = "__esbuild_esm_mermaid";
/// The name `--out` gives the sibling copy. Owned by `build.rs`; asserted here so the two
/// halves (the href in the page, the file on disk) cannot drift apart silently.
const MERMAID_FILE: &str = "mermaid.min.js";

/// A plain fence rather than the `{mermaid}` cell form: a diagram is emitted at render time
/// either way, and the plain fence keeps this test independent of a kernel.
const DIAGRAM: &str = "```mermaid\nflowchart LR\n  A --> B\n```\n";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-outmermaid-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_doc(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
    let doc = dir.join(format!("{name}.tmd"));
    fs::write(
        &doc,
        format!("---\ntitle: {name}\n---\n\n# {name}\n\n{body}"),
    )
    .unwrap();
    doc
}

fn build(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("build")
        .args(args)
        .output()
        .expect("run taliesin build");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn out_dir_links_the_mermaid_library_instead_of_inlining_it() {
    let dir = scratch("linked");
    let doc = write_doc(&dir, "diagram", DIAGRAM);
    let out = dir.join("dist");
    let (ok, err) = build(&[doc.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    assert!(ok, "build --out exited non-zero: {err}");

    let index = out.join("index.html");
    let html = fs::read_to_string(&index).expect("--out writes index.html");
    let lib = out.join(MERMAID_FILE);

    assert!(
        lib.is_file(),
        "--out must write the vendored library beside the page as {MERMAID_FILE}"
    );
    assert!(
        fs::read_to_string(&lib).unwrap().contains(LIB_MARKER),
        "{MERMAID_FILE} must be the vendored library, not a stub"
    );
    assert!(
        !html.contains(LIB_MARKER),
        "the page must not ALSO inline the library it now links"
    );
    assert!(
        html.contains(MERMAID_FILE),
        "the page must point the mermaid loader at its sibling copy"
    );
    // The whole point, in bytes: 3,803,736 before, ~231 KB after. A generous ceiling, so
    // this fails on a regression rather than on ordinary page growth.
    let bytes = fs::metadata(&index).unwrap().len();
    assert!(
        bytes < 1_000_000,
        "the page still carries the library: {bytes} B"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Content-gated exactly as the inline path already was: a page with no diagram must not
/// gain a 3.5 MB file it never loads.
#[test]
fn out_dir_writes_no_library_for_a_page_with_no_diagram() {
    let dir = scratch("prose");
    let doc = write_doc(&dir, "prose", "Just prose.\n");
    let out = dir.join("dist");
    let (ok, err) = build(&[doc.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    assert!(ok, "build --out exited non-zero: {err}");

    assert!(
        !out.join(MERMAID_FILE).exists(),
        "a diagram-free page must not get the mermaid library"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// The other half of the split, and the reason this is not a blanket change: `build
/// <file.tmd>` produces ONE file, so it keeps inlining. A single-file build that fetched a
/// sibling would stop being self-contained, which is that spelling's entire contract.
#[test]
fn a_single_file_build_still_inlines_the_library() {
    let dir = scratch("selfcontained");
    let doc = write_doc(&dir, "diagram", DIAGRAM);
    let (ok, err) = build(&[doc.to_str().unwrap()]);
    assert!(ok, "build exited non-zero: {err}");

    let html = fs::read_to_string(dir.join("diagram.html")).expect("build writes <stem>.html");
    assert!(
        html.contains(LIB_MARKER),
        "a single-file build must stay self-contained"
    );
    assert!(
        !dir.join(MERMAID_FILE).exists(),
        "a single-file build writes no sibling library"
    );
    let _ = fs::remove_dir_all(&dir);
}
