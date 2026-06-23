//! `_extensions/` format-extension resolution: a deck that selects a slide theme
//! extension via `format: <ext>-revealjs` gets the extension's contributed theme +
//! includes injected (the mechanism behind liquid-glass-revealjs).

use std::fs;
use std::path::Path;

mod common;
use common::TempProj;

fn fixture(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
}

/// `format: <ext>-html` extensions are resolved, not just `-revealjs`: the
/// contributed header AND before-body includes both land (before-body was
/// previously untested).
/// A `{{< name >}}` that no extension (and no built-in) recognises must not fail
/// silently. It stays as literal text in the output (so nothing breaks), but the
/// renderer surfaces a warning naming the shortcode and its line, so a typo'd name
/// shows up in the build log and the preview diagnostics instead of shipping
/// verbatim into the page.
#[test]
fn unknown_shortcode_warns_with_its_name_and_line() {
    let proj = TempProj::new();
    let doc = qmd_fast_core::render_document_with_includes(
        "# Title\n\nIntro.\n\n{{< videoo clip.mp4 >}}\n",
        &proj.0,
    );
    assert!(
        doc.warnings.iter().any(|w| w.contains("unknown shortcode")
            && w.contains("videoo")
            && w.contains("line 5")),
        "expected an unknown-shortcode warning naming `videoo` at line 5, got: {:?}",
        doc.warnings
    );
}

/// A missing include is left verbatim by the include resolver (which reports it on
/// its own); the shortcode pass must not *also* flag `{{< include >}}` as an unknown
/// shortcode, or every broken include would double-warn.
#[test]
fn a_leftover_include_directive_is_not_flagged_as_an_unknown_shortcode() {
    let proj = TempProj::new();
    let doc = qmd_fast_core::render_document_with_includes(
        "# Title\n\n{{< include does-not-exist.qmd >}}\n",
        &proj.0,
    );
    assert!(
        !doc.warnings.iter().any(|w| w.contains("unknown shortcode")),
        "a leftover include must not warn as an unknown shortcode: {:?}",
        doc.warnings
    );
}

#[test]
fn html_base_extension_injects_header_and_before_body() {
    let d = TempProj::new();
    d.ext(
        "brand",
        "head:
  - file: brand-head.html
body-start:
  - file: brand-top.html
",
    );
    d.file("_extensions/brand/brand-head.html", "<meta name=\"brand\">");
    d.file(
        "_extensions/brand/brand-top.html",
        "<div id=\"brand-top\"></div>",
    );

    let src = "---\ntitle: T\nformat: brand-html\n---\n\n# H\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(
        doc.includes.in_header.contains("name=\"brand\""),
        "header not injected: {}",
        doc.includes.in_header
    );
    assert!(
        doc.includes.before_body.contains("brand-top"),
        "before-body not injected: {}",
        doc.includes.before_body
    );
}

/// `format-resources` (scalar or list, possibly in a subdir) are collected onto
/// `includes.resources` so `build` can copy them next to the output page.
#[test]
fn format_resources_are_collected_for_copying() {
    let d = TempProj::new();
    d.ext(
        "deck",
        "resources:
  - plugin.js
  - assets/extra.css
",
    );
    d.file("_extensions/deck/plugin.js", "// js");
    d.file("_extensions/deck/assets/extra.css", "/* css */");

    let src = "---\ntitle: T\nformat: deck-revealjs\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    let names: Vec<String> = doc
        .includes
        .resources
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&"plugin.js".to_string()), "{names:?}");
    assert!(names.contains(&"extra.css".to_string()), "{names:?}");
    // The collected paths must exist (point inside the extension dir).
    assert!(
        doc.includes.resources.iter().all(|p| p.exists()),
        "{names:?}"
    );
}

/// The extension's contributed header is placed *ahead* of the document's own
/// `include-in-header`, so the author's front matter can override the extension.
#[test]
fn extension_header_precedes_document_header() {
    let d = TempProj::new();
    d.ext(
        "lib",
        "head:
  - text: \"<!--EXT-->\"
",
    );
    let src = "---\ntitle: T\nformat: lib-revealjs\ninclude-in-header:\n  - text: \"<!--DOC-->\"\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    let h = &doc.includes.in_header;
    let ext_at = h.find("EXT").expect("extension header present");
    let doc_at = h.find("DOC").expect("document header present");
    assert!(ext_at < doc_at, "extension must precede doc: {h}");
}

/// A contributed `theme: [dark, x.css]` inlines the `.css` layer AND applies the
/// built-in `dark` base, so the deck defaults to dark when the doc names no
/// `theme:` of its own (matching Quarto, where the extension owns the look).
#[test]
fn extension_theme_inlines_css_and_applies_builtin_base() {
    let d = TempProj::new();
    d.ext("glassy", "theme: [dark, glassy.css]\n");
    d.file("_extensions/glassy/glassy.css", ".qmd-deck{--marker:1}");
    let src = "---\ntitle: T\nformat: glassy-revealjs\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(
        doc.includes.in_header.contains("--marker:1"),
        "css layer should be inlined: {}",
        doc.includes.in_header
    );
    assert_eq!(
        doc.theme_default, "dark",
        "the extension's built-in `dark` base should set the default mode"
    );
}

/// The doc's own `theme:` wins over the extension's contributed base: the
/// extension only supplies the default when the doc didn't pick one.
#[test]
fn doc_theme_overrides_extension_theme_base() {
    let d = TempProj::new();
    d.ext("glassy", "theme: [dark, glassy.css]\n");
    d.file("_extensions/glassy/glassy.css", ".qmd-deck{--marker:1}");
    let src = "---\ntitle: T\nformat: glassy-revealjs\ntheme: light\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert_eq!(
        doc.theme_default, "light",
        "the doc's own theme: must beat the extension's contributed base"
    );
}

/// A typo'd / unknown extension name renders cleanly AND is reported via the
/// warnings channel (so the author isn't left guessing why it did nothing).
#[test]
fn unknown_extension_name_is_reported() {
    let d = TempProj::new();
    let src = "---\ntitle: T\nformat: doesnotexist-revealjs\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(doc.includes.in_header.is_empty());
    assert!(!doc.blocks.is_empty(), "the doc still renders normally");
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("doesnotexist") && w.contains("not found")),
        "expected a 'not found' warning, got: {:?}",
        doc.warnings
    );
}

/// A bare base format (`revealjs`/`html`) is NOT an extension request, so it must
/// render silently — no spurious "extension not found" warning.
#[test]
fn bare_base_format_does_not_warn() {
    let d = TempProj::new();
    let src = "---\ntitle: T\nformat: revealjs\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(
        doc.warnings.iter().all(|w| !w.contains("extension")),
        "a plain base format must not warn: {:?}",
        doc.warnings
    );
}

/// A malformed `_extension.yml` is reported (not fatal): the render still
/// succeeds and a parse warning is surfaced.
#[test]
fn malformed_manifest_is_reported_not_fatal() {
    let d = TempProj::new();
    d.ext("broken", "theme: [this is not, valid: yaml");
    let src = "---\ntitle: T\nformat: broken-revealjs\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(doc.includes.in_header.is_empty(), "malformed ext ignored");
    assert!(!doc.blocks.is_empty(), "render still succeeds");
    assert!(
        doc.warnings.iter().any(|w| w.contains("could not parse")),
        "expected a parse warning, got: {:?}",
        doc.warnings
    );
}

/// A manifest that references a missing file leaves an HTML-comment breadcrumb
/// in the header rather than failing. (Missing *extensions* and malformed
/// manifests are now reported through the warnings channel; a missing *included
/// file* still only leaves this in-output breadcrumb.)
#[test]
fn missing_referenced_file_leaves_a_breadcrumb_comment() {
    let d = TempProj::new();
    d.ext(
        "partial",
        "head:
  - file: nope.html
",
    );
    let src = "---\ntitle: T\nformat: partial-revealjs\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(
        doc.includes
            .in_header
            .contains("include file not found: nope.html"),
        "expected a not-found breadcrumb, got: {}",
        doc.includes.in_header
    );
    assert!(!doc.blocks.is_empty());
}

/// An extension's `shortcodes:` template expands `{{< name args >}}` in the body,
/// with a positional arg filling `{{1}}`.
#[test]
fn declarative_shortcode_expands_positional() {
    let d = TempProj::new();
    d.ext(
        "media",
        "shortcodes:
  yt: '<iframe src=\"https://www.youtube.com/embed/{{1}}\"></iframe>'
",
    );
    let src = "---\ntitle: T\nformat: media-html\n---\n\nWatch {{< yt dQw4 >}} now.\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    let body = doc.body_html();
    assert!(
        body.contains("youtube.com/embed/dQw4"),
        "shortcode not expanded: {body}"
    );
    assert!(!body.contains("{{<"), "raw shortcode left behind: {body}");
}

/// Named args fill `{{key}}` placeholders; quotes group spaces.
#[test]
fn declarative_shortcode_named_args() {
    let d = TempProj::new();
    d.ext(
        "media",
        "shortcodes:
  embed: '<iframe width=\"{{width}}\" title=\"{{title}}\" src=\"/v/{{id}}\"></iframe>'
",
    );
    let src = "---\ntitle: T\nformat: media-html\n---\n\n{{< embed id=abc width=560 title=\"A Clip\" >}}\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    let body = doc.body_html();
    assert!(body.contains("width=\"560\""), "named width: {body}");
    assert!(body.contains("/v/abc"), "named id: {body}");
    assert!(body.contains("title=\"A Clip\""), "quoted value: {body}");
}

/// The built-in `{{< embed deck.qmd >}}` works with no extensions loaded: it emits
/// an isolating iframe whose `src` maps `.qmd` → `.html`, an accessible title, and an
/// "open in a new tab" link to the same deck.
#[test]
fn builtin_embed_emits_deck_iframe() {
    let src = "---\ntitle: T\n---\n\n{{< embed talk.qmd title=\"My Talk\" >}}\n";
    let doc = qmd_fast_core::render_document_with_includes(src, std::path::Path::new("."));
    let body = doc.body_html();
    assert!(
        body.contains("class=\"qmd-embed\""),
        "embed wrapper: {body}"
    );
    assert!(body.contains("src=\"talk.html\""), "qmd->html src: {body}");
    assert!(body.contains("title=\"My Talk\""), "title arg: {body}");
    assert!(body.contains("href=\"talk.html\""), "open link: {body}");
}

/// The built-in `{{< video clip.mp4 >}}` emits a framed, autoplaying, muted, looping
/// `<video>` with an optional caption — so a page needs no raw `<video>` HTML.
#[test]
fn builtin_video_emits_autoplay_figure() {
    let src = "---\ntitle: T\n---\n\n{{< video assets/clip.mp4 caption=\"A demo\" >}}\n";
    let doc = qmd_fast_core::render_document_with_includes(src, std::path::Path::new("."));
    let body = doc.body_html();
    assert!(
        body.contains("class=\"qmd-video\""),
        "video wrapper: {body}"
    );
    assert!(
        body.contains("src=\"assets/clip.mp4\""),
        "video src: {body}"
    );
    assert!(
        body.contains("autoplay") && body.contains("muted") && body.contains("loop"),
        "screencast attrs: {body}"
    );
    assert!(
        body.contains("<figcaption>A demo</figcaption>"),
        "caption: {body}"
    );
}

/// A shortcode shown as an example inside an inline code span stays literal (it is
/// not expanded), so docs can describe `{{< embed … >}}` without triggering it.
#[test]
fn shortcode_in_inline_code_stays_literal() {
    let src = "---\ntitle: T\n---\n\nUse `{{< embed deck.qmd >}}` to embed a deck.\n";
    let doc = qmd_fast_core::render_document_with_includes(src, std::path::Path::new("."));
    let body = doc.body_html();
    assert!(
        body.contains("{{&lt; embed deck.qmd &gt;}}") || body.contains("{{< embed deck.qmd >}}"),
        "inline-code shortcode should stay literal: {body}"
    );
    assert!(
        !body.contains("class=\"qmd-embed\""),
        "must not expand inside inline code: {body}"
    );
}

/// A shortcode the active extension does not declare is left verbatim (not an
/// error — it may be Quarto syntax qmd-fast doesn't handle).
#[test]
fn unknown_shortcode_is_left_verbatim() {
    let d = TempProj::new();
    d.ext(
        "media",
        "shortcodes:
  yt: '<iframe src=\"/v/{{1}}\"></iframe>'
",
    );
    let src = "---\ntitle: T\nformat: media-html\n---\n\nText {{< unknownsc x >}} more.\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(
        doc.body_html().contains("unknownsc x"),
        "unknown shortcode should survive: {}",
        doc.body_html()
    );
}

#[test]
fn format_extension_injects_theme_and_includes() {
    let dir = fixture("deck-ext");
    let src = fs::read_to_string(dir.join("slides.qmd")).expect("read slides.qmd");
    let html = qmd_fast_core::render_html_page_with_includes(&src, &dir, "Glass Deck");

    // Renders as a deck (the `-revealjs` base format is detected).
    assert!(
        html.contains("<div class=\"qmd-deck\">"),
        "should render as a deck"
    );
    // contributed theme: glass.css inlined as <style>.
    assert!(
        html.contains(".qmd-deck .qmd-slides section h2 { color: #2bd4a0; }"),
        "extension theme css not inlined"
    );
    // contributed head (file: glass-head.html), inside <head>.
    let head = &html[..html.find("</head>").expect("has </head>")];
    assert!(
        head.contains(r#"<meta name="glass-ext" content="active">"#),
        "extension head not injected into <head>"
    );
    // contributed body-end (file: glass-init.html).
    assert!(
        html.contains("window.__glassExt = true;"),
        "extension body-end not injected"
    );
}

#[test]
fn plain_format_without_extension_is_untouched() {
    // A bare `format: revealjs` has no extension prefix, so nothing extra is pulled.
    let src = "---\ntitle: T\nformat: revealjs\n---\n\n## S\n";
    let html = qmd_fast_core::render_html_page_with_includes(src, &fixture("deck-ext"), "T");
    assert!(html.contains("<div class=\"qmd-deck\">"));
    assert!(
        !html.contains("glass-ext"),
        "a non-extension format must not pull extension includes"
    );
}

/// A flat native `_extension.yml` contributes theme / head / body-end / resources
/// / shortcodes — the friendly schema.
#[test]
fn native_flat_manifest_contributes_everything() {
    let d = TempProj::new();
    d.ext(
        "glassy",
        "name: Glassy\n\
         theme: [dark, glassy.css]\n\
         head: head.html\n\
         body-end: init.html\n\
         resources: [glassy.js]\n\
         shortcodes:\n  yt: '<iframe src=\"/v/{{1}}\"></iframe>'\n",
    );
    d.file("_extensions/glassy/glassy.css", ".qmd-deck{--g:1}");
    d.file("_extensions/glassy/head.html", "<meta name=\"glassy\">");
    d.file(
        "_extensions/glassy/init.html",
        "<script>window.__g=1</script>",
    );
    d.file("_extensions/glassy/glassy.js", "// js");

    let src = "---\ntitle: T\nformat: glassy-revealjs\n---\n\nWatch {{< yt abc >}}.\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(
        doc.includes.in_header.contains("--g:1"),
        "theme layer inlined"
    );
    assert!(
        doc.includes.in_header.contains("name=\"glassy\""),
        "head injected"
    );
    assert!(
        doc.includes.after_body.contains("window.__g=1"),
        "body-end injected"
    );
    assert!(
        doc.includes
            .resources
            .iter()
            .any(|p| p.ends_with("glassy.js")),
        "resource collected"
    );
    assert!(
        doc.body_html().contains("/v/abc"),
        "native shortcode expanded"
    );
    assert!(
        doc.warnings.is_empty(),
        "clean manifest: {:?}",
        doc.warnings
    );
}

/// A typo'd key in a native manifest is reported with a "did you mean".
#[test]
fn native_manifest_unknown_key_is_warned() {
    let d = TempProj::new();
    d.ext("typo", "name: T\nresorces: [x.js]\n"); // resorces -> resources
    let src = "---\ntitle: T\nformat: typo-revealjs\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("resorces") && w.contains("resources")),
        "expected a did-you-mean manifest warning, got: {:?}",
        doc.warnings
    );
}

/// The `extensions: [name]` list activates a shortcode/enhancer extension without
/// hijacking `format:` (the general activation).
#[test]
fn extensions_list_activates_without_format() {
    let d = TempProj::new();
    d.ext(
        "widgets",
        "name: Widgets\ncss: w.css\nshortcodes:\n  hi: '<b>hi {{1}}</b>'\n",
    );
    d.file("_extensions/widgets/w.css", ".w{color:red}");
    let src = "---\ntitle: T\nextensions: [widgets]\n---\n\nSay {{< hi there >}}.\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(
        doc.body_html().contains("<b>hi there</b>"),
        "shortcode from the extensions: list: {}",
        doc.body_html()
    );
    assert!(
        doc.includes.in_header.contains(".w{color:red}"),
        "css from the extensions: list"
    );
    assert!(doc.warnings.is_empty(), "clean: {:?}", doc.warnings);
}

/// An extension vendored at the project root is found from a doc in a subdirectory
/// (resolution walks up the tree), so a whole book/site shares one `_extensions/`.
#[test]
fn extension_resolved_from_project_root() {
    let d = TempProj::new();
    d.ext("widgets", "name: W\nshortcodes:\n  hi: '<b>{{1}}</b>'\n"); // at <root>/_extensions
    let src = "---\ntitle: T\nextensions: [widgets]\n---\n\n{{< hi yo >}}\n";
    let base = d.0.join("chapters"); // a doc one level deeper than the extension
    let doc = qmd_fast_core::render_document_with_includes(src, &base);
    assert!(
        doc.body_html().contains("<b>yo</b>"),
        "extension should be found by walking up to the project root: {}",
        doc.body_html()
    );
}

/// A shortcode shown as an *example* inside a fenced code block stays literal (it's
/// documentation, not an invocation) — only the real invocation expands.
#[test]
fn shortcode_in_code_block_is_left_literal() {
    let d = TempProj::new();
    d.ext(
        "media",
        "name: M\nshortcodes:\n  yt: '<iframe src=\"/v/{{1}}\"></iframe>'\n",
    );
    let src = "---\ntitle: T\nextensions: [media]\n---\n\nExample:\n\n```\n{{< yt abc >}}\n```\n\nLive: {{< yt xyz >}}\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    let body = doc.body_html();
    assert!(
        body.contains("/v/xyz"),
        "the real invocation expands: {body}"
    );
    assert!(
        !body.contains("/v/abc"),
        "the code-block example must stay literal (not expand): {body}"
    );
}
