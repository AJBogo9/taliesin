//! The shared static-lint kernel: one definition of a document defect, for
//! `build --check-only`, `build --strict`, the live preview and the LSP alike.
//!
//! **What:** renders a file or a project in memory and returns every located diagnostic:
//! the render warning channel plus the static validators (xrefs, duplicate ids, anchors,
//! assets, media, links, reactive graph, a11y, citations, front-matter YAML). No code
//! execution, no output written; the only IO is stat-ing referenced local files.
//!
//! **How to use:** it is a library with four consumers, not a verb. `build` calls
//! [`page_static_diagnostics`] on every page it renders and counts [`blocking`] against
//! `--strict`; `build --check-only` ([`cmd_check_only`]) is the front door that reports
//! without writing; the preview server and `lsp` reach it through
//! [`buffer_diagnostics_in_site`].
//!
//! Until 2026-08-08 this was `crates/server/src/check.rs`, the implementation of a `check`
//! subcommand with five flags, an interpreter probe, an `--explain` catalogue and a 190-row
//! message-substring table that derived a `TAL-*` code and a severity for every diagnostic.
//! The verb is gone (`build --check-only` is the gate), the probe belongs to `doctor`, and
//! severity is now a field on [`taliesin_core::render::Warning`] set by the validator that
//! found the defect.
//!
//! **Depends on:** [`taliesin_core`] for rendering + the `diagnostics`/`cite` validators
//! + `Site`, [`crate::log`], and `serde_json` for the JSON formatter.

use crate::log;
use std::path::Path;
use std::process::ExitCode;
use taliesin_core::render::Severity;

/// One located diagnostic, ready to print or serialize. Under `--format json` it is
/// agent-grade: a `severity` and (for a "did you mean" typo) a structured `suggestion`
/// (`{ replacement }`). (Keys serialize alphabetically: the formatters route through
/// `serde_json::json!`, whose object is key-sorted.)
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Diagnostic {
    severity: Severity,
    file: String,
    line: Option<u32>,
    /// 1-based `[col, end_col)` character span on `line`, present only when the underlying
    /// warning located a precise token (front-matter key typos). Omitted otherwise.
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
    /// Build an **error** diagnostic, lifting any inline "did you mean" hint into a
    /// structured `suggestion`. Every caller is a hard failure the tool discovered outside a
    /// validator: an unreadable or unwritable path, malformed front-matter YAML, a cell that
    /// raised, a kernel that would not start, a project with no publishable pages. A
    /// diagnostic that came from a validator carries its own severity and goes through
    /// [`diag_from`] instead.
    pub(crate) fn new(file: String, line: Option<u32>, message: String) -> Self {
        let suggestion = taliesin_core::diagnostics::extract_suggestion(&message)
            .map(|replacement| Suggestion { replacement });
        Diagnostic {
            severity: Severity::Error,
            file,
            line,
            col: None,
            end_col: None,
            message,
            suggestion,
        }
    }

    /// Project this diagnostic to LSP for the `lsp` server. `lines` is the buffer split by
    /// [`crate::lsp_pos::lines`] (needed to clamp the line and to bound a whole-line span),
    /// and it must be that splitter and not `split('\n')`: `self.line` is comrak's line
    /// number, so an index that disagrees with comrak about where a line ends puts the
    /// squiggle on the wrong line. 1-based line → 0-based, clamped to the buffer; a precise
    /// 1-based `[col, end_col)` → 0-based when present, else the whole line.
    pub(crate) fn to_lsp(&self, lines: &[&str]) -> lsp_types::Diagnostic {
        use lsp_types::{DiagnosticSeverity, Position, Range};
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
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Suggestion => DiagnosticSeverity::HINT,
        });
        // Carry a one-click fix on `data` (the client echoes it back in a codeAction request)
        // ONLY when a suggestion has a precise column span — then `range` above is exactly the
        // token to overwrite. Without a column we cannot locate the token unambiguously, so we
        // attach nothing rather than offer an imprecise fix.
        let data = match (&self.suggestion, self.col, self.end_col) {
            (Some(s), Some(_), Some(_)) => {
                Some(serde_json::json!({ "replacement": s.replacement }))
            }
            _ => None,
        };
        lsp_types::Diagnostic {
            range,
            severity,
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
    d.severity = w.severity;
    d.col = w.col;
    d.end_col = w.end_col;
    d
}

/// Whether a render warning is **advice** rather than a defect, so `build --strict` reports
/// it and does not fail on it.
pub(crate) fn is_advice(w: &taliesin_core::render::Warning) -> bool {
    w.severity == Severity::Suggestion
}

/// How many of `warnings` block a `--strict` build (i.e. all but the advice).
pub(crate) fn blocking(warnings: &[taliesin_core::render::Warning]) -> usize {
    warnings.iter().filter(|w| !is_advice(w)).count()
}

/// Serialize just the diagnostics as `{ "diagnostics": [...] }`: the shape `build` emits
/// under `--format json`: the machine-readable surface for documents (`doctor --format
/// json`, which reports on the environment rather than the prose, is the other one).
pub(crate) fn diagnostics_json(diags: &[Diagnostic]) -> String {
    let payload = serde_json::json!({ "diagnostics": diags });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{\"diagnostics\":[]}".to_string())
}

/// Render `path` (a file or a project directory) in memory and return every located
/// diagnostic. No code execution, no output written. `Err` for an unreadable file or
/// an empty project.
///
/// `kernel_cells` is set to how many cells `build` WOULD execute, which is what the caller
/// needs to say what this run did not check.
///
/// The count rides out of the walk that already happened rather than costing a second one:
/// every page is rendered here anyway, and `Block::cell_blocks` is the one definition of
/// "which cells does this document have" (reading `block.cell` directly is the bug it
/// exists to close — it forgets every cell inside a callout or a container).
///
/// Only [`crate::exec::kernel_lang`] languages count. That is the same question `build` asks
/// before it demands an interpreter, so the number is exactly the work `--check-only` did
/// not do.
fn collect_diagnostics(path: &Path, kernel_cells: &mut usize) -> Result<Vec<Diagnostic>, String> {
    if path.is_dir() {
        // A project is what `_site.yml` declares, and `build` refuses a directory without
        // one. **The gate has to refuse it in the same breath**: a pre-publish check that
        // passes on a tree the publish step rejects is not a gate — CI runs the documented
        // command, gets "no problems found" and exit 0, and the build behind it dies. It
        // is refused here rather than at the `--check-only` dispatch so `--format json`
        // still gets an `{"error": …}` object instead of a bare stderr line and empty
        // stdout. Second consequence closed with it: without the guard,
        // `collect_site_diagnostics` treated ANY directory as its own project, including a
        // subdirectory of a real one.
        if !path.join("_site.yml").is_file() {
            return Err(crate::serve::not_a_project_error(path, "build"));
        }
        collect_site_diagnostics(path, kernel_cells)
    } else {
        // Site-aware when the file is a page of a project, so `--check-only` on a file and on
        // its project answer the same question about that page.
        let src = std::fs::read_to_string(path).map_err(|e| cannot_read(path, &e))?;
        let site = enclosing_site_of(path);
        collect_file_diagnostics_in_site(path, &src, site.as_ref(), kernel_cells)
    }
}

/// How many of a rendered document's cells `build` would run against a kernel.
fn kernel_cell_count(blocks: &[taliesin_core::Block]) -> usize {
    blocks
        .iter()
        .flat_map(taliesin_core::render::Block::cells)
        .filter(|c| crate::exec::kernel_lang(&c.lang).is_some())
        .count()
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
/// This is the single definition of the superset, so `build --check-only` and
/// `build --strict` cannot drift on what counts as a defect. It deliberately excludes the
/// two checks the callers already run themselves (`cite::validate_xrefs`, and the
/// front-matter YAML parse), so nothing is counted twice.
///
/// Run it on the document **before** its code cells execute: a matplotlib figure spliced in
/// by a cell is generated output, and linting it for alt text would report a defect the
/// author cannot fix in the source. Do not relocate the call sites in `serve_site/mod.rs`
/// or `build.rs` while changing this list.
///
/// [`Scope::InSite`] omits `validate_local_links`. An intra-site `[x](other.tmd)` link
/// rewrites to `other.html`, and only the site's page registry knows the real URLs, so on
/// a site page that rule reports every internal link as broken. `Site::validate_cross_page_links`
/// is its site-aware counterpart, run once over the whole project.
pub(crate) fn page_static_diagnostics(
    src: &str,
    blocks: &[taliesin_core::Block],
    base: &Path,
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
    out.extend(dx::validate_a11y(blocks));
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

/// Lint an already-in-hand source buffer as if it were the file at `path` — the seam the
/// LSP uses to lint an editor buffer (unsaved edits) instead of the last-saved file.
/// `path` supplies the base dir (relative includes/assets/links) + the reported location;
/// the file on disk is never read. `site` names the project the page belongs to.
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
/// project above it (or for one inside a project but deliberately not one
/// of `site.pages`, so the site rules would remove its link check and put nothing back).
fn collect_file_diagnostics_in_site(
    path: &Path,
    src: &str,
    site: Option<&taliesin_core::Site>,
    kernel_cells: &mut usize,
) -> Result<Vec<Diagnostic>, String> {
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let doc = taliesin_core::render_single_doc(src, base);
    *kernel_cells += kernel_cell_count(&doc.blocks);
    let path_str = path.display().to_string();
    // A document inside a site project may legitimately refer across its pages, so
    // resolve what the project defines before calling anything broken: this path is the
    // editor's every-keystroke validator, and it used to report every valid cross-page
    // `@sec-`/`@fig-`/`@tbl-` as an error while the same tree linted clean as a project.
    // Outside a project the scan is empty and nothing changes.
    let elsewhere = taliesin_core::site::anchors_defined_elsewhere_in_project(path);
    // `src` so a broken `@ref` is squiggled under the token and not across the line, and
    // so the did-you-mean it already computes can become a one-click fix (`to_lsp`
    // attaches that payload only for a precisely-columned diagnostic).
    let xref =
        taliesin_core::cite::validate_xrefs_known_elsewhere(&doc.blocks, &elsewhere, Some(src));
    let page = site.and_then(|s| s.page_for_input(path));
    let scope_kind = if page.is_some() {
        Scope::InSite
    } else {
        Scope::Standalone
    };
    let statics = page_static_diagnostics(src, &doc.blocks, base, scope_kind);
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
/// [`collect_file_diagnostics_in_site`] so that one place decides it: a `draft: true`
/// chapter sits inside a project and is still linted standalone.
///
/// `DraftMode::Exclude`, matching a project lint rather than the preview.
///
/// Discovery costs a full walk, and it is paid per call — far too slow to pay per keystroke,
/// so the language server passes a stat-validated `lsp_project::SiteCache` instead of
/// calling this. A file outside any project costs nothing.
fn enclosing_site_of(path: &Path) -> Option<taliesin_core::Site> {
    let root = taliesin_core::site::enclosing_site_root_across_git(path.parent()?)?;
    Some(taliesin_core::Site::discover(&root))
}

/// Lint an in-memory editor buffer as if it were the file at `path`, returning the
/// diagnostics directly. Used by the `lsp` server on every `didOpen`/`didChange`. The buffer
/// path can't fail to render, but a hypothetical error surfaces as one line-1 diagnostic
/// rather than vanishing.
///
/// `site` is the enclosing project or `None`; see [`collect_file_diagnostics_in_site`].
pub(crate) fn buffer_diagnostics_in_site(
    path: &Path,
    src: &str,
    site: Option<&taliesin_core::Site>,
) -> Vec<Diagnostic> {
    // The LSP publishes diagnostics, not a summary line, so the cell count has no reader
    // here and is discarded.
    match collect_file_diagnostics_in_site(path, src, site, &mut 0) {
        Ok(diags) => diags,
        Err(e) => vec![Diagnostic::new(path.display().to_string(), Some(1), e)],
    }
}

/// Every located diagnostic in a project, page by page, in `site.pages` order.
///
/// Renders each page the way the build and the preview do, with the project's own
/// `render_defaults()` and chapter scoping, or it reports diagnostics for a document nobody
/// ships (a page inheriting the project `bibliography:` looked uncited) and prints numbers no
/// reader sees ("Theorem 2.3").
fn collect_site_diagnostics(
    root: &Path,
    kernel_cells: &mut usize,
) -> Result<Vec<Diagnostic>, String> {
    let site = taliesin_core::Site::discover(root);
    if site.pages.is_empty() {
        return Err(format!("no .tmd pages found under {}", root.display()));
    }
    // Drafts (`draft: true`) are held out of the published set and so out of this lint. A
    // lint that reports nothing is read as "this project is clean", so the omission is
    // printed rather than left implicit; the live preview lints them where they are written.
    if let Some(line) = crate::build::draft_report_line(&site.excluded_drafts) {
        log::info(&line);
    }
    // No filter for the "no `_site.yml`" advisory: `collect_diagnostics` has already
    // refused a directory that has none, so `discover` cannot raise it here. It used to be
    // filtered out on the grounds that a bare directory of pages was a legitimate project
    // — the pre-wave-13 stance, and the second half of why the gate passed on a tree
    // `build` refuses.
    let mut out: Vec<Diagnostic> = site
        .warnings
        .iter()
        .map(|m| Diagnostic::new("_site.yml".to_string(), None, m.clone()))
        .collect();
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
        let doc = taliesin_core::render_document_scoped_with_site(
            &src,
            base,
            site.chapter_for(page),
            Some(&defaults),
        );
        *kernel_cells += kernel_cell_count(&doc.blocks);
        // Static lints over the page's blocks (xrefs are added by render_page_doc_warned
        // below); run before `doc` is consumed.
        for w in &page_static_diagnostics(&src, &doc.blocks, base, Scope::InSite) {
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
    // The `_site.yml` chrome's own hrefs. The loop above harvests links out of rendered
    // page *bodies*, and a `nav:`/`footer:` href never passes through one — it goes
    // straight from the config onto every page, so a typo there was the one broken link
    // class nothing checked and the one that ships site-wide.
    for w in site.validate_chrome_links() {
        out.push(diag_from(&w, "_site.yml"));
    }
    // Hygiene for the project-wide `bibliography:`, reported against `_site.yml` because
    // that is where it is declared. Unused-entry is site-wide by necessity: a shared entry
    // one page cites is used, however many pages leave it alone.
    for w in site.validate_shared_bibliography() {
        out.push(diag_from(&w, "_site.yml"));
    }
    Ok(out)
}

/// Serialize a `--format json` failure (an unreadable path, an empty project) as a
/// single `{"error": "<message>"}` object, so the JSON stream a caller pipes to `jq`
/// stays valid even when nothing could be linted. The message is JSON-escaped (quotes,
/// newlines), never raw-concatenated.
pub(crate) fn json_error(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// The path to print for one diagnostic: the one the reader would type from the directory
/// they ran the command in.
///
/// A project's diagnostics are located against the **project root** (`sub/page.tmd`), which
/// names nothing from anywhere else: run from the repo, that printed `sub/page.tmd:5:`, a path
/// no terminal can open and no editor can resolve. Re-rooting it on the target *as typed*
/// gives `docs/guide/sub/page.tmd:5:` — the tsc/cargo convention, and what a problem-matcher
/// resolving against the invocation directory needs.
///
/// `root` is `None` for the single-file path, which already reports the path as
/// given; re-rooting it onto its own directory would double the prefix. A `.` target keeps
/// printing a bare `sub/page.tmd`, so every existing grep of that output still matches.
///
/// The JSON format is deliberately untouched: its consumer passed the root itself and resolves
/// against it, so a path relative to the project is the right contract there.
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

/// The severity word a human line prints.
fn severity_word(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Suggestion => "suggestion",
    }
}

/// Greppable linter lines: `file:line: severity: message`. The `file:line:` prefix stays
/// first (the linter convention VS Code problem-matchers / gcc / tsc key off), with the
/// severity word before the message (the gcc/clang shape).
///
/// `root` re-roots each path onto the target as typed; see [`displayed_path`].
fn format_human(diags: &[Diagnostic], color: bool, root: Option<&Path>) -> String {
    let mut s = String::new();
    for d in diags {
        // Paint just the severity word (rustc/cargo/tsc all colorize severity). `color` is
        // TTY-gated by the caller, so the non-TTY greppable contract stays byte-identical —
        // the `file:line: severity:` shape a problem-matcher keys off is untouched.
        let word = severity_word(d.severity);
        let sev = if color {
            let code = match d.severity {
                Severity::Error => "\x1b[31m",      // red
                Severity::Warning => "\x1b[33m",    // yellow
                Severity::Suggestion => "\x1b[90m", // grey
            };
            format!("{code}{word}\x1b[0m")
        } else {
            word.to_string()
        };
        let file = displayed_path(&d.file, root);
        match d.line {
            Some(l) => s.push_str(&format!("{}:{}: {}: {}\n", file, l, sev, d.message)),
            None => s.push_str(&format!("{}: {}: {}\n", file, sev, d.message)),
        }
    }
    s
}

/// The summary line printed after the per-diagnostic block: a per-severity breakdown
/// (`3 problems (1 error, 2 warnings)`) keeping the leading `N problem(s)` token so existing
/// greps still match. Pure, so the split is unit-testable.
fn human_summary(diags: &[Diagnostic], kernel_cells: usize) -> String {
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    if diags.is_empty() {
        // `--check-only` is the pre-publish gate, and it is STATIC: it never starts a
        // kernel. On a project whose `build` exits 1 for want of an interpreter, a bare
        // "no problems found" told the author the project was clean and then the publish
        // build failed. The static superset is right — wave 9 removed the interpreter probe
        // deliberately and this does not put it back — so the sentence is what moves: it
        // stops claiming completeness and names the command that does the rest.
        //
        // Only cells `build` would actually run are counted. A `{js}`/`{mermaid}` cell runs
        // in the reader's browser, so calling it skipped work would be its own false claim.
        //
        // With nothing executable the message is byte-identical to what it always was: a
        // prose-only project must not gain noise it cannot act on.
        return match kernel_cells {
            0 => "no problems found\n".to_string(),
            n => format!(
                "no static problems found  ·  {n} code cell{} not run \
                 (build without --check-only to execute {})\n",
                plural(n),
                if n == 1 { "it" } else { "them" }
            ),
        };
    }
    let count = |s: Severity| diags.iter().filter(|d| d.severity == s).count();
    let errors = count(Severity::Error);
    let warnings = count(Severity::Warning);
    let suggestions = count(Severity::Suggestion);
    let mut parts = Vec::new();
    for (n, word) in [
        (errors, "error"),
        (warnings, "warning"),
        (suggestions, "suggestion"),
    ] {
        if n > 0 {
            parts.push(format!("{n} {word}{}", plural(n)));
        }
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
    s
}

/// Which reported diagnostics **fail** the run: everything but advice, or everything at all
/// under `--strict`. Advice is always printed (advice you cannot see is advice you cannot act
/// on) and only gates when the run asks for it, which is the whole point of the third severity.
fn gating(diags: &[Diagnostic], strict: bool) -> usize {
    diags
        .iter()
        .filter(|d| strict || d.severity != Severity::Suggestion)
        .count()
}

/// `build <file|dir> --check-only`: render in memory, report every located diagnostic, write
/// nothing, and exit non-zero if any of them gates (a CI / pre-publish gate). Static only:
/// no kernel is started and no cell runs, so `--no-exec` is implied rather than needed.
///
/// This is the front door the retired `check` verb was: ~40 lines over the same
/// [`collect_diagnostics`] kernel `build` itself uses, instead of a second subcommand with
/// its own flag set, interpreter probe and code catalogue. `--format json` is the tool's one
/// machine-readable surface; `--strict` widens the gate to include advice.
pub(crate) fn cmd_check_only(target: &Path, format: &str, strict: bool) -> ExitCode {
    // Guard the render: a panic in core rendering becomes a clean located error + non-zero
    // exit (routed through the same error path, so `--format json` stays valid) instead of
    // a raw abort that would crash a CI gate.
    let mut kernel_cells = 0usize;
    let collected = crate::serve::guarded(|| collect_diagnostics(target, &mut kernel_cells))
        .map_err(|panic| format!("render panicked on {}: {panic}", target.display()))
        .and_then(|r| r);
    let diags = match collected {
        Ok(d) => d,
        // Honour `--format json` on the error path too: a human stderr line would corrupt a
        // `--format json | jq` stream (and leave stdout empty), so emit an `{"error": …}`
        // object to stdout. Human format keeps the stderr message.
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
        println!("{}", diagnostics_json(&diags));
    } else {
        // Greppable `path:line: severity: message` lines to stderr (linter-style), then the
        // per-severity summary. The severity word is colorized only at a TTY (and not under
        // NO_COLOR), so piped/redirected output stays byte-identical for problem-matchers.
        let color = std::io::IsTerminal::is_terminal(&std::io::stderr())
            && std::env::var_os("NO_COLOR").is_none();
        eprint!("{}", format_human(&diags, color, human_root(target)));
        eprint!("{}", human_summary(&diags, kernel_cells));
    }
    if gating(&diags, strict) == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
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

    /// The broken-cross-reference findings, matched on the message rather than on a code:
    /// the `TAL-*` catalogue went on 2026-08-08, and this phrase is the one `cite::validate`
    /// emits (both its did-you-mean and its no-such-target arm open with it).
    fn broken_xrefs(diags: &[Diagnostic]) -> Vec<&str> {
        diags
            .iter()
            .filter(|d| d.message.starts_with("broken cross-reference:"))
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
            diags
                .iter()
                .any(|d| d.message.starts_with("broken link anchor:")),
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
            .filter(|d| d.message.starts_with("broken link:"))
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
            diags
                .iter()
                .any(|d| d.message.starts_with("broken link anchor:")),
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
            !after
                .iter()
                .any(|d| d.message.starts_with("broken link anchor:")),
            "a link fixed in the buffer must clear before saving: {:?}",
            after.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// The claim `docs/guide/reference/cli.tmd` makes about `taliesin lsp`, pinned: the same
    /// validators the project lint runs, on the unsaved buffer. Measured false before this:
    /// the buffer path missed the broken-cross-page-anchor rule entirely and described a
    /// broken link with a rule that does not apply to a site page, so it is asserted rather
    /// than restated.
    ///
    /// Compared as a set of message *openings* for one page, not message-for-message: a
    /// project lint renders each page with the project's numbering and defaults, so a message
    /// may name "Figure 2.1" where the buffer path says "Figure 1". What must not differ is
    /// which defects each one finds. (It was a set of `TAL-*` codes until the catalogue went
    /// on 2026-08-08; the message opening is the same partition by another name.)
    #[test]
    fn the_editor_finds_the_same_defects_on_a_page_as_a_project_lint_does() {
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

        // The family, not the whole sentence: everything up to the first backtick or colon.
        let family = |d: &Diagnostic| {
            let m = &d.message;
            m[..m.find(['`', ':']).unwrap_or(m.len())]
                .trim()
                .to_string()
        };
        let mut site_codes: Vec<String> = collect_diagnostics(&dir, &mut 0)
            .expect("site ok")
            .iter()
            .filter(|d| d.file.contains("page.tmd"))
            .map(family)
            .collect();
        let mut buffer_codes: Vec<String> =
            buffer_diagnostics(&page, &src).iter().map(family).collect();
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
            "the editor and `build --check-only` disagree about what is wrong with this page"
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

    /// The end-to-end payoff of a columned xref diagnostic: the one-click fix appears.
    ///
    /// **Fable audit FA30.** `to_lsp` attaches the fix payload ONLY when a suggestion has a
    /// precise span, and the xref validator filed whole-line warnings, so the did-you-mean
    /// it had already computed could never become a "Change to `@fig-results`" code action.
    /// This runs the real editor path: buffer text in, LSP diagnostics out.
    #[test]
    fn a_broken_xref_offers_its_did_you_mean_as_a_one_click_fix() {
        let dir = std::env::temp_dir().join(format!("tali-xref-fix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("refs.tmd");
        let src = "---\ntitle: T\n---\n\n\
                   ![cap](a.png){#fig-results}\n\n\
                   A near-miss reference: see @fig-reslts.\n";
        std::fs::write(&path, src).unwrap();

        let diags = super::buffer_diagnostics_in_site(&path, src, None);
        let d = diags
            .iter()
            .find(|d| d.message.contains("broken cross-reference"))
            .unwrap_or_else(|| panic!("no broken-xref diagnostic: {diags:?}"));
        let lines: Vec<&str> = crate::lsp_pos::lines(src).collect();
        let lsp = d.to_lsp(&lines);
        assert_eq!(
            lsp.data,
            Some(serde_json::json!({ "replacement": "@fig-results" })),
            "the columned diagnostic must carry the fix the message already names: {lsp:?}"
        );
        // And the range is the token, so applying that replacement is correct.
        let line = lines[lsp.range.start.line as usize];
        let (a, b) = (
            lsp.range.start.character as usize,
            lsp.range.end.character as usize,
        );
        assert_eq!(&line[a..b], "@fig-reslts", "in line {line:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn to_lsp_uses_a_precise_span_when_columned() {
        let d = super::Diagnostic {
            severity: Severity::Warning,
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
        assert_eq!(lsp.source.as_deref(), Some("taliesin"));
        // The `TAL-*` code and its `code_description` link went with the catalogue on
        // 2026-08-08: a code an editor shows with nothing to look it up in is a token the
        // author cannot act on, and the link opened a browser out of the editor.
        assert!(lsp.code.is_none(), "no code: {lsp:?}");
        assert!(lsp.code_description.is_none(), "no doc link: {lsp:?}");
    }

    #[test]
    fn to_lsp_columns_are_utf16_when_an_astral_char_precedes_the_token() {
        // The `check` validators produce 1-based *character* columns. LSP columns are UTF-16
        // code units, so an astral char (😀 = 2 UTF-16 units) before the token must shift the
        // emitted column by one extra unit. `😀tittle`: `tittle` is char cols [2,8), which is
        // UTF-16 cols [2,8) (0-based) once the emoji's second unit is counted.
        let d = super::Diagnostic {
            severity: Severity::Warning,
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
            severity: Severity::Error,
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
            severity: Severity::Warning,
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
            severity: Severity::Error,
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

    /// The human line carries the severity word and the JSON carries the severity field, and
    /// neither carries a `TAL-*` code or a `docs_url` any more: the catalogue those pointed
    /// into went on 2026-08-08, and a code with nothing to look it up in is a token an author
    /// cannot act on.
    #[test]
    fn severity_reaches_both_channels_and_no_code_or_url_survives() {
        let d = diag_from(
            &taliesin_core::render::Warning::new("broken cross-reference: @fig-x")
                .at(Some("a.tmd".into()), 2)
                .severity(taliesin_core::Severity::Error),
            "a.tmd",
        );
        let json = diagnostics_json(std::slice::from_ref(&d));
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let one = &parsed["diagnostics"][0];
        assert_eq!(one["severity"], "error", "{json}");
        assert!(one.get("code").is_none(), "no code key: {json}");
        assert!(one.get("docs_url").is_none(), "no docs_url key: {json}");
        let human = format_human(std::slice::from_ref(&d), false, None);
        assert_eq!(
            human, "a.tmd:2: error: broken cross-reference: @fig-x\n",
            "the greppable `file:line: severity: message` shape, with no bracket"
        );
    }

    #[test]
    fn collect_diagnostics_flags_frontmatter_typo_and_broken_xref() {
        let dir = tmp("check-file");
        let f = dir.join("doc.tmd");
        fs::write(&f, "---\ntitle: T\ntitel: oops\n---\n\nSee @fig-nope.\n").unwrap();
        let diags = collect_diagnostics(&f, &mut 0).expect("ok");
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
        // This pins `collect_file_diagnostics_in_site`, the seam `taliesin lsp` calls
        // through `buffer_diagnostics` on every didOpen/didChange.
        let dir = tmp("check-buffer");
        let f = dir.join("doc.tmd");
        fs::write(&f, "---\ntitle: Clean\n---\n\nAll good on disk.\n").unwrap();
        let buffer = "---\ntitle: T\ntitel: oops\n---\n\nUnsaved buffer.\n";
        let diags = collect_file_diagnostics_in_site(&f, buffer, None, &mut 0).expect("ok");
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
        let diags = collect_diagnostics(&f, &mut 0).expect("ok");
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
        let diags = collect_diagnostics(&f, &mut 0).expect("ok");
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
        let diags = collect_diagnostics(&dir, &mut 0).expect("site ok");
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
            let diags = collect_diagnostics(t, &mut 0).unwrap_or_default();
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
             <video src=\"clip.mp4\"></video>\n\n\
             ```{js}\n//| input: nope\nreturn nope;\n```\n\n\
             ```{js}\n//| name: a\n//| input: b\nreturn b;\n```\n\n\
             ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&f, &mut 0).expect("ok");
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
    fn collect_diagnostics_surfaces_a11y_rules() {
        // One doc tripping each surviving static a11y rule: a raw `<img>` with no alt and an
        // authored `##`->`####` heading skip. Both must be surfaced, located, while an
        // `alt`-bearing image and a single-level heading step are left alone. The doc has a
        // title block, so heading demotion renders `##`/`####` as h3/h5: the skip is preserved
        // (difference-invariant) and reported at the shipped levels.
        let dir = tmp("lint-a11y");
        let f = dir.join("doc.tmd");
        fs::write(
            &f,
            "---\ntitle: T\n---\n\n\
             ## Section\n\n\
             <img src=\"raw.png\">\n\n\
             ![described](ok.png) and a [real link](page.html).\n\n\
             #### Skips a level\n",
        )
        .unwrap();
        let diags = collect_diagnostics(&f, &mut 0).expect("ok");
        let has = |needle: &str| diags.iter().any(|d| d.message.contains(needle));
        assert!(has("image is missing alt text"), "raw img: {diags:?}");
        assert!(
            has("heading level skips from h2 to h4"),
            "heading skip: {diags:?}"
        );
        // The markdown image (auto-alt) must NOT be flagged.
        assert_eq!(
            diags
                .iter()
                .filter(|d| d.message.contains("image is missing alt text"))
                .count(),
            1,
            "only the raw alt-less img: {diags:?}"
        );
        assert!(
            diags
                .iter()
                .filter(|d| d.message.contains("missing alt text")
                    || d.message.contains("heading level skips"))
                .all(|d| d.line.is_some() && d.file.contains("doc.tmd")),
            "a11y diagnostics located to file+line: {diags:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corpus_a11y_pin_doc_trips_each_rule_through_the_front_door() {
        // The corpus pin (`corpus/diagnostics/a11y.tmd`, exempt from the no-false-positive
        // guard) must fire every surviving a11y rule through the real `collect_diagnostics`
        // flow, not only through a unit test over hand-written HTML.
        let doc = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnostics/a11y.tmd");
        let diags = collect_diagnostics(&doc, &mut 0).expect("pin doc lints");
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
        // The accessible-name rules went on 2026-08-08, so the pin must not still be
        // reporting them: a leftover finding here would mean a leftover emitter.
        assert!(
            !has("has no accessible name") && !has("disagrees with its visible text"),
            "the accessible-name family is gone: {diags:?}"
        );
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
        let diags = collect_diagnostics(&dir, &mut 0).expect("site ok");
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
        assert!(collect_diagnostics(&f, &mut 0).expect("ok").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_without_site_yml_is_refused_exactly_as_build_refuses_it() {
        // The gate has one job: fail on what the publish step would fail on. It used to
        // treat a bare directory of pages as its own project and print "no problems
        // found" on a tree `build` flatly refuses, so CI went green and the deploy died.
        // (This test asserted the opposite until 2026-08-13. Its premise — "a bare page
        // directory is a legitimate project" — was the pre-wave-13 stance, reversed in
        // `build` and never propagated here.)
        let dir = tmp("check-nositeyml");
        fs::write(
            dir.join("index.tmd"),
            "---\ntitle: Home\n---\n\nClean prose.\n",
        )
        .unwrap();
        let err = collect_diagnostics(&dir, &mut 0)
            .expect_err("a directory with no _site.yml is not a project");
        assert!(
            err.contains("no _site.yml"),
            "and says so in the same words `build` uses: {err}"
        );

        // A *malformed* `_site.yml` is a real problem, and still counted rather than
        // turned into the refusal above: the file is there, it is the content that is wrong.
        fs::write(dir.join("_site.yml"), "title: \"unterminated\n").unwrap();
        let diags = collect_diagnostics(&dir, &mut 0).expect("still discoverable");
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
        assert!(
            collect_diagnostics(&dir, &mut 0).is_err(),
            "empty site -> Err"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_human_lists_located_lines() {
        // The `file:line:` linter prefix stays first, with the severity word before the
        // message (located and unlocated alike). No `[CODE]` bracket: the catalogue a code
        // pointed into went on 2026-08-08.
        let diags = vec![
            Diagnostic::new("a.tmd".into(), Some(3), "m1".into()),
            Diagnostic::new("b.tmd".into(), None, "m2".into()),
        ];
        let text = format_human(&diags, false, None);
        assert!(
            text.contains("a.tmd:3: error: m1"),
            "located line carries the severity word: {text}"
        );
        assert!(
            text.contains("b.tmd: error: m2"),
            "unlocated line carries the severity word: {text}"
        );
    }

    #[test]
    fn a_project_lint_prints_paths_relative_to_where_the_command_ran() {
        // A project's diagnostics are located against the PROJECT ROOT. Printed bare, they name
        // nothing: run from the repo, `docs/guide` said `sub/page.tmd:5:`, which no terminal
        // can open and which an editor's problem-matcher resolves onto a file that does not
        // exist. Both the located and the unlocated line carry the prefix: the unlocated form
        // is how a `_site.yml` finding is reported, and dropping the prefix from just that one
        // is the shape this asserts against.
        let diags = vec![
            Diagnostic::new("sub/page.tmd".into(), Some(5), "m1".into()),
            Diagnostic::new("_site.yml".into(), None, "m2".into()),
        ];
        let text = format_human(&diags, false, Some(Path::new("docs/guide")));
        assert!(
            text.contains("docs/guide/sub/page.tmd:5: error: m1"),
            "located line names the path the reader would type: {text}"
        );
        assert!(
            text.contains("docs/guide/_site.yml: error: m2"),
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
        assert!(plain.contains("a.tmd:3: error: m1"));
        let colored = format_human(&diags, true, None);
        assert!(
            colored.contains("\x1b[31merror\x1b[0m"),
            "severity must be painted: {colored:?}"
        );
        // Only the severity word is wrapped; the file:line prefix stays bare.
        assert!(colored.contains("a.tmd:3: \x1b[31merror\x1b[0m: m1"));
    }

    #[test]
    fn human_summary_splits_by_severity() {
        // Mixed set: a broken xref (error) + an unknown front-matter key (warning). The summary
        // keeps the leading `N problem(s)` token a grep matches, and breaks it out per severity.
        let diags = vec![error_diag(), warning_diag()];
        let s = human_summary(&diags, 0);
        assert!(s.contains("2 problems"), "leading count kept: {s}");
        assert!(
            s.contains("(1 error, 1 warning)"),
            "per-severity breakdown: {s}"
        );
        // The `--explain <CODE>` footer went with the code catalogue on 2026-08-08: there is
        // nothing to look a code up in, so pointing at one would be a dead end.
        assert!(!s.contains("--explain"), "no dead footer: {s}");
        // A single warning pluralizes correctly.
        let one = human_summary(std::slice::from_ref(&diags[1]), 0);
        assert!(one.contains("1 problem (1 warning)"), "singular: {one}");
        let none = human_summary(&[], 0);
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
    /// (no native `_site.yml`, no `.tmd` pages) gets the ordinary not-a-project refusal,
    /// which never names Quarto.
    #[test]
    fn quarto_only_dir_gets_generic_diagnostic_not_a_breadcrumb() {
        // Neutral dir name: the diagnostic echoes the path, so a "quarto" in the dir name
        // would be a false positive for the breadcrumb we are asserting is gone.
        let dir = tmp("legacy-config-only");
        fs::write(dir.join("_quarto.yml"), "project:\n  type: website\n").unwrap();

        let err = collect_diagnostics(&dir, &mut 0).expect_err("a page-less dir is an error");
        assert!(
            !err.to_lowercase().contains("quarto"),
            "no Quarto breadcrumb should remain: {err}"
        );
        // It used to be the site-walker's "no .tmd pages", which is the second thing
        // wrong with this directory; the first is that it is not a project at all. That
        // branch is still exercised, by `collect_diagnostics_empty_site_is_err` — a
        // `_site.yml` with no pages beside it.
        assert!(
            err.contains("no _site.yml"),
            "expected the not-a-project refusal: {err}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // --- the exit gate ---

    /// A diagnostic at each severity, built the way the validators build them: severity is a
    /// field on the `Warning` now, not a classification of its message.
    fn at(severity: taliesin_core::Severity, message: &str) -> Diagnostic {
        diag_from(
            &taliesin_core::render::Warning::new(message.to_string())
                .at(Some("a.tmd".into()), 1)
                .severity(severity),
            "a.tmd",
        )
    }
    fn error_diag() -> Diagnostic {
        at(
            taliesin_core::Severity::Error,
            "broken cross-reference: @fig-x",
        )
    }
    fn warning_diag() -> Diagnostic {
        at(
            taliesin_core::Severity::Warning,
            "unknown front-matter key `x`",
        )
    }
    fn suggestion_diag() -> Diagnostic {
        at(
            taliesin_core::Severity::Suggestion,
            "bibliography entry `@x` is declared but never cited",
        )
    }

    /// Advice is always printed and only gates when the run asks. That is the whole point of
    /// the third severity: a rule whose fix is "consider rewording" must not turn a green CI
    /// gate red, or the only way to stay green is to leave the rule off.
    #[test]
    fn advice_is_always_printed_and_only_gates_under_strict() {
        let all = vec![error_diag(), warning_diag(), suggestion_diag()];
        // Printed: `format_human` shows everything at either setting, because advice you cannot
        // see is advice you cannot act on, so `--strict` changes the GATE, not the output.
        assert_eq!(format_human(&all, false, None).lines().count(), 3);
        assert_eq!(gating(&all, false), 2, "default: the error and the warning");
        assert_eq!(gating(&all, true), 3, "--strict: the advice too");
    }

    #[test]
    fn an_advice_only_document_passes_the_default_gate_but_fails_strict() {
        let advice = vec![suggestion_diag(), suggestion_diag()];
        assert_eq!(gating(&advice, false), 0);
        assert_eq!(gating(&advice, true), 2);
        // …and the summary says so rather than calling advice a "problem" beside an exit 0.
        let s = human_summary(&advice, 0);
        assert!(s.contains("2 suggestions"), "counts the advice: {s}");
        assert!(
            s.contains("nothing here fails the run"),
            "explains the exit code: {s}"
        );
        assert!(!s.contains("problem"), "advice is not a problem: {s}");
    }

    /// A clean run on a document with executable cells must not claim more than it checked.
    ///
    /// Measured on a fresh `init` project with one `{python}` cell and no `ipykernel`:
    ///
    /// ```text
    /// build .                        exit=1   "no python kernel available, …"
    /// build . --check-only           exit=0   "no problems found"
    /// build . --check-only --strict  exit=0   "no problems found"
    /// ```
    ///
    /// `CLAUDE.md` and the User Guide both call `--check-only` **the pre-publish gate**, so
    /// an author runs it, is told the project is clean, and the publish build fails. The
    /// static superset is correct — wave 9 removed the interpreter probe on purpose, and
    /// this does NOT put it back. The lie is in the sentence, so the sentence is what moves.
    #[test]
    fn a_clean_run_names_the_cells_it_did_not_execute() {
        // Nothing executable: the message must not gain noise a prose project cannot act on.
        assert_eq!(human_summary(&[], 0), "no problems found\n");

        let one = human_summary(&[], 1);
        assert!(
            one.contains("no static problems found"),
            "it stops claiming completeness: {one}"
        );
        assert!(
            one.contains("1 code cell not run"),
            "it names what it skipped, singular: {one}"
        );
        assert!(
            one.contains("--check-only"),
            "it names the command that does execute them: {one}"
        );
        assert!(
            human_summary(&[], 4).contains("4 code cells not run"),
            "plural"
        );

        // A run that DID find something already says what it found; the qualifier is for the
        // success path, where "no problems found" is the whole message.
        let with_problems = human_summary(std::slice::from_ref(&error_diag()), 3);
        assert!(
            with_problems.contains("1 problem"),
            "unchanged: {with_problems}"
        );
    }

    /// The count is of cells `build` would EXECUTE, not of every fenced cell: a `{js}` or
    /// `{mermaid}` cell runs in the reader's browser, so `build` does not run it either and
    /// naming it as skipped work would be its own false statement.
    #[test]
    fn only_kernel_language_cells_are_counted_as_not_run() {
        let dir = tmp("check-cellcount");
        let f = dir.join("doc.tmd");
        fs::write(
            &f,
            "---\ntitle: T\n---\n\n\
             ```{python}\nprint(1)\n```\n\n\
             ```{js}\nreturn 1;\n```\n\n\
             ```{mermaid}\ngraph TD; a-->b;\n```\n\n\
             ```python\nnot a cell, just a fence\n```\n",
        )
        .unwrap();
        let mut cells = 0;
        let diags = collect_diagnostics(&f, &mut cells).expect("ok");
        assert!(diags.is_empty(), "the fixture is clean: {diags:?}");
        assert_eq!(
            cells, 1,
            "only the `{{python}}` cell is work `build` would do"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Every direct `Diagnostic::new` call site is a hard failure the tool found outside a
    /// validator (an unreadable path, malformed YAML, a cell that raised), so the constructor
    /// is an ERROR and gates by default. Until 2026-08-08 that came from an unclassified
    /// message falling through to `(GENERIC, ERROR)`; it is explicit now, and this pins it,
    /// because a constructor that defaulted to `Warning` would silently stop failing a
    /// `--check-only` run on a page it could not read.
    #[test]
    fn a_hard_failure_gates_without_being_classified() {
        let d = Diagnostic::new("a.tmd".into(), None, "cannot read a.tmd".into());
        assert_eq!(d.severity, taliesin_core::Severity::Error);
        assert_eq!(gating(std::slice::from_ref(&d), false), 1);
    }

    #[test]
    fn build_strict_counts_defects_and_ignores_advice() {
        use taliesin_core::Severity;
        use taliesin_core::render::Warning;
        let ws = vec![
            Warning::new("broken cross-reference: @fig-x".to_string()).severity(Severity::Error),
            Warning::new("unknown front-matter key `x`".to_string()),
            Warning::new("bibliography entry `@x` is declared but never cited".to_string())
                .severity(Severity::Suggestion),
        ];
        // `--strict` fails on the broken ref and the unknown key, never on the advice: a
        // shared `.bib` whose entries most pages leave alone would otherwise be unusable.
        assert_eq!(blocking(&ws), 2);
        assert!(is_advice(&ws[2]) && !is_advice(&ws[0]) && !is_advice(&ws[1]));
    }
}
