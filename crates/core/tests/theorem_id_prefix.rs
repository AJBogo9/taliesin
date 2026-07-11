//! A theorem/lemma/... div is numbered and displayed ("Theorem 1") from its
//! `.theorem` class alone, but `@id` only resolves when the id carries a
//! cross-reference kind prefix (`thm-`, `lem-`, …; see `cite::is_xref_anchor`).
//! An id without that prefix (`#pythagoras`) is silently unreferenceable: it is
//! numbered, but `@pythagoras` renders as literal text and `check` used to report
//! "no problems found". These tests pin the render-time warning that catches it,
//! and its absence when the id is correct (no false positive).

use std::path::Path;

fn warnings(src: &str) -> Vec<String> {
    let doc = taliesin_core::render_document_with_includes(src, Path::new("."));
    doc.warnings.into_iter().map(|w| w.message).collect()
}

#[test]
fn theorem_id_without_a_kind_prefix_is_flagged_as_unreferenceable() {
    let src =
        "---\ntitle: T\n---\n\n::: {.theorem #pythagoras}\nThe square of the hypotenuse.\n:::\n";
    let ws = warnings(src);
    assert!(
        ws.iter()
            .any(|m| m.contains("pythagoras") && m.contains("cross-referenc")),
        "a bare theorem id must warn it cannot be cross-referenced: {ws:?}"
    );
    assert!(
        ws.iter().any(|m| m.contains("thm-pythagoras")),
        "the warning must suggest the kind's prefix (`thm-pythagoras`): {ws:?}"
    );
}

#[test]
fn a_correctly_prefixed_theorem_id_does_not_warn() {
    let src = "---\ntitle: T\n---\n\n::: {.theorem #thm-pythagoras}\nThe square of the hypotenuse.\n:::\n";
    let ws = warnings(src);
    assert!(
        !ws.iter().any(|m| m.contains("cross-referenc")),
        "a `#thm-` id is referenceable and must not warn: {ws:?}"
    );
}

#[test]
fn a_theorem_without_any_id_does_not_warn() {
    // No id at all is a deliberate unnumbered-or-uncited theorem, not a mistake.
    let src = "---\ntitle: T\n---\n\n::: {.theorem}\nAn anonymous statement.\n:::\n";
    let ws = warnings(src);
    assert!(
        !ws.iter().any(|m| m.contains("cross-referenc")),
        "an id-less theorem must not warn: {ws:?}"
    );
}

#[test]
fn the_suggested_prefix_matches_the_theorem_kind() {
    let src = "---\ntitle: T\n---\n\n::: {.lemma #zorn}\nEvery chain has an upper bound.\n:::\n";
    let ws = warnings(src);
    assert!(
        ws.iter().any(|m| m.contains("lem-zorn")),
        "a lemma's suggestion must use the `lem-` prefix, not `thm-`: {ws:?}"
    );
}
