//! The native flat `_quarto.yml` schema (and its Quarto-shaped fallback): parsing,
//! `chapters:`-implies-book inference, the `icon:` shorthand, and typo validation.

use qmd_fast_core::Site;

mod common;
use common::TempProj;

/// A throwaway site project: `_quarto.yml` = `config`, plus a minimal `index.qmd`
/// (so `Site::discover` always has a home page).
fn site(config: &str) -> TempProj {
    let d = TempProj::new();
    d.file("_quarto.yml", config);
    d.file("index.qmd", "---\ntitle: Home\n---\n\n# Hi\n");
    d
}

#[test]
fn native_flat_config_parses_nav_footer_and_icon() {
    let d = site(
        "title: \"My Site\"\n\
         nav:\n  - { text: Home, href: index.qmd }\n\
         footer:\n  left: \"© 2026\"\n  right:\n    - { icon: github, href: \"https://github.com/x\" }\n",
    );
    let site = Site::discover(&d.0);
    assert!(!site.is_book(), "a config without chapters is a website");
    assert_eq!(site.config.title.as_deref(), Some("My Site"));
    assert_eq!(site.config.nav.left.len(), 1, "nav list -> left side");
    assert!(
        site.warnings.is_empty(),
        "clean config: {:?}",
        site.warnings
    );

    let html = site.render_page("index.qmd").expect("renders");
    assert!(html.contains("My Site"), "brand from title");
    assert!(
        html.contains("aria-label=\"github\"") && html.contains("viewBox=\"0 0 16 16\""),
        "icon: github should render the bundled SVG"
    );
}

#[test]
fn chapters_present_infers_a_book() {
    let d = site("title: \"Bk\"\nchapters:\n  - index.qmd\n");
    let site = Site::discover(&d.0);
    assert!(
        site.is_book(),
        "chapters: present ⇒ a book, no type: needed"
    );
    assert_eq!(site.output_dir(), "_book");
}

#[test]
fn unknown_native_key_is_warned_with_a_suggestion() {
    // `favicn` is a typo of `favicon`
    let d = site("title: \"S\"\nfavicn: x.svg\n");
    let site = Site::discover(&d.0);
    assert!(
        site.warnings
            .iter()
            .any(|w| w.contains("favicn") && w.contains("favicon")),
        "expected a did-you-mean warning, got: {:?}",
        site.warnings
    );
}

#[test]
fn quarto_shaped_config_still_works_via_fallback() {
    // The old nested shape must keep parsing into the same model.
    let d = site(
        "project:\n  type: website\nwebsite:\n  title: \"Old\"\n  navbar:\n    left:\n      - { text: Home, href: index.qmd }\n",
    );
    let site = Site::discover(&d.0);
    assert!(!site.is_book());
    assert_eq!(site.config.title.as_deref(), Some("Old"));
    assert_eq!(site.config.nav.left.len(), 1);
    // a Quarto config carries keys we ignore — but it must NOT trip the native
    // typo validator (that only runs on the native shape).
    assert!(
        site.warnings.is_empty(),
        "fallback warnings: {:?}",
        site.warnings
    );
}
