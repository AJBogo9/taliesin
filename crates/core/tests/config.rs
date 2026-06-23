//! The native flat `_site.yml` schema: parsing, `chapters:`-implies-book
//! inference, the `icon:` shorthand, and typo validation.

use qmd_fast_core::Site;

mod common;
use common::TempProj;

/// A throwaway site project: `_site.yml` = `config`, plus a minimal `index.qmd`
/// (so `Site::discover` always has a home page).
fn site(config: &str) -> TempProj {
    let d = TempProj::new();
    d.file("_site.yml", config);
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
fn quarto_shaped_config_is_no_longer_parsed_and_warns() {
    // The compat shim is gone: the native flat schema is the only path. A
    // Quarto-shaped config no longer translates — its nested values are not
    // lifted, and its now-unknown top-level keys warn.
    let d = site(
        "project:\n  type: website\nwebsite:\n  title: \"Old\"\n  navbar:\n    left:\n      - { text: Home, href: index.qmd }\n",
    );
    let site = Site::discover(&d.0);
    assert!(!site.is_book());
    assert_eq!(
        site.config.title, None,
        "a nested `website.title` must not be parsed by the native schema"
    );
    assert!(
        site.config.nav.left.is_empty(),
        "a nested `website.navbar` must not be parsed by the native schema"
    );
    // The native typo validator flags the unrecognized top-level keys.
    assert!(
        site.warnings.iter().any(|w| w.contains("project")),
        "expected an unknown-key warning for `project`, got: {:?}",
        site.warnings
    );
    assert!(
        site.warnings.iter().any(|w| w.contains("website")),
        "expected an unknown-key warning for `website`, got: {:?}",
        site.warnings
    );
}
