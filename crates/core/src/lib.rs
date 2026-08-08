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
//!   - the `include-in-header` / `-before-body` / `-after-body` / `css` markup
//!     ([`render::PageIncludes`]), injected verbatim into the page template,
//!   - the site `page-footer` item text (icon SVGs) in the chrome.
//!
//! Code cells are likewise *executed* against a live kernel. None of this is a
//! vulnerability under the intended use, but it means taliesin must **not** be
//! pointed at a `.tmd` from an untrusted source: doing so would be arbitrary
//! HTML/JS injection (and arbitrary code execution via cells). If multi-author
//! or hosted rendering is ever added, these passthrough sites are exactly what
//! needs sanitizing first. Navbar labels and other text *are* escaped
//! ([`escape_attr`] / [`html_escape`]); the list above is the deliberate
//! exception.

pub mod agents;
pub(crate) mod author;
pub mod cite;
pub mod diagnostics;
pub mod diff;
pub mod ext;
pub mod features;
pub mod frontmatter;
pub mod hash;
pub mod highlight;
pub mod includes;
pub mod math;
pub mod math_preview;
mod math_vocab;
pub mod prose;
pub mod render;
pub mod schema;
pub mod site;
pub mod vocab;

pub use diff::{BlockOp, diff_blocks};
pub use frontmatter::{closest, closest_of};
pub use includes::single_doc_root;
pub use render::{
    AssetMode, Block, DeckParts, DocFormat, ExecOutput, ExternalAssets, FONT_FILES, OutputMode,
    PREVIEW_MERMAID_PATH, PageParts, RenderedDoc, SEARCH_JS, ScriptSummary, TOC_SPY_JS,
    assemble_deck_page, assemble_html_page, classify_exec_output, code_scripts, core_enhance_js,
    deck_overlay_html, deck_shared_css, deck_shared_css_linked_fonts, deck_shared_js, escape_attr,
    favicon_link, has_mermaid, html_escape, html_page_from_doc_in_site_external, js_cell_libs_js,
    katex_css, mermaid_bundle_js, mermaid_min_js, render_deck_to_page_external, render_doc_to_page,
    render_doc_to_page_external, render_document, render_document_scoped_with_site,
    render_document_with_includes, render_document_with_includes_scoped, render_html_page,
    render_html_page_with_includes, render_single_doc, script_summary, shared_site_css,
    shared_site_css_linked_fonts, slides_html, title_with_site_suffix,
};
pub use site::{DraftMode, Page, Site};

/// Crate version, surfaced so the server/CLI can report a single source of truth.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_present() {
        assert!(!VERSION.is_empty());
    }
}
