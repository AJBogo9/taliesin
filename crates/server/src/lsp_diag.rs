//! The LSP's diagnostic surface beyond `publishDiagnostics`: the 3.17 **pull** model
//! (`textDocument/diagnostic` + `workspace/diagnostic`).
//!
//! **Why the pull model.** `publishDiagnostics` can only ever speak about documents the editor
//! has opened, so a 25-chapter book shows problems for the two chapters on screen and silence
//! for the other 23 — while a project lint answers the whole question in a third of a second.
//! `workspace/diagnostic` is the primitive built for exactly that, and it also inverts the
//! ownership of *invalidation*: the client asks, instead of the server guessing which open
//! buffer a `git pull` just invalidated.
//!
//! The hover used to carry a second body: a `TAL-*` code's catalogued cause and canonical fix,
//! the same rows `check --explain` printed. Both went on 2026-08-08 with the code catalogue.
//! What replaces it is the diagnostic message itself, which names the fix inline (a
//! did-you-mean, or a retirement note out of the register), so there is one text to keep true
//! instead of two.

use std::path::{Path, PathBuf};

/// The narrowest range among our diagnostics covering `pos`, so a hover that carries *only* an
/// explanation still highlights the token it is about rather than nothing.
pub(crate) fn narrowest_range_at(
    diagnostics: &[lsp_types::Diagnostic],
    pos: lsp_types::Position,
) -> Option<lsp_types::Range> {
    diagnostics
        .iter()
        .filter(|d| d.source.as_deref() == Some(crate::lint::LSP_SOURCE))
        .filter(|d| d.range.start <= pos && pos <= d.range.end)
        .min_by_key(|d| {
            (
                d.range.end.line - d.range.start.line,
                d.range
                    .end
                    .character
                    .saturating_sub(d.range.start.character),
            )
        })
        .map(|d| d.range)
}

// ---- the 3.17 pull model ---------------------------------------------------------------

/// Lint one file — an open buffer if the server holds one, otherwise the file on disk — as the
/// site page it actually is.
///
/// The same call `publish` makes, and deliberately so: `textDocument/diagnostic` and
/// `publishDiagnostics` disagreeing about the same buffer is the one thing a client cannot
/// reconcile, because it shows them in the same panel.
pub(crate) fn diagnose_file(
    sites: &mut crate::lsp_project::SiteCache,
    path: &Path,
    text: &str,
) -> Vec<lsp_types::Diagnostic> {
    let lines: Vec<&str> = text.split('\n').collect();
    let site = sites.get(path);
    crate::lint::buffer_diagnostics_in_site(path, text, site)
        .iter()
        .map(|d| d.to_lsp(&lines))
        .collect()
}

/// One page's row in a workspace report: either a fresh set of diagnostics, or "the result id
/// you already hold is still current" — which is the whole reason a client can afford to poll
/// this method.
pub(crate) enum PageReport {
    Full {
        uri: lsp_types::Url,
        result_id: String,
        items: Vec<lsp_types::Diagnostic>,
    },
    Unchanged {
        uri: lsp_types::Url,
        result_id: String,
    },
}

/// Every page of every project the open documents belong to, with the diagnostics for each.
///
/// This is the answer `check <dir>` gives, on the protocol. Pages are discovered from the open
/// buffers' enclosing projects — `workspace/diagnostic` names no file, exactly as
/// `workspace/symbol` does not, and picking one open document to stand for "the project" is a
/// coin flip the moment two are open.
///
/// A page the editor has open is linted from its **buffer**, not from disk: the Problems panel
/// must not contradict the squiggles in the window above it.
///
/// **`previous` is what makes this pollable.** A client re-issues `workspace/diagnostic`
/// whenever it feels like it, and re-linting a 25-chapter book each time would be a walk per
/// poll. Each page carries a [`result_id`] derived from the project's stat stamps and, for an
/// open page, its buffer text — so an unchanged project answers `Unchanged` for every page at
/// the cost of one `stat` each, and nothing is rendered at all.
///
/// `progress` is called as each page is decided, with `(done, total)`. It is how the one
/// genuinely long operation this server performs becomes visible: linting 25 chapters is
/// ~0.36 s of real work, and a client that asked for it with a `workDoneToken` gets told
/// where it has got to rather than being left to wonder. Called per page rather than once
/// at the end on purpose — a progress report that all arrives with the answer is a flicker,
/// not progress.
pub(crate) fn workspace_report(
    docs: &std::collections::HashMap<lsp_types::Url, String>,
    sites: &mut crate::lsp_project::SiteCache,
    previous: &std::collections::HashMap<lsp_types::Url, String>,
    progress: &mut dyn FnMut(usize, usize),
) -> Vec<PageReport> {
    let open: Vec<PathBuf> = docs.keys().filter_map(|u| u.to_file_path().ok()).collect();
    let roots = crate::lsp_project::ProjectCache::roots_of(open.iter().map(|p| p.as_path()));
    // One stamp per project, not per page: a cross-page anchor makes every page's answer
    // depend on every other page, which is exactly what `interFileDependencies` declares.
    let mut pages: Vec<(PathBuf, u64)> = Vec::new();
    for root in &roots {
        let (found, stamp) = crate::lsp_project::pages_and_stamp(root);
        pages.extend(found.into_iter().map(|p| (p, stamp)));
    }
    // A standalone buffer belongs to no project and would otherwise vanish from the panel the
    // moment the pull model replaced the push one.
    for p in &open {
        if !pages.iter().any(|(q, _)| q == p) {
            pages.push((p.clone(), 0));
        }
    }
    pages.sort();
    pages.dedup();

    let total = pages.len();
    let mut out = Vec::new();
    for (done, (page, stamp)) in pages.into_iter().enumerate() {
        progress(done, total);
        let Ok(uri) = lsp_types::Url::from_file_path(&page) else {
            continue;
        };
        let buffer = docs.get(&uri);
        let result_id = result_id(stamp, &page, buffer.map(String::as_str));
        if previous.get(&uri) == Some(&result_id) {
            out.push(PageReport::Unchanged { uri, result_id });
            continue;
        }
        let text = match buffer {
            Some(b) => b.clone(),
            None => match std::fs::read_to_string(&page) {
                Ok(t) => t,
                // A page the walk listed and the filesystem no longer has: report nothing for
                // it rather than a diagnostic about our own read failure.
                Err(_) => continue,
            },
        };
        let items = diagnose_file(sites, &page, &text);
        out.push(PageReport::Full {
            uri,
            result_id,
            items,
        });
    }
    progress(total, total);
    out
}

/// The token a client hands back to say "I already have this page's answer".
///
/// Folds in the project's stat stamp (so an edit to *any* page invalidates every page, which
/// is what a cross-page anchor means), this page's own stamp, and the buffer text when the
/// editor is holding one — because an unsaved buffer's mtime says nothing about what is on
/// screen. Never a bare mtime: two edits inside one filesystem timestamp tick would collide.
pub(crate) fn result_id(project_stamp: u64, page: &Path, buffer: Option<&str>) -> String {
    let meta = std::fs::metadata(page).ok();
    let len = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let mtime = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let own = taliesin_core::hash::fnv1a(&format!("{}:{len}:{mtime}", page.display()));
    let buf = buffer.map(taliesin_core::hash::fnv1a).unwrap_or(0);
    format!("{project_stamp:016x}-{own:016x}-{buf:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Diagnostic, Position, Range};

    /// A diagnostic of ours with a `line`-local `[start, end)` range.
    fn diag(line: u32, start: u32, end: u32) -> Diagnostic {
        Diagnostic {
            range: Range::new(Position::new(line, start), Position::new(line, end)),
            source: Some(crate::lint::LSP_SOURCE.to_string()),
            message: "m".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn the_narrowest_range_is_the_one_a_hover_highlights() {
        let r = narrowest_range_at(&[diag(0, 0, 40), diag(0, 4, 9)], Position::new(0, 5))
            .expect("a covering range");
        assert_eq!((r.start.character, r.end.character), (4, 9));
    }

    /// Another provider's diagnostic is not ours to anchor a hover on. An editor attaches
    /// several to the same buffer, and the `source` guard is the only thing separating them.
    #[test]
    fn only_our_own_diagnostics_anchor_a_hover() {
        let mut theirs = diag(0, 0, 5);
        theirs.source = Some("eslint".to_string());
        assert!(narrowest_range_at(&[theirs], Position::new(0, 2)).is_none());
    }

    #[test]
    fn a_position_outside_every_range_anchors_nothing() {
        assert!(narrowest_range_at(&[diag(3, 0, 5)], Position::new(0, 2)).is_none());
    }
}
