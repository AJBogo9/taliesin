//! `taliesin symbols <file.tmd>` emits the document's cross-reference targets, so the
//! editor's `@`-completion can offer every one of them.
//!
//! The fixture is `corpus/reader/hovercards.tmd`, which carries both shapes an anchor can
//! take: a brace anchor (`## Why it works {#sec-why}`) and a *cell* label
//! (`%%| label: fig-flow`). The companion's completion harvested only the first with a
//! `/\{#([\w-]+)\}/` regex, so cell-labeled figures, tables and listings — the majority of
//! the corpus's cross-reference targets — were invisible. Emitting the registry Rust
//! already builds is what keeps the two from drifting.
//!
//! `symbols` must stay parse-only: an editor calls it on a keystroke, and it must never
//! boot a Jupyter kernel. `hovercards.tmd`'s `fig-flow` is a cell label, and it resolves
//! here without one.

use std::process::Command;

fn corpus(rel: &str) -> String {
    format!("{}/../../corpus/{rel}", env!("CARGO_MANIFEST_DIR"))
}

fn symbols(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(args)
        .output()
        .expect("run taliesin symbols");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn json_lists_cell_labeled_and_brace_anchored_targets() {
    let (ok, stdout, stderr) = symbols(&[
        "symbols",
        &corpus("reader/hovercards.tmd"),
        "--format",
        "json",
    ]);
    assert!(ok, "symbols should succeed; stderr: {stderr}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must be valid JSON ({e}):\n{stdout}"));
    let ids: Vec<&str> = parsed
        .as_array()
        .expect("a JSON array")
        .iter()
        .map(|s| s["id"].as_str().expect("each symbol has a string id"))
        .collect();

    // The bug: a `%%| label: fig-flow` cell was invisible to the editor.
    assert!(
        ids.contains(&"fig-flow"),
        "cell-labeled target missing: {ids:?}"
    );
    // The no-regression: a brace anchor is still offered.
    assert!(
        ids.contains(&"sec-why"),
        "brace-anchored target missing: {ids:?}"
    );

    // Each symbol carries the kind prefix and the resolved number the registry assigned.
    let fig = parsed
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "fig-flow")
        .unwrap();
    assert_eq!(fig["kind"], "fig", "got: {fig}");
    assert_eq!(fig["number"], "1", "got: {fig}");

    // Deterministic: sorted by id, so a diff of two runs is empty.
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "symbols must be sorted by id");
}

#[test]
fn the_human_format_is_the_default_and_lists_the_same_targets() {
    let (ok, stdout, stderr) = symbols(&["symbols", &corpus("reader/hovercards.tmd")]);
    assert!(ok, "symbols should succeed; stderr: {stderr}");
    assert!(stdout.contains("fig-flow"), "got:\n{stdout}");
    assert!(stdout.contains("sec-why"), "got:\n{stdout}");
}

#[test]
fn an_unknown_format_is_a_hard_error_with_a_did_you_mean() {
    let (ok, _out, err) = symbols(&[
        "symbols",
        &corpus("reader/hovercards.tmd"),
        "--formt",
        "json",
    ]);
    assert!(!ok, "a typo'd flag must not silently run with defaults");
    assert!(
        err.contains("--format"),
        "expected a did-you-mean; got: {err}"
    );
}

#[test]
fn an_unknown_format_value_is_rejected() {
    let (ok, _out, err) = symbols(&[
        "symbols",
        &corpus("reader/hovercards.tmd"),
        "--format",
        "yaml",
    ]);
    assert!(!ok, "an unknown --format value must fail");
    assert!(err.contains("yaml"), "got: {err}");
}

#[test]
fn a_directory_is_rejected_with_a_helpful_message() {
    let (ok, _out, err) = symbols(&["symbols", &corpus("reader")]);
    assert!(!ok, "a directory is not a single document");
    assert!(err.contains("is a directory"), "got: {err}");
}

#[test]
fn a_missing_path_prints_usage() {
    let (ok, _out, err) = symbols(&["symbols"]);
    assert!(!ok);
    assert!(err.contains("usage: taliesin symbols"), "got: {err}");
}

/// `symbols` answers "what can I write after `@`", so it must not offer an anchor that
/// `@` can never resolve. A `.theorem` div registers whatever id it is given, but
/// `cite` only links an anchor whose prefix names a cross-reference kind, so a
/// `::: {.theorem #pythagoras}` is numbered and displayed yet is unreferenceable:
/// `@pythagoras` stays literal text.
#[test]
fn an_anchor_that_cannot_be_referenced_is_not_a_symbol() {
    let dir = std::env::temp_dir().join(format!("tali-symbols-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("thm.tmd");
    std::fs::write(
        &doc,
        "---\ntitle: t\n---\n\n\
         ::: {.theorem #pythagoras title=\"Pythagoras\"}\nText.\n:::\n\n\
         ::: {.theorem #thm-good title=\"Good\"}\nText.\n:::\n",
    )
    .unwrap();

    let (ok, stdout, stderr) = symbols(&["symbols", doc.to_str().unwrap(), "--format", "json"]);
    assert!(ok, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let ids: Vec<&str> = parsed
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&"thm-good"),
        "a referenceable theorem is a symbol: {ids:?}"
    );
    assert!(
        !ids.contains(&"pythagoras"),
        "`@pythagoras` never resolves, so it must not be offered: {ids:?}"
    );
    // Every symbol therefore carries a real kind prefix; none is blank.
    for s in parsed.as_array().unwrap() {
        assert_ne!(s["kind"], "", "a symbol with no kind: {s}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
