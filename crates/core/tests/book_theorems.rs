//! Book-level `theorems:` (item 16 F-01): a `_site.yml` numbering policy is inherited by a
//! chapter with no `theorems:` block of its own, and overridden by a chapter that declares
//! one. Pins `corpus/theorem-book/` (alpha inherits `numbered: false`; beta overrides back
//! to numbered).

mod common;
use common::corpus_dir;
use taliesin_core::Site;

fn book() -> Site {
    Site::discover(&corpus_dir().join("theorem-book"))
}

#[test]
fn a_chapter_without_its_own_theorems_inherits_the_book_policy() {
    let alpha = book().render_page("alpha.tmd").expect("alpha renders");
    // Book policy is `numbered: false`, so the theorem's number span is empty.
    assert!(
        alpha.contains(r#"<span class="tali-theorem-number"></span>"#),
        "alpha inherits book numbered:false (empty number span):\n{alpha}"
    );
    assert!(
        !alpha.contains(r#"tali-theorem-number">&nbsp;"#),
        "alpha theorem must carry no number"
    );
}

#[test]
fn a_chapter_with_its_own_theorems_overrides_the_book_policy() {
    let beta = book().render_page("beta.tmd").expect("beta renders");
    // beta declares `numbered: true`, overriding the book, so its theorem is numbered and
    // chapter-scoped (chapter 2 -> "Theorem 2.1").
    assert!(
        beta.contains(r#"<span class="tali-theorem-number">&nbsp;2.1</span>"#),
        "beta overrides to numbered, chapter-scoped:\n{beta}"
    );
}
