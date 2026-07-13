//! Read-only query subcommands: `render`, `blocks`, `schema`, `vocab`, `symbols`.
//!
//! **What:** one-shot, side-effect-light commands — `render` dumps a full HTML page to
//! stdout (static, no kernel), `blocks` lists the block model (a debugging aid), `schema`
//! emits the bundled JSON Schemas for editor autocomplete, `vocab` emits the bundled
//! editor vocabulary, and `symbols` lists a document's cross-reference targets.
//!
//! **How to use:** `main()` dispatches each to the `cmd_*` fns here.
//!
//! **Depends on:** [`taliesin_core`] for rendering + the bundled schemas, and
//! [`crate::log`] for the no-execution warning. No code execution, no kernel — `symbols`
//! in particular is called from an editor's completion request and must never start one.

use crate::log;
use std::path::Path;
use std::process::ExitCode;

pub(crate) fn cmd_render(path: Option<&String>) -> ExitCode {
    let Some(path) = path else {
        eprintln!("usage: taliesin render <file.tmd>");
        return ExitCode::FAILURE;
    };
    if let Some(msg) = directory_rejection(path, "render renders a single .tmd file") {
        log::error(&msg);
        return ExitCode::FAILURE;
    }
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

/// `taliesin read <file.tmd>`: a plain-text projection of the rendered document, for an
/// agent (or a blind author) reading what it made without a browser. Parse-only and
/// static like `render`/`symbols`: it never starts a kernel, so `{python}`/`{r}` cells
/// project as their source with no executed output (warned, like `render`). A VIEW, not an
/// output format.
pub(crate) fn cmd_read(path: Option<&String>) -> ExitCode {
    let Some(path) = path else {
        eprintln!("usage: taliesin read <file.tmd>");
        return ExitCode::FAILURE;
    };
    if let Some(msg) = directory_rejection(path, "read projects a single .tmd file") {
        log::error(&msg);
        return ExitCode::FAILURE;
    }
    match std::fs::read_to_string(path) {
        Ok(src) => {
            let p = Path::new(path);
            let base = p.parent().unwrap_or_else(|| Path::new("."));
            let text = crate::serve::guarded(|| {
                let doc = taliesin_core::render_document_with_includes(&src, base);
                // Parse-only, like `render`: kernel cells (python/r) project as source with
                // no output. Warn so an empty output isn't mistaken for a projection bug.
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
                        "read does not execute code cells ({kernel_cells} kernel cell{} projected \
                         as source; outputs will be absent). Use `build` or `preview` to run them.",
                        if kernel_cells == 1 { "" } else { "s" }
                    ));
                }
                doc.body_text()
            });
            match text {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(panic) => {
                    log::error(&format!("read panicked on {path}: {panic}"));
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
        eprintln!("usage: taliesin blocks <file.tmd>");
        return ExitCode::FAILURE;
    };
    if let Some(msg) =
        directory_rejection(path, "blocks lists the block model of a single .tmd file")
    {
        log::error(&msg);
        return ExitCode::FAILURE;
    }
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

/// Emit the bundled JSON Schemas for taliesin's YAML config (document front matter +
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

/// Emit the bundled editor vocabulary JSON (front-matter keys, cell options, callout and
/// theorem kinds, div classes, input types, cross-reference prefixes) so the VS Code
/// companion's autocomplete can never drift from what the validator enforces. Prints the
/// committed, bundled string (no runtime generation), like `cmd_schema`.
pub(crate) fn cmd_vocab() -> ExitCode {
    print!("{}", taliesin_core::vocab::VOCAB_JSON);
    ExitCode::SUCCESS
}

/// The cross-reference-target JSON for a single `.tmd` file (the `symbols` tool's output),
/// reusing `collect_symbols`. Parse-only, no kernel.
pub(crate) fn symbols_json(path: &str) -> Result<String, String> {
    if Path::new(path).is_dir() {
        return Err(format!(
            "symbols expects a single .tmd file, not a directory: {path}"
        ));
    }
    let src = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let base = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
    let doc = crate::serve::guarded(|| taliesin_core::render_document_with_includes(&src, base))
        .map_err(|p| format!("render panicked on {path}: {p}"))?;
    Ok(serde_json::to_string_pretty(&collect_symbols(&doc)).unwrap_or_else(|_| "[]".to_string()))
}

/// The whole-project outline JSON for a directory (the `map` tool's output), reusing
/// `Site::discover` + `build_project_map`. No kernel.
pub(crate) fn map_json(path: &str) -> Result<String, String> {
    if !Path::new(path).is_dir() {
        return Err(format!("map expects a project directory: {path}"));
    }
    let site = taliesin_core::Site::discover(Path::new(path));
    if site.pages.is_empty() {
        return Err(format!("no .tmd pages found under {path}"));
    }
    Ok(
        serde_json::to_string_pretty(&build_project_map(&site))
            .unwrap_or_else(|_| "{}".to_string()),
    )
}

/// The plain-text projection of a single `.tmd` file (the `read` tool's output), reusing
/// `RenderedDoc::body_text`. Parse-only, no kernel.
pub(crate) fn read_text(path: &str) -> Result<String, String> {
    if Path::new(path).is_dir() {
        return Err(format!(
            "read projects a single .tmd file, not a directory: {path}"
        ));
    }
    let src = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let base = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
    crate::serve::guarded(|| taliesin_core::render_document_with_includes(&src, base).body_text())
        .map_err(|p| format!("read panicked on {path}: {p}"))
}

/// One cross-reference target a document defines: the anchor an author writes after `@`.
#[derive(Debug, serde::Serialize)]
struct Symbol {
    id: String,
    /// The kind prefix (`fig`, `sec`, `tbl`, `thm`, …), which decides the rendered label.
    kind: String,
    /// The number the registry resolved, e.g. `3` or `2.1` inside a numbered chapter.
    number: String,
}

/// Every cross-reference target in a rendered document, sorted by id.
///
/// `RenderedDoc::xref_numbers` is the registry `render` builds while numbering figures,
/// tables, sections and theorems, so it already holds *both* shapes an anchor can take:
/// the brace form (`{#sec-why}`) and the cell form (`#| label: fig-scree`). Reading it
/// back is what stops the editor from reimplementing Taliesin's numbering in a regex.
///
/// The registry is a superset of what `@` can name, though: a `::: {.theorem #pythagoras}`
/// is numbered and displayed, yet `cite` only links an anchor whose prefix names a
/// cross-reference kind, so `@pythagoras` stays literal text. `symbols` answers "what can I
/// write after `@`", so those are filtered with `cite`'s own predicate rather than a prefix
/// list copied out here.
fn collect_symbols(doc: &taliesin_core::RenderedDoc) -> Vec<Symbol> {
    let mut out: Vec<Symbol> = doc
        .xref_numbers
        .iter()
        .filter(|(id, _)| taliesin_core::cite::is_xref_anchor(id))
        .map(|(id, number)| Symbol {
            id: id.clone(),
            kind: id.split_once('-').map_or("", |(k, _)| k).to_string(),
            number: number.clone(),
        })
        .collect();
    // `xref_numbers` is a `HashMap`: sort so two runs of `symbols` diff to nothing.
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The whole-project outline `taliesin map` emits: what pages exist, in what order, how
/// they're navigated, and how they cross-reference each other — everything an agent needs
/// to orient in a project in one read-only call. Built from `Site::discover` (no kernel).
#[derive(Debug, serde::Serialize)]
struct ProjectMap {
    title: Option<String>,
    is_book: bool,
    output_dir: String,
    /// The site's canonical `url:` (for absolute links / sitemaps), when configured.
    url: Option<String>,
    /// Pages in nav / chapter order (drafts are excluded, exactly as a build excludes them).
    pages: Vec<PageEntry>,
    nav: NavMap,
    mounts: Vec<MountEntry>,
    /// The cross-reference graph: each anchor → where it's defined + which pages cite it.
    /// A `BTreeMap` so two runs of `map` diff to nothing.
    xref_targets: std::collections::BTreeMap<String, XrefEntry>,
    /// `{{< embed >}}`-referenced decks (built + served, but not pages/nav entries).
    decks: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct PageEntry {
    rel: String,
    url: String,
    title: Option<String>,
    date: Option<String>,
    description: Option<String>,
    categories: Vec<String>,
    page_layout: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct NavItemEntry {
    text: Option<String>,
    href: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct NavMap {
    left: Vec<NavItemEntry>,
    right: Vec<NavItemEntry>,
}

#[derive(Debug, serde::Serialize)]
struct MountEntry {
    at: String,
    path: String,
}

#[derive(Debug, serde::Serialize)]
struct XrefEntry {
    url: String,
    number: String,
    /// Urls of the pages that reference this anchor (the reverse edges), in page order.
    backlinks: Vec<String>,
}

fn build_project_map(site: &taliesin_core::Site) -> ProjectMap {
    let nav_items = |items: &[taliesin_core::site::NavItem]| {
        items
            .iter()
            .map(|n| NavItemEntry {
                text: n.text.clone(),
                href: n.href.clone(),
            })
            .collect()
    };
    let mut xref_targets = std::collections::BTreeMap::new();
    for (anchor, target) in &site.xref_targets {
        xref_targets.insert(
            anchor.clone(),
            XrefEntry {
                url: target.url.clone(),
                number: target.number.clone(),
                backlinks: site.backlinks.get(anchor).cloned().unwrap_or_default(),
            },
        );
    }
    ProjectMap {
        title: site.config.title.clone(),
        is_book: site.is_book(),
        output_dir: site.output_dir().to_string(),
        url: site.config.url.clone(),
        pages: site
            .pages
            .iter()
            .map(|p| PageEntry {
                rel: p.rel.clone(),
                url: p.url.clone(),
                title: p.title.clone(),
                date: p.date.clone(),
                description: p.description.clone(),
                categories: p.categories.clone(),
                page_layout: p.page_layout.clone(),
            })
            .collect(),
        nav: NavMap {
            left: nav_items(&site.config.nav.left),
            right: nav_items(&site.config.nav.right),
        },
        mounts: site
            .config
            .mounts
            .iter()
            .map(|m| MountEntry {
                at: m.at.clone(),
                path: m.path.clone(),
            })
            .collect(),
        xref_targets,
        decks: site.decks.iter().map(|d| d.url.clone()).collect(),
    }
}

/// A compact human rendering of the project map (the JSON is the agent-facing form).
fn map_human(m: &ProjectMap) -> String {
    let mut s = String::new();
    let kind = if m.is_book { "book" } else { "site" };
    s.push_str(&format!(
        "{} ({kind}) → {}\n",
        m.title.as_deref().unwrap_or("(untitled)"),
        m.output_dir
    ));
    s.push_str(&format!("\n{} page(s):\n", m.pages.len()));
    for p in &m.pages {
        s.push_str(&format!(
            "  {:<32}  {}\n",
            p.url,
            p.title.as_deref().unwrap_or("")
        ));
    }
    let nav: Vec<&str> = m
        .nav
        .left
        .iter()
        .chain(m.nav.right.iter())
        .filter_map(|n| n.text.as_deref())
        .collect();
    if !nav.is_empty() {
        s.push_str(&format!("\nnav: {}\n", nav.join(" · ")));
    }
    if !m.mounts.is_empty() {
        s.push_str("\nmounts:\n");
        for mt in &m.mounts {
            s.push_str(&format!("  /{}/ → {}\n", mt.at, mt.path));
        }
    }
    s.push_str(&format!(
        "\n{} cross-reference target(s)\n",
        m.xref_targets.len()
    ));
    s
}

/// Every long flag `map` accepts (drives the unknown-flag did-you-mean).
const MAP_FLAGS: &[&str] = &["--format"];

/// `taliesin map <dir> [--format human|json]`: the whole-project outline in one read-only
/// call (pages in order, nav, mounts, the cross-reference graph, embedded decks). Reuses
/// `Site::discover` — no kernel, no code execution. `map`'s customer is usually an agent
/// orienting in a project; `--format json` is the machine form.
pub(crate) fn cmd_map(args: &[String]) -> ExitCode {
    let mut path: Option<&str> = None;
    let mut format = "human";
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--format" => {
                if let Some(v) = it.next() {
                    format = v;
                }
            }
            s if s.starts_with("--") => {
                log::error(&crate::serve::unknown_flag_error(s, MAP_FLAGS));
                return ExitCode::FAILURE;
            }
            s => {
                if path.is_none() {
                    path = Some(s);
                }
            }
        }
    }
    let Some(path) = path else {
        eprintln!("usage: taliesin map <dir> [--format human|json]");
        return ExitCode::FAILURE;
    };
    if format != "human" && format != "json" {
        log::error(&format!(
            "unknown --format `{format}` (expected human or json)"
        ));
        return ExitCode::FAILURE;
    }
    let target = Path::new(path);
    if !target.is_dir() {
        log::error(&format!(
            "map describes a project directory (an _site.yml + .tmd pages); `{path}` is not a \
             directory. Use `symbols` or `read` for a single file."
        ));
        return ExitCode::FAILURE;
    }
    let site = taliesin_core::Site::discover(target);
    if site.pages.is_empty() {
        log::error(&format!("no .tmd pages found under {path}"));
        return ExitCode::FAILURE;
    }
    let map = build_project_map(&site);
    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&map).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        print!("{}", map_human(&map));
    }
    ExitCode::SUCCESS
}

/// Every long flag `symbols` accepts (drives the unknown-flag did-you-mean).
const SYMBOLS_FLAGS: &[&str] = &["--format"];

/// `taliesin symbols <file.tmd> [--format human|json]`: list the document's
/// cross-reference targets, for an editor's `@`-completion.
///
/// **Parse-only, like `render` and `blocks`.** An editor calls this on a keystroke, so it
/// must never start a kernel. It doesn't need to: a cell's `label:` is registered while
/// the block model is built, long before the cell would run, so a `#| label: fig-scree`
/// figure resolves here with no Python in sight.
pub(crate) fn cmd_symbols(args: &[String]) -> ExitCode {
    let mut path: Option<&str> = None;
    let mut format = "human";
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--format" => {
                if let Some(v) = it.next() {
                    format = v;
                }
            }
            s if s.starts_with("--") => {
                log::error(&crate::serve::unknown_flag_error(s, SYMBOLS_FLAGS));
                return ExitCode::FAILURE;
            }
            s => {
                if path.is_none() {
                    path = Some(s);
                }
            }
        }
    }
    let Some(path) = path else {
        eprintln!("usage: taliesin symbols <file.tmd> [--format human|json]");
        return ExitCode::FAILURE;
    };
    if format != "human" && format != "json" {
        log::error(&format!(
            "unknown --format `{format}` (expected human or json)"
        ));
        return ExitCode::FAILURE;
    }
    if let Some(msg) = directory_rejection(
        path,
        "symbols lists the cross-reference targets of a single .tmd file",
    ) {
        log::error(&msg);
        return ExitCode::FAILURE;
    }
    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) => {
            log::error(&format!("cannot read {path}: {e}"));
            return ExitCode::FAILURE;
        }
    };
    let base = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
    // Guard the render so a panic becomes a clean error + non-zero exit, never a raw
    // abort inside the editor's completion request.
    let doc =
        match crate::serve::guarded(|| taliesin_core::render_document_with_includes(&src, base)) {
            Ok(doc) => doc,
            Err(panic) => {
                log::error(&format!("render panicked on {path}: {panic}"));
                return ExitCode::FAILURE;
            }
        };
    let symbols = collect_symbols(&doc);
    if format == "json" {
        // JSON to stdout only, so it pipes cleanly.
        println!(
            "{}",
            serde_json::to_string_pretty(&symbols).unwrap_or_else(|_| "[]".to_string())
        );
    } else {
        for s in &symbols {
            println!("{:<28}  {:<5}  {}", s.id, s.kind, s.number);
        }
    }
    ExitCode::SUCCESS
}

/// `render` and `blocks` each render a single `.tmd` file. Handed a directory (a project
/// root), `read_to_string` would fail with a bare "Is a directory" OS error; instead build
/// a clear diagnostic that points at the directory-aware subcommands. Returns the message,
/// or `None` when `path` is not a directory (so the caller proceeds to read it). Split out
/// so it's unit-testable without spawning the binary.
fn directory_rejection(path: &str, lead: &str) -> Option<String> {
    Path::new(path).is_dir().then(|| {
        format!(
            "{lead}, but {path} is a directory. For a multi-page project, \
             use `taliesin build {path}` or `taliesin preview {path}`."
        )
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    // `CARGO_MANIFEST_DIR` is always a directory; its `Cargo.toml` is always a file. Using
    // them keeps the test independent of the working directory `cargo test` runs from.
    const A_DIRECTORY: &str = env!("CARGO_MANIFEST_DIR");
    const A_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

    #[test]
    fn a_directory_is_rejected_with_a_helpful_message() {
        let msg = directory_rejection(A_DIRECTORY, "render renders a single .tmd file")
            .expect("a directory must be rejected");
        assert!(msg.contains("is a directory"), "message was: {msg}");
        // The message must steer the user to the directory-aware subcommands.
        assert!(msg.contains("taliesin build"), "message was: {msg}");
        assert!(msg.contains("taliesin preview"), "message was: {msg}");
        // ...and carry the caller's lead clause so render vs blocks reads correctly.
        assert!(
            msg.starts_with("render renders a single .tmd file"),
            "message was: {msg}"
        );
    }

    #[test]
    fn a_regular_file_is_not_rejected() {
        assert!(
            directory_rejection(A_FILE, "blocks lists the block model of a single .tmd file")
                .is_none()
        );
    }
}
