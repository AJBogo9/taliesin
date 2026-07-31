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

pub(crate) fn cmd_render(path: Option<&String>) -> ExitCode {
    let Some(path) = path else {
        return crate::usage_error("render");
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
                let doc = taliesin_core::render_single_doc(&src, base);
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
                    // `render` is a single self-contained page in Build + Inline asset mode,
                    // exactly like `build <file> out.html`, so it hits the one output path that
                    // cannot carry the 12.9 MB Pyodide runtime: `pyodide_index_meta` returns
                    // `""` here and the enhancer has no index URL to boot from. Degrade the same
                    // way `build.rs` does, or the page ships a live `{pyodide}` wrapper whose
                    // only possible outcome is an error box. A no-op (byte-identical) when the
                    // document has no `{pyodide}` cells.
                    let html = taliesin_core::degrade_pyodide_cells(&html);
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
            log::error(&crate::check::cannot_read(Path::new(path), &e));
            ExitCode::FAILURE
        }
    }
}

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

pub(crate) fn cmd_blocks(path: Option<&String>) -> ExitCode {
    let Some(path) = path else {
        return crate::usage_error("blocks");
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
            let doc = match crate::serve::guarded(|| taliesin_core::render_single_doc(&src, base)) {
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
            log::error(&crate::check::cannot_read(Path::new(path), &e));
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
    /// Prose words, from `prose::word_count` — the same selection `lint` and the page's own
    /// reading-time figure use, so `map` cannot report a length the page contradicts.
    words: usize,
    /// The page's heading outline, each carrying the number the rendered page shows.
    /// Numbers exist only after the render's post-passes, so these come from the same
    /// projection `skim` prints rather than from a markdown scan, which would report every
    /// heading unnumbered in a numbered book.
    headings: Vec<MapHeading>,
}

#[derive(Debug, serde::Serialize)]
struct MapHeading {
    id: String,
    level: u8,
    depth: usize,
    text: String,
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
                // Urls only: the reverse index also carries each referrer's citing
                // sentence now, but `map`'s JSON is a machine contract and a page url
                // is what a consumer resolves against `pages`.
                backlinks: site
                    .backlinks
                    .get(anchor)
                    .map(|refs| refs.iter().map(|r| r.url.clone()).collect())
                    .unwrap_or_default(),
            },
        );
    }
    ProjectMap {
        title: site.config.title.clone(),
        is_book: site.is_book(),
        output_dir: site.output_dir().to_string(),
        url: site.config.url.clone(),
        pages: {
            // One projection pass for the whole site, indexed by url: `skim` renders each
            // page, so doing it per-page inside the map would render every page twice.
            let mut proj: std::collections::HashMap<String, taliesin_core::site::skim::PageSkim> =
                site.skim()
                    .into_iter()
                    .map(|p| (p.url.clone(), p))
                    .collect();
            site.pages
                .iter()
                .map(|p| {
                    let s = proj.remove(&p.url);
                    PageEntry {
                        rel: p.rel.clone(),
                        url: p.url.clone(),
                        title: p.title.clone(),
                        date: p.date.clone(),
                        description: p.description.clone(),
                        categories: p.categories.clone(),
                        page_layout: p.page_layout.clone(),
                        words: s.as_ref().map_or(0, |s| s.words),
                        headings: s
                            .as_ref()
                            .map(|s| {
                                s.sections
                                    .iter()
                                    .map(|sec| MapHeading {
                                        id: sec.id.clone(),
                                        level: sec.level,
                                        depth: sec.depth,
                                        text: sec.title.clone(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    }
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

/// Every long flag `skim` accepts (drives the unknown-flag did-you-mean).
const SKIM_FLAGS: &[&str] = &["--format", "--json"];

/// `taliesin skim <dir> [--format human|json]`: the whole book as the layers a reader
/// actually skims — numbered headings, each section's opening sentence, and the captions,
/// callout titles and theorem statements that carry meaning on their own — as one linear
/// stream. Reuses `Site::discover` + `Site::skim`; no kernel, no code execution.
///
/// Its first customer is not a reader but the structural work itself: you cannot calibrate a
/// lint about document structure against a corpus whose shape nobody can see. That is why
/// **every section prints its raw first sentence** and why a judgement (here, "no prose")
/// appears as a visible annotation beside the text rather than replacing it — the moment a
/// weak section and a heuristic misfire render identically, the instrument stops measuring.
pub(crate) fn cmd_skim(args: &[String]) -> ExitCode {
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
                log::error(&crate::serve::unknown_flag_error(s, SKIM_FLAGS));
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
        return crate::usage_error("skim");
    };
    if format != "human" && format != "json" {
        log::error(&crate::serve::bad_format_error(Some(format)));
        return ExitCode::FAILURE;
    }
    let target = Path::new(path);
    if !target.is_dir() {
        log::error(&format!(
            "skim projects a whole project directory (an _site.yml + .tmd pages); `{path}` is \
             not a directory. Use `symbols` or `read` for a single file."
        ));
        return ExitCode::FAILURE;
    }
    let site = taliesin_core::Site::discover(target);
    if site.pages.is_empty() {
        log::error(&format!("no .tmd pages found under {path}"));
        return ExitCode::FAILURE;
    }
    let pages: Vec<SkimPage> = site.skim().iter().map(SkimPage::from).collect();
    if format == "json" {
        let out = SkimDoc {
            title: site.config.title.clone(),
            is_book: site.is_book(),
            words: pages.iter().map(|p| p.words).sum(),
            pages,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        print!("{}", skim_human(&site, &pages));
    }
    ExitCode::SUCCESS
}

#[derive(Debug, serde::Serialize)]
struct SkimDoc {
    title: Option<String>,
    is_book: bool,
    /// Prose words across every page — the same count `lint` and the reading-time figure use.
    words: usize,
    pages: Vec<SkimPage>,
}

#[derive(Debug, serde::Serialize)]
struct SkimPage {
    url: String,
    title: String,
    chapter: Option<u32>,
    words: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    intro: Option<String>,
    sections: Vec<SkimSection>,
}

#[derive(Debug, serde::Serialize)]
struct SkimSection {
    id: String,
    level: u8,
    depth: usize,
    title: String,
    /// `null` when the section has no prose — never omitted, so a consumer can tell "no
    /// prose" from "not projected".
    first_sentence: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    layers: Vec<SkimLayer>,
}

#[derive(Debug, serde::Serialize)]
struct SkimLayer {
    kind: &'static str,
    text: String,
}

impl From<&taliesin_core::site::skim::PageSkim> for SkimPage {
    fn from(p: &taliesin_core::site::skim::PageSkim) -> Self {
        SkimPage {
            url: p.url.clone(),
            title: p.title.clone(),
            chapter: p.chapter,
            words: p.words,
            intro: p.intro.clone(),
            sections: p
                .sections
                .iter()
                .map(|s| SkimSection {
                    id: s.id.clone(),
                    level: s.level,
                    depth: s.depth,
                    title: s.title.clone(),
                    first_sentence: s.first_sentence.clone(),
                    layers: s
                        .layers
                        .iter()
                        .map(|l| SkimLayer {
                            kind: l.kind.tag(),
                            text: l.text.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

/// The linear human stream. Indentation carries structure; the gutter tag carries layer.
fn skim_human(site: &taliesin_core::Site, pages: &[SkimPage]) -> String {
    let mut s = String::new();
    let kind = if site.is_book() { "book" } else { "site" };
    let words: usize = pages.iter().map(|p| p.words).sum();
    s.push_str(&format!(
        "{} ({kind}) — {} page(s), {} words\n",
        site.config.title.as_deref().unwrap_or("(untitled)"),
        pages.len(),
        thousands(words),
    ));
    for p in pages {
        s.push('\n');
        let num = p.chapter.map(|c| format!("{c}  ")).unwrap_or_default();
        s.push_str(&format!(
            "{num}{}  ({}, {} words)\n",
            p.title,
            p.url,
            thousands(p.words)
        ));
        // Annotated, not omitted: a chapter that opens straight onto its first section
        // heading is a structural fact worth seeing, and silence would render it the same
        // as a chapter whose opening the projection simply failed to find.
        match &p.intro {
            Some(intro) => s.push_str(&format!("    ▸ {intro}\n")),
            None => s.push_str("    ▸ (no opening prose)\n"),
        }
        for sec in &p.sections {
            let pad = "  ".repeat(sec.depth + 1);
            s.push_str(&format!("{pad}{}\n", sec.title));
            match &sec.first_sentence {
                Some(t) => s.push_str(&format!("{pad}  ▸ {t}\n")),
                // A visible annotation, never a suppression: an empty section and a section
                // the projection failed on must not look alike.
                None => s.push_str(&format!("{pad}  ▸ (no prose)\n")),
            }
            for l in &sec.layers {
                s.push_str(&format!("{pad}  [{}] {}\n", l.kind, l.text));
            }
        }
    }
    s
}

/// `32600` → `32,600`. The stream is read by a person counting words against a chapter.
fn thousands(n: usize) -> String {
    let d = n.to_string();
    let mut out = String::with_capacity(d.len() + d.len() / 3);
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Every long flag `symbols` accepts (drives the unknown-flag did-you-mean).
const SYMBOLS_FLAGS: &[&str] = &["--format", "--json"];

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
            // `--json`: clig.dev shorthand for `--format json`.
            "--json" => format = "json",
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
        return crate::usage_error("symbols");
    };
    if format != "human" && format != "json" {
        log::error(&crate::serve::bad_format_error(Some(format)));
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
            log::error(&crate::check::cannot_read(Path::new(path), &e));
            return ExitCode::FAILURE;
        }
    };
    let base = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
    // Guard the render so a panic becomes a clean error + non-zero exit, never a raw
    // abort inside the editor's completion request.
    let doc = match crate::serve::guarded(|| taliesin_core::render_single_doc(&src, base)) {
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
