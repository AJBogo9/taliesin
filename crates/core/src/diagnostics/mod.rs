//! Static document-lint validators — the "check-superset".
//!
//! **What:** one read-only validator per family — headings, anchors, bibliography, assets,
//! media, links, reactive graph, a11y, retired cell languages — each takes the rendered
//! block model (and, where needed, the doc base dir) and returns located [`Warning`]s on the
//! same click-to-source channel as render-time diagnostics, so a green run means the document
//! is publishable.
//!
//! **How to use:** call the re-exported `validate_*` / `citations_without_bibliography`
//! fns. `crates/server/src/lint.rs`'s `page_static_diagnostics` runs the whole set, and is
//! the single definition of it for `build`, `build --check-only`, the live preview and the
//! LSP alike.
//!
//! **Depends on:** [`crate::render`] for the block model + `Warning` channel, and
//! `std::path` for the asset/link existence checks. Pure static analysis; the only IO
//! is stat-ing referenced local files.
//!
//! The families cut on 2026-08-08 (document shape, KaTeX render failure, link-text
//! collision, accessible-name, and the generic unknown-fence-language lint) all named a
//! defect the author can see in the preview, which is the test the survivors are kept by.
//!
//! [`Warning`]: crate::render::Warning

mod a11y;
mod anchors;
mod assets;
mod bibliography;
mod headings;
mod helpers;
mod links;
mod media;
mod reactive;

#[cfg(test)]
mod tests;

pub use a11y::validate_a11y;
pub use anchors::validate_internal_anchors;
pub use assets::validate_local_assets;
pub use bibliography::{bare_citation_key_not_rendered, citations_without_bibliography};
pub use headings::validate_duplicate_heading_ids;
pub use helpers::extract_suggestion;
pub use links::validate_local_links;
pub use media::validate_local_media;
pub use reactive::validate_js_reactive_graph;
