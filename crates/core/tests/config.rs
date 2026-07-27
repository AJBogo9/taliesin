//! The native flat `_site.yml` schema: parsing, `chapters:`-implies-book
//! inference, the `icon:` shorthand, and typo validation.

use taliesin_core::Site;

mod common;
use common::TempProj;

/// A throwaway site project: `_site.yml` = `config`, plus a minimal `index.tmd`
/// (so `Site::discover` always has a home page).
fn site(config: &str) -> TempProj {
    let d = TempProj::new();
    d.file("_site.yml", config);
    d.file("index.tmd", "---\ntitle: Home\n---\n\n# Hi\n");
    d
}

#[test]
fn native_flat_config_parses_nav_footer_and_icon() {
    let d = site(
        "title: \"My Site\"\n\
         nav:\n  - { text: Home, href: index.tmd }\n\
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

    let html = site.render_page("index.tmd").expect("renders");
    assert!(html.contains("My Site"), "brand from title");
    assert!(
        html.contains("aria-label=\"github\"") && html.contains("viewBox=\"0 0 16 16\""),
        "icon: github should render the bundled SVG"
    );
}

/// A site's `head:`, `body-start:` and `body-end:` (from `_site.yml`) are each injected into
/// every page: head inside <head>, body-start ahead of the content, body-end after it. The
/// tech-blog dropped its own head:/body-*: (Quarto nav-prefetch residue), so this synthetic
/// site is the injection mechanism's net. `body-start:` had no test of its own, and the
/// ordering asserts distinguish the before/after slots so a swap can't pass on presence alone.
#[test]
fn site_head_body_start_and_body_end_are_injected() {
    let d = TempProj::new();
    d.file(
        "_site.yml",
        "title: T\n\
         head:\n  text: '<meta name=\"probe-head\" content=\"1\">'\n\
         body-start:\n  text: '<span id=\"probe-body-start\"></span>'\n\
         body-end:\n  text: '<span id=\"probe-body-end\"></span>'\n",
    );
    d.file("index.tmd", "---\ntitle: Home\n---\n\n# Content Anchor\n");
    let site = Site::discover(&d.0);
    let html = site.render_page("index.tmd").expect("renders");

    let head = &html[..html.find("</head>").expect("has </head>")];
    assert!(
        head.contains("probe-head"),
        "head: must be injected inside <head>"
    );

    // body-start lands ahead of the content, body-end after it — assert the ordering, not just
    // presence, so swapping the before_body/after_body include slots fails the test.
    let start = html.find("probe-body-start").expect("body-start present");
    let content = html.find("Content Anchor").expect("content present");
    let end = html.find("probe-body-end").expect("body-end present");
    assert!(
        start < content,
        "body-start: must be injected ahead of the page content"
    );
    assert!(
        content < end,
        "body-end: must be injected after the page content"
    );
}

#[test]
fn scholarly_citation_meta_for_authored_dated_posts_only() {
    let d = TempProj::new();
    d.file(
        "_site.yml",
        "title: \"My Journal\"\nurl: \"https://ex.org\"\nnav:\n  - { text: Home, href: index.tmd }\n",
    );
    // An article: author + date -> gets citation_* (Google Scholar) meta.
    d.file(
        "post.tmd",
        "---\ntitle: On Gradients\nauthor:\n  - Ada Lovelace\n  - Alan Turing\ndate: 2026-01-15\n---\n\nBody.\n",
    );
    // A plain page: no author/date -> NO citation_* meta.
    d.file("index.tmd", "---\ntitle: Home\n---\n\n# Hi\n");
    let site = Site::discover(&d.0);

    let post = site.render_page("post.tmd").expect("renders");
    assert!(
        post.contains("<meta name=\"citation_title\" content=\"On Gradients\">"),
        "citation_title missing: {post}"
    );
    assert!(
        post.contains("<meta name=\"citation_author\" content=\"Ada Lovelace\">")
            && post.contains("<meta name=\"citation_author\" content=\"Alan Turing\">"),
        "one citation_author per author expected"
    );
    assert!(
        post.contains("<meta name=\"citation_publication_date\" content=\"2026-01-15\">"),
        "citation_publication_date missing"
    );
    assert!(
        post.contains("<meta name=\"citation_journal_title\" content=\"My Journal\">"),
        "citation_journal_title (site title) missing"
    );

    let home = site.render_page("index.tmd").expect("renders");
    assert!(
        !home.contains("citation_"),
        "a non-article (no author/date) must not emit citation_* meta"
    );
}

#[test]
fn chapters_present_infers_a_book() {
    let d = site("title: \"Bk\"\nchapters:\n  - index.tmd\n");
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
fn logo_is_a_known_key_but_a_typo_of_it_still_warns() {
    // Registering `logo:` has to move BOTH ways: the real key must stop drawing the
    // unknown-key diagnostic, and a near-miss must still draw it (a key added to the
    // parser but not to NATIVE_KEYS warns on correct config; one added to neither is
    // silently inert).
    let d = site("title: \"S\"\nlogo: brand.svg\n");
    let s = Site::discover(&d.0);
    assert!(
        !s.warnings.iter().any(|w| w.contains("logo")),
        "a correctly spelled `logo:` must not be diagnosed: {:?}",
        s.warnings
    );
    let d = site("title: \"S\"\nlogos: brand.svg\n");
    let s = Site::discover(&d.0);
    assert!(
        s.warnings
            .iter()
            .any(|w| w.contains("logos") && w.contains("logo")),
        "expected a did-you-mean warning for `logos`, got: {:?}",
        s.warnings
    );
}

#[test]
fn legacy_shaped_config_is_no_longer_parsed_and_warns() {
    // The compat shim is gone: the native flat schema is the only path. A
    // legacy nested config no longer translates; its nested values are not
    // lifted, and its now-unknown top-level keys warn.
    let d = site(
        "project:\n  type: website\nwebsite:\n  title: \"Old\"\n  navbar:\n    left:\n      - { text: Home, href: index.tmd }\n",
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

/// A `{{< embed deck.tmd >}}` living inside an `{{< include >}}`d partial must still be
/// discovered as a deck (otherwise the deck flattens to a chrome-wrapped article, its
/// slides leak into search, and the embed iframe loads the wrong page).
#[test]
fn embed_inside_an_included_partial_is_discovered_as_a_deck() {
    let d = TempProj::new();
    d.file("_site.yml", "title: S\n");
    d.file(
        "index.tmd",
        "---\ntitle: Home\n---\n\n{{< include _includes/_talk.tmd >}}\n",
    );
    d.file(
        "_includes/_talk.tmd",
        "Here is a talk:\n\n{{< embed slides.tmd title=\"Talk\" >}}\n",
    );
    d.file(
        "slides.tmd",
        "---\ntitle: Slides\nformat: deck\n---\n\n## One\n\n## Two\n",
    );
    let site = Site::discover(&d.0);
    assert!(
        site.decks.iter().any(|deck| deck.url == "slides.html"),
        "deck embedded via an included partial must be discovered; got decks: {:?}",
        site.decks.iter().map(|deck| &deck.url).collect::<Vec<_>>()
    );
    // Downstream: a discovered deck is dropped from the page set, so it is not flattened to
    // a chrome-wrapped article, not indexed into search, and not warned about as a loose page.
    assert!(
        !site.pages.iter().any(|p| p.url == "slides.html"),
        "the discovered deck must be removed from the page set"
    );
    assert!(
        !site.warnings.iter().any(|w| w.contains("loose page")),
        "no false loose-page warning once the embed is discovered: {:?}",
        site.warnings
    );
}
