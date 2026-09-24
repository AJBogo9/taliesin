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
/// here matching output-path *file extensions*. HTML is the only output: the print/PDF track
/// was cut on 2026-08-08.
const NON_HTML_OUTPUT_EXTS: &[&str] = &[
    "pdf", "docx", "doc", "odt", "rtf", "tex", "latex", "typ", "epub", "pptx", "ppt", "md",
    "markdown",
];

/// The friendly rejection for a `build … <out>` whose extension names a non-HTML format
/// ([`NON_HTML_OUTPUT_EXTS`]), or `None` when the output path is absent or an acceptable
/// target. Names the extension, hands over the concrete `.html` fix (the out path with its
/// extension swapped, so `dist/x.pdf` → `dist/x.html`) and offers the browser's Print to
/// PDF. No `error:` prefix, like the other `parse_build_args` errors (`cmd_build` frames
/// them with `log::error`).
fn non_html_output_error(out_html: Option<&str>) -> Option<String> {
    let out = out_html?;
    let ext = Path::new(out).extension()?.to_str()?.to_ascii_lowercase();
    if !NON_HTML_OUTPUT_EXTS.contains(&ext.as_str()) {
        return None;
    }
    let html = Path::new(out).with_extension("html");
    let html = html.display();
    Some(format!(
        "`build` renders HTML only, but the output path `{out}` ends in `.{ext}`. \
         Write `{html}` instead (or omit it to build `{html}` beside the source). For a PDF, \
         open the built page and use your browser's Print to PDF."
    ))
}

/// The refusal for a `build <src> <out>` whose output path is itself a Taliesin **source**
/// document. `taliesin build *.tmd` in a directory of pages expands to `build a.tmd b.tmd`,
/// and the second positional is where `build` *writes*: without this, `b.tmd`'s source is
/// replaced by rendered HTML, exit 0, log line `built b.tmd`, and the only way back is git.
///
/// Deliberately its own guard rather than a row in [`NON_HTML_OUTPUT_EXTS`]: that list's
/// message ("write `b.html` instead") answers a *format-conversion* expectation, and the
/// mistake here is a glob that handed `build` two sources, so it needs its own fix line.
/// Case-insensitive because on a case-insensitive filesystem `B.TMD` **is** `b.tmd`, even
/// though `ext::is_source_path` (which answers "is this a page to build?") is not.
fn source_output_error(out_html: Option<&str>) -> Option<String> {
    let out = out_html?;
    let ext = Path::new(out).extension()?.to_str()?.to_ascii_lowercase();
    if !taliesin_core::ext::is_source_ext(&ext) {
        return None;
    }
    Some(format!(
        "`{out}` is a Taliesin source file, and the second positional is the path \
         `build` WRITES to. Refusing to overwrite your source. `build` takes one page at a \
         time, so a shell glob (`build *.tmd`) hands it the next page as the output path — \
         build the whole project instead (`build <dir>`), or name an `.html` output."
    ))
}

/// Parse `build` argv (`args[2..]`; `args[0..2]` are the binary + "build") by the grammar
/// every verb shares ([`crate::serve::parse_args`]). Flags may appear anywhere; the first
/// positional is the source, the optional second is `[out.html]`, and a third is refused.
/// Returns `Err(usage/error message)` for a bad `--jobs` value, a value-less `--out`, an
/// unknown flag, an extra positional, or a missing source path.
fn parse_build_args(args: &[String]) -> Result<BuildArgs<'_>, String> {
    let mut out_dir: Option<&str> = None;
    let mut strict = false;
    let mut no_exec = false;
    let mut stdout = false;
    let mut check_only = false;
    // The `--jobs` value as typed, when given: `--check-only` refuses the flag whatever its
    // value, so `0` and `auto` (both "no cap") must not read as "not given".
    let mut jobs_given: Option<&str> = None;
    let mut jobs: Option<usize> = None;
    let mut format: &str = "human";
    let positionals =
        crate::serve::parse_args("build", &args[2..], BUILD_FLAGS, 2, |flag, value| {
            match flag {
                // `--format human|json`: mirror `check`'s flag exactly.
                "--format" => match value.take() {
                    Some(v @ ("human" | "json")) => format = v,
                    other => return Err(crate::serve::bad_format_error(other)),
                },
                // `--json`: clig.dev shorthand for `--format json`, accepted on every
                // machine-output command so neither spelling dead-ends.
                "--json" => format = "json",
                // `--out <dir>` needs a real value. A missing one (end of args, or a flag
                // follows) is a hard error rather than silently leaving out_dir None and
                // writing `<stem>.html` to an unexpected place. (`--out` = output dir; the
                // undocumented `--dir` alias was dropped: `--dir` is the scaffold-input flag.)
                "--out" => match value.take() {
                    Some(v) => out_dir = Some(v),
                    None => {
                        return Err(format!(
                            "{flag} requires a directory value (e.g. {flag} site)"
                        ));
                    }
                },
                "--jobs" | "-j" => {
                    let raw = value.take();
                    jobs = parse_jobs_value(raw)?;
                    jobs_given = raw;
                }
                "--strict" => strict = true,
                // `--no-exec`: render code cells as source, deliberately. `serve` has accepted
                // it all along (as sugar for `TALIESIN_NO_EXEC`); `build` had only the env var,
                // which is a poor thing to make someone reach for now that a missing kernel
                // *fails* the build. This is that failure's opt-out.
                "--no-exec" => no_exec = true,
                // `--stdout`: the page to stdout rather than to a file. This is the whole of what
                // the `render` verb was, minus a second code path; pair it with `--no-exec` for
                // `render`'s static, kernel-free dump.
                "--stdout" => stdout = true,
                // `--check-only`: lint, write nothing. Never executes a cell, so it needs no
                // `--no-exec` (and accepts one, which agrees with it rather than contradicting it).
                "--check-only" => check_only = true,
                // Anything else is refused with a did-you-mean. **Any leading dash counts, not
                // just `--`.** `-o` is the output flag in most other renderers, so it is a likely
                // typo here; with only `--` rejected it fell through to the positionals and became
                // the output *path*, writing a file named `-o` that then resists `rm`/`cat`
                // without a `--` sentinel. A genuinely dash-named source file is still buildable,
                // as `./-weird.tmd`.
                _ => return Ok(false),
            }
            Ok(true)
        })?;
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
    // The data-loss sibling of DX11, and checked in the same place for the same reason: a
    // second positional that is itself a `.tmd` is a source file about to be overwritten with
    // rendered HTML. Disjoint from the denylist above (`tmd` is not in it), so the order of
    // the two is free.
    if let Some(msg) = source_output_error(positionals.get(1).copied()) {
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
                "--stdout writes the page to stdout, but {other} writes it to a file. \
                 Pick one."
            ));
        }
        if format == "json" {
            return Err(
                "--stdout and --format json both write to stdout. Use one or the other."
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
            .or_else(|| jobs_given.map(|n| format!("--jobs {n}")))
    {
        return Err(format!(
            "--check-only writes nothing, but {other} describes output. Drop one."
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
        // A missing path prints the bare usage line, as `preview` does.
        Err(msg) if msg.starts_with("usage:") => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
        Err(msg) => {
            log::error(&msg);
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
        // The same for the single-file `[out.html]`: a project builds to a directory, and
        // silently building `_site/` while ignoring the name the author typed is the trap.
        if let Some(out) = out_html {
            log::error(&format!(
                "{path} is a project, which builds to a directory, so the output file `{out}` \
                 does not apply. Name the directory with --out <dir>, or leave it out."
            ));
            return ExitCode::FAILURE;
        }
        return build_site(Path::new(path), out_dir, strict, jobs, json);
    }
    // Through the one normalizing reader, like every other `.tmd` read: a lone-CR file was
    // one line to the front-matter scans here while the renderer split it, so its broken
    // front matter went unreported and the build passed.
    let src = match taliesin_core::includes::read_source(Path::new(path)) {
        Ok(s) => s,
        Err(e) => {
            log::error(&crate::lint::cannot_read(Path::new(path), &e));
            return ExitCode::FAILURE;
        }
    };
    let p = Path::new(path);
    // A single file is a source document iff its extension is accepted
    // (`taliesin_core::ext::is_source_path`, the same vocabulary the site walker
    // discovers by): a `note.md` built here would still be invisible to `build <dir>`,
    // silently, at exit 0. Checked after the read so a missing file keeps its
    // `cannot_read` did-you-mean.
    if !taliesin_core::ext::is_source_path(p) {
        log::error(&crate::serve::not_a_source_error(p, "build"));
        return ExitCode::FAILURE;
    }
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
    let executed = crate::serve::guarded(|| {
        // The project this document belongs to, discovered for this one document: the same
        // `Site` its preview finishes the page through, so the two agree on its `hero:`, its
        // `listing:`, its numbering and its table of contents (audit 2026-09-24,
        // config-seam #4), and on the project `_freeze/` entry and `python:` it runs with.
        let site = taliesin_core::Site::discover_document(p);
        build_page_executing(&site, src, stem, path, mermaid_src)
    });
    let (html, mut problems, unparseable, mut diagnostics, kernel_failure) = match executed {
        Ok(Ok(BuiltPage {
            html,
            problems,
            unparseable,
            diagnostics,
            kernel_failure,
        })) => (html, problems, unparseable, diagnostics, kernel_failure),
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
    // rather than download — the tool does not fetch arbitrary URLs at build time. Carried
    // into `--format json` below (the machine surface must see what the console sees) but
    // never counted into `problems`: the `--strict` exemption is deliberate, and the CLI
    // reference documents the carve-out.
    for w in &offline_ref_warnings(&html) {
        log::warn(&locate(w, path));
        diagnostics.push(crate::lint::diag_from(w, path));
    }

    // In `--strict` mode, a cell that crashed (its traceback is baked into the HTML)
    // or any located warning fails the build instead of shipping a broken page with
    // exit 0. Without `--strict` the warnings were already logged; we still write.

    // `--stdout`: the page IS the output, so nothing is written and nothing is copied
    // beside it (an asset the page references stays where the author put it — a stdout
    // dump has no directory of its own to populate). The human log is already on stderr,
    // so the HTML pipes cleanly. `--format json` is refused beside it (both are stdout).
    if stdout {
        print!("{html}");
        return finalize_build(
            true,
            strict,
            problems,
            unparseable,
            kernel_failure.as_deref(),
            None,
        );
    }
    // The page, then the local files it references copied beside it, so the output keeps
    // working away from the source tree (a no-op for an in-place build).
    let written = match out_dir {
        Some(dir) => build_dir(&html, base, Path::new(dir)),
        None => {
            let out: PathBuf = out_html
                .map(PathBuf::from)
                .unwrap_or_else(|| base.join(format!("{stem}.html")));
            match std::fs::write(&out, &html) {
                Ok(()) => {
                    let bundled = copy_local_assets(&html, base, out.parent().unwrap_or(base));
                    Some((out, bundled))
                }
                Err(e) => {
                    log::error(&format!("cannot write {}: {e}", out.display()));
                    None
                }
            }
        }
    };
    // The `built` line is printed by `finalize_build`, and only for a build that succeeded.
    let mut built = None;
    if let Some((page, bundled)) = &written {
        for w in &bundled.problems {
            log_located(w, path);
            diagnostics.push(crate::lint::diag_from(w, path));
        }
        problems += crate::lint::blocking(&bundled.problems);
        // A folder always says what it holds; a single file mentions its copies only when
        // it made some, so an in-place build (whose assets are already beside it) is quiet.
        let assets = if out_dir.is_some() || bundled.copied > 0 {
            format!(
                "  ·  {} asset{}",
                bundled.copied,
                if bundled.copied == 1 { "" } else { "s" }
            )
        } else {
            String::new()
        };
        built = Some(format!(
            "{}{assets}{}",
            page.display(),
            elapsed_note(started)
        ));
    }
    // Structured diagnostics to stdout (the human log stays on stderr, so the JSON stream
    // pipes cleanly), after the bundling pass so its refusals are in it too. The page is
    // still written: `--format json` only changes the *reporting* channel.
    if json {
        println!("{}", crate::lint::diagnostics_json(&diagnostics));
    }
    finalize_build(
        written.is_some(),
        strict,
        problems,
        unparseable,
        kernel_failure.as_deref(),
        built,
    )
}

/// `path:line: message` for a located warning, falling back to `path: message` for one the
/// renderer could not place. `fallback` names the document the warning came from; one
/// located in an `{{< include >}}`d file names that file beside it ([`crate::lint::diag_from`]).
pub(crate) fn locate(w: &taliesin_core::render::Warning, fallback: &str) -> String {
    crate::lint::diag_from(w, fallback).located()
}

/// Print one located diagnostic at the severity its validator gave it.
///
/// Every one of these sites called [`log::warn`] unconditionally until 2026-08-13, which
/// printed an `error` as `warn` and so made `build` disagree with `--check-only` about the
/// same diagnostic on the same document. Severity is a field on `render::Warning`, set by
/// the validator that found the defect, so `build` and `--check-only` can no longer
/// deciding what fails the run — so a reporting channel that discards it is the channel
/// that has to be fixed, not the exit code alone.
pub(crate) fn log_located(w: &taliesin_core::render::Warning, fallback: &str) {
    log_diag(&crate::lint::diag_from(w, fallback));
}

/// Print one diagnostic, located, at its severity.
fn log_diag(d: &crate::lint::Diagnostic) {
    let line = d.located();
    match d.severity {
        taliesin_core::Severity::Error => log::error(&line),
        _ => log::warn(&line),
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

/// Final exit for a single-doc build. An **unparseable** YAML block (front matter or
/// `_site.yml`) fails unconditionally; `--strict` widens the failure to every other problem.
/// Either way the page is still written, but CI gets a non-zero exit. A non-strict build
/// that shipped with problems prints a closing tally so the silent degradation is visible
/// (DX12). `wrote` is false only on a write/create error, which already failed and
/// reported itself, so no summary applies.
///
/// **Only unparseable YAML is unconditional, and the line is deliberate.** A broken
/// cross-reference or a dead link is `error` severity too, but the tool has always shipped
/// a page carrying one and left the exit to `--strict`. Widening this to all of severity
/// `error` also fails `--no-exec`, where an unexecuted `{js}` figure's own `@fig-` ref is
/// broken *by the flag* rather than by the document -- and `--no-exec` is what the
/// pre-push gate and `tools/publish.sh --check` run. An unparseable block is different
/// in kind: nothing in it was read, so the page silently lost its `title:`,
/// `bibliography:` and `listing:` while reporting success.
///
/// `built` is the `built <what>` line for a build that wrote something, and it is printed
/// only once the verdict is success: a failed build that announced `built` first read as a
/// success followed by an unrelated error (first-hour #12).
fn finalize_build(
    wrote: bool,
    strict: bool,
    problems: usize,
    unparseable: usize,
    kernel_failure: Option<&str>,
    built: Option<String>,
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
    // Before `--strict` for the same reason: "this block did not parse" is the more
    // specific report, and naming `--strict` in its message would be a lie -- the flag is
    // not what failed this build and turning it off will not un-fail it.
    if unparseable > 0 {
        warn_unparseable(unparseable);
        return ExitCode::FAILURE;
    }
    if strict && problems > 0 {
        warn_strict(problems);
        return ExitCode::FAILURE;
    }
    if let Some(line) = built {
        log::built(&line);
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

/// Log the unconditional unparseable-YAML failure summary (shared by both build paths).
///
/// Deliberately does not mention `--strict`: that flag neither caused this failure nor can
/// suppress it, and the non-strict tally's "run with --strict to fail the build" line right
/// beside it would read as the opposite advice.
fn warn_unparseable(unparseable: usize) {
    log::error(&format!(
        "{unparseable} unparseable YAML block{}; failing the build. Every key in it was \
         dropped, so the output was written without it.",
        if unparseable == 1 { "" } else { "s" }
    ));
}

/// Log the unconditional "a page did not read or write" failure summary for the site path.
///
/// Like [`warn_unparseable`], it says nothing about `--strict`: the flag neither caused
/// this nor can suppress it. What it does say is what the stale output means, because the
/// page that failed to write still has a URL in the deploy and the sweep kept it.
fn warn_io_failures(pages: usize) {
    log::error(&format!(
        "{pages} page{} could not be read or written; failing the build. The output still \
         holds the previous build's copy of {}.",
        if pages == 1 { "" } else { "s" },
        if pages == 1 { "it" } else { "them" }
    ));
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

/// The located "cell error" message for a failed cell — one string shape shared by
/// the single-doc and site build paths (and their structured-diagnostic mirror).
///
/// Two different things land here and they must not be described the same way: a cell that
/// RAN and raised (its traceback is baked into the page, and the fix is in the author's
/// code), and a cell that never ran at all because the executor could not reach a kernel
/// (the fix is `TALIESIN_PYTHON` or the environment).
fn cell_error_message(page_label: &str, f: &exec::CellFailure) -> String {
    let where_ = f
        .source_file
        .as_deref()
        .map(|file| format!("{file} "))
        .unwrap_or_default();
    format!(
        "cell error in {page_label} ({where_}@ {}): {}",
        f.sourcepos,
        failure_reason(f.failure)
    )
}

/// What a failed cell's console line says about why.
fn failure_reason(failure: exec::Failure) -> &'static str {
    use crate::exec::{Failure, NOT_RUN_CRASHED, NOT_RUN_DIED, NOT_RUN_REQUEST, NOT_RUN_TIMEOUT};
    match failure {
        Failure::Raised => {
            "code cell raised an uncaught exception; its traceback is baked into the output"
        }
        // The executor logs the full "which interpreter, and why it could not launch"
        // diagnostic separately, once per language, so this line does not repeat it.
        Failure::NotRun(NOT_RUN_CRASHED) => {
            "code cell crashed the kernel: its process exited while this cell ran, so the \
             cells after it did not run"
        }
        Failure::NotRun(NOT_RUN_DIED) => {
            "code cell did not run: the kernel exited during an earlier cell; it runs again \
             next time"
        }
        Failure::NotRun(NOT_RUN_REQUEST) => {
            "code cell did not complete: the execution request failed"
        }
        Failure::NotRun(NOT_RUN_TIMEOUT) => {
            "code cell did not complete: it hit a liveness cap and was interrupted \
             (a cell producing no output for TALIESIN_CELL_SILENCE seconds, default 600; \
             or TALIESIN_CELL_TIMEOUT if you set a wall-clock cap). Printing progress \
             from a long cell keeps it alive; 0 disables either cap"
        }
        Failure::NotRun(_) => "code cell did not run: no kernel was available for its language",
        Failure::Truncated => {
            "code cell's output was cut at an output cap, so the page shows only part of it"
        }
    }
}

/// The "cell error" diagnostics (build-only additions over `check`'s superset), in document
/// order: each failed cell's own sentence, for the console and for `--format json` alike.
///
/// A hidden (`#| include: false`) cell's failure is counted but not repeated here: the
/// executor already reported it, located, with what the cell raised. The executor says which
/// cells failed ([`exec::Executor::take_failures`]); reading it back out of the HTML was
/// spoofable by a cell that merely printed the error markup (audit exec #11).
fn cell_error_diagnostics(
    failures: &[exec::CellFailure],
    page_label: &str,
) -> Vec<crate::lint::Diagnostic> {
    failures
        .iter()
        .filter(|f| !f.hidden)
        .map(|f| {
            crate::lint::Diagnostic::new(
                page_label.to_string(),
                None,
                cell_error_message(page_label, f),
            )
        })
        .collect()
}

/// A single document built: the rendered HTML + its `--strict` problem count.
struct BuiltPage {
    html: String,
    problems: usize,
    /// The `error`-severity subset of `problems`: diagnostics saying the document is
    /// *wrong* rather than merely degraded. These fail the build with no `--strict`.
    /// A crashed code cell is counted in `problems` but never here.
    unparseable: usize,
    /// The located diagnostics, structured, for `--format json`. Same set the human
    /// log emits, in the same order.
    diagnostics: Vec<crate::lint::Diagnostic>,
    /// Set when the document has executable cells whose kernel could not start: the
    /// full "here is everything I searched" report. The page is still written (as
    /// under `--strict`), then the build exits non-zero with this message.
    kernel_failure: Option<String>,
}

/// Build one document to a self-contained HTML page, executing its cells first so figures
/// and `define(...)` outputs are baked in: [`crate::lint::PagePass`], the one page pass,
/// over the one page `site` (the document's own discovery) holds. A missing kernel leaves
/// the cells as source, matching the preview, and is reported in `kernel_failure`.
///
/// Two names, deliberately: `stem` is the page-title fallback, while `label` is what a
/// diagnostic is prefixed with and so must be a path an editor can open (the path as the
/// user typed it). They used to be one `fallback` argument carrying `file_stem()`, which made
/// every single-doc diagnostic read `pca-geometry:12:`, a name no tool resolves.
///
/// `mermaid_src` is [`MERMAID_FILE`] on the `--out <dir>` path and `""` everywhere else; see
/// [`build_dir`], which writes the file this names.
fn build_page_executing(
    site: &taliesin_core::Site,
    src: String,
    stem: &str,
    label: &str,
    mermaid_src: &str,
) -> std::io::Result<BuiltPage> {
    let page = site
        .pages
        .first()
        .expect("a document's own discovery holds exactly its page");
    let rt = tokio::runtime::Runtime::new()?;
    Ok(rt.block_on(async {
        // The located diagnostics, structured, for `--format json`: the same set the human log
        // prints.
        let mut diagnostics: Vec<crate::lint::Diagnostic> = Vec::new();
        // The project's own diagnostics: its `_site.yml` (whose `python:`, `bibliography:` and
        // book order this page is built with) and this page's front matter as discovery reads
        // it, counted as the site build counts them. A malformed `_site.yml` fails the build
        // like a malformed front matter: nothing in it was read.
        let config = crate::lint::project_label(label, page, site);
        let mut problems = crate::lint::blocking(&site.warnings);
        let mut unparseable = site
            .warnings
            .iter()
            .filter(|w| taliesin_core::site::is_malformed_config_warning(w))
            .count();
        for d in crate::lint::project_diagnostics(site, &config) {
            log_diag(&d);
            diagnostics.push(d);
        }
        let render = crate::lint::PageRender::of(site, page);
        let mut pass = crate::lint::PagePass::begin(&render, page, src, label);
        // What the page says as written prints before any cell runs.
        pass.diags.iter().for_each(log_diag);
        let printed = pass.diags.len();
        let mut exec = page_executor(site, page);
        // Execution's own findings are already on the console: the executor printed each at
        // its cell. They ride `--format json` only.
        let announced = pass.execute(&mut exec).await;
        pass.finish(site, page);
        // The links this page carries, judged against the one page it is. This build writes
        // nothing else, so a link to another page of its project, or to a sibling document,
        // is dead in the file it writes: it is reported, and written as the `.html` URL the
        // preview writes, rather than passed as a link to the raw source (audit 2026-09-24,
        // config-seam #15).
        pass.add(&site.validate_cross_page_links_for(&page.rel));
        for (i, d) in pass.diags.iter().enumerate().skip(printed) {
            if !announced.contains(&i) {
                log_diag(d);
            }
        }
        // A crashed cell bakes its traceback into the page; name it and count it.
        let cells = cell_error_diagnostics(&pass.failures, label);
        for d in &cells {
            match d.severity {
                taliesin_core::Severity::Error => log::error(&d.message),
                _ => log::warn(&d.message),
            }
        }
        problems += pass.problems;
        unparseable += pass.unparseable;
        diagnostics.append(&mut pass.diags);
        diagnostics.extend(cells);
        // The Cmd-K index is this same `Site`'s, inlined, because one file has no
        // `search-index.js` beside it. It is the index `preview <file.tmd>` serves; the
        // palette used to build its own from the DOM here, and searched raw TeX and
        // `<script>` bodies the preview's index does not.
        let search_index = site.inline_search_index(page);
        BuiltPage {
            html: single_doc_page(&pass.doc, stem, mermaid_src, &search_index),
            problems,
            unparseable,
            diagnostics,
            // Executable cells that could not execute: fatal, not a warning. Carried out
            // rather than reported here so the page is still written first, the same shape as
            // `--strict`, which writes and then fails.
            kernel_failure: pass.kernel_failure,
        }
    }))
}

/// The one page `build <file>` writes, from its finished document: self-contained, the
/// mermaid library inline unless `mermaid_src` names the sibling file a `--out` folder
/// carries, and its `.tmd` links written as the `.html` URLs the preview writes.
fn single_doc_page(
    doc: &taliesin_core::RenderedDoc,
    stem: &str,
    mermaid_src: &str,
    search_index: &str,
) -> String {
    let html = taliesin_core::render_doc_to_page(
        doc,
        stem,
        None,
        search_index,
        taliesin_core::AssetMode::Inline { mermaid_src },
    );
    taliesin_core::site::rewrite_tmd_links(&html)
}

/// A page's own executor: its `_freeze/` entry under the project root, keyed by the page's
/// path in the project (`<root>/_freeze/posts/p.json`, the entry the preview and the site
/// build use for the same page), cwd its own folder, and the interpreter the project's
/// `python:` names, resolved from the project root. A page built on its own and the same page
/// in its project's build SHARE this cache, so both name it the same way; the single-file
/// build used to key it by the file's canonical path, which a symlinked page resolved out
/// of the project, writing a second `_freeze/<stem>.json` (audit 2026-09-24,
/// config-seam #14).
fn page_executor(site: &taliesin_core::Site, page: &taliesin_core::site::Page) -> exec::Executor {
    let base = page.input.parent().unwrap_or(&site.root);
    let mut exec =
        exec::Executor::with_freeze(freeze::page_path(&site.root.join("_freeze"), &page.rel))
            .in_dir(base)
            .in_project(&site.root);
    exec.set_interpreters(crate::interpreter::resolve_python(
        site.config.python.as_deref(),
        &site.root,
    ));
    exec
}

#[cfg(test)]
mod single_doc_toc_tests {
    use super::*;

    /// Build one standalone document the way `cmd_build` does and return its page HTML.
    fn build_one(dir: &Path, name: &str, src: &str) -> String {
        let file = dir.join(name);
        std::fs::write(&file, src).expect("write doc");
        let stem = name.strip_suffix(".tmd").unwrap_or(name);
        build_page_executing(
            &taliesin_core::Site::discover_document(&file),
            src.to_string(),
            stem,
            file.to_str().expect("utf-8 path"),
            "",
        )
        .expect("runtime")
        .html
    }

    fn doc(front: &str, headings: usize) -> String {
        let mut s = format!("---\ntitle: \"T\"\n{front}---\n\nAn opening paragraph.\n");
        for i in 0..headings {
            s.push_str(&format!("\n## Section {i}\n\nProse under it.\n"));
        }
        s
    }

    /// `build <file.tmd>` and `preview <file.tmd>` must answer the same question about a
    /// table of contents. The build was the only page path that never built a `Site`, so
    /// `Site::page_toc`'s auto-gate never ran for it: the same three-heading paper listed
    /// entries in the preview and none in the build, and the manual documented the
    /// preview's answer for both. The explicit cases are the control — they passed before
    /// this and must keep passing, or the fix has replaced one divergence with another.
    #[test]
    fn a_single_file_build_auto_gates_its_toc_exactly_as_the_preview_does() {
        let dir =
            std::env::temp_dir().join(format!("tali-build-{}-single-doc-toc", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let toc = |html: &str| html.contains("<nav id=\"TOC\"");

        // Auto: at/above MIN_TOC_HEADINGS it earns one, below it does not.
        assert!(
            toc(&build_one(&dir, "long.tmd", &doc("", 3))),
            "a 3-heading page with no `toc:` earns the automatic TOC the preview renders"
        );
        assert!(
            !toc(&build_one(&dir, "short.tmd", &doc("", 2))),
            "the auto-gate still gates: a 2-heading page reads as one column"
        );
        // Explicit: unchanged either way.
        assert!(
            !toc(&build_one(&dir, "off.tmd", &doc("toc: false\n", 3))),
            "an explicit `toc: false` suppresses it regardless of length"
        );
        assert!(
            toc(&build_one(&dir, "on.tmd", &doc("toc: true\n", 1))),
            "an explicit `toc: true` forces it on regardless of length"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The sibling copy of the vendored mermaid library a `--out <dir>` build writes for a page
/// with a diagram. One name for both halves (the href handed to the renderer and the file
/// written here), so the page cannot point at something that was never written.
const MERMAID_FILE: &str = "mermaid.min.js";

/// Write `<dir>/index.html` and copy each referenced local asset (an `src=`/
/// `href=` value pointing to an existing file under `base`) to the same relative
/// path under `dir`, leaving the HTML's paths untouched so the folder is portable.
/// Returns the page written and what was bundled beside it, or `None` when the page could
/// not be written (the caller reports and finalizes, so a non-strict problem tally / a
/// `--strict` failure decide the exit uniformly with the single-file path).
fn build_dir(html: &str, base: &Path, dir: &Path) -> Option<(PathBuf, Bundled)> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::error(&format!("cannot create {}: {e}", dir.display()));
        return None;
    }
    let mut bundled = copy_local_assets(html, base, dir);
    // The mermaid library, for a page that has a diagram. Not reachable through
    // `copy_local_assets`: its href is a string inside the loader script, not an `src=`
    // attribute, and it comes from the binary rather than from `base`. Content-gated exactly
    // as the inline path was, so a prose page gains nothing.
    if taliesin_core::has_mermaid(html) {
        let to = dir.join(MERMAID_FILE);
        match std::fs::write(&to, taliesin_core::mermaid_min_js()) {
            Ok(()) => bundled.copied += 1,
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
        return None;
    }
    Some((index, bundled))
}

/// What [`copy_local_assets`] left beside a page: how many referenced files are in place
/// at the destination, and the references it refused to bundle, located at the block
/// that carries each one so the caller can report and count them like any diagnostic.
struct Bundled {
    copied: usize,
    problems: Vec<taliesin_core::render::Warning>,
}

/// Copy each referenced local asset (a relative `src=`/`href=` under `base`) to
/// the same relative path under `dest`, so a built page's images/audio/etc. travel
/// with it. Shared by the portable `--out` folder and the single-file build (so
/// `build doc.tmd out.html` into another directory isn't left with dangling asset
/// references). An in-place build copies nothing: every file is already where the page
/// points.
///
/// Which files it may place is the one publication rule,
/// [`taliesin_core::includes::publishable`], with the page's own folder as the boundary:
/// the output mirrors that folder, so a file above it (a project image a page reaches as
/// `../img/i.png`) has nowhere to go. Each existing file it cannot place for THAT reason is
/// an error-severity warning located at its reference, because the output then points at a
/// file it does not have: a warning `--strict` ignored until the 2026-09-24 audit. A file
/// no build publishes (a `.`-prefixed path, a symlink out of the checkout) is the page's
/// defect, not the folder's: the lint reports it for every verb, so saying it here too
/// printed one defect twice.
///
/// **It never replaces a different file.** The destination is a directory the author
/// chose, not one this build owns, so a file already at the target path with other bytes
/// is someone's: another post built into the same folder, or a file of the author's that
/// shares the name. Overwriting it was silent data loss, so it is refused with an error
/// naming both paths. The same bytes are what a rebuild into the same folder finds, and
/// count as bundled.
fn copy_local_assets(html: &str, base: &Path, dest: &Path) -> Bundled {
    use taliesin_core::includes::{Reach, Unpublishable, publishable};
    let mut copied = 0usize;
    let mut problems = Vec::new();
    // `Path::new("doc.tmd").parent()` is the empty path, which names the cwd.
    let dir = |p: &Path| match p.as_os_str().is_empty() {
        true => PathBuf::from("."),
        false => p.to_path_buf(),
    };
    if same_file(&dir(base), &dir(dest)) {
        return Bundled { copied, problems };
    }
    // Destinations already accounted for, so two spellings of one file (`a.png` and
    // `./a.png`, `my%20pic.png` and `my pic.png`) are bundled and counted once.
    let mut placed = std::collections::HashSet::new();
    for (r, at) in local_refs(html) {
        // The filesystem path comes from the shared resolution step (`asset_fs_path`,
        // also behind the local-asset validator and the dev server's request decode):
        // no ?query / #fragment (a static host ignores those, so `img.png?v=2` is the
        // file `img.png`) and `%XX` decoded, so `my%20image.png` is the file
        // `my image.png` — copied under its DECODED name, the one a static host
        // resolves the emitted src to. Decoded BEFORE the rule is asked, so an encoded
        // `..` cannot slip past it.
        let path = taliesin_core::render::asset_fs_path(&r);
        // The file a reference names in place: a root-absolute `/img/i.png` is served from
        // the project root (the preview's, and a root deploy's), which a folder built from
        // one page cannot reproduce.
        let on_disk = match path.strip_prefix('/') {
            Some(rooted) => taliesin_core::single_doc_root(base).join(rooted),
            None => base.join(&path),
        };
        let rel = match publishable(base, base, Path::new(&path), Reach::Referenced) {
            Ok(rel) => rel,
            // Above the document's folder: the one thing the folder cannot hold that the
            // project publishes.
            Err(Unpublishable::Outside) if on_disk.is_file() => {
                let mut w = taliesin_core::render::Warning::new(format!(
                    "asset not bundled: `{path}` is outside the document's folder, so the \
                     output points at a file it does not have"
                ))
                .severity(taliesin_core::Severity::Error);
                w.file = source_file_before(html, at);
                w.line = sourcepos_line_before(html, at);
                problems.push(w);
                continue;
            }
            // A reference that names no file (a link to a page URL, `/`) has nothing to
            // bundle, and one no build publishes is reported by the lint.
            Err(_) => continue,
        };
        let from = base.join(&rel);
        if !from.is_file() {
            continue; // e.g. an href to something that isn't a local file
        }
        let to = dest.join(&rel);
        if !placed.insert(to.clone()) {
            continue;
        }
        match bundle_file(&from, &to, &path) {
            Ok(placed) => copied += usize::from(placed),
            Err(mut w) => {
                w.file = source_file_before(html, at);
                w.line = sourcepos_line_before(html, at);
                problems.push(w);
            }
        }
    }
    copied += copy_js_imports(html, base, dest, &mut problems);
    Bundled { copied, problems }
}

/// Put `from` at `to`, beside a page built into a directory this build does not own.
/// `Ok(true)` once the file is in place (copied, or the same bytes were already there),
/// `Ok(false)` after an I/O failure it has logged, and an error-severity warning naming
/// `shown` (the reference as the page spells it) when a DIFFERENT file already holds `to`:
/// that file is someone's, so it is never replaced (see [`copy_local_assets`]).
fn bundle_file(
    from: &Path,
    to: &Path,
    shown: &str,
) -> Result<bool, taliesin_core::render::Warning> {
    if to.exists() {
        if std::fs::read(to).ok() == std::fs::read(from).ok() {
            return Ok(true);
        }
        return Err(taliesin_core::render::Warning::new(format!(
            "asset not bundled: `{shown}` would replace {}, a different file already there \
             (remove it to let the build copy over it)",
            to.display()
        ))
        .severity(taliesin_core::Severity::Error));
    }
    if let Some(parent) = to.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::copy(from, to) {
        Ok(_) => Ok(true),
        Err(e) => {
            log::warn(&format!("cannot copy {}: {e}", from.display()));
            Ok(false)
        }
    }
}

/// Ship each file a page references that [`mirror_assets`] left out: a source-only
/// extension ([`SKIP_EXT`]: a linked `.md` download, a `.scss` offered for inspection) or
/// anything under an `_`-prefixed folder (an `_images/` picture, a figure kept beside an
/// `_includes/` partial, a navbar `logo:`). A reference is intentional, so dropping it
/// left a broken page on a green build: until the 2026-09-24 audit this pass shipped the
/// source extensions only, and an image in `_images/` was served by the preview, passed
/// both gates and was missing from every deploy.
///
/// Judged by the one publication rule, [`taliesin_core::includes::publishable`], as a
/// reference from the page's directory `page_dir` (relative to `root`), so a nested page's
/// `../../_images/x.png` resolves against the project, and a `.`-prefixed path, a climb out
/// of the project or a symlink out of the repository never ships. `shipped` holds every
/// out-relative path this build has already written (pages, the mirror's copies, earlier
/// pages' references), so each file is copied and counted once. Returns the count copied.
///
/// `judged` holds each reference already judged, as its page's folder joined with its
/// spelling: the same spelling from the same folder gets the same answer, so it is judged
/// once. Every page of a book links every chapter and judging costs two canonicalizations,
/// so a 500-page book made 250,000 of them, 8 million `readlink`s and over half its build.
fn deploy_referenced_sources(
    html: &str,
    root: &Path,
    page_dir: &Path,
    out: &Path,
    shipped: &mut std::collections::HashSet<PathBuf>,
    judged: &mut std::collections::HashSet<PathBuf>,
) -> usize {
    local_refs(html)
        .into_iter()
        .filter(|(r, _)| judged.insert(page_dir.join(r)))
        .filter(|(r, _)| ship_referenced(r, root, page_dir, out, shipped))
        .count()
}

/// Ship the one file `r` (a reference as a page spells it, from `page_dir` under `root`)
/// names, unless the publication rule refuses it, it is no file, or `shipped` already has
/// it. Whether it was copied. See [`deploy_referenced_sources`].
fn ship_referenced(
    r: &str,
    root: &Path,
    page_dir: &Path,
    out: &Path,
    shipped: &mut std::collections::HashSet<PathBuf>,
) -> bool {
    use taliesin_core::includes::{Reach, publishable};
    // Decode through the shared resolution step (T3): a `%20`-spelled link must find the
    // on-disk file with the space, and the DECODED name is what a static host resolves
    // the emitted href to.
    let path = taliesin_core::render::asset_fs_path(r);
    let Ok(rel) = publishable(
        root,
        &root.join(page_dir),
        Path::new(&path),
        Reach::Referenced,
    ) else {
        return false;
    };
    let from = root.join(&rel);
    if shipped.contains(&rel) || !from.is_file() {
        return false;
    }
    let to = out.join(&rel);
    if let Some(parent) = to.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::copy(&from, &to).is_ok() && shipped.insert(rel)
}

/// Second asset pass for a site build: after every page is written, ship the files pages
/// actually *reference* that the mirror left out. The output tree mirrors the source tree,
/// so each page's relative refs resolve from its source directory. `shipped` is the set of
/// out-relative paths the build already wrote. Returns the count deployed. See
/// [`deploy_referenced_sources`].
fn deploy_referenced_sources_for_site(
    root: &Path,
    out: &Path,
    shipped: &mut std::collections::HashSet<PathBuf>,
) -> usize {
    fn walk(
        dir: &Path,
        root: &Path,
        out: &Path,
        seen: &mut std::collections::HashSet<PathBuf>,
        judged: &mut std::collections::HashSet<PathBuf>,
        shipped: &mut std::collections::HashSet<PathBuf>,
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
                walk(&p, root, out, seen, judged, shipped, copied);
            } else if p.extension().and_then(|s| s.to_str()) == Some("html") {
                let Ok(html) = std::fs::read_to_string(&p) else {
                    continue;
                };
                let rel_dir = p
                    .strip_prefix(out)
                    .ok()
                    .and_then(Path::parent)
                    .unwrap_or(Path::new(""));
                *copied += deploy_referenced_sources(&html, root, rel_dir, out, shipped, judged);
            }
        }
    }
    let mut copied = 0usize;
    walk(
        out,
        root,
        out,
        &mut std::collections::HashSet::new(),
        &mut std::collections::HashSet::new(),
        shipped,
        &mut copied,
    );
    copied
}

/// Whether two paths resolve to the same file on disk (so we don't self-copy).
fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

/// Bodies of the `<script type="application/tali-js">…</script>` cells in `html` (the
/// author's `{js}` source, where relative `import()`/`fetch()` specifiers live —
/// invisible to the `src=`/`href=` scan). Read through [`taliesin_core::render::tags`], so a
/// cell is a `<script>` tag whose `type` says so, never the text `type="application/tali-js"`
/// wherever a page happens to show it (a code sample, an attribute value), which the
/// substring scan this replaced took for a cell and read to its next `</script>`.
/// `</script` is server-escaped in the source, so the next `</script` reliably ends the body.
fn tali_js_cell_sources(html: &str) -> Vec<&str> {
    taliesin_core::render::tags(html)
        .filter(|t| t.name.eq_ignore_ascii_case("script"))
        .filter(|t| {
            taliesin_core::render::attrs(t)
                .any(|a| a.name.eq_ignore_ascii_case("type") && a.value == "application/tali-js")
        })
        .map(|t| {
            let start = t.at + t.text.len();
            let body = &html[start..];
            let end = body
                .match_indices("</")
                .map(|(i, _)| i)
                .find(|&i| {
                    body.as_bytes()
                        .get(i + 2..i + 8)
                        .is_some_and(|b| b.eq_ignore_ascii_case(b"script"))
                })
                .unwrap_or(body.len());
            &body[..end]
        })
        .collect()
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
/// are ignored; tree-escaping ones warn. Returns the count copied; a different file already
/// at a destination is refused into `problems` exactly as [`copy_local_assets`] refuses one.
fn copy_js_imports(
    html: &str,
    base: &Path,
    dest: &Path,
    problems: &mut Vec<taliesin_core::render::Warning>,
) -> usize {
    let mut copied = 0usize;
    let mut visited = std::collections::HashSet::new();
    let mut queue: Vec<String> = Vec::new();
    // An import that climbs out of the folder cannot be copied into it, so the cell fails
    // to load it: an error `--strict` counts, like every file the copier cannot place. It
    // was an uncounted notice (audit 2026-09-24, WP1 residual). `at` locates it at the cell
    // that imports it; a module's own import has no line in this page.
    let enqueue = |queue: &mut Vec<String>,
                   problems: &mut Vec<taliesin_core::render::Warning>,
                   dir: &str,
                   spec: &str,
                   at: Option<usize>| {
        match normalize_rel(dir, spec) {
            Some(rel) => queue.push(rel),
            None => {
                let mut w = taliesin_core::render::Warning::new(format!(
                    "{{js}} import not bundled: `{spec}` climbs out of the document's folder, \
                     so the output has no copy of it and the cell cannot load it"
                ))
                .severity(taliesin_core::Severity::Error);
                if let Some(at) = at {
                    w.file = source_file_before(html, at);
                    w.line = sourcepos_line_before(html, at);
                }
                problems.push(w);
            }
        }
    };
    for body in tali_js_cell_sources(html) {
        let at = body.as_ptr() as usize - html.as_ptr() as usize;
        for spec in relative_specifiers(body) {
            enqueue(&mut queue, problems, "", &spec, Some(at));
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
            match bundle_file(&from, &to, &rel) {
                Ok(true) => copied += 1,
                Ok(false) => continue,
                Err(w) => {
                    problems.push(w);
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
                enqueue(&mut queue, problems, dir, &spec, None);
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
    /// Log lines, in the exact order the sequential build emitted them (cell errors
    /// first, then render/cross-ref warnings), replayed by the caller in page order.
    ///
    /// Each carries the severity its validator set, because the caller has to print it at
    /// that severity. Flattening these to plain strings and replaying them all through
    /// `log::warn` is what let a site build report an `error` as `warn` — and once an
    /// error-severity diagnostic fails the build, a failure whose every printed line said
    /// `warn` would name nothing the author could act on.
    warnings: Vec<(taliesin_core::Severity, String)>,
    /// The same findings, structured, for `--format json` — in the same order as `warnings`.
    diagnostics: Vec<crate::lint::Diagnostic>,
    problems: usize,
    /// The `error`-severity subset of `problems`, folded across pages into the build's
    /// unconditional failure. A crashed cell is in `problems` but never here.
    unparseable: usize,
    /// Set when this page has executable cells whose kernel could not start: the full
    /// "here is everything I searched" report. Folded across the build into one fatal
    /// error (the interpreter cannot differ between pages of one build, so the first is
    /// the whole story).
    kernel_failure: Option<String>,
    written: bool,
    /// This page could not be read from, or could not be written to. Folded across the
    /// build into an unconditional failure, `--strict` or not: a page the build could not
    /// write is not a built page, and the sweep keeps the stale output that is standing in
    /// for it, so exit 0 tells CI the deploy is current when it is a rebuild-old copy. The
    /// single-doc path has always returned `FAILURE` here (`finalize_build`'s `wrote`); the
    /// site path counted it into neither `problems` nor `unparseable` and exited 0.
    io_failed: bool,
    /// The conditional `_assets/` blobs this page's HTML links, folded across the build so
    /// only the linked ones are written (item 137).
    used: AssetUse,
}

/// Build one page: [`crate::lint::PagePass`] on a *fresh, page-private* executor (own
/// kernel + own `_freeze/<rel>.json` under the project root, which the preview shares,
/// cwd = the page's own dir), then the chrome-wrapped HTML, written. Pure w.r.t. shared
/// state: the only writes are to this page's own output file + freeze file, so it is safe
/// to run many of these at once. All logging is deferred into the returned
/// [`PageOutcome`].
async fn build_one_page(
    site: &taliesin_core::Site,
    page: &taliesin_core::site::Page,
    out: &Path,
    bundle: &AssetBundle,
) -> PageOutcome {
    let mut warnings = Vec::new();
    let mut diagnostics: Vec<crate::lint::Diagnostic> = Vec::new();
    let Ok(src) = taliesin_core::includes::read_source(&page.input) else {
        let msg = format!("cannot read {}", page.input.display());
        diagnostics.push(crate::lint::Diagnostic::new(
            page.rel.clone(),
            None,
            msg.clone(),
        ));
        warnings.push((taliesin_core::Severity::Error, msg));
        return PageOutcome {
            warnings,
            diagnostics,
            problems: 0,
            unparseable: 0,
            kernel_failure: None,
            written: false,
            io_failed: true,
            used: AssetUse::default(),
        };
    };
    let mut exec = page_executor(site, page);
    // No progress sink (a build has no client), but name the page: a cold site build runs
    // pages concurrently, so bare interleaved `cell 2/4` lines belong to nobody.
    exec.set_progress(None, Some(page.rel.clone()));
    // THE page pass, as every verb runs it, deferred: nothing is printed here, and the
    // caller replays each page's lines in page order.
    let mut pass = crate::lint::PagePass::run(site, page, src, &page.rel, Some(&mut exec)).await;
    for d in &pass.diags {
        warnings.push((d.severity, d.located()));
    }
    // A crashed cell bakes its traceback into the page: its own sentence names it.
    let cells = cell_error_diagnostics(&pass.failures, &page.rel);
    for d in &cells {
        warnings.push((d.severity, d.message.clone()));
    }
    diagnostics.append(&mut pass.diags);
    diagnostics.extend(cells);
    // Every page links the shared `_assets/` bundle instead of inlining its own copy of the
    // framework CSS/JS; hrefs are depth-adjusted so a nested page's `../` prefix count
    // matches.
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
    let html = site.page_html_external(page, &pass.doc, ext);
    // Offline-guarantee, per page: flag any external reference this page keeps, exactly like the
    // single-doc build, so the common multi-page deploy (`build <dir>`) is covered too.
    // Informational — deferred into the page's warnings and carried into the structured
    // channel (`--format json`), never counted in `problems`/`--strict`.
    for w in &offline_ref_warnings(&html) {
        warnings.push((w.severity, locate(w, &page.rel)));
        diagnostics.push(crate::lint::diag_from(w, &page.rel));
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
            warnings.push((taliesin_core::Severity::Error, msg));
            false
        }
    };
    PageOutcome {
        warnings,
        diagnostics,
        problems: pass.problems,
        unparseable: pass.unparseable,
        kernel_failure: pass.kernel_failure,
        io_failed: !written,
        written,
        used,
    }
}

/// The result of a directory (site/book) build: whether it succeeded, and the structured
/// diagnostics it produced (for `--format json`), in deterministic page order.
struct SiteBuildOutcome {
    ok: bool,
    diagnostics: Vec<crate::lint::Diagnostic>,
}

/// The resolved shared-asset filenames (content-hashed), computed once per site build.
struct AssetBundle {
    app_css: String,
    katex_css: String,
    /// The rewritten math sheet `write_conditional` writes when something links it (T6),
    /// computed once beside its hash so the bytes and the name can never disagree.
    katex_css_text: String,
    /// The KaTeX faces as `(hashed filename, bytes)`: named up front (the sheet above
    /// references them as siblings), written only with the sheet (item 137).
    katex_fonts: Vec<(String, &'static [u8])>,
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
            put(&self.katex_css, &self.katex_css_text)?;
            // The faces the sheet references as siblings (T6). Skipping one the sheet
            // names would be the same live 404 as skipping the sheet itself.
            for (name, bytes) in &self.katex_fonts {
                std::fs::write(dir.join(name), bytes)?;
            }
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
        //
        // Matched by exact name, not `contains("normal")`: the mono's regular weight also
        // contains "normal", so that scan matched either FONT_FILES entry depending on
        // iteration order and could silently preload the wrong face.
        if *src_name == taliesin_core::FONT_PRELOAD_NAME {
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
    // The KaTeX faces (T6, the math sibling of item 150): NAMED here because the math
    // sheet references them by hashed sibling name, but not written; they land beside
    // katex.css in `write_conditional`, so a prose-only project still ships no math
    // bytes at all (item 137).
    let mut katex_font_hrefs: Vec<(&str, String)> = Vec::new();
    let mut katex_fonts: Vec<(String, &'static [u8])> = Vec::new();
    for (src_name, bytes) in taliesin_core::KATEX_FONT_FILES {
        let name = format!(
            "{}.{:x}.woff2",
            src_name.strip_suffix(".woff2").unwrap_or(src_name),
            fnv1a_bytes(bytes)
        );
        katex_font_hrefs.push((src_name, name.clone()));
        katex_fonts.push((name, *bytes));
    }
    // Named, not written: see `write_conditional`.
    let katex_css_text =
        taliesin_core::minify_css(&taliesin_core::katex_css_linked_fonts(&katex_font_hrefs));
    let katex_css = hashed("katex", "css", &katex_css_text);
    let mermaid_js = hashed("mermaid", "js", &taliesin_core::mermaid_bundle_js());
    let jslibs_js = hashed("jslibs", "js", &taliesin_core::js_cell_libs_js());
    Ok(AssetBundle {
        app_css,
        katex_css,
        katex_css_text,
        katex_fonts,
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

/// `build <dir>`: the site build ([`build_site_async`]) on a tokio runtime, its diagnostics
/// printed as JSON under `--format json`. `cmd_build` has already refused a directory
/// with no `_site.yml`, ahead of `--stdout` (`project_required.rs`).
fn build_site(
    root: &Path,
    out_override: Option<&str>,
    strict: bool,
    jobs: Option<usize>,
    json: bool,
) -> ExitCode {
    // Executing code cells needs the async kernel, so the whole site build runs on a
    // tokio runtime (mirrors the preview server's setup). A multi-thread runtime so
    // concurrent page builds (each its own kernel) actually overlap on the CPU.
    let outcome = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt.block_on(build_site_async(root, out_override, strict, jobs)),
        Err(e) => {
            let msg = format!("cannot start runtime: {e}");
            log::error(&msg);
            SiteBuildOutcome {
                ok: false,
                diagnostics: vec![crate::lint::Diagnostic::new(
                    root.display().to_string(),
                    None,
                    msg,
                )],
            }
        }
    };
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
/// One project per call, and since 2026-08-16 that is also one project per DEPLOY: this
/// repo's four sites (marketing, the two docs books, the gallery) each publish to their own
/// Cloudflare Pages project and reach each other by absolute URL, because Pages has no
/// subpath deploy and a composed tree would have to be re-uploaded whole on every change.
///
/// Until 2026-08-19 the gallery was the one project that wrote others under its own output
/// (its three exhibits), one `build … --out <out>/<prefix>` per exhibit, parent first,
/// because the parent's sweep deletes what it did not write, and `tools/publish.sh` did
/// that composing. That is gone: the gallery is now a flat, self-contained project of
/// one-page demos, and nothing this repo publishes composes another project into its own
/// output any more. Nested builds (`build <dir> --out <parent-out>/<prefix>`) remain a
/// plain capability of this function for any future consumer; no deploy exercises it
/// today. The reason `.githooks/pre-push` still runs `tools/publish.sh --check` as a gate
/// is item 149: a deploy whose call-to-action once 404'd from a script nobody ran.
async fn build_site_async(
    root: &Path,
    out_override: Option<&str>,
    strict: bool,
    jobs: Option<usize>,
) -> SiteBuildOutcome {
    let started = std::time::Instant::now();
    let site = taliesin_core::Site::discover(root);
    // Structured diagnostics accumulated in deterministic order (config → pages → site-wide),
    // for `--format json`. Mirrors the human log the build already emits.
    let mut diagnostics: Vec<crate::lint::Diagnostic> = Vec::new();
    // The project's own diagnostics (`_site.yml`, a page's front matter as discovery reads
    // it), reported and counted exactly as a page's are, so `--strict` fails on what
    // `--check-only` fails on. They were logged as advice and counted not at all, and a
    // `_site.yml` that dropped a book part built green under `--strict` (audit 2026-09-24
    // NEW-B). Located relative to the site root, like every other line this build prints.
    let config_problems = crate::lint::blocking(&site.warnings);
    // A malformed `_site.yml` is also unparseable: with the config unread the site has no
    // title, no nav and no `url:`, so the feed/sitemap surface silently vanishes. That
    // fails the build with no `--strict`, like an unparseable front matter.
    let config_errors = site
        .warnings
        .iter()
        .filter(|w| taliesin_core::site::is_malformed_config_warning(w))
        .count();
    for w in &site.warnings {
        log_located(w, "_site.yml");
        diagnostics.push(crate::lint::diag_from(w, "_site.yml"));
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

    // Render each page with chrome + rewritten links. Code cells run against a
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
    let mut unparseable = config_errors;
    // Pages this build could not read or could not write. Never `--strict`-conditional.
    let mut io_failures = 0usize;

    // Build into a slot per page (indexed by page order) so results aggregate
    // deterministically regardless of completion order. A `Semaphore` of size
    // `build_cap` bounds how many build kernels run at once (memory-aware); the file
    // write each page does is on its own paths, so no lock is held across the `.await`.
    let site = std::sync::Arc::new(site);
    let out = std::sync::Arc::new(out);
    let bundle = std::sync::Arc::new(bundle);
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(build_cap));
    let mut set: tokio::task::JoinSet<(usize, PageOutcome)> = tokio::task::JoinSet::new();
    for (idx, _page) in site.pages.iter().enumerate() {
        let site = site.clone();
        let out = out.clone();
        let bundle = bundle.clone();
        let sem = sem.clone();
        set.spawn(async move {
            // Hold a permit only for this page's build; dropping it on return frees the
            // slot for the next queued page. The permit guards kernel count, not any
            // shared data structure, so nothing is locked across the build's `.await`.
            let _permit = sem.acquire().await.expect("build semaphore not closed");
            let page = &site.pages[idx];
            let outcome = build_one_page(&site, page, &out, &bundle).await;
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
        for (sev, w) in &outcome.warnings {
            match sev {
                taliesin_core::Severity::Error => log::error(w),
                _ => log::warn(w),
            }
        }
        diagnostics.extend(outcome.diagnostics);
        problems += outcome.problems;
        unparseable += outcome.unparseable;
        io_failures += usize::from(outcome.io_failed);
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
    // Mirror non-source assets (images, etc.) preserving the tree. AFTER the pages are
    // built, not before: a figure a cell writes to disk while its page builds
    // (`savefig("gen.png")` then `![…](gen.png)`) is then published by the build that
    // wrote it, where mirroring first left the first build of every fresh clone without it
    // (audit images #12). A folder whose pages are all drafts is the drafts' own: its
    // figures and data are as unpublished as its text. A folder that also holds (or sits
    // above) a published page is not held back, and the site root never is.
    let held_back: Vec<PathBuf> = site
        .excluded_drafts
        .iter()
        .filter_map(|d| Path::new(d).parent())
        .filter(|d| !d.as_os_str().is_empty())
        .filter(|d| !site.pages.iter().any(|p| Path::new(&p.rel).starts_with(d)))
        .map(Path::to_path_buf)
        .collect();
    let (asset_paths, skipped_residue) = mirror_assets(root, &out, &held_back);
    if !skipped_residue.is_empty() {
        log::warn(&format!(
            "skipped {} build-cache dir(s) (not deployed): {}",
            skipped_residue.len(),
            skipped_residue.join(", ")
        ));
    }
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
        log_located(&w, &rel);
        diagnostics.push(crate::lint::diag_from(&w, &rel));
    }
    // The `_site.yml` chrome's own hrefs, which no page body carries and so no page-body
    // harvest could ever see. Same registry, same rules; site-wide blast radius.
    for w in site.validate_chrome_links() {
        problems += 1;
        log_located(&w, "_site.yml");
        diagnostics.push(crate::lint::diag_from(&w, "_site.yml"));
    }
    for w in site.validate_shared_bibliography() {
        problems += 1;
        log_located(&w, "_site.yml");
        diagnostics.push(crate::lint::diag_from(&w, "_site.yml"));
    }

    // Second asset pass: ship the files pages actually reference that mirror_assets left
    // out (a source extension, an `_`-prefixed folder). A reference is intentional, so
    // skipping it would leave a dead link or a broken image on a green build.
    let mut assets = asset_paths.len() + deploy_referenced_sources_for_site(root, &out, &mut keep);
    // A page's front-matter `image:` is a reference no page body need carry: it is the
    // `og:image` a shared link unfurls with, stored site-root-relative by discovery (an
    // external URL is no local ref and ships nothing).
    for img in site.pages.iter().filter_map(|p| p.card_image.as_deref()) {
        if taliesin_core::diagnostics::is_local_ref(img) {
            assets += usize::from(ship_referenced(img, root, Path::new(""), &out, &mut keep));
        }
    }

    let built = format!(
        "{}  ·  {pages} page{}  ·  {assets} asset{}{search}{not_found}{seo_note}{}",
        out.display(),
        if pages == 1 { "" } else { "s" },
        if assets == 1 { "" } else { "s" },
        elapsed_note(started),
    );
    // In `--strict` mode a problem (crashed cell / located warning / broken ref)
    // fails the build after writing it, so CI catches a broken site. Without `--strict`
    // the site still ships, but a closing tally (DX12) makes the shipped problems visible
    // rather than a wordless green exit after pages of scrolled-past warnings.
    // A site with executable cells and no usable kernel fails outright, ahead of the
    // `--strict` tally and regardless of it: every cell stripped back to source is not a
    // successful build of a book whose value is its executed output. `--no-exec` is the
    // way to ask for source-only rendering on purpose.
    let kernel_fail = report_kernel_failure(kernel_failure.as_deref());
    // An `error`-severity diagnostic fails the site build with no `--strict`, on the same
    // rule the single-doc path uses: a malformed `_site.yml` drops the title, the nav and
    // the `url:` that gates the whole feed/sitemap surface, and a page whose front matter
    // did not parse lost its own `title:`/`listing:`. Both used to ship green.
    let unparseable = unparseable;
    let strict_fail = strict && problems > 0;
    // A page that could not be read or written fails the build outright, `--strict` or
    // not, matching what the single-doc path has always done. The per-page `error` line is
    // already printed; this is the closing verdict, because `built … N pages` above it
    // otherwise reads as success, and the sweep keeps the failed page's URL, so what is
    // still sitting in the output is the previous build's body.
    let io_fail = io_failures > 0;
    if kernel_fail {
        // The kernel error is the actionable one; don't bury it under a second tally.
    } else if io_fail {
        warn_io_failures(io_failures);
    } else if unparseable > 0 {
        warn_unparseable(unparseable);
    } else if strict_fail {
        warn_strict(problems);
    } else {
        // Only a build that succeeded says `built` (first-hour #12).
        log::built(&built);
        warn_nonstrict_problems(problems);
    }
    SiteBuildOutcome {
        ok: unparseable == 0 && !strict_fail && !kernel_fail && !io_fail,
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
/// the `_`/`.`-prefix convention still applies; these are excluded by kind.) `.orig` and
/// `.rej` are a merge's and a patch's leftovers, each a copy of a source file (see
/// [`is_editor_residue`]).
const SKIP_EXT: &[&str] = &["tmd", "bib", "Rproj", "md", "scss", "sass", "orig", "rej"];

/// An editor's backup (`index.tmd~`) or autosave (`#index.tmd#`): a full copy of the page's
/// source under a name [`SKIP_EXT`] does not recognise, so it published the source the
/// extension rule exists to keep out (and a draft's, whose page is held back).
fn is_editor_residue(name: &str) -> bool {
    name.ends_with('~') || (name.len() > 1 && name.starts_with('#') && name.ends_with('#'))
}

/// Copy every non-source file under `root` into `out`, mirroring the directory tree.
/// Skips: source-only extensions ([`SKIP_EXT`]: `.tmd`/`.bib`/`.Rproj`/`.md`/`.scss`/
/// `.sass`, and merge residue), editor residue ([`is_editor_residue`]), `_`-prefixed and
/// dot entries (`_site.yml`, `_includes`, `_site`, `.RData`, …), build-tool cache/artifact
/// dirs (`*_cache/`, `*_files/`, knitr/RMarkdown residue), the output dir itself, and the
/// `held_back` folders (root-relative): a folder whose only pages are drafts, whose figures
/// and data are as unpublished as its text. A file a published page REFERENCES from any of
/// these still ships, through [`deploy_referenced_sources`].
/// Returns `(out-relative paths copied, names of skipped cache dirs)` so the caller can
/// report residue it dropped rather than silently omitting it, and knows which output
/// files this build owns (for the stale-file sweep).
///
/// Every entry goes through the one publication rule, [`taliesin_core::includes::publishable`],
/// as [`Reach::Wholesale`](taliesin_core::includes::Reach): that is where the `_`/`.`
/// convention and the repository boundary live. It is applied to what an entry REACHES,
/// not to its name alone. A symlink is followed only while its real path stays inside the
/// repository and adds no `.`/`_` component to the path it shares with the project: a link
/// to a sibling directory of the same checkout is first-party authoring, while `vendor ->
/// ../.git` (an ordinary name, referenced by no page) published `.git/config` and every
/// object until the 2026-09-24 audit, because only the link's own name was tested.
fn mirror_assets(root: &Path, out: &Path, held_back: &[PathBuf]) -> (Vec<PathBuf>, Vec<String>) {
    #[allow(clippy::too_many_arguments)]
    fn walk(
        dir: &Path,
        root: &Path,
        out: &Path,
        held_back: &[PathBuf],
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
            let Ok(rel) = p.strip_prefix(root) else {
                continue;
            };
            if taliesin_core::includes::publishable(
                root,
                root,
                rel,
                taliesin_core::includes::Reach::Wholesale,
            )
            .is_err()
                || held_back.iter().any(|d| rel == d)
                || is_editor_residue(name)
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
                walk(&p, root, out, held_back, seen, copied, skipped);
            } else if !SKIP_EXT.contains(&p.extension().and_then(|s| s.to_str()).unwrap_or("")) {
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
        held_back,
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
/// one into a nested deploy and [`sweep_stale`] never deletes it, which is what lets a
/// nested `build <dir> --out <parent-out>/<prefix>` survive the parent's own sweep.
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
///
/// **The fallback matches the emitted shape exactly, hash segment included.** It was
/// `starts_with("app.") && ends_with(".css")` until 2026-08-17, which also claims
/// `app.min.css` and every webpack/parcel `app.<contenthash>.css`, conventional names in
/// a stranger's `dist/`. Claiming one skipped the refusal and let [`sweep_stale`] delete
/// their files with an exit-0 build: the exact disaster the refusal was installed to
/// prevent. A fallback that is looser than what it recognises is not evidence.
fn is_taliesin_output(out: &Path) -> bool {
    if out.join(OUTPUT_MARKER).is_file() {
        return true;
    }
    std::fs::read_dir(out.join("_assets")).is_ok_and(|mut entries| {
        entries.any(|e| {
            e.is_ok_and(|e| {
                let name = e.file_name();
                is_hashed_asset_name(&name.to_string_lossy(), "app", "css")
            })
        })
    })
}

/// Is `name` the `<stem>.<hash>.<ext>` shape [`write_asset_bundle`] emits? The hash is a
/// `u64` written `{:x}`, so it is 1-16 lowercase hex digits and never empty.
fn is_hashed_asset_name(name: &str, stem: &str, ext: &str) -> bool {
    let Some(rest) = name.strip_prefix(stem).and_then(|r| r.strip_prefix('.')) else {
        return false;
    };
    let Some(hash) = rest.strip_suffix(ext).and_then(|h| h.strip_suffix('.')) else {
        return false;
    };
    (1..=16).contains(&hash.len()) && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
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

/// Unique local URLs in `html`'s `src=`/`href=`/`poster=`/`srcset=` attributes
/// ([`taliesin_core::render::URL_ATTRS`]; skips external URLs, protocol-relative refs, data
/// URIs, in-page anchors, and other schemes).
///
/// `poster=` is a media attribute the first two never carry: harvesting only `src`/`href`
/// built a folder whose `<video>` still 404s. It stays because raw `<video>` HTML is in the
/// trust model. `srcset=` (each candidate) joined it on 2026-09-24: a folder built without
/// it 404'd the 2x image on every high-density screen and the dark `<picture><source>`.
///
/// `data-src=` was harvested here too until 2026-08-09, for a theme-adaptive `dark=` pair
/// that shipped both clips as `data-src` so the hidden one was never fetched. Wave 7 cut
/// `{{< video >}}`, which took the page-shell promoter that turned `data-src` into `src`
/// with it — so nothing emits the attribute and nothing would load a file harvested from
/// it. Its comment cited `corpus/media/screencast.tmd`, which does not exist either.
///
/// Reading whole attributes off [`taliesin_core::render::tags`] is what keeps the
/// click-to-source `data-tali-src="…"` — which *contains* `src="` — from publishing every
/// post's own source, and it is now also what keeps a code sample and an inlined script
/// from doing the same (Fable audit FA13).
///
/// Each value comes with the byte offset of its first occurrence, which is what locates a
/// refused reference at the block that carries it ([`sourcepos_line_before`]).
fn local_refs(html: &str) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for tag in taliesin_core::render::tags(html) {
        for a in taliesin_core::render::attrs(&tag) {
            // The one list of URL attributes (`render::URL_ATTRS`) and the one reading of a
            // `srcset`, shared with the gate and the 404 rewrite.
            for v in taliesin_core::render::attr_urls(a.name, &a.value) {
                if taliesin_core::diagnostics::is_local_ref(v)
                    && !out.iter().any(|(seen, _)| seen == v)
                {
                    out.push((v.to_string(), a.at));
                }
            }
        }
    }
    out
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
    /// The included file the enclosing block came from, when it came from one. `None` means
    /// the primary document, whose name the caller already has.
    file: Option<String>,
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

/// The included file the block enclosing byte `offset` came from, read from the
/// `data-source-file="…"` on the same tag as the `data-sourcepos` [`sourcepos_line_before`]
/// used. `None` for a block of the primary document.
///
/// **The two must be read from the same tag** (Fable audit FA14). The line already came
/// from the block, while the file was the *page's* name, so an offline-ref warning inside an
/// `{{< include >}}`d partial pointed the author at the parent document at the partial's
/// line number: a real file, an openable line, and nothing there. This is the same
/// file-and-line pairing rule CLAUDE.md states for the renderer's own warnings.
fn source_file_before(html: &str, offset: usize) -> Option<String> {
    const KEY: &str = "data-sourcepos=\"";
    let at = html[..offset].rfind(KEY)?;
    // Bounded to the one tag: from its `<` to its `>`, so the next block's attribute is
    // never picked up for this one.
    let lt = html[..at].rfind('<')?;
    let gt = taliesin_core::render::tag_end(&html[lt..]).map_or(html.len(), |i| lt + i);
    const FILE: &str = "data-source-file=\"";
    let from = html[lt..gt].find(FILE)? + lt + FILE.len();
    let len = html[from..gt].find('"')?;
    Some(html[from..from + len].to_string())
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

/// The `<link rel=>` values whose `href` the browser really fetches while rendering the page,
/// which is the only kind that can make a `--out` build non-self-contained. Everything else a
/// `<link>` can say — `alternate` (the tool's own Atom autodiscovery), `canonical`, `me`,
/// `author`, `license`, `next`/`prev` — is metadata the browser never requests, so flagging it
/// tells the author about a fetch that does not happen, in a file that does not contain it.
/// `preconnect`/`dns-prefetch` are deliberately absent: they open a connection, fetch no
/// resource, and cost an offline reader nothing.
const FETCHING_LINK_RELS: &[&str] = &[
    "stylesheet",
    "preload",
    "modulepreload",
    "prefetch",
    "prerender",
    "icon",
    "apple-touch-icon",
    "mask-icon",
    "manifest",
];

/// Whether `tag` is a `<link>` the browser fetches, per [`FETCHING_LINK_RELS`]. Reads the
/// whole tag rather than the text before `href`, because `rel=` may come on either side of
/// it. A `<link>` with no `rel` at all does nothing, so it is not a fetch either.
fn link_rel_fetches(tag: &taliesin_core::render::Tag<'_>) -> bool {
    taliesin_core::render::attrs(tag)
        .filter(|a| a.name.eq_ignore_ascii_case("rel"))
        .any(|a| {
            a.value
                .split_ascii_whitespace()
                .any(|t| FETCHING_LINK_RELS.iter().any(|r| t.eq_ignore_ascii_case(r)))
        })
}

/// Every external (network-fetched-at-view-time) reference left verbatim in built `html`: a
/// resource `src=` (img/script/iframe/audio/video/…), a `<link href=>` stylesheet/preload, and
/// a remote or bare `{js}` `import()` specifier. These keep a `--out` build from being
/// self-contained (offline viewing fails). NOT flagged: `<a href>` hyperlinks (navigation, not
/// a fetch), `data:` URIs, local/relative paths, or the tool's own inlined assets (the import
/// scan reads only author `{js}` cell bodies). Each ref carries its enclosing block's line.
fn external_refs(html: &str) -> Vec<ExternalRef> {
    let mut out: Vec<ExternalRef> = Vec::new();
    let push = |url: &str, off: usize, out: &mut Vec<ExternalRef>| {
        let line = sourcepos_line_before(html, off);
        let file = source_file_before(html, off);
        if !out
            .iter()
            .any(|r| r.url == url && r.line == line && r.file == file)
        {
            out.push(ExternalRef {
                url: url.to_string(),
                line,
                file,
            });
        }
    };
    // (1) resource `src=` (always a fetch) and `<link href=>` (a stylesheet/preload fetch).
    for tag in taliesin_core::render::tags(html) {
        for a in taliesin_core::render::attrs(&tag) {
            let is_src = a.name.eq_ignore_ascii_case("src");
            if !is_src && !a.name.eq_ignore_ascii_case("href") {
                continue;
            }
            if !is_external_fetch(&a.value) {
                continue;
            }
            // `href=` fetches only on a `<link>` — an `<a>`/`<area>`/`<base>` href is not one —
            // and only for a `rel` that really fetches (the tool's own `rel="alternate"` feed
            // autodiscovery is the one every page of a published site carries).
            if !is_src && !(tag.name.eq_ignore_ascii_case("link") && link_rel_fetches(&tag)) {
                continue;
            }
            push(&a.value, a.at, &mut out);
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
/// tool deliberately does not download arbitrary URLs at build time — both call sites keep
/// these out of their `problems` count, and `docs/guide/reference/cli.tmd` documents the
/// carve-out. They DO ride the structured channel (`diag_from` → `--format json`), so a
/// machine consumer sees what the console prints. Empty for an all-local page. Shared by the
/// single-doc build (logged immediately) and the site build (collected into the page's
/// deferred warning list), so both deploy shapes are covered.
fn offline_ref_warnings(html: &str) -> Vec<taliesin_core::render::Warning> {
    external_refs(html)
        .into_iter()
        .map(|r| {
            let mut w = taliesin_core::render::Warning::new(format!(
                "external reference not bundled: {} — the build will fetch it at view time, \
                 so the output is not self-contained (offline viewing fails)",
                r.url
            ));
            // The file and the line must come from the same block, or the warning names a
            // real file at a line that belongs to another one (FA14). `file: None` means
            // the document being built, whose name the caller supplies at print time.
            w.file = r.file;
            w.line = r.line;
            w
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

    /// A `<link>` `href` is only a view-time fetch for the rel values that make it one.
    /// The tool emits its OWN absolute `<link rel="alternate" type="application/atom+xml">`
    /// on every page of a site with `url:` set (`site::meta::feed_head`), and until
    /// 2026-08-13 every one of them drew "the build will fetch it at view time, so the
    /// output is not self-contained" — false (a browser does not fetch a feed
    /// autodiscovery link), unfixable by the author, and located to source files
    /// containing nothing of the kind. Measured on `corpus/tech-blog`: 34 of the build's
    /// 50 warn lines, burying the 16 real ones 2:1, and only once `url:` is set — the step
    /// an author takes to publish.
    #[test]
    fn external_refs_reads_a_link_rel_and_skips_the_ones_that_never_fetch() {
        let html = concat!(
            // The tool's own feed autodiscovery, exactly as `feed_head` emits it.
            "<link rel=\"alternate\" type=\"application/atom+xml\" title=\"Blog\" ",
            "href=\"https://example.com/blog.xml\">",
            // Author-written head metadata that is also not a fetch.
            "<link rel=\"canonical\" href=\"https://example.com/post.html\">",
            "<link rel=\"me\" href=\"https://mastodon.test/@a\">",
            // …and the ones that really are.
            "<link rel=\"stylesheet\" href=\"https://cdn.test/x.css\">",
            "<link rel=\"shortcut icon\" href=\"https://cdn.test/f.ico\">",
            "<link rel=\"preload\" as=\"font\" href=\"https://cdn.test/f.woff2\" crossorigin>",
        );
        let urls: Vec<String> = external_refs(html).into_iter().map(|r| r.url).collect();
        for skipped in [
            "https://example.com/blog.xml",
            "https://example.com/post.html",
            "https://mastodon.test/@a",
        ] {
            assert!(
                !urls.iter().any(|u| u == skipped),
                "`{skipped}` is not fetched at view time, so warning about it is a lie: {urls:?}"
            );
        }
        for flagged in [
            "https://cdn.test/x.css",
            "https://cdn.test/f.ico",
            "https://cdn.test/f.woff2",
        ] {
            assert!(
                urls.iter().any(|u| u == flagged),
                "`{flagged}` IS fetched at view time and must still be flagged: {urls:?}"
            );
        }
    }

    /// An offline-ref warning inside an included partial names the PARTIAL, at the
    /// partial's line.
    ///
    /// **The defect (Fable audit FA14).** The line came from the block's `data-sourcepos`
    /// (the included file's own numbering) while the file was always the page being built,
    /// so the author was pointed at a real, openable file at a line that belongs to a
    /// different one. The include source map had the right answer on the same tag all along:
    /// `data-source-file`.
    #[test]
    fn an_offline_ref_inside_an_include_names_the_included_file() {
        let html = concat!(
            "<p data-block-id=\"b-1\" data-sourcepos=\"3:1-3:40\">",
            "<img src=\"https://example.com/own.png\"></p>",
            "<p data-block-id=\"b-2\" data-sourcepos=\"7:1-7:40\" data-source-file=\"_includes/part.tmd\">",
            "<img src=\"https://example.com/partial.png\"></p>",
        );
        let warnings: Vec<String> = offline_ref_warnings(html)
            .iter()
            .map(|w| locate(w, "index.tmd"))
            .collect();
        assert!(
            warnings.iter().any(|w| w.starts_with("index.tmd:3:")),
            "the page's own block keeps the page's name: {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.starts_with("_includes/part.tmd:7:")),
            "an included block must name the file its line belongs to: {warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| w.starts_with("index.tmd:7:")),
            "the parent file paired with the partial's line points at nothing: {warnings:?}"
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

    /// A `{js}` cell body is a `<script type="application/tali-js">` the walker reads as a
    /// tag, not the text `type="application/tali-js"` wherever it appears: an attribute
    /// value quoting it is not a cell, and what follows it is not that cell's source.
    #[test]
    fn a_js_cell_is_a_script_tag_not_a_string_that_names_one() {
        let html = concat!(
            "<p title='type=\"application/tali-js\">'>import(\"./not-a-cell.js\")</p>",
            "<script type=\"application/tali-js\">const m = await import(\"./real.js\");</script>",
        );
        assert_eq!(
            tali_js_cell_sources(html),
            ["const m = await import(\"./real.js\");"]
        );
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

    /// A hand-written raw-HTML tag may quote its attributes either way, and the offline
    /// warning is the author's only signal that a "portable" `--out` folder still fetches at
    /// view time. Reading `src="` alone meant a single-quoted remote resource was copied by
    /// nothing and warned about by nothing: the folder shipped broken, silently.
    #[test]
    fn external_refs_sees_a_single_quoted_remote_resource() {
        let html = "<p data-sourcepos=\"6:1-6:40\"><img src='https://cdn.test/pic.png'></p>";
        let urls: Vec<String> = external_refs(html).into_iter().map(|r| r.url).collect();
        assert_eq!(urls, vec!["https://cdn.test/pic.png".to_string()]);
        let w: Vec<String> = offline_ref_warnings(html)
            .iter()
            .map(|w| locate(w, "posts/p.tmd"))
            .collect();
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].starts_with("posts/p.tmd:6:"), "located: {}", w[0]);
    }

    #[test]
    fn offline_ref_warnings_locate_the_source_and_stay_empty_for_local() {
        // The shared helper the single-doc AND site-build paths both emit through: located
        // `label:line:` prefix + the url, and silent for an all-local page.
        let html = "<p data-sourcepos=\"4:1-4:9\"><img src=\"https://x.test/y.png\"></p>";
        let refs = offline_ref_warnings(html);
        assert!(
            refs.iter()
                .all(|w| w.severity == taliesin_core::Severity::Warning),
            "informational severity — the --strict carve-out: {refs:?}"
        );
        let w: Vec<String> = refs.iter().map(|w| locate(w, "posts/p.tmd")).collect();
        assert_eq!(w.len(), 1);
        assert!(w[0].starts_with("posts/p.tmd:4:"), "located: {}", w[0]);
        assert!(
            w[0].contains("https://x.test/y.png"),
            "names the url: {}",
            w[0]
        );
        assert!(
            offline_ref_warnings("<p data-sourcepos=\"1:1\"><img src=\"a.png\"></p>").is_empty()
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

    /// The harvested values alone, for the tests that are about WHICH values are read.
    fn local_urls(html: &str) -> Vec<String> {
        local_refs(html).into_iter().map(|(v, _)| v).collect()
    }

    #[test]
    fn local_refs_matches_whole_attributes_not_substrings() {
        // `data-tali-src="…"` (the click-to-source attribute on listing cards) *contains*
        // the substring `src="`, so a bare search harvested each post's `.tmd` and
        // `deploy_referenced_sources` published the sources into `_site/`.
        let refs = local_urls(
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

    /// A code sample that merely *shows* an attribute is TEXT, and the scrapers must read
    /// tags only. `escape_html` did not escape `"` until 2026-09-24, so an inline `<code>`
    /// span showing `<a href="draft.md">` put a literal `href="` into the document's text, and the
    /// substring scan harvested it, so `deploy_referenced_sources` published a file nothing
    /// on the site linked. Exactly the FA11/FA12 defect one layer down: those were fixed on
    /// the render side by `render::rewrite_attr_in_tags`, and the build scrapers never got
    /// the same treatment.
    #[test]
    fn local_refs_reads_tags_not_prose_that_merely_shows_an_attribute() {
        let refs = local_urls(
            "<p>Write <code>&lt;a href=\"draft.md\"&gt;</code> to link a source.</p>\
             <p><a href=\"real.md\">the real link</a></p>",
        );
        assert!(refs.contains(&"real.md".to_string()), "{refs:?}");
        assert!(
            !refs.contains(&"draft.md".to_string()),
            "a code sample is not a reference: {refs:?}"
        );
    }

    /// The other half of the same rule: a `<script>` body is raw text, not markup. Every
    /// built page inlines mermaid and the `{js}` vendor bundles, whose source really does
    /// build HTML out of string fragments (`<a href="'+e+'"`, `<img src="${e}"`), so a scan
    /// with no notion of raw text harvests JS syntax as if it were a file.
    #[test]
    fn local_refs_ignores_html_built_inside_an_inlined_script() {
        let refs = local_urls(
            "<script>var t = '<a href=\"'+e+'\">' + '<img src=\"pic.png\">';</script>\
             <img src=\"real.png\">",
        );
        assert_eq!(refs, vec!["real.png".to_string()], "{refs:?}");
    }

    /// A single-quoted attribute is valid HTML, and raw HTML is in the trust model, so
    /// `<img src='pic.png'>` is something an author may hand-write. The scan only knew
    /// `src="`, so the file was never copied and the portable folder shipped a broken image.
    #[test]
    fn copy_local_assets_bundles_a_single_quoted_attribute() {
        let dir = tmp_dir("single-quote-copy");
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(dir.join("pic.png"), "x").unwrap();

        let copied = copy_local_assets("<img src='pic.png' alt='a'>", &dir, &out).copied;

        assert!(out.join("pic.png").is_file(), "a single-quoted src bundles");
        assert_eq!(copied, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    /// `%20` in a ref is how VS Code spells a dragged-in file whose name has spaces; the
    /// dev server decodes it when serving and a static host decodes the URL the same way,
    /// so the copier must bundle the DECODED file under its decoded name or the portable
    /// folder 404s the image the preview showed.
    #[test]
    fn copy_local_assets_bundles_a_percent_encoded_src_under_its_decoded_name() {
        let dir = tmp_dir("pct-copy");
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(dir.join("my image.png"), "x").unwrap();

        let copied = copy_local_assets("<img src=\"my%20image.png\" alt=\"a\">", &dir, &out).copied;

        assert_eq!(copied, 1);
        assert!(
            out.join("my image.png").is_file(),
            "the decoded file is what a static host resolves the emitted src to"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// `build post.tmd elsewhere/p.html` copies each local image beside the output, and a
    /// bare `fs::copy` replaced whatever was already there: two posts built into one folder
    /// clobbered each other's `figures/plot.png`, and an unrelated file of the author's was
    /// overwritten, with nothing printed. A DIFFERENT file at the destination is refused,
    /// named and located at the block that references it; the same bytes (a rebuild into the
    /// same folder) are not a conflict and count as bundled.
    #[test]
    fn copy_local_assets_never_overwrites_a_different_file() {
        let dir = tmp_dir("no-clobber");
        let (post, desk) = (dir.join("post"), dir.join("desk"));
        fs::create_dir_all(post.join("img")).unwrap();
        fs::create_dir_all(desk.join("img")).unwrap();
        fs::write(post.join("img/a.png"), "POST FIGURE").unwrap();
        fs::write(desk.join("img/a.png"), "USER FILE").unwrap();
        let html = r#"<p data-sourcepos="5:1-5:24"><img src="img/a.png" alt="A square."></p>"#;

        let got = copy_local_assets(html, &post, &desk);

        assert_eq!(
            fs::read_to_string(desk.join("img/a.png")).unwrap(),
            "USER FILE",
            "a file already at the destination must survive"
        );
        assert_eq!(got.copied, 0);
        let [w] = &got.problems[..] else {
            panic!("one refusal expected, got {:?}", got.problems);
        };
        assert_eq!(w.severity, taliesin_core::Severity::Error, "{w:?}");
        assert_eq!(w.line, Some(5), "located at the referencing block: {w:?}");
        assert!(w.message.contains("img/a.png"), "names the file: {w:?}");

        // The same bytes are what a rebuild into the same folder finds: not a conflict.
        fs::write(desk.join("img/a.png"), "POST FIGURE").unwrap();
        let again = copy_local_assets(html, &post, &desk);
        assert!(again.problems.is_empty(), "{:?}", again.problems);
        assert_eq!(again.copied, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Two spellings of one file are one asset. The `--out` summary counted each distinct
    /// attribute VALUE, so a page naming `my pic.png` as `my%20pic.png` and `<my pic.png>`
    /// reported more assets than the folder held.
    #[test]
    fn copy_local_assets_counts_two_spellings_of_one_file_once() {
        let dir = tmp_dir("two-spellings");
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(dir.join("my pic.png"), "x").unwrap();

        let html = r#"<img src="my%20pic.png"><img src="my pic.png"><img src="./my pic.png">"#;
        let got = copy_local_assets(html, &dir, &out);

        assert!(out.join("my pic.png").is_file());
        assert_eq!(got.copied, 1, "one file, however it is spelled");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A `srcset` candidate and a `<picture><source srcset>` are images the browser
    /// fetches (on a high-density screen, in dark mode), so they travel with the page.
    /// Harvesting `src`/`href`/`poster` only built a folder whose 2x and dark images 404'd
    /// while the preview, which serves any file, looked right.
    #[test]
    fn copy_local_assets_bundles_every_srcset_candidate() {
        let dir = tmp_dir("srcset-copy");
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        for f in ["fig.png", "fig-2x.png", "dark.png"] {
            fs::write(dir.join(f), f).unwrap();
        }
        let html = r#"<picture><source srcset="dark.png" media="(prefers-color-scheme: dark)">
            <img src="fig.png" srcset="fig.png 1x, fig-2x.png 2x" alt="A."></picture>"#;

        let got = copy_local_assets(html, &dir, &out);

        for f in ["fig.png", "fig-2x.png", "dark.png"] {
            assert!(out.join(f).is_file(), "`{f}` must be bundled");
        }
        assert_eq!(got.copied, 3);
        let _ = fs::remove_dir_all(&dir);
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
        let copied = deploy_referenced_sources(
            html,
            &dir,
            Path::new(""),
            &out,
            &mut Default::default(),
            &mut Default::default(),
        );

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

    /// A SINGLE-dash token is a flag too, not a path. `-o` is the output flag in most other
    /// renderers, so `build index.tmd -o out.html` is a plausible typo; with only `--`
    /// rejected it fell through to the positionals and `-o` became the *output file*, with
    /// `out.html` silently discarded and exit 0. Same reclassification `init`/`new` took in
    /// wave 8 when `-y` was removed.
    #[test]
    fn a_single_dash_token_is_a_flag_error_not_the_output_path() {
        let argv = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        let err = parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "-o", "out.html"]))
            .expect_err("-o must error rather than becoming the output path");
        assert!(err.contains("-o"), "names the bad token: {err}");
        // `--out` must not swallow one as its directory value either — same defect, one
        // token later (`--out -o` used to write a *directory* called `-o`).
        let err = parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "--out", "-o"]))
            .expect_err("--out must not take a flag as its directory");
        assert!(err.contains("--out") && err.contains("requires"), "{err}");
        // The one short flag `build` really does take still parses.
        assert!(parse_build_args(&argv(&["taliesin", "build", "doc.tmd", "-j", "2"])).is_ok());
        // A genuinely dash-named source file is still buildable, spelled portably.
        let a = argv(&["taliesin", "build", "./-weird.tmd"]);
        assert_eq!(parse_build_args(&a).unwrap().path, "./-weird.tmd");
    }

    /// `build` reads its argv by the grammar every verb shares (audit 2026-09-24, leads
    /// cluster 10), and each case below misbehaved before it did.
    #[test]
    fn build_args_follow_the_grammar_every_verb_shares() {
        let argv = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        // `--flag=value` means `--flag value`. `--format=json` was an unknown flag with no
        // did-you-mean.
        let a = argv(&[
            "taliesin",
            "build",
            "doc.tmd",
            "--check-only",
            "--format=json",
        ]);
        assert_eq!(parse_build_args(&a).unwrap().format, "json");
        let a = argv(&["taliesin", "build", "doc.tmd", "--out=dist"]);
        assert_eq!(parse_build_args(&a).unwrap().out_dir, Some("dist"));
        // A third positional is refused, not silently dropped.
        let err = parse_build_args(&argv(&[
            "taliesin",
            "build",
            "a.tmd",
            "out.html",
            "extra.html",
        ]))
        .expect_err("a third positional must be refused");
        assert!(
            err.contains("`extra.html`"),
            "names the extra argument: {err}"
        );
        // `--jobs` describes output whatever its value, so `--check-only` refuses `0` and
        // `auto` too. Both passed (they parse to "no cap") while `--jobs 4` was refused.
        for v in ["0", "auto", "4"] {
            let err = parse_build_args(&argv(&[
                "taliesin",
                "build",
                "site",
                "--check-only",
                "--jobs",
                v,
            ]))
            .expect_err("--check-only refuses --jobs");
            assert!(err.contains("--jobs"), "--jobs {v}: {err}");
        }
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

        let (copied, skipped) = mirror_assets(&root, &out, &[]);

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

        let copied = deploy_referenced_sources(
            html,
            &root,
            Path::new(""),
            &out,
            &mut Default::default(),
            &mut Default::default(),
        );

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

        let copied = copy_local_assets(html, &base, &out).copied;

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

        let copied = copy_local_assets(html, &base, &out).copied;

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

        let copied = copy_local_assets(html, &base, &out).copied;

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

        let copied = copy_local_assets(html, &base, &out).copied;

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
    use taliesin_core::render::{Cell, JsOpts};

    #[test]
    fn a_cell_that_never_ran_is_not_reported_as_an_author_exception() {
        // AP11-1. With a bogus `TALIESIN_PYTHON` the build logged "code cell raised an
        // uncaught exception; its traceback is baked into the output". Both halves were
        // false: no kernel ever launched, so no cell ran and no traceback exists — the most
        // likely setup failure there is, reported as a bug in the author's code. The kind of
        // failure now travels from the executor as data (`exec::Failure`), so the wording is
        // chosen from what happened rather than from the shape of the output HTML.
        let failure = |f| crate::exec::CellFailure {
            sourcepos: "7:1-9:3".into(),
            source_file: None,
            failure: f,
            hidden: false,
        };
        let unavailable = failure(crate::exec::Failure::NotRun(
            crate::exec::NOT_RUN_UNAVAILABLE,
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
        for kind in [
            crate::exec::NOT_RUN_CRASHED,
            crate::exec::NOT_RUN_DIED,
            crate::exec::NOT_RUN_REQUEST,
            crate::exec::NOT_RUN_TIMEOUT,
        ] {
            let msg = cell_error_message("p.tmd", &failure(crate::exec::Failure::NotRun(kind)));
            assert!(!msg.contains("exception"), "{kind}: {msg}");
        }

        // The real thing still reads as the real thing.
        let raised = failure(crate::exec::Failure::Raised);
        let msg = cell_error_message("p.tmd", &raised);
        assert!(
            msg.contains("uncaught exception") && msg.contains("traceback"),
            "a genuine crash keeps its wording: {msg}"
        );

        // Both are still *problems*: they reach `--format json` (and the page pass counts
        // every failed cell toward `--strict`), which is what AP11 verified as correct. Only
        // the wording was wrong.
        let failures = vec![unavailable, raised];
        assert_eq!(cell_error_diagnostics(&failures, "p.tmd").len(), 2);
    }

    /// `render` must flag kernel-executed cells — but not `{js}` cells,
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

    /// `build *.tmd` expands to `build a.tmd b.tmd`, and the second positional is the path
    /// `build` WRITES: without this guard `b.tmd`'s source is replaced by rendered HTML with
    /// exit 0 and a `built b.tmd` log line. The message must name the file and the glob, not
    /// DX11's "write `b.html` instead" (the mistake is two sources, not a format expectation).
    #[test]
    fn parse_build_args_refuses_to_overwrite_a_source_file() {
        let two = argv("about.tmd index.tmd");
        let err = parse_build_args(&two).expect_err("a .tmd output must Err");
        assert!(err.contains("index.tmd"), "names the target: {err}");
        assert!(err.contains("source file"), "says why: {err}");
        assert!(
            err.contains("*.tmd"),
            "names the glob that causes it: {err}"
        );

        // Case-insensitive: on a case-insensitive filesystem `A.TMD` IS the source file.
        assert!(
            parse_build_args(&argv("a.tmd B.TMD")).is_err(),
            "uppercase .TMD is the same file on macOS"
        );
        // Building a page onto itself is the same refusal.
        assert!(
            parse_build_args(&argv("a.tmd a.tmd")).is_err(),
            "self-write"
        );

        // Legitimate targets are untouched.
        for ok in ["out.html", "out.htm", "draft", "notes.txt"] {
            let a = argv(&format!("doc.tmd {ok}"));
            assert!(parse_build_args(&a).is_ok(), "allow {ok}");
        }
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
        let (copied, _skipped) = mirror_assets(&book, &out, &[]);

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

    /// The `.`/`_` privacy convention was tested on the LINK's name only, never on what the
    /// link reaches, so a link no page references, with an ordinary name, published a
    /// checkout's private files: `vendor -> ../.git` shipped `.git/config` (a token in a
    /// remote URL) and every object, and `data.txt -> ../.deploysecret` shipped the secret,
    /// under a clean `--check-only --strict`.
    #[test]
    fn mirror_assets_refuses_a_symlink_that_reaches_a_private_path() {
        //   <dir>/repo/.git/config, .git/objects/ab/cdef     private: never published
        //   <dir>/repo/.deploysecret                         private
        //   <dir>/repo/_drafts/wip.png                       not mirrored wholesale
        //   <dir>/repo/paper/fig.png                         an ordinary sibling
        //   <dir>/repo/blog/_site.yml                        the site root
        let dir = tmp("mirror-private-target");
        let repo = dir.join("repo");
        let blog = repo.join("blog");
        fs::create_dir_all(repo.join(".git/objects/ab")).unwrap();
        fs::create_dir_all(repo.join("_drafts")).unwrap();
        fs::create_dir_all(repo.join("paper")).unwrap();
        fs::create_dir_all(&blog).unwrap();
        fs::write(repo.join(".git/config"), b"url = https://TOKEN@x/y").unwrap();
        fs::write(repo.join(".git/objects/ab/cdef"), b"OBJECT").unwrap();
        fs::write(repo.join(".deploysecret"), b"SECRET").unwrap();
        fs::write(repo.join("_drafts/wip.png"), b"WIP").unwrap();
        fs::write(repo.join("paper/fig.png"), b"FIG").unwrap();
        fs::write(blog.join("_site.yml"), b"title: B\n").unwrap();
        symlink("../.git", blog.join("vendor")).unwrap();
        symlink("../.deploysecret", blog.join("data.txt")).unwrap();
        symlink("../_drafts", blog.join("drafts")).unwrap();
        symlink("../paper", blog.join("paper")).unwrap();

        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        let (copied, _skipped) = mirror_assets(&blog, &out, &[]);

        for leaked in ["vendor/config", "vendor/objects/ab/cdef", "data.txt"] {
            assert!(
                !out.join(leaked).exists(),
                "`{leaked}` reaches a dot-prefixed path and must not be published; \
                 copied: {copied:?}"
            );
        }
        assert!(
            !out.join("drafts/wip.png").exists(),
            "a link into an underscore folder is not mirrored wholesale either; copied: {copied:?}"
        );
        assert!(
            out.join("paper/fig.png").exists(),
            "a link to an ordinary sibling in the repository is still mirrored; copied: {copied:?}"
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
        let copied = copy_local_assets(html, &repo.join("doc"), &dest).copied;

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
        let copied = deploy_referenced_sources(
            r#"<a href="notes.md">notes</a>"#,
            &repo,
            Path::new(""),
            &dest,
            &mut Default::default(),
            &mut Default::default(),
        );

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

        let copied = deploy_referenced_sources_for_site(&root, &out, &mut Default::default());

        assert!(out.join("notes.md").is_file(), "the linked source ships");
        assert_eq!(
            copied, 1,
            "the mount must be walked once, so the linked source is deployed once"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
