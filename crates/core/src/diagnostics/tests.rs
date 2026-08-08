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

/// Item 17 F-04: a page inside a site may link a project that site MOUNTS. The mount
/// resolves by URL prefix, so nothing named `gallery/course` exists under the document's own
/// directory and the on-disk rule called it broken — while `check <dir>` on the very same
/// page was clean. The disagreement reached the author through the editor companion, on every
/// keystroke. The exemption must stay exactly as wide as the mount: a near-miss prefix, a
/// sibling that is not a mount, and an ordinary missing file are all still broken.
#[test]
fn local_links_accept_a_link_to_an_enclosing_sites_mount() {
    let dir = Tmp::new("links-mounts");
    // A site whose `_site.yml` mounts two sibling projects, and a `.git` marker so the
    // upward walk stops here rather than climbing into the real repo above the temp dir.
    std::fs::create_dir_all(dir.0.join(".git")).unwrap();
    std::fs::write(
        dir.0.join("_site.yml"),
        "title: S\nmounts:\n  gallery/course: ../elsewhere/course\n  docs/guide: ../elsewhere/guide\n",
    )
    .unwrap();
    let doc = render_document(
        "[mount root](gallery/course/) [deep in a mount](docs/guide/using/formats.html) \
         [typo'd prefix](galery/course/) [not a mount](gallery/nope/) [plain](missing.tmd)\n",
    );
    let m = msgs(&validate_local_links(&doc.blocks, &dir.0));
    assert_eq!(m.len(), 3, "only the three real breaks: {m:?}");
    assert!(m.iter().any(|s| s.contains("`galery/course/`")), "{m:?}");
    assert!(m.iter().any(|s| s.contains("`gallery/nope/`")), "{m:?}");
    assert!(m.iter().any(|s| s.contains("`missing.tmd`")), "{m:?}");
}

/// The same links, with no enclosing site: the exemption must not fire on a bare directory of
/// documents, or it would be a blanket amnesty for any path that happens to have two segments.
#[test]
fn local_links_still_flag_mount_shaped_paths_outside_a_site() {
    let dir = Tmp::new("links-no-site");
    std::fs::create_dir_all(dir.0.join(".git")).unwrap();
    let doc = render_document("[looks mounted](gallery/course/) [also](docs/guide/x.html)\n");
    let m = msgs(&validate_local_links(&doc.blocks, &dir.0));
    assert_eq!(m.len(), 2, "no site, no mounts, no exemption: {m:?}");
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
fn link_text_collision_fires_on_same_words_different_destination() {
    let doc = render_document(
        "See [the docs](one.tmd) and also [the docs](two.tmd).\n\n\
         And [the docs](one.tmd) once more, plus [something else](two.tmd).\n",
    );
    let m = msgs(&validate_link_text_collisions(&doc.blocks));
    assert_eq!(
        m.len(),
        1,
        "one finding per colliding phrase, not one per link: {m:?}"
    );
    assert!(m[0].contains("ambiguous link text `the docs`"), "{m:?}");
}

#[test]
fn link_text_collision_ignores_two_deep_links_into_one_document() {
    // The trim that makes this rule shippable. Compared on the whole href these are two
    // destinations and this fires; compared modulo fragment they are one.
    let doc = render_document(
        "[chapter four](exec.tmd#sec-plan) and [chapter four](exec.tmd#sec-replay), \
         plus [top](#a) and [top](#b).\n",
    );
    assert!(
        validate_link_text_collisions(&doc.blocks).is_empty(),
        "{:?}",
        msgs(&validate_link_text_collisions(&doc.blocks))
    );
}

#[test]
fn link_text_collision_ignores_repetition_to_one_place() {
    let doc = render_document("[the guide](guide.tmd) and again [the guide](guide.tmd).\n");
    assert!(validate_link_text_collisions(&doc.blocks).is_empty());
}

#[test]
fn link_text_collision_exempts_generated_cross_reference_labels() {
    // Two references to two *unnumbered* theorems both render a bare "Theorem": generated
    // text the author cannot reword without abandoning `@`-refs. The exemption has to be
    // checked on hand-built blocks, since a corpus doc cannot produce a collision the
    // numbering scheme prevents. The same pair WITHOUT the xref class must still fire, or
    // this test would pass with the exemption deleted for the wrong reason.
    let xrefs = crate::render::Block {
        id: "b".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: "<p><a href=\"a.html#thm-a\" class=\"tali-xref\">Theorem</a> and \
               <a href=\"b.html#thm-b\" class=\"tali-xref\">Theorem</a></p>"
            .into(),
        cell: None,
        nested: Vec::new(),
    };
    assert!(
        validate_link_text_collisions(std::slice::from_ref(&xrefs)).is_empty(),
        "cross-reference labels are generated, so they are exempt"
    );
    let authored = crate::render::Block {
        html: xrefs.html.replace(" class=\"tali-xref\"", ""),
        ..xrefs
    };
    assert_eq!(
        validate_link_text_collisions(std::slice::from_ref(&authored)).len(),
        1,
        "the same pair written by hand is exactly what the rule exists to catch"
    );
}

#[test]
fn link_text_collision_reads_aria_label_over_visible_text() {
    // `aria-label` is what assistive tech announces, so it is the name that must match —
    // in both directions: distinguishing labels clear a visible-text collision, and
    // colliding labels create one out of distinct visible text.
    let block = |html: &str| crate::render::Block {
        id: "b".into(),
        sourcepos: "1:1-1:1".into(),
        source_file: None,
        html: html.into(),
        cell: None,
        nested: Vec::new(),
    };
    let labelled = block(
        "<p><a href=\"one.html\" aria-label=\"read the intro\">more</a> \
         <a href=\"two.html\" aria-label=\"read the appendix\">more</a></p>",
    );
    assert!(
        validate_link_text_collisions(std::slice::from_ref(&labelled)).is_empty(),
        "distinguishing aria-labels resolve a visible-text collision"
    );
    let collided = block(
        "<p><a href=\"one.html\" aria-label=\"read on\">intro</a> \
         <a href=\"two.html\" aria-label=\"read on\">appendix</a></p>",
    );
    assert_eq!(
        validate_link_text_collisions(std::slice::from_ref(&collided)).len(),
        1,
        "colliding aria-labels are a collision even when the visible text differs"
    );
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
fn a11y_flags_link_with_no_accessible_name() {
    // An icon-only / empty link is flagged; a normal text link, an aria-labelled link,
    // and a link wrapping an alt-bearing image are clean.
    let doc = render_document(
        "Here is [](#) an empty link, a [real link](page.html), \
         and <a href=\"x\" aria-label=\"Home\"></a>, \
         and <a href=\"y\"><img src=\"i.png\" alt=\"icon\"></a>.\n",
    );
    let ws = validate_a11y(&doc.blocks);
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
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(m.is_empty(), "title= names the link: {m:?}");
}

#[test]
fn a11y_flags_button_with_no_name() {
    let doc = render_document("A <button></button> here.\n");
    let m = msgs(&validate_a11y(&doc.blocks));
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
    let ws = validate_a11y(&doc.blocks);
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
    let m = msgs(&validate_a11y(&doc.blocks));
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
    let m = msgs(&validate_a11y(&doc.blocks));
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
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(m.is_empty(), "a clean doc must be silent: {m:?}");
}

#[test]
fn a11y_named_anchor_is_not_a_link() {
    // An `<a id="x">` with no href is a named anchor target, not an interactive link;
    // it must not be flagged for "no accessible name".
    let doc = render_document("Anchor: <a id=\"jump\"></a> here.\n");
    let m = msgs(&validate_a11y(&doc.blocks));
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

#[test]
fn code_language_typo_is_flagged_with_a_location() {
    let doc = render_document("intro\n\n```pyton\nx = 1\n```\n");
    let ws = validate_code_languages(&doc.blocks);
    assert_eq!(ws.len(), 1, "{:?}", msgs(&ws));
    assert!(
        ws[0].message.contains("unknown code language `pyton`"),
        "{}",
        ws[0].message
    );
    assert_eq!(
        ws[0].line,
        Some(3),
        "points at the fence, not the doc start"
    );
}

#[test]
fn real_languages_and_aliases_do_not_warn() {
    let doc = render_document(
        "```python\nx = 1\n```\n\n```rs\nlet x = 1;\n```\n\n\
         ```ts\nconst x: number = 1;\n```\n\n```toml\n[a]\nb = 1\n```\n",
    );
    assert!(
        validate_code_languages(&doc.blocks).is_empty(),
        "{:?}",
        msgs(&validate_code_languages(&doc.blocks))
    );
}

#[test]
fn intentionally_plain_fences_do_not_warn() {
    let doc = render_document("```text\nnot code\n```\n\n```console\n$ ls\n```\n");
    assert!(
        validate_code_languages(&doc.blocks).is_empty(),
        "{:?}",
        msgs(&validate_code_languages(&doc.blocks))
    );
}

#[test]
fn an_unlabelled_fence_does_not_warn() {
    let doc = render_document("```\nplain\n```\n");
    assert!(validate_code_languages(&doc.blocks).is_empty());
}

/// A code block whose *content* mentions the marker must not be mistaken for a
/// fence label: block text is HTML-escaped before it is embedded.
#[test]
fn the_marker_inside_code_content_is_not_a_fence_label() {
    let doc = render_document("```html\n<code class=\"language-pyton\">x</code>\n```\n");
    assert!(
        validate_code_languages(&doc.blocks).is_empty(),
        "{:?}",
        msgs(&validate_code_languages(&doc.blocks))
    );
}

/// `{mermaid}` cells emit a bare `<pre class="mermaid">` with no `<code>`, so they
/// never carry a `language-` class and must not be flagged.
#[test]
fn mermaid_cells_do_not_warn() {
    let doc = render_document("```{mermaid}\ngraph TD;\n  A-->B;\n```\n");
    assert!(
        validate_code_languages(&doc.blocks).is_empty(),
        "{:?}",
        msgs(&validate_code_languages(&doc.blocks))
    );
}

// --- bare `@key` that silently fails to become a citation -------------------
// Regression net for the a-star bug: `corpus/tech-blog/posts/a-star` declared a
// bibliography, wrote a bare `@russell2022artificial` whose key IS in that .bib,
// and shipped the raw key as prose under an empty References heading, with
// `check` reporting "no problems found".

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

fn shape(src: &str) -> Vec<String> {
    msgs(&validate_document_shape(&render_document(src).blocks))
}

#[test]
fn shape_flags_a_heading_with_neither_text_nor_subsections() {
    // Same level: `Alpha` has no prose and no subsection, so the section is empty.
    let m = shape("# Alpha\n\n# Beta\n\ntext\n");
    assert_eq!(m.len(), 1, "only Alpha is hollow: {m:?}");
    assert!(
        m[0].contains("`Alpha`") && m[0].contains("no content under it"),
        "{m:?}"
    );
    // Last heading on the page, nothing after it at all.
    let m = shape("# Alpha\n\ntext\n\n## Trailing\n");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("`Trailing`"), "{m:?}");
}

#[test]
fn shape_does_not_flag_a_grouping_parent_heading() {
    // A heading followed by DEEPER headings has content in the document tree; demanding an
    // intro paragraph there is a style opinion, not a defect. Measured before narrowing:
    // the broad form fired 13 times across the 14 corpus projects and every one was an
    // ordinary grouping parent, so the broad rule was pure noise on real documents.
    let m = shape("# Parent\n\n## Child\n\ntext\n");
    assert!(m.is_empty(), "a grouping parent is legitimate: {m:?}");
}

#[test]
fn shape_does_not_call_a_list_or_code_section_hollow() {
    // The false positive that killed the `skim`-projection draft of this rule: `skim`
    // reads the first `<p>`, and a `<ul>` / fenced block / table is not a `<p>`, so 55
    // real sections (11.8% of the corpus) read as "contentless". Content is any block.
    for body in [
        "- one\n- two\n",
        "```python\nx = 1\n```\n",
        "| a | b |\n| - | - |\n| 1 | 2 |\n",
        "> quoted\n",
    ] {
        let m = shape(&format!("# Alpha\n\n{body}"));
        assert!(
            m.is_empty(),
            "a section whose body is {body:?} has content: {m:?}"
        );
    }
}

#[test]
fn shape_flags_two_headings_that_read_the_same_and_an_empty_one() {
    let m = shape("# Same\n\na\n\n## Same\n\nb\n");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].contains("duplicate heading text") && m[0].contains("`Same`"),
        "{m:?}"
    );

    let m = shape("# \n\ntext\n");
    assert_eq!(m.len(), 1, "an unnamed heading: {m:?}");
    assert!(m[0].contains("empty heading"), "{m:?}");
    // An empty heading is reported once, not also as a duplicate of the next empty one.
    let m = shape("# \n\na\n\n## \n\nb\n");
    assert_eq!(m.len(), 2, "two empties, no duplicate report: {m:?}");
    assert!(m.iter().all(|s| s.contains("empty heading")), "{m:?}");
}

#[test]
fn shape_flags_a_body_title_echo_but_never_the_leading_heading() {
    // The landing-page idiom: front-matter title, then a heading that restates it. Four of
    // the 14 corpus projects do this deliberately, including both dogfood books, so
    // flagging it would fire on house style alone.
    let lead = "---\ntitle: Why Taliesin\n---\n\n## Why Taliesin\n\ntext\n";
    assert!(
        shape(lead).is_empty(),
        "leading echo is the idiom: {:?}",
        shape(lead)
    );

    let body = "---\ntitle: Why Taliesin\n---\n\n## Intro\n\ntext\n\n## Why Taliesin\n\nmore\n";
    let m = shape(body);
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("repeats the page title"), "{m:?}");

    // With no title block there is no title to echo, so the rule is silent (a repeat is
    // already TAL-SHAPE-DUP's business).
    let m = shape("# Why Taliesin\n\na\n\n## Other\n\nb\n");
    assert!(
        !m.iter().any(|s| s.contains("repeats the page title")),
        "{m:?}"
    );
}

#[test]
fn shape_flags_a_numbered_figure_whose_caption_is_only_its_label() {
    let m = shape("# H\n\n![](b.png){#fig-b}\n");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("caption is only its label"), "{m:?}");

    // A real caption, and an unnumbered image with no figcaption at all, are both clean.
    assert!(shape("# H\n\n![A real caption](a.png){#fig-a}\n").is_empty());
    assert!(shape("# H\n\n![](c.png)\n").is_empty());
}

#[test]
fn shape_reads_every_caption_in_a_block_not_just_the_first() {
    // A `:::` div holding two figures is ONE block carrying TWO `<figcaption>`s, so the
    // caption scan has to step past the first closing tag and keep looking. Every other
    // caption test uses a one-figure block, which never runs that loop a second time —
    // and with only those, rewinding the cursor instead of advancing it is invisible
    // (mutation-found: `rest[next + len..]` → `rest[next - len..]` survived the suite).
    let m = shape(
        "# H\n\n::: {.columns}\n![A real caption](a.png){#fig-a}\n\n![](b.png){#fig-b}\n:::\n",
    );
    assert_eq!(
        m.len(),
        1,
        "only the second figure's caption is bare: {m:?}"
    );
    assert!(m[0].contains("caption is only its label"), "{m:?}");
}

#[test]
fn shape_is_silent_on_a_well_formed_document() {
    // The anti-vacuous guard: the rules above must not be firing on everything.
    let m = shape(
        "---\ntitle: A title\n---\n\n## First section\n\nSome prose here.\n\n\
         ## Second section\n\n![A described figure](f.png){#fig-f}\n\n\
         ### A subsection\n\n- a list item\n",
    );
    assert!(m.is_empty(), "well-shaped document must be clean: {m:?}");
}

#[test]
fn a11y_flags_a_label_that_disagrees_with_the_visible_text() {
    // WCAG 2.5.3 Label in Name. A voice-control user says what they can READ, and the
    // browser matches against the accessible NAME, so a control reading "Save draft"
    // while named "Submit" cannot be reached by voice at all.
    let doc = render_document(
        "<button aria-label=\"Submit\">Save draft</button>\n\n\
         <a href=\"/x\" aria-label=\"Search the site\">Search</a>\n\n\
         <button aria-label=\"Close\">Close</button>\n",
    );
    let ws = validate_a11y(&doc.blocks);
    let m = msgs(&ws);
    let flagged: Vec<&String> = m
        .iter()
        .filter(|s| s.contains("disagrees with its visible text"))
        .collect();
    assert_eq!(
        flagged.len(),
        1,
        "only the Submit/Save-draft mismatch: {m:?}"
    );
    assert!(
        flagged[0].contains("Submit"),
        "names the label: {flagged:?}"
    );
    assert!(
        flagged[0].contains("Save draft"),
        "names the visible text: {flagged:?}"
    );
    assert!(ws.iter().any(|w| w.line.is_some()), "located: {ws:?}");
}

#[test]
fn a11y_label_in_name_accepts_a_name_that_only_adds_context() {
    // 2.5.3 is CONTAINMENT, not equality: an accessible name may add words around the
    // visible label ("Search the site" for a control reading "Search"), and case and
    // punctuation are noise — a voice user saying the visible words still matches.
    let doc = render_document(
        "<a href=\"/s\" aria-label=\"Search the site\">Search</a>\n\n\
         <button aria-label=\"next page\">Next Page</button>\n\n\
         <button aria-label=\"Read more about decks\">Read more…</button>\n",
    );
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(
        !m.iter()
            .any(|s| s.contains("disagrees with its visible text")),
        "a name that contains the visible label is correct, not a defect: {m:?}"
    );
}

#[test]
fn a11y_label_in_name_ignores_an_aria_hidden_subtree() {
    // The shape item 124 shipped on the search button, and the reason this rule needs to
    // understand `aria-hidden` at all: a shortcut hint is PAINTED but is deliberately not
    // part of the accessible name, so counting it as the visible label would accuse the
    // sanctioned fix of being the bug.
    let doc = render_document(
        "<button aria-label=\"Search\">\
         <svg aria-hidden=\"true\"><path d=\"M0 0\"/></svg>\
         <kbd aria-hidden=\"true\">\u{2318}K</kbd></button>\n",
    );
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(
        !m.iter()
            .any(|s| s.contains("disagrees with its visible text")),
        "an aria-hidden hint is not the visible label: {m:?}"
    );
    // Control: the SAME markup with the hint exposed IS the 2.5.3 failure — otherwise this
    // test would pass just as well against a rule that never fires.
    let bad = render_document(
        "<button aria-label=\"Search\">\
         <svg aria-hidden=\"true\"><path d=\"M0 0\"/></svg>\
         <kbd>\u{2318}K</kbd></button>\n",
    );
    let bm = msgs(&validate_a11y(&bad.blocks));
    assert!(
        bm.iter()
            .any(|s| s.contains("disagrees with its visible text")),
        "an EXPOSED shortcut hint pollutes the name and must be flagged: {bm:?}"
    );
}

#[test]
fn a11y_label_in_name_is_silent_on_an_icon_only_control() {
    // No visible text at all is what `aria-label` is FOR. 2.5.3 has nothing to say about
    // it, and rule 2 (no accessible name) already owns the label-less case — so this must
    // not double-report the control the other rule is happy with.
    let doc = render_document(
        "<button aria-label=\"Close\"><svg aria-hidden=\"true\"><path d=\"M0 0\"/></svg></button>\n",
    );
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(
        m.is_empty(),
        "an icon-only labelled control is clean: {m:?}"
    );
}

#[test]
fn a11y_label_in_name_survives_a_void_element_inside_the_hidden_subtree() {
    // The hidden-subtree scan must end where the subtree ends. A void element (`<img>`,
    // `<br>`) never closes, so counting it as a nesting level runs the "hidden" state off
    // the end of the element and swallows the visible label — which silently converts this
    // rule into one that reports nothing. That failure is INVISIBLE to the other tests here,
    // because losing the visible text makes the rule skip rather than misfire.
    let doc = render_document(
        "<button aria-label=\"Submit\">\
         <span aria-hidden=\"true\"><img src=\"i.png\" alt=\"\"><br></span>\
         Save draft</button>\n",
    );
    let m = msgs(&validate_a11y(&doc.blocks));
    let hit = m
        .iter()
        .find(|s| s.contains("disagrees with its visible text"));
    assert!(
        hit.is_some(),
        "the visible label must survive a void element in the hidden subtree: {m:?}"
    );
    assert!(
        hit.unwrap().contains("Save draft"),
        "and it must be quoted whole, not truncated at the void element: {hit:?}"
    );
}

#[test]
fn a11y_label_in_name_declines_to_judge_aria_labelledby() {
    // `aria-labelledby` outranks `aria-label` and resolves against ids ELSEWHERE in the
    // document, which this block-local scan cannot see. Guessing would be a false
    // accusation, which costs more here than a missed one.
    let doc = render_document(
        "<button aria-labelledby=\"t\" aria-label=\"Submit\">Save draft</button>\n",
    );
    let m = msgs(&validate_a11y(&doc.blocks));
    assert!(
        !m.iter()
            .any(|s| s.contains("disagrees with its visible text")),
        "an aria-labelledby control is not judged from aria-label: {m:?}"
    );
}
