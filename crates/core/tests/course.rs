//! Interaction pin for the "course author" demand-probe pilot (corpus/course/).
//! Locks the feature *combinations* the single-feature corpus docs never exercise
//! together: chapter-scoped numbering across THREE float kinds in one book, and
//! cross-PAGE cross-references in both directions (a later chapter citing an earlier
//! one's equation and figure, and a still-later chapter citing that chapter's section).
//! See notes/2026-07-22-corpus-demand-probe-course-author.md for the findings this
//! produced.

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn course() -> Site {
    Site::discover(&corpus_dir().join("course"))
}

#[test]
fn ch2_scopes_equation_numbers_to_its_chapter() {
    let mle = course().render_page("mle.tmd").expect("mle renders");
    // Chapter scoping on an EQUATION, which numbers through the same `float_number`
    // helper as figures and tables but is the one float kind demo-book does not carry.
    assert!(
        mle.contains("Equation&nbsp;2.1") || mle.contains("(2.1)"),
        "the score equation is numbered 2.1 in chapter 2: {mle}"
    );
}

#[test]
fn cross_page_refs_resolve_to_scoped_numbers() {
    let mle = course().render_page("mle.tmd").expect("mle renders");
    // ch2 references ch1's equation and figure across pages.
    assert!(
        mle.contains("#eq-expectation") && mle.contains("1.1"),
        "cross-page ref to the ch1 equation resolves to 1.1: {mle}"
    );
    assert!(
        mle.contains("#fig-distributions") && mle.contains("Figure&nbsp;1.1"),
        "cross-page ref to the ch1 figure resolves to 1.1: {mle}"
    );

    let em = course().render_page("em.tmd").expect("em renders");
    // ch3 references ch2's section and ch1's equation across pages, and numbers its own
    // equation in its own chapter.
    assert!(
        em.contains("#sec-consistency") && em.contains("Section&nbsp;2."),
        "cross-page ref to the ch2 section resolves into chapter 2: {em}"
    );
    assert!(
        em.contains("#eq-expectation") && em.contains("1.1"),
        "cross-page ref to the ch1 equation resolves to 1.1: {em}"
    );
    assert!(
        em.contains("#eq-elbo") && em.contains("3.1"),
        "the ELBO equation is 3.1 in chapter 3: {em}"
    );
}
