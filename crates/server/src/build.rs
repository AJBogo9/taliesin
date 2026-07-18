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
//! [`crate::build_budget`] (execution + the memory budget split), [`crate::log`], and
//! [`taliesin_core`] for rendering.
//!
//! **Load-bearing:** the concurrent site build (`build_site_async`/`PageOutcome`) defers
//! all logging and replays it in `site.pages` order, so a parallel build is byte-for-byte
//! identical to `--jobs 1`. Pinned by `tests/parallel_build_determinism.rs`. Do not
//! restructure that ordering or the per-page output/freeze isolation.

use crate::{build_budget, exec, freeze, log, warm_pool};
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
    /// `--format json` emits the build's static-lint diagnostics as `{diagnostics:[...]}`
    /// to stdout (for an agent/CI) instead of only the human log. Default `"human"`.
    format: &'a str,
}

/// Every long flag `build` accepts (drives the unknown-flag did-you-mean). `-j` is the
/// only short alias; it's not in this set (suggestions are between long flags).
const BUILD_FLAGS: &[&str] = &["--out", "--dir", "--jobs", "--strict", "--bare", "--format"];

/// Output-path extensions that name a format Taliesin does not produce (DX11). `build`
/// writes HTML; a second positional ending in one of these means the author expected format
/// *conversion* (a PDF/DOCX to open, a `.md` to round-trip) and would otherwise get HTML bytes
/// silently written into that file with a green exit — the academic persona's abandonment
/// moment. A denylist, not an allowlist: an extensionless or `.html`/`.htm`/unusual-but-named
/// target is the author's deliberate choice (HTML content in the file they asked for), not a
/// format-expectation trap. The CLI analog of `frontmatter::NON_HTML_FORMATS` (format *names*),
/// here matching output-path *file extensions*. Real PDF is a sanctioned future track (ROADMAP
/// Pillar IV / Wave 5, derived from the built HTML); this is only the interim guardrail.
const NON_HTML_OUTPUT_EXTS: &[&str] = &[
    "pdf", "docx", "doc", "odt", "rtf", "tex", "latex", "typ", "epub", "pptx", "ppt", "md",
    "markdown",
];

/// The friendly rejection for a `build … <out>` whose extension names a non-HTML format
/// ([`NON_HTML_OUTPUT_EXTS`]), or `None` when the output path is absent or an acceptable
/// target. Names the extension, hands over the concrete `.html` fix (the out path with its
/// extension swapped, so `dist/x.pdf` → `dist/x.html`), offers the browser Print-to-PDF escape
/// hatch, and points at the planned print track. `error: `-prefixed to match the other
/// `parse_build_args` errors (`cmd_build` prints it verbatim to stderr).
fn non_html_output_error(out_html: Option<&str>) -> Option<String> {
    let out = out_html?;
    let ext = Path::new(out).extension()?.to_str()?.to_ascii_lowercase();
    if !NON_HTML_OUTPUT_EXTS.contains(&ext.as_str()) {
        return None;
    }
    let html = Path::new(out).with_extension("html");
    let html = html.display();
    Some(format!(
        "error: `build` renders HTML only, but the output path `{out}` ends in `.{ext}`. \
         Write `{html}` instead (or omit it to build `{html}` beside the source). For a rough \
         PDF, open the built page and use your browser's Print to PDF; a real print/PDF track \
         is planned (ROADMAP Pillar IV)."
    ))
}

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
    let mut format: &str = "human";
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // `--format human|json`: mirror `check`'s flag exactly (value validated below).
            "--format" => match it.next().map(|s| s.as_str()) {
                Some(v) if v == "human" || v == "json" => format = v,
                other => {
                    return Err(format!(
                        "error: --format expects human or json (got {})",
                        other.unwrap_or("nothing")
                    ));
                }
            },
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
        "usage: taliesin build <file.tmd|dir> [out.html] [--out <dir>] [--strict] [--bare] [--jobs <N>]"
            .to_string()
    })?;
    // DX11: a format-implying output extension (`build doc.tmd doc.pdf`) is a hard error, not a
    // silent HTML-into-a-.pdf write. Checked here so it is caught for any invocation carrying
    // that second positional (even a contradictory `--out dist doc.pdf`, where it is otherwise
    // ignored), and stays unit-testable as pure arg parsing.
    if let Some(msg) = non_html_output_error(positionals.get(1).copied()) {
        return Err(msg);
    }
    Ok(BuildArgs {
        path,
        out_html: positionals.get(1).copied(),
        out_dir,
        strict,
        bare,
        jobs,
        format,
    })
}

/// `build <file.tmd> [out.html]`: write a self-contained HTML page to a file
/// (default `<stem>.html` beside the source). With `--out <dir>` it instead
/// writes `<dir>/index.html` and copies every referenced local asset alongside
/// (paths preserved), so the directory is deployable as-is. `render` is stdout.
pub(crate) fn cmd_build(args: &[String]) -> ExitCode {
    let started = std::time::Instant::now();
    // Positionals: <file> [out.html]. Flags: `--out <dir>` (alias `--dir`),
    // `--strict` (a cell error / broken-ref warning fails the build).
    let BuildArgs {
        path,
        out_html,
        out_dir,
        strict,
        bare,
        jobs,
        format,
    } = match parse_build_args(args) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };
    let json = format == "json";
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
        return build_site(Path::new(path), out_dir, strict, jobs, json);
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
    let (html, problems, diagnostics) = match executed {
        Ok(Ok(BuildResult::Page {
            html,
            problems,
            diagnostics,
        })) => (html, problems, diagnostics),
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

    // Structured diagnostics to stdout (the human log stays on stderr, so the JSON stream
    // pipes cleanly). The page is still written — `--format json` only changes the
    // *reporting* channel, not what a build produces.
    if json {
        println!("{}", crate::check::diagnostics_json(&diagnostics));
    }

    if let Some(dir) = out_dir {
        let code = build_dir(&html, base, Path::new(dir), started);
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
            log::built(&format!("{}{}", out.display(), elapsed_note(started)));
            strict_exit(ExitCode::SUCCESS, strict_fail, problems)
        }
        Err(e) => {
            log::error(&format!("cannot write {}: {e}", out.display()));
            ExitCode::FAILURE
        }
    }
}

/// `path:line: message` for a located warning, falling back to `path: message` for one the
/// renderer could not place. `fallback` names the document when the warning came from it
/// rather than from an `{{< include >}}`d file.
fn locate(w: &taliesin_core::render::Warning, fallback: &str) -> String {
    let file = w.file.as_deref().unwrap_or(fallback);
    match w.line {
        Some(l) => format!("{file}:{l}: {}", w.message),
        None => format!("{file}: {}", w.message),
    }
}

/// A `  ·  412ms` / `  ·  1.34s` suffix for a build summary. `preview` has always printed
/// how long startup took; a build printed nothing, so a cold kernel boot or a slow page was
/// invisible without wrapping the command in `time`.
fn elapsed_note(started: std::time::Instant) -> String {
    let d = started.elapsed();
    if d.as_secs() >= 1 {
        format!("  ·  {:.2}s", d.as_secs_f64())
    } else {
        format!("  ·  {}ms", d.as_millis())
    }
}

/// Turn a successful build into a failure when `--strict` saw problems (the page is
/// still written, but CI gets a non-zero exit). A pre-existing non-success `code`
/// (a write/create error) is returned unchanged.
fn strict_exit(code: ExitCode, strict_fail: bool, problems: usize) -> ExitCode {
    if strict_fail {
        warn_strict(problems);
        return ExitCode::FAILURE;
    }
    code
}

/// Log the `--strict` failure summary (shared by the single-doc and site build paths).
fn warn_strict(problems: usize) {
    log::error(&format!(
        "--strict: {problems} problem{} (cell error or located warning); failing the build",
        if problems == 1 { "" } else { "s" }
    ));
}

/// Count the executed output blocks that are uncaught runtime errors (their HTML
/// A block is a crashed *cell output* only when it is an actual executed-cell output
/// wrapper (`<div class="tali-output" …>`, produced by the executor) that carries the
/// `tali-error` marker. Keying on the wrapper as well as the marker avoids a false
/// positive on ordinary prose that merely *documents* the class in an inline `<code>`
/// span (HTML text content doesn't escape `"`, so `class="tali-error"` appears verbatim
/// in the rendered paragraph — e.g. the internals book's execution chapter).
fn is_cell_error_output(html: &str) -> bool {
    html.trim_start().starts_with("<div class=\"tali-output\"")
        && html.contains("class=\"tali-error\"")
}

/// carries the `tali-error` marker), logging a located warning per failing cell so a
/// crashing cell isn't baked into the build silently. Returns the count.
fn report_cell_errors(blocks: &[taliesin_core::Block], page_label: &str) -> usize {
    let mut n = 0;
    for b in blocks {
        if is_cell_error_output(&b.html) {
            n += 1;
            log::warn(&cell_error_message(page_label, b));
        }
    }
    n
}

/// Build `path` (a file or a site dir) to its default output and return the structured
/// `{diagnostics:[…]}` JSON (the `build` MCP tool's output), reusing the same build path as
/// `taliesin build … --format json`. A directory builds to `_site/`; a file writes
/// `<stem>.html` beside the source.
pub(crate) fn build_json(path: &Path) -> String {
    if path.is_dir() {
        let outcome = run_site_build(path, None, false, None);
        return crate::check::diagnostics_json(&outcome.diagnostics);
    }
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return crate::check::json_error(&format!("cannot read {}: {e}", path.display())),
    };
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    match crate::serve::guarded(|| {
        build_page_executing(&src, base, stem, taliesin_core::OutputMode::Build)
    }) {
        Ok(Ok(BuildResult::Page {
            html, diagnostics, ..
        })) => {
            let _ = std::fs::write(base.join(format!("{stem}.html")), &html);
            crate::check::diagnostics_json(&diagnostics)
        }
        Ok(Ok(BuildResult::Refused(msg))) => crate::check::json_error(&msg),
        Ok(Err(e)) => crate::check::json_error(&format!("cannot start runtime: {e}")),
        Err(p) => crate::check::json_error(&format!("build panicked on {}: {p}", path.display())),
    }
}

/// The citation keys a page actually cites, scanned from its source: every `@key` inside a
/// `[ … ]` span (the Pandoc bracketed-citation form), deduped in first-seen order. Bracketed
/// only, so a narrative `@fig-x` cross-reference or a `@decorator` in a code cell is never
/// mistaken for a citation; a key that is itself a cross-reference anchor is excluded too.
fn cited_keys(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'['
            && let Some(rel_close) = src[i + 1..].find(']')
        {
            let span = &src[i + 1..i + 1 + rel_close];
            let sb = span.as_bytes();
            let mut j = 0;
            while j < sb.len() {
                if sb[j] == b'@' {
                    let start = j + 1;
                    let mut k = start;
                    while k < sb.len()
                        && (sb[k].is_ascii_alphanumeric()
                            || matches!(sb[k], b'_' | b':' | b'.' | b'-' | b'/'))
                    {
                        k += 1;
                    }
                    let key = span[start..k].trim_end_matches(['.', ':', '-']);
                    if !key.is_empty()
                        && !taliesin_core::cite::is_xref_anchor(key)
                        && !out.iter().any(|k| k == key)
                    {
                        out.push(key.to_string());
                    }
                    j = k;
                } else {
                    j += 1;
                }
            }
            i += 1 + rel_close + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// The located "cell error" message for a crashed cell output — one string shape shared by
/// the single-doc and site build paths (and their structured-diagnostic mirror).
fn cell_error_message(page_label: &str, b: &taliesin_core::Block) -> String {
    let where_ = b
        .source_file
        .as_deref()
        .map(|f| format!("{f} "))
        .unwrap_or_default();
    format!(
        "cell error in {page_label} ({where_}@ {}): code cell raised an uncaught \
         exception; its traceback is baked into the output",
        b.sourcepos
    )
}

/// Structured "cell error" diagnostics (build-only additions over `check`'s superset), in
/// block order, for `--format json`.
fn cell_error_diagnostics(
    blocks: &[taliesin_core::Block],
    page_label: &str,
) -> Vec<crate::check::Diagnostic> {
    blocks
        .iter()
        .filter(|b| is_cell_error_output(&b.html))
        .map(|b| {
            crate::check::Diagnostic::new(
                page_label.to_string(),
                None,
                cell_error_message(page_label, b),
            )
        })
        .collect()
}

/// Render a single document to a self-contained HTML page, executing its code
/// cells first so figures / `ojs_define` outputs are baked in (mirrors the site
/// build's per-page execution). A missing kernel logs a warning and the cells fall
/// back to source, matching the preview's behaviour.
/// Result of building a single page: the rendered HTML (+ its `--strict` problem
/// count), or a `--bare` refusal whose message is user-facing.
enum BuildResult {
    Page {
        html: String,
        problems: usize,
        /// The located diagnostics, structured, for `--format json`. Same set the human
        /// log emits, in the same order.
        diagnostics: Vec<crate::check::Diagnostic>,
    },
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
        // The same diagnostics, structured, for `--format json` — collected in the exact
        // order they are logged so the two channels agree.
        let mut diagnostics: Vec<crate::check::Diagnostic> = Vec::new();
        // Malformed front-matter YAML: the live servers + `check` report this, but a
        // single-doc `build` used to skip it, so a typo'd `---` block built clean and
        // even passed `--strict`. Surface it (located) and count it toward --strict.
        if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(src) {
            log::warn(&format!("{fallback}:{line}: {message}"));
            diagnostics.push(crate::check::Diagnostic::new(
                fallback.to_string(),
                Some(line),
                message,
            ));
            problems += 1;
        }
        // Single-document build: confine includes/resources to the doc's own directory
        // (PT-2), matching single-doc preview.
        let mut doc = taliesin_core::render_document_with_includes_rooted(src, base, Some(base));
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
            // Located, as `check` reports them: a `--strict` failure should name the line.
            log::warn(&locate(w, fallback));
            diagnostics.push(crate::check::diag_from(w, fallback));
        }
        problems += doc.warnings.len();
        // `{{< embed >}}` only resolves in a SITE build, which also builds the
        // embedded target beside the page. A single-doc build ships the iframe but
        // not its target, so the embed would 404 — warn (and count it toward `--strict`,
        // like the render/xref warnings) instead of failing silently or passing green.
        let embeds = taliesin_core::render::embed_targets(src);
        for target in &embeds {
            let msg = format!(
                "{{{{< embed {target} >}}}} won't resolve in a single-doc build (its \
                 target isn't built); build the containing directory as a site, or \
                 inline the content instead."
            );
            log::warn(&msg);
            diagnostics.push(crate::check::Diagnostic::new(
                fallback.to_string(),
                None,
                msg,
            ));
        }
        problems += embeds.len();
        // Broken cross-refs (a single doc has no site to resolve them across pages),
        // so a `build` doesn't ship a dangling `@fig-`/`@sec-` link silently.
        let xrefs = taliesin_core::cite::validate_xrefs(&doc.blocks);
        for w in &xrefs {
            log::warn(&locate(w, fallback));
            diagnostics.push(crate::check::diag_from(w, fallback));
        }
        problems += xrefs.len();
        // The rest of the check-superset. These ran only in `check`, so a `--strict` build
        // exited 0 while shipping a missing image, a broken anchor or a dangling link —
        // and a green `--strict` reasonably reads as "safe to ship". Run *before* the code
        // cells execute, exactly as `check` does, so a figure a cell generates is never
        // linted as if the author had written it.
        let statics = crate::check::page_static_diagnostics(
            src,
            &doc.blocks,
            base,
            doc.format,
            crate::check::Scope::Standalone,
        );
        for w in &statics {
            log::warn(&locate(w, fallback));
            diagnostics.push(crate::check::diag_from(w, fallback));
        }
        problems += statics.len();
        // Persistent execution cache keyed off the doc's stem, beside the source.
        let mut ex =
            exec::Executor::with_freeze(freeze::page_path(&base.join("_freeze"), fallback))
                .in_dir(base);
        // Single-file build: no _site.yml, so resolve from the doc's own dir
        // (.venv / env / default), the same set_interpreters path the site build uses.
        ex.set_interpreters(
            crate::interpreter::resolve_python(None, base),
            crate::interpreter::resolve_r(None, base),
        );
        doc.blocks = ex.run(std::mem::take(&mut doc.blocks)).await;
        // The executor's own diagnostic already names the failing language and the
        // right env var (`TALIESIN_R` for R, `TALIESIN_PYTHON` otherwise) — use it
        // verbatim instead of a hardcoded python-only hint.
        if let Some(d) = ex.diagnostic() {
            log::warn(&d);
        }
        // A crashed cell bakes its traceback into the page (exit 0 + silent stderr
        // before this); log it located and count it toward `--strict`.
        problems += report_cell_errors(&doc.blocks, fallback);
        diagnostics.extend(cell_error_diagnostics(&doc.blocks, fallback));
        if mode == taliesin_core::OutputMode::Bare {
            warn_bare_exclusions(&doc);
        }
        BuildResult::Page {
            html: taliesin_core::render_doc_to_page(&doc, fallback, mode),
            problems,
            diagnostics,
        }
    }))
}

/// Write `<dir>/index.html` and copy each referenced local asset (an `src=`/
/// `href=` value pointing to an existing file under `base`) to the same relative
/// path under `dir`, leaving the HTML's paths untouched so the folder is portable.
fn build_dir(html: &str, base: &Path, dir: &Path, started: std::time::Instant) -> ExitCode {
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
        "{}  ·  {copied} asset{}{}",
        index.display(),
        if copied == 1 { "" } else { "s" },
        elapsed_note(started)
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
    /// The same findings, structured, for `--format json` — in the same order as `warnings`.
    diagnostics: Vec<crate::check::Diagnostic>,
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
    bundle: &AssetBundle,
) -> PageOutcome {
    let mut warnings = Vec::new();
    let mut diagnostics: Vec<crate::check::Diagnostic> = Vec::new();
    let Ok(src) = std::fs::read_to_string(&page.input) else {
        let msg = format!("cannot read {}", page.input.display());
        diagnostics.push(crate::check::Diagnostic::new(
            page.rel.clone(),
            None,
            msg.clone(),
        ));
        warnings.push(msg);
        return PageOutcome {
            warnings,
            diagnostics,
            problems: 0,
            kernel_unavailable: false,
            written: false,
        };
    };
    let base = page.input.parent().unwrap_or(root);
    let mut doc =
        taliesin_core::render_document_with_includes_scoped(&src, base, site.chapter_for(page));
    let mut problems = 0usize;
    // Malformed front-matter YAML: the lenient line-parser silently mis-extracts fields, so
    // the page builds with the wrong title/format. `check` reports it; the site build did not.
    if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(&src) {
        problems += 1;
        warnings.push(format!("{}:{line}: {message}", page.rel));
        diagnostics.push(crate::check::Diagnostic::new(
            page.rel.clone(),
            Some(line),
            message,
        ));
    }
    // The check-superset, over the page's blocks *before* its cells execute (as `check`
    // does). `Scope::InSite` omits the single-doc link rule: an intra-site `[x](other.tmd)`
    // rewrites to `other.html`, so only `validate_cross_page_links` can judge it, and that
    // runs once for the whole project after every page is built.
    let statics = crate::check::page_static_diagnostics(
        &src,
        &doc.blocks,
        base,
        doc.format,
        crate::check::Scope::InSite,
    );
    problems += statics.len();
    for w in &statics {
        warnings.push(locate(w, &page.rel));
        diagnostics.push(crate::check::diag_from(w, &page.rel));
    }
    let mut exec =
        exec::Executor::with_freeze(freeze::page_path(freeze_dir, &page.rel)).in_dir(base);
    // No progress sink (a build has no client), but name the page: a cold site build runs
    // pages concurrently, so bare interleaved `cell 2/4` lines belong to nobody.
    exec.set_progress(None, Some(page.rel.clone()));
    // Draw this page's Python kernel from the shared warm pool (when one booted) so a
    // page with code cells starts near-instantly instead of cold-booting. `None`
    // (unset interpreter / inert pool) cold-starts exactly as before.
    exec.set_warm_pool(warm_pool);
    // Resolve this project's interpreters (from _site.yml python:/r:, a .venv, env, or
    // default) against the site root, matching the warm pool the orchestrator booted.
    exec.set_interpreters(
        crate::interpreter::resolve_python(site.config.python.as_deref(), root),
        crate::interpreter::resolve_r(site.config.r.as_deref(), root),
    );
    doc.blocks = exec.run(std::mem::take(&mut doc.blocks)).await;
    let kernel_unavailable = exec.diagnostic().is_some();
    // A crashed cell bakes its traceback into the page; collect a located line + count it
    // (same shape/order as the sequential `report_cell_errors`, but deferred).
    for b in &doc.blocks {
        if is_cell_error_output(&b.html) {
            problems += 1;
            let msg = cell_error_message(&page.rel, b);
            diagnostics.push(crate::check::Diagnostic::new(
                page.rel.clone(),
                None,
                msg.clone(),
            ));
            warnings.push(msg);
        }
    }
    // Surface render warnings *and* broken cross-refs so a broken site doesn't deploy
    // silently (these previously only showed in the preview dev menu). Every page links
    // the shared `_assets/` bundle instead of inlining its own copy of the framework
    // CSS/JS; hrefs are depth-adjusted so a nested page's `../` prefix count matches.
    let app_css = asset_href(&page.url, &bundle.app_css);
    let katex_css = asset_href(&page.url, &bundle.katex_css);
    let app_js = asset_href(&page.url, &bundle.app_js);
    let mermaid_js = asset_href(&page.url, &bundle.mermaid_js);
    let jslibs_js = asset_href(&page.url, &bundle.jslibs_js);
    let ext = taliesin_core::ExternalAssets {
        app_css: &app_css,
        katex_css: &katex_css,
        app_js: &app_js,
        mermaid_js: &mermaid_js,
        jslibs_js: &jslibs_js,
    };
    let (html, render_warnings) = site.render_page_doc_external(page, doc, ext);
    for w in &render_warnings {
        // Located, the way `check` reports them. These carry a file + line and were being
        // flattened to `page.rel: message`, so a `--strict` failure named no line to fix.
        warnings.push(locate(w, &page.rel));
        diagnostics.push(crate::check::diag_from(w, &page.rel));
    }
    problems += render_warnings.len();
    let dest = out.join(&page.url);
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let written = match std::fs::write(&dest, html) {
        Ok(()) => true,
        Err(e) => {
            let msg = format!("cannot write {}: {e}", dest.display());
            diagnostics.push(crate::check::Diagnostic::new(
                page.rel.clone(),
                None,
                msg.clone(),
            ));
            warnings.push(msg);
            false
        }
    };
    PageOutcome {
        warnings,
        diagnostics,
        problems,
        kernel_unavailable,
        written,
    }
}

/// The result of a directory (site/book) build: whether it succeeded, and the structured
/// diagnostics it produced (for `--format json` on `build`/`publish`), in deterministic
/// page order.
pub(crate) struct SiteBuildOutcome {
    pub ok: bool,
    pub diagnostics: Vec<crate::check::Diagnostic>,
}

/// The resolved shared-asset filenames (content-hashed), computed once per site build.
struct AssetBundle {
    app_css: String,
    katex_css: String,
    app_js: String,
    mermaid_js: String,
    jslibs_js: String,
}

/// Minify + content-hash each shared blob, write it once under `<out>/_assets/`, and
/// return the (root-relative) filenames. Clears any stale `_assets/` first so old hashes
/// do not accumulate across rebuilds.
fn write_asset_bundle(out: &Path) -> std::io::Result<AssetBundle> {
    use taliesin_core::hash::fnv1a;
    let dir = out.join("_assets");
    let _ = std::fs::remove_dir_all(&dir); // own the lifecycle; clear stale hashes
    std::fs::create_dir_all(&dir)?;
    let named = |stem: &str, ext: &str, bytes: &str| -> std::io::Result<String> {
        let name = format!("{stem}.{:x}.{ext}", fnv1a(bytes));
        std::fs::write(dir.join(&name), bytes)?;
        Ok(format!("_assets/{name}"))
    };
    let app_css = named(
        "app",
        "css",
        &crate::minify::minify_css(&taliesin_core::shared_site_css()),
    )?;
    let katex_css = named(
        "katex",
        "css",
        &crate::minify::minify_css(taliesin_core::katex_css()),
    )?;
    let app_js = named(
        "app",
        "js",
        &crate::minify::minify_js(&taliesin_core::core_enhance_js()),
    )?;
    // Vendored libs are already minified: hash + write as-is (do not re-minify).
    let mermaid_js = named("mermaid", "js", &taliesin_core::mermaid_bundle_js())?;
    let jslibs_js = named("jslibs", "js", &taliesin_core::js_cell_libs_js())?;
    Ok(AssetBundle {
        app_css,
        katex_css,
        app_js,
        mermaid_js,
        jslibs_js,
    })
}

/// Rebase a root-relative `_assets/...` href for a page at `page_url` (e.g. `sub/p.html`
/// gets `../_assets/...`; a root page keeps `_assets/...`).
fn asset_href(page_url: &str, root_rel: &str) -> String {
    let depth = page_url.matches('/').count();
    format!("{}{root_rel}", "../".repeat(depth))
}

/// The "N drafts not published" build report line, or `None` when nothing was held back.
/// Singular/plural aware; names the rel paths so the author sees exactly what was excluded.
pub(crate) fn draft_report_line(excluded: &[String]) -> Option<String> {
    if excluded.is_empty() {
        return None;
    }
    let n = excluded.len();
    let noun = if n == 1 { "draft" } else { "drafts" };
    Some(format!("{n} {noun} not published: {}", excluded.join(", ")))
}

/// Run a directory (site/book) build to disk, returning whether it succeeded + its
/// structured diagnostics. Shared by `cmd_build`'s directory branch and `publish` (which
/// needs the success signal, not just an opaque `ExitCode`, plus the freedom to keep
/// working with the output dir afterward).
pub(crate) fn run_site_build(
    root: &Path,
    out_override: Option<&str>,
    strict: bool,
    jobs: Option<usize>,
) -> SiteBuildOutcome {
    // Executing code cells needs the async kernel, so the whole site build runs on a
    // tokio runtime (mirrors the preview server's setup). A multi-thread runtime so
    // concurrent page builds (each its own kernel) actually overlap on the CPU.
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let msg = format!("cannot start runtime: {e}");
            log::error(&msg);
            return SiteBuildOutcome {
                ok: false,
                diagnostics: vec![crate::check::Diagnostic::new(
                    root.display().to_string(),
                    None,
                    msg,
                )],
            };
        }
    };
    rt.block_on(build_site_async(root, out_override, strict, jobs))
}

fn build_site(
    root: &Path,
    out_override: Option<&str>,
    strict: bool,
    jobs: Option<usize>,
    json: bool,
) -> ExitCode {
    let outcome = run_site_build(root, out_override, strict, jobs);
    if json {
        println!("{}", crate::check::diagnostics_json(&outcome.diagnostics));
    }
    if outcome.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn build_site_async(
    root: &Path,
    out_override: Option<&str>,
    strict: bool,
    jobs: Option<usize>,
) -> SiteBuildOutcome {
    // Timed here rather than in `cmd_build`, so `publish` (which reaches the site build
    // through `run_site_build`) reports its build time too.
    let started = std::time::Instant::now();
    let site = taliesin_core::Site::discover(root);
    // Structured diagnostics accumulated in deterministic order (config → pages → site-wide),
    // for `--format json`. Mirrors the human log the build already emits.
    let mut diagnostics: Vec<crate::check::Diagnostic> = Vec::new();
    // A malformed `_site.yml` silently degrades the whole site to defaults (no nav, no
    // title, wrong output dir): a real `--strict` problem, unlike a benign missing config.
    let mut config_problems = 0usize;
    for w in &site.warnings {
        if taliesin_core::site::is_malformed_config_warning(w) {
            config_problems += 1;
            diagnostics.push(crate::check::Diagnostic::new(
                "_site.yml".to_string(),
                None,
                w.clone(),
            ));
        }
        log::warn(w);
    }
    // Drafts (`draft: true`) are excluded from the build; report what was held back so a
    // forgotten `draft:` flag is visible rather than a silently missing page.
    if let Some(line) = draft_report_line(&site.excluded_drafts) {
        log::info(&line);
    }
    if site.pages.is_empty() {
        // "no pages found" would be a lie when there ARE pages and every one is a draft —
        // and `--format json` shows only this diagnostic, so an agent would go hunting for
        // files that exist. Name the real cause instead.
        let msg = if site.excluded_drafts.is_empty() {
            format!("no .tmd pages found under {}", root.display())
        } else {
            format!(
                "no publishable .tmd pages under {}: all {} are drafts ({})",
                root.display(),
                site.excluded_drafts.len(),
                site.excluded_drafts.join(", ")
            )
        };
        log::error(&msg);
        diagnostics.push(crate::check::Diagnostic::new(
            root.display().to_string(),
            None,
            msg,
        ));
        return SiteBuildOutcome {
            ok: false,
            diagnostics,
        };
    }
    // Cross-page `@fig-`/`@eq-`/`@thm-` ref numbers are filled by `Site::discover`'s
    // render-harvest (shared with the live preview), so no separate build-time pass here.
    let out = match out_override {
        Some(d) => PathBuf::from(d),
        None => root.join(site.output_dir()),
    };
    if let Err(e) = std::fs::create_dir_all(&out) {
        let msg = format!("cannot create {}: {e}", out.display());
        log::error(&msg);
        diagnostics.push(crate::check::Diagnostic::new(
            root.display().to_string(),
            None,
            msg,
        ));
        return SiteBuildOutcome {
            ok: false,
            diagnostics,
        };
    }
    let out = out.canonicalize().unwrap_or(out);

    // Refuse to build into the source directory: `mirror_assets` and the page writes
    // would copy files onto themselves, and `fs::copy` truncates the destination
    // first — silently zeroing the user's own assets. (Triggered by `output-dir: .`
    // or `--out <root>`.)
    if root.canonicalize().is_ok_and(|r| r == out) {
        let msg = format!(
            "output directory is the source directory ({}); refusing to build in place \
             (it would overwrite/truncate your source files). Use a different `output-dir:` or `--out <dir>`.",
            out.display()
        );
        log::error(&msg);
        diagnostics.push(crate::check::Diagnostic::new(
            root.display().to_string(),
            None,
            msg,
        ));
        return SiteBuildOutcome {
            ok: false,
            diagnostics,
        };
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
    let (asset_paths, skipped_residue) = mirror_assets(root, &out);
    if !skipped_residue.is_empty() {
        log::warn(&format!(
            "skipped {} build-cache dir(s) (not deployed): {}",
            skipped_residue.len(),
            skipped_residue.join(", ")
        ));
    }

    // The shared framework CSS/JS, written once as content-hashed files under `_assets/`
    // (dedups what would otherwise be a copy inlined into every page); every page below
    // links to it instead of shipping its own inline blob.
    let bundle = match write_asset_bundle(&out) {
        Ok(b) => b,
        Err(e) => {
            let msg = format!("cannot write {}/_assets: {e}", out.display());
            log::error(&msg);
            diagnostics.push(crate::check::Diagnostic::new(
                root.display().to_string(),
                None,
                msg,
            ));
            return SiteBuildOutcome {
                ok: false,
                diagnostics,
            };
        }
    };

    // 2. Render each page with chrome + rewritten links. Code cells run against a
    //    fresh kernel per page (clean state per document; pages with no cells never
    //    boot one), so the static `_site/` carries real computed outputs.
    //
    //    Pages are independent (each writes only its own output + `_freeze/<rel>.json`,
    //    runs its own kernel in its own cwd), so we build up to `cap` of them at once.
    //    Determinism is preserved: scheduling only changes *when* a page builds, never
    //    *what* it produces, and per-page outcomes (file bytes + log lines) are replayed
    //    in `site.pages` order so a `--jobs N` build is byte- and log-identical to the
    //    sequential one. `--jobs 1` takes the in-order serial path (it is not the default;
    //    the default is auto, which sizes the cap against free RAM and the core count).
    //    Cross-page ordering edges (a page that must build after another) are deferred to
    //    Task 9; here every dirty page is treated as independent.
    // Plan the build's concurrency: how many pages at once, and how many kernels to
    // pre-warm beside them. `build_plan` keys off who chose the number — an explicit
    // `--jobs N` is the user's PAGE count and is honored exactly (the warm pool is
    // additive on top); auto mode splits one memory-aware budget between the two
    // (`warm_pool + build_kernels <= cap`). The build semaphore is sized to
    // `build_kernels`; the warm pool pre-warms `warm_pool` so each page build can draw a
    // hot kernel instead of paying a cold boot. Determinism is untouched: a pooled kernel
    // runs the same ipykernel with the same preambles as a cold one, so it produces
    // identical bytes — the pool only changes *when* a kernel is ready, never *what* it
    // computes.
    //
    // Resolve the interpreter BEFORE consulting the plan's warm size: whether a pool can
    // exist at all depends on this resolution, and the log line below must not claim to
    // pre-warm a pool that never boots (M1).
    let interp_python = crate::interpreter::resolve_python(site.config.python.as_deref(), root);
    let plan = build_budget::build_plan(jobs, build_budget::PER_KERNEL_MB);

    // The one process-wide warm pool for this build. `None` (so every page cold-starts
    // exactly as today) when `TALIESIN_PYTHON` is unset or the forkserver can't boot;
    // dropped at the end of this fn, killing the daemon + idle kernels. The pool boots
    // only for a concrete interpreter choice (not the bare python3 default), and each
    // page/deck executor resolves the same way.
    let warm_pool = warm_pool::warm_pool_for_build(plan.warm_pool, &interp_python).await;

    // Count only the kernels that ACTUALLY exist. A pool that declined to boot (bare
    // `python3`) or failed to (`is_warm()` false) holds no kernels, so it costs no RAM,
    // must not be announced as a purchase, and must not cost build slots (M1).
    // `build_cap` applies that last rule where it means something: auto mode hands the
    // unbooted slots back to the build; under an explicit --jobs the pool never held any.
    let warmed = match &warm_pool {
        Some(p) if p.is_warm() => plan.warm_pool,
        _ => 0,
    };
    let build_cap = plan.build_cap(warmed).max(1);
    log::info(&if warmed > 0 {
        format!("building with up to {build_cap} parallel page(s); pre-warming {warmed} kernel(s)")
    } else {
        // Say nothing about pre-warming rather than "pre-warming 0 kernel(s)": the point
        // of the line is what the build is doing, and it is not pre-warming anything.
        format!("building with up to {build_cap} parallel page(s)")
    });
    let mut pages = 0usize;
    let mut kernel_unavailable = false;
    // `--strict` problem tally across the whole site: a malformed `_site.yml`, per-page
    // located warnings, broken cross-refs, crashed cells, and page-task panics (each
    // already logged where it occurs).
    let mut problems = config_problems;

    // Build into a slot per page (indexed by page order) so results aggregate
    // deterministically regardless of completion order. A `Semaphore` of size
    // `build_cap` bounds how many build kernels run at once (memory-aware, reconciled
    // with the warm pool); the pool lookup / file write each page does is on its own
    // paths, so no lock is held across the `.await`.
    let site = std::sync::Arc::new(site);
    let out = std::sync::Arc::new(out);
    let freeze_dir = std::sync::Arc::new(freeze_dir);
    let root_arc = std::sync::Arc::new(root.to_path_buf());
    let bundle = std::sync::Arc::new(bundle);
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(build_cap));
    let mut set: tokio::task::JoinSet<(usize, PageOutcome)> = tokio::task::JoinSet::new();
    for (idx, _page) in site.pages.iter().enumerate() {
        let site = site.clone();
        let out = out.clone();
        let freeze_dir = freeze_dir.clone();
        let root_arc = root_arc.clone();
        let bundle = bundle.clone();
        let sem = sem.clone();
        let warm_pool = warm_pool.clone();
        set.spawn(async move {
            // Hold a permit only for this page's build; dropping it on return frees the
            // slot for the next queued page. The permit guards kernel count, not any
            // shared data structure, so nothing is locked across the build's `.await`.
            let _permit = sem.acquire().await.expect("build semaphore not closed");
            let page = &site.pages[idx];
            let outcome = build_one_page(
                &site,
                page,
                &freeze_dir,
                &out,
                &root_arc,
                warm_pool,
                &bundle,
            )
            .await;
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
                let msg = format!("page build task failed: {e}");
                log::error(&msg);
                diagnostics.push(crate::check::Diagnostic::new(
                    root.display().to_string(),
                    None,
                    msg,
                ));
            }
        }
    }

    // Replay every page's deferred logs + tally counters in page order, so the build's
    // output is identical whether it ran 1-wide or N-wide.
    for outcome in outcomes.into_iter().flatten() {
        for w in &outcome.warnings {
            log::warn(w);
        }
        diagnostics.extend(outcome.diagnostics);
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
        ex.set_interpreters(
            crate::interpreter::resolve_python(site.config.python.as_deref(), root),
            crate::interpreter::resolve_r(site.config.r.as_deref(), root),
        );
        ex.set_progress(None, Some(deck.url.clone()));
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
             (set TALIESIN_PYTHON to a python with ipykernel)",
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
    // Cross-page hover-preview snippet index, lazy-loaded by 12-link-preview.js (pages
    // point at it via window.TALIESIN_HOVER_URL). Same file:// rationale as search-index.js.
    if !site.hover_index_json.is_empty() {
        let js = format!("window.TALIESIN_HOVER_INDEX={};", site.hover_index_json);
        if let Err(e) = std::fs::write(out.join("hover-index.js"), js) {
            log::warn(&format!("cannot write hover-index.js: {e}"));
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
    // SEO + discoverability sidecars: emitted only when `url:` is set (absolute URLs
    // are mandatory for feeds/sitemap/JSON-LD). All auto-derived from the site's own
    // content; the author writes nothing SEO-specific.
    let mut seo_written: Vec<PathBuf> = Vec::new();
    let mut card_paths: Vec<PathBuf> = Vec::new();
    if site.config.url.is_some() {
        let mut emit = |rel: &str, body: String| {
            let dest = out.join(rel);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&dest, body) {
                Ok(()) => seo_written.push(PathBuf::from(rel)),
                Err(e) => log::warn(&format!("cannot write {rel}: {e}")),
            }
        };
        for (path, xml) in site.atom_feeds() {
            emit(&path, xml);
        }
        if let Some(x) = site.sitemap() {
            emit("sitemap.xml", x);
        }
        if let Some(x) = site.robots() {
            emit("robots.txt", x);
        }
        if let Some(x) = site.llms_txt() {
            emit("llms.txt", x);
        }
        if let Some(x) = site.llms_full_txt() {
            emit("llms-full.txt", x);
        }

        // Per-page cited-references sidecar (reader-side AI/crawler legibility): the citation
        // keys a page actually cites, as `<page>.citations.json`, so a machine can read a
        // page's references without parsing its prose. Written via `emit` so the stale-sweep
        // keeps it; skips pages that cite nothing.
        for page in &site.pages {
            if page.url == "404.html" {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let keys = cited_keys(&src);
            if keys.is_empty() {
                continue;
            }
            if let Some(stem) = page.url.strip_suffix(".html") {
                let body = serde_json::json!({ "page": page.url, "cited": keys });
                emit(
                    &format!("{stem}.citations.json"),
                    serde_json::to_string_pretty(&body).unwrap_or_default(),
                );
            }
        }

        // OG social cards: one branded 1200x630 PNG per content page (og:image points
        // at /og/<hash>.png). Identical specs dedupe by hash. A failed encode/write is a
        // warning, never a build abort — the page still ships, its og:image just 404s.
        let mut seen_cards: std::collections::HashSet<String> = std::collections::HashSet::new();
        for page in &site.pages {
            if page.url == "404.html" {
                continue;
            }
            let spec = taliesin_core::site::card_spec(&site, page);
            let rel = taliesin_core::site::card_rel_path(&spec);
            if !seen_cards.insert(rel.clone()) {
                continue;
            }
            let dest = out.join(&rel);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // The bundled card font is Latin-only, and a missing glyph draws as a tofu box
            // with a real advance — the card lays out and encodes "successfully" while
            // reading as garbage. Nobody ever looks at a social card, so say so here or it
            // is never caught. A diagnostic, not a knob: the author rephrases the title.
            let missing = taliesin_core::site::uncovered_glyphs(&spec);
            if !missing.is_empty() {
                let chars: Vec<String> = missing.iter().map(|c| format!("`{c}`")).collect();
                log::warn(&format!(
                    "{}: social card font has no glyph for {} (they render as blank boxes)",
                    page.input.display(),
                    chars.join(" ")
                ));
            }
            match std::fs::write(&dest, taliesin_core::site::render_card(&spec)) {
                Ok(()) => card_paths.push(PathBuf::from(&rel)),
                Err(e) => log::warn(&format!("cannot write {rel}: {e}")),
            }
        }
    }
    let seo_note = if seo_written.is_empty() {
        String::new()
    } else {
        format!("  ·  {} SEO file(s)", seo_written.len())
    };
    let deck_note = if decks > 0 {
        format!("  ·  {decks} deck{}", if decks == 1 { "" } else { "s" })
    } else {
        String::new()
    };

    // Sweep stale output: a page or asset removed/renamed in the source must not linger
    // across rebuilds (the output tree is a mirror of what this build produced). Anything
    // in `out` that this build didn't write — and isn't dot/underscore deploy metadata —
    // is stale. Runs before the referenced-source pass so a no-longer-linked `.md`/`.scss`
    // (a SKIP_EXT file `mirror_assets` never mirrors, so it's absent from `keep`) is swept
    // too; the pass below then re-ships only the sources the current pages still link.
    let mut keep: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    keep.extend(site.pages.iter().map(|p| PathBuf::from(&p.url)));
    keep.extend(site.decks.iter().map(|d| PathBuf::from(&d.url)));
    keep.extend(asset_paths.iter().cloned());
    keep.extend(card_paths.iter().cloned());
    keep.insert(PathBuf::from("404.html"));
    if !site.search_index_json.is_empty() && site.search_index_json != "[]" {
        keep.insert(PathBuf::from("search-index.js"));
    }
    if !site.hover_index_json.is_empty() {
        keep.insert(PathBuf::from("hover-index.js"));
    }
    keep.extend(seo_written.iter().cloned());
    let swept = sweep_stale(&out, &keep);
    if swept > 0 {
        log::info(&format!(
            "swept {swept} stale file{} no longer produced",
            if swept == 1 { "" } else { "s" }
        ));
    }

    // The site-wide half of the check-superset: rules only the whole page registry can
    // judge. A broken cross-page link and a typo'd category are exactly the defects a
    // `--strict` build used to deploy with exit 0.
    for (rel, w) in site.validate_cross_page_links() {
        problems += 1;
        log::warn(&locate(&w, &rel));
        diagnostics.push(crate::check::diag_from(&w, &rel));
    }
    for (rel, w) in site.validate_categories() {
        problems += 1;
        log::warn(&locate(&w, &rel));
        diagnostics.push(crate::check::diag_from(&w, &rel));
    }

    // Second asset pass: ship source files (`.md`/`.scss`/…) that pages actually link to.
    // mirror_assets drops them by extension (publish hygiene), but a *referenced* source is
    // an intentional download — skipping it would leave a dead link on a green build.
    let assets = asset_paths.len() + deploy_referenced_sources_for_site(root, &out);

    log::built(&format!(
        "{}  ·  {pages} page{}  ·  {assets} asset{}{search}{deck_note}{not_found}{seo_note}{}",
        out.display(),
        if pages == 1 { "" } else { "s" },
        if assets == 1 { "" } else { "s" },
        elapsed_note(started),
    ));
    // In `--strict` mode a problem (crashed cell / located warning / broken ref)
    // fails the build after writing it, so CI catches a broken site.
    let strict_fail = strict && problems > 0;
    if strict_fail {
        warn_strict(problems);
    }
    SiteBuildOutcome {
        ok: !strict_fail,
        diagnostics,
    }
}

/// Source-only file extensions that are build *inputs* / prose / stylesheet sources,
/// never referenced by the rendered HTML, so they are not mirrored into the deploy:
/// `.tmd` (rendered separately), `.bib` (citations resolved server-side), `.Rproj`
/// (an editor project file), `.md` (prose/planning the renderer never serves), and `.scss`/
/// `.sass` (stylesheet sources — output references the compiled `.css`). Keeping these
/// out of `_site/` is publish hygiene: a stray `notes.md` or `theme.scss` in the source
/// tree never leaks onto the live site. (To deploy a private *binary* asset selectively,
/// the `_`/`.`-prefix convention still applies; these are excluded by kind.)
const SKIP_EXT: &[&str] = &["tmd", "bib", "Rproj", "md", "scss", "sass"];

/// Copy every non-source file under `root` into `out`, mirroring the directory tree.
/// Skips: source-only extensions ([`SKIP_EXT`]: `.tmd`/`.bib`/`.Rproj`/`.md`/`.scss`/
/// `.sass`), `_`-prefixed and dot entries (`_site.yml`, `_includes`, `_site`, `.RData`, …),
/// build-tool cache/artifact dirs (`*_cache/`, `*_files/`, knitr/RMarkdown
/// residue), and the output dir itself.
/// Returns `(out-relative paths copied, names of skipped cache dirs)` so the caller can
/// report residue it dropped rather than silently omitting it, and knows which output
/// files this build owns (for the stale-file sweep).
fn mirror_assets(root: &Path, out: &Path) -> (Vec<PathBuf>, Vec<String>) {
    fn walk(
        dir: &Path,
        root: &Path,
        out: &Path,
        seen: &mut std::collections::HashSet<PathBuf>,
        copied: &mut Vec<PathBuf>,
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
                // Build-tool cache/artifact dirs (knitr/RMarkdown) are residue, not
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
                    copied.push(rel.to_path_buf());
                }
            }
        }
    }
    let mut copied = Vec::new();
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

/// Delete files under `out` that this build did not produce, so a page or asset removed
/// or renamed in the source doesn't linger in the deploy across rebuilds. `keep` holds
/// every out-relative path the build wrote (pages, decks, mirrored assets, the index /
/// 404 files). Dot- and underscore-prefixed entries are never descended into or deleted:
/// the build never emits them, so anything there (`.git`, `.nojekyll`, `_headers`,
/// `_redirects`) is deploy metadata the author placed deliberately, mirroring the same
/// prefix rule [`mirror_assets`] uses to keep them *out* of the deploy. Symlinks are
/// skipped whole (never followed): the build never emits one, so a symlink in `out` is an
/// author's deliberate mount (e.g. a large shared media dir linked in) — following it
/// would delete *through* the link into their content and risk a cycle. Directories left
/// empty by the sweep are pruned. Returns the number of files swept.
fn sweep_stale(out: &Path, keep: &std::collections::HashSet<PathBuf>) -> usize {
    fn walk(dir: &Path, out: &Path, keep: &std::collections::HashSet<PathBuf>, swept: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.starts_with('.') || name.starts_with('_') {
                continue;
            }
            // `file_type()` does not follow the link, so a symlinked dir/file is left alone.
            if entry.file_type().is_ok_and(|t| t.is_symlink()) {
                continue;
            }
            if p.is_dir() {
                walk(&p, out, keep, swept);
                // Prune the directory if the sweep emptied it (its files were all stale).
                if std::fs::read_dir(&p).is_ok_and(|mut e| e.next().is_none()) {
                    let _ = std::fs::remove_dir(&p);
                }
            } else if let Ok(rel) = p.strip_prefix(out)
                && !keep.contains(rel)
                && std::fs::remove_file(&p).is_ok()
            {
                *swept += 1;
            }
        }
    }
    let mut swept = 0;
    walk(out, out, keep, &mut swept);
    swept
}

/// Unique local `src=`/`href=` values in `html` (skips external URLs, protocol-
/// relative refs, data URIs, in-page anchors, and other schemes).
fn local_refs(html: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = html.as_bytes();
    for attr in ["src=\"", "href=\""] {
        let mut i = 0;
        while let Some(pos) = html[i..].find(attr) {
            let at = i + pos; // first byte of the attribute name
            let start = at + attr.len();
            let Some(len) = html[start..].find('"') else {
                break;
            };
            let val = &html[start..start + len];
            i = start + len;
            // The match must *begin* an attribute name, not end one: `data-qmd-src="…"`
            // (the click-to-source attribute on listing cards) contains `src="`, and
            // harvesting it published every post's `.tmd` source into `_site/`. Only a
            // tag opener or whitespace can precede an attribute name; a multi-byte lead
            // byte is neither, so the byte test is safe.
            if at > 0 && bytes[at - 1] != b'<' && !bytes[at - 1].is_ascii_whitespace() {
                continue;
            }
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
    fn draft_report_line_counts_and_names() {
        assert_eq!(draft_report_line(&[]), None);
        assert_eq!(
            draft_report_line(&["only.tmd".into()]),
            Some("1 draft not published: only.tmd".to_string())
        );
        assert_eq!(
            draft_report_line(&["a.tmd".into(), "posts/b/index.tmd".into()]),
            Some("2 drafts not published: a.tmd, posts/b/index.tmd".to_string())
        );
    }

    #[test]
    fn elapsed_note_switches_from_ms_to_seconds() {
        use std::time::{Duration, Instant};
        let ms = elapsed_note(Instant::now());
        assert!(ms.ends_with("ms"), "sub-second builds report ms: {ms}");
        let slow = elapsed_note(Instant::now() - Duration::from_millis(1340));
        assert!(
            slow.contains("1.34s"),
            "second-scale builds report s: {slow}"
        );
        // The summary joins on the same separator the rest of the line uses.
        assert!(ms.starts_with("  ·  "), "{ms}");
    }

    #[test]
    fn local_refs_matches_whole_attributes_not_substrings() {
        // `data-qmd-src="…"` (the click-to-source attribute on listing cards) *contains*
        // the substring `src="`, so a bare search harvested each post's `.tmd` and
        // `deploy_referenced_sources` published the sources into `_site/`.
        let refs = local_refs(
            r#"<a class="card" data-qmd-src="posts/a/index.tmd" href="posts/a/index.html">
                 <img src="posts/a/thumb.png" alt="">
               </a>
               <div data-qmd-src="_site.yml"></div>
               <p>A <a href="notes.md">note</a> you may download.</p>
               <img
                 src="wrapped.png">"#,
        );
        assert!(refs.contains(&"posts/a/index.html".to_string()), "{refs:?}");
        assert!(refs.contains(&"posts/a/thumb.png".to_string()), "{refs:?}");
        assert!(refs.contains(&"notes.md".to_string()), "{refs:?}");
        // A newline between the tag name and the attribute is still an attribute start.
        assert!(refs.contains(&"wrapped.png".to_string()), "{refs:?}");
        // The dev-only attributes are not references to deploy.
        assert!(!refs.contains(&"posts/a/index.tmd".to_string()), "{refs:?}");
        assert!(!refs.contains(&"_site.yml".to_string()), "{refs:?}");
    }

    #[test]
    fn deploy_referenced_sources_ships_a_linked_source_but_not_a_card_target() {
        // The function exists to ship a *linked* source (a `.md` download, a `.scss`
        // offered for inspection). A listing card's `data-qmd-src` is not a link.
        let dir = tmp_dir("deploy-refs");
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(dir.join("index.tmd"), "x").unwrap();
        fs::write(dir.join("notes.md"), "y").unwrap();

        let html = r#"<a data-qmd-src="index.tmd" href="index.html">card</a>
                      <a href="notes.md">the source</a>"#;
        let copied = deploy_referenced_sources(html, &dir, &out);

        assert!(
            out.join("notes.md").is_file(),
            "an explicitly linked source ships"
        );
        assert!(
            !out.join("index.tmd").exists(),
            "a listing card must not publish the post's source"
        );
        assert_eq!(copied, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tali-build-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn build_args_distinguish_outhtml_positional_from_out_dir_flag() {
        // `BuildArgs` borrows from the argv, so each case binds its vec first.
        let argv = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<String>>();

        // file only: path, no [out.html] target, no portable-folder dir.
        let a = argv(&["taliesin", "build", "doc.tmd"]);
        let p = parse_build_args(&a).unwrap();
        assert_eq!((p.path, p.out_html, p.out_dir), ("doc.tmd", None, None));

        // second positional = the [out.html] single-file target.
        let a = argv(&["taliesin", "build", "doc.tmd", "out.html"]);
        let p = parse_build_args(&a).unwrap();
        assert_eq!(
            (p.path, p.out_html, p.out_dir),
            ("doc.tmd", Some("out.html"), None)
        );

        // --out <dir> is the portable-folder flag, distinct from the positional.
        let a = argv(&["taliesin", "build", "doc.tmd", "--out", "site"]);
        let p = parse_build_args(&a).unwrap();
        assert_eq!(
            (p.path, p.out_html, p.out_dir),
            ("doc.tmd", None, Some("site"))
        );

        // --out never captures a following flag as its directory: a value-less --out is
        // now a HARD ERROR (rather than silently dropping the flag + writing <stem>.html).
        let err = parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "--out", "--bare"]))
            .expect_err("value-less --out errors");
        assert!(err.contains("--out") && err.contains("requires"), "{err}");
        // --out at the very end (no following token) is the same hard error.
        let err = parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "--out"]))
            .expect_err("trailing --out errors");
        assert!(err.contains("--out"), "{err}");
        // --dir is the alias and errors the same way.
        assert!(parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "--dir"])).is_err());

        // flags may appear anywhere; both positionals still bind in order.
        let a = argv(&["taliesin", "build", "--bare", "doc.tmd", "out.html"]);
        let p = parse_build_args(&a).unwrap();
        assert!(p.bare);
        assert_eq!((p.path, p.out_html), ("doc.tmd", Some("out.html")));

        // a missing path is a usage error.
        assert!(parse_build_args(&argv(&["taliesin", "build"])).is_err());
        assert!(parse_build_args(&argv(&["taliesin", "build", "--strict"])).is_err());
    }

    #[test]
    fn build_unknown_flag_errors_with_did_you_mean() {
        let argv = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        // A typo'd flag is a hard error (not silently dropped) and suggests the real one.
        let err = parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "--stict"]))
            .expect_err("--stict must error");
        assert!(err.contains("--stict"), "names the bad flag: {err}");
        assert!(err.contains("--strict"), "suggests the near match: {err}");
        // A flag with no near match still errors (no wild guess).
        let err = parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "--frobnicate"]))
            .expect_err("unknown flag must error");
        assert!(err.contains("--frobnicate"), "{err}");
        assert!(!err.contains("did you mean"), "no wild guess: {err}");
        // The real flags still parse (no regression).
        assert!(parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "--strict"])).is_ok());
        assert!(parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "--bare"])).is_ok());
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
        fs::write(root.join("post.tmd"), b"x").unwrap(); // .tmd source -> not deployed
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
        assert!(
            !out.join("post.tmd").exists(),
            ".tmd is the native source extension -> not deployed as a stray asset"
        );
        assert!(!out.join("index_cache").exists(), "*_cache dir is residue");
        assert!(!out.join("report_files").exists(), "*_files dir is residue");
        assert!(!out.join("_freeze").exists(), "_-prefixed dir skipped");
        assert!(!out.join(".RData").exists(), "dotfile skipped");
        assert_eq!(
            copied,
            vec![PathBuf::from("keep.png")],
            "only keep.png is deployed"
        );
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

    #[test]
    fn report_cell_errors_ignores_prose_that_merely_documents_the_class() {
        // Ordinary prose describing the tali-error class in an inline <code> span (e.g.
        // the internals book's execution chapter) is not wrapped in the tali-output cell
        // marker, so it must not be miscounted as a crashed cell: `class="tali-error"`
        // appears unescaped in the block's HTML (HTML doesn't escape `"` in text content),
        // but there is no real cell output here.
        let blocks = vec![output_block(
            "<p>anything carrying <code>class=\"tali-error\"</code>: an exception</p>",
        )];
        assert_eq!(report_cell_errors(&blocks, "page"), 0);
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
        let src = "---\nformat: deck\n---\n\n# Slide one\n\n## Slide two\n";
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

#[cfg(test)]
mod dx11_tests {
    use super::*;

    /// Build a `build` argv (`["taliesin", "build", …tokens]`) so `parse_build_args`,
    /// which reads `args[2..]`, sees exactly the tokens after "build".
    fn argv(tokens: &str) -> Vec<String> {
        std::iter::once("taliesin")
            .chain(std::iter::once("build"))
            .chain(tokens.split(' '))
            .map(String::from)
            .collect()
    }

    /// DX11: a format-implying output extension (`.pdf`, `.docx`, `.tex`, …) is rejected with a
    /// friendly HTML-only message; an HTML target, an extensionless name, and a plain `.txt`
    /// are all left alone (the denylist targets format-conversion traps, not every filename).
    #[test]
    fn non_html_output_error_flags_format_extensions() {
        let m = non_html_output_error(Some("methods.pdf")).expect(".pdf must be rejected");
        assert!(m.contains(".pdf"), "names the extension: {m}");
        assert!(m.contains("HTML only"), "states HTML-only: {m}");
        assert!(m.contains("methods.html"), "suggests the .html fix: {m}");
        assert!(
            m.contains("Print"),
            "offers the browser-Print escape hatch: {m}"
        );

        // Case-insensitive.
        assert!(
            non_html_output_error(Some("out.PDF")).is_some(),
            ".PDF (caps)"
        );
        // The rest of the denylist.
        for bad in [
            "slides.pptx",
            "paper.docx",
            "x.tex",
            "x.typ",
            "x.md",
            "a.epub",
            "a.rtf",
        ] {
            assert!(non_html_output_error(Some(bad)).is_some(), "reject {bad}");
        }

        // Left alone: HTML targets, extensionless, and non-format extensions.
        for ok in ["page.html", "page.htm", "draft", "notes.txt"] {
            assert!(non_html_output_error(Some(ok)).is_none(), "allow {ok}");
        }
        assert!(
            non_html_output_error(None).is_none(),
            "no second positional"
        );

        // The .html suggestion keeps any directory component.
        let nested = non_html_output_error(Some("dist/methods.pdf")).unwrap();
        assert!(
            nested.contains("dist/methods.html"),
            "suggestion keeps the dir: {nested}"
        );
    }

    /// DX11: the rejection is wired through `parse_build_args`, and a valid `.html` target
    /// still parses (regression guard — the guard must not reject legitimate output paths).
    #[test]
    fn parse_build_args_rejects_pdf_output() {
        // `BuildArgs<'a>` borrows from the argv, so each argv is bound before parsing.
        let pdf = argv("doc.tmd out.pdf");
        let err = parse_build_args(&pdf).expect_err(".pdf output must Err");
        assert!(err.contains(".pdf"), "names the extension: {err}");
        assert!(err.contains("HTML only"), "states HTML-only: {err}");

        let html = argv("doc.tmd out.html");
        let ok = parse_build_args(&html).expect(".html output parses");
        assert_eq!(ok.out_html, Some("out.html"), "html target preserved");

        // No second positional: nothing to reject.
        let none = argv("doc.tmd");
        let bare = parse_build_args(&none).expect("no out path parses");
        assert_eq!(bare.out_html, None);
    }
}

#[cfg(test)]
mod asset_bundle_tests {
    use super::*;

    /// The vendored libs ship already minified, so `write_asset_bundle` hashes and writes them
    /// as-is rather than re-minifying. That was asserted only by a code comment, which is not
    /// a thing that fails: this pins the bytes.
    ///
    /// Note what this does NOT rest on. `minify_js` is separately proven token-identical on
    /// both bundles (`minify::tests::js_minify_is_token_identical_on_the_vendored_bundles_too`),
    /// so the bypass is a build-cost choice — skipping a megabyte of pointless rescanning — and
    /// not a safety rail. If a future change routes them through the minifier deliberately,
    /// that is a trade to reconsider, not a bug to fix by re-adding the bypass.
    #[test]
    fn vendored_libs_are_written_verbatim_not_reminified() {
        // pid + a stem, matching `tali-build-{pid}-{name}` / `tali-mirror-{pid}-{name}` in this
        // file: tests in one binary share a pid and run on threads, so a bare-pid path is safe
        // only while exactly one test uses it.
        let dir = std::env::temp_dir().join(format!(
            "tali-bundle-{}-vendored-verbatim",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let bundle = write_asset_bundle(&dir).expect("write bundle");

        let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).expect("read asset");
        // `assert_eq!` on two megabyte bundles prints BOTH on failure (~3.5MB of minified
        // vendor code), burying the one line that says what broke. Compare, then report short.
        assert!(
            read(&bundle.mermaid_js) == taliesin_core::mermaid_bundle_js(),
            "mermaid was rewritten on the way to disk"
        );
        assert!(
            read(&bundle.jslibs_js) == taliesin_core::js_cell_libs_js(),
            "the {{js}}-cell libs were rewritten on the way to disk"
        );
        // Control: the hand-written bundle IS minified, or the assertions above would pass
        // just as well against a `write_asset_bundle` that had stopped minifying entirely.
        assert!(
            read(&bundle.app_js) != taliesin_core::core_enhance_js(),
            "app.js should have been minified"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
