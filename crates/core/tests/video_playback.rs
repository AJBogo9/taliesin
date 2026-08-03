//! Corpus pin for the `{{< video >}}` playback ladder (`corpus/media/screencast.tmd`).
//!
//! Native `controls` became the DEFAULT 2026-08-03 (visual minimalism pass): hover-play
//! and the click-to-lightbox touch path were both deleted, so the browser's own control
//! bar is the reader's only way to start a clip at all. `controls=false` is the escape
//! hatch for a deliberately non-interactive decorative clip, and `audio` still narrates.
//!
//! Two traps this file is written around:
//!
//! 1. **The inlined-asset needle trap.** Every Taliesin page inlines the whole CSS+JS
//!    payload, and both mention `controls`, so a page-wide `contains("controls")` passes on
//!    a page that renders no video at all. Every assertion here needles a complete
//!    `<video …></video>` tag.
//! 2. **A one-sided assertion.** Asserting only that the default clip carries `controls`
//!    would leave the `controls=false` escape hatch unpinned, and a regression that started
//!    ignoring it would ship silently. Both directions are held.

mod common;
use common::corpus_dir;

fn page() -> String {
    let dir = corpus_dir().join("media");
    let src = std::fs::read_to_string(dir.join("screencast.tmd")).expect("screencast.tmd");
    taliesin_core::render_document_with_includes(&src, &dir).body_html()
}

#[test]
fn the_default_clip_gets_native_controls() {
    let h = page();
    assert!(
        h.contains(
            "<video src=\"tour.mp4\" muted loop controls playsinline preload=\"metadata\" \
             tabindex=\"0\" aria-label=\"The default: native controls, muted and looping \
             until the reader presses play.\"></video>"
        ),
        "with hover-play and the lightbox both deleted, native `controls` is the reader's \
         only play path and must ship by default: {h}"
    );
}

#[test]
fn the_audio_flag_unmutes_unloops_and_carries_its_caption_track() {
    let h = page();
    assert!(
        h.contains(
            "<video src=\"tour.mp4\" controls playsinline preload=\"metadata\" tabindex=\"0\" \
             aria-label=\"With `audio` and `captions=`: a narrated explainer.\">\
             <track kind=\"captions\" src=\"tour.vtt\" label=\"Captions\" default></video>"
        ),
        "a narrated clip: controls, no `muted`, no `loop`, and a caption track (WCAG 1.2.2): {h}"
    );
}

#[test]
fn controls_false_opts_the_clip_out_of_the_control_bar() {
    let h = page();
    assert!(
        h.contains(
            "<video src=\"tour.mp4\" muted loop playsinline preload=\"metadata\" \
             tabindex=\"0\" aria-label=\"With `controls=false`: no control bar at \
             all.\"></video>"
        ),
        "an explicit `controls=false` is honoured — the DEFAULT flipped, the escape hatch \
         did not: {h}"
    );
}

#[test]
fn a_flag_written_before_the_path_is_not_taken_for_the_clip() {
    let h = page();
    assert!(
        h.contains(
            "<video src=\"tour.mp4\" muted loop controls playsinline preload=\"metadata\" \
             tabindex=\"0\" aria-label=\"The flags are not paths, so this still plays \
             `tour.mp4`.\"></video>"
        ),
        "`{{< video controls tour.mp4 >}}` plays tour.mp4: {h}"
    );
    assert!(
        !h.contains("src=\"controls\"") && !h.contains("src=\"audio\""),
        "a bare flag must never be mistaken for the positional source path: {h}"
    );
}

#[test]
fn every_media_file_the_pin_names_exists_on_disk() {
    // The pin asserts on emitted paths, which would stay green if the fixture media were
    // deleted — and `taliesin check`'s local-media diagnostic would then be the only thing
    // that noticed, on a document no `check` run covers.
    let dir = corpus_dir().join("media");
    for f in ["tour.mp4", "tour.vtt"] {
        assert!(dir.join(f).is_file(), "corpus/media/{f} must exist");
    }
}
