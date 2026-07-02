//! Feature coverage for replacing Quarto when *iterating* on the author's
//! tech-blog (https://andreasbogossian.com). Three corpus posts come straight
//! from it: `em-algorithm`, `pca-geometry`, and `fourier-transform`.
//!
//! Each test locks in a slice of the per-post / per-site feature surface you'd
//! actually exercise in the edit-preview loop (math, callouts, citations,
//! `{js}`-as-live-output, listings, the about page, includes, leak-free whole-doc
//! rendering), all asserted against the *real* blog documents. Synthetic-input
//! unit tests for the same features live in the render module's `tests.rs`.
//!
//! History: this file used to also carry `#[ignore]`d tests marking known gaps
//! vs Quarto; every one of those gaps has since been closed, so only the
//! locked-in surface remains.

use std::fs;

use taliesin_core::{Site, render_document_with_includes};

mod common;
use common::corpus_dir;

/// Render a corpus post (resolving its includes) and return the body HTML.
fn render_post(rel: &str) -> String {
    let path = corpus_dir().join(rel);
    let base = path.parent().unwrap();
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
    render_document_with_includes(&src, base).body_html()
}

/// Shortcode / fenced-div / cell-option / attribute markers that must never
/// reach the output: if any leak, the live preview shows Quarto source instead
/// of rendered content.
fn assert_no_source_leaks(label: &str, html: &str) {
    for marker in [
        "{{<",
        ":::",
        "\n#|",
        "//|",
        "{#eq-",
        "{#sec-",
        "language-=html",
    ] {
        assert!(
            !html.contains(marker),
            "{label}: source marker {marker:?} leaked into rendered output"
        );
    }
}

// ---------------------------------------------------------------------------
// Per-post rendering features (math, callouts, citations, code-fold, `{js}` cells).
// ---------------------------------------------------------------------------

/// The three blog posts in the corpus all render to substantial, leak-free HTML.
/// This is the load-bearing guarantee for using qmd-fast as the edit-preview
/// loop while writing a post.
#[test]
fn tech_blog_posts_render_leak_free() {
    for post in [
        "posts/em-algorithm/index.qmd",
        "posts/pca-geometry/index.qmd",
        "posts/fourier-transform/index.qmd",
    ] {
        let html = render_post(post);
        assert!(
            html.len() > 2_000,
            "{post}: suspiciously small output ({} bytes)",
            html.len()
        );
        assert_no_source_leaks(post, &html);
    }
}

/// `em-algorithm` exercises the heavy-math feature set: inline `$...$`, display
/// `$$...$$`, and `\begin{align*}` blocks all reach KaTeX.
#[test]
fn math_renders_inline_display_and_align() {
    let html = render_post("posts/em-algorithm/index.qmd");
    assert!(
        html.contains("class=\"katex\""),
        "no inline KaTeX spans rendered"
    );
    assert!(
        html.contains("katex-display"),
        "no display-math (`$$`/align) rendered"
    );
    // sanity: this post is math-dense, so there should be many spans
    assert!(
        html.matches("class=\"katex\"").count() > 20,
        "far fewer KaTeX spans than expected"
    );
}

/// Callouts (`::: {.callout-note ...}`) render with a title and body, and the
/// fenced-div markers do not leak.
#[test]
fn callout_renders_with_title_and_body() {
    let html = render_post("posts/em-algorithm/index.qmd");
    assert!(html.contains("callout-note"), "callout-note class missing");
    assert!(
        html.contains("class=\"callout-title\""),
        "callout title missing"
    );
    assert!(
        html.contains("Notation used in this post"),
        "callout title text missing"
    );
    assert!(
        html.contains("class=\"callout-body\""),
        "callout body missing"
    );
}

/// Citations (`[@key]`, `[@key, chap. 9]`) become numbered links and a CSL
/// References section is generated at the end.
#[test]
fn citations_resolve_and_emit_references_section() {
    let html = render_post("posts/em-algorithm/index.qmd");
    assert!(
        html.contains("href=\"#ref-bishop2006pattern\""),
        "citation did not become an anchor to the reference"
    );
    assert!(
        html.contains("<h2>References</h2>"),
        "References section not generated"
    );
    assert!(
        html.contains("id=\"ref-bishop2006pattern\"") && html.contains("csl-entry"),
        "CSL reference entry missing"
    );
}

/// A callout with `collapse="true"` renders as a native `<details>` with the
/// title as its `<summary>`, so it is collapsible without any JavaScript.
#[test]
fn callout_collapse_renders_as_details() {
    let html = render_post("posts/em-algorithm/index.qmd");
    assert!(
        html.contains("callout-collapse"),
        "collapsible callout class missing"
    );
    // <details>/<summary> structure with the title as the summary (a kind icon now
    // precedes the title text inside the summary).
    assert!(
        html.contains("<details><summary class=\"callout-title\">")
            && html.contains("Notation used in this post</summary>"),
        "collapsible callout is not a <details>/<summary>"
    );
}

/// A code cell with `#| code-fold: true` wraps its listing in a `<details>`,
/// using `code-summary` as the disclosure label.
#[test]
fn code_fold_wraps_listing_in_details() {
    let html = render_post("posts/pca-geometry/index.qmd");
    assert!(
        html.contains("class=\"qmd-code-fold\""),
        "code-fold did not produce a <details>"
    );
    assert!(
        html.contains("<summary>How the data was generated</summary>"),
        "code-summary label missing from folded code"
    );
    // the folded listing still carries its block id (click-to-source intact)
    assert!(
        html.contains("<details data-block-id="),
        "folded code lost its block data attributes"
    );
}

/// `code-fold: true` with no `code-summary` falls back to the "Code" label.
#[test]
fn code_fold_defaults_to_code_label() {
    let html = render_post("posts/em-algorithm/index.qmd");
    assert!(
        html.contains("class=\"qmd-code-fold\""),
        "code-fold did not produce a <details>"
    );
    assert!(
        html.contains("<summary>Code</summary>"),
        "default code-fold label missing"
    );
}

/// `{js}` cells render as live placeholders (run client-side by the qmd-js
/// enhancer), not static highlighted source. Each emits a target div + an
/// `application/qmd-js` script, and `//|` option lines are stripped.
#[test]
fn js_cells_render_as_live_placeholders() {
    let html = render_post("posts/fourier-transform/index.qmd");
    assert!(
        html.contains("class=\"cell qmd-js-cell\""),
        "js cell not emitted as a live placeholder"
    );
    assert!(
        html.contains("<script type=\"application/qmd-js\""),
        "js cell missing its qmd-js script"
    );
    assert!(
        !html.contains("ojs-module-contents"),
        "the OJS wire format must be gone"
    );
    assert!(
        !html.contains("//| input"),
        "js cell-option line leaked into output"
    );
}

/// A page with `{js}` cells ships the vendored d3 + Plot libs (never the Observable
/// runtime); a prose-only page ships neither.
#[test]
fn js_page_ships_libs_when_cells_present() {
    let dir = corpus_dir().join("posts/fourier-transform");
    let src = std::fs::read_to_string(dir.join("index.qmd")).unwrap();
    let page = taliesin_core::render_html_page_with_includes(&src, &dir, "post");
    assert!(
        page.contains("@observablehq/plot") && page.contains("d3js.org"),
        "vendored Plot/d3 not shipped on a page with {{js}} cells"
    );
    assert!(
        !page.contains("quarto-ojs-runtime") && !page.contains("window._ojs"),
        "the Observable runtime must be gone"
    );

    // A doc with no live cells must not pay for the (large) libs.
    let prose =
        taliesin_core::render_html_page("---\ntitle: x\n---\n\nJust prose, no cells.\n", "p");
    assert!(
        !prose.contains("@observablehq/plot"),
        "Plot shipped on a doc with no cells"
    );
}

// ---------------------------------------------------------------------------
// Site-level + cross-document features (raw-HTML passthrough end-to-end,
// equation/figure cross-refs, listings, about, includes, favicon, 404).
// ---------------------------------------------------------------------------

/// The raw-HTML audio players in `fourier-transform` reach the output as live
/// `<audio>`/`<source>` elements (end-to-end, through the real post).
#[test]
fn fourier_audio_players_render_live() {
    let html = render_post("posts/fourier-transform/index.qmd");
    assert!(
        html.contains("<audio controls"),
        "audio players not passed through"
    );
    assert!(
        html.contains("src=\"chord.wav\""),
        "audio source reference missing"
    );
    assert!(!html.contains("&lt;audio"), "audio markup was escaped");
}

/// End-to-end in `fourier-transform`: the labelled equation gets `id="eq-dft"`
/// and the `@eq-dft` cross-reference resolves to a numbered "Equation N" link.
#[test]
fn equation_crossref_resolves_to_number() {
    let html = render_post("posts/fourier-transform/index.qmd");
    assert!(
        html.contains("id=\"eq-dft\""),
        "labelled equation id missing"
    );
    assert!(!html.contains("{#eq-dft}"), "equation label leaked as text");
    assert!(
        html.contains("<a href=\"#eq-dft\" class=\"qmd-xref\">Equation&nbsp;1</a>"),
        "@eq-dft did not resolve to a numbered Equation link"
    );
}

/// A `listing:` page (the blog index, projects index, and the homepage's "recent
/// posts") crawls its `contents:` directory and emits post cards. This is a
/// site-level feature, so it's exercised through `Site` against the real blog.
#[test]
fn listing_frontmatter_emits_post_cards() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));

    // The blog index lists every post as a card, newest-first.
    let blog = site.render_page("blog.qmd").expect("blog renders");
    assert!(
        blog.contains("qmd-listing-grid"),
        "blog: no listing grid produced"
    );
    let card_count = blog.matches("class=\"qmd-card\"").count();
    assert!(
        card_count >= 5,
        "blog: expected a card per post, got {card_count}"
    );
    // Cards link to the built `.html`, show a card title, and a category badge.
    assert!(
        blog.contains("href=\"posts/em-algorithm/index.html\""),
        "blog: post card link not rewritten to .html"
    );
    assert!(blog.contains("qmd-card-title"), "blog: card has no title");
    assert!(blog.contains("qmd-cat"), "blog: category badges missing");
    // Newest-first: the latest-dated post's card precedes an older one.
    let fourier = blog.find("posts/fourier-transform/").unwrap(); // 2026-05-15
    let em = blog.find("posts/em-algorithm/").unwrap(); // 2026-04-14
    assert!(fourier < em, "blog: cards not sorted newest-first");
    // Grid card <img> carries the post's front-matter `image-alt:` (a11y), not an
    // empty alt: every tech-blog post supplies one.
    assert!(
        blog.contains(
            "alt=\"Visualisation of the EM-algorithm fitting two Gaussian distributions to unlabelled data\""
        ),
        "blog: card image-alt not emitted from post front matter"
    );

    // The homepage fills its `::: {#recent-posts}` placeholder, capped at 2.
    let home = site.render_page("index.qmd").expect("home renders");
    assert!(
        home.contains("id=\"recent-posts\""),
        "home: recent-posts container missing"
    );
    let recent = home.matches("class=\"qmd-card\"").count();
    assert_eq!(recent, 2, "home: max-items: 2 not honoured (got {recent})");
}

/// An `about:` page (the homepage uses `template: jolla`) renders a profile block
/// from the front matter. Site-level, so exercised through `Site` on the real blog.
#[test]
fn about_page_renders_profile_block() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let home = site.render_page("index.qmd").expect("home renders");
    assert!(
        home.contains("qmd-about-jolla"),
        "about: produced no jolla profile block"
    );
    assert!(
        home.contains("qmd-about-img") && home.contains("src=\"profile.webp\""),
        "about: profile image missing"
    );
    assert!(
        home.contains("<h1 class=\"qmd-about-name\">Andreas Bogossian</h1>"),
        "about: name (page title) missing"
    );
    // The profile replaces the default title block (no duplicate header).
    assert!(
        !home.contains("class=\"qmd-title-block\""),
        "about: default title block should be replaced"
    );
}

/// The site's native `head` / `body-end` / `css` (from `_site.yml`) are injected
/// into every page, and a page's own front-matter `include-in-header` is injected on
/// top of the site's. Without this the blog's preconnect hints, prefetch script, and
/// custom stylesheet silently vanish.
#[test]
fn site_and_page_includes_are_injected() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));

    // A regular post gets the site-wide includes (it declares none of its own).
    let post = site
        .render_page("posts/em-algorithm/index.qmd")
        .expect("post renders");
    // include-in-header (text:) — preconnect + speculationrules.
    assert!(
        post.contains("rel=\"preconnect\" href=\"https://cdn.jsdelivr.net\""),
        "site include-in-header preconnect missing"
    );
    assert!(
        post.contains("<script type=\"speculationrules\">"),
        "site include-in-header speculationrules missing"
    );
    // include-after-body (text:) — the prefetch script.
    assert!(
        post.contains("src=\"/instantpage.js\""),
        "site include-after-body script missing"
    );
    // css: custom.css — inlined, so a known selector from the file is present.
    assert!(
        post.contains(".back-to-top"),
        "site css (custom.css) not inlined"
    );
    // Header injection lands inside <head>, body injection after </body>'s content.
    let head = &post[..post.find("</head>").expect("has </head>")];
    assert!(
        head.contains("speculationrules"),
        "include-in-header must be inside <head>"
    );

    // The homepage adds its OWN include-in-header on top of the site's.
    let home = site.render_page("index.qmd").expect("home renders");
    assert!(
        home.contains("name=\"description\" content=\"MSc student"),
        "page-level include-in-header (meta description) missing"
    );
    assert!(
        home.contains("rel=\"preconnect\" href=\"https://cdn.jsdelivr.net\""),
        "site include should still apply alongside the page's own"
    );
}

/// The site `favicon:` is emitted as a `<link rel="icon">` with a depth-relative
/// href, on every page. Without it the browser auto-requests `/favicon.ico` and
/// 404s on a static deploy (the bug the deploy-validation pass caught).
#[test]
fn site_favicon_link_is_emitted_depth_relative() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let home = site.render_page("index.qmd").expect("home renders");
    assert!(
        home.contains(r#"<link rel="icon" type="image/svg+xml" href="bell-curve.svg" />"#),
        "root page favicon link missing/!depth-relative"
    );
    let post = site
        .render_page("posts/em-algorithm/index.qmd")
        .expect("post renders");
    assert!(
        post.contains(r#"href="../../bell-curve.svg""#),
        "post favicon link not resolved relative to its depth"
    );
}

/// The site emits a self-contained `404.html`: a complete page, on-theme, whose
/// links are root-absolute (it is served at arbitrary depth, so depth-relative
/// `../` links would resolve against the wrong directory).
#[test]
fn site_404_page_is_self_contained_with_absolute_links() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let page = site.render_404_page();
    assert!(
        page.contains("<!doctype") || page.contains("<html"),
        "not a full page"
    );
    assert!(
        page.contains("404") && page.contains("Page not found"),
        "missing 404 body"
    );
    // The one home link is root-absolute, not depth-relative.
    assert!(page.contains(r#"href="/""#), "home link not root-absolute");
    // No relative `../` link anywhere (would break when served at depth), and the
    // favicon is the inlined data URI, not a relative file ref.
    assert!(
        !page.contains("href=\"../"),
        "404 page has a depth-relative link"
    );
    assert!(
        page.contains("data:image/svg+xml;base64,"),
        "favicon not inlined"
    );
}

/// Every `{js}` cell across the interactive posts is a live placeholder whose
/// target div id matches its `application/qmd-js` script's `data-target`.
#[test]
fn every_js_cell_has_matching_target_and_script() {
    for post in [
        "posts/fourier-transform/index.qmd",
        "posts/em-algorithm/index.qmd",
        "posts/pca-geometry/index.qmd",
    ] {
        let html = render_post(post);
        let cells = html.matches("class=\"cell qmd-js-cell\"").count();
        let scripts = html.matches("<script type=\"application/qmd-js\"").count();
        assert!(cells > 0, "{post}: no live js cells emitted");
        assert_eq!(cells, scripts, "{post}: cell/script count mismatch");
        // every target div id is the data-target of a qmd-js script
        for id in js_target_ids(&html) {
            assert!(
                html.contains(&format!("data-target=\"qmd-js-{id}\"")),
                "{post}: qmd-js-{id} has no matching script"
            );
        }
    }
}

/// Cross-references to computed outputs resolve to numbers, and the
/// client-rendered targets (`{js}` figures, code listings) carry their anchors at
/// render time. (Python figure outputs are wrapped by the executor; see serve.)
#[test]
fn computed_output_crossrefs_resolve() {
    // fourier: a matplotlib figure (fig-components) gets a number; the {js} winding
    // figure is a real <figure> anchor.
    let f = render_post("posts/fourier-transform/index.qmd");
    assert!(
        f.contains("<a href=\"#fig-components\" class=\"qmd-xref\">Figure&nbsp;1</a>"),
        "@fig-components did not resolve to a numbered link"
    );
    assert!(
        f.contains("id=\"fig-winding\""),
        "labelled js-cell figure anchor missing"
    );

    // pca: a {js} figure (fig-3d-pca) and a code listing (lst-data-generation)
    // resolve to numbered, anchored targets at render time.
    let p = render_post("posts/pca-geometry/index.qmd");
    assert!(
        p.contains("id=\"fig-3d-pca\""),
        "js-cell figure anchor missing"
    );
    assert!(
        p.contains("<a href=\"#fig-3d-pca\" class=\"qmd-xref\">Figure&nbsp;"),
        "@fig-3d-pca did not resolve to a numbered link"
    );
    assert!(
        p.contains("class=\"qmd-listing\"") && p.contains("id=\"lst-data-generation\""),
        "code listing anchor missing"
    );
    assert!(
        p.contains("<a href=\"#lst-data-generation\" class=\"qmd-xref\">Listing&nbsp;1</a>"),
        "@lst-data-generation did not resolve to a numbered Listing link"
    );
    // a Python matplotlib figure still resolves to a number (its anchor appears
    // once executed — verified live in serve, not here).
    assert!(
        p.contains("<a href=\"#fig-cov\" class=\"qmd-xref\">Figure&nbsp;"),
        "@fig-cov did not resolve to a numbered link"
    );
}

/// Pull the block ids out of `id="qmd-js-<id>"` target divs.
fn js_target_ids(html: &str) -> Vec<String> {
    html.match_indices("id=\"qmd-js-")
        .filter_map(|(i, _)| {
            let rest = &html[i + "id=\"qmd-js-".len()..];
            rest.split('"').next().map(str::to_string)
        })
        .collect()
}
