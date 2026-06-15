//! qmd-fast-core
//!
//! The editor-agnostic rendering core: `.qmd` parsing (comrak + sourcepos),
//! the block model, and HTML rendering. All intelligence lives here; the
//! server and clients are thin layers over this crate.

pub mod cite;
pub mod diff;
pub mod includes;
pub mod math;
pub mod render;

pub use diff::{BlockOp, diff_blocks};
pub use render::{
    Block, DocFormat, RenderedDoc, client_styles, render_document, render_document_with_includes,
    render_html_page, render_html_page_with_includes, reveal_client_head, reveal_client_script,
    slides_html,
};

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
