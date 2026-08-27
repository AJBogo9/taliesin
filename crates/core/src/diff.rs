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
    // SetMeta patches only the OUTER block element's `data-sourcepos`. A block whose
    // html carries more than one `data-sourcepos` (a fenced `:::` div with inner
    // blocks) would keep its inner sourcepos stale after a line-shifting edit above —
    // silently sending Ctrl-click and reverse cursor-sync *inside* the div to the wrong
    // line. For those, fall through to a full `Update`, which replaces the whole block
    // html and refreshes every inner `data-sourcepos`. (The client already applies
    // Update without losing block identity, keyed off the unchanged `data-block-id`.)
    let single_sourcepos = sourcepos_count(&new.html) <= 1;
    if single_sourcepos
        && old.sourcepos != new.sourcepos
        && eq_ignoring_sourcepos(&old.html, &new.html)
    {
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

/// How many `data-sourcepos` ATTRIBUTES the html carries. A leaf block has one (on its
/// outer element); a fenced `:::` div wraps inner blocks that each carry their own, so it
/// has more.
///
/// Prose that merely *mentions* the attribute carries it in the page's visible TEXT: comrak
/// does not escape `"` inside a `<code>` span, so a paragraph quoting
/// `data-sourcepos="5:1-5:9"` counted two and lost `SetMeta`. It then took a destructive
/// `Update` on every line-number shift above it — replacing the element, and with it any
/// live DOM state (an open `<details>`, a playing video, a `{js}` widget) that `SetMeta`
/// exists to preserve. `docs/internals/block-model.tmd`, the page that documents `SetMeta`,
/// mentions the attribute ten times.
///
/// **Two tiers, because this is the keystroke path.** [`diff_blocks`] asks this of every
/// matched block on every save, so answering it by walking each block's tags walks the whole
/// page: measured on `corpus/tech-blog/posts/em-algorithm` (287 KB, 55 blocks) **in 2026-08,
/// against that day's 11.9 ms warm edit**, the diff went from 319 µs to 1704 µs and the warm
/// edit from 11.9 ms to 13.4 ms. Those absolutes are historical — the warm edit is 2.6 ms
/// since 1.1.0, so the rejected walk would cost proportionally far more of it now, which
/// only strengthens the conclusion. The walk therefore runs
/// only where the ambiguity is real. [`sourcepos_mentions`] is an upper bound and cheap; at
/// most one mention cannot be ambiguous, because the block model gives EVERY block its own
/// `data-sourcepos` (`crates/core/tests/corpus.rs` enforces it), so a single mention is that
/// attribute and never text.
fn sourcepos_count(html: &str) -> usize {
    match sourcepos_mentions(html) {
        n @ (0 | 1) => n,
        _ => crate::render::attr_values(html, "data-sourcepos").count(),
    }
}

/// How many times the emitted spelling of the attribute appears anywhere in `html`, as
/// markup or as text. An upper bound on the number of real attributes, since every one the
/// renderer emits is written this way, and a substring scan rather than a parse.
fn sourcepos_mentions(html: &str) -> usize {
    html.matches(SOURCEPOS_KEY).count()
}

const SOURCEPOS_KEY: &str = "data-sourcepos=\"";

/// Two block htmls compared with the OUTER `data-sourcepos` value blanked and everything
/// else required to match byte for byte, so a pure line-number shift on the block itself
/// reads as equal while any other difference does not.
///
/// The outer attribute is the first mention: a block's html opens with its own element tag.
/// Blanking exactly that one is what `SetMeta` actually does — it patches the outer element
/// and nothing else — so anything further along, an inner block's attribute or a `<code>`
/// span quoting the name, has to survive the comparison verbatim. Blanking *every* value
/// instead (which is what this did) could call two blocks equal because their inner
/// sourcepos differences had been masked away, and only [`sourcepos_count`]'s separate
/// nested-block guard stopped that becoming a `SetMeta` that left those inner values stale.
///
/// Allocation-free: two `memcmp`s on slices of the originals. Building the masked copies
/// meant allocating and copying a whole block's html twice per compared pair, for every
/// block on the page, on every save.
fn eq_ignoring_sourcepos(a: &str, b: &str) -> bool {
    let (Some(ia), Some(ib)) = (a.find(SOURCEPOS_KEY), b.find(SOURCEPOS_KEY)) else {
        return a == b; // neither carries it, or only one does
    };
    let (from_a, from_b) = (ia + SOURCEPOS_KEY.len(), ib + SOURCEPOS_KEY.len());
    let (Some(len_a), Some(len_b)) = (a[from_a..].find('"'), b[from_b..].find('"')) else {
        return a == b; // malformed (no closing quote): nothing safe to blank
    };
    a[..from_a] == b[..from_b] && a[from_a + len_a..] == b[from_b + len_b..]
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
    // The reduction above is only sound while ids are unique. A duplicate would be
    // absorbed silently (`pos_in_b` keeps just the last position) and the diff
    // would emit ops against the wrong element. Both lists are checked because a
    // duplicate in `a` instead yields two pairs sharing a `new_idx`, of which the
    // strictly-increasing LIS can only ever keep one.
    debug_assert_eq!(
        pos_in_b.len(),
        b.len(),
        "duplicate block id in the new list; the LCS→LIS reduction assumes ids are unique"
    );
    debug_assert_eq!(
        a.iter().collect::<std::collections::HashSet<_>>().len(),
        a.len(),
        "duplicate block id in the old list; the LCS→LIS reduction assumes ids are unique"
    );
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
            nested: Vec::new(),
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
    #[should_panic(expected = "duplicate block id in the new list")]
    fn duplicate_new_id_trips_the_lis_uniqueness_assert() {
        // `pos_in_b` keeps only the last position of a repeated id, which silently
        // invalidates the LCS→LIS reduction. Catch it in debug/test builds instead
        // of emitting a corrupt op stream.
        lcs_pairs(&["a", "b"], &["a", "b", "a"]);
    }

    #[test]
    #[should_panic(expected = "duplicate block id in the old list")]
    fn duplicate_old_id_trips_the_lis_uniqueness_assert() {
        lcs_pairs(&["a", "b", "a"], &["a", "b"]);
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
    fn consecutive_inserts_in_one_gap_chain_each_after_the_previous() {
        // Two (or more) new blocks in a SINGLE gap must chain: the second inserts after the
        // first, not after the stale anchor before the gap. Otherwise the client runs both
        // `insertAfter(a)` and the pair lands in reverse DOM order. Every other insertion
        // test above inserts exactly one block per gap, so none exercises the `*prev_new`
        // chaining update inside the insert loop.
        let old = ids(&["a", "d"]);
        let new = ids(&["a", "b", "c", "d"]);
        let ops = diff_blocks(&old, &new);
        assert_eq!(
            ops,
            vec![
                BlockOp::Insert {
                    after_id: Some("a".into()),
                    html: block("b").html
                },
                BlockOp::Insert {
                    after_id: Some("b".into()),
                    html: block("c").html
                },
            ]
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
            nested: Vec::new(),
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
    fn nested_div_sourcepos_shift_is_a_full_update_not_setmeta() {
        // A fenced `:::` div carries its OWN data-sourcepos plus an inner block's. A
        // line-shifting edit above moves BOTH. SetMeta would patch only the outer one,
        // leaving the inner block's sourcepos stale (Ctrl-click + reverse cursor-sync
        // inside the div would jump to the wrong line). So the op must be a full
        // Update, which refreshes every inner data-sourcepos.
        let old = Block {
            id: "d".into(),
            sourcepos: "5:1-7:3".into(),
            source_file: None,
            html: "<div data-block-id=\"d\" data-sourcepos=\"5:1-7:3\">\
                   <p data-sourcepos=\"6:1-6:5\">Inner.</p></div>"
                .into(),
            cell: None,
            nested: Vec::new(),
        };
        let new = Block {
            sourcepos: "3:1-5:3".into(),
            html: "<div data-block-id=\"d\" data-sourcepos=\"3:1-5:3\">\
                   <p data-sourcepos=\"4:1-4:5\">Inner.</p></div>"
                .into(),
            ..old.clone()
        };
        assert_eq!(
            diff_blocks(std::slice::from_ref(&old), std::slice::from_ref(&new)),
            vec![BlockOp::Update {
                target_id: "d".into(),
                html: new.html,
            }],
            "a multi-sourcepos block must full-Update so inner sourcepos refresh"
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
            nested: Vec::new(),
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

    /// A paragraph that *quotes* `data-sourcepos="…"` must still get `SetMeta` when only
    /// its line numbers move.
    ///
    /// comrak does not escape `"` inside a `<code>` span, so the attribute name appears
    /// verbatim in the page's visible TEXT. `sourcepos_count` matched the string, counted
    /// two, and concluded the block wrapped inner blocks — so every line-number shift above
    /// such a paragraph took a destructive `Update` that replaces the element and discards
    /// whatever live DOM state it held. Reproduced in a browser against
    /// `docs/internals/block-model.tmd`, the page that documents `SetMeta`: one inserted
    /// line gave 34 `SetMeta` ops and one full replacement, and the replaced block was the
    /// one quoting the attribute.
    #[test]
    fn a_block_quoting_the_sourcepos_attribute_still_gets_set_meta() {
        let quoting = |pos: &str| {
            let mut b = block_html(
                "q",
                &format!(
                    "<p data-block-id=\"q\" data-sourcepos=\"{pos}\">every block carries \
                     <code>data-sourcepos=\"5:1-5:9\"</code>.</p>"
                ),
            );
            b.sourcepos = pos.to_string();
            b
        };
        let ops = diff_blocks(&[quoting("5:1-5:44")], &[quoting("7:1-7:44")]);
        assert!(
            matches!(ops.as_slice(), [BlockOp::SetMeta { target_id, sourcepos, .. }]
                     if target_id == "q" && sourcepos == "7:1-7:44"),
            "a shifted paragraph that merely mentions the attribute must patch its \
             metadata, not be replaced: {ops:?}"
        );

        // The narrowing must not go too far: a real `:::` div wrapping inner blocks that
        // each carry their own sourcepos still has more than one, and still takes an
        // `Update` — the whole reason the count exists.
        let wrapper = |pos: &str| {
            let mut b = block_html(
                "w",
                &format!(
                    "<div data-block-id=\"w\" data-sourcepos=\"{pos}\">\
                     <p data-sourcepos=\"{pos}\">inner</p></div>"
                ),
            );
            b.sourcepos = pos.to_string();
            b
        };
        assert!(
            matches!(
                diff_blocks(&[wrapper("1:1-2:9")], &[wrapper("3:1-4:9")]).as_slice(),
                [BlockOp::Update { .. }]
            ),
            "a div wrapping its own sourcepos-bearing children must still take an Update"
        );
    }

    /// The mask must blank attribute values only, never a value the prose quotes: two
    /// paragraphs differing solely in the sourcepos they *print* are different content, and
    /// patching one's metadata would leave the other's text on screen.
    #[test]
    fn masking_leaves_a_quoted_sourcepos_in_the_text_alone() {
        let a = "<p data-sourcepos=\"1:1-1:9\">shows <code>data-sourcepos=\"4:1-4:2\"</code></p>";
        let b = "<p data-sourcepos=\"9:1-9:9\">shows <code>data-sourcepos=\"8:1-8:2\"</code></p>";
        assert!(
            !eq_ignoring_sourcepos(a, b),
            "the quoted values are content, not metadata"
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
