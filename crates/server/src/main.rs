//! qmd-fast — dev server & CLI entry point.
//!
//! Phase 0 skeleton. The real surface arrives in later phases:
//!   - `qmd-fast render <file.qmd>`  (Phase 1: one-shot HTML)
//!   - `qmd-fast serve <path>`       (Phase 2: long-running dev server)

fn main() {
    println!("qmd-fast {} (Phase 0 skeleton)", qmd_fast_core::VERSION);
}
