//! The `hero:` landing-header primitive: the imageless header (the marketing site)
//! and the two-column portrait variant (`image:`, the blog homepage). Synthetic
//! one-page sites, so the contract is pinned independently of the real corpus docs.

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

/// An imageless hero renders the plain `.hero` header with no media wrapper, exactly
/// as before the portrait slot existed. The marketing site relies on this.
#[test]
fn imageless_hero_has_no_media_markup() {
    let html = home("hero:\n  eyebrow: WRITING\n  headline: A headline\n  lead: A lead.\n");
    assert!(
        html.contains("<header class=\"hero\""),
        "plain .hero header still emitted"
    );
    // Markup-specific negatives: the class names also appear in the inlined base.css,
    // so assert on the emitted header/wrappers, not bare substrings.
    assert!(
        !html.contains("class=\"hero hero-has-media\""),
        "no media class on an imageless hero header"
    );
    assert!(
        !html.contains("<div class=\"hero-body\">") && !html.contains("<img class=\"hero-media\""),
        "no media wrappers on an imageless hero"
    );
}

/// A hero with `image:` renders the two-column media variant: the `hero-has-media`
/// class, a `.hero-body` wrapper around the text, and a `.hero-media` portrait.
#[test]
fn hero_with_image_renders_portrait_media() {
    let html = home(
        "hero:\n  eyebrow: WRITING\n  headline: A headline\n  lead: A lead.\n  \
         image: profile.webp\n  image-alt: A face\n",
    );
    assert!(
        html.contains("class=\"hero hero-has-media\""),
        "media hero gets the hero-has-media class"
    );
    assert!(
        html.contains("<div class=\"hero-body\">"),
        "text is wrapped in .hero-body"
    );
    assert!(
        html.contains("<img class=\"hero-media\" src=\"profile.webp\" alt=\"A face\">"),
        "portrait emitted as .hero-media with src + alt"
    );
}
