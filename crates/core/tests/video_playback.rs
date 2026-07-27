//! Corpus pin for the `{{< video >}}` playback ladder (`corpus/media/screencast.tmd`).
//!
//! The shortcode used to give a reader no player controls at all: no play/pause, no
//! scrubber, no volume, and a hard-coded `muted`, which made a narrated clip impossible
//! (backlog item 73). `controls` and `audio` fix that with the browser's own player rather
//! than a 50-150 KB player library, so what has to stay pinned is the exact emitted tag.
//!
//! Two traps this file is written around:
//!
//! 1. **The inlined-asset needle trap.** Every Taliesin page inlines the whole CSS+JS
//!    payload, and both mention `controls`, so a page-wide `contains("controls")` passes on
//!    a page that renders no video at all. Every assertion here needles a complete
//!    `<video …></video>` tag.
//! 2. **A one-sided assertion.** Asserting only that the opt-in tag *gains* `controls`
//!    leaves "emit `controls` unconditionally" green, which would silently replace the
//!    hover-preview screencast the marketing site is built on. The default clip is pinned
//!    as an exact tag too, so both directions are held.

mod common;
use common::corpus_dir;

fn page() -> String {
    let dir = corpus_dir().join("media");
    let src = std::fs::read_to_string(dir.join("screencast.tmd")).expect("screencast.tmd");
    taliesin_core::render_document_with_includes(&src, &dir).body_html()
}

#[test]
fn the_default_clip_is_a_silent_screencast_with_no_control_bar() {
    let h = page();
    assert!(
        h.contains(
            "<video src=\"tour.mp4\" muted loop playsinline preload=\"metadata\" tabindex=\"0\" \
             aria-label=\"The default: a silent screencast, started by the reader.\"></video>"
        ),
        "the default stays the bare hover-preview screencast — no controls, still muted + \
         looping, still labelled: {h}"
    );
}

#[test]
fn the_controls_flag_gives_the_reader_the_browsers_own_player() {
    let h = page();
    assert!(
        h.contains(
            "<video src=\"tour.mp4\" muted loop controls playsinline preload=\"metadata\" \
             tabindex=\"0\" aria-label=\"With `controls`: the browser's own player, still \
             silent.\"></video>"
        ),
        "`controls` emits the native control bar (scrubber, keyboard, fullscreen, PiP) while \
         the clip stays a silent screencast: {h}"
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
