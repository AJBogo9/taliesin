//! Static document-lint validators for `qmd-fast check` (the "check-superset").
//!
//! **What:** one read-only validator per family — headings, anchors, bibliography,
//! assets, media, links, reactive graph, a11y — each takes the rendered block model
//! (and, where needed, the doc base dir) and returns located [`Warning`]s on the same
//! click-to-source channel as render-time diagnostics, so a green `check` means the
//! document is publishable.
//!
//! **How to use:** call the re-exported `validate_*` / `citations_without_bibliography`
//! fns; `qmd-fast check` (`crates/server/src/main.rs`) runs the whole set.
//!
//! **Depends on:** [`crate::render`] for the block model + `Warning` channel, and
//! `std::path` for the asset/link existence checks. Pure static analysis; the only IO
//! is stat-ing referenced local files.
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
pub use bibliography::citations_without_bibliography;
pub use headings::validate_duplicate_heading_ids;
pub use links::validate_local_links;
pub use media::validate_local_media;
pub use reactive::validate_js_reactive_graph;
