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
fn cmd_build(args: &[String]) -> ExitCode {
    // Positionals: <file> [out.html]. Flag: `--out <dir>` (alias `--dir`).
    let mut positionals: Vec<&str> = Vec::new();
    let mut out_dir: Option<&str> = None;
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
            s if s.starts_with("--") => {}
            s => positionals.push(s),
        }
    }
    let Some(path) = positionals.first().copied() else {
        eprintln!("usage: qmd-fast build <file.qmd|dir> [out.html] [--out <dir>]");
        return ExitCode::FAILURE;
    };
    // A directory is a multi-page site project (`_site.yml` + `.qmd` pages);
    // a single `.qmd` keeps the original self-contained-page behaviour.
    if Path::new(path).is_dir() {
        return build_site(Path::new(path), out_dir);
    }
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
    let (html, resources) = match build_page_executing(&src, base, stem) {
        Ok(h) => h,
        Err(e) => {
            log::error(&format!("cannot start runtime: {e}"));
            return ExitCode::FAILURE;
        }
    };

    if let Some(dir) = out_dir {
        let code = build_dir(&html, base, Path::new(dir));
        copy_resources(&resources, Path::new(dir));
        return code;
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
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error(&format!("cannot write {}: {e}", out.display()));
            ExitCode::FAILURE
        }
    }
}

/// Render a single document to a self-contained HTML page, executing its code
/// cells first so figures / `ojs_define` outputs are baked in (mirrors the site
/// build's per-page execution). A missing kernel logs a warning and the cells fall
/// back to source, matching the preview's behaviour.
fn build_page_executing(
    src: &str,
    base: &Path,
    fallback: &str,
) -> std::io::Result<(String, Vec<PathBuf>)> {
    // An include that doesn't resolve leaves its `{{< include … >}}` directive
    // literal in the output; warn rather than ship it silently (the preview's
    // diagnostics already flag this, so build matches that behaviour).
    for dep in qmd_fast_core::includes::dependencies(src, base) {
        if !dep.exists() {
            let shown = dep.strip_prefix(base).unwrap_or(&dep);
            log::warn(&format!("include not found: {}", shown.display()));
        }
    }
    let rt = tokio::runtime::Runtime::new()?;
    Ok(rt.block_on(async {
        let mut doc = qmd_fast_core::render_document_with_includes(src, base);
        for w in &doc.warnings {
            log::warn(&w.message);
        }
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
        for w in qmd_fast_core::cite::validate_xrefs(&doc.blocks) {
            log::warn(&w.message);
        }
        // Persistent execution cache keyed off the doc's stem, beside the source.
        let mut ex =
            exec::Executor::with_freeze(freeze::page_path(&base.join("_freeze"), fallback));
        doc.blocks = ex.run(std::mem::take(&mut doc.blocks)).await;
        if ex.diagnostic().is_some() {
            log::warn(
                "kernel unavailable; code cells emitted as source \
                 (set QMD_FAST_PYTHON to a python with ipykernel)",
            );
        }
        let resources = doc.includes.resources.clone();
        (qmd_fast_core::render_doc_to_page(&doc, fallback), resources)
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
        if r.starts_with('/') || r.split('/').any(|seg| seg == "..") {
            log::warn(&format!("asset outside the doc tree, not bundled: {r}"));
            continue;
        }
        let from = base.join(&r);
        if !from.is_file() {
            continue; // e.g. an href to something that isn't a local file
        }
        let to = dest.join(&r);
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
    copied
}

/// Whether two paths resolve to the same file on disk (so we don't self-copy).
fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

/// Build a multi-page site: render every `.qmd` page with the shared chrome to
/// `<out>/<page>.html` and mirror the project's non-source assets alongside, so
/// the output directory is a deployable static site. `out_override` (the `--out`
/// flag) wins over the config's `output-dir` (default `_site`).
fn build_site(root: &Path, out_override: Option<&str>) -> ExitCode {
    // Executing code cells needs the async kernel, so the whole site build runs on
    // a tokio runtime (mirrors the preview server's setup).
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            log::error(&format!("cannot start runtime: {e}"));
            return ExitCode::FAILURE;
        }
    };
    rt.block_on(build_site_async(root, out_override))
}

async fn build_site_async(root: &Path, out_override: Option<&str>) -> ExitCode {
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
    for page in &site.pages {
        let Ok(src) = std::fs::read_to_string(&page.input) else {
            log::warn(&format!("cannot read {}", page.input.display()));
            continue;
        };
        let base = page.input.parent().unwrap_or(root);
        let mut doc = qmd_fast_core::render_document_with_includes(&src, base);
        let mut exec = exec::Executor::with_freeze(freeze::page_path(&freeze_dir, &page.rel));
        doc.blocks = exec.run(std::mem::take(&mut doc.blocks)).await;
        kernel_unavailable |= exec.diagnostic().is_some();
        let resources = doc.includes.resources.clone();
        // Surface render warnings *and* broken cross-refs so a broken site doesn't
        // deploy silently (these previously only showed in the preview dev menu).
        let (html, warnings) = site.render_page_doc_warned(page, doc);
        for w in &warnings {
            log::warn(&format!("{}: {}", page.rel, w.message));
        }
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
        let mut ex = exec::Executor::with_freeze(freeze::page_path(&freeze_dir, &deck.url));
        doc.blocks = ex.run(std::mem::take(&mut doc.blocks)).await;
        kernel_unavailable |= ex.diagnostic().is_some();
        let stem = deck
            .url
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".html"))
            .unwrap_or("deck");
        let html = qmd_fast_core::render_doc_to_page(&doc, stem);
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
    ExitCode::SUCCESS
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
    println!("  build  <file.qmd> [out.html] [--out <dir>]");
    println!("                             render a self-contained HTML file");
    println!("                             (default <name>.html beside the source);");
    println!("                             --out <dir> writes <dir>/index.html and");
    println!("                             copies referenced assets for a portable folder");
    println!("  render <file.qmd>          render a full HTML page to stdout");
    println!("  blocks <file.qmd>          list block ids + sourcepos (debug)");
    println!(
        "  schema [--out <dir>]       emit JSON Schemas for _site.yml + front matter (editor autocomplete)"
    );
    println!();
    println!("ENV: QMD_FAST_PYTHON (kernel), QMD_FAST_OPEN (=--open),");
    println!("     QMD_FAST_HOST (=--host), QMD_FAST_NO_CLEAR,");
    println!("     QMD_FAST_NO_CACHE (skip the _freeze/ execution cache),");
    println!("     QMD_FAST_NO_EXEC (=--no-exec, never run code cells)");
}
