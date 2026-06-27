//! qmd-fast — dev server & CLI entry point.
//!
//!   - `qmd-fast preview <file.qmd> [port]` live preview server (aliases: dev, serve)
//!   - `qmd-fast build  <file.qmd> [out]`   render a self-contained HTML file
//!   - `qmd-fast render <file.qmd>`         one-shot full HTML page to stdout
//!   - `qmd-fast blocks <file.qmd>`         list block ids + sourcepos (debugging)

mod exec;
mod freeze;
mod kernel;
mod log;
mod protocol;
mod serve;
mod serve_site;
#[cfg(test)]
mod testutil;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("render") => cmd_render(args.get(2)),
        Some("build") => cmd_build(&args),
        Some("blocks") => cmd_blocks(args.get(2)),
        Some("schema") => cmd_schema(&args),
        Some("check") => cmd_check(&args),
        // `preview`/`dev` are vite-style aliases for the live server.
        Some("serve" | "preview" | "dev") => cmd_serve(&args),
        Some("--version" | "-V") => {
            println!(
                "qmd-fast {} ({})",
                qmd_fast_core::VERSION,
                env!("QMD_FAST_GIT_SHA")
            );
            ExitCode::SUCCESS
        }
        // No command, or an explicit help request: print usage and succeed.
        Some("--help" | "-h" | "help") | None => {
            usage();
            ExitCode::SUCCESS
        }
        // An unrecognized command is an error (non-zero), not a silent success.
        Some(other) => {
            log::error(&format!("unknown command: `{other}`"));
            usage();
            ExitCode::FAILURE
        }
    }
}

fn cmd_serve(args: &[String]) -> ExitCode {
    // Positionals are <file.qmd> [port]; flags (--open, --host) may appear anywhere.
    let positionals: Vec<&String> = args[2..].iter().filter(|a| !a.starts_with("--")).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let open = flag("--open") || std::env::var_os("QMD_FAST_OPEN").is_some();
    let expose = flag("--host") || std::env::var_os("QMD_FAST_HOST").is_some();
    // `--no-exec` is sugar for `QMD_FAST_NO_EXEC=1`, which `exec::Executor` reads:
    // preview a document you don't trust without running its code cells.
    if flag("--no-exec") {
        // SAFETY: set once at CLI startup, before the tokio runtime / kernel
        // threads spawn, so no other thread is touching the environment.
        unsafe { std::env::set_var("QMD_FAST_NO_EXEC", "1") };
    }
    let Some(path) = positionals.first() else {
        eprintln!("usage: qmd-fast preview <file.qmd|dir> [port] [--host] [--open] [--no-exec]");
        return ExitCode::FAILURE;
    };
    // The optional second positional is the port; a present-but-unparseable value
    // is an error rather than a silent fall-back to the default.
    let port: u16 = match positionals.get(1) {
        None => 4321,
        Some(p) => match p.parse() {
            Ok(n) => n,
            Err(_) => {
                log::error(&format!("invalid port: `{p}` (expected 0-65535)"));
                return ExitCode::FAILURE;
            }
        },
    };
    // A directory is a multi-page site project; a single `.qmd` is one document.
    let result = if Path::new(path.as_str()).is_dir() {
        serve_site::run(PathBuf::from(path), port, open, expose)
    } else {
        serve::run(PathBuf::from(path), port, open, expose)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error(&format!("serve: {e}"));
            ExitCode::FAILURE
        }
    }
}

/// `build <file.qmd> [out.html]`: write a self-contained HTML page to a file
/// (default `<stem>.html` beside the source). With `--out <dir>` it instead
/// writes `<dir>/index.html` and copies every referenced local asset alongside
/// (paths preserved), so the directory is deployable as-is. `render` is stdout.
/// One located diagnostic from the render warning channel, ready to print or serialize.
#[derive(Debug, Clone, serde::Serialize)]
struct Diagnostic {
    file: String,
    line: Option<u32>,
    message: String,
}

fn diag_from(w: &qmd_fast_core::render::Warning, fallback_file: &str) -> Diagnostic {
    Diagnostic {
        file: w.file.clone().unwrap_or_else(|| fallback_file.to_string()),
        line: w.line,
        message: w.message.clone(),
    }
}

/// Render `path` (a file or a site directory) in memory and return every located
/// diagnostic. No code execution, no output written. `Err` for an unreadable file or
/// an empty site.
fn collect_diagnostics(path: &Path) -> Result<Vec<Diagnostic>, String> {
    if path.is_dir() {
        collect_site_diagnostics(path)
    } else {
        collect_file_diagnostics(path)
    }
}

fn collect_file_diagnostics(path: &Path) -> Result<Vec<Diagnostic>, String> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let doc = qmd_fast_core::render_document_with_includes(&src, base);
    let path_str = path.display().to_string();
    use qmd_fast_core::diagnostics as dx;
    let xref = qmd_fast_core::cite::validate_xrefs(&doc.blocks);
    let dups = dx::validate_duplicate_heading_ids(&doc.blocks);
    let anchors = dx::validate_internal_anchors(&doc.blocks);
    let assets = dx::validate_local_assets(&doc.blocks, base);
    let cites = dx::citations_without_bibliography(&src, &doc.blocks);
    let mut out: Vec<Diagnostic> = Vec::new();
    // Malformed YAML front matter: the lenient line-parser silently mis-extracts
    // fields, so surface the parse error here too (the live servers already do).
    if let Some((message, line)) = qmd_fast_core::frontmatter::yaml_error(&src) {
        out.push(Diagnostic {
            file: path_str.clone(),
            line: Some(line),
            message,
        });
    }
    out.extend(
        doc.warnings
            .iter()
            .chain(xref.iter())
            .chain(dups.iter())
            .chain(anchors.iter())
            .chain(assets.iter())
            .chain(cites.iter())
            .map(|w| diag_from(w, &path_str)),
    );
    Ok(out)
}

fn collect_site_diagnostics(root: &Path) -> Result<Vec<Diagnostic>, String> {
    let site = qmd_fast_core::Site::discover(root);
    if site.pages.is_empty() {
        return Err(format!("no .qmd pages found under {}", root.display()));
    }
    let mut out: Vec<Diagnostic> = site
        .warnings
        .iter()
        .map(|m| Diagnostic {
            file: "_site.yml".to_string(),
            line: None,
            message: m.clone(),
        })
        .collect();
    for page in &site.pages {
        let Ok(src) = std::fs::read_to_string(&page.input) else {
            out.push(Diagnostic {
                file: page.rel.clone(),
                line: None,
                message: format!("cannot read {}", page.input.display()),
            });
            continue;
        };
        if let Some((message, line)) = qmd_fast_core::frontmatter::yaml_error(&src) {
            out.push(Diagnostic {
                file: page.rel.clone(),
                line: Some(line),
                message,
            });
        }
        let base = page.input.parent().unwrap_or(root);
        let doc = qmd_fast_core::render_document_with_includes(&src, base);
        // Static lints over the page's blocks (xrefs are added by render_page_doc_warned
        // below); run before `doc` is consumed.
        use qmd_fast_core::diagnostics as dx;
        let dups = dx::validate_duplicate_heading_ids(&doc.blocks);
        let anchors = dx::validate_internal_anchors(&doc.blocks);
        let assets = dx::validate_local_assets(&doc.blocks, base);
        let cites = dx::citations_without_bibliography(&src, &doc.blocks);
        for w in dups
            .iter()
            .chain(anchors.iter())
            .chain(assets.iter())
            .chain(cites.iter())
        {
            out.push(diag_from(w, &page.rel));
        }
        let (_html, warnings) = site.render_page_doc_warned(page, doc);
        for w in &warnings {
            out.push(diag_from(w, &page.rel));
        }
    }
    Ok(out)
}

fn format_json(diags: &[Diagnostic]) -> String {
    serde_json::to_string_pretty(diags).unwrap_or_else(|_| "[]".to_string())
}

fn format_human(diags: &[Diagnostic]) -> String {
    let mut s = String::new();
    for d in diags {
        match d.line {
            Some(l) => s.push_str(&format!("{}:{}: {}\n", d.file, l, d.message)),
            None => s.push_str(&format!("{}: {}\n", d.file, d.message)),
        }
    }
    s
}

/// `qmd-fast check <file|dir> [--format human|json]`: render in memory, list every
/// located diagnostic, and exit non-zero if any are found (a CI gate). Static-only
/// (no code execution).
fn cmd_check(args: &[String]) -> ExitCode {
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
            s if s.starts_with("--") => {}
            s => {
                if path.is_none() {
                    path = Some(s);
                }
            }
        }
    }
    let Some(path) = path else {
        eprintln!("usage: qmd-fast check <file.qmd|dir> [--format human|json]");
        return ExitCode::FAILURE;
    };
    if format != "human" && format != "json" {
        log::error(&format!(
            "unknown --format `{format}` (expected human or json)"
        ));
        return ExitCode::FAILURE;
    }
    let diags = match collect_diagnostics(Path::new(path)) {
        Ok(d) => d,
        Err(e) => {
            log::error(&e);
            return ExitCode::FAILURE;
        }
    };
    if format == "json" {
        // JSON to stdout only, so it pipes cleanly.
        println!("{}", format_json(&diags));
    } else {
        // Greppable `path:line: message` lines to stderr (linter-style), then a summary.
        eprint!("{}", format_human(&diags));
        if diags.is_empty() {
            eprintln!("no problems found");
        } else {
            eprintln!(
                "{} problem{}",
                diags.len(),
                if diags.len() == 1 { "" } else { "s" }
            );
        }
    }
    if diags.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn cmd_build(args: &[String]) -> ExitCode {
    // Positionals: <file> [out.html]. Flags: `--out <dir>` (alias `--dir`),
    // `--strict` (a cell error / broken-ref warning fails the build).
    let mut positionals: Vec<&str> = Vec::new();
    let mut out_dir: Option<&str> = None;
    let mut strict = false;
    let mut bare = false;
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // Take the next token as the value, but not if it's itself a flag (so
            // `--out --open` doesn't silently swallow `--open` as the directory).
            "--out" | "--dir" => {
                out_dir = it
                    .next()
                    .map(|s| s.as_str())
                    .filter(|s| !s.starts_with("--"));
            }
            "--strict" => strict = true,
            // `--bare`: zero-`<script>`, zero-CDN, CSS-only-theme single-doc output.
            "--bare" => bare = true,
            s if s.starts_with("--") => {}
            s => positionals.push(s),
        }
    }
    let Some(path) = positionals.first().copied() else {
        eprintln!(
            "usage: qmd-fast build <file.qmd|dir> [out.html] [--out <dir>] [--strict] [--bare]"
        );
        return ExitCode::FAILURE;
    };
    // A directory is a multi-page site project (`_site.yml` + `.qmd` pages);
    // a single `.qmd` keeps the original self-contained-page behaviour.
    if Path::new(path).is_dir() {
        if bare {
            log::error(
                "--bare builds a single document, not a site (a site's navigation + \
                 search need JavaScript)",
            );
            return ExitCode::FAILURE;
        }
        return build_site(Path::new(path), out_dir, strict);
    }
    let mode = if bare {
        qmd_fast_core::OutputMode::Bare
    } else {
        qmd_fast_core::OutputMode::Build
    };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            log::error(&format!("cannot read {path}: {e}"));
            return ExitCode::FAILURE;
        }
    };
    let p = Path::new(path);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    let base = p.parent().unwrap_or_else(|| Path::new("."));
    let (html, resources, problems) = match build_page_executing(&src, base, stem, mode) {
        Ok(BuildResult::Page {
            html,
            resources,
            problems,
        }) => (html, resources, problems),
        // `--bare` refused (e.g. a slide deck): the message is already user-facing.
        Ok(BuildResult::Refused(msg)) => {
            log::error(&msg);
            return ExitCode::FAILURE;
        }
        Err(e) => {
            log::error(&format!("cannot start runtime: {e}"));
            return ExitCode::FAILURE;
        }
    };

    // In `--strict` mode, a cell that crashed (its traceback is baked into the HTML)
    // or any located warning fails the build instead of shipping a broken page with
    // exit 0. Without `--strict` the warnings were already logged; we still write.
    let strict_fail = strict && problems > 0;

    if let Some(dir) = out_dir {
        let code = build_dir(&html, base, Path::new(dir));
        copy_resources(&resources, Path::new(dir));
        return strict_exit(code, strict_fail, problems);
    }
    let out: PathBuf = positionals
        .get(1)
        .map(|&s| PathBuf::from(s))
        .unwrap_or_else(|| base.join(format!("{stem}.html")));
    match std::fs::write(&out, &html) {
        Ok(()) => {
            let dest = out.parent().unwrap_or(base);
            copy_resources(&resources, dest);
            // Bundle the doc's own referenced assets (images, audio, …) next to the
            // page too, so `build doc.qmd out.html` into another directory doesn't
            // leave them dangling. A no-op for an in-place build.
            copy_local_assets(&html, base, dest);
            log::built(&out.display().to_string());
            strict_exit(ExitCode::SUCCESS, strict_fail, problems)
        }
        Err(e) => {
            log::error(&format!("cannot write {}: {e}", out.display()));
            ExitCode::FAILURE
        }
    }
}

/// Turn a successful build into a failure when `--strict` saw problems (the page is
/// still written, but CI gets a non-zero exit). A pre-existing non-success `code`
/// (a write/create error) is returned unchanged.
fn strict_exit(code: ExitCode, strict_fail: bool, problems: usize) -> ExitCode {
    if strict_fail {
        log::error(&format!(
            "--strict: {problems} problem{} (cell error or located warning); failing the build",
            if problems == 1 { "" } else { "s" }
        ));
        return ExitCode::FAILURE;
    }
    code
}

/// Count the executed output blocks that are uncaught runtime errors (their HTML
/// carries the `qmd-error` marker), logging a located warning per failing cell so a
/// crashing cell isn't baked into the build silently. Returns the count.
fn report_cell_errors(blocks: &[qmd_fast_core::Block], page_label: &str) -> usize {
    let mut n = 0;
    for b in blocks {
        if b.html.contains("class=\"qmd-error\"") {
            n += 1;
            let where_ = b
                .source_file
                .as_deref()
                .map(|f| format!("{f} "))
                .unwrap_or_default();
            log::warn(&format!(
                "cell error in {page_label} ({where_}@ {}): code cell raised an uncaught \
                 exception; its traceback is baked into the output",
                b.sourcepos
            ));
        }
    }
    n
}

/// Render a single document to a self-contained HTML page, executing its code
/// cells first so figures / `ojs_define` outputs are baked in (mirrors the site
/// build's per-page execution). A missing kernel logs a warning and the cells fall
/// back to source, matching the preview's behaviour.
/// Result of building a single page: the rendered HTML (+ its referenced resources
/// and `--strict` problem count), or a `--bare` refusal whose message is user-facing.
enum BuildResult {
    Page {
        html: String,
        resources: Vec<PathBuf>,
        problems: usize,
    },
    Refused(String),
}

/// Warn (never silently degrade) about the constructs `--bare` drops: a `{js}` cell
/// is inert without its browser runtime, and Mermaid ships its diagram as source.
fn warn_bare_exclusions(doc: &qmd_fast_core::RenderedDoc) {
    let js_cells = doc
        .blocks
        .iter()
        .filter(|b| b.cell.as_ref().is_some_and(|c| c.lang == "js"))
        .count();
    if js_cells > 0 {
        log::warn(&format!(
            "--bare drops {js_cells} interactive {{js}} cell{} (no browser runtime ships); \
             the output container is left empty",
            if js_cells == 1 { "" } else { "s" }
        ));
    }
    let mermaid = doc
        .blocks
        .iter()
        .filter(|b| b.html.contains("class=\"mermaid\""))
        .count();
    if mermaid > 0 {
        log::warn(&format!(
            "--bare shows {mermaid} Mermaid diagram{} as source (no renderer ships)",
            if mermaid == 1 { "" } else { "s" }
        ));
    }
}

fn build_page_executing(
    src: &str,
    base: &Path,
    fallback: &str,
    mode: qmd_fast_core::OutputMode,
) -> std::io::Result<BuildResult> {
    let rt = tokio::runtime::Runtime::new()?;
    Ok(rt.block_on(async {
        // `problems` is what `--strict` fails on: located render warnings, broken
        // cross-refs, and crashed code cells — each already logged below.
        let mut problems = 0usize;
        let mut doc = qmd_fast_core::render_document_with_includes(src, base);
        // `--bare` is prose-shaped, JS-free output: a slide deck (whose navigation is
        // JavaScript) can't be one. Refuse before doing any execution work.
        if mode == qmd_fast_core::OutputMode::Bare && doc.format == qmd_fast_core::DocFormat::Reveal
        {
            return BuildResult::Refused(
                "--bare cannot build a slide deck: deck navigation needs JavaScript. \
                 Build it without --bare."
                    .to_string(),
            );
        }
        // Located render warnings (front-matter typos, broken refs, and now
        // unresolved `{{< include … >}}` directives — the path-resolution channel)
        // are logged here so a `build` never ships a silently dropped include.
        for w in &doc.warnings {
            log::warn(&w.message);
        }
        problems += doc.warnings.len();
        // `{{< embed >}}` only resolves in a SITE build, which also builds the
        // embedded target beside the page. A single-doc build ships the iframe but
        // not its target, so the embed would 404 — warn instead of failing silently.
        for target in qmd_fast_core::render::embed_targets(src) {
            log::warn(&format!(
                "{{{{< embed {target} >}}}} won't resolve in a single-doc build (its \
                 target isn't built); build the containing directory as a site, or \
                 inline the content instead."
            ));
        }
        // Broken cross-refs (a single doc has no site to resolve them across pages),
        // so a `build` doesn't ship a dangling `@fig-`/`@sec-` link silently.
        let xrefs = qmd_fast_core::cite::validate_xrefs(&doc.blocks);
        for w in &xrefs {
            log::warn(&w.message);
        }
        problems += xrefs.len();
        // Persistent execution cache keyed off the doc's stem, beside the source.
        let mut ex =
            exec::Executor::with_freeze(freeze::page_path(&base.join("_freeze"), fallback))
                .in_dir(base);
        doc.blocks = ex.run(std::mem::take(&mut doc.blocks)).await;
        if ex.diagnostic().is_some() {
            log::warn(
                "kernel unavailable; code cells emitted as source \
                 (set QMD_FAST_PYTHON to a python with ipykernel)",
            );
        }
        // A crashed cell bakes its traceback into the page (exit 0 + silent stderr
        // before this); log it located and count it toward `--strict`.
        problems += report_cell_errors(&doc.blocks, fallback);
        if mode == qmd_fast_core::OutputMode::Bare {
            warn_bare_exclusions(&doc);
        }
        let resources = doc.includes.resources.clone();
        BuildResult::Page {
            html: qmd_fast_core::render_doc_to_page(&doc, fallback, mode),
            resources,
            problems,
        }
    }))
}

/// Copy a format extension's `format-resources` (a reveal plugin's `.js`, etc.)
/// next to the output page by file name, so an injected `<script src="x.js">`
/// resolves. Skips silently when there are none.
fn copy_resources(resources: &[PathBuf], dest_dir: &Path) {
    for r in resources {
        let Some(name) = r.file_name() else { continue };
        let dest = dest_dir.join(name);
        // Don't copy a resource onto itself (fs::copy truncates the dest first).
        if same_file(r, &dest) {
            continue;
        }
        if let Err(e) = std::fs::copy(r, &dest) {
            log::warn(&format!("cannot copy resource {}: {e}", r.display()));
        }
    }
}

/// Write `<dir>/index.html` and copy each referenced local asset (an `src=`/
/// `href=` value pointing to an existing file under `base`) to the same relative
/// path under `dir`, leaving the HTML's paths untouched so the folder is portable.
fn build_dir(html: &str, base: &Path, dir: &Path) -> ExitCode {
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::error(&format!("cannot create {}: {e}", dir.display()));
        return ExitCode::FAILURE;
    }
    let copied = copy_local_assets(html, base, dir);
    let index = dir.join("index.html");
    if let Err(e) = std::fs::write(&index, html) {
        log::error(&format!("cannot write {}: {e}", index.display()));
        return ExitCode::FAILURE;
    }
    log::built(&format!(
        "{}  ·  {copied} asset{}",
        index.display(),
        if copied == 1 { "" } else { "s" }
    ));
    ExitCode::SUCCESS
}

/// Copy each referenced local asset (a relative `src=`/`href=` under `base`) to
/// the same relative path under `dest`, so a built page's images/audio/etc. travel
/// with it. Skips paths escaping the tree (absolute or `..`) and no-op self-copies
/// (an in-place build, where the asset already sits next to the output). Returns
/// the number copied. Shared by the portable `--out` folder and the single-file
/// build (so `build doc.qmd out.html` into another directory isn't left with
/// dangling asset references).
fn copy_local_assets(html: &str, base: &Path, dest: &Path) -> usize {
    let mut copied = 0usize;
    for r in local_refs(html) {
        // The filesystem path is the ref without any ?query / #fragment (a static
        // host ignores those, so `img.png?v=2` is the file `img.png`).
        let path = &r[..r.find(['?', '#']).unwrap_or(r.len())];
        if path.starts_with('/') || path.split('/').any(|seg| seg == "..") {
            log::warn(&format!("asset outside the doc tree, not bundled: {r}"));
            continue;
        }
        let from = base.join(path);
        if !from.is_file() {
            continue; // e.g. an href to something that isn't a local file
        }
        let to = dest.join(path);
        // In-place build: the asset is already where the page points, and copying a
        // file onto itself would truncate it.
        if same_file(&from, &to) {
            continue;
        }
        if let Some(parent) = to.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::copy(&from, &to) {
            Ok(_) => copied += 1,
            Err(e) => log::warn(&format!("cannot copy {}: {e}", from.display())),
        }
    }
    copied + copy_js_imports(html, base, dest)
}

/// Whether two paths resolve to the same file on disk (so we don't self-copy).
fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

/// Bodies of the `<script type="application/qmd-js">…</script>` cells in `html` (the
/// author's `{js}` source, where relative `import()`/`fetch()` specifiers live —
/// invisible to the `src=`/`href=` scan). `</script` is server-escaped in the source, so
/// the next `</script>` reliably ends the body.
fn qmd_js_cell_sources(html: &str) -> Vec<&str> {
    let needle = "type=\"application/qmd-js\"";
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find(needle) {
        let tag = i + pos;
        let Some(gt) = html[tag..].find('>') else {
            break;
        };
        let body_start = tag + gt + 1;
        let Some(end) = html[body_start..].find("</script>") else {
            break;
        };
        out.push(&html[body_start..body_start + end]);
        i = body_start + end + "</script>".len();
    }
    out
}

/// Every quoted string literal in `src` whose value starts with `./` or `../` — the
/// relative files a `{js}` cell (or a copied module) imports/fetches. Quote-escaping is
/// not handled (module specifiers don't contain escaped quotes), matching `local_refs`.
fn relative_specifiers(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let q = bytes[i];
        if (q == b'"' || q == b'\'')
            && let Some(end) = src[i + 1..].find(q as char)
        {
            let val = &src[i + 1..i + 1 + end];
            if val.starts_with("./") || val.starts_with("../") {
                out.push(val.to_string());
            }
            i += 1 + end + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// Resolve a relative `spec` (from a file whose dir, relative to the doc base, is `dir`)
/// to a normalized base-relative path, collapsing `.`/`..`. `None` if it escapes the base
/// tree (a `..` above the root, or an absolute path).
fn normalize_rel(dir: &str, spec: &str) -> Option<String> {
    if spec.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for seg in spec.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

/// Bundle the local files a `{js}` cell imports/fetches via relative specifiers, which the
/// `src=`/`href=` scan can't see. Resolves against the doc `base`, copies to the same
/// relative path under `dest`, and follows the chain through copied `.js`/`.mjs` modules
/// (each specifier resolved against its own dir). Remote (`https://…`) and bare specifiers
/// are ignored; tree-escaping ones warn. Returns the count copied.
fn copy_js_imports(html: &str, base: &Path, dest: &Path) -> usize {
    let mut copied = 0usize;
    let mut visited = std::collections::HashSet::new();
    let mut queue: Vec<String> = Vec::new();
    let enqueue = |queue: &mut Vec<String>, dir: &str, spec: &str| match normalize_rel(dir, spec) {
        Some(rel) => queue.push(rel),
        None => log::warn(&format!(
            "{{js}} import escapes the doc tree, not bundled: {spec}"
        )),
    };
    for body in qmd_js_cell_sources(html) {
        for spec in relative_specifiers(body) {
            enqueue(&mut queue, "", &spec);
        }
    }
    while let Some(rel) = queue.pop() {
        if !visited.insert(rel.clone()) {
            continue;
        }
        let from = base.join(&rel);
        if !from.is_file() {
            continue; // a relative-looking string that isn't a real local file
        }
        let to = dest.join(&rel);
        if !same_file(&from, &to) {
            if let Some(parent) = to.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::copy(&from, &to) {
                Ok(_) => copied += 1,
                Err(e) => {
                    log::warn(&format!("cannot copy {}: {e}", from.display()));
                    continue;
                }
            }
        }
        // Follow the chain: a copied module may import further local files (relative to
        // its OWN dir).
        let ext = Path::new(&rel).extension().and_then(|s| s.to_str());
        if matches!(ext, Some("js") | Some("mjs"))
            && let Ok(src) = std::fs::read_to_string(&from)
        {
            let dir = rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            for spec in relative_specifiers(&src) {
                enqueue(&mut queue, dir, &spec);
            }
        }
    }
    copied
}

/// Build a multi-page site: render every `.qmd` page with the shared chrome to
/// `<out>/<page>.html` and mirror the project's non-source assets alongside, so
/// the output directory is a deployable static site. `out_override` (the `--out`
/// flag) wins over the config's `output-dir` (default `_site`).
/// One warning line per `mounts:` entry: the static build does not wire mounts (only
/// `preview` serves them), so a previewed site's `/<at>/` links 404 in the deploy. Each
/// line gives the exact command to build that mount into `<out>/<at>/`. Empty when the
/// site has no mounts. (Auto-building mounts is a deferred follow-up.)
fn mount_warnings(mounts: &[qmd_fast_core::site::Mount], root: &Path, out: &Path) -> Vec<String> {
    mounts
        .iter()
        .map(|m| {
            format!(
                "mount '/{}/' is preview-only and not in the static build (its links will 404). \
                 Build it: qmd-fast build {} --out {}",
                m.at,
                root.join(&m.path).display(),
                out.join(&m.at).display(),
            )
        })
        .collect()
}

fn build_site(root: &Path, out_override: Option<&str>, strict: bool) -> ExitCode {
    // Executing code cells needs the async kernel, so the whole site build runs on
    // a tokio runtime (mirrors the preview server's setup).
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            log::error(&format!("cannot start runtime: {e}"));
            return ExitCode::FAILURE;
        }
    };
    rt.block_on(build_site_async(root, out_override, strict))
}

async fn build_site_async(root: &Path, out_override: Option<&str>, strict: bool) -> ExitCode {
    let site = qmd_fast_core::Site::discover(root);
    for w in &site.warnings {
        log::warn(w);
    }
    if site.pages.is_empty() {
        log::error(&format!("no .qmd pages found under {}", root.display()));
        return ExitCode::FAILURE;
    }
    let out = match out_override {
        Some(d) => PathBuf::from(d),
        None => root.join(site.output_dir()),
    };
    if let Err(e) = std::fs::create_dir_all(&out) {
        log::error(&format!("cannot create {}: {e}", out.display()));
        return ExitCode::FAILURE;
    }
    let out = out.canonicalize().unwrap_or(out);

    // Refuse to build into the source directory: `mirror_assets` and the page writes
    // would copy files onto themselves, and `fs::copy` truncates the destination
    // first — silently zeroing the user's own assets. (Triggered by `output-dir: .`
    // or `--out <root>`.)
    if root.canonicalize().is_ok_and(|r| r == out) {
        log::error(&format!(
            "output directory is the source directory ({}); refusing to build in place \
             (it would overwrite/truncate your source files). Use a different `output-dir:` or `--out <dir>`.",
            out.display()
        ));
        return ExitCode::FAILURE;
    }

    // `mounts:` are served live in `preview` but the static build doesn't wire them, so
    // warn (with the per-mount build command) rather than ship 404'ing links silently.
    for w in mount_warnings(&site.config.mounts, root, &out) {
        log::warn(&w);
    }

    // Persistent execution cache, rooted at the project source (not the build
    // output), so a `build` and the `preview` server share it and it survives a
    // clean of `_site/`.
    let freeze_dir = root.join("_freeze");

    // 1. Mirror non-source assets (images, etc.) preserving the tree.
    let (assets, skipped_residue) = mirror_assets(root, &out);
    if !skipped_residue.is_empty() {
        log::warn(&format!(
            "skipped {} build-cache dir(s) (not deployed): {}",
            skipped_residue.len(),
            skipped_residue.join(", ")
        ));
    }

    // 2. Render each page with chrome + rewritten links. Code cells run against a
    //    fresh kernel per page (clean state per document; pages with no cells never
    //    boot one), so the static `_site/` carries real computed outputs.
    let mut pages = 0usize;
    let mut kernel_unavailable = false;
    // `--strict` problem tally across the whole site: per-page located warnings +
    // broken cross-refs + crashed cells (each already logged where it occurs).
    let mut problems = 0usize;
    for page in &site.pages {
        let Ok(src) = std::fs::read_to_string(&page.input) else {
            log::warn(&format!("cannot read {}", page.input.display()));
            continue;
        };
        let base = page.input.parent().unwrap_or(root);
        let mut doc = qmd_fast_core::render_document_with_includes(&src, base);
        let mut exec =
            exec::Executor::with_freeze(freeze::page_path(&freeze_dir, &page.rel)).in_dir(base);
        doc.blocks = exec.run(std::mem::take(&mut doc.blocks)).await;
        kernel_unavailable |= exec.diagnostic().is_some();
        // A crashed cell bakes its traceback into the page; log it located + count it.
        problems += report_cell_errors(&doc.blocks, &page.rel);
        let resources = doc.includes.resources.clone();
        // Surface render warnings *and* broken cross-refs so a broken site doesn't
        // deploy silently (these previously only showed in the preview dev menu).
        let (html, warnings) = site.render_page_doc_warned(page, doc);
        for w in &warnings {
            log::warn(&format!("{}: {}", page.rel, w.message));
        }
        problems += warnings.len();
        let dest = out.join(&page.url);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
            copy_resources(&resources, parent);
        }
        match std::fs::write(&dest, html) {
            Ok(()) => pages += 1,
            Err(e) => log::warn(&format!("cannot write {}: {e}", dest.display())),
        }
    }

    // 3. Build each deck referenced by a `{{< embed >}}` to its own self-contained
    //    `.html` (not a chapter/page: no site chrome), so the embedding iframes
    //    resolve in the deployed tree.
    let mut decks = 0usize;
    for deck in &site.decks {
        let Ok(src) = std::fs::read_to_string(&deck.input) else {
            log::warn(&format!(
                "cannot read embedded deck {}",
                deck.input.display()
            ));
            continue;
        };
        let base = deck.input.parent().unwrap_or(root);
        let mut doc = qmd_fast_core::render_document_with_includes(&src, base);
        let mut ex =
            exec::Executor::with_freeze(freeze::page_path(&freeze_dir, &deck.url)).in_dir(base);
        doc.blocks = ex.run(std::mem::take(&mut doc.blocks)).await;
        kernel_unavailable |= ex.diagnostic().is_some();
        problems += report_cell_errors(&doc.blocks, &deck.url);
        let stem = deck
            .url
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".html"))
            .unwrap_or("deck");
        let html = qmd_fast_core::render_doc_to_page(&doc, stem, qmd_fast_core::OutputMode::Build);
        let dest = out.join(&deck.url);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
            copy_resources(&doc.includes.resources, parent);
        }
        match std::fs::write(&dest, html) {
            Ok(()) => decks += 1,
            Err(e) => log::warn(&format!("cannot write {}: {e}", dest.display())),
        }
    }

    if kernel_unavailable {
        log::warn(
            "kernel unavailable; code cells were emitted as source \
             (set QMD_FAST_PYTHON to a python with ipykernel)",
        );
    }
    // Full-text search index, lazy-loaded by the Cmd-K palette (pages link to it
    // via window.QMD_SEARCH_URL rather than inlining it).
    let mut search = "";
    if !site.search_index_json.is_empty() && site.search_index_json != "[]" {
        match std::fs::write(out.join("search.json"), &site.search_index_json) {
            Ok(()) => search = "  ·  search.json",
            Err(e) => log::warn(&format!("cannot write search.json: {e}")),
        }
    }

    // Self-contained `404.html` at the site root: most static hosts serve it for
    // any unknown path (root-absolute links inside, so it works at any depth).
    let mut not_found = "";
    match std::fs::write(out.join("404.html"), site.render_404_page()) {
        Ok(()) => not_found = "  ·  404.html",
        Err(e) => log::warn(&format!("cannot write 404.html: {e}")),
    }
    let deck_note = if decks > 0 {
        format!("  ·  {decks} deck{}", if decks == 1 { "" } else { "s" })
    } else {
        String::new()
    };

    log::built(&format!(
        "{}  ·  {pages} page{}  ·  {assets} asset{}{search}{deck_note}{not_found}",
        out.display(),
        if pages == 1 { "" } else { "s" },
        if assets == 1 { "" } else { "s" },
    ));
    // In `--strict` mode a problem (crashed cell / located warning / broken ref)
    // fails the build after writing it, so CI catches a broken site.
    strict_exit(ExitCode::SUCCESS, strict && problems > 0, problems)
}

/// Source-only file extensions that are build *inputs*, never referenced by the rendered
/// HTML, so they are not mirrored into the deploy: `.qmd` (rendered separately), `.bib`
/// (citations are resolved server-side), `.Rproj` (an editor project file).
const SKIP_EXT: &[&str] = &["qmd", "bib", "Rproj"];

/// Copy every non-source file under `root` into `out`, mirroring the directory tree.
/// Skips: `.qmd`/`.bib`/`.Rproj` sources ([`SKIP_EXT`]), `_`-prefixed and dot entries
/// (`_site.yml`, `_includes`, `_site`, `.RData`, …), build-tool cache/artifact dirs
/// (`*_cache/`, `*_files/` — knitr/RMarkdown/Quarto residue), and the output dir itself.
/// Returns `(files copied, names of skipped cache dirs)` so the caller can report residue
/// it dropped rather than silently omitting it.
fn mirror_assets(root: &Path, out: &Path) -> (usize, Vec<String>) {
    fn walk(
        dir: &Path,
        root: &Path,
        out: &Path,
        seen: &mut std::collections::HashSet<PathBuf>,
        copied: &mut usize,
        skipped: &mut Vec<String>,
    ) {
        // Break symlink cycles: descend into each directory at most once (keyed by
        // canonical path), so a dir symlink pointing at an ancestor can't loop.
        if let Ok(canon) = dir.canonicalize()
            && !seen.insert(canon)
        {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.starts_with('_') || name.starts_with('.') {
                continue;
            }
            if p.is_dir() {
                // Never recurse into the output directory (it may live in-tree).
                if p.canonicalize().ok().as_deref() == Some(out) {
                    continue;
                }
                // Build-tool cache/artifact dirs (knitr/RMarkdown/Quarto) are residue, not
                // content — never drag them into the deployed output.
                if name.ends_with("_cache") || name.ends_with("_files") {
                    skipped.push(name.to_string());
                    continue;
                }
                walk(&p, root, out, seen, copied, skipped);
            } else if !SKIP_EXT.contains(&p.extension().and_then(|s| s.to_str()).unwrap_or("")) {
                let Ok(rel) = p.strip_prefix(root) else {
                    continue;
                };
                let dest = out.join(rel);
                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::copy(&p, &dest).is_ok() {
                    *copied += 1;
                }
            }
        }
    }
    let mut copied = 0;
    let mut skipped = Vec::new();
    walk(
        root,
        root,
        out,
        &mut std::collections::HashSet::new(),
        &mut copied,
        &mut skipped,
    );
    skipped.sort();
    skipped.dedup();
    (copied, skipped)
}

// Helper fns (local_refs, is_local_ref, cmd_render, …) deliberately follow this
// build-test module; the lint about that ordering is a style preference here.
#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod mirror_tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("qmd-mirror-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn mirror_assets_skips_build_residue() {
        let root = tmp("residue");
        let out = tmp("residue-out");
        fs::write(root.join("keep.png"), b"x").unwrap();
        fs::write(root.join("notes.md"), b"x").unwrap(); // not residue -> kept (use _/. to hide)
        fs::write(root.join("refs.bib"), b"x").unwrap(); // source-only -> skipped
        for d in ["index_cache", "report_files", "_freeze"] {
            fs::create_dir_all(root.join(d)).unwrap();
            fs::write(root.join(d).join("a"), b"x").unwrap();
        }
        fs::write(root.join(".RData"), b"x").unwrap(); // dotfile -> skipped

        let (copied, skipped) = mirror_assets(&root, &out);

        assert!(out.join("keep.png").exists(), "plain asset should copy");
        assert!(
            out.join("notes.md").exists(),
            "non-residue file copies (the _/. convention marks private)"
        );
        assert!(
            !out.join("refs.bib").exists(),
            ".bib is source-only residue"
        );
        assert!(!out.join("index_cache").exists(), "*_cache dir is residue");
        assert!(!out.join("report_files").exists(), "*_files dir is residue");
        assert!(!out.join("_freeze").exists(), "_-prefixed dir skipped");
        assert!(!out.join(".RData").exists(), "dotfile skipped");
        assert_eq!(copied, 2, "only keep.png + notes.md copied");
        assert!(
            skipped.contains(&"index_cache".to_string())
                && skipped.contains(&"report_files".to_string()),
            "skipped cache dirs reported: {skipped:?}"
        );

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn copy_local_assets_bundles_js_cell_imports_recursively() {
        let base = tmp("jsimp");
        let out = tmp("jsimp-out");
        // A {js} cell importing a local helper + a remote module; plus a normal image.
        let html = concat!(
            "<img src=\"pic.png\">",
            "<script type=\"application/qmd-js\" data-target=\"c\">\n",
            "const lib = await import(\"./helper.js\");\n",
            "const three = await import(\"https://esm.sh/three@0.163.0\");\n",
            "</script>"
        );
        fs::write(base.join("pic.png"), b"x").unwrap();
        fs::write(
            base.join("helper.js"),
            "import { z } from \"./util.js\";\nexport const y = z;\n",
        )
        .unwrap();
        fs::write(base.join("util.js"), "export const z = 1;\n").unwrap();
        fs::write(base.join("secret.js"), "export const s = 0;\n").unwrap(); // not referenced

        let copied = copy_local_assets(html, &base, &out);

        assert!(
            out.join("helper.js").exists(),
            "directly-imported helper bundled"
        );
        assert!(
            out.join("util.js").exists(),
            "transitively-imported file bundled (recursion)"
        );
        assert!(out.join("pic.png").exists(), "src= asset still bundled");
        assert!(
            !out.join("secret.js").exists(),
            "unreferenced file not bundled"
        );
        assert!(
            !out.join("three").exists() && !out.join("esm.sh").exists(),
            "remote import must not be fetched/copied"
        );
        assert_eq!(copied, 3, "pic.png + helper.js + util.js, got {copied}");

        let _ = fs::remove_dir_all(&base);
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn copy_local_assets_strips_query_and_fragment_from_refs() {
        let base = tmp("query");
        let out = tmp("query-out");
        fs::write(base.join("pic.png"), b"x").unwrap();
        fs::write(base.join("doc.pdf"), b"x").unwrap();
        // A cache-busted image and a fragment-anchored link: the file paths are
        // `pic.png` / `doc.pdf` (a static host ignores the ?query / #fragment).
        let html = "<img src=\"pic.png?v=2\"><a href=\"doc.pdf#page=3\">x</a>";

        let copied = copy_local_assets(html, &base, &out);

        assert!(
            out.join("pic.png").exists(),
            "?query asset should be bundled"
        );
        assert!(
            out.join("doc.pdf").exists(),
            "#fragment asset should be bundled"
        );
        assert_eq!(copied, 2, "got {copied}");

        let _ = fs::remove_dir_all(&base);
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn mount_warnings_name_each_unwired_mount_with_a_build_command() {
        use qmd_fast_core::site::Mount;
        let root = Path::new("/proj/site");
        let out = Path::new("/proj/site/_site");

        let none = mount_warnings(&[], root, out);
        assert!(none.is_empty(), "no mounts -> no warnings");

        let ws = mount_warnings(
            &[Mount {
                at: "docs".into(),
                path: "../docs".into(),
            }],
            root,
            out,
        );
        assert_eq!(ws.len(), 1, "one warning per mount");
        let w = &ws[0];
        assert!(w.contains("docs"), "names the mount: {w}");
        assert!(w.contains("404"), "explains the consequence: {w}");
        assert!(
            w.contains("build") && w.contains("--out"),
            "gives the build command: {w}"
        );
        // the --out path is <out>/<at>
        assert!(
            w.contains(&out.join("docs").display().to_string()),
            "points at <out>/<at>: {w}"
        );
    }

    #[test]
    fn collect_diagnostics_flags_frontmatter_typo_and_broken_xref() {
        let dir = tmp("check-file");
        let f = dir.join("doc.qmd");
        fs::write(&f, "---\ntitle: T\ntitel: oops\n---\n\nSee @fig-nope.\n").unwrap();
        let diags = collect_diagnostics(&f).expect("ok");
        assert!(
            diags.iter().any(|d| d.message.contains("titel")),
            "front-matter typo: {diags:?}"
        );
        assert!(
            diags.iter().any(|d| d.message.contains("@fig-nope")),
            "broken xref: {diags:?}"
        );
        assert!(
            diags.iter().all(|d| d.file.contains("doc.qmd")),
            "located to file: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_diagnostics_flags_malformed_yaml_front_matter() {
        // The live servers report a YAML parse error via frontmatter::yaml_error, but
        // `check`/`build`/`render` silently accept malformed front matter (the lenient
        // line-parser then mis-extracts fields). `check` must surface it too.
        let dir = tmp("check-badyaml");
        let f = dir.join("doc.qmd");
        // Unterminated double-quoted scalar -> serde_yaml parse error.
        fs::write(&f, "---\ntitle: \"unterminated\nauthor: A\n---\n\nBody.\n").unwrap();
        let diags = collect_diagnostics(&f).expect("ok");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("YAML") && d.file.contains("doc.qmd")),
            "malformed YAML must be reported, located: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_diagnostics_surfaces_check_superset_validators() {
        // One doc tripping each new static check; `check` must surface them all.
        let dir = tmp("check-superset");
        let f = dir.join("doc.qmd");
        fs::write(
            &f,
            "---\ntitle: T\n---\n\n## A {#dup}\n\n## B {#dup}\n\nSee [bad](#nope) and ![x](missing.png) and [@key2020].\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&f).expect("ok");
        let has = |needle: &str| diags.iter().any(|d| d.message.contains(needle));
        assert!(has("duplicate heading id"), "dup id: {diags:?}");
        assert!(has("#nope"), "broken anchor: {diags:?}");
        assert!(has("missing.png"), "missing asset: {diags:?}");
        assert!(has("bibliography"), "citation w/o bib: {diags:?}");
        assert!(
            diags.iter().all(|d| d.file.contains("doc.qmd")),
            "located to file: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_site_diagnostics_surfaces_validators_located_per_page() {
        // The site path (per-page base dir + page.rel plumbing) must trip the validators too.
        let dir = tmp("check-site");
        fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
        fs::write(dir.join("index.qmd"), "---\ntitle: Home\n---\n\nWelcome.\n").unwrap();
        fs::write(
            dir.join("page.qmd"),
            "---\ntitle: P\n---\n\n## A {#dup}\n\n## B {#dup}\n\nA missing ![x](nope.png).\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&dir).expect("site ok");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("duplicate heading id") && d.file.contains("page.qmd")),
            "dup id located to its page: {diags:?}"
        );
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nope.png") && d.file.contains("page.qmd")),
            "missing image located to its page: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_superset_has_no_false_positives_across_corpus() {
        // The load-bearing half of the feature ("a green check is publishable") pinned to the
        // REAL check flow: projects as dirs, standalone docs as files, diagnostics/ exempt.
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
        let new_checks = [
            "duplicate heading id",
            "broken in-page link",
            "local asset not found",
            "citations are present",
            "not valid YAML",
        ];
        fn walk(dir: &Path, skip: &[&str], out: &mut Vec<std::path::PathBuf>) {
            for e in fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                if p.is_dir() {
                    if !skip.contains(&name.as_str()) {
                        walk(&p, skip, out);
                    }
                } else if p.extension().is_some_and(|x| x == "qmd") && !name.starts_with('_') {
                    out.push(p);
                }
            }
        }
        // projects (sites/books) are checked as dirs, mirroring `check <dir>`.
        let mut targets: Vec<std::path::PathBuf> = ["bayesian-website", "demo-book", "tech-blog"]
            .iter()
            .map(|s| corpus.join(s))
            .collect();
        // everything else is a standalone doc; diagnostics/ is deliberately tripping (exempt).
        walk(
            &corpus,
            &[
                "diagnostics",
                "bayesian-website",
                "demo-book",
                "tech-blog",
                "_includes",
            ],
            &mut targets,
        );
        for t in &targets {
            let diags = collect_diagnostics(t).unwrap_or_default();
            for d in &diags {
                for c in new_checks {
                    assert!(
                        !d.message.contains(c),
                        "check-superset false positive in {}: {}",
                        t.display(),
                        d.message
                    );
                }
            }
        }
    }

    #[test]
    fn collect_diagnostics_clean_doc_is_empty() {
        let dir = tmp("check-clean");
        let f = dir.join("ok.qmd");
        fs::write(&f, "---\ntitle: T\n---\n\nJust clean prose.\n").unwrap();
        assert!(collect_diagnostics(&f).expect("ok").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_diagnostics_empty_site_is_err() {
        let dir = tmp("check-emptysite");
        fs::write(dir.join("_site.yml"), "title: Empty\n").unwrap();
        assert!(collect_diagnostics(&dir).is_err(), "empty site -> Err");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_json_emits_file_line_message_array() {
        let diags = vec![
            Diagnostic {
                file: "a.qmd".into(),
                line: Some(3),
                message: "weasel word `very`".into(),
            },
            Diagnostic {
                file: "b.qmd".into(),
                line: None,
                message: "needs a \"name\"".into(),
            },
        ];
        let json = format_json(&diags);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed[0]["file"], "a.qmd");
        assert_eq!(parsed[0]["line"], 3);
        assert_eq!(parsed[1]["line"], serde_json::Value::Null);
        assert_eq!(parsed[1]["message"], "needs a \"name\"");
    }

    #[test]
    fn format_human_lists_located_lines() {
        let diags = vec![
            Diagnostic {
                file: "a.qmd".into(),
                line: Some(3),
                message: "m1".into(),
            },
            Diagnostic {
                file: "b.qmd".into(),
                line: None,
                message: "m2".into(),
            },
        ];
        let text = format_human(&diags);
        assert!(text.contains("a.qmd:3: m1"), "located line: {text}");
        assert!(text.contains("b.qmd: m2"), "unlocated line: {text}");
    }
}

/// Unique local `src=`/`href=` values in `html` (skips external URLs, protocol-
/// relative refs, data URIs, in-page anchors, and other schemes).
fn local_refs(html: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for attr in ["src=\"", "href=\""] {
        let mut i = 0;
        while let Some(pos) = html[i..].find(attr) {
            let start = i + pos + attr.len();
            let Some(len) = html[start..].find('"') else {
                break;
            };
            let val = &html[start..start + len];
            i = start + len;
            if is_local_ref(val) && !out.iter().any(|v| v == val) {
                out.push(val.to_string());
            }
        }
    }
    out
}

fn is_local_ref(v: &str) -> bool {
    !v.is_empty()
        && !v.starts_with('#')
        && !v.starts_with("//")
        && !v.contains("://")
        && !v.starts_with("data:")
        && !v.starts_with("mailto:")
        && !v.starts_with("tel:")
        && !v.starts_with("vscode:")
        && !v.starts_with("javascript:")
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
            let doc = qmd_fast_core::render_document_with_includes(&src, base);
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
                    "render does not execute code cells ({kernel_cells} kernel cell{} emitted as \
                     source; figures/outputs will be empty). Use `build` or `preview` to run them.",
                    if kernel_cells == 1 { "" } else { "s" }
                ));
            }
            print!(
                "{}",
                qmd_fast_core::render_doc_to_page(&doc, stem, qmd_fast_core::OutputMode::Build)
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

/// Emit the bundled JSON Schemas for qmd-fast's YAML config (document front matter +
/// `_site.yml`) so an editor's YAML language server can validate them. With `--out <dir>`
/// it writes two files there; otherwise it prints both to stdout. The strings are the
/// committed, bundled schemas (no runtime JSON generation).
fn cmd_schema(args: &[String]) -> ExitCode {
    use qmd_fast_core::schema::{FRONTMATTER_SCHEMA, SITE_SCHEMA};
    let files = [
        ("qmd-frontmatter.schema.json", FRONTMATTER_SCHEMA),
        ("qmd-site.schema.json", SITE_SCHEMA),
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
                eprintln!("qmd-fast schema: cannot create {dir}: {e}");
                return ExitCode::FAILURE;
            }
            for (name, body) in files {
                let path = std::path::Path::new(&dir).join(name);
                if let Err(e) = std::fs::write(&path, body) {
                    eprintln!("qmd-fast schema: cannot write {}: {e}", path.display());
                    return ExitCode::FAILURE;
                }
                println!("wrote {}", path.display());
            }
            println!(
                "add `# yaml-language-server: $schema={dir}/qmd-site.schema.json` atop _site.yml"
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

fn usage() {
    println!(
        "qmd-fast {} ({})",
        qmd_fast_core::VERSION,
        env!("QMD_FAST_GIT_SHA")
    );
    println!("A fast .qmd -> HTML renderer and live preview server.");
    println!();
    println!("USAGE:");
    println!("  qmd-fast <command> <file.qmd> [args]");
    println!();
    println!("COMMANDS:");
    println!("  preview <file.qmd> [port] [--host] [--open] [--no-exec]");
    println!("                             live preview server (aliases: dev, serve;");
    println!("                             default port 4321, auto-picks a free one;");
    println!("                             --host exposes it on your LAN with a QR code");
    println!("                             to open on a phone; --open launches a browser;");
    println!("                             --no-exec previews untrusted docs as source,");
    println!("                             never running their code cells)");
    println!("  build  <file.qmd> [out.html] [--out <dir>] [--strict]");
    println!("                             render a self-contained HTML file");
    println!("                             (default <name>.html beside the source);");
    println!("                             --out <dir> writes <dir>/index.html and");
    println!("                             copies referenced assets for a portable folder;");
    println!("                             --strict exits non-zero on a cell error or");
    println!("                             located warning (broken ref, bad include)");
    println!("  render <file.qmd>          render a full HTML page to stdout");
    println!("                             (static; does NOT execute code cells)");
    println!("  blocks <file.qmd>          list block ids + sourcepos (debug)");
    println!(
        "  schema [--out <dir>]       emit JSON Schemas for _site.yml + front matter (editor autocomplete)"
    );
    println!(
        "  check <file|dir> [--format human|json]  list located diagnostics; exits non-zero if any"
    );
    println!();
    println!("ENV: QMD_FAST_PYTHON (kernel), QMD_FAST_OPEN (=--open),");
    println!("     QMD_FAST_HOST (=--host), QMD_FAST_NO_CLEAR,");
    println!("     QMD_FAST_NO_CACHE (skip the _freeze/ execution cache),");
    println!("     QMD_FAST_NO_EXEC (=--no-exec, never run code cells)");
}

#[cfg(test)]
mod build_diag_tests {
    use super::*;
    use qmd_fast_core::Block;
    use qmd_fast_core::render::{Cell, JsOpts};

    /// A block standing in for an executed cell output, with the given inner HTML.
    fn output_block(html: &str) -> Block {
        Block {
            id: "c-out".into(),
            sourcepos: "7:1-9:3".into(),
            source_file: None,
            html: html.into(),
            cell: None,
        }
    }

    #[test]
    fn report_cell_errors_counts_only_qmd_error_outputs() {
        let blocks = vec![
            output_block("<div class=\"qmd-output\"><pre class=\"qmd-error\">boom</pre></div>"),
            output_block("<div class=\"qmd-output\"><pre>ok</pre></div>"),
            // A *successful* cell that merely prints the text "qmd-error" must not count
            // (we match the class attribute, not the bare substring).
            output_block("<div class=\"qmd-output\"><pre>printed qmd-error here</pre></div>"),
        ];
        assert_eq!(report_cell_errors(&blocks, "page"), 1);
    }

    /// `render` must flag kernel-executed cells (python/r) — but not `{js}` cells,
    /// which run in the browser. This pins the cell-detection predicate `cmd_render`
    /// uses, without spawning a process.
    #[test]
    fn render_flags_kernel_cells_not_js() {
        let cell = |lang: &str| {
            Some(Cell {
                lang: lang.into(),
                code: String::new(),
                figure: None,
                table: None,
                echo: true,
                include: true,
                cache: true,
                fig_export: None,
                js: JsOpts::default(),
            })
        };
        let kernel = |c: &Option<Cell>| {
            c.as_ref()
                .is_some_and(|c| matches!(c.lang.as_str(), "python" | "r"))
        };
        assert!(kernel(&cell("python")));
        assert!(kernel(&cell("r")));
        assert!(!kernel(&cell("js")));
        assert!(!kernel(&None));
    }

    /// `--bare` refuses a slide deck (its navigation is JavaScript). The refusal
    /// happens before any execution, so no kernel is needed here.
    #[test]
    fn bare_refuses_a_slide_deck() {
        let src = "---\nformat: revealjs\n---\n\n# Slide one\n\n## Slide two\n";
        let res = build_page_executing(
            src,
            std::path::Path::new("."),
            "deck",
            qmd_fast_core::OutputMode::Bare,
        )
        .unwrap();
        match res {
            BuildResult::Refused(msg) => {
                assert!(msg.contains("--bare"), "message names the flag: {msg}");
                assert!(
                    msg.to_lowercase().contains("deck"),
                    "message names decks: {msg}"
                );
            }
            BuildResult::Page { .. } => panic!("--bare on a slide deck must be refused"),
        }
    }

    /// A plain document still builds under `--bare`, and the page is script-free.
    #[test]
    fn bare_builds_a_plain_doc_script_free() {
        let base = std::env::temp_dir().join(format!("qmd-bare-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&base);
        let res = build_page_executing(
            "---\ntitle: Draft\n---\n\nProse.\n",
            &base,
            "draft",
            qmd_fast_core::OutputMode::Bare,
        )
        .unwrap();
        match res {
            BuildResult::Page { html, .. } => {
                assert!(!html.contains("<script"), "bare page must have no scripts")
            }
            BuildResult::Refused(m) => panic!("a plain doc should build under --bare: {m}"),
        }
    }
}
