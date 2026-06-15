//! qmd-fast — dev server & CLI entry point.
//!
//! Phase 1 surface:
//!   - `qmd-fast render <file.qmd>`  one-shot full HTML page to stdout
//!   - `qmd-fast blocks <file.qmd>`  list block ids + sourcepos (debugging)
//!
//! Phase 2 will add `qmd-fast serve <path>` (long-running dev server).

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("render") => cmd_render(args.get(2)),
        Some("blocks") => cmd_blocks(args.get(2)),
        _ => {
            usage();
            ExitCode::SUCCESS
        }
    }
}

fn cmd_render(path: Option<&String>) -> ExitCode {
    let Some(path) = path else {
        eprintln!("usage: qmd-fast render <file.qmd>");
        return ExitCode::FAILURE;
    };
    match std::fs::read_to_string(path) {
        Ok(src) => {
            let stem = Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("document");
            print!("{}", qmd_fast_core::render_html_page(&src, stem));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error reading {path}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_blocks(path: Option<&String>) -> ExitCode {
    let Some(path) = path else {
        eprintln!("usage: qmd-fast blocks <file.qmd>");
        return ExitCode::FAILURE;
    };
    match std::fs::read_to_string(path) {
        Ok(src) => {
            let doc = qmd_fast_core::render_document(&src);
            eprintln!("title: {:?}", doc.title);
            eprintln!("{} block(s)\n", doc.blocks.len());
            println!("{:<16}  {:<14}  preview", "id", "sourcepos");
            for b in &doc.blocks {
                println!("{:<16}  {:<14}  {}", b.id, b.sourcepos, preview(&b.html));
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error reading {path}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A short, single-line, tag-free preview of a block's HTML.
fn preview(html: &str) -> String {
    let mut s = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => s.push(if c == '\n' { ' ' } else { c }),
            _ => {}
        }
        if s.chars().count() >= 64 {
            s.push('…');
            break;
        }
    }
    s.trim().to_string()
}

fn usage() {
    println!("qmd-fast {} (Phase 1)", qmd_fast_core::VERSION);
    println!();
    println!("usage:");
    println!("  qmd-fast render <file.qmd>   full HTML page to stdout");
    println!("  qmd-fast blocks <file.qmd>   list block ids + sourcepos");
}
