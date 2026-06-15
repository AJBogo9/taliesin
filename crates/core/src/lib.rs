//! qmd-fast-core
//!
//! The editor-agnostic rendering core: `.qmd` parsing (comrak + sourcepos),
//! the block model, and incremental HTML rendering. All intelligence lives
//! here; the server and clients are thin layers over this crate.
//!
//! Currently a Phase 0 skeleton — nothing renders yet.

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
