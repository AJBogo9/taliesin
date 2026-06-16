//! qmd-fast — dev server & CLI entry point.
//!
//!   - `qmd-fast render <file.qmd>`       one-shot full HTML page to stdout
//!   - `qmd-fast blocks <file.qmd>`       list block ids + sourcepos (debugging)
//!   - `qmd-fast serve  <file.qmd> [port]` long-running preview dev server

mod exec;
mod kernel;
mod log;
mod serve;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("render") => cmd_render(args.get(2)),
        Some("blocks") => cmd_blocks(args.get(2)),
        Some("serve") => cmd_serve(&args),
        _ => {
            usage();
            ExitCode::SUCCESS
        }
    }
}

fn cmd_serve(args: &[String]) -> ExitCode {
    // Positionals are <file.qmd> [port]; flags (e.g. --open) may appear anywhere.
    let positionals: Vec<&String> = args[2..].iter().filter(|a| !a.starts_with("--")).collect();
    let open = args.iter().any(|a| a == "--open") || std::env::var_os("QMD_FAST_OPEN").is_some();
    let Some(path) = positionals.first() else {
        eprintln!("usage: qmd-fast serve <file.qmd> [port] [--open]");
        return ExitCode::FAILURE;
    };
    let port: u16 = positionals
        .get(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(4321);
    match serve::run(PathBuf::from(path), port, open) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error(&format!("serve: {e}"));
            ExitCode::FAILURE
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
            let p = Path::new(path);
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
            let base = p.parent().unwrap_or_else(|| Path::new("."));
            print!(
                "{}",
                qmd_fast_core::render_html_page_with_includes(&src, base, stem)
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error(&format!("cannot read {path}: {e}"));
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
            let p = Path::new(path);
            let base = p.parent().unwrap_or_else(|| Path::new("."));
            let doc = qmd_fast_core::render_document_with_includes(&src, base);
            eprintln!("title: {:?}", doc.title);
            eprintln!("{} block(s)\n", doc.blocks.len());
            println!(
                "{:<16}  {:<14}  {:<22}  preview",
                "id", "sourcepos", "source-file"
            );
            for b in &doc.blocks {
                let file = b.source_file.as_deref().unwrap_or("(primary)");
                println!(
                    "{:<16}  {:<14}  {:<22}  {}",
                    b.id,
                    b.sourcepos,
                    file,
                    preview(&b.html)
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error(&format!("cannot read {path}: {e}"));
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
    println!("qmd-fast {}", qmd_fast_core::VERSION);
    println!("A fast .qmd -> HTML renderer and live preview server.");
    println!();
    println!("USAGE:");
    println!("  qmd-fast <command> <file.qmd> [args]");
    println!();
    println!("COMMANDS:");
    println!("  render <file.qmd>          render a full HTML page to stdout");
    println!("  blocks <file.qmd>          list block ids + sourcepos (debug)");
    println!("  serve  <file.qmd> [port] [--open]");
    println!("                             live preview server (default port 4321;");
    println!("                             auto-picks the next free port if busy;");
    println!("                             --open launches the browser)");
    println!();
    println!("ENV: QMD_FAST_PYTHON (kernel), QMD_FAST_OPEN (=--open), QMD_FAST_NO_CLEAR");
}
