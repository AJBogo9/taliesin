use live_edit_bench::measure_live_edit;
use std::path::Path;

/// A synthetic doc with the two block shapes the live-edit moat treats differently:
/// a raw `<details>` (a single-`data-sourcepos`, stateful element) and a `:::` collapse
/// callout (a fenced div that also carries its inner blocks' `data-sourcepos`).
/// Inserting a paragraph ABOVE everything shifts every block's line numbers but not
/// its content, so:
///   - the single-`sourcepos` `<details>` is patched in place (`SetMeta`), keeping its
///     open/closed DOM state alive — this is the moat;
///   - the multi-`sourcepos` callout is re-rendered (`Update`) so its inner
///     `data-sourcepos` refresh (Ctrl-click / reverse cursor-sync inside the div must
///     not go stale). That is the deliberate 2026-06-30 diff-hardening tradeoff — see
///     `diff::nested_div_sourcepos_shift_is_a_full_update_not_setmeta`.
const SYNTHETIC: &str = "\
# Title

First paragraph above.

<details><summary>State</summary>Open/closed state lives here.</details>

::: {.callout-note collapse=\"true\"}
## Note
Body of the collapsible note.
:::

More text after.
";

#[test]
fn edit_above_preserves_a_single_sourcepos_stateful_block() {
    let m = measure_live_edit("synthetic", SYNTHETIC, Path::new("."), |s| {
        s.replace(
            "First paragraph",
            "A freshly typed line.\n\nFirst paragraph",
        )
    });
    // The moat: a single-`sourcepos` stateful block (the raw `<details>`) below the
    // edit keeps its DOM node via a `SetMeta`, so its open/closed state survives.
    assert!(
        m.dom_preserved,
        "the single-sourcepos <details> below the edit should keep its DOM node via SetMeta, got: {m:?}"
    );
    assert!(
        m.insert_count >= 1,
        "the new paragraph is an Insert, got: {m:?}"
    );
    assert!(
        m.set_meta_count >= 1,
        "shifted single-sourcepos blocks are SetMeta, got: {m:?}"
    );
    // The `:::` callout is a multi-`sourcepos` fenced div: it is deliberately
    // re-rendered (`Update`) so its inner `data-sourcepos` refresh, unlike the
    // pure-leaf blocks. Documenting the tradeoff keeps this benchmark honest.
    assert!(
        m.update_count >= 1,
        "the fenced callout should re-render as an Update (inner sourcepos refresh), got: {m:?}"
    );
}

/// On a real corpus doc, the warm-edit payload (the BlockOps sent over the wire) is
/// far smaller than the full page HTML a reload re-sends. (em-algorithm has a
/// `collapse=\"true\"` callout and `{python}` cells, which render as source here.)
#[test]
fn warm_edit_payload_is_far_smaller_than_full_render() {
    let doc = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/tech-blog/posts/em-algorithm/index.tmd"
    );
    let src = std::fs::read_to_string(doc).expect("read em-algorithm corpus doc");
    let base = Path::new(doc).parent().unwrap();
    let m = measure_live_edit("em-algorithm", &src, base, |s| {
        s.replace(
            "Let's start from a practical example.",
            "A freshly typed opening line.\n\nLet's start from a practical example.",
        )
    });
    // The warm-edit payload is far below a full reload (measured 9.03x on 2026-08-02).
    // It is not the 83x this bench once published: since the 2026-06-30 diff hardening
    // (`6cdbc218`), `:::` fenced divs (this doc's collapse callout) re-render as a full
    // `Update` rather than a cheap `SetMeta`, so their whole html rides in the payload:
    // the deliberate cost of keeping their inner `data-sourcepos` fresh. That one op is
    // 90% of the payload. The floor is 8x rather than 5x so a repeat of that 10x shift
    // fails here instead of silently invalidating `RESULTS.md`.
    assert!(
        m.edit_payload_bytes * 8 < m.full_html_bytes,
        "payload {} should be far below full html {} (ratio guard), metrics: {m:?}",
        m.edit_payload_bytes,
        m.full_html_bytes
    );
    // Pin the op SHAPE exactly. This is the gate that was missing: the 2026-06-30
    // hardening moved `update_count` 0 -> 1 and grew the payload 10x, and because only
    // a loose 5x ratio floor was asserted, nothing failed and `RESULTS.md` kept
    // publishing the pre-hardening number for five weeks. Any future change to which
    // blocks are `SetMeta`-eligible moves one of these counts and must be accompanied
    // by regenerating `RESULTS.md` + `RESULTS.json`.
    assert_eq!(
        (m.insert_count, m.update_count, m.remove_count),
        (1, 1, 0),
        "op shape changed: exactly one Insert (the typed paragraph), one Update (the \
         single multi-`data-sourcepos` block, the collapse callout) and no Removes. \
         Regenerate RESULTS.md/RESULTS.json if this change is intended. metrics: {m:?}"
    );
    assert!(
        m.dom_preserved,
        "a stateful single-sourcepos block below the edit should survive, got: {m:?}"
    );
}
