//! Corpus-wide invariants: every real document must render and satisfy the
//! load-bearing guarantees (a block id + valid sourcepos on every block, ids
//! unique, blocks in document order). The corpus is the spec, so this runs the
//! whole pipeline over each real `.qmd` rather than synthetic snippets.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

mod common;
use common::corpus_dir;

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
    // qmd-fast's front-matter validator must not warn on any real document: a warning
    // here means the allowlist is missing a key the corpus legitimately uses.
    // corpus/diagnostics/ is exempt (it deliberately holds typo'd keys).
    let mut files = Vec::new();
    collect_qmd(&corpus_dir(), &mut files);
    let mut offenders = Vec::new();
    for f in &files {
        if f.components().any(|c| c.as_os_str() == "diagnostics") {
            continue;
        }
        let src = fs::read_to_string(f).unwrap();
        for w in qmd_fast_core::frontmatter::validate_front_matter(&src) {
            let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
            offenders.push(format!("{label}: {}", w.message));
        }
    }
    assert!(
        offenders.is_empty(),
        "front-matter validator warned on corpus docs:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn every_corpus_doc_emits_no_unknown_key_warnings() {
    // qmd-fast has its own closed vocabulary: every real corpus doc must use only
    // recognized cell options, callout kinds, and config keys, so the validators stay
    // silent. corpus/diagnostics/ is exempt (its exact warnings are pinned in
    // crates/core/tests/nested_validation.rs).
    let mut files = Vec::new();
    collect_qmd(&corpus_dir(), &mut files);
    let mut offenders = Vec::new();
    for f in &files {
        if f.components().any(|c| c.as_os_str() == "diagnostics") {
            continue;
        }
        let src = fs::read_to_string(f).unwrap();
        let base = f.parent().unwrap();
        let doc = qmd_fast_core::render_document_with_includes(&src, base);
        for w in doc
            .warnings
            .iter()
            .filter(|w| w.message.starts_with("unknown "))
        {
            let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
            offenders.push(format!("{label}: {}", w.message));
        }
    }
    assert!(
        offenders.is_empty(),
        "validator warned on corpus docs (clean the doc or extend the vocabulary):\n{}",
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

    // the single-page report pulls in subsections; every subsection contributes blocks
    let book = corpus_dir().join("bayesian-website");
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
        "the deck format should be detected"
    );

    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    // Title slide built from front matter.
    assert!(slides.contains("id=\"title-slide\""), "missing title slide");
    assert!(slides.contains("<h1 class=\"title\">Liquid Glass</h1>"));
    assert!(slides.contains("<p class=\"subtitle\">A RevealJS theme for Quarto</p>"));
    // One slide per `##` heading (the corpus deck has four).
    let content_slides = slides.matches("data-level=\"2\"").count();
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
fn a11y_chrome_emits_landmarks_skip_link_and_slide_roles() {
    use qmd_fast_core::{render_document_with_includes, slides_html};
    // --- deck slides carry ARIA slide roles (additive on the <section> open tag, so
    // the inner [data-block-id] blocks are untouched — block ids stay byte-stable). ---
    let dir = corpus_dir().join("liquid-glass-slides");
    let src = fs::read_to_string(dir.join("example.qmd")).unwrap();
    let doc = render_document_with_includes(&src, &dir);
    let slides = slides_html(doc.title.as_deref(), doc.subtitle.as_deref(), &doc.blocks);
    // Every content slide announces as a slide group so a screen reader can navigate
    // the deck. (The "N of M" aria-label is applied at runtime by deck.js, where the
    // flat slide order across stacks is known.)
    assert!(
        slides.contains("role=\"group\""),
        "deck slides must carry role=\"group\""
    );
    assert!(
        slides.contains("aria-roledescription=\"slide\""),
        "deck slides must carry aria-roledescription=\"slide\""
    );
    // The slide role rides on the same <section> as the slide class — never on an
    // inner block — so it can't perturb a [data-block-id].
    assert!(
        slides.contains("class=\"qmd-slide\" role=\"group\" aria-roledescription=\"slide\""),
        "slide ARIA must sit on the .qmd-slide <section>, got: {slides}"
    );
    // Headings still keep their block ids inside the now-role'd section.
    assert!(
        slides.contains("<h2 data-block-id="),
        "headings lost their block id after adding slide roles"
    );

    // --- a page with a TOC emits the skip-link + focusable <main> SERVER-SIDE (works
    // with JS off) and a distinguishable TOC landmark. ---
    let page = qmd_fast_core::render_doc_to_page(
        &qmd_fast_core::render_document(
            "---\ntitle: \"T\"\ntoc: true\n---\n\n# One\n\nbody\n\n## Two\n\nmore\n",
        ),
        "fallback",
        qmd_fast_core::OutputMode::Build,
    );
    // Skip-to-content link is the first thing in the body, before JS runs.
    assert!(
        page.contains("class=\"qmd-skip\"") && page.contains("href=\"#qmd-main\""),
        "server-side skip-to-content link missing"
    );
    // The content container is a focusable <main id="qmd-main">.
    assert!(
        page.contains("<main id=\"qmd-main\" tabindex=\"-1\">"),
        "server-side focusable <main id=qmd-main> missing"
    );
    // The TOC is a distinguishable landmark (named + role) for screen-reader landmark nav.
    assert!(
        page.contains("role=\"doc-toc\"") && page.contains("aria-label=\"Table of contents\""),
        "TOC landmark must carry role + an aria-label"
    );
}

#[test]
fn website_renders_with_toc_anchored_headings_and_numbered_figures() {
    // bayesian-website is a single-page website (no `chapters:`), assembled from
    // `subsections/` includes — not a book; the assertions below exercise TOC,
    // heading anchors, and document-order figure numbering on that one page.
    let dir = corpus_dir().join("bayesian-website");
    let src = fs::read_to_string(dir.join("index.qmd")).unwrap();
    let page = qmd_fast_core::render_html_page_with_includes(&src, &dir, "report");

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
fn reverse_sync_sourcepos_is_total() {
    // Reverse cursor-sync (`highlightAtLine` in web-client/client.js) scans every
    // `[data-sourcepos]` element and matches the strict regex `^(\d+):\d+-(\d+):\d+$`;
    // a non-matching sourcepos is silently skipped (the block becomes cursor-invisible).
    // So EVERY non-empty `data-sourcepos` in the emitted HTML must match that exact
    // format. Empty sourcepos (generated References/footnotes blocks) is exempt — those
    // blocks carry neither `data-block-id` nor `data-sourcepos`, so forward + reverse
    // sync agree they are not locatable.
    let re = |s: &str| -> bool {
        // crude ^(\d+):\d+-(\d+):\d+$ check without the regex crate
        let (a, b) = match s.split_once('-') {
            Some(x) => x,
            None => return false,
        };
        let ok = |p: &str| {
            let mut it = p.split(':');
            let (l, c) = (it.next(), it.next());
            it.next().is_none()
                && l.is_some_and(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()))
                && c.is_some_and(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()))
        };
        ok(a) && ok(b)
    };
    let mut files = Vec::new();
    collect_qmd(&corpus_dir(), &mut files);
    let mut offenders = Vec::new();
    for f in &files {
        let src = fs::read_to_string(f).unwrap();
        let base = f.parent().unwrap();
        let doc = qmd_fast_core::render_document_with_includes(&src, base);
        // Scan EVERY data-sourcepos="..." in the emitted HTML (what highlightAtLine sees),
        // not just top-level blocks — nested elements inside containers carry their own.
        let html = doc.body_html();
        let mut rest = html.as_str();
        while let Some(i) = rest.find("data-sourcepos=\"") {
            rest = &rest[i + "data-sourcepos=\"".len()..];
            let end = rest.find('"').unwrap_or(rest.len());
            let sp = &rest[..end];
            rest = &rest[end..];
            if !sp.is_empty() && !re(sp) {
                let label = f.strip_prefix(corpus_dir()).unwrap_or(f).display();
                offenders.push(format!("{label}: sourcepos={sp:?}"));
            }
        }
    }
    offenders.sort();
    offenders.dedup();
    assert!(
        offenders.is_empty(),
        "{} block(s) have a sourcepos that reverse cursor-sync cannot match \
         (must be `L:C-L:C`); fix at the attr-injection seam:\n{}",
        offenders.len(),
        offenders.join("\n")
    );
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
        !site.config.nav.left.is_empty(),
        "navbar items should parse from _site.yml"
    );
    for p in &site.pages {
        assert!(p.url.ends_with(".html"), "page url not .html: {}", p.url);
    }

    // A top-level page renders with the site chrome and rewrites its nav links.
    let blog = site.render_page("blog.qmd").expect("blog renders");
    assert!(blog.contains("qmd-site-nav"), "navbar missing");
    assert!(blog.contains("qmd-site-footer"), "footer missing");
    // Mobile nav toggle must stay a real, keyboard/SR-operable <button> (WCAG 2.1.1):
    // never regress to the old display:none `<input type=checkbox>` + role-less label hack.
    assert!(
        blog.contains("<button")
            && blog.contains("class=\"qmd-nav-burger\"")
            && blog.contains("aria-expanded")
            && blog.contains("aria-controls=\"qmd-nav-links\""),
        "mobile nav toggle must be an aria <button.qmd-nav-burger> controlling #qmd-nav-links"
    );
    assert!(
        blog.contains("id=\"qmd-nav-links\""),
        "nav menu must carry the controlled id"
    );
    // The exact old-hack signature (a checkbox input as the toggle) must be gone. (Bare
    // `type="checkbox"` would false-match the unrelated `input[type="checkbox"]` CSS selector.)
    assert!(
        !blog.contains("<input type=\"checkbox\" id=\"qmd-nav-toggle\""),
        "mobile nav must not use the inaccessible checkbox-hack toggle"
    );
    assert!(
        blog.contains("href=\"blog.html\""),
        "nav link not rewritten"
    );
    assert!(
        !blog.contains("href=\"blog.qmd\""),
        "raw .qmd nav link leaked"
    );
    // Blog-specific features were removed: no RSS feed is generated, so there is
    // no discovery <link>, no rss+xml, no feed.xml anywhere, and the footer's
    // local `.xml` item is dropped (there is no feed to point it at).
    assert!(!blog.contains("feed.xml"), "feed.xml link should be gone");
    assert!(
        !blog.contains("blog.xml"),
        "local .xml footer link should be dropped"
    );
    assert!(
        !blog.contains("application/rss+xml"),
        "RSS discovery <link> should be gone"
    );

    // A post is a plain page: no "back to listing" button and no post-nav at all,
    // but cross-page `.qmd` links are still rewritten to `.html`.
    let post = site
        .render_page("posts/evidence-lower-bound/index.qmd")
        .expect("post renders");
    assert!(
        !post.contains("qmd-back-link")
            && !post.contains("Back to Blog")
            && !post.contains("qmd-postnav"),
        "post should have no back-to-listing / post-nav"
    );
    assert!(
        post.contains("../KL-divergence/index.html"),
        "cross-page .qmd link not rewritten to .html"
    );
    assert!(
        !post.contains("../KL-divergence/index.qmd"),
        "raw cross-page .qmd link leaked"
    );

    // OpenGraph / SEO meta for sharing: a post gets og:type=article, og:title,
    // an absolute og:url + canonical (the site has a `url:`), a meta description,
    // and a twitter card.
    assert!(
        post.contains("property=\"og:type\" content=\"article\""),
        "post og:type should be article"
    );
    assert!(post.contains("property=\"og:title\""), "og:title missing");
    assert!(
        post.contains("property=\"og:url\" content=\"https://"),
        "absolute og:url missing"
    );
    assert!(
        post.contains("<meta name=\"description\""),
        "meta description missing"
    );
    assert!(
        post.contains("name=\"twitter:card\""),
        "twitter card missing"
    );
    assert!(
        post.contains("rel=\"canonical\" href=\"https://"),
        "canonical link missing"
    );

    // No reading-time decoration: a post's title block is like any page's.
    assert!(
        !post.contains("class=\"qmd-read-time\"") && !post.contains("min read"),
        "reading-time decoration should be gone"
    );

    // No per-tag archive pages, and a post no longer carries a category strip
    // linking to them (the in-listing category filter on the grid is unaffected).
    let fourier = site
        .render_page("posts/fourier-transform/index.qmd")
        .expect("fourier post renders");
    assert!(
        !fourier.contains("qmd-post-cats") && !fourier.contains("categories/signal-processing/"),
        "post should not carry a category archive strip"
    );
}

#[test]
fn standalone_doc_carries_opengraph_seo_meta() {
    // A single .qmd (no site) gets text OpenGraph/SEO meta from its own front matter.
    let doc = qmd_fast_core::render_document(
        "---\ntitle: \"T\"\ndescription: \"D\"\n---\n\n# Hi\n\nbody\n",
    );
    let page =
        qmd_fast_core::render_doc_to_page(&doc, "fallback", qmd_fast_core::OutputMode::Build);
    assert!(
        page.contains("property=\"og:title\" content=\"T\""),
        "og:title"
    );
    assert!(
        page.contains("property=\"og:description\" content=\"D\""),
        "og:description"
    );
    assert!(
        page.contains("name=\"description\" content=\"D\""),
        "meta description"
    );
    assert!(
        page.contains("property=\"og:type\" content=\"article\""),
        "og:type"
    );
    assert!(page.contains("name=\"twitter:card\""), "twitter card");

    // A doc with no description omits the description tags but still has og:title.
    let bare = qmd_fast_core::render_doc_to_page(
        &qmd_fast_core::render_document("---\ntitle: \"Only\"\n---\n\n# x\n"),
        "fb",
        qmd_fast_core::OutputMode::Build,
    );
    assert!(bare.contains("property=\"og:title\" content=\"Only\""));
    assert!(
        !bare.contains("name=\"description\""),
        "no description tag when absent"
    );
}

#[test]
fn bare_build_is_script_free_css_themed_and_drops_js() {
    // The `--bare` build target: zero <script>, zero CDN, CSS-only theming — yet
    // server-rendered math still works and a {js} cell is dropped (not shipped dead).
    let src = fs::read_to_string(corpus_dir().join("bare-draft.qmd")).unwrap();
    let doc = qmd_fast_core::render_document_with_includes(&src, &corpus_dir());
    let bare = qmd_fast_core::render_doc_to_page(&doc, "bare", qmd_fast_core::OutputMode::Bare);

    // The contract: not one <script> tag (no theme bootstrap, no enhancers, no {js}
    // runtime, no TOC/search) and no CDN host.
    assert!(
        !bare.contains("<script"),
        "bare output ships zero <script> tags"
    );
    assert!(!bare.contains("cdn.jsdelivr"), "no jsDelivr CDN reference");
    assert!(!bare.contains("unpkg.com"), "no unpkg CDN reference");

    // Server-rendered math survives a script-free page.
    assert!(
        bare.contains("class=\"katex"),
        "KaTeX math renders without JS"
    );

    // The {js} cell's runtime `<script type="application/qmd-js">` is stripped.
    assert!(
        !bare.contains("application/qmd-js"),
        "bare drops the {{js}} cell script block"
    );

    // Click-to-source must survive the {js}-script strip: the cell wrapper keeps its
    // block id + sourcepos (the strip removes only the inner <script>, and
    // `emit_js_cell` puts the block attrs on the outer <div>). This pins the
    // load-bearing block-model invariants on the bare-assembled page specifically.
    let cell_at = bare
        .find("class=\"cell qmd-js-cell\"")
        .expect("the {js} cell wrapper survives the strip");
    let tag_open = bare[..cell_at].rfind('<').expect("wrapper open tag");
    let wrapper_tag = &bare[tag_open..cell_at];
    assert!(
        wrapper_tag.contains("data-block-id=\"b-"),
        "bare {{js}} cell wrapper keeps its data-block-id: {wrapper_tag}"
    );
    assert!(
        wrapper_tag.contains("data-sourcepos=\""),
        "bare {{js}} cell wrapper keeps its data-sourcepos: {wrapper_tag}"
    );

    // Theming is CSS-only: an unforced (auto) theme follows the OS via a media query
    // that carries the dark layer rewritten from `[data-theme="dark"]` onto `:root`.
    assert!(
        bare.contains("@media (prefers-color-scheme: dark)"),
        "bare auto-theme uses a prefers-color-scheme media query"
    );
    assert!(
        bare.contains(":root .qhl-"),
        "the dark layer is rewritten from [data-theme] onto :root"
    );

    // Contrast: a normal (non-bare) build of the same doc DOES ship the enhancer
    // bundle and the {js} cell, proving `--bare` is what strips them.
    let build = qmd_fast_core::render_doc_to_page(&doc, "build", qmd_fast_core::OutputMode::Build);
    assert!(
        build.contains("<script"),
        "a normal build still ships scripts"
    );
    assert!(
        build.contains("application/qmd-js"),
        "a normal build keeps the {{js}} cell"
    );
}

#[test]
fn book_discovers_chapters_with_parts_numbering_and_chrome() {
    use qmd_fast_core::Site;
    let root = corpus_dir().join("demo-book");
    let site = Site::discover(&root);

    // Detected as a book; the chapter pages come from `book: chapters:` in order.
    assert!(site.is_book(), "demo-book should be a book project");
    assert_eq!(site.output_dir(), "_book", "book builds to _book");
    let book = site.book.as_ref().expect("book nav resolved");
    assert_eq!(book.title.as_deref(), Some("A Short Demo Book"));

    // The sidebar order: Preface (unnumbered), Introduction (1), the "Core" part
    // header, Methodology (2), Results (3), Wrap-up (4). "Methodology" and "Wrap-up"
    // come from per-chapter `{ file:, text: }` label overrides in `_site.yml`
    // (Methods/Summary are the chapters' own H1s, which the override replaces).
    let chapters: Vec<(&str, Option<u32>)> = book
        .entries
        .iter()
        .filter(|e| e.part.is_none())
        .map(|e| (e.title.as_str(), e.number))
        .collect();
    assert_eq!(
        chapters,
        vec![
            ("Preface", None),
            ("Introduction", Some(1)),
            ("Methodology", Some(2)),
            ("Results", Some(3)),
            ("Wrap-up", Some(4)),
        ],
        "chapter order + numbering with per-chapter label overrides (preface unnumbered)"
    );
    assert!(
        book.entries
            .iter()
            .any(|e| e.part.as_deref() == Some("Core")),
        "the `Core` part header should be in the sidebar"
    );

    // A chapter renders with the book chrome: left sidebar (active chapter),
    // section numbers on its headings, and prev/next-chapter navigation.
    let methods = site.render_page("methods.qmd").expect("methods renders");
    assert!(
        methods.contains("<nav class=\"qmd-book-sidebar\""),
        "book sidebar missing"
    );
    assert!(
        methods.contains("qmd-book-chapter qmd-book-active"),
        "active chapter not marked"
    );
    assert!(
        methods.contains("qmd-section-number\">2</span>")
            && methods.contains("qmd-section-number\">2.1</span>"),
        "chapter/section numbering missing"
    );
    assert!(
        methods.contains("qmd-book-postnav")
            && methods.contains("3  Results")
            && methods.contains("1  Introduction"),
        "prev/next-chapter navigation missing"
    );
    // A book uses the sidebar, not the website navbar element.
    assert!(
        !methods.contains("<header class=\"qmd-site-nav\""),
        "a book should not emit the website navbar"
    );

    // Cross-chapter `@ref`s resolve to the other page with the right number: the
    // Results chapter references `@sec-methods` (a chapter -> "Chapter 2") and
    // `@sec-setup` (a subsection -> "Section 2.1"), both on methods.html.
    let results = site.render_page("results.qmd").expect("results renders");
    assert!(
        results
            .contains("<a href=\"methods.html#sec-methods\" class=\"qmd-xref\">Chapter&nbsp;2</a>"),
        "cross-chapter ref to a chapter not resolved: {}",
        results
            .match_indices("qmd-xref")
            .next()
            .map(|_| "(see qmd-xref links)")
            .unwrap_or("(no qmd-xref at all)")
    );
    assert!(
        results
            .contains("<a href=\"methods.html#sec-setup\" class=\"qmd-xref\">Section&nbsp;2.1</a>"),
        "cross-chapter ref to a subsection not resolved"
    );
    // No unresolved marker should leak into the output once a target is known.
    assert!(
        !results.contains("data-qmd-xref=\"sec-methods\""),
        "resolved cross-ref still carries its marker"
    );
}
