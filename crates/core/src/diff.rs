//! Diff two block lists into a minimal set of DOM operations.
//!
//! Block ids are content hashes and unique within a document, so an unchanged
//! block keeps its id and is left untouched (this is what preserves scroll and
//! the runtime state of live blocks like Three.js/`{js}` cells). Changed regions are
//! aligned positionally between the stable anchors (an LCS over ids) and turned
//! into in-place updates, with extras as inserts/removes.

use crate::render::Block;

/// A DOM mutation for the preview client. Ids in `target_id`/`after_id` refer to
/// elements currently in the DOM; the `html` carries the block's new id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockOp {
    /// Replace the element `target_id` with `html` (which has the new id).
    Update { target_id: String, html: String },
    /// Insert `html` after `after_id` (or at the start when `None`).
    Insert {
        after_id: Option<String>,
        html: String,
    },
    /// Remove the element `target_id`.
    Remove { target_id: String },
    /// Patch only the position metadata (`data-sourcepos` / `data-source-file`) of
    /// the element `target_id`, leaving its rendered content — and its live DOM state
    /// (video playback, `{js}` widgets, open `<details>`) — in place. Emitted when a
    /// structural edit elsewhere shifts an unchanged block's line numbers: its
    /// content-hash id and body are identical, only the attribute moved.
    SetMeta {
        target_id: String,
        sourcepos: String,
        source_file: Option<String>,
    },
}

/// Produce the ops that transform the `old` block sequence into `new`.
pub fn diff_blocks(old: &[Block], new: &[Block]) -> Vec<BlockOp> {
    let a: Vec<&str> = old.iter().map(|b| b.id.as_str()).collect();
    let b: Vec<&str> = new.iter().map(|b| b.id.as_str()).collect();
    let anchors = lcs_pairs(&a, &b);

    let mut ops = Vec::new();
    let mut oi = 0;
    let mut nj = 0;
    let mut prev_new: Option<String> = None;
    for (ai, bj) in &anchors {
        emit_gap(&mut ops, old, new, oi..*ai, nj..*bj, &mut prev_new);
        // Matched id, but the HTML can still differ: either only the block's
        // `data-sourcepos` moved (a structural edit elsewhere shifted its lines —
        // patch the attribute, keeping its live DOM state) or its content genuinely
        // changed (a *derived* block whose id isn't a content hash, e.g. a code
        // cell's output keyed to the cell id, re-run upstream — full re-render).
        if old[*ai].html != new[*bj].html {
            ops.push(anchor_op(&old[*ai], &new[*bj]));
        }
        prev_new = Some(new[*bj].id.clone()); // the anchor now precedes
        oi = ai + 1;
        nj = bj + 1;
    }
    emit_gap(
        &mut ops,
        old,
        new,
        oi..old.len(),
        nj..new.len(),
        &mut prev_new,
    );
    // Apply every Remove before any Insert. A reorder splits a moved block into a
    // Remove (its old slot) + an Insert (its new slot) of the *same* id; if the
    // Insert is emitted first (a move toward the front), the client's id-based
    // lookup for the Remove matches the just-inserted element and deletes the wrong
    // one. Removes are positionally independent, so hoisting them is safe; this
    // stable sort keeps Inserts in document order (so after_id chains still hold).
    ops.sort_by_key(|op| !matches!(op, BlockOp::Remove { .. }));
    ops
}

/// The op for an id-matched anchor whose html changed. If *only* the position
/// metadata moved (same content-hashed body, just a shifted `data-sourcepos`), patch
/// the attribute in place via `SetMeta` so the element's live DOM state survives a
/// structural edit elsewhere. Otherwise the content actually changed (a derived
/// block such as a cell's output), so re-render it with a full `Update`.
fn anchor_op(old: &Block, new: &Block) -> BlockOp {
    if old.sourcepos != new.sourcepos && eq_ignoring_sourcepos(&old.html, &new.html) {
        BlockOp::SetMeta {
            target_id: new.id.clone(),
            sourcepos: new.sourcepos.clone(),
            source_file: new.source_file.clone(),
        }
    } else {
        BlockOp::Update {
            target_id: new.id.clone(),
            html: new.html.clone(),
        }
    }
}

/// Two block htmls compared with every `data-sourcepos="…"` value blanked, so a
/// pure line-number shift reads as equal (the rest of the markup — content,
/// `data-block-id`, `data-source-file` — must still match exactly).
fn eq_ignoring_sourcepos(a: &str, b: &str) -> bool {
    mask_sourcepos(a) == mask_sourcepos(b)
}

fn mask_sourcepos(html: &str) -> String {
    const KEY: &str = "data-sourcepos=\"";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(i) = rest.find(KEY) {
        out.push_str(&rest[..i + KEY.len()]);
        rest = &rest[i + KEY.len()..];
        match rest.find('"') {
            Some(q) => rest = &rest[q..], // drop the value; keep the closing quote on
            None => break,                // malformed (no closing quote): stop masking
        }
    }
    out.push_str(rest);
    out
}

/// Turn one gap (a run of old + new blocks between anchors) into ops: pair them
/// as in-place updates, then surplus old -> removes, surplus new -> inserts.
fn emit_gap(
    ops: &mut Vec<BlockOp>,
    old: &[Block],
    new: &[Block],
    o: std::ops::Range<usize>,
    n: std::ops::Range<usize>,
    prev_new: &mut Option<String>,
) {
    let (o0, o1) = (o.start, o.end);
    let (n0, n1) = (n.start, n.end);
    let pairs = (o1 - o0).min(n1 - n0);
    for k in 0..pairs {
        ops.push(BlockOp::Update {
            target_id: old[o0 + k].id.clone(),
            html: new[n0 + k].html.clone(),
        });
        *prev_new = Some(new[n0 + k].id.clone());
    }
    for k in pairs..(o1 - o0) {
        ops.push(BlockOp::Remove {
            target_id: old[o0 + k].id.clone(),
        });
    }
    for k in pairs..(n1 - n0) {
        ops.push(BlockOp::Insert {
            after_id: prev_new.clone(),
            html: new[n0 + k].html.clone(),
        });
        *prev_new = Some(new[n0 + k].id.clone());
    }
}

/// Longest common subsequence over two id sequences, returned as matched index
/// pairs `(old_idx, new_idx)` in increasing order.
///
/// Block ids are unique within a document (`make_id` disambiguates collisions; the
/// corpus tests assert it), so a common subsequence is exactly a set of shared ids
/// in the same relative order — i.e. the longest run of new-list positions that is
/// increasing when the shared ids are taken in old-list order. That reduces the LCS
/// to a Longest Increasing Subsequence, solved in O(n log n) time and O(n) space via
/// patience sorting. (The textbook O(m·n) DP table would allocate tens of MB on
/// every keystroke-save once a document reaches a few thousand blocks.)
fn lcs_pairs(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    // New-list position of each id (unique ⇒ exactly one position per id).
    let mut pos_in_b: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::with_capacity(b.len());
    for (j, &id) in b.iter().enumerate() {
        pos_in_b.insert(id, j);
    }
    // The shared ids as `(old_idx, new_idx)` in old-list order; the LCS is the
    // longest strictly-increasing-by-`new_idx` subsequence of this.
    let seq: Vec<(usize, usize)> = a
        .iter()
        .enumerate()
        .filter_map(|(i, id)| pos_in_b.get(id).map(|&j| (i, j)))
        .collect();
    // Patience sorting: `tails[k]` is the `seq`-index of the smallest tail of an
    // increasing run of length `k + 1`; `prev` links each element to the tail of the
    // run it extends, so the actual subsequence can be rebuilt.
    let mut tails: Vec<usize> = Vec::new();
    let mut prev: Vec<usize> = vec![usize::MAX; seq.len()];
    for (s, &(_, j)) in seq.iter().enumerate() {
        let k = tails.partition_point(|&t| seq[t].1 < j);
        if k > 0 {
            prev[s] = tails[k - 1];
        }
        if k == tails.len() {
            tails.push(s);
        } else {
            tails[k] = s;
        }
    }
    // Walk back from the longest run's tail to recover the pairs, then put them in
    // increasing order.
    let mut pairs = Vec::new();
    let mut cur = tails.last().copied();
    while let Some(s) = cur {
        pairs.push(seq[s]);
        cur = (prev[s] != usize::MAX).then_some(prev[s]);
    }
    pairs.reverse();
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(id: &str) -> Block {
        Block {
            id: id.to_string(),
            sourcepos: "1:1-1:1".to_string(),
            source_file: None,
            html: format!("<p data-block-id=\"{id}\">{id}</p>"),
            cell: None,
        }
    }

    fn block_html(id: &str, html: &str) -> Block {
        Block {
            html: html.to_string(),
            ..block(id)
        }
    }

    fn ids(blocks: &[&str]) -> Vec<Block> {
        blocks.iter().map(|s| block(s)).collect()
    }

    #[test]
    fn same_id_changed_html_updates_in_place() {
        // A code cell's output block keeps its id (keyed to the cell) but its
        // content changes when an upstream cell re-runs.
        let old = vec![block_html("a", "<div>old output</div>")];
        let new = vec![block_html("a", "<div>new output</div>")];
        let ops = diff_blocks(&old, &new);
        assert_eq!(
            ops,
            vec![BlockOp::Update {
                target_id: "a".into(),
                html: "<div>new output</div>".into()
            }]
        );
    }

    #[test]
    fn identical_lists_produce_no_ops() {
        let v = ids(&["a", "b", "c"]);
        assert!(diff_blocks(&v, &v).is_empty());
    }

    #[test]
    fn single_edit_in_place_is_one_update() {
        // middle block edited: its content-hash id changes a -> a2.
        let old = ids(&["a", "b", "c"]);
        let new = ids(&["a", "b2", "c"]);
        let ops = diff_blocks(&old, &new);
        assert_eq!(ops.len(), 1);
        assert_eq!(
            ops[0],
            BlockOp::Update {
                target_id: "b".into(),
                html: block("b2").html
            }
        );
    }

    #[test]
    fn insertion_references_preceding_block() {
        let old = ids(&["a", "c"]);
        let new = ids(&["a", "b", "c"]);
        let ops = diff_blocks(&old, &new);
        assert_eq!(
            ops,
            vec![BlockOp::Insert {
                after_id: Some("a".into()),
                html: block("b").html
            }]
        );
    }

    #[test]
    fn insertion_at_start_has_no_after() {
        let ops = diff_blocks(&ids(&["b"]), &ids(&["a", "b"]));
        assert_eq!(
            ops,
            vec![BlockOp::Insert {
                after_id: None,
                html: block("a").html
            }]
        );
    }

    #[test]
    fn removal_emits_remove() {
        let ops = diff_blocks(&ids(&["a", "b", "c"]), &ids(&["a", "c"]));
        assert_eq!(
            ops,
            vec![BlockOp::Remove {
                target_id: "b".into()
            }]
        );
    }

    #[test]
    fn reorder_reconstructs_via_remove_plus_insert() {
        // There is no Move op: swapping a/b keeps one block's identity (b here,
        // an anchor) and rebuilds the other as remove+insert. The trailing c
        // (an anchor) stays untouched. This documents the content-hash-id
        // limitation: a *moved* live block loses its runtime state.
        let ops = diff_blocks(&ids(&["a", "b", "c"]), &ids(&["b", "a", "c"]));
        assert!(
            ops.iter().all(|op| !format!("{op:?}").contains("\"c\"")),
            "the unmoved anchor c must not appear in any op: {ops:?}"
        );
        assert_eq!(
            ops,
            vec![
                BlockOp::Remove {
                    target_id: "a".into()
                },
                BlockOp::Insert {
                    after_id: Some("b".into()),
                    html: block("a").html
                },
            ]
        );
    }

    #[test]
    fn matched_id_with_only_a_sourcepos_shift_is_a_metadata_patch() {
        // Same content-hash id and body; only data-sourcepos moved (a structural
        // edit above shifted this block's lines). A SetMeta, not a re-render.
        let old = Block {
            id: "a".into(),
            sourcepos: "5:1-5:6".into(),
            source_file: None,
            html: "<p data-block-id=\"a\" data-sourcepos=\"5:1-5:6\">Body.</p>".into(),
            cell: None,
        };
        let new = Block {
            sourcepos: "3:1-3:6".into(),
            html: "<p data-block-id=\"a\" data-sourcepos=\"3:1-3:6\">Body.</p>".into(),
            ..old.clone()
        };
        assert_eq!(
            diff_blocks(std::slice::from_ref(&old), std::slice::from_ref(&new)),
            vec![BlockOp::SetMeta {
                target_id: "a".into(),
                sourcepos: "3:1-3:6".into(),
                source_file: None,
            }]
        );
    }

    #[test]
    fn matched_id_with_a_real_content_change_is_still_a_full_update() {
        // A derived block (a cell's output, keyed to the cell id) whose content
        // changed: even though its sourcepos also moved, the body differs, so the
        // mask comparison fails and it's a full Update (not a metadata patch).
        let old = Block {
            id: "out".into(),
            sourcepos: "5:1-5:6".into(),
            source_file: None,
            html: "<div data-block-id=\"out\" data-sourcepos=\"5:1-5:6\">old</div>".into(),
            cell: None,
        };
        let new = Block {
            sourcepos: "3:1-3:6".into(),
            html: "<div data-block-id=\"out\" data-sourcepos=\"3:1-3:6\">NEW</div>".into(),
            ..old.clone()
        };
        assert_eq!(
            diff_blocks(std::slice::from_ref(&old), std::slice::from_ref(&new)),
            vec![BlockOp::Update {
                target_id: "out".into(),
                html: new.html,
            }]
        );
    }

    #[test]
    fn unchanged_blocks_around_edit_are_untouched() {
        // a, <big live block L>, c ; edit only c.
        let old = ids(&["a", "L", "c"]);
        let new = ids(&["a", "L", "c2"]);
        let ops = diff_blocks(&old, &new);
        // L must not appear in any op.
        assert!(ops.iter().all(|op| !format!("{op:?}").contains("\"L\"")));
        assert_eq!(ops.len(), 1);
    }
}
