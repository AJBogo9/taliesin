//! One document must resolve the same `{{< include >}}` whichever command renders it.
//!
//! PP-3 (2026-07-26 path-parity audit): `build <site>` shipped a page with its include
//! expanded while `build <that page>` shipped the same page without it, warning `include
//! not resolved (path escapes the project root)`. Two commands, one source, two documents.
//!
//! The cause is *not* that one caller forgot to infer a root. Confining a single invoked
//! document to its own directory is PT-2 (`9359a2c`, the 2026-07-17 release audit): the
//! inferred walk stops at the nearest ancestor holding `.git` **or** `_site.yml`, so an
//! untrusted `.tmd` dropped anywhere inside a checkout could `../`-climb to a sibling
//! repo-local file. Reverting that to fix parity would re-open it.
//!
//! What reconciles them: **the containment root is the project the document belongs to.**
//! `_site.yml` is an author declaring a project boundary, so it widens the root to exactly
//! that boundary (which is what the site build already uses, hence parity). `.git` is a
//! checkout, not a project the author pointed this tool at, so it never widens a single
//! invoked document again (which is what PT-2 bought, hence the escape stays closed).
//!
//! These tests pin all three edges of that rule, on the product entry point rather than on
//! the library one the corpus test used (an include assertion that is true of the library
//! and false of the command is the vacuous-test shape one level up).

mod common;
use common::TempProj;

/// A site project with a shared `_includes/` above the page that pulls from it, which is
/// the shape the corpus blog uses and the shape PP-3 was measured on.
fn site_with_shared_include(proj: &TempProj) {
    proj.file("_site.yml", "title: Parity\n");
    proj.file(
        "_includes/frag.tmd",
        "A shared fragment: PARITY_SENTINEL.\n",
    );
    proj.file(
        "posts/p/index.tmd",
        "---\ntitle: P\n---\n\nIntro.\n\n{{< include ../../_includes/frag.tmd >}}\n",
    );
}

#[test]
fn single_doc_render_of_a_site_page_resolves_the_same_includes_as_the_site_build() {
    let proj = TempProj::new();
    site_with_shared_include(&proj);
    let base = proj.0.join("posts/p");
    let src = std::fs::read_to_string(base.join("index.tmd")).unwrap();

    // The site build's page render infers the root and finds `_site.yml`.
    let as_site_page = taliesin_core::render_document_with_includes(&src, &base);
    // The single-document commands (`build`/`preview`/`check`/`read` of one file).
    let as_single_doc = taliesin_core::render_single_doc(&src, &base);

    assert!(
        as_site_page.body_html().contains("PARITY_SENTINEL"),
        "fixture is wrong: the site path must resolve the include"
    );
    // The assertion that matters is parity, not the token: a per-path `contains` is what
    // let two assemblers drift apart in the first place.
    assert_eq!(
        as_single_doc.body_html(),
        as_site_page.body_html(),
        "one source, two commands, two documents: single-doc warnings were {:?}",
        as_single_doc
            .warnings
            .iter()
            .map(|w| w.message.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_site_page_still_cannot_climb_above_its_own_project() {
    // The widening stops AT the declared boundary. `_site.yml` names the project; the
    // checkout around it is not part of the bargain.
    let proj = TempProj::new();
    site_with_shared_include(&proj);
    proj.file(".git", "");
    proj.file("outside.tmd", "NEIGHBOUR_SENTINEL\n");
    proj.file(
        "site/_site.yml", // the project root is BELOW the checkout marker
        "title: Inner\n",
    );
    proj.file(
        "site/posts/p/index.tmd",
        "---\ntitle: P\n---\n\n{{< include ../../../outside.tmd >}}\n",
    );

    let base = proj.0.join("site/posts/p");
    let src = std::fs::read_to_string(base.join("index.tmd")).unwrap();
    let doc = taliesin_core::render_single_doc(&src, &base);

    assert!(
        !doc.body_html().contains("NEIGHBOUR_SENTINEL"),
        "a climb above `_site.yml` must not resolve just because a `.git` sits higher"
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("outside.tmd") && w.line.is_some()),
        "the refusal must be reported and click-to-source, got {:?}",
        doc.warnings
            .iter()
            .map(|w| w.message.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_loose_document_is_confined_to_its_own_directory_despite_an_ancestor_checkout() {
    // PT-2 itself, on the product entry point. No `_site.yml` anywhere, so the document is
    // its own project and the ancestor `.git` grants it nothing.
    let proj = TempProj::new();
    proj.file(".git", "");
    proj.file("sibling.tmd", "REPO_LOCAL_SENTINEL\n");
    proj.file(
        "loose/index.tmd",
        "---\ntitle: L\n---\n\n{{< include ../sibling.tmd >}}\n",
    );

    let base = proj.0.join("loose");
    let src = std::fs::read_to_string(base.join("index.tmd")).unwrap();
    let doc = taliesin_core::render_single_doc(&src, &base);

    assert!(
        !doc.body_html().contains("REPO_LOCAL_SENTINEL"),
        "an ancestor `.git` must not widen a loose document's containment root (PT-2)"
    );
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.message.contains("sibling.tmd")),
        "the refusal must still be reported, got {:?}",
        doc.warnings
            .iter()
            .map(|w| w.message.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_single_doc_root_is_the_declared_project_or_the_document_itself() {
    // The rule stated directly, so a regression names the rule rather than a symptom.
    // One checkout, two branches, only one of which declares a project:
    //   <tmp>/.git                the checkout, which must never become a root
    //   <tmp>/proj/_site.yml      a declared project
    //   <tmp>/proj/posts/p/       a page inside it     -> roots at <tmp>/proj
    //   <tmp>/elsewhere/doc/      no project above it  -> roots at itself
    let proj = TempProj::new();
    proj.file(".git", "");
    proj.file("proj/_site.yml", "title: P\n");
    proj.file("proj/posts/p/index.tmd", "x\n");
    proj.file("elsewhere/doc/index.tmd", "x\n");
    let canon = proj.0.canonicalize().unwrap();

    assert_eq!(
        taliesin_core::single_doc_root(&canon.join("proj/posts/p"))
            .canonicalize()
            .unwrap(),
        canon.join("proj"),
        "a page inside a declared project roots at the project"
    );
    let loose = canon.join("elsewhere/doc");
    assert_eq!(
        taliesin_core::single_doc_root(&loose)
            .canonicalize()
            .unwrap(),
        loose,
        "a document with no declared project roots at itself, never at the checkout"
    );
}
