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

/// One located diagnostic from the render warning channel, ready to print or serialize.
#[derive(Debug, Clone, serde::Serialize)]
struct Diagnostic {
    file: String,
    line: Option<u32>,
    message: String,
}

fn diag_from(w: &taliesin_core::render::Warning, fallback_file: &str) -> Diagnostic {
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
    let doc = taliesin_core::render_document_with_includes(&src, base);
    let path_str = path.display().to_string();
    use taliesin_core::diagnostics as dx;
    let xref = taliesin_core::cite::validate_xrefs(&doc.blocks);
    let dups = dx::validate_duplicate_heading_ids(&doc.blocks);
    let anchors = dx::validate_internal_anchors(&doc.blocks);
    let assets = dx::validate_local_assets(&doc.blocks, base);
    let media = dx::validate_local_media(&doc.blocks, base);
    let links = dx::validate_local_links(&doc.blocks, base);
    let reactive = dx::validate_js_reactive_graph(&doc.blocks);
    let a11y = dx::validate_a11y(&doc.blocks, doc.format);
    let math = dx::validate_math(&doc.blocks);
    let cites = dx::citations_without_bibliography(&src, &doc.blocks);
    let mut out: Vec<Diagnostic> = Vec::new();
    // Malformed YAML front matter: the lenient line-parser silently mis-extracts
    // fields, so surface the parse error here too (the live servers already do).
    if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(&src) {
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
            .chain(media.iter())
            .chain(links.iter())
            .chain(reactive.iter())
            .chain(a11y.iter())
            .chain(math.iter())
            .chain(cites.iter())
            .map(|w| diag_from(w, &path_str)),
    );
    Ok(out)
}

fn collect_site_diagnostics(root: &Path) -> Result<Vec<Diagnostic>, String> {
    let site = taliesin_core::Site::discover(root);
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
        if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(&src) {
            out.push(Diagnostic {
                file: page.rel.clone(),
                line: Some(line),
                message,
            });
        }
        let base = page.input.parent().unwrap_or(root);
        // Scope a numbered book chapter's theorems to its chapter ("Theorem 2.3"), matching
        // the build + live-preview paths; otherwise `number-within: chapter` would warn here.
        let doc =
            taliesin_core::render_document_with_includes_scoped(&src, base, site.chapter_for(page));
        // Static lints over the page's blocks (xrefs are added by render_page_doc_warned
        // below); run before `doc` is consumed.
        use taliesin_core::diagnostics as dx;
        let dups = dx::validate_duplicate_heading_ids(&doc.blocks);
        let anchors = dx::validate_internal_anchors(&doc.blocks);
        let assets = dx::validate_local_assets(&doc.blocks, base);
        let media = dx::validate_local_media(&doc.blocks, base);
        let reactive = dx::validate_js_reactive_graph(&doc.blocks);
        let a11y = dx::validate_a11y(&doc.blocks, doc.format);
        let math = dx::validate_math(&doc.blocks);
        let cites = dx::citations_without_bibliography(&src, &doc.blocks);
        for w in dups
            .iter()
            .chain(anchors.iter())
            .chain(assets.iter())
            .chain(media.iter())
            .chain(reactive.iter())
            .chain(a11y.iter())
            .chain(math.iter())
            .chain(cites.iter())
        {
            out.push(diag_from(w, &page.rel));
        }
        let (_html, warnings) = site.render_page_doc_warned(page, doc);
        for w in &warnings {
            out.push(diag_from(w, &page.rel));
        }
    }
    // Cross-page relative-link + anchor existence, resolved against the site page
    // registry (file links here, not the single-doc `validate_local_links`: a `.qmd`
    // link rewrites to its built `.html` and only the registry knows the real urls).
    for (page_rel, w) in site.validate_cross_page_links() {
        out.push(diag_from(&w, &page_rel));
    }
    Ok(out)
}

fn format_json(diags: &[Diagnostic]) -> String {
    serde_json::to_string_pretty(diags).unwrap_or_else(|_| "[]".to_string())
}

/// Serialize a `check --format json` failure (an unreadable path, an empty site) as a
/// single `{"error": "<message>"}` object, so the JSON stream a caller pipes to `jq`
/// stays valid even when `check` couldn't run. The message is JSON-escaped (quotes,
/// newlines), never raw-concatenated.
fn json_error(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// A migration breadcrumb when `dir` is a Quarto project (`_quarto.yml` present) with
/// no native `_site.yml`. Without it, the site walker reports `no _site.yml at <root>`
/// — a message naming a file the user never created. Returns `None` for any directory
/// that already has a `_site.yml`, lacks a `_quarto.yml`, or isn't a directory.
/// Shared with the site build (`main::build_site`), which surfaces the same breadcrumb.
pub(crate) fn quarto_migration_hint(dir: &Path) -> Option<String> {
    if !dir.is_dir() || dir.join("_site.yml").exists() || !dir.join("_quarto.yml").exists() {
        return None;
    }
    Some(
        "found `_quarto.yml` — qmd-fast uses `_site.yml` (a flat native schema), not \
         Quarto's `_quarto.yml`; run `qmd-fast init` for a starter, or see the docs"
            .to_string(),
    )
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

/// `qmd-fast check <file|dir> [--format human|json]`: render in memory, list every
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
        eprintln!("usage: qmd-fast check <file.qmd|dir> [--format human|json]");
        return ExitCode::FAILURE;
    };
    if format != "human" && format != "json" {
        log::error(&format!(
            "unknown --format `{format}` (expected human or json)"
        ));
        return ExitCode::FAILURE;
    }
    let target = Path::new(path);
    // A directory carrying a `_quarto.yml` but no `_site.yml` is a Quarto project, not
    // a native one: surface a migration breadcrumb instead of the confusing
    // `_site.yml: no _site.yml` diagnostic the site walker would otherwise emit.
    if let Some(hint) = quarto_migration_hint(target) {
        if format == "json" {
            let diag = Diagnostic {
                file: "_quarto.yml".to_string(),
                line: None,
                message: hint,
            };
            println!("{}", format_json(std::slice::from_ref(&diag)));
        } else {
            eprintln!("{path}: {hint}");
            eprintln!("1 problem");
        }
        return ExitCode::FAILURE;
    }
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
            "broken link",
            "broken link anchor",
            "local video not found",
            "unknown reactive input",
            "reactive dependency cycle",
            "heading level skips",
            "has no accessible name",
            "image is missing alt text",
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
        fs::write(dir.join("real.qmd"), "x").unwrap();
        let f = dir.join("doc.qmd");
        fs::write(
            &f,
            "---\ntitle: T\n---\n\n\
             A [gone](missing.qmd), an [ok](real.qmd), an [ext](https://example.com).\n\n\
             {{< video clip.mp4 >}}\n\n\
             ```{js}\n//| input: nope\nreturn nope;\n```\n\n\
             ```{js}\n//| name: a\n//| input: b\nreturn b;\n```\n\n\
             ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&f).expect("ok");
        let has = |needle: &str| diags.iter().any(|d| d.message.contains(needle));
        assert!(has("broken link: `missing.qmd`"), "broken link: {diags:?}");
        assert!(has("local video not found"), "missing video: {diags:?}");
        assert!(has("`clip.mp4`"), "video path: {diags:?}");
        assert!(
            has("unknown reactive input `nope`"),
            "dangling input: {diags:?}"
        );
        assert!(has("reactive dependency cycle"), "cycle: {diags:?}");
        // The existing sibling + external link must NOT be flagged.
        assert!(
            !has("real.qmd"),
            "sibling that exists must be clean: {diags:?}"
        );
        assert!(
            !has("example.com"),
            "external link must be skipped: {diags:?}"
        );
        assert!(
            diags.iter().all(|d| d.file.contains("doc.qmd")),
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
            dir.join("index.qmd"),
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
        // One doc tripping each new static a11y rule: a raw `<img>` with no alt, an h2->h4
        // heading skip, and an empty (icon-only) link. `check` must surface them all, located,
        // while leaving an `alt`-bearing image and a single-level heading step alone.
        let dir = tmp("check-a11y");
        let f = dir.join("doc.qmd");
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
            has("heading level skips from h2 to h4"),
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
                .all(|d| d.line.is_some() && d.file.contains("doc.qmd")),
            "a11y diagnostics located to file+line: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corpus_a11y_pin_doc_trips_each_rule_through_check() {
        // The corpus pin (`corpus/diagnostics/a11y.qmd`, exempt from the no-false-positive
        // guard) must fire every a11y rule through the real `collect_diagnostics` flow.
        let doc = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics/a11y.qmd");
        let diags = collect_diagnostics(&doc).expect("pin doc checks");
        let has = |needle: &str| diags.iter().any(|d| d.message.contains(needle));
        assert!(has("image is missing alt text"), "raw img: {diags:?}");
        assert!(
            has("heading level skips from h2 to h4"),
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
        let f = dir.join("deck.qmd");
        fs::write(
            &f,
            "---\ntitle: T\nformat: revealjs\n---\n\n## Slide one\n\n#### A deeper heading\n",
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
        // The site path resolves links against the page registry: a `.qmd` link to a
        // missing page, and a `page.html#frag` whose anchor isn't on the target page.
        let dir = tmp("check-site-links");
        fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
        fs::write(dir.join("index.qmd"), "---\ntitle: Home\n---\n\nWelcome.\n").unwrap();
        fs::write(
            dir.join("about.qmd"),
            "---\ntitle: About\n---\n\n## Team {#team}\n\nAbout us.\n",
        )
        .unwrap();
        fs::write(
            dir.join("page.qmd"),
            "---\ntitle: P\n---\n\n\
             A [missing page](ghost.qmd), a [good page](about.qmd), \
             a [good anchor](about.qmd#team), a [bad anchor](about.qmd#nope).\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&dir).expect("site ok");
        let has = |needle: &str| diags.iter().any(|d| d.message.contains(needle));
        // `ghost.qmd` -> `ghost.html`, no such page.
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("ghost.html") && d.file.contains("page.qmd")),
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

    /// The `--format json` error path must produce a single valid JSON object
    /// (`{"error": "..."}`) so a `check … --format json | jq` pipeline stays parseable
    /// even when the path can't be read. This pins the serialized shape.
    #[test]
    fn json_error_is_valid_json_object() {
        let s = json_error("cannot read missing.qmd: No such file or directory");
        let v: serde_json::Value = serde_json::from_str(&s).expect("error envelope is valid JSON");
        assert_eq!(
            v.get("error").and_then(|e| e.as_str()),
            Some("cannot read missing.qmd: No such file or directory")
        );
        // Quotes/newlines in the message stay escaped (not a raw concatenation).
        let tricky = json_error("bad \"path\"\nline2");
        let v2: serde_json::Value = serde_json::from_str(&tricky).expect("escaped JSON");
        assert_eq!(
            v2.get("error").and_then(|e| e.as_str()),
            Some("bad \"path\"\nline2")
        );
    }

    /// A directory with a `_quarto.yml` but no `_site.yml` gets a clear migration
    /// breadcrumb (naming both files), not the confusing `no _site.yml` message.
    #[test]
    fn quarto_hint_fires_only_without_site_yml() {
        let dir = tmp("quarto-only");
        fs::write(dir.join("_quarto.yml"), "project:\n  type: website\n").unwrap();

        let hint =
            quarto_migration_hint(&dir).expect("breadcrumb fires for a _quarto.yml-only dir");
        assert!(
            hint.contains("_quarto.yml"),
            "names the Quarto file: {hint}"
        );
        assert!(hint.contains("_site.yml"), "names the native file: {hint}");

        // Once a native `_site.yml` exists, the project is native — no breadcrumb.
        fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
        assert!(
            quarto_migration_hint(&dir).is_none(),
            "no breadcrumb once _site.yml is present"
        );

        // A plain directory (neither file) never triggers it.
        let plain = tmp("plain");
        assert!(quarto_migration_hint(&plain).is_none());

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&plain);
    }
}
