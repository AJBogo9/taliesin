//! The native flat `_quarto.yml` schema (and its Quarto-shaped fallback): parsing,
//! `chapters:`-implies-book inference, the `icon:` shorthand, and typo validation.

use qmd_fast_core::Site;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

struct TempProj(PathBuf);
impl TempProj {
    fn new(config: &str) -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "qmd-cfg-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("_quarto.yml"), config).unwrap();
        fs::write(p.join("index.qmd"), "---\ntitle: Home\n---\n\n# Hi\n").unwrap();
        TempProj(p)
    }
}
impl Drop for TempProj {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_flat_config_parses_nav_footer_and_icon() {
    let d = TempProj::new(
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
    let d = TempProj::new("title: \"Bk\"\nchapters:\n  - index.qmd\n");
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
    let d = TempProj::new("title: \"S\"\nfavicn: x.svg\n");
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
    let d = TempProj::new(
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
