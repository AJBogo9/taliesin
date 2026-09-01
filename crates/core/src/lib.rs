//! taliesin-core
//!
//! The editor-agnostic rendering core: `.tmd` parsing (comrak + sourcepos),
//! the block model, and HTML rendering. All intelligence lives here; the
//! server and clients are thin layers over this crate.
//!
//! # Trust model
//!
//! taliesin renders **one author's own `.tmd` files** (the single-author
//! workflow this tool exists for), so the document source is *trusted*: it is
//! treated like code the author runs, not like untrusted input. Concretely, the
//! renderer passes several things through **without HTML-escaping** by design:
//!
//!   - raw HTML in the body (`HtmlBlock` / `HtmlInline`) and `` ```{=html} ``
//!     passthrough blocks (the AST emitter),
//!   - the site `page-footer` item text (icon SVGs) in the chrome.
//!
//! There is no longer any AUTHOR-CONFIGURED raw injection: the
//! `include-in-header`/`-before-body`/`-after-body`/`css` family went on 2026-08-02 and
//! `_site.yml`'s `head:` on 2026-08-18, so [`render::PageIncludes`] now carries only the
//! chrome's own markup (SEO meta, feed links, the draft banner).
//!
//! Code cells are likewise *executed* against a live kernel. None of this is a
//! vulnerability under the intended use, but it means taliesin must **not** be
//! pointed at a `.tmd` from an untrusted source: doing so would be arbitrary
//! HTML/JS injection (and arbitrary code execution via cells). If multi-author
//! or hosted rendering is ever added, these passthrough sites are exactly what
//! needs sanitizing first. Navbar labels and other text *are* escaped
//! ([`escape_attr`] / [`html_escape`]); the list above is the deliberate
//! exception.

pub(crate) mod author;
pub mod cite;
pub mod diagnostics;
pub mod diff;
pub mod ext;
pub mod frontmatter;
pub mod hash;
pub mod highlight;
pub mod includes;
pub mod math;
mod math_vocab;
pub mod minify;
pub mod prose;
pub mod render;
pub mod schema;
pub mod site;
pub mod vocab;

pub use diff::{BlockOp, diff_blocks};
pub use frontmatter::{closest, closest_of};
pub use includes::single_doc_root;
pub use minify::minify_css;
pub use render::Severity;
/// The built-in shortcodes the renderer dispatches on, re-exported because the LSP's
/// completion list is a second copy of this set and nothing else ties the two together
/// (`lsp_complete.rs`'s `shortcode_names_and_cell_option_values_are_non_empty_closed_sets`).
/// `render::extension` itself stays crate-private.
pub use render::extension::SHORTCODE_NAMES;
pub use render::{
    AssetMode, Block, ExternalAssets, FONT_FILES, FONT_PRELOAD_NAME, KATEX_FONT_FILES, OutputMode,
    PREVIEW_MERMAID_PATH, PageParts, RenderedDoc, SEARCH_JS, TOC_SPY_JS, assemble_html_page,
    code_scripts, core_enhance_js, escape_attr, favicon_link, has_mermaid, html_escape,
    html_page_from_doc_in_site_external, js_cell_libs_js, katex_css_linked_fonts,
    mermaid_bundle_js, mermaid_min_js, render_doc_to_page, render_doc_to_page_external,
    render_doc_to_page_mermaid_file, render_document, render_document_scoped_with_site,
    render_document_with_includes, render_document_with_includes_scoped, render_html_page,
    render_html_page_with_includes, render_single_doc, shared_site_css,
    shared_site_css_linked_fonts, title_with_site_suffix,
};
pub use site::{DraftMode, Page, Site};

/// Crate version, surfaced so the server/CLI can report a single source of truth.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Start building the process-wide lazy statics in the background, so the first render
/// does not pay for them on the critical path.
///
/// Three costs, all measured 2026-08-27 on a release build, all paid exactly once per
/// process and all of them pure setup no document's content can change:
///
/// | | |
/// |---|---|
/// | syntect's bundled `SyntaxSet` | 13.1 ms |
/// | the `two-face` extras (`ts`, `toml`) | 138.0 ms |
/// | KaTeX's QuickJS context on the [`math`] worker | 24.7 ms |
///
/// Together they are the larger half of a cold `Site::discover`, which every `preview` and
/// every `build` runs before it can show or write anything. Two threads rather than one so
/// the syntax sets and the JS engine boot side by side; each is independent of the other
/// and of everything the caller is about to do (parse `_site.yml`, enumerate pages, read
/// sources), so the whole of it overlaps startup I/O instead of following it.
///
/// Fire-and-forget by design: nothing joins these threads and nothing waits on them. A
/// render that arrives before they finish simply blocks on the same `OnceLock` it always
/// did, so this can only make a run faster or leave it unchanged, never wrong. Spawn
/// failure is ignored for the same reason — the lazy path is still there.
pub fn prewarm() {
    let spawn = |name: &str, f: fn()| {
        let _ = std::thread::Builder::new()
            .name(format!("taliesin-prewarm-{name}"))
            .spawn(f);
    };
    spawn("syntax", highlight::load_syntax_sets);
    // Renders one trivial expression, which boots the KaTeX worker thread's JS context.
    // The result is discarded; the cost it removes is the boot, not this expression.
    spawn("katex", || {
        let _ = math::render("x", false);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_present() {
        assert!(!VERSION.is_empty());
    }
}
