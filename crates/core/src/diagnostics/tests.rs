use super::*;
use crate::render::{Warning, render_document, render_document_with_includes};
use std::path::Path;

/// A throwaway directory under the system temp dir, removed on drop.
struct Tmp(std::path::PathBuf);
impl Tmp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "tali-diag-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn msgs(ws: &[Warning]) -> Vec<String> {
    ws.iter().map(|w| w.message.clone()).collect()
}

#[test]
fn local_links_flag_missing_relative_target_only() {
    let dir = Tmp::new("links");
    std::fs::write(dir.0.join("exists.tmd"), "x").unwrap();
    let doc = render_document(
        "[gone](missing.tmd) [here](exists.tmd) [ext](https://example.com) \
         [page](sub/page.html#frag) [anchor](#top) [abs](/root.html)\n",
    );
    let ws = validate_local_links(&doc.blocks, &dir.0);
    let m = msgs(&ws);
    assert_eq!(m.len(), 2, "only the two missing local files: {m:?}");
    assert!(m.iter().any(|s| s.contains("`missing.tmd`")), "{m:?}");
    assert!(m.iter().any(|s| s.contains("`sub/page.html`")), "{m:?}");
    // The existing sibling, external, in-page anchor, and absolute links are clean.
    assert!(!m.iter().any(|s| s.contains("exists.tmd")), "{m:?}");
    assert!(!m.iter().any(|s| s.contains("example.com")), "{m:?}");
    assert!(!m.iter().any(|s| s.contains("/root.html")), "{m:?}");
    // Located to a line.
    assert!(ws.iter().all(|w| w.line.is_some()), "located: {ws:?}");
}

#[test]
fn local_links_skip_xref_links() {
    // A `@sec-`/`@fig-` cross-reference renders an `<a … data-tali-xref>`; it is
    // validated by `validate_xrefs`, so the link checker must not double-flag it.
    let doc = render_document("## Sec {#sec-a}\n\nSee @sec-a.\n");
    let ws = validate_local_links(&doc.blocks, Path::new("."));
    assert!(msgs(&ws).is_empty(), "xref link must be skipped: {ws:?}");
}

#[test]
fn local_links_flag_html_link_whose_only_source_is_the_retired_ext() {
    // After the .tmd-only flip, a `.qmd` file on disk is no longer a recognized source:
    // an `.html` link (or directory link) whose only on-disk source is `.qmd` is now
    // flagged broken, same as any other missing target.
    let dir = Tmp::new("links-html-retired-ext-gone");
    std::fs::write(dir.0.join("page.qmd"), "x").unwrap();
    std::fs::create_dir_all(dir.0.join("guide")).unwrap();
    std::fs::write(dir.0.join("guide/index.qmd"), "x").unwrap();
    let doc =
        render_document("[built page](page.html) [dir link](guide/) [really gone](ghost.html)\n");
    let m = msgs(&validate_local_links(&doc.blocks, &dir.0));
    assert_eq!(
        m.len(),
        3,
        "page.html, guide/, and ghost.html are all now broken (.qmd is not a source): {m:?}"
    );
    assert!(m.iter().any(|s| s.contains("`page.html`")), "{m:?}");
    assert!(m.iter().any(|s| s.contains("`guide/`")), "{m:?}");
    assert!(m.iter().any(|s| s.contains("`ghost.html`")), "{m:?}");
}

#[test]
fn local_links_accept_html_link_with_tmd_source() {
    // Unlike `local_links_flag_html_link_whose_only_source_is_the_retired_ext`, the on-disk source
    // here is spelled `.tmd` (Taliesin's native and only source extension), so the
    // probe finds it and the `.html` link resolves clean.
    let dir = Tmp::new("links-html-tmd");
    std::fs::write(dir.0.join("page.tmd"), "x").unwrap();
    std::fs::create_dir_all(dir.0.join("guide")).unwrap();
    std::fs::write(dir.0.join("guide/index.tmd"), "x").unwrap();
    let doc =
        render_document("[built page](page.html) [dir link](guide/) [really gone](ghost.html)\n");
    let m = msgs(&validate_local_links(&doc.blocks, &dir.0));
    assert_eq!(m.len(), 1, "only the truly missing target: {m:?}");
    assert!(m[0].contains("`ghost.html`"), "{m:?}");
}

#[test]
fn local_media_flags_missing_video() {
    let dir = Tmp::new("video");
    std::fs::write(dir.0.join("there.mp4"), "x").unwrap();
    let doc = render_document_with_includes(
        "<video src=\"gone.mp4\"></video>\n\n<video src=\"there.mp4\"></video>\n\n\
         <video src=\"https://cdn.example/clip.mp4\"></video>\n",
        &dir.0,
    );
    let ws = validate_local_media(&doc.blocks, &dir.0);
    let m = msgs(&ws);
    assert_eq!(m.len(), 1, "only the missing local clip: {m:?}");
    assert!(m[0].contains("local video not found"), "{m:?}");
    assert!(m[0].contains("`gone.mp4`"), "{m:?}");
    assert!(ws[0].line.is_some(), "located: {ws:?}");
}

#[test]
fn media_poster_and_extra_sources_are_checked() {
    let dir = Tmp::new("video-dark");
    std::fs::write(dir.0.join("light.mp4"), "x").unwrap();
    // A `<source>` inside the element and the `poster=` still are references: both are
    // paths a build must be able to copy, and both used to ship broken in silence.
    let doc = render_document_with_includes(
        "<video poster=\"cover.png\"><source src=\"light.mp4\"><source src=\"dark.mp4\"></video>\n",
        &dir.0,
    );
    let m = msgs(&validate_local_media(&doc.blocks, &dir.0));
    assert!(m.iter().any(|s| s.contains("`dark.mp4`")), "{m:?}");
    assert!(m.iter().any(|s| s.contains("`cover.png`")), "{m:?}");
    assert!(!m.iter().any(|s| s.contains("light.mp4")), "{m:?}");
}

#[test]
fn audio_source_is_not_a_video_false_positive() {
    // An `<audio><source>` is NOT a video; its (often streamed/generated) source must
    // not be flagged. A real `<video><source>` next to it IS checked.
    let doc = render_document(
        "<audio controls><source src=\"tone.wav\" type=\"audio/wav\"></audio>\n\n\
         <video><source src=\"clip.mp4\" type=\"video/mp4\"></video>\n",
    );
    let m = msgs(&validate_local_media(&doc.blocks, Path::new(".")));
    assert!(
        !m.iter().any(|s| s.contains("tone.wav")),
        "audio src skipped: {m:?}"
    );
    assert!(
        m.iter().any(|s| s.contains("clip.mp4")),
        "video <source> checked: {m:?}"
    );
}

#[test]
fn js_reactive_graph_flags_dangling_input() {
    let doc = render_document(
        "```{js}\n//| viewof: n\nreturn html`<input type=range>`;\n```\n\n\
         ```{js}\n//| input: n, missing\nreturn n;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert_eq!(
        m.len(),
        1,
        "only `missing` is dangling (`n` is defined): {m:?}"
    );
    assert!(m[0].contains("unknown reactive input `missing`"), "{m:?}");
}

#[test]
fn js_reactive_graph_did_you_mean_over_defines() {
    let doc = render_document(
        "```{js}\n//| name: count\nreturn 1;\n```\n\n\
         ```{js}\n//| input: cont\nreturn cont;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("did you mean `count`?"), "{m:?}");
}

#[test]
fn js_reactive_graph_input_shortcode_define_clears_dangling() {
    // A declarative `{{< input name="k" >}}` defines `k`, so a cell consuming it is clean.
    let dir = Tmp::new("js-input");
    let doc = render_document_with_includes(
        "{{< input name=\"k\" type=\"slider\" min=\"0\" max=\"10\" >}}\n\n\
         ```{js}\n//| input: k\nreturn k;\n```\n",
        &dir.0,
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(m.is_empty(), "shortcode-defined input must resolve: {m:?}");
}

#[test]
fn js_reactive_graph_detects_cycle() {
    let doc = render_document(
        "```{js}\n//| name: a\n//| input: b\nreturn b;\n```\n\n\
         ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    // Both cells are undrained -> one cycle warning each; `a` and `b` are mutually defined.
    assert_eq!(m.len(), 2, "both cycle members flagged: {m:?}");
    assert!(
        m.iter().all(|s| s.contains("reactive dependency cycle")),
        "{m:?}"
    );
}

/// The suppressing half of the pair. A Python cell that CALLS `define(` really can publish
/// `runtime_name` at runtime through a blob no static pass can enumerate, so the
/// dangling-input check must stay quiet.
#[test]
fn js_dangling_input_suppressed_when_a_python_cell_calls_define() {
    let doc = render_document(
        "```{python}\ndefine(runtime_name=5)\n```\n\n\
         ```{js}\n//| input: runtime_name\nreturn runtime_name;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(m.is_empty(), "dangling-input must be suppressed: {m:?}");
}

/// The other half, and the one the narrowing bought. A Python cell that does NOT call
/// `define(` publishes nothing into the reactive graph, so the broken reference below is
/// reported — where until 2026-08-03 merely *having* a `{python}` cell switched the check
/// off, which is every real blog post in the corpus.
#[test]
fn js_dangling_input_is_reported_when_the_python_cell_defines_nothing() {
    let doc = render_document(
        "```{python}\nx = 5\n```\n\n\
         ```{js}\n//| input: runtime_name\nreturn runtime_name;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(
        m.iter().any(|s| s.contains("`runtime_name`")),
        "a kernel cell that defines nothing must not suppress the check: {m:?}"
    );
}

#[test]
fn js_cycle_still_flagged_with_python_cell_present() {
    // The cycle half is a structural fact among js cells; it survives a python cell.
    let doc = render_document(
        "```{python}\nx = 1\n```\n\n\
         ```{js}\n//| name: a\n//| input: b\nreturn b;\n```\n\n\
         ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(
        m.iter().filter(|s| s.contains("cycle")).count() == 2,
        "cycle still flagged: {m:?}"
    );
}

#[test]
fn js_reactive_graph_clean_chain_is_silent() {
    // n -> squared -> consumer, no cycle, every input defined.
    let doc = render_document(
        "```{js}\n//| viewof: n\nreturn html`<input type=range>`;\n```\n\n\
         ```{js}\n//| name: squared\n//| input: n\nreturn n*n;\n```\n\n\
         ```{js}\n//| input: squared\nreturn squared;\n```\n",
    );
    assert!(
        validate_js_reactive_graph(&doc.blocks).is_empty(),
        "a clean reactive chain must be silent"
    );
}

#[test]
fn a11y_flags_heading_level_skip_mid_document() {
    // h2 -> h4 skips h3; flagged, located. The leading h2 (no prior heading) is fine,
    // and "doesn't start at h1" is never flagged.
    let doc = render_document("## Top\n\nbody\n\n#### Deep\n\nmore\n");
    let ws = validate_a11y(&doc.blocks);
    let m = msgs(&ws);
    assert_eq!(m.len(), 1, "only the h2->h4 skip: {m:?}");
    assert!(m[0].contains("heading level skips from h2 to h4"), "{m:?}");
    assert!(ws[0].line.is_some(), "located: {ws:?}");
}

#[test]
fn a11y_one_level_deeper_is_fine() {
    // h2 -> h3 is a single step, not a skip; never flagged.
    let doc = render_document("## A\n\n### B\n\n#### C\n");
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(m.is_empty(), "single-level steps must be silent: {m:?}");
}

#[test]
fn a11y_does_not_flag_first_heading_below_h1() {
    // A doc whose first heading is an h2 (a common pattern; the title is the h1) must
    // NOT be flagged — only a mid-document skip counts.
    let doc = render_document("## Section\n\nbody\n\n## Another\n");
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(m.is_empty(), "first-heading-below-h1 is not a skip: {m:?}");
}

#[test]
fn a11y_flags_raw_img_without_alt() {
    // A hand-written `<img>` with no `alt` is flagged; an `<img alt="">` (decorative)
    // and a markdown image (which always emits an alt) are clean.
    let doc = render_document(
        "<img src=\"logo.png\">\n\n<img src=\"ok.png\" alt=\"described\">\n\n\
         <img src=\"deco.png\" alt=\"\">\n\n![real alt](pic.png)\n",
    );
    let ws = validate_a11y(&doc.blocks);
    let m = msgs(&ws);
    assert_eq!(m.len(), 1, "only the alt-less raw img: {m:?}");
    assert!(m[0].contains("image is missing alt text"), "{m:?}");
    assert!(ws[0].line.is_some(), "located: {ws:?}");
}

#[test]
fn a11y_flags_placeholder_alt_but_not_descriptive() {
    // A non-empty but useless alt (a bare medium word, or an echo of the filename) is
    // flagged; a descriptive alt and alt="" (decorative) are clean. The common LLM tell.
    let doc = render_document(
        "![image](photo.png)\n\n\
         <img src=\"scree.png\" alt=\"scree.png\">\n\n\
         ![A scree plot of the eigenvalues](scree.png)\n\n\
         <img src=\"deco.png\" alt=\"\">\n",
    );
    let ws = validate_a11y(&doc.blocks);
    let m = msgs(&ws);
    assert_eq!(
        m.iter()
            .filter(|s| s.contains("looks like a placeholder"))
            .count(),
        2,
        "medium-word + filename-echo alts flagged, descriptive + decorative clean: {m:?}"
    );
    assert!(
        !m.iter().any(|s| s.contains("scree plot")),
        "a descriptive alt is never accused: {m:?}"
    );
}

#[test]
fn a11y_clean_document_is_silent() {
    // Markdown headings stepping by one, a markdown image (auto-alt), and a text link:
    // no a11y warnings at all.
    let doc = render_document(
        "# Title\n\n## Section\n\n### Subsection\n\n\
         ![a described picture](pic.png)\n\nA [normal link](page.html).\n",
    );
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(m.is_empty(), "a clean doc must be silent: {m:?}");
}

const BIB: &str = "@book{russell2022artificial,\n  title = {AIMA},\n  author = {Russell, S.},\n  year = {2022}\n}\n\n@article{smith2020,\n  title = {T},\n  author = {Smith, A.},\n  year = {2020}\n}\n";

fn bare_cite_warnings(dir: &Tmp, body: &str) -> Vec<Warning> {
    std::fs::write(dir.0.join("refs.bib"), BIB).unwrap();
    let src = format!("---\ntitle: T\nbibliography: refs.bib\n---\n\n{body}");
    let doc = render_document_with_includes(&src, &dir.0);
    bare_citation_key_not_rendered(&src, &doc.blocks, &dir.0)
}

#[test]
fn bare_key_matching_the_bibliography_is_flagged() {
    let dir = Tmp::new("bare-cite");
    let ws = bare_cite_warnings(&dir, "Please refer to @russell2022artificial.\n");
    let m = msgs(&ws);
    assert_eq!(m.len(), 1, "the dangling bare key: {m:?}");
    assert!(
        m[0].contains("@russell2022artificial"),
        "names the key: {m:?}"
    );
    assert!(
        m[0].contains("[@russell2022artificial]"),
        "suggests the bracketed form (did-you-mean): {m:?}"
    );
    assert!(
        ws[0].line.is_some(),
        "must be located for click-to-source: {:?}",
        ws[0]
    );
}

#[test]
fn bracketed_citation_is_clean() {
    let dir = Tmp::new("bare-cite-ok");
    let ws = bare_cite_warnings(&dir, "As shown [@russell2022artificial].\n");
    assert!(ws.is_empty(), "a real citation must not trip this: {ws:?}");
}

#[test]
fn bare_at_word_outside_the_bibliography_is_clean() {
    // The greedy-match hazard: `is_cite_key_char` admits `/ . : +`, so an unguarded
    // scan eats `@media`, `@types/node` and e-mail. Membership gating is what
    // makes this rule safe, so pin it.
    let dir = Tmp::new("bare-cite-noise");
    let ws = bare_cite_warnings(
        &dir,
        "Use @media queries, install @types/node, mail bob@russell2022artificial.com \
         or ping @russell2022artificialXYZ today.\n",
    );
    assert!(ws.is_empty(), "must not fire on non-bib `@word`s: {ws:?}");
}

#[test]
fn bare_key_in_a_code_block_is_clean() {
    let dir = Tmp::new("bare-cite-code");
    let ws = bare_cite_warnings(&dir, "```\n@russell2022artificial\n```\n");
    assert!(ws.is_empty(), "code is not prose: {ws:?}");
}

#[test]
fn no_bibliography_declared_means_no_scan() {
    let dir = Tmp::new("bare-cite-nobib");
    let src = "---\ntitle: T\n---\n\nPlease refer to @russell2022artificial.\n";
    let doc = render_document_with_includes(src, &dir.0);
    let ws = bare_citation_key_not_rendered(src, &doc.blocks, &dir.0);
    assert!(ws.is_empty(), "no bibliography, nothing to match: {ws:?}");
}

// The `csl:` recognized-but-unsupported tests moved to `frontmatter::tests` with the rule
// itself, which now runs on the render path so the preview is not silent. This module is
// check-only, so testing it here would have pinned the wrong surface.

// ---- document-shape lints (item 24c) -------------------------------------------------
//
// Every threshold-bearing candidate was cut after measuring it against the corpus with
// `taliesin skim`; what survives is binary and threshold-free. The rules were calibrated
// on the real 14-project corpus, and the cases below pin both what they catch and, just
// as importantly, what they must not.
