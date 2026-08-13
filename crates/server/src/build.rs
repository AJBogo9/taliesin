//! The `build` subcommand: render a single document or a whole site to disk.
//!
//! **What:** `build <file>` writes a self-contained HTML page (executing its code cells
//! first); `build <dir>` builds a multi-page site to `_site/`, rendering pages
//! concurrently (memory-capped) while keeping the output byte-identical to a sequential
//! build. Also `--out <dir>` (portable folder), `--strict`, and `--jobs <N>`.
//!
//! **How to use:** `main()` dispatches `build` to [`cmd_build`].
//!
//! **Depends on:** [`crate::exec`] + [`crate::freeze`] +
//! [`crate::build_budget`] (execution + the memory-aware concurrency cap),
//! [`crate::log`], and [`taliesin_core`] for rendering.
//!
//! **Load-bearing:** the concurrent site build (`build_site_async`/`PageOutcome`) defers
//! all logging and replays it in `site.pages` order, so a parallel build is byte-for-byte
//! identical to `--jobs 1`. Pinned by `tests/parallel_build_determinism.rs`. Do not
//! restructure that ordering or the per-page output/freeze isolation.

use crate::{build_budget, exec, freeze, log};
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
    /// `--no-exec`: render code cells as source on purpose. Also the opt-out from the
    /// "executable cells but no kernel" build failure.
    no_exec: bool,
    jobs: Option<usize>,
    /// `--stdout`: write the page to stdout instead of to a file. Single-document only —
    /// the one-shot HTML dump the retired `render` verb used to be (`build <f> --stdout
    /// --no-exec` is exactly what `render <f>` did).
    stdout: bool,
    /// `--format json` emits the build's static-lint diagnostics as `{diagnostics:[...]}`
    /// to stdout (for an agent/CI) instead of only the human log. Default `"human"`.
    format: &'a str,
    /// `--check-only`: lint and write nothing. The front door the retired `check` verb was,
    /// as `lint::cmd_check_only` over the same validator set this build runs, with no output
    /// tree, no kernel and no asset bundle. Refuses every "where to write" flag rather than
    /// silently ignoring one (see [`parse_build_args`]).
    check_only: bool,
}

/// Every long flag `build` accepts (drives the unknown-flag did-you-mean). `-j` is the
/// only short alias; it's not in this set (suggestions are between long flags).
pub(crate) const BUILD_FLAGS: &[&str] = &[
    "--out",
    "--jobs",
    "--strict",
    "--format",
    "--json",
    "--no-exec",
    "--stdout",
    "--check-only",
];

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
    let mut no_exec = false;
    let mut stdout = false;
    let mut check_only = false;
    let mut jobs_result: Result<Option<usize>, String> = Ok(None);
    let mut format: &str = "human";
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // `--format human|json`: mirror `check`'s flag exactly (value validated below).
            "--format" => match it.next().map(|s| s.as_str()) {
                Some(v) if v == "human" || v == "json" => format = v,
                other => return Err(format!("error: {}", crate::serve::bad_format_error(other))),
            },
            // `--json`: clig.dev shorthand for `--format json`, accepted on every
            // machine-output command so neither spelling dead-ends.
            "--json" => format = "json",
            // `--out <dir>` needs a real value. A missing one (end of args, or a flag
            // follows) is a hard error rather than silently leaving out_dir None and
            // writing `<stem>.html` to an unexpected place. (`--out` = output dir; the
            // undocumented `--dir` alias was dropped — `--dir` is the scaffold-input flag.)
            "--out" => match it.next().map(|s| s.as_str()) {
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
            // `--no-exec`: render code cells as source, deliberately. `serve` has accepted
            // it all along (as sugar for `TALIESIN_NO_EXEC`); `build` had only the env var,
            // which is a poor thing to make someone reach for now that a missing kernel
            // *fails* the build. This is that failure's opt-out.
            "--no-exec" => no_exec = true,
            // `--stdout`: the page to stdout rather than to a file. This is the whole of what
            // the `render` verb was, minus a second code path — pair it with `--no-exec` for
            // `render`'s static, kernel-free dump.
            "--stdout" => stdout = true,
            // `--check-only`: lint, write nothing. Never executes a cell, so it needs no
            // `--no-exec` (and accepts one, which agrees with it rather than contradicting it).
            "--check-only" => check_only = true,
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
    // Derives the synopsis from `build`'s `--help` block so it can't drift (it once omitted
    // `--format json`).
    let path = positionals
        .first()
        .copied()
        .ok_or_else(|| crate::usage_line("build"))?;
    // DX11: a format-implying output extension (`build doc.tmd doc.pdf`) is a hard error, not a
    // silent HTML-into-a-.pdf write. Checked here so it is caught for any invocation carrying
    // that second positional (even a contradictory `--out dist doc.pdf`, where it is otherwise
    // ignored), and stays unit-testable as pure arg parsing.
    if let Some(msg) = non_html_output_error(positionals.get(1).copied()) {
        return Err(msg);
    }
    // `--stdout` says "the page goes to stdout"; each of these says "the page goes
    // somewhere else", and `--format json` says "the diagnostics go to stdout". Silently
    // letting one win would either lose the page or interleave two streams on one fd, so
    // the contradiction is a loud error naming both spellings.
    if stdout {
        if let Some(other) = out_dir
            .map(|d| format!("--out {d}"))
            .or_else(|| positionals.get(1).map(|o| format!("`{o}`")))
        {
            return Err(format!(
                "error: --stdout writes the page to stdout, but {other} writes it to a file. \
                 Pick one."
            ));
        }
        if format == "json" {
            return Err(
                "error: --stdout and --format json both write to stdout. Use one or the other."
                    .to_string(),
            );
        }
    }
    // `--check-only` writes nothing, so every flag that says *where* to write contradicts it.
    // Named loudly rather than ignored: a `build x --check-only --out dist` that quietly
    // produced no `dist/` is the trap the `--stdout` conflict above was written against, and
    // `--jobs` describes output that never happens.
    if check_only
        && let Some(other) = out_dir
            .map(|d| format!("--out {d}"))
            .or_else(|| positionals.get(1).map(|o| format!("`{o}`")))
            .or_else(|| stdout.then(|| "--stdout".to_string()))
            .or_else(|| jobs.map(|n| format!("--jobs {n}")))
    {
        return Err(format!(
            "error: --check-only writes nothing, but {other} describes output. Drop one."
        ));
    }
    Ok(BuildArgs {
        path,
        out_html: positionals.get(1).copied(),
        out_dir,
        strict,
        no_exec,
        jobs,
        stdout,
        format,
        check_only,
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
        no_exec,
        jobs,
        stdout,
        format,
        check_only,
    } = match parse_build_args(args) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };
    // `--no-exec` is sugar for `TALIESIN_NO_EXEC=1`, exactly as on `serve`: one owner
    // (`taliesin_core::render::no_exec_in_force`) read by both the executor and the
    // render pass, so the flag and the env var can never mean different things.
    if no_exec {
        // SAFETY: set once at CLI startup, before the tokio runtime / kernel threads
        // spawn, so no other thread is touching the environment.
        unsafe { std::env::set_var("TALIESIN_NO_EXEC", "1") };
    }
    let json = format == "json";
    // `--check-only` is the static-lint front door: it shares `build`'s arg parsing, its
    // validator set and its `--format json` shape, and diverges before anything is written or
    // executed. Dispatched here, ahead of the project/single-doc split, because
    // `lint::collect_diagnostics` already handles both.
    if check_only {
        return crate::lint::cmd_check_only(Path::new(path), format, strict);
    }
    // A directory is a multi-page site project (`_site.yml` + `.tmd` pages);
    // a single `.tmd` keeps the original self-contained-page behaviour.
    if Path::new(path).is_dir() {
        // A directory is a project, and a project is what `_site.yml` declares. Without one
        // there is nothing to build: no nav, no title, no page at `/`. This is the stance
        // `read` already takes (`query.rs`); `build` used to warn and synthesize a website.
        if !Path::new(path).join("_site.yml").is_file() {
            log::error(&crate::serve::not_a_project_error(Path::new(path), "build"));
            return ExitCode::FAILURE;
        }
        // A site is many pages; there is no one page to put on stdout. Reject rather than
        // pick a page.
        if stdout {
            log::error(&format!(
                "--stdout writes one page, but {path} is a project of many. Name a single \
                 .tmd file, or build the site to a directory."
            ));
            return ExitCode::FAILURE;
        }
        return build_site(Path::new(path), out_dir, strict, jobs, json);
    }
    let mode = taliesin_core::OutputMode::Build;
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            log::error(&crate::lint::cannot_read(Path::new(path), &e));
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
    // `path` as the user typed it is the diagnostic prefix: it round-trips back into their
    // shell and into an editor's "open at line". `stem` stays the freeze key + page title.
    //
    // `--out <dir>` is the one spelling of `build <file.tmd>` whose output is a FOLDER, so
    // the vendored mermaid library goes beside the page there instead of inside it. The
    // single-file spellings (`build doc.tmd`, `build doc.tmd out.html`, `--stdout`) keep
    // inlining: each is one file, and one file that renders a diagram offline is the point.
    let mermaid_src = if out_dir.is_some() { MERMAID_FILE } else { "" };
    let executed =
        crate::serve::guarded(|| build_page_executing(&src, base, stem, path, mode, mermaid_src));
    let (html, problems, diagnostics, kernel_failure) = match executed {
        Ok(Ok(BuildResult::Page {
            html,
            problems,
            diagnostics,
            kernel_failure,
        })) => (html, problems, diagnostics, kernel_failure),
        Ok(Err(e)) => {
            log::error(&format!("cannot start runtime: {e}"));
            return ExitCode::FAILURE;
        }
        Err(panic) => {
            log::error(&format!("render panicked while building {path}: {panic}"));
            return ExitCode::FAILURE;
        }
    };
    // Offline-guarantee nudge: a built page keeps any external reference the author wrote
    // (a remote image, an external stylesheet, a remote/bare `{js}` import) verbatim, so a
    // "portable" output can silently need the network at view time. Warn (located, never fail)
    // rather than download — the tool does not fetch arbitrary URLs at build time.
    for w in offline_ref_warnings(&html, path) {
        log::warn(&w);
    }

    // In `--strict` mode, a cell that crashed (its traceback is baked into the HTML)
    // or any located warning fails the build instead of shipping a broken page with
    // exit 0. Without `--strict` the warnings were already logged; we still write.

    // Structured diagnostics to stdout (the human log stays on stderr, so the JSON stream
    // pipes cleanly). The page is still written — `--format json` only changes the
    // *reporting* channel, not what a build produces.
    if json {
        println!("{}", crate::lint::diagnostics_json(&diagnostics));
    }

    // `--stdout`: the page IS the output, so nothing is written and nothing is copied
    // beside it (an asset the page references stays where the author put it — a stdout
    // dump has no directory of its own to populate). The human log is already on stderr,
    // so the HTML pipes cleanly.
    if stdout {
        print!("{html}");
        return finalize_build(true, strict, problems, kernel_failure.as_deref());
    }
    if let Some(dir) = out_dir {
        let wrote = build_dir(&html, base, Path::new(dir), started);
        return finalize_build(wrote, strict, problems, kernel_failure.as_deref());
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
            finalize_build(true, strict, problems, kernel_failure.as_deref())
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

/// Final exit for a single-doc build. `--strict` turns problems into a failure (the page
/// is still written, but CI gets a non-zero exit); otherwise a non-strict build that
/// shipped with problems prints a closing tally so the silent degradation is visible
/// (DX12). `wrote` is false only on a write/create error, which already failed and
/// reported itself, so neither summary applies.
fn finalize_build(
    wrote: bool,
    strict: bool,
    problems: usize,
    kernel_failure: Option<&str>,
) -> ExitCode {
    if !wrote {
        return ExitCode::FAILURE;
    }
    // Before `--strict`, because this failure is the more specific one and its message is
    // the actionable one. A document whose whole value is executed output, shipped with
    // every cell stripped back to source, is not a build that succeeded.
    if report_kernel_failure(kernel_failure) {
        return ExitCode::FAILURE;
    }
    if strict && problems > 0 {
        warn_strict(problems);
        return ExitCode::FAILURE;
    }
    warn_nonstrict_problems(problems);
    ExitCode::SUCCESS
}

/// Log the build-fatal "executable cells but no kernel" report, and say whether there was
/// one. Shared by the single-doc and site build paths so they cannot drift on either the
/// wording or the decision.
///
/// The output is still written before this runs — same shape as `--strict`. What changes
/// is what gets *reported*: previously a warning and exit 0, which is the one outcome a CI
/// pipeline reads as "the book built fine".
fn report_kernel_failure(kernel_failure: Option<&str>) -> bool {
    match kernel_failure {
        Some(msg) => {
            log::error(msg);
            true
        }
        None => false,
    }
}

/// Log the `--strict` failure summary (shared by the single-doc and site build paths).
fn warn_strict(problems: usize) {
    log::error(&format!(
        "--strict: {problems} problem{} (cell error or located warning); failing the build",
        if problems == 1 { "" } else { "s" }
    ));
}

/// The non-strict closing tally (DX12): a `build` without `--strict` still writes even
/// when it hit problems (a missing image, a dead link, a broken cross-ref), and its exit
/// is 0 — so the per-warning lines have already scrolled past by the time it prints
/// `built …`. Restate the count and point at the flag that would have failed CI, instead
/// of a wordless green exit. A no-op when the build was clean. Shared by the single-doc
/// and site build paths.
fn warn_nonstrict_problems(problems: usize) {
    if problems == 0 {
        return;
    }
    log::warn(&format!(
        "built with {problems} problem{} (run with --strict to fail the build)",
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

/// The located "cell error" message for a failed cell output — one string shape shared by
/// the single-doc and site build paths (and their structured-diagnostic mirror).
///
/// Two different things land here and they must not be described the same way: a cell that
/// RAN and raised (its traceback is baked into the page, and the fix is in the author's
/// code), and a cell that never ran at all because the executor could not reach a kernel
/// (the fix is `TALIESIN_PYTHON`/`TALIESIN_R` or the environment). The executor marks the
/// diagnostics it writes itself with [`crate::exec::NOT_RUN_ATTR`]; asking that marker is
/// the source of truth, since the two share an HTML shape on purpose.
fn cell_error_message(page_label: &str, b: &taliesin_core::Block) -> String {
    let where_ = b
        .source_file
        .as_deref()
        .map(|f| format!("{f} "))
        .unwrap_or_default();
    let what = not_run_reason(&b.html).unwrap_or(
        "code cell raised an uncaught exception; its traceback is baked into the output",
    );
    format!(
        "cell error in {page_label} ({where_}@ {}): {what}",
        b.sourcepos
    )
}

/// Why a `tali-error` output is there, when the **executor** wrote it about a cell that
/// never ran rather than the interpreter raising about code that did. `None` for a genuine
/// traceback, the only case that may be called an exception.
///
/// Reads the marker's *kind* rather than the diagnostic's own prose, because that prose is
/// not reliably reachable: a `#| label: fig-x` cell wraps the block in a `<figure>`, which
/// is enough to make `classify_exec_output` report a figure rather than an error.
fn not_run_reason(html: &str) -> Option<&'static str> {
    use crate::exec;
    let is = |kind: &str| html.contains(&format!("{}=\"{}\"", exec::NOT_RUN_ATTR, kind));
    if is(exec::NOT_RUN_UNAVAILABLE) {
        // The executor logs the full "which interpreter, and why it could not launch"
        // diagnostic separately, once per language, so this line does not repeat it.
        Some("code cell did not run: no kernel was available for its language")
    } else if is(exec::NOT_RUN_DIED) {
        Some("code cell did not run: the kernel exited first; it re-runs on the next save")
    } else if is(exec::NOT_RUN_REQUEST) {
        Some("code cell did not complete: the execution request failed")
    } else if is(exec::NOT_RUN_TIMEOUT) {
        Some(
            "code cell did not complete: it hit a liveness cap and was interrupted \
             (a cell producing no output for TALIESIN_CELL_SILENCE seconds, default 600; \
             or TALIESIN_CELL_TIMEOUT if you set a wall-clock cap). Printing progress \
             from a long cell keeps it alive; 0 disables either cap",
        )
    } else {
        None
    }
}

/// Structured "cell error" diagnostics (build-only additions over `check`'s superset), in
/// block order, for `--format json`.
fn cell_error_diagnostics(
    blocks: &[taliesin_core::Block],
    page_label: &str,
) -> Vec<crate::lint::Diagnostic> {
    blocks
        .iter()
        .filter(|b| is_cell_error_output(&b.html))
        .map(|b| {
            crate::lint::Diagnostic::new(
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
/// Result of building a single page: the rendered HTML + its `--strict` problem count.
enum BuildResult {
    Page {
        html: String,
        problems: usize,
        /// The located diagnostics, structured, for `--format json`. Same set the human
        /// log emits, in the same order.
        diagnostics: Vec<crate::lint::Diagnostic>,
        /// Set when the document has executable cells whose kernel could not start: the
        /// full "here is everything I searched" report. The page is still written (as
        /// under `--strict`), then the build exits non-zero with this message.
        kernel_failure: Option<String>,
    },
}

/// Build one document, executing its cells.
///
/// Two names, deliberately: `stem` is the document's *identity* (the `_freeze/` cache key
/// and the page-title fallback), while `label` is what a diagnostic is prefixed with and so
/// must be a path an editor can open. They used to be one `fallback` argument carrying
/// `file_stem()`, which made every single-doc diagnostic read `pca-geometry:12:` — a name no
/// tool resolves. Swapping `stem` for the path instead would have renamed the freeze entry
/// and the page title, which is why this is a second parameter and not a substitution.
///
/// `mermaid_src` is [`MERMAID_FILE`] on the `--out <dir>` path and `""` everywhere else; see
/// [`build_dir`], which writes the file this names.
fn build_page_executing(
    src: &str,
    base: &Path,
    stem: &str,
    label: &str,
    mode: taliesin_core::OutputMode,
    mermaid_src: &str,
) -> std::io::Result<BuildResult> {
    let rt = tokio::runtime::Runtime::new()?;
    Ok(rt.block_on(async {
        // `problems` is what `--strict` fails on: located render warnings, broken
        // cross-refs, and crashed code cells — each already logged below.
        let mut problems = 0usize;
        // The same diagnostics, structured, for `--format json` — collected in the exact
        // order they are logged so the two channels agree.
        let mut diagnostics: Vec<crate::lint::Diagnostic> = Vec::new();
        // Malformed front-matter YAML: the live servers + `check` report this, but a
        // single-doc `build` used to skip it, so a typo'd `---` block built clean and
        // even passed `--strict`. Surface it (located) and count it toward --strict.
        if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(src) {
            log::warn(&format!("{label}:{line}: {message}"));
            diagnostics.push(crate::lint::Diagnostic::new(
                label.to_string(),
                Some(line),
                message,
            ));
            problems += 1;
        }
        // Single-document build: confine includes/resources to the document's own project
        // (its nearest `_site.yml`, else its own directory), so this emits the same
        // document the site build emits for the same page (PP-3) without re-opening the
        // climb-out-of-a-checkout escape PT-2 closed.
        let mut doc = taliesin_core::render_single_doc(src, base);
        // Located render warnings (front-matter typos, broken refs, and now
        // unresolved `{{< include … >}}` directives — the path-resolution channel)
        // are logged here so a `build` never ships a silently dropped include.
        for w in &doc.warnings {
            // Located, as `check` reports them: a `--strict` failure should name the line.
            log::warn(&locate(w, label));
            diagnostics.push(crate::lint::diag_from(w, label));
        }
        // Advice (severity `suggestion`) is reported but never blocks: a rule that suggests
        // a reword must not fail a build, or the only way to keep CI green is to leave the
        // rule off. Same classification `check` gates on, so the two cannot disagree.
        problems += crate::lint::blocking(&doc.warnings);
        // Broken cross-refs (a single doc has no site to resolve them across pages),
        // so a `build` doesn't ship a dangling `@fig-`/`@sec-` link silently.
        let xrefs = taliesin_core::cite::validate_xrefs(&doc.blocks);
        for w in &xrefs {
            log::warn(&locate(w, label));
            diagnostics.push(crate::lint::diag_from(w, label));
        }
        problems += xrefs.len();
        // The rest of the check-superset. These ran only in `check`, so a `--strict` build
        // exited 0 while shipping a missing image, a broken anchor or a dangling link —
        // and a green `--strict` reasonably reads as "safe to ship". Run *before* the code
        // cells execute, exactly as `check` does, so a figure a cell generates is never
        // linted as if the author had written it.
        let statics = crate::lint::page_static_diagnostics(
            src,
            &doc.blocks,
            base,
            crate::lint::Scope::Standalone,
        );
        for w in &statics {
            log::warn(&locate(w, label));
            diagnostics.push(crate::lint::diag_from(w, label));
        }
        problems += crate::lint::blocking(&statics);
        // Persistent execution cache, rooted at the ENCLOSING PROJECT when there is one.
        //
        // This used to be `base.join("_freeze")` keyed on the doc's stem, unconditionally —
        // `cmd_build` branches only on `is_dir()`, so the single-file path did no project
        // resolution at all. `preview <file.tmd>` has resolved a file to its enclosing
        // `_site.yml` since wave 1.1, so the two disagreed about what document this is:
        //
        //   build <project>              -> <project>/_freeze/posts/p.json
        //   build <project>/posts/p.tmd  -> <project>/posts/_freeze/p.json  (a SECOND cache)
        //
        // which re-executed every time and left a stray `_freeze/` in a project
        // subdirectory that no sweep removes (audit finding 03). It also made wave 13's
        // `run` retirement note false where it promises "a later `build` still replays
        // without one".
        //
        // The interpreter moves with the root for the same reason: its identity seeds every
        // cumulative key, so resolving a project `.venv` from `posts/` instead of the
        // project root would bust the cache on the axis the freeze path just fixed. A file
        // with no ancestor `_site.yml` keeps exactly the old behaviour.
        let project_root = taliesin_core::site::enclosing_site_root(base);
        let (freeze_file, interp_dir) = match &project_root {
            Some(root) => {
                // The key must be the page's path RELATIVE TO THE PROJECT, because that is
                // what the site build writes (`page_path(freeze_dir, &page.rel)`). Both
                // sides are canonical here — `enclosing_site_root` canonicalizes as it
                // climbs — so the strip cannot miss on a `..` or a symlink.
                let rel = Path::new(label)
                    .canonicalize()
                    .ok()
                    .and_then(|abs| abs.strip_prefix(root).ok().map(Path::to_path_buf))
                    .unwrap_or_else(|| Path::new(stem).to_path_buf());
                (
                    freeze::page_path(&root.join("_freeze"), &rel.to_string_lossy()),
                    root.as_path(),
                )
            }
            None => (freeze::page_path(&base.join("_freeze"), stem), base),
        };
        let mut ex = exec::Executor::with_freeze(freeze_file).in_dir(base);
        ex.set_interpreters(crate::interpreter::resolve_python(None, interp_dir));
        doc.blocks = ex.run(std::mem::take(&mut doc.blocks)).await;
        // Executable cells that could not execute: fatal, not a warning (see
        // `kernel_failure_report`). Carried out rather than reported here so the page is
        // still written first — same shape as `--strict`, which writes and then fails.
        let kernel_failure = ex.kernel_failure_report();
        // No re-log of `ex.diagnostic()` here: the executor already announced this exact
        // message at the point of failure, so repeating it printed the same fact twice.
        // The dev-menu channel (`serve`/`serve_site`) still reads `diagnostic()`, which is
        // a different surface, not a duplicate.
        // A crashed cell bakes its traceback into the page (exit 0 + silent stderr
        // before this); log it located and count it toward `--strict`.
        problems += report_cell_errors(&doc.blocks, label);
        diagnostics.extend(cell_error_diagnostics(&doc.blocks, label));
        BuildResult::Page {
            html: if mermaid_src.is_empty() {
                taliesin_core::render_doc_to_page(&doc, stem, mode)
            } else {
                taliesin_core::render_doc_to_page_mermaid_file(&doc, stem, mermaid_src)
            },
            problems,
            diagnostics,
            kernel_failure,
        }
    }))
}

/// The sibling copy of the vendored mermaid library a `--out <dir>` build writes for a page
/// with a diagram. One name for both halves (the href handed to the renderer and the file
/// written here), so the page cannot point at something that was never written.
const MERMAID_FILE: &str = "mermaid.min.js";

/// Write `<dir>/index.html` and copy each referenced local asset (an `src=`/
/// `href=` value pointing to an existing file under `base`) to the same relative
/// path under `dir`, leaving the HTML's paths untouched so the folder is portable.
/// Returns whether the page was written (the caller finalizes the exit code, so a
/// non-strict problem tally / a `--strict` failure decide it uniformly with the
/// single-file path).
fn build_dir(html: &str, base: &Path, dir: &Path, started: std::time::Instant) -> bool {
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::error(&format!("cannot create {}: {e}", dir.display()));
        return false;
    }
    let mut copied = copy_local_assets(html, base, dir);
    // The mermaid library, for a page that has a diagram. Not reachable through
    // `copy_local_assets`: its href is a string inside the loader script, not an `src=`
    // attribute, and it comes from the binary rather than from `base`. Content-gated exactly
    // as the inline path was, so a prose page gains nothing.
    if taliesin_core::has_mermaid(html) {
        let to = dir.join(MERMAID_FILE);
        match std::fs::write(&to, taliesin_core::mermaid_min_js()) {
            Ok(()) => copied += 1,
            Err(e) => {
                // Not fatal: the page is still written and the loader shows its
                // `[data-mermaid-error]` banner over the diagram source rather than a blank.
                log::warn(&format!("cannot write {}: {e}", to.display()));
            }
        }
    }
    let index = dir.join("index.html");
    if let Err(e) = std::fs::write(&index, html) {
        log::error(&format!("cannot write {}: {e}", index.display()));
        return false;
    }
    log::built(&format!(
        "{}  ·  {copied} asset{}{}",
        index.display(),
        if copied == 1 { "" } else { "s" },
        elapsed_note(started)
    ));
    true
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
    let boundary = taliesin_core::includes::repo_boundary(base);
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
        if !inside_repo(&from, &boundary) {
            log::warn(&format!(
                "asset resolves outside the repository, not bundled: {r}"
            ));
            continue;
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
    let boundary = taliesin_core::includes::repo_boundary(base);
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
        if !from.is_file() || !inside_repo(&from, &boundary) {
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
    fn walk(
        dir: &Path,
        root: &Path,
        out: &Path,
        seen: &mut std::collections::HashSet<PathBuf>,
        copied: &mut usize,
    ) {
        // The build never emits a symlink, so one under `out` is the author's own mount
        // (`sweep_stale` leaves them alone for that reason) and reading through it is
        // intended — but a mount pointing back up the tree used to re-walk the whole
        // deploy once per level, re-resolving each page against a longer path and
        // re-copying what it had already shipped. Descend into each directory once.
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
            if p.is_dir() {
                walk(&p, root, out, seen, copied);
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
    walk(
        out,
        root,
        out,
        &mut std::collections::HashSet::new(),
        &mut copied,
    );
    copied
}

/// Whether a path resolved out of a page's `src=`/`href=` still lands inside the
/// repository once symlinks are followed. The lexical rule the callers apply first
/// (no absolute path, no `..` segment) constrains what the *page text* may ask for and
/// says nothing about what an in-tree path resolves *to*: `<img src="fig.png">` where
/// `fig.png` is a symlink is contained by that rule and can still leave the checkout.
fn inside_repo(from: &Path, boundary: &Path) -> bool {
    from.canonicalize().is_ok_and(|c| c.starts_with(boundary))
}

/// Whether two paths resolve to the same file on disk (so we don't self-copy).
fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

/// Bodies of the `<script type="application/tali-js">…</script>` cells in `html` (the
/// author's `{js}` source, where relative `import()`/`fetch()` specifiers live —
/// invisible to the `src=`/`href=` scan). `</script` is server-escaped in the source, so
/// the next `</script>` reliably ends the body.
fn tali_js_cell_sources(html: &str) -> Vec<&str> {
    let needle = "type=\"application/tali-js\"";
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
    for body in tali_js_cell_sources(html) {
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
    diagnostics: Vec<crate::lint::Diagnostic>,
    problems: usize,
    /// Set when this page has executable cells whose kernel could not start: the full
    /// "here is everything I searched" report. Folded across the build into one fatal
    /// error (the interpreter cannot differ between pages of one build, so the first is
    /// the whole story).
    kernel_failure: Option<String>,
    written: bool,
    /// The conditional `_assets/` blobs this page's HTML links, folded across the build so
    /// only the linked ones are written (item 137).
    used: AssetUse,
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
    bundle: &AssetBundle,
) -> PageOutcome {
    let mut warnings = Vec::new();
    let mut diagnostics: Vec<crate::lint::Diagnostic> = Vec::new();
    let Ok(src) = std::fs::read_to_string(&page.input) else {
        let msg = format!("cannot read {}", page.input.display());
        diagnostics.push(crate::lint::Diagnostic::new(
            page.rel.clone(),
            None,
            msg.clone(),
        ));
        warnings.push(msg);
        return PageOutcome {
            warnings,
            diagnostics,
            problems: 0,
            kernel_failure: None,
            written: false,
            used: AssetUse::default(),
        };
    };
    let base = page.input.parent().unwrap_or(root);
    let mut doc = taliesin_core::render_document_scoped_with_site(
        &src,
        base,
        site.chapter_for(page),
        Some(&site.render_defaults()),
    );
    let mut problems = 0usize;
    // Malformed front-matter YAML: the lenient line-parser silently mis-extracts fields, so
    // the page builds with the wrong title/format. `check` reports it; the site build did not.
    if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(&src) {
        problems += 1;
        warnings.push(format!("{}:{line}: {message}", page.rel));
        diagnostics.push(crate::lint::Diagnostic::new(
            page.rel.clone(),
            Some(line),
            message,
        ));
    }
    // The check-superset, over the page's blocks *before* its cells execute (as `check`
    // does). `Scope::InSite` omits the single-doc link rule: an intra-site `[x](other.tmd)`
    // rewrites to `other.html`, so only `validate_cross_page_links` can judge it, and that
    // runs once for the whole project after every page is built.
    let statics =
        crate::lint::page_static_diagnostics(&src, &doc.blocks, base, crate::lint::Scope::InSite);
    problems += crate::lint::blocking(&statics);
    for w in &statics {
        warnings.push(locate(w, &page.rel));
        diagnostics.push(crate::lint::diag_from(w, &page.rel));
    }
    let mut exec =
        exec::Executor::with_freeze(freeze::page_path(freeze_dir, &page.rel)).in_dir(base);
    // No progress sink (a build has no client), but name the page: a cold site build runs
    // pages concurrently, so bare interleaved `cell 2/4` lines belong to nobody.
    exec.set_progress(None, Some(page.rel.clone()));
    // Resolve this project's interpreter (from _site.yml python:, a .venv, env, or
    // default) against the site root.
    exec.set_interpreters(crate::interpreter::resolve_python(
        site.config.python.as_deref(),
        root,
    ));
    doc.blocks = exec.run(std::mem::take(&mut doc.blocks)).await;
    let kernel_failure = exec.kernel_failure_report();
    // A crashed cell bakes its traceback into the page; collect a located line + count it
    // (same shape/order as the sequential `report_cell_errors`, but deferred).
    for b in &doc.blocks {
        if is_cell_error_output(&b.html) {
            problems += 1;
            let msg = cell_error_message(&page.rel, b);
            diagnostics.push(crate::lint::Diagnostic::new(
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
    let font_preload = asset_href(&page.url, &bundle.font_preload);
    let ext = taliesin_core::ExternalAssets {
        app_css: &app_css,
        katex_css: &katex_css,
        app_js: &app_js,
        mermaid_js: &mermaid_js,
        jslibs_js: &jslibs_js,
        font_preload: &font_preload,
    };
    let (html, render_warnings) = site.render_page_doc_external(page, doc, ext);
    for w in &render_warnings {
        // Located, the way `check` reports them. These carry a file + line and were being
        // flattened to `page.rel: message`, so a `--strict` failure named no line to fix.
        warnings.push(locate(w, &page.rel));
        diagnostics.push(crate::lint::diag_from(w, &page.rel));
    }
    problems += crate::lint::blocking(&render_warnings);
    // Offline-guarantee, per page: flag any external reference this page keeps, exactly like the
    // single-doc build, so the common multi-page deploy (`build <dir>`) is covered too.
    // Informational — deferred into the page's warnings, never counted in `problems`/`--strict`.
    for w in offline_ref_warnings(&html, &page.rel) {
        warnings.push(w);
    }
    // Which conditional blobs this page linked, read off the finished HTML (item 137). Taken
    // BEFORE the write, which moves `html`.
    let used = bundle.used_by(&html);
    let dest = out.join(&page.url);
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let written = match std::fs::write(&dest, html) {
        Ok(()) => true,
        Err(e) => {
            let msg = format!("cannot write {}: {e}", dest.display());
            diagnostics.push(crate::lint::Diagnostic::new(
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
        kernel_failure,
        written,
        used,
    }
}

/// The result of a directory (site/book) build: whether it succeeded, and the structured
/// diagnostics it produced (for `--format json` on `build`/`publish`), in deterministic
/// page order.
pub(crate) struct SiteBuildOutcome {
    pub ok: bool,
    pub diagnostics: Vec<crate::lint::Diagnostic>,
}

/// The resolved shared-asset filenames (content-hashed), computed once per site build.
struct AssetBundle {
    app_css: String,
    katex_css: String,
    app_js: String,
    mermaid_js: String,
    jslibs_js: String,
    /// The roman body face, root-relative, for each page's `preload` (item 150).
    font_preload: String,
}

/// The on-disk name inside `_assets/` for a root-relative asset href (`_assets/app.ab.css`
/// -> `app.ab.css`). One definition, so the href a page links and the file the build writes
/// can never be spelled apart.
fn file_of(root_rel: &str) -> &str {
    root_rel.rsplit('/').next().unwrap_or(root_rel)
}

/// Which of the three conditional blobs a built page or deck actually links.
///
/// Item 137: `katex.css`, `mermaid.js` and `jslibs.js` are ~85-92% of a site build's
/// `_assets/` bytes, and on a prose-only project **no page references any of them**. They
/// are named up front (a page needs the href to link) but written only once something has.
#[derive(Clone, Copy, Default)]
struct AssetUse {
    katex: bool,
    mermaid: bool,
    jslibs: bool,
}

impl AssetUse {
    /// Union, for folding per-page results together. Deliberately order-independent, so a
    /// `--jobs N` build reaches the same set as the sequential one.
    fn merge(&mut self, other: AssetUse) {
        self.katex |= other.katex;
        self.mermaid |= other.mermaid;
        self.jslibs |= other.jslibs;
    }
}

impl AssetBundle {
    /// What this finished page HTML links, read off the **emitted href** rather than
    /// re-deriving the render-time predicates. That is the whole reason this cannot go
    /// stale: any future emitter that links a conditional asset is covered automatically,
    /// and the thing being asserted is exactly the thing a browser will request.
    ///
    /// Matched on the hashed *filename* (not the root-relative path), because a nested
    /// page's href carries a `../` climb.
    fn used_by(&self, html: &str) -> AssetUse {
        let links = |rel: &String| {
            let name = file_of(rel);
            !name.is_empty() && html.contains(name)
        };
        AssetUse {
            katex: links(&self.katex_css),
            mermaid: links(&self.mermaid_js),
            jslibs: links(&self.jslibs_js),
        }
    }

    /// Write the conditional blobs something linked, and only those.
    ///
    /// Erring here is asymmetric: writing one nothing links costs deploy bytes, while
    /// *skipping* one a page links is a live 404 on a published site. That is why `used`
    /// comes from the emitted HTML and why the pin asserts both directions.
    fn write_conditional(&self, out: &Path, used: AssetUse) -> std::io::Result<()> {
        let dir = out.join("_assets");
        let put = |rel: &String, bytes: &str| -> std::io::Result<()> {
            std::fs::write(dir.join(file_of(rel)), bytes)
        };
        if used.katex {
            put(
                &self.katex_css,
                &taliesin_core::minify_css(taliesin_core::katex_css()),
            )?;
        }
        // Vendored libs are already minified: write as-is (do not re-minify).
        if used.mermaid {
            put(&self.mermaid_js, &taliesin_core::mermaid_bundle_js())?;
        }
        if used.jslibs {
            put(&self.jslibs_js, &taliesin_core::js_cell_libs_js())?;
        }
        Ok(())
    }
}

/// Minify + content-hash each shared blob, write it once under `<out>/_assets/`, and
/// return the (root-relative) filenames. Clears any stale `_assets/` first so old hashes
/// do not accumulate across rebuilds.
///
/// Two departures from "hash it and write it", both about weight:
///
/// * The body typeface's faces (item 150) are written **first**, because `app_css` now
///   references them by hashed name and so cannot be hashed until those names exist.
/// * The three conditional blobs are hashed here but **not** written, so a page has an
///   href to link; [`AssetBundle::write_conditional`] then writes whichever ones a page
///   actually did link (item 137).
fn write_asset_bundle(out: &Path) -> std::io::Result<AssetBundle> {
    use taliesin_core::hash::{fnv1a, fnv1a_bytes};
    let dir = out.join("_assets");
    let _ = std::fs::remove_dir_all(&dir); // own the lifecycle; clear stale hashes
    std::fs::create_dir_all(&dir)?;
    // The root-relative href a page links; `file_of` recovers the on-disk name from it, so
    // the two spellings are derived from one place rather than formatted twice.
    let hashed =
        |stem: &str, ext: &str, bytes: &str| format!("_assets/{stem}.{:x}.{ext}", fnv1a(bytes));
    let named = |stem: &str, ext: &str, bytes: &str| -> std::io::Result<String> {
        let rel = hashed(stem, ext, bytes);
        std::fs::write(dir.join(file_of(&rel)), bytes)?;
        Ok(rel)
    };

    // The body faces first: both stylesheets below reference them by hashed name, so their
    // names have to exist before either sheet is hashed.
    let mut font_hrefs: Vec<(&str, String)> = Vec::new();
    let mut font_preload = String::new();
    for (src_name, bytes) in taliesin_core::FONT_FILES {
        let stem = src_name.strip_suffix(".woff2").unwrap_or(src_name);
        let name = format!("{stem}.{:x}.woff2", fnv1a_bytes(bytes));
        std::fs::write(dir.join(&name), bytes)?;
        // The sheet references a SIBLING (both live in `_assets/`), so no path prefix: a
        // `url()` resolves against the stylesheet, not the page. The preload href does the
        // opposite and is depth-adjusted per page by `asset_href`.
        if src_name.contains("normal") {
            font_preload = format!("_assets/{name}");
        }
        font_hrefs.push((src_name, name));
    }

    let app_css = named(
        "app",
        "css",
        &taliesin_core::minify_css(&taliesin_core::shared_site_css_linked_fonts(&font_hrefs)),
    )?;
    let app_js = named("app", "js", &taliesin_core::core_enhance_js())?;
    // Named, not written: see `write_conditional`.
    let katex_css = hashed(
        "katex",
        "css",
        &taliesin_core::minify_css(taliesin_core::katex_css()),
    );
    let mermaid_js = hashed("mermaid", "js", &taliesin_core::mermaid_bundle_js());
    let jslibs_js = hashed("jslibs", "js", &taliesin_core::js_cell_libs_js());
    Ok(AssetBundle {
        app_css,
        katex_css,
        app_js,
        mermaid_js,
        jslibs_js,
        font_preload,
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
/// structured diagnostics. Shared by `cmd_build`'s directory branch, `build_json` (the MCP
/// `build` tool), and `publish` (which needs the success signal, not just an opaque
/// `ExitCode`, plus the freedom to keep working with the output dir afterward). `verb` is
/// the CLI verb the caller was invoked as (`"build"`/`"publish"`), so a rejection names the
/// right command to retry.
///
/// This is THE enforcement point for "a directory is a project, and a project is what
/// `_site.yml` declares": every caller inherits the guard by construction rather than
/// having to remember to add it. `cmd_build`'s own directory branch still checks this
/// first too (it must run ahead of `--stdout`, which the guard here, reached only
/// after those, would otherwise shadow; see `project_required.rs`'s
/// `stdout_conflicts_are_loud`), so for `build` this is a redundant backstop. It is not
/// redundant for `publish`, which used to call straight into the site build and skip the
/// check entirely: `publish` on a directory with no `_site.yml` warned, synthesized a
/// one-page site, and deployed it. `project_required.rs`'s
/// `publish_of_a_non_project_directory_is_rejected_with_guidance` pins the fix.
pub(crate) fn run_site_build(
    root: &Path,
    out_override: Option<&str>,
    strict: bool,
    jobs: Option<usize>,
    verb: &str,
) -> SiteBuildOutcome {
    if !root.join("_site.yml").is_file() {
        let msg = crate::serve::not_a_project_error(root, verb);
        log::error(&msg);
        return SiteBuildOutcome {
            ok: false,
            diagnostics: vec![crate::lint::Diagnostic::new(
                root.display().to_string(),
                None,
                msg,
            )],
        };
    }
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
                diagnostics: vec![crate::lint::Diagnostic::new(
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
    let outcome = run_site_build(root, out_override, strict, jobs, "build");
    if json {
        println!("{}", crate::lint::diagnostics_json(&outcome.diagnostics));
    }
    if outcome.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Build a multi-page site: render every `.tmd` page with the shared chrome to
/// `<out>/<page>.html` and mirror the project's non-source assets alongside, so the output
/// directory is a deployable static site. `out_override` (the `--out` flag) wins over the
/// config's `output-dir` (default `_site`).
///
/// One project per call. Composing several into one deploy (this repo's own site, with the
/// two docs books and the gallery exhibits under it) is `tools/build-site.sh`, which runs
/// one `build … --out <out>/<prefix>` per sub-project, parent first, because the parent's sweep
/// deletes what it did not write. That script is run by `.githooks/pre-push`, and it
/// resolves every cross-project link against the composed output, because the reason the
/// `mounts:` key existed at all was a deploy whose call-to-action 404'd from a script
/// nobody ran (item 149).
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
    let mut diagnostics: Vec<crate::lint::Diagnostic> = Vec::new();
    // A malformed `_site.yml` silently degrades the whole site to defaults (no nav, no
    // title, wrong output dir): a real `--strict` problem, unlike a benign missing config.
    let mut config_problems = 0usize;
    for w in &site.warnings {
        if taliesin_core::site::is_malformed_config_warning(w) {
            config_problems += 1;
            diagnostics.push(crate::lint::Diagnostic::new(
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
        diagnostics.push(crate::lint::Diagnostic::new(
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
        diagnostics.push(crate::lint::Diagnostic::new(
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

    // Refuse to build into the source directory *or any directory above it*. Equal:
    // `mirror_assets` and the page writes would copy files onto themselves, and
    // `fs::copy` truncates the destination first — silently zeroing the user's own
    // assets. Above: worse, because `sweep_stale` then walks *down* into the source and
    // deletes it. `build myblog --out .` (the natural deploy-to-repo-root spelling) used
    // to report "swept 4 stale files", exit 0, and leave `_site.yml` alone in a directory
    // that had held the `.tmd` sources, the README and `src/`. Testing equality only was
    // the whole gap: `starts_with` is component-wise on canonical paths, so a sibling
    // named `myblog2` is not caught by the prefix.
    // Canonical, so both halves of the message below are in the same spelling: `out` is
    // already canonical, and printing it beside a relative `myblog` reads as if the two
    // were unrelated.
    let canon_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if canon_root.starts_with(&out) {
        let msg = if canon_root == out {
            format!(
                "output directory is the source directory ({}); refusing to build in place \
                 (it would overwrite/truncate your source files). Use a different `output-dir:` or `--out <dir>`.",
                out.display()
            )
        } else {
            format!(
                "output directory ({}) contains the source directory ({}); refusing to build \
                 (the stale-file sweep would delete your sources). Use a different `output-dir:` \
                 or `--out <dir>` outside the project.",
                out.display(),
                canon_root.display()
            )
        };
        log::error(&msg);
        diagnostics.push(crate::lint::Diagnostic::new(
            root.display().to_string(),
            None,
            msg,
        ));
        return SiteBuildOutcome {
            ok: false,
            diagnostics,
        };
    }

    // The build owns its output directory: it mirrors the source into it and sweeps
    // everything under it that this build did not write. So it may only write into a
    // directory it created or previously claimed. `--out public` on a GitHub Pages
    // folder deleted `CNAME`, the author's `thesis.txt` and their `photos/` tree, and
    // exited 0. Refusing (rather than a `--force` knob) is the "perfect the default"
    // answer: there is one safe directory to name and the message names the files that
    // stopped it.
    if let Some(found) = unowned_output_entries(&out) {
        let msg = format!(
            "output directory ({}) is not a Taliesin build directory: it holds {}, which \
             this build did not produce. `build` deletes everything under its output that \
             it did not write, so point `--out` at a new or empty directory, or empty this \
             one first.",
            out.display(),
            found.join(", ")
        );
        log::error(&msg);
        diagnostics.push(crate::lint::Diagnostic::new(
            root.display().to_string(),
            None,
            msg,
        ));
        return SiteBuildOutcome {
            ok: false,
            diagnostics,
        };
    }
    claim_output(&out);

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
            diagnostics.push(crate::lint::Diagnostic::new(
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
    // How many pages at once. An explicit `--jobs N` is the user's stated PAGE count and
    // is honored exactly; auto sizes the cap against free RAM and the core count, on the
    // worst-case assumption that every concurrent page boots a kernel. Determinism is
    // untouched: the cap only changes *when* a page builds, never *what* it produces.
    let build_cap = build_budget::concurrency_cap(jobs, build_budget::PER_KERNEL_MB).max(1);
    log::info(&format!("building with up to {build_cap} parallel page(s)"));
    let mut pages = 0usize;
    // The first page whose kernel could not start. One report for the whole run: the
    // interpreter and its error cannot differ between pages of a single build, so
    // repeating it per page would be noise.
    let mut kernel_failure: Option<String> = None;
    // `--strict` problem tally across the whole site: a malformed `_site.yml`, per-page
    // located warnings, broken cross-refs, crashed cells, and page-task panics (each
    // already logged where it occurs).
    let mut problems = config_problems;

    // Build into a slot per page (indexed by page order) so results aggregate
    // deterministically regardless of completion order. A `Semaphore` of size
    // `build_cap` bounds how many build kernels run at once (memory-aware); the file
    // write each page does is on its own paths, so no lock is held across the `.await`.
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
        set.spawn(async move {
            // Hold a permit only for this page's build; dropping it on return frees the
            // slot for the next queued page. The permit guards kernel count, not any
            // shared data structure, so nothing is locked across the build's `.await`.
            let _permit = sem.acquire().await.expect("build semaphore not closed");
            let page = &site.pages[idx];
            let outcome = build_one_page(&site, page, &freeze_dir, &out, &root_arc, &bundle).await;
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
                diagnostics.push(crate::lint::Diagnostic::new(
                    root.display().to_string(),
                    None,
                    msg,
                ));
            }
        }
    }

    // Replay every page's deferred logs + tally counters in page order, so the build's
    // output is identical whether it ran 1-wide or N-wide.
    // Item 137: the union of what the pages linked. `merge` is order-independent, so this
    // reaches the same set whichever order the concurrent builds completed in.
    let mut used = AssetUse::default();
    for outcome in outcomes.into_iter().flatten() {
        for w in &outcome.warnings {
            log::warn(w);
        }
        diagnostics.extend(outcome.diagnostics);
        problems += outcome.problems;
        kernel_failure = kernel_failure.or(outcome.kernel_failure);
        used.merge(outcome.used);
        if outcome.written {
            pages += 1;
        }
    }
    // Reclaim the owned values the tail of this function still uses.
    let site = std::sync::Arc::try_unwrap(site).unwrap_or_else(|arc| (*arc).clone());
    let out = std::sync::Arc::try_unwrap(out).unwrap_or_else(|arc| (*arc).clone());

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
        // Root-ABSOLUTE asset hrefs, unlike every other page in this build: this one file is
        // served for any unknown path, so `../_assets/…` would resolve against whatever
        // directory the reader landed in. Same reasoning as the page's own `/` home link.
        let abs = |root_rel: &str| format!("/{root_rel}");
        let (app_css, katex_css) = (abs(&bundle.app_css), abs(&bundle.katex_css));
        let (app_js, mermaid_js) = (abs(&bundle.app_js), abs(&bundle.mermaid_js));
        let jslibs_js = abs(&bundle.jslibs_js);
        let font_preload = abs(&bundle.font_preload);
        let ext = taliesin_core::ExternalAssets {
            app_css: &app_css,
            katex_css: &katex_css,
            app_js: &app_js,
            mermaid_js: &mermaid_js,
            jslibs_js: &jslibs_js,
            font_preload: &font_preload,
        };
        let html = site.render_404_page_external(ext);
        // The generated 404 votes on the conditional blobs like any other emitted page
        // (item 137) — it is chrome, so it links none of them today, but "today" is not a
        // thing to hard-code when the cost of being wrong is a 404 inside the 404.
        used.merge(bundle.used_by(&html));
        match std::fs::write(out.join("404.html"), html) {
            Ok(()) => not_found = "  ·  404.html",
            Err(e) => log::warn(&format!("cannot write 404.html: {e}")),
        }
    }

    // Every HTML surface that could link a conditional blob has now been emitted (pages,
    // decks, the generated 404), so write the ones something actually did — item 137. On a
    // prose-only project that is none of them, which is 85-92% of what `_assets/` used to
    // weigh. Deliberately placed *after* the 404: a vote that arrives after the flush is a
    // published page pointing at a file that was never written.
    if let Err(e) = bundle.write_conditional(&out, used) {
        let msg = format!("cannot write {}/_assets: {e}", out.display());
        log::error(&msg);
        diagnostics.push(crate::lint::Diagnostic::new(
            root.display().to_string(),
            None,
            msg,
        ));
        problems += 1;
    }
    // SEO sidecars: emitted only when `url:` is set (absolute URLs are mandatory for a feed
    // and a sitemap). Both are auto-derived from the site's own content; the author writes
    // nothing SEO-specific.
    let mut seo_written: Vec<PathBuf> = Vec::new();
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
    }
    let seo_note = if seo_written.is_empty() {
        String::new()
    } else {
        format!("  ·  {} SEO file(s)", seo_written.len())
    };
    // Sweep stale output: a page or asset removed/renamed in the source must not linger
    // across rebuilds (the output tree is a mirror of what this build produced). Anything
    // in `out` that this build didn't write — and isn't dot/underscore deploy metadata —
    // is stale. Runs before the referenced-source pass so a no-longer-linked `.md`/`.scss`
    // (a SKIP_EXT file `mirror_assets` never mirrors, so it's absent from `keep`) is swept
    // too; the pass below then re-ships only the sources the current pages still link.
    let mut keep: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    keep.extend(site.pages.iter().map(|p| PathBuf::from(&p.url)));
    keep.extend(asset_paths.iter().cloned());
    keep.insert(PathBuf::from("404.html"));
    if !site.search_index_json.is_empty() && site.search_index_json != "[]" {
        keep.insert(PathBuf::from("search-index.js"));
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
    // judge. A broken cross-page link is exactly the defect a `--strict` build used to
    // deploy with exit 0.
    for (rel, w) in site.validate_cross_page_links() {
        problems += 1;
        log::warn(&locate(&w, &rel));
        diagnostics.push(crate::lint::diag_from(&w, &rel));
    }
    // The `_site.yml` chrome's own hrefs, which no page body carries and so no page-body
    // harvest could ever see. Same registry, same rules; site-wide blast radius.
    for w in site.validate_chrome_links() {
        problems += 1;
        log::warn(&locate(&w, "_site.yml"));
        diagnostics.push(crate::lint::diag_from(&w, "_site.yml"));
    }
    for w in site.validate_shared_bibliography() {
        problems += 1;
        log::warn(&locate(&w, "_site.yml"));
        diagnostics.push(crate::lint::diag_from(&w, "_site.yml"));
    }

    // Second asset pass: ship source files (`.md`/`.scss`/…) that pages actually link to.
    // mirror_assets drops them by extension (publish hygiene), but a *referenced* source is
    // an intentional download — skipping it would leave a dead link on a green build.
    let assets = asset_paths.len() + deploy_referenced_sources_for_site(root, &out);

    log::built(&format!(
        "{}  ·  {pages} page{}  ·  {assets} asset{}{search}{not_found}{seo_note}{}",
        out.display(),
        if pages == 1 { "" } else { "s" },
        if assets == 1 { "" } else { "s" },
        elapsed_note(started),
    ));
    // In `--strict` mode a problem (crashed cell / located warning / broken ref)
    // fails the build after writing it, so CI catches a broken site. Without `--strict`
    // the site still ships, but a closing tally (DX12) makes the shipped problems visible
    // rather than a wordless green exit after pages of scrolled-past warnings.
    // A site with executable cells and no usable kernel fails outright, ahead of the
    // `--strict` tally and regardless of it: every cell stripped back to source is not a
    // successful build of a book whose value is its executed output. `--no-exec` is the
    // way to ask for source-only rendering on purpose.
    let kernel_fail = report_kernel_failure(kernel_failure.as_deref());
    let strict_fail = strict && problems > 0;
    if kernel_fail {
        // The kernel error is the actionable one; don't bury it under a second tally.
    } else if strict_fail {
        warn_strict(problems);
    } else {
        warn_nonstrict_problems(problems);
    }
    SiteBuildOutcome {
        ok: !strict_fail && !kernel_fail,
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
/// A symlink is followed only while its target stays inside the repository, matching
/// what [`taliesin_core::includes`] allows a document path to resolve to: a link to a
/// sibling directory of the same checkout is first-party authoring, one that leaves the
/// checkout would publish a file the author never put in the project.
fn mirror_assets(root: &Path, out: &Path) -> (Vec<PathBuf>, Vec<String>) {
    #[allow(clippy::too_many_arguments)]
    fn walk(
        dir: &Path,
        root: &Path,
        out: &Path,
        boundary: &Path,
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
            // Testing the link itself is enough: anything deeper can only leave the
            // repository through a link this same test already refused.
            if entry.file_type().is_ok_and(|t| t.is_symlink())
                && !p.canonicalize().is_ok_and(|c| c.starts_with(boundary))
            {
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
                walk(&p, root, out, boundary, seen, copied, skipped);
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
        &taliesin_core::includes::repo_boundary(root),
        &mut std::collections::HashSet::new(),
        &mut copied,
        &mut skipped,
    );
    skipped.sort();
    skipped.dedup();
    (copied, skipped)
}

/// The marker `build` writes into its output directory and reads back to recognise that
/// directory as its own on the next run. Dot-prefixed, so [`mirror_assets`] never copies
/// one into a nested deploy and [`sweep_stale`] never deletes it — which is also what
/// lets a composed deploy (`tools/build-site.sh`, parent first) survive the parent's
/// sweep and rebuild each sub-project into its own prefix.
const OUTPUT_MARKER: &str = ".taliesin-build";

const OUTPUT_MARKER_BODY: &str = "\
Taliesin build output. `taliesin build` deletes files under this directory that it did
not produce. Delete this file to make it refuse to write here again.
";

/// Is `out` a directory this build already owns? Two pieces of evidence for one question.
/// [`OUTPUT_MARKER`] is the authoritative one: it is written before the first byte of the
/// build, so it identifies even a run that died half-way through mirroring assets. The
/// `_assets/app.<hash>.css` bundle is the fallback, and it is what lets an output
/// directory written by an EARLIER binary keep working — nothing but [`write_asset_bundle`]
/// produces that name, and without this clause every `_site/` and every live deploy folder
/// in existence would be refused once, for bookkeeping the build had not started keeping
/// yet.
fn is_taliesin_output(out: &Path) -> bool {
    if out.join(OUTPUT_MARKER).is_file() {
        return true;
    }
    std::fs::read_dir(out.join("_assets")).is_ok_and(|mut entries| {
        entries.any(|e| {
            e.is_ok_and(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name.starts_with("app.") && name.ends_with(".css")
            })
        })
    })
}

/// Entries under `out` that this build does not own, or `None` when `out` is the build's
/// to write into: it is empty, or [`is_taliesin_output`] recognises it, or everything in
/// it is the dot/underscore deploy metadata [`sweep_stale`] already promises never to
/// touch (`.git`, `.nojekyll`, `_headers`). That last case is the whole reason the test is
/// "what could the sweep delete" rather than "is the directory empty": a `gh-pages`
/// worktree with a `.nojekyll` in it is the ordinary deploy target, and nothing in it is
/// at risk.
///
/// At most three names are returned, so a directory full of a stranger's files does not
/// print a wall of text at them.
fn unowned_output_entries(out: &Path) -> Option<Vec<String>> {
    if is_taliesin_output(out) {
        return None;
    }
    let mut found: Vec<String> = std::fs::read_dir(out)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            (!name.starts_with('.') && !name.starts_with('_')).then_some(name)
        })
        .collect();
    if found.is_empty() {
        return None;
    }
    found.sort();
    let more = found.len().saturating_sub(3);
    found.truncate(3);
    if more > 0 {
        found.push(format!("and {more} more"));
    }
    Some(found)
}

/// Claim `out` as this build's output so the next build recognises it. Not fatal on
/// failure — every later write into `out` fails the build with its own error — but not
/// silent either: an unwritten marker makes the *next* build refuse a directory that is
/// in fact its own.
fn claim_output(out: &Path) {
    let marker = out.join(OUTPUT_MARKER);
    if let Err(e) = std::fs::write(&marker, OUTPUT_MARKER_BODY) {
        log::warn(&format!(
            "cannot write {}: {e} (the next build will not recognise this output directory)",
            marker.display()
        ));
    }
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
///
/// **Precondition: `out` is a directory this build owns.** Nothing here can tell a stale
/// page from a stranger's file, so the ownership question is settled once, before the
/// first write, by [`unowned_output_entries`] + [`claim_output`]. Deleting was never the
/// bug: sweeping somewhere the build had no business writing was.
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

/// Unique local `src=`/`href=`/`poster=` values in `html` (skips external URLs,
/// protocol-relative refs, data URIs, in-page anchors, and other schemes).
///
/// `poster=` is a media attribute the first two never carry: harvesting only `src`/`href`
/// built a folder whose `<video>` still 404s. It stays because raw `<video>` HTML is in the
/// trust model and `diagnostics/media.rs` validates the same attribute.
///
/// `data-src=` was harvested here too until 2026-08-09, for a theme-adaptive `dark=` pair
/// that shipped both clips as `data-src` so the hidden one was never fetched. Wave 7 cut
/// `{{< video >}}`, which took the page-shell promoter that turned `data-src` into `src`
/// with it — so nothing emits the attribute and nothing would load a file harvested from
/// it. Its comment cited `corpus/media/screencast.tmd`, which does not exist either.
/// **The whole-attribute guard below is unaffected and still load-bearing**: `src="` is a
/// substring of the click-to-source `data-tali-src="`, which must stay unharvested.
fn local_refs(html: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = html.as_bytes();
    for attr in ["src=\"", "href=\"", "poster=\""] {
        let mut i = 0;
        while let Some(pos) = html[i..].find(attr) {
            let at = i + pos; // first byte of the attribute name
            let start = at + attr.len();
            let Some(len) = html[start..].find('"') else {
                break;
            };
            let val = &html[start..start + len];
            i = start + len;
            // The match must *begin* an attribute name, not end one: `data-tali-src="…"`
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

/// A reference the browser fetches over the network at view time: an absolute `http(s)://`
/// URL or a protocol-relative `//host/…`. Deliberately narrow — `data:` (inline), a `#frag`,
/// and `mailto:`/`tel:`/`vscode:`/`javascript:` are not view-time fetches, and a relative or
/// root path is local.
fn is_external_fetch(v: &str) -> bool {
    v.starts_with("//") || v.starts_with("http://") || v.starts_with("https://")
}

/// A bare ESM specifier (`import("three")`): not relative, not root-absolute, not a URL, not a
/// data URI. In a browser `{js}` cell it is unresolvable without an import map, so it also
/// breaks a portable build (like a remote import).
fn is_bare_specifier(v: &str) -> bool {
    !v.is_empty()
        && !v.starts_with("./")
        && !v.starts_with("../")
        && !v.starts_with('/')
        && !v.starts_with("data:")
        && !is_external_fetch(v)
}

/// An external reference left verbatim in a built page, with the best-effort source line of
/// the block that contains it (from the nearest preceding `data-sourcepos`).
#[derive(Debug, PartialEq, Eq)]
struct ExternalRef {
    url: String,
    line: Option<u32>,
}

/// The 1-based source line of the block enclosing byte `offset` in `html`, read from the
/// nearest preceding `data-sourcepos="L:…"`. `None` when no located block precedes it.
fn sourcepos_line_before(html: &str, offset: usize) -> Option<u32> {
    const KEY: &str = "data-sourcepos=\"";
    let at = html[..offset].rfind(KEY)? + KEY.len();
    let digits: String = html[at..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Whether the tag opened just before byte `attr_at` in `html` has tag name `name` (ASCII,
/// case-insensitive) — used to keep an external `href=` flag to `<link>` (a stylesheet/preload
/// fetch), never `<a>`/`<area>`/`<base>` (navigation/base).
fn tag_named_before(html: &str, attr_at: usize, name: &str) -> bool {
    let Some(lt) = html[..attr_at].rfind('<') else {
        return false;
    };
    let after = &html[lt + 1..];
    after.len() >= name.len()
        && after.as_bytes()[..name.len()].eq_ignore_ascii_case(name.as_bytes())
        && after[name.len()..]
            .chars()
            .next()
            .is_none_or(|c| c.is_ascii_whitespace() || c == '>' || c == '/')
}

/// Each dynamic `import("spec")` / `import('spec')` in `src`, as `(byte offset of `import`,
/// specifier)`. Only the dynamic-call form (the `{js}` cell shape the audit found) is matched,
/// with an `import` word boundary, so a substring like `reimport(` or a comment is ignored.
fn dynamic_import_specifiers(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let b = src.as_bytes();
    let mut i = 0;
    while let Some(pos) = src[i..].find("import") {
        let at = i + pos;
        i = at + "import".len();
        // `import` must start a word (not `reimport`); a non-ASCII lead byte is a boundary too.
        if at > 0 {
            let p = b[at - 1];
            if p == b'_' || p.is_ascii_alphanumeric() {
                continue;
            }
        }
        let mut j = at + "import".len();
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= b.len() || b[j] != b'(' {
            continue; // a static `import x from …`, not the dynamic call form
        }
        j += 1;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= b.len() || (b[j] != b'"' && b[j] != b'\'') {
            continue; // import(expr) with a non-literal specifier — can't classify, skip
        }
        let q = b[j] as char;
        let spec_start = j + 1;
        let Some(rel) = src[spec_start..].find(q) else {
            break;
        };
        out.push((at, src[spec_start..spec_start + rel].to_string()));
        i = spec_start + rel + 1;
    }
    out
}

/// Every external (network-fetched-at-view-time) reference left verbatim in built `html`: a
/// resource `src=` (img/script/iframe/audio/video/…), a `<link href=>` stylesheet/preload, and
/// a remote or bare `{js}` `import()` specifier. These keep a `--out` build from being
/// self-contained (offline viewing fails). NOT flagged: `<a href>` hyperlinks (navigation, not
/// a fetch), `data:` URIs, local/relative paths, or the tool's own inlined assets (the import
/// scan reads only author `{js}` cell bodies). Each ref carries its enclosing block's line.
fn external_refs(html: &str) -> Vec<ExternalRef> {
    let bytes = html.as_bytes();
    let mut out: Vec<ExternalRef> = Vec::new();
    let push = |url: &str, off: usize, out: &mut Vec<ExternalRef>| {
        let line = sourcepos_line_before(html, off);
        if !out.iter().any(|r| r.url == url && r.line == line) {
            out.push(ExternalRef {
                url: url.to_string(),
                line,
            });
        }
    };
    // (1) resource `src=` (always a fetch) and `<link href=>` (a stylesheet/preload fetch).
    for attr in ["src=\"", "href=\""] {
        let mut i = 0;
        while let Some(pos) = html[i..].find(attr) {
            let at = i + pos;
            let start = at + attr.len();
            let Some(len) = html[start..].find('"') else {
                break;
            };
            let val = &html[start..start + len];
            i = start + len;
            // The match must *begin* an attribute name, so `data-tali-src="…"` (click-to-source)
            // is not harvested — mirrors `local_refs`.
            if at > 0 && bytes[at - 1] != b'<' && !bytes[at - 1].is_ascii_whitespace() {
                continue;
            }
            if !is_external_fetch(val) {
                continue;
            }
            // `href=` fetches only on `<link>`; an `<a>`/`<area>`/`<base>` href is not a fetch.
            if attr == "href=\"" && !tag_named_before(html, at, "link") {
                continue;
            }
            push(val, at, &mut out);
        }
    }
    // (2) remote / bare `{js}` `import()` specifiers — only inside author cell bodies, so the
    //     tool's own inlined vendored libraries (d3/Plot) can't false-positive.
    for body in tali_js_cell_sources(html) {
        let base = body.as_ptr() as usize - html.as_ptr() as usize;
        for (rel_off, spec) in dynamic_import_specifiers(body) {
            if is_external_fetch(&spec) || is_bare_specifier(&spec) {
                push(&spec, base + rel_off, &mut out);
            }
        }
    }
    out
}

/// One located, informational warning per external reference the build left in `html`, so the
/// author learns a "portable" output is not self-contained at the one moment they can act.
/// Never fails the build (even under `--strict`): an external ref may be intentional, and the
/// tool deliberately does not download arbitrary URLs at build time. `label` names the document
/// for the located `path:line:` prefix (mirrors the other build warnings). Empty for an
/// all-local page. Shared by the single-doc build (logged immediately) and the site build
/// (collected into the page's deferred warning list), so both deploy shapes are covered.
fn offline_ref_warnings(html: &str, label: &str) -> Vec<String> {
    external_refs(html)
        .into_iter()
        .map(|r| {
            let loc = match r.line {
                Some(l) => format!("{label}:{l}"),
                None => label.to_string(),
            };
            format!(
                "{loc}: external reference not bundled: {} — the build will fetch it at view time, \
                 so the output is not self-contained (offline viewing fails)",
                r.url
            )
        })
        .collect()
}

#[cfg(test)]
mod mirror_tests {
    use super::*;
    use std::fs;

    #[test]
    fn external_refs_flags_remote_resources_not_hyperlinks_or_local() {
        let html = concat!(
            "<p data-sourcepos=\"3:1-3:40\"><img src=\"https://example.com/pic.png\" alt=\"a\"></p>",
            "<p data-sourcepos=\"5:1-5:20\"><a href=\"https://example.com\">link</a></p>",
            "<p data-sourcepos=\"7:1-7:20\"><img src=\"local.png\"></p>",
            "<link href=\"//cdn.test/x.css\" rel=\"stylesheet\">",
            "<img src=\"data:image/png;base64,AAAA\">",
            // data-tali-src=\"…\" contains `src=\"` but must NOT be harvested (click-to-source attr).
            "<div data-tali-src=\"post.tmd:1\"></div>",
        );
        let refs = external_refs(html);
        let urls: Vec<&str> = refs.iter().map(|r| r.url.as_str()).collect();
        assert!(
            urls.contains(&"https://example.com/pic.png"),
            "a remote <img> src is a view-time fetch: {refs:?}"
        );
        assert!(
            urls.contains(&"//cdn.test/x.css"),
            "a protocol-relative <link> href is external: {refs:?}"
        );
        assert!(
            !urls.contains(&"https://example.com"),
            "an <a> hyperlink is navigation, not a view-time fetch: {refs:?}"
        );
        assert!(
            !urls.iter().any(|u| u.contains("local.png")),
            "local ref is fine"
        );
        assert!(
            !urls.iter().any(|u| u.starts_with("data:")),
            "data: URI is inline"
        );
        assert!(
            !urls.iter().any(|u| u.contains("post.tmd")),
            "data-tali-src must not be read as a resource ref: {refs:?}"
        );
        let img = refs
            .iter()
            .find(|r| r.url == "https://example.com/pic.png")
            .unwrap();
        assert_eq!(
            img.line,
            Some(3),
            "located to the enclosing block's sourcepos"
        );
    }

    #[test]
    fn external_refs_flags_remote_and_bare_js_imports_not_relative() {
        let html = concat!(
            "<div data-sourcepos=\"8:1-11:3\" class=\"cell tali-js-cell\">",
            "<script type=\"application/tali-js\" data-target=\"x\">",
            "const three = await import(\"https://esm.sh/three@0.163.0\");\n",
            "const local = await import(\"./helper.js\");\n",
            "const bare = await import('lodash-es');\n",
            "</script></div>",
        );
        let refs = external_refs(html);
        let urls: Vec<&str> = refs.iter().map(|r| r.url.as_str()).collect();
        assert!(
            urls.contains(&"https://esm.sh/three@0.163.0"),
            "a remote dynamic import is external: {refs:?}"
        );
        assert!(
            urls.contains(&"lodash-es"),
            "a bare specifier is unresolvable offline (no import map): {refs:?}"
        );
        assert!(
            !urls.iter().any(|u| u.contains("helper.js")),
            "a relative import is bundled by copy_js_imports, not flagged: {refs:?}"
        );
        let remote = refs.iter().find(|r| r.url.contains("esm.sh")).unwrap();
        assert_eq!(remote.line, Some(8), "located to the cell's sourcepos");
    }

    #[test]
    fn external_refs_is_empty_for_a_fully_local_page() {
        // The nudge must not cry wolf: a self-contained page (local assets, inline data URIs,
        // relative imports, ordinary external <a> links) yields nothing.
        let html = concat!(
            "<p data-sourcepos=\"1:1-1:10\"><img src=\"fig.png\"><a href=\"https://ok.test\">x</a></p>",
            "<img src=\"data:image/svg+xml,%3Csvg/%3E\">",
            "<link href=\"style.css\" rel=\"stylesheet\">",
            "<div class=\"cell tali-js-cell\"><script type=\"application/tali-js\">",
            "const m = await import(\"./mod.js\");\n</script></div>",
        );
        assert_eq!(external_refs(html), Vec::new());
    }

    #[test]
    fn offline_ref_warnings_locate_the_source_and_stay_empty_for_local() {
        // The shared helper the single-doc AND site-build paths both emit through: located
        // `label:line:` prefix + the url, and silent for an all-local page.
        let html = "<p data-sourcepos=\"4:1-4:9\"><img src=\"https://x.test/y.png\"></p>";
        let w = offline_ref_warnings(html, "posts/p.tmd");
        assert_eq!(w.len(), 1);
        assert!(w[0].starts_with("posts/p.tmd:4:"), "located: {}", w[0]);
        assert!(
            w[0].contains("https://x.test/y.png"),
            "names the url: {}",
            w[0]
        );
        assert!(
            offline_ref_warnings("<p data-sourcepos=\"1:1\"><img src=\"a.png\"></p>", "p.tmd")
                .is_empty()
        );
    }

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
        // `data-tali-src="…"` (the click-to-source attribute on listing cards) *contains*
        // the substring `src="`, so a bare search harvested each post's `.tmd` and
        // `deploy_referenced_sources` published the sources into `_site/`.
        let refs = local_refs(
            r#"<a class="card" data-tali-src="posts/a/index.tmd" href="posts/a/index.html">
                 <img src="posts/a/thumb.png" alt="">
               </a>
               <div data-tali-src="_site.yml"></div>
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
        // offered for inspection). A listing card's `data-tali-src` is not a link.
        let dir = tmp_dir("deploy-refs");
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(dir.join("index.tmd"), "x").unwrap();
        fs::write(dir.join("notes.md"), "y").unwrap();

        let html = r#"<a data-tali-src="index.tmd" href="index.html">card</a>
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
        let err = parse_build_args(&argv(&[
            "taliesin", "build", "doc.tmd", "--out", "--strict",
        ]))
        .expect_err("value-less --out errors");
        assert!(err.contains("--out") && err.contains("requires"), "{err}");
        // --out at the very end (no following token) is the same hard error.
        let err = parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "--out"]))
            .expect_err("trailing --out errors");
        assert!(err.contains("--out"), "{err}");
        // --dir is the alias and errors the same way.
        assert!(parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "--dir"])).is_err());

        // flags may appear anywhere; both positionals still bind in order.
        let a = argv(&["taliesin", "build", "--strict", "doc.tmd", "out.html"]);
        let p = parse_build_args(&a).unwrap();
        assert!(p.strict);
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
            "<script type=\"application/tali-js\" data-target=\"c\">\n",
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

    /// A `<video>` names its still through `poster="…"`, an attribute that is neither
    /// `src=` nor `href=`. It was invisible to `local_refs`, so `build --out <dir>` emitted
    /// a page whose poster 404s — and a poster that fails to load also collapses the
    /// element to the UA default 150px, because no intrinsic ratio ever arrives.
    ///
    /// This used to cover a `data-src` theme-pair too. Wave 7 cut `{{< video >}}` and with
    /// it the page-shell promoter that turned `data-src` into `src`, so that half was
    /// pinning a harvest of an attribute nothing emits and nothing would load; it went with
    /// the branch on 2026-08-09. `poster=` stays: raw `<video>` HTML is in the trust model,
    /// and `diagnostics/media.rs` validates this same attribute.
    #[test]
    fn copy_local_assets_bundles_the_video_poster() {
        let base = tmp("video-attrs");
        let out = tmp("video-attrs-out");
        for f in ["clip.mp4", "still.png"] {
            fs::write(base.join(f), b"x").unwrap();
        }
        let html = "<video src=\"clip.mp4\" poster=\"still.png\"></video>\
                    <video src=\"clip.mp4\" poster=\"still.png\"></video>";

        let copied = copy_local_assets(html, &base, &out);

        for f in ["clip.mp4", "still.png"] {
            assert!(
                out.join(f).exists(),
                "`{f}` must be bundled, got {copied} copies"
            );
        }
        assert_eq!(
            copied, 2,
            "each file once, deduped across the pair: got {copied}"
        );

        let _ = fs::remove_dir_all(&base);
        let _ = fs::remove_dir_all(&out);
    }

    /// `data-tali-src="…"` is click-to-source metadata pointing at a page's `.tmd` SOURCE,
    /// and harvesting it once published every post's source into `_site/`. `src="` is a
    /// SUBSTRING of it, so the whole-attribute guard in `local_refs` is the only thing
    /// standing between the harvest and that leak — this pins the guard, not the harvest.
    ///
    /// It used to make the point by contrasting `data-src` (media, harvested) with
    /// `data-tali-src` (metadata, refused). The `data-src` branch went on 2026-08-09 with
    /// the `{{< video >}}` promoter that gave it meaning, so the contrast is now plain
    /// `src=` against `data-tali-src` — which is the pair the guard actually has to
    /// separate, and always was.
    #[test]
    fn copy_local_assets_still_refuses_the_click_to_source_attribute() {
        let base = tmp("dts");
        let out = tmp("dts-out");
        fs::write(base.join("post.tmd"), b"secret source").unwrap();
        fs::write(base.join("clip.mp4"), b"x").unwrap();
        let html = "<a data-tali-src=\"post.tmd\">card</a>\
                    <video src=\"clip.mp4\"></video>";

        let copied = copy_local_assets(html, &base, &out);

        assert!(out.join("clip.mp4").exists(), "real media src is bundled");
        assert!(
            !out.join("post.tmd").exists(),
            "click-to-source metadata must NEVER be published: got {copied} copies"
        );
        assert_eq!(copied, 1, "got {copied}");

        let _ = fs::remove_dir_all(&base);
        let _ = fs::remove_dir_all(&out);
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
            nested: Vec::new(),
        }
    }

    #[test]
    fn report_cell_errors_counts_only_tali_error_outputs() {
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

    #[test]
    fn a_cell_that_never_ran_is_not_reported_as_an_author_exception() {
        // AP11-1. With a bogus `TALIESIN_PYTHON` the build logged "code cell raised an
        // uncaught exception; its traceback is baked into the output". Both halves were
        // false: no kernel ever launched, so no cell ran and no traceback exists — the most
        // likely setup failure there is, reported as a bug in the author's code. The cause
        // was classification by HTML SHAPE: the executor's own "did not run" diagnostic is
        // a `tali-error` pre inside a `tali-output` div (deliberately, so it is styled as an
        // error and never cached), which is exactly the shape a real traceback has. So the
        // executor now marks what it wrote itself, and the message asks the marker.
        // The `<figure>` wrapper is load-bearing here, not decoration: a `#| label: fig-x`
        // cell wraps its output that way, which is exactly the case where reading the
        // diagnostic's prose back out of the HTML fails (`classify_exec_output` reports a
        // figure, not an error). The marker's kind survives it.
        let unavailable = output_block(&format!(
            "<div class=\"tali-output\"><figure id=\"fig-x\"><pre class=\"tali-error\"{}>python \
             kernel unavailable; this cell did not execute (No such file or directory (os error \
             2))</pre><figcaption>Figure&nbsp;1: Sales</figcaption></figure></div>",
            crate::exec::not_run_mark(crate::exec::NOT_RUN_UNAVAILABLE)
        ));
        let msg = cell_error_message("p.tmd", &unavailable);
        assert!(
            !msg.contains("exception") && !msg.contains("traceback"),
            "a cell that never ran must not be reported as a raised exception: {msg}"
        );
        assert!(
            msg.contains("did not run") && msg.contains("no kernel was available"),
            "the message must say what actually happened, and why: {msg}"
        );

        // The real thing still reads as the real thing: an interpreter traceback carries no
        // marker, so it keeps the exception wording (and the summary line names it).
        let raised = output_block(
            "<div class=\"tali-output\"><pre class=\"tali-error\">Traceback (most recent call \
             last)\nValueError: bad value</pre></div>",
        );
        let msg = cell_error_message("p.tmd", &raised);
        assert!(
            msg.contains("uncaught exception") && msg.contains("traceback"),
            "a genuine crash keeps its wording: {msg}"
        );

        // Both are still *problems*: they count toward `--strict` and reach `--format json`,
        // which is what AP11 verified as correct. Only the wording was wrong.
        let blocks = vec![unavailable, raised];
        assert_eq!(report_cell_errors(&blocks, "p.tmd"), 2);
        assert_eq!(cell_error_diagnostics(&blocks, "p.tmd").len(), 2);
    }

    #[test]
    fn every_executor_authored_error_block_carries_the_not_run_marker() {
        // The marker is only as good as its coverage: each of these is written by the
        // EXECUTOR about a cell that did not complete, not by the interpreter about code
        // that ran. Asserted against the real emitters rather than copies of their strings,
        // so a fourth one added without the marker fails here rather than silently
        // regressing into "raised an uncaught exception".
        //
        // The last two are the LIVE path and are why this list grew: a timeout-killed or
        // mid-cell-death output is not built by any `*_html` helper here — it is an
        // `Output::Error` rendered by `kernel::render_outputs`, which carried no marker at
        // all, so this test passed while the thing it names shipped broken. Constructed via
        // the real constructors the kernel loop calls, not copies of their strings.
        for html in [
            crate::exec::kernel_unavailable_html("python", Some("No such file or directory")),
            crate::exec::kernel_unavailable_html("r", None),
            crate::exec::KERNEL_DIED_HTML.to_string(),
            crate::exec::execution_error_html("timed out"),
            crate::kernel::render_outputs(&[crate::kernel::Output::timeout(
                "cell exceeded 120s; sent interrupt".into(),
            )]),
            crate::kernel::render_outputs(&[crate::kernel::Output::kernel_died()]),
        ] {
            assert!(
                html.contains("class=\"tali-error\""),
                "still styled + uncacheable as an error: {html}"
            );
            let b = output_block(&format!("<div class=\"tali-output\">{html}</div>"));
            assert!(
                not_run_reason(&b.html).is_some(),
                "executor-authored, so it must carry a known not-run kind: {html}"
            );
            assert!(
                !cell_error_message("p.tmd", &b).contains("exception"),
                "{html}"
            );
        }
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

    /// Every `.js` asset is written verbatim — the vendored libs because they ship already
    /// minified, and the hand-written bundles because `minify_js` was cut on 2026-08-08. That
    /// was asserted only by a code comment, which is not a thing that fails: this pins the
    /// bytes. The control is CSS, which IS still minified, so the assertions below cannot pass
    /// against a `write_asset_bundle` that had stopped writing anything at all.
    #[test]
    fn js_assets_are_written_verbatim_and_css_is_still_minified() {
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
        // The vendored pair is conditional (item 137): named by `write_asset_bundle`, written
        // only for a build whose pages link them. This test is about the BYTES, so ask for
        // both and then read them.
        bundle
            .write_conditional(
                &dir,
                AssetUse {
                    katex: true,
                    mermaid: true,
                    jslibs: true,
                },
            )
            .expect("write conditional");

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
        assert!(
            read(&bundle.app_js) == taliesin_core::core_enhance_js(),
            "app.js should ship verbatim now that minify_js is gone"
        );
        // Control: CSS IS still minified, or the assertions above would pass just as well
        // against a `write_asset_bundle` that had stopped transforming anything at all.
        assert!(
            read(&bundle.app_css).len() < taliesin_core::shared_site_css().len(),
            "app.css should still be minified"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The build's own filesystem walks, held to the same symlink boundary
/// `taliesin_core::includes` applies to every path resolved out of a document.
///
/// Two boundaries, because the two trees are owned by different parties:
///
/// * The **source** tree is authored, so a walk that publishes from it (`mirror_assets`,
///   `copy_local_assets`) may follow a symlink only while the target stays inside the
///   repository. Otherwise a link the author dropped in for convenience silently ships
///   out-of-repo files into a public deploy.
/// * The **output** tree is ours: the build never emits a symlink, so one found there is
///   the author's deliberate mount and reading through it is intended. It still must not
///   be walked twice, or a mount pointing back up the tree re-walks the whole deploy once
///   per level.
#[cfg(all(test, unix))]
mod symlink_containment_tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tali-symcontain-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).expect("temp dir");
        d
    }

    #[test]
    fn mirror_assets_refuses_a_symlink_leaving_the_repository() {
        //   <dir>/outside/secret.png                     out of tree
        //   <dir>/repo/.git
        //   <dir>/repo/paper/figures/fig.png             in-repo, above the site root
        //   <dir>/repo/book/_site.yml                    the site root
        //   <dir>/repo/book/shared -> ../paper/figures   in-repo: mirrored
        //   <dir>/repo/book/private -> ../../outside     out-of-repo: refused
        //   <dir>/repo/book/leak.png -> ../../outside/secret.png   likewise
        let dir = tmp("mirror-assets");
        let book = dir.join("repo/book");
        fs::create_dir_all(&book).unwrap();
        fs::create_dir_all(dir.join("repo/paper/figures")).unwrap();
        fs::create_dir_all(dir.join("outside")).unwrap();
        fs::write(dir.join("repo/.git"), b"").unwrap();
        fs::write(dir.join("outside/secret.png"), b"SECRET").unwrap();
        fs::write(dir.join("repo/paper/figures/fig.png"), b"FIG").unwrap();
        fs::write(book.join("_site.yml"), b"title: Book\n").unwrap();
        symlink("../paper/figures", book.join("shared")).unwrap();
        symlink("../../outside", book.join("private")).unwrap();
        symlink("../../outside/secret.png", book.join("leak.png")).unwrap();

        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        let (copied, _skipped) = mirror_assets(&book, &out);

        assert!(
            !out.join("private/secret.png").exists(),
            "a directory symlinked out of the repository must not be mirrored into the \
             deploy; copied: {copied:?}"
        );
        assert!(
            !out.join("leak.png").exists(),
            "a file symlinked out of the repository must not be mirrored either; copied: {copied:?}"
        );
        assert!(
            out.join("shared/fig.png").exists(),
            "a symlink to a sibling inside the repository is first-party authoring and \
             must still be mirrored; copied: {copied:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_local_assets_refuses_an_asset_symlinked_out_of_the_repository() {
        // The single-doc `--out` bundle resolves each `src=`/`href=` under the doc's own
        // directory. The ref is held to the lexical rule (no absolute path, no `..`), but
        // that says nothing about what the path *resolves* to.
        let dir = tmp("copy-local-assets");
        let repo = dir.join("repo");
        fs::create_dir_all(repo.join("doc")).unwrap();
        fs::create_dir_all(repo.join("paper")).unwrap();
        fs::create_dir_all(dir.join("outside")).unwrap();
        fs::write(repo.join(".git"), b"").unwrap();
        fs::write(dir.join("outside/secret.png"), b"SECRET").unwrap();
        fs::write(repo.join("paper/fig.png"), b"FIG").unwrap();
        symlink("../../outside/secret.png", repo.join("doc/leak.png")).unwrap();
        symlink("../paper/fig.png", repo.join("doc/shared.png")).unwrap();

        let dest = dir.join("bundle");
        fs::create_dir_all(&dest).unwrap();
        let html = r#"<img src="leak.png"><img src="shared.png">"#;
        let copied = copy_local_assets(html, &repo.join("doc"), &dest);

        assert!(
            !dest.join("leak.png").exists(),
            "an asset symlinked out of the repository must not be bundled"
        );
        assert!(
            dest.join("shared.png").exists(),
            "an asset symlinked to a sibling inside the repository must still be bundled \
             ({copied} copied)"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn referenced_sources_refuse_a_source_symlinked_out_of_the_repository() {
        // The second asset pass ships the source-only files a page *links* to (a `.md`
        // download, a `.scss` offered for inspection). Those are exactly the extensions
        // `mirror_assets` deliberately keeps out of the deploy, so this pass is the one
        // that would publish a symlinked private note.
        let dir = tmp("referenced-sources-escape");
        let repo = dir.join("repo");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(dir.join("outside")).unwrap();
        fs::write(repo.join(".git"), b"").unwrap();
        fs::write(dir.join("outside/diary.md"), b"# Private\n").unwrap();
        symlink("../outside/diary.md", repo.join("notes.md")).unwrap();

        let dest = dir.join("_site");
        fs::create_dir_all(&dest).unwrap();
        let copied = deploy_referenced_sources(r#"<a href="notes.md">notes</a>"#, &repo, &dest);

        assert!(
            !dest.join("notes.md").exists(),
            "a linked source symlinked out of the repository must not be deployed \
             ({copied} copied)"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn referenced_sources_are_deployed_once_through_a_mounted_directory() {
        // Same walk shape, in the pass that ships linked `.md`/`.scss` sources: without a
        // cycle guard the deploy recursed through the mount, re-resolving the same page
        // against a longer path each time and re-copying what it had already shipped.
        let dir = tmp("referenced-sources-loop");
        let root = dir.join("src");
        let out = dir.join("_site");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&out).unwrap();
        fs::write(dir.join(".git"), b"").unwrap();
        fs::write(root.join("notes.md"), b"# Notes\n").unwrap();
        symlink(".", root.join("loop")).unwrap();
        fs::write(
            out.join("index.html"),
            br#"<p>A <a href="notes.md">note</a>.</p>"#,
        )
        .unwrap();
        symlink(".", out.join("loop")).unwrap();

        let copied = deploy_referenced_sources_for_site(&root, &out);

        assert!(out.join("notes.md").is_file(), "the linked source ships");
        assert_eq!(
            copied, 1,
            "the mount must be walked once, so the linked source is deployed once"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
