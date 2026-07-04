use super::*;
use crate::render::{DocFormat, Warning, render_document, render_document_with_includes};
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
    // A `@sec-`/`@fig-` cross-reference renders an `<a … data-qmd-xref>`; it is
    // validated by `validate_xrefs`, so the link checker must not double-flag it.
    let doc = render_document("## Sec {#sec-a}\n\nSee @sec-a.\n");
    let ws = validate_local_links(&doc.blocks, Path::new("."));
    assert!(msgs(&ws).is_empty(), "xref link must be skipped: {ws:?}");
}

#[test]
fn local_links_accept_html_link_with_qmd_source() {
    // A `.html` link whose `.qmd` source exists on disk (an intra-project page link the
    // site build will emit) must NOT be flagged; only a target with no file *and* no
    // source is broken. Mirrors the docs/ cross-book links checked standalone.
    let dir = Tmp::new("links-html");
    std::fs::write(dir.0.join("page.qmd"), "x").unwrap();
    std::fs::create_dir_all(dir.0.join("guide")).unwrap();
    std::fs::write(dir.0.join("guide/index.qmd"), "x").unwrap();
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
        "{{< video gone.mp4 >}}\n\n{{< video there.mp4 >}}\n\n\
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
fn media_dark_and_poster_sources_checked() {
    let dir = Tmp::new("video-dark");
    std::fs::write(dir.0.join("light.mp4"), "x").unwrap();
    // dark= source missing; poster missing.
    let doc = render_document_with_includes(
        "{{< video light.mp4 dark=dark.mp4 poster=cover.png >}}\n",
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

#[test]
fn js_dangling_input_suppressed_when_python_cell_present() {
    // A Python `ojs_define` can publish `runtime_name` at runtime, which a static pass
    // can't see; so the presence of any non-js cell suppresses the dangling-input half.
    let doc = render_document(
        "```{python}\nojs_define(runtime_name=5)\n```\n\n\
         ```{js}\n//| input: runtime_name\nreturn runtime_name;\n```\n",
    );
    let m = msgs(&validate_js_reactive_graph(&doc.blocks));
    assert!(m.is_empty(), "dangling-input must be suppressed: {m:?}");
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
    let ws = validate_a11y(&doc.blocks, DocFormat::Html);
    let m = msgs(&ws);
    assert_eq!(m.len(), 1, "only the h2->h4 skip: {m:?}");
    assert!(m[0].contains("heading level skips from h2 to h4"), "{m:?}");
    assert!(ws[0].line.is_some(), "located: {ws:?}");
}

#[test]
fn a11y_one_level_deeper_is_fine() {
    // h2 -> h3 is a single step, not a skip; never flagged.
    let doc = render_document("## A\n\n### B\n\n#### C\n");
    let m = msgs(&validate_a11y(&doc.blocks, DocFormat::Html));
    assert!(m.is_empty(), "single-level steps must be silent: {m:?}");
}

#[test]
fn a11y_heading_skip_skipped_for_decks() {
    // A deck's per-slide `## … ####` is slide structure, not a single outline; the
    // heading-skip rule must not fire when the format is a reveal deck.
    let doc = render_document("## Top\n\n#### Deep\n");
    let m = msgs(&validate_a11y(&doc.blocks, DocFormat::Reveal));
    assert!(m.is_empty(), "decks skip the heading-skip rule: {m:?}");
}

#[test]
fn a11y_does_not_flag_first_heading_below_h1() {
    // A doc whose first heading is an h2 (a common pattern; the title is the h1) must
    // NOT be flagged — only a mid-document skip counts.
    let doc = render_document("## Section\n\nbody\n\n## Another\n");
    let m = msgs(&validate_a11y(&doc.blocks, DocFormat::Html));
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
    let ws = validate_a11y(&doc.blocks, DocFormat::Html);
    let m = msgs(&ws);
    assert_eq!(m.len(), 1, "only the alt-less raw img: {m:?}");
    assert!(m[0].contains("image is missing alt text"), "{m:?}");
    assert!(ws[0].line.is_some(), "located: {ws:?}");
}

#[test]
fn a11y_flags_link_with_no_accessible_name() {
    // An icon-only / empty link is flagged; a normal text link, an aria-labelled link,
    // and a link wrapping an alt-bearing image are clean.
    let doc = render_document(
        "Here is [](#) an empty link, a [real link](page.html), \
         and <a href=\"x\" aria-label=\"Home\"></a>, \
         and <a href=\"y\"><img src=\"i.png\" alt=\"icon\"></a>.\n",
    );
    let ws = validate_a11y(&doc.blocks, DocFormat::Html);
    let m = msgs(&ws);
    assert_eq!(m.len(), 1, "only the empty link: {m:?}");
    assert!(
        m[0].contains("link has no accessible name"),
        "wrong message: {m:?}"
    );
    assert!(ws[0].line.is_some(), "located: {ws:?}");
}

#[test]
fn a11y_title_attr_names_a_link() {
    // A `title=` (tooltip) is an accessible name, same as `scanA11y`.
    let doc = render_document("A <a href=\"x\" title=\"Home\"></a> link.\n");
    let m = msgs(&validate_a11y(&doc.blocks, DocFormat::Html));
    assert!(m.is_empty(), "title= names the link: {m:?}");
}

#[test]
fn a11y_flags_button_with_no_name() {
    let doc = render_document("A <button></button> here.\n");
    let m = msgs(&validate_a11y(&doc.blocks, DocFormat::Html));
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("button has no accessible name"), "{m:?}");
}

#[test]
fn a11y_flags_role_button_with_no_name() {
    // A `<div role="button">` with no text/label is a control to assistive tech but
    // invisible to the literal `<a>`/`<button>` scan: it must be flagged. One carrying
    // an aria-label, and one with visible text, are clean.
    let doc = render_document(
        "<div role=\"button\"></div>\n\n\
         <div role=\"button\" aria-label=\"Close\"></div>\n\n\
         <div role=\"button\">Submit</div>\n",
    );
    let ws = validate_a11y(&doc.blocks, DocFormat::Html);
    let m = msgs(&ws);
    assert_eq!(m.len(), 1, "only the unnamed role=button div: {m:?}");
    assert!(
        m[0].contains("button has no accessible name"),
        "wrong message: {m:?}"
    );
}

#[test]
fn a11y_flags_role_link_and_role_tab_without_name() {
    // `role="link"` and `role="tab"` on a non-native element are audited too.
    let doc =
        render_document("A <span role=\"link\"></span> and a <span role=\"tab\"></span> here.\n");
    let m = msgs(&validate_a11y(&doc.blocks, DocFormat::Html));
    assert_eq!(m.len(), 2, "both the unnamed role link + tab: {m:?}");
    assert!(
        m.iter().any(|s| s.contains("link has no accessible name")),
        "{m:?}"
    );
    assert!(
        m.iter().any(|s| s.contains("tab has no accessible name")),
        "{m:?}"
    );
}

#[test]
fn a11y_native_button_with_role_tab_is_not_flagged_twice() {
    // The panel-tabset emits `<button role="tab">Label</button>`: a named native button.
    // It must produce ZERO findings — the role scan skips native `<a>`/`<button>`, and
    // the label gives it an accessible name anyway (no double-count even if unnamed).
    let doc =
        render_document("A <button role=\"tab\" aria-selected=\"true\">Overview</button> tab.\n");
    let m = msgs(&validate_a11y(&doc.blocks, DocFormat::Html));
    assert!(
        m.is_empty(),
        "a labelled role=tab button must be silent: {m:?}"
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
    let m = msgs(&validate_a11y(&doc.blocks, DocFormat::Html));
    assert!(m.is_empty(), "a clean doc must be silent: {m:?}");
}

#[test]
fn a11y_named_anchor_is_not_a_link() {
    // An `<a id="x">` with no href is a named anchor target, not an interactive link;
    // it must not be flagged for "no accessible name".
    let doc = render_document("Anchor: <a id=\"jump\"></a> here.\n");
    let m = msgs(&validate_a11y(&doc.blocks, DocFormat::Html));
    assert!(m.is_empty(), "named anchor (no href) is not a link: {m:?}");
}

#[test]
fn validate_math_flags_only_unparseable_katex_located() {
    // A malformed inline expression renders a red `katex-error` span (KaTeX runs with
    // throw_on_error off), which used to ship with NO diagnostic; validate_math
    // re-surfaces it on the located channel.
    let doc = render_document("Intro.\n\nBad math $\\frac{$ here.\n");
    let ws = validate_math(&doc.blocks);
    assert_eq!(ws.len(), 1, "one broken expression: {:?}", msgs(&ws));
    assert!(
        ws[0].message.contains("math failed to render"),
        "{:?}",
        ws[0].message
    );
    assert_eq!(
        ws[0].line,
        Some(3),
        "located at the math's source line: {:?}",
        ws[0]
    );

    // Valid math produces no diagnostic.
    let ok = render_document("Good $x^2 + y^2$ math.\n");
    assert!(
        validate_math(&ok.blocks).is_empty(),
        "valid math must be silent: {:?}",
        msgs(&validate_math(&ok.blocks))
    );
}
