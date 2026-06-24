use live_edit_bench::measure_live_edit;
use std::path::Path;

/// A synthetic doc: a paragraph and a collapsible callout (which renders as a
/// `<details>`). Inserting a paragraph ABOVE everything shifts the callout's line
/// numbers but not its content, so the diff must patch it in place (`SetMeta`),
/// never replace it (`Update`), which is what keeps its open/closed DOM state alive.
const SYNTHETIC: &str = "\
# Title

First paragraph above the callout.

::: {.callout-note collapse=\"true\"}
## Note
Body of the collapsible note.
:::

More text after the callout.
";

#[test]
fn edit_above_preserves_the_collapsible_dom_node() {
    let m = measure_live_edit("synthetic", SYNTHETIC, Path::new("."), |s| {
        s.replace(
            "First paragraph",
            "A freshly typed line.\n\nFirst paragraph",
        )
    });
    assert!(
        m.dom_preserved,
        "the <details> block below the edit should get a SetMeta (same DOM node), got metrics: {m:?}"
    );
    assert_eq!(
        m.update_count, 0,
        "no block below the edit should be re-rendered (no Update), got: {m:?}"
    );
    assert!(
        m.insert_count >= 1,
        "the new paragraph is an Insert, got: {m:?}"
    );
    assert!(
        m.set_meta_count >= 1,
        "shifted blocks below are SetMeta, got: {m:?}"
    );
}

/// On a real corpus doc, the warm-edit payload (the BlockOps sent over the wire) is
/// far smaller than the full page HTML a reload re-sends. (em-algorithm has a
/// `collapse=\"true\"` callout and `{python}` cells, which render as source here.)
#[test]
fn warm_edit_payload_is_far_smaller_than_full_render() {
    let doc = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/posts/em-algorithm/index.qmd"
    );
    let src = std::fs::read_to_string(doc).expect("read em-algorithm corpus doc");
    let base = Path::new(doc).parent().unwrap();
    let m = measure_live_edit("em-algorithm", &src, base, |s| {
        s.replace(
            "Let's start from a practical example.",
            "A freshly typed opening line.\n\nLet's start from a practical example.",
        )
    });
    assert!(
        m.edit_payload_bytes * 10 < m.full_html_bytes,
        "payload {} should be far below full html {} (ratio guard), metrics: {m:?}",
        m.edit_payload_bytes,
        m.full_html_bytes
    );
    assert!(
        m.dom_preserved,
        "the collapse callout below the edit should survive, got: {m:?}"
    );
}
