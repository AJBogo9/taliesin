//! The incremental-update seam: a real edit to a document, rendered through the
//! full pipeline, must diff into the minimal DOM ops the preview client applies.
//! `diff.rs` unit-tests the diff over synthetic id lists; this proves the two
//! load-bearing properties hold end-to-end on *rendered* output — content-hash
//! ids stay stable for untouched blocks, and change only where the source did,
//! so an edit re-renders one block and leaves live blocks' state intact.

use qmd_fast_core::{BlockOp, diff_blocks, render_document};

#[test]
fn editing_one_paragraph_is_a_single_in_place_update() {
    let v1 = render_document("# Title\n\nAlpha.\n\nBeta.\n\nGamma.\n");
    let v2 = render_document("# Title\n\nAlpha.\n\nBeta EDITED.\n\nGamma.\n");
    let ops = diff_blocks(&v1.blocks, &v2.blocks);

    assert_eq!(ops.len(), 1, "one edit -> one op: {ops:?}");
    // The edited block updates in place, targeting its OLD (v1) id; the html is
    // the freshly rendered block.
    assert_eq!(
        ops[0],
        BlockOp::Update {
            target_id: v1.blocks[2].id.clone(),
            html: v2.blocks[2].html.clone(),
        }
    );

    // The surrounding blocks keep identical ids across the edit — this is what
    // preserves scroll position and the runtime state of live blocks.
    assert_eq!(v1.blocks[0].id, v2.blocks[0].id, "title id drifted");
    assert_eq!(v1.blocks[1].id, v2.blocks[1].id, "Alpha id drifted");
    assert_eq!(v1.blocks[3].id, v2.blocks[3].id, "Gamma id drifted");
    for keep in [&v1.blocks[0].id, &v1.blocks[1].id, &v1.blocks[3].id] {
        assert!(
            !format!("{ops:?}").contains(keep.as_str()),
            "untouched block {keep} leaked into ops: {ops:?}"
        );
    }
}

#[test]
fn appending_a_paragraph_is_a_single_insert_after_the_last_block() {
    let v1 = render_document("Alpha.\n\nBeta.\n");
    let v2 = render_document("Alpha.\n\nBeta.\n\nGamma.\n");
    let ops = diff_blocks(&v1.blocks, &v2.blocks);
    assert_eq!(
        ops,
        vec![BlockOp::Insert {
            after_id: Some(v1.blocks[1].id.clone()),
            html: v2.blocks[2].html.clone(),
        }]
    );
}

#[test]
fn deleting_the_last_paragraph_is_a_single_remove() {
    // Removing the *trailing* block shifts no line numbers below it, so it's a
    // lone Remove (the unshifted clean case).
    let v1 = render_document("Alpha.\n\nBeta.\n\nGamma.\n");
    let v2 = render_document("Alpha.\n\nBeta.\n");
    let ops = diff_blocks(&v1.blocks, &v2.blocks);
    assert_eq!(
        ops,
        vec![BlockOp::Remove {
            target_id: v1.blocks[2].id.clone(),
        }]
    );
}

#[test]
fn structural_edit_preserves_live_blocks_below_via_metadata_only_op() {
    // A middle delete shifts the *line numbers* of every block below it. Block ids
    // are content hashes (sourcepos-independent), so those blocks stay anchors and
    // are NOT recreated. Their *only* change is data-sourcepos, so the diff emits a
    // lightweight SetMeta (patch the attribute) instead of a full Update: the
    // element, and its live DOM state (video playback, OJS widgets, open
    // <details>), is left untouched while click-to-source stays exact.
    let v1 = render_document("Alpha.\n\nBeta.\n\nGamma.\n");
    let v2 = render_document("Alpha.\n\nGamma.\n");
    let ops = diff_blocks(&v1.blocks, &v2.blocks);
    assert_eq!(
        ops,
        vec![
            BlockOp::Remove {
                target_id: v1.blocks[1].id.clone(), // Beta
            },
            BlockOp::SetMeta {
                target_id: v1.blocks[2].id.clone(), // Gamma, line-shifted
                sourcepos: v2.blocks[1].sourcepos.clone(),
                source_file: v2.blocks[1].source_file.clone(),
            },
        ]
    );
    // Gamma's id is identical before and after: it's patched in place, never
    // recreated.
    assert_eq!(
        v1.blocks[2].id, v2.blocks[1].id,
        "Gamma's id must be stable"
    );
}

#[test]
fn re_rendering_unchanged_source_produces_no_ops() {
    let src = "# Doc\n\nStable body.\n\n## Section\n\nMore.\n";
    let ops = diff_blocks(&render_document(src).blocks, &render_document(src).blocks);
    assert!(ops.is_empty(), "a no-op edit must produce no ops: {ops:?}");
}
