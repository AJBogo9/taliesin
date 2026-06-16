//! qmd-fast — dev server & CLI entry point.
//!
//!   - `qmd-fast preview <file.qmd> [port]` live preview server (aliases: dev, serve)
//!   - `qmd-fast build  <file.qmd> [out]`   render a self-contained HTML file
//!   - `qmd-fast render <file.qmd>`         one-shot full HTML page to stdout
//!   - `qmd-fast blocks <file.qmd>`         list block ids + sourcepos (debugging)

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
        Some("build") => cmd_build(&args),
        Some("blocks") => cmd_blocks(args.get(2)),
        // `preview`/`dev` are vite-style aliases for the live server.
        Some("serve" | "preview" | "dev") => cmd_serve(&args),
        _ => {
            usage();
            ExitCode::SUCCESS
        }
    }
}

fn cmd_serve(args: &[String]) -> ExitCode {
    // Positionals are <file.qmd> [port]; flags (--open, --host) may appear anywhere.
    let positionals: Vec<&String> = args[2..].iter().filter(|a| !a.starts_with("--")).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let open = flag("--open") || std::env::var_os("QMD_FAST_OPEN").is_some();
    let expose = flag("--host") || std::env::var_os("QMD_FAST_HOST").is_some();
    let Some(path) = positionals.first() else {
        eprintln!("usage: qmd-fast preview <file.qmd> [port] [--host] [--open]");
        return ExitCode::FAILURE;
    };
    let port: u16 = positionals
        .get(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(4321);
    match serve::run(PathBuf::from(path), port, open, expose) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error(&format!("serve: {e}"));
            ExitCode::FAILURE
        }
    }
}

/// `build <file.qmd> [out.html]`: write a self-contained HTML page to a file
/// (default `<stem>.html` beside the source). `render` stays the stdout path.
fn cmd_build(args: &[String]) -> ExitCode {
    let positionals: Vec<&String> = args[2..].iter().filter(|a| !a.starts_with("--")).collect();
    let Some(path) = positionals.first() else {
        eprintln!("usage: qmd-fast build <file.qmd> [out.html]");
        return ExitCode::FAILURE;
    };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            log::error(&format!("cannot read {path}: {e}"));
            return ExitCode::FAILURE;
        }
    };
    let p = Path::new(path.as_str());
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    let base = p.parent().unwrap_or_else(|| Path::new("."));
    let html = qmd_fast_core::render_html_page_with_includes(&src, base, stem);
    let out: PathBuf = positionals
        .get(1)
        .map(|s| PathBuf::from(s.as_str()))
        .unwrap_or_else(|| base.join(format!("{stem}.html")));
    match std::fs::write(&out, html) {
        Ok(()) => {
            log::built(&out.display().to_string());
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error(&format!("cannot write {}: {e}", out.display()));
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
    println!("  preview <file.qmd> [port] [--host] [--open]");
    println!("                             live preview server (aliases: dev, serve;");
    println!("                             default port 4321, auto-picks a free one;");
    println!("                             --host exposes it on your LAN with a QR code");
    println!("                             to open on a phone; --open launches a browser)");
    println!("  build  <file.qmd> [out.html]");
    println!("                             render a self-contained HTML file");
    println!("                             (default <name>.html beside the source)");
    println!("  render <file.qmd>          render a full HTML page to stdout");
    println!("  blocks <file.qmd>          list block ids + sourcepos (debug)");
    println!();
    println!("ENV: QMD_FAST_PYTHON (kernel), QMD_FAST_OPEN (=--open),");
    println!("     QMD_FAST_HOST (=--host), QMD_FAST_NO_CLEAR");
}
