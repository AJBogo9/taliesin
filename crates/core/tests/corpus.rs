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
    let sl = start
        .split(':')
        .next()
        .unwrap()
        .parse()
        .expect("start line");
    let el = end.split(':').next().unwrap().parse().expect("end line");
    (sl, el)
}

#[test]
fn every_corpus_doc_has_clean_front_matter() {
    // The front-matter linter must not warn on any real document: a warning here
    // means the KNOWN_KEYS allowlist is missing a key the corpus legitimately uses.
    let mut files = Vec::new();
    collect_qmd(&corpus_dir(), &mut files);
    let mut offenders = Vec::new();
    for f in &files {
        let src = fs::read_to_string(f).unwrap();
        for w in qmd_fast_core::frontmatter::lint(&src) {
            let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
            offenders.push(format!("{label}: {w}"));
        }
    }
    assert!(
        offenders.is_empty(),
        "front-matter lint warned on corpus docs:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn every_corpus_doc_renders_with_invariants() {
    let mut files = Vec::new();
    collect_qmd(&corpus_dir(), &mut files);
    files.sort();
    assert!(
        files.len() >= 5,
        "expected the corpus docs, found {}",
        files.len()
    );

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

            // Generated blocks (e.g. the References section) carry no sourcepos.
            if b.sourcepos.is_empty() {
                continue;
            }
            let (sl, el) = line_range(&b.sourcepos);
            assert!(
                sl >= 1,
                "{label}: zero/invalid start line in {}",
                b.sourcepos
            );
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
    assert!(
        !body.contains("{{< include"),
        "include shortcode leaked into output"
    );

    // some blocks must now originate from the included file, with their own lines
    let from_include: Vec<_> = doc
        .blocks
        .iter()
        .filter(|b| {
            b.source_file
                .as_deref()
                .is_some_and(|f| f.contains("three-scene"))
        })
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
fn reveal_deck_detects_format_and_splits_into_slides() {
    use qmd_fast_core::{DocFormat, render_document_with_includes, slides_html};
    let dir = corpus_dir().join("liquid-glass-slides");
    let src = fs::read_to_string(dir.join("example.qmd")).unwrap();
    let doc = render_document_with_includes(&src, &dir);

    assert_eq!(
        doc.format,
        DocFormat::Reveal,
        "the deck should be detected as reveal.js"
    );

    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    // Title slide built from front matter.
    assert!(slides.contains("id=\"title-slide\""), "missing title slide");
    assert!(slides.contains("<h1 class=\"title\">Liquid Glass</h1>"));
    assert!(slides.contains("<p class=\"subtitle\">A RevealJS theme for Quarto</p>"));
    // One slide per `##` heading (the corpus deck has four).
    let content_slides = slides.matches("class=\"slide level2\"").count();
    assert_eq!(
        content_slides, 4,
        "expected 4 content slides, got {content_slides}"
    );
    // Slide ids are slugged from the heading text, matching Quarto.
    assert!(
        slides.contains("id=\"what-is-liquid-glass\""),
        "got: {slides}"
    );
    // Blocks keep their data attributes inside sections (block-swap/click-to-source).
    assert!(
        slides.contains("<h2 data-block-id="),
        "headings lost their block id"
    );
    assert!(
        !slides.contains("{{<"),
        "shortcodes must not leak into slide output"
    );
}

#[test]
fn book_renders_with_toc_anchored_headings_and_numbered_figures() {
    let dir = corpus_dir().join("bayesian-book");
    let src = fs::read_to_string(dir.join("index.qmd")).unwrap();
    let page = qmd_fast_core::render_html_page_with_includes(&src, &dir, "book");

    // toc: true -> a TOC nav + the sidebar layout, with anchor-linked entries.
    assert!(
        page.contains("id=\"TOC\""),
        "book should render a table of contents"
    );
    assert!(page.contains("class=\"has-toc\""), "missing toc layout");
    assert!(
        page.contains("<a href=\"#introduction\">Introduction</a>"),
        "TOC entry missing"
    );
    // Headings carry matching anchor ids.
    assert!(
        page.contains("<h1 id=\"introduction\""),
        "heading anchor missing"
    );

    // The three labelled image figures render as numbered <figure>s, attrs not
    // leaked. They are Figures 4–6: three earlier `#| fig-cap:` code cells take
    // numbers 1–3 (counted in document order, matching Quarto — even though those
    // R cells aren't executed here, so their output isn't shown).
    assert!(
        !page.contains("{#fig-"),
        "figure attribute block leaked into output"
    );
    assert!(
        page.contains("id=\"fig-model-hierarchical\""),
        "figure id missing"
    );
    for n in 4..=6 {
        assert!(
            page.contains(&format!("Figure&nbsp;{n}:")),
            "missing 'Figure {n}:' caption"
        );
    }
    // The labelled image figure resolves to its number via the registry.
    assert!(page.contains("id=\"fig-model-hierarchical\""));
}

#[test]
fn ids_and_sourcepos_present_on_visible_blocks() {
    // Every visible block element should carry both data attributes. (Raw HTML
    // comment blocks legitimately carry neither — they are emitted verbatim.)
    let src = fs::read_to_string(corpus_dir().join("posts/em-algorithm/index.qmd")).unwrap();
    let doc = qmd_fast_core::render_document(&src);
    for b in &doc.blocks {
        // Raw HTML comments are emitted verbatim; generated blocks (References)
        // have no sourcepos. Both legitimately lack the data attributes.
        if b.html.starts_with("<!--") || b.sourcepos.is_empty() {
            continue;
        }
        assert!(
            b.html.contains("data-block-id=") && b.html.contains("data-sourcepos="),
            "block missing data attributes: {}",
            &b.html[..b.html.len().min(80)]
        );
    }
}

#[test]
fn tech_blog_site_discovers_renders_chrome_and_rewrites_links() {
    use qmd_fast_core::Site;
    let root = corpus_dir().join("tech-blog");
    let site = Site::discover(&root);

    // The project config parses (navbar items) and every `.qmd` page is found,
    // each mapped to a `.html` output url.
    assert!(
        site.pages.len() >= 10,
        "expected the tech-blog pages, found {}",
        site.pages.len()
    );
    assert!(
        !site.config.website.navbar.left.is_empty(),
        "navbar items should parse from _quarto.yml"
    );
    for p in &site.pages {
        assert!(p.url.ends_with(".html"), "page url not .html: {}", p.url);
    }

    // A top-level page renders with the site chrome and rewrites its nav links.
    let blog = site.render_page("blog.qmd").expect("blog renders");
    assert!(blog.contains("qmd-site-nav"), "navbar missing");
    assert!(blog.contains("qmd-site-footer"), "footer missing");
    assert!(
        blog.contains("href=\"blog.html\""),
        "nav link not rewritten"
    );
    assert!(
        !blog.contains("href=\"blog.qmd\""),
        "raw .qmd nav link leaked"
    );
    // The RSS/feed link is dropped.
    assert!(!blog.contains("blog.xml"), "RSS link should be dropped");

    // A post carries a "back to the blog listing" button and rewrites cross-page
    // `.qmd` links.
    let post = site
        .render_page("posts/evidence-lower-bound/index.qmd")
        .expect("post renders");
    assert!(
        post.contains("qmd-back-link") && post.contains("Back to Blog"),
        "post missing the back-to-blog button"
    );
    assert!(
        post.contains("href=\"../../blog.html\""),
        "back button should link to the blog listing"
    );
    assert!(
        post.contains("../KL-divergence/index.html"),
        "cross-page .qmd link not rewritten to .html"
    );
    assert!(
        !post.contains("../KL-divergence/index.qmd"),
        "raw cross-page .qmd link leaked"
    );
}
