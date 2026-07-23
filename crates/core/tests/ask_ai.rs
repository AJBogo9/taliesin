//! Server-observable contract for the Ask-AI hand-off feature.
//! Spec: notes/2026-07-23-ask-ai-handoff-design.md §9. The built book page must carry the
//! `<link rel="canonical">` the client keys off for Tier A, and ship the `19-ask-ai.js` asset.

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn course() -> Site {
    Site::discover(&corpus_dir().join("course"))
}

#[test]
fn course_book_emits_canonical_link_for_tier_a() {
    let html = course().render_page("mle.tmd").expect("mle renders");
    assert!(
        html.contains(r#"<link rel="canonical" href="https://course.example.edu/mle.html">"#),
        "book with url: must emit the canonical link the Ask-AI client keys off:\n{html}"
    );
}
