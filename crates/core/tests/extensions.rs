//! `_extensions/` format-extension resolution: a deck that selects a reveal theme
//! extension via `format: <ext>-revealjs` gets the extension's contributed theme +
//! includes injected (the mechanism behind liquid-glass-revealjs).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

fn fixture(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
}

/// A throwaway project directory for building ad-hoc extensions in-test, so each
/// case can express exactly the manifest/files it needs without committed fixtures.
struct TempProj(PathBuf);

impl TempProj {
    fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "qmd-ext-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        TempProj(p)
    }

    /// Write a file (creating parent dirs) relative to the project root.
    fn file(&self, rel: &str, content: &str) -> &Self {
        let f = self.0.join(rel);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, content).unwrap();
        self
    }

    /// Install an extension `name` whose `_extension.yml` is `manifest`.
    fn ext(&self, name: &str, manifest: &str) -> &Self {
        self.file(&format!("_extensions/{name}/_extension.yml"), manifest)
    }
}

impl Drop for TempProj {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `format: <ext>-html` extensions are resolved, not just `-revealjs`: the
/// contributed header AND before-body includes both land (before-body was
/// previously untested).
#[test]
fn html_base_extension_injects_header_and_before_body() {
    let d = TempProj::new();
    d.ext(
        "brand",
        "contributes:
  formats:
    html:
      include-in-header:
        - file: brand-head.html
      include-before-body:
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
        "contributes:
  formats:
    revealjs:
      format-resources:
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
        "contributes:
  formats:
    revealjs:
      include-in-header:
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

/// A contributed `theme: [dark, x.css]` inlines the `.css` layer, but the
/// built-in `dark` base is currently NOT applied (only a doc's own top-level
/// `theme:` selects built-in light/dark). AUDIT: fidelity gap vs Quarto.
#[test]
fn extension_theme_inlines_css_but_drops_builtin_base() {
    let d = TempProj::new();
    d.ext(
        "glassy",
        "contributes:
  formats:
    revealjs:
      theme: [dark, glassy.css]
",
    );
    d.file("_extensions/glassy/glassy.css", ".reveal{--marker:1}");
    let src = "---\ntitle: T\nformat: glassy-revealjs\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(
        doc.includes.in_header.contains("--marker:1"),
        "css layer should be inlined: {}",
        doc.includes.in_header
    );
    assert_ne!(
        doc.theme_default, "dark",
        "the extension's built-in `dark` base is not applied today (the gap)"
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
    d.ext("broken", "contributes: [this is not, valid: yaml");
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

/// An installed extension that declares no matching `contributes.formats.<base>`
/// block is reported (a common copy/paste mistake).
#[test]
fn extension_without_matching_format_block_is_reported() {
    let d = TempProj::new();
    // declares an `html` block, but the deck asked for the `revealjs` base
    d.ext(
        "mismatch",
        "contributes:
  formats:
    html:
      include-in-header:
        - text: \"<!--x-->\"
",
    );
    let src = "---\ntitle: T\nformat: mismatch-revealjs\n---\n\n## S\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("mismatch") && w.contains("revealjs")),
        "expected a missing-format-block warning, got: {:?}",
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
        "contributes:
  formats:
    revealjs:
      include-in-header:
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

/// An extension's `contributes.shortcodes` template expands `{{< name args >}}`
/// in the body, with a positional arg filling `{{1}}`.
#[test]
fn declarative_shortcode_expands_positional() {
    let d = TempProj::new();
    d.ext(
        "media",
        "contributes:
  shortcodes:
    yt: '<iframe src=\"https://www.youtube.com/embed/{{1}}\"></iframe>'
  formats:
    html:
      include-in-header:
        - text: \"<!--m-->\"
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
        "contributes:
  shortcodes:
    embed: '<iframe width=\"{{width}}\" title=\"{{title}}\" src=\"/v/{{id}}\"></iframe>'
  formats:
    html: {}
",
    );
    let src = "---\ntitle: T\nformat: media-html\n---\n\n{{< embed id=abc width=560 title=\"A Clip\" >}}\n";
    let doc = qmd_fast_core::render_document_with_includes(src, &d.0);
    let body = doc.body_html();
    assert!(body.contains("width=\"560\""), "named width: {body}");
    assert!(body.contains("/v/abc"), "named id: {body}");
    assert!(body.contains("title=\"A Clip\""), "quoted value: {body}");
}

/// A shortcode the active extension does not declare is left verbatim (not an
/// error — it may be Quarto syntax qmd-fast doesn't handle).
#[test]
fn unknown_shortcode_is_left_verbatim() {
    let d = TempProj::new();
    d.ext(
        "media",
        "contributes:
  shortcodes:
    yt: '<iframe src=\"/v/{{1}}\"></iframe>'
  formats:
    html: {}
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

    // Renders as a reveal deck (the `-revealjs` base format is detected).
    assert!(
        html.contains("<div class=\"reveal\">"),
        "should render as a reveal deck"
    );
    // contributed theme: glass.css inlined as <style>.
    assert!(
        html.contains(".reveal .slides section h2 { color: #2bd4a0; }"),
        "extension theme css not inlined"
    );
    // contributed include-in-header (file: glass-head.html), inside <head>.
    let head = &html[..html.find("</head>").expect("has </head>")];
    assert!(
        head.contains(r#"<meta name="glass-ext" content="active">"#),
        "extension include-in-header not injected into <head>"
    );
    // contributed include-after-body (file: glass-init.html).
    assert!(
        html.contains("window.__glassExt = true;"),
        "extension include-after-body not injected"
    );
}

#[test]
fn plain_format_without_extension_is_untouched() {
    // A bare `format: revealjs` has no extension prefix, so nothing extra is pulled.
    let src = "---\ntitle: T\nformat: revealjs\n---\n\n## S\n";
    let html = qmd_fast_core::render_html_page_with_includes(src, &fixture("deck-ext"), "T");
    assert!(html.contains("<div class=\"reveal\">"));
    assert!(
        !html.contains("glass-ext"),
        "a non-extension format must not pull extension includes"
    );
}

/// A flat **native** `_extension.yml` (no `contributes:` nesting) contributes the
/// same theme / head / body-end / resources / shortcodes — the friendly schema.
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
    d.file("_extensions/glassy/glassy.css", ".reveal{--g:1}");
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
