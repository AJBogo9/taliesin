//! The `build` subcommand: render a single document or a whole site to disk.
//!
//! **What:** `build <file>` writes a self-contained HTML page (executing its code cells
//! first); `build <dir>` builds a multi-page site to `_site/`, rendering pages
//! concurrently (memory-capped, drawing kernels from the warm pool) while keeping the
//! output byte-identical to a sequential build. Also `--out <dir>` (portable folder),
//! `--strict`, `--bare`, and `--jobs <N>`.
//!
//! **How to use:** `main()` dispatches `build` to [`cmd_build`].
//!
//! **Depends on:** [`crate::exec`] + [`crate::freeze`] + [`crate::warm_pool`] +
//! [`crate::build_budget`] (execution + the memory budget split), [`crate::check`] (the
//! `_quarto.yml` breadcrumb), [`crate::log`], and [`taliesin_core`] for rendering.
//!
//! **Load-bearing:** the concurrent site build (`build_site_async`/`PageOutcome`) defers
//! all logging and replays it in `site.pages` order, so a parallel build is byte-for-byte
//! identical to `--jobs 1`. Pinned by `tests/parallel_build_determinism.rs`. Do not
//! restructure that ordering or the per-page output/freeze isolation.

use crate::{build_budget, check, exec, freeze, log, warm_pool};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Parse a single `--jobs` raw value token into `Option<usize>` or an error string.
///
/// `raw` is the token immediately following `--jobs`/`-j` on the command line,
/// already filtered to `None` when no non-flag token follows.
///
/// - `None` (flag with no following token): `Err` (requires a value)
/// - `"auto"` or `"0"`: `Ok(None)` (auto, memory- and core-capped)
/// - `"1"` / `"N"`: `Ok(Some(N))`
/// - anything unparseable: `Err(message)`
fn parse_jobs_value(raw: Option<&str>) -> Result<Option<usize>, String> {
    match raw {
        None => Err("--jobs requires a value (e.g. --jobs 4 or --jobs 0 for auto)".to_string()),
        Some("auto") => Ok(None),
        Some(n) => match n.parse::<usize>() {
            Ok(0) => Ok(None),
            Ok(v) => Ok(Some(v)),
            Err(_) => Err(format!(
                "--jobs: invalid value {n:?} (expected a non-negative integer or \"auto\")"
            )),
        },
    }
}

/// The parsed `build` argv (pure; no I/O), so the positional/flag rules are unit-testable.
/// `out_html` (the second positional) and `out_dir` (`--out`/`--dir`) are the two distinct
/// "where to write" meanings: a single-file target vs. a portable folder.
#[derive(Debug)]
struct BuildArgs<'a> {
    path: &'a str,
    out_html: Option<&'a str>,
    out_dir: Option<&'a str>,
    strict: bool,
    bare: bool,
    jobs: Option<usize>,
}

/// Every long flag `build` accepts (drives the unknown-flag did-you-mean). `-j` is the
/// only short alias; it's not in this set (suggestions are between long flags).
const BUILD_FLAGS: &[&str] = &["--out", "--dir", "--jobs", "--strict", "--bare"];

/// Parse `build` argv (`args[2..]`; `args[0..2]` are the binary + "build"). Flags may
/// appear anywhere; the first positional is the source, the optional second is `[out.html]`.
/// Returns `Err(usage/error message)` for a bad `--jobs` value, a value-less `--out`/`--dir`,
/// an unknown `--flag`, or a missing source path.
fn parse_build_args(args: &[String]) -> Result<BuildArgs<'_>, String> {
    let mut positionals: Vec<&str> = Vec::new();
    let mut out_dir: Option<&str> = None;
    let mut strict = false;
    let mut bare = false;
    let mut jobs_result: Result<Option<usize>, String> = Ok(None);
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // `--out <dir>` needs a real value. A missing one (end of args, or a flag
            // follows) is a hard error rather than silently leaving out_dir None and
            // writing `<stem>.html` to an unexpected place.
            "--out" | "--dir" => match it.next().map(|s| s.as_str()) {
                Some(v) if !v.starts_with("--") => out_dir = Some(v),
                _ => {
                    return Err(format!(
                        "error: {a} requires a directory value (e.g. {a} site)"
                    ));
                }
            },
            "--jobs" | "-j" => {
                let raw = it.next().filter(|s| !s.starts_with("--"));
                jobs_result = parse_jobs_value(raw.map(|s| s.as_str()));
            }
            "--strict" => strict = true,
            // `--bare`: zero-`<script>`, zero-CDN, CSS-only-theme single-doc output.
            "--bare" => bare = true,
            // An unrecognized `--flag` is a hard error with a did-you-mean, not silently
            // dropped (a typo'd `--stict` would otherwise build without the intended flag).
            s if s.starts_with("--") => {
                return Err(format!(
                    "error: {}",
                    crate::serve::unknown_flag_error(s, BUILD_FLAGS)
                ));
            }
            s => positionals.push(s),
        }
    }
    // Errors are returned ready-to-print, preserving cmd_build's original messages
    // (the `--jobs` failure was prefixed `error: `; the missing-path one was the usage line).
    let jobs = jobs_result.map_err(|m| format!("error: {m}"))?;
    let path = positionals.first().copied().ok_or_else(|| {
        "usage: taliesin build <file.qmd|dir> [out.html] [--out <dir>] [--strict] [--bare] [--jobs <N>]"
            .to_string()
    })?;
    Ok(BuildArgs {
        path,
        out_html: positionals.get(1).copied(),
        out_dir,
        strict,
        bare,
        jobs,
    })
}

/// `build <file.qmd> [out.html]`: write a self-contained HTML page to a file
/// (default `<stem>.html` beside the source). With `--out <dir>` it instead
/// writes `<dir>/index.html` and copies every referenced local asset alongside
/// (paths preserved), so the directory is deployable as-is. `render` is stdout.
pub(crate) fn cmd_build(args: &[String]) -> ExitCode {
    // Positionals: <file> [out.html]. Flags: `--out <dir>` (alias `--dir`),
    // `--strict` (a cell error / broken-ref warning fails the build).
    let BuildArgs {
        path,
        out_html,
        out_dir,
        strict,
        bare,
        jobs,
    } = match parse_build_args(args) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };
    // A directory is a multi-page site project (`_site.yml` + `.tmd` pages);
    // a single `.tmd` keeps the original self-contained-page behaviour.
    if Path::new(path).is_dir() {
        if bare {
            log::error(
                "--bare builds a single document, not a site (a site's navigation + \
                 search need JavaScript)",
            );
            return ExitCode::FAILURE;
        }
        return build_site(Path::new(path), out_dir, strict, jobs);
    }
    let mode = if bare {
        taliesin_core::OutputMode::Bare
    } else {
        taliesin_core::OutputMode::Build
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
    // Guard the render/execute path: a panic in core rendering (a malformed doc that trips
    // a renderer assertion) must become a located error + non-zero exit, not a raw abort.
    // `block_on` propagates a panic from the directly-awaited future, so the catch here
    // sees it. Outer `Result` = panic; inner = runtime-start I/O failure.
    let executed = crate::serve::guarded(|| build_page_executing(&src, base, stem, mode));
    let (html, problems) = match executed {
        Ok(Ok(BuildResult::Page { html, problems })) => (html, problems),
        // `--bare` refused (e.g. a slide deck): the message is already user-facing.
        Ok(Ok(BuildResult::Refused(msg))) => {
            log::error(&msg);
            return ExitCode::FAILURE;
        }
        Ok(Err(e)) => {
            log::error(&format!("cannot start runtime: {e}"));
            return ExitCode::FAILURE;
        }
        Err(panic) => {
            log::error(&format!("render panicked while building {path}: {panic}"));
            return ExitCode::FAILURE;
        }
    };

    // In `--strict` mode, a cell that crashed (its traceback is baked into the HTML)
    // or any located warning fails the build instead of shipping a broken page with
    // exit 0. Without `--strict` the warnings were already logged; we still write.
    let strict_fail = strict && problems > 0;

    if let Some(dir) = out_dir {
        let code = build_dir(&html, base, Path::new(dir));
        return strict_exit(code, strict_fail, problems);
    }
    let out: PathBuf = out_html
        .map(PathBuf::from)
        .unwrap_or_else(|| base.join(format!("{stem}.html")));
    match std::fs::write(&out, &html) {
        Ok(()) => {
            let dest = out.parent().unwrap_or(base);
            // Bundle the doc's own referenced assets (images, audio, …) next to the
            // page too, so `build doc.tmd out.html` into another directory doesn't
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
/// carries the `tali-error` marker), logging a located warning per failing cell so a
/// crashing cell isn't baked into the build silently. Returns the count.
fn report_cell_errors(blocks: &[taliesin_core::Block], page_label: &str) -> usize {
    let mut n = 0;
    for b in blocks {
        if b.html.contains("class=\"tali-error\"") {
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
/// Result of building a single page: the rendered HTML (+ its `--strict` problem
/// count), or a `--bare` refusal whose message is user-facing.
enum BuildResult {
    Page { html: String, problems: usize },
    Refused(String),
}

/// Warn (never silently degrade) about the constructs `--bare` drops: a `{js}` cell
/// is inert without its browser runtime, and Mermaid ships its diagram as source.
fn warn_bare_exclusions(doc: &taliesin_core::RenderedDoc) {
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
    mode: taliesin_core::OutputMode,
) -> std::io::Result<BuildResult> {
    let rt = tokio::runtime::Runtime::new()?;
    Ok(rt.block_on(async {
        // `problems` is what `--strict` fails on: located render warnings, broken
        // cross-refs, and crashed code cells — each already logged below.
        let mut problems = 0usize;
        let mut doc = taliesin_core::render_document_with_includes(src, base);
        // `--bare` is prose-shaped, JS-free output: a slide deck (whose navigation is
        // JavaScript) can't be one. Refuse before doing any execution work.
        if mode == taliesin_core::OutputMode::Bare && doc.format == taliesin_core::DocFormat::Reveal
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
        for target in taliesin_core::render::embed_targets(src) {
            log::warn(&format!(
                "{{{{< embed {target} >}}}} won't resolve in a single-doc build (its \
                 target isn't built); build the containing directory as a site, or \
                 inline the content instead."
            ));
        }
        // Broken cross-refs (a single doc has no site to resolve them across pages),
        // so a `build` doesn't ship a dangling `@fig-`/`@sec-` link silently.
        let xrefs = taliesin_core::cite::validate_xrefs(&doc.blocks);
        for w in &xrefs {
            log::warn(&w.message);
        }
        problems += xrefs.len();
        // Persistent execution cache keyed off the doc's stem, beside the source.
        let mut ex =
            exec::Executor::with_freeze(freeze::page_path(&base.join("_freeze"), fallback))
                .in_dir(base);
        doc.blocks = ex.run(std::mem::take(&mut doc.blocks)).await;
        // The executor's own diagnostic already names the failing language and the
        // right env var (`QMD_FAST_R` for R, `QMD_FAST_PYTHON` otherwise) — use it
        // verbatim instead of a hardcoded python-only hint.
        if let Some(d) = ex.diagnostic() {
            log::warn(&d);
        }
        // A crashed cell bakes its traceback into the page (exit 0 + silent stderr
        // before this); log it located and count it toward `--strict`.
        problems += report_cell_errors(&doc.blocks, fallback);
        if mode == taliesin_core::OutputMode::Bare {
            warn_bare_exclusions(&doc);
        }
        BuildResult::Page {
            html: taliesin_core::render_doc_to_page(&doc, fallback, mode),
            problems,
        }
    }))
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
/// build (so `build doc.tmd out.html` into another directory isn't left with
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

/// Deploy any in-tree file a page links to whose extension is in [`SKIP_EXT`] — the
/// source-only set [`mirror_assets`] drops as potential stray residue. A *referenced*
/// source (a linked `.md` download, a `.scss` offered for inspection) is intentional, so
/// dropping it leaves a dead link on an otherwise-green build. Non-source assets are
/// already mirrored, and cross-page / out-of-tree refs are silently ignored here (the
/// loud out-of-tree warning belongs to the single-doc [`copy_local_assets`]).
fn deploy_referenced_sources(html: &str, base: &Path, dest: &Path) -> usize {
    let mut copied = 0usize;
    for r in local_refs(html) {
        let path = &r[..r.find(['?', '#']).unwrap_or(r.len())];
        // Cross-page / out-of-tree refs aren't ours to ship; mirror_assets already
        // handled every non-source asset, so only the SKIP_EXT files can be missing.
        if path.starts_with('/') || path.split('/').any(|seg| seg == "..") {
            continue;
        }
        let ext = Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !SKIP_EXT.contains(&ext) {
            continue;
        }
        let from = base.join(path);
        if !from.is_file() {
            continue;
        }
        let to = dest.join(path);
        if same_file(&from, &to) {
            continue;
        }
        if let Some(parent) = to.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::copy(&from, &to).is_ok() {
            copied += 1;
        }
    }
    copied
}

/// Second asset pass for a site build: after every page is written, ship the source
/// files (`.md`/`.scss`/…) that pages actually *link to*. The output tree mirrors the
/// source tree, so each page's relative refs resolve from its source directory. Returns
/// the count deployed. See [`deploy_referenced_sources`].
fn deploy_referenced_sources_for_site(root: &Path, out: &Path) -> usize {
    fn walk(dir: &Path, root: &Path, out: &Path, copied: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, root, out, copied);
            } else if p.extension().and_then(|s| s.to_str()) == Some("html") {
                let Ok(html) = std::fs::read_to_string(&p) else {
                    continue;
                };
                let rel_dir = p
                    .strip_prefix(out)
                    .ok()
                    .and_then(Path::parent)
                    .unwrap_or(Path::new(""));
                *copied +=
                    deploy_referenced_sources(&html, &root.join(rel_dir), &out.join(rel_dir));
            }
        }
    }
    let mut copied = 0usize;
    walk(out, root, out, &mut copied);
    copied
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

/// Build a multi-page site: render every `.tmd` page with the shared chrome to
/// `<out>/<page>.html` and mirror the project's non-source assets alongside, so
/// the output directory is a deployable static site. `out_override` (the `--out`
/// flag) wins over the config's `output-dir` (default `_site`).
/// One warning line per `mounts:` entry: the static build does not wire mounts (only
/// `preview` serves them), so a previewed site's `/<at>/` links 404 in the deploy. Each
/// line gives the exact command to build that mount into `<out>/<at>/`. Empty when the
/// site has no mounts. (Auto-building mounts is a deferred follow-up.)
fn mount_warnings(mounts: &[taliesin_core::site::Mount], root: &Path, out: &Path) -> Vec<String> {
    mounts
        .iter()
        .map(|m| {
            format!(
                "mount '/{}/' is preview-only and not in the static build (its links will 404). \
                 Build it: taliesin build {} --out {}",
                m.at,
                root.join(&m.path).display(),
                out.join(&m.at).display(),
            )
        })
        .collect()
}

/// Concurrent page builds move an owned [`exec::Executor`] into a spawned task, so it must
/// be `Send`. It is — its kernel handles are `tokio::process::{Child, Child*}` (all `Send`)
/// and everything else is plain data — but assert it at compile time so a future field that
/// breaks `Send` (e.g. an `Rc`) is caught here, not as an opaque spawn error.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<exec::Executor>();
};

/// The result of building one page concurrently: the deferred log lines (replayed in
/// page order so parallel and sequential builds log identically), the `--strict` problem
/// count, whether a kernel was unavailable, and whether the page file was written.
///
/// Logging is *collected*, not emitted, inside the per-page task: only file writes happen
/// off-thread, and those go to per-page destinations (the page's own `url`, its own
/// `_freeze/<rel>.json`), so concurrent pages never race on the same path. The caller
/// replays everything in `site.pages` order, making the whole build deterministic.
struct PageOutcome {
    /// Warn lines, in the exact order the sequential build emitted them (cell errors
    /// first, then render/cross-ref warnings), replayed by the caller in page order.
    warnings: Vec<String>,
    problems: usize,
    kernel_unavailable: bool,
    written: bool,
}

/// Build one page: render its markdown, execute its code cells on a *fresh, page-private*
/// executor (own kernel + own `_freeze/<rel>.json`, cwd = the page's own dir), render the
/// chrome-wrapped HTML, then write it and copy its resources. Pure w.r.t. shared state:
/// the only writes are to this page's own output file + freeze file, so it is safe to run
/// many of these at once. All logging is deferred into the returned [`PageOutcome`].
async fn build_one_page(
    site: &taliesin_core::Site,
    page: &taliesin_core::site::Page,
    freeze_dir: &Path,
    out: &Path,
    root: &Path,
    warm_pool: Option<std::sync::Arc<warm_pool::WarmPool>>,
) -> PageOutcome {
    let mut warnings = Vec::new();
    let Ok(src) = std::fs::read_to_string(&page.input) else {
        warnings.push(format!("cannot read {}", page.input.display()));
        return PageOutcome {
            warnings,
            problems: 0,
            kernel_unavailable: false,
            written: false,
        };
    };
    let base = page.input.parent().unwrap_or(root);
    let mut doc =
        taliesin_core::render_document_with_includes_scoped(&src, base, site.chapter_for(page));
    let mut exec =
        exec::Executor::with_freeze(freeze::page_path(freeze_dir, &page.rel)).in_dir(base);
    // Draw this page's Python kernel from the shared warm pool (when one booted) so a
    // page with code cells starts near-instantly instead of cold-booting. `None`
    // (unset interpreter / inert pool) cold-starts exactly as before.
    exec.set_warm_pool(warm_pool);
    doc.blocks = exec.run(std::mem::take(&mut doc.blocks)).await;
    let kernel_unavailable = exec.diagnostic().is_some();
    let mut problems = 0usize;
    // A crashed cell bakes its traceback into the page; collect a located line + count it
    // (same shape/order as the sequential `report_cell_errors`, but deferred).
    for b in &doc.blocks {
        if b.html.contains("class=\"tali-error\"") {
            problems += 1;
            let where_ = b
                .source_file
                .as_deref()
                .map(|f| format!("{f} "))
                .unwrap_or_default();
            warnings.push(format!(
                "cell error in {} ({where_}@ {}): code cell raised an uncaught \
                 exception; its traceback is baked into the output",
                page.rel, b.sourcepos
            ));
        }
    }
    // Surface render warnings *and* broken cross-refs so a broken site doesn't deploy
    // silently (these previously only showed in the preview dev menu).
    let (html, render_warnings) = site.render_page_doc_warned(page, doc);
    for w in &render_warnings {
        warnings.push(format!("{}: {}", page.rel, w.message));
    }
    problems += render_warnings.len();
    let dest = out.join(&page.url);
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let written = match std::fs::write(&dest, html) {
        Ok(()) => true,
        Err(e) => {
            warnings.push(format!("cannot write {}: {e}", dest.display()));
            false
        }
    };
    PageOutcome {
        warnings,
        problems,
        kernel_unavailable,
        written,
    }
}

fn build_site(
    root: &Path,
    out_override: Option<&str>,
    strict: bool,
    jobs: Option<usize>,
) -> ExitCode {
    // Executing code cells needs the async kernel, so the whole site build runs on
    // a tokio runtime (mirrors the preview server's setup). A multi-thread runtime so
    // concurrent page builds (each its own kernel) actually overlap on the CPU.
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log::error(&format!("cannot start runtime: {e}"));
            return ExitCode::FAILURE;
        }
    };
    rt.block_on(build_site_async(root, out_override, strict, jobs))
}

async fn build_site_async(
    root: &Path,
    out_override: Option<&str>,
    strict: bool,
    jobs: Option<usize>,
) -> ExitCode {
    // A `_quarto.yml`-only directory gets the migration breadcrumb instead of the
    // site walker's `no _site.yml at <root>` (which names a file the user never made).
    let quarto_hint = check::quarto_migration_hint(root);
    if let Some(hint) = &quarto_hint {
        log::warn(hint);
    }
    let mut site = taliesin_core::Site::discover(root);
    // A malformed `_site.yml` silently degrades the whole site to defaults (no nav, no
    // title, wrong output dir): a real `--strict` problem, unlike a benign missing config.
    let mut config_problems = 0usize;
    for w in &site.warnings {
        // When the breadcrumb already fired, drop the redundant `no _site.yml` warning.
        if quarto_hint.is_some() && w.starts_with("no _site.yml") {
            continue;
        }
        if taliesin_core::site::is_malformed_config_warning(w) {
            config_problems += 1;
        }
        log::warn(w);
    }
    if site.pages.is_empty() {
        log::error(&format!("no .qmd pages found under {}", root.display()));
        return ExitCode::FAILURE;
    }
    // Build-only render-harvest: give cross-page `@fig-`/`@eq-`/`@thm-` refs their number
    // (assigned only during render, so the source-scan couldn't). The live preview skips
    // this extra render pass; there a cross-page fig/eq ref stays bare (the link resolves).
    site.harvest_xref_numbers();
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
    //
    //    Pages are independent (each writes only its own output + `_freeze/<rel>.json`,
    //    runs its own kernel in its own cwd), so we build up to `cap` of them at once.
    //    Determinism is preserved: scheduling only changes *when* a page builds, never
    //    *what* it produces, and per-page outcomes (file bytes + log lines) are replayed
    //    in `site.pages` order so a `--jobs N` build is byte- and log-identical to the
    //    sequential one. `--jobs 1` (today's default) takes the in-order serial path.
    //    Cross-page ordering edges (a page that must build after another) are deferred to
    //    Task 9; here every dirty page is treated as independent.
    // One memory/core budget covers *all* resident kernels: the eager warm pool plus
    // the kernels the concurrent build runs at once. `budget_split` reserves a small
    // warm pool (build-first: never below 1 build kernel) so the two never together
    // exceed the cap. The build semaphore is sized to `build_kernels`; the warm pool
    // pre-warms `warm_pool` so each page build can draw a hot kernel instead of paying
    // a cold boot. Determinism is untouched: a pooled kernel runs the same ipykernel
    // with the same preambles as a cold one, so it produces identical bytes — the pool
    // only changes *when* a kernel is ready, never *what* it computes.
    let cap = build_budget::concurrency_cap(jobs, build_budget::PER_KERNEL_MB).max(1);
    let split = build_budget::budget_split(cap);
    let build_cap = split.build_kernels.max(1);
    log::info(&format!(
        "building with up to {} parallel page(s); pre-warming {} kernel(s)",
        build_cap, split.warm_pool
    ));
    let mut pages = 0usize;
    let mut kernel_unavailable = false;
    // `--strict` problem tally across the whole site: a malformed `_site.yml`, per-page
    // located warnings, broken cross-refs, crashed cells, and page-task panics (each
    // already logged where it occurs).
    let mut problems = config_problems;

    // The one process-wide warm pool for this build. `None` (so every page cold-starts
    // exactly as today) when `QMD_FAST_PYTHON` is unset or the forkserver can't boot;
    // dropped at the end of this fn, killing the daemon + idle kernels.
    let warm_pool = warm_pool::warm_pool_for_build(split.warm_pool).await;

    // Build into a slot per page (indexed by page order) so results aggregate
    // deterministically regardless of completion order. A `Semaphore` of size
    // `build_cap` bounds how many build kernels run at once (memory-aware, reconciled
    // with the warm pool); the pool lookup / file write each page does is on its own
    // paths, so no lock is held across the `.await`.
    let site = std::sync::Arc::new(site);
    let out = std::sync::Arc::new(out);
    let freeze_dir = std::sync::Arc::new(freeze_dir);
    let root_arc = std::sync::Arc::new(root.to_path_buf());
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(build_cap));
    let mut set: tokio::task::JoinSet<(usize, PageOutcome)> = tokio::task::JoinSet::new();
    for (idx, _page) in site.pages.iter().enumerate() {
        let site = site.clone();
        let out = out.clone();
        let freeze_dir = freeze_dir.clone();
        let root_arc = root_arc.clone();
        let sem = sem.clone();
        let warm_pool = warm_pool.clone();
        set.spawn(async move {
            // Hold a permit only for this page's build; dropping it on return frees the
            // slot for the next queued page. The permit guards kernel count, not any
            // shared data structure, so nothing is locked across the build's `.await`.
            let _permit = sem.acquire().await.expect("build semaphore not closed");
            let page = &site.pages[idx];
            let outcome =
                build_one_page(&site, page, &freeze_dir, &out, &root_arc, warm_pool).await;
            (idx, outcome)
        });
    }

    let mut outcomes: Vec<Option<PageOutcome>> = (0..site.pages.len()).map(|_| None).collect();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((idx, outcome)) => outcomes[idx] = Some(outcome),
            // A page task panicked: keep going so the rest of the site still builds (the
            // missing page just won't be written), but count it as a `--strict` problem so
            // a panicked page can't ship a green build with a silently dropped page.
            Err(e) => {
                problems += 1;
                log::error(&format!("page build task failed: {e}"));
            }
        }
    }

    // Replay every page's deferred logs + tally counters in page order, so the build's
    // output is identical whether it ran 1-wide or N-wide.
    for outcome in outcomes.into_iter().flatten() {
        for w in &outcome.warnings {
            log::warn(w);
        }
        problems += outcome.problems;
        kernel_unavailable |= outcome.kernel_unavailable;
        if outcome.written {
            pages += 1;
        }
    }
    // Reclaim the owned values the deck loop below still uses.
    let site = std::sync::Arc::try_unwrap(site).unwrap_or_else(|arc| (*arc).clone());
    let out = std::sync::Arc::try_unwrap(out).unwrap_or_else(|arc| (*arc).clone());
    let freeze_dir = std::sync::Arc::try_unwrap(freeze_dir).unwrap_or_else(|arc| (*arc).clone());

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
        let mut doc = taliesin_core::render_document_with_includes(&src, base);
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
        let html = taliesin_core::render_doc_to_page(&doc, stem, taliesin_core::OutputMode::Build);
        let dest = out.join(&deck.url);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
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
    // Full-text search index, lazy-loaded by the Cmd-K palette (pages link to it via
    // window.TALIESIN_SEARCH_URL rather than inlining it). Written as a `search-index.js`
    // script that assigns window.TALIESIN_SEARCH_INDEX (not a raw `.json`): the client loads
    // it with a <script>, which works under file:// too, so Cmd-K works from disk.
    let mut search = "";
    if !site.search_index_json.is_empty() && site.search_index_json != "[]" {
        let js = format!("window.TALIESIN_SEARCH_INDEX={};", site.search_index_json);
        match std::fs::write(out.join("search-index.js"), js) {
            Ok(()) => search = "  ·  search-index.js",
            Err(e) => log::warn(&format!("cannot write search-index.js: {e}")),
        }
    }

    // Self-contained `404.html` at the site root: most static hosts serve it for
    // any unknown path (root-absolute links inside, so it works at any depth). But
    // honor an author's own `404.tmd` — it already rendered to `out/404.html` in the
    // page loop above, so emitting the built-in template would clobber it. Only fall
    // back to the built-in when the author supplied none.
    let mut not_found = "";
    if site.has_author_404() {
        not_found = "  ·  404.html (yours)";
    } else {
        match std::fs::write(out.join("404.html"), site.render_404_page()) {
            Ok(()) => not_found = "  ·  404.html",
            Err(e) => log::warn(&format!("cannot write 404.html: {e}")),
        }
    }
    let deck_note = if decks > 0 {
        format!("  ·  {decks} deck{}", if decks == 1 { "" } else { "s" })
    } else {
        String::new()
    };

    // Second asset pass: ship source files (`.md`/`.scss`/…) that pages actually link to.
    // mirror_assets drops them by extension (publish hygiene), but a *referenced* source is
    // an intentional download — skipping it would leave a dead link on a green build.
    let assets = assets + deploy_referenced_sources_for_site(root, &out);

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

/// Source-only file extensions that are build *inputs* / prose / stylesheet sources,
/// never referenced by the rendered HTML, so they are not mirrored into the deploy:
/// `.qmd` (rendered separately), `.bib` (citations resolved server-side), `.Rproj` (an
/// editor project file), `.md` (prose/planning the renderer never serves), and `.scss`/
/// `.sass` (stylesheet sources — output references the compiled `.css`). Keeping these
/// out of `_site/` is publish hygiene: a stray `notes.md` or `theme.scss` in the source
/// tree never leaks onto the live site. (To deploy a private *binary* asset selectively,
/// the `_`/`.`-prefix convention still applies; these are excluded by kind.)
const SKIP_EXT: &[&str] = &["qmd", "bib", "Rproj", "md", "scss", "sass"];

/// Copy every non-source file under `root` into `out`, mirroring the directory tree.
/// Skips: source-only extensions ([`SKIP_EXT`]: `.qmd`/`.bib`/`.Rproj`/`.md`/`.scss`/
/// `.sass`), `_`-prefixed and dot entries (`_site.yml`, `_includes`, `_site`, `.RData`, …),
/// build-tool cache/artifact dirs (`*_cache/`, `*_files/` — knitr/RMarkdown/Quarto
/// residue), and the output dir itself.
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

#[cfg(test)]
mod mirror_tests {
    use super::*;
    use std::fs;

    #[test]
    fn build_args_distinguish_outhtml_positional_from_out_dir_flag() {
        // `BuildArgs` borrows from the argv, so each case binds its vec first.
        let argv = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<String>>();

        // file only: path, no [out.html] target, no portable-folder dir.
        let a = argv(&["qmd-fast", "build", "doc.tmd"]);
        let p = parse_build_args(&a).unwrap();
        assert_eq!((p.path, p.out_html, p.out_dir), ("doc.tmd", None, None));

        // second positional = the [out.html] single-file target.
        let a = argv(&["qmd-fast", "build", "doc.tmd", "out.html"]);
        let p = parse_build_args(&a).unwrap();
        assert_eq!(
            (p.path, p.out_html, p.out_dir),
            ("doc.tmd", Some("out.html"), None)
        );

        // --out <dir> is the portable-folder flag, distinct from the positional.
        let a = argv(&["qmd-fast", "build", "doc.tmd", "--out", "site"]);
        let p = parse_build_args(&a).unwrap();
        assert_eq!(
            (p.path, p.out_html, p.out_dir),
            ("doc.tmd", None, Some("site"))
        );

        // --out never captures a following flag as its directory: a value-less --out is
        // now a HARD ERROR (rather than silently dropping the flag + writing <stem>.html).
        let err = parse_build_args(&argv(&["qmd-fast", "build", "doc.tmd", "--out", "--bare"]))
            .expect_err("value-less --out errors");
        assert!(err.contains("--out") && err.contains("requires"), "{err}");
        // --out at the very end (no following token) is the same hard error.
        let err = parse_build_args(&argv(&["qmd-fast", "build", "doc.tmd", "--out"]))
            .expect_err("trailing --out errors");
        assert!(err.contains("--out"), "{err}");
        // --dir is the alias and errors the same way.
        assert!(parse_build_args(&argv(&["qmd-fast", "build", "doc.tmd", "--dir"])).is_err());

        // flags may appear anywhere; both positionals still bind in order.
        let a = argv(&["qmd-fast", "build", "--bare", "doc.tmd", "out.html"]);
        let p = parse_build_args(&a).unwrap();
        assert!(p.bare);
        assert_eq!((p.path, p.out_html), ("doc.tmd", Some("out.html")));

        // a missing path is a usage error.
        assert!(parse_build_args(&argv(&["qmd-fast", "build"])).is_err());
        assert!(parse_build_args(&argv(&["qmd-fast", "build", "--strict"])).is_err());
    }

    #[test]
    fn build_unknown_flag_errors_with_did_you_mean() {
        let argv = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        // A typo'd flag is a hard error (not silently dropped) and suggests the real one.
        let err = parse_build_args(&argv(&["qmd-fast", "build", "doc.tmd", "--stict"]))
            .expect_err("--stict must error");
        assert!(err.contains("--stict"), "names the bad flag: {err}");
        assert!(err.contains("--strict"), "suggests the near match: {err}");
        // A flag with no near match still errors (no wild guess).
        let err = parse_build_args(&argv(&["qmd-fast", "build", "doc.tmd", "--frobnicate"]))
            .expect_err("unknown flag must error");
        assert!(err.contains("--frobnicate"), "{err}");
        assert!(!err.contains("did you mean"), "no wild guess: {err}");
        // The real flags still parse (no regression).
        assert!(parse_build_args(&argv(&["qmd-fast", "build", "doc.tmd", "--strict"])).is_ok());
        assert!(parse_build_args(&argv(&["qmd-fast", "build", "doc.tmd", "--bare"])).is_ok());
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tali-mirror-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn mirror_assets_skips_build_residue() {
        let root = tmp("residue");
        let out = tmp("residue-out");
        fs::write(root.join("keep.png"), b"x").unwrap();
        fs::write(root.join("notes.md"), b"x").unwrap(); // prose/planning source -> not deployed
        fs::write(root.join("theme.scss"), b"x").unwrap(); // stylesheet source -> not deployed
        fs::write(root.join("refs.bib"), b"x").unwrap(); // source-only -> skipped
        for d in ["index_cache", "report_files", "_freeze"] {
            fs::create_dir_all(root.join(d)).unwrap();
            fs::write(root.join(d).join("a"), b"x").unwrap();
        }
        fs::write(root.join(".RData"), b"x").unwrap(); // dotfile -> skipped

        let (copied, skipped) = mirror_assets(&root, &out);

        assert!(out.join("keep.png").exists(), "plain asset should copy");
        assert!(
            !out.join("notes.md").exists(),
            ".md is a prose/planning source, never referenced by the rendered HTML -> not deployed"
        );
        assert!(
            !out.join("theme.scss").exists(),
            ".scss is a stylesheet source (output references compiled .css) -> not deployed"
        );
        assert!(
            !out.join("refs.bib").exists(),
            ".bib is source-only residue"
        );
        assert!(!out.join("index_cache").exists(), "*_cache dir is residue");
        assert!(!out.join("report_files").exists(), "*_files dir is residue");
        assert!(!out.join("_freeze").exists(), "_-prefixed dir skipped");
        assert!(!out.join(".RData").exists(), "dotfile skipped");
        assert_eq!(copied, 1, "only keep.png is a deployable asset");
        assert!(
            skipped.contains(&"index_cache".to_string())
                && skipped.contains(&"report_files".to_string()),
            "skipped cache dirs reported: {skipped:?}"
        );

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn deploy_referenced_sources_ships_linked_source_but_not_stray() {
        // A page linking a `.md`/`.scss` source means an intentional download; mirror_assets
        // drops those by extension, so this second pass must ship the REFERENCED ones while
        // leaving an unreferenced stray source out (publish hygiene preserved).
        let root = tmp("refsrc");
        let out = tmp("refsrc-out");
        fs::write(root.join("notes.md"), b"# notes").unwrap();
        fs::write(root.join("stray.md"), b"stray").unwrap();
        fs::write(root.join("theme.scss"), b"x").unwrap();
        let html = r#"<a href="notes.md">notes</a> <link href="theme.scss">"#;

        let copied = deploy_referenced_sources(html, &root, &out);

        assert!(out.join("notes.md").is_file(), "a linked .md must deploy");
        assert!(
            out.join("theme.scss").is_file(),
            "a linked .scss must deploy"
        );
        assert!(
            !out.join("stray.md").exists(),
            "an unreferenced source must NOT deploy"
        );
        assert_eq!(copied, 2, "exactly the two referenced sources");

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
        use taliesin_core::site::Mount;
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
}

#[cfg(test)]
mod build_diag_tests {
    use super::*;
    use taliesin_core::Block;
    use taliesin_core::render::{Cell, JsOpts};

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
            output_block("<div class=\"tali-output\"><pre class=\"tali-error\">boom</pre></div>"),
            output_block("<div class=\"tali-output\"><pre>ok</pre></div>"),
            // A *successful* cell that merely prints the text "tali-error" must not count
            // (we match the class attribute, not the bare substring).
            output_block("<div class=\"tali-output\"><pre>printed tali-error here</pre></div>"),
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
            taliesin_core::OutputMode::Bare,
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
        let base = std::env::temp_dir().join(format!("tali-bare-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&base);
        let res = build_page_executing(
            "---\ntitle: Draft\n---\n\nProse.\n",
            &base,
            "draft",
            taliesin_core::OutputMode::Bare,
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

#[cfg(test)]
mod jobs_tests {
    use super::*;

    /// `parse_jobs_value` maps the token that follows `--jobs` to `Option<usize>`:
    /// - `None` (flag present, no token follows) → Err (requires a value)
    /// - `"auto"` or `"0"`                       → Ok(None)  (auto)
    /// - `"1"`                                    → Ok(Some(1))  (sequential)
    /// - `"N"` (e.g. `"4"`)                      → Ok(Some(N))  (explicit)
    /// - bad string                               → Err
    ///
    /// The "flag absent" case is handled by the caller: `jobs_result` defaults to
    /// `Ok(None)` (auto) and is only overwritten when `--jobs` actually appears.
    #[test]
    fn jobs_flag_parses_correctly() {
        // "auto" keyword → auto
        assert_eq!(parse_jobs_value(Some("auto")), Ok(None));
        // "0" → auto (same as None/absent)
        assert_eq!(parse_jobs_value(Some("0")), Ok(None));
        // "1" → sequential
        assert_eq!(parse_jobs_value(Some("1")), Ok(Some(1)));
        // explicit N
        assert_eq!(parse_jobs_value(Some("4")), Ok(Some(4)));
        assert_eq!(parse_jobs_value(Some("16")), Ok(Some(16)));
        // --jobs with no following token (e.g. at end of arg list) → clear error
        let no_val = parse_jobs_value(None);
        assert!(no_val.is_err(), "--jobs with no value should error");
        let msg_no_val = no_val.unwrap_err();
        assert!(
            msg_no_val.contains("--jobs"),
            "error names the flag: {msg_no_val}"
        );
        // bad value → error
        let bad = parse_jobs_value(Some("fish"));
        assert!(bad.is_err(), "non-integer should be an error");
        let msg = bad.unwrap_err();
        assert!(
            msg.contains("fish"),
            "error message names the bad value: {msg}"
        );
        assert!(
            msg.contains("--jobs"),
            "error message names the flag: {msg}"
        );
    }
}
