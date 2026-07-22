//! Interaction pin for the "course author" demand-probe pilot (corpus/course/).
//! Locks the feature *combinations* the single-feature corpus docs never exercise
//! together: shared theorem counters × chapter scoping, cross-PAGE crossrefs, a deck
//! embedded in a book chapter, and the hover index over definitional blocks. See
//! notes/2026-07-22-corpus-demand-probe-course-author.md for the findings this produced.

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn course() -> Site {
    Site::discover(&corpus_dir().join("course"))
}

#[test]
fn ch2_shares_theorem_counter_and_scopes_to_chapter() {
    let mle = course().render_page("mle.tmd").expect("mle renders");
    // Shared counter (theorem+lemma one sequence) AND chapter scoping (2.x): a
    // combination pinned nowhere else — theorems-shared.tmd is flat, demo-book scopes
    // an un-shared counter.
    assert!(
        mle.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;2.1</span></span>"
        ),
        "consistency theorem is 2.1: {mle}"
    );
    assert!(
        mle.contains(
            "<span class=\"tali-theorem-label\">Lemma<span class=\"tali-theorem-number\">&nbsp;2.2</span></span>"
        ),
        "score lemma shares the counter as 2.2: {mle}"
    );
}

#[test]
fn cross_page_refs_resolve_to_scoped_numbers() {
    let mle = course().render_page("mle.tmd").expect("mle renders");
    // ch2 references ch1's definition and figure across pages.
    assert!(
        mle.contains("#def-expectation") && mle.contains("Definition&nbsp;1.1"),
        "cross-page ref to the ch1 definition resolves to 1.1: {mle}"
    );
    assert!(
        mle.contains("#fig-distributions") && mle.contains("Figure&nbsp;1.1"),
        "cross-page ref to the ch1 figure resolves to 1.1: {mle}"
    );

    let em = course().render_page("em.tmd").expect("em renders");
    // ch3 references ch2's theorem across pages, and its own theorem is 3.1.
    assert!(
        em.contains("#thm-consistency") && em.contains("Theorem&nbsp;2.1"),
        "cross-page ref to the ch2 theorem resolves to 2.1: {em}"
    );
    assert!(
        em.contains(
            "<span class=\"tali-theorem-label\">Theorem<span class=\"tali-theorem-number\">&nbsp;3.1</span></span>"
        ),
        "the ELBO theorem is 3.1 in chapter 3: {em}"
    );
}

#[test]
fn em_chapter_embeds_the_lecture_deck() {
    let em = course().render_page("em.tmd").expect("em renders");
    // The {{< embed lecture.tmd >}} lowers to an iframe pointing at the built deck html.
    assert!(
        em.contains("<iframe") && em.contains("lecture.html"),
        "the EM chapter embeds the lecture deck as an iframe: {em}"
    );
}

#[test]
fn defined_blocks_enter_the_hover_index_sections_do_not() {
    let idx = course().hover_index_json;
    assert!(
        idx.contains("\"thm-elbo\":\""),
        "ELBO theorem is hover-indexed: {idx}"
    );
    assert!(
        idx.contains("\"def-expectation\":\""),
        "definition is hover-indexed: {idx}"
    );
    assert!(
        !idx.contains("\"sec-em\":\""),
        "section headings are not hover-indexed: {idx}"
    );
    assert!(
        !idx.contains("</script"),
        "raw </script must be neutralized: {idx}"
    );
}
