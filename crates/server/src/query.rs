//! Read-only query subcommands: `read`, `map`, `features`, `schema`, `vocab`.
//!
//! **What:** one-shot, side-effect-light commands — `read` projects a document to plain
//! text, `map` outlines a project (or, on a single `.tmd`, that one document's
//! cross-reference targets), `features` inventories what a tree uses, `schema` emits the
//! bundled JSON Schemas for editor autocomplete, and `vocab` emits the bundled editor
//! vocabulary.
//!
//! **How to use:** `main()` dispatches each to the `cmd_*` fns here. The `*_json` fns
//! beside them are the same collections without the CLI wrapper, which is what
//! [`crate::mcp`] serves — including `symbols_json`, whose CLI verb was retired in Wave 5
//! while its MCP tool stayed.
//!
//! **Depends on:** [`taliesin_core`] for rendering + the bundled schemas, and
//! [`crate::log`] for the no-execution warning. No code execution, no kernel — an editor
//! may call `map`/`symbols_json` on a keystroke, so neither may ever start one.

use crate::headless_js::JsOutcome;
// Only the real `observe_js` reaches the module itself; the no-feature stub needs the
// outcome type and nothing else.
#[cfg(feature = "headless-js")]
use crate::headless_js;
use crate::log;
use std::collections::HashMap;
use std::path::Path;
use std::process::ExitCode;
// The enclosing-`_site.yml` walk lives in core: the standalone link checker needs the same
// walk to recognize a link into an enclosing site's `mounts:`, so there is one owner, not two.
use taliesin_core::site::enclosing_site_root;

/// `taliesin read <file.tmd>`: a plain-text projection of the rendered document, for an
/// agent (or a blind author) reading what it made without a browser. Parse-only and
/// static like `render`/`symbols`: it never starts a kernel, so `{python}`/`{r}` cells
/// project as their source with no executed output (warned, like `render`). A VIEW, not an
/// output format.
const READ_FLAGS: &[&str] = &["--run", "--format", "--json"];

pub(crate) fn cmd_read(args: &[String]) -> ExitCode {
    let mut path: Option<&str> = None;
    let mut run = false;
    let mut format = "human";
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--run" => run = true,
            "--format" => {
                if let Some(v) = it.next() {
                    format = v;
                }
            }
            // `--json`: clig.dev shorthand for `--format json`.
            "--json" => format = "json",
            s if s.starts_with("--") => {
                log::error(&crate::serve::unknown_flag_error(s, READ_FLAGS));
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
        return crate::usage_error("read");
    };
    if format != "human" && format != "json" {
        log::error(&crate::serve::bad_format_error(Some(format)));
        return ExitCode::FAILURE;
    }
    // A directory that is a site reads as a whole book; a bare directory keeps the
    // single-file guidance (inside `cmd_read_dir`).
    if Path::new(path).is_dir() {
        return cmd_read_dir(path, format, run);
    }
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            log::error(&crate::check::cannot_read(Path::new(path), &e));
            return ExitCode::FAILURE;
        }
    };
    let p = Path::new(path);
    let base = p.parent().unwrap_or_else(|| Path::new("."));

    // Auto-scope: if this file lives in a site (an enclosing `_site.yml`), render it as the
    // site does (chapter numbering + cross-page refs), so a book chapter reads "Theorem
    // 3.1" / "Chapter 2", not a bare "Theorem". A standalone `.tmd` falls back to the
    // panic-guarded standalone render, exactly as before.
    let mut doc = match scoped_site_doc(p, &src) {
        Some(d) => d,
        None => match crate::serve::guarded(|| taliesin_core::render_single_doc(&src, base)) {
            Ok(d) => d,
            Err(panic) => {
                log::error(&format!("read panicked on {path}: {panic}"));
                return ExitCode::FAILURE;
            }
        },
    };

    // `--run` executes python/r (reusing build's exec path); `--run` under TALIESIN_NO_EXEC
    // never touches a kernel, so for the report it counts as "not executed".
    let executed = run && std::env::var_os("TALIESIN_NO_EXEC").is_none();
    if run {
        let blocks = std::mem::take(&mut doc.blocks);
        doc.blocks = run_cells(blocks, base, p);
    } else {
        // Parse-only: kernel cells (python/r) project as source. Warn so an empty output
        // isn't mistaken for a projection bug.
        let kernel_cells = count_kernel_cells(&doc.blocks);
        if kernel_cells > 0 {
            log::warn(&format!(
                "read does not execute code cells ({kernel_cells} kernel cell{} projected \
                 as source; outputs will be absent). Use `read --run`, `build`, or `preview`.",
                if kernel_cells == 1 { "" } else { "s" }
            ));
        }
    }

    // DX17b: a `{js}` cell (Observable Plot, the corpus's own idiom) runs in the *browser*,
    // so nothing above ever sees whether its chart painted. When executing, drive a local
    // headless Chrome over the built page and observe each `{js}` cell. Gated + optional: no
    // Chrome → every cell reports `skipped (chrome unavailable)`, never a hard failure; a
    // python/r-only doc (no `{js}`) never launches a browser.
    let js_ids = js_cell_ids(&doc.blocks);
    let js_outcomes = if executed && !js_ids.is_empty() {
        observe_js(&doc, &js_ids, p)
    } else {
        HashMap::new()
    };

    if format == "json" {
        print!("{}", read_json(path, &doc, executed, &js_outcomes));
    } else {
        let js_lines: HashMap<String, String> = js_outcomes
            .iter()
            .map(|(id, o)| (id.clone(), o.text_line()))
            .collect();
        print!("{}", doc.body_text_with_js(&js_lines));
    }
    ExitCode::SUCCESS
}

/// One page's projection in a whole-directory read.
struct DirPage {
    rel: String,
    title: Option<String>,
    chapter: Option<u32>,
    text: String,
}

/// `taliesin read <dir>`: project a whole book/site to text, page by page in chapter/nav
/// order, each scoped (chapter numbering + cross-page refs) exactly as a single in-book page
/// read is. Parse-only (whole-book execution is out of scope); `--run` is rejected with a
/// pointer to per-page `--run`.
fn cmd_read_dir(path: &str, format: &str, run: bool) -> ExitCode {
    let dir = Path::new(path);
    // Only a discoverable site (an `_site.yml`) reads as a whole book; a bare directory
    // keeps the single-file guidance and points at `map` for the outline.
    if !dir.join("_site.yml").is_file() {
        log::error(&format!(
            "read projects a .tmd file or a site directory, but {path} has no _site.yml. \
             For a project outline use `taliesin map {path}`; to read one page use \
             `taliesin read {path}/<page>.tmd`."
        ));
        return ExitCode::FAILURE;
    }
    if run {
        log::error(
            "read --run executes one page at a time; run it on a single .tmd file. A \
             whole-directory read is parse-only.",
        );
        return ExitCode::FAILURE;
    }
    let site = taliesin_core::Site::discover_with(dir, taliesin_core::DraftMode::Include);
    if site.pages.is_empty() {
        log::error(&format!("no .tmd pages found under {path}"));
        return ExitCode::FAILURE;
    }
    let mut kernel_cells = 0usize;
    let pages: Vec<DirPage> = site
        .pages
        .iter()
        .filter_map(|page| {
            let src = std::fs::read_to_string(&page.input).ok()?;
            let base = page.input.parent().unwrap_or_else(|| Path::new("."));
            let doc = crate::serve::guarded(|| {
                let mut d = taliesin_core::render_document_scoped_with_site(
                    &src,
                    base,
                    site.chapter_for(page),
                    Some(&site.render_defaults()),
                );
                site.number_chapter(page, &mut d.blocks);
                site.resolve_cross_refs(&mut d.blocks, &page.url);
                d
            })
            .ok()?;
            kernel_cells += count_kernel_cells(&doc.blocks);
            Some(DirPage {
                rel: page.rel.clone(),
                title: page.title.clone(),
                chapter: site.chapter_for(page),
                text: doc.body_text(),
            })
        })
        .collect();
    if kernel_cells > 0 {
        log::warn(&format!(
            "read does not execute code cells ({kernel_cells} kernel cell{} across the book \
             projected as source). Use `build` or `preview` to run them.",
            if kernel_cells == 1 { "" } else { "s" }
        ));
    }
    if format == "json" {
        print!("{}", dir_json(path, &pages));
    } else {
        print!("{}", dir_human(&pages));
    }
    ExitCode::SUCCESS
}

/// The concatenated human projection: each page under a `===== rel (Chapter N) =====`
/// header (the `(Chapter N)` clause only for a numbered chapter), blank-line separated.
fn dir_human(pages: &[DirPage]) -> String {
    let mut out = String::new();
    for p in pages {
        match p.chapter {
            Some(n) => out.push_str(&format!("===== {} (Chapter {n}) =====\n\n", p.rel)),
            None => out.push_str(&format!("===== {} =====\n\n", p.rel)),
        }
        out.push_str(p.text.trim_end());
        out.push_str("\n\n");
    }
    format!("{}\n", out.trim_end())
}

#[derive(serde::Serialize)]
struct ReadDir<'a> {
    path: &'a str,
    pages: Vec<DirPageJson<'a>>,
}

#[derive(serde::Serialize)]
struct DirPageJson<'a> {
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chapter: Option<u32>,
    text: &'a str,
}

/// The machine form of a whole-directory read: `{path, pages:[{path,title,chapter,text}]}`.
fn dir_json(path: &str, pages: &[DirPage]) -> String {
    let out = ReadDir {
        path,
        pages: pages
            .iter()
            .map(|p| DirPageJson {
                path: &p.rel,
                title: p.title.as_deref(),
                chapter: p.chapter,
                text: &p.text,
            })
            .collect(),
    };
    format!(
        "{}\n",
        serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string())
    )
}

/// The block ids of every `{js}` cell in the document, in document order.
fn js_cell_ids(blocks: &[taliesin_core::Block]) -> Vec<String> {
    blocks
        .iter()
        .filter(|b| b.cell.as_ref().is_some_and(|c| c.lang == "js"))
        .map(|b| b.id.clone())
        .collect()
}

/// The same contract as the real [`observe_js`] for a binary built without the browser
/// driver: every cell reports `Skipped`, with a reason naming the rebuild rather than
/// pretending Chrome was missing. `headless-js` is off by default because the driver is
/// **24% of a clean release build** (measured; see `crates/server/Cargo.toml`) and this is
/// its only caller, so most binaries should not carry it.
#[cfg(not(feature = "headless-js"))]
fn observe_js(
    _doc: &taliesin_core::RenderedDoc,
    js_ids: &[String],
    _doc_path: &Path,
) -> HashMap<String, JsOutcome> {
    skip_all_js(
        js_ids,
        "built without headless-js support (rebuild with `--features headless-js`)",
    )
}

/// Observe the document's `{js}` cells headlessly: render the self-contained page they run
/// in to a temp file, then drive a local headless Chrome over it (see [`headless_js`]).
/// Never fails to the caller — no Chrome or any render/IO/launch failure degrades every
/// cell to a `Skipped` outcome. Observation-only: it never writes source or `_freeze`.
#[cfg(feature = "headless-js")]
fn observe_js(
    doc: &taliesin_core::RenderedDoc,
    js_ids: &[String],
    doc_path: &Path,
) -> HashMap<String, JsOutcome> {
    // No Chrome → skip every cell up front, so a Chrome-less run pays ~nothing.
    if !headless_js::chrome_available() {
        return skip_all_js(js_ids, "chrome unavailable");
    }
    // The page the cells actually run in: a self-contained build page (D3/Plot + the tali-js
    // runtime inlined, no network), written to a temp `.html`.
    let stem = doc_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("doc");
    let html = match crate::serve::guarded(|| {
        taliesin_core::render_doc_to_page(doc, stem, taliesin_core::OutputMode::Build)
    }) {
        Ok(h) => h,
        Err(_) => return skip_all_js(js_ids, "page render failed"),
    };
    let tmp = std::env::temp_dir().join(format!(
        "tali-read-js-{}_{}.html",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    if std::fs::write(&tmp, html.as_bytes()).is_err() {
        return skip_all_js(js_ids, "temp write failed");
    }
    let page_path = std::fs::canonicalize(&tmp).unwrap_or_else(|_| tmp.clone());
    let outcomes = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt.block_on(headless_js::observe_js_cells(&page_path, js_ids)),
        Err(_) => skip_all_js(js_ids, "async runtime unavailable"),
    };
    let _ = std::fs::remove_file(&tmp);
    outcomes
}

/// Every `{js}` cell reports the same skip reason (no Chrome / a setup failure).
fn skip_all_js(js_ids: &[String], reason: &str) -> HashMap<String, JsOutcome> {
    js_ids
        .iter()
        .map(|id| (id.clone(), JsOutcome::Skipped(reason.to_string())))
        .collect()
}

fn count_kernel_cells(blocks: &[taliesin_core::Block]) -> usize {
    blocks
        .iter()
        .filter(|b| {
            b.cell
                .as_ref()
                .is_some_and(|c| matches!(c.lang.as_str(), "python" | "r"))
        })
        .count()
}

/// Render a single file the way its enclosing site would: chapter-scoped numbering
/// (`@thm-elbo` → "Theorem 3.1") plus cross-page reference resolution (`@thm-consistency`
/// on another page → "Theorem 2.1"). Returns `None` when the file is not part of a
/// discoverable site (the caller then does today's standalone render). Reuses the exact
/// sequence `site/search.rs::page_fragment` is proven on, plus heading numbering.
fn scoped_site_doc(path: &Path, src: &str) -> Option<taliesin_core::RenderedDoc> {
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let root = enclosing_site_root(base)?;
    let site = taliesin_core::Site::discover_with(&root, taliesin_core::DraftMode::Include);
    let canon = path.canonicalize().ok()?;
    let page = site
        .pages
        .iter()
        .find(|p| p.input.canonicalize().ok().as_deref() == Some(canon.as_path()))?;
    crate::serve::guarded(|| {
        let mut doc = taliesin_core::render_document_scoped_with_site(
            src,
            base,
            site.chapter_for(page),
            Some(&site.render_defaults()),
        );
        site.number_chapter(page, &mut doc.blocks);
        site.resolve_cross_refs(&mut doc.blocks, &page.url);
        doc
    })
    .ok()
}

/// Execute a single doc's cells, mirroring build's single-file exec (no HTML assembly).
/// Takes owned blocks and returns them with output blocks spliced in.
fn run_cells(
    blocks: Vec<taliesin_core::Block>,
    base: &Path,
    doc_path: &Path,
) -> Vec<taliesin_core::Block> {
    let stem = doc_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("doc");
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            log::error(&format!("cannot start async runtime: {e}"));
            return blocks;
        }
    };
    rt.block_on(async {
        let mut ex = crate::exec::Executor::with_freeze(crate::freeze::page_path(
            &base.join("_freeze"),
            stem,
        ))
        .in_dir(base);
        ex.set_interpreters(
            crate::interpreter::resolve_python(None, base),
            crate::interpreter::resolve_r(None, base),
        );
        let out = ex.run(blocks).await;
        // The executor announces a kernel failure itself, once, at the point of failure;
        // re-logging `diagnostic()` here printed the identical fact a second time.
        out
    })
}

#[derive(serde::Serialize)]
struct ReadDoc<'a> {
    path: &'a str,
    executed: bool,
    cells: Vec<CellResult>,
    text: String,
}

#[derive(serde::Serialize)]
struct CellResult {
    id: String,
    lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    produced: bool,
    kind: &'static str,
    /// A `{js}` cell's node kind + dims (`"svg 640×400"`) or a skip reason. `skip_if_none`
    /// so python/r cell JSON stays byte-identical to the pre-DX17b shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fig_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Serialize the per-cell executed-output summary for `read --format json`. An output block
/// (when a cell produced one) immediately follows its cell, so we attach the next
/// `tali-output` block to the last-seen executable cell.
fn read_json(
    path: &str,
    doc: &taliesin_core::RenderedDoc,
    executed: bool,
    js: &HashMap<String, JsOutcome>,
) -> String {
    use taliesin_core::ExecOutput;
    let mut cells = Vec::new();
    let mut pending: Option<(String, String)> = None; // (cell block id, lang)
    for b in &doc.blocks {
        if let Some(c) = &b.cell {
            if let Some((id, lang)) = pending.take() {
                cells.push(empty_or_not_run(id, lang, executed));
            }
            match c.lang.as_str() {
                "python" | "r" => pending = Some((b.id.clone(), c.lang.clone())),
                // A `{js}` cell has no server-side output block; its outcome (if it was
                // observed) comes from the headless browser pass, keyed by block id.
                "js" => {
                    if let Some(outcome) = js.get(&b.id) {
                        cells.push(js_cell_result(b.id.clone(), outcome));
                    }
                }
                _ => {}
            }
            continue;
        }
        if let (Some((id, lang)), Some(kind)) = (
            pending.as_ref(),
            taliesin_core::classify_exec_output(&b.html),
        ) {
            let (id, lang) = (id.clone(), lang.clone());
            pending = None;
            cells.push(match kind {
                ExecOutput::Figure { fig_id, alt } => CellResult {
                    id,
                    lang,
                    label: fig_id.clone(),
                    produced: true,
                    kind: "figure",
                    detail: None,
                    fig_id,
                    alt,
                    error: None,
                },
                ExecOutput::Table { tbl_id } => CellResult {
                    id,
                    lang,
                    label: tbl_id,
                    produced: true,
                    kind: "table",
                    detail: None,
                    fig_id: None,
                    alt: None,
                    error: None,
                },
                ExecOutput::Stream(_) => CellResult {
                    id,
                    lang,
                    label: None,
                    produced: true,
                    kind: "stream",
                    detail: None,
                    fig_id: None,
                    alt: None,
                    error: None,
                },
                ExecOutput::Rich => CellResult {
                    id,
                    lang,
                    label: None,
                    produced: true,
                    kind: "rich",
                    detail: None,
                    fig_id: None,
                    alt: None,
                    error: None,
                },
                ExecOutput::Error(msg) => CellResult {
                    id,
                    lang,
                    label: None,
                    produced: false,
                    kind: "error",
                    detail: None,
                    fig_id: None,
                    alt: None,
                    error: Some(msg),
                },
            });
        }
    }
    if let Some((id, lang)) = pending.take() {
        cells.push(empty_or_not_run(id, lang, executed));
    }
    // The `text` field mirrors the human projection, `[js: …]` lines included, so the two
    // formats never disagree.
    let js_lines: HashMap<String, String> = js
        .iter()
        .map(|(id, o)| (id.clone(), o.text_line()))
        .collect();
    let out = ReadDoc {
        path,
        executed,
        cells,
        text: doc.body_text_with_js(&js_lines),
    };
    format!(
        "{}\n",
        serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string())
    )
}

fn empty_or_not_run(id: String, lang: String, executed: bool) -> CellResult {
    CellResult {
        id,
        lang,
        label: None,
        produced: false,
        kind: if executed { "empty" } else { "not-run" },
        detail: None,
        fig_id: None,
        alt: None,
        error: None,
    }
}

/// A `{js}` cell's JSON entry, projected from its headless observation ([`JsOutcome`]): the
/// `kind`/`produced`/`detail`/`error` fields come straight off the outcome (`js` /
/// `js-error` / `js-empty` / `skipped`).
fn js_cell_result(id: String, outcome: &JsOutcome) -> CellResult {
    CellResult {
        id,
        lang: "js".to_string(),
        label: None,
        produced: outcome.produced(),
        kind: outcome.json_kind(),
        detail: outcome.detail(),
        fig_id: None,
        alt: None,
        error: outcome.error(),
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
    // Optional `--out <dir>` (the output dir), parsed like `cmd_build`.
    let mut out: Option<String> = None;
    let mut it = args.iter().skip(2);
    while let Some(a) = it.next() {
        if a == "--out" {
            out = it.next().cloned();
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
            // The manual step is for editors this repo does not ship a client for. The VS
            // Code companion contributes `yamlValidation` for `_site.yml`, so saying nothing
            // here would leave its users pasting a comment they do not need.
            println!(
                "add `# yaml-language-server: $schema={dir}/tali-site.schema.json` atop _site.yml"
            );
            println!(
                "(not needed in VS Code: the Taliesin companion validates _site.yml already, \
                 given the YAML extension)"
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

/// The `Site` a `map` target describes: a directory is the project rooted there, a single
/// `.tmd` is a project of just that document (scoped before decks, cross-references and the
/// search index are computed, so the map is built from the one page rather than filtered
/// afterwards). One owner, so the CLI verb and the MCP tool cannot answer the same path
/// differently.
/// A target that is neither a directory nor a readable file is a mistyped path, not an
/// empty project. Answering it with "no .tmd pages found under intro.tdm" would drop the
/// near-miss suggestion every other file-taking front door gives (`missing_input_suggests`
/// pins that), so probe it first and report it the same way they do.
fn discover_map_target(target: &Path) -> Result<taliesin_core::Site, String> {
    if target.is_dir() {
        return Ok(taliesin_core::Site::discover(target));
    }
    // `File::open` rather than `exists()`: a path that is there but unreadable deserves the
    // permission error verbatim, not a did-you-mean over its siblings.
    if let Err(e) = std::fs::File::open(target) {
        return Err(crate::check::cannot_read(target, &e));
    }
    Ok(taliesin_core::Site::discover_single(target))
}

/// The cross-reference-target JSON for a single `.tmd` file (the `symbols` tool's output),
/// reusing `collect_symbols`. Parse-only, no kernel.
pub(crate) fn symbols_json(path: &str) -> Result<String, String> {
    if Path::new(path).is_dir() {
        return Err(format!(
            "symbols expects a single .tmd file, not a directory: {path}"
        ));
    }
    let src = std::fs::read_to_string(path)
        .map_err(|e| crate::check::cannot_read(Path::new(path), &e))?;
    let base = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
    let doc = crate::serve::guarded(|| taliesin_core::render_single_doc(&src, base))
        .map_err(|p| format!("render panicked on {path}: {p}"))?;
    Ok(serde_json::to_string_pretty(&collect_symbols(&doc)).unwrap_or_else(|_| "[]".to_string()))
}

/// The whole-project outline JSON for a project or one document (the `map` tool's output),
/// reusing [`discover_map_target`] + `build_project_map`. No kernel.
pub(crate) fn map_json(path: &str) -> Result<String, String> {
    let site = discover_map_target(Path::new(path))?;
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
    let src = std::fs::read_to_string(path)
        .map_err(|e| crate::check::cannot_read(Path::new(path), &e))?;
    let base = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
    crate::serve::guarded(|| taliesin_core::render_single_doc(&src, base).body_text())
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
    /// The cross-reference graph: each anchor → where it's defined.
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
            },
        );
    }
    ProjectMap {
        title: site.config.title.clone(),
        is_book: site.is_book(),
        output_dir: site.output_dir().to_string(),
        url: site.config.url.clone(),
        pages: {
            // A cheap read: every field here is already on the discovered `Page`, so `map`
            // renders nothing. It used to render the whole site for a per-page `words` and
            // `headings`, which had no consumer on either side (retired 2026-08-03).
            site.pages
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
                .collect()
        },
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
const MAP_FLAGS: &[&str] = &["--format", "--json"];

/// `taliesin map <file.tmd | dir> [--format human|json]`: the whole-project outline in one
/// read-only call (pages in order, nav, mounts, the cross-reference graph, embedded decks).
/// Reuses `Site::discover` — no kernel, no code execution. `map`'s customer is usually an
/// agent orienting in a project; `--format json` is the machine form.
///
/// **A single `.tmd` is a project of one document** (`Site::discover_single`, the same
/// scoping `preview <file>` uses), so `map post.tmd` answers "what can I cross-reference
/// in here" — which is the whole of what the retired `symbols` verb did, in the shape a
/// caller already parses for a directory. Parse-only either way: an editor may call this
/// on a keystroke, so it must never start a kernel, and it doesn't need to (a cell's
/// `label:` is registered while the block model is built, long before the cell would run).
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
            // `--json`: clig.dev shorthand for `--format json`.
            "--json" => format = "json",
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
        return crate::usage_error("map");
    };
    if format != "human" && format != "json" {
        log::error(&crate::serve::bad_format_error(Some(format)));
        return ExitCode::FAILURE;
    }
    let target = Path::new(path);
    let site = match discover_map_target(target) {
        Ok(site) => site,
        Err(e) => {
            log::error(&e);
            return ExitCode::FAILURE;
        }
    };
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

/// Every long flag `features` accepts (drives the unknown-flag did-you-mean).
const FEATURES_FLAGS: &[&str] = &["--format", "--json"];

/// `taliesin features <file.tmd | dir> [--format human|json]`: which constructs a document
/// uses, and which constructs no document uses.
///
/// **The shape follows the target, deliberately, instead of taking a flag.** A directory is
/// an inventory, so it reports feature-first (the adoption table, zero rows included); a
/// single file is the question "what does this document use", so it reports document-first.
/// The JSON is the SAME shape either way (a single file is a one-document project), so a
/// consumer never has to branch on which target it was given.
///
/// **Unlike `read`/`map`/`skim` this accepts a bare directory**, and the divergence is the
/// point: those project a document for a reader and want a project's page order, this
/// inventories a tree for an auditor. `corpus/` is the single most useful target and has no
/// `_site.yml` at its root, so requiring one would fail on the exact question the command
/// exists to answer. When the directory IS a project, page order and draft handling come
/// from `Site::discover` so the report matches what a build would produce.
///
/// A report, never a gate: a successful scan exits 0 whatever it found. Parse-only, no
/// kernel, no render.
pub(crate) fn cmd_features(args: &[String]) -> ExitCode {
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
            "--json" => format = "json",
            s if s.starts_with("--") => {
                log::error(&crate::serve::unknown_flag_error(s, FEATURES_FLAGS));
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
        return crate::usage_error("features");
    };
    if format != "human" && format != "json" {
        log::error(&crate::serve::bad_format_error(Some(format)));
        return ExitCode::FAILURE;
    }
    let target = Path::new(path);
    let docs = match collect_feature_docs(target) {
        Ok(d) => d,
        Err(msg) => {
            log::error(&msg);
            return ExitCode::FAILURE;
        }
    };
    if docs.is_empty() {
        log::error(&format!("no .tmd documents found under {path}"));
        return ExitCode::FAILURE;
    }
    let single_file = target.is_file();
    let adoption = taliesin_core::features::Adoption::build(&docs);
    if format == "json" {
        println!("{}", features_json(path, &adoption));
    } else if single_file {
        print!("{}", features_document_human(&docs[0].0, &docs[0].1));
    } else {
        print!("{}", features_table_human(path, &adoption));
    }
    ExitCode::SUCCESS
}

/// The documents `features` will report, as `(label, scan)`.
///
/// A project reports in `chapters:`/nav order with drafts handled as a build handles them;
/// a bare directory walks in sorted path order. A single `.tmd` file is a one-document
/// project. Labels are relative to the target so the output is stable wherever it is run.
fn collect_feature_docs(
    target: &Path,
) -> Result<Vec<(String, taliesin_core::features::DocFeatures)>, String> {
    let read = |p: &Path| -> Result<String, String> {
        std::fs::read_to_string(p).map_err(|e| crate::check::cannot_read(p, &e))
    };
    if target.is_file() {
        let label = target.to_string_lossy().to_string();
        return Ok(vec![(label, taliesin_core::features::scan(&read(target)?))]);
    }
    if !target.is_dir() {
        return Err(format!(
            "features: no such file or directory: {}",
            target.display()
        ));
    }
    // A project: use the site's own page order + draft policy, so the report and a build
    // agree on which documents exist. `DraftMode::Include` because a draft is still a
    // document whose feature use is real; excluding it would hide adoption mid-write.
    if target.join("_site.yml").is_file() {
        let site = taliesin_core::Site::discover_with(target, taliesin_core::DraftMode::Include);
        if !site.pages.is_empty() {
            return Ok(site
                .pages
                .iter()
                .filter_map(|p| {
                    let src = std::fs::read_to_string(&p.input).ok()?;
                    Some((p.rel.clone(), taliesin_core::features::scan(&src)))
                })
                .collect());
        }
    }
    let mut paths = Vec::new();
    walk_tmd(target, &mut paths);
    paths.sort();
    Ok(paths
        .iter()
        .filter_map(|p| {
            let src = std::fs::read_to_string(p).ok()?;
            let label = p
                .strip_prefix(target)
                .unwrap_or(p)
                .to_string_lossy()
                .to_string();
            Some((label, taliesin_core::features::scan(&src)))
        })
        .collect())
}

/// Every `.tmd` under `dir`, skipping build output and version-control noise. Build outputs
/// hold generated copies of the very documents being counted, so walking them would double
/// every number in the report.
fn walk_tmd(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    const SKIP: &[&str] = &["_site", "_freeze", "target", "node_modules", ".git"];
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if !name.starts_with('.') && !SKIP.contains(&name.as_str()) {
                walk_tmd(&p, out);
            }
        } else if p.extension().and_then(|s| s.to_str()) == Some("tmd") {
            out.push(p);
        }
    }
}

/// The adoption table: every group, every catalogued construct, most-used first.
///
/// **A construct used by three or fewer documents names them inline.** That tail is the
/// half the report exists for (a feature with one user is the question corpus-plus-roadmap
/// asks), and needing a second command to see which document it was would put the answer
/// one step further away than the problem deserves. Above three, the count alone; `--json`
/// always carries the full list.
fn features_table_human(path: &str, a: &taliesin_core::features::Adoption) -> String {
    let mut s = format!("{path}, {} document(s)\n", a.documents);
    for g in &a.groups {
        s.push_str(&format!(
            "\n{:<30} {} known · {} used · {} unused\n",
            g.name,
            g.known(),
            g.used(),
            g.unused()
        ));
        let mut features: Vec<&taliesin_core::features::FeatureAdoption> =
            g.features.iter().collect();
        features.sort_by(|x, y| {
            y.documents
                .len()
                .cmp(&x.documents.len())
                .then_with(|| x.name.cmp(&y.name))
        });
        for f in features {
            let n = f.documents.len();
            let detail = match n {
                0 => "  (no document)".to_string(),
                1..=3 => format!("  {}", f.documents.join(", ")),
                _ => String::new(),
            };
            s.push_str(&format!("  {:<28} {:>4}{detail}\n", f.name, n));
        }
    }
    let (unused, known) = a.unused_totals();
    s.push_str(&format!(
        "\n{unused} of {known} features are used by no document\n"
    ));
    s
}

/// The single-file view: what this one document uses, group by group. Groups it does not
/// touch are omitted, because the denominator is a property of the tool, not of one page.
fn features_document_human(label: &str, f: &taliesin_core::features::DocFeatures) -> String {
    let mut s = format!("{label}\n");
    for g in taliesin_core::features::catalogue() {
        let Some(used) = f.used.get(g.slug) else {
            continue;
        };
        let names: Vec<&str> = used.iter().map(String::as_str).collect();
        s.push_str(&format!("\n  {:<24} {}\n", g.name, names.join(", ")));
    }
    s.push_str(&format!("\n{} feature(s) used\n", f.count()));
    s
}

#[derive(serde::Serialize)]
struct FeaturesJson<'a> {
    path: &'a str,
    documents: usize,
    groups: Vec<FeatureGroupJson<'a>>,
}

#[derive(serde::Serialize)]
struct FeatureGroupJson<'a> {
    name: &'a str,
    slug: &'a str,
    known: usize,
    used: usize,
    unused: usize,
    features: Vec<FeatureJson<'a>>,
}

#[derive(serde::Serialize)]
struct FeatureJson<'a> {
    name: &'a str,
    /// Always present, empty for an unused feature: a consumer must be able to tell "no
    /// document uses this" from "this is not a feature", and an omitted key cannot.
    documents: &'a [String],
}

/// The machine form. Feature-first, which a consumer can invert to document-first; there is
/// deliberately no second flag for the inverse.
fn features_json(path: &str, a: &taliesin_core::features::Adoption) -> String {
    let out = FeaturesJson {
        path,
        documents: a.documents,
        groups: a
            .groups
            .iter()
            .map(|g| FeatureGroupJson {
                name: g.name,
                slug: g.slug,
                known: g.known(),
                used: g.used(),
                unused: g.unused(),
                features: g
                    .features
                    .iter()
                    .map(|f| FeatureJson {
                        name: &f.name,
                        documents: &f.documents,
                    })
                    .collect(),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string())
}
