//! Progress tracking toward replacing Quarto for *iterating* on the author's
//! tech-blog (https://andreasbogossian.com). Three corpus posts come straight
//! from it: `em-algorithm`, `pca-geometry`, and `fourier-transform`.
//!
//! Two kinds of tests live here:
//!
//!  * **Passing tests** lock in the per-post feature surface that already works,
//!    i.e. the edit-preview loop you'd actually iterate in: math, callouts,
//!    citations, OJS-as-source, and leak-free whole-doc rendering.
//!  * **`#[ignore]`d tests** encode the *target*: each is a known gap between
//!    qmd-fast and Quarto for this blog. They fail today; flip off `#[ignore]`
//!    when the gap is closed. Run them with `cargo test -- --ignored`.
//!
//! Keeping both in one file makes "how close are we?" answerable with
//! `cargo test -p qmd-fast-core --test tech_blog -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};

use qmd_fast_core::{render_document, render_document_with_includes, render_html_page};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

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
    for marker in ["{{<", ":::", "\n#|", "//|", "{#eq-", "{#sec-", "language-=html"] {
        assert!(
            !html.contains(marker),
            "{label}: source marker {marker:?} leaked into rendered output"
        );
    }
}

// ---------------------------------------------------------------------------
// Passing: the per-post feature surface you iterate on already works.
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
        assert!(html.len() > 2_000, "{post}: suspiciously small output ({} bytes)", html.len());
        assert_no_source_leaks(post, &html);
    }
}

/// `em-algorithm` exercises the heavy-math feature set: inline `$...$`, display
/// `$$...$$`, and `\begin{align*}` blocks all reach KaTeX.
#[test]
fn math_renders_inline_display_and_align() {
    let html = render_post("posts/em-algorithm/index.qmd");
    assert!(html.contains("class=\"katex\""), "no inline KaTeX spans rendered");
    assert!(html.contains("katex-display"), "no display-math (`$$`/align) rendered");
    // sanity: this post is math-dense, so there should be many spans
    assert!(html.matches("class=\"katex\"").count() > 20, "far fewer KaTeX spans than expected");
}

/// Callouts (`::: {.callout-note ...}`) render with a title and body, and the
/// fenced-div markers do not leak.
#[test]
fn callout_renders_with_title_and_body() {
    let html = render_post("posts/em-algorithm/index.qmd");
    assert!(html.contains("callout-note"), "callout-note class missing");
    assert!(html.contains("class=\"callout-title\""), "callout title missing");
    assert!(html.contains("Notation used in this post"), "callout title text missing");
    assert!(html.contains("class=\"callout-body\""), "callout body missing");
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
    assert!(html.contains("<h2>References</h2>"), "References section not generated");
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
    assert!(html.contains("callout-collapse"), "collapsible callout class missing");
    assert!(
        html.contains("<details><summary class=\"callout-title\">Notation used in this post</summary>"),
        "collapsible callout is not a <details>/<summary>"
    );
}

/// A code cell with `#| code-fold: true` wraps its listing in a `<details>`,
/// using `code-summary` as the disclosure label.
#[test]
fn code_fold_wraps_listing_in_details() {
    let html = render_post("posts/pca-geometry/index.qmd");
    assert!(html.contains("class=\"qmd-code-fold\""), "code-fold did not produce a <details>");
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
    assert!(html.contains("class=\"qmd-code-fold\""), "code-fold did not produce a <details>");
    assert!(html.contains("<summary>Code</summary>"), "default code-fold label missing");
}

/// OJS cells render as live Observable placeholders (executed client-side by the
/// vendored runtime), not as static highlighted source. Each cell emits an output
/// target div plus a base64 `ojs-module-contents` script, and cell-option lines
/// are stripped. (Was a gap; OJS now executes — see also the page-level head.)
#[test]
fn ojs_cells_render_as_live_placeholders() {
    let html = render_post("posts/fourier-transform/index.qmd");
    assert!(html.contains("class=\"ojs-cell\""), "OJS cell not emitted as a live placeholder");
    assert!(
        html.contains("<script type=\"ojs-module-contents\">"),
        "OJS cell missing its module-contents script"
    );
    assert!(!html.contains("class=\"language-ojs\""), "OJS still rendered as a static listing");
    assert!(!html.contains("//| echo"), "OJS cell-option line leaked into output");
}

/// The full page ships the Observable runtime + init only when the doc has OJS.
#[test]
fn ojs_page_ships_runtime_when_cells_present() {
    let dir = corpus_dir().join("posts/fourier-transform");
    let src = std::fs::read_to_string(dir.join("index.qmd")).unwrap();
    let page = qmd_fast_core::render_html_page_with_includes(&src, &dir, "post");
    assert!(page.contains("window._ojs"), "OJS runtime bundle not shipped on a page with cells");
    assert!(page.contains("qmdRunOJS"), "OJS init script missing");

    // A doc with no OJS must not pay for the (large) runtime.
    let prose = qmd_fast_core::render_html_page("---\ntitle: x\n---\n\nJust prose, no cells.\n", "p");
    assert!(!prose.contains("window._ojs"), "OJS runtime shipped on a doc with no cells");
}

// ---------------------------------------------------------------------------
// Gaps (#[ignore]): the target for "fully replace Quarto for this blog".
// Each fails today; remove `#[ignore]` once the feature lands.
// Run with: cargo test -p qmd-fast-core --test tech_blog -- --ignored
// ---------------------------------------------------------------------------

/// ```` ```{=html} ```` is Pandoc/Quarto raw-passthrough: its body is emitted
/// verbatim, not escaped as a code listing. The audio players in
/// `fourier-transform` are authored this way. (Fixed; was a gap.)
#[test]
fn raw_html_block_is_passed_through() {
    let src = "```{=html}\n<audio controls><source src=\"x.wav\" type=\"audio/wav\"></audio>\n```\n";
    let html = render_document(src).body_html();
    assert!(html.contains("<audio controls"), "raw <audio> HTML was not passed through");
    assert!(!html.contains("&lt;audio"), "raw HTML was escaped");
    assert!(!html.contains("language-=html"), "raw block was treated as a code cell");
}

/// The raw-HTML audio players in `fourier-transform` reach the output as live
/// `<audio>`/`<source>` elements (end-to-end, through the real post).
#[test]
fn fourier_audio_players_render_live() {
    let html = render_post("posts/fourier-transform/index.qmd");
    assert!(html.contains("<audio controls"), "audio players not passed through");
    assert!(html.contains("src=\"chord.wav\""), "audio source reference missing");
    assert!(!html.contains("&lt;audio"), "audio markup was escaped");
}

/// A display equation labelled `$$ ... $$ {#eq-foo}` consumes the attribute,
/// emits a matching `id`, and gets a number. (Fixed; was a gap.)
#[test]
fn display_equation_label_becomes_numbered_id() {
    let src = "$$\nX = 1\n$$ {#eq-foo}\n";
    let html = render_document(src).body_html();
    assert!(html.contains("id=\"eq-foo\""), "equation did not get its #eq-foo id");
    assert!(!html.contains("{#eq-foo}"), "the {{#eq-foo}} attribute leaked as text");
    assert!(html.contains("qmd-eqn-number"), "equation was not numbered");
}

/// End-to-end in `fourier-transform`: the labelled equation gets `id="eq-dft"`
/// and the `@eq-dft` cross-reference resolves to a numbered "Equation N" link.
#[test]
fn equation_crossref_resolves_to_number() {
    let html = render_post("posts/fourier-transform/index.qmd");
    assert!(html.contains("id=\"eq-dft\""), "labelled equation id missing");
    assert!(!html.contains("{#eq-dft}"), "equation label leaked as text");
    assert!(
        html.contains("<a href=\"#eq-dft\" class=\"qmd-xref\">Equation&nbsp;1</a>"),
        "@eq-dft did not resolve to a numbered Equation link"
    );
}

/// GAP (site generator): a `listing:` page (the blog index, projects index, and
/// the homepage's "recent posts") should crawl its `contents:` directory and
/// emit post cards. Today the frontmatter is ignored and only the page's prose
/// renders. This is out of qmd-fast's current single-doc scope; the test marks
/// the boundary of "iterate on a post" vs "build the whole site".
#[test]
#[ignore = "gap: listing: frontmatter is ignored — no site-level post-card generation (single-doc scope)"]
fn listing_frontmatter_emits_post_cards() {
    let src = "---\ntitle: \"Blog\"\nlisting:\n  contents: posts\n  type: grid\n---\n\nIntro paragraph.\n";
    let html = render_html_page(src, "Blog");
    assert!(
        html.contains("quarto-listing") || html.contains("class=\"card") || html.contains("listing"),
        "listing produced no post-card markup"
    );
}

/// GAP (site generator): an `about:` page (the homepage uses `template: jolla`)
/// should render a profile block from the frontmatter. Today the `about:` key is
/// ignored and only the body prose renders.
#[test]
#[ignore = "gap: about: frontmatter is ignored — no profile/about block rendered (single-doc scope)"]
fn about_page_renders_profile_block() {
    let src = "---\ntitle: \"Andreas Bogossian\"\nabout:\n  template: jolla\n  image: profile.webp\n---\n\nMSc student.\n";
    let html = render_html_page(src, "About");
    assert!(
        html.contains("about-") || html.contains("quarto-about"),
        "about: frontmatter produced no about/profile block"
    );
}

/// Every `{ojs}` cell across the OJS-heavy posts becomes a live placeholder with
/// a matching `id == cellName`, so the runtime can interpret each into its target.
#[test]
fn every_ojs_cell_has_matching_target_and_script() {
    for post in [
        "posts/fourier-transform/index.qmd",
        "posts/em-algorithm/index.qmd",
        "posts/pca-geometry/index.qmd",
    ] {
        let html = render_post(post);
        let cells = html.matches("class=\"ojs-cell\"").count();
        let scripts = html.matches("<script type=\"ojs-module-contents\">").count();
        assert!(cells > 0, "{post}: no live OJS cells emitted");
        assert_eq!(cells, scripts, "{post}: cell/script count mismatch");
        // every target div id appears as a cellName in a module-contents script
        for id in ojs_target_ids(&html) {
            assert!(html.contains(&format!("ojs-cell-{id}")), "{post}: {id} has no target div");
        }
    }
}

/// Cross-references to computed outputs resolve to numbers, and the
/// client-rendered targets (OJS figures, code listings) carry their anchors at
/// render time. (Python figure outputs are wrapped by the executor; see serve.)
#[test]
fn computed_output_crossrefs_resolve() {
    // fourier: a matplotlib figure (fig-components) gets a number; the OJS winding
    // figure is a real <figure> anchor.
    let f = render_post("posts/fourier-transform/index.qmd");
    assert!(
        f.contains("<a href=\"#fig-components\" class=\"qmd-xref\">Figure&nbsp;1</a>"),
        "@fig-components did not resolve to a numbered link"
    );
    assert!(f.contains("id=\"fig-winding\""), "labelled OJS figure anchor missing");

    // pca: an OJS figure (fig-3d-pca) and a code listing (lst-data-generation)
    // resolve to numbered, anchored targets at render time.
    let p = render_post("posts/pca-geometry/index.qmd");
    assert!(p.contains("id=\"fig-3d-pca\""), "OJS figure anchor missing");
    assert!(
        p.contains("<a href=\"#fig-3d-pca\" class=\"qmd-xref\">Figure&nbsp;"),
        "@fig-3d-pca did not resolve to a numbered link"
    );
    assert!(p.contains("class=\"qmd-listing\"") && p.contains("id=\"lst-data-generation\""),
        "code listing anchor missing");
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

/// Pull the block ids out of `id="ojs-cell-<id>"` target divs.
fn ojs_target_ids(html: &str) -> Vec<String> {
    html.match_indices("id=\"ojs-cell-")
        .filter_map(|(i, _)| {
            let rest = &html[i + "id=\"ojs-cell-".len()..];
            rest.split('"').next().map(str::to_string)
        })
        .collect()
}
