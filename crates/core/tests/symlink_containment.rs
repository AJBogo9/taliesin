//! Containment of *symlinked* paths: what the filesystem may resolve a path to, as
//! distinct from what the document text may lexically ask for.
//!
//! Two rules are pinned here, and they pull in opposite directions:
//!
//! * A symlink whose target leaves the enclosing repository is refused, and that
//!   refusal must not depend on how the document was addressed on the command line.
//! * A symlink to a sibling directory *inside* the same repository resolves, which is
//!   how a book shares one `references.bib` with the `paper/` next to it. A symlink is
//!   a filesystem fact placed by whoever owns the checkout, not something the document
//!   text can conjure, so the repo is the right unit of first-party trust.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use taliesin_core::render_document_with_includes;

/// A throwaway dir under the system temp, unique per test name + process.
fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tali-symcontain-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn html(doc: &taliesin_core::RenderedDoc) -> String {
    doc.blocks
        .iter()
        .map(|b| b.html.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn an_escaping_symlink_is_refused_when_the_doc_is_addressed_by_bare_filename() {
    // `taliesin build index.tmd` (no directory component) makes the doc's base dir the
    // EMPTY path. `std::path::absolute("")` is an error, so the base stayed relative,
    // the containment root came out empty, and `"".canonicalize()` failed — which the
    // symlink check treated as "nothing to compare" and let the read through. The same
    // include was correctly refused as `./index.tmd`, so a `cd` decided whether an
    // out-of-repo file was inlined into the page.
    let dir = tmp("bare-filename");
    let proj = dir.join("proj");
    fs::create_dir_all(&proj).unwrap();
    fs::create_dir_all(dir.join("outside")).unwrap();
    fs::write(proj.join(".git"), b"").unwrap(); // project-root marker
    let secret = dir.join("outside/secret.txt");
    fs::write(&secret, b"SECRET-LEAKED-CONTENT").unwrap();
    symlink(&secret, proj.join("leak.tmd")).unwrap();

    let prev = std::env::current_dir().unwrap();
    std::env::set_current_dir(&proj).unwrap();
    let doc = render_document_with_includes("{{< include leak.tmd >}}\n", Path::new(""));
    std::env::set_current_dir(prev).unwrap();

    assert!(
        !html(&doc).contains("SECRET-LEAKED-CONTENT"),
        "an out-of-repo symlink target must not be inlined just because the doc was \
         addressed by bare filename; got: {}",
        html(&doc)
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_bibliography_symlinked_to_a_sibling_inside_the_repo_resolves() {
    // <repo>/.git
    // <repo>/paper/references.bib          the real file
    // <repo>/book/_site.yml                the narrower lexical root
    // <repo>/book/references.bib -> ../paper/references.bib
    //
    // The lexical root is `book/` (its `_site.yml` stops the walk), so canonicalizing the
    // symlink lands outside it and the bibliography was refused — reported as "not found"
    // for a file that plainly exists, and every reference then rendered as a bare BibTeX
    // key. Sharing one `.bib` across sibling directories of one repo is first-party
    // authoring, not an escape.
    let repo = tmp("sibling-bib");
    fs::write(repo.join(".git"), b"").unwrap();
    fs::create_dir_all(repo.join("paper")).unwrap();
    fs::create_dir_all(repo.join("book")).unwrap();
    fs::write(
        repo.join("paper/references.bib"),
        "@article{Fiedler1973, title={Algebraic connectivity of graphs}, \
         author={Fiedler, Miroslav}, year={1973}}\n",
    )
    .unwrap();
    fs::write(repo.join("book/_site.yml"), b"title: Book\n").unwrap();
    symlink("../paper/references.bib", repo.join("book/references.bib")).unwrap();

    let src = "---\ntitle: T\nbibliography: references.bib\n---\n\nSee [@Fiedler1973].\n";
    let doc = render_document_with_includes(src, &repo.join("book"));

    let bib_warnings: Vec<&str> = doc
        .warnings
        .iter()
        .map(|w| w.message.as_str())
        .filter(|m| m.contains("bibliograph"))
        .collect();
    assert!(
        bib_warnings.is_empty(),
        "a symlink to a sibling in the same repo is first-party, not an escape; got: {bib_warnings:?}"
    );
    assert!(
        html(&doc).contains("Algebraic connectivity"),
        "the entry must render, not degrade to a bare citation key; got: {}",
        html(&doc)
    );

    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn site_discovery_refuses_a_page_symlinked_out_of_the_repository() {
    // Page discovery walks the project directory itself, so it never went through
    // `safe_join` and applied no containment at all: a `.tmd` symlinked out of the tree
    // was walked, rendered, and published as its own page in `_site/`. The walker is
    // held to the same repository boundary as every other resolved path.
    //
    //   <dir>/outside/secret.tmd            out of tree
    //   <dir>/repo/.git
    //   <dir>/repo/paper/appendix.tmd       in-repo, above the site root
    //   <dir>/repo/book/_site.yml           the site root
    //   <dir>/repo/book/index.tmd
    //   <dir>/repo/book/shared.tmd -> ../paper/appendix.tmd    in-repo: a page
    //   <dir>/repo/book/leak.tmd   -> ../../outside/secret.tmd out-of-repo: refused
    let dir = tmp("discovery");
    let book = dir.join("repo/book");
    fs::create_dir_all(&book).unwrap();
    fs::create_dir_all(dir.join("repo/paper")).unwrap();
    fs::create_dir_all(dir.join("outside")).unwrap();
    fs::write(dir.join("repo/.git"), b"").unwrap();
    fs::write(
        dir.join("outside/secret.tmd"),
        "# Secret\n\nSECRET-LEAKED-CONTENT\n",
    )
    .unwrap();
    fs::write(dir.join("repo/paper/appendix.tmd"), "# Appendix\n").unwrap();
    fs::write(book.join("_site.yml"), b"title: Book\n").unwrap();
    fs::write(book.join("index.tmd"), "# Home\n").unwrap();
    symlink("../paper/appendix.tmd", book.join("shared.tmd")).unwrap();
    symlink("../../outside/secret.tmd", book.join("leak.tmd")).unwrap();

    let site = taliesin_core::site::Site::discover(&book);
    let rels: Vec<&str> = site.pages.iter().map(|p| p.rel.as_str()).collect();

    assert!(
        !rels.contains(&"leak.tmd"),
        "a page symlinked out of the repository must not be published; got: {rels:?}"
    );
    assert!(
        rels.contains(&"shared.tmd"),
        "a page symlinked to a sibling inside the repository is first-party; got: {rels:?}"
    );
    assert!(rels.contains(&"index.tmd"), "sanity: got {rels:?}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn site_discovery_terminates_on_a_symlink_loop() {
    // An in-repo symlink pointing back at an ancestor is allowed by the boundary above,
    // so the walker needs its own cycle guard: `is_dir()` follows the link, and every
    // trip through it yields a longer path that still reads. It only stopped when the
    // path outgrew `PATH_MAX`, by which point one real page had been re-discovered a few
    // hundred times over, each copy a separate output page. Canonical directories already
    // walked are now skipped, so the loop is entered exactly once.
    let dir = tmp("loop");
    let book = dir.join("book");
    fs::create_dir_all(book.join("sub")).unwrap();
    fs::write(dir.join(".git"), b"").unwrap();
    fs::write(book.join("_site.yml"), b"title: Book\n").unwrap();
    fs::write(book.join("index.tmd"), "# Home\n").unwrap();
    symlink("..", book.join("sub/up")).unwrap();

    let site = taliesin_core::site::Site::discover(&book);
    let rels: Vec<&str> = site.pages.iter().map(|p| p.rel.as_str()).collect();
    assert!(
        rels.contains(&"index.tmd"),
        "discovery must terminate and still find the real page; got {} pages",
        rels.len()
    );
    assert_eq!(
        rels.len(),
        1,
        "the loop must be walked once, not re-entered per level; got: {rels:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_bibliography_symlinked_outside_the_repo_is_refused_and_says_why() {
    // The widening above stops at the repository. A `.bib` symlinked to somewhere the
    // repo does not reach is still refused — and the diagnostic must name containment
    // rather than claim the file is missing, since "not found" for a file that exists is
    // what hid this failure mode.
    let dir = tmp("outside-bib");
    let repo = dir.join("repo");
    fs::create_dir_all(&repo).unwrap();
    fs::write(repo.join(".git"), b"").unwrap();
    let outside = dir.join("elsewhere.bib");
    fs::write(&outside, "@article{X, title={Out Of Tree}, year={2020}}\n").unwrap();
    symlink(&outside, repo.join("references.bib")).unwrap();

    let src = "---\ntitle: T\nbibliography: references.bib\n---\n\nSee [@X].\n";
    let doc = render_document_with_includes(src, &repo);

    assert!(
        !html(&doc).contains("Out Of Tree"),
        "a `.bib` outside the repository must not be read"
    );
    let w = doc
        .warnings
        .iter()
        .find(|w| w.message.contains("bibliograph"))
        .unwrap_or_else(|| panic!("expected a bibliography warning; got {:?}", doc.warnings));
    assert!(
        w.message.contains("outside the project"),
        "the refusal must name containment, not absence; got: {}",
        w.message
    );

    let _ = fs::remove_dir_all(&dir);
}
