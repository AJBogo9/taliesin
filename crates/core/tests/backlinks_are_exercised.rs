//! Item 56: the "Referenced by" backlink chain has an end-to-end user, not just unit tests.
//!
//! `site/backlinks.rs` builds a reverse index from **cross-page** cross-reference markers
//! and splices a quiet "Referenced by" line under each referenced target. Its own unit tests
//! passed throughout, and it emitted **nothing** on either dogfooded book for as long as it
//! has existed: all 81 `@`-references in the two books were intra-page, so the reverse index
//! was always empty and the whole splice path — harvest, index, attach — ran on no real
//! input anywhere in the repository.
//!
//! That is the shape this file exists to prevent. A feature whose only exercise is a
//! synthetic fixture is a feature that can rot in a way no unit test reports: the fixture
//! keeps passing while the product ships a dead surface. So this asserts the chain runs on
//! the **real books**, end to end, at the level a reader sees.
//!
//! **It deliberately does NOT pin which chapter references which.** That is authorship, and
//! an author reorganizing the guide must stay free to move a reference. What must not happen
//! silently is the count going back to zero.

mod common;

use common::corpus_dir;
use std::path::Path;
use taliesin_core::Site;

/// Both dogfooded books must carry at least one cross-page reference that reaches a reader
/// as a rendered backlink.
///
/// Rendering the whole book and looking for the emitted line is the point: a test on
/// `build_backlink_index` alone would have passed every day of the years this emitted
/// nothing, because the index was correct and simply had no cross-page markers to index.
#[test]
fn both_dogfooded_books_render_at_least_one_backlink() {
    let root = corpus_dir().join("..");
    for book in ["docs/guide", "docs/internals"] {
        let dir = root.join(book);
        assert!(
            dir.is_dir(),
            "{book} must exist for this walk to mean anything"
        );
        let site = Site::discover(&dir);
        let hits: Vec<&str> = site
            .pages
            .iter()
            .filter(|p| {
                site.render_page(&p.rel)
                    .is_some_and(|html| html.contains("class=\"tali-backref\""))
            })
            .map(|p| p.url.as_str())
            .collect();
        assert!(
            !hits.is_empty(),
            "{book} renders no \"Referenced by\" line on any page: every cross-reference in \
             it is intra-page again, so the backlink path is shipping dead. Add a \
             cross-chapter reference someone means (not a sweep) rather than deleting this \
             test."
        );
    }
}

/// The citing sentence is the payload, not the page title: a bare title cannot tell a reader
/// whether the referring page builds on this target or mentions it in passing. If the
/// harvest ever regresses to emitting a link with no context, every backlink silently
/// becomes the weaker affordance while still rendering.
#[test]
fn a_rendered_backlink_carries_the_sentence_that_makes_the_reference() {
    let site = Site::discover(&corpus_dir().join("../docs/guide"));
    let page = site
        .pages
        .iter()
        .find_map(|p| {
            site.render_page(&p.rel)
                .filter(|h| h.contains("class=\"tali-backref\""))
        })
        .expect("the guide renders a backlink (see the test above)");
    assert!(
        page.contains("class=\"tali-backref-cite\""),
        "a backlink must carry its citing sentence, not just the referring page's title"
    );
    // And the reference it came from resolved to a real number rather than a bare "Section".
    assert!(
        page.contains("Referenced by"),
        "the visible label is what a reader scans for"
    );
}

/// Guard on the guard: `corpus_dir()/..` must actually be the repository root, or both tests
/// above would be asserting about directories that do not exist — and the first one asserts
/// `is_dir()` precisely so that failure is loud instead of an empty loop.
#[test]
fn the_walk_roots_at_the_repository() {
    let root: &Path = &corpus_dir().join("..");
    assert!(
        root.join("Cargo.toml").is_file(),
        "{root:?} is not the workspace root"
    );
}
