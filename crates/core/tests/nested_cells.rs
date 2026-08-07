//! Backlog item 210, the RENDER half: a `:::` container must hand the executor every code
//! cell it folds away, and leave the output slot that lets the executor put each output
//! back where the cell sits.
//!
//! The corpus walker renders but never executes, so this pins what render alone can prove —
//! `Block::nested`, the slots, their order, and which cells earn one. That the cells then
//! actually run is `crates/server/tests/nested_cell_executes.rs`, which needs a kernel.
//!
//! The fixture is `corpus/nested-cells.tmd`: one executable cell per container kind
//! (callout, tabset, layout grid, width escape, theorem, two levels deep) plus a `{js}`
//! cell, which must NOT earn a slot.

use std::path::PathBuf;
use taliesin_core::render::{Block, CELL_OUT_SLOT_ATTR};

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn fixture() -> taliesin_core::RenderedDoc {
    let base = corpus_dir();
    let path = base.join("nested-cells.tmd");
    let src = std::fs::read_to_string(&path).expect("corpus/nested-cells.tmd");
    taliesin_core::render_document_with_includes(&src, &base)
}

/// Every block that folded a cell away, with its nested cells.
fn containers(doc: &taliesin_core::RenderedDoc) -> Vec<&Block> {
    doc.blocks.iter().filter(|b| !b.nested.is_empty()).collect()
}

fn slot_of(id: &str) -> String {
    format!("{CELL_OUT_SLOT_ATTR}=\"{id}\"></div>")
}

/// One document, one count: eight `{python}` cells across six containers (the tabset and
/// the layout grid hold two each), and NOT the `{js}` one.
#[test]
fn every_python_cell_in_a_div_reaches_the_executor_and_the_js_one_does_not() {
    let doc = fixture();
    let nested: Vec<&Block> = containers(&doc)
        .into_iter()
        .flat_map(|b| &b.nested)
        .collect();

    assert_eq!(
        nested.len(),
        8,
        "expected the fixture's eight folded {{python}} cells, got {:?}",
        nested.iter().map(|b| &b.id).collect::<Vec<_>>()
    );
    for b in &nested {
        let lang = b.cell.as_ref().map(|c| c.lang.as_str());
        assert_eq!(
            lang,
            Some("python"),
            "a folded cell in a language the kernel does not run reached the executor: {b:?}"
        );
    }

    // The `{js}` cell is in a callout of its own and mounts client-side, so that callout
    // folds a cell but hands the executor nothing.
    let js_callout = doc
        .blocks
        .iter()
        .find(|b| b.html.contains("data-tali-js-src") || b.html.contains("tali-js"))
        .expect("the {js} cell's container");
    assert!(
        js_callout.nested.is_empty(),
        "a {{js}} cell never produces a server-side output block, so it must earn no slot: {}",
        js_callout.html
    );
    assert!(
        !js_callout.html.contains(CELL_OUT_SLOT_ATTR),
        "an output slot that can never fill: {}",
        js_callout.html
    );
}

/// Each nested cell's slot sits in the container's own HTML, keyed to that cell's id.
///
/// This is the whole placement contract: the executor finds the slot by the exact literal
/// this asserts, so a renderer that emitted the attribute in a different position (or
/// spelled the id differently) would leave every nested output on the floor — silently,
/// which is the failure mode item 210 was in the first place.
#[test]
fn each_folded_cell_has_an_empty_slot_keyed_to_its_own_id() {
    let doc = fixture();
    let containers = containers(&doc);
    assert!(!containers.is_empty(), "the fixture folded no cells at all");

    for c in containers {
        for cell in &c.nested {
            assert!(
                c.html.contains(&slot_of(&cell.id)),
                "no empty slot for folded cell {} in:\n{}",
                cell.id,
                c.html
            );
            // The slot carries the same `{id}-out` block id a top-level output block would,
            // which is what lets `client.js` stream into a nested cell's output with no
            // second lookup path.
            assert!(
                c.html
                    .contains(&format!("data-block-id=\"{}-out\"", cell.id)),
                "the slot for {} must be addressable as its output block:\n{}",
                cell.id,
                c.html
            );
        }
    }
}

/// A tabset's two cells stay in their own panels, in document order.
///
/// The reason the output goes in a slot rather than into a sibling block after the
/// container: a sibling would stack both tabs' outputs below the tabs, hidden one included.
#[test]
fn a_tabsets_cells_keep_their_panels_and_their_order() {
    let doc = fixture();
    let tabset = doc
        .blocks
        .iter()
        .find(|b| b.html.contains("class=\"panel-tabset\""))
        .expect("the tabset block");
    assert_eq!(tabset.nested.len(), 2, "{:?}", tabset.nested);

    let (first, second) = (&tabset.nested[0].id, &tabset.nested[1].id);
    let at = |needle: &str| tabset.html.find(needle).expect(needle);
    assert!(
        at(&slot_of(first)) < at(&slot_of(second)),
        "the tabs' slots are out of document order, so their outputs would swap"
    );
    // Each slot is inside its own panel: the second panel opens between them.
    let second_panel = tabset
        .html
        .find("hidden=\"until-found\"")
        .expect("a second panel");
    assert!(
        at(&slot_of(first)) < second_panel && at(&slot_of(second)) > second_panel,
        "a tab's output slot is not in that tab's own panel"
    );
}

/// A container two levels deep keeps its cell, flattened onto the outermost block.
///
/// `group_divs` closes innermost-first, so the inner container collects its own cell and
/// leaves the slot inside its own HTML; the outer one then takes that list wholesale. The
/// cell has to surface at the top level (the executor scans only top-level blocks) while
/// the slot stays at the depth it was written.
#[test]
fn a_cell_two_containers_deep_surfaces_on_the_outer_block() {
    let doc = fixture();
    let outer = doc
        .blocks
        .iter()
        .find(|b| b.html.contains("class=\"column-page\"") && b.html.contains("callout-tip"))
        .expect("the two-level container");
    assert_eq!(
        outer.nested.len(),
        1,
        "the inner container's cell must surface here: {:?}",
        outer.nested
    );
    assert!(
        outer.nested[0].nested.is_empty(),
        "nested cells are flattened to one level, so an entry carries none of its own"
    );
    // The slot is where the cell was written — inside the inner callout, not after it.
    let slot = outer
        .html
        .find(&slot_of(&outer.nested[0].id))
        .expect("slot");
    let callout_end = outer.html.rfind("callout-body").expect("the inner callout");
    assert!(
        slot > callout_end,
        "the slot escaped the inner callout it belongs to:\n{}",
        outer.html
    );
}
