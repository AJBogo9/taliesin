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
use std::io::IsTerminal;
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
    /// Where to read more about this `code`: the committed diagnostics catalog anchored by
    /// the lowercased code (`…/DIAGNOSTICS.md#tal-fm-key`). Computed from `code`, so it can
    /// never drift; the same text is available offline via `check --explain <code>`.
    docs_url: String,
    severity: &'static str,
    file: String,
    line: Option<u32>,
    /// 1-based `[col, end_col)` character span on `line`, present only when the underlying
    /// warning located a precise token (front-matter key typos). Omitted otherwise, so an
    /// un-columned diagnostic's JSON is byte-identical to before E3.
    #[serde(skip_serializing_if = "Option::is_none")]
    col: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_col: Option<u32>,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<Suggestion>,
}

/// A structured, applicable fix lifted from an inline "did you mean `X`?" hint.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Suggestion {
    replacement: String,
}

/// The `source` every diagnostic this server publishes is stamped with. A const because
/// `resolve_code_actions` reads it back to tell our diagnostics from the other providers an
/// editor attaches to the same buffer, and a quick fix built from someone else's diagnostic
/// rewrites the buffer using someone else's range.
pub(crate) const LSP_SOURCE: &str = "taliesin";

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
            docs_url: codes::docs_url(code),
            severity,
            file,
            line,
            col: None,
            end_col: None,
            message,
            suggestion,
        }
    }

    /// Project this diagnostic to LSP for the `lsp` server. `lines` is the buffer split
    /// on `\n` (needed to clamp the line and to bound a whole-line span). Mirrors the
    /// companion's `check.ts` mapping: 1-based line → 0-based, clamped to the buffer;
    /// a precise 1-based `[col, end_col)` → 0-based when present, else the whole line.
    pub(crate) fn to_lsp(&self, lines: &[&str]) -> lsp_types::Diagnostic {
        use lsp_types::{
            CodeDescription, DiagnosticSeverity, NumberOrString, Position, Range, Url,
        };
        let last = lines.len().saturating_sub(1) as u32;
        let line0 = self.line.unwrap_or(1).saturating_sub(1).min(last);
        // `col`/`end_col` are 1-based *character* columns; LSP columns are UTF-16 code units,
        // so convert against this line's text (a no-op for BMP text, which is why all realistic
        // natural-language docs already worked; astral chars are what shift).
        let line_text = lines.get(line0 as usize).copied().unwrap_or("");
        let to_u16 =
            |char_col: u32| crate::lsp_pos::char_to_utf16(line_text, char_col as usize) as u32;
        let range = match (self.col, self.end_col) {
            (Some(c), Some(e)) => Range::new(
                Position::new(line0, to_u16(c.saturating_sub(1))),
                Position::new(line0, to_u16(e.saturating_sub(1))),
            ),
            _ => {
                // CRLF-aware: a `\r` terminator is not a column the editor can put a cursor
                // on, so a whole-line squiggle that included it ran one column past the text.
                let len = crate::lsp_pos::line_end_utf16(line_text) as u32;
                Range::new(Position::new(line0, 0), Position::new(line0, len))
            }
        };
        let severity = Some(match self.severity {
            "error" => DiagnosticSeverity::ERROR,
            "warning" => DiagnosticSeverity::WARNING,
            "info" => DiagnosticSeverity::INFORMATION,
            _ => DiagnosticSeverity::HINT,
        });
        // Carry a one-click fix on `data` (the client echoes it back in a codeAction request)
        // ONLY when a suggestion has a precise column span — then `range` above is exactly the
        // token to overwrite. Without a column we cannot locate the token unambiguously, so we
        // attach nothing rather than offer an imprecise fix (mirrors the companion).
        let data = match (&self.suggestion, self.col, self.end_col) {
            (Some(s), Some(_), Some(_)) => {
                Some(serde_json::json!({ "replacement": s.replacement }))
            }
            _ => None,
        };
        lsp_types::Diagnostic {
            range,
            severity,
            code: Some(NumberOrString::String(self.code.to_string())),
            code_description: Url::parse(&self.docs_url)
                .ok()
                .map(|href| CodeDescription { href }),
            source: Some(LSP_SOURCE.to_string()),
            message: self.message.clone(),
            data,
            ..Default::default()
        }
    }
}

pub(crate) fn diag_from(w: &taliesin_core::render::Warning, fallback_file: &str) -> Diagnostic {
    let mut d = Diagnostic::new(
        w.file.clone().unwrap_or_else(|| fallback_file.to_string()),
        w.line,
        w.message.clone(),
    );
    d.col = w.col;
    d.end_col = w.end_col;
    d
}

/// Whether a render warning is **advice** rather than a defect (severity `suggestion`), so
/// `build --strict` and `publish` must report it and not fail on it. The classification is
/// the same one `check` uses, derived from the message, so the three commands cannot
/// disagree about what blocks a release.
pub(crate) fn is_advice(w: &taliesin_core::render::Warning) -> bool {
    use taliesin_core::diagnostics::codes;
    codes::classify(&w.message).1 == codes::SUGGESTION
}

/// How many of `warnings` block a `--strict` build (i.e. all but the advice).
pub(crate) fn blocking(warnings: &[taliesin_core::render::Warning]) -> usize {
    warnings.iter().filter(|w| !is_advice(w)).count()
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
fn collect_diagnostics(path: &Path, scope: &mut CheckScope) -> Result<Vec<Diagnostic>, String> {
    if path.is_dir() {
        collect_site_diagnostics(path, scope)
    } else {
        // Site-aware when the file is a page of a project, so `check <file.tmd>` and
        // `check <dir>` answer the same question about that page. The deck pass inside
        // `collect_site_diagnostics` keeps calling `collect_file_diagnostics` directly: it
        // is already inside a discovered project, and re-discovering per deck would make a
        // site check quadratic in the number of decks.
        let src = std::fs::read_to_string(path).map_err(|e| cannot_read(path, &e))?;
        let site = enclosing_site_of(path);
        collect_file_diagnostics_in_site(path, &src, Some(scope), site.as_ref())
    }
}

/// Whether the document being validated is a page of a multi-page site, which changes two
/// rules (see [`page_static_diagnostics`]).
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
    out.extend(dx::validate_link_text_collisions(blocks));
    out.extend(dx::validate_document_shape(blocks, format));
    out.extend(dx::validate_math(blocks));
    out.extend(dx::validate_code_languages(blocks));
    out.extend(dx::citations_without_bibliography(src, blocks));
    out.extend(dx::bare_citation_key_not_rendered(src, blocks, base));
    // No `csl:` rule here: it lives on the render path (`frontmatter::validate_front_matter`),
    // so it reaches the preview too and arrives with the rendered doc's warnings. Calling it
    // here as well would report it twice.
    out
}

/// The one "cannot read <path>" message every front door prints, with a "did you mean" when
/// the path does not exist but a near-miss sibling does.
///
/// `cannot read notes.tdm: No such file or directory (os error 2)` is technically complete
/// and practically useless: the user typed a transposition, the answer is one `read_dir`
/// away, and `closest` was already in the tree suggesting subcommands and front-matter keys.
/// Only the *missing* case is suggested for — a permission error or a directory-as-file is a
/// different problem, and offering a neighbour there would be a wrong guess dressed as help.
pub(crate) fn cannot_read(path: &Path, e: &std::io::Error) -> String {
    let base = format!("cannot read {}: {e}", path.display());
    if e.kind() != std::io::ErrorKind::NotFound {
        return base;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return base;
    };
    // The directory the user typed, which may legitimately be "" (a bare filename). Kept as
    // typed so the suggestion echoes their spelling: `./kern.tmd` for someone who wrote
    // `kern.tdm` is a correct path and a worse answer.
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let Ok(entries) = std::fs::read_dir(if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    }) else {
        return base;
    };
    // Candidates are the sibling *names*; the suggestion is re-joined onto the directory the
    // user actually typed, so `chapters/intro.tdm` suggests `chapters/intro.tmd` and not a
    // bare filename they would then have to relocate themselves.
    let names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    match taliesin_core::closest_of(name, names.iter().map(String::as_str)) {
        Some(near) => format!("{base} (did you mean `{}`?)", dir.join(near).display()),
        None => base,
    }
}

/// `scope` is `None` for the callers that only want diagnostics (a site's per-deck pass hands
/// its own in; `buffer_diagnostics` has no report to fill).
fn collect_file_diagnostics(
    path: &Path,
    scope: Option<&mut CheckScope>,
) -> Result<Vec<Diagnostic>, String> {
    let src = std::fs::read_to_string(path).map_err(|e| cannot_read(path, &e))?;
    collect_file_diagnostics_from_src(path, &src, scope)
}

/// Lint an already-in-hand source buffer as if it were the file at `path` — the seam the
/// LSP uses to lint an editor buffer (unsaved edits) instead of the last-saved file.
/// `path` supplies the base dir (relative includes/assets/links) + the reported location;
/// the file on disk is never read. `collect_file_diagnostics` is just this with the buffer
/// read from disk first.
fn collect_file_diagnostics_from_src(
    path: &Path,
    src: &str,
    scope: Option<&mut CheckScope>,
) -> Result<Vec<Diagnostic>, String> {
    collect_file_diagnostics_in_site(path, src, scope, None)
}

/// [`collect_file_diagnostics_from_src`] told which project the page belongs to.
///
/// `site` is `Some` only when this file is a published page of that project (see
/// [`taliesin_core::Site::page_for_input`]), and it changes two things, both of which the
/// live preview has done since DX1 while the editor did not:
///
/// * the scope becomes [`Scope::InSite`], which drops `validate_local_links` — a rule whose
///   "no such file under the document directory" phrasing describes a standalone document
///   and not a site page, whose links are `.html` urls the page registry resolves;
/// * `validate_cross_page_links_for_src` runs, which is the site-aware counterpart and the
///   only thing that can see a broken cross-page **anchor** at all.
///
/// Passing `None` keeps the standalone behaviour, which is right for a document with no
/// project above it and for a deck (a deck is built and served but is deliberately not one
/// of `site.pages`, so the site rules would remove its link check and put nothing back).
fn collect_file_diagnostics_in_site(
    path: &Path,
    src: &str,
    scope: Option<&mut CheckScope>,
    site: Option<&taliesin_core::Site>,
) -> Result<Vec<Diagnostic>, String> {
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let doc = taliesin_core::render_single_doc(src, base);
    // Free: this render already happened for the lints below.
    if let Some(scope) = scope {
        scope.note_languages(&doc.blocks);
    }
    let path_str = path.display().to_string();
    // A document inside a site project may legitimately refer across its pages, so
    // resolve what the project defines before calling anything broken: this path is the
    // editor's every-keystroke validator (and `check <file.tmd>`), and it used to report
    // every valid cross-page `@sec-`/`@fig-`/`@tbl-` as an error while `check <dir>` on
    // the same tree was clean. Outside a project the scan is empty and nothing changes.
    let elsewhere = taliesin_core::site::anchors_defined_elsewhere_in_project(path);
    let xref = taliesin_core::cite::validate_xrefs_known_elsewhere(&doc.blocks, &elsewhere);
    let page = site.and_then(|s| s.page_for_input(path));
    let scope_kind = if page.is_some() {
        Scope::InSite
    } else {
        Scope::Standalone
    };
    let statics = page_static_diagnostics(src, &doc.blocks, base, doc.format, scope_kind);
    let mut out: Vec<Diagnostic> = Vec::new();
    // Malformed YAML front matter: the lenient line-parser silently mis-extracts
    // fields, so surface the parse error here too (the live servers already do).
    if let Some((message, line)) = taliesin_core::frontmatter::yaml_error(src) {
        out.push(Diagnostic::new(path_str.clone(), Some(line), message));
    }
    // From the BUFFER, not from `page.input`: the file on disk is a different document as
    // soon as the author types, and the squiggle has to describe what is on screen.
    let cross: Vec<taliesin_core::render::Warning> = match (site, page) {
        (Some(site), Some(page)) => site.validate_cross_page_links_for_src(&page.rel, src),
        _ => Vec::new(),
    };
    out.extend(
        doc.warnings
            .iter()
            .chain(xref.iter())
            .chain(statics.iter())
            .chain(cross.iter())
            .map(|w| diag_from(w, &path_str)),
    );
    Ok(out)
}

/// The project enclosing `path`, discovered now.
///
/// Whether `path` is a *page* of it is a separate question, settled downstream in
/// [`collect_file_diagnostics_in_site`] so that one place decides it: a deck and a
/// `draft: true` chapter both sit inside a project and are both linted standalone.
///
/// `DraftMode::Exclude`, matching `check <dir>` rather than the preview: this is the seam
/// whose whole claim is "the same validators as `check`".
///
/// Discovery costs a full walk, and it is paid per call. Measured end to end on
/// `docs/guide`: `check <file.tmd>` goes from **14 ms to 213 ms**, against 387 ms for
/// `check <dir>`. That is the price of the two commands giving the same answer about the
/// same page, and it is worth it on a one-shot command — but it is far too slow to pay per
/// keystroke, so the language server passes a stat-validated `lsp_project::SiteCache`
/// instead of calling this. A file outside any project still costs nothing (11 ms).
fn enclosing_site_of(path: &Path) -> Option<taliesin_core::Site> {
    let root = taliesin_core::site::enclosing_site_root_across_git(path.parent()?)?;
    Some(taliesin_core::Site::discover(&root))
}

/// Lint an in-memory editor buffer as if it were the file at `path`, returning the
/// diagnostics directly. Used by the `lsp` server on every `didOpen`/`didChange`. This is
/// the whole of what the retired `check --stdin` was for: the CLI grew that flag before
/// `taliesin lsp` existed, and once the LSP owned on-type diagnostics nothing invoked it. The buffer path can't fail to render,
/// but a hypothetical error surfaces as one line-1 diagnostic (parity with the
/// companion's check-error handling) rather than vanishing.
///
/// `site` is the enclosing project or `None`; see [`collect_file_diagnostics_in_site`].
pub(crate) fn buffer_diagnostics_in_site(
    path: &Path,
    src: &str,
    site: Option<&taliesin_core::Site>,
) -> Vec<Diagnostic> {
    match collect_file_diagnostics_in_site(path, src, None, site) {
        Ok(diags) => diags,
        Err(e) => vec![Diagnostic::new(path.display().to_string(), Some(1), e)],
    }
}

/// What a check deliberately did not look at. Filled in by whichever `collect_*` ran, so
/// the facts come from the discovery that already happened — a second `Site::discover`
/// just to learn them was measured at **+50 to +83 ms** (~20% of a whole check) on the
/// three largest projects in the tree, which is far too much for a line most projects
/// never print.
#[derive(Default)]
pub(crate) struct CheckScope {
    /// `draft: true` pages held out of the published set, and so out of this check.
    pub excluded_drafts: Vec<String>,
    /// Executable languages (`python`/`r`) seen anywhere in the checked target, in
    /// first-seen order — the input to the Environment report.
    ///
    /// Recorded **from the walk the diagnostics already did**, which is the whole reason item
    /// 122's cost objection evaporated: `collect_environment` used to re-render every page of
    /// a site purely to learn this, measured at **+50%** on a site check. The rendered block
    /// model is right there in `collect_site_diagnostics`; reading two booleans off it is free.
    ///
    /// A site's DECKS contribute here too, and deliberately: a deck is built and served but
    /// held out of `site.pages`, so the old page-only walk reported an empty environment for a
    /// project whose only code cells live in a talk. Same class as item 109.
    pub used_languages: Vec<&'static str>,
    /// The project's `_site.yml` `python:` / `r:` pins, so resolution needs no second
    /// `Site::discover`. `None` for a single file, which has no project to pin them.
    pub python_pin: Option<String>,
    pub r_pin: Option<String>,
}

impl CheckScope {
    /// Merge one page's languages in, preserving first-seen order and skipping duplicates.
    fn note_languages(&mut self, blocks: &[taliesin_core::Block]) {
        for lang in used_languages(blocks) {
            if !self.used_languages.contains(&lang) {
                self.used_languages.push(lang);
            }
        }
    }
}

/// The one-line "here is what I did **not** look at" note for a site check, or `None` when
/// nothing was held back.
///
/// A `check` that reports nothing is read as "this project is clean", so every deliberate
/// omission has to be visible or the verdict is wider than the work. Item 109 was the
/// expensive version of this lesson: a site check silently skipped decks, and a deck with
/// six real defects reported "no problems found", exit 0.
///
/// **Drafts are deliberately excluded, and that is the ruling, not an oversight.** A
/// `draft: true` page is not in the published set, so linting it would report defects in
/// something that does not ship, and the live preview (`DraftMode::Include`) already lints
/// it where the author is writing it. What was wrong was doing that *silently* — `build`
/// has always said `N drafts not published`, and `check` said nothing at all.
///
/// Pure, so the wording is unit-testable without a filesystem.
fn scope_note(excluded_drafts: &[String]) -> Option<String> {
    if excluded_drafts.is_empty() {
        return None;
    }
    let n = excluded_drafts.len();
    Some(format!(
        "not checked: {n} draft{} ({}) — `draft: true` pages are not published, so they are \
         not linted here; the live preview lints them as you write",
        if n == 1 { "" } else { "s" },
        excluded_drafts.join(", ")
    ))
}

fn collect_site_diagnostics(
    root: &Path,
    scope: &mut CheckScope,
) -> Result<Vec<Diagnostic>, String> {
    let site = taliesin_core::Site::discover(root);
    // Free: this discovery already ran, and it is the only thing that knows what it dropped —
    // or which interpreters the project pinned.
    scope.excluded_drafts = site.excluded_drafts.clone();
    scope.python_pin = site.config.python.clone();
    scope.r_pin = site.config.r.clone();
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
    // The project's own policies, bound once: `check` must render each page the way the
    // build and the preview do, or it reports diagnostics for a document nobody ships (a
    // page inheriting the project `bibliography:` looked uncited here).
    let defaults = site.render_defaults();
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
        // Scope a numbered book chapter's floats + theorems to its chapter ("Theorem 2.3"),
        // matching the build + live-preview paths, so `check` reports the same numbers a
        // reader sees.
        let doc = taliesin_core::render_document_scoped_with_site(
            &src,
            base,
            site.chapter_for(page),
            Some(&defaults),
        );
        // Free: this page is already rendered, so the Environment report costs no second walk.
        scope.note_languages(&doc.blocks);
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
    // No `mounts:` finding here any more. It used to report TAL-MOUNT-PREVIEW — a mount is
    // preview-only, so every link into it 404s in the deploy (item 149) — and that stopped
    // being true when `build` learned to build the mounts too. A diagnostic that names a
    // defect the tool no longer has is worse than none: it sends the author to write the
    // shell script the build already replaced.

    // Cross-page relative-link + anchor existence, resolved against the site page
    // registry (file links here, not the single-doc `validate_local_links`: a `.tmd`
    // link rewrites to its built `.html` and only the registry knows the real urls).
    for (page_rel, w) in site.validate_cross_page_links() {
        out.push(diag_from(&w, &page_rel));
    }
    // Hygiene for the project-wide `bibliography:`, reported against `_site.yml` because
    // that is where it is declared. Unused-entry is site-wide by necessity: a shared entry
    // one page cites is used, however many pages leave it alone.
    for w in site.validate_shared_bibliography() {
        out.push(Diagnostic::new("_site.yml".to_string(), None, w.message));
    }
    // An `{{< embed >}}`-referenced deck is BUILT and SERVED but deliberately kept out of
    // `site.pages` so it stays out of nav + search (`site/mod.rs`'s `pages.retain`). Every
    // static validator walked `site.pages`, so a deck in a site reached **none** of them:
    // measured, a site whose deck carried two missing assets, a broken link, an alt-less
    // `<img>`, an unnamed link and malformed `$$` gave "no problems found", exit 0, while
    // `check talk.tmd` reported all six (item 109). The asymmetry mattered more than its
    // severity suggests, because a deck's defects are otherwise found by an *audience* —
    // the latest and most expensive point in the stream (item 132).
    //
    // `Scope::Standalone` is the honest scope: a deck is not a page of the site, so
    // cross-page xref resolution does not apply to it — it is checked as the standalone
    // document it is served as, which is also what `check talk.tmd` does.
    for deck in &site.decks {
        let rel = deck
            .input
            .strip_prefix(root)
            .unwrap_or(&deck.input)
            .display()
            .to_string();
        // A deck's languages count toward the Environment report: it is built and served, so
        // its cells run, and holding it out of `site.pages` must not also hold it out of the
        // report of what this project needs installed.
        match collect_file_diagnostics(&deck.input, Some(scope)) {
            // Report the deck by its site-relative path, like every page above it, rather
            // than the absolute path the single-file path uses.
            Ok(diags) => out.extend(diags.into_iter().map(|mut d| {
                d.file = rel.clone();
                d
            })),
            Err(e) => out.push(Diagnostic::new(rel, None, e)),
        }
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
    /// `Some(true/false)`: the interpreter binary spawned + returned a version (it exists
    /// and runs); `Some(false)` means the binary itself is absent/broken and
    /// `kernel_pkg_ok` is moot. **`None` means no probe was run**, so runnability is
    /// unknown — see `not_probed` (item 81).
    runs: Option<bool>,
    /// `ipykernel` (python) / `IRkernel` (r).
    kernel_pkg: &'static str,
    kernel_pkg_ok: Option<bool>,
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// Why this interpreter was resolved but not spawned, and how to ask for a probe.
    /// Absent when it was probed.
    #[serde(skip_serializing_if = "Option::is_none")]
    not_probed: Option<String>,
    /// What the upward `.venv` walk examined and where it stopped, e.g.
    /// `searched /a/b, /a; stopped at /a (.git)`. Python only (R performs no walk), and
    /// present whether or not the walk won — a *successful-looking* wrong pick is exactly
    /// the case where "which venv did you consider?" needs answering.
    #[serde(skip_serializing_if = "Option::is_none")]
    venv_search: Option<String>,
}

impl EnvEntry {
    /// Whether this entry reports a kernel that is *known* not ready.
    ///
    /// An unprobed entry (item 81) is **unknown**, not degraded: nothing spawned that
    /// binary, so printing "interpreter not found or failed to run" for it would be the
    /// same class of misreport as the promise item 79 fixes.
    fn known_not_ready(&self) -> bool {
        self.runs == Some(false) || self.kernel_pkg_ok == Some(false)
    }

    /// Whether a probe confirmed a ready kernel. `--require-kernel` needs this positive
    /// form: "not confirmed ready" must fail the gate, whether the probe said no or never ran.
    fn confirmed_ready(&self) -> bool {
        self.runs == Some(true) && self.kernel_pkg_ok == Some(true)
    }
}

/// Which executable languages (`python`/`r`) a document actually uses, in first-seen
/// order. Scans the rendered block model's cells (so `{{< include >}}`d cells count),
/// stopping once both are seen.
fn used_languages(blocks: &[taliesin_core::Block]) -> Vec<&'static str> {
    let mut seen = Vec::new();
    for c in blocks.iter().flat_map(|b| b.cells()) {
        if let Some(lang) = crate::exec::kernel_lang(&c.lang)
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

/// Whether an [`env_entry`] may SPAWN the interpreter it resolved.
///
/// The three states exist because "report the environment" and "run something" are separate
/// decisions that two booleans kept conflating. Item 122 is the case that forced them apart:
/// naming the interpreter a document would use costs nothing and is what a user needs when a
/// cell cannot run, while spawning it on every keystroke is what PL14 removed.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ProbePolicy {
    /// Resolve and report; spawn nothing. The default human `check` — a static linter that
    /// says which interpreter *would* be used and states plainly that it did not run it.
    Never,
    /// Spawn, except an interpreter the checked *project* chose (item 81). `--format json`.
    UnlessProjectSupplied,
    /// Spawn whatever was resolved: the user asked with `--require-kernel`.
    Always,
}

/// Build one `EnvEntry` for `lang` given the resolved interpreter, spawning it only when
/// `policy` allows (item 81 for the project-supplied case, item 122 for the default case).
///
/// [`ProbePolicy::UnlessProjectSupplied`] is item 81's rule: a *project-supplied* interpreter
/// — a `_site.yml` `python:`/`r:` field, or the project's own `.venv` — is reported but never
/// spawned, because `check` is the kernel-free pass an agent runs first on a project it has
/// not read, and `Command::new(bin)` on a string that project's author wrote is execution the
/// user did not ask for. Resolution, path and provenance are unchanged under every policy, so
/// the report always says exactly which interpreter *would* be used.
fn env_entry(
    lang: &'static str,
    resolved: &crate::interpreter::Resolved,
    policy: ProbePolicy,
) -> EnvEntry {
    let lang_enum = if lang == "r" {
        crate::interpreter::Lang::R
    } else {
        crate::interpreter::Lang::Python
    };
    let kernel_pkg = if lang == "r" { "IRkernel" } else { "ipykernel" };
    let base = EnvEntry {
        lang,
        path: resolved.path.display().to_string(),
        provenance: resolved.provenance.label(lang_enum).to_string(),
        runs: None,
        kernel_pkg,
        kernel_pkg_ok: None,
        version: None,
        error: None,
        not_probed: None,
        venv_search: resolved.trail.ancestor.as_ref().map(|v| v.summary()),
    };
    if policy == ProbePolicy::Never {
        return EnvEntry {
            not_probed: Some(
                "not probed: `check` is a static linter and does not spawn interpreters. \
                 Run `taliesin doctor`, or pass --require-kernel, to probe it"
                    .to_string(),
            ),
            ..base
        };
    }
    if resolved.provenance.is_project_supplied() && policy != ProbePolicy::Always {
        return EnvEntry {
            not_probed: Some(format!(
                "not probed: this interpreter was chosen by the project ({}), and `check` \
                 does not run a project-supplied binary. Pass --require-kernel, or run \
                 `taliesin doctor`, to probe it",
                resolved.provenance.label(lang_enum)
            )),
            ..base
        };
    }
    let p = crate::interpreter::probe(resolved, lang_enum);
    EnvEntry {
        runs: Some(p.runs),
        kernel_pkg_ok: Some(p.kernel_pkg_ok),
        version: p.version,
        error: p.error,
        ..base
    }
}

/// The informational Environment section for a file or site: for each executable
/// language the target uses, the resolved interpreter and — when `policy` allows the spawn
/// — its kernel-package probe. Never affects `check`'s exit code.
///
/// Everything this needs was learned by the diagnostics walk and handed over in `scope`:
/// which languages appear, and the project's `python:`/`r:` pins. It renders nothing itself.
/// That is deliberate and load-bearing — the earlier version re-rendered every page of a site
/// to recover the language list, which is the **+50%** that made item 122 look expensive.
/// Empty when the target has no python/r cells.
fn collect_environment(path: &Path, scope: &CheckScope, policy: ProbePolicy) -> Vec<EnvEntry> {
    // Interpreter resolution is relative to the *project*: the directory itself for a site,
    // the containing directory for a single file (matching what `exec` will do at run time).
    let project_dir = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or_else(|| Path::new("."))
    };
    scope
        .used_languages
        .iter()
        .map(|&lang| {
            let resolved = if lang == "r" {
                crate::interpreter::resolve_r(scope.r_pin.as_deref(), project_dir)
            } else {
                crate::interpreter::resolve_python(scope.python_pin.as_deref(), project_dir)
            };
            env_entry(lang, &resolved, policy)
        })
        .collect()
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
pub(crate) fn json_error(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// Produce `check`'s `--format json` payload for `target` (a file or a site dir): the exact
/// `{diagnostics, environment}` object (or a `{"error": …}` envelope on failure), so the MCP
/// `check` tool and the CLI can't drift. Mirrors `cmd_check`'s json branch.
pub(crate) fn check_json(target: &Path) -> String {
    let mut scope = CheckScope::default();
    let collected = crate::serve::guarded(|| collect_diagnostics(target, &mut scope))
        .map_err(|panic| format!("render panicked on {}: {panic}", target.display()))
        .and_then(|r| r);
    match collected {
        // `UnlessProjectSupplied`: this is the MCP `check` tool's path, described to the agent
        // only as "Validate". An agent pointed at an unknown project has made no choice to run
        // anything in it, so a project-supplied interpreter is reported, not spawned
        // (item 81). There is no opt-in here by design — an agent that wants a live probe
        // has `doctor`.
        Ok(diags) => format_json(
            &diags,
            &collect_environment(target, &scope, ProbePolicy::UnlessProjectSupplied),
        ),
        Err(e) => json_error(&e),
    }
}

/// The path to print for one diagnostic: the one the reader would type from the directory
/// they ran the command in.
///
/// A site's diagnostics are located against the **project root** (`sub/page.tmd`), which names
/// nothing from anywhere else: `taliesin check docs/guide` run from the repo printed
/// `sub/page.tmd:5:`, a path no terminal can open and no editor can resolve. Re-rooting it on
/// the target *as typed* gives `docs/guide/sub/page.tmd:5:` — the tsc/cargo convention, and
/// what a problem-matcher resolving against the invocation directory needs.
///
/// `root` is `None` for the single-file path, which already reports the path as
/// given; re-rooting it onto its own directory would double the prefix. `check .` keeps
/// printing a bare `sub/page.tmd`, so every existing grep of that output still matches.
///
/// The JSON format is deliberately untouched: its consumer passed the root itself and resolves
/// against it (`editor/vscode/src/checkstatus.ts` does exactly that), so a path relative to the
/// project is the right contract there.
fn displayed_path(file: &str, root: Option<&Path>) -> String {
    let Some(root) = root else {
        return file.to_string();
    };
    let joined = root.join(file);
    // Component-wise, not a string strip, so this is right on every separator.
    let clean = joined.strip_prefix(".").unwrap_or(&joined);
    clean.display().to_string()
}

/// Which root the human lines should be printed against: the target as typed when it is a
/// project directory, nothing when it is a single file.
///
/// Split out so the wiring is pinned by a test. Inlining the `is_dir()` at the call site put
/// the decision somewhere no test could reach, which is how the printed path could go back to
/// being project-relative with the formatter's own tests still green.
fn human_root(target: &Path) -> Option<&Path> {
    target.is_dir().then_some(target)
}

/// Greppable linter lines that also surface the DX6 machinery the JSON path already carries:
/// the `severity` word and the stable `TAL-*` code. The `file:line:` prefix stays first (the
/// linter convention VS Code problem-matchers / gcc / tsc key off), with `severity[CODE]:`
/// inserted before the message (the gcc/clang shape). The `docs_url` is JSON-only; it never
/// leaks here — the code + `--explain` footer are the human path back to the catalog.
///
/// `root` re-roots each path onto the target as typed; see [`displayed_path`].
fn format_human(diags: &[Diagnostic], color: bool, root: Option<&Path>) -> String {
    let mut s = String::new();
    for d in diags {
        // Paint just the severity word (rustc/cargo/tsc all colorize severity). `color` is
        // TTY-gated by the caller, so the non-TTY greppable contract stays byte-identical —
        // the `file:line: severity[CODE]:` shape a problem-matcher keys off is untouched.
        let sev = if color {
            let code = match d.severity {
                "error" => "\x1b[31m",   // red
                "warning" => "\x1b[33m", // yellow
                _ => "\x1b[90m",         // grey (info)
            };
            format!("{code}{}\x1b[0m", d.severity)
        } else {
            d.severity.to_string()
        };
        let file = displayed_path(&d.file, root);
        match d.line {
            Some(l) => s.push_str(&format!(
                "{}:{}: {}[{}]: {}\n",
                file, l, sev, d.code, d.message
            )),
            None => s.push_str(&format!("{}: {}[{}]: {}\n", file, sev, d.code, d.message)),
        }
    }
    s
}

/// The human summary line + `--explain` footer printed after the per-diagnostic block. Split
/// the bare `N problem(s)` into a per-severity breakdown (`(1 error, 2 warnings)`) — keeping
/// the leading `N problem(s)` token so existing greps still match — and, when anything is
/// reported, teach the `--explain` command (rustc's "For more information…"). Every line above
/// shows a concrete `[CODE]` to substitute. Pure, so the split + footer are unit-testable.
fn human_summary(diags: &[Diagnostic]) -> String {
    use taliesin_core::diagnostics::codes;
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    if diags.is_empty() {
        return "no problems found\n".to_string();
    }
    let errors = diags.iter().filter(|d| d.severity == codes::ERROR).count();
    let warnings = diags
        .iter()
        .filter(|d| d.severity == codes::WARNING)
        .count();
    let suggestions = diags
        .iter()
        .filter(|d| d.severity == codes::SUGGESTION)
        .count();
    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(format!("{errors} error{}", plural(errors)));
    }
    if warnings > 0 {
        parts.push(format!("{warnings} warning{}", plural(warnings)));
    }
    if suggestions > 0 {
        parts.push(format!("{suggestions} suggestion{}", plural(suggestions)));
    }
    let n = diags.len();
    // Don't call advice a "problem". When nothing above `suggestion` fired, the run passed;
    // saying "3 problems" and then exiting 0 reads like the exit code is broken.
    let mut s = if errors + warnings == 0 && suggestions > 0 {
        format!(
            "{suggestions} suggestion{} (advice; nothing here fails the run)",
            plural(suggestions)
        )
    } else {
        let mut s = format!("{n} problem{}", plural(n));
        if !parts.is_empty() {
            s.push_str(&format!(" ({})", parts.join(", ")));
        }
        s
    };
    s.push('\n');
    s.push_str(
        "\nFor more information about a diagnostic, try `taliesin check --explain <CODE>`.\n",
    );
    s
}

/// Render `check --explain`: expand one diagnostic code into cause + canonical fix
/// (rustc `--explain` style), or list every code when `code` is `None` (an index). `Ok`
/// text goes to stdout and exits 0; `Err(message)` is an unknown code, which the caller
/// routes through the same human/json error split as a render failure (non-zero exit).
///
/// `format` is `"human"` or `"json"` (already validated). The catalog + the `docs_url`
/// both come from `taliesin_core::diagnostics::codes`, so the offline `--explain` text and
/// the JSON `docs_url` on every diagnostic never disagree.
fn explain_output(code: Option<&str>, format: &str) -> Result<String, String> {
    use taliesin_core::diagnostics::codes;
    match code {
        // Expand one code.
        Some(c) => {
            let Some(e) = codes::explain(c) else {
                let all = codes::all_codes();
                let hint = match taliesin_core::closest(&c.to_ascii_uppercase(), &all) {
                    Some(near) => format!("unknown diagnostic code `{c}` (did you mean `{near}`?)"),
                    None => format!("unknown diagnostic code `{c}`"),
                };
                return Err(format!(
                    "{hint}\nrun `taliesin check --explain` to list every code"
                ));
            };
            let url = codes::docs_url(e.code);
            if format == "json" {
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "code": e.code,
                    "title": e.title,
                    "cause": e.cause,
                    "fix": e.fix,
                    "docs_url": url,
                }))
                .unwrap_or_else(|_| "{}".to_string()))
            } else {
                Ok(format!(
                    "{}: {}\n\n{}\n\nTo fix: {}\n\nLearn more: {url}\n",
                    e.code, e.title, e.cause, e.fix
                ))
            }
        }
        // Index: list every code.
        None => {
            let all = codes::all_codes();
            if format == "json" {
                let codes: Vec<_> = all
                    .iter()
                    .filter_map(|c| codes::explain(c))
                    .map(|e| {
                        serde_json::json!({
                            "code": e.code,
                            "title": e.title,
                            "docs_url": codes::docs_url(e.code),
                        })
                    })
                    .collect();
                Ok(
                    serde_json::to_string_pretty(&serde_json::json!({ "codes": codes }))
                        .unwrap_or_else(|_| "{\"codes\":[]}".to_string()),
                )
            } else {
                let mut s = String::new();
                for c in &all {
                    if let Some(e) = codes::explain(c) {
                        s.push_str(&format!("{:<18} {}\n", e.code, e.title));
                    }
                }
                s.push_str("\nRun `taliesin check --explain <CODE>` for the cause + fix.\n");
                Ok(s)
            }
        }
    }
}

/// Every long flag `check` accepts (drives the unknown-flag did-you-mean).
const CHECK_FLAGS: &[&str] = &[
    "--format",
    "--json",
    "--explain",
    "--errors-only",
    "--strict",
    "--require-kernel",
];

/// `taliesin check <file|dir> [--format human|json]`: render in memory, list every
/// located diagnostic, and exit non-zero if any are found (a CI gate). Static-only
/// (no code execution).
pub(crate) fn cmd_check(args: &[String]) -> ExitCode {
    let mut path: Option<&str> = None;
    let mut format = "human";
    let mut explain = false;
    let mut explain_code: Option<&str> = None;
    // DX18 exit-gating knobs. `--errors-only` narrows the floor to errors (dropping warnings
    // from the output too); `--strict` widens it to include advice; `--require-kernel`
    // promotes a missing kernel for a used language from informational to a failure.
    let mut floor = Floor::Warnings;
    let mut require_kernel = false;
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
            "--errors-only" => floor = Floor::Errors,
            "--strict" => floor = Floor::All,
            "--require-kernel" => require_kernel = true,
            // `--explain [CODE]`: expand a diagnostic code, not lint a file. Consume the next
            // token as the code only when it isn't itself a flag, so `--explain --format json`
            // is the index in JSON (code = None), not "code = `--format`".
            "--explain" => {
                explain = true;
                if let Some(next) = it.clone().next()
                    && !next.starts_with('-')
                {
                    explain_code = Some(next.as_str());
                    it.next();
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
    if format != "human" && format != "json" {
        log::error(&crate::serve::bad_format_error(Some(format)));
        return ExitCode::FAILURE;
    }
    // `--explain` is a code lookup, not a lint: print cause + fix (or the code index) and
    // exit, needing no path. A positional path, if also given, is ignored (as with rustc).
    if explain {
        return match explain_output(explain_code, format) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            // Unknown code: mirror the render-error split so `--format json | jq` stays valid.
            Err(e) => {
                if format == "json" {
                    println!("{}", json_error(&e));
                } else {
                    log::error(&e);
                }
                ExitCode::FAILURE
            }
        };
    }
    let Some(path) = path else {
        return crate::usage_error("check");
    };
    let target = Path::new(path);
    // Filled in by the site path; stays empty for a single file, which has no published-set
    // notion to hold anything back from.
    let mut scope = CheckScope::default();
    // Guard the render: a panic in core rendering becomes a clean located error + non-zero
    // exit (routed through the same error path, so `--format json` stays valid) instead of
    // a raw abort that would crash a CI gate.
    let collected = crate::serve::guarded(|| collect_diagnostics(target, &mut scope))
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
    // WHICH interpreter the document would use is always reported; whether anything SPAWNS it
    // is the separate decision `ProbePolicy` carries (item 122). PL14 tied the two together and
    // so bought its "no spawn on every keystroke" win by going silent — a document whose only
    // code cell could not run printed "no problems found", exit 0, while `build` warned twice.
    let policy = if require_kernel {
        ProbePolicy::Always
    } else if format == "json" {
        // A bare `--format json` resolves a project-supplied interpreter without running it
        // (item 81); only `--require-kernel` opts into that spawn.
        ProbePolicy::UnlessProjectSupplied
    } else {
        // The default human run: name it, never spawn it.
        ProbePolicy::Never
    };
    let environment = collect_environment(target, &scope, policy);
    // `--errors-only` drops warnings from what is shown AND from the exit decision.
    let diags = at_severity_floor(diags, floor);
    let kernel_fail = kernel_gate_fails(&environment, require_kernel);
    if format == "json" {
        // JSON to stdout only, so it pipes cleanly.
        println!("{}", format_json(&diags, &environment));
    } else {
        // Greppable `path:line: severity[CODE]: message` lines to stderr (linter-style), then a
        // per-severity summary + the `--explain` footer (both in `human_summary`). The severity
        // word is colorized only at a TTY (and not under NO_COLOR), so piped/redirected output
        // stays byte-identical for problem-matchers.
        let color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        eprint!("{}", format_human(&diags, color, human_root(target)));
        eprint!("{}", human_summary(&diags));
        // What this run deliberately did not cover.
        if let Some(note) = scope_note(&scope.excluded_drafts) {
            eprintln!("{note}");
        }
        if policy == ProbePolicy::Never {
            // Item 122. The document runs code, so say what it would run it WITH and be
            // explicit that nothing was spawned. This is the line whose absence let `check`
            // answer "no problems found" for a document whose only cell cannot execute: the
            // interpreter is named, so a wrong `TALIESIN_PYTHON` or a stale `.venv` is visible
            // at a glance, and the verdict `check` has not earned is not asserted.
            //
            // No probe means no `runs`/`kernel_pkg_ok` to report, which is the honest shape —
            // reporting "ready" here would be the same class of misreport as item 79's.
            if !environment.is_empty() {
                eprintln!("\nEnvironment (not probed):");
                for e in &environment {
                    eprintln!("  {}: {} ({})", e.lang, e.path, e.provenance);
                    // Where the upward `.venv` walk looked and stopped. Without it, a
                    // resolution that skipped the venv the author can see is named but
                    // not explained.
                    if let Some(v) = &e.venv_search {
                        eprintln!("    .venv search: {v}");
                    }
                }
                eprintln!("run `taliesin doctor` to check these kernels are ready");
            }
        } else {
            // Under a probing policy (`--require-kernel`), surface just the DEGRADED languages
            // — an all-green probe is `doctor`'s business, not a linter's — then point at
            // `doctor` for the full audit.
            let degraded: Vec<&EnvEntry> =
                environment.iter().filter(|e| e.known_not_ready()).collect();
            if !degraded.is_empty() {
                eprintln!("\nEnvironment (kernels not ready):");
                for e in &degraded {
                    let pkg = if e.runs == Some(false) {
                        // The interpreter binary itself is absent/broken, so the kernel
                        // package is moot; name that instead of a misleading "pkg MISSING".
                        "interpreter not found or failed to run".to_string()
                    } else {
                        format!("{} MISSING", e.kernel_pkg)
                    };
                    eprintln!("  {}: {} ({}), {}", e.lang, e.path, e.provenance, pkg);
                    if let Some(v) = &e.venv_search {
                        eprintln!("    .venv search: {v}");
                    }
                }
                eprintln!("run `taliesin doctor` for the full environment audit");
            }
        }
        // Make the reason legible when `--require-kernel` is the *only* thing failing (0
        // diagnostics would otherwise print "no problems found" then exit non-zero).
        if kernel_fail {
            let unready: Vec<&str> = environment
                .iter()
                .filter(|e| !e.confirmed_ready())
                .map(|e| e.lang)
                .collect();
            eprintln!(
                "\n--require-kernel: no runnable kernel for {}",
                unready.join(", ")
            );
        }
    }
    // Fail on any reported diagnostic AT OR ABOVE the chosen severity floor (so advice is
    // printed and does not fail the run) OR, under `--require-kernel`, a used language whose
    // kernel isn't ready.
    if gating(&diags, floor).next().is_none() && !kernel_fail {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Which severities fail the run. Three states, because two could not express "print the
/// advice but do not fail on it": with only `--errors-only` and the default, a rule whose
/// whole point is to suggest a rewrite turned a green gate red, so the only way to keep CI
/// green was to leave the rule off. Printed output is unaffected except under
/// `--errors-only`, which has always narrowed what it shows as well.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Floor {
    /// `--errors-only`: report and fail on errors alone.
    Errors,
    /// The default: report everything, fail on errors and warnings (never on advice).
    Warnings,
    /// `--strict`: report everything and fail on all of it, advice included.
    All,
}

impl Floor {
    /// The severity string this floor gates at (see `codes::gates_at`).
    fn severity(self) -> &'static str {
        use taliesin_core::diagnostics::codes;
        match self {
            Floor::Errors => codes::ERROR,
            Floor::Warnings => codes::WARNING,
            Floor::All => codes::SUGGESTION,
        }
    }
}

/// The diagnostics `check` **reports**. `--errors-only` drops warnings from the output as
/// well as the gate (its long-standing behaviour); every other floor shows everything,
/// because advice you cannot see is advice you cannot act on. Pure, so it is unit-testable.
fn at_severity_floor(diags: Vec<Diagnostic>, floor: Floor) -> Vec<Diagnostic> {
    if floor == Floor::Errors {
        diags
            .into_iter()
            .filter(|d| d.severity == taliesin_core::diagnostics::codes::ERROR)
            .collect()
    } else {
        diags
    }
}

/// The reported diagnostics that actually **fail** the run at this floor. Separate from
/// [`at_severity_floor`] on purpose: a suggestion is printed and does not gate, which is
/// the whole point of the third state.
fn gating(diags: &[Diagnostic], floor: Floor) -> impl Iterator<Item = &Diagnostic> {
    let at = floor.severity();
    diags
        .iter()
        .filter(move |d| taliesin_core::diagnostics::codes::gates_at(d.severity, at))
}

/// Whether `--require-kernel` should fail the run: it is set AND some used language's
/// interpreter is absent/broken or its Jupyter kernel package isn't importable. Off by
/// default, so kernel readiness stays informational and a Python-less CI box can still lint.
fn kernel_gate_fails(environment: &[EnvEntry], require_kernel: bool) -> bool {
    // Positive form on purpose: the gate asks "is a kernel confirmed ready", so an entry
    // that was never probed fails it. Under `--require-kernel` every entry *is* probed
    // (item 81's skip is exactly what that flag opts out of), so this cannot silently
    // downgrade the gate to a pass — it can only refuse to guess.
    require_kernel && environment.iter().any(|e| !e.confirmed_ready())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A missing input file suggests a near-miss sibling — and only when it can honestly
    /// help. `(os error 2)` alone made the user re-read their own typo; the answer was one
    /// `read_dir` away and `closest` was already in the tree.
    #[test]
    fn cannot_read_suggests_a_near_miss_sibling_and_only_then() {
        let dir = std::env::temp_dir().join(format!("tali-cr-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("intro.tmd"), "x").unwrap();
        let missing = |name: &str| {
            std::fs::read_to_string(dir.join(name)).expect_err("the fixture must be absent")
        };

        // Transposed extension: within edit distance 2 of a real sibling, so suggest it —
        // re-joined onto the directory the caller typed, not as a bare filename.
        let msg = cannot_read(&dir.join("intro.tdm"), &missing("intro.tdm"));
        assert!(
            msg.contains(&format!(
                "did you mean `{}`",
                dir.join("intro.tmd").display()
            )),
            "near miss must be suggested with its directory: {msg}"
        );

        // Nothing close: no guess. A wrong suggestion is worse than none.
        let msg = cannot_read(&dir.join("zzzzzzzzzz.tmd"), &missing("zzzzzzzzzz.tmd"));
        assert!(
            !msg.contains("did you mean"),
            "no sibling is within distance 2, so nothing is offered: {msg}"
        );

        // A *different* io error is a different problem: a neighbouring filename is not the
        // fix for it, so the suggestion stays out of the way.
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let msg = cannot_read(&dir.join("intro.tdm"), &denied);
        assert!(
            !msg.contains("did you mean"),
            "only a NotFound earns a spelling suggestion: {msg}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// The tests care about diagnostics, not scope, so they keep the old one-argument
    /// spelling and discard the scope. Shadows the parent fn by name on purpose.
    fn collect_diagnostics(path: &Path) -> Result<Vec<Diagnostic>, String> {
        super::collect_diagnostics(path, &mut CheckScope::default())
    }
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn buffer_diagnostics_flags_a_front_matter_typo() {
        // A misspelled front-matter key: the static validator locates it with a column span.
        let src = "---\ntittle: Hi\n---\n\n# Body\n";
        let diags = buffer_diagnostics(Path::new("buf.tmd"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("tittle") || d.message.contains("title")),
            "expected a typo diagnostic, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    /// A temp site project: `_site.yml` plus `(relative name, contents)` files.
    fn tmp_project(tag: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tali-check-proj-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            fs::write(dir.join(name), body).unwrap();
        }
        dir
    }

    /// What the language server publishes for `path` with `src` in the buffer: the same
    /// two calls `lsp::publish` makes, cache included, so these tests exercise the real
    /// path rather than a convenience wrapper that only they use.
    fn buffer_diagnostics(path: &Path, src: &str) -> Vec<Diagnostic> {
        let mut sites = crate::lsp_project::SiteCache::new();
        let site = sites.get(path);
        super::buffer_diagnostics_in_site(path, src, site)
    }

    fn broken_xrefs(diags: &[Diagnostic]) -> Vec<&str> {
        diags
            .iter()
            .filter(|d| d.code == "TAL-XREF-UNDEF")
            .map(|d| d.message.as_str())
            .collect()
    }

    /// AN-6: the per-document path is what the editor's language server runs on every
    /// keystroke, and it has no page registry — so it reported every *valid* cross-page
    /// reference as a broken one while `check <dir>` on the same tree was clean. Both
    /// definition shapes have to be seen: a `{#sec-}` heading anchor (source-visible)
    /// and a `#| label:` cell anchor (inside a fence, which the anchor scan skips).
    #[test]
    fn a_valid_cross_page_reference_is_not_reported_as_broken() {
        let dir = tmp_project(
            "xpage-ok",
            &[
                ("_site.yml", "title: Project\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\n## A topic {#sec-topic}\n\n\
                     ```{python}\n#| label: fig-plot\n#| fig-cap: A plot\nplot()\n```\n",
                ),
                (
                    "other.tmd",
                    "---\ntitle: Other\n---\n\nSee @sec-topic and @fig-plot.\n",
                ),
            ],
        );
        let src = fs::read_to_string(dir.join("other.tmd")).unwrap();
        let diags = buffer_diagnostics(&dir.join("other.tmd"), &src);
        assert!(
            broken_xrefs(&diags).is_empty(),
            "a reference the project resolves must not be reported broken: {:?}",
            broken_xrefs(&diags)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// The other half, and the reason this is a scope fix rather than a severity
    /// downgrade: an anchor no page in the project defines is still an error, reported
    /// where the author can act on it.
    #[test]
    fn a_reference_no_page_defines_is_still_reported_as_broken() {
        let dir = tmp_project(
            "xpage-bad",
            &[
                ("_site.yml", "title: Project\n"),
                (
                    "index.tmd",
                    "---\ntitle: Home\n---\n\n## A topic {#sec-topic}\n",
                ),
                ("other.tmd", "---\ntitle: Other\n---\n\nSee @sec-ghost.\n"),
            ],
        );
        let src = fs::read_to_string(dir.join("other.tmd")).unwrap();
        let diags = buffer_diagnostics(&dir.join("other.tmd"), &src);
        assert!(
            broken_xrefs(&diags)
                .iter()
                .any(|m| m.contains("@sec-ghost")),
            "an anchor nothing defines is still broken: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// The editor's every-keystroke validator linted every buffer as a STANDALONE
    /// document, even one sitting inside a site — so the single editing surface got a
    /// strictly weaker diagnostic than the read-only preview, which has passed
    /// `Scope::InSite` and called the cross-page validator since DX1. Measured on this
    /// exact shape: `check <dir>` reported `TAL-LINK-ANCHOR`, the buffer path did not.
    #[test]
    fn a_broken_cross_page_anchor_is_reported_on_the_buffer_path() {
        let dir = tmp_project(
            "xlink-anchor",
            &[
                ("_site.yml", "title: Project\n"),
                (
                    "a.tmd",
                    "---\ntitle: A\n---\n\nSee [the target](b.html#the-target).\n",
                ),
                ("b.tmd", "---\ntitle: B\n---\n\n## Renamed Heading\n"),
            ],
        );
        let src = fs::read_to_string(dir.join("a.tmd")).unwrap();
        let diags = buffer_diagnostics(&dir.join("a.tmd"), &src);
        assert!(
            diags.iter().any(|d| d.code == "TAL-LINK-ANCHOR"),
            "an anchor no page in the project defines must reach the editor: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// The other half of the scope flip, and the reason it is not just "add a validator":
    /// `Scope::InSite` also *removes* `validate_local_links`, whose standalone phrasing
    /// ("no such file under the document directory") described a rule that does not apply
    /// to a site page. The site-aware rule reports the same link in the site's terms.
    #[test]
    fn a_link_to_no_page_is_reported_in_the_site_s_terms() {
        let dir = tmp_project(
            "xlink-site-phrasing",
            &[
                ("_site.yml", "title: Project\n"),
                ("a.tmd", "---\ntitle: A\n---\n\n[nowhere](nope.html)\n"),
                ("b.tmd", "---\ntitle: B\n---\n\nBody.\n"),
            ],
        );
        let src = fs::read_to_string(dir.join("a.tmd")).unwrap();
        let diags = buffer_diagnostics(&dir.join("a.tmd"), &src);
        let link: Vec<&str> = diags
            .iter()
            .filter(|d| d.code == "TAL-LINK")
            .map(|d| d.message.as_str())
            .collect();
        assert_eq!(link.len(), 1, "exactly one broken-link finding: {link:?}");
        assert!(
            link[0].contains("no page in this site"),
            "the site-aware phrasing, not the standalone one: {link:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// The buffer, not the file on disk. This is the whole point of the buffer path: an
    /// author fixing a broken link must see the squiggle clear before saving, and one
    /// typing a new link must see it appear. `page_link_facts` reads the page from disk,
    /// so a naive wiring of the site validator would answer about the last saved version.
    #[test]
    fn cross_page_links_are_judged_from_the_buffer_not_the_saved_file() {
        let dir = tmp_project(
            "xlink-unsaved",
            &[
                ("_site.yml", "title: Project\n"),
                ("a.tmd", "---\ntitle: A\n---\n\nNo links yet.\n"),
                (
                    "b.tmd",
                    "---\ntitle: B\n---\n\n## Real Heading {#sec-real}\n",
                ),
            ],
        );
        // Typed but not saved: the file on disk still has no links at all.
        let unsaved = "---\ntitle: A\n---\n\nSee [gone](b.html#sec-ghost).\n";
        let diags = buffer_diagnostics(&dir.join("a.tmd"), unsaved);
        assert!(
            diags.iter().any(|d| d.code == "TAL-LINK-ANCHOR"),
            "a link typed into the buffer must be judged: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );

        // And the reverse: a broken link on disk that the author has just deleted in the
        // buffer must stop being reported, or the squiggle outlives the defect.
        fs::write(
            dir.join("a.tmd"),
            "---\ntitle: A\n---\n\nSee [gone](b.html#sec-ghost).\n",
        )
        .unwrap();
        let fixed = "---\ntitle: A\n---\n\nSee [real](b.html#sec-real).\n";
        let after = buffer_diagnostics(&dir.join("a.tmd"), fixed);
        assert!(
            !after.iter().any(|d| d.code == "TAL-LINK-ANCHOR"),
            "a link fixed in the buffer must clear before saving: {:?}",
            after.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// The claim `docs/guide/reference/cli.tmd` makes about `taliesin lsp`, pinned: "the
    /// same validators as `check`, run on the unsaved buffer". Measured false before this —
    /// the buffer path missed `TAL-LINK-ANCHOR` entirely and described a broken link with a
    /// rule that does not apply to a site page — so it is asserted rather than restated.
    ///
    /// Compared as a SET OF CODES for one page, not message-for-message: `check <dir>`
    /// renders each page with the project's numbering and defaults, so a message may name
    /// "Figure 2.1" where the buffer path says "Figure 1". What must not differ is which
    /// defects each one finds.
    #[test]
    fn the_editor_finds_the_same_defects_on_a_page_as_check_does() {
        let dir = tmp_project(
            "parity",
            &[
                ("_site.yml", "title: Project\n"),
                (
                    "page.tmd",
                    "---\ntitle: P\ntitel: typo\n---\n\n\
                     A [missing page](ghost.html), a [bad anchor](other.html#nope), \
                     a [good anchor](other.html#sec-real).\n\n\
                     ![](missing.png)\n\n\
                     See @sec-ghost.\n",
                ),
                (
                    "other.tmd",
                    "---\ntitle: Other\n---\n\n## Real {#sec-real}\n",
                ),
            ],
        );
        let page = dir.join("page.tmd");
        let src = fs::read_to_string(&page).unwrap();

        let mut site_codes: Vec<String> = collect_diagnostics(&dir)
            .expect("site ok")
            .into_iter()
            .filter(|d| d.file.contains("page.tmd"))
            .map(|d| d.code.to_string())
            .collect();
        let mut buffer_codes: Vec<String> = buffer_diagnostics(&page, &src)
            .into_iter()
            .map(|d| d.code.to_string())
            .collect();
        site_codes.sort();
        site_codes.dedup();
        buffer_codes.sort();
        buffer_codes.dedup();

        assert!(
            site_codes.len() >= 4,
            "the fixture must exercise several validators, or parity is vacuous: {site_codes:?}"
        );
        assert_eq!(
            buffer_codes, site_codes,
            "the editor and `check` disagree about what is wrong with this page"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// A standalone `.tmd` with no ancestor `_site.yml` is unaffected: there is no
    /// project to resolve against, so its broken references stay broken.
    #[test]
    fn a_standalone_document_still_reports_its_broken_reference() {
        let dir = tmp_project("xpage-solo", &[("solo.tmd", "# Solo\n\nSee @fig-nope.\n")]);
        // `tmp_project` writes no `_site.yml` here, which is the point.
        let src = fs::read_to_string(dir.join("solo.tmd")).unwrap();
        let diags = buffer_diagnostics(&dir.join("solo.tmd"), &src);
        assert!(
            broken_xrefs(&diags).iter().any(|m| m.contains("@fig-nope")),
            "a standalone document has no project to excuse a broken ref: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn to_lsp_uses_a_precise_span_when_columned() {
        let d = super::Diagnostic {
            code: "TAL-FM-KEY",
            docs_url: "https://example.test/DIAGNOSTICS.md#tal-fm-key".to_string(),
            severity: "warning",
            file: "buf.tmd".to_string(),
            line: Some(2),
            col: Some(1),
            end_col: Some(7),
            message: "unknown key `tittle`".to_string(),
            suggestion: None,
        };
        let lines = ["---", "tittle: Hi", "---"];
        let lsp = d.to_lsp(&lines);
        // 1-based line 2 → 0-based 1; 1-based [1,7) → 0-based [0,6).
        assert_eq!(lsp.range.start, lsp_types::Position::new(1, 0));
        assert_eq!(lsp.range.end, lsp_types::Position::new(1, 6));
        assert_eq!(lsp.severity, Some(lsp_types::DiagnosticSeverity::WARNING));
        assert_eq!(
            lsp.code,
            Some(lsp_types::NumberOrString::String("TAL-FM-KEY".to_string()))
        );
        assert_eq!(lsp.source.as_deref(), Some("taliesin"));
        assert_eq!(
            lsp.code_description.map(|c| c.href.to_string()),
            Some("https://example.test/DIAGNOSTICS.md#tal-fm-key".to_string())
        );
    }

    #[test]
    fn to_lsp_columns_are_utf16_when_an_astral_char_precedes_the_token() {
        // The `check` validators produce 1-based *character* columns. LSP columns are UTF-16
        // code units, so an astral char (😀 = 2 UTF-16 units) before the token must shift the
        // emitted column by one extra unit. `😀tittle`: `tittle` is char cols [2,8), which is
        // UTF-16 cols [2,8) (0-based) once the emoji's second unit is counted.
        let d = super::Diagnostic {
            code: "TAL-FM-KEY",
            docs_url: "https://example.test/x".to_string(),
            severity: "warning",
            file: "buf.tmd".to_string(),
            line: Some(1),
            col: Some(2),
            end_col: Some(8),
            message: "unknown key `tittle`".to_string(),
            suggestion: None,
        };
        let lines = ["😀tittle: Hi"];
        let lsp = d.to_lsp(&lines);
        assert_eq!(lsp.range.start, lsp_types::Position::new(0, 2));
        assert_eq!(lsp.range.end, lsp_types::Position::new(0, 8));
    }

    #[test]
    fn to_lsp_whole_line_span_length_is_in_utf16_units() {
        // `😀 hello` is 7 scalars but 8 UTF-16 units; the uncolumned whole-line span must end
        // at the UTF-16 length, not the char count.
        let d = super::Diagnostic {
            code: "TAL-XREF-UNDEF",
            docs_url: "https://example.test/x".to_string(),
            severity: "error",
            file: "buf.tmd".to_string(),
            line: Some(1),
            col: None,
            end_col: None,
            message: "undefined".to_string(),
            suggestion: None,
        };
        let lines = ["😀 hello"];
        let lsp = d.to_lsp(&lines);
        assert_eq!(lsp.range.end, lsp_types::Position::new(0, 8));
    }

    #[test]
    fn to_lsp_carries_a_precise_fix_on_data_but_never_an_imprecise_one() {
        let base = super::Diagnostic {
            code: "TAL-FM-KEY",
            docs_url: "https://example.test/x".to_string(),
            severity: "warning",
            file: "buf.tmd".to_string(),
            line: Some(2),
            col: Some(1),
            end_col: Some(7),
            message: "unknown key `tittle` (did you mean `title`?)".to_string(),
            suggestion: Some(super::Suggestion {
                replacement: "title".to_string(),
            }),
        };
        let lines = ["---", "tittle: Hi", "---"];
        // Columned + suggestion → the fix rides on `data`.
        let with_span = base.to_lsp(&lines);
        assert_eq!(
            with_span.data,
            Some(serde_json::json!({ "replacement": "title" }))
        );
        // Same suggestion but no column → no fix (would be imprecise).
        let uncolumned = super::Diagnostic {
            col: None,
            end_col: None,
            ..base.clone()
        };
        assert_eq!(uncolumned.to_lsp(&lines).data, None);
        // No suggestion → no fix.
        let no_sugg = super::Diagnostic {
            suggestion: None,
            ..base.clone()
        };
        assert_eq!(no_sugg.to_lsp(&lines).data, None);
    }

    #[test]
    fn to_lsp_spans_the_whole_line_when_uncolumned() {
        let d = super::Diagnostic {
            code: "TAL-XREF-UNDEF",
            docs_url: "https://example.test/x".to_string(),
            severity: "error",
            file: "buf.tmd".to_string(),
            line: Some(3),
            col: None,
            end_col: None,
            message: "undefined @fig-x".to_string(),
            suggestion: None,
        };
        let lines = ["a", "bb", "hello world"]; // line 3 (0-based 2) has 11 chars
        let lsp = d.to_lsp(&lines);
        assert_eq!(lsp.range.start, lsp_types::Position::new(2, 0));
        assert_eq!(lsp.range.end, lsp_types::Position::new(2, 11));
        assert_eq!(lsp.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("tali-check-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn human_surfaces_code_and_severity_while_json_alone_carries_the_docs_url() {
        use taliesin_core::diagnostics::codes;
        // A broken xref -> TAL-XREF-UNDEF (severity error); its JSON diagnostic carries a
        // docs_url anchored by the lowercased code. PL1: the human line now surfaces the
        // severity word + the code so the reader can `--explain` it, but never the url (the
        // code + footer are the human path back to the catalog; the url stays JSON-only).
        let d = Diagnostic::new(
            "a.tmd".into(),
            Some(2),
            "broken cross-reference: @fig-x".into(),
        );
        let json = format_json(std::slice::from_ref(&d), &[]);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let url = parsed["diagnostics"][0]["docs_url"].as_str().unwrap_or("");
        assert!(
            url.starts_with(codes::DIAGNOSTICS_DOC_URL),
            "docs_url present: {json}"
        );
        assert!(
            url.ends_with("#tal-xref-undef"),
            "anchored by lowercased code: {json}"
        );
        let human = format_human(std::slice::from_ref(&d), false, None);
        assert!(!human.contains("http"), "no url in human output: {human}");
        assert!(
            human.contains("error[TAL-XREF-UNDEF]"),
            "human surfaces severity + code: {human}"
        );
    }

    #[test]
    fn explain_known_code_human_has_cause_fix_and_url() {
        let text = explain_output(Some("TAL-XREF-UNREF"), "human").expect("known code");
        assert!(text.starts_with("TAL-XREF-UNREF:"), "titled block: {text}");
        assert!(text.contains("To fix:"), "has a fix: {text}");
        assert!(
            text.contains("Learn more: https://github.com/AJBogo9/taliesin"),
            "has a docs url: {text}"
        );
    }

    #[test]
    fn explain_known_code_json_is_structured_and_case_insensitive() {
        // Lowercase input resolves; the JSON echoes the canonical uppercase code + the fields.
        let text = explain_output(Some("tal-fm-key"), "json").expect("known code");
        let v: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(v["code"], "TAL-FM-KEY");
        assert!(v["title"].is_string() && v["cause"].is_string() && v["fix"].is_string());
        assert!(
            v["docs_url"]
                .as_str()
                .unwrap_or("")
                .ends_with("#tal-fm-key"),
            "docs_url anchored: {text}"
        );
    }

    #[test]
    fn explain_unknown_code_is_err_with_did_you_mean() {
        // A near-miss of a real code draws a suggestion; the message names the bad input.
        let err = explain_output(Some("TAL-XREF-UNDEFF"), "human").expect_err("unknown");
        assert!(err.contains("TAL-XREF-UNDEFF"), "names the bad code: {err}");
        assert!(err.contains("did you mean"), "suggests a near-miss: {err}");
        assert!(err.contains("--explain"), "points at the index: {err}");
    }

    #[test]
    fn explain_no_code_lists_every_code() {
        use taliesin_core::diagnostics::codes;
        // Human index: one line per code, each naming the code.
        let human = explain_output(None, "human").expect("index");
        for c in codes::all_codes() {
            assert!(human.contains(c), "index lists {c}: {human}");
        }
        // JSON index: an array of {code,title,docs_url} of the full length.
        let json = explain_output(None, "json").expect("index");
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let arr = v["codes"].as_array().expect("codes array");
        assert_eq!(
            arr.len(),
            codes::all_codes().len(),
            "every code listed: {json}"
        );
        assert!(
            arr[0]["docs_url"].is_string(),
            "each carries a docs_url: {json}"
        );
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
        // Exactly one diagnostic per issue: the assembly must not duplicate a channel. A
        // doubled `.chain(xref.iter())` (or a validator run twice) emits the broken-xref
        // diagnostic twice; the `any()` existence checks above never counted, so a
        // duplication shipped unnoticed.
        assert_eq!(
            diags
                .iter()
                .filter(|d| d.message.contains("@fig-nope"))
                .count(),
            1,
            "the broken-xref diagnostic must appear exactly once, not be duplicated: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_editor_buffer_is_linted_instead_of_the_on_disk_file() {
        // E2: the buffer seam lints what the editor has, not the last-saved file, so
        // real-time on-type diagnostics see unsaved edits. The disk file is CLEAN (no typo);
        // the buffer carries the `titel:` typo. The diagnostic must come from the buffer,
        // and it must still locate to the real file path (for click-to-source) and resolve
        // the base dir from it (so relative includes/assets still work).
        //
        // This pins `collect_file_diagnostics_from_src`, which SURVIVED the `--stdin` cut:
        // `taliesin lsp` calls it through `buffer_diagnostics` on every didOpen/didChange.
        // The retired flag was one of two callers, and the test is named for the other now.
        let dir = tmp("check-buffer");
        let f = dir.join("doc.tmd");
        fs::write(&f, "---\ntitle: Clean\n---\n\nAll good on disk.\n").unwrap();
        let buffer = "---\ntitle: T\ntitel: oops\n---\n\nUnsaved buffer.\n";
        let diags = collect_file_diagnostics_from_src(&f, buffer, None).expect("ok");
        assert!(
            diags.iter().any(|d| d.message.contains("titel")),
            "the buffer's front-matter typo must be linted, not the clean disk file: {diags:?}"
        );
        assert!(
            diags.iter().all(|d| d.file.contains("doc.tmd")),
            "buffer diagnostics still locate to the real file path: {diags:?}"
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
            "is not a citation",
            "is recognized but not supported",
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
        let mut targets: Vec<std::path::PathBuf> = ["single-page-report", "demo-book", "tech-blog"]
            .iter()
            .map(|s| corpus.join(s))
            .collect();
        // everything else is a standalone doc; diagnostics/ is deliberately tripping (exempt).
        walk(
            &corpus,
            &[
                "diagnostics",
                "single-page-report",
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
                Some("error" | "warning" | "suggestion")
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

    /// Run the real two-step the CLI runs: the diagnostics walk fills `CheckScope`, then the
    /// Environment report reads it. Going through `collect_diagnostics` rather than calling
    /// `collect_environment` with a hand-built scope is the point — it pins the *handoff*,
    /// which is where item 122's whole cost argument lives. A walk that forgot to record its
    /// languages would leave these green if the scope were faked here.
    fn environment_for(path: &Path, policy: ProbePolicy) -> Vec<EnvEntry> {
        let mut scope = CheckScope::default();
        // `super::` on purpose: this module has a one-arg `collect_diagnostics` shim that
        // would shadow the real walk, and the real walk is exactly what fills the scope.
        super::collect_diagnostics(path, &mut scope).expect("target lints");
        collect_environment(path, &scope, policy)
    }

    #[test]
    fn environment_is_empty_for_a_doc_with_no_code_cells() {
        let dir = tmp("env-nocells");
        let f = dir.join("x.tmd");
        std::fs::write(&f, "# Title\n\nJust prose, no cells.\n").unwrap();
        assert!(
            environment_for(&f, ProbePolicy::Always).is_empty(),
            "a doc with no python/r cells reports no Environment entries"
        );
    }

    #[test]
    fn environment_lists_python_for_a_python_cell_doc() {
        let dir = tmp("env-pycell");
        let f = dir.join("x.tmd");
        std::fs::write(&f, "# T\n\n```{python}\nprint(1)\n```\n").unwrap();
        let env = environment_for(&f, ProbePolicy::Always);
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
    fn never_policy_resolves_the_interpreter_and_spawns_nothing() {
        // Item 122's core contract at unit level: the entry is fully populated with WHICH
        // interpreter would run (path + provenance) and carries no verdict about whether it
        // works. `runs`/`kernel_pkg_ok` staying `None` is what keeps `check` kernel-free.
        let dir = tmp("env-never");
        let f = dir.join("x.tmd");
        std::fs::write(&f, "# T\n\n```{python}\nprint(1)\n```\n").unwrap();
        let env = environment_for(&f, ProbePolicy::Never);
        assert_eq!(env.len(), 1);
        assert!(!env[0].path.is_empty(), "the interpreter is still named");
        assert_eq!(
            env[0].runs, None,
            "nothing was spawned, so nothing is known"
        );
        assert_eq!(env[0].kernel_pkg_ok, None);
        assert!(
            env[0].not_probed.is_some(),
            "and the report says so out loud"
        );
    }

    #[test]
    fn a_site_deck_contributes_its_language_to_the_environment() {
        // A deck is built and served but held out of `site.pages`, so the old page-only walk
        // reported an empty environment for a project whose only code cells live in a talk
        // (item 109's family). Sourcing languages from the diagnostics walk fixes it, and this
        // pins that it stays fixed.
        let dir = tmp("env-deck");
        std::fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
        std::fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\n---\n\nProse only.\n\n{{< embed talk.tmd >}}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("talk.tmd"),
            "---\ntitle: Talk\nformat: deck\n---\n\n## One\n\n```{python}\nprint(1)\n```\n",
        )
        .unwrap();
        let env = environment_for(&dir, ProbePolicy::Never);
        assert_eq!(
            env.iter().map(|e| e.lang).collect::<Vec<_>>(),
            vec!["python"],
            "the deck's python cell is reported even though the deck is not a page"
        );
    }

    #[test]
    fn format_human_lists_located_lines() {
        // Unmatched messages classify to (TAL-CHECK, error). PL1: the `file:line:` linter
        // prefix stays first, with `severity[CODE]:` before the message (located + unlocated).
        let diags = vec![
            Diagnostic::new("a.tmd".into(), Some(3), "m1".into()),
            Diagnostic::new("b.tmd".into(), None, "m2".into()),
        ];
        let text = format_human(&diags, false, None);
        assert!(
            text.contains("a.tmd:3: error[TAL-CHECK]: m1"),
            "located line carries severity + code: {text}"
        );
        assert!(
            text.contains("b.tmd: error[TAL-CHECK]: m2"),
            "unlocated line carries severity + code: {text}"
        );
    }

    #[test]
    fn a_site_check_prints_paths_relative_to_where_the_command_ran() {
        // A site's diagnostics are located against the PROJECT ROOT. Printed bare, they name
        // nothing: `taliesin check docs/guide` from the repo said `sub/page.tmd:5:`, which no
        // terminal can open and which an editor's problem-matcher resolves onto a file that
        // does not exist. Both the located and the unlocated line carry the prefix — the
        // unlocated form is how a `_site.yml` finding is reported, and dropping the prefix
        // from just that one is the shape this asserts against.
        let diags = vec![
            Diagnostic::new("sub/page.tmd".into(), Some(5), "m1".into()),
            Diagnostic::new("_site.yml".into(), None, "m2".into()),
        ];
        let text = format_human(&diags, false, Some(Path::new("docs/guide")));
        assert!(
            text.contains("docs/guide/sub/page.tmd:5: error[TAL-CHECK]: m1"),
            "located line names the path the reader would type: {text}"
        );
        assert!(
            text.contains("docs/guide/_site.yml: error[TAL-CHECK]: m2"),
            "unlocated line is re-rooted too: {text}"
        );
    }

    #[test]
    fn checking_the_current_directory_does_not_grow_a_dot_slash_prefix() {
        // `check .` is the in-project spelling, and there the project-relative path already IS
        // the path from the shell. `./sub/page.tmd` would be a gratuitous change to output
        // that greps and matchers already read.
        let diags = vec![Diagnostic::new("sub/page.tmd".into(), Some(5), "m".into())];
        let text = format_human(&diags, false, Some(Path::new(".")));
        assert!(
            text.starts_with("sub/page.tmd:5:"),
            "no ./ prefix on the current directory: {text}"
        );
    }

    #[test]
    fn a_single_file_check_leaves_the_path_exactly_as_typed() {
        // `collect_file_diagnostics_from_src` already reports `path.display()`, i.e. the path
        // as given. Re-rooting that onto its own directory would double the prefix.
        let diags = vec![Diagnostic::new(
            "docs/guide/index.tmd".into(),
            Some(2),
            "m".into(),
        )];
        let text = format_human(&diags, false, None);
        assert!(
            text.starts_with("docs/guide/index.tmd:2:"),
            "single-file path is untouched: {text}"
        );
    }

    #[test]
    fn only_a_directory_target_re_roots_its_paths() {
        // The wiring, not the formatter: `human_root` is what decides, and with this test
        // absent a `None` for every target would leave the formatter's own tests green while
        // the shipped output went back to naming files that do not exist.
        let dir = std::env::temp_dir().join(format!("tali-human-root-{}", std::process::id()));
        let file = dir.join("index.tmd");
        fs::create_dir_all(&dir).expect("temp dir");
        fs::write(&file, "---\ntitle: x\n---\n").expect("temp file");

        assert_eq!(
            human_root(&dir),
            Some(dir.as_path()),
            "a project directory re-roots its diagnostics onto the target as typed"
        );
        assert_eq!(
            human_root(&file),
            None,
            "a single file already reports the path as given"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_human_colorizes_only_when_asked_and_stays_greppable_plain() {
        // The non-TTY path must be byte-identical to the historical greppable line (no ANSI),
        // so a problem-matcher keeps working; the TTY path paints just the severity word.
        let diags = vec![Diagnostic::new("a.tmd".into(), Some(3), "m1".into())];
        let plain = format_human(&diags, false, None);
        assert!(
            !plain.contains('\x1b'),
            "plain output must carry no ANSI: {plain:?}"
        );
        assert!(plain.contains("a.tmd:3: error[TAL-CHECK]: m1"));
        let colored = format_human(&diags, true, None);
        assert!(
            colored.contains("\x1b[31merror\x1b[0m"),
            "severity must be painted: {colored:?}"
        );
        // Only the severity word is wrapped — the file:line prefix and code stay bare.
        assert!(colored.contains("a.tmd:3: \x1b[31merror\x1b[0m[TAL-CHECK]: m1"));
    }

    /// The scope note exists so "no problems found" cannot be read as "nothing was
    /// skipped". Nothing held back means no line at all — a check that covered everything
    /// should not spend a line saying so.
    #[test]
    fn the_scope_note_names_held_back_drafts_and_is_silent_when_there_are_none() {
        assert_eq!(scope_note(&[]), None, "nothing skipped ⇒ no line");

        let one = scope_note(&["wip.tmd".to_string()]).expect("a draft was held back");
        assert!(one.starts_with("not checked: 1 draft ("), "singular: {one}");
        assert!(one.contains("wip.tmd"), "names the file: {one}");
        // The *reason* is the load-bearing half: without it the line reads as a defect
        // report rather than a deliberate exclusion the author chose with `draft: true`.
        assert!(one.contains("not published"), "states why: {one}");

        let two = scope_note(&["a.tmd".to_string(), "posts/b/index.tmd".to_string()])
            .expect("two drafts were held back");
        assert!(two.starts_with("not checked: 2 drafts ("), "plural: {two}");
        assert!(
            two.contains("a.tmd") && two.contains("posts/b/index.tmd"),
            "names every file, not just a count: {two}"
        );
    }

    #[test]
    fn human_summary_splits_by_severity_and_points_at_explain() {
        // Mixed set: a broken xref (error) + an unknown front-matter key (warning). The summary
        // keeps the leading `N problem(s)` token, breaks it out per severity, and prints the
        // `--explain` footer so the DX6 catalog is reachable from human output.
        let diags = vec![
            Diagnostic::new(
                "a.tmd".into(),
                Some(2),
                "broken cross-reference: @fig-x".into(),
            ),
            Diagnostic::new(
                "a.tmd".into(),
                Some(1),
                "unknown front-matter key: pyton".into(),
            ),
        ];
        let s = human_summary(&diags);
        assert!(s.contains("2 problems"), "leading count kept: {s}");
        assert!(
            s.contains("(1 error, 1 warning)"),
            "per-severity breakdown: {s}"
        );
        assert!(
            s.contains("taliesin check --explain <CODE>"),
            "teaches --explain: {s}"
        );
        // A single warning pluralizes correctly and still shows the footer.
        let one = human_summary(&[Diagnostic::new(
            "a.tmd".into(),
            Some(1),
            "unknown front-matter key: pyton".into(),
        )]);
        assert!(one.contains("1 problem (1 warning)"), "singular: {one}");
        // No diagnostics: the clean line, and NO --explain footer (nothing to explain).
        let none = human_summary(&[]);
        assert_eq!(none, "no problems found\n");
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

    // --- DX18 exit-gating ---

    fn error_diag() -> Diagnostic {
        // Classifies as TAL-XREF-UNDEF / ERROR.
        Diagnostic::new(
            "a.tmd".into(),
            Some(1),
            "broken cross-reference: @fig-x".into(),
        )
    }
    fn warning_diag() -> Diagnostic {
        // Classifies as TAL-FM-KEY / WARNING.
        Diagnostic::new(
            "a.tmd".into(),
            Some(1),
            "unknown front-matter key `x`".into(),
        )
    }

    fn suggestion_diag() -> Diagnostic {
        // Classifies as TAL-PROSE-WEASEL / SUGGESTION.
        Diagnostic::new(
            "a.tmd".into(),
            Some(1),
            "weasel word `simply` (consider cutting)".into(),
        )
    }

    #[test]
    fn errors_only_drops_warnings_but_keeps_the_default_inclusive() {
        let both = vec![error_diag(), warning_diag()];
        // Default: every diagnostic is reported + gated on.
        assert_eq!(at_severity_floor(both.clone(), Floor::Warnings).len(), 2);
        // --errors-only: warnings vanish from the reported (and thus gated) set.
        let errs = at_severity_floor(both, Floor::Errors);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].severity, taliesin_core::diagnostics::codes::ERROR);
        // A warning-only doc becomes empty under --errors-only (so the run passes).
        assert!(at_severity_floor(vec![warning_diag()], Floor::Errors).is_empty());
    }

    #[test]
    fn advice_is_always_printed_and_only_gates_under_strict() {
        let all = vec![error_diag(), warning_diag(), suggestion_diag()];
        // Printed: the default and --strict both show everything. Advice you cannot see is
        // advice you cannot act on, so the third state changes the GATE, not the output.
        assert_eq!(at_severity_floor(all.clone(), Floor::Warnings).len(), 3);
        assert_eq!(at_severity_floor(all.clone(), Floor::All).len(), 3);
        // Gated: default = error + warning; --strict = all three; --errors-only = the error.
        assert_eq!(gating(&all, Floor::Warnings).count(), 2);
        assert_eq!(gating(&all, Floor::All).count(), 3);
        assert_eq!(gating(&all, Floor::Errors).count(), 1);
    }

    #[test]
    fn an_advice_only_document_passes_the_default_gate_but_fails_strict() {
        // The whole point of the third state: a rule that suggests a reword must not turn a
        // green CI gate red, or the only way to stay green is to leave the rule off.
        let advice = vec![suggestion_diag(), suggestion_diag()];
        assert_eq!(at_severity_floor(advice.clone(), Floor::Warnings).len(), 2);
        assert_eq!(gating(&advice, Floor::Warnings).count(), 0);
        assert_eq!(gating(&advice, Floor::All).count(), 2);
        // …and the summary says so rather than calling advice a "problem" beside an exit 0.
        let s = human_summary(&advice);
        assert!(s.contains("2 suggestions"), "counts the advice: {s}");
        assert!(
            s.contains("nothing here fails the run"),
            "explains the exit code: {s}"
        );
        assert!(!s.contains("problem"), "advice is not a problem: {s}");
    }

    #[test]
    fn an_unclassified_severity_still_gates() {
        // A diagnostic nobody catalogued is not something to silently stop failing on.
        use taliesin_core::diagnostics::codes;
        assert!(codes::gates_at("nonsense-severity", codes::WARNING));
        assert_eq!(
            codes::severity_rank("nonsense-severity"),
            codes::severity_rank(codes::ERROR)
        );
    }

    #[test]
    fn build_strict_counts_defects_and_ignores_advice() {
        use taliesin_core::render::Warning;
        let ws = vec![
            Warning::new("broken cross-reference: @fig-x".to_string()),
            Warning::new("weasel word `simply` (consider cutting)".to_string()),
            Warning::new("repeated word `the`".to_string()),
        ];
        // `--strict` (and `publish`, which is strict by default) fail on the broken ref and
        // nothing else: advice is logged by the build and never blocks a release.
        assert_eq!(blocking(&ws), 1);
        assert!(is_advice(&ws[1]) && is_advice(&ws[2]) && !is_advice(&ws[0]));
    }

    fn env_fixture(runs: bool, kernel_pkg_ok: bool) -> EnvEntry {
        EnvEntry {
            lang: "python",
            path: "/usr/bin/python3".into(),
            provenance: "default".into(),
            runs: Some(runs),
            kernel_pkg: "ipykernel",
            kernel_pkg_ok: Some(kernel_pkg_ok),
            version: None,
            error: None,
            not_probed: None,
            venv_search: None,
        }
    }

    /// An entry that was resolved but never spawned (item 81): the project supplied the
    /// interpreter and the user did not opt in.
    fn env_fixture_unprobed() -> EnvEntry {
        EnvEntry {
            runs: None,
            kernel_pkg_ok: None,
            not_probed: Some("not probed: …".into()),
            ..env_fixture(false, false)
        }
    }

    #[test]
    fn require_kernel_gate_is_off_by_default_and_needs_a_used_language() {
        let ready = [env_fixture(true, true)];
        let no_interp = [env_fixture(false, false)];
        let no_pkg = [env_fixture(true, false)];
        // Off by default: never gates, even with a broken kernel.
        assert!(!kernel_gate_fails(&no_interp, false));
        // On + everything ready: passes.
        assert!(!kernel_gate_fails(&ready, true));
        // On + a used language whose interpreter or kernel package is missing: fails.
        assert!(kernel_gate_fails(&no_interp, true));
        assert!(kernel_gate_fails(&no_pkg, true));
        // On but no code cells (empty environment): nothing to require, so it passes.
        assert!(!kernel_gate_fails(&[], true));
        // An unprobed entry is "not confirmed ready", so the gate refuses rather than
        // guessing (item 81). Unreachable in practice — `--require-kernel` is the opt-in
        // that makes every entry probed — which is why it is asserted here instead.
        assert!(kernel_gate_fails(&[env_fixture_unprobed()], true));
        // …and it is not *degraded* either: nothing spawned it, so the human "kernels not
        // ready" block must not claim the interpreter was missing.
        assert!(!env_fixture_unprobed().known_not_ready());
    }
}
