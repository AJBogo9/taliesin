//! Corpus-wide invariants: every real document must render and satisfy the
//! load-bearing guarantees (a block id + valid sourcepos on every block, ids
//! unique, blocks in document order). The corpus is the spec, so this runs the
//! whole pipeline over each real `.qmd` rather than synthetic snippets.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn collect_qmd(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "_extensions" || name == "expected" {
                continue; // not source documents
            }
            collect_qmd(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("qmd") {
            out.push(p);
        }
    }
}

/// Parse "L:C-L:C" into (start_line, end_line).
fn line_range(sourcepos: &str) -> (usize, usize) {
    let (start, end) = sourcepos.split_once('-').expect("sourcepos has a dash");
    let sl = start.split(':').next().unwrap().parse().expect("start line");
    let el = end.split(':').next().unwrap().parse().expect("end line");
    (sl, el)
}

#[test]
fn every_corpus_doc_renders_with_invariants() {
    let mut files = Vec::new();
    collect_qmd(&corpus_dir(), &mut files);
    files.sort();
    assert!(files.len() >= 5, "expected the corpus docs, found {}", files.len());

    for f in &files {
        let label = f
            .strip_prefix(corpus_dir())
            .unwrap_or(f)
            .display()
            .to_string();
        let src = fs::read_to_string(f).unwrap();
        let base = f.parent().unwrap();
        let doc = qmd_fast_core::render_document_with_includes(&src, base);

        assert!(!doc.blocks.is_empty(), "{label}: produced no blocks");

        let mut ids = HashSet::new();
        // Document order holds *within* a single source file; included files
        // reset to their own line numbering, so track order per file.
        let mut prev_start: std::collections::HashMap<Option<String>, usize> = HashMap::new();
        for b in &doc.blocks {
            assert!(!b.html.is_empty(), "{label}: empty html for block {}", b.id);
            assert!(ids.insert(&b.id), "{label}: duplicate block id {}", b.id);

            let (sl, el) = line_range(&b.sourcepos);
            assert!(sl >= 1, "{label}: zero/invalid start line in {}", b.sourcepos);
            assert!(sl <= el, "{label}: start line after end in {}", b.sourcepos);
            let prev = prev_start.entry(b.source_file.clone()).or_insert(0);
            assert!(
                sl >= *prev,
                "{label}: blocks out of order within {:?} ({sl} after {prev})",
                b.source_file
            );
            *prev = sl;
        }
    }
}

#[test]
fn includes_are_resolved_with_origin_files() {
    // pca-geometry pulls in _includes/three-scene.qmd via {{< include >}}.
    let dir = corpus_dir().join("posts/pca-geometry");
    let src = fs::read_to_string(dir.join("index.qmd")).unwrap();
    let doc = qmd_fast_core::render_document_with_includes(&src, &dir);

    let body = doc.body_html();
    assert!(!body.contains("{{< include"), "include shortcode leaked into output");

    // some blocks must now originate from the included file, with their own lines
    let from_include: Vec<_> = doc
        .blocks
        .iter()
        .filter(|b| b.source_file.as_deref().is_some_and(|f| f.contains("three-scene")))
        .collect();
    assert!(
        !from_include.is_empty(),
        "expected blocks sourced from the included three-scene.qmd"
    );

    // the book pulls in subsections; every subsection should contribute blocks
    let book = corpus_dir().join("bayesian-book");
    let bsrc = fs::read_to_string(book.join("index.qmd")).unwrap();
    let bdoc = qmd_fast_core::render_document_with_includes(&bsrc, &book);
    assert!(!bdoc.body_html().contains("{{< include"));
    let included_files: HashSet<_> = bdoc
        .blocks
        .iter()
        .filter_map(|b| b.source_file.clone())
        .collect();
    assert!(
        included_files.len() >= 5,
        "expected blocks from several subsection files, got {included_files:?}"
    );
}

#[test]
fn ids_and_sourcepos_present_on_visible_blocks() {
    // Every visible block element should carry both data attributes. (Raw HTML
    // comment blocks legitimately carry neither — they are emitted verbatim.)
    let src = fs::read_to_string(corpus_dir().join("posts/em-algorithm/index.qmd")).unwrap();
    let doc = qmd_fast_core::render_document(&src);
    for b in &doc.blocks {
        if b.html.starts_with("<!--") {
            continue;
        }
        assert!(
            b.html.contains("data-block-id=") && b.html.contains("data-sourcepos="),
            "block missing data attributes: {}",
            &b.html[..b.html.len().min(80)]
        );
    }
}
