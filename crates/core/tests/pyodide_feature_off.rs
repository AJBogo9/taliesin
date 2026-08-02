//! The feature-OFF half of the `{pyodide}` contract (backlog item 205).
//!
//! `tests/pyodide.rs` is gated on `required-features = ["pyodide"]` and pins the delivery
//! path when the runtime is compiled in. This file is its mirror: it runs in a DEFAULT build
//! and pins what a reader gets when it is not.
//!
//! The failure this exists to prevent is specific. Gating only the payload would still let
//! the renderer emit a live `<script type="application/tali-pyodide">` wrapper whose
//! `indexURL` meta is absent: an empty husk that loads nothing and shows the reader an error
//! box where the author's Python used to be. So the assertions below are mostly *negative*,
//! and negative assertions are where this repo's inlined-asset trap bites hardest: every page
//! inlines the whole JS bundle, and `assets/js/pyodide.js` calls
//! `registerLanguage("application/tali-pyodide", …)`, so a bare
//! `contains("application/tali-pyodide")` is TRUE on a correctly degraded page. Every needle
//! here is therefore the full opening tag `<script type="application/tali-pyodide"`, the same
//! prefix `has_pyodide_cell_markup` uses for exactly this reason.
//!
//! When the feature IS on this file compiles to nothing, so it never contradicts its twin.

#![cfg(not(feature = "pyodide"))]

use taliesin_core::render::{client_lang, client_lang_runnable};

/// The corpus document that pins the feature-on path, rendered the other way.
fn corpus_doc() -> (String, std::path::PathBuf) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/reactive");
    let src = std::fs::read_to_string(base.join("pyodide.tmd"))
        .expect("corpus/reactive/pyodide.tmd should exist");
    (src, base)
}

fn render(src: &str, base: &std::path::Path) -> taliesin_core::RenderedDoc {
    taliesin_core::render_document_with_includes(src, base)
}

/// The language stays registered. Only its ability to RUN is gated, and that distinction is
/// what keeps the diagnostics, the mime contract and the completion vocabulary from drifting
/// between the two builds.
#[test]
fn the_language_stays_registered_but_is_not_runnable() {
    let spec = client_lang("pyodide").expect("`{pyodide}` stays in CLIENT_LANGS feature-off");
    assert_eq!(spec.mime, "application/tali-pyodide");
    assert!(!client_lang_runnable("pyodide"));
    // The other client languages need no vendored payload and are unaffected.
    assert!(client_lang_runnable("js"));
    assert!(client_lang_runnable("glsl"));
}

/// The corpus document renders its `{pyodide}` cells as ordinary source blocks: no wrapper,
/// no runtime index, and (the part that makes it a *block* rather than a hole) every one of
/// them still carrying `data-block-id` and `data-sourcepos`, so click-to-source and the
/// incremental swap work on a feature-off build exactly as they do on any other page.
#[test]
fn the_corpus_document_renders_its_cells_as_source_blocks() {
    let (src, base) = corpus_doc();
    let doc = render(&src, &base);
    let body = doc.body_html();

    // The document must actually contain the cells, or every negative below is vacuous.
    let cell_count = src.matches("```{pyodide}").count();
    assert!(
        cell_count >= 2,
        "corpus/reactive/pyodide.tmd should still carry its `{{pyodide}}` cells (found \
         {cell_count}); without them this whole file asserts nothing"
    );

    // The husk, needled as the full opening tag (see the module comment).
    assert!(
        !body.contains("<script type=\"application/tali-pyodide\""),
        "feature-off must emit no live wrapper, or the reader gets an empty husk: {body}"
    );
    assert!(
        !body.contains("tali-pyodide-cell"),
        "no wrapper div either: {body}"
    );
    assert!(
        !body.contains("tali-pyodide-index"),
        "no runtime index meta: {body}"
    );

    // The positive half: the author's source is visible, and it is a real block.
    assert!(
        body.contains("data-tali-cell=\"pyodide\""),
        "the cell is still emitted as a source block the reader's show/hide-code control \
         can target: {body}"
    );
    let blocks: Vec<_> = doc
        .blocks
        .iter()
        .filter(|b| b.html.contains("data-tali-cell=\"pyodide\""))
        .collect();
    assert_eq!(
        blocks.len(),
        cell_count,
        "every `{{pyodide}}` cell should survive as its own block, not be swallowed"
    );
    for b in &blocks {
        assert!(
            b.html.contains("data-block-id=\"") && b.html.contains("data-sourcepos=\""),
            "a degraded cell is still a block and must carry both attrs: {}",
            b.html
        );
        assert!(!b.sourcepos.is_empty(), "block sourcepos must be populated");
    }
    // A distinctive literal from the document's own source, proving the code is on the page
    // rather than merely a shell with the right attributes.
    assert!(
        body.contains("default_rng") || body.contains("np.random"),
        "the author's Python must be visible on the page: {body}"
    );
}

/// The runtime is not compiled in, so there is nothing to serve or copy.
#[test]
fn the_payload_is_empty() {
    assert!(
        taliesin_core::render::pyodide_payload().is_empty(),
        "feature-off there are no vendored bytes in the binary"
    );
}

/// The per-language asset gate must go false for pyodide, so a feature-off page does not
/// ship the pyodide enhancer for cells that can never run.
///
/// Two documents, because the corpus one is deliberately mixed (it carries a `{js}` cell as
/// well, at `corpus/reactive/pyodide.tmd:28`). On the mixed page the SHARED runtime must stay
/// on, because `{js}` still runs and still needs it, while the pyodide-specific gate goes off; a
/// synthetic pyodide-only page then shows the shared gate going off too once nothing else
/// claims it. Asserting only the second would hide a regression that killed `{js}`.
#[test]
fn the_pyodide_asset_gate_goes_off_without_taking_js_with_it() {
    let (src, base) = corpus_doc();
    let body = render(&src, &base).body_html();
    assert!(
        !taliesin_core::render::has_client_cells_of(&body, "pyodide"),
        "the pyodide enhancer must not ship for cells that cannot run: {body}"
    );
    assert!(
        taliesin_core::render::has_client_cells_of(&body, "js"),
        "the `{{js}}` cell on the same page is unaffected and still needs its runtime: {body}"
    );
    assert!(
        taliesin_core::render::has_client_cells(&body),
        "the shared runtime stays on for the `{{js}}` cell's sake"
    );

    let only_py = render(
        "```{pyodide}\nimport numpy as np\nnp.arange(3).tolist()\n```\n",
        std::path::Path::new("."),
    )
    .body_html();
    assert!(
        !taliesin_core::render::has_client_cells(&only_py),
        "with nothing else claiming it, the shared runtime gate goes off too: {only_py}"
    );
}
