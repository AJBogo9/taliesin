//! AP2 fuzzing harness (audit-only, not shipped).
//!
//! Reads ONE case from stdin, runs it through a chosen render entry point, and
//! prints `OK`. A per-case subprocess driver isolates panics (exit 101),
//! stack-overflow aborts (SIGABRT/SIGSEGV), and hangs (external timeout).
//!
//! Usage: `fuzz_render <mode> [base_dir]`  where mode is one of:
//!   doc  | page | deck | site
//! stdin is the raw `.tmd` bytes (decoded lossily, matching how the server
//! reads a file as a UTF-8 string before rendering).

use std::io::Read;
use std::path::Path;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "doc".into());
    let base = std::env::args().nth(2);
    let mut buf = Vec::new();
    std::io::stdin()
        .read_to_end(&mut buf)
        .expect("read stdin (harness plumbing, not under test)");
    let src = String::from_utf8_lossy(&buf).into_owned();

    match mode.as_str() {
        "doc" => {
            let _ = taliesin_core::render_document(&src);
        }
        "page" => {
            let _ = taliesin_core::render_html_page(&src, "fuzz");
        }
        "inc" => {
            let dir = base.as_deref().unwrap_or(".");
            let _ = taliesin_core::render_document_with_includes(&src, Path::new(dir));
        }
        "page_inc" => {
            let dir = base.as_deref().unwrap_or(".");
            let _ = taliesin_core::render_html_page_with_includes(&src, Path::new(dir), "fuzz");
        }
        other => {
            eprintln!("unknown mode: {other}");
            std::process::exit(2);
        }
    }
    // Reaching here means the pipeline returned without panicking.
    println!("OK");
}
