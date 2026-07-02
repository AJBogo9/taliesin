//! Read-only query subcommands: `render`, `blocks`, `schema`.
//!
//! **What:** one-shot, side-effect-light commands — `render` dumps a full HTML page to
//! stdout (static, no kernel), `blocks` lists the block model (a debugging aid), and
//! `schema` emits the bundled JSON Schemas for editor autocomplete.
//!
//! **How to use:** `main()` dispatches `render`/`blocks`/`schema` to the `cmd_*` fns here.
//!
//! **Depends on:** [`taliesin_core`] for rendering + the bundled schemas, and
//! [`crate::log`] for the no-execution warning. No code execution, no kernel.

use crate::log;
use std::path::Path;
use std::process::ExitCode;

pub(crate) fn cmd_render(path: Option<&String>) -> ExitCode {
    let Some(path) = path else {
        eprintln!("usage: taliesin render <file.qmd>");
        return ExitCode::FAILURE;
    };
    match std::fs::read_to_string(path) {
        Ok(src) => {
            let p = Path::new(path);
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
            let base = p.parent().unwrap_or_else(|| Path::new("."));
            // Guard the render: a panic in core rendering becomes a located error +
            // non-zero exit, not a raw abort (this one-shot has no async loop to absorb it).
            let rendered = crate::serve::guarded(|| {
                let doc = taliesin_core::render_document_with_includes(&src, base);
                // `render` is a static, one-shot HTML dump: unlike `build`/`preview` it
                // never starts a kernel, so kernel-executed cells (python/r) emit as
                // source with empty output blocks — broken `@fig-` refs, no plots. Warn
                // loudly so the empty output isn't mistaken for a render bug. (`{js}` cells
                // run in the browser, so they're fine here.)
                let kernel_cells = doc
                    .blocks
                    .iter()
                    .filter(|b| {
                        b.cell
                            .as_ref()
                            .is_some_and(|c| matches!(c.lang.as_str(), "python" | "r"))
                    })
                    .count();
                if kernel_cells > 0 {
                    log::warn(&format!(
                        "render does not execute code cells ({kernel_cells} kernel cell{} emitted \
                         as source; figures/outputs will be empty). Use `build` or `preview` to \
                         run them.",
                        if kernel_cells == 1 { "" } else { "s" }
                    ));
                }
                taliesin_core::render_doc_to_page(&doc, stem, taliesin_core::OutputMode::Build)
            });
            match rendered {
                Ok(html) => {
                    print!("{html}");
                    ExitCode::SUCCESS
                }
                Err(panic) => {
                    log::error(&format!("render panicked on {path}: {panic}"));
                    ExitCode::FAILURE
                }
            }
        }
        Err(e) => {
            log::error(&format!("cannot read {path}: {e}"));
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn cmd_blocks(path: Option<&String>) -> ExitCode {
    let Some(path) = path else {
        eprintln!("usage: taliesin blocks <file.qmd>");
        return ExitCode::FAILURE;
    };
    match std::fs::read_to_string(path) {
        Ok(src) => {
            let p = Path::new(path);
            let base = p.parent().unwrap_or_else(|| Path::new("."));
            // Guard the render so a panic becomes a clean error + non-zero exit.
            let doc = match crate::serve::guarded(|| {
                taliesin_core::render_document_with_includes(&src, base)
            }) {
                Ok(doc) => doc,
                Err(panic) => {
                    log::error(&format!("render panicked on {path}: {panic}"));
                    return ExitCode::FAILURE;
                }
            };
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

/// Emit the bundled JSON Schemas for qmd-fast's YAML config (document front matter +
/// `_site.yml`) so an editor's YAML language server can validate them. With `--out <dir>`
/// it writes two files there; otherwise it prints both to stdout. The strings are the
/// committed, bundled schemas (no runtime JSON generation).
pub(crate) fn cmd_schema(args: &[String]) -> ExitCode {
    use taliesin_core::schema::{FRONTMATTER_SCHEMA, SITE_SCHEMA};
    let files = [
        ("tali-frontmatter.schema.json", FRONTMATTER_SCHEMA),
        ("tali-site.schema.json", SITE_SCHEMA),
    ];
    // Optional `--out <dir>` (alias `--dir`), parsed like `cmd_build`.
    let mut out: Option<String> = None;
    let mut it = args.iter().skip(2);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" | "--dir" => out = it.next().cloned(),
            _ => {}
        }
    }
    match out {
        Some(dir) => {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("taliesin schema: cannot create {dir}: {e}");
                return ExitCode::FAILURE;
            }
            for (name, body) in files {
                let path = std::path::Path::new(&dir).join(name);
                if let Err(e) = std::fs::write(&path, body) {
                    eprintln!("taliesin schema: cannot write {}: {e}", path.display());
                    return ExitCode::FAILURE;
                }
                println!("wrote {}", path.display());
            }
            println!(
                "add `# yaml-language-server: $schema={dir}/tali-site.schema.json` atop _site.yml"
            );
        }
        None => {
            for (name, body) in files {
                println!("// {name}");
                print!("{body}");
            }
        }
    }
    ExitCode::SUCCESS
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
