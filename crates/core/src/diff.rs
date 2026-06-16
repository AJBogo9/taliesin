//! Diff two block lists into a minimal set of DOM operations.
//!
//! Block ids are content hashes and unique within a document, so an unchanged
//! block keeps its id and is left untouched (this is what preserves scroll and
//! the runtime state of live blocks like Three.js/OJS). Changed regions are
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
        // Matched id, but the HTML can still differ for *derived* blocks whose id
        // isn't a hash of their content (e.g. a code cell's output block, keyed to
        // the cell id, whose content changes when an upstream cell re-runs).
        if old[*ai].html != new[*bj].html {
            ops.push(BlockOp::Update {
                target_id: new[*bj].id.clone(),
                html: new[*bj].html.clone(),
            });
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
    ops
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

/// Longest common subsequence over two id sequences (unique elements), returned
/// as matched index pairs `(old_idx, new_idx)` in increasing order.
fn lcs_pairs(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < m && j < n {
        if a[i] == b[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
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
