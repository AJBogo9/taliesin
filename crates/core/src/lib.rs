//! qmd-fast-core
//!
//! The editor-agnostic rendering core: `.qmd` parsing (comrak + sourcepos),
//! the block model, and HTML rendering. All intelligence lives here; the
//! server and clients are thin layers over this crate.
//!
//! # Trust model
//!
//! qmd-fast renders **one author's own `.qmd` files** (the single-author
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
//! vulnerability under the intended use, but it means qmd-fast must **not** be
//! pointed at a `.qmd` from an untrusted source: doing so would be arbitrary
//! HTML/JS injection (and arbitrary code execution via cells). If multi-author
//! or hosted rendering is ever added, these passthrough sites are exactly what
//! needs sanitizing first. Navbar labels and other text *are* escaped
//! ([`escape_attr`] / [`html_escape`]); the list above is the deliberate
//! exception.

pub mod cite;
pub mod diff;
pub mod frontmatter;
pub mod highlight;
pub mod includes;
pub mod math;
pub(crate) mod prose;
pub mod render;
pub mod schema;
pub mod site;

pub use diff::{BlockOp, diff_blocks};
pub use render::{
    Block, DeckParts, DocFormat, PageParts, RenderedDoc, SEARCH_JS, SiteCtx, TOC_SPY_JS,
    assemble_deck_page, assemble_html_page, code_scripts, deck_client_script, escape_attr,
    favicon_link, html_escape, html_page_from_doc_in_site, render_doc_to_page, render_document,
    render_document_with_includes, render_html_page, render_html_page_with_includes, slides_html,
};
pub use site::{DeckRef, Page, Site, SiteConfig};

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
