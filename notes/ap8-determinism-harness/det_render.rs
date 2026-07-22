//! AP8 determinism harness (audit-only). Render a `.tmd` to a full HTML page and
//! print it to stdout. Run the SAME file through this in TWO SEPARATE PROCESSES and
//! byte-compare: separate processes get different HashMap `RandomState` seeds, so any
//! map-iteration-order dependence in the render path shows up as differing output.
//!
//! Usage: `det_render <file.tmd>`  (base_dir = the file's parent, for includes)

use std::path::Path;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: det_render <file.tmd>");
    let src = std::fs::read_to_string(&path).expect("read .tmd");
    let base = Path::new(&path).parent().unwrap_or(Path::new("."));
    let html = taliesin_core::render_html_page_with_includes(&src, base, "det");
    print!("{html}");
}
