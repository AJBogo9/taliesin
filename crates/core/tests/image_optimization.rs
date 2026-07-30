//! Corpus pin for image optimization (backlog item 169), render half.
//!
//! The corpus walker only *renders*, so this pins what a render can know: the intrinsic
//! dimensions, the loading policy, and the LCP exception. The AVIF transcode is build-only
//! and is pinned by `crates/server/src/image_opt.rs`'s own tests.
//!
//! **Every assertion needles a full emitted tag.** A whole-page `contains("loading=\"lazy\"")`
//! would pass on a page with no image at all, because every Taliesin page inlines the whole
//! CSS/JS payload — a trap that has cost this project a debugging round more than once
//! (`notes/LESSONS.md`).

use std::path::Path;

fn render_pin() -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/media");
    let src = std::fs::read_to_string(dir.join("optimized-images.tmd"))
        .expect("the corpus pin document exists");
    taliesin_core::render_document_with_includes(&src, &dir).body_html()
}

#[test]
fn the_first_image_is_eager_and_the_rest_are_lazy() {
    let html = render_pin();
    // Full tags. `fit-a.png` is 535x428 and is the document's first image.
    assert!(
        html.contains(
            r#"<img src="fit-a.png" alt="A model fit, at a size that earns two width rungs." width="535" height="428" fetchpriority="high" />"#
        ),
        "the first image must be eager, at its intrinsic size:\n{}",
        img_tags(&html)
    );
    // Every later image carries the lazy pair, and none carries fetchpriority.
    let later: Vec<&str> = img_tags_vec(&html).into_iter().skip(1).collect();
    assert!(!later.is_empty(), "the pin must have more than one image");
    for tag in &later {
        assert!(
            tag.contains(r#"loading="lazy""#) && tag.contains(r#"decoding="async""#),
            "a non-first image must be lazy: {tag}"
        );
        assert!(
            !tag.contains("fetchpriority"),
            "only the first image is prioritized: {tag}"
        );
    }
}

#[test]
fn an_image_below_the_smallest_rung_still_states_its_own_size() {
    let html = render_pin();
    // fit-small.png is 320x164 — narrower than the 480 rung. The render half does not know
    // about rungs, but the dimensions it states are what the build's never-upscale rule reads.
    assert!(
        html.matches(r#"width="320" height="164""#).count() >= 2,
        "both references to fit-small.png must carry its intrinsic size:\n{}",
        img_tags(&html)
    );
}

#[test]
fn every_local_raster_image_in_the_pin_is_annotated() {
    // Guards against the annotation silently covering only the figure path or only the
    // inline path: the pin deliberately contains both, plus a repeat of one file.
    let html = render_pin();
    let tags = img_tags_vec(&html);
    assert_eq!(tags.len(), 3, "expected 3 images in the pin:\n{tags:#?}");
    for tag in &tags {
        assert!(
            tag.contains(r#"width=""#) && tag.contains(r#"height=""#),
            "unannotated image survived: {tag}"
        );
    }
}

fn img_tags_vec(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("<img ") {
        let from = &rest[at..];
        let end = from.find('>').map(|e| e + 1).unwrap_or(from.len());
        out.push(&from[..end]);
        rest = &from[end..];
    }
    out
}

fn img_tags(html: &str) -> String {
    img_tags_vec(html).join("\n")
}
