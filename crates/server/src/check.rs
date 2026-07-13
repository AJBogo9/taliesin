//! The `check` subcommand: static, kernel-free document linting (the "check-superset").
//!
//! **What:** renders a file or site in memory and lists every located diagnostic — the
//! render warning channel plus the static validators (xrefs, duplicate ids, anchors,
//! assets, media, links, reactive graph, a11y, citations, front-matter YAML) — exiting
//! non-zero on any finding. A CI / pre-publish gate; no code execution.
//!
//! **How to use:** `main()` dispatches `check` to [`cmd_check`]; `--format human|json`.
//!
//! **Depends on:** [`taliesin_core`] for rendering + the `diagnostics`/`cite` validators
//! + `Site`, [`crate::log`], and `serde_json` for the JSON formatter.

use crate::log;
use std::path::Path;
use std::process::ExitCode;

/// One located diagnostic, ready to print or serialize. Under `--format json` it is
/// agent-grade: a stable `code`, a `severity`, and (for a "did you mean" typo) a
/// structured `suggestion` (`{ replacement }`). `--format human` ignores those extra
/// fields, so its output is byte-identical to before. (Keys serialize alphabetically:
/// `format_json` routes through `serde_json::json!`, whose object is key-sorted.)
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Diagnostic {
    code: &'static str,
    severity: &'static str,
    file: String,
    line: Option<u32>,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<Suggestion>,
}

/// A structured, applicable fix lifted from an inline "did you mean `X`?" hint.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Suggestion {
    replacement: String,
}

impl Diagnostic {
    /// Build a diagnostic, classifying its `code`/`severity` and lifting any inline
    /// "did you mean" hint into a structured `suggestion` from the message. Shared with the
    /// `build`/`publish` structured-error path.
    pub(crate) fn new(file: String, line: Option<u32>, message: String) -> Self {
        use taliesin_core::diagnostics::codes;
        let (code, severity) = codes::classify(&message);
        let suggestion =
            codes::extract_suggestion(&message).map(|replacement| Suggestion { replacement });
        Diagnostic {
            code,
            severity,
            file,
            line,
            message,
            suggestion,
        }
    }
}

pub(crate) fn diag_from(w: &taliesin_core::render::Warning, fallback_file: &str) -> Diagnostic {
    Diagnostic::new(
        w.file.clone().unwrap_or_else(|| fallback_file.to_string()),
        w.line,
        w.message.clone(),
    )
}

/// Serialize just the diagnostics as `{ "diagnostics": [...] }` — the shape `build`/`publish`
/// emit under `--format json` (no `environment`; a build already runs kernels, and the
/// agent consuming a failing build wants the problems, not the interpreter probe). Reuses
/// the exact per-diagnostic shape as `check`, so the two channels can't drift.
pub(crate) fn diagnostics_json(diags: &[Diagnostic]) -> String {
    let payload = serde_json::json!({ "diagnostics": diags });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{\"diagnostics\":[]}".to_string())
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

/// Whether the document being validated is a page of a multi-page site, which changes
/// exactly one rule (see [`page_static_diagnostics`]).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Scope {
    Standalone,
    InSite,
}

/// Every **static** validator, over one already-rendered document: the "check-superset".
/// No code execution, no filesystem writes; the local-asset/media/link rules do stat the
/// filesystem.
///
/// This is the single definition of the superset, so `check`, `build --strict` and
/// `publish` cannot drift on what counts as a defect. It deliberately excludes the two
/// checks the callers already run themselves (`cite::validate_xrefs`, and the front-matter
/// YAML parse), so nothing is counted twice.
///
/// Run it on the document **before** its code cells execute, as `check` does: a matplotlib
/// figure spliced in by a cell is generated output, and linting it for alt text would
/// report a defect the author cannot fix in the source.
///
/// [`Scope::InSite`] omits `validate_local_links`. An intra-site `[x](other.tmd)` link
/// rewrites to `other.html`, and only the site's page registry knows the real URLs, so on
/// a site page that rule reports every internal link as broken. `Site::validate_cross_page_links`
/// is its site-aware counterpart, run once over the whole project.
pub(crate) fn page_static_diagnostics(
    src: &str,
    blocks: &[taliesin_core::Block],
    base: &Path,
    format: taliesin_core::DocFormat,
    scope: Scope,
) -> Vec<taliesin_core::render::Warning> {
    use taliesin_core::diagnostics as dx;
    let mut out = Vec::new();
    out.extend(dx::validate_duplicate_heading_ids(blocks));
    out.extend(dx::validate_internal_anchors(blocks));
    out.extend(dx::validate_local_assets(blocks, base));
    out.extend(dx::validate_local_media(blocks, base));
    if scope == Scope::Standalone {
        out.extend(dx::validate_local_links(blocks, base));
    }
    out.extend(dx::validate_js_reactive_graph(blocks));
    out.extend(dx::validate_a11y(blocks, format));
    out.extend(dx::validate_math(blocks));
    out.extend(dx::validate_code_languages(blocks));
    out.extend(dx::citations_without_bibliography(src, blocks));
    out
}

fn collect_file_diagnostics(path: &Path) -> Result<Vec<Diagnostic>, String> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let doc = taliesin_core::render_document_with_includes(&src, base);
    let path_str = path.display().to_string();
    let xref = taliesin_core::cite::validate_xrefs(&doc.blocks);
    let statics = page_static_diagnostics(&src, &doc.blocks, base, doc.format, Scope::Standalone);
    let mut out: Vec<Diagnostic> = Vec::new();
    // Malformed YAML front matter: the lenient line-parser silently mis-extracts
    // fields, so surface the parse error here too (the live servers already do).
    if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(&src) {
        out.push(Diagnostic::new(path_str.clone(), Some(line), message));
    }
    out.extend(
        doc.warnings
            .iter()
            .chain(xref.iter())
            .chain(statics.iter())
            .map(|w| diag_from(w, &path_str)),
    );
    Ok(out)
}

fn collect_site_diagnostics(root: &Path) -> Result<Vec<Diagnostic>, String> {
    let site = taliesin_core::Site::discover(root);
    if site.pages.is_empty() {
        return Err(format!("no .tmd pages found under {}", root.display()));
    }
    // A bare directory of `.tmd` pages is a legitimate project, so a missing `_site.yml` is
    // an advisory, not a defect: reporting it made `check` print "1 problem" and exit 1 on
    // a perfectly good tree, while `build` had always declined to count it.
    let mut out: Vec<Diagnostic> = site
        .warnings
        .iter()
        .filter(|m| !taliesin_core::site::is_missing_config_warning(m))
        .map(|m| Diagnostic::new("_site.yml".to_string(), None, m.clone()))
        .collect();
    for page in &site.pages {
        let Ok(src) = std::fs::read_to_string(&page.input) else {
            out.push(Diagnostic::new(
                page.rel.clone(),
                None,
                format!("cannot read {}", page.input.display()),
            ));
            continue;
        };
        if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(&src) {
            out.push(Diagnostic::new(page.rel.clone(), Some(line), message));
        }
        let base = page.input.parent().unwrap_or(root);
        // Scope a numbered book chapter's theorems to its chapter ("Theorem 2.3"), matching
        // the build + live-preview paths; otherwise `number-within: chapter` would warn here.
        let doc =
            taliesin_core::render_document_with_includes_scoped(&src, base, site.chapter_for(page));
        // Static lints over the page's blocks (xrefs are added by render_page_doc_warned
        // below); run before `doc` is consumed.
        for w in &page_static_diagnostics(&src, &doc.blocks, base, doc.format, Scope::InSite) {
            out.push(diag_from(w, &page.rel));
        }
        let (_html, warnings) = site.render_page_doc_warned(page, doc);
        for w in &warnings {
            out.push(diag_from(w, &page.rel));
        }
    }
    // Cross-page relative-link + anchor existence, resolved against the site page
    // registry (file links here, not the single-doc `validate_local_links`: a `.tmd`
    // link rewrites to its built `.html` and only the registry knows the real urls).
    for (page_rel, w) in site.validate_cross_page_links() {
        out.push(diag_from(&w, &page_rel));
    }
    // A typo'd category silently forks the listing filter into two chips; only the whole
    // site's vocabulary reveals it, so this runs here rather than per page.
    for (page_rel, w) in site.validate_categories() {
        out.push(diag_from(&w, &page_rel));
    }
    Ok(out)
}

/// One line of the informational Environment section: the interpreter `check`
/// resolved for a language the document runs, and whether its Jupyter kernel package
/// is importable. Serialized into `--format json` and printed after the diagnostics.
#[derive(serde::Serialize)]
struct EnvEntry {
    lang: &'static str,
    path: String,
    provenance: String,
    /// The interpreter binary spawned + returned a version (it exists and runs). When
    /// `false`, the binary itself is absent/broken and `kernel_pkg_ok` is moot.
    runs: bool,
    /// `ipykernel` (python) / `IRkernel` (r).
    kernel_pkg: &'static str,
    kernel_pkg_ok: bool,
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Which executable languages (`python`/`r`) a document actually uses, in first-seen
/// order. Scans the rendered block model's cells (so `{{< include >}}`d cells count),
/// stopping once both are seen.
fn used_languages(blocks: &[taliesin_core::Block]) -> Vec<&'static str> {
    let mut seen = Vec::new();
    for b in blocks {
        if let Some(c) = &b.cell
            && let Some(lang) = crate::exec::kernel_lang(&c.lang)
            && !seen.contains(&lang)
        {
            seen.push(lang);
            if seen.len() == 2 {
                break;
            }
        }
    }
    seen
}

/// Build one `EnvEntry` for `lang` given the resolved interpreter (probes it).
fn env_entry(lang: &'static str, resolved: &crate::interpreter::Resolved) -> EnvEntry {
    let lang_enum = if lang == "r" {
        crate::interpreter::Lang::R
    } else {
        crate::interpreter::Lang::Python
    };
    let p = crate::interpreter::probe(resolved, lang_enum);
    EnvEntry {
        lang,
        path: resolved.path.display().to_string(),
        provenance: resolved.provenance.label(lang_enum).to_string(),
        runs: p.runs,
        kernel_pkg: if lang == "r" { "IRkernel" } else { "ipykernel" },
        kernel_pkg_ok: p.kernel_pkg_ok,
        version: p.version,
        error: p.error,
    }
}

/// The informational Environment section for a file or site: for each executable
/// language the target uses, the resolved interpreter + kernel-package probe. Never
/// affects `check`'s exit code. Field pins come from `_site.yml` for a site; a single
/// file has none. Empty when the target has no python/r cells.
fn collect_environment(path: &Path) -> Vec<EnvEntry> {
    if path.is_dir() {
        let site = taliesin_core::Site::discover(path);
        // Union of languages across pages, plus the project-level field pins + root.
        let mut langs: Vec<&'static str> = Vec::new();
        for page in &site.pages {
            let Ok(src) = std::fs::read_to_string(&page.input) else {
                continue;
            };
            let base = page.input.parent().unwrap_or(path);
            let doc = taliesin_core::render_document_with_includes_scoped(
                &src,
                base,
                site.chapter_for(page),
            );
            for l in used_languages(&doc.blocks) {
                if !langs.contains(&l) {
                    langs.push(l);
                }
            }
            if langs.len() == 2 {
                break;
            }
        }
        langs
            .into_iter()
            .map(|lang| {
                let resolved = if lang == "r" {
                    crate::interpreter::resolve_r(site.config.r.as_deref(), path)
                } else {
                    crate::interpreter::resolve_python(site.config.python.as_deref(), path)
                };
                env_entry(lang, &resolved)
            })
            .collect()
    } else {
        let Ok(src) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let doc = taliesin_core::render_document_with_includes(&src, base);
        used_languages(&doc.blocks)
            .into_iter()
            .map(|lang| {
                let resolved = if lang == "r" {
                    crate::interpreter::resolve_r(None, base)
                } else {
                    crate::interpreter::resolve_python(None, base)
                };
                env_entry(lang, &resolved)
            })
            .collect()
    }
}

/// Serialize `check --format json` as `{ "diagnostics": [...], "environment": [...] }`.
/// The Environment array is informational (it never changes the exit code); a consumer
/// that only wants problems reads `.diagnostics`.
fn format_json(diags: &[Diagnostic], environment: &[EnvEntry]) -> String {
    let payload = serde_json::json!({
        "diagnostics": diags,
        "environment": environment,
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}

/// Serialize a `check --format json` failure (an unreadable path, an empty site) as a
/// single `{"error": "<message>"}` object, so the JSON stream a caller pipes to `jq`
/// stays valid even when `check` couldn't run. The message is JSON-escaped (quotes,
/// newlines), never raw-concatenated.
fn json_error(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
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

/// Every long flag `check` accepts (drives the unknown-flag did-you-mean).
const CHECK_FLAGS: &[&str] = &["--format"];

/// `taliesin check <file|dir> [--format human|json]`: render in memory, list every
/// located diagnostic, and exit non-zero if any are found (a CI gate). Static-only
/// (no code execution).
pub(crate) fn cmd_check(args: &[String]) -> ExitCode {
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
            // An unrecognized `--flag` is a hard error with a did-you-mean (not silently
            // dropped — a typo'd `--formt json` would otherwise run with default human output).
            s if s.starts_with("--") => {
                log::error(&crate::serve::unknown_flag_error(s, CHECK_FLAGS));
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
        eprintln!("usage: taliesin check <file.tmd|dir> [--format human|json]");
        return ExitCode::FAILURE;
    };
    if format != "human" && format != "json" {
        log::error(&format!(
            "unknown --format `{format}` (expected human or json)"
        ));
        return ExitCode::FAILURE;
    }
    let target = Path::new(path);
    // Guard the render: a panic in core rendering becomes a clean located error + non-zero
    // exit (routed through the same error path, so `--format json` stays valid) instead of
    // a raw abort that would crash a CI gate.
    let collected = crate::serve::guarded(|| collect_diagnostics(target))
        .map_err(|panic| format!("render panicked on {path}: {panic}"))
        .and_then(|r| r);
    let diags = match collected {
        Ok(d) => d,
        // Honour `--format json` on the error path too: a human stderr line would
        // corrupt a `check … --format json | jq` stream (and leave stdout empty), so
        // emit a `{"error": …}` object to stdout. Human format keeps the stderr message.
        Err(e) => {
            if format == "json" {
                println!("{}", json_error(&e));
            } else {
                log::error(&e);
            }
            return ExitCode::FAILURE;
        }
    };
    // Informational only: which interpreter each used language resolves to + whether
    // its Jupyter kernel package is importable. This never contributes to the exit code
    // (a CI box without Python must not fail static linting).
    let environment = collect_environment(target);
    if format == "json" {
        // JSON to stdout only, so it pipes cleanly.
        println!("{}", format_json(&diags, &environment));
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
        if !environment.is_empty() {
            eprintln!("\nEnvironment:");
            for e in &environment {
                let pkg = if !e.runs {
                    // The interpreter binary itself is absent/broken, so the kernel
                    // package is moot; name that instead of a misleading "pkg MISSING".
                    "interpreter not found or failed to run".to_string()
                } else if e.kernel_pkg_ok {
                    match &e.version {
                        Some(v) => format!("{} present ({v})", e.kernel_pkg),
                        None => format!("{} present", e.kernel_pkg),
                    }
                } else {
                    format!("{} MISSING", e.kernel_pkg)
                };
                eprintln!("  {}: {} ({}), {}", e.lang, e.path, e.provenance, pkg);
            }
        }
    }
    if diags.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("tali-check-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn collect_diagnostics_flags_frontmatter_typo_and_broken_xref() {
        let dir = tmp("check-file");
        let f = dir.join("doc.tmd");
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
            diags.iter().all(|d| d.file.contains("doc.tmd")),
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
        let f = dir.join("doc.tmd");
        // Unterminated double-quoted scalar -> serde_yaml parse error.
        fs::write(&f, "---\ntitle: \"unterminated\nauthor: A\n---\n\nBody.\n").unwrap();
        let diags = collect_diagnostics(&f).expect("ok");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("YAML") && d.file.contains("doc.tmd")),
            "malformed YAML must be reported, located: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_diagnostics_surfaces_check_superset_validators() {
        // One doc tripping each new static check; `check` must surface them all.
        let dir = tmp("check-superset");
        let f = dir.join("doc.tmd");
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
            diags.iter().all(|d| d.file.contains("doc.tmd")),
            "located to file: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_site_diagnostics_surfaces_validators_located_per_page() {
        // The site path (per-page base dir + page.rel plumbing) must trip the validators too.
        let dir = tmp("check-site");
        fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
        fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nWelcome.\n").unwrap();
        fs::write(
            dir.join("page.tmd"),
            "---\ntitle: P\n---\n\n## A {#dup}\n\n## B {#dup}\n\nA missing ![x](nope.png).\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&dir).expect("site ok");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("duplicate heading id") && d.file.contains("page.tmd")),
            "dup id located to its page: {diags:?}"
        );
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nope.png") && d.file.contains("page.tmd")),
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
            "broken link",
            "broken link anchor",
            "local video not found",
            "unknown reactive input",
            "reactive dependency cycle",
            "heading level skips",
            "has no accessible name",
            "image is missing alt text",
            "looks like a placeholder",
        ];
        fn walk(dir: &Path, skip: &[&str], out: &mut Vec<std::path::PathBuf>) {
            for e in fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                if p.is_dir() {
                    if !skip.contains(&name.as_str()) {
                        walk(&p, skip, out);
                    }
                } else if taliesin_core::ext::is_source_path(&p) && !name.starts_with('_') {
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
    fn collect_diagnostics_surfaces_links_video_and_reactive_rules() {
        // One doc tripping each NEW static rule: broken relative link, missing local
        // video, dangling `//| input`, and a reactive cycle. `check` must surface them all,
        // located, while leaving an external link + an existing sibling alone.
        let dir = tmp("check-links");
        fs::write(dir.join("real.tmd"), "x").unwrap();
        let f = dir.join("doc.tmd");
        fs::write(
            &f,
            "---\ntitle: T\n---\n\n\
             A [gone](missing.tmd), an [ok](real.tmd), an [ext](https://example.com).\n\n\
             {{< video clip.mp4 >}}\n\n\
             ```{js}\n//| input: nope\nreturn nope;\n```\n\n\
             ```{js}\n//| name: a\n//| input: b\nreturn b;\n```\n\n\
             ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&f).expect("ok");
        let has = |needle: &str| diags.iter().any(|d| d.message.contains(needle));
        assert!(has("broken link: `missing.tmd`"), "broken link: {diags:?}");
        assert!(has("local video not found"), "missing video: {diags:?}");
        assert!(has("`clip.mp4`"), "video path: {diags:?}");
        assert!(
            has("unknown reactive input `nope`"),
            "dangling input: {diags:?}"
        );
        assert!(has("reactive dependency cycle"), "cycle: {diags:?}");
        // The existing sibling + external link must NOT be flagged.
        assert!(
            !has("real.tmd"),
            "sibling that exists must be clean: {diags:?}"
        );
        assert!(
            !has("example.com"),
            "external link must be skipped: {diags:?}"
        );
        assert!(
            diags.iter().all(|d| d.file.contains("doc.tmd")),
            "located to file: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_diagnostics_does_not_flag_links_into_a_mounted_subsite() {
        // A site that `mounts:` another project under a URL prefix; a page links into
        // that prefix (both the `dir/page.html` and the `dir/` index forms). Those links
        // resolve only when the mount is served, so `check` must NOT report them broken.
        // Regression guard: validate_cross_page_links ignored `mounts:` and flagged the
        // project's own deployed marketing-site links (8 false positives).
        let dir = tmp("check-mounts");
        fs::write(
            dir.join("_site.yml"),
            "output: _site\nmounts:\n  docs: ../docs\n",
        )
        .unwrap();
        fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\n---\n\n\
             See the [guide](docs/intro.html) and the [docs home](docs/).\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&dir).expect("ok");
        assert!(
            !diags.iter().any(|d| d.message.contains("broken link")),
            "links into a mount prefix must not be flagged broken: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_diagnostics_surfaces_a11y_rules() {
        // One doc tripping each new static a11y rule: a raw `<img>` with no alt, an authored
        // `##`->`####` heading skip, and an empty (icon-only) link. `check` must surface them
        // all, located, while leaving an `alt`-bearing image and a single-level heading step
        // alone. The doc has a title block, so heading demotion (#11) renders `##`/`####` as
        // h3/h5: the skip is preserved (difference-invariant) and reported at the shipped levels.
        let dir = tmp("check-a11y");
        let f = dir.join("doc.tmd");
        fs::write(
            &f,
            "---\ntitle: T\n---\n\n\
             ## Section\n\n\
             <img src=\"raw.png\">\n\n\
             ![described](ok.png) and a [real link](page.html).\n\n\
             #### Skips a level\n\n\
             Here is [](#) an icon-only link.\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&f).expect("ok");
        let has = |needle: &str| diags.iter().any(|d| d.message.contains(needle));
        assert!(has("image is missing alt text"), "raw img: {diags:?}");
        assert!(
            has("heading level skips from h3 to h5"),
            "heading skip: {diags:?}"
        );
        assert!(has("link has no accessible name"), "empty link: {diags:?}");
        // The markdown image (auto-alt) and the text link must NOT be flagged.
        assert_eq!(
            diags
                .iter()
                .filter(|d| d.message.contains("image is missing alt text"))
                .count(),
            1,
            "only the raw alt-less img: {diags:?}"
        );
        assert_eq!(
            diags
                .iter()
                .filter(|d| d.message.contains("link has no accessible name"))
                .count(),
            1,
            "only the empty link: {diags:?}"
        );
        assert!(
            diags
                .iter()
                .filter(|d| d.message.contains("has no accessible name")
                    || d.message.contains("missing alt text")
                    || d.message.contains("heading level skips"))
                .all(|d| d.line.is_some() && d.file.contains("doc.tmd")),
            "a11y diagnostics located to file+line: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corpus_a11y_pin_doc_trips_each_rule_through_check() {
        // The corpus pin (`corpus/diagnostics/a11y.tmd`, exempt from the no-false-positive
        // guard) must fire every a11y rule through the real `collect_diagnostics` flow.
        let doc = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics/a11y.tmd");
        let diags = collect_diagnostics(&doc).expect("pin doc checks");
        let has = |needle: &str| diags.iter().any(|d| d.message.contains(needle));
        assert!(has("image is missing alt text"), "raw img: {diags:?}");
        assert!(
            has("looks like a placeholder"),
            "placeholder alt (alt=\"image\"): {diags:?}"
        );
        assert!(
            has("heading level skips from h3 to h5"),
            "heading skip: {diags:?}"
        );
        assert!(has("link has no accessible name"), "empty link: {diags:?}");
        assert!(
            has("button has no accessible name"),
            "empty button: {diags:?}"
        );
        // The `[role=button|link|tab]` path fires the same rule on a `<div role="button">` /
        // `<span role="link">` with no name — so BOTH the native and the role-based elements
        // are flagged (count >= 2 each). Pins `role_interactives` end-to-end through the doc.
        let count = |needle: &str| diags.iter().filter(|d| d.message.contains(needle)).count();
        assert!(
            count("button has no accessible name") >= 2,
            "native <button> + <div role=button> should both flag: {diags:?}"
        );
        assert!(
            count("link has no accessible name") >= 2,
            "native <a> + <span role=link> should both flag: {diags:?}"
        );
    }

    #[test]
    fn collect_diagnostics_skips_heading_skip_for_decks() {
        // A reveal deck's `## … ####` is per-slide structure, not a single outline, so the
        // heading-skip rule must not fire on a deck.
        let dir = tmp("check-a11y-deck");
        let f = dir.join("deck.tmd");
        fs::write(
            &f,
            "---\ntitle: T\nformat: deck\n---\n\n## Slide one\n\n#### A deeper heading\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&f).expect("ok");
        assert!(
            !diags
                .iter()
                .any(|d| d.message.contains("heading level skips")),
            "decks skip the heading-skip rule: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_site_diagnostics_flags_broken_cross_page_link_and_anchor() {
        // The site path resolves links against the page registry: a `.tmd` link to a
        // missing page, and a `page.html#frag` whose anchor isn't on the target page.
        let dir = tmp("check-site-links");
        fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
        fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nWelcome.\n").unwrap();
        fs::write(
            dir.join("about.tmd"),
            "---\ntitle: About\n---\n\n## Team {#team}\n\nAbout us.\n",
        )
        .unwrap();
        fs::write(
            dir.join("page.tmd"),
            "---\ntitle: P\n---\n\n\
             A [missing page](ghost.tmd), a [good page](about.tmd), \
             a [good anchor](about.tmd#team), a [bad anchor](about.tmd#nope).\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&dir).expect("site ok");
        let has = |needle: &str| diags.iter().any(|d| d.message.contains(needle));
        // `ghost.tmd` -> `ghost.html`, no such page.
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("ghost.html") && d.file.contains("page.tmd")),
            "missing cross-page link located to its page: {diags:?}"
        );
        // `about.html#nope` -> the anchor `nope` is not on `about.html`.
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("broken link anchor") && d.message.contains("#nope")),
            "broken cross-page anchor: {diags:?}"
        );
        // The good page link + good anchor must NOT be flagged.
        assert!(
            !has("about.html#team"),
            "good anchor must be clean: {diags:?}"
        );
        assert!(
            !diags
                .iter()
                .any(|d| d.message.contains("broken link") && d.message.contains("about.html\"")),
            "good page link must be clean: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_diagnostics_clean_doc_is_empty() {
        let dir = tmp("check-clean");
        let f = dir.join("ok.tmd");
        fs::write(&f, "---\ntitle: T\n---\n\nJust clean prose.\n").unwrap();
        assert!(collect_diagnostics(&f).expect("ok").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_without_site_yml_is_advisory_not_a_problem() {
        // `check` counted the benign "no _site.yml" note as a problem and exited 1 on a
        // clean bare directory of pages, disagreeing with `build`, which never counted it.
        let dir = tmp("check-nositeyml");
        fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\n---\n\nClean prose.\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&dir).expect("a bare page directory is a site");
        assert!(
            diags.is_empty(),
            "a missing _site.yml is an advisory, not a problem: {diags:?}"
        );

        // A *malformed* `_site.yml` is still a real problem, and still counted.
        fs::write(dir.join("_site.yml"), "title: \"unterminated\n").unwrap();
        let diags = collect_diagnostics(&dir).expect("still discoverable");
        assert!(
            diags.iter().any(|d| d.message.contains("not valid YAML")),
            "malformed config must still be reported: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_site_diagnostics_flags_a_typod_category_but_not_distinct_short_tags() {
        // Three spellings of one category render three chips, each count 1, and the
        // reader's filter silently splits the archive. Nothing errored before this rule.
        let dir = tmp("check-categories");
        fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
        fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nWelcome.\n").unwrap();
        for (name, cat) in [
            ("a", "statistics"),
            ("b", "statistics"),
            ("c", "statistics"),
            ("d", "Statistics"),
            ("e", "statstics"),
        ] {
            fs::write(
                dir.join(format!("{name}.tmd")),
                format!("---\ntitle: {name}\ncategories:\n  - {cat}\n  - R\n  - C\n---\n\nBody.\n"),
            )
            .unwrap();
        }
        let diags = collect_diagnostics(&dir).expect("site ok");
        let cat_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.message.starts_with("category "))
            .collect();

        assert!(
            cat_diags.iter().any(|d| d.message.contains("`Statistics`")
                && d.message.contains("`statistics`")
                && d.file.contains("d.tmd")),
            "case-only fork, located to its page: {cat_diags:?}"
        );
        assert!(
            cat_diags.iter().any(|d| d.message.contains("`statstics`")
                && d.message.contains("`statistics`")
                && d.file.contains("e.tmd")),
            "near-miss typo, located to its page: {cat_diags:?}"
        );
        // The correct spelling, and the deliberately-short `R`/`C` tags (two edits apart),
        // must never be accused.
        assert_eq!(cat_diags.len(), 2, "exactly two findings: {cat_diags:?}");
        // Located to the `categories:` value line, so the editor can jump there.
        assert!(
            cat_diags.iter().all(|d| d.line.is_some()),
            "category diagnostics carry a line: {cat_diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_real_corpus_sites_have_no_category_false_positives() {
        // The load-bearing half: a correct site stays green. `tech-blog` is the only
        // corpus project with a real category vocabulary.
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
        for proj in ["tech-blog", "bayesian-website", "demo-book"] {
            let diags = collect_diagnostics(&corpus.join(proj)).unwrap_or_default();
            let cats: Vec<_> = diags
                .iter()
                .filter(|d| d.message.starts_with("category "))
                .collect();
            assert!(
                cats.is_empty(),
                "category false positive in {proj}: {cats:?}"
            );
        }
    }

    #[test]
    fn collect_diagnostics_empty_site_is_err() {
        let dir = tmp("check-emptysite");
        fs::write(dir.join("_site.yml"), "title: Empty\n").unwrap();
        assert!(collect_diagnostics(&dir).is_err(), "empty site -> Err");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_json_emits_diagnostics_and_environment_object() {
        // The JSON top level is `{ diagnostics: [...], environment: [...] }` (ruled
        // 2026-07-12): diagnostics keep their file/line/message shape under a named key,
        // and the informational environment probe rides alongside.
        let diags = vec![
            Diagnostic::new("a.tmd".into(), Some(3), "weasel word `very`".into()),
            Diagnostic::new("b.tmd".into(), None, "needs a \"name\"".into()),
        ];
        let json = format_json(&diags, &[]);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed["diagnostics"][0]["file"], "a.tmd");
        assert_eq!(parsed["diagnostics"][0]["line"], 3);
        // Agent-grade fields ride alongside the file/line/message.
        assert!(
            parsed["diagnostics"][0]["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("TAL-")),
            "each diagnostic carries a stable code: {json}"
        );
        assert!(
            matches!(
                parsed["diagnostics"][0]["severity"].as_str(),
                Some("error" | "warning")
            ),
            "each diagnostic carries a severity: {json}"
        );
        assert_eq!(parsed["diagnostics"][1]["line"], serde_json::Value::Null);
        assert_eq!(parsed["diagnostics"][1]["message"], "needs a \"name\"");
        assert!(
            parsed["environment"].is_array(),
            "environment rides alongside diagnostics as an array"
        );
    }

    #[test]
    fn environment_is_empty_for_a_doc_with_no_code_cells() {
        let dir = tmp("env-nocells");
        let f = dir.join("x.tmd");
        std::fs::write(&f, "# Title\n\nJust prose, no cells.\n").unwrap();
        assert!(
            collect_environment(&f).is_empty(),
            "a doc with no python/r cells reports no Environment entries"
        );
    }

    #[test]
    fn environment_lists_python_for_a_python_cell_doc() {
        let dir = tmp("env-pycell");
        let f = dir.join("x.tmd");
        std::fs::write(&f, "# T\n\n```{python}\nprint(1)\n```\n").unwrap();
        let env = collect_environment(&f);
        assert_eq!(
            env.len(),
            1,
            "one entry for the single python language used"
        );
        assert_eq!(env[0].lang, "python");
        // Path + provenance are populated; kernel_pkg_ok reflects the box (may be false
        // in CI). The section is informational, so we assert shape, not availability.
        assert!(!env[0].path.is_empty());
    }

    #[test]
    fn format_human_lists_located_lines() {
        let diags = vec![
            Diagnostic::new("a.tmd".into(), Some(3), "m1".into()),
            Diagnostic::new("b.tmd".into(), None, "m2".into()),
        ];
        let text = format_human(&diags);
        assert!(text.contains("a.tmd:3: m1"), "located line: {text}");
        assert!(text.contains("b.tmd: m2"), "unlocated line: {text}");
    }

    /// The `--format json` error path must produce a single valid JSON object
    /// (`{"error": "..."}`) so a `check … --format json | jq` pipeline stays parseable
    /// even when the path can't be read. This pins the serialized shape.
    #[test]
    fn json_error_is_valid_json_object() {
        let s = json_error("cannot read missing.tmd: No such file or directory");
        let v: serde_json::Value = serde_json::from_str(&s).expect("error envelope is valid JSON");
        assert_eq!(
            v.get("error").and_then(|e| e.as_str()),
            Some("cannot read missing.tmd: No such file or directory")
        );
        // Quotes/newlines in the message stay escaped (not a raw concatenation).
        let tricky = json_error("bad \"path\"\nline2");
        let v2: serde_json::Value = serde_json::from_str(&tricky).expect("escaped JSON");
        assert_eq!(
            v2.get("error").and_then(|e| e.as_str()),
            Some("bad \"path\"\nline2")
        );
    }

    /// The Quarto migration breadcrumb is shed: a directory carrying only a `_quarto.yml`
    /// (no native `_site.yml`, no `.tmd` pages) falls through to the normal site-walker
    /// diagnostic — a generic "no pages" message that never names Quarto.
    #[test]
    fn quarto_only_dir_gets_generic_diagnostic_not_a_breadcrumb() {
        // Neutral dir name: the diagnostic echoes the path, so a "quarto" in the dir name
        // would be a false positive for the breadcrumb we are asserting is gone.
        let dir = tmp("legacy-config-only");
        fs::write(dir.join("_quarto.yml"), "project:\n  type: website\n").unwrap();

        let err = collect_diagnostics(&dir).expect_err("a page-less dir is an error");
        assert!(
            !err.to_lowercase().contains("quarto"),
            "no Quarto breadcrumb should remain: {err}"
        );
        assert!(
            err.contains("no .tmd pages"),
            "expected the generic no-pages diagnostic: {err}"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
