//! Feature coverage for *iterating* on the author's
//! tech-blog (https://andreasbogossian.com). Three corpus posts come straight
//! from it: `em-algorithm`, `pca-geometry`, and `fourier-transform`.
//!
//! Each test locks in a slice of the per-post / per-site feature surface you'd
//! actually exercise in the edit-preview loop (math, callouts, citations,
//! `{js}`-as-live-output, listings, the about page, includes, leak-free whole-doc
//! rendering), all asserted against the *real* blog documents. Synthetic-input
//! unit tests for the same features live in the render module's `tests.rs`.
//!
//! History: this file used to also carry `#[ignore]`d tests marking known
//! feature gaps; every one of those gaps has since been closed, so only the
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
/// reach the output: if any leak, the live preview shows raw source instead
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
/// This is the load-bearing guarantee for using taliesin as the edit-preview
/// loop while writing a post.
#[test]
fn tech_blog_posts_render_leak_free() {
    for post in [
        "posts/em-algorithm/index.tmd",
        "posts/pca-geometry/index.tmd",
        "posts/fourier-transform/index.tmd",
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
    let html = render_post("posts/em-algorithm/index.tmd");
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
    let html = render_post("posts/em-algorithm/index.tmd");
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
    let html = render_post("posts/em-algorithm/index.tmd");
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
    let html = render_post("posts/em-algorithm/index.tmd");
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
    let html = render_post("posts/pca-geometry/index.tmd");
    assert!(
        html.contains("class=\"tali-code-fold\""),
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
    let html = render_post("posts/em-algorithm/index.tmd");
    assert!(
        html.contains("class=\"tali-code-fold\""),
        "code-fold did not produce a <details>"
    );
    assert!(
        html.contains("<summary>Code</summary>"),
        "default code-fold label missing"
    );
}

/// `{js}` cells render as live placeholders (run client-side by the tali-js
/// enhancer), not static highlighted source. Each emits a target div + an
/// `application/tali-js` script, and `//|` option lines are stripped.
#[test]
fn js_cells_render_as_live_placeholders() {
    let html = render_post("posts/fourier-transform/index.tmd");
    assert!(
        html.contains("class=\"cell tali-js-cell\""),
        "js cell not emitted as a live placeholder"
    );
    assert!(
        html.contains("<script type=\"application/tali-js\""),
        "js cell missing its tali-js script"
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
    let src = std::fs::read_to_string(dir.join("index.tmd")).unwrap();
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
    let html = render_post("posts/fourier-transform/index.tmd");
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
    let html = render_post("posts/fourier-transform/index.tmd");
    assert!(
        html.contains("id=\"eq-dft\""),
        "labelled equation id missing"
    );
    assert!(!html.contains("{#eq-dft}"), "equation label leaked as text");
    assert!(
        html.contains("<a href=\"#eq-dft\" class=\"tali-xref\">Equation&nbsp;1</a>"),
        "@eq-dft did not resolve to a numbered Equation link"
    );
}

/// A `listing:` page (the blog index, projects index, and the homepage's "recent
/// posts") crawls its `contents:` directory and emits post cards. This is a
/// site-level feature, so it's exercised through `Site` against the real blog.
#[test]
fn listing_frontmatter_emits_post_cards() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));

    // The blog index lists every post as a card, newest-first. It uses the reading-first
    // `type: list` layout (stacked rows → class `tali-listing-default`), NOT the grid.
    // Match the emitted class ATTRIBUTE: the inlined site.css names every class as a bare
    // rule selector, so a `contains("tali-listing-grid")` would pass vacuously off the CSS.
    let blog = site.render_page("blog.tmd").expect("blog renders");
    assert!(
        blog.contains("class=\"tali-listing tali-listing-default\"")
            && !blog.contains("class=\"tali-listing tali-listing-grid\""),
        "blog: reading-first list layout not emitted"
    );
    let card_count = blog.matches("class=\"tali-card\"").count();
    assert!(
        card_count >= 5,
        "blog: expected a card per post, got {card_count}"
    );
    // Cards link to the built `.html`, show a card title, and a category badge.
    assert!(
        blog.contains("href=\"posts/em-algorithm/index.html\""),
        "blog: post card link not rewritten to .html"
    );
    assert!(blog.contains("tali-card-title"), "blog: card has no title");
    // The category-filter chip row was deleted 2026-08-03; each card still carries its
    // own category badges (page-level `categories:` front matter survives).
    assert!(blog.contains("tali-cat"), "blog: category badges missing");
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
    let home = site.render_page("index.tmd").expect("home renders");
    assert!(
        home.contains("id=\"recent-posts\""),
        "home: recent-posts container missing"
    );
    let recent = home.matches("class=\"tali-card\"").count();
    assert_eq!(recent, 2, "home: max-items: 2 not honoured (got {recent})");
}

/// Post dates render humanized ("14 April 2026"), never the raw ISO string, in both the
/// post title block and the listing cards. The machine-readable ISO stays where machines
/// read it (JSON-LD `datePublished`, `citation_*`, the feed) — so the check is scoped to
/// the visible regions, not the whole page.
#[test]
fn dates_are_humanized_in_the_title_block_and_cards() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    // Post title block (em-algorithm, dated 2026-04-14).
    let post = site
        .render_page("posts/em-algorithm/index.tmd")
        .expect("post");
    let meta = post
        .split("class=\"tali-title-meta\">") // the element, not the CSS selector
        .nth(1)
        .and_then(|s| s.split("</div>").next())
        .expect("title meta div");
    // The date is a `<time datetime>` (PA-M1): the ISO is machine-readable in the attribute,
    // the humanized form is the visible text — the raw ISO must never be the visible text.
    assert!(
        meta.contains("<time datetime=\"2026-04-14\">14 April 2026</time>"),
        "post date is a <time> with ISO attr + humanized text: {meta}"
    );
    // Listing cards (fourier 2026-05-15, em 2026-04-14): each a `<time class="tali-card-date">`.
    let blog = site.render_page("blog.tmd").expect("blog");
    assert!(
        blog.contains("<time class=\"tali-card-date\" datetime=\"2026-05-15\">15 May 2026</time>"),
        "fourier card date is a humanized <time>"
    );
    assert!(
        blog.contains(
            "<time class=\"tali-card-date\" datetime=\"2026-04-14\">14 April 2026</time>"
        ),
        "em card date is a humanized <time>"
    );
}

/// Reading time shows on posts (a dated title block) but not on undated pages (the CV,
/// listing indexes) — the gate is the same `date:` that marks an article.
#[test]
fn reading_time_shows_on_posts_not_on_undated_pages() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let post = site
        .render_page("posts/em-algorithm/index.tmd")
        .expect("post");
    assert!(
        post.contains("class=\"tali-read-time\"") && post.contains(" min read"),
        "a post shows a reading-time estimate"
    );
    // The CV has no `date:` → no reading time.
    let cv = site.render_page("cv.tmd").expect("cv");
    assert!(
        !cv.contains("tali-read-time"),
        "an undated page shows no reading time"
    );
    // A listing index (blog) has no `date:` either.
    let blog = site.render_page("blog.tmd").expect("blog");
    assert!(
        !blog.contains("tali-read-time"),
        "a listing index shows no reading time"
    );
}

/// The homepage renders the Marginalia hero (native text-only `hero:`), not the old
/// Quarto `about: jolla` profile block. Site-level, exercised on the real blog.
#[test]
fn home_page_renders_marginalia_hero() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let home = site.render_page("index.tmd").expect("home renders");
    assert!(
        home.contains("<header class=\"hero\""),
        "homepage: Marginalia hero header missing"
    );
    assert!(
        home.contains("ML · STATISTICS · ALGORITHMS"),
        "homepage: mono eyebrow missing"
    );
    assert!(
        home.contains("<h1>Machine learning, worked out from first principles</h1>"),
        "homepage: POV headline missing"
    );
    assert!(
        home.contains("Notes on concepts I'm working to understand"),
        "homepage: trimmed lead missing"
    );
    // The photo was removed (let the content speak): the hero is text-only, no portrait.
    assert!(
        !home.contains("<img class=\"hero-media\""),
        "homepage: hero should be photoless"
    );
    // The `about:` block was removed from the framework (superseded by `hero:`), so its
    // `.tali-about*` markup must never appear.
    assert!(
        !home.contains("class=\"tali-about"),
        "about: header markup should be gone (the feature was removed)"
    );
    assert!(
        !home.contains("class=\"tali-title-block\""),
        "homepage: hero should replace the default title block"
    );
}

/// The de-Quarto sweep stays swept. The blog dropped its Quarto nav-prefetch stack (CDN
/// preconnects, a speculationrules prerender hint, the third-party instant.page module),
/// all redundant with Taliesin's native hover-preview, so none may reappear; offline-first
/// means zero external connections. The site `description:` is the single source of truth
/// (exactly one `<meta description>`).
///
/// **This test used to end by asserting the page contains `@view-transition`, as proof
/// that the site-level `css:` was still inlined.** That needle stopped proving anything
/// the moment the rule moved into the bundled `base.css`, which every page inlines whole —
/// it would have passed on a page with no `css:` at all, the standing inlined-asset trap.
/// The blog's `custom.css` is gone with it (the 2026-07-11 audit's own prescription: its
/// last live rule *belonged in* base.css), so there is no longer a `css:` claim to make
/// here. The `css:` mechanism keeps its own coverage in
/// `theme_css::a_custom_css_theme_file_is_read_from_disk_and_inlined` and in `config.rs`.
///
/// **If one of the negative assertions below fails, suspect the bundled assets before the
/// site config.** The same whole-page inlining cuts the other way: a *comment* in
/// `base.css` or the JS payload that merely names a forbidden token fails this test with
/// no config change at all. Not hypothetical — it happened while promoting
/// `@view-transition` into `base.css`, whose comment named the two dropped mechanisms.
#[test]
fn blog_nav_prefetch_stack_stays_dropped() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));

    let post = site
        .render_page("posts/em-algorithm/index.tmd")
        .expect("post renders");
    assert!(
        !post.contains("rel=\"preconnect\""),
        "no CDN preconnect may survive (offline-first)"
    );
    assert!(
        !post.contains("speculationrules"),
        "the speculationrules prerender hint was dropped"
    );
    assert!(
        !post.contains("instantpage"),
        "the third-party instant.page module was dropped"
    );
    // The site-level `description:` is the single source of truth for the meta description.
    let home = site.render_page("index.tmd").expect("home renders");
    assert_eq!(
        home.matches("name=\"description\"").count(),
        1,
        "homepage must carry exactly one <meta description>"
    );
}

/// The site `favicon:` is emitted as a `<link rel="icon">` with a depth-relative
/// href, on every page. Without it the browser auto-requests `/favicon.ico` and
/// 404s on a static deploy (the bug the deploy-validation pass caught).
#[test]
fn site_favicon_link_is_emitted_depth_relative() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let home = site.render_page("index.tmd").expect("home renders");
    assert!(
        home.contains(r#"<link rel="icon" type="image/svg+xml" href="bell-curve.svg" />"#),
        "root page favicon link missing/!depth-relative"
    );
    let post = site
        .render_page("posts/em-algorithm/index.tmd")
        .expect("post renders");
    assert!(
        post.contains(r#"href="../../bell-curve.svg""#),
        "post favicon link not resolved relative to its depth"
    );
}

/// The site `logo:` is the navbar brand's image, with the site `title:` as its alt, and
/// its href resolves depth-relative exactly like `favicon:` (a post two levels down has
/// to climb back out, or the mirrored file 404s in `_site/`). Needles the whole
/// `<a …><img …></a>` construct: every page inlines the full CSS + JS payload, so a bare
/// `contains("logo")` is satisfied by the stylesheet and would pass with nothing rendered.
#[test]
fn site_logo_is_the_navbar_brand_image_depth_relative() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let home = site.render_page("index.tmd").expect("home renders");
    assert!(
        home.contains(
            "<a class=\"tali-nav-brand\" href=\"index.html\">\
             <img class=\"tali-brand-logo\" src=\"logo.svg\" alt=\"Andreas Bogossian\" /></a>"
        ),
        "root page navbar brand is not the configured logo"
    );
    let post = site
        .render_page("posts/em-algorithm/index.tmd")
        .expect("post renders");
    assert!(
        post.contains(
            "<img class=\"tali-brand-logo\" src=\"../../logo.svg\" alt=\"Andreas Bogossian\" />"
        ),
        "post logo not resolved relative to its depth"
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

/// The build's form of the same page links the shared `_assets/` bundle. Both renderers
/// exist on purpose: the live preview has no `_assets/` to link, so it keeps the
/// self-contained form above, and a build has one for every other page.
#[test]
fn site_404_page_links_the_shared_bundle_in_a_build() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let page = site.render_404_page_external(taliesin_core::ExternalAssets {
        app_css: "/_assets/app.aaaa.css",
        katex_css: "/_assets/katex.bbbb.css",
        app_js: "/_assets/app.cccc.js",
        mermaid_js: "",
        jslibs_js: "",
        deck_css: "",
        deck_js: "",
        font_preload: "/_assets/newsreader-latin-wght-normal.dddd.woff2",
    });
    assert!(
        page.contains(
            r#"<link rel="preload" as="font" type="font/woff2" href="/_assets/newsreader-latin-wght-normal.dddd.woff2" crossorigin>"#
        ),
        "the body face is preloaded ahead of the sheet that would otherwise discover it"
    );
    assert!(
        page.contains(r#"<link rel="stylesheet" href="/_assets/app.aaaa.css">"#),
        "the framework CSS is linked, not inlined"
    );
    assert!(
        page.contains(r#"<script src="/_assets/app.cccc.js" defer></script>"#),
        "the enhancers are linked, not inlined"
    );
    // The page's own scoped style stays inline, so the layout survives even where the
    // stylesheet does not resolve (a project-subpath deploy, which the root-absolute hrefs
    // do not support any more than the `/` home link does).
    assert!(
        page.contains(".tali-404-code{"),
        "scoped style still inline"
    );
    // Still a 404 page, still absolutely linked.
    assert!(
        page.contains(r#"href="/""#),
        "home link still root-absolute"
    );
    assert!(!page.contains("href=\"../"), "no depth-relative link");
    // Nothing conditional got linked: this page has no math and no mermaid.
    assert!(!page.contains("katex."), "no katex on a page with no math");
}

/// Every `{js}` cell across the interactive posts is a live placeholder whose
/// target div id matches its `application/tali-js` script's `data-target`.
#[test]
fn every_js_cell_has_matching_target_and_script() {
    for post in [
        "posts/fourier-transform/index.tmd",
        "posts/em-algorithm/index.tmd",
        "posts/pca-geometry/index.tmd",
    ] {
        let html = render_post(post);
        let cells = html.matches("class=\"cell tali-js-cell\"").count();
        let scripts = html.matches("<script type=\"application/tali-js\"").count();
        assert!(cells > 0, "{post}: no live js cells emitted");
        assert_eq!(cells, scripts, "{post}: cell/script count mismatch");
        // every target div id is the data-target of a tali-js script
        for id in js_target_ids(&html) {
            assert!(
                html.contains(&format!("data-target=\"tali-js-{id}\"")),
                "{post}: tali-js-{id} has no matching script"
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
    let f = render_post("posts/fourier-transform/index.tmd");
    assert!(
        f.contains("<a href=\"#fig-components\" class=\"tali-xref\">Figure&nbsp;1</a>"),
        "@fig-components did not resolve to a numbered link"
    );
    assert!(
        f.contains("id=\"fig-winding\""),
        "labelled js-cell figure anchor missing"
    );

    // pca: a {js} figure (fig-3d-pca) and a code listing (lst-data-generation)
    // resolve to numbered, anchored targets at render time.
    let p = render_post("posts/pca-geometry/index.tmd");
    assert!(
        p.contains("id=\"fig-3d-pca\""),
        "js-cell figure anchor missing"
    );
    assert!(
        p.contains("<a href=\"#fig-3d-pca\" class=\"tali-xref\">Figure&nbsp;"),
        "@fig-3d-pca did not resolve to a numbered link"
    );
    assert!(
        p.contains("class=\"tali-listing\"") && p.contains("id=\"lst-data-generation\""),
        "code listing anchor missing"
    );
    assert!(
        p.contains("<a href=\"#lst-data-generation\" class=\"tali-xref\">Listing&nbsp;1</a>"),
        "@lst-data-generation did not resolve to a numbered Listing link"
    );
    // a Python matplotlib figure still resolves to a number (its anchor appears
    // once executed — verified live in serve, not here).
    assert!(
        p.contains("<a href=\"#fig-cov\" class=\"tali-xref\">Figure&nbsp;"),
        "@fig-cov did not resolve to a numbered link"
    );
}

/// Every website post/project links back to the single un-capped listing that owns
/// it ("← Blog" / "← Projects"), resolved relative to the post's depth. The Home
/// page's `recent-posts` preview is `max-items`-capped, so it does NOT count as an
/// owner — posts resolve uniquely to the full Blog listing rather than reading as
/// ambiguous. Pages that belong to no listing (the listing pages themselves, the
/// about/home page, standalone nav pages) show no backlink.
#[test]
fn post_pages_link_back_to_their_listing() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));

    // A post (posts/<slug>/ = depth 2): owned only by blog.tmd → "← Blog".
    let post = site
        .render_page("posts/em-algorithm/index.tmd")
        .expect("post renders");
    assert!(
        post.contains("<nav class=\"tali-postnav tali-listing-backnav\""),
        "post: no back-to-listing link rendered"
    );
    assert!(
        post.contains("href=\"../../blog.html\""),
        "post: backlink not resolved to the Blog listing at the right depth"
    );
    assert!(
        post.contains("</span> Blog</a>"),
        "post: backlink label is not the owning listing page's title"
    );

    // A project belongs to NO single listing here: both projects.tmd AND cv.tmd (its
    // "selected projects" section) list `contents: projects` un-capped, so the owner is
    // genuinely ambiguous and the rule correctly skips the backlink. This pins the
    // ambiguity guard against real corpus content, not just a synthetic fixture.
    let project = site
        .render_page("projects/iphone-premium-analysis/index.tmd")
        .expect("project renders");
    assert!(
        !project.contains("<nav class=\"tali-postnav tali-listing-backnav\""),
        "project: ambiguous owner (Projects page + CV both list projects) → no backlink"
    );

    // Pages that belong to no listing show no backlink.
    for page in [
        "index.tmd",
        "blog.tmd",
        "projects.tmd",
        "cv.tmd",
        "publications.tmd",
    ] {
        let html = site
            .render_page(page)
            .unwrap_or_else(|| panic!("{page} renders"));
        assert!(
            !html.contains("<nav class=\"tali-postnav tali-listing-backnav\""),
            "{page}: should have no back-to-listing link"
        );
    }
}

/// Pull the block ids out of `id="tali-js-<id>"` target divs.
fn js_target_ids(html: &str) -> Vec<String> {
    html.match_indices("id=\"tali-js-")
        .filter_map(|(i, _)| {
            let rest = &html[i + "id=\"tali-js-".len()..];
            rest.split('"').next().map(str::to_string)
        })
        .collect()
}

/// The real blog has `url:` set, so `build` emits the discoverability sidecars. Pins
/// each artifact against the actual corpus (the regression net) — matching emitted
/// strings, never inlined-CSS/JS substrings ("gate the gate").
#[test]
fn seo_and_llm_artifacts_are_generated_for_the_blog() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let base = "https://andreasbogossian.com";

    // Atom feeds: one per distinct uncapped dated listing (blog + projects); the CV's
    // re-listed projects tail is deduped, and the homepage teaser is capped → no feed.
    let feeds = site.atom_feeds();
    let paths: Vec<&str> = feeds.iter().map(|(p, _)| p.as_str()).collect();
    assert!(paths.contains(&"blog.xml"), "blog feed: {paths:?}");
    assert!(paths.contains(&"projects.xml"), "projects feed: {paths:?}");
    assert!(
        !paths.contains(&"cv.xml"),
        "no duplicate CV feed: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.starts_with("index")),
        "no feed for the capped homepage teaser: {paths:?}"
    );
    let (_, blog_xml) = feeds.iter().find(|(p, _)| p == "blog.xml").unwrap();
    assert!(
        blog_xml.contains(r#"<feed xmlns="http://www.w3.org/2005/Atom">"#),
        "atom root"
    );
    assert!(
        blog_xml.contains(&format!(r#"href="{base}/posts/"#)),
        "absolute post links"
    );
    assert!(
        blog_xml.matches("<entry>").count() >= 5,
        "an entry per post, got {}",
        blog_xml.matches("<entry>").count()
    );

    // sitemap + robots.
    let sitemap = site.sitemap().expect("sitemap");
    assert!(
        sitemap.contains(&format!("<loc>{base}/</loc>")),
        "home in sitemap"
    );
    assert!(
        sitemap.contains(&format!("<loc>{base}/posts/em-algorithm/</loc>")),
        "a post in sitemap"
    );
    assert!(
        !sitemap.contains("404"),
        "the 404 page is not in the sitemap"
    );
    let robots = site.robots().expect("robots");
    assert!(
        robots.contains(&format!("Sitemap: {base}/sitemap.xml")),
        "robots names sitemap"
    );

    // llms.txt: identity from the home hero + linked posts.
    let llms = site.llms_txt().expect("llms.txt");
    assert!(llms.starts_with("# Andreas Bogossian"), "identity H1");
    assert!(
        llms.contains(&format!("]({base}/posts/")),
        "posts linked absolutely"
    );
    assert!(!llms.contains("404"), "the 404 page is not in llms.txt");
    // llms-full.txt: real prose, identity header.
    let full = site.llms_full_txt().expect("llms-full.txt");
    assert!(full.contains("Andreas Bogossian"), "identity header");
    assert!(
        full.len() > 2000,
        "carries real page prose, got {} bytes",
        full.len()
    );

    // JSON-LD in the rendered pages. `em-algorithm` declares a `bibliography:`, so it is a
    // cited/scholarly document and upgrades from `BlogPosting` to `ScholarlyArticle`
    // (author-free — no `author:` is set on any tech-blog post).
    let post = site
        .render_page("posts/em-algorithm/index.tmd")
        .expect("post renders");
    assert!(
        post.contains(r#""@type":"ScholarlyArticle""#),
        "a bibliography-bearing post is a ScholarlyArticle"
    );
    assert!(
        !post.contains(r#""@type":"BlogPosting""#),
        "the scholarly post must not also carry BlogPosting"
    );
    let home = site.render_page("index.tmd").expect("home renders");
    assert!(
        home.contains(r#""@type":"WebSite""#) && home.contains(r#""@type":"Person""#),
        "WebSite+Person on home"
    );

    // Footer feed link honored (url: is set).
    let blog = site.render_page("blog.tmd").expect("blog renders");
    assert!(
        blog.contains("href=\"blog.xml\""),
        "footer feed link honored"
    );

    // Feed autodiscovery: every rendered page advertises the site's Atom feed(s) in its
    // <head> so a browser/reader detects them — distinct from the human-only footer link
    // (relative `blog.xml`); autodiscovery is absolute, gated on `url:` like the feeds.
    let autodiscover = format!(
        r#"<link rel="alternate" type="application/atom+xml" title="Blog" href="{base}/blog.xml">"#
    );
    for (label, page) in [("blog", &blog), ("post", &post), ("home", &home)] {
        assert!(
            page.contains(&autodiscover),
            "{label} advertises the blog feed in <head>"
        );
    }
}

/// Every inner page's `<title>` names both the page and the site (" · <site>") so a
/// browser tab / search result is unambiguous; the home (root index) and any page whose
/// own title is already exactly the site name (the CV) stay bare — the collapse rule.
#[test]
fn page_titles_carry_the_site_name_suffix() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let site_name = "Andreas Bogossian";
    let title_of = |rel: &str| -> String {
        let html = site.render_page(rel).expect("renders");
        let start = html.find("<title>").expect("has <title>") + "<title>".len();
        let end = start + html[start..].find("</title>").expect("has </title>");
        html[start..end].to_string()
    };
    // Home + CV (titled exactly the site name) stay bare — no suffix, no "Name · Name".
    assert_eq!(title_of("index.tmd"), site_name, "home bare");
    assert_eq!(
        title_of("cv.tmd"),
        site_name,
        "CV title == site name collapses to bare"
    );
    // Distinct inner pages name page + site.
    assert_eq!(title_of("blog.tmd"), format!("Blog · {site_name}"));
    let post = title_of("posts/em-algorithm/index.tmd");
    assert!(
        post.ends_with(&format!(" · {site_name}")) && post != site_name,
        "a post names page + site: {post:?}"
    );
}

use taliesin_core::site::{card_rel_path, card_spec};

/// The blog home ships a generated OG card (never the removed static `og-image.webp`),
/// and its og:image URL is exactly the card path the build writes.
#[test]
fn home_og_image_is_the_generated_card() {
    let site = Site::discover(&corpus_dir().join("tech-blog"));
    let home = site
        .pages
        .iter()
        .find(|p| p.url == "index.html")
        .expect("home page");
    let rel = card_rel_path(&card_spec(&site, home)); // "og/<hex>.png"
    let html = site.render_page("index.tmd").unwrap();
    assert!(
        html.contains(&format!(
            r#"property="og:image" content="https://andreasbogossian.com/{rel}""#
        )),
        "home og:image points at the generated card ({rel})"
    );
    assert!(
        !html.contains("og-image.webp"),
        "stale static card is not referenced"
    );
}

/// Draft-aware preview (§A#7): a `draft: true` post is absent from the published view
/// (`Site::discover`) and recorded in `excluded_drafts`, but present + tagged in the
/// preview view (`discover_with(Include)`). Pinned by `posts/draft-example/`.
#[test]
fn tech_blog_draft_is_preview_only() {
    let root = corpus_dir().join("tech-blog");

    let published = Site::discover(&root);
    assert!(
        !published
            .pages
            .iter()
            .any(|p| p.rel.contains("draft-example")),
        "the draft post must be absent from the published set"
    );
    assert!(
        published
            .excluded_drafts
            .iter()
            .any(|d| d.contains("draft-example")),
        "the draft is recorded in excluded_drafts: {:?}",
        published.excluded_drafts
    );

    let preview = Site::discover_with(&root, taliesin_core::DraftMode::Include);
    let d = preview
        .pages
        .iter()
        .find(|p| p.rel.contains("draft-example"))
        .expect("draft present in the preview view");
    assert!(d.draft, "the previewed draft page is tagged");
}
