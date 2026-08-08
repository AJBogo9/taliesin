//! The `hero:` landing-header primitive. Synthetic one-page sites, so the contract is
//! pinned independently of the real corpus docs.
//!
//! There is one variant. The two-column portrait (`hero.image:`/`image-alt:`) was retired
//! on 2026-08-02 and its emitter deleted on 2026-08-08: the hero banner is type, not a
//! figure, and an image belongs in the page body as a normal figure.

use taliesin_core::Site;

mod common;
use common::TempProj;

/// A throwaway one-page site whose `index.tmd` front matter is `fm`; returns the
/// rendered home page HTML.
fn home(fm: &str) -> String {
    let d = TempProj::new();
    d.file("_site.yml", "title: T\n");
    d.file("index.tmd", &format!("---\n{fm}---\n\nBody.\n"));
    Site::discover(&d.0)
        .render_page("index.tmd")
        .expect("home renders")
}

/// The hero renders one plain `.hero` header with no media wrapper.
///
/// The negatives are markup-specific on purpose: the class names would also appear in an
/// inlined stylesheet, so a bare substring check could not tell "the CSS survived" from
/// "the emitter survived", and this test has to see the second one.
#[test]
fn the_hero_is_a_plain_header_with_no_media_markup() {
    for fm in [
        "hero:\n  eyebrow: WRITING\n  headline: A headline\n  lead: A lead.\n",
        // The retired keys must not resurrect the portrait layout: an author who still
        // has them in a document gets the plain header plus a located warning, not a
        // half-styled two-column banner.
        "hero:\n  eyebrow: WRITING\n  headline: A headline\n  lead: A lead.\n  \
         image: profile.webp\n  image-alt: A face\n",
    ] {
        let html = home(fm);
        assert!(
            html.contains("<header class=\"hero\""),
            "plain .hero header still emitted for {fm:?}"
        );
        assert!(
            !html.contains("class=\"hero hero-has-media\""),
            "no media class on the hero header for {fm:?}"
        );
        assert!(
            !html.contains("<div class=\"hero-body\">")
                && !html.contains("<img class=\"hero-media\""),
            "no media wrappers on the hero for {fm:?}"
        );
        assert!(
            !html.contains("profile.webp"),
            "a retired `hero.image:` must not reach the page for {fm:?}"
        );
    }
}
