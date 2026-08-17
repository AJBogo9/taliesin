//! The corpus roster: every document under `corpus/` that the suite is entitled to
//! believe it swept.
//!
//! **Why a hand-maintained list.** Every corpus sweep in this crate iterates whatever is on
//! disk and floors the count at a handful (`files.len() >= 5` against 82 documents), so a
//! deleted document removes coverage without removing a test and every gate stays green.
//! CLAUDE.md's ordering rule ("a pin and its docs page are deleted in the SAME commit as
//! their feature, never before") is precisely the discipline nothing was enforcing, and the
//! scar is on record: `corpus/transclude.tmd` went in wave 7 while `corpus.rs` went on
//! citing it by name to justify a weakened ordering check, which then guarded a feature
//! that no longer existed.
//!
//! This is `token_contract.rs`'s philosophy applied to documents: deleting one is fine, and
//! it costs exactly one line of deliberate diff in the same commit.

use std::path::{Path, PathBuf};

/// Every `.tmd` under `corpus/`, corpus-relative, sorted. Sweeps skip `_extensions/` and
/// `expected/` (fixtures, not documents), so this does too.
const CORPUS_DOCS: &[&str] = &[
    "agent/executed-read.tmd",
    "analyst/index.tmd",
    "analyst/methods.tmd",
    "callouts/kinds.tmd",
    "demo-book/appendix.tmd",
    "demo-book/index.tmd",
    "demo-book/intro.tmd",
    "demo-book/methods.tmd",
    "demo-book/results.tmd",
    "demo-book/summary.tmd",
    "descent/index.tmd",
    "diagnostics/a11y.tmd",
    "diagnostics/check-superset.tmd",
    "diagnostics/links.tmd",
    "diagnostics/refs.tmd",
    "diagnostics/typos.tmd",
    "diagnostics/widgets.tmd",
    "highlight.tmd",
    "layout/dense-output.tmd",
    "layout/escapes.tmd",
    "layout/structure.tmd",
    "media/gallery.tmd",
    "media/optimized-images.tmd",
    "media/themed-figure.tmd",
    "native-tmd.tmd",
    "nested-cells.tmd",
    "posts/born-machines.tmd",
    "posts/cite-coverage/index.tmd",
    "reactive/graph.tmd",
    "reactive/inputs.tmd",
    "reactive/js-error.tmd",
    "reader/long-read.tmd",
    "reader/preferences.tmd",
    "recipes/csv-figure.tmd",
    "render-fixes/index.tmd",
    "shared-bib/index.tmd",
    "shared-bib/notes.tmd",
    "single-page-report/index.tmd",
    "single-page-report/subsections/_complete-pooling-model.tmd",
    "single-page-report/subsections/_data-description.tmd",
    "single-page-report/subsections/_data-modeling.tmd",
    "single-page-report/subsections/_introduction.tmd",
    "single-page-report/subsections/_model-comparison.tmd",
    "single-page-report/subsections/_no-pooling-model.tmd",
    "single-page-report/subsections/_partial-pooling-model.tmd",
    "structured-authors/index.tmd",
    "structured-authors/note.tmd",
    "structured-authors/paper.tmd",
    "tarn/api-frame.tmd",
    "tarn/api-io.tmd",
    "tarn/api-query.tmd",
    "tarn/concepts.tmd",
    "tarn/errors.tmd",
    "tarn/filtering.tmd",
    "tarn/glossary.tmd",
    "tarn/grouping.tmd",
    "tarn/index.tmd",
    "tarn/install.tmd",
    "tarn/joins.tmd",
    "tarn/loading.tmd",
    "tarn/performance.tmd",
    "tarn/quickstart.tmd",
    "tech-blog/404.tmd",
    "tech-blog/_includes/three-scene.tmd",
    "tech-blog/blog.tmd",
    "tech-blog/cv.tmd",
    "tech-blog/index.tmd",
    "tech-blog/posts/KL-divergence/index.tmd",
    "tech-blog/posts/Kruskal-Wallis-test/index.tmd",
    "tech-blog/posts/a-star/index.tmd",
    "tech-blog/posts/draft-example/index.tmd",
    "tech-blog/posts/em-algorithm/index.tmd",
    "tech-blog/posts/evidence-lower-bound/index.tmd",
    "tech-blog/posts/fourier-transform/index.tmd",
    "tech-blog/posts/pca-geometry/index.tmd",
    "tech-blog/projects.tmd",
    "tech-blog/projects/activity-challenge-bot/index.tmd",
    "tech-blog/projects/bayesian-aviation-safety/index.tmd",
    "tech-blog/projects/iphone-premium-analysis/index.tmd",
    "tech-blog/projects/supercollider-mcp/index.tmd",
    "tech-blog/publications.tmd",
    "theme-css/index.tmd",
];

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "_extensions" || name == "expected" {
                continue;
            }
            collect(&p, root, out);
        } else if p.extension().is_some_and(|x| x == "tmd") {
            out.push(
                p.strip_prefix(root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
}

#[test]
fn the_corpus_roster_matches_the_tree() {
    let root = corpus_dir();
    let mut actual = Vec::new();
    collect(&root, &root, &mut actual);
    actual.sort();
    let expected: Vec<String> = CORPUS_DOCS.iter().map(|s| s.to_string()).collect();

    let gone: Vec<&String> = expected.iter().filter(|d| !actual.contains(d)).collect();
    let added: Vec<&String> = actual.iter().filter(|d| !expected.contains(d)).collect();
    assert!(
        gone.is_empty() && added.is_empty(),
        "the corpus roster and the tree disagree.\n\
         DELETED (each one silently removed coverage from every sweep in this crate; \
         confirm the feature it pinned went in the same commit, then drop the line):\n{gone:#?}\n\
         ADDED (paste into CORPUS_DOCS):\n{added:#?}"
    );
    assert_eq!(actual, expected, "the roster must stay sorted");
}

#[test]
fn the_roster_is_sorted_and_unique() {
    let mut sorted = CORPUS_DOCS.to_vec();
    sorted.sort_unstable();
    assert_eq!(CORPUS_DOCS, sorted.as_slice(), "CORPUS_DOCS must be sorted");
    let mut seen = std::collections::BTreeSet::new();
    for d in CORPUS_DOCS {
        assert!(seen.insert(*d), "duplicate roster entry {d}");
    }
}
