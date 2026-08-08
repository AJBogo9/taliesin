//! Interaction pin for the "interactive-explainer" demand-probe persona
//! (corpus/descent/, a single-page gradient-descent explorable explanation).
//! Locks the feature *combinations* the single-feature corpus docs never exercise
//! together on one page: `{{< input >}}` sliders driving a "once" `{js}` cell that
//! also drags, a `.scrolly` whose sticky `{js}` graphic keys off the scene, a
//! reactive Plot cell reading the same sliders, and two numbered figures with
//! resolved cross-references, all with the block-id/sourcepos invariants intact.
//! See notes/2026-07-22-corpus-demand-probe-interactive-explainer.md for the
//! findings this produced. Behavior (drag, reactivity, scene changes) is verified
//! in the browser, not here; this test pins the *static* structure those rely on.

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn page() -> String {
    Site::discover(&corpus_dir().join("descent"))
        .render_page("index.tmd")
        .expect("descent index renders")
}

#[test]
fn three_reactive_input_sliders_emit() {
    let h = page();
    for (name, max) in [("lr", "0.35"), ("beta", "0.9"), ("steps", "60")] {
        assert!(
            h.contains(&format!("data-tali-input=\"{name}\""))
                && h.contains(&format!("max=\"{max}\"")),
            "reactive slider {name} (max {max}) emits as a range control: {h}"
        );
    }
}

#[test]
fn headline_is_a_once_cell_that_drags_and_subscribes() {
    let h = page();
    // The load-bearing combination: the headline cell is a "once" cell (it declares
    // no `//| input:`, so the DAG never tears it down), yet it stays live by
    // subscribing with tali.onInput AND owns a pointer-drag. If any of these three
    // regress, sliders and drag can no longer coexist without state loss.
    assert!(
        h.contains("tali.onInput([\"lr\", \"beta\", \"steps\"], redraw)"),
        "the headline cell subscribes to the sliders via onInput (redraw without teardown): {h}"
    );
    assert!(
        h.contains("setPointerCapture"),
        "the start point is draggable via pointer capture: {h}"
    );
    // Pin the once-ness structurally: the `<script>` that carries the onInput call
    // must declare no data-inputs, else the DAG would re-run (and rebuild) it on every
    // slider move, throwing away the dragged start point.
    let at = h
        .find("tali.onInput")
        .expect("the headline cell is present");
    let script_open = h[..at]
        .rfind("<script")
        .expect("onInput sits inside a <script>");
    let tag_end = h[script_open..].find('>').expect("the script tag closes") + script_open;
    let open_tag = &h[script_open..=tag_end];
    assert!(
        !open_tag.contains("data-inputs"),
        "the headline (onInput) cell is a once cell — no data-inputs: {open_tag}"
    );
}

#[test]
fn plot_cell_is_a_reactive_sink_over_the_same_sliders() {
    let h = page();
    assert!(
        h.contains("data-inputs=\"lr,beta,steps\""),
        "the loss-vs-iteration cell re-runs on every slider (a reactive sink): {h}"
    );
    assert!(
        h.contains("Plot.plot("),
        "that cell draws with the vendored Observable Plot: {h}"
    );
}

#[test]
fn five_named_scenes_drive_one_graphic_from_a_select_control() {
    let h = page();
    assert!(
        h.contains("data-inputs=\"scene\""),
        "the graphic is a {{js}} cell keyed off the scene value: {h}"
    );
    // The reader picks the scene, so the control's option list IS the scene vocabulary:
    // a scene the cell branches on but the control cannot select is unreachable.
    for state in ["landscape", "gradient", "step", "iterate", "diverge"] {
        assert!(
            h.contains(&format!(">{state}</option>")),
            "the scene control offers '{state}': {h}"
        );
    }
}

#[test]
fn two_figures_number_and_their_crossrefs_resolve() {
    let h = page();
    assert!(
        h.contains("id=\"fig-landscape\"") && h.contains("id=\"fig-momentum\""),
        "both authored SVG figures are present: {h}"
    );
    // Numbering increments across the two figures, and the inline @fig- refs resolve
    // to a link carrying the same number (not a bare "Figure").
    assert!(
        h.contains("<figcaption>Figure&nbsp;1:") && h.contains("<figcaption>Figure&nbsp;2:"),
        "figures number 1 then 2: {h}"
    );
    assert!(
        h.contains("href=\"#fig-landscape\" class=\"tali-xref\">Figure&nbsp;1</a>"),
        "@fig-landscape resolves to Figure 1: {h}"
    );
    assert!(
        h.contains("href=\"#fig-momentum\" class=\"tali-xref\">Figure&nbsp;2</a>"),
        "@fig-momentum resolves to Figure 2: {h}"
    );
}

#[test]
fn math_and_callouts_render_alongside_the_interactives() {
    let h = page();
    assert!(
        h.contains("class=\"katex"),
        "display/inline math renders via KaTeX: {h}"
    );
    assert!(
        h.contains("callout-warning") && h.contains("callout-note"),
        "the step-size warning and takeaways note render: {h}"
    );
}

#[test]
fn interactive_blocks_keep_the_block_model_invariants() {
    let h = page();
    // An `{{< input >}}` control expands to raw HTML mid-render; it must still come out
    // of the block model carrying the block-id + sourcepos the incremental client and
    // click-to-source key off. (corpus.rs enforces this document-wide; this pins it on
    // the interactive blocks directly, which is where a shortcode-expanded block is
    // easiest to lose.)
    let open = h
        .find("data-tali-input=\"scene\"")
        .expect("the scene control renders");
    let block = &h[..open];
    let tag = block
        .rfind("<div")
        .map(|i| &block[i..])
        .expect("the control sits inside a block div");
    assert!(
        tag.contains("data-block-id=") && tag.contains("data-sourcepos="),
        "the control's block keeps data-block-id + data-sourcepos: {tag}"
    );
}
