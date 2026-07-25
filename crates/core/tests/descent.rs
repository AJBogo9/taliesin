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
fn scrolly_has_five_named_scenes_driving_a_sticky_cell() {
    let h = page();
    assert!(
        h.contains("class=\"tali-scrolly\""),
        "the scrolly section renders: {h}"
    );
    assert!(
        h.contains("data-inputs=\"scene\""),
        "the sticky graphic is a {{js}} cell keyed off the scene value: {h}"
    );
    for state in ["landscape", "gradient", "step", "iterate", "diverge"] {
        assert!(
            h.contains(&format!("data-state=\"{state}\"")),
            "scrolly step drives scene '{state}': {h}"
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
    // The scrolly steps are generated blocks; they must still carry the block-id +
    // sourcepos the incremental client and click-to-source key off. (corpus.rs
    // enforces this document-wide; this pins it on the interactive blocks directly.)
    let open = h.find("class=\"step\"").expect("a scrolly step renders");
    let state = h[open..]
        .find("data-state=\"landscape\"")
        .expect("the first step drives the landscape scene");
    let step_tag = &h[open..open + state];
    assert!(
        step_tag.contains("data-block-id=") && step_tag.contains("data-sourcepos="),
        "a scrolly step block keeps data-block-id + data-sourcepos: {step_tag}"
    );
}
