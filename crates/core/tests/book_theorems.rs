//! Per-chapter `theorems:` in a book. Pins `corpus/theorem-book/`.
//!
//! The book-wide `_site.yml theorems:` policy was retired on 2026-08-02, so a chapter's
//! counter configuration is entirely its own. The property that replaces "inheritance
//! works" is **no leak**: alpha declares `shared: [theorem, lemma]`, beta declares
//! nothing, and beta must count each kind separately. That is the failure a shared
//! `TheoremConfig` threaded through the site would reintroduce silently.

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn book() -> Site {
    Site::discover(&corpus_dir().join("theorem-book"))
}

#[test]
fn a_chapters_shared_counter_applies_and_is_chapter_scoped() {
    let alpha = book().render_page("alpha.tmd").expect("alpha renders");
    // One counter across both kinds, scoped to chapter 1.
    assert!(
        alpha.contains(r#"<span class="tali-theorem-number">&nbsp;1.1</span>"#),
        "the theorem opens the shared sequence at 1.1:\n{alpha}"
    );
    assert!(
        alpha.contains(r#"<span class="tali-theorem-number">&nbsp;1.2</span>"#),
        "the lemma CONTINUES that sequence at 1.2 rather than restarting:\n{alpha}"
    );
}

#[test]
fn a_chapter_declaring_nothing_does_not_inherit_its_siblings_config() {
    let beta = book().render_page("beta.tmd").expect("beta renders");
    // Independent counters: both kinds are 2.1 in chapter 2. If alpha's `shared:` leaked,
    // the lemma would read 2.2 instead.
    assert_eq!(
        beta.matches(r#"<span class="tali-theorem-number">&nbsp;2.1</span>"#)
            .count(),
        2,
        "theorem and lemma each count from 1 in their own chapter:\n{beta}"
    );
    assert!(
        !beta.contains(r#"<span class="tali-theorem-number">&nbsp;2.2</span>"#),
        "a sibling chapter's `shared:` must not reach this one:\n{beta}"
    );
}

/// Every theorem carries a number. The `numbered:` opt-outs are gone, so no corpus page
/// and no config can produce the empty number span this book used to assert.
#[test]
fn no_chapter_renders_an_unnumbered_theorem() {
    for page in ["alpha.tmd", "beta.tmd"] {
        let html = book().render_page(page).expect("renders");
        assert!(
            !html.contains(r#"<span class="tali-theorem-number"></span>"#),
            "{page} rendered an unnumbered theorem:\n{html}"
        );
    }
}
